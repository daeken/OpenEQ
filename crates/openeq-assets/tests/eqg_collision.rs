//! Public-loader regressions for EQG's independent drawing and physical channels.
#[path = "support/eqg_collision.rs"]
mod support;
use glam::{Mat4, Quat, Vec3};
use openeq_assets::{
    Instance, Scene,
    collision::CollisionWorld,
    loader, mesh,
    pfs::Archive,
    zone::{Placeable, TerMod, ZoneFile},
};
use support::*;

fn near(actual: [f32; 3], expected: [f32; 3]) {
    assert!(
        Vec3::from(actual).distance(Vec3::from(expected)) < 0.03,
        "{actual:?} != {expected:?}"
    );
}
fn source_triangles(scene: &Scene) -> usize {
    scene
        .collision_meshes
        .iter()
        .map(|m| m.indices.len() / 3)
        .sum()
}

#[test]
fn loaded_eqg_keeps_material_batches_but_applies_physics_per_polygon() {
    let mut source = model(true);
    source
        .materials
        .insert(0, material("Opaque.fx", Some("stone.dds")));
    source
        .materials
        .insert(1, material("Opaque_MaxWater.fx", Some("water.dds")));
    source.materials.insert(2, material("Opaque.fx", None));
    quad(&mut source, floor(0.), 0, 0);
    quad(&mut source, wall(10.), 0, 0x800a_0000);
    quad(&mut source, wall(-5.), 0, 0xffff_ffff);
    quad(&mut source, wall(5.), u32::MAX, 2);
    quad(&mut source, wall(-10.), u32::MAX, 3);
    quad(&mut source, wall(-15.), u32::MAX - 1, 0);
    quad(
        &mut source,
        [[-3., 12., 3.], [3., 12., 3.], [3., 18., 3.], [-3., 18., 3.]],
        1,
        0,
    );
    quad(
        &mut source,
        [
            [-3., -18., 2.],
            [3., -18., 2.],
            [3., -12., 2.],
            [-3., -12., 2.],
        ],
        2,
        0,
    );
    let fixture = Fixture::new(&[("ground.ter", &source)], &[]);
    let scene = fixture.scene();
    assert_eq!(
        (
            scene.meshes.len(),
            scene.materials.len(),
            scene.triangle_count()
        ),
        (3, 3, 10)
    );
    assert_eq!(source_triangles(&scene), 8);
    assert!(scene.meshes.iter().all(|m| !m.collidable));
    assert!(scene.objects.is_empty());
    assert_eq!(scene.materials[0].textures, ["stone.dds"]);
    assert!(scene.materials[1].water.is_some());
    assert_eq!(scene.materials[2].textures, ["missing.dds"]);
    // Same material grouping and exact original attributes/indices, including
    // the drawn passable wall. Physics must not split or filter these batches.
    let groups = source.mesh_groups();
    for (id, draw) in scene.meshes.iter().enumerate() {
        let (vertices, indices) = mesh::pack(
            &source.positions,
            &source.normals,
            &source.tex_coords,
            &groups[&(id as u32)],
        );
        assert_eq!(draw.vertices, vertices);
        assert_eq!(draw.indices, indices);
        assert_eq!(draw.material, id);
    }
    let world = CollisionWorld::build(&scene);
    assert_eq!(
        world.triangle_count(),
        8,
        "drawable channel must not duplicate physical faces"
    );
    near(
        world.move_player([0., 0., 0.], [8., 0., 0.], 1., 6., 2.),
        [3.999, 0., 0.],
    );
    near(
        world.move_player([0., 0., 0.], [-18., 0., 0.], 1., 6., 2.),
        [-18., 0., 0.],
    );
    near(
        world.move_player([7., 0., 0.], [7., 0., 0.], 1., 6., 2.),
        [8.999, 0., 0.],
    );
    near(
        world.clip_camera([0., 0., 4.], [8., 0., 4.], 0.),
        [5., 0., 4.],
    );
    near(
        world.clip_camera([8., 0., 4.], [0., 0., 4.], 0.),
        [5., 0., 4.],
    );
    near(
        world.clip_camera([0., 0., 4.], [-18., 0., 4.], 0.),
        [-18., 0., 4.],
    );
    assert_eq!(
        world.ground_height(0., 15., 3., 0.1, 0.1),
        None,
        "water is not a physical floor"
    );
    assert_eq!(
        world.ground_height(0., -15., 2., 0.1, 0.1),
        Some(2.),
        "missing diffuse is still resolved ordinary geometry"
    );
}

#[test]
fn hidden_mods_retain_ownership_local_winding_and_all_instance_transforms() {
    let mut terrain = model(true);
    quad(
        &mut terrain,
        [
            [100., 100., 7.],
            [102., 100., 7.],
            [102., 102., 7.],
            [100., 102., 7.],
        ],
        u32::MAX,
        2,
    );
    let mut hidden = model(false);
    quad(
        &mut hidden,
        [[0., 0., 0.], [2., 0., 0.], [2., 2., 0.], [0., 2., 0.]],
        u32::MAX,
        0x8000_0002,
    );
    let fixture = Fixture::new(
        &[
            ("ground.ter", &terrain),
            ("hidden.mod", &hidden),
            ("unplaced.mod", &hidden),
        ],
        &[
            Placeable {
                object_id: 0,
                name: "ignored terrain placement".into(),
                position: [1000.; 3],
                rotation: [0.; 3],
                scale: 20.,
            },
            Placeable {
                object_id: 1,
                name: "placed hidden object".into(),
                position: [10., 20., 5.],
                rotation: [0., 0., std::f32::consts::FRAC_PI_2],
                scale: 2.,
            },
        ],
    );
    let mut scene = fixture.scene();
    assert!(scene.meshes.is_empty() && scene.materials.is_empty());
    assert_eq!(scene.objects.len(), 2);
    assert_eq!(source_triangles(&scene), 6);
    let world = CollisionWorld::build(&scene);
    assert_eq!(
        world.triangle_count(),
        4,
        "unplaced MOD must not collide at origin"
    );
    assert_eq!(world.ground_height(1., 1., 0., 0., 0.), None);
    assert_eq!(
        world.ground_height(101., 101., 7., 0., 0.),
        Some(7.),
        "TER placement must not transform direct terrain"
    );
    assert_eq!(world.ground_height(8., 22., 5., 0.01, 0.01), Some(5.));
    let placed = scene
        .instances
        .iter_mut()
        .find(|i| i.object == "object_1")
        .unwrap();
    placed.scale = [2., 3., 1.];
    scene.instances.push(Instance {
        object: "object_1".into(),
        position: [-10., -20., 7.],
        scale: [-2., 1.5, 2.],
        rotation: Quat::from_rotation_z(std::f32::consts::FRAC_PI_4).to_array(),
    });
    let world = CollisionWorld::build(&scene);
    assert_eq!(world.triangle_count(), 6);
    for instance in scene.instances.iter().filter(|i| i.object == "object_1") {
        let transform = Mat4::from_scale_rotation_translation(
            instance.scale.into(),
            Quat::from_array(instance.rotation),
            instance.position.into(),
        );
        let point = transform.transform_point3(Vec3::new(1., 1., 0.));
        assert_eq!(
            world.ground_height(point.x, point.y, point.z, 0.01, 0.01),
            Some(point.z)
        );
        assert!(world.supports_player(point.to_array(), 0.2, 0.01));
    }
    let local = scene.object_model("OBJECT_1_ACTORDEF").unwrap();
    assert!(local.meshes.is_empty() && local.instances.is_empty());
    let physical = &local.collision_meshes[0];
    let positions: Vec<_> = physical
        .indices
        .iter()
        .map(|&i| physical.positions[i as usize])
        .collect();
    assert_eq!(
        positions,
        [
            hidden.positions[0],
            hidden.positions[1],
            hidden.positions[2],
            hidden.positions[0],
            hidden.positions[2],
            hidden.positions[3]
        ],
        "local positions and winding survive extraction"
    );
    assert_eq!(
        CollisionWorld::build(&local).ground_height(1., 1., 0., 0., 0.),
        Some(0.)
    );
    let library = loader::load_object_library(&fixture.0, "fixture").unwrap();
    assert_eq!(library.objects.len(), 2);
    assert_eq!(CollisionWorld::build(&library).triangle_count(), 0);
    let extracted = library.object_model("HIDDEN.MOD").unwrap();
    assert_eq!(CollisionWorld::build(&extracted).triangle_count(), 2);
}

#[test]
fn source_triangle_scale_does_not_preempt_valid_world_space_collision() {
    let mut models = Vec::new();
    for edge in [1e-4, 1e20] {
        let mut source = model(false);
        source.materials.insert(0, material("Opaque.fx", None));
        quad(
            &mut source,
            [
                [0., 0., 0.],
                [edge, 0., 0.],
                [edge, edge, 0.],
                [0., edge, 0.],
            ],
            0,
            0,
        );
        models.push(source);
    }
    let fixture = Fixture::new(
        &[("small.mod", &models[0]), ("large.mod", &models[1])],
        &[
            Placeable {
                object_id: 0,
                name: "small".into(),
                position: [10., 20., 5.],
                rotation: [0.; 3],
                scale: 100.,
            },
            Placeable {
                object_id: 1,
                name: "large".into(),
                position: [500., 600., 7.],
                rotation: [0.; 3],
                scale: 1e-18,
            },
        ],
    );
    let mut scene = fixture.scene();
    let world = CollisionWorld::build(&scene);
    assert_eq!(world.triangle_count(), 4);
    assert_eq!(world.ground_height(10.005, 20.005, 5., 0., 0.), Some(5.));
    assert_eq!(world.ground_height(550., 650., 7., 0., 0.), Some(7.));
    restore_legacy_collision(&mut scene);
    assert_eq!(
        CollisionWorld::build(&scene).triangle_count(),
        world.triangle_count()
    );
}

#[test]
fn invalid_hidden_faces_do_not_poison_loaded_objects_or_create_collision() {
    let source = hidden_door();
    let fixture = Fixture::new(&[("hidden.mod", &source)], &[]);
    let library = loader::load_object_library(&fixture.0, "fixture").unwrap();
    assert!(library.meshes.is_empty());
    assert_eq!(CollisionWorld::build(&library).triangle_count(), 0);
    let local = library.object_model("hidden").unwrap();
    assert_eq!(source_triangles(&local), 4);
    let world = CollisionWorld::build(&local);
    assert_eq!(world.triangle_count(), 4);
    assert_eq!(world.ground_height(6., 0., 0., 0., 0.), Some(0.));
    assert_eq!(world.ground_height(10000., 10000., 10000., 0., 0.), None);
    near(
        world.clip_camera([-3., 0., 4.], [3., 0., 4.], 0.),
        [0., 0., 4.],
    );
}

fn assert_physical_source_face(scene: &Scene, object_id: usize, source: &TerMod, polygon: usize) {
    let (a, b, c, _, _) = source.polygons[polygon];
    let expected = [a, b, c].map(|i| source.positions[i as usize]);
    let owned: Vec<usize> = if source.is_terrain {
        (0..scene.collision_meshes.len())
            .filter(|i| !scene.objects.iter().any(|o| o.collision_meshes.contains(i)))
            .collect()
    } else {
        scene
            .objects
            .iter()
            .find(|o| o.name == format!("object_{object_id}"))
            .unwrap()
            .collision_meshes
            .clone()
    };
    assert!(
        owned.iter().any(|&i| {
            let mesh = &scene.collision_meshes[i];
            mesh.indices.chunks_exact(3).any(|t| {
                [
                    mesh.positions[t[0] as usize],
                    mesh.positions[t[1] as usize],
                    mesh.positions[t[2] as usize],
                ] == expected
            })
        }),
        "object {object_id}, polygon {polygon} lost its physical face or winding"
    );
}

#[test]
#[ignore = "requires original Bloodfields archive; run explicitly with --ignored"]
fn original_bloodfields_flags_change_physics_without_changing_frozen_draw_inputs() {
    let base = loader::default_client_dir().expect("original EverQuest assets");
    assert!(
        base.join("bloodfields.eqg").is_file(),
        "original Bloodfields archive required"
    );
    let archive = Archive::open(base.join("bloodfields.eqg")).unwrap();
    let zone = ZoneFile::parse(&archive.read("bloodfields.zon").unwrap(), |n| {
        archive.read(n)
    })
    .unwrap();
    let mut populations = [0usize; 5];
    let mut source_objects = 0;
    for name in archive
        .names()
        .iter()
        .filter(|n| n.ends_with(".mod") || n.ends_with(".ter"))
    {
        let source = TerMod::parse(&archive.read(name).unwrap(), name.ends_with(".ter")).unwrap();
        assert_eq!(source.version, 2);
        source_objects += 1;
        for &(_, _, _, material, flags) in &source.polygons {
            let bucket = if source.material_for_polygon(material).is_some() {
                match flags {
                    0 => 0,
                    1 => 1,
                    _ => {
                        assert_eq!(flags & 0xffff, 0);
                        3
                    }
                }
            } else if material == u32::MAX {
                assert_eq!(flags, 2);
                2
            } else {
                4
            };
            populations[bucket] += 1;
        }
    }
    assert_eq!(source_objects, 524);
    assert_eq!(populations, [83462, 9644, 14059, 203858, 0]);
    assert_eq!((zone.objects.len(), zone.placeables.len()), (530, 697));
    let mut scene = loader::load_zone(&base, "bloodfields").unwrap();
    assert_eq!(draw_fingerprints(&scene), BLOODFIELDS_DRAW_FINGERPRINTS);
    assert_eq!(
        (
            scene.meshes.len(),
            scene.materials.len(),
            scene.objects.len(),
            scene.instances.len(),
            scene.triangle_count()
        ),
        (971, 971, 529, 697, 297388)
    );
    assert!(scene.meshes.iter().all(|m| !m.collidable));
    // 301379 eligible source faces before degenerate-source rejection. The
    // expanded valid world count is the physical behavioral oracle below.
    let eligible: usize = zone
        .objects
        .iter()
        .map(|o| {
            o.polygons
                .iter()
                .filter(|p| {
                    p.4 & 1 == 0 && (p.3 == u32::MAX || o.material_for_polygon(p.3).is_some())
                })
                .count()
        })
        .sum();
    assert_eq!(eligible, 301379);
    let terrain = &zone.objects[236];
    assert!(terrain.is_terrain);
    assert_eq!(terrain.polygons[107681], (64421, 64422, 64423, u32::MAX, 2));
    assert_eq!(
        [64421, 64422, 64423].map(|i| terrain.positions[i]),
        [
            [-809.4191, -1206.0122, -944.9932],
            [-760.62317, -1275.8777, -944.9932],
            [-760.62317, -1275.8777, -825.56146]
        ]
    );
    for (object, polygon, material, flags) in [
        (236, 107681, u32::MAX, 2),
        (259, 1, 6, 0x80000),
        (6, 22, 4, 0),
        (6, 0, 4, 0x20000),
        (244, 192, 5, 0xa0000),
        (200, 149, 0, 0x80000000),
        (236, 107667, u32::MAX, 2),
    ] {
        let source = &zone.objects[object];
        assert_eq!(
            (source.polygons[polygon].3, source.polygons[polygon].4),
            (material, flags)
        );
        assert_physical_source_face(&scene, object, source, polygon);
    }
    assert_eq!(zone.placeables[422].object_id, 259);
    assert_eq!(zone.placeables[287].object_id, 147);
    assert_eq!(
        zone.placeables[287].position,
        [1103.2761, -664.9831, -923.5582]
    );
    assert_eq!(
        zone.placeables[287].rotation,
        [0., 0., -std::f32::consts::FRAC_PI_2]
    );
    assert_eq!(zone.objects[147].polygons.len(), 184);
    assert!(zone.objects[147].polygons.iter().all(|p| p.4 & 1 != 0));
    assert!(
        scene
            .objects
            .iter()
            .find(|o| o.name == "object_147")
            .unwrap()
            .collision_meshes
            .is_empty()
    );
    let world = CollisionWorld::build(&scene);
    assert_eq!(world.triangle_count(), 324800);
    let start = [-780.9877, -1255.4523, -932.1253];
    assert!(world.supports_player(start, 1., 0.05));
    near(
        world.move_player(start, [8.198372, 5.7259674, 0.], 1., 6., 2.),
        [-777.7089, -1253.1626, -932.1253],
    );
    near(
        world.move_player(
            [-772.7893, -1249.7262, -931.43945],
            [-8.198372, -5.7259674, 0.],
            1.,
            6.,
            2.,
        ),
        [-776.06805, -1252.0156, -932.1253],
    );
    let from = [1101.2278, -643.74927, -871.67426];
    let to = [1101.8054, -647.64734, -872.361];
    near(world.clip_camera(from, to, 0.), to);
    assert_eq!(
        world.ground_height(23.138184, -672.88495, -1060.235, 0.1, 2.),
        Some(-1060.285)
    );
    restore_legacy_collision(&mut scene);
    assert_eq!(draw_fingerprints(&scene), BLOODFIELDS_DRAW_FINGERPRINTS);
    let before = CollisionWorld::build(&scene);
    assert_eq!(before.triangle_count(), 320817);
    near(
        before.move_player(start, [8.198372, 5.7259674, 0.], 1., 6., 2.),
        [-772.7894, -1249.7272, -931.43945],
    );
    near(
        before.clip_camera(from, to, 0.),
        [1101.5166, -645.6983, -872.01764],
    );
    assert_eq!(
        before.ground_height(23.138184, -672.88495, -1060.235, 0.1, 2.),
        None
    );
}
