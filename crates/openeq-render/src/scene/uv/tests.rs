use super::*;

#[test]
fn native_instruction_witnesses_cover_quantization_wrapping_and_invalids() {
    // Independent EQGraphicsDX9 layout-1 probe results, not Rust casts used
    // as the expected-value oracle. Words are original input f32 bits.
    for (word, packed) in [
        (0x0000_0000, 0_i16),
        (0x8000_0000, 0),
        (0x0000_0001, 0),
        (0x807f_ffff, 0),
        (0x3f80_0001, 256),
        (0xbf80_0001, -256),
        (0x3b7f_ffff, 0),
        (0xbb7f_ffff, 0),
        (0x42ff_ffff, 32767),
        (0x4300_0000, -32768),
        (0xc300_0000, -32768),
        (0x477f_ffff, -1),
        (0x4aff_ffff, -128),
        (0x4b00_0000, 0),
        // These differ from the legacy x87 branch; do not use i64 casts.
        (0x4b00_0001, 0),
        (0xcb00_0001, 0),
        (0x4eff_ffff, 0),
        (0x4f00_0000, 0),
        (0x5aff_ffff, 0),
        (0x5b00_0000, 0),
        (0x638a_681c, 0),
        (0xe38a_6804, 0),
        (0x7f80_0000, 0),
        (0xff80_0000, 0),
        (0x7fff_ffff, 0),
        (0xffff_ffff, 0),
        (0x7f80_0001, 0),
        (0x7f7f_ffff, 0),
        (0xff7f_ffff, 0),
    ] {
        let value = f32::from_bits(word);
        assert_eq!(
            short2_sse2(value).to_bits(),
            (f32::from(packed) / 256.0).to_bits(),
            "{word:08x}"
        );
        assert_eq!(
            shader_uv(UvEncoding::Float32, [value; 2]).map(f32::to_bits),
            [word; 2]
        );
    }
}

#[test]
#[ignore = "requires original Causeway assets and GPU; writes /tmp/openeq-ter-uv.png"]
fn original_causeway_upload_and_native_uv_pixel_witness() {
    use crate::{Camera, GpuScene, Renderer, environment::EnvironmentSettings};
    use openeq_assets::{Scene, loader, mesh::Geometry, pfs::Archive, zone::TerMod};
    use std::time::Duration;

    let base = loader::default_client_dir().unwrap();
    let archive = Archive::open(base.join("causeway.eqg")).unwrap();
    let ter = TerMod::parse(&archive.read("ter_gorge.ter").unwrap(), true).unwrap();
    let source = loader::load_zone(&base, "causeway").unwrap();
    let mut renderer = Renderer::new_headless(256, 256).unwrap();
    renderer.set_environment(
        EnvironmentSettings {
            sky_enabled: false,
            fog_enabled: false,
            ..Default::default()
        },
        None,
    );
    let full = GpuScene::build(renderer.device(), renderer.queue(), &source).unwrap();
    let (mesh_index, mesh) = source
        .meshes
        .iter()
        .enumerate()
        .find(|(_, mesh)| {
            mesh.indices.len() == 879 * 3
                && source.materials[mesh.material].textures == ["sp2_c.dds"]
        })
        .unwrap();
    let start: usize = source.meshes[..mesh_index]
        .iter()
        .map(|m| m.vertex_count())
        .sum();
    assert_eq!(
        source.materials[mesh.material].uv_encoding,
        UvEncoding::NativeTerShort2Sse2
    );
    for (source_index, expected) in [
        (87375, [511. / 256., 0.]),
        (96771, [0., 0.]),
        (96772, [0., 0.]),
        (96773, [0., 0.]),
        (96775, [0., 0.]),
        (97517, [0., 0.]),
    ] {
        let raw: Vec<_> = ter.positions[source_index]
            .into_iter()
            .chain(ter.normals[source_index])
            .chain(ter.tex_coords[source_index])
            .map(f32::to_bits)
            .collect();
        let index = mesh
            .vertices
            .chunks_exact(8)
            .position(|v| v.iter().map(|x| x.to_bits()).eq(raw.iter().copied()))
            .unwrap();
        assert_eq!(full.vertex_data[start + index].uv, expected);
        assert_eq!(
            full.vertex_data[start + index].position.map(f32::to_bits),
            ter.positions[source_index].map(f32::to_bits)
        );
        assert_eq!(
            full.vertex_data[start + index].normal.map(f32::to_bits),
            ter.normals[source_index].map(f32::to_bits)
        );
    }
    assert!(
        full.vertex_data[start..start + mesh.vertex_count()]
            .iter()
            .all(|v| v.uv.iter().all(|x| x.is_finite()))
    );
    assert_eq!(
        mesh.vertices
            .chunks_exact(8)
            .filter(|v| !v[6].is_finite() || !v[7].is_finite())
            .count(),
        9
    );
    drop(full);

    // Enlarge four independent native UV witnesses onto a visible quad. This
    // tests interpolation/sampling after upload, not visibility of Causeway's
    // original thin triangles. The original diffuse texture is unchanged.
    let mut material = source.materials[mesh.material].clone();
    material.emissive = true;
    let texture = source.texture("sp2_c.dds").unwrap();
    let raw_uv = [87375, 96771, 96772, 97517].map(|i| ter.tex_coords[i]);
    let points = [
        [-10., 10., -10.],
        [10., 10., -10.],
        [10., 10., 10.],
        [-10., 10., 10.],
    ];
    let geometry = |uv: [[f32; 2]; 4]| Geometry {
        vertices: points
            .into_iter()
            .zip(uv)
            .flat_map(|(p, uv)| p.into_iter().chain([0., -1., 0.]).chain(uv))
            .collect(),
        indices: vec![0, 1, 2, 0, 2, 3],
        material: 0,
        collidable: false,
    };
    let converted = Scene::from_geometry(
        "native UV witness".into(),
        vec![material.clone()],
        vec![geometry(raw_uv)],
        vec![texture.clone()],
    );
    material.uv_encoding = UvEncoding::Float32;
    let expected = Scene::from_geometry(
        "native UV expected".into(),
        vec![material],
        vec![geometry([[511. / 256., 0.], [0., 0.], [0., 0.], [0., 0.]])],
        vec![texture],
    );
    let capture = |renderer: &mut Renderer, scene: &Scene| {
        let gpu = GpuScene::build(renderer.device(), renderer.queue(), scene).unwrap();
        renderer.set_scene(&gpu);
        renderer.render_at(
            &gpu,
            &Camera {
                pitch: 0.,
                ..Default::default()
            },
            Duration::ZERO,
        );
        renderer.read_rgba().unwrap().2
    };
    let actual = capture(&mut renderer, &converted);
    assert_eq!(actual, capture(&mut renderer, &expected));
    let empty = Scene::from_geometry("empty".into(), vec![], vec![], vec![]);
    let background = capture(&mut renderer, &empty);
    assert!(
        actual
            .chunks_exact(4)
            .zip(background.chunks_exact(4))
            .filter(|(a, b)| a != b)
            .count()
            > 20_000
    );
    image::save_buffer(
        "/tmp/openeq-ter-uv.png",
        &actual,
        256,
        256,
        image::ColorType::Rgba8,
    )
    .unwrap();
}
