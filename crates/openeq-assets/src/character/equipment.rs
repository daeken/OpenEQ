//! Static EQG item meshes. Their origin is the authored hand grip, just like
//! classic IT models. Weighted/animated items need a separate item skeleton.
use super::*;

impl CharacterLibrary {
    pub(super) fn eqg_equipment_geometry(
        &self,
        code: &str,
    ) -> Result<(Vec<Material>, Vec<Geometry>)> {
        let key = code.to_ascii_lowercase();
        let cached = self.modern_archives.lock().unwrap().get(&key).cloned();
        let archive = if let Some(archive) = cached {
            archive
        } else {
            let path = self
                .eqg_files
                .get(&key)
                .ok_or_else(|| Error::NotFound(format!("equipment {code}")))?;
            let archive = Arc::new(Archive::open(path)?);
            self.modern_archives
                .lock()
                .unwrap()
                .insert(key.clone(), archive.clone());
            archive
        };
        let data = archive.read(&format!("{key}.mod"))?;
        if data.len() < 28 || &data[..4] != b"EQGM" {
            return Err(Error::Format(format!("{code}: invalid EQG item header")));
        }
        let version = u32::from_le_bytes(data[4..8].try_into().unwrap());
        let bones = u32::from_le_bytes(data[24..28].try_into().unwrap());
        if !(1..=3).contains(&version) || bones != 0 {
            return Err(Error::Format(format!(
                "{code}: unsupported animated item or version {version}"
            )));
        }
        let source = crate::zone::TerMod::parse(&data, false)?;
        if source
            .positions
            .iter()
            .chain(&source.normals)
            .flatten()
            .any(|v| !v.is_finite())
            || source.tex_coords.iter().flatten().any(|v| !v.is_finite())
        {
            return Err(Error::Format(format!(
                "{code}: nonfinite equipment vertices"
            )));
        }
        let mut materials = Vec::new();
        let mut meshes = Vec::new();
        let groups: BTreeMap<_, _> = source.mesh_groups().into_iter().collect();
        for (id, indices) in groups {
            // EQG editor-only polygons use material -1.
            if id == u32::MAX {
                continue;
            }
            let surface = source
                .materials
                .get(&id)
                .ok_or_else(|| Error::Format(format!("{code}: invalid item material {id}")))?;
            if indices
                .iter()
                .any(|i| *i as usize >= source.positions.len())
            {
                return Err(Error::Format(format!("{code}: invalid item vertex index")));
            }
            let diffuse = surface
                .properties
                .get("e_TextureDiffuse0")
                .and_then(crate::zone::Property::as_text)
                .ok_or_else(|| Error::Format(format!("{code}: item material has no diffuse")))?;
            let normal_map = surface
                .properties
                .get("e_TextureNormal0")
                .and_then(crate::zone::Property::as_text)
                .filter(|name| !name.eq_ignore_ascii_case("none"))
                .map(str::to_owned);
            let shader = surface.shader.to_ascii_lowercase();
            let (vertices, indices) = mesh::pack(
                &source.positions,
                &source.normals,
                &source.tex_coords,
                &indices,
            );
            meshes.push(Geometry {
                vertices,
                indices,
                material: materials.len(),
                collidable: false,
            });
            materials.push(Material {
                textures: vec![diffuse.to_owned()],
                normal_map,
                water: None,
                flags: 0,
                anim_speed: 0,
                alpha_mask: shader.starts_with("alpha") || shader.starts_with("chroma"),
                transparent: false,
                emissive: false,
            });
        }
        if meshes.is_empty() {
            return Err(Error::Format(format!(
                "{code}: no visible equipment geometry"
            )));
        }
        Ok((materials, meshes))
    }
}
