//! Narrow native TER upload provenance; no source attribute conversion.
use crate::{
    mesh::UvEncoding,
    zone::{TerMaterial, TerMod},
};

pub(super) fn encoding(object: &TerMod, material: &TerMaterial) -> UvEncoding {
    // Proven primary SHORT2 upload: ordinary bump terrain and the non-bump
    // waterfall family. MOD and two-UV families require their own evidence.
    // See EQG_TER_UV_PACKING.md and EQG_WATERFALL_UPLOAD.md.
    if object.is_terrain
        && matches!(object.version, 1..=3)
        && (material.shader == "Opaque_MaxCB1.fx"
            || super::waterfall_material(object, material).is_some())
    {
        UvEncoding::NativeTerShort2Sse2
    } else {
        UvEncoding::Float32
    }
}
