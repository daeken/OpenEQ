//! Pixel parity with the brute-force light loop. Grid math has separate unit
//! tests; these exercise both deferred opaque and forward fractional surfaces.
use super::*;
use openeq_assets::{
    Scene,
    loader::{self, Light},
    mesh::{Geometry, Material},
    texture::Texture,
};

fn floor(z: f32, transparent: bool) -> Scene {
    Scene::from_geometry(
        "light-grid parity plane".into(),
        vec![Material {
            textures: vec!["gray".into()],
            normal_map: None,
            water: None,
            flags: 0,
            anim_speed: 0,
            alpha_mask: transparent,
            transparent,
            additive: false,
            emissive: false,
            clamp_uv: false,
            uv_encoding: Default::default(),
        }],
        vec![Geometry {
            vertices: vec![
                -900., -900., z, 0., 0., 1., 0., 0., 900., -900., z, 0., 0., 1., 1., 0., 900.,
                900., z, 0., 0., 1., 1., 1., -900., 900., z, 0., 0., 1., 0., 1.,
            ],
            indices: vec![0, 1, 2, 0, 2, 3],
            material: 0,
            collidable: false,
        }],
        vec![Texture {
            name: "gray".into(),
            width: 1,
            height: 1,
            rgba: vec![95, 95, 95, if transparent { 100 } else { 255 }],
        }],
    )
}

fn upload(renderer: &Renderer, scene: &Scene) -> GpuScene {
    GpuScene::build(renderer.device(), renderer.queue(), scene).unwrap()
}

fn render(
    renderer: &mut Renderer,
    scene: &GpuScene,
    camera: &Camera,
    actors: &[&GpuActor],
    grid: bool,
) -> Vec<u8> {
    scene.set_light_grid_enabled(renderer.queue(), grid);
    renderer.set_scene(scene);
    renderer.render_with_actors(scene, camera, actors);
    renderer.read_rgba().unwrap().2
}

fn compare(grid: &[u8], brute: &[u8], label: &str) {
    assert_eq!(grid.len(), brute.len());
    let worst = grid
        .iter()
        .zip(brute)
        .map(|(a, b)| a.abs_diff(*b))
        .max()
        .unwrap();
    // GPU compilers may round equivalent loop branches at an 8-bit boundary.
    // A single channel unit is the entire allowance; no changed pixels are
    // hidden by an average error threshold or a broad image similarity score.
    assert!(worst <= 1, "{label}: grid changed a channel by {worst}");
    eprintln!("{label}: maximum RGBA channel difference {worst}");
}

#[test]
fn grid_matches_every_pixel_across_cell_edges_and_outside_for_opaque_and_soft_alpha() {
    let Ok(mut renderer) = Renderer::new_headless(192, 144) else {
        return;
    };
    renderer.set_environment(
        EnvironmentSettings {
            sky_enabled: false,
            ..Default::default()
        },
        None,
    );
    let mut source = floor(0., false);
    // Asymmetric centers straddle positive/negative cell boundaries; unequal
    // radii and colors make missed neighbors and reordered contributions visible.
    for (i, x) in [-193., -65., -1., 63., 127., 255.].into_iter().enumerate() {
        for (j, y) in [-129., -1., 127.].into_iter().enumerate() {
            source.lights.push(Light {
                position: [x, y, 12. + (i + j) as f32],
                radius: [28., 63.9, 96.][(i + j) % 3],
                color: [0.08 + i as f32 * 0.025, 0.07 + j as f32 * 0.04, 0.19],
                attenuation: 0.8,
                eqg_source: None,
            });
        }
    }
    // Same XY cell, far outside the light sphere in Z: the grid must retain
    // the original three-dimensional radius rejection rather than lighting it.
    source.lights.push(Light {
        position: [0., 0., 500.],
        radius: 30.,
        color: [2., 0., 0.],
        attenuation: 1.,
        eqg_source: None,
    });
    let world = upload(&renderer, &source);
    source.lights.clear();
    let unlit = upload(&renderer, &source);
    let alpha = renderer.prepare_actor(upload(&renderer, &floor(0.6, true)));
    for transparent in [false, true] {
        let actors: Vec<_> = if transparent { vec![&alpha] } else { vec![] };
        for (view, [x, y]) in [
            [-129., -65.],
            [-1., 0.],
            [63.9, 63.9],
            [127.9, 127.9],
            [750., 750.],
            [-750., -750.],
        ]
        .into_iter()
        .enumerate()
        {
            let camera = Camera {
                position: [x, y - 45., 125.],
                yaw: 0.,
                pitch: -1.2,
                fov_y: 50f32.to_radians(),
            };
            let brute = render(&mut renderer, &world, &camera, &actors, false);
            let grid = render(&mut renderer, &world, &camera, &actors, true);
            compare(
                &grid,
                &brute,
                &format!("synthetic alpha={transparent} view={view}"),
            );
            let no_lights = render(&mut renderer, &unlit, &camera, &actors, true);
            let changed = brute
                .chunks_exact(4)
                .zip(no_lights.chunks_exact(4))
                .filter(|(a, b)| a.iter().zip(*b).any(|(a, b)| a.abs_diff(*b) > 2))
                .count();
            if view < 4 {
                assert!(changed > 50, "view {view} did not exercise point lighting");
            } else {
                assert_eq!(
                    changed, 0,
                    "outside-grid fixture unexpectedly intersects a light"
                );
            }
        }
    }
}

#[test]
#[ignore = "requires original Plane of Knowledge assets and GPU"]
fn original_poknowledge_views_match_brute_force_with_static_materials() {
    let base = loader::default_client_dir().expect("original client directory");
    let mut source = loader::load_zone(&base, "poknowledge").unwrap();
    assert!(source.lights.len() > 100, "original PoK light data missing");
    // Freeze time-dependent material behavior for an exact lighting comparison.
    // The fixture retains original meshes/textures; water-specific animation is
    // removed because it is unrelated to point-light indexing.
    for material in &mut source.materials {
        material.textures.truncate(1);
        material.anim_speed = 0;
        material.water = None;
    }
    let mut renderer = Renderer::new_headless(480, 270).unwrap();
    renderer.set_environment(
        EnvironmentSettings {
            sky_enabled: false,
            ..Default::default()
        },
        None,
    );
    let world = upload(&renderer, &source);
    let views = [
        Camera {
            position: [-315., 915., -85.],
            yaw: 0.,
            pitch: -0.12,
            fov_y: 60f32.to_radians(),
        },
        Camera {
            position: [-280., 945., -87.],
            yaw: -std::f32::consts::FRAC_PI_2,
            pitch: -0.08,
            fov_y: 60f32.to_radians(),
        },
        Camera {
            position: [-360., 945., -87.],
            yaw: std::f32::consts::FRAC_PI_2,
            pitch: -0.08,
            fov_y: 60f32.to_radians(),
        },
    ];
    for (i, camera) in views.into_iter().enumerate() {
        let brute = render(&mut renderer, &world, &camera, &[], false);
        let grid = render(&mut renderer, &world, &camera, &[], true);
        compare(&grid, &brute, &format!("original poknowledge view={i}"));
        let colors: std::collections::BTreeSet<_> =
            grid.chunks_exact(4).map(|p| [p[0], p[1], p[2]]).collect();
        assert!(
            colors.len() > 100,
            "PoK view {i} did not show textured geometry"
        );
        image::save_buffer(
            format!("/tmp/openeq-light-grid-poknowledge-{i}.png"),
            &grid,
            480,
            270,
            image::ColorType::Rgba8,
        )
        .unwrap();
    }
}
