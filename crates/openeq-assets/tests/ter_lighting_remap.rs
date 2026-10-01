//! Original-index lighting must survive grouping and vertex deduplication.
use openeq_assets::{loader, mesh, pfs::Archive, zone::TerMod};

fn raw_embedded(bytes: &[u8]) -> Vec<u32> {
    let word = |at| u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap()) as usize;
    assert_eq!(&bytes[..4], b"EQGZ");
    assert_eq!(word(4), 2);
    let first = 28 + word(8) + word(12) * 4;
    let count = word(first + 36);
    bytes[first + 40..first + 40 + count * 4]
        .chunks_exact(4)
        .map(|b| u32::from_le_bytes(b.try_into().unwrap()))
        .collect()
}

#[test]
#[ignore = "requires original TER and lighting assets"]
fn original_material_corners_keep_their_distinct_lighting_and_geometry() {
    let base = loader::default_client_dir().unwrap();
    for (zone, expected_extra) in [
        ("causeway", 0),
        ("delveb", 61),
        ("guildhall", 0),
        ("guildlobby", 831),
        ("roost", 0),
        ("thundercrest", 9991),
    ] {
        let archive = Archive::open(base.join(format!("{zone}.eqg"))).unwrap();
        let terrain: Vec<_> = archive
            .names()
            .iter()
            .filter(|name| name.to_ascii_lowercase().ends_with(".ter"))
            .collect();
        assert_eq!(terrain.len(), 1, "fixture terrain selection: {zone}");
        let ter_name = terrain[0];
        let ter = TerMod::parse(&archive.read(ter_name).unwrap(), true).unwrap();
        let colors = if ter.version == 3 {
            raw_embedded(&std::fs::read(base.join(format!("{zone}.zon"))).unwrap())
        } else {
            let name = format!("{}.lit", ter_name.trim_end_matches(".ter"));
            let bytes = archive.read(&name).unwrap();
            assert_eq!(&bytes[..4], b"EQGP");
            let count = u32::from_le_bytes(bytes[4..8].try_into().unwrap()) as usize;
            assert_eq!(count, ter.positions.len());
            bytes[8..8 + count * 4]
                .chunks_exact(4)
                .map(|b| u32::from_le_bytes(b.try_into().unwrap()))
                .collect()
        };
        assert_eq!(colors.len(), ter.positions.len());
        let scene = loader::load_zone(&base, zone).unwrap();
        let groups = ter.mesh_groups();
        let mut ordinals: Vec<_> = groups
            .keys()
            .filter(|&&ordinal| {
                ter.material_for_polygon(ordinal)
                    .is_some_and(|mat| mat.shader == "Opaque_MaxCB1.fx")
            })
            .copied()
            .collect();
        ordinals.sort_unstable();
        assert_eq!(scene.native_ter_lighting.len(), ordinals.len(), "{zone}");
        let mut extra = 0;
        for ((mesh_index, metadata), ordinal) in scene.native_ter_lighting.iter().zip(ordinals) {
            assert_eq!(
                metadata.selection.colors, colors,
                "{zone}: source selection"
            );
            let geometry = &scene.meshes[*mesh_index];
            assert_eq!(metadata.source_indices.len(), geometry.vertex_count());
            let original_corners = &groups[&ordinal];
            assert_eq!(geometry.indices.len(), original_corners.len());
            for (&packed, &original) in geometry.indices.iter().zip(original_corners) {
                let representative = metadata.source_indices[packed as usize] as usize;
                assert_eq!(
                    colors[representative], colors[original as usize],
                    "{zone}: material{ordinal} corner{original}"
                );
                let original = original as usize;
                let expected: Vec<_> = ter.positions[original]
                    .into_iter()
                    .chain(ter.normals[original])
                    .chain(ter.tex_coords[original])
                    .map(f32::to_bits)
                    .collect();
                let at = packed as usize * 8;
                let actual: Vec<_> = geometry.vertices[at..at + 8]
                    .iter()
                    .map(|x| x.to_bits())
                    .collect();
                assert_eq!(actual, expected, "{zone}: material{ordinal} geometry");
            }
            let (old, _) = mesh::pack(
                &ter.positions,
                &ter.normals,
                &ter.tex_coords,
                original_corners,
            );
            assert!(geometry.vertex_count() >= old.len() / 8);
            extra += geometry.vertex_count() - old.len() / 8;
        }
        assert_eq!(
            extra, expected_extra,
            "{zone}: independent raw-color dedup audit"
        );
        eprintln!(
            "{zone}: {} material groups; {extra} distinct lighting vertices restored",
            scene.native_ter_lighting.len()
        );
    }
}

#[path = "support/eqg_collision.rs"]
mod support;

#[test]
fn malformed_auxiliary_lighting_does_not_hide_valid_terrain() {
    use support::*;
    let mut terrain = model(true);
    terrain.version = 2;
    terrain.materials.push(material("Opaque_MaxCB1.fx", None));
    quad(&mut terrain, floor(0.), 0, 0);
    let fixture = Fixture::new(&[("test.ter", &terrain)], &[]);
    let baseline = fixture.scene();
    assert_eq!(baseline.native_ter_lighting.len(), 1);
    assert!(baseline.ter_lighting_issues.is_empty());
    // The archived scene declaration stays valid. A separate malformed loose
    // v2 stream must not turn independent terrain into a zone-load failure.
    std::fs::write(
        fixture.0.join("fixture.zon"),
        [b"EQGZ".as_slice(), &2u32.to_le_bytes()].concat(),
    )
    .unwrap();
    let scene = fixture.scene();
    assert!(scene.native_ter_lighting.is_empty());
    assert_eq!(scene.ter_lighting_issues.len(), 1);
    assert!(scene.ter_lighting_issues.contains_key("test.ter"));
    assert_eq!(scene.triangle_count(), baseline.triangle_count());
    assert_eq!(scene.meshes[0].vertices, baseline.meshes[0].vertices);
    assert_eq!(scene.meshes[0].indices, baseline.meshes[0].indices);
    let report = openeq_assets::audit::geometry(&fixture.0, "fixture").unwrap();
    assert_eq!(report.native_ter_lighting_meshes, 0);
    assert_eq!(report.ter_lighting_issues, scene.ter_lighting_issues);
    assert_eq!(report.invalid_meshes, 0);
}
