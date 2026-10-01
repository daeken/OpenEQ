use super::*;
use crate::collision::PlayerMovePath;

const DT: f32 = 1. / 120.;
const START: [f32; 3] = [0.; 3];

fn ramp() -> CollisionWorld {
    let mut world = CollisionWorld::default();
    world.add_triangle(
        Triangle::new([
            Vec3::new(-100., -100., -50.),
            Vec3::new(100., -100., 50.),
            Vec3::new(0., 100., 0.),
        ])
        .unwrap(),
    );
    world
}

fn request() -> [f32; 3] {
    (Vec3::new(40., 0., -128. * DT) * DT).to_array()
}

fn certify(world: &CollisionWorld) -> Option<AscendingSupport<'_>> {
    world.certify_ascending_support(None, START, request(), 1., 6., 2.)
}

fn horizontal(world: &mut CollisionWorld, z: f32) {
    world.add_triangle(
        Triangle::new([
            Vec3::new(-100., -100., z),
            Vec3::new(100., -100., z),
            Vec3::new(0., 100., z),
        ])
        .unwrap(),
    );
}

#[test]
fn source_plane_matches_exact_prefix_calls_and_keeps_classification() {
    let world = ramp();
    let empty = CollisionWorld::default();
    for (static_world, dynamic) in [(&world, None), (&empty, Some(&world))] {
        let before = static_world.move_player_with_path(dynamic, START, request(), 1., 6., 2.);
        let proof = static_world
            .certify_ascending_support(dynamic, START, request(), 1., 6., 2.)
            .expect("isolated ascending support");
        assert_eq!(before.path, PlayerMovePath::Deflected);
        assert_eq!(before.position, [0.33333334, 0., 0.16666794]);
        let velocity = Vec3::new(40., 0., -128. * DT);
        let mut different_multiplication_order = 0;
        for index in 0..=1000 {
            let fraction = index as f32 / 1000.;
            let prefix = (velocity * (DT * fraction)).to_array();
            different_multiplication_order += usize::from(
                prefix.map(f32::to_bits)
                    != ((velocity * DT) * fraction).to_array().map(f32::to_bits),
            );
            let expected = static_world.move_player_with_path(dynamic, START, prefix, 1., 6., 2.);
            assert_eq!(
                proof.position_for_delta(prefix).unwrap().map(f32::to_bits),
                expected.position.map(f32::to_bits),
                "prefix {index}"
            );
            let replay = proof.resolve_prefix(prefix).unwrap();
            assert_eq!(
                replay.position.map(f32::to_bits),
                expected.position.map(f32::to_bits)
            );
            assert_eq!(replay.path, expected.path);
        }
        assert!(different_multiplication_order > 0);
        let after = static_world.move_player_with_path(dynamic, START, request(), 1., 6., 2.);
        assert_eq!(
            before.position.map(f32::to_bits),
            after.position.map(f32::to_bits)
        );
        assert_eq!(before.path, after.path);
    }
}

#[test]
fn exact_prefix_domain_and_short_request_cutoff_are_preserved() {
    let world = ramp();
    let proof = certify(&world).unwrap();
    for prefix in [[0.; 3], [1e-8, 0., 0.]] {
        assert_eq!(proof.position_for_delta(prefix), Some(START));
        assert_eq!(proof.resolve_prefix(prefix).unwrap().position, START);
    }
    // The proof covers the whole component rectangle, including non-collinear
    // rounded requests, rather than recovering a fraction from a chosen axis.
    for prefix in [[0.1, 0., request()[2]], [request()[0], 0., 0.]] {
        assert!(proof.resolve_prefix(prefix).is_some());
    }
    for prefix in [
        [-0.001, 0., 0.],
        [request()[0].next_up(), 0., 0.],
        [0., 0.001, 0.],
        [0., 0., 0.001],
        [0., 0., request()[2].next_down()],
        [f32::NAN, 0., 0.],
    ] {
        assert!(proof.position_for_delta(prefix).is_none());
        assert!(proof.resolve_prefix(prefix).is_none());
    }
}

#[test]
fn descending_airborne_multiple_substeps_and_invalid_inputs_reject() {
    let world = ramp();
    for (start, delta, radius, height, step) in [
        (START, [-request()[0], 0., request()[2]], 1., 6., 2.),
        ([0., 0., 0.001], request(), 1., 6., 2.),
        (START, [0.6, 0., -0.01], 1., 6., 2.),
        (START, [1000., 0., -0.01], 1., 6., 2.),
        (START, [0.1, 0., 0.01], 1., 6., 2.),
        (START, [0.; 3], 1., 6., 2.),
        (START, request(), 0., 6., 2.),
        (START, request(), 1., 0.02, 2.),
        (START, request(), 1., 6., 0.),
        (START, request(), 1., 6., f32::INFINITY),
        (START, [f32::NAN, 0., 0.], 1., 6., 2.),
    ] {
        assert!(
            world
                .certify_ascending_support(None, start, delta, radius, height, step)
                .is_none()
        );
    }
}

#[test]
fn full_support_query_envelope_rejects_lower_floors_and_ceilings() {
    // Lower surfaces do not intersect the body, but can compete for the
    // closest-height ground selection all the way down the drop interval.
    for z in [-3., -0.02, 0.05, 2., 6.1] {
        let mut world = ramp();
        horizontal(&mut world, z);
        assert!(certify(&world).is_none(), "competing height {z}");
        let world = ramp();
        let mut dynamic = CollisionWorld::default();
        horizontal(&mut dynamic, z);
        assert!(
            world
                .certify_ascending_support(Some(&dynamic), START, request(), 1., 6., 2.)
                .is_none()
        );
    }
    // A distant layer cannot touch the body or win support for any prefix.
    let mut world = ramp();
    horizontal(&mut world, -10.);
    horizontal(&mut world, 20.);
    assert!(certify(&world).is_some());
}

#[test]
fn wall_slide_and_support_edge_have_no_certificate() {
    let mut world = ramp();
    world.add_triangle(
        Triangle::new([
            Vec3::new(1.1, -20., 0.),
            Vec3::new(1.1, 20., 0.),
            Vec3::new(1.1, 20., 20.),
        ])
        .unwrap(),
    );
    assert!(certify(&world).is_none());

    let mut narrow = CollisionWorld::default();
    narrow.add_triangle(
        Triangle::new([
            Vec3::new(-1., -1., -0.5),
            Vec3::new(1., -1., 0.5),
            Vec3::new(0., 1., 0.),
        ])
        .unwrap(),
    );
    // Center remains on the ramp, but the full footprint crosses its edges.
    let moved = narrow.move_player_with_path(None, START, request(), 1., 6., 2.);
    assert!(moved.position[2] > 0.);
    assert!(certify(&narrow).is_none());
}

#[test]
fn opposite_winding_negative_axis_and_clamped_parameters_work() {
    let mut world = CollisionWorld::default();
    world.add_triangle(
        Triangle::new([
            Vec3::new(-100., -100., 50.),
            Vec3::new(0., 100., 0.),
            Vec3::new(100., -100., -50.),
        ])
        .unwrap(),
    );
    let delta = [-request()[0], 0., request()[2]];
    let proof = world
        .certify_ascending_support(None, START, delta, 1., 6., 200.)
        .unwrap();
    for index in 0..=100 {
        assert!(
            proof
                .resolve_prefix((Vec3::from(delta) * (index as f32 / 100.)).to_array())
                .is_some()
        );
    }
    let tiny = world
        .certify_ascending_support(None, START, [-0.005, 0., -0.0001], 0.001, 6., 200.)
        .unwrap();
    assert!(tiny.resolve_prefix([-0.0025, 0., -0.00005]).is_some());
}

#[test]
fn full_two_axis_prefix_rectangle_matches_translated_source_planes() {
    for offset in [Vec3::ZERO, Vec3::new(8192., -4096., 1024.)] {
        for sign_x in [-1., 1.] {
            for sign_y in [-1., 1.] {
                for reversed in [false, true] {
                    let mut world = CollisionWorld::default();
                    let mut points = [(-100., -100.), (100., -100.), (0., 100.)].map(|(x, y)| {
                        offset + Vec3::new(x, y, 0.3 * sign_x * x + 0.4 * sign_y * y)
                    });
                    if reversed {
                        points.swap(1, 2);
                    }
                    let triangle = Triangle::new(points).unwrap();
                    let start = [
                        offset.x,
                        offset.y,
                        triangle.plane_z(offset.truncate()).unwrap(),
                    ];
                    world.add_triangle(triangle);
                    let delta = [0.2 * sign_x, 0.15 * sign_y, -0.01];
                    let proof = world
                        .certify_ascending_support(None, start, delta, 1., 6., 2.)
                        .unwrap();
                    // All combinations are covered, including independently
                    // rounded axes and downward requests unrelated to XY time.
                    for x in 0..=10 {
                        for y in 0..=10 {
                            for z in [0., delta[2]] {
                                let prefix =
                                    [delta[0] * (x as f32 / 10.), delta[1] * (y as f32 / 10.), z];
                                assert!(
                                    proof.resolve_prefix(prefix).is_some(),
                                    "{start:?} {delta:?} {prefix:?}"
                                );
                            }
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn mixed_axis_height_cancellation_and_uncertain_coverage_reject() {
    let mut world = CollisionWorld::default();
    world.add_triangle(
        Triangle::new([
            Vec3::new(-100., -100., -100.),
            Vec3::new(100., -100., 0.),
            Vec3::new(0., 100., 50.),
        ])
        .unwrap(),
    );
    assert!(
        world
            .certify_ascending_support(None, START, [0.3, -0.1, -0.01], 1., 6., 2.)
            .is_none()
    );
    let mut thin = CollisionWorld::default();
    thin.add_triangle(
        Triangle::new([
            Vec3::new(-100., -0.00001, -50.),
            Vec3::new(100., -0.00001, 50.),
            Vec3::new(0., 0.00001, 0.),
        ])
        .unwrap(),
    );
    assert!(certify(&thin).is_none());
}
