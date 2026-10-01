//! Native TER UV provenance must survive loading without editing raw data.
#[path = "support/eqg_collision.rs"]
mod support;

use openeq_assets::{Scene, loader, mesh, mesh::UvEncoding, pfs::Archive, zone::TerMod};
use support::*;

#[test]
fn ter_uv_scope_versions_raw_words_and_material_remapping() {
    for version in [0, 1, 2, 3, 4] {
        for is_terrain in [false, true] {
            let mut source = model(is_terrain);
            source.version = version;
            for (index, shader) in [
                "Opaque_MaxCB1.fx",
                "OPAQUE_MAXCB1.FX",
                "Opaque_MaxCB2.fx",
                "Opaque_MaxCB1.fx.extra",
                "AddAlpha_MaxCB1.fx",
                "Opaque_MaxC1.fx",
                "Opaque_MPLBump.fx",
                "Opaque_MaxCB1_2UV.fx",
                "Opaque_MaxCB2.fx",
            ]
            .into_iter()
            .enumerate()
            {
                let mut m = material(shader, None);
                m.name = format!("material {index}");
                source.materials.push(m);
                quad(&mut source, floor(index as f32), index as u32, 0);
            }
            // Duplicate names resolve to the first record before provenance
            // selection; stored IDs are not the material lookup key.
            source.materials[2].name = source.materials[0].name.clone();
            source.materials[3].stored_id = source.materials[0].stored_id;
            source.tex_coords[0] = [f32::from_bits(0xffff_ffff), -0.0];
            source.tex_coords[1] = [f32::from_bits(0x638a_681c), 128.0];
            let filename = if is_terrain { "test.ter" } else { "test.mod" };
            let fixture = Fixture::new(&[(filename, &source)], &[]);
            let archive = Archive::open(fixture.0.join("fixture.eqg")).unwrap();
            let parsed = TerMod::parse(&archive.read(filename).unwrap(), is_terrain).unwrap();
            assert_eq!(parsed.version, version);
            assert_eq!(parsed.polygons, source.polygons);
            assert_eq!(
                parsed
                    .tex_coords
                    .iter()
                    .flatten()
                    .map(|v| v.to_bits())
                    .collect::<Vec<_>>(),
                source
                    .tex_coords
                    .iter()
                    .flatten()
                    .map(|v| v.to_bits())
                    .collect::<Vec<_>>()
            );
            let scene = fixture.scene();
            let groups = source.mesh_groups();
            for (index, draw) in scene.meshes.iter().enumerate() {
                let (vertices, indices) = mesh::pack(
                    &source.positions,
                    &source.normals,
                    &source.tex_coords,
                    &groups[&(index as u32)],
                );
                assert_eq!(draw.indices, indices);
                assert_eq!(
                    draw.vertices
                        .iter()
                        .map(|v| v.to_bits())
                        .collect::<Vec<_>>(),
                    vertices.iter().map(|v| v.to_bits()).collect::<Vec<_>>()
                );
                let expected = if is_terrain && (1..=3).contains(&version) && matches!(index, 0 | 2)
                {
                    UvEncoding::NativeTerShort2Sse2
                } else {
                    UvEncoding::Float32
                };
                assert_eq!(scene.materials[draw.material].uv_encoding, expected);
                // Clone/reindex exactly as object extraction and material
                // isolation do. Encoding travels with the material itself.
                let mut mesh = draw.clone();
                let material = scene.materials[mesh.material].clone();
                mesh.material = 0;
                let extracted =
                    Scene::from_geometry("isolated".into(), vec![material], vec![mesh], vec![]);
                assert_eq!(extracted.materials[0].uv_encoding, expected);
            }
        }
    }
}

#[test]
#[ignore = "requires original Causeway assets"]
fn original_causeway_keeps_all_source_geometry_and_nonfinite_words() {
    let base = loader::default_client_dir().unwrap();
    let archive = Archive::open(base.join("causeway.eqg")).unwrap();
    let ter = TerMod::parse(&archive.read("ter_gorge.ter").unwrap(), true).unwrap();
    let scene = loader::load_zone(&base, "causeway").unwrap();
    let groups = ter.mesh_groups();
    let material = &ter.materials[6];
    assert_eq!(material.shader, "Opaque_MaxCB1.fx");
    let (vertices, indices) =
        mesh::pack(&ter.positions, &ter.normals, &ter.tex_coords, &groups[&6]);
    let original_bits: Vec<_> = vertices.iter().map(|v| v.to_bits()).collect();
    let matching: Vec<_> = scene
        .meshes
        .iter()
        .filter(|mesh| {
            mesh.indices == indices
                && mesh
                    .vertices
                    .iter()
                    .map(|v| v.to_bits())
                    .eq(original_bits.iter().copied())
        })
        .collect();
    assert_eq!(matching.len(), 1);
    let draw = matching[0];
    assert_eq!(draw.indices.len(), 879 * 3);
    assert_eq!(
        scene.materials[draw.material].uv_encoding,
        UvEncoding::NativeTerShort2Sse2
    );
    assert_eq!(
        ter.tex_coords[96771].map(f32::to_bits),
        [0x8000_0000, 0xffff_ffff]
    );
    assert_eq!(
        ter.tex_coords[96772].map(f32::to_bits),
        [0x638a_681c, 0xe38a_6804]
    );
    assert_eq!(
        draw.vertices
            .chunks_exact(8)
            .filter(|v| !v[6].is_finite() || !v[7].is_finite())
            .count(),
        9
    );
    assert_eq!(
        draw.vertices
            .iter()
            .map(|v| v.to_bits())
            .collect::<Vec<_>>(),
        original_bits
    );
}

#[test]
#[ignore = "requires original Guild Hall, Guild Lobby and Roost assets"]
fn original_version_three_ter_materials_preserve_primary_uvs() {
    let base = loader::default_client_dir().unwrap();
    for (zone, file, expected_groups) in [
        ("guildhall", "ter_guildhall.ter", 1),
        ("guildlobby", "ter_guildlobby.ter", 4),
        ("roost", "ter_roost.ter", 1),
    ] {
        let archive = Archive::open(base.join(format!("{zone}.eqg"))).unwrap();
        let source = TerMod::parse(&archive.read(file).unwrap(), true).unwrap();
        assert_eq!(source.version, 3);
        let scene = loader::load_zone(&base, zone).unwrap();
        let mut selected = 0;
        for (ordinal, indices) in source.mesh_groups() {
            let Some(material) = source.material_for_polygon(ordinal) else {
                continue;
            };
            if !material.shader.eq_ignore_ascii_case("Opaque_MaxCB1.fx") {
                continue;
            }
            selected += 1;
            let (vertices, indices) = mesh::pack(
                &source.positions,
                &source.normals,
                &source.tex_coords,
                &indices,
            );
            assert!(
                scene.meshes.iter().any(|draw| {
                    scene.materials[draw.material].uv_encoding == UvEncoding::NativeTerShort2Sse2
                        && draw.indices.len() == indices.len()
                        && draw
                            .indices
                            .iter()
                            .zip(&indices)
                            .all(|(&actual, &previous)| {
                                let a = actual as usize * mesh::VERTEX_STRIDE;
                                let b = previous as usize * mesh::VERTEX_STRIDE;
                                draw.vertices[a..a + mesh::VERTEX_STRIDE]
                                    .iter()
                                    .map(|v| v.to_bits())
                                    .eq(vertices[b..b + mesh::VERTEX_STRIDE]
                                        .iter()
                                        .map(|v| v.to_bits()))
                            })
                }),
                "{zone} material {ordinal} changed raw triangle-corner attributes"
            );
        }
        assert_eq!(selected, expected_groups, "{zone}");
    }
}
