# Version 2 TER trailing texture coordinates

The post-polygon data in The Nest is an active second texture-coordinate
source stream. The original TER loader recognizes a leading word of **1 or 2**
for **version 2 only**, then reads one float32 pair per original vertex. It
copies those pairs into the same expanded vertex fields used by version 3's
interleaved secondary coordinates. The installed corpus contains 16 such
version 2 streams, covering 4,473,834 vertices.

This establishes source retention and the native CPU copy. It does not prove
the complete `_2UV` shader binding, upload format, texture selection or pixels.
Those require separate research before changing rendering. It extends
[the existing vertex-channel report](EQG_TER_VERTEX_CHANNELS.md).

## Native dispatch and copy

Addresses refer to the installed `EQGraphicsDX9.dll`, SHA-256
`615e7ba9e20745ec03a908cf47c038cc764358ad820d9d4d3232734fadff5383`.
The ordinary EQG member parser calls TER reader `0x100643b0` at
`0x10066499`. The TER reader accepts header versions at least 1, then calls
shared reader `0x10062580` at `0x1006445d`.

The shared reader advances past all material records, source vertices and
20-byte polygons. The resulting cursor is stored at `0x100625f7` and is the
tail position used by the TER reader. Source vertex stride is 32 bytes below
version 3 and 44 bytes for version 3 or higher.

At `0x1006449e`, the TER reader compares the version to **exactly 2**. Only
that version reads the first tail word, at `0x100644a8`. Comparisons at
`0x100644aa` and `0x100644af` recognize 1 and 2; either sets the coordinate
pointer to `tail + 4` at `0x100644b4`. Other values leave the pointer null.
The leading word is therefore documented here as a **tag**, not as a stream
count: tag 2 does not make this path read two streams.

The legacy expansion loop `0x100644f1..0x100645aa` builds 44-byte vertices:

| Expanded offsets | Source |
| --- | --- |
| `+0..+20` | Original position and normal float words |
| `+24` | Literal source color `0xff808080` |
| `+28/+32` | Primary pair from legacy vertex `+24/+28` |
| `+36/+40` | Tail pair at `tail + 4 + original_vertex_index * 8`, if selected |
| `+36/+40`, without selected tail | Both positive zero |

The selected pair is loaded and stored at `0x1006457f..0x1006458e`; the zero
branch is `0x10064594..0x1006459c`. There is no scaling, V inversion, material
filter or remapping in this copy. The loop uses the TER header's vertex count.

Version 1 does not inspect this tail and expands with zero secondary pairs.
Version 3 or higher uses the already-expanded interleaved records and skips
the legacy conversion. Version 4 was exercised solely to confirm that native
branch condition; it is not a claim of general version 4 compatibility.
The MOD reader is outside this witness. Do not apply this TER tail rule to
MOD files just because the primary layouts share a parser.

Original CPU copy `0x10020970` subsequently copies expanded `+28/+32` to
attribute `+0/+4`, and expanded `+36/+40` to attribute `+8/+12` at
`0x10020a17..0x10020a2c`. Both source pairs remain separate and retain original
vertex association. This does not establish which GPU layouts consume them.

## Executed witness and limits

The bounded witness executes the TER entry, header checks, shared reader's
material/vertex/polygon cursor arithmetic, tail selection, complete legacy
expansion and the original CPU attribute-copy loop. Allocation and `memcmp`
are controlled. After its cursor arithmetic at `0x100625f9`, the shared reader
jumps to its original epilogue, omitting material resolution and registration.
The TER execution stops at `0x100645b9`, before lighting-source selection.
Thus no complete loader, filesystem or GPU execution is claimed.

There are **28 ordinary cases**: versions 0–4 crossed with tags 0, 1, 2, 3
and `0xffffffff`, a special-float case, and two original-asset samples. Those
samples preserve exact source records and tail words from Nest vertices
0/192021/384041 and Thundercrest vertices 0/55098/762814, placed in bounded
three-vertex inputs. They are not full-zone native executions. The witness
asserts cursor placement, complete expanded record bytes and both pairs in
the CPU attribute stream. It uses x87 CW `0x037f` and MXCSR `0x1f80`.

An additional **11 guard-page cases** put the final file byte immediately
before unmapped memory. They establish the actual read behavior:

| Input | Observed original instruction behavior |
| --- | --- |
| v1 or v3, no tail | Reaches end of expansion |
| v2, no tail or one tag byte | Unmapped read at `0x100644a8` |
| v2, complete tag 0 or 3 only | Reaches end of expansion; no pair reads |
| v2, tag 1 or 2 without pairs | Unmapped read at `0x1006457f` |
| v2, tag 1 with one float, or final float missing one byte | Unmapped read at `0x1006458a` |
| v2, tag 1 and all three pairs | Reaches end of expansion |

The native slice has no tail length check or safe truncated-input return
contract. Ordinary allocations may have readable memory beyond the payload;
the guard-page result does not claim every truncated real file crashes.
Preserving OpenEQ's acceptance of a completely absent version 2 tail is a
deliberate compatibility policy for existing synthetic inputs, not a claimed
native fallback. A nonempty partial tag or recognized truncated stream should
produce a checked parser error.

## Installed corpus

An independent archive/byte walk examined all **228 TER payloads** under the
installed client's `.eqg` archives, with no scan errors:

| Version / tail shape | Payloads |
| --- | ---: |
| v1, no tail | 1 |
| v2, exactly four bytes with tag 0 | 17 |
| v2, tag 1 followed by exactly `vertex_count * 8` bytes | 16 |
| v3, no tail after interleaved vertices and polygons | 194 |

No original tag 2 or unknown tail tag was found. Tag 2 support follows the
executed loader branch, not a corpus example. All 16 recognized tails end
exactly after one pair per vertex; none has a residual suffix.

| Archive | TER member | Vertex pairs |
| --- | --- | ---: |
| bazaar | ter_bazaar.ter | 225,781 |
| broodlands | ter_broodlands.ter | 80,359 |
| delvea | ter_volcano.ter | 297,090 |
| delveb | ter_volcano.ter | 268,623 |
| dranikcatacombsa | ter_catacomba.ter | 236,286 |
| dranikcatacombsb | ter_catacombb.ter | 176,386 |
| dranikcatacombsc | ter_catacombc.ter | 359,681 |
| draniksewersa | ter_sewera.ter | 172,876 |
| draniksewersb | ter_sewerb.ter | 210,524 |
| draniksewersc | ter_sewerc.ter | 221,665 |
| harbingers | ter_harbingers.ter | 232,179 |
| lavastorm | ter_lava.ter | 103,728 |
| stillmoona | ter_main.ter | 669,674 |
| stillmoonb | ter_easterntemple.ter | 72,125 |
| thenest | ter_abyss01.ter | 384,042 |
| thundercrest | ter_stormtower01.ter | 762,815 |

Across those streams, 4,430,111 pairs differ by source words from primary UVs.
Bazaar's entire secondary stream is zero, so stream presence does not imply a
visible second texture layer or a nonzero coordinate.

Nest payload SHA-256 is
`238b0f41bd5133de9e9a5f04cc2502f090b120028bf7473b4f247340e3c2a176`.
Its tail starts at byte 19,523,796 and contains 3,072,340 bytes:
`4 + 384042 * 8`. Vertex 0's primary words are `43c31308/c32a8000`; its
secondary words are `43524702/c1f701f2`, or approximately
`[210.2773743, -30.8759499]`. Of 375,477 original vertices referenced by
resolved `_2UV` materials, 375,326 have secondary words different from UV0.
These counts cover source polygon references, not visibility or the subset
previously restored to rendering.

## Nonfinite and bit preservation

There is exactly **one nonfinite component** among the 8,947,668 floats in the
16 streams. Thundercrest vertex 55,098 V is quiet NaN word **`0x7fff0005`** at
file byte `0x1d9dd84`. It is referenced by polygon 19,895, material ordinal
227, resolved shader `AddAlpha_MaxCB1.fx`, flags zero. It is not a `_2UV`
material in this corpus; its source channel must still be retained.

The executed original-word sample preserves that exact quiet-NaN payload
through both legacy expansion and CPU attribute copy. Synthetic signed zero,
infinities, subnormal and NaN words also survive unchanged in this Unicorn
witness. The synthetic signaling-NaN observation is emulator-specific;
physical x87 signaling-NaN conversion behavior is not certified here. A raw
source parser can preserve every word without claiming to emulate an x87
conversion. Do not sanitize, normalize or reuse primary UVs while retaining
this source channel.

## Parser integration

`TerMod::parse` now retains recognized version 2 **TER** tails in the existing
`secondary_tex_coords` field. It checks the full stream length before
allocating the coordinate vector, preserves float words without arithmetic,
and leaves primary coordinates, materials, polygon order and source colors
unchanged. No source tag field or rendering behavior is added.

An absent tail, tag 0 or an unrecognized complete tag leaves the optional
field absent. An explicit recognized empty stream has `Some(empty)`. A
nonempty incomplete tag or recognized truncated stream returns `Truncated`.
Version 1, interleaved version 3 and MOD retain their existing behavior.
Any bytes after a complete recognized stream remain outside this parser's
interpreted fields; they are not treated as another coordinate stream.

Focused synthetic tests cover both accepted tags, every partial tag/stream
prefix, raw special float bits, alignment, empty-versus-absent and version/MOD
scope. The original-assets test checks all 16 streams against independently
frozen offsets, counts and FNV-1a-64 digests, including the exact Thundercrest
quiet-NaN word. Existing vertex-lighting/source-channel tests also pass with
their original-assets cases enabled. Shader binding remains a separate task.

## Reproduction and frozen evidence

```sh
PYTHONPATH=/tmp/openeq-re-tools python3 /tmp/openeq-ter-trailing-native.py
python3 /tmp/openeq-ter-trailing-corpus.py
```

The native JSON retains all expanded and CPU coordinate words, dispatch
results and guard-page fault instructions. The independent corpus JSON keeps
per-payload SHA-256, tail offsets/counts, stream SHA-256 and FNV-1a-64,
nonfinite words and source witnesses. No proprietary payload is copied into
the repository.

| Artifact | SHA-256 |
| --- | --- |
| `/tmp/openeq-ter-trailing-native.py` | `aa6a75856c6d5b25cecd4415676570e04292e9e8c38ee57e5ef35ef77a213993` |
| `/tmp/openeq-ter-trailing-native.json` | `a9cf6843de17517200bc71f957d6728c91c35268c3088de242f706d7389178e3` |
| `/tmp/openeq-ter-trailing-corpus.py` | `c1c3683fdcbc680efd1f5b0c087f620cdea50d7b850eefd3bd27e3c644d89a14` |
| `/tmp/openeq-ter-trailing-corpus.json` | `3fc9fc11432261fcd2d73195ad5938078d9e69bddb3ab8dcc01885248b191a77` |
