//! Luclin WLD conventions: A/B animation takes, layered body textures, and
//! modular appearance parts. These rules do not change classic WLD materials.
use super::*;

pub(super) fn install_animation_aliases(model: &mut CharacterModel) {
    let names: Vec<_> = model.animations.keys().cloned().collect();
    for suffix in ['A', 'B'] {
        for name in &names {
            if name.len() == 4 && name.ends_with(suffix) {
                let alias = &name[..3];
                if !model.animations.contains_key(alias) {
                    let clip = model.animations[name].clone();
                    Arc::make_mut(&mut model.animations).insert(alias.into(), clip);
                }
            }
        }
    }
}

fn armor(appearance: &CharacterAppearance, slot: usize) -> u32 {
    let explicit = appearance.equipment[slot].material;
    let fallback = if slot == 0 {
        appearance.helm_texture
    } else {
        appearance.texture
    };
    if explicit != 0 {
        explicit
    } else if fallback == 255 {
        0
    } else {
        u32::from(fallback)
    }
}

impl CharacterLibrary {
    pub(super) fn normalize_luclin_appearance(
        &self,
        code: &str,
        appearance: CharacterAppearance,
    ) -> CharacterAppearance {
        let mut value = self.normalize_appearance(appearance);
        let valid = |value: u8, maximum| if value <= maximum { value } else { 0 };
        value.face = valid(
            appearance.face,
            if matches!(code, "BAM" | "BAF") { 79 } else { 7 },
        );
        // Luclin hair is an authored grayscale texture. Its client-side tint
        // palette is not described by the WLD, so colors remain canonicalized
        // by normalize_appearance until that palette is recovered.
        value.eye_color_1 = valid(appearance.eye_color_1, 9);
        value.eye_color_2 = valid(appearance.eye_color_2, 9);
        value.hair_style = valid(
            appearance.hair_style,
            if code == "ERM" {
                5
            } else if code == "ERF" {
                8
            } else {
                3
            },
        );
        value.beard = valid(appearance.beard, 5);
        value
    }

    /// Returns true only for a replacement actor. The caller retains common
    /// hand equipment and alpha-mask handling after this material pass.
    pub(super) fn apply_luclin_appearance(
        &self,
        model: &mut CharacterModel,
        appearance: &CharacterAppearance,
    ) -> bool {
        if !self.luclin_codes.contains(&model.code) {
            return false;
        }
        for (index, material) in model.materials.iter_mut().enumerate() {
            let Some(original) = material.textures.first() else {
                continue;
            };
            let lower = original.to_ascii_lowercase();
            let Some((stem, _)) = lower.rsplit_once('.') else {
                continue;
            };
            if stem.len() != 9
                || !stem.is_ascii()
                || !stem.starts_with(&model.code.to_ascii_lowercase())
            {
                continue;
            }
            let slot = match &stem[3..5] {
                "he" => 0,
                "ch" => 1,
                "ua" => 2,
                "fa" => 3,
                "hn" => 4,
                "lg" => 5,
                "ft" => 6,
                _ => continue,
            };
            let Ok(piece) = stem[7..9].parse::<u8>() else {
                continue;
            };
            let selector = if slot == 0 {
                if matches!(model.code.as_str(), "ERM" | "ERF") {
                    // Erudites use scalp glyph layers instead of hair meshes.
                    u32::from(appearance.hair_style)
                } else if matches!(model.code.as_str(), "BAM" | "BAF") {
                    u32::from(appearance.face / 10)
                } else {
                    0
                }
            } else {
                armor(appearance, slot)
            };
            let piece = if slot == 0 {
                (appearance.face % 10) * 10 + piece % 10
            } else {
                piece
            };
            let name = format!(
                "{}{:02}{piece:02}_MDF",
                stem[..5].to_ascii_uppercase(),
                selector
            );
            let Some(layers) = self.luclin_material_layers(&name).or_else(|| {
                self.luclin_material_layers(&format!(
                    "{}00{:02}_MDF",
                    stem[..5].to_ascii_uppercase(),
                    piece % 10
                ))
            }) else {
                continue;
            };
            let tint = if slot != 0 && selector > 0 {
                appearance.equipment[slot].color
            } else {
                0
            };
            if let Some(texture) = self.luclin_layered_texture(&layers, tint) {
                material.textures = vec![texture];
                material.transparent = false;
                material.alpha_mask = false;
                model.material_slots[index] = Some(slot);
            }
        }
        self.luclin_eyes(model, appearance);
        self.luclin_parts(model, appearance);
        true
    }

    fn luclin_eyes(&self, model: &mut CharacterModel, appearance: &CharacterAppearance) {
        for index in 0..model.meshes.len() {
            let original = model.meshes[index].material;
            if !model.materials[original]
                .textures
                .iter()
                .any(|name| name.to_ascii_lowercase().starts_with("chr_eye"))
            {
                continue;
            }
            let left = model.bindings[index]
                .iter()
                .any(|binding| model.bone_names[binding.bone].contains("EYEL_TRACK"));
            let color = if left {
                appearance.eye_color_2
            } else {
                appearance.eye_color_1
            };
            let name = format!("chr_eye{:03}.dds", u16::from(color) + 1);
            if !self.textures.contains_key(&name) && !self.loose_textures.contains_key(&name) {
                continue;
            }
            let mut material = model.materials[original].clone();
            material.textures = vec![name];
            let material_index = model
                .materials
                .iter()
                .position(|entry| *entry == material)
                .unwrap_or_else(|| {
                    model.materials.push(material);
                    model.material_slots.push(None);
                    model.materials.len() - 1
                });
            model.meshes[index].material = material_index;
        }
    }

    fn luclin_parts(&self, model: &mut CharacterModel, appearance: &CharacterAppearance) {
        let Some(index) = part_index(&model.code) else {
            return;
        };
        let offset = index * 30;
        let helm = armor(appearance, 0);
        let mut parts = Vec::new();
        // Styles 0 are bald/clean-shaven. Modular model numbering starts at 0
        // for the first visible style, hence the explicit subtraction here.
        if appearance.hair_style > 0 && helm < 2 && !matches!(model.code.as_str(), "ERM" | "ERF") {
            parts.push((
                1000 + offset + u32::from(appearance.hair_style) - 1,
                "HAIR_POINT_TRACK",
                None,
                0,
            ));
        }
        if appearance.beard > 0 && helm < 3 {
            parts.push((
                2000 + offset + u32::from(appearance.beard) - 1,
                "BEARD_POINT_TRACK",
                None,
                0,
            ));
        }
        if (1..=3).contains(&helm) {
            parts.push((
                5000 + offset + helm - 1,
                "HEAD_POINT_TRACK",
                Some(0),
                appearance.equipment[0].color,
            ));
        }
        let chest = armor(appearance, 1);
        if (10..=16).contains(&chest) {
            parts.push((
                4000 + offset + chest - 10,
                "TUNIC_POINT_TRACK",
                Some(1),
                appearance.equipment[1].color,
            ));
        } else if (1..=3).contains(&chest) {
            parts.push((
                4010 + offset + chest - 1,
                "TUNIC_POINT_TRACK",
                Some(1),
                appearance.equipment[1].color,
            ));
        }
        if chest == 3 {
            parts.push((
                6000 + offset,
                "CHEST_POINT_TRACK",
                Some(1),
                appearance.equipment[1].color,
            ));
        }
        // Authored plate modules are paired consecutively, left then right;
        // the six preceding IDs reserve the other armor material variants.
        for (base, slot, left, right) in [
            (7000, 2, "SHOULDL_POINT_TRACK", "SHOULDR_POINT_TRACK"),
            (8000, 4, "GAUNTL_POINT_TRACK", "GAUNTR_POINT_TRACK"),
            (9000, 5, "LEGL_POINT_TRACK", "LEGR_POINT_TRACK"),
        ] {
            if armor(appearance, slot) == 3 {
                for (side, anchor) in [left, right].into_iter().enumerate() {
                    parts.push((
                        base + offset + 6 + side as u32,
                        anchor,
                        Some(slot),
                        appearance.equipment[slot].color,
                    ));
                }
            }
        }
        for (part, anchor, slot, tint) in parts {
            if let Err(error) = self.append_luclin_part(model, part, anchor, slot, tint) {
                tracing::debug!(%error, model = %model.code, part, "Luclin appearance part unavailable");
            }
        }
    }

    fn append_luclin_part(
        &self,
        model: &mut CharacterModel,
        part: u32,
        anchor: &str,
        slot: Option<usize>,
        tint: u32,
    ) -> Result<()> {
        let code = format!("IT{part}");
        let &(wld_index, actor_index) = self
            .actors
            .get(&code)
            .ok_or_else(|| Error::NotFound(code.clone()))?;
        let wld = &self.wlds[wld_index];
        if !wld.filename.to_ascii_lowercase().starts_with("lgequip") {
            return Err(Error::NotFound(format!("Luclin part {code}")));
        }
        let Fragment::ActorDef(actor) = &wld.chunks()[actor_index].fragment else {
            unreachable!()
        };
        let anchor = model
            .bone_names
            .iter()
            .position(|name| name.ends_with(anchor))
            .ok_or_else(|| Error::NotFound(format!("{} {anchor}", model.code)))?;
        let body_bind = model
            .bone_transforms("", 0., false)
            .ok_or_else(|| Error::Format("missing Luclin body bind pose".into()))?;
        let skeleton = actor.references.iter().find_map(|reference| {
            match &wld.resolve(*reference)?.fragment {
                Fragment::Skeleton(skeleton) => Some(skeleton),
                Fragment::SkeletonRef(reference) => {
                    match &wld.resolve(reference.skeleton)?.fragment {
                        Fragment::Skeleton(skeleton) => Some(skeleton),
                        _ => None,
                    }
                }
                _ => None,
            }
        });
        let (references, remap, corrections) = if let Some(skeleton) = skeleton {
            let (parents, order) = hierarchy(skeleton)?;
            let mut transforms = vec![Mat4::IDENTITY; skeleton.tracks.len()];
            let mut remap = vec![anchor; skeleton.tracks.len()];
            for &bone in &order {
                let track = &skeleton.tracks[bone];
                let Fragment::PieceTrackRef(reference) = &wld
                    .resolve(track.piece_track)
                    .ok_or_else(|| Error::Format("missing Luclin part track".into()))?
                    .fragment
                else {
                    return Err(Error::Format("invalid Luclin part track".into()));
                };
                let Fragment::PieceTrack(track_data) = &wld
                    .resolve(reference.track)
                    .ok_or_else(|| Error::Format("missing Luclin part bind frame".into()))?
                    .fragment
                else {
                    return Err(Error::Format("invalid Luclin part bind frame".into()));
                };
                let frame = track_data
                    .frames
                    .first()
                    .ok_or_else(|| Error::Format("empty Luclin part track".into()))?;
                let local = Mat4::from_scale_rotation_translation(
                    Vec3::splat(frame.scale),
                    rotation(frame.rotation),
                    Vec3::from_array(frame.translation),
                );
                transforms[bone] =
                    parents[bone].map_or(body_bind[anchor], |parent| transforms[parent]) * local;
                if let Some(suffix) = wld
                    .reference_name(track.piece_track)
                    .and_then(|name| name.strip_prefix(&code))
                {
                    let name = format!("{}{suffix}", model.code);
                    if let Some(index) = model.bone_names.iter().position(|bone| *bone == name) {
                        remap[bone] = index;
                    }
                }
            }
            let corrections = transforms
                .iter()
                .zip(&remap)
                .map(|(transform, &target)| body_bind[target].inverse() * transform)
                .collect::<Vec<_>>();
            (skeleton.meshes.clone(), remap, corrections)
        } else {
            (actor.references.clone(), vec![anchor], vec![Mat4::IDENTITY])
        };
        let material_start = model.materials.len();
        for reference in references {
            let Some((_, source)) = resolve_mesh(wld, reference) else {
                continue;
            };
            let mut mesh = source.clone();
            if skeleton.is_none() {
                mesh.vertex_pieces = vec![(mesh.vertices.len() as u16, anchor as u16)];
            } else {
                let mut offset = 0;
                for (count, source_bone) in &mut mesh.vertex_pieces {
                    let bone = usize::from(*source_bone);
                    let correction = corrections
                        .get(bone)
                        .ok_or_else(|| Error::Format("Luclin part bone out of range".into()))?;
                    let end = offset + usize::from(*count);
                    if end > mesh.vertices.len() {
                        return Err(Error::Format(
                            "Luclin part bone runs exceed vertices".into(),
                        ));
                    }
                    for index in offset..end {
                        mesh.vertices[index] = correction
                            .transform_point3(Vec3::from_array(mesh.vertices[index]))
                            .to_array();
                        mesh.normals[index] = correction
                            .transform_vector3(Vec3::from_array(mesh.normals[index]))
                            .normalize_or_zero()
                            .to_array();
                    }
                    *source_bone = remap[bone] as u16;
                    offset = end;
                }
            }
            append_mesh(wld, &mesh, model.bone_names.len(), slot, model)?;
        }
        if tint >> 24 != 0 {
            for material in &mut model.materials[material_start..] {
                for name in &mut material.textures {
                    *name = format!("{name}#tint={tint:08x}");
                }
            }
        }
        Ok(())
    }

    fn luclin_material_layers(&self, name: &str) -> Option<Vec<String>> {
        self.wlds.iter().find_map(|wld| {
            // A classic material with the same name must not supply a Luclin
            // variant that is missing in the replacement's authored palette.
            if !wld.filename.to_ascii_lowercase().contains("_amr")
                && !self.luclin_codes.iter().any(|code| {
                    wld.filename.eq_ignore_ascii_case(&format!(
                        "global{}_chr.wld",
                        code.to_ascii_lowercase()
                    ))
                })
            {
                return None;
            }
            let Fragment::Material(material) = &wld.by_name(name)?.fragment else {
                return None;
            };
            let Fragment::AnimationRef(reference) = &wld.resolve(material.animation)?.fragment
            else {
                return None;
            };
            let Fragment::Animation(animation) = &wld.resolve(reference.animation)?.fragment else {
                return None;
            };
            let Fragment::TextureList(list) = &wld.resolve(*animation.textures.first()?)?.fragment
            else {
                return None;
            };
            Some(
                list.filenames
                    .iter()
                    .map(|name| {
                        name.trim_end_matches("_LAYER")
                            .trim_end_matches("_layer")
                            .to_ascii_lowercase()
                    })
                    .collect(),
            )
        })
    }

    fn luclin_layered_texture(&self, layers: &[String], tint: u32) -> Option<String> {
        let key = format!("luclin:{}:{tint:08x}", layers.join("+"));
        if self.decoded_textures.lock().ok()?.contains_key(&key) {
            return Some(key);
        }
        let textures: Vec<_> = layers
            .iter()
            .map(|name| self.texture(name))
            .collect::<Option<_>>()?;
        let width = textures.iter().map(|texture| texture.width).max()?;
        let height = textures.iter().map(|texture| texture.height).max()?;
        let mut rgba = vec![0_u8; width as usize * height as usize * 4];
        for (index, texture) in textures.into_iter().enumerate() {
            let source = image::RgbaImage::from_raw(texture.width, texture.height, texture.rgba)?;
            let source = if source.width() == width && source.height() == height {
                source
            } else {
                image::imageops::resize(
                    &source,
                    width,
                    height,
                    image::imageops::FilterType::Triangle,
                )
            };
            for (dst, source) in rgba
                .chunks_exact_mut(4)
                .zip(source.as_raw().chunks_exact(4))
            {
                let alpha = u32::from(source[3]);
                for channel in 0..3 {
                    let color = if index > 0 && tint >> 24 != 0 {
                        u32::from(source[channel]) * ((tint >> (16 - channel * 8)) & 255) / 255
                    } else {
                        u32::from(source[channel])
                    };
                    dst[channel] = ((color * alpha + u32::from(dst[channel]) * (255 - alpha) + 127)
                        / 255) as u8;
                }
                dst[3] = (alpha + (u32::from(dst[3]) * (255 - alpha) + 127) / 255) as u8;
            }
        }
        self.decoded_textures.lock().ok()?.insert(
            key.clone(),
            Texture {
                name: key.clone(),
                width,
                height,
                rgba,
            },
        );
        Some(key)
    }
}

fn part_index(code: &str) -> Option<u32> {
    [
        "GNM", "GNF", "HAM", "HAF", "DWM", "DWF", "DAM", "DAF", "ELM", "ELF", "HIM", "HIF", "HUM",
        "HUF", "IKM", "IKF", "ERM", "ERF", "HOM", "HOF", "TRM", "TRF", "OGM", "OGF", "BAM", "BAF",
        "KEM", "KEF",
    ]
    .iter()
    .position(|entry| *entry == code)
    .map(|index| index as u32)
}
