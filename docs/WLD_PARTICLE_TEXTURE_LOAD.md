# Native WLD particle indices and DDS load boundary

2026-10-01. The native particle index buffer uses `0,1,3,1,2,3` for each
four-vertex quad. For the original full-sized CSMOKE and GENG00 textures, native
Microsoft D3DX preserves every level-zero BC1 byte through texture creation and
the particle texture's surface copy on the controlled BC1-capable device below.
That establishes the block and row order on this path. It does not establish an
actual original-client framebuffer or driver-generated mipmaps.

This extends [the GPU contract](WLD_PARTICLE_GPU_CONTRACT.md) and closes parts of
its deliberately unresolved index and image-load boundaries. The
[standalone diagnostic](WLD_PARTICLE_GPU_DIAGNOSTIC.md) now uses the verified
index order. No automatic world-particle integration is enabled.

## Sources and controlled boundaries

The installed graphics DLL is `/Users/daeken/EverQuest/EQGraphicsDX9.dll`, image
base `0x10000000`, SHA256
`615e7ba9e20745ec03a908cf47c038cc764358ad820d9d4d3232734fadff5383`.

Its imported `d3dx9_30.dll` is the matching Microsoft x86 library already obtained
for [the animation witness](WLD_OBJECT_ANIMATION.md#reproducible-source), from
`Apr2006_d3dx9_30_x86.cab` in the official June 2010 DirectX redistributable. The
local file `/tmp/openeq-d3dx9-native/d3dx9_30.dll` has image base `0x00400000` and
SHA256 `5edeed79f2359527a55b8189cfa8b9b121cd608d44eead905a0f3436938ad532`.
`0x100...` addresses below belong to EQGraphics; `0x004...` addresses belong to
D3DX. These libraries and original assets are not repository fixtures.

The temporary Unicorn 2.1.4 x86 witnesses execute unchanged instructions and
data from those images. They substitute the following boundaries:

- The index witness supplies successful `CreateIndexBuffer`, `Lock` and
  `Unlock` services and guarded writable memory. The original routine writes
  every index; the witness does not supply index values.
- The EQ image witness supplies archive open/length/read/close operations using
  the exact original DDS bytes, bounded allocation and ASCII comparison. It
  intercepts D3DX creation to capture the complete call and source bytes.
- A separate D3DX witness executes the original creation, requirement checking,
  DDS parser, image copy and filtering functions. Its D3D COM device reports
  support for the requested BC1 format, managed textures and mipmaps. It
  provides real writable rows in emulator memory, with 16 bytes of padding per
  block row and 16-byte guards around each surface. D3DX writes the contents.
- The controlled device reports texture caps `0x4106`, maximum texture width
  and height 4096, maximum volume extent 256 and aspect ratio 4096. Format checks
  succeed. This establishes the supported-format path, not fallback behavior
  on a different real adapter.
- Used CRT shims supply a C locale, string duplication, allocation and deletion.
  Optional `GetModuleHandleA` lookups for `d3d9.dll`/`d3d9d.dll` return null and
  the optional `Software\Microsoft\Direct3D` registry lookup returns not found.
  No D3DX image, codec, requirement or filter routine is substituted. The test
  sets x87 control word `0x37f` and MXCSR `0x1f80`; full DLL/host initialization
  and a real Windows D3D device are not executed.

## Executed native shared index buffer

The index owner is the object at global `0x101d2fb0`. Its initialization chain
`0x100af780 -> 0x100b4940 -> 0x100b0690` creates the shared buffer. The particle
draw at `0x100725a5..0x100725bb` reads owner `+0x0c` and passes that exact buffer
to the native index-binding cache.

Executed routine `0x100b0690` requests:

| Parameter | Native value |
| --- | --- |
| Byte length | `0x1fffe` = 131070 |
| Usage | `0x208`: DYNAMIC and WRITEONLY |
| Format | `101`: INDEX16 |
| Pool | `0`: DEFAULT |
| Lock | Offset 0, size 0, flag `0x2000`: DISCARD |

The loop at `0x100b0730..0x100b0782` fills `0x2aaa` = 10922 quads:

```text
quad n: [4*n, 4*n+1, 4*n+3, 4*n+1, 4*n+2, 4*n+3]
first:  [0, 1, 3, 1, 2, 3]
last:   [43684, 43685, 43687, 43685, 43686, 43687]
```

The witness compares the complete 131064-byte written stream to independently
constructed index values. The remaining six bytes and both allocation guards
retain their sentinels. The original routine stores the allocated buffer in
owner `+0x0c`, unlocks it and succeeds.

With captured corner order TL, TR, BR, BL, the diagonal joins TR and BL. The
diagnostic previously explicitly chose the other diagonal while this boundary
was untraced. Its revised expansion uses the native order. A synthetic GPU
regression colors only TR red and BL blue: pixel center `(3.5,3.5)` in an
8-by-8 quad must contain both. The alternate diagonal would interpolate the
two black corners there. This proves topology in the diagnostic, not native
rasterization/interpolation equivalence for arbitrary varying quads.

## Original archive bytes and EQ creation arguments

Both original inputs come from `poknowledge_obj.s3d` and contain a single BC1
level, with DDS pixel-format flags 4 (FOURCC):

| Image | Dimensions | DDS bytes | DDS SHA256 |
| --- | --- | ---: | --- |
| CSMOKE.DDS | 16 × 32 | 384 | `e90cdb641d5af44ba7d943c2587b4fe8e74f92c95fcffaa7262682a7092c8d88` |
| GENG00.DDS | 64 × 64 | 2176 | `8dd0be192449bff88c973cf6cca0252d65112b5f89b298da164cb0ad1caed46f` |

The original archive-backed loader `0x100610d0` reaches import wrapper
`0x100b9af2` / IAT `0x101332a0`, verified by the import table as
`D3DXCreateTextureFromFileInMemoryEx`. Twelve controls cover the two originals,
renderer quality field `+0xefc` in `{0,2,3}` and mip byte `+0xee6` in `{0,1}`.
Every call supplies the complete DDS unchanged, format DXT1, usage 0, pool
MANAGED (1), color key 0 and mip filter BOX (5).

| Quality field | CSMOKE requested dimensions / filter | GENG00 requested dimensions / filter |
| --- | --- | --- |
| 0 | DEFAULT / NONE (1) | DEFAULT / NONE (1) |
| 2 | 8 × 16 / BOX (5) | 32 × 32 / BOX (5) |
| 3 | 0 × 0 / LINEAR (3) | 16 × 16 / LINEAR (3) |

These are captured numeric field values, not inferred UI preference names.
They are requests; the quality-3 CSMOKE zero dimensions are not a zero-sized
GPU result. For quality 0, CSMOKE requests one mip regardless of the tested mip
byte. GENG00 requests one mip when that byte is zero and DEFAULT (`0xffffffff`)
when it is one. Both reduced-quality cases request one mip. The full-sized
diagnostic therefore corresponds to an explicit full-quality input assumption;
the source archive alone does not prove the running client's selected quality.

## Executed Microsoft D3DX texture and surface copies

The D3DX witness executes these original entry points:

| Entry | Function |
| --- | --- |
| `0x004be7c5` | D3DXGetImageInfoFromFileInMemory |
| `0x004c1f41` | D3DXCreateTextureFromFileInMemoryEx |
| `0x004c16b3` | Shared texture-file creation implementation |
| `0x004bec5d` | Texture requirement implementation |
| `0x004be805` | D3DXLoadSurfaceFromMemory |
| `0x004c0c95` | D3DXLoadSurfaceFromSurface |
| `0x004c10e8` | D3DXFilterTexture |

For each original image, creation uses the captured full-quality arguments
and independently controls requested mip levels to one or DEFAULT. D3DX's
actual `CreateTexture` request keeps the original dimensions and DXT1 format.
It writes the compressed level-zero payload unchanged into the supplied
positive-pitch surface. Comparison includes every byte, preserving block row
order and selectors within every block; all row padding and guards survive.

The witness then executes `D3DXLoadSurfaceFromSurface` with the particle
resolver's captured arguments: equally sized DXT1 source/destination, complete
destination rectangle, null source rectangle/palettes, DEFAULT filter and no
color key. The destination reports the assembled texture's usage `0x400`
(AUTOGENMIPMAP) and pool MANAGED. Every destination level-zero byte again equals
the original DDS payload. Source and destination guards remain unchanged.

The resulting compressed payload hashes, identical before upload, after
creation and after assembly copy, are:

| Image | Level-zero BC1 SHA256 |
| --- | --- |
| CSMOKE | `a0baafbfff0a24554533afbec40da4629094b75a802598acf4a8cf8440b0f861` |
| GENG00 | `441bd2132fca7360c47c2ecc9aabb813858b450780573bee8ffa3787799d9147` |

An asymmetric synthetic 8-by-8 DDS with four different BC1 blocks, differing
selector rows and three-color/transparent selectors undergoes the same two
creation and assembly-copy controls. Its complete level-zero payload is also
unchanged. This makes the orientation check independent of the artwork.

On this full-quality BC1-supported path, D3DX does not vertically flip,
recolor, apply a magenta key, or convert level zero to RGBA before upload.
The WLD vertex V reversal still applies once. The independent GPU decoder test
checks OpenEQ's BC1 decoding against the available GPU; it remains separate
from original-adapter decompression precision and final-frame rasterization.

## Three distinct mip boundaries

1. **Source file:** both original DDS files contain one stored level.
2. **Loaded source texture:** requesting DEFAULT on the controlled mip-capable
   device executes native D3DX BOX filtering and creates six levels for CSMOKE
   (16×32 through 1×1), seven for GENG00 (64×64 through 1×1), and four for the
   synthetic 8×8 input. All level guards survive and mip-zero bytes remain
   unchanged. Lower-level compressed hashes are captured as evidence from this
   native implementation, not as a claim about a real adapter. DEFAULT CSMOKE
   here is an additional control; the tested EQ full-quality call requests one.
3. **Assembled particle texture:** the EQ resolver requests levels 0 with
   AUTOGENMIPMAP and copies only the source's level-zero surface. Its lower
   levels belong to the D3D driver's automatic generation path. The D3DX source
   chain is not copied wholesale, so those source-mip hashes do not establish
   the final sampled lower levels.

Driver acceptance/fallback of the requested resource, autogenerated mip
contents/filter precision, reduced-quality resampling, non-BC1 fallback formats,
host state history, pixel-center equivalence and original-frame output remain
outside this witness. The standalone GPU diagnostic remains level-zero-only.

## Frozen reproducers

Run with `PYTHONPATH=/tmp/openeq-re-tools python3 <script>`. All assertions pass.
The EQ scripts reuse only the documented emulator setup/asset readers from
earlier temporary witnesses; the D3DX script is independent except its PFS
reader. SHA256s identify this frozen evidence:

| Temporary file | SHA256 |
| --- | --- |
| `/tmp/openeq-particle-native-indices.py` | `763ad6db59f5eb974607ebb147498664bc354b9a8ecb26e26d4b8fac8dcb54b3` |
| `/tmp/openeq-particle-native-indices.json` | `27b6d9440083ec3b2d3d6ca54f83ed553884ee7f9a034e2022f4c7ca0ad25e00` |
| `/tmp/openeq-particle-native-dds-upload.py` | `c71c72806183068bf6f4d2860c5fe5cf6f15d44a21eed080622ac555b18e0990` |
| `/tmp/openeq-particle-native-dds-upload.json` | `4ff8e7a25485a501dac04a521e54a979b676c50578aecfbe50b83a32ff4530b5` |
| `/tmp/openeq-particle-d3dx-textures.py` | `80595496b6b9ac92125e4815ebe556b4841d41efd9748c9a261b7c8d85668ca4` |
| `/tmp/openeq-particle-d3dx-textures.json` | `38036f70abfdb231825593c0c8db19f0f636d2f929a029e138c2f61f8e4f0111` |

The renderer's focused GPU suite now passes eight tests, including its native
diagonal regression. This is the only production-source change in this
follow-up; the native research does not enable live particles or replace the
existing scene renderer.
