// Standalone WebGPU experiment, not an original-client rasterization claim.
struct Screen {
    origin_size: vec4<f32>,
    pixel_offset: vec4<f32>,
};
@group(0) @binding(0) var<uniform> screen: Screen;
@group(1) @binding(0) var source: texture_2d<f32>;
@group(1) @binding(1) var source_sampler: sampler;

struct Output {
    @builtin(position) position: vec4<f32>,
    @location(0) color: vec4<f32>,
    @location(1) uv: vec2<f32>,
};

@vertex
fn vs_main(@location(0) xyzrhw: vec4<f32>,
           @location(1) diffuse: u32,
           @location(2) uv: vec2<f32>) -> Output {
    let xy = xyzrhw.xy + screen.pixel_offset.xy;
    let ndc = (xy - screen.origin_size.xy) / screen.origin_size.zw * 2.0 - 1.0;
    let w = 1.0 / xyzrhw.w;
    var out: Output;
    out.position = vec4<f32>(ndc.x * w, -ndc.y * w, xyzrhw.z * w, w);
    out.color = vec4<f32>(f32((diffuse >> 16u) & 255u),
                          f32((diffuse >> 8u) & 255u),
                          f32(diffuse & 255u), f32(diffuse >> 24u)) / 255.0;
    // Captured WLD vertices already contain the V reversal. Do not flip twice.
    out.uv = uv;
    return out;
}

@fragment
fn fs_main(in: Output) -> @location(0) vec4<f32> {
    // Explicit level zero: native driver-generated mipmaps remain unverified.
    let modulated = textureSampleLevel(source, source_sampler, in.uv, 0.0) * in.color;
    if (modulated.a < 1.0 / 255.0) { discard; }
    // Straight source; both RGB and alpha use SRCALPHA / ONE / ADD.
    return modulated;
}
