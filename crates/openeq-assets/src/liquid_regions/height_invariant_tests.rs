use super::*;
use crate::{
    binary_regions::BinaryRegions,
    bsp_regions::Node,
    liquid_regions::{LiquidBox, LiquidKind},
    terrain::regions::NativeTopLevelRegions,
};
use std::{collections::BTreeMap, sync::Arc};

fn liquid_box(center: [f32; 3], half_extents: [f32; 3]) -> LiquidBox {
    LiquidBox {
        kind: LiquidKind::Water,
        center,
        half_extents,
        rotation: [0., 0., 0., 1.],
    }
}

fn leaf(region: u32) -> Node {
    Node {
        plane: [0.; 4],
        region,
        children: [0, 0],
    }
}

fn bsp(nodes: Vec<Node>) -> LiquidRegions {
    LiquidRegions::from_bsp(
        nodes,
        2,
        BTreeMap::from([(1, LiquidKind::Water), (2, LiquidKind::Lava)]),
    )
    .unwrap()
}

#[test]
fn identity_boxes_require_invariant_membership_with_inclusive_faces() {
    let regions = LiquidRegions::from_boxes([liquid_box([0.; 3], [1.; 3])]).unwrap();
    assert!(regions.height_invariant_in_bounds([-2., -2., -1.], [2., 2., 1.]));
    assert!(!regions.height_invariant_in_bounds([-0.5, -0.5, -2.], [0.5, 0.5, 2.]));
    assert!(!regions.height_invariant_in_bounds([0., 0., 1.], [0.5, 0.5, 2.]));
    assert!(!regions.height_invariant_in_bounds([0., 0., -2.], [0.5, 0.5, -1.]));
    assert!(regions.height_invariant_in_bounds([0., 0., 1_f32.next_up()], [0.5, 0.5, 2.]));
    assert!(regions.height_invariant_in_bounds([1_f32.next_up(), 0., -2.], [2., 0.5, 2.]));
    assert!(!regions.height_invariant_in_bounds([1., 0., -2.], [2., 0.5, 2.]));
    assert_eq!(regions.at([0., 0., 1.]), Some(LiquidKind::Water));
    assert_eq!(regions.at([0., 0., 1_f32.next_up()]), None);
}

#[test]
fn box_precedence_and_rotations_are_not_erased() {
    let mut lava = liquid_box([0., 0., 1.], [1., 1., 0.5]);
    lava.kind = LiquidKind::Lava;
    let water = liquid_box([0.; 3], [2.; 3]);
    let regions = LiquidRegions::from_boxes([lava, water]).unwrap();
    assert_eq!(regions.at([0.; 3]), Some(LiquidKind::Water));
    assert_eq!(regions.at([0., 0., 1.]), Some(LiquidKind::Lava));
    assert!(!regions.height_invariant_in_bounds([-0.5, -0.5, 0.], [0.5, 0.5, 1.]));
    assert!(regions.height_invariant_in_bounds([-0.5, -0.5, -1.], [0.5, 0.5, 0.]));
    let mut rotated = water;
    rotated.rotation = glam::Quat::from_rotation_z(0.1).to_array();
    let regions = LiquidRegions::from_boxes([rotated]).unwrap();
    assert!(!regions.height_invariant_in_bounds([100.; 3], [101.; 3]));
    let mut negative_identity = water;
    negative_identity.rotation = [-0., 0., -0., -1.];
    let regions = LiquidRegions::from_boxes([negative_identity]).unwrap();
    assert!(regions.height_invariant_in_bounds([-1.; 3], [1.; 3]));
}

#[test]
fn bsp_height_planes_must_strictly_separate_entire_bounds() {
    let regions = bsp(vec![
        Node {
            plane: [0., 0., 1., -1.],
            region: 0,
            children: [2, 3],
        },
        leaf(1),
        leaf(2),
    ]);
    assert!(regions.height_invariant_in_bounds([-2., -2., 2.], [2., 2., 3.]));
    assert!(regions.height_invariant_in_bounds([-2., -2., -2.], [2., 2., 0.]));
    assert!(!regions.height_invariant_in_bounds([-2., -2., 1.], [2., 2., 3.]));
    assert!(!regions.height_invariant_in_bounds([-2., -2., 0.], [2., 2., 1.]));
    assert!(!regions.height_invariant_in_bounds([-2., -2., 0.], [2., 2., 3.]));
    assert_eq!(regions.at([0., 0., 1.]), None);
    let regions = bsp(vec![
        Node {
            plane: [0., 0., 1., -1.],
            region: 0,
            children: [2, 3],
        },
        leaf(1),
        leaf(1),
    ]);
    assert!(!regions.height_invariant_in_bounds([-2., -2., 0.], [2., 2., 3.]));
}

#[test]
fn bsp_xy_splits_inspect_both_relevant_children_and_prune_dry_trees() {
    let regions = bsp(vec![
        Node {
            plane: [1., 0., 0., 0.],
            region: 0,
            children: [2, 5],
        },
        Node {
            plane: [0., 0., 1., 0.],
            region: 0,
            children: [3, 4],
        },
        leaf(1),
        leaf(0),
        leaf(2),
    ]);
    assert!(!regions.height_invariant_in_bounds([-1.; 3], [1.; 3]));
    assert!(regions.height_invariant_in_bounds([-2., -1., -1.], [-1., 1., 1.]));
    let dry_tree = bsp(vec![
        Node {
            plane: [1., 0., 0., 0.],
            region: 0,
            children: [2, 5],
        },
        Node {
            plane: [0., 0., 1., 0.],
            region: 0,
            children: [3, 4],
        },
        leaf(0),
        leaf(0),
        leaf(1),
    ]);
    assert!(dry_tree.height_invariant_in_bounds([-1.; 3], [1.; 3]));
    let vertical_regions = bsp(vec![
        Node {
            plane: [1., -0.5, 0., 0.],
            region: 0,
            children: [2, 3],
        },
        leaf(1),
        leaf(2),
    ]);
    assert!(vertical_regions.height_invariant_in_bounds([-10., -10., -100.], [10., 10., 100.]));
}

#[test]
fn rounded_distance_cancellation_rejects_unproven_side() {
    let plane = [1., 0., 1., -16_777_216.];
    let regions = bsp(vec![
        Node {
            plane,
            region: 0,
            children: [2, 3],
        },
        leaf(1),
        leaf(0),
    ]);
    // Mathematical distance is positive, but below the uncertainty of the
    // actual addition near this coordinate. Require a strict rounded bound.
    assert!(
        !regions.height_invariant_in_bounds([16_777_216., 0., 1e-20], [16_777_216., 0., 2e-20])
    );
    assert!(regions.height_invariant_in_bounds([16_777_216., 0., 1.], [16_777_216., 0., 2.]));
    assert_eq!(regions.at([16_777_216., 0., 1e-20]), None);
}

#[test]
fn large_height_staircase_liquid_layer_rejects() {
    let regions =
        LiquidRegions::from_boxes([liquid_box([0.15, 0., 1027.125], [0.1, 1., 0.0001])]).unwrap();
    assert!(!regions.height_invariant_in_bounds([0., 0., 1027.], [0.33333334, 0., 1027.1667]));
    let tall = LiquidRegions::from_boxes([liquid_box([0.15, 0., 1027.], [0.1, 1., 1.])]).unwrap();
    assert!(tall.height_invariant_in_bounds([0., 0., 1027.], [0.33333334, 0., 1027.1667]));
}

#[test]
fn invalid_bounds_and_native_volumes_do_not_claim_proof() {
    let empty = LiquidRegions::default();
    assert!(empty.height_invariant_in_bounds([-1.; 3], [1.; 3]));
    assert!(!empty.height_invariant_in_bounds([1.; 3], [-1.; 3]));
    assert!(!empty.height_invariant_in_bounds([f32::NAN; 3], [1.; 3]));
    assert!(!empty.height_invariant_in_bounds([-1.; 3], [f32::INFINITY; 3]));
    for volumes in [
        Volumes::NativeBinary(BinaryRegions {
            version: 2,
            boxes: Vec::new(),
        }),
        Volumes::NativeTerrain(NativeTopLevelRegions { boxes: Vec::new() }),
    ] {
        let regions = LiquidRegions {
            volumes: Arc::new(volumes),
        };
        assert!(!regions.height_invariant_in_bounds([-1.; 3], [1.; 3]));
    }
}
