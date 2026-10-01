use std::sync::Arc;

use openeq_assets::{
    Scene,
    mesh::{Geometry, Material},
    spell_effects::SpellEffectCatalog,
    texture::Texture,
};
use openeq_render::{
    Camera, GpuScene, Renderer,
    environment::EnvironmentSettings,
    particles::{
        MAX_PARTICLE_TEXTURES, MAX_PARTICLES, ParticleBlend, ParticleFrame, ParticleInstance,
    },
};

fn color_texture(name: &str, rgba: [u8; 4]) -> Texture {
    Texture {
        name: name.into(),
        width: 1,
        height: 1,
        rgba: rgba.to_vec(),
    }
}

fn stage(renderer: &Renderer, y: f32, color: [u8; 4]) -> GpuScene {
    let scene = Scene::from_geometry(
        "particle stage".into(),
        vec![Material {
            textures: vec!["stage".into()],
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
            vertices: vec![
                -20., y, -20., 0., -1., 0., 0., 0., 20., y, -20., 0., -1., 0., 1., 0., 20., y, 20.,
                0., -1., 0., 1., 1., -20., y, 20., 0., -1., 0., 0., 1.,
            ],
            indices: vec![0, 1, 2, 0, 2, 3],
            material: 0,
            collidable: false,
        }],
        vec![color_texture("stage", color)],
    );
    GpuScene::build(renderer.device(), renderer.queue(), &scene).unwrap()
}

fn render(renderer: &mut Renderer, scene: &GpuScene, camera: Camera) -> (u32, u32, Vec<u8>) {
    renderer.set_scene(scene);
    renderer.render(scene, &camera);
    renderer.read_rgba().unwrap()
}

fn center(image: &(u32, u32, Vec<u8>)) -> [u8; 4] {
    let offset = ((image.1 / 2 * image.0 + image.0 / 2) * 4) as usize;
    image.2[offset..offset + 4].try_into().unwrap()
}

fn linear(value: u8) -> f32 {
    let value = f32::from(value) / 255.;
    if value <= 0.04045 {
        value / 12.92
    } else {
        ((value + 0.055) / 1.055).powf(2.4)
    }
}

fn camera() -> Camera {
    Camera {
        pitch: 0.,
        ..Default::default()
    }
}

#[test]
fn particles_blend_original_alpha_select_textures_sort_and_obey_opaque_depth() {
    let Ok(mut renderer) = Renderer::new_headless(64, 64) else {
        return;
    };
    let world = stage(&renderer, 4., [0, 0, 255, 255]);
    let mut frame = ParticleFrame {
        textures: Arc::new(vec![
            color_texture("red", [255, 0, 0, 64]),
            color_texture("green", [0, 255, 0, 128]),
        ]),
        instances: vec![ParticleInstance {
            position: [0., 2., 0.],
            size: [8.; 2],
            ..Default::default()
        }],
    };
    assert_eq!(renderer.set_particles(&frame).rendered, 1);
    let pixel = center(&render(&mut renderer, &world, camera()));
    assert!(
        (linear(pixel[0]) - 64. / 255.).abs() < 0.012,
        "alpha red {pixel:?}"
    );
    assert!(
        (linear(pixel[2]) - (1. - 64. / 255.)).abs() < 0.012,
        "alpha blue {pixel:?}"
    );
    frame.instances[0].blend = ParticleBlend::Additive;
    renderer.set_particles(&frame);
    let pixel = center(&render(&mut renderer, &world, camera()));
    assert!((linear(pixel[0]) - 64. / 255.).abs() < 0.012);
    assert_eq!(pixel[2], 255, "additive darkened background: {pixel:?}");

    frame.instances[0].blend = ParticleBlend::Alpha;
    frame.instances.push(ParticleInstance {
        position: [0., 2.5, 0.],
        size: [8.; 2],
        texture: 1,
        ..Default::default()
    });
    renderer.set_particles(&frame);
    let sorted = center(&render(&mut renderer, &world, camera()));
    frame.instances.reverse();
    renderer.set_particles(&frame);
    let reversed = center(&render(&mut renderer, &world, camera()));
    assert_eq!(
        sorted, reversed,
        "alpha order followed submission rather than camera depth"
    );
    let red = 64. / 255.;
    let green = 128. / 255.;
    assert!(
        (linear(sorted[1]) - green * (1. - red)).abs() < 0.012,
        "rear green blend incorrect: {sorted:?}"
    );

    let wall = stage(&renderer, 1., [128, 128, 128, 255]);
    assert_eq!(
        center(&render(&mut renderer, &wall, camera())),
        [128, 128, 128, 255],
        "particles leaked through opaque depth"
    );
    renderer.clear_particles();
    assert_eq!(
        center(&render(&mut renderer, &world, camera())),
        [0, 0, 255, 255]
    );
}

#[test]
fn particle_flipbooks_billboards_fog_resize_and_ui_are_consistent() {
    let Ok(mut renderer) = Renderer::new_headless(64, 64) else {
        return;
    };
    let world = stage(&renderer, 6., [0, 0, 0, 255]);
    let mut frame = ParticleFrame {
        textures: Arc::new(vec![Texture {
            name: "two frames".into(),
            width: 2,
            height: 1,
            rgba: vec![255, 0, 0, 255, 0, 255, 0, 255],
        }]),
        instances: vec![ParticleInstance {
            position: [0., 2., 0.],
            size: [8.; 2],
            uv_rect: [0., 0., 0.4, 1.],
            ..Default::default()
        }],
    };
    renderer.set_particles(&frame);
    let red = center(&render(&mut renderer, &world, camera()));
    assert!(red[0] > 240 && red[1] < 20, "first frame: {red:?}");
    frame.instances[0].uv_rect = [0.6, 0., 1., 1.];
    renderer.set_particles(&frame);
    let green = center(&render(&mut renderer, &world, camera()));
    assert!(green[1] > 240 && green[0] < 20, "second frame: {green:?}");
    renderer.resize(80, 48);
    let image = render(&mut renderer, &world, camera());
    assert_eq!((image.0, image.1), (80, 48));
    assert_eq!(center(&image), green);

    // Rotating the camera still shows a face-on sprite in a different EQ axis.
    frame.instances[0].position = [2., 0., 0.];
    frame.instances[0].size = [2., 2.];
    renderer.set_particles(&frame);
    let turned = center(&render(
        &mut renderer,
        &world,
        Camera {
            yaw: std::f32::consts::FRAC_PI_2,
            ..camera()
        },
    ));
    assert!(
        turned[1] > 240 && turned[0] < 20,
        "billboard did not face camera: {turned:?}"
    );
    frame.instances[0].position = [0., 2., 0.];
    frame.instances[0].size = [8.; 2];
    renderer.set_environment(
        EnvironmentSettings {
            fog_enabled: true,
            fog_start: 0.,
            fog_end: 1.,
            fog_color: [0.2, 0.4, 0.6],
            ..Default::default()
        },
        None,
    );
    for blend in [ParticleBlend::Alpha, ParticleBlend::Additive] {
        frame.instances[0].blend = blend;
        renderer.set_particles(&frame);
        let fogged = center(&render(&mut renderer, &world, camera()));
        assert!(
            fogged[..3]
                .iter()
                .zip([51, 102, 153])
                .all(|(x, y)| x.abs_diff(y) <= 2),
            "{blend:?} ignored fog: {fogged:?}"
        );
    }
    renderer.set_environment(EnvironmentSettings::default(), None);
    let rect = openeq_ui::Rect::new(0., 0., 80., 48.);
    renderer.set_ui(&openeq_ui::UiFrame {
        commands: vec![openeq_ui::DrawCommand::Fill {
            rect,
            clip: rect,
            color: [0, 0, 255, 255],
        }],
        ..Default::default()
    });
    assert_eq!(
        center(&render(&mut renderer, &world, camera())),
        [0, 0, 255, 255],
        "particle covered UI"
    );
}

#[test]
fn particle_caps_and_missing_textures_are_explicit_without_placeholders() {
    let Ok(mut renderer) = Renderer::new_headless(32, 32) else {
        return;
    };
    let frame = ParticleFrame {
        textures: Arc::new(vec![color_texture("original", [255; 4])]),
        instances: vec![ParticleInstance::default(); MAX_PARTICLES + 19],
    };
    let stats = renderer.set_particles(&frame);
    assert_eq!(stats.rendered, MAX_PARTICLES);
    assert_eq!(stats.over_capacity, 19);
    let invalid = ParticleInstance {
        position: [f32::NAN, 0., 0.],
        ..Default::default()
    };
    let missing = ParticleInstance {
        texture: MAX_PARTICLE_TEXTURES as u32,
        ..Default::default()
    };
    let malformed = Texture {
        name: "malformed".into(),
        width: 1,
        height: 1,
        rgba: vec![],
    };
    let bad_frame = ParticleFrame {
        textures: Arc::new(vec![malformed]),
        instances: vec![invalid, missing, ParticleInstance::default()],
    };
    let stats = renderer.set_particles(&bad_frame);
    assert_eq!(stats.invalid, 1);
    assert_eq!(stats.missing_texture, 2);
    assert_eq!(stats.rendered, 0);
    let world = stage(&renderer, 4., [30, 50, 70, 255]);
    assert_eq!(
        center(&render(&mut renderer, &world, camera())),
        [30, 50, 70, 255],
        "missing texture created a placeholder"
    );
    renderer.clear_particles();
    assert_eq!(renderer.particle_stats().submitted, 0);
}

#[test]
fn particles_sample_both_texture_arrays_at_the_boundary_and_last_slot() {
    let Ok(mut renderer) = Renderer::new_headless(32, 32) else {
        return;
    };
    let colors = [[255, 0, 0, 255], [0, 255, 0, 255], [0, 0, 255, 255]];
    let slots = [255, 256, MAX_PARTICLE_TEXTURES - 1];
    let mut textures = vec![color_texture("unused", [0; 4]); MAX_PARTICLE_TEXTURES];
    for (slot, color) in slots.into_iter().zip(colors) {
        textures[slot] = color_texture("boundary", color);
    }
    let world = stage(&renderer, 4., [0, 0, 0, 255]);
    let mut frame = ParticleFrame {
        textures: Arc::new(textures),
        instances: vec![ParticleInstance {
            position: [0., 2., 0.],
            ..Default::default()
        }],
    };
    for (slot, color) in slots.into_iter().zip(colors) {
        frame.instances[0].texture = slot as u32;
        assert_eq!(renderer.set_particles(&frame).rendered, 1);
        assert_eq!(
            center(&render(&mut renderer, &world, camera())),
            color,
            "slot {slot}"
        );
    }
}

#[test]
#[ignore = "requires original SpellEffects textures and GPU; writes /tmp/openeq-spell-billboards.png"]
fn original_spell_textures_render_as_distinct_alpha_and_additive_billboards() {
    let base = openeq_assets::loader::default_client_dir().expect("original client directory");
    let names = [
        "flare_blsp501.tga",
        "skull_purpsp501.tga",
        "runesp504.dds",
        "iceshardsp501.dds",
    ];
    let catalog = SpellEffectCatalog::load(&base).unwrap();
    let cache = catalog.load_textures(&base).unwrap();
    let indices = names.map(|name| cache.index(name).expect(name));
    assert!(indices.iter().any(|index| *index >= 256));
    let mut renderer = Renderer::new_headless(800, 400).unwrap();
    let world = stage(&renderer, 12., [18, 24, 35, 255]);
    let frame = ParticleFrame {
        textures: cache.textures.clone(),
        instances: names
            .iter()
            .enumerate()
            .flat_map(|(i, _)| {
                [ParticleBlend::Alpha, ParticleBlend::Additive]
                    .into_iter()
                    .enumerate()
                    .map(move |(row, blend)| ParticleInstance {
                        position: [-4.5 + i as f32 * 3., 8., 1.6 - row as f32 * 3.2],
                        size: [2.5; 2],
                        texture: indices[i],
                        blend,
                        ..Default::default()
                    })
            })
            .collect(),
    };
    renderer.set_particles(&frame);
    let (w, h, pixels) = render(&mut renderer, &world, camera());
    assert!(
        pixels
            .chunks_exact(4)
            .filter(|p| p[0] > 80 || p[1] > 80 || p[2] > 80)
            .count()
            > 1000
    );
    image::save_buffer(
        "/tmp/openeq-spell-billboards.png",
        &pixels,
        w,
        h,
        image::ColorType::Rgba8,
    )
    .unwrap();
}

#[test]
#[ignore = "requires original spell catalog and textures plus GPU"]
fn original_spell_catalog_and_common_spell_texture_indices_all_render() {
    let base = openeq_assets::loader::default_client_dir().expect("original client directory");
    let catalog = SpellEffectCatalog::load(&base).unwrap();
    let cache = catalog.load_textures(&base).unwrap();
    assert!(cache.textures.len() > 256);
    assert!(cache.textures.len() <= MAX_PARTICLE_TEXTURES);
    let mut renderer = Renderer::new_headless(64, 64).unwrap();
    let world = stage(&renderer, 6., [0, 0, 0, 255]);
    let mut frame = ParticleFrame {
        textures: cache.textures.clone(),
        instances: (0..cache.textures.len())
            .map(|index| ParticleInstance {
                position: [0., 2., 0.],
                texture: index as u32,
                ..Default::default()
            })
            .collect(),
    };
    let stats = renderer.set_particles(&frame);
    assert_eq!(stats.rendered, cache.textures.len());
    assert_eq!(stats.missing_texture, 0);
    render(&mut renderer, &world, camera());

    // Original spells_us.txt maps these spells to the following EFF rows.
    for (spell, effect) in [(200, 278), (54, 179), (288, 220), (36, 218)] {
        frame.instances = catalog
            .effect(effect)
            .unwrap()
            .stages
            .iter()
            .flat_map(|stage| stage.emitters)
            .filter(|reference| reference.emitter_id != 0)
            .map(|reference| {
                let emitter = catalog.emitter(reference.emitter_id).unwrap();
                let texture = cache.index(&emitter.texture).unwrap_or_else(|| {
                    panic!(
                        "spell {spell} needs missing original texture {}",
                        emitter.texture
                    )
                });
                ParticleInstance {
                    position: [0., 2., 0.],
                    texture,
                    uv_rect: emitter.uv_rect(0.2),
                    blend: if emitter.additive {
                        ParticleBlend::Additive
                    } else {
                        ParticleBlend::Alpha
                    },
                    ..Default::default()
                }
            })
            .collect();
        assert!(!frame.instances.is_empty());
        let stats = renderer.set_particles(&frame);
        assert_eq!(stats.rendered, frame.instances.len(), "spell {spell}");
        assert_eq!(stats.missing_texture, 0, "spell {spell}");
        eprintln!(
            "spell {spell}, effect {effect}: {} emitters; texture indices {:?}",
            stats.rendered,
            frame
                .instances
                .iter()
                .map(|p| p.texture)
                .collect::<Vec<_>>()
        );
        render(&mut renderer, &world, camera());
    }
}
