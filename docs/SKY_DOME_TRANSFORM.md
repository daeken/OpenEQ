# Native D3DX sky transform and pole-aligned times

The imported DirectX matrix routine has now been executed with the original
sky time setter and dome draw. The local positive-Z pole maps toward the
**negative** sky axis. At exactly parallel input, the x87 and SSE backends
produce a matrix with zero lateral basis vectors; they do not choose an
alternate up vector. SSE and SSE2 also collapse the tiny nonzero cross products
at raw native times **0.5 and 1.0**. The scalar x87 backends preserve those
small directions instead.

This resolves the D3DX numerical boundary left open in
[the frozen geometry report](SKY_DOME_GEOMETRY.md), within the explicit
backend, floating-point and emulator conditions below. It does not establish
complete client pixels or a visible sky disappearance at those times.

## Original library and execution

The graphics DLL imports `d3dx9_30.dll`; its `D3DXMatrixLookAtRH` import address
is `0x10133274`, reached through trampoline `0x100b9b34`. The Microsoft x86 DLL
was already extracted locally from `Apr2006_d3dx9_30_x86.cab` in the official
[June 2010 DirectX redistributable](https://download.microsoft.com/download/8/4/A/84A35BF1-DAFE-4AE8-82AF-AD2AE20B6B14/directx_Jun2010_redist.exe).
Acquisition provenance is also recorded in
[the object-animation report](WLD_OBJECT_ANIMATION.md).

| Binary | SHA-256 |
| --- | --- |
| `EQGraphicsDX9.dll` | `615e7ba9e20745ec03a908cf47c038cc764358ad820d9d4d3232734fadff5383` |
| `d3dx9_30.dll` | `5edeed79f2359527a55b8189cfa8b9b121cd608d44eead905a0f3436938ad532` |

The D3DX image is mapped at its preferred base `0x00400000`, alongside the
graphics DLL at `0x10000000`. No host `eqgame.exe` image is mapped in this
witness. D3DX addresses below refer to that Microsoft DLL, not the host.

The new probe reuses the frozen geometry harness, removes its controlled
look-at callback and resolves the actual import to the Microsoft export
`0x00442925`. The original EQ time setter, complete dome draw, imported
look-at code, normalization backend and native transpose all execute.
Graphics device calls still record their arguments; this is not GPU execution.

D3DX math initialization `0x004465d2` executes its original table reset and
backend selection. General allocation/debug initialization, registry queries
and processor-feature interfaces are controlled to select each backend.
The numerical routines are not replaced. A Unicorn Phenom CPU model with
SSE enabled permits execution of the 3DNow instructions as well.

Two explicit floating-point environments are exercised:

| Label | x87 control word | MXCSR |
| --- | --- | --- |
| Nearest, extended precision | `0x037f` | `0x1f80` |
| Host-requested toward-zero state | `0x0e7f` | `0x7f80` |

The latter corresponds to the executable’s requested PC53/toward-zero mode
described in [the FPU lifecycle report](EQG_TER_FPU_LIFECYCLE.md). Its presence
through every real sky call remains unproven. The witness records both states
rather than assuming one universal live-client state.

## Basis construction and orientation

The Microsoft function computes the following basis, where `N` means the
selected native normalization routine:

```text
Z = N(eye - at)
X = N(up cross Z)
Y = Z cross X
```

The sky supplies `eye=[0,0,0]`, `at=A` and `up=[0,0,1]`. After the original
transpose and camera-translation overwrite, row-vector world transformation
has basis rows `X`, `Y`, `Z` and translation row `C`:

```text
W = [ X.x X.y X.z 0 ]
    [ Y.x Y.y Y.z 0 ]
    [ Z.x Z.y Z.z 0 ]
    [ C.x C.y C.z 1 ]

worldPosition = local.x*X + local.y*Y + local.z*Z + C
```

For a nonsingular time axis `[0,s,c]`, the ideal mathematical basis is:

```text
X = [sign(s), 0, 0]
Y = [0, -c*sign(s), abs(s)]
Z = [0, -s, -c]
```

Native normalization can change the last bits and even retain a length
slightly different from one. Thus these expressions establish handedness
and orientation, not a replacement for the native numeric routine.

For asymmetric axis `[.25,.5,.75]`, SSE/SSE2 in the nearest witness produce
these basis rows:

```text
X = [ 0.8944271803, -0.4472135901,  0             ]
Y = [-0.3585685492, -0.7171370983,  0.5976142883 ]
Z = [-0.2672612369, -0.5345224738, -0.8017836809 ]
```

All finite, noncollapsed cases agree with the independent ideal basis within
`1e-5` per element. The complete float32 words, including signed zeros, are
retained in the result JSON. Inputs `[0,1,0]` and `[0,-1,0]` separately verify
the opposite X directions and the positive local-Y-to-world-Z direction.

## Time setter: true zero versus trig rounding

Original setter `0x100c2c40` reads a float32 time, multiplies it by the stored
float64 pi constant at `0x10136ee8`, doubles the product, then executes x87
`fsin` and `fcos`. The constant’s little-endian bytes are
`182d4454fb210940`, the usual nearest float64 value of pi. The resulting
axis is stored as float32 at sky `+0x10..+0x18`.

These are the exact setter outputs in the nearest witness:

| Raw native time | Axis | Float32 Y word |
| --- | --- | --- |
| `0` | `[0, +0, 1]` | `00000000` |
| `0.5` | `[0, 1.2246468525851679e-16, -1]` | `250d3132` |
| `1` | `[0, -2.4492937051703357e-16, 1]` | `a58d3132` |

Toward-zero stores instead give Y words `250d3131` and `a58d3131` for the
latter two times. Their values are still nonzero. Raw `t=1` is **not wrapped**
to zero by this setter and must not be substituted with the public loader’s
normalized color-table time. Equality of sampled table colors does not prove
equality of the native axis.

The probe also executes the immediately adjacent float32 values around
`0.5` and `1`, a minimum positive subnormal time, two small positive times,
quarter-day times and asymmetric explicit axes. This distinguishes actual
parallel vectors from near-parallel vectors caused by finite-precision trig.

## Backend-dependent normalization

`D3DXVec3Normalize` at `0x00440b3b` dispatches through pointer `0x0061e064`.
Original initialization selects these entries:

| Selected backend | Mode ID | Entry | Relevant behavior |
| --- | --- | --- | --- |
| Reference x87 | `0xffff` | `0x004446f3` | Near-unit shortcut; square root; zero for squared length at or below `FLT_MIN` |
| Optimized x87 | `0` | `0x0044a915` | Near-unit shortcut and reciprocal-length lookup; zero when stored squared length is zero |
| SSE | `3` | `0x006111f1` | Zero below squared length `2^-46`; reciprocal-square-root estimate plus refinement |
| SSE2 | `2` | `0x006138a5` | Same cutoff and refinement structure |
| 3DNow | `1` | `0x0060daff` | MMX/3DNow reciprocal-square-root refinement and an `FLT_MIN` mask |

The SSE constant is float32 `0x28800000`,
`1.4210854715202004e-14`, at D3DX `0x0042a680`; SSE2 uses the same constant at
`0x0042a800`. The `comiss`/`jae` sequence admits equality. Independent direct
normalization calls verify that `[2^-23,0,0]` survives, its immediately smaller
float32 neighbor becomes zero, and its immediately larger neighbor survives.
The tiny noon sine becomes zero in SSE/SSE2 but survives both x87 backends.

Consequently, in both tested floating-point states:

| Raw time | Reference/optimized x87 | SSE/SSE2 |
| --- | --- | --- |
| `0` | X and Y basis rows zero | X and Y basis rows zero |
| `0.5` | Finite nonzero lateral basis | X and Y basis rows zero |
| `1` | Finite nonzero lateral basis with opposite X sign | X and Y basis rows zero |

At `t=0`, the finite collapsed world matrix is, omitting signed-zero spelling:

```text
[0 0  0 0]
[0 0  0 0]
[0 0 -1 0]
[10 -20 30 1]
```

At `t=.5` and `1`, SSE/SSE2 preserve the small Y component in the Z basis,
but the first two basis rows are still zero. Their linear transform has rank
one. There is no look-at fallback that chooses another up vector.

The 3DNow instructions execute under the Phenom emulator model. They produce
NaN lateral values for the true-zero cross product, while `.5` and `1` remain
finite. Those emulated words are recorded, but legacy physical-CPU behavior
is not independently certified. More generally, reciprocal-square-root ISA
estimates can vary in their last bits across processors; the frozen matrices
are exact outputs of these original instructions under this witness, not a
promise of identical words on every historical x86 CPU. The explicit SSE
cutoff and the distinction between zero and tiny native sine remain directly
established by the original instructions.

## Outer draw path and limits on visual conclusions

This dome routine is used by both direct renderer calls and the cubemap path.
Static direct calls to outer wrapper `0x1002d0f0` occur at:

```text
0x1008c152    argument 0
0x1008c1a9    argument 1
0x1008c1d5    argument 1
0x1008c47a    argument 0
```

These read the sky from engine `+0x44`. Separately, renderer `0x10097f4e`
calls `0x1002cdd0`; its six-face loop calls immediate sky draw `0x1002cb80`
at `0x1002d04a`. Both paths eventually call dome draw `0x1002f630`.
The cubemap path has its own resource and update gates; no complete frame
schedule is inferred here.

Eight additional executed SSE2 outer calls cover arguments 0 and 1 at raw
times `0`, `.25`, `.5` and `1`. The ordinary argument-0 cases enable the sky
flags at `+0x220/+0x221` and its mode at `+0x1f8`; secondary sky objects are
absent. Both arguments pass through the collapsed dome matrix at the three
special times and still issue exactly one 1,861-primitive draw. Neither takes
the immediate wrapper’s device-Clear fallback. Argument 0 also performs the
usual additional world-transform reset before the absent secondary objects.

The immediate wrapper only falls back when the dome draw returns false;
the dome does not inspect matrix rank. This proves submission through those
bounded paths. The active sky flags, CPU backend, live FPU state, surrounding
render passes, sky objects, cached cube contents and actual rasterized frame
remain separate observations. A claim that the complete visible sky vanishes
for a game minute would require those observations.

## Frozen verification

The witness executes **170 direct dome cases** across five math backends and
two floating-point states, **35 direct normalization boundary cases**, and
**eight outer sky calls**. It executes the real Microsoft look-at export 178
times and the normalizer 391 times. Assertions cover input-word distinctions,
finite-basis orientation, the SSE cutoff, singular matrix submission, camera
translation, primitive count and absence of the wrapper’s Clear fallback.
No production code, GPU test or build is changed by this research.

Run:

```sh
PYTHONPATH=/tmp/openeq-re-tools python3 /tmp/openeq-sky-dome-transform.py
```

| Artifact | SHA-256 |
| --- | --- |
| `/tmp/openeq-sky-dome-transform.py` | `c73dc65d77efe2d6f20c2d22f34cd0128d10d290ce548862e0ea8fecf88f7569` |
| `/tmp/openeq-sky-dome-transform.json` | `537030068fe053b66c0dcaf4ed8dcaa7b491dd292e2558a2da183dec84717a75` |
| Frozen geometry harness dependency | `aabc0f34cdbcfa5d69f97dc16c82363f7d95761495118b445319fbbfa092afac` |
