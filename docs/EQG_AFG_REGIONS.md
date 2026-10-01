# Native AFG region boxes

Static native trace and bounded CPU support, 2026-10-01. `AFG` regions use the
ordinary oriented-box containment path after replacing their two horizontal
half-extents with the **larger signed value**. Center, vertical extent,
orientation, name and raw type are unchanged. This applies to binary EQGZ and
top-level heightmap DAT regions through their shared native constructor.

The previous whole-set AFG rejection is replaced by this recovered transform.
This enables Arelis's authored liquid set and accepts Pohealth's dry-only
region metadata. It does not implement AFG fog rendering, fog transitions,
audio behavior or region-trigger side effects.

## Native evidence and complete path

The installed `EQGraphicsDX9.dll` is PE32 at preferred base `0x10000000`,
1,615,360 bytes, SHA-256
`615e7ba9e20745ec03a908cf47c038cc764358ad820d9d4d3232734fadff5383`.
The addresses below are virtual addresses in that file. `.text` starts at
VA `0x10001000`, file offset `0x400`; `.rdata` starts at VA `0x10133000`,
file offset `0x131600`. No native code was executed.

The reader and active manager traces are described in
`EQGZ_NATIVE_REGIONS.md` and `EQG_LIQUID_TRANSFORMS.md`. The active factory is
`0x10022230`, which constructs the derived 0xf8-byte object through
`0x10022050`; that forwards all original constructor arguments to
`0x10021de0` at `0x100220b9`.

Within `0x10021de0`:

1. `0x10021e14..0x10021e38` compares the first three name bytes against
   `AFG` at `0x1013595c`. The comparison is case-sensitive.
2. `0x10021e3a..0x10021e50` loads the supplied half-extents into local XYZ
   slots. After the final store, the x87 stack contains `[Y, X]`.
3. `0x10021e52` compares Y with X. `fnstsw ax; test ah, 5; jp` chooses
   the Y-copy branch when `Y >= X`. Otherwise it chooses the X-copy branch.
4. `0x10021e5b..0x10021e5d` drops Y and stores X in the local Y slot;
   `0x10021e63..0x10021e65` drops X and stores Y in the local X slot.
   The local Z slot is untouched.
5. `0x10021e69..0x10021e82` passes that local extent vector to the same
   builder, `0x10021c50`, called at `0x10021e9f`. It does not replace the
   center or orientation vectors.

For a concrete stack check, let B be ESP after the prologue's local allocation
and saved ESI. Original argument slots are center `B+0x24`, extents `B+0x28`,
angles `B+0x2c`, name `B+0x30`, and type `B+0x34`. The AFG path pushes type,
name, angles, the local vector at `B+8`, then the original center. The ordinary
branch pushes the same values except that it passes the original extents.
Thus extent normalization precedes rotation; it is not a square in world axes.

The rest of the constructor was also checked. Its only later AFG-specific
branch, `0x10021ff5..0x10022012`, writes color `0xffc6930a` to `this+0x60`.
That color is consumed by the manager's optional debug drawing, for example
at `0x100215b3..0x100215c9`. It does not change the inverse transform.
The derived constructor changes its destructor vtable and initializes
display data at `this+0x6c` and above; it does not rewrite the inverse matrix
at `this+4`.

The active manager's preferred-prefix and generic passes call the same
containment function **directly**, at `0x100214b6` and `0x100215e9`:
`0x10021d10`. There is no virtual AFG containment override. That routine reads
only the inverse transform at `this+4..this+0x3c` and compares the transformed
point to the inclusive unit box. It does not read name, color, debug data or
an AFG-specific shape flag.

The separate native fog query explicitly requests the AFG prefix at
`0x10068e79..0x10068e84` and then invokes fog-related behavior. This does not
remove AFG from generic environment selection: the generic manager fallback
excludes APV, and AFG remains an ordinary ordered candidate there.

## Signs, rotations and the supported boundary

For finite input half-extents `(x,y,z)`:

```text
m = max(x, y)                 // signed comparison; no absolute values
AFG extents = (m, m, z)
M = T(center) * R * S(AFG extents)
```

Examples:

| Authored half-extents | Registered half-extents |
| --- | --- |
| `(2,7,4)` | `(7,7,4)` |
| `(2,-7,-4)` | `(2,2,-4)` |
| `(-7,2,4)` | `(2,2,4)` |
| `(-2,-7,-4)` | `(-2,-2,-4)` |
| `(0,2,4)` | `(2,2,4)` |
| `(0,-2,4)` | `(0,0,4)`, rejected as singular |

Both-negative XY therefore choose the smaller horizontal magnitude. A
negative Z stays negative. Square XY are unchanged. Neither rotation reset,
absolute-value maximum nor swapping dimensions after rotation matches native
construction.

Binary EQGZ uses the existing native rotation contract:
`Rz(q(raw[0])) * Ry(q(-raw[1])) * Rx(q(raw[2]))`, where q truncates raw
512-unit angles and masks the table index with 511. The AFG branch performs no
additional unit conversion or sign change. The shared matrix builder stores
rotation entries and scaled entries as float32 before inversion.

Top-level DAT first constructs its existing center from tile position and the
native terrain-height anchor, divides full size by two, and converts its
degree angles to the native units. Its supported subset still requires
positive dimensions, unit stored scale, yaw-only rotation and the existing
grid/anchor checks. AFG support does not extend DAT to tilted or signed-size
records, or establish embedded object-group transforms.

Both formats use `native_box_half_extents` before building their inverse.
Binary `half_extents` retains the raw record; `registered_half_extents`
reports the effective geometry. DAT `NativeRegionBox::half_extents` reports
registered geometry while `TerrainRegion::full_size` retains the authored
record. Non-finite input remains rejected, even if normalization could discard
the offending component. Binary zero/subnormal resulting extents and singular
or overflowing resulting matrices still reject the **whole set**. A discarded
zero horizontal input is permitted when normalization produces a valid basis.

Native inversion uses x87 and stored float32 values, while these CPU boxes use
f64 inverse arithmetic. Tests use boundary margins except for exact cardinal
cases; support does not promise identical rounding on every surface.

## Precedence and original fixtures

AFG does not imply water, lava or freezing water. The native type callback
retains the raw word for that name. Runtime liquid classification continues
to require explicit AWT/ALV/AVW names. An AFG winner therefore suppresses later
liquid evidence without inventing a liquid from its raw numeric type.
Repeated AFG names remain separate records with separate transforms.
An earlier liquid can likewise win over a later AFG. Preferred-prefix queries
and generic queries retain their existing separate behavior.

Point and swept queries both use the normalized box. The sweep subtracts
winning AFG intervals from later water, rather than discarding AFG records
before unioning liquid intervals. Unsupported geometry still rejects the
complete ordered set; it is never replaced by an empty individual record.

### Arelis

The original version-21 DAT has six regions and six region-free TOG
placements. `AFG_arelis`, source offset **8,559,690**, has full size
`[3510,3510,700]`, zero rotation and unit scale. Its registered half-extents
are `[1755,1755,350]`; its square XY make normalization a no-op. There was no
demonstrated original geometry error for this record, but the prior AFG guard
disabled the entire zone's liquid metadata.

The original fixture now verifies that the runtime set is nonempty, both
`AWT_lake` and `AWT_waterfall` centers are water, the AFG center is dry, and the
metadata audit reports `supported_top_level_subset` with all six records.

### Pohealth

The original binary version-2 ZON has 19 regions: one APV, two ATP, and 16 AFG.
It has no authored AWT/ALV/AVW records. `AFG_10`, source index **3**, source
offset **5,864,886**, has:

```text
center = [1495.302978515625, 2190.63232421875, 25.553619384765625]
angles = [-128, 0, 0]
raw half-extents = [832.6297607421875, 487.094482421875, 171.72036743164062]
registered half-extents = [832.6297607421875, 832.6297607421875, 171.72036743164062]
```

Native yaw maps local Y to world X. The point `center+[600,0,0]` is inside
AFG_10's expanded box and selects source index 3 through the complete generic
query; an ordinary unexpanded box would omit it. The individual box excludes
`center+[833,0,0]` and `center+[0,0,172]`.

The original fixture verifies all 19 records parse and the metadata audit
reports `no_supported_volumes` with no unsupported diagnostic. The public
liquid API stays empty, correctly, because this is a supported dry-only set.

## Validation

`tests/binary_regions.rs` covers both signed-max directions, mixed signs,
both-negative XY, retained negative Z, case-sensitive short/full names,
cardinal rotation on all three axes, post-normalization singular rejection,
duplicate names, dry/wet precedence and finite swept dry gaps.

The noncardinal binary test uses center `[10,-20,30]`, raw angles
`[31.9,-73.8,19.2]`, and raw extents `[2,-7,4]` / `[-2,-7,-4]`.
Expected points were generated by an independent static instruction replay of
`0x100c2889..0x100c2906`, including its float32 stores, after applying the
recovered AFG signed comparison. This does not use the Rust implementation.
The resulting boundary probes distinguish rotated containment from a world
axis square and distinguish signed maximum from absolute-value maximum.

`tests/terrain_regions.rs` covers unequal dimensions in both orders, square
dimensions, yaw 0, 90 and -35 degrees, preserved authored metadata, dry-region
precedence through verified region-free groups, reversed source order and
finite swept intervals. Existing unsupported DAT transform checks remain.

Validation command, including original assets:

```text
CARGO_INCREMENTAL=0 cargo test -p openeq-assets --test binary_regions --test terrain_regions -- --include-ignored
```

Result: **14 binary-region tests and 27 terrain-region tests passed**, with no
ignored tests. No live character, server state, GPU rendering or audio changed.
