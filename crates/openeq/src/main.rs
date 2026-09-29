//! OpenEQ: an EverQuest client built on Bevy's ECS with a custom `wgpu` renderer.
//!
//! Bevy provides the application loop, ECS, timing and input. Rendering is
//! entirely ours: the window's raw handle is handed to `openeq-render`, which
//! owns its own `wgpu` device, swapchain and deferred pipeline.
//!
//! ```text
//! openeq gfaydark
//! openeq akanon --dir /path/to/EverQuest --pos 100,-200,20
//! ```

use openeq::{chat, hud, live, loading, loading_ui, movement, zone_loading};
use openeq_net::session::ConnectionConfig;
use openeq_render::actors::ActorRenderer;
use std::path::PathBuf;

use bevy::ecs::system::NonSendMarker;
use bevy::input::{
    ButtonState,
    keyboard::KeyboardInput,
    mouse::{AccumulatedMouseMotion, MouseWheel},
};
use bevy::prelude::*;
use bevy::window::{CursorGrabMode, CursorOptions, Ime, PrimaryWindow, WindowFocused};
use bevy::winit::{WINIT_WINDOWS, WinitSettings};
use openeq_assets::loader;
use openeq_render::Renderer;
use openeq_render::scene::{Camera, GpuScene};
use raw_window_handle::{HasDisplayHandle, HasWindowHandle};

/// How fast the free-fly camera moves, in EverQuest units per second.
const MOVE_SPEED: f32 = 320.0;
const RUN_MULTIPLIER: f32 = 4.0;
/// Mouse sensitivity, in radians per pixel.
const LOOK_SENSITIVITY: f32 = 0.0035;

#[derive(Resource)]
struct Options {
    zone: String,
    dir: PathBuf,
    position: Option<[f32; 3]>,
    connection: Option<ConnectionConfig>,
}

/// Main-thread presentation, with destination assets prepared by a worker.
#[derive(Resource)]
struct Runtime {
    instance: Option<wgpu::Instance>,
    renderer: Option<Renderer>,
    scene: Option<GpuScene>,
    camera: Camera,
    size: (u32, u32),
    live: Option<live::LiveWorld>,
    actors: Option<ActorRenderer>,
    started: std::time::Instant,
    hud: Option<hud::Hud>,
    ui_frame: openeq_ui::UiFrame,
    chat_link_hits: Vec<openeq_ui::HitTarget>,
    atmosphere_zone: Option<(String, u8)>,
    moving: bool,
    collision: Option<openeq_assets::collision::CollisionWorld>,
    fly: bool,
    ground_motion: movement::GroundMotion,
    interaction: openeq::interaction::Interaction,
    loaded_zone: Option<String>,
    loaded_destination: Option<loading::Destination>,
    loading_destination: Option<loading::Destination>,
    loading_job: Option<loading::Job<zone_loading::PreparedZone>>,
    loading_error: Option<String>,
    client_job: Option<loading::Job<zone_loading::ClientData>>,
    zone_map: Option<openeq::map::ZoneMap>,
    map_state: openeq::map::MapState,
    map_open: bool,
    third_person: bool,
    doors: Option<openeq_render::doors::DoorRenderer>,
}

impl Runtime {
    fn world_ready(&self) -> bool {
        self.scene.is_some()
            && self.loading_error.is_none()
            && self.loading_job.is_none()
            && self.live.as_ref().is_none_or(|live| {
                live.ready
                    && live.error.is_none()
                    && self.loaded_destination.as_ref().is_some_and(|destination| {
                        destination.generation == live.zone_generation
                            && live
                                .environment
                                .as_ref()
                                .is_some_and(|env| env.short_name == destination.zone)
                    })
            })
    }

    fn new(camera: Camera) -> Self {
        Self {
            instance: None,
            renderer: None,
            scene: None,
            camera,
            size: (0, 0),
            live: None,
            actors: None,
            started: std::time::Instant::now(),
            hud: None,
            ui_frame: openeq_ui::UiFrame::default(),
            chat_link_hits: Vec::new(),
            atmosphere_zone: None,
            moving: false,
            collision: None,
            fly: true,
            ground_motion: movement::GroundMotion::default(),
            interaction: openeq::interaction::Interaction::default(),
            loaded_zone: None,
            loaded_destination: None,
            loading_destination: None,
            loading_job: None,
            loading_error: None,
            client_job: None,
            zone_map: None,
            map_state: openeq::map::MapState::default(),
            map_open: false,
            third_person: false,
            doors: None,
        }
    }
}

fn main() -> AppExit {
    let options = match parse_args() {
        Ok(options) => options,
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(2);
        }
    };

    let camera = Camera {
        position: options.position.unwrap_or([0.0, 0.0, 100.0]),
        ..Default::default()
    };

    let mut runtime = Runtime::new(camera);
    if options.connection.is_some() {
        runtime.client_job = Some(zone_loading::start_client(options.dir.clone()));
        runtime.fly = false;
    }

    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: format!("OpenEQ - {}", options.zone),
                ..default()
            }),
            ..default()
        }))
        .insert_resource(WinitSettings::game())
        .insert_resource(runtime)
        .insert_resource(options)
        // Capture runs first so the camera sees this frame's grab state.
        .add_systems(
            Update,
            (
                handle_gameplay_input,
                handle_targeting,
                handle_cursor_capture,
                update_camera,
            )
                .chain(),
        )
        .add_systems(Last, render_frame)
        .run()
}

fn parse_args() -> anyhow::Result<Options> {
    let mut args = std::env::args().skip(1);
    let mut zone = None;
    let mut dir = None;
    let mut position = None;
    let mut connection = None;

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--connect" => {
                let path = args
                    .next()
                    .ok_or_else(|| anyhow::anyhow!("--connect needs a config file"))?;
                connection = Some(ConnectionConfig::load(std::path::Path::new(&path))?);
            }
            "--dir" => dir = args.next().map(PathBuf::from),
            "--pos" => {
                position = args.next().and_then(|value| {
                    let parts: Vec<f32> = value
                        .split(',')
                        .filter_map(|p| p.trim().parse().ok())
                        .collect();
                    (parts.len() == 3).then(|| [parts[0], parts[1], parts[2]])
                })
            }
            "-h" | "--help" => {
                println!("usage: openeq [zone] [--dir DIR] [--pos X,Y,Z] [--connect CONFIG]");
                std::process::exit(0);
            }
            other if zone.is_none() => zone = Some(other.to_string()),
            other => anyhow::bail!("unrecognised argument {other}"),
        }
    }

    let zone = zone
        .or_else(|| connection.as_ref().map(|_| "poknowledge".to_string()))
        .unwrap_or_else(|| {
            eprintln!("usage: openeq [zone] [--dir DIR] [--pos X,Y,Z] [--connect CONFIG]");
            std::process::exit(2);
        });
    let dir = match dir.or_else(loader::default_client_dir) {
        Some(dir) => dir,
        None => anyhow::bail!("no client directory; pass --dir"),
    };
    Ok(Options {
        zone,
        dir,
        position,
        connection,
    })
}

/// WASD to move, mouse to look, shift to run, space/ctrl to rise and sink.
///
/// Mouse look only applies while the cursor is captured, so moving the mouse
/// around the desktop does not spin the view.
fn update_camera(
    keys: Res<ButtonInput<KeyCode>>,
    motion: Res<AccumulatedMouseMotion>,
    time: Res<Time>,
    cursors: Query<&CursorOptions, With<PrimaryWindow>>,
    mut runtime: ResMut<Runtime>,
) {
    runtime.moving = false;
    if !runtime.world_ready() {
        return;
    }
    let online = runtime.live.is_some();
    let controls = !runtime.interaction.editor.active
        && !runtime.interaction.controls_blocked
        && runtime
            .live
            .as_ref()
            .is_none_or(|live| live.ready && live.error.is_none());
    if online && controls && keys.just_pressed(KeyCode::KeyF) {
        runtime.fly = !runtime.fly;
        runtime.ground_motion = movement::GroundMotion::default();
    }
    let mut camera = runtime.camera;

    let captured = cursors.single().map(is_captured).unwrap_or(false);
    let delta = if captured && controls {
        motion.delta
    } else {
        Vec2::ZERO
    };
    if delta != Vec2::ZERO {
        // Yaw increases anticlockwise in EverQuest space, so moving the mouse
        // right (positive x) turns right by adding.
        camera.yaw += delta.x * LOOK_SENSITIVITY;
        camera.pitch = (camera.pitch - delta.y * LOOK_SENSITIVITY).clamp(
            -std::f32::consts::FRAC_PI_2 + 0.01,
            std::f32::consts::FRAC_PI_2 - 0.01,
        );
    }

    let mut forward = 0.0f32;
    let mut strafe = 0.0f32;
    let mut lift = 0.0f32;
    if controls && keys.pressed(KeyCode::KeyW) {
        forward += 1.0;
    }
    if controls && keys.pressed(KeyCode::KeyS) {
        forward -= 1.0;
    }
    if controls && keys.pressed(KeyCode::KeyD) {
        strafe += 1.0;
    }
    if controls && keys.pressed(KeyCode::KeyA) {
        strafe -= 1.0;
    }
    if controls && keys.pressed(KeyCode::Space) {
        lift += 1.0;
    }
    if controls && keys.pressed(KeyCode::ControlLeft) {
        lift -= 1.0;
    }
    let horizontal_length = (forward * forward + strafe * strafe).sqrt().max(1.);
    forward /= horizontal_length;
    strafe /= horizontal_length;
    let dt = time.delta_secs().min(0.25);

    // Movement is applied in EverQuest space so the camera maths stays simple.
    let (sin_yaw, cos_yaw) = camera.yaw.sin_cos();
    let heading = [sin_yaw, cos_yaw];
    let right = [cos_yaw, -sin_yaw];
    let speed = if online { 40.0 } else { MOVE_SPEED }
        * if keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight) {
            if online { 1.5 } else { RUN_MULTIPLIER }
        } else {
            1.0
        };

    let velocity_xy = [
        (heading[0] * forward + right[0] * strafe) * speed,
        (heading[1] * forward + right[1] * strafe) * speed,
    ];
    let mut position = camera.position;
    position[0] += velocity_xy[0] * dt;
    position[1] += velocity_xy[1] * dt;
    let feet = [
        camera.position[0],
        camera.position[1],
        camera.position[2] - 6.,
    ];
    let jump = controls && keys.just_pressed(KeyCode::Space);
    let allow_carry = !runtime.fly && runtime.ground_motion.velocity_z <= 0. && !jump;
    let platform_displacement = runtime.doors.as_mut().map_or([0.; 3], |doors| {
        doors.take_platform_displacement(feet, allow_carry)
    });
    if runtime.fly || runtime.collision.is_none() {
        position[2] += lift * speed * dt;
    } else {
        // Carry only supported riders, and resolve against static ceilings
        // before ordinary gravity/walking uses the lift's new collision pose.
        let feet = runtime.collision.as_ref().unwrap().move_player(
            feet,
            platform_displacement,
            1.,
            6.,
            0.,
        );
        let mut motion = std::mem::take(&mut runtime.ground_motion);
        let moved = motion.step_with_dynamic(
            runtime.collision.as_ref().unwrap(),
            runtime.doors.as_ref().map(|doors| doors.collision_world()),
            feet,
            velocity_xy,
            jump,
            dt,
        );
        runtime.ground_motion = motion;
        position = [moved[0], moved[1], moved[2] + 6.];
    }
    runtime.moving = forward != 0. || strafe != 0.;
    camera.position = position;
    runtime.camera = camera;
}

/// Click to capture the cursor, `Escape` to release, `Escape` again to quit.
///
/// Losing focus releases the capture as well, so clicking away to another
/// window never leaves the pointer trapped.
fn handle_cursor_capture(
    mut cursors: Query<(Entity, &mut CursorOptions), With<PrimaryWindow>>,
    mouse: Res<ButtonInput<MouseButton>>,
    keys: Res<ButtonInput<KeyCode>>,
    mut focus: MessageReader<WindowFocused>,
    mut exit: MessageWriter<AppExit>,
    runtime: Res<Runtime>,
    windows: Query<&Window, With<PrimaryWindow>>,
) {
    let Ok((entity, mut cursor)) = cursors.single_mut() else {
        return;
    };

    if !runtime.world_ready() {
        release_cursor(&mut cursor);
        focus.clear();
        if keys.just_pressed(KeyCode::Escape) {
            exit.write(AppExit::Success);
        }
        return;
    }
    let mut captured = is_captured(&cursor);
    for event in focus.read() {
        if event.window == entity && !event.focused && captured {
            release_cursor(&mut cursor);
            captured = false;
        }
    }

    if runtime.interaction.editor.active || runtime.interaction.escape_handled {
        return;
    }
    let ui_hit = windows
        .single()
        .ok()
        .and_then(|window| window.cursor_position().map(|p| [p.x, p.y]))
        .is_some_and(|p| runtime.ui_frame.hit_test(p).is_some());

    if (mouse.just_pressed(MouseButton::Right)
        || (runtime.live.is_none() && mouse.just_pressed(MouseButton::Left)))
        && !captured
        && !ui_hit
    {
        cursor.visible = false;
        cursor.grab_mode = CursorGrabMode::Locked;
        tracing::debug!("cursor captured; escape releases it");
        return;
    }

    if keys.just_pressed(KeyCode::Escape) {
        if captured {
            release_cursor(&mut cursor);
            tracing::debug!("cursor released; escape again quits");
        } else {
            exit.write(AppExit::Success);
        }
    }
}

fn is_captured(options: &CursorOptions) -> bool {
    options.grab_mode != CursorGrabMode::None
}

fn handle_gameplay_input(
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    (mut keyboard, mut ime, mut wheel): (
        MessageReader<KeyboardInput>,
        MessageReader<Ime>,
        MessageReader<MouseWheel>,
    ),
    mut windows: Query<(Entity, &mut Window, &mut CursorOptions), With<PrimaryWindow>>,
    mut runtime: ResMut<Runtime>,
    mut exit: MessageWriter<AppExit>,
) {
    let Ok((window_id, mut window, mut cursor)) = windows.single_mut() else {
        return;
    };
    if let Some(live) = runtime.live.as_mut() {
        live.poll();
    }
    if !runtime.world_ready() {
        keyboard.clear();
        ime.clear();
        wheel.clear();
        window.ime_enabled = false;
        release_cursor(&mut cursor);
        runtime.interaction.editor.cancel();
        runtime.interaction.ime_composing = false;
        runtime.interaction.escape_handled = false;
        runtime.interaction.controls_blocked = true;
        return;
    }
    let camera_position = runtime.camera.position;
    let Runtime {
        live: Some(live),
        interaction,
        ui_frame,
        chat_link_hits,
        zone_map,
        map_state,
        map_open,
        third_person,
        ..
    } = &mut *runtime
    else {
        keyboard.clear();
        ime.clear();
        wheel.clear();
        return;
    };
    if let Some(environment) = &live.environment {
        let title = format!("OpenEQ - {}", environment.short_name);
        if window.title != title {
            window.title = title;
        }
    }
    interaction.controls_blocked = interaction.editor.active;
    interaction.escape_handled = false;
    interaction.pointer = window.cursor_position().map(|p| [p.x, p.y]);
    let command_modifier = keys.pressed(KeyCode::ControlLeft)
        || keys.pressed(KeyCode::ControlRight)
        || keys.pressed(KeyCode::SuperLeft)
        || keys.pressed(KeyCode::SuperRight);
    for event in keyboard
        .read()
        .filter(|event| event.window == window_id && event.state == ButtonState::Pressed)
    {
        if !interaction.editor.active {
            if event.key_code == KeyCode::Enter || event.key_code == KeyCode::NumpadEnter {
                interaction.editor.open("");
            } else if event.text.as_deref() == Some("/") && !command_modifier {
                interaction.editor.open("/");
            } else {
                continue;
            }
            release_cursor(&mut cursor);
            interaction.controls_blocked = true;
            continue;
        }
        interaction.controls_blocked = true;
        match event.key_code {
            KeyCode::Enter | KeyCode::NumpadEnter if !interaction.ime_composing => {
                if let Some(text) = interaction.editor.submit()
                    && interaction.submit(&text, live, camera_position)
                {
                    exit.write(AppExit::Success);
                }
            }
            KeyCode::Escape => {
                interaction.editor.cancel();
                interaction.ime_composing = false;
                interaction.escape_handled = true;
            }
            KeyCode::Backspace if !interaction.ime_composing => {
                interaction.editor.backspace(command_modifier)
            }
            KeyCode::Delete if !interaction.ime_composing => interaction.editor.delete(),
            KeyCode::ArrowLeft if !interaction.ime_composing => interaction.editor.left(),
            KeyCode::ArrowRight if !interaction.ime_composing => interaction.editor.right(),
            KeyCode::Home => interaction.editor.cursor = 0,
            KeyCode::End => interaction.editor.cursor = interaction.editor.text.len(),
            KeyCode::ArrowUp if !interaction.ime_composing => interaction.editor.history(true),
            KeyCode::ArrowDown if !interaction.ime_composing => interaction.editor.history(false),
            KeyCode::PageUp => {
                interaction.chat_scroll = interaction.chat_scroll.saturating_add(8).min(5000)
            }
            KeyCode::PageDown => {
                interaction.chat_scroll = interaction.chat_scroll.saturating_sub(8)
            }
            KeyCode::KeyU if command_modifier => {
                interaction.editor.text.clear();
                interaction.editor.cursor = 0;
            }
            _ if !command_modifier && !interaction.ime_composing => {
                if let Some(text) = &event.text {
                    interaction.editor.insert(text);
                }
            }
            _ => {}
        }
    }
    for event in ime.read() {
        if !interaction.editor.active {
            continue;
        }
        match event {
            Ime::Preedit { window, value, .. } if *window == window_id => {
                interaction.editor.preedit.clone_from(value);
                interaction.ime_composing = !value.is_empty();
            }
            Ime::Commit { window, value } if *window == window_id => {
                interaction.editor.insert(value);
                interaction.editor.preedit.clear();
                interaction.ime_composing = false;
            }
            Ime::Disabled { window } if *window == window_id => {
                interaction.editor.preedit.clear();
                interaction.ime_composing = false;
            }
            _ => {}
        }
    }
    window.ime_enabled = interaction.editor.active;
    if !interaction.controls_blocked && !interaction.editor.active {
        if keys.just_pressed(KeyCode::KeyM) {
            if zone_map.is_some() {
                *map_open = !*map_open;
            } else {
                live.game
                    .notice("No original map file was found for this zone.");
            }
        }
        if keys.just_pressed(KeyCode::F9) {
            *third_person = !*third_person;
        }
        if keys.just_pressed(KeyCode::KeyB) {
            interaction.spellbook_open = !interaction.spellbook_open;
        }
        if keys.just_pressed(KeyCode::F1) {
            live.set_target(live.own_id);
        }
        if keys.pressed(KeyCode::AltLeft) || keys.pressed(KeyCode::AltRight) {
            for (gem, key) in [
                KeyCode::Digit1,
                KeyCode::Digit2,
                KeyCode::Digit3,
                KeyCode::Digit4,
                KeyCode::Digit5,
                KeyCode::Digit6,
                KeyCode::Digit7,
                KeyCode::Digit8,
                KeyCode::Digit9,
                KeyCode::Digit0,
            ]
            .into_iter()
            .enumerate()
            {
                if keys.just_pressed(key) {
                    interaction.cast(gem as u8, live);
                }
            }
        }
        if keys.just_pressed(KeyCode::KeyE) {
            let closest = live
                .doors
                .values()
                .map(|door| {
                    (
                        door.id,
                        door.position
                            .iter()
                            .zip(camera_position)
                            .map(|(a, b)| (a - b).powi(2))
                            .sum::<f32>(),
                    )
                })
                .filter(|(_, distance)| *distance < 25. * 25.)
                .min_by(|a, b| a.1.total_cmp(&b.1))
                .map(|entry| entry.0);
            if let (Some(door_id), Some(player_id)) = (closest, live.own_id) {
                live.command(openeq_net::gameplay::Command::ClickDoor { door_id, player_id });
            } else {
                live.game.notice("No door or portal is within reach.");
            }
        }
        for (key, action) in [
            (KeyCode::KeyI, chat::Action::Inventory),
            (KeyCode::KeyR, chat::Action::UseTarget),
            (KeyCode::KeyQ, chat::Action::Attack(None)),
            (KeyCode::KeyH, chat::Action::Hail),
            (KeyCode::KeyX, chat::Action::Sit(!live.game.sitting)),
            (KeyCode::KeyL, chat::Action::Loot),
            (KeyCode::KeyC, chat::Action::Consider),
            (KeyCode::KeyV, chat::Action::Assist(None)),
        ] {
            if keys.just_pressed(key) {
                interaction.action(action, live, camera_position);
            }
        }
        if live.game.sitting
            && [
                KeyCode::KeyW,
                KeyCode::KeyA,
                KeyCode::KeyS,
                KeyCode::KeyD,
                KeyCode::Space,
            ]
            .iter()
            .any(|key| keys.just_pressed(*key))
        {
            interaction.action(chat::Action::Sit(false), live, camera_position);
        }
        if keys.just_pressed(KeyCode::Escape) && !is_captured(&cursor) {
            if live.game.casting.is_some()
                || live
                    .game
                    .cast_pending_until
                    .is_some_and(|until| until > std::time::Instant::now())
            {
                live.command(openeq_net::gameplay::Command::InterruptSpell);
                interaction.escape_handled = true;
            } else if interaction.inspected_item.is_some() || live.game.linked_item.is_some() {
                interaction.close_window("inspect", live);
                interaction.escape_handled = true;
            } else if live.game.trade.session.is_some() {
                interaction.close_window("trade", live);
                interaction.escape_handled = true;
            } else if live.game.commerce.merchant.is_some() {
                interaction.close_window("merchant", live);
                interaction.escape_handled = true;
            } else if live.game.commerce.bank.is_some() {
                interaction.close_window("bank", live);
                interaction.escape_handled = true;
            } else if interaction.spellbook_open {
                interaction.spellbook_open = false;
                interaction.escape_handled = true;
            } else if live.game.loot.is_some() {
                interaction.close_window("loot", live);
                interaction.escape_handled = true;
            } else if !interaction.open_bags.is_empty() {
                interaction.open_bags.clear();
                interaction.escape_handled = true;
            } else if interaction.inventory_open {
                interaction.inventory_open = false;
                interaction.escape_handled = true;
            } else if *map_open {
                *map_open = false;
                interaction.escape_handled = true;
            }
        }
    }
    if !is_captured(&cursor)
        && let Some(point) = interaction.pointer
    {
        if mouse.pressed(MouseButton::Left)
            && let Some((window_name, offset)) = &interaction.drag
        {
            let origin = [
                (point[0] - offset[0]).clamp(0., (window.width() - 40.).max(0.)),
                (point[1] - offset[1]).clamp(0., (window.height() - 30.).max(0.)),
            ];
            if window_name == "map" {
                map_state.rect.x = origin[0];
                map_state.rect.y = origin[1];
            } else {
                interaction
                    .window_positions
                    .insert(window_name.clone(), origin);
            }
        }
        if mouse.just_released(MouseButton::Left) {
            interaction.drag = None;
        }
        if let Some(hit) = ui_frame.hit_test(point) {
            if hit.item == "game:chat_log"
                && mouse.just_pressed(MouseButton::Left)
                && let Some(link) = chat_link_hits
                    .iter()
                    .rev()
                    .find(|hit| hit.rect.contains(point))
                && let Some(action) = hud::UiAction::from_hit(link)
            {
                interaction.ui_action(action, false, false, live, camera_position);
            }
            if hit.item == "commerce:stock_list" || hit.item.starts_with("commerce:stock:") {
                for event in wheel.read() {
                    interaction.ui_action(
                        hud::UiAction::Commerce(openeq::commerce_ui::CommerceAction::Scroll(
                            (-event.y * 3.).round() as i32,
                        )),
                        false,
                        false,
                        live,
                        camera_position,
                    );
                }
            }
            if let Some(action) = openeq::map::MapAction::from_hit(hit) {
                use openeq::map::{MapAction, MapMarker};
                if mouse.just_pressed(MouseButton::Left) {
                    match action {
                        MapAction::Close => *map_open = false,
                        MapAction::ZoomIn => {
                            map_state.units_per_pixel = (map_state.units_per_pixel / 1.4).max(0.25)
                        }
                        MapAction::ZoomOut => {
                            map_state.units_per_pixel = (map_state.units_per_pixel * 1.4).min(128.)
                        }
                        MapAction::Recenter => map_state.center = None,
                        MapAction::BeginDrag => {
                            interaction.drag = Some((
                                "map".into(),
                                [point[0] - map_state.rect.x, point[1] - map_state.rect.y],
                            ))
                        }
                        MapAction::Canvas => {
                            if let Some(position) = map_state.world_at(point) {
                                map_state.waypoints = vec![MapMarker {
                                    position,
                                    label: "Waypoint".into(),
                                    ..Default::default()
                                }];
                            }
                        }
                        MapAction::Label(index) => {
                            if let Some(label) =
                                zone_map.as_ref().and_then(|map| map.labels.get(index))
                            {
                                map_state.waypoints = vec![MapMarker {
                                    position: label.position,
                                    label: label.text.clone(),
                                    ..Default::default()
                                }];
                            }
                        }
                    }
                }
                if mouse.just_pressed(MouseButton::Right) {
                    map_state.waypoints.clear();
                }
                for event in wheel.read() {
                    map_state.units_per_pixel =
                        (map_state.units_per_pixel * 1.2f32.powf(-event.y)).clamp(0.25, 128.);
                }
            }
            if (mouse.just_pressed(MouseButton::Left) || mouse.just_pressed(MouseButton::Right))
                && let Some(action) = hud::UiAction::from_hit(hit)
            {
                if let hud::UiAction::BeginWindowDrag(name) = action {
                    if mouse.just_pressed(MouseButton::Left) {
                        interaction.drag =
                            Some((name, [point[0] - hit.rect.x, point[1] - hit.rect.y]));
                    }
                } else {
                    interaction.ui_action(
                        action,
                        mouse.just_pressed(MouseButton::Right),
                        keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight),
                        live,
                        camera_position,
                    );
                }
            }
            if hit.item == "game:chat_log" || hit.item == "game:chat_input" {
                for event in wheel.read() {
                    let rows = (event.y.abs() * 3.).ceil() as usize;
                    if event.y > 0. {
                        interaction.chat_scroll =
                            interaction.chat_scroll.saturating_add(rows).min(5000);
                    } else {
                        interaction.chat_scroll = interaction.chat_scroll.saturating_sub(rows);
                    }
                }
            }
        }
    }
    wheel.clear();
    interaction.tick(live);
}

fn release_cursor(cursor: &mut CursorOptions) {
    cursor.visible = true;
    cursor.grab_mode = CursorGrabMode::None;
}

/// Builds the renderer on the first frame, then draws.
fn render_frame(
    windows: Query<Entity, With<PrimaryWindow>>,
    window: Query<&Window, With<PrimaryWindow>>,
    mut runtime: ResMut<Runtime>,
    options: Res<Options>,
    // Accessing the window handle requires the thread that owns the event loop.
    _main_thread: NonSendMarker,
) {
    let Ok(window_entity) = windows.single() else {
        return;
    };

    if let Some(live) = runtime.live.as_mut() {
        live.poll();
    }
    if runtime.renderer.is_none() {
        let window_ready =
            WINIT_WINDOWS.with(|cell| cell.borrow().get_window(window_entity).is_some());
        if !window_ready {
            return;
        }
        if let Err(error) = initialise(&mut runtime, window_entity) {
            tracing::error!(%error, "renderer or asset initialization failed");
            std::process::exit(1);
        }
    }

    let Ok(window) = window.single() else {
        return;
    };
    let size = (window.physical_width(), window.physical_height());
    let ui_size = [
        window.width().round() as u32,
        window.height().round() as u32,
    ];
    if size.0 > 0
        && size.1 > 0
        && size != runtime.size
        && let Some(renderer) = runtime.renderer.as_mut()
    {
        renderer.resize(size.0, size.1);
        runtime.size = size;
        tracing::debug!(width = size.0, height = size.1, "resized");
    }

    if let Some(live) = runtime.live.as_mut() {
        live.poll();
        if let Some(position) = live.initial_position.take() {
            runtime.camera.position = [position.x, position.y, position.z + 3.];
            runtime.camera.yaw = position.heading * std::f32::consts::TAU / 512.;
            runtime.ground_motion = movement::GroundMotion::default();
        }
    }
    if let Some(mut renderer) = runtime.renderer.take() {
        prepare_world(&mut runtime, &mut renderer, &options);
        if !runtime.world_ready() {
            draw_loading_screen(
                &mut runtime,
                &mut renderer,
                &options,
                ui_size,
                window.scale_factor(),
            );
            runtime.renderer = Some(renderer);
            return;
        }
        let player_camera = runtime.camera;
        let camera = view_camera(&runtime);
        if let Some(live) = runtime.live.as_ref() {
            live.camera_position(&player_camera, runtime.moving);
        }
        if let Some(live) = &runtime.live
            && let Some(env) = &live.environment
        {
            let desired = (env.short_name.clone(), live.hour);
            if runtime.atmosphere_zone.as_ref() != Some(&desired) {
                let settings = zone_environment(env);
                let sky = openeq_assets::environment::load_sky(
                    &options.dir,
                    &env.short_name,
                    (live.hour as f32 + live.minute as f32 / 60.) / 24.,
                )
                .ok();
                renderer.set_environment(settings, sky.as_ref());
                runtime.atmosphere_zone = Some(desired);
            }
        }
        let states = runtime.live.as_ref().map(|live| {
            live.actor_states_with_terrain(
                camera.position,
                runtime
                    .third_person
                    .then_some((&player_camera, runtime.moving)),
                runtime.collision.as_ref().map(|world| {
                    (
                        world,
                        runtime.doors.as_ref().map(|doors| doors.collision_world()),
                    )
                }),
            )
        });
        let elapsed = runtime.started.elapsed().as_secs_f32();
        if let (Some(actors), Some(states)) = (runtime.actors.as_mut(), states) {
            actors.update(&renderer, &states, elapsed);
        }
        let doors = runtime.live.as_ref().map(door_states);
        if let (Some(renderer_doors), Some(states)) = (runtime.doors.as_mut(), doors) {
            renderer_doors.update(&renderer, &states, elapsed);
        }
        if let (Some(hud), Some(live)) = (&runtime.hud, &runtime.live) {
            let player = live.own_id.and_then(|id| live.entities.get(&id));
            let target =
                live.target
                    .and_then(|id| live.entities.get(&id))
                    .map(|e| hud::HudTarget {
                        name: display_name(&e.spawn.name),
                        hp: e.spawn.hp_percent as f32 / 100.,
                        level: e.spawn.level,
                    });
            let state = hud::HudState {
                character: live.character.clone(),
                player_level: player.map_or(0, |e| e.spawn.level),
                hp: player.map_or(0., |e| e.spawn.hp_percent as f32 / 100.),
                mana: live.game.mana.fraction(),
                endurance: live.game.endurance.fraction(),
                target,
                status: live.error.clone().unwrap_or_else(|| {
                    if live.ready {
                        "Connected • Enter chat • I inventory • Q attack • /help".into()
                    } else {
                        "Connecting to world…".into()
                    }
                }),
                entities: live.entities.len(),
                movement_updates: live.moves,
            };
            let game = runtime.interaction.view(live);
            let mut frame = hud.gameplay_frame(ui_size, &state, &game);
            if let Some(actors) = &runtime.actors {
                add_nameplates(&mut frame, live, actors, &camera, ui_size);
            }
            if runtime.map_open {
                let mut map_state = runtime.map_state.clone();
                map_state.player_position = [
                    player_camera.position[0],
                    player_camera.position[1],
                    player_camera.position[2] - 3.,
                ];
                map_state.heading = player_camera.yaw;
                map_state.pointer = runtime.interaction.pointer;
                map_state.target =
                    live.target
                        .and_then(|id| live.entities.get(&id))
                        .map(|entity| openeq::map::MapMarker {
                            position: entity.position(std::time::Instant::now()),
                            label: display_name(&entity.spawn.name),
                            color: [255, 120, 100, 255],
                        });
                if let Some(map) = &runtime.zone_map {
                    let mut overlay = map.frame(ui_size, &map_state);
                    frame.commands.append(&mut overlay.commands);
                    frame.hit_targets.append(&mut overlay.hit_targets);
                }
                runtime.map_state = map_state;
            }
            renderer.set_ui_scaled(&frame, window.scale_factor());
            runtime.chat_link_hits = renderer.ui_link_hits().to_vec();
            runtime.ui_frame = frame;
        }
        if let Some(scene) = runtime.scene.as_ref() {
            let mut actors = runtime
                .actors
                .as_ref()
                .map(|a| a.draws())
                .unwrap_or_default();
            if let Some(doors) = &runtime.doors {
                actors.extend(doors.draws());
            }
            renderer.render_with_actors(scene, &camera, &actors);
        }
        runtime.renderer = Some(renderer);
    }
}

fn initialise(runtime: &mut Runtime, window_entity: Entity) -> anyhow::Result<()> {
    // The window lives in bevy_winit's thread-local; take its raw handles.
    let (window_handle, display_handle) = WINIT_WINDOWS.with(|cell| {
        let windows = cell.borrow();
        let wrapper = windows
            .get_window(window_entity)
            .ok_or_else(|| anyhow::anyhow!("the primary window has no winit window yet"))?;
        Ok::<_, anyhow::Error>((
            wrapper.window_handle()?.as_raw(),
            wrapper.display_handle()?.as_raw(),
        ))
    })?;

    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
    // SAFETY: the winit window outlives the surface, which the renderer owns and
    // drops before the window closes.
    let surface = unsafe {
        instance.create_surface_unsafe(wgpu::SurfaceTargetUnsafe::RawHandle {
            raw_display_handle: Some(display_handle),
            raw_window_handle: window_handle,
        })?
    };

    let (width, height) = if runtime.size.0 > 0 {
        runtime.size
    } else {
        (1280, 720)
    };
    let renderer = pollster::block_on(Renderer::new_surface(&instance, surface, width, height))?;
    runtime.size = (width, height);
    runtime.instance = Some(instance);
    runtime.renderer = Some(renderer);
    Ok(())
}

fn desired_destination(runtime: &Runtime, options: &Options) -> Option<loading::Destination> {
    if let Some(live) = &runtime.live {
        if !live.ready || live.error.is_some() {
            return None;
        }
        live.environment.as_ref().map(|env| loading::Destination {
            zone: env.short_name.clone(),
            generation: live.zone_generation,
        })
    } else {
        Some(loading::Destination {
            zone: options.zone.clone(),
            generation: 0,
        })
    }
}

/// Polling, cancellation, and installation are deliberately cheap: all asset
/// decoding, collision building, and model/texture uploads run in the job.
fn prepare_world(runtime: &mut Runtime, renderer: &mut Renderer, options: &Options) {
    if let Some(config) = &options.connection
        && runtime.live.is_none()
    {
        if let Some(result) = runtime.client_job.as_mut().and_then(|job| job.poll()) {
            runtime.client_job = None;
            match result {
                Ok(client) => {
                    let mut live = live::LiveWorld::start(config.clone());
                    live.game.strings = client.strings;
                    live.game.spell_catalog = client.spells;
                    runtime.hud = client.hud;
                    runtime.live = Some(live);
                }
                Err(error) => runtime.loading_error = Some(error),
            }
        }
        return;
    }
    let desired = desired_destination(runtime, options);
    if desired != runtime.loading_destination {
        runtime.loading_job = None; // Cancels at the worker's next checkpoint.
        runtime.loading_error = None;
        runtime.loading_destination.clone_from(&desired);
    }
    let Some(destination) = desired else {
        return;
    };
    if runtime.loaded_destination.as_ref() == Some(&destination) {
        return;
    }
    if runtime.loading_job.is_none() && runtime.loading_error.is_none() {
        let (actors, doors, time_of_day) = runtime.live.as_ref().map_or_else(
            || (Vec::new(), Vec::new(), 0.5),
            |live| {
                (
                    live.actor_states(runtime.camera.position, None),
                    door_states(live),
                    (live.hour as f32 + live.minute as f32 / 60.) / 24.,
                )
            },
        );
        runtime.loading_job = Some(zone_loading::start(
            zone_loading::Request {
                dir: options.dir.clone(),
                zone: destination.zone.clone(),
                online: runtime.live.is_some(),
                time_of_day,
                actors,
                doors,
            },
            renderer.upload_context(),
        ));
    }
    let Some(result) = runtime.loading_job.as_mut().and_then(|job| job.poll()) else {
        return;
    };
    runtime.loading_job = None;
    match result {
        Err(error) => {
            tracing::error!(%error, zone = %destination.zone, "zone asset loading failed");
            runtime.loading_error = Some(error);
        }
        Ok(prepared) => {
            // Offline viewing and a missing XML skin have no HUD to replace
            // the opaque loading overlay on the first world frame.
            runtime.ui_frame = openeq_ui::UiFrame::default();
            runtime.chat_link_hits.clear();
            renderer.set_ui(&runtime.ui_frame);
            renderer.set_scene(&prepared.scene);
            let settings = runtime
                .live
                .as_ref()
                .and_then(|live| live.environment.as_ref())
                .map(zone_environment)
                .unwrap_or_else(|| {
                    openeq_render::environment::EnvironmentSettings::for_zone(&destination.zone)
                });
            renderer.set_environment(settings, prepared.sky.as_ref());
            if runtime.live.is_none() && runtime.scene.is_none() && options.position.is_none() {
                let center = (prepared.scene.bounds_min + prepared.scene.bounds_max) * 0.5;
                runtime.camera.position = [center.x, center.y, center.z + 80.];
            }
            runtime.scene = Some(prepared.scene);
            runtime.collision = prepared.collision;
            runtime.actors = prepared.actors;
            runtime.doors = prepared.doors;
            runtime.zone_map = prepared.map;
            runtime.map_state.waypoints.clear();
            runtime.map_state.center = None;
            runtime.map_open = false;
            runtime.ground_motion = movement::GroundMotion::default();
            runtime.loaded_zone = Some(destination.zone.clone());
            runtime.loaded_destination = Some(destination);
            runtime.atmosphere_zone = runtime
                .live
                .as_ref()
                .map(|live| (runtime.loaded_zone.clone().unwrap(), live.hour));
            runtime.interaction.controls_blocked = false;
        }
    }
}

fn zone_environment(
    env: &openeq_net::zone::Environment,
) -> openeq_render::environment::EnvironmentSettings {
    openeq_render::environment::EnvironmentSettings {
        fog_color: env.fog_color[0],
        fog_start: env.fog_start[0],
        fog_end: env.fog_end[0],
        fog_density: env.fog_density,
        fog_enabled: env.fog_end[0] > env.fog_start[0],
        sky_enabled: !matches!(env.zone_type, 0 | 3 | 4) && env.sky != 0,
        ..Default::default()
    }
}

fn door_states(live: &live::LiveWorld) -> Vec<openeq_render::doors::DoorState> {
    live.doors
        .values()
        .map(|door| openeq_render::doors::DoorState {
            id: door.id,
            name: door.name.clone(),
            position: door.position,
            heading: door.heading,
            incline: door.incline,
            size: door.size,
            open_type: door.open_type,
            state: door.state,
            inverted: door.inverted,
            parameter: door.parameter,
        })
        .collect()
}

fn draw_loading_screen(
    runtime: &mut Runtime,
    renderer: &mut Renderer,
    options: &Options,
    viewport: [u32; 2],
    scale: f32,
) {
    let live = runtime.live.as_ref();
    let error = runtime
        .loading_error
        .as_deref()
        .or_else(|| live.and_then(|live| live.error.as_deref()));
    let title = live
        .and_then(|live| live.environment.as_ref())
        .map(|env| {
            if env.long_name.is_empty() {
                env.short_name.as_str()
            } else {
                env.long_name.as_str()
            }
        })
        .unwrap_or(if options.connection.is_none() {
            &options.zone
        } else {
            "Norrath"
        });
    let job_progress = runtime
        .client_job
        .as_ref()
        .map(|job| &job.progress)
        .or_else(|| runtime.loading_job.as_ref().map(|job| &job.progress));
    let detail = job_progress.map_or_else(
        || {
            if runtime.loaded_zone.is_some() {
                "Traveling to your next zone…"
            } else if options.connection.is_some() {
                "Connecting to the world…"
            } else {
                "Preparing your journey…"
            }
        },
        |progress| progress.detail.as_str(),
    );
    let progress = job_progress.and_then(|progress| progress.fraction);
    let frame = loading_ui::loading_frame(
        viewport,
        title,
        detail,
        progress,
        runtime.started.elapsed().as_secs_f32(),
        error,
    );
    renderer.set_ui_scaled(&frame, scale);
    renderer.render_ui();
    runtime.ui_frame = frame;
    runtime.chat_link_hits.clear();
    runtime.moving = false;
}

fn display_name(name: &str) -> String {
    name.trim_end_matches(|c: char| c.is_ascii_digit())
        .replace('_', " ")
        .trim_start_matches('#')
        .to_owned()
}

fn view_camera(runtime: &Runtime) -> Camera {
    let mut camera = runtime.camera;
    if runtime.third_person {
        let delta = [
            -camera.yaw.sin() * camera.pitch.cos() * 18.,
            -camera.yaw.cos() * camera.pitch.cos() * 18.,
            -camera.pitch.sin() * 18.,
        ];
        let focus = camera.position;
        let desired = std::array::from_fn(|i| focus[i] + delta[i]);
        camera.position = runtime.collision.as_ref().map_or(desired, |collision| {
            collision.clip_camera(focus, desired, 0.35)
        });
        if let Some(doors) = &runtime.doors {
            camera.position = doors
                .collision_world()
                .clip_camera(focus, camera.position, 0.35);
        }
    }
    camera
}

fn add_nameplates(
    frame: &mut openeq_ui::UiFrame,
    live: &live::LiveWorld,
    actors: &ActorRenderer,
    camera: &Camera,
    size: [u32; 2],
) {
    let viewport = openeq_ui::Rect::new(0., 0., size[0] as f32, size[1] as f32);
    let mut labels = Vec::new();
    for entity in live.entities.values() {
        let spawn = &entity.spawn;
        if Some(spawn.id) == live.own_id || spawn.race == 127 || spawn.body_type >= 66 {
            continue;
        }
        let Some(bounds) = actors.bounds().get(&spawn.id) else {
            continue;
        };
        let p = bounds.center();
        let distance: f32 = p
            .iter()
            .zip(camera.position)
            .map(|(a, b)| (a - b).powi(2))
            .sum();
        if distance > 180. * 180. {
            continue;
        }
        let Some([left, top, right, bottom]) =
            openeq::targeting::screen_bounds(camera, size.map(|v| v as f32), bounds)
        else {
            continue;
        };
        if right < 0. || left > size[0] as f32 || bottom < 0. || top > size[1] as f32 {
            continue;
        }
        let rect = openeq_ui::Rect::new((left + right) * 0.5 - 110., top - 22., 220., 20.);
        let color = if live.target == Some(spawn.id) {
            [255, 230, 100, 255]
        } else {
            [190, 235, 245, 255]
        };
        labels.push(openeq_ui::DrawCommand::Text {
            rect,
            clip: viewport,
            text: display_name(&spawn.name),
            font: 2,
            color,
            align: openeq_ui::TextAlign::Center,
            vertical_center: true,
            wrap: false,
        });
    }
    labels.append(&mut frame.commands);
    frame.commands = labels;
}

fn handle_targeting(
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    windows: Query<(&Window, &CursorOptions), With<PrimaryWindow>>,
    mut runtime: ResMut<Runtime>,
) {
    if !runtime.world_ready() {
        return;
    }
    if runtime.interaction.controls_blocked || runtime.interaction.editor.active {
        return;
    }
    let Ok((window, cursor)) = windows.single() else {
        return;
    };
    let camera = view_camera(&runtime);
    let point = window.cursor_position().map(|p| [p.x, p.y]);
    let ui_hit = point.is_some_and(|p| runtime.ui_frame.hit_test(p).is_some());
    let clicked = mouse.just_pressed(MouseButton::Left) && !is_captured(cursor) && !ui_hit;
    let picked = if clicked {
        point.and_then(|point| {
            runtime.actors.as_ref().and_then(|actors| {
                openeq::targeting::pick_actor(
                    actors.bounds(),
                    runtime.live.as_ref().and_then(|live| live.own_id),
                    &camera,
                    [window.width(), window.height()],
                    point,
                )
            })
        })
    } else {
        None
    };
    let Some(live) = runtime.live.as_mut() else {
        return;
    };
    if keys.just_pressed(KeyCode::Tab) {
        let mut nearby: Vec<_> = live
            .entities
            .values()
            .filter(|e| {
                Some(e.spawn.id) != live.own_id && e.spawn.race != 127 && e.spawn.body_type < 66
            })
            .map(|e| {
                let p = e.position(std::time::Instant::now());
                (
                    e.spawn.id,
                    p.iter()
                        .zip(camera.position)
                        .map(|(a, b)| (a - b).powi(2))
                        .sum::<f32>(),
                )
            })
            .filter(|(_, d)| *d < 250. * 250.)
            .collect();
        nearby.sort_by(|a, b| a.1.total_cmp(&b.1));
        let next = live
            .target
            .and_then(|id| nearby.iter().position(|e| e.0 == id))
            .map_or(0, |i| i + 1);
        if !nearby.is_empty() {
            live.set_target(Some(nearby[next % nearby.len()].0));
        }
    }
    if clicked && point.is_some() {
        live.set_target(picked);
    }
}

#[cfg(test)]
mod loading_tests {
    use super::*;
    use std::time::{Duration, Instant};

    fn options(zone: &str) -> Options {
        Options {
            zone: zone.into(),
            dir: loader::default_client_dir().unwrap(),
            position: Some([123., 456., 789.]),
            connection: None,
        }
    }

    #[test]
    #[ignore = "requires original zone assets and a GPU"]
    fn startup_and_zone_replacement_keep_presenting_loading_frames() {
        let mut renderer = Renderer::new_headless(640, 360).unwrap();
        let mut runtime = Runtime::new(Camera {
            position: [123., 456., 789.],
            ..Default::default()
        });
        let mut client_job = zone_loading::start_client(loader::default_client_dir().unwrap());
        let client_started = Instant::now();
        loop {
            if let Some(client) = client_job.poll() {
                runtime.hud = client.unwrap().hud;
                break;
            }
            assert!(client_started.elapsed() < Duration::from_secs(30));
            let frame = loading_ui::loading_frame(
                [640, 360],
                "Norrath",
                &client_job.progress.detail,
                client_job.progress.fraction,
                client_started.elapsed().as_secs_f32(),
                None,
            );
            renderer.set_ui(&frame);
            renderer.render_ui();
            std::thread::sleep(Duration::from_millis(8));
        }
        for zone in ["gfaydark", "poknowledge"] {
            let options = options(zone);
            // Exercise online preparation without displacing a player's live
            // session: character libraries and collision, plus metadata above.
            runtime.loading_destination = desired_destination(&runtime, &options);
            runtime.loading_job = Some(zone_loading::start(
                zone_loading::Request {
                    dir: options.dir.clone(),
                    zone: zone.into(),
                    online: true,
                    time_of_day: 0.5,
                    actors: [112, 367, 34]
                        .into_iter()
                        .map(|race| openeq_render::actors::ActorState {
                            id: race,
                            race,
                            gender: if race == 112 { 0 } else { 2 },
                            size: 6.,
                            ..Default::default()
                        })
                        .collect(),
                    doors: Vec::new(),
                },
                renderer.upload_context(),
            ));
            let before = runtime.camera.position;
            let started = Instant::now();
            let mut frames = 0;
            loop {
                prepare_world(&mut runtime, &mut renderer, &options);
                if runtime.world_ready() {
                    break;
                }
                assert!(
                    runtime.loading_error.is_none(),
                    "{:?}",
                    runtime.loading_error
                );
                assert!(started.elapsed() < Duration::from_secs(120));
                draw_loading_screen(&mut runtime, &mut renderer, &options, [640, 360], 1.);
                frames += 1;
                if frames == 5 {
                    let (width, height, rgba) = renderer.read_rgba().unwrap();
                    assert!(rgba.chunks_exact(4).any(|p| p[0] > 150));
                    image::save_buffer(
                        format!("/tmp/openeq-loading-{zone}.png"),
                        &rgba,
                        width,
                        height,
                        image::ColorType::Rgba8,
                    )
                    .unwrap();
                    // Resize during a real texture upload, including HiDPI UI.
                    renderer.resize(1280, 720);
                    draw_loading_screen(&mut runtime, &mut renderer, &options, [640, 360], 2.);
                    renderer.resize(640, 360);
                }
                std::thread::sleep(Duration::from_millis(8));
            }
            assert!(frames > 5, "no loading frames were drawn");
            assert_eq!(
                runtime.camera.position, before,
                "explicit position overwritten"
            );
            assert_eq!(runtime.loaded_zone.as_deref(), Some(zone));
            assert!(runtime.collision.is_some());
            assert!(runtime.hud.is_some());
            assert!(runtime.actors.as_ref().unwrap().rendered_instances > 0);
            assert!(runtime.doors.is_some());
            // The UI-only pass never needed a scene; now the full world can draw.
            assert!(runtime.ui_frame.commands.is_empty());
            renderer.render_ui();
            let (_, _, pixels) = renderer.read_rgba().unwrap();
            assert!(
                pixels.chunks_exact(4).all(|p| p[..3] == [0, 0, 0]),
                "the renderer retained the opaque loading overlay"
            );
            renderer.render(runtime.scene.as_ref().unwrap(), &runtime.camera);
            eprintln!(
                "{zone}: {frames} responsive loading frames in {:?}",
                started.elapsed()
            );
        }
    }

    #[test]
    #[ignore = "requires a GPU"]
    fn superseded_job_and_error_never_install_a_world() {
        let mut renderer = Renderer::new_headless(320, 240).unwrap();
        let mut runtime = Runtime::new(Camera::default());
        let (release, wait) = std::sync::mpsc::channel();
        runtime.loading_destination = Some(loading::Destination {
            zone: "old".into(),
            generation: 0,
        });
        runtime.loading_job = Some(loading::Job::start(move |_| {
            wait.recv().unwrap();
            anyhow::bail!("old failure must not appear in the new destination")
        }));
        let options = options("missing-zone-for-loading-regression");
        prepare_world(&mut runtime, &mut renderer, &options);
        // Dropping an obsolete worker must not wait for this release.
        release.send(()).unwrap();
        let started = Instant::now();
        while runtime.loading_error.is_none() {
            prepare_world(&mut runtime, &mut renderer, &options);
            assert!(started.elapsed() < Duration::from_secs(10));
            std::thread::sleep(Duration::from_millis(1));
        }
        assert!(
            !runtime
                .loading_error
                .as_ref()
                .unwrap()
                .contains("old failure")
        );
        assert!(!runtime.world_ready());
        assert!(runtime.scene.is_none());
        // Keep drawing the error without automatically respawning failed work.
        for _ in 0..3 {
            prepare_world(&mut runtime, &mut renderer, &options);
            draw_loading_screen(&mut runtime, &mut renderer, &options, [320, 240], 1.);
            assert!(runtime.loading_job.is_none());
        }
    }
}
