use super::*;
use crate::{bsp_regions::decode, wld::WLD_MAGIC};

fn box_nodes(nodes: &mut Vec<Node>, min: [f64; 3], max: [f64; 3], region: u32) -> u32 {
    let first = nodes.len();
    for axis in 0..3 {
        for (sign, bound) in [(1., min[axis]), (-1., max[axis])] {
            let mut plane = [0.; 4];
            plane[axis] = sign;
            plane[3] = -sign * bound;
            nodes.push(Node {
                plane,
                region: 0,
                children: [nodes.len() as u32 + 2, 0],
            });
        }
    }
    nodes.push(Node {
        plane: [0.; 4],
        region,
        children: [0, 0],
    });
    first as u32 + 1
}

fn fixture(declaration: &[u8], payload_name: bool) -> Vec<u8> {
    let strings = [b"\0".as_slice(), declaration, b"\0"].concat();
    let mut out = Vec::new();
    for word in [WLD_MAGIC, 0x15500, 3, 0, 0, strings.len() as u32, 0] {
        out.extend(word.to_le_bytes());
    }
    out.extend(decode(&strings));
    while out.len() % 4 != 0 {
        out.push(0);
    }
    let mut nodes = Vec::new();
    box_nodes(&mut nodes, [0.; 3], [1.; 3], 1);
    let mut tree = Vec::new();
    tree.extend(0_i32.to_le_bytes());
    tree.extend((nodes.len() as u32).to_le_bytes());
    for node in nodes {
        for value in node.plane {
            tree.extend((value as f32).to_le_bytes());
        }
        for value in [node.region, node.children[0], node.children[1]] {
            tree.extend(value.to_le_bytes());
        }
    }
    let mut region = Vec::new();
    for value in [
        -1_i32,
        0,
        1,
        0,
        if payload_name {
            declaration.len() as i32
        } else {
            0
        },
    ] {
        region.extend(value.to_le_bytes());
    }
    if payload_name {
        region.extend(decode(declaration));
    }
    for (kind, bytes) in [(0x21_u32, tree), (0x22, vec![0; 4]), (0x29, region)] {
        out.extend((bytes.len() as u32).to_le_bytes());
        out.extend(kind.to_le_bytes());
        out.extend(bytes);
    }
    out
}

fn make_box(kind: LiquidKind, center: [f32; 3], half_extents: [f32; 3]) -> LiquidBox {
    LiquidBox {
        kind,
        center,
        half_extents,
        rotation: [0., 0., 0., 1.],
    }
}
fn span(kind: LiquidKind, enter: f32, exit: f32) -> LiquidSpan {
    LiquidSpan { kind, enter, exit }
}
fn near(actual: f32, expected: f32) {
    assert!((actual - expected).abs() < 1e-6, "{actual} != {expected}");
}

#[test]
fn wld_names_and_encoded_declarations_select_only_known_liquids() {
    for payload_name in [false, true] {
        for (tag, kind) in [
            ("WT_ZONE", LiquidKind::Water),
            ("WTN__000", LiquidKind::Water),
            ("WTNTP00255000004", LiquidKind::Water),
            ("LA_ZONE", LiquidKind::Lava),
            ("LAN__000", LiquidKind::Lava),
            ("LANTP00255000004", LiquidKind::Lava),
            ("VWN__000", LiquidKind::FreezingWater),
            ("SLN__000", LiquidKind::OpaqueWater),
        ] {
            let regions = LiquidRegions::from_wld(&fixture(tag.as_bytes(), payload_name)).unwrap();
            assert_eq!(regions.at([0.5; 3]), Some(kind), "{tag}");
            assert!(!regions.is_empty());
        }
        for tag in [
            "DRNTP00255000004",
            "DRN_zone_s_",
            "DRP_ZONE",
            "WATER_ZONE",
            "WTWRONG",
            "UNKNOWN",
        ] {
            assert!(
                LiquidRegions::from_wld(&fixture(tag.as_bytes(), payload_name))
                    .unwrap()
                    .is_empty(),
                "{tag}"
            );
        }
    }
}

#[test]
fn wld_volume_has_bounded_swept_entry_and_exit() {
    let regions = LiquidRegions::from_wld(&fixture(b"WTN_ZONE", true)).unwrap();
    for point in [
        [-0.1, 0.5, 0.5],
        [1.1, 0.5, 0.5],
        [0.5, -0.1, 0.5],
        [0.5, 1.1, 0.5],
        [0.5, 0.5, -100.],
        [0.5, 0.5, 100.],
    ] {
        assert_eq!(regions.at(point), None, "{point:?}");
    }
    let expected = vec![span(LiquidKind::Water, 1. / 3., 2. / 3.)];
    assert_eq!(regions.segment([-1., 0.5, 0.5], [2., 0.5, 0.5]), expected);
    assert_eq!(regions.segment([2., 0.5, 0.5], [-1., 0.5, 0.5]), expected);
    assert_eq!(
        regions.segment([0.5, 0.5, 0.5], [2., 0.5, 0.5]),
        vec![span(LiquidKind::Water, 0., 1. / 3.)]
    );
    assert_eq!(
        regions.segment([-1., 0.5, 0.5], [0.5, 0.5, 0.5]),
        vec![span(LiquidKind::Water, 2. / 3., 1.)]
    );
    // EQEmu treats exact BSP split boundaries as dry. A crossing still has
    // well-defined finite entry and exit; merely running along a face does not.
    assert_eq!(regions.at([0., 0.5, 0.5]), None);
    assert!(regions.segment([0., -1., 0.5], [0., 2., 0.5]).is_empty());
    assert!(regions.segment([-1., 0., 0.5], [0., 1., 0.5]).is_empty());
    assert!(regions.segment([-1., 0.5, -1.], [2., 0.5, -1.]).is_empty());
}

#[test]
fn bsp_disjoint_and_concave_liquids_preserve_dry_gaps() {
    let mut nodes = vec![Node {
        plane: [1., 0., 0., 2.],
        region: 0,
        children: [0, 0],
    }];
    let right = box_nodes(&mut nodes, [0., 0., 0.], [1., 1., 1.], 1);
    let left = box_nodes(&mut nodes, [-4., 0., 0.], [-3., 1., 1.], 2);
    nodes[0].children = [right, left];
    let regions = LiquidRegions::from_bsp(
        nodes,
        2,
        BTreeMap::from([(1, LiquidKind::Water), (2, LiquidKind::Lava)]),
    )
    .unwrap();
    assert_eq!(
        regions.segment([-5., 0.5, 0.5], [2., 0.5, 0.5]),
        vec![
            span(LiquidKind::Lava, 1. / 7., 2. / 7.),
            span(LiquidKind::Water, 5. / 7., 6. / 7.)
        ]
    );
    assert_eq!(regions.at([-2., 0.5, 0.5]), None);

    let mut nodes = vec![Node {
        plane: [1., 0., 0., -1.],
        region: 0,
        children: [0, 0],
    }];
    let right = box_nodes(&mut nodes, [1., 0., 0.], [2., 1., 1.], 1);
    let left = box_nodes(&mut nodes, [0., 0., 0.], [1., 2., 1.], 2);
    nodes[0].children = [right, left];
    let l_shape = LiquidRegions::from_bsp(
        nodes,
        2,
        BTreeMap::from([(1, LiquidKind::Water), (2, LiquidKind::Water)]),
    )
    .unwrap();
    assert_eq!(l_shape.at([1.5, 1.5, 0.5]), None);
    assert_eq!(l_shape.at([0.5, 1.5, 0.5]), Some(LiquidKind::Water));
    assert_eq!(
        l_shape.segment([-1., 0.5, 0.5], [3., 0.5, 0.5]),
        vec![span(LiquidKind::Water, 0.25, 0.75)]
    );
    assert_eq!(
        l_shape.segment([-1., 1.5, 0.5], [3., 1.5, 0.5]),
        vec![span(LiquidKind::Water, 0.25, 0.5)]
    );
}

#[test]
fn finite_boxes_preserve_stacked_rooms_and_overlap_precedence() {
    let regions = LiquidRegions::from_boxes([
        make_box(LiquidKind::Water, [0., 0., 0.], [1.; 3]),
        make_box(LiquidKind::Lava, [0., 0., 4.], [1.; 3]),
    ])
    .unwrap();
    assert_eq!(regions.at([0., 0., 2.]), None);
    assert_eq!(regions.at([0., 0., -100.]), None);
    assert_eq!(
        regions.segment([0., 0., -3.], [0., 0., 7.]),
        vec![
            span(LiquidKind::Water, 0.2, 0.4),
            span(LiquidKind::Lava, 0.6, 0.8)
        ]
    );
    let overlap = LiquidRegions::from_boxes([
        make_box(LiquidKind::Water, [0.; 3], [1.; 3]),
        make_box(LiquidKind::Lava, [1., 0., 0.], [1.; 3]),
    ])
    .unwrap();
    assert_eq!(overlap.at([0.5, 0., 0.]), Some(LiquidKind::Water));
    assert_eq!(overlap.at([1.5, 0., 0.]), Some(LiquidKind::Lava));
    assert_eq!(
        overlap.segment([-2., 0., 0.], [3., 0., 0.]),
        vec![
            span(LiquidKind::Water, 0.2, 0.6),
            span(LiquidKind::Lava, 0.6, 0.8)
        ]
    );
}

#[test]
fn rotated_boxes_clip_in_local_space_without_enlarging_world_bounds() {
    let mut volume = make_box(LiquidKind::Water, [10., 20., 30.], [2., 0.5, 1.]);
    volume.rotation = glam::Quat::from_rotation_z(std::f32::consts::FRAC_PI_2).to_array();
    let regions = LiquidRegions::from_boxes([volume]).unwrap();
    assert_eq!(regions.at([10., 21.5, 30.]), Some(LiquidKind::Water));
    assert_eq!(regions.at([11.5, 20., 30.]), None);
    let spans = regions.segment([10., 16., 30.], [10., 24., 30.]);
    assert_eq!(spans.len(), 1);
    near(spans[0].enter, 0.25);
    near(spans[0].exit, 0.75);
}

#[test]
fn malformed_inputs_and_nonfinite_queries_fail_closed() {
    let data = fixture(b"WT_ZONE", true);
    for end in 0..data.len() {
        assert!(
            LiquidRegions::from_wld(&data[..end]).is_err(),
            "accepted truncation {end}"
        );
    }
    let regions = LiquidRegions::from_wld(&data).unwrap();
    assert_eq!(regions.at([f32::NAN; 3]), None);
    assert!(regions.segment([0.5; 3], [0.5; 3]).is_empty());
    assert!(regions.segment([0.; 3], [f32::INFINITY; 3]).is_empty());
    for node in [
        Node {
            plane: [1., 0., 0., 0.],
            region: 0,
            children: [1, 0],
        },
        Node {
            plane: [1., 0., 0., 0.],
            region: 0,
            children: [2, 0],
        },
        Node {
            plane: [f64::NAN, 0., 0., 0.],
            region: 1,
            children: [0, 0],
        },
        Node {
            plane: [0.; 4],
            region: 2,
            children: [0, 0],
        },
    ] {
        assert!(
            LiquidRegions::from_bsp(vec![node], 1, BTreeMap::from([(1, LiquidKind::Water)]))
                .is_err()
        );
    }
    let leaf = Node {
        plane: [0.; 4],
        region: 1,
        children: [0, 0],
    };
    for plane in [[0.; 4], [1., 0., 0., 0.]] {
        assert!(
            LiquidRegions::from_bsp(
                vec![
                    Node {
                        plane,
                        region: 0,
                        children: [2, 2]
                    },
                    leaf.clone()
                ],
                1,
                BTreeMap::from([(1, LiquidKind::Water)])
            )
            .is_err()
        );
    }
    for volume in [
        make_box(LiquidKind::Water, [f32::NAN, 0., 0.], [1.; 3]),
        make_box(LiquidKind::Water, [0.; 3], [0., 1., 1.]),
        make_box(LiquidKind::Water, [0.; 3], [-1., 1., 1.]),
        LiquidBox {
            rotation: [0.; 4],
            ..make_box(LiquidKind::Water, [0.; 3], [1.; 3])
        },
    ] {
        assert!(LiquidRegions::from_boxes([volume]).is_err());
    }
}

#[test]
fn declarations_reject_missing_regions_and_conflicting_liquid_kinds() {
    let declaration = b"WT_ZONE";
    let mut missing = fixture(declaration, true);
    let body = missing.len() - 20 - declaration.len();
    missing[body + 12..body + 16].copy_from_slice(&1_u32.to_le_bytes());
    assert!(LiquidRegions::from_wld(&missing).is_err());
    missing[body + 12..body + 16].copy_from_slice(&u32::MAX.to_le_bytes());
    assert!(LiquidRegions::from_wld(&missing).is_err());

    let mut conflicting = fixture(declaration, true);
    conflicting[8..12].copy_from_slice(&4_u32.to_le_bytes());
    let mut region = Vec::new();
    for value in [-1_i32, 0, 1, 0, 7] {
        region.extend(value.to_le_bytes());
    }
    region.extend(decode(b"LA_ZONE"));
    conflicting.extend((region.len() as u32).to_le_bytes());
    conflicting.extend(0x29_u32.to_le_bytes());
    conflicting.extend(region);
    assert!(LiquidRegions::from_wld(&conflicting).is_err());
}
