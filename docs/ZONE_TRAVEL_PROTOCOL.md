# Natural zone travel (RoF2 / EQEmu)

This documents the client-initiated border protocol against EQEmu commit
`4aceae18b94ffaafc08e2b17bc41cd72c77f795d`. Crossing detection needs zone-asset
trigger geometry; the server's zone-point packet supplies the destinations for
those triggers. A local crossing requests travel. Only server acceptance starts
the world/zone handoff.

## Destination table: `OP_SendZonepoints` (`0x69a4`)

The packet is little-endian and exactly `4 + (count + 1) * 32` bytes. The first
four bytes are a `u32 count`. Each of the first `count` records has this layout:

| Offset | Type | Meaning |
| --- | --- | --- |
| 0 | `u32` | `iterator`: exact `zone_points.number` |
| 4 | `f32` | Destination Y |
| 8 | `f32` | Destination X |
| 12 | `f32` | Destination Z |
| 16 | `f32` | Destination heading, EQ units |
| 20 | `u16` | Destination zone ID |
| 22 | `u16` | Destination instance ID |
| 24 | 8 bytes | Unknown, ignored |

The final 32-byte record is an unused trailer outside `count`, including when
`count` is zero. It must not create a destination. Do not infer a sentinel from
record contents or require the unknown fields/trailer to contain zero.

`Client::SendZonePoints` copies **target** X/Y/Z/heading from the database.
The record number is not its packet ordinal, a zone ID, or a source position.
EQEmu does not multiply or divide the number before transmitting it; any
asset-specific encoding must be decoded by the asset loader. Source geometry
cannot be reconstructed by treating these destination coordinates as triggers.

A destination position component of `999999` means keep that component of the
source position. Heading `999` similarly preserves the source heading. Retain
these values; EQEmu resolves them after selecting the exit. When a destination
is in the current zone and its configured instance is zero, the server sends
the current instance instead.

The server filters the table by client-version mask and loads records according
to zone version and content settings. Virtual zone points are handled separately
by the server and are not included in this destination table.

OpenEQ exposes `gameplay::ZonePoint` and
`GameplayEvent::ZonePoints(Vec<ZonePoint>)`. `position` is server XYZ; conversion
to scene coordinates belongs at the client boundary. Each table replaces the
previous table for that zone and must be discarded during a handoff. The parser
checks exact size before allocation, rejects truncated/extra records and
nonfinite destination floats, and preserves the wildcard values above.

Sources:

- `EQEmu/utils/patches/patch_RoF2.conf`: opcode mapping.
- `EQEmu/common/patches/rof2_structs.h`: `ZonePoint_Entry`, `ZonePoints`.
- `EQEmu/common/patches/rof2.cpp`: `ENCODE(OP_SendZonepoints)`.
- `EQEmu/zone/client.cpp`: `Client::SendZonePoints`.
- `EQEmu/zone/zone.cpp`: `ZoneDatabase::LoadStaticZonePoints`.

## Client request: `OP_ZoneChange` (`0x2d18`)

The existing `Command::ZoneChange` encoder emits the required 100-byte RoF2
layout:

| Offset | Type | Meaning |
| --- | --- | --- |
| 0 | 64 bytes | NUL-terminated character name |
| 64 | `u16` | Requested destination zone ID |
| 66 | `u16` | Requested destination instance ID |
| 68 | 8 bytes | Unknown, zero |
| 76 | `f32` | Y |
| 80 | `f32` | X |
| 84 | `f32` | Z |
| 88 | `u32` | Reason; zero for an ordinary unsolicited border crossing |
| 92 | `i32` | Success; zero in a client request |
| 96 | 4 bytes | Unknown, zero |

There is **no trigger-number field** in this request. The client uses the asset
trigger number to choose the advertised destination zone/instance. It then
sends the request through the ordinary zone connection. Sending the player's
latest movement first matters: EQEmu selects the source zone point using its
stored `Client::GetPosition()`, not the XYZ embedded in this request.

For an unsolicited request with a nonzero destination zone, EQEmu chooses the
nearest eligible source point for that target zone. For zone ID zero, it chooses
the nearest eligible point without a destination filter. Prefer the advertised
nonzero target for a resolved trigger. The source lookup uses database source
X/Y, client-version masks, and wildcard coordinates; it does not use a trigger
number from the client. Do not use the server's broad search range as a local
crossing radius. Source-map zone-line checks can also produce cheat reports
even when a point was found.

The handler ignores client-supplied XYZ when computing an accepted natural
destination. It uses the selected point's target XYZ/heading and resolves
wildcards against the server's stored source position. It also validates:

- Instance existence, membership, expiry, and destination-zone match.
- Destination zone/version availability.
- The player quest `EVENT_ZONE` hook, which may cancel travel.
- Required status, level range, zone flags, and expansion availability.
- World-server availability of the destination process during the handoff.

Server content routing can select a different instance. Accept the server's
resolved result instead of requiring its instance to match the initial request.
Prevent repeated requests while one crossing is pending; rearm crossing
detection only after leaving the trigger or an equivalent cooldown policy.

## Acceptance, cancellation, and errors

`GameplayEvent::ZoneChangeResult` preserves the reply's zone/instance, XYZ, and
signed success code. The distinction between these replies is essential:

- **Success 1 for a different zone/instance:** start the existing authenticated
  world/zone handoff. Do not teleport using this reply's XYZ: the world-server
  success path leaves those fields zero. The destination profile/spawn supplies
  the actual arrival position and heading.
- **Success 1 for the current zone/instance during a pending natural crossing:**
  EQEmu's `SendZoneCancel` uses this form to cancel the request and supplies the
  rewind position. Clear the pending crossing and apply the rewind without a
  handoff or immediate retrigger.
- **Negative success:** rejection (restrictions, unavailable destination, etc.).
  Clear the pending crossing and retain the current zone. The RoF2 encoder
  subtracts one from EQEmu's negative internal code, so raw wire error numbers
  are not the same as the common server enum values.

Do not discard current entities, doors, or assets on a local crossing request.
`ZoneClient` emits `ZoneTransition` only after an accepted change to another
zone/instance and then performs the authenticated handoff. Clear old zone-point
and trigger state at that accepted transition.

Sources:

- `EQEmu/common/patches/rof2_structs.h`: `ZoneChange_Struct`.
- `EQEmu/common/patches/rof2.cpp`: `DECODE/ENCODE(OP_ZoneChange)`.
- `EQEmu/zone/zoning.cpp`: `Handle_OP_ZoneChange`, `SendZoneCancel`,
  `SendZoneError`, `DoZoneSuccess`.
- `EQEmu/zone/zone.cpp`: `GetClosestZonePoint`,
  `GetClosestZonePointWithoutZone`.
- `EQEmu/zone/worldserver.cpp`: `ServerOP_ZoneToZoneRequest` handling.

## Server-solicited travel and limits

Door clicks, teleports, and virtual zone points can already use the separate
server-solicited `OP_RequestClientZoneChange` (`0x3fcf`) path. Its 176-byte
RoF2 packet has zone/instance at offsets 0/2, destination Y/X/Z at 8/12/16,
heading at 20, and type at 24. EQEmu's RoF2 encoder forces type `0x0b`. Common
server documentation says clients echo type as the `OP_ZoneChange` reason;
OpenEQ currently sends zero, which the reviewed handler does not use to gate
travel. Natural border decoding does not change that existing behavior.

Destination-table decoding alone does not add source geometry support for every
WLD/EQG format, nor does it implement local same-zone asset teleports. Such
triggers require explicit asset semantics; avoid inventing a proximity trigger
around a destination or issuing speculative zone requests. Unknown trigger
numbers should remain inactive until their meaning is resolved.

Protocol regression tests cover destination field order, exact record numbers,
wildcards, ignored trailer data, zero points, every truncation, surplus bytes,
invalid counts, nonfinite coordinates/headings, and the existing natural request
offsets. Live crossing validation is a separate client integration check.

## Implemented source geometry

`openeq-assets::zone_lines` compiles convex cells from WLD fragment 0x21 BSP
ancestry and fragment 0x29 region declarations. Leaf region IDs are one-based;
region-group indices are zero-based. Reference declarations `DRNTP`, `WTNTP`,
and `LANTP` with the five-digit zone field `00255` map their final six-digit field
to the exact server point number. Declarations can live in the name table or the
XOR-encoded fragment payload. Tree cycles, out-of-range pointers, nonfinite
planes and excessive expanded plane counts are rejected.

Point queries support spawn suppression; segment clipping catches thin volumes
even if a frame moves through them completely. Actual archive regressions cover
all four Greater Faydark exits and return volumes in Crushbone, Butcherblock and
Lesser Faydark. Geometry and triggers both prefer EQG if its archive exists;
EQG currently yields no authored border triggers until its oriented-volume
transforms are verified. Absolute WLD destinations are likewise not inferred.
