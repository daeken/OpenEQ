// Directional shadow map pass: depth only, from the light's point of view.
//
// Alpha-masked materials are cut out here as well as in the G-buffer. EverQuest
// foliage is built from cross-plane quads with a cut-out texture, so without
// that test a tree canopy casts a solid slab instead of dappled shade.

const EQ_TO_WORLD: mat4x4<f32> = mat4x4<f32>(
    vec4<f32>(1.0, 0.0, 0.0, 0.0),
    vec4<f32>(0.0, 0.0, -1.0, 0.0),
    vec4<f32>(0.0, 1.0, 0.0, 0.0),
    vec4<f32>(0.0, 0.0, 0.0, 1.0),
);

// Must match the flags the CPU packs into each vertex.
const FLAG_ALPHA_MASK: u32 = 1u;
const FLAG_TRANSPARENT: u32 = 2u;
const FLAG_WATER: u32 = 8u;

struct Environment {
    fog_color: vec4<f32>, // linear RGB, w = enabled
    fog_params: vec4<f32>, // start, end, authored density, reserved
    sky_horizon: vec4<f32>, // fallback linear RGB, w = authored textures
    sky_zenith: vec4<f32>, // fallback linear RGB, w = cloud opacity
    sky_params: vec4<f32>, // cloud velocity, cloud scale, sky enabled, reserved
};

struct Globals {
    view_proj: mat4x4<f32>,
    light_view_proj: mat4x4<f32>,
    inv_view_proj: mat4x4<f32>,
    camera_pos: vec4<f32>,
    ambient: vec4<f32>,
    sun_direction: vec4<f32>,
    sun_color: vec4<f32>,
    // x = elapsed milliseconds, y = point light count, z = shadow texel size in
    // UV space, w = shadow texel size in world units.
    params: vec4<f32>,
    environment: Environment,
};

struct PointLight {
    position: vec4<f32>,
    color: vec4<f32>,
};

@group(0) @binding(0) var<uniform> globals: Globals;
@group(1) @binding(0) var shadow_map: texture_depth_2d;
@group(1) @binding(1) var shadow_sampler: sampler_comparison;
@group(1) @binding(2) var<storage, read> point_lights: array<PointLight>;
@group(2) @binding(0) var atlas: texture_2d_array<f32>;
@group(2) @binding(1) var atlas_sampler: sampler;

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
    @location(1) @interpolate(flat) layer: u32,
    @location(2) @interpolate(flat) flags: u32,
};

@vertex
fn vs_main(vertex: Vertex, instance: Instance) -> Fragment {
    let model = mat4x4<f32>(instance.col0, instance.col1, instance.col2, instance.col3);
    let world = EQ_TO_WORLD * model * vec4<f32>(vertex.position, 1.0);

    var out: Fragment;
    out.clip = globals.light_view_proj * world;
    out.uv = vertex.uv;
    out.flags = vertex.flags;

    let count = max(vertex.frame_count, 1u);
    let step = max(vertex.frame_ms, 1u);
    out.layer = vertex.layer + (u32(globals.params.x) / step) % count;
    return out;
}

@fragment
fn fs_main(in: Fragment) {
    // Partial alpha cannot cast an opaque shadow. Hair's fully opaque core
    // still casts, while tattoo decals never add solid shadow speckles.
    if ((in.flags & FLAG_TRANSPARENT) != 0u && (in.flags & FLAG_WATER) == 0u) {
        let texel = textureSampleLevel(atlas, atlas_sampler, in.uv, i32(in.layer), 0.0);
        if (texel.a < 1.0) { discard; }
    }
    if ((in.flags & FLAG_ALPHA_MASK) != 0u) {
        let texel = textureSampleLevel(atlas, atlas_sampler, in.uv, i32(in.layer), 0.0);
        if (texel.a < 0.5) {
            discard;
        }
    }
}
