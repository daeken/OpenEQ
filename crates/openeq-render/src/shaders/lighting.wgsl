// Deferred lighting pass: a fullscreen triangle that reconstructs world
// positions from depth and shades the G-buffer.
//
// The falloff for zone lights matches the original engine's deferred pathway:
// `intensity * pow(1 - distance / radius, 3)`.

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

struct PointLight {
    position: vec4<f32>,
    color: vec4<f32>,
};

@group(0) @binding(0) var<uniform> globals: Globals;

@group(1) @binding(0) var shadow_map: texture_depth_2d;
@group(1) @binding(1) var shadow_sampler: sampler_comparison;
@group(1) @binding(2) var<storage, read> point_lights: array<PointLight>;

@group(2) @binding(0) var albedo_tex: texture_2d<f32>;
@group(2) @binding(1) var gbuffer_sampler: sampler;
@group(2) @binding(2) var normal_tex: texture_2d<f32>;
@group(2) @binding(3) var depth_tex: texture_depth_2d;

struct Fragment {
    @builtin(position) clip: vec4<f32>,
    @location(0) ndc: vec2<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) index: u32) -> Fragment {
    var corners = array<vec2<f32>, 3>(
        vec2<f32>(-1.0, -1.0),
        vec2<f32>(3.0, -1.0),
        vec2<f32>(-1.0, 3.0),
    );
    let corner = corners[index];
    var out: Fragment;
    out.clip = vec4<f32>(corner, 0.0, 1.0);
    out.ndc = corner;
    return out;
}

@fragment
fn fs_main(in: Fragment) -> @location(0) vec4<f32> {
    let uv = vec2<f32>(in.ndc.x * 0.5 + 0.5, 0.5 - in.ndc.y * 0.5);
    // Load rather than sample: depth textures are not filterable, and this pass
    // is one-to-one with the G-buffer anyway.
    let depth = textureLoad(depth_tex, vec2<i32>(in.clip.xy), 0);

    // Nothing was drawn here, so draw a simple sky gradient instead.
    if (depth >= 0.9999) {
        let t = clamp(in.ndc.y * 0.5 + 0.5, 0.0, 1.0);
        let horizon = vec3<f32>(0.42, 0.50, 0.62);
        let zenith = vec3<f32>(0.09, 0.16, 0.32);
        return vec4<f32>(mix(horizon, zenith, t), 1.0);
    }

    let albedo = textureSampleLevel(albedo_tex, gbuffer_sampler, uv, 0.0);
    let packed = textureSampleLevel(normal_tex, gbuffer_sampler, uv, 0.0);

    let clip = vec4<f32>(in.ndc, depth, 1.0);
    let world_h = globals.inv_view_proj * clip;
    let world = world_h.xyz / world_h.w;
    let normal = normalize(packed.xyz * 2.0 - 1.0);

    if (albedo.a > 0.5) {
        // Emissive geometry is unaffected by lighting.
        return vec4<f32>(albedo.rgb, 1.0);
    }

    var accum = globals.ambient.rgb;

    // Directional sun, with a shadow lookup.
    let sun = normalize(globals.sun_direction.xyz);
    let lambert = max(dot(normal, sun), 0.0);
    if (lambert > 0.0) {
        let light_clip = globals.light_view_proj * vec4<f32>(world, 1.0);
        let light_ndc = light_clip.xyz / light_clip.w;
        let shadow_uv = vec2<f32>(light_ndc.x * 0.5 + 0.5, 0.5 - light_ndc.y * 0.5);
        var shadow = 1.0;
        if (all(shadow_uv >= vec2<f32>(0.0)) && all(shadow_uv <= vec2<f32>(1.0)) && light_ndc.z <= 1.0) {
            shadow = textureSampleCompare(shadow_map, shadow_sampler, shadow_uv, light_ndc.z - 0.0015);
        }
        accum += globals.sun_color.rgb * lambert * shadow;
    }

    // Zone lights.
    let count = u32(globals.params.y);
    for (var i = 0u; i < count; i = i + 1u) {
        let light = point_lights[i];
        let light_world = (EQ_TO_WORLD * vec4<f32>(light.position.xyz, 1.0)).xyz;
        let to_light = light_world - world;
        let distance = length(to_light);
        let radius = max(light.position.w, 0.001);
        if (distance >= radius) {
            continue;
        }
        let direction = to_light / max(distance, 0.0001);
        let intensity = clamp(dot(normal, direction), 0.0, 1.0);
        let falloff = pow(1.0 - distance / radius, 3.0);
        accum += light.color.rgb * falloff * intensity;
    }

    return vec4<f32>(albedo.rgb * accum, 1.0);
}
