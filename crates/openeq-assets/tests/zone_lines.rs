//! Original-client fixtures are opt-in; no proprietary data is checked in.
use openeq_assets::{
    loader,
    zone_lines::{ZoneLine, ZoneLines},
};

#[test]
#[ignore = "requires original EverQuest zone archives"]
fn gfaydark_authored_borders_match_server_zone_point_numbers() {
    let base = loader::default_client_dir().expect("original client assets");
    let lines = ZoneLines::load(&base, "gfaydark").unwrap();
    assert!(!lines.is_empty());
    // Asset coordinates. Start points are server source anchors with X/Y
    // exchanged; every endpoint is OUTSIDE the narrow authored trigger.
    for (number, from, inside, to, beside, above) in [
        (
            1,
            [-1934., -2597., 23.],
            [-1934., -2645., 23.],
            [-1934., -2670., 23.],
            [-1900., -2645., 23.],
            [-1934., -2645., 100.],
        ), // Felwithe
        (
            2,
            [-2612., -1112., 3.13],
            [-2635., -1112., 3.13],
            [-2660., -1112., 3.13],
            [-2635., -1000., 3.13],
            [-2635., -1112., 200.],
        ), // Lesser Faydark
        (
            3,
            [-1641., 2665., 3.13],
            [-1641., 2684., 3.13],
            [-1641., 2700., 3.13],
            [-1500., 2684., 3.13],
            [-1641., 2684., 200.],
        ), // Butcherblock
        (
            4,
            [2600., -55., 19.],
            [2619., -55., 19.],
            [2640., -55., 19.],
            [2619., 0., 19.],
            [2619., -55., 100.],
        ), // Crushbone
    ] {
        let expected = Some(ZoneLine { number });
        assert_eq!(lines.region_at(from), None, "source anchor {number}");
        assert_eq!(lines.region_at(to), None, "beyond border {number}");
        assert_eq!(lines.region_at(inside), expected, "inside border {number}");
        assert_eq!(lines.crossed(from, to), expected, "swept crossing {number}");
        assert_eq!(
            lines.crossed(to, from),
            expected,
            "reverse crossing {number}"
        );
        assert_eq!(lines.region_at(beside), None, "beside opening {number}");
        assert_eq!(lines.region_at(above), None, "above opening {number}");
        assert_eq!(
            lines.crossed([from[0], from[1], above[2]], [to[0], to[1], above[2]]),
            None,
            "movement above volume {number}"
        );
    }
    assert_eq!(lines.region_at([0., 0., 10.]), None);
    assert_eq!(lines.crossed([0., 0., 10.], [100., 100., 10.]), None);
}

#[test]
#[ignore = "requires original EverQuest zone archives"]
fn adjacent_classic_zones_support_named_and_encoded_region_declarations() {
    let base = loader::default_client_dir().expect("original client assets");
    for (zone, number, from, to) in [
        // Crushbone uses an encoded region payload, not a DRNTP fragment name.
        ("crushbone", 1, [-660., 160., 10.], [-690., 160., 10.]),
        ("butcher", 1, [-1320., -3080., 10.], [-1320., -3120., 10.]),
        ("lfaydark", 2, [2170., -1200., 10.], [2205., -1200., 10.]),
    ] {
        let lines = ZoneLines::load(&base, zone).unwrap();
        assert_eq!(lines.crossed(from, to), Some(ZoneLine { number }), "{zone}");
    }
    // EQG declarations need separately verified transforms; never manufacture
    // crossings from their server destination coordinates or an old S3D.
    assert!(ZoneLines::load(&base, "anguish").unwrap().is_empty());
}
