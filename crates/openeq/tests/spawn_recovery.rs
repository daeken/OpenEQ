//! Spawn-only recovery from small safe-point floor penetrations.
use openeq::{
    coordinates,
    movement::{GroundMotion, recover_spawn},
};
use openeq_assets::{Scene, collision::CollisionWorld, loader, mesh::Geometry};

fn world(quads: &[[[f32; 3]; 4]]) -> CollisionWorld {
    let mut vertices = Vec::new();
    let mut indices = Vec::new();
    for points in quads {
        let offset = (vertices.len() / 8) as u32;
        for &[x, y, z] in points {
            vertices.extend_from_slice(&[x, y, z, 0., 0., 1., 0., 0.]);
        }
        indices.extend([0, 1, 2, 0, 2, 3].map(|index| index + offset));
    }
    CollisionWorld::build(&Scene::from_geometry(
        "spawn recovery".into(),
        vec![],
        vec![Geometry {
            vertices,
            indices,
            material: 0,
            collidable: true,
        }],
        vec![],
    ))
}
fn floor(z: f32) -> [[f32; 3]; 4] {
    [
        [-100., -100., z],
        [100., -100., z],
        [100., 100., z],
        [-100., 100., z],
    ]
}

#[test]
fn embedded_spawn_recovers_once_then_walks_normally() {
    let world = world(&[floor(0.)]);
    for penetration in [0.25, 1., 2.] {
        let feet = [3., -7., -penetration];
        assert_eq!(recover_spawn(&world, feet), [3., -7., 0.]);
        let mut position = recover_spawn(&world, feet);
        let mut motion = GroundMotion::default();
        for _ in 0..120 {
            position = motion.step(&world, position, [40., 0.], false, 1. / 120.);
        }
        assert!((position[0] - 43.).abs() < 0.025);
        assert_eq!(position[1], -7.);
        assert_eq!(position[2], 0.);
    }
}

#[test]
fn recovery_preserves_airborne_grounded_gap_and_deeply_embedded_arrivals() {
    let flat = world(&[floor(0.)]);
    for feet in [[0., 0., 0.], [0., 0., 0.25], [0., 0., 8.], [0., 0., -2.1]] {
        assert_eq!(recover_spawn(&flat, feet), feet);
    }
    let gap = world(&[[
        [-100., -100., 0.],
        [-0.1, -100., 0.],
        [-0.1, 100., 0.],
        [-100., 100., 0.],
    ]]);
    // Part of the circular footprint overlaps the edge, but no floor exists at
    // the center. Recovery must not pull a falling arrival up onto that edge.
    let feet = [0., 0., -0.25];
    assert_eq!(recover_spawn(&gap, feet), feet);
}

#[test]
fn recovery_rejects_low_ceiling_and_wall_without_moving_sideways() {
    let feet = [0., 0., -0.25];
    let ceiling = world(&[floor(0.), floor(5.9)]);
    assert_eq!(recover_spawn(&ceiling, feet), feet);
    let wall = world(&[
        floor(0.),
        [
            [0.5, -100., -1.],
            [0.5, 100., -1.],
            [0.5, 100., 10.],
            [0.5, -100., 10.],
        ],
    ]);
    assert_eq!(recover_spawn(&wall, feet), feet);
}

#[test]
#[ignore = "requires original Plane of Knowledge assets; no GPU or server"]
fn poknowledge_safe_arrival_recovers_and_all_wasd_directions_work() {
    let base = loader::default_client_dir().expect("set OPENEQ_CLIENT_DIR to original assets");
    let world = CollisionWorld::build(&loader::load_zone(&base, "poknowledge").unwrap());
    // Storage2 zone safe XYZ is (-285,-148,-159). ZoneEntry usually adds0.75;
    // camera=center+3 and feet=camera-6. The actual asset floor is at -161.
    for server_lift in [0., 0.75] {
        let feet = coordinates::server_point_to_scene([-285., -148., -159. + server_lift - 3.]);
        let recovered = recover_spawn(&world, feet);
        assert_eq!(recovered, [-148., -285., -161.]);
        for fps in [10, 30, 60, 120] {
            for velocity in [[40., 0.], [-40., 0.], [0., 40.], [0., -40.]] {
                let mut motion = GroundMotion::default();
                let mut position = recovered;
                for _ in 0..fps {
                    position = motion.step(&world, position, velocity, false, 1. / fps as f32);
                }
                let distance = (position[0] - recovered[0]).hypot(position[1] - recovered[1]);
                assert!(
                    distance > 30.,
                    "{fps} FPS, velocity{velocity:?}: arrival still stuck at {position:?}"
                );
                assert!(position.iter().all(|value| value.is_finite()));
            }
        }
    }
}
