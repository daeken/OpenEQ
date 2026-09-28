//! Loads a complete zone into engine-agnostic geometry.
//!
//! EverQuest zones come in two flavours and both are handled here:
//!
//! * **Classic zones** ship as `{name}.s3d` plus `{name}_obj.s3d`/`{name}_2_obj.s3d`
//!   archives of `WLD` fragments. Terrain lives in `{name}.wld`; placeable
//!   objects live in the `_obj` archives as `*_DMSPRITEDEF` meshes placed by
//!   `0x15` actor instances.
//! * **Newer zones** ship as a single `{name}.eqg` archive containing a `.zon`
//!   description plus `.ter`/`.mod` meshes.
//!
//! The result is a [`Scene`]: flat triangle geometry grouped by material, a set
//! of placeable objects, their instances, and static lights. Textures are
//! decoded lazily through [`Scene::texture`].

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::mesh::{self, Geometry, Material};
use crate::pfs::Archive;
use crate::texture::Texture;
use crate::wld::{self, Fragment, Wld};
use crate::zone::ZoneFile;
use crate::{Error, Result};

/// A placed copy of a named object.
#[derive(Debug, Clone)]
pub struct Instance {
    pub object: String,
    pub position: [f32; 3],
    pub scale: [f32; 3],
    /// Rotation as a quaternion `(x, y, z, w)`.
    pub rotation: [f32; 4],
}

/// A static light.
#[derive(Debug, Clone)]
pub struct Light {
    pub position: [f32; 3],
    pub color: [f32; 3],
    pub radius: f32,
    pub attenuation: f32,
}

/// A named group of meshes that instances can refer to.
#[derive(Debug, Clone)]
pub struct SceneObject {
    pub name: String,
    /// Indices into [`Scene::meshes`].
    pub meshes: Vec<usize>,
}

/// Everything needed to render a zone.
pub struct Scene {
    pub name: String,
    pub materials: Vec<Material>,
    pub meshes: Vec<Geometry>,
    pub objects: Vec<SceneObject>,
    pub instances: Vec<Instance>,
    pub lights: Vec<Light>,
    archives: Vec<Archive>,
    textures: HashMap<String, (usize, String)>,
}

impl Scene {
    /// Decodes a texture by name, if the zone references one.
    pub fn texture(&self, name: &str) -> Option<Texture> {
        let (archive, entry) = self.textures.get(&name.to_ascii_lowercase())?;
        let data = self.archives.get(*archive)?.read(entry).ok()?;
        Some(Texture::decode_or_placeholder(name, &data))
    }

    /// All texture names referenced by the scene's materials.
    pub fn texture_names(&self) -> Vec<String> {
        let mut names: Vec<String> = self.textures.keys().cloned().collect();
        names.sort();
        names
    }

    /// Total triangle count across the scene.
    pub fn triangle_count(&self) -> usize {
        self.meshes.iter().map(|mesh| mesh.indices.len() / 3).sum()
    }
}

/// Loads a zone by name from a client data directory.
pub fn load_zone(base: impl AsRef<Path>, name: &str) -> Result<Scene> {
    let base = base.as_ref();
    let eqg = base.join(format!("{name}.eqg"));
    if eqg.is_file() {
        return load_eqg(base, name, &eqg);
    }
    load_wld(base, name)
}

fn load_wld(base: &Path, name: &str) -> Result<Scene> {
    let primary = base.join(format!("{name}.s3d"));
    let object_archive = base.join(format!("{name}_obj.s3d"));
    if !object_archive.is_file() {
        return Err(Error::Format(format!(
            "{name} has no .eqg and no {name}_obj.s3d to load as a classic zone"
        )));
    }

    let mut paths = vec![primary];
    let prefix = format!("{name}_");
    for entry in std::fs::read_dir(base).map_err(|source| Error::Io {
        path: base.to_path_buf(),
        source,
    })? {
        let entry = entry?;
        let file_name = entry.file_name().to_string_lossy().to_ascii_lowercase();
        if file_name.starts_with(&prefix)
            && file_name.ends_with(".s3d")
            && !file_name.contains("_chr")
        {
            paths.push(entry.path());
        }
    }
    paths.sort();
    paths.dedup();

    let mut scene = Scene {
        name: name.to_owned(),
        materials: Vec::new(),
        meshes: Vec::new(),
        objects: Vec::new(),
        instances: Vec::new(),
        lights: Vec::new(),
        archives: Vec::new(),
        textures: HashMap::new(),
    };

    // Keep the archive index for every WLD so texture lookups can prefer the
    // archive a reference came from.
    let mut wlds: Vec<(usize, Wld)> = Vec::new();
    for path in &paths {
        if !path.is_file() {
            continue;
        }
        let archive = Archive::open(path)?;
        let archive_index = scene.archives.len();
        let wld_names: Vec<String> = archive
            .names()
            .iter()
            .filter(|n| n.to_ascii_lowercase().ends_with(".wld"))
            .cloned()
            .collect();
        scene.archives.push(archive);
        for wld_name in wld_names {
            let wld = Wld::open(&scene.archives[archive_index], &wld_name)?;
            wlds.push((archive_index, wld));
        }
    }

    register_wld_textures(&mut scene, &wlds);

    // Terrain: every mesh in the zone's own WLD, baked together.
    let main_name = format!("{name}.wld");
    if let Some((archive_index, wld)) = wlds
        .iter()
        .find(|(_, wld)| wld.filename.eq_ignore_ascii_case(&main_name))
    {
        let meshes: Vec<&wld::Mesh> = wld.iter::<wld::Mesh>().map(|(_, mesh)| mesh).collect();
        let (materials, geometries) = mesh::bake_wld_meshes(wld, meshes);
        append_baked(&mut scene, *archive_index, materials, geometries);
    }

    // Objects: one object per mesh fragment in the non-terrain WLDs.
    for (archive_index, wld) in &wlds {
        if wld.filename.eq_ignore_ascii_case(&main_name) {
            continue;
        }
        for (chunk, object_mesh) in wld.iter::<wld::Mesh>() {
            let object_name = chunk
                .name
                .to_ascii_lowercase()
                .trim_end_matches("_dmspritedef")
                .to_string();
            if object_name.is_empty() {
                continue;
            }
            let (materials, geometries) = mesh::bake_wld_meshes(wld, std::iter::once(object_mesh));
            let start = scene.meshes.len();
            append_baked(&mut scene, *archive_index, materials, geometries);
            let end = scene.meshes.len();
            scene.objects.push(SceneObject {
                name: object_name,
                meshes: (start..end).collect(),
            });
        }
    }

    // Instances: actor placements in the object WLDs.
    for (_, wld) in &wlds {
        if wld.filename.eq_ignore_ascii_case(&main_name) {
            continue;
        }
        for (_, instance) in wld.iter::<wld::ActorInstance>() {
            let Some(target) = wld.resolve_str(instance.actor) else {
                continue;
            };
            if target.is_empty() {
                continue;
            }
            let object = strip_suffix(target, "_ACTORDEF").to_ascii_lowercase();
            let quat = rotation_from_euler([
                instance.rotation[0],
                instance.rotation[1],
                instance.rotation[2],
            ]);
            scene.instances.push(Instance {
                object,
                position: instance.position,
                scale: instance.scale,
                rotation: quat,
            });
        }
    }

    // Lights: fragment 0x28 in lights.wld, coloured by its 0x1B source.
    if let Some((_, lights_wld)) = wlds
        .iter()
        .find(|(_, wld)| wld.filename.eq_ignore_ascii_case("lights.wld"))
    {
        for (_, light) in lights_wld.iter::<wld::Light>() {
            let (color, attenuation) =
                light_source(lights_wld, light.source).unwrap_or(([1.0, 1.0, 1.0], 200.0));
            scene.lights.push(Light {
                position: light.position,
                color,
                radius: light.radius,
                attenuation,
            });
        }
    }

    Ok(scene)
}

fn load_eqg(base: &Path, name: &str, path: &Path) -> Result<Scene> {
    let archive = Archive::open(path)?;
    let zon_name = format!("{name}.zon");
    let zon_data = if archive.contains(&zon_name) {
        archive.read(&zon_name)?
    } else {
        let fallback = base.join(&zon_name);
        std::fs::read(&fallback).map_err(|source| Error::Io {
            path: fallback,
            source,
        })?
    };

    let zone = ZoneFile::parse(&zon_data, |file_name| {
        if let Ok(data) = archive.read(file_name) {
            return Ok(data);
        }
        let fallback = base.join(file_name);
        std::fs::read(&fallback).map_err(|source| Error::Io {
            path: fallback,
            source,
        })
    })?;

    let mut scene = Scene {
        name: name.to_owned(),
        materials: Vec::new(),
        meshes: Vec::new(),
        objects: Vec::new(),
        instances: Vec::new(),
        lights: Vec::new(),
        archives: vec![archive],
        textures: HashMap::new(),
    };
    let archive_index = 0;

    // The terrain object contributes its geometry to the zone itself; the
    // remaining objects become reusable, placeable definitions.
    for (id, object) in zone.objects.iter().enumerate() {
        let groups = object.mesh_groups();
        let mut keys: Vec<&u32> = groups.keys().collect();
        keys.sort();

        let mut object_meshes = Vec::new();
        for material_id in keys {
            let Some(material) = object.materials.get(material_id) else {
                continue;
            };
            let diffuse = material
                .properties
                .get("e_TextureDiffuse0")
                .and_then(|value| value.as_text())
                .unwrap_or("missing.dds")
                .to_owned();
            let normal_map = material
                .properties
                .get("e_TextureNormal0")
                .and_then(|value| value.as_text())
                .filter(|value| !value.eq_ignore_ascii_case("none"))
                .map(|value| value.to_owned());

            let indices = &groups[material_id];
            let (vertices, indices) = mesh::pack(
                &object.positions,
                &object.normals,
                &object.tex_coords,
                indices,
            );

            let id = scene.materials.len();
            scene.materials.push(Material {
                textures: vec![diffuse.clone()],
                normal_map: normal_map.clone(),
                flags: 0,
                anim_speed: 0,
                alpha_mask: false,
                transparent: false,
                emissive: false,
            });
            scene.meshes.push(Geometry {
                vertices,
                indices,
                material: id,
                collidable: true,
            });
            register_texture(&mut scene, archive_index, &diffuse);
            if let Some(normal_map) = &normal_map {
                register_texture(&mut scene, archive_index, normal_map);
            }
            object_meshes.push(scene.meshes.len() - 1);
        }

        if !object.is_terrain {
            scene.objects.push(SceneObject {
                name: format!("object_{id}"),
                meshes: object_meshes,
            });
        }
    }

    for placeable in &zone.placeables {
        if placeable.object_id < 0 {
            continue;
        }
        scene.instances.push(Instance {
            object: format!("object_{}", placeable.object_id),
            position: placeable.position,
            scale: [placeable.scale; 3],
            rotation: rotation_from_euler(placeable.rotation),
        });
    }

    for light in &zone.lights {
        scene.lights.push(Light {
            position: light.position,
            color: light.color,
            radius: light.radius,
            attenuation: 200.0,
        });
    }

    Ok(scene)
}

fn append_baked(
    scene: &mut Scene,
    archive_index: usize,
    materials: Vec<Material>,
    geometries: Vec<Geometry>,
) {
    let base = scene.materials.len();
    for material in materials {
        for texture in &material.textures {
            register_texture(scene, archive_index, texture);
        }
        if let Some(normal_map) = &material.normal_map {
            register_texture(scene, archive_index, normal_map);
        }
        scene.materials.push(material);
    }
    for mut geometry in geometries {
        geometry.material += base;
        scene.meshes.push(geometry);
    }
}

fn register_wld_textures(scene: &mut Scene, wlds: &[(usize, Wld)]) {
    for (archive_index, wld) in wlds {
        for (_, list) in wld.iter::<wld::TextureList>() {
            for filename in &list.filenames {
                register_texture(scene, *archive_index, filename);
            }
        }
    }
}

fn register_texture(scene: &mut Scene, archive_index: usize, name: &str) {
    let key = name.to_ascii_lowercase();
    if scene.textures.contains_key(&key) || name.is_empty() {
        return;
    }
    // Prefer the archive the reference came from, then fall back to any other.
    let mut candidates = vec![archive_index];
    candidates.extend((0..scene.archives.len()).filter(|index| *index != archive_index));
    for index in candidates {
        let archive = &scene.archives[index];
        if let Some(entry) = archive.names().iter().find(|entry| {
            entry.eq_ignore_ascii_case(name)
                || strip_extension(entry).eq_ignore_ascii_case(strip_extension(name))
        }) {
            scene.textures.insert(key, (index, entry.clone()));
            return;
        }
    }
}

fn light_source(wld: &Wld, reference: wld::Ref) -> Option<([f32; 3], f32)> {
    let Some(Fragment::LightSourceRef(source_ref)) =
        wld.resolve(reference).map(|chunk| &chunk.fragment)
    else {
        return None;
    };
    let Some(Fragment::LightSource(source)) =
        wld.resolve(source_ref.source).map(|chunk| &chunk.fragment)
    else {
        return None;
    };
    Some((source.color, source.attenuation.unwrap_or(200) as f32))
}

fn strip_suffix<'a>(value: &'a str, suffix: &str) -> &'a str {
    if value.len() >= suffix.len()
        && value[value.len() - suffix.len()..].eq_ignore_ascii_case(suffix)
    {
        &value[..value.len() - suffix.len()]
    } else {
        value
    }
}

fn strip_extension(value: &str) -> &str {
    match value.rfind('.') {
        Some(index) => &value[..index],
        None => value,
    }
}

/// Builds a placement quaternion from per-axis angles in `(X, Y, Z)` order,
/// composed as `Rz * Ry * Rx` like the original client.
///
/// The parameter order is part of the contract, not a detail: placement angles
/// are stored in the files as `(Z, Y, X)`, and applying them to the wrong axes
/// turns a tree's yaw into a lean. Callers normalise into `(X, Y, Z)` first.
fn rotation_from_euler([around_x, around_y, around_z]: [f32; 3]) -> [f32; 4] {
    let x_axis = axis_angle([1.0, 0.0, 0.0], around_x);
    let y_axis = axis_angle([0.0, 1.0, 0.0], around_y);
    let z_axis = axis_angle([0.0, 0.0, 1.0], around_z);
    quat_mul(quat_mul(z_axis, y_axis), x_axis)
}

fn axis_angle(axis: [f32; 3], angle: f32) -> [f32; 4] {
    let half = angle * 0.5;
    let sin = half.sin();
    [axis[0] * sin, axis[1] * sin, axis[2] * sin, half.cos()]
}

fn quat_mul(a: [f32; 4], b: [f32; 4]) -> [f32; 4] {
    [
        a[3] * b[0] + a[0] * b[3] + a[1] * b[2] - a[2] * b[1],
        a[3] * b[1] - a[0] * b[2] + a[1] * b[3] + a[2] * b[0],
        a[3] * b[2] + a[0] * b[1] - a[1] * b[0] + a[2] * b[3],
        a[3] * b[3] - a[0] * b[0] - a[1] * b[1] - a[2] * b[2],
    ]
}

/// Convenience: the default client data directory on this machine.
pub fn default_client_dir() -> Option<PathBuf> {
    let candidates = [
        std::env::var_os("OPENEQ_CLIENT_DIR").map(PathBuf::from),
        Some(PathBuf::from("/Users/daeken/EverQuest")),
    ];
    candidates.into_iter().flatten().find(|path| path.is_dir())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f32::consts::FRAC_PI_2;

    /// A yaw must come out as a rotation about Z, not as a lean. Getting this
    /// backwards turns every placed object in a zone into a tilted one.
    #[test]
    fn x_angle_leaves_z_alone() {
        let quarter_turn_about_x = rotation_from_euler([FRAC_PI_2, 0.0, 0.0]);
        // Rotation about X by 90 degrees maps up onto -Y.
        let up = rotate([0.0, 0.0, 1.0], quarter_turn_about_x);
        assert!(
            up[1].abs() > 0.99,
            "expected up to tip toward Y, got {up:?}"
        );
        assert!(up[2].abs() < 0.01);
    }

    #[test]
    fn z_angle_spins_in_plane() {
        let quarter_turn_about_z = rotation_from_euler([0.0, 0.0, FRAC_PI_2]);
        let up = rotate([0.0, 0.0, 1.0], quarter_turn_about_z);
        assert!(up[2] > 0.999, "a yaw must not tilt, got {up:?}");
        let east = rotate([1.0, 0.0, 0.0], quarter_turn_about_z);
        assert!(east[1] > 0.99, "a yaw should map +X to +Y, got {east:?}");
    }

    /// Rotates a vector by a quaternion `(x, y, z, w)`.
    fn rotate(v: [f32; 3], q: [f32; 4]) -> [f32; 3] {
        let [x, y, z, w] = q;
        let cross = |a: [f32; 3], b: [f32; 3]| {
            [
                a[1] * b[2] - a[2] * b[1],
                a[2] * b[0] - a[0] * b[2],
                a[0] * b[1] - a[1] * b[0],
            ]
        };
        let axis = [x, y, z];
        let first = cross(axis, v);
        let second = cross(axis, first);
        [
            v[0] + 2.0 * (w * first[0] + second[0]),
            v[1] + 2.0 * (w * first[1] + second[1]),
            v[2] + 2.0 * (w * first[2] + second[2]),
        ]
    }
}
