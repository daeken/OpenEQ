//! TER v2 trailing channels: native source layout, with checked parser bounds.
use openeq_assets::{
    Error, loader,
    pfs::Archive,
    zone::{MOD_MAGIC, TER_MAGIC, TerMod},
};

fn words(values: &[u32]) -> Vec<u8> {
    values.iter().flat_map(|word| word.to_le_bytes()).collect()
}

const SECONDARY: [u32; 6] = [
    0x7fff_0005, // Actual Thundercrest quiet-NaN payload.
    0x7f80_0001, // Raw source retention also preserves signaling-NaN words.
    0x8000_0000,
    0x7f80_0000,
    0xff80_0000,
    1,
];

fn fixture(terrain: bool, version: u32, count: u32) -> Vec<u8> {
    let strings = b"mat\0Opaque_MaxCB1_2UV.fx\0property\0";
    let mut bytes = words(&[
        if terrain { TER_MAGIC } else { MOD_MAGIC },
        version,
        strings.len() as u32,
        1,
        count,
        u32::from(count != 0),
    ]);
    if !terrain {
        bytes.extend(words(&[0]));
    }
    bytes.extend(strings);
    bytes.extend(words(&[73, 0, 4, 1, 24, 1, 0x1234_5678]));
    for i in 0..count {
        bytes.extend(words(&[i, 0, 0, 0, 0, 0x3f80_0000]));
        if version == 3 {
            bytes.extend(words(&[0xa1b2_c3d4]));
        }
        bytes.extend(words(&[0x3f00_0000 + i, 0x8000_0000]));
        if version == 3 {
            bytes.extend(words(&SECONDARY[i as usize * 2..i as usize * 2 + 2]));
        }
    }
    if count != 0 {
        bytes.extend(words(&[0, 2, 1, 0, 0x1234]));
    }
    bytes
}

#[test]
fn v2_tags_one_and_two_preserve_original_words_and_vertex_order() {
    for tag in [1, 2] {
        let mut bytes = fixture(true, 2, 3);
        bytes.extend(words(&[tag]));
        bytes.extend(words(&SECONDARY));
        // The selected stream has a fixed size; a suffix is not a second stream.
        bytes.extend([0xde, 0xad, 0xbe]);
        let parsed = TerMod::parse(&bytes, true).unwrap();
        let secondary: Vec<_> = parsed
            .secondary_tex_coords
            .unwrap()
            .into_iter()
            .flat_map(|pair| pair.map(f32::to_bits))
            .collect();
        assert_eq!(secondary, SECONDARY);
        assert_eq!(parsed.materials[0].stored_id, 73);
        assert_eq!(parsed.polygons, [(0, 2, 1, 0, 0x1234)]);
        assert!(parsed.vertex_colors.is_none());
        for (i, pair) in parsed.tex_coords.iter().enumerate() {
            assert_eq!(
                pair.map(f32::to_bits),
                [0x3f00_0000 + i as u32, 0x8000_0000]
            );
        }
    }
}

#[test]
fn absent_and_unrecognized_v2_tails_do_not_invent_secondary_coordinates() {
    let base = fixture(true, 2, 3);
    // Deliberate compatibility for existing synthetic inputs, not a native
    // bounded read behavior: the original loader blindly reads a tag here.
    assert!(
        TerMod::parse(&base, true)
            .unwrap()
            .secondary_tex_coords
            .is_none()
    );
    for tag in [0, 3, u32::MAX] {
        let mut bytes = base.clone();
        bytes.extend(words(&[tag]));
        bytes.extend([1, 2, 3]);
        assert!(
            TerMod::parse(&bytes, true)
                .unwrap()
                .secondary_tex_coords
                .is_none()
        );
    }
    for tag in [1, 2] {
        let mut bytes = fixture(true, 2, 0);
        bytes.extend(words(&[tag]));
        assert_eq!(
            TerMod::parse(&bytes, true).unwrap().secondary_tex_coords,
            Some(vec![])
        );
    }
}

#[test]
fn every_partial_tag_or_declared_v2_coordinate_stream_fails_safely() {
    for tag in [1, 2] {
        let base = fixture(true, 2, 3);
        let mut complete = base.clone();
        complete.extend(words(&[tag]));
        complete.extend(words(&SECONDARY));
        for end in base.len() + 1..complete.len() {
            let error = TerMod::parse(&complete[..end], true).unwrap_err();
            let expected_offset = base.len() + if end < base.len() + 4 { 0 } else { 4 };
            assert!(
                matches!(error, Error::Truncated { offset, .. } if offset == expected_offset),
                "tag {tag}, prefix {end}: {error:?}"
            );
        }
    }
}

#[test]
fn v1_v3_and_mod_keep_their_existing_source_layouts() {
    for (terrain, version) in [(true, 1), (true, 3), (false, 1), (false, 2), (false, 3)] {
        let mut bytes = fixture(terrain, version, 3);
        // Even a partial apparent tag must not invoke the TER-v2 extension.
        bytes.push(1);
        let parsed = TerMod::parse(&bytes, terrain).unwrap();
        if version == 3 {
            let secondary: Vec<_> = parsed
                .secondary_tex_coords
                .unwrap()
                .into_iter()
                .flat_map(|pair| pair.map(f32::to_bits))
                .collect();
            assert_eq!(secondary, SECONDARY);
            assert_eq!(parsed.vertex_colors.unwrap(), [0xa1b2_c3d4; 3]);
        } else {
            assert!(parsed.secondary_tex_coords.is_none());
            assert!(parsed.vertex_colors.is_none());
        }
    }
}

fn fnv1a64(bytes: impl IntoIterator<Item = u8>) -> u64 {
    bytes
        .into_iter()
        .fold(0xcbf2_9ce4_8422_2325, |digest, byte| {
            (digest ^ u64::from(byte)).wrapping_mul(0x100_0000_01b3)
        })
}

#[test]
#[ignore = "requires original EverQuest assets"]
fn all_sixteen_original_v2_streams_match_independent_counts_and_digests() {
    // Frozen independently by /tmp/openeq-ter-trailing-corpus.py. Counts,
    // offsets and FNV-1a-64 cover original on-disk float words, not f32 math.
    let cases: [(&str, &str, usize, usize, u64); 16] = [
        (
            "bazaar.eqg",
            "ter_bazaar.ter",
            225781,
            10609531,
            0x919eb5914c749245,
        ),
        (
            "broodlands.eqg",
            "ter_broodlands.ter",
            80359,
            5144639,
            0x6b24b4d8a951190e,
        ),
        (
            "delvea.eqg",
            "ter_volcano.ter",
            297090,
            16885144,
            0xf7bbc16046119565,
        ),
        (
            "delveb.eqg",
            "ter_volcano.ter",
            268623,
            12118788,
            0x47df775503568402,
        ),
        (
            "dranikcatacombsa.eqg",
            "ter_catacomba.ter",
            236286,
            12052128,
            0xbc3ab7fe6a8865f4,
        ),
        (
            "dranikcatacombsb.eqg",
            "ter_catacombb.ter",
            176386,
            9197675,
            0xb13216b102cc1cc5,
        ),
        (
            "dranikcatacombsc.eqg",
            "ter_catacombc.ter",
            359681,
            17829908,
            0x6f128c367fc8c84a,
        ),
        (
            "draniksewersa.eqg",
            "ter_sewera.ter",
            172876,
            8887787,
            0xf542d0e1fe92f112,
        ),
        (
            "draniksewersb.eqg",
            "ter_sewerb.ter",
            210524,
            10868763,
            0x6e30f8f0d49d9108,
        ),
        (
            "draniksewersc.eqg",
            "ter_sewerc.ter",
            221665,
            11261550,
            0x68bc547511d0f887,
        ),
        (
            "harbingers.eqg",
            "ter_harbingers.ter",
            232179,
            9267440,
            0x7e47f7fca08f762e,
        ),
        (
            "lavastorm.eqg",
            "ter_lava.ter",
            103728,
            5500636,
            0x7670b66add1fbe45,
        ),
        (
            "stillmoona.eqg",
            "ter_main.ter",
            669674,
            26868212,
            0x90c4bcffd898ec6b,
        ),
        (
            "stillmoonb.eqg",
            "ter_easterntemple.ter",
            72125,
            4015314,
            0x36ee29bed50ce596,
        ),
        (
            "thenest.eqg",
            "ter_abyss01.ter",
            384042,
            19523796,
            0x6df6e0ef2df4b6ef,
        ),
        (
            "thundercrest.eqg",
            "ter_stormtower01.ter",
            762815,
            30614444,
            0x29f16c49135f5316,
        ),
    ];
    let base = loader::default_client_dir().unwrap();
    let mut vertices = 0;
    let mut nonfinite_components = 0;
    for (archive_name, member, count, tail_offset, digest) in cases {
        let archive = Archive::open(base.join(archive_name)).unwrap();
        let raw = archive.read(member).unwrap();
        assert_eq!(&raw[tail_offset..tail_offset + 4], &1_u32.to_le_bytes());
        assert_eq!(raw.len(), tail_offset + 4 + 8 * count);
        assert_eq!(
            fnv1a64(raw[tail_offset + 4..].iter().copied()),
            digest,
            "{archive_name} raw"
        );
        let parsed = TerMod::parse(&raw, true).unwrap();
        assert_eq!(parsed.version, 2);
        assert_eq!(parsed.positions.len(), count);
        assert_eq!(parsed.tex_coords.len(), count);
        let coords = parsed.secondary_tex_coords.unwrap();
        assert_eq!(coords.len(), count);
        assert_eq!(
            fnv1a64(
                coords
                    .iter()
                    .flatten()
                    .flat_map(|v| v.to_bits().to_le_bytes())
            ),
            digest,
            "{archive_name} parsed"
        );
        nonfinite_components += coords.iter().flatten().filter(|v| !v.is_finite()).count();
        vertices += count;
        if archive_name == "thenest.eqg" {
            assert_eq!(coords[0].map(f32::to_bits), [0x4352_4702, 0xc1f7_01f2]);
            assert_eq!(coords[192021].map(f32::to_bits), [0x41f5_fad5, 0xc1b4_0e01]);
            assert_eq!(coords[384041].map(f32::to_bits), [0, 0]);
            assert_eq!(
                parsed.tex_coords[0].map(f32::to_bits),
                [0x43c3_1308, 0xc32a_8000]
            );
        }
        if archive_name == "thundercrest.eqg" {
            assert_eq!(coords[55098][1].to_bits(), 0x7fff_0005);
        }
    }
    assert_eq!(vertices, 4_473_834);
    assert_eq!(nonfinite_components, 1);
}
