//! GPU resources that may be cloned into an asset-loading worker.
//! No window surface, frame targets, or active render state is shared.

use crate::{GpuActor, GpuScene};

#[derive(Clone)]
pub struct UploadContext {
    device: wgpu::Device,
    queue: wgpu::Queue,
    atlas_layout: wgpu::BindGroupLayout,
}

impl UploadContext {
    pub(crate) fn new(
        device: wgpu::Device,
        queue: wgpu::Queue,
        atlas_layout: wgpu::BindGroupLayout,
    ) -> Self {
        Self {
            device,
            queue,
            atlas_layout,
        }
    }

    pub fn device(&self) -> &wgpu::Device {
        &self.device
    }

    pub fn queue(&self) -> &wgpu::Queue {
        &self.queue
    }

    pub(crate) fn make_atlas_group(&self, scene: &GpuScene) -> wgpu::BindGroup {
        let sampler = self.device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("atlas sampler"),
            address_mode_u: wgpu::AddressMode::Repeat,
            address_mode_v: wgpu::AddressMode::Repeat,
            address_mode_w: wgpu::AddressMode::Repeat,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Linear,
            ..Default::default()
        });
        // Opaque baked tiles need true edge addressing at every mip level.
        // A level-zero half-texel inset cannot prevent repeat bleed at coarse LOD.
        let clamp_sampler = self.device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("clamped atlas sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Linear,
            ..Default::default()
        });
        self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("atlas bind group"),
            layout: &self.atlas_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&scene.atlas_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: scene.water_materials.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::TextureView(&scene.atlas_linear_view),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: wgpu::BindingResource::TextureView(&scene.terrain.view),
                },
                wgpu::BindGroupEntry {
                    binding: 5,
                    resource: scene.terrain.headers.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 6,
                    resource: scene.terrain.layers.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 7,
                    resource: scene.terrain.masks.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 8,
                    resource: wgpu::BindingResource::Sampler(&clamp_sampler),
                },
            ],
        })
    }

    pub fn prepare_actor(&self, scene: GpuScene) -> GpuActor {
        let atlas = self.make_atlas_group(&scene);
        GpuActor { scene, atlas }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        Camera, Renderer,
        actors::{ActorAction, ActorRenderer, ActorState},
        doors::{DoorRenderer, DoorState},
    };
    use openeq_assets::{Scene, loader};

    fn handles(draws: Vec<&GpuActor>) -> Vec<(wgpu::Buffer, wgpu::Texture, wgpu::BindGroup)> {
        draws
            .into_iter()
            .map(|actor| {
                (
                    actor.scene.vertices.clone(),
                    actor.scene.atlas.clone(),
                    actor.atlas.clone(),
                )
            })
            .collect()
    }

    #[test]
    #[ignore = "requires original character/door assets and a GPU"]
    fn worker_preloads_models_and_main_thread_reuses_their_gpu_resources() {
        let base = loader::default_client_dir().expect("original client directory");
        let mut renderer = Renderer::new_headless(256, 256).unwrap();
        let upload = renderer.upload_context();
        let states: Vec<_> = [ActorAction::Stand, ActorAction::Attack, ActorAction::Cast]
            .into_iter()
            .enumerate()
            .map(|(i, action)| ActorState {
                id: i as u32 + 1,
                race: 1,
                size: 6.,
                action,
                position: [i as f32 * 5., 0., 3.],
                ..Default::default()
            })
            .collect();
        let door_states = vec![
            DoorState {
                id: 1,
                name: "POKDOOR500".into(),
                size: 100,
                heading: 128.,
                open_type: 5,
                ..Default::default()
            },
            DoorState {
                id: 2,
                name: "POKDOOR500".into(),
                position: [20., 0., 0.],
                size: 100,
                heading: 128.,
                open_type: 5,
                ..Default::default()
            },
        ];
        let worker = std::thread::spawn(move || {
            let mut actors = ActorRenderer::load(&base, "poknowledge").unwrap();
            let mut doors = DoorRenderer::load(&base, "poknowledge").unwrap();
            // Cancellation before the first batch allocates no drawable model.
            assert!(!actors.preload_with_progress(&upload, &states, 10., |_, _| false));
            assert!(actors.draws().is_empty());
            assert!(!doors.preload_with_progress(&upload, &door_states, 10., |_, _| false));
            assert!(doors.draws().is_empty());
            let mut progress = Vec::new();
            assert!(
                actors.preload_with_progress(&upload, &states, 10., |done, total| {
                    progress.push((done, total));
                    true
                })
            );
            assert_eq!(progress.first(), Some(&(0, 1)));
            assert_eq!(progress.last(), Some(&(1, 1)));
            assert!(progress.windows(2).all(|p| p[0].0 <= p[1].0));
            doors.preload(&upload.clone(), &door_states, 10.);
            assert_eq!(actors.rendered_instances, 3);
            assert_eq!(actors.bounds().len(), 3);
            assert_eq!(actors.draws().len(), 1, "appearances must share one atlas");
            assert_eq!(doors.rendered_instances, 2);
            assert_eq!(doors.draws().len(), 1, "doors must share one model");
            assert!(doors.collision_world().triangle_count() > 0);
            (actors, doors, states, door_states)
        });
        // The original renderer remains usable while the worker owns only the
        // cloned upload resources; no window/surface crosses the thread boundary.
        let scene = Scene::from_geometry("loading frame".into(), vec![], vec![], vec![]);
        let scene = GpuScene::build(renderer.device(), renderer.queue(), &scene).unwrap();
        let camera = Camera {
            position: [5., -30., 10.],
            pitch: -0.2,
            ..Default::default()
        };
        renderer.render(&scene, &camera);
        let (mut actors, mut doors, states, door_states) = worker.join().unwrap();
        let actor_handles = handles(actors.draws());
        let door_handles = handles(doors.draws());
        let bounds = actors.bounds().clone();
        actors.update(&renderer, &states, 10.25);
        doors.update(&renderer, &door_states, 10.25);
        assert_eq!(
            actor_handles,
            handles(actors.draws()),
            "first frame reuploaded actors"
        );
        assert_eq!(
            door_handles,
            handles(doors.draws()),
            "first frame reuploaded doors"
        );
        assert_ne!(
            bounds,
            *actors.bounds(),
            "preloaded animation stopped updating"
        );
        let mut draws = actors.draws();
        draws.extend(doors.draws());
        renderer.render_with_actors(&scene, &camera, &draws);
        let (_, _, pixels) = renderer.read_rgba().unwrap();
        assert!(
            pixels
                .chunks_exact(4)
                .any(|p| p[0] < 100 && p[1] < 100 && p[2] < 100)
        );
    }
}
