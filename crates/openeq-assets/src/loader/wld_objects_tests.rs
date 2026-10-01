use super::*;
use crate::collision::CollisionWorld;
use crate::wld::{Frame, Track, WLD_MAGIC};

fn words(values: &[u32]) -> Vec<u8> {
    values
        .iter()
        .flat_map(|value| value.to_le_bytes())
        .collect()
}

#[derive(Default)]
struct Fixture(Vec<(u32, String, Vec<u8>)>);
impl Fixture {
    fn add(&mut self, kind: u32, name: &str, body: Vec<u8>) -> Ref {
        self.0.push((kind, name.into(), body));
        Ref(self.0.len() as i32)
    }
    fn track(&mut self, frames: &[Frame], speed: Option<u32>) -> Ref {
        let mut bytes = words(&[0, frames.len() as u32]);
        for frame in frames {
            bytes.extend(
                [frame.scale]
                    .into_iter()
                    .chain(frame.translation)
                    .chain([frame.rotation[3]])
                    .chain(frame.rotation[..3].iter().copied())
                    .flat_map(f32::to_le_bytes),
            );
        }
        let track = self.add(0x12, "", bytes);
        let mut bytes = words(&[track.0 as u32, u32::from(speed.is_some())]);
        if let Some(speed) = speed {
            bytes.extend(speed.to_le_bytes());
        }
        self.add(0x13, "", bytes)
    }
    fn mesh(&mut self, weighted: bool) -> Ref {
        let visible = self.add(0x30, "", words(&[0, 1, 0, 0, 0, 0]));
        let hidden = self.add(0x30, "", words(&[0; 6]));
        let materials = self.add(0x31, "", words(&[0, 2, visible.0 as u32, hidden.0 as u32]));
        let mut mesh = words(&[0, materials.0 as u32, 0, 0, 0]);
        mesh.extend([2.0f32, 3., 4.].into_iter().flat_map(f32::to_le_bytes));
        mesh.extend([0; 40]); // fragment metadata, bounds
        for count in [6u16, 6, 6, 0, 2, if weighted { 1 } else { 0 }, 2, 0, 0, 0] {
            mesh.extend(count.to_le_bytes());
        }
        for point in [
            [0i16, 0, 0],
            [1, 0, 0],
            [0, 1, 0],
            [0, 0, 2],
            [1, 0, 2],
            [0, 1, 2],
        ] {
            mesh.extend(point.into_iter().flat_map(i16::to_le_bytes));
        }
        mesh.extend([0; 24]); // six legacy UV pairs
        for _ in 0..6 {
            mesh.extend([127, 0, 0]);
        }
        for polygon in [[0u16, 0, 1, 2], [0, 3, 4, 5]] {
            mesh.extend(polygon.into_iter().flat_map(u16::to_le_bytes));
        }
        if weighted {
            mesh.extend([6u16, 1].into_iter().flat_map(u16::to_le_bytes));
        }
        mesh.extend([1u16, 0, 1, 1].into_iter().flat_map(u16::to_le_bytes));
        let reference = self.add(0x36, "PART_DMSPRITEDEF", mesh);
        self.add(0x2d, "", words(&[reference.0 as u32]))
    }
    fn skeleton(&mut self, tracks: &[(Ref, Ref, &[u32])], extra_meshes: &[Ref]) -> Ref {
        let mut bytes = words(&[512, tracks.len() as u32, 0]);
        for (track, mesh, children) in tracks {
            bytes.extend(words(&[
                0,
                0,
                track.0 as u32,
                mesh.0 as u32,
                children.len() as u32,
            ]));
            bytes.extend(words(children));
        }
        bytes.extend((extra_meshes.len() as u32).to_le_bytes());
        bytes.extend(
            extra_meshes
                .iter()
                .flat_map(|reference| reference.0.to_le_bytes()),
        );
        let reference = self.add(0x10, "TREE_HS_DEF", bytes);
        self.add(0x11, "", words(&[reference.0 as u32]))
    }
    fn actor(&mut self, name: &str, reference: Ref) {
        self.add(0x14, name, words(&[0, 0, 0, 1, 0, reference.0 as u32]));
    }
    fn finish(self) -> Wld {
        let mut strings = vec![0u8];
        let names: Vec<_> = self
            .0
            .iter()
            .map(|(_, name, _)| {
                let offset = strings.len() as i32;
                strings.extend(name.as_bytes());
                strings.push(0);
                -offset
            })
            .collect();
        let mut bytes = words(&[
            WLD_MAGIC,
            0x0001_5500,
            self.0.len() as u32,
            0,
            0,
            strings.len() as u32,
            0,
        ]);
        let key = [0x95, 0x3a, 0xc5, 0x2a, 0x95, 0x7a, 0x95, 0x6a];
        bytes.extend(
            strings
                .iter()
                .enumerate()
                .map(|(i, byte)| byte ^ key[i % 8]),
        );
        while !bytes.len().is_multiple_of(4) {
            bytes.push(0);
        }
        for ((kind, _, body), name) in self.0.into_iter().zip(names) {
            bytes.extend(words(&[body.len() as u32 + 4, kind]));
            bytes.extend(name.to_le_bytes());
            bytes.extend(body);
        }
        Wld::parse("fixture_obj.wld".into(), &bytes).unwrap()
    }
}

fn frame(translation: [f32; 3], scale: f32) -> Frame {
    Frame {
        rotation: [0., 0., 0., 1.],
        translation,
        scale,
    }
}
fn tree_fixture(weighted: bool) -> Wld {
    let mut fixture = Fixture::default();
    let mesh = fixture.mesh(weighted);
    let root = fixture.track(
        &[Frame {
            rotation: [0., 0., 1., 1.],
            ..frame([10., 20., 30.], 2.)
        }],
        None,
    );
    let child = fixture.track(
        &[frame([1., 2., 3.], 3.), frame([4., 5., 6.], 2.)],
        Some(1000),
    );
    let skeleton = fixture.skeleton(&[(root, Ref(0), &[1]), (child, mesh, &[])], &[mesh]);
    fixture.actor("TREE_ACTORDEF", skeleton);
    fixture.actor("SECOND_ACTORDEF", skeleton);
    if !weighted {
        fixture.actor("STATIC_ACTORDEF", mesh);
    }
    fixture.finish()
}
fn source(wld: &Wld, name: &str) -> ObjectSource {
    let chunk = wld.by_name(name).unwrap();
    let Fragment::ActorDef(actor) = &chunk.fragment else {
        panic!("actor");
    };
    actor_source(wld, chunk, actor).unwrap()
}
fn near(actual: [f32; 3], expected: [f32; 3]) {
    assert!(
        Vec3::from_array(actual).distance(Vec3::from_array(expected)) < 0.0002,
        "{actual:?} != {expected:?}"
    );
}
fn empty_scene() -> Scene {
    Scene::from_geometry("fixture".into(), vec![], vec![], vec![])
}

fn triangles(geometry: &mesh::Geometry) -> Vec<Vec<u32>> {
    let mut triangles: Vec<_> = geometry
        .indices
        .chunks_exact(3)
        .map(|indices| {
            indices
                .iter()
                .flat_map(|index| {
                    geometry.vertices[*index as usize * 8..][..8]
                        .iter()
                        .map(|value| value.to_bits())
                })
                .collect()
        })
        .collect();
    // The existing material baker can merge runs in hash iteration order.
    // Compare authored triangles and attributes rather than buffer ordering.
    triangles.sort_unstable();
    triangles
}

#[test]
fn rigid_child_pose_uses_parent_rotation_scale_and_center_once() {
    let wld = tree_fixture(false);
    let source = source(&wld, "TREE_ACTORDEF");
    assert_eq!(source.parts.len(), 1);
    assert_eq!(source.parts[0].rigid_track, Some(1));
    near(source.parts[0].mesh.vertices[0], [2., 3., 4.]);
    let skeleton = source.skeleton.as_ref().unwrap();
    assert_eq!(skeleton.parents, [None, Some(0)]);
    assert_eq!(skeleton.tracks[1].speed, Some(1000));
    assert_eq!(skeleton.tracks[1].definition.frames.len(), 2);
    near(
        skeleton.tracks[1].definition.frames[1].translation,
        [4., 5., 6.],
    );
    let posed = posed_meshes(&source).unwrap();
    // Child: 3*(2,3,4)+(1,2,3)=(7,11,15).
    // Root: 90°Z * 2*(7,11,15)+(10,20,30)=(-12,34,60).
    near(posed[0].vertices[0], [-12., 34., 60.]);
    near(posed[0].normals[0], [0., 1., 0.]);
    // The hidden collision triangle is transformed by precisely the same pose.
    near(posed[0].vertices[3], [-12., 34., 72.]);
    let collisions = mesh::bake_wld_collision_meshes(&wld, &posed);
    near(collisions[0].positions[0], [-12., 34., 72.]);
}

#[test]
fn track_reference_flags_and_optional_timing_are_retained_verbatim() {
    for (flags, speed) in [
        (0, None),
        (4, None),
        (5, Some(1000)),
        (7, Some(0)),
        (0x8000_0004, None),
    ] {
        let mut fixture = Fixture::default();
        let mut bytes = words(&[17, flags]);
        if let Some(speed) = speed {
            bytes.extend(words(&[speed]));
        }
        fixture.add(0x13, "TRACK", bytes);
        let wld = fixture.finish();
        let Fragment::PieceTrackRef(reference) = &wld.chunks()[0].fragment else {
            panic!("track reference");
        };
        assert_eq!(reference.track, Ref(17));
        assert_eq!(reference.flags, flags);
        assert_eq!(reference.speed, speed);
    }
}

#[test]
fn explicit_frames_preserve_sources_topology_and_actor_ownership() {
    for weighted in [false, true] {
        let wld = tree_fixture(weighted);
        let mut scene = empty_scene();
        append_objects(&mut scene, 0, &wld).unwrap();
        let source = &scene.wld_object_sources["tree"];
        let first = source.sample_authored_frames(&[0, 0]).unwrap();
        let later = source.sample_authored_frames(&[0, 1]).unwrap();
        near(first[0].vertices[0], [-12., 34., 60.]);
        // Child: 2*(2,3,4)+(4,5,6)=(8,11,14).
        // Root: 90°Z * 2*(8,11,14)+(10,20,30)=(-12,36,58).
        near(later[0].vertices[0], [-12., 36., 58.]);
        near(later[0].normals[0], [0., 1., 0.]);
        assert_eq!(later[0].tex_coords, first[0].tex_coords);
        assert_eq!(later[0].polygon_textures, first[0].polygon_textures);
        assert_eq!(later[0].materials, first[0].materials);
        assert_eq!(later[0].vertex_pieces, first[0].vertex_pieces);
        let topology = |meshes: &[Mesh]| {
            meshes
                .iter()
                .flat_map(|mesh| mesh.polygons.iter().map(|p| (p.a, p.b, p.c, p.collidable)))
                .collect::<Vec<_>>()
        };
        assert_eq!(topology(&first), topology(&later));
        near(source.parts[0].mesh.vertices[0], [2., 3., 4.]);
        assert_eq!(posed_meshes(source).unwrap()[0].vertices, first[0].vertices);
        assert_eq!(
            posed_meshes(&scene.wld_object_sources["second"]).unwrap()[0].vertices,
            first[0].vertices
        );
        // Diagnostic samples do not modify any actor's existing scene bake.
        for key in ["tree", "second"] {
            let object = scene
                .objects
                .iter()
                .find(|object| object.name == key)
                .unwrap();
            near(
                scene.meshes[object.meshes[0]].vertices[..3]
                    .try_into()
                    .unwrap(),
                [-12., 34., 60.],
            );
            near(
                scene.collision_meshes[object.collision_meshes[0]].positions[0],
                [-12., 34., 72.],
            );
        }
    }
}

#[test]
fn explicit_frames_reject_bad_selections_and_later_poses_without_changing_first_pose() {
    let wld = tree_fixture(false);
    let mut source = source(&wld, "TREE_ACTORDEF");
    let first = posed_meshes(&source).unwrap();
    for indices in [
        vec![],
        vec![0],
        vec![0, 0, 0],
        vec![1, 0],
        vec![0, 2],
        vec![0, usize::MAX],
    ] {
        assert!(source.sample_authored_frames(&indices).is_err());
    }
    for bad in [
        frame([f32::NAN, 0., 0.], 1.),
        frame([0.; 3], 0.),
        Frame {
            rotation: [0.; 4],
            ..frame([0.; 3], 1.)
        },
    ] {
        source.skeleton.as_mut().unwrap().tracks[1]
            .definition
            .frames[1] = bad;
        assert!(source.sample_authored_frames(&[0, 1]).is_err());
        assert_eq!(
            posed_meshes(&source).unwrap()[0].vertices,
            first[0].vertices
        );
    }
    let static_source = self::source(&wld, "STATIC_ACTORDEF");
    assert_eq!(
        static_source.sample_authored_frames(&[]).unwrap()[0].vertices,
        static_source.parts[0].mesh.vertices
    );
    assert!(static_source.sample_authored_frames(&[0]).is_err());
}

#[test]
fn actor_names_and_shared_source_meshes_have_independent_draw_collision_ownership() {
    let wld = tree_fixture(false);
    let mut scene = empty_scene();
    append_objects(&mut scene, 0, &wld).unwrap();
    assert_eq!(
        scene
            .objects
            .iter()
            .map(|object| object.name.as_str())
            .collect::<Vec<_>>(),
        ["tree", "second", "static", "part"]
    );
    assert_eq!(scene.wld_object_sources.len(), 3);
    let mut render = BTreeSet::new();
    let mut collision = BTreeSet::new();
    for object in &scene.objects {
        assert_eq!(object.meshes.len(), 1);
        assert_eq!(object.collision_meshes.len(), 1);
        assert!(object.meshes.iter().all(|index| render.insert(*index)));
        assert!(
            object
                .collision_meshes
                .iter()
                .all(|index| collision.insert(*index))
        );
    }
    assert_eq!(
        CollisionWorld::build(&scene).triangle_count(),
        0,
        "unplaced definitions never become terrain"
    );
    for name in ["tree", "second"] {
        scene.instances.push(super::super::Instance {
            object: name.into(),
            position: [0.; 3],
            scale: [1.; 3],
            rotation: [0., 0., 0., 1.],
        });
    }
    assert_eq!(CollisionWorld::build(&scene).triangle_count(), 4);
    let static_object = scene
        .objects
        .iter()
        .find(|object| object.name == "static")
        .unwrap();
    near(
        scene.meshes[static_object.meshes[0]].vertices[..3]
            .try_into()
            .unwrap(),
        [2., 3., 4.],
    );
    assert_eq!(
        scene
            .object_model("PART_DMSPRITEDEF")
            .unwrap()
            .triangle_count(),
        1
    );
    let extracted = scene.object_model("TREE_ACTORDEF").unwrap();
    assert_eq!(extracted.wld_object_sources["tree"].parts.len(), 1);
    assert_eq!(CollisionWorld::build(&extracted).triangle_count(), 2);
}

#[test]
fn weighted_parts_use_their_runs_and_reject_incomplete_or_invalid_bindings() {
    let wld = tree_fixture(true);
    let mut source = source(&wld, "TREE_ACTORDEF");
    near(
        posed_meshes(&source).unwrap()[0].vertices[0],
        [-12., 34., 60.],
    );
    source.parts[0].mesh.vertex_pieces = vec![(3, 0), (3, 1)];
    let posed = posed_meshes(&source).unwrap();
    near(posed[0].vertices[0], [4., 24., 38.]);
    near(posed[0].vertices[3], [-12., 34., 72.]);
    for runs in [vec![(5, 1)], vec![(7, 1)], vec![(6, 2)]] {
        source.parts[0].mesh.vertex_pieces = runs;
        assert!(posed_meshes(&source).is_err());
    }
}

#[test]
fn repeated_rigid_mesh_references_are_distinct_parts_not_deduplicated() {
    let mut fixture = Fixture::default();
    let mesh = fixture.mesh(false);
    let a = fixture.track(&[frame([0., 0., 0.], 1.)], None);
    let b = fixture.track(&[frame([10., 0., 0.], 1.)], None);
    let skeleton = fixture.skeleton(&[(a, mesh, &[]), (b, mesh, &[])], &[mesh]);
    fixture.actor("TWIN_ACTORDEF", skeleton);
    let wld = fixture.finish();
    let source = source(&wld, "TWIN_ACTORDEF");
    assert_eq!(source.parts.len(), 2);
    let posed = posed_meshes(&source).unwrap();
    near(posed[0].vertices[0], [2., 3., 4.]);
    near(posed[1].vertices[0], [12., 3., 4.]);
}

#[test]
fn unsupported_actor_does_not_remove_independent_static_meshes() {
    let mut fixture = Fixture::default();
    let mesh = fixture.mesh(false);
    let track = fixture.track(&[frame([0.; 3], 1.)], None);
    let unsupported = fixture.add(0x34, "", vec![]);
    let skeleton = fixture.skeleton(&[(track, unsupported, &[])], &[mesh]);
    fixture.actor("UNKNOWN_ACTORDEF", skeleton);
    let wld = fixture.finish();
    let chunk = wld.by_name("UNKNOWN_ACTORDEF").unwrap();
    let Fragment::ActorDef(actor) = &chunk.fragment else {
        panic!("actor");
    };
    assert!(
        actor_source(&wld, chunk, actor)
            .unwrap_err()
            .to_string()
            .contains("0x34")
    );
    let mut scene = empty_scene();
    append_objects(&mut scene, 0, &wld).unwrap();
    assert_eq!(scene.objects.len(), 1);
    assert_eq!(scene.objects[0].name, "part");
    assert!(scene.wld_object_sources.is_empty());
}

#[test]
fn hierarchy_and_first_pose_reject_cycles_shared_children_and_degenerate_frames() {
    let track = |children| Track {
        name: String::new(),
        flags: 0,
        piece_track: Ref(0),
        mesh: Ref(0),
        children,
    };
    for tracks in [
        vec![track(vec![1]), track(vec![0])],
        vec![track(vec![2]), track(vec![2]), track(vec![])],
        vec![track(vec![-1])],
        vec![track(vec![1])],
        vec![track(vec![]); MAX_TRACKS + 1],
    ] {
        assert!(
            hierarchy(&Skeleton {
                tracks,
                meshes: vec![]
            })
            .is_err()
        );
    }
    let wld = tree_fixture(false);
    let source = source(&wld, "TREE_ACTORDEF");
    let mut skeleton = source.skeleton.unwrap();
    for bad in [
        frame([f32::NAN, 0., 0.], 1.),
        frame([0.; 3], 0.),
        Frame {
            rotation: [0.; 4],
            ..frame([0.; 3], 1.)
        },
        Frame {
            rotation: [f32::MAX; 4],
            ..frame([0.; 3], 1.)
        },
    ] {
        skeleton.tracks[0].definition.frames[0] = bad;
        assert!(first_pose(&skeleton).is_err());
    }
}

#[test]
#[ignore = "requires original City of Mist archives in EQ_DIR; CPU geometry only"]
fn original_citymist_trees_keep_authored_parts_frames_and_trunk_collision() {
    let base = std::env::var_os("EQ_DIR").expect("set EQ_DIR");
    let scene = super::super::load_zone(&base, "citymist").unwrap();
    assert_eq!(
        scene
            .instances
            .iter()
            .filter(|instance| instance.object == "jntree103")
            .count(),
        50
    );
    let source = &scene.wld_object_sources["jntree103"];
    let skeleton = source.skeleton.as_ref().unwrap();
    assert_eq!(skeleton.tracks.len(), 8);
    assert_eq!(source.parts.len(), 7);
    assert_eq!(
        source
            .parts
            .iter()
            .map(|part| part.mesh.vertices.len())
            .sum::<usize>(),
        106
    );
    assert_eq!(
        source
            .parts
            .iter()
            .map(|part| part.mesh.polygons.len())
            .sum::<usize>(),
        72
    );
    assert_eq!(
        skeleton
            .tracks
            .iter()
            .filter(|track| track.definition.frames.len() == 4 && track.speed == Some(1000))
            .count(),
        6
    );
    for track in &skeleton.tracks {
        assert_eq!(track.definition.flags, 8);
        assert_eq!(
            track.reference_flags,
            if track.speed.is_some() { 5 } else { 4 }
        );
    }
    let posed = posed_meshes(source).unwrap();
    let trunk = source
        .parts
        .iter()
        .position(|part| part.name == "JNT3TNK_DMSPRITEDEF")
        .unwrap();
    assert_eq!(source.parts[trunk].rigid_track, Some(7));
    assert_eq!(
        source.parts[trunk]
            .mesh
            .polygons
            .iter()
            .filter(|polygon| polygon.collidable)
            .count(),
        42
    );
    for (original, actual) in source.parts[trunk]
        .mesh
        .vertices
        .iter()
        .zip(&posed[trunk].vertices)
    {
        near(
            *actual,
            [
                original[0] * 0.83984375,
                original[1] * 0.83984375 + 0.3671875,
                original[2] * 0.83984375,
            ],
        );
    }
    let branch = source
        .parts
        .iter()
        .position(|part| part.name == "JNT3BR3_DMSPRITEDEF")
        .unwrap();
    for (original, actual) in source.parts[branch]
        .mesh
        .vertices
        .iter()
        .zip(&posed[branch].vertices)
    {
        near(
            *actual,
            [
                original[0] + 0.125,
                original[1] - 0.421875,
                original[2] + 85.53125,
            ],
        );
    }
    // All four exact authored-frame snapshots are available for comparing with
    // the native runtime. This deliberately makes no playback-time assertion.
    for frame_index in 0..4 {
        let selection = skeleton
            .tracks
            .iter()
            .map(|track| {
                if track.definition.frames.len() == 1 {
                    0
                } else {
                    frame_index
                }
            })
            .collect::<Vec<_>>();
        let sampled = source.sample_authored_frames(&selection).unwrap();
        assert_eq!(sampled[trunk].vertices, posed[trunk].vertices);
        assert_eq!(sampled[trunk].normals, posed[trunk].normals);
        assert_eq!(
            sampled
                .iter()
                .map(|mesh| mesh.polygons.len())
                .sum::<usize>(),
            72
        );
        for (index, mesh) in sampled
            .iter()
            .enumerate()
            .filter(|(index, _)| *index != trunk)
        {
            assert!(mesh.polygons.iter().all(|polygon| !polygon.collidable));
            if frame_index == 0 {
                assert_eq!(mesh.vertices, posed[index].vertices);
            } else {
                assert_ne!(mesh.vertices, posed[index].vertices);
            }
        }
    }
    // Independently rotate one original branch from its packed quaternion words.
    let branch = source
        .parts
        .iter()
        .position(|part| part.name == "JNT3BR1_DMSPRITEDEF")
        .unwrap();
    let raw_q = [1619.0f64, 1639., -11540., -11399.];
    let length = raw_q.iter().map(|value| value * value).sum::<f64>().sqrt();
    let q = raw_q.map(|value| value / length);
    let cross = |a: [f64; 3], b: [f64; 3]| {
        [
            a[1] * b[2] - a[2] * b[1],
            a[2] * b[0] - a[0] * b[2],
            a[0] * b[1] - a[1] * b[0],
        ]
    };
    for (original, actual) in source.parts[branch]
        .mesh
        .vertices
        .iter()
        .zip(&posed[branch].vertices)
    {
        let p = original.map(f64::from);
        let uv = cross([q[0], q[1], q[2]], p);
        let uuv = cross([q[0], q[1], q[2]], uv);
        let translation = [6.21875, -1.80078125, 82.98828125];
        near(
            *actual,
            std::array::from_fn(|i| {
                (p[i] + 2. * q[3] * uv[i] + 2. * uuv[i] + translation[i]) as f32
            }),
        );
    }
    // Every baked visible triangle belongs to this actor, and none of its raw
    // component mesh names is substituted for the actor's placement key.
    let object = scene
        .objects
        .iter()
        .find(|object| object.name == "jntree103")
        .unwrap();
    assert_eq!(
        object
            .meshes
            .iter()
            .map(|index| scene.meshes[*index].indices.len() / 3)
            .sum::<usize>(),
        72
    );
    assert_eq!(
        object
            .meshes
            .iter()
            .filter(|index| scene.meshes[**index].collidable)
            .map(|index| scene.meshes[*index].indices.len() / 3)
            .sum::<usize>(),
        42
    );
    assert_eq!(object.collision_meshes.len(), 0);
    let mut isolated = scene.object_model("jntree103").unwrap();
    assert_eq!(CollisionWorld::build(&isolated).triangle_count(), 42);
    isolated.objects.push(SceneObject {
        name: "jntree103".into(),
        meshes: (0..isolated.meshes.len()).collect(),
        collision_meshes: vec![],
    });
    isolated.instances = scene
        .instances
        .iter()
        .filter(|instance| instance.object == "jntree103")
        .cloned()
        .collect();
    assert_eq!(CollisionWorld::build(&isolated).triangle_count(), 50 * 42);
    let library = super::super::load_object_library(&base, "citymist").unwrap();
    assert_eq!(library.wld_object_sources["jntree103"].parts.len(), 7);
}

#[test]
#[ignore = "requires original global/GFay/PoK object archives in EQ_DIR; CPU geometry only"]
fn original_static_objects_keep_raw_lookups_geometry_and_unique_ownership() {
    let base = std::path::PathBuf::from(std::env::var_os("EQ_DIR").expect("set EQ_DIR"));
    for (name, expected_raw) in [
        ("global_obj", 1),
        ("gfaydark_obj", 84),
        ("poknowledge_obj", 131),
    ] {
        let archive = crate::pfs::Archive::open(base.join(format!("{name}.s3d"))).unwrap();
        let wlds: Vec<_> = archive
            .names()
            .iter()
            .filter(|name| name.ends_with(".wld"))
            .map(|name| Wld::open(&archive, name).unwrap())
            .collect();
        let mut scene = empty_scene();
        scene.archives.push(archive);
        for wld in &wlds {
            append_objects(&mut scene, 0, wld).unwrap();
        }
        let mut raw_names = BTreeSet::new();
        let mut checked = 0;
        let mut duplicate_count = 0;
        let mut duplicate_triangles = 0;
        for wld in &wlds {
            for (chunk, raw) in wld.iter::<Mesh>() {
                let key = object_key(&chunk.name);
                if key.is_empty() {
                    continue;
                }
                if !raw_names.insert(key.clone()) {
                    duplicate_count += 1;
                    duplicate_triangles += mesh::bake_wld_meshes(wld, [raw])
                        .1
                        .iter()
                        .map(|geometry| geometry.indices.len() / 3)
                        .sum::<usize>();
                    continue;
                }
                let object = scene
                    .objects
                    .iter()
                    .find(|object| object.name == key)
                    .unwrap_or_else(|| panic!("{name} lost historical raw lookup {key}"));
                if scene
                    .wld_object_sources
                    .get(&key)
                    .is_some_and(|source| source.skeleton.is_some())
                {
                    continue; // Corrected skeletal actor pose supersedes its same-name raw mesh.
                }
                let (materials, expected) = mesh::bake_wld_meshes(wld, [raw]);
                assert_eq!(object.meshes.len(), expected.len(), "{name}/{key}");
                for (&index, expected) in object.meshes.iter().zip(expected) {
                    let actual = &scene.meshes[index];
                    assert!(
                        triangles(actual) == triangles(&expected),
                        "{name}/{key} geometry changed"
                    );
                    assert_eq!(actual.collidable, expected.collidable, "{name}/{key}");
                    assert_eq!(
                        scene.materials[actual.material],
                        materials[expected.material]
                    );
                }
                let expected = mesh::bake_wld_collision_meshes(wld, [raw]);
                assert_eq!(
                    object.collision_meshes.len(),
                    expected.len(),
                    "{name}/{key}"
                );
                for (&index, expected) in object.collision_meshes.iter().zip(expected) {
                    assert_eq!(scene.collision_meshes[index].positions, expected.positions);
                    assert_eq!(scene.collision_meshes[index].indices, expected.indices);
                }
                checked += 1;
            }
        }
        let mut render = BTreeSet::new();
        let mut collision = BTreeSet::new();
        for object in &scene.objects {
            assert!(
                object.meshes.iter().all(|index| render.insert(*index)),
                "{name}"
            );
            assert!(
                object
                    .collision_meshes
                    .iter()
                    .all(|index| collision.insert(*index)),
                "{name}"
            );
        }
        assert_eq!(render.len(), scene.meshes.len());
        assert_eq!(collision.len(), scene.collision_meshes.len());
        assert_eq!(CollisionWorld::build(&scene).triangle_count(), 0);
        assert!(checked > 0);
        assert_eq!(
            (raw_names.len(), checked, scene.objects.len()),
            (expected_raw, expected_raw, expected_raw)
        );
        if name == "poknowledge_obj" {
            // The previous loader retained these later duplicate definitions,
            // but both renderer and collision always select the first name.
            // Removing their dead copies changes the inventory by exactly 984.
            assert_eq!((duplicate_count, duplicate_triangles), (18, 984));
        }
        eprintln!(
            "{name}: {} raw lookups retained, {checked} raw poses unchanged, {} total objects",
            raw_names.len(),
            scene.objects.len()
        );
    }
    let gfay = super::super::load_object_library(&base, "gfaydark").unwrap();
    for name in ["FAYLEVATOR", "FELE2"] {
        assert!(
            gfay.object_model(name).unwrap().triangle_count() > 0,
            "{name}"
        );
    }
}
