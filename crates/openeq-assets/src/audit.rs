//! Installation survey using the same declaration selection as the live loader.
//! A successful load is not certification of rendering, traversal or acoustics.
use std::{
    collections::{BTreeMap, BTreeSet},
    path::Path,
};

use crate::{
    Result, bsp_regions::BspRegions, collision::CollisionWorld, liquid_regions::LiquidRegions,
    loader, mesh::VERTEX_STRIDE, pfs::Archive, zone_lines::ZoneLines,
};

#[derive(Default, Debug)]
pub struct ZoneDiscovery {
    pub zones: BTreeSet<String>,
    pub errors: Vec<(String, String)>,
    pub archives_examined: usize,
}

/// Find zone declarations, not character/object archives with similar names.
/// Failed archive/metadata inspection is retained rather than silently skipped.
pub fn discover(base: &Path) -> Result<ZoneDiscovery> {
    let mut entries = std::fs::read_dir(base)?.collect::<std::io::Result<Vec<_>>>()?;
    entries.sort_by_key(|entry| entry.file_name());
    let mut result = ZoneDiscovery::default();
    for entry in entries {
        if !entry.file_type()?.is_file() {
            continue;
        }
        let path = entry.path();
        let Some(extension) = path.extension().and_then(|s| s.to_str()) else {
            continue;
        };
        if !extension.eq_ignore_ascii_case("eqg") && !extension.eq_ignore_ascii_case("s3d") {
            continue;
        }
        let Some(stem) = path.file_stem().and_then(|s| s.to_str()) else {
            continue;
        };
        result.archives_examined += 1;
        let found = (|| -> Result<bool> {
            let archive = Archive::open(&path)?;
            if extension.eq_ignore_ascii_case("eqg") {
                return Ok(archive
                    .names()
                    .iter()
                    .any(|name| name.to_ascii_lowercase().ends_with(".zon"))
                    || base.join(format!("{stem}.zon")).is_file());
            }
            let Some(bytes) = archive.read_opt(&format!("{stem}.wld"))? else {
                return Ok(false);
            };
            Ok(BspRegions::parse(&bytes)?.nodes.is_some())
        })();
        match found {
            Ok(true) => {
                result.zones.insert(stem.to_ascii_lowercase());
            }
            Ok(false) => {}
            Err(error) => result.errors.push((
                entry.file_name().to_string_lossy().into(),
                error.to_string(),
            )),
        }
    }
    Ok(result)
}

#[derive(Debug)]
pub struct ZoneMetadata {
    pub format: &'static str,
    pub version: Option<u32>,
    pub terrain_tiles: usize,
    pub terrain_groups: usize,
    /// None means this survey does not decode the format's region records.
    pub authored_regions: Option<usize>,
    pub liquid_status: &'static str,
    pub liquid_detail: Option<String>,
    pub border_status: &'static str,
}

pub fn metadata(base: &Path, zone: &str) -> Result<ZoneMetadata> {
    let path = loader::zone_archive(base, zone)?;
    let mut report = ZoneMetadata {
        format: "wld",
        version: None,
        terrain_tiles: 0,
        terrain_groups: 0,
        authored_regions: None,
        liquid_status: "not_checked",
        liquid_detail: None,
        border_status: "not_checked",
    };
    if path
        .extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case("eqg"))
    {
        let archive = Archive::open(path)?;
        let declaration = loader::read_eqg_declaration(base, zone, &archive)?;
        report.border_status = "unsupported_eqg";
        if declaration.trim_ascii_start().starts_with(b"EQTZP") {
            report.format = "heightmap";
            let map = loader::read_heightmap(&archive, &declaration)?;
            report.terrain_tiles = map.tiles.len();
            report.terrain_groups = map.groups.len();
            report.authored_regions = Some(map.regions.len());
            match LiquidRegions::from_heightmap_with_groups(&map, |name| {
                loader::read_terrain_group(base, &archive, name)
            }) {
                Ok(regions) => {
                    report.liquid_status = if regions.is_empty() {
                        "no_supported_volumes"
                    } else {
                        "supported_top_level_subset"
                    }
                }
                Err(error) => {
                    report.liquid_status = "unsupported_heightmap";
                    report.liquid_detail = Some(error.to_string());
                }
            }
        } else {
            report.format = "eqgz";
            report.version = declaration
                .get(4..8)
                .map(|b| u32::from_le_bytes(b.try_into().unwrap()));
            match crate::binary_regions::BinaryRegions::parse(&declaration) {
                Ok(regions) => {
                    report.authored_regions = Some(regions.boxes.len());
                    report.liquid_status = if LiquidRegions::from_binary_regions(regions).is_empty()
                    {
                        "no_supported_volumes"
                    } else {
                        "supported_eqgz"
                    };
                }
                Err(error) => {
                    report.liquid_status = "unsupported_eqgz";
                    report.liquid_detail = Some(error.to_string());
                }
            }
        }
    } else {
        let archive = Archive::open(path)?;
        let bytes = archive.read(&format!("{zone}.wld"))?;
        let bsp = BspRegions::parse(&bytes)?;
        report.authored_regions = Some(bsp.declarations.len());
        report.liquid_status = if LiquidRegions::from_wld(&bytes)?.is_empty() {
            "no_supported_volumes"
        } else {
            "supported_wld"
        };
        report.border_status = if ZoneLines::from_wld(&bytes)?.is_empty() {
            "no_supported_reference_triggers"
        } else {
            "supported_reference_subset"
        };
    }
    Ok(report)
}

#[derive(Default, Debug)]
pub struct GeometryAudit {
    pub meshes: usize,
    pub triangles: usize,
    /// None when invalid source structure prevented a safe build.
    pub collision_triangles: Option<usize>,
    pub instances: usize,
    pub lights: usize,
    pub texture_references: usize,
    pub invalid_meshes: usize,
    pub mesh_problems: BTreeMap<String, usize>,
    pub invalid_collision_meshes: usize,
    pub invalid_object_references: usize,
    pub invalid_instances: usize,
    /// Unresolved placements can include authored markers. They are diagnostics,
    /// not proof of invalid transforms or missing visible geometry.
    pub unresolved_objects: BTreeMap<String, usize>,
    /// Restored mesh placements whose attached native effects remain unsupported.
    pub unsupported_particle_placements: BTreeMap<String, usize>,
    pub invalid_lights: usize,
    pub native_ter_lighting_meshes: usize,
    pub ter_lighting_issues: BTreeMap<String, String>,
}

pub fn geometry(base: &Path, zone: &str) -> Result<GeometryAudit> {
    let scene = loader::load_zone(base, zone)?;
    let objects: BTreeSet<_> = scene
        .objects
        .iter()
        .map(|object| object.name.as_str())
        .collect();
    let mut report = GeometryAudit {
        meshes: scene.meshes.len(),
        triangles: scene.triangle_count(),
        instances: scene.instances.len(),
        lights: scene.lights.len(),
        texture_references: scene.texture_names().len(),
        native_ter_lighting_meshes: scene.native_ter_lighting.len(),
        ter_lighting_issues: scene.ter_lighting_issues.clone(),
        ..Default::default()
    };
    for mesh in &scene.meshes {
        let mut bad = false;
        for (problem, present) in [
            (
                "vertex_stride",
                !mesh.vertices.len().is_multiple_of(VERTEX_STRIDE),
            ),
            ("triangle_stride", !mesh.indices.len().is_multiple_of(3)),
            (
                "nonfinite_positions",
                mesh.vertices
                    .chunks_exact(VERTEX_STRIDE)
                    .any(|v| v[..3].iter().any(|x| !x.is_finite())),
            ),
            (
                "nonfinite_normals",
                mesh.vertices
                    .chunks_exact(VERTEX_STRIDE)
                    .any(|v| v[3..6].iter().any(|x| !x.is_finite())),
            ),
            (
                "nonfinite_uv",
                mesh.vertices
                    .chunks_exact(VERTEX_STRIDE)
                    .any(|v| v[6..].iter().any(|x| !x.is_finite())),
            ),
            (
                "vertex_index",
                mesh.indices
                    .iter()
                    .any(|index| *index as usize >= mesh.vertex_count()),
            ),
            ("material_index", mesh.material >= scene.materials.len()),
        ] {
            if present {
                bad = true;
                *report.mesh_problems.entry(problem.into()).or_default() += 1;
            }
        }
        report.invalid_meshes += usize::from(bad);
    }
    for instance in &scene.instances {
        if scene
            .wld_object_sources
            .get(&instance.object)
            .is_some_and(|source| !source.particle_attachments.is_empty())
        {
            *report
                .unsupported_particle_placements
                .entry(instance.object.clone())
                .or_default() += 1;
        }
        if !objects.contains(instance.object.as_str()) {
            *report
                .unresolved_objects
                .entry(instance.object.clone())
                .or_default() += 1;
        }
        if instance
            .position
            .iter()
            .chain(&instance.scale)
            .chain(&instance.rotation)
            .any(|v| !v.is_finite())
        {
            report.invalid_instances += 1;
        }
    }
    for mesh in &scene.collision_meshes {
        if !mesh.indices.len().is_multiple_of(3)
            || mesh
                .positions
                .iter()
                .flatten()
                .any(|value| !value.is_finite())
            || mesh
                .indices
                .iter()
                .any(|index| *index as usize >= mesh.positions.len())
        {
            report.invalid_collision_meshes += 1;
        }
    }
    for object in &scene.objects {
        report.invalid_object_references += object
            .meshes
            .iter()
            .filter(|&&index| index >= scene.meshes.len())
            .count();
        report.invalid_object_references += object
            .collision_meshes
            .iter()
            .filter(|&&index| index >= scene.collision_meshes.len())
            .count();
    }
    for light in &scene.lights {
        if light
            .position
            .iter()
            .chain(&light.color)
            .chain([&light.radius, &light.attenuation])
            .any(|v| !v.is_finite())
        {
            report.invalid_lights += 1;
        }
    }
    // Do not feed malformed indexed geometry to the collision builder.
    if report.invalid_meshes
        + report.invalid_instances
        + report.invalid_collision_meshes
        + report.invalid_object_references
        == 0
    {
        report.collision_triangles = Some(CollisionWorld::build(&scene).triangle_count());
    }
    Ok(report)
}
