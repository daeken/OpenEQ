use glam::{Mat4, Quat, Vec3};
use openeq_assets::{
    Instance, Scene, SceneObject,
    collision::CollisionWorld,
    mesh::{self, CollisionGeometry, Geometry},
    pfs::Archive,
    wld::{Fragment, Mesh, Polygon, Ref, WLD_MAGIC, Wld},
};

fn near(actual: [f32; 3], expected: [f32; 3]) {
    assert!(
        Vec3::from(actual).distance(Vec3::from(expected)) < 0.03,
        "{actual:?} != {expected:?}"
    );
}

fn words(values: &[u32]) -> Vec<u8> {
    values
        .iter()
        .flat_map(|value| value.to_le_bytes())
        .collect()
}

fn fragment(data: &mut Vec<u8>, kind: u32, name: i32, body: &[u8]) {
    data.extend(words(&[body.len() as u32 + 4, kind]));
    data.extend(name.to_le_bytes());
    data.extend(body);
}

// Two actual materials followed by a material list containing both a missing
// reference and a reference to the wrong fragment type. Missing texture chains
// on the real materials do not change their authored rendering methods.
fn wld_bytes(mesh: Option<&[u8]>) -> Vec<u8> {
    let strings = b"\0GATE_DMSPRITEDEF\0";
    let mut data = words(&[
        WLD_MAGIC,
        0x0001_5500,
        if mesh.is_some() { 4 } else { 3 },
        0,
        0,
        strings.len() as u32,
        0,
    ]);
    let key = [0x95, 0x3a, 0xc5, 0x2a, 0x95, 0x7a, 0x95, 0x6a];
    data.extend(strings.iter().enumerate().map(|(i, b)| b ^ key[i % 8]));
    while !data.len().is_multiple_of(4) {
        data.push(0);
    }
    fragment(&mut data, 0x30, 0, &words(&[0, 0, 0, 0, 0, 0]));
    fragment(&mut data, 0x30, 0, &words(&[0, 0x8000_0001, 0, 0, 0, 0]));
    fragment(&mut data, 0x31, 0, &words(&[0, 4, 1, 2, 999, 3]));
    if let Some(mesh) = mesh {
        fragment(&mut data, 0x36, -1, mesh);
    }
    data
}

fn source_mesh() -> Mesh {
    Mesh {
        materials: Ref(3),
        animation: Ref(0),
        center: [0.; 3],
        bounds_min: [0.; 3],
        bounds_max: [0.; 3],
        vertices: Vec::new(),
        normals: Vec::new(),
        tex_coords: Vec::new(),
        colors: Vec::new(),
        polygons: Vec::new(),
        vertex_pieces: Vec::new(),
        polygon_textures: Vec::new(),
    }
}

fn quad(mesh: &mut Mesh, points: [[f32; 3]; 4], slot: u16, collidable: bool) {
    let start = mesh.vertices.len() as u32;
    mesh.vertices.extend(points);
    mesh.normals.extend([[0., 0., 1.]; 4]);
    mesh.tex_coords.extend([[0., 0.]; 4]);
    for [a, b, c] in [[0, 1, 2], [0, 2, 3]] {
        mesh.polygons.push(Polygon {
            collidable,
            a: start + a,
            b: start + b,
            c: start + c,
        });
    }
    mesh.polygon_textures.push((2, slot));
}

fn wall(x: f32) -> [[f32; 3]; 4] {
    [[x, -10., 0.], [x, 10., 0.], [x, 10., 10.], [x, -10., 10.]]
}

#[test]
fn hidden_collision_blocks_without_changing_the_drawable_bake() {
    let wld = Wld::parse("synthetic.wld".into(), &wld_bytes(None)).unwrap();
    let mut mesh = source_mesh();
    quad(
        &mut mesh,
        [
            [-20., -20., 0.],
            [20., -20., 0.],
            [20., 20., 0.],
            [-20., 20., 0.],
        ],
        1,
        true,
    );
    quad(&mut mesh, wall(5.), 0, true);
    quad(&mut mesh, wall(-5.), 0, false);
    quad(&mut mesh, wall(10.), 1, true);
    quad(
        &mut mesh,
        [[-3., 12., 3.], [3., 12., 3.], [3., 18., 3.], [-3., 18., 3.]],
        0,
        true,
    );
    let (materials, visible) = mesh::bake_wld_meshes(&wld, [&mesh]);
    assert_eq!(materials.len(), 1);
    assert!(materials.iter().all(|material| material.flags != 0));
    let mut scene = Scene::from_geometry("paired".into(), materials, visible, Vec::new());
    let drawable_before: Vec<_> = scene
        .meshes
        .iter()
        .map(|mesh| {
            (
                mesh.vertices.clone(),
                mesh.indices.clone(),
                mesh.material,
                mesh.collidable,
            )
        })
        .collect();
    let before = CollisionWorld::build(&scene);
    assert_eq!(scene.triangle_count(), 4);
    near(
        before.move_player([0., 0., 0.], [8., 0., 0.], 1., 6., 2.),
        [8., 0., 0.],
    );
    near(
        before.move_player([0., 0., 0.], [14., 0., 0.], 1., 6., 2.),
        [8.999, 0., 0.],
    );
    assert_eq!(before.ground_height(0., 15., 3., 0.1, 0.1), None);
    scene.collision_meshes = mesh::bake_wld_collision_meshes(&wld, [&mesh]);
    assert_eq!(
        scene
            .collision_meshes
            .iter()
            .map(|mesh| mesh.indices.len() / 3)
            .sum::<usize>(),
        4
    );
    let after = CollisionWorld::build(&scene);
    assert_eq!(after.triangle_count(), 8);
    near(
        after.move_player([0., 0., 0.], [8., 0., 0.], 1., 6., 2.),
        [3.999, 0., 0.],
    );
    near(
        after.move_player([-2., 0., 0.], [-6., 0., 0.], 1., 6., 2.),
        [-8., 0., 0.],
    );
    near(
        after.clip_camera([0., 0., 4.], [12., 0., 4.], 0.),
        [5., 0., 4.],
    );
    near(
        after.clip_camera([8., 0., 4.], [0., 0., 4.], 0.),
        [5., 0., 4.],
    );
    assert_eq!(after.ground_height(0., 15., 3., 0.1, 0.1), Some(3.));
    assert!(after.supports_player([0., 15., 3.], 1., 0.01));
    assert_eq!(
        drawable_before,
        scene
            .meshes
            .iter()
            .map(|mesh| (
                mesh.vertices.clone(),
                mesh.indices.clone(),
                mesh.material,
                mesh.collidable
            ))
            .collect::<Vec<_>>()
    );
    assert_eq!(scene.triangle_count(), 4);
}

#[test]
fn only_resolved_invisible_materials_can_author_collision() {
    let wld = Wld::parse("synthetic.wld".into(), &wld_bytes(None)).unwrap();
    for slot in [1, 2, 3, 99] {
        let mut mesh = source_mesh();
        quad(&mut mesh, wall(5.), slot, true);
        assert!(
            mesh::bake_wld_collision_meshes(&wld, [&mesh]).is_empty(),
            "slot {slot}"
        );
    }
    let mut mesh = source_mesh();
    quad(&mut mesh, wall(3.), 2, true); // Bad run must not shift the valid run.
    quad(&mut mesh, wall(7.), 0, true);
    let collision = mesh::bake_wld_collision_meshes(&wld, [&mesh]);
    assert_eq!(collision.len(), 1);
    assert_eq!(collision[0].indices.len(), 6);
    assert!(collision[0].positions.iter().all(|point| point[0] == 7.));
    mesh.materials = Ref(999);
    assert!(mesh::bake_wld_collision_meshes(&wld, [&mesh]).is_empty());
}

#[test]
fn malformed_hidden_runs_and_triangles_do_not_panic_or_invent_collision() {
    let wld = Wld::parse("synthetic.wld".into(), &wld_bytes(None)).unwrap();
    let mut mesh = source_mesh();
    quad(&mut mesh, wall(5.), 0, true);
    mesh.polygons[0].a = u32::MAX;
    let collision = mesh::bake_wld_collision_meshes(&wld, [&mesh]);
    assert_eq!(collision[0].indices.len(), 3);
    mesh.vertices[0][0] = f32::NAN;
    assert!(mesh::bake_wld_collision_meshes(&wld, [&mesh]).is_empty());
    mesh.polygon_textures[0].0 = u16::MAX;
    assert!(mesh::bake_wld_collision_meshes(&wld, [&mesh]).is_empty());

    let collision = CollisionGeometry {
        positions: vec![
            [0., 0., 2.],
            [2., 0., 2.],
            [0., 2., 2.],
            [f32::NAN, 0., 2.],
            [f32::INFINITY, 0., 2.],
            [f32::MAX, 0., 0.],
            [0., f32::MAX, 0.],
        ],
        indices: vec![0, 1, 2, 0, 0, 0, 0, 1, 99, 0, 1, 3, 0, 1, 4, 0, 5, 6, 0, 1],
    };
    let mut world = CollisionWorld::default();
    world.add_collision_geometry(&collision, Mat4::IDENTITY);
    assert_eq!(world.triangle_count(), 1);
    assert_eq!(world.ground_height(0.5, 0.5, 2., 0., 0.), Some(2.));
    world.add_collision_geometry(&collision, Mat4::from_scale(Vec3::splat(f32::NAN)));
    world.add_collision_geometry(&collision, Mat4::from_scale(Vec3::splat(f32::MAX)));
    assert_eq!(world.triangle_count(), 1);
    // The visible channel shares the same validation, including incomplete
    // indices and finite source points whose cross product overflows.
    let visible = Geometry {
        vertices: collision
            .positions
            .iter()
            .flat_map(|p| [p[0], p[1], p[2], 0., 0., 1., 0., 0.])
            .collect(),
        indices: collision.indices,
        material: 0,
        collidable: true,
    };
    world.add_geometry(&visible, Mat4::IDENTITY);
    assert_eq!(world.triangle_count(), 2);
}

#[test]
fn hidden_only_objects_keep_local_coordinates_and_all_instance_transforms() {
    let geometry = CollisionGeometry {
        positions: vec![[0., 0., 0.], [2., 0., 0.], [2., 2., 0.], [0., 2., 0.]],
        indices: vec![0, 1, 2, 0, 2, 3],
    };
    let mut scene = Scene::from_geometry("instances".into(), Vec::new(), Vec::new(), Vec::new());
    scene.collision_meshes.push(geometry.clone());
    scene.objects.push(SceneObject {
        name: "deck".into(),
        meshes: Vec::new(),
        collision_meshes: vec![0],
    });
    assert_eq!(CollisionWorld::build(&scene).triangle_count(), 0);
    scene.instances = vec![
        Instance {
            object: "deck".into(),
            position: [10., 20., 5.],
            scale: [2., 3., 1.],
            rotation: Quat::from_rotation_z(std::f32::consts::FRAC_PI_2).to_array(),
        },
        Instance {
            object: "deck".into(),
            position: [-10., -20., 7.],
            scale: [-2., 1.5, 2.],
            rotation: Quat::from_rotation_z(std::f32::consts::FRAC_PI_4).to_array(),
        },
    ];
    let world = CollisionWorld::build(&scene);
    assert_eq!(world.triangle_count(), 4);
    assert_eq!(world.ground_height(1., 1., 0., 0., 0.), None);
    for instance in &scene.instances {
        let transform = Mat4::from_scale_rotation_translation(
            Vec3::from(instance.scale),
            Quat::from_array(instance.rotation),
            Vec3::from(instance.position),
        );
        let point = transform.transform_point3(Vec3::new(1., 1., 0.));
        assert_eq!(
            world.ground_height(point.x, point.y, point.z, 0.01, 0.01),
            Some(point.z)
        );
        assert!(world.supports_player(point.to_array(), 0.2, 0.01));
    }
    let local = scene.object_model("DECK_ACTORDEF").unwrap();
    assert!(local.meshes.is_empty() && local.materials.is_empty() && local.instances.is_empty());
    assert_eq!(local.collision_meshes[0].positions, geometry.positions);
    assert_eq!(local.collision_meshes[0].indices, geometry.indices);
    assert_eq!(
        CollisionWorld::build(&local).ground_height(1., 1., 0., 0., 0.),
        Some(0.)
    );
    let mut moving = CollisionWorld::default();
    moving.add_collision_geometry(
        &local.collision_meshes[0],
        Mat4::from_translation(Vec3::new(3., 4., 6.)),
    );
    near(
        moving.clip_camera([4., 5., 9.], [4., 5., 3.], 0.),
        [4., 5., 6.],
    );
    assert_eq!(scene.triangle_count(), 0);
}

struct TempDir(std::path::PathBuf);
impl TempDir {
    fn new() -> Self {
        let dir = Self(std::env::temp_dir().join(format!(
            "openeq-invisible-collision-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        )));
        std::fs::create_dir(&dir.0).unwrap();
        dir
    }
}
impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn archive_bytes(name: &str, payload: &[u8]) -> Vec<u8> {
    use std::io::Write;
    fn block(data: &mut Vec<u8>, content: &[u8]) -> u32 {
        let offset = data.len() as u32;
        let mut encoder =
            flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
        encoder.write_all(content).unwrap();
        let compressed = encoder.finish().unwrap();
        data.extend(words(&[compressed.len() as u32, content.len() as u32]));
        data.extend(compressed);
        offset
    }
    let mut data = words(&[0, openeq_assets::pfs::PFS_MAGIC, 0]);
    let file_offset = block(&mut data, payload);
    let mut names = words(&[1, name.len() as u32 + 1]);
    names.extend(name.as_bytes());
    names.push(0);
    let directory_offset = block(&mut data, &names);
    let table = data.len() as u32;
    data.extend(words(&[
        2,
        1,
        file_offset,
        payload.len() as u32,
        openeq_assets::pfs::DIR_CRC,
        directory_offset,
        names.len() as u32,
    ]));
    data[..4].copy_from_slice(&table.to_le_bytes());
    data
}

fn collision_wld_bytes() -> Vec<u8> {
    // Quantized positions /2 plus a nonzero fragment center must be applied
    // exactly once before instance scaling/rotation/translation.
    let mut body = words(&[0, 3, 0, 0, 0]);
    body.extend([7f32, -4., 3.].into_iter().flat_map(f32::to_le_bytes));
    body.extend([0; 12]);
    body.extend(10f32.to_le_bytes());
    body.extend([0; 24]);
    body.extend(
        [4u16, 0, 0, 0, 2, 0, 1, 0, 0, 1]
            .into_iter()
            .flat_map(u16::to_le_bytes),
    );
    body.extend(
        [0i16, 0, 0, 4, 0, 0, 4, 4, 0, 0, 4, 0]
            .into_iter()
            .flat_map(i16::to_le_bytes),
    );
    body.extend(
        [0u16, 0, 1, 2, 0, 0, 2, 3, 2, 0]
            .into_iter()
            .flat_map(u16::to_le_bytes),
    );
    wld_bytes(Some(&body))
}

#[test]
fn classic_zone_without_optional_object_archive_keeps_terrain_collision() {
    let dir = TempDir::new();
    // Windows client files may retain mixed-case archive and world names.
    std::fs::write(
        dir.0.join("Fixture.S3D"),
        archive_bytes("FIXTURE.WLD", &collision_wld_bytes()),
    )
    .unwrap();
    let scene = openeq_assets::load_zone(&dir.0, "fixture").unwrap();
    assert!(scene.objects.is_empty());
    assert_eq!(scene.collision_meshes.len(), 1);
    let world = CollisionWorld::build(&scene);
    assert_eq!(world.triangle_count(), 2);
    assert_eq!(world.ground_height(7.5, -3.5, 3., 0., 0.), Some(3.));
    assert_eq!(
        openeq_assets::audit::metadata(&dir.0, "fixture")
            .unwrap()
            .format,
        "wld"
    );
}

#[test]
fn classic_zone_requires_primary_archive_even_with_props_and_modern_replacement() {
    let dir = TempDir::new();
    for (archive, world) in [
        ("freportw_obj.s3d", "freportw.wld"),
        ("freportw_chr.s3d", "freportw.wld"),
        ("freeportwest.eqg", "freeportwest.zon"),
    ] {
        std::fs::write(
            dir.0.join(archive),
            archive_bytes(world, &collision_wld_bytes()),
        )
        .unwrap();
    }
    let error = openeq_assets::load_zone(&dir.0, "freportw")
        .err()
        .expect("classic archive is missing");
    assert!(
        matches!(error, openeq_assets::Error::MissingZone { ref zone, ref directory }
        if zone == "freportw" && directory == &dir.0)
    );
    let message = error.to_string();
    assert!(message.contains("freportw.s3d"));
    assert!(message.contains("copy this zone's original client files"));
    assert!(matches!(
        openeq_assets::audit::metadata(&dir.0, "freportw"),
        Err(openeq_assets::Error::MissingZone { .. })
    ));
}

#[test]
fn classic_zone_requires_matching_world_inside_primary_archive() {
    let dir = TempDir::new();
    // A matching world in a supplemental archive must not hide a broken primary.
    std::fs::write(
        dir.0.join("fixture_obj.s3d"),
        archive_bytes("fixture.wld", &collision_wld_bytes()),
    )
    .unwrap();
    for wrong_name in ["objects.wld", "otherzone.wld", "fixture.bmp"] {
        std::fs::write(
            dir.0.join("fixture.s3d"),
            archive_bytes(wrong_name, &collision_wld_bytes()),
        )
        .unwrap();
        let error = openeq_assets::load_zone(&dir.0, "fixture")
            .err()
            .expect("primary archive has no matching terrain world");
        assert!(matches!(error, openeq_assets::Error::Format(ref message)
            if message.contains("fixture.s3d") && message.contains("missing its terrain world fixture.wld")));
    }
}

#[test]
fn loaders_preserve_hidden_terrain_and_unplaced_object_ownership() {
    let payload = collision_wld_bytes();
    let dir = TempDir::new();
    for (archive, wld) in [
        ("fixture.s3d", "fixture.wld"),
        ("fixture_obj.s3d", "fixture_obj.wld"),
    ] {
        std::fs::write(dir.0.join(archive), archive_bytes(wld, &payload)).unwrap();
    }
    let mut scene = openeq_assets::load_zone(&dir.0, "fixture").unwrap();
    assert!(scene.meshes.is_empty() && scene.materials.is_empty());
    assert_eq!(scene.collision_meshes.len(), 2);
    assert_eq!(scene.objects.len(), 1);
    assert_eq!(CollisionWorld::build(&scene).triangle_count(), 2);
    assert_eq!(
        CollisionWorld::build(&scene).ground_height(7.5, -3.5, 3., 0., 0.),
        Some(3.)
    );
    scene.instances.push(Instance {
        object: "gate".into(),
        position: [100., 200., 10.],
        scale: [-2., 3., 1.],
        rotation: Quat::IDENTITY.to_array(),
    });
    let world = CollisionWorld::build(&scene);
    assert_eq!(world.triangle_count(), 4);
    assert_eq!(world.ground_height(85., 189.5, 13., 0., 0.), Some(13.));
    let library = openeq_assets::loader::load_object_library(&dir.0, "fixture").unwrap();
    assert_eq!(library.collision_meshes.len(), 1);
    assert_eq!(CollisionWorld::build(&library).triangle_count(), 0);
    let model = library.object_model("GATE_DMSPRITEDEF").unwrap();
    assert!(model.meshes.is_empty());
    assert_eq!(
        CollisionWorld::build(&model).ground_height(7.5, -3.5, 3., 0., 0.),
        Some(3.)
    );
}

#[test]
#[ignore = "requires original Timorous archives; run explicitly with --ignored"]
fn original_timorous_dry_barrier_blocks_without_changing_drawable_inventory() {
    let base =
        openeq_assets::loader::default_client_dir().expect("original EverQuest client directory");
    for archive in ["timorous.s3d", "timorous_obj.s3d"] {
        assert!(
            base.join(archive).is_file(),
            "required original archive {archive}"
        );
    }
    let archive = Archive::open(base.join("timorous.s3d")).unwrap();
    let wld = Wld::open(&archive, "timorous.wld").unwrap();
    let Fragment::Mesh(source) = &wld.by_name("R10684_DMSPRITEDEF").unwrap().fragment else {
        panic!("fixture mesh");
    };
    assert!(source.polygons[12].collidable);
    let Fragment::MaterialList(materials) = &wld.resolve(source.materials).unwrap().fragment else {
        panic!("fixture material list");
    };
    let mut run_start = 0;
    let slot = source
        .polygon_textures
        .iter()
        .find_map(|&(count, slot)| {
            let run_end = run_start + usize::from(count);
            let contains_fixture = (run_start..run_end).contains(&12);
            run_start = run_end;
            contains_fixture.then_some(usize::from(slot))
        })
        .expect("fixture polygon material run");
    let material = wld.resolve(materials.materials[slot]).unwrap();
    assert_eq!(material.name, "M0000_MDF");
    let Fragment::Material(material) = &material.fragment else {
        panic!("fixture material");
    };
    assert_eq!(material.flags, 0);
    let source_points = [
        source.polygons[12].a,
        source.polygons[12].c,
        source.polygons[12].b,
    ]
    .map(|i| source.vertices[i as usize]);
    assert_eq!(
        source_points,
        [
            [-11358., -2382.0313, -4.09375],
            [-11317.656, -2232.0938, 96.5625],
            [-11317.656, -2232.0938, -4.09375]
        ]
    );
    let mut scene = openeq_assets::load_zone(&base, "timorous").unwrap();
    // Replay the previous unresolved actors by temporarily emptying only the
    // new actor-owned buffers. Keep owners and all original raw meshes intact.
    let additions = timorous_new_actor_meshes(&base, &scene);
    let added_indices: Vec<_> = additions
        .into_iter()
        .map(|index| (index, std::mem::take(&mut scene.meshes[index].indices)))
        .collect();
    assert_eq!(
        added_indices
            .iter()
            .map(|(_, indices)| indices.len() / 3)
            .sum::<usize>(),
        620
    );
    assert_eq!(scene.triangle_count(), 227_791);
    let legacy_hidden = std::mem::take(&mut scene.collision_meshes);
    assert_eq!(CollisionWorld::build(&scene).triangle_count(), 259_201);
    scene.collision_meshes = legacy_hidden;
    assert_eq!(CollisionWorld::build(&scene).triangle_count(), 271_639);
    for (index, indices) in added_indices {
        scene.meshes[index].indices = indices;
    }

    // All barrier movement checks below use the full restored runtime scene.
    let hidden = std::mem::take(&mut scene.collision_meshes);
    assert_eq!(
        hidden
            .iter()
            .map(|mesh| mesh.indices.len() / 3)
            .sum::<usize>(),
        12_438
    );
    let before = CollisionWorld::build(&scene);
    assert_eq!(before.triangle_count(), 259_201 + 67_192);
    let drawable = scene.triangle_count();
    let materials = scene.materials.clone();
    let start = [-11326.276, -2283.372, 10.];
    let delta = [-9.65625, 2.5981445, 0.];
    assert!(before.supports_player(start, 1., 0.05));
    near(
        before.move_player(start, delta, 1., 6., 2.),
        [-11335.925, -2280.7744, 10.],
    );
    scene.collision_meshes = hidden;
    let after = CollisionWorld::build(&scene);
    assert_eq!(after.triangle_count(), 271_639 + 67_192);
    assert_eq!(after.triangle_count() - before.triangle_count(), 12_438);
    assert!(after.supports_player(start, 1., 0.05));
    near(
        after.move_player(start, delta, 1., 6., 2.),
        [-11330.137, -2282.3325, 10.],
    );
    near(
        after.move_player(start, [2.0786328, 7.725237, 0.], 1., 6., 2.),
        [-11324.198, -2275.6455, 10.],
    );
    near(
        after.move_player(
            [-11335.933, -2280.774, 10.],
            [9.65625, -2.5981445, 0.],
            1.,
            6.,
            2.,
        ),
        [-11332.071, -2281.8137, 10.],
    );
    let liquids = openeq_assets::liquid_regions::LiquidRegions::load(&base, "timorous").unwrap();
    assert_eq!(liquids.at([start[0], start[1], start[2] + 3.]), None);
    assert_eq!(drawable, 227_791 + 620);
    assert_eq!(scene.triangle_count(), drawable);
    assert_eq!(scene.materials, materials);
    assert!(scene.materials.iter().all(|material| material.flags != 0));
}

/// Derive additions from original mesh keys, not from a broad "all skeletons"
/// filter: a same-name skeletal actor could have replaced an existing model.
fn timorous_new_actor_meshes(base: &std::path::Path, scene: &Scene) -> Vec<usize> {
    use std::collections::BTreeSet;
    let archive = Archive::open(base.join("timorous_obj.s3d")).unwrap();
    let wld = Wld::open(&archive, "timorous_obj.wld").unwrap();
    let original_keys: BTreeSet<_> = wld
        .iter::<Mesh>()
        .map(|(chunk, _)| {
            chunk
                .name
                .to_ascii_lowercase()
                .trim_end_matches("_dmspritedef")
                .to_owned()
        })
        .collect();
    let mut keys = BTreeSet::new();
    let mut indices = BTreeSet::new();
    let mut placements = 0;
    for object in &scene.objects {
        if original_keys.contains(&object.name) {
            continue;
        }
        let source = scene
            .wld_object_sources
            .get(&object.name)
            .expect("newly resolved object must have an authored ActorDef");
        assert_eq!(source.wld_filename, "timorous_obj.wld");
        assert!(source.skeleton.is_some());
        assert!(
            object.collision_meshes.is_empty(),
            "new actors add no hidden collision"
        );
        assert!(keys.insert(object.name.clone()));
        assert!(
            object.meshes.iter().all(|index| indices.insert(*index)),
            "actor geometry is independently owned"
        );
        placements += scene
            .instances
            .iter()
            .filter(|instance| instance.object == object.name)
            .count();
    }
    assert_eq!(
        keys.into_iter().collect::<Vec<_>>(),
        [
            "cbbarrel103",
            "cbcrate103",
            "date101",
            "date102",
            "jngrass101",
            "jntree103",
            "jntree104",
            "jntree105"
        ]
    );
    assert_eq!(placements, 1_996);
    indices.into_iter().collect()
}
