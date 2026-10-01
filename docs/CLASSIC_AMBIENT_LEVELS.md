# Classic ambient base levels

October 1, 2026. Supported classic kind-0 ambience now uses the original
emitter's base gain. Zero previously became unity in OpenEQ; the native default
is **0.2**. Negative values also select that default, rather than attenuation
by their absolute value. Music and EMT level interpretation are unchanged.

## Original execution

The installed `eqgame.exe` has SHA-256
`bab4ee0bd724b80c85de7df7020e7049a2bedaa1eca65b1abc59294c472cd593`.
Addresses are preferred virtual addresses at base `0x00400000`.
The local Unicorn probe executes the complete `CreateOldEmitter` at
`0x004db3d0`, including its native `0x005fefc0` constructor and base initializer
`0x005fdd80`. Only memory allocation is supplied. Original CRT exponentiation
executes; no substitute mathematical callback supplies the gain.

At `0x004db65a`, the source level is negated as a wrapping signed 32-bit word.
The branch at `0x004db679` tests that result's sign. Ordinary nonpositive source
levels select a kind-dependent default: kind 0 loads float word `3e4ccccd`
(0.2) at `0x004db692`; kinds 2/3 select unity. Positive source levels and the
`i32::MIN` wrapping edge enter the original decibel helper `0x005fcda0`:

| Kind-0 source level | Base gain |
| --- | --- |
| `i32::MIN` | 0 (wrapping negation retains a negative value below the cutoff) |
| `i32::MIN + 1` through 0 | 0.2 |
| 1 through 10,000 | `10^(-level / 2000)` |
| above 10,000 | 0 |

The helper compares the negated level against -10,000 before exponentiation;
exactly 10,000 therefore retains approximately 0.00001, while 10,001 is silent.
The implementation uses f64 exponentiation followed by f32 storage. It matches
the recovered operation and selected stored native results to one ULP; this
is not a promise of universal cross-platform CRT bit equivalence.

The probe additionally starts at the real EFF record-dispatch instruction
`0x004dbaa2`, supplying one already-read 84-byte record in the native stack
slot. It executes both original construction callsites, all-day coalescing,
period writes, enable setter and sound-manager registration, stopping at
`0x004dbbce` before the next file-read step. Day and night use source offsets
60 and 64 respectively. Five asymmetric/boundary pairs verify both branches.
File I/O, effect-bank resolution and the whole zone loader are outside this
slice; a known asset pointer is supplied in the bank.

For every resulting emitter, the complete active-volume setter `0x005fe580`
executes with a controlled master gain of 0.5. It forwards the stored emitter
`+8` gain multiplied by master to the instance's volume virtual method. Only
that output endpoint is intercepted. This also distinguishes stored base gain
from the final output level. No audio device opens and no audio is played.

## Coverage and limits

- 36 native construction cases: kinds 0/2/3 × 12 signed/range boundaries.
  Only the already-supported kind 0 gains new production behavior.
- Five native record pairs and nine subsequent active-volume updates.
- Rust regressions cover native boundary values, parser-to-scheduler day/night
  propagation, silent extremes, and original GFay row 12 (sound IDs 162/164):
  both authored zero levels yield scheduled gain 0.2 at the emitter position.
- A raw inventory of all 134 installed EFF files, applying finite positive
  radius and native pair-coalescing rules, finds 3,610 default-level kind-0
  sides and 396 positive attenuated sides. These are source observations, not
  counts of audible runtime voices: missing sounds, EMT precedence, unsupported
  assets and activation/scheduling still apply.

This correction establishes base levels, not native distance curves, EAL
selection, environment flags, final master settings, priority/fades or acoustic
parity. Existing scheduling/device policies remain documented in
`AUDIO_RUNTIME.md`. Tests remain metadata-only, offline or digitally silent.

## Frozen local reproduction

Run the native probe with `PYTHONPATH=/tmp/openeq-re-tools`; the corpus probe
uses ordinary Python. Original binaries/assets and derived outputs are not
committed. The helper dependency is the established frozen prefix of
`/tmp/openeq-ter-light-binding.py`, checked by hash within the native probe.

| Artifact | SHA-256 |
| --- | --- |
| `/tmp/openeq-classic-gain-native.py` | `6f6b89c74965356c2575e8cd8038b6fa3878d42653b90b5045d5418de25e9772` |
| `/tmp/openeq-classic-gain-native.json` | `683c37ded9f9395264fa41db5b27fda388e60812e340986b3189ea84acebf80c` |
| `/tmp/openeq-classic-gain-reader.json` | `82975bbdfd11f6e0a1bdae6bd36e0aecb0dea3185cdcbdb470695137c8d691c1` |
| `/tmp/openeq-classic-gain-corpus.py` | `769072e1570ba18be2a4c789c9c40c965ce4d0dfba0143a280e68361a1c2bfd8` |
| `/tmp/openeq-classic-gain-corpus.json` | `bf92669d65ba2e1763d163b8ae98832b229056e15c906273aac83c8f5a7c0081` |

## Independent verification and implementation checks

A second probe executes the complete legacy reader `0x004db8f0` through return,
with controlled file/filename/allocation interfaces. Tagged fields establish
source argument mapping independently. All **10,000 positive levels** match
native stored gain bit-for-bit against double-precision exponentiation followed
by f32 rounding on this host; boundary cases and four master values also pass.
No EAL lookup or base-gain rewrite occurs in this reader/factory/setter path.
Downstream asset/device behavior remains outside that result.

Independent probe `/tmp/openeq-classic-gain-review.py` has SHA-256
`d2ecec8a984b3d1570c200d0a37f138f8ce6c72bfc9f0e3b67ef66dd055befd9`;
its result JSON/log hash is
`76f310aa9013142097d52697a02f948a2a664ed6a4be8afa50a569a51fc8b02c`.

The integrated workspace passes **1,189 tests, zero failures/ignored across
114 suites**, including originals, GPU and offline/digitally silent audio.
A Clippy-requested rewrite from overlapping match arms to equivalent conditions
was followed by the focused scheduler suite. Strict workspace lint, client/audit
builds, all-target no-default and formatting checks pass. Logs use
`/tmp/openeq-classic-gain-{workspace,tests-final,clippy,build,no-default}.log`.
