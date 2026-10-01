// Deferred lighting pass: a fullscreen triangle that reconstructs world
// positions from depth and shades the G-buffer.
//
// The falloff for zone lights matches the original engine's deferred pathway:
// `intensity * pow(1 - distance / radius, 3)`.

@group(2) @binding(0) var albedo_tex: texture_2d<f32>;
@group(2) @binding(1) var gbuffer_sampler: sampler;
@group(2) @binding(2) var normal_tex: texture_2d<f32>;
@group(2) @binding(3) var depth_tex: texture_depth_2d;

@group(3) @binding(0) var sky_color_map: texture_2d<f32>;
@group(3) @binding(1) var sky_cloud: texture_2d<f32>;
@group(3) @binding(2) var sky_cloud_color: texture_2d<f32>;
@group(3) @binding(3) var sky_sampler: sampler;

fn sky_lookup_uv(point: vec2<f32>, dimensions: vec2<u32>) -> vec2<f32> {
    // Color tables address discrete vertex colors: include the endpoint texel
    // centers without sampling across the texture border. Cloud sprites use
    // the repeat sampler directly so their bilinear seam also wraps correctly.
    let size = vec2<f32>(dimensions);
    return (vec2<f32>(0.5) + clamp(point, vec2<f32>(0.0), vec2<f32>(1.0)) * (size - 1.0)) / size;
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
        color = textureSampleLevel(sky_color_map, sky_sampler, sky_lookup_uv(lookup, textureDimensions(sky_color_map)), 0.0).rgb;
        if (env.sky_zenith.w > 0.0 && ray.y > 0.0) {
            // A dome follows orientation but never camera translation. Cloud
            // drift uses the authored velocity, expressed in UV units/ms.
            let drift = globals.params.x * 0.001 * env.sky_params.x;
            let cloud_uv = fract(ray.xz / max(ray.y, 0.08) * env.sky_params.y + vec2<f32>(drift, drift * 0.37));
            let cloud = textureSampleLevel(sky_cloud, sky_sampler, cloud_uv, 0.0);
            let tint = textureSampleLevel(sky_cloud_color, sky_sampler, sky_lookup_uv(lookup, textureDimensions(sky_cloud_color)), 0.0).rgb;
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

    let flags = u32(round(packed.w * 255.0));
    let color_factor = select(1.0, 2.0, (flags & 64u) != 0u);
    return vec4<f32>(shade_surface(world, normal, albedo.rgb * color_factor, albedo.a > 0.5), 1.0);
}
