//! Raid membership comes only from received updates. A roster is rebuilt as a
//! bounded sequence of adds, with no wire count or end-of-roster marker.
use openeq_net::raid::{RaidCommand, RaidEvent, RaidMember};
use std::{
    collections::BTreeMap,
    time::{Duration, Instant},
};

#[derive(Clone, Debug)]
pub struct Invitation {
    pub inviter: String,
    pub token: u64,
}

#[derive(Clone, Debug)]
pub enum Request {
    Invite { invitee: String },
    Accept { token: u64 },
    Dismiss { token: u64 },
    Leave { revision: u64 },
    MakeLeader { revision: u64, leader: String },
}

#[derive(Clone, Debug)]
pub struct QueuedRequest {
    pub token: u64,
    pub command: RaidCommand,
}
struct Pending {
    token: u64,
    command: RaidCommand,
    started: Instant,
    sent: bool,
}

#[derive(Default)]
pub struct RaidState {
    pub active: bool,
    pub confirmed: bool,
    /// Counts only received membership changes, shared with the worker guard.
    pub generation: u64,
    pub revision: u64,
    pub leader: String,
    pub members: Vec<RaidMember>,
    pub invitation: Option<Invitation>,
    pub locked: Option<bool>,
    pub motd: Option<String>,
    pub notes: BTreeMap<String, String>,
    pub status: Option<String>,
    pending: Option<Pending>,
    outgoing_invite: Option<String>,
}

impl RaidState {
    fn changed(&mut self) {
        self.revision = self.revision.wrapping_add(1);
    }
    pub fn busy(&self) -> bool {
        self.pending.is_some()
    }
    pub fn member(&self, name: &str) -> Option<&RaidMember> {
        self.members
            .iter()
            .find(|member| member.name.eq_ignore_ascii_case(name))
    }
    pub fn is_leader(&self, name: &str) -> bool {
        self.active && self.confirmed && self.leader.eq_ignore_ascii_case(name)
    }
    fn clear(&mut self) {
        let revision = self.revision.wrapping_add(1);
        *self = Self {
            revision,
            generation: self.generation,
            ..Default::default()
        };
    }
    /// Retain the last received membership through travel, but invalidate all
    /// old choices. Destination create/add events rebuild the displayed roster.
    pub fn begin_zone(&mut self) {
        self.changed();
        self.confirmed = false;
        self.invitation = None;
        self.pending = None;
        self.outgoing_invite = None;
        self.status = None;
    }
    pub fn apply(&mut self, event: RaidEvent, own_name: &str) {
        if changes_membership(&event) {
            self.generation = self.generation.wrapping_add(1);
        }
        match event {
            RaidEvent::Invitation { inviter, invitee }
                if (!self.active || !self.confirmed) && invitee.eq_ignore_ascii_case(own_name) =>
            {
                if self
                    .pending
                    .as_ref()
                    .is_some_and(|pending| matches!(pending.command, RaidCommand::Accept { .. }))
                {
                    return;
                }
                self.changed();
                self.invitation = Some(Invitation {
                    inviter,
                    token: self.revision,
                });
                if self.pending.is_none() {
                    self.status = None;
                }
            }
            RaidEvent::Created { leader } => {
                self.clear();
                self.active = true;
                self.confirmed = true;
                self.leader = leader;
            }
            RaidEvent::MemberAdded(member) if self.active => {
                if self
                    .outgoing_invite
                    .as_ref()
                    .is_some_and(|name| name.eq_ignore_ascii_case(&member.name))
                {
                    self.outgoing_invite = None;
                    if self.pending.as_ref().is_some_and(|pending| matches!(&pending.command,
                        RaidCommand::Invite { invitee, .. } if invitee.eq_ignore_ascii_case(&member.name))) {
                        self.pending = None;
                    }
                    if self.pending.is_none() {
                        self.status = None;
                    }
                }
                if let Some(previous) = self
                    .members
                    .iter_mut()
                    .find(|previous| previous.name.eq_ignore_ascii_case(&member.name))
                {
                    *previous = member;
                } else if self.members.len() < 72 {
                    self.members.push(member);
                } else {
                    return;
                }
                self.changed();
            }
            RaidEvent::MemberRemoved { name } if self.active => {
                // Subgroup moves remove and re-add even the local member.
                // Only Disbanded/NoRaid establishes that we left the raid.
                self.members
                    .retain(|member| !member.name.eq_ignore_ascii_case(&name));
                self.notes
                    .retain(|member, _| !member.eq_ignore_ascii_case(&name));
                self.changed();
            }
            RaidEvent::Disbanded | RaidEvent::NoRaid => {
                self.clear();
                self.confirmed = true;
            }
            RaidEvent::LeaderChanged { leader } if self.active => {
                if self.pending.as_ref().is_some_and(|pending| {
                    matches!(
                        &pending.command, RaidCommand::MakeLeader { leader: requested, .. }
                            if requested.eq_ignore_ascii_case(&leader)
                    )
                }) {
                    self.pending = None;
                    self.status = None;
                }
                self.leader = leader;
                self.changed();
            }
            RaidEvent::LockChanged { locked } if self.active => {
                self.locked = Some(locked);
                self.changed();
            }
            RaidEvent::Motd { text } if self.active => self.motd = Some(text),
            RaidEvent::Note { member, text } if self.active => {
                if let Some(name) = self.member(&member).map(|member| member.name.clone()) {
                    self.notes.insert(name, text);
                }
            }
            _ => {}
        }
    }
    /// Reserve one request before queueing. A successful send never creates a
    /// roster, transfers leadership or removes a member optimistically.
    pub fn request(
        &mut self,
        request: Request,
        own_name: &str,
        now: Instant,
    ) -> Result<Option<QueuedRequest>, String> {
        if self.busy() {
            return Err("Waiting for the current raid request.".into());
        }
        if self.active
            && !self.confirmed
            && !matches!(request, Request::Accept { .. } | Request::Dismiss { .. })
        {
            return Err("Waiting for the current zone to confirm raid membership.".into());
        }
        let command = match request {
            Request::Invite { invitee } => {
                if self.active && !self.is_leader(own_name) {
                    return Err("Only the raid leader can invite players.".into());
                }
                if invitee.eq_ignore_ascii_case(own_name) || self.member(&invitee).is_some() {
                    return Err("Select another player who is not already in this raid.".into());
                }
                RaidCommand::Invite {
                    inviter: own_name.into(),
                    invitee,
                }
            }
            Request::Accept { token } | Request::Dismiss { token } => {
                let invitation = self
                    .invitation
                    .as_ref()
                    .filter(|invitation| {
                        invitation.token == token && (!self.active || !self.confirmed)
                    })
                    .ok_or("That raid invitation is no longer current.")?;
                if matches!(request, Request::Dismiss { .. }) {
                    self.invitation = None;
                    self.changed();
                    self.status = Some("Raid invitation dismissed.".into());
                    return Ok(None);
                }
                RaidCommand::Accept {
                    inviter: invitation.inviter.clone(),
                    invitee: own_name.into(),
                }
            }
            Request::Leave { revision } => {
                if !self.active || revision != self.revision {
                    return Err("The raid changed; use the current raid window.".into());
                }
                RaidCommand::Leave {
                    character: own_name.into(),
                }
            }
            Request::MakeLeader { revision, leader } => {
                if revision != self.revision
                    || !self.is_leader(own_name)
                    || self.member(&leader).is_none()
                    || leader.eq_ignore_ascii_case(own_name)
                {
                    return Err(
                        "Only the raid leader can choose another current member as leader.".into(),
                    );
                }
                RaidCommand::MakeLeader {
                    character: own_name.into(),
                    leader,
                }
            }
        };
        // Check names/shape before reserving the request; no invalid user text
        // can leave the UI waiting for a command that could never be encoded.
        openeq_net::raid::encode_command(command.clone()).map_err(|error| error.to_string())?;
        if let RaidCommand::Invite { invitee, .. } = &command {
            self.outgoing_invite = Some(invitee.clone());
        }
        self.changed();
        let token = self.revision;
        self.pending = Some(Pending {
            token,
            command: command.clone(),
            started: now,
            sent: false,
        });
        self.status = Some(
            match command {
                RaidCommand::Invite { .. } => "Raid invitation requested.",
                RaidCommand::Accept { .. } => "Waiting for the raid roster…",
                RaidCommand::Leave { .. } => "Waiting to leave the raid…",
                RaidCommand::MakeLeader { .. } => "Waiting for the raid leader to change…",
            }
            .into(),
        );
        Ok(Some(QueuedRequest { token, command }))
    }
    pub fn sent(&mut self, token: u64, now: Instant) {
        if let Some(pending) = &mut self.pending
            && pending.token == token
        {
            pending.started = now;
            pending.sent = true;
        }
    }
    pub fn rejected(&mut self, token: u64) -> bool {
        if !self
            .pending
            .as_ref()
            .is_some_and(|pending| pending.token == token)
        {
            return false;
        }
        let pending = self.pending.take().unwrap();
        if matches!(pending.command, RaidCommand::Invite { .. }) {
            self.outgoing_invite = None;
        }
        self.status = Some("Raid request was not sent.".into());
        true
    }
    pub fn tick(&mut self, now: Instant) {
        let Some(pending) = &self.pending else {
            return;
        };
        if !pending.sent {
            return;
        }
        let invite = matches!(pending.command, RaidCommand::Invite { .. });
        let wait = Duration::from_secs(if invite { 3 } else { 15 });
        if now.saturating_duration_since(pending.started) < wait {
            return;
        }
        if matches!(pending.command, RaidCommand::Accept { .. }) {
            // No confirmed result. Require a fresh invitation instead of
            // resending the stale accept; a later real roster still applies.
            self.invitation = None;
            self.changed();
        }
        self.pending = None;
        self.status = Some(
            if invite {
                "Waiting for the other player to accept."
            } else {
                "No raid confirmation received. Membership is unchanged; check server messages."
            }
            .into(),
        );
    }
}

pub(crate) fn changes_membership(event: &RaidEvent) -> bool {
    matches!(
        event,
        RaidEvent::Created { .. }
            | RaidEvent::Disbanded
            | RaidEvent::NoRaid
            | RaidEvent::MemberAdded(_)
            | RaidEvent::MemberRemoved { .. }
            | RaidEvent::LeaderChanged { .. }
            | RaidEvent::LockChanged { .. }
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    fn member(name: &str, group: Option<u8>) -> RaidMember {
        RaidMember {
            name: name.into(),
            class: 1,
            level: 12,
            group,
            group_leader: false,
        }
    }
    fn joined() -> RaidState {
        let mut state = RaidState::default();
        state.apply(
            RaidEvent::Created {
                leader: "Alice".into(),
            },
            "Alice",
        );
        state.apply(RaidEvent::MemberAdded(member("Alice", None)), "Alice");
        state.apply(RaidEvent::MemberAdded(member("Bob", None)), "Alice");
        state
    }
    #[test]
    fn incremental_rebuilds_and_local_subgroup_moves_do_not_invent_disband() {
        let mut state = joined();
        state.apply(
            RaidEvent::MemberRemoved {
                name: "ALICE".into(),
            },
            "Alice",
        );
        assert!(state.active);
        state.apply(RaidEvent::MemberAdded(member("Alice", Some(2))), "Alice");
        state.apply(RaidEvent::MemberAdded(member("BOB", Some(1))), "Alice");
        assert_eq!(state.members.len(), 2);
        assert_eq!(state.member("bob").unwrap().group, Some(1));
        state.begin_zone();
        assert_eq!(state.members.len(), 2);
        state.apply(
            RaidEvent::Created {
                leader: "Bob".into(),
            },
            "Alice",
        );
        assert!(state.active && state.members.is_empty());
        for i in 0..80 {
            state.apply(
                RaidEvent::MemberAdded(member(&format!("M{i}"), None)),
                "Alice",
            );
        }
        assert_eq!(state.members.len(), 72);
        state.apply(RaidEvent::Disbanded, "Alice");
        assert!(!state.active && state.members.is_empty());
    }
    #[test]
    fn invitations_require_current_identity_and_accept_never_creates_membership() {
        let mut state = RaidState::default();
        let now = Instant::now();
        state.apply(
            RaidEvent::Invitation {
                inviter: "Alice".into(),
                invitee: "Other".into(),
            },
            "Bob",
        );
        assert!(state.invitation.is_none());
        let invite = RaidEvent::Invitation {
            inviter: "Alice".into(),
            invitee: "Bob".into(),
        };
        state.apply(invite.clone(), "Bob");
        let stale = state.invitation.as_ref().unwrap().token;
        state.apply(invite, "Bob");
        assert!(
            state
                .request(Request::Accept { token: stale }, "Bob", now)
                .is_err()
        );
        let token = state.invitation.as_ref().unwrap().token;
        assert!(matches!(
            state
                .request(Request::Accept { token }, "Bob", now)
                .unwrap(),
            Some(QueuedRequest {
                command: RaidCommand::Accept { .. },
                ..
            })
        ));
        assert!(!state.active && state.members.is_empty());
        assert!(
            state
                .request(Request::Accept { token }, "Bob", now)
                .is_err()
        );
        state.sent(state.pending.as_ref().unwrap().token, now);
        state.tick(now + Duration::from_secs(16));
        assert!(state.invitation.is_none() && !state.busy() && !state.active);
        state.apply(
            RaidEvent::Created {
                leader: "Alice".into(),
            },
            "Bob",
        );
        assert!(state.active);
    }
    #[test]
    fn stale_roster_actions_and_nonleader_transfer_cannot_send() {
        let mut state = joined();
        let now = Instant::now();
        let stale = state.revision;
        state.apply(RaidEvent::MemberAdded(member("Charlie", None)), "Alice");
        assert!(
            state
                .request(Request::Leave { revision: stale }, "Alice", now)
                .is_err()
        );
        let request = Request::MakeLeader {
            revision: state.revision,
            leader: "Bob".into(),
        };
        assert!(state.request(request.clone(), "Charlie", now).is_err());
        let command = state.request(request, "Alice", now).unwrap().unwrap();
        assert_eq!(state.leader, "Alice");
        state.rejected(command.token);
        assert!(!state.busy());
        assert_eq!(state.leader, "Alice");
        state.apply(
            RaidEvent::LeaderChanged {
                leader: "Bob".into(),
            },
            "Alice",
        );
        assert_eq!(state.leader, "Bob");
        let revision = state.revision;
        state
            .request(Request::Leave { revision }, "Alice", now)
            .unwrap();
        assert!(state.active);
        state.apply(
            RaidEvent::MemberRemoved {
                name: "Alice".into(),
            },
            "Alice",
        );
        assert!(state.active && state.busy());
        state.apply(RaidEvent::Disbanded, "Alice");
        assert!(!state.active && !state.busy());
    }

    #[test]
    fn invited_member_confirmation_clears_only_its_own_waiting_request() {
        let now = Instant::now();
        for delayed in [false, true] {
            let mut state = joined();
            state
                .request(
                    Request::Invite {
                        invitee: "Charlie".into(),
                    },
                    "Alice",
                    now,
                )
                .unwrap();
            state.apply(RaidEvent::MemberAdded(member("David", None)), "Alice");
            assert!(state.busy());
            if delayed {
                state.sent(state.pending.as_ref().unwrap().token, now);
                state.tick(now + Duration::from_secs(4));
            }
            state.apply(RaidEvent::MemberAdded(member("CHARLIE", None)), "Alice");
            assert!(!state.busy() && state.status.is_none());
        }
        let mut state = joined();
        state
            .request(
                Request::Invite {
                    invitee: "Charlie".into(),
                },
                "Alice",
                now,
            )
            .unwrap();
        state.sent(state.pending.as_ref().unwrap().token, now);
        state.tick(now + Duration::from_secs(4));
        state
            .request(
                Request::MakeLeader {
                    revision: state.revision,
                    leader: "Bob".into(),
                },
                "Alice",
                now,
            )
            .unwrap();
        state.apply(RaidEvent::MemberAdded(member("Charlie", None)), "Alice");
        assert!(state.busy());
        state.apply(
            RaidEvent::LeaderChanged {
                leader: "Bob".into(),
            },
            "Alice",
        );
        assert!(!state.busy());
    }

    #[test]
    fn old_worker_results_and_unsent_timeouts_cannot_release_a_new_request() {
        let now = Instant::now();
        let mut state = joined();
        let first = state
            .request(
                Request::Invite {
                    invitee: "Charlie".into(),
                },
                "Alice",
                now,
            )
            .unwrap()
            .unwrap();
        state.tick(now + Duration::from_secs(30));
        assert!(
            state.busy(),
            "a queued request cannot expire before send acknowledgment"
        );
        state.sent(first.token, now);
        state.tick(now + Duration::from_secs(4));
        let second = state
            .request(
                Request::Invite {
                    invitee: "Charlie".into(),
                },
                "Alice",
                now,
            )
            .unwrap()
            .unwrap();
        assert_ne!(first.token, second.token);
        assert!(!state.rejected(first.token));
        state.sent(first.token, now);
        state.tick(now + Duration::from_secs(30));
        assert!(state.busy());
        assert!(state.rejected(second.token));
        assert!(!state.busy());
    }

    #[test]
    fn zone_snapshot_has_no_authority_and_new_invite_cannot_replace_pending_accept() {
        let now = Instant::now();
        let mut state = joined();
        state.begin_zone();
        assert!(state.active && !state.confirmed);
        assert!(!state.is_leader("Alice"));
        assert!(
            state
                .request(
                    Request::MakeLeader {
                        revision: state.revision,
                        leader: "Bob".into()
                    },
                    "Alice",
                    now
                )
                .is_err()
        );
        assert!(
            state
                .request(
                    Request::Leave {
                        revision: state.revision
                    },
                    "Alice",
                    now
                )
                .is_err()
        );
        let invitation = RaidEvent::Invitation {
            inviter: "Charlie".into(),
            invitee: "Alice".into(),
        };
        state.apply(invitation, "Alice");
        let token = state.invitation.as_ref().unwrap().token;
        state
            .request(Request::Accept { token }, "Alice", now)
            .unwrap();
        state.apply(
            RaidEvent::Invitation {
                inviter: "David".into(),
                invitee: "Alice".into(),
            },
            "Alice",
        );
        assert!(state.busy());
        assert_eq!(state.invitation.as_ref().unwrap().token, token);
        assert!(
            state
                .request(Request::Accept { token }, "Alice", now)
                .is_err()
        );
        state.apply(
            RaidEvent::Created {
                leader: "Charlie".into(),
            },
            "Alice",
        );
        assert!(state.active && state.confirmed && !state.busy());
        assert!(state.members.is_empty());
    }

    #[test]
    fn worker_and_foreground_membership_generations_survive_disband_and_rebuild() {
        let now = Instant::now();
        let mut state = joined();
        let queued_generation = state.generation;
        state
            .request(
                Request::Leave {
                    revision: state.revision,
                },
                "Alice",
                now,
            )
            .unwrap();
        assert_eq!(
            state.generation, queued_generation,
            "local requests are not server membership events"
        );
        let events = [
            RaidEvent::Disbanded,
            RaidEvent::Created {
                leader: "Bob".into(),
            },
            RaidEvent::MemberAdded(member("Alice", None)),
            RaidEvent::Motd {
                text: "New raid".into(),
            },
        ];
        let mut worker_generation = queued_generation;
        for event in events {
            if changes_membership(&event) {
                worker_generation = worker_generation.wrapping_add(1);
            }
            state.apply(event, "Alice");
            assert_eq!(state.generation, worker_generation);
        }
        assert_ne!(
            worker_generation, queued_generation,
            "a queued leave cannot apply to the new raid"
        );
        assert!(state.active && !state.busy());
        state.begin_zone();
        assert_eq!(state.generation, worker_generation);
    }
}
