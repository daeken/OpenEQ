# Native WLD particle materials and draw submission

Evidence checkpoint: 2026-10-01. All four distinct Plane of Knowledge particle
definitions resolve to **source-alpha additive blending**, including
`CSMOKE_PCD`, when the original renderer's default material aliases are used.
Smoke selects `CSMOKE.DDS`; L301, L308 and L500 select `GENG00.DDS`. These are
original texture references, not replacements inferred from effect names.

This follows [WLD_PARTICLE_RUNTIME.md](WLD_PARTICLE_RUNTIME.md), which establishes
bounded CPU simulation and projected-vertex output, and
[WLD_PARTICLE_ACTORS.md](WLD_PARTICLE_ACTORS.md), which records the attachment
and source fragment inventory. The executable witness here adds native texture
resolution, descriptor conversion, manager registration and draw submission.
It does not enable production effects or establish effect-generated lighting.

## Provenance and controlled boundaries

The source is the installed PE32 `EQGraphicsDX9.dll`, preferred image base
`0x10000000`, SHA-256
`615e7ba9e20745ec03a908cf47c038cc764358ad820d9d4d3232734fadff5383`.
The `poknowledge_obj.wld` member of `poknowledge_obj.s3d` has SHA-256
`e7fbed560a7b81bbfe7495440418de0020f98ef84f63b3f5e4850fcfc2af10e3`.
Addresses below are preferred virtual addresses; descriptive function names
are research labels unless an original diagnostic is quoted.

A temporary Unicorn 2.1.4 harness executes original code and data. This is
**not a running original client or GPU rasterizer**. Its boundaries are:

- Heap allocation and ASCII `strnicmp` have bounded substitutes. An unused
  debug formatter is intercepted; diagnostic error calls fail the witness.
- The native loader entry table contains the first 20 original WLD fragment
  bodies, including their common name-reference words. Named texture resource
  lookup is forced to miss, exercising recursive resolution rather than reuse.
- Original bitmap fragments receive controlled, already-loaded image objects.
  Image-load/getter boundaries `0x10002100` and `0x100022f0` supply those objects.
  Width, height and the source texture's reported format come from original DDS
  headers. Native file decoding, actual loaded format and shared bitmap-object
  identity are not established by this substitution.
- D3D texture/surface APIs and `D3DXLoadSurfaceFromSurface` are intercepted.
  Native code chooses the source, creates the destination and supplies the copy
  rectangle, but pixels are not copied or uploaded. State-setting calls update
  a software device-state record; `DrawIndexedPrimitive` captures that record.
- Native defaults establish ordinary render, sampler and texture-stage state.
  BLENDOP=ADD and SEPARATEALPHABLENDENABLE=false are seeded D3D defaults, not
  calls attributed to the particle pass. Controls below expose their inheritance.
- Registration runs unchanged, but the witness supplies ready vertex/index
  buffers and a controlled two-triangle count per definition. It does not run
  particle simulation, camera projection, owner transforms or the drawing gate.

The original device-initialization path reaches `0x1009b790` at `0x10099b24`,
between diagnostics "Initializing engine internals." and
"CRender::InitDevice completed successfully." The wrapper calls default alias
installation `0x1009b4f0` with argument zero. The witness executes the installer
directly; it does not execute complete device creation or exclude later alias
overrides by a host.

## Texture selection and native registration

These original early definitions have complete, positive-reference chains:

| Cloud fragment / name | `0x26` sprite | `0x05` | `0x04` sprite frames | `0x03` bitmap | Original image |
| --- | --- | ---: | --- | ---: | --- |
| 5 / `CSMOKE_PCD` | 4 / `I_CSMOKE_SPB` | 3 | 2 / `I_CSMOKE` | 1 | `CSMOKE.DDS` |
| 10 / `L301_PCD` | 9 / `I_L301_SPB` | 8 | 7 / `L301_SPRITE` | 6 | `GENG00.DDS` |
| 15 / `L308_PCD` | 14 / `I_L308_SPB` | 13 | 12 / `L308_SPRITE` | 11 | `GENG00.DDS` |
| 20 / `L500_PCD` | 19 / `I_L500_SPB` | 18 | 17 / `L500_SPRITE` | 16 | `GENG00.DDS` |

Every `0x26` has flags zero and material word `0x80000017`. Every `0x04` has
flags `0x18`, one bitmap reference and timing 100. The local archive contains:

| Image | Header dimensions | Header format | Member SHA-256 |
| --- | --- | --- | --- |
| `csmoke.dds` | 16 × 32 | DXT1 | `e90cdb641d5af44ba7d943c2587b4fe8e74f92c95fcffaa7262682a7092c8d88` |
| `geng00.dds` | 64 × 64 | DXT1 | `8dd0be192449bff88c973cf6cca0252d65112b5f89b298da164cb0ad1caed46f` |

### Source record layout and variants

An independent raw audit of all 293 installed `*_obj*.s3d` archives and their
293 WLD members finds **623 kind-`0x26` records**. Every one has exactly 12 body
bytes, flags zero, material `0x80000017` and a positive child reference; there
are no audit errors. Body offsets here exclude the common four-byte fragment
name reference. Native reader pointers include that word:

| Body offset | Native pointer offset | Width / representation | Meaning |
| --- | --- | --- | --- |
| `+0x00` | `+0x04` | u32 | Raw flags; zero in this corpus |
| `+0x04` | `+0x08` | full 32-bit fragment reference | Child texture-animation reference; `0x05` in the audited chains |
| `+0x08` | `+0x0c` | u32 | Raw material word, which may be a negative alias handle |

The on-demand reader fetches child and material at `0x1001d5c8` and
`0x1001d5f0`. The bulk kind-`0x26` reader `0x1001d7e0` fetches the same fixed
fields at `0x1001d8c1` and `0x1001d8da`. Neither path examines this record's
flags or advances these offsets conditionally. Thus 12 bytes is the observed
complete body and the fixed prefix required by these readers; this is not a
claim that all unobserved formats or flag values have no extensions.

Asset metadata should preserve the raw flags and material word, and retain the
child reference through the existing signed-reference representation. Native
`0x100c0590` accepts both positive indices and negative string-table references;
the corpus provides no negative `0x26` child witness. Do not narrow this child
reference to a byte: these reader instructions load a full dword. The earlier
low-byte finding applies to a different field, the cloud `0x34` texture read.
Nonzero flags or extra body data remain unproven variants and must not silently
inherit playback support from this audit.

Texture resolver `0x1001d520` follows the sprite reference using native
`0x100c0590`. For kind `0x26`, it first checks named resource type `0x1011`
through `0x10062300`. On the controlled miss, it resolves the `0x05` reference
and calls `0x1001d4b0`, which validates and follows the `0x04` reference into
`0x1001d100`. That routine reads flags, timing and bitmap count, resolves each
bitmap, and accesses its loaded image object through the runtime entry.

For these single-bitmap records, the resulting texture descriptor has a 1 × 1
frame grid, the original width/height, frame count 1 and timing 100. Native code
queries the source level description, creates an equally sized texture with
the reported source format, levels 0, usage OR `0x400` and pool 1, then obtains
source and destination level-zero surfaces. It requests one whole-image copy
through `0x100b9b28`, destination rectangle `(0,0,width,height)`, null palettes
and source rectangle, filter `0xffffffff`, color key zero. No framebuffer blend
operation occurs at this copy boundary.

The resolver returns the assembled texture pointer at texture descriptor `+0`,
copies `0x80000017` to `+0x24`, and sets byte `+0x28` to 1. Native registration
`0x10070ce0` inserts a cloned particle descriptor into manager list `+0x38`
through `0x1006eb10`; it binds that assembled texture at definition record
`+0x18` through `0x1006d270`. It also inserts the texture into the manager's
named texture cache and calls AddRef. Every captured draw binds the exact
texture pointer returned by its resolver call.

This witness deliberately starts with the early definitions. It does not prove
the actual client's full load order or that every later duplicate follows the
same miss path. The conditional reuse and low-byte cloud texture-reference
behavior remain as documented in the runtime follow-up.

## Material alias and descriptor conversion

`0x80000017` is an alias handle, not a positive material bitfield. The native
default installer writes **`0x80000017 -> 0x011b0507`** at
`0x1009b680..0x1009b68c`. Setter `0x1009e740` removes the high bit from the
index, bounds it below 42, and writes state-cache `+0x238c + 4*index`.
The state cache is renderer `+0xb2a8`, so this table is renderer `+0xd634`.

Converter `0x1001c410` reads texture descriptor `+0x24` at `0x1001c7fc`.
Negative handles resolve through that table at `0x1001c803..0x1001c80e`.
When resolved bit `0x01000000` is set, the converter writes
`(resolved_word >> 20) & 1` to particle descriptor `+0x68`.
For the original default alias, this is 1. The converter also writes
descriptor `+0x64 = 1` at `0x1001c834`, requests one animation frame at `+0x130`
and derives 10 frames/second at `+0x134` from timing 100. A one-frame texture
does not visibly animate merely because that rate is nonzero.

The executable controls distinguish alias semantics from a hard-coded result:

| Converter input / table state | Descriptor `+0x64`, `+0x68` | Draw DESTBLEND |
| --- | --- | --- |
| Four original records, native default alias | 1, 1 | ONE (2) |
| Direct resolved word `0x010b0507` | 1, 0 | INVSRCALPHA (6) |
| Original alias overridden to `0x010b0507` using native setter | 1, 0 | INVSRCALPHA (6) |
| Original alias restored to `0x011b0507` using native setter | 1, 1 | ONE (2) |

The override controls reconvert a fresh descriptor and then copy its two
demonstrated draw switches into the registered definition. They do not claim
that changing the alias table retroactively changes existing descriptors.

## Active submission and blend state

The main frame calls manager update `0x100762f0` at `0x1009812c`, then draw
dispatcher **`0x10072a60` at `0x1009813b`**. The dispatcher calls `0x100724e0`
for lists `+0x14`, `+0x5c`, `+0x80`, then the WLD list `+0x38`.

Draw requires a renderer device at `+0xf08` and a ready list vertex buffer at
`+0x20`. It selects preset 5 through `0x1009f970`, FVF `0x1c4`
(`XYZRHW | DIFFUSE | SPECULAR | TEX1`), stream zero with stride 32, and the
shared particle index buffer. It skips disabled records or zero triangle
counts. Registered texture `+0x18` is bound to stage zero; the separate lazy
texture-cache/file-load branch is not exercised here.

Descriptor `+0x64 = 1` disables depth writes at `0x100727f6..0x10072806`.
Descriptor `+0x68 = 1` changes DESTBLEND to ONE at
`0x10072813..0x10072823`. Native state flush `0x1009edb0` executes at
`0x10072834` before the actual D3D virtual call at `0x10072851..0x10072857`.
The call is `DrawIndexedPrimitive`, TRIANGLELIST, base vertex from record `+8`,
minimum vertex 0, vertex count `triangle_count * 2`, start index 0 and primitive
count from record `+0xc`. Each controlled two-triangle draw submits four vertices.

Captured device state at all four original draws is:

| State | Value | Established by |
| --- | --- | --- |
| ALPHABLENDENABLE | true | Particle preset 5 / `0x1009f6f0` |
| SRCBLEND | SRCALPHA (5) | Particle preset |
| DESTBLEND | ONE (2) | Converted descriptor switch |
| ZWRITEENABLE | false | Preset and descriptor switch |
| ZENABLE / ZFUNC | true / LESSEQUAL (4) | Native baseline, inherited |
| ALPHATESTENABLE / ALPHAREF | true / 1 | Particle preset |
| ALPHAFUNC | GREATEREQUAL (7) | Native baseline, inherited |
| CULLMODE | NONE (1) | Particle preset |
| FOGENABLE | false | Particle preset |
| Stage 0 color | TEXTURE × DIFFUSE | Native baseline, inherited |
| Stage 0 alpha | TEXTURE × DIFFUSE | Preset sets MODULATE; arguments inherited |
| BLENDOP / SEPARATEALPHABLENDENABLE | ADD / false | Seeded D3D defaults, inherited |

Under this captured baseline, let `Ct, At` be sampled texture color/alpha and
`Cv, Av` be vertex diffuse color/alpha. The RGB equation is
`Cout = (Ct * Cv) * (At * Av) + Cdst`, subject to depth testing and alpha test
at reference 1 (nominally 1/255). Vertex life fading therefore weights the
additive contribution; smoke's gray vertex color also scales it. This is a
blend-state derivation, not a claim of pixel-exact GPU output, color-space
conversion or hardware alpha-test precision.

After each relevant draw, native code restores DESTBLEND to INVSRCALPHA and
ZWRITEENABLE to true **in the state cache**. The list later selects preset 0.
These setters do not imply immediate D3D calls; subsequent flushes apply
pending state. The witness captures device state at draw time, not merely the
last requested cache values.

## Sampler and blend-operation inheritance

Baseline native render setup `0x10092050`, executed through `0x10092512` before
its unrelated global fog configuration, selects WRAP addressing and LINEAR
minification/magnification. Baseline mip filtering depends on renderer byte
`+0xee5`. Particle preset 5 starts at `0x1009fd68`: when that byte is nonzero,
it sets stage-zero MIPFILTER to POINT. It does not reset ADDRESSU/V or MIN/MAG.

The controlled draws prove this distinction:

| Seeded stage-zero state before the pass | Renderer `+0xee5` | State captured at draw |
| --- | ---: | --- |
| U/V CLAMP, MIN/MAG POINT, MIP LINEAR | 1 | U/V CLAMP, MIN/MAG POINT, MIP POINT |
| U/V CLAMP, MIN/MAG POINT, MIP LINEAR | 0 | All seeded values retained |

A separate control seeds BLENDOP=SUBTRACT and separate-alpha blending enabled;
the native pass retains both. Consequently the common additive equation above
depends on the surrounding blend-operation state. It would be incorrect to
claim this pass unconditionally establishes ADD or a complete sampler policy.

## Reproduction and remaining limits

The temporary harness and output are not committed or used by production:

| Artifact | SHA-256 |
| --- | --- |
| `/tmp/openeq-native-particle-materials.py` | `9a7becda9a62dc3983309ca8abf47512c468fd91d00572018d1cc350b9f8e8be` |
| `/tmp/openeq-native-particle-materials.json` | `7e426f07753c210e1be594eafa5cf79ab6eb20eb4d2afb5557953a0c41897ff6` |
| `/tmp/openeq-particle-texture26-layout.py` | `a5755ec761df1b9087ce73009759d4712ba8f0a772e80ddbe5f86724e748c556` |
| `/tmp/openeq-particle-texture26-layout.json` | `ebc2b74abaa45a57a30705305b5cadb76a20a319c8dca95d1a181eef56c2a7f0` |

Run with `PYTHONPATH=/tmp/openeq-re-tools python3
/tmp/openeq-native-particle-materials.py`. The run passes assertions for four
original texture resolutions/copy requests, native registry texture identity,
four original draw-state snapshots, three material controls, two sampler
controls and one blend-operation control: ten captured draws total. The
script also verifies entry into the original resolver, converter, registry,
dispatcher, per-list draw routine and state flush.
The separate layout audit runs with the same Python environment and
`/tmp/openeq-particle-texture26-layout.py`.

This closes the bounded material, registration and submission gap in the CPU
runtime witness. Full real-placement transform composition, the drawing gate,
actual bitmap decoding/upload, complete host state history, duplicate-resource
load contexts and full-frame original-client parity require separate evidence.
Neither additive particles nor their names establish a dynamic-light source.
