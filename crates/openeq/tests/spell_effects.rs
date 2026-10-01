//! End-to-end original definitions → timed effects → original textures → GPU.
use openeq::{
    spell_effects::{EffectAnchor, EffectAssets, SpellEffects},
    spells::SpellCatalog,
};
use openeq_assets::{
    Scene, loader,
    mesh::{Geometry, Material},
    texture::Texture,
};
use openeq_net::gameplay::GameplayEvent;
use openeq_render::{
    Camera, GpuScene, Renderer,
    actors::{ActorAction, ActorRenderer, ActorState, CharacterModelSet},
    environment::EnvironmentSettings,
};
use std::{
    collections::BTreeMap,
    time::{Duration, Instant},
};
fn stage(renderer: &Renderer) -> GpuScene {
    GpuScene::build(
        renderer.device(),
        renderer.queue(),
        &Scene::from_geometry(
            "spell stage".into(),
            vec![Material {
                textures: vec!["floor".into()],
                normal_map: None,
                water: None,
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
                vertices: vec![
                    -80., -80., 0., 0., 0., 1., 0., 0., 80., -80., 0., 0., 0., 1., 1., 0., 80.,
                    80., 0., 0., 0., 1., 1., 1., -80., 80., 0., 0., 0., 1., 0., 1.,
                ],
                indices: vec![0, 1, 2, 0, 2, 3],
                material: 0,
                collidable: false,
            }],
            vec![Texture {
                name: "floor".into(),
                width: 1,
                height: 1,
                rgba: vec![50, 55, 60, 255],
            }],
        ),
    )
    .unwrap()
}
fn action(spell_id: u32, flag: u8) -> GameplayEvent {
    GameplayEvent::SpellAction {
        source_id: 1,
        target_id: 1,
        spell_id,
        level: 20,
        action_type: 231,
        spell_level: 20,
        instrument_modifier: 1.,
        effect_flag: flag,
    }
}
#[test]
#[ignore = "requires original client assets and GPU"]
fn original_heal_frost_shield_and_gate_cast_and_impact_gallery() {
    let base = loader::default_client_dir().unwrap();
    let spells = SpellCatalog::load(&base).unwrap();
    let mut effects = SpellEffects::default();
    effects.set_assets(EffectAssets::load(&base).unwrap());
    let mut renderer = Renderer::new_headless(480, 540).unwrap();
    let stage = stage(&renderer);
    renderer.set_scene(&stage);
    renderer.set_environment(
        EnvironmentSettings {
            fog_start: 500.,
            fog_end: 1000.,
            ..Default::default()
        },
        None,
    );
    let mut actors =
        ActorRenderer::load_with_model_set(&base, "poknowledge", CharacterModelSet::Luclin)
            .unwrap();
    let mut state = ActorState {
        id: 1,
        race: 1,
        gender: 0,
        size: 6.,
        position: [0., 0., 3.],
        heading: 256.,
        ..Default::default()
    };
    let camera = Camera {
        position: [0., -25., 8.],
        yaw: 0.,
        pitch: -0.15,
        fov_y: 40f32.to_radians(),
    };
    let mut gallery = image::RgbaImage::new(480 * 4, 540 * 2);
    for (column, spell_id) in [200, 54, 288, 36].into_iter().enumerate() {
        for impact in [false, true] {
            effects.clear();
            let now = Instant::now();
            state.action_sequence += 1;
            state.action = if impact {
                ActorAction::Stand
            } else {
                ActorAction::Animation(spells.spells[&spell_id].casting_animation)
            };
            actors.update(&renderer, std::slice::from_ref(&state), 0.);
            let event = if impact {
                action(spell_id, 0)
            } else {
                GameplayEvent::BeginCast {
                    caster_id: 1,
                    spell_id,
                    cast_time_ms: 2500,
                }
            };
            effects.event(&event, &spells, Some(1), now);
            let mut frame = Default::default();
            for tick in 0..=36 {
                let time = tick as f32 / 30.;
                actors.update(&renderer, std::slice::from_ref(&state), time);
                let sockets = actors.sockets()[&1];
                let anchors = BTreeMap::from([(
                    1,
                    EffectAnchor {
                        position: sockets.chest.unwrap_or(state.position),
                        heading: state.heading,
                        size: 6.,
                        sockets,
                    },
                )]);
                frame = effects.frame(now + Duration::from_secs_f32(time), &anchors);
            }
            assert!(
                !frame.instances.is_empty(),
                "spell{spell_id} impact{impact} empty"
            );
            for particle in &frame.instances {
                assert!(
                    particle
                        .position
                        .into_iter()
                        .chain(particle.size)
                        .chain(particle.color)
                        .all(f32::is_finite)
                );
            }
            renderer.clear_particles();
            renderer.render_with_actors(&stage, &camera, &actors.draws());
            let (_, _, baseline) = renderer.read_rgba().unwrap();
            let stats = renderer.set_particles(&frame);
            assert_eq!(stats.missing_texture, 0, "spell{spell_id}");
            assert_eq!(stats.invalid, 0);
            renderer.render_with_actors(&stage, &camera, &actors.draws());
            let (_, _, pixels) = renderer.read_rgba().unwrap();
            let changed = pixels
                .chunks_exact(4)
                .zip(baseline.chunks_exact(4))
                .filter(|(a, b)| a.iter().zip(*b).any(|(a, b)| a.abs_diff(*b) > 5))
                .count();
            assert!(
                changed > 10,
                "spell{spell_id} impact{impact}: invisible ({changed})"
            );
            let card = image::RgbaImage::from_raw(480, 540, pixels).unwrap();
            image::imageops::replace(
                &mut gallery,
                &card,
                column as i64 * 480,
                i64::from(impact) * 540,
            );
            eprintln!(
                "spell={spell_id} phase={} particles={} visible_pixels={changed}",
                if impact { "impact" } else { "cast" },
                frame.instances.len()
            );
            if impact {
                let emitted = effects.stats().emitted_particles;
                effects.event(
                    &action(spell_id, 4),
                    &spells,
                    Some(1),
                    now + Duration::from_millis(100),
                );
                // Opposite success flag at packet time must not add a second cue.
                assert_eq!(effects.stats().emitted_particles, emitted);
            }
            effects.remove_entity(1);
            assert!(
                effects
                    .frame(now + Duration::from_secs(2), &BTreeMap::new())
                    .instances
                    .is_empty()
            );
        }
    }
    gallery.save("/tmp/openeq-spell-families.png").unwrap();
}
