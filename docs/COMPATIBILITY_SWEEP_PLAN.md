# Next original-asset compatibility sweep

This plan selects three zones absent from the current targeted scene/collision
fixtures, one per zone format. It records a read-only metadata audit of the local
EverQuest installation on 2026-09-29. No original assets were modified or copied
into the repository, and no GPU or live-server tests were run for this plan.
Counts describe this installation, not every client release.

## Existing coverage and its limits

`openeq-assets/tests/zones.rs` covers general loading for Greater Faydark,
Chardok, City of Mist, Lower Guk, Abysmal Sea, Acrylia and Dawnshroud, plus
specific PoK invisible-material/animation and Anguish water regressions.
`tests/terrain.rs` checks complete DAT alignment and sampled textured terrain
for Nektulos, Old Commonlands and Dead Hills; Crescent supplies binary EQGZ v2
placement coverage. These checks do not establish collision continuity along
tile borders or correct swimming in newer zones.

Liquid fixtures cover selected WLD zones. Binary EQG and heightmap liquid
volumes intentionally remain unsupported; see [LIQUID_REGIONS.md](LIQUID_REGIONS.md).
Many original-data tests return early when local assets are absent. An explicit
original-asset sweep must report missing fixtures as skipped, not count an early
return as compatibility evidence.

Model fixtures cover classic, Luclin and selected EQG races, with broader PoK
model enumeration. They do not prove that every NPC used by the three zones
below has a loadable, correctly animated model.

## 1. Feerrott, the Dream (`feerrott2`): missing tile-defined water

**Format:** text EQTZP version 4 plus heightmap DAT. This is the highest priority
because the authored water path is demonstrably omitted by the current loader.

**Observed assets:** `feerrott2.eqg` contains `feerrott.zon` and `feerrott.dat`;
the internal name differs from the archive name. The DAT parses to its final
byte with 329 tiles, 10,857 single-object placements, four groups, 11 regions
and no point lights. Tiles have 16 quads per side at 16 units per vertex.
Water levels are -1000 on 253 tiles, -30 on 73 and -50 on three. Every tile
has a secondary water record: 264 use tag 0 and 65 use tag 1. Region names
include `AWT_small_pond` and `AWT_07_river`.

`water.dat` contains an indexed `*WATERSHEETDATA` definition (`*INDEX 1`),
with `water_n.dds`, `water_e.dds` and authored water colors, but **zero finite
`*WATERSHEET` blocks**. `terrain::parse_water` accepts only the latter, and
`loader::load_heightmap` creates water geometry only from its result.
`TerrainTile::water_level` is retained but unused for surface generation;
secondary water records are consumed only for alignment. See
[HEIGHTMAP_FORMAT.md](HEIGHTMAP_FORMAT.md).

**Expected defect:** missing river/pond surfaces despite available water
material data. This follows from the surface loader; the exact shoreline,
secondary-record meaning and liquid volume transforms still need verification.
Do not assume -1000 is universally a sentinel, draw water across entire tiles
without clipping evidence, or infer an infinitely deep swimming volume.

**Next verification:** add a CPU fixture for the internal-name fallback,
complete DAT alignment, exact water record counts and indexed material lookup.
Preserve full secondary records and regions before interpreting them. Compare
neighboring tile heights and relevant quad flags at river boundaries; the
observed flag histogram is 0:52,342, 128:31,855, 1:25, 4:1, 132:1, so treating
all nonzero flags as holes would erase substantial terrain. After the water
record interpretation is established, capture fixed cameras above and below
the named pond and river, walk their banks and cross adjacent tile seams.
Acceptance requires bounded water surfaces, continuous shore geometry and no
invented swimming support while region transforms remain unresolved.

## 2. Timorous Deep (`timorous`): invisible collision geometry is discarded

**Format:** classic S3D/WLD. This directly tests the distinction between hidden
rendering surfaces and physical barriers.

**Observed assets:** `timorous.wld` contains 13,189 mesh fragments and 222,673
polygons in material runs. Of these, **12,438 polygons have both a zero WLD
render method and a collidable polygon flag**. Examples include material
`M0000_MDF` on `R117_DMSPRITEDEF` (six polygons), `R118_DMSPRITEDEF` (four),
`R127_DMSPRITEDEF` (four) and `R128_DMSPRITEDEF` (six). The other observed render
methods are `0x80000001` and `0x80000013`.

`mesh::bake_wld_meshes` skips every zero-render material before producing geometry.
`collision::CollisionWorld` is built from the rendered scene's collidable
triangles and explicitly documents that these hidden surfaces are unavailable.
Thus this installation loses all 12,438 of those source collision polygons
from the static collision input. The metadata audit did not establish which
ones form player-reachable barriers, so a specific walk-through location is
still a hypothesis, not a visually reproduced failure.

**Next verification:** first assert source polygon/material counts and show
that representative hidden triangles never enter drawable meshes. Determine
their world bounds, winding and reachable purpose before selecting a movement
fixture. Then preserve physical triangles through a collision-only channel,
with a paired synthetic test for an invisible blocking wall and an invisible
noncollidable surface. Walk against a verified barrier from both sides and
capture the same camera before/after. Acceptance requires the barrier to block
where authored while remaining invisible. Do not undo the PoK magenta/collision
mesh rendering fix to recover collision.

## 3. Bloodfields (`bloodfields`): polygon semantics and baked lighting

**Format:** binary EQGZ version 1 with TER/MOD geometry. This is a strong mixed
material and collision fixture beyond the existing Anguish water checks.

**Observed assets:** the archive has 523 MOD files, one TER, 691 LIT files and
157 DDS files. The ZON header reports 530 object references, 697 placements,
three regions and 134 lights. Auditing all MOD/TER polygon records finds
311,023 polygons and 487,905 vertices before placement duplication. There are
9,644 polygons with flag `0x1`, and 14,059 with flag `0x2`; every `0x2` polygon
also has material index `0xffffffff`. Numerous high flag bits also occur.
Material declarations include 198 `Chroma_MaxCB1.fx`, 1,295
`Opaque_MaxCB1.fx`, six `Opaque_MaxCBSG1.fx` and four `Opaque_MaxC1DTP.fx`.
These are declaration counts, not unique shaders or placed draw counts.

`TerMod::mesh_groups` drops polygon flags when grouping by material.
`loader::append_eqg_object` skips unresolvable material indices and marks every
remaining geometry collidable. It reduces shader handling to diffuse, normal,
water and alpha-mask behavior; authored LIT data is not applied. The current
code therefore cannot preserve differing collision semantics if the low bits
encode them, and omits this archive's baked illumination. **The audit does not
prove the meanings of flag 1, flag 2 or the high bits**; no behavior should be
assigned just from their values.

**Expected defects to investigate:** incorrect blocking on flagged visible
surfaces, missing hidden collision faces where material indices are absent,
and substantial lighting/detail differences around props. Polygon semantics
must be established with a trusted format implementation or matching original
client behavior before changing them. Treat the visual impact of omitted LIT
data separately from the existing dynamic point-light implementation.

**Next verification:** add a CPU inventory fixture preserving exact flag,
material and shader distributions, then select one named triangle/model for
each verified collision class. Record material-property usage and LIT-to-
placement associations rather than guessing from shader names. Capture fixed
cameras at an alpha-cutout prop and a baked-lit interior, with stable exposure,
fog and time-of-day. Walk through or against the chosen flag fixtures. Accept
only evidence-backed changes that retain visible-material assignment and
avoid turning every high flag bit into a collision or transparency rule.

## Shared acceptance procedure

1. Run the CPU inventory/alignment fixtures first, with an explicit installed
   asset root and a report of unavailable archives. Keep proprietary payloads
   out of fixtures and the repository; store derived counts and synthetic cases.
2. Resolve the server's zone/version and player spawn position before a live
   sweep. Use the matching asset generation and record camera coordinates,
   heading, time-of-day and environment settings for reproducible images.
3. For each zone, enumerate the NPC race/gender/texture/equipment combinations
   actually used by the selected world database. Report successful model and
   animation resolution, unavailable assets and unsupported variants separately.
   Verify standing, walking and turning against server updates, including actor
   feet on the tested surfaces; a generic model fallback is not model coverage.
4. Run fixed-camera GPU checks and bounded collision walks after the CPU work,
   without simultaneous GPU galleries. Check terrain/props, liquid boundaries,
   hidden barriers, alpha cutouts, fog and lighting independently. Record the
   concrete locations of failures instead of certifying an entire zone from a
   successful load or a single screenshot.

The present deliverable is this plan and its metadata evidence. It does not
claim these zones have passed rendering, movement, swimming or live-NPC tests.
