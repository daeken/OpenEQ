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
}

/// Everything that must exist before the first frame can be drawn.
#[derive(Resource)]
struct Runtime {
    instance: Option<wgpu::Instance>,
    renderer: Option<Renderer>,
    scene: Option<GpuScene>,
    camera: Camera,
    size: (u32, u32),
    /// Frames spent waiting for the window to exist.
    attempts: u32,
}

impl Runtime {
    fn new(camera: Camera) -> Self {
        Self {
            instance: None,
            renderer: None,
            scene: None,
            camera,
            size: (0, 0),
            attempts: 0,
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

    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: format!("OpenEQ - {}", options.zone),
                ..default()
            }),
            ..default()
        }))
        .insert_resource(WinitSettings::game())
        .insert_resource(Runtime::new(camera))
        .insert_resource(options)
        // Capture runs first so the camera sees this frame's grab state.
        .add_systems(Update, (handle_cursor_capture, update_camera).chain())
        .add_systems(Last, render_frame)
        .run()
}

fn parse_args() -> anyhow::Result<Options> {
    let mut args = std::env::args().skip(1);
    let mut zone = None;
    let mut dir = None;
    let mut position = None;

    while let Some(arg) = args.next() {
        match arg.as_str() {
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
                println!("usage: openeq <zone> [--dir DIR] [--pos X,Y,Z]");
                std::process::exit(0);
            }
            other if zone.is_none() => zone = Some(other.to_string()),
            other => anyhow::bail!("unrecognised argument {other}"),
        }
    }

    let zone = zone.unwrap_or_else(|| {
        eprintln!("usage: openeq <zone> [--dir DIR] [--pos X,Y,Z]");
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
    let camera = &mut runtime.camera;

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
    if forward == 0.0 && strafe == 0.0 && lift == 0.0 {
        return;
    }

    // Movement is applied in EverQuest space so the camera maths stays simple.
    let (sin_yaw, cos_yaw) = camera.yaw.sin_cos();
    let heading = [sin_yaw, cos_yaw];
    let right = [cos_yaw, -sin_yaw];
    let speed = MOVE_SPEED
        * if keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight) {
            RUN_MULTIPLIER
        } else {
            1.0
        }
        * time.delta_secs();

    let mut position = camera.position;
    position[0] += (heading[0] * forward + right[0] * strafe) * speed;
    position[1] += (heading[1] * forward + right[1] * strafe) * speed;
    position[2] += lift * speed;
    camera.position = position;
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

    if mouse.just_pressed(MouseButton::Left) && !captured {
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

    if runtime.renderer.is_none() {
        // The winit window is created during the first event-loop resume, which
        // can land after the first update; retry until it exists.
        match initialise(&mut runtime, window_entity, &options) {
            Ok(()) => {}
            Err(error) => {
                runtime.attempts += 1;
                if runtime.attempts > 600 {
                    tracing::error!(%error, "renderer initialisation failed");
                    std::process::exit(1);
                }
                tracing::debug!(%error, attempt = runtime.attempts, "waiting for the window");
                return;
            }
        }
    }

    let Ok(window) = window.single() else {
        return;
    };
    let size = (window.physical_width(), window.physical_height());
    if size.0 > 0 && size.1 > 0 && size != runtime.size {
        if let Some(renderer) = runtime.renderer.as_mut() {
            renderer.resize(size.0, size.1);
            runtime.size = size;
            tracing::debug!(width = size.0, height = size.1, "resized");
        }
    }

    // `Camera` is `Copy`, and the renderer is taken out and put back so the
    // borrow checker can see the two accesses as disjoint.
    let camera = runtime.camera;
    if let Some(mut renderer) = runtime.renderer.take() {
        if let Some(scene) = runtime.scene.as_ref() {
            renderer.render(scene, &camera);
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

    tracing::info!(zone = %options.zone, dir = %options.dir.display(), "loading zone");
    let started = std::time::Instant::now();
    let scene = loader::load_zone(&options.dir, &options.zone)?;
    tracing::info!(
        materials = scene.materials.len(),
        triangles = scene.triangle_count(),
        instances = scene.instances.len(),
        lights = scene.lights.len(),
        elapsed_ms = started.elapsed().as_millis() as u64,
        "zone loaded"
    );

    let gpu_scene = GpuScene::build(renderer.device(), renderer.queue(), &scene)?;
    renderer.set_scene(&gpu_scene);

    // Drop the camera into the middle of the zone unless the user said otherwise.
    if options.position.is_none() {
        let center = (gpu_scene.bounds_min + gpu_scene.bounds_max) * 0.5;
        runtime.camera.position = [center.x, center.y, center.z + 80.0];
    }
    runtime.size = (width, height);
    runtime.instance = Some(instance);
    runtime.scene = Some(gpu_scene);
    runtime.renderer = Some(renderer);
    Ok(())
}
