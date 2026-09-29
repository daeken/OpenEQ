//! Direct terrain must preserve paint order, masks, repeat and bounded fallback.
use openeq_assets::{
    Scene,
    mesh::{Geometry, Material},
    terrain::{EcoLayer, MaterialLayer, TerrainMaterial},
    texture::Texture,
};
use openeq_render::{
    Camera, GpuScene, Renderer, environment::EnvironmentSettings, terrain::TerrainMode,
};
use std::time::Duration;

fn eco(name: &str) -> EcoLayer {
    EcoLayer {
        detail_map: name.into(),
        repeat: 1.,
        min_height: -10000.,
        max_height: 10000.,
        height_tolerance: 0.,
        min_slope: 0.,
        max_slope: 180.,
        slope_tolerance: 0.,
    }
}

fn solid(name: &str, rgb: [u8; 3]) -> Texture {
    Texture {
        name: name.into(),
        width: 1,
        height: 1,
        rgba: vec![rgb[0], rgb[1], rgb[2], 255],
    }
}

fn scene(uv: [f32; 2], expected: [u8; 3], paints: Vec<MaterialLayer>) -> Scene {
    let mut vertices = Vec::new();
    for [x, z] in [[-10., -10.], [10., -10.], [10., 10.], [-10., 10.]] {
        vertices.extend([x, 4., z, 0., -1., 0., uv[0], uv[1]]);
    }
    let mut scene = Scene::from_geometry(
        "terrain reference".into(),
        vec![Material {
            textures: vec!["fallback".into()],
            normal_map: None,
            water: None,
            flags: 0,
            anim_speed: 0,
            alpha_mask: false,
            transparent: false,
            emissive: false,
            clamp_uv: true,
        }],
        vec![Geometry {
            vertices,
            indices: vec![0, 1, 2, 0, 2, 3],
            material: 0,
            collidable: true,
        }],
        vec![
            solid("fallback", expected),
            solid("red", [255, 0, 0]),
            solid("green", [0, 255, 0]),
            solid("blue", [0, 0, 255]),
        ],
    );
    scene.terrain_materials.insert(
        0,
        TerrainMaterial {
            fallback_texture: "fallback".into(),
            base_texture: None,
            layers: paints,
        },
    );
    scene
}

fn paint(layers: Vec<EcoLayer>, mask: &[u8]) -> MaterialLayer {
    MaterialLayer {
        layers,
        mask_size: if mask.is_empty() { 0 } else { 2 },
        mask: mask.to_vec(),
    }
}

fn draw(
    renderer: &mut Renderer,
    scene: &Scene,
    mode: TerrainMode,
) -> (Vec<u8>, openeq_render::terrain::TerrainStats) {
    let gpu =
        GpuScene::build_with_terrain(renderer.device(), renderer.queue(), scene, mode).unwrap();
    let stats = gpu.terrain_stats().clone();
    renderer.set_scene(&gpu);
    renderer.render_at(
        &gpu,
        &Camera {
            pitch: 0.,
            ..Default::default()
        },
        Duration::ZERO,
    );
    (renderer.read_rgba().unwrap().2, stats)
}

fn matches_reference(renderer: &mut Renderer, scene: &Scene) {
    let (actual, stats) = draw(renderer, scene, TerrainMode::Direct);
    assert_eq!(stats.materials, 1, "{:?}", stats.fallback_reason);
    let (reference, _) = draw(renderer, scene, TerrainMode::Baked);
    let worst = actual
        .iter()
        .zip(&reference)
        .map(|(a, b)| a.abs_diff(*b))
        .max()
        .unwrap();
    assert!(
        worst <= 2,
        "paint differs from independently specified reference by {worst}"
    );
}

#[test]
#[ignore = "requires GPU"]
fn ordered_terrain_masks_ranges_and_repeats_match_independent_colors() {
    let mut renderer = Renderer::new_headless(48, 48).unwrap();
    renderer.set_environment(
        EnvironmentSettings {
            sky_enabled: false,
            ..Default::default()
        },
        None,
    );
    let mut base_only = scene([0.5; 2], [0, 255, 0], vec![]);
    base_only
        .terrain_materials
        .get_mut(&0)
        .unwrap()
        .base_texture = Some("green".into());
    matches_reference(&mut renderer, &base_only);
    let mut first = eco("green");
    first.min_height = 1000.; // First ECO is unconditional, even outside range.
    first.max_height = 2000.;
    for (u, expected) in [
        (-0.2, [0, 255, 0]),
        (0.25, [0, 255, 0]),
        (0.5, [0, 128, 128]),
        (0.75, [0, 0, 255]),
        (1.2, [0, 0, 255]),
    ] {
        matches_reference(
            &mut renderer,
            &scene(
                [u, 0.5],
                expected,
                vec![
                    paint(vec![first.clone()], &[0; 4]), // First tile mask is also ignored.
                    paint(vec![eco("blue")], &[0, 255, 0, 255]),
                ],
            ),
        );
    }
    // This wall has a 90-degree slope and varying height. A zero-width slope
    // interval is valid; out-of-range heights/slope suppress later ECO entries.
    for (lo, hi, expected) in [(90., 90., [0, 0, 255]), (0., 30., [255, 0, 0])] {
        let mut second = eco("blue");
        second.min_slope = lo;
        second.max_slope = hi;
        matches_reference(
            &mut renderer,
            &scene(
                [0.5; 2],
                expected,
                vec![paint(vec![eco("red"), second], &[])],
            ),
        );
    }
    let mut layer = eco("quadrants");
    layer.repeat = 2.;
    let mut value = scene([0.375, 0.125], [0, 255, 0], vec![paint(vec![layer], &[])]);
    let mut rgba = Vec::new();
    for _y in 0..4 {
        for x in 0..4 {
            rgba.extend(if x < 2 {
                [255, 0, 0, 255]
            } else {
                [0, 255, 0, 255]
            });
        }
    }
    // from_geometry owns its texture map; rebuild with the exact same recipe.
    let mut with_texture = Scene::from_geometry(
        value.name.clone(),
        value.materials.clone(),
        value.meshes.clone(),
        vec![
            solid("fallback", [0, 255, 0]),
            Texture {
                name: "quadrants".into(),
                width: 4,
                height: 4,
                rgba,
            },
        ],
    );
    with_texture.terrain_materials = std::mem::take(&mut value.terrain_materials);
    matches_reference(&mut renderer, &with_texture);
}

#[test]
#[ignore = "requires GPU"]
fn unsupported_or_rebound_recipes_use_the_exact_baked_fallback() {
    let mut renderer = Renderer::new_headless(32, 32).unwrap();
    for case in 0..4 {
        let mut value = scene([0.5; 2], [255, 0, 0], vec![paint(vec![eco("green")], &[])]);
        match case {
            0 => {
                value.terrain_materials.get_mut(&0).unwrap().layers[0].layers[0].detail_map =
                    "missing".into()
            }
            1 => {
                value
                    .terrain_materials
                    .get_mut(&0)
                    .unwrap()
                    .fallback_texture = "old identity".into()
            }
            2 => value.terrain_materials.get_mut(&0).unwrap().layers[0].layers[0].repeat = f32::NAN,
            3 => value
                .terrain_materials
                .get_mut(&0)
                .unwrap()
                .layers
                .push(MaterialLayer {
                    layers: vec![eco("blue")],
                    mask_size: 2,
                    mask: vec![0],
                }),
            _ => unreachable!(),
        }
        let (actual, stats) = draw(&mut renderer, &value, TerrainMode::Direct);
        let (expected, _) = draw(&mut renderer, &value, TerrainMode::Baked);
        assert_eq!(stats.materials, 0);
        assert!(stats.fallback_reason.is_some());
        assert_eq!(actual, expected, "fallback case {case}");
    }
}

#[test]
#[ignore = "requires original heightmap assets and GPU; writes /tmp/openeq-gpu-terrain"]
fn original_zones_share_details_without_changing_geometry_water_or_bounds() {
    let dir = openeq_assets::loader::default_client_dir().expect("original assets");
    let capture = std::path::Path::new("/tmp/openeq-gpu-terrain");
    std::fs::create_dir_all(capture).unwrap();
    let mut renderer = Renderer::new_headless(960, 540).unwrap();
    for zone in ["feerrott2", "deadhills", "oldcommons"] {
        let mut scene = openeq_assets::load_zone(&dir, zone).unwrap();
        scene.instances.clear(); // Isolate terrain detail and authored water.
        renderer.set_environment(
            EnvironmentSettings {
                sky_enabled: false,
                ..Default::default()
            },
            None,
        );
        let terrain = scene
            .meshes
            .iter()
            .filter(|mesh| scene.terrain_materials.contains_key(&mesh.material))
            .collect::<Vec<_>>();
        let mesh = terrain[terrain.len() / 2];
        let vertex = &mesh.vertices[(mesh.vertex_count() / 2) * 8..];
        let camera = if zone == "feerrott2" {
            Camera {
                position: [-1544., 744., 60.],
                pitch: -1.395,
                ..Default::default()
            }
        } else {
            Camera {
                position: [vertex[0], vertex[1] - 25., vertex[2] + 50.],
                pitch: -1.1,
                ..Default::default()
            }
        };
        let mut reference = None;
        for mode in [TerrainMode::Baked, TerrainMode::Direct] {
            let gpu =
                GpuScene::build_with_terrain(renderer.device(), renderer.queue(), &scene, mode)
                    .unwrap();
            let state = (
                gpu.vertices.size(),
                gpu.indices.size(),
                gpu.bounds_min,
                gpu.bounds_max,
                gpu.draws.len(),
            );
            let label = if mode == TerrainMode::Direct {
                "direct"
            } else {
                "baked"
            };
            let atlas_layers = gpu.atlas.depth_or_array_layers();
            if mode == TerrainMode::Baked {
                reference = Some((state, atlas_layers));
            } else {
                let (prior, layers) = reference.unwrap();
                assert_eq!(prior, state, "geometry changed in {zone}");
                assert_eq!(gpu.terrain_stats().materials, scene.terrain_materials.len());
                assert!(gpu.terrain_stats().detail_layers <= 8);
                assert!(atlas_layers + scene.terrain_materials.len() as u32 >= layers);
                assert!(
                    atlas_layers < layers / 2,
                    "baked tile layers were not removed"
                );
                eprintln!(
                    "{zone}: atlas {layers}->{atlas_layers}; direct {:?}",
                    gpu.terrain_stats()
                );
            }
            renderer.set_scene(&gpu);
            renderer.render_at(&gpu, &camera, Duration::from_secs(12));
            let pixels = renderer.read_rgba().unwrap().2;
            let magenta = pixels
                .chunks_exact(4)
                .filter(|p| p[0] > 180 && p[1] < 20 && p[2] > 180)
                .count();
            assert_eq!(magenta, 0, "unexpected placeholder in {zone}/{label}");
            image::RgbaImage::from_raw(960, 540, pixels)
                .unwrap()
                .save(capture.join(format!("{zone}-{label}.png")))
                .unwrap();
        }
    }
}

#[test]
#[ignore = "requires GPU"]
fn ordinary_material_sharing_terrain_fallback_keeps_its_texture_in_both_orders() {
    let mut renderer = Renderer::new_headless(48, 48).unwrap();
    renderer.set_environment(
        EnvironmentSettings {
            sky_enabled: false,
            ..Default::default()
        },
        None,
    );
    for terrain_first in [true, false] {
        let mut value = scene([0.5; 2], [255, 0, 0], vec![paint(vec![eco("green")], &[])]);
        // Identical atlas keys, but only one material owns a terrain recipe.
        value.materials.push(value.materials[0].clone());
        value.meshes[0].material = 1;
        let mut terrain_mesh = value.meshes[0].clone();
        terrain_mesh.material = 0;
        for vertex in terrain_mesh.vertices.chunks_exact_mut(8) {
            vertex[0] += 1000.;
        }
        // Keep terrain outside the view so the comparison isolates the ordinary
        // surface, while still admitting the shared terrain recipe and atlas.
        value.meshes.push(terrain_mesh);
        if !terrain_first {
            value.materials.swap(0, 1);
            for mesh in &mut value.meshes {
                mesh.material = 1 - mesh.material;
            }
            let recipe = value.terrain_materials.remove(&0).unwrap();
            value.terrain_materials.insert(1, recipe);
        }
        let (actual, stats) = draw(&mut renderer, &value, TerrainMode::Direct);
        let (expected, _) = draw(&mut renderer, &value, TerrainMode::Baked);
        assert_eq!(stats.materials, 1, "{:?}", stats.fallback_reason);
        assert!(
            expected
                .chunks_exact(4)
                .any(|pixel| pixel[0] > 80 && pixel[1] < 20 && pixel[2] < 20),
            "the ordinary red surface must be visible"
        );
        assert_eq!(
            actual, expected,
            "ordinary atlas texture changed with terrain_first={terrain_first}"
        );
    }
}

fn varying_uv_wall(value: &mut Scene) {
    let corners = [[0., 0.], [1., 0.], [1., 1.], [0., 1.]];
    for (vertex, uv) in value.meshes[0].vertices.chunks_exact_mut(8).zip(corners) {
        vertex[0] /= 10.;
        vertex[2] /= 10.;
        vertex[6..8].copy_from_slice(&uv);
    }
}

#[test]
#[ignore = "requires GPU"]
fn minified_terrain_detail_matches_checker_area_average() {
    let mut renderer = Renderer::new_headless(48, 48).unwrap();
    renderer.set_environment(
        EnvironmentSettings {
            sky_enabled: false,
            ..Default::default()
        },
        None,
    );
    let mut detail = eco("checker");
    detail.repeat = 32.;
    let mut value = scene([0.; 2], [128, 0, 128], vec![paint(vec![detail], &[])]);
    varying_uv_wall(&mut value);
    // A 2x2 red/blue checker has a known equal-area mean. Across this roughly
    // 17-pixel-wide wall, 32 repetitions put several detail texels in a pixel;
    // the explicit UV gradients must select the final, uniform mip level.
    let mut with_checker = Scene::from_geometry(
        value.name,
        value.materials,
        value.meshes,
        vec![
            solid("fallback", [128, 0, 128]),
            Texture {
                name: "checker".into(),
                width: 2,
                height: 2,
                rgba: vec![
                    255, 0, 0, 255, 0, 0, 255, 255, 0, 0, 255, 255, 255, 0, 0, 255,
                ],
            },
        ],
    );
    with_checker.terrain_materials = value.terrain_materials;
    matches_reference(&mut renderer, &with_checker);
}

#[test]
#[ignore = "requires GPU"]
fn minified_terrain_mask_matches_checker_area_average() {
    let mut renderer = Renderer::new_headless(48, 48).unwrap();
    renderer.set_environment(
        EnvironmentSettings {
            sky_enabled: false,
            ..Default::default()
        },
        None,
    );
    let size = 128;
    let mask = (0..size * size)
        .map(|index| {
            if (index % size + index / size) % 2 == 0 {
                0
            } else {
                255
            }
        })
        .collect();
    let mut value = scene(
        [0.; 2],
        [127, 0, 128],
        vec![
            paint(vec![eco("red")], &[]),
            MaterialLayer {
                layers: vec![eco("blue")],
                mask_size: size,
                mask,
            },
        ],
    );
    varying_uv_wall(&mut value);
    // Every 2x2 mask block averages to the quantized opacity 128/255. With
    // roughly seven mask texels per pixel, both selected mip levels must use
    // that average, giving 127 red and 128 blue over the whole visible wall.
    matches_reference(&mut renderer, &value);
}

#[test]
#[ignore = "requires GPU"]
fn later_terrain_ecosystem_respects_height_interval_and_tolerance() {
    let mut renderer = Renderer::new_headless(48, 48).unwrap();
    renderer.set_environment(
        EnvironmentSettings {
            sky_enabled: false,
            ..Default::default()
        },
        None,
    );
    for (height, tolerance, expected) in [
        (9., 0., [255, 0, 0]),
        (10., 0., [0, 0, 255]),
        (20., 0., [0, 0, 255]),
        (21., 0., [255, 0, 0]),
        (6., 4., [255, 0, 0]),
        (8., 4., [128, 0, 128]),
        (10., 4., [0, 0, 255]),
        (20., 4., [0, 0, 255]),
        (22., 4., [128, 0, 128]),
        (24., 4., [255, 0, 0]),
    ] {
        let mut upper = eco("blue");
        upper.min_height = 10.;
        upper.max_height = 20.;
        upper.height_tolerance = tolerance;
        let mut value = scene(
            [0.5; 2],
            expected,
            vec![paint(vec![eco("red"), upper], &[])],
        );
        // Constant-height horizontal geometry makes every fragment's expected
        // blend known independently: red outside, blue inside, purple halfway
        // through either four-unit tolerance band.
        for vertex in value.meshes[0].vertices.chunks_exact_mut(8) {
            vertex[1] = vertex[2];
            vertex[2] = height;
            vertex[3..6].copy_from_slice(&[0., 0., 1.]);
        }
        let camera = Camera {
            position: [0., -4., height + 8.],
            pitch: -8.0_f32.atan2(4.),
            ..Default::default()
        };
        let mut frames = Vec::new();
        for mode in [TerrainMode::Direct, TerrainMode::Baked] {
            let gpu =
                GpuScene::build_with_terrain(renderer.device(), renderer.queue(), &value, mode)
                    .unwrap();
            if mode == TerrainMode::Direct {
                assert_eq!(gpu.terrain_stats().materials, 1);
            }
            renderer.set_scene(&gpu);
            renderer.render_at(&gpu, &camera, Duration::ZERO);
            frames.push(renderer.read_rgba().unwrap().2);
        }
        assert!(
            frames[1]
                .chunks_exact(4)
                .any(|pixel| { (pixel[0] > 80 || pixel[2] > 80) && pixel[1] < 20 }),
            "height fixture must show the expected colored plane"
        );
        let worst = frames[0]
            .iter()
            .zip(&frames[1])
            .map(|(actual, expected)| actual.abs_diff(*expected))
            .max()
            .unwrap();
        assert!(
            worst <= 2,
            "height={height}, tolerance={tolerance}: reference differs by {worst}"
        );
    }
}
