//! Native TER UV packing, decoded to the existing float shader interface.
use openeq_assets::mesh::UvEncoding;

pub(super) fn shader_uv(encoding: UvEncoding, raw: [f32; 2]) -> [f32; 2] {
    match encoding {
        UvEncoding::Float32 => raw,
        UvEncoding::NativeTerShort2Sse2 => raw.map(short2_sse2),
    }
}

fn short2_sse2(value: f32) -> f32 {
    f32::from(short2_word(value)) / 256.0
}

pub(super) fn packed_short2(raw: [f32; 2]) -> u32 {
    u32::from(short2_word(raw[0]) as u16) | (u32::from(short2_word(raw[1]) as u16) << 16)
}

fn short2_word(value: f32) -> i16 {
    // Native x87 multiplication followed by FSTP double preserves every
    // finite f32 * 256 exactly; doing the multiply in f32 could overflow.
    let scaled = f64::from(value) * 256.0;
    // CVTTSD2SI returns integer indefinite for NaN/overflow with invalid
    // masked. Rust's saturating float cast alone would disagree. Its low
    // word is zero; valid conversions retain low bits rather than clamp.
    let integer = if (-2_147_483_648.0..2_147_483_648.0).contains(&scaled) {
        scaled as i32
    } else {
        i32::MIN
    };
    integer as i16
}

#[cfg(test)]
mod tests;
