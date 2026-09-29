//! Local navigation around the existing account stages. These pages do not
//! authenticate, issue network requests, or persist credentials/settings.
use super::*;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Page {
    Welcome,
    Connection,
    Credentials,
}

pub(super) struct Navigation {
    pub page: Page,
    pub revision: u64,
    pub held_keys: HashSet<KeyCode>,
    pub connection_snapshot: Option<[String; 3]>,
}
impl Default for Navigation {
    fn default() -> Self {
        Self {
            page: Page::Welcome,
            revision: 1,
            held_keys: HashSet::new(),
            connection_snapshot: None,
        }
    }
}

impl AccountInput {
    pub(super) fn page(&self, view: &View) -> Page {
        if view.stage == Stage::Credentials {
            self.navigation
                .as_ref()
                .map_or(Page::Credentials, |navigation| navigation.page)
        } else {
            Page::Credentials
        }
    }
    pub(super) fn title(&self, view: &View) -> &'static str {
        match self.page(view) {
            Page::Welcome => "OpenEQ",
            Page::Connection => "Connection settings",
            Page::Credentials => view.stage.label(),
        }
    }
    pub(super) fn editable_field(&self, view: &View) -> Option<usize> {
        if view.stage != Stage::Credentials {
            return None;
        }
        self.focus.field().filter(|index| match self.page(view) {
            Page::Welcome => false,
            Page::Connection => *index < 3,
            Page::Credentials => true,
        })
    }
    pub(super) fn focus_order(&self, view: &View) -> &'static [Focus] {
        match self.page(view) {
            Page::Welcome => &[Focus::Primary, Focus::Connection, Focus::Exit],
            Page::Connection => &[
                Focus::Host,
                Focus::LoginPort,
                Focus::WorldPort,
                Focus::Primary,
                Focus::Back,
            ],
            Page::Credentials if view.stage == Stage::Credentials && self.navigation.is_some() => {
                &[
                    Focus::Host,
                    Focus::LoginPort,
                    Focus::WorldPort,
                    Focus::Username,
                    Focus::Password,
                    Focus::Primary,
                    Focus::Back,
                ]
            }
            Page::Credentials => focus_order(view.stage),
        }
    }
    pub(super) fn navigate(&mut self, page: Page) {
        self.cancel_composition();
        self.edits[4].clear();
        let Some(navigation) = &mut self.navigation else {
            return;
        };
        if page == Page::Connection {
            navigation.connection_snapshot =
                Some(std::array::from_fn(|index| self.edits[index].text.clone()));
        }
        navigation.page = page;
        navigation.revision = navigation.revision.wrapping_add(1);
        navigation.held_keys.clone_from(&self.captured);
        self.pressed = None;
        self.notice = None;
        self.focus = match page {
            Page::Welcome => Focus::Primary,
            Page::Connection => Focus::Host,
            Page::Credentials => Focus::Username,
        };
    }
    pub(super) fn restore_connection(&mut self) {
        if let Some(snapshot) = self
            .navigation
            .as_mut()
            .and_then(|navigation| navigation.connection_snapshot.take())
        {
            for (edit, text) in self.edits.iter_mut().zip(snapshot) {
                *edit = Edit::new(text);
            }
        }
    }
}

impl Paint<'_> {
    pub(super) fn welcome(&mut self, inner: Rect, input: &AccountInput) {
        let width = (inner.width - 28.).clamp(0., 300.);
        let x = inner.x + (inner.width - width) * 0.5;
        self.text(
            Rect::new(
                inner.x + 14.,
                inner.y + 50.,
                (inner.width - 28.).max(0.),
                26.,
            ),
            &format!("Server: {}", bounded_text(&input.endpoint().host, 253)),
            2,
            MUTED,
            false,
        );
        let top = (inner.height * 0.3).clamp(88., 120.);
        let height = ((inner.height - top - 40.) / 3.).clamp(0., 36.);
        for (index, (template, action, label, focus)) in [
            ("MAIN_ConnectButton", "primary", "Play", Focus::Primary),
            (
                "MAIN_OptionsButton",
                "connection",
                "Connection settings",
                Focus::Connection,
            ),
            ("MAIN_ExitButton", "exit", "Exit", Focus::Exit),
        ]
        .into_iter()
        .enumerate()
        {
            self.button(
                template,
                action,
                Rect::new(
                    x,
                    inner.y + top + index as f32 * (height + 8.),
                    width,
                    height,
                ),
                label,
                true,
                input.focus == focus,
                input,
            );
        }
    }

    pub(super) fn connection(&mut self, inner: Rect, input: &AccountInput, time: f32) {
        let width = (inner.width - 28.).max(0.);
        let x = inner.x + 14.;
        // Two rows fit compact screens without making the endpoint and ports
        // share space with the validation notice or footer controls.
        let compact = inner.height < 300.;
        let field_height = if compact { 25. } else { 36. };
        let top = if compact { 64. } else { 112. };
        let gap = if compact { 47. } else { 77. };
        let port_width = ((width - 14.) * 0.5).max(0.);
        for (index, label, rect) in [
            (
                0,
                "Server hostname",
                Rect::new(x, inner.y + top, width, field_height),
            ),
            (
                1,
                "Login port",
                Rect::new(x, inner.y + top + gap, port_width, field_height),
            ),
            (
                2,
                "World port",
                Rect::new(
                    x + port_width + 14.,
                    inner.y + top + gap,
                    port_width,
                    field_height,
                ),
            ),
        ] {
            self.edit_field(index, label, rect, input, time);
        }
        let button_y = inner.bottom() - if compact { 42. } else { 54. };
        let button_width = (width * 0.45).min(160.);
        let button_height = if compact { 24. } else { 32. };
        self.button(
            "LOGIN_CancelButton",
            "back",
            Rect::new(x, button_y, button_width, button_height),
            "Back",
            true,
            input.focus == Focus::Back,
            input,
        );
        self.button(
            "LOGIN_ConnectButton",
            "primary",
            Rect::new(
                x + width - button_width,
                button_y,
                button_width,
                button_height,
            ),
            "Done",
            true,
            input.focus == Focus::Primary,
            input,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::super::tests::{click, frame, press, release, send, window};
    use super::*;
    use bevy::input::mouse::MouseButtonInput;

    fn actions(frame: &UiFrame) -> Vec<&str> {
        frame
            .hit_targets
            .iter()
            .filter(|hit| hit.kind == "AccountButton")
            .map(|hit| hit.screen_id.as_str())
            .collect()
    }

    #[test]
    fn welcome_requires_local_play_before_sign_in_and_direct_constructor_is_unchanged() {
        let view = View::default();
        let mut input = AccountInput::with_main_menu(Endpoint::default());
        let welcome = frame(&view, &input);
        assert_eq!(actions(&welcome), ["primary", "connection", "exit"]);
        assert!(
            welcome
                .hit_targets
                .iter()
                .all(|hit| hit.kind != "AccountEdit")
        );
        assert!(!input.ime_enabled(&view));
        assert_eq!(input.title(&view), "OpenEQ");
        assert!(
            send(
                &mut input,
                &view,
                press(KeyCode::KeyA, Some("unowned text"))
            )
            .is_none()
        );
        assert!(input.edits[3].text.is_empty());
        send(&mut input, &view, release(KeyCode::KeyA));
        assert!(send(&mut input, &view, press(KeyCode::Enter, None)).is_none());
        assert_eq!(input.title(&view), "Sign in");
        assert!(input.focus == Focus::Username);
        assert!(input.ime_enabled(&view));
        send(&mut input, &view, release(KeyCode::Enter));
        send(&mut input, &view, press(KeyCode::KeyA, Some("Account")));
        send(&mut input, &view, press(KeyCode::Tab, None));
        send(
            &mut input,
            &view,
            press(KeyCode::KeyP, Some("PRIVATE_SENTINEL")),
        );
        assert!(!format!("{:?}", frame(&view, &input)).contains("PRIVATE_SENTINEL"));
        assert_eq!(
            send(&mut input, &view, press(KeyCode::Enter, None)),
            Some(Intent::SignIn)
        );
        let (endpoint, username, password) = input.take_credentials();
        assert_eq!(endpoint, Endpoint::default());
        assert_eq!(username, "Account");
        assert_eq!(password, "PRIVATE_SENTINEL");
        let direct = AccountInput::new(Endpoint::default());
        assert!(direct.page(&view) == Page::Credentials);
        assert_eq!(actions(&frame(&view, &direct)), ["primary", "exit"]);
    }

    #[test]
    fn settings_validate_apply_and_back_discards_endpoint_draft_without_sign_in() {
        let view = View::default();
        let mut input = AccountInput::with_main_menu(Endpoint::default());
        let welcome = frame(&view, &input);
        assert!(click(&mut input, &view, &welcome, "connection").is_none());
        assert!(input.page(&view) == Page::Connection);
        assert!(input.focus == Focus::Host);
        let settings = frame(&view, &input);
        let fields: Vec<_> = settings
            .hit_targets
            .iter()
            .filter(|hit| hit.kind == "AccountEdit")
            .map(|hit| hit.screen_id.as_str())
            .collect();
        assert_eq!(fields, ["field:0", "field:1", "field:2"]);
        assert_eq!(actions(&settings), ["back", "primary"]);
        for invalid in ["0", "65536", ""] {
            input.edits[1] = Edit::new(invalid.into());
            assert!(send(&mut input, &view, press(KeyCode::Enter, None)).is_none());
            assert!(input.page(&view) == Page::Connection);
            assert!(input.notice.is_some());
            send(&mut input, &view, release(KeyCode::Enter));
        }
        input.edits[0] = Edit::new("eq.example".into());
        input.edits[1] = Edit::new("5998".into());
        input.edits[2] = Edit::new("9001".into());
        assert!(send(&mut input, &view, press(KeyCode::Enter, None)).is_none());
        assert!(input.page(&view) == Page::Welcome);
        assert!(input.notice.is_none());
        let applied = input.endpoint();
        assert_eq!(
            applied,
            Endpoint {
                host: "eq.example".into(),
                login_port: 5998,
                world_port: 9001
            }
        );
        send(&mut input, &view, release(KeyCode::Enter));
        let welcome = frame(&view, &input);
        click(&mut input, &view, &welcome, "connection");
        input.edits[0] = Edit::new("unfinished.example".into());
        input.edits[2] = Edit::new("0".into());
        assert!(send(&mut input, &view, press(KeyCode::Escape, None)).is_none());
        assert!(input.page(&view) == Page::Welcome);
        assert_eq!(input.endpoint(), applied);
        assert_eq!(view.stage, Stage::Credentials);
    }

    #[test]
    fn navigation_clears_password_composition_and_rejects_stale_frames_and_held_keys() {
        let view = View::default();
        let mut input = AccountInput::with_main_menu(Endpoint::default());
        let welcome = frame(&view, &input);
        send(&mut input, &view, press(KeyCode::KeyP, Some("p")));
        click(&mut input, &view, &welcome, "primary");
        assert!(input.page(&view) == Page::Credentials);
        let mut repeat = press(KeyCode::KeyP, Some("p"));
        if let WindowEvent::KeyboardInput(event) = &mut repeat {
            event.repeat = true;
        }
        send(&mut input, &view, repeat);
        assert!(input.edits[3].text.is_empty());
        assert!(
            input
                .event(
                    &view,
                    &welcome,
                    window(),
                    &press(KeyCode::KeyA, Some("stale"))
                )
                .is_none()
        );
        assert!(input.edits[3].text.is_empty());
        send(&mut input, &view, release(KeyCode::KeyP));
        send(&mut input, &view, press(KeyCode::KeyP, Some("p")));
        assert_eq!(input.edits[3].text, "p");
        send(&mut input, &view, release(KeyCode::KeyP));
        input.focus = Focus::Password;
        input.edits[4] = Edit::new("PRIVATE_SENTINEL".into());
        send(
            &mut input,
            &view,
            WindowEvent::Ime(Ime::Preedit {
                window: window(),
                value: "PRIVATE_COMPOSITION".into(),
                cursor: None,
            }),
        );
        let credentials = frame(&view, &input);
        assert!(click(&mut input, &view, &credentials, "back").is_none());
        assert!(input.page(&view) == Page::Welcome);
        assert!(input.edits[4].text.is_empty());
        assert!(input.edits[4].preedit.is_empty());
        assert!(!input.ime_enabled(&view));
        let new_welcome = frame(&view, &input);
        click(&mut input, &view, &new_welcome, "primary");
        assert!(
            input
                .event(
                    &view,
                    &credentials,
                    window(),
                    &WindowEvent::Ime(Ime::Preedit {
                        window: window(),
                        value: "STALE_COMPOSITION".into(),
                        cursor: None,
                    })
                )
                .is_none()
        );
        send(
            &mut input,
            &view,
            WindowEvent::Ime(Ime::Commit {
                window: window(),
                value: "PRIVATE_COMPOSITION".into(),
            }),
        );
        assert_eq!(input.edits[3].text, "p");
        let rendered = format!("{:?}", frame(&view, &input));
        for secret in [
            "PRIVATE_SENTINEL",
            "PRIVATE_COMPOSITION",
            "STALE_COMPOSITION",
        ] {
            assert!(!rendered.contains(secret));
        }
        assert!(
            input
                .event(
                    &view,
                    &new_welcome,
                    window(),
                    &WindowEvent::MouseButtonInput(MouseButtonInput {
                        window: window(),
                        button: MouseButton::Left,
                        state: ButtonState::Released,
                    })
                )
                .is_none()
        );
        assert!(input.page(&view) == Page::Credentials);
    }

    #[test]
    fn menu_tab_order_exit_and_existing_network_back_actions_are_preserved() {
        let mut view = View::default();
        let mut input = AccountInput::with_main_menu(Endpoint::default());
        send(&mut input, &view, press(KeyCode::Tab, None));
        assert!(input.focus == Focus::Connection);
        send(&mut input, &view, release(KeyCode::Tab));
        send(&mut input, &view, press(KeyCode::Tab, None));
        assert!(input.focus == Focus::Exit);
        assert_eq!(
            send(&mut input, &view, press(KeyCode::Enter, None)),
            Some(Intent::Exit)
        );
        let mut input = AccountInput::with_main_menu(Endpoint::default());
        input.navigate(Page::Credentials);
        view.stage = Stage::Authenticating;
        assert_eq!(
            send(&mut input, &view, press(KeyCode::Escape, None)),
            Some(Intent::Cancel)
        );
        view.stage = Stage::Characters;
        assert_eq!(
            send(&mut input, &view, press(KeyCode::Escape, None)),
            Some(Intent::Back { token: view.token })
        );
        view.stage = Stage::Credentials;
        assert!(input.page(&view) == Page::Credentials);
        assert!(send(&mut input, &view, press(KeyCode::Escape, None)).is_none());
        send(&mut input, &view, release(KeyCode::Escape));
        assert_eq!(
            send(&mut input, &view, press(KeyCode::Escape, None)),
            Some(Intent::Exit)
        );
    }

    #[test]
    fn fallback_menu_and_settings_are_bounded_on_tiny_screens() {
        let view = View::default();
        for page in [Page::Welcome, Page::Connection] {
            let mut input = AccountInput::with_main_menu(Endpoint::default());
            input.navigate(page);
            for size in [[640, 480], [320, 240], [1, 1], [0, 0]] {
                let frame = AccountUi::default().frame(size, &view, &input, f32::NAN);
                assert!(frame.hit_targets.len() <= 6);
                assert!(frame.commands.len() < 100);
                assert!(
                    frame
                        .hit_targets
                        .iter()
                        .all(|hit| hit.rect.intersect(frame.bounds) == hit.rect)
                );
                assert!(frame.hit_targets.iter().all(|hit| {
                    ![
                        "AccountButton",
                        "EQWebpageButton",
                        "HelpButton",
                        "QuickConnectButton",
                    ]
                    .contains(&hit.screen_id.as_str())
                }));
                if size[0] >= 320 {
                    let buttons: Vec<_> = frame
                        .hit_targets
                        .iter()
                        .filter(|hit| hit.kind == "AccountButton")
                        .collect();
                    assert!(buttons.iter().all(|hit| !hit.rect.is_empty()));
                    for (index, a) in buttons.iter().enumerate() {
                        for b in &buttons[index + 1..] {
                            assert!(a.rect.intersect(b.rect).is_empty());
                        }
                    }
                }
            }
        }
    }

    #[test]
    #[ignore = "requires original UI assets and GPU; no network or audio"]
    fn original_main_menu_and_connection_settings_at_two_scales() {
        let directory = openeq_assets::loader::default_client_dir().expect("original UI assets");
        let ui = AccountUi::load(&directory);
        let document = ui.login.as_ref().expect("login XML");
        for name in [
            "main",
            "MAIN_ConnectButton",
            "MAIN_OptionsButton",
            "MAIN_ExitButton",
        ] {
            assert!(document.definition(name).is_some(), "{name}");
        }
        let view = View::default();
        for (name, page) in [
            ("main-menu", Page::Welcome),
            ("connection-settings", Page::Connection),
        ] {
            let mut input = AccountInput::with_main_menu(Endpoint {
                host: "storage2.daeken.dev".into(),
                ..Default::default()
            });
            input.navigate(page);
            for (viewport, scale, suffix) in [
                ([800, 600], 1., "1x"),
                ([800, 600], 2., "2x"),
                ([320, 240], 1., "compact"),
            ] {
                let frame = ui.frame(viewport, &view, &input, 0.4);
                assert!(frame.commands.iter().any(
                |command| matches!(command, DrawCommand::Image { texture, .. } if texture.exists())
            ));
                assert!(frame.hit_targets.iter().all(|hit| {
                    ![
                        "AccountButton",
                        "EQWebpageButton",
                        "HelpButton",
                        "QuickConnectButton",
                    ]
                    .contains(&hit.screen_id.as_str())
                }));
                let mut renderer = openeq_render::Renderer::new_headless(
                    (viewport[0] as f32 * scale) as u32,
                    (viewport[1] as f32 * scale) as u32,
                )
                .unwrap();
                renderer.set_ui_scaled(&frame, scale);
                renderer.render_ui();
                let (width, height, pixels) = renderer.read_rgba().unwrap();
                assert!(
                    pixels
                        .chunks_exact(4)
                        .any(|pixel| pixel[0] > 80 && pixel[1] > 80)
                );
                if let Some(output) = std::env::var_os("OPENEQ_UI_CAPTURE_DIR") {
                    let path = std::path::PathBuf::from(output)
                        .join(format!("account-{name}-{suffix}.png"));
                    image::save_buffer(path, &pixels, width, height, image::ColorType::Rgba8)
                        .unwrap();
                }
            }
        }
    }
}
