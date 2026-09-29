@group(0) @binding(0) var accumulation: texture_2d<f32>;
@group(0) @binding(1) var revealage: texture_2d<f32>;

@vertex
fn vs_main(@builtin(vertex_index) index: u32) -> @builtin(position) vec4<f32> {
    let x = f32((index << 1u) & 2u);
    let y = f32(index & 2u);
    return vec4<f32>(x * 2.0 - 1.0, y * 2.0 - 1.0, 0.0, 1.0);
}

@fragment
fn fs_main(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
    let pixel = vec2<i32>(position.xy);
    let accum = textureLoad(accumulation, pixel, 0);
    let alpha = 1.0 - clamp(textureLoad(revealage, pixel, 0).r, 0.0, 1.0);
    let color = accum.rgb / max(accum.a, 0.00001);
    // Composite in linear color with premultiplied alpha over opaque lighting.
    return vec4<f32>(color * alpha, alpha);
}
