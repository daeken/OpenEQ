//! Original TER dual-coordinate metadata: independent raw corner comparison.
use std::collections::HashMap;

use openeq_assets::{loader, mesh::UvEncoding, pfs::Archive};

fn word(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap())
}
fn name(bytes: &[u8], offset: usize) -> &[u8] {
    bytes[offset..].split(|byte| *byte == 0).next().unwrap()
}

#[test]
#[ignore = "requires original EverQuest assets"]
fn original_nest_dual_uv_corners_match_both_raw_source_streams() {
    let base = loader::default_client_dir().expect("original client directory");
    let bytes = Archive::open(base.join("thenest.eqg"))
        .unwrap()
        .read("ter_abyss01.ter")
        .unwrap();
    assert_eq!(&bytes[..4], b"EQGT");
    assert_eq!(word(&bytes, 4), 2);
    let string_size = word(&bytes, 8) as usize;
    let material_count = word(&bytes, 12) as usize;
    let vertex_count = word(&bytes, 16) as usize;
    let polygon_count = word(&bytes, 20) as usize;
    assert_eq!(
        (material_count, vertex_count, polygon_count),
        (3813, 384042, 320324)
    );
    let strings = &bytes[24..24 + string_size];
    let mut first = HashMap::new();
    let mut selected = Vec::new();
    let mut cursor = 24 + string_size;
    for ordinal in 0..material_count {
        let material_name = name(strings, word(&bytes, cursor + 4) as usize);
        let shader = name(strings, word(&bytes, cursor + 8) as usize);
        let is_dual = matches!(shader, b"Opaque_MaxCB1_2UV.fx" | b"Opaque_MaxCBSG1_2UV.fx");
        let resolved = *first.entry(material_name).or_insert(ordinal);
        selected.push(if resolved == ordinal {
            is_dual
        } else {
            selected[resolved]
        });
        cursor += 16 + word(&bytes, cursor + 12) as usize * 12;
    }
    let vertex_start = cursor;
    let polygon_start = vertex_start + vertex_count * 32;
    let tail = polygon_start + polygon_count * 20;
    assert_eq!(
        (vertex_start, polygon_start, tail),
        (827972, 13117316, 19523796)
    );
    assert_eq!(word(&bytes, tail), 1);
    let source = |index: usize| -> [u32; 10] {
        std::array::from_fn(|component| {
            word(
                &bytes,
                if component < 8 {
                    vertex_start + index * 32 + component * 4
                } else {
                    tail + 4 + index * 8 + (component - 8) * 4
                },
            )
        })
    };
    let mut expected = HashMap::<[u32; 30], usize>::new();
    let mut expected_triangles = 0;
    for polygon in 0..polygon_count {
        let offset = polygon_start + polygon * 20;
        let material = word(&bytes, offset + 12) as usize;
        if !selected.get(material).copied().unwrap_or(false) {
            continue;
        }
        let corners: [[u32; 10]; 3] =
            std::array::from_fn(|corner| source(word(&bytes, offset + corner * 4) as usize));
        let key = std::array::from_fn(|component| corners[component / 10][component % 10]);
        *expected.entry(key).or_default() += 1;
        expected_triangles += 1;
    }
    assert_eq!(expected_triangles, 305522);
    let scene = loader::load_zone(&base, "thenest").unwrap();
    let mut actual_triangles = 0;
    for (&mesh_index, metadata) in &scene.secondary_ter_uv {
        let mesh = &scene.meshes[mesh_index];
        assert_eq!(metadata.tex_coords.len(), mesh.vertex_count());
        assert_eq!(metadata.source_indices.len(), mesh.vertex_count());
        assert_eq!(
            scene.materials[mesh.material].uv_encoding,
            UvEncoding::Float32
        );
        assert!(scene.materials[mesh.material].waterfall.is_none());
        assert!(!scene.native_ter_lighting.contains_key(&mesh_index));
        let packed = |index: usize| -> [u32; 10] {
            std::array::from_fn(|component| {
                if component < 8 {
                    mesh.vertices[index * 8 + component].to_bits()
                } else {
                    metadata.tex_coords[index][component - 8].to_bits()
                }
            })
        };
        for (index, &original) in metadata.source_indices.iter().enumerate() {
            assert_eq!(packed(index), source(original as usize));
        }
        for indices in mesh.indices.chunks_exact(3) {
            let corners: [[u32; 10]; 3] =
                std::array::from_fn(|corner| packed(indices[corner] as usize));
            let key = std::array::from_fn(|component| corners[component / 10][component % 10]);
            let remaining = expected
                .get_mut(&key)
                .expect("baked corner tuple exists in raw TER");
            *remaining = remaining
                .checked_sub(1)
                .expect("no duplicate baked triangle");
            actual_triangles += 1;
        }
    }
    assert_eq!(actual_triangles, expected_triangles);
    assert!(expected.values().all(|&remaining| remaining == 0));
}
