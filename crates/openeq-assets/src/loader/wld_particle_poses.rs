//! Explicit static attachment diagnostics; no particle activation or draw policy.

use super::{ObjectSource, hierarchy, invalid, object_key};
use crate::Result;
use crate::loader::Instance;
use crate::wld::{Frame, Ref};
use glam::Mat4;

/// An attachment owner matrix in native coordinates for one explicit placement.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ObjectParticleOwnerTransform {
    /// Index in the source object's `particle_attachments`, in unchanged order.
    pub attachment_index: usize,
    pub owner_track: usize,
    pub source_reference: Ref,
    pub definition_reference: Ref,
    /// Native row-vector affine matrix, with XYZ translation in row 3.
    /// This is numerically the column array of the equivalent glam matrix.
    /// Do not transpose it again, convert to render coordinates, or preapply
    /// the particle birth-axis permutation before passing it to a sampler.
    pub native_world_rows: [[f32; 4]; 4],
}

impl ObjectSource {
    /// Extract static native owner matrices without creating or drawing effects.
    ///
    /// The instance must name this actor and have finite translation, a near-unit
    /// rotation, and positive uniform scale. Supported skeletons have one root
    /// at track 0, ordinary unaliased nodes (flags 0), and one packed frame per
    /// track with reference flags 0 and no timing. Packed rotations must be near
    /// unit, but are deliberately **not normalized**: the native initial node
    /// constructor preserves their authored magnitude and positive packed scale.
    /// Recomputed hierarchy and attachment identities must match retained data.
    /// The instance's decoded rotation is used as supplied; this does not
    /// reconstruct the original client's placement-angle table quantization.
    ///
    /// Output uses native EQ XYZ/Z-up coordinates. A returned matrix can still
    /// contain the small distortion of a packed quaternion, and a downstream
    /// sampler may reject it. Extraction does not establish particle body,
    /// texture, visibility, cache, or live playback support. Empty attachment
    /// lists return an empty result. See `docs/WLD_PARTICLE_OWNER_POSES.md`.
    pub fn diagnostic_particle_owner_transforms(
        &self,
        instance: &Instance,
    ) -> Result<Vec<ObjectParticleOwnerTransform>> {
        if instance.object != object_key(&self.actor_name) {
            return Err(invalid(
                "particle placement does not name this source actor",
            ));
        }
        let scale = instance.scale[0];
        if !instance.position.iter().all(|v| v.is_finite())
            || !scale.is_finite()
            || scale <= 0.
            || !(1. / scale).is_finite()
            || instance.scale.iter().any(|v| *v != scale)
            || !near_unit(instance.rotation, 1e-5)
        {
            return Err(invalid("unsupported particle placement transform"));
        }
        if self.particle_attachments.is_empty() {
            return Ok(Vec::new());
        }
        let skeleton = self
            .skeleton
            .as_ref()
            .ok_or_else(|| invalid("particle owners have no skeleton"))?;
        let (parents, order) = hierarchy(&skeleton.definition)?;
        if skeleton.tracks.len() != parents.len()
            || skeleton.parents != parents
            || skeleton.parent_first_order != order
            || parents[0].is_some()
            || parents[1..].iter().any(Option::is_none)
        {
            return Err(invalid(
                "unsupported or inconsistent particle owner hierarchy",
            ));
        }
        if self.particle_attachments.len() > parents.len() {
            return Err(invalid("too many particle owner attachments"));
        }
        let mut attached = vec![false; parents.len()];
        for attachment in &self.particle_attachments {
            let track = skeleton
                .definition
                .tracks
                .get(attachment.owner_track)
                .ok_or_else(|| invalid("particle owner track is out of range"))?;
            if std::mem::replace(&mut attached[attachment.owner_track], true)
                || attachment.source_reference.0 <= 0
                || attachment.source_reference != track.mesh
                || attachment.definition_reference != attachment.source_reference
            {
                return Err(invalid("inconsistent particle attachment identity"));
            }
        }
        let placement = raw_matrix(instance.position, instance.rotation, scale);
        let mut world = vec![Mat4::IDENTITY; parents.len()];
        for index in order {
            let track = &skeleton.tracks[index];
            if skeleton.definition.tracks[index].flags != 0
                || track.definition.flags != 8
                || track.definition.frames.len() != 1
                || track.reference_flags != 0
                || track.speed.is_some()
            {
                return Err(invalid("unsupported particle owner track layout or motion"));
            }
            let frame = &track.definition.frames[0];
            validate_frame(frame)?;
            let local = raw_matrix(frame.translation, frame.rotation, frame.scale);
            world[index] = parents[index].map_or(placement, |parent| world[parent]) * local;
            if !world[index].is_finite() {
                return Err(invalid("particle owner transform overflow"));
            }
            let m = world[index]
                .to_cols_array_2d()
                .map(|row| row.map(f64::from));
            let determinant = m[0][0] * (m[1][1] * m[2][2] - m[1][2] * m[2][1])
                - m[0][1] * (m[1][0] * m[2][2] - m[1][2] * m[2][0])
                + m[0][2] * (m[1][0] * m[2][1] - m[1][1] * m[2][0]);
            if determinant <= 0. {
                return Err(invalid("degenerate or reflected particle owner transform"));
            }
        }
        Ok(self
            .particle_attachments
            .iter()
            .enumerate()
            .map(
                |(attachment_index, attachment)| ObjectParticleOwnerTransform {
                    attachment_index,
                    owner_track: attachment.owner_track,
                    source_reference: attachment.source_reference,
                    definition_reference: attachment.definition_reference,
                    native_world_rows: world[attachment.owner_track].to_cols_array_2d(),
                },
            )
            .collect())
    }
}

fn near_unit(rotation: [f32; 4], tolerance: f64) -> bool {
    rotation.iter().all(|v| v.is_finite())
        && (rotation.iter().map(|v| f64::from(*v).powi(2)).sum::<f64>() - 1.).abs() <= tolerance
}

fn packed_signed(value: f32, divisor: f32) -> bool {
    let word = value * divisor;
    word.is_finite()
        && word.fract() == 0.
        && (f32::from(i16::MIN)..=f32::from(i16::MAX)).contains(&word)
}

fn validate_frame(frame: &Frame) -> Result<()> {
    let scale_word = frame.scale * 256.;
    if !near_unit(frame.rotation, 0.01)
        || frame.rotation.iter().any(|v| !packed_signed(*v, 16384.))
        || frame.translation.iter().any(|v| !packed_signed(*v, 256.))
        || !scale_word.is_finite()
        || scale_word.fract() != 0.
        || !(1. ..=f32::from(u16::MAX)).contains(&scale_word)
    {
        return Err(invalid("unsupported packed particle owner frame"));
    }
    Ok(())
}

fn raw_matrix(position: [f32; 3], rotation: [f32; 4], scale: f32) -> Mat4 {
    // D3DX initial-pose convention uses the raw first-frame W sign. In contrast,
    // the ordinary mesh first-pose helper normalizes. Keep this explicit rather
    // than passing a non-unit quaternion to glam's normalized-quaternion API.
    let [x, y, z, w] = rotation;
    let (x2, y2, z2) = (x + x, y + y, z + z);
    let (xx, xy, xz) = (x * x2, x * y2, x * z2);
    let (yy, yz, zz) = (y * y2, y * z2, z * z2);
    let (wx, wy, wz) = (w * x2, w * y2, w * z2);
    Mat4::from_cols_array_2d(&[
        [
            (1. - (yy + zz)) * scale,
            (xy + wz) * scale,
            (xz - wy) * scale,
            0.,
        ],
        [
            (xy - wz) * scale,
            (1. - (xx + zz)) * scale,
            (yz + wx) * scale,
            0.,
        ],
        [
            (xz + wy) * scale,
            (yz - wx) * scale,
            (1. - (xx + yy)) * scale,
            0.,
        ],
        [position[0], position[1], position[2], 1.],
    ])
}

#[cfg(test)]
#[path = "wld_particle_poses_tests.rs"]
mod tests;
