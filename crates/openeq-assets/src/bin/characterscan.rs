//! Inspect classic character bones, textures and equipment geometry.
use openeq_assets::{
    character::CharacterLibrary,
    loader,
    pfs::Archive,
    wld::{Fragment, Wld},
};
fn main() -> anyhow::Result<()> {
    let base =
        loader::default_client_dir().ok_or_else(|| anyhow::anyhow!("no client directory"))?;
    let code = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "HUM".into())
        .to_ascii_uppercase();
    if code.starts_with("IT") {
        for entry in std::fs::read_dir(&base)? {
            let path = entry?.path();
            if !path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .starts_with("gequip")
            {
                continue;
            }
            let archive = Archive::open(&path)?;
            for filename in archive.names().iter().filter(|name| name.ends_with(".wld")) {
                let wld = Wld::open(&archive, filename)?;
                for chunk in wld.chunks() {
                    if (chunk.name == format!("{code}_DMSPRITEDEF") || code == "IT")
                        && let Fragment::Mesh(mesh) = &chunk.fragment
                    {
                        let (materials, _) = openeq_assets::mesh::bake_wld_meshes(&wld, [mesh]);
                        if code == "IT"
                            && !materials
                                .iter()
                                .flat_map(|m| &m.textures)
                                .any(|n| n.to_ascii_lowercase().contains("shield"))
                        {
                            continue;
                        }
                        println!(
                            "{} in {} center {:?}, bounds {:?}..{:?}",
                            chunk.name,
                            path.display(),
                            mesh.center,
                            mesh.bounds_min,
                            mesh.bounds_max
                        );
                        println!(
                            "textures: {:?}",
                            materials
                                .iter()
                                .flat_map(|m| &m.textures)
                                .collect::<Vec<_>>()
                        );
                    }
                }
            }
        }
    } else {
        let library = CharacterLibrary::load(base, "poknowledge")?;
        let model = library.load_model(&code)?;
        println!(
            "{code}: {} meshes, bounds {:?}..{:?}",
            model.meshes.len(),
            model.bounds_min,
            model.bounds_max
        );
        println!("bones: {:?}", model.bone_names);
        println!(
            "textures: {:?}",
            model
                .materials
                .iter()
                .flat_map(|m| &m.textures)
                .collect::<Vec<_>>()
        );
        for (name, clip) in model.animations.iter() {
            println!(
                "{name}: {} frames @ {} ms",
                clip.frame_count, clip.frame_time_ms
            );
        }
    }
    Ok(())
}
