//! Actor-owned WLD objects with retained skeletal sources and bounded animation.
//!
//! This retains the source tracks and meshes for future animation and permits
//! explicit authored-frame inspection and bounded native-compatible time
//! sampling. Stationary-collision actors may animate through render bindings.
//! This does not flatten
//! LOD variants or turn mesh names into actor aliases. See
//! `docs/WLD_OBJECT_ANIMATION.md` for timing evidence and supported bounds.

use super::{Scene, SceneObject, append_baked, object_key};
use crate::wld::{
    ActorDef, Chunk, Fragment, Frame, Mesh, ParticleCloud, PieceTrack, Ref, Skeleton, Wld,
};
use crate::{Error, Result, mesh};
use glam::{Mat4, Quat, Vec3};
use std::collections::BTreeSet;
use std::sync::Arc;
use std::time::Duration;

#[path = "wld_object_animation.rs"]
mod animation;
pub use animation::RenderAnimation;

#[path = "wld_object_key_reduction.rs"]
mod key_reduction;

#[path = "wld_object_translation.rs"]
mod translation;

#[path = "wld_object_zero_reduction.rs"]
mod zero_reduction;

#[path = "wld_particle_poses.rs"]
mod particle_poses;
pub use particle_poses::ObjectParticleOwnerTransform;

#[path = "wld_particle_textures.rs"]
mod particle_textures;
use particle_textures::particle_texture_source;
pub use particle_textures::{
    ObjectParticleTexture, ObjectParticleTextureIssue, ObjectParticleTextureNode,
};

const MAX_TRACKS: usize = 4096;
const MAX_PARTS: usize = 4096;
const MAX_FRAMES: usize = 1_000_000;
const MAX_VERTICES: usize = 1_000_000;

/// Decoded source data belonging to one successfully assembled actor.
#[derive(Debug, Clone)]
pub struct ObjectSource {
    pub wld_filename: String,
    pub actor_name: String,
    pub actor: ActorDef,
    pub skeleton: Option<ObjectSkeleton>,
    pub parts: Vec<ObjectPart>,
    /// Unsupported effects retained on an otherwise validated static mesh actor.
    /// A nonempty list means only the ordinary mesh portion is supported.
    pub particle_attachments: Vec<ObjectParticleAttachment>,
    render_animation: Option<RenderAnimation>,
}

/// Source ownership for a particle effect whose playback is not implemented.
#[derive(Debug, Clone)]
pub struct ObjectParticleAttachment {
    pub owner_track: usize,
    /// Original signed attachment reference from the owning skeleton track.
    pub source_reference: Ref,
    /// Exact resolved one-based fragment identity, including same-name copies.
    pub definition_reference: Ref,
    pub name: String,
    pub definition: ParticleCloud,
    /// Authored texture chain only; never a native named-cache substitution.
    /// Unsupported texture metadata does not invalidate ordinary mesh siblings.
    pub texture: ObjectParticleTexture,
}

#[derive(Debug, Clone)]
pub struct ObjectSkeleton {
    /// Original names, child indices, flags and mesh/track references.
    pub definition: Skeleton,
    /// Resolved source tracks, in the definition's track order.
    pub tracks: Vec<ObjectTrack>,
    pub parents: Vec<Option<usize>>,
    pub parent_first_order: Vec<usize>,
}

#[derive(Debug, Clone)]
pub struct ObjectTrack {
    pub definition: PieceTrack,
    /// Original fragment 0x13 flags; distinct from the frame-definition flags.
    pub reference_flags: u32,
    /// Raw optional fragment 0x13 timing word; no runtime timeline is implied.
    pub speed: Option<u32>,
}

impl ObjectSource {
    /// Shared loop period for the supported short packed-WLD animation family.
    ///
    /// All animated tracks must have the same explicit positive interval and
    /// two to five or sixteen frames, reference flags 5, and constant scale.
    /// Five-frame tracks use the native six-to-five rotation-key reduction.
    /// They additionally require packed near-unit keys and a well-separated
    /// reduction choice (or structurally identical tied candidates).
    /// Static tracks require reference flags 4 and no interval. Unknown flags,
    /// Translation changes additionally require five packed frames with constant
    /// rotation, no repeated adjacent positions, and exact unambiguous native
    /// reduction scores. Other translation, floating-point track layouts,
    /// long/mixed clips and degenerate/ambiguous rotations are rejected.
    /// Sixteen-frame clips additionally require constant translation and the
    /// bounded single-axis scalar zero-error reduction policy.
    pub fn animation_period(&self) -> Result<Duration> {
        let skeleton = self
            .skeleton
            .as_ref()
            .ok_or_else(|| invalid("static object has no skeletal animation"))?;
        let (count, interval) = animation_timing(skeleton)?;
        Ok(Duration::from_millis(count as u64 * u64::from(interval)))
    }

    /// Sample supported packed-WLD motion at an explicitly supplied shared time.
    ///
    /// Native time is whole milliseconds. The supplied time is truncated to
    /// milliseconds and wrapped once by the shared animation-set period. This
    /// includes the last-to-first closing interval and normalized linear
    /// quaternion interpolation established in the native D3DX implementation.
    /// No instance phase, runtime clock, bounds or collision state is changed.
    /// Returned meshes preserve source topology and binding ownership, as in
    /// [`Self::sample_authored_frames`]. See [`Self::animation_period`] for bounds.
    pub fn sample_animation(&self, elapsed: Duration) -> Result<Vec<Mesh>> {
        let skeleton = self
            .skeleton
            .as_ref()
            .ok_or_else(|| invalid("static object has no skeletal animation"))?;
        let (count, interval) = animation_timing(skeleton)?;
        let phase = elapsed.as_millis() % (count as u128 * u128::from(interval));
        let index = (phase / u128::from(interval)) as usize;
        let fraction = (phase % u128::from(interval)) as f32 / interval as f32;
        let transforms = pose_with(skeleton, |track| {
            let frames = &skeleton.tracks[track].definition.frames;
            let mut frame = frames[0];
            if frames
                .iter()
                .any(|key| key.translation != frame.translation)
            {
                frame.translation =
                    translation::FiveFrameTranslations::new(frames)?.sample(phase, interval);
            }
            if frames.len() == 5 {
                frame.rotation = key_reduction::FiveFrameKeys::new(frames)?.sample(phase, interval);
            } else if frames.len() == 16 {
                frame.rotation =
                    zero_reduction::SixteenFrameKeys::new(frames)?.sample(phase, interval);
            } else if frames.len() > 1 {
                let a = Quat::from_array(frames[index].rotation);
                let mut b = Quat::from_array(frames[(index + 1) % count].rotation);
                if a.dot(b) < 0. {
                    b = -b;
                }
                // Keep the packed keys' original magnitudes until the blend.
                // Native GetSRT conjugates EQ's negative-W keys, yielding -rawQ;
                // that global sign has no effect on the final rotation matrix.
                frame.rotation = (a + (b - a) * fraction).to_array();
            }
            frame_transform(&frame)
        })?;
        transform_meshes(self, Some(&transforms))
    }

    /// Inspect exact authored frames using the same transform convention as the
    /// initial-pose loader. Supply one frame index per track (empty for static
    /// actors). This does not infer timing, looping, or native animation output.
    ///
    /// The returned mesh copies retain topology and polygon collision flags.
    /// Source meshes and any already-baked scene/collision geometry are unchanged.
    pub fn sample_authored_frames(&self, frame_indices: &[usize]) -> Result<Vec<Mesh>> {
        let transforms = match &self.skeleton {
            Some(skeleton) => Some(pose_at_frames(skeleton, Some(frame_indices))?),
            None if frame_indices.is_empty() => None,
            None => return Err(invalid("static object has no skeletal frames")),
        };
        transform_meshes(self, transforms.as_deref())
    }
}

#[derive(Debug, Clone)]
pub struct ObjectPart {
    /// One-based source mesh fragment, after resolving a MeshRef.
    pub mesh_reference: Ref,
    pub name: String,
    /// Unweighted skeletal parts belong to the track referencing their mesh.
    /// Weighted meshes instead retain their own vertex-piece runs.
    pub rigid_track: Option<usize>,
    /// Original decoded vertices already include the fragment's center.
    pub mesh: Mesh,
}

fn invalid(message: impl Into<String>) -> Error {
    Error::Format(message.into())
}

fn resolve_mesh(wld: &Wld, reference: Ref) -> Result<(usize, &Chunk, &Mesh)> {
    let mut chunk = wld
        .resolve(reference)
        .ok_or_else(|| invalid(format!("missing object mesh reference {}", reference.0)))?;
    if let Fragment::MeshRef(reference) = &chunk.fragment {
        chunk = wld
            .resolve(reference.mesh)
            .ok_or_else(|| invalid("object MeshRef has no mesh"))?;
    }
    let Fragment::Mesh(mesh) = &chunk.fragment else {
        return Err(invalid(format!(
            "unsupported object mesh fragment 0x{:02x}",
            chunk.fragment.type_code()
        )));
    };
    let index = wld
        .chunks()
        .iter()
        .position(|candidate| std::ptr::eq(candidate, chunk))
        .expect("resolved WLD chunk belongs to its source");
    Ok((index, chunk, mesh))
}

fn hierarchy(skeleton: &Skeleton) -> Result<(Vec<Option<usize>>, Vec<usize>)> {
    if skeleton.tracks.is_empty() || skeleton.tracks.len() > MAX_TRACKS {
        return Err(invalid(
            "object skeleton track count outside supported bounds",
        ));
    }
    let mut parents = vec![None; skeleton.tracks.len()];
    for (parent, track) in skeleton.tracks.iter().enumerate() {
        if track.children.len() > skeleton.tracks.len() {
            return Err(invalid("too many object skeleton child references"));
        }
        for &child in &track.children {
            let child = usize::try_from(child)
                .ok()
                .filter(|child| *child < parents.len() && *child != parent)
                .ok_or_else(|| invalid("invalid object skeleton child index"))?;
            if parents[child].replace(parent).is_some() {
                return Err(invalid("object skeleton track has multiple parents"));
            }
        }
    }
    let mut order: Vec<_> = parents
        .iter()
        .enumerate()
        .filter_map(|(index, parent)| parent.is_none().then_some(index))
        .collect();
    let mut cursor = 0;
    while cursor < order.len() {
        order.extend(
            skeleton.tracks[order[cursor]]
                .children
                .iter()
                .map(|child| *child as usize),
        );
        cursor += 1;
    }
    if order.len() != parents.len() {
        return Err(invalid("cycle in object skeleton"));
    }
    Ok((parents, order))
}

fn skeleton_source(wld: &Wld, skeleton: &Skeleton) -> Result<ObjectSkeleton> {
    let (parents, parent_first_order) = hierarchy(skeleton)?;
    if skeleton.meshes.len() > MAX_PARTS {
        return Err(invalid("object skeleton mesh count exceeds limit"));
    }
    let mut frames = 0usize;
    let mut tracks = Vec::with_capacity(skeleton.tracks.len());
    for track in &skeleton.tracks {
        let Some(Fragment::PieceTrackRef(reference)) =
            wld.resolve(track.piece_track).map(|chunk| &chunk.fragment)
        else {
            return Err(invalid("object skeleton has no supported PieceTrackRef"));
        };
        let Some(Fragment::PieceTrack(definition)) =
            wld.resolve(reference.track).map(|chunk| &chunk.fragment)
        else {
            return Err(invalid("object skeleton has no PieceTrack"));
        };
        frames = frames
            .checked_add(definition.frames.len())
            .filter(|count| *count <= MAX_FRAMES)
            .ok_or_else(|| invalid("object skeleton frame count exceeds limit"))?;
        if definition.frames.is_empty() {
            return Err(invalid("object skeleton track has no first frame"));
        }
        tracks.push(ObjectTrack {
            definition: definition.clone(),
            reference_flags: reference.flags,
            speed: reference.speed,
        });
    }
    Ok(ObjectSkeleton {
        definition: skeleton.clone(),
        tracks,
        parents,
        parent_first_order,
    })
}

fn first_pose(source: &ObjectSkeleton) -> Result<Vec<Mat4>> {
    pose_at_frames(source, None)
}

fn animation_timing(source: &ObjectSkeleton) -> Result<(usize, u32)> {
    if source.tracks.len() != source.definition.tracks.len() {
        return Err(invalid("object animation does not match track count"));
    }
    hierarchy(&source.definition)?;
    let mut timing = None;
    for track in &source.tracks {
        let frames = &track.definition.frames;
        if track.definition.flags != 8
            || frames.is_empty()
            || (frames.len() > 5 && frames.len() != 16)
        {
            return Err(invalid(
                "unsupported object animation track layout or length",
            ));
        }
        if frames.len() == 1 {
            if track.reference_flags != 4 || track.speed.is_some() {
                return Err(invalid("unsupported static object animation reference"));
            }
        } else {
            let interval = track.speed.filter(|interval| {
                // Every key timestamp must be an exactly representable float.
                *interval > 0 && *interval <= (1 << 24) / frames.len() as u32
            });
            if track.reference_flags != 5 || interval.is_none() {
                return Err(invalid("unsupported object animation timing or reference"));
            }
            let candidate = (frames.len(), interval.unwrap());
            if timing.is_some_and(|timing| timing != candidate) {
                return Err(invalid("mixed object animation timelines are unsupported"));
            }
            timing = Some(candidate);
        }
        for frame in frames {
            frame_transform(frame)?;
            if frame.scale != frames[0].scale {
                return Err(invalid("changing object animation scale is unsupported"));
            }
        }
        if frames
            .iter()
            .any(|frame| frame.translation != frames[0].translation)
        {
            translation::FiveFrameTranslations::new(frames)?;
        }
        for (index, frame) in frames.iter().enumerate() {
            let next = &frames[(index + 1) % frames.len()];
            let dot = Quat::from_array(frame.rotation)
                .normalize()
                .dot(Quat::from_array(next.rotation).normalize());
            if !dot.is_finite() || dot.abs() < 1e-6 {
                return Err(invalid("ambiguous object animation quaternion hemisphere"));
            }
        }
        if frames.len() == 5 {
            key_reduction::FiveFrameKeys::new(frames)?;
        } else if frames.len() == 16 {
            zero_reduction::SixteenFrameKeys::new(frames)?;
        }
    }
    timing.ok_or_else(|| invalid("object has no animated tracks"))
}

fn frame_transform(frame: &Frame) -> Result<Mat4> {
    let rotation = Quat::from_array(frame.rotation);
    if !rotation.is_finite()
        || !rotation.length_squared().is_finite()
        || rotation.length_squared() <= 1e-12
        || !frame.translation.iter().all(|value| value.is_finite())
        || !frame.scale.is_finite()
        || frame.scale <= 0.
    {
        return Err(invalid("unsupported nonfinite or degenerate object pose"));
    }
    Ok(Mat4::from_scale_rotation_translation(
        Vec3::splat(frame.scale),
        rotation.normalize(),
        Vec3::from_array(frame.translation),
    ))
}

fn pose_at_frames(source: &ObjectSkeleton, frame_indices: Option<&[usize]>) -> Result<Vec<Mat4>> {
    if source.tracks.len() != source.definition.tracks.len()
        || frame_indices.is_some_and(|indices| indices.len() != source.tracks.len())
    {
        return Err(invalid("object frame selection does not match track count"));
    }
    pose_with(source, |index| {
        let frame_index = frame_indices.map_or(0, |indices| indices[index]);
        let frame = source.tracks[index]
            .definition
            .frames
            .get(frame_index)
            .ok_or_else(|| invalid("object frame selection is out of range"))?;
        frame_transform(frame)
    })
}

fn pose_with(
    source: &ObjectSkeleton,
    mut local_pose: impl FnMut(usize) -> Result<Mat4>,
) -> Result<Vec<Mat4>> {
    let (parents, order) = hierarchy(&source.definition)?;
    let mut transforms = vec![Mat4::IDENTITY; source.tracks.len()];
    for index in order {
        let local = local_pose(index)?;
        transforms[index] = parents[index].map_or(local, |parent| transforms[parent] * local);
        if !transforms[index].is_finite() {
            return Err(invalid("object skeleton transform overflow"));
        }
    }
    Ok(transforms)
}

fn make_part(
    wld: &Wld,
    reference: Ref,
    rigid_track: Option<usize>,
    vertices: &mut usize,
) -> Result<ObjectPart> {
    let (index, chunk, mesh) = resolve_mesh(wld, reference)?;
    *vertices = vertices
        .checked_add(mesh.vertices.len())
        .filter(|count| *count <= MAX_VERTICES)
        .ok_or_else(|| invalid("object vertex count exceeds limit"))?;
    Ok(ObjectPart {
        mesh_reference: Ref(
            i32::try_from(index + 1).map_err(|_| invalid("object mesh reference overflow"))?
        ),
        name: chunk.name.clone(),
        rigid_track,
        mesh: mesh.clone(),
    })
}

fn actor_source(wld: &Wld, chunk: &Chunk, actor: &ActorDef) -> Result<ObjectSource> {
    // All 12,388 actors in the installed *_obj.s3d corpus have one reference.
    // Additional references can represent variants; do not combine them blindly.
    if actor.references.len() != 1 {
        return Err(invalid("unsupported object actor reference count"));
    }
    let reference = actor.references[0];
    let mut target = wld
        .resolve(reference)
        .ok_or_else(|| invalid("object actor reference is unresolved"))?;
    if let Fragment::SkeletonRef(reference) = &target.fragment {
        target = wld
            .resolve(reference.skeleton)
            .ok_or_else(|| invalid("object SkeletonRef is unresolved"))?;
    }
    let mut vertices = 0;
    let mut parts = Vec::new();
    let mut particle_attachments = Vec::new();
    let skeleton = if let Fragment::Skeleton(skeleton) = &target.fragment {
        let source = skeleton_source(wld, skeleton)?;
        let mut referenced = BTreeSet::new();
        // A rigid mesh can appear on more than one track. Each reference is a
        // distinct part, so mesh identity alone must not suppress a placement.
        for (index, track) in skeleton.tracks.iter().enumerate() {
            if track.mesh.0 == 0 {
                continue;
            }
            if let Some(attachment) = track
                .mesh
                .fragment_index()
                .and_then(|index| wld.chunks().get(index))
                && let Fragment::ParticleCloud(definition) = &attachment.fragment
            {
                // Partial restoration is deliberately limited to the record
                // family proven in PoK. Retain other layouts in Wld, but do not
                // silently treat an unknown attachment layout as supported.
                if definition.flags() != 4 || !definition.tail.is_empty() {
                    return Err(invalid("unsupported object particle definition layout"));
                }
                let fragment_index = wld
                    .chunks()
                    .iter()
                    .position(|candidate| std::ptr::eq(candidate, attachment))
                    .expect("resolved particle chunk belongs to its source");
                particle_attachments.push(ObjectParticleAttachment {
                    owner_track: index,
                    source_reference: track.mesh,
                    definition_reference: Ref(i32::try_from(fragment_index + 1)
                        .map_err(|_| invalid("object particle reference overflow"))?),
                    name: attachment.name.clone(),
                    definition: definition.clone(),
                    texture: particle_texture_source(wld, definition.texture_reference),
                });
                continue;
            }
            let (mesh_index, _, mesh) = resolve_mesh(wld, track.mesh)?;
            if mesh.vertex_pieces.is_empty() || referenced.insert(mesh_index) {
                parts.push(make_part(wld, track.mesh, Some(index), &mut vertices)?);
            }
            referenced.insert(mesh_index);
        }
        for &reference in &skeleton.meshes {
            if reference.0 == 0 {
                continue;
            }
            let (index, _, mesh) = resolve_mesh(wld, reference)?;
            if referenced.insert(index) {
                if mesh.vertex_pieces.is_empty() {
                    return Err(invalid("unweighted object mesh has no referencing track"));
                }
                parts.push(make_part(wld, reference, None, &mut vertices)?);
            }
        }
        Some(source)
    } else {
        parts.push(make_part(wld, reference, None, &mut vertices)?);
        None
    };
    if parts.is_empty() || parts.len() > MAX_PARTS {
        return Err(invalid("object actor part count outside supported bounds"));
    }
    let source = ObjectSource {
        wld_filename: wld.filename.clone(),
        actor_name: chunk.name.clone(),
        actor: actor.clone(),
        skeleton,
        parts,
        particle_attachments,
        render_animation: None,
    };
    if !source.particle_attachments.is_empty() {
        validate_partial_particle_actor(&source)?;
    }
    Ok(source)
}

fn validate_partial_particle_actor(source: &ObjectSource) -> Result<()> {
    let skeleton = source
        .skeleton
        .as_ref()
        .ok_or_else(|| invalid("object particle attachments have no skeleton"))?;
    if skeleton.tracks.iter().any(|track| {
        track.definition.flags != 8
            || track.definition.frames.len() != 1
            || track.reference_flags != 0
            || track.speed.is_some()
    }) {
        return Err(invalid(
            "unsupported animated or nonpacked particle-linked actor",
        ));
    }
    // Includes every track's finite first pose, complete vertex runs, normals
    // and polygon indices; hidden collision geometry receives the same checks.
    posed_meshes(source)?;
    let mut particle_ancestry = vec![false; skeleton.tracks.len()];
    for attachment in &source.particle_attachments {
        particle_ancestry[attachment.owner_track] = true;
    }
    for &index in &skeleton.parent_first_order {
        if let Some(parent) = skeleton.parents[index] {
            particle_ancestry[index] |= particle_ancestry[parent];
        }
    }
    for part in &source.parts {
        if part_bindings(part, skeleton.tracks.len())?
            .iter()
            .any(|&track| particle_ancestry[track])
        {
            return Err(invalid(
                "object mesh depends on a particle attachment ancestry",
            ));
        }
    }
    Ok(())
}

fn posed_meshes(source: &ObjectSource) -> Result<Vec<Mesh>> {
    let transforms = source.skeleton.as_ref().map(first_pose).transpose()?;
    transform_meshes(source, transforms.as_deref())
}

fn part_bindings(part: &ObjectPart, track_count: usize) -> Result<Vec<usize>> {
    let mesh = &part.mesh;
    let bindings = if mesh.vertex_pieces.is_empty() {
        let track = part
            .rigid_track
            .filter(|track| *track < track_count)
            .ok_or_else(|| invalid("unweighted object part has no track"))?;
        vec![track; mesh.vertices.len()]
    } else {
        let mut bindings = Vec::with_capacity(mesh.vertices.len());
        for &(count, track) in &mesh.vertex_pieces {
            if track as usize >= track_count
                || bindings.len() + count as usize > mesh.vertices.len()
            {
                return Err(invalid("invalid object vertex-piece run"));
            }
            bindings.extend(std::iter::repeat_n(track as usize, count as usize));
        }
        if bindings.len() != mesh.vertices.len() {
            return Err(invalid("object vertex-piece runs do not cover mesh"));
        }
        bindings
    };
    Ok(bindings)
}

fn transform_meshes(source: &ObjectSource, transforms: Option<&[Mat4]>) -> Result<Vec<Mesh>> {
    source
        .parts
        .iter()
        .map(|part| {
            let mut mesh = part.mesh.clone();
            if mesh.normals.len() != mesh.vertices.len()
                || mesh.tex_coords.len() != mesh.vertices.len()
                || !mesh
                    .vertices
                    .iter()
                    .flatten()
                    .chain(mesh.normals.iter().flatten())
                    .all(|value| value.is_finite())
                || mesh.polygons.iter().any(|polygon| {
                    [polygon.a, polygon.b, polygon.c]
                        .iter()
                        .any(|index| *index as usize >= mesh.vertices.len())
                })
            {
                return Err(invalid(
                    "invalid object mesh vertex attributes or polygon indices",
                ));
            }
            let Some(transforms) = transforms else {
                if !mesh.vertex_pieces.is_empty() {
                    return Err(invalid("object bone runs have no skeleton"));
                }
                return Ok(mesh);
            };
            let bindings = part_bindings(part, transforms.len())?;
            for (index, &track) in bindings.iter().enumerate() {
                // read_mesh already included center in every decoded vertex.
                mesh.vertices[index] = transforms[track]
                    .transform_point3(Vec3::from_array(mesh.vertices[index]))
                    .to_array();
                // All authored skeletal scales are uniform. Normalizing the
                // transformed vector matches inverse-transpose direction.
                mesh.normals[index] = transforms[track]
                    .transform_vector3(Vec3::from_array(mesh.normals[index]))
                    .normalize_or_zero()
                    .to_array();
                if !mesh.vertices[index].iter().all(|value| value.is_finite()) {
                    return Err(invalid("object posed vertex is nonfinite"));
                }
            }
            Ok(mesh)
        })
        .collect()
}

fn append_group(scene: &mut Scene, archive: usize, wld: &Wld, name: String, meshes: &[Mesh]) {
    append_group_bound(scene, archive, wld, name, meshes, false);
}

fn append_group_bound(
    scene: &mut Scene,
    archive: usize,
    wld: &Wld,
    name: String,
    meshes: &[Mesh],
    preserve_sources: bool,
) -> Vec<Vec<usize>> {
    let collision_start = scene.collision_meshes.len();
    scene
        .collision_meshes
        .extend(mesh::bake_wld_collision_meshes(wld, meshes));
    let start = scene.meshes.len();
    let (materials, geometries, bindings) = if preserve_sources {
        mesh::bake_wld_meshes_with_sources(wld, meshes)
    } else {
        let (materials, geometries) = mesh::bake_wld_meshes(wld, meshes);
        (materials, geometries, Vec::new())
    };
    append_baked(scene, archive, materials, geometries);
    scene.objects.push(SceneObject {
        name,
        meshes: (start..scene.meshes.len()).collect(),
        collision_meshes: (collision_start..scene.collision_meshes.len()).collect(),
    });
    bindings
}

/// Unsupported actor variants are diagnosed independently. Their original
/// standalone meshes remain available without an invented actor alias.
pub(super) fn append_objects(scene: &mut Scene, archive: usize, wld: &Wld) -> Result<()> {
    let mut names: BTreeSet<_> = scene
        .objects
        .iter()
        .map(|object| object.name.clone())
        .collect();
    for (chunk, actor) in wld.iter::<ActorDef>() {
        let name = object_key(&chunk.name);
        if name.is_empty() || names.contains(&name) {
            tracing::warn!(wld=%wld.filename,actor=%chunk.name,"skipped empty or duplicate WLD actor name");
            continue;
        }
        let result = actor_source(wld, chunk, actor)
            .and_then(|source| posed_meshes(&source).map(|meshes| (source, meshes)));
        let (mut source, meshes) = match result {
            Ok(value) => value,
            Err(error) => {
                tracing::warn!(wld=%wld.filename,actor=%chunk.name,%error,"unsupported WLD placed actor");
                continue;
            }
        };
        if !source.particle_attachments.is_empty() {
            tracing::warn!(
                wld=%wld.filename,
                actor=%chunk.name,
                particle_attachments=source.particle_attachments.len(),
                "restored static WLD actor meshes; attached particle playback is unsupported"
            );
        }
        let radius = source.stationary_collision_animation_radius().ok();
        let bindings =
            append_group_bound(scene, archive, wld, name.clone(), &meshes, radius.is_some());
        if let Some(radius) = radius {
            source.render_animation = Some(RenderAnimation::new(bindings, radius));
        }
        names.insert(name.clone());
        scene.wld_object_sources.insert(name, Arc::new(source));
    }
    // Preserve historical raw model lookups used by doors and object-library
    // callers when their keys differ. These are independent bakes, never
    // aliases sharing another object's geometry indices.
    for chunk in wld.chunks() {
        let Fragment::Mesh(mesh) = &chunk.fragment else {
            continue;
        };
        let name = object_key(&chunk.name);
        if name.is_empty() || !names.insert(name.clone()) {
            continue;
        }
        append_group(scene, archive, wld, name, std::slice::from_ref(mesh));
    }
    Ok(())
}

#[cfg(test)]
#[path = "wld_objects_tests.rs"]
mod tests;
