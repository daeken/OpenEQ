//! Startup camera/model extents must describe submitted world geometry.
use glam::{Quat, Vec3};
use openeq_assets::{
    Instance, Scene, SceneObject,
    mesh::{CollisionGeometry, Geometry, Material},
    texture::Texture,
};
use openeq_render::{GpuScene, Renderer};

fn quad(offset: f32) -> Geometry {
    Geometry {
        vertices: [
            [-1. + offset, 0., -1.],
            [1. + offset, 0., -1.],
            [1. + offset, 0., 1.],
            [-1. + offset, 0., 1.],
            [1e20; 3], // Not referenced by a submitted triangle.
        ]
        .into_iter()
        .flat_map(|[x, y, z]| [x, y, z, 0., -1., 0., 0., 0.])
        .collect(),
        indices: vec![0, 1, 2, 0, 2, 3],
        material: 0,
        collidable: false,
    }
}

#[test]
#[ignore = "requires GPU; no original assets or live connection"]
fn placed_bounds_ignore_unplaced_duplicates_unused_vertices_and_hidden_geometry() {
    let mut scene = Scene::from_geometry(
        "world bounds".into(),
        vec![Material {
            textures: vec!["white".into()],
            normal_map: None,
            water: None,
            flags: 1,
            anim_speed: 0,
            alpha_mask: false,
            transparent: false,
            additive: false,
            emissive: true,
            clamp_uv: false,
        }],
        vec![quad(0.), quad(1e6), quad(-1e6)],
        vec![Texture {
            name: "white".into(),
            width: 1,
            height: 1,
            rgba: vec![255; 4],
        }],
    );
    scene.objects = vec![
        SceneObject {
            name: "placed".into(),
            meshes: vec![0],
            collision_meshes: vec![],
        },
        SceneObject {
            name: "unused".into(),
            meshes: vec![1],
            collision_meshes: vec![],
        },
        // Existing first-name lookup must remain consistent between bounds
        // and draw calls even if later definitions share that name.
        SceneObject {
            name: "placed".into(),
            meshes: vec![2],
            collision_meshes: vec![],
        },
    ];
    scene.instances = vec![
        Instance {
            object: "placed".into(),
            position: [100., 200., 300.],
            rotation: Quat::from_rotation_z(std::f32::consts::FRAC_PI_2).to_array(),
            scale: [2., 3., 4.],
        },
        Instance {
            object: "placed".into(),
            position: [-100., -200., -300.],
            rotation: Quat::IDENTITY.to_array(),
            scale: [1.; 3],
        },
        Instance {
            object: "unresolved".into(),
            position: [1e10; 3],
            rotation: Quat::IDENTITY.to_array(),
            scale: [1.; 3],
        },
    ];
    scene.collision_meshes.push(CollisionGeometry {
        positions: vec![[1e10, 0., 0.], [1e10, 0., 1.], [1e10, 1., 0.]],
        indices: vec![0, 1, 2],
    });
    let renderer = Renderer::new_headless(64, 64).unwrap();
    let gpu = GpuScene::build(renderer.device(), renderer.queue(), &scene).unwrap();
    assert_eq!(gpu.draws.len(), 1);
    assert_eq!(gpu.draws[0].instance_count, 2);
    assert!(
        gpu.bounds_min
            .abs_diff_eq(Vec3::new(-101., -200., -301.), 0.001)
    );
    assert!(
        gpu.bounds_max
            .abs_diff_eq(Vec3::new(100., 202., 304.), 0.001)
    );

    scene.instances.clear();
    let empty = GpuScene::build(renderer.device(), renderer.queue(), &scene).unwrap();
    assert!(empty.draws.is_empty());
    assert_eq!(empty.bounds_min, Vec3::ZERO);
    assert_eq!(empty.bounds_max, Vec3::ZERO);

    // A direct mesh uses identity exactly once, regardless of object instances.
    scene.meshes.push(quad(400.));
    let direct = GpuScene::build(renderer.device(), renderer.queue(), &scene).unwrap();
    assert_eq!(direct.draws.len(), 1);
    assert_eq!(direct.draws[0].instance_count, 1);
    assert_eq!(direct.bounds_min, Vec3::new(399., 0., -1.));
    assert_eq!(direct.bounds_max, Vec3::new(401., 0., 1.));
}
