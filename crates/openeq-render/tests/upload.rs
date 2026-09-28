//! Uploads real zones to a real device when one is available.
//!
//! Skips silently when there is no client data or no GPU, so it stays usable
//! everywhere while still catching breakage on a machine that has both.

use openeq_assets::loader;
use openeq_render::Renderer;
use openeq_render::scene::GpuScene;

/// Zones with enough distinct textures to strain the texture atlas. Plane of
/// Knowledge needs around 480 array layers, which is past the default cap.
const HEAVY_ZONES: [&str; 4] = ["poknowledge", "gfaydark", "chardok", "abysmal"];

#[test]
fn zones_upload_within_the_device_limits() {
    let Some(dir) = loader::default_client_dir() else {
        eprintln!("no client directory; skipping");
        return;
    };
    let Ok(renderer) = Renderer::new_headless(64, 64) else {
        eprintln!("no usable GPU adapter; skipping");
        return;
    };

    let mut checked = 0;
    for zone in HEAVY_ZONES {
        let classic = dir.join(format!("{zone}_obj.s3d")).is_file();
        let eqg = dir.join(format!("{zone}.eqg")).is_file();
        if !classic && !eqg {
            continue;
        }
        let scene = loader::load_zone(&dir, zone)
            .unwrap_or_else(|error| panic!("{zone} failed to load: {error}"));
        // Building the scene allocates the atlas, which is where an
        // over-large texture array used to abort the process.
        GpuScene::build(renderer.device(), renderer.queue(), &scene)
            .unwrap_or_else(|error| panic!("{zone} failed to upload: {error}"));
        checked += 1;
    }

    if checked == 0 {
        eprintln!("client directory present but none of the expected zones; skipping");
    }
}
