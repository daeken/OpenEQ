//! Weighted EQG character meshes (EQGS/EQGM) and timed EQGA bone tracks.
//! Coordinates stay in the file's EQ space; skinning uses inverse bind matrices.
use super::*;
use crate::read::Reader;

#[path = "modern_drakkin.rs"]
mod drakkin;

#[derive(Debug, Clone, Copy)]
struct Transform {
    position: Vec3,
    rotation: Quat,
    scale: Vec3,
}
impl Transform {
    fn read(reader: &mut Reader<'_>) -> Result<Self> {
        let value = Self {
            position: Vec3::from_array(reader.vec3()?),
            // EQG stores the inverse quaternion convention to glam. Bind and
            // animation transforms both need conjugation before composition.
            rotation: rotation([reader.f32()?, reader.f32()?, reader.f32()?, reader.f32()?])
                .conjugate(),
            scale: Vec3::from_array(reader.vec3()?),
        };
        if !value.position.is_finite()
            || !value.scale.is_finite()
            || value.scale.abs().min_element() < 1e-8
        {
            return Err(Error::Format("invalid EQG bone transform".into()));
        }
        Ok(value)
    }
    fn matrix(self) -> Mat4 {
        Mat4::from_scale_rotation_translation(self.scale, self.rotation, self.position)
    }
    fn blend(self, b: Self, t: f32) -> Self {
        Self {
            position: self.position.lerp(b.position, t),
            rotation: self.rotation.slerp(b.rotation, t),
            scale: self.scale.lerp(b.scale, t),
        }
    }
}
#[derive(Debug, Clone)]
struct Bone {
    name: String,
    next: i32,
    children: usize,
    child: i32,
    bind: Transform,
}
#[derive(Debug, Clone, Copy)]
struct Weight {
    bone: usize,
    weight: f32,
}
#[derive(Debug, Clone)]
struct Vertex {
    position: Vec3,
    normal: Vec3,
    uv: [f32; 2],
    weights: Vec<Weight>,
}
#[derive(Debug, Clone)]
struct Piece {
    name: String,
    vertices: Vec<Vertex>,
    triangles: Vec<([u32; 3], i32)>,
}
#[derive(Debug, Clone)]
struct Key {
    time: f32,
    transform: Transform,
}
#[derive(Debug, Clone)]
struct Clip {
    tracks: Vec<Vec<Key>>,
    duration: f32,
}
#[derive(Debug, Clone)]
pub(super) struct ModernModel {
    bones: Vec<Bone>,
    parents: Vec<Option<usize>>,
    order: Vec<usize>,
    inverse_bind: Vec<Mat4>,
    pieces: Vec<Piece>,
    materials: Vec<Material>,
    material_names: Vec<String>,
    clips: BTreeMap<String, Clip>,
    /// Baked mesh -> original vertices, so material splitting preserves weights.
    bindings: Vec<Vec<Vertex>>,
    baked_materials: Vec<usize>,
    baked_slots: Vec<Option<usize>>,
    /// Keep source modules distinct even when their shader labels match.
    baked_pieces: Vec<usize>,
    center_z: f32,
}
fn name(strings: &[u8], offset: u32) -> Result<String> {
    let tail = strings
        .get(offset as usize..)
        .ok_or_else(|| Error::Format("EQG name offset out of range".into()))?;
    let end = tail
        .iter()
        .position(|b| *b == 0)
        .ok_or_else(|| Error::Format("unterminated EQG name".into()))?;
    Ok(String::from_utf8_lossy(&tail[..end]).into_owned())
}
fn materials(
    reader: &mut Reader<'_>,
    strings: &[u8],
    count: usize,
) -> Result<(Vec<Material>, Vec<String>)> {
    let mut result = Vec::new();
    let mut names = Vec::new();
    for _ in 0..count {
        let id = reader.bounded_count()?;
        names.push(name(strings, reader.u32()?)?);
        let shader = name(strings, reader.u32()?)?.to_ascii_lowercase();
        let properties = reader.bounded_count()?;
        let mut texture = None;
        let mut normal = None;
        for _ in 0..properties {
            let key = name(strings, reader.u32()?)?;
            let kind = reader.u32()?;
            let value = reader.u32()?;
            if kind == 2 {
                let value = name(strings, value)?;
                match key.as_str() {
                    "e_TextureDiffuse0" => texture = Some(value),
                    "e_TextureNormal0" => normal = Some(value),
                    _ => {}
                }
            }
        }
        if id != result.len() {
            return Err(Error::Format(
                "nonsequential EQG character material ids".into(),
            ));
        }
        result.push(Material {
            textures: texture.into_iter().collect(),
            normal_map: normal,
            water: None,
            flags: 0,
            anim_speed: 0,
            alpha_mask: shader.starts_with("chroma") || shader.starts_with("alpha"),
            transparent: shader.starts_with("alpha"),
            emissive: shader.contains("add"),
        });
    }
    Ok((result, names))
}
fn bones(reader: &mut Reader<'_>, strings: &[u8], count: usize) -> Result<Vec<Bone>> {
    let mut bones = Vec::with_capacity(count);
    for _ in 0..count {
        bones.push(Bone {
            name: name(strings, reader.u32()?)?,
            next: reader.i32()?,
            children: reader.bounded_count()?,
            child: reader.i32()?,
            bind: Transform::read(reader)?,
        });
    }
    Ok(bones)
}
fn vertices(reader: &mut Reader<'_>, count: usize, version: u32) -> Result<Vec<Vertex>> {
    let mut values = Vec::with_capacity(count);
    for _ in 0..count {
        let position = Vec3::from_array(reader.vec3()?);
        let normal = Vec3::from_array(reader.vec3()?);
        if version >= 3 {
            reader.u32()?;
        }
        let uv = reader.vec2()?;
        if version >= 3 {
            reader.skip(8)?;
        }
        if !position.is_finite() || !normal.is_finite() || !uv.iter().all(|v| v.is_finite()) {
            return Err(Error::Format("nonfinite EQG character vertex".into()));
        }
        values.push(Vertex {
            position,
            normal,
            uv,
            weights: Vec::new(),
        });
    }
    Ok(values)
}
fn triangles(
    reader: &mut Reader<'_>,
    count: usize,
    vertices: usize,
    materials: usize,
) -> Result<Vec<([u32; 3], i32)>> {
    let mut values = Vec::with_capacity(count);
    for _ in 0..count {
        let indices = [reader.u32()?, reader.u32()?, reader.u32()?];
        let material = reader.i32()?;
        reader.u32()?;
        if indices.iter().any(|i| *i as usize >= vertices)
            || material < -1
            || material >= materials as i32
        {
            return Err(Error::Format("EQG character triangle out of range".into()));
        }
        values.push((indices, material));
    }
    Ok(values)
}
fn weights(reader: &mut Reader<'_>, vertices: &mut [Vertex], bones: usize) -> Result<()> {
    for vertex in vertices {
        let count = reader.u32()?;
        if count > 4 {
            return Err(Error::Format("more than four EQG bone weights".into()));
        }
        for i in 0..4 {
            let bone = reader.i32()?;
            let weight = reader.f32()?;
            if i < count {
                if bone < 0 || bone as usize >= bones || !weight.is_finite() || weight < 0. {
                    return Err(Error::Format("invalid EQG bone weight".into()));
                }
                vertex.weights.push(Weight {
                    bone: bone as usize,
                    weight,
                });
            }
        }
        let sum: f32 = vertex.weights.iter().map(|w| w.weight).sum();
        if sum > 1e-8 {
            for weight in &mut vertex.weights {
                weight.weight /= sum;
            }
        } else {
            vertex.weights = vec![Weight {
                bone: 0,
                weight: 1.,
            }];
        }
    }
    Ok(())
}
impl ModernModel {
    fn parse(data: &[u8], code: &str) -> Result<Self> {
        let mut r = Reader::new(data);
        let magic = r.take(4)?;
        let mds = magic == b"EQGS";
        if !mds && magic != b"EQGM" {
            return Err(Error::Format("not an EQGS/EQGM character".into()));
        }
        let version = r.u32()?;
        if !(1..=3).contains(&version) {
            return Err(Error::Format(format!(
                "unsupported EQG character version {version}"
            )));
        }
        let ns = r.bounded_count()?;
        let nm = r.bounded_count()?;
        let (nb, np, nv, nt) = if mds {
            (r.bounded_count()?, r.bounded_count()?, 0, 0)
        } else {
            let nv = r.bounded_count()?;
            let nt = r.bounded_count()?;
            (r.bounded_count()?, 1, nv, nt)
        };
        if nb == 0 || nb > 4096 || np == 0 || np > 512 {
            return Err(Error::Format(
                "invalid EQG character bone/piece count".into(),
            ));
        }
        let strings = r.take(ns)?;
        let (materials, material_names) = materials(&mut r, strings, nm)?;
        let (bones, mut pieces) = if mds {
            let bones = bones(&mut r, strings, nb)?;
            let mut pieces = Vec::new();
            for _ in 0..np {
                let _main = r.u32()?;
                let name = name(strings, r.u32()?)?;
                let nv = r.bounded_count()?;
                let nt = r.bounded_count()?;
                let _assignments = r.bounded_count()?;
                let mut vertices = vertices(&mut r, nv, version)?;
                let triangles = triangles(&mut r, nt, nv, nm)?;
                weights(&mut r, &mut vertices, nb)?;
                pieces.push(Piece {
                    name,
                    vertices,
                    triangles,
                });
            }
            (bones, pieces)
        } else {
            let mut vertices = vertices(&mut r, nv, version)?;
            let triangles = triangles(&mut r, nt, nv, nm)?;
            let bones = bones(&mut r, strings, nb)?;
            weights(&mut r, &mut vertices, nb)?;
            (
                bones,
                vec![Piece {
                    name: code.to_owned(),
                    vertices,
                    triangles,
                }],
            )
        };
        if drakkin::is_drakkin(code.split('_').next().unwrap_or(code)) {
            // Drakkin MOD UVs use bottom-origin V, while decoded DDS rows use
            // top-origin V. This includes every modular garment and face part.
            // The authored nose is at V=.606, matching DDS row 1-.606, not .606.
            for vertex in pieces.iter_mut().flat_map(|piece| &mut piece.vertices) {
                vertex.uv[1] = 1. - vertex.uv[1];
            }
        }
        if r.remaining() > 4 {
            return Err(Error::Format(format!(
                "{} unparsed EQG character bytes",
                r.remaining()
            )));
        }
        let mut parents = vec![None; nb];
        for (parent, bone) in bones.iter().enumerate() {
            let mut child = bone.child;
            for _ in 0..bone.children {
                if child < 0
                    || child as usize >= nb
                    || child as usize == parent
                    || parents[child as usize].replace(parent).is_some()
                {
                    return Err(Error::Format("invalid EQG bone hierarchy".into()));
                }
                child = bones[child as usize].next;
            }
        }
        let mut order: Vec<_> = parents
            .iter()
            .enumerate()
            .filter_map(|(i, p)| p.is_none().then_some(i))
            .collect();
        let mut cursor = 0;
        while cursor < order.len() {
            let parent = order[cursor];
            order.extend(
                parents
                    .iter()
                    .enumerate()
                    .filter_map(|(i, p)| (*p == Some(parent)).then_some(i)),
            );
            cursor += 1;
        }
        if order.len() != nb {
            return Err(Error::Format("cycle in EQG bone hierarchy".into()));
        }
        let mut absolute = vec![Mat4::IDENTITY; nb];
        for &i in &order {
            let local = bones[i].bind.matrix();
            absolute[i] = parents[i].map_or(local, |p| absolute[p] * local);
        }
        let inverse_bind = absolute.iter().map(|m| m.inverse()).collect();
        Ok(Self {
            bones,
            parents,
            order,
            inverse_bind,
            pieces,
            materials,
            material_names,
            clips: BTreeMap::new(),
            bindings: Vec::new(),
            baked_materials: Vec::new(),
            baked_slots: Vec::new(),
            baked_pieces: Vec::new(),
            center_z: 0.,
        })
    }
    fn animation(&self, data: &[u8]) -> Result<Clip> {
        let mut r = Reader::new(data);
        if r.take(4)? != b"EQGA" {
            return Err(Error::Format("not EQGA animation".into()));
        }
        let version = r.u32()?;
        let ns = r.bounded_count()?;
        let count = r.bounded_count()?;
        if version > 1 {
            r.u32()?;
        }
        let strings = r.take(ns)?;
        let mut tracks = vec![Vec::new(); self.bones.len()];
        let mut duration = 0f32;
        for _ in 0..count {
            let frames = r.bounded_count()?;
            let name = name(strings, r.u32()?)?;
            let bone = self
                .bones
                .iter()
                .position(|bone| bone.name.eq_ignore_ascii_case(&name));
            let mut track = Vec::with_capacity(frames);
            let mut previous = 0.;
            for _ in 0..frames {
                let time = r.u32()? as f32 / 1000.;
                if time < previous {
                    return Err(Error::Format("unordered EQGA frame times".into()));
                }
                previous = time;
                track.push(Key {
                    time,
                    transform: Transform::read(&mut r)?,
                });
                duration = duration.max(time);
            }
            if let Some(bone) = bone {
                tracks[bone] = track;
            }
        }
        Ok(Clip { tracks, duration })
    }
    fn local_transforms(&self, animation: &str, time: f32, looping: bool) -> Vec<Transform> {
        let clip = self.clips.get(animation);
        let time = if time.is_finite() { time.max(0.) } else { 0. };
        let time = clip.map_or(0., |clip| {
            if looping && clip.duration > 0. {
                time % clip.duration
            } else {
                time.min(clip.duration)
            }
        });
        let mut result = Vec::with_capacity(self.bones.len());
        for index in 0..self.bones.len() {
            let mut transform = self.bones[index].bind;
            if let Some(track) = clip
                .map(|c| &c.tracks[index])
                .filter(|track| !track.is_empty())
            {
                let upper = track.partition_point(|key| key.time <= time);
                if upper == 0 {
                    transform = track[0].transform;
                } else if upper == track.len() {
                    transform = track[upper - 1].transform;
                } else {
                    let a = &track[upper - 1];
                    let b = &track[upper];
                    transform = a
                        .transform
                        .blend(b.transform, (time - a.time) / (b.time - a.time).max(1e-8));
                }
            }
            result.push(transform);
        }
        result
    }
    fn global_transforms(&self, transforms: &[Transform]) -> Vec<Mat4> {
        let mut result = vec![Mat4::IDENTITY; self.bones.len()];
        for &index in &self.order {
            let local = transforms[index].matrix();
            result[index] = self.parents[index].map_or(local, |parent| result[parent] * local);
        }
        result
    }
    pub(super) fn bone_transforms(&self, animation: &str, time: f32, looping: bool) -> Vec<Mat4> {
        self.global_transforms(&self.local_transforms(animation, time, looping))
    }
    pub(super) fn sample_blended_into(
        &self,
        from: CharacterPose<'_>,
        to: CharacterPose<'_>,
        blend: f32,
        meshes: &mut [Geometry],
    ) -> bool {
        self.skin_into(&self.blended_bone_transforms(from, to, blend), meshes)
    }
    pub(super) fn blended_bone_transforms(
        &self,
        from: CharacterPose<'_>,
        to: CharacterPose<'_>,
        blend: f32,
    ) -> Vec<Mat4> {
        if blend <= 0. {
            return self.bone_transforms(from.animation, from.time_seconds, from.looping);
        }
        if blend >= 1. {
            return self.bone_transforms(to.animation, to.time_seconds, to.looping);
        }
        let from = self.local_transforms(from.animation, from.time_seconds, from.looping);
        let to = self.local_transforms(to.animation, to.time_seconds, to.looping);
        let local: Vec<_> = from
            .into_iter()
            .zip(to)
            .map(|(a, b)| a.blend(b, blend))
            .collect();
        self.global_transforms(&local)
    }
    pub(super) fn center_offset(&self) -> Mat4 {
        Mat4::from_translation(Vec3::new(0., 0., -self.center_z))
    }
    pub(super) fn sample_into(
        &self,
        animation: &str,
        time: f32,
        looping: bool,
        meshes: &mut [Geometry],
    ) -> bool {
        self.skin_into(&self.bone_transforms(animation, time, looping), meshes)
    }
    fn skin_into(&self, transforms: &[Mat4], meshes: &mut [Geometry]) -> bool {
        if meshes.len() != self.bindings.len()
            || meshes
                .iter()
                .zip(&self.bindings)
                .any(|(m, b)| m.vertex_count() != b.len())
        {
            return false;
        }
        let skin: Vec<_> = transforms
            .iter()
            .zip(&self.inverse_bind)
            .map(|(a, b)| *a * *b)
            .collect();
        for (mesh, vertices) in meshes.iter_mut().zip(&self.bindings) {
            for (out, vertex) in mesh.vertices.chunks_exact_mut(8).zip(vertices) {
                let mut position = Vec3::ZERO;
                let mut normal = Vec3::ZERO;
                for weight in &vertex.weights {
                    position += skin[weight.bone].transform_point3(vertex.position) * weight.weight;
                    normal += skin[weight.bone].transform_vector3(vertex.normal) * weight.weight;
                }
                position.z -= self.center_z;
                out[..3].copy_from_slice(&position.to_array());
                out[3..6].copy_from_slice(&normal.normalize_or_zero().to_array());
            }
        }
        true
    }
}

impl ModernModel {
    fn bake(
        &mut self,
        code: &str,
        appearance: &CharacterAppearance,
    ) -> (Vec<Geometry>, Vec<Material>) {
        let variant = if appearance.helm_texture > 0 && appearance.helm_texture != 255 {
            appearance.helm_texture
        } else {
            appearance.texture
        };
        let wanted = format!("{code}{variant:02}");
        let base = self
            .pieces
            .iter()
            .position(|p| p.name.eq_ignore_ascii_case(&wanted))
            .or_else(|| {
                self.pieces
                    .iter()
                    .position(|p| p.name.eq_ignore_ascii_case(code))
            })
            .or_else(|| {
                self.pieces
                    .iter()
                    .position(|p| p.name.eq_ignore_ascii_case(&format!("{code}00")))
            })
            .unwrap_or(0);
        let mut selected = vec![base];
        if let Some(head) = self.pieces.iter().position(|p| {
            p.name
                .eq_ignore_ascii_case(&format!("{code}HE{variant:02}"))
        }) {
            selected.push(head);
        }
        let selected = if drakkin::is_drakkin(code) {
            drakkin::select(self, code, appearance)
        } else {
            selected.into_iter().map(|index| (index, None)).collect()
        };
        self.bindings.clear();
        self.baked_materials.clear();
        self.baked_slots.clear();
        self.baked_pieces.clear();
        let mut meshes = Vec::new();
        let mut mesh_slots = Vec::new();
        let mut mesh_pieces = Vec::new();
        for (index, slot) in selected {
            let piece = &self.pieces[index];
            let mut groups: BTreeMap<usize, Vec<u32>> = BTreeMap::new();
            for (indices, material) in &piece.triangles {
                if *material >= 0 {
                    groups
                        .entry(*material as usize)
                        .or_default()
                        .extend(indices);
                }
            }
            for (material, indices) in groups {
                let mut vertices = Vec::new();
                let mut bindings = Vec::new();
                let mut mapped = HashMap::new();
                let mut out = Vec::new();
                for index in indices {
                    let mapped = *mapped.entry(index).or_insert_with(|| {
                        let vertex = &piece.vertices[index as usize];
                        let index = bindings.len() as u32;
                        vertices.extend(vertex.position.to_array());
                        vertices.extend(vertex.normal.to_array());
                        vertices.extend(vertex.uv);
                        bindings.push(vertex.clone());
                        index
                    });
                    out.push(mapped);
                }
                meshes.push(Geometry {
                    vertices,
                    indices: out,
                    material,
                    collidable: false,
                });
                self.bindings.push(bindings);
                mesh_slots.push(slot);
                mesh_pieces.push(index);
            }
        }
        // MOD/MDS material tables can contain unused editor placeholders (for
        // example grid_standard.dds in Drakkin). Keep only rendered materials.
        let mut materials = Vec::new();
        let mut material_map = HashMap::new();
        for ((mesh, slot), piece) in meshes.iter_mut().zip(mesh_slots).zip(mesh_pieces) {
            mesh.material = *material_map
                .entry((mesh.material, slot, piece))
                .or_insert_with(|| {
                    let index = materials.len();
                    materials.push(self.materials[mesh.material].clone());
                    self.baked_materials.push(mesh.material);
                    self.baked_slots.push(slot);
                    self.baked_pieces.push(piece);
                    index
                });
        }
        (meshes, materials)
    }
}
fn clip_code(prefix: &str) -> Option<&'static str> {
    Some(match prefix {
        "stnd" => "P01",
        "idle" => "O01",
        "walk" | "wlk" => "L01",
        "nrun" | "mrun" => "L02",
        "crmp" | "dest" => "D05",
        "flch" => "D01",
        "crch" => "L08",
        "nsit" => "P07",
        "slpr" | "satk" => "C05",
        "bash" => "C07",
        "kick" => "C01",
        "gcst" => "T05",
        "swim" => "P06",
        "jmpu" => "L04",
        "jmpa" => "L03",
        _ => return None,
    })
}
impl CharacterLibrary {
    pub(super) fn normalize_modern_appearance(
        &self,
        code: &str,
        mut value: CharacterAppearance,
    ) -> CharacterAppearance {
        let drakkin = drakkin::is_drakkin(code);
        let valid_armor = |value: u32| value <= 4 || (10..=16).contains(&value);
        if value.texture > 99 || (drakkin && !valid_armor(u32::from(value.texture))) {
            value.texture = 0;
        }
        if value.helm_texture > if drakkin { 4 } else { 99 } {
            value.helm_texture = 0;
        }
        if drakkin {
            let male = code.eq_ignore_ascii_case("DKM");
            let gender = u8::from(!male);
            if self
                .customization
                .get(522, value.drakkin_heritage, gender)
                .is_none()
            {
                value.drakkin_heritage = 0;
            }
            let metadata = self.customization.get(522, value.drakkin_heritage, gender);
            let count = |select: fn(&customization::FeatureCounts) -> u32, fallback| {
                metadata.map_or(fallback, |entry| select(&entry.features).min(fallback))
            };
            let colors = metadata.map_or(0, |entry| entry.colors.len());
            if usize::from(value.hair_color) >= colors {
                value.hair_color = 0;
            }
            if usize::from(value.beard_color) >= colors {
                value.beard_color = 0;
            }
            if u32::from(value.face) >= count(|f| f.faces, 7) {
                value.face = 0;
            }
            if u32::from(value.eye_color_1) >= count(|f| f.eyes, 12) {
                value.eye_color_1 = 0;
            }
            if u32::from(value.eye_color_2) >= count(|f| f.eyes, 12) {
                value.eye_color_2 = 0;
            }
            if u32::from(value.hair_style) >= count(|f| f.hair_styles, if male { 9 } else { 8 }) {
                value.hair_style = 0;
            }
            if u32::from(value.beard) >= count(|f| f.beards, if male { 12 } else { 4 }) {
                value.beard = 0;
            }
            if value.drakkin_tattoo >= count(|f| f.tattoos, 8) {
                value.drakkin_tattoo = 0;
            }
            if value.drakkin_details >= count(|f| f.facial_attachments, 8) {
                value.drakkin_details = 0;
            }
        } else {
            value.hair_color = 0;
            value.beard_color = 0;
            value.drakkin_heritage = 0;
            value.face = 0;
            value.eye_color_1 = 0;
            value.eye_color_2 = 0;
            value.hair_style = 0;
            value.beard = 0;
            value.drakkin_tattoo = 0;
            value.drakkin_details = 0;
        }
        for (index, slot) in value.equipment.iter_mut().enumerate() {
            slot.elite_material = 0;
            slot.hero_forge_model = 0;
            if slot.color >> 24 == 0 || slot.color & 0x00ff_ffff == 0x00ff_ffff {
                slot.color = 0;
            }
            if index < 7 {
                if !drakkin || index == 3 {
                    *slot = EquipmentAppearance::default();
                } else if (index == 0 && slot.material > 4) || !valid_armor(slot.material) {
                    slot.material = 0;
                }
            } else if !self.has_equipment(slot.material) {
                *slot = EquipmentAppearance::default();
            }
        }
        value
    }

    fn attach_modern_equipment(
        &self,
        modern: &mut ModernModel,
        meshes: &mut Vec<Geometry>,
        materials: &mut Vec<Material>,
        equipment: EquipmentAppearance,
        secondary: bool,
    ) -> Result<()> {
        let (mut item_materials, mut item_meshes) = self.equipment_geometry(equipment.material)?;
        let shield = secondary
            && item_materials
                .iter()
                .flat_map(|m| &m.textures)
                .any(|name| name.to_ascii_lowercase().contains("shield"));
        let attachment = if shield {
            "ARML_SHLD"
        } else if secondary {
            "ARML_WEAP"
        } else {
            "ARMR_WEAP"
        };
        let bone = modern
            .bones
            .iter()
            .position(|b| b.name.eq_ignore_ascii_case(attachment))
            .ok_or_else(|| Error::NotFound(format!("EQG attachment {attachment}")))?;
        let bind = modern.inverse_bind[bone].inverse();
        for material in &mut item_materials {
            for name in &mut material.textures {
                if material.alpha_mask {
                    *name = format!("{name}#masked");
                }
                if equipment.color >> 24 != 0 {
                    *name = format!("{name}#tint={:08x}", equipment.color);
                }
            }
        }
        for mesh in &mut item_meshes {
            mesh.material += materials.len();
            mesh.collidable = false;
            modern.bindings.push(
                mesh.vertices
                    .chunks_exact(8)
                    .map(|v| Vertex {
                        // IT mesh origin is its grip point. Convert to the character
                        // bind space once; the shared skin pass handles animation and
                        // transitions exactly as it does for weighted body geometry.
                        position: bind.transform_point3(Vec3::new(v[0], v[1], v[2])),
                        normal: bind.transform_vector3(Vec3::new(v[3], v[4], v[5])),
                        uv: [v[6], v[7]],
                        weights: vec![Weight { bone, weight: 1. }],
                    })
                    .collect(),
            );
        }
        materials.extend(item_materials);
        meshes.extend(item_meshes);
        Ok(())
    }

    pub(super) fn load_modern_model(&self, code: &str) -> Result<CharacterModel> {
        let path = self
            .eqg_files
            .get(&code.to_ascii_lowercase())
            .ok_or_else(|| Error::NotFound(format!("EQG character {code}")))?;
        let archive = Arc::new(Archive::open(path)?);
        let filename = [format!("{code}.mds"), format!("{code}.mod")]
            .into_iter()
            .find(|name| archive.contains(name))
            .ok_or_else(|| Error::NotFound(format!("EQG character mesh {code}")))?;
        let mut modern = ModernModel::parse(&archive.read(&filename)?, code)?;
        let mut metadata = BTreeMap::from([(
            String::new(),
            CharacterAnimation {
                frame_time_ms: 100,
                frame_count: 1,
                tracks: Vec::new(),
            },
        )]);
        let mut names: Vec<_> = archive
            .names()
            .iter()
            .filter(|name| name.to_ascii_lowercase().ends_with(".ani"))
            .collect();
        names.sort();
        for name in names {
            let Some(prefix) = name.split('_').next() else {
                continue;
            };
            let Some(alias) = clip_code(prefix) else {
                continue;
            };
            if modern.clips.contains_key(alias) {
                continue;
            }
            match archive.read(name).and_then(|data| modern.animation(&data)) {
                Ok(clip) => {
                    metadata.insert(
                        alias.to_owned(),
                        CharacterAnimation {
                            frame_time_ms: 33,
                            frame_count: (clip.duration / 0.033).ceil().max(1.) as usize,
                            tracks: Vec::new(),
                        },
                    );
                    modern.clips.insert(alias.to_owned(), clip);
                }
                Err(error) => {
                    tracing::warn!(%code,animation=%name,%error,"EQG animation unavailable")
                }
            }
        }
        // Body bounds define actor scale and feet placement. Clothing, hair,
        // horns, and held equipment must never change that reference frame.
        let (meshes, _) = modern.bake(code, &CharacterAppearance::default());
        let mut min = Vec3::splat(f32::INFINITY);
        let mut max = Vec3::splat(f32::NEG_INFINITY);
        for v in meshes.iter().flat_map(|m| m.vertices.chunks_exact(8)) {
            let p = Vec3::new(v[0], v[1], v[2]);
            min = min.min(p);
            max = max.max(p);
        }
        modern.center_z = (min.z + max.z) * 0.5;
        min.z -= modern.center_z;
        max.z -= modern.center_z;
        if drakkin::is_drakkin(code) {
            drakkin::load_modules(&mut modern, &archive, code)?;
        }
        let (mut meshes, mut materials) = modern.bake(code, &CharacterAppearance::default());
        if drakkin::is_drakkin(code) {
            drakkin::apply_materials(
                &modern,
                &mut materials,
                &CharacterAppearance::default(),
                &archive,
                self.customization
                    .get(522, 0, u8::from(code.eq_ignore_ascii_case("DKF"))),
            );
        }
        modern.sample_into("", 0., false, &mut meshes);
        let model = CharacterModel {
            code: code.to_owned(),
            material_slots: vec![None; materials.len()],
            materials,
            meshes,
            animations: Arc::new(metadata),
            bounds_min: min.to_array(),
            bounds_max: max.to_array(),
            bindings: Vec::new(),
            parents: modern.parents.clone(),
            bone_order: modern.order.clone(),
            bone_names: modern.bones.iter().map(|b| b.name.clone()).collect(),
            modern: Some(Arc::new(modern)),
        };
        self.modern_archives
            .lock()
            .unwrap()
            .insert(code.to_owned(), archive);
        Ok(model)
    }
    pub(super) fn apply_modern_appearance(
        &self,
        model: &mut CharacterModel,
        appearance: &CharacterAppearance,
    ) {
        let normalized = self.normalize_modern_appearance(&model.code, *appearance);
        let appearance = &normalized;
        let mut modern = (**model.modern.as_ref().unwrap()).clone();
        let (mut meshes, mut materials) = modern.bake(&model.code, appearance);
        let archives = self.modern_archives.lock().unwrap();
        let archive = &archives[&model.code];
        if drakkin::is_drakkin(&model.code) {
            drakkin::apply_materials(
                &modern,
                &mut materials,
                appearance,
                archive,
                self.customization.get(
                    522,
                    appearance.drakkin_heritage,
                    u8::from(model.code.eq_ignore_ascii_case("DKF")),
                ),
            );
        } else if appearance.texture > 0 && appearance.texture < 100 {
            for material in &mut materials {
                for name in &mut material.textures {
                    let lower = name.to_ascii_lowercase();
                    if let Some(index) = lower.find("_s")
                        && lower.len() >= index + 5
                    {
                        let candidate = format!(
                            "{}{:02}{}",
                            &lower[..index + 2],
                            appearance.texture,
                            &lower[index + 4..]
                        );
                        if archive.contains(&candidate) {
                            *name = candidate;
                        }
                    }
                }
            }
        }
        drop(archives);
        for slot in [7, 8] {
            let equipment = appearance.equipment[slot];
            if equipment.material != 0
                && let Err(error) = self.attach_modern_equipment(
                    &mut modern,
                    &mut meshes,
                    &mut materials,
                    equipment,
                    slot == 8,
                )
            {
                tracing::debug!(model = %model.code, item = equipment.material, %error, "modern equipment unavailable");
            }
        }
        modern.sample_into("", 0., false, &mut meshes);
        model.meshes = meshes;
        model.materials = materials;
        model.material_slots = vec![None; model.materials.len()];
        model.modern = Some(Arc::new(modern));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pose_blending_rotates_local_bones_without_shrinking_the_chain() {
        let identity = Transform {
            position: Vec3::ZERO,
            rotation: Quat::IDENTITY,
            scale: Vec3::ONE,
        };
        let child = Transform {
            position: Vec3::X,
            ..identity
        };
        let model = ModernModel {
            bones: vec![
                Bone {
                    name: "root".into(),
                    next: -1,
                    children: 1,
                    child: 1,
                    bind: identity,
                },
                Bone {
                    name: "child".into(),
                    next: -1,
                    children: 0,
                    child: -1,
                    bind: child,
                },
            ],
            parents: vec![None, Some(0)],
            order: vec![0, 1],
            inverse_bind: vec![Mat4::IDENTITY, child.matrix().inverse()],
            pieces: Vec::new(),
            materials: Vec::new(),
            material_names: Vec::new(),
            clips: BTreeMap::from([(
                "turn".into(),
                Clip {
                    tracks: vec![
                        vec![Key {
                            time: 0.,
                            transform: Transform {
                                rotation: Quat::from_rotation_z(std::f32::consts::PI),
                                ..identity
                            },
                        }],
                        Vec::new(),
                    ],
                    duration: 0.,
                },
            )]),
            bindings: vec![vec![Vertex {
                position: Vec3::X,
                normal: Vec3::Z,
                uv: [0., 0.],
                weights: vec![Weight {
                    bone: 1,
                    weight: 1.,
                }],
            }]],
            baked_materials: Vec::new(),
            baked_slots: Vec::new(),
            baked_pieces: Vec::new(),
            center_z: 0.,
        };
        let mut meshes = vec![Geometry {
            vertices: vec![0.; 8],
            indices: Vec::new(),
            material: 0,
            collidable: false,
        }];
        let from = CharacterPose {
            animation: "",
            time_seconds: 0.,
            looping: false,
        };
        let to = CharacterPose {
            animation: "turn",
            ..from
        };
        assert!(model.sample_blended_into(from, to, 0.5, &mut meshes));
        let position = Vec3::from_slice(&meshes[0].vertices[..3]);
        assert!(
            (position.length() - 1.).abs() < 1e-6,
            "bone shortened: {position}"
        );
        assert!(position.x.abs() < 1e-6);
        assert!((position.y.abs() - 1.).abs() < 1e-6);
        let mut endpoint = meshes.clone();
        for (blend, pose) in [(0., from), (1., to)] {
            assert!(model.sample_blended_into(from, to, blend, &mut meshes));
            assert!(model.sample_into(
                pose.animation,
                pose.time_seconds,
                pose.looping,
                &mut endpoint
            ));
            assert_eq!(meshes[0].vertices, endpoint[0].vertices);
        }
    }

    #[test]
    fn eqg_rotation_uses_inverse_quaternion_convention() {
        let half_turn = std::f32::consts::FRAC_1_SQRT_2;
        let data: Vec<_> = [
            0f32, 0., 0., // translation
            0., 0., half_turn, half_turn, // stored 90 degree Z rotation
            1., 1., 1., // scale
        ]
        .into_iter()
        .flat_map(f32::to_le_bytes)
        .collect();
        let transform = Transform::read(&mut Reader::new(&data)).unwrap();
        let rotated = transform.matrix().transform_vector3(Vec3::X);
        assert!((rotated - Vec3::NEG_Y).length() < 1e-5);
    }
}
