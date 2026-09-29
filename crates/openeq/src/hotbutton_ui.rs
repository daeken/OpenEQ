//! Original hotbutton/social-edit art with application-owned command identities.
//! Painting and hit decoding do not dispatch commands or edit saved bindings.
use crate::{
    gameplay_ui::{GOLD, GameHudState, MUTED, Painter, WHITE, chat_edit_text, position},
    hotbuttons::{HOTBUTTON_COUNT, HotbuttonField, HotbuttonToken},
};
use openeq_ui::{HitTarget, Rect, UiBindings};

const SLOT_SIZE: f32 = 40.;
const SLOT_GAP: f32 = 4.;
const BAR_PADDING: f32 = 8.;
const BAR_HEADER: f32 = 24.;

#[cfg(test)]
mod tests;

#[derive(Clone, Debug, Default)]
pub struct UiHotbutton {
    pub label: String,
    pub command: String,
}

#[derive(Clone, Debug, Default)]
pub struct UiHotbuttonEditor {
    pub token: HotbuttonToken,
    pub slot: u8,
    pub label: String,
    pub command: String,
    pub label_cursor: Option<usize>,
    pub command_cursor: Option<usize>,
    pub label_selected: bool,
    pub command_selected: bool,
    pub focus: HotbuttonField,
    pub error: Option<String>,
}

#[derive(Clone, Debug, Default)]
pub struct UiHotbuttons {
    pub token: HotbuttonToken,
    pub open: bool,
    pub enabled: bool,
    pub slots: [Option<UiHotbutton>; HOTBUTTON_COUNT],
    pub editor: Option<UiHotbuttonEditor>,
    pub pressed: Option<HotbuttonAction>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HotbuttonAction {
    pub token: HotbuttonToken,
    pub kind: HotbuttonActionKind,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum HotbuttonActionKind {
    Slot(u8),
    Focus(HotbuttonField),
    Clear,
    Save,
    Cancel,
    CloseBar,
}

fn hit_parts(hit: &HitTarget) -> Option<(HotbuttonToken, &str)> {
    let (revision, rest) = hit.item.strip_prefix("hotbutton:")?.split_once(':')?;
    let (generation, action) = rest.split_once(':')?;
    Some((
        HotbuttonToken {
            revision: revision.parse().ok()?,
            zone_generation: generation.parse().ok()?,
        },
        action,
    ))
}

fn editor_parts(action: &str) -> Option<(u8, &str)> {
    let (slot, control) = action.strip_prefix("editor:")?.split_once(':')?;
    let slot = slot
        .parse::<u8>()
        .ok()
        .filter(|slot| (*slot as usize) < HOTBUTTON_COUNT)?;
    matches!(
        control,
        "label" | "command" | "clear" | "save" | "cancel" | "body"
    )
    .then_some((slot, control))
}

impl HotbuttonAction {
    pub fn from_hit(hit: &HitTarget) -> Option<Self> {
        if !hit.enabled {
            return None;
        }
        let (token, action) = hit_parts(hit)?;
        let kind = match hit.window_id.as_deref()? {
            "hotbuttons" if action == "close" => HotbuttonActionKind::CloseBar,
            "hotbuttons" => HotbuttonActionKind::Slot(
                action
                    .strip_prefix("slot:")?
                    .parse::<u8>()
                    .ok()
                    .filter(|slot| (*slot as usize) < HOTBUTTON_COUNT)?,
            ),
            "hotbutton_editor" => match editor_parts(action)?.1 {
                "label" => HotbuttonActionKind::Focus(HotbuttonField::Label),
                "command" => HotbuttonActionKind::Focus(HotbuttonField::Command),
                "clear" => HotbuttonActionKind::Clear,
                "save" => HotbuttonActionKind::Save,
                "cancel" => HotbuttonActionKind::Cancel,
                _ => return None,
            },
            _ => return None,
        };
        Some(Self { token, kind })
    }
}

/// Includes the editor's inert blocking body and disabled controls, unlike actions.
pub fn hotbutton_editor_hit_identity(hit: &HitTarget) -> Option<(HotbuttonToken, u8)> {
    if hit.window_id.as_deref() != Some("hotbutton_editor") {
        return None;
    }
    let (token, action) = hit_parts(hit)?;
    Some((token, editor_parts(action)?.0))
}

/// Captures pointer ownership for both windows, including their standard drag hits.
pub fn hotbutton_window_hit(hit: &HitTarget) -> bool {
    matches!(
        hit.window_id.as_deref(),
        Some("hotbuttons" | "hotbutton_editor")
    )
}

fn id(token: HotbuttonToken, suffix: &str) -> String {
    format!(
        "hotbutton:{}:{}:{suffix}",
        token.revision, token.zone_generation
    )
}

/// Responsive slot geometry in logical pixels; callers may use it in previews.
pub fn hotbutton_bar_size(viewport_width: u32) -> (usize, [f32; 2]) {
    let available = (viewport_width as f32).min(540.);
    let columns = (((available - BAR_PADDING * 2. + SLOT_GAP) / (SLOT_SIZE + SLOT_GAP)).floor()
        as usize)
        .clamp(1, HOTBUTTON_COUNT);
    let rows = HOTBUTTON_COUNT.div_ceil(columns);
    (
        columns,
        [
            (columns as f32 * (SLOT_SIZE + SLOT_GAP) - SLOT_GAP + BAR_PADDING * 2.).min(available),
            BAR_HEADER + rows as f32 * (SLOT_SIZE + SLOT_GAP) - SLOT_GAP + BAR_PADDING,
        ],
    )
}

impl Painter<'_> {
    pub(crate) fn hotbuttons(&mut self, state: &GameHudState, bar: &UiHotbuttons) {
        if bar.open {
            let (columns, [width, height]) = hotbutton_bar_size(self.screen.width as u32);
            let rect = position(
                state,
                "hotbuttons",
                Rect::new(
                    (self.screen.width - width) * 0.5,
                    self.screen.height - height - 280.,
                    width,
                    height,
                ),
                self.screen,
            );
            self.shell("HotButtonWnd", "hotbuttons", rect, "", true);
            self.hotbutton_shell_hits(bar.token, None, bar.editor.is_none());
            self.text(
                Rect::new(rect.x + 8., rect.y + 3., (rect.width - 36.).max(0.), 18.),
                "Hotbuttons",
                WHITE,
                false,
            );
            let enabled = bar.enabled && bar.editor.is_none();
            for (slot, binding) in bar.slots.iter().enumerate() {
                let bounds = Rect::new(
                    rect.x + BAR_PADDING + (slot % columns) as f32 * (SLOT_SIZE + SLOT_GAP),
                    rect.y + BAR_HEADER + (slot / columns) as f32 * (SLOT_SIZE + SLOT_GAP),
                    SLOT_SIZE,
                    SLOT_SIZE,
                )
                .intersect(rect);
                if bounds.is_empty() {
                    continue;
                }
                let action = HotbuttonAction {
                    token: bar.token,
                    kind: HotbuttonActionKind::Slot(slot as u8),
                };
                let tooltip = binding.as_ref().map_or_else(
                    || format!("Button {}\nClick to add a command.", slot + 1),
                    |binding| {
                        format!(
                            "{}\n{}\nRight-click to edit.",
                            binding.label, binding.command
                        )
                    },
                );
                self.hotbutton_control(
                    &format!("HB_Button{}", slot + 1),
                    &id(bar.token, &format!("slot:{slot}")),
                    bounds,
                    &binding
                        .as_ref()
                        .map_or_else(|| (slot + 1).to_string(), |binding| binding.label.clone()),
                    enabled,
                    bar.pressed.as_ref() == Some(&action),
                    Some(tooltip),
                );
            }
        }
        if let Some(editor) = bar
            .editor
            .as_ref()
            .filter(|editor| (editor.slot as usize) < HOTBUTTON_COUNT)
        {
            self.hotbutton_editor(state, bar, editor);
        }
    }

    fn hotbutton_shell_hits(
        &mut self,
        token: HotbuttonToken,
        editor_slot: Option<u8>,
        close_enabled: bool,
    ) {
        // shell() starts a fresh logical window; retain only its standard drag
        // as a generic action. Close must use the ordered press/release owner.
        for hit in &mut self.frame.hit_targets {
            if hit.kind == "WindowTitle" {
                continue;
            }
            let close = hit.item.starts_with("game:close:");
            let suffix = editor_slot.map_or_else(
                || if close { "close".into() } else { "body".into() },
                |slot| format!("editor:{slot}:{}", if close { "cancel" } else { "body" }),
            );
            hit.item = id(token, &suffix);
            hit.screen_id = hit.item.clone();
            hit.kind = if close {
                "HotbuttonControl"
            } else {
                "HotbuttonBody"
            }
            .into();
            hit.enabled = !close || close_enabled;
            hit.tooltip = close.then(|| {
                if editor_slot.is_some() {
                    "Cancel edits"
                } else {
                    "Close hotbuttons"
                }
                .into()
            });
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn hotbutton_control(
        &mut self,
        template: &str,
        identity: &str,
        bounds: Rect,
        text: &str,
        enabled: bool,
        pressed: bool,
        tooltip: Option<String>,
    ) {
        let mut bindings = UiBindings::default();
        let widget = bindings.widget_mut(template);
        widget.rect = Some(bounds);
        widget.text = Some(text.into());
        widget.enabled = Some(enabled);
        widget.hovered = enabled && self.pointer.is_some_and(|point| bounds.contains(point));
        widget.pressed = enabled && pressed;
        let start = self.frame.hit_targets.len();
        self.widget(template, &bindings);
        self.frame.hit_targets.truncate(start);
        self.hit_enabled(identity, "HotbuttonControl", bounds, tooltip, enabled);
    }

    fn hotbutton_editor(
        &mut self,
        state: &GameHudState,
        bar: &UiHotbuttons,
        editor: &UiHotbuttonEditor,
    ) {
        let rect = position(
            state,
            "hotbutton_editor",
            Rect::new(self.screen.width * 0.5 - 240., 70., 480., 350.),
            self.screen,
        );
        self.shell(
            "SocialEditWnd",
            "hotbutton_editor",
            rect,
            &format!("Edit hotbutton {}", editor.slot + 1),
            true,
        );
        self.hotbutton_shell_hits(editor.token, Some(editor.slot), true);
        self.text(
            Rect::new(rect.x + 12., rect.y + 29., (rect.width - 24.).max(0.), 34.),
            "One slash command per button. Save does not run it.",
            MUTED,
            true,
        );
        for (field, label, template, y, text, cursor, selected) in [
            (
                HotbuttonField::Label,
                "Name",
                "SEW_NameInput",
                68.,
                &editor.label,
                editor.label_cursor,
                editor.label_selected,
            ),
            (
                HotbuttonField::Command,
                "Command",
                "SEW_Line0Input",
                122.,
                &editor.command,
                editor.command_cursor,
                editor.command_selected,
            ),
        ] {
            self.text(
                Rect::new(rect.x + 12., rect.y + y, (rect.width - 24.).max(0.), 18.),
                label,
                GOLD,
                false,
            );
            let bounds = Rect::new(
                rect.x + 12.,
                rect.y + y + 20.,
                (rect.width - 24.).max(0.),
                26.,
            );
            let focused = editor.focus == field;
            let mut bindings = UiBindings::default();
            let widget = bindings.widget_mut(template);
            widget.rect = Some(bounds);
            widget.text = Some(String::new());
            let start = self.frame.hit_targets.len();
            self.widget(template, &bindings);
            self.frame.hit_targets.truncate(start);
            let inner = Rect::new(
                bounds.x + 5.,
                bounds.y + 4.,
                (bounds.width - 10.).max(0.),
                18.,
            );
            if focused {
                self.outline(bounds, GOLD);
            }
            if focused && selected {
                self.fill(inner, [43, 69, 107, 240]);
            }
            let visible = (inner.width / 8.).max(1.) as usize;
            let shown = if focused {
                chat_edit_text(text, cursor, visible)
            } else {
                text.chars().take(visible).collect()
            };
            self.text(inner, shown, WHITE, false);
            let suffix = match field {
                HotbuttonField::Label => "label",
                HotbuttonField::Command => "command",
            };
            self.hit_enabled(
                id(editor.token, &format!("editor:{}:{suffix}", editor.slot)),
                "HotbuttonField",
                bounds,
                None,
                bar.enabled,
            );
        }
        let buttons_top = rect.bottom() - 38.;
        let error_height = if editor.error.is_some() { 44. } else { 0. };
        let help_bottom = buttons_top - error_height - 8.;
        self.text(Rect::new(rect.x+12.,rect.y+176.,(rect.width-24.).max(0.),(help_bottom-rect.y-176.).max(0.)),
            "Uses your current target and spell gems.\n/useitem and /scribe use the inspected item.\nClear edits the draft; Save confirms changes.",MUTED,true);
        if let Some(error) = &editor.error {
            self.text(
                Rect::new(
                    rect.x + 12.,
                    buttons_top - 48.,
                    (rect.width - 24.).max(0.),
                    42.,
                ),
                error,
                [255, 170, 130, 255],
                true,
            );
        }
        let button_width = ((rect.width - 32.) / 3.).max(0.);
        for (index, (suffix, label, template, kind)) in [
            (
                "clear",
                "Clear",
                "SEW_Clear_Button",
                HotbuttonActionKind::Clear,
            ),
            (
                "save",
                "Save",
                "SEW_Accept_Button",
                HotbuttonActionKind::Save,
            ),
            (
                "cancel",
                "Cancel",
                "SEW_Clear_Button",
                HotbuttonActionKind::Cancel,
            ),
        ]
        .into_iter()
        .enumerate()
        {
            let enabled = bar.enabled || kind == HotbuttonActionKind::Cancel;
            let pressed = bar.pressed.as_ref()
                == Some(&HotbuttonAction {
                    token: editor.token,
                    kind,
                });
            self.hotbutton_control(
                template,
                &id(editor.token, &format!("editor:{}:{suffix}", editor.slot)),
                Rect::new(
                    rect.x + 12. + index as f32 * (button_width + 4.),
                    buttons_top,
                    button_width,
                    26.,
                ),
                label,
                enabled,
                pressed,
                None,
            );
        }
    }

    pub(crate) fn hotbutton_tooltip(&mut self, state: &GameHudState) {
        if state.hover_blocked
            || state.cursor_item.is_some()
            || state
                .hotbuttons
                .as_ref()
                .is_some_and(|bar| bar.editor.is_some())
        {
            return;
        }
        let Some(point) = state.pointer else {
            return;
        };
        let Some(hit) = self.frame.hit_test(point) else {
            return;
        };
        if !matches!(
            HotbuttonAction::from_hit(hit).map(|action| action.kind),
            Some(HotbuttonActionKind::Slot(_))
        ) {
            return;
        }
        let Some(text) = hit.tooltip.clone() else {
            return;
        };
        let width = (self.screen.width - 16.).clamp(0., 420.);
        let line_chars = ((width - 16.).max(8.) / 8.) as usize;
        let rows = text
            .lines()
            .map(|line| line.chars().count().div_ceil(line_chars.max(1)).max(1))
            .sum::<usize>();
        let height = (rows as f32 * 18. + 18.).min(self.screen.height);
        let rect = Rect::new(
            (point[0] + 12.).clamp(0., (self.screen.width - width).max(0.)),
            (point[1] - height - 8.).clamp(0., (self.screen.height - height).max(0.)),
            width,
            height,
        );
        self.fill(rect, [12, 15, 20, 245]);
        self.outline(rect, GOLD);
        self.text(
            Rect::new(
                rect.x + 8.,
                rect.y + 6.,
                (rect.width - 16.).max(0.),
                (rect.height - 12.).max(0.),
            ),
            text,
            WHITE,
            true,
        );
    }
}
