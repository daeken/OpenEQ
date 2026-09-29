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

use openeq::profiling::{FrameProfiler, FrameSample};
use openeq::{chat, hud, live, loading, loading_ui, movement, zone_loading};
use openeq_net::session::ConnectionConfig;
use openeq_render::actors::{ActorRenderer, CharacterModelSet};
use std::path::PathBuf;

use bevy::ecs::system::NonSendMarker;
use bevy::input::ButtonState;
use bevy::input::mouse::{AccumulatedMouseMotion, MouseWheel};
use bevy::prelude::*;
use bevy::window::{CursorGrabMode, CursorOptions, PrimaryWindow, WindowEvent, WindowFocused};
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
    model_set: CharacterModelSet,
    no_audio: bool,
    interactive: bool,
    login_host: Option<String>,
    login_port: Option<u16>,
    world_port: Option<u16>,
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
    /// Resolve only an authoritative arrival, after its own zone assets load.
    spawn_needs_recovery: bool,
    interaction: openeq::interaction::Interaction,
    layout_store: Option<openeq::ui_layout::LayoutStore>,
    loaded_zone: Option<String>,
    loaded_destination: Option<loading::Destination>,
    loading_destination: Option<loading::Destination>,
    loading_job: Option<loading::Job<zone_loading::PreparedZone>>,
    loading_error: Option<String>,
    client_job: Option<loading::Job<zone_loading::ClientData>>,
    zone_lines: openeq_assets::zone_lines::ZoneLines,
    liquids: openeq_assets::liquid_regions::LiquidRegions,
    zone_travel: openeq::zone_travel::ZoneTravel,
    zone_map: Option<openeq::map::ZoneMap>,
    map_state: openeq::map::MapState,
    map_open: bool,
    third_person: bool,
    doors: Option<openeq_render::doors::DoorRenderer>,
    profiler: FrameProfiler,
    audio: Option<openeq::audio::AudioService>,
    account: Option<openeq::account::AccountController>,
    account_input: Option<openeq::account_ui::AccountInput>,
    account_ui: Option<openeq::account_ui::AccountUi>,
    account_ui_job: Option<loading::Job<openeq::account_ui::AccountUi>>,
    account_preferences: Option<openeq::account::preferences::PreferenceStore>,
    client_data: Option<zone_loading::ClientData>,
}

impl Runtime {
    fn world_ready(&self) -> bool {
        self.scene.is_some()
            && self.account.is_none()
            && self.client_job.is_none()
            && self.loading_error.is_none()
            && self.loading_job.is_none()
            && self.live.as_ref().is_none_or(|live| {
                live.ready
                    && !live.zone_request_pending()
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
            spawn_needs_recovery: false,
            interaction: openeq::interaction::Interaction::default(),
            layout_store: None,
            loaded_zone: None,
            loaded_destination: None,
            loading_destination: None,
            loading_job: None,
            loading_error: None,
            client_job: None,
            zone_lines: Default::default(),
            liquids: Default::default(),
            zone_travel: Default::default(),
            zone_map: None,
            map_state: openeq::map::MapState::default(),
            map_open: false,
            third_person: false,
            profiler: FrameProfiler::default(),
            doors: None,
            audio: None,
            account: None,
            account_input: None,
            account_ui: None,
            account_ui_job: None,
            account_preferences: None,
            client_data: None,
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
    runtime.audio = Some(openeq::audio::AudioService::new(
        options.dir.clone(),
        !options.no_audio,
    ));
    if options.interactive {
        let store = openeq::account::preferences::PreferenceStore::open()
            .map_err(|error| {
                tracing::warn!(%error,"connection preferences unavailable; preserving saved file");
            })
            .ok();
        let preferences = store
            .as_ref()
            .map(|store| store.preferences.clone())
            .unwrap_or_default();
        let mut endpoint = preferences.endpoint;
        if let Some(host) = &options.login_host {
            endpoint.host.clone_from(host);
        }
        if let Some(port) = options.login_port {
            endpoint.login_port = port;
        }
        if let Some(port) = options.world_port {
            endpoint.world_port = port;
        }
        let mut account = openeq::account::AccountController::default();
        account.view.selected_world = preferences.last_server;
        account.view.selected_character = preferences.last_character;
        runtime.account = Some(account);
        runtime.account_input = Some(openeq::account_ui::AccountInput::new(endpoint));
        runtime.account_preferences = store;
        let dir = options.dir.clone();
        runtime.account_ui_job = Some(loading::Job::start(move |_| {
            Ok(openeq::account_ui::AccountUi::load(&dir))
        }));
        runtime.client_job = Some(zone_loading::start_client(options.dir.clone()));
        runtime.fly = false;
    }
    if let Some(config) = &options.connection {
        runtime.client_job = Some(zone_loading::start_client(options.dir.clone()));
        runtime.fly = false;
        match openeq::ui_layout::LayoutStore::open(config) {
            Ok(store) => {
                store.layout().apply(
                    &mut runtime.interaction.window_positions,
                    &mut runtime.interaction.window_stack,
                    &mut runtime.map_state,
                );
                runtime.layout_store = Some(store);
            }
            Err(error) => eprintln!(
                "Could not restore UI layout; using defaults and preserving saved file: {error}"
            ),
        }
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
        .add_systems(
            PreUpdate,
            recover_window_focus.after(bevy::input::InputSystems),
        )
        // Input ownership and capture run before camera movement.
        .add_systems(
            Update,
            (
                handle_account_input,
                handle_gameplay_input,
                handle_targeting,
                handle_cursor_capture,
                update_camera,
            )
                .chain(),
        )
        .add_systems(
            Last,
            (render_frame, update_audio, persist_ui_layout).chain(),
        )
        .run()
}

fn update_audio(runtime: Res<Runtime>) {
    let Some(audio) = &runtime.audio else {
        return;
    };
    let zone = runtime
        .world_ready()
        .then(|| runtime.loaded_destination.as_ref())
        .flatten()
        .map(|destination| (destination.generation, destination.zone.as_str()));
    audio.update(
        zone,
        runtime.camera.position,
        runtime.live.as_ref().map_or(12, |live| live.hour),
    );
}

fn persist_ui_layout(
    mut runtime: ResMut<Runtime>,
    mouse: Res<ButtonInput<MouseButton>>,
    mut exits: MessageReader<AppExit>,
) {
    let flush = exits.read().next().is_some()
        || mouse.just_released(MouseButton::Left)
        || mouse.just_released(MouseButton::Right);
    if runtime.layout_store.is_none() {
        return;
    }
    let layout = openeq::ui_layout::Layout::capture(
        &runtime.interaction.window_positions,
        runtime.interaction.window_stack.order(),
        &runtime.map_state,
    );
    if let Err(error) =
        runtime
            .layout_store
            .as_mut()
            .unwrap()
            .update(layout, std::time::Instant::now(), flush)
    {
        tracing::warn!(%error, "could not save UI layout");
    }
}

fn parse_args() -> anyhow::Result<Options> {
    let mut args = std::env::args().skip(1);
    let mut zone = None;
    let mut dir = None;
    let mut position = None;
    let mut connection = None;
    let mut model_set = CharacterModelSet::Classic;
    let mut no_audio = false;
    let mut login_host = None;
    let mut login_port = None;
    let mut world_port = None;

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--no-audio" => no_audio = true,
            "--login" => {
                login_host = Some(
                    args.next()
                        .ok_or_else(|| anyhow::anyhow!("--login needs a hostname"))?,
                )
            }
            "--login-port" => {
                login_port = Some(
                    args.next()
                        .ok_or_else(|| anyhow::anyhow!("--login-port needs a port"))?
                        .parse::<u16>()?,
                )
            }
            "--world-port" => {
                world_port = Some(
                    args.next()
                        .ok_or_else(|| anyhow::anyhow!("--world-port needs a port"))?
                        .parse::<u16>()?,
                )
            }
            "--models" => {
                model_set = match args.next().as_deref() {
                    Some("classic") => CharacterModelSet::Classic,
                    Some("luclin") => CharacterModelSet::Luclin,
                    _ => anyhow::bail!("--models requires classic or luclin"),
                };
            }
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
                println!(
                    "usage: openeq [zone] [--dir DIR] [--pos X,Y,Z] [--connect CONFIG | --login HOST] [--login-port PORT] [--world-port PORT] [--models classic|luclin] [--no-audio]\nNo zone or --connect opens interactive sign-in."
                );
                std::process::exit(0);
            }
            other if !other.starts_with('-') && zone.is_none() => zone = Some(other.to_string()),
            other => anyhow::bail!("unrecognised argument {other}"),
        }
    }

    anyhow::ensure!(
        connection.is_none()
            || (login_host.is_none() && login_port.is_none() && world_port.is_none()),
        "--connect cannot be combined with interactive login options"
    );
    anyhow::ensure!(
        zone.is_none() || (login_host.is_none() && login_port.is_none() && world_port.is_none()),
        "choose offline zone viewing or interactive login"
    );
    anyhow::ensure!(
        login_port != Some(0) && world_port != Some(0),
        "ports must be between 1 and 65535"
    );
    let interactive = connection.is_none() && zone.is_none();
    let zone = zone
        .or_else(|| connection.as_ref().map(|_| "poknowledge".to_string()))
        .unwrap_or_else(|| "Sign in".into());
    let dir = match dir.or_else(loader::default_client_dir) {
        Some(dir) => dir,
        None => anyhow::bail!("no client directory; pass --dir"),
    };
    Ok(Options {
        zone,
        dir,
        position,
        connection,
        model_set,
        no_audio,
        interactive,
        login_host,
        login_port,
        world_port,
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
    windows: Query<(&Window, &CursorOptions), With<PrimaryWindow>>,
    mut runtime: ResMut<Runtime>,
) {
    runtime.moving = false;
    if !runtime.world_ready() {
        return;
    }
    if runtime
        .live
        .as_ref()
        .is_some_and(|live| !live.movement_allowed())
    {
        runtime.ground_motion = movement::GroundMotion::default();
        runtime.fly = false;
        return;
    }
    let online = runtime.live.is_some();
    let focused = windows.single().is_ok_and(|(window, _)| window.focused);
    let controls = focused
        && !runtime.interaction.editor.active
        && !runtime.interaction.controls_blocked
        && runtime
            .live
            .as_ref()
            .is_none_or(|live| live.movement_allowed());
    if online && controls && keys.just_pressed(KeyCode::KeyF) {
        runtime.fly = !runtime.fly;
        runtime.ground_motion = movement::GroundMotion::default();
    }
    let mut camera = runtime.camera;

    let captured = windows
        .single()
        .is_ok_and(|(_, cursor)| is_captured(cursor));
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
    if controls && (keys.pressed(KeyCode::ControlLeft) || keys.pressed(KeyCode::ControlRight)) {
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
    let (sin_pitch, cos_pitch) = camera.pitch.sin_cos();
    let volume_direction = Vec3::new(
        heading[0] * forward * cos_pitch + right[0] * strafe,
        heading[1] * forward * cos_pitch + right[1] * strafe,
        forward * sin_pitch + lift,
    )
    .clamp_length_max(1.);
    let gravity = runtime
        .live
        .as_ref()
        .map_or(Default::default(), |live| live.player_gravity());
    let mut position = camera.position;
    position[0] += velocity_xy[0] * dt;
    position[1] += velocity_xy[1] * dt;
    let feet = [
        camera.position[0],
        camera.position[1],
        camera.position[2] - 6.,
    ];
    let jump = controls && keys.just_pressed(KeyCode::Space);
    let motion_input = movement::MotionInput {
        walk_velocity: velocity_xy,
        volume_velocity: (volume_direction * speed).to_array(),
        jump,
        gravity,
    };
    let mode = runtime.collision.as_ref().map(|collision| {
        movement::MotionWorld {
            collision,
            dynamic: None,
            liquids: Some(&runtime.liquids),
        }
        .mode(feet, &motion_input)
    });
    let allow_carry = !runtime.fly
        && runtime.ground_motion.velocity_z <= 0.
        && !jump
        && matches!(
            mode,
            Some(movement::MotionMode::Ground | movement::MotionMode::Levitating)
        );
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
        let moved = motion.step_in_world(
            movement::MotionWorld {
                collision: runtime.collision.as_ref().unwrap(),
                dynamic: runtime.doors.as_ref().map(|doors| doors.collision_world()),
                liquids: Some(&runtime.liquids),
            },
            feet,
            motion_input,
            dt,
        );
        runtime.ground_motion = motion;
        position = [moved[0], moved[1], moved[2] + 6.];
    }
    runtime.moving = forward != 0.
        || strafe != 0.
        || (lift != 0.
            && matches!(
                mode,
                Some(movement::MotionMode::Swimming | movement::MotionMode::Flying)
            ));
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
    if runtime.account.is_some() {
        release_cursor(&mut cursor);
        focus.clear();
        return;
    }

    if !runtime.world_ready() {
        release_cursor(&mut cursor);
        focus.clear();
        if keys.just_pressed(KeyCode::Escape)
            && runtime
                .live
                .as_ref()
                .is_none_or(|live| !live.game.recovery.blocks_movement())
        {
            exit.write(AppExit::Success);
        }
        return;
    }
    if runtime
        .live
        .as_ref()
        .is_some_and(|live| live.game.recovery.blocks_movement())
    {
        release_cursor(&mut cursor);
        focus.clear();
        return;
    }
    let mut captured = is_captured(&cursor);
    for event in focus.read() {
        if event.window == entity && !event.focused && captured {
            release_cursor(&mut cursor);
            captured = false;
        }
    }

    if !windows.single().is_ok_and(|window| window.focused) {
        return;
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

fn recover_window_focus(
    windows: Query<Entity, With<PrimaryWindow>>,
    mut events: MessageReader<WindowEvent>,
    mut keys: ResMut<ButtonInput<KeyCode>>,
    mut mouse: ResMut<ButtonInput<MouseButton>>,
) {
    if let Ok(window) = windows.single() {
        openeq::input::recover_focus_loss(window, events.read(), &mut keys, &mut mouse);
    } else {
        events.clear();
    }
}

fn handle_gameplay_input(
    mut keys: ResMut<ButtonInput<KeyCode>>,
    mut mouse: ResMut<ButtonInput<MouseButton>>,
    mut events: MessageReader<WindowEvent>,
    mut wheel: MessageReader<MouseWheel>,
    mut windows: Query<(Entity, &mut Window, &mut CursorOptions), With<PrimaryWindow>>,
    mut runtime: ResMut<Runtime>,
    mut exit: MessageWriter<AppExit>,
) {
    let Ok((window_id, mut window, mut cursor)) = windows.single_mut() else {
        return;
    };
    if runtime.account.is_some() {
        events.clear();
        wheel.clear();
        return;
    }
    if let Some(live) = runtime.live.as_mut() {
        live.poll();
    }
    runtime.interaction.controls_blocked = runtime.interaction.editor.active;
    runtime.interaction.escape_handled = false;
    if let Some(input) = runtime.account_input.as_mut() {
        input.begin_handoff_frame(&mut keys);
    }
    if !runtime.world_ready() {
        if let Some(input) = runtime.account_input.as_mut() {
            for event in events.read() {
                input.filter_handoff_event(event, window_id, &mut keys);
            }
        }
        if keys.just_pressed(KeyCode::Escape)
            && runtime
                .live
                .as_ref()
                .is_none_or(|live| !live.game.recovery.blocks_movement())
        {
            exit.write(AppExit::Success);
        }
        events.clear();
        wheel.clear();
        keys.reset_all();
        mouse.reset_all();
        window.ime_enabled = false;
        release_cursor(&mut cursor);
        let interaction = &mut runtime.interaction;
        interaction.chat_input.reset(&mut interaction.editor);
        interaction.drag = None;
        interaction.pointer = None;
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
        audio,
        account_input,
        ..
    } = &mut *runtime
    else {
        events.clear();
        wheel.clear();
        return;
    };
    if let Some(environment) = &live.environment {
        let title = format!("OpenEQ - {}", environment.short_name);
        if window.title != title {
            window.title = title;
        }
    }
    if interaction.pointer.is_none() {
        interaction.pointer = window.cursor_position().map(|p| [p.x, p.y]);
    }
    let mut chat_pointer_owned = false;
    for event in events.read() {
        if account_input
            .as_mut()
            .is_some_and(|input| input.filter_handoff_event(event, window_id, &mut keys))
        {
            continue;
        }
        // Use the displayed frame's original hit. Raising only changes the
        // next frame's order, so this press still activates the control seen.
        // This precedes chat's early consumption of an edit-box click.
        if matches!(event, WindowEvent::MouseButtonInput(event)
            if event.window == window_id
                && event.state == ButtonState::Pressed
                && matches!(event.button, MouseButton::Left | MouseButton::Right))
            && window.focused
            && !is_captured(&cursor)
            && let Some(hit) = interaction
                .pointer
                .and_then(|point| ui_frame.hit_test(point))
        {
            interaction.window_stack.raise_hit(hit);
        }
        // Honor a chat click before any later text in this same event batch.
        // Clicking an active edit box keeps its draft and composition intact.
        match event {
            WindowEvent::CursorMoved(event) if event.window == window_id => {
                interaction.pointer = Some(event.position.to_array());
            }
            WindowEvent::MouseButtonInput(event)
                if event.window == window_id
                    && event.state == ButtonState::Pressed
                    && event.button == MouseButton::Left
                    && !is_captured(&cursor) =>
            {
                let chat_hit = interaction
                    .pointer
                    .and_then(|point| ui_frame.hit_test(point))
                    .is_some_and(|hit| hit.item == "game:chat_input");
                if chat_hit {
                    chat_pointer_owned = true;
                    if !interaction.editor.active {
                        interaction.editor.open("");
                    }
                    interaction.controls_blocked = true;
                } else if !chat_hit && interaction.editor.active {
                    interaction.editor.cancel();
                    interaction.controls_blocked = true;
                }
            }
            _ => {}
        }
        let result = interaction
            .chat_input
            .event(&mut interaction.editor, window_id, event);
        interaction.controls_blocked |= result.captured;
        interaction.escape_handled |= result.escape_handled;
        interaction.chat_scroll = interaction
            .chat_scroll
            .saturating_add_signed(result.scroll as isize)
            .min(5000);
        if matches!(event, WindowEvent::WindowFocused(event) if event.window == window_id && !event.focused)
        {
            interaction.drag = None;
            release_cursor(&mut cursor);
        }
        if interaction.editor.active {
            release_cursor(&mut cursor);
        }
        if let Some(text) = result.submitted {
            if let Some(response) = audio.as_ref().and_then(|audio| audio.command(&text)) {
                live.game.notice(response);
            } else if interaction.submit(&text, live, camera_position) {
                exit.write(AppExit::Success);
            }
        }
    }
    interaction.chat_input.suppress_captured_keys(&mut keys);
    if live.game.recovery.blocks_movement() && keys.just_pressed(KeyCode::Escape) {
        interaction.escape_handled = true;
    }
    if window.focused && !interaction.controls_blocked && !interaction.editor.active {
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
            } else if interaction.spell_inspection.close() {
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
    let ui_left_click = !chat_pointer_owned && mouse.just_pressed(MouseButton::Left);
    let ui_right_click = !chat_pointer_owned && mouse.just_pressed(MouseButton::Right);
    if window.focused
        && !is_captured(&cursor)
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
            if ui_left_click && let Some(action) = openeq::death::RecoveryAction::from_hit(hit) {
                live.recovery_action(action);
            }
            if hit.window_id.as_deref() == Some("spell_inspection") {
                for event in wheel.read() {
                    interaction.spell_inspection.wheel(hit, event.y);
                }
            }
            if hit.item == "game:chat_log"
                && ui_left_click
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
                if ui_left_click {
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
                if ui_right_click {
                    map_state.waypoints.clear();
                }
                for event in wheel.read() {
                    map_state.units_per_pixel =
                        (map_state.units_per_pixel * 1.2f32.powf(-event.y)).clamp(0.25, 128.);
                }
            }
            if (ui_left_click || ui_right_click)
                && let Some(action) = hud::UiAction::from_hit(hit)
            {
                if let hud::UiAction::BeginWindowDrag(name) = action {
                    if ui_left_click {
                        interaction.drag =
                            Some((name, [point[0] - hit.rect.x, point[1] - hit.rect.y]));
                    }
                } else if !matches!(action, hud::UiAction::FocusChat) {
                    interaction.ui_action(
                        action,
                        ui_right_click,
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
    // Clicks can focus chat too: synchronize native text input only after
    // both keyboard and pointer actions have chosen the final owner.
    window.ime_enabled = window.focused && interaction.editor.active;
    if interaction.editor.active {
        interaction.controls_blocked = true;
        release_cursor(&mut cursor);
    }
    wheel.clear();
    interaction.tick(live);
}

fn handle_account_input(
    mut events: MessageReader<WindowEvent>,
    mut windows: Query<(Entity, &mut Window, &mut CursorOptions), With<PrimaryWindow>>,
    mut runtime: ResMut<Runtime>,
    mut keys: ResMut<ButtonInput<KeyCode>>,
    mut mouse: ResMut<ButtonInput<MouseButton>>,
    mut exit: MessageWriter<AppExit>,
) {
    use openeq::account::{Action, Stage};
    use openeq::account_ui::Intent;
    let Ok((window_id, mut window, mut cursor)) = windows.single_mut() else {
        events.clear();
        return;
    };
    let Runtime {
        account,
        account_input,
        ui_frame,
        ..
    } = &mut *runtime;
    let Some(input) = account_input else {
        events.clear();
        return;
    };
    let Some(controller) = account else {
        // Gameplay filters held account keys in native event order, before
        // chat sees raw repeats or gameplay sees physical shortcut edges.
        events.clear();
        return;
    };
    release_cursor(&mut cursor);
    for event in events.read() {
        let Some(intent) = input.event(&controller.view, ui_frame, window_id, event) else {
            continue;
        };
        match intent {
            Intent::SignIn => {
                let (endpoint, username, password) = input.take_credentials();
                if let Err(error) = controller.sign_in(endpoint, username, password) {
                    controller.view.notice = Some(error.to_string());
                }
            }
            Intent::Cancel => {
                controller.cancel();
                input.reset();
            }
            Intent::Exit => {
                exit.write(AppExit::Success);
            }
            Intent::SelectWorld { token, id }
                if token == controller.view.token && controller.view.stage == Stage::Worlds =>
            {
                if controller
                    .view
                    .servers
                    .iter()
                    .any(|server| server.server_id == id)
                {
                    controller.view.selected_world = Some(id);
                }
            }
            Intent::SelectCharacter { token, name }
                if token == controller.view.token && controller.view.stage == Stage::Characters =>
            {
                if controller
                    .view
                    .characters
                    .iter()
                    .any(|character| character.name == name && character.enabled)
                {
                    controller.view.selected_character = Some(name);
                }
            }
            Intent::Play { token } => {
                let action = match controller.view.stage {
                    Stage::Worlds => controller.view.selected_world.map(Action::ChooseWorld),
                    Stage::Characters => controller
                        .view
                        .selected_character
                        .clone()
                        .map(Action::ChooseCharacter),
                    _ => None,
                };
                if let Some(action) = action {
                    controller.action(token, action);
                }
            }
            Intent::Refresh { token } => {
                controller.action(token, Action::RefreshWorlds);
            }
            Intent::Back { token } => {
                controller.action(token, Action::Back);
            }
            _ => {}
        }
    }
    window.ime_enabled = window.focused && input.ime_enabled(&controller.view);
    // Every account event belongs to this screen. AccountInput retains held-key
    // ownership separately until release so it cannot leak through a handoff.
    keys.reset_all();
    mouse.reset_all();
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
    let mut profile = runtime.profiler.begin();
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
            runtime
                .zone_travel
                .rebase([position.x, position.y, position.z]);
            runtime.ground_motion = movement::GroundMotion::default();
            runtime.spawn_needs_recovery = true;
        }
    }
    if let Some(mut renderer) = runtime.renderer.take() {
        if present_account(&mut runtime, &mut renderer, ui_size, window.scale_factor()) {
            runtime.renderer = Some(renderer);
            return;
        }
        prepare_world(&mut runtime, &mut renderer, &options);
        recover_arrival_floor(&mut runtime);
        if runtime.world_ready() && runtime.live.is_some() {
            let runtime = &mut *runtime;
            let camera = runtime.camera;
            let position = [
                camera.position[0],
                camera.position[1],
                camera.position[2] - 3.,
            ];
            if let Some(line) = runtime.zone_travel.observe(&runtime.zone_lines, position) {
                runtime
                    .live
                    .as_mut()
                    .unwrap()
                    .cross_zone_line(line.number, &camera);
            }
        }
        if !runtime.world_ready() {
            runtime.profiler.reset();
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
        renderer.set_view_liquid(runtime.liquids.at(camera.position));
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
        FrameSample::mark(&mut profile, "setup");
        let states = runtime.live.as_ref().map(|live| {
            let mut states = live.actor_states_with_terrain(
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
            );
            if !runtime.fly && runtime.ground_motion.mode == movement::MotionMode::Swimming {
                for state in &mut states {
                    if Some(state.id) == live.own_id
                        && matches!(
                            state.action,
                            openeq_render::actors::ActorAction::Auto
                                | openeq_render::actors::ActorAction::Walk
                                | openeq_render::actors::ActorAction::Run
                        )
                    {
                        state.action = openeq_render::actors::ActorAction::Swim;
                    }
                }
            }
            states
        });
        let elapsed = runtime.started.elapsed().as_secs_f32();
        FrameSample::mark(&mut profile, "terrain_states");
        if let (Some(actors), Some(states)) = (runtime.actors.as_mut(), states) {
            actors.update(&renderer, &states, elapsed);
        }
        FrameSample::mark(&mut profile, "animation_upload");
        {
            let runtime = &mut *runtime;
            if let Some(live) = &mut runtime.live {
                let anchors = live.effect_anchors(&player_camera, runtime.actors.as_ref());
                let frame = live
                    .spell_effects
                    .frame(std::time::Instant::now(), &anchors);
                renderer.set_particles(&frame);
                let projectiles = live
                    .spell_effects
                    .projectiles(std::time::Instant::now(), &anchors);
                if let Some(actors) = &mut runtime.actors {
                    actors.update_projectiles(&renderer, &projectiles);
                }
            }
        }
        FrameSample::mark(&mut profile, "effects");
        let doors = runtime.live.as_ref().map(door_states);
        if let (Some(renderer_doors), Some(states)) = (runtime.doors.as_mut(), doors) {
            renderer_doors.update(&renderer, &states, elapsed);
        }
        FrameSample::mark(&mut profile, "doors");
        let runtime = &mut *runtime;
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
            let mut recovery = hud.recovery_frame(
                ui_size,
                runtime.interaction.pointer,
                &live.game.recovery.view(std::time::Instant::now()),
            );
            let mut additional = Vec::new();
            if runtime.map_open {
                let mut map_state = runtime.map_state.clone();
                map_state.fit_viewport(ui_size);
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
                    additional.push(("map".into(), map.frame(ui_size, &map_state)));
                }
                runtime.map_state = map_state;
            }
            let mut game = runtime.interaction.view(live);
            game.hover_blocked = game
                .pointer
                .is_some_and(|point| recovery.hit_test(point).is_some());
            let mut frame = hud.gameplay_frame_with_windows(
                ui_size,
                &state,
                &game,
                additional,
                &mut runtime.interaction.window_stack,
            );
            if let Some(actors) = &runtime.actors {
                add_nameplates(&mut frame, live, actors, &camera, ui_size);
                let mut feedback = openeq_ui::UiFrame {
                    bounds: frame.bounds,
                    ..Default::default()
                };
                live.combat_feedback.append(
                    &mut feedback,
                    &camera,
                    ui_size,
                    actors.bounds(),
                    live.own_id,
                    std::time::Instant::now(),
                );
                feedback.commands.append(&mut frame.commands);
                frame.commands = feedback.commands;
            }
            frame.commands.append(&mut recovery.commands);
            frame.hit_targets.append(&mut recovery.hit_targets);
            frame.warnings.append(&mut recovery.warnings);
            FrameSample::mark(&mut profile, "ui_build");
            renderer.set_ui_scaled(&frame, window.scale_factor());
            runtime
                .interaction
                .spell_inspection
                .update_metrics(renderer.ui_text_scroll_metrics());
            runtime.chat_link_hits = renderer.ui_link_hits().to_vec();
            runtime.ui_frame = frame;
        } else {
            // A refused border request resumes the same scene, so scene
            // installation cannot clear this overlay for a missing XML skin.
            runtime.ui_frame = openeq_ui::UiFrame::default();
            runtime.chat_link_hits.clear();
            renderer.set_ui(&runtime.ui_frame);
        }
        FrameSample::mark(&mut profile, "ui_upload");
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
        FrameSample::mark(&mut profile, "render_submit");
        let profile_size = runtime.size;
        runtime
            .profiler
            .finish(profile, &mut renderer, profile_size);
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
    let mut renderer =
        pollster::block_on(Renderer::new_surface(&instance, surface, width, height))?;
    if runtime.profiler.enabled() {
        let gpu_timestamps = renderer.enable_profiling(true);
        tracing::info!(gpu_timestamps, "frame profiling enabled (OPENEQ_PROFILE=1)");
    }
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

fn poll_client_assets(runtime: &mut Runtime) {
    if let Some(result) = runtime.client_job.as_mut().and_then(|job| job.poll()) {
        runtime.client_job = None;
        match result {
            Ok(client) => runtime.client_data = Some(client),
            Err(error) => runtime.loading_error = Some(error),
        }
    }
    if runtime.live.is_some()
        && let Some(client) = runtime.client_data.take()
    {
        let live = runtime.live.as_mut().unwrap();
        live.game.strings = client.strings;
        live.game.spell_catalog = client.spells;
        if let Some(effects) = client.spell_effects {
            live.spell_effects.set_assets(effects);
        }
        runtime.hud = client.hud;
    }
}

fn present_account(
    runtime: &mut Runtime,
    renderer: &mut Renderer,
    viewport: [u32; 2],
    scale: f32,
) -> bool {
    if runtime.account.is_none() {
        return false;
    }
    poll_client_assets(runtime);
    if let Some(result) = runtime.account_ui_job.as_mut().and_then(|job| job.poll()) {
        runtime.account_ui_job = None;
        match result {
            Ok(ui) => runtime.account_ui = Some(ui),
            Err(error) => runtime.account.as_mut().unwrap().view.notice = Some(error),
        }
    }
    if let Some(ready) = runtime.account.as_mut().and_then(|account| account.poll()) {
        runtime.account = None;
        if let Some(store) = &mut runtime.account_preferences
            && let Err(error) = store.save_session(&ready.identity)
        {
            tracing::warn!(%error,"could not save connection preferences");
        }
        match openeq::ui_layout::LayoutStore::open_session(&ready.identity) {
            Ok(store) => {
                store.layout().apply(
                    &mut runtime.interaction.window_positions,
                    &mut runtime.interaction.window_stack,
                    &mut runtime.map_state,
                );
                runtime.layout_store = Some(store);
            }
            Err(error) => {
                tracing::warn!(%error,"could not restore character layout; preserving saved file")
            }
        }
        runtime.live = Some(ready.live);
        runtime.fly = false;
        poll_client_assets(runtime);
        return false;
    }
    let controller = runtime.account.as_ref().unwrap();
    let frame = if let (Some(ui), Some(input)) = (&runtime.account_ui, &runtime.account_input) {
        ui.frame(
            viewport,
            &controller.view,
            input,
            runtime.started.elapsed().as_secs_f32(),
        )
    } else {
        loading_ui::loading_frame(
            viewport,
            "Welcome to Norrath",
            "Loading the sign-in screen",
            None,
            runtime.started.elapsed().as_secs_f32(),
            controller.view.notice.as_deref(),
        )
    };
    renderer.set_ui_scaled(&frame, scale);
    runtime.ui_frame = frame;
    renderer.render_ui();
    true
}

/// Polling, cancellation, and installation are deliberately cheap: all asset
/// decoding, collision building, and model/texture uploads run in the job.
fn prepare_world(runtime: &mut Runtime, renderer: &mut Renderer, options: &Options) {
    poll_client_assets(runtime);
    if let Some(config) = &options.connection
        && runtime.live.is_none()
    {
        if runtime.client_data.is_some() {
            runtime.live = Some(live::LiveWorld::start(config.clone()));
            poll_client_assets(runtime);
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
                model_set: options.model_set,
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
            runtime.zone_lines = prepared.zone_lines;
            runtime.liquids = prepared.liquids;
            runtime.zone_travel.reset([
                runtime.camera.position[0],
                runtime.camera.position[1],
                runtime.camera.position[2] - 3.,
            ]);
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

fn recover_arrival_floor(runtime: &mut Runtime) {
    if !runtime.spawn_needs_recovery || !runtime.world_ready() {
        return;
    }
    if let Some(world) = &runtime.collision {
        let [x, y, z] = runtime.camera.position;
        let feet = movement::recover_spawn(world, [x, y, z - 6.]);
        runtime.camera.position = [feet[0], feet[1], feet[2] + 6.];
    }
    runtime.spawn_needs_recovery = false;
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
    if runtime.interaction.controls_blocked
        || runtime.interaction.editor.active
        || runtime
            .live
            .as_ref()
            .is_some_and(|live| !live.movement_allowed())
    {
        return;
    }
    let Ok((window, cursor)) = windows.single() else {
        return;
    };
    if !window.focused {
        return;
    }
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
            model_set: CharacterModelSet::Classic,
            no_audio: true,
            interactive: false,
            login_host: None,
            login_port: None,
            world_port: None,
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
                    model_set: options.model_set,
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
