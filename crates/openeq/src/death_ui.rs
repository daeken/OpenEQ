//! Recovery dialogs using the original XML skin. Append this frame after the
//! ordinary HUD/map, and dispatch RecoveryAction before generic HUD actions.
use crate::{
    death::{RecoveryAction, RecoveryIntent, RecoveryPhase, RecoveryToken, RecoveryView},
    gameplay_ui::{GOLD, MUTED, Painter, WHITE},
    hud::Hud,
};
use openeq_ui::{HitTarget, Rect, UiBindings, UiFrame};

impl RecoveryAction {
    /// Only enabled, exactly formed recovery hits are actions. Panel capture
    /// hits deliberately return None so disabled controls cannot click through.
    pub fn from_hit(hit: &HitTarget) -> Option<Self> {
        if !hit.enabled {
            return None;
        }
        let mut parts = hit.item.strip_prefix("recovery:")?.split(':');
        let generation = parts.next()?.parse().ok()?;
        let revision = parts.next()?.parse().ok()?;
        let intent = match parts.next()? {
            "select" => RecoveryIntent::SelectOption(parts.next()?.parse().ok()?),
            "respawn" => RecoveryIntent::Respawn(parts.next()?.parse().ok()?),
            "page" => RecoveryIntent::PageOptions(parts.next()?.parse().ok()?),
            "accept" => RecoveryIntent::AcceptResurrection,
            "decline" => RecoveryIntent::DeclineResurrection,
            _ => return None,
        };
        if parts.next().is_some() {
            return None;
        }
        Some(Self {
            token: RecoveryToken {
                generation,
                revision,
            },
            intent,
        })
    }
}

fn action_id(view: &RecoveryView, suffix: &str) -> String {
    format!(
        "recovery:{}:{}:{suffix}",
        view.token.generation, view.token.revision
    )
}

impl Hud {
    /// A self-contained overlay; drawing has no command or timeout side effects.
    pub fn recovery_frame(
        &self,
        viewport: [u32; 2],
        pointer: Option<[f32; 2]>,
        view: &RecoveryView,
    ) -> UiFrame {
        let screen = Rect::new(0., 0., viewport[0] as f32, viewport[1] as f32);
        let mut draw = Painter {
            hud: self,
            screen,
            pointer,
            frame: UiFrame {
                bounds: screen,
                ..Default::default()
            },
        };
        if screen.is_empty() || (view.phase == RecoveryPhase::Alive && view.resurrection.is_none())
        {
            return draw.frame;
        }
        let margin = 8_f32.min(screen.width * 0.05).min(screen.height * 0.05);
        let width = 520_f32.min(screen.width - margin * 2.);
        let height = if let Some(choices) = &view.respawn {
            150. + choices.options.len().min(8) as f32 * 28.
                + if view.resurrection.is_some() { 40. } else { 0. }
        } else if view.resurrection.is_some() {
            186.
        } else {
            108.
        };
        let height = height.min(screen.height - margin * 2.);
        let rect = Rect::new(
            (screen.width - width) * 0.5,
            (screen.height - height) * 0.35,
            width,
            height,
        );
        if view.respawn.is_some() {
            draw.respawn_dialog(rect, view);
        } else if view.resurrection.is_some() {
            draw.resurrection_dialog(rect, view);
        } else {
            draw.recovery_shell("RespawnWnd", rect, "Recovery");
            draw.text(
                Rect::new(
                    rect.x + 12.,
                    rect.y + 32.,
                    rect.width - 24.,
                    rect.height - 42.,
                ),
                view.status,
                WHITE,
                true,
            );
        }
        draw.frame.warnings.sort();
        draw.frame.warnings.dedup();
        draw.frame
    }
}

impl Painter<'_> {
    fn recovery_shell(&mut self, template: &str, rect: Rect, title: &str) {
        let mut bindings = UiBindings::default();
        if let Some(root) = self.hud.ui.definition(template) {
            for child in root.values("Pieces").chain(root.values("Pages")) {
                bindings.widget_mut(child).visible = Some(false);
            }
        }
        bindings.widget_mut(template).rect = Some(rect);
        bindings.widget_mut(template).text = Some(title.into());
        self.widget(template, &bindings);
        // Add the capture before interactive rows/buttons. Disabled hits are
        // skipped by UiFrame::hit_test, so this remains their topmost fallback.
        self.hit("recovery:panel", "RecoveryDialog", rect, None);
    }

    fn respawn_dialog(&mut self, rect: Rect, view: &RecoveryView) {
        let choices = view.respawn.as_ref().unwrap();
        self.recovery_shell("RespawnWnd", rect, "Respawn options");
        let inset = 12_f32.min(rect.width * 0.05);
        let inner_x = rect.x + inset;
        let inner_width = rect.width - 2. * inset;
        let header = 52_f32.min(rect.height * 0.25);
        let footer = 76_f32.min(rect.height * 0.4);
        let banner = if view.resurrection.is_some() {
            40_f32.min(rect.height * 0.15)
        } else {
            0.
        };
        let mut bindings = UiBindings::default();
        let gauge = bindings.widget_mut("RW_TimeGauge");
        gauge.rect = Some(Rect::new(
            inner_x,
            rect.y + header * 0.55,
            inner_width,
            header * 0.4,
        ));
        gauge.gauge = Some(choices.timer_fraction);
        gauge.text = Some(format!(
            "{}:{:02} left",
            choices.remaining_seconds / 60,
            choices.remaining_seconds % 60
        ));
        self.widget("RW_TimeGauge", &bindings);
        if let Some(offer) = &view.resurrection {
            self.text(
                Rect::new(inner_x, rect.y + header, inner_width, banner),
                format!("{} offers to return you to your corpse.", offer.caster),
                GOLD,
                true,
            );
        }
        let list_top = rect.y + header + banner;
        let list_height = (rect.height - header - banner - footer).max(0.);
        let row_height = 28_f32.min(list_height.max(1.));
        let visible = ((list_height / row_height).floor() as usize).max(1);
        let first = choices
            .first_visible
            .min(choices.options.len().saturating_sub(visible));
        let end = (first + visible).min(choices.options.len());
        for (row, option) in choices.options[first..end].iter().enumerate() {
            let bounds = Rect::new(
                inner_x,
                list_top + row as f32 * row_height,
                inner_width,
                row_height - 1.,
            );
            self.fill(
                bounds,
                if option.selected {
                    [65, 59, 38, 235]
                } else {
                    [12, 16, 21, 225]
                },
            );
            if option.selected {
                self.outline(bounds, GOLD);
            }
            self.text(
                Rect::new(
                    bounds.x + 6.,
                    bounds.y + 5.,
                    (bounds.width - 12.).max(0.),
                    (bounds.height - 5.).max(0.),
                ),
                if option.requires_resurrection && !option.enabled && view.resurrection.is_none() {
                    format!("{} — awaiting an offer", option.label)
                } else {
                    option.label.clone()
                },
                if option.enabled { WHITE } else { MUTED },
                false,
            );
            self.hit_enabled(
                action_id(view, &format!("select:{}", option.id)),
                "RecoveryChoice",
                bounds,
                Some(option.label.clone()),
                option.enabled,
            );
        }
        let footer_y = rect.bottom() - footer;
        let unit = footer / 3.;
        self.text(
            Rect::new(inner_x, footer_y, inner_width, unit),
            view.status,
            MUTED,
            false,
        );
        if choices.options.len() > visible {
            let button_width = (inner_width * 0.25).min(110.);
            self.button_enabled(
                "RW_SelectButton",
                &action_id(view, &format!("page:-{visible}")),
                Rect::new(inner_x, footer_y + unit, button_width, unit - 2.),
                "Previous",
                false,
                first > 0 && !view.pending,
            );
            self.text(
                Rect::new(
                    inner_x + button_width + 4.,
                    footer_y + unit + 3.,
                    inner_width - 2. * button_width - 8.,
                    unit - 3.,
                ),
                format!("{}–{} of {}", first + 1, end, choices.options.len()),
                MUTED,
                false,
            );
            self.button_enabled(
                "RW_SelectButton",
                &action_id(view, &format!("page:{visible}")),
                Rect::new(
                    rect.right() - inset - button_width,
                    footer_y + unit,
                    button_width,
                    unit - 2.,
                ),
                "Next",
                false,
                end < choices.options.len() && !view.pending,
            );
        }
        let button_y = footer_y + 2. * unit;
        let split = if view.resurrection.is_some() {
            (inner_width - 8.) * 0.5
        } else {
            inner_width
        };
        self.button_enabled(
            "RW_SelectButton",
            &action_id(
                view,
                &format!("respawn:{}", choices.selected.unwrap_or(u32::MAX)),
            ),
            Rect::new(inner_x, button_y, split, unit - 4.),
            if view.pending {
                "Waiting…"
            } else {
                "Respawn"
            },
            false,
            choices.can_respawn,
        );
        if let Some(offer) = &view.resurrection {
            self.button_enabled(
                "CD_No_Button",
                &action_id(view, "decline"),
                Rect::new(inner_x + split + 8., button_y, split, unit - 4.),
                "Decline resurrection",
                false,
                offer.can_decline,
            );
        }
    }

    fn resurrection_dialog(&mut self, rect: Rect, view: &RecoveryView) {
        let offer = view.resurrection.as_ref().unwrap();
        self.recovery_shell("ConfirmationDialogBox", rect, "Resurrection");
        let inset = 12_f32.min(rect.width * 0.05);
        let footer = 34_f32.min(rect.height * 0.24);
        let message = if view.pending {
            if view.phase == RecoveryPhase::Alive {
                "Sending your answer…".to_owned()
            } else {
                view.status.to_owned()
            }
        } else if offer.can_accept {
            format!(
                "{} offers to resurrect you at your corpse.\n\nAccept this resurrection?",
                offer.caster
            )
        } else {
            format!(
                "{} offers resurrection. Waiting for an available respawn choice.",
                offer.caster
            )
        };
        self.text(
            Rect::new(
                rect.x + inset,
                rect.y + 30.,
                rect.width - inset * 2.,
                (rect.height - footer - 34.).max(0.),
            ),
            message,
            WHITE,
            true,
        );
        let width = (rect.width - inset * 2. - 8.) * 0.5;
        self.button_enabled(
            "CD_Yes_Button",
            &action_id(view, "accept"),
            Rect::new(rect.x + inset, rect.bottom() - footer, width, footer - 6.),
            "Accept",
            false,
            offer.can_accept,
        );
        self.button_enabled(
            "CD_No_Button",
            &action_id(view, "decline"),
            Rect::new(
                rect.x + inset + width + 8.,
                rect.bottom() - footer,
                width,
                footer - 6.,
            ),
            "Decline",
            false,
            offer.can_decline,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::death::{RecoveryChoiceView, RespawnView, ResurrectionView};
    use openeq_ui::{DrawCommand, Element, UiDocument};

    fn hud() -> Hud {
        let mut ui = UiDocument::default();
        for (kind, item) in [
            ("Screen", "RespawnWnd"),
            ("Screen", "ConfirmationDialogBox"),
            ("Gauge", "RW_TimeGauge"),
            ("Button", "RW_SelectButton"),
            ("Button", "CD_Yes_Button"),
            ("Button", "CD_No_Button"),
        ] {
            ui.definitions.insert(
                item.into(),
                Element {
                    kind: kind.into(),
                    item: item.into(),
                    ..Default::default()
                },
            );
        }
        Hud {
            ui,
            initial_bindings: UiBindings::default(),
        }
    }
    fn view(count: usize) -> RecoveryView {
        RecoveryView {
            token: RecoveryToken {
                generation: 3,
                revision: 7,
            },
            phase: RecoveryPhase::ChoosingRespawn,
            respawn: Some(RespawnView {
                options: (0..count)
                    .map(|i| RecoveryChoiceView {
                        id: i as u32,
                        zone_id: 202,
                        label: if i == count - 1 {
                            "Resurrect".into()
                        } else {
                            format!("Bind location {i}")
                        },
                        enabled: i != count - 1,
                        selected: i == 0,
                        requires_resurrection: i == count - 1,
                    })
                    .collect(),
                first_visible: 0,
                selected: Some(0),
                remaining_seconds: 299,
                timer_fraction: 299. / 300.,
                can_respawn: true,
            }),
            resurrection: None,
            pending: false,
            status: "Choose a respawn location.",
        }
    }
    fn center(hit: &HitTarget) -> [f32; 2] {
        [
            hit.rect.x + hit.rect.width * 0.5,
            hit.rect.y + hit.rect.height * 0.5,
        ]
    }

    #[test]
    fn recovery_hits_preserve_tokens_and_disabled_choice_captures_clicks() {
        let frame = hud().recovery_frame([800, 600], None, &view(2));
        assert!(frame.warnings.is_empty(), "{:?}", frame.warnings);
        let choice = frame
            .hit_targets
            .iter()
            .find(|hit| hit.item.ends_with(":select:0"))
            .unwrap();
        let action = RecoveryAction::from_hit(frame.hit_test(center(choice)).unwrap()).unwrap();
        assert_eq!(
            action.token,
            RecoveryToken {
                generation: 3,
                revision: 7
            }
        );
        assert_eq!(action.intent, RecoveryIntent::SelectOption(0));
        let disabled = frame
            .hit_targets
            .iter()
            .find(|hit| hit.item.ends_with(":select:1"))
            .unwrap();
        assert!(!disabled.enabled);
        assert!(RecoveryAction::from_hit(disabled).is_none());
        assert_eq!(
            frame.hit_test(center(disabled)).unwrap().item,
            "recovery:panel"
        );
        assert!(
            !frame.hit_targets.iter().any(
                |hit| hit.item.starts_with("game:close:") || hit.item.starts_with("game:drag:")
            )
        );
    }

    #[test]
    fn small_viewport_pages_long_lists_without_outside_hit_targets() {
        let mut view = view(256);
        view.respawn.as_mut().unwrap().options[0].label =
            "A very long quest-defined destination ".repeat(40);
        let frame = hud().recovery_frame([320, 240], None, &view);
        let rows: Vec<_> = frame
            .hit_targets
            .iter()
            .filter(|hit| hit.item.contains(":select:"))
            .collect();
        assert!(!rows.is_empty() && rows.len() < 256);
        assert!(frame.hit_targets.iter().any(|hit| hit.enabled
            && hit.item.contains(":page:")
            && !hit.item.contains(":page:-")));
        for hit in &frame.hit_targets {
            assert!(
                hit.rect.x >= 0.
                    && hit.rect.y >= 0.
                    && hit.rect.right() <= 320.
                    && hit.rect.bottom() <= 240.,
                "{} {:?}",
                hit.item,
                hit.rect
            );
        }
        view.respawn.as_mut().unwrap().first_visible = 255;
        let frame = hud().recovery_frame([320, 240], None, &view);
        assert!(
            frame
                .hit_targets
                .iter()
                .any(|hit| hit.item.ends_with(":select:255"))
        );
        assert!(
            frame
                .hit_targets
                .iter()
                .any(|hit| hit.enabled && hit.item.contains(":page:-"))
        );
    }

    #[test]
    fn pending_resurrection_has_no_live_buttons_or_success_claim() {
        let mut view = view(2);
        view.respawn = None;
        view.phase = RecoveryPhase::AwaitingRevival;
        view.pending = true;
        view.status = "Waiting for the server to return you to play…";
        view.resurrection = Some(ResurrectionView {
            caster: "Cleric".into(),
            corpse: "Player corpse".into(),
            spell_id: 388,
            zone_id: 202,
            hovering: false,
            can_accept: false,
            can_decline: false,
        });
        let frame = hud().recovery_frame([640, 480], None, &view);
        assert!(
            !frame
                .hit_targets
                .iter()
                .any(|hit| RecoveryAction::from_hit(hit).is_some())
        );
        for action in ["accept", "decline"] {
            let hit = frame
                .hit_targets
                .iter()
                .find(|hit| hit.item.ends_with(action))
                .unwrap();
            assert_eq!(frame.hit_test(center(hit)).unwrap().item, "recovery:panel");
        }
        assert!(frame.commands.iter().any(
            |command| matches!(command, DrawCommand::Text { text, .. } if text == view.status)
        ));
    }

    #[test]
    fn malformed_or_unrelated_hits_never_create_recovery_actions() {
        for id in [
            "recovery:panel",
            "recovery:0:0:accept:extra",
            "recovery:x:0:accept",
            "recovery:0:0:select:-1",
            "game:attack",
        ] {
            let hit = HitTarget {
                item: id.into(),
                screen_id: String::new(),
                kind: String::new(),
                rect: Rect::default(),
                enabled: true,
                tooltip: None,
            };
            assert!(RecoveryAction::from_hit(&hit).is_none(), "{id}");
        }
        let mut alive = view(2);
        alive.phase = RecoveryPhase::Alive;
        alive.respawn = None;
        assert!(
            hud()
                .recovery_frame([640, 480], None, &alive)
                .commands
                .is_empty()
        );
        assert!(
            hud()
                .recovery_frame([0, 0], None, &view(2))
                .commands
                .is_empty()
        );
    }

    #[test]
    #[ignore = "requires original UI assets and GPU; writes OPENEQ_UI_CAPTURE_DIR"]
    fn capture_original_recovery_dialogs() {
        let Some(directory) = std::env::var_os("OPENEQ_UI_CAPTURE_DIR") else {
            return;
        };
        let base = std::env::var_os("EQ_DIR")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| {
                std::path::PathBuf::from(std::env::var_os("HOME").unwrap()).join("EverQuest")
            });
        let skin = Hud::load(base).unwrap();
        std::fs::create_dir_all(&directory).unwrap();
        let hover = view(2);
        let mut small = view(256);
        small.respawn.as_mut().unwrap().options[0].label =
            "A long quest-provided bind destination by the city gates".into();
        let mut resurrection = view(2);
        resurrection.respawn = None;
        resurrection.phase = RecoveryPhase::Alive;
        resurrection.status = "";
        resurrection.resurrection = Some(ResurrectionView {
            caster: "ClericTest".into(),
            corpse: "Player corpse".into(),
            spell_id: 388,
            zone_id: 202,
            hovering: false,
            can_accept: true,
            can_decline: true,
        });
        let mut offered = hover.clone();
        offered.resurrection = resurrection.resurrection.clone();
        offered.resurrection.as_mut().unwrap().hovering = true;
        offered.respawn.as_mut().unwrap().options[1].enabled = true;
        for (name, viewport, scale, state) in [
            ("respawn.png", [640, 480], 1., hover),
            ("respawn-small.png", [320, 240], 1., small),
            ("resurrection.png", [640, 360], 2., resurrection),
            ("respawn-offer.png", [640, 480], 1., offered),
        ] {
            let frame = skin.recovery_frame(viewport, None, &state);
            assert!(frame.warnings.is_empty(), "{name}: {:?}", frame.warnings);
            let mut renderer = openeq_render::Renderer::new_headless(
                (viewport[0] as f32 * scale) as u32,
                (viewport[1] as f32 * scale) as u32,
            )
            .unwrap();
            renderer.set_ui_scaled(&frame, scale);
            renderer.render_ui();
            let (width, height, pixels) = renderer.read_rgba().unwrap();
            let path = std::path::PathBuf::from(&directory).join(name);
            image::save_buffer(&path, &pixels, width, height, image::ColorType::Rgba8).unwrap();
            eprintln!("wrote {}", path.display());
        }
    }
}
