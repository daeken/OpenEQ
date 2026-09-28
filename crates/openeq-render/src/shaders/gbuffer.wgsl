// Deferred geometry pass: writes albedo and a view-independent normal.
//
// EverQuest assets live in a Z-up, north-positive space. Rather than rewriting
// vertex data on load, the conversion happens here in `EQ_TO_WORLD`, which maps
// (x, y, z) to (x, z, -y) so that up becomes +Y and north becomes -Z.

const EQ_TO_WORLD: mat4x4<f32> = mat4x4<f32>(
    vec4<f32>(1.0, 0.0, 0.0, 0.0),
    vec4<f32>(0.0, 0.0, -1.0, 0.0),
    vec4<f32>(0.0, 1.0, 0.0, 0.0),
    vec4<f32>(0.0, 0.0, 0.0, 1.0),
);

struct Globals {
    view_proj: mat4x4<f32>,
    light_view_proj: mat4x4<f32>,
    inv_view_proj: mat4x4<f32>,
    camera_pos: vec4<f32>,
    ambient: vec4<f32>,
    sun_direction: vec4<f32>,
    sun_color: vec4<f32>,
    // x = elapsed milliseconds, y = point light count
    params: vec4<f32>,
};

struct PointLight {
    // xyz in EQ space, w = radius
    position: vec4<f32>,
    // rgb, w = attenuation
    color: vec4<f32>,
};

@group(0) @binding(0) var<uniform> globals: Globals;

@group(1) @binding(0) var shadow_map: texture_depth_2d;
@group(1) @binding(1) var shadow_sampler: sampler_comparison;
@group(1) @binding(2) var<storage, read> point_lights: array<PointLight>;

@group(2) @binding(0) var atlas: texture_2d_array<f32>;
@group(2) @binding(1) var atlas_sampler: sampler;

// Material flags shared with the CPU side.
const FLAG_ALPHA_MASK: u32 = 1u;
const FLAG_EMISSIVE: u32 = 4u;

struct Vertex {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
    @location(3) layer: u32,
    @location(4) frame_offset: u32,
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

    // Animated textures are stored as consecutive layers; pick the frame.
    let count = max(vertex.frame_count, 1u);
    let step = max(vertex.frame_ms, 1u);
    let frame = (u32(globals.params.x) / step) % count;
    out.layer = vertex.layer + frame;
    return out;
}

struct Targets {
    @location(0) albedo: vec4<f32>,
    @location(1) normal: vec4<f32>,
};

@fragment
fn fs_main(in: Fragment) -> Targets {
    let texel = textureSampleLevel(atlas, atlas_sampler, in.uv, i32(in.layer), 0.0);
    if ((in.flags & FLAG_ALPHA_MASK) != 0u && texel.a < 0.5) {
        discard;
    }

    var out: Targets;
    // Emissive surfaces ignore lighting; flag them for the lighting pass.
    let emissive = select(0.0, 1.0, (in.flags & FLAG_EMISSIVE) != 0u);
    out.albedo = vec4<f32>(texel.rgb, emissive);
    out.normal = vec4<f32>(normalize(in.normal) * 0.5 + 0.5, f32(in.flags) / 255.0);
    return out;
}
