# Original spell particle files

OpenEQ reads the installed client's `spellsnew.eff`, `spellsnew.edd`, and loose
particle textures. This document records the layout and the evidence behind the
decoded fields. No original client assets are included in the repository.

The reference installation has 679 effect records and 2,587 emitter records.
All nonzero emitter references are valid table indices. Names are terminated by
the first NUL; bytes after it are frequently stale data, not zero padding.

## Effect selection and stages

`spellsnew.eff` has no header. Each 268-byte record contains:

| Offset | Type | Meaning |
| --- | --- | --- |
| 0 | char[64] | Display name |
| 64 | stage[68] | Casting stage |
| 132 | stage[68] | Projectile travel stage |
| 200 | stage[68] | Target impact stage |

A stage is one little-endian sound ID followed by four 16-byte emitter slots.
Each slot contains four little-endian words: EDD table index, unresolved word,
mode, attachment ID. Emitter index zero disables that slot. Other indices are
used directly, without subtracting one. The unresolved word is zero in the
reference installation. Attachment IDs 4 and 5 are consistently paired on
casting hands. The full attachment/mode enums and left/right ordering have not
been proved from native code and remain numeric in the asset API.

The stage order agrees with the older 2,792-byte `spells.eff` layout and with
modern emitter names: cast emitters appear in stage zero, and effects such as
Swarm have their traveling emitter in stage one. The modern parser does not
silently interpret the older file as this layout.

The `spellanim`/`SPELL_EFFECT_INDEX` column in `spells_us.txt` selects this EFF
table. It is field 145 in the fixed layout and field 85 in the installed compact
layout. Example selections are Frost Bolt -> 179 (`FrostCoat`), Minor Shielding
-> 220 (`MajorShielding`), Gate -> 218 (`RingOfKarana`), and Minor Healing -> 278
(`Ethereal_Remedy`). Shared visual names need not equal the spell name.
Persistent-particle and nimbus fields are separate; the parser does not invent a
fourth stage or infer packet timing from them. Native handling of
`OP_SpellEffect.finish_delay_ms` is not established by this asset investigation.

## Emitter definitions

`spellsnew.edd` begins with the eight bytes `EDD\0` and `110\0`. Everything
following the header is a 416-byte emitter record. Record zero is the unused
sentinel. All numbers are little-endian. `f32` values below are finite-checked;
the complete 80 words at offsets 96 through 415 are also retained unchanged.

| Offset | Type | Meaning |
| --- | --- | --- |
| 0 | char[64] | Display name |
| 64 | char[32] | Texture basename |
| 96 | u32 | When 1, take orientation from an attached bone when available |
| 100 | u32 | When 1, disable depth writes |
| 104 | u32 | When 1, use additive destination blending |
| 108 | u32 | Actor scaling of particle dimensions |
| 112 | u32 | Billboard/oriented-quad mode; 0, 1, 2 observed |
| 116 | u32 | When 1, existing particles follow the attachment basis |
| 120 | f32 | Emitter duration, seconds; negative forces its absolute duration |
| 124 | f32 | Particle lifetime, seconds; nonpositive uses emitter duration |
| 128 | u32 | Initial particle count |
| 132 | u32 | Particles spawned per emission tick |
| 136 | f32 | Emission ticks per second |
| 140 | f32 | Emission delay, seconds |
| 144, 148 | f32 | Opacity fade-in / fade-out seconds |
| 152, 156 | f32 | Size grow / shrink seconds |
| 160 | f32 | Distance used to reduce particle counts |
| 164 | f32 | Maximum opacity |
| 168 | u32 | Shape enum, described below |
| 172, 176, 180 | f32 | Shape dimensions |
| 184, 188, 192 | f32 | Local basis position offsets |
| 196, 200 | f32 | Two local orientation angles |
| 204, 388 | f32 | Random full quad width endpoints |
| 208 | f32 | View-space depth offset |
| 212, 216, 220 | u32 | Initial R, G, B, 0..255 |
| 224, 228, 232 | u32 | Final R, G, B, interpolated over particle age |
| 236, 240, 244 | f32 | Axial velocity endpoints and acceleration |
| 248, 252, 256 | f32 | First transverse velocity endpoints and acceleration |
| 260, 264, 268 | f32 | Second transverse velocity endpoints and acceleration |
| 272, 276, 280 | f32 | Radial velocity endpoints and acceleration |
| 284, 288, 292 | f32 | Orbital angular velocity endpoints and acceleration |
| 296 | f32 | World downward acceleration; positive decreases world Z |
| 300 | f32 | World X acceleration |
| 304 | u32 | Flipbook frame count |
| 308 | f32 | Flipbook frames per second |
| 312, 392 | f32 | Random spin velocity endpoints |
| 364, 368 | f32 | Additional scale factors, both 1 in reference installation |
| 372 | u32 | Random initial rotation flag |
| 380, 384 | f32 | Random full quad height endpoints |
| 396 | u32 | When 1, share the same random sample for width and height |
| 400, 404 | f32 | Additional independent dimension shrink ramps |
| 408 | u32 | Allows negative radial coordinate in rendering |
| 412 | u32 | Actor scaling of emitter basis/offsets |

The name `emission_rate` in the Rust API means ticks per second, not particles
per second. The sustained rate is `particles_per_emission * emission_rate`.
At offset 120, negative values such as -5, -5000, and the legacy conversion
sentinel -9999 are durations, **not gravity**. A positive value can be replaced
by the caller's requested duration. This distinction matters for preventing
effects from accelerating wildly or continuously spawning the wrong count.

Width and height endpoints can be reversed; interpolate them in file order.
Color endpoints are sRGB-like authored channel values. The API supplies
normalized channels and alpha, leaving conversion to the renderer's linear
color space explicit. Alpha is `min(fade_in_factor * fade_out_factor, opacity)`;
it is not opacity multiplied by both ramps. Negative ramp durations disable
that ramp. The simulation owns lifetime and particle count limits.

All native orientation, orbit, and spin angles use 512 units per revolution.
Native sine/cosine methods convert the angle to an integer and mask with 511
before lookup. The default basis is actor third matrix axis, first matrix axis,
and negated second matrix axis. With an identity native actor matrix this is
`(+Z, +X, -Y)`: **the axial direction is up**, and the transverse plane is
horizontal. When flag 96 is set and a bone attachment exists, the same axis
ordering is taken from its matrix. Offsets and velocities are in this basis;
gravity and the world X term are applied separately.

## Shape enum

The native switch at `0x10073b54` provides the following geometry. Shape
dimensions are radius/half-extent values unless explicitly stated otherwise.
Dimensions zero in the second transverse direction usually fall back to the
first radius. Existing native scaling rules still apply.

| ID | Geometry | Distribution |
| --- | --- | --- |
| 0 | Point | At emitter origin |
| 1 | Ring / ellipse | Evenly spaced around perimeter |
| 2 | Sphere / ellipsoid | Ordered latitude rings; counts select ring layout |
| 3 | Cylinder side | Random azimuth, fixed radius, axial `(u - 0.5) * dimension[2]` |
| 4 | Disk / ellipse interior | Rejection-sampled transverse unit disk |
| 5 | Sphere / ellipsoid surface | Uniform axial sample [-1,1], transverse radius `sqrt(1-z*z)` |
| 6 | Box surface | Select one of six faces; remaining two coordinates random [-1,1] |
| 7 | Cone side | Same random [0,1] multiplier for radius and axial dimension |
| 8 | Torus surface | Major radius dimension[0], minor radius dimension[2], two angles |
| 9 | Ring / ellipse | Random azimuth around perimeter |

For sphere shape 5, dimension[0] is the first transverse radius,
dimension[1] is the second transverse radius (zero means use dimension[0]),
and dimension[2] is the axial radius (zero means use dimension[0]). Shape 6
also substitutes dimension[0] for zero dimensions. The table distinguishes
surface emitters from volumes: sampling a box for every shape is visibly wrong
for the common rings and spherical bursts. The exact ordered point placement
for shape 2 and the client's pseudo-random sequence are not reproduced by this
asset module.

## Textures and flipbooks

The graphics DLL searches `SpellEffects/`, `EnvEmitterEffects/`, then
`ActorEffects/`. The loader follows that order with case-insensitive basenames.
For example, `mist_white_full.dds`, used by several frost emitters, belongs to
`EnvEmitterEffects/`. Basenames containing path separators or traversal are
rejected. Missing or damaged files are reported separately; no magenta texture
or synthesized sprite is substituted.

The cache decodes each distinct referenced filename once into straight RGBA8
with top-left rows and shares the complete texture vector with `Arc`. TGA
requires explicit format selection because it has no dependable magic for
automatic image detection. Decoding retains authored alpha. Texture reads,
dimensions, decoded pixels, and aggregate cache size have explicit limits.

| Frame count | Native grid |
| --- | --- |
| 1 | Full texture |
| 2..4 | 2 x 2 |
| 5..8 | 4 x 2 |
| 9..16 | 4 x 4 |
| Other values | Full texture |

Cells advance across columns then rows. Frame selection uses particle age and
authored frames per second. The helper wraps negative animation speeds safely;
those are present in the reference data, although the original unsigned-wrap
behavior for negative time indices is not reproduced. Grid dimensions must not
be guessed from the square root of the frame count.

## Reverse engineering evidence

Addresses below refer to the installed 32-bit `EQGraphicsDX9.dll`, image base
`0x10000000`. They describe behavior rather than copied implementation code.

| Address | Evidence |
| --- | --- |
| `0x10071840` | Writes EDD header/version 110 and 0x1a0-byte records |
| `0x10071f90..0x100724c7` | Converts legacy particle records, seconds/rate units, RGB channels |
| `0x1006d2a0..0x1006d483` | Initializes emitter duration and particle capacity |
| `0x10073410..0x10073599` | Delay, initial count, emission ticks, distance scaling |
| `0x100727e9..0x100728a0` | Flag 100 toggles D3DRS_ZWRITEENABLE; flag 104 sets DESTBLEND to ONE and restores INVSRCALPHA |
| `0x10073264..0x100732b8` | Actor matrix basis selection and signs |
| `0x10073780..0x10073864` | Random rotation, spin, width/height endpoint sampling |
| `0x10073864..0x10073b54` | Local offsets and orientation |
| `0x10073b54..0x10074642` | Shape switch |
| `0x10074720..0x1007480e` | Velocity ranges and particle lifetime |
| `0x10074cbb..0x10074e1a` | Acceleration, radial/orbital movement, gravity and wind |
| `0x100753f5..0x10075602` | Particle age, scale ramps, full-size to half-size quad conversion |
| `0x10075c99..0x10075dd9` | Opacity ramps/cap and age-interpolated RGB |
| `0x10075e0e..0x10075f64` | Flipbook grids and frame selection |
| `0x100b9d70`, `0x100b9d90` | 512-entry angular lookup |

The earlier classic layout is also independently documented by
[CoastalRedwood/Zeal](https://github.com/CoastalRedwood/Zeal/blob/83ba90556b4a03292a014471a908cad6439df41d/Zeal/game_structures.h#L1484).
That reference informed stage comparison; this implementation was written from
the binary format observations above.

Portable tests cover malformed/truncated/versioned inputs, reference bounds,
nonfinite known floats, unsafe filenames, unknown-word preservation, color and
opacity behavior, rectangular flipbooks, and TGA alpha. An ignored integration
test reads a locally installed client when `EQ_DIR` is set. No proprietary
binary fixtures are checked in.

## Projectile actor models and attached particles

Normal spell bolts can select an item actor independently of the EFF travel
stage. `Resources/OnDemandResources.txt` maps the actor to its archive and mesh:

```text
missle.eqg^IT11504.MOD^IT11504_ACTORDEF^EQGM
```

`CharacterLibrary::load_equipment_scene` resolves those shared-archive entries
as well as classic equipment and individual item EQGs. It preserves authored
coordinates and item origin, resolves original textures through the library
cache, preserves animated texture frames, and rejects weighted/animated item
geometry. For IT11504 the mesh is a small 80-triangle sphere with `color.dds`
and the native `AddAlpha_MPLBasicA.fx` shader. The material currently records
transparency and emission; exact additive mesh blending remains a renderer
concern. The visible fire trail comes from separate authored particle bindings.

The same archive contains `it11504.prt` and `it11504.pts`:

| File | Header | Record |
| --- | --- | --- |
| PRT | `PTCL`, u32 count, u32 version=5 | 108 bytes: actor EDD index, char[64] point name, 10 raw words |
| PTS | `EQPT`, u32 count, u32 version=1 | 164 bytes: char[64] point name, char[64] attachment name, position[3], rotation[3], scale[3] f32 |

Headers, versions and record strides are confirmed by native loaders at
`0x10062b90..0x10062da5` and `0x10062de0..0x1006308e`. Earlier PTCL versions
have shorter records; they are explicitly unsupported by this parser. As with
EDD names, PTS names can have stale memory bytes after the first NUL. Unresolved
PRT words remain intact; one duplicate emitter index is overwritten by the
native loader, so its persisted value is not authoritative.

These indices refer to **actoremittersnew.edd**, which has the same EDD 110
format and 1,106 records in the reference client. They do not refer to the
spell EDD table. All 17 missile actors in this archive use one identity
`ATTACH_TO_ORIGIN` point and map IT11503..IT11519 to actor emitter 471..487:

| Models | Actor emitters | Authored effect |
| --- | --- | --- |
| IT11503..IT11508 | 471..476 | Fire: yellow, red, blue, orange, green, purple |
| IT11509..IT11514 | 477..482 | Electricity in the same color order |
| IT11515 | 483 | Brown mud |
| IT11516 | 484 | White snow |
| IT11517 | 485 | Blue water |
| IT11518 | 486 | Black darkness |
| IT11519 | 487 | Green poison |

`load_projectile_effects` exposes the complete actor EDD table and these
model-to-emitter bindings. Unknown, non-origin, or transformed attachment
points are reported in `skipped`; they are not silently approximated. File
size, PFS inflate sizes, record counts, reference indices and finite transforms
are checked. Spell and actor EDD textures together resolve 443 distinct files
in this client.

Server `ProjectileAnimation` resolves an item ID to its `IDFile` model name;
the default item ID 8005 is not the model code IT8005. The installed original
arrow is IT10: `AROHEAD.BMP` vertices lie at negative Z, while `FLETCH.BMP`
vertices lie near zero, confirming the arrow's authored forward axis is -Z.
The classic sword IT1 points along +X. Projectile orientation therefore cannot
assume that every original model uses the same forward axis.
