// Directional shadow map pass: depth only, from the light's point of view.

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
    params: vec4<f32>,
};

@group(0) @binding(0) var<uniform> globals: Globals;
@group(1) @binding(0) var shadow_map: texture_depth_2d;
@group(1) @binding(1) var shadow_sampler: sampler_comparison;

struct PointLight {
    position: vec4<f32>,
    color: vec4<f32>,
};

@group(1) @binding(2) var<storage, read> point_lights: array<PointLight>;

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

@vertex
fn vs_main(vertex: Vertex, instance: Instance) -> @builtin(position) vec4<f32> {
    let model = mat4x4<f32>(instance.col0, instance.col1, instance.col2, instance.col3);
    let world = EQ_TO_WORLD * model * vec4<f32>(vertex.position, 1.0);
    return globals.light_view_proj * world;
}

// Depth-only pipeline; never rasterised.
@fragment
fn fs_main() {
}
