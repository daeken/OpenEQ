# OpenEQ

An open-source EverQuest client, rebuilt in Rust on [Bevy](https://bevy.org) for
the ECS, input and app loop, with a custom `wgpu` renderer.

The client is a **RoF2** client: it speaks the protocol of the Rain of Fear-era
EverQuest binary (the one distributed on Steam, `eqgame.exe` built 2016-09-19),
which is also the best-supported client in EQEmu. It connects only to EQEmu.

This is a ground-up reboot. The previous C# implementation lives on the `master`
branch of this repository and in a sibling checkout, `../OpenEQ-csharp`, for
reference.

## Status

Working today:

* **Assets** - reads `S3D`/`PFS` archives, `WLD` fragment files, `ZON`/`TER`/`MOD`
  `.eqg` zones and `DDS`/`BMP` textures, and bakes them into renderable
  geometry. 246 classic zones and 34 `EQGZ` v1 `.eqg` zones load end to end.
* **Renderer** - a deferred `wgpu` renderer: G-buffer, shadow-mapped directional
  sun, zone point lights matching the original engine's falloff, animated
  textures in a texture array.
* **Networking** - the reliable UDP session layer, the login handshake including
  its DES-encrypted credentials, the server list, world handoff and character
  select.

Not yet implemented: `EQGZ` v2 and `EQTZ` zone variants, animated character
models, collision, UI, and the zone-side of the protocol beyond entry.

## Layout

| Crate | Responsibility |
| --- | --- |
| [`openeq-assets`](crates/openeq-assets) | Legacy file formats and mesh baking |
| [`openeq-net`](crates/openeq-net) | EQEmu wire protocol |
| [`openeq-render`](crates/openeq-render) | Custom `wgpu` renderer |
| [`openeq`](crates/openeq) | Bevy application: ECS, input, camera, wiring |

## Quick start

The client reads assets straight out of an EverQuest installation; point it at
one with `--dir` or the `OPENEQ_CLIENT_DIR` environment variable.

```sh
# Fly around a zone.
cargo run -p openeq -- akanon --dir /path/to/EverQuest

# Inspect what the asset pipeline sees.
cargo run -p openeq-assets --bin zonescan -- gfaydark

# Render a zone to a PNG without opening a window.
cargo run -p openeq-render --bin renderzone -- gfaydark --out /tmp/gfaydark.png

# Isolate one texture to check materials line up with their geometry.
cargo run -p openeq-render --bin renderzone -- gfaydark --only-material nekpine

# Dump the fragment graph of a WLD file.
cargo run -p openeq-assets --bin wldscan -- /path/to/EverQuest/gfaydark.s3d

# Walk the login and world handshake against an EQEmu login server.
cargo run -p openeq-net --bin eqlogin -- --host 127.0.0.1 --user me --pass secret
```

Controls: click the window to capture the mouse, then `W`/`A`/`S`/`D` to move,
the mouse to look, `Shift` to run, `Space` and `Ctrl` to rise and sink.
`Escape` releases the mouse, and `Escape` again quits. Clicking away to another
window releases it too, so the pointer never gets stuck.

## Documentation

[`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md) covers the design decisions, the
exact quirks of the legacy formats, the wire protocol, and the roadmap.
