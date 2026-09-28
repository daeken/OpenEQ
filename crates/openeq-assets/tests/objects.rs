//! Original-client dynamic object definitions (server-placed doors and props).
#[test]
fn poknowledge_doors_and_supplemental_eqg_models_are_available() {
    let Some(base) = openeq_assets::loader::default_client_dir() else {
        return;
    };
    if !base.join("poknowledge_obj.s3d").is_file() {
        return;
    }
    let library = openeq_assets::loader::load_object_library(&base, "poknowledge").unwrap();
    eprintln!("{} definitions", library.objects.len());
    for name in [
        "POKDOOR500",
        "POKDOOR501",
        "POKDOOR502",
        "POKDOOR503",
        "POKELEVATOR500",
    ] {
        let model = library.object_model(name).unwrap();
        assert!(model.triangle_count() > 0);
        assert!(model.instances.is_empty());
        assert!(model.objects.is_empty());
        let mut min = [f32::INFINITY; 3];
        let mut max = [f32::NEG_INFINITY; 3];
        for mesh in &model.meshes {
            for v in mesh.vertices.chunks_exact(8) {
                for a in 0..3 {
                    min[a] = min[a].min(v[a]);
                    max[a] = max[a].max(v[a]);
                }
            }
        }
        eprintln!(
            "{name}: {} triangles, {:?}..{:?}",
            model.triangle_count(),
            min,
            max
        );
        for material in &model.materials {
            for name in &material.textures {
                assert!(model.texture(name).is_some(), "{name}");
            }
        }
    }
    let modern: Vec<_> = library
        .objects
        .iter()
        .filter(|o| o.name.contains("book") || o.name.contains("gukta"))
        .map(|o| o.name.as_str())
        .collect();
    eprintln!("supplemental props: {modern:?}");
    assert!(library.object_model("no_such_object").is_err());
}
