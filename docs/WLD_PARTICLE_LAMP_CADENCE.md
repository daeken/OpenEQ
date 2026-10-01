# Stationary lamp particle invocation cadence

The original placed `POKLAMP500` emitter proves that each actual particle
manager invocation matters, including a second invocation at the same engine
timestamp. A zero-delta call can admit scheduled births and consume random
table entries. A preceding object-preview camera can also change world
particle radial motion even though the drawing contexts do not match.

This is a conditional native execution witness. It does **not** establish
that an object preview is enabled during a normal live PoK session, that
preview/main calls always see the same engine tick, or that all scenes receive
multiple particle updates. No live particle path or global random seed is
introduced.

## Chosen emitter and scope

`POKLAMP500` is simpler than the two-emitter `FTORCH301`: its skeleton 1389 has
one `L500_PCD` attachment, source fragment 20. The original placement 571 and
native node composition give emitter origin
`(925.6063232421875, 144.00927734375, -124.46380615234375)`.

The probe reuses the hash-checked setup prefix of the native placement
witness, ending before its context cross-product and all output writes. That
prefix constructs the original torch and lamp and performs their first
zero-delta updates. Only the lamp manager is invoked in this experiment.
Every scenario restores the same complete emulated memory snapshot, including
the native lamp ring, tables, and counters. Initial occupancy is one; remaining
particle life is `0.6500000357627869`. The seed-1 diagnostic table setup has
already consumed 10 signed and 21 unit values across those setup births.
This controlled initial state is not a claim about live creation order.

The executed instructions include:

- Preview block `0x10017a87..0x10017abc`, including context set/restore.
- Main update/draw block `0x10098125..0x10098140`.
- Camera conversion `0x10070ab0`, engine tick getter and manager wrapper
  `0x100762f0`, and all four complete list updates `0x10072ab0`.
- Actual native owner getters, schedule, random-table selection, motion,
  culling/context decisions, and projected vertex writes.

Only terminal draw dispatcher `0x10072a60` is intercepted to snapshot its
context and triangle counts. This experiment does not submit native GPU
draws. The existing fake vertex-buffer lock/unlock interface is supplied to
all four lists, including the three empty lists: the original updater returns
failure for a missing list vertex buffer, so leaving them null would fail
before the WLD list. Native update results and branch flow remain unmodified.
Owner suppression and emitter visibility bytes are observed, never forced.

## Shared clock and conditional caller order

The central scene caller at `0x10097e84..0x10097eab` checks two preview
objects, at renderer `+0xd7e8/+0xd7ec`. Each must be nonnull and have byte
`+0x10` nonzero before its render routine runs. That is static caller evidence,
not an observed live enablement decision. Main particle update/draw follows at
`0x1009812c/0x1009813b` when its surrounding scene gates admit the block.

The preview and main blocks use global manager `0x1017c0b0`. The wrapper
stores the engine tick into manager `+4` before updating lists
`+0x14, +0x5c, +0x80, +0x38`, in that order. A preview invocation does not have
a separate simulation clock or a list filtered to its drawing context.

Here the manager starts at tick 1000. Five controlled tick groups use
1100, 1200, 1300, 1400, and 1500. All invocations within one group receive the
same engine tick. Consequently the first invocation's four list calls get
approximately 0.1 seconds; every later invocation's four calls get exactly
zero. This proves behavior for equal ticks; the probe does not trace the
live host's clock refresh boundary.

All cameras have a 90-degree FOV, 800 by 600 viewport, near/far inputs 1/10000,
and the same orientation. The near camera is 60 units before the actual lamp
origin; the behind camera is 60 units beyond it; the far camera is 1000 units
before it. The original camera converter runs for each invocation. World
emitter captured context stays -1. Preview contexts are 0 and optionally 1;
main context is -1 after native preview restoration.

## Observed births, motion, and random consumption

The following counts are ring occupancy at the end of each main invocation.
No particle expires during these five tick groups, so counts also equal the
total schedule count. All stored particles pass the final near-camera drawing
gates and generate two triangles each.

| Invocations at each controlled tick | Occupancy at ticks 1100–1500 | Last main triangles | First particle radial distance at 1500 |
| --- | --- | ---: | ---: |
| Main only | 1, 2, 4, 5, 7 | 14 | 0.01415009144693613 |
| Near preview, main | 2, 4, 5, 7, 9 | 18 | 0.01415009144693613 |
| Behind preview, main | 2, 4, 5, 7, 9 | 18 | 0 |
| Far preview, main | 2, 4, 5, 7, 9 | 18 | 0 |
| Near preview 0, behind preview 1, main | 2, 4, 5, 7, 9 | 18 | 0.01415009144693613 |

Every preview dispatch has zero world-lamp triangles because of its context
mismatch. Every invocation observes owner suppression false. Native radial
visibility is true for the near camera and false for the behind/far cameras.
Regardless of scenario, the first particle's final axial distance is 1 and
remaining life is approximately 0.15 seconds. Thus the behind/far preview
advances axial motion and life while preventing radial motion; the subsequent
main call has zero delta and cannot add that missed radial displacement.

Birth differences follow the original operation order, rather than a second
application of elapsed time. Schedule calculation at `0x10073410` reads
elapsed emitter duration before the call's final duration decrement at
`0x100761dd`. In the first tick group, a 0.1-second call still sees the original
zero-elapsed schedule and leaves occupancy one. The immediately following
zero-delta call sees that duration decrement and admits a second particle.
Later tick groups follow the same pattern. A third invocation at the same
tick does not repeatedly recreate the already consumed schedule difference.

The native unit counter ends at 63 for main-only and 77 for all preview
scenarios: 42 versus 56 new unit reads, exactly seven per newly admitted
particle. Signed counters end at 28 versus 32. In the two-preview scenario,
preview 1 itself admits the second birth and consumes its seven unit values at
tick 1100 despite context mismatch and a false radial-visibility gate. These
are native table-counter observations under the common diagnostic state, not
an inferred live RNG stream.

## Input contract and next blocker

An eventual stationary-lamp caller needs an **ordered invocation sequence**,
not just a final visible frame timestamp. Each actual invocation must supply
the engine tick, current camera and derived emitter/particle gates, current
owner pose/scale/alpha/suppression, and drawing context. Shared manager time
must advance once per actual invocation. Zero delta alone is insufficient to
skip the original update, and context mismatch alone is insufficient to skip
simulation or random consumption. State and randomness must preserve the
native list/emitter traversal order within the admitted scope.

A scene that proves no previews are enabled may legitimately have one admitted
main invocation per frame. This witness does not reject that case. It closes
the behavior of each invocation and the conditional interaction when preview
and main invocations both occur.

The next concrete cadence blocker is to establish, for the intended live PoK
entry path, who writes preview enabled byte `+0x10`, whether the surrounding
main update block is admitted, and when the host refreshes the engine tick
relative to these calls. Capture those inputs or trace their original writers
before selecting a live schedule. Full owner-suppression applicability,
camera/culling policy, live emitter creation order and random-table origin
remain separate admission requirements; the controlled unsuppressed owner
here does not close them. Existing sampler, owner, texture and GPU diagnostic
evidence remains necessary and does not automatically enable live rendering.

## Reproduction and checkpoints

Run the separate probe with:

```text
PYTHONPATH=/tmp/openeq-re-tools python3 /tmp/openeq-particle-lamp-cadence.py
```

It writes only `/tmp/openeq-particle-lamp-cadence.json`. Its companion log is
`/tmp/openeq-particle-lamp-cadence.log`. All assertions passed for 25 tick groups
and 50 admitted update/draw invocations, comprising 200 native list updates.
The prior runtime/placement JSON checkpoints remain unchanged.

| Input or artifact | SHA-256 |
| --- | --- |
| `EQGraphicsDX9.dll` | `615e7ba9e20745ec03a908cf47c038cc764358ad820d9d4d3232734fadff5383` |
| Native `d3dx9_30.dll` | `5edeed79f2359527a55b8189cfa8b9b121cd608d44eead905a0f3436938ad532` |
| Object WLD | `e7fbed560a7b81bbfe7495440418de0020f98ef84f63b3f5e4850fcfc2af10e3` |
| Placement WLD | `f872a141e7600399ea00f8f4c6eb965fe03002251ca4be3c6adcb4fd5c3a11d2` |
| Setup runtime script | `9e7b3158d9022e3f99c9939e45fbf7618e4c143a25ab2813f919a760e21efcd0` |
| Setup placement script | `abf53f5e09ae584933b7b172c2b92b86c1cc3a7866a6f56c529c168cefbdb164` |
| Setup WLD helper | `bfe0df62f37beca4d4990cb0b99b84bf48c321846e8affacd778cb38cf667753` |
| Cadence probe | `555ac44c5d25db7b389cdc105f7e6d71f44e55ce654ea015616f60bd3a8d29ee` |
| Cadence JSON | `a51e1fc6dc92875bcc007c2e467ed6166a0ba8f00640d9fbd07ff7997b8dcd6b` |
