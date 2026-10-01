use super::*;

fn bitmap_body(filenames: &[&str]) -> Vec<u8> {
    let mut bytes = words(&[filenames.len() as u32 - 1]);
    let key = [0x95, 0x3a, 0xc5, 0x2a, 0x95, 0x7a, 0x95, 0x6a];
    for filename in filenames {
        bytes.extend(((filename.len() + 1) as u16).to_le_bytes());
        bytes.extend(
            filename
                .bytes()
                .chain([0])
                .enumerate()
                .map(|(i, byte)| byte ^ key[i % 8]),
        );
    }
    while !bytes.len().is_multiple_of(4) {
        bytes.push(0);
    }
    bytes
}

fn textured_fixture() -> (Fixture, [Ref; 4]) {
    let mut fixture = particle_fixture(true, false);
    let bitmap = fixture.add(0x03, "BITMAP", bitmap_body(&["Original.DDS"]));
    let animation = fixture.add(0x04, "FRAMES", words(&[0x18, 1, 100, bitmap.0 as u32]));
    let link = fixture.add(0x05, "LINK", words(&[animation.0 as u32, 0]));
    let binding = Ref(257);
    fixture.0[256].2 = words(&[0, link.0 as u32, 0x8000_0017]);
    (fixture, [binding, link, animation, bitmap])
}

fn chain(source: &ObjectSource) -> &ObjectParticleTexture {
    &source.particle_attachments[0].texture
}

fn check_body_survives(wld: &Wld) -> ObjectSource {
    let source = source(wld, "TORCH_ACTORDEF");
    assert_eq!(source.parts.len(), 1);
    assert_eq!(source.particle_attachments.len(), 2);
    assert_eq!(source.parts[0].mesh.polygons.len(), 2);
    let mut scene = empty_scene();
    append_objects(&mut scene, 0, wld).unwrap();
    assert!(scene.wld_object_sources.contains_key("torch"));
    let object = scene
        .objects
        .iter()
        .find(|object| object.name == "torch")
        .unwrap();
    assert_eq!(object.meshes.len(), 1);
    assert_eq!(object.collision_meshes.len(), 1);
    assert_eq!(scene.meshes[object.meshes[0]].indices.len(), 3);
    assert_eq!(
        scene.collision_meshes[object.collision_meshes[0]]
            .indices
            .len(),
        3
    );
    assert_eq!(
        CollisionWorld::build(&scene.object_model("torch").unwrap()).triangle_count(),
        2
    );
    assert!(source.render_animation().is_none());
    source
}

#[test]
fn authored_texture_chain_preserves_full_refs_names_timing_and_duplicate_identity() {
    let (mut fixture, refs) = textured_fixture();
    // None of these same-name definitions may replace an authored positive ref.
    fixture.add(0x26, "PARTICLE_TEXTURE", words(&[7, 0, 0]));
    fixture.add(0x05, "LINK", words(&[0, 9]));
    fixture.add(0x04, "FRAMES", words(&[0, 0]));
    fixture.add(0x03, "BITMAP", bitmap_body(&["Wrong.DDS"]));
    let source = check_body_survives(&fixture.finish());
    for attachment in &source.particle_attachments {
        let texture = &attachment.texture;
        assert!(texture.issues.is_empty(), "{:?}", texture.issues);
        assert_eq!(texture.wld_filename, "fixture_obj.wld");
        assert_eq!(texture.source_reference, Some(refs[0]));
        let binding = texture.binding.as_ref().unwrap();
        assert_eq!(binding.source_reference, refs[0]);
        assert_eq!(binding.definition_reference, refs[0]);
        assert_eq!(binding.name, "PARTICLE_TEXTURE");
        assert_eq!(binding.definition.texture, refs[1]);
        assert_eq!(binding.definition.material, 0x8000_0017);
        let link = texture.animation_reference.as_ref().unwrap();
        assert_eq!(link.source_reference, refs[1]);
        assert_eq!(link.definition_reference, refs[1]);
        assert_eq!(link.definition.animation, refs[2]);
        let animation = texture.animation.as_ref().unwrap();
        assert_eq!(animation.definition_reference, refs[2]);
        assert_eq!(animation.definition.flags, 0x18);
        assert_eq!(animation.definition.frame_time, 100);
        assert_eq!(animation.definition.textures, [refs[3]]);
        let bitmap = texture.frames[0].as_ref().unwrap();
        assert_eq!(bitmap.source_reference, refs[3]);
        assert_eq!(bitmap.definition_reference, refs[3]);
        assert_eq!(bitmap.definition.filenames, ["Original.DDS"]);
    }
}

fn named_reference(fixture: &Fixture, reference: Ref) -> Ref {
    Ref(-1
        - fixture.0[..reference.fragment_index().unwrap()]
            .iter()
            .map(|(_, name, _)| name.len() as i32 + 1)
            .sum::<i32>())
}

#[test]
fn authored_negative_refs_retain_their_signed_values_and_exact_resolved_nodes() {
    let (mut fixture, refs) = textured_fixture();
    let names = refs.map(|reference| named_reference(&fixture, reference));
    fixture.0[257].2[80..84].copy_from_slice(&names[0].0.to_le_bytes());
    fixture.0[256].2[4..8].copy_from_slice(&names[1].0.to_le_bytes());
    fixture.0[refs[1].fragment_index().unwrap()].2[..4].copy_from_slice(&names[2].0.to_le_bytes());
    fixture.0[refs[2].fragment_index().unwrap()].2[12..16]
        .copy_from_slice(&names[3].0.to_le_bytes());
    let source = check_body_survives(&fixture.finish());
    let texture = chain(&source);
    assert!(texture.issues.is_empty());
    assert_eq!(texture.source_reference, Some(names[0]));
    let binding = texture.binding.as_ref().unwrap();
    assert_eq!(
        (binding.source_reference, binding.definition_reference),
        (names[0], refs[0])
    );
    let link = texture.animation_reference.as_ref().unwrap();
    assert_eq!(
        (link.source_reference, link.definition_reference),
        (names[1], refs[1])
    );
    let animation = texture.animation.as_ref().unwrap();
    assert_eq!(
        (animation.source_reference, animation.definition_reference),
        (names[2], refs[2])
    );
    let bitmap = texture.frames[0].as_ref().unwrap();
    assert_eq!(
        (bitmap.source_reference, bitmap.definition_reference),
        (names[3], refs[3])
    );
}

#[test]
fn missing_or_wrong_texture_targets_preserve_mesh_siblings_and_partial_metadata() {
    for stage in 0..4 {
        for target in [Ref(0), Ref(50000), Ref(i32::MIN), Ref(-12345), Ref(200)] {
            let (mut fixture, refs) = textured_fixture();
            let (index, offset) = match stage {
                0 => (257, 80),
                1 => (256, 4),
                2 => (refs[1].fragment_index().unwrap(), 0),
                3 => (refs[2].fragment_index().unwrap(), 12),
                _ => unreachable!(),
            };
            fixture.0[index].2[offset..offset + 4].copy_from_slice(&target.0.to_le_bytes());
            let source = check_body_survives(&fixture.finish());
            let texture = chain(&source);
            assert_eq!(texture.binding.is_some(), stage > 0);
            assert_eq!(texture.animation_reference.is_some(), stage > 1);
            assert_eq!(texture.animation.is_some(), stage > 2);
            if stage == 3 {
                assert!(texture.frames[0].is_none());
            }
            assert_eq!(texture.issues.len(), 1);
            if target == Ref(200) {
                assert!(
                    matches!(texture.issues[0], ObjectParticleTextureIssue::UnexpectedFragment { source_reference, actual_kind: 0x99, .. } if source_reference == target)
                );
            } else {
                assert_eq!(
                    texture.issues[0],
                    ObjectParticleTextureIssue::MissingReference {
                        source_reference: target
                    }
                );
            }
        }
    }
}

#[test]
fn cycles_are_distinguished_from_repeated_bitmap_frames() {
    for stage in 1..4 {
        let (mut fixture, refs) = textured_fixture();
        let (index, offset) = match stage {
            1 => (256, 4),
            2 => (refs[1].fragment_index().unwrap(), 0),
            3 => (refs[2].fragment_index().unwrap(), 12),
            _ => unreachable!(),
        };
        fixture.0[index].2[offset..offset + 4].copy_from_slice(&refs[0].0.to_le_bytes());
        let source = check_body_survives(&fixture.finish());
        assert_eq!(
            chain(&source).issues,
            [ObjectParticleTextureIssue::Cycle {
                source_reference: refs[0],
                definition_reference: refs[0]
            }]
        );
    }
    let (mut fixture, refs) = textured_fixture();
    fixture.0[refs[2].fragment_index().unwrap()].2 =
        words(&[0x18, 2, 75, refs[3].0 as u32, refs[3].0 as u32]);
    let source = check_body_survives(&fixture.finish());
    let texture = chain(&source);
    assert_eq!(texture.frames.len(), 2);
    assert!(
        texture
            .frames
            .iter()
            .all(|frame| frame.as_ref().unwrap().definition_reference == refs[3])
    );
    assert_eq!(
        texture.animation.as_ref().unwrap().definition.frame_time,
        75
    );
    assert_eq!(
        texture.animation.as_ref().unwrap().definition.textures,
        [refs[3]; 2]
    );
    assert!(matches!(
        texture.issues.as_slice(),
        [ObjectParticleTextureIssue::UnsupportedLayout { .. }]
    ));
}

#[test]
fn unknown_texture_layouts_remain_explicit_without_rejecting_the_body() {
    for case in 0..12 {
        let (mut fixture, refs) = textured_fixture();
        let binding = refs[0].fragment_index().unwrap();
        let link = refs[1].fragment_index().unwrap();
        let animation = refs[2].fragment_index().unwrap();
        let bitmap = refs[3].fragment_index().unwrap();
        match case {
            0 => fixture.0[binding].2[..4].copy_from_slice(&7u32.to_le_bytes()),
            1 => fixture.0[binding].2.extend([1, 2, 3, 4]),
            2 => fixture.0[binding].2[8..12].copy_from_slice(&0x1234u32.to_le_bytes()),
            3 => fixture.0[link].2[4..8].copy_from_slice(&1u32.to_le_bytes()),
            4 => fixture.0[link].2.extend([9; 4]),
            5 => fixture.0[animation].2[..4].copy_from_slice(&0x19u32.to_le_bytes()),
            6 => fixture.0[animation].2 = words(&[0x1c, 1, 123, 100, refs[3].0 as u32]),
            7 => fixture.0[animation].2.extend([0x44; 4]),
            8 => fixture.0[bitmap].2 = bitmap_body(&["Original.DDS", "Layer.DDS"]),
            9 => fixture.0[bitmap].2 = bitmap_body(&[""]),
            10 => fixture.0[bitmap].2.extend([1]),
            11 => fixture.0[bitmap].2.extend([0; 4]),
            _ => unreachable!(),
        }
        let source = check_body_survives(&fixture.finish());
        let texture = chain(&source);
        assert_eq!(texture.issues.len(), 1, "case {case}: {:?}", texture.issues);
        assert!(
            texture.binding.is_some()
                && texture.animation_reference.is_some()
                && texture.animation.is_some()
                && texture.frames[0].is_some()
        );
        if case == 2 {
            assert_eq!(
                texture.issues[0],
                ObjectParticleTextureIssue::UnsupportedMaterial { material: 0x1234 }
            );
        } else {
            assert!(matches!(
                texture.issues[0],
                ObjectParticleTextureIssue::UnsupportedLayout { .. }
            ));
        }
        if case == 1 {
            assert_eq!(
                texture.binding.as_ref().unwrap().definition.tail,
                [1, 2, 3, 4]
            );
        }
        if case == 4 {
            assert_eq!(
                texture
                    .animation_reference
                    .as_ref()
                    .unwrap()
                    .definition
                    .tail,
                [9; 4]
            );
        }
        if case == 6 {
            assert_eq!(
                texture.animation.as_ref().unwrap().definition.parameter,
                Some(123)
            );
        }
        if case == 7 {
            assert_eq!(
                texture.animation.as_ref().unwrap().definition.tail,
                [0x44; 4]
            );
        }
    }
}

#[test]
fn metadata_limits_stop_expansion_before_copying_large_lists_names_or_tails() {
    for case in 0..8 {
        let (mut fixture, refs) = textured_fixture();
        let animation = refs[2].fragment_index().unwrap();
        let bitmap = refs[3].fragment_index().unwrap();
        match case {
            0 => {
                let mut body = words(&[0x18, 17, 100]);
                body.extend(words(&[refs[3].0 as u32; 17]));
                fixture.0[animation].2 = body;
            }
            1 => fixture.0[256].2.extend(vec![0; 4097]),
            2 => fixture.0[refs[1].fragment_index().unwrap()]
                .2
                .extend(vec![0; 4097]),
            3 => fixture.0[animation].2.extend(vec![0; 4097]),
            4 => fixture.0[bitmap].2 = bitmap_body(&["layer.dds"; 17]),
            5 => fixture.0[bitmap].2 = bitmap_body(&[&"x".repeat(4097)]),
            6 => fixture.0[bitmap].1 = "x".repeat(4097),
            7 => fixture.0[bitmap].2.extend(vec![0; 4097]),
            _ => unreachable!(),
        }
        let source = check_body_survives(&fixture.finish());
        let texture = chain(&source);
        assert!(
            matches!(
                texture.issues.as_slice(),
                [ObjectParticleTextureIssue::MetadataLimit { .. }]
            ),
            "case {case}: {:?}",
            texture.issues
        );
        if case == 0 || case == 3 {
            assert!(texture.animation.is_none() && texture.frames.is_empty());
        }
        if case == 1 {
            assert!(texture.binding.is_none());
        }
        if case == 2 {
            assert!(texture.animation_reference.is_none());
        }
        if case >= 4 {
            assert!(texture.frames[0].is_none());
        }
    }
}

#[test]
fn missing_root_and_zero_padding_are_reported_without_inventing_texture_data() {
    let (fixture, _) = textured_fixture();
    let texture = particle_texture_source(&fixture.finish(), None);
    assert_eq!(
        texture.issues,
        [ObjectParticleTextureIssue::MissingTextureReference]
    );
    assert!(texture.binding.is_none() && texture.animation.is_none() && texture.frames.is_empty());
    for filename in ["a.dds", "ab.dds", "abc.dds", "abcd.dds"] {
        let (mut fixture, refs) = textured_fixture();
        fixture.0[refs[3].fragment_index().unwrap()].2 = bitmap_body(&[filename]);
        let source = check_body_survives(&fixture.finish());
        let texture = chain(&source);
        assert!(texture.issues.is_empty());
        let expected_padding = (4 - (4 + 2 + filename.len() + 1) % 4) % 4;
        assert_eq!(
            texture.frames[0].as_ref().unwrap().definition.tail,
            vec![0; expected_padding]
        );
    }
}

#[test]
#[ignore = "requires original Plane of Knowledge object texture metadata"]
fn original_pok_clouds_and_eight_actor_sources_preserve_duplicate_texture_identities() {
    let base = crate::loader::default_client_dir().expect("original client assets");
    let archive = crate::pfs::Archive::open(base.join("poknowledge_obj.s3d")).unwrap();
    let wld = Wld::open(&archive, "poknowledge_obj.wld").unwrap();
    for (cloud_ref, binding_ref, filename) in [
        (5, 4, "CSMOKE.DDS"),
        (10, 9, "GENG00.DDS"),
        (15, 14, "GENG00.DDS"),
        (20, 19, "GENG00.DDS"),
        (406, 405, "CSMOKE.DDS"),
        (411, 410, "GENG00.DDS"),
        (438, 437, "GENG00.DDS"),
        (471, 470, "GENG00.DDS"),
    ] {
        let Fragment::ParticleCloud(cloud) = &wld.resolve(Ref(cloud_ref)).unwrap().fragment else {
            panic!("cloud")
        };
        let texture = particle_texture_source(&wld, cloud.texture_reference);
        assert!(
            texture.issues.is_empty(),
            "cloud {cloud_ref}: {:?}",
            texture.issues
        );
        assert_eq!(
            texture.binding.as_ref().unwrap().definition_reference,
            Ref(binding_ref)
        );
        assert_eq!(
            texture
                .animation_reference
                .as_ref()
                .unwrap()
                .definition_reference,
            Ref(binding_ref - 1)
        );
        assert_eq!(
            texture.animation.as_ref().unwrap().definition_reference,
            Ref(binding_ref - 2)
        );
        assert_eq!(
            texture.frames[0].as_ref().unwrap().definition_reference,
            Ref(binding_ref - 3)
        );
        assert_eq!(
            texture.frames[0].as_ref().unwrap().definition.filenames,
            [filename]
        );
    }
    let scene = crate::loader::load_zone(&base, "poknowledge").unwrap();
    let mut attachment_count = 0;
    for actor in [
        "ftorch301",
        "ftorch302",
        "ftorch304",
        "poklamp500",
        "poklamp501",
        "poklamp502",
        "poksconce500",
        "poktorch500",
    ] {
        let source = &scene.wld_object_sources[actor];
        for attachment in &source.particle_attachments {
            attachment_count += 1;
            assert!(
                attachment.texture.issues.is_empty(),
                "{actor}: {:?}",
                attachment.texture.issues
            );
            assert_eq!(
                attachment
                    .texture
                    .binding
                    .as_ref()
                    .unwrap()
                    .source_reference,
                attachment.definition.texture_reference.unwrap()
            );
        }
    }
    assert_eq!(attachment_count, 10);
}

#[test]
#[ignore = "requires original object-WLD corpus; texture metadata only"]
fn original_object_corpus_particle_texture_metadata_is_complete_without_cache_aliases() {
    let base = crate::loader::default_client_dir().expect("original client assets");
    let mut total = 0;
    let mut high_refs = 0;
    for entry in std::fs::read_dir(&base).unwrap() {
        let path = entry.unwrap().path();
        let name = path
            .file_name()
            .unwrap()
            .to_string_lossy()
            .to_ascii_lowercase();
        if !name.contains("_obj") || !name.ends_with(".s3d") {
            continue;
        }
        let archive = crate::pfs::Archive::open(path).unwrap();
        for member in archive.names().iter().filter(|name| name.ends_with(".wld")) {
            let wld = Wld::open(&archive, member).unwrap();
            for (_, cloud) in wld.iter::<ParticleCloud>() {
                total += 1;
                high_refs += usize::from(cloud.texture_reference.unwrap().0 > 255);
                let texture = particle_texture_source(&wld, cloud.texture_reference);
                assert!(
                    texture.issues.is_empty(),
                    "{name}/{member}: {:?}",
                    texture.issues
                );
                assert_eq!(
                    texture.binding.as_ref().unwrap().definition_reference,
                    cloud.texture_reference.unwrap()
                );
                assert_eq!(
                    texture.animation.as_ref().unwrap().definition.frame_time,
                    100
                );
                assert_eq!(texture.frames.len(), 1);
            }
        }
    }
    assert_eq!((total, high_refs), (623, 178));
}
