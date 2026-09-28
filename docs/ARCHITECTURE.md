# Architecture

## Goals and constraints

The client must be faithful to the original EverQuest client where that is
observable, must talk to EQEmu, and must leave room for modern rendering -
deferred shading, real-time shadows - that the original never had.

Bevy supplies the application loop, ECS, timing, transforms and input. It does
*not* supply rendering: `openeq-render` owns its own `wgpu` device, swapchain and
passes. Bevy is built with `default-features = false` and no render features, so
`bevy_render`, `bevy_pbr` and friends are never compiled in.

The renderer takes the window's raw handle out of Bevy's `winit` window and
creates a surface from it. The system that does this takes a `NonSendMarker`,
because the window handle lives in a thread-local that only the event-loop
thread may touch.

## Target client

The assets in a typical installation come from a mid-2016 build whose binary
speaks the Rain of Fear-era protocol, which EQEmu calls **RoF2** and identifies
from the client's first world packet (`OP_SendLoginInfo` with a 464-byte
payload). EQEmu's struct has a stale 488-byte offset comment; its packed field
sizes sum to 464, which is the signature checked by the live server. Asset
format support is independent of this network protocol choice.

## Asset formats

### `S3D` / `PFS` archives

```text
0x00 u32  directory offset
0x04 u32  magic "PFS "
dir   u32  entry count
      (u32 crc, u32 offset, u32 size) * count
```

Three things are easy to get wrong, and are handled in
[`pfs.rs`](../crates/openeq-assets/src/pfs.rs):

* The entry table is in **arbitrary order**, but the directory lists names in
  **ascending offset** order. Names must be matched to entries after sorting by
  offset. Matching by table position instead silently attaches the wrong payload
  to every name: it parses fine and renders nonsense.
* A payload is a sequence of blocks, `(u32 compressed_len, u32 inflated_len,
  bytes...)`, where each block is a *complete zlib stream* including its header
  and trailer, and `compressed_len` spans that whole stream.
* Textures inside `.s3d` files keep `.bmp` names but often contain `DDS` data,
  so decoding sniffs the magic rather than trusting the extension.

### `WLD` fragments

A `WLD` is a flat list of typed fragments joined by signed 32-bit references: a
positive value `n` is fragment `n - 1`, and a negative value `-n` is the string
at byte offset `n` of the decoded string table. That table, and the texture names
inside `0x03` fragments, are XOR-obfuscated with the same 8-byte key; the key
counter restarts for each texture name.

References may point **forward**, so
[`wld.rs`](../crates/openeq-assets/src/wld.rs) parses every fragment first and
resolves references afterwards. The C# implementation resolved eagerly and
silently dropped forward references.

One fragment layout differs from both the documentation and the C# reader: in a
`0x30` material, the texture reference sits *before* the optional trailing pair,
not after. Reading it the other way yields a reference of zero for every classic
zone material, which shows up as "no textures at all".

Those zero references matter in a second way. A material with no texture is
still an entry in its fragment's material list, and polygon runs address that
list *positionally*. Dropping empty materials from the resolved list shifts
every material after them, so a trunk ends up wearing the canopy's texture while
the canopy gets whatever came next. The list is therefore built with an entry
for every material, textured or not, and any run that still points out of range
is skipped with a warning instead of being guessed at.

A WLD material whose render method is zero is invisible, even when it references
a texture such as `COLLIDE.DDS`. Its material-list slot is preserved during
resolution, but its polygons are excluded from the drawable bake. This is
independent of the polygon's collision flag: ordinary visible floors and walls
are also collidable. The original WLD geometry retains the invisible polygons
for future collision support.

Texture animation frames come from the references in a `0x04` fragment. Each
referenced `0x03` bitmap can contain several texture layers: the diffuse image
first, then optional detail maps. Only the diffuse layer is currently rendered.
Flattening these layers into animation frames made Plane of Knowledge's static
cliffs alternate between their diffuse image and a magenta placeholder for
`CLIFFROCK02.DDS_DETAIL_4.000000`.

### `ZON` / `TER` / `MOD`

Newer zones are a `.zon` description plus `.ter` (terrain) and `.mod` (object)
meshes inside one `.eqg`. The binary `EQGZ` v1 form lists its files by name,
which is what [`zone.rs`](../crates/openeq-assets/src/zone.rs) reads.

`Opaque_MaxWater.fx` materials carry two water colors, Fresnel settings and a
reflection color in addition to texture names. Anguish's water names an absent
`ra_watertest_c_01.dds`; for this shader only, a missing diffuse map falls back
to the client's shared `water_c.bmp`. Texture lookup prefers the zone archive,
then loose textures in the client directory, `Resources`, and
`Resources/waterswap`. The shared `water_n.dds` uses packed ARGB4444 rather than
DXT compression; `water_e.dds` is a six-face DXT cubemap with a mip chain after
each face. Both layouts are decoded explicitly.

The heightmap work is documented in the terrain module and its format notes.
The loader distinguishes binary EQGZ descriptions from EQTZP text descriptions;
heightmaps combine tile elevation, quad flags, ecosystem masks and object groups.

### Characters

`CharacterLibrary` indexes global, zone and imported character archives. It
resolves WLD skeletons and vertex-to-bone ranges, including shared humanoid
animation tracks. Packed tracks use a fixed translation divisor of 256 and an
independent scale field; float tracks have their own frame layout. Quaternion
interpolation skins the original geometry without changing its topology.

Actor rendering caches normalized appearances, decoded textures and reusable pose
slots. Independent timelines restart repeated server actions and return one-shot
animations to locomotion. Matching appearance/pose pairs use instancing. Classic
armor/face textures, tints, helmets, robes and held IT equipment are supported;
weapons follow authored skeleton attachment points. Modern EQGS/EQGM weighted
meshes use inverse bind matrices and EQGA timed tracks, including gargoyles and
Drakkin. Drakkin modular clothing/hair/attachments remain incomplete.

The game predicts sparse NPC patrol updates using EQEmu's heading and speed
convention, smooths small corrections over 150 ms, snaps teleports and stops
predicting after six seconds without fresh motion data. Classic humanoids face
+X in source data; heading conversion includes an initial +90° rotation. Spawn
size normalizes the bind-pose height. See [character details](CHARACTER_RENDERING.md).

### Coordinate system

EverQuest is Z-up with north along +Y. The renderer converts in the vertex shader
with a fixed matrix mapping `(x, y, z)` to `(x, z, -y)`, which preserves
handedness, puts up along +Y and north along -Z, matching `wgpu`'s conventions.
Doing the conversion on the GPU keeps the asset data faithful to the original.

Placement rotations are a separate trap. Both zone formats store them as
`(around Z, around Y, around X)`, composed as `Rz * Ry * Rx`. Applying those
components to the matching-looking axes instead turns a tree's yaw into a lean:
in Greater Faydark the median placed object ends up rotated 90 degrees, lying
flat. The readers normalise every placement into a plain `(X, Y, Z)` triple so
one helper, with named per-axis angles, serves both formats.

## Renderer

Four passes per frame:

1. **Shadow map** - a 2048 squared depth map from a camera-anchored orthographic
   frustum around the directional sun.
2. **G-buffer** - albedo (sRGB) plus a view-independent normal and material
   flags.
3. **Lighting** - a fullscreen triangle reconstructs world positions from depth
   and shades with ambient, the shadow-mapped sun, and the zone's point lights.
   It applies world-distance fog and an orientation-anchored sky dome.
4. **UI** - alpha-blended original XML atlas images and text over the lit scene.

Common material flags and animation settings are stored per vertex. EQG water
also indexes a small material buffer for its colors, Fresnel settings and map
layers. Textures live in one array texture, resized to 256 squared; animated
flipbooks occupy consecutive layers and the shader picks a frame from elapsed
time, so multi-frame textures animate without touching buffers.

Water uses two scrolling normal-map samples, blends the authored water colors
and samples all six faces of the client's environment map for Fresnel reflection.
The atlas has a linear view for normal data and an sRGB view for color, plus mip
levels to filter the distant ripples. This is an opaque water approximation;
refraction, depth-based transparency and reflections of live scene geometry are
not implemented.

The array needs one layer per texture, and zones are texture-hungry: Plane of
Knowledge references around 480 distinct textures, past the default cap of 256
array layers. The renderer asks for the adapter's real layer limit and fails
with a named error if a zone still exceeds it, rather than aborting inside the
driver. A material's frames have to stay in consecutive layers because the
shader addresses them as `base + frame`, so layers are reused only when an
existing run is already consecutive.

Zone light falloff is `pow(1 - distance / radius, 3)` scaled by `N dot L`, the
same curve the original engine's deferred pathway used.

### Shadow details that matter here

The shadow camera sits **toward the sun** from the focus point, matching the
surface-to-light direction used in lighting. Reversing that sign views the scene
from below: even an empty horizontal plane shadows itself and reveals the
camera-following map rectangle. The projection's center is snapped to texels in
light space, keeping its sampling grid stable under camera movement. Shadow
coverage still uses one local map with an edge fade, not cascades.

EverQuest's assets break three assumptions a textbook shadow pass makes, and
each one produces a very visible artifact:

* **Foliage is cut out, not solid.** Trees are cross-plane quads with a texture
  whose transparent texels carry the silhouette. A depth-only shadow pass with
  no fragment work makes every one of those quads cast a solid slab, which reads
  as broad diagonal bands across whatever is underneath. The shadow shader
  samples the atlas and discards below the same alpha threshold the G-buffer
  uses, so canopies cast dappled shade.
* **Surfaces are single-sided.** Culling back faces in the shadow pass means a
  wall facing away from the sun occludes nothing, and light leaks through it.
  The shadow pass disables culling so every surface casts.
* **Large flat surfaces self-shadow.** Platforms, floors and roofs are big,
  coplanar, and often nearly edge-on to the light, so they stripe with acne. The
  lighting pass offsets the receiver along its own normal by a couple of shadow
  texels before projecting into light space, and the shadow pipeline adds a
  slope-scaled depth bias on top. A 3x3 comparison tap softens what is left, and
  the shadow contribution fades out over the last few percent of the map so its
  boundary is not a line drawn across the world.

`Renderer` can target a window surface or an offscreen texture, which is what
makes `renderzone` possible: the whole pipeline runs headlessly and the result is
read back as a PNG.

## Networking

```text
protocol packet:    [opcode u16 BE][sequence u16 BE][payload][crc u16 BE]
application packet: [opcode u16 LE][payload]
```

Protocol opcodes are big-endian; application opcodes are little-endian with one
quirk: an opcode whose low byte is zero gains a leading `0x00`, so `0x4200` goes
out as `00 00 42`. That is why a decoder must inspect the first two bytes before
trusting the payload offset.

The checksum is EQEmu's keyed CRC-32 truncated to 16 bits: the key is mixed in
little-endian byte order, then the packet bytes, and the result is complemented.

[`stream.rs`](../crates/openeq-net/src/stream.rs) implements the session:
handshake, sequencing with wrap-aware comparison, acknowledgements,
retransmission, `Combined` subpackets and fragment reassembly, including
fragments that arrive out of order.

Above that, [`login.rs`](../crates/openeq-net/src/login.rs) performs the
`SessionReady` / `ChatMessage` / `Login` / `LoginAccepted` exchange (credentials
DES-CBC encrypted under an all-zero key, as EQEmu's `eqcrypt_block` expects),
then the server list and the play request.
[`world.rs`](../crates/openeq-net/src/world.rs) sends the fixed-size login
handoff, which is what makes EQEmu recognise the client as RoF2, and parses
character select, then sends EnterWorld and consumes the zone address.

`zone.rs` completes the RoF2 entry sequence and decodes variable-length spawns,
24-byte packed movement updates, despawns, health, time and NewZone atmosphere.
Positions use signed 19-bit coordinates /8, velocity /64, and heading /4.
The client publishes movement at 10 Hz and sends target selection to the server.
Reliable fragments are acknowledged individually so large profiles cannot stall
behind the server's send window. Compression is negotiated, CRC checked before
inflation, and direct/unreliable application packets are handled both standalone
and inside Combined packets. Dropping a stream aborts its reader and ticker.

`LiveWorld` runs networking on a background Tokio runtime and transfers events
to the Bevy loop. Connection JSON files carry credentials; diagnostic output
never prints passwords or session keys. There is no interactive character
creation or account setup flow yet.

## Player movement and collision

Online movement uses a 120 Hz fixed simulation step, with gravity, jumping,
grounded stepping and sliding along walls. A spatial grid indexes collidable
triangles from the drawable zone and placed objects; floor selection handles
stacked decks and slopes, and ceiling checks limit upward motion. Invisible
collision-only geometry and swimming remain unsupported. A second collision
world contains server-owned doors in their current final poses, rebuilt on state
changes. Visual door transforms interpolate independently. The third-person
camera clips its sight segment against static and door triangles on either side,
rather than using the walking cylinder solver, so it cannot slide under slopes.
Networking always uses the player position.
The camera sits six units above the player's feet; outgoing server positions
use a center three units above the feet. Developer flight bypasses collision.

## Atmosphere and XML UI

Live fog colors/ranges and indoor/sky flags come from `OP_NewZone`. The offline
viewer uses a versioned PEQ snapshot with provenance in the assets data folder.
Original `Resources/sky` INI files resolve zone weather to sky color maps and
cloud textures, sampled at the server hour. The current implementation draws
the first cloud layer; full weather transitions, celestial objects and backdrop
geometry remain future work. Fog affects emissive materials as well as lit ones.
The directional sun remains fixed while point lights use authored zone data.

`openeq-ui` loads the original SIDL XML include graph, definitions, animation
atlases and widgets into renderer-independent draw commands and hit targets.
The GPU overlay clips and blends images and text. Game bindings supply all
state: XML files cannot execute scripts or network commands. The live HUD binds player/target resources, colored chat, inventory/equipment,
bags, loot, spells, buffs and casting. Logical-pixel layout/hits are independent
of framebuffer scale; images, clips and glyph rasterization use the actual
physical density. Windows can be dragged; advanced XML widgets and STML remain
incomplete.

`chat.rs` owns Unicode-safe text editing and slash-command parsing.
`interaction.rs` translates UI intentions into typed network commands and builds
presentation models. `game.rs` reduces server gameplay events into inventory,
resources, chat, loot and spell state. `live.rs` coordinates the network worker,
entity actions, target state, door state and zone transitions. The UI does not
send packets or infer inventory addresses from XML control IDs.

Successful EQEmu item moves have no acknowledgment. The reducer applies a move
after network submission and accepts later server corrections; tests verify
persistence after reconnect. Loot delivery supplies its actual destination slot.
Cooldowns begin on successful action-3 spell notifications, not spellbar unlocks,
which also arrive for failed casts. See [protocol details](GAMEPLAY_PROTOCOL.md).

During a zone handoff the worker retains authentication and suppresses movement;
old actors, inventory and targets are cleared. Fresh server environment data
selects the new geometry, character/door library, collision, map and atmosphere.
Movement resumes only with a fresh player position. Server portals and requested
travel work; authored border zone-line detection remains unfinished.

Original client map files are parsed independently of the XML UI. Lines and
landmarks use the original `(-X,-Y,Z)` map convention and are projected around the
player with clipped drawing, zoom, target marker and waypoints.

## Testing

`openeq-net`'s tests cover opcode framing, the keyed CRC, DES round-trips,
sequence wraparound, in-order and out-of-order fragment reassembly, and
character-select parsing. The renderer and asset stack are verified against real
client data: `zonescan` reports per-zone statistics, and `renderzone` produces a
PNG that can be inspected directly.

## Remaining work

- Merchants, banking, trading, richer quest links, group/raid management and UCS.
- Interactive login/character creation and complete XML/STML widgets.
- Authored zone-line triggers and exact special door/platform motion.
- Luclin replacements, modular modern appearance, item/AA casting, particles/audio.
- Full original collision volumes and movement rules, including swimming.
- Cascaded shadows, animated sun, complete weather and water refraction.
- Remaining terrain ecosystem effects, procedural vegetation and tile water.

The [milestone checklist](PLAYABLE_CLIENT_TASKS.md) records implemented scope and
live verification rather than treating scaffolding as parity.
