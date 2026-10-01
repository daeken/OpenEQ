use super::*;
use crate::zone::{Property, TerMaterial};

fn fixture() -> (TerMod, TerMaterial) {
    let object = TerMod {
        is_terrain: true,
        version: 2,
        materials: vec![],
        positions: vec![],
        normals: vec![],
        tex_coords: vec![],
        vertex_colors: None,
        secondary_tex_coords: None,
        polygons: vec![],
    };
    let mut material = TerMaterial {
        stored_id: 0,
        name: "waterfall".into(),
        shader: "Opaque_MaxWaterFall.fx".into(),
        properties: [
            ("e_fSlide1X", -0.12),
            ("e_fSlide1Y", -0.32),
            ("e_fSlide2X", 0.),
            ("e_fSlide2Y", -0.5),
        ]
        .into_iter()
        .map(|(key, value)| (key.into(), Property::Float(value)))
        .collect(),
    };
    material.properties.insert(
        "e_TextureDiffuse0".into(),
        Property::Text("waterfall.dds".into()),
    );
    (object, material)
}

#[test]
fn waterfall_admission_requires_exact_ter_family_and_complete_finite_authored_state() {
    let (mut object, mut material) = fixture();
    for version in [1, 2, 3] {
        object.version = version;
        assert_eq!(
            waterfall_material(&object, &material),
            Some([-0.12, -0.32, 0., -0.5])
        );
        assert_eq!(
            ter_uv::encoding(&object, &material),
            mesh::UvEncoding::NativeTerShort2Sse2
        );
    }
    for version in [0, 4] {
        object.version = version;
        assert!(waterfall_material(&object, &material).is_none());
    }
    object.version = 2;
    object.is_terrain = false;
    assert!(waterfall_material(&object, &material).is_none());
    object.is_terrain = true;
    for name in [
        "opaque_maxwaterfall.fx",
        "Opaque_MaxWaterFall2.fx",
        "Alpha_MaxWaterFall.fx",
    ] {
        material.shader = name.into();
        assert!(waterfall_material(&object, &material).is_none());
    }
    material.shader = "Opaque_MaxWaterFall.fx".into();
    for value in [
        Property::IntegerBits(0),
        Property::Uint(0),
        Property::Text("0".into()),
        Property::Float(f32::NAN),
        Property::Float(f32::INFINITY),
    ] {
        material.properties.insert("e_fSlide1X".into(), value);
        assert!(waterfall_material(&object, &material).is_none());
    }
    material.properties.remove("e_fSlide1X");
    assert!(
        waterfall_material(&object, &material).is_none(),
        "missing values must not invent defaults"
    );
}

#[test]
fn waterfall_bake_preserves_raw_primary_uvs_material_rates_and_collision() {
    let (mut object, material) = fixture();
    object.materials.push(material);
    object.positions = vec![[0., 0., 0.], [1., 0., 0.], [0., 1., 0.]];
    object.normals = vec![[0., 0., 1.]; 3];
    object.tex_coords = vec![[0.12345, -129.75]; 3];
    object.secondary_tex_coords = Some(vec![[99., f32::NAN]; 3]);
    object.polygons.push((0, 1, 2, 0, 0));
    let mut scene = Scene::from_geometry(
        "waterfall".into(),
        vec![],
        vec![],
        vec![Texture {
            name: "waterfall.dds".into(),
            width: 1,
            height: 1,
            rgba: vec![255; 4],
        }],
    );
    append_eqg_object(&mut scene, &object, "terrain", 0, None);
    assert_eq!(scene.materials[0].waterfall, Some([-0.12, -0.32, 0., -0.5]));
    assert_eq!(scene.meshes[0].indices.len(), 3);
    for vertex in scene.meshes[0].vertices.chunks_exact(8) {
        assert_eq!(&vertex[6..8], &[0.12345, -129.75]);
    }
    assert_eq!(scene.collision_meshes.len(), 1);
    assert_eq!(scene.collision_meshes[0].indices.len(), 3);
}
