# Placed WLD animation CPU measurements

Measured on October 1, 2026, on an Apple M4. A full CPU pose of the original
North Qeynos Temple of Life costs about **12.4 µs**; the original Swamp of No Hope
lamp costs about **2.7 µs**. The renderer samples each placed object definition
once per update, so all **36 lamp placements share that one sample**. These
measurements do not justify changing animation ownership late in the current
implementation. No production code changed for this investigation.

## Method and limits

The CPU-only harness loads the original zones and samples the actual retained
`ObjectSource` values. It builds a frozen copy of `openeq-assets` directly with
Rust 1.97.1, `opt-level=3`, and `codegen-units=1`, linking existing dependency
artifacts. The harness uses the same options. It does not use Cargo, start a GPU,
touch a live character, or play audio. This is not the exact release profile:
release thin LTO is absent.

There are five runs per subject. Each measurement warms for four batches and
then records 41 batches. Full poses, validation, authored-frame poses and key
preparation use 256 calls per batch: **52,480 measured calls** across five runs.
Prebuilt-key interpolation uses 100,000 calls per batch. Every timed call uses
`black_box`; animation phases advance by 137 ms modulo the complete period.
Returned mesh allocations are consumed and dropped inside the timed interval.
Zone loading is outside the interval.

Numbers below are medians and p95 of **205 batch means**, not individual-call
latency percentiles. The p95 selects sorted zero-based index
`floor(0.95 * (N - 1))`. Root independently recomputed all ten median/p95
pairs and checked every artifact, archive and dependency hash. CPU scheduling, frequency and other system activity are not
controlled. The harness excludes the renderer's subsequent vertex-template
copies, queue writes and GPU execution. Individual measurements use separate
calls and are not an additive profiler breakdown. The authored-frame path is
only a transform/allocation diagnostic: it does not implement reduced-key time
sampling and must not be substituted for a cached animation implementation.

## Results

| CPU operation | Temple median / p95, µs | Lamp median / p95, µs |
| --- | ---: | ---: |
| Full `sample_animation`, including destruction | 12.379 / 12.746 | 2.728 / 2.849 |
| `animation_period`: validation and key certification | 5.130 / 5.342 | 0.667 / 0.683 |
| Authored-frame pose and mesh transformation, diagnostic only | 2.311 / 2.376 | 1.493 / 1.516 |
| One animated track's key certification/reduction | 4.846 / 5.042 | 0.510 / 0.523 |
| Interpolation using already prepared rotation keys | 0.0050 / 0.0051 | 0.0039 / 0.0040 |

The five full-pose run medians range from 11.936–12.718 µs for the Temple and
2.690–2.771 µs for the lamp. The standalone interpolation result only measures
quaternion-component interpolation, not normalization, hierarchy composition or
mesh transformation.

| Source property | `qeynos2` / `templelife` | `swampofnohope` / `krlamp101` |
| --- | ---: | ---: |
| Original placements | 1 | 36 |
| Placed animation controllers in one original scene | 1 | 1 |
| Source parts / vertices / triangles | 1 / 412 / 372 | 2 / 220 / 185 |
| Baked meshes / vertices | 6 / 412 | 3 / 220 |
| Track frame counts | 1, 40 | 1, 1, 16 |
| Animated interval / period | 200 / 8000 ms | 100 / 1600 ms |
| Omitted rotation keys | 16, 22, 2, 7, 31 | 1, 8 |

An additional diagnostic using the existing optimized-development assets
artifact (`opt-level=1`) measured 30.669 µs for the Temple and 3.667 µs for the
lamp. That single run had more scheduling variation and is retained in the
artifacts, rather than mixed into the fully optimized results.

## Sharing and possible follow-up

`GpuScene::build` prepares one `PlacedAnimation` per object definition with
instances. Its update loop calls each controller once. The loader deduplicates
actor names and retains one `Arc<ObjectSource>` per definition; extracting an
object model clones that `Arc`. Separate GPU scenes can nevertheless create
separate controllers, so these measurements apply per controller update, not
globally across every scene using the same source.

`ObjectSource::sample_animation` currently calls `animation_timing`, which
validates all frames, builds the hierarchy and certifies/reduces the animation
keys. Sampling then constructs the same key reducers again. `pose_with` also
rebuilds the hierarchy. All of that preparation depends on source data, not the
current phase.

A concrete future optimization is an immutable prepared sampler containing
validated timing, hierarchy, retained rotation/translation keys and mesh
bindings. Construct it once when preparing a controller, then interpolate and
transform using its immutable source snapshot. Shared prepared source data may
also avoid repeating admission work when multiple scenes reference the same
source; pose output still depends on each update's time.

The ownership boundary matters: `ObjectSource` and its skeleton, tracks and
meshes are publicly mutable and clonable. An unqualified cache inside that
mutable structure could become stale after edits or cloning. A controller-owned
prepared snapshot, retaining its source `Arc`, gives a clearer contract: changes
made through another `Arc::make_mut` create a different source and require new
controller preparation. Arbitrary callers of the public one-shot sampler should
retain validation unless an explicit prepared API gives equivalent guarantees.
Cross-source cache identity would need exact frames, timing, reference flags,
hierarchy, bindings and reduction policy, not just actor name or frame count.
Hashing all source data every frame would undermine the optimization.

Even after key preparation is cached, `transform_meshes` clones mesh arrays,
validates attributes, constructs per-part bindings and transforms vertices and
normals. The renderer then builds a flattened vertex-reference vector and clones
its baked vertex templates before uploading. Reusable scratch buffers are a
separate allocation follow-up, with their own ownership and invalidation rules.
None of these possible savings is claimed as a measured frame-time improvement.

## Frozen local evidence

The harness, source snapshot, build arguments, libraries, logs and raw batch
results are in `/tmp/openeq-placed-animation-perf/`. Files are read-only, and
executables are mode 555. Original client assets remain outside the repository.
`manifest.json` includes hashes of every local artifact, source snapshot, direct
dependency and the four relevant zone/object archives.

| Artifact | SHA-256 |
| --- | --- |
| `manifest.json` | `e4b66d1bcedede9b4d57c8c3cfbc279087d6c27e491badbd02f889bdbd92841c` |
| `summary.json` | `af1109e7199b618eb79a0c41e51807b9d535325403c58c820ee5a5322de87c52` |
| `bench.rs` | `52a4e98cd11d37d54fefa026ef1194ff65d673101a7c0d8199494585d29df4bb` |
| `libopeneq_assets_opt3.rlib` | `5aca4ca5d7406d54866760a4e8f166de5e5a151ad95e5954561062d64e4f25bc` |
| Snapshot `loader/wld_objects.rs` | `8c4d6025f892b104bdeb7b9219cdb986a2ca64f3a866c2eaf86700a4f175d00b` |
| Snapshot `loader/wld_object_positive_reduction.rs` | `cd3af8d270e16c89291087e17b53977bf32185325819ab50da27faac3e505f07` |
| Snapshot `loader/wld_object_zero_reduction.rs` | `8d3fe13ea977d329ff6bda43cb3d169e7f927571827b8ac2854e1f92aa859242` |

See `build.json` for the exact argument arrays and `opt3-run1.json` through
`opt3-run5.json` for raw batches. The two key-reduction helpers are compiled
unchanged from the same snapshot as the measured library.
