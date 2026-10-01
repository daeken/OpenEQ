//! Fragment extents are record boundaries even when later file bytes exist.
use openeq_assets::{
    Error,
    wld::{Fragment, WLD_MAGIC, Wld},
};

fn file(words: &[u32]) -> Vec<u8> {
    [WLD_MAGIC, 0x15500, 2, 0, 0, 0, 0]
        .into_iter()
        .chain(words.iter().copied())
        .flat_map(u32::to_le_bytes)
        .collect()
}

#[test]
fn short_fragment_cannot_borrow_its_missing_field_from_the_next_header() {
    let data = file(&[4, 0x11, 0, 8, 0x11, 0, 1]);
    assert!(matches!(
        Wld::parse("short.wld".into(), &data),
        Err(Error::Truncated {
            offset: 40,
            needed: 4,
            available: 0
        })
    ));
    // The fragment's name is part of the same declared extent.
    let data = file(&[0, 0x99, 8, 0x11, 0, 1]);
    assert!(matches!(
        Wld::parse("empty.wld".into(), &data),
        Err(Error::Truncated {
            offset: 36,
            needed: 4,
            available: 0
        })
    ));
}

#[test]
fn unmodeled_record_tails_stay_inside_their_declared_fragment() {
    let data = file(&[12, 0x11, 0, 2, 0xdeadbeef, 8, 0x11, 0, 1]);
    let wld = Wld::parse("tails.wld".into(), &data).unwrap();
    for (chunk, reference) in wld.chunks().iter().zip([2, 1]) {
        let Fragment::SkeletonRef(value) = &chunk.fragment else {
            panic!("wrong fragment")
        };
        assert_eq!(value.skeleton.0, reference);
    }
}

#[test]
fn oversized_fragment_rejects_before_reading_or_allocating_its_payload() {
    let data = file(&[u32::MAX, 0x99, 0]);
    assert!(matches!(Wld::parse("oversized.wld".into(), &data),
        Err(Error::Truncated {offset:36, needed, available:4}) if needed==u32::MAX as usize));
}
