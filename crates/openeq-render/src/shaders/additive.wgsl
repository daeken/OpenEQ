// AddAlpha_MaxCB1 region contract. Alpha gates visibility; it never weights RGB.
@fragment
fn fs_main(in: Fragment) -> @location(0) vec4<f32> {
    let inset = vec2<f32>(0.5) / vec2<f32>(textureDimensions(atlas));
    let uv = select(in.uv, clamp(in.uv, inset, vec2<f32>(1.0) - inset),
        (in.flags & FLAG_CLAMP_UV) != 0u);
    let texel = textureSample(atlas, atlas_sampler, uv, i32(in.layer));
    if (texel.a < 16.0 / 255.0) { discard; }
    // Reuse current geometric-normal lighting, including zone lights and
    // received shadows. Native bump/baked-light parity is separate work.
    let color = shade_surface_unfogged(in.world, normalize(in.normal), texel.rgb, false);
    return vec4<f32>(color, texel.a);
}
