# Next ten milestones

Difficulty order, hardest first. Work proceeds in this order; discoveries and
unfinished compatibility work stay visible here rather than silently becoming
claims of full parity. This builds on the already working client; it does not
replace the completed gameplay, commerce, trade, and exploration checklists.

## 1. Character models — first pass complete

Luclin replacement models, modern armor/hair/facial appearance, held equipment,
and transitions between animations. Keep classic models available. Preserve
appearance updates from the server through rendering, avoid duplicate model
loads, and maintain stable body scale and target bounds when equipment changes.

Verification: original-asset appearance comparisons, animated attachments,
independent repeated actions, GPU captures, and source-backed protocol tests.
Document unsupported appearance families explicitly (including Hero's Forge).

## 2. Spell effects — first pass complete

Decode the installed effect definitions and connect cast, projectile, impact,
and persistent effects to server events. Support bone attachment points,
bounded particle lifetimes, cancellation, and cleanup on zoning/despawn.
Verify recognizable effects from several different spell families.

## 3. Movement and liquids — queued

Identify liquid volumes, support swimming and underwater movement, apply
server levitation/gravity rules, and strengthen slopes and moving-platform
transitions. Test forest floors, Kelethin ramp/lift seams, stacked platforms,
water entry/exit, and zoning without compromising server authority.

## 4. Death and recovery — queued

Complete the player death → respawn/bind → corpse → resurrection loop and
recover cleanly from disconnects during it. Use a dedicated test character;
verify inventories, experience, corpse identity, and server-confirmed outcomes.

## 5. UI compatibility — queued

Expand original XML widgets and rich text, consistent focus/window stacking,
saved window positions and layouts, and scaling on small and high-density
screens. Verify with actual client layouts and interactive controls.

## 6. Audio — queued

Read installed sounds and zone emitters; connect combat/spell sounds, footsteps,
ambience, and music with volume controls and zone cleanup. Verify distance,
looping, and missing-asset fallback without bundling original assets.

## 7. Account flow — queued

Interactive login, server and character selection, and character creation with
clear connection errors. Keep saved credentials private; use server race/class
and starting-zone rules. Verify creation and reconnect with a dedicated account.

## 8. Social systems — queued

Guild and raid state/controls and custom chat channels. Build on existing group
and channel chat. Verify membership changes, invitations, permissions, and
reconnect state with isolated fixtures.

## 9. Quest interaction — queued

NPC item/coin hand-ins and quest journal integration. Preserve server authority,
returned items, cancellation, and uncertain outcomes. Verify a real PEQ quest
with dedicated items and characters.

## 10. Progression controls — queued

Experience feedback, skill display, trainer interactions, and configurable
hotbuttons. Show server-confirmed gains and costs and persist user controls.
Verify ordinary play and reconnect without altering the user's character.

## Discoveries

- 2026-09-28, PoK profiling: the shared surface shader checked all 620 zone
  lights for every shaded pixel. At the Mac's scaled maximized resolution
  (3420×2074), lighting dominated a 107 ms GPU frame. A conservative spatial
  lookup preserves the original lights and pixels, reducing that view to
  6.2 ms GPU time. CPU pose sampling/uploads and draw submission are now the
  next bottlenecks, especially with Luclin models. Reproduction and measurement
  limits are recorded in [the performance notes](PERFORMANCE.md).

- 2026-09-28, character models: installed male/female Drakkin base models use
  EQGM v3. Their archives contain 83/75 weighted modules plus the base body.
  These share model-space bind coordinates but have different bone indices and
  subtrees. Assembly must remap weights by bone name and attachment context;
  concatenating bone indices would corrupt animation.
- 2026-09-28, appearances: RoF2 spawns include six hair/beard/eye bytes and three
  Drakkin fields that the client previously skipped. Illusion packets omit eye
  colors, while face-change packets carry them. Preserve existing eye colors
  when applying illusions.
- 2026-09-28, Luclin: replacement player models still use WLD skeletons, but
  animation names add A/B suffixes. Their armor material definitions can point
  at a differently numbered texture, so replacing digits in a filename is not
  sufficient. Replacement and classic tracks must remain separated.
- 2026-09-28, equipment: standalone `itNNN.eqg` archives contain static EQGM
  items as well as newer asset families. Rigid items can use the same authored
  hand-grip attachment path as classic IT weapons. Animated item skeletons and
  their emitters require separate support.
- 2026-09-28, visual verification: Drakkin nose/eyebrow vertices address their
  DDS landmarks at `1 - V`, not `V`. Drakkin UVs require a vertical correction
  distinct from the earlier classic BMP row correction; other EQG model and
  item families retain their own conventions.
  This was visible in portraits even though geometry and missing-texture tests
  passed, so both kinds of verification remain necessary.
- 2026-09-28, Luclin attachments: matching the suffix `R_POINT_TRACK` also
  matches `HAIR_POINT_TRACK`. The socket selector must match the full named
  socket (allowing a skeleton donor prefix), or right-hand weapons attach to
  the character's hair.
- 2026-09-28, Drakkin colors: `Resources/playercustomization.txt` contains all
  six heritage base colors, gender-specific feature counts, and the original
  hair shade lists. It resolves the earlier missing-palette question for
  Drakkin. Its race-1 record does not match Luclin feature counts and is not
  evidence for applying that palette to the older models.
- 2026-09-28, transparency: some authored Drakkin tattoo textures have almost
  all visible alpha below 0.5. A cutout renderer makes them disappear even when
  the right geometry, textures, and UVs are loaded. Soft alpha support is part
  of finishing their appearance rendering. Weighted blended transparency now
  preserves these soft edges with zone lighting/fog and opaque depth occlusion.
- 2026-09-28, spell definitions: `spellsnew.eff` has 679 three-stage effects;
  `spellsnew.edd` has 2,587 emitters. Native DLL behavior establishes the timing,
  shapes, colors, texture-sheet layout and 512-unit angular convention. The
  first emitter axis points up, not forward. Negative emitter durations force
  an authored lifetime; that field is not gravity.
- 2026-09-28, spell messages: nearby observers see Action flag 0; caster/target
  also receive flag 4 on success. Requiring flag 4 hides others' spells, while
  playing both messages doubles the effect. A projectile landing must not
  cancel a new cast by its source.
- 2026-09-28, spell textures: native lookup searches SpellEffects,
  EnvEmitterEffects, then ActorEffects. The original spell table resolves 400
  textures, with 80 authored filenames absent from this installation. Core
  spells use slots above 256, so a single default-sized texture array silently
  loses them; the renderer now supports all 512 slots with two arrays.
- 2026-09-28, projectiles: a visual travel type does not mean a server-tracked
  bolt. Actual bolts use item projectiles; `OnDemandResources.txt` maps IT11504
  to the shared `missle.eqg` archive. Its PRT attaches actor emitter 472,
  `missle_fire_red`; the visible model and flame trail are separate assets.
- 2026-09-28, movement follow-up: EQEmu deliberately adds six packed units to
  the player's ZoneEntry Z, decoded as +0.75. Sending that lifted position back
  explains the dedicated fixtures' small height drift across logins. Correct
  the presentation/network anchor distinction in milestone 3.

## Follow-ups found during implementation

- Verify coverage of Hero's Forge and animated/particle equipment separately
  from ordinary armor and rigid held items; these are distinct asset families.
- Luclin hair/beard color still needs its original tint palette. Preserve the
  server fields, but omit them from render cache keys until they have a visual
  effect; do not substitute the unrelated modern human color record.
- Ground anchors differ from rendered body centers. Original-asset galleries
  exposed residual standing-height differences; movement work should verify
  actual server floor alignment for both PCs and NPCs, separately from model
  scale and modular-attachment correctness.
- Soft transparency approximates overlapping layers and does not yet transmit
  colored shadows or refract the scene.

## Verification record

The published starting point is `0a2d336` (natural zone travel and combat
feedback).

### Character models, 2026-09-28

Completed the scoped first pass: optional Luclin models, modular Drakkin
appearances and authored heritage/hair/beard palettes, static EQG held items,
server appearance updates, skeletal animation transitions, and soft alpha.
Classic models remain the default; try `--models luclin` for replacements.

- Full workspace run including original-asset and GPU tests: 284 passed,
  zero failed or ignored. Strict workspace Clippy passed.
- GPU appearance gallery: 76 comparisons, inspected armor, faces, all six
  Drakkin heritages, Luclin modules, and WLD/EQG held equipment.
- Transparency checks cover original tattoo alpha, overlap order, occlusion,
  resizing, UI layering, colored lights, fog, and the existing water path.
- Dedicated Broker live probe: 322 entities, 313 rendered NPCs, 28 server
  movement updates and 96 interpolated movement frames. Explorer was untouched.
  Broker was offline afterward with XY/heading, items, money and resources
  unchanged; the server adjusted spawn Z from -93 to -92.25 during login.

Remaining limits: Luclin hair/beard tint palettes, Hero's Forge, animated item
skeletons/particles, some alternate NPC mesh conventions, approximate overlapping
transparency, and the grounding follow-up above. This is not full model parity.

### Spell effects, 2026-09-28 (final live runs 2026-09-29 UTC)

Completed the scoped first pass: original casting and impact emitters, animated
hand attachments, buff/nimbus lifetimes, server-driven item projectiles and
native missile flame trails, alpha/additive rendering, fog and depth occlusion.
The client loads effect assets during its responsive startup sequence.

- Full workspace run including original-asset and GPU tests: 326 passed,
  zero failed or ignored. Strict workspace Clippy passed.
- Original-asset GPU comparisons cover heal, frost, shielding and gate casting
  and impact; the actual red missile model and fire texture move and expire
  without inventing an impact. All 17 native missile models/attachments resolve.
- Texture tests exercise the entire catalog and indices 255, 256 and 511 on
  a device limited to 256 layers per array. No core effect textures are dropped.
- Live Minor Shielding on dedicated Arcanist passed in third- and first-person:
  cast and impact particles were visible; interruption cleared particles and
  applied no buff. Both sessions logged out and removed only the test buff.
  Persistent character/item content, currency, book and gems were unchanged.
  Normal server regeneration, ZoneEntry height adjustment and regenerated item
  instance GUIDs are recorded separately in the protocol notes.

Remaining fidelity limits include the full attachment/mode enum, nonzero
finish-delay semantics, late-join nimbus permanence, regular sphere-ring
stepping, exact projectile paths, transformed actor particle points, additive
mesh shaders, and ordering against other translucent surfaces. First-person
hand locations are estimated until a first-person arm model is available.
Sound is retained as metadata for milestone 6.

See [spell rendering](SPELL_EFFECT_RENDERING.md),
[asset findings](SPELL_EFFECT_FORMAT.md), and
[protocol/live verification](SPELL_EFFECT_PROTOCOL.md).
