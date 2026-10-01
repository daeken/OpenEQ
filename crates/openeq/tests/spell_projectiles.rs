//! RoF2 projectile packet → original item/trail assets → timed presentation → GPU.
use std::{
    collections::{BTreeMap, BTreeSet},
    time::{Duration, Instant},
};

use openeq::{
    coordinates,
    spell_effects::{EffectAnchor, EffectAssets, SpellEffects},
    spells::SpellCatalog,
};
use openeq_assets::{
    Scene, loader,
    mesh::{Geometry, Material},
    texture::Texture,
};
use openeq_net::gameplay::{GameplayEvent, parse_packet};
use openeq_render::{Camera, GpuScene, Renderer, actors::ActorRenderer, particles::ParticleFrame};

const WIDTH: u32 = 640;
const HEIGHT: u32 = 400;

fn backdrop(renderer: &Renderer) -> GpuScene {
    let scene = Scene::from_geometry(
        "projectile trail backdrop".into(),
        vec![Material {
            textures: vec!["backdrop".into()],
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
                -100., 40., -100., 0., -1., 0., 0., 0., 100., 40., -100., 0., -1., 0., 1., 0.,
                100., 40., 100., 0., -1., 0., 1., 1., -100., 40., 100., 0., -1., 0., 0., 1.,
            ],
            indices: vec![0, 1, 2, 0, 2, 3],
            material: 0,
            collidable: false,
        }],
        vec![Texture {
            name: "backdrop".into(),
            width: 1,
            height: 1,
            rgba: vec![15, 22, 32, 255],
        }],
    );
    GpuScene::build(renderer.device(), renderer.queue(), &scene).unwrap()
}

fn original_projectile_packet() -> GameplayEvent {
    // RoF2 Arrow_Struct. Its first coordinates are server Y/X/Z. LiveWorld
    // converts server XYZ once into original-asset XYZ before presentation.
    let mut bytes = [0u8; 116];
    for (offset, value) in [
        (0, -6f32),
        (4, 0.),
        (8, 3.),
        (24, 4.),
        (28, 0.),
        (32, 0.),
        (44, 0.),
    ] {
        bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
    }
    for (offset, value) in [(48, 1u32), (52, 2), (56, 8005)] {
        bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
    }
    bytes[70] = 175;
    bytes[89..96].copy_from_slice(b"IT11504");
    let mut event = parse_packet(0x747c, &bytes).unwrap().unwrap();
    let GameplayEvent::Projectile(projectile) = &mut event else {
        panic!("projectile packet")
    };
    assert_eq!(projectile.position, [0., -6., 3.]);
    assert_eq!(projectile.model_name, "IT11504");
    projectile.position = coordinates::server_point_to_scene(projectile.position);
    projectile.launch_angle = coordinates::server_heading_to_scene(projectile.launch_angle);
    event
}

fn changed_pixels(a: &[u8], b: &[u8]) -> usize {
    a.chunks_exact(4)
        .zip(b.chunks_exact(4))
        .filter(|(a, b)| a.iter().zip(*b).any(|(a, b)| a.abs_diff(*b) > 5))
        .count()
}

#[test]
#[ignore = "requires original client assets and GPU; writes /tmp/openeq-spell-projectile-trail.png"]
fn original_bolt_packet_renders_moving_item_and_authored_trail_then_expires_without_impact() {
    let base = loader::default_client_dir().expect("original client directory");
    let mut effects = SpellEffects::default();
    effects.set_assets(EffectAssets::load(&base).unwrap());
    let mut renderer = Renderer::new_headless(WIDTH, HEIGHT).unwrap();
    let mut actors = ActorRenderer::load(&base, "poknowledge").unwrap();
    let stage = backdrop(&renderer);
    renderer.set_scene(&stage);
    let camera = Camera {
        position: [0., -20., 3.],
        pitch: 0.,
        fov_y: 40f32.to_radians(),
        ..Default::default()
    };
    renderer.render_with_actors(&stage, &camera, &actors.draws());
    let empty = renderer.read_rgba().unwrap().2;
    let anchors = BTreeMap::from([
        (
            1,
            EffectAnchor {
                position: [-6., 0., 3.],
                size: 6.,
                ..Default::default()
            },
        ),
        (
            2,
            EffectAnchor {
                position: [6., 0., 3.],
                size: 6.,
                ..Default::default()
            },
        ),
    ]);
    let now = Instant::now();
    // A model-bearing packet suffices. No BeginCast or guessed spell ID.
    effects.event(
        &original_projectile_packet(),
        &SpellCatalog::default(),
        Some(1),
        now,
    );
    let mut gallery = image::RgbaImage::new(WIDTH * 3, HEIGHT);
    let mut first_position = None;
    let mut first_pixels: Option<Vec<u8>> = None;
    let mut frame = ParticleFrame::default();
    for tick in 0..=54 {
        let time = now + Duration::from_secs_f64(tick as f64 / 60.);
        let projectiles = effects.projectiles(time, &anchors);
        assert_eq!(projectiles.len(), 1, "flight disappeared at tick {tick}");
        assert_eq!(projectiles[0].model_name, "IT11504");
        let item_stats = actors.update_projectiles(&renderer, &projectiles);
        assert_eq!(item_stats.rendered, 1);
        assert_eq!(item_stats.missing_model, 0);
        frame = effects.frame(time, &anchors);
        assert_eq!(
            effects.stats().impact_particles,
            0,
            "projectile invented an impact"
        );
        if ![24, 54].contains(&tick) {
            continue;
        }
        assert!(
            !frame.instances.is_empty(),
            "missing original trail at tick {tick}"
        );
        let names: BTreeSet<_> = frame
            .instances
            .iter()
            .map(|particle| {
                let texture = &frame.textures[particle.texture as usize];
                assert!(texture.width > 0 && texture.height > 0);
                assert_eq!(
                    texture.rgba.len(),
                    texture.width as usize * texture.height as usize * 4
                );
                texture.name.as_str()
            })
            .collect();
        assert!(!names.is_empty());
        assert!(
            names.contains("fire_missle.dds"),
            "IT11504 did not select its authored fire trail"
        );
        renderer.clear_particles();
        renderer.render_with_actors(&stage, &camera, &actors.draws());
        let model_only = renderer.read_rgba().unwrap().2;
        assert!(
            changed_pixels(&model_only, &empty) > 10,
            "original projectile mesh invisible"
        );
        let particle_stats = renderer.set_particles(&frame);
        assert_eq!(particle_stats.missing_texture, 0);
        assert_eq!(particle_stats.invalid, 0);
        assert_eq!(particle_stats.rendered, frame.instances.len());
        renderer.render_with_actors(&stage, &camera, &actors.draws());
        let pixels = renderer.read_rgba().unwrap().2;
        let changed = changed_pixels(&pixels, &model_only);
        assert!(
            changed > 25,
            "original trail invisible at tick {tick}: {changed}"
        );
        let column = if tick == 24 { 0 } else { 1 };
        let card = image::RgbaImage::from_raw(WIDTH, HEIGHT, pixels.clone()).unwrap();
        image::imageops::replace(&mut gallery, &card, i64::from(column * WIDTH), 0);
        if let Some(position) = first_position {
            assert!(
                projectiles[0].position[0] > position + 1.,
                "projectile did not move toward target"
            );
            assert!(changed_pixels(&pixels, first_pixels.as_ref().unwrap()) > 25);
        } else {
            first_position = Some(projectiles[0].position[0]);
            first_pixels = Some(pixels);
        }
        eprintln!(
            "tick={tick} position={:?} trail_particles={} trail_pixels={changed} original_textures={names:?}",
            projectiles[0].position,
            frame.instances.len()
        );
    }
    assert!(!frame.instances.is_empty());
    // Cross the estimated arrival while stepping normally, so an erroneous
    // short-lived impact cannot disappear unnoticed during the final jump.
    for tick in 55..=120 {
        let time = now + Duration::from_secs_f64(tick as f64 / 60.);
        let projectiles = effects.projectiles(time, &anchors);
        actors.update_projectiles(&renderer, &projectiles);
        effects.frame(time, &anchors);
        assert_eq!(
            effects.stats().impact_particles,
            0,
            "arrival invented an impact at tick {tick}"
        );
        if tick == 120 {
            assert!(projectiles.is_empty());
        }
    }
    let timeout = now + Duration::from_secs(40);
    let projectiles = effects.projectiles(timeout, &anchors);
    assert!(projectiles.is_empty());
    actors.update_projectiles(&renderer, &projectiles);
    let frame = effects.frame(timeout, &anchors);
    assert!(frame.instances.is_empty(), "trail survived its lifetime");
    assert_eq!(
        effects.stats().impact_particles,
        0,
        "timeout invented an impact"
    );
    assert_eq!(effects.stats().active_effects, 0);
    renderer.set_particles(&frame);
    renderer.render_with_actors(&stage, &camera, &actors.draws());
    let pixels = renderer.read_rgba().unwrap().2;
    assert_eq!(pixels, empty, "projectile or trail leaked after timeout");
    image::imageops::replace(
        &mut gallery,
        &image::RgbaImage::from_raw(WIDTH, HEIGHT, pixels).unwrap(),
        i64::from(WIDTH * 2),
        0,
    );
    gallery
        .save("/tmp/openeq-spell-projectile-trail.png")
        .unwrap();
}
