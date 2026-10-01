# WLD signed-reference follow-up

Discovered during the 2026-10-01 review of partial particle-linked static actors
and repaired as a separate change after that checkpoint froze. The particle
metadata gate retains its proven positive-fragment-reference scope. This note
preserves the original reproduction and the focused regression evidence.

The repair uses unsigned magnitude conversion for signed references and checks
negative string references before named lookup. Invalid or empty negative names
now return `None` from `resolve`, `resolve_str` and `reference_name`. Valid
positive references, including unnamed fragments, remain unchanged. Nonempty
substring names and last-definition duplicate-name lookup also remain unchanged.
Fragment-header and skeleton names retain their existing empty-name fallback,
with overflow-safe arithmetic. No string-table encoding policy changed.

## Confirmed behavior

Before the repair, `Ref::string_offset` in `crates/openeq-assets/src/wld.rs` negated the signed
reference directly. `Ref(i32::MIN)` therefore panics when overflow checks are
enabled. `Wld::resolve` then passes the offset to `string_at`, which substitutes
an empty string for an out-of-range offset. Looking up that empty name can
return a real unnamed fragment instead of rejecting the malformed reference.

The following complete 40-byte WLD is enough to demonstrate both problems. It
contains no string table and only one unnamed opaque fragment, kind `0x26`.
It requires neither original EverQuest assets nor a renderer, server or device.

```rust
use openeq_assets::wld::{Ref, Wld, WLD_MAGIC};

fn fixture() -> Wld {
    let bytes: Vec<u8> = [
        WLD_MAGIC, 0x15500, 1, 0, 0, 0, 0, // header, zero string bytes
        4, 0x26, 0,                       // size, kind, unnamed fragment
    ]
    .into_iter()
    .flat_map(u32::to_le_bytes)
    .collect();
    Wld::parse("reference-check.wld".into(), &bytes).unwrap()
}

fn main() {
    let wld = fixture();
    println!(
        "out-of-range negative resolves to: {:?}",
        wld.resolve(Ref(-12345)).map(|c| c.fragment.type_code())
    );
    let min = std::panic::catch_unwind(|| wld.resolve(Ref(i32::MIN)));
    println!("minimum signed reference panics: {}", min.is_err());
}
```

Executed against the debug assets library before the repair, the output was:

```text
out-of-range negative resolves to: Some(38)
minimum signed reference panics: true
```

Decimal 38 is fragment kind `0x26`. The panic was caught, not an out-of-process
crash in this probe. Temporary executable/source artifacts are
`/tmp/openeq-particle-ref-review` and `/tmp/openeq-particle-ref-review.rs`.
The latter suppresses the panic hook's diagnostic so only the two result lines
are printed. The generic problem does not require a `0x34` particle record.

## Original affected paths

- `Ref::string_offset`: direct `-self.0` can overflow for `i32::MIN`.
- `Wld::resolve`: negative references use `by_name(string_at(...))`, so an
  invalid offset aliases the empty-name entry. The name index stores unnamed
  fragments too; the last inserted empty name wins.
- `Wld::resolve_str`: wraps `string_at` in `Some`, so an invalid negative
  offset returns `Some("")` rather than `None`.
- `Wld::reference_name`: delegates its negative-reference case to
  `resolve_str`, inheriting the same behavior.
- `Wld::parse`'s fragment-name decoding and `strings_at` also directly negate
  signed values. Those are related input-validation sites to cover in the
  independent repair; only the resolver reproduction above was executed here.

With overflow checks disabled, direct negation alone would not repair the
invalid-name alias. Both checked magnitude handling and strict offset
validation are needed. This note does not claim a release-mode execution test.

## Focused regression tests for the independent fix

A portable test using `fixture()` above requires all malformed and null
references to remain unresolved without panicking, while retaining valid
positive references:

```rust
#[test]
fn malformed_names_do_not_resolve_to_unnamed_fragments() {
    let wld = fixture();
    for reference in [Ref(-12345), Ref(i32::MIN), Ref(0), Ref(2)] {
        assert!(wld.resolve(reference).is_none(), "{reference:?}");
        assert!(wld.resolve_str(reference).is_none(), "{reference:?}");
        assert!(wld.reference_name(reference).is_none(), "{reference:?}");
    }
    assert_eq!(wld.resolve(Ref(1)).unwrap().fragment.type_code(), 0x26);
}
```

The separate integration file
[`wld_references.rs`](../crates/openeq-assets/tests/wld_references.rs) now covers
these assertions in the normal debug profile, so unchecked signed arithmetic
cannot be hidden by wrapping. Its other portable fixtures cover a nonempty
encoded string table, an in-range negative name, a nonempty substring name,
duplicate fragment identity, inline strings, empty negative names, offsets at
and beyond the table length, and minimum signed fragment/skeleton names.
Positive references to unnamed fragments still return their valid empty names.

The ignored original-file fixture passed for named actor, mesh and duplicate
particle references in the Plane of Knowledge and Citymist object archives.
The three portable tests and this original fixture passed together with the
three fragment-boundary tests and three particle-parser tests, including the
original PoK definitions: **10 tests passed**. Original files were read only.
This repair does not change duplicate-name resolution or the native particle
reader's observed low-byte texture lookup.
