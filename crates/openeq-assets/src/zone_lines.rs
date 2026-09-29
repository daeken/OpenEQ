//! Authored classic zone-line volumes, in asset/scene coordinates (Z up).
//!
//! WLD fragment 0x21 partitions space with a BSP. Fragment 0x29 associates
//! zero-based region indices with declarations such as `DRNTP00255000004`:
//! `00255` means a server-provided destination, and `000004` is the exact
//! `OP_SendZonepoints` number, not a destination zone ID. The declaration can
//! be the fragment name or its XOR-encoded payload. The BSP's leaf regions
//! and child pointers are one-based. These layouts agree with EQEmu's WLD
//! water-map reader and LanternExtractor's BspRegionType decoder.
//!
//! Only reference destinations are supported. Absolute WLD destinations and
//! EQG region transforms are deliberately not inferred from nearby server
//! points: those packets contain destination positions, not source volumes.
use std::{collections::BTreeMap, path::Path};

use crate::{Error, Result, pfs::Archive, read::Reader, wld::WLD_MAGIC};

const STRING_KEY: [u8; 8] = [0x95, 0x3a, 0xc5, 0x2a, 0x95, 0x7a, 0x95, 0x6a];
// Limits the expanded representation even for a malicious, extremely deep BSP.
const MAX_CELL_PLANES: usize = 1_000_000;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ZoneLine {
    /// Exact server zone-point number. Resolve only against this zone's list.
    pub number: u32,
}

#[derive(Clone, Debug)]
struct Node {
    plane: [f64; 4],
    region: u32,
    children: [u32; 2],
}

#[derive(Clone, Debug)]
struct Cell {
    line: ZoneLine,
    // Inward-facing planes: dot(normal, position) + distance >= 0.
    planes: Vec<[f64; 4]>,
}

/// Exact convex BSP cells containing supported authored zone-line triggers.
#[derive(Clone, Debug, Default)]
pub struct ZoneLines {
    cells: Vec<Cell>,
}

impl ZoneLines {
    /// Loads only trigger metadata; does not decode textures or build meshes.
    /// EQG zones currently return an empty set, including when an older S3D
    /// also exists, matching the renderer's preference for the EQG geometry.
    pub fn load(base: &Path, zone: &str) -> Result<Self> {
        if base.join(format!("{zone}.eqg")).is_file() {
            return Ok(Self::default());
        }
        let archive = Archive::open(base.join(format!("{zone}.s3d")))?;
        Self::from_wld(&archive.read(&format!("{zone}.wld"))?)
    }

    /// Reads bounded fragment payloads independently of visual mesh decoding.
    pub fn from_wld(data: &[u8]) -> Result<Self> {
        let mut reader = Reader::new(data);
        let magic = reader.u32()?;
        if magic != WLD_MAGIC {
            return Err(Error::BadMagic {
                found: magic,
                expected: WLD_MAGIC,
            });
        }
        reader.u32()?; // Version does not change these two fragment layouts.
        let fragment_count = reader.bounded_count()?;
        reader.skip(8)?;
        let string_size = reader.bounded_count()?;
        reader.skip(4)?;
        let strings = decode(reader.take(string_size)?);
        reader.align4()?;
        if fragment_count > reader.remaining() / 12 {
            return Err(invalid("fragment count exceeds WLD data"));
        }
        let mut nodes = None;
        let mut regions = BTreeMap::new();
        let mut region_count = 0;
        for _ in 0..fragment_count {
            let size = reader.u32()? as usize;
            let kind = reader.u32()?;
            let mut fragment = Reader::new(reader.take(size)?);
            let name_ref = fragment.i32()?;
            match kind {
                0x21 => {
                    if nodes.is_some() {
                        return Err(invalid("multiple BSP trees in zone"));
                    }
                    let count = fragment.bounded_count()?;
                    if count > fragment.remaining() / 28 {
                        return Err(invalid("BSP node count exceeds fragment"));
                    }
                    let mut tree = Vec::with_capacity(count);
                    for _ in 0..count {
                        let normal = fragment.vec3()?;
                        let distance = fragment.f32()?;
                        let plane = [normal[0], normal[1], normal[2], distance].map(f64::from);
                        if !plane.iter().all(|v| v.is_finite()) {
                            return Err(invalid("non-finite BSP plane"));
                        }
                        tree.push(Node {
                            plane,
                            region: fragment.u32()?,
                            children: [fragment.u32()?, fragment.u32()?],
                        });
                    }
                    nodes = Some(tree);
                }
                0x22 => region_count += 1,
                0x29 => {
                    fragment.u32()?; // Flags.
                    let count = fragment.bounded_count()?;
                    if count > fragment.remaining().saturating_sub(4) / 4 {
                        return Err(invalid("region count exceeds fragment"));
                    }
                    let mut indices = Vec::with_capacity(count);
                    for _ in 0..count {
                        indices.push(fragment.u32()?);
                    }
                    let length = fragment.bounded_count()?;
                    let declaration = if length == 0 {
                        if name_ref > 0 {
                            return Err(invalid("region name is not a string reference"));
                        }
                        let start = name_ref.unsigned_abs() as usize;
                        strings
                            .get(start..)
                            .ok_or_else(|| invalid("region name outside string table"))?
                            .to_vec()
                    } else {
                        decode(fragment.take(length)?)
                    };
                    if let Some(line) = reference_line(&declaration) {
                        for region in indices {
                            let region = region
                                .checked_add(1)
                                .ok_or_else(|| invalid("region index overflow"))?;
                            if let Some(previous) = regions.insert(region, line)
                                && previous != line
                            {
                                return Err(invalid("conflicting zone lines in one BSP region"));
                            }
                        }
                    }
                }
                _ => {}
            }
        }
        if regions.is_empty() {
            return Ok(Self::default());
        }
        if regions.keys().any(|index| *index > region_count) {
            return Err(invalid("zone line references a missing BSP region"));
        }
        Self::compile(
            &nodes.ok_or_else(|| invalid("zone-line regions have no BSP tree"))?,
            &regions,
        )
    }

    fn compile(nodes: &[Node], regions: &BTreeMap<u32, ZoneLine>) -> Result<Self> {
        if nodes.is_empty() {
            return Err(invalid("zone-line BSP is empty"));
        }
        let mut parents = vec![None; nodes.len()];
        let mut visited = vec![false; nodes.len()];
        visited[0] = true;
        let mut pending = vec![0];
        // Validate without recursive calls. One node cannot be its own ancestor
        // or have two parents, so hostile asset data cannot loop during queries.
        while let Some(index) = pending.pop() {
            let node = &nodes[index];
            if node.children != [0, 0] && node.plane[..3] == [0.; 3] {
                return Err(invalid("BSP split has a zero normal"));
            }
            for (side, child) in node.children.iter().copied().enumerate() {
                if child == 0 {
                    continue;
                }
                let child = child as usize - 1;
                if child >= nodes.len() || visited[child] {
                    return Err(invalid("invalid, cyclic, or shared BSP child"));
                }
                visited[child] = true;
                parents[child] = Some((index, side));
                pending.push(child);
            }
        }
        let mut cells = Vec::new();
        let mut total_planes = 0;
        for (index, node) in nodes.iter().enumerate() {
            if !visited[index] || node.children != [0, 0] {
                continue;
            }
            let Some(line) = regions.get(&node.region) else {
                continue;
            };
            let mut planes = Vec::new();
            let mut child = index;
            while let Some((parent, side)) = parents[child] {
                total_planes += 1;
                if total_planes > MAX_CELL_PLANES {
                    return Err(invalid("zone-line BSP exceeds plane budget"));
                }
                let sign = if side == 0 { 1. } else { -1. };
                planes.push(nodes[parent].plane.map(|v| v * sign));
                child = parent;
            }
            if planes.is_empty() {
                return Err(invalid("zone line has no boundary planes"));
            }
            cells.push(Cell {
                line: *line,
                planes,
            });
        }
        Ok(Self { cells })
    }

    pub fn is_empty(&self) -> bool {
        self.cells.is_empty()
    }

    /// Returns a trigger containing a finite scene position. This also allows
    /// callers to suppress spawning/teleporting inside a trigger until leaving.
    pub fn region_at(&self, point: [f32; 3]) -> Option<ZoneLine> {
        if !point.iter().all(|v| v.is_finite()) {
            return None;
        }
        let point = point.map(f64::from);
        self.cells
            .iter()
            .find(|cell| {
                cell.planes
                    .iter()
                    .all(|plane| distance(*plane, point) >= 0.)
            })
            .map(|cell| cell.line)
    }

    /// Returns the first trigger intersected by a movement segment, including
    /// segments that pass completely through a thin volume in one frame.
    /// Zero-length movement, non-finite positions, and point-only tangencies
    /// do not cross. A nonzero segment starting inside returns that trigger;
    /// callers are responsible for spawn suppression and teleport resets.
    pub fn crossed(&self, from: [f32; 3], to: [f32; 3]) -> Option<ZoneLine> {
        if from == to || !from.iter().chain(&to).all(|v| v.is_finite()) {
            return None;
        }
        let from = from.map(f64::from);
        let to = to.map(f64::from);
        self.cells
            .iter()
            .filter_map(|cell| {
                let (mut enter, mut leave) = (0.0_f64, 1.0_f64);
                for &plane in &cell.planes {
                    let start = distance(plane, from);
                    let end = distance(plane, to);
                    if start < 0. && end < 0. {
                        return None;
                    }
                    if start < 0. {
                        enter = enter.max(start / (start - end));
                    } else if end < 0. {
                        leave = leave.min(start / (start - end));
                    }
                    if enter >= leave {
                        return None;
                    }
                }
                Some((enter, cell.line))
            })
            .min_by(|a, b| a.0.total_cmp(&b.0))
            .map(|(_, line)| line)
    }
}

fn decode(bytes: &[u8]) -> Vec<u8> {
    bytes
        .iter()
        .enumerate()
        .map(|(i, byte)| byte ^ STRING_KEY[i % STRING_KEY.len()])
        .collect()
}

fn reference_line(declaration: &[u8]) -> Option<ZoneLine> {
    let text = declaration.split(|byte| *byte == 0).next()?;
    let prefix = text.get(..5)?;
    if ![b"DRNTP", b"WTNTP", b"LANTP"]
        .iter()
        .any(|known| prefix.eq_ignore_ascii_case(*known))
        || text.get(5..10)? != b"00255"
    {
        return None;
    }
    let digits = text.get(10..16)?;
    if !digits.iter().all(u8::is_ascii_digit) {
        return None;
    }
    let number = digits
        .iter()
        .fold(0, |number, digit| number * 10 + u32::from(digit - b'0'));
    Some(ZoneLine { number })
}

fn distance([x, y, z, d]: [f64; 4], point: [f64; 3]) -> f64 {
    x * point[0] + y * point[1] + z * point[2] + d
}

fn invalid(detail: &str) -> Error {
    Error::Format(format!("zone lines: {detail}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(payload_name: bool) -> Vec<u8> {
        let declaration = b"DRNTP00255000004_ZONE\0";
        let strings = [b"\0".as_slice(), declaration].concat();
        let mut out = Vec::new();
        for word in [WLD_MAGIC, 0x15500, 3, 0, 0, strings.len() as u32, 0] {
            out.extend(word.to_le_bytes());
        }
        out.extend(decode(&strings));
        while out.len() % 4 != 0 {
            out.push(0);
        }
        let mut tree = Vec::new();
        tree.extend(0_i32.to_le_bytes());
        tree.extend(7_u32.to_le_bytes());
        // Authored unit box [0, 1]^3, with empty outside branches.
        for (i, plane) in [
            [1., 0., 0., 0.],
            [-1., 0., 0., 1.],
            [0., 1., 0., 0.],
            [0., -1., 0., 1.],
            [0., 0., 1., 0.],
            [0., 0., -1., 1.],
            [0., 0., 0., 0.],
        ]
        .into_iter()
        .enumerate()
        {
            for value in plane {
                tree.extend(f32::to_le_bytes(value));
            }
            for value in if i == 6 {
                [1, 0, 0]
            } else {
                [0, i as u32 + 2, 0]
            } {
                tree.extend(u32::to_le_bytes(value));
            }
        }
        let mut region = Vec::new();
        for value in [
            -1_i32,
            0,
            1,
            0,
            if payload_name {
                declaration.len() as i32
            } else {
                0
            },
        ] {
            region.extend(value.to_le_bytes());
        }
        if payload_name {
            region.extend(decode(declaration));
        }
        for (kind, bytes) in [(0x21_u32, tree), (0x22, vec![0; 4]), (0x29, region)] {
            out.extend((bytes.len() as u32).to_le_bytes());
            out.extend(kind.to_le_bytes());
            out.extend(bytes);
        }
        out
    }

    #[test]
    fn segment_crosses_thin_volume_with_both_endpoints_outside() {
        for payload_name in [false, true] {
            let lines = ZoneLines::from_wld(&fixture(payload_name)).unwrap();
            let from = [-10., 0.5, 0.5];
            let to = [10., 0.5, 0.5];
            assert_eq!(lines.region_at(from), None);
            assert_eq!(lines.region_at(to), None);
            assert_eq!(lines.region_at([0.5; 3]), Some(ZoneLine { number: 4 }));
            assert_eq!(lines.crossed(from, to), Some(ZoneLine { number: 4 }));
            assert_eq!(lines.crossed(to, from), Some(ZoneLine { number: 4 }));
            assert_eq!(lines.crossed([-1., 2., 0.5], [2., 2., 0.5]), None);
            assert_eq!(lines.crossed([-1., 0., 0.5], [0., 1., 0.5]), None);
            assert_eq!(lines.crossed([0.5; 3], [0.5; 3]), None);
            assert_eq!(lines.region_at([f32::NAN; 3]), None);
            assert_eq!(lines.crossed(from, [f32::INFINITY; 3]), None);
        }
    }

    #[test]
    fn strict_reference_names_do_not_turn_water_or_absolute_destinations_into_indices() {
        for prefix in ["DRNTP", "WTNTP", "LANTP"] {
            assert_eq!(
                reference_line(format!("{prefix}00255000177_ZONE").as_bytes()),
                Some(ZoneLine { number: 177 })
            );
        }
        for name in [
            "WT_ZONE",
            "DRNTP_ZONE",
            "DRNTP00058000004_ZONE",
            "DRNTP00255000X04",
            "DRNTP00255",
        ] {
            assert_eq!(reference_line(name.as_bytes()), None);
        }
    }

    #[test]
    fn malformed_or_truncated_assets_fail_without_unbounded_traversal() {
        let data = fixture(true);
        for end in 0..data.len() {
            assert!(
                ZoneLines::from_wld(&data[..end]).is_err(),
                "accepted truncation {end}"
            );
        }
        let regions = BTreeMap::from([(1, ZoneLine { number: 4 })]);
        let node = Node {
            plane: [1., 0., 0., 0.],
            region: 0,
            children: [1, 0],
        };
        assert!(ZoneLines::compile(std::slice::from_ref(&node), &regions).is_err());
        let mut invalid = node;
        invalid.children = [2, 0];
        assert!(ZoneLines::compile(&[invalid], &regions).is_err());
    }

    #[test]
    fn segment_returns_earliest_of_multiple_triggers() {
        let mut lines = ZoneLines::from_wld(&fixture(false)).unwrap();
        let mut later = lines.cells[0].clone();
        later.line.number = 9;
        // Translate the second authored box by +3 on X.
        for plane in &mut later.planes {
            plane[3] -= plane[0] * 3.;
        }
        lines.cells.insert(0, later);
        assert_eq!(
            lines.crossed([-1., 0.5, 0.5], [5., 0.5, 0.5]),
            Some(ZoneLine { number: 4 })
        );
        assert_eq!(
            lines.crossed([5., 0.5, 0.5], [-1., 0.5, 0.5]),
            Some(ZoneLine { number: 9 })
        );
    }
}
