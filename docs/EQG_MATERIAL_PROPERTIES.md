# Binary EQG material property words

`TER` and `MOD` materials contain a property count followed by 12-byte records:
string-table key offset, type tag, and a four-byte value. The parser now retains
all four verified tags distinctly:

| On-disk tag | Retained value | Native material storage |
| --- | --- | --- |
| 0 | `Property::Float(f32)` | Runtime kind 1; floating-point load/store |
| 1 | `Property::IntegerBits(u32)` | Runtime kind 2; unchanged 32-bit copy |
| 2 | `Property::Text(String)` | Runtime kind 0; string-table offset resolution |
| 3 | `Property::Uint(u32)` | Runtime kind 3; unchanged 32-bit copy, including colors |

Keeping tag 1 separate avoids treating its payload as a float, string offset,
or tag-3 color. The retained value is deliberately raw bits; this parser change
does not implement shader parameter binding, UV-channel selection, or establish
signed arithmetic semantics. Unverified tags still produce an explicit error.

## Native evidence

The installed `EQGraphicsDX9.dll` has SHA-256
`615e7ba9e20745ec03a908cf47c038cc764358ad820d9d4d3232734fadff5383`.
Addresses below use its preferred image base `0x10000000`.

- TER reader `0x100643b0` recognizes `EQGT` and calls common geometry/material
  processing at `0x10062580` (`0x1006445d`). The common code walks each material
  using `16 + property_count * 12` bytes (`0x100625b3..0x100625b9`).
- It passes the material and its property table to `0x10014450` at `0x1006289d`.
- The material property dispatcher reads the tag, bounds it to 0–3 and branches
  through table `0x10015f1c` at `0x10015387..0x10015394`.
- The four jump-table entries, in tag order, are `0x1001539b`, `0x10015444`,
  `0x10015488`, `0x10015466`.
- Tag 1's branch sets runtime kind 2 (`0x1001544e`), reads the source word
  (`0x10015459`), and copies it unchanged (`0x1001545d`). Tag 3 separately sets
  kind 3 and copies its word at `0x10015466..0x1001547f`.
- The common property loop advances by 12 bytes. No variable-length payload is
  attached to tag 1.

The independent EQEmu zone-utilities reader also defines a 12-byte
`mod_material_property` containing a `uint32_t`/`float` union. Its
`eqg_model_loader.cpp` resolves tag 2 strings, reads tag 0 floats, and preserves
other tags in `value_i`; the native switch above establishes the narrower set
accepted here.

## Original Housegarden regression

The installed `housegarden.eqg` uses the loose `housegarden.zon` declaration
(version 2): 101 model references and 6,267 placements. Its 25 affected meshes
contain 150 tag-1 properties across `Housewall` and `roof` materials using
`Opaque_MaxCB1_2UV.fx`:

| Property | Raw value | Count |
| --- | ---: | ---: |
| `e_TextureDiffuse0mapChannel` | 1 | 50 |
| `e_TextureNormal0mapChannel` | 1 | 50 |
| `e_TextureSecond0mapChannel` | 2 | 50 |

One affected definition (`obj_hut_lod1.mod`) is not referenced by the ZON. The
24 referenced meshes contain 144 words, but `ter_gardens.ter` has 33 material
records with only 23 distinct IDs. The parser's existing last-record-per-ID map
replaces the terrain's six channel words, leaving 138 retained words (46 of each
property). Native material identity for these repeated IDs needs separate
investigation; this change preserves that existing policy. The old parser
stopped at the first tag-1 property. The original-asset regression checks the
138 retained words, the model/placement counts, and successful
creation of drawable and collision geometry. Synthetic TER/MOD fixtures verify
all 32 bits survive, following float/string/color properties and vertices stay
aligned, truncated words fail, and unverified property kinds remain errors.

This resolves the loading failure. The renderer still uses its existing
material interpretation and primary texture coordinates; the channel words
are retained for later native shader work. No proprietary binary assets or
disassembly dumps are committed.
