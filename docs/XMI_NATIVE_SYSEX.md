# Native XMI system-exclusive transport

Offline investigation, October 1, 2026. This extends `XMI_NATIVE_LOOPS.md`
using the same installed Miles DLL, source hash, PE mapping, reset routine and
bounded instruction execution. It establishes byte transport and timing, not
Roland device emulation or original instrument timbre.

## Active native path

The timer recognizes statuses F0 and F7 at `0x211169a6..0x211169b7`.
`0x21116c5e` reads the authored VLQ using `0x2111abd0`, calculates the source
record length, and calls sender `0x21116f70` at `0x21116c95`. That sender strips
the length bytes and preserves the source status followed by its payload.

`0x21116f9c` computes payload length plus one, bounded by preference 10. Native
startup writes `0x600` (1536 bytes) at `0x2110139b` to preference address
`0x21151670`. The probe explicitly establishes that startup value; a zero-filled
preference array is not representative. The sender stores output-header length
at `0x21117019`. Unless output is disabled or the driver is the sentinel -2,
`0x2111703f` calls import `0x2113e1a8`, verified as `WINMM!midiOutLongMsg`.
The optional driver+0x198 trap receives an encoded CC105 event; its callback
is null in these witnesses. No Roland-specific semantic interpretation occurs
at this boundary.

The native copy loop also reads one byte beyond the logical output: the status
is stored separately, but the copy count includes it. The recorded output length
excludes that extra byte. OpenEQ must preserve logical packet bytes, never this
overread. Oversized packet truncation is not enabled in the new support scope.

Synthetic F0 and F7 witnesses both retain their authored status. In particular,
F7 is not rewritten to F0. The installed synthesis API instead requires complete
F0..F7 messages, so continuation/escape F7 source records remain unsupported.

## Original The Deep witness

`thedeep.xmi` SHA-256:
`cfbc214a417f027f33f254ab2ad6494870a7925bae8d73bbce059e29dd05bf34`.
Ordinal zero has 22 complete F0 packets, maximum 26 output bytes, at ticks 0–10.
Each native output exactly equals source status plus payload, excluding its VLQ.

| Effective tick | EVNT status offsets | Output lengths |
| ---: | --- | --- |
| 0 | 0, 12, 39 | 11, 26, 11 |
| 1 | 52, 64 | 11, 11 |
| 2 | 77, 91 | 13, 11 |
| 3 | 104, 116 | 11, 14 |
| 4 | 132, 147 | 14, 14 |
| 5 | 163, 178 | 14, 14 |
| 6 | 194, 209 | 14, 14 |
| 7 | 225, 240 | 14, 14 |
| 8 | 256, 271 | 14, 14 |
| 9 | 287, 302 | 14, 14 |
| 10 | 318 | 14 |

Native EOT occurs at tick 23,925, with 1,349 ordinary source channel messages
and 2,443 requested ordinary MIDI outputs. The original reset and unmodified
walker/sender run in isolation; output-disable driver+0x1c4 is one and EOT stop
is intercepted as in the loop witness. No WINMM call or audio device runs.
This does not verify the eventual physical device's SysEx interpretation.

## OpenEQ boundary and validation

The macOS worker's `MusicDeviceSysEx` API accepts complete F0..F7 packets,
according to the installed SDK's `MusicDevice.h`. `MidiSynth::sysex` validates
length 2–1536, framing and seven-bit interior bytes, then forwards unchanged
bytes synchronously. Invalid input returns before calling or poisoning the
unit. A native failure poisons it consistently with channel MIDI/render errors.
The unit is owned by its worker, and no hardware-output component is created.

Five synth tests pass, including silent original packet transport: all 22
The Deep packets are submitted in authored tick order with intervening PCM
rendered only to memory. A subsequent synthetic note produces finite nonzero
PCM; a rejected malformed packet leaves the unit usable. This proves API
acceptance and continued rendering, not Roland GS effects or original timbre.
The subsequent scheduler/stream integration admits these complete packets,
keeps their source payloads immutable, and dispatches an explicit SysEx event
through fixed bounded worker storage before the next rendered samples. Native
ordinary/packet/combined digests match at batch sizes 1/7/512. Synthetic cases
cover release/source ordering, loop reentry and cancellation; an injected SysEx
failure discards partial PCM and cleans active notes/pedals. The entire original
The Deep sequence plus two-second release tail renders finite nonzero PCM only
to memory. No physical output or original-timbre claim follows from that test.

Temporary evidence:

| Artifact | SHA-256 |
| --- | --- |
| `/tmp/openeq-xmi-sysex-witness.py` | `5565c8c7512edc16d044b290edb6bc67b73bed44367614429545ff4a448eb026` |
| `/tmp/openeq-xmi-sysex-witness.json` | `6fa7fdad6e7431dc72d9fc9624ea0ae24af619da89e7c3584bb3a4ea9946e6c6` |

The script imports the existing loop witness and runs in the same temporary
Unicorn environment. Test log: `/tmp/openeq-sysex-synth-tests.log`. Original
assets and native binaries remain outside the repository.

## Independent complete-trace digests

A second offline run retains the complete native ordinary-output trace as well
as all SysEx output records for `thedeep.xmi` ordinal zero. It subclasses the
same witness only to record chronological output order at the existing ordinary
output and SysEx-header-ready hooks. It changes no additional native code or
sequence state. The original reset, walker and SysEx sender execute under the
same bounded, output-disabled conditions. Every SysEx record, EOT tick and
output/source count matches the earlier witness. Separately decoding the source
VLQs confirms every native logical packet is status plus authored payload.

The following canonical byte streams use little-endian unsigned 64-bit integers.
Ordinary records contain effective tick followed by three bytes:
status/data1/data2. The unused native data2 argument for C0/D0 is normalized to
zero, as in the loop witnesses. SysEx records contain effective tick, EVNT-relative
status offset, logical output length, then that many exact bytes including F0
and F7. Combined records are emitted in native output order with a one-byte tag:
zero followed by an ordinary record, or one followed by a SysEx record.
FNV-1a uses offset basis `cbf29ce484222325` and multiplier `100000001b3` with
64-bit wrapping.

| Trace | Records | Canonical bytes | FNV-1a 64 |
| --- | ---: | ---: | --- |
| Ordinary MIDI | 2,443 | 26,873 | `7c7b681d2ef68c11` |
| SysEx | 22 | 829 | `ff375c25ef67e49e` |
| Tagged combined | 2,465 | 30,167 | `698546ba61a0c3e2` |

Canonical byte-stream SHA-256 values, respectively:

- Ordinary: `ac8640bf0f033a6d1570b7b64ce24e061f5bf19b8c1abd6f344faf6323929cbc`.
- SysEx: `ece664e7be71560b145c118222be5d6d230de8de6281ba127a98d6f6d0b834df`.
- Combined: `671cec8ab7e9ae0a4dbb5283733100aa3cf3eff3e8814c60ce6c65c404266a5c`.

EOT is tick 23,925 and there are 1,349 authored channel-message visits. All 22
SysEx outputs precede the first ordinary output, which is CC0 at tick 92. Thus
this original establishes complete bytes and timing, but synthetic fixtures
are still required to verify same-tick ordering between the two output kinds.
These digests cover native requested transport, not device interpretation,
instrument effects or synthesized PCM.

| Temporary artifact | SHA-256 |
| --- | --- |
| `/tmp/openeq-xmi-sysex-digests.py` | `a20177c8c90edc8329d08e36d44646c7e281b03624fa4001fc4b11470304e866` |
| `/tmp/openeq-xmi-sysex-digests.json` | `2c7b373931f34ea2d17ac62fc6a13b921494e67122f1b71db3808969934276c6` |

The script imports the earlier SysEx witness and its loop witness dependency;
the latter's SHA-256 is
`007b4962f3a75eba7b786faf9319c6266ba8e72a53a19f8f5de7e596175db208`.
The result retains raw ordinary arguments, SysEx records, source visits and
cross-type output order so the canonical streams can be independently rebuilt.
