//! Loads a complete zone into engine-agnostic geometry.
//!
//! EverQuest zones use three supported geometry layouts:
//!
//! * **Classic zones** ship as `{name}.s3d` plus `{name}_obj.s3d`/`{name}_2_obj.s3d`
//!   archives of `WLD` fragments. Terrain lives in `{name}.wld`; placeable
//!   objects live in the `_obj` archives as `*_DMSPRITEDEF` meshes placed by
//!   `0x15` actor instances.
//! * **Newer zones** ship as a single `{name}.eqg` archive containing a `.zon`
//!   description plus `.ter`/`.mod` meshes.
//! * **Heightmap zones** use `EQTZP` text descriptions, tiled `.dat` elevation
//!   grids, ecosystem `.eco` texture layers and `.tog` object groups.
//!
//! The result is a [`Scene`]: flat triangle geometry grouped by material, a set
//! of placeable objects, their instances, and static lights. Textures are
//! decoded lazily through [`Scene::texture`].

use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::mesh::{self, CollisionGeometry, Geometry, Material, WaterMaterial};
use crate::pfs::Archive;
use crate::terrain;
use crate::texture::Texture;
use crate::wld::{self, Fragment, Wld};
use crate::zone::{Property, TerMaterial, TerMod, ZoneFile};
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
    /// Indices into [`Scene::collision_meshes`], owned by the same instances.
    pub collision_meshes: Vec<usize>,
}

/// Everything needed to render a zone.
pub struct Scene {
    pub name: String,
    pub materials: Vec<Material>,
    /// Direct terrain recipes keyed by the current material index. Material
    /// replacement/remapping must clear or remap these keys too; ordinary
    /// object extraction starts with an empty map.
    pub terrain_materials: BTreeMap<usize, terrain::TerrainMaterial>,
    pub meshes: Vec<Geometry>,
    /// Physical geometry independent of drawable materials, never uploaded for
    /// drawing. Includes hidden WLD faces and EQG's separate physical bake.
    pub collision_meshes: Vec<CollisionGeometry>,
    pub objects: Vec<SceneObject>,
    pub instances: Vec<Instance>,
    pub lights: Vec<Light>,
    archives: Vec<Archive>,
    textures: HashMap<String, TextureSource>,
    loose_textures: HashMap<String, PathBuf>,
}

enum TextureSource {
    Archive(usize, String),
    File(PathBuf),
    Decoded(Arc<Texture>),
    DeferredTerrain(Box<terrain::DeferredTexture>),
}

impl Scene {
    /// Builds a scene from decoded geometry, e.g. animated character assets.
    pub fn from_geometry(
        name: String,
        materials: Vec<Material>,
        meshes: Vec<Geometry>,
        textures: Vec<Texture>,
    ) -> Self {
        Self {
            name,
            materials,
            terrain_materials: BTreeMap::new(),
            meshes,
            collision_meshes: Vec::new(),
            objects: Vec::new(),
            instances: Vec::new(),
            lights: Vec::new(),
            archives: Vec::new(),
            loose_textures: HashMap::new(),
            textures: textures
                .into_iter()
                .map(|t| {
                    (
                        t.name.to_ascii_lowercase(),
                        TextureSource::Decoded(Arc::new(t)),
                    )
                })
                .collect(),
        }
    }

    fn from_prepared_terrain(name: String, prepared: terrain::PreparedTerrain) -> Self {
        let mut scene = Self::from_geometry(name, prepared.materials, prepared.meshes, Vec::new());
        scene.terrain_materials = prepared.terrain_materials;
        // Preserve the eager API's registration order if an unusual native
        // source happens to share a generated fallback name.
        for texture in prepared.deferred_textures {
            scene.textures.insert(
                texture.name.clone(),
                TextureSource::DeferredTerrain(Box::new(texture)),
            );
        }
        for texture in prepared.textures {
            scene
                .textures
                .insert(texture.name.clone(), TextureSource::Decoded(texture));
        }
        scene
    }

    /// Decodes a texture by name, if the zone references one. Terrain fallback
    /// images are painted on first access and cached for subsequent requests.
    pub fn texture(&self, name: &str) -> Option<Texture> {
        match self.textures.get(&name.to_ascii_lowercase()) {
            Some(TextureSource::Decoded(texture)) => return Some((**texture).clone()),
            Some(TextureSource::DeferredTerrain(texture)) => {
                return Some(texture.texture().clone());
            }
            _ => {}
        }
        Some(Texture::decode_or_placeholder(
            name,
            &self.texture_bytes(name)?,
        ))
    }

    /// Decodes all six faces of a referenced DDS environment map.
    pub fn texture_cube(&self, name: &str) -> Option<Vec<Texture>> {
        Texture::decode_cube(name, &self.texture_bytes(name)?).ok()
    }

    fn texture_bytes(&self, name: &str) -> Option<Vec<u8>> {
        match self.textures.get(&name.to_ascii_lowercase())? {
            TextureSource::Archive(archive, entry) => self.archives.get(*archive)?.read(entry).ok(),
            TextureSource::File(path) => std::fs::read(path).ok(),
            TextureSource::Decoded(_) | TextureSource::DeferredTerrain(_) => None,
        }
    }

    /// Extracts one unplaced object definition for dynamic doors, lifts and
    /// server-spawned props. Only that object's used materials are decoded.
    pub fn object_model(&self, name: &str) -> Result<Scene> {
        let key = object_key(name);
        let object = self
            .objects
            .iter()
            .find(|object| object_key(&object.name) == key)
            .ok_or_else(|| Error::NotFound(format!("object {name} in {}", self.name)))?;
        let mut materials = Vec::new();
        let mut textures = HashMap::new();
        let mut remap = HashMap::new();
        let mut meshes = Vec::new();
        for index in &object.meshes {
            let mut mesh = self.meshes[*index].clone();
            let material = *remap.entry(mesh.material).or_insert_with(|| {
                let mut material = self.materials[mesh.material].clone();
                for name in &mut material.textures {
                    let source = name.clone();
                    if material.alpha_mask {
                        *name = format!("{source}#masked");
                    }
                    if let Some(mut texture) = self.texture(&source) {
                        if material.alpha_mask
                            && let Some(bytes) = self.texture_bytes(&source)
                        {
                            texture.mask_palette_index_zero(&bytes);
                        }
                        texture.name = name.clone();
                        textures.insert(name.clone(), texture);
                    }
                }
                let index = materials.len();
                materials.push(material);
                index
            });
            mesh.material = material;
            meshes.push(mesh);
        }
        let collision_meshes: Vec<_> = object
            .collision_meshes
            .iter()
            .map(|index| self.collision_meshes[*index].clone())
            .collect();
        if meshes.is_empty() && collision_meshes.is_empty() {
            return Err(Error::NotFound(format!("object geometry {name}")));
        }
        let mut model =
            Scene::from_geometry(key, materials, meshes, textures.into_values().collect());
        model.collision_meshes = collision_meshes;
        Ok(model)
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

fn object_key(name: &str) -> String {
    name.trim()
        .to_ascii_lowercase()
        .trim_end_matches("_actordef")
        .trim_end_matches("_dmspritedef")
        .trim_end_matches(".mod")
        .to_owned()
}

/// Loads reusable object definitions without loading zone terrain. Includes
/// modern supplemental archives named by the original zone's assets manifest.
/// Dynamic doors are generally absent from the zone's static placement list.
pub fn load_object_library(base: impl AsRef<Path>, zone: &str) -> Result<Scene> {
    let base = base.as_ref();
    let zone = zone.to_ascii_lowercase();
    let mut files = std::collections::BTreeMap::new();
    for entry in std::fs::read_dir(base)? {
        let entry = entry?;
        files.insert(
            entry.file_name().to_string_lossy().to_ascii_lowercase(),
            entry.path(),
        );
    }
    let mut wanted: Vec<String> = files
        .keys()
        .filter(|name| {
            ((name.starts_with(&format!("{zone}_obj"))
                || name.starts_with(&format!("{zone}_2_obj")))
                && (name.ends_with(".s3d") || name.ends_with(".eqg")))
                || *name == &format!("{zone}.eqg")
                || *name == "global_obj.s3d"
        })
        .cloned()
        .collect();
    if let Some(manifest) = files.get(&format!("{zone}_assets.txt")) {
        for line in std::fs::read_to_string(manifest)?.lines() {
            let line = line.trim().to_ascii_lowercase();
            if !line.contains(['/', '\\']) && (line.ends_with(".eqg") || line.ends_with(".s3d")) {
                wanted.push(line);
            }
        }
    }
    wanted.sort();
    wanted.dedup();
    let mut scene = Scene {
        name: format!("{zone} object library"),
        materials: Vec::new(),
        terrain_materials: BTreeMap::new(),
        meshes: Vec::new(),
        collision_meshes: Vec::new(),
        objects: Vec::new(),
        instances: Vec::new(),
        lights: Vec::new(),
        archives: Vec::new(),
        textures: HashMap::new(),
        loose_textures: loose_textures(base),
    };
    for name in wanted {
        let Some(path) = files.get(&name) else {
            continue;
        };
        let archive = Archive::open(path)?;
        let index = scene.archives.len();
        let filenames = archive.names().to_vec();
        scene.archives.push(archive);
        for filename in filenames {
            let lower = filename.to_ascii_lowercase();
            if lower.ends_with(".wld") {
                let wld = Wld::open(&scene.archives[index], &filename)?;
                for (chunk, mesh) in wld.iter::<wld::Mesh>() {
                    let (materials, meshes) = mesh::bake_wld_meshes(&wld, [mesh]);
                    let collision_start = scene.collision_meshes.len();
                    scene
                        .collision_meshes
                        .extend(mesh::bake_wld_collision_meshes(&wld, [mesh]));
                    let start = scene.meshes.len();
                    append_baked(&mut scene, index, materials, meshes);
                    let end = scene.meshes.len();
                    scene.objects.push(SceneObject {
                        name: object_key(&chunk.name),
                        meshes: (start..end).collect(),
                        collision_meshes: (collision_start..scene.collision_meshes.len()).collect(),
                    });
                }
            } else if lower.ends_with(".mod") {
                let object = TerMod::parse(&scene.archives[index].read(&filename)?, false)?;
                append_eqg_object(&mut scene, &object, &object_key(&filename), index);
            }
        }
    }
    // Supplemental archives can provide textures referenced by an earlier
    // object archive. Retry unresolved names once the full library is present.
    let names: Vec<_> = scene
        .materials
        .iter()
        .flat_map(|m| m.textures.iter().chain(m.normal_map.iter()))
        .cloned()
        .collect();
    for name in names {
        register_texture(&mut scene, 0, &name);
    }
    Ok(scene)
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
        terrain_materials: BTreeMap::new(),
        meshes: Vec::new(),
        collision_meshes: Vec::new(),
        objects: Vec::new(),
        instances: Vec::new(),
        lights: Vec::new(),
        archives: Vec::new(),
        textures: HashMap::new(),
        loose_textures: loose_textures(base),
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
        scene
            .collision_meshes
            .extend(mesh::bake_wld_collision_meshes(wld, meshes.iter().copied()));
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
            let collision_start = scene.collision_meshes.len();
            scene
                .collision_meshes
                .extend(mesh::bake_wld_collision_meshes(wld, [object_mesh]));
            let start = scene.meshes.len();
            append_baked(&mut scene, *archive_index, materials, geometries);
            let end = scene.meshes.len();
            scene.objects.push(SceneObject {
                name: object_name,
                meshes: (start..end).collect(),
                collision_meshes: (collision_start..scene.collision_meshes.len()).collect(),
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
    load_eqg_archive(base, name, Archive::open(path)?)
}

fn load_eqg_archive(base: &Path, name: &str, archive: Archive) -> Result<Scene> {
    let zon_data = read_eqg_declaration(base, name, &archive)?;

    if zon_data.trim_ascii_start().starts_with(b"EQTZP") {
        return load_heightmap(base, name, archive, &zon_data);
    }

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
        terrain_materials: BTreeMap::new(),
        meshes: Vec::new(),
        collision_meshes: Vec::new(),
        objects: Vec::new(),
        instances: Vec::new(),
        lights: Vec::new(),
        archives: vec![archive],
        textures: HashMap::new(),
        loose_textures: loose_textures(base),
    };
    // Terrain contributes directly; objects retain reusable definitions.
    for (id, object) in zone.objects.iter().enumerate() {
        append_eqg_object(&mut scene, object, &format!("object_{id}"), 0);
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

/// Exact archive and loose declarations keep their existing precedence. Some
/// archives use an older internal name (feerrott2 -> feerrott, chambersb ->
/// chambersa). Accept only an unambiguous declaration with archived dependencies.
pub(crate) fn read_eqg_declaration(base: &Path, name: &str, archive: &Archive) -> Result<Vec<u8>> {
    let zon_name = format!("{name}.zon");
    if archive.contains(&zon_name) {
        return archive.read(&zon_name);
    }
    let fallback = case_insensitive_file(base, &zon_name);
    match std::fs::read(&fallback) {
        Ok(data) => Ok(data),
        Err(source) => {
            if source.kind() == std::io::ErrorKind::NotFound
                && let Some(data) = unique_zone_declaration(archive)?
            {
                return Ok(data);
            }
            Err(Error::Io {
                path: fallback,
                source,
            })
        }
    }
}

fn unique_zone_declaration(archive: &Archive) -> Result<Option<Vec<u8>>> {
    let mut selected = unique_heightmap_declaration(archive)?.map(|(data, _)| data);
    let mut seen_binary = Vec::new();
    let mut unresolved = Vec::new();
    for filename in archive
        .names()
        .iter()
        .filter(|name| name.to_ascii_lowercase().ends_with(".zon"))
    {
        let data = archive.read(filename)?;
        if !data.starts_with(b"EQGZ") || seen_binary.contains(&data) {
            continue;
        }
        seen_binary.push(data.clone());
        // The original renamed dungeon archives contain one complete EQGZ
        // declaration. Resolve its own mesh table, never infer a terrain name
        // from the outer archive or select an arbitrary first ZON.
        let zone = match ZoneFile::parse(&data, |name| archive.read(name)) {
            Ok(zone) => zone,
            Err(error) => {
                unresolved.push((filename, error));
                continue;
            }
        };
        if !zone.objects.iter().any(|object| object.is_terrain) {
            unresolved.push((
                filename,
                Error::Format("declaration has no terrain mesh".into()),
            ));
            continue;
        }
        if let Some(previous) = &selected {
            if previous != &data {
                return Err(Error::Format(
                    "multiple distinct archived zone declarations name available geometry".into(),
                ));
            }
        } else {
            selected = Some(data);
        }
    }
    if selected.is_none() {
        // A sole broken internal declaration is useful evidence. Report its
        // missing/corrupt dependency rather than the unrelated outer ZON path.
        if unresolved.len() == 1 {
            let (filename, error) = unresolved.pop().unwrap();
            return Err(Error::Format(format!(
                "archived zone declaration {filename}: {error}"
            )));
        }
        if unresolved.len() > 1 {
            return Err(Error::Format(format!(
                "multiple distinct archived zone declarations could not resolve geometry: {}",
                unresolved
                    .iter()
                    .map(|(name, _)| name.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            )));
        }
    }
    Ok(selected)
}

fn unique_heightmap_declaration(
    archive: &Archive,
) -> Result<Option<(Vec<u8>, terrain::TerrainOptions)>> {
    let mut selected: Option<(Vec<u8>, terrain::TerrainOptions)> = None;
    for filename in archive
        .names()
        .iter()
        .filter(|name| name.to_ascii_lowercase().ends_with(".zon"))
    {
        // An unreadable candidate cannot be ruled out as a second valid
        // declaration. Fail instead of making the choice depend on corruption.
        let data = archive.read(filename)?;
        let Ok(options) = terrain::TerrainOptions::parse(&data) else {
            continue;
        };
        if !archive.contains(&format!("{}.dat", options.name)) {
            continue;
        }
        if let Some((previous, _)) = &selected {
            if previous != &data {
                return Err(Error::Format(
                    "multiple distinct archived heightmap declarations name available terrain data"
                        .into(),
                ));
            }
        } else {
            selected = Some((data, options));
        }
    }
    Ok(selected)
}

fn append_eqg_object(scene: &mut Scene, object: &TerMod, object_name: &str, archive_index: usize) {
    let collision_start = scene.collision_meshes.len();
    scene
        .collision_meshes
        .extend(eqg_collision::collect(object));
    let groups = object.mesh_groups();
    let mut keys: Vec<&u32> = groups.keys().collect();
    keys.sort();

    let mut object_meshes = Vec::new();
    for material_id in keys {
        let Some(material) = object.materials.get(material_id) else {
            continue;
        };
        let mut diffuse = material
            .properties
            .get("e_TextureDiffuse0")
            .and_then(|value| value.as_text())
            .unwrap_or("missing.dds")
            .to_owned();
        let water = water_material(material);
        register_texture(scene, archive_index, &diffuse);
        // MaxWater materials can name an obsolete editor test texture.
        // Use the client's shared water diffuse only for that shader and
        // only when the authored diffuse is missing. Other missing assets
        // must continue to show up as errors/placeholders.
        if water.is_some() && !scene.textures.contains_key(&diffuse.to_ascii_lowercase()) {
            register_texture(scene, archive_index, "water_c.bmp");
            if scene.textures.contains_key("water_c.bmp") {
                diffuse = "water_c.bmp".to_owned();
            }
        }
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
        if let Some(environment) = water
            .as_ref()
            .and_then(|water| water.environment_map.as_ref())
        {
            register_texture(scene, archive_index, environment);
        }
        scene.materials.push(Material {
            textures: vec![diffuse.clone()],
            normal_map: normal_map.clone(),
            water,
            flags: 0,
            anim_speed: 0,
            alpha_mask: {
                let shader = material.shader.to_ascii_lowercase();
                shader.starts_with("alpha") || shader.starts_with("chroma")
            },
            transparent: false,
            emissive: false,
            clamp_uv: false,
        });
        scene.meshes.push(Geometry {
            vertices,
            indices,
            material: id,
            // EQG polygon flags can differ within one drawable material batch.
            // Keep draw grouping intact and use the separate physical bake.
            collidable: false,
        });
        register_texture(scene, archive_index, &diffuse);
        if let Some(normal_map) = &normal_map {
            register_texture(scene, archive_index, normal_map);
        }
        object_meshes.push(scene.meshes.len() - 1);
    }

    if !object.is_terrain {
        scene.objects.push(SceneObject {
            name: object_name.to_owned(),
            meshes: object_meshes,
            collision_meshes: (collision_start..scene.collision_meshes.len()).collect(),
        });
    }
}

mod eqg_collision;
mod indexed_water;

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
            scene
                .textures
                .insert(key, TextureSource::Archive(index, entry.clone()));
            return;
        }
    }
    if let Some(path) = scene.loose_textures.get(&key) {
        scene
            .textures
            .insert(key, TextureSource::File(path.clone()));
    }
}

/// Shared client textures are loose files, including the water maps omitted
/// from zone archives. Index a few known directories once, case-insensitively.
fn loose_textures(base: &Path) -> HashMap<String, PathBuf> {
    let mut textures = HashMap::new();
    for dir in [
        base.to_path_buf(),
        base.join("Resources"),
        base.join("Resources/waterswap"),
    ] {
        let Ok(entries) = std::fs::read_dir(dir) else {
            continue;
        };
        let mut paths: Vec<_> = entries.flatten().map(|entry| entry.path()).collect();
        paths.sort();
        for path in paths {
            let name = path
                .file_name()
                .unwrap()
                .to_string_lossy()
                .to_ascii_lowercase();
            if path.is_file() && (name.ends_with(".dds") || name.ends_with(".bmp")) {
                textures.entry(name).or_insert(path);
            }
        }
    }
    textures
}

fn water_material(material: &TerMaterial) -> Option<WaterMaterial> {
    if !material.shader.eq_ignore_ascii_case("Opaque_MaxWater.fx") {
        return None;
    }
    let float = |name: &str, default| match material.properties.get(name) {
        Some(Property::Float(value)) if value.is_finite() => *value,
        _ => default,
    };
    let color = |name: &str, default| {
        let argb = match material.properties.get(name) {
            Some(Property::Uint(value)) => *value,
            _ => default,
        };
        [16, 8, 0, 24].map(|shift| ((argb >> shift) & 255) as f32 / 255.0)
    };
    Some(WaterMaterial {
        indexed_uv_scale: None,
        color1: color("e_fWaterColor1", 0xFF000A1C),
        color2: color("e_fWaterColor2", 0xFF003B2B),
        reflection_color: color("e_fReflectionColor", 0xFFFFFFFF),
        fresnel_bias: float("e_fFresnelBias", 0.25).clamp(0.0, 1.0),
        fresnel_power: float("e_fFresnelPower", 8.0).max(0.01),
        reflection_amount: float("e_fReflectionAmount", 0.7).clamp(0.0, 1.0),
        environment_map: material
            .properties
            .get("e_TextureEnvironment0")
            .and_then(Property::as_text)
            .filter(|name| !name.eq_ignore_ascii_case("none"))
            .map(str::to_owned),
    })
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

fn case_insensitive_file(base: &Path, name: &str) -> PathBuf {
    let direct = base.join(name);
    if direct.is_file() {
        return direct;
    }
    std::fs::read_dir(base)
        .ok()
        .into_iter()
        .flatten()
        .filter_map(std::result::Result::ok)
        .find(|entry| {
            entry
                .file_name()
                .to_string_lossy()
                .eq_ignore_ascii_case(name)
        })
        .map(|entry| entry.path())
        .unwrap_or(direct)
}

/// Shared terrain metadata selection for rendering and environment queries.
/// An alternate DAT is accepted only through its actual archived declaration.
pub(crate) fn read_heightmap(archive: &Archive, zon: &[u8]) -> Result<terrain::Heightmap> {
    let mut options = terrain::TerrainOptions::parse(zon)?;
    if !archive.contains(&format!("{}.dat", options.name)) {
        // oldcommons ships a renamed ZON alongside commonlands.zon/DAT.
        // Resolve an actual terrain declaration, rather than guessing a DAT.
        if let Some((_, candidate)) = unique_heightmap_declaration(archive)? {
            options = candidate;
        }
    }
    let data = archive.read(&format!("{}.dat", options.name))?;
    terrain::Heightmap::parse(options, &data)
}

fn load_heightmap(base: &Path, name: &str, archive: Archive, zon: &[u8]) -> Result<Scene> {
    let map = read_heightmap(&archive, zon)?;
    let mut ecosystems = terrain::Ecosystems::new();
    for tile in &map.tiles {
        for layer in &tile.layers {
            let key = layer.ecosystem.to_ascii_lowercase();
            if ecosystems.contains_key(&key) {
                continue;
            }
            let data = archive.read(&format!("{key}.eco"))?;
            ecosystems.insert(key, terrain::parse_ecosystem(&data)?);
        }
    }
    let loose = loose_textures(base);
    let prepared = terrain::prepare(&map, &ecosystems, |texture_name| {
        let bytes = archive.read(texture_name).ok().or_else(|| {
            loose
                .get(&texture_name.to_ascii_lowercase())
                .and_then(|path| std::fs::read(path).ok())
        })?;
        Texture::decode(texture_name, &bytes).ok()
    })?;
    let mut scene = Scene::from_prepared_terrain(name.to_owned(), prepared);
    scene.archives.push(archive);
    scene.loose_textures = loose;
    let mut placements = map.placements.clone();
    for group in &map.groups {
        let group_file = format!("{}.tog", group.model);
        let data = scene.archives[0].read(&group_file).or_else(|_| {
            let path = case_insensitive_file(base, &group_file);
            std::fs::read(&path).map_err(|source| Error::Io { path, source })
        });
        let Ok(data) = data else {
            tracing::warn!(zone=name, group=%group.model, "terrain object group is missing from client assets");
            continue;
        };
        for mut placement in terrain::parse_object_group(&data)? {
            placement.transform = group.transform * placement.transform;
            placements.push(placement);
        }
    }
    let mut models = std::collections::BTreeSet::new();
    for placement in &placements {
        models.insert(placement.model.clone());
    }
    let mut loaded = std::collections::HashSet::new();
    for model in models {
        let filename = if model.ends_with(".mod") {
            model.clone()
        } else {
            format!("{model}.mod")
        };
        let Ok(data) = scene.archives[0].read(&filename) else {
            tracing::warn!(
                zone = name,
                model,
                "terrain object model is missing from client archive"
            );
            continue;
        };
        let object = TerMod::parse(&data, false)?;
        append_eqg_object(&mut scene, &object, &model, 0);
        loaded.insert(model);
    }
    for placement in placements {
        if !loaded.contains(&placement.model) {
            continue;
        }
        let (scale, rotation, position) = placement.transform.to_scale_rotation_translation();
        if !scale.is_finite() || !rotation.is_finite() || !position.is_finite() {
            tracing::warn!(zone=name,model=%placement.model,"ignoring invalid terrain object transform");
            continue;
        }
        scene.instances.push(Instance {
            object: placement.model,
            position: position.to_array(),
            scale: scale.to_array(),
            rotation: rotation.to_array(),
        });
    }
    for light in &map.lights {
        let color = scene.archives[0]
            .read(&format!("{}.def", light.definition))
            .ok()
            .and_then(|data| terrain::parse_light_color(&data))
            .unwrap_or([1.0; 3]);
        scene.lights.push(Light {
            position: light.position,
            color,
            radius: light.radius,
            attenuation: 200.0,
        });
    }
    if let Ok(data) = scene.archives[0].read("water.dat") {
        for water in terrain::parse_water(&data)? {
            if water.max[0] <= water.min[0] || water.max[1] <= water.min[1] {
                continue;
            }
            let positions = [
                [water.min[0], water.min[1], water.height],
                [water.max[0], water.min[1], water.height],
                [water.max[0], water.max[1], water.height],
                [water.min[0], water.max[1], water.height],
            ];
            let uv: Vec<_> = positions
                .iter()
                .map(|p| {
                    [
                        p[0] / water.uv_scale.max(0.01),
                        p[1] / water.uv_scale.max(0.01),
                    ]
                })
                .collect();
            let (vertices, indices) =
                mesh::pack(&positions, &[[0.0, 0.0, 1.0]; 4], &uv, &[0, 1, 2, 0, 2, 3]);
            let material = scene.materials.len();
            for texture in [
                Some("water_c.bmp"),
                Some(water.normal_map.as_str()),
                water.material.environment_map.as_deref(),
            ]
            .into_iter()
            .flatten()
            {
                register_texture(&mut scene, 0, texture);
            }
            scene.materials.push(Material {
                textures: vec!["water_c.bmp".into()],
                normal_map: Some(water.normal_map),
                water: Some(water.material),
                flags: 0,
                anim_speed: 0,
                alpha_mask: false,
                transparent: false,
                emissive: false,
                clamp_uv: false,
            });
            scene.meshes.push(Geometry {
                vertices,
                indices,
                material,
                collidable: false,
            });
        }
        indexed_water::append(&mut scene, &map, &data);
    }
    tracing::info!(
        zone = name,
        tiles = map.tiles.len(),
        instances = scene.instances.len(),
        lights = scene.lights.len(),
        "loaded heightmap terrain"
    );
    Ok(scene)
}

#[cfg(test)]
mod declaration_tests;

#[cfg(test)]
mod deferred_terrain_tests;

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
