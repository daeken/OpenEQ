# Native TER UV upload compatibility

2026-10-01. This implements the UV boundary established in
[EQG_NONFINITE_TER_UPLOAD.md](EQG_NONFINITE_TER_UPLOAD.md), with a deliberately
explicit **SSE2, masked-exception** conversion target. Source parsing, baked
CPU vertices, collision, and structural nonfinite diagnostics remain unchanged.
This does not complete native normal, tangent, vertex-color or lighting parity.

## Supported material and version scope

Only ordinary `.ter` versions 1–3 whose resolved material shader is exactly
`Opaque_MaxCB1.fx` (case-sensitive) receive `NativeTerShort2Sse2` provenance.
The name is matched after the native first-exact-material-name resolution.
Other material families, `_2UV` variants, `.mod`, WLD, heightmap terrain, water
and unknown TER versions retain their existing float UV upload.

`Material::uv_encoding` travels with material clones and remapping. At
`GpuScene::build`, it converts UV0 into the float values that the native shader
would obtain from its signed `SHORT2` input. Every supported UV is converted,
including ordinary finite coordinates. Raw `TerMod::tex_coords` and
`Geometry::vertices` are never rewritten, nor are polygons removed.

The installed `EQGraphicsDX9.dll` is the same binary identified in the preceding
research. Its TER reader at `0x100643b0` accepts version >= 1 and tests version
>= 3 at `0x1006440b`. Shared reader `0x10062580` selects a 32-byte source stride
for v1/v2 (`0x100625d9..e3`) or 44 bytes for v3 (`0x100625ca..d4`). The legacy
expansion copies UV0 from +24/+28 to +28/+32. Version 3 already has that expanded
layout. Both reach the same attribute-copy, region and layout-1 upload route
documented previously. The scope stops at v3 because the current parser does
not establish higher-version compatibility.

Material type 8 becomes runtime type 9 with bump byte +8 set and second-UV byte
+9 clear (`0x10015b03..13`). Ordinary region selection at `0x100196ed..0x1001970c`
therefore chooses layout 1. Its declaration and every established SPL/MPL
technique decode `SHORT2 TEXCOORD0 / 256`. Secondary UVs do not participate.

The native family parser uses case-sensitive substring searches, including
`CB1` at `0x10014d94` through `0x1010f330`. OpenEQ deliberately recognizes only
the complete authored spelling established here; it does not infer compatibility
for arbitrary substrings or case variants. All 278 surveyed records have that
same exact spelling.

## CPU selection and precise numeric rule

The native conversion flag at `0x101d6d0c` is not a renderer preference. CRT
initializer `0x101199cc`, referenced by the initializer table at `0x10133438`,
calls the import at `0x101330f0`: `IsProcessorFeaturePresent(10)`
(`PF_XMMI64_INSTRUCTIONS_AVAILABLE`, Windows SSE2 availability). Its result is
stored at `0x101199d4`. The upload calls helper `0x1010f280`, which selects SSE2
when this flag is nonzero, otherwise the legacy x87 implementation.

The SSE2 route is reproduced in software on every OpenEQ host:

1. Multiply the source `f32` value by 256 in a wider representation. The native
   x87 multiply and double store retain this product exactly for finite f32
   inputs; an f32 multiply could overflow prematurely.
2. Truncate to signed i32. With masked invalid exceptions, NaN, infinity or a
   product outside `[-2147483648, 2147483648)` produce integer indefinite
   `0x80000000`, as native `CVTTSD2SI` does.
3. Keep the low 16 bits, interpret them as signed i16, then divide by 256.

This wraps valid large integers instead of saturating them: UV 128 becomes
-128, and source word `477fffff` becomes -1/256. Source words `638a681c` and
`e38a6804` from Causeway's extremely large finite neighbor both become zero,
as do its authored NaNs. This is not a nonfinite-only replacement.

`CVTTSD2SI` ignores the rounding mode. Isolated execution of the original
layout-1 loop checked 5,029 source words, including random bit patterns and
boundary cases, under 24 combinations: SSE2/x87, x87 24/53/64-bit precision,
and all four rounding modes, with exceptions masked. The software rule matched
every SSE2 result. The x87 results were also invariant across those tested
control words, but do not universally equal SSE2.

The legacy x87 helper uses a signed 64-bit integer store before returning the
low word. For example, UV 8,388,609 (word `4b000001`) becomes decoded UV 1 on
x87 but 0 on SSE2; its negative counterpart becomes -1 versus 0. For source
f32 inputs, differing low words occur only within `2^23 < abs(UV) < 2^31`,
and some values in that interval still coincide. Above that interval, f32's
spacing makes valid integer products multiples of 65536; still larger values
take indefinite conversion paths, again yielding zero low words.

No running original-client process was instrumented. Its effective exception
masks remain unobserved; unmasked exceptions need not yield a rendered vertex.
The implementation explicitly targets the proven masked SSE2 path, not every
possible original CPU/FPU environment. It does not execute native binaries,
inspect the OpenEQ host's CPU features, or modify host floating-point settings.

## Original corpus and verification

An independent walk of all local `.eqg` archives found 228 TER payloads without
archive/parse errors: 1 v1, 33 v2 and 194 v3. The supported exact shader occurs
in 278 referenced material records across 24 TERs: 21 v2 and 3 v3. The v3
fixtures are Guild Hall, Guild Lobby and Roost. The lone v1 TER, `fhalls.eqg`
`ter_temple01.ter`, uses older differently named shader families outside this
scope; v1's shared route is established statically and tested synthetically.

Across those supported records, 5,376,858 referenced UV components include
96 nonfinite components and 5,242,012 finite values changed by quantization or
wrapping. No supported original coordinate has differing SSE2/x87 low words.
These counts are per source material's referenced vertices, before baked
deduplication, and are not a replacement for the loaded-zone structural survey.

Tests cover version parsing/UV0 selection, exact shader exclusions, resolved
material names, raw word preservation, cloning/remapping, the original v2/v3
geometry and independent native numeric witnesses. Causeway material 6 keeps
all 879 faces and every baked attribute word. Its ten source vertices with bad
UVs deduplicate to nine baked vertices; that preexisting diagnostic count stays
nine after GPU upload.

The GPU witness loads the entire original Causeway scene and checks the actual
upload values against six native instruction witnesses, preserving positions
and normals. It then enlarges four of those UV witnesses onto a visible quad
with the original `sp2_c.dds` texture. The rendered pixels exactly equal a
separately specified native-decoded reference; over 20,000 pixels differ from
the empty frame. `/tmp/openeq-ter-uv.png` records that **synthetic enlarged UV
witness**, not a claim about visibility of the original thin triangles.

This fixes the UV upload boundary only. Native packed normals/tangent frames,
authored vertex colors, bump shading, and other material families retain their
separate compatibility gaps. The structural nonfinite survey must continue
reporting original malformed attributes.

Temporary independent evidence on the investigation host:

| Artifact | SHA-256 |
| --- | --- |
| `/tmp/openeq-ter-uv-range-probe.py` | `baf1b3e7635a6a515974c2f5c5683cf25b844a560b5312dc6b02a1a3055ad54c` |
| `/tmp/openeq-ter-uv-range-probe.json` | `c57d1f8e8edaaff890ac555df905b80bb661e5c329689adba508da7dbaaf9f68` |
| `/tmp/openeq-ter-uv-corpus.py` | `bbc9df93c280aab462de2c392b7b9ad1b17ebc5237702c806c84a6685e34ee7d` |
| `/tmp/openeq-ter-uv-corpus.json` | `7443eb1dbd564022822939fc50c1097c3cbd09bfc3b1767918dcf24e3d911c49` |

The numeric probe uses the existing `/tmp/openeq-ter-upload-venv` Unicorn
environment. Temporary scripts and original assets are not runtime dependencies
and are not included in the repository.

Independent review found no actionable issue. A second native instruction probe
checked 4,406 float bit patterns across all four rounding modes against an
integer-only IEEE-754 oracle; every result matched. The integrated workspace
passed 1,031 tests (zero failed/ignored), including original assets, GPU and
silent audio, plus strict lint, client build, no-default checks and formatting.
Evidence uses `/tmp/openeq-uv-loops-*`. This run also includes the separate
loop scheduler and SysEx synthesis API changes.
