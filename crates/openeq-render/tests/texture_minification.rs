//! Repeating opaque textures should average subpixel detail without losing
//! nearby detail. Synthetic fixtures require no original client assets.
use std::time::Duration;

use openeq_assets::{
    Scene,
    mesh::{Geometry, Material},
    texture::Texture,
};
use openeq_render::{Camera, GpuScene, Renderer, environment::EnvironmentSettings};

const SIZE: u32 = 128;

fn striped_plane(repeat: f32) -> Scene {
    let mut rgba = Vec::new();
    for _ in 0..256 {
        for x in 0..256 {
            let value = if x / 4 % 2 == 0 { 0 } else { 255 };
            rgba.extend([value, value, value, 255]);
        }
    }
    let mut vertices = Vec::new();
    for (x, z, u, v) in [
        (-4., -4., 0., 0.),
        (4., -4., repeat, 0.),
        (4., 4., repeat, repeat),
        (-4., 4., 0., repeat),
    ] {
        vertices.extend([x, 4., z, 0., -1., 0., u, v]);
    }
    Scene::from_geometry(
        "opaque minification fixture".into(),
        vec![Material {
            textures: vec!["stripes".into()],
            normal_map: None,
            water: None,
            flags: 0,
            anim_speed: 0,
            alpha_mask: false,
            transparent: false,
            additive: false,
            emissive: true,
            clamp_uv: false,
        }],
        vec![Geometry {
            vertices,
            indices: vec![0, 1, 2, 0, 2, 3],
            material: 0,
            collidable: false,
        }],
        vec![Texture {
            name: "stripes".into(),
            width: 256,
            height: 256,
            rgba,
        }],
    )
}

fn capture(renderer: &mut Renderer, scene: &GpuScene, x: f32) -> Vec<u8> {
    renderer.set_scene(scene);
    renderer.render_at(
        scene,
        &Camera {
            position: [x, 0., 0.],
            yaw: 0.,
            pitch: 0.,
            fov_y: std::f32::consts::FRAC_PI_2,
        },
        Duration::ZERO,
    );
    renderer.read_rgba().unwrap().2
}

fn center_red(pixels: &[u8]) -> Vec<u8> {
    (32..96)
        .flat_map(|y| (32..96).map(move |x| pixels[((y * SIZE + x) * 4) as usize]))
        .collect()
}

#[test]
#[ignore = "requires GPU"]
fn opaque_repeated_diffuse_filters_subpixels_and_preserves_magnified_detail() {
    let mut renderer = Renderer::new_headless(SIZE, SIZE).unwrap();
    renderer.set_environment(
        EnvironmentSettings {
            fog_enabled: false,
            sky_enabled: false,
            ..Default::default()
        },
        None,
    );

    // About 25 source texels per screen pixel: the four-texel-wide stripes
    // cannot be resolved. Moving a fraction of a pixel must not make them
    // strobe. Several nonintegral repeat rates avoid a lucky sample phase.
    for repeat in [11.3, 12.7, 15.1] {
        let source = striped_plane(repeat);
        let gpu = GpuScene::build(renderer.device(), renderer.queue(), &source).unwrap();
        let first = center_red(&capture(&mut renderer, &gpu, 0.));
        let shifted = center_red(&capture(&mut renderer, &gpu, 0.017));
        let range = first.iter().max().unwrap() - first.iter().min().unwrap();
        let motion = first
            .iter()
            .zip(&shifted)
            .map(|(a, b)| f32::from(a.abs_diff(*b)))
            .sum::<f32>()
            / first.len() as f32;
        let mean = first.iter().map(|x| f32::from(*x)).sum::<f32>() / first.len() as f32;
        eprintln!("repeat={repeat} range={range} motion_mean={motion:.3} mean={mean:.3}");
        // Existing CPU mips use a nonperiodic triangle kernel at tile edges,
        // so allow a small seam residual, but not black/white aliasing.
        assert!(
            range <= 16,
            "unresolved stripes alias at repeat {repeat}: range {range}"
        );
        assert!(motion <= 2., "subpixel camera motion strobes: {motion}");
        // The existing CPU mip chain averages stored sRGB bytes. Its neutral
        // gray should remain visible; this check is not a linear-light claim.
        assert!((mean - 128.).abs() < 5., "unexpected mip color: {mean}");
    }

    let source = striped_plane(0.125);
    let gpu = GpuScene::build(renderer.device(), renderer.queue(), &source).unwrap();
    let nearby = center_red(&capture(&mut renderer, &gpu, 0.));
    assert!(nearby.iter().filter(|&&x| x < 8).count() > nearby.len() / 4);
    assert!(nearby.iter().filter(|&&x| x > 247).count() > nearby.len() / 4);
}
