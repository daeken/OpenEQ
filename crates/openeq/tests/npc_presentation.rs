//! Original-asset/GPU regression for the models used by PEQ in Greater Faydark.
//! Run with `cargo test -p openeq --test npc_presentation -- --ignored`.

use openeq::targeting::{pick_actor, screen_bounds};
use openeq_assets::{Scene, character::CharacterLibrary, loader};
use openeq_render::{
    Camera, GpuScene, Renderer,
    actors::{ActorBounds, ActorRenderer, ActorState},
};
use openeq_ui::{DrawCommand, Rect, TextAlign, UiFrame};

const SIZE: [u32; 2] = [1280, 720];

fn project(camera: &Camera, size: [f32; 2], point: [f32; 3]) -> [f32; 2] {
    let clip =
        camera.view_projection(size[0] / size[1], 0.2, 20000.) * Camera::to_world(point).extend(1.);
    assert!(clip.w > 0. && clip.z >= 0.);
    [
        (clip.x / clip.w * 0.5 + 0.5) * size[0],
        (0.5 - clip.y / clip.w * 0.5) * size[1],
    ]
}

fn label_rect(camera: &Camera, size: [f32; 2], bounds: &ActorBounds) -> Rect {
    let [left, top, right, _] = screen_bounds(camera, size, bounds).unwrap();
    // Match the logical-pixel nameplate rectangle used by the live client.
    Rect::new((left + right) * 0.5 - 110., top - 22., 220., 20.)
}

#[test]
#[ignore = "requires installed Greater Faydark character assets and a GPU"]
fn actual_animated_npcs_have_visible_models_pickable_extents_and_labels_above_them() {
    let base = loader::default_client_dir().expect("set OPENEQ_CLIENT_DIR to the original client");
    let library = CharacterLibrary::load(&base, "gfaydark").unwrap();
    let mut renderer = Renderer::new_headless(SIZE[0], SIZE[1]).unwrap();
    let empty = Scene::from_geometry("NPC presentation".into(), vec![], vec![], vec![]);
    let scene = GpuScene::build(renderer.device(), renderer.queue(), &empty).unwrap();
    let camera = Camera {
        position: [1., -38., 8.],
        yaw: 0.,
        pitch: 0.,
        fov_y: 43f32.to_radians(),
    };
    renderer.render(&scene, &camera);
    let (_, _, background) = renderer.read_rgba().unwrap();
    let mut actors = ActorRenderer::load(&base, "gfaydark").unwrap();
    let mut states: Vec<_> = [
        (112, 0, 5., [-18., 0., 3.125]),
        (106, 0, 6., [-11., 0., 3.75]),
        (367, 2, 6., [-3., 0., 3.75]),
        (34, 2, 3., [14., 0., 1.875]),
    ]
    .into_iter()
    .map(|(race, gender, size, position)| ActorState {
        id: race,
        race,
        gender,
        size,
        position,
        heading: 256., // Authored +X points toward the camera along scene -Y.
        ..Default::default()
    })
    .collect();
    let models: Vec<_> = states
        .iter()
        .map(|state| library.load_race(state.race, state.gender).unwrap())
        .collect();
    assert_eq!(models[2].code, "SKT");
    let mut first_bat = None;
    let mut captured = false;
    for moving in [false, true] {
        for state in &mut states {
            state.moving = moving;
        }
        for time in [0., 0.2, 0.4, 0.6] {
            actors.update(&renderer, &states, time);
            assert_eq!(actors.rendered_instances, 4);
            assert_eq!(actors.bounds().len(), 4);
            let bat = actors.bounds()[&34];
            assert!(bat.max[2] > states[3].position[2] + states[3].size * 2.);
            assert!(bat.max[0] - bat.min[0] > states[3].size * 2.);
            if let Some(first) = first_bat {
                if time == 0.4 {
                    assert_ne!(bat, first, "bounds retained an old animation pose");
                }
            } else {
                first_bat = Some(bat);
            }
            for (state, model) in states.iter().zip(&models) {
                let clip = if moving {
                    model.walk_animation()
                } else {
                    model.idle_animation()
                };
                let scale = state.size / (model.bounds_max[2] - model.bounds_min[2]);
                let posed = model.sample(clip, time);
                // Independently transform real posed vertices at heading 256.
                // This checks the bounds against the visible geometry, rather
                // than testing picking with points invented from those bounds.
                let vertices: Vec<_> = posed
                    .iter()
                    .flat_map(|mesh| mesh.vertices.chunks_exact(8))
                    .map(|v| {
                        [
                            state.position[0] + v[1] * scale,
                            state.position[1] - v[0] * scale,
                            state.position[2] + v[2] * scale,
                        ]
                    })
                    .collect();
                let bounds = &actors.bounds()[&state.id];
                for point in &vertices {
                    for (axis, coordinate) in point.iter().enumerate() {
                        assert!(
                            *coordinate >= bounds.min[axis] - 0.001
                                && *coordinate <= bounds.max[axis] + 0.001,
                            "{} pose escaped its bounds",
                            model.code
                        );
                    }
                }
                for size in [SIZE.map(|v| v as f32), [640., 360.]] {
                    let label = label_rect(&camera, size, bounds);
                    let mut projected: Vec<_> = vertices
                        .iter()
                        .map(|p| project(&camera, size, *p))
                        .collect();
                    assert!(projected.iter().all(|p| label.bottom() + 1. <= p[1]));
                    // Left/right wing or shoulder extremes, head, feet/body.
                    for axis in [0, 1] {
                        projected.sort_by(|a, b| a[axis].total_cmp(&b[axis]));
                        for point in [projected[0], *projected.last().unwrap()] {
                            assert_eq!(
                                pick_actor(actors.bounds(), None, &camera, size, point),
                                Some(state.id),
                                "{} extent at {time}, moving={moving}",
                                model.code
                            );
                        }
                    }
                    let center = project(&camera, size, bounds.center());
                    assert_eq!(
                        pick_actor(actors.bounds(), None, &camera, size, center),
                        Some(state.id)
                    );
                    assert_ne!(
                        pick_actor(actors.bounds(), Some(state.id), &camera, size, center),
                        Some(state.id)
                    );
                }
            }
            if !moving && time == 0.4 {
                renderer.render_with_actors(&scene, &camera, &actors.draws());
                let (_, _, pixels) = renderer.read_rgba().unwrap();
                for state in &states {
                    let [left, top, right, bottom] =
                        screen_bounds(&camera, SIZE.map(|v| v as f32), &actors.bounds()[&state.id])
                            .unwrap();
                    let mut changed = 0;
                    for y in top.max(0.) as u32..(bottom.ceil() as u32).min(SIZE[1]) {
                        for x in left.max(0.) as u32..(right.ceil() as u32).min(SIZE[0]) {
                            let offset = ((y * SIZE[0] + x) * 4) as usize;
                            changed += usize::from(
                                pixels[offset..offset + 3] != background[offset..offset + 3],
                            );
                        }
                    }
                    assert!(
                        changed > 80,
                        "race {} did not render visible pixels",
                        state.race
                    );
                }
                let viewport = Rect::new(0., 0., SIZE[0] as f32, SIZE[1] as f32);
                let mut frame = UiFrame {
                    bounds: viewport,
                    ..Default::default()
                };
                for (state, name) in states.iter().zip([
                    "Kelethin guard (112)",
                    "Faydark guard (106)",
                    "a decaying skeleton",
                    "a sylvan bat",
                ]) {
                    frame.commands.push(DrawCommand::Text {
                        rect: label_rect(
                            &camera,
                            SIZE.map(|v| v as f32),
                            &actors.bounds()[&state.id],
                        ),
                        clip: viewport,
                        text: name.into(),
                        font: 2,
                        color: [190, 235, 245, 255],
                        align: TextAlign::Center,
                        vertical_center: true,
                        wrap: false,
                    });
                }
                frame.commands.push(DrawCommand::Text {
                    rect: Rect::new(20., 20., 1240., 28.),
                    clip: viewport,
                    text: "Greater Faydark models - animated bounds and nameplates".into(),
                    font: 2,
                    color: [255, 230, 150, 255],
                    align: TextAlign::Center,
                    vertical_center: true,
                    wrap: false,
                });
                renderer.set_ui(&frame);
                renderer.render_with_actors(&scene, &camera, &actors.draws());
                let (width, height, pixels) = renderer.read_rgba().unwrap();
                let path = "/tmp/openeq-gfay-npc-presentation.png";
                image::save_buffer(path, &pixels, width, height, image::ColorType::Rgba8).unwrap();
                eprintln!("wrote {path}");
                captured = true;
            }
        }
    }
    assert!(captured);
    states.retain(|state| state.id != 367);
    actors.update(&renderer, &states, 1.);
    assert_eq!(actors.rendered_instances, 3);
    assert!(!actors.bounds().contains_key(&367));
    actors.update(&renderer, &[], 1.1);
    assert!(actors.bounds().is_empty());
    assert!(actors.draws().is_empty());
    assert_eq!(actors.rendered_instances, 0);
}
