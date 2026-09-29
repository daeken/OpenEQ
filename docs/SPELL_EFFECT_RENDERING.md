# Spell effect presentation

The client loads the installed spell definitions and textures on the startup
worker. `spells_us.txt` chooses the visual definition; network messages choose
when a cast begins, is interrupted, or reaches a target. Visuals never apply
buffs, subtract mana, deal damage, or delay a confirmed outcome.

Casting effects follow the current skeletal hand sockets for classic, Luclin,
and modern characters, including blends between poses. First-person casting
uses camera-relative hand locations because that view does not render the
player's body. Unknown attachment codes use the actor's body anchor. This is
an explicit compatibility fallback, not a recovered native attachment enum.

Emitter simulation uses the original rate/counts, delays, particle lifetimes,
color/opacity ramps, width/height ranges, flipbooks, spin, local acceleration,
radial/orbital motion, gravity, wind, and emitter orientation. Its axial basis
points up, with transverse axes +X and -Y. Correlated width/height reuse a random
factor; rectangular particles keep their authored aspect ratio. The two scale
flags independently select actor-size scaling for geometry and particle size.
Shapes include points, rings, disks, sphere/ellipsoid surfaces, cylinder and
cone sides, box surfaces, and torus surfaces. Regular sphere-ring stepping is
approximated by an even spherical distribution.

Item projectile packets load the authored rigid mesh using the native
`OnDemandResources.txt` mapping. The shared `missle.eqg` archive also supplies
PRT/PTS attachments; its 17 supported origin-attached missiles use the separate
`actoremittersnew.edd` table. IT11504's red flame trail uses actor emitter 472,
not emitter 472 in the spell table. These definitions and their textures load
before entering the world. Ordinary arrows retain their authored shaft/head
orientation. Particle and model flight follows the same bounded trajectory.

Particles share a bounded texture atlas, use alpha or additive blending, test
opaque depth without writing it, fade with zone fog, and draw before the UI.
Alpha particles are sorted by camera depth. Two texture arrays preserve all 512
slots even on adapters limited to 256 layers per array. Texture sheets keep
the original frame layout and clamp samples inside the selected cell.

## Lifetime and server behavior

- BeginCast starts the casting stage using the server's duration. Repeated
  casts have separate identities; interruption removes that cast's particles.
- Action type 231 starts an immediate impact. The observer message and the
  caster/target success copy are deduplicated per spell/source/target.
- A bolt arriving during another cast does not stop the new cast. An unrelated
  proc does not retire an in-flight bolt. Travel estimates cannot create impacts.
- Persistent-particle metadata requires actual buff state. Buff updates refresh
  existing emitters without restarting them; complete lists and removals clean
  them up. Explicit nimbus additions/removals are also supported.
- Despawn, death, disconnect, and zone handoff clear the relevant effects. No
  effects from the old zone survive into the destination.

Runtime caps are 256 effects, 8,192 particles, 64 projectiles, 512 particle
textures, and 32 cached projectile GPU models. A suspended window does not
replay an unbounded emission backlog. Individual particles are capped at 30
seconds and ordinary authored transients at 60 seconds. Missing assets are
omitted and counted, with no magenta replacement.

## Known fidelity limits

- The full EFF attachment/mode enum is not recovered. Paired hand IDs 4/5 use
  skeletal sockets; other IDs use a body anchor. Hand ordering and unusual
  nonhumanoid conventions still need original-client comparisons.
- First-person hands are estimated positions; a first-person arm model is a
  separate feature.
- Nonzero OP_SpellEffect finish-delay semantics remain unverified and are not
  used to invent a start delay. Those fields are preserved by the decoder.
- EQEmu does not transmit a permanence flag with its late-join nimbus replay.
  An ordinary timed SpellEffect cannot safely be treated as permanent.
- Regular sphere rings, homing projectile paths, and arc timing are
  approximations. Authoritative impact messages take precedence.
- Projectile particle attachments currently support the proven identity
  `ATTACH_TO_ORIGIN` records in `missle.eqg`. Other actor particle systems,
  transformed points, animated item skeletons, and exact additive mesh shaders
  remain incomplete; unsupported records are reported rather than guessed.
- Particle ordering against other translucent surfaces, and additive against
  alpha particles, is approximate. Particles do not cast shadows or write
  depth, including authored legacy depth-writing effects. Refraction and
  spell-driven dynamic lights are not implemented.
- Sound IDs are retained for the audio milestone. Original assets are not
  redistributed.

## Reproducible checks

```sh
cargo test -p openeq --lib spell_effects
cargo test -p openeq --test spell_effects -- --ignored --nocapture
cargo test -p openeq --test spell_projectiles -- --ignored --nocapture
cargo test -p openeq-render --test particles -- --ignored --nocapture
cargo test -p openeq-render --test spell_sockets -- --ignored --nocapture
cargo run -p openeq --bin spell_effect_smoke -- \
  "$HOME/.config/openeq/storage2-spell-credentials.json"
```

The original-asset gallery covers healing, frost, shielding, and gate casting
and impact stages, writing `/tmp/openeq-spell-families.png`. The live probe is
restricted to the dedicated Arcanist fixture. It casts an existing memorized
Minor Shielding, checks visible cast/impact particles, removes only its test
buff, interrupts a second cast, and verifies that particles disappear.

The projectile test starts from the actual RoF2 packet layout and needs no
preceding cast message. It checks the original model and flame texture, visible
motion, expiry without a fabricated impact, and writes
`/tmp/openeq-spell-projectile-trail.png`. Live first- and third-person cast,
impact and interruption checks are recorded in the protocol notes below.

See [format evidence](SPELL_EFFECT_FORMAT.md) and
[protocol evidence](SPELL_EFFECT_PROTOCOL.md) for the exact layouts and native
observations behind this implementation.
