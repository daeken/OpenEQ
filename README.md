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
| Q, X, H, C, V | Autoattack, sit/stand, hail, consider, assist |
| L | Loot targeted corpse; click items or use Loot All |
| B; Alt+1 through Alt+0 | Spellbook; cast gems 1–10 |
| Right-click gem | Open spellbook and select destination gem |
| Right-click buff | Dismiss buff |
| M; wheel over map | Map; zoom; click a landmark to mark a waypoint |
| F9 | Toggle first/third person |
| E | Use nearest door or portal within reach |
| F; Space/Ctrl in flight | Development free flight; rise/sink |

Chat supports `/say`, `/tell NAME`, `/reply`, `/group`, `/guild`, `/ooc`,
`/shout`, `/auction`, `/emote`, `/attack [on|off]`, `/sit`, `/stand`, `/hail`,
`/con`, `/assist [NAME]`, `/target NAME`, `/loot`, `/inventory`, `/loc`, `/help`
and `/quit`. Use `/cast 1` through `/cast 12`, `/book` and `/stopcast` for spells.
With the spellbook open, select a gem then a learned spell to memorize it.
Window title bars can be dragged. Escape interrupts casting before closing panels;
with no active panel or captured mouse, it exits.

## Working now

- Login, character selection, live NPC movement and authenticated zone handoffs.
- Inventory, bags, equipment, stack moves, item inspection and cursor items,
  using original icons and real server item records.
- Editable, scrollable channel chat and formatted system/combat messages.
- Target/assist/consider, autoattack, damage, deaths, corpse loot and Loot All.
- Spellbook/gems, memorization, casting, interruption, cooldown feedback, mana
  consumption, buffs and dismissal. Effects remain authoritative on EQEmu.
- Classic armor/skins/tints and held equipment; independent attack/hit/sit/death
  animation. Weighted modern EQG characters include gargoyles and Drakkin.
- Dynamic doors/portals, door collision, maps with waypoints, third-person camera.
- Original XML artwork and layout definitions for gameplay windows, with sharp
  high-density text and correctly scaled mouse hit testing.
- Deferred sun and authored zone lights, stable shadows, server-dependent fog,
  original sky/cloud textures, animated textures and EQG water materials.
- Classic WLD zones, binary EQGZ v1/v2 zones and EQTZP heightmap terrain.

This is a playable development milestone, with substantial parity work remaining:
merchants/trade/banking, group/raid management, richer quest interactions,
clickable saylinks, interactive account/character creation, authored border zone
triggers, spell particles/audio, advanced XML widgets and full swimming/movement
rules. Modern Drakkin modular armor/hair/equipment, Luclin replacements, weather,
terrain ecosystem effects and water refraction remain incomplete. Door motion
classes include approximations; dynamic collision switches to the final pose.
Some skins/layouts need more work at small window sizes. No original assets are
redistributed.

See the [completed milestone and remaining parity list](docs/PLAYABLE_CLIENT_TASKS.md).

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
- [Character rendering and equipment coverage](docs/CHARACTER_RENDERING.md)
- [Dynamic doors and collision](docs/DYNAMIC_OBJECTS.md)
