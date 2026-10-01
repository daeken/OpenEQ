//! Narrow native TER upload provenance; no source attribute conversion.
use crate::{
    mesh::UvEncoding,
    zone::{TerMaterial, TerMod},
};

pub(super) fn encoding(object: &TerMod, material: &TerMaterial) -> UvEncoding {
    // This exact family has one primary-UV bump layout for every established
    // TER record version. Neither MOD nor similarly named/two-UV families
    // inherit this evidence. See docs/EQG_TER_UV_PACKING.md.
    if object.is_terrain && matches!(object.version, 1..=3) && material.shader == "Opaque_MaxCB1.fx"
    {
        UvEncoding::NativeTerShort2Sse2
    } else {
        UvEncoding::Float32
    }
}
