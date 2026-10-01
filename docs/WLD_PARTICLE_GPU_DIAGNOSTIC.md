# Standalone WLD particle GPU diagnostic

2026-10-01. `openeq_render::wld_particle_gpu` renders caller-supplied captured
XYZRHW quads to an independent, readable `Rgba8Unorm` image. It is an opt-in
experiment for the fixed state described in
[the native GPU contract](WLD_PARTICLE_GPU_CONTRACT.md), not an in-game particle
pass or a comparison against original-client framebuffer pixels.

The renderer has its own headless device. No scene loader, camera, WLD sampler,
spell renderer, light extractor, or world renderer calls it. It does not select,
spawn, update, project, sort, or distance-cull particles. The caller supplies the
already selected projected vertices and decoded original-sized textures.

## Explicit assumptions and fixed state

| Item | Diagnostic behavior |
| --- | --- |
| Source and destination | `Rgba8Unorm`; no sRGB read, write or destination conversion |
| Texture upload | Exact input width/height and RGBA bytes; one mip; no atlas or resizing |
| Sampler | Repeat U/V, linear min/mag, nearest mip; explicit LOD zero |
| Modulation | Texture RGBA multiplied by unpacked vertex RGBA |
| Alpha test | Discard when modulated alpha is below nominal `1/255` |
| Blend RGB | `source.rgb * source.a + destination.rgb` |
| Blend alpha | `source.a * source.a + destination.a` |
| Depth | `Depth32Float`, LESSEQUAL, no particle depth writes; caller supplies uniform initial depth |
| Rasterization | WebGPU triangle list, no culling, single sample, no fog |
| Submission | Input order, native six-index order per quad: `0,1,3,1,2,3` |

The sRGB, ADD operation and nonseparate alpha behavior explicitly assume the
documented native defaults. The native particle draw inherits those states; the
complete original host lifecycle has not been captured. The native image path
requests generated mipmaps, so this level-zero experiment does not establish
native minification behavior. Nor does it establish original fixed-function
rounding, alpha-test precision or arbitrary texture-load transformations. The
[native texture-load witness](WLD_PARTICLE_TEXTURE_LOAD.md) now establishes
unchanged level-zero BC1 bytes and row order for these two original images on
the controlled full-quality, BC1-supported path.

`ProjectedVertex` retains the native 32-byte layout: four XYZRHW floats, packed
`0xAARRGGBB` diffuse, zero specular, and two UV floats. UVs pass through unchanged;
the captured WLD stream already reverses V. The caller supplies corners in
top-left, top-right, bottom-right, bottom-left order. The captured witnesses have
constant diffuse, depth and RHW across each quad. More general vertex variation
uses WebGPU's interpolation and clipping; no cross-API equivalence is asserted.
The native shared index-buffer builder has been executed and its entire output
checked: it uses the top-right to bottom-left diagonal. See
[the index and texture-load witness](WLD_PARTICLE_TEXTURE_LOAD.md). The diagnostic
expands those same indices to six vertices.

Screen coordinates refer to the full target, with Y increasing downward. The
caller must choose a viewport inside that target and a `PixelCenterConvention`:

- `AsCaptured` uses the captured numeric screen positions directly.
- `ShiftByHalfPixel` adds `+0.5` to screen X and Y before viewport conversion.

Neither choice certifies D3D9 edge coverage. There is intentionally no default
choice. Clip W is `1 / RHW`; clip XY and Z are multiplied by W so the supplied
screen position and depth survive perspective division. Finite depths outside
`[0,1]` are passed to GPU clipping, not silently clamped.

## API example

```rust
use openeq_assets::texture::Texture;
use openeq_render::wld_particle_gpu::{
    DiagnosticFrame, DiagnosticRenderer, PixelCenterConvention,
    ProjectedQuad, ProjectedVertex, Viewport,
};

let sources = [Texture {
    name: "white diagnostic source".into(),
    width: 1,
    height: 1,
    rgba: vec![255; 4],
}];
let quads = [ProjectedQuad {
    texture: 0,
    vertices: [
        ([10., 10.], [0., 1.]),
        ([20., 10.], [1., 1.]),
        ([20., 20.], [1., 0.]),
        ([10., 20.], [0., 0.]),
    ].map(|(xy, uv)| ProjectedVertex {
        xyzrhw: [xy[0], xy[1], 0.5, 1.],
        diffuse: 0xff646464,
        specular: 0,
        uv,
    }),
}];
let frame = DiagnosticFrame {
    target_size: [32, 32],
    viewport: Viewport { x: 0, y: 0, width: 32, height: 32 },
    pixel_centers: PixelCenterConvention::AsCaptured,
    clear_rgba: [0.; 4],
    clear_depth: 1.,
    textures: &sources,
    quads: &quads,
};
frame.validate()?; // also called by render before per-frame GPU work
let renderer = DiagnosticRenderer::new_headless()?;
let image = renderer.render(&frame)?;
assert_eq!(image.uploaded_texture_sizes, [[1, 1]]);
// image.rgba contains stored target bytes, with no display transfer conversion.
# Ok::<(), anyhow::Error>(())
```

`render` blocks through submission and readback and returns tightly packed
RGBA bytes. PNG export or presentation in the existing sRGB scene is a separate
step with its own display policy; viewing an exported PNG is not proof of native
framebuffer parity.

## Limits and rejection

All frame inputs are validated before per-frame GPU allocation or submission:

- Target and source dimensions are nonzero and at most 2048 per edge.
- At most 32 textures, 16 MiB total source RGBA, and 1024 quads are accepted.
- The viewport must fit the target, texture byte counts must match dimensions,
  and every quad's texture index must exist.
- Clear color/depth must be finite and within `[0,1]`.
- XYZRHW and UV must be finite. RHW and its reciprocal must be positive normal
  floats; subnormal divisors/reciprocals could be flushed differently by a GPU.
  Clip-coordinate overflow and nonzero specular are rejected.

These caps bound source uploads, expanded vertices, color/depth targets and
aligned readback allocations. Empty input is allowed and returns the clear
image. The diagnostic does not accept a preexisting scene depth buffer.

## Frozen test scope

Run the CPU rejection test and all seven opt-in GPU checks with:

```sh
cargo test -p openeq-render --test wld_particle_gpu -- --include-ignored
```

The suite fails if it cannot create a GPU device; it does not silently skip a
requested GPU check. The original-texture case additionally requires the local
client assets via the existing asset-directory lookup. The checked cases are:

1. Invalid frame dimensions/counts, malformed images, bad floats, reciprocal
   extremes, clear values, texture references and unsupported specular.
2. Byte-space texture/diffuse modulation over a nonzero background, repeated
   additive draws and the independent alpha-square equation.
3. Nominal alpha-test equality at `1/255`, with lower products rejected.
4. Depth equality accepted, greater depth rejected, both out-of-range depth
   signs clipped, and a nearer quad not preventing a later farther draw.
5. Exact asymmetric 3-by-2 texel output with captured V reversal, plus
   wrap-linear seam blending and UVs outside `[0,1]`.
6. Native top-right to bottom-left diagonal, with deliberately varied corner
   colors that distinguish it from the other diagonal. This synthetic case
   checks topology, not native interpolation equivalence.
7. Offset viewport, explicit half-pixel translation, nonunit RHW and empty draw.
8. Original `CSMOKE.DDS` (16-by-32) and `GENG00.DDS` (64-by-64) from
   `poknowledge_obj.s3d`, using all four previously captured native quads.

The final case checks GPU upload dimensions, nonzero pixels and the expected
projected region. Its temporary PNGs are
`/tmp/openeq-wld-particle-gpu-{csmoke,l301,l308,l500}.png`. They are diagnostic
artifacts, not golden images from the original client. The independent
`wld_particle_texture_decode` suite compares CPU DDS decoding with hardware BC1
at source resolution. The separate native D3DX witness closes the tested
full-quality BC1 level-zero load/copy boundary; other quality settings, format
fallback and the assembled texture's driver-generated mips remain open.

All eight tests passed on the available headless GPU on 2026-10-01. No automatic
scene integration, source animation, mip generation, arbitrary native blend
alias, camera conversion, fog, effect lighting or original-frame pixel parity is
included in this frozen scope.
