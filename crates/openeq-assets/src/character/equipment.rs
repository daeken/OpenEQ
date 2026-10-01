//! Static EQG item meshes. Their origin is the authored hand grip, just like
//! classic IT models. Weighted/animated items need a separate item skeleton.
use super::*;

impl CharacterLibrary {
    /// Loads an original rigid item model for an independent projectile or
    /// other dynamic object. Vertices retain the authored item origin; there
    /// is no character centering, hand transform, or size normalization.
    /// Texture animation remains in the materials. Skeletal/weighted items
    /// require their own animation path and are rejected by the item reader.
    pub fn load_equipment_scene(&self, code: &str) -> Result<crate::Scene> {
        let material_id = equipment_id(code)?;
        let code = format!("IT{material_id}");
        let (mut materials, mut meshes) = self.equipment_geometry(material_id)?;
        let mut textures = BTreeMap::new();
        for material in &mut materials {
            for name in &mut material.textures {
                if material.alpha_mask && !name.ends_with("#masked") {
                    name.push_str("#masked");
                }
                if let std::collections::btree_map::Entry::Vacant(entry) =
                    textures.entry(name.to_ascii_lowercase())
                {
                    let texture = self.texture(name).ok_or_else(|| {
                        Error::NotFound(format!("equipment {code} texture {name}"))
                    })?;
                    entry.insert(texture);
                }
            }
            if let Some(name) = &material.normal_map
                && !textures.contains_key(&name.to_ascii_lowercase())
                && let Some(texture) = self.texture(name)
            {
                textures.insert(name.to_ascii_lowercase(), texture);
            }
        }
        for mesh in &mut meshes {
            mesh.collidable = false;
        }
        if meshes.is_empty() {
            return Err(Error::Format(format!(
                "{code}: no visible equipment geometry"
            )));
        }
        Ok(crate::Scene::from_geometry(
            code,
            materials,
            meshes,
            textures.into_values().collect(),
        ))
    }

    pub(super) fn eqg_equipment_geometry(
        &self,
        code: &str,
    ) -> Result<(Vec<Material>, Vec<Geometry>)> {
        let (key, member) = self.equipment_eqg_source(code)?;
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
        let data = archive.read(&member)?;
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
                .material_for_polygon(id)
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
                transparent: shader.starts_with("addalpha"),
                additive: false,
                emissive: shader.starts_with("addalpha"),
                clamp_uv: false,
                waterfall: None,
                uv_encoding: Default::default(),
            });
        }
        if meshes.is_empty() {
            return Err(Error::Format(format!(
                "{code}: no visible equipment geometry"
            )));
        }
        Ok((materials, meshes))
    }

    fn equipment_eqg_source(&self, code: &str) -> Result<(String, String)> {
        use std::io::Read;
        let key = code.to_ascii_lowercase();
        if self.eqg_files.contains_key(&key) {
            return Ok((key.clone(), format!("{key}.mod")));
        }
        // EQGM item actors can belong to shared archives rather than
        // itNNN.eqg. All eqg_files entries were indexed from the client root.
        let base = self
            .eqg_files
            .values()
            .find_map(|path| path.parent())
            .ok_or_else(|| Error::NotFound(format!("equipment {code}")))?;
        let path = base.join("Resources/OnDemandResources.txt");
        let file = std::fs::File::open(&path).map_err(|source| Error::Io {
            path: path.clone(),
            source,
        })?;
        const LIMIT: u64 = 32 * 1024 * 1024;
        let mut text = String::new();
        file.take(LIMIT + 1)
            .read_to_string(&mut text)
            .map_err(|source| Error::Io {
                path: path.clone(),
                source,
            })?;
        if text.len() as u64 > LIMIT {
            return Err(Error::Format(
                "equipment resource mapping exceeds byte limit".into(),
            ));
        }
        let (archive, member) = on_demand_equipment_source(&text, code)
            .ok_or_else(|| Error::NotFound(format!("equipment {code}")))?;
        Ok((
            archive[..archive.len() - 4].to_ascii_lowercase(),
            member.to_ascii_lowercase(),
        ))
    }
}

fn on_demand_equipment_source<'a>(text: &'a str, code: &str) -> Option<(&'a str, &'a str)> {
    let actor_name = format!("{code}_ACTORDEF");
    text.lines().find_map(|line| {
        let mut fields = line.split('^');
        let archive = fields.next()?;
        let member = fields.next()?;
        let actor = fields.next()?;
        let kind = fields.next()?;
        if !actor.eq_ignore_ascii_case(&actor_name)
            || !kind.eq_ignore_ascii_case("EQGM")
            || !archive.to_ascii_lowercase().ends_with(".eqg")
            || !member.to_ascii_lowercase().ends_with(".mod")
            || [archive, member]
                .iter()
                .any(|name| name.contains(['/', '\\', ':']) || name.chars().any(char::is_control))
        {
            return None;
        }
        Some((archive, member))
    })
}

fn equipment_id(code: &str) -> Result<u32> {
    let invalid = || {
        Error::Format(format!(
            "invalid equipment code {code:?}; expected IT followed by a numeric ID"
        ))
    };
    if !code.is_ascii()
        || code.len() < 3
        || !code[..2].eq_ignore_ascii_case("IT")
        || !code[2..].bytes().all(|byte| byte.is_ascii_digit())
    {
        return Err(invalid());
    }
    code[2..]
        .parse::<u32>()
        .ok()
        .filter(|id| *id != 0)
        .ok_or_else(invalid)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn equipment_codes_are_case_insensitive_numeric_ids() {
        assert_eq!(equipment_id("IT11504").unwrap(), 11504);
        assert_eq!(equipment_id("it0001").unwrap(), 1);
        for code in [
            "",
            "IT",
            "IT0",
            "IT-1",
            "IT+1",
            "IT1.mod",
            "../IT1",
            "IT1/2",
            "IT4294967296",
            "ĪT1",
        ] {
            assert!(
                equipment_id(code).is_err(),
                "accepted invalid code {code:?}"
            );
        }
    }

    #[test]
    fn on_demand_mapping_resolves_exact_actor_and_rejects_paths() {
        let source = "missle.eqg^IT11504.MOD^IT11504_ACTORDEF^EQGM\r\n";
        assert_eq!(
            on_demand_equipment_source(source, "it11504"),
            Some(("missle.eqg", "IT11504.MOD"))
        );
        assert!(on_demand_equipment_source(source, "IT1150").is_none());
        assert!(
            on_demand_equipment_source(
                "../missle.eqg^IT11504.MOD^IT11504_ACTORDEF^EQGM",
                "IT11504"
            )
            .is_none()
        );
        assert!(
            on_demand_equipment_source(
                "missle.eqg^../IT11504.MOD^IT11504_ACTORDEF^EQGM",
                "IT11504"
            )
            .is_none()
        );
        assert!(
            on_demand_equipment_source("missle.eqg^IT11504.MOD^IT11504_ACTORDEF^EQGA", "IT11504")
                .is_none()
        );
    }

    #[test]
    #[ignore = "requires an installed EverQuest client (EQ_DIR)"]
    fn original_projectile_equipment_preserves_geometry_and_textures() {
        let base = std::env::var_os("EQ_DIR").expect("set EQ_DIR to the EverQuest directory");
        let library = CharacterLibrary::load(&base, "poknowledge").unwrap();
        for id in [1, 10, 8005].into_iter().chain(11503..=11519) {
            let scene = match library.load_equipment_scene(&format!("it{id}")) {
                Ok(scene) => scene,
                Err(error) => {
                    println!("IT{id}: {error}");
                    assert!(
                        id == 8005,
                        "required original projectile unavailable: {error}"
                    );
                    continue;
                }
            };
            let (_, original) = library.equipment_geometry(id).unwrap();
            assert_eq!(scene.meshes.len(), original.len());
            for (actual, expected) in scene.meshes.iter().zip(&original) {
                assert_eq!(
                    actual.vertices, expected.vertices,
                    "item origin moved for IT{id}"
                );
                assert_eq!(actual.indices, expected.indices);
                assert!(!actual.collidable);
            }
            for material in &scene.materials {
                for name in &material.textures {
                    let texture = scene.texture(name).unwrap();
                    assert!(texture.width > 1 && texture.height > 1);
                    assert!(texture.rgba.chunks_exact(4).any(|pixel| pixel[3] > 0));
                }
            }
            println!(
                "IT{id}: {} meshes, {} triangles, textures {:?}",
                scene.meshes.len(),
                scene
                    .meshes
                    .iter()
                    .map(|mesh| mesh.indices.len() / 3)
                    .sum::<usize>(),
                scene
                    .materials
                    .iter()
                    .flat_map(|material| material.textures.iter())
                    .collect::<Vec<_>>()
            );
            let mut min = [f32::INFINITY; 3];
            let mut max = [f32::NEG_INFINITY; 3];
            for vertex in scene
                .meshes
                .iter()
                .flat_map(|mesh| mesh.vertices.chunks_exact(8))
            {
                for axis in 0..3 {
                    min[axis] = min[axis].min(vertex[axis]);
                    max[axis] = max[axis].max(vertex[axis]);
                }
            }
            println!("IT{id} bounds {min:?}..{max:?}");
            if id == 10 {
                for mesh in &scene.meshes {
                    let z = mesh
                        .vertices
                        .chunks_exact(8)
                        .map(|vertex| vertex[2])
                        .fold((f32::INFINITY, f32::NEG_INFINITY), |(min, max), z| {
                            (min.min(z), max.max(z))
                        });
                    println!(
                        "arrowmaterial {:?} Z{z:?}",
                        scene.materials[mesh.material].textures
                    );
                }
            }
            if id == 11504 {
                let archives = library.modern_archives.lock().unwrap();
                let archive = archives.get("missle").unwrap();
                let model =
                    crate::zone::TerMod::parse(&archive.read("it11504.mod").unwrap(), false)
                        .unwrap();
                assert!(
                    model
                        .materials
                        .iter()
                        .all(|material| material.shader == "AddAlpha_MPLBasicA.fx")
                );
                assert!(
                    scene
                        .materials
                        .iter()
                        .all(|material| material.emissive && material.transparent)
                );
            }
        }
    }
}
