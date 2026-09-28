# Dynamic doors and world props

The server's door records also represent elevators, portals, books and other
interactive props. They are not necessarily in the zone's static placement list.
`load_object_library` loads reusable WLD meshes from zone object archives and MOD
meshes from EQG archives, including supplemental files in `<zone>_assets.txt`.
`Scene::object_model(name)` extracts a decoded, unplaced model on demand.

`DoorRenderer` consumes network-neutral `DoorState` records. Geometry/textures
are cached by model name and multiple copies are instanced. Opening modifies
only instance transforms around each model's authored origin; the zone mesh is
never rebuilt. A state change can reverse a partly completed opening smoothly.

Implemented motion classes follow the documented [EQEmu door open types](https://github.com/EQEmu/eqemu-docs-v2/blob/main/docs/server/zones/door-open-types.md):

- Types 0–8: 90 degree forward/backward hinge rotation.
- Types 10–27 and 45: forward/sideways translations.
- Types 59/60: vertical lifts using the server's door parameter as travel.
- Types 100–107: spinning props.
- Invisible types 50/53/54 are omitted. Teleporter/book/static types remain visible
  at their server position without inventing an opening animation.

Exact original slide distances, cycle timings and special trap motions have
not been recovered. Current slide travel uses model footprint and the motion
class; hinge animation takes 0.75 seconds. Types 30/35/36/40 use the hinge
transform and depend on subsequent server state for closure. Continuous spins
use approximate rates. These approximations are isolated in `doors.rs`.

The dynamic collision world is rebuilt only when server door state/placement
changes. It uses the final closed/open pose, while the visible transform eases
between them. This avoids rebuilding the zone or a collision grid every frame.
It does not carry a passenger along with an interpolating lift, nor update
continuous spinning/trap collisions each frame. The movement layer must combine
this world with static zone collision.

Verification with installed original assets:

```sh
cargo test -p openeq-assets --test objects
cargo test -p openeq-render doors::gpu_tests -- --ignored --nocapture
```

The GPU test writes `/tmp/openeq-doors.png`, showing an original PoK door closed
and opened. It also checks one GPU model is shared, closed-door collision blocks
passage, opened-door collision clears passage, and removed objects stop drawing
and colliding.
