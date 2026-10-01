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

fn short_animation_source(weighted: bool) -> ObjectSource {
    let mut source = source(&tree_fixture(weighted), "TREE_ACTORDEF");
    let skeleton = source.skeleton.as_mut().unwrap();
    for track in &mut skeleton.tracks {
        track.definition.flags = 8;
        track.reference_flags = if track.speed.is_some() { 5 } else { 4 };
    }
    let child = &mut skeleton.tracks[1].definition.frames;
    child[1] = Frame {
        // Packed approximation to a 120-degree Z rotation. Keep the source
        // magnitudes so this fixture distinguishes nlerp from slerp.
        rotation: [0., 0., 14189. / 16384., 0.5],
        ..child[0]
    };
    source
}

#[test]
fn timed_frames_close_the_loop_and_preserve_sources_and_bindings() {
    for weighted in [false, true] {
        let source = short_animation_source(weighted);
        let first = source.sample_authored_frames(&[0, 0]).unwrap();
        assert_eq!(source.animation_period().unwrap(), Duration::from_secs(2));
        let start = source.sample_animation(Duration::ZERO).unwrap();
        assert_eq!(start[0].vertices, first[0].vertices);
        let end = source.sample_animation(Duration::from_secs(2)).unwrap();
        assert_eq!(start[0].vertices, end[0].vertices);
        let middle = source
            .sample_animation(Duration::from_millis(1000))
            .unwrap();
        let authored = source.sample_authored_frames(&[0, 1]).unwrap();
        for (actual, expected) in middle[0].vertices.iter().zip(&authored[0].vertices) {
            near(*actual, *expected);
        }
        let opening = source.sample_animation(Duration::from_millis(500)).unwrap();
        let closing = source
            .sample_animation(Duration::from_millis(1500))
            .unwrap();
        for (actual, expected) in opening[0].vertices.iter().zip(&closing[0].vertices) {
            near(*actual, *expected);
        }
        assert_ne!(opening[0].vertices, start[0].vertices);
        for elapsed in [Duration::from_millis(8500), Duration::from_micros(500999)] {
            assert_eq!(
                source.sample_animation(elapsed).unwrap()[0].vertices,
                opening[0].vertices
            );
        }
        assert_eq!(opening[0].tex_coords, source.parts[0].mesh.tex_coords);
        assert_eq!(opening[0].materials, source.parts[0].mesh.materials);
        assert_eq!(opening[0].vertex_pieces, source.parts[0].mesh.vertex_pieces);
        assert!(
            opening[0]
                .polygons
                .iter()
                .zip(&first[0].polygons)
                .all(|(a, b)| { (a.a, a.b, a.c, a.collidable) == (b.a, b.b, b.c, b.collidable) })
        );
        near(source.parts[0].mesh.vertices[0], [2., 3., 4.]);
        assert_eq!(
            posed_meshes(&source).unwrap()[0].vertices,
            first[0].vertices
        );
    }
}

#[test]
fn timed_frames_use_native_nlerp_and_quaternion_sign_continuity() {
    let source = short_animation_source(false);
    let mut opposite_sign = source.clone();
    opposite_sign.skeleton.as_mut().unwrap().tracks[1]
        .definition
        .frames[1]
        .rotation = source.skeleton.as_ref().unwrap().tracks[1]
        .definition
        .frames[1]
        .rotation
        .map(|value| -value);
    for milliseconds in [0, 250, 500, 999, 1000, 1250, 1750, 1999, 2000] {
        let time = Duration::from_millis(milliseconds);
        assert_eq!(
            source.sample_animation(time).unwrap()[0].vertices,
            opposite_sign.sample_animation(time).unwrap()[0].vertices
        );
    }
    // At 25% of the first interval, nlerp's angle is about 27.8 degrees,
    // whereas slerp would rotate by 30 degrees. Parent is 90 degrees Z.
    let actual = source.sample_animation(Duration::from_millis(250)).unwrap();
    let child_angle = 2. * ((14189.0f64 / 16384. * 0.25) / 0.875).atan();
    let (s, c) = child_angle.sin_cos();
    let child_x = 6. * c - 9. * s + 1.;
    let child_y = 6. * s + 9. * c + 2.;
    near(
        actual[0].vertices[0],
        [
            (10. - 2. * child_y) as f32,
            (20. + 2. * child_x) as f32,
            60.,
        ],
    );
}

#[test]
fn timed_frames_share_one_phase_and_reject_unproven_timelines() {
    let mut source = short_animation_source(false);
    let skeleton = source.skeleton.as_mut().unwrap();
    let root_frame = skeleton.tracks[0].definition.frames[0];
    skeleton.tracks[0].definition.frames.push(root_frame);
    skeleton.tracks[0].reference_flags = 5;
    skeleton.tracks[0].speed = Some(1000);
    assert_eq!(source.animation_period().unwrap(), Duration::from_secs(2));
    // A constant animated parent still participates in the shared set period.
    let duplicate = source.clone();
    assert_eq!(
        source.sample_animation(Duration::from_millis(700)).unwrap()[0].vertices,
        duplicate
            .sample_animation(Duration::from_millis(2700))
            .unwrap()[0]
            .vertices
    );
    source.skeleton.as_mut().unwrap().tracks[0].speed = Some(500);
    assert!(source.animation_period().is_err());
    assert!(source.sample_animation(Duration::ZERO).is_err());
    source.skeleton.as_mut().unwrap().tracks[0].speed = Some(1000);
    source.skeleton.as_mut().unwrap().tracks[0]
        .definition
        .frames
        .push(root_frame);
    assert!(source.animation_period().is_err());
}

#[test]
fn timed_frames_reject_unsupported_or_invalid_tracks_without_affecting_first_pose() {
    let source = short_animation_source(false);
    let first = posed_meshes(&source).unwrap();
    let mut mutations: Vec<ObjectSource> = vec![];
    for (flags, speed) in [
        (1, Some(1000)),
        (7, Some(1000)),
        (5, None),
        (5, Some(0)),
        (5, Some(u32::MAX)),
    ] {
        let mut bad = source.clone();
        let track = &mut bad.skeleton.as_mut().unwrap().tracks[1];
        track.reference_flags = flags;
        track.speed = speed;
        mutations.push(bad);
    }
    for flags in [0, 9] {
        let mut bad = source.clone();
        bad.skeleton.as_mut().unwrap().tracks[1].definition.flags = flags;
        mutations.push(bad);
    }
    for frame in [
        frame([f32::NAN, 0., 0.], 3.),
        frame([1., 2., 3.], 4.),
        frame([1., 2., 4.], 3.),
        Frame {
            rotation: [0.; 4],
            ..frame([1., 2., 3.], 3.)
        },
        Frame {
            rotation: [0., 0., 1., 0.],
            ..frame([1., 2., 3.], 3.)
        },
    ] {
        let mut bad = source.clone();
        bad.skeleton.as_mut().unwrap().tracks[1].definition.frames[1] = frame;
        mutations.push(bad);
    }
    let mut long = source.clone();
    let frames = &mut long.skeleton.as_mut().unwrap().tracks[1].definition.frames;
    frames.resize(5, frames[0]);
    mutations.push(long);
    let mut static_flags = source.clone();
    static_flags.skeleton.as_mut().unwrap().tracks[0].reference_flags = 5;
    mutations.push(static_flags);
    for bad in mutations {
        assert!(bad.animation_period().is_err());
        assert!(bad.sample_animation(Duration::ZERO).is_err());
        assert_eq!(posed_meshes(&bad).unwrap()[0].vertices, first[0].vertices);
    }
    let static_source = self::source(&tree_fixture(false), "STATIC_ACTORDEF");
    assert!(static_source.animation_period().is_err());
    assert!(static_source.sample_animation(Duration::ZERO).is_err());
}

#[test]
fn timed_br1_matches_original_d3dx_scalar_output_witness() {
    let mut source = short_animation_source(false);
    let skeleton = source.skeleton.as_mut().unwrap();
    skeleton.tracks[0].definition.frames[0] = frame([0.; 3], 1.);
    skeleton.tracks[1].definition.frames = [
        [-11399., 1619., 1639., -11540.],
        [-11418., 1479., 1498., -11559.],
        [-11399., 1622., 1216., -11592.],
        [-11388., 1692., 1287., -11584.],
    ]
    .into_iter()
    .map(|[w, x, y, z]| Frame {
        rotation: [x / 16384., y / 16384., z / 16384., w / 16384.],
        ..frame([1592. / 256., -461. / 256., 21245. / 256.], 1.)
    })
    .collect();
    // Native GetSRT at 500 ms, then controller-equivalent normalization and
    // D3DXMatrixRotationQuaternion. Exact binary provenance is in the research
    // document. D3DX rows flatten to glam columns for the same point transform.
    let native = Mat4::from_cols_array(&[
        -0.012286968,
        0.9999245,
        0.000030211837,
        0.,
        -0.9637163,
        -0.011833985,
        -0.26666594,
        0.,
        -0.26664546,
        -0.0033056452,
        0.963789,
        0.,
        6.21875,
        -461. / 256.,
        82.98828,
        1.,
    ]);
    let sampled = source.sample_animation(Duration::from_millis(500)).unwrap();
    for (original, actual) in source.parts[0]
        .mesh
        .vertices
        .iter()
        .zip(&sampled[0].vertices)
    {
        near(
            *actual,
            native
                .transform_point3(Vec3::from_array(*original))
                .to_array(),
        );
    }
}

#[test]
fn live_animation_rejects_static_collision_vertices_under_an_animated_ancestor() {
    let mut source = short_animation_source(true);
    let skeleton = source.skeleton.as_mut().unwrap();
    let root = &mut skeleton.tracks[0];
    root.definition.frames.push(Frame {
        rotation: Quat::from_rotation_x(0.8).to_array(),
        ..root.definition.frames[0]
    });
    root.reference_flags = 5;
    root.speed = Some(1000);
    let child = &mut skeleton.tracks[1];
    child.definition.frames.truncate(1);
    child.reference_flags = 4;
    child.speed = None;
    // Every vertex belongs to the locally static child, but its parent moves.
    assert_eq!(source.parts[0].mesh.vertex_pieces, [(6, 1)]);
    assert_eq!(source.animation_period().unwrap(), Duration::from_secs(2));
    assert_ne!(
        source.sample_animation(Duration::ZERO).unwrap()[0].vertices,
        source
            .sample_animation(Duration::from_millis(1000))
            .unwrap()[0]
            .vertices
    );
    assert!(
        source
            .stationary_collision_animation_radius()
            .unwrap_err()
            .to_string()
            .contains("moves collision ancestry")
    );
}

#[test]
fn live_animation_collision_gate_includes_invisible_faces() {
    let wld = tree_fixture(true);
    let mut source = short_animation_source(true);
    let part = &mut source.parts[0];
    part.mesh.vertex_pieces = vec![(3, 0), (3, 1)];
    part.mesh.polygons[0].collidable = false;
    // The visible triangle is attached to the static root. Only the invisible
    // material's triangle is collidable, and it follows the animated child.
    let initial = source.sample_animation(Duration::ZERO).unwrap();
    let (_, drawable) = mesh::bake_wld_meshes(&wld, &initial);
    let hidden = mesh::bake_wld_collision_meshes(&wld, &initial);
    assert_eq!(drawable.len(), 1);
    assert!(!drawable[0].collidable);
    assert_eq!(drawable[0].indices.len(), 3);
    assert_eq!(hidden.len(), 1);
    assert_eq!(hidden[0].indices.len(), 3);
    assert!(
        source
            .stationary_collision_animation_radius()
            .unwrap_err()
            .to_string()
            .contains("moves collision ancestry")
    );
    // Removing that physical triangle is sufficient to admit the animation;
    // visibility was never the criterion for the rejected model.
    source.parts[0].mesh.polygons[1].collidable = false;
    assert!(source.stationary_collision_animation_radius().is_ok());
}

#[test]
fn live_animation_without_collision_has_bounds_for_nested_intermediate_poses() {
    for weighted in [false, true] {
        let mut source = short_animation_source(weighted);
        for polygon in &mut source.parts[0].mesh.polygons {
            polygon.collidable = false;
        }
        let root = &mut source.skeleton.as_mut().unwrap().tracks[0];
        root.definition.frames.push(Frame {
            rotation: Quat::from_rotation_y(1.3).to_array(),
            ..root.definition.frames[0]
        });
        root.reference_flags = 5;
        root.speed = Some(1000);
        let radius = source.stationary_collision_animation_radius().unwrap();
        assert!(radius.is_finite() && radius > 0.);
        let initial = source.sample_animation(Duration::ZERO).unwrap();
        let mut changed = false;
        // These are containment witnesses, including fractional-key poses and
        // closure; the production radius comes from an analytic ancestry bound.
        for milliseconds in [0, 1, 137, 250, 503, 777, 999, 1000, 1251, 1500, 1999, 2000] {
            let sampled = source
                .sample_animation(Duration::from_millis(milliseconds))
                .unwrap();
            changed |= sampled[0].vertices != initial[0].vertices;
            for mesh in sampled {
                for point in mesh.vertices {
                    let length = point
                        .into_iter()
                        .map(|v| f64::from(v).powi(2))
                        .sum::<f64>()
                        .sqrt();
                    assert!(
                        length <= f64::from(radius),
                        "{point:?} at {milliseconds} ms exceeds radius {radius}"
                    );
                }
            }
        }
        assert!(changed);
    }
}

#[test]
fn live_animation_bindings_keep_coincident_source_vertices_distinct_across_poses() {
    let wld = tree_fixture(true);
    let mut source = short_animation_source(true);
    let skeleton = source.skeleton.as_mut().unwrap();
    skeleton.tracks[0].definition.frames[0] = frame([0.; 3], 1.);
    for frame in &mut skeleton.tracks[1].definition.frames {
        frame.translation = [0.; 3];
        frame.scale = 1.;
    }
    let part = &mut source.parts[0];
    // Two coincident triangles with identical initial vertex attributes but
    // different weighted motion owners. Put both on the visible material.
    for index in 0..3 {
        part.mesh.vertices[index + 3] = part.mesh.vertices[index];
    }
    part.mesh.vertex_pieces = vec![(3, 0), (3, 1)];
    part.mesh.polygon_textures = vec![(2, 0)];
    for polygon in &mut part.mesh.polygons {
        polygon.collidable = false;
    }
    // A second coincident part also needs its own flattened source offsets.
    let mut static_part = part.clone();
    static_part.mesh.vertex_pieces.clear();
    static_part.rigid_track = Some(0);
    source.parts.push(static_part);
    assert!(source.stationary_collision_animation_radius().is_ok());

    let initial = source.sample_animation(Duration::ZERO).unwrap();
    let (_, ordinary) = mesh::bake_wld_meshes(&wld, &initial);
    let (_, geometries, bindings) = mesh::bake_wld_meshes_with_sources(&wld, &initial);
    assert_eq!(ordinary.len(), 1);
    assert_eq!(ordinary[0].vertices.len() / mesh::VERTEX_STRIDE, 3);
    assert_eq!(geometries.len(), 1);
    assert_eq!(bindings.len(), geometries.len());
    assert_eq!(geometries[0].vertices.len() / mesh::VERTEX_STRIDE, 12);
    assert_eq!(geometries[0].indices.len(), 12);
    let mapped = &bindings[0];
    assert_eq!(
        mapped.iter().copied().collect::<BTreeSet<_>>(),
        (0..12).collect()
    );
    let flat_initial: Vec<_> = initial.iter().flat_map(|mesh| &mesh.vertices).collect();
    for (vertex, &original) in geometries[0]
        .vertices
        .chunks_exact(mesh::VERTEX_STRIDE)
        .zip(mapped)
    {
        assert_eq!(vertex[..3], *flat_initial[original]);
    }
    let later = source
        .sample_animation(Duration::from_millis(1000))
        .unwrap();
    let flat_later: Vec<_> = later.iter().flat_map(|mesh| &mesh.vertices).collect();
    let static_slot = mapped.iter().position(|index| *index == 0).unwrap();
    let moving_slot = mapped.iter().position(|index| *index == 3).unwrap();
    let second_part_slot = mapped.iter().position(|index| *index == 6).unwrap();
    assert_ne!(static_slot, moving_slot);
    assert_ne!(static_slot, second_part_slot);
    assert_eq!(
        flat_initial[mapped[static_slot]],
        flat_initial[mapped[moving_slot]]
    );
    assert_eq!(
        flat_initial[mapped[static_slot]],
        flat_initial[mapped[second_part_slot]]
    );
    assert_eq!(
        flat_later[mapped[static_slot]],
        flat_later[mapped[second_part_slot]]
    );
    assert_ne!(
        flat_later[mapped[static_slot]],
        flat_later[mapped[moving_slot]]
    );
    // Repacking a later pose changes attribute deduplication. The original
    // source map still has one stable slot for each independently owned vertex.
    let (_, later_ordinary) = mesh::bake_wld_meshes(&wld, &later);
    assert_eq!(later_ordinary[0].vertices.len() / mesh::VERTEX_STRIDE, 6);
    assert_eq!(bindings[0].len(), 12);
}

#[test]
#[ignore = "requires original City of Mist object archive in EQ_DIR; CPU animation bindings"]
fn original_citymist_live_animation_admits_static_trunks_and_rejects_moving_colliders() {
    let base = std::env::var_os("EQ_DIR").expect("set EQ_DIR");
    let scene = super::super::load_object_library(&base, "citymist").unwrap();
    let source = &scene.wld_object_sources["jntree103"];
    let bindings = source
        .render_animation()
        .expect("static trunk permits live animation");
    let radius = source.stationary_collision_animation_radius().unwrap();
    assert_eq!(bindings.radius(), radius);
    let object = scene
        .objects
        .iter()
        .find(|object| object.name == "jntree103")
        .unwrap();
    assert_eq!(bindings.vertices().len(), object.meshes.len());
    let initial = source.sample_animation(Duration::ZERO).unwrap();
    let flat: Vec<_> = initial
        .iter()
        .flat_map(|mesh| mesh.vertices.iter().zip(&mesh.normals))
        .collect();
    for (&mesh_index, map) in object.meshes.iter().zip(bindings.vertices()) {
        let geometry = &scene.meshes[mesh_index];
        assert_eq!(geometry.vertices.len() / mesh::VERTEX_STRIDE, map.len());
        for (vertex, &source_index) in geometry.vertices.chunks_exact(mesh::VERTEX_STRIDE).zip(map)
        {
            assert_eq!(vertex[..3], *flat[source_index].0);
            assert_eq!(vertex[3..6], *flat[source_index].1);
        }
    }
    for milliseconds in [0, 500, 1000, 1500, 2000, 2500, 3000, 3500, 3999, 4000] {
        let sampled = source
            .sample_animation(Duration::from_millis(milliseconds))
            .unwrap();
        for mesh in sampled {
            assert!(
                mesh.vertices
                    .into_iter()
                    .all(|point| Vec3::from_array(point).length() <= radius)
            );
        }
    }
    for name in ["jntree101", "jntree102"] {
        let source = &scene.wld_object_sources[name];
        assert_eq!(source.animation_period().unwrap(), Duration::from_secs(4));
        assert!(
            source.render_animation().is_none(),
            "{name} moves physical vertices"
        );
        assert!(
            source
                .stationary_collision_animation_radius()
                .unwrap_err()
                .to_string()
                .contains("moves collision ancestry")
        );
    }
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
    assert_eq!(source.animation_period().unwrap(), Duration::from_secs(4));
    let closed = source.sample_animation(Duration::from_secs(4)).unwrap();
    for (actual, expected) in closed.iter().zip(&posed) {
        assert_eq!(actual.vertices, expected.vertices);
    }
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
    // Native whole-second keys match the corresponding authored poses.
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
        let timed = source
            .sample_animation(Duration::from_secs(frame_index as u64))
            .unwrap();
        for (actual, expected) in timed.iter().zip(&sampled) {
            for (actual, expected) in actual.vertices.iter().zip(&expected.vertices) {
                near(*actual, *expected);
            }
        }
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
    let between = source.sample_animation(Duration::from_millis(500)).unwrap();
    assert_eq!(between[trunk].vertices, posed[trunk].vertices);
    assert_eq!(between[trunk].normals, posed[trunk].normals);
    assert!(
        between
            .iter()
            .enumerate()
            .any(|(index, mesh)| { index != trunk && mesh.vertices != posed[index].vertices })
    );
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
