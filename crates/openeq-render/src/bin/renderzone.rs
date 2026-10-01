//! Renders a zone headlessly to a PNG.
//!
//! ```text
//! renderzone gfaydark --out /tmp/gfaydark.png
//! renderzone akanon --pos 100,-200,20 --yaw 45 --pitch -10
//! ```
//!
//! This exercises the whole renderer (shadow pass, deferred G-buffer, lighting)
//! without needing a window, which keeps it usable in CI and from a plain shell.

use anyhow::Context;
use openeq_assets::loader;
use openeq_render::Renderer;
use openeq_render::scene::{Camera, GpuScene};
use std::path::PathBuf;

fn main() -> anyhow::Result<()> {
    let mut args = std::env::args().skip(1);
    let Some(zone) = args.next() else {
        eprintln!(
            "usage: renderzone <zone> [--dir DIR] [--out FILE] [--width N] [--height N] \
             [--pos X,Y,Z] [--yaw DEG] [--pitch DEG] [--profile FRAMES] \
             [--brute-lights] [--no-lights] [--baked-terrain]"
        );
        std::process::exit(2);
    };

    let mut dir = loader::default_client_dir();
    let mut out = PathBuf::from(format!("/tmp/{zone}.png"));
    let mut width = 1280u32;
    let mut height = 720u32;
    let mut position = None;
    let mut yaw = 0f32;
    let mut pitch = -10f32;
    let mut only_material = None;
    let mut profile_frames = 0usize;
    let mut no_lights = false;
    let mut brute_lights = false;
    let mut terrain_mode = openeq_render::terrain::TerrainMode::Direct;

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--dir" => dir = args.next().map(PathBuf::from),
            "--out" => out = args.next().map(PathBuf::from).unwrap_or(out),
            "--width" => width = args.next().and_then(|v| v.parse().ok()).unwrap_or(width),
            "--height" => height = args.next().and_then(|v| v.parse().ok()).unwrap_or(height),
            "--pos" => {
                position = args.next().and_then(|value| {
                    let parts: Vec<f32> = value
                        .split(',')
                        .filter_map(|p| p.trim().parse().ok())
                        .collect();
                    (parts.len() == 3).then(|| [parts[0], parts[1], parts[2]])
                })
            }
            "--yaw" => yaw = args.next().and_then(|v| v.parse().ok()).unwrap_or(yaw),
            "--pitch" => pitch = args.next().and_then(|v| v.parse().ok()).unwrap_or(pitch),
            "--only-material" => only_material = args.next(),
            "--no-lights" => no_lights = true,
            "--brute-lights" => brute_lights = true,
            "--baked-terrain" => terrain_mode = openeq_render::terrain::TerrainMode::Baked,
            "--profile" => {
                profile_frames = args
                    .next()
                    .context("--profile needs a frame count")?
                    .parse()?;
                anyhow::ensure!(
                    (1..=10000).contains(&profile_frames),
                    "profile count must be 1–10000"
                );
            }
            other => anyhow::bail!("unrecognised argument {other}"),
        }
    }

    let dir = dir.context("no client directory (pass --dir)")?;

    println!("loading {} from {}", zone, dir.display());
    let load_start = std::time::Instant::now();
    let mut scene = loader::load_zone(&dir, &zone)?;
    println!(
        "  asset preparation: {:.1}ms",
        load_start.elapsed().as_secs_f64() * 1000.
    );
    if no_lights {
        scene.lights.clear();
    }

    // A debugging aid: keep only the surfaces whose material names contain a
    // substring, which makes a texture mix-up obvious at a glance.
    if let Some(needle) = only_material {
        let needle = needle.to_ascii_lowercase();
        let kept: Vec<_> = scene
            .meshes
            .iter()
            .filter(|geometry| {
                scene.materials[geometry.material]
                    .textures
                    .iter()
                    .any(|name| name.to_ascii_lowercase().contains(&needle))
            })
            .cloned()
            .collect();
        let materials = kept
            .iter()
            .map(|geometry| scene.materials[geometry.material].clone())
            .collect();
        scene.terrain_materials = kept
            .iter()
            .enumerate()
            .filter_map(|(index, geometry)| {
                scene
                    .terrain_materials
                    .get(&geometry.material)
                    .cloned()
                    .map(|recipe| (index, recipe))
            })
            .collect();
        scene.meshes = kept
            .into_iter()
            .enumerate()
            .map(|(index, mut geometry)| {
                geometry.material = index;
                geometry
            })
            .collect();
        scene.materials = materials;
        println!("  filtered to materials containing {needle:?}");
    }
    println!(
        "  {} materials, {} meshes, {} triangles, {} instances, {} lights",
        scene.materials.len(),
        scene.meshes.len(),
        scene.triangle_count(),
        scene.instances.len(),
        scene.lights.len()
    );

    let mut renderer = Renderer::new_headless(width, height)?;
    let settings = openeq_render::environment::EnvironmentSettings::for_zone(&zone);
    let sky = openeq_assets::environment::load_sky(&dir, &zone, 0.5).ok();
    renderer.set_environment(settings, sky.as_ref());
    let upload_start = std::time::Instant::now();
    let gpu_scene =
        GpuScene::build_with_terrain(renderer.device(), renderer.queue(), &scene, terrain_mode)?;
    println!(
        "  GPU preparation/upload: {:.1}ms; terrain {:?}",
        upload_start.elapsed().as_secs_f64() * 1000.,
        gpu_scene.terrain_stats()
    );
    let mut side = openeq_render::scene::ATLAS_SIZE;
    let mut atlas_bytes = 0u64;
    loop {
        atlas_bytes += u64::from(side)
            * u64::from(side)
            * 4
            * u64::from(gpu_scene.atlas.depth_or_array_layers());
        if side == 1 {
            break;
        }
        side /= 2;
    }
    println!(
        "  scene material GPU bytes: {} (atlas {} + terrain {})",
        atlas_bytes
            + gpu_scene.terrain_stats().texture_bytes
            + gpu_scene.terrain_stats().buffer_bytes,
        atlas_bytes,
        gpu_scene.terrain_stats().texture_bytes + gpu_scene.terrain_stats().buffer_bytes
    );
    gpu_scene.set_light_grid_enabled(renderer.queue(), !brute_lights);
    println!(
        "  uploaded {} draw calls, {} lights, bounds {:?}..{:?}",
        gpu_scene.draws.len(),
        gpu_scene.light_count,
        gpu_scene.bounds_min,
        gpu_scene.bounds_max
    );

    let default_position = {
        let center = (gpu_scene.bounds_min + gpu_scene.bounds_max) * 0.5;
        // Sit a little above the middle of the zone, looking north.
        [center.x, center.y, center.z + 60.0]
    };

    let camera = Camera {
        position: position.unwrap_or(default_position),
        yaw: yaw.to_radians(),
        pitch: pitch.to_radians(),
        fov_y: 70f32.to_radians(),
    };
    let eye = Camera::to_world(camera.position);
    println!("  camera at {:?} looking {:?}", eye, camera.forward());

    renderer.render(&gpu_scene, &camera);
    if profile_frames > 0 {
        println!(
            "GPU timestamps supported: {}",
            renderer.enable_profiling(true)
        );
        println!(
            "Lights: spatial_grid={}, removed={}; GPU values attribute completion boundaries (overlapping raw pass intervals are not additive). CPU render/submit and completion wait are serialized diagnostic timings, not windowed FPS.",
            !brute_lights, no_lights
        );
        let mut cpu = Vec::new();
        let mut wait = Vec::new();
        let mut timings = Vec::new();
        let mut last_gpu_frame = None;
        for frame in 0..profile_frames + 60 {
            let begin = std::time::Instant::now();
            renderer.render(&gpu_scene, &camera);
            let submitted = std::time::Instant::now();
            renderer
                .device()
                .poll(wgpu::PollType::wait_indefinitely())?;
            let finished = std::time::Instant::now();
            let gpu = renderer.latest_gpu_timings();
            let gpu = gpu.filter(|gpu| {
                if last_gpu_frame.is_none_or(|last| gpu.frame_id > last) {
                    last_gpu_frame = Some(gpu.frame_id);
                    true
                } else {
                    false
                }
            });
            if frame >= 60 {
                cpu.push((submitted - begin).as_secs_f64() * 1000.);
                wait.push((finished - submitted).as_secs_f64() * 1000.);
                if let Some(gpu) = gpu {
                    timings.push(gpu);
                }
            }
        }
        let report = |name: &str, mut values: Vec<f64>| {
            if values.is_empty() {
                return;
            }
            values.sort_by(f64::total_cmp);
            let p = |q: f64| values[(values.len() as f64 * q).ceil() as usize - 1];
            println!(
                "{name}: median={:.3}ms p95={:.3}ms samples={}",
                p(0.5),
                p(0.95),
                values.len()
            );
        };
        report("CPU render/submit", cpu);
        report("GPU completion wait", wait);
        for (i, name) in [
            "GPU shadow",
            "GPU gbuffer",
            "GPU lighting",
            "GPU transparency",
            "GPU additive",
            "GPU particles",
            "GPU UI",
            "GPU attributed total",
            "GPU frame span",
        ]
        .iter()
        .enumerate()
        {
            report(
                name,
                timings
                    .iter()
                    .map(|t| {
                        [
                            t.shadow_ms,
                            t.gbuffer_ms,
                            t.lighting_ms,
                            t.transparency_ms,
                            t.additive_ms,
                            t.particles_ms,
                            t.ui_ms,
                            t.total_ms,
                            t.frame_span_ms,
                        ][i]
                    })
                    .collect(),
            );
        }
        println!("Timestamp readbacks: {:?}", renderer.profiling_stats());
    }
    let (width, height, pixels) = renderer
        .read_rgba()
        .context("headless renderer should support readback")?;
    let image = image::RgbaImage::from_raw(width, height, pixels)
        .context("readback produced an inconsistent image")?;
    image.save(&out)?;
    println!("wrote {}", out.display());
    Ok(())
}
