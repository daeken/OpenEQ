# OpenEQ

An open-source EverQuest client in Rust, using Bevy for ECS/input and a custom
wgpu renderer. It reads your existing EverQuest assets and speaks EQEmu's RoF2
protocol. The previous C# version is in the sibling `OpenEQ-csharp` checkout.

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

- `W/A/S/D`: move; `Shift`: move faster; `Space`: jump.
- Right-click: capture the mouse for looking; `Escape`: release, then quit.
- Left-click an NPC or press `Tab`: select a target.
- `F`: toggle development free flight; `Space/Ctrl`: rise/sink in flight.

Online movement has basic floor, wall, step and ceiling collision. This is not
the complete original collision system: invisible collision-only geometry,
swimming and full movement rules still need work.

## Working now

- Login, character selection and zone handoff against the live Storage2 server.
- Server NPC spawns, movement, despawns, health and target selection; classic
  WLD characters with idle/walk skeletal animation and instanced rendering.
- Original EverQuest XML player, target and status windows, backed by an XML
  include/layout/binding engine and GPU atlas/text rendering.
- Deferred sun and authored zone lights, stable shadows, server-dependent fog,
  and original sky/cloud textures. Offline fog uses a documented PEQ snapshot.
- Classic WLD zones, binary EQGZ v1/v2 zones, and EQTZP heightmap zones including
  tile terrain, ecosystem textures and placed objects.
- Animated textures and EQG water colors, scrolling normals and cube reflections.

This is a connected exploration client, not yet a complete playable replacement.
Combat, spells, inventory interaction, doors, chat, character creation and
inter-zone travel remain unfinished. Modern EQG character models and NPC
armor/skin variants are not rendered yet. Unsupported models are reported and
skipped. The XML system still lacks some advanced widgets and rich text.
Terrain grass/water effects and full sky/weather transitions also remain.

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

cargo test --workspace
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
