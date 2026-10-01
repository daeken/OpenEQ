# WLD packed track scale: unsigned decode and corpus audit

Audit date: 2026-10-01. Packed fragment `0x12` scale is an **unsigned 16-bit
word divided by 256**. The native decoder zero-extends this word while
sign-extending the seven preceding quaternion/translation words. The parser
now follows that distinction. This changes neither floating-point track
layout nor quaternion, translation, timing, hierarchy, or animation selection.

## Independent native verification

The installed `EQGraphicsDX9.dll` is 1,615,360 bytes, preferred image base
`0x10000000`, SHA-256
`615e7ba9e20745ec03a908cf47c038cc764358ad820d9d4d3232734fadff5383`.
This investigation only disassembled the binary; it did not execute it.

The fragment `0x13` handler at `0x1001ae20` resolves a track definition,
reads its frame count at `0x1001af3f`, and requires packed flag `0x08` at
`0x1001af45`. In its frame loop:

| Address | Instruction / effect |
| --- | --- |
| `0x1001af90` through `0x1001b015` | Seven `movsx` reads for signed quaternion and translation words |
| `0x1001b02a` | `movzx esi, word ptr [eax-6]` (`0f b7 70 fa`): unsigned scale |
| `0x1001b035` | Integer-to-float conversion of that zero-extended value |
| `0x1001b039` | Multiply by `1/256` |
| `0x1001b03b` | Store to decoded frame offset `+0x1c` |

Source frames advance by 16 bytes; decoded frames advance by 32 bytes. The
scale word is the eighth source word. Constant `0x1013572c` contains bytes
`00 00 80 3b`, exactly float32 `0.00390625`. Both the opcode and constant were
independently read from the PE bytes, rather than inferred from a type name.
The separate quaternion-sign/output-matrix boundary remains documented in
[WLD_OBJECT_ANIMATION.md](WLD_OBJECT_ANIMATION.md).

## Installed original corpus

An independent Python PFS/WLD reader scanned every `.s3d` and `.eqg` archive
under `/Users/daeken/EverQuest`. It decompressed only WLD members, followed the
declared fragment boundaries, and examined the eighth word of every packed
frame directly. It did not use Rust's scale decoder, name-based track lookup,
or the character loader to select records.

| Measure | Result |
| --- | ---: |
| Archives scanned | 3,247: 1,314 S3D and 1,933 EQG |
| Archives containing WLD members | 1,305 |
| WLD members | 1,806, all in S3D archives |
| Legacy / newer WLD versions | 1,782 / 24 |
| Fragments | 6,353,405 |
| Track definitions | 1,662,414, all packed |
| Packed frames | 27,118,476 |
| Distinct packed scale words | 655 |
| High-bit frames / definitions | 63 / 3 |
| Archive or WLD scan errors | 0 |

All 63 affected frames contain **`0xd579` (54,649)**. Native decoding yields
**213.47265625**; signed decoding yielded **-42.52734375**. No zero scale words
were found. All words below `0x8000` retain exactly the same decoded values.
The object-archive subset contains 216,511 packed frames and no high-bit scale
words; this correction does not alter the existing placed-object fixtures.

The three affected definitions have identical payload bytes and 21 frames
apiece. The definition name is `WRWWRW_TRACKDEF`, flags `0x08`:

| Archive / WLD member | Definition fragment | Reference fragment | First scale byte offset in WLD |
| --- | ---: | ---: | ---: |
| `potactics_chr.s3d` / `potactics_chr.wld` | 29,430 | 29,431 | 8,668,878 |
| `potimeb_chr.s3d` / `potimeb_chr.wld` | 43,556 | 43,557 | 12,873,530 |
| `wrw_chr.s3d` / `wrw_chr.wld` | 2,087 | 2,088 | 715,750 |

Fragment numbers are one-based; frame indices and byte offsets are zero-based.
Every subsequent scale in each definition is 16 bytes after the preceding
one. The payload SHA-256, covering flags, count, and all 21 packed frames but
excluding the name reference, is
`0bdf5eef3d43a82058caf8ca83607350ed52c4e479db059748b1ac5309dd83d0`.

Uncompressed WLD SHA-256 values:

- `potactics_chr.wld`:
  `20c395da7896cfbc504e86929b1f83a3cf3595ace1a1c73ed4f4ee4bd167bda7`
- `potimeb_chr.wld`:
  `bc0d2ff475b50d659f6214e204d625e91ac428f86555e4ea69469b3e59372ba2`
- `wrw_chr.wld`:
  `f4914b0448a35c2dfa5d3cc969d508eeb7f8d7cdb982f1cc0e3485be3a8b39fe`

## Reachability and effect boundary

Each affected definition is the second of two **same-name** definitions; the
earlier one begins two fragments before it and has ordinary scale 1. Each has
its own following `WRWWRW_TRACK` reference, with flags 1 and timing word 33.
A name-only search returning the first match therefore misses this witness.

The affected reference is not used directly by a skeleton's base-track field
in any of the three WLDs. `CharacterLibrary` also retains the first same-name
track reference in its lookup table. These facts prevent claiming that this
parser correction visibly repairs the installed WRW character or changes a
particular runtime animation. Native duplicate-name/clip selection was not
established by this bounded audit. The supported claim is the correct numeric
decode of every packed scale, including original high-bit source records.

## Regression and reproduction

`crates/openeq-assets/tests/wld_scale.rs` exercises a seven-frame synthetic WLD
across `0x7fff`, `0x8000`, `0x8001`, the original `0xd579` witness, and `0xffff`,
with zero and unit controls. Signed high-bit quaternion/translation components
retain their signs. A separate floating-point frame preserves negative scale
and its existing field layout. The synthetic test failed before the fix at
`0x8000` (`-128` instead of `128`).

The ignored original-asset regression selects all three affected definitions
by fragment number and verifies all 63 frame scales, signed first-frame
components, and the neighboring same-name control. It reads installed archives
at test time; no original asset bytes or captures were added to the repository.

Focused checks:

```sh
CARGO_INCREMENTAL=0 cargo test -p openeq-assets --test wld_scale -- --include-ignored
CARGO_INCREMENTAL=0 cargo test -p openeq-assets --test characters wld_animation_decodes_packed_and_float_transforms
```

Local audit evidence is retained as `/tmp/openeq-wld-scale-audit.py`, `.json`,
and `.log`; the script uses the existing raw PFS helper
`/tmp/openeq-group-region-audit.py`. The JSON records per-member identities,
counts, the full scale-word histogram, and every high-bit frame offset.
`/tmp/openeq-wld-scale-witnesses.py` and `.json` record duplicate definitions
and reference checks. These temporary files contain metadata, not extracted
original WLD assets.
