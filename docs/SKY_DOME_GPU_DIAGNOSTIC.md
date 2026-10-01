# Native sky dome GPU diagnostic

The opt-in CPU dome now has a test-only GPU diagnostic that draws its exact
source topology and packed colors. It uses an explicit unlit, untextured
RGBA8 UNORM target, no fog, no alpha test or blending, no culling, and writable
LESSEQUAL depth. It does not alter the live sky renderer.

These choices reproduce the bounded default-state drawing contract in
`SKY_DOME_DRAW_STATE.md` and `NATIVE_COLOR_SPACE_STATE.md`. They do not infer
whole-frame inherited state. In particular, the diagnostic explicitly chooses
all color-write channels and a clear depth of one. It does not apply the
original display gamma ramp or claim that an exported PNG reproduces its
appearance on the original display.

## Geometry and shader contract

`NativeSkyDome::build` supplies 962 vertices and 5,583 indices, including the
native ring source-color addressing, seam, positive pole and unusual bottom
cap. The CPU builder's original-instruction topology/color witnesses remain
in `SKY_DOME_GEOMETRY.md`; this diagnostic does not regenerate an independent
sphere or replace its authored colors.

The GPU receives the native 16-byte XYZ/AARRGGBB records and original u16
triangle list. A small shader extracts each packed color byte and interpolates
unlit diffuse RGBA across the triangles. It performs no texture lookup,
palette filtering, linearization or output sRGB encoding.

Camera matrices are explicit diagnostic inputs built with a perspective
projection and chosen look direction in the dome's own coordinate system.
The test does **not** port the host celestial transform or infer how the
native sky should orient at a particular game time. Original D3DX singular
behavior remains documented separately in `SKY_DOME_TRANSFORM.md`.

## GPU checks

Three tests in `crates/openeq-render/tests/native_sky_dome.rs` pass:

- A constant palette produces exact `[51,102,153,0]` bytes in every pixel
  across all six axis views. This catches unintended sRGB encoding, channel
  swapping, alpha blending/testing, missing faces and unwanted lighting.
- Modifying the valid near-pole source word 1 changes more than 100 rendered
  components in a close pole view. Every unused source-table word can then
  be replaced with magenta without changing those pixels. Thus this mesh
  preserves a valid ring color while excluding auxiliary palette entries.
  The live texture-based sky's pole-row suppression remains an approximation.
- Four original PoK tables at fractions 0, 0.25, 0.5, 0.75 render distinct images
  with no uncovered magenta-clear pixels in a fixed diagnostic view. Captures
  are `/tmp/openeq-native-sky-gpu/pok-{night,dawn,day,dusk}.png`.

These establish GPU behavior for the supplied geometry, palette and state;
they are not comparisons against original-client framebuffers. Live dome
integration, celestial transform policy, weather lifecycle, clouds, inherited
frame state and display/color transfer remain separate work.

Run with original assets:

```sh
CARGO_INCREMENTAL=0 EQ_DIR=/Users/daeken/EverQuest cargo test \
  -p openeq-render --test native_sky_dome -- --include-ignored
```

Verification log: `/tmp/openeq-native-sky-gpu.log`. No original asset bytes are
committed, and no audio or live character state is involved.
