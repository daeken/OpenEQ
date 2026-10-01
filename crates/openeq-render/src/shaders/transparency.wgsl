// Weighted blended transparency; uses the same lighting and fog as the G-buffer.
struct Accumulation {
    @location(0) color: vec4<f32>,
    @location(1) revealage: f32,
};

@fragment
fn fs_main(in: Fragment) -> Accumulation {
    // Baked tiles are not periodic. Clamp after interpolation to preserve
    // interior UVs and keep linear filtering away from the opposite edge.
    let inset = vec2<f32>(0.5) / vec2<f32>(textureDimensions(atlas));
    let uv = select(in.uv, clamp(in.uv, inset, vec2<f32>(1.0) - inset),
        (in.flags & FLAG_CLAMP_UV) != 0u);
    let texel = textureSampleLevel(atlas, atlas_sampler, uv, i32(in.layer), 0.0);
    let alpha = clamp(texel.a, 0.0, 1.0);
    // Fully opaque interiors already wrote the G-buffer and opaque depth.
    if (alpha <= 0.0 || alpha >= 1.0) { discard; }
    let color = shade_surface(in.world, normalize(in.normal), texel.rgb, (in.flags & FLAG_EMISSIVE) != 0u);
    let distance_to_camera = distance(in.world, globals.camera_pos.xyz);
    // Bounded weights retain half-float headroom, favoring nearer and more
    // opaque fragments. A single layer resolves to exact straight alpha.
    let weight = clamp((alpha * 8.0 + 0.01) / (1.0 + pow(distance_to_camera / 200.0, 2.0)), 0.01, 8.0);
    var out: Accumulation;
    out.color = vec4<f32>(color * alpha, alpha) * weight;
    out.revealage = alpha;
    return out;
}
