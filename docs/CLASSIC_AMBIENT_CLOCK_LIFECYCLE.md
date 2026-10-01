# Classic ambient clock lifecycle follow-up

October 1, 2026. Additional original-client execution narrows the remaining
scheduler work after [cooldown construction](CLASSIC_AMBIENT_COOLDOWNS.md).
This is research only: no production scheduling, audio output or character
state changes. The first witness covers positive-cooldown kind-0 emitters,
whose one-shot instances are not retained by the emitter. A separate bounded
continuous-instance witness follows below.

## Executed boundary

The complete original constructor `0x004db3d0` and ordinary emitter update
`0x00600320` execute, including their native timer setter `0x005fdfa0`.
The original executable hash and shared emulation setup are asserted against
the frozen constructor witness. Clock, random return, listener coordinates,
enabled byte and play result are controlled inputs. The play boundary checks
loop count 1 and records timer state before returning a synthetic instance or
null. There is no decoder, Miles implementation, device, real elapsed time or
native voice-resource simulation in this probe.

The enabled byte at emitter `+0x17c` is a controlled field. These cases do not
execute the host's day/night controller or prove its actual enable/disable
ordering. Likewise, changing the listener directly is a range-admission test,
not an original movement or zone-lifecycle test.

## Results

All **29 scenarios / 165 full updates / 55 play attempts** match an independent
integer timer model:

- Failed one-shot play still consumes the admission and resets the timer before
  the play function returns null. A same-tick retry is not issued; the next
  attempt must satisfy the new delay. This is distinct from OpenEQ's current
  failed-file suppression policy.
- Initial updates start the timer while disabled or outside range. Subsequent
  disabled/outside updates leave an existing positive anchor and selected delay
  alone. An eligible reentry after expiry plays immediately and resets the
  timer. This requires no completion callback from earlier one-shots.
- Ordinary 32-bit clock wrap works by wrapping subtraction followed by a signed
  comparison. Equality remains ineligible. A difference at or above the signed
  half-range becomes negative and does not pass a positive delay; it is not an
  unsigned elapsed-duration comparison.
- Anchor zero is also the uninitialized sentinel. A first update at tick zero
  leaves that sentinel in place, so the next update initializes again. Repeated
  disabled updates at tick zero can repeat random-delay selection. A successful
  admission exactly at wrapped tick zero likewise causes initialization on the
  following update. This is observed native behavior, not a recommendation to
  copy the sentinel into a new monotonic-clock design.
- No tested delayed emitter retains the returned instance. Null and nonnull
  results produce the same admission-clock behavior. This does not establish
  whether an audio device admits overlapping voices or how they are stopped.

The deterministic clock matrix covers anchors `1`, `17`, `0x7ffffff0`,
`0x80000000`, `0xfffffff0` and positive delays `1`, `1000`, `32768`,
`86400000`, with equality and adjacent ticks. Focused controls cover disabled
and range transitions, legal random outputs, null playback and both zero-anchor
paths. Overflowed constructor bounds and continuous loop-zero ownership remain
outside this follow-up.

## Implementation boundary

The constructor fix remains unchanged. A future classic one-shot timer model
can use this evidence for initialization, strict deadline checks and attempted-
play anchoring, but still needs an explicit policy for active voice caps,
overlap, failed resources, random-stream ownership and clock representation.
Actual continuous voice stop/fade and resource lifetime remain separate. EMT and music
must not acquire these kind-0 rules incidentally. These tests do not authorize
a blanket replacement of the shared audio scheduler.

## Local reproduction

Run `PYTHONPATH=/tmp/openeq-re-tools python3
/tmp/openeq-classic-clock-lifecycle.py /tmp/your-new-result.json`. The explicit
output argument preserves the frozen result. Original binaries and derived
artifacts remain local, and no original assets are committed.

The source script checks the prior native constructor probe SHA-256
`02ee386d62405440d2abf7c4afd0bd5ebfd07f1ff78f51b092bae721cea39f1d`.
Result JSON SHA-256 is
`804155fd8966013e429ac7c499f91cf5ff8c1a4f396ae0ef0c97303ec6d6f14c`.
The follow-up script SHA-256 is
`69d4c275f81d7dd8ae5f182bbd5e224daab07a76f5d63221548bd250bcefa329`;
its compact log is
`16bb2664cfdd92de744e2bf72db610dec9de73e9036d9ed4d4c32b19c8a87a75`.

## Continuous-instance retention and release requests

A second original-update witness covers **four scenarios / 15 updates / eight
play attempts / four instance endpoint calls** for base zero or negative,
including positive random fields that the constructor ignores. It executes
the same original routines and native release helper `0x005fdf60`; only the
returned instance's virtual endpoints are controlled.

- Successful loop-zero play retains the returned pointer and increments its
  reference count. Repeated eligible updates reset the emitter timer but do
  not replay while that pointer remains retained.
- Leaving range or disabling the controlled emitter causes the native helper
  to call instance virtual slot `+0x14` with float zero and the emitter's
  `+0x1c` word. It then decrements the reference count and clears its pointer.
  Further ineligible updates make no repeated request. Eligible reentry starts
  a new loop-zero instance.
- If the synthetic play result is null, each eligible update can retry,
  including another update with the exact same tick. Failed continuous play
  therefore differs from the positive-cooldown retry clock above.
- Removing the synthetic external reference before exit makes the native
  decrement reach zero, and the helper invokes virtual slot zero with argument
  one before clearing the pointer. This exercises the original destructor-call
  branch; the endpoint itself does not free or manipulate any real resource.

The `+0x14` call is recorded as a zero-value request, **not claimed as an
executed fade or stop**. Its actual implementation, fade duration semantics,
device effects and ownership of any external references were not executed.
The external-reference removal is an explicit harness action, not an observed
native completion callback. The existing playback policies remain unchanged.

Replay with `PYTHONPATH=/tmp/openeq-re-tools python3
/tmp/openeq-classic-loop-lifecycle.py /tmp/your-new-loop-result.json`.
Frozen script SHA-256:
`cd7ed9b70c61d5b9d34285f16d8f6e10f91961c478a25f6afa5ffd3b89b8dcfb`.
Frozen result:
`80a33969943984c6bbdad5f3e91c59720f4813b86679d351e7009e2a524be855`.
Frozen compact log:
`f1dca452f2ed9eca405735c8737d778d8f5a530dfe814dad31ca21b4827713cb`.

Independent agent replays reproduce both frozen result hashes. Source/model
review and original update/release disassembly agree with these bounded claims.

A later [concrete 2D sample witness](CLASSIC_SAMPLE_GAIN_RAMPS.md) now executes
the original gain setter and instance update reached by this release call.
It establishes finite gain-ramp timing and end/init endpoint requests with
controlled sample status. Actual Miles execution, asset-class admission and
manager/device ownership remain outside that follow-up too.
