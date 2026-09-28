//! Pixel-level regressions; use installed client data and a real GPU when present.

use super::*;
use openeq_assets::{Scene, loader};

fn anguish() -> Option<(Scene, Renderer)> {
    let dir = loader::default_client_dir()?;
    if !dir.join("anguish.eqg").is_file() {
        return None;
    }
    let scene = loader::load_zone(dir, "anguish").expect("Anguish should load");
    let renderer = Renderer::new_headless(320, 180).ok()?;
    Some((scene, renderer))
}

#[test]
fn flat_ground_does_not_reveal_the_camera_shadow_rectangle() {
    let Some((mut scene, mut renderer)) = anguish() else {
        return;
    };
    let water = scene
        .materials
        .iter()
        .position(|material| material.water.is_some())
        .expect("Anguish has a large flat water plane");
    scene.meshes.retain(|mesh| mesh.material == water);
    scene.objects.clear();
    scene.instances.clear();
    scene.lights.clear();
    // A constant magenta texture makes lighting differences easy to measure.
    // Isolate the plane so no legitimate caster can obscure the result.
    scene.materials[water].water = None;
    scene.materials[water].textures.clear();
    let gpu = GpuScene::build(renderer.device(), renderer.queue(), &scene).unwrap();
    for position in [
        [-800.0, -1500.0, -238.0],
        [100.0, -1200.0, -238.0],
        [400.0, -900.0, -238.0],
    ] {
        let camera = Camera {
            position,
            pitch: -50f32.to_radians(),
            ..Default::default()
        };
        renderer.render(&gpu, &camera);
        let (_, _, pixels) = renderer.read_rgba().unwrap();
        let dark = pixels
            .chunks_exact(4)
            .filter(|pixel| pixel[0] < 220 || pixel[2] < 220)
            .count();
        assert_eq!(
            dark, 0,
            "unoccluded ground must stay sunlit at {position:?}"
        );
    }
}

#[test]
fn anguish_water_renders_without_magenta_and_animates() {
    let Some((scene, mut renderer)) = anguish() else {
        return;
    };
    let gpu = GpuScene::build(renderer.device(), renderer.queue(), &scene).unwrap();
    let camera = Camera {
        position: [-800.0, -1500.0, 250.0],
        pitch: -15f32.to_radians(),
        ..Default::default()
    };
    renderer.render(&gpu, &camera);
    let (_, _, first) = renderer.read_rgba().unwrap();
    // Advance animation time without blocking the test or changing the camera.
    renderer.start -= std::time::Duration::from_secs(2);
    renderer.render(&gpu, &camera);
    let (_, _, second) = renderer.read_rgba().unwrap();
    for pixels in [&first, &second] {
        assert!(
            !pixels
                .chunks_exact(4)
                .any(|p| p[0] > 180 && p[1] < 20 && p[2] > 180),
            "the water plane must not render placeholder magenta"
        );
    }
    // The lower half is predominantly water in this view.
    let animated = first
        .chunks_exact(4)
        .zip(second.chunks_exact(4))
        .skip(320 * 90)
        .filter(|(a, b)| a != b)
        .count();
    assert!(
        animated > 1000,
        "water should animate, only {animated} pixels changed"
    );
}
