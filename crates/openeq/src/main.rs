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

mod hud;
mod live;
mod movement;
use openeq_net::session::ConnectionConfig;
use openeq_render::actors::ActorRenderer;
use std::path::PathBuf;

use bevy::ecs::system::NonSendMarker;
use bevy::input::mouse::AccumulatedMouseMotion;
use bevy::prelude::*;
use bevy::window::{CursorGrabMode, CursorOptions, PrimaryWindow, WindowFocused};
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

/// Everything that must exist before the first frame can be drawn.
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
    atmosphere_zone: Option<(String, u8)>,
    moving: bool,
    collision: Option<openeq_assets::collision::CollisionWorld>,
    fly: bool,
    ground_motion: movement::GroundMotion,
}

impl Runtime {
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
            atmosphere_zone: None,
            moving: false,
            collision: None,
            fly: true,
            ground_motion: movement::GroundMotion::default(),
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
    if let Some(config) = options.connection.clone() {
        runtime.live = Some(live::LiveWorld::start(config));
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
            (handle_targeting, handle_cursor_capture, update_camera).chain(),
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
    let online = runtime.live.is_some();
    if online && keys.just_pressed(KeyCode::KeyF) {
        runtime.fly = !runtime.fly;
        runtime.ground_motion = movement::GroundMotion::default();
    }
    let mut camera = runtime.camera;

    let captured = cursors.single().map(is_captured).unwrap_or(false);
    let delta = if captured { motion.delta } else { Vec2::ZERO };
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
    if keys.pressed(KeyCode::KeyW) {
        forward += 1.0;
    }
    if keys.pressed(KeyCode::KeyS) {
        forward -= 1.0;
    }
    if keys.pressed(KeyCode::KeyD) {
        strafe += 1.0;
    }
    if keys.pressed(KeyCode::KeyA) {
        strafe -= 1.0;
    }
    if keys.pressed(KeyCode::Space) {
        lift += 1.0;
    }
    if keys.pressed(KeyCode::ControlLeft) {
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
    if runtime.fly || runtime.collision.is_none() {
        position[2] += lift * speed * dt;
    } else {
        let feet = [
            camera.position[0],
            camera.position[1],
            camera.position[2] - 6.,
        ];
        let mut motion = std::mem::take(&mut runtime.ground_motion);
        let moved = motion.step(
            runtime.collision.as_ref().unwrap(),
            feet,
            velocity_xy,
            keys.just_pressed(KeyCode::Space),
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
) {
    let Ok((entity, mut cursor)) = cursors.single_mut() else {
        return;
    };

    let mut captured = is_captured(&cursor);
    for event in focus.read() {
        if event.window == entity && !event.focused && captured {
            release_cursor(&mut cursor);
            captured = false;
        }
    }

    if (mouse.just_pressed(MouseButton::Right)
        || (runtime.live.is_none() && mouse.just_pressed(MouseButton::Left)))
        && !captured
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
    if runtime.renderer.is_none() && runtime.live.as_ref().is_some_and(|live| !live.ready) {
        if let Some(error) = runtime.live.as_ref().and_then(|live| live.error.as_ref()) {
            eprintln!("Unable to enter world: {error}");
            std::process::exit(1);
        }
        return;
    }
    if runtime.renderer.is_none() {
        let window_ready =
            WINIT_WINDOWS.with(|cell| cell.borrow().get_window(window_entity).is_some());
        if !window_ready {
            return;
        }
        if let Err(error) = initialise(&mut runtime, window_entity, &options) {
            tracing::error!(%error, "renderer or asset initialization failed");
            std::process::exit(1);
        }
    }

    let Ok(window) = window.single() else {
        return;
    };
    let size = (window.physical_width(), window.physical_height());
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
    // `Camera` is `Copy`, and the renderer is taken out and put back so the
    // borrow checker can see the two accesses as disjoint.
    let camera = runtime.camera;
    if let Some(mut renderer) = runtime.renderer.take() {
        if let Some(live) = runtime.live.as_ref() {
            live.camera_position(&camera, runtime.moving);
        }
        if let Some(live) = &runtime.live
            && let Some(env) = &live.environment
        {
            let desired = (env.short_name.clone(), live.hour);
            if runtime.atmosphere_zone.as_ref() != Some(&desired) {
                let settings = openeq_render::environment::EnvironmentSettings {
                    fog_color: env.fog_color[0],
                    fog_start: env.fog_start[0],
                    fog_end: env.fog_end[0],
                    fog_density: env.fog_density,
                    fog_enabled: env.fog_end[0] > env.fog_start[0],
                    sky_enabled: !matches!(env.zone_type, 0 | 3 | 4) && env.sky != 0,
                    ..Default::default()
                };
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
        let states = runtime
            .live
            .as_ref()
            .map(|live| live.actors(camera.position));
        let elapsed = runtime.started.elapsed().as_secs_f32();
        if let (Some(actors), Some(states)) = (runtime.actors.as_mut(), states) {
            actors.update(&renderer, &states, elapsed);
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
                mana: None,
                endurance: None,
                target,
                status: live.error.clone().unwrap_or_else(|| {
                    if live.ready {
                        "Connected • Tab selects an NPC • Right-click to look".into()
                    } else {
                        "Connecting to world…".into()
                    }
                }),
                entities: live.entities.len(),
                movement_updates: live.moves,
            };
            let mut frame = hud.frame([size.0, size.1], &state);
            add_nameplates(&mut frame, live, &camera, [size.0, size.1]);
            renderer.set_ui(&frame);
            runtime.ui_frame = frame;
        }
        if let Some(scene) = runtime.scene.as_ref() {
            let actors = runtime
                .actors
                .as_ref()
                .map(|a| a.draws())
                .unwrap_or_default();
            renderer.render_with_actors(scene, &camera, &actors);
        }
        runtime.renderer = Some(renderer);
    }
}

fn initialise(
    runtime: &mut Runtime,
    window_entity: Entity,
    options: &Options,
) -> anyhow::Result<()> {
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

    let zone = runtime
        .live
        .as_ref()
        .and_then(|live| live.environment.as_ref())
        .map_or(options.zone.as_str(), |env| env.short_name.as_str())
        .to_owned();
    tracing::info!(zone = %zone, dir = %options.dir.display(), "loading zone");
    let started = std::time::Instant::now();
    let scene = loader::load_zone(&options.dir, &zone)?;
    tracing::info!(
        materials = scene.materials.len(),
        triangles = scene.triangle_count(),
        instances = scene.instances.len(),
        lights = scene.lights.len(),
        elapsed_ms = started.elapsed().as_millis() as u64,
        "zone loaded"
    );

    if runtime.live.is_some() {
        runtime.collision = Some(openeq_assets::collision::CollisionWorld::build(&scene));
    }
    let gpu_scene = GpuScene::build(renderer.device(), renderer.queue(), &scene)?;
    renderer.set_scene(&gpu_scene);
    if let Ok(sky) = openeq_assets::environment::load_sky(&options.dir, &zone, 0.5) {
        renderer.set_environment(
            openeq_render::environment::EnvironmentSettings::for_zone(&zone),
            Some(&sky),
        );
    }
    if runtime.live.is_some() {
        match hud::Hud::load(&options.dir) {
            Ok(hud) => runtime.hud = Some(hud),
            Err(error) => tracing::warn!(%error, "XML HUD unavailable"),
        }
    }

    // Drop the camera into the middle of the zone unless the user said otherwise.
    if options.position.is_none() {
        let center = (gpu_scene.bounds_min + gpu_scene.bounds_max) * 0.5;
        runtime.camera.position = [center.x, center.y, center.z + 80.0];
    }
    runtime.size = (width, height);
    runtime.instance = Some(instance);
    runtime.scene = Some(gpu_scene);
    if runtime.live.is_some() {
        runtime.actors = Some(ActorRenderer::load(&options.dir, &zone)?);
    }
    runtime.renderer = Some(renderer);
    Ok(())
}

fn display_name(name: &str) -> String {
    name.trim_end_matches(|c: char| c.is_ascii_digit())
        .replace('_', " ")
        .trim_start_matches('#')
        .to_owned()
}

fn add_nameplates(
    frame: &mut openeq_ui::UiFrame,
    live: &live::LiveWorld,
    camera: &Camera,
    size: [u32; 2],
) {
    let matrix = camera.view_projection(size[0] as f32 / size[1].max(1) as f32, 0.2, 20000.);
    let viewport = openeq_ui::Rect::new(0., 0., size[0] as f32, size[1] as f32);
    let mut labels = Vec::new();
    for entity in live.entities.values() {
        let spawn = &entity.spawn;
        if !spawn.npc || spawn.race == 127 || spawn.body_type >= 66 {
            continue;
        }
        let p = entity.position(std::time::Instant::now());
        let distance: f32 = p
            .iter()
            .zip(camera.position)
            .map(|(a, b)| (a - b).powi(2))
            .sum();
        if distance > 180. * 180. {
            continue;
        }
        let world = Camera::to_world([p[0], p[1], p[2] + spawn.size * 0.65]);
        let clip = matrix * world.extend(1.);
        if clip.w <= 0. {
            continue;
        }
        let ndc = clip.truncate() / clip.w;
        if ndc.x.abs() > 1. || ndc.y.abs() > 1. || ndc.z < 0. {
            continue;
        }
        let rect = openeq_ui::Rect::new(
            (ndc.x * 0.5 + 0.5) * size[0] as f32 - 110.,
            (0.5 - ndc.y * 0.5) * size[1] as f32 - 16.,
            220.,
            20.,
        );
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
    let Ok((window, cursor)) = windows.single() else {
        return;
    };
    let camera = runtime.camera;
    let point = window
        .cursor_position()
        .map(|p| [p.x * window.scale_factor(), p.y * window.scale_factor()]);
    let ui_hit = point.is_some_and(|p| runtime.ui_frame.hit_test(p).is_some());
    let Some(live) = runtime.live.as_mut() else {
        return;
    };
    if keys.just_pressed(KeyCode::Tab) {
        let mut nearby: Vec<_> = live
            .entities
            .values()
            .filter(|e| e.spawn.npc && e.spawn.race != 127 && e.spawn.body_type < 66)
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
    if mouse.just_pressed(MouseButton::Left)
        && !is_captured(cursor)
        && !ui_hit
        && let Some(point) = point
    {
        let size = [
            window.physical_width() as f32,
            window.physical_height() as f32,
        ];
        let matrix = camera.view_projection(size[0] / size[1].max(1.), 0.2, 20000.);
        let target = live
            .entities
            .values()
            .filter(|e| e.spawn.npc && e.spawn.race != 127 && e.spawn.body_type < 66)
            .filter_map(|e| {
                let p = e.position(std::time::Instant::now());
                let clip = matrix * Camera::to_world(p).extend(1.);
                if clip.w <= 0. || clip.w > 250. {
                    return None;
                }
                let ndc = clip.truncate() / clip.w;
                let screen = [(ndc.x * 0.5 + 0.5) * size[0], (0.5 - ndc.y * 0.5) * size[1]];
                let radius = (e.spawn.size * size[1] / clip.w).clamp(16., 100.);
                let dist = (screen[0] - point[0]).powi(2) + (screen[1] - point[1]).powi(2);
                (dist < radius * radius).then_some((e.spawn.id, dist))
            })
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|v| v.0);
        live.set_target(target);
    }
}
