//! EverQuest characters with rigid WLD or weighted EQG skeletal animations.
//!
//! A library reads global character archives, a zone's character archives and
//! its `_chr.txt` imports. Models retain their source vertex-to-bone bindings;
//! [`CharacterModel::sample`] skins them on the CPU into the same geometry
//! format used by the zone renderer. Skinning never changes mesh topology.
//! Classic equipment and appearance variants share these skeleton bindings.
//! Modern EQGS/EQGM models use inverse bind matrices and timed EQGA tracks.

mod appearance;
mod modern;
pub use appearance::{CharacterAppearance, EquipmentAppearance};

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use glam::{Mat4, Quat, Vec3};

use crate::mesh::{self, Geometry, Material};
use crate::pfs::Archive;
use crate::texture::Texture;
use crate::wld::{Fragment, Frame, Mesh, PieceTrackRef, Ref, Skeleton, Wld};
use crate::{Error, Result};

/// Client character assets available to a zone. Share this between NPCs and
/// cache loaded models by race/gender; archives and animations need only load once.
pub struct CharacterLibrary {
    archives: Vec<Archive>,
    wlds: Vec<Wld>,
    actors: BTreeMap<String, (usize, usize)>,
    tracks: HashMap<String, (usize, usize)>,
    equipment: BTreeMap<String, (usize, usize)>,
    textures: HashMap<String, (usize, String)>,
    loose_textures: HashMap<String, PathBuf>,
    eqg_files: HashMap<String, PathBuf>,
    modern_archives: Mutex<HashMap<String, Arc<Archive>>>,
    base_models: Mutex<HashMap<String, CharacterModel>>,
    decoded_textures: Mutex<HashMap<String, Texture>>,
}

/// An authored animation. WLD skeletal tracks are normally sampled at 10 Hz;
/// a fragment's explicit frame duration takes precedence when present.
#[derive(Debug, Clone)]
pub struct CharacterAnimation {
    pub frame_time_ms: u32,
    pub frame_count: usize,
    tracks: Vec<Vec<Frame>>,
}

impl CharacterAnimation {
    pub fn duration_seconds(&self) -> f32 {
        self.frame_count as f32 * self.frame_time_ms as f32 / 1000.0
    }
}

#[derive(Debug, Clone)]
struct BoundVertex {
    position: Vec3,
    normal: Vec3,
    bone: usize,
}

/// A reusable, textured character in EQ coordinates (Z up), facing the authored
/// model direction. Spawn position, heading and size belong to the instance.
#[derive(Debug, Clone)]
pub struct CharacterModel {
    pub code: String,
    pub materials: Vec<Material>,
    /// Bind pose geometry, with indices stable across every animation sample.
    pub meshes: Vec<Geometry>,
    /// EQ animation codes, e.g. `P01` (standing), `L01` (walking).
    /// The empty code is the bind pose.
    pub animations: Arc<BTreeMap<String, CharacterAnimation>>,
    pub bounds_min: [f32; 3],
    pub bounds_max: [f32; 3],
    bindings: Vec<Vec<BoundVertex>>,
    material_slots: Vec<Option<usize>>,
    parents: Vec<Option<usize>>,
    bone_order: Vec<usize>,
    /// Original attachment and skeletal track names, in transform order.
    pub bone_names: Vec<String>,
    modern: Option<Arc<modern::ModernModel>>,
}

impl CharacterLibrary {
    pub fn load(base: impl AsRef<Path>, zone: &str) -> Result<Self> {
        let base = base.as_ref();
        let mut files = BTreeMap::new();
        for entry in std::fs::read_dir(base).map_err(|source| Error::Io {
            path: base.to_owned(),
            source,
        })? {
            let entry = entry?;
            files.insert(
                entry.file_name().to_string_lossy().to_ascii_lowercase(),
                entry.path(),
            );
        }
        // Prefer the original global models over their Luclin replacements:
        // the latter use different asset and material conventions.
        let mut wanted = vec!["global_chr.s3d".to_owned()];
        wanted.extend((2..=7).map(|i| format!("global{i}_chr.s3d")));
        wanted.extend([
            "globalfroglok_chr.s3d".to_owned(),
            "globalpcfroglok_chr.s3d".to_owned(),
            // The client's GlobalLoad.txt loads LDON skeletons globally.
            // PEQ uses race 367 even in classic zones such as Greater Faydark.
            "skt_chr.s3d".to_owned(),
        ]);
        wanted.extend(
            files
                .keys()
                .filter(|name| {
                    name.starts_with(&format!("{}_chr", zone.to_ascii_lowercase()))
                        && name.ends_with(".s3d")
                })
                .cloned(),
        );
        if let Some(path) = files.get(&format!("{}_chr.txt", zone.to_ascii_lowercase()))
            && let Ok(imports) = std::fs::read_to_string(path)
        {
            for line in imports.lines() {
                if let Some((_, archive)) = line.split_once(',') {
                    let archive = archive.trim().to_ascii_lowercase();
                    if !archive.is_empty() && !archive.contains(['/', '\\', '.']) {
                        wanted.push(format!("{archive}.s3d"));
                        wanted.push(format!("{archive}2.s3d"));
                    }
                }
            }
        }
        wanted.extend(
            files
                .keys()
                .filter(|name| name.starts_with("gequip") && name.ends_with(".s3d"))
                .cloned(),
        );
        let mut library = Self {
            archives: Vec::new(),
            wlds: Vec::new(),
            actors: BTreeMap::new(),
            equipment: BTreeMap::new(),
            tracks: HashMap::new(),
            textures: HashMap::new(),
            loose_textures: HashMap::new(),
            eqg_files: files
                .iter()
                .filter_map(|(name, path)| {
                    name.strip_suffix(".eqg")
                        .map(|code| (code.to_owned(), path.clone()))
                })
                .collect(),
            modern_archives: Mutex::new(HashMap::new()),
            base_models: Mutex::new(HashMap::new()),
            decoded_textures: Mutex::new(HashMap::new()),
        };
        let mut seen = BTreeSet::new();
        for name in wanted {
            if !seen.insert(name.clone()) {
                continue;
            }
            let Some(path) = files.get(&name) else {
                continue;
            };
            let archive = Archive::open(path)?;
            let archive_index = library.archives.len();
            for filename in archive.names() {
                if filename.to_ascii_lowercase().ends_with(".wld") {
                    let wld = Wld::open(&archive, filename)?;
                    let wld_index = library.wlds.len();
                    for (index, chunk) in wld.chunks().iter().enumerate() {
                        match &chunk.fragment {
                            Fragment::ActorDef(_) => {
                                if let Some(code) = chunk.name.strip_suffix("_ACTORDEF") {
                                    let actors = if name.starts_with("gequip") {
                                        &mut library.equipment
                                    } else {
                                        &mut library.actors
                                    };
                                    actors
                                        .entry(code.to_ascii_uppercase())
                                        .or_insert((wld_index, index));
                                }
                            }
                            Fragment::PieceTrackRef(_) => {
                                library
                                    .tracks
                                    .entry(chunk.name.clone())
                                    .or_insert((wld_index, index));
                            }
                            _ => {}
                        }
                    }
                    library.wlds.push(wld);
                } else {
                    library
                        .textures
                        .entry(filename.to_ascii_lowercase())
                        .or_insert((archive_index, filename.clone()));
                }
            }
            library.archives.push(archive);
        }
        for (name, path) in files {
            if name.ends_with(".dds") || name.ends_with(".bmp") {
                library.loose_textures.insert(name, path);
            }
        }
        if library.actors.is_empty() {
            return Err(Error::NotFound(format!("WLD character models for {zone}")));
        }
        Ok(library)
    }

    pub fn model_codes(&self) -> impl Iterator<Item = &str> {
        self.actors.keys().map(String::as_str)
    }

    pub fn texture(&self, name: &str) -> Option<Texture> {
        if let Some((source, color)) = name.rsplit_once("#tint=") {
            let color = u32::from_str_radix(color, 16).ok()?;
            let mut texture = self.texture(source)?;
            texture.name = name.to_owned();
            let tint = [(color >> 16) as u8, (color >> 8) as u8, color as u8];
            for pixel in texture.rgba.chunks_exact_mut(4) {
                for channel in 0..3 {
                    pixel[channel] =
                        (u16::from(pixel[channel]) * u16::from(tint[channel]) / 255) as u8;
                }
            }
            return Some(texture);
        }
        let (source_name, masked) = name
            .strip_suffix("#masked")
            .map_or((name, false), |source| (source, true));
        let name_lower = source_name.to_ascii_lowercase();
        if let Some(texture) = self
            .decoded_textures
            .lock()
            .ok()?
            .get(&name.to_ascii_lowercase())
        {
            let mut texture = texture.clone();
            texture.name = name.to_owned();
            return Some(texture);
        }
        let (bytes, classic) = if let Some((archive, filename)) = self.textures.get(&name_lower) {
            (self.archives[*archive].read(filename).ok()?, true)
        } else if let Some(path) = self.loose_textures.get(&name_lower) {
            (std::fs::read(path).ok()?, true)
        } else {
            (
                self.modern_archives
                    .lock()
                    .ok()?
                    .values()
                    .find_map(|archive| archive.read(&name_lower).ok())?,
                false,
            )
        };
        let mut texture = if classic {
            Texture::decode_wld_character(name, &bytes).ok()?
        } else {
            Texture::decode(name, &bytes).ok()?
        };
        if masked {
            texture.mask_palette_index_zero(&bytes);
        }
        self.decoded_textures
            .lock()
            .ok()?
            .insert(name.to_ascii_lowercase(), texture.clone());
        Some(texture)
    }

    /// Returns an error for unmapped/unavailable races. The caller may display
    /// an explicit placeholder, rather than silently substituting a human.
    pub fn load_race(&self, race: u32, gender: u8) -> Result<CharacterModel> {
        let code = race_model_code(race, gender).ok_or_else(|| {
            Error::NotFound(format!(
                "character model mapping for race {race}, gender {gender}"
            ))
        })?;
        self.load_model(code)
    }

    pub fn load_model(&self, code: &str) -> Result<CharacterModel> {
        self.load_model_with_appearance(code, &CharacterAppearance::default())
    }

    pub fn load_race_with_appearance(
        &self,
        race: u32,
        gender: u8,
        appearance: &CharacterAppearance,
    ) -> Result<CharacterModel> {
        let code = race_model_code(race, gender).ok_or_else(|| {
            Error::NotFound(format!(
                "character model mapping for race {race}, gender {gender}"
            ))
        })?;
        self.load_model_with_appearance(code, appearance)
    }

    pub fn load_model_with_appearance(
        &self,
        code: &str,
        appearance: &CharacterAppearance,
    ) -> Result<CharacterModel> {
        let code = code.to_ascii_uppercase();
        let cached = self.base_models.lock().unwrap().get(&code).cloned();
        let mut model = if let Some(model) = cached {
            model
        } else {
            let model = self.load_model_uncached(&code)?;
            self.base_models
                .lock()
                .unwrap()
                .insert(code.clone(), model.clone());
            model
        };
        if *appearance != CharacterAppearance::default() {
            if model.modern.is_some() {
                self.apply_modern_appearance(&mut model, appearance);
            } else {
                self.rebuild_appearance_meshes(&mut model, appearance)?;
            }
        }
        Ok(model)
    }

    fn load_model_uncached(&self, code: &str) -> Result<CharacterModel> {
        if !self.actors.contains_key(code) {
            return self.load_modern_model(code);
        }
        let appearance = &CharacterAppearance::default();
        let code = code.to_ascii_uppercase();
        let &(wld_index, actor_index) = self
            .actors
            .get(&code)
            .ok_or_else(|| Error::NotFound(format!("character {code}")))?;
        let wld = &self.wlds[wld_index];
        let Fragment::ActorDef(actor) = &wld.chunks()[actor_index].fragment else {
            unreachable!()
        };
        let skeleton = actor
            .references
            .iter()
            .find_map(
                |reference| match wld.resolve(*reference).map(|c| &c.fragment) {
                    Some(Fragment::SkeletonRef(reference)) => {
                        match wld.resolve(reference.skeleton).map(|c| &c.fragment) {
                            Some(Fragment::Skeleton(skeleton)) => Some(skeleton),
                            _ => None,
                        }
                    }
                    Some(Fragment::Skeleton(skeleton)) => Some(skeleton),
                    _ => None,
                },
            )
            .ok_or_else(|| Error::Format(format!("{code} has no supported WLD skeleton")))?;
        if skeleton.tracks.is_empty() {
            return Err(Error::Format(format!("{code} has an empty skeleton")));
        }
        let (parents, bone_order) = hierarchy(skeleton)?;
        let mut model = CharacterModel {
            code,
            materials: Vec::new(),
            meshes: Vec::new(),
            animations: Arc::new(BTreeMap::new()),
            bounds_min: [f32::INFINITY; 3],
            bounds_max: [f32::NEG_INFINITY; 3],
            bindings: Vec::new(),
            material_slots: Vec::new(),
            parents,
            bone_order,
            bone_names: Vec::new(),
            modern: None,
        };
        let mut seen_meshes = BTreeSet::new();
        let mut original_meshes = Vec::new();
        for reference in &skeleton.meshes {
            if reference.0 == 0 {
                continue;
            }
            let (mesh_ref, mesh) = resolve_mesh(wld, *reference).ok_or_else(|| {
                Error::Format(format!(
                    "{} references a missing character mesh",
                    model.code
                ))
            })?;
            if !seen_meshes.insert(mesh_ref.0) {
                continue;
            }
            original_meshes.push(mesh);
            let mesh = self.appearance_mesh(wld, mesh_ref, mesh, &model.code, appearance);
            let is_head = wld
                .resolve(mesh_ref)
                .is_some_and(|chunk| chunk.name.starts_with(&format!("{}HE", model.code)));
            append_mesh(
                wld,
                mesh,
                skeleton.tracks.len(),
                is_head.then_some(0),
                &mut model,
            )?;
        }
        if model.meshes.is_empty() {
            return Err(Error::Format(format!(
                "{} has no visible character meshes",
                model.code
            )));
        }
        let names: Vec<_> = skeleton
            .tracks
            .iter()
            .map(|bone| {
                wld.reference_name(bone.piece_track)
                    .unwrap_or("")
                    .to_owned()
            })
            .collect();
        model.bone_names = names.clone();
        let donor = animation_source(&model.code);
        let donor_names: Vec<_> = names
            .iter()
            .map(|name| {
                donor.and_then(|donor| {
                    name.strip_prefix(&model.code)
                        .map(|suffix| format!("{donor}{suffix}"))
                })
            })
            .collect();
        let mut prefixes = BTreeSet::from([String::new()]);
        // Some clips omit the root track because its transform never changes.
        // Inspect every bone, rather than requiring the root to be animated.
        for name in names.iter().chain(donor_names.iter().flatten()) {
            if name.is_empty() {
                continue;
            }
            for track_name in self.tracks.keys() {
                if let Some(prefix) = track_name.strip_suffix(name)
                    && prefix.len() == 3
                    && prefix.as_bytes()[0].is_ascii_alphabetic()
                    && prefix.as_bytes()[1..].iter().all(u8::is_ascii_digit)
                {
                    prefixes.insert(prefix.to_owned());
                }
            }
        }
        for prefix in prefixes {
            let mut tracks = Vec::with_capacity(names.len());
            let mut frame_time_ms = None;
            let mut frame_count = 1;
            let has_own_clip = !prefix.is_empty()
                && names
                    .iter()
                    .any(|name| self.tracks.contains_key(&format!("{prefix}{name}")));
            for ((bone, name), donor_name) in skeleton.tracks.iter().zip(&names).zip(&donor_names) {
                let animated = (!prefix.is_empty()).then(|| format!("{prefix}{name}"));
                let frame_track = animated
                    .as_deref()
                    .and_then(|name| self.named_track(name))
                    .or_else(|| {
                        (!has_own_clip && !prefix.is_empty())
                            .then(|| {
                                donor_name
                                    .as_ref()
                                    .and_then(|name| self.named_track(&format!("{prefix}{name}")))
                            })
                            .flatten()
                    });
                let (track_wld, track) = frame_track
                    .or_else(
                        || match wld.resolve(bone.piece_track).map(|c| &c.fragment) {
                            Some(Fragment::PieceTrackRef(track)) => Some((wld, track)),
                            _ => self.named_track(name),
                        },
                    )
                    .ok_or_else(|| Error::Format(format!("missing bone track {name}")))?;
                let frames = match track_wld.resolve(track.track).map(|c| &c.fragment) {
                    Some(Fragment::PieceTrack(track)) if !track.frames.is_empty() => {
                        track.frames.clone()
                    }
                    _ => return Err(Error::Format(format!("missing frames for bone {name}"))),
                };
                if frames.len() > 1 {
                    frame_count = frame_count.max(frames.len());
                    if let Some(speed) = track.speed.filter(|speed| *speed > 0) {
                        frame_time_ms = Some(speed);
                    }
                }
                tracks.push(frames);
            }
            Arc::make_mut(&mut model.animations).insert(
                prefix,
                CharacterAnimation {
                    frame_time_ms: frame_time_ms.unwrap_or(100),
                    frame_count,
                    tracks,
                },
            );
        }
        model.meshes = model.sample("", 0.0);
        // Instance size describes the body, not the selected helmet/robe.
        // Measure the original naked bind meshes so gear changes cannot resize
        // an actor, and keep weapons outside these normalization bounds.
        let transforms = model.bone_transforms("", 0., false).expect("bind pose");
        for mesh in original_meshes {
            let bones: Vec<_> = mesh
                .vertex_pieces
                .iter()
                .flat_map(|(count, bone)| std::iter::repeat_n(*bone as usize, *count as usize))
                .collect();
            for (index, position) in mesh.vertices.iter().enumerate() {
                let bone = bones.get(index).copied().unwrap_or(0);
                let vertex = transforms[bone].transform_point3(Vec3::from_array(*position));
                for axis in 0..3 {
                    model.bounds_min[axis] = model.bounds_min[axis].min(vertex[axis]);
                    model.bounds_max[axis] = model.bounds_max[axis].max(vertex[axis]);
                }
            }
        }
        self.apply_appearance(&mut model, appearance);
        Ok(model)
    }

    fn named_track(&self, name: &str) -> Option<(&Wld, &PieceTrackRef)> {
        let &(wld, index) = self.tracks.get(name)?;
        let wld = &self.wlds[wld];
        match &wld.chunks()[index].fragment {
            Fragment::PieceTrackRef(track) => Some((wld, track)),
            _ => None,
        }
    }
}

impl CharacterModel {
    pub fn idle_animation(&self) -> &str {
        ["P01", "C01", ""]
            .into_iter()
            .find(|code| self.animations.contains_key(*code))
            .unwrap_or("")
    }

    pub fn walk_animation(&self) -> &str {
        ["L01", "L02"]
            .into_iter()
            .find(|code| self.animations.contains_key(*code))
            .unwrap_or_else(|| self.idle_animation())
    }

    /// Skins a looping animation at a continuous time. Translation and bone
    /// rotation interpolate between authored frames; normals are rotated only.
    /// Unknown animation codes use the bind pose.
    pub fn sample(&self, animation: &str, time_seconds: f32) -> Vec<Geometry> {
        let mut meshes = self.meshes.clone();
        self.sample_into(animation, time_seconds, &mut meshes);
        meshes
    }

    /// Reuses geometry buffers from an earlier sample of this model. Returns
    /// false if the supplied mesh topology does not match, without modifying it.
    pub fn sample_into(&self, animation: &str, time_seconds: f32, meshes: &mut [Geometry]) -> bool {
        self.sample_into_mode(animation, time_seconds, true, meshes)
    }

    /// Samples a loop or a one-shot. One-shots hold their last authored frame.
    pub fn sample_into_mode(
        &self,
        animation: &str,
        time_seconds: f32,
        looping: bool,
        meshes: &mut [Geometry],
    ) -> bool {
        if let Some(modern) = &self.modern {
            return modern.sample_into(animation, time_seconds, looping, meshes);
        }
        if meshes.len() != self.bindings.len()
            || meshes
                .iter()
                .zip(&self.bindings)
                .any(|(mesh, bindings)| mesh.vertices.len() != bindings.len() * 8)
        {
            return false;
        }
        let Some(transforms) = self.bone_transforms(animation, time_seconds, looping) else {
            return false;
        };
        for (mesh, bindings) in meshes.iter_mut().zip(&self.bindings) {
            for (vertex, binding) in mesh.vertices.chunks_exact_mut(8).zip(bindings) {
                let transform = transforms[binding.bone];
                vertex[..3]
                    .copy_from_slice(&transform.transform_point3(binding.position).to_array());
                vertex[3..6].copy_from_slice(
                    &transform
                        .transform_vector3(binding.normal)
                        .normalize_or_zero()
                        .to_array(),
                );
            }
        }
        true
    }
    /// Absolute transforms of the named bones, before spawn position/heading.
    /// Equipment attachment points and body vertices use the same transforms.
    pub fn bone_transforms(
        &self,
        animation: &str,
        time_seconds: f32,
        looping: bool,
    ) -> Option<Vec<Mat4>> {
        if let Some(modern) = &self.modern {
            return Some(modern.bone_transforms(animation, time_seconds, looping));
        }
        let clip = self
            .animations
            .get(animation)
            .or_else(|| self.animations.get(""))?;
        let time = if time_seconds.is_finite() {
            time_seconds.max(0.0)
        } else {
            0.0
        };
        let phase = time * 1000.0 / clip.frame_time_ms as f32;
        let phase = if looping {
            phase % clip.frame_count as f32
        } else {
            phase.min(clip.frame_count.saturating_sub(1) as f32)
        };
        let frame = phase.floor() as usize;
        let blend = phase.fract();
        let mut transforms = vec![Mat4::IDENTITY; self.parents.len()];
        for &bone in &self.bone_order {
            let frames = &clip.tracks[bone];
            let index = |frame: usize| {
                if looping {
                    frame % frames.len()
                } else {
                    frame.min(frames.len() - 1)
                }
            };
            let a = frames[index(frame)];
            let b = frames[index(frame + 1)];
            let rotation = rotation(a.rotation).slerp(rotation(b.rotation), blend);
            let translation =
                Vec3::from_array(a.translation).lerp(Vec3::from_array(b.translation), blend);
            let scale = a.scale + (b.scale - a.scale) * blend;
            let local =
                Mat4::from_scale_rotation_translation(Vec3::splat(scale), rotation, translation);
            transforms[bone] =
                self.parents[bone].map_or(local, |parent| transforms[parent] * local);
        }
        Some(transforms)
    }
}

fn rotation(value: [f32; 4]) -> Quat {
    let q = Quat::from_array(value);
    if q.is_finite() && q.length_squared() > 1e-12 {
        q.normalize()
    } else {
        Quat::IDENTITY
    }
}

fn hierarchy(skeleton: &Skeleton) -> Result<(Vec<Option<usize>>, Vec<usize>)> {
    let mut parents = vec![None; skeleton.tracks.len()];
    for (parent, track) in skeleton.tracks.iter().enumerate() {
        for &child in &track.children {
            if child < 0 || child as usize >= parents.len() || child as usize == parent {
                return Err(Error::Format("invalid WLD bone child index".into()));
            }
            if parents[child as usize].replace(parent).is_some() {
                return Err(Error::Format("WLD bone has multiple parents".into()));
            }
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
        order.extend(skeleton.tracks[parent].children.iter().map(|i| *i as usize));
        cursor += 1;
    }
    if order.len() != skeleton.tracks.len() {
        return Err(Error::Format("cycle in WLD skeleton".into()));
    }
    Ok((parents, order))
}

fn resolve_mesh(wld: &Wld, reference: Ref) -> Option<(Ref, &Mesh)> {
    match wld.resolve(reference).map(|c| &c.fragment) {
        Some(Fragment::Mesh(mesh)) => Some((reference, mesh)),
        Some(Fragment::MeshRef(reference)) => {
            match wld.resolve(reference.mesh).map(|c| &c.fragment) {
                Some(Fragment::Mesh(mesh)) => Some((reference.mesh, mesh)),
                _ => None,
            }
        }
        _ => None,
    }
}

fn append_mesh(
    wld: &Wld,
    source: &Mesh,
    bones: usize,
    slot: Option<usize>,
    model: &mut CharacterModel,
) -> Result<()> {
    let mut vertex_bones = Vec::with_capacity(source.vertices.len());
    for &(count, bone) in &source.vertex_pieces {
        if bone as usize >= bones {
            return Err(Error::Format("mesh references invalid WLD bone".into()));
        }
        vertex_bones.extend(std::iter::repeat_n(bone as usize, count as usize));
    }
    if vertex_bones.is_empty() {
        vertex_bones.resize(source.vertices.len(), 0);
    }
    if vertex_bones.len() != source.vertices.len() {
        return Err(Error::Format(
            "WLD bone runs do not cover mesh vertices".into(),
        ));
    }
    let mut groups: BTreeMap<u16, Vec<crate::wld::Polygon>> = BTreeMap::new();
    let mut cursor = 0usize;
    for &(count, material) in &source.polygon_textures {
        let end = cursor + count as usize;
        if end > source.polygons.len() {
            return Err(Error::Format(
                "WLD material runs exceed polygon count".into(),
            ));
        }
        groups
            .entry(material)
            .or_default()
            .extend_from_slice(&source.polygons[cursor..end]);
        cursor = end;
    }
    for (material_index, mut polygons) in groups {
        if polygons.is_empty() {
            continue;
        }
        for polygon in &mut polygons {
            if [polygon.a, polygon.b, polygon.c]
                .iter()
                .any(|i| *i as usize >= source.vertices.len())
            {
                return Err(Error::Format(
                    "WLD character polygon index out of range".into(),
                ));
            }
            polygon.collidable = false;
        }
        // Reuse the zone material resolver, including invisible surfaces and
        // bitmap-layer semantics, while preserving source indices ourselves.
        // Geometric deduplication would incorrectly join vertices on two bones.
        let mut subset = source.clone();
        subset.polygons = polygons;
        subset.polygon_textures = vec![(subset.polygons.len() as u16, material_index)];
        let (materials, _) = mesh::bake_wld_meshes(wld, [&subset]);
        let Some(material) = materials.into_iter().next() else {
            continue;
        };
        let material_index = model
            .materials
            .iter()
            .position(|m| *m == material)
            .unwrap_or_else(|| {
                model.materials.push(material);
                model.material_slots.push(slot);
                model.materials.len() - 1
            });
        let mut remap = HashMap::new();
        let mut vertices = Vec::new();
        let mut indices = Vec::new();
        let mut bindings = Vec::new();
        for polygon in &subset.polygons {
            for index in [polygon.a, polygon.c, polygon.b] {
                let mapped = *remap.entry(index).or_insert_with(|| {
                    let index = index as usize;
                    let next = bindings.len() as u32;
                    vertices.extend(source.vertices[index]);
                    vertices.extend(source.normals[index]);
                    vertices.extend(source.tex_coords[index]);
                    bindings.push(BoundVertex {
                        position: Vec3::from_array(source.vertices[index]),
                        normal: Vec3::from_array(source.normals[index]),
                        bone: vertex_bones[index],
                    });
                    next
                });
                indices.push(mapped);
            }
        }
        model.meshes.push(Geometry {
            vertices,
            indices,
            material: material_index,
            collidable: false,
        });
        model.bindings.push(bindings);
    }
    Ok(())
}

/// Classic race/gender model codes. Gender 0 is male, 1 female, 2 neutral.
/// IDs with a single model ignore gender. Unsupported IDs remain explicit.
/// Citizen models have distinct appearances and are not replaced by PCs.
pub fn race_model_code(race: u32, gender: u8) -> Option<&'static str> {
    let female = gender == 1;
    Some(match race {
        1 => {
            if female {
                "HUF"
            } else {
                "HUM"
            }
        }
        2 => {
            if female {
                "BAF"
            } else {
                "BAM"
            }
        }
        3 => {
            if female {
                "ERF"
            } else {
                "ERM"
            }
        }
        4 => {
            if female {
                "ELF"
            } else {
                "ELM"
            }
        }
        5 => {
            if female {
                "HIF"
            } else {
                "HIM"
            }
        }
        6 => {
            if female {
                "DAF"
            } else {
                "DAM"
            }
        }
        7 => {
            if female {
                "HAF"
            } else {
                "HAM"
            }
        }
        8 => {
            if female {
                "DWF"
            } else {
                "DWM"
            }
        }
        9 => {
            if female {
                "TRF"
            } else {
                "TRM"
            }
        }
        10 => {
            if female {
                "OGF"
            } else {
                "OGM"
            }
        }
        11 => {
            if female {
                "HOF"
            } else {
                "HOM"
            }
        }
        12 => {
            if female {
                "GNF"
            } else {
                "GNM"
            }
        }
        13 => "AVI",
        14 => "WER",
        15 => {
            if female {
                "BRF"
            } else {
                "BRM"
            }
        }
        16 => "CEN",
        17 => "GOL",
        18 => "GIA",
        19 => "TRK",
        21 => "BEH",
        22 => "BET",
        23 => {
            if female {
                "CPF"
            } else {
                "CPM"
            }
        }
        24 => "FIS",
        25 => "FAF",
        26 => "FRO",
        27 => "FRG",
        28 => "FUN",
        29 => "GAR",
        31 => "GEL",
        32 => "GHO",
        33 => "GHU",
        34 => "BAT",
        35 => "EEL",
        36 => "RAT",
        37 => "SNA",
        38 => "SPI",
        39 => "GNN",
        40 => "GOB",
        41 => "GOR",
        42 => "WOL",
        43 => "BEA",
        44 => "FPM",
        45 => "DML",
        46 => "IMP",
        47 => "GRI",
        48 => "KOB",
        50 => {
            if female {
                "LIF"
            } else {
                "LIM"
            }
        }
        51 => "LIZ",
        53 => "MIN",
        54 => "ORC",
        55 => "BGM",
        56 => "PIF",
        58 => "SOL",
        59 => "BGG",
        60 => "SKE",
        61 => "SHA",
        62 => "TUN",
        63 => "TIG",
        64 => "TRE",
        66 => "RAL",
        67 => "HHM",
        68 => "TEN",
        69 => "WIL",
        70 => {
            if female {
                "ZOF"
            } else {
                "ZOM"
            }
        }
        71 => {
            if female {
                "QCF"
            } else {
                "QCM"
            }
        }
        74 => "PIR",
        75 => "ELE",
        76 => "PUM",
        77 => "NGM",
        78 => "EGM",
        80 => "REA",
        81 => {
            if female {
                "RIF"
            } else {
                "RIM"
            }
        }
        82 => "SCA",
        83 => "SKU",
        85 => "SPE",
        86 => "SPH",
        87 => "ARM",
        88 => {
            if female {
                "CLF"
            } else {
                "CLM"
            }
        }
        89 => "DRK",
        90 => {
            if female {
                "HLF"
            } else {
                "HLM"
            }
        }
        91 => "ALL",
        92 => {
            if female {
                "GRF"
            } else {
                "GRM"
            }
        }
        93 => {
            if female {
                "OKF"
            } else {
                "OKM"
            }
        }
        94 => {
            if female {
                "KAF"
            } else {
                "KAM"
            }
        }
        95 => "CAZ",
        96 => "COC",
        106 => {
            if female {
                "FEF"
            } else {
                "FEM"
            }
        }
        107 => "MAM",
        108 => "EYE",
        109 => "WAS",
        110 => "MER",
        112 => {
            if female {
                "GFF"
            } else {
                "GFM"
            }
        }
        113 => "DRI",
        119 => "STC",
        120 => "WOE",
        123 => "INN",
        127 => "IVM",
        128 => {
            if female {
                "IKF"
            } else {
                "IKM"
            }
        }
        130 => {
            if female {
                "KEF"
            } else {
                "KEM"
            }
        }
        131 => "SRW",
        137 => "KGO",
        139 => {
            if female {
                "ICF"
            } else {
                "ICM"
            }
        }
        150 => "ERO",
        151 => "TRI",
        153 => "BRI",
        154 => "FDR",
        155 => "SSK",
        156 => {
            if female {
                "VRF"
            } else {
                "VRM"
            }
        }
        158 => "WUR",
        161 => "IKS",
        166 => "IKH",
        181 => "YAK",
        183 => {
            if female {
                "COF"
            } else {
                "COM"
            }
        }
        187 => "SIR",
        190 => "OTM",
        240 => match gender {
            0 => "TPM",
            1 => "TPF",
            _ => "TPN",
        },
        243 => "NYM",
        330 => {
            if female {
                "FRF"
            } else {
                "FRM"
            }
        }
        367 => "SKT",
        464 => "GGY",
        522 => {
            if female {
                "DKF"
            } else {
                "DKM"
            }
        }
        _ => return None,
    })
}

// Classic client animation sharing. Model-specific clips override a shared
// clip as a whole. These relationships are documented by LanternExtractor's
// ClientData/animationsources.txt; bone names match after removing race codes.
fn animation_source(code: &str) -> Option<&'static str> {
    Some(match code {
        "HUM" | "BAM" | "ERM" | "HIM" | "DAM" | "HAM" | "BRM" | "FPM" | "BGM" | "SKE" | "HHM"
        | "ZOM" | "QCM" | "NGM" | "EGM" | "FEM" | "GFM" => "ELM",
        "HUF" | "BAF" | "ERF" | "HIF" | "DAF" | "HAF" | "BRF" | "ZOF" | "QCF" | "HLF" | "FEF"
        | "GFF" => "ELF",
        "TRM" | "TRF" | "OGM" | "GRM" | "GRF" | "OKM" | "OKF" => "OGF",
        "HOM" | "GNM" | "RIM" | "CLM" | "CL" | "KAM" | "KA" | "COM" | "COK" => "DWM",
        "HOF" | "GNF" | "RIF" | "CLF" | "KAF" | "COF" => "DWF",
        "GOM" | "GOL" => "GIA",
        "BET" => "SPI",
        "CPF" => "CPM",
        "FRG" => "FRO",
        "GAM" | "IMP" => "GAR",
        "GHU" => "GOB",
        "GRI" | "SPH" => "DRK",
        "KOB" => "WER",
        "LIF" | "TIG" | "PUM" | "STC" => "LIM",
        "MIN" => "GNN",
        "PIF" => "FAF",
        "BGG" => "KGO",
        "SKU" | "ARM" => "RAT",
        "IKF" | "ICM" | "ICF" | "ICN" | "IKS" => "IKM",
        "FDF" => "FDR",
        "SSK" => "SRW",
        "VRF" => "VRM",
        "WUR" => "DRA",
        "IKH" => "REA",
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::wld::Track;

    fn track(children: &[i32]) -> Track {
        Track {
            name: String::new(),
            flags: 0,
            piece_track: Ref(0),
            mesh: Ref(0),
            children: children.to_vec(),
        }
    }

    #[test]
    fn hierarchy_orders_parents_and_rejects_cycles() {
        let skeleton = Skeleton {
            tracks: vec![track(&[]), track(&[0])],
            meshes: vec![],
        };
        assert_eq!(
            hierarchy(&skeleton).unwrap(),
            (vec![Some(1), None], vec![1, 0])
        );
        let cycle = Skeleton {
            tracks: vec![track(&[1]), track(&[0])],
            meshes: vec![],
        };
        assert!(hierarchy(&cycle).is_err());
        let shared = Skeleton {
            tracks: vec![track(&[2]), track(&[2]), track(&[])],
            meshes: vec![],
        };
        assert!(hierarchy(&shared).is_err());
    }

    #[test]
    fn skeletal_interpolation_composes_parents_and_does_not_translate_normals() {
        let frame = |translation, rotation| Frame {
            translation,
            rotation,
            scale: 1.0,
        };
        let animation = CharacterAnimation {
            frame_time_ms: 100,
            frame_count: 2,
            tracks: vec![
                vec![frame([10., 0., 0.], Quat::IDENTITY.to_array())],
                vec![
                    frame([0., 2., 0.], Quat::IDENTITY.to_array()),
                    frame(
                        [0., 2., 0.],
                        Quat::from_rotation_z(std::f32::consts::FRAC_PI_2).to_array(),
                    ),
                ],
            ],
        };
        let model = CharacterModel {
            code: "TEST".into(),
            materials: vec![],
            material_slots: vec![],
            meshes: vec![Geometry {
                vertices: vec![0.; 8],
                indices: vec![0],
                material: 0,
                collidable: false,
            }],
            animations: Arc::new(BTreeMap::from([("L01".into(), animation)])),
            bounds_min: [0.; 3],
            bounds_max: [0.; 3],
            parents: vec![None, Some(0)],
            bone_order: vec![0, 1],
            bone_names: vec!["root".into(), "hand".into()],
            modern: None,
            bindings: vec![vec![BoundVertex {
                position: Vec3::X,
                normal: Vec3::X,
                bone: 1,
            }]],
        };
        let pose = model.sample("L01", 0.05);
        let vertex = &pose[0].vertices;
        let s = std::f32::consts::FRAC_1_SQRT_2;
        assert!((vertex[0] - (10. + s)).abs() < 1e-5);
        assert!((vertex[1] - (2. + s)).abs() < 1e-5);
        assert!((vertex[3] - s).abs() < 1e-5);
        assert!((vertex[4] - s).abs() < 1e-5);
        assert_eq!(pose[0].indices, model.meshes[0].indices);
        assert_eq!(
            model.sample("L01", 0.0)[0].vertices,
            model.sample("L01", 0.2)[0].vertices
        );
    }

    #[test]
    fn unsupported_races_are_not_silently_replaced() {
        assert_eq!(race_model_code(1, 0), Some("HUM"));
        assert_eq!(race_model_code(1, 1), Some("HUF"));
        assert_eq!(race_model_code(60, 2), Some("SKE"));
        assert_eq!(race_model_code(367, 2), Some("SKT"));
        assert_eq!(race_model_code(99999, 0), None);
    }
}
