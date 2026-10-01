//! Malformed signed names must not alias unnamed fragments or overflow.
use openeq_assets::wld::{Fragment, Ref, WLD_MAGIC, Wld};

fn fixture(strings: &[u8], fragments: &[(i32, u32, &[u32])]) -> Wld {
    let mut data: Vec<_> = [
        WLD_MAGIC,
        0x15500,
        fragments.len() as u32,
        0,
        0,
        strings.len() as u32,
        0,
    ]
    .into_iter()
    .flat_map(u32::to_le_bytes)
    .collect();
    let key = [0x95, 0x3a, 0xc5, 0x2a, 0x95, 0x7a, 0x95, 0x6a];
    data.extend(
        strings
            .iter()
            .enumerate()
            .map(|(i, byte)| byte ^ key[i % 8]),
    );
    while !data.len().is_multiple_of(4) {
        data.push(0);
    }
    for &(name, kind, body) in fragments {
        data.extend(
            [4 + body.len() as u32 * 4, kind, name as u32]
                .into_iter()
                .chain(body.iter().copied())
                .flat_map(u32::to_le_bytes),
        );
    }
    Wld::parse("reference-fixture.wld".into(), &data).unwrap()
}

fn assert_unresolved(wld: &Wld, reference: Ref) {
    assert!(wld.resolve(reference).is_none(), "{reference:?}");
    assert!(wld.resolve_str(reference).is_none(), "{reference:?}");
    assert!(wld.reference_name(reference).is_none(), "{reference:?}");
}

#[test]
fn signed_extremes_never_alias_unnamed_fragments() {
    // A complete 40-byte WLD with no strings and one unnamed opaque fragment.
    let wld = fixture(&[], &[(0, 0x96, &[])]);
    for reference in [i32::MIN, -12345, -1, 0, 2, i32::MAX] {
        assert_unresolved(&wld, Ref(reference));
    }
    // Positive identity and direct unnamed lookup remain available.
    assert_eq!(wld.resolve(Ref(1)).unwrap().fragment.type_code(), 0x96);
    assert_eq!(wld.resolve_str(Ref(1)), Some(""));
    assert_eq!(wld.reference_name(Ref(1)), Some(""));
    assert!(std::ptr::eq(wld.by_name("").unwrap(), &wld.chunks()[0]));
    assert_eq!(Ref(i32::MIN).string_offset(), Some(0x8000_0000usize));
    assert_eq!(Ref(-1).string_offset(), Some(1));
    assert_eq!(Ref(0).string_offset(), None);
    assert_eq!(Ref(i32::MAX).string_offset(), None);
}

#[test]
fn named_substrings_and_duplicate_positive_identities_keep_existing_behavior() {
    let strings = b"\0PREFIX_TEX\0TEX\0INLINE\0\0";
    let wld = fixture(
        strings,
        &[
            (0, 0x96, &[]),
            (-1, 0x99, &[]),
            (-12, 0x98, &[]),
            (-12, 0x97, &[]),
        ],
    );
    assert!(std::ptr::eq(
        wld.resolve(Ref(-1)).unwrap(),
        &wld.chunks()[1]
    ));
    // Byte 8 starts TEX inside PREFIX_TEX; native-style substring names remain
    // accepted, and duplicate named resolution still selects the last record.
    for reference in [Ref(-8), Ref(-12)] {
        assert!(std::ptr::eq(
            wld.resolve(reference).unwrap(),
            &wld.chunks()[3]
        ));
        assert_eq!(wld.resolve_str(reference), Some("TEX"));
        assert_eq!(wld.reference_name(reference), Some("TEX"));
    }
    assert_eq!(wld.resolve(Ref(3)).unwrap().fragment.type_code(), 0x98);
    assert_eq!(wld.resolve(Ref(4)).unwrap().fragment.type_code(), 0x97);
    // An inline string does not need a matching fragment for the string APIs.
    assert!(wld.resolve(Ref(-16)).is_none());
    assert_eq!(wld.resolve_str(Ref(-16)), Some("INLINE"));
    assert_eq!(wld.reference_name(Ref(-16)), Some("INLINE"));
    for offset in [11, 15, 22, 23, strings.len(), strings.len() + 1, 12345] {
        assert_unresolved(&wld, Ref(-(offset as i32)));
    }
}

#[test]
fn minimum_signed_fragment_and_skeleton_names_keep_empty_fallback_without_panic() {
    let wld = fixture(
        b"\0ROOT\0",
        &[
            (i32::MIN, 0x99, &[]),
            (-12345, 0x98, &[]),
            (-1, 0x10, &[0, 1, 0, i32::MIN as u32, 0, 0, 0, 0]),
        ],
    );
    assert_eq!(wld.chunks()[0].name, "");
    assert_eq!(wld.chunks()[1].name, "");
    assert_eq!(wld.chunks()[2].name, "ROOT");
    let Fragment::Skeleton(skeleton) = &wld.chunks()[2].fragment else {
        panic!("skeleton")
    };
    assert_eq!(skeleton.tracks[0].name, "");
    assert_unresolved(&wld, Ref(i32::MIN));
    assert_unresolved(&wld, Ref(-12345));
}

#[test]
fn source_byte_offsets_survive_expansion_before_and_inside_names() {
    let strings = b"\0\xe9\0TEX\0\xc5_\xff_TEX\0TEX\0";
    let wld = fixture(
        strings,
        &[
            (-3, 0x99, &[]),
            (-7, 0x98, &[]),
            (-15, 0x97, &[]),
            (-7, 0x10, &[0, 1, 0, (-9i32) as u32, 0, 0, 0, 0]),
        ],
    );
    assert_eq!(wld.chunks()[0].name, "TEX");
    assert_eq!(wld.chunks()[1].name, "Å_ÿ_TEX");
    assert_eq!(wld.chunks()[2].name, "TEX");
    assert_eq!(wld.resolve_str(Ref(-1)), Some("é"));
    assert_eq!(wld.resolve_str(Ref(-7)), Some("Å_ÿ_TEX"));
    assert_eq!(wld.resolve_str(Ref(-8)), Some("_ÿ_TEX"));
    assert_eq!(wld.resolve_str(Ref(-9)), Some("ÿ_TEX"));
    let Fragment::Skeleton(skeleton) = &wld.chunks()[3].fragment else {
        panic!("skeleton")
    };
    assert_eq!(skeleton.tracks[0].name, "ÿ_TEX");
    // Full names and in-name substrings all retain duplicate-name semantics.
    for offset in [3, 11, 15] {
        let reference = Ref(-offset);
        assert_eq!(wld.reference_name(reference), Some("TEX"));
        assert!(std::ptr::eq(
            wld.resolve(reference).unwrap(),
            &wld.chunks()[2]
        ));
    }
    for offset in [2, 6, 14, 18, 19, 20, 21, 22, 1000] {
        assert_unresolved(&wld, Ref(-offset));
    }
}

#[test]
fn every_source_byte_uses_its_own_address_without_changing_display_conversion() {
    // Exercise every possible decoded byte, including the UTF-8 continuation
    // byte range. Expected text deliberately preserves the existing byte-to-
    // Unicode policy; native locale/code-page interpretation is separate.
    let mut strings: Vec<_> = (0..=255).collect();
    strings.extend_from_slice(b"\0AFTER\0");
    let fragments: Vec<_> = (1..strings.len())
        .map(|offset| (-(offset as i32), 0x99, &[][..]))
        .collect();
    let wld = fixture(&strings, &fragments);
    for offset in 1..strings.len() {
        let expected: String = strings[offset..]
            .iter()
            .copied()
            .take_while(|byte| *byte != 0)
            .map(char::from)
            .collect();
        assert_eq!(wld.chunks()[offset - 1].name, expected);
        let reference = Ref(-(offset as i32));
        if expected.is_empty() {
            assert_unresolved(&wld, reference);
        } else {
            assert_eq!(wld.resolve_str(reference), Some(expected.as_str()));
            assert_eq!(wld.reference_name(reference), Some(expected.as_str()));
            assert_eq!(wld.resolve(reference).unwrap().name, expected);
        }
    }
    for offset in strings.len()..=strings.len() + 128 {
        assert_unresolved(&wld, Ref(-(offset as i32)));
    }
}

#[test]
#[ignore = "requires original Plane of Knowledge and Citymist object archives"]
fn original_named_references_still_resolve_exact_actor_and_particle_definitions() {
    let base = openeq_assets::loader::default_client_dir().expect("original client assets");
    for (zone, names) in [
        ("poknowledge", &["FTORCH301_ACTORDEF", "L301_PCD"][..]),
        (
            "citymist",
            &["JNTREE103_ACTORDEF", "JNT3BR1_DMSPRITEDEF"][..],
        ),
    ] {
        let archive =
            openeq_assets::pfs::Archive::open(base.join(format!("{zone}_obj.s3d"))).unwrap();
        let filename = format!("{zone}_obj.wld");
        let bytes = archive.read(&filename).unwrap();
        let size = u32::from_le_bytes(bytes[20..24].try_into().unwrap()) as usize;
        let key = [0x95, 0x3a, 0xc5, 0x2a, 0x95, 0x7a, 0x95, 0x6a];
        let strings: Vec<_> = bytes[28..28 + size]
            .iter()
            .enumerate()
            .map(|(i, byte)| byte ^ key[i % 8])
            .collect();
        let wld = Wld::parse(filename, &bytes).unwrap();
        for &name in names {
            let nul_terminated = format!("{name}\0");
            let offset = strings
                .windows(nul_terminated.len())
                .position(|window| window == nul_terminated.as_bytes())
                .unwrap();
            assert!(offset > 0);
            let reference = Ref(-(offset as i32));
            assert!(std::ptr::eq(
                wld.resolve(reference).unwrap(),
                wld.by_name(name).unwrap()
            ));
            assert_eq!(wld.resolve_str(reference), Some(name));
            assert_eq!(wld.reference_name(reference), Some(name));
        }
        assert_unresolved(&wld, Ref(i32::MIN));
        // Source offsets cannot address the expanded display string's tail.
        for offset in strings.len()..=strings.len() * 2 {
            assert_unresolved(&wld, Ref(-(offset as i32)));
        }
    }
}
