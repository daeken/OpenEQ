use std::sync::Arc;

use openeq_assets::{
    Scene,
    mesh::{Geometry, Material},
    texture::Texture,
};
use openeq_render::{
    Camera, GpuScene, Renderer,
    particles::{ParticleFrame, ParticleInstance},
};

fn complete(renderer: &mut Renderer) -> openeq_render::profiling::GpuProfileStats {
    // The production renderer never waits. Tests drain both asynchronous
    // stages: render completion, then the deferred timestamp resolve/map.
    for _ in 0..3 {
        renderer
            .device()
            .poll(wgpu::PollType::wait_indefinitely())
            .unwrap();
        let stats = renderer.profiling_stats();
        if stats.in_flight == 0 {
            return stats;
        }
    }
    panic!("GPU timestamp readback did not complete");
}

fn plane(renderer: &Renderer, y: f32, transparent: bool, additive: bool) -> GpuScene {
    surface(renderer, y, transparent, additive, false)
}

fn surface(
    renderer: &Renderer,
    y: f32,
    transparent: bool,
    additive: bool,
    waterfall: bool,
) -> GpuScene {
    let scene = Scene::from_geometry(
        "GPU profiling fixture".into(),
        vec![Material {
            textures: vec!["solid".into()],
            normal_map: None,
            water: None,
            flags: 0,
            anim_speed: 0,
            alpha_mask: transparent,
            transparent,
            additive,
            emissive: true,
            clamp_uv: false,
            waterfall: waterfall.then_some([0.; 4]),
            uv_encoding: Default::default(),
        }],
        vec![Geometry {
            vertices: vec![
                -10., y, -10., 0., -1., 0., 0., 0., 10., y, -10., 0., -1., 0., 1., 0., 10., y, 10.,
                0., -1., 0., 1., 1., -10., y, 10., 0., -1., 0., 0., 1.,
            ],
            indices: vec![0, 1, 2, 0, 2, 3],
            material: 0,
            collidable: false,
        }],
        vec![Texture {
            name: "solid".into(),
            width: 1,
            height: 1,
            rgba: if transparent {
                vec![200, 20, 50, 128]
            } else {
                vec![10, 30, 80, 255]
            },
        }],
    );
    GpuScene::build(renderer.device(), renderer.queue(), &scene).unwrap()
}

#[test]
fn empty_forward_draws_skip_raster_and_resolve_timings_in_zone_and_actor_scenes() {
    let mut renderer = Renderer::new_headless(64, 64).expect("GPU required");
    renderer.set_environment(
        openeq_render::environment::EnvironmentSettings {
            sky_enabled: false,
            fog_enabled: false,
            fog_color: [0.2, 0.4, 0.6],
            ..Default::default()
        },
        None,
    );
    let source = Scene::from_geometry("empty".into(), vec![], vec![], vec![]);
    let empty = GpuScene::build(renderer.device(), renderer.queue(), &source).unwrap();
    let camera = Camera {
        pitch: 0.,
        ..Default::default()
    };
    renderer.set_scene(&empty);
    renderer.render_at(&empty, &camera, std::time::Duration::ZERO);
    let reference = renderer.read_rgba().unwrap().2;
    if !renderer.enable_profiling(true) {
        return;
    }
    let mut failures = Vec::new();
    for family in 0..3 {
        for actor_scene in [false, true] {
            for zero_indices in [true, false] {
                let gpu = surface(&renderer, 2., family == 0, family == 1, family == 2);
                let mut actor = renderer.prepare_actor(gpu);
                let render = |r: &mut Renderer, actor: &openeq_render::GpuActor| {
                    let world = if actor_scene { &empty } else { &actor.scene };
                    r.set_scene(world);
                    if actor_scene {
                        r.render_with_actors(world, &camera, &[actor]);
                    } else {
                        r.render_at(world, &camera, std::time::Duration::ZERO);
                    }
                    r.read_rgba().unwrap().2
                };
                renderer.enable_profiling(false);
                assert!(renderer.enable_profiling(true));
                let visible = render(&mut renderer, &actor);
                assert_ne!(visible, reference, "family {family} positive draw control");
                let live_stats = complete(&mut renderer);
                assert_eq!(live_stats.failed, 0);
                let live = live_stats.latest.unwrap();
                let slots: &[usize] = match family {
                    0 => &[4, 5],
                    1 => &[7],
                    _ => &[6],
                };
                assert!(slots.iter().all(|&slot| live.raw_pass_ms[slot] > 0.));
                if zero_indices {
                    actor.scene.draws[0].index_count = 0;
                } else {
                    actor.scene.draws[0].instance_count = 0;
                }
                renderer.enable_profiling(false);
                assert!(renderer.enable_profiling(true));
                assert_eq!(
                    render(&mut renderer, &actor),
                    reference,
                    "stale color in family {family}"
                );
                let stats = complete(&mut renderer);
                let absent = stats.latest.is_some_and(|t| {
                    t.raw_pass_ms[4..8].iter().all(|&v| v == 0.)
                        && t.transparency_ms == 0.
                        && t.additive_ms == 0.
                        && t.waterfall_ms == 0.
                });
                eprintln!(
                    "empty-forward family={family} actor={actor_scene} zero_indices={zero_indices} failed={} completed={} absent={absent}",
                    stats.failed, stats.completed
                );
                if stats.failed != 0 || stats.completed != 1 || !absent {
                    failures.push((
                        family,
                        actor_scene,
                        zero_indices,
                        stats.failed,
                        stats.completed,
                        absent,
                    ));
                }
            }
        }
    }
    assert!(
        failures.is_empty(),
        "invalid empty forward timings: {failures:?}"
    );
}

#[test]
fn profiler_is_send_sync_for_bevy_resources() {
    fn check<T: Send + Sync>() {}
    check::<Renderer>();
}

#[test]
fn gpu_profiling_is_opt_in_async_bounded_and_preserves_pixels() {
    let Ok(mut renderer) = Renderer::new_headless(64, 64) else {
        return;
    };
    let world = plane(&renderer, 8., false, false);
    let camera = Camera {
        pitch: 0.,
        ..Default::default()
    };
    renderer.render(&world, &camera);
    let initial = renderer.read_rgba().unwrap().2;
    let disabled = renderer.profiling_stats();
    assert!(!disabled.enabled);
    assert!(disabled.latest.is_none());
    assert_eq!(disabled.submitted, 0);
    assert_eq!(renderer.enable_profiling(true), disabled.supported);
    if !disabled.supported {
        assert!(renderer.latest_gpu_timings().is_none());
        return;
    }
    renderer.render(&world, &camera);
    // Only the test/harness waits. Renderer profiling uses Poll exclusively.
    let timing = complete(&mut renderer)
        .latest
        .expect("completed timestamp readback");
    assert_eq!(timing.frame_id, 1);
    assert!(timing.total_ms > 0.);
    assert_eq!(timing.transparency_ms, 0.);
    assert_eq!(timing.additive_ms, 0.);
    assert_eq!(timing.particles_ms, 0.);
    assert_eq!(timing.ui_ms, 0.);
    assert_eq!(renderer.read_rgba().unwrap().2, initial);

    let actor = renderer.prepare_actor(plane(&renderer, 6., true, false));
    let glass = renderer.prepare_actor(plane(&renderer, 5., false, true));
    renderer.set_particles(&ParticleFrame {
        textures: Arc::new(vec![Texture {
            name: "particle".into(),
            width: 1,
            height: 1,
            rgba: vec![0, 255, 0, 128],
        }]),
        instances: vec![ParticleInstance {
            position: [0., 4., 0.],
            size: [2.; 2],
            ..Default::default()
        }],
    });
    let rect = openeq_ui::Rect::new(0., 0., 10., 10.);
    renderer.set_ui(&openeq_ui::UiFrame {
        commands: vec![openeq_ui::DrawCommand::Fill {
            rect,
            clip: rect,
            color: [255, 255, 255, 128],
        }],
        ..Default::default()
    });
    for _ in 0..12 {
        renderer.render_with_actors(&world, &camera, &[&actor, &glass]);
        assert!(renderer.profiling_stats().in_flight <= 4);
    }
    let stats = complete(&mut renderer);
    assert_eq!(stats.submitted + stats.dropped, 13);
    assert_eq!(stats.completed, stats.submitted, "{stats:?}");
    assert_eq!(stats.in_flight, 0);
    assert_eq!(stats.failed, 0);
    let timing = stats.latest.unwrap();
    assert_eq!(timing.waterfall_ms, 0.);
    for (index, value) in timing.raw_pass_ms.into_iter().enumerate() {
        if matches!(index, 3 | 6) {
            assert_eq!(value, 0.);
            continue;
        }
        assert!(
            value.is_finite() && value > 0.,
            "missing GPU pass duration: {timing:?}"
        );
    }
    let sum = timing.shadow_ms
        + timing.gbuffer_ms
        + timing.lighting_ms
        + timing.transparency_ms
        + timing.waterfall_ms
        + timing.lava_ms
        + timing.additive_ms
        + timing.particles_ms
        + timing.ui_ms;
    assert!((sum - timing.total_ms).abs() < 1e-8);
    // Tile-based GPUs can overlap one pass's fragments with the next pass's
    // vertices; their per-pass sums can therefore exceed the frame span.
    assert!(timing.frame_span_ms > 0.);
    let profiled = renderer.read_rgba().unwrap().2;
    assert!(!renderer.enable_profiling(false));
    renderer.render_with_actors(&world, &camera, &[&actor, &glass]);
    assert_eq!(renderer.read_rgba().unwrap().2, profiled);
    assert!(renderer.latest_gpu_timings().is_none());
    assert!(renderer.enable_profiling(true));
    renderer.clear_particles();
    renderer.render(&world, &camera);
    let timing = complete(&mut renderer).latest.unwrap();
    assert_eq!(timing.frame_id, 1);
    assert_eq!(timing.transparency_ms, 0.);
    assert_eq!(timing.additive_ms, 0.);
    assert_eq!(timing.particles_ms, 0.);
    // Dropping a profiler with queued callbacks must remain safe too.
    renderer.render(&world, &camera);
    renderer.enable_profiling(false);
    renderer
        .device()
        .poll(wgpu::PollType::wait_indefinitely())
        .unwrap();
}
