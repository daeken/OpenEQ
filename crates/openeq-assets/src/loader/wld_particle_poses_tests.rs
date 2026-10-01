use super::super::{ObjectParticleAttachment, ObjectParticleTexture, ObjectSkeleton, ObjectTrack};
use super::*;
use crate::wld::{ActorDef, ParticleCloud, PieceTrack, Skeleton, Track};

fn packed(words: [i32; 8]) -> Frame {
    Frame {
        rotation: [words[1], words[2], words[3], words[0]].map(|v| v as f32 / 16384.),
        translation: [words[4], words[5], words[6]].map(|v| v as f32 / 256.),
        scale: words[7] as f32 / 256.,
    }
}

fn source(frames: &[Frame], attachments: &[(usize, i32)]) -> ObjectSource {
    let tracks = frames
        .iter()
        .enumerate()
        .map(|(i, _)| Track {
            name: format!("NODE{i}"),
            flags: 0,
            piece_track: Ref(i as i32 + 1),
            mesh: Ref(attachments
                .iter()
                .find(|(owner, _)| *owner == i)
                .map_or(0, |(_, r)| *r)),
            children: if i + 1 < frames.len() {
                vec![i as i32 + 1]
            } else {
                vec![]
            },
        })
        .collect();
    let definition = Skeleton {
        tracks,
        meshes: vec![],
    };
    let (parents, parent_first_order) = hierarchy(&definition).unwrap();
    ObjectSource {
        actor_name: "TORCH_ACTORDEF".into(),
        wld_filename: "fixture.wld".into(),
        actor: ActorDef {
            magic: Ref(0),
            references: vec![Ref(1)],
        },
        skeleton: Some(ObjectSkeleton {
            definition,
            tracks: frames
                .iter()
                .map(|frame| ObjectTrack {
                    definition: PieceTrack {
                        flags: 8,
                        frames: vec![*frame],
                    },
                    reference_flags: 0,
                    speed: None,
                })
                .collect(),
            parents,
            parent_first_order,
        }),
        parts: vec![],
        particle_attachments: attachments
            .iter()
            .map(|&(owner_track, reference)| ObjectParticleAttachment {
                owner_track,
                source_reference: Ref(reference),
                definition_reference: Ref(reference),
                name: "SAME_NAME_PCD".into(),
                definition: ParticleCloud {
                    fixed_words: [0; 20],
                    optional_vectors: None,
                    optional_block: None,
                    texture_reference: None,
                    tail: vec![],
                },
                texture: ObjectParticleTexture {
                    wld_filename: "fixture.wld".into(),
                    source_reference: None,
                    binding: None,
                    animation_reference: None,
                    animation: None,
                    frames: vec![],
                    issues: vec![],
                },
            })
            .collect(),
        render_animation: None,
    }
}

fn identity_instance() -> Instance {
    Instance {
        object: "torch".into(),
        position: [0.; 3],
        rotation: [0., 0., 0., 1.],
        scale: [1.; 3],
    }
}

#[test]
fn particle_owner_composes_placement_parent_scale_and_preserves_attachment_identity() {
    let source = source(
        &[
            packed([16384, 0, 0, 0, 256, 512, 768, 512]),
            packed([8192, 8192, 8192, 8192, 256, 0, 0, 384]),
            packed([16384, 0, 0, 0, 0, 256, 0, 256]),
        ],
        &[(2, 301), (1, 300)],
    );
    let instance = Instance {
        position: [10., 20., 30.],
        rotation: [0.5; 4],
        scale: [1.25; 3],
        ..identity_instance()
    };
    let poses = source
        .diagnostic_particle_owner_transforms(&instance)
        .unwrap();
    // Placement cyclically maps local XYZ to YZX. Its scale multiplies both
    // local scales; translation of the last child uses its rotated parent.
    assert_eq!(
        poses[0].native_world_rows,
        [
            [0., 0., 3.75, 0.],
            [3.75, 0., 0., 0.],
            [0., 3.75, 0., 0.],
            [17.5, 23.75, 32.5, 1.],
        ]
    );
    assert_eq!(poses[1].native_world_rows[3], [13.75, 23.75, 32.5, 1.]);
    assert_eq!(
        poses
            .iter()
            .map(|p| (
                p.attachment_index,
                p.owner_track,
                p.source_reference.0,
                p.definition_reference.0
            ))
            .collect::<Vec<_>>(),
        [(0, 2, 301, 301), (1, 1, 300, 300)]
    );
}

#[test]
fn particle_owner_preserves_raw_packed_quaternion_magnitude_and_first_frame_w() {
    let frame = packed([11585, 11585, 0, 0, 0, 0, 0, 256]);
    let source = source(&[frame], &[(0, 438)]);
    let rows = source
        .diagnostic_particle_owner_transforms(&identity_instance())
        .unwrap()[0]
        .native_world_rows;
    // Native FTORCH302 local matrix: normalization would replace the small
    // diagonal entries with approximately zero and the off-diagonal with one.
    assert_eq!(
        rows,
        [
            [1., 0., 0., 0.],
            [0., 0.000041007996, 0.999959, 0.],
            [0., -0.999959, 0.000041007996, 0.],
            [0., 0., 0., 1.],
        ]
    );
    let mut negative = source.clone();
    negative.skeleton.as_mut().unwrap().tracks[0]
        .definition
        .frames[0]
        .rotation = frame.rotation.map(|v| -v);
    assert_eq!(
        negative
            .diagnostic_particle_owner_transforms(&identity_instance())
            .unwrap()[0]
            .native_world_rows,
        rows
    );
    negative.skeleton.as_mut().unwrap().tracks[0]
        .definition
        .frames[0]
        .rotation[3] *= -1.;
    assert_eq!(
        negative
            .diagnostic_particle_owner_transforms(&identity_instance())
            .unwrap()[0]
            .native_world_rows[1][2],
        -rows[1][2]
    );
}

#[test]
fn particle_owner_rejects_unsupported_placements_without_normalizing() {
    let source = source(&[packed([16384, 0, 0, 0, 0, 0, 0, 256])], &[(0, 20)]);
    for case in 0..10 {
        let mut instance = identity_instance();
        match case {
            0 => instance.object = "other".into(),
            1 => instance.scale = [-1.; 3],
            2 => instance.scale = [-1., -1., 1.],
            3 => instance.scale = [1., 2., 1.],
            4 => instance.scale = [0.; 3],
            5 => instance.scale = [f32::from_bits(1); 3],
            6 => instance.rotation = [0.; 4],
            7 => instance.rotation[3] = 1.001,
            8 => instance.rotation[0] = f32::NAN,
            9 => instance.position[1] = f32::INFINITY,
            _ => unreachable!(),
        }
        assert!(
            source
                .diagnostic_particle_owner_transforms(&instance)
                .is_err(),
            "case {case}"
        );
    }
}

#[test]
fn particle_owner_revalidates_public_hierarchy_tracks_and_attachment_indices() {
    let frame = packed([16384, 0, 0, 0, 0, 0, 0, 256]);
    let original = source(&[frame, frame], &[(1, 20)]);
    for case in 0..19 {
        let mut source = original.clone();
        let sk = source.skeleton.as_mut().unwrap();
        match case {
            0 => sk.tracks.clear(),
            1 => sk.parents.clear(),
            2 => sk.parent_first_order.reverse(),
            3 => sk.definition.tracks[0].children[0] = 1000,
            4 => sk.definition.tracks[1].children.push(0),
            5 => {
                sk.definition.tracks[0].children.clear();
                sk.parents[1] = None;
            }
            6 => sk.definition.tracks[0].flags = 1,
            7 => sk.tracks[0].definition.flags = 0,
            8 => sk.tracks[0].definition.frames.push(frame),
            9 => sk.tracks[0].reference_flags = 4,
            10 => sk.tracks[0].speed = Some(0),
            11 => sk.tracks[0].definition.frames[0].scale = -1.,
            12 => sk.tracks[0].definition.frames[0].rotation[0] = 0.00001,
            13 => sk.tracks[0].definition.frames[0].translation[0] = f32::NAN,
            14 => sk.tracks[0].definition.frames[0].rotation[0] = 1.,
            15 => source.particle_attachments[0].owner_track = usize::MAX,
            16 => source.particle_attachments[0].source_reference = Ref(-20),
            17 => source.particle_attachments[0].definition_reference = Ref(21),
            18 => source
                .particle_attachments
                .push(source.particle_attachments[0].clone()),
            _ => unreachable!(),
        }
        assert!(
            source
                .diagnostic_particle_owner_transforms(&identity_instance())
                .is_err(),
            "case {case}"
        );
    }
}

#[test]
fn particle_owner_rejects_composition_overflow_and_underflow() {
    for (scale, placement_scale) in [(65535, f32::MAX / 2.), (1, f32::MIN_POSITIVE)] {
        let source = source(
            &vec![packed([16384, 0, 0, 0, 0, 0, 0, scale]); 20],
            &[(19, 20)],
        );
        let instance = Instance {
            scale: [placement_scale; 3],
            ..identity_instance()
        };
        assert!(
            source
                .diagnostic_particle_owner_transforms(&instance)
                .is_err()
        );
    }
}

#[test]
fn particle_owner_matches_native_real_placement_matrices() {
    // Facts captured by the original x86 node constructor/composition/update;
    // see WLD_PARTICLE_OWNER_POSES.md. These expected matrices are not computed
    // through the implementation under test. Quaternions are existing Scene
    // placement values, retaining their small native-table approximation error.
    let cases = [
        (
            vec![
                [16384, 0, 0, 0, 0, 0, 0, 256],
                [16384, 0, 0, 0, 0, -370, 1792, 256],
                [16384, 0, 0, 0, 0, 0, 64, 256],
                [8192, 8192, 8192, 8192, 0, 0, 104, 256],
                [16384, 0, 0, 0, 0, 105, 0, 256],
            ],
            vec![(3, 411), (4, 406)],
            [142., 1375.0557, -108.49898],
            [0., 0., 0.70710677, -0.70710677],
            1.,
            [[1., 0., 0., 0.], [0., 0., 1., 0.], [0., -1., 0., 0.]],
            vec![
                [140.55469, 1375.0557, -100.84273],
                [140.55469, 1375.0557, -100.43257],
            ],
        ),
        (
            vec![
                [-16384, 0, 0, 0, 0, 0, 0, 256],
                [-16384, 0, 0, 0, -1548, 11, 6912, 256],
                [-16384, 0, 0, 0, 0, 0, 40, 256],
                [-8192, -8192, -8192, -8192, 0, 1, 97, 256],
            ],
            vec![(3, 20)],
            [925.6532, 150.05615, -151.99896],
            [0., 0., 0.70710677, 0.70710677],
            1.,
            [[-1., 0., 0., 0.], [0., 0., 1., 0.], [0., 1., 0., 0.]],
            vec![[925.6063, 144.00928, -124.463806]],
        ),
        (
            vec![
                [-16384, 0, 0, 0, 0, 0, 0, 256],
                [-16384, 0, 0, 0, 288, 0, 867, 256],
                [-16384, 0, 0, 0, 0, 0, 40, 256],
                [-8192, -8192, -8192, -8192, 0, 1, 30, 256],
            ],
            vec![(3, 20)],
            [-781.74994, 666.5186, -147.99896],
            [0., 0., 1., -4.371139e-8],
            1.25,
            [[0., -1.25, 0., 0.], [0., 0., 1.25, 0.], [-1.25, 0., 0., 0.]],
            vec![[-783.1562, 666.51373, -143.42377]],
        ),
        (
            vec![
                [16384, 0, 0, 0, 0, 0, 0, 256],
                [16384, 0, 0, 0, 2, -1, 1213, 256],
                [16384, 0, 0, 0, 0, -433, 26, 256],
                [11585, 11585, 0, 0, 0, 0, 19, 256],
            ],
            vec![(3, 438)],
            [-334.9683, 755.8504, -94.998985],
            [0., 0., 0.70710677, 0.70710677],
            1.5,
            [
                [0., 1.5, 0., 0.],
                [-f32::from_bits(0x3881_0000), 0., 1.4999385, 0.],
                [1.4999385, 0., f32::from_bits(0x3881_0000), 0.],
            ],
            vec![[-332.42532, 755.8621, -87.62789]],
        ),
    ];
    for (words, attachments, position, rotation, scale, axes, origins) in cases {
        let source = source(
            &words.into_iter().map(packed).collect::<Vec<_>>(),
            &attachments,
        );
        let instance = Instance {
            position,
            rotation,
            scale: [scale; 3],
            ..identity_instance()
        };
        let poses = source
            .diagnostic_particle_owner_transforms(&instance)
            .unwrap();
        assert_eq!(poses.len(), origins.len());
        for (pose, origin) in poses.iter().zip(origins) {
            assert_eq!(
                pose.native_world_rows[3],
                [origin[0], origin[1], origin[2], 1.]
            );
            for (actual, expected) in pose.native_world_rows[..3]
                .iter()
                .flatten()
                .zip(axes.iter().flatten())
            {
                assert!((actual - expected).abs() < 2e-7, "{actual} != {expected}");
            }
        }
    }
}
