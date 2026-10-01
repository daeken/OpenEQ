# Authored nonfinite EQG attributes

2026-10-01, diagnostic research only. The 21 nonfinite-mesh outliers in
[WORLD_AUDIO_PARITY.md](WORLD_AUDIO_PARITY.md) contain nonfinite words in the
original TER/MOD vertex records. This investigation found no vertex-offset or
version-decoding error responsible for those outliers. It does **not** establish
how every original render path displays malformed values, and changes neither
the parser nor drawable/collision geometry.

## Source and loaded-mesh correlation

An independent PFS/payload walk inspected all **4,572 TER/MOD payloads** in the
21 archives. A separate Rust probe compared every parsed position, normal,
primary UV and polygon word to its exact source bits, including NaN payloads.
All matched; all polygon vertex indices were in range. These archives use
versions 1, 2 and 3. Source material groups referenced by the loaded scenes
reproduce **every** position/normal/UV count in the final structural survey.

`mesh::pack` already copies only vertices referenced by each material's index
list. These diagnostics are not unused vertices accidentally copied into every
material batch. Heightmap props were correlated against loaded object names,
not all archived MODs; for example, Direwind's unused weapon-rack variants and
Zhisza's unused still LOD must not increase loaded-scene counts.

The table counts source triangles once per loaded zone definition, without
multiplying by instance count. P/N/UV are affected drawable material groups.
"Finite faces" have finite positions and **strictly nonzero** squared cross
product in f64, but a nonfinite normal or primary UV. No affected finite face
had exactly zero area. These are geometry facts, not visibility measurements.

| Zone | P | N | UV | Nonfinite-position faces | Finite faces |
| --- | ---: | ---: | ---: | ---: | ---: |
| arxmentis | 26 | 26 | 28 | 189 | 122 |
| bloodfields | 0 | 0 | 9 | 0 | 45 |
| breedinggrounds | 1 | 1 | 1 | 1 | 0 |
| causeway | 0 | 0 | 1 | 0 | 10 |
| chapterhouse | 1 | 2 | 1 | 2 | 3 |
| direwind | 0 | 0 | 4 | 0 | 48 |
| dranik | 0 | 0 | 4 | 0 | 39 |
| draniksscar | 0 | 0 | 1 | 0 | 20 |
| kattacastrum | 0 | 0 | 3 | 0 | 36 |
| kattacastrumb | 0 | 0 | 3 | 0 | 36 |
| thevoida–thevoidh, each | 0 | 0 | 1 | 0 | 2 |
| valdeholm | 0 | 0 | 1 | 0 | 3 |
| wallofslaughter | 0 | 0 | 1 | 0 | 20 |
| zhisza | 0 | 0 | 1 | 0 | 6 |

The totals are 192 nonfinite-position faces and 404 finite faces. ArxMentis's
189 position-invalid faces all have polygon bit 0 set, so the existing physical
bake excludes them as passable. Breedinggrounds's one face has flags `0x40000`;
Chapterhouse's two have flags zero. Their invalid positions are already rejected
by the physical bake. UV faults do not justify deleting collision surfaces.

## Record layout and exact witnesses

The installed `EQGraphicsDX9.dll` SHA-256 is
`615e7ba9e20745ec03a908cf47c038cc764358ad820d9d4d3232734fadff5383`.
Native addresses use preferred image base `0x10000000`.

- MOD reader `0x10062985` tests version >= 3 at `0x100629e4`; TER reader
  `0x100643b0` does the same at `0x1006440b`.
- Shared processing `0x10062580` walks each material as
  `16 + 12 * property_count` bytes, then uses a 44-byte vertex stride at
  `0x100625ce` or 32 bytes at `0x100625e0`. Polygons are 20 bytes.
- Version 1/2 vertices contain XYZ at `+0`, normal XYZ at `+12`, UV at `+24`.
  Version 3 adds a packed color at `+24`, then UV0 at `+28` and UV1 at `+36`.
  Here UV0 means the first stored pair, which OpenEQ currently retains as
  `tex_coords`; UV1 means the second pair, currently skipped.
- Native legacy conversion copies `+24/+28` to the expanded record's
  `+28/+32` at `0x10062b15..0x10062b24`, independently confirming which pair is
  primary. TER has the corresponding copy at `0x1006455b..0x10064573`.

All offsets below are **uncompressed MOD payload byte offsets**, and vertex and
polygon indices are zero-based. Values are little-endian raw IEEE-754 words.

| Archive / payload | Vertex | Source evidence |
| --- | ---: | --- |
| `bloodfields.eqg/obj_gatedoor.mod` (v2) | 234 | UV at `0x1fea/0x1fee` = `ffffffff/7fffffff`; finite position/normal |
| `arxmentis.eqg/obp_am_drway_a.mod` (v3) | 26 | XYZ starts `0x205f`; XYZ, normals, UV0 `0x207b/0x207f`, UV1 `0x2083/0x2087` all `ffc00000` |
| same ArxMentis payload | 18 | Normal starts `0x1f0b` = three `ffc00000` words despite finite position and UVs |
| `breedinggrounds.eqg/obj_maus_innerwalls.mod` (v3) | 1613 | XYZ starts `0x125e0`; XYZ/normal/both UV pairs are `ffc00000` |
| `chapterhouse.eqg/obj_bfs_hallwallh.mod` (v3) | 323 | UV0 `0x59af/0x59b3`, UV1 `0x59b7/0x59bb` all `ffc00000` |
| `thevoida.eqg/obj_rallos_axe.mod` (v3) | 289 | UV0 `0x32eb/0x32ef` = `00000000/ffffffff`; UV1 `0x32f3/0x32f7` = zero/zero |
| `direwind.eqg/obj_direwind_tent_b.mod` (v3) | 984 | UV0 `0xade7/0xadeb` = `7fffff00/80000000`; UV1 `0xadef/0xadf3` = zero/zero |
| `valdeholm.eqg/obj_rz_statue.mod` (v3) | 3231 | UV0 `0x22f50/0x22f54` = `00000000/7fffffff`; UV1 `0x22f58/0x22f5c` = zero/zero |
| `zhisza.eqg/obj_cave_still.mod` (v3) | 46 | UV0 `0xb8e/0xb92` = `ffffffff/7fffffff`; UV1 `0xb96/0xb9a` = zero/zero |

These are NaNs, not infinities. Source `ffffffff`, `7fffffff`, `7fffff00` and
`ffc00000` all have the IEEE exponent set and nonzero fraction. `80000000` in
the tent's other component is ordinary negative zero.

Representative payload SHA-256 values:

| Payload | SHA-256 |
| --- | --- |
| Bloodfields `obj_gatedoor.mod` | `9f148917d1beec85e642d0bdd2cb2a709b2cb00775bb0ac5c10dd7b0eb52d4fb` |
| ArxMentis `obp_am_drway_a.mod` | `c12ee63f939c689cd0b1d79602d937bbde95fb9ac2f90fd0619b926adb82480b` |
| The Void `obj_rallos_axe.mod` | `49f64dda4139fc07c9afa21c7567bdba622324bc9d6c3bc44abc6279f200b551` |
| Valdeholm `obj_rz_statue.mod` | `162eac35d296c9add1cc5c83c9ac61764b8f956e1f745971d2943de662c10b47` |

The axe's NaN belongs to polygons 54/59, whose squared cross products are
approximately `0.104972/0.104875`. The statue's affected polygons 3425–3427 have
values `5.21618/46.94646/5.21631`. Arx doorway polygon 23 has a bad normal but
finite positions and area squared `0.209604`. Dropping only zero-area geometry
cannot resolve these cases; dropping every bad-attribute face removes real area.

## Material and UV evidence

No affected material in this investigation has a `mapChannel` property or any
other property whose name contains `channel`. The explicit tag-1 channel words
in [Housegarden](EQG_MATERIAL_PROPERTIES.md) therefore do not explain these
outliers. A finite UV1 pair is not permission to substitute it for UV0.

Representative affected materials and their complete property roles:

| Payload/material | Shader | Authored properties |
| --- | --- | --- |
| Bloodfields gate / `Material #2` | `Opaque_MaxC1.fx` | diffuse `ab_wall_T01.dds` |
| Bloodfields entrance / `Material #30` | `Opaque_MaxCB1.fx` | diffuse `Di_ent_cinderblock02_high_c.dds`, normal `Di_ent_cinderblock01_mid_n.dds` |
| Arx doorway / `dark blue marble` | `Opaque_MPLBump.fx` | diffuse `darkblue_marble_plainC_512_c.dds`, normal `temp_n.dds`, coverage `coverage_base.dds`, coverage scale 0.08 |
| Chapterhouse wall / `conctrimA` | `Opaque_MPLBump2UV.fx` | diffuse `bfs_conctrimE_c.dds`, normal `bfs_conctrimE_n.dds`, coverage `clz_erosion_ov_vert.dds` |
| The Void axe / `a` | `Opaque_MPLBasic.fx` | diffuse `rallos_axe_01_c.dds` |
| Direwind tent / `wood` | `Opaque_MPLBump.fx` | diffuse `book_stump_bark_01_c.dds`, normal `book_stump_bark_01_n.dds`, coverage `coverage.dds`, coverage scale 0.05 |
| Valdeholm statue / `stone_a` | `Opaque_MPLBump.fx` | diffuse `rz_stone_a_01.dds`, normal `rallos_zek_01_n.dds`, coverage `coverage_snow.dds`, coverage scale 0.05 |
| Zhisza still / `tub` | `Opaque_MPLSB.fx` | diffuse `ew_still_Tub_c4.dds`, normal `ew_still_Tub_n3.dds`, coverage `coverage_a.dds`, shininess 48, coverage scale 0.05 |

UV1 can also be malformed when UV0 is entirely finite. For example, ArxMentis
`obj_wizard_spire.mod` has 512 vertices with NaNs in UV1 and no nonfinite XYZ,
normal or UV0. Chapterhouse has further UV1-only examples in its bed and pipe
models. These do not appear in OpenEQ's current primary-UV diagnostic; auditing
both stored pairs is necessary before implementing coverage/channel support.

## Native upload and shader lead; remaining boundary

Native source copying is not the end of the path. The MOD reader constructs a
simple-model definition at `0x100634c3`, initializes it through `0x100568b0` at
`0x1006350e`, and invokes its vertex-copy slot `+0x18` at `0x10056a1e`.
That slot is `0x100579f0`, which preserves both CPU UV pairs. The simple-model
upload described in [the indexed-water research](HEIGHTMAP_WATER_REVERSE_ENGINEERING.md)
converts UV components to **signed SHORT2 with scale 256** before shader input;
this shared machinery is a promising explanation for why source NaNs need not
reach native texture sampling as floating-point NaNs.

At `0x1008ed6b..0x1008ed87`, for example, upload multiplies by 256, calls
`0x1010f280`, then stores AX. Its SSE branch executes `cvttsd2si` at
`0x1010f295`; with masked invalid exceptions NaN produces integer indefinite
`0x80000000`, whose low 16 bits are zero. The x87 fallback uses `fistp qword`
at `0x1010f2c5`; masked NaN produces the 64-bit indefinite value, and its
zero-low-word branch at `0x1010f315..0x1010f325` also returns low word zero.
This is **conditional instruction-level evidence**, not an implemented policy:
the selected per-material upload layout, active floating-point environment,
terrain path, and normal/position treatment still need complete tracing.

The following installed effect containers were parsed to exact EOF and their
programs decoded through END, using the same D3DX9 readers as the water research:

| Effect beneath `RenderEffects/` | Bytes | Relevant program evidence (file offsets) |
| --- | ---: | --- |
| `MPL/SModel_TextureD1.fxo` | 2076 | VS at `0x5b0`; `TEXCOORD0` input v1, `oT0 = v1 / 256` at `0x7a4` |
| `SPL/SModel_Bump.fxo` | 11588 | VS at `0x1fb0`; `TEXCOORD0` v4 divided by 256 at `0x2c70`; same result feeds diffuse/normal, coverage additionally multiplies `e_fCoverageScale0` at `0x2c94` |
| `SPL/SModel_Bump_2UV.fxo` | 11572 | VS at `0x1fb8`; diffuse/normal use v4=`TEXCOORD0` at `0x2c84`; coverage uses v5=`TEXCOORD1` at `0x2c74`; both divide by 256 |
| `SPL/SModel_SB.fxo` | 17008 | VS at `0x2444` and `0x3484`; primary UV divided by 256 at `0x3104`/`0x4144`, then shared by diffuse/normal and scaled for coverage |

Their SHA-256 values in the same order are
`d0545ba2f9bd6e3145354b6b7757084b85336edb174e0b84965121f498c2f915`,
`37ff430457f1316c77e1e3361f41b2531029a2967c11126182bb1b80b07314cc`,
`32e195caaa8b1d20c697534d24a9c534cb7de005d7da6364a7c6b5bf56eb8a73`, and
`855bd119aec23733907d794b27c64ee7e66250eb927890c943e9118f74f04770`.

Those programs use supplied UVs for diffuse sampling, not generated world
coordinates. That statement is restricted to the examined effect programs:
the complete material-to-pass/technique selection for the affected fixtures
has **not** been established. Similar effect names alone do not close that
boundary. They also do not establish that a zero substitution in OpenEQ's raw
parser would reproduce the original renderer.

Next work should complete that binding and packed-vertex bridge with original
finite and nonfinite witnesses, then implement any proven conversion at the
appropriate render boundary while retaining raw source data. Position handling
and normal packing need separate evidence. Do not remove finite faces, invent
UV/normal values, switch to UV1, or relabel these zones as structurally clean
on the strength of this diagnostic note.

Local investigation files: `/tmp/openeq-nonfinite-source.py`, its `.json`/`.log`
reports, `/tmp/openeq-nonfinite-probe.rs` and `.log`, and
`/tmp/openeq-nonfinite-*-{effect,shader}.txt`. The source report stores archive
and payload hashes, exact offsets, raw words, material properties and affected
source polygons. The probe verified all 4,572 payloads and the loaded-scene
counts against `/tmp/openeq-world-audio-zone-survey-final/zones.jsonl`.
Only original assets under `/Users/daeken/EverQuest` were read; no original
binary data or derived disassembly is included in this repository.
