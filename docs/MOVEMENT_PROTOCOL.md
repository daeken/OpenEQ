# RoF2 gravity, levitation, and liquid state

These notes distinguish server permissions and authored liquid regions from
client movement tuning. Evidence is the local EQEmu source at
`/Users/daeken/projects/EQEmu` and read-only Storage2 PEQ/map checks on 2026-09-29.

## Gravity and the affected player's buffs

`common/emu_constants.h` defines gravity modes 0 Ground, 1 Flying, 2 Levitating,
3 Water, 4 Floating, and 5 LevitateWhileRunning. `Mob::FillSpawnStruct` in
`zone/mob.cpp` explains that mode 3 is the usual NPC setting, on land as well as
in water; it is not a packet telling us that the entity is currently swimming.
Player spawns report mode 2 when a levitation buff is present, otherwise 0.
OpenEQ already retains the spawn mode and applies appearance type 19 updates.

`zone/spell_effects.cpp`, `SpellEffect::Levitate`, sends appearance type 19 with
`ignore_self=true`. The affected player instead receives the authoritative
buff. Effect 57 with a limit/base2 value of 1 means mode 5; any other limit means
mode 2. The same rule appears in `zone/client_packet.cpp` when restoring buffs
on login. Fading the last levitation buff sends mode 0. `SendAppearancePacket`
in `zone/mob.cpp` confirms the fourth argument excludes the originating client;
the RoF2 appearance encoder forwards all types except size without conversion.

`LiveWorld::player_gravity()` therefore combines the player's current spawn
appearance with `GameplayState::levitation_mode()`. An explicit Flying or
Floating appearance takes precedence. Otherwise an active server buff can
grant levitation without a self-directed appearance packet. Modes 0 and 3, and
unknown modes, use normal gravity; liquid occupancy is evaluated separately.
An elapsed UI buff countdown does not revoke an effect: only an authoritative
buff removal/full refresh does that. Buff lists for other entities are ignored
by the player's gameplay reducer.

`spells.rs` reads gameplay effect 57 (levitation) and 14 (water breathing), which
are distinct from a spell's particle effect identifier. Fixed exports use
effect columns 86..97 and limit columns 32..43 (`common/shareddb.cpp`). The
installed compact client stores final-field entries separated by `$`, each
`slot|effect|base|limit|formula|max`. Local spells 86 Enduring Breath, 261
Levitate, 457 Dead Man Floating, 970 Levitation, and 2517 Spirit of Eagle were
checked against the server's `spells_new` table; their relevant effect/limit
values agree. As with the original client, custom server spell definitions
require matching client spell data.

`has_water_breathing_buff()` reports only a decoded active buff contribution.
It does not assert that racial traits, worn items, or alternate abilities are
fully represented, and it does not drive an invented breath timer.

## Zone movement fields

The RoF2 `NewZone_Struct` is 948 bytes. `zone::Environment` now preserves these
fields from `common/patches/rof2_structs.h` and `ENCODE(OP_NewZone)`:

| Offset | Field | Interpretation |
|---:|---|---|
| 516 | `gravity` f32 | Raw zone gravity coefficient; tested zones send 0.4 |
| 608 | `underworld` f32 | Authored lower world boundary |
| 868 | `underworld_teleport_index` u32 | Raw index/sentinel, including `0xffffffff` |
| 880 | `lava_damage` u32 | Zone lava damage setting |
| 884 | `min_lava_damage` u32 | Minimum lava damage setting |
| 894 | `fall_damage_disabled` byte | Raw wire flag |
| 940 | `levitation_disabled` u32 | Raw wire flag |

EQEmu currently hardcodes both disabled flags to false in its RoF2 encoder.
In particular, `levitation_disabled=false` does not prove the database allows
levitation. `zone->CanLevitate()` is enforced by spell rejection and buff fades
in `zone/spells.cpp` and `zone/client_packet.cpp`, with a GM exception. Do not
replace those server decisions with assumptions based on the wire flag.

No measured original-client mapping from the raw gravity coefficient to
world-units/second squared has been established. OpenEQ acceleration, terminal
speed, buoyancy, swim speed, and levitation descent remain client tuning unless
backed by a separate measurement. These packet changes provide permissions
and values; they do not claim original-client motion equivalence.

## Swimming and environmental damage

`Client::Handle_OP_ClientUpdate` checks the authoritative server water map. A
moving client in liquid can train Swimming (skill index 50); entering liquid
can also dismount a mount according to `Character:DismountWater`. No separate
"you are swimming" packet was found in this path. OpenEQ must query the zone's
authored region volumes rather than infer water from a rendered horizontal
surface or from spawn gravity mode 3.

`WaterMapV1::ReturnRegionType` and V2 both swap the incoming server X/Y before
querying the stored map. V1 considers Water/Lava liquid; V2 additionally includes
VWater. Other region identifiers (normal, PvP, zone line, slime, ice) are not
automatically interchangeable with swimming water.

The player-profile encoder includes `air_remaining`, but zone entry explicitly
resets it to 60 (`zone/client_packet.cpp`) to avoid arrival drowning. No ongoing
authoritative breath countdown update was identified. OpenEQ does not expose
that one-time initialization as a live breath gauge.

RoF2 `DECODE(OP_EnvDamage)` takes a client-reported damage amount and type.
`Client::Handle_OP_EnvDamage` applies server modifiers/protection and rejects
falling damage when the server position is in liquid. The damage kinds are 250
lava, 251 drowning, 252 falling, and 253 trap. Packet knowledge alone does not
establish the original client's damage formula or tick schedule. No guessed
damage, drowning timer, or environmental packet generation was added here.

## Dedicated test fixtures and read-only map verification

Explorer must never be used for probes. Existing dedicated offline fixtures
at inspection time were Broker (`openeq_commerce`, PoK), Arcanist
(`openeq_spells`, Arena), and Mechanic (`openeq_gameplay`, PoK). All have account
status 250 and persisted flymode 0. Coordinate ownership before logging into
one; a second client must not displace an active fixture session.

Server maps exist under `/srv/eqemu/maps/water` for PoK, Arena, Qeynos, North
Qeynos, East/West Freeport, Ocean of Tears, Lake of Ill Omen, and Erud's Crossing.
PoK uses V1 with 3,603 BSP nodes and six Water leaves. Arena uses V2 with four
regions, all PvP/zone-line; Arena is appropriate for levitation probes but not
for proving swimming.

PoK's small northern pool supplies a bounded swimming candidate:

| Point | Server XYZ | Scene/map XYZ | Server map result |
|---|---|---|---|
| Pool interior | `[1455, 15, -132]` | `[15, 1455, -132]` | Water |
| Above surface | `[1455, 15, -125]` | `[15, 1455, -125]` | Normal |
| Below volume | `[1455, 15, -141]` | `[15, 1455, -141]` | Normal |
| Outside east side | `[1455, 35, -132]` | `[35, 1455, -132]` | Normal |
| Outside south side | `[1439, 15, -132]` | `[15, 1439, -132]` | Normal |

The relevant pool bounds are map X `(-31,31)`, Y `(1441,1471)`, Z `(-140,-126)`;
exact BSP split planes return Normal. Physical feet are server-center Z minus
3, and the camera is server-center Z plus 3. The full original scene's floor at
both interior XY coordinates is Z `-134`: use grounded server center `-131`,
feet `-134`, and camera `-128` for a live probe. The table's volume-only center
`-132` would place physical feet one unit inside that floor. The original PoK
WLD independently confirms every table row
(interior is water region 42; the mirrored west interior is region 48).
This investigation only read maps/database; it did not log
in, teleport, cast on, or modify any fixture.

For an eventual levitation probe, prefer an offline dedicated GM in Arena and
an authoritative `#castspell 261`/buff fade sequence, retaining the prior buffs
and pose for verification. `#set flymode` writes `account.flymode`, so it requires
explicit restoration even though the visible effect looks temporary. Login
itself can add the known 0.75 server-Z spawn adjustment; report that separately
instead of claiming exact pose restoration. No credentials belong in logs.

## Validation

The integrated player motion uses 120 Hz simulation ticks and samples liquid
occupancy at the six-unit body's center. WASD swimming follows view pitch;
Space ascends and either Control key descends. Full-input swimming is 60% of
walking speed (24 units/s at the ordinary 40 units/s walk setting). It holds
depth without input. Flight obeys normal body collision; the explicit developer
flight toggle retains its diagnostic behavior. Floating holds height; levitation
uses the normal jump ascent and caps downward speed at 2 units/s. Removing the
authoritative effect restores ordinary gravity. These rates are client tuning.

Only grounded riders receive lift displacement. All modes still collide with
static/dynamic walls, floors and ceilings. Camera-liquid state is separate from
body-liquid state, so third-person fog uses the actual view location. Rendering
restores the exact zone atmosphere after resurfacing. Player swimming selects
the original P06 animation when no server action overrides it.

The movement regressions cover 10/30/60/120/240 FPS boundary crossings, buoyancy,
surface exit, shore ramps, underwater doors, floor/ceiling clearance, explicit
flight/floating, levitation removal and original PoK/Kelethin geometry. The live
`swimming_smoke` probe confirmed server-accepted depth hold, horizontal travel,
ascent and descent, then restored Mechanic and checked persistent state unchanged.
Its exact fixture/outcome record is in [liquid notes](LIQUID_REGIONS.md).

Liquid occupancy is currently point-sampled at tick starts. A volume thinner
than one tick's travel can be crossed without entering swimming; segment queries
exist for a future swept transition implementation. This did not affect the
verified PoK pool. NPC water-aware terrain following is also separate future work.

Focused tests cover all known/unknown gravity modes, explicit flight priority,
both spell export layouts, actual installed spell metadata, own versus other
entity buffs, zero-display-time buffs, authoritative fade/appearance updates,
zone field offsets/sentinels, non-finite gravity, and truncated zone packets.
