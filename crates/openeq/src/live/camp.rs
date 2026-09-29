//! A camp attempt belongs to one living player and one dispatch, never a UI frame.
use super::*;
use std::sync::atomic::{AtomicU64, Ordering};

pub const COUNTDOWN: std::time::Duration = std::time::Duration::from_secs(30);
static NEXT_TOKEN: AtomicU64 = AtomicU64::new(1);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Request {
    pub token: u64,
    pub epoch: u64,
    pub motion_revision: u64,
    pub interruption_revision: u64,
}

#[derive(Clone, Copy, Debug)]
enum Phase {
    Requested,
    Counting(Instant),
    Finishing,
}

#[derive(Default)]
pub(crate) struct State {
    pub enabled: bool,
    active: Option<(Request, Phase)>,
    authority: Authority,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct View {
    pub token: u64,
    pub seconds: Option<u32>,
    pub cancellable: bool,
}

/// Damage and authoritative posture changes do not change movement/action
/// epochs, but must still retire camp requests queued before they arrived.
#[derive(Default)]
pub(crate) struct Authority {
    revision: u64,
}
impl Authority {
    pub fn observe(&mut self, event: &GameplayEvent, own: Option<u32>) -> Option<bool> {
        let stand = gameplay_interruption(event, own)?;
        self.revision = self.revision.wrapping_add(1);
        Some(stand)
    }
    pub fn permits(&self, request: Request) -> bool {
        request.interruption_revision == self.revision
    }
}

impl State {
    pub fn observe(&mut self, event: &GameplayEvent, own: Option<u32>) {
        if self.authority.observe(event, own).is_some() {
            self.clear();
        }
    }
    pub fn request(&mut self, epoch: u64, motion_revision: u64) -> Option<Request> {
        if !self.enabled || self.active.is_some() {
            return None;
        }
        let request = Request {
            token: NEXT_TOKEN.fetch_add(1, Ordering::Relaxed),
            epoch,
            motion_revision,
            interruption_revision: self.authority.revision,
        };
        self.active = Some((request, Phase::Requested));
        Some(request)
    }

    pub fn started(&mut self, request: Request, deadline: Instant) {
        if let Some((active, phase)) = &mut self.active
            && *active == request
            && matches!(phase, Phase::Requested)
        {
            *phase = Phase::Counting(deadline);
        }
    }

    pub fn finishing(&mut self, request: Request) {
        if let Some((active, phase)) = &mut self.active
            && *active == request
        {
            *phase = Phase::Finishing;
        }
    }

    pub fn cancel_request(&self, token: u64) -> Option<Request> {
        self.active.and_then(|(request, phase)| {
            (request.token == token && !matches!(phase, Phase::Finishing)).then_some(request)
        })
    }

    pub fn retire(&mut self, request: Request) -> bool {
        if self.active.is_some_and(|(active, _)| active == request) {
            self.active = None;
            true
        } else {
            false
        }
    }

    pub fn clear(&mut self) {
        self.active = None;
    }

    pub fn view(&self, now: Instant) -> Option<View> {
        self.active.map(|(request, phase)| View {
            token: request.token,
            seconds: match phase {
                Phase::Counting(deadline) => {
                    Some(deadline.saturating_duration_since(now).as_secs_f64().ceil() as u32)
                }
                _ => None,
            },
            cancellable: !matches!(phase, Phase::Finishing),
        })
    }

    pub fn active(&self) -> bool {
        self.active.is_some()
    }
}

/// Whether this authoritative event interrupts a camp and whether a standing
/// appearance must cancel the server timer while the player is still alive.
#[cfg(test)]
pub(crate) fn interruption(event: &ZoneEvent, own: Option<u32>) -> Option<bool> {
    match event {
        ZoneEvent::Gameplay(event) => gameplay_interruption(event, own),
        _ => None,
    }
}

pub(crate) fn gameplay_interruption(event: &GameplayEvent, own: Option<u32>) -> Option<bool> {
    match event {
        GameplayEvent::Damage(damage) if own == Some(damage.target_id) && damage.amount > 0 => {
            Some(true)
        }
        GameplayEvent::SpawnAppearance {
            id,
            kind: 14,
            parameter,
        } if own == Some(*id) && *parameter != 110 => Some(false),
        GameplayEvent::Death(death) if own == Some(death.id) => Some(false),
        GameplayEvent::ZoneTransition { .. }
        | GameplayEvent::ZoneChangeRequested(_)
        | GameplayEvent::Recovery(
            openeq_net::death::DeathEvent::BindTransfer(_)
            | openeq_net::death::DeathEvent::RespawnWindow(_),
        ) => Some(false),
        _ => None,
    }
}

pub(super) enum Input {
    Event(Result<Box<ZoneEvent>, openeq_net::zone::ZoneError>),
    Request(NetworkCommand),
    MovementClosed(bool),
    Heartbeat,
    Deadline,
}

/// Resolve ready authority and cancellation before committing a camp deadline.
/// Factoring this selection lets tests exercise simultaneous readiness using
/// the same selector as the live worker, without a server or real 30s timer.
pub(super) async fn next_input(
    event: impl std::future::Future<Output = Result<ZoneEvent, openeq_net::zone::ZoneError>>,
    requests: &mut tokio::sync::mpsc::UnboundedReceiver<NetworkCommand>,
    updates: &mut tokio::sync::watch::Receiver<Option<MovementUpdate>>,
    heartbeat: &mut tokio::time::Interval,
    deadline: Option<Instant>,
) -> Input {
    tokio::select! {
        biased;
        event = event => Input::Event(event.map(Box::new)),
        Some(request) = requests.recv() => Input::Request(request),
        changed = updates.changed() => Input::MovementClosed(changed.is_err()),
        _ = heartbeat.tick() => Input::Heartbeat,
        _ = async {
            if let Some(deadline) = deadline {
                tokio::time::sleep_until(tokio::time::Instant::from_std(deadline)).await;
            } else { std::future::pending::<()>().await; }
        } => Input::Deadline,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn countdown_starts_on_dispatch_and_old_callbacks_cannot_change_retry() {
        let now = Instant::now();
        let mut state = State {
            enabled: true,
            ..Default::default()
        };
        let first = state.request(2, 3).unwrap();
        assert!(state.request(2, 3).is_none());
        assert_eq!(state.view(now).unwrap().seconds, None);
        state.started(first, now + COUNTDOWN);
        assert_eq!(state.view(now).unwrap().seconds, Some(30));
        assert_eq!(state.view(now + COUNTDOWN).unwrap().seconds, Some(0));
        assert!(state.retire(first));
        let retry = state.request(2, 3).unwrap();
        assert_ne!(first.token, retry.token);
        state.started(first, now);
        state.finishing(first);
        assert!(!state.retire(first));
        assert_eq!(state.view(now).unwrap().token, retry.token);
        assert_eq!(state.view(now).unwrap().seconds, None);
        state.finishing(retry);
        assert!(state.cancel_request(retry.token).is_none());
        assert!(!state.view(now).unwrap().cancellable);
    }

    #[test]
    fn direct_sessions_never_offer_a_camp_return() {
        assert!(State::default().request(0, 0).is_none());
    }
}

#[cfg(test)]
mod interruption_tests {
    use super::*;
    use openeq_net::{
        death::{BindTransfer, DeathEvent, RespawnWindow},
        gameplay::{Damage, Death},
    };

    fn damage(target_id: u32, amount: i32) -> GameplayEvent {
        GameplayEvent::Damage(Damage {
            target_id,
            source_id: 9,
            amount,
            skill: 1,
            spell_id: 0,
            secondary: false,
            special: 0,
        })
    }
    #[test]
    fn damage_or_standing_before_dispatch_rejects_queued_start_without_resurrection() {
        for event in [
            damage(1, 2),
            GameplayEvent::SpawnAppearance {
                id: 1,
                kind: 14,
                parameter: 100,
            },
        ] {
            let mut state = State {
                enabled: true,
                ..Default::default()
            };
            let mut worker = Authority::default();
            let request = state.request(3, 4).unwrap();
            assert!(worker.permits(request));
            assert!(worker.observe(&event, Some(1)).is_some());
            assert!(
                !worker.permits(request),
                "worker accepted a queued pre-interruption start"
            );
            state.observe(&event, Some(1));
            state.started(request, Instant::now() + COUNTDOWN);
            assert!(state.view(Instant::now()).is_none());
            let retry = state.request(3, 4).unwrap();
            assert!(worker.permits(retry));
            assert!(!state.retire(request));
            assert_eq!(state.view(Instant::now()).unwrap().token, retry.token);
        }
    }
    #[tokio::test]
    async fn ready_authority_and_cancel_win_over_elapsed_deadline() {
        let (sender, mut requests) = tokio::sync::mpsc::unbounded_channel();
        let (_poses, mut updates) = tokio::sync::watch::channel(None);
        let mut heartbeat = tokio::time::interval(std::time::Duration::from_secs(60));
        heartbeat.tick().await;
        let mut state = State {
            enabled: true,
            ..Default::default()
        };
        let request = state.request(0, 0).unwrap();
        sender.send(NetworkCommand::CancelCamp(request)).unwrap();
        let deadline = Some(Instant::now() - std::time::Duration::from_secs(1));
        let event = std::future::ready(Ok(ZoneEvent::Gameplay(damage(1, 3))));
        let Input::Event(Ok(event)) =
            next_input(event, &mut requests, &mut updates, &mut heartbeat, deadline).await
        else {
            panic!("elapsed deadline beat ready authority");
        };
        assert!(matches!(
            *event,
            ZoneEvent::Gameplay(GameplayEvent::Damage(_))
        ));
        assert!(matches!(
            next_input(std::future::pending(), &mut requests, &mut updates, &mut heartbeat, deadline).await,
            Input::Request(NetworkCommand::CancelCamp(active)) if active == request
        ));
        assert!(matches!(
            next_input(
                std::future::pending(),
                &mut requests,
                &mut updates,
                &mut heartbeat,
                deadline
            )
            .await,
            Input::Deadline
        ));
    }

    #[test]
    fn corpse_events_do_not_desynchronize_camp_authority_after_respawn() {
        let (mut live, _requests) = super::super::tests::command_world(1, 1.);
        let mut worker = Authority::default();
        let mut motion = MovementAuthority {
            own_id: Some(1),
            ..Default::default()
        };
        live.enable_roster_return();
        let mut spawn = live.entities.get(&2).unwrap().spawn.clone();
        for event in [
            GameplayEvent::Death(Death {
                id: 1,
                killer_id: 9,
                corpse_id: 1,
                skill: 1,
                spell_id: 0,
                damage: 3,
            }),
            GameplayEvent::SpawnAppearance {
                id: 1,
                kind: 14,
                parameter: 115,
            },
            damage(1, 2),
        ] {
            worker.observe(&event, motion.own_id);
            motion.gameplay(&event, None);
            live.gameplay_event(event);
        }
        assert_eq!(live.own_id, Some(1)); // Presentation still knows the corpse.
        assert!(live.movement_authority.own_id.is_none());
        spawn.id = 3;
        spawn.name = "Player".into();
        spawn.npc = false;
        spawn.is_corpse = false;
        assert!(motion.spawn(&spawn, "Player"));
        assert!(live.movement_authority.spawn(&spawn, "Player"));
        let request = live
            .camp
            .request(
                live.movement_authority.action_epoch,
                live.movement_authority.revision,
            )
            .unwrap();
        assert!(
            worker.permits(request),
            "corpse packets poisoned future camp requests"
        );
        assert_eq!(request.motion_revision, motion.revision);
        assert_eq!(request.epoch, motion.action_epoch);
    }
    fn boundaries() -> Vec<GameplayEvent> {
        vec![
            damage(1, 3),
            GameplayEvent::SpawnAppearance {
                id: 1,
                kind: 14,
                parameter: 100,
            },
            GameplayEvent::Death(Death {
                id: 1,
                killer_id: 9,
                corpse_id: 1,
                skill: 1,
                spell_id: 0,
                damage: 3,
            }),
            GameplayEvent::Recovery(DeathEvent::RespawnWindow(RespawnWindow {
                initial_selection: 0,
                remaining_ms: 30_000,
                options: vec![],
            })),
            GameplayEvent::Recovery(DeathEvent::BindTransfer(BindTransfer {
                zone_id: 0,
                instance_id: 0,
                position: [0.; 3],
                heading: 0.,
                label: String::new(),
                save_items: 0,
                resources: [0; 3],
            })),
            GameplayEvent::ZoneTransition {
                zone_id: 77,
                instance_id: 0,
            },
            GameplayEvent::ZoneChangeRequested(openeq_net::gameplay::ZoneDestination {
                zone_id: 77,
                instance_id: 0,
                position: [0.; 3],
                heading: 0.,
            }),
        ]
    }
    #[test]
    fn own_damage_standing_death_and_travel_retire_countdown() {
        for event in boundaries() {
            let (mut live, mut requests) = super::super::tests::command_world(1, 1.);
            live.enable_roster_return();
            assert!(live.request_camp());
            let NetworkCommand::StartCamp(request, _) = requests.try_recv().unwrap() else {
                panic!()
            };
            assert!(interruption(&ZoneEvent::Gameplay(event.clone()), Some(1)).is_some());
            live.gameplay_event(event);
            assert!(live.camp_view().is_none());
            live.camp.started(request, Instant::now() + COUNTDOWN);
            live.camp.finishing(request);
            assert!(
                live.camp_view().is_none(),
                "late timer resurrected interrupted camp"
            );
        }
        assert_eq!(gameplay_interruption(&damage(1, 2), Some(1)), Some(true));
        for event in [
            damage(2, 3),
            damage(1, 0),
            damage(1, -1),
            GameplayEvent::SpawnAppearance {
                id: 1,
                kind: 14,
                parameter: 110,
            },
        ] {
            assert_eq!(gameplay_interruption(&event, Some(1)), None);
        }
    }
    #[test]
    fn camp_captures_valid_movement_and_blocks_gameplay_until_retired() {
        let (mut live, mut requests) = super::super::tests::command_world(1, 1.);
        live.enable_roster_return();
        let position = Position {
            x: 12.,
            y: -31.,
            z: 7.,
            heading: 49.,
            velocity: [1., 2., 3.],
            delta_heading: 5.,
            animation: 12,
        };
        live.movement.send_replace(Some(MovementUpdate {
            id: 1,
            revision: 0,
            position,
        }));
        assert!(live.request_camp());
        assert!(!live.request_camp());
        assert!(live.movement.borrow().is_none());
        let NetworkCommand::StartCamp(request, pose) = requests.try_recv().unwrap() else {
            panic!()
        };
        let (id, stopped) = live.movement_authority.stopped_position(pose).unwrap();
        assert_eq!(id, 1);
        assert_eq!(
            [stopped.x, stopped.y, stopped.z, stopped.heading],
            [12., -31., 7., 49.]
        );
        assert_eq!(stopped.velocity, [0.; 3]);
        assert_eq!((stopped.delta_heading, stopped.animation), (0., 0));
        assert!(!live.movement_allowed());
        assert!(!live.command(Command::AutoAttack(true)));
        live.set_target(Some(2));
        assert!(requests.try_recv().is_err());
        assert!(!live.cancel_camp(request.token + 1));
        assert!(live.cancel_camp(request.token));
        assert!(
            matches!(requests.try_recv().unwrap(), NetworkCommand::CancelCamp(active) if active == request)
        );
        live.camp.finishing(request);
        assert!(!live.cancel_camp(request.token));
        live.movement_authority.revision += 1;
        assert!(live.movement_authority.stopped_position(pose).is_none());
        assert!(live.movement_authority.stopped_position(None).is_none());
    }
}
