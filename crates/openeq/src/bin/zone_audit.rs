//! Read-only CPU survey. Each zone is independently runnable for timeout/process isolation.
use anyhow::{Result, bail};
use openeq_assets::{audit, loader};
use serde_json::json;
use std::{path::PathBuf, time::Instant};

fn main() -> Result<()> {
    let mut args = std::env::args().skip(1);
    let target = args.next().ok_or_else(|| {
        anyhow::anyhow!("usage: zone_audit --list|ZONE [--dir DIR] [--metadata-only]")
    })?;
    let mut base = loader::default_client_dir();
    let mut metadata_only = false;
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--dir" => {
                base = Some(PathBuf::from(
                    args.next()
                        .ok_or_else(|| anyhow::anyhow!("missing directory"))?,
                ))
            }
            "--metadata-only" => metadata_only = true,
            _ => bail!("unknown argument {arg}"),
        }
    }
    let base = base.ok_or_else(|| anyhow::anyhow!("no installed assets; supply --dir"))?;
    if target == "--list" {
        let result = audit::discover(&base)?;
        println!(
            "{}",
            json!({"zones":result.zones, "discovery_errors":result.errors,
            "archives_examined":result.archives_examined})
        );
        return Ok(());
    }
    if target.is_empty()
        || !target
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_')
    {
        bail!("zone name must contain only ASCII letters, numbers or underscores");
    }
    let zone = target.to_ascii_lowercase();
    let start = Instant::now();
    let mut report = json!({"zone":zone,"rendering":"not_checked","traversal":"not_checked",
        "textures":"not_checked","audio":"not_checked"});
    let mut failed = false;
    match audit::metadata(&base, &zone) {
        Ok(meta) => {
            report["metadata"] = json!({"format":meta.format,"version":meta.version,
                "terrain_tiles":meta.terrain_tiles,"terrain_groups":meta.terrain_groups,
                "authored_regions":meta.authored_regions,"liquids":meta.liquid_status,
                "liquid_detail":meta.liquid_detail,"borders":meta.border_status});
        }
        Err(error) => {
            failed = true;
            report["metadata_error"] = error.to_string().into();
        }
    }
    if !metadata_only {
        match audit::geometry(&base, &zone) {
            Ok(g) => {
                failed |= g.invalid_meshes
                    + g.invalid_instances
                    + g.invalid_lights
                    + g.invalid_collision_meshes
                    + g.invalid_object_references
                    > 0;
                report["geometry"] = json!({"meshes":g.meshes,"triangles":g.triangles,
                    "collision_triangles":g.collision_triangles,"instances":g.instances,
                    "lights":g.lights,"texture_references":g.texture_references,
                    "invalid_meshes":g.invalid_meshes,"mesh_problems":g.mesh_problems,"invalid_instances":g.invalid_instances,
                    "invalid_collision_meshes":g.invalid_collision_meshes,
                    "invalid_object_references":g.invalid_object_references,
                    "unresolved_objects":g.unresolved_objects,
                    "invalid_lights":g.invalid_lights});
            }
            Err(error) => {
                failed = true;
                report["geometry_error"] = error.to_string().into();
            }
        }
    }
    report["seconds"] = start.elapsed().as_secs_f64().into();
    println!("{report}");
    if failed {
        std::process::exit(1);
    }
    Ok(())
}
