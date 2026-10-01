# Normal sky ambient: packed-byte floor across FPU modes

October 1, 2026. This bounded follow-up to
[NATIVE_HOST_AMBIENT.md](NATIVE_HOST_AMBIENT.md) checks a practical source
transfer independently of the unresolved live FPU lifecycle. In all **3,144
original-host executions**, normal-vision ordinary ambient packs each RGB
channel as `max(source_byte, 20)`, ignores source alpha, and writes alpha FF.
No production files are changed by this research.

This supports a narrow source correction: consume the sampled sky's ambient
swatch for the native sky-zone types 1, 2 and 5, using the established normal
vision floor. It does not establish native pixel equivalence or authorize
applying the same policy to other zone types, enhanced vision, missing sky,
or the host's alternate status path.

## Executed boundary

The new probe uses the frozen ambient harness's initialization and `run`
function, excluding its experiment loops and result writes. The inherited
original instructions execute:

- Vision initialization `eqgame.exe:0x005523b0`.
- Complete ordinary host ambient routine `0x004b6b20`.
- Original sky ambient getter `EQGraphicsDX9.dll:0x1002c730` and manager getter
  `0x10032b60`.
- Original ordinary ambient setter `0x1006b130` and special-A setter
  `0x1006b1e0`.

All six entries execute 3,144 times. Character/profile/effect/clock interfaces
and the final host notification retain the parent's controlled boundaries.
The supplied conditions are active ordinary host state, zone type 1, a present
sky and color table, normal vision, missing character-scalar object (default
scalar 1), and saved smoothing sentinel -1. Special A becomes black in every
case; special B retains its sentinel. This does not multiply ordinary ambient
by the character scalar.

An observer sets CW at the original ambient-routine entry. Initializer calls
retain the parent harness's CW `0x037f`. The normal minimum is the original
stored float32 constant 0.08. Each full host invocation returns with its
explicitly supplied CW unchanged. This supplies input states for the numeric
comparison; it does not infer which state a live game or graphics driver uses.
MXCSR remains the parent harness's `0x1f80` throughout this x87 comparison.

## Input corpus and independent assertion

For each of 12 combinations—x87 PC24/53/64, each with nearest, downward,
upward and toward-zero rounding—the probe supplies all 256 grayscale words
with source alpha zero, plus six asymmetric/source examples:

| Input word | Packed ordinary result |
| --- | --- |
| `00412838` | `ff412838` |
| `005c3e6b` | `ff5c3e6b` |
| `ffdbd9d9` | `ffdbd9d9` |
| `000001ff` | `ff1414ff` |
| `a0017fe0` | `ff147fe0` |
| `000d56e7` | `ff1456e7` |

The expected packed result is computed independently with integer extraction,
`max(byte,20)`, and bit assembly. It does not reuse host floating-point
intermediates or setter outputs. Grayscale spans every possible source value
for each independently treated channel; the asymmetric cases expose channel
order and per-channel floors. Original day/dawn/dusk source provenance is
already recorded in the parent note; these values are explicit inputs here,
not newly loaded DDS files.

All 12 modes pass 256 grayscale and six mixed cases. No output chooses a
production x87 control mode. The original images, intermediate memory, and
JSON remain outside the repository.

## Packed-byte stability is not float-bit stability

The engine setter converts the resulting bytes back to floats using its
stored float32 reciprocal of 255. Those **normalized float bits vary with
rounding mode**, even though all packed bytes are stable. Relative to PC64
nearest, each downward/toward-zero run differs in 124 of 262 color cases;
each upward run differs in 139 cases. Precision choice alone produces no
additional difference in this corpus.

For example, a source channel 24 yields normalized bits `3dc0c0c2` under
nearest and `3dc0c0c1` under downward/toward-zero. A floored channel 20 yields
`3da0a0a1` under nearest and `3da0a0a2` under upward rounding. The JSON retains
all three normalized RGB words for every case rather than hiding this
remaining distinction.

A modern renderer can therefore adopt the proven integer source/floor rule
while retaining an explicit Rust float normalization policy, such as
`f32(max(byte,20)) / 255`. That policy must not be described as universal native
setter bit parity. No additional sRGB decode or alpha multiplication belongs
in the proven host coefficient transfer; the modern renderer's existing
linear/sRGB framebuffer pipeline remains a separate approximation documented
in [NATIVE_COLOR_SPACE_STATE.md](NATIVE_COLOR_SPACE_STATE.md).

The current shared surface lighting still differs from native TER effects:
constant directional orientation/color, material-specific ambient/bounce/
special-ambient roles, baked-light alpha, and native final display transfer
are not solved by this ordinary ambient correction. Enhanced-vision selection,
missing-sky hourly fallback, special ambient, status shortcuts, and full host
scheduling remain separate work. A scalar that belongs to special ambient
must not be used to dim ordinary sky ambient.

## Frozen reproduction

```sh
PYTHONPATH=/tmp/openeq-re-tools python3 /tmp/openeq-host-ambient-byte-floor.py
```

| New artifact | SHA-256 |
| --- | --- |
| `/tmp/openeq-host-ambient-byte-floor.py` | `0e1d16288e844e54be26770b40a35b721f812df17457c72bcfd20f561e7329ea` |
| `/tmp/openeq-host-ambient-byte-floor.json` | `85017d331c84efe7b573289216b74089d1ca97af9e5a255e5be4fe6385fb16b8` |
| `/tmp/openeq-host-ambient-byte-floor.log` | `4a036d595fbaeb9ef8ab17508ff571b4256aa03e01b0f862008a83355611d83a` |

The probe verifies these dependencies before execution:

| Frozen dependency | SHA-256 |
| --- | --- |
| `/tmp/openeq-host-ambient.py` | `517716fdebadd2225eeea6e0b3a097bcc01b39b1f18857dd441112f209ce7669` |
| `/tmp/openeq-host-sky-light.py` | `56871adeb13c9d81fb9364c8aa9ad8312f1218917528f68eceace7ded8d144e0` |
| `/tmp/openeq-ter-light-binding.py` | `9c278977e3c7d9c8b983489f34f7aef8a3a49c51dea5a5b6ddcd816a787e1fb6` |
| `eqgame.exe` | `bab4ee0bd724b80c85de7df7020e7049a2bedaa1eca65b1abc59294c472cd593` |
| `EQGraphicsDX9.dll` | `615e7ba9e20745ec03a908cf47c038cc764358ad820d9d4d3232734fadff5383` |

The older probes and result files are not overwritten. Executing original x87
instructions in Unicorn retains the parent's emulator/CPU-parity boundary;
this is not a real client or GPU/display capture.

Root independently replayed all 3,144 cases to a separate output path; the
result JSON matches the frozen hash. Bounded production use is documented in
`SKY_AMBIENT_RENDERING.md`.
