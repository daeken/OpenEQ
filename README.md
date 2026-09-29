# OpenEQ

An open-source EverQuest client in Rust, using Bevy for ECS/input and a custom
wgpu renderer. It reads your existing EverQuest assets and speaks EQEmu's RoF2
protocol. The previous C# mainline is preserved on branch
`codex/legacy-csharp-mainline-2026-09-28`; `master` is the Rust client.

## Play on the development server

The populated EQEmu world on `storage2.daeken.dev` is running with PEQ NPCs,
patrols, quests and navigation data. The private connection file selects the
`Explorer` development character in Plane of Knowledge:

```sh
cargo run -p openeq -- --connect "$HOME/.config/openeq/storage2-credentials.json"
```

Use `--dir /path/to/EverQuest` or `OPENEQ_CLIENT_DIR` to select an installation.
The default lookup includes `~/EverQuest`. Connection files contain `host`,
`login_port`, `username`, `password`, and `character`; optional `world_port`
defaults to 9000 and `server_id` selects a particular world. Keep these files
private and outside the repository.

Startup and zone travel show an animated loading screen while world, character,
and interface assets load in the background. The window remains responsive;
movement resumes once the destination is ready. Loading or connection errors
stay visible, and Escape exits the loading screen and client.

Add `--models luclin` to use the Luclin player models, including their layered
armor, hair, facial pieces and robes. Classic models remain the default; use
`--models classic` to select them explicitly. Missing replacements fall back
to the available original models. This choice persists across zone travel for
the current session.

Controls:

| Input | Action |
| --- | --- |
| W/A/S/D, Shift, Space | Move, run, jump |
| Right-click world, Escape | Mouse look; release mouse / close current panel |
| Left-click NPC, Tab, F1 | Select target, cycle nearby targets, target self |
| Enter or / | Chat; Up/Down recall history; Escape cancels |
| I | Inventory and equipment |
| Left-click item; Shift-click | Pick up/place item; move one from a stack |
| Right-click item | Open bag or inspect item |
| Scribe / Use in item inspection | Learn a scroll or activate an owned item's effect |
| /trade; /canceltrade | Invite the selected nearby player; cancel and return offers |
| Q, X, H, C, V | Autoattack, sit/stand, hail, consider, assist |
| L | Loot targeted corpse; click items or use Loot All |
| B; Alt+1 through Alt+0 | Spellbook; cast gems 1–10 |
| Right-click gem | Open spellbook and select destination gem |
| Right-click buff | Dismiss buff |
| M; wheel over map | Map; zoom; click a landmark to mark a waypoint |
| F9 | Toggle first/third person |
| E | Use nearest door or portal within reach |
| R | Open the selected merchant or banker within reach |
| Click underlined chat link | Activate a quest response or inspect a linked item |
| F; Space/Ctrl in flight | Development free flight; rise/sink |

Chat supports `/say`, `/tell NAME`, `/reply`, `/group`, `/guild`, `/ooc`,
`/shout`, `/auction`, `/emote`, `/attack [on|off]`, `/sit`, `/stand`, `/hail`,
`/con`, `/assist [NAME]`, `/target NAME`, `/loot`, `/inventory`, `/loc`, `/help`
and `/quit`. Use `/cast 1` through `/cast 12`, `/book` and `/stopcast` for spells.
Use `/merchant`, `/bank` or `/use` for NPC services. Select merchant stock to buy;
select an inventory item to sell, then choose a quantity. Bank slots use the same
pickup/place controls as inventory. Choose a coin type and amount to deposit or
withdraw; shared coin controls move platinum on the configured Storage2 server.
Groups support `/invite [NAME]`, `/accept`, `/decline`, `/leavegroup` (or
`/disband`) and `/makeleader NAME`, with invitation/member controls in the group
window. `/group` sends to the current group.
Use `/trade` with a nearby player selected, then accept the invitation on the
other client. Pick up an inventory item and click an empty slot in your offer;
choose a denomination and amount to add coins. Both players must accept the
current offer. Any addition clears both acceptance indicators. Cancel to return
offered items and coins before changing an existing offer.
Right-click a carried scroll or clickable item to inspect it, then choose
**Scribe** or **Use**. `/scribe` and `/useitem` act on that inspected owned item.
Scribing moves one scroll to an empty cursor and waits for the server to confirm
the book entry and consumption. Item effects use server cast times, charge
updates and reuse timers, including effects from inside bags.
With the spellbook open, select a gem then a learned spell to memorize it.
Window title bars can be dragged. Escape interrupts casting before closing panels;
with no active panel or captured mouse, it exits.
Walking through a supported classic zone exit requests travel automatically.
The server still checks access and chooses the arrival point. If it refuses a
crossing, back away before trying again; spawning inside a border never sends
you straight back. Combat numbers come from server hit packets; heals and resists
retain their chat messages until their separate event formats are supported.

In Kelethin, stand on a lift near its button and press E. The platform carries
you up and automatically returns, following the default five-second cycle.

## Working now

- Login, character selection, live NPC movement and authenticated zone handoffs.
- Inventory, bags, equipment, stack moves, item inspection and cursor items,
  using original icons and real server item records.
- Editable, scrollable channel chat and formatted system/combat messages.
- Merchant catalogs with server prices/stock, purchases, sales and quantities.
- Personal/shared bank items, containers and coins, with service range checks
  and persistence verified across reconnects.
- Player trade invitations, separate item/coin offers, acceptance resets,
  cancellation/refunds and server-confirmed termination.
- Clickable quest and item links; group invitations, acceptance/decline, member
  health/targeting, group chat, leaving and leadership transfer.
- Target/assist/consider, autoattack, damage, deaths, corpse loot and Loot All.
- Floating combat feedback: gold outgoing hits, red incoming hits, misses and
  named avoidance outcomes, with short lifetimes and bounded stacking.
- Spellbook/gems, memorization, casting, interruption, cooldown feedback, mana
  consumption, buffs and dismissal. Effects remain authoritative on EQEmu.
- Original spell particles for casting, impacts, buffs and nimbus effects,
  with animated hand attachments, texture sheets, soft/additive blending and fog.
  Original arrows/bolts and authored missile flame trails follow server packets.
- Scroll scribing and item effects, with owned-item validation, finite charges,
  consumable stacks, interrupted casts and shared reuse timers.
- Classic and optional Luclin armor/skins/tints and held equipment, including
  static EQG weapons/shields; independent actions with short skeletal blends.
  Drakkin support modular clothing, armor, robes, hair, facial features and
  original heritage/color palettes, including softly blended tattoos.
- Dynamic doors/portals, door collision, maps with waypoints, third-person camera.
- Natural border travel through authored classic WLD reference volumes, including
  Greater Faydark's four exits; authenticated handoffs use server destination data.
- Kelethin lift buttons, moving platform collision and passenger carrying.
- Original XML artwork and layout definitions for gameplay windows, with sharp
  high-density text and correctly scaled mouse hit testing.
- Deferred sun and authored zone lights, stable shadows, server-dependent fog,
  original sky/cloud textures, animated textures and EQG water materials.
- Classic WLD zones, binary EQGZ v1/v2 zones and EQTZP heightmap terrain.

This is a playable development milestone, with substantial parity work remaining:
NPC quest hand-ins, augmentation, raid and guild management, quest journals,
interactive account/character creation, EQG/absolute-destination border
triggers, audio, advanced XML widgets and full swimming/movement
rules. Luclin hair/beard colors, Hero's Forge, animated equipment, weather,
terrain ecosystem effects and water refraction remain incomplete. Door motion
classes include approximations; ordinary door collision switches to the final
pose, while lifts collide throughout their motion. Lift timing is approximate.
Some skins/layouts need more work at small window sizes. No original assets are
redistributed.

Spell effects have a verified first pass, with remaining attachment/mode enums,
some timing rules and translucent ordering tracked in the
[spell rendering notes](docs/SPELL_EFFECT_RENDERING.md).

The [next ten milestones](docs/NEXT_TEN_MILESTONES.md) are ordered hardest first,
with the active work, discoveries, and verification recorded as implementation
progresses.

The [rendering performance notes](docs/PERFORMANCE.md) record the PoK lighting
profile, spatial light lookup, and opt-in CPU/GPU profiling commands.

See the [exploration and combat checklist](docs/EXPLORATION_COMBAT_TASKS.md) and
[zone travel protocol](docs/ZONE_TRAVEL_PROTOCOL.md) for supported border formats
and reproducible verification.

See the [completed milestone and remaining parity list](docs/PLAYABLE_CLIENT_TASKS.md).
The [commerce and social checklist](docs/COMMERCE_SOCIAL_TASKS.md) records the
next verified milestone. Unknown purchase outcomes deliberately block additional
trades until reconnecting; the client never retries an uncertain purchase.
The [trading and item-use checklist](docs/TRADE_ITEM_USE_TASKS.md) tracks the next
milestone. See [trade protocol findings](docs/TRADE_PROTOCOL.md) and
[item-use findings](docs/ITEM_USE_PROTOCOL.md) for server behavior and live probes.

## Offline tools and verification

```sh
# Explore a zone without a server (free-flight controls).
cargo run -p openeq -- nektulos

# Inspect asset geometry.
cargo run -p openeq-assets --bin zonescan -- poknowledge

# Render an offline image, including the zone atmosphere.
cargo run -p openeq-render --bin renderzone -- anguish --out /tmp/anguish.png

# Exercise login, zone entry and real NPC movement without a window.
cargo run -p openeq-net --bin eqlogin -- \
  --config "$HOME/.config/openeq/storage2-credentials.json" --seconds 35

# Render a live zone with animated NPCs and the XML HUD to a PNG.
cargo run -p openeq --bin live_smoke -- \
  "$HOME/.config/openeq/storage2-credentials.json" /tmp/openeq-live.png

# Integrated gameplay state and presentation probe (dedicated caster fixture).
cargo run -p openeq --bin gameplay_smoke -- \
  "$HOME/.config/openeq/storage2-spell-credentials.json"

# Visible spell casting, impact and interruption (dedicated Arcanist fixture).
cargo run -p openeq --bin spell_effect_smoke -- \
  "$HOME/.config/openeq/storage2-spell-credentials.json" \
  /tmp/openeq-spell-effects --first-person

# Integrated merchant/bank roundtrip, restoring the dedicated Broker fixture.
cargo run -p openeq --bin commerce_smoke -- \
  "$HOME/.config/openeq/storage2-commerce-credentials.json"

# Two-character group and real NPC quest-link protocol proof.
cargo run -p openeq --bin social_smoke -- \
  "$HOME/.config/openeq/storage2-social1-credentials.json" \
  "$HOME/.config/openeq/storage2-social2-credentials.json"

# Player trading through the ordinary client controls, restoring both fixtures.
cargo run -p openeq --bin trade_smoke -- \
  "$HOME/.config/openeq/storage2-trade1-credentials.json" \
  "$HOME/.config/openeq/storage2-trade2-credentials.json"

# Scroll/item protocol proof; consumes dedicated fixture items (see item-use docs).
cargo run -p openeq --bin item_use_smoke -- \
  "$HOME/.config/openeq/storage2-itemuse-credentials.json"

# Actual Kelethin buttons, animated lift collision and rider physics.
cargo run -p openeq --bin lift_smoke -- \
  "$HOME/.config/openeq/storage2-commerce-credentials.json"

cargo test --workspace
cargo test --workspace -- --ignored
cargo clippy --workspace --all-targets -- -D warnings
```

Run live probes separately from the client when using the same character.
Some real-asset/GPU tests require the original installation; ignored tests
state their requirements. No original client assets are distributed here.

## Crates and documentation

| Crate | Responsibility |
| --- | --- |
| `openeq-assets` | Archives, zone/character formats, texture decoding, collision |
| `openeq-net` | Reliable UDP, login/world/zone protocol |
| `openeq-render` | Deferred rendering, animated actors, atmosphere, UI overlay |
| `openeq-ui` | Original XML definitions, layout, binding and hit testing |
| `openeq` | Bevy app, networking integration, movement and HUD |

- [Architecture](docs/ARCHITECTURE.md)
- [Storage2 setup, credentials location, operations and backups](docs/SERVER_SETUP.md)
- [Heightmap format findings and remaining limitations](docs/HEIGHTMAP_FORMAT.md)
- [XML UI coverage](crates/openeq-ui/README.md)

- [Gameplay protocol, live probes and test characters](docs/GAMEPLAY_PROTOCOL.md)
- [Merchant/bank protocol and persistence proof](docs/COMMERCE_PROTOCOL.md)
- [Groups, quest links and two-client proof](docs/SOCIAL_PROTOCOL.md)
- [Character rendering and equipment coverage](docs/CHARACTER_RENDERING.md)
- [Spell rendering and verification](docs/SPELL_EFFECT_RENDERING.md)
- [Dynamic doors and collision](docs/DYNAMIC_OBJECTS.md)
- [Kelethin buttons and lift verification](docs/LIFT_PROTOCOL.md)
