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

Drakkin body/module UVs invert V to match their DDS landmarks. Other weighted
model families and rigid EQG items retain their own UV conventions. This is
separate from the classic BMP row conversion above; no shared DDS decode rule
is changed. Original nose/eyebrow vertices and the shield-tip texture provide
regressions for the differing conventions.

`--models luclin` selects replacement globals for the fourteen supported
player-race families while retaining classic fallbacks. Their `chr2` track
patches load before the base archives. Four-character A/B clips are kept and
aliased to the ordinary action codes; classic tracks never substitute for a
replacement skeleton. Body scale uses the naked standing pose, since Luclin
rig construction poses extend their feet below the standing body.

Luclin materials reference actual skin/clothing alpha layers, sometimes with
texture numbers different from the requested material. The loader resolves the
authored MDF variant and composites layers, tinting clothing rather than skin.
Separate hair, beard, helm, tunic, robe and plate modules remap their bone
bindings into the body skeleton. Faces, independent eye colors, Erudite glyphs
and Barbarian tattoo selectors use their original definitions.

`ActorState` carries race/gender, EQ position/heading/size, appearance, action and
an action sequence number. The network layer increments the sequence for each
new action, including consecutive attacks. The renderer keeps start times per
actor, returns completed one-shots to locomotion, and holds death/crouch poses.
Sitting uses P07 where available or reverses the original P02 sit-to-stand clip.
Original OP_Animation numbers map to their C/D/L/O/P/S/T clips.

Changes between locomotion, actions and completed one-shots blend for five
30-Hz steps. Local bone translations/scales interpolate and rotations slerp
before composing the hierarchy. Held items share those transforms. Repeated
actions restart independently, and equal sampled blends still share geometry;
model or appearance changes discard stale transition endpoints.

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

Classic characters also load the original `global17_amr.s3d` through
`global23_amr.s3d` extended armor archives. They are shared appearance assets,
not Luclin-only replacements. For example, Plane of Knowledge's Tratlan Matrick
uses male wood elf skin 20/face 2, Higwyn Matrick uses skin 21/face 3, and Sherin
Matrick uses female skin 21/face 1. Missing these archives previously left their
bodies wearing the default skin despite the server requesting those outfits.
Explicit equipment still overrides each body slot, and unequipping restores the
requested NPC outfit. Default player appearances retain their original skin 0.

Actors sharing an appearance and current sampled pose are instanced. Each
appearance has reusable pose slots, expanded only when additional concurrent
poses are required; texture and index buffers are not recreated each frame.
Unused appearance batches expire after 30 seconds. Race-aware normalization
removes unsupported fields, missing item requests and no-op tints without
discarding supported modern appearance variants.

Modern appearance selects the requested MDS body/head variant and available
`_sNN_` diffuse texture variants. Drakkin assemble their separate weighted armor,
clothing, hair, facial features and tattoo modules. Module bone indices differ
from the body: names and subtree context establish the mapping, and original
bind matrices verify it. Face and both eyes select independent textures. Robe
materials 10–16 share authored geometry with seven texture variants. Gear does
not change the body's scale or center. Unused source materials, including editor
grid placeholders, are excluded.

`Resources/playercustomization.txt` supplies Drakkin heritage base colors and
gender-specific hair/beard shade lists. The reusable customization catalog keys
records by race, heritage and gender. Module provenance distinguishes hair from
facial detail even when their material names are identical; source skin, alpha
and normal maps remain unchanged. Missing or invalid metadata retains original
textures. Its modern human record is not a verified Luclin tint palette.

Fractional-alpha character materials use weighted blended transparency after
opaque lighting and before the UI. They share sun, zone lights, shadows and fog
with opaque surfaces. Fully opaque texels still write depth and cast shadows;
soft tattoo and hair edges blend without disappearing at the old cutout
threshold. Overlapping transparent layers are approximate, with no translucent
shadow transmission or refraction. The existing water path remains separate.

Rigid WLD and static `itNNN.eqg` equipment can attach to classic, Luclin and
Drakkin hands/shields. Socket lookup matches the actual socket name; suffix-only
matching incorrectly treats Luclin `HAIR_POINT` as `R_POINT`. EQG item skeletons
are rejected explicitly rather than rendering their unskinned bind geometry.

RoF2 spawn, illusion and server face-change packets preserve hair/beard/eye
features and Drakkin values through LiveWorld to rendering. Illusions preserve
eyes, which RoF2 does not transmit in that packet, and keep armor when the wire
value is 255. Separate wear changes remain authoritative for equipment.

Current limits: Luclin hair/beard tint palettes, animated item
skeletons/particles and Hero's Forge geometry are not supported. Shield detection
from texture names is conservative; unusual unnamed shields may use the hand
point. Alternate NPC mesh conventions beyond classic head/robe selection are
incomplete. Remaining player/NPC grounding differences are tracked with movement
work; the model gallery's synthetic anchor is not proof of server floor alignment.

Verification:

```sh
cargo test -p openeq-assets --test characters
cargo test -p openeq-assets --test modern_characters
cargo test -p openeq-assets --test luclin_characters
cargo test -p openeq-assets --test drakkin_customization
cargo test -p openeq-render --test character_appearances -- --ignored --nocapture
cargo test -p openeq-render --test transparency -- --ignored --nocapture
cargo test -p openeq-render actors::tests::render_equipment -- --ignored --nocapture
cargo test -p openeq-render modern_gpu_tests -- --ignored --nocapture
cargo test -p openeq-render authored_toes_face_the_rendered_heading -- --ignored --nocapture
cargo test -p openeq --lib actual_kelethin_guard -- --ignored --nocapture
cargo test -p openeq --test npc_presentation -- --ignored --nocapture
cargo run -p openeq-assets --bin characterscan -- HUM
cargo run -p openeq-assets --bin characterscan -- HUM --luclin
cargo run -p openeq-assets --bin characterscan -- IT201
```

The `original_classic_matrick_outfits_and_equipment_render` gallery writes
`/tmp/openeq-matrick-outfits.png`, compares the two reported NPC outfits with
default clothing, and verifies that an equipment change can be reversed without
altering the restored image.

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
