//! Group presentation; membership and invitation decisions remain server-owned.
use crate::gameplay_ui::{GOLD, GameHudState, MUTED, Painter, WHITE, position};
use openeq_ui::{HitTarget, Rect, UiBindings};

#[derive(Clone, Debug, Default)]
pub struct UiGroupMember {
    pub name: String,
    pub level: Option<u8>,
    pub hp: Option<f32>,
    pub mana: Option<f32>,
    pub in_zone: bool,
    pub leader: bool,
}
#[derive(Clone, Debug, Default)]
pub struct UiGroup {
    pub leader: String,
    pub members: Vec<UiGroupMember>,
    pub invitation: Option<String>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SocialAction {
    InviteTarget,
    AcceptInvite,
    DeclineInvite,
    Leave,
    SelectMember(String),
}
impl SocialAction {
    pub fn from_hit(hit: &HitTarget) -> Option<Self> {
        if let Some(name) = hit.item.strip_prefix("social:member:") {
            return (!name.is_empty()).then(|| Self::SelectMember(name.into()));
        }
        match hit.item.as_str() {
            "social:invite" => Some(Self::InviteTarget),
            "social:accept" => Some(Self::AcceptInvite),
            "social:decline" => Some(Self::DeclineInvite),
            "social:leave" => Some(Self::Leave),
            _ => None,
        }
    }
}
impl Painter<'_> {
    pub(crate) fn group(&mut self, state: &GameHudState, group: &UiGroup) {
        let rows = group.members.len().min(6);
        let invitation_height = if group.invitation.is_some() { 72. } else { 0. };
        let rect = position(
            state,
            "group",
            Rect::new(12., 190., 250., rows as f32 * 48. + 65. + invitation_height),
            self.screen,
        );
        self.shell("GroupWindow", "group", rect, "Group", false);
        for (index, member) in group.members.iter().take(6).enumerate() {
            let bounds = Rect::new(
                rect.x + 10.,
                rect.y + 25. + index as f32 * 48.,
                rect.width - 20.,
                44.,
            );
            let mut name = member.name.clone();
            if let Some(level) = member.level {
                name.push_str(&format!(" · {level}"));
            }
            if member.leader || member.name == group.leader {
                name.push_str(" (leader)");
            }
            self.text(
                Rect::new(bounds.x, bounds.y, bounds.width, 17.),
                name,
                if member.in_zone { WHITE } else { MUTED },
                false,
            );
            if member.in_zone {
                for (name, value, y) in [
                    ("GW_Gauge1", member.hp, 18.),
                    ("GW_ManaGauge1", member.mana, 31.),
                ] {
                    if let Some(value) = value {
                        let mut bindings = UiBindings::default();
                        let gauge = bindings.widget_mut(name);
                        let offset = if name == "GW_Gauge1" { 15. } else { 0. };
                        gauge.rect = Some(Rect::new(
                            bounds.x,
                            bounds.y + y - offset,
                            bounds.width,
                            12. + offset,
                        ));
                        gauge.text = Some(String::new());
                        gauge.gauge = Some(value);
                        self.widget(name, &bindings);
                    }
                }
            } else {
                self.text(
                    Rect::new(bounds.x, bounds.y + 20., bounds.width, 17.),
                    "Outside this zone",
                    MUTED,
                    false,
                );
            }
            self.hit_enabled(
                format!("social:member:{}", member.name),
                "GroupMember",
                bounds,
                Some(member.name.clone()),
                member.in_zone,
            );
        }
        let y = rect.y + 28. + rows as f32 * 48.;
        self.button(
            "GW_InviteButton",
            "social:invite",
            Rect::new(rect.x + 10., y, 112., 24.),
            "Invite target",
            false,
        );
        self.button_enabled(
            "GW_DisbandButton",
            "social:leave",
            Rect::new(rect.x + 130., y, 110., 24.),
            "Leave group",
            false,
            !group.members.is_empty(),
        );
        if let Some(inviter) = &group.invitation {
            self.text(
                Rect::new(rect.x + 12., y + 33., rect.width - 24., 30.),
                format!("{inviter} invites you to join a group."),
                GOLD,
                true,
            );
            self.button(
                "GW_FollowButton",
                "social:accept",
                Rect::new(rect.x + 10., y + 69., 112., 24.),
                "Accept",
                false,
            );
            self.button(
                "GW_DeclineButton",
                "social:decline",
                Rect::new(rect.x + 130., y + 69., 110., 24.),
                "Decline",
                false,
            );
        }
    }
}
