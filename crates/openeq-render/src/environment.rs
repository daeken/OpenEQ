//! Renderer-neutral zone atmosphere settings and original-client sky textures.
use bytemuck::{Pod, Zeroable};
use openeq_assets::liquid_regions::LiquidKind;
use openeq_assets::{
    environment::{SkyAssets, SkyColorMapLayout},
    texture::Texture,
};

/// Retained OpenEQ fallback while native non-sky/vision lighting is separate.
pub const DEFAULT_AMBIENT: [f32; 3] = [0.22, 0.24, 0.30];

#[derive(Debug, Clone, Copy)]
pub struct EnvironmentSettings {
    /// Native environment/time type (NewZone byte 520, PEQ time_type), not ztype.
    /// None leaves authored ambient admission disabled for standalone fixtures.
    pub zone_type: Option<u8>,
    /// Server fog color, normalized sRGB.
    pub fog_color: [f32; 3],
    pub fog_start: f32,
    pub fog_end: f32,
    pub fog_density: f32,
    pub fog_enabled: bool,
    pub sky_enabled: bool,
    pub authored_sky: bool,
    pub cloud_strength: f32,
    pub cloud_velocity: f32,
    pub cloud_scale: f32,
}
impl Default for EnvironmentSettings {
    fn default() -> Self {
        Self {
            zone_type: None,
            fog_color: [0.42, 0.50, 0.62],
            fog_start: 0.0,
            fog_end: 2000.0,
            fog_density: 0.0,
            fog_enabled: false,
            sky_enabled: true,
            authored_sky: false,
            cloud_strength: 0.0,
            cloud_velocity: 0.001,
            cloud_scale: 0.28,
        }
    }
}
/// Append this after `Globals.params`; layout matches the WGSL Environment.
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct EnvironmentUniform {
    pub fog_color: [f32; 4],
    pub fog_params: [f32; 4],
    pub sky_horizon: [f32; 4],
    pub sky_zenith: [f32; 4],
    pub sky_params: [f32; 4],
}
fn linear(v: f32) -> f32 {
    let v = if v.is_finite() {
        v.clamp(0.0, 1.0)
    } else {
        0.0
    };
    if v <= 0.04045 {
        v / 12.92
    } else {
        ((v + 0.055) / 1.055).powf(2.4)
    }
}
impl EnvironmentSettings {
    /// Presentation defaults for a camera inside a verified liquid volume.
    /// These palettes are client tuning, not authored zone fog or damage rules.
    /// Return a copy so resurfacing restores the exact server atmosphere/sky.
    pub fn with_view_liquid(mut self, liquid: Option<LiquidKind>) -> Self {
        let Some(liquid) = liquid else {
            return self;
        };
        let (color, distance) = match liquid {
            LiquidKind::Water => ([0.08, 0.22, 0.30], 100.),
            LiquidKind::FreezingWater => ([0.20, 0.32, 0.40], 72.),
            LiquidKind::OpaqueWater => ([0.08, 0.16, 0.09], 24.),
            LiquidKind::Lava => ([0.50, 0.12, 0.015], 24.),
        };
        self.fog_color = color;
        self.fog_start = 0.;
        self.fog_end = distance;
        self.fog_density = 0.;
        self.fog_enabled = true;
        self.sky_enabled = false;
        self
    }

    /// Offline PEQ snapshot; live clients replace this with OP_NewZone values.
    pub fn for_zone(name: &str) -> Self {
        let Some(zone) = openeq_assets::environment::load_zone_environment(name) else {
            return Self::default();
        };
        Self {
            zone_type: Some(zone.time_type),
            fog_color: zone.fog_color[0],
            fog_start: zone.fog_start[0],
            fog_end: zone.fog_end[0],
            fog_density: zone.fog_density,
            fog_enabled: zone.fog_end[0] > zone.fog_start[0],
            sky_enabled: !matches!(zone.time_type, 0 | 3 | 4) && zone.sky != 0,
            ..Self::default()
        }
    }

    /// Authored ambient under the current normal-vision rendering policy.
    /// Native packed RGB uses a per-channel floor of 20, independent of alpha.
    /// Keep normalized bytes in our existing linear lighting pipeline: native
    /// encoded-color arithmetic, vision effects and display transfer are separate.
    /// Missing/malformed/unsupported sky inputs reset to the retained fallback;
    /// this does not emulate native cached-sky or hourly-fallback lifecycle.
    pub fn normal_vision_ambient(&self, sky: Option<&SkyAssets>) -> [f32; 3] {
        if !matches!(self.zone_type, Some(1 | 2 | 5)) {
            return DEFAULT_AMBIENT;
        }
        let Some(colors) = sky.and_then(SkyAssets::raw_light_colors) else {
            return DEFAULT_AMBIENT;
        };
        [16, 8, 0].map(|shift| ((colors.ambient >> shift) as u8).max(20) as f32 / 255.)
    }

    pub fn apply_sky(&mut self, assets: &SkyAssets) {
        self.authored_sky = true;
        self.cloud_strength = if assets.cloud_texture.is_some() {
            1.0
        } else {
            0.0
        };
        self.cloud_velocity = assets.cloud_velocity;
    }
    /// Original effect Time is an unsigned millisecond clock reduced before
    /// conversion to seconds. Keep its precision for long-running sessions.
    /// Our epoch remains renderer startup; the native host uses timer startup.
    pub fn uniform_at(&self, elapsed: std::time::Duration) -> EnvironmentUniform {
        let milliseconds = elapsed.as_millis() as u32;
        let mut uniform = self.uniform();
        uniform.sky_params[3] = (milliseconds % 100_000) as f32 * 0.001;
        uniform
    }

    pub fn uniform(&self) -> EnvironmentUniform {
        let fog_valid = self.fog_enabled
            && self.fog_start.is_finite()
            && self.fog_end.is_finite()
            && self.fog_end > self.fog_start;
        EnvironmentUniform {
            fog_color: [
                linear(self.fog_color[0]),
                linear(self.fog_color[1]),
                linear(self.fog_color[2]),
                if fog_valid { 1.0 } else { 0.0 },
            ],
            fog_params: [
                if fog_valid {
                    self.fog_start.max(0.0)
                } else {
                    0.0
                },
                if fog_valid {
                    self.fog_end.max(1.0)
                } else {
                    1.0
                },
                self.fog_density,
                0.0,
            ],
            sky_horizon: [0.42, 0.50, 0.62, if self.authored_sky { 1.0 } else { 0.0 }],
            sky_zenith: [0.09, 0.16, 0.32, self.cloud_strength.clamp(0.0, 1.0)],
            sky_params: [
                self.cloud_velocity,
                self.cloud_scale,
                if self.sky_enabled { 1.0 } else { 0.0 },
                0.0,
            ],
        }
    }
}

/// Lighting bind group 3: color lookup, cloud opacity, cloud color lookup,
/// and a clamp sampler. A fallback group is always valid, even without assets.
pub struct SkyResources {
    pub layout: wgpu::BindGroupLayout,
    pub group: wgpu::BindGroup,
}

/// The native 32px sky color table stores a 31-sector, 29-step dome plus
/// auxiliary color swatches. Those swatches must never become sky pixels.
/// This live lookup remains an approximation of the native mesh: duplicating
/// pole colors around each row suppresses a pinwheel under bilinear sampling,
/// but also suppresses valid native near-pole ring colors. The opt-in assets
/// dome diagnostic preserves those colors and the exact source addressing.
fn color_map_upload(texture: &Texture, layout: SkyColorMapLayout) -> std::borrow::Cow<'_, Texture> {
    if layout != SkyColorMapLayout::OriginalDome {
        return std::borrow::Cow::Borrowed(texture);
    }
    debug_assert_eq!((texture.width, texture.height), (32, 32));
    let mut rgba = Vec::with_capacity(31 * 30 * 4);
    for y in 0..30 {
        for x in 0..31 {
            let column = if y == 0 || y == 29 { 0 } else { x };
            let offset = (y * 32 + column) * 4;
            rgba.extend_from_slice(&texture.rgba[offset..offset + 4]);
        }
    }
    std::borrow::Cow::Owned(Texture {
        name: texture.name.clone(),
        width: 31,
        height: 30,
        rgba,
    })
}
impl SkyResources {
    pub fn new(device: &wgpu::Device, queue: &wgpu::Queue, assets: Option<&SkyAssets>) -> Self {
        let entries = (0..3)
            .map(|binding| wgpu::BindGroupLayoutEntry {
                binding,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            })
            .chain(std::iter::once(wgpu::BindGroupLayoutEntry {
                binding: 3,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                count: None,
            }))
            .collect::<Vec<_>>();
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("zone sky textures"),
            entries: &entries,
        });
        let fallback = Texture {
            name: "sky fallback".into(),
            width: 1,
            height: 1,
            rgba: vec![255, 255, 255, 0],
        };
        let textures = [
            assets.map(|a| &a.color_map).unwrap_or(&fallback),
            assets
                .and_then(|a| a.cloud_texture.as_ref())
                .unwrap_or(&fallback),
            assets
                .and_then(|a| a.cloud_color_map.as_ref())
                .unwrap_or(&fallback),
        ];
        let layouts = [
            assets.map_or(SkyColorMapLayout::default(), |a| a.color_map_layout),
            SkyColorMapLayout::FullTexture,
            assets.map_or(SkyColorMapLayout::default(), |a| a.cloud_color_map_layout),
        ];
        let views = std::array::from_fn::<_, 3, _>(|index| {
            let texture = color_map_upload(textures[index], layouts[index]);
            let size = wgpu::Extent3d {
                width: texture.width,
                height: texture.height,
                depth_or_array_layers: 1,
            };
            let gpu = device.create_texture(&wgpu::TextureDescriptor {
                label: Some(&texture.name),
                size,
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8UnormSrgb,
                usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                view_formats: &[],
            });
            queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: &gpu,
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                &texture.rgba,
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(texture.width * 4),
                    rows_per_image: Some(texture.height),
                },
                size,
            );
            gpu.create_view(&Default::default())
        });
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("sky lookup sampler"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            address_mode_u: wgpu::AddressMode::Repeat,
            address_mode_v: wgpu::AddressMode::Repeat,
            ..Default::default()
        });
        let group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("zone sky"),
            layout: &layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&views[0]),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&views[1]),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(&views[2]),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
            ],
        });
        Self { layout, group }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn degenerate_fog_disables_instead_of_dividing_by_zero() {
        let mut settings = EnvironmentSettings {
            fog_enabled: true,
            fog_start: 500.,
            fog_end: 500.,
            ..Default::default()
        };
        assert_eq!(settings.uniform().fog_color[3], 0.0);
        settings.fog_end = 2000.0;
        settings.fog_color = [1.0, 0.5, 0.0];
        let uniform = settings.uniform();
        assert_eq!(uniform.fog_color[3], 1.0);
        assert!((uniform.fog_color[1] - 0.214041).abs() < 0.00001);
        assert_eq!(std::mem::size_of::<EnvironmentUniform>(), 80);
    }

    #[test]
    fn effect_time_matches_native_integer_clock_witnesses() {
        // Original full host-clock -> engine -> SetFloat binder outputs.
        // See docs/EQG_EFFECT_CLOCK.md. No wall clock or GPU is used here.
        let native = [
            (0_u64, 0x00000000_u32),
            (1_u64, 0x3a83126f_u32),
            (2_u64, 0x3b03126f_u32),
            (999_u64, 0x3f7fbe78_u32),
            (1_000_u64, 0x3f800000_u32),
            (12_345_u64, 0x4145851f_u32),
            (99_998_u64, 0x42c7fefa_u32),
            (99_999_u64, 0x42c7ff7e_u32),
            (100_000_u64, 0x00000000_u32),
            (100_001_u64, 0x3a83126f_u32),
            (100_999_u64, 0x3f7fbe78_u32),
            (123_456_789_u64, 0x426327f0_u32),
            (2_147_483_647_u64, 0x42a74b44_u32),
            (2_147_483_648_u64, 0x42a74bc7_u32),
            (4_294_967_295_u64, 0x4286970b_u32),
            (4_294_967_296_u64, 0x00000000_u32),
            (4_294_968_296_u64, 0x3f800000_u32),
        ];
        for (milliseconds, bits) in native {
            let uniform = EnvironmentSettings::default()
                .uniform_at(std::time::Duration::from_millis(milliseconds));
            assert_eq!(uniform.sky_params[3].to_bits(), bits, "{milliseconds} ms");
        }
        let submillisecond = std::time::Duration::from_nanos(999_999);
        assert_eq!(
            EnvironmentSettings::default()
                .uniform_at(submillisecond)
                .sky_params[3],
            0.
        );
        let settings = EnvironmentSettings::default();
        let mut timed = settings.uniform_at(std::time::Duration::from_secs(17));
        timed.sky_params[3] = 0.;
        assert_eq!(
            bytemuck::bytes_of(&timed),
            bytemuck::bytes_of(&settings.uniform())
        );
    }
}
