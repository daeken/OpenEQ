//! Medium timing on certified ascending support, and conservative fallbacks.
use super::*;
use openeq_assets::{
    Scene,
    liquid_regions::{LiquidBox, LiquidKind},
    mesh::Geometry,
};

fn ramp(extra: &[[[f32; 3]; 3]]) -> CollisionWorld {
    ramp_with_extent(100., extra)
}

fn ramp_with_extent(extent: f32, extra: &[[[f32; 3]; 3]]) -> CollisionWorld {
    let triangles = std::iter::once([
        [-extent, -100., -extent * 0.5],
        [extent, -100., extent * 0.5],
        [0., 100., 0.],
    ])
    .chain(extra.iter().copied());
    let mut vertices = Vec::new();
    let mut indices = Vec::new();
    for triangle in triangles {
        for point in triangle {
            indices.push((vertices.len() / 8) as u32);
            vertices.extend_from_slice(&[point[0], point[1], point[2], 0., 0., 1., 0., 0.]);
        }
    }
    CollisionWorld::build(&Scene::from_geometry(
        "planar-medium".into(),
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

fn regions(center: [f32; 3], half_extents: [f32; 3]) -> LiquidRegions {
    LiquidRegions::from_boxes([LiquidBox {
        kind: LiquidKind::Water,
        center,
        half_extents,
        rotation: [0., 0., 0., 1.],
    }])
    .unwrap()
}

fn keys() -> MotionInput {
    MotionInput {
        walk_velocity: [40., 0.],
        volume_velocity: [40., 0., 0.],
        jump: false,
        gravity: PlayerGravity::Grounded,
    }
}

fn advance(
    world: &MotionWorld<'_>,
    liquids: &LiquidRegions,
    input: MotionInput,
) -> ([f32; 3], MotionMode) {
    let mut motion = GroundMotion::default();
    let feet = motion.step_in_world(
        MotionWorld {
            collision: world.collision,
            dynamic: world.dynamic,
            liquids: Some(liquids),
        },
        [0.; 3],
        input,
        STEP as f32,
    );
    (feet, motion.mode)
}

#[test]
fn ascending_ramp_spends_time_inside_thin_water_even_with_dry_endpoints() {
    let slope = ramp(&[]);
    let empty = CollisionWorld::default();
    let water = regions([0.15, 0., 3.], [0.05, 10., 3.]);
    for (collision, dynamic) in [(&slope, None), (&empty, Some(&slope))] {
        let world = MotionWorld {
            collision,
            dynamic,
            liquids: None,
        };
        let (feet, mode) = advance(&world, &water, keys());
        assert!((feet[0] - 0.26666668).abs() < 0.00001, "{feet:?}");
        assert!((feet[2] - feet[0] * 0.5).abs() < 0.00001, "{feet:?}");
        assert_eq!(mode, MotionMode::Ground);
        assert!(water.at(center([0.; 3])).is_none());
        assert!(water.at(center(feet)).is_none());
    }
}

#[test]
fn planar_medium_queries_preserve_dry_and_fully_submerged_solver_output() {
    let slope = ramp(&[]);
    let empty = LiquidRegions::default();
    let far = regions([200., 0., 3.], [1.; 3]);
    let wet = regions([0., 0., 3.], [100.; 3]);
    let world = MotionWorld {
        collision: &slope,
        dynamic: None,
        liquids: None,
    };
    assert_eq!(
        advance(&world, &far, keys()),
        advance(&world, &empty, keys())
    );
    let (feet, mode) = advance(&world, &wet, keys());
    let expected = world.move_velocity(
        [0.; 3],
        [24., 0., 0.],
        STEP as f32,
        true,
        MotionMode::Swimming,
    );
    assert_eq!(feet, expected.feet);
    assert_eq!(mode, MotionMode::Swimming);
}

#[test]
fn descending_ramp_and_competing_geometry_keep_original_collision_result() {
    let water = regions([0.15, 0., 3.], [0.05, 10., 3.]);
    let empty = LiquidRegions::default();
    // A lower support remains within the solver's search envelope even though
    // it does not obstruct the body at either endpoint.
    let lower = [[[-100., -100., -0.1], [100., -100., -0.1], [0., 100., -0.1]]];
    for (slope, input, water) in [
        (ramp(&lower), keys(), water),
        (
            ramp(&[]),
            MotionInput {
                walk_velocity: [-40., 0.],
                volume_velocity: [-40., 0., 0.],
                ..keys()
            },
            regions([-0.15, 0., 3.], [0.05, 10., 3.]),
        ),
    ] {
        let world = MotionWorld {
            collision: &slope,
            dynamic: None,
            liquids: None,
        };
        assert_eq!(
            advance(&world, &water, input),
            advance(&world, &empty, input)
        );
    }
}

#[test]
fn rounded_height_staircase_is_explicitly_outside_medium_certificate() {
    let slope = ramp_with_extent(1_000_000., &[]);
    let dt = STEP as f32;
    let velocity = [40., 0., -GRAVITY * dt];
    let delta = velocity.map(|v| v * dt);
    let proof = slope
        .certify_ascending_support(None, [0.; 3], delta, RADIUS, HEIGHT, 2.)
        .unwrap();
    let endpoint = proof.position_for_delta(delta).unwrap();
    let prefix_dt = dt * 0.004;
    let prefix = proof
        .resolve_prefix(velocity.map(|v| v * prefix_dt))
        .unwrap()
        .position;
    let water = regions(center(prefix), [0.001, 10., 0.0001]);
    assert!(water.at(center(prefix)).is_some());
    assert!(water.segment(center([0.; 3]), center(endpoint)).is_empty());
    assert!(!water.height_invariant_in_bounds(center([0.; 3]), center(endpoint)));
    let world = MotionWorld {
        collision: &slope,
        dynamic: None,
        liquids: None,
    };
    assert_eq!(
        advance(&world, &water, keys()),
        advance(&world, &LiquidRegions::default(), keys())
    );
}

#[test]
fn rounded_diagonal_xy_remains_outside_new_planar_medium_scope() {
    let slope = ramp(&[]);
    let world = MotionWorld {
        collision: &slope,
        dynamic: None,
        liquids: None,
    };
    let water = regions([0.15, 0., 3.], [0.05, 10., 3.]);
    let input = MotionInput {
        walk_velocity: [40., 1.],
        volume_velocity: [40., 1., 0.],
        ..keys()
    };
    assert_eq!(
        advance(&world, &water, input),
        advance(&world, &LiquidRegions::default(), input)
    );
}

#[test]
fn planar_boundary_time_inverts_actual_rounded_travel_distance() {
    let vertices = [
        [999_900., -100., -50.],
        [1_000_100., -100., 50.],
        [1_000_000., 100., 0.],
    ]
    .into_iter()
    .flat_map(|p| [p[0], p[1], p[2], 0., 0., 1., 0., 0.])
    .collect();
    let slope = CollisionWorld::build(&Scene::from_geometry(
        "rounded-time".into(),
        vec![],
        vec![Geometry {
            vertices,
            indices: vec![0, 1, 2],
            material: 0,
            collidable: true,
        }],
        vec![],
    ));
    let start = [1_000_000., 0., 0.];
    let dt = STEP as f32;
    let velocity = [40., 0., -GRAVITY * dt];
    let delta = velocity.map(|v| v * dt);
    let proof = slope
        .certify_ascending_support(None, start, delta, RADIUS, HEIGHT, 2.)
        .unwrap();
    let end = proof.position_for_delta(delta).unwrap();
    let water = regions([1_000_000_f32.next_up(), 0., 3.], [0.01, 10., 100.]);
    assert!(water.height_invariant_in_bounds(center(start), center(end)));
    let spans = water.segment(center(start), center(end));
    let boundary = medium_boundary(&spans, false).unwrap();
    let position = |fraction| proof.position_for_delta(velocity.map(|v| v * (dt * fraction)));
    let fraction = support_boundary_fraction(&water, start, end, position, &boundary).unwrap();
    assert_eq!(fraction, 0.09375);
    assert!(
        boundary.fraction > 0.16,
        "endpoint distance must expose the old late boundary"
    );
    assert!(water.at(center(position(fraction).unwrap())).is_some());
    assert!(
        water
            .at(center(position(fraction.next_down()).unwrap()))
            .is_none()
    );
    assert_eq!(
        proof
            .resolve_prefix(velocity.map(|v| v * (dt * fraction)))
            .unwrap()
            .position,
        position(fraction).unwrap()
    );
}

#[test]
fn merged_bsp_wet_spans_with_a_dry_internal_plane_keep_saved_move() {
    let plane_x = f32::from_bits(0x3655_5556);
    // Minimal public WLD input: one BSP tree, one region and one annotation.
    let words = |values: &[u32]| {
        values
            .iter()
            .flat_map(|word| word.to_le_bytes())
            .collect::<Vec<_>>()
    };
    let mut data = words(&[openeq_assets::wld::WLD_MAGIC, 0, 3, 0, 0, 0, 0]);
    let mut tree = words(&[0, 4]);
    for (plane, region, children) in [
        ([1_f32, 0., 0., 0.], 0, [2, 0]),
        ([1., 0., 0., -plane_x], 0, [3, 4]),
        ([0.; 4], 1, [0, 0]),
        ([0.; 4], 1, [0, 0]),
    ] {
        tree.extend(words(&plane.map(f32::to_bits)));
        tree.extend(words(&[region, children[0], children[1]]));
    }
    let mut declaration = words(&[0, 0, 1, 0, 3]);
    declaration.extend([b'W' ^ 0x95, b'T' ^ 0x3a, b'_' ^ 0xc5]);
    for (kind, body) in [(0x21, tree), (0x22, words(&[0])), (0x29, declaration)] {
        data.extend(words(&[body.len() as u32, kind]));
        data.extend(body);
    }
    let water = LiquidRegions::from_wld(&data).unwrap();
    let slope = ramp(&[]);
    let world = MotionWorld {
        collision: &slope,
        dynamic: None,
        liquids: None,
    };
    let empty = LiquidRegions::default();
    let (expected, _) = advance(&world, &empty, keys());
    assert!(water.height_invariant_in_bounds(center([0.; 3]), center(expected)));
    assert!(
        water
            .at(center([plane_x.next_down(), 0., plane_x * 0.5]))
            .is_some()
    );
    assert!(water.at(center([plane_x, 0., plane_x * 0.5])).is_none());
    assert!(
        water
            .at(center([plane_x.next_up(), 0., plane_x * 0.5]))
            .is_some()
    );
    // The interval API merges both wet leaves, but the actual point predicate
    // is not monotone from initial dry to wet through the rounded dry seam.
    assert_eq!(water.segment(center([0.; 3]), center(expected)).len(), 1);
    assert_eq!(advance(&world, &water, keys()).0, expected);
}

#[test]
fn box_gap_hidden_by_rounded_segment_fractions_keeps_saved_move() {
    let box_at = |x, half_x| LiquidBox {
        kind: LiquidKind::Water,
        center: [x, 0., 3.],
        half_extents: [half_x, 1., 1.],
        rotation: [0., 0., 0., 1.],
    };
    let water = LiquidRegions::from_boxes([
        box_at(0.09375, 0.00625),
        box_at(0.10625, f32::from_bits(0x3bcc_cccf)),
    ])
    .unwrap();
    let slope = ramp(&[]);
    let world = MotionWorld {
        collision: &slope,
        dynamic: None,
        liquids: None,
    };
    let (expected, _) = advance(&world, &LiquidRegions::default(), keys());
    let spans = water.segment(center([0.; 3]), center(expected));
    assert_eq!(spans.len(), 2);
    assert_eq!(spans[0].exit, spans[1].enter);
    assert!(water.at(center([0.1, 0., 0.05])).is_none());
    for x in [0.1_f32.next_down(), 0.1_f32.next_up()] {
        assert!(water.at(center([x, 0., x * 0.5])).is_some());
    }
    assert_eq!(advance(&world, &water, keys()).0, expected);
}
