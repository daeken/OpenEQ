//! Server-driven spell presentation using the installed client's particle data.
mod timeline;
use glam::{Quat, Vec3};
use openeq_assets::spell_effects::{
    EmitterDefinition, EmitterReference, SpellEffectCatalog, SpellEffectTextures,
};
use openeq_net::gameplay::GameplayEvent;
use openeq_render::{
    actors::ActorSockets,
    particles::{MAX_PARTICLES, ParticleBlend, ParticleFrame, ParticleInstance},
};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::Path,
    sync::Arc,
    time::{Duration, Instant},
};
use timeline::{Cue, Owner, Phase, Timeline};

pub struct EffectAssets {
    pub catalog: SpellEffectCatalog,
    pub textures: SpellEffectTextures,
    projectile_definitions: Vec<openeq_assets::spell_effects::EffectDefinition>,
    projectile_models: BTreeMap<String, u32>,
}
impl EffectAssets {
    /// Decodes once on the loading worker, never during an active frame.
    pub fn load(base: &Path) -> anyhow::Result<Self> {
        let mut catalog = SpellEffectCatalog::load(base)?;
        let mut projectile_definitions = Vec::new();
        let mut projectile_models = BTreeMap::new();
        match openeq_assets::spell_effects::load_projectile_effects(base) {
            Ok(projectiles) => {
                let mut mapped = BTreeMap::new();
                for (model, bindings) in projectiles.bindings {
                    let mut definition = openeq_assets::spell_effects::EffectDefinition {
                        name: model.clone(),
                        stages: std::array::from_fn(|_| Default::default()),
                    };
                    for (slot, binding) in bindings.into_iter().take(4).enumerate() {
                        let Some(emitter) = projectiles.emitters.get(binding.emitter_id as usize)
                        else {
                            continue;
                        };
                        let index = *mapped.entry(binding.emitter_id).or_insert_with(|| {
                            let index = catalog.emitters.len() as u32;
                            catalog.emitters.push(emitter.clone());
                            index
                        });
                        definition.stages[1].emitters[slot] = EmitterReference {
                            emitter_id: index,
                            ..Default::default()
                        };
                    }
                    projectile_models.insert(model, projectile_definitions.len() as u32);
                    projectile_definitions.push(definition);
                }
            }
            Err(error) => {
                tracing::warn!(%error,"original projectile particle definitions unavailable")
            }
        }
        let textures = catalog.load_textures(base)?;
        tracing::info!(
            effects = catalog.effects.len(),
            emitters = catalog.emitters.len(),
            textures = textures.textures.len(),
            missing_textures = textures.missing.len(),
            projectile_models = projectile_models.len(),
            "original spell effects loaded"
        );
        Ok(Self {
            catalog,
            textures,
            projectile_definitions,
            projectile_models,
        })
    }
    fn effect(&self, cue: &Cue) -> Option<&openeq_assets::spell_effects::EffectDefinition> {
        if matches!(cue.owner, Owner::Projectile(_)) {
            self.projectile_definitions.get(cue.effect_id as usize)
        } else {
            self.catalog.effect(cue.effect_id)
        }
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct EffectAnchor {
    pub position: [f32; 3],
    /// Scene bearing in EQ's 512 units per turn.
    pub heading: f32,
    pub size: f32,
    pub sockets: ActorSockets,
}
impl EffectAnchor {
    fn point(&self, attachment: u32) -> Vec3 {
        // Authored paired hand emitters use 4/5. Remaining attachment codes
        // retain a body fallback until their original enum is established.
        Vec3::from(match attachment {
            4 => self.sockets.right_hand.unwrap_or(self.position),
            5 => self.sockets.left_hand.unwrap_or(self.position),
            _ => self.position,
        })
    }
    fn rotation(&self) -> Quat {
        Quat::from_rotation_z(
            std::f32::consts::FRAC_PI_2 - self.heading * std::f32::consts::TAU / 512.,
        )
    }
    fn emitter_rotation(&self, attachment: u32, emitter: &EmitterDefinition) -> Quat {
        if emitter.use_attachment_basis {
            let bone = match attachment {
                4 => self.sockets.right_hand_rotation,
                5 => self.sockets.left_hand_rotation,
                _ => None,
            };
            if let Some(rotation) = bone {
                return Quat::from_array(rotation);
            }
        }
        self.rotation()
    }
}

#[derive(Default, Debug, Clone, Copy)]
pub struct EffectStats {
    pub active_effects: usize,
    pub particles: usize,
    pub missing_definitions: u64,
    pub dropped_effects: u64,
    pub emitted_particles: u64,
    pub cast_particles: usize,
    pub impact_particles: usize,
    pub travel_particles: usize,
}

#[derive(Default)]
pub struct SpellEffects {
    assets: Option<Arc<EffectAssets>>,
    timeline: Timeline,
    clocks: BTreeMap<(u64, usize), f32>,
    particles: Vec<Particle>,
    stats: EffectStats,
    last_frame: Option<Instant>,
}
struct Particle {
    cue: u64,
    emitter: u32,
    texture: u32,
    born: Instant,
    lifetime: f32,
    origin: Vec3,
    rotation: Quat,
    offset: Vec3,
    velocity: Vec3,
    acceleration: Vec3,
    world_acceleration: Vec3,
    orbit: f32,
    orbit_acceleration: f32,
    size: [f32; 2],
    scale: f32,
    roll: f32,
    spin: f32,
    attachment: u32,
    entity: u32,
    attached: bool,
}
impl SpellEffects {
    pub fn set_assets(&mut self, assets: EffectAssets) {
        self.assets = Some(Arc::new(assets));
    }
    pub fn stats(&self) -> EffectStats {
        self.stats
    }
    pub fn clear(&mut self) {
        self.timeline.clear();
        self.clocks.clear();
        self.particles.clear();
        self.last_frame = None;
        self.stats.active_effects = 0;
        self.stats.particles = 0;
        self.stats.cast_particles = 0;
        self.stats.impact_particles = 0;
        self.stats.travel_particles = 0;
    }
    pub fn remove_entity(&mut self, id: u32) {
        self.timeline.remove_entity(id);
        self.prune_cancelled();
    }
    fn prune_cancelled(&mut self) {
        let tokens: BTreeSet<_> = self.timeline.cues.iter().map(|cue| cue.token).collect();
        self.particles
            .retain(|particle| tokens.contains(&particle.cue));
        self.clocks.retain(|(token, _), _| tokens.contains(token));
    }
    pub fn event(
        &mut self,
        event: &GameplayEvent,
        spells: &crate::spells::SpellCatalog,
        own: Option<u32>,
        now: Instant,
    ) {
        self.timeline.event(event, spells, own, now);
        if let GameplayEvent::Projectile(projectile) = event
            && self
                .timeline
                .flights
                .last()
                .is_some_and(|flight| flight.started == now && flight.packet == *projectile)
            && let Some(effect_id) = self
                .assets
                .as_ref()
                .and_then(|assets| {
                    assets
                        .projectile_models
                        .get(&projectile.model_name.to_ascii_uppercase())
                })
                .copied()
        {
            self.timeline.attach_projectile(effect_id);
        }
        self.prune_cancelled();
    }
    pub fn projectiles(
        &mut self,
        now: Instant,
        anchors: &BTreeMap<u32, EffectAnchor>,
    ) -> Vec<openeq_render::projectiles::ProjectileState> {
        let mut result = Vec::new();
        self.timeline.flights.retain(|flight| {
            let Some(target) = anchors.get(&flight.packet.target_id) else {
                return false;
            };
            let age = now.saturating_duration_since(flight.started).as_secs_f32();
            let (position, direction, t) = projectile_sample(&flight.packet, age, target.position);
            if age >= 10. {
                return false;
            }
            // Keep the cast identity briefly after visual arrival so a later
            // authoritative impact cannot accidentally cancel a new cast.
            if t >= 1. {
                return true;
            }
            result.push(openeq_render::projectiles::ProjectileState {
                id: flight.token,
                model_name: flight.packet.model_name.clone(),
                position,
                direction,
            });
            true
        });
        result
    }
    pub fn frame(&mut self, now: Instant, anchors: &BTreeMap<u32, EffectAnchor>) -> ParticleFrame {
        if self.last_frame.is_some_and(|last| now < last) {
            self.clear();
        }
        self.last_frame = Some(now);
        let Some(assets) = self.assets.clone() else {
            // Probes may not load graphics. Bound their unrendered event queue.
            self.timeline
                .cues
                .retain(|cue| now.saturating_duration_since(cue.started) < Duration::from_secs(60));
            return ParticleFrame::default();
        };
        let mut frame = ParticleFrame {
            textures: assets.textures.textures.clone(),
            instances: Vec::new(),
        };
        self.particles
            .retain(|p| now.saturating_duration_since(p.born).as_secs_f32() < p.lifetime);
        for cue in &mut self.timeline.cues {
            if let Some(projectile) = &cue.travel
                && cue.stopped.is_none()
                && let Some(target) = anchors.get(&cue.target)
                && projectile_sample(
                    projectile,
                    now.saturating_duration_since(cue.started).as_secs_f32(),
                    target.position,
                )
                .2 >= 1.
            {
                cue.stopped = Some(now);
            }
        }
        self.timeline.cues.retain(|cue| {
            let Some(effect) = assets.effect(cue) else {
                self.stats.missing_definitions += 1;
                return false;
            };
            let phase = phase_index(cue.phase);
            let age = now.saturating_duration_since(cue.started).as_secs_f32();
            effect.stages[phase].emitters.iter().any(|reference| {
                assets
                    .catalog
                    .emitter(reference.emitter_id)
                    .is_some_and(|emitter| {
                        age < emission_end(cue, emitter) + particle_lifetime(cue, emitter)
                    })
            })
        });
        self.prune_cancelled();
        for cue in &self.timeline.cues {
            if now < cue.started {
                continue;
            }
            let Some(effect) = assets.effect(cue) else {
                continue;
            };
            let Some(anchor) = cue_anchor(cue, now, anchors) else {
                continue;
            };
            for (slot, reference) in effect.stages[phase_index(cue.phase)]
                .emitters
                .iter()
                .enumerate()
            {
                let Some(emitter) = assets.catalog.emitter(reference.emitter_id) else {
                    continue;
                };
                let Some(texture) = assets.textures.index(&emitter.texture) else {
                    continue;
                };
                let age = now.saturating_duration_since(cue.started).as_secs_f32();
                let stop = emission_end(cue, emitter);
                let end = age.min(stop);
                let previous = self
                    .clocks
                    .entry((cue.token, slot))
                    .or_insert(-f32::EPSILON);
                let life = particle_lifetime(cue, emitter);
                let delay = emitter.emission_delay.max(0.);
                // No frame catch-up storm after loading or a suspended window.
                // Reconstruct only still-visible births from the last quarter second.
                let start = (*previous).max(age - life).max(age - 0.25);
                let mut births = Vec::new();
                if *previous < delay && delay <= end && delay >= age - life {
                    births.push((delay, emitter.initial_particles.min(128), 0u64));
                }
                let rate = emitter.emission_rate.clamp(0., 120.);
                if rate > 0. && end >= delay {
                    let first = (((start - delay).max(0.) * rate).floor() as u64).saturating_add(1);
                    let last = (((end - delay).max(0.) * rate).floor() as u64)
                        .min(first.saturating_add(120));
                    for tick in first..=last {
                        births.push((
                            delay + tick as f32 / rate,
                            emitter.particles_per_emission.min(128),
                            tick,
                        ));
                    }
                }
                *previous = age;
                for (at, count, tick) in births {
                    for ordinal in 0..count {
                        if self.particles.len() >= MAX_PARTICLES {
                            break;
                        }
                        let seed = cue.token.wrapping_mul(0x9e3779b97f4a7c15)
                            ^ (slot as u64 * 0x1000001)
                            ^ tick.wrapping_mul(193)
                            ^ u64::from(ordinal);
                        let mut rng = Rng(seed);
                        let particle = make_particle(
                            cue,
                            reference,
                            emitter,
                            texture,
                            &anchor,
                            (
                                cue.started + Duration::from_secs_f32(at.max(0.)),
                                [ordinal, count],
                            ),
                            &mut rng,
                        );
                        self.particles.push(particle);
                        self.stats.emitted_particles += 1;
                    }
                }
            }
        }
        self.stats.cast_particles = 0;
        self.stats.impact_particles = 0;
        self.stats.travel_particles = 0;
        let phases: BTreeMap<_, _> = self
            .timeline
            .cues
            .iter()
            .map(|cue| (cue.token, cue.phase))
            .collect();
        for particle in &self.particles {
            let Some(emitter) = assets.catalog.emitter(particle.emitter) else {
                continue;
            };
            let age = now.saturating_duration_since(particle.born).as_secs_f32();
            let (origin, rotation) = if particle.attached {
                anchors.get(&particle.entity).map_or(
                    (particle.origin, particle.rotation),
                    |anchor| {
                        (
                            anchor.point(particle.attachment),
                            anchor.emitter_rotation(particle.attachment, emitter),
                        )
                    },
                )
            } else {
                (particle.origin, particle.rotation)
            };
            let local =
                particle.offset + particle.velocity * age + 0.5 * particle.acceleration * age * age;
            let local = Quat::from_rotation_x(
                particle.orbit * age + 0.5 * particle.orbit_acceleration * age * age,
            ) * local;
            let local = Vec3::from(emitter.offset) * particle.scale
                + Vec3::from(emitter.orient_local_vector(local.to_array()));
            let local = Vec3::new(local.y, -local.z, local.x);
            let mut color = emitter.color_rgba_with_lifetime(age, particle.lifetime);
            for component in &mut color[..3] {
                *component = srgb_to_linear(*component);
            }
            let size = emitter.size_envelope_with_lifetime(age, particle.lifetime);
            if color[3] <= 0. || size <= 0. {
                continue;
            }
            if let Some(phase) = phases.get(&particle.cue) {
                match phase {
                    Phase::Cast => self.stats.cast_particles += 1,
                    Phase::Impact => self.stats.impact_particles += 1,
                    Phase::Travel => self.stats.travel_particles += 1,
                }
            }
            frame.instances.push(ParticleInstance {
                position: (origin
                    + rotation * local
                    + 0.5 * particle.world_acceleration * age * age)
                    .to_array(),
                size: particle.size.map(|v| v * size),
                rotation: particle.roll + particle.spin * age,
                color,
                texture: particle.texture,
                uv_rect: emitter.uv_rect(age),
                blend: if emitter.additive {
                    ParticleBlend::Additive
                } else {
                    ParticleBlend::Alpha
                },
            });
        }
        self.stats.active_effects = self.timeline.cues.len();
        self.stats.particles = frame.instances.len();
        self.stats.dropped_effects = self.timeline.dropped;
        frame
    }
}
fn phase_index(phase: Phase) -> usize {
    match phase {
        Phase::Cast => 0,
        Phase::Travel => 1,
        Phase::Impact => 2,
    }
}
fn emission_end(cue: &Cue, emitter: &EmitterDefinition) -> f32 {
    let mut end = match cue.duration {
        Some(duration) => duration.as_secs_f32(),
        None => emitter.emitter_lifetime_raw.abs().clamp(0., 60.),
    };
    if emitter.emitter_lifetime_raw < 0.
        && !matches!(
            cue.owner,
            Owner::Buff(..) | Owner::Nimbus(..) | Owner::Projectile(_)
        )
    {
        end = emitter.emitter_lifetime_raw.abs().min(60.);
        if matches!(cue.owner, Owner::Cast(_)) {
            end = end.min(cue.duration.unwrap_or_default().as_secs_f32());
        }
    }
    if let Some(stop) = cue.stopped {
        end = end.min(stop.saturating_duration_since(cue.started).as_secs_f32());
    }
    end
}
fn particle_lifetime(cue: &Cue, emitter: &EmitterDefinition) -> f32 {
    let lifetime = if emitter.lifetime > 0. {
        emitter.lifetime
    } else {
        emission_end(cue, emitter)
    };
    lifetime.clamp(0.01, 30.)
}
fn cue_anchor(
    cue: &Cue,
    now: Instant,
    anchors: &BTreeMap<u32, EffectAnchor>,
) -> Option<EffectAnchor> {
    let mut anchor = *anchors.get(&if cue.phase == Phase::Cast {
        cue.source
    } else {
        cue.target
    })?;
    if let Some(projectile) = &cue.travel {
        let age = now.saturating_duration_since(cue.started).as_secs_f32();
        let (position, direction, t) = projectile_sample(projectile, age, anchor.position);
        if t >= 1. {
            return None;
        }
        anchor.position = position;
        anchor.heading = direction[0].atan2(direction[1]) * 512. / std::f32::consts::TAU;
        anchor.size = 6.;
        anchor.sockets = ActorSockets::default();
    }
    Some(anchor)
}
fn projectile_sample(
    projectile: &openeq_net::gameplay::Projectile,
    age: f32,
    target: [f32; 3],
) -> ([f32; 3], [f32; 3], f32) {
    let origin = Vec3::from(projectile.position);
    let destination = Vec3::from(target);
    let distance = origin.distance(destination);
    // Same bounded timing estimate as EQEmu; authoritative Action remains the
    // only impact notification. LOS misses simply expire without an impact.
    let speed = projectile.velocity.clamp(0.1, 100.);
    let travel_distance = if distance <= 125. {
        distance * (1. + (speed - 4.) * -0.2).max(0.1)
    } else if distance <= 200. {
        std::f32::consts::PI * distance * 0.5
    } else {
        std::f32::consts::PI * distance * 0.65
    };
    let seconds = (1.2 + travel_distance / (100. * speed)).clamp(0.1, 10.);
    let t = (age / seconds).clamp(0., 1.);
    let height = distance * projectile.arc.to_radians().sin() * 0.25;
    let mut position = origin.lerp(destination, t);
    position.z += (t * std::f32::consts::PI).sin() * height;
    let tangent = destination - origin
        + Vec3::Z * (t * std::f32::consts::PI).cos() * height * std::f32::consts::PI;
    (
        position.to_array(),
        tangent.normalize_or_zero().to_array(),
        t,
    )
}
fn make_particle(
    cue: &Cue,
    reference: &EmitterReference,
    emitter: &EmitterDefinition,
    texture: u32,
    anchor: &EffectAnchor,
    birth: (Instant, [u32; 2]),
    rng: &mut Rng,
) -> Particle {
    let (born, distribution) = birth;
    let spread = sample_shape(emitter.shape, emitter.shape_dimensions, distribution, rng);

    let mut velocity = Vec3::from_array(
        emitter
            .velocity_ranges
            .map(|range| rng.between(range[0], range[1])),
    );
    velocity += Vec3::new(0., spread.y, spread.z).normalize_or_zero()
        * rng.between(emitter.radial_velocity[0], emitter.radial_velocity[1]);
    let mut acceleration = Vec3::from(emitter.acceleration);
    acceleration +=
        Vec3::new(0., spread.y, spread.z).normalize_or_zero() * emitter.radial_acceleration;

    let actor_scale = if anchor.size > 0. {
        (anchor.size / 6.).clamp(0.2, 5.)
    } else {
        1.
    };
    let scale = if emitter.scale_emitter_basis {
        actor_scale
    } else {
        1.
    };
    let size_scale = if emitter.scale_particle_size {
        actor_scale
    } else {
        1.
    };
    let u = rng.between(0., 1.);
    let factors = [
        u,
        if emitter.correlated_size {
            u
        } else {
            rng.between(0., 1.)
        },
    ];
    let size = std::array::from_fn(|axis| {
        let [a, b] = emitter.size_ranges[axis];
        (a + (b - a) * factors[axis]).abs().clamp(0.01, 100.) * size_scale
    });
    Particle {
        cue: cue.token,
        emitter: reference.emitter_id,
        texture,
        born,
        lifetime: particle_lifetime(cue, emitter),
        origin: anchor.point(reference.attachment),
        rotation: anchor.emitter_rotation(reference.attachment, emitter),
        offset: spread * scale,
        velocity: velocity * scale,
        acceleration: acceleration * scale,
        world_acceleration: Vec3::new(emitter.wind, 0., -emitter.gravity),
        orbit: rng.between(emitter.orbit_velocity[0], emitter.orbit_velocity[1])
            * std::f32::consts::TAU
            / 512.,
        orbit_acceleration: emitter.orbit_acceleration * std::f32::consts::TAU / 512.,
        size,
        scale,
        roll: if emitter.random_rotation {
            rng.between(0., std::f32::consts::TAU)
        } else {
            0.
        },
        spin: rng.between(emitter.spin[0], emitter.spin[1]) * std::f32::consts::TAU / 512.,
        attachment: reference.attachment,
        entity: if cue.phase == Phase::Cast {
            cue.source
        } else {
            cue.target
        },
        attached: emitter.follow_attachment && cue.travel.is_none(),
    }
}
fn srgb_to_linear(value: f32) -> f32 {
    let value = value.clamp(0., 1.);
    if value <= 0.04045 {
        value / 12.92
    } else {
        ((value + 0.055) / 1.055).powf(2.4)
    }
}
struct Rng(u64);
impl Rng {
    fn between(&mut self, a: f32, b: f32) -> f32 {
        self.0 = self.0.wrapping_add(0x9e3779b97f4a7c15);
        let mut n = self.0;
        n = (n ^ (n >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
        n = (n ^ (n >> 27)).wrapping_mul(0x94d049bb133111eb);
        let f = ((n ^ (n >> 31)) >> 40) as f32 / (1u32 << 24) as f32;
        a + (b - a) * f
    }
}

/// Original EDD shape order: axial, transverse X, transverse -Y. Dimensions
/// are radii/half-extents, except cylinder/cone axial length. Regular sphere
/// rings use an even spherical distribution until native ring stepping is known.
fn sample_shape(shape: u32, dimensions: [f32; 3], distribution: [u32; 2], rng: &mut Rng) -> Vec3 {
    let [r, alternate, length] = dimensions.map(f32::abs);
    let r2 = if alternate == 0. { r } else { alternate };
    let axial = if length == 0. { r } else { length };
    let angle = if shape == 1 {
        std::f32::consts::TAU * distribution[0] as f32 / distribution[1].max(1) as f32
    } else {
        rng.between(0., std::f32::consts::TAU)
    };
    let (sin, cos) = angle.sin_cos();
    match shape {
        1 | 9 => Vec3::new(0., r * cos, r2 * sin),
        2 | 5 => {
            let z = if shape == 2 {
                1. - 2. * (distribution[0] as f32 + 0.5) / distribution[1].max(1) as f32
            } else {
                rng.between(-1., 1.)
            };
            let radial = (1. - z * z).max(0.).sqrt();
            Vec3::new(z * axial, radial * r * cos, radial * r2 * sin)
        }
        3 => Vec3::new(rng.between(-0.5, 0.5) * length, r * cos, r * sin),
        4 => {
            let radial = rng.between(0., 1.).sqrt();
            Vec3::new(0., radial * r * cos, radial * r2 * sin)
        }
        6 => {
            let extents = [r, r2, axial];
            let face = rng.between(0., 6.) as usize;
            let mut values = extents.map(|v| rng.between(-v, v));
            values[face / 2] = extents[face / 2] * if face.is_multiple_of(2) { 1. } else { -1. };
            Vec3::from(values)
        }
        7 => {
            let t = rng.between(0., 1.);
            Vec3::new(t * length, t * r * cos, t * r * sin)
        }
        8 => {
            let minor = rng.between(0., std::f32::consts::TAU);
            Vec3::new(
                length * minor.sin(),
                (r + length * minor.cos()) * cos,
                (r2 + length * minor.cos()) * sin,
            )
        }
        _ => Vec3::ZERO,
    }
}

#[cfg(test)]
mod simulation_tests {
    use super::*;
    fn emitter() -> EmitterDefinition {
        let mut data = b"EDD\0".to_vec();
        data.extend_from_slice(b"110\0");
        data.resize(8 + 416 * 2, 0);
        let base = 8 + 416;
        for (offset, value) in [(120, 2f32), (124, 2.), (164, 1.)] {
            data[base + offset..base + offset + 4].copy_from_slice(&value.to_le_bytes());
        }
        SpellEffectCatalog::parse(&[0; 268], &data)
            .unwrap()
            .emitters[1]
            .clone()
    }
    fn cue(now: Instant) -> Cue {
        Cue {
            token: 1,
            owner: Owner::Explicit,
            effect_id: 1,
            spell_id: None,
            source: 1,
            target: 1,
            phase: Phase::Impact,
            started: now,
            duration: Some(Duration::from_secs(1)),
            stopped: None,
            travel: None,
        }
    }
    #[test]
    fn negative_authored_duration_overrides_request_but_cancellation_wins() {
        let mut emitter = emitter();
        emitter.emitter_lifetime_raw = -7.;
        let now = Instant::now();
        let mut cue = cue(now);
        assert_eq!(emission_end(&cue, &emitter), 7.);
        cue.stopped = Some(now + Duration::from_millis(300));
        assert!((emission_end(&cue, &emitter) - 0.3).abs() < 1e-6);
    }
    #[test]
    fn correlated_sizes_share_random_factor_without_losing_aspect_ratio() {
        let mut emitter = emitter();
        emitter.size_ranges = [[3., 3.], [8., 8.]];
        emitter.correlated_size = true;
        let now = Instant::now();
        let cue = cue(now);
        let anchor = EffectAnchor {
            size: 30.,
            ..Default::default()
        };
        let particle = make_particle(
            &cue,
            &EmitterReference::default(),
            &emitter,
            0,
            &anchor,
            (now, [0, 1]),
            &mut Rng(1),
        );
        assert_eq!(particle.size, [3., 8.]);
        emitter.size_ranges = [[1., 3.], [5., 9.]];
        let particle = make_particle(
            &cue,
            &EmitterReference::default(),
            &emitter,
            0,
            &anchor,
            (now, [0, 1]),
            &mut Rng(4),
        );
        assert!(((particle.size[0] - 1.) / 2. - (particle.size[1] - 5.) / 4.).abs() < 1e-6);
    }
    #[test]
    fn source_sphere_ring_disk_and_box_samples_stay_on_their_authored_surfaces() {
        let mut rng = Rng(19);
        for i in 0..1000 {
            let ring = sample_shape(9, [3., 2., 0.], [i, 1000], &mut rng);
            assert_eq!(ring.x, 0.);
            assert!(((ring.y / 3.).powi(2) + (ring.z / 2.).powi(2) - 1.).abs() < 1e-5);
            let sphere = sample_shape(5, [3., 3., 3.], [i, 1000], &mut rng);
            assert!((sphere.length() - 3.).abs() < 1e-5);
            let disk = sample_shape(4, [3., 2., 0.], [i, 1000], &mut rng);
            assert_eq!(disk.x, 0.);
            assert!((disk.y / 3.).powi(2) + (disk.z / 2.).powi(2) <= 1.00001);
            let cube = sample_shape(6, [2., 2., 2.], [i, 1000], &mut rng);
            assert_eq!(cube.abs().max_element(), 2.);
        }
    }
    #[test]
    fn a_projectile_timeout_never_invents_an_impact() {
        let now = Instant::now();
        let mut effects = SpellEffects::default();
        effects.event(
            &GameplayEvent::Projectile(openeq_net::gameplay::Projectile {
                source_id: 1,
                target_id: 2,
                position: [0.; 3],
                velocity: 4.,
                launch_angle: 0.,
                tilt: 0.,
                arc: 0.,
                item_id: 0,
                skill: 0,
                item_type: 0,
                model_name: "IT10".into(),
            }),
            &crate::spells::SpellCatalog::default(),
            Some(1),
            now,
        );
        let anchors = BTreeMap::from([(
            2,
            EffectAnchor {
                position: [0., 100., 0.],
                size: 6.,
                ..Default::default()
            },
        )]);
        assert_eq!(
            effects
                .projectiles(now + Duration::from_millis(500), &anchors)
                .len(),
            1
        );
        assert!(
            effects
                .projectiles(now + Duration::from_secs(11), &anchors)
                .is_empty()
        );
        assert!(effects.timeline.cues.is_empty());
    }
}
