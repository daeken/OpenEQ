//! Authored liquid volumes in asset/scene coordinates (Z up).
//!
//! These queries use explicit WLD / heightmap DAT region tags and bounded geometry.
//! Rendered water materials alone never establish a swimming volume.
use std::{collections::BTreeMap, path::Path, sync::Arc};

use glam::{DQuat, DVec3};

mod height_invariant;

use crate::{
    Error, Result,
    binary_regions::BinaryRegions,
    bsp_regions::{BspRegions, Node},
    loader::{read_eqg_declaration, read_heightmap, read_terrain_group, zone_archive},
    pfs::Archive,
    terrain::{Heightmap, regions::NativeTopLevelRegions},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LiquidKind {
    Water,
    Lava,
    FreezingWater,
    /// WLD SLN: water that also blocks visibility, not a slippery floor.
    OpaqueWater,
}

/// A finite authored box. Rotation is a normalized quaternion (X, Y, Z, W).
#[derive(Debug, Clone, Copy)]
pub struct LiquidBox {
    pub kind: LiquidKind,
    pub center: [f32; 3],
    pub half_extents: [f32; 3],
    pub rotation: [f32; 4],
}

/// Positive-length portion of a segment inside one liquid, expressed as
/// fractions of `from + (to - from) * t`. Adjacent same-kind spans are merged.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LiquidSpan {
    pub kind: LiquidKind,
    pub enter: f32,
    pub exit: f32,
}

#[derive(Debug)]
struct OrientedBox {
    kind: LiquidKind,
    center: DVec3,
    half_extents: DVec3,
    inverse_rotation: DQuat,
}

#[derive(Debug, Default)]
enum Volumes {
    #[default]
    Empty,
    Bsp {
        nodes: Vec<Node>,
        kinds: Vec<Option<LiquidKind>>,
        /// Allows segment queries to prune entire dry subtrees.
        wet: Vec<bool>,
    },
    Boxes(Vec<OrientedBox>),
    /// Every native record is retained: a dry/unknown winner masks later water.
    /// Unknown means unsupported evidence, not a recovered dry classification.
    NativeTerrain(NativeTopLevelRegions),
    NativeBinary(BinaryRegions),
}

/// Cheaply cloned, immutable metadata. An empty set means no supported liquid
/// evidence, not proof that the original zone contains no liquid.
#[derive(Debug, Clone, Default)]
pub struct LiquidRegions {
    volumes: Arc<Volumes>,
}

impl LiquidRegions {
    /// Loads metadata without texture decoding or building collision meshes.
    /// Like zone rendering, EQG takes precedence when both zone formats exist.
    pub fn load(base: &Path, zone: &str) -> Result<Self> {
        let zone = zone.to_ascii_lowercase();
        let path = zone_archive(base, &zone)?;
        if path
            .extension()
            .is_some_and(|ext| ext.eq_ignore_ascii_case("eqg"))
        {
            let archive = Archive::open(path)?;
            let declaration = read_eqg_declaration(base, &zone, &archive)?;
            if !declaration.trim_ascii_start().starts_with(b"EQTZP") {
                return match Self::from_eqgz(&declaration) {
                    Ok(regions) => Ok(regions),
                    Err(error) => {
                        tracing::warn!(zone, %error, "binary liquid regions unsupported; no volumes enabled");
                        Ok(Self::default())
                    }
                };
            }
            let map = read_heightmap(&archive, &declaration)?;
            return match Self::from_heightmap_with_groups(&map, |name| {
                read_terrain_group(base, &archive, name)
            }) {
                Ok(regions) => Ok(regions),
                Err(error) => {
                    tracing::warn!(zone, %error, "heightmap liquid regions unsupported; no volumes enabled");
                    Ok(Self::default())
                }
            };
        }
        let archive = Archive::open(path)?;
        Self::from_wld(&archive.read(&format!("{zone}.wld"))?)
    }

    pub fn from_wld(data: &[u8]) -> Result<Self> {
        let metadata = BspRegions::parse(data)?;
        let mut labels = BTreeMap::new();
        for declaration in &metadata.declarations {
            let Some(kind) = wld_kind(&declaration.declaration) else {
                continue;
            };
            for &index in &declaration.indices {
                let index = index
                    .checked_add(1)
                    .ok_or_else(|| invalid("region index overflow"))?;
                if index > metadata.region_count {
                    return Err(invalid("liquid references a missing BSP region"));
                }
                if labels
                    .insert(index, kind)
                    .is_some_and(|previous| previous != kind)
                {
                    return Err(invalid("conflicting liquid types in one BSP region"));
                }
            }
        }
        if labels.is_empty() {
            return Ok(Self::default());
        }
        let nodes = metadata
            .nodes
            .ok_or_else(|| invalid("liquid regions have no BSP tree"))?;
        Self::from_bsp(nodes, metadata.region_count, labels)
    }

    /// Native binary ZON regions retain source order and signed half-extents.
    /// Raw rotations use quantized 512-unit angles, not radians or degrees.
    /// Unsupported records reject the complete set, including dry overrides.
    pub fn from_eqgz(data: &[u8]) -> Result<Self> {
        let regions = BinaryRegions::parse(data)?;
        Ok(Self::from_binary_regions(regions))
    }

    pub(crate) fn from_binary_regions(regions: BinaryRegions) -> Self {
        let has_liquid = regions
            .boxes
            .iter()
            .any(|volume| terrain_kind(&volume.name).is_some());
        Self {
            volumes: Arc::new(if has_liquid {
                Volumes::NativeBinary(regions)
            } else {
                Volumes::Empty
            }),
        }
    }

    /// Builds the verified top-level DAT subset, preserving native registration
    /// order, startup height anchors, quantized yaw and dry/unknown winners.
    /// Unsupported records or unresolved object groups reject the whole set;
    /// use `from_heightmap_with_groups` to check referenced group definitions.
    /// Only explicit AWT/ALV/AVW names establish a known liquid; numeric unnamed
    /// types and classic-name semantics are not inferred. An unsupported winner
    /// suppresses liquid evidence without claiming a native dry classification.
    pub fn from_heightmap(map: &Heightmap) -> Result<Self> {
        let regions = NativeTopLevelRegions::from_heightmap(map)?;
        Ok(Self::from_native_terrain(regions))
    }

    /// Also supports object-bearing terrain after proving that all referenced
    /// group files are complete and have no embedded region declarations.
    pub fn from_heightmap_with_groups(
        map: &Heightmap,
        resolve: impl FnMut(&str) -> Result<Vec<u8>>,
    ) -> Result<Self> {
        let regions = NativeTopLevelRegions::from_heightmap_with_groups(map, resolve)?;
        Ok(Self::from_native_terrain(regions))
    }

    fn from_native_terrain(regions: NativeTopLevelRegions) -> Self {
        let has_liquid = regions
            .boxes
            .iter()
            .any(|volume| terrain_kind(&volume.name).is_some());
        Self {
            volumes: Arc::new(if has_liquid {
                Volumes::NativeTerrain(regions)
            } else {
                Volumes::Empty
            }),
        }
    }

    fn from_bsp(
        nodes: Vec<Node>,
        region_count: u32,
        labels: BTreeMap<u32, LiquidKind>,
    ) -> Result<Self> {
        if nodes.is_empty() {
            return Err(invalid("empty liquid BSP"));
        }
        let mut visited = vec![false; nodes.len()];
        let mut order = Vec::new();
        let mut pending = vec![0];
        visited[0] = true;
        while let Some(index) = pending.pop() {
            order.push(index);
            let node = &nodes[index];
            if !node.plane.iter().all(|v| v.is_finite())
                || (node.children != [0, 0] && node.plane[..3] == [0.; 3])
            {
                return Err(invalid("invalid BSP split plane"));
            }
            if node.region > region_count {
                return Err(invalid("BSP leaf references missing region"));
            }
            for child in node.children {
                if child == 0 {
                    continue;
                }
                let child = child as usize - 1;
                if child >= nodes.len() || visited[child] {
                    return Err(invalid("invalid, cyclic, or shared BSP child"));
                }
                visited[child] = true;
                pending.push(child);
            }
        }
        let mut kinds = vec![None; region_count as usize + 1];
        for (index, kind) in labels {
            kinds[index as usize] = Some(kind);
        }
        let mut wet = vec![false; nodes.len()];
        for &index in order.iter().rev() {
            let node = &nodes[index];
            wet[index] = if node.children == [0, 0] {
                kinds[node.region as usize].is_some()
            } else {
                node.children
                    .into_iter()
                    .any(|child| child != 0 && wet[child as usize - 1])
            };
        }
        if !wet[0] {
            return Ok(Self::default());
        }
        Ok(Self {
            volumes: Arc::new(Volumes::Bsp { nodes, kinds, wet }),
        })
    }

    /// Builds real bounded volumes, also useful for tools and procedural maps.
    /// Extents must be positive; non-finite or degenerate transforms fail.
    /// Overlapping boxes use first-source precedence consistently in both queries.
    pub fn from_boxes(boxes: impl IntoIterator<Item = LiquidBox>) -> Result<Self> {
        let mut volumes = Vec::new();
        for value in boxes {
            if !value
                .center
                .iter()
                .chain(&value.half_extents)
                .chain(&value.rotation)
                .all(|v| v.is_finite())
                || value.half_extents.iter().any(|v| *v <= 0.)
            {
                return Err(invalid("invalid liquid box dimensions"));
            }
            let rotation = DQuat::from_array(value.rotation.map(f64::from));
            if (rotation.length_squared() - 1.).abs() > 0.001 {
                return Err(invalid("liquid box rotation is not normalized"));
            }
            volumes.push(OrientedBox {
                kind: value.kind,
                center: DVec3::from_array(value.center.map(f64::from)),
                half_extents: DVec3::from_array(value.half_extents.map(f64::from)),
                inverse_rotation: rotation.normalize().conjugate(),
            });
        }
        Ok(Self {
            volumes: Arc::new(if volumes.is_empty() {
                Volumes::Empty
            } else {
                Volumes::Boxes(volumes)
            }),
        })
    }

    pub fn is_empty(&self) -> bool {
        matches!(&*self.volumes, Volumes::Empty)
    }

    /// Queries one finite position; it does not extend water below a surface.
    /// WLD split-plane boundaries are dry, matching EQEmu's WTR v1 query.
    pub fn at(&self, point: [f32; 3]) -> Option<LiquidKind> {
        if !point.iter().all(|v| v.is_finite()) {
            return None;
        }
        let point = DVec3::from_array(point.map(f64::from));
        match &*self.volumes {
            Volumes::Empty => None,
            Volumes::NativeTerrain(regions) => regions
                .at(point.to_array(), None)
                .and_then(|volume| terrain_kind(&volume.name)),
            Volumes::NativeBinary(regions) => regions
                .at(point.to_array(), None)
                .and_then(|volume| terrain_kind(&volume.name)),
            Volumes::Boxes(boxes) => boxes
                .iter()
                .find(|volume| {
                    let local = volume.inverse_rotation * (point - volume.center);
                    local.abs().cmple(volume.half_extents).all()
                })
                .map(|volume| volume.kind),
            Volumes::Bsp { nodes, kinds, wet } => {
                let mut index = 0;
                loop {
                    if !wet[index] {
                        return None;
                    }
                    let node = &nodes[index];
                    if node.children == [0, 0] {
                        return kinds[node.region as usize];
                    }
                    let distance = distance(node.plane, point);
                    if distance == 0. {
                        return None;
                    }
                    let child = node.children[usize::from(distance < 0.)];
                    if child == 0 {
                        return None;
                    }
                    index = child as usize - 1;
                }
            }
        }
    }

    /// Finds every liquid interval, including thin regions traversed with both
    /// endpoints dry. Dry gaps and different liquid types remain separate.
    /// Non-finite/zero-length segments and point-only tangencies return no spans.
    pub fn segment(&self, from: [f32; 3], to: [f32; 3]) -> Vec<LiquidSpan> {
        if from == to || !from.iter().chain(&to).all(|v| v.is_finite()) {
            return Vec::new();
        }
        let from = DVec3::from_array(from.map(f64::from));
        let to = DVec3::from_array(to.map(f64::from));
        let mut spans = Vec::new();
        match &*self.volumes {
            Volumes::Empty => {}
            Volumes::Bsp { nodes, kinds, wet } => {
                let mut pending = vec![(0, 0., 1.)];
                while let Some((index, enter, exit)) = pending.pop() {
                    if !wet[index] || enter >= exit {
                        continue;
                    }
                    let node = &nodes[index];
                    if node.children == [0, 0] {
                        if let Some(kind) = kinds[node.region as usize] {
                            spans.push((enter, exit, kind));
                        }
                        continue;
                    }
                    let start = distance(node.plane, from.lerp(to, enter));
                    let end = distance(node.plane, from.lerp(to, exit));
                    if start == 0. && end == 0. {
                        continue;
                    }
                    for (side, child) in node.children.into_iter().enumerate() {
                        if child == 0 {
                            continue;
                        }
                        let positive = side == 0;
                        let inside_start = if positive { start > 0. } else { start < 0. };
                        let inside_end = if positive { end > 0. } else { end < 0. };
                        if inside_start && inside_end {
                            pending.push((child as usize - 1, enter, exit));
                        } else if inside_start != inside_end {
                            let split = enter + (exit - enter) * start / (start - end);
                            let range = if inside_start {
                                (enter, split)
                            } else {
                                (split, exit)
                            };
                            pending.push((child as usize - 1, range.0, range.1));
                        }
                    }
                }
            }
            Volumes::Boxes(boxes) => {
                let mut candidates = Vec::new();
                for volume in boxes {
                    let a = volume.inverse_rotation * (from - volume.center);
                    let b = volume.inverse_rotation * (to - volume.center);
                    if let Some((enter, exit)) = clip_box(a, b, volume.half_extents) {
                        candidates.push((enter, exit, Some(volume.kind)));
                    }
                }
                spans.extend(ordered_spans(&candidates));
            }
            Volumes::NativeTerrain(regions) => {
                let candidates: Vec<_> = regions
                    .boxes
                    .iter()
                    .filter(|volume| !volume.name.starts_with("APV"))
                    .filter_map(|volume| {
                        let [enter, exit] = volume.segment(from.to_array(), to.to_array())?;
                        Some((enter, exit, terrain_kind(&volume.name)))
                    })
                    .collect();
                spans.extend(ordered_spans(&candidates));
            }
            Volumes::NativeBinary(regions) => {
                let candidates: Vec<_> = regions
                    .boxes
                    .iter()
                    .filter(|volume| !volume.name.starts_with("APV"))
                    .filter_map(|volume| {
                        let [enter, exit] = volume.segment(from.to_array(), to.to_array())?;
                        Some((enter, exit, terrain_kind(&volume.name)))
                    })
                    .collect();
                spans.extend(ordered_spans(&candidates));
            }
        }
        spans.sort_by(|a, b| a.0.total_cmp(&b.0));
        let mut merged: Vec<(f64, f64, LiquidKind)> = Vec::new();
        for (enter, exit, kind) in spans {
            if let Some(previous) = merged.last_mut()
                && previous.2 == kind
                && enter <= previous.1
            {
                previous.1 = previous.1.max(exit);
            } else {
                merged.push((enter, exit, kind));
            }
        }
        merged
            .into_iter()
            .map(|(enter, exit, kind)| LiquidSpan {
                kind,
                enter: enter as f32,
                exit: exit as f32,
            })
            .collect()
    }
}

/// Candidate order is region precedence, independent of intersection order.
/// Selecting an unknown/dry candidate ends the search and leaves a dry gap.
fn ordered_spans(candidates: &[(f64, f64, Option<LiquidKind>)]) -> Vec<(f64, f64, LiquidKind)> {
    let mut boundaries: Vec<_> = candidates
        .iter()
        .flat_map(|&(enter, exit, _)| [enter, exit])
        .collect();
    boundaries.sort_by(f64::total_cmp);
    boundaries.dedup();
    boundaries
        .windows(2)
        .filter_map(|pair| {
            let middle = (pair[0] + pair[1]) * 0.5;
            let (_, _, kind) = candidates
                .iter()
                .find(|(enter, exit, _)| *enter < middle && middle < *exit)?;
            kind.map(|kind| (pair[0], pair[1], kind))
        })
        .collect()
}

fn terrain_kind(name: &str) -> Option<LiquidKind> {
    if name.len() < 4 {
        return None;
    }
    match name.as_bytes().get(..3)? {
        b"AWT" => Some(LiquidKind::Water),
        b"ALV" => Some(LiquidKind::Lava),
        b"AVW" => Some(LiquidKind::FreezingWater),
        _ => None,
    }
}

fn clip_box(from: DVec3, to: DVec3, extents: DVec3) -> Option<(f64, f64)> {
    let (mut enter, mut exit) = (0.0_f64, 1.0_f64);
    for axis in 0..3 {
        let delta = to[axis] - from[axis];
        if delta == 0. {
            if from[axis].abs() > extents[axis] {
                return None;
            }
            continue;
        }
        let a = (-extents[axis] - from[axis]) / delta;
        let b = (extents[axis] - from[axis]) / delta;
        enter = enter.max(a.min(b));
        exit = exit.min(a.max(b));
        if enter >= exit {
            return None;
        }
    }
    Some((enter, exit))
}

fn distance(plane: [f64; 4], point: DVec3) -> f64 {
    DVec3::new(plane[0], plane[1], plane[2]).dot(point) + plane[3]
}

fn wld_kind(declaration: &[u8]) -> Option<LiquidKind> {
    let name = std::str::from_utf8(declaration.split(|b| *b == 0).next()?)
        .ok()?
        .to_ascii_uppercase();
    if name.starts_with("WT_") || name.starts_with("WTN_") || name.starts_with("WTNTP") {
        Some(LiquidKind::Water)
    } else if name.starts_with("LA_") || name.starts_with("LAN_") || name.starts_with("LANTP") {
        Some(LiquidKind::Lava)
    } else if name.starts_with("VWN_") {
        Some(LiquidKind::FreezingWater)
    } else if name.starts_with("SLN_") {
        Some(LiquidKind::OpaqueWater)
    } else {
        None
    }
}

fn invalid(detail: &str) -> Error {
    Error::Format(format!("liquid regions: {detail}"))
}

#[cfg(test)]
mod tests;
