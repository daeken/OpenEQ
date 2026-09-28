use crate::{
    chat::Action,
    gameplay_ui::GameHudState,
    interaction::Interaction,
    live::LiveWorld,
    social_ui::{SocialAction, UiGroup, UiGroupMember},
};
use openeq_net::{gameplay::Command, social::SocialCommand};
impl Interaction {
    pub(crate) fn social_view(&self, live: &LiveWorld, view: &mut GameHudState) {
        let group = &live.game.group;
        view.group = (!group.members.is_empty() || group.invitation.is_some()).then(|| UiGroup {
            leader: group.leader.clone(),
            invitation: group.invitation.clone(),
            members: group
                .members
                .iter()
                .map(|member| {
                    let entity = live
                        .entities
                        .values()
                        .find(|entity| entity.spawn.name.eq_ignore_ascii_case(&member.name));
                    UiGroupMember {
                        name: member.name.clone(),
                        level: entity.map(|entity| entity.spawn.level),
                        hp: entity.map(|entity| f32::from(entity.spawn.hp_percent) / 100.),
                        mana: if member.name.eq_ignore_ascii_case(&live.character) {
                            live.game.mana.fraction()
                        } else {
                            None
                        },
                        in_zone: entity.is_some() && live.ready,
                        leader: member.name.eq_ignore_ascii_case(&group.leader),
                    }
                })
                .collect(),
        });
    }
    pub(crate) fn invite(&mut self, live: &mut LiveWorld, name: Option<String>) {
        let name = name.or_else(|| {
            live.target
                .and_then(|id| live.entities.get(&id))
                .filter(|entity| !entity.spawn.npc && !entity.spawn.is_corpse)
                .map(|entity| entity.spawn.name.clone())
        });
        if let Some(invitee) = name.filter(|name| !name.eq_ignore_ascii_case(&live.character)) {
            live.command(Command::Social(SocialCommand::Invite {
                inviter: live.character.clone(),
                invitee,
            }));
        } else {
            live.game
                .notice("Select another player, or use /invite NAME.");
        }
    }
    pub(crate) fn answer_invite(&mut self, live: &mut LiveWorld, accept: bool) {
        let Some(inviter) = live.game.group.invitation.clone() else {
            live.game.notice("There is no pending group invitation.");
            return;
        };
        let invitee = live.character.clone();
        let command = if accept {
            SocialCommand::Accept { inviter, invitee }
        } else {
            SocialCommand::Decline { inviter, invitee }
        };
        if live.command(Command::Social(command)) && !accept {
            live.game.group.invitation = None;
        }
    }
    pub(crate) fn social_action(&mut self, action: SocialAction, live: &mut LiveWorld) {
        match action {
            SocialAction::InviteTarget => self.invite(live, None),
            SocialAction::AcceptInvite => self.answer_invite(live, true),
            SocialAction::DeclineInvite => self.answer_invite(live, false),
            SocialAction::Leave => {
                self.action(
                    Action::LeaveGroup,
                    live,
                    live.player_position().unwrap_or([0.; 3]),
                );
            }
            SocialAction::SelectMember(name) => {
                if let Some(id) = live
                    .entities
                    .values()
                    .find(|entity| entity.spawn.name.eq_ignore_ascii_case(&name))
                    .map(|entity| entity.spawn.id)
                {
                    live.set_target(Some(id));
                } else {
                    live.game.notice("That group member is outside this zone.");
                }
            }
        }
    }
}
