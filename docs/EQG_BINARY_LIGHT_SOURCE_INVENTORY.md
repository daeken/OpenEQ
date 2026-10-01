# Installed binary EQG light source inventory

A read-only scan of the installed `/Users/daeken/EverQuest` tree on
2026-10-01 found **one black RGB light and no zero, negative or nonfinite
radii, negative or nonfinite RGB, or nonfinite positions** in its binary
EQGZ declarations. The black source is not eligible under the native
ordinary-terrain name test. These are authored-data findings, not evidence
that a singular score occurs in a live scene.

This contextualizes [the singular selection witness](EQG_TER_SINGULAR_LIGHT_SELECTION.md)
and [the ordinary terrain light-list contract](EQG_TER_LIGHT_LISTS.md).
No production code changed for this inventory.

## Coverage and counts

The scan enumerates every `.eqg` archive and loose `.zon` file recursively,
case-insensitively by extension. It inspects every archived `.zon` member,
including internal aliases; it does not choose a declaration using client
lookup precedence. All **1,933 archives and 190 loose files** were read
without an archive or declaration parse error.

There are 283 physical declarations: 223 binary EQGZ and 60 heightmap-text
declarations. The binary subset consists of 36 archived members and 187
loose files, with 34 version-1 and 189 version-2 declarations. Heightmap
text is recognized and excluded from numeric light analysis.

| Binary source measure | Physical declarations | Distinct declaration payloads |
| --- | ---: | ---: |
| Declarations | 223 | 213 |
| Light records | 27,432 | 27,187 |
| Third name byte `B` or `b` | 1,706 | 1,671 |
| Zero radius | 0 | 0 |
| Negative radius | 0 | 0 |
| Nonfinite radius | 0 | 0 |
| All three RGB channels zero | 1 | 1 |
| Any negative RGB channel | 0 | 0 |
| Any nonfinite RGB channel | 0 | 0 |
| Any nonfinite position coordinate | 0 | 0 |

Distinct payloads are deduplicated by SHA-256 of the complete declaration,
not by zone name or light name. Physical counts include identical archived
or loose copies. Both signed zeros count as zero; a negative channel means
a numerical value below zero. All seven anomaly counts are zero within
the subset whose third original name byte is `B` or `b`.

## The black source

The sole black source is in loose `eastsepulcher.zon`:

| Field | Value |
| --- | --- |
| Declaration SHA-256 | `eebb186a88f1e1300507eddb5d21fab5c71be83fc81f80bb09337ad5f85a19f0` |
| Declaration version / light count | 2 / 679 |
| Zero-based light ordinal / byte offset | 6 / 2,218,449 |
| Name | `LIT_light_black_mesa` |
| Source position, before coordinate conversion | `(1810.8673095703125, 845.587890625, -21.734901428222656)` |
| RGB words | `00000000 00000000 00000000` |
| Radius / word | `50.0` / `42480000` |
| Ordinary-terrain name eligibility | False: third byte is `T` |

The original ordinary-terrain selector excludes that name unless a
different receiver policy or creation path overrides its default. This
inventory executes neither path and makes no claim about another use of
the source light.

## Parsing and independent check

The main probe validates PFS directory lengths and decompressed block
lengths, then parses the binary header, string table, model references,
variable version-2 placement lighting, regions and 32-byte light records.
It requires each binary declaration's light table to end exactly at EOF.
Each declaration retains source provenance, byte length, SHA-256, counts
and anomalous records with the original numeric words.

A separate check enumerates the same installation with the retained PFS
reader from `openeq-group-region-audit.py`. It locates each light table
backwards from EOF using the header count, rather than walking the main
probe's preceding records. It classifies IEEE-754 words directly rather
than using floating-point comparisons. All declaration hashes, light
counts, eligibility counts and anomaly classifications agree. A replay
of the main probe also reproduced the result byte-for-byte.

```sh
python3 /tmp/openeq-binary-light-corpus.py --output /tmp/openeq-binary-light-corpus-replay.json
python3 /tmp/openeq-binary-light-corpus-check.py
```

| Artifact | SHA-256 |
| --- | --- |
| `/tmp/openeq-binary-light-corpus.py` | `60040979a71502942b21e4684deb5103cec2e42f9fe3fbbd2e02eccbb0e101c3` |
| `/tmp/openeq-binary-light-corpus.json` | `b82224771a9a313ccdd1962a4f8d9bfe700536377108cc61d56bb49ede558fac` |
| `/tmp/openeq-binary-light-corpus-check.py` | `c3f3e5a97ac4b457d5dd45807f4bb95610a724f348245af3d9492cff657cc9a5` |
| `/tmp/openeq-binary-light-corpus-check.json` | `464c6ce2e1726d2bccdee86065c9de1ea17744db255d4f45a4bdda9b45bbca15` |
| Required check helper `/tmp/openeq-group-region-audit.py` | `79781a51d3240bb1911833369227528cb8b9cac874b14db6767ef2ed77d5a627` |

## Limits

This covers binary `.zon` declarations in this installed tree only. It
does not cover WLD lights, heightmap lights, runtime-created lights,
other client installations, receiver-center geometry, DPVS membership or
event ordering. Strictly positive finite source attributes can still
produce a singular score at exact receiver-center coincidence; this scan
does not test that coincidence or all arithmetic overflow/underflow.
The native singular behavior remains a controlled execution result, not
a demonstrated live-zone occurrence.
