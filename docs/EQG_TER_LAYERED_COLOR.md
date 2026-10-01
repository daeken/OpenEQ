# TER second-color rendering

October 1, 2026. Exact `Opaque_MaxCB1_2UV.fx` terrain now samples its authored
second color texture with independent secondary coordinates and applies the
recovered `2 * diffuse * second` color operation. This is a bounded rendering
improvement, not complete native shader fidelity. Existing sRGB texture decode,
linear lighting, geometric normals, fog, atlas resizing and mip construction
remain OpenEQ approximations. Native packed tangent/normal behavior, point-light
membership, encoded-color arithmetic and display gamma remain separate.

## Admission and source preservation

The retained mesh channel must originate from TER version 1–3 with a complete
secondary stream and exact CB1_2UV family. All three authored diffuse, normal
and second-color properties must be nonempty text other than `none`. No missing
property receives an invented default; native shared-effect state is not replayed.
The current parser supplies actual secondary channels for versions 2 and 3.
CBSG's distinct glow/specular/normal expression is deliberately not enabled here.

`PackedTerSecondaryUv.color_blend` retains all three exact resource identities.
Only the second texture receives new scene registration; no material animation
frames are appended. At GPU upload, current diffuse and normal references must
still match their authored bindings. Missing/wrong-sized channel metadata,
rebinding, alpha/transparent/additive/water/waterfall/clamped materials or direct
heightmap recipes decline this path. Missing physical texture files use the
existing explicit missing-texture behavior, without a guessed alias.

Source and baked coordinates stay unchanged. Both shader coordinate pairs use
the recovered masked-SSE2 signed SHORT2 / 256 conversion. The secondary pair is
stored in one u32 at vertex byte 52 and decoded with signed shifts in the vertex
shader; its independent atlas layer is at byte 56. Vertex stride grows from
52 to 60 bytes (eight bytes per vertex). Existing pass/instance attribute
locations remain unchanged; new inputs occupy locations 12 and 13. Dynamic pose
uploads continue to use the shared Rust vertex type. Material-buffer stride is
unchanged. This memory cost is explicit; no speed improvement is claimed.

## Deferred rendering contract

Opaque texture samples use each coordinate pair's own derivatives. The G-buffer
stores `diffuse * second` and a dedicated flag in its existing normal/flags target.
The lighting pass recovers factor two **after** normalized G-buffer storage and
**before** lighting/fog. Storing the complete doubled value in the normalized
buffer would clamp bright inputs prematurely. No new attachment or pass is added.
Texture alpha does not enable blending, alpha testing or holes for this opaque
family. The material retains opaque depth writes and ordinary opaque shadows.
Current G-buffer quantization remains; this is not native framebuffer equivalence.

## Verification and original corpus

Five GPU tests pass in `tests/layered_color.rs`:

- Independent color multiplication, factor two and zero-alpha opaque depth.
- Bright albedo under ambient lighting, detecting an early G-buffer clamp;
  complete fog replaces the final surface color as before.
- Independent UVs, signed SHORT2 wrapping/quantization and stale-binding fallback.
- Independent secondary minification derivatives, with high-frequency stripes.
- Original Nest CB admission/resource resolution, stable full-scene capture,
  and visible second-color detail against primary-only rendering.

The original fixture targets horizontal DWPC polygon 299832, with the camera
15 units above and 10 units south of its centroid. It covers 52 CB draw groups /
188,101 triangles; 212,643 pixels change in the 640×360 capture. The earlier
metalwall camera targets CBSG and correctly showed no change; it cannot witness
this narrower CB path. Captures `/tmp/openeq-layered-gpu/thenest-{layered,
primary-only}.png` were visually inspected. This is an offline source-backed
view, not a tested player route or original-client screenshot comparison.

An independent raw byte walker inspects all 228 installed TERs and reconciles
production TER-only loads for the 15 affected payloads. Exact bindings cover
673 CB batches / 2,050,190 triangles. All 20 distinct authored second textures
resolve even in those isolated archive loads. All 2,869,912 primary triangle
corners in the affected payloads remain bit-identical; secondary/source identity
checks remain intact. Dranikcatacombsa's separate full-zone dependency remains
unresolved; these terrain-only counts do not certify its complete zone load.

| Local corpus artifact | SHA-256 |
| --- | --- |
| `/tmp/openeq-layered-loader-survey.rs` | `26828df67a920d659ea16b2239e4ebe0fc38475ebf04344d65d7b97dc604c4cd` |
| `/tmp/openeq-layered-loader-survey.json` | `2cbc9867f25659781ab6c2a80e83f0a5621e88b63dc42f2401c9933699c47c6e` |

Independent code review found no blockers. An additional GPU probe verifies
animated primary-layer aliases, case-insensitive reuse, diffuse rebinding,
malformed-channel fallback and vertex offsets; root replay matches output hash
`93151f9a5700a60d199660b5bc8fc370b9c06598de069f797e0cf354fa43156f`.
Reproducer: `/tmp/openeq-layered-color-review.py`.

The integrated workspace passes **1,186 tests, zero failed/ignored, 114 suites**,
including original assets, GPU and digitally silent audio. This includes the
separate three wall-prefix regressions. Strict lint, client/audit builds,
all-target no-default, formatting and diff checks pass. Logs use
`/tmp/openeq-layered-{workspace,clippy,build,no-default,fmt,gpu-final}.log`.
The full 523-zone structural survey retains 501 passes and the same 22 known
problem cases. Its only non-timing changes are 25 newly registered texture
references across 14 fully loadable zones. Reports:
`/tmp/openeq-layered-zone-survey/`. The physical geometry result is unchanged.
