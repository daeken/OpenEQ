//! Particle records retain file structure without inferring a playback model.
use openeq_assets::{
    Error,
    wld::{Fragment, ParticleCloud, ParticleTexture, Ref, WLD_MAGIC, Wld},
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

#[test]
fn particle_texture_words_are_fixed_offset_lossless_and_record_bounded() {
    for flags in [0u32, 7, u32::MAX] {
        for reference in [470i32, -17, i32::MIN] {
            let mut body: Vec<_> = [flags, reference as u32, 0x8000_0017]
                .into_iter()
                .flat_map(u32::to_le_bytes)
                .collect();
            for length in 0..12 {
                let mut bytes = file(&body[..length]);
                bytes[32..36].copy_from_slice(&0x26u32.to_le_bytes());
                assert!(matches!(
                    Wld::parse("short-texture.wld".into(), &bytes),
                    Err(Error::Truncated { .. })
                ));
            }
            body.extend([0xaa, 0xbb, 0xcc]);
            let mut bytes = file(&body);
            bytes[32..36].copy_from_slice(&0x26u32.to_le_bytes());
            let wld = Wld::parse("texture.wld".into(), &bytes).unwrap();
            let (_, texture) = wld.iter::<ParticleTexture>().next().unwrap();
            assert_eq!(texture.flags, flags);
            assert_eq!(texture.texture, Ref(reference));
            assert_eq!(texture.material, 0x8000_0017);
            assert_eq!(texture.tail, [0xaa, 0xbb, 0xcc]);
            assert_eq!(wld.chunks()[0].fragment.type_code(), 0x26);
            assert!(
                matches!(&wld.chunks()[1].fragment, Fragment::SkeletonRef(s) if s.skeleton == Ref(1))
            );
        }
    }
}

#[test]
#[ignore = "requires original Plane of Knowledge particle texture chains"]
fn original_pok_particle_texture_chains_preserve_full_refs_and_raw_alias() {
    let base = openeq_assets::loader::default_client_dir().expect("original client assets");
    let archive = openeq_assets::pfs::Archive::open(base.join("poknowledge_obj.s3d")).unwrap();
    let wld = Wld::open(&archive, "poknowledge_obj.wld").unwrap();
    assert_eq!(wld.iter::<ParticleTexture>().count(), 8);
    for (cloud, texture_ref, child_ref, filename) in [
        (5, 4, 3, "CSMOKE.DDS"),
        (10, 9, 8, "GENG00.DDS"),
        (15, 14, 13, "GENG00.DDS"),
        (20, 19, 18, "GENG00.DDS"),
        (406, 405, 404, "CSMOKE.DDS"),
        (411, 410, 409, "GENG00.DDS"),
        (438, 437, 436, "GENG00.DDS"),
        (471, 470, 469, "GENG00.DDS"),
    ] {
        let Fragment::ParticleCloud(cloud) = &wld.resolve(Ref(cloud)).unwrap().fragment else {
            panic!("cloud")
        };
        assert_eq!(cloud.texture_reference, Some(Ref(texture_ref)));
        let Fragment::ParticleTexture(texture) = &wld.resolve(Ref(texture_ref)).unwrap().fragment
        else {
            panic!("texture")
        };
        assert_eq!(texture.flags, 0);
        assert_eq!(texture.texture, Ref(child_ref));
        assert_eq!(texture.material, 0x8000_0017);
        assert!(texture.tail.is_empty());
        let Fragment::AnimationRef(link) = &wld.resolve(texture.texture).unwrap().fragment else {
            panic!("link")
        };
        let Fragment::Animation(animation) = &wld.resolve(link.animation).unwrap().fragment else {
            panic!("animation")
        };
        assert_eq!(link.flags, 0);
        assert!(link.tail.is_empty());
        assert_eq!(animation.flags, 0x18);
        assert_eq!(animation.parameter, None);
        assert!(animation.tail.is_empty());
        assert_eq!(animation.frame_time, 100);
        assert_eq!(animation.textures.len(), 1);
        let Fragment::TextureList(bitmap) = &wld.resolve(animation.textures[0]).unwrap().fragment
        else {
            panic!("bitmap")
        };
        assert_eq!(bitmap.filenames, [filename]);
    }
}

#[test]
fn texture_animation_flags_optional_words_and_extensions_remain_lossless() {
    for flags in [0u32, 4, 8, 12, 0x8000_001c] {
        let mut body: Vec<_> = [flags, 2].into_iter().flat_map(u32::to_le_bytes).collect();
        if flags & 4 != 0 {
            body.extend(0x7fc0_1234u32.to_le_bytes());
        }
        if flags & 8 != 0 {
            body.extend(123u32.to_le_bytes());
        }
        body.extend([i32::MIN, 470].into_iter().flat_map(i32::to_le_bytes));
        for len in 0..body.len() {
            let mut bytes = file(&body[..len]);
            bytes[32..36].copy_from_slice(&4u32.to_le_bytes());
            assert!(Wld::parse("short-animation.wld".into(), &bytes).is_err());
        }
        body.extend([7, 8, 9]);
        let mut bytes = file(&body);
        bytes[32..36].copy_from_slice(&4u32.to_le_bytes());
        let wld = Wld::parse("animation.wld".into(), &bytes).unwrap();
        let Fragment::Animation(animation) = &wld.chunks()[0].fragment else {
            panic!("animation")
        };
        assert_eq!(animation.flags, flags);
        assert_eq!(animation.parameter, (flags & 4 != 0).then_some(0x7fc0_1234));
        assert_eq!(animation.frame_time, if flags & 8 != 0 { 123 } else { 0 });
        assert_eq!(animation.textures, [Ref(i32::MIN), Ref(470)]);
        assert_eq!(animation.tail, [7, 8, 9]);
    }
    let mut body: Vec<_> = [i32::MIN as u32, 0x8000_0011]
        .into_iter()
        .flat_map(u32::to_le_bytes)
        .collect();
    for len in 0..body.len() {
        let mut bytes = file(&body[..len]);
        bytes[32..36].copy_from_slice(&5u32.to_le_bytes());
        assert!(Wld::parse("short-animation-ref.wld".into(), &bytes).is_err());
    }
    body.extend([4, 5, 6]);
    let mut bytes = file(&body);
    bytes[32..36].copy_from_slice(&5u32.to_le_bytes());
    let wld = Wld::parse("animation-ref.wld".into(), &bytes).unwrap();
    let Fragment::AnimationRef(link) = &wld.chunks()[0].fragment else {
        panic!("link")
    };
    assert_eq!(link.animation, Ref(i32::MIN));
    assert_eq!(link.flags, 0x8000_0011);
    assert_eq!(link.tail, [4, 5, 6]);
}

#[test]
fn bitmap_source_padding_is_retained_without_entering_the_filename() {
    let key = [0x95u8, 0x3a, 0xc5, 0x2a, 0x95, 0x7a, 0x95, 0x6a];
    for suffix in [&[][..], &[0, 0, 0][..], &[0xde, 0xad, 0xbe, 0xef][..]] {
        let name = b"a.dds\0";
        let mut body = 0u32.to_le_bytes().to_vec();
        body.extend((name.len() as u16).to_le_bytes());
        body.extend(name.iter().enumerate().map(|(i, b)| b ^ key[i % key.len()]));
        body.extend(suffix);
        let mut bytes = file(&body);
        bytes[32..36].copy_from_slice(&3u32.to_le_bytes());
        let wld = Wld::parse("bitmap.wld".into(), &bytes).unwrap();
        let Fragment::TextureList(bitmap) = &wld.chunks()[0].fragment else {
            panic!("bitmap")
        };
        assert_eq!(bitmap.filenames, ["a.dds"]);
        assert_eq!(bitmap.tail, suffix);
        assert_eq!(wld.chunks()[1].fragment.type_code(), 0x11);
    }
}
