//! Bounded CB1_2UV color path. Native normal/light/color-space parity is separate.
use openeq_assets::{
    Scene,
    loader::{PackedTerSecondaryUv, TerColorBlend},
};

pub(super) fn channel(
    scene: &Scene,
    mesh: usize,
) -> Option<(&PackedTerSecondaryUv, &TerColorBlend)> {
    let geometry = scene.meshes.get(mesh)?;
    let material = scene.materials.get(geometry.material)?;
    let channel = scene.secondary_ter_uv.get(&mesh)?;
    let blend = channel.color_blend.as_ref()?;
    if channel.tex_coords.len() != geometry.vertex_count()
        || channel.source_indices.len() != geometry.vertex_count()
        || material.textures.as_slice() != [blend.diffuse.as_str()]
        || material.normal_map.as_deref() != Some(blend.normal.as_str())
        || material.water.is_some()
        || material.waterfall.is_some()
        || material.alpha_mask
        || material.transparent
        || material.additive
        || material.clamp_uv
        || scene.terrain_materials.contains_key(&geometry.material)
    {
        return None;
    }
    Some((channel, blend))
}
