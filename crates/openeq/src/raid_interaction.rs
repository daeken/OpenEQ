use crate::{
    gameplay_ui::{
        GameHudState, RaidAction, RaidActionKind, UiRaid, UiRaidInvitation, UiRaidMember,
        UiRaidPage,
    },
    interaction::Interaction,
    live::LiveWorld,
    raid::Request,
};
use std::time::Instant;

impl LiveWorld {
    pub fn raid_request(&mut self, request: Request) -> bool {
        if !self.ready || self.error.is_some() || !self.movement_allowed() {
            self.game
                .error("Wait until your character is ready before changing the raid.");
            return false;
        }
        if let Request::Invite { invitee } = &request {
            let available = self.entities.values().any(|entity| {
                !entity.spawn.npc
                    && !entity.spawn.is_corpse
                    && entity.spawn.name.eq_ignore_ascii_case(invitee)
            });
            if !available {
                self.game
                    .error("Raid invitations need a player in this zone.");
                return false;
            }
        }
        let command = match self
            .game
            .raid
            .request(request, &self.character, Instant::now())
        {
            Ok(Some(command)) => command,
            Ok(None) => return true,
            Err(error) => {
                self.game.error(error);
                return false;
            }
        };
        let token = command.token;
        if !self.queue_raid(command) {
            self.game.raid.rejected(token);
            return false;
        }
        true
    }
}

impl Interaction {
    pub(crate) fn raid_view(&self, live: &LiveWorld, view: &mut GameHudState) {
        if !self.raid_open {
            return;
        }
        let raid = &live.game.raid;
        let ready = live.movement_allowed() && !raid.busy();
        let status = if !live.ready || live.error.is_some() {
            "Raid state is unavailable while disconnected or changing zones.".into()
        } else if raid.active && !raid.confirmed && raid.invitation.is_none() {
            "Last known roster; waiting for this zone to confirm raid membership.".into()
        } else if let Some(status) = &raid.status {
            status.clone()
        } else if raid.active && raid.members.is_empty() {
            "Receiving raid members…".into()
        } else if raid.active {
            format!(
                "{} known members · Subgroups 1–12 · /rsay to chat",
                raid.members.len()
            )
        } else {
            "No raid roster received. Select another player in this zone to invite.".into()
        };
        let mut members: Vec<_> = raid
            .members
            .iter()
            .map(|member| UiRaidMember {
                name: member.name.clone(),
                class: member.class,
                level: member.level,
                group: member.group,
                group_leader: member.group_leader,
            })
            .collect();
        members.sort_by(|a, b| {
            a.group
                .unwrap_or(12)
                .cmp(&b.group.unwrap_or(12))
                .then_with(|| {
                    a.name
                        .to_ascii_lowercase()
                        .cmp(&b.name.to_ascii_lowercase())
                })
        });
        view.raid = Some(UiRaid {
            revision: raid.revision,
            status,
            leader: raid.active.then(|| raid.leader.clone()),
            locked: raid.locked,
            members,
            invitation: raid.invitation.as_ref().map(|invitation| UiRaidInvitation {
                token: invitation.token,
                inviter: invitation.inviter.clone(),
                invitee: live.character.clone(),
                pending: raid.busy() || !ready,
            }),
            can_invite: ready && (!raid.active || raid.is_leader(&live.character)),
            can_leave: ready && raid.active && raid.confirmed,
            scroll: self.raid_scroll,
            selected_member: self.raid_selection.clone(),
            page: self.raid_page,
            motd: raid.motd.clone(),
            motd_scroll_rows: self.raid_motd_scroll,
        });
    }
    pub(crate) fn raid_tick(&mut self, live: &LiveWorld) {
        let raid = &live.game.raid;
        let invitation = raid.invitation.as_ref().map(|invitation| invitation.token);
        if (invitation.is_some() && invitation != self.raid_seen_invitation)
            || (raid.active && !self.raid_was_active)
        {
            self.raid_open = true;
            self.raid_page = UiRaidPage::Members;
            self.raid_scroll = 0;
        }
        self.raid_seen_invitation = invitation;
        self.raid_was_active = raid.active;
        if self
            .raid_selection
            .as_ref()
            .is_some_and(|name| raid.member(name).is_none())
        {
            self.raid_selection = None;
        }
        self.raid_scroll = self.raid_scroll.min(
            raid.members
                .len()
                .saturating_sub(self.raid_visible_rows.max(1)),
        );
    }
    pub(crate) fn raid_invite(&mut self, live: &mut LiveWorld, name: Option<String>) {
        let name = name.or_else(|| {
            live.target
                .and_then(|id| live.entities.get(&id))
                .filter(|entity| !entity.spawn.npc && !entity.spawn.is_corpse)
                .map(|entity| entity.spawn.name.clone())
        });
        if let Some(invitee) = name {
            live.raid_request(Request::Invite { invitee });
            self.raid_open = true;
        } else {
            live.game
                .notice("Select another player, or use /raidinvite NAME.");
        }
    }
    pub(crate) fn raid_answer(&mut self, live: &mut LiveWorld, accept: bool) {
        let Some(token) = live
            .game
            .raid
            .invitation
            .as_ref()
            .map(|invite| invite.token)
        else {
            live.game.notice("There is no current raid invitation.");
            return;
        };
        live.raid_request(if accept {
            Request::Accept { token }
        } else {
            Request::Dismiss { token }
        });
    }
    pub(crate) fn raid_action(&mut self, action: RaidAction, live: &mut LiveWorld) {
        if action.revision != live.game.raid.revision {
            return;
        }
        match action.kind {
            RaidActionKind::InviteTarget => self.raid_invite(live, None),
            RaidActionKind::AcceptInvite { token } => {
                live.raid_request(Request::Accept { token });
            }
            RaidActionKind::DismissInvite { token } => {
                live.raid_request(Request::Dismiss { token });
            }
            RaidActionKind::Leave => {
                live.raid_request(Request::Leave {
                    revision: action.revision,
                });
            }
            RaidActionKind::SelectMember { name } => {
                if let Some(member) = live.game.raid.member(&name) {
                    self.raid_selection = Some(member.name.clone());
                }
            }
            RaidActionKind::Scroll { rows } => {
                let max = live
                    .game
                    .raid
                    .members
                    .len()
                    .saturating_sub(self.raid_visible_rows.max(1));
                self.raid_scroll = self
                    .raid_scroll
                    .min(max)
                    .saturating_add_signed(rows as isize)
                    .min(max);
            }
            RaidActionKind::Page(page) => {
                self.raid_page = page;
            }
            RaidActionKind::ScrollMotd { rows } => {
                self.raid_motd_scroll = self
                    .raid_motd_scroll
                    .saturating_add_signed(rows as isize)
                    .min(self.raid_motd_max_scroll);
            }
        }
    }
    pub fn raid_wheel(&mut self, live: &mut LiveWorld, delta: f32) {
        if !delta.is_finite() || delta == 0. {
            return;
        }
        let rows = (-delta * 3.).clamp(-72., 72.) as i32;
        self.raid_action(
            RaidAction {
                revision: live.game.raid.revision,
                kind: if self.raid_page == UiRaidPage::Members {
                    RaidActionKind::Scroll { rows }
                } else {
                    RaidActionKind::ScrollMotd { rows }
                },
            },
            live,
        );
    }
    pub fn raid_text_metrics(&mut self, revision: u64, metrics: &[openeq_ui::TextScrollMetrics]) {
        let id = crate::raid_ui::raid_motd_scroll_id(revision);
        if let Some(metric) = metrics.iter().find(|metric| metric.id == id) {
            self.raid_motd_max_scroll = metric.total_rows.saturating_sub(metric.visible_rows);
            self.raid_motd_scroll = self.raid_motd_scroll.min(self.raid_motd_max_scroll);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::live::{NetworkCommand, tests::command_world};
    use openeq_net::gameplay::Command;
    use openeq_net::raid::{RaidCommand, RaidEvent, RaidMember};

    #[test]
    fn current_invitation_sends_once_and_stale_ui_cannot_accept() {
        let (mut live, mut wire) = command_world(1, 10.);
        live.game.raid.apply(
            RaidEvent::Invitation {
                inviter: "Other".into(),
                invitee: "Player".into(),
            },
            "Player",
        );
        let token = live.game.raid.invitation.as_ref().unwrap().token;
        let revision = live.game.raid.revision;
        let mut ui = Interaction::default();
        ui.raid_action(
            RaidAction {
                revision: revision.wrapping_sub(1),
                kind: RaidActionKind::AcceptInvite { token },
            },
            &mut live,
        );
        assert!(wire.try_recv().is_err());
        let action = RaidAction {
            revision,
            kind: RaidActionKind::AcceptInvite { token },
        };
        ui.raid_action(action.clone(), &mut live);
        ui.raid_action(action, &mut live);
        let NetworkCommand::Raid { request, .. } = wire.try_recv().unwrap() else {
            panic!("raid command");
        };
        assert!(matches!(request.command, RaidCommand::Accept { .. }));
        assert!(wire.try_recv().is_err());
        assert!(!live.game.raid.active);
        live.game.raid.rejected(request.token);
        assert!(!live.game.raid.busy());
        assert!(live.raid_request(Request::Dismiss { token }));
        assert!(wire.try_recv().is_err());
        assert!(live.game.raid.invitation.is_none());
    }

    #[test]
    fn invite_validates_player_presence_and_raid_chat_keeps_channel_fifteen() {
        let (mut live, mut wire) = command_world(1, 10.);
        live.entities.get_mut(&2).unwrap().spawn.name = "Other".into();
        assert!(!live.raid_request(Request::Invite {
            invitee: "Other".into()
        }));
        live.entities.get_mut(&2).unwrap().spawn.npc = false;
        assert!(!live.raid_request(Request::Invite {
            invitee: "Absent".into()
        }));
        assert!(wire.try_recv().is_err());
        assert!(live.raid_request(Request::Invite {
            invitee: "Other".into()
        }));
        assert!(matches!(
            wire.try_recv().unwrap(),
            NetworkCommand::Raid {
                request: crate::raid::QueuedRequest {
                    command: RaidCommand::Invite { .. },
                    ..
                },
                ..
            }
        ));
        let mut ui = Interaction::default();
        ui.submit("/rsay Ready here", &mut live, [0.; 3]);
        assert!(
            matches!(wire.try_recv().unwrap(), NetworkCommand::Gameplay(Command::Chat {
            channel: openeq_net::gameplay::ChatChannel::Raid, text, ..
        }, _) if text == "Ready here")
        );
    }

    #[test]
    fn roster_scroll_reverses_immediately_at_tail_and_selection_is_read_only() {
        let (mut live, mut wire) = command_world(1, 10.);
        live.game.raid.apply(
            RaidEvent::Created {
                leader: "Player".into(),
            },
            "Player",
        );
        for index in 0..72 {
            live.game.raid.apply(
                RaidEvent::MemberAdded(RaidMember {
                    name: format!("Member{index}"),
                    class: 1,
                    level: 20,
                    group: None,
                    group_leader: false,
                }),
                "Player",
            );
        }
        let mut ui = Interaction {
            raid_open: true,
            raid_scroll: 71,
            raid_visible_rows: 12,
            ..Default::default()
        };
        ui.raid_wheel(&mut live, 1.);
        assert_eq!(ui.raid_scroll, 57);
        ui.raid_action(
            RaidAction {
                revision: live.game.raid.revision,
                kind: RaidActionKind::SelectMember {
                    name: "Member4".into(),
                },
            },
            &mut live,
        );
        assert_eq!(ui.raid_selection.as_deref(), Some("Member4"));
        assert!(wire.try_recv().is_err());
        assert!(live.target.is_none());
        let frame = ui.view(&live);
        assert_eq!(frame.raid.as_ref().unwrap().members.len(), 72);
        ui.close_window("raid", &mut live);
        assert!(ui.view(&live).raid.is_none());
        assert!(live.game.raid.active);
        assert!(wire.try_recv().is_err());
    }
}
