# Restored EQG terrain: bounded offline GPU audit

Baseline recorded 2026-10-01 against renderer **3dd9f90**, before any
ordinary-surface mip filtering changes. The renderer files are unchanged between
that commit and the audit checkpoint 24a3e3b. This is local original-asset evidence,
not a native EverQuest reference comparison. No live client/server or player
session was used.

## Reproduction and boundaries

Run the ignored test in `crates/openeq-render/tests/restored_eqg_audit.rs` with
`CARGO_INCREMENTAL=0 cargo test -p openeq-render --test restored_eqg_audit
-- --ignored --nocapture --test-threads=1`. Originals come from the normal
client directory (`/Users/daeken/EverQuest` here); captures default to
`/tmp/openeq-restored-eqg-gpu/`; set `OPEN_EQ_RESTORED_AUDIT_OUTPUT` to preserve
separate A/B captures. Baseline captures and the final log are preserved
locally in `/tmp/openeq-restored-eqg-gpu-baseline/`. Do not commit or distribute
these images or extracted original assets.

The harness reparses each source TER, independently matches every terrain
material group's packed vertices and indices to the loaded scene, and identifies
restored groups as polygon material ordinals absent from the former stored-ID
map. It verifies restored totals of **142,594 Bazaar**, **250,988 The Nest**, and
**270,445 Thundercrest** triangles. Full-scene, representative isolated-group,
and restored-groups-omitted captures use the same fixed camera and elapsed time
(3 seconds). Repeated full captures are byte-identical. The omission comparison
retains corrected bindings on formerly visible groups; it is **not a complete
replay of the old renderer**.

Every distinct referenced diffuse texture resolves in these three scenes, with
no missing or magenta 1-pixel placeholder decode. The selected representative
diffuses are each 256×256, entirely alpha 255, and use neither alpha masking nor
blending. Successful upload and these selected views do not establish native
parity, complete traversal, every material's appearance, or moving-camera shadow
stability.

## Fixed cameras and source witnesses

All positions are EQ coordinates, FOV 70 degrees. Yaw zero faces +Y; positive
pitch looks up. The renderer maps EQ `(x,y,z)` to world `(x,z,-y)`.

| Zone | Source member; polygon; material ordinal | Eye | Yaw / pitch (degrees) | Material / diffuse |
| --- | --- | --- | --- | --- |
| Bazaar | `ter_bazaar.ter`; 26136; 65 | (760, -775, -26) | -91.29278 / -9.112199 | `stone`, `Opaque_MaxC1.fx`; `clz_stone_base_c.dds` |
| The Nest | `ter_abyss01.ter`; 152182; 1126 | (545, 4340, -230) | -59.049778 / -10.335182 | `metalwall`, `Opaque_MaxCBSG1_2UV.fx`; `Di_nest_metal_wall_c.dds` |
| Thundercrest | `ter_stormtower01.ter`; 200458; 1174 | (-38, 35, 285) | -0.7281657 / -39.55094 | `STORMTOWER_5`, `Opaque_MaxCB1.fx`; `Di_et_ext_woodplain_c.dds` |

The representative groups contain 28,774 / 25,314 / 22,383 triangles respectively.
Additional unprofiled context views start 8 units above actual restored upward
faces: Nest polygon 299832, ordinal 1655, eye
`(69.229485,119.183014,53.403545)`, yaw 0, pitch -5; Thundercrest polygon 200458,
eye `(-38.589172,81.35658,254.71422)`, yaw 45, pitch -5.

All eleven reproducible images were visually inspected. Bazaar's stone pillar,
decorative wall, and tiled floor are solid. Nest's close cave-wall view and wider
cave/pillar/floor view contain restored textured surfaces, with pronounced fine
speckling at distance. Thundercrest's elevated building view and interior view
show solid wood, paper walls, a chest, and hanging kettle. Isolated-group gaps
come from intentionally omitting other material groups. The full-versus-omitted
pixel changes are 518,389 / 518,400 / 404,207 out of 518,400 pixels; isolated versus
empty changes are 68,222 / 331,532 / 195,754. These are visibility checks, not
reference-image quality scores.

## Baseline performance

Apple M4 (10 GPU cores), Metal 4, 960×540. Each profile warms 60 frames, then
measures 100 frames, with a GPU completion wait after each submission. Timestamp
readbacks are separately drained; every profile completed 160/160 queries with
zero failures/drops. This serialized experiment is **not windowed FPS**. GPU
pass values use completion-boundary attribution; overlapping raw pass intervals
must not be added as independent frame costs.

| Zone | Mesh definitions | Scene instances | Lights | Definition triangles | Submitted instanced triangles | Diffuse refs / atlas layers |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| Bazaar | 116 | 840 | 237 | 182,661 | 519,888 | 61 / 61 |
| The Nest | 388 | 1,234 | 0 | 364,385 | 653,763 | 68 / 69 |
| Thundercrest | 374 | 1,728 | 372 | 333,783 | 804,312 | 68 / 75 |

GPU draw-instance totals differ from scene instance counts because the source
objects contain multiple meshes. Vertex/index bytes are 12,418,484 / 2,191,932
(Bazaar), 15,453,776 / 4,372,620 (Nest), and 19,369,688 / 4,005,396 (Thundercrest).

Final baseline run, milliseconds (paired cells are median / p95):

| Zone | CPU load | Upload + completion | CPU submit | GPU frame span | GPU shadow | Omitted GPU frame span |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| Bazaar | 99.895 | 90.373 | 1.0168 / 1.4900 | 4.3672 / 5.2642 | 2.0035 / 2.6647 | 2.0143 / 2.8753 |
| The Nest | 169.048 | 141.373 | 1.4316 / 1.8580 | 3.9534 / 4.7683 | 2.4001 / 2.7767 | 2.0769 / 2.5640 |
| Thundercrest | 222.615 | 134.322 | 1.3863 / 2.3003 | 3.8589 / 8.6673 | 3.0662 / 7.2742 | 3.2579 / 4.4338 |

An earlier identical-camera baseline run measured full GPU frame spans of
4.4145 / 5.1595 (Bazaar), 3.6891 / 4.7088 (Nest), and 6.6817 / 8.7574
(Thundercrest); omitted spans were 1.9987 / 2.2673, 1.9333 / 3.5693, and
3.3700 / 4.1154. The substantial Thundercrest variation prevents a narrow
performance claim. The shadow pass accounts for much of the restored geometry
cost. The original logs remain local as
`/tmp/openeq-restored-eqg-gpu-audit{,-final}.log`.

## Concrete remaining appearance findings

- **Ordinary diffuse minification:** `gbuffer.wgsl` forces LOD 0 despite nine
  uploaded atlas mip levels and a trilinear sampler. The distant Nest view has
  pronounced texture speckling. A bounded derivative-sampling A/B follows this
  frozen baseline; the baseline measurements above must not be relabeled as
  filtered-renderer results.
- **Normal maps:** ordinary opaque material normal references are retained by
  assets, but the renderer only uploads/samples normal maps in its water path.
  This audit does not implement ordinary normal mapping.
- **Nest specialized materials:** 512 restored triangles use
  `Opaque_MaxLava.fx` and 916 use `Opaque_MaxWaterFall.fx`. They currently fall
  through ordinary diffuse rendering; layered/scrolling native behavior is
  unimplemented. Lava witnesses are ordinals 2267, 2291, 3251, 3275 (128 triangles
  each), canonical first-name entry 53 `lavaflow`, with `kl_lavaTop_c.dds`,
  `kl_lavaBottom_c.dds`, and `kl_lava_n.dds`. First polygon 308894 has ordinal
  2267 and flags 262145. Waterfall ordinals 3790, 3792, 3794, 3798, 3800 have
  128 / 128 / 240 / 240 / 180 triangles; canonical entry 3788 `waterfalls`
  references `wtr_waterfall_tile.dds`, first polygon 319110, flags 65537.
- **Thundercrest additive glass:** 72 restored triangles at ordinal 227 use
  `AddAlpha_MaxCB1.fx`, canonical entry 32 `STORMTOWER_33`, diffuse
  `rc_ST_Gcglass_c.dds`, normal `test2_n.dds`. Polygon 19671 is a source witness.
  The current alpha classifier does not recognize this shader and renders it
  as opaque. No speculative additive implementation is included.
- **Sky/fog:** Thundercrest authored sky lookup fails with missing weather
  pattern `clz-0`; its fallback exterior is strongly green. Bazaar lookup fails
  on `NULL`, which may intentionally mean no sky and is not by itself a missing
  asset defect. Nest successfully loads `DefaultClear`, but its offline zone
  settings disable sky. These captures use current offline zone settings,
  not a live server's environment packet; their fog/sky fidelity is unverified.

Restored shader totals: Bazaar `MaxC1` 142,594; Nest `MaxCB1_2UV` 121,401,
`MaxCBSG1_2UV` 117,421, `MaxCG1` 736, `MaxLava` 512, `MaxWater` 10,002,
`MaxWaterFall` 916; Thundercrest `AddAlpha_MaxCB1` 72, `MaxC1` 22,052,
`MaxCB1` 213,132, `MaxCB1_2UV` 33,741, `MaxCBS1` 1,088, `MaxWater` 360.

## Bounded opaque mip correction (after baseline freeze)

The ordinary G-buffer path now uses existing pre-branch UV derivatives with
`textureSampleGrad` for **opaque repeating diffuse textures**. The CPU upload
already provides nine mip levels, and `upload.rs` already supplies a trilinear
sampler. This changes no asset bindings, geometry, normals, instances, lights,
or collision. Water and direct terrain retain their specialized paths.

Alpha-masked/blended materials retain LOD 0 so G-buffer, shadow, and forward
coverage stay consistent. Baked tiles (`FLAG_CLAMP_UV`) also retain LOD 0: their
half-base-texel edge inset is insufficient for minified levels with the current
repeating atlas sampler. Broadening either path requires separate coverage or
edge-filtering work. Ordinary normal mapping, lava, waterfalls, additive glass,
and sky/fog changes are not included.

`tests/texture_minification.rs` adds an original-asset-independent headless test.
Four-texel black/white stripes are projected to roughly 25 source texels per
pixel. Three nonintegral repeat rates exercise different sample phases, and a
0.017-unit camera move shifts the image by a fraction of one screen pixel.
Before the shader edit, repeat 11.3 produced a full **255-step spatial range**
and **115.422-step mean change under motion**, failing the regression. After:

| UV repeat | Spatial range (8-bit steps) | Mean change under camera shift | Mean gray |
| --- | ---: | ---: | ---: |
| 11.3 | 10 | 0.531 | 127.984 |
| 12.7 | 6 | 0.562 | 128.031 |
| 15.1 | 7 | 0.562 | 128.031 |

The same test confirms black and white details remain resolved when magnified.
This does **not** claim physically correct area filtering: the preexisting CPU
mip builder averages encoded sRGB bytes, rather than linear light, and uses
nonperiodic triangle-filter boundaries. The latter leaves the small measured
seam residual. Those upload changes are deliberately outside this correction;
using existing mips also makes distant high-contrast textures darker than a
linear-light mip chain would.

The original audit passes with filtered captures preserved separately in
`/tmp/openeq-restored-eqg-gpu-mip/` and log
`/tmp/openeq-restored-eqg-gpu-mip.log`. Full views for all three zones and the
Nest/Thundercrest context views were inspected. Nest's formerly dense fine
speckling is substantially reduced; nearby rock, Bazaar stonework, and
Thundercrest interior details remain visible. Same-camera/time captures are
still byte-identical. This is a static appearance A/B plus synthetic camera
motion test, not a native-client or live traversal comparison.

The filtered run's GPU frame-span median / p95 in milliseconds was
3.8112 / 6.6477 (Bazaar), 4.0953 / 6.3368 (Nest), and 7.0676 / 9.1285
(Thundercrest). These are **separate from the frozen baseline above**. Wide
run-to-run variation, including in the unchanged shadow pass, prevents a
performance improvement/regression claim from these observations.

Validation after the change: the new minification test and restored-terrain
audit pass; 14 neighboring GPU checks pass across `eqg_material_identity`,
`indexed_water`, `terrain_addressing`, `terrain_materials`, and `transparency`
(including original Housegarden, Feerrott2, Deadhills, and Oldcommons assets).
The focused new tests pass strict Clippy; formatting and whitespace checks
pass. Commands used `CARGO_INCREMENTAL=0` and serialized GPU execution.
