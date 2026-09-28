//! Conversion at the boundary between EQEmu coordinates and original zone assets.
//!
//! EQEmu's map loader swaps the asset X/Y axes (`zone/map.cpp`). Packets and
//! database positions therefore need one swap before reaching rendering or
//! collision, and the inverse swap when sent back. Both spaces use Z up.
//! Bearings use 512 units per turn and forward `[sin(heading), cos(heading)]`;
//! exchanging axes reflects the bearing and reverses angular velocity.

use openeq_net::zone::Position;

/// Converts a server position or direction vector into asset/scene coordinates.
pub fn server_point_to_scene([x, y, z]: [f32; 3]) -> [f32; 3] {
    [y, x, z]
}

/// Converts a scene position or direction vector into server coordinates.
pub fn scene_point_to_server(point: [f32; 3]) -> [f32; 3] {
    server_point_to_scene(point)
}

/// Converts a server bearing to the scene bearing, in 512 units per turn.
pub fn server_heading_to_scene(heading: f32) -> f32 {
    (128. - heading).rem_euclid(512.)
}

/// Converts a scene bearing to the server bearing, in 512 units per turn.
pub fn scene_heading_to_server(heading: f32) -> f32 {
    server_heading_to_scene(heading)
}

/// Converts one complete network pose, including movement and turning rates.
pub fn server_to_scene(position: Position) -> Position {
    Position {
        x: position.y,
        y: position.x,
        heading: server_heading_to_scene(position.heading),
        velocity: server_point_to_scene(position.velocity),
        delta_heading: -position.delta_heading,
        ..position
    }
}

/// Converts a scene pose for transmission; the axis reflection is involutive.
pub fn scene_to_server(position: Position) -> Position {
    server_to_scene(position)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn near(actual: f32, expected: f32) {
        assert!((actual - expected).abs() < 1e-4, "{actual} != {expected}");
    }

    #[test]
    fn asymmetric_pose_swaps_axes_and_reflects_turning() {
        let server = Position {
            x: 944.,
            y: -315.,
            z: -93.625,
            heading: 32.,
            delta_heading: 2.5,
            velocity: [1.5, -4.25, 0.75],
            animation: 23,
        };
        let scene = server_to_scene(server);
        assert_eq!([scene.x, scene.y, scene.z], [-315., 944., -93.625]);
        assert_eq!(scene.heading, 96.);
        assert_eq!(scene.velocity, [-4.25, 1.5, 0.75]);
        assert_eq!(scene.delta_heading, -2.5);
        assert_eq!(scene.animation, 23);
        let restored = scene_to_server(scene);
        assert_eq!([restored.x, restored.y, restored.z], [944., -315., -93.625]);
        assert_eq!(restored.heading, server.heading);
        assert_eq!(restored.velocity, server.velocity);
        assert_eq!(restored.delta_heading, server.delta_heading);
        assert_eq!(restored.animation, server.animation);
    }

    #[test]
    fn cardinal_and_oblique_bearings_follow_the_transposed_direction() {
        for (server, scene) in [(0., 128.), (128., 0.), (256., 384.), (384., 256.)] {
            assert_eq!(server_heading_to_scene(server), scene);
            assert_eq!(scene_heading_to_server(scene), server);
        }
        for heading in [-32., 23., 173., 402., 560.] {
            let radians = heading * std::f32::consts::TAU / 512.;
            let expected = server_point_to_scene([radians.sin(), radians.cos(), 0.]);
            let scene = server_heading_to_scene(heading) * std::f32::consts::TAU / 512.;
            near(scene.sin(), expected[0]);
            near(scene.cos(), expected[1]);
            near(
                scene_heading_to_server(server_heading_to_scene(heading)),
                heading.rem_euclid(512.),
            );
        }
        // Increasing server bearings turn the opposite way after reflection.
        near(
            server_heading_to_scene(24.) - server_heading_to_scene(20.),
            -4.,
        );
    }

    #[test]
    #[ignore = "requires original Plane of Knowledge zone assets and map labels"]
    fn original_poknowledge_bank_landmark_matches_server_after_conversion() {
        use openeq_assets::{collision::CollisionWorld, loader};
        let base = loader::default_client_dir().expect("original client assets");
        let scene = loader::load_zone(&base, "poknowledge").unwrap();
        let collision = CollisionWorld::build(&scene);
        // Live DB/spawn positions are deliberately far from their transposes.
        for server in [[944., -315., -93.625], [944., -305., -93.625]] {
            let [x, y, z] = server_point_to_scene(server);
            near(collision.ground_height(x, y, z, 5., 10.).unwrap(), -96.);
            assert_eq!(
                collision.ground_height(server[0], server[1], server[2], 1000., 1000.),
                None
            );
        }
        let map = crate::map::ZoneMap::load(&base, "poknowledge").unwrap();
        let bank = map
            .labels
            .iter()
            .find(|label| label.text == "Dogle (Bank)")
            .unwrap();
        assert_eq!(bank.position, [-305., 944., -91.624]);
        near(
            collision
                .ground_height(
                    bank.position[0],
                    bank.position[1],
                    bank.position[2],
                    5.,
                    10.,
                )
                .unwrap(),
            -96.,
        );
    }
}
