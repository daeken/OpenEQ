// Shared by opaque deferred lighting and transparent forward lighting.
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

fn apply_fog(color: vec3<f32>, distance: f32) -> vec3<f32> {
    let env = globals.environment;
    let amount = clamp((distance - env.fog_params.x) / max(env.fog_params.y - env.fog_params.x, 0.001), 0.0, 1.0) * env.fog_color.w;
    return mix(color, env.fog_color.rgb, amount);
}

fn shade_surface(world: vec3<f32>, normal: vec3<f32>, albedo: vec3<f32>, emissive: bool) -> vec3<f32> {
    if (emissive) {
        // Emissive geometry is unaffected by lighting.
        return apply_fog(albedo, distance(world, globals.camera_pos.xyz));
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

    return apply_fog(albedo * accum, distance(world, globals.camera_pos.xyz));
}
