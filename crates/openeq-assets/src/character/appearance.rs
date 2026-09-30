//! Classic WLD appearance selection and rigid equipment attachments.
use super::*;

/// EQ's visible slots: head, chest, arms, wrist, hands, legs, feet, primary,
/// secondary. Colors are 0xAARRGGBB; a zero high byte disables tint.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct EquipmentAppearance {
    pub material: u32,
    pub elite_material: u32,
    pub hero_forge_model: u32,
    pub color: u32,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CharacterAppearance {
    pub texture: u8,
    pub helm_texture: u8,
    pub face: u8,
    pub hair_color: u8,
    pub beard_color: u8,
    pub eye_color_1: u8,
    pub eye_color_2: u8,
    pub hair_style: u8,
    pub beard: u8,
    pub drakkin_heritage: u32,
    pub drakkin_tattoo: u32,
    pub drakkin_details: u32,
    pub equipment: [EquipmentAppearance; 9],
}

impl CharacterAppearance {
    fn armor(&self, slot: usize) -> u32 {
        let material = self.equipment[slot].material;
        let fallback = if slot == 0 {
            self.helm_texture
        } else {
            self.texture
        };
        if material != 0 {
            material
        } else if fallback == 255 {
            0
        } else {
            u32::from(fallback)
        }
    }
}

impl CharacterLibrary {
    /// Canonicalize only fields that the selected model family ignores.
    /// Modern faces and armor variants must not pass through classic limits.
    pub fn normalize_race_appearance(
        &self,
        race: u32,
        gender: u8,
        appearance: CharacterAppearance,
    ) -> CharacterAppearance {
        if let Some(code) = race_model_code(race, gender) {
            if !self.actors.contains_key(code) {
                return self.normalize_modern_appearance(code, appearance);
            }
            if self.luclin_codes.contains(code) {
                return self.normalize_luclin_appearance(code, appearance);
            }
        }
        self.normalize_appearance(appearance)
    }

    /// Drop protocol fields that have no visual effect in the current asset
    /// path before constructing an appearance cache key.
    pub fn normalize_appearance(&self, appearance: CharacterAppearance) -> CharacterAppearance {
        let mut value = appearance;
        value.hair_color = 0;
        value.beard_color = 0;
        value.eye_color_1 = 0;
        value.eye_color_2 = 0;
        value.hair_style = 0;
        value.beard = 0;
        value.drakkin_heritage = 0;
        value.drakkin_tattoo = 0;
        value.drakkin_details = 0;
        if value.texture == 255 {
            value.texture = 0;
        }
        if value.helm_texture == 255 || value.helm_texture > 3 {
            value.helm_texture = 0;
        }
        if value.face > 9 {
            value.face = 0;
        }
        for (index, slot) in value.equipment.iter_mut().enumerate() {
            slot.elite_material = 0;
            slot.hero_forge_model = 0;
            if slot.color >> 24 == 0 || slot.color & 0x00ff_ffff == 0x00ff_ffff {
                slot.color = 0;
            }
            if (index == 0 && slot.material > 3)
                || (index < 7 && slot.material > 99)
                || (index >= 7 && !self.has_equipment(slot.material))
            {
                slot.material = 0;
            }
            if index >= 7 && slot.material == 0 {
                slot.color = 0;
            }
        }
        value
    }

    pub(super) fn rebuild_appearance_meshes(
        &self,
        model: &mut CharacterModel,
        appearance: &CharacterAppearance,
    ) -> Result<()> {
        let &(wld_index, actor_index) = &self.actors[&model.code];
        let wld = &self.wlds[wld_index];
        let Fragment::ActorDef(actor) = &wld.chunks()[actor_index].fragment else {
            unreachable!()
        };
        let skeleton = actor
            .references
            .iter()
            .find_map(
                |reference| match wld.resolve(*reference).map(|c| &c.fragment) {
                    Some(Fragment::Skeleton(skeleton)) => Some(skeleton),
                    Some(Fragment::SkeletonRef(reference)) => {
                        match wld.resolve(reference.skeleton).map(|c| &c.fragment) {
                            Some(Fragment::Skeleton(skeleton)) => Some(skeleton),
                            _ => None,
                        }
                    }
                    _ => None,
                },
            )
            .ok_or_else(|| Error::Format(format!("{} has no WLD skeleton", model.code)))?;
        model.materials.clear();
        model.meshes.clear();
        model.bindings.clear();
        model.material_slots.clear();
        model.appearance_resolved = true;
        let mut seen = BTreeSet::new();
        for reference in &skeleton.meshes {
            if reference.0 == 0 {
                continue;
            }
            let Some((reference, mesh)) = resolve_mesh(wld, *reference) else {
                model.appearance_resolved = false;
                continue;
            };
            if !seen.insert(reference.0) {
                continue;
            }
            let (mesh, resolved) =
                self.appearance_mesh(wld, reference, mesh, &model.code, appearance);
            model.appearance_resolved &= resolved;
            let is_head = wld
                .resolve(reference)
                .is_some_and(|chunk| chunk.name.starts_with(&format!("{}HE", model.code)));
            append_mesh(
                wld,
                mesh,
                skeleton.tracks.len(),
                is_head.then_some(0),
                model,
            )?;
        }
        self.apply_appearance(model, appearance);
        Ok(())
    }

    fn has_texture(&self, name: &str) -> bool {
        let name = name.to_ascii_lowercase();
        self.textures.contains_key(&name) || self.loose_textures.contains_key(&name)
    }

    pub(super) fn appearance_mesh<'a>(
        &self,
        wld: &'a Wld,
        reference: Ref,
        mesh: &'a Mesh,
        code: &str,
        appearance: &CharacterAppearance,
    ) -> (&'a Mesh, bool) {
        if self.luclin_codes.contains(code) {
            return (mesh, true);
        }
        let Some(name) = wld.resolve(reference).map(|chunk| chunk.name.as_str()) else {
            return (mesh, false);
        };
        let helm = appearance.armor(0);
        let chest = appearance.armor(1);
        let replacement = if name == format!("{code}HE00_DMSPRITEDEF") && helm > 0 && helm < 4 {
            Some(format!("{code}HE{helm:02}_DMSPRITEDEF"))
        } else if name == format!("{code}_DMSPRITEDEF") && (10..=16).contains(&chest) {
            Some(format!("{code}01_DMSPRITEDEF"))
        } else {
            None
        };
        let Some(replacement) = replacement else {
            return (mesh, true);
        };
        wld.chunks()
            .iter()
            .find_map(|chunk| {
                if chunk.name == replacement
                    && let Fragment::Mesh(mesh) = &chunk.fragment
                {
                    Some((mesh, true))
                } else {
                    None
                }
            })
            .unwrap_or((mesh, false))
    }

    pub(super) fn apply_appearance(
        &self,
        model: &mut CharacterModel,
        appearance: &CharacterAppearance,
    ) {
        if !self.apply_luclin_appearance(model, appearance) {
            let appearance_wld = &self.wlds[self.actors[&model.code].0];
            // Face zero means the actor's authored default. Some classic
            // actors (HUF/DWF) start at face two and have no face-zero palette.
            let mut face_resolved = appearance.face == 0;
            for (index, material) in model.materials.iter_mut().enumerate() {
                for name in &mut material.textures {
                    let original = name.to_ascii_lowercase();
                    let Some((stem, extension)) = original.rsplit_once('.') else {
                        continue;
                    };
                    if !stem.is_ascii() {
                        continue;
                    }
                    let mut slot = model.material_slots[index];
                    let mut skin = None;
                    let mut replacement = original.clone();
                    if stem.len() == 9 && stem[5..].bytes().all(|c| c.is_ascii_digit()) {
                        slot = match &stem[3..5] {
                            "he" => Some(0),
                            "ch" => Some(1),
                            "ua" => Some(2),
                            "fa" => Some(3),
                            "hn" => Some(4),
                            "lg" => Some(5),
                            "ft" => Some(6),
                            _ => slot,
                        };
                        skin = stem[5..7].parse::<u32>().ok();
                        let armor = slot
                            .map(|i| appearance.armor(i))
                            .unwrap_or(u32::from(appearance.texture));
                        if slot == Some(0) {
                            // Faces occupy the tens digit of the final pair and
                            // may remain visible through an open helm.
                            if appearance.face < 10 {
                                let candidate = format!(
                                    "{}{}{}.{}",
                                    &stem[..7],
                                    appearance.face,
                                    &stem[8..],
                                    extension
                                );
                                if self.has_texture(&candidate) {
                                    replacement = candidate;
                                    face_resolved = true;
                                } else if appearance.face != 0 && {
                                    let stem = candidate.rsplit_once('.').unwrap().0;
                                    appearance_wld
                                        .by_name(&format!("{}_MDF", stem.to_ascii_uppercase()))
                                        .is_some_and(|chunk| {
                                            matches!(chunk.fragment, Fragment::Material(_))
                                        })
                                } {
                                    // A referenced palette entry must load, but
                                    // unauthored variants reuse the base piece
                                    // (teeth, helmet plates, Iksar head details).
                                    model.appearance_resolved = false;
                                }
                            }
                        } else if armor > 0 && armor < 100 {
                            let texture = if (10..=16).contains(&armor) {
                                armor - 6
                            } else {
                                armor
                            };
                            let candidate =
                                format!("{}{:02}{}.{}", &stem[..5], texture, &stem[7..], extension);
                            if self.has_texture(&candidate) {
                                replacement = candidate;
                            } else {
                                model.appearance_resolved = false;
                            }
                        }
                    } else if stem.starts_with("clk") && stem.len() == 7 {
                        // Classic robes use shared CLK0401..CLK1004 textures;
                        // visible material ids 10..16 select those seven skins.
                        slot = Some(1);
                        let armor = appearance.armor(1);
                        if (10..=16).contains(&armor) {
                            let candidate =
                                format!("clk{:02}{}.{}", armor - 6, &stem[5..], extension);
                            if self.has_texture(&candidate) {
                                replacement = candidate;
                            } else {
                                model.appearance_resolved = false;
                            }
                        }
                    }
                    if let Some(slot) = slot {
                        // Non-face head materials (helm/chain/leather) tint too.
                        // HE00 is exposed skin, regardless of the helmet id.
                        let tintable = if slot == 0 {
                            skin != Some(0)
                        } else {
                            appearance.armor(slot) > 0
                        };
                        let color = appearance.equipment[slot].color;
                        if tintable && color >> 24 != 0 {
                            replacement = format!("{replacement}#tint={color:08x}");
                        }
                    }
                    *name = replacement;
                }
            }
            model.appearance_resolved &= face_resolved;
        }
        for slot in [7, 8] {
            let equipment = appearance.equipment[slot];
            if equipment.material == 0 {
                continue;
            }
            if let Err(error) =
                self.attach_equipment(model, equipment.material, slot == 8, equipment.color)
            {
                model.appearance_resolved = false;
                tracing::debug!(model = %model.code, item = equipment.material, %error, "equipment unavailable");
            }
        }
        for material in &mut model.materials {
            if material.alpha_mask {
                for name in &mut material.textures {
                    // Mask before tint so the key matches the source palette.
                    *name = if let Some((source, tint)) = name.split_once("#tint=") {
                        format!("{source}#masked#tint={tint}")
                    } else {
                        format!("{name}#masked")
                    };
                }
            }
        }
        // Update equipment into the same bind pose as the body without
        // changing body bounds (weapon length must not shrink the character).
        let mut meshes = model.meshes.clone();
        model.sample_into("", 0., &mut meshes);
        model.meshes = meshes;
    }

    /// Loads a classic IT model and binds its local coordinates directly to
    /// the authored R_POINT/L_POINT hand transform. No world-space offset is
    /// applied: the mesh origin is the grip point in the original assets.
    fn attach_equipment(
        &self,
        model: &mut CharacterModel,
        material: u32,
        secondary: bool,
        color: u32,
    ) -> Result<()> {
        let (mut materials, mut meshes) = self.equipment_geometry(material)?;
        // The spawn packet has no item type. Explicit shield surface names
        // identify authored shields; generic offhand bags/orbs stay in hand.
        let shield = secondary
            && materials
                .iter()
                .flat_map(|m| &m.textures)
                .any(|name| name.to_ascii_lowercase().contains("shield"));
        let suffix = if shield
            && model
                .bone_names
                .iter()
                .any(|name| name.ends_with("SHIELD_POINT_TRACK"))
        {
            "SHIELD_POINT_TRACK"
        } else if secondary {
            "L_POINT_TRACK"
        } else {
            "R_POINT_TRACK"
        };
        let bone = model
            .bone_names
            .iter()
            .position(|name| name == &format!("{}{suffix}", model.code))
            // A borrowed skeleton can retain its donor's three-letter code.
            // Suffix-only matching also matches HAIR_POINT as R_POINT!
            .or_else(|| {
                model.bone_names.iter().position(|name| {
                    name.strip_suffix(suffix)
                        .is_some_and(|prefix| prefix.len() == 3)
                })
            })
            .ok_or_else(|| Error::NotFound(format!("{} attachment {suffix}", model.code)))?;
        let offset = model.materials.len();
        if color >> 24 != 0 {
            for material in &mut materials {
                for name in &mut material.textures {
                    *name = format!("{name}#tint={color:08x}");
                }
            }
        }
        for mesh in &mut meshes {
            mesh.material += offset;
            mesh.collidable = false;
            model.bindings.push(
                mesh.vertices
                    .chunks_exact(8)
                    .map(|v| BoundVertex {
                        position: Vec3::new(v[0], v[1], v[2]),
                        normal: Vec3::new(v[3], v[4], v[5]),
                        bone,
                    })
                    .collect(),
            );
        }
        model
            .material_slots
            .extend(std::iter::repeat_n(None, materials.len()));
        model.materials.extend(materials);
        model.meshes.extend(meshes);
        Ok(())
    }

    pub(super) fn has_equipment(&self, material: u32) -> bool {
        self.equipment.contains_key(&format!("IT{material}"))
            || self.eqg_files.contains_key(&format!("it{material}"))
    }

    pub(super) fn equipment_geometry(
        &self,
        material: u32,
    ) -> Result<(Vec<Material>, Vec<Geometry>)> {
        let code = format!("IT{material}");
        if !self.equipment.contains_key(&code) {
            return self.eqg_equipment_geometry(&code);
        }
        let &(wld_index, actor_index) = self
            .equipment
            .get(&code)
            .ok_or_else(|| Error::NotFound(format!("equipment {code}")))?;
        let wld = &self.wlds[wld_index];
        let Fragment::ActorDef(actor) = &wld.chunks()[actor_index].fragment else {
            unreachable!()
        };
        let sources: Vec<_> = actor
            .references
            .iter()
            .filter_map(|reference| resolve_mesh(wld, *reference).map(|(_, mesh)| mesh))
            .collect();
        if sources.is_empty() {
            return Err(Error::Format(format!(
                "{code}: animated equipment not supported"
            )));
        }
        Ok(mesh::bake_wld_meshes(wld, sources))
    }
}
