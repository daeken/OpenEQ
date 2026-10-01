# Preserve secondary TER coordinates through mesh packing

October 1, 2026. The asset bake now retains the second raw coordinate pair for
exact `Opaque_MaxCB1_2UV.fx` and `Opaque_MaxCBSG1_2UV.fx` terrain materials.
This is a source-preservation prerequisite for the native routes documented in
[EQG_TER_SECONDARY_UV_SHADERS.md](EQG_TER_SECONDARY_UV_SHADERS.md); it does not
enable those shaders.

`Scene.secondary_ter_uv` is keyed by baked mesh index. Each
`PackedTerSecondaryUv` contains one raw `tex_coords` pair and one representative
original `source_indices` entry per baked vertex. Geometry deduplication compares
position, normal, primary coordinates **and secondary coordinate bits**. Vertices
whose primary attributes match but whose secondary coordinates differ remain
separate. Identical complete attributes still merge in first-seen order.

The gate requires a TER object, version 1–3, one of those two exact case-sensitive
shader names, and a present secondary stream whose length equals the source
vertex count. It does not invent a stream for absent or unsupported input. The
current parser supplies version 3 interleaved pairs and version 2 recognized
trailing streams; version 1 currently supplies none. MOD, other families and
other versions retain existing packing.

No arithmetic is applied to either coordinate stream. NaN payloads, infinities
and signed zero remain source words; native SHORT2 conversion is a later upload
concern. The existing renderer still receives the same eight primary float
attributes per triangle corner and `UvEncoding::Float32` for these families.
Secondary coordinates are metadata only. Texture selection, material transparency,
lighting admission and the separate physical collision bake remain unchanged.
The finer deduplication can increase vertex counts and memory use.

The representative source index certifies only the retained geometry and UV
attributes. It must not later stand in for unretained lighting, source colors,
or tangent channels without adding their bits to vertex identity. The existing
ordinary-CB1 lighting sidecar is independently gated and does not overlap these
two families. Future shader work still needs appropriate second-texture metadata,
lighting/tangent provenance, technique/state selection and native upload handling.

Like the lighting sidecar, secondary-UV keys must be cleared or remapped whenever
mesh indices change. Existing diagnostic mesh filters clear the new map; the
render comparison fixture takes/restores it while swapping packing. Scene
constructors and independent object extraction begin with an empty map.

Three focused synthetic tests verify exact admission, distinct NaN/signed-zero
identity, stable first-seen remapping, equivalence to old packing when UV1 is
identical, and collision separation. The ignored original-Nest test independently
walks raw material records and both coordinate streams, then compares every
triangle-corner tuple and representative source vertex against the bake. All
**305,522 triangles** in the two families match exactly. Original assets remain
outside the repository.

```sh
CARGO_INCREMENTAL=0 cargo test -p openeq-assets --lib ter_secondary_uv
CARGO_INCREMENTAL=0 cargo test -p openeq-assets --test ter_secondary_uv_bake -- --ignored
```

An independent raw-byte survey inspected all 228 installed TER payloads and
compared the 15 admitted payloads through production TER-only loading. It checked
all primary triangle-corner attributes across 2,869,912 terrain triangles,
including other material families. The retained channel covers 727 material
batches, 2,167,611 dual-UV triangles and 2,325,211 packed vertices. Of those vertices,
20,831 remain distinct specifically because their secondary-coordinate bits
differ; the previous primary-only packing merged them. All original referenced
secondary components in this admitted corpus are finite. Nonfinite retention is
covered by the synthetic bit-pattern tests rather than claimed for this corpus.
The survey does not claim complete original GPU, lighting or whole-zone behavior.

| Independent local survey | SHA-256 |
| --- | --- |
| `/tmp/openeq-secondary-loader-survey.rs` | `3724f2444fda775d1e802c0f9e76c21727c0c3e90ad193fd5a0638deb6ec965e` |
| `/tmp/openeq-secondary-loader-survey.json` | `f121b8f95c5d0983db90fe19b51dbf61fbb642fc23ab6e48e55d297e43c0b597` |
| `/tmp/openeq-secondary-loader-survey.log` | `440d397547945c4af50cf3ca77fbcbfa224e7184527c93945c25d6cc16cb1bf4` |


A separate raw numeric pass checks both primary and secondary coordinates for
3,065,589 unique referenced source vertices across the 15 admitted payloads.
All components are finite and remain inside the signed-32 conversion range
after multiplication by 256. This bounds future native SHORT2 conversion for
this installed corpus; the current preservation change performs no conversion.
The numeric probe/output hashes are
`f29c3fd67aff00a4b2170ffbb6f184836e2b8731b1ed61f5f3708cbaa2a858d9` /
`fa48632edf3cb4d0c0b7ed1faf4a16ada2d457671a5dfe27f6426a269ee23b4c`,
files `/tmp/openeq-secondary-numeric-corpus.{py,json}`.

The fixed-camera original Bazaar, Nest and Thundercrest GPU audit also passes.
Restoring former primary-only packing produces pixel-identical full-scene
images under the same camera and time, confirming these extra source identities
do not alter current shading. Captures: `/tmp/openeq-secondary-gpu/`; log:
`/tmp/openeq-secondary-gpu.log`. This is a same-renderer comparison, not an
original-client framebuffer comparison or performance equivalence claim.


Integrated verification passes 1,177 workspace tests with zero failures or
ignored tests, including original assets, GPU and digitally silent audio.
Strict workspace lint, client/audit builds, all-target no-default, formatting
and diff checks pass. Independent implementation review found no blockers.
The 523-zone CPU survey matches every previous non-timing field; the existing
21 nonfinite cases and Dranikcatacombsa dependency remain unchanged. Logs use
`/tmp/openeq-secondary-*`; survey `/tmp/openeq-secondary-zone-survey/`.
