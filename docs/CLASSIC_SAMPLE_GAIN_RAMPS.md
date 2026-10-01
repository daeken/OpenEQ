# Classic sample gain ramp and release endpoints

October 1, 2026. This research connects the previously observed
[continuous-emitter release request](CLASSIC_AMBIENT_CLOCK_LIFECYCLE.md)
to a concrete original 2D sample implementation. No OpenEQ audio scheduler or
mixer changes are made. All device endpoints are intercepted; nothing plays.

## Concrete native boundary

Original constructor `0x005fd2d0` installs vtable `0x00ad2934`. Its gain setter
at virtual +0x14 is `0x005fd350`; virtual +0x0c update is `0x005fd440`.
The witness executes these complete original methods and release helper
`0x005fdf60`, with an explicit constructed sample attached to a kind-0 emitter.
The native release calls the concrete setter, decrements the retained reference
from two to one, and clears the emitter pointer.

The sample implementation is chosen explicitly. This does not execute asset
loading/type selection, sample pool admission, playback start, decoder, device
or the complete manager update loop. A controlled driver/allocated handle and
sample status stand in for Miles. The original PE import table identifies the
intercepted endpoints:

| Address | Original import |
| --- | --- |
| `0x00ab15a0` | `AIL_allocate_sample_handle` |
| `0x00ab1584` | `AIL_set_sample_volume_levels` |
| `0x00ab15a4` | `AIL_sample_status` |
| `0x00ab15b0` | `AIL_end_sample` |
| `0x00ab15a8` | `AIL_init_sample` |
| `0x00ab120c` | `GetTickCount` |

The constructor, method dispatch and gain arithmetic execute; only those clock
and device interfaces return controlled values. The source reuses and asserts
the original executable and constructor-harness hashes from the earlier probe.

## Observed finite controls

Ten cases cover ordinary positive/zero/negative duration, unsigned wrap,
signed half-range crossing, repeated and retargeted requests, three nonplaying
statuses and a missing handle. The five release histories each have eight
subsequent updates. Native positive-duration gain samples match a separate
exact-rational model followed by the final f32 store.

- A request stores target gain at +0x20, captures current gain +0x24 into start
  gain +0x28, anchors tick +0x2c and stores duration +0x30. Nonpositive duration
  also sets current gain to target immediately. The setter submits current gain
  equally to the two channel-volume endpoints.
- A nonzero-duration request for an already equal target is ignored. It does
  not restart the ramp. A changed target captures the last **updated** current
  gain; it does not independently advance the previous ramp to the new tick.
- With controlled status4, a 100ms fade from1 to0 gives0.5 at50ms. The update at
  100ms submits zero volume. The next update, already at equal zero target and
  current gain, requests `AIL_end_sample` followed by `AIL_init_sample`.
- Status0/2/8 returns false without advancing the ramp or issuing device calls.
  A missing sample handle leaves gain state untouched and returns false.
- The terminal check compares signed absolute tick with signed wrapped
  `anchor + duration`. The tested `0xfffffff0` unsigned-wrap fade behaves like
  the ordinary history. Starting at `0x7ffffff0` with100ms crosses the signed
  boundary and immediately reaches target on its first update. This is a
  demonstrated native edge, not a recommended modern timing policy.

The harness deliberately keeps status4 even after end/init requests. Repeated
requests in later captured updates therefore do not demonstrate real Miles
status transitions or repeated stopping in a live client. No actual handle is
released or external reference removed in these concrete-sample histories.
The earlier destructor-branch witness remains a separate controlled endpoint.

## Runtime implications and limits

For this concrete class, emitter +0x1c is consumed as the gain-ramp duration by
the native release endpoint. Reaching zero and issuing the device end request
are separate updates; counting admission delay from sound completion would
still be wrong. A future OpenEQ model needs separate emitter, instance/ramp
and resource lifetimes rather than one shared cooldown.

This establishes finite controlled sample gain behavior, not acoustic parity,
Miles volume units, output amplitude, 3D/stream behavior, live status transitions,
voice-cap policy or original asset-to-class admission. Original device ownership
and manager cadence remain necessary evidence for a production scheduler change.

## Frozen local reproduction

```sh
PYTHONPATH=/tmp/openeq-re-tools python3 /tmp/openeq-classic-sample-fade.py /tmp/your-sample-fade-result.json
```

| Local artifact | SHA256 |
| --- | --- |
| `/tmp/openeq-classic-sample-fade.py` | `396cbbbf2914acbd0f5099a41d196a38a2880cbd5f14dee2d46dff7c84b3979b` |
| `/tmp/openeq-classic-sample-fade.json` | `4b4999c77ce7516765eb0459b72fd5aee4064916a2feb9de5f2a70cf937ba6ba` |
| `/tmp/openeq-classic-sample-fade.log` | `d512080b1c1a7ad27a586b4d1784b6582bc64fc77b9d5e69b1ea95dac08db34d` |

All probes and original binary-derived outputs remain local and read-only.
No original assets or binary bytes are committed.

Independent review reproduced the native output twice, then checked all40
release updates against a separate rational state-and-event model, plus complete
repeat/retarget sequences, inactive statuses and the final missing-handle events.
It also checked original setter/update disassembly; no findings. The review
script `/tmp/openeq-classic-sample-fade-independent-review.py` has SHA256
`8db31ae03cc998d68e6857005e3d1666055c2df47e8b3f488d97f7862e6fb434`;
its JSON is `d2e24ddfe5b48575123a11fdd762360751dffe78d3fac3f8071845f66dd19b1b`.
