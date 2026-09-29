// Terrain's shared detail maps contain encoded color, matching the compatibility
// painter. Blend in that space, then decode once for the linear G-buffer.
struct TerrainHeader { words: vec4<u32> };
struct TerrainLayer {
    words: vec4<u32>,
    height: vec4<f32>,
    slope: vec4<f32>,
};
@group(2) @binding(4) var terrain_details: texture_2d_array<f32>;
@group(2) @binding(5) var<storage, read> terrain_headers: array<TerrainHeader>;
@group(2) @binding(6) var<storage, read> terrain_layers: array<TerrainLayer>;
@group(2) @binding(7) var<storage, read> terrain_masks: array<u32>;
const FLAG_TERRAIN: u32 = 32u;

fn mask_texel(offset: u32, size: u32, point: vec2<u32>) -> f32 {
    let index = point.y * size + point.x;
    let word = terrain_masks[offset + index / 4u];
    return f32((word >> ((index % 4u) * 8u)) & 255u) / 255.0;
}

fn mask_level(start: u32, original_size: u32, level: u32, uv: vec2<f32>) -> f32 {
    var offset = start;
    var size = original_size;
    for (var i = 0u; i < level; i += 1u) {
        offset += (size * size + 3u) / 4u;
        size = max(1u, size / 2u);
    }
    let coordinate = clamp(uv * f32(size) - vec2<f32>(0.5), vec2<f32>(0.0), vec2<f32>(f32(size - 1u)));
    let a = vec2<u32>(floor(coordinate));
    let b = min(a + vec2<u32>(1u), vec2<u32>(size - 1u));
    let phase = fract(coordinate);
    return mix(
        mix(mask_texel(offset, size, a), mask_texel(offset, size, vec2<u32>(b.x, a.y)), phase.x),
        mix(mask_texel(offset, size, vec2<u32>(a.x, b.y)), mask_texel(offset, size, b), phase.x),
        phase.y,
    );
}

fn terrain_opacity(layer: TerrainLayer, uv: vec2<f32>, dx: vec2<f32>, dy: vec2<f32>) -> f32 {
    let size = layer.words.z;
    if (size == 0u) { return 1.0; }
    let footprint = max(length(dx), length(dy)) * f32(size);
    let lod = clamp(log2(max(footprint, 1.0)), 0.0, log2(f32(size)));
    let low = u32(floor(lod));
    let high = u32(ceil(lod));
    return mix(mask_level(layer.words.y, size, low, uv), mask_level(layer.words.y, size, high, uv), fract(lod));
}

fn terrain_range(value: f32, lo: f32, hi: f32, tolerance: f32) -> f32 {
    if (tolerance <= 0.0) { return select(0.0, 1.0, value >= lo && value <= hi); }
    return clamp((value - lo + tolerance) / tolerance, 0.0, 1.0)
        * clamp((hi + tolerance - value) / tolerance, 0.0, 1.0);
}

fn terrain_albedo(material: u32, uv: vec2<f32>, height: f32, normal: vec3<f32>, dx: vec2<f32>, dy: vec2<f32>) -> vec3<f32> {
    let header = terrain_headers[material].words;
    var color = vec4<f32>(1.0, 0.0, 1.0, 1.0);
    // A resolved first paint layer replaces the base unconditionally.
    if (header.y == 0u && header.z != 0xffffffffu) {
        color = textureSampleGrad(terrain_details, atlas_sampler, uv * 8.0, i32(header.z), dx * 8.0, dy * 8.0);
    }
    let slope = degrees(acos(clamp(normalize(normal).y, -1.0, 1.0)));
    var ecosystem = color;
    var opacity = 1.0;
    for (var i = 0u; i < header.y; i += 1u) {
        let layer = terrain_layers[header.x + i];
        let repeat = layer.height.x;
        var weight = 1.0;
        if ((layer.words.w & 1u) != 0u) {
            opacity = terrain_opacity(layer, uv, dx, dy);
        } else {
            weight = terrain_range(height, layer.height.y, layer.height.z, layer.height.w)
                * terrain_range(slope, layer.slope.x, layer.slope.y, layer.slope.z);
        }
        // Explicit gradients remain valid in this branch. Fully masked or
        // out-of-range layers need no detail fetch, common in real ecosystems.
        if (opacity > 0.0 && weight > 0.0) {
            let sample = textureSampleGrad(terrain_details, atlas_sampler, uv * repeat, i32(layer.words.x), dx * repeat, dy * repeat);
            if ((layer.words.w & 1u) != 0u) { ecosystem = sample; }
            else { ecosystem = mix(ecosystem, sample, weight); }
        }
        if ((layer.words.w & 2u) != 0u) {
            color = mix(color, ecosystem, opacity);
        }
    }
    let rgb = clamp(color.rgb, vec3<f32>(0.0), vec3<f32>(1.0));
    return select(pow((rgb + vec3<f32>(0.055)) / 1.055, vec3<f32>(2.4)), rgb / 12.92, rgb <= vec3<f32>(0.04045));
}
