# Native XMI selection and emitter evidence

Static inspection of the locally installed client, 2026-09-30. Neither the
Windows executable nor its Miles DLL was executed. No original music was
rendered or played. Addresses below are absolute virtual addresses; subtract
the listed image base for an RVA. This extends and corrects the unresolved
selector/controller discussion in [XMI_PLAN.md](XMI_PLAN.md).

| Binary | Image base | Bytes | SHA-256 |
| --- | --- | ---: | --- |
| `eqgame.exe` | `0x400000` | 11,678,208 | `bab4ee0bd724b80c85de7df7020e7049a2bedaa1eca65b1abc59294c472cd593` |
| `mss32.dll` | `0x21100000` | 349,696 | `fa78565a1e07df215532c611a5089256fcc1d81c1ae808eca66614c3ab77f9e0` |

Inspection used LLVM `llvm-objdump -d --x86-asm-syntax=intel` and PE import,
export, string and jump-table inspection. Function names below describe the
identified routines; the installed executable is not a source-symbol build.

## Legacy EFF records have two sound kinds

`LoadOldEmitters` reads 84-byte records at `0x4dba88`–`0x4dba9c`.
The first kind is **byte 56**, and the second kind is **byte 57**; the latter
is not padding. The first/day selector is signed i32 at byte 48, and the
second/night selector is signed i32 at byte 52.

| Record offsets | Native use |
| --- | --- |
| 56 / 57 | Separate first and second sound kinds |
| 48 / 52 | Separate signed sound selectors |
| 60 / 64 | Separate level/loop values |
| 32 / 36 | Separate cooldown values |

At `0x4dbab4`–`0x4dbacc`, the loader collapses the pair into one always-active
emitter only if **all four pairs** above compare equal. At `0x4dbace`–`0x4dbad2`,
first kind 2 overrides that comparison: it is always collapsed and the second
side is ignored, regardless of its kind or selector. Do not erase those raw
second-side bytes from diagnostics, but do not create a second runtime emitter.

The first `CreateOldEmitter` call is at `0x4dbb0e`; its active-period field
at emitter `+0x50` is assigned 0 for collapsed/always-active or 1 for first/day
at `0x4dbb19`–`0x4dbb24`. A collapsed pair skips the second call. Otherwise
byte 57 supplies the kind at `0x4dbb77` and the second call is `0x4dbb97`;
its period is assigned 2 at `0x4dbba6`.

The `CreateOldEmitter` switch begins at `0x4db3d0`, table `0x4db790`: kind 1
selects the music path at `0x4db45c`; kinds 0, 2 and 3 select the effect path
at `0x4db606`. Unknown kind bytes should remain identifiable as unknown.

An audit of 134 local EFF files found 8,732 **raw records**. Kind-pair counts:

| First / second kind | Raw records |
| --- | ---: |
| 0 / 0 | 3,252 |
| 1 / 0 | 242 |
| 1 / 1 | 104 |
| 2 / 0 | 4,053 |
| 2 / 1 | 2 |
| 2 / 2 | 932 |
| 3 / 0 | 7 |
| 3 / 3 | 137 |
| 16 / 0 | 1 |
| 68 / 65 | 2 |

These counts include unknown/junk rows and are not counts of valid or audible
emitters. In particular, the large number of kind 2/0 pairs does not imply two
active channels. The 242 kind 1/0 pairs explain many incorrectly inferred
nighttime XMI references.

## Music selectors are direct zero-based ordinals, including zero

At `0x4db460`–`0x4db462`, the kind-1 path compares the signed selector with
zero and sends **every nonnegative value** to the zone-XMI branch. Negative
selectors go through the old MP3 table after negation. Zero is not a silence
sentinel for music.

| Native location | Forwarding evidence |
| --- | --- |
| `0x4db50f`–`0x4db512` | Requires the zone MIDI asset at manager `+0x28`; absent asset creates no emitter |
| `0x4db54c`, `0x4db55d`–`0x4db560` | Passes the original selector unchanged as the third `MusicManager::Set` argument |
| `0x5ff440`–`0x5ff459` | `MusicManager::Set` stores that argument in music-entry `+0x18` |
| `0x5ff94c`–`0x5ff955` | Fade-in copies entry `+0x18` to sound-control `+0x0c` |
| `0x5fd9e1`–`0x5fd9ed` | `MidiInstance::Play` passes control `+0x0c` unchanged to `_AIL_init_sequence@12`, IAT `0xab1608` |
| `0x21106c7f`–`0x21106c82` | Miles export wrapper forwards the same ordinal |
| `0x2111815c`–`0x2111816d` | Initializer forwards it to sequence finder `0x2111acb0` |
| `0x2111ad68`–`0x2111ad6d` | Finder tests the pre-decrement ordinal; zero selects the first `FORM XMID` |

Container exhaustion returns zero at `0x2111ad98`–`0x2111ad9d`. Initialization
then fails at `0x21118174`–`0x2111818d`, with the error string “No valid XMIDI
sequences present in file.” EQ releases the failed handle at
`0x5fd9f3`–`0x5fda35`. This path has no ordinal remapping or global alias fallback.

`SetCurrentZone` formats `%s.xmi` from the zone name at `0x4dbd1a`–`0x4dbd34`
and stores the resulting asset at manager `+0x28`. Global `gl.xmi` is loaded
separately at `0x4db05d`–`0x4db06e` into `+0x10`, for explicit global music IDs.
Its existence is not a fallback rule for a missing zone XMI.

This establishes the **requested asset name**, not complete loose/archive
precedence. `GetAsset` checks a cached name, then `sounds\\`, `voice\\default\\`,
and the archive manager; preloading and every overlay source have not been
traced. Local root-file counts below are corpus observations, not proof that
all possible native asset sources are absent.

## EMT selection and legacy precedence

The EMT parser at `0x4d9f30` stores zero-based field 17 unchanged in
`EqEmitterData+0x64` at `0x4da03f`–`0x4da044`. Field 1 provides the explicitly
named asset. `EmitterAdd` resolves that name, recognizes `.mp3`/`.xmi` at
`0x4dace9`–`0x4dad0b`, and passes `+0x64` unchanged to `MusicManager::Set`
at `0x4dad8c`–`0x4dad9b`. EMT XMI selectors therefore use the same zero-based
ordinal convention, including zero.

The two installed EMT XMI entries found in `neighborhood.emt` request
`qeynos.xmi` ordinal 5 and `gl.xmi` ordinal 21; both are in range.

At `0x4dbd52`–`0x4dbd67`, `SetCurrentZone` loads EMT first and calls the legacy
loader only if EMT loading returns false. The loader `0x4db7a0` returns true
whenever `fopen` succeeds (`0x4db878`–`0x4db88b`), even if there are no usable
rows. The native fallback condition is **open failure**, not “no valid parsed
EMT entries.” The installed `nektulos.emt` therefore supersedes its legacy
EFF file in ordinary selection.

## Fixtures that distinguish the mappings

Rows below are zero-based raw 84-byte EFF record indices. “Effect” means a
separate effect-bank lookup, not the similarly numbered XMI ordinal.

| Zone / row | Kinds | Selectors | Interpretation |
| --- | --- | --- | --- |
| GFay / 0 | 1,1 | 2,0 | XMI day 2; night 0, both within six sequences |
| GFay / 2 | 1,0 | 0,3 | XMI day 0; night effect 3 |
| GFay / 3 | 1,0 | 5,5 | XMI day 5; night effect 5 |
| Butcher / 0 | 1,1 | 1,1 | XMI 1 within two sequences |
| Butcher / 65 | 1,0 | 0,0 | XMI day 0; night effect 0 |
| Nektulos / 0 | 1,1 | 1,1 | Ordinal 1 exceeds the one-sequence root XMI; EMT supersedes EFF |
| Nektulos / 1 | 1,1 | 0,0 | Ordinal 0 valid; EMT supersedes EFF |
| Blackburrow / 1 | 1,1 | 1,0 | Day 1; night 0, both within twelve sequences |
| Blackburrow / 2 | 1,1 | 11,11 | Ordinal 11 valid |
| Blackburrow / 19 | 1,0 | 10,10 | XMI day 10; night effect 10 |
| Blackburrow / 20 | 1,0 | 12,12 | Day ordinal 12 out of range; night effect 12 |
| Bothunder / 59 | 1,1 | -1,163 | Day MP3 table entry; night zone-XMI ordinal 163, no named root XMI found |

Butcher also has first-side music selectors 2–7 in kind 1/0 records. Those
requests really exceed its two-sequence root file; subtracting one or inventing
an alias does not resolve them. Equal selectors alone do not establish all-day
collapse: kinds, levels and cooldowns must also match.

## Clock convention and exact active window

The native world pointer is `0x10171e0`; hour is its byte at `+4`. It is a
**1–24 stored hour**, not the normalized display hour:

- The time packet handler copies its first byte unchanged to `WorldData+4`
  at `0x4fdadf`–`0x4fdaff`.
- `AdvanceTime`, `0x86971b`–`0x869744`, adds elapsed hours to the last stored
  hour, subtracts one, reduces modulo 24, then adds one back.
- `CurrentGameTime`, `0x869819`–`0x86981e`, subtracts one from the stored hour
  before supplying it to the time-formatting structure.
- `0x4ac214`–`0x4ac240` sends “day” when stored hour is greater than 4 and
  less than 19: **stored 5–18 inclusive, normalized 4–17 inclusive**.

`EmitterSetNight` (`0x4da430`) sets manager `+0x1000` to 1 for false/day or 2
for true/night. `UpdateEmitterStates`, `0x4d9885`–`0x4d9894`, lets period 0
through unconditionally and otherwise requires a match to that state.

At the time of inspection, OpenEQ's `ZoneEvent::Time` parser and `LiveState`
copied the raw packet byte without normalization. A runtime must document
which convention it accepts; using the native numeric comparison on a newly
normalized hour would shift the boundary by an hour. EQEmu's pinned
`common/eqtime.cpp` also computes packet hours in 1–24.

## Extended-controller findings and remaining limits

Miles channel dispatch is `0x21117050`. Its extended-controller switch indexes
`controller - 7`, capped at controller 119, using byte table `0x21117408` and
address table `0x211173d8`.

| Controller | Handler | Established behavior |
| ---: | --- | --- |
| 108 | `0x21117106` | Invokes optional sequence prefix callback; returned value is stored per channel. Without callback follows common output |
| 109 | `0x211172c5` | Calls `AIL_branch_index` at `0x211172cb` |
| 116 | `0x2111721a` | Finds a free one of four loop slots, stores count and EVNT cursor |
| 117 | `0x21117253` | Uses highest occupied loop slot; value below 64 clears it; value at least 64 decrements/restarts, or repeats indefinitely when count is zero |
| 120 | Outside switch | Common MIDI path, optional event trap then output at `0x21117399`–`0x211173ca`; **not a native branch controller here** |

`AIL_branch_index`'s implementation at `0x21118d20` reads RBRN's u16 marker
and u32 payload offset records (six bytes each). On a match it sets the event
cursor to `EVNT_chunk + 8 + offset` at `0x21118d5b`–`0x21118d69` and clears
the pending delay. Preference 7 defaults to zero; in that case it also clears
the four loop counters. Controller 120's correlation with marker IDs in the
corpus does **not** establish branch semantics and must not override this code
evidence.

The exact loop cursor re-entry semantics remain unverified. Preserve loop
metadata and reject unsupported playback rather than claim exact parity.
The EQ import table contains no sequence callback-registration or sequence-tempo
API. Its two direct `AIL_set_preference` call sites in `0x5fce80` set preference
42, not the MIDI clock preference 3; no direct clock override was found. This
does not rule out dynamically resolved APIs.

## Duration expiry, ordering and overlapping notes

The MIDI timer callback begins at `0x21116850`. Each effective sequence tick
first processes note durations (`0x211168d6`–`0x21116969`), then the source
event delay and events (`0x2111696b` onward). The note table has 32 slots:
channel at sequence `+0x65c+4*i`, key at `+0x6dc+4*i`, remaining duration at
`+0x75c+4*i`, and active count at `+0x658`. Channel -1 marks a free slot.

The expiry pass walks slots in ascending order. It decrements each occupied
slot's duration at `0x211168f9`, then emits `0x80 | channel`, key, velocity zero
when the counter is at most zero (`0x2111690b`–`0x2111692b`). It marks the slot
free and decrements the active count. Thus existing releases precede source
events at the same effective tick, with simultaneous releases in slot order.

For a source note-on, the walker sends the event first at
`0x211169e4`–`0x211169f6`, then finds the first free slot at
`0x21116a39`–`0x21116a74`. It stores the channel, key and decoded duration
unchanged at `0x21116a83`–`0x21116abe`. There is **no same-key lookup or
replacement**. Distinct overlapping note-ons on the same channel/key retain
distinct duration entries; each expiry emits its own ordinary MIDI note-off.
The channel dispatcher at `0x21117132`–`0x21117146` and
`0x21117369`–`0x211173ca` does not suppress the old release based on newer
same-key notes. The receiving synthesizer's interpretation of overlapping MIDI
notes remains a separate fidelity question.

A stored duration of zero cannot be visited by the already-completed expiry
pass until the next effective tick. Its first decrement produces -1 and emits
the release: **zero-duration notes have a one-tick minimum duration**, unless
sequence cleanup stops them earlier. This corrects the earlier plan's proposed
immediate zero-duration release. Exhausting all 32 slots reports an error and
stops the sequence at `0x21116e67`–`0x21116e83`; a bounded scheduler must not
silently steal/reassign active duration slots.

For a non-repeating EOT, `0x21116bd3`–`0x21116c0c` calls `AIL_stop_sequence`.
The export forwards to `0x21118420` at `0x21106e2c`. That routine sets stopped
state and calls `0x211184e0`, which walks all 32 slots in ascending order,
emits ordinary velocity-zero note-offs, clears each occupied slot, and sets the
active count to zero (`0x211184f3`–`0x2111851f`). It subsequently visits the
16 channels and emits CC64=0 when remembered sustain was at least 64
(`0x21118465`–`0x2111847c`). Other branches clean up the native lock/protect
controllers. No unconditional CC120/all-sound-off was found in this cleanup.
Notes extending beyond EOT are therefore released at EOT rather than making
the sequence unplayable; a cancellation safety reset is a separate operation.
