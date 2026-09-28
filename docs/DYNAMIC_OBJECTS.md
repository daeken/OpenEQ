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
Positions and headings use the scene coordinate convention. Incoming server
headings become `128 - heading`; original door meshes already have their local
X/Y axes exchanged, so their base rotation is `pi/2 - scene_heading * TAU/512`.
For example, a server heading of zero leaves the authored model unrotated.

`DoorState.state` is the logical server open state. EQEmu's spawn record encodes
`state_at_spawn = inverted ? !IsDoorOpen() : IsDoorOpen()` (`zone/entity.cpp`),
whereas `OP_MoveDoor` uses action 2/3 with the same inversion (`zone/doors.cpp`).
The live adapter normalizes the initial spawn state once. Rendering must not
apply inversion again: an inverted Kelethin lift otherwise starts at state 1 and
never changes on its first button press.

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

Lift collision uses the same animated pose as its visible model. The small
dynamic collision world is rebuilt while a lift moves; the static zone is never
rebuilt. Other doors still use their final closed/open collision pose, and
continuous spinning/trap collisions are not animated.

After an update, `take_platform_displacement(feet, allow_carry)` consumes the
latest lift displacement once. It checks support against the original collision
mesh at its previous pose, including the player's footprint; it does not use an
approximate platform bounding box. The movement layer applies that displacement
before ordinary static/dynamic collision. Pass `false` while jumping or flying
to discard carry motion. A second call without another update returns zero.
Exact lift timing is still approximate (25 units/second with eased endpoints);
the travel distance comes from the server's signed `door_param`.

EQEmu resets ordinary lifts and their buttons silently when its close timer
expires. `LiveWorld` predicts a five-second return for types 59/60, including
their buttons; a fresh open restarts the deadline, and an explicit close or
replacement door list clears it. This matches every installed Kelethin lift and
button record. RoF2 omits the timer and disable-timer flag, so five seconds is a
compatibility default and may differ on customized servers. Returning locally
sends no artificial button or door packet.

The installed PEQ Greater Faydark records provide three Kelethin lifts, all type
59 with inversion enabled. Read-only database inspection verified these links:

| Lift door | Lower / upper button | Travel |
| --- | --- | --- |
| 69 | 73 / 74 | 68 units |
| 77 | 79 / 78 | 98 units |
| 80 | 81 / 82 | 69 units |

`FAYLEVATOR` is the platform model; the buttons use `FELE2` with one unit of
vertical button travel. EQEmu's `Doors::HandleClick` follows `triggerdoor` and
sends both state changes, so the client sends only the clicked button ID.
No production door records need modification. The original platform's walkable
surface lies about 100 units from its authored origin. For lift 69, a verified
supported point is scene feet `(132.44852, 249.78363, 5.15713)`, near lower button
73; checking only distance to the lift origin would miss a standing passenger.

Verification with installed original assets:

```sh
cargo test -p openeq-assets --test objects
cargo test -p openeq-render doors::gpu_tests -- --ignored --nocapture
```

The GPU test writes `/tmp/openeq-doors.png`, showing an original PoK door closed
and opened. It also checks one GPU model is shared, closed-door collision blocks
passage, opened-door collision clears passage, and removed objects stop drawing
and colliding.
The Greater Faydark test loads both original lift and button models and verifies
the moving floor, ascent/descent carry, off-platform rejection, jump opt-out,
single consumption, and removal cleanup.
