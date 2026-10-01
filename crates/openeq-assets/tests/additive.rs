//! Region-only additive classification through the real EQG archive loader.
#[path = "support/eqg_collision.rs"]
mod support;
use support::*;

#[test]
fn only_the_proven_region_shader_is_additive_even_when_textures_are_shared() {
    let shaders = [
        "Opaque_MaxCB1.fx",
        "AddAlpha_MaxCB1.fx",
        "addalpha_maxcb1.FX",
        "AddAlpha_MaxCB1_2UV.fx",
        "AddAlpha_MaxC1.fx",
        "AddAlpha_MPLBasicA.fx",
        "AddAlpha_MaxCB1.fx.extra",
        "Alpha_MaxCB1.fx",
    ];
    for is_terrain in [true, false] {
        let mut source = model(is_terrain);
        for (index, shader) in shaders.iter().enumerate() {
            source.materials.push(material(shader, Some("shared.dds")));
            quad(&mut source, floor(index as f32 * 2.), index as u32, 0);
        }
        let name = if is_terrain {
            "ground.ter"
        } else {
            "ground.mod"
        };
        let fixture = Fixture::new(&[(name, &source)], &[]);
        let scene = fixture.scene();
        assert_eq!(scene.materials.len(), shaders.len());
        assert_eq!(scene.meshes.len(), shaders.len());
        for (index, surface) in scene.materials.iter().enumerate() {
            let additive = is_terrain && matches!(index, 1 | 2);
            assert_eq!(
                surface.additive, additive,
                "region={is_terrain} {}",
                shaders[index]
            );
            assert!(!surface.transparent);
            assert!(!surface.emissive);
            assert!(surface.water.is_none());
            assert_eq!(surface.textures, ["shared.dds"]);
            assert_eq!(scene.meshes[index].material, index);
        }
        // Material identity still comes from the first exact source name.
        source.materials[1].name = source.materials[0].name.clone();
        let overridden = Fixture::new(&[(name, &source)], &[]).scene();
        assert!(!overridden.materials[1].additive);
    }
}
