//! Bounded CPU diagnostics for four original Plane of Knowledge particle bodies.
//!
//! This is not connected to scene rendering. Coordinates are native WLD world
//! coordinates. See `docs/WLD_PARTICLE_SAMPLER.md` for native evidence and limits.

mod random;
pub use random::DiagnosticRandom;

use openeq_assets::wld::ParticleCloud;
use std::collections::VecDeque;

#[cfg(test)]
mod tests;

/// Exact authored families, selected by all twenty fixed source words.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PokDefinition {
    Smoke,
    Flame301,
    Flame308,
    Flame500,
}

const BODIES: [[u32; 20]; 4] = [
    [
        4, 3, 3, 0x30500, 50, 0, 0, 0, 0, 0, 0x3ecccccd, 0x42700000, 3000, 0x3f800000, 0,
        0x3f800000, 0, 160, 0x3f800000, 0x3f646464,
    ],
    [
        4, 3, 3, 0x30500, 40, 0, 0, 0, 0, 0, 0x40000000, 0x41700000, 650, 0x40400000, 0,
        0x3f800000, 0, 60, 0x3f800000, 0x3fffffff,
    ],
    [
        4, 3, 3, 0x30500, 10, 0, 0, 0, 0, 0, 0x3e99999a, 0x41a00000, 750, 0x3f333333, 0,
        0x3f800000, 0, 90, 0x3e800000, 0x3effffff,
    ],
    [
        4, 3, 3, 0x30500, 40, 0, 0, 0, 0, 0, 0x3f800000, 0x41700000, 650, 0x40000000, 0,
        0x3f800000, 0, 60, 0x3f99999a, 0x3fffffff,
    ],
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SampleError {
    UnsupportedDefinition,
    InvalidFrame,
    UnsupportedOwnerTransform,
}

impl std::fmt::Display for SampleError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::UnsupportedDefinition => "particle body is outside the four verified PoK families",
            Self::InvalidFrame => "particle frame contains unsupported numeric inputs",
            Self::UnsupportedOwnerTransform => "particle owner requires a finite, uniform orthogonal transform with scale at most 1000",
        })
    }
}
impl std::error::Error for SampleError {}

impl PokDefinition {
    pub fn from_cloud(cloud: &ParticleCloud) -> Result<Self, SampleError> {
        if cloud.optional_vectors.is_some()
            || cloud.optional_block.is_some()
            || cloud.texture_reference.is_none()
            || !cloud.tail.is_empty()
        {
            return Err(SampleError::UnsupportedDefinition);
        }
        [Self::Smoke, Self::Flame301, Self::Flame308, Self::Flame500]
            .into_iter()
            .find(|definition| cloud.fixed_words == *definition.fixed_words())
            .ok_or(SampleError::UnsupportedDefinition)
    }

    pub fn fixed_words(self) -> &'static [u32; 20] {
        &BODIES[self as usize]
    }
    pub fn capacity(self) -> usize {
        self.fixed_words()[4] as usize
    }
    pub fn lifetime(self) -> f32 {
        (self.fixed_words()[12] as f64 * f64::from(0.001_f32)) as f32
    }
    pub fn size(self) -> f32 {
        f32::from_bits(self.fixed_words()[18])
    }
    fn speed(self) -> f32 {
        f32::from_bits(self.fixed_words()[13])
    }
    fn rate(self) -> f32 {
        (1000.0 / self.fixed_words()[17] as f64) as f32
    }
    fn radial_speed(self) -> f32 {
        // Native converter outputs, including its float table rounding.
        [0.906_347_2, 0.558_556_2, 0.175_340_88, 0.372_370_8][self as usize]
    }
    fn rgb(self) -> [u8; 3] {
        let packed = self.fixed_words()[19];
        [(packed >> 16) as u8, (packed >> 8) as u8, packed as u8]
    }
}

/// A finite, uniformly scaled orthogonal native node world matrix.
/// Particle birth axes incorporate the native mode-3 quarter turn: normalized
/// world rows `[1, 0, 2]`, not the intermediate emitter basis `[2, 0, -1]`.
#[derive(Debug, Clone, Copy)]
pub struct OwnerPose {
    origin: [f32; 3],
    axes: [[f32; 3]; 3],
    scale: f32,
}

impl OwnerPose {
    /// Native row-major, row-vector matrix; no OpenEQ coordinate conversion.
    /// Scale must be positive, at most 1000, with a finite f32 reciprocal.
    /// The scale ceiling is a diagnostic admission bound, not a native limit.
    pub fn from_native_world(matrix: [[f32; 4]; 4]) -> Result<Self, SampleError> {
        if !matrix.iter().flatten().all(|v| v.is_finite())
            || matrix[0][3] != 0.0
            || matrix[1][3] != 0.0
            || matrix[2][3] != 0.0
            || matrix[3][3] != 1.0
        {
            return Err(SampleError::UnsupportedOwnerTransform);
        }
        let rows: [[f32; 3]; 3] = std::array::from_fn(|i| std::array::from_fn(|j| matrix[i][j]));
        let norm = |a: [f32; 3]| a.iter().map(|v| f64::from(*v).powi(2)).sum::<f64>().sqrt();
        let scale = norm(rows[0]) as f32;
        if !scale.is_finite()
            || scale <= 0.0
            || scale > 1000.0
            || rows
                .iter()
                .any(|row| (norm(*row) / f64::from(scale) - 1.0).abs() > 1e-5)
        {
            return Err(SampleError::UnsupportedOwnerTransform);
        }
        let inv = (1.0 / f64::from(scale)) as f32;
        if !inv.is_finite() {
            return Err(SampleError::UnsupportedOwnerTransform);
        }
        let normalized = rows.map(|row| row.map(|v| (f64::from(v) * f64::from(inv)) as f32));
        for (a, b) in [(0, 1), (1, 2), (2, 0)] {
            if (0..3)
                .map(|i| f64::from(normalized[a][i]) * f64::from(normalized[b][i]))
                .sum::<f64>()
                .abs()
                > 1e-5
            {
                return Err(SampleError::UnsupportedOwnerTransform);
            }
        }
        Ok(Self {
            origin: [matrix[3][0], matrix[3][1], matrix[3][2]],
            axes: [normalized[1], normalized[0], normalized[2]],
            scale,
        })
    }
    pub fn origin(self) -> [f32; 3] {
        self.origin
    }
    pub fn scale(self) -> f32 {
        self.scale
    }
}

/// The caller resolves engine visibility/owner policy. None is inferred here.
#[derive(Debug, Clone, Copy)]
pub struct FrameInput {
    pub delta_seconds: f32,
    pub owner: OwnerPose,
    pub camera_position: [f32; 3],
    /// Native owner suppression clears occupancy and prevents births this call.
    pub owner_suppressed: bool,
    /// Native emitter view gate: axial motion/life still advance when false.
    pub radial_motion_visible: bool,
    pub draw_context: i32,
    pub owner_alpha: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Particle {
    pub birth_origin: [f32; 3],
    /// Axial, radial cosine, radial sine basis, retained across owner movement.
    pub birth_axes: [[f32; 3]; 3],
    pub azimuth: u16,
    pub radial_distance: f32,
    pub axial_distance: f32,
    pub radial_speed: f32,
    pub axial_speed: f32,
    pub total_life: f32,
    pub remaining_life: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ParticleSample {
    pub position: [f32; 3],
    pub size: f32,
    /// Evaluated before this call subtracts life; source high byte is not alpha.
    pub rgba: [u8; 4],
    /// Also requires the caller's per-particle drawing predicate to pass.
    pub draw: bool,
}

/// Fixed-capacity emitter. Each call has bounded births and at most 50 particles.
#[derive(Debug, Clone)]
pub struct Sampler {
    definition: PokDefinition,
    context: i32,
    remaining: f32,
    schedule_counter: i32,
    spawn_counter: u32,
    particles: VecDeque<Particle>,
}

impl Sampler {
    pub fn new(definition: PokDefinition, captured_context: i32) -> Self {
        Self {
            definition,
            context: captured_context,
            remaining: 9999.0,
            schedule_counter: -1,
            spawn_counter: 0,
            particles: VecDeque::with_capacity(definition.capacity()),
        }
    }
    pub fn particles(&self) -> &VecDeque<Particle> {
        &self.particles
    }
    /// Includes births discarded because the ring was full, like native +0x14.
    pub fn spawn_counter(&self) -> u32 {
        self.spawn_counter
    }
    pub fn remaining_duration(&self) -> f32 {
        self.remaining
    }

    /// Advance one original-style update. No implicit substeps or catch-up calls.
    /// `random` must be shared in caller update order to reproduce a global stream.
    /// The draw predicate supplies per-particle clipping; the sampler adds context
    /// and emitter view gates. Invalid input leaves the sampler/random unchanged.
    pub fn update(
        &mut self,
        frame: FrameInput,
        random: &mut DiagnosticRandom,
        mut draw_visible: impl FnMut(&ParticleSample) -> bool,
    ) -> Result<Vec<ParticleSample>, SampleError> {
        if !frame.delta_seconds.is_finite()
            || frame.delta_seconds < 0.0
            || !frame.camera_position.iter().all(|v| v.is_finite())
            || !frame.owner_alpha.is_finite()
            || !(0.0..=1.0).contains(&frame.owner_alpha)
        {
            return Err(SampleError::InvalidFrame);
        }
        let dt = frame.delta_seconds.min(1.0);
        self.remaining = self.remaining.max(0.0);
        if frame.owner_suppressed {
            self.particles.clear();
        }
        while self
            .particles
            .front()
            .is_some_and(|p| p.remaining_life <= 0.0)
        {
            self.particles.pop_front();
        }
        if self.remaining > 0.0 && !frame.owner_suppressed {
            let target =
                ((9999.0 - f64::from(self.remaining)) * f64::from(self.definition.rate())) as i32;
            let mut births = 0;
            if self.schedule_counter < 0 && target >= 0 {
                births = 1;
                self.schedule_counter = 0;
            }
            if target > self.schedule_counter {
                let distance_squared = (0..3)
                    .map(|i| {
                        (f64::from(frame.owner.origin[i]) - f64::from(frame.camera_position[i]))
                            .powi(2)
                    })
                    .sum::<f64>() as f32;
                let distance = f64::from(distance_squared).sqrt();
                let floor = (16.0 / self.definition.capacity() as f64).min(1.0) as f32;
                let factor = (80.0 / distance).min(1.0).max(f64::from(floor));
                let batch = (factor * f64::from(target - self.schedule_counter)) as i32;
                if batch > 0 {
                    births += batch;
                    self.schedule_counter = target;
                }
            }
            self.spawn_counter += births as u32;
            let available = self.definition.capacity() - self.particles.len();
            for _ in 0..(births as usize).min(available) {
                let (azimuth, fraction) = random.birth();
                let life = self.definition.lifetime().min(self.remaining);
                self.particles.push_back(Particle {
                    birth_origin: frame.owner.origin,
                    birth_axes: frame.owner.axes,
                    azimuth,
                    radial_distance: 0.0,
                    axial_distance: 0.0,
                    radial_speed: (f64::from(fraction) * f64::from(self.definition.radial_speed()))
                        as f32,
                    axial_speed: self.definition.speed(),
                    total_life: life,
                    remaining_life: life,
                });
            }
        }
        let mut samples = Vec::with_capacity(self.particles.len());
        let inverse_fade = (1.0 / f64::from(self.definition.lifetime())) as f32;
        for particle in &mut self.particles {
            particle.axial_distance = (f64::from(particle.axial_distance)
                + f64::from(particle.axial_speed) * f64::from(dt) * f64::from(frame.owner.scale))
                as f32;
            if frame.radial_motion_visible {
                particle.radial_distance = (f64::from(particle.radial_distance)
                    + f64::from(particle.radial_speed)
                        * f64::from(dt)
                        * f64::from(frame.owner.scale))
                    as f32;
            }
            let (sine, cosine) = random.sin_cos(particle.azimuth);
            let radial_cos = (f64::from(cosine) * f64::from(particle.radial_distance)) as f32;
            let radial_sin = f64::from(sine) * f64::from(particle.radial_distance);
            let position = std::array::from_fn(|i| {
                (f64::from(particle.birth_origin[i])
                    + f64::from(particle.birth_axes[0][i]) * f64::from(particle.axial_distance)
                    + f64::from(particle.birth_axes[1][i]) * f64::from(radial_cos)
                    + f64::from(particle.birth_axes[2][i]) * radial_sin) as f32
            });
            let fade =
                (f64::from(inverse_fade) * f64::from(particle.remaining_life)).clamp(0.0, 1.0);
            let alpha = (fade * f64::from(frame.owner_alpha) * 255.0) as f32;
            let [red, green, blue] = self.definition.rgb();
            let mut sample = ParticleSample {
                position,
                size: self.definition.size() * frame.owner.scale,
                rgba: [red, green, blue, alpha.round_ties_even() as u8],
                draw: false,
            };
            sample.draw = frame.radial_motion_visible
                && self.context == frame.draw_context
                && draw_visible(&sample);
            samples.push(sample);
            particle.remaining_life = (f64::from(particle.remaining_life) - f64::from(dt)) as f32;
        }
        self.remaining = (f64::from(self.remaining) - f64::from(dt)) as f32;
        Ok(samples)
    }
}
