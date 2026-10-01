//! Bounded offline appearance/performance audit of restored original terrain.
//! Captures are evidence to inspect, not native-client image goldens.
use std::{
    collections::{BTreeMap, HashSet},
    time::{Duration, Instant},
};

use glam::Vec3;
use openeq_assets::{Scene, loader, mesh, pfs::Archive, zone::TerMod};
use openeq_render::{Camera, GpuScene, Renderer, environment::EnvironmentSettings};

const WIDTH: u32 = 960;
const HEIGHT: u32 = 540;
const WARMUP: usize = 60;
const SAMPLES: usize = 100;
const TIME: Duration = Duration::from_secs(3);

fn capture(renderer: &mut Renderer, gpu: &GpuScene, camera: &Camera) -> Vec<u8> {
    renderer.set_scene(gpu);
    renderer.render_at(gpu, camera, TIME);
    renderer.read_rgba().unwrap().2
}

fn profile(renderer: &mut Renderer, gpu: &GpuScene, camera: &Camera) {
    renderer.enable_profiling(false);
    let supported = renderer.enable_profiling(true);
    renderer.set_scene(gpu);
    let mut values = vec![Vec::new(); 12];
    for frame in 0..WARMUP + SAMPLES {
        let start = Instant::now();
        renderer.render_at(gpu, camera, TIME);
        let submitted = Instant::now();
        renderer
            .device()
            .poll(wgpu::PollType::wait_indefinitely())
            .unwrap();
        let complete = Instant::now();
        let mut stats = renderer.profiling_stats();
        // Drain timestamp resolve/map separately from the render completion
        // wait above. Production uses asynchronous polling; only this harness
        // serializes frames to obtain bounded, unambiguous samples.
        for _ in 0..3 {
            if stats.in_flight == 0 {
                break;
            }
            renderer
                .device()
                .poll(wgpu::PollType::wait_indefinitely())
                .unwrap();
            stats = renderer.profiling_stats();
        }
        assert_eq!(stats.in_flight, 0);
        if frame < WARMUP {
            continue;
        }
        values[0].push((submitted - start).as_secs_f64() * 1000.);
        values[1].push((complete - submitted).as_secs_f64() * 1000.);
        if supported {
            let t = stats.latest.unwrap();
            assert_eq!(t.frame_id, frame as u64 + 1);
            for (i, v) in [
                t.shadow_ms,
                t.gbuffer_ms,
                t.lighting_ms,
                t.transparency_ms,
                t.waterfall_ms,
                t.lava_ms,
                t.additive_ms,
                t.total_ms,
                t.frame_span_ms,
                t.raw_pass_sum_ms,
            ]
            .into_iter()
            .enumerate()
            {
                values[i + 2].push(v);
            }
        }
    }
    for (name, mut samples) in [
        "cpu_submit",
        "completion_wait",
        "gpu_shadow",
        "gpu_gbuffer",
        "gpu_lighting",
        "gpu_transparency",
        "gpu_waterfall",
        "gpu_lava",
        "gpu_additive",
        "gpu_attributed_total",
        "gpu_frame_span",
        "gpu_raw_pass_sum",
    ]
    .into_iter()
    .zip(values)
    {
        if samples.is_empty() {
            continue;
        }
        samples.sort_by(f64::total_cmp);
        eprintln!(
            "TIMING {name} median_ms={:.4} p95_ms={:.4} samples={}",
            samples[samples.len() / 2 - 1],
            samples[(samples.len() * 95).div_ceil(100) - 1],
            samples.len()
        );
    }
    let stats = renderer.profiling_stats();
    assert_eq!(stats.failed, 0);
    assert_eq!(stats.dropped, 0);
    eprintln!(
        "TIMESTAMP supported={supported} submitted={} completed={}",
        stats.submitted, stats.completed
    );
    renderer.enable_profiling(false);
}

#[test]
#[ignore = "requires original Bazaar, The Nest, Thundercrest and GPU; captures to /tmp/openeq-restored-eqg-gpu"]
fn original_restored_terrain_fixed_cameras_and_serialized_gpu_timings() {
    let base = loader::default_client_dir().expect("original assets");
    let output = std::env::var_os("OPEN_EQ_RESTORED_AUDIT_OUTPUT")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| "/tmp/openeq-restored-eqg-gpu".into());
    std::fs::create_dir_all(&output).unwrap();
    let mut renderer = Renderer::new_headless(WIDTH, HEIGHT).unwrap();
    for (zone, member, ordinal, witness, eye, restored_count) in [
        (
            "bazaar",
            "ter_bazaar.ter",
            65,
            26136,
            [760., -775., -26.],
            142594,
        ),
        (
            "thenest",
            "ter_abyss01.ter",
            1126,
            152182,
            [545., 4340., -230.],
            250988,
        ),
        (
            "thundercrest",
            "ter_stormtower01.ter",
            1174,
            200458,
            [-38., 35., 285.],
            270445,
        ),
    ] {
        eprintln!(
            "AUDIT {zone} dimensions={WIDTH}x{HEIGHT} warmup={WARMUP} measured={SAMPLES} fixed_seconds=3"
        );
        let begin = Instant::now();
        let mut source = loader::load_zone(&base, zone).unwrap();
        eprintln!(
            "LOAD cpu_ms={:.3} definitions={} instances={} lights={} triangles={}",
            begin.elapsed().as_secs_f64() * 1000.,
            source.meshes.len(),
            source.instances.len(),
            source.lights.len(),
            source.triangle_count()
        );
        let archive = Archive::open(base.join(format!("{zone}.eqg"))).unwrap();
        let terrain = TerMod::parse(&archive.read(member).unwrap(), true).unwrap();
        let old_ids: HashSet<_> = terrain.materials.iter().map(|m| m.stored_id).collect();
        assert!(!old_ids.contains(&ordinal));
        let canonical = terrain.material_for_polygon(ordinal).unwrap();
        let (a, b, c, reference, flags) = terrain.polygons[witness];
        assert_eq!(reference, ordinal);
        let points = [a, b, c].map(|i| Vec3::from(terrain.positions[i as usize]));
        let target = (points[0] + points[1] + points[2]) / 3.;
        let direction = (target - Vec3::from(eye)).normalize();
        let camera = Camera {
            position: eye,
            yaw: direction.x.atan2(direction.y),
            pitch: direction.z.asin(),
            fov_y: 70f32.to_radians(),
        };
        eprintln!(
            "CAMERA eye={eye:?} target={target:?} yaw_deg={} pitch_deg={} polygon={witness} indices={:?} flags={flags:#x} ordinal={ordinal} canonical={} shader={}",
            camera.yaw.to_degrees(),
            camera.pitch.to_degrees(),
            [a, b, c],
            canonical.name,
            canonical.shader
        );

        let owned: HashSet<_> = source
            .objects
            .iter()
            .flat_map(|o| o.meshes.iter().copied())
            .collect();
        let terrain_meshes: Vec<_> = (0..source.meshes.len())
            .filter(|i| !owned.contains(i))
            .collect();
        let groups: BTreeMap<_, _> = terrain
            .mesh_groups()
            .into_iter()
            .filter(|(i, _)| terrain.material_for_polygon(*i).is_some())
            .collect();
        assert_eq!(
            terrain_meshes.len(),
            groups.len(),
            "one direct TER with source-order batches"
        );
        let mut restored = Vec::new();
        let mut restored_shaders = BTreeMap::new();
        let mut selected = None;
        let mut former_packing = Vec::new();
        for ((id, indices), &mesh_id) in groups.iter().zip(&terrain_meshes) {
            let mesh = &source.meshes[mesh_id];
            let (packed, packed_indices) = mesh::pack(
                &terrain.positions,
                &terrain.normals,
                &terrain.tex_coords,
                indices,
            );
            // Lighting and secondary UVs add vertex identity; corner attributes and
            // order must remain bit-identical to the former geometry bake.
            assert_eq!(mesh.indices.len(), packed_indices.len());
            for (&actual, &previous) in mesh.indices.iter().zip(&packed_indices) {
                let a = actual as usize * 8;
                let b = previous as usize * 8;
                assert_eq!(
                    mesh.vertices[a..a + 8]
                        .iter()
                        .map(|v| v.to_bits())
                        .collect::<Vec<_>>(),
                    packed[b..b + 8]
                        .iter()
                        .map(|v| v.to_bits())
                        .collect::<Vec<_>>()
                );
            }
            former_packing.push((mesh_id, packed, packed_indices));
            if !old_ids.contains(id) {
                restored.push(mesh_id);
                *restored_shaders
                    .entry(terrain.material_for_polygon(*id).unwrap().shader.clone())
                    .or_insert(0usize) += indices.len() / 3;
            }
            if *id == ordinal {
                selected = Some(mesh_id);
            }
        }
        assert_eq!(
            restored
                .iter()
                .map(|&i| source.meshes[i].indices.len() / 3)
                .sum::<usize>(),
            restored_count
        );
        eprintln!("RESTORED shader_triangles={restored_shaders:?}");
        let selected = selected.unwrap();
        let mut geometry = source.meshes[selected].clone();
        let material = source.materials[geometry.material].clone();
        assert_eq!(
            material.textures[0],
            canonical.properties["e_TextureDiffuse0"].as_text().unwrap()
        );
        assert!(!material.alpha_mask && !material.transparent && material.water.is_none());
        let diffuse = source
            .texture(&material.textures[0])
            .expect("selected original diffuse");
        let alpha_zero = diffuse.rgba.chunks_exact(4).filter(|p| p[3] == 0).count();
        let alpha_fractional = diffuse
            .rgba
            .chunks_exact(4)
            .filter(|p| p[3] > 0 && p[3] < 255)
            .count();
        eprintln!(
            "SELECTED triangles={} texture={} dimensions={}x{} alpha_zero={alpha_zero} alpha_fractional={alpha_fractional} mask={} blend={}",
            geometry.indices.len() / 3,
            diffuse.name,
            diffuse.width,
            diffuse.height,
            material.alpha_mask,
            material.transparent
        );

        let mut textures = BTreeMap::new();
        for material in &source.materials {
            for name in &material.textures {
                textures.entry(name.to_ascii_lowercase()).or_insert(name);
            }
        }
        let mut missing = Vec::new();
        let mut placeholders = Vec::new();
        for (key, name) in &textures {
            match source.texture(name) {
                None => missing.push(key.as_str()),
                Some(t) if t.width == 1 && t.height == 1 && t.rgba == [255, 0, 255, 255] => {
                    placeholders.push(key.as_str())
                }
                Some(_) => {}
            }
        }
        eprintln!(
            "TEXTURES unique_diffuse={} missing={missing:?} placeholders={placeholders:?}",
            textures.len()
        );
        let settings = EnvironmentSettings::for_zone(zone);
        let sky_result = openeq_assets::environment::load_sky(&base, zone, 0.5);
        eprintln!("SKY result={:?}", sky_result.as_ref().map(|s| &s.weather));
        let sky = sky_result.ok();
        eprintln!("ENVIRONMENT {settings:?} authored_sky={}", sky.is_some());
        renderer.set_environment(settings, sky.as_ref());
        let upload = Instant::now();
        let gpu = GpuScene::build(renderer.device(), renderer.queue(), &source).unwrap();
        renderer
            .device()
            .poll(wgpu::PollType::wait_indefinitely())
            .unwrap();
        eprintln!(
            "UPLOAD cpu_and_completion_ms={:.3} draws={} instances={} atlas_layers={} vertices_bytes={} indices_bytes={} lights={} bounds={:?}..{:?}",
            upload.elapsed().as_secs_f64() * 1000.,
            gpu.draws.len(),
            gpu.draws.iter().map(|d| d.instance_count).sum::<u32>(),
            gpu.atlas.depth_or_array_layers(),
            gpu.vertices.size(),
            gpu.indices.size(),
            gpu.light_count,
            gpu.bounds_min,
            gpu.bounds_max
        );
        eprintln!(
            "SUBMITTED instanced_triangles={}",
            gpu.draws
                .iter()
                .map(|d| u64::from(d.index_count / 3) * u64::from(d.instance_count))
                .sum::<u64>()
        );
        eprintln!("PROFILE {zone} full_corrected");
        profile(&mut renderer, &gpu, &camera);
        let full = capture(&mut renderer, &gpu, &camera);
        let repeated = capture(&mut renderer, &gpu, &camera);
        assert_eq!(full, repeated, "same-camera/time capture must be stable");
        // A second full-scene view starts above an actual restored horizontal
        // face. This is a fixed offline diagnostic, not a verified player route.
        if let Some((floor_polygon, yaw)) = match zone {
            "thenest" => Some((299832, 0f32)),
            "thundercrest" => Some((200458, 45f32)),
            _ => None,
        } {
            let (a, b, c, ordinal, _) = terrain.polygons[floor_polygon];
            assert!(!old_ids.contains(&ordinal));
            let [a, b, c] = [a, b, c].map(|i| Vec3::from(terrain.positions[i as usize]));
            assert!((b - a).cross(c - a).normalize().z > 0.95);
            let context = Camera {
                position: ((a + b + c) / 3. + Vec3::Z * 8.).to_array(),
                yaw: yaw.to_radians(),
                pitch: (-5f32).to_radians(),
                ..camera
            };
            eprintln!(
                "CONTEXT polygon={floor_polygon} ordinal={ordinal} eye={:?} yaw_deg={yaw} pitch_deg=-5",
                context.position
            );
            let pixels = capture(&mut renderer, &gpu, &context);
            image::save_buffer(
                output.join(format!("{zone}-context.png")),
                &pixels,
                WIDTH,
                HEIGHT,
                image::ColorType::Rgba8,
            )
            .unwrap();
        }
        drop(gpu);
        // Isolate vertex packing from the separately enabled second-color path.
        // With source sidecars removed in both comparisons, former eight-word
        // deduplication must still preserve primary geometry shading.
        let lighting = std::mem::take(&mut source.native_ter_lighting);
        let secondary_uv = std::mem::take(&mut source.secondary_ter_uv);
        let primary_gpu = GpuScene::build(renderer.device(), renderer.queue(), &source).unwrap();
        let primary_only = capture(&mut renderer, &primary_gpu, &camera);
        drop(primary_gpu);
        for (mesh_id, vertices, indices) in &mut former_packing {
            std::mem::swap(&mut source.meshes[*mesh_id].vertices, vertices);
            std::mem::swap(&mut source.meshes[*mesh_id].indices, indices);
        }
        let former = GpuScene::build(renderer.device(), renderer.queue(), &source).unwrap();
        assert_eq!(
            capture(&mut renderer, &former, &camera),
            primary_only,
            "source identity retention changed primary-only pixels in {zone}"
        );
        drop(former);
        for (mesh_id, vertices, indices) in &mut former_packing {
            std::mem::swap(&mut source.meshes[*mesh_id].vertices, vertices);
            std::mem::swap(&mut source.meshes[*mesh_id].indices, indices);
        }
        source.native_ter_lighting = lighting;
        source.secondary_ter_uv = secondary_uv;
        // This isolates the cost/appearance of the added source terrain. It
        // does not replay the former bindings on geometry that already drew.
        for mesh_id in restored {
            source.meshes[mesh_id].indices.clear();
        }
        let omitted_gpu = GpuScene::build(renderer.device(), renderer.queue(), &source).unwrap();
        eprintln!("PROFILE {zone} restored_groups_omitted");
        profile(&mut renderer, &omitted_gpu, &camera);
        let omitted = capture(&mut renderer, &omitted_gpu, &camera);
        drop(omitted_gpu);

        geometry.material = 0;
        let selected_scene = Scene::from_geometry(
            format!("{zone} source ordinal {ordinal}"),
            vec![material],
            vec![geometry],
            vec![diffuse],
        );
        renderer.set_environment(
            EnvironmentSettings {
                sky_enabled: false,
                ..Default::default()
            },
            None,
        );
        let selected_gpu =
            GpuScene::build(renderer.device(), renderer.queue(), &selected_scene).unwrap();
        let isolated = capture(&mut renderer, &selected_gpu, &camera);
        drop(selected_gpu);
        let empty_scene = Scene::from_geometry("empty reference".into(), vec![], vec![], vec![]);
        let empty_gpu = GpuScene::build(renderer.device(), renderer.queue(), &empty_scene).unwrap();
        let empty = capture(&mut renderer, &empty_gpu, &camera);
        let different = |a: &[u8], b: &[u8]| {
            a.chunks_exact(4)
                .zip(b.chunks_exact(4))
                .filter(|(a, b)| a != b)
                .count()
        };
        eprintln!(
            "PIXELS full_vs_omitted={} isolated_vs_empty={}",
            different(&full, &omitted),
            different(&isolated, &empty)
        );
        assert!(
            different(&full, &omitted) > 5000,
            "restored terrain should be visible in the full scene"
        );
        assert!(
            different(&isolated, &empty) > 5000,
            "selected restored group should be visible"
        );
        for (label, pixels) in [
            ("full", full),
            ("restored-omitted", omitted),
            ("isolated", isolated),
        ] {
            image::save_buffer(
                output.join(format!("{zone}-{label}.png")),
                &pixels,
                WIDTH,
                HEIGHT,
                image::ColorType::Rgba8,
            )
            .unwrap();
        }
    }
}
