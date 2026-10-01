# Anguish: entrance route and the authored water box

October 1, 2026. An offline route through the original static scene reaches the
central room from the entrance-side bridge at 10/30/120 FPS. The complete
traversed route is dry. Its final physical floor is about 47 units above the
top of the registered `AWT_water` box. Two bounded lower approaches stop at
physical walls before reaching the box. **A playable swimming route into this
particular box is still not established.** These checks do not prove global
inaccessibility and do not justify moving regions, surfaces or collision.

This extends the original intersection evidence in
[EQGZ_NATIVE_REGIONS.md](EQGZ_NATIVE_REGIONS.md) and the coarse floor survey in
[OVERNIGHT_2026-10-01.md](OVERNIGHT_2026-10-01.md). It uses production
`GroundMotion::step_in_world`, `CollisionWorld::build` and `LiquidRegions::load`.
There are no live characters, server connections, audio devices, substituted
floors, flight permissions or position corrections during the route. Placed
static objects are included; dynamic doors, actors and server movement are not.

## Three distinct authored layers

All coordinates here are scene XYZ, with Z up. The terrain stays in its original
coordinates, as established by the native registration evidence. No terrain
placement transform or server X/Y exchange is applied to region queries.

`anguish.eqg` has SHA-256
`b417dbcb0932c8f1fa4a66d7650e587267145125c74fd38f94ec5d97558f2a28`.
Its `ter_island.ter` payload has SHA-256
`09dfce100dc74bc8d67c37bb8f95e25db500832d9ce718fb99a15fd89be3ce06`.
The version-1 ZON contains two regions: `ATP_1_prison` and `AWT_water`. There is
no authored ALV lava box in this declaration.

The native water box has center `[700.905884, 2.677391, -256.871124]`, signed
half-extents `[125.730637, -123.330093, 11.755912]`, and native angular units
`[-1,0,0]`. Its vertical interval is approximately
`[-268.627036, -245.115212]`. Its interpretation and registered point-query
parity were established independently in the native-region investigation;
this route probe does not reinterpret those fields.

A separate raw TER parser intersects original indexed triangles at scene XY
`[700,0]`, independently of the scene bake and collision queries:

| Source polygon | Z | Material / shader | Flags |
| --- | ---: | --- | --- |
| 74295 | -361.927246 | `grnd` / `Opaque_MaxCB1.fx` | `0x10000`, physical |
| 90888 | -288.193085 | `water` / `Opaque_MaxWater.fx` | `0x1`, passable |
| 75750 | -245.41164 | `grnd` / `Opaque_MaxCB1.fx` | `0x1`, passable |
| 96358 | -245.25470 | `grnd` / `Opaque_MaxCB1.fx` | `0x1`, passable |
| 70637 | -198.040100 | `prison 14` / `Opaque_MaxCB1.fx` | `0x0`, physical |
| 6473 | -16.71902 | `prison 08` / `Opaque_MaxCB1.fx` | `0x0`, physical |

The large visible water sheet has 20,000 source/baked triangles at constant
Z `-288.193085`, with bounds approximately
`[-3803.9426,-2457.142]..[2196.0574,2542.858]` in XY. It is about 19.566 units
below the **bottom** of the water box. The two passable ground faces near the
box's top have ordinary ground material, not a water shader. Neither their
texture nor their passability supplies missing liquid boundaries.

At the route destination `[700,-10]`, production collision finds the physical
floor at `-198.0401`. The standing center at floor + 3 is dry. The point
`[700,-10,-256.87112]` is water, while the visible-sheet point
`[700,-10,-288.19308]` is dry. These assertions distinguish a correctly loaded
finite box from absent liquid metadata or a guessed volume filling the exterior.

## Controlled entrance-side route

The initial feet are `[-2000,0,-90.230705]`, supported by original collision on
the entrance side of the curved bridge. This is an offline geometric start in
the main entrance vicinity, not a claim about the server's zone-in position.
In particular, the separate `ATP_1_prison` region center is not used as a spawn
or swimming start.

The controller follows these XY targets in order; Z always comes from movement:

```text
[-1900, 10], [-1850, 40], [-1750, 90], [-1600,110], [-1500,130],
[-1350, 70], [-1200, 50], [-1050, 10], [ -900,-35], [ -750,-80],
[ -650,-90], [ -500,-75], [ -350,-35], [ -200,-25], [    0,  0],
[  200,  0], [  350,  0], [  550,  0], [  700,-10]
```

Walking speed is 40 units/s, with the final input frame reduced only when
necessary to avoid overshooting a waypoint. A target is reached within one
unit. The input controller requests a normal jump after 0.25 seconds without
horizontal progress, at most three times per target, and stops after two
seconds of continued stationary motion or 40 seconds per target. These are
explicit probe controls, not new production movement rules.

| Frame rate | Reached targets | Final feet | Elapsed simulated time | Jump requests | Swimming frames |
| --- | ---: | --- | ---: | ---: | ---: |
| 10 | 19/19 | `[700.00018,-10.00000,-198.04010]` | 70.5 s | 2 | 0 |
| 30 | 19/19 | `[699.69116,-9.97953,-198.04010]` | 69.0 s | 0 | 0 |
| 120 | 19/19 | `[699.02271,-9.93536,-198.04010]` | 69.6667 s | 2 | 0 |

At 10 and 120 FPS the two jump requests occur near bridge coordinates
`[-924.82,-27.49,-165.31]` and `[-604.77,-85.4,-190.13]`. Input direction is
recomputed once per frame, so the three runs do not use identical trajectories
or jump histories. This establishes successful traversal at those frame rates,
not native movement parity or frame-invariant steering. Every run reaches the
lower entrance ramp near `[200,0,-250.70256]`, then rises to the central room's
floor. Being at a Z within the water interval on that earlier ramp does not
make it wet: the ramp is outside the box's XY footprint.

## Bounded lower approaches

Two additional 120 FPS runs follow the same entrance route through `[200,0]`,
then try `[200,+80] -> [400,+80] -> [575,+80] -> [700,+80]` and the matching
negative-Y sequence. Both reach the first side target and stop during the next:

| Approach | Last accepted feet | Reached targets | Swimming frames |
| --- | --- | ---: | ---: |
| Positive Y | `[291.23257,65.10417,-250.70256]` | 17/20 | 0 |
| Negative Y | `[291.23297,-65.10428,-250.70256]` | 17/20 | 0 |

Each makes three bounded jump attempts at the blocking wall, in addition to
the two earlier bridge jumps. A diagnostic camera at `[270,65,-244.70256]`
looking toward positive X shows the original room wall at this approach.
No position is advanced through the wall and no later waypoint is certified.
Other corridors, drops, door states, alternate inputs and live reachability
remain outside this bounded search.

## Rendering observations and limits

Headless captures use the original scene and a fixed one-second render time.
The diagnostic disables fog with `EnvironmentSettings::default()` so the
geometry is visible; it does not claim native lighting, atmosphere, transparency
or framebuffer parity. Local captures are:

- `/tmp/openeq-anguish-route-overview-clear.png`: camera
  `[-650,0,2200]`, yaw 0, pitch -89; entrance, curved bridge, central structure
  and exterior water sheet.
- `/tmp/openeq-anguish-basin-overhead-floor.png`: camera
  `[650,-60,-188]`, yaw 60, pitch -15; original indoor floor, stairs and bowl
  geometry above the water box.
- `/tmp/openeq-anguish-basin-lower-approach.png`: camera
  `[270,65,-244.70256]`, yaw 90, pitch 0; the wall blocking the lower approach.

The room capture does not establish a rendered liquid body coincident with
`AWT_water`. The visible exterior sheet renders separately, and its prior
animation/no-magenta regression remains separate evidence. This investigation
finds no justified liquid-coordinate or renderer correction. It also does not
upgrade the isolated-box swimming test into a verified playable-water route.

## Regression and frozen artifacts

The original-assets regression is in
`crates/openeq/tests/anguish_basin_route.rs`, with its frozen draft retained at
`/tmp/anguish_basin_route.rs`. It verifies retained wet metadata,
the dry visible-sheet point, original physical floor, all 19 targets at the
three frame rates, dry movement and supported endpoints. It passed an
independent standalone test build using the exact production movement source
and current assets library. The integrated focused Cargo test also passes
(one test, zero failed/ignored), logged at
`/tmp/openeq-anguish-basin-route-tests.log`. The subsequent workspace count is
separate; this investigation changes no production behavior.

The route executable was built from `/tmp/openeq-anguish-basin-route.rs` using
the workspace's `openeq_assets`, `glam` and `serde_json` libraries. Its movement
modules reference the production source paths. Recorded source hashes are:
`movement.rs` =
`de83012aabbc0acf96d3d4252b665a134e040cdf7a8271e6a1692d14b4e3d483`;
`collision.rs` =
`8fe9ff9da1224b74c31578f077158f598a21f132b21b52e6ba41c5149f27a5e4`.
The resulting static collision world contains 459,858 triangles. Run the frozen
route executable to regenerate its JSON and compact log; rebuild against these
source hashes before treating later results as the same witness. Use
`CARGO_INCREMENTAL=0` for Cargo checks.

| Local artifact | SHA-256 |
| --- | --- |
| `/tmp/openeq-anguish-basin-route.rs` | `8090abc32e1682426c50221b3d89f9bb0feae188a0f04b86273913eab2804415` |
| `/tmp/openeq-anguish-basin-route.json` | `c7f16ad1d38a3e694a00bcf366ace4ac7c9e55197966db2c36cb239906e34182` |
| `/tmp/openeq-anguish-basin-route.log` | `685499d91524d39662f43ff842f2fc7b47e1ac0b77e8a7d1ca304dd68bf20c97` |
| `/tmp/openeq-anguish-source-section.py` | `c4b241ee816b2b2b9b2b913413a757279ce50e852712ef7eac3f1a8e37402d33` |
| `/tmp/openeq-anguish-source-section.json` | `d3e9edb0e8a73458c00ff8c711e79b8f6ffa0383fe5985a3dce112e6623255a6` |
| `/tmp/openeq-anguish-route-render.rs` | `da675b5a5918d9c6862b24ce427c6f7a1f7e1cd3e672a49860039c249bf17072` |
| `/tmp/anguish_basin_route.rs` | `0894b368d6ce00a860cc54807dc86dd4983c5f5b1e66a8cf0e1194c2f0548324` |

The raw-section parser reads the original archive through the existing local
`/tmp/openeq-zone-regions.py` archive helper, SHA-256
`fefdbec6f8db61f1cd5f8bbadd5195a54dda388fc72dd4bb6585aaa0da8f6d35`.
Source assets and captures are not committed. The local JSON artifacts retain
each sampled route, requested jumps,
checkpoints and independent source-triangle intersections for later review.
