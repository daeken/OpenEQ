# TER lighting source selection and packed-vertex association

2026-10-01. This freezes the next bounded contract after
[raw channel retention](EQG_TER_VERTEX_CHANNELS.md): select the native lighting
stream for ordinary TER versions 1–3 in the exact `Opaque_MaxCB1.fx` family,
record its provenance, and preserve its original vertex association through
material packing. It does not establish a new renderer equation or visible
lighting parity. Shader programs, native normal/tangent arithmetic, runtime
light binding and color-space behavior remain separate boundaries.

Native addresses below refer to the installed `EQGraphicsDX9.dll`, image base
`0x10000000`, SHA-256
`615e7ba9e20745ec03a908cf47c038cc764358ad820d9d4d3232734fadff5383`.
The selection witness executes original instructions with synthetic file and
allocation interfaces. The original-asset corpus scan independently parses
data and reproduces packing keys; it does not execute the original client.

## Source selection is independent of the scene declaration

The zone loader clears global embedded pointer `0x1017c200` at `0x1006676f`.
At `0x100668c2..0x10066953` it independently opens the **loose sibling named
after the outer archive, with extension `.ZON`**, through the CRT file API and
passes its bytes to `0x100649f0`. This happens independently of whichever
archive or loose ZON the scene declaration parser uses. In particular, an
archived scene declaration can win OpenEQ's declaration lookup while a loose
outer-name ZON still supplies native terrain lighting.

The helper recognizes `EQGZ` version **exactly 2**. Starting after the 28-byte
header, string block and model-name-offset table, it takes the **first
placement's** count at +36 and stream at +40. It neither searches for a TER
object nor validates the placement's object ID/name before installing this
stream. The only stores to the global pointer/count are `0x10064a6c` and
`0x10064a76`. The archived ZON parser does not install this pointer.

This refines the earlier channel note's wording “terrain placement”: the
native helper unconditionally uses the first placement. The original Guild
Hall, Guild Lobby and Roost files happen to place their terrain first, but
that corpus property is not the selection rule.

| Condition | Selected source / result | Retry behavior |
| --- | --- | --- |
| Loose outer-name EQGZ v2 exists | First placement stream | LIT is never opened |
| Selected embedded count equals TER vertex count | Use all embedded words | None |
| Selected embedded count differs, including zero for a nonempty TER | Use `0x001f1f1f` for every vertex | No LIT retry |
| No eligible loose embedded source; TER came from archive | Same TER stem `.LIT` in that archive | No loose-file retry |
| No eligible loose embedded source; TER came from loose file | Same TER stem `.LIT` in loose-file context | No archive retry |
| Selected LIT has EQGP magic and matching count | Use all LIT words | None |
| Selected LIT is missing, has unrecognized complete magic, or has a different count | Use `0x001f1f1f` for every vertex | No other-source retry |

TER reader `0x100645b9` tests the global pointer. A nonnull pointer jumps
directly to the count comparison at `0x10064784`; the pointer remains nonnull
when the embedded count is zero. A mismatch logs and clears the final lighting
pointer rather than returning to source selection. Without an embedded
pointer, `0x10064610` tests the archive handle. The archive branch is
`0x10064618..0x100646d3`; the loose-file branch is `0x100646da..0x10064781`.
An archive miss does not enter the latter branch. Native member parsing removes
the four-character TER extension before appending `.LIT`.

The source-selection instruction witness covers nine cases: embedded matching,
mismatched and empty streams with a valid fallback LIT available; archive LIT
matching, mismatched, unrecognized-magic and missing; and loose LIT matching
and missing. It checks the selected words `11223344`, `aabbccdd`, `55667788`,
null results for defaults, no file opens under an installed embedded pointer,
and archive/loose exclusivity. Allocation, file open/read/close and mismatch
formatting/logging are stubbed. The executed block is
`0x100645b9..0x100647b6`; this is not a full native filesystem or client run.

## Bounded parser policy

The implementation keeps absence, complete unrecognized magic and count
mismatch as observable default-lighting outcomes, with the attempted source
and reason retained. A successful selection always has exactly the TER vertex
count in original vertex order. A mismatch discards the **whole** supplied
stream; partially zipping or truncating it would change the native result.

Malformed-input safety is intentionally separate from native instruction
behavior. Required headers and byte ranges are checked, counts use the shared
parser's plausibility limit, length multiplication is checked, and the full
color range is verified before allocation. Truncated required input,
implausible counts and non-NotFound I/O failures return errors. A v2 ZON with
no first placement also errors instead of reproducing the native unchecked
read. An unrelated suffix or later placement is not parsed once the first
required stream is complete. A complete non-EQGZ header or EQGZ version other
than 2 falls through to LIT selection.

The scene loader records selector errors in `ter_lighting_issues` and retains
the valid TER's ordinary geometry bake without a lighting sidecar. It does not
invent a native default for malformed auxiliary input or fail the zone solely
because this newly retained channel is malformed. The geometry audit exposes
the issues and the number of meshes with selected lighting.

Loose-file lookup uses the loader's existing case-insensitive resolution.
Provenance records the path used to read a loose source or the archive member
name used for LIT lookup. Filesystems can resolve an existing pathname with
different casing; path spelling alone is not a distinct source identity.

## Original source inventory

The independent corpus covers 24 TER payloads, 278 resolved material records,
and 980 source material-ordinal groups. The latter matches the loader's
per-ordinal grouping, including repeated material names resolved to the first
matching record. It contains 2,678,538 unique selected original vertices when
counted once per payload, and 2,688,429 original vertex uses when counted once
per material group.

Source outcomes are **18 matching archive LITs, three mismatched archive LITs
using defaults, and three loose embedded streams**. Every one of the 21
payloads without eligible loose embedded lighting has a matching-stem LIT;
the three embedded cases have no matching-stem LIT.

| Mismatched source | TER vertex count | LIT count | Result |
| --- | ---: | ---: | --- |
| `dranikhollowsa / ter_cavea.ter` | 131,590 | 89,819 | Entire stream defaults |
| `dranikhollowsb / ter_caveb.ter` | 98,382 | 85,582 | Entire stream defaults |
| `dranikhollowsc / ter_cavec.ter` | 101,791 | 77,873 | Entire stream defaults |

| Loose ZON | First placement ID / name / model | Stream offset | Count = TER vertices |
| --- | --- | --- | ---: |
| `guildhall.zon` | 0 / `TER_GuildHall` / `TER_GuildHall.TER` | `0x0c4b` | 28,584 |
| `guildlobby.zon` | 0 / `TER_GuildLobby` / `TER_GuildLobby.TER` | `0x241c` | 57,912 |
| `roost.zon` | 0 / `TER_roost` / `TER_roost.TER` | `0xa536` | 73,847 |

Per-payload TER/LIT hashes, source counts, original-index witnesses and group
totals are retained in the corpus JSON. Across the selected material groups,
1,739,340 original vertex uses have lighting alpha zero and 949,089 have
nonzero alpha. These counts include the three defaulted streams.

## Packing must retain lighting in the vertex identity

The native CPU-copy loop reads `lighting[original_vertex_index]` at
`0x10020a33`. Its attribute +16 is the selected lighting word; attribute +20
holds the separate expanded TER source color. Layout 1 copies **+16 unchanged**
to GPU COLOR0, while region remapping copies all nine attribute dwords at
`0x1001f02f`. Reading the original stream with a packed mesh index is wrong.

The existing packing identity comprises eight raw words: position XYZ,
normal XYZ and UV0 XY. Preserve those words' exact bit identity and add the
**entire selected lighting word** to the key. Keep a representative original
vertex index for each resulting packed vertex. Looking up the selected stream
with that retained index then reproduces the color for every source corner,
including when two source vertices share geometry and lighting.

Retaining an index alone is insufficient: otherwise vertices with different
lighting have already merged. In this corpus, adding lighting changes packed
vertices from 1,966,221 to 1,977,104, requiring **10,883 additional vertices**.
There are 9,794 old geometry keys with conflicting lighting; a key can contain
more than two lighting values, so the number of extra vertices is larger.

| Zone | Geometry-only packed count | Geometry + lighting packed count | Conflicting old keys |
| --- | ---: | ---: | ---: |
| `delveb` | 92,007 | 92,068 | 59 |
| `guildlobby` | 5,255 | 6,086 | 642 |
| `thundercrest` | 249,914 | 259,905 | 9,093 |

No other selected corpus payload changes its packed vertex count. Concrete
conflicts have identical position, normal and UV0 bit patterns:

| Zone / source material ordinal | Old packed index | First original index / lighting | Other original index / lighting |
| --- | ---: | --- | --- |
| `delveb / 334` | 215 | 31,377 / `00312f2b` | 32,016 / `005e513f` |
| `guildlobby / 4` | 13 | 7,421 / `006e6c65` | 15,576 / `006e6d65` |
| `thundercrest / 60` | 29 | 45,523 / `41000000` | 45,624 / `47000000` |

The Thundercrest witness differs **only in lighting alpha**. Dropping alpha
from the identity would still merge distinct inputs.

Causeway material 6 supplies stable mapping witnesses because it has no
lighting conflicts:

| Original index | Packed index | Selected lighting |
| ---: | ---: | --- |
| 87,375 | 0 | `001b1202` |
| 87,468 | 17 | `4c0d0901` |
| 96,771 | 928 | `00000000` |
| 103,109 | 1,140 | `e5000000` |

Guild Hall material 21 original vertex 16,302 maps to packed index 12 and
selects `32a6a6a6`, although its TER-stored color is `ff232727`. Roost
material 19 original vertex 72,880 maps to packed index 0 and selects
`00020202`, although its TER-stored color is `ff808080`. Guild Lobby packed
indices must be recalculated after the required splits.

This identity preserves the currently retained geometry and selected lighting
only. Future per-source tangent or other channel integration must extend the
identity if that channel differs between otherwise equal vertices. A single
representative original index does not establish equivalence for channels
that have not participated in deduplication.

## Lighting meaning and implementation boundary

The established D3DCOLOR declaration maps `0xAARRGGBB` to normalized RGBA;
it requests no vertex-input sRGB conversion. In the decoded SPL RegionCB1
program, baked RGB contributes directly, while alpha weights ambient, bounce
and directional lighting. Special ambient and vertex point-light terms are
not multiplied by this alpha. Output alpha is explicitly set to 1 at shader
offset `0x3448`. Thus input alpha is a **lighting weight**, not opacity.

The multipass family has separate scaling and clamping behavior. The decoded
programs do not settle runtime technique selection, light constants, precise
normal/tangent arithmetic and FPU state, interpolation, clamping or final
color-space behavior. Neither multiplying all current lighting by the input
alpha nor using TER color as surface opacity follows from these findings.

The bounded implementation selects and records CPU metadata, preserves
original source indices and prevents merges that discard distinct lighting.
It leaves renderer/shader behavior unchanged. Source-selection regressions
cover first-placement precedence, empty/mismatch no-retry behavior, independent
archive/loose contexts, missing/bad-magic/default outcomes, every required
truncated prefix, bounds, and original Causeway/Dranik Hollows/embedded sources.
The separate original remapping fixture verifies source geometry and selected
lighting at every material corner, including the three split-count witnesses.

## Frozen temporary evidence

These probes read original assets in place; no original binary, asset payload
or disassembly is committed. Run the native selection witness with
`PYTHONPATH=/tmp/openeq-re-tools python3 /tmp/openeq-ter-lighting-selection.py`.
The corpus scan uses the same helper path and
`/tmp/openeq-ter-lighting-contract.py`. Temporary paths are local evidence,
not permanent downloadable fixtures; hashes identify the exact artifacts
used for this note.

| Artifact | SHA-256 |
| --- | --- |
| `/tmp/openeq-ter-lighting-selection.py` | `472ce7795efd881acfd391a4db394c32454296570be598c46772c690ce67a9bf` |
| `/tmp/openeq-ter-lighting-selection.json` | `44831db88554d56f63c9939f3e59ad1d4c6a72ab4210c1a9d53d65dbe6cf06d3` |
| `/tmp/openeq-ter-lighting-selection.log` | `44831db88554d56f63c9939f3e59ad1d4c6a72ab4210c1a9d53d65dbe6cf06d3` |
| `/tmp/openeq-ter-lighting-contract.py` | `65df095c949c42649b74b6a6e994920481aedeb37e33085d501d534a5a756950` |
| `/tmp/openeq-ter-lighting-contract.json` | `1e52cae25ac70d8a9e7fe40cdbd2721b2f2244eb454068d3d75d20cf5dce9680` |
| `/tmp/openeq-ter-lighting-contract.log` | `fe7abb2a91d54c08ea9b7c3173f5d7ab27c56bae01580affb3031b49fcd7ba4e` |

## Integrated verification checkpoint

`Scene::native_ter_lighting` now keeps per-mesh representative source indices
and a shared selected original-index stream. The selected full word joins the
raw eight attribute words during packing; alpha-only differences stay distinct.
Malformed auxiliary lighting is recorded in `Scene::ter_lighting_issues` and
exported by the zone audit, without hiding valid geometry or manufacturing a
native default for malformed bytes. The two existing mesh-filtering diagnostics
clear the sidecar when replacing mesh indices.

All **329 assets tests pass**, none failed/ignored, including original source
selection and every triangle corner of Causeway, Delveb, Guild Hall, Guild
Lobby, Roost and Thundercrest. The latter reproduces the independently audited
10,883 added packed vertices with unchanged corner attributes and order.
A malformed loose-ZON fixture retains its terrain and reports its missing
lighting channel explicitly. Evidence: `/tmp/openeq-ter-lighting-assets-verified.log`
and `/tmp/openeq-ter-lighting-remap-final.log`.

The fixed-camera original Bazaar, The Nest and Thundercrest GPU audit compares
new packing to the former geometry-only packing and gets **identical pixels**.
This metadata step makes no lighting appearance claim. GPU evidence:
`/tmp/openeq-ter-lighting-gpu.log`. Strict workspace lint passes in
`/tmp/openeq-ter-lighting-clippy.log`; independent integration review cleared
source alignment, admission, packing, error retention and diagnostics. Root
reran the nine-case native selector and reproduced its frozen result hash.

An earlier original Guild Lobby UV fixture compared old packed arrays and
therefore failed at the intentional lighting splits. Its revised check still
compares every triangle-corner attribute bit, rather than requiring vertices
with distinct lighting to merge. A separate initial selector test assumed
canonical pathname casing; it now checks actual file identity. Both corrected
fixtures pass in the complete assets run above.
