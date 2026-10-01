# Authored liquid regions

`openeq_assets::liquid_regions::LiquidRegions` provides immutable, cheaply cloned
point and segment queries in **asset/scene coordinates: X/Y horizontal, Z up**.
Server X/Y must be exchanged at the protocol boundary, not in this API.

Supported sources are classic WLD BSP declarations, verified binary EQGZ boxes,
the verified top-level heightmap DAT subset, and explicit finite `LiquidBox` volumes. A water material, water surface, or texture name is
never evidence of a swimming volume. Empty metadata means **no supported volume
source**, not a declaration that the original zone has no water.

## WLD evidence and decoding

The bounded WLD metadata parser is shared with `ZoneLines`. It reads:

- `0x21`: BSP nodes, float plane `(nx, ny, nz, distance)`, region, two child IDs.
  Children and leaf region IDs are one-based; zero children mean no subtree.
- `0x22`: region definitions, counted to validate references.
- `0x29`: region annotations with **zero-based** region indices and either an
  XOR-encoded declaration payload or, when the payload is empty, the fragment
  name from the XOR string table. Payloads supersede names.

Known prefixes are deliberately narrow:

| Declaration | Liquid |
| --- | --- |
| `WT_`, `WTN_`, `WTNTP` | Water, including water with a zone line |
| `LA_`, `LAN_`, `LANTP` | Lava, including lava with a zone line |
| `VWN_` | Freezing water |
| `SLN_` | Water that blocks visibility (`OpaqueWater`) |

Zone-line-only `DRNTP`, PvP `DRP_`, slippery `DRN_..._s_`, and unknown declarations
are excluded. `SLN_` is not treated as a slippery floor. Classification follows
LanternExtractor's `BspRegionType` interpretation; EQEmu's deprecated `azone2`
labels some prefixes less precisely.

The BSP is queried directly, preserving concave unions, disjoint areas, stacked
rooms, and its actual lower and upper bounds. The reader rejects invalid child
IDs, cycles/shared children, nonfinite/zero split planes, invalid liquid region
references, conflicting known liquid tags, and truncated/count-corrupt data.
Segment queries prune dry subtrees, clip the full movement segment, and report
ordered `LiquidSpan { kind, enter, exit }` fractions. Crossing a thin volume is
found even if both endpoints are dry. Same-kind contiguous spans merge; dry gaps
and different kinds remain distinct.

Exact WLD split-plane points are dry, matching
`EQEmu/zone/water_map_v1.cpp::BSPReturnRegionType`. A segment lying along a split
plane has no wet span; an ordinary crossing has a finite entry/exit interval.
Point-only tangencies, zero-length segments and nonfinite input produce no spans.

`LiquidBox` uses positive half-extents and a normalized XYZW quaternion. Queries
transform into box-local space, so rotated boxes are not inflated into world
axis-aligned bounds. Box faces are inclusive; point-only segment tangencies are
still excluded. Overlaps use first-source precedence for both point and swept
queries.

## Original-asset fixtures

No client data is checked into the repository. Run:

```sh
cargo test -p openeq-assets --test liquid_regions -- --ignored --nocapture
cargo test -p openeq-assets --test zone_lines -- --ignored --nocapture
cargo test -p openeq-assets --lib liquid_regions
```

Plane of Knowledge has six water-labelled leaves. The fountain/pool fixture was
checked independently against the original WLD and Storage2's EQEmu WTR v1 map:

| Scene point | Result |
| --- | --- |
| `[15, 1455, -132]`, `[-15, 1455, -132]` | Water |
| `[15, 1455, -125]` | Dry above |
| `[15, 1455, -141]` | Dry below |
| `[35, 1455, -132]`, `[15, 1439, -132]` | Dry beside |

At scene XY `[15,1455]`, the supported water interval is Z `(-140,-126)` and
collision floor is Z `-134`. Segment Z `-150 -> -120` yields `enter=1/3, exit=.8`.
The server point for the wet sample is `[1455,15,-132]`. These are region facts;
body center and eye placement remain movement/presentation decisions.

Additional original fixtures exercise Qeynos water, Solusek's Eye water and lava,
and opaque water in Cazic Thule and Velketor. Greater Faydark contains zone lines
but no supported liquid declaration. Synthetic tests cover rotated boxes,
concavity, dry gaps, stacked rooms, overlap precedence, exact boundaries,
malformed BSPs, and a swept crossing with both endpoints outside.

## EQG native volumes and conservative boundary

The loader follows rendering's primary archive/declaration selection. EQG takes
precedence over S3D; unsupported EQG records never fall back to stale classic
metadata. Water textures and `water.dat` sheets never invent a lower volume bound.

Binary EQGZ v1/v2 records now contribute finite volumes. The native readers pass
center XYZ, raw Z/Y/X orientation and signed XYZ half-extents directly to the
registered box builder. Angles are **512-unit turns truncated toward zero**, not
radians or degrees. Anguish's raw -1.5707964 therefore becomes -1 native angular
unit, approximately -0.703125 degrees. See [EQGZ_NATIVE_REGIONS.md](EQGZ_NATIVE_REGIONS.md)
for reader/factory addresses, signed matrix construction and independent original
Anguish/Crescent boundary fixtures. No axis exchange or extra half-size is applied.

Binary registration preserves file order. Generic selection excludes APV but
retains dry/unknown winners; only explicit AWT/ALV/AVW names establish a supported
liquid. Swept queries use that same precedence. Unsupported or malformed records
reject the whole set, including nonfinite/singular transforms and invalid
references. AFG uses its verified signed-maximum XY extent normalization before
rotation and remains an ordered dry/unknown candidate; see
[EQG_AFG_REGIONS.md](EQG_AFG_REGIONS.md). They cannot be skipped to
expose a later water box. The metadata audit reports these limits separately from
a successfully decoded set with no supported liquid.

Heightmap DAT retains its separately recovered terrain anchoring, degree-to-native
angle conversion and ATP-first registration order, documented in
[EQG_LIQUID_TRANSFORMS.md](EQG_LIQUID_TRANSFORMS.md). The bounded subset requires
matching grids, strictly interior anchors, positive dimensions, unit stored scale
and yaw-only rotation. Placed groups are allowed only after every referenced TOG
is proven complete and region-free. Missing group files, actual embedded areas,
unknown grammar or unsupported transforms reject the whole set. Parent transforms
for embedded group regions remain unresolved; no rendered surface substitutes.

The October 1 installed metadata survey reports supported liquids in 92 binary
zones and 28 heightmap zones, versus no binary zones and one heightmap zone before
these changes. This is format/query coverage, not a certificate of traversability
or agreement with EQEmu's independently generated WTR boundaries. Zone-line side
effects, drowning/damage and native movement rules remain separate work.

Offline movement through Crescent's shallow river slice, above an independently
identified original floor, holds depth and reaches the authored volume surface
at 10/30/120 FPS. An Anguish fixture isolates its finite volume from scene
collision; its box/visible-water/floor relationship is not yet a verified live
route. Region centers can lie below terrain, so they are not automatically safe
swimming start positions. See the coordinate/traversability evidence in the
binary-region note.

Sources consulted:

- [EQEmu zone-utilities](https://github.com/EQEmu/zone-utilities/tree/b361e63dd067e8959f5bf2341579f481d2374fd5),
  `src/common/eqg_loader.cpp`, `src/common/eqg_structs.h`, `src/awater/water_map.cpp`.
- [Quail ZON reader](https://github.com/xackery/quail/blob/main/raw/zon_read.go).
- [EQ Sage zone reader](https://github.com/knervous/eqsage/blob/master/sage/lib/eqg/zone/zone.js).
- Local EQEmu `zone/water_map_v1.cpp`, `zone/water_map_v2.cpp` and deprecated
  `utils/deprecated/azone2/wld.cpp`.

Region classification does not grant breathing, infer damage, or change server
buffs. Lava/freezing/opaque kinds are retained so later presentation and protocol
work can distinguish them without inventing gameplay outcomes.

## Live swimming verification

`crates/openeq/src/bin/swimming_smoke.rs` runs a bounded authenticated proof against
Storage2 using only the dedicated `openeq_gameplay` / `Mechanic` fixture. It records
a private recovery file before login, refuses an already-online or instanced
fixture, uses GM positioning only for setup and restoration, then exercises the
production `GroundMotion::step_in_world` and normal position packets. Self-target
`#save` acknowledgments plus read-only database checks verify what the server
actually accepted. It always attempts restoration, including a fresh recovery
session if necessary, and checks the saved pose and persistent state after logout.

On 2026-09-29, the live run passed:

- Held feet at `[15,1455,-134]` without sinking.
- Swam horizontally to scene X approximately `3`, preserving depth. EQEmu saved
  server center `[1455,3.00001,-131]`.
- Ascended through the authored water surface, reaching maximum feet Z `-126.649`.
  The ascent checkpoint saved server center `[1455,3.00001,-125.427]`.
- Descended to the collision floor at feet Z `-134`, with the eye submerged again.
  EQEmu saved server center `[1455,3.00001,-131]`.
- Remained in a usable Plane of Knowledge session with 321 NPCs.
- Restored original server pose zone 202, instance 0, `[-285,-128,-158]`, heading 0;
  verified offline, with stats, currency, inventory, HP, mana and endurance unchanged.

Run with the private gameplay connection configuration; never print its contents:

```sh
cargo run -p openeq --bin swimming_smoke -- \
  "$HOME/.config/openeq/storage2-gameplay-credentials.json" "$HOME/EverQuest"
```

This is a movement and transport proof. It does not simulate breathing or apply
water/lava damage, and it does not validate unsupported EQG volume transforms.
