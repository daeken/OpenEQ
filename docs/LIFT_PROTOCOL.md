# Kelethin lift verification

Greater Faydark's three Kelethin lifts use ordinary server door records and
`ClickDoor`/`DoorMoved` messages. The original models are `FAYLEVATOR` for the
platform and `FELE2` for its buttons. The Storage2 PEQ database supplies:

| Lift ID | Lower button | Upper button | Vertical travel |
| --- | --- | --- | --- |
| 69 | 73 | 74 | 68 units |
| 77 | 79 | 78 | 98 units |
| 80 | 81 | 82 | 69 units |

All three lifts have `opentype=59`, `invert_state=1`, size 100 and a 5000 ms close
timer. Their `door_param` supplies the travel distance. Each button's
`triggerdoor` points to its lift. `Doors::HandleClick` sends the button action
and recursively activates that linked lift; the client must send the clicked
button's ID, not substitute an assumed platform ID.

EQEmu's spawn state is `(invert_state ? !IsDoorOpen() : IsDoorOpen())`.
`LiveWorld` normalizes this once on receipt, then normalizes `DoorMoved` actions
using the same inversion flag. Dynamic rendering and collision consume the
resulting logical state. Position and heading are converted from server space
to asset scene space at the same boundary.

`Doors::Process` silently resets ordinary type-59 lift/button state after its
timer. It only broadcasts a closing action for type 40 or trigger type 1. The
client therefore schedules a local return five seconds after an opening state
for types 59/60, including buttons. A subsequent opening restarts that deadline;
an explicit close cancels it, and zoning or a fresh door snapshot discards stale
deadlines. Without this return, the next button press can send another OPEN
after the server has reset while the client still thinks the lift is open.

`DoorRenderer` shares model geometry and updates instance transforms. Its
dynamic collision follows the animated lift pose. `take_platform_displacement`
returns a translation only when the player's feet were supported by the lift's
original mesh at the previous pose; consuming it prevents a second application
in the same frame. Jumping or flying discards pending carry. Static zone
collision remains separate from these moving objects.

## Reproducible live proof

```sh
cargo run -p openeq --bin lift_smoke -- \
  "$HOME/.config/openeq/storage2-commerce-credentials.json"
```

The executable refuses any host/account/character except the private Storage2
**Broker** fixture. It connects through `LiveWorld`, uses authenticated `#zone`
travel to Greater Faydark, converts scene points to server coordinates for
fixture-only `#goto` commands, and awaits actual position corrections before
clicking the six buttons. It then waits for automatic return and presses the
same button again to verify reuse after the server's silent reset. No world or
NPC database content is changed.

Each real lift transition then drives the original `FAYLEVATOR` model in a
headless GPU renderer. The rider starts at the centroid of its largest upward
horizontal triangle, away from edges. At 120 sampled animation steps, the probe
consumes platform movement once, advances `GroundMotion` against the animated
collision, and requires continuous support and the exact final travel distance.
The headless rider is a physics verification; the network character stays beside
the clicked button. This avoids claiming the probe itself performs a native
interactive ride.

Verified against Storage2 on 2026-09-28:

- Buttons 73/74 changed lift 69 from logical 0 → 1 → 0. Platform support height
  moved from 5.157 to 73.157 and back, a 68-unit roundtrip.
- Buttons 79/78 changed lift 77 from 0 → 1 → 0. Support height moved from
  -24.653 to 73.347 and back, a 98-unit roundtrip.
- Buttons 81/82 changed lift 80 from 0 → 1 → 0. Support height moved from
  5.407 to 74.407 and back, a 69-unit roundtrip.
- All three lift/button cycles returned after approximately five seconds without
  another click. Reusing the same lower button afterward produced a new ascent,
  and the upper button returned the platform.
- All 2160 physics samples retained support across explicit, automatic and
  repeated roundtrips; consuming the same frame's platform displacement twice
  returned zero on the second call.
- Broker returned through the normal zone handoff to PoK and the safe server
  center `[934, -305, -92.875]`, then logged out. Restoration is attempted even
  when a verification step fails. Inventory and money are untouched.

The server supplies endpoints and state changes, not continuous platform poses.
The final native check stood on lift 69 and used E to click lower button 73.
The visible platform carried the character up, then returned automatically with
the character still supported. The center returned to Z=8.2 (feet Z=5.157),
and pressing E again started another ascent. A preceding native ride confirmed
the upper center Z=76.2 (feet Z=73.157). The fixture then traveled back to PoK,
was placed inside the bank and logged out with its inventory unchanged.

Current travel timing is client-side smoothing at `abs(door_param) / 25` seconds
(minimum parameter magnitude 20). This proof validates that rendering, collision
and rider motion agree; it does not establish exact proprietary-client timing.
RoF2's door list does not carry `close_timer_ms`; the five-second local return
matches these PEQ fixtures and the server default. Custom server timer values
cannot currently be discovered from the protocol.
