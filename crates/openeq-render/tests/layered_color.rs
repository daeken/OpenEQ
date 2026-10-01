//! Native CB1_2UV color/coordinate operation under OpenEQ's current lighting policy.
use openeq_assets::{
    Scene,
    loader::{PackedTerSecondaryUv, TerColorBlend},
    mesh::{Geometry, Material},
    texture::Texture,
};
use openeq_render::{Camera, GpuScene, Renderer, environment::EnvironmentSettings};
use std::time::Duration;
const SIZE: u32 = 96;
fn material() -> Material {
    Material {
        textures: vec!["diffuse".into()],
        normal_map: Some("normal".into()),
        water: None,
        flags: 0,
        anim_speed: 0,
        alpha_mask: false,
        transparent: false,
        additive: false,
        emissive: true,
        clamp_uv: false,
        waterfall: None,
        uv_encoding: Default::default(),
    }
}
fn texture(name: &str, color: [u8; 4]) -> Texture {
    Texture {
        name: name.into(),
        width: 1,
        height: 1,
        rgba: color.to_vec(),
    }
}
fn fixture(first: Texture, second: Texture, uv0: [f32; 2], uv1: [f32; 2]) -> Scene {
    let vertices = [
        [-10., 2., -10.],
        [10., 2., -10.],
        [10., 2., 10.],
        [-10., 2., 10.],
    ]
    .into_iter()
    .flat_map(|p| [p[0], p[1], p[2], 0., 0., -1., uv0[0], uv0[1]])
    .collect();
    let mut scene = Scene::from_geometry(
        "CB1_2UV fixture".into(),
        vec![material()],
        vec![Geometry {
            vertices,
            indices: vec![0, 1, 2, 0, 2, 3],
            material: 0,
            collidable: false,
        }],
        vec![first, second],
    );
    scene.secondary_ter_uv.insert(
        0,
        PackedTerSecondaryUv {
            color_blend: Some(TerColorBlend {
                diffuse: "diffuse".into(),
                normal: "normal".into(),
                second: "second".into(),
            }),
            tex_coords: vec![uv1; 4],
            source_indices: vec![0, 1, 2, 3],
        },
    );
    scene
}
fn renderer() -> Renderer {
    let mut r = Renderer::new_headless(SIZE, SIZE).unwrap();
    r.set_environment(
        EnvironmentSettings {
            sky_enabled: false,
            ..Default::default()
        },
        None,
    );
    r
}
fn capture(r: &mut Renderer, s: &Scene) -> Vec<u8> {
    let gpu = GpuScene::build(r.device(), r.queue(), s).unwrap();
    assert!(
        gpu.draws
            .iter()
            .all(|d| !d.transparent && !d.additive && !d.waterfall)
    );
    r.set_scene(&gpu);
    r.render_at(
        &gpu,
        &Camera {
            position: [0., 0., 0.],
            yaw: 0.,
            pitch: 0.,
            fov_y: 90_f32.to_radians(),
        },
        Duration::ZERO,
    );
    r.read_rgba().unwrap().2
}
fn center(p: &[u8]) -> [u8; 4] {
    let at = ((SIZE / 2 * SIZE + SIZE / 2) * 4) as usize;
    p[at..at + 4].try_into().unwrap()
}
fn linear(v: u8) -> f32 {
    let v = f32::from(v) / 255.;
    if v <= 0.04045 {
        v / 12.92
    } else {
        ((v + 0.055) / 1.055).powf(2.4)
    }
}
fn srgb(v: f32) -> u8 {
    let v = if v <= 0.0031308 {
        12.92 * v
    } else {
        1.055 * v.powf(1. / 2.4) - 0.055
    };
    (v.clamp(0., 1.) * 255.).round() as u8
}
fn near(actual: [u8; 4], expected: [u8; 4]) {
    for (a, b) in actual.into_iter().zip(expected) {
        assert!(
            a.abs_diff(b) <= 2,
            "actual={actual:?} expected={expected:?}"
        );
    }
}
#[test]
fn second_color_multiplies_with_factor_two_and_alpha_does_not_cut_opaque_depth() {
    let mut r = renderer();
    let d = [180, 120, 64, 0];
    let s = [100, 200, 250, 0];
    let mut source = fixture(
        texture("diffuse", d),
        texture("second", s),
        [0.25; 2],
        [0.75; 2],
    );
    let expected = [
        srgb(2. * linear(d[0]) * linear(s[0])),
        srgb(2. * linear(d[1]) * linear(s[1])),
        srgb(2. * linear(d[2]) * linear(s[2])),
        255,
    ];
    let before = capture(&mut r, &source);
    near(center(&before), expected);
    let mut behind = source.meshes[0].clone();
    for v in behind.vertices.chunks_exact_mut(8) {
        v[1] = 4.;
    }
    source.meshes.push(behind);
    assert_eq!(
        capture(&mut r, &source),
        before,
        "opaque foreground retains depth despite zero texture alpha"
    );
}
#[test]
fn native_factor_is_applied_after_gbuffer_storage_before_lighting_and_fog() {
    let mut r = renderer();
    let mut source = fixture(
        texture("diffuse", [255; 4]),
        texture("second", [255; 4]),
        [0.25; 2],
        [0.75; 2],
    );
    source.materials[0].emissive = false;
    // Supplied normal points straight down: only the renderer's documented ambient remains.
    near(
        center(&capture(&mut r, &source)),
        [srgb(0.44), srgb(0.48), srgb(0.60), 255],
    );
    r.set_environment(
        EnvironmentSettings {
            sky_enabled: false,
            fog_enabled: true,
            fog_start: 0.,
            fog_end: 1.,
            fog_color: [0.2, 0.4, 0.6],
            ..Default::default()
        },
        None,
    );
    near(center(&capture(&mut r, &source)), [51, 102, 153, 255]);
}
#[test]
fn secondary_coordinates_are_independent_quantized_wrapped_and_rebinding_disables_stale_metadata() {
    let mut r = renderer();
    let pixels = (0..256 * 256)
        .flat_map(|i| {
            if i % 256 < 128 {
                [255, 0, 0, 0]
            } else {
                [0, 255, 0, 0]
            }
        })
        .collect();
    let second = Texture {
        name: "second".into(),
        width: 256,
        height: 256,
        rgba: pixels,
    };
    let mut source = fixture(
        texture("diffuse", [180, 180, 180, 255]),
        second,
        [0.25; 2],
        [0.25; 2],
    );
    let red = capture(&mut r, &source);
    assert!(center(&red)[0] > 200 && center(&red)[1] == 0);
    source
        .secondary_ter_uv
        .get_mut(&0)
        .unwrap()
        .tex_coords
        .fill([0.75, 0.25]);
    let green = capture(&mut r, &source);
    assert!(center(&green)[1] > 200 && center(&green)[0] == 0);
    assert_ne!(red, green);
    source
        .secondary_ter_uv
        .get_mut(&0)
        .unwrap()
        .tex_coords
        .fill([128.75, -128.75]);
    assert_eq!(capture(&mut r, &source), green, "signed SHORT2 wrapping");
    source
        .secondary_ter_uv
        .get_mut(&0)
        .unwrap()
        .tex_coords
        .fill([0.2501, 0.2501]);
    assert_eq!(
        capture(&mut r, &source),
        red,
        "sub-1/256 coordinates truncate"
    );
    source.materials[0].normal_map = Some("rebound".into());
    let rebound = capture(&mut r, &source);
    source.secondary_ter_uv.clear();
    assert_eq!(capture(&mut r, &source), rebound);
    near(center(&rebound), [180, 180, 180, 255]);
}
#[test]
fn secondary_minification_uses_its_own_derivatives() {
    let mut r = renderer();
    let second = Texture {
        name: "second".into(),
        width: 256,
        height: 256,
        rgba: (0..256 * 256)
            .flat_map(|i| {
                let v = if (i % 256) / 4 % 2 == 0 { 0 } else { 255 };
                [v, v, v, 255]
            })
            .collect(),
    };
    let mut source = fixture(
        texture("diffuse", [180, 180, 180, 255]),
        second,
        [0.25; 2],
        [0.; 2],
    );
    // Plane covers ten view widths, so 113 UV repeats become 11.3 repeats on screen.
    source.secondary_ter_uv.get_mut(&0).unwrap().tex_coords =
        vec![[0., 0.], [113., 0.], [113., 113.], [0., 113.]];
    let image = capture(&mut r, &source);
    let values: Vec<_> = image.chunks_exact(4).map(|p| p[0]).collect();
    let range = values.iter().max().unwrap() - values.iter().min().unwrap();
    assert!(range < 20, "second layer aliases: range={range}");
    let mean = values.iter().map(|&v| f32::from(v)).sum::<f32>() / values.len() as f32;
    assert!((110.0..135.0).contains(&mean), "mean={mean}");
}

#[test]
#[ignore = "requires original Nest assets and GPU; captures to /tmp/openeq-layered-gpu"]
fn original_nest_second_color_changes_authored_terrain_and_remains_stable() {
    use glam::Vec3;
    use openeq_assets::{loader, pfs::Archive, zone::TerMod};
    let base = loader::default_client_dir().unwrap();
    let mut source = loader::load_zone(&base, "thenest").unwrap();
    let ter = TerMod::parse(
        &Archive::open(base.join("thenest.eqg"))
            .unwrap()
            .read("ter_abyss01.ter")
            .unwrap(),
        true,
    )
    .unwrap();
    let groups: std::collections::BTreeMap<_, _> = ter.mesh_groups().into_iter().collect();
    let mut enabled = 0;
    let mut triangles = 0;
    for (mesh, (ordinal, _)) in groups.iter().enumerate() {
        let material = ter.material_for_polygon(*ordinal).unwrap();
        if material.shader == "Opaque_MaxCB1_2UV.fx" {
            let channel = &source.secondary_ter_uv[&mesh];
            let blend = channel.color_blend.as_ref().unwrap();
            assert_eq!(
                blend.second,
                material.properties["e_TextureSecond0"].as_text().unwrap()
            );
            assert_eq!(
                blend.diffuse,
                material.properties["e_TextureDiffuse0"].as_text().unwrap()
            );
            assert!(source.texture(&blend.second).is_some());
            enabled += 1;
            triangles += source.meshes[mesh].indices.len() / 3;
        } else {
            assert!(
                source
                    .secondary_ter_uv
                    .get(&mesh)
                    .is_none_or(|s| s.color_blend.is_none())
            );
        }
    }
    assert_eq!(triangles, 188101);
    assert!(enabled > 19);
    let mut renderer = Renderer::new_headless(640, 360).unwrap();
    renderer.set_environment(EnvironmentSettings::for_zone("thenest"), None);
    // Original restored horizontal DWPC face; the older metalwall camera
    // sees CBSG, which deliberately remains outside this color-path gate.
    let (a, b, c, ordinal, _) = ter.polygons[299832];
    assert_eq!(
        ter.material_for_polygon(ordinal).unwrap().shader,
        "Opaque_MaxCB1_2UV.fx"
    );
    let target = (Vec3::from(ter.positions[a as usize])
        + Vec3::from(ter.positions[b as usize])
        + Vec3::from(ter.positions[c as usize]))
        / 3.;
    let eye = target + Vec3::new(0., -10., 15.);
    let direction = (target - eye).normalize();
    let camera = Camera {
        position: eye.to_array(),
        yaw: direction.x.atan2(direction.y),
        pitch: direction.z.asin(),
        fov_y: 70_f32.to_radians(),
    };
    let render = |r: &mut Renderer, s: &Scene| {
        let gpu = GpuScene::build(r.device(), r.queue(), s).unwrap();
        r.set_scene(&gpu);
        r.render_at(&gpu, &camera, Duration::from_secs(3));
        r.read_rgba().unwrap().2
    };
    let new = render(&mut renderer, &source);
    assert_eq!(new, render(&mut renderer, &source));
    for metadata in source.secondary_ter_uv.values_mut() {
        metadata.color_blend = None;
    }
    let old = render(&mut renderer, &source);
    let changed = new
        .chunks_exact(4)
        .zip(old.chunks_exact(4))
        .filter(|(a, b)| a != b)
        .count();
    assert!(changed > 5000, "changed={changed}");
    let path = std::path::Path::new("/tmp/openeq-layered-gpu");
    std::fs::create_dir_all(path).unwrap();
    for (name, pixels) in [
        ("thenest-layered.png", new),
        ("thenest-primary-only.png", old),
    ] {
        image::save_buffer(path.join(name), &pixels, 640, 360, image::ColorType::Rgba8).unwrap();
    }
    eprintln!(
        "original Nest: {enabled} CB draw groups / {triangles} triangles; {changed} changed pixels"
    );
}
