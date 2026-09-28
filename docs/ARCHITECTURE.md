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
from the client's first world packet (`OP_SendLoginInfo` with a 488-byte
payload). Newer zone files are present on disk because the launcher downloads
everything the account is entitled to, but a RoF2 client cannot enter them; that
is why some zones use formats this code does not read yet.

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

### `ZON` / `TER` / `MOD`

Newer zones are a `.zon` description plus `.ter` (terrain) and `.mod` (object)
meshes inside one `.eqg`. The binary `EQGZ` v1 form lists its files by name,
which is what [`zone.rs`](../crates/openeq-assets/src/zone.rs) reads.

Two further variants remain:

* `EQGZ` v2, used from roughly the Call of the Forsaken era. The archive holds a
  single `.ter` and many `.mod`, but object placement lives in the binary
  `.zon`, so its layout has to be decoded.
* `EQTZ`, a plain-text description (extents, quads per tile, covermap sizes) used
  by Rain of Fear-era zones. Those archives contain only `obj_*`/`obp_*` models
  plus a `.dat` terrain payload, with placements in a `.tog` text file.

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

Three passes per frame:

1. **Shadow map** - a 2048 squared depth map from a camera-anchored orthographic
   frustum around the directional sun.
2. **G-buffer** - albedo (sRGB) plus a view-independent normal and material
   flags.
3. **Lighting** - a fullscreen triangle reconstructs world positions from depth
   and shades with ambient, the shadow-mapped sun, and the zone's point lights.

Materials are stored per vertex rather than in a material buffer. The asset
pipeline already splits geometry per material, so this costs nothing and avoids
a dynamic-offset uniform. Textures live in one array texture, resized to 256
squared; animated flipbooks occupy consecutive layers and the shader picks a
frame from elapsed time, so multi-frame textures animate without touching
buffers.

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
character select.

## Testing

`openeq-net`'s tests cover opcode framing, the keyed CRC, DES round-trips,
sequence wraparound, in-order and out-of-order fragment reassembly, and
character-select parsing. The renderer and asset stack are verified against real
client data: `zonescan` reports per-zone statistics, and `renderzone` produces a
PNG that can be inspected directly.

## Roadmap

1. `EQGZ` v2 and `EQTZ` zone support, including `.tog` placements.
2. Animated character and object models (fragment `0x10`/`0x12`/`0x36`
   skeletons), which the WLD reader already parses but nothing builds yet.
3. Zone entry: spawns, movement updates, doors, and the in-world protocol.
4. Collision from the collidable polygon flag, replacing free flight.
5. Cascaded shadow maps, a real sky, and water/skybox surfaces.
