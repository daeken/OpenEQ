//! Polygon material references are source ordinals, then first exact-name wins.
#[path = "support/eqg_collision.rs"]
mod support;

use std::collections::{HashMap, HashSet};

use glam::{Mat4, Quat, Vec3};
use openeq_assets::{
    collision::CollisionWorld,
    loader, mesh,
    pfs::Archive,
    zone::{TerMod, ZoneFile},
};
use support::*;

#[test]
fn source_order_and_exact_names_control_draw_and_water_without_discarding_records() {
    for is_terrain in [false, true] {
        let mut source = model(is_terrain);
        for (stored_id, name, shader, texture) in [
            (7, "shared", "Opaque.fx", "first.dds"),
            (7, "shared", "Opaque_MaxWater.fx", "overridden.dds"),
            (1, "Shared", "Opaque_MaxWater.fx", "water.dds"),
            (7, "unique", "Opaque.fx", "restored.dds"),
        ] {
            let mut value = material(shader, Some(texture));
            value.stored_id = stored_id;
            value.name = name.into();
            source.materials.push(value);
        }
        // The first record is unused by polygons, but still supplies the
        // material for record 1. Name matching is case-sensitive for record 2.
        for (z, ordinal) in [(0., 1), (10., 2), (20., 3), (30., 7), (40., u32::MAX)] {
            quad(&mut source, floor(z), ordinal, 0);
        }
        let filename = if is_terrain {
            "ground.ter"
        } else {
            "ground.mod"
        };
        let fixture = Fixture::new(&[(filename, &source)], &[]);
        let archive = Archive::open(fixture.0.join("fixture.eqg")).unwrap();
        let parsed = TerMod::parse(&archive.read(filename).unwrap(), is_terrain).unwrap();
        assert_eq!(parsed.materials.len(), 4);
        assert_eq!(
            parsed
                .materials
                .iter()
                .map(|m| m.stored_id)
                .collect::<Vec<_>>(),
            [7, 7, 1, 7]
        );
        assert_eq!(parsed.materials[1].shader, "Opaque_MaxWater.fx");
        assert_eq!(
            parsed.materials[1].properties["e_TextureDiffuse0"].as_text(),
            Some("overridden.dds")
        );
        assert!(std::ptr::eq(
            parsed.material_for_polygon(1).unwrap(),
            &parsed.materials[0]
        ));
        assert!(std::ptr::eq(
            parsed.material_for_polygon(2).unwrap(),
            &parsed.materials[2]
        ));
        assert!(parsed.material_for_polygon(7).is_none());
        assert!(parsed.material_for_polygon(u32::MAX).is_none());
        assert_eq!(parsed.polygons, source.polygons);

        let scene = fixture.scene();
        assert_eq!(scene.meshes.len(), 3);
        let groups = source.mesh_groups();
        for ((draw, expected), ordinal) in scene
            .meshes
            .iter()
            .zip(["first.dds", "water.dds", "restored.dds"])
            .zip([1, 2, 3])
        {
            let (vertices, indices) = mesh::pack(
                &source.positions,
                &source.normals,
                &source.tex_coords,
                &groups[&ordinal],
            );
            assert_eq!(draw.vertices, vertices);
            assert_eq!(draw.indices, indices);
            assert_eq!(scene.materials[draw.material].textures, [expected]);
        }
        assert!(scene.materials[0].water.is_none());
        assert!(scene.materials[1].water.is_some());
        let physical = &scene.collision_meshes[0];
        assert_eq!(physical.indices.len(), 18);
        let heights: HashSet<_> = physical.positions.iter().map(|p| p[2] as i32).collect();
        assert_eq!(heights, HashSet::from([0, 20, 40]));
        // Swap only the shaders of the same-name records. Record 1 still uses
        // record 0's shader, so the first surface now becomes nonphysical water.
        source.materials[0].shader = "Opaque_MaxWater.fx".into();
        source.materials[1].shader = "Opaque.fx".into();
        let water_fixture = Fixture::new(&[(filename, &source)], &[]);
        let water_scene = water_fixture.scene();
        assert!(water_scene.materials[0].water.is_some());
        let physical = &water_scene.collision_meshes[0];
        assert_eq!(physical.indices.len(), 12);
        assert!(
            physical
                .positions
                .iter()
                .all(|p| p[2] == 20. || p[2] == 40.)
        );
    }
}

#[test]
#[ignore = "requires original Housegarden assets; checks exact source geometry and textures"]
fn original_housegarden_restores_missing_batches_and_stonewall_diffuse() {
    let base = loader::default_client_dir().expect("original client assets");
    let archive = Archive::open(base.join("housegarden.eqg")).unwrap();
    let bytes = archive.read("ter_gardens.ter").unwrap();
    assert_eq!(bytes.len(), 1_843_330);
    let terrain = TerMod::parse(&bytes, true).unwrap();
    assert_eq!(
        (
            terrain.materials.len(),
            terrain.positions.len(),
            terrain.polygons.len()
        ),
        (33, 31_336, 22_884)
    );
    let old_id_map: HashMap<_, _> = terrain
        .materials
        .iter()
        .enumerate()
        .map(|(ordinal, m)| (m.stored_id, ordinal))
        .collect();
    assert_eq!(old_id_map.len(), 23);
    assert_eq!(old_id_map[&3], 32);
    assert_eq!(terrain.materials[32].name, "branchalt");
    assert_eq!(
        terrain.materials[32].properties["e_TextureDiffuse0"].as_text(),
        Some("branches01.dds")
    );
    assert_eq!(terrain.polygons[0], (0, 1, 2, 3, 0x20000));
    assert_eq!(terrain.positions[0], [-251.53598, -321.49957, -38.971634]);

    let scene = loader::load_zone(&base, "housegarden").unwrap();
    let owned: HashSet<_> = scene
        .objects
        .iter()
        .flat_map(|o| o.meshes.iter().copied())
        .collect();
    let terrain_draws: Vec<_> = scene
        .meshes
        .iter()
        .enumerate()
        .filter(|(id, _)| !owned.contains(id))
        .map(|(_, m)| m)
        .collect();
    let groups = terrain.mesh_groups();
    assert_eq!(
        terrain_draws
            .iter()
            .map(|m| m.indices.len() / 3)
            .sum::<usize>(),
        21_708
    );
    let old_count: usize = terrain
        .polygons
        .iter()
        .filter(|p| old_id_map.contains_key(&p.3))
        .count();
    assert_eq!(old_count, 21_531);
    let mut newly_drawn = 0;
    for (ordinal, name, texture, count, witness) in [
        (3, "stonewall", "thule_stonestack_c.dds", 2210, 0),
        (23, "brk", "Di_birch_bark256.dds", 71, 21775),
        (26, "stem", "sp_hedgeA.dds", 52, 22608),
        (29, "bloodbark", "bloodmoon_bark512_c.dds", 54, 22660),
    ] {
        let material = terrain.material_for_polygon(ordinal).unwrap();
        assert_eq!(material.name, name);
        assert_eq!(
            material.properties["e_TextureDiffuse0"].as_text(),
            Some(texture)
        );
        assert_eq!(groups[&ordinal].len() / 3, count);
        assert_eq!(terrain.polygons[witness].3, ordinal);
        if ordinal != 3 {
            assert!(!old_id_map.contains_key(&ordinal));
            newly_drawn += count;
        }
        let (vertices, indices) = mesh::pack(
            &terrain.positions,
            &terrain.normals,
            &terrain.tex_coords,
            &groups[&ordinal],
        );
        let matching: Vec<_> = terrain_draws
            .iter()
            .filter(|draw| draw.vertices == vertices && draw.indices == indices)
            .collect();
        assert_eq!(matching.len(), 1, "source ordinal {ordinal}: {name}");
        let loaded = &scene.materials[matching[0].material];
        assert_eq!(loaded.textures, [texture]);
        assert!(
            !loaded.alpha_mask,
            "the old branch material was alpha-masked"
        );
        let decoded = scene.texture(texture).expect("authored diffuse decodes");
        assert!(decoded.width > 1 && decoded.height > 1);
    }
    assert_eq!(newly_drawn, 177);

    // The loose declaration still retains the unreferenced channel-bearing
    // records, while the draw witness above proves the visible restoration.
    let source = ZoneFile::parse(
        &std::fs::read(base.join("housegarden.zon")).unwrap(),
        |name| archive.read(name),
    )
    .unwrap();
    assert_eq!(source.placeables.len(), scene.instances.len());
}

#[test]
#[ignore = "requires original Anguish assets; verifies first same-name normal-map binding"]
fn original_anguish_same_name_records_preserve_source_but_bind_first_normal() {
    let base = loader::default_client_dir().expect("original client assets");
    let archive = Archive::open(base.join("anguish.eqg")).unwrap();
    let terrain = TerMod::parse(&archive.read("ter_island.ter").unwrap(), true).unwrap();
    for ordinal in [17, 20] {
        assert_eq!(terrain.materials[ordinal].stored_id, ordinal as u32);
        assert_eq!(terrain.materials[ordinal].name, "prison 13");
        assert_eq!(
            terrain.materials[ordinal].properties["e_TextureDiffuse0"].as_text(),
            Some("av_prison13_c.dds")
        );
    }
    assert_eq!(
        terrain.materials[17].properties["e_TextureNormal0"].as_text(),
        Some("av_rock05_n.dds")
    );
    assert_eq!(
        terrain.materials[20].properties["e_TextureNormal0"].as_text(),
        Some("av_prison12a_n.dds")
    );
    assert!(std::ptr::eq(
        terrain.material_for_polygon(20).unwrap(),
        &terrain.materials[17]
    ));
    let groups = terrain.mesh_groups();
    assert_eq!(groups[&17].len() / 3, 3592);
    assert_eq!(groups[&20].len() / 3, 6403);
    let scene = loader::load_zone(base, "anguish").unwrap();
    for ordinal in [17, 20] {
        let (vertices, indices) = mesh::pack(
            &terrain.positions,
            &terrain.normals,
            &terrain.tex_coords,
            &groups[&ordinal],
        );
        let matching: Vec<_> = scene
            .meshes
            .iter()
            .filter(|draw| draw.vertices == vertices && draw.indices == indices)
            .collect();
        assert_eq!(matching.len(), 1, "ordinal {ordinal}");
        let material = &scene.materials[matching[0].material];
        assert_eq!(material.textures, ["av_prison13_c.dds"]);
        assert_eq!(material.normal_map.as_deref(), Some("av_rock05_n.dds"));
    }
    assert!(scene.texture("av_rock05_n.dds").is_some());
}

#[test]
#[ignore = "requires original Feerrott2, Buried Sea, and Arelis assets; traces restored MOD faces through instances"]
fn original_heightmap_material_gaps_explain_draw_and_instance_collision_deltas() {
    struct Witness {
        model: &'static str,
        ordinal: u32,
        stored_id: u32,
        name: &'static str,
        diffuse: &'static str,
        triangles: usize,
        instances: usize,
        polygon: usize,
        indices: [u32; 3],
        flags: u32,
    }
    let base = loader::default_client_dir().expect("original client assets");
    let cases = [
        (
            "feerrott2",
            245377,
            246313,
            824399,
            826571,
            vec![
                Witness {
                    model: "obj_feerrott_river_fence_arched",
                    ordinal: 7,
                    stored_id: 8,
                    name: "mtl2uv",
                    diffuse: "Di_ent_metal01_c.dds",
                    triangles: 524,
                    instances: 1,
                    polygon: 0,
                    indices: [0, 2, 3],
                    flags: 0x20000,
                },
                Witness {
                    model: "obj_feerrott_river_fence_straight",
                    ordinal: 7,
                    stored_id: 8,
                    name: "mtl2uv",
                    diffuse: "Di_ent_metal01_c.dds",
                    triangles: 412,
                    instances: 4,
                    polygon: 0,
                    indices: [0, 2, 3],
                    flags: 0x20000,
                },
            ],
        ),
        (
            "buriedsea",
            1520802,
            1520810,
            1567097,
            1567129,
            vec![Witness {
                model: "obj_sh_mainmast",
                ordinal: 6,
                stored_id: 8,
                name: "Material #8979",
                diffuse: "hp_stumptopa01_c.dds",
                triangles: 8,
                instances: 4,
                polygon: 447,
                indices: [491, 492, 493],
                flags: 0x20000,
            }],
        ),
        (
            "arelis",
            296968,
            296984,
            1338660,
            1338676,
            vec![Witness {
                model: "obj_lunanyn_g",
                ordinal: 6,
                stored_id: 7,
                name: "mosspanels",
                diffuse: "LG_wood_mosspanel_c.dds",
                triangles: 16,
                instances: 1,
                polygon: 1848,
                indices: [927, 3136, 3137],
                flags: 0,
            }],
        ),
    ];
    // Exact ordered source triangle positions, preserving winding and float bits.
    let key = |points: [[f32; 3]; 3]| points.map(|p| p.map(f32::to_bits));
    for (zone, old_draw, new_draw, old_collision, new_collision, witnesses) in cases {
        let archive = Archive::open(base.join(format!("{zone}.eqg"))).unwrap();
        let mut scene = loader::load_zone(&base, zone).unwrap();
        assert_eq!(scene.triangle_count(), new_draw, "{zone}");
        assert_eq!(
            CollisionWorld::build(&scene).triangle_count(),
            new_collision,
            "{zone}"
        );
        let mut restored_draw = 0;
        let mut restored_world = 0;
        for witness in witnesses {
            let source = TerMod::parse(
                &archive.read(&format!("{}.mod", witness.model)).unwrap(),
                false,
            )
            .unwrap();
            let old: HashMap<_, _> = source
                .materials
                .iter()
                .enumerate()
                .map(|(i, m)| (m.stored_id, i))
                .collect();
            assert!(
                !old.contains_key(&witness.ordinal),
                "former ID map dropped this group"
            );
            let material = source.material_for_polygon(witness.ordinal).unwrap();
            assert_eq!(material.stored_id, witness.stored_id);
            assert_eq!(material.name, witness.name);
            assert_eq!(
                material.properties["e_TextureDiffuse0"].as_text(),
                Some(witness.diffuse)
            );
            let [a, b, c] = witness.indices;
            assert_eq!(
                source.polygons[witness.polygon],
                (a, b, c, witness.ordinal, witness.flags)
            );
            let added: Vec<_> = source
                .polygons
                .iter()
                .filter(|p| p.3 == witness.ordinal)
                .collect();
            assert_eq!(added.len(), witness.triangles);
            assert!(added.iter().all(|p| p.4 & 1 == 0));
            let expected_points: Vec<_> = added
                .iter()
                .map(|p| [p.0, p.1, p.2].map(|index| source.positions[index as usize]))
                .collect();
            let expected_keys: HashSet<_> = expected_points.iter().copied().map(key).collect();

            let object = scene
                .objects
                .iter()
                .find(|o| o.name.trim_end_matches(".mod") == witness.model)
                .unwrap();
            let groups = source.mesh_groups();
            let (vertices, indices) = mesh::pack(
                &source.positions,
                &source.normals,
                &source.tex_coords,
                &groups[&witness.ordinal],
            );
            let matches: Vec<_> = object
                .meshes
                .iter()
                .copied()
                .filter(|&i| {
                    scene.meshes[i].vertices == vertices && scene.meshes[i].indices == indices
                })
                .collect();
            assert_eq!(matches.len(), 1, "{} exact original batch", witness.model);
            let loaded = &scene.materials[scene.meshes[matches[0]].material];
            assert_eq!(loaded.textures, [witness.diffuse]);
            assert!(loaded.water.is_none());
            assert_eq!(
                loaded.normal_map.as_deref(),
                material
                    .properties
                    .get("e_TextureNormal0")
                    .and_then(|p| p.as_text())
            );
            assert!(scene.texture(witness.diffuse).is_some());

            let instances: Vec<_> = scene
                .instances
                .iter()
                .filter(|i| i.object == object.name)
                .collect();
            assert_eq!(
                instances.len(),
                witness.instances,
                "{} placements",
                witness.model
            );
            // Independently check every restored world face. The exact original
            // placements make all these faces finite and above the area gate;
            // the instance multiplier is therefore an exact collision delta.
            for instance in &instances {
                let transform = Mat4::from_scale_rotation_translation(
                    Vec3::from(instance.scale),
                    Quat::from_array(instance.rotation),
                    Vec3::from(instance.position),
                );
                for points in &expected_points {
                    let [a, b, c] = points.map(|p| transform.transform_point3(Vec3::from(p)));
                    assert!(a.is_finite() && b.is_finite() && c.is_finite());
                    let area = (b - a).cross(c - a).length_squared();
                    assert!(area.is_finite() && area >= 1e-10);
                }
            }
            eprintln!(
                "{zone}: {} ordinal{} stored{}: {} restored source faces x {} instances = {}; witness local {:?}; placements {:?}",
                witness.model,
                witness.ordinal,
                witness.stored_id,
                witness.triangles,
                instances.len(),
                witness.triangles * instances.len(),
                expected_points[0],
                instances.iter().map(|i| i.position).collect::<Vec<_>>()
            );
            restored_draw += witness.triangles;
            restored_world += witness.triangles * instances.len();

            // Replay the former missing group by removing only its exact source
            // faces from the loaded physical channel. All other object/terrain
            // geometry and all placements remain unchanged.
            let mut removed = Vec::new();
            for &mesh_id in &object.collision_meshes {
                let mesh = &mut scene.collision_meshes[mesh_id];
                mesh.indices = mesh
                    .indices
                    .chunks_exact(3)
                    .filter(|triangle| {
                        let points = [triangle[0], triangle[1], triangle[2]]
                            .map(|i| mesh.positions[i as usize]);
                        if expected_keys.contains(&key(points)) {
                            removed.push(points);
                            false
                        } else {
                            true
                        }
                    })
                    .flatten()
                    .copied()
                    .collect();
            }
            assert_eq!(
                removed, expected_points,
                "{} exact source collision faces and winding",
                witness.model
            );
            scene.meshes[matches[0]].indices.clear();

            if zone == "arelis" {
                // A second group previously bound mosspanels by stored ID 7.
                // It retains its geometry but correctly binds the roof moss.
                assert_eq!(old[&7], 6);
                let (vertices, indices) = mesh::pack(
                    &source.positions,
                    &source.normals,
                    &source.tex_coords,
                    &groups[&7],
                );
                assert_eq!(indices.len() / 3, 64);
                let draw = object
                    .meshes
                    .iter()
                    .map(|&i| &scene.meshes[i])
                    .find(|m| m.vertices == vertices && m.indices == indices)
                    .unwrap();
                assert_eq!(
                    scene.materials[draw.material].textures,
                    ["LG_roofmossalt_c.dds"]
                );
                assert!(
                    !scene
                        .objects
                        .iter()
                        .any(|o| o.name.contains("obj_lunanyn_g_lod1")),
                    "the archived LOD also has a gap but is not placed by this loader"
                );
            }
        }
        assert_eq!(
            new_draw - old_draw,
            restored_draw,
            "{zone} source draw delta"
        );
        assert_eq!(
            new_collision - old_collision,
            restored_world,
            "{zone} instance collision delta"
        );
        assert_eq!(
            scene.triangle_count(),
            old_draw,
            "{zone} replayed previous drawing"
        );
        assert_eq!(
            CollisionWorld::build(&scene).triangle_count(),
            old_collision,
            "{zone} replayed previous physics"
        );
    }
}
