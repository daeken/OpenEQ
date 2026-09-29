//! Uploading an asset-level scene into GPU buffers.
//!
//! Textures all live in one array texture. EverQuest textures are mostly 256x256
//! but not uniformly, so each is resized to fit for this first pass.
//!
//! Material parameters are stored per vertex rather than in a material buffer.
//! The asset pipeline already splits geometry per material, so this costs no
//! extra vertices and removes a binding and a dynamic-offset dance.

use std::collections::HashMap;

use bytemuck::{Pod, Zeroable};
use glam::{Mat4, Quat, Vec3};
use openeq_assets::Scene;

/// Side length of every layer in the texture array.
pub const ATLAS_SIZE: u32 = 256;

pub const FLAG_ALPHA_MASK: u32 = 1;
pub const FLAG_TRANSPARENT: u32 = 2;
pub const FLAG_EMISSIVE: u32 = 4;
pub const FLAG_WATER: u32 = 8;

/// Additional parameters for EQG water, indexed by the vertex's material ID.
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct WaterParams {
    color1: [f32; 4],
    color2: [f32; 4],
    reflection_color: [f32; 4],
    /// Fresnel bias, Fresnel power, reflection amount, indexed UV scale.
    params: [f32; 4],
    /// Normal map layer, environment map layer, indexed UV mode, reserved.
    layers: [u32; 4],
}

/// A vertex as the renderer wants it: geometry plus material parameters.
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct Vertex {
    pub position: [f32; 3],
    pub normal: [f32; 3],
    pub uv: [f32; 2],
    /// First layer of this material's texture animation.
    pub layer: u32,
    pub material: u32,
    pub frame_count: u32,
    pub flags: u32,
    /// Milliseconds per animation frame.
    pub frame_ms: u32,
}

/// Per-instance model matrix in EverQuest space.
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct Instance {
    pub columns: [[f32; 4]; 4],
}

impl Instance {
    pub const IDENTITY: Self = Self {
        columns: [
            [1.0, 0.0, 0.0, 0.0],
            [0.0, 1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
            [0.0, 0.0, 0.0, 1.0],
        ],
    };

    pub fn from_parts(position: [f32; 3], rotation: [f32; 4], scale: [f32; 3]) -> Self {
        let matrix = Mat4::from_scale_rotation_translation(
            Vec3::from(scale),
            Quat::from_xyzw(rotation[0], rotation[1], rotation[2], rotation[3]),
            Vec3::from(position),
        );
        Self {
            columns: matrix.to_cols_array_2d(),
        }
    }
}

/// A single indexed, instanced draw.
#[derive(Debug, Clone, Copy)]
pub struct DrawCall {
    pub index_start: u32,
    pub index_count: u32,
    pub base_vertex: i32,
    pub instance_start: u32,
    pub instance_count: u32,
    /// Non-water materials with fractional alpha need the forward blend pass.
    pub transparent: bool,
}

/// GPU-resident scene: geometry, instances, draws, textures and lights.
pub struct GpuScene {
    pub name: String,
    pub vertices: wgpu::Buffer,
    pub indices: wgpu::Buffer,
    pub instances: wgpu::Buffer,
    pub draws: Vec<DrawCall>,
    pub atlas: wgpu::Texture,
    pub atlas_view: wgpu::TextureView,
    pub atlas_linear_view: wgpu::TextureView,
    pub water_materials: wgpu::Buffer,
    pub lights: wgpu::Buffer,
    pub(crate) light_grid: wgpu::Buffer,
    light_grid_dimensions: [u32; 2],
    pub light_count: u32,
    pub bounds_min: Vec3,
    pub bounds_max: Vec3,
    vertex_data: Vec<Vertex>,
}

impl GpuScene {
    /// Diagnostic A/B switch: disabling spatial lookup evaluates every zone
    /// light, retaining the same lights and shading. No scene rebuild needed.
    pub fn set_light_grid_enabled(&self, queue: &wgpu::Queue, enabled: bool) {
        let dimensions = if enabled {
            self.light_grid_dimensions
        } else {
            [0; 2]
        };
        queue.write_buffer(&self.light_grid, 16, bytemuck::cast_slice(&dimensions));
    }

    /// Updates a fixed-topology pose without re-uploading textures or indices.
    pub fn update_geometry(
        &mut self,
        queue: &wgpu::Queue,
        meshes: &[openeq_assets::mesh::Geometry],
    ) {
        let count: usize = meshes
            .iter()
            .map(|m| m.vertices.len() / openeq_assets::mesh::VERTEX_STRIDE)
            .sum();
        assert_eq!(
            count,
            self.vertex_data.len(),
            "animated geometry changed topology"
        );
        for (out, v) in self.vertex_data.iter_mut().zip(
            meshes
                .iter()
                .flat_map(|m| m.vertices.chunks_exact(openeq_assets::mesh::VERTEX_STRIDE)),
        ) {
            out.position.copy_from_slice(&v[..3]);
            out.normal.copy_from_slice(&v[3..6]);
        }
        queue.write_buffer(&self.vertices, 0, bytemuck::cast_slice(&self.vertex_data));
    }

    pub fn update_instances(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        instances: &[Instance],
    ) {
        let bytes = bytemuck::cast_slice(instances);
        if bytes.len() as u64 > self.instances.size() {
            self.instances = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("dynamic character instances"),
                size: (bytes.len() as u64).next_power_of_two(),
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
        }
        if !bytes.is_empty() {
            queue.write_buffer(&self.instances, 0, bytes);
        }
    }

    pub fn build(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        scene: &Scene,
    ) -> anyhow::Result<Self> {
        let atlas = build_atlas(device, queue, scene)?;
        let water: Vec<WaterParams> = scene
            .materials
            .iter()
            .map(|material| {
                let Some(water) = &material.water else {
                    return WaterParams::zeroed();
                };
                let layer = |name: Option<&String>| {
                    name.and_then(|name| atlas.layers.get(&name.to_ascii_lowercase()).copied())
                        .unwrap_or(u32::MAX)
                };
                WaterParams {
                    color1: water.color1,
                    color2: water.color2,
                    reflection_color: water.reflection_color,
                    params: [
                        water.fresnel_bias,
                        water.fresnel_power,
                        water.reflection_amount,
                        water.indexed_uv_scale.unwrap_or(0.0),
                    ],
                    layers: [
                        layer(material.normal_map.as_ref()),
                        water
                            .environment_map
                            .as_ref()
                            .and_then(|name| {
                                atlas.environments.get(&name.to_ascii_lowercase()).copied()
                            })
                            .unwrap_or(u32::MAX),
                        u32::from(water.indexed_uv_scale.is_some()),
                        0,
                    ],
                }
            })
            .collect();
        let water_materials = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("water materials"),
            size: (water.len().max(1) * std::mem::size_of::<WaterParams>()) as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        queue.write_buffer(&water_materials, 0, bytemuck::cast_slice(&water));

        let mut vertices: Vec<Vertex> = Vec::new();
        let mut indices: Vec<u32> = Vec::new();
        let mut draws: Vec<DrawCall> = Vec::new();
        let mut instances: Vec<Instance> = Vec::new();
        let mut bounds_min = Vec3::splat(f32::MAX);
        let mut bounds_max = Vec3::splat(f32::MIN);

        // Which geometry indices belong to placeable objects rather than to the
        // zone itself; those get instanced, the rest are drawn once.
        let mut object_of_mesh: HashMap<usize, usize> = HashMap::new();
        for (object_index, object) in scene.objects.iter().enumerate() {
            for mesh in &object.meshes {
                object_of_mesh.insert(*mesh, object_index);
            }
        }
        let mut object_instances: Vec<Vec<Instance>> = vec![Vec::new(); scene.objects.len()];
        for instance in &scene.instances {
            let Some(index) = scene
                .objects
                .iter()
                .position(|object| object.name == instance.object)
            else {
                continue;
            };
            object_instances[index].push(Instance::from_parts(
                instance.position,
                instance.rotation,
                instance.scale,
            ));
        }

        for (mesh_index, geometry) in scene.meshes.iter().enumerate() {
            let material = &scene.materials[geometry.material];
            let (layer, frame_count) = atlas.materials[&material_layer_key(material)];
            let mut flags = 0u32;
            if material.alpha_mask {
                flags |= FLAG_ALPHA_MASK;
            }
            if material.transparent {
                flags |= FLAG_TRANSPARENT;
            }
            if material.emissive {
                flags |= FLAG_EMISSIVE;
            }
            if material.water.is_some() {
                flags |= FLAG_WATER;
            }

            let base_vertex = vertices.len() as i32;
            for vertex in geometry
                .vertices
                .chunks_exact(openeq_assets::mesh::VERTEX_STRIDE)
            {
                bounds_min = bounds_min.min(Vec3::new(vertex[0], vertex[1], vertex[2]));
                bounds_max = bounds_max.max(Vec3::new(vertex[0], vertex[1], vertex[2]));
                vertices.push(Vertex {
                    position: [vertex[0], vertex[1], vertex[2]],
                    normal: [vertex[3], vertex[4], vertex[5]],
                    uv: [vertex[6], vertex[7]],
                    layer,
                    material: geometry.material as u32,
                    frame_count,
                    flags,
                    frame_ms: material.anim_speed.max(1),
                });
            }
            let index_start = indices.len() as u32;
            indices.extend_from_slice(&geometry.indices);
            let index_count = geometry.indices.len() as u32;

            let (instance_start, instance_count) = match object_of_mesh.get(&mesh_index) {
                Some(object_index) => {
                    let list = &object_instances[*object_index];
                    if list.is_empty() {
                        continue;
                    }
                    let start = instances.len() as u32;
                    instances.extend_from_slice(list);
                    (start, list.len() as u32)
                }
                None => {
                    let start = instances.len() as u32;
                    instances.push(Instance::IDENTITY);
                    (start, 1)
                }
            };

            draws.push(DrawCall {
                index_start,
                index_count,
                base_vertex,
                instance_start,
                instance_count,
                transparent: material.transparent && material.water.is_none(),
            });
        }

        let lights: Vec<[f32; 8]> = scene
            .lights
            .iter()
            .map(|light| {
                [
                    light.position[0],
                    light.position[1],
                    light.position[2],
                    light.radius.max(24.0),
                    light.color[0],
                    light.color[1],
                    light.color[2],
                    light.attenuation,
                ]
            })
            .collect();
        let light_count = lights.len() as u32;
        let grid = crate::light_grid::LightGrid::build(&lights);
        let light_grid_dimensions = grid.stats.dimensions;
        if !lights.is_empty() {
            tracing::info!(enabled=grid.stats.enabled, lights=grid.stats.light_count,
                dimensions=?grid.stats.dimensions, cell_size=grid.stats.cell_size,
                references=grid.stats.references, max_cell_lights=grid.stats.max_cell_lights,
                mean_cell_lights=grid.stats.mean_cell_lights, fallback=grid.stats.fallback_reason,
                "zone light grid built");
        }
        let light_grid = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("zone light spatial index"),
            size: grid.bytes.len() as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        queue.write_buffer(&light_grid, 0, &grid.bytes);

        let vertex_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("scene vertices"),
            size: (vertices.len() * std::mem::size_of::<Vertex>()).max(4) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        queue.write_buffer(&vertex_buffer, 0, bytemuck::cast_slice(&vertices));

        let index_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("scene indices"),
            size: (indices.len() * std::mem::size_of::<u32>()).max(4) as u64,
            usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        queue.write_buffer(&index_buffer, 0, bytemuck::cast_slice(&indices));

        let instance_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("scene instances"),
            size: (instances.len() * std::mem::size_of::<Instance>()).max(4) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        queue.write_buffer(&instance_buffer, 0, bytemuck::cast_slice(&instances));

        let light_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("scene lights"),
            size: (lights.len() * 32).max(32) as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        queue.write_buffer(&light_buffer, 0, bytemuck::cast_slice(&lights));

        Ok(Self {
            name: scene.name.clone(),
            vertices: vertex_buffer,
            vertex_data: vertices,
            indices: index_buffer,
            instances: instance_buffer,
            draws,
            atlas: atlas.texture,
            atlas_view: atlas.view,
            atlas_linear_view: atlas.linear_view,
            water_materials,
            lights: light_buffer,
            light_grid,
            light_grid_dimensions,
            light_count,
            bounds_min: if bounds_min.x == f32::MAX {
                Vec3::ZERO
            } else {
                bounds_min
            },
            bounds_max: if bounds_max.x == f32::MIN {
                Vec3::ZERO
            } else {
                bounds_max
            },
        })
    }
}

/// Identifies a material's texture set, used to look up its atlas layers.
pub type MaterialKey = (String, u32);

pub fn material_layer_key(material: &openeq_assets::mesh::Material) -> MaterialKey {
    (material.textures.join(","), material.anim_speed)
}

struct Atlas {
    texture: wgpu::Texture,
    view: wgpu::TextureView,
    linear_view: wgpu::TextureView,
    materials: HashMap<MaterialKey, (u32, u32)>,
    layers: HashMap<String, u32>,
    environments: HashMap<String, u32>,
}

fn build_atlas(device: &wgpu::Device, queue: &wgpu::Queue, scene: &Scene) -> anyhow::Result<Atlas> {
    let mut layers: Vec<image::RgbaImage> = Vec::new();
    let mut mapping: HashMap<MaterialKey, (u32, u32)> = HashMap::new();
    let mut layer_of_name: HashMap<String, u32> = HashMap::new();
    let mut environments = HashMap::new();

    for material in &scene.materials {
        let key = material_layer_key(material);
        if mapping.contains_key(&key) {
            continue;
        }
        let names: Vec<String> = material
            .textures
            .iter()
            .map(|name| name.to_ascii_lowercase())
            .collect();

        // The shader picks an animation frame as `base + frame`, so a material's
        // frames must sit in consecutive layers. Reuse an existing run when it
        // already does, and otherwise add fresh copies side by side.
        let existing: Option<Vec<u32>> = names
            .iter()
            .map(|name| layer_of_name.get(name).copied())
            .collect();
        let base = match existing {
            Some(found) if !found.is_empty() && is_consecutive(&found) => found[0],
            _ => {
                let first = layers.len() as u32;
                for name in &material.textures {
                    let image = decode_texture(scene, name);
                    layer_of_name.insert(name.to_ascii_lowercase(), layers.len() as u32);
                    layers.push(image);
                }
                if layers.len() as u32 == first {
                    // A material with no textures still needs a layer to draw.
                    layers.push(placeholder_texture());
                }
                first
            }
        };
        mapping.insert(key, (base, material.textures.len().max(1) as u32));
    }

    // Water's normal and reflection maps are separate layers, never frames of
    // its diffuse animation. Missing optional maps use shader defaults.
    for material in &scene.materials {
        let Some(water) = &material.water else {
            continue;
        };
        for name in [material.normal_map.as_ref()].into_iter().flatten() {
            let key = name.to_ascii_lowercase();
            if layer_of_name.contains_key(&key) {
                continue;
            }
            let Some(texture) = scene.texture(name) else {
                continue;
            };
            if texture.rgba == [255, 0, 255, 255] {
                continue;
            }
            let Some(image) =
                image::RgbaImage::from_raw(texture.width, texture.height, texture.rgba)
            else {
                continue;
            };
            layer_of_name.insert(key, layers.len() as u32);
            layers.push(image);
        }
        if let Some(name) = &water.environment_map {
            let key = name.to_ascii_lowercase();
            if let std::collections::hash_map::Entry::Vacant(entry) = environments.entry(key)
                && let Some(faces) = scene.texture_cube(name)
            {
                let faces: Option<Vec<_>> = faces
                    .into_iter()
                    .map(|face| image::RgbaImage::from_raw(face.width, face.height, face.rgba))
                    .collect();
                if let Some(faces) = faces {
                    entry.insert(layers.len() as u32);
                    layers.extend(faces);
                }
            }
        }
    }

    if layers.is_empty() {
        layers.push(placeholder_texture());
    }

    // Zones reference hundreds of distinct textures; Plane of Knowledge alone
    // wants around 480 layers, well past the default cap of 256.
    let limit = device.limits().max_texture_array_layers;
    anyhow::ensure!(
        layers.len() as u32 <= limit,
        "{} needs {} texture array layers but this device allows {limit}",
        scene.name,
        layers.len()
    );

    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("texture atlas"),
        size: wgpu::Extent3d {
            width: ATLAS_SIZE,
            height: ATLAS_SIZE,
            depth_or_array_layers: layers.len() as u32,
        },
        mip_level_count: ATLAS_SIZE.ilog2() + 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8UnormSrgb,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[wgpu::TextureFormat::Rgba8Unorm],
    });

    for (index, image) in layers.iter().enumerate() {
        let mut resized = image::imageops::resize(
            image,
            ATLAS_SIZE,
            ATLAS_SIZE,
            image::imageops::FilterType::Triangle,
        );
        for mip in 0..=ATLAS_SIZE.ilog2() {
            let size = ATLAS_SIZE >> mip;
            if mip > 0 {
                resized = image::imageops::resize(
                    &resized,
                    size,
                    size,
                    image::imageops::FilterType::Triangle,
                );
            }
            queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: &texture,
                    mip_level: mip,
                    origin: wgpu::Origin3d {
                        x: 0,
                        y: 0,
                        z: index as u32,
                    },
                    aspect: wgpu::TextureAspect::All,
                },
                &resized,
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(size * 4),
                    rows_per_image: Some(size),
                },
                wgpu::Extent3d {
                    width: size,
                    height: size,
                    depth_or_array_layers: 1,
                },
            );
        }
    }

    let view = texture.create_view(&wgpu::TextureViewDescriptor {
        label: Some("atlas view"),
        dimension: Some(wgpu::TextureViewDimension::D2Array),
        ..Default::default()
    });
    // Normals are data, so sampling them must bypass the diffuse sRGB decode.
    let linear_view = texture.create_view(&wgpu::TextureViewDescriptor {
        label: Some("linear atlas view"),
        format: Some(wgpu::TextureFormat::Rgba8Unorm),
        dimension: Some(wgpu::TextureViewDimension::D2Array),
        ..Default::default()
    });
    Ok(Atlas {
        texture,
        view,
        linear_view,
        materials: mapping,
        layers: layer_of_name,
        environments,
    })
}

/// Whether layer indices run `n, n + 1, n + 2, ...`.
fn is_consecutive(layers: &[u32]) -> bool {
    layers.windows(2).all(|pair| pair[1] == pair[0] + 1)
}

fn placeholder_texture() -> image::RgbaImage {
    image::RgbaImage::from_pixel(1, 1, image::Rgba([255, 0, 255, 255]))
}

fn decode_texture(scene: &Scene, name: &str) -> image::RgbaImage {
    scene
        .texture(name)
        .and_then(|texture| image::RgbaImage::from_raw(texture.width, texture.height, texture.rgba))
        .unwrap_or_else(placeholder_texture)
}

/// A free-fly camera. Position and angles are in EverQuest coordinates.
#[derive(Debug, Clone, Copy)]
pub struct Camera {
    pub position: [f32; 3],
    /// Radians, 0 faces +Y (north).
    pub yaw: f32,
    /// Radians, positive looks up.
    pub pitch: f32,
    pub fov_y: f32,
}

impl Default for Camera {
    fn default() -> Self {
        Self {
            position: [0.0, 0.0, 0.0],
            yaw: 0.0,
            pitch: -0.15,
            fov_y: 70f32.to_radians(),
        }
    }
}

impl Camera {
    /// Converts an EverQuest position to renderer space (`(x, y, z)` -> `(x, z, -y)`).
    pub fn to_world(position: [f32; 3]) -> Vec3 {
        Vec3::new(position[0], position[2], -position[1])
    }

    pub fn forward(&self) -> Vec3 {
        let (sin_yaw, cos_yaw) = self.yaw.sin_cos();
        let (sin_pitch, cos_pitch) = self.pitch.sin_cos();
        let eq = Vec3::new(sin_yaw * cos_pitch, cos_yaw * cos_pitch, sin_pitch);
        Vec3::new(eq.x, eq.z, -eq.y)
    }

    pub fn view_projection(&self, aspect: f32, near: f32, far: f32) -> Mat4 {
        let eye = Self::to_world(self.position);
        let view = Mat4::look_at_rh(eye, eye + self.forward(), Vec3::Y);
        let projection = Mat4::perspective_rh(self.fov_y, aspect, near, far);
        projection * view
    }
}
