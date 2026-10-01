use super::*;

fn packed_track(fixture: &mut Fixture, translation: [i16; 3]) -> Ref {
    let mut bytes = words(&[8, 1]);
    bytes.extend(
        [16384i16, 0, 0, 0]
            .into_iter()
            .chain(translation)
            .chain([256])
            .flat_map(i16::to_le_bytes),
    );
    let definition = fixture.add(0x12, "", bytes);
    fixture.add(0x13, "", words(&[definition.0 as u32, 0]))
}

fn particle_fixture(weighted: bool, mesh_under_particle: bool) -> Fixture {
    let mut fixture = Fixture::default();
    let mesh = fixture.mesh(weighted);
    // Full-width texture identity must survive even though the native reader
    // has a low-byte/cache quirk. This is only a structural metadata gate.
    while fixture.0.len() < 256 {
        fixture.add(0x99, "", vec![]);
    }
    let texture = fixture.add(0x26, "PARTICLE_TEXTURE", words(&[0, 0, 0x8000_0017]));
    let mut cloud_words = [0; 20];
    cloud_words[0] = 4;
    cloud_words[4] = 40;
    let mut body = words(&cloud_words);
    body.extend(texture.0.to_le_bytes());
    let cloud = fixture.add(0x34, "FIRE_PCD", body.clone());
    // Same-name lookup would find this different, unused definition instead.
    body[16..20].copy_from_slice(&80u32.to_le_bytes());
    fixture.add(0x34, "FIRE_PCD", body);
    let root = packed_track(&mut fixture, [2560, 0, 0]);
    let base = packed_track(&mut fixture, [0, 512, 0]);
    let flame = packed_track(&mut fixture, [0, 0, 768]);
    let smoke = packed_track(&mut fixture, [0, 0, 1024]);
    let rigid = if weighted { Ref(0) } else { mesh };
    let skeleton = if mesh_under_particle {
        fixture.skeleton(
            &[
                (root, Ref(0), &[2]),
                (base, rigid, &[3]),
                (flame, cloud, &[1]),
                (smoke, cloud, &[]),
            ],
            &[mesh],
        )
    } else {
        fixture.skeleton(
            &[
                (root, Ref(0), &[1]),
                (base, rigid, &[2]),
                (flame, cloud, &[3]),
                (smoke, cloud, &[]),
            ],
            &[mesh],
        )
    };
    fixture.actor("TORCH_ACTORDEF", skeleton);
    fixture
}

fn actor_error(fixture: Fixture) -> String {
    let wld = fixture.finish();
    let chunk = wld.by_name("TORCH_ACTORDEF").unwrap();
    let Fragment::ActorDef(actor) = &chunk.fragment else {
        panic!("actor")
    };
    let error = actor_source(&wld, chunk, actor).unwrap_err().to_string();
    let mut scene = empty_scene();
    append_objects(&mut scene, 0, &wld).unwrap();
    assert!(!scene.objects.iter().any(|object| object.name == "torch"));
    assert!(scene.objects.iter().any(|object| object.name == "part"));
    assert!(!scene.wld_object_sources.contains_key("torch"));
    error
}

#[test]
fn static_particle_actor_retains_typed_owners_geometry_and_hidden_collision() {
    for weighted in [false, true] {
        let wld = particle_fixture(weighted, false).finish();
        let source = source(&wld, "TORCH_ACTORDEF");
        assert_eq!(source.parts.len(), 1);
        assert_eq!(source.particle_attachments.len(), 2);
        let skeleton = source.skeleton.as_ref().unwrap();
        assert_eq!(skeleton.parents, [None, Some(0), Some(1), Some(2)]);
        assert_eq!(skeleton.tracks.len(), 4);
        for (attachment, track) in source.particle_attachments.iter().zip([2, 3]) {
            assert_eq!(attachment.owner_track, track);
            assert_eq!(attachment.source_reference, Ref(258));
            assert_eq!(attachment.definition_reference, Ref(258));
            assert_eq!(attachment.definition.fixed_words[4], 40);
            assert_eq!(attachment.definition.texture_reference, Some(Ref(257)));
            assert_eq!(attachment.name, "FIRE_PCD");
        }
        let posed = source.sample_authored_frames(&[0; 4]).unwrap();
        assert_eq!(posed[0].polygons.len(), 2);
        near(posed[0].vertices[0], [12., 5., 4.]);
        near(posed[0].vertices[3], [12., 5., 6.]);
        assert!(source.animation_period().is_err());
        let mut scene = empty_scene();
        append_objects(&mut scene, 0, &wld).unwrap();
        let retained = &scene.wld_object_sources["torch"];
        assert_eq!(retained.particle_attachments.len(), 2);
        assert!(retained.render_animation().is_none());
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
        let model = scene.object_model("torch").unwrap();
        assert_eq!(CollisionWorld::build(&model).triangle_count(), 2);
    }
}

#[test]
fn particle_dependency_rejects_rigid_weighted_and_hidden_mesh_vertices() {
    for weighted in [false, true] {
        assert!(
            actor_error(particle_fixture(weighted, true)).contains("particle attachment ancestry")
        );
    }
    let mut fixture = particle_fixture(true, false);
    let (_, _, mesh) = fixture
        .0
        .iter_mut()
        .find(|(kind, _, _)| *kind == 0x36)
        .unwrap();
    // The visible triangle still binds to track 1. Only the hidden collidable
    // triangle binds to particle node 2; it must not escape the ancestry gate.
    mesh[82..84].copy_from_slice(&2u16.to_le_bytes());
    let run_start = mesh.len() - 12;
    mesh.splice(
        run_start..run_start + 4,
        [3u16, 1, 3, 2].into_iter().flat_map(u16::to_le_bytes),
    );
    assert!(actor_error(fixture).contains("particle attachment ancestry"));
}

#[test]
fn partial_particle_actor_rejects_unproven_motion_layouts_and_broken_bindings() {
    for case in 0..6 {
        let mut fixture = particle_fixture(true, false);
        match case {
            0 => {
                let (_, _, track) = fixture
                    .0
                    .iter_mut()
                    .find(|(kind, _, _)| *kind == 0x12)
                    .unwrap();
                track[4..8].copy_from_slice(&2u32.to_le_bytes());
                track.extend_from_within(8..24);
            }
            1 => {
                let (_, _, track) = fixture
                    .0
                    .iter_mut()
                    .find(|(kind, _, _)| *kind == 0x13)
                    .unwrap();
                track[4..8].copy_from_slice(&4u32.to_le_bytes());
            }
            2 => {
                let (_, _, track) = fixture
                    .0
                    .iter_mut()
                    .find(|(kind, _, _)| *kind == 0x13)
                    .unwrap();
                track[4..8].copy_from_slice(&1u32.to_le_bytes());
                track.extend(33u32.to_le_bytes());
            }
            3 => {
                let (_, _, track) = fixture
                    .0
                    .iter_mut()
                    .find(|(kind, _, _)| *kind == 0x12)
                    .unwrap();
                *track = words(&[0, 1]);
                track.extend(
                    [1f32, 0., 0., 0., 1., 0., 0., 0.]
                        .into_iter()
                        .flat_map(f32::to_le_bytes),
                );
            }
            4 => {
                let (_, _, mesh) = fixture
                    .0
                    .iter_mut()
                    .find(|(kind, _, _)| *kind == 0x36)
                    .unwrap();
                let run_start = mesh.len() - 12;
                mesh[run_start..run_start + 2].copy_from_slice(&5u16.to_le_bytes());
            }
            5 => {
                let (_, _, track) = fixture
                    .0
                    .iter_mut()
                    .find(|(kind, _, _)| *kind == 0x12)
                    .unwrap();
                track[22..24].fill(0); // Degenerate packed scale.
            }
            _ => unreachable!(),
        }
        let error = actor_error(fixture);
        assert!(
            error.contains(if case < 4 {
                "particle-linked actor"
            } else if case == 4 {
                "vertex-piece"
            } else {
                "pose"
            }),
            "case {case}: {error}"
        );
    }
}

#[test]
fn partial_particle_actor_rejects_unknown_attachments_tails_and_bad_texture_targets() {
    for case in 0..7 {
        let mut fixture = particle_fixture(true, false);
        let (kind, _, cloud) = &mut fixture.0[257];
        let expected = match case {
            0 => {
                *kind = 0x99;
                "0x99"
            }
            1 => {
                cloud.extend([0; 4]);
                "particle definition layout"
            }
            2 => {
                cloud[..4].copy_from_slice(&0x84u32.to_le_bytes());
                "particle definition layout"
            }
            3 => {
                cloud[80..84].copy_from_slice(&50000i32.to_le_bytes());
                "particle texture reference"
            }
            4 => {
                // Existing unrelated metadata is not a particle texture target.
                cloud[80..84].copy_from_slice(&200i32.to_le_bytes());
                "particle texture reference"
            }
            5 => {
                cloud[80..84].copy_from_slice(&i32::MIN.to_le_bytes());
                "particle texture reference"
            }
            6 => {
                cloud[80..84].copy_from_slice(&(-12345i32).to_le_bytes());
                "particle texture reference"
            }
            _ => unreachable!(),
        };
        assert!(actor_error(fixture).contains(expected));
    }
}

#[test]
fn invalid_negative_attachment_cannot_alias_an_unnamed_particle_definition() {
    let mut fixture = particle_fixture(true, false);
    let cloud = fixture.0[257].2.clone();
    fixture.add(0x34, "", cloud);
    let (_, _, skeleton) = fixture
        .0
        .iter_mut()
        .find(|(kind, _, _)| *kind == 0x10)
        .unwrap();
    // Third node's attachment ref; this invalid name offset can resolve to an
    // unnamed fragment in the legacy generic resolver. It must not gain partial
    // actor admission through the new typed particle branch.
    skeleton[72..76].copy_from_slice(&(-12345i32).to_le_bytes());
    actor_error(fixture);
}

#[test]
#[ignore = "requires original Plane of Knowledge archives; CPU geometry only"]
fn original_pok_particle_actors_restore_static_meshes_with_explicit_partial_ownership() {
    let base = crate::loader::default_client_dir().expect("original client assets");
    let scene = crate::loader::load_zone(&base, "poknowledge").unwrap();
    let mut totals = [0; 4];
    for (name, placements, parts, vertices, polygons, attachments) in [
        ("ftorch301", 60, 1, 307, 168, vec![(3, 411), (4, 406)]),
        ("ftorch302", 13, 1, 183, 128, vec![(3, 438)]),
        ("ftorch304", 17, 1, 273, 160, vec![(3, 471)]),
        ("poklamp500", 16, 11, 831, 482, vec![(3, 20)]),
        ("poklamp501", 13, 9, 1376, 1000, vec![(3, 20)]),
        ("poklamp502", 30, 12, 1660, 918, vec![(3, 20)]),
        ("poksconce500", 49, 1, 94, 90, vec![(3, 20), (4, 406)]),
        ("poktorch500", 168, 3, 269, 184, vec![(3, 20)]),
    ] {
        let source = &scene.wld_object_sources[name];
        assert_eq!(source.parts.len(), parts, "{name}");
        assert_eq!(
            source
                .parts
                .iter()
                .map(|part| part.mesh.vertices.len())
                .sum::<usize>(),
            vertices,
            "{name}"
        );
        assert_eq!(
            source
                .parts
                .iter()
                .map(|part| part.mesh.polygons.len())
                .sum::<usize>(),
            polygons,
            "{name}"
        );
        assert!(
            source
                .parts
                .iter()
                .flat_map(|part| &part.mesh.polygons)
                .all(|polygon| polygon.collidable)
        );
        let skeleton = source.skeleton.as_ref().unwrap();
        for part in &source.parts {
            assert!(
                part_bindings(part, skeleton.tracks.len())
                    .unwrap()
                    .iter()
                    .all(|&track| track == 1)
            );
        }
        assert_eq!(skeleton.parents[1], Some(0));
        assert_eq!(
            source
                .particle_attachments
                .iter()
                .map(|attachment| (attachment.owner_track, attachment.definition_reference.0))
                .collect::<Vec<_>>(),
            attachments
        );
        assert!(source.render_animation().is_none());
        assert!(source.animation_period().is_err());
        let actual_placements = scene
            .instances
            .iter()
            .filter(|instance| instance.object == name)
            .count();
        assert_eq!(actual_placements, placements, "{name}");
        let model = scene.object_model(name).unwrap();
        assert_eq!(model.triangle_count(), polygons, "{name}");
        // Source polygons remain intact, including authored degenerates that
        // the collision builder deliberately excludes. Count usable source
        // faces independently before comparing the baked collision inventory.
        let usable_faces = source
            .parts
            .iter()
            .flat_map(|part| {
                part.mesh.polygons.iter().filter(|polygon| {
                    let [a, b, c] = [polygon.a, polygon.b, polygon.c]
                        .map(|index| Vec3::from_array(part.mesh.vertices[index as usize]));
                    (b - a).cross(c - a).length_squared() >= 1e-10
                })
            })
            .count();
        assert_eq!(
            CollisionWorld::build(&model).triangle_count(),
            usable_faces,
            "{name}"
        );
        // Every source vertex receives the independently known track-1 offset;
        // particle-node transforms must not enter the mesh's pose.
        let translation = skeleton.tracks[1].definition.frames[0].translation;
        let posed = source
            .sample_authored_frames(&vec![0; skeleton.tracks.len()])
            .unwrap();
        for (part, posed) in source.parts.iter().zip(posed) {
            for (original, actual) in part.mesh.vertices.iter().zip(posed.vertices) {
                near(
                    actual,
                    std::array::from_fn(|axis| original[axis] + translation[axis]),
                );
            }
        }
        for (total, value) in totals
            .iter_mut()
            .zip([placements, parts, vertices, polygons])
        {
            *total += value;
        }
    }
    assert_eq!(totals, [366, 39, 4993, 3130]);
    let lamp = &scene.wld_object_sources["poklamp500"];
    assert_eq!(
        lamp.skeleton.as_ref().unwrap().definition.tracks[3].name,
        " PKPAR500_DAG"
    );
}
