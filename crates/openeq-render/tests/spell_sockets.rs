//! Spell emitters must follow the same posed skeleton as the visible actor.
use glam::{Mat4, Quat, Vec3};
use openeq_assets::{
    character::{CharacterLibrary, CharacterModel, CharacterModelSet, CharacterPose},
    pfs::Archive,
    zone::TerMod,
};
use openeq_render::{
    Renderer,
    actors::{ActorAction, ActorRenderer, ActorSockets, ActorState},
};

fn exact_bone(model: &CharacterModel, suffix: &str) -> usize {
    model
        .bone_names
        .iter()
        .position(|name| name == suffix || name == &format!("{}{suffix}", model.code))
        .unwrap_or_else(|| panic!("{} missing {suffix}", model.code))
}

fn instance(state: &ActorState, model: &CharacterModel) -> Mat4 {
    let scale = state.size / (model.bounds_max[2] - model.bounds_min[2]);
    Mat4::from_scale_rotation_translation(
        Vec3::splat(scale),
        Quat::from_rotation_z(
            std::f32::consts::FRAC_PI_2 - state.heading * std::f32::consts::TAU / 512.,
        ),
        Vec3::from(state.position),
    )
}

fn close(actual: [f32; 3], expected: Vec3, context: &str) {
    assert!(
        (Vec3::from(actual) - expected).length() < 0.0002,
        "{context}: {actual:?} != {expected:?}"
    );
}

fn assert_hands(
    sockets: ActorSockets,
    model: &CharacterModel,
    state: &ActorState,
    bones: &[Mat4],
    center_z: f32,
    modern: bool,
) {
    let model_to_scene =
        instance(state, model) * Mat4::from_translation(Vec3::new(0., 0., -center_z));
    for (socket, name) in [
        (
            sockets.left_hand,
            if modern { "ARML_WEAP" } else { "L_POINT_TRACK" },
        ),
        (
            sockets.right_hand,
            if modern { "ARMR_WEAP" } else { "R_POINT_TRACK" },
        ),
    ] {
        let bone = exact_bone(model, name);
        close(
            socket.expect("humanoid hand socket"),
            (model_to_scene * bones[bone]).transform_point3(Vec3::ZERO),
            name,
        );
    }
}

#[test]
#[ignore = "requires original classic/Luclin/Drakkin assets and GPU"]
fn original_character_spell_sockets_follow_scale_heading_pose_and_despawn() {
    let base = openeq_assets::loader::default_client_dir().expect("original client directory");
    let renderer = Renderer::new_headless(64, 64).unwrap();
    for (family, race, modern) in [
        (CharacterModelSet::Classic, 1, false),
        (CharacterModelSet::Luclin, 1, false),
        (CharacterModelSet::Classic, 522, true),
    ] {
        let library = CharacterLibrary::load_with_model_set(&base, "poknowledge", family).unwrap();
        let model = library.load_race(race, 0).unwrap();
        let mut actors = ActorRenderer::load_with_model_set(&base, "poknowledge", family).unwrap();
        let center_z = if modern {
            // Derive the body recentering independently from the original MOD,
            // rather than using the attachment API that this test validates.
            let archive = Archive::open(base.join("dkm.eqg")).unwrap();
            let source = TerMod::parse(&archive.read("dkm.mod").unwrap(), false).unwrap();
            let min = source
                .positions
                .iter()
                .map(|p| p[2])
                .fold(f32::INFINITY, f32::min);
            let max = source
                .positions
                .iter()
                .map(|p| p[2])
                .fold(f32::NEG_INFINITY, f32::max);
            (min + max) * 0.5
        } else {
            0.
        };
        let states: Vec<_> = [0., 128., 256., 384.]
            .into_iter()
            .enumerate()
            .flat_map(|(i, heading)| {
                [3., 12.]
                    .into_iter()
                    .enumerate()
                    .map(move |(j, size)| ActorState {
                        id: (i * 2 + j + 1) as u32,
                        race,
                        size,
                        position: [11., -7., 3.],
                        heading,
                        action: ActorAction::Stand,
                        ..Default::default()
                    })
            })
            .collect();
        actors.update(&renderer, &states, 0.);
        assert_eq!(actors.sockets().len(), states.len());
        let bind = model.bone_transforms("P01", 0., true).unwrap();
        for state in &states {
            assert_hands(
                actors.sockets()[&state.id],
                &model,
                state,
                &bind,
                center_z,
                modern,
            );
        }
        if family == CharacterModelSet::Luclin {
            let hair = exact_bone(&model, "HAIR_POINT_TRACK");
            let hair = (instance(&states[0], &model) * bind[hair]).transform_point3(Vec3::ZERO);
            assert!(
                (Vec3::from(actors.sockets()[&states[0].id].right_hand.unwrap()) - hair).length()
                    > 0.5,
                "R_POINT suffix accidentally selected HAIR_POINT"
            );
        }

        let mut state = states[0].clone();
        actors.update(&renderer, std::slice::from_ref(&state), 0.2);
        let before = actors.sockets()[&state.id];
        state.action = ActorAction::Cast;
        state.action_sequence = 1;
        actors.update(&renderer, std::slice::from_ref(&state), 0.2);
        let started = actors.sockets()[&state.id];
        close(
            started.right_hand.unwrap(),
            Vec3::from(before.right_hand.unwrap()),
            "transition start continuity",
        );
        close(
            started.left_hand.unwrap(),
            Vec3::from(before.left_hand.unwrap()),
            "transition start continuity",
        );
        actors.update(&renderer, std::slice::from_ref(&state), 0.4);
        let finished = actors.sockets()[&state.id];
        let cast = model.bone_transforms("T05", 0.2, false).unwrap();
        assert_hands(finished, &model, &state, &cast, center_z, modern);
        assert!(
            (Vec3::from(finished.right_hand.unwrap()) - Vec3::from(before.right_hand.unwrap()))
                .length()
                > 0.01,
            "{family:?} race{race} casting hand did not move"
        );

        let from = CharacterPose {
            animation: "P01",
            time_seconds: 0.2,
            looping: true,
        };
        let to = CharacterPose {
            animation: "T05",
            time_seconds: 0.2,
            looping: false,
        };
        for (blend, pose) in [(0., from), (1., to)] {
            let raw = model
                .bone_transforms(pose.animation, pose.time_seconds, pose.looping)
                .unwrap();
            let attachments = model.attachment_transforms(from, to, blend).unwrap();
            for (a, b) in attachments.iter().zip(raw) {
                assert!(
                    a.abs_diff_eq(
                        Mat4::from_translation(Vec3::new(0., 0., -center_z)) * b,
                        0.0001
                    ),
                    "{family:?} race{race} attachment blend endpoint disagrees with visible skeleton"
                );
            }
        }
        // Every frame replaces socket ownership; cached pose batches must not
        // leave emission anchors behind after an actor has despawned.
        actors.update(&renderer, &[], 0.5);
        assert!(actors.sockets().is_empty());
        assert_eq!(actors.rendered_instances, 0);
    }
}
