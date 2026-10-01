# Authored WLD particle texture metadata

`ObjectParticleAttachment.texture` retains a bounded, typed view of the
authored `0x26 -> 0x05 -> 0x04 -> 0x03` texture chain. Missing references,
cycles, unknown source variants and metadata limits are recorded on the effect;
they no longer reject an otherwise supported static object's ordinary meshes
or collision. Particle playback remains disabled.

The implementation is in
`crates/openeq-assets/src/loader/wld_particle_textures.rs`, re-exported through
`loader::wld_objects`. Native material and submission evidence is documented in
[WLD_PARTICLE_MATERIALS.md](WLD_PARTICLE_MATERIALS.md); simulation and placement
have separate checkpoints. This metadata is source evidence, not a native
resource-cache implementation or a rendering recipe.

## Retained source data

`ObjectParticleTexture` contains the source WLD filename, the optional authored
cloud texture reference, binding/link/animation nodes, frame bitmap nodes and a
typed issue list. Each expanded `ObjectParticleTextureNode<T>` retains:

- The full signed reference on the authored edge.
- The exact resolved one-based fragment identity and original fragment name.
- The typed original fields, including flags and extension bytes.

Binding metadata preserves `ParticleTexture.flags`, its child reference, raw
material handle and tail. Link metadata preserves the `0x05` animation reference,
flags and tail. Animation metadata preserves `0x04` flags, optional parameter,
frame interval, the complete ordered frame-reference vector and tail. The frame
count is that vector's length. Bitmap metadata preserves filenames in their
decoded original case/order and trailing bytes.

Positive references directly address the original fragment, including values
above 255 and later same-name copies. Authored negative references use the WLD
parser's validated string-reference resolution; their signed source values and
resolved identities are both retained. No edge is narrowed to a byte. No
same-name lookup replaces a positive reference, and no earlier definition is
substituted to imitate the original client's resource cache.

Each frame occupies its original position in `frames`; an unresolved bitmap
occupies `None`. The animation's reference vector still identifies that frame.
If an earlier chain stage cannot be expanded, later stages remain absent.
Repeated bitmap references on different frames are allowed: cycle detection is
per path, not a global deduplication set.

## Structural failures and unsupported source variants

Malformed graph edges have explicit `MissingTextureReference`,
`MissingReference`, `UnexpectedFragment` or `Cycle` issues. Unexpected fragment
issues retain the authored/resolved references and expected/actual kinds.
Unsupported layouts and materials are separate issue variants; they retain the
expanded source fields when within the metadata limits.

An empty issue list currently requires this observed source family:

| Fragment | Source conditions |
| --- | --- |
| `0x26` | flags 0, no extension bytes, raw material `0x80000017` |
| `0x05` | flags 0, no extension bytes |
| `0x04` | flags `0x18`, no optional parameter or extension bytes, one frame, interval 100 |
| `0x03` | one nonempty filename, at most three trailing zero padding bytes |

Other well-formed frame counts, intervals, flags, extension bytes, bitmap
layers or material handles remain explicit unsupported variants. For example,
a two-frame animation still retains both frame references and both resolved
bitmap nodes; it receives an unsupported frame-count/interval issue.

These checks describe source layouts, not effect behavior. They do not resolve
the negative material handle against a renderer, verify a texture file exists,
decode its pixels, authorize a sampler/blend choice or establish native loading
order. An empty issue list must not be used alone to enable playback.

## Expansion and allocation limits

Resolution follows three fixed parent stages and at most 16 bitmap edges, with
no unbounded recursive traversal. Before cloning a node, the implementation
checks these explicit limits:

| Field | Maximum retained per node |
| --- | ---: |
| Animation frame references | 16 |
| Bitmap filenames/layers | 16 |
| Fragment name | 4096 UTF-8 bytes |
| Each filename | 4096 UTF-8 bytes |
| Each fragment tail | 4096 bytes |

An over-limit node is left unexpanded. `MetadataLimit` records the original edge,
resolved fragment identity, field, observed count and limit. The code does not
copy a truncated record and present it as complete. Earlier expanded nodes still
retain their original references. A fragment identity too large for a positive
signed `Ref` is reported separately as `UnrepresentableIdentity`.

These are additional metadata limits after WLD parsing. Physically truncated
records or invalid encoded counts can still fail the WLD parser; this layer
does not turn a failed archive parse into a partially trusted object. Existing
static-pose, track-hierarchy and mesh-ancestry gates remain in force. Unknown
particle-cloud layouts still do not gain actor admission merely because their
texture prefix resembles a known chain.

## Original bitmap padding audit

An independent raw audit reads declared `0x03` body lengths, stored filename
counts and encoded filename lengths without the Rust metadata resolver. Across
293 installed object WLDs, all 623 particle-chain bitmap edges end in exactly
the zero bytes needed to align the body to four bytes:

| Suffix | Edges |
| --- | ---: |
| Three zero bytes | 562 |
| One zero byte | 57 |
| Two zero bytes | 4 |

There are no non-padding suffixes or read errors. The metadata resolver retains
these bytes and accepts zero padding of length 0..3; it does not classify all
nonempty tails as unknown extensions. Longer or nonzero suffixes are explicitly
unsupported. This bounded check is not a canonicality proof for every possible
encoded bitmap-name record.

Temporary audit artifacts, excluded from production:

| Artifact | SHA-256 |
| --- | --- |
| `/tmp/openeq-particle-bitmap-padding.py` | `b6bbf508a045588cbbef2be068966ed0d33992719b74387396d34626a4228fc1` |
| `/tmp/openeq-particle-bitmap-padding.json` | `1b2be264fc29a516418de5542eaaf6a8f697ad41eb33835154623860c7032284` |

Run with `PYTHONPATH=/tmp/openeq-re-tools python3
/tmp/openeq-particle-bitmap-padding.py` against the installed original files.

## Validation checkpoint

Dedicated portable tests cover positive references above 255, same-name
duplicates, authored negative references, full frame/reference/filename/material
retention, missing and wrong-kind references at each stage, cycles, legitimate
bitmap reuse, unknown flags/parameters/tails/materials, zero padding and metadata
limits before copying. Every malformed or unsupported texture case also checks
that the static actor retains its visible triangle and hidden collidable
triangle. Existing geometry tests still reject unknown cloud layouts, invalid
poses and meshes beneath a particle ancestor.

Original-file tests cover all eight PoK cloud records, including later duplicate
source identities, and all ten particle attachments across the eight restored
PoK actor sources. A corpus test finds 623 complete chains, including 178 authored
cloud texture references above 255, without native cache substitutions.

Run the bounded unit/regression suite with:

```text
EQ_DIR=/path/to/EverQuest cargo test -p openeq-assets --lib loader::wld_objects -- --include-ignored
```

The ignored cases require the installed original client assets. They retain no
original asset bytes in the repository and do not launch a client or GPU test.
At the 2026-10-01 checkpoint, all 36 object unit/regression cases pass with
`EQ_DIR` set, including seven portable metadata cases and both new original-file
audits. `cargo clippy -p openeq-assets --all-targets -- -D warnings` also passes.

Integrated verification: all 319 assets tests pass with original fixtures and
zero failures/ignored tests. Strict assets all-target Clippy and independent
code review pass. The same run includes separate source-lighting parser tests;
it does not claim particle playback or pixel parity. Logs:
`/tmp/openeq-lighting-source-assets-final.log` and
`/tmp/openeq-source-metadata-clippy.log`.
