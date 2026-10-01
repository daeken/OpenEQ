//! Native region additive color/depth contract, exercised through real GPU frames.
use glam::Vec3;
use openeq_assets::{
    Scene,
    loader::{self, Light},
    mesh::{self, Geometry, Material},
    pfs::Archive,
    texture::Texture,
    zone::TerMod,
};
use openeq_render::{Camera, GpuActor, GpuScene, Renderer, environment::EnvironmentSettings};
use std::time::Duration;

fn material(name: &str, additive: bool, emissive: bool) -> Material {
    Material {
        textures: vec![name.into()],
        normal_map: None,
        water: None,
        flags: 0,
        anim_speed: 0,
        alpha_mask: false,
        transparent: false,
        additive,
        emissive,
        clamp_uv: false,
        uv_encoding: Default::default(),
    }
}
fn plane(color: [u8; 4], y: f32, additive: bool, emissive: bool) -> Scene {
    Scene::from_geometry(
        "additive fixture".into(),
        vec![material("solid", additive, emissive)],
        vec![Geometry {
            vertices: vec![
                -10., y, -10., 0., -1., 0., 0., 0., 10., y, -10., 0., -1., 0., 1., 0., 10., y, 10.,
                0., -1., 0., 1., 1., -10., y, 10., 0., -1., 0., 0., 1.,
            ],
            indices: vec![0, 1, 2, 0, 2, 3],
            material: 0,
            collidable: false,
        }],
        vec![Texture {
            name: "solid".into(),
            width: 1,
            height: 1,
            rgba: color.to_vec(),
        }],
    )
}
fn upload(renderer: &Renderer, scene: &Scene) -> GpuScene {
    GpuScene::build(renderer.device(), renderer.queue(), scene).unwrap()
}
fn actor(renderer: &Renderer, color: [u8; 4], y: f32) -> GpuActor {
    renderer.prepare_actor(upload(renderer, &plane(color, y, true, false)))
}
fn pixels(
    renderer: &mut Renderer,
    scene: &GpuScene,
    camera: &Camera,
    actors: &[&GpuActor],
) -> Vec<u8> {
    renderer.set_scene(scene);
    if actors.is_empty() {
        renderer.render_at(scene, camera, Duration::ZERO);
    } else {
        renderer.render_with_actors(scene, camera, actors);
    }
    renderer.read_rgba().unwrap().2
}
fn center(renderer: &mut Renderer, scene: &GpuScene, actors: &[&GpuActor]) -> [u8; 4] {
    renderer.set_scene(scene);
    renderer.render_with_actors(
        scene,
        &Camera {
            pitch: 0.,
            ..Default::default()
        },
        actors,
    );
    let (width, height, rgba) = renderer.read_rgba().unwrap();
    let offset = ((height / 2 * width + width / 2) * 4) as usize;
    rgba[offset..offset + 4].try_into().unwrap()
}
fn linear(value: u8) -> f32 {
    let value = f32::from(value) / 255.;
    if value <= 0.04045 {
        value / 12.92
    } else {
        ((value + 0.055) / 1.055).powf(2.4)
    }
}
fn assert_sum(actual: [u8; 4], a: [u8; 4], b: [u8; 4], background: [u8; 4]) {
    for channel in 0..3 {
        let expected = linear(a[channel]) + linear(b[channel]) - linear(background[channel]);
        assert!(
            (linear(actual[channel]) - expected).abs() < 0.012,
            "additive RGB mismatch: {actual:?}, layers {a:?} + {b:?}, background {background:?}"
        );
    }
    assert_eq!(actual[3], background[3], "target compositor alpha changed");
}

#[test]
fn alpha_is_a_cutoff_with_full_rgb_background_preservation_and_read_only_depth() {
    let Ok(mut renderer) = Renderer::new_headless(64, 64) else {
        return;
    };
    let world = upload(&renderer, &plane([0, 0, 64, 255], 4., false, true));
    let background = center(&mut renderer, &world, &[]);
    let full = actor(&renderer, [128, 0, 0, 255], 2.);
    let full_pixel = center(&mut renderer, &world, &[&full]);
    assert!(
        full_pixel[0] > 40 && full_pixel[0] < 128,
        "glass must be lit, not emissive: {full_pixel:?}"
    );
    assert_eq!(
        full_pixel[2], background[2],
        "opaque G-buffer replaced the background"
    );
    for alpha in [0, 1, 15, 16, 17, 102, 254, 255] {
        let glass = actor(&renderer, [128, 0, 0, alpha], 2.);
        let pixel = center(&mut renderer, &world, &[&glass]);
        assert_eq!(
            pixel,
            if alpha < 16 { background } else { full_pixel },
            "sampled alpha {alpha}"
        );
    }
    let near = actor(&renderer, [128, 0, 0, 102], 2.);
    let far = actor(&renderer, [0, 128, 0, 102], 3.);
    let a = center(&mut renderer, &world, &[&near]);
    let b = center(&mut renderer, &world, &[&far]);
    let ab = center(&mut renderer, &world, &[&near, &far]);
    let ba = center(&mut renderer, &world, &[&far, &near]);
    assert_sum(ab, a, b, background);
    assert_eq!(
        ab, ba,
        "a near glass draw wrote depth and hid a later far draw"
    );
    let wall = upload(&renderer, &plane([0, 64, 0, 255], 1., false, true));
    assert_eq!(
        center(&mut renderer, &wall, &[&near, &far]),
        [0, 64, 0, 255],
        "glass leaked through opaque depth"
    );
    let equal = upload(&renderer, &plane([0, 0, 64, 255], 2., false, true));
    assert_eq!(
        center(&mut renderer, &equal, &[&near]),
        a,
        "equal opaque depth must pass"
    );
}

#[test]
fn cutoff_uses_filtered_alpha_and_not_an_unfiltered_texel_or_opacity_weight() {
    let Ok(mut renderer) = Renderer::new_headless(64, 64) else {
        return;
    };
    let world = upload(&renderer, &plane([0, 0, 0, 255], 4., false, true));
    let full = actor(&renderer, [128, 0, 0, 102], 2.);
    let expected = center(&mut renderer, &world, &[&full]);
    let texture = Texture {
        name: "solid".into(),
        width: 256,
        height: 256,
        rgba: (0..256)
            .flat_map(|_| (0..256).flat_map(|x| [128, 0, 0, if x < 128 { 0 } else { 32 }]))
            .collect(),
    };
    for (u, visible) in [
        (0.5 - 1. / 1024., false),
        (0.5, true),
        (0.5 + 1. / 1024., true),
    ] {
        let mut source = plane([0; 4], 2., true, false);
        for v in source.meshes[0].vertices.chunks_exact_mut(8) {
            v[6] = u;
            v[7] = 0.5;
        }
        let sampled = Scene::from_geometry(
            "filtered alpha".into(),
            source.materials,
            source.meshes,
            vec![texture.clone()],
        );
        let glass = renderer.prepare_actor(upload(&renderer, &sampled));
        let actual = center(&mut renderer, &world, &[&glass]);
        assert_eq!(
            actual,
            if visible { expected } else { [0, 0, 0, 255] },
            "u={u} (filtered alpha 8/16/24)"
        );
    }
}

#[test]
fn shared_texture_keeps_opaque_and_additive_draws_distinct_and_additive_uses_zone_lights_without_fog()
 {
    let Ok(mut renderer) = Renderer::new_headless(64, 64) else {
        return;
    };
    let mut shared = plane([128, 64, 32, 102], 4., false, false);
    let mut glass = shared.meshes[0].clone();
    for v in glass.vertices.chunks_exact_mut(8) {
        v[1] = 2.;
    }
    glass.material = 1;
    let mut additive = shared.materials[0].clone();
    additive.additive = true;
    shared.materials.push(additive);
    shared.meshes.push(glass);
    let gpu = upload(&renderer, &shared);
    assert_eq!(
        gpu.draws
            .iter()
            .map(|d| (d.additive, d.transparent))
            .collect::<Vec<_>>(),
        [(false, false), (true, false)]
    );
    let combined = center(&mut renderer, &gpu, &[]);
    let black = upload(&renderer, &plane([0, 0, 0, 255], 6., false, true));
    let opaque = renderer.prepare_actor(upload(
        &renderer,
        &plane([128, 64, 32, 255], 4., false, false),
    ));
    let glass = actor(&renderer, [128, 64, 32, 102], 2.);
    let a = center(&mut renderer, &black, &[&opaque]);
    let b = center(&mut renderer, &black, &[&glass]);
    assert_sum(combined, a, b, [0, 0, 0, 255]);
    for channel in 0..3 {
        assert!(
            a[channel].abs_diff(b[channel]) <= 2,
            "normal lighting diverged: {a:?} {b:?}"
        );
    }
    let mut source = plane([0, 0, 0, 255], 6., false, true);
    source.lights.push(Light {
        position: [0., 0., 0.],
        color: [0.8, 0.1, 0.0],
        radius: 24.,
        attenuation: 1.,
    });
    let lit = upload(&renderer, &source);
    let lit_pixel = center(&mut renderer, &lit, &[&glass]);
    assert!(
        lit_pixel[0] > b[0] + 15,
        "authored zone light had no effect: {b:?} {lit_pixel:?}"
    );
    renderer.set_environment(
        EnvironmentSettings {
            fog_enabled: true,
            fog_start: 0.,
            fog_end: 1.,
            fog_color: [0.; 3],
            ..Default::default()
        },
        None,
    );
    assert_eq!(
        center(&mut renderer, &lit, &[&opaque]),
        [0, 0, 0, 255],
        "fixture failed to enable opaque fog"
    );
    assert_eq!(
        center(&mut renderer, &lit, &[&glass]),
        lit_pixel,
        "glass contribution was fogged"
    );
}

fn look_at(eye: Vec3, target: Vec3) -> Camera {
    let direction = (target - eye).normalize();
    Camera {
        position: eye.to_array(),
        yaw: direction.x.atan2(direction.y),
        pitch: direction.z.asin(),
        ..Default::default()
    }
}
fn horizontal(center: [f32; 3], half: f32, material: usize) -> Geometry {
    let [x, y, z] = center;
    let mut vertices = Vec::new();
    for ([dx, dy], [u, v]) in [[-half, -half], [half, -half], [half, half], [-half, half]]
        .into_iter()
        .zip([[0., 0.], [1., 0.], [1., 1.], [0., 1.]])
    {
        vertices.extend([x + dx, y + dy, z, 0., 0., 1., u, v]);
    }
    Geometry {
        vertices,
        indices: vec![0, 1, 2, 0, 2, 3],
        material,
        collidable: false,
    }
}

#[test]
fn additive_surfaces_do_not_cast_opaque_shadows_or_cover_ui() {
    let Ok(mut renderer) = Renderer::new_headless(96, 96) else {
        return;
    };
    let floor = Scene::from_geometry(
        "receiver".into(),
        vec![material("floor", false, false)],
        vec![horizontal([0.; 3], 40., 0)],
        vec![Texture {
            name: "floor".into(),
            width: 1,
            height: 1,
            rgba: vec![120, 120, 120, 255],
        }],
    );
    let receiver = upload(&renderer, &floor);
    let camera = look_at(Vec3::new(0., -15., 12.), Vec3::ZERO);
    let base = pixels(&mut renderer, &receiver, &camera, &[]);
    let mut black = plane([0, 0, 0, 255], 2., true, false);
    // Along the sun ray above the receiver; an opaque version visibly shadows it.
    black.meshes[0] = horizontal([-2.745, -2.135, 5.], 4., 0);
    let glass = renderer.prepare_actor(upload(&renderer, &black));
    let actual = pixels(&mut renderer, &receiver, &camera, &[&glass]);
    assert_eq!(
        actual, base,
        "black additive glass changed the receiver (opaque/shadow leakage)"
    );
    black.materials[0].additive = false;
    let solid = renderer.prepare_actor(upload(&renderer, &black));
    let opaque = pixels(&mut renderer, &receiver, &camera, &[&solid]);
    let changed = base
        .chunks_exact(4)
        .zip(opaque.chunks_exact(4))
        .filter(|(a, b)| a != b)
        .count();
    assert!(
        changed > 50,
        "fixture did not exercise an opaque blocker: {changed}"
    );
    // A colored additive draw must remain underneath the final UI pass.
    let backdrop = upload(&renderer, &plane([0, 0, 0, 255], 4., false, true));
    let red = actor(&renderer, [255, 0, 0, 102], 2.);
    renderer.resize(48, 48);
    let rect = openeq_ui::Rect::new(0., 0., 48., 48.);
    renderer.set_ui(&openeq_ui::UiFrame {
        commands: vec![openeq_ui::DrawCommand::Fill {
            rect,
            clip: rect,
            color: [0, 255, 0, 255],
        }],
        ..Default::default()
    });
    assert_eq!(center(&mut renderer, &backdrop, &[&red]), [0, 255, 0, 255]);
}

#[test]
#[ignore = "requires original Thundercrest assets and GPU; offline, no native-client golden"]
fn original_thundercrest_glass_binding_and_full_strength_alpha_witness() {
    let base = loader::default_client_dir().expect("original assets");
    let source = loader::load_zone(&base, "thundercrest").unwrap();
    let archive = Archive::open(base.join("thundercrest.eqg")).unwrap();
    let terrain = TerMod::parse(&archive.read("ter_stormtower01.ter").unwrap(), true).unwrap();
    assert_eq!(terrain.polygons[19671], (57396, 54999, 55001, 227, 0));
    let canonical = terrain.material_for_polygon(227).unwrap();
    assert!(std::ptr::eq(canonical, &terrain.materials[32]));
    assert_eq!(canonical.shader, "AddAlpha_MaxCB1.fx");
    assert_eq!(canonical.properties.len(), 2);
    let meshes: Vec<_> = source
        .meshes
        .iter()
        .filter(|m| source.materials[m.material].additive)
        .collect();
    assert_eq!(meshes.len(), 1);
    let mesh = meshes[0];
    assert_eq!(mesh.indices.len(), 72 * 3);
    let (vertices, indices) = mesh::pack(
        &terrain.positions,
        &terrain.normals,
        &terrain.tex_coords,
        &terrain.mesh_groups()[&227],
    );
    assert_eq!(mesh.vertices, vertices);
    assert_eq!(mesh.indices, indices);
    let mat = source.materials[mesh.material].clone();
    assert!(!mat.transparent && !mat.emissive && mat.water.is_none());
    assert_eq!(mat.textures, ["rc_ST_Gcglass_c.dds"]);
    let diffuse = source.texture(&mat.textures[0]).unwrap();
    assert_eq!((diffuse.width, diffuse.height), (128, 128));
    assert!(diffuse.rgba.chunks_exact(4).all(|p| p[3] == 102));
    let normal = source.texture(mat.normal_map.as_ref().unwrap()).unwrap();
    assert!(
        normal
            .rgba
            .chunks_exact(4)
            .all(|p| p == [128, 128, 255, 255])
    );
    let mut geometry = mesh.clone();
    geometry.material = 0;
    let mut scene = Scene::from_geometry(
        "Thundercrest glass witness".into(),
        vec![mat],
        vec![geometry],
        vec![diffuse.clone()],
    );
    scene.lights = source.lights.clone();
    let points: [Vec3; 3] = std::array::from_fn(|i| {
        let start = mesh.indices[i] as usize * 8;
        Vec3::from_slice(&mesh.vertices[start..start + 3])
    });
    let target = (points[0] + points[1] + points[2]) / 3.;
    let face = (points[1] - points[0])
        .cross(points[2] - points[0])
        .normalize();
    let camera = look_at(target + face * 40., target);
    let mut renderer = Renderer::new_headless(480, 320).unwrap();
    renderer.set_environment(
        EnvironmentSettings {
            sky_enabled: false,
            ..Default::default()
        },
        None,
    );
    let mut gpu = upload(&renderer, &scene);
    assert!(gpu.draws.iter().all(|d| d.additive && !d.transparent));
    let glass = pixels(&mut renderer, &gpu, &camera, &[]);
    gpu.draws.iter_mut().for_each(|d| d.instance_count = 0);
    let hidden = pixels(&mut renderer, &gpu, &camera, &[]);
    let changed = glass
        .chunks_exact(4)
        .zip(hidden.chunks_exact(4))
        .filter(|(a, b)| a != b)
        .count();
    assert!(changed > 500, "glass witness is not visible: {changed}");
    let mut alpha255 = diffuse;
    for pixel in alpha255.rgba.chunks_exact_mut(4) {
        pixel[3] = 255;
    }
    let mut full_scene =
        Scene::from_geometry(scene.name, scene.materials, scene.meshes, vec![alpha255]);
    full_scene.lights = scene.lights;
    let full = upload(&renderer, &full_scene);
    assert_eq!(
        glass,
        pixels(&mut renderer, &full, &camera, &[]),
        "original alpha102 was used to weight RGB"
    );
    let output = std::env::var_os("OPEN_EQ_ADDITIVE_CAPTURE")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| "/tmp/openeq-thundercrest-additive-glass.png".into());
    image::save_buffer(&output, &glass, 480, 320, image::ColorType::Rgba8).unwrap();
    eprintln!(
        "Thundercrest: 72 triangles, all 16,384 alpha-102 texels, {changed} visible pixels; alpha255 identical; {}",
        output.display()
    );
}
