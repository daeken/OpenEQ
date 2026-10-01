//! Source channels stay distinct until native material lighting is implemented.
use openeq_assets::{
    loader,
    pfs::Archive,
    zone::{TerMod, VertexLighting, ZoneFile},
};

fn words(values: &[u32]) -> Vec<u8> {
    values.iter().flat_map(|v| v.to_le_bytes()).collect()
}

#[test]
fn lit_words_are_lossless_bounded_and_keep_extensions() {
    let mut bytes = b"EQGP".to_vec();
    bytes.extend(words(&[3, 0x001f1f1f, 0x4c0d0901, 0xe5000000]));
    for end in 0..bytes.len() {
        assert!(
            VertexLighting::parse(&bytes[..end]).is_err(),
            "prefix {end}"
        );
    }
    bytes.extend([0xde, 0xad, 0xbe]);
    let lighting = VertexLighting::parse(&bytes).unwrap();
    assert_eq!(lighting.colors, [0x001f1f1f, 0x4c0d0901, 0xe5000000]);
    assert_eq!(lighting.tail, [0xde, 0xad, 0xbe]);
    bytes[4..8].copy_from_slice(&u32::MAX.to_le_bytes());
    assert!(VertexLighting::parse(&bytes).is_err());
    assert!(
        VertexLighting::parse(b"EQGP\0\0\0\0")
            .unwrap()
            .colors
            .is_empty()
    );
    assert!(VertexLighting::parse(b"EQGT\0\0\0\0").is_err());
}

#[test]
fn source_color_and_secondary_uv_preserve_bits_separately_from_lighting() {
    for terrain in [false, true] {
        for version in [1, 2, 3] {
            let mut bytes = if terrain { b"EQGT" } else { b"EQGM" }.to_vec();
            bytes.extend(words(&[version, 0, 0, 1, 0]));
            if !terrain {
                bytes.extend(words(&[0]));
            }
            bytes.extend(words(&[0, 0, 0, 0, 0, 0x3f800000]));
            if version == 3 {
                bytes.extend(words(&[0xa1b2c3d4]));
            }
            bytes.extend(words(&[0x80000000, 0x7fc01234]));
            if version == 3 {
                bytes.extend(words(&[0xff800000, 0x7f801234]));
            }
            for end in 0..bytes.len() {
                assert!(TerMod::parse(&bytes[..end], terrain).is_err());
            }
            let parsed = TerMod::parse(&bytes, terrain).unwrap();
            assert_eq!(
                parsed.tex_coords[0].map(f32::to_bits),
                [0x80000000, 0x7fc01234]
            );
            if version == 3 {
                assert_eq!(parsed.vertex_colors.as_deref(), Some(&[0xa1b2c3d4][..]));
                assert_eq!(
                    parsed.secondary_tex_coords.unwrap()[0].map(f32::to_bits),
                    [0xff800000, 0x7f801234]
                );
            } else {
                assert!(parsed.vertex_colors.is_none());
                assert!(parsed.secondary_tex_coords.is_none());
            }
        }
    }
}

#[test]
fn embedded_lighting_preserves_placement_alignment_and_empty_vs_absent() {
    for version in [1, 2] {
        let mut bytes = b"EQGZ".to_vec();
        bytes.extend(words(&[version, 1, 0, 2, 0, 0]));
        bytes.push(0);
        for i in 0..2 {
            bytes.extend(words(&[u32::MAX, 0, i, 0, 0, 0, 0, 0, 0x3f800000]));
            if version == 2 {
                if i == 0 {
                    bytes.extend(words(&[2, 0x11223344, 0xaabbccdd]));
                } else {
                    bytes.extend(words(&[0]));
                }
            }
        }
        for end in 0..bytes.len() {
            assert!(ZoneFile::parse(&bytes[..end], |_| panic!("no objects")).is_err());
        }
        let zone = ZoneFile::parse(&bytes, |_| panic!("no objects")).unwrap();
        assert_eq!(zone.placeables[1].position[0].to_bits(), 1);
        if version == 2 {
            assert_eq!(
                zone.placeables[0].vertex_lighting.as_deref(),
                Some(&[0x11223344, 0xaabbccdd][..])
            );
            assert_eq!(zone.placeables[1].vertex_lighting.as_deref(), Some(&[][..]));
        } else {
            assert!(zone.placeables.iter().all(|p| p.vertex_lighting.is_none()));
        }
    }
}

#[test]
#[ignore = "requires original EverQuest assets"]
fn original_lighting_sources_match_independent_raw_streams() {
    let base = loader::default_client_dir().unwrap();
    let archive = Archive::open(base.join("causeway.eqg")).unwrap();
    let raw = archive.read("ter_gorge.lit").unwrap();
    let lit = VertexLighting::parse(&raw).unwrap();
    assert_eq!(lit.colors.len(), 111269);
    assert!(lit.tail.is_empty());
    for (index, color) in [
        (87375, 0x001b1202),
        (87468, 0x4c0d0901),
        (96771, 0),
        (103109, 0xe5000000),
    ] {
        assert_eq!(lit.colors[index], color);
    }
    for (zone, offset, count, index, stored, lighting) in [
        ("guildhall", 0xc4b, 28584, 16302, 0xff232727, 0x32a6a6a6),
        ("guildlobby", 0x241c, 57912, 14, 0xff808080, 0x00686456),
        ("roost", 0xa536, 73847, 72880, 0xff808080, 0x00020202),
    ] {
        let raw = std::fs::read(base.join(format!("{zone}.zon"))).unwrap();
        let archive = Archive::open(base.join(format!("{zone}.eqg"))).unwrap();
        let parsed = ZoneFile::parse(&raw, |name| archive.read(name)).unwrap();
        let colors = parsed.placeables[0].vertex_lighting.as_ref().unwrap();
        assert_eq!(colors.len(), count);
        let independent: Vec<_> = raw[offset..offset + 4 * count]
            .chunks_exact(4)
            .map(|b| u32::from_le_bytes(b.try_into().unwrap()))
            .collect();
        assert_eq!(colors, &independent, "{zone}");
        let terrain = parsed
            .objects
            .iter()
            .find(|object| object.is_terrain)
            .unwrap();
        assert_eq!(terrain.positions.len(), count);
        assert_eq!(terrain.vertex_colors.as_ref().unwrap()[index], stored);
        assert_eq!(colors[index], lighting);
        assert_eq!(terrain.secondary_tex_coords.as_ref().unwrap().len(), count);
    }
}
