//! Particle records retain file structure without inferring a playback model.
use openeq_assets::{
    Error,
    wld::{Fragment, ParticleCloud, Ref, WLD_MAGIC, Wld},
};

fn file(body: &[u8]) -> Vec<u8> {
    let mut data: Vec<_> = [WLD_MAGIC, 0x15500, 2, 0, 0, 0, 0]
        .into_iter()
        .flat_map(u32::to_le_bytes)
        .collect();
    data.extend(
        [body.len() as u32 + 4, 0x34, 0]
            .into_iter()
            .flat_map(u32::to_le_bytes),
    );
    data.extend(body);
    // A valid following record must not supply missing particle fields.
    data.extend([8u32, 0x11, 0, 1].into_iter().flat_map(u32::to_le_bytes));
    data
}

fn body(flags: u32, reference: i32) -> Vec<u8> {
    let mut data = Vec::new();
    // Distinct words include float NaN bits and packed colors, which must be
    // retained without numeric conversion or normalization by this parser.
    let words = std::array::from_fn::<_, 20, _>(|i| match i {
        0 => flags,
        5 => 0x7fc1_2345,
        19 => 0x3f64_6464,
        _ => 0x1234_0000 | i as u32,
    });
    data.extend(words.into_iter().flat_map(u32::to_le_bytes));
    if flags & 1 != 0 {
        data.extend(
            [0x8000_0000u32, 0x7fc0_1234, 1, 2, 3, 4]
                .into_iter()
                .flat_map(u32::to_le_bytes),
        );
    }
    if flags & 2 != 0 {
        data.extend(100..124);
    }
    if flags & 4 != 0 {
        data.extend(reference.to_le_bytes());
    }
    data
}

#[test]
fn particle_optional_fields_full_references_and_tail_are_lossless() {
    for flags in 0..8 {
        for reference in [470, -12345, i32::MIN] {
            let mut bytes = body(flags | 0x8000_0000, reference);
            bytes.extend([0xde, 0xad, 0x55]);
            let wld = Wld::parse("particle.wld".into(), &file(&bytes)).unwrap();
            let (_, cloud) = wld.iter::<ParticleCloud>().next().unwrap();
            assert_eq!(cloud.flags(), flags | 0x8000_0000);
            assert_eq!(cloud.fixed_words[5], 0x7fc1_2345);
            assert_eq!(cloud.fixed_words[9], 0x1234_0009);
            assert_eq!(cloud.fixed_words[19], 0x3f64_6464);
            assert_eq!(
                cloud.optional_vectors,
                (flags & 1 != 0).then_some([0x8000_0000, 0x7fc0_1234, 1, 2, 3, 4])
            );
            assert_eq!(
                cloud.optional_block,
                (flags & 2 != 0).then(|| std::array::from_fn(|i| 100 + i as u8))
            );
            assert_eq!(
                cloud.texture_reference,
                (flags & 4 != 0).then_some(Ref(reference))
            );
            assert_eq!(cloud.tail, [0xde, 0xad, 0x55]);
            assert_eq!(wld.chunks()[0].fragment.type_code(), 0x34);
            assert!(matches!(
                &wld.chunks()[1].fragment,
                Fragment::SkeletonRef(value) if value.skeleton == Ref(1)
            ));
        }
    }
}

#[test]
fn truncated_particle_fields_cannot_consume_neighboring_fragment_bytes() {
    for flags in 0..8 {
        let full = body(flags, 470);
        for length in 0..full.len() {
            assert!(
                matches!(
                    Wld::parse("short-particle.wld".into(), &file(&full[..length])),
                    Err(Error::Truncated { offset, .. }) if offset <= 40 + length
                ),
                "flags {flags}, body length {length}"
            );
        }
        assert!(Wld::parse("complete-particle.wld".into(), &file(&full)).is_ok());
    }
}

#[test]
#[ignore = "requires original Plane of Knowledge object archive"]
fn original_pok_particle_definitions_keep_duplicate_identity_and_full_texture_refs() {
    let base = openeq_assets::loader::default_client_dir().expect("original client assets");
    let archive = openeq_assets::pfs::Archive::open(base.join("poknowledge_obj.s3d")).unwrap();
    let wld = Wld::open(&archive, "poknowledge_obj.wld").unwrap();
    assert_eq!(wld.iter::<ParticleCloud>().count(), 8);
    for (index, earlier, name, texture) in [
        (5, 5, "CSMOKE_PCD", 4),
        (10, 10, "L301_PCD", 9),
        (15, 15, "L308_PCD", 14),
        (20, 20, "L500_PCD", 19),
        (406, 5, "CSMOKE_PCD", 405),
        (411, 10, "L301_PCD", 410),
        (438, 15, "L308_PCD", 437),
        (471, 10, "L301_PCD", 470),
    ] {
        let chunk = wld.resolve(Ref(index)).unwrap();
        assert_eq!(chunk.name, name);
        let Fragment::ParticleCloud(cloud) = &chunk.fragment else {
            panic!("particle fragment {index}")
        };
        let Fragment::ParticleCloud(first) = &wld.resolve(Ref(earlier)).unwrap().fragment else {
            panic!("earlier particle definition")
        };
        assert_eq!(cloud.fixed_words, first.fixed_words);
        assert_eq!(cloud.flags(), 4);
        assert_eq!(cloud.fixed_words[1..4], [3, 3, 0x30500]);
        assert_eq!(cloud.texture_reference, Some(Ref(texture)));
        assert!(cloud.optional_vectors.is_none());
        assert!(cloud.optional_block.is_none());
        assert!(cloud.tail.is_empty());
    }
}
