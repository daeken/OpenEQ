// Camera-facing original spell sprites. No shadow pass or depth writes.
struct Environment {
    fog_color: vec4<f32>,
    fog_params: vec4<f32>,
    sky_horizon: vec4<f32>,
    sky_zenith: vec4<f32>,
    sky_params: vec4<f32>,
};
struct Globals {
    view_proj: mat4x4<f32>, light_view_proj: mat4x4<f32>, inv_view_proj: mat4x4<f32>,
    camera_pos: vec4<f32>, ambient: vec4<f32>, sun_direction: vec4<f32>, sun_color: vec4<f32>,
    params: vec4<f32>, environment: Environment,
};
@group(0) @binding(0) var<uniform> globals: Globals;
@group(1) @binding(0) var atlas: texture_2d_array<f32>;
@group(1) @binding(1) var atlas_sampler: sampler;
@group(1) @binding(2) var atlas_upper: texture_2d_array<f32>;

struct Particle {
    @location(0) position: vec3<f32>,
    @location(1) rotation: f32,
    @location(2) size: vec2<f32>,
    @location(3) texture: u32,
    @location(4) blend: u32,
    @location(5) color: vec4<f32>,
    @location(6) uv_rect: vec4<f32>,
};
struct Fragment {
    @builtin(position) clip: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) color: vec4<f32>,
    @location(2) world: vec3<f32>,
    @location(3) @interpolate(flat) layer: u32,
    @location(4) @interpolate(flat) blend: u32,
    @location(5) @interpolate(flat) uv_rect: vec4<f32>,
};

@vertex
fn vs_main(particle: Particle, @builtin(vertex_index) vertex: u32) -> Fragment {
    let corners = array<vec2<f32>, 6>(
        vec2<f32>(-0.5,-0.5), vec2<f32>(0.5,-0.5), vec2<f32>(0.5,0.5),
        vec2<f32>(-0.5,-0.5), vec2<f32>(0.5,0.5), vec2<f32>(-0.5,0.5),
    );
    let corner = corners[vertex];
    let local = corner * particle.size;
    let c = cos(particle.rotation);
    let s = sin(particle.rotation);
    let rotated = vec2<f32>(local.x*c-local.y*s, local.x*s+local.y*c);
    let right = normalize(globals.inv_view_proj[0].xyz);
    let up = normalize(globals.inv_view_proj[1].xyz);
    let center = vec3<f32>(particle.position.x, particle.position.z, -particle.position.y);
    let world = center + right*rotated.x + up*rotated.y;
    let uv = vec2<f32>(corner.x+0.5, 0.5-corner.y);
    var out: Fragment;
    out.clip = globals.view_proj * vec4<f32>(world,1.0);
    out.uv = mix(particle.uv_rect.xy, particle.uv_rect.zw, uv);
    out.color = particle.color;
    out.world = world;
    out.layer = particle.texture;
    out.blend = particle.blend;
    out.uv_rect = particle.uv_rect;
    return out;
}

@fragment
fn fs_main(in: Fragment) -> @location(0) vec4<f32> {
    // Clamp within this flipbook cell, not merely within the entire texture;
    // adjacent animation frames must never bleed across billboard edges.
    let half_texel = min(vec2<f32>(0.5)/vec2<f32>(textureDimensions(atlas,0)),
        (in.uv_rect.zw-in.uv_rect.xy)*0.5);
    let uv = clamp(in.uv,in.uv_rect.xy+half_texel,in.uv_rect.zw-half_texel);
    // Two arrays preserve all 512 slots on adapters with 256 array layers.
    // There are no mipmaps; explicit LOD also permits nonuniform slot choice.
    var sample: vec4<f32>;
    if (in.layer < 256u) {
        sample = textureSampleLevel(atlas,atlas_sampler,uv,i32(in.layer),0.0);
    } else {
        sample = textureSampleLevel(atlas_upper,atlas_sampler,uv,i32(in.layer-256u),0.0);
    }
    let alpha = sample.a * in.color.a;
    if (alpha <= 0.0) { discard; }
    var color = sample.rgb * in.color.rgb;
    let env = globals.environment;
    let amount = clamp((distance(in.world,globals.camera_pos.xyz)-env.fog_params.x)
        /max(env.fog_params.y-env.fog_params.x,0.001),0.0,1.0)*env.fog_color.w;
    if (in.blend == 1u) {
        // Additive light vanishes into fog instead of adding the fog color a
        // second time and turning the entire horizon bright.
        color *= 1.0-amount;
    } else {
        color = mix(color,env.fog_color.rgb,amount);
    }
    return vec4<f32>(color*alpha,alpha);
}
