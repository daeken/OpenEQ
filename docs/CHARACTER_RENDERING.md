# Character rendering

Classic actors load the original WLD skeleton, rigid vertex-to-bone runs and
named animation tracks. A character library indexes global/zone archives once,
caches base models and decoded textures, and shares animation data between
appearance variants. Nothing here requires preconverted or redistributed EQ
assets.

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
