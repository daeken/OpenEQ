# WLD fragment boundaries

The WLD parser previously decoded each fragment against the entire remaining
file, then moved the cursor back to the declared fragment end. A fragment that
omitted a required field could therefore borrow that field from the following
fragment's header. For example, a SkeletonRef containing only its name could
read the next fragment's size as its skeleton reference and return success.

Every fragment now gets a reader bounded to its declared payload. Reference
resolution still uses global WLD tables, and unmodeled trailing bytes still
remain inside their own fragment. Child readers preserve absolute diagnostic
offsets. The common byte reader also compares requested length with remaining
bytes before addition, preventing an oversized request from overflowing on
32-bit hosts or through the new reader-window API.

Five regressions cover cross-fragment field/name reads, opaque tails, oversized
fragments/windows, unchanged cursor on failed window creation and nested absolute
failure offsets. The complete original-assets suite passed 282 tests before the
last two reader checks; both additional checks passed separately. Independent
review found the overflow edge and cleared its correction.

Independent before/after parsing compared all 1,804 WLD files in 3,243 readable
S3D/EQG archives. All 6,337,278 parsed fragment representations were identical,
with zero WLD failures or differences. Three existing archive-directory count
mismatches reproduce before WLD parsing on both versions: eye_chr.s3d,
greatdivide_chr.s3d and velketor_chr.s3d. They are not regressions introduced by
record bounding. A separate S3D-only probe agreed on the WLD/fragment totals.

Evidence: `/tmp/openeq-wld-boundary-review/results.log`,
`/tmp/openeq-wld-boundaries-assets.log`,
`/tmp/openeq-wld-boundaries-reader.log` and
`/tmp/openeq-wld-boundaries-survey/result.txt`.
