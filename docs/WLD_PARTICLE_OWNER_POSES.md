# Static placed WLD particle owner transforms

2026-10-01. `ObjectSource::diagnostic_particle_owner_transforms(&Instance)`
connects retained particle attachment identities to explicit placed-owner
matrices in native coordinates. This is an opt-in CPU diagnostic, with no
call from live scene loading, rendering, or effect updates.

The API returns all 475 attachment matrices for the 366 particle-bearing
placements in the installed Plane of Knowledge. The existing bounded sampler
accepts 462 of them. It deliberately rejects the other 13: `FTORCH302` has a
slightly non-unit packed quaternion that the original node constructor does
not normalize. Normalizing it would hide a real difference from the source.

This extends [native placement research](WLD_PARTICLE_PLACEMENT.md) and the
[CPU sampler contract](WLD_PARTICLE_SAMPLER.md). It is independent of the
[projected GPU diagnostic](WLD_PARTICLE_GPU_DIAGNOSTIC.md).

## Coordinate and hierarchy contract

`Scene::instances` and the retained WLD `Frame` values use EQ's native XYZ,
with Z up. Instance rotation is XYZW. The WLD placement reader converts raw
Z/Y/X angles in pi/256 units to X/Y/Z radians; the existing loader builds
`Rz * Ry * Rx`. The diagnostic uses the already-decoded instance quaternion
and scale unchanged. It does not recover the original angle-table values
from that quaternion or promise original-client bit parity for arbitrary
placements.

In glam's column-vector convention, the explicit composition is:

```text
world[root]  = placement * raw_first_local[root]
world[child] = world[parent] * raw_first_local[child]
native_world_rows = world.to_cols_array_2d()
```

The final line is intentional. Native matrices use row vectors, so the
numeric column array of the equivalent glam matrix is already the native
row array. Translation is row 3. An additional transpose is incorrect.
The original recursion computes `local * parent` in row-vector notation;
the two expressions are equivalent.

The raw first local matrix uses the retained packed quaternion, including
its W sign and magnitude. Original decoder `0x1001ae20` negates packed W,
then skeleton-reader instructions `0x1001e122..0x1001e12a` negate frame-zero
W again. The resulting initial node uses raw XYZW. This differs from the
ordinary mesh first-pose helper, which normalizes the quaternion. It also
differs from animated-track handling; this API admits no animated tracks.

OpenEQ's later rendering conversion `(x,y,z) -> (x,z,-y)` is a determinant
positive rotation, not a reflection. Do not apply it to these matrices before
calling `OwnerPose::from_native_world`. Likewise do not preapply emitter
axes or the mode-3 quarter turn: the sampler handles the final normalized
row permutation `[1,0,2]` itself. Negative, mixed-sign and nonuniform instance
scales are rejected explicitly rather than losing their signs in a row norm.

## Admission and identity

The method is available on the existing public `ObjectSource`. Its return
type, `loader::wld_objects::ObjectParticleOwnerTransform`, contains:

- The index in `particle_attachments`, in unchanged attachment order.
- The owning track index and exact source/definition `Ref` identities.
- `native_world_rows: [[f32;4];4]`, before rendering conversion.

The supplied instance must name the actor's existing canonical object key.
Position must be finite; scale must be exactly uniform, positive and have a
finite reciprocal. Placement squared quaternion length must be within
`1e-5` of one. No quaternion normalization or scale snapping takes place.

Skeleton admission requires one root at index 0, ordinary node flags 0,
one packed frame per track, reference flags 0, and no timing word. The
hierarchy is recomputed with the existing 4096-track bound and compared with
the retained parent/order arrays before indexing. Attachment indices must
be valid and unique by owner track; each positive source reference must
match both the owner's attachment reference and exact definition identity.
These are the direct-fragment bindings retained by the current loader.
Repeated definition references on different owner tracks remain distinct.

Frame quaternion and translation components must still lie on their signed
packed-word grids (divisors 16384 and 256). Scale is the positive unsigned
packed word divided by 256, matching the native reader. Squared quaternion
length must be within `0.01` of one; this is an explicit diagnostic admission
bound, not a native engine limit. Every composed transform must be finite
and have a positive nonzero determinant. Unsupported layouts, stale public
caches, cycles, reflection, and composition overflow/underflow return errors.
An empty attachment list returns an empty result after placement validation.

The result preserves raw matrix distortions. Extracting a matrix does not
guarantee `OwnerPose` admission, an accepted particle body, supported texture
state, or permission to activate it in a world. A caller can use the returned
attachment index to retain the exact source body and authored texture chain;
it must not replace that identity with a same-name lookup.

## Extended original-x86 placement witness

The original two-placement witness remains unchanged. A separate execution
adds real `POKTORCH500` scale 1.25 and `FTORCH302` scale 1.5 placements, using
the same installed graphics DLL and D3DX binary:

| Binary | SHA-256 |
| --- | --- |
| `EQGraphicsDX9.dll` | `615e7ba9e20745ec03a908cf47c038cc764358ad820d9d4d3232734fadff5383` |
| `d3dx9_30.dll` | `5edeed79f2359527a55b8189cfa8b9b121cd608d44eead905a0f3436938ad532` |

Archive/member hashes are unchanged from
[WLD_PARTICLE_PLACEMENT.md](WLD_PARTICLE_PLACEMENT.md). The extended harness
executes the original rotation-setter block `0x10042dfd..0x10042e68`, including
its three scale-getter calls, D3DX scaling/multiply and model matrix write.
The existing local actor matrix is supplied on its stack. Scale comes from
the controlled owner data at `+0x64`, reached through the native virtual
getter chain. Full placement construction, world notifications and the
surrounding function epilogue remain outside the witness.

The original packed decoder, frame-zero adjustment, node constructors,
recursive composition and particle update execute as in the earlier witness.
All real source tracks have local scale 1. Additional portable composition
tests cover cumulative positive packed local scales algebraically; no new
native witness of those artificial track scales is claimed.

| Actor | Placement fragment | Scale | Cloud | Original first particle origin XYZ |
| --- | ---: | ---: | ---: | --- |
| FTORCH301 | 18 | 1 | 411 | `(140.5546875,1375.0556640625,-100.84272766113281)` |
| FTORCH301 | 18 | 1 | 406 | `(140.5546875,1375.0556640625,-100.43257141113281)` |
| POKLAMP500 | 571 | 1 | 20 | `(925.6063232421875,144.00927734375,-124.46380615234375)` |
| POKTORCH500 | 254 | 1.25 | 20 | `(-783.1561889648438,666.5137329101562,-143.42376708984375)` |
| FTORCH302 | 536 | 1.5 | 438 | `(-332.4253234863281,755.8621215820312,-87.62789154052734)` |

All five origins are bit-exact after composing the existing OpenEQ instances
with raw locals. Across every node of these four placements, maximum matrix
component difference is `1.1920928955078125e-7`, attributable to the existing
placement rotation representation. The portable tests allow `2e-7` for basis
components and require exact f32 translation. This is numeric tolerance for
these fixtures, not a bit-exact matrix promise across hosts or arbitrary angles.

`FTORCH302` leaf `FTO302P1F_DAG` has raw WXYZ `(11585,11585,0,0)`, whose
squared magnitude is `0.9999589994549751`. Its original local Y/Z diagonal
is `0.00004100799560546875`, with off-diagonal magnitude
`0.9999589920043945`. At placement scale 1.5, the native world basis is:

```text
(0,                       1.5, 0)
(-0.000061511993408203125, 0,   1.4999384880065918)
(1.4999384880065918,       0,   0.000061511993408203125)
```

Normalizing that source quaternion changes matrix components by up to
`6.142258644814547e-5` after placement, and falsely passes the current sampler's
uniform-axis gate. The raw diagnostic preserves the distortion, and that
sampler correctly reports `UnsupportedOwnerTransform`. This work does not
broaden the sampler or alter ordinary mesh geometry.

## Reproducibility and validation

Frozen local research artifacts are under `/tmp/openeq-particle-owner-probe`;
the source assets, DLLs, library copies and temporary executable are not
committed. The initial Rust prototype used pinned pre-change libraries,
recorded in `provenance.json`. It compared normalized and raw paths explicitly;
its successful normalized-path sampling is a comparison, not the final API.

| Artifact | SHA-256 |
| --- | --- |
| `native-scaled.py` | `9ea25b397b0dfe9fcb3250e9998af9098a6174aee439902ab9203c45e7686e17` |
| `native-scaled.json` | `a51f6b5e38e5827c2e24d1085a18fb05e42a6e86543d6568185c55b629a48225` |
| `probe.rs` | `8c45b04f860a710cfe68bf95e228050741913d6a2d5e71f7146a911bff266d5a` |
| `results.json` | `609ba18f5f3fdce8ade5bbb8b7223cdba9b6b2605e187fb1b93476ff43b1cde2` |
| `provenance.json` | `86ddb1504cab291adaf9138885c486cd1f779045b4339a33a4bdf75126ca4e61` |

`cargo test -p openeq-assets particle_owner --lib` runs six portable tests.
They check the four native placement witnesses, raw quaternion sign/magnitude,
hierarchical placement/local scale, exact attachment identity, unsupported
inputs and public hierarchy corruption, and composition overflow/underflow.

`EQ_DIR=/path/to/EverQuest cargo test -p openeq-render --test
wld_particle_owner_poses -- --ignored --nocapture` loads the original zone and
passes all 475 returned matrices directly to `OwnerPose`. The 462 supported
matrices each produce one zero-time birth at the extracted origin; the 13
`FTORCH302` matrices fail explicitly. All five recorded native origins match.
This check is CPU-only and does not connect a client or enable effects.

Live emitter creation/update ordering, camera and visibility policy, texture
cache ownership, world-space GPU geometry and rendering state remain separate
integration work. This diagnostic supplies explicit transform data for that
work without copying temporary harness transforms into the live renderer.
