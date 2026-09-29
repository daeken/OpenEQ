// Weighted blended transparency; uses the same lighting and fog as the G-buffer.
@group(2) @binding(0) var atlas: texture_2d_array<f32>;
@group(2) @binding(1) var atlas_sampler: sampler;
const FLAG_EMISSIVE: u32 = 4u;

struct Vertex {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
    @location(3) layer: u32,
    @location(4) material: u32,
    @location(5) frame_count: u32,
    @location(6) flags: u32,
    @location(7) frame_ms: u32,
};

struct Instance {
    @location(8) col0: vec4<f32>,
    @location(9) col1: vec4<f32>,
    @location(10) col2: vec4<f32>,
    @location(11) col3: vec4<f32>,
};

struct Fragment {
    @builtin(position) clip: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) world: vec3<f32>,
    @location(2) normal: vec3<f32>,
    @location(3) @interpolate(flat) layer: u32,
    @location(4) @interpolate(flat) flags: u32,
    @location(5) @interpolate(flat) material: u32,
};

@vertex
fn vs_main(vertex: Vertex, instance: Instance) -> Fragment {
    let model = mat4x4<f32>(instance.col0, instance.col1, instance.col2, instance.col3);
    let local = model * vec4<f32>(vertex.position, 1.0);
    let world = EQ_TO_WORLD * local;

    var out: Fragment;
    out.clip = globals.view_proj * world;
    out.world = world.xyz;
    out.normal = normalize((EQ_TO_WORLD * model * vec4<f32>(vertex.normal, 0.0)).xyz);
    out.uv = vertex.uv;
    out.flags = vertex.flags;
    out.material = vertex.material;

    // Animated textures are stored as consecutive layers; pick the frame.
    let count = max(vertex.frame_count, 1u);
    let step = max(vertex.frame_ms, 1u);
    let frame = (u32(globals.params.x) / step) % count;
    out.layer = vertex.layer + frame;
    return out;
}


struct Accumulation {
    @location(0) color: vec4<f32>,
    @location(1) revealage: f32,
};

@fragment
fn fs_main(in: Fragment) -> Accumulation {
    let texel = textureSampleLevel(atlas, atlas_sampler, in.uv, i32(in.layer), 0.0);
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
