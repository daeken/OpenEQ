//! Original-asset GPU galleries for modern faces, armor and held equipment.
//! Run explicitly with --ignored --nocapture; outputs stay outside the repo.
use font8x8::UnicodeFonts;
use image::{Rgba, RgbaImage};
use openeq_assets::{
    Scene,
    character::{CharacterAppearance, CharacterLibrary, CharacterModelSet},
    loader,
    mesh::{Geometry, Material},
    pfs::Archive,
    texture::Texture,
    wld::{Fragment, Wld},
};
use openeq_render::{
    Camera, GpuScene, Renderer,
    actors::{ActorAction, ActorRenderer, ActorState},
    environment::EnvironmentSettings,
};

const WIDTH: u32 = 360;
const HEIGHT: u32 = 440;
const LABEL_HEIGHT: u32 = 40;

fn stage(renderer: &Renderer) -> GpuScene {
    let scene = Scene::from_geometry(
        "appearance gallery stage".into(),
        vec![Material {
            textures: vec!["gallery-floor".into()],
            normal_map: None,
            water: None,
            flags: 0,
            anim_speed: 0,
            alpha_mask: false,
            transparent: false,
            emissive: false,
            clamp_uv: false,
        }],
        vec![Geometry {
            vertices: vec![
                -60., -60., 0., 0., 0., 1., 0., 0., 60., -60., 0., 0., 0., 1., 1., 0., 60., 60.,
                0., 0., 0., 1., 1., 1., -60., 60., 0., 0., 0., 1., 0., 1.,
            ],
            indices: vec![0, 1, 2, 0, 2, 3],
            material: 0,
            collidable: false,
        }],
        vec![Texture {
            name: "gallery-floor".into(),
            width: 1,
            height: 1,
            rgba: vec![112, 121, 133, 255],
        }],
    );
    GpuScene::build(renderer.device(), renderer.queue(), &scene).unwrap()
}

fn label(image: &mut RgbaImage, text: &str, x: u32, y: u32) {
    for (i, character) in text.chars().enumerate() {
        if let Some(bitmap) = font8x8::BASIC_FONTS.get(character) {
            for (row, bits) in bitmap.into_iter().enumerate() {
                for column in 0..8 {
                    if bits & (1 << column) != 0 {
                        for dx in 0..2 {
                            for dy in 0..2 {
                                let px = x + i as u32 * 16 + column * 2 + dx;
                                let py = y + row as u32 * 2 + dy;
                                if px < image.width() && py < image.height() {
                                    image.put_pixel(px, py, Rgba([235, 240, 248, 255]));
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

fn gallery(cards: &[(String, RgbaImage)], path: &str) {
    let columns = 4;
    let rows = (cards.len() as u32).div_ceil(columns);
    let mut image = RgbaImage::from_pixel(
        WIDTH * columns,
        (HEIGHT + LABEL_HEIGHT) * rows,
        Rgba([21, 28, 40, 255]),
    );
    for (index, (title, card)) in cards.iter().enumerate() {
        let x = index as u32 % columns * WIDTH;
        let y = index as u32 / columns * (HEIGHT + LABEL_HEIGHT);
        image::imageops::replace(&mut image, card, i64::from(x), i64::from(y + LABEL_HEIGHT));
        label(&mut image, title, x + 8, y + 12);
    }
    image.save(path).unwrap();
    eprintln!("GALLERY {path}");
}

fn render_card(
    renderer: &mut Renderer,
    actors: &mut ActorRenderer,
    stage: &GpuScene,
    state: &ActorState,
    time: f32,
    portrait: bool,
) -> RgbaImage {
    actors.update(renderer, std::slice::from_ref(state), 0.);
    actors.update(renderer, std::slice::from_ref(state), time);
    assert_eq!(
        actors.rendered_instances, 1,
        "race {} gender {} failed to render",
        state.race, state.gender
    );
    let bounds = actors
        .bounds()
        .get(&state.id)
        .expect("rendered actor bounds");
    assert!(bounds.min.into_iter().chain(bounds.max).all(f32::is_finite));
    assert!(
        bounds.max[2] - bounds.min[2] > 3.,
        "degenerate character bounds: {bounds:?}"
    );
    let center = (bounds.min[2] + bounds.max[2]) * 0.5;
    let camera = if portrait {
        Camera {
            position: [0., -4., bounds.max[2] - 0.65],
            yaw: 0.,
            pitch: 0.,
            fov_y: 34f32.to_radians(),
        }
    } else {
        Camera {
            position: [0., -12., center + 0.8],
            yaw: 0.,
            pitch: -(0.8f32 / 12.).atan(),
            fov_y: 39f32.to_radians(),
        }
    };
    renderer.render_with_actors(stage, &camera, &actors.draws());
    let (w, h, pixels) = renderer.read_rgba().expect("GPU readback");
    let magenta = pixels
        .chunks_exact(4)
        .filter(|p| p[0] > 230 && p[1] < 25 && p[2] > 230)
        .count();
    assert_eq!(
        magenta, 0,
        "missing-texture magenta in race{} gender{}",
        state.race, state.gender
    );
    RgbaImage::from_raw(w, h, pixels).unwrap()
}

fn state(id: u32, race: u32, gender: u8, appearance: CharacterAppearance) -> ActorState {
    ActorState {
        id,
        race,
        gender,
        size: 6.,
        position: [0., 0., 3.75],
        heading: 256.,
        appearance,
        action: ActorAction::Stand,
        ..Default::default()
    }
}

fn changed_pixels(a: &RgbaImage, b: &RgbaImage) -> usize {
    a.pixels()
        .zip(b.pixels())
        .filter(|(a, b)| a.0.iter().zip(b.0).any(|(a, b)| a.abs_diff(b) > 8))
        .count()
}

#[test]
#[ignore = "requires original classic wood elf assets and GPU; writes /tmp/openeq-matrick-outfits.png"]
fn original_classic_matrick_outfits_and_equipment_render() {
    let base = loader::default_client_dir().expect("original client assets");
    let mut actors = ActorRenderer::load(&base, "poknowledge").unwrap();
    let mut renderer = Renderer::new_headless(WIDTH, HEIGHT).expect("GPU");
    renderer.set_environment(
        EnvironmentSettings {
            sky_enabled: false,
            ..Default::default()
        },
        None,
    );
    let stage = stage(&renderer);
    let mut cards = Vec::new();
    for (gender, title, texture, face) in [
        (0, "Male default", 0, 2),
        (0, "Tratlan outfit20", 20, 2),
        (0, "Higwyn outfit21", 21, 3),
        (1, "Sherin outfit21", 21, 1),
    ] {
        let actor = state(
            100,
            4,
            gender,
            CharacterAppearance {
                texture,
                helm_texture: 255,
                face,
                ..Default::default()
            },
        );
        cards.push((
            title.to_owned(),
            render_card(&mut renderer, &mut actors, &stage, &actor, 0., false),
        ));
    }
    assert!(
        changed_pixels(&cards[0].1, &cards[1].1) > 2_000,
        "Tratlan's extended outfit fell back to the default clothing"
    );
    assert!(
        changed_pixels(&cards[1].1, &cards[2].1) > 2_000,
        "distinct Matrick outfits did not render"
    );
    let mut higwyn = state(
        100,
        4,
        0,
        CharacterAppearance {
            texture: 21,
            helm_texture: 255,
            face: 3,
            ..Default::default()
        },
    );
    higwyn.appearance.equipment[1].material = 3;
    higwyn.appearance.equipment[1].color = 0xff4080ff;
    higwyn.appearance.equipment[7].material = 1;
    let equipped = render_card(&mut renderer, &mut actors, &stage, &higwyn, 0., false);
    assert!(
        changed_pixels(&cards[2].1, &equipped) > 500,
        "equipment change failed on extended outfit"
    );
    higwyn.appearance.equipment = Default::default();
    let restored = render_card(&mut renderer, &mut actors, &stage, &higwyn, 0., false);
    assert_eq!(
        cards[2].1, restored,
        "removing equipment did not restore the original outfit"
    );
    gallery(&cards, "/tmp/openeq-matrick-outfits.png");
}

fn verify_attachments(library: &CharacterLibrary, race: u32, gender: u8, sword: &[Geometry]) {
    let mut appearance = CharacterAppearance {
        texture: 3,
        ..Default::default()
    };
    let body = library
        .load_race_with_appearance(race, gender, &appearance)
        .unwrap();
    appearance.equipment[7].material = 1;
    appearance.equipment[8].material = 201;
    let equipped = library
        .load_race_with_appearance(race, gender, &appearance)
        .unwrap();
    assert_eq!(
        body.bounds_min, equipped.bounds_min,
        "held gear changed body scale"
    );
    assert_eq!(
        body.bounds_max, equipped.bounds_max,
        "held gear changed body scale"
    );
    assert!(
        equipped.meshes.len() >= body.meshes.len() + 2,
        "missing primary or shield geometry for race{race} gender{gender}"
    );
    for name in equipped
        .materials
        .iter()
        .flat_map(|material| &material.textures)
    {
        assert!(
            library.texture(name).is_some(),
            "missing original texture {name}"
        );
    }
    assert!(
        equipped.animations.contains_key("C05"),
        "no attack animation for race{race} gender{gender}"
    );
    if race == 1 {
        let socket_name = format!("{}R_POINT_TRACK", equipped.code);
        let socket = equipped
            .bone_names
            .iter()
            .position(|name| *name == socket_name)
            .unwrap();
        for (clip, time) in [("", 0.), ("P01", 0.), ("C05", 0.), ("C05", 0.4)] {
            let transform = equipped.bone_transforms(clip, time, true).unwrap()[socket];
            let pose = equipped.sample(clip, time);
            for (source, attached) in sword.iter().zip(&pose[body.meshes.len()..]) {
                assert_eq!(source.indices, attached.indices);
                assert_eq!(source.vertices.len(), attached.vertices.len());
                for (vertex, placed) in source
                    .vertices
                    .chunks_exact(8)
                    .zip(attached.vertices.chunks_exact(8))
                {
                    let expected = transform.transform_point3(glam::Vec3::from_slice(vertex));
                    let actual = glam::Vec3::from_slice(placed);
                    assert!(
                        expected.distance(actual) < 0.001,
                        "{} sword is not on exact right-hand socket in {clip} at {time}: expected {expected:?}, got {actual:?}",
                        equipped.code
                    );
                }
            }
        }
    }
    let early = equipped.sample("C05", 0.);
    let later = equipped.sample("C05", 0.4);
    for (index, (a, b)) in early.iter().zip(&later).enumerate().skip(body.meshes.len()) {
        let bounds = mesh_bounds(std::slice::from_ref(a));
        eprintln!(
            "GEAR race={race} gender={gender} mesh={index} bind_body={:?}..{:?} attack0_bounds={bounds:?} textures={:?}",
            body.bounds_min, body.bounds_max, equipped.materials[a.material].textures
        );
        assert_eq!(a.indices, b.indices, "held mesh topology changed");
        assert!(
            b.vertices.iter().all(|v| v.is_finite()),
            "invalid attachment transform"
        );
        assert!(
            a.vertices
                .iter()
                .zip(&b.vertices)
                .any(|(a, b)| (a - b).abs() > 0.01),
            "held mesh{index} did not follow attack for race{race} gender{gender}"
        );
    }
}

fn original_sword_geometry(base: &std::path::Path) -> Vec<Geometry> {
    for entry in std::fs::read_dir(base).unwrap() {
        let path = entry.unwrap().path();
        if !path
            .file_name()
            .unwrap()
            .to_string_lossy()
            .starts_with("gequip")
        {
            continue;
        }
        let archive = Archive::open(&path).unwrap();
        for name in archive.names().iter().filter(|name| name.ends_with(".wld")) {
            let wld = Wld::open(&archive, name).unwrap();
            for chunk in wld.chunks() {
                if chunk.name == "IT1_DMSPRITEDEF"
                    && let Fragment::Mesh(mesh) = &chunk.fragment
                {
                    return openeq_assets::mesh::bake_wld_meshes(&wld, [mesh]).1;
                }
            }
        }
    }
    panic!("original IT1 sword mesh missing");
}

fn mesh_bounds(meshes: &[Geometry]) -> ([f32; 3], [f32; 3]) {
    let mut min = [f32::INFINITY; 3];
    let mut max = [f32::NEG_INFINITY; 3];
    for vertex in meshes.iter().flat_map(|m| m.vertices.chunks_exact(8)) {
        for axis in 0..3 {
            min[axis] = min[axis].min(vertex[axis]);
            max[axis] = max[axis].max(vertex[axis]);
        }
    }
    (min, max)
}

fn report_origins(library: &CharacterLibrary, label: &str) {
    for (race, gender) in [(1, 0), (1, 1), (522, 0), (522, 1)] {
        let body = library.load_race(race, gender).unwrap();
        let scale = 6. / (body.bounds_max[2] - body.bounds_min[2]);
        for time in [0., 0.4] {
            let pose = body.sample("P01", time);
            let (min, max) = mesh_bounds(&pose);
            eprintln!(
                "ORIGIN family={label} race={race} gender={gender} body_bounds={:?}..{:?} scale={scale:.6} P01_t={time} posed={min:?}..{max:?} normalized_minz={:.6} world_foot_at_anchor3.75={:.6}",
                body.bounds_min,
                body.bounds_max,
                min[2] * scale,
                3.75 + min[2] * scale
            );
        }
    }
}

#[test]
#[ignore = "requires original Drakkin/Luclin assets and GPU; writes /tmp/openeq-appearance-*.png"]
fn original_modern_appearances_and_animated_equipment_render() {
    let base = loader::default_client_dir().expect("original client assets");
    let mut renderer = Renderer::new_headless(WIDTH, HEIGHT).expect("GPU");
    renderer.set_environment(
        EnvironmentSettings {
            sky_enabled: false,
            ..Default::default()
        },
        None,
    );
    let stage = stage(&renderer);
    let library =
        CharacterLibrary::load_with_model_set(&base, "poknowledge", CharacterModelSet::Luclin)
            .unwrap();
    report_origins(&library, "Luclin");
    let classic_library =
        CharacterLibrary::load_with_model_set(&base, "poknowledge", CharacterModelSet::Classic)
            .unwrap();
    report_origins(&classic_library, "Classic");
    let mut actors =
        ActorRenderer::load_with_model_set(&base, "poknowledge", CharacterModelSet::Luclin)
            .unwrap();
    let mut cards = Vec::new();
    for (index, (title, texture, helm, gender)) in [
        ("M cloth", 0, 0, 0),
        ("M leather", 1, 0, 0),
        ("M chain", 2, 0, 0),
        ("M plate", 3, 0, 0),
        ("M monk", 4, 0, 0),
        ("M robe 10", 10, 0, 0),
        ("M robe 12", 12, 0, 0),
        ("M robe 16", 16, 0, 0),
        ("M plate helm3", 3, 3, 0),
        ("F cloth", 0, 0, 1),
        ("F plate helm3", 3, 3, 1),
        ("F robe 12", 12, 0, 1),
    ]
    .into_iter()
    .enumerate()
    {
        let appearance = CharacterAppearance {
            texture,
            helm_texture: helm,
            ..Default::default()
        };
        let actor = state(index as u32 + 1, 522, gender, appearance);
        let frame = render_card(&mut renderer, &mut actors, &stage, &actor, 0., false);
        cards.push((title.to_owned(), frame));
    }
    gallery(&cards, "/tmp/openeq-appearance-armor.png");
    for index in [1usize, 2, 3, 4, 5, 6, 7, 8] {
        assert!(
            changed_pixels(&cards[0].1, &cards[index].1) > 200,
            "Drakkin armor variant {} unchanged",
            cards[index].0
        );
    }

    let features = [
        ("Default", CharacterAppearance::default()),
        (
            "Face4",
            CharacterAppearance {
                face: 4,
                ..Default::default()
            },
        ),
        (
            "Hair5",
            CharacterAppearance {
                hair_style: 5,
                ..Default::default()
            },
        ),
        (
            "Beard11",
            CharacterAppearance {
                beard: 11,
                ..Default::default()
            },
        ),
        (
            "Eyes3 / 8",
            CharacterAppearance {
                eye_color_1: 3,
                eye_color_2: 8,
                ..Default::default()
            },
        ),
        (
            "Details4",
            CharacterAppearance {
                drakkin_details: 4,
                ..Default::default()
            },
        ),
        (
            "Tattoo3",
            CharacterAppearance {
                drakkin_tattoo: 3,
                ..Default::default()
            },
        ),
        (
            "Combined",
            CharacterAppearance {
                face: 4,
                hair_style: 5,
                beard: 11,
                eye_color_1: 3,
                eye_color_2: 8,
                drakkin_details: 4,
                drakkin_tattoo: 3,
                ..Default::default()
            },
        ),
    ];
    for gender in [0, 1] {
        cards.clear();
        for (index, (title, mut appearance)) in features.into_iter().enumerate() {
            let title = if gender == 1 && title == "Beard11" {
                "Brows3"
            } else {
                title
            };
            if gender == 1 && appearance.beard > 3 {
                appearance.beard = 3;
            }
            let actor = state(
                index as u32 + 20 + u32::from(gender) * 10,
                522,
                gender,
                appearance,
            );
            let frame = render_card(&mut renderer, &mut actors, &stage, &actor, 0., true);
            cards.push((title.to_owned(), frame));
        }
        let path = if gender == 0 {
            "/tmp/openeq-appearance-faces.png"
        } else {
            "/tmp/openeq-appearance-faces-female.png"
        };
        gallery(&cards, path);
        for index in 1..cards.len() {
            assert!(
                changed_pixels(&cards[0].1, &cards[index].1) > 10,
                "Drakkin gender{gender} feature {} unchanged",
                cards[index].0
            );
        }
    }

    cards.clear();
    for gender in [0, 1] {
        for heritage in 0..6 {
            let actor = state(
                300 + u32::from(gender) * 6 + heritage,
                522,
                gender,
                CharacterAppearance {
                    drakkin_heritage: heritage,
                    hair_color: 2,
                    beard_color: 1,
                    hair_style: if gender == 0 { 6 } else { 4 },
                    beard: 1,
                    drakkin_details: 3,
                    drakkin_tattoo: 3,
                    ..Default::default()
                },
            );
            let frame = render_card(&mut renderer, &mut actors, &stage, &actor, 0., true);
            cards.push((
                format!("{} heritage{heritage}", if gender == 0 { "M" } else { "F" }),
                frame,
            ));
        }
    }
    gallery(&cards, "/tmp/openeq-appearance-heritage.png");
    for offset in [0usize, 6] {
        for heritage in 1..6 {
            assert!(
                changed_pixels(&cards[offset].1, &cards[offset + heritage].1) > 100,
                "Drakkin heritage{heritage} at row{offset} has unchanged palette"
            );
        }
    }

    cards.clear();
    for (row, (title, race, gender)) in [
        ("Drakkin M", 522, 0),
        ("Drakkin F", 522, 1),
        ("Luclin HUM", 1, 0),
        ("Luclin HUF", 1, 1),
    ]
    .into_iter()
    .enumerate()
    {
        let mut appearance = CharacterAppearance {
            texture: 3,
            ..Default::default()
        };
        appearance.equipment[7].material = 1;
        appearance.equipment[8].material = 201;
        let mut actor = state(row as u32 + 40, race, gender, appearance);
        let standing = render_card(&mut renderer, &mut actors, &stage, &actor, 0., false);
        cards.push((format!("{title} stand"), standing));
        actor.action = ActorAction::Attack;
        actor.action_sequence += 1;
        let attack_a = render_card(&mut renderer, &mut actors, &stage, &actor, 0.2, false);
        let attack_b = render_card(&mut renderer, &mut actors, &stage, &actor, 0.45, false);
        assert!(
            changed_pixels(&attack_a, &attack_b) > 50,
            "{title} attack is static"
        );
        cards.push((format!("{title} atk.20"), attack_a));
        cards.push((format!("{title} atk.45"), attack_b));
        actor.appearance.equipment[7].material = 0;
        actor.appearance.equipment[8].material = 0;
        let unarmed = render_card(&mut renderer, &mut actors, &stage, &actor, 0.45, false);
        assert!(
            changed_pixels(&cards.last().unwrap().1, &unarmed) > 50,
            "{title} held equipment missing"
        );
        cards.push((format!("{title} no gear"), unarmed));
    }
    gallery(&cards, "/tmp/openeq-appearance-equipment.png");
    cards.clear();
    for gender in [0, 1] {
        let prefix = if gender == 0 { "HUM" } else { "HUF" };
        for (index, (title, texture, features)) in [
            ("default", 0, false),
            ("leather", 1, true),
            ("plate", 3, true),
            ("robe10", 10, true),
        ]
        .into_iter()
        .enumerate()
        {
            let appearance = CharacterAppearance {
                texture,
                face: if features { 3 } else { 0 },
                hair_style: u8::from(features),
                beard: u8::from(features && gender == 0),
                ..Default::default()
            };
            let actor = state(
                200 + u32::from(gender) * 4 + index as u32,
                1,
                gender,
                appearance,
            );
            let frame = render_card(&mut renderer, &mut actors, &stage, &actor, 0., false);
            cards.push((format!("{prefix} {title}"), frame));
        }
    }
    for (index, (title, gender, features)) in [
        ("HUM default face", 0, false),
        ("HUM face3 hair1", 0, true),
        ("HUF default face", 1, false),
        ("HUF face3 hair1", 1, true),
    ]
    .into_iter()
    .enumerate()
    {
        let actor = state(
            220 + index as u32,
            1,
            gender,
            CharacterAppearance {
                face: if features { 3 } else { 0 },
                hair_style: u8::from(features),
                beard: u8::from(features && gender == 0),
                ..Default::default()
            },
        );
        let frame = render_card(&mut renderer, &mut actors, &stage, &actor, 0., true);
        cards.push((title.into(), frame));
    }
    gallery(&cards, "/tmp/openeq-appearance-luclin.png");
    for row in [0usize, 4] {
        for column in 1..4 {
            assert!(
                changed_pixels(&cards[row].1, &cards[row + column].1) > 200,
                "Luclin appearance {} unchanged",
                cards[row + column].0
            );
        }
    }
    cards.clear();
    let mut classic_actors =
        ActorRenderer::load_with_model_set(&base, "poknowledge", CharacterModelSet::Classic)
            .unwrap();
    // IT100044 is a static ram sword with an authored grip at the origin;
    // IT13911 is a shield. Arbitrary IT archives can contain large props.
    for (index, (title, race, gender, classic, primary, secondary, heading)) in [
        ("Classic IT1/201", 1, 0, true, 1, 201, 256.),
        ("Classic EQG gear", 1, 0, true, 100044, 13911, 256.),
        ("DKM EQG gear", 522, 0, false, 100044, 13911, 256.),
        ("DKF EQG gear", 522, 1, false, 100044, 13911, 256.),
        ("Luclin HUM front", 1, 0, false, 1, 201, 256.),
        ("Luclin HUM angled", 1, 0, false, 1, 201, 224.),
        ("Luclin HUF angled", 1, 1, false, 1, 201, 224.),
        ("Luclin HUM EQG", 1, 0, false, 100044, 13911, 224.),
    ]
    .into_iter()
    .enumerate()
    {
        let mut appearance = CharacterAppearance {
            texture: 3,
            ..Default::default()
        };
        appearance.equipment[7].material = primary;
        appearance.equipment[8].material = secondary;
        let mut actor = state(index as u32 + 100, race, gender, appearance);
        actor.heading = heading;
        let actor_renderer = if classic {
            &mut classic_actors
        } else {
            &mut actors
        };
        let frame = render_card(&mut renderer, actor_renderer, &stage, &actor, 0., false);
        cards.push((title.into(), frame));
    }
    gallery(&cards, "/tmp/openeq-appearance-eqg-gear.png");
    let sword = original_sword_geometry(&base);
    for (race, gender) in [(522, 0), (522, 1), (1, 0), (1, 1)] {
        verify_attachments(&library, race, gender, &sword);
    }
}
