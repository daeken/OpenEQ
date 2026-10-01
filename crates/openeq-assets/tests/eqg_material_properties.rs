//! Material type 1 is a distinct 32-bit word, not a float or string offset.
use openeq_assets::{
    Error, loader,
    pfs::Archive,
    zone::{MOD_MAGIC, Property, TER_MAGIC, TerMod, ZoneFile},
};

fn words(data: &mut Vec<u8>, values: &[u32]) {
    data.extend(values.iter().flat_map(|v| v.to_le_bytes()));
}

fn fixture(terrain: bool, version: u32, integer: u32) -> (Vec<u8>, usize) {
    let mut strings = Vec::new();
    let offsets: Vec<_> = [
        "stone",
        "Opaque.fx",
        "roughness",
        "channel",
        "diffuse",
        "color",
        "stone.dds",
    ]
    .into_iter()
    .map(|name| {
        let offset = strings.len() as u32;
        strings.extend(name.as_bytes());
        strings.push(0);
        offset
    })
    .collect();
    let mut data = Vec::new();
    words(
        &mut data,
        &[
            if terrain { TER_MAGIC } else { MOD_MAGIC },
            version,
            strings.len() as u32,
            1,
            3,
            1,
        ],
    );
    if !terrain {
        words(&mut data, &[0]);
    }
    data.extend(strings);
    words(&mut data, &[7, offsets[0], offsets[1], 4]);
    words(&mut data, &[offsets[2], 0, 0.75f32.to_bits()]);
    let integer_record = data.len();
    words(&mut data, &[offsets[3], 1, integer]);
    words(&mut data, &[offsets[4], 2, offsets[6]]);
    words(&mut data, &[offsets[5], 3, 0xff123456]);
    for x in [1f32, 4., 7.] {
        for value in [x, x + 1., x + 2., 0., 0., 1.] {
            data.extend(value.to_le_bytes());
        }
        if version == 3 {
            words(&mut data, &[0xaabbccdd]);
        }
        for value in [0.25f32, 0.75] {
            data.extend(value.to_le_bytes());
        }
        if version == 3 {
            words(&mut data, &[0; 2]);
        }
    }
    words(&mut data, &[0, 1, 2, 0, 0]);
    (data, integer_record)
}

#[test]
fn distinct_integer_words_preserve_every_bit_and_following_material_geometry_alignment() {
    for terrain in [false, true] {
        for version in [1, 3] {
            for integer in [0, 1, 2, 0xffff_fffd, 0x8000_0000, 0x7fc0_1234] {
                let (data, _) = fixture(terrain, version, integer);
                let model = TerMod::parse(&data, terrain).unwrap();
                let material = &model.materials[0];
                assert_eq!(material.stored_id, 7);
                assert!(
                    matches!(material.properties["channel"], Property::IntegerBits(value) if value == integer)
                );
                assert_eq!(material.properties["channel"].as_text(), None);
                assert!(matches!(
                    material.properties["roughness"],
                    Property::Float(0.75)
                ));
                assert_eq!(material.properties["diffuse"].as_text(), Some("stone.dds"));
                assert!(matches!(
                    material.properties["color"],
                    Property::Uint(0xff123456)
                ));
                assert_eq!(model.positions, [[1., 2., 3.], [4., 5., 6.], [7., 8., 9.]]);
                assert_eq!(model.normals, [[0., 0., 1.]; 3]);
                assert_eq!(model.tex_coords, [[0.25, 0.75]; 3]);
                assert_eq!(model.polygons, [(0, 1, 2, 0, 0)]);
            }
        }
    }
}

#[test]
fn truncated_integer_words_and_unestablished_property_kinds_remain_errors() {
    let (mut data, record) = fixture(true, 3, 2);
    for size in 0..4 {
        assert!(matches!(
            TerMod::parse(&data[..record + 8 + size], true),
            Err(Error::Truncated { .. })
        ));
    }
    for kind in [4u32, 255, u32::MAX] {
        data[record + 4..record + 8].copy_from_slice(&kind.to_le_bytes());
        assert!(
            matches!(TerMod::parse(&data, true), Err(Error::Format(message))
            if message.contains("property type") && message.contains("channel"))
        );
    }
}

#[test]
#[ignore = "requires original Housegarden archive and loose zone declaration; CPU only"]
fn original_housegarden_retains_channel_properties_and_loads_authored_geometry() {
    let base = loader::default_client_dir().expect("original client assets");
    let archive = Archive::open(base.join("housegarden.eqg")).unwrap();
    let zon = std::fs::read(base.join("housegarden.zon")).unwrap();
    let source = ZoneFile::parse(&zon, |name| archive.read(name)).unwrap();
    assert_eq!(source.objects.len(), 101);
    assert_eq!(source.placeables.len(), 6267);
    let mut channels = [0; 3];
    for model in &source.objects {
        for material in &model.materials {
            for (name, value) in &material.properties {
                if let Property::IntegerBits(word) = value {
                    match (name.as_str(), word) {
                        ("e_TextureDiffuse0mapChannel", 1) => channels[0] += 1,
                        ("e_TextureNormal0mapChannel", 1) => channels[1] += 1,
                        ("e_TextureSecond0mapChannel", 2) => channels[2] += 1,
                        _ => panic!("unexpected original integer property {name}={word:#x}"),
                    }
                }
            }
        }
    }
    // One affected MOD is unreferenced. The six words in TER records whose
    // stored IDs repeat are retained alongside every other source record.
    assert_eq!(channels, [48, 48, 48]);
    let scene = loader::load_zone(base, "housegarden").unwrap();
    assert_eq!(scene.instances.len(), 6267);
    assert!(scene.triangle_count() > 20_000);
    assert!(
        scene
            .collision_meshes
            .iter()
            .map(|m| m.indices.len() / 3)
            .sum::<usize>()
            > 20_000
    );
    assert!(
        scene
            .materials
            .iter()
            .any(|m| m.textures.iter().any(|name| name != "missing.dds"))
    );
}
