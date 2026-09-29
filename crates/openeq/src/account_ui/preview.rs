//! Local roster appearance navigation. Opening or rotating a preview never
//! emits an account/network intent or changes the selected character.
use super::*;
use openeq_net::world::Character;

pub(super) struct PreviewChoice {
    token: Token,
    name: String,
    heading: f32,
}

impl AccountInput {
    pub fn preview_character<'a>(&self, view: &'a View) -> Option<&'a Character> {
        let choice = self.preview.as_ref()?;
        if view.stage != Stage::Characters
            || view.token != choice.token
            || view.selected_character.as_deref() != Some(choice.name.as_str())
        {
            return None;
        }
        view.characters
            .iter()
            .find(|c| c.enabled && c.name == choice.name)
    }

    pub fn preview_heading(&self) -> f32 {
        self.preview.as_ref().map_or(256., |choice| choice.heading)
    }

    pub(super) fn open_preview(&mut self, view: &View) {
        if view.stage != Stage::Characters || !playable(view) {
            return;
        }
        self.preview = Some(PreviewChoice {
            token: view.token,
            name: view.selected_character.clone().unwrap(),
            heading: 256.,
        });
        self.preview_transition();
        self.focus = Focus::Back;
    }

    pub(super) fn close_preview(&mut self) {
        self.preview = None;
        self.preview_transition();
        self.focus = Focus::Preview;
    }

    fn preview_transition(&mut self) {
        self.cancel_composition();
        self.preview_revision = self.preview_revision.wrapping_add(1);
        self.preview_held.clone_from(&self.captured);
        self.pressed = None;
        self.notice = None;
    }

    fn rotate_preview(&mut self, delta: f32) {
        if let Some(choice) = &mut self.preview {
            choice.heading = (choice.heading + delta).rem_euclid(512.);
        }
    }

    pub(super) fn preview_action(&mut self, action: &str) {
        match action {
            "back" => self.close_preview(),
            "rotate_left" => {
                self.focus = Focus::RotateLeft;
                self.rotate_preview(-32.);
            }
            "rotate_right" => {
                self.focus = Focus::RotateRight;
                self.rotate_preview(32.);
            }
            _ => {}
        }
    }

    pub(super) fn preview_key(&mut self, key: KeyCode, repeat: bool) {
        match key {
            KeyCode::ArrowLeft => self.rotate_preview(-32.),
            KeyCode::ArrowRight => self.rotate_preview(32.),
            KeyCode::Enter | KeyCode::NumpadEnter | KeyCode::Space if !repeat => match self.focus {
                Focus::Back => self.close_preview(),
                Focus::RotateLeft => self.rotate_preview(-32.),
                Focus::RotateRight => self.rotate_preview(32.),
                _ => {}
            },
            _ => {}
        }
    }
}

impl Paint<'_> {
    pub(super) fn preview(
        &mut self,
        character: &Character,
        input: &AccountInput,
        status: Option<&str>,
    ) {
        let compact = self.screen.height < 360.;
        let header = if compact { 56. } else { 82. };
        let footer = if compact { 78. } else { 100. };
        if status.is_some() {
            self.fill(self.screen, [9, 13, 20, 255]);
        }
        self.fill(
            Rect::new(0., 0., self.screen.width, header),
            [9, 13, 20, 238],
        );
        self.fill(
            Rect::new(0., self.screen.height - footer, self.screen.width, footer),
            [9, 13, 20, 238],
        );
        self.text(
            Rect::new(16., 8., self.screen.width - 32., 28.),
            &bounded_text(&character.name, 128),
            if compact { 4 } else { 5 },
            GOLD,
            false,
        );
        self.text(
            Rect::new(
                16.,
                if compact { 34. } else { 43. },
                self.screen.width - 32.,
                20.,
            ),
            &format!(
                "Level {} · {} · Appearance preview",
                character.level,
                class_name(character.class)
            ),
            2,
            MUTED,
            false,
        );
        if let Some(status) = status {
            self.text(
                Rect::new(
                    24.,
                    header + 18.,
                    self.screen.width - 48.,
                    (self.screen.height - header - footer - 24.).max(0.),
                ),
                status,
                3,
                WHITE,
                true,
            );
        }
        let bottom = self.screen.height;
        self.text(
            Rect::new(
                16.,
                bottom - footer + 4.,
                self.screen.width - 32.,
                if compact { 22. } else { 34. },
            ),
            if compact {
                "Some appearance details are not yet supported."
            } else {
                "Some armor, hair colors and appearance details are not yet supported."
            },
            1,
            MUTED,
            true,
        );
        let gap = 8.;
        let width = ((self.screen.width - 32. - gap * 2.) / 3.).max(0.);
        for (index, (action, label, focus)) in [
            ("back", "Back", Focus::Back),
            ("rotate_left", "Rotate left", Focus::RotateLeft),
            ("rotate_right", "Rotate right", Focus::RotateRight),
        ]
        .into_iter()
        .enumerate()
        {
            self.button(
                "CLW_Quit_Button",
                action,
                Rect::new(
                    16. + index as f32 * (width + gap),
                    bottom - if compact { 48. } else { 55. },
                    width,
                    if compact { 26. } else { 32. },
                ),
                label,
                true,
                input.focus == focus,
                input,
            );
        }
        self.text(
            Rect::new(16., bottom - 17., self.screen.width - 32., 15.),
            "←/→ rotate · Tab to move · Escape to return",
            1,
            MUTED,
            false,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::super::tests::{characters, click, frame, press, release, send, window};
    use super::*;

    #[test]
    fn preview_navigation_rotation_and_back_never_emit_network_intents() {
        let view = characters();
        let mut input = AccountInput::new(Endpoint::default());
        input.sync(&view);
        let roster = frame(&view, &input);
        assert!(click(&mut input, &view, &roster, "preview").is_none());
        assert_eq!(input.preview_character(&view).unwrap().name, "Adventurer1");
        assert!(!input.ime_enabled(&view));
        for key in [
            KeyCode::ArrowUp,
            KeyCode::ArrowDown,
            KeyCode::PageDown,
            KeyCode::KeyW,
        ] {
            assert!(send(&mut input, &view, press(key, None)).is_none());
        }
        assert_eq!(view.selected_character.as_deref(), Some("Adventurer1"));
        for _ in 0..40 {
            assert!(send(&mut input, &view, press(KeyCode::ArrowLeft, None)).is_none());
        }
        assert!((0.0..512.0).contains(&input.preview_heading()));
        let before = input.preview_heading();
        let preview = frame(&view, &input);
        assert!(click(&mut input, &view, &preview, "rotate_right").is_none());
        assert_eq!(input.preview_heading(), (before + 32.).rem_euclid(512.));
        assert!(send(&mut input, &view, press(KeyCode::Escape, None)).is_none());
        assert!(input.preview_character(&view).is_none());
        // A held Escape cannot immediately leave the roster as well.
        assert!(send(&mut input, &view, press(KeyCode::Escape, None)).is_none());
        send(&mut input, &view, release(KeyCode::Escape));
        let roster = frame(&view, &input);
        assert_eq!(
            click(&mut input, &view, &roster, "primary"),
            Some(Intent::Play { token: view.token })
        );
    }

    #[test]
    fn preview_transition_rejects_stale_frames_and_held_enter() {
        let view = characters();
        let mut input = AccountInput::with_main_menu(Endpoint::default());
        input.sync(&view);
        input.focus = Focus::Preview;
        let roster = frame(&view, &input);
        assert!(
            input
                .event(&view, &roster, window(), &press(KeyCode::Enter, None))
                .is_none()
        );
        assert!(input.preview_character(&view).is_some());
        assert!(
            input
                .event(&view, &roster, window(), &press(KeyCode::Space, None))
                .is_none()
        );
        assert!(send(&mut input, &view, press(KeyCode::Enter, None)).is_none());
        assert!(input.preview_character(&view).is_some());
        send(&mut input, &view, release(KeyCode::Enter));
        assert!(send(&mut input, &view, press(KeyCode::Enter, None)).is_none());
        assert!(input.preview_character(&view).is_none());
        // Even a new frame cannot reactivate the Preview button until release.
        assert!(send(&mut input, &view, press(KeyCode::Enter, None)).is_none());
        assert!(input.preview_character(&view).is_none());
        send(&mut input, &view, release(KeyCode::Enter));
        assert!(send(&mut input, &view, press(KeyCode::Enter, None)).is_none());
        assert!(input.preview_character(&view).is_some());
    }

    #[test]
    fn changed_roster_identity_disabled_rows_and_other_stages_invalidate_preview() {
        let original = characters();
        for mode in 0..5 {
            let mut view = characters();
            let mut input = AccountInput::new(Endpoint::default());
            input.sync(&view);
            input.open_preview(&view);
            let old_frame = frame(&view, &input);
            match mode {
                0 => view.token.attempt += 1,
                1 => view.token.revision += 1,
                2 => view.stage = Stage::EnteringZone,
                3 => view.selected_character = Some("Adventurer2".into()),
                _ => view.characters[1].enabled = false,
            }
            assert!(input.preview_character(&view).is_none());
            assert!(
                input
                    .event(
                        &view,
                        &old_frame,
                        window(),
                        &press(KeyCode::ArrowRight, None)
                    )
                    .is_none()
            );
            assert!(input.preview.is_none());
        }
        for name in [None, Some("Missing".into()), Some("Adventurer0".into())] {
            let mut view = characters();
            view.selected_character = name;
            let mut input = AccountInput::new(Endpoint::default());
            input.open_preview(&view);
            assert!(input.preview_character(&view).is_none());
            assert!(
                !frame(&view, &input)
                    .hit_targets
                    .iter()
                    .find(|h| h.screen_id == "preview")
                    .unwrap()
                    .enabled
            );
        }
        assert_eq!(original.selected_character.as_deref(), Some("Adventurer1"));
    }

    #[test]
    fn preview_controls_stay_outside_model_area_and_failure_retains_back() {
        let view = characters();
        let mut input = AccountInput::new(Endpoint::default());
        input.sync(&view);
        input.open_preview(&view);
        for viewport in [[800, 600], [320, 240], [180, 640]] {
            for status in [
                None,
                Some("Loading appearance…"),
                Some("Appearance preview is unavailable."),
            ] {
                let frame = AccountUi::default()
                    .frame_with_preview_status(viewport, &view, &input, 0., status);
                let model = crate::account_preview::model_rect(viewport);
                for action in ["back", "rotate_left", "rotate_right"] {
                    let hit = frame
                        .hit_targets
                        .iter()
                        .find(|h| h.screen_id == action)
                        .unwrap();
                    assert!(hit.enabled && !hit.rect.is_empty());
                    assert!(hit.rect.intersect(model).is_empty());
                    assert!(hit.rect.x >= 0. && hit.rect.right() <= viewport[0] as f32);
                    assert!(hit.rect.y >= 0. && hit.rect.bottom() <= viewport[1] as f32);
                }
                assert!(
                    !frame
                        .hit_targets
                        .iter()
                        .any(|h| h.screen_id == "primary" || h.kind == "AccountRow")
                );
            }
        }
    }

    #[test]
    #[ignore = "requires original character/UI assets and GPU; no network or audio"]
    fn original_roster_previews_classic_luclin_drakkin_at_both_scales_and_compact() {
        use crate::account_preview::{Preview, Request};
        use openeq_render::{Renderer, actors::CharacterModelSet};
        use std::time::{Duration, Instant};
        let dir = std::env::var_os("EQ_CLIENT_DIR")
            .map(std::path::PathBuf::from)
            .or_else(openeq_assets::loader::default_client_dir)
            .unwrap();
        let ui = AccountUi::load(&dir);
        assert!(ui.characters.is_some());
        let output = std::env::var_os("OPENEQ_PREVIEW_CAPTURE_DIR").map(std::path::PathBuf::from);
        if let Some(output) = &output {
            std::fs::create_dir_all(output).unwrap();
        }
        let mut renderer = Renderer::new_headless(800, 600).unwrap();
        for (label, race, model_set) in [
            ("classic", 1, CharacterModelSet::Classic),
            ("luclin", 1, CharacterModelSet::Luclin),
            ("drakkin", 522, CharacterModelSet::Luclin),
        ] {
            let mut view = characters();
            let character = &mut view.characters[1];
            character.race = race;
            character.appearance.face = 2;
            character.appearance.hair_style = 1;
            character.appearance.hair_color = 2;
            character.appearance.beard = 1;
            character.appearance.beard_color = 3;
            character.appearance.eye_color_1 = 2;
            character.appearance.eye_color_2 = 3;
            character.appearance.drakkin_heritage = 2;
            character.appearance.drakkin_tattoo = 3;
            character.appearance.drakkin_details = 2;
            for piece in &mut character.appearance.equipment[..7] {
                piece.material = 3;
                piece.color = 0xff60_90ff;
            }
            character.appearance.primary_model = 1;
            character.appearance.secondary_model = 201;
            let request = Request {
                token: view.token,
                character: character.clone(),
                dir: dir.clone(),
                model_set,
            };
            let mut input = AccountInput::new(Endpoint::default());
            input.sync(&view);
            let roster = ui.frame([800, 600], &view, &input, 0.);
            assert!(click(&mut input, &view, &roster, "preview").is_none());
            let mut preview = Preview::default();
            let deadline = Instant::now() + Duration::from_secs(90);
            while preview.status().is_some() {
                preview.update(Some(request.clone()), &renderer);
                assert!(
                    preview
                        .status()
                        .is_none_or(|text| !text.contains("unavailable")),
                    "{label}: {:?}",
                    preview.status()
                );
                assert!(Instant::now() < deadline, "{label} preview timed out");
                let frame =
                    ui.frame_with_preview_status([800, 600], &view, &input, 0., preview.status());
                renderer.set_ui(&frame);
                renderer.render_ui();
                std::thread::sleep(Duration::from_millis(8));
            }
            for (viewport, scale) in [([800, 600], 1_u32), ([800, 600], 2), ([320, 240], 1)] {
                renderer.resize(viewport[0] * scale, viewport[1] * scale);
                let frame =
                    ui.frame_with_preview_status(viewport, &view, &input, 0., preview.status());
                renderer.set_ui_scaled(&frame, scale as f32);
                let mut front = None;
                for (side, heading) in [("front", 256.), ("back", 0.)] {
                    assert!(preview.render(&mut renderer, viewport, heading));
                    let (width, height, pixels) = renderer.read_rgba().unwrap();
                    let area = crate::account_preview::model_rect(viewport);
                    let body: Vec<_> = pixels
                        .chunks_exact(4)
                        .enumerate()
                        .filter(|(i, _)| {
                            let point = [
                                (*i as u32 % width) as f32 / scale as f32,
                                (*i as u32 / width) as f32 / scale as f32,
                            ];
                            area.contains(point)
                        })
                        .map(|(_, p)| p)
                        .collect();
                    assert!(
                        body.iter()
                            .filter(|p| p[0].max(p[1]).max(p[2]) > 80)
                            .count()
                            > 100,
                        "{label} model is missing"
                    );
                    assert!(
                        body.iter()
                            .filter(|p| p[0] > 230 && p[1] < 25 && p[2] > 230)
                            .count()
                            < 10,
                        "{label} placeholder texture"
                    );
                    if let Some(front) = &front {
                        assert_ne!(&pixels, front, "rotation did not change the preview");
                    } else {
                        front = Some(pixels.clone());
                    }
                    if let Some(output) = &output {
                        image::save_buffer(
                            output.join(format!(
                                "{label}-{}x{}-{scale}x-{side}.png",
                                viewport[0], viewport[1]
                            )),
                            &pixels,
                            width,
                            height,
                            image::ColorType::Rgba8,
                        )
                        .unwrap();
                    }
                }
            }
            let frame = ui.frame([320, 240], &view, &input, 0.);
            assert!(click(&mut input, &view, &frame, "back").is_none());
            assert!(input.preview_character(&view).is_none());
            preview.update(None, &renderer);
        }
    }
}
