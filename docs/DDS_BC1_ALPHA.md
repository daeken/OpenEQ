# Legacy DDS DXT1 alpha: original assets and native upload

Evidence checkpoint: 2026-10-01. The installed assets contain **51 DXT1 texture
members with transparent base-level pixels**, representing 39 distinct DDS
files across 23 EQG archives. The ordinary native DDS upload path requests
`D3DFMT_DXT1` and forwards their original compressed bytes unchanged. Decoding
these DDS files to RGB and then adding alpha 255 loses authored transparency.
Restoring BC1 alpha must not depend on the DDS alpha-pixel flag: all 51 affected
members have pixel-format flags `0x4` (FOURCC only).

This audit is independent of the Rust texture/PFS parsers. It adds no original
asset bytes to the repository. Encoded alpha does not establish whether a
particular material enables alpha testing or blending.

## BC1 rule and decoder change

Microsoft's Direct3D 9 documentation,
[Opaque and 1-Bit Alpha Textures](https://learn.microsoft.com/en-us/windows/win32/direct3d9/opaque-and-1-bit-alpha-textures),
defines the rule per eight-byte DXT1 block:

- Read the two RGB565 endpoints as unsigned 16-bit integers.
- If endpoint 0 is greater than endpoint 1, all four selectors are opaque.
- Otherwise, selector 3 is transparent; selectors 0, 1 and 2 remain opaque.
- The sixteen two-bit selectors are in row-major texel order within the
  four-by-four block. Equal endpoints use the transparent-selector mode too.

`image` 0.25.10 exposes legacy DXT1 decoding as RGB8. The OpenEQ fix retains
that decoder's existing RGB values and restores only alpha zero for the
encoded transparent selectors. It processes the current image's base level,
clips partial edge blocks and leaves later mip levels untouched. Opaque black
must stay opaque; black RGB is not itself a transparency key.

Review of `crates/openeq-assets/src/texture.rs` found the fix consistent with
that rule, including equality, four-color selector 3, the level-zero boundary,
payload bounds and edge texels. This alpha correction does not alter the RGB
palette quantization or enable material transparency automatically.

## Original-asset survey

The temporary Python survey reads the PFS directory and zlib blocks directly,
sniffs DDS magic regardless of member extension, reads the legacy DDS header,
and inspects compressed BC1 blocks without decoding their RGB colors. For
full blocks, it counts selector 3 with
`(selectors & (selectors >> 1) & 0x55555555).bit_count()` only when
`endpoint0 <= endpoint1`. Edge masks exclude texels outside the declared
image. Cubemap face offsets include each face's complete mip chain, but pixel
counts concern base levels only. Volume depth is accounted for as well.

| Corpus/result | Count |
| --- | ---: |
| Root-level `.s3d`/`.eqg` archives in `/Users/daeken/EverQuest` | 3,246 |
| Archive bytes inspected | 9,361,840,058 |
| Payload members inspected | 313,594 |
| DDS members | 124,813 |
| DXT1 members | 63,858 |
| Distinct DXT1 file SHA-256 values | 27,999 |
| Members with at least one transparent base texel | 51 |
| Distinct affected file SHA-256 values | 39 |
| Archives containing affected members | 23 |
| Transparent base texels, counting archive copies | 869,900 |
| Parsing errors | 0 |

Three archives have one stale directory name without a payload: `eye_chr.s3d`
(`eye_chr.wld`), `greatdivide_chr.s3d` (`growthplane_chr.wld`) and
`velketor_chr.s3d` (`kael_chr.wld`). For these only, the survey matches actual
payloads using the original filename CRC algorithm instead of pairing unequal
name/entry lists. Every payload resolves; the three stale names are recorded
separately, not treated as texture data or silently assigned to another file.

All 51 affected members are ordinary 2D textures in EQG archives. None uses
DDS alpha flags beyond FOURCC. Forty-nine DXT1 members in the whole corpus
have multiple faces or depth; none has affected base texels. A second,
independent per-pixel coordinate/selector loop checks every base texel of all
39 distinct affected files and reproduces every transparent-pixel count.

Representative originals:

| Archive / member | Dimensions | Transparent base texels | DDS SHA-256 |
| --- | --- | ---: | --- |
| `cosul.eqg / lamp_chainlink.dds` | 64 × 64 | 2,938 | `1a733b4fd5d8910cda3347a49d40cf47c173a323b57484ec8068a08b3ac9bfa4` |
| `broodlands.eqg / swmp_canopy_trim.dds` | 256 × 256 | 22,977 | `9c93049c8450d87461765bcb54a92feb4954d95a87e851c0fbb3b7ea3902d90c` |
| `broodlands.eqg / swmp_canopy_trim2.dds` | 256 × 256 | 28,122 | `091db56eb2b0455cc37354f2abbf1d8b84a6ba2103228fee775e7d97bc16ddda` |
| `bloodiron_green.eqg / chain_c.dds` | 256 × 256 | 43,528 | `8c27dbcedaa5c68ba5e90a4dd9a18edbffb6a0fafa68e06800672217ff5cf66e` |
| `thulehouse2.eqg / ice_ab_chainlink_c.dds` | 64 × 64 | 2,938 | `a062066ffffaf670a570a9d173c6e4870ac807562bfd1a78e55974c993cbfa34` |
| `undequip.eqg / sku17_weapons_flamefist_invisble.dds` | 64 × 64 | 4,096 | `94de1e38c1f3ed0c0d977285541842c6c4795f7052682121ebb41d8990c3ecce` |

The exact `lamp_chainlink.dds` bytes occur in twelve archives. Some other
files have only a few transparent texels: for example,
`olddranik.eqg / stone_tile02.dds` has one. The survey does not infer material
intent from filenames or from the number of transparent pixels.

### PoK and Greater Faydark are negative controls

None of these original DXT1 base levels contains a transparent selector:

| Archive | DXT1 members | Affected |
| --- | ---: | ---: |
| `gfaydark.s3d` | 34 | 0 |
| `gfaydark_obj.s3d` | 125 | 0 |
| `poknowledge.s3d` | 34 | 0 |
| `poknowledge_chr.s3d` | 561 | 0 |
| `poknowledge_obj.s3d` | 289 | 0 |
| `poknowledge_obj2.s3d` | 8 | 0 |
| `poknowledge_obj3.eqg` | 12 | 0 |

`gfaydark_chr.s3d` contains no DDS members. These results do not explain
separate BMP palette masking, DXT3/DXT5 alpha, material selection or lighting
problems in those zones. Lower mip levels are not included in these counts.

## Native ordinary DDS upload witness

The installed `EQGraphicsDX9.dll` has preferred base `0x10000000`, SHA-256
`615e7ba9e20745ec03a908cf47c038cc764358ad820d9d4d3232734fadff5383`.
Its ordinary archive-backed texture loader `0x100610d0`:

1. Reads the original member through the archive interface.
2. Recognizes DDS and DXT1 at `0x10061426..0x1006145b`.
3. Selects format `0x31545844`, `D3DFMT_DXT1`, at `0x100615e5`.
4. Calls `0x100b9af2` through the call at `0x100619e4` for the captured path.
   The wrapper jumps through import `0x101332a0`, verified as
   `d3dx9_30.dll!D3DXCreateTextureFromFileInMemoryEx`.

A bounded Unicorn witness executes this routine with the original
`lamp_chainlink.dds` and `swmp_canopy_trim.dds` files. It tests renderer quality
values 0, 2 and 3 with mip selection disabled/enabled, producing twelve calls.
Every captured API call requests `D3DFMT_DXT1`, passes color key 0 and forwards
an exact byte-for-byte copy of the original DDS member, including its existing
FOURCC-only flags. Quality settings change dimensions/filter arguments; they
do not switch these inputs to an opaque RGB format or rewrite the BC1 blocks.

Microsoft's
[D3DXCreateTextureFromFileInMemoryEx documentation](https://learn.microsoft.com/en-us/windows/win32/direct3d9/d3dxcreatetexturefromfileinmemoryex)
defines color key 0 as disabling color-key replacement. Thus no caller-side
color-key convention replaces this alpha rule. The documentation also warns
that a returned texture can have a format different from the requested one.
The witness intercepts the D3DX boundary: it proves the original caller's
request and unmodified source, not D3DX's implementation, device behavior,
resized/generated mip contents or final rendered pixels. Archive interfaces,
allocation/free and the final API result are controlled; no original client
process or graphics device is running inside this witness.

## Decoder and rendered-mask validation

The implementation tests in
`crates/openeq-render/tests/wld_particle_texture_decode.rs` compare CPU output
with hardware `Bc1RgbaUnorm` at unfiltered original texel coordinates. Synthetic
blocks cover both endpoint orderings, equal endpoints, opaque black and all
selectors. Original PoK particle textures remain negative controls; the
chain-link and canopy originals above exercise actual transparent pixels.
Alpha agrees exactly. Existing RGB interpolation differences remain bounded
at two stored-byte steps; this patch does not change the CPU RGB decoder.

A separate headless scene uses the original `lamp_chainlink.dds` on a plane
with `alpha_mask = true`, with an opaque blue plane behind it. The before
control forces alpha to 255 while preserving every RGB value; the after
render uses the restored alpha. In the central 128-by-128 footprint, the
passing run reveals blue through **13,312** previously opaque pixels and
retains **3,072** identical opaque pixels. Both captures were visually
inspected. They are `/tmp/openeq-bc1-alpha-preview/before.png` and `after.png`,
kept outside the repository.

This exercises OpenEQ's actual masked-material render path with original
texture bytes. It is a controlled material/geometry fixture, not evidence of
Cosul's original material assignment or complete original-client rendering.
The three targeted GPU/texture tests pass; these original-asset tests are
opt-in because they require the installed files and a compatible GPU.

## Frozen evidence

All survey, crosscheck and native-upload assertions pass. Temporary artifacts
contain metadata and checksums, not extracted texture binaries:

| Artifact | SHA-256 |
| --- | --- |
| `/tmp/openeq-dds-bc1-alpha-survey.py` | `558fdbde80b98bf687295d74e0403daa3b91a91d3ad9d5da3b527029d0adb3e1` |
| `/tmp/openeq-dds-bc1-alpha-survey.json` | `0587492962b412e0d0df1be27ab02f19993334b318f4f0b27515715442bddc54` |
| `/tmp/openeq-dds-bc1-alpha-crosscheck.py` | `ab6cd17255db8087f8ffb5af4d55ba1c46376da9f94835431575971b0c653dc0` |
| `/tmp/openeq-dds-bc1-alpha-crosscheck.json` | `9ee8aa96313985cb5bdc32caf266b3ac1220bf7f0c1aa8ff624af9f0727bec47` |
| `/tmp/openeq-native-dds-upload.py` | `615039133dd41513737b88798116856e60ba790162e6de65733a4662ef438555` |
| `/tmp/openeq-native-dds-upload.json` | `8ba566164830f7ed2d42175e2760dcd0c67765c8ccf61b203dd70e975726f642` |

The survey and crosscheck use Python's standard library. The native witness
runs with `PYTHONPATH=/tmp/openeq-re-tools` and reuses only emulator setup from
`/tmp/openeq-native-particle-context.py` plus this survey's PFS reader. It does
not execute those scripts' output-writing experiment bodies.
