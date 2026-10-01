# Embedded TOG areas: initialization follow-up

Static research, 2026-10-01. This follow-up strengthens the unsupported boundary
in [EQG_GROUP_REGIONS.md](EQG_GROUP_REGIONS.md). The inspected construction path
allocates areas without zero-fill, leaves their grid words unwritten, and never
calls the separate area transform. Construction callbacks, the group interface,
and the executable's graphics loader do not supply a replacement initializer.
**Keep whole-set rejection for area-bearing groups.** The candidate transform
in the earlier document is not an established native placement contract.

This was a bounded static investigation. Neither native binary was executed;
no live player, server, credentials, or audio was used. No production behavior
changes accompany this document.

## Binary identity and scope

All addresses are preferred virtual addresses in these installed binaries:

| Binary | Preferred base | Bytes | SHA-256 |
| --- | --- | --- | --- |
| `EQGraphicsDX9.dll` | `0x10000000` | 1,615,360 | `615e7ba9e20745ec03a908cf47c038cc764358ad820d9d4d3232734fadff5383` |
| `eqgame.exe` | `0x00400000` | 11,678,208 | `bab4ee0bd724b80c85de7df7020e7049a2bedaa1eca65b1abc59294c472cd593` |

The reference scan searched the area-update body `[0x101046c0,0x10104a2b)`
and corresponding RVA interval `[0x001046c0,0x00104a2b)`. It checked decoded
calls/jumps and instruction operands in executable PE sections, plus unaligned
32-bit values in all section bytes. Targeted disassembly followed allocation,
construction callbacks, the group vtable, and the executable's graphics-module
loader. This does not establish a complete interprocedural proof against every
possible computed call, external writer, or another client build.

## Allocation supplies no default grid

Runtime area factory `0x100a3d40`, reached through terrain vtable
`0x10140644+0x58`, pushes size `0xa0` at `0x100a3d56` and calls operator new
`0x1010eebc` at `0x100a3d5b`. The successful allocation path is:

```text
0x1010eed6  call 0x10115397
0x101153d3  push allocation_size
0x101153d4  push 0                 ; HeapAlloc flags
0x101153d6  push [0x101d55e8]       ; heap handle
0x101153dc  call [0x10133114]       ; imported HeapAlloc
```

The returned pointer is passed through unchanged. No zero-fill operation occurs
before the factory calls derived constructor `0x100a0c00` at `0x100a3d74`.
The derived constructor calls base constructor `0x100f12a0`, installs vtable
`0x1013ff0c`, and clears its additional byte at `+0x9c`.

Base construction zeroes local XYZ (`+0x14..+0x1c`), XYZ rotation
(`+0x20..+0x28`), scale (`+0x2c..+0x34`), and authored Z offset (`+0x38`).
It **does not write grid words `+0x0c/+0x10`**. The separate debug allocation
at `+0x98` does not overlap those fields. Because `HeapAlloc` receives flags
zero, the unwritten words have no guaranteed zero value. Their actual contents
depend on the allocation history and were not observed in this investigation.

Consequently, the original Ocean Green Hills fixture cannot be interpreted as
an area at the parent origin, world origin, or any other deterministic fallback.
Along the inspected path, and absent an additional writer, its runtime state
would be:

| Area property | State before registration |
| --- | --- |
| Grid | Uninitialized heap contents |
| Local XYZ | `[0,0,0]` |
| XYZ rotation | `[0,0,0]` |
| Full extents | Original authored extents, unchanged |
| Authored area Z offset | `0` |
| Type / shape | `0` / `Box` |

The original source area's nonzero position and `[-180,0,-90]` rotation are
preserved in the definition, but are not copied by construction. Registration
reads the runtime composite, including the uninitialized grid, so its actual
center is not recoverable from the source fixture alone. The earlier document's
candidate center remains conditional on invoking the dormant transform.

## Construction callbacks do not repair the composite

The creation loop at `0x10104d99..0x10104de9` sets name, type, shape,
authored Z offset, full extents, and dirty state. The setters can call virtual
refresh `+0x60`, so examining only the loop's direct stores would be incomplete.
The active area vtable resolves the relevant slots as follows:

| Slot | Target | Effect relevant to initialization |
| --- | --- | --- |
| `+0x24` | `0x100a0bd0` | Name setter; forwards string and updates name-prefix flag |
| `+0x30` | `0x100f1b70` | Composite setter; not invoked by this creation loop |
| `+0x4c` | `0x100c9ed0` | Builds debug basis from existing rotation |
| `+0x5c` | `0x100f1290` | Writes dirty byte `+0x51` |
| `+0x60` | `0x100f1420` | Refreshes basis and optional debug vertices |

Refresh calls `+0x4c` at `0x100f143d`. The basis routine's area writes are
limited to `+0x58/+0x5c/+0x60`, `+0x68/+0x6c/+0x70`, and
`+0x78/+0x7c/+0x80`. It reads the existing rotation and supplies no grid,
position, or rotation writes.

At `0x100f1444`, refresh skips its remaining work unless engine field `+0x1c`
equals one. The remaining branch reads grid words at `0x100f145e` and
`0x100f146e`, copies the composite to stack arguments, converts it to world
coordinates, and writes debug vertices through the separate `area+0x98`
pointer. Its final loop at `0x100f1b00..0x100f1b12` writes vertex colors.
These are consumers of the current composite, not coordinate initializers.

## Group interface and executable references

The 16-entry group vtable at `0x10144cfc` exposes composite, position,
rotation, scale, Z adjustment, name, flags, bounds, and destruction. It contains
neither the area updater nor an area-list accessor. Its five placement setters
resolve to `0x10104fa0`, `0x10104f30`, `0x10104f50`, `0x10104f70`, and
`0x10104f90`; each updates objects through `0x10104340`. Construction likewise
ends with the object updater. No area-update dispatch was found in this
interface.

The DLL exports exactly five functions: `CreateGraphicsEngine`,
`GetGraphicsEngineInfo`, `ReleaseGraphicsEngine`, `EQG_GetCpuSpeed2`, and
`EQG_GetCpuSpeed3`. It does not export the group-area updater. The executable
has no static import from the graphics DLL; loader `0x009333a0` loads it and
resolves those five exported names. It calls `CreateGraphicsEngine` at
`0x0093369e` and saves the resulting interfaces. Every decoded reference to
module handle `0x01822660` is in this loader or its teardown. Inspection found
no addition of the updater RVA to that handle.

Broader reference checks produced these results:

| Check outside the updater body | Graphics DLL | Executable |
| --- | --- | --- |
| Decoded direct branch into updater | None | None |
| Decoded operand in updater VA or RVA interval | None | None |
| Raw unaligned 32-bit absolute candidates | None | Six instruction-byte overlaps |

The executable's six raw absolute candidates are false positives, not pointers:
`0x007939d2` overlaps the encoding of `call 0x00894a20`; `0x007d2ae5` and
`0x007d453c` overlap `mov [esi+0x10],0x00af9c10`; `0x008dcecb`,
`0x0099778b`, and `0x009a68e5` overlap `mov [esi+0x10],0x10`. Raw RVA
matches similarly overlap instruction bytes. The lone executable data-section
RVA candidate at `0x00b0671f` straddles adjacent aligned small-integer entries,
including `0x5c5b`, `0x1049`, and `0x587e`.

The semantic scan decodes actual operand strings, avoiding disassembler symbol
annotations that can contain unrelated offsets. It scanned 1,249,792 DLL text
bytes and 7,010,816 executable text bytes. Capstone skip-data handling advanced
over 15 and 1,054 undecodable bytes respectively; linear decoding is supporting
evidence, not proof of every possible instruction boundary or control path.

## Independent reference parser

The checked `EQEmu/zone-utilities` source at commit
`b361e63dd067e8959f5bf2341579f481d2374fd5` does not close this gap.
`src/common/eqg_v4_loader.cpp:331..396` loads top-level DAT regions, but its
TOG handling at `424..542` recognizes object blocks and their name, position,
rotation, and scale tokens. It has no `BEGIN_AREA`, `END_AREA`, or `EXTENTS`
handling and no group-loop `AddRegion` call. Its object placement arithmetic
cannot serve as evidence for a native embedded-area initializer.

## Reproduction and support decision

Temporary evidence retained for this investigation:

- `/tmp/openeq-group-followup-references.json`: full raw VA/RVA candidates and
  direct-branch scan results.
- `/tmp/openeq-group-followup-semantic.py` and matching `.json`: repeatable PE
  identities, exports, vtables, decoded operand/branch scans, module-handle
  references, and targeted callback/allocation disassembly. Run with the
  existing `pefile` and `capstone` environment; the script only reads binaries.

No production tests were needed for this documentation-only investigation.
Original fixture parsing and transform arithmetic remain in the earlier report;
this follow-up does not reinterpret that arithmetic as an observed native result.

The actionable boundary remains rejection of the whole region set when an
embedded area is present. Supporting it requires a demonstrated reachable
initializer or equivalent writer before registration, followed by original
fixture comparisons. Replaying the dormant routine would invent a behavior;
emulating unwritten heap fields would supply no stable compatibility contract.
