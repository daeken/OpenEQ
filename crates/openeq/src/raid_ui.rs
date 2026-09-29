//! Original raid skin bound to verified state. Drawing cannot change membership;
//! action revisions and invitation tokens are revalidated by the live adapter.
use crate::gameplay_ui::{GOLD, GameHudState, MUTED, Painter, WHITE, position};
use openeq_ui::{HitTarget, Rect, UiBindings};

pub const RAID_VISIBLE_ROWS: usize = 12;
const WINDOW_HEIGHT: f32 = 560.;
const CONTENT_TOP: f32 = 106.;
const INVITATION_HEIGHT: f32 = 64.;
const FOOTER_HEIGHT: f32 = 98.;
const HEADER_HEIGHT: f32 = 24.;
const ROW_HEIGHT: f32 = 20.;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum UiRaidPage {
    #[default]
    Members,
    Information,
}

#[derive(Clone, Debug, Default)]
pub struct UiRaidMember {
    pub name: String,
    pub class: u8,
    pub level: u8,
    /// Wire groups 0..11 are displayed as 1..12; None means ungrouped.
    pub group: Option<u8>,
    pub group_leader: bool,
}

#[derive(Clone, Debug, Default)]
pub struct UiRaidInvitation {
    pub token: u64,
    pub inviter: String,
    pub invitee: String,
    pub pending: bool,
}

#[derive(Clone, Debug, Default)]
pub struct UiRaid {
    /// Must remain distinct across sessions as well as roster revisions.
    pub revision: u64,
    /// The adapter distinguishes waiting, receiving and confirmed membership.
    pub status: String,
    pub leader: Option<String>,
    pub locked: Option<bool>,
    pub members: Vec<UiRaidMember>,
    pub invitation: Option<UiRaidInvitation>,
    pub can_invite: bool,
    pub can_leave: bool,
    pub scroll: usize,
    pub selected_member: Option<String>,
    pub page: UiRaidPage,
    pub motd: Option<String>,
    pub motd_scroll_rows: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RaidAction {
    pub revision: u64,
    pub kind: RaidActionKind,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RaidActionKind {
    InviteTarget,
    AcceptInvite { token: u64 },
    DismissInvite { token: u64 },
    Leave,
    SelectMember { name: String },
    Scroll { rows: i32 },
    Page(UiRaidPage),
    ScrollMotd { rows: i32 },
}

impl RaidAction {
    pub fn from_hit(hit: &HitTarget) -> Option<Self> {
        if !hit.enabled || hit.window_id.as_deref() != Some("raid") {
            return None;
        }
        let (revision, action) = hit.item.strip_prefix("raid:")?.split_once(':')?;
        let revision = revision.parse().ok()?;
        let kind = match action {
            "invite" => RaidActionKind::InviteTarget,
            "leave" => RaidActionKind::Leave,
            "members" => RaidActionKind::Page(UiRaidPage::Members),
            "information" => RaidActionKind::Page(UiRaidPage::Information),
            _ => {
                let (action, value) = action.split_once(':')?;
                match action {
                    "accept" => RaidActionKind::AcceptInvite {
                        token: value.parse().ok()?,
                    },
                    "dismiss" => RaidActionKind::DismissInvite {
                        token: value.parse().ok()?,
                    },
                    "member" if !value.is_empty() => {
                        RaidActionKind::SelectMember { name: value.into() }
                    }
                    "scroll" | "motd" => {
                        let rows = value
                            .parse::<i32>()
                            .ok()
                            .filter(|rows| matches!(rows, -1 | 1))?;
                        if action == "scroll" {
                            RaidActionKind::Scroll { rows }
                        } else {
                            RaidActionKind::ScrollMotd { rows }
                        }
                    }
                    _ => return None,
                }
            }
        };
        Some(Self { revision, kind })
    }
}

/// Matches the actual number of visible rows in the viewport-clamped window.
pub fn raid_visible_rows(viewport_height: u32, has_invitation: bool) -> usize {
    let available = (viewport_height as f32).min(WINDOW_HEIGHT)
        - CONTENT_TOP
        - FOOTER_HEIGHT
        - HEADER_HEIGHT
        - if has_invitation {
            INVITATION_HEIGHT
        } else {
            0.
        };
    ((available.max(0.) / ROW_HEIGHT) as usize).min(RAID_VISIBLE_ROWS)
}

pub fn raid_motd_scroll_id(revision: u64) -> String {
    format!("raid:motd:{revision}")
}

impl Painter<'_> {
    pub(crate) fn raid(&mut self, state: &GameHudState, raid: &UiRaid) {
        let rect = position(
            state,
            "raid",
            Rect::new(self.screen.width * 0.5 - 340., 24., 680., WINDOW_HEIGHT),
            self.screen,
        );
        self.shell("RaidWindow", "raid", rect, "Raid", true);
        let status = if raid.status.is_empty() {
            "Waiting for raid information"
        } else {
            &raid.status
        };
        self.text(
            Rect::new(rect.x + 12., rect.y + 29., (rect.width - 180.).max(0.), 18.),
            status,
            GOLD,
            false,
        );
        self.text(
            Rect::new(rect.right() - 165., rect.y + 29., 153., 18.),
            format!("{} known members", raid.members.len()),
            MUTED,
            false,
        );
        let leader = raid.leader.as_deref().unwrap_or("Unknown");
        let lock = match raid.locked {
            Some(true) => "Locked",
            Some(false) => "Unlocked",
            None => "Lock state unknown",
        };
        self.text(
            Rect::new(rect.x + 12., rect.y + 51., (rect.width - 24.).max(0.), 18.),
            format!("Leader: {leader} · {lock}"),
            WHITE,
            false,
        );
        for (index, (id, label, page)) in [
            ("members", "Members", UiRaidPage::Members),
            ("information", "Information", UiRaidPage::Information),
        ]
        .into_iter()
        .enumerate()
        {
            self.button(
                "RAID_InviteButton",
                &format!("raid:{}:{id}", raid.revision),
                Rect::new(rect.x + 12. + index as f32 * 124., rect.y + 75., 116., 24.),
                label,
                raid.page == page,
            );
        }
        let mut top = rect.y + CONTENT_TOP;
        if let Some(invitation) = &raid.invitation {
            self.text(
                Rect::new(rect.x + 12., top, (rect.width - 24.).max(0.), 20.),
                format!(
                    "{} invites {} to join a raid.",
                    invitation.inviter, invitation.invitee
                ),
                GOLD,
                false,
            );
            self.button_enabled(
                "RAID_AcceptButton",
                &format!("raid:{}:accept:{}", raid.revision, invitation.token),
                Rect::new(rect.x + 12., top + 25., 118., 24.),
                if invitation.pending {
                    "Accepting…"
                } else {
                    "Accept"
                },
                false,
                !invitation.pending,
            );
            self.button_enabled(
                "RAID_DeclineButton",
                &format!("raid:{}:dismiss:{}", raid.revision, invitation.token),
                Rect::new(rect.x + 140., top + 25., 118., 24.),
                "Dismiss",
                false,
                !invitation.pending,
            );
            if let Some(hit) = self.frame.hit_targets.last_mut() {
                hit.tooltip = Some("Dismiss this invitation locally".into());
            }
            top += INVITATION_HEIGHT;
        }
        match raid.page {
            UiRaidPage::Members => self.raid_members(rect, top, raid),
            UiRaidPage::Information => self.raid_information(rect, top, raid),
        }
        self.button_enabled(
            "RAID_InviteButton",
            &format!("raid:{}:invite", raid.revision),
            Rect::new(rect.x + 12., rect.bottom() - 34., 118., 24.),
            "Invite target",
            false,
            raid.can_invite,
        );
        self.button_enabled(
            "RAID_DisbandButton",
            &format!("raid:{}:leave", raid.revision),
            Rect::new(rect.right() - 130., rect.bottom() - 34., 118., 24.),
            "Leave raid",
            false,
            raid.can_leave,
        );
        if let Some(hit) = self.frame.hit_targets.last_mut() {
            hit.tooltip = Some("Leave this raid yourself".into());
        }
    }

    fn raid_members(&mut self, rect: Rect, top: f32, raid: &UiRaid) {
        let rows = raid_visible_rows(self.screen.height as u32, raid.invitation.is_some());
        let list = Rect::new(
            rect.x + 10.,
            top,
            (rect.width - 20.).max(0.),
            HEADER_HEIGHT + rows as f32 * ROW_HEIGHT,
        );
        let mut bindings = UiBindings::default();
        bindings.widget_mut("RAID_PlayerList").rect = Some(list);
        self.widget("RAID_PlayerList", &bindings);
        let cells = (list.width - 24.).max(0.);
        let columns: Vec<(&str, f32)> = if cells >= 570. {
            vec![
                ("Grp", 52.),
                ("Player name", cells - 322.),
                ("Lvl", 40.),
                ("Class", 115.),
                ("Leadership", 115.),
            ]
        } else if cells >= 380. {
            vec![
                ("Grp", 52.),
                ("Player name", cells - 207.),
                ("Lvl", 40.),
                ("Class", 115.),
            ]
        } else {
            vec![
                ("Grp", 52_f32.min(cells)),
                ("Player name", (cells - 52.).max(0.)),
            ]
        };
        let mut x = list.x + 4.;
        for (label, width) in &columns {
            self.text(
                Rect::new(x, list.y + 3., (width - 4.).max(0.), 18.),
                *label,
                GOLD,
                false,
            );
            x += width;
        }
        let max_scroll = raid.members.len().saturating_sub(rows.max(1));
        let start = raid.scroll.min(max_scroll);
        for (index, member) in raid.members.iter().skip(start).take(rows).enumerate() {
            let bounds = Rect::new(
                list.x + 3.,
                list.y + HEADER_HEIGHT + index as f32 * ROW_HEIGHT,
                cells,
                ROW_HEIGHT,
            );
            let selected = raid
                .selected_member
                .as_ref()
                .is_some_and(|name| name.eq_ignore_ascii_case(&member.name));
            if selected {
                self.fill(bounds, [65, 65, 49, 255]);
            } else if index % 2 == 1 {
                self.fill(bounds, [25, 28, 31, 170]);
            }
            let values = [
                group_label(member.group),
                member.name.clone(),
                level_label(member.level),
                class_label(member.class),
                leadership(raid, member).into(),
            ];
            let mut x = bounds.x + 1.;
            for ((_, width), value) in columns.iter().zip(values) {
                self.text(
                    Rect::new(x, bounds.y + 1., (width - 4.).max(0.), 18.),
                    value,
                    WHITE,
                    false,
                );
                x += width;
            }
            self.hit(
                format!("raid:{}:member:{}", raid.revision, member.name),
                "RaidMember",
                bounds,
                Some(member_details(raid, member)),
            );
        }
        if raid.members.is_empty() {
            self.text(
                Rect::new(
                    list.x + 8.,
                    list.y + HEADER_HEIGHT + 8.,
                    (list.width - 35.).max(0.),
                    40.,
                ),
                "No raid members have been received.",
                MUTED,
                true,
            );
        }
        for (direction, label, y, enabled) in [
            (-1, "↑", list.y + 2., start > 0),
            (1, "↓", list.bottom() - 22., start < max_scroll),
        ] {
            self.button_enabled(
                "RAID_InviteButton",
                &format!("raid:{}:scroll:{direction}", raid.revision),
                Rect::new(list.right() - 20., y, 18., 20.),
                label,
                false,
                enabled,
            );
        }
        if max_scroll > 0 && rows > 0 {
            let track = (list.height - 48.).max(0.);
            let thumb = (track * rows as f32 / raid.members.len() as f32)
                .max(10.)
                .min(track);
            self.fill(
                Rect::new(
                    list.right() - 14.,
                    list.y + 24. + (track - thumb) * start as f32 / max_scroll as f32,
                    6.,
                    thumb,
                ),
                [140, 130, 99, 255],
            );
        }
        let details = raid
            .selected_member
            .as_ref()
            .and_then(|name| {
                raid.members
                    .iter()
                    .find(|member| member.name.eq_ignore_ascii_case(name))
            })
            .map(|member| member_details(raid, member))
            .unwrap_or_else(|| {
                "Select a member for details. Presence is not included in the raid roster.".into()
            });
        self.text(
            Rect::new(
                rect.x + 12.,
                rect.bottom() - 86.,
                (rect.width - 24.).max(0.),
                44.,
            ),
            details,
            MUTED,
            true,
        );
    }

    fn raid_information(&mut self, rect: Rect, top: f32, raid: &UiRaid) {
        self.text(
            Rect::new(rect.x + 12., top, (rect.width - 24.).max(0.), 19.),
            "Message of the day",
            GOLD,
            false,
        );
        let mut bindings = UiBindings::default();
        let motd = bindings.widget_mut("RAID_MOTDViewer");
        motd.rect = Some(Rect::new(
            rect.x + 10.,
            top + 25.,
            (rect.width - 20.).max(0.),
            (rect.bottom() - top - 69.).max(0.),
        ));
        motd.text = Some(
            match raid.motd.as_deref() {
                Some("") => "The raid message is empty.",
                Some(text) => text,
                None => "No raid message has been received.",
            }
            .into(),
        );
        motd.scroll_rows = Some(raid.motd_scroll_rows);
        motd.scroll_id = Some(raid_motd_scroll_id(raid.revision));
        let hit_start = self.frame.hit_targets.len();
        self.widget("RAID_MOTDViewer", &bindings);
        // The XML name is shared across every frame; capture the displayed
        // revision in scroll-arrow hits before they reach live interaction.
        for hit in &mut self.frame.hit_targets[hit_start..] {
            if let Some(rows) = hit.item.strip_prefix("RAID_MOTDViewer:scroll:") {
                hit.item = format!("raid:{}:motd:{rows}", raid.revision);
                hit.screen_id = hit.item.clone();
            }
        }
    }
}

fn group_label(group: Option<u8>) -> String {
    group
        .filter(|group| *group < 12)
        .map_or_else(|| "—".into(), |group| (group + 1).to_string())
}

fn level_label(level: u8) -> String {
    if level == 0 {
        "?".into()
    } else {
        level.to_string()
    }
}

fn class_label(class: u8) -> String {
    [
        "Unknown",
        "Warrior",
        "Cleric",
        "Paladin",
        "Ranger",
        "Shadowknight",
        "Druid",
        "Monk",
        "Bard",
        "Rogue",
        "Shaman",
        "Necromancer",
        "Wizard",
        "Magician",
        "Enchanter",
        "Beastlord",
        "Berserker",
    ]
    .get(class as usize)
    .map_or_else(|| format!("Class {class}"), |name| (*name).into())
}

fn leadership(raid: &UiRaid, member: &UiRaidMember) -> &'static str {
    if raid
        .leader
        .as_ref()
        .is_some_and(|name| name.eq_ignore_ascii_case(&member.name))
    {
        "Raid leader"
    } else if member.group_leader {
        "Group leader"
    } else {
        ""
    }
}

fn member_details(raid: &UiRaid, member: &UiRaidMember) -> String {
    let group = member.group.filter(|group| *group < 12).map_or_else(
        || "Ungrouped".into(),
        |group| format!("Group {}", group + 1),
    );
    let role = leadership(raid, member);
    format!(
        "{} · Level {} {} · {group}{}",
        member.name,
        level_label(member.level),
        class_label(member.class),
        if role.is_empty() {
            String::new()
        } else {
            format!(" · {role}")
        }
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        gameplay_ui::{UiAction, WindowStack, valid_window_id},
        hud::{Hud, HudState},
    };
    use openeq_ui::{DrawCommand, UiDocument};

    fn hit(item: &str) -> HitTarget {
        HitTarget {
            item: item.into(),
            screen_id: item.into(),
            window_id: Some("raid".into()),
            kind: "Button".into(),
            rect: Rect::new(0., 0., 20., 20.),
            enabled: true,
            tooltip: None,
        }
    }

    fn fixture() -> UiRaid {
        UiRaid {
            revision: 41,
            status: "Raid membership received".into(),
            leader: Some("Member00".into()),
            locked: Some(false),
            members: (0..72)
                .map(|index| UiRaidMember {
                    name: format!("Member{index:02}"),
                    level: 65,
                    class: (index % 16 + 1) as u8,
                    group: (index < 66).then_some((index / 6) as u8),
                    group_leader: index % 6 == 0,
                })
                .collect(),
            selected_member: Some("Member00".into()),
            can_invite: true,
            can_leave: true,
            ..Default::default()
        }
    }

    fn minimal_hud() -> Hud {
        Hud {
            ui: UiDocument::default(),
            initial_bindings: UiBindings::default(),
        }
    }

    #[test]
    fn actions_preserve_revision_invitation_and_stable_member_keys() {
        for (item, kind) in [
            (
                "raid:41:accept:93",
                RaidActionKind::AcceptInvite { token: 93 },
            ),
            (
                "raid:41:dismiss:93",
                RaidActionKind::DismissInvite { token: 93 },
            ),
            (
                "raid:41:member:Member72",
                RaidActionKind::SelectMember {
                    name: "Member72".into(),
                },
            ),
            ("raid:41:motd:-1", RaidActionKind::ScrollMotd { rows: -1 }),
        ] {
            assert_eq!(
                UiAction::from_hit(&hit(item)),
                Some(UiAction::Raid(RaidAction { revision: 41, kind }))
            );
        }
        for item in [
            "raid:41:scroll:2",
            "raid:41:motd:0",
            "raid:41:member:",
            "raid:41:accept:-1",
            "raid:bad:leave",
            "social:raid:leave",
        ] {
            assert!(RaidAction::from_hit(&hit(item)).is_none(), "{item}");
        }
        let mut stale_owner = hit("raid:41:leave");
        stale_owner.window_id = Some("group".into());
        assert!(RaidAction::from_hit(&stale_owner).is_none());
        let mut disabled = hit("raid:41:accept:93");
        disabled.enabled = false;
        assert!(RaidAction::from_hit(&disabled).is_none());
    }

    #[test]
    fn roster_work_is_bounded_and_scroll_keeps_member_identity() {
        let hud = minimal_hud();
        let mut state = GameHudState {
            raid: Some(fixture()),
            ..Default::default()
        };
        for (viewport, scroll) in [
            ([900, 650], 0),
            ([900, 650], usize::MAX),
            ([350, 400], usize::MAX),
        ] {
            state.raid.as_mut().unwrap().scroll = scroll;
            let frame = hud.gameplay_frame(viewport, &HudState::default(), &state);
            let rows: Vec<_> = frame
                .hit_targets
                .iter()
                .filter(|hit| hit.kind == "RaidMember")
                .collect();
            assert_eq!(rows.len(), raid_visible_rows(viewport[1], false));
            let expected = if scroll == 0 { "Member00" } else { "Member71" };
            assert!(rows.iter().any(|hit| hit.item.ends_with(expected)));
            for row in rows {
                assert_eq!(row.window_id.as_deref(), Some("raid"));
                assert!(matches!(
                    RaidAction::from_hit(row),
                    Some(RaidAction {
                        revision: 41,
                        kind: RaidActionKind::SelectMember { .. }
                    })
                ));
            }
        }
        assert!(state.raid.as_ref().unwrap().members.len() == 72);
        assert_eq!(
            state.raid.as_ref().unwrap().selected_member.as_deref(),
            Some("Member00")
        );
    }

    #[test]
    fn pending_invitation_and_capability_controls_are_disabled() {
        let hud = minimal_hud();
        let mut raid = fixture();
        raid.can_invite = false;
        raid.can_leave = false;
        raid.invitation = Some(UiRaidInvitation {
            token: 92,
            inviter: "Scout".into(),
            invitee: "Adventurer".into(),
            pending: true,
        });
        let state = GameHudState {
            raid: Some(raid),
            ..Default::default()
        };
        let frame = hud.gameplay_frame([900, 650], &HudState::default(), &state);
        for item in [
            "raid:41:accept:92",
            "raid:41:dismiss:92",
            "raid:41:invite",
            "raid:41:leave",
        ] {
            let control = frame
                .hit_targets
                .iter()
                .find(|hit| hit.item == item)
                .unwrap();
            assert!(!control.enabled, "{item}");
            assert!(RaidAction::from_hit(control).is_none());
        }
        assert_eq!(
            frame
                .hit_targets
                .iter()
                .find(|hit| hit.item == "raid:41:leave")
                .unwrap()
                .tooltip
                .as_deref(),
            Some("Leave this raid yourself")
        );
    }

    #[test]
    fn page_visibility_and_window_stack_do_not_change_membership() {
        let hud = minimal_hud();
        let mut state = GameHudState {
            raid: Some(fixture()),
            inventory_open: true,
            ..Default::default()
        };
        state.window_positions.insert("raid".into(), [0., 0.]);
        state.window_positions.insert("inventory".into(), [0., 0.]);
        let mut stack = WindowStack::default();
        let frame = hud.gameplay_frame_with_windows(
            [900, 650],
            &HudState::default(),
            &state,
            vec![],
            &mut stack,
        );
        let member = frame
            .hit_targets
            .iter()
            .find(|hit| hit.item == "raid:41:member:Member00")
            .unwrap();
        let point = [member.rect.x + 4., member.rect.y + 4.];
        assert_eq!(
            frame.hit_test(point).unwrap().window_id.as_deref(),
            Some("raid")
        );
        assert!(valid_window_id("raid"));
        stack.raise("inventory");
        let covered = hud.gameplay_frame_with_windows(
            [900, 650],
            &HudState::default(),
            &state,
            vec![],
            &mut stack,
        );
        assert_eq!(
            covered.hit_test(point).unwrap().window_id.as_deref(),
            Some("inventory")
        );
        state.raid.as_mut().unwrap().page = UiRaidPage::Information;
        let information = hud.gameplay_frame([900, 650], &HudState::default(), &state);
        assert!(
            !information
                .hit_targets
                .iter()
                .any(|hit| hit.kind == "RaidMember")
        );
        state.raid = None;
        let closed = hud.gameplay_frame([900, 650], &HudState::default(), &state);
        assert!(
            !closed
                .hit_targets
                .iter()
                .any(|hit| hit.window_id.as_deref() == Some("raid"))
        );
    }

    #[test]
    #[ignore = "requires original UI assets and GPU; optional OPENEQ_UI_CAPTURE_DIR"]
    fn original_raid_skin_rows_invitations_and_motd_at_both_scales() {
        let base = std::env::var_os("EQ_UI_DIR")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| {
                std::path::PathBuf::from(std::env::var_os("HOME").unwrap()).join("EverQuest")
            });
        let hud = Hud::load(base).unwrap();
        let resources = HudState {
            character: "Adventurer".into(),
            hp: 1.,
            ..Default::default()
        };
        for scale in [1, 2] {
            let mut renderer =
                openeq_render::Renderer::new_headless(900 * scale, 650 * scale).unwrap();
            for mode in [
                "members",
                "invitation",
                "pending",
                "information-top",
                "information-bottom",
                "narrow-members",
                "narrow-invitation",
            ] {
                let viewport = if mode.starts_with("narrow") {
                    [350, 400]
                } else {
                    [900, 650]
                };
                renderer.resize(viewport[0] * scale, viewport[1] * scale);
                let mut raid = fixture();
                if mode.starts_with("narrow") {
                    raid.members[0].name = "ÉowynAnUnusuallyLongMemberNameForClipping".into();
                    raid.selected_member = Some(raid.members[0].name.clone());
                    raid.leader = Some(raid.members[0].name.clone());
                }
                if matches!(mode, "invitation" | "pending" | "narrow-invitation") {
                    raid.invitation = Some(UiRaidInvitation {
                        token: 92,
                        inviter: "Scout".into(),
                        invitee: "Adventurer".into(),
                        pending: mode == "pending",
                    });
                    raid.can_invite = false;
                    raid.can_leave = false;
                }
                if mode.starts_with("information") {
                    raid.page = UiRaidPage::Information;
                    raid.motd = Some((1..=45).map(|line| format!("{line}. Gather at the entrance and wait for the raid leader. Group assignments and instructions will appear here.\n")).collect());
                    if mode == "information-bottom" {
                        raid.motd_scroll_rows = usize::MAX;
                    }
                }
                let state = GameHudState {
                    raid: Some(raid),
                    ..Default::default()
                };
                let frame = hud.gameplay_frame(viewport, &resources, &state);
                assert!(frame.warnings.is_empty(), "{:?}", frame.warnings);
                let close = frame
                    .hit_targets
                    .iter()
                    .find(|hit| hit.item == "game:close:raid")
                    .unwrap();
                assert_eq!(
                    frame
                        .hit_test([close.rect.x + 3., close.rect.y + 3.])
                        .unwrap()
                        .item,
                    close.item
                );
                let owned: Vec<_> = frame
                    .hit_targets
                    .iter()
                    .filter(|hit| hit.item.starts_with("raid:"))
                    .collect();
                assert!(
                    owned
                        .iter()
                        .all(|hit| hit.window_id.as_deref() == Some("raid"))
                );
                renderer.set_ui_scaled(&frame, scale as f32);
                if mode.starts_with("information") {
                    assert!(!frame.hit_targets.iter().any(|hit| hit.kind == "RaidMember"));
                    let arrow = owned
                        .iter()
                        .find(|hit| hit.item.ends_with("motd:1"))
                        .unwrap();
                    assert_eq!(
                        RaidAction::from_hit(arrow),
                        Some(RaidAction {
                            revision: 41,
                            kind: RaidActionKind::ScrollMotd { rows: 1 }
                        })
                    );
                    let metrics = renderer
                        .ui_text_scroll_metrics()
                        .iter()
                        .find(|metrics| metrics.id == raid_motd_scroll_id(41))
                        .unwrap();
                    assert!(metrics.max_scroll() > 0);
                    assert_eq!(
                        metrics.first_row,
                        if mode == "information-bottom" {
                            metrics.max_scroll()
                        } else {
                            0
                        }
                    );
                    let thumb = frame
                        .commands
                        .iter()
                        .find_map(|command| match command {
                            DrawCommand::TextArea { thumb, .. } => thumb.as_ref(),
                            _ => None,
                        })
                        .unwrap();
                    assert!(thumb.images.iter().all(Option::is_some));
                } else {
                    assert_eq!(
                        frame
                            .hit_targets
                            .iter()
                            .filter(|hit| hit.kind == "RaidMember")
                            .count(),
                        raid_visible_rows(
                            viewport[1],
                            state.raid.as_ref().unwrap().invitation.is_some()
                        )
                    );
                }
                renderer.render_ui();
                if let Some(directory) = std::env::var_os("OPENEQ_UI_CAPTURE_DIR") {
                    std::fs::create_dir_all(&directory).unwrap();
                    let (width, height, pixels) = renderer.read_rgba().unwrap();
                    image::save_buffer(
                        std::path::PathBuf::from(directory)
                            .join(format!("raid-{mode}-{scale}x.png")),
                        &pixels,
                        width,
                        height,
                        image::ColorType::Rgba8,
                    )
                    .unwrap();
                }
            }
        }
    }
}
