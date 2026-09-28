# Character rendering

Classic actors load the original WLD skeleton, rigid vertex-to-bone runs and
named animation tracks. A character library indexes global/zone archives once,
caches base models and decoded textures, and shares animation data between
appearance variants. Nothing here requires preconverted or redistributed EQ
assets.

The global `skt_chr.s3d` archive supplies race 367 (`SKT`, the LDON skeleton).
PEQ uses it for Greater Faydark's decaying skeletons even though the zone import
list omits the archive. Race 60 retains the distinct classic `SKE` model.

WLD character BMPs use bottom-origin texture rows. The character library flips
decoded BMP rows once, matching the old C# loader, before masking/tinting and
caching. DDS data already matches the model UVs and stays unchanged, including
DDS replacements whose filenames end in `.bmp`. This conversion is scoped to
character loading; ordinary zone and UI image decoding keeps its existing row
order. Actual human and gnome face variants cover the cache and tint paths.

Modern EQG actors load weighted EQGS (`.mds`) and EQGM (`.mod`) meshes, with
up to four bone weights per vertex and inverse bind matrices. EQGA (`.ani`)
tracks are matched by bone name and sampled at their authored timestamps;
missing tracks retain the bind transform. Stored EQG quaternions use the inverse
convention to the renderer and are conjugated for both bind poses and animation.
EQG archives are opened on demand and their textures are cached. Gargoyle and
male/female Drakkin race mappings use this path; classic races still prefer WLD.

`ActorState` carries race/gender, EQ position/heading/size, appearance, action and
an action sequence number. The network layer increments the sequence for each
new action, including consecutive attacks. The renderer keeps start times per
actor, returns completed one-shots to locomotion, and holds death/crouch poses.
Sitting uses P07 where available or reverses the original P02 sit-to-stand clip.
Original OP_Animation numbers map to their C/D/L/O/P/S/T clips.

Character fronts point along authored +X. Instance rotation maps that direction
to the scene bearing `[sin(heading), cos(heading)]`; no extra half-turn is needed.
An original-asset regression derives forward from the boot/toe bones of human,
gnome and barbarian skeletons and checks all four cardinal instance headings.
Front/back GPU portraits also verify the Vah Shir convention.

The renderer records world bounds from each sampled pose, including authored
root motion, scale, heading, and equipment. Nameplates sit above those bounds
in screen space; mouse targeting covers their projected extent with a small
logical-pixel margin and chooses the nearer body when targets overlap. This
matters for Greater Faydark bats: their original flight tracks put their bodies
and wings well above the bind-pose origin. Bounds disappear with the actor and
are shared by rendering, labels, and picking.

Ground NPC presentation follows nearby connected floors between movement
packets, retaining the authoritative anchor-to-floor offset. EQEmu sends zero
vertical velocity during ground travel and refreshes position about every five
seconds, so extrapolating only X/Y otherwise makes guards sink into ramps.
Short terrain probes preserve stacked platforms and leave unsupported gaps
alone. RoF2 gravity mode is retained from spawns and appearance updates: modes
0 and 3 permit terrain following; flight, levitation, floating, unknown modes,
and corpses retain their network positions. Mode 3 also permits swimming;
complete liquid-volume classification remains part of future movement work.
This presentation adjustment never alters authoritative state or player packets.

An appearance includes the nine server-visible slots: head, chest, arms, wrists,
hands, legs, feet, primary and secondary. Supported classic appearance features:

- Per-piece armor and face textures, alternate helmet meshes, and robe meshes
  with their shared CLK textures (materials 10–16).
- Packed `0xAARRGGBB` tint, where the high byte enables tint. Exposed face textures
  remain untinted. Masked BMP palette entry zero becomes transparent before tint.
- Static IT equipment from the installed `gequip*.s3d` archives, attached to
  R_POINT/L_POINT skeletal bones. Explicit shield texture names select the
  authored SHIELD_POINT when present. Other offhand items remain in the hand.
- Body size stays constant when helmets, robes or weapons change.

Actors sharing an appearance and current sampled pose are instanced. Each
appearance has reusable pose slots, expanded only when additional concurrent
poses are required; texture and index buffers are not recreated each frame.
Unused appearance batches expire after 30 seconds. Ignored hero/elite fields,
missing item requests and no-op tints are removed from appearance keys.

Modern appearance currently selects the requested MDS body/head variant and
available `_sNN_` diffuse texture variants. Drakkin base bodies and animations
render, but their modular armor, hair/facial pieces and equipment attachments
are not assembled. Classic equipment/tint fields do not affect modern actors.
Unused source materials, including editor grid placeholders, are excluded.

Current limits: Luclin replacement character conventions, EQG equipment,
animated item skeletons/particles and Hero's Forge geometry are not supported. Shield detection
from texture names is conservative; unusual unnamed shields may use the hand
point. Alternate NPC mesh conventions beyond classic head/robe selection are
incomplete. Action blending between separate clips is not implemented.

Verification:

```sh
cargo test -p openeq-assets --test characters
cargo test -p openeq-render actors::tests::render_equipment -- --ignored --nocapture
cargo test -p openeq-render modern_gpu_tests -- --ignored --nocapture
cargo test -p openeq-render authored_toes_face_the_rendered_heading -- --ignored --nocapture
cargo test -p openeq --lib actual_kelethin_guard -- --ignored --nocapture
cargo test -p openeq --test npc_presentation -- --ignored --nocapture
cargo run -p openeq-assets --bin characterscan -- HUM
cargo run -p openeq-assets --bin characterscan -- IT201
```

The GPU test uses original assets, exercises equipment/tint/independent actions,
checks for magenta pixels, and writes `/tmp/openeq-characters.png`. Set
`OPENEQ_CHARACTER_RENDER_OUT` to change the output path. It also verifies action
restarts, pose-slot reuse, cache expiry and removal of disappeared actors.
The weighted-character asset test loads Gargoyle, both Drakkin genders, BDR and
ONM, checking animated vertices, stable topology and every referenced diffuse
texture. The modern GPU test writes `/tmp/openeq-modern-characters.png` with
Gargoyle and both Drakkin genders on an original PoK elevator platform.

The read-only `npc_facing_probe` compares consecutive authoritative movement
packets with server and scene bearings. A 2026-09-28 Storage2 run observed 48
stable straight-line pairs, all forward and none reversed (median error 0.177°).
Savage Lord Etherat and Oracle Maeth use PEQ patrol grids 18 and 19: their room
endpoints pause for 600–900 seconds with heading `-1`, retaining their direction
of travel. They can therefore face a corner for several minutes without a
client rotation error. No world database headings were changed.

```sh
cargo run -p openeq --bin npc_facing_probe -- \
  "$HOME/.config/openeq/storage2-trade1-credentials.json" 75 --walls
```
