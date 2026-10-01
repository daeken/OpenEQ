// RegionLava programmable color expression under current OpenEQ lighting.
struct LavaParams {
    color1: vec4<f32>,
    color2: vec4<f32>,
    reflection_color: vec4<f32>,
    params: vec4<f32>,
    layers: vec4<u32>,
    scroll_offsets: vec4<f32>,
};
@group(2) @binding(2) var<storage, read> lava_materials: array<LavaParams>;

@fragment
fn fs_main(in: Fragment) -> @location(0) vec4<f32> {
    let material = lava_materials[in.material];
    let top = textureSample(atlas, atlas_sampler, in.uv + material.scroll_offsets.xy, i32(in.layer));
    let bottom = textureSample(atlas, atlas_sampler, in.uv + material.scroll_offsets.zw, i32(material.layers.x));
    let lit_top = shade_surface_unfogged(in.world, normalize(in.normal), top.rgb, false);
    // Top alpha blends the two source layers; final surface remains opaque.
    let combined = mix(2.0 * bottom.rgb, lit_top, top.a);
    return vec4<f32>(apply_fog(combined, distance(in.world, globals.camera_pos.xyz)), 1.0);
}
