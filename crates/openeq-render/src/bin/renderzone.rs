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
             [--pos X,Y,Z] [--yaw DEG] [--pitch DEG]"
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
            other => anyhow::bail!("unrecognised argument {other}"),
        }
    }

    let dir = dir.context("no client directory (pass --dir)")?;

    println!("loading {} from {}", zone, dir.display());
    let scene = loader::load_zone(&dir, &zone)?;
    println!(
        "  {} materials, {} meshes, {} triangles, {} instances, {} lights",
        scene.materials.len(),
        scene.meshes.len(),
        scene.triangle_count(),
        scene.instances.len(),
        scene.lights.len()
    );

    let mut renderer = Renderer::new_headless(width, height)?;
    let gpu_scene = GpuScene::build(renderer.device(), renderer.queue(), &scene);
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
    let (width, height, pixels) = renderer
        .read_rgba()
        .context("headless renderer should support readback")?;
    let image = image::RgbaImage::from_raw(width, height, pixels)
        .context("readback produced an inconsistent image")?;
    image.save(&out)?;
    println!("wrote {}", out.display());
    Ok(())
}
