# WLD particle runtime source families

October 1, 2026. This is a source inventory supporting the native runtime work
in `WLD_PARTICLE_RUNTIME.md`. It does not enable effects or establish actual
resource-registration order, visible placement counts or complete native output.

## Independent source inventory

A raw PFS/WLD reader scanned all **293 installed object S3D archives**, each
containing one object WLD. It found **623 particle-cloud fragments in 119
archives**, with no archive, record or texture-chain error. Counts include
unused definitions and same-name copies, not just active zones/actors.

There are **22 distinct 80-byte fixed bodies**, excluding the trailing full
texture reference. Every record has optional flags 4, selector 3 and mode 3;
none uses optional vector/skipped blocks. Their separate converter bitfield is:

| Body +0x0c | Fragment occurrences |
| --- | ---: |
| `0x10500` | 6 |
| `0x24500` | 46 |
| `0x28500` | 5 |
| `0x30500` | 553 |
| `0x34500` | 11 |
| `0x38500` | 2 |

**409 fixed bodies across 81 archives** exactly match one of PoK's four fixed
bodies. This is a byte identity, not a claim that their texture binding, owner
transform, effective material context or draw output matches PoK. Even among
the 553 records sharing PoK's bitfield, parameters vary. Capacity values are
10/20/30/40/50/80; source lifetimes are 600/650/750/3000/3500 ms; source emission
intervals are 30/60/90/100/120/150/160 ms. Names alone do not identify a body:
for example, CSMOKE has a second parameter set in some archives.

A separate Rust executable using the production PFS/WLD parser compared every
record against the raw oracle. All **623 names, full texture references and
20 fixed words** matched exactly. The raw reader and typed reader traverse the
records independently; the raw inventory does not use OpenEQ's cloud parser.

## Texture metadata

Every full source reference resolves through `0x26 -> 0x05 -> 0x04 -> 0x03`.
Every `0x26` has flags zero and material word `0x80000017`; every `0x04` has
flags `0x18`, one frame and timing word 100. These are full authored references,
not an emulation of the native low-byte resolver or named-resource cache.

| Bitmap filename | Cloud occurrences |
| --- | ---: |
| GENG00.DDS | 316 |
| CSMOKE.DDS | 165 |
| GENG10.DDS | 64 |
| GENPOP00.DDS | 47 |
| GENE01.DDS | 16 |
| GENPOP01.DDS | 9 |
| GENG501.DDS | 2 |
| GENG600.DDS | 2 |
| POPSMOKE.DDS | 1 |
| GENZ20.DDS | 1 |

All **178 references above 255** have an earlier same-name cloud in the same
WLD. This supports investigating named-resource reuse, but does not prove that
it succeeds in an actual loader context. Filters, prior registration failures,
context lifetime and other resource types still matter. The full references
must remain preserved; do not truncate them or replace them with name aliases.

## Bounded converter extension

A temporary extension of the native runtime witness executes initializer
`0x10071c20` and converter `0x1001c410` on one original example of each of the
22 fixed bodies. All return normally. It retains the established fixed-field
transfer and native math tables, with optional blocks absent. As in the first
numeric witness, the controlled texture descriptor has one frame, timing 100
and **material word zero**. This deliberately excludes actual material alias
resolution and GPU behavior, which have a separate active investigation.

The result records all converted descriptor words and source identity. It does
not execute all 22 emitter simulations, prove their visibility, or equate them
to PoK solely because the converter returns. Only the original four PoK runtime
witnesses currently establish the documented spawn/motion/color behavior.

## Temporary reproduction artifacts

Original assets remain outside git. Local investigation artifacts:

| File | SHA-256 |
| --- | --- |
| `/tmp/openeq-particle-runtime-corpus.py` | `0a53c86f950faa3b03faf3c782f32813270541097087e2516f45165ca9e6d35b` |
| `/tmp/openeq-particle-runtime-corpus.json` | `e5f3c0caa98edebe7217c7f81f90ba361ba3f36f836c349110cb5b93d29bc854` |
| `/tmp/openeq-particle-runtime-corpus.rs` | `b835b1672bc8f4af9299ae2ec174cc926c1c45bb79a161f0ef4540dff9fc99dc` |
| `/tmp/openeq-particle-runtime-corpus.tsv` | `3e3895fbe9e6bad40eccf38b6377060be1644eecc669ddd0961b6ef89586b452` |
| `/tmp/openeq-particle-descriptor-corpus.py` | `f51e781eb9b57de609400ee91b56b826700bdf0e09dcc1037692a4c341f65d9e` |
| `/tmp/openeq-particle-descriptor-corpus.json` | `0d58cb112c7e4d5acd1d517c71024ef82fd1610a8d4ff119e8376f8ad73ba9ec` |

The Python archive helper is `/tmp/openeq-group-region-audit.py`; the native
extension runs with `PYTHONPATH=/tmp/openeq-re-tools` and imports the frozen
particle runtime witness. No original process, graphics device or audio output
was started for this inventory.

## Typed texture binding prerequisite

The WLD parser now retains `0x26` as `ParticleTexture`: raw flags, full signed
child reference, raw material word and any remaining record bytes. It does not
resolve caches, aliases or particle playback. Unknown flags/tails are preserved
as metadata, not declared compatible effect formats. The diagnostic scanner
prints the new fields. Existing opaque-reference tests now use a still-unknown
fragment kind instead of an intentionally short `0x26` record.

A broader raw audit covers **1,806 WLD members and 839 `0x26` records** across
all installed S3D archives. All 839 bodies are 12 bytes with flags zero. The
production parser's 1,804 readable members retain every one of those 839 records
with exact flags/reference/material words and empty tails. The three known PFS
count-mismatch archives contain no additional `0x26` records in the permissive
raw survey. Temporary evidence: `/tmp/openeq-particle-texture-layout.{py,json,log}`
and `/tmp/openeq-particle-texture-typed.{rs,tsv}`.

All **304 assets tests pass**, including the original assets. New tests cover
every truncation before the fixed prefix, signed extremes, opaque extensions,
neighboring-record isolation and all eight original PoK texture chains.
Independent code review and strict workspace lint pass; logs use
`/tmp/openeq-particle-texture-*`. No renderer behavior changes in this prerequisite.
