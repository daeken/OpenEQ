//! Validate retained source bindings before changing the ordinary fallback.
use openeq_assets::{Scene, loader::TerLava};

pub(super) fn recipe(scene: &Scene, index: usize) -> Option<&TerLava> {
    let lava = scene.ter_lava.get(&index)?;
    let material = scene.materials.get(index)?;
    if material.textures.as_slice() != [lava.top.as_str()]
        || material.normal_map.as_deref() != Some(lava.normal.as_str())
        || material.anim_speed != 0
        || material.water.is_some()
        || material.waterfall.is_some()
        || material.alpha_mask
        || material.transparent
        || material.additive
        || material.emissive
        || material.clamp_uv
        || scene.terrain_materials.contains_key(&index)
        || lava.rates.iter().any(|rate| !rate.is_finite())
    {
        return None;
    }
    // Texture decoding failures use the common magenta sentinel. Preserve the
    // existing primary-only fallback if any required binding cannot resolve.
    for name in [&lava.top, &lava.bottom, &lava.normal] {
        let texture = scene.texture(name)?;
        if texture.rgba == [255, 0, 255, 255]
            || texture.width == 0
            || texture.height == 0
            || texture.rgba.len() != texture.width as usize * texture.height as usize * 4
        {
            return None;
        }
    }
    Some(lava)
}
