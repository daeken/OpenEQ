//! Opt-in original-client tests; no proprietary asset bytes are checked in.
use openeq_assets::{
    collision::CollisionWorld,
    liquid_regions::{LiquidKind, LiquidRegions},
    loader,
};

#[test]
#[ignore = "requires original EverQuest zone archives"]
fn poknowledge_pool_matches_original_bsp_and_server_water_map() {
    let base = loader::default_client_dir().expect("original client assets");
    let regions = LiquidRegions::load(&base, "poknowledge").unwrap();
    for point in [[15., 1455., -132.], [-15., 1455., -132.]] {
        assert_eq!(regions.at(point), Some(LiquidKind::Water));
    }
    // These match independently queried EQEmu WTR v1 points. Coordinates here
    // are scene X/Y/Z; EQEmu's server X/Y are exchanged.
    for point in [
        [15., 1455., -125.],
        [15., 1455., -141.],
        [35., 1455., -132.],
        [15., 1439., -132.],
        [15., 1455., -1000.],
        [0., 0., 10.],
    ] {
        assert_eq!(regions.at(point), None, "false liquid at {point:?}");
    }
    let crossing = regions.segment([15., 1455., -150.], [15., 1455., -120.]);
    assert_eq!(crossing.len(), 1);
    assert_eq!(crossing[0].kind, LiquidKind::Water);
    assert!((crossing[0].enter - 1. / 3.).abs() < 1e-6);
    assert!((crossing[0].exit - 0.8).abs() < 1e-6);
    let scene = loader::load_zone(&base, "poknowledge").unwrap();
    let collision = CollisionWorld::build(&scene);
    assert_eq!(
        collision.ground_height(15., 1455., -132., 0., 100.),
        Some(-134.)
    );
}

#[test]
#[ignore = "requires original EverQuest zone archives"]
fn original_wld_water_lava_and_opaque_water_are_distinct() {
    let base = loader::default_client_dir().expect("original client assets");
    for (zone, point, kind) in [
        ("qeynos", [-182.9, -3., -85.], LiquidKind::Water),
        ("soldunga", [-332.9, -1011., 28.], LiquidKind::Lava),
        ("soldunga", [-340.9, -572.8, 2.], LiquidKind::Water),
        (
            "cazicthule",
            [1070.6, 587.2, -63.8],
            LiquidKind::OpaqueWater,
        ),
        ("velketor", [-246.9, 7.2, -56.], LiquidKind::OpaqueWater),
    ] {
        let regions = LiquidRegions::load(&base, zone).unwrap();
        assert_eq!(regions.at(point), Some(kind), "{zone} {point:?}");
        assert_eq!(
            regions.at([point[0], point[1], 10000.]),
            None,
            "{zone} above zone"
        );
        assert_eq!(
            regions.at([point[0], point[1], -10000.]),
            None,
            "{zone} below zone"
        );
    }
    assert!(LiquidRegions::load(&base, "gfaydark").unwrap().is_empty());
}

#[test]
#[ignore = "requires original EverQuest zone archives"]
fn rendered_eqg_and_heightmap_water_never_invent_swimming_volumes() {
    let base = loader::default_client_dir().expect("original client assets");
    for zone in [
        "anguish",
        "crescent",
        "guildhall",
        "wallofslaughter",
        "nektulos",
    ] {
        let regions = LiquidRegions::load(&base, zone).unwrap();
        assert!(
            regions.is_empty(),
            "{zone} has no verified liquid transform yet"
        );
        assert_eq!(regions.at([0., 0., -10000.]), None);
        assert!(
            regions
                .segment([0., 0., 1000.], [0., 0., -1000.])
                .is_empty()
        );
    }
}
