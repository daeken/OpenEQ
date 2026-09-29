use openeq_assets::{loader, mesh::VERTEX_STRIDE, pfs::Archive};

#[test]
#[ignore = "requires original Feerrott2 assets"]
fn feerrott2_loads_its_authored_internal_terrain_without_renaming_assets() {
    let base = loader::default_client_dir().expect("original client directory is required");
    let archive = Archive::open(base.join("feerrott2.eqg")).unwrap();
    assert!(!archive.contains("feerrott2.zon"));
    assert!(archive.contains("feerrott.zon") && archive.contains("feerrott.dat"));
    let scene = loader::load_zone(&base, "feerrott2").unwrap();
    assert_eq!(scene.name, "feerrott2");
    assert!(scene.instances.len() >= 10_857);
    assert!(scene.triangle_count() > 100_000);
    assert!(
        scene
            .meshes
            .iter()
            .all(|mesh| mesh.vertices.iter().all(|value| value.is_finite()))
    );
    let terrain = scene
        .materials
        .iter()
        .filter_map(|material| material.textures.first())
        .filter(|name| name.starts_with("__terrain_"))
        .collect::<Vec<_>>();
    assert_eq!(terrain.len(), 329);
    for name in terrain.iter().step_by(17) {
        let texture = scene.texture(name).unwrap();
        let magenta = texture
            .rgba
            .chunks_exact(4)
            .filter(|p| p[0] > 240 && p[1] < 15 && p[2] > 240)
            .count();
        assert!(
            magenta < texture.width as usize * texture.height as usize / 100,
            "unresolved {name}"
        );
    }
    let positions = scene
        .meshes
        .iter()
        .flat_map(|mesh| mesh.vertices.chunks_exact(VERTEX_STRIDE));
    assert!(positions.count() > 10_000);
    // Indexed tile water is retained as metadata in a separate slice. It must
    // not become invented finite-sheet geometry or swimming volumes here.
    assert!(
        !scene
            .materials
            .iter()
            .any(|material| material.water.is_some())
    );
}
