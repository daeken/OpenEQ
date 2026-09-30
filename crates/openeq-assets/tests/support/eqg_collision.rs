//! Synthetic EQG archives shared by CPU and GPU loader regressions.
#![allow(dead_code)] // The two integration binaries use different fixture subsets.
use openeq_assets::{
    Scene,
    zone::{MOD_MAGIC, Placeable, Property, TER_MAGIC, TerMaterial, TerMod, ZON_MAGIC},
};
use std::{collections::HashMap, io::Write, path::PathBuf};

pub struct Fixture(pub PathBuf);
impl Fixture {
    pub fn new(objects: &[(&str, &TerMod)], placements: &[Placeable]) -> Self {
        static SERIAL: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "openeq-eqg-collision-{}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
            SERIAL.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        std::fs::create_dir(&path).unwrap();
        let mut files = vec![("fixture.zon".to_owned(), zon_bytes(objects, placements))];
        files.extend(
            objects
                .iter()
                .map(|(name, object)| (name.to_string(), model_bytes(object))),
        );
        std::fs::write(path.join("fixture.eqg"), archive_bytes(&files)).unwrap();
        Self(path)
    }
    pub fn scene(&self) -> Scene {
        openeq_assets::load_zone(&self.0, "fixture").unwrap()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn words(values: &[u32]) -> Vec<u8> {
    values.iter().flat_map(|v| v.to_le_bytes()).collect()
}
fn string(strings: &mut Vec<u8>, value: &str) -> u32 {
    let offset = strings.len() as u32;
    strings.extend(value.as_bytes());
    strings.push(0);
    offset
}
fn archive_bytes(files: &[(String, Vec<u8>)]) -> Vec<u8> {
    let mut bytes = words(&[0, openeq_assets::pfs::PFS_MAGIC, 0]);
    let mut entries = Vec::new();
    let mut names = words(&[files.len() as u32]);
    let mut block = |crc: u32, data: &[u8]| {
        let mut encoder = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::fast());
        encoder.write_all(data).unwrap();
        let compressed = encoder.finish().unwrap();
        let offset = bytes.len() as u32;
        bytes.extend(words(&[compressed.len() as u32, data.len() as u32]));
        bytes.extend(compressed);
        entries.push((crc, offset, data.len() as u32));
    };
    for (i, (name, data)) in files.iter().enumerate() {
        block(i as u32 + 1, data);
        names.extend(words(&[name.len() as u32 + 1]));
        names.extend(name.as_bytes());
        names.push(0);
    }
    block(openeq_assets::pfs::DIR_CRC, &names);
    let table = bytes.len() as u32;
    bytes[..4].copy_from_slice(&table.to_le_bytes());
    bytes.extend(words(&[entries.len() as u32]));
    for (crc, offset, size) in entries.into_iter().rev() {
        bytes.extend(words(&[crc, offset, size]));
    }
    bytes
}
fn model_bytes(model: &TerMod) -> Vec<u8> {
    let mut strings = vec![0];
    let mut materials = Vec::new();
    let mut ids: Vec<_> = model.materials.keys().copied().collect();
    ids.sort_unstable();
    for id in ids {
        let material = &model.materials[&id];
        materials.extend(words(&[
            id,
            string(&mut strings, &material.name),
            string(&mut strings, &material.shader),
            material.properties.len() as u32,
        ]));
        let mut keys: Vec<_> = material.properties.keys().collect();
        keys.sort();
        for key in keys {
            let offset = string(&mut strings, key);
            let (kind, value) = match &material.properties[key] {
                Property::Float(v) => (0, v.to_bits()),
                Property::IntegerBits(v) => (1, *v),
                Property::Text(v) => (2, string(&mut strings, v)),
                Property::Uint(v) => (3, *v),
            };
            materials.extend(words(&[offset, kind, value]));
        }
    }
    let mut bytes = words(&[
        if model.is_terrain {
            TER_MAGIC
        } else {
            MOD_MAGIC
        },
        2,
        strings.len() as u32,
        model.materials.len() as u32,
        model.positions.len() as u32,
        model.polygons.len() as u32,
    ]);
    if !model.is_terrain {
        bytes.extend(words(&[0]));
    }
    bytes.extend(strings);
    bytes.extend(materials);
    for i in 0..model.positions.len() {
        bytes.extend(
            model.positions[i]
                .into_iter()
                .chain(model.normals[i])
                .chain(model.tex_coords[i])
                .flat_map(f32::to_le_bytes),
        );
    }
    for &(a, b, c, material, flags) in &model.polygons {
        bytes.extend(words(&[a, b, c, material, flags]));
    }
    bytes
}
fn zon_bytes(objects: &[(&str, &TerMod)], placements: &[Placeable]) -> Vec<u8> {
    let mut strings = vec![0];
    let objects: Vec<_> = objects
        .iter()
        .map(|(name, _)| string(&mut strings, name))
        .collect();
    let names: Vec<_> = placements
        .iter()
        .map(|p| string(&mut strings, &p.name))
        .collect();
    let mut bytes = words(&[
        ZON_MAGIC,
        1,
        strings.len() as u32,
        objects.len() as u32,
        placements.len() as u32,
        0,
        0,
    ]);
    bytes.extend(strings);
    bytes.extend(words(&objects));
    for (p, name) in placements.iter().zip(names) {
        bytes.extend(words(&[p.object_id as u32, name]));
        bytes.extend(
            p.position
                .into_iter()
                .chain([p.rotation[2], p.rotation[1], p.rotation[0], p.scale])
                .flat_map(f32::to_le_bytes),
        );
    }
    bytes
}

pub fn model(is_terrain: bool) -> TerMod {
    TerMod {
        is_terrain,
        version: 2,
        materials: HashMap::new(),
        positions: vec![],
        normals: vec![],
        tex_coords: vec![],
        polygons: vec![],
    }
}
pub fn material(shader: &str, diffuse: Option<&str>) -> TerMaterial {
    TerMaterial {
        name: "fixture material".into(),
        shader: shader.into(),
        properties: diffuse
            .map(|d| ("e_TextureDiffuse0".into(), Property::Text(d.into())))
            .into_iter()
            .collect(),
    }
}
pub fn quad(model: &mut TerMod, points: [[f32; 3]; 4], material: u32, flags: u32) {
    let start = model.positions.len() as u32;
    let p = points.map(glam::Vec3::from);
    let normal = (p[1] - p[0])
        .cross(p[2] - p[0])
        .normalize_or_zero()
        .to_array();
    model.positions.extend(points);
    model.normals.extend([normal; 4]);
    model
        .tex_coords
        .extend([[0., 0.], [1., 0.], [1., 1.], [0., 1.]]);
    for [a, b, c] in [[0, 1, 2], [0, 2, 3]] {
        model
            .polygons
            .push((start + a, start + b, start + c, material, flags));
    }
}
pub fn wall(x: f32) -> [[f32; 3]; 4] {
    [[x, -10., 0.], [x, 10., 0.], [x, 10., 10.], [x, -10., 10.]]
}
pub fn floor(z: f32) -> [[f32; 3]; 4] {
    [
        [-20., -20., z],
        [20., -20., z],
        [20., 20., z],
        [-20., 20., z],
    ]
}
pub fn hidden_door() -> TerMod {
    let mut source = model(false);
    quad(
        &mut source,
        [
            [-10., -3., 0.],
            [10., -3., 0.],
            [10., 3., 0.],
            [-10., 3., 0.],
        ],
        u32::MAX,
        2,
    );
    quad(
        &mut source,
        [[0., -3., 0.], [0., 3., 0.], [0., 3., 10.], [0., -3., 10.]],
        u32::MAX,
        0x8000_0000,
    );
    source.positions.extend([[10000.; 3], [f32::NAN; 3]]);
    source.normals.extend([[0.; 3]; 2]);
    source.tex_coords.extend([[0.; 2]; 2]);
    source.polygons.extend([
        (8, 8, 8, u32::MAX, 2),
        (0, 9, 1, u32::MAX, 2),
        (0, 1, 999, u32::MAX, 2),
    ]);
    source
}

struct Fingerprint(u64);
impl Fingerprint {
    fn new() -> Self {
        Self(0xcbf29ce484222325)
    }
    fn bytes(&mut self, bytes: &[u8]) {
        for &b in bytes {
            self.0 ^= u64::from(b);
            self.0 = self.0.wrapping_mul(0x100000001b3);
        }
    }
    fn u32(&mut self, v: u32) {
        self.bytes(&v.to_le_bytes());
    }
    fn len(&mut self, v: usize) {
        self.bytes(&(v as u64).to_le_bytes());
    }
    fn text(&mut self, v: &str) {
        self.len(v.len());
        self.bytes(v.as_bytes());
    }
    fn optional_text(&mut self, v: Option<&str>) {
        self.bytes(&[u8::from(v.is_some())]);
        if let Some(v) = v {
            self.text(v);
        }
    }
    fn floats(&mut self, v: &[f32]) {
        self.len(v.len());
        for f in v {
            self.u32(f.to_bits());
        }
    }
}
/// Stable FNV1a over draw inputs, deliberately excluding physical flags/channel.
/// Baseline recorded against Bloodfields before installing EQG collision collection.
pub fn draw_fingerprints(scene: &Scene) -> [u64; 3] {
    let mut mesh = Fingerprint::new();
    mesh.len(scene.meshes.len());
    for v in &scene.meshes {
        mesh.floats(&v.vertices);
        mesh.len(v.indices.len());
        for &i in &v.indices {
            mesh.u32(i);
        }
        mesh.len(v.material);
    }
    let mut material = Fingerprint::new();
    material.len(scene.materials.len());
    for v in &scene.materials {
        material.len(v.textures.len());
        for t in &v.textures {
            material.text(t);
        }
        material.optional_text(v.normal_map.as_deref());
        material.u32(v.flags);
        material.u32(v.anim_speed);
        material.bytes(&[
            u8::from(v.alpha_mask),
            u8::from(v.transparent),
            u8::from(v.emissive),
            u8::from(v.water.is_some()),
        ]);
        if let Some(w) = &v.water {
            material.floats(&w.color1);
            material.floats(&w.color2);
            material.floats(&w.reflection_color);
            material.floats(&[w.fresnel_bias, w.fresnel_power, w.reflection_amount]);
            material.optional_text(w.environment_map.as_deref());
        }
    }
    let mut instances = Fingerprint::new();
    instances.len(scene.objects.len());
    for v in &scene.objects {
        instances.text(&v.name);
        instances.len(v.meshes.len());
        for &i in &v.meshes {
            instances.len(i);
        }
    }
    instances.len(scene.instances.len());
    for v in &scene.instances {
        instances.text(&v.object);
        instances.floats(&v.position);
        instances.floats(&v.scale);
        instances.floats(&v.rotation);
    }
    [mesh.0, material.0, instances.0]
}
pub const BLOODFIELDS_DRAW_FINGERPRINTS: [u64; 3] =
    [0x788b0e681cba32fa, 0x40ac2b6644e185fb, 0x8ac04774af24cb60];
/// Reconstruct the pre-fix physical channel without touching any draw inputs.
pub fn restore_legacy_collision(scene: &mut Scene) {
    scene.collision_meshes.clear();
    for object in &mut scene.objects {
        object.collision_meshes.clear();
    }
    for mesh in &mut scene.meshes {
        mesh.collidable = scene.materials[mesh.material].water.is_none();
    }
}
