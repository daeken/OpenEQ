//! Exact, complete static TER MaxLava recipes. No shared-effect defaults.
use super::*;

pub(super) fn recipe(object: &TerMod, material: &TerMaterial) -> Option<TerLava> {
    if !object.is_terrain
        || !matches!(object.version, 1..=3)
        || material.shader != "Opaque_MaxLava.fx"
    {
        return None;
    }
    let texture = |key| {
        let name = material.properties.get(key)?.as_text()?;
        (!name.is_empty() && !name.eq_ignore_ascii_case("none")).then(|| name.to_owned())
    };
    let mut rates = [0.; 4];
    for (rate, key) in
        rates
            .iter_mut()
            .zip(["e_fSlide1X", "e_fSlide1Y", "e_fSlide2X", "e_fSlide2Y"])
    {
        let Property::Float(value) = material.properties.get(key)? else {
            return None;
        };
        if !value.is_finite() {
            return None;
        }
        *rate = *value;
    }
    Some(TerLava {
        top: texture("e_TextureDiffuse0")?,
        bottom: texture("e_TextureDiffuse1")?,
        normal: texture("e_TextureNormal0")?,
        rates,
    })
}

pub(super) fn is_static(scene: &Scene, lava: &TerLava) -> bool {
    // register_texture can resolve from any scene archive. Conservatively
    // reject a sidecar in any candidate, even when a nearer texture wins.
    !scene.archives.iter().any(|archive| {
        [&lava.top, &lava.bottom, &lava.normal].iter().any(|name| {
            let sidecar = Path::new(name).with_extension("txt");
            archive.contains(&sidecar.to_string_lossy())
                || archive.path().parent().is_some_and(|base| {
                    [
                        base.to_path_buf(),
                        base.join("Resources"),
                        base.join("Resources/waterswap"),
                    ]
                    .iter()
                    .any(|dir| case_insensitive_file(dir, &sidecar.to_string_lossy()).is_file())
                })
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn archive(files: &[(&str, &[u8])]) -> Archive {
        use std::io::Write;
        let mut bytes = vec![0; 12];
        bytes[4..8].copy_from_slice(&crate::pfs::PFS_MAGIC.to_le_bytes());
        let mut entries = Vec::new();
        let mut names = (files.len() as u32).to_le_bytes().to_vec();
        let mut block = |crc: u32, data: &[u8]| {
            let mut encoder =
                flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::fast());
            encoder.write_all(data).unwrap();
            let compressed = encoder.finish().unwrap();
            let offset = bytes.len() as u32;
            bytes.extend((compressed.len() as u32).to_le_bytes());
            bytes.extend((data.len() as u32).to_le_bytes());
            bytes.extend(compressed);
            entries.push((crc, offset, data.len() as u32));
        };
        for (index, (name, data)) in files.iter().enumerate() {
            block(index as u32 + 1, data);
            names.extend((name.len() as u32 + 1).to_le_bytes());
            names.extend(name.as_bytes());
            names.push(0);
        }
        block(crate::pfs::DIR_CRC, &names);
        let offset = bytes.len() as u32;
        bytes[..4].copy_from_slice(&offset.to_le_bytes());
        bytes.extend((entries.len() as u32).to_le_bytes());
        for (crc, offset, size) in entries {
            for word in [crc, offset, size] {
                bytes.extend(word.to_le_bytes());
            }
        }
        Archive::from_bytes("lava-fixture.eqg".into(), bytes).unwrap()
    }

    fn fixture() -> TerMod {
        TerMod {
            is_terrain: true,
            version: 2,
            materials: vec![TerMaterial {
                stored_id: 0,
                name: "lava".into(),
                shader: "Opaque_MaxLava.fx".into(),
                properties: [
                    ("e_TextureDiffuse0", Property::Text("top.dds".into())),
                    ("e_TextureDiffuse1", Property::Text("bottom.dds".into())),
                    ("e_TextureNormal0", Property::Text("normal.dds".into())),
                    ("e_fSlide1X", Property::Float(0.3)),
                    ("e_fSlide1Y", Property::Float(0.)),
                    ("e_fSlide2X", Property::Float(0.2)),
                    ("e_fSlide2Y", Property::Float(-0.)),
                ]
                .into_iter()
                .map(|(k, v)| (k.into(), v))
                .collect(),
            }],
            positions: vec![[0., 0., 0.], [1., 0., 0.], [0., 1., 0.]],
            normals: vec![[0., 0., 1.]; 3],
            tex_coords: vec![[128.125, -129.75]; 3],
            vertex_colors: None,
            secondary_tex_coords: Some(vec![[99., f32::NAN]; 3]),
            polygons: vec![(0, 1, 2, 0, 0)],
        }
    }

    #[test]
    fn exact_ter_family_requires_all_authored_bindings_without_defaults() {
        let mut object = fixture();
        let material = object.materials[0].clone();
        for version in 1..=3 {
            object.version = version;
            assert_eq!(
                recipe(&object, &material).unwrap().rates.map(f32::to_bits),
                [
                    0.3_f32.to_bits(),
                    0,
                    0.2_f32.to_bits(),
                    (-0.0_f32).to_bits()
                ]
            );
        }
        object.version = 4;
        assert!(recipe(&object, &material).is_none());
        object.version = 2;
        object.is_terrain = false;
        assert!(recipe(&object, &material).is_none());
        object.is_terrain = true;
        for shader in [
            "Opaque_MaxLava2.fx",
            "opaque_maxlava.fx",
            "Alpha_MaxLava.fx",
        ] {
            let mut m = material.clone();
            m.shader = shader.into();
            assert!(recipe(&object, &m).is_none());
        }
        for key in material.properties.keys() {
            let mut m = material.clone();
            m.properties.remove(key);
            assert!(recipe(&object, &m).is_none(), "missing {key}");
        }
        for value in [
            Property::Float(f32::NAN),
            Property::Float(f32::INFINITY),
            Property::IntegerBits(0),
        ] {
            let mut m = material.clone();
            m.properties.insert("e_fSlide1X".into(), value);
            assert!(recipe(&object, &m).is_none());
        }
    }

    #[test]
    fn bake_retains_source_uv_collision_and_independent_bottom_binding() {
        let object = fixture();
        let textures = ["top.dds", "bottom.dds", "normal.dds"].map(|name| Texture {
            name: name.into(),
            width: 1,
            height: 1,
            rgba: vec![255; 4],
        });
        let mut scene = Scene::from_geometry("lava".into(), vec![], vec![], textures.into());
        append_eqg_object(&mut scene, &object, "terrain", 0, None);
        assert_eq!(
            scene.ter_lava[&0],
            recipe(&object, &object.materials[0]).unwrap()
        );
        assert_eq!(scene.materials[0].textures, ["top.dds"]);
        assert!(scene.secondary_ter_uv.is_empty());
        assert_eq!(scene.materials[0].uv_encoding, mesh::UvEncoding::Float32);
        for v in scene.meshes[0].vertices.chunks_exact(8) {
            assert_eq!(&v[6..8], &[128.125, -129.75]);
        }
        assert_eq!(scene.collision_meshes[0].indices.len(), 3);
    }

    #[test]
    fn sidecars_in_fallback_archives_exclude_every_animated_texture_binding() {
        for sidecar in [
            None,
            Some("TOP.txt"),
            Some("bottom.txt"),
            Some("normal.txt"),
        ] {
            let mut scene = Scene::from_geometry("lava".into(), vec![], vec![], vec![]);
            scene.archives.push(archive(&[("terrain.ter", b"source")]));
            let mut files: Vec<(&str, &[u8])> = vec![
                ("top.dds", b"top"),
                ("bottom.dds", b"bottom"),
                ("normal.dds", b"normal"),
            ];
            if let Some(sidecar) = sidecar {
                files.push((sidecar, b"animation"));
            }
            scene.archives.push(archive(&files));
            append_eqg_object(&mut scene, &fixture(), "terrain", 0, None);
            assert_eq!(scene.ter_lava.contains_key(&0), sidecar.is_none());
            assert!(matches!(
                scene.textures.get("top.dds"),
                Some(TextureSource::Archive(1, _))
            ));
            assert!(matches!(
                scene.textures.get("normal.dds"),
                Some(TextureSource::Archive(1, _))
            ));
            assert_eq!(scene.materials[0].textures, ["top.dds"]);
            assert_eq!(scene.materials[0].uv_encoding, mesh::UvEncoding::Float32);
        }
    }
}
