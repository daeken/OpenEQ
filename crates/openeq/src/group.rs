//! Server-authoritative party membership, separate from visible zone entities.
use openeq_net::social::{GroupMember, SocialEvent};
#[derive(Default)]
pub struct GroupState {
    pub leader: String,
    pub members: Vec<GroupMember>,
    pub invitation: Option<String>,
}
impl GroupState {
    pub fn apply(&mut self, event: SocialEvent, own_name: &str) {
        match event {
            SocialEvent::Invitation { inviter, invitee }
                if invitee.eq_ignore_ascii_case(own_name) =>
            {
                self.invitation = Some(inviter)
            }
            SocialEvent::InvitationCancelled { inviter, invitee } => {
                if invitee.eq_ignore_ascii_case(own_name)
                    && self
                        .invitation
                        .as_ref()
                        .is_some_and(|name| name.eq_ignore_ascii_case(&inviter))
                {
                    self.invitation = None;
                }
            }
            SocialEvent::Snapshot { leader, members } => {
                self.leader = leader;
                self.members = members;
                if self.members.len() > 1 {
                    self.invitation = None;
                }
            }
            SocialEvent::MemberJoined(member) => {
                if let Some(entry) = self
                    .members
                    .iter_mut()
                    .find(|entry| entry.name.eq_ignore_ascii_case(&member.name))
                {
                    *entry = member;
                } else if self.members.len() < 6 {
                    self.members.push(member);
                }
            }
            SocialEvent::MemberLeft { member, .. } => {
                if member.eq_ignore_ascii_case(own_name) {
                    self.members.clear();
                    self.leader.clear();
                } else {
                    self.members
                        .retain(|entry| !entry.name.eq_ignore_ascii_case(&member));
                }
            }
            SocialEvent::Disbanded { .. } => {
                self.members.clear();
                self.leader.clear();
            }
            SocialEvent::LeaderChanged(leader) => self.leader = leader,
            _ => {}
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn member(name: &str) -> GroupMember {
        GroupMember {
            index: None,
            name: name.into(),
            owner: String::new(),
            mercenary: false,
            roles: [false; 3],
            offline: false,
        }
    }
    #[test]
    fn authoritative_rosters_reconcile_joins_leadership_and_removals() {
        let mut state = GroupState::default();
        state.apply(
            SocialEvent::Invitation {
                inviter: "Alice".into(),
                invitee: "Bob".into(),
            },
            "Bob",
        );
        assert_eq!(state.invitation.as_deref(), Some("Alice"));
        state.apply(
            SocialEvent::Snapshot {
                leader: "Alice".into(),
                members: vec![member("Alice"), member("Bob")],
            },
            "Bob",
        );
        assert!(state.invitation.is_none());
        state.apply(SocialEvent::MemberJoined(member("ALICE")), "Bob");
        assert_eq!(state.members.len(), 2);
        state.apply(SocialEvent::LeaderChanged("Bob".into()), "Bob");
        assert_eq!(state.leader, "Bob");
        state.apply(
            SocialEvent::MemberLeft {
                character: "Alice".into(),
                member: "Alice".into(),
            },
            "Bob",
        );
        assert_eq!(state.members.len(), 1);
        state.apply(
            SocialEvent::Disbanded {
                character: "Bob".into(),
                member: "Bob".into(),
            },
            "Bob",
        );
        assert!(state.members.is_empty() && state.leader.is_empty());
    }
}
