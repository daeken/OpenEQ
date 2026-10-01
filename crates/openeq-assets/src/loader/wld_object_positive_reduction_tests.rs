use super::*;
use positive_reduction::FortyFrameKeys;

fn accepted_frames() -> Vec<Frame> {
    [
        [-4125, -8292, -13416, 1378],
        [-4025, -7732, -13593, 2476],
        [-3839, -7671, -13405, 3768],
        [-4007, -7700, -12993, 4903],
        [3779, 7352, 12747, -6057],
        [3580, 7442, 12141, -7168],
        [-3226, -7072, -11745, 8322],
        [-3285, -6599, -10943, 9696],
        [3057, 6194, 10612, -10354],
        [2804, 5821, 9760, -11396],
        [-2517, -5433, -8978, 12277],
        [-2300, -4695, -7926, 13297],
        [-1882, -4163, -7112, 14009],
        [-1678, -3444, -6094, 14675],
        [1581, 3096, 5244, -15105],
        [1119, 2485, 4182, -15556],
        [924, 1895, 3129, -15896],
        [655, 1428, 2438, -16101],
        [-260, -525, -904, 16322],
        [-64, -139, -224, -16350],
        [223, 511, 851, 16317],
        [-650, -1312, -2240, -16143],
        [923, 1859, 3117, 15918],
        [-1050, -2396, -3962, -15672],
        [1328, 3055, 5029, 15205],
        [1624, 3518, 6208, 14631],
        [-2009, -4201, -6978, -14028],
        [2331, 4666, 7995, 13279],
        [2433, 5306, 8951, 12372],
        [2788, 5598, 9844, 11441],
        [-2909, -6014, -10232, -10900],
        [-3319, -6505, -11107, -9526],
        [-3365, -6574, -11668, -8731],
        [3470, 7014, 12175, 7620],
        [3788, 7155, 12717, 6405],
        [4056, 7762, 12946, 4861],
        [3730, 8061, 13153, 3993],
        [-3619, -7899, -13570, -2797],
        [-3981, -8060, -13604, -1350],
        [-4036, -8151, -13579, -129],
    ]
    .map(|words| {
        let mut value = frame([0.; 3], 1.);
        value.rotation = words.map(|word| word as f32 / 16384.);
        value
    })
    .to_vec()
}

fn stale_slot_frames() -> Vec<Frame> {
    [
        [-10701, -6688, 9931, 3078],
        [10434, 6870, -9622, -4377],
        [-10009, -6774, 9259, 5973],
        [9767, 6344, -9162, -6880],
        [-9634, -6099, 8646, 7929],
        [8766, 5509, -8377, -9463],
        [-8281, -5617, 7831, 10291],
        [7801, 5178, -7445, -11143],
        [-7367, -4714, 6915, 11987],
        [6513, 4485, -6283, -12886],
        [-5678, -3725, 5587, 13768],
        [5160, 3518, -4779, -14355],
        [4383, 3013, -4115, -14903],
        [-3806, -2501, 3536, 15324],
        [-2785, -1928, 2616, 15807],
        [-2049, -1360, 1936, 16063],
        [1178, 781, -1126, -16235],
        [511, 318, -489, -16340],
        [657, 447, -644, 16322],
        [-1565, -1052, 1464, -16180],
        [2310, 1453, -2131, 15992],
        [2888, 1788, -2679, 15770],
        [-3882, -2468, 3634, -15286],
        [4758, 3064, -4259, 14725],
        [-5426, -3451, 4984, -14207],
        [6248, 3875, -5692, 13462],
        [-6850, -4271, 6224, -12797],
        [7189, 4936, -7021, 11897],
        [-7882, -5259, 7436, -11080],
        [-8639, -5466, 8190, -9798],
        [-9010, -5964, 8436, -8936],
        [9201, 6013, -9071, 8057],
        [-9639, -6413, 9322, -6792],
        [10143, 6368, -9828, 5225],
        [-10389, -6813, 9663, -4487],
        [-10476, -6735, 10192, -3021],
        [-10866, -6835, 9959, -1692],
        [-10805, -7144, 10001, -233],
        [10745, 7036, -10065, -962],
        [-10574, -6931, 10152, 2101],
    ]
    .map(|words| {
        let mut value = frame([0.; 3], 1.);
        value.rotation = words.map(|word| word as f32 / 16384.);
        value
    })
    .to_vec()
}

fn accepted_source() -> ObjectSource {
    let mut source = short_animation_source(false);
    let skeleton = source.skeleton.as_mut().unwrap();
    skeleton.tracks[0].definition.frames[0] = frame([0.; 3], 1.);
    skeleton.tracks[1].definition.frames = accepted_frames();
    skeleton.tracks[1].speed = Some(1);
    for polygon in &mut source.parts[0].mesh.polygons {
        polygon.collidable = false;
    }
    source
}

#[test]
fn certified_native_positive_scores_keep_five_omissions_and_timing() {
    let frames = accepted_frames();
    let keys = FortyFrameKeys::new(&frames, 1).unwrap();
    assert_eq!(keys.omitted, [34, 3, 23, 7, 30]);
    let source = accepted_source();
    assert_eq!(
        source.animation_period().unwrap(),
        Duration::from_millis(40)
    );
    assert_eq!(
        source.sample_animation(Duration::ZERO).unwrap()[0].vertices,
        source.sample_animation(Duration::from_millis(40)).unwrap()[0].vertices
    );
    let radius = source.stationary_collision_animation_radius().unwrap();
    for time in 0..40 {
        for mesh in source
            .sample_animation(Duration::from_millis(time))
            .unwrap()
        {
            assert!(
                mesh.vertices
                    .iter()
                    .all(|point| Vec3::from(*point).length() <= radius)
            );
        }
    }
    let mut flipped = frames;
    for index in [0, 7, 22, 39] {
        flipped[index].rotation = flipped[index].rotation.map(|v| -v);
    }
    assert_eq!(
        FortyFrameKeys::new(&flipped, 1).unwrap().omitted,
        keys.omitted
    );
}

#[test]
fn positive_reduction_rejects_stale_native_slot_and_uncertain_scores() {
    assert!(
        FortyFrameKeys::new(&stale_slot_frames(), 100)
            .unwrap_err()
            .to_string()
            .contains("saved right")
    );
    let identity = vec![frame([0.; 3], 1.); 40];
    assert!(
        FortyFrameKeys::new(&identity, 100)
            .unwrap_err()
            .to_string()
            .contains("uncertain")
    );
    let mut source = accepted_source();
    source.parts[0].mesh.polygons[0].collidable = true;
    assert!(
        source
            .stationary_collision_animation_radius()
            .unwrap_err()
            .to_string()
            .contains("moves collision ancestry")
    );
    let mut frames = accepted_frames();
    frames[1].translation[0] = 1.;
    assert!(FortyFrameKeys::new(&frames, 1).is_err());
    assert!(FortyFrameKeys::new(&frames[..39], 1).is_err());
    assert!(FortyFrameKeys::new(&accepted_frames(), 0).is_err());
    assert!(FortyFrameKeys::new(&accepted_frames(), (1 << 24) / 40 + 1).is_err());
    frames = accepted_frames();
    frames[1].rotation[0] = f32::NAN;
    assert!(FortyFrameKeys::new(&frames, 1).is_err());
}

#[test]
#[ignore = "requires original Qeynos object archives in EQ_DIR"]
fn original_temple_has_certified_native_omissions_and_fixed_collision() {
    let base = std::env::var_os("EQ_DIR").expect("set EQ_DIR");
    let scene = super::super::super::load_object_library(&base, "qeynos2").unwrap();
    let source = &scene.wld_object_sources["templelife"];
    assert_eq!(
        source.animation_period().unwrap(),
        Duration::from_millis(8000)
    );
    assert!(source.render_animation().is_some());
    let skeleton = source.skeleton.as_ref().unwrap();
    let frames = &skeleton
        .tracks
        .iter()
        .find(|track| track.definition.frames.len() == 40)
        .unwrap()
        .definition
        .frames;
    assert_eq!(
        FortyFrameKeys::new(frames, 200).unwrap().omitted,
        [16, 22, 2, 7, 31]
    );
    let initial = source.sample_animation(Duration::ZERO).unwrap();
    assert!(
        initial
            .iter()
            .flat_map(|mesh| &mesh.polygons)
            .all(|polygon| !polygon.collidable)
    );
    assert_ne!(
        initial[0].vertices,
        source
            .sample_animation(Duration::from_millis(2000))
            .unwrap()[0]
            .vertices
    );
    assert_eq!(
        initial[0].vertices,
        source
            .sample_animation(Duration::from_millis(8000))
            .unwrap()[0]
            .vertices
    );
}
