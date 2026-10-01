# Native XMI loop controls

Offline investigation, 2026-10-01. This extends the controller findings in
[XMI_NATIVE_SELECTION.md](XMI_NATIVE_SELECTION.md) and resolves the saved-cursor
question for the installed Miles build. **There is no production loop
implementation in this change.** `XmiScheduler::preflight` must continue rejecting
controllers 116/117 until a bounded interpreter and its tests are implemented.

The important result is unusual: the saved loop cursor points to the **CC116
status byte itself**. CC117 resumes there without advancing past it. Consequently
the native timer executes CC116 again and can allocate another of its four loop
slots. A conventional interpreter that jumps to the first event after CC116
would disagree with this binary, including on the installed original sequences.

## Evidence and scope

| Input | Bytes | SHA-256 |
| --- | ---: | --- |
| `mss32.dll` | 349,696 | `fa78565a1e07df215532c611a5089256fcc1d81c1ae808eca66614c3ab77f9e0` |
| `thurgadinb.xmi` | 13,040 | `3a12e65204803464686b306047098a6a2ee1ac866acc275108655e97689bf908` |
| `thurgadina.xmi` | 46,430 | `232919f4049b9b041ae32a79a00243a5b91f9080d8910fdeef50646b1356b578` |
| `templeveeshan.xmi` | 47,434 | `34b32e5eece4cfe7e8b8a4df9153ecab8a88618f80a5b793038fb05be41fa548` |

All addresses below are absolute virtual addresses in the DLL, whose image base
is `0x21100000`. Subtract that base for an RVA. Inputs came from the local
`/Users/daeken/EverQuest` installation and were read without modification.

Static disassembly was checked with bounded, isolated x86 instruction execution
using Unicorn 2.1.4. The probe mapped the DLL's original PE sections, executed its
pure reset routine at `0x21118090`, and drove the **unmodified** timer callback
`0x21116850`, event walker, dispatcher, loop handlers and variable-length reader.
One synthetic sequence/driver used unlocked channels, identity channel mapping,
null callbacks, speed 100, volume 127 and a 120 Hz preference. The synth/output
boundary at `0x21117480` was intercepted with a `ret 8`; its requested MIDI messages
were recorded instead of sent anywhere. The stop export at `0x21106da0` was
intercepted with `ret 4`, so this probe does not test EOT cleanup itself. No DLL
entry point, Windows service, synthesizer or audio device was initialized. No
music was rendered or played, and no original file was patched.

This is executed evidence for those native routines under the stated state,
not an end-to-end EQ listening test or a claim about other Miles versions. The
finite original trace was repeated using the DLL's own reset rather than only
hand-initialized state; the entire captured result matched exactly.

Temporary reproduction artifacts on the investigation host:

| Artifact | SHA-256 |
| --- | --- |
| `/tmp/openeq-xmi-loop-witness.py` | `007b4962f3a75eba7b786faf9319c6266ba8e72a53a19f8f5de7e596175db208` |
| `/tmp/openeq-xmi-loop-witness.json` | `3e3bdd41558e0576d1a8becc891e612ac580ab42adf6618b1954b8c48e36ed3e` |
| `/tmp/openeq-xmi-loop-additional.json` | `121237dc7fa6e82d86c90869a7eae3fd242a139beab56dfbbfca0798bfdecf99` |
| `/tmp/openeq-xmi-thurgadinb-full-native.json` | `48ea1cc39939b86f797820ad663e457c6f4451299bf40311355348befbdaba83` |

The probe runs with `/tmp/openeq-native-animation-venv/bin/python` and uses the
small PE reader `/tmp/openeq-native-research.py`. These are temporary research
artifacts, not required runtime dependencies or committed original assets. The
address chains and synthetic fixtures below retain the essential evidence if
those temporary files are removed.

## State layout and handlers

`sequence+0x18` is the current EVNT cursor. Loop state consists of four saved
cursors at `+0x78 + 4*i` and four signed counters at `+0x88 + 4*i`. Counter `-1`
means free; zero is an occupied infinite loop. Reset sets all four counters to
`-1` at `0x211180cc`–`0x211180e0`.

The dispatcher is `0x21117050`. Its controller switch subtracts 7 and reads byte
table `0x21117408` then address table `0x211173d8`. Controller 116 selects table
entry 7, address `0x2111721a`; 117 selects entry 8, address `0x21117253`.

| Address | Effect |
| --- | --- |
| `0x2111721a`–`0x2111722e` | CC116 scans slots 0 through 3 for the first counter equal to `-1` |
| `0x21117230`–`0x21117233` | If no slot is free, it returns without changing any loop state or cursor |
| `0x21117239`–`0x21117248` | Stores the effective controller value as the counter and the current `sequence+0x18` as the saved cursor |
| `0x21117253`–`0x2111726b` | CC117 scans slots 3 through 0 for the highest counter not equal to `-1` |
| `0x2111726d`–`0x21117284` | Value below 64 clears that counter to `-1` without a jump |
| `0x21117287`–`0x2111728a` | For value at least 64, an empty stack returns without a jump |
| `0x21117290`–`0x211172a7` | An occupied counter of zero restores the saved cursor without decrementing |
| `0x211172aa`–`0x211172b4` | A nonzero counter is decremented; reaching zero clears the slot and falls through |
| `0x211172b6`–`0x211172c2` | Otherwise restores the saved cursor |

Loop state belongs to the sequence, **not the MIDI channel**. A CC117 on another
channel still uses the highest occupied sequence slot. Both handlers consume the
control; neither forwards it to ordinary MIDI output.

The usual dispatcher prefix override precedes these handlers
(`0x2111707a`–`0x21117092`): a pending per-channel prefix value can replace the
controller value when dispatching a source event. The probes use no prefix
callback/override, matching the currently observed EQ path. Do not claim that
the authored data byte always wins in an arbitrary callback-enabled host.

Malformed control behavior has a native bounds bug. With all slots free,
CC117 below 64 reaches its store with index `-1`, writes `0xffffffff` at
`sequence+0x84` (the fourth saved cursor), and continues. The isolated witness
observed this write. A safe implementation should reject this malformed state;
reproducing the out-of-range write is not a compatibility requirement. CC117
at least 64 with all slots free is a no-op. A fifth CC116 when all four slots
are occupied is also a no-op, as verified with five consecutive start markers.

## Why restart executes CC116 again

| Address | Cursor continuation |
| --- | --- |
| `0x21116990`–`0x211169a0` | Reads the status byte at `sequence+0x18` |
| `0x211169c6`–`0x211169c9` | Saves that same pre-dispatch cursor in global `0x21153078` |
| `0x211169e4`–`0x211169f6` | Reads status/data directly from the current cursor and dispatches it; no pre-increment |
| `0x21116a17`–`0x21116a20` | After a non-note command, compares current cursor with the saved pre-dispatch cursor |
| `0x21116a26`–`0x21116a32` | If unchanged, advances by message size; CC116/117 are three bytes |
| `0x21116cb3`–`0x21116cb9` | If changed, skips that advancement and returns directly to `0x21116990` |

Thus CC116 initially records its own status byte and then advances normally.
When CC117 jumps, the next decoded event is that exact CC116. Its preceding
delay bytes are **not** replayed, because the saved cursor excludes them. Delay
bytes after CC116 are replayed. At slot capacity, the replayed CC116 is a no-op
and its three bytes are still skipped by normal cursor advancement.

## Repeat counts and nesting witnesses

The synthetic fixture below has no initial delay, one note of duration 1, a
two-tick loop body, and EOT immediately after CC117:

```text
B0 74 NN  90 3C 5A 01  02  B0 75 7F  FF 2F 00
CC116=N   note 60       d2  CC117=127  EOT
```

For initially free slots, every positive count 1 through 127 was separately
executed and checked. Count 1 produces one body pass. Counts 2 through 127
produce **N+3 body passes**, because the first three restarts allocate additional
slots before the fourth slot can count down to zero. This formula is specific
to this single-loop shape with four initially free slots, not arbitrary nesting.

| Count | Note-on ticks | EOT tick | Counters at EOT |
| ---: | --- | ---: | --- |
| 0 | 0, 2, 4, 6, 8, 10, 12, … | Not reached in bounded run | Four zeros after the third restart |
| 1 | 0 | 2 | `[-1,-1,-1,-1]` |
| 2 | 0, 2, 4, 6, 8 | 10 | `[1,1,1,-1]` |
| 3 | 0, 2, 4, 6, 8, 10 | 12 | `[2,2,2,-1]` |
| 127 | 0, 2, …, 258 | 260 | `[126,126,126,-1]` |

For count 2, counters after each start/restart become `[2,-1,-1,-1]`,
`[1,2,-1,-1]`, `[1,1,2,-1]`, `[1,1,1,2]`, then `[1,1,1,1]`. The last NEXT
clears only slot 3 and falls through. Earlier occupied slots remain; the native
handler does not pop all duplicate cursors or continue closing outer slots.
CC117=64 produces the same repeat trace as CC117=127. CC117=63 clears the
occupied slot and falls through after one pass even when CC116=0.

A nested fixture demonstrates why lexical FOR/NEXT pairing is insufficient:

```text
B0 74 02  90 3C 5A 01       # outer start, note 60
B1 74 03  90 3D 5A 01  02   # inner start, note 61, delay 2
B5 75 7F  90 3E 5A 01  03   # inner NEXT on another channel, note 62, delay 3
B7 75 7F  FF 2F 00          # outer NEXT on another channel, EOT
```

Note 60 plays once at tick 0. Note 61 plays at ticks 0, 2, 4, 6, 8, 13, 15,
17; note 62 plays at 10 and 19. EOT is tick 22, with counters `[2,2,-1,-1]`.
After the first inner NEXT falls through at tick 10, the apparent outer NEXT
at tick 13 selects a remaining **inner** saved cursor at EVNT+7. This is the
actual highest-occupied-slot behavior, not conventional nested-loop semantics.

## Time and notes continue across jumps

The event walker expires active notes before processing source commands on each
effective tick (`0x211168d6`–`0x21116969`). It decrements the pending event delay
at `0x2111696b`, processes commands when the delay is nonpositive, and consumes
a delay byte at `0x21116cd3`–`0x21116ce4` before yielding that effective tick.
The loop handlers change only loop state/cursor. They do not reset the timer,
beat counters, pending note durations, sustain or other controller state.

There is no implicit one-tick pause at NEXT: after its jump, CC116 and the first
body commands execute on **the same effective tick**. A three-tick delay before
CC116 shifts the simple count-2 note ticks to 3,5,7,9,11 and EOT to 13; it is
not repeated. Putting those three ticks after CC116 instead yields note ticks
3,8,13,18,23 and EOT 25.

Changing the simple fixture's note duration to 5, then inserting delay 8 before
EOT, produced note-ons at 0,2,4,6,8 and releases at 5,7,9,11,13. EOT was tick
18. Notes survive the loop boundary and retain independent native slots; they
are neither flushed nor restarted when the cursor jumps. Existing scheduler
rules about expiry ordering and same-key overlap therefore remain relevant.

An infinite loop with no delay (`B0 74 00 B0 75 7F FF 2F 00`) never returned
from its first timer invocation before the probe's 200,000-instruction cap.
All observed commands stayed on tick 0. Native execution does not supply a safe
per-render work bound for this case. A future interpreter must have its own
bounded work and zero-time-jump limits; an output batch limit alone is
insufficient because these commands emit no MIDI output.

## Original sequence witnesses

The four unsupported original sequences reduce to two byte-identical EVNT
pairs. Offsets below are relative to the EVNT payload and point to status bytes;
event indices and ordinals are zero-based. Ticks in this table are the source's
single linear traversal before executing loop control.

| Files / ordinals | CC116 event, offset, tick, bytes | CC117 event, offset, tick | Linear EOT tick |
| --- | --- | --- | ---: |
| `thurgadinb.xmi` / 0; `thurgadina.xmi` / 5 | 232, 947, 2094, `B1 74 7F` | 1383, 6450, 15185 | 15193 |
| `templeveeshan.xmi` / 0; `thurgadina.xmi` / 0 | 941, 4763, 6428, `B0 74 00` | 2022, 9875, 16971 | 16971 |

Both NEXT commands use value 127. The first pair's 6,458-byte EVNT payload
hashes to `9a18e12075d844b79fda8b6851e5ba4b3286848d28bcc48ee9d9fa60ecd2f780`.
The second pair's 9,882-byte payload hashes to
`d9157e1588fadf0b9214c6bc9e2fe9be843d03d4644c6f84bbcb37f30098057e`.
Each includes a trailing padding byte after EOT. Matching EVNT does not claim
that the whole containers or their other metadata are identical.

The full `thurgadinb.xmi` ordinal-0 native trace completed without hitting the
instruction cap or active-note error path:

- Initial CC116: tick 2,094, cursor 947, counter 127 in slot 0.
- First NEXT: tick 15,185, cursor 6450. It restores cursor 947, decrements slot
  0 to 126, and immediately reexecutes CC116 into slot 1 with count 127.
- Subsequent NEXT ticks: 28,276; 41,367; 54,458; 67,549; …, spaced by 13,091.
  At 54,458 the four slots are full and replaying CC116 changes no state.
- Last NEXT: tick 1,703,924, clears slot 3 from 1 to `-1` and falls through.
- EOT: **tick 1,703,932**, after the original eight-tick tail. There are **130
  body passes**, 260 loop-control dispatches, 147,645 source channel-message
  dispatches and 261,313 requested ordinary MIDI outputs. Counters are
  `[126,126,126,-1]`. This is about 236.66 minutes at the default 120 Hz; it
  was processed offline without waiting that wall-clock duration.

At the first restart, native output first releases channel-1 keys 37 and 44
at tick 15,185, then sends the repeated body notes `93 3C 3F` and `96 50 22`
on that same tick. The next source note, `98 24 63`, follows at tick 15,186.
One old active note remains across the jump. The final note in the finite
trace releases during ordinary expiry at tick 1,703,932 before EOT, so stopping
at the intercepted EOT cleanup does not omit any active-note release there.

The `templeveeshan.xmi` ordinal-0 bounded witness reached NEXT at ticks 16,971;
27,514; 38,057; 48,600; 59,143; and 69,686, a 10,543-tick body period. Restarts
reexecuted CC116 at cursor 4763 immediately and saturated all four counters
with zero. EOT was not reached. The zero-counter branch has no decrement or
exit condition; the bounded trace verifies repeated execution, while the
handler establishes the indefinite behavior absent an external stop/control.

## Remaining implementation boundary

The current parser already retains status offsets, additive delays and source
indices. A later interpreter can represent the native cursor and four slots
without expanding loops into a huge event vector. It must retain a separate
monotonic execution clock rather than treating source absolute ticks as the
repeated timeline. The loop start's preceding delay cannot simply be replayed
with that source event.

Before admitting these controls, verify the above finite/infinite/nested traces,
cross-boundary note releases, capacity behavior, same-tick continuation and
cancellation under small/large pull batches. Preflight's existing linear
active-note occupancy analysis is not sufficient for arbitrary looped input:
repeats can overlap earlier notes. Execution must keep the 32-note bound and
bounded source work even on control-only loops. Malformed unmatched breaks
need a safe diagnostic. RBRN/controller109 execution, callback-enabled prefix
overrides, arbitrary host tempo changes, pause/resume and external branches
into/out of loop bodies are outside this witness and remain separate gates.

Nothing here enables the four sequences, changes automatic music selection,
asserts original instrument timbre, or adds background playback.

## Independent review

A second reviewer reproduced all finite counts 1–127, the listed synthetic edge
cases, both original EVNT pairs, and the complete finite original trace. Removing
the probe's redundant writes to native-reset fields and restoring the original
output function with its native output-disable field (`driver + 0x1c4 = 1`)
produced the identical full trace: EOT tick 1,703,932, 260 loop-control records,
147,645 source messages and 261,313 outputs. Thus neither those duplicate reset
writes nor the substituted output-return instruction affected the recorded loop
result. Output records are function arguments; shorter MIDI messages can leave
an unused third argument, so they are not universally literal wire byte triples.
The stop/cleanup hook remains outside the proven scope. No playback occurred.
