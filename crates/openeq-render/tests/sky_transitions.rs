//! Original day-key interpolation reaches the GPU without changing table layout.
use openeq_assets::{
    Scene,
    environment::{SkyColorMapLayout, load_sky},
};
use openeq_render::{Camera, GpuScene, Renderer, environment::EnvironmentSettings};
use std::path::PathBuf;

#[test]
#[ignore = "requires original EverQuest sky assets and a GPU"]
fn original_dawn_blend_matches_independent_byte_table_through_gpu() {
    let base = PathBuf::from(std::env::var_os("EQ_DIR").expect("set EQ_DIR"));
    let mut old = load_sky(&base, "poknowledge", 0.1).unwrap();
    let mut new = load_sky(&base, "poknowledge", 0.27).unwrap();
    let mut actual = load_sky(&base, "poknowledge", 0.25).unwrap();
    // The installed DefaultClear dawn starts at .234985 with .034988 duration.
    // Compute its native fixed-tick weight independently of the loader.
    let tick = (0.25_f32 * 65536.) as u32;
    let start = (0.234985_f32 * 65536.) as u32;
    let duration = (0.034988_f32 * 65536.) as u32;
    let weight = (tick - start) * 255 / duration;
    assert!(weight > 0 && weight < 255);
    let mut expected = actual.clone();
    expected.color_map.rgba = old
        .color_map
        .rgba
        .iter()
        .zip(&new.color_map.rgba)
        .map(|(&a, &b)| ((u32::from(a) * (255 - weight) + u32::from(b) * weight) >> 8) as u8)
        .collect();
    assert_eq!(actual.color_map.rgba, expected.color_map.rgba);
    for sky in [&mut old, &mut new, &mut actual, &mut expected] {
        sky.cloud_texture = None;
        sky.cloud_color_map = None;
        sky.cloud_color_map_layout = SkyColorMapLayout::FullTexture;
    }
    let mut renderer = Renderer::new_headless(96, 96).expect("GPU required");
    let scene = Scene::from_geometry("dawn sky".into(), vec![], vec![], vec![]);
    let gpu = GpuScene::build(renderer.device(), renderer.queue(), &scene).unwrap();
    for (yaw, pitch) in [(0., 0.4), (1.7, 1.2), (4.1, 1.48)] {
        let camera = Camera {
            yaw,
            pitch,
            ..Default::default()
        };
        let mut frames = Vec::new();
        for sky in [&old, &new, &actual, &expected] {
            renderer.set_environment(EnvironmentSettings::default(), Some(sky));
            renderer.render(&gpu, &camera);
            frames.push(renderer.read_rgba().unwrap().2);
        }
        assert_eq!(frames[2], frames[3], "native blend upload mismatch");
        assert_ne!(frames[2], frames[0], "dawn remained on the night map");
        assert_ne!(frames[2], frames[1], "dawn jumped to the next map");
        // Lighting swatches still must not leak into the visible dome after
        // interpolation. Poison the complete excluded domain independently.
        let mut poisoned = actual.clone();
        for y in 0..32 {
            for x in 0..32 {
                if x == 31 || y >= 30 || ((y == 0 || y == 29) && x != 0) {
                    let offset = (y * 32 + x) * 4;
                    poisoned.color_map.rgba[offset..offset + 4]
                        .copy_from_slice(&[255, 0, 255, 255]);
                }
            }
        }
        renderer.set_environment(EnvironmentSettings::default(), Some(&poisoned));
        renderer.render(&gpu, &camera);
        assert_eq!(
            frames[2],
            renderer.read_rgba().unwrap().2,
            "auxiliary sky colors leaked"
        );
    }
}
