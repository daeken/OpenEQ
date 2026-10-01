//! Packed track scale is unsigned even though rotation and translation are signed.

use openeq_assets::wld::{Fragment, WLD_MAGIC, Wld};

fn track_wld(flags: u32, frame_count: u32, frames: &[u8]) -> Wld {
    let mut bytes = Vec::new();
    for word in [WLD_MAGIC, 0x0001_5500, 1, 0, 0, 4, 0] {
        bytes.extend(word.to_le_bytes());
    }
    bytes.extend([0x95, 0x3a, 0xc5, 0x2a]); // Four encoded NULs.
    for word in [12 + frames.len() as u32, 0x12, 0, flags, frame_count] {
        bytes.extend(word.to_le_bytes());
    }
    bytes.extend(frames);
    Wld::parse("scale-fixture.wld".into(), &bytes).unwrap()
}

#[test]
fn packed_scale_crosses_the_signed_boundary_without_changing_other_components() {
    let cases = [
        (0x0000u16, 0.),
        (0x0100, 1.),
        (0x7fff, 128. - 1. / 256.),
        (0x8000, 128.),
        (0x8001, 128. + 1. / 256.),
        (0xd579, 213. + 121. / 256.),
        (0xffff, 256. - 1. / 256.),
    ];
    let mut bytes = Vec::new();
    for (scale, _) in cases {
        // High bits in the other seven words must still represent negatives.
        for word in [
            0xc000, 0x8000, 0x7fff, 0xffff, 0x8000, 0xffff, 0x7fff, scale,
        ] {
            bytes.extend(word.to_le_bytes());
        }
    }
    let wld = track_wld(8, cases.len() as u32, &bytes);
    let Fragment::PieceTrack(track) = &wld.chunks()[0].fragment else {
        panic!("expected packed track");
    };
    assert_eq!(track.frames.len(), cases.len());
    for (frame, (word, expected)) in track.frames.iter().zip(cases) {
        assert_eq!(frame.scale, expected, "packed scale word {word:#06x}");
        assert_eq!(frame.rotation, [-2., 2. - 1. / 16384., -1. / 16384., -1.]);
        assert_eq!(frame.translation, [-128., -1. / 256., 128. - 1. / 256.]);
    }
}

#[test]
fn float_scale_keeps_its_authored_sign_and_layout() {
    let frames = [-2.5f32, -128., 0.5, 128., 1., -0.5, 0.25, -0.125]
        .into_iter()
        .flat_map(f32::to_le_bytes)
        .collect::<Vec<_>>();
    let wld = track_wld(0, 1, &frames);
    let Fragment::PieceTrack(track) = &wld.chunks()[0].fragment else {
        panic!("expected float track");
    };
    assert_eq!(track.frames[0].scale, -2.5);
    assert_eq!(track.frames[0].translation, [-128., 0.5, 128.]);
    assert_eq!(track.frames[0].rotation, [-0.5, 0.25, -0.125, 1.]);
}

#[test]
#[ignore = "requires original WRW, Plane of Tactics and Plane of Time B character assets"]
fn original_high_bit_scale_tracks_match_the_native_unsigned_read() {
    let base = openeq_assets::loader::default_client_dir().expect("original client assets");
    for (zone, fragment) in [("wrw", 2087), ("potactics", 29430), ("potimeb", 43556)] {
        let archive =
            openeq_assets::pfs::Archive::open(base.join(format!("{zone}_chr.s3d"))).unwrap();
        let wld = Wld::open(&archive, &format!("{zone}_chr.wld")).unwrap();
        // This is the second of two same-name definitions. Preserve identity
        // by fragment number rather than selecting the first name match.
        let chunk = &wld.chunks()[fragment - 1];
        assert_eq!(chunk.name, "WRWWRW_TRACKDEF");
        let Fragment::PieceTrack(track) = &chunk.fragment else {
            panic!("expected original packed track");
        };
        assert_eq!(track.flags, 8);
        assert_eq!(track.frames.len(), 21);
        for frame in &track.frames {
            assert_eq!(frame.scale, 213. + 121. / 256.);
        }
        assert_eq!(track.frames[0].rotation, [-0.5; 4]);
        assert_eq!(track.frames[0].translation, [0., 0., -94.140_625]);
        let Fragment::PieceTrack(previous) = &wld.chunks()[fragment - 3].fragment else {
            panic!("expected earlier same-name track");
        };
        assert_eq!(previous.frames[0].scale, 1.);
    }
}
