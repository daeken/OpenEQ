//! Server-driven visual lifetimes. No visual changes resources or spell outcomes.
use crate::spells::SpellCatalog;
use openeq_net::gameplay::{Buff, GameplayEvent, Projectile};
use std::{
    collections::BTreeMap,
    time::{Duration, Instant},
};

pub const MAX_EFFECTS: usize = 256;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    Cast,
    Travel,
    Impact,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Owner {
    Cast(u32),
    Transient,
    Explicit,
    Projectile(u64),
    Buff(u32, u32),
    Nimbus(u32, u32),
}
#[derive(Clone, Debug)]
pub struct Cue {
    pub token: u64,
    pub owner: Owner,
    pub effect_id: u32,
    pub spell_id: Option<u32>,
    pub source: u32,
    pub target: u32,
    pub phase: Phase,
    pub started: Instant,
    /// None selects the emitter's authored emission period, not infinity.
    pub duration: Option<Duration>,
    pub stopped: Option<Instant>,
    pub travel: Option<Projectile>,
}
pub struct Flight {
    pub token: u64,
    pub packet: Projectile,
    pub started: Instant,
    pub spell_id: Option<u32>,
}
#[derive(Default)]
pub struct Timeline {
    pub cues: Vec<Cue>,
    pub flights: Vec<Flight>,
    next: u64,
    // OP_Action goes to observers, then a second success copy reaches caster
    // and target. Only the opposite flag is a duplicate; repeated same-flag
    // actions can be separate procs or instant casts.
    recent: BTreeMap<(u32, u32, u32), (u8, Instant)>,
    pub dropped: u64,
    casts: BTreeMap<u32, (u32, Instant)>,
}
impl Timeline {
    pub fn attach_projectile(&mut self, effect_id: u32) {
        let Some(flight) = self.flights.last() else {
            return;
        };
        if self
            .cues
            .iter()
            .any(|cue| cue.owner == Owner::Projectile(flight.token))
        {
            return;
        }
        self.push(Cue {
            token: 0,
            owner: Owner::Projectile(flight.token),
            effect_id,
            spell_id: flight.spell_id,
            source: flight.packet.source_id,
            target: flight.packet.target_id,
            phase: Phase::Travel,
            started: flight.started,
            duration: Some(Duration::from_secs(10)),
            stopped: None,
            travel: Some(flight.packet.clone()),
        });
    }
    pub fn clear(&mut self) {
        self.cues.clear();
        self.flights.clear();
        self.recent.clear();
        self.casts.clear();
    }
    pub fn remove_entity(&mut self, id: u32) {
        self.cues.retain(|cue| cue.source != id && cue.target != id);
        self.recent
            .retain(|(source, target, _), _| *source != id && *target != id);
        self.casts.remove(&id);
        self.flights
            .retain(|flight| flight.packet.source_id != id && flight.packet.target_id != id);
    }
    fn stop_cast(&mut self, id: u32, spell_id: Option<u32>, now: Instant) {
        for cue in &mut self.cues {
            if cue.owner == Owner::Cast(id) && spell_id.is_none_or(|id| cue.spell_id == Some(id)) {
                cue.stopped = Some(now);
            }
        }
    }
    fn push(&mut self, mut cue: Cue) {
        if self.cues.len() >= MAX_EFFECTS {
            self.dropped += 1;
            return;
        }
        self.next = self.next.wrapping_add(1);
        cue.token = self.next;
        self.cues.push(cue);
    }
    fn buff(&mut self, id: u32, buff: &Buff, removed: bool, spells: &SpellCatalog, now: Instant) {
        let owner = Owner::Buff(id, buff.slot);
        let effect = spells
            .spells
            .get(&buff.spell_id)
            .filter(|spell| spell.persistent_particles)
            .map(|spell| spell.effect_id);
        if removed || effect.is_none() {
            self.cues.retain(|cue| cue.owner != owner);
            return;
        }
        let effect_id = effect.unwrap();
        // Tick updates refresh the deadline without restarting the emitter.
        let duration =
            Duration::from_secs(u64::from(buff.ticks_remaining).saturating_mul(6).max(6));
        if let Some(cue) = self
            .cues
            .iter_mut()
            .find(|cue| cue.owner == owner && cue.effect_id == effect_id)
        {
            cue.duration = Some(now.saturating_duration_since(cue.started) + duration);
            return;
        }
        self.cues.retain(|cue| cue.owner != owner);
        self.push(Cue {
            token: 0,
            owner,
            effect_id,
            spell_id: Some(buff.spell_id),
            source: id,
            target: id,
            phase: Phase::Impact,
            started: now,
            duration: Some(duration),
            stopped: None,
            travel: None,
        });
    }
    pub fn event(
        &mut self,
        event: &GameplayEvent,
        spells: &SpellCatalog,
        own: Option<u32>,
        now: Instant,
    ) {
        self.recent
            .retain(|_, (_, when)| now.saturating_duration_since(*when) < Duration::from_secs(2));
        self.casts
            .retain(|_, (_, when)| now.saturating_duration_since(*when) < Duration::from_secs(60));
        match event {
            GameplayEvent::BeginCast {
                caster_id,
                spell_id,
                cast_time_ms,
            } => {
                self.stop_cast(*caster_id, None, now);
                if self.casts.len() >= MAX_EFFECTS {
                    self.casts.clear();
                }
                self.casts.insert(*caster_id, (*spell_id, now));
                self.recent.retain(|(source, _, _), _| source != caster_id);
                if let Some(spell) = spells.spells.get(spell_id) {
                    self.push(Cue {
                        token: 0,
                        owner: Owner::Cast(*caster_id),
                        effect_id: spell.effect_id,
                        spell_id: Some(*spell_id),
                        source: *caster_id,
                        target: *caster_id,
                        phase: Phase::Cast,
                        started: now,
                        duration: Some(Duration::from_millis(
                            u64::from(*cast_time_ms).min(600_000),
                        )),
                        stopped: None,
                        travel: None,
                    });
                }
            }
            GameplayEvent::CastInterrupted { id, .. } => {
                if let Some(id) = if *id == 0 { own } else { Some(*id) } {
                    // Interrupted casts remove the emission and existing sparks.
                    self.cues.retain(|cue| cue.owner != Owner::Cast(id));
                    self.casts.remove(&id);
                }
            }
            GameplayEvent::SpellBarEnabled {
                keep_casting: false,
                spell_id,
                ..
            } => {
                if let Some(id) = own {
                    self.stop_cast(id, Some(*spell_id), now);
                }
            }
            GameplayEvent::SpellAction {
                source_id,
                target_id,
                spell_id,
                action_type,
                effect_flag,
                ..
            } if *action_type == 231 => {
                let key = (*source_id, *target_id, *spell_id);
                if self.recent.get(&key).is_some_and(|(flag, when)| {
                    flag != effect_flag
                        && now.saturating_duration_since(*when) < Duration::from_millis(500)
                }) {
                    self.recent.remove(&key);
                    return;
                }
                if self.recent.len() >= MAX_EFFECTS {
                    self.recent.clear();
                }
                self.recent.insert(key, (*effect_flag, now));
                let arriving_bolt = self.flights.iter().position(|flight| {
                    flight.packet.source_id == *source_id
                        && flight.packet.target_id == *target_id
                        && flight.spell_id == Some(*spell_id)
                });
                if let Some(index) = arriving_bolt {
                    let flight = self.flights.remove(index);
                    self.cues.retain(|cue| {
                        !(cue.owner == Owner::Projectile(flight.token)
                            || (cue.phase == Phase::Travel
                                && cue.started == flight.started
                                && cue.source == *source_id
                                && cue.target == *target_id
                                && cue.spell_id == Some(*spell_id)))
                    });
                } else {
                    self.stop_cast(*source_id, Some(*spell_id), now);
                }
                if let Some(spell) = spells.spells.get(spell_id) {
                    self.push(Cue {
                        token: 0,
                        owner: Owner::Transient,
                        effect_id: spell.effect_id,
                        spell_id: Some(*spell_id),
                        source: *source_id,
                        target: *target_id,
                        phase: Phase::Impact,
                        started: now,
                        duration: None,
                        stopped: None,
                        travel: None,
                    });
                }
            }
            GameplayEvent::Projectile(projectile) => {
                let spell = self
                    .casts
                    .get(&projectile.source_id)
                    .and_then(|(id, _)| spells.spells.get(id))
                    .filter(|spell| {
                        spell.target_type == 1
                            && !spell.projectile_model.is_empty()
                            && spell
                                .projectile_model
                                .eq_ignore_ascii_case(&projectile.model_name)
                    });
                if self.flights.len() < 64 {
                    self.next = self.next.wrapping_add(1);
                    self.flights.push(Flight {
                        token: self.next,
                        packet: projectile.clone(),
                        started: now,
                        spell_id: spell.map(|spell| spell.id),
                    });
                }
                if let Some(spell) = spell {
                    self.stop_cast(projectile.source_id, Some(spell.id), now);
                    self.push(Cue {
                        token: 0,
                        owner: Owner::Transient,
                        effect_id: spell.effect_id,
                        spell_id: Some(spell.id),
                        source: projectile.source_id,
                        target: projectile.target_id,
                        phase: Phase::Travel,
                        started: now,
                        duration: Some(Duration::from_secs(10)),
                        stopped: None,
                        travel: Some(projectile.clone()),
                    });
                }
            }
            GameplayEvent::SpellEffect(effect) => self.push(Cue {
                token: 0,
                owner: Owner::Explicit,
                effect_id: effect.effect_id,
                spell_id: None,
                source: effect.source_id,
                target: effect.target_id,
                phase: Phase::Impact,
                started: now,
                duration: Some(Duration::from_millis(
                    u64::from(effect.duration_ms).min(60_000),
                )),
                stopped: None,
                travel: None,
            }),
            GameplayEvent::NimbusEffect(effect) => {
                self.cues.retain(|cue| {
                    cue.owner != Owner::Nimbus(effect.id, effect.effect_id)
                        && !(effect.removed
                            && cue.target == effect.id
                            && cue.effect_id == effect.effect_id
                            && cue.owner == Owner::Explicit)
                });
                if !effect.removed {
                    self.push(Cue {
                        token: 0,
                        owner: Owner::Nimbus(effect.id, effect.effect_id),
                        effect_id: effect.effect_id,
                        spell_id: None,
                        source: effect.id,
                        target: effect.id,
                        phase: Phase::Impact,
                        started: now,
                        duration: Some(Duration::MAX),
                        stopped: None,
                        travel: None,
                    });
                }
            }
            GameplayEvent::BuffChanged { id, buff, removed } => {
                self.buff(*id, buff, *removed, spells, now)
            }
            GameplayEvent::Buffs { id, all, buffs, .. } => {
                if *all {
                    self.cues.retain(|cue| !matches!(cue.owner,Owner::Buff(owner,slot) if owner == *id && !buffs.iter().any(|b| b.slot == slot)));
                }
                for buff in buffs {
                    self.buff(*id, buff, false, spells, now);
                }
            }
            GameplayEvent::Profile(profile) => {
                if let Some(id) = own {
                    for buff in &profile.buffs {
                        self.buff(id, buff, false, spells, now);
                    }
                }
            }
            GameplayEvent::Death(death) => self.remove_entity(death.id),
            GameplayEvent::ZoneTransition { .. } => self.clear(),
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn catalog() -> SpellCatalog {
        let mut fields = vec!["0"; 220];
        fields[0] = "200";
        fields[1] = "Heal";
        fields[145] = "278";
        fields[153] = "1";
        SpellCatalog::parse(&fields.join("^"))
    }
    fn action(flag: u8) -> GameplayEvent {
        GameplayEvent::SpellAction {
            source_id: 1,
            target_id: 2,
            spell_id: 200,
            level: 1,
            effect_flag: flag,
            action_type: 231,
            spell_level: 1,
            instrument_modifier: 1.,
        }
    }
    #[test]
    fn observers_see_impact_success_copy_is_deduplicated_and_next_cast_restarts() {
        let spells = catalog();
        let now = Instant::now();
        let mut state = Timeline::default();
        state.event(&action(0), &spells, Some(1), now);
        state.event(
            &action(4),
            &spells,
            Some(1),
            now + Duration::from_millis(20),
        );
        assert_eq!(state.cues.len(), 1);
        assert_eq!(state.cues[0].phase, Phase::Impact);
        state.event(
            &GameplayEvent::BeginCast {
                caster_id: 1,
                spell_id: 200,
                cast_time_ms: 1000,
            },
            &spells,
            Some(1),
            now,
        );
        state.event(
            &action(4),
            &spells,
            Some(1),
            now + Duration::from_millis(40),
        );
        assert_eq!(
            state
                .cues
                .iter()
                .filter(|cue| cue.owner == Owner::Transient)
                .count(),
            2
        );
        assert!(
            state
                .cues
                .iter()
                .find(|cue| cue.owner == Owner::Cast(1))
                .unwrap()
                .stopped
                .is_some()
        );
    }
    #[test]
    fn interruption_and_despawn_remove_only_related_effects() {
        let spells = catalog();
        let now = Instant::now();
        let mut state = Timeline::default();
        for caster in [1, 3] {
            state.event(
                &GameplayEvent::BeginCast {
                    caster_id: caster,
                    spell_id: 200,
                    cast_time_ms: 1000,
                },
                &spells,
                Some(1),
                now,
            );
        }
        state.event(
            &GameplayEvent::CastInterrupted {
                id: 0,
                string_id: 0,
                message: String::new(),
            },
            &spells,
            Some(1),
            now,
        );
        assert_eq!(state.cues.len(), 1);
        assert_eq!(state.cues[0].source, 3);
        state.remove_entity(3);
        assert!(state.cues.is_empty());
        state.event(&action(0), &spells, Some(1), now);
        state.event(
            &GameplayEvent::ZoneTransition {
                zone_id: 54,
                instance_id: 0,
            },
            &spells,
            Some(1),
            now,
        );
        assert!(state.cues.is_empty());
        assert!(state.recent.is_empty());
    }
    #[test]
    fn buff_ticks_refresh_without_restart_and_removal_cleans_up() {
        let spells = catalog();
        let now = Instant::now();
        let mut state = Timeline::default();
        let buff = Buff {
            slot: 5,
            spell_id: 200,
            ticks_remaining: 10,
            num_hits: 0,
            caster: "Caster".into(),
        };
        state.event(
            &GameplayEvent::BuffChanged {
                id: 1,
                buff: buff.clone(),
                removed: false,
            },
            &spells,
            Some(1),
            now,
        );
        let token = state.cues[0].token;
        state.event(
            &GameplayEvent::BuffChanged {
                id: 1,
                buff: buff.clone(),
                removed: false,
            },
            &spells,
            Some(1),
            now + Duration::from_secs(5),
        );
        assert_eq!(state.cues.len(), 1);
        assert_eq!(state.cues[0].token, token);
        assert_eq!(state.cues[0].started, now);
        state.event(
            &GameplayEvent::BuffChanged {
                id: 1,
                buff,
                removed: true,
            },
            &spells,
            Some(1),
            now,
        );
        assert!(state.cues.is_empty());
    }
    #[test]
    fn unknown_spells_do_not_invent_effects_and_packet_flood_is_bounded() {
        let now = Instant::now();
        let mut state = Timeline::default();
        state.event(&action(0), &SpellCatalog::default(), Some(1), now);
        assert!(state.cues.is_empty());
        let spells = catalog();
        for _ in 0..1000 {
            state.event(&action(0), &spells, Some(1), now);
        }
        assert_eq!(state.cues.len(), MAX_EFFECTS);
        assert!(state.dropped > 0);
    }
}

#[cfg(test)]
mod flight_tests {
    use super::*;
    fn catalog() -> SpellCatalog {
        let mut fields = vec!["0"; 220];
        fields[0] = "200";
        fields[1] = "Bolt";
        fields[2] = "IT11504";
        fields[98] = "1";
        fields[145] = "198";
        SpellCatalog::parse(&fields.join("^"))
    }
    fn bolt() -> GameplayEvent {
        GameplayEvent::Projectile(Projectile {
            source_id: 1,
            target_id: 2,
            position: [0.; 3],
            velocity: 1.,
            launch_angle: 0.,
            tilt: 0.,
            arc: 0.,
            item_id: 0,
            skill: 0,
            item_type: 0,
            model_name: "IT11504".into(),
        })
    }
    fn cast() -> GameplayEvent {
        GameplayEvent::BeginCast {
            caster_id: 1,
            spell_id: 200,
            cast_time_ms: 2500,
        }
    }
    fn impact(flag: u8) -> GameplayEvent {
        GameplayEvent::SpellAction {
            source_id: 1,
            target_id: 2,
            spell_id: 200,
            level: 20,
            action_type: 231,
            spell_level: 20,
            instrument_modifier: 1.,
            effect_flag: flag,
        }
    }
    #[test]
    fn a_landed_bolt_and_its_duplicate_do_not_cancel_the_next_cast() {
        let now = Instant::now();
        let spells = catalog();
        let mut timeline = Timeline::default();
        timeline.event(&cast(), &spells, Some(1), now);
        timeline.event(&bolt(), &spells, Some(1), now + Duration::from_secs(3));
        timeline.event(&cast(), &spells, Some(1), now + Duration::from_secs(4));
        let token = timeline.cues.last().unwrap().token;
        for flag in [0, 4] {
            timeline.event(
                &impact(flag),
                &spells,
                Some(1),
                now + Duration::from_secs(5),
            );
        }
        assert!(timeline.flights.is_empty());
        assert!(
            timeline
                .cues
                .iter()
                .find(|cue| cue.token == token)
                .unwrap()
                .stopped
                .is_none()
        );
        assert_eq!(
            timeline
                .cues
                .iter()
                .filter(|cue| cue.phase == Phase::Impact)
                .count(),
            1
        );
    }
    #[test]
    fn interruption_clears_stale_correlation_but_preserves_launched_projectiles() {
        let now = Instant::now();
        let spells = catalog();
        let mut timeline = Timeline::default();
        timeline.event(&cast(), &spells, Some(1), now);
        timeline.event(&bolt(), &spells, Some(1), now + Duration::from_secs(3));
        timeline.event(
            &GameplayEvent::CastInterrupted {
                id: 1,
                string_id: 0,
                message: String::new(),
            },
            &spells,
            Some(1),
            now + Duration::from_secs(4),
        );
        assert_eq!(timeline.flights.len(), 1);
        timeline.event(&bolt(), &spells, Some(1), now + Duration::from_secs(5));
        assert_eq!(timeline.flights[1].spell_id, None);
        timeline.remove_entity(2);
        assert!(timeline.flights.is_empty());
    }
    #[test]
    fn removing_nimbus_keeps_unrelated_spell_impact_with_same_effect_id() {
        let now = Instant::now();
        let spells = catalog();
        let mut timeline = Timeline::default();
        timeline.event(&impact(0), &spells, Some(1), now);
        for removed in [false, true] {
            timeline.event(
                &GameplayEvent::NimbusEffect(openeq_net::gameplay::NimbusEffect {
                    id: 2,
                    effect_id: 198,
                    removed,
                }),
                &spells,
                Some(1),
                now,
            );
        }
        assert_eq!(timeline.cues.len(), 1);
        assert_eq!(timeline.cues[0].owner, Owner::Transient);
    }
}
