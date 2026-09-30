//! Drakkin EQGM modules share the body's bind space, but each module numbers
//! its own skeleton. A module's ROOT_BONE is often a renamed subtree root.
use super::*;

pub(super) fn is_drakkin(code: &str) -> bool {
    code.eq_ignore_ascii_case("DKM") || code.eq_ignore_ascii_case("DKF")
}

pub(super) fn armor(appearance: &CharacterAppearance, slot: usize) -> u32 {
    let item = appearance.equipment[slot].material;
    if item != 0 {
        item
    } else if slot == 0 {
        u32::from(appearance.helm_texture)
    } else {
        u32::from(appearance.texture)
    }
}

pub(super) fn load_modules(model: &mut ModernModel, archive: &Archive, code: &str) -> Result<()> {
    let prefix = format!("{}_", code.to_ascii_lowercase());
    let mut names: Vec<_> = archive
        .names()
        .iter()
        .filter(|name| {
            let name = name.to_ascii_lowercase();
            name.starts_with(&prefix) && name.ends_with(".mod")
        })
        .collect();
    names.sort();
    for name in names {
        let stem = name.strip_suffix(".mod").unwrap_or(name);
        let part = ModernModel::parse(&archive.read(name)?, stem)?;
        append_module(model, part)?;
    }
    Ok(())
}

fn append_module(model: &mut ModernModel, mut part: ModernModel) -> Result<()> {
    let piece_name = &part.pieces[0].name;
    let mut mapping = Vec::with_capacity(part.bones.len());
    for (index, bone) in part.bones.iter().enumerate() {
        let mapped = if bone.name.eq_ignore_ascii_case("ROOT_BONE") {
            // Infer the original subtree root from its children's parents in
            // the full skeleton. This distinguishes ROOT_BODY (clothes),
            // HEAD_HEAD (horns), and the real ROOT_BONE (tattoos).
            let parents: BTreeSet<_> = part
                .parents
                .iter()
                .enumerate()
                .filter(|(_, parent)| **parent == Some(index))
                .filter_map(|(child, _)| {
                    model
                        .bones
                        .iter()
                        .position(|b| b.name.eq_ignore_ascii_case(&part.bones[child].name))
                })
                .filter_map(|child| model.parents[child])
                .collect();
            if parents.len() == 1 {
                parents.first().copied()
            } else {
                // Leaf modules have no children from which to infer a parent.
                // Their authored filename ends in the replaced bone's name.
                model.bones.iter().position(|b| {
                    piece_name
                        .to_ascii_lowercase()
                        .ends_with(&format!("_{}", b.name.to_ascii_lowercase()))
                })
            }
        } else {
            model
                .bones
                .iter()
                .position(|b| b.name.eq_ignore_ascii_case(&bone.name))
        };
        mapping.push(mapped);
    }
    for piece in &mut part.pieces {
        for vertex in &mut piece.vertices {
            for weight in &mut vertex.weights {
                if weight.weight == 0. {
                    continue;
                }
                let original = weight.bone;
                let mapped = mapping[original].ok_or_else(|| {
                    Error::Format(format!(
                        "{}: unmatched modular bone {}",
                        piece.name, part.bones[original].name
                    ))
                })?;
                // Remapped indices are only valid if both skeletons describe
                // the same bind space. Reject incompatible modules instead of
                // silently stretching them around an unrelated body bone.
                if !part.inverse_bind[original].abs_diff_eq(model.inverse_bind[mapped], 0.02) {
                    return Err(Error::Format(format!(
                        "{}: incompatible bind transform {} -> {}",
                        piece.name, part.bones[original].name, model.bones[mapped].name
                    )));
                }
                weight.bone = mapped;
            }
            vertex.weights.retain(|weight| weight.weight > 0.);
        }
        for (_, material) in &mut piece.triangles {
            if *material >= 0 {
                *material += model.materials.len() as i32;
            }
        }
    }
    model.materials.extend(part.materials);
    model.material_names.extend(part.material_names);
    model.pieces.extend(part.pieces);
    Ok(())
}

pub(super) fn select(
    model: &ModernModel,
    code: &str,
    appearance: &CharacterAppearance,
) -> (Vec<(usize, Option<usize>)>, bool) {
    let mut selected = Vec::new();
    let mut resolved = true;
    let mut add = |name: String, slot, required| {
        if let Some(index) = model
            .pieces
            .iter()
            .position(|p| p.name.eq_ignore_ascii_case(&name))
        {
            selected.push((index, slot));
        } else if required {
            resolved = false;
        }
    };
    add(code.to_owned(), None, true);
    for (slot, suffix) in [
        (1, "root_body"),
        (1, "chest_chest01"),
        (0, "chest_chest02"),
        (2, "arml_clav"),
        (2, "armr_clav"),
        (4, "arml_bcep"),
        (4, "armr_bcep"),
        (5, "pelv"),
        (6, "legl_thgh"),
        (6, "legr_thgh"),
    ] {
        let material = armor(appearance, slot);
        let set = if (10..=16).contains(&material) {
            10
        } else {
            material
        };
        // The authored cloth set has only these three modules. Other suffixes
        // are intentionally absent, not evidence of a missing client archive.
        let required = set != 0 || matches!(suffix, "root_body" | "legl_thgh" | "legr_thgh");
        add(format!("{code}_{set:02}_00_{suffix}"), Some(slot), required);
    }
    // The male archive intentionally has no hair_08 (the bald option).
    // Helms cover hair; eyebrows and authored facial details remain independent.
    if armor(appearance, 0) == 0 {
        add(
            format!("{code}_hair_{:02}", appearance.hair_style),
            None,
            !(code.eq_ignore_ascii_case("DKM") && appearance.hair_style == 8),
        );
    }
    add(
        format!("{code}_facialhair_{:02}", appearance.beard),
        None,
        true,
    );
    add(
        format!("{code}_facialatt_{:02}", appearance.drakkin_details),
        None,
        true,
    );
    add(format!("{code}_tattoo_00"), None, true);
    (selected, resolved)
}

fn substitute_skin(name: &mut String, skin: u32, archive: &Archive) -> bool {
    let lower = name.to_ascii_lowercase();
    if let Some(index) = lower.find("_s")
        && lower
            .as_bytes()
            .get(index + 2..index + 4)
            .is_some_and(|digits| digits.iter().all(u8::is_ascii_digit))
    {
        let candidate = format!("{}{:02}{}", &lower[..index + 2], skin, &lower[index + 4..]);
        if archive.contains(&candidate) {
            *name = candidate;
            return true;
        }
    }
    false
}

pub(super) fn apply_materials(
    model: &ModernModel,
    materials: &mut [Material],
    appearance: &CharacterAppearance,
    archive: &Archive,
    customization: Option<&customization::CustomizationEntry>,
) -> bool {
    let mut resolved = true;
    for (index, material) in materials.iter_mut().enumerate() {
        let name = model.material_names[model.baked_materials[index]].to_ascii_lowercase();
        let slot = model.baked_slots[index];
        let skin = match name.as_str() {
            "head" | "dkf_head" => Some(u32::from(appearance.face)),
            // Both authored eye materials reference the RIGHTEYE texture set.
            "right_eye" | "dkf_reye" => Some(u32::from(appearance.eye_color_1)),
            "left_eye" | "dkf_leye" => Some(u32::from(appearance.eye_color_2)),
            "tats" | "tattoos" => Some(appearance.drakkin_tattoo),
            _ => slot
                .map(|slot| armor(appearance, slot))
                .filter(|material| (10..=16).contains(material))
                .map(|material| material - 10),
        };
        if let Some(skin) = skin {
            for texture in &mut material.textures {
                resolved &= substitute_skin(texture, skin, archive);
            }
            if let Some(normal) = &mut material.normal_map {
                substitute_skin(normal, skin, archive);
            }
        }
        let piece = &model.pieces[model.baked_pieces[index]].name;
        let color = appearance_tint(piece, slot, appearance, customization);
        if let Some(color) = color {
            for name in &mut material.textures {
                *name = format!("{name}#tint={color:08x}");
            }
        }
    }
    resolved
}

fn appearance_tint(
    piece: &str,
    slot: Option<usize>,
    appearance: &CharacterAppearance,
    customization: Option<&customization::CustomizationEntry>,
) -> Option<u32> {
    if let Some(slot) = slot {
        let color = appearance.equipment[slot].color;
        return (color >> 24 != 0).then_some(color);
    }
    let entry = customization?;
    let piece = piece.to_ascii_lowercase();
    // Facial attachment MODs deliberately reuse HAIR_06/HAIR_04 material
    // labels and textures. Their source module, not its material label,
    // determines whether heritage or a selectable hair shade controls tint.
    let color = if piece.contains("_facialatt_") || piece.contains("_tattoo_") {
        entry.base_color
    } else if piece.contains("_facialhair_") {
        *entry.colors.get(usize::from(appearance.beard_color))?
    } else if piece.contains("_hair_") {
        *entry.colors.get(usize::from(appearance.hair_color))?
    } else {
        return None;
    };
    Some(color | 0xff00_0000)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "requires original Drakkin modules; CPU only"]
    fn missing_required_module_is_unresolved_but_authored_bald_choice_is_valid() {
        let base = crate::loader::default_client_dir().expect("original assets");
        let archive = Archive::open(base.join("dkm.eqg")).unwrap();
        let mut model = ModernModel::parse(&archive.read("dkm.mod").unwrap(), "DKM").unwrap();
        load_modules(&mut model, &archive, "DKM").unwrap();
        let appearance = CharacterAppearance {
            hair_style: 1,
            ..Default::default()
        };
        assert!(select(&model, "DKM", &appearance).1);
        model
            .pieces
            .retain(|piece| !piece.name.eq_ignore_ascii_case("dkm_hair_01"));
        let (selected, resolved) = select(&model, "DKM", &appearance);
        assert!(!resolved);
        assert!(!selected.is_empty(), "ordinary rendering keeps the body");
        assert!(select(&model, "DKM", &CharacterAppearance::default()).1);
        assert!(
            select(
                &model,
                "DKM",
                &CharacterAppearance {
                    hair_style: 8,
                    ..Default::default()
                }
            )
            .1,
            "male hair08 is an intentionally absent bald module"
        );
    }

    #[test]
    fn missing_palette_preserves_geometry_choices_but_drops_unsupported_dyes() {
        let Some(base) = crate::loader::default_client_dir() else {
            return;
        };
        if !base.join("dkm.eqg").is_file() {
            return;
        }
        let mut library = CharacterLibrary::load(base, "poknowledge").unwrap();
        library.customization = customization::CustomizationCatalog::default();
        let appearance = CharacterAppearance {
            drakkin_heritage: 4,
            hair_color: 3,
            beard_color: 2,
            hair_style: 6,
            beard: 3,
            drakkin_tattoo: 7,
            drakkin_details: 7,
            ..Default::default()
        };
        let normalized = library.normalize_modern_appearance("DKM", appearance);
        assert_eq!(
            (
                normalized.drakkin_heritage,
                normalized.hair_color,
                normalized.beard_color
            ),
            (0, 0, 0)
        );
        assert_eq!(
            (
                normalized.hair_style,
                normalized.beard,
                normalized.drakkin_tattoo,
                normalized.drakkin_details
            ),
            (6, 3, 7, 7)
        );
        let model = library
            .load_model_with_appearance("DKM", &appearance)
            .unwrap();
        assert!(
            model
                .materials
                .iter()
                .flat_map(|m| &m.textures)
                .all(|name| !name.contains("#tint="))
        );
        assert!(appearance_tint("dkm_facialatt_03", None, &appearance, None).is_none());
    }
}
