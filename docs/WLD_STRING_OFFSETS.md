# WLD source bytes and UTF-8 offsets

Separate follow-up identified during signed-reference validation, 2026-10-01.
**No string-table decoding change is included in the resolver repair.** The
current decoder XORs each source byte and casts it to a Rust `char`, collecting
the result into a UTF-8 `String`. A decoded byte at least `0x80` occupies two
bytes in that String, while a WLD string reference still counts source bytes.
Applying a later source offset directly to the expanded String can select the
wrong text or a non-character boundary.

This mechanism is proven with a synthetic fixture. An actual shifted referenced
name was **not found** in the installed archive audit below; the observed original
high bytes are confined to trailing padding. Do not describe this follow-up as
an established cause of missing original actors or textures.

## Original Citymist witness

`citymist_obj.s3d` / `citymist_obj.wld` contains 402,316 uncompressed bytes.
The WLD SHA-256 is
`373cac216595b1e9d948f07d6d578e9625b8c766356777b08b5f804a3d426d68`.
Its header declares a string table of **11,156 source bytes**, beginning at
file byte 28. The existing decoder produces a **11,157-byte UTF-8 String**.

| Location | Exact observation |
| --- | --- |
| Source table byte 11,154 / file byte 11,182 | Stored byte `00`, XOR key byte `C5`, decoded byte `C5` |
| Source table byte 11,155 / file byte 11,183 | Stored byte `00`, XOR key byte `2A`, decoded byte `2A` |
| Current UTF-8 table bytes 11,154–11,156 | `C3 85 2A`, because casting `C5` to `char` encodes U+00C5 in two bytes |
| Last named fragment | One-based fragment 1367, kind `0x14`, `URN5_ACTORDEF` |
| Its header name field | File byte 402,268, signed reference `-11140` |
| Its source name | Starts at table byte 11,140; terminating NUL at 11,153 |

Thus both high-byte expansion and the final actor name were located
independently in the raw decompressed bytes. The high byte is **after** that
name, so this actor is unaffected. It explains why a test using the source
table length as an invalid String offset was mistaken: offset 11,156 is outside
the source table but addresses `*` inside the expanded String. The separate
resolver test now chooses a boundary outside either representation; it does
not change decoding or claim that this padding is a real named resource.

## Installed archive audit

A temporary independent PFS/WLD reader scanned **1,314 local S3D archives** and
**1,806 WLD members**. Counts are per member occurrence, not deduplicated asset
hashes. It decoded the XOR table as raw bytes, reconstructed the current UTF-8
expansion separately, and compared nonempty names addressed by negative
fragment-header and skeleton-track name values. Other typed body references
were not exhaustively enumerated.

- **931 tables contained high bytes.** In every case, all high bytes were
  confined to the final three source bytes; no table had more than two.
- **Zero compared names shifted.** No affected original fragment-header or
  skeleton-track name could be supplied from this corpus.
- PoK's object WLD has 26,840 source table bytes, no high bytes and no expansion.
  Its SHA-256 is
  `e7fbed560a7b81bbfe7495440418de0020f98ef84f63b3f5e4850fcfc2af10e3`.

The audit includes negative header fields on otherwise unmodeled fragment
types; it does not infer new semantics for those types. It compares source
byte slices with the current expansion rather than using OpenEQ's name lookup
as its oracle. No original archive was modified and no runtime was launched.

Temporary artifacts:

| File | SHA-256 |
| --- | --- |
| `/tmp/openeq-wld-string-offset-all.py` | `2a58fe50f6b3e6a821cd5aebb60bd0a3f3d5dac705fc93e9a9376b2d7b7bf3e7` |
| `/tmp/openeq-wld-string-offset-all.json` | `8e2557bb0328fa87cf68ddf31c887ff142f11394aa7c6ab8ff0831a7be49e0e2` |

The reader uses only PFS decompression from
`/tmp/openeq-group-region-audit.py`. Its log is
`/tmp/openeq-wld-string-offset-all.log`.

## Portable counterexample and later fix boundary

This decoded source string table is sufficient to demonstrate the mechanism:

```text
Source bytes:       00 E9 00 54 45 58 00
Source offsets:      0  1  2  3  4  5  6
Current UTF-8:      00 C3 A9 00 54 45 58 00
```

An otherwise valid fragment with name reference `-3` should obtain ASCII `TEX`
from the raw source bytes. The current expanded String instead has a NUL at
byte 3, so the parsed fragment name is empty and the repaired resolver returns
`None` for the negative name. A temporary executable using the actual assets
library confirmed those outputs. Its source is
`/tmp/openeq-wld-string-expansion-review.rs`; this is a synthetic witness, not an
original asset or an assumption about which code page a native high byte uses.

A later change should keep source-byte addressing separate from display-string
encoding: for example, retain the XOR-decoded byte table, select the referenced
byte range first, then convert that range to a string. Alternatively, retain an
explicit source-byte-to-String-offset mapping. Do not merely relax UTF-8
boundary checks or reinterpret source offsets as character indices.

Test a high byte before an ASCII name, a high byte within a name, multiple
high bytes, valid nonempty substring references, empty/invalid offsets and
same-name duplicates. Establish the desired high-byte text conversion policy
separately while preserving ASCII originals. Re-run the original-name audit
before attributing any visible change to this mapping fix.
