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

@group(2) @binding(0) var albedo_tex: texture_2d<f32>;
@group(2) @binding(1) var gbuffer_sampler: sampler;
@group(2) @binding(2) var normal_tex: texture_2d<f32>;
@group(2) @binding(3) var depth_tex: texture_depth_2d;

@group(3) @binding(0) var sky_color_map: texture_2d<f32>;
@group(3) @binding(1) var sky_cloud: texture_2d<f32>;
@group(3) @binding(2) var sky_cloud_color: texture_2d<f32>;
@group(3) @binding(3) var sky_sampler: sampler;

fn apply_fog(color: vec3<f32>, distance: f32) -> vec3<f32> {
    let env = globals.environment;
    let amount = clamp((distance - env.fog_params.x) / max(env.fog_params.y - env.fog_params.x, 0.001), 0.0, 1.0) * env.fog_color.w;
    return mix(color, env.fog_color.rgb, amount);
}

fn sky_color(ray: vec3<f32>) -> vec3<f32> {
    let env = globals.environment;
    if (env.sky_params.z < 0.5) {
        return env.fog_color.rgb;
    }
    let elevation = clamp(asin(clamp(ray.y, -1.0, 1.0)) / 1.5707963, 0.0, 1.0);
    var color = mix(env.sky_horizon.rgb, env.sky_zenith.rgb, elevation);
    if (env.sky_horizon.w > 0.5) {
        let horizontal = ray.xz / max(length(ray.xz), 0.001);
        let sun_horizontal = globals.sun_direction.xz / max(length(globals.sun_direction.xz), 0.001);
        let facing_sun = clamp(dot(horizontal, sun_horizontal) * 0.5 + 0.5, 0.0, 1.0);
        let lookup = vec2<f32>(facing_sun, elevation);
        color = textureSampleLevel(sky_color_map, sky_sampler, lookup, 0.0).rgb;
        if (env.sky_zenith.w > 0.0 && ray.y > 0.0) {
            // A dome follows orientation but never camera translation. Cloud
            // drift uses the authored velocity, expressed in UV units/ms.
            let drift = globals.params.x * 0.001 * env.sky_params.x;
            let cloud_uv = fract(ray.xz / max(ray.y, 0.08) * env.sky_params.y + vec2<f32>(drift, drift * 0.37));
            let cloud = textureSampleLevel(sky_cloud, sky_sampler, cloud_uv, 0.0);
            let tint = textureSampleLevel(sky_cloud_color, sky_sampler, lookup, 0.0).rgb;
            let opacity = cloud.a * env.sky_zenith.w * smoothstep(0.03, 0.2, ray.y);
            color = mix(color, cloud.rgb * tint, opacity);
        }
    }
    // Atmospheric haze meets the same horizon as distant zone geometry.
    let haze = (1.0 - smoothstep(0.0, 0.16, max(ray.y, 0.0))) * env.fog_color.w;
    return mix(color, env.fog_color.rgb, haze);
}

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

    // Clear depth is exactly 1.0; near-one depth still belongs to distant
    // geometry. Reconstructing a world-space ray prevents the sky from being
    // attached to screen pixels when the camera pitches or turns.
    if (depth >= 1.0) {
        let far_h = globals.inv_view_proj * vec4<f32>(in.ndc, 0.99999, 1.0);
        let ray = normalize(far_h.xyz / far_h.w - globals.camera_pos.xyz);
        return vec4<f32>(sky_color(ray), 1.0);
    }

    let albedo = textureSampleLevel(albedo_tex, gbuffer_sampler, uv, 0.0);
    let packed = textureSampleLevel(normal_tex, gbuffer_sampler, uv, 0.0);

    let clip = vec4<f32>(in.ndc, depth, 1.0);
    let world_h = globals.inv_view_proj * clip;
    let world = world_h.xyz / world_h.w;
    let normal = normalize(packed.xyz * 2.0 - 1.0);

    if (albedo.a > 0.5) {
        // Emissive geometry is unaffected by lighting.
        return vec4<f32>(apply_fog(albedo.rgb, distance(world, globals.camera_pos.xyz)), 1.0);
    }

    var accum = globals.ambient.rgb;

    // Directional sun, with a shadow lookup.
    let sun = normalize(globals.sun_direction.xyz);
    let lambert = max(dot(normal, sun), 0.0);
    if (lambert > 0.0) {
        // Offset the receiver along its own normal before projecting into light
        // space. This is what keeps large flat surfaces - platforms, floors,
        // roofs - from shadowing themselves into stripes.
        let biased = world + normal * (globals.params.w * 2.0);
        let light_clip = globals.light_view_proj * vec4<f32>(biased, 1.0);
        let light_ndc = light_clip.xyz / light_clip.w;
        let shadow_uv = vec2<f32>(light_ndc.x * 0.5 + 0.5, 0.5 - light_ndc.y * 0.5);
        var shadow = 1.0;
        let inside = all(shadow_uv >= vec2<f32>(0.0))
            && all(shadow_uv <= vec2<f32>(1.0))
            && light_ndc.z >= 0.0
            && light_ndc.z <= 1.0;
        if (inside) {
            // A 3x3 comparison tap softens the edges and hides residual acne.
            let texel = globals.params.z;
            var lit = 0.0;
            for (var y = -1; y <= 1; y = y + 1) {
                for (var x = -1; x <= 1; x = x + 1) {
                    let offset = vec2<f32>(f32(x), f32(y)) * texel;
                    lit = lit + textureSampleCompare(
                        shadow_map,
                        shadow_sampler,
                        shadow_uv + offset,
                        light_ndc.z - 0.0006,
                    );
                }
            }
            shadow = lit / 9.0;

            // Fade out near the edge of the map so its boundary is not a hard
            // line drawn across the world.
            let edge = min(
                min(shadow_uv.x, 1.0 - shadow_uv.x),
                min(shadow_uv.y, 1.0 - shadow_uv.y),
            );
            shadow = mix(1.0, shadow, smoothstep(0.0, 0.06, edge));
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

    return vec4<f32>(apply_fog(albedo.rgb * accum, distance(world, globals.camera_pos.xyz)), 1.0);
}
