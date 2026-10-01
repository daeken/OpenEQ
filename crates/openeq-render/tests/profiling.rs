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
    for value in timing.raw_pass_ms {
        assert!(
            value.is_finite() && value > 0.,
            "missing GPU pass duration: {timing:?}"
        );
    }
    let sum = timing.shadow_ms
        + timing.gbuffer_ms
        + timing.lighting_ms
        + timing.transparency_ms
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
