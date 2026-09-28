//! Regression tests against a real EverQuest installation.
//!
//! These skip silently when no client data is present, so they stay usable
//! everywhere while still catching breakage on a machine that has it.
//!
//! The zones below were chosen because they exercise material lists that contain
//! an entry with no texture. Dropping such an entry shifts every material after
//! it, which used to panic here and scramble textures on zones that survived.

use std::path::PathBuf;

fn client_dir() -> Option<PathBuf> {
    openeq_assets::loader::default_client_dir()
}

fn classic_zones() -> Vec<&'static str> {
    vec![
        "gfaydark",
        "chardok",
        "citymist",
        "gukd",
        "abysmal",
        "acrylia",
        "dawnshroud",
    ]
}

#[test]
fn classic_zones_load_with_geometry_and_textures() {
    let Some(dir) = client_dir() else {
        eprintln!("no client directory; skipping");
        return;
    };

    let mut checked = 0;
    for zone in classic_zones() {
        if !dir.join(format!("{zone}_obj.s3d")).is_file() {
            continue;
        }
        let scene = openeq_assets::load_zone(&dir, zone)
            .unwrap_or_else(|error| panic!("{zone} failed to load: {error}"));

        assert!(
            scene.triangle_count() > 1_000,
            "{zone} produced only {} triangles",
            scene.triangle_count()
        );
        // Every zone should resolve at least some of the textures its geometry
        // asks for; a material index shift would leave references dangling.
        let textured = scene
            .materials
            .iter()
            .filter(|material| !material.textures.is_empty())
            .count();
        assert!(
            textured > 10,
            "{zone} resolved only {textured} textured materials"
        );
        checked += 1;
    }

    if checked == 0 {
        eprintln!("client directory present but no expected zones; skipping");
    }
}

/// The material that carries the canopy texture must be the one on the wide,
/// high geometry, not the narrow trunk. This is the shape of the bug where
/// leaves and trunks swap.
#[test]
fn tree_materials_line_up_with_their_geometry() {
    let Some(dir) = client_dir() else {
        return;
    };
    if !dir.join("gfaydark_obj.s3d").is_file() {
        return;
    }
    let scene = openeq_assets::load_zone(&dir, "gfaydark").expect("gfaydark should load");

    let mut leaves = None;
    let mut bark = None;
    for geometry in &scene.meshes {
        let material = &scene.materials[geometry.material];
        let names = material.textures.join(",").to_ascii_lowercase();
        let spread = horizontal_spread(geometry);
        if names.contains("nekpine") {
            leaves = Some(leaves.map_or(spread, |best: f32| best.max(spread)));
        } else if names.contains("kbarkd") {
            bark = Some(bark.map_or(spread, |best: f32| best.min(spread)));
        }
    }

    let (Some(leaves), Some(bark)) = (leaves, bark) else {
        eprintln!("gfaydark has no pine/bark materials; skipping");
        return;
    };
    assert!(
        leaves > bark,
        "pine canopies should be wider than bark trunks (pine {leaves}, bark {bark})"
    );
}

/// Mean horizontal distance from the geometry's centre.
fn horizontal_spread(geometry: &openeq_assets::mesh::Geometry) -> f32 {
    let stride = openeq_assets::mesh::VERTEX_STRIDE;
    let count = geometry.vertices.len() / stride;
    if count == 0 {
        return 0.0;
    }
    let mut cx = 0.0;
    let mut cy = 0.0;
    for vertex in geometry.vertices.chunks_exact(stride) {
        cx += vertex[0];
        cy += vertex[1];
    }
    cx /= count as f32;
    cy /= count as f32;
    let mut total = 0.0;
    for vertex in geometry.vertices.chunks_exact(stride) {
        total += ((vertex[0] - cx).powi(2) + (vertex[1] - cy).powi(2)).sqrt();
    }
    total / count as f32
}

/// Placed objects stand up. A misplaced yaw shows up here as a large lean,
/// which is how a zone full of trees ends up looking wind-blown.
#[test]
fn placed_objects_stand_upright() {
    let Some(dir) = client_dir() else {
        return;
    };
    if !dir.join("gfaydark_obj.s3d").is_file() {
        return;
    }
    let scene = openeq_assets::load_zone(&dir, "gfaydark").expect("gfaydark should load");
    let mut tilts: Vec<f32> = scene
        .instances
        .iter()
        .map(|instance| tilt_degrees(instance.rotation))
        .collect();
    if tilts.is_empty() {
        eprintln!("gfaydark has no placed objects; skipping");
        return;
    }
    tilts.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let median = tilts[tilts.len() / 2];
    let upright = tilts.iter().filter(|tilt| **tilt < 30.0).count() as f32 / tilts.len() as f32;
    eprintln!(
        "{} instances, median tilt {median:.1} deg, {:.0}% within 30 deg",
        tilts.len(),
        upright * 100.0
    );
    assert!(
        median < 15.0,
        "placed objects should mostly stand upright, median tilt was {median:.1} deg"
    );
    assert!(
        upright > 0.8,
        "expected most placed objects to be near-upright, only {:.0}% were",
        upright * 100.0
    );
}

/// Angle between the instance's up axis and the world up axis. A pure yaw
/// scores zero.
fn tilt_degrees(q: [f32; 4]) -> f32 {
    let [x, y, _, _] = q;
    let up_z = (1.0 - 2.0 * (x * x + y * y)).clamp(-1.0, 1.0);
    up_z.acos().to_degrees()
}
