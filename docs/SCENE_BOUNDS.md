# Bounds of the rendered scene

The GPU upload previously accumulated every source vertex in definition space.
It ignored object placements, included unused model definitions and unused
vertices, and could therefore report the wrong center or dimensions. The offline
client and `renderzone` use these bounds for their initial camera position;
dynamic door loading also reads the extracted model dimensions.

Startup bounds now follow the same mesh ownership, first-name object lookup and
instance matrices as the submitted draw calls. A mesh contributes only through
complete indexed triangles with finite positions. Each local mesh box is
transformed by the actual draw instances before being merged into world bounds.
Direct terrain contributes once at identity. Unplaced definitions, unresolved
placements, unreferenced vertices and physical-only geometry contribute nothing.
Empty visible scenes produce zero bounds rather than sentinel extrema.

This computes conservative transformed mesh boxes. It does not claim the
smallest possible box for a rotated irregular mesh, account for transparent
texels, or change geometry/indices/materials/collision. Authored nonfinite mesh
values remain intact and are still reported by the asset audit. Dynamic pose and
instance updates do not refresh these startup bounds; actor pose bounds and
runtime culling are separate concerns. Correct center calculation also does not
prove that a zone's geometric center is a safe walkable spawn location.

Verification includes portable cases for translation, nonuniform/reflected
scale, rotation, multiple instances, finite-triangle filtering and empty bounds.
A GPU fixture retains large unplaced and duplicate-name model definitions,
unresolved placements, unused vertices and hidden collision while checking the
bounds against known submitted-world coordinates. Direct geometry and removal
of all placements are checked separately. The original City of Mist GPU test
independently expands actual vertices from all 50 tree placements and verifies
that every one lies within the reported bounds. Existing invisible-collision
pixel/upload/bounds comparisons continue to pass.

Focused logs: `/tmp/openeq-world-extents-{unit,gpu}.log`.
