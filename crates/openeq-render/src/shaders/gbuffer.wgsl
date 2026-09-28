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
    // x = elapsed milliseconds, y = point light count
    params: vec4<f32>,
    environment: Environment,
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

struct WaterParams {
    color1: vec4<f32>,
    color2: vec4<f32>,
    reflection_color: vec4<f32>,
    params: vec4<f32>,
    layers: vec4<u32>,
};
@group(2) @binding(2) var<storage, read> water_materials: array<WaterParams>;
@group(2) @binding(3) var linear_atlas: texture_2d_array<f32>;

// Material flags shared with the CPU side.
const FLAG_ALPHA_MASK: u32 = 1u;
const FLAG_EMISSIVE: u32 = 4u;
const FLAG_WATER: u32 = 8u;

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

struct Targets {
    @location(0) albedo: vec4<f32>,
    @location(1) normal: vec4<f32>,
};

// DDS cubemap faces occupy six consecutive atlas layers in +X,-X,+Y,-Y,+Z,-Z order.
fn cube_uv(direction: vec3<f32>) -> vec3<f32> {
    let a = abs(direction);
    var uv: vec2<f32>;
    var face: f32;
    if (a.x >= a.y && a.x >= a.z) {
        uv = vec2<f32>(-direction.z * sign(direction.x), -direction.y) / a.x;
        face = select(1.0, 0.0, direction.x >= 0.0);
    } else if (a.y >= a.z) {
        uv = vec2<f32>(direction.x, direction.z * sign(direction.y)) / a.y;
        face = select(3.0, 2.0, direction.y >= 0.0);
    } else {
        uv = vec2<f32>(direction.x * sign(direction.z), -direction.y) / a.z;
        face = select(5.0, 4.0, direction.z >= 0.0);
    }
    return vec3<f32>(uv * 0.5 + 0.5, face);
}

@fragment
fn fs_main(in: Fragment) -> Targets {
    // Derivatives must be taken before branching on the material. Filtering
    // the ripples at distance prevents sparkling along the water's horizon.
    let water_dx = dpdx(in.world.xz / 80.0);
    let water_dy = dpdy(in.world.xz / 80.0);
    if ((in.flags & FLAG_WATER) != 0u) {
        let water = water_materials[in.material];
        let seconds = globals.params.x * 0.001;
        // Two crossing ripples keep large water planes from visibly sliding as
        // one sheet. World coordinates also keep adjacent pieces continuous.
        let uv = in.world.xz / 80.0;
        let uv1 = uv + seconds * vec2<f32>(0.012, 0.007);
        let uv2 = uv * 1.37 + seconds * vec2<f32>(-0.008, 0.011);
        var ripple = vec2<f32>(0.0);
        if (water.layers.x != 0xffffffffu) {
            let a = textureSampleGrad(linear_atlas, atlas_sampler, uv1, i32(water.layers.x), water_dx, water_dy);
            let b = textureSampleGrad(linear_atlas, atlas_sampler, uv2, i32(water.layers.x), water_dx * 1.37, water_dy * 1.37);
            ripple = (a.xy + b.xy - vec2<f32>(1.0)) * 0.35;
        }
        let normal = normalize(in.normal + vec3<f32>(ripple.x, 0.0, ripple.y));
        let view = normalize(globals.camera_pos.xyz - in.world);
        let fresnel = water.params.x + (1.0 - water.params.x)
            * pow(1.0 - max(dot(normal, view), 0.0), water.params.y);
        let blend = textureSampleGrad(atlas, atlas_sampler, uv1, i32(in.layer), water_dx, water_dy).r;
        let base = mix(water.color1.rgb, water.color2.rgb, blend);
        let reflected = reflect(-view, normal);
        var environment = mix(vec3<f32>(0.42, 0.50, 0.62), vec3<f32>(0.09, 0.16, 0.32),
            clamp(reflected.y, 0.0, 1.0));
        if (water.layers.y != 0xffffffffu) {
            let cube = cube_uv(reflected);
            // Keep filtering within this face; the diffuse sampler repeats.
            let inset = 0.5 / f32(textureDimensions(atlas).x);
            let cube_texcoord = clamp(cube.xy, vec2<f32>(inset), vec2<f32>(1.0 - inset));
            environment = textureSampleLevel(atlas, atlas_sampler, cube_texcoord,
                i32(water.layers.y) + i32(cube.z), 0.0).rgb;
        }
        var out: Targets;
        out.albedo = vec4<f32>(mix(base, environment * water.reflection_color.rgb,
            clamp(fresnel * water.params.z, 0.0, 1.0)), 0.0);
        out.normal = vec4<f32>(normal * 0.5 + 0.5, f32(in.flags) / 255.0);
        return out;
    }
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
