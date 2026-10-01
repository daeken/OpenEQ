//! Actor-owned WLD objects, sampled at the first authored skeletal frame.
//!
//! This retains the source tracks and meshes for future animation. It does not
//! choose an animation rate, flatten LOD variants, or turn mesh names into actor
//! aliases. See `docs/WLD_PLACED_OBJECTS.md` for evidence and supported bounds.

use super::{Scene, SceneObject, append_baked, object_key};
use crate::wld::{ActorDef, Chunk, Fragment, Mesh, PieceTrack, Ref, Skeleton, Wld};
use crate::{Error, Result, mesh};
use glam::{Mat4, Quat, Vec3};
use std::collections::BTreeSet;
use std::sync::Arc;

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
    /// Raw optional source speed; no guessed default or playback conversion.
    pub speed: Option<u32>,
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
    let mut transforms = vec![Mat4::IDENTITY; source.tracks.len()];
    for &index in &source.parent_first_order {
        let frame = source.tracks[index].definition.frames[0];
        let rotation = Quat::from_array(frame.rotation);
        if !rotation.is_finite()
            || !rotation.length_squared().is_finite()
            || rotation.length_squared() <= 1e-12
            || !frame.translation.iter().all(|value| value.is_finite())
            || !frame.scale.is_finite()
            || frame.scale <= 0.
        {
            return Err(invalid(
                "unsupported nonfinite or degenerate object first pose",
            ));
        }
        let local = Mat4::from_scale_rotation_translation(
            Vec3::splat(frame.scale),
            rotation.normalize(),
            Vec3::from_array(frame.translation),
        );
        transforms[index] =
            source.parents[index].map_or(local, |parent| transforms[parent] * local);
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
    let skeleton = if let Fragment::Skeleton(skeleton) = &target.fragment {
        let source = skeleton_source(wld, skeleton)?;
        let mut referenced = BTreeSet::new();
        // A rigid mesh can appear on more than one track. Each reference is a
        // distinct part, so mesh identity alone must not suppress a placement.
        for (index, track) in skeleton.tracks.iter().enumerate() {
            if track.mesh.0 == 0 {
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
    Ok(ObjectSource {
        wld_filename: wld.filename.clone(),
        actor_name: chunk.name.clone(),
        actor: actor.clone(),
        skeleton,
        parts,
    })
}

fn posed_meshes(source: &ObjectSource) -> Result<Vec<Mesh>> {
    let transforms = source.skeleton.as_ref().map(first_pose).transpose()?;
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
            let Some(transforms) = &transforms else {
                if !mesh.vertex_pieces.is_empty() {
                    return Err(invalid("object bone runs have no skeleton"));
                }
                return Ok(mesh);
            };
            let bindings = if mesh.vertex_pieces.is_empty() {
                let track = part
                    .rigid_track
                    .ok_or_else(|| invalid("unweighted object part has no track"))?;
                vec![track; mesh.vertices.len()]
            } else {
                let mut bindings = Vec::with_capacity(mesh.vertices.len());
                for &(count, track) in &mesh.vertex_pieces {
                    if track as usize >= transforms.len()
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
    let collision_start = scene.collision_meshes.len();
    scene
        .collision_meshes
        .extend(mesh::bake_wld_collision_meshes(wld, meshes));
    let start = scene.meshes.len();
    let (materials, geometries) = mesh::bake_wld_meshes(wld, meshes);
    append_baked(scene, archive, materials, geometries);
    scene.objects.push(SceneObject {
        name,
        meshes: (start..scene.meshes.len()).collect(),
        collision_meshes: (collision_start..scene.collision_meshes.len()).collect(),
    });
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
        let (source, meshes) = match result {
            Ok(value) => value,
            Err(error) => {
                tracing::warn!(wld=%wld.filename,actor=%chunk.name,%error,"unsupported WLD placed actor");
                continue;
            }
        };
        append_group(scene, archive, wld, name.clone(), &meshes);
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
