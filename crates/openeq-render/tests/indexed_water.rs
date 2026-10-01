//! Deterministic water-coordinate checks and original bounded-surface captures.
use openeq_assets::{
    Scene, loader,
    mesh::{Geometry, Material, WaterMaterial},
    texture::Texture,
};
use openeq_render::{Camera, GpuScene, Renderer, environment::EnvironmentSettings};
use std::time::Duration;

fn renderer(width: u32, height: u32) -> Renderer {
    let mut renderer = Renderer::new_headless(width, height).unwrap();
    renderer.set_environment(
        EnvironmentSettings {
            sky_enabled: false,
            ..Default::default()
        },
        None,
    );
    renderer
}
fn upload(renderer: &Renderer, scene: &Scene) -> GpuScene {
    GpuScene::build(renderer.device(), renderer.queue(), scene).unwrap()
}
fn capture(renderer: &mut Renderer, gpu: &GpuScene, camera: &Camera, seconds: f64) -> Vec<u8> {
    renderer.set_scene(gpu);
    renderer.render_at(gpu, camera, Duration::from_secs_f64(seconds));
    renderer.read_rgba().unwrap().2
}
fn assert_close(a: &[u8], b: &[u8], description: &str) {
    assert_eq!(a.len(), b.len());
    let max = a.iter().zip(b).map(|(a, b)| a.abs_diff(*b)).max().unwrap();
    let changed = a.iter().zip(b).filter(|(a, b)| a.abs_diff(**b) > 1).count();
    assert!(
        max <= 3 && changed < a.len() / 200,
        "{description}: max channel delta {max}, {changed} channels differ by >1"
    );
}
fn changed_pixels(a: &[u8], b: &[u8]) -> usize {
    a.chunks_exact(4)
        .zip(b.chunks_exact(4))
        .filter(|(a, b)| a.iter().zip(b.iter()).any(|(a, b)| a.abs_diff(*b) > 3))
        .count()
}
fn water_scene(
    scale: Option<f32>,
    uv_offset: f32,
    uv_multiplier: f32,
    translation: [f32; 2],
) -> Scene {
    let mut vertices = Vec::new();
    for ([x, y], [u, v]) in [
        ([-40., -40.], [0., 0.]),
        ([40., -40.], [1., 0.]),
        ([40., 40.], [1., 1.]),
        ([-40., 40.], [0., 1.]),
    ] {
        vertices.extend([
            x + translation[0],
            y + translation[1],
            0.,
            0.,
            0.,
            1.,
            u * uv_multiplier + uv_offset,
            v * uv_multiplier + uv_offset,
        ]);
    }
    let mut normal = Vec::new();
    for y in 0..256 {
        for x in 0..256 {
            // Several smooth frequencies make both layer motion and UV scale
            // visible without relying on the appearance of a client texture.
            let a = std::f32::consts::TAU * x as f32 / 256.;
            let b = std::f32::consts::TAU * y as f32 / 256.;
            normal.extend([
                (127.5 + 90. * (a + 2. * b).sin()) as u8,
                (127.5 + 90. * (3. * a - b).cos()) as u8,
                255,
                255,
            ]);
        }
    }
    Scene::from_geometry(
        "indexed water UV fixture".into(),
        vec![Material {
            textures: vec!["constant diffuse".into()],
            normal_map: Some("patterned normal".into()),
            water: Some(WaterMaterial {
                color1: [0.04, 0.15, 0.25, 1.],
                color2: [0.04, 0.15, 0.25, 1.],
                reflection_color: [1.; 4],
                fresnel_bias: 0.1,
                fresnel_power: 3.,
                reflection_amount: 0.8,
                environment_map: None,
                indexed_uv_scale: scale,
            }),
            flags: 0,
            anim_speed: 0,
            alpha_mask: false,
            transparent: false,
            additive: false,
            emissive: false,
            clamp_uv: false,
            waterfall: None,
            uv_encoding: Default::default(),
        }],
        vec![Geometry {
            vertices,
            indices: vec![0, 1, 2, 0, 2, 3],
            material: 0,
            collidable: false,
        }],
        vec![
            Texture {
                name: "constant diffuse".into(),
                width: 1,
                height: 1,
                rgba: vec![128, 128, 128, 255],
            },
            Texture {
                name: "patterned normal".into(),
                width: 256,
                height: 256,
                rgba: normal,
            },
        ],
    )
}

#[test]
#[ignore = "requires GPU"]
fn indexed_uv_repeat_scale_and_two_native_layers_are_distinct_from_legacy_water() {
    let mut renderer = renderer(160, 160);
    let camera = Camera {
        position: [0., -32., 35.],
        yaw: 0.,
        pitch: -0.85,
        ..Default::default()
    };
    let mut draw = |scale, offset, multiplier, translation: [f32; 2], seconds| {
        let scene = water_scene(scale, offset, multiplier, translation);
        let gpu = upload(&renderer, &scene);
        let camera = Camera {
            position: [
                camera.position[0] + translation[0],
                camera.position[1] + translation[1],
                camera.position[2],
            ],
            ..camera
        };
        capture(&mut renderer, &gpu, &camera, seconds)
    };
    let base = draw(Some(1.), 0., 1., [0.; 2], 0.);
    assert!(
        base.chunks_exact(4).map(|p| p[0]).max().unwrap()
            - base.chunks_exact(4).map(|p| p[0]).min().unwrap()
            > 20,
        "water fixture must produce visible normal-map shading"
    );
    for offset in [-1., 1.] {
        assert_close(
            &base,
            &draw(Some(1.), offset, 1., [0.; 2], 0.),
            "integer asset-UV repeat",
        );
    }
    assert_close(
        &base,
        &draw(Some(1.), 0., 1., [0.; 2], 100.),
        "native100 second repeat period",
    );
    assert!(
        changed_pixels(&base, &draw(Some(1.), 0.25, 1., [0.; 2], 0.)) > 1000,
        "fractional asset UV shift must change ripples"
    );
    for translation in [[256., 384.], [-512., -768.]] {
        assert_close(
            &base,
            &draw(Some(1.), 0., 1., translation, 0.),
            "indexed phase must not depend on signed world coordinates",
        );
    }
    assert_close(
        &draw(Some(1.25), 0., 1., [0.; 2], 7.),
        &draw(Some(1.), 0., 1.25, [0.; 2], 7.),
        "fractional authored scale including sampling derivatives",
    );
    // At dt=100/7 and du=2/7, layer1's shift is zero and layer2's
    // shift is exactly one period: du-.02dt=0, 2du+.03dt=1.
    // This fails with the legacy1.37 second layer or altered scroll rates.
    assert_close(
        &base,
        &draw(Some(1.), 2. / 7., 1., [0.; 2], 100. / 7.),
        "two native layer frequencies and opposite scrolling",
    );
    let legacy = draw(None, 0., 1., [0.; 2], 7.);
    assert_close(
        &legacy,
        &draw(None, 0.31, 2.75, [0.; 2], 7.),
        "legacy water must continue ignoring asset UVs",
    );
    assert!(
        changed_pixels(&legacy, &draw(Some(1.), 0., 1., [0.; 2], 7.)) > 1000,
        "explicit indexed mode must select different normal coordinates"
    );
}

#[test]
#[ignore = "requires GPU"]
fn indexed_water_keeps_integer_millisecond_phase_after_long_uptime_and_clock_wrap() {
    let mut renderer = renderer(160, 160);
    let scene = water_scene(Some(1.), 0., 1., [0.; 2]);
    let gpu = upload(&renderer, &scene);
    let camera = Camera {
        position: [0., -32., 35.],
        yaw: 0.,
        pitch: -0.85,
        ..Default::default()
    };
    let mut draw = |milliseconds| {
        renderer.render_at(&gpu, &camera, Duration::from_millis(milliseconds));
        renderer.read_rgba().unwrap().2
    };
    let phase_ms = 37_777;
    let expected = draw(phase_ms);
    assert!(changed_pixels(&expected, &draw(phase_ms + 2_000)) > 1000);
    // Native binder reduces unsigned integer milliseconds before f32 conversion.
    // The old full-uptime float uniform loses low bits before the modulo.
    for time in [
        phase_ms + 100_000 * 1_000,
        phase_ms + 100_000 * 40_000,
        phase_ms + (1_u64 << 32),
        phase_ms + 2 * (1_u64 << 32),
    ] {
        assert!(expected == draw(time), "effect phase drift at {time} ms");
    }
}

fn look_at(position: [f32; 3], target: [f32; 3]) -> Camera {
    let d = glam::Vec3::from(target) - glam::Vec3::from(position);
    Camera {
        position,
        yaw: d.x.atan2(d.y),
        pitch: d.z.atan2(d.truncate().length()),
        ..Default::default()
    }
}
fn indexed(scene: &Scene, mesh: &Geometry) -> bool {
    scene.materials[mesh.material]
        .water
        .as_ref()
        .is_some_and(|w| w.indexed_uv_scale.is_some())
}
fn save(name: &str, rgba: Vec<u8>) {
    let directory = std::path::Path::new("/tmp/openeq-indexed-water");
    std::fs::create_dir_all(directory).unwrap();
    image::RgbaImage::from_raw(960, 540, rgba)
        .unwrap()
        .save(directory.join(name))
        .unwrap();
}

#[test]
#[ignore = "requires original Feerrott2 assets and GPU; writes /tmp/openeq-indexed-water"]
fn original_feerrott_pond_edge_seam_and_occluded_tile_use_bounded_surfaces() {
    let base = loader::default_client_dir().expect("original EverQuest assets");
    assert!(base.join("feerrott2.eqg").is_file());
    let mut scene = loader::load_zone(base, "feerrott2").unwrap();
    assert_eq!(
        scene.meshes.iter().filter(|m| indexed(&scene, m)).count(),
        65
    );
    // Isolate the original terrain and authored water. Otherwise nearby trees
    // obscure the exact shoreline, and foliage could mask the depth-occlusion
    // fixture without proving that solid terrain hides tile271.
    scene.instances.clear();
    let mut renderer = renderer(960, 540);
    let full = upload(&renderer, &scene);
    let edge = look_at([-704., -2880., -30.], [-704., -2848., -50.]);
    let seam = look_at([-768., -2860., 10.], [-768., -2912., -50.]);
    let hidden = look_at([-1544., 744., 60.], [-1544., 760., -30.]);
    let edge_pixels = capture(&mut renderer, &full, &edge, 12.);
    let seam_pixels = capture(&mut renderer, &full, &seam, 12.);
    let hidden_pixels = capture(&mut renderer, &full, &hidden, 12.);
    let hidden_mesh = scene
        .meshes
        .iter()
        .position(|m| {
            indexed(&scene, m)
                && m.vertices.chunks_exact(8).all(|v| {
                    v[0] >= -1552. && v[0] <= -1536. && v[1] >= 752. && v[1] <= 768. && v[2] == -30.
                })
        })
        .expect("tile271 water rectangle");
    assert_eq!(scene.meshes[hidden_mesh].indices.len(), 12);
    scene.meshes[hidden_mesh].indices.clear();
    let without_hidden = upload(&renderer, &scene);
    assert_eq!(
        hidden_pixels,
        capture(&mut renderer, &without_hidden, &hidden, 12.),
        "solid terrain must occlude tile271 without clipping its authored geometry"
    );
    for mesh in &mut scene.meshes {
        if scene.materials[mesh.material]
            .water
            .as_ref()
            .is_some_and(|w| w.indexed_uv_scale.is_some())
        {
            mesh.indices.clear();
        }
    }
    let dry = upload(&renderer, &scene);
    let dry_edge = capture(&mut renderer, &dry, &edge, 12.);
    let dry_seam = capture(&mut renderer, &dry, &seam, 12.);
    assert!(
        changed_pixels(&edge_pixels, &dry_edge) > 100,
        "authored pond edge must appear in capture"
    );
    assert!(
        changed_pixels(&seam_pixels, &dry_seam) > 100,
        "authored seam must appear in capture"
    );
    save("pond-edge.png", edge_pixels);
    save("pond-edge-without-water.png", dry_edge);
    save("seam.png", seam_pixels);
    save("seam-without-water.png", dry_seam);
    save("occluded-tile271.png", hidden_pixels);
}
