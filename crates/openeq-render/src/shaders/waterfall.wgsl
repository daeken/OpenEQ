// RegionWaterFall PS1.1 contract: alpha comes from the second sample only.
struct WaterfallParams {
    color1: vec4<f32>,
    color2: vec4<f32>,
    reflection_color: vec4<f32>,
    params: vec4<f32>,
    layers: vec4<u32>,
    scroll_offsets: vec4<f32>,
};
@group(2) @binding(2) var<storage, read> waterfall_materials: array<WaterfallParams>;

@fragment
fn fs_main(in: Fragment) -> @location(0) vec4<f32> {
    let offsets = waterfall_materials[in.material].scroll_offsets;
    let color_uv = in.uv + offsets.xy;
    let alpha_uv = in.uv + offsets.zw;
    let rgb = textureSample(atlas, atlas_sampler, color_uv, i32(in.layer)).rgb;
    let alpha = textureSample(atlas, atlas_sampler, alpha_uv, i32(in.layer)).a;
    if (alpha < 16.0 / 255.0) { discard; }
    // Native baked light, packed normals and three-slot light membership
    // remain separate; retain our current geometric-normal lighting and fog.
    let color = shade_surface(in.world, normalize(in.normal), rgb, false);
    return vec4<f32>(color, alpha);
}
