//! Receive-only guild presentation. Every action changes local presentation;
//! membership, presence, ranks, notes and messages come from verified state.
use crate::gameplay_ui::{GOLD, GameHudState, MUTED, Painter, WHITE, position};
use openeq_ui::{HitTarget, Rect, UiBindings};
use std::sync::Arc;

pub const GUILD_VISIBLE_ROWS: usize = 12;
const WINDOW_HEIGHT: f32 = 560.;
const LIST_TOP: f32 = 156.;
const HEADER_HEIGHT: f32 = 24.;
const ROW_HEIGHT: f32 = 20.;
const TEXT_TEMPLATE: &str = "GT_MOTDViewer";

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum UiGuildPage {
    #[default]
    Members,
    Information,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum UiGuildPresence {
    #[default]
    Unknown,
    ReportedOffline,
    ReportedOnline {
        location: Option<String>,
    },
}

#[derive(Clone, Debug, Default)]
pub struct UiGuildMember {
    /// Stable member key within the verified guild/session, never a row index.
    pub name: String,
    pub level: Option<u32>,
    pub class: Option<u32>,
    /// Adapter-supplied rank name, or its numeric "Rank N" fallback.
    pub rank: String,
    pub alt: Option<bool>,
    pub banker: Option<bool>,
    pub presence: UiGuildPresence,
    /// Source-qualified display text; absence never implies a fabricated date.
    pub last_seen: Option<String>,
    pub public_note: Option<String>,
}

#[derive(Clone, Debug, Default)]
pub struct UiGuildMotd {
    pub text: String,
    pub author: Option<String>,
}

#[derive(Clone, Debug, Default)]
pub struct UiGuild {
    /// Must remain distinct across membership/session changes and revisions.
    pub revision: u64,
    /// The adapter distinguishes unknown, confirmed-none and received state.
    pub status: String,
    pub name: Option<String>,
    /// Full validated roster, sorted by the adapter; drawing filters it locally.
    pub members: Arc<[UiGuildMember]>,
    /// None is not received; Some with empty text is a confirmed empty message.
    pub motd: Option<UiGuildMotd>,
    pub page: UiGuildPage,
    pub show_offline: bool,
    pub hide_alts: bool,
    pub scroll: usize,
    pub selected_member: Option<String>,
    pub motd_scroll_rows: usize,
    pub note_scroll_rows: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GuildAction {
    pub revision: u64,
    pub kind: GuildActionKind,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GuildActionKind {
    SelectMember { name: String },
    SetShowOffline(bool),
    SetHideAlts(bool),
    Scroll { rows: i32 },
    Page(UiGuildPage),
    ScrollMotd { rows: i32 },
    ScrollNote { name: String, rows: i32 },
}

impl GuildAction {
    pub fn from_hit(hit: &HitTarget) -> Option<Self> {
        if !hit.enabled || hit.window_id.as_deref() != Some("guild") {
            return None;
        }
        let (revision, action) = hit.item.strip_prefix("guild:")?.split_once(':')?;
        let revision = revision.parse().ok()?;
        let kind = match action {
            "members" => GuildActionKind::Page(UiGuildPage::Members),
            "information" => GuildActionKind::Page(UiGuildPage::Information),
            _ => {
                let (action, value) = action.split_once(':')?;
                match action {
                    "member" if !value.is_empty() => {
                        GuildActionKind::SelectMember { name: value.into() }
                    }
                    "offline" => GuildActionKind::SetShowOffline(parse_bool(value)?),
                    "alts" => GuildActionKind::SetHideAlts(parse_bool(value)?),
                    "scroll" => GuildActionKind::Scroll {
                        rows: parse_rows(value)?,
                    },
                    "motd" => GuildActionKind::ScrollMotd {
                        rows: parse_rows(value)?,
                    },
                    "note" => {
                        let (rows, name) = value.split_once(':')?;
                        if name.is_empty() {
                            return None;
                        }
                        GuildActionKind::ScrollNote {
                            name: name.into(),
                            rows: parse_rows(rows)?,
                        }
                    }
                    _ => return None,
                }
            }
        };
        Some(Self { revision, kind })
    }
}

fn parse_bool(value: &str) -> Option<bool> {
    match value {
        "0" => Some(false),
        "1" => Some(true),
        _ => None,
    }
}
fn parse_rows(value: &str) -> Option<i32> {
    match value {
        "-1" => Some(-1),
        "1" => Some(1),
        _ => None,
    }
}

/// Unknown presence and alt state remain visible under either local filter.
pub fn guild_member_visible(member: &UiGuildMember, show_offline: bool, hide_alts: bool) -> bool {
    (show_offline || member.presence != UiGuildPresence::ReportedOffline)
        && (!hide_alts || member.alt != Some(true))
}

pub fn guild_visible_rows(viewport_height: u32) -> usize {
    let height = (viewport_height as f32).min(WINDOW_HEIGHT);
    let footer = (height * 0.32).min(176.);
    (((height - LIST_TOP - HEADER_HEIGHT - footer).max(0.) / ROW_HEIGHT) as usize)
        .min(GUILD_VISIBLE_ROWS)
}

pub fn guild_motd_scroll_id(revision: u64) -> String {
    format!("guild:motd:{revision}")
}
pub fn guild_note_scroll_id(revision: u64, name: &str) -> String {
    format!("guild:note:{revision}:{name}")
}

/// Reads the displayed note identity for wheel routing on either its body or
/// arrows. A newly selected member must not inherit an older viewport's wheel.
pub fn guild_note_hit_identity(hit: &HitTarget) -> Option<(u64, &str)> {
    if !hit.enabled || hit.window_id.as_deref() != Some("guild") || hit.kind != "GuildNote" {
        return None;
    }
    let (revision, action) = hit.item.strip_prefix("guild:")?.split_once(':')?;
    let revision = revision.parse().ok()?;
    let name = if let Some(name) = action.strip_prefix("note_body:") {
        name
    } else {
        let (rows, name) = action.strip_prefix("note:")?.split_once(':')?;
        parse_rows(rows)?;
        name
    };
    (!name.is_empty()).then_some((revision, name))
}

impl Painter<'_> {
    pub(crate) fn guild(&mut self, state: &GameHudState, guild: &UiGuild) {
        let rect = position(
            state,
            "guild",
            Rect::new(self.screen.width * 0.5 - 340., 24., 680., WINDOW_HEIGHT),
            self.screen,
        );
        self.shell("GuildManagementWnd", "guild", rect, "Guild", true);
        self.text(
            Rect::new(rect.x + 12., rect.y + 29., (rect.width - 24.).max(0.), 20.),
            guild
                .name
                .as_deref()
                .unwrap_or("Guild identity unavailable"),
            GOLD,
            false,
        );
        self.text(
            Rect::new(rect.x + 12., rect.y + 53., (rect.width - 24.).max(0.), 18.),
            if guild.status.is_empty() {
                "Waiting for guild information"
            } else {
                &guild.status
            },
            MUTED,
            false,
        );
        for (index, (id, label, page)) in [
            ("members", "Members", UiGuildPage::Members),
            ("information", "Information", UiGuildPage::Information),
        ]
        .into_iter()
        .enumerate()
        {
            self.button(
                "RAID_InviteButton",
                &format!("guild:{}:{id}", guild.revision),
                Rect::new(rect.x + 12. + index as f32 * 124., rect.y + 78., 116., 24.),
                label,
                guild.page == page,
            );
        }
        match guild.page {
            UiGuildPage::Members => self.guild_members(rect, guild),
            UiGuildPage::Information => self.guild_information(rect, guild),
        }
    }

    fn guild_members(&mut self, rect: Rect, guild: &UiGuild) {
        for (index, (template, action, label, checked)) in [
            (
                "GT_ShowOfflineButton",
                "offline",
                "Show offline",
                guild.show_offline,
            ),
            ("GT_HideAltButton", "alts", "Hide alts", guild.hide_alts),
        ]
        .into_iter()
        .enumerate()
        {
            let bounds = Rect::new(rect.x + 12. + index as f32 * 143., rect.y + 111., 136., 20.);
            let mut bindings = UiBindings::default();
            let widget = bindings.widget_mut(template);
            widget.rect = Some(Rect::new(bounds.x, bounds.y + 2., 16., 16.));
            widget.text = Some(String::new());
            widget.checked = checked;
            widget.hovered = self.pointer.is_some_and(|point| bounds.contains(point));
            self.widget(template, &bindings);
            self.text(
                Rect::new(bounds.x + 23., bounds.y + 1., bounds.width - 23., 18.),
                label,
                WHITE,
                false,
            );
            self.hit(
                format!("guild:{}:{action}:{}", guild.revision, u8::from(!checked)),
                "GuildFilter",
                bounds,
                None,
            );
        }
        let visible = |member: &&UiGuildMember| {
            guild_member_visible(member, guild.show_offline, guild.hide_alts)
        };
        let count = guild.members.iter().filter(visible).count();
        self.text(
            Rect::new(rect.x + 12., rect.y + 135., (rect.width - 24.).max(0.), 18.),
            format!(
                "Showing {count} of {} received members",
                guild.members.len()
            ),
            MUTED,
            false,
        );
        let rows = guild_visible_rows(self.screen.height as u32);
        let list = Rect::new(
            rect.x + 10.,
            rect.y + LIST_TOP,
            (rect.width - 20.).max(0.),
            HEADER_HEIGHT + rows as f32 * ROW_HEIGHT,
        );
        let mut bindings = UiBindings::default();
        bindings.widget_mut("GT_MemberList").rect = Some(list);
        self.widget("GT_MemberList", &bindings);
        self.hit(
            format!("guild:{}:roster", guild.revision),
            "GuildRoster",
            list,
            None,
        );
        let cells = (list.width - 24.).max(0.);
        let columns = if cells >= 570. {
            vec![
                ("Name", cells - 415., 0),
                ("Lvl", 38., 1),
                ("Class", 100., 2),
                ("Rank", 110., 3),
                ("Last reported", 167., 4),
            ]
        } else if cells >= 380. {
            vec![
                ("Name", cells - 270., 0),
                ("Lvl", 38., 1),
                ("Rank", 102., 3),
                ("Last reported", 130., 4),
            ]
        } else {
            vec![
                ("Name", (cells - 125.).max(0.), 0),
                ("Last reported", 125_f32.min(cells), 4),
            ]
        };
        let mut x = list.x + 4.;
        for (label, width, _) in &columns {
            self.text(
                Rect::new(x, list.y + 3., (width - 4.).max(0.), 18.),
                *label,
                GOLD,
                false,
            );
            x += width;
        }
        let max_scroll = count.saturating_sub(rows.max(1));
        let start = guild.scroll.min(max_scroll);
        for (index, member) in guild
            .members
            .iter()
            .filter(visible)
            .skip(start)
            .take(rows)
            .enumerate()
        {
            let bounds = Rect::new(
                list.x + 3.,
                list.y + HEADER_HEIGHT + index as f32 * ROW_HEIGHT,
                cells,
                ROW_HEIGHT,
            );
            let selected = guild
                .selected_member
                .as_ref()
                .is_some_and(|name| name.eq_ignore_ascii_case(&member.name));
            if selected {
                self.fill(bounds, [65, 65, 49, 255]);
            } else if index % 2 == 1 {
                self.fill(bounds, [25, 28, 31, 170]);
            }
            let values = [
                member.name.clone(),
                member
                    .level
                    .map_or_else(|| "?".into(), |level| level.to_string()),
                class_label(member.class),
                rank_label(member).into(),
                presence_label(&member.presence),
            ];
            let mut x = bounds.x + 1.;
            for (_, width, value) in &columns {
                self.text(
                    Rect::new(x, bounds.y + 1., (width - 4.).max(0.), 18.),
                    &values[*value],
                    WHITE,
                    false,
                );
                x += width;
            }
            self.hit(
                format!("guild:{}:member:{}", guild.revision, member.name),
                "GuildMember",
                bounds,
                Some(member_details(member)),
            );
        }
        if count == 0 && rows > 0 {
            self.text(
                Rect::new(
                    list.x + 8.,
                    list.y + HEADER_HEIGHT + 6.,
                    (list.width - 35.).max(0.),
                    (list.height - HEADER_HEIGHT - 6.).max(0.),
                ),
                if guild.members.is_empty() {
                    "No members to display."
                } else {
                    "No members match these filters."
                },
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
                &format!("guild:{}:scroll:{direction}", guild.revision),
                Rect::new(list.right() - 20., y, 18., 20.),
                label,
                false,
                enabled,
            );
            if let Some(hit) = self.frame.hit_targets.last_mut() {
                hit.kind = "GuildRoster".into();
            }
        }
        if max_scroll > 0 && rows > 0 {
            let track = (list.height - 48.).max(0.);
            let thumb = (track * rows as f32 / count as f32).max(10.).min(track);
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
        self.text(
            Rect::new(
                rect.x + 12.,
                list.bottom() + 8.,
                (rect.width - 24.).max(0.),
                18.,
            ),
            "Member details and public note",
            GOLD,
            false,
        );
        let note = Rect::new(
            rect.x + 10.,
            list.bottom() + 32.,
            (rect.width - 20.).max(0.),
            (rect.bottom() - list.bottom() - 42.).max(0.),
        );
        let selected = guild.selected_member.as_ref().and_then(|name| {
            guild
                .members
                .iter()
                .filter(visible)
                .find(|member| member.name.eq_ignore_ascii_case(name))
        });
        if let Some(member) = selected {
            self.guild_text_area(note, guild, Some(&member.name), &member_details(member));
        } else {
            self.text(
                note,
                "Select a visible member to read their details and public note.",
                MUTED,
                true,
            );
        }
    }

    fn guild_information(&mut self, rect: Rect, guild: &UiGuild) {
        self.text(
            Rect::new(rect.x + 12., rect.y + 112., (rect.width - 24.).max(0.), 18.),
            "Message of the day",
            GOLD,
            false,
        );
        let author = guild
            .motd
            .as_ref()
            .and_then(|motd| motd.author.as_deref())
            .filter(|author| !author.is_empty());
        self.text(
            Rect::new(rect.x + 12., rect.y + 137., (rect.width - 24.).max(0.), 18.),
            author.map_or_else(
                || "Author unavailable".into(),
                |author| format!("From {author}"),
            ),
            MUTED,
            false,
        );
        let text = match guild.motd.as_ref() {
            None => "No guild message has been received.",
            Some(motd) if motd.text.is_empty() => "The guild message is empty.",
            Some(motd) => &motd.text,
        };
        self.guild_text_area(
            Rect::new(
                rect.x + 10.,
                rect.y + 163.,
                (rect.width - 20.).max(0.),
                (rect.height - 173.).max(0.),
            ),
            guild,
            None,
            text,
        );
    }

    fn guild_text_area(&mut self, bounds: Rect, guild: &UiGuild, member: Option<&str>, text: &str) {
        if bounds.is_empty() {
            return;
        }
        let mut bindings = UiBindings::default();
        let widget = bindings.widget_mut(TEXT_TEMPLATE);
        widget.rect = Some(bounds);
        widget.text = Some(text.into());
        widget.scroll_rows = Some(if member.is_some() {
            guild.note_scroll_rows
        } else {
            guild.motd_scroll_rows
        });
        widget.scroll_id = Some(member.map_or_else(
            || guild_motd_scroll_id(guild.revision),
            |name| guild_note_scroll_id(guild.revision, name),
        ));
        let start = self.frame.hit_targets.len();
        self.widget(TEXT_TEMPLATE, &bindings);
        // Reuse original inner text/scroll art, but keep each viewport's action
        // identity separate from the XML item and its shared MOTDViewer ScreenID.
        for hit in &mut self.frame.hit_targets[start..] {
            let rows = hit.item.strip_prefix("GT_MOTDViewer:scroll:");
            hit.item = match (member, rows) {
                (Some(name), Some(rows)) => format!("guild:{}:note:{rows}:{name}", guild.revision),
                (Some(name), None) => format!("guild:{}:note_body:{name}", guild.revision),
                (None, Some(rows)) => format!("guild:{}:motd:{rows}", guild.revision),
                (None, None) => format!("guild:{}:motd_body", guild.revision),
            };
            hit.screen_id = hit.item.clone();
            hit.kind = if member.is_some() {
                "GuildNote"
            } else {
                "GuildMotd"
            }
            .into();
            hit.tooltip = Some(
                if member.is_some() {
                    "Received member details and public note"
                } else {
                    "Received guild message of the day"
                }
                .into(),
            );
        }
    }
}

fn class_label(class: Option<u32>) -> String {
    const NAMES: [&str; 16] = [
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
    ];
    match class {
        Some(class @ 1..=16) => NAMES[(class - 1) as usize].into(),
        Some(class) => format!("Class {class}"),
        None => "Class unavailable".into(),
    }
}

fn rank_label(member: &UiGuildMember) -> &str {
    if member.rank.is_empty() {
        "Rank unavailable"
    } else {
        &member.rank
    }
}

fn presence_label(presence: &UiGuildPresence) -> String {
    match presence {
        UiGuildPresence::Unknown => "Unknown".into(),
        UiGuildPresence::ReportedOffline => "Offline".into(),
        UiGuildPresence::ReportedOnline {
            location: Some(location),
        } if !location.is_empty() => format!("Online: {location}"),
        UiGuildPresence::ReportedOnline { .. } => "Online; zone unknown".into(),
    }
}

fn member_details(member: &UiGuildMember) -> String {
    let flag = |value| match value {
        Some(true) => "Yes",
        Some(false) => "No",
        None => "Unknown",
    };
    let note = match member.public_note.as_deref() {
        None => "Not received.",
        Some("") => "Empty.",
        Some(note) => note,
    };
    format!(
        "{}\nLevel {} · {} · {}\nLast reported: {}\nLast seen: {}\nAlt: {} · Banker: {}\n\nPublic note: {}",
        member.name,
        member
            .level
            .map_or_else(|| "unavailable".into(), |level| level.to_string()),
        class_label(member.class),
        rank_label(member),
        presence_label(&member.presence),
        member.last_seen.as_deref().unwrap_or("Unavailable"),
        flag(member.alt),
        flag(member.banker),
        note
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

    fn fixture(count: usize) -> UiGuild {
        UiGuild {
            revision: 73,
            status: "Received guild roster · Presence may have changed".into(),
            name: Some("Keepers of the Lost Library".into()),
            members: (0..count)
                .map(|index| UiGuildMember {
                    name: format!("Member{index:05}"),
                    level: Some(65),
                    class: Some((index % 16 + 1) as u32),
                    rank: "Member".into(),
                    alt: Some(index % 3 == 1),
                    banker: Some(index == 0),
                    presence: match index % 3 {
                        0 => UiGuildPresence::ReportedOffline,
                        1 => UiGuildPresence::ReportedOnline {
                            location: Some("Plane of Knowledge".into()),
                        },
                        _ => UiGuildPresence::Unknown,
                    },
                    last_seen: (index % 3 == 0).then(|| "2026-09-28 23:15 UTC".into()),
                    public_note: Some(format!("Received public note for member {index}.")),
                })
                .collect::<Vec<_>>()
                .into(),
            motd: Some(UiGuildMotd {
                text: "Welcome to the guild!".into(),
                author: Some("Archivist".into()),
            }),
            show_offline: true,
            selected_member: (count > 0).then(|| "Member00000".into()),
            ..Default::default()
        }
    }

    fn minimal_hud() -> Hud {
        Hud {
            ui: UiDocument::from_xml(r#"<XML><STMLbox item="GT_MOTDViewer"><Style_VScroll>true</Style_VScroll></STMLbox></XML>"#).unwrap(),
            initial_bindings: UiBindings::default(),
        }
    }

    fn hit(item: &str) -> HitTarget {
        HitTarget {
            item: item.into(),
            screen_id: item.into(),
            window_id: Some("guild".into()),
            kind: "GuildNote".into(),
            rect: Rect::new(0., 0., 20., 20.),
            enabled: true,
            tooltip: None,
        }
    }

    #[test]
    fn filters_only_hide_known_offline_members_and_known_alts() {
        let mut unknown = UiGuildMember::default();
        for show_offline in [false, true] {
            for hide_alts in [false, true] {
                assert!(guild_member_visible(&unknown, show_offline, hide_alts));
            }
        }
        unknown.presence = UiGuildPresence::ReportedOffline;
        assert!(!guild_member_visible(&unknown, false, true));
        assert!(guild_member_visible(&unknown, true, true));
        unknown.presence = UiGuildPresence::ReportedOnline { location: None };
        unknown.alt = Some(true);
        assert!(!guild_member_visible(&unknown, true, true));
        assert!(guild_member_visible(&unknown, false, false));
    }

    #[test]
    fn local_actions_and_note_wheel_keep_displayed_identity() {
        for (item, kind) in [
            (
                "guild:73:member:Member00001",
                GuildActionKind::SelectMember {
                    name: "Member00001".into(),
                },
            ),
            ("guild:73:offline:1", GuildActionKind::SetShowOffline(true)),
            ("guild:73:alts:0", GuildActionKind::SetHideAlts(false)),
            (
                "guild:73:note:-1:Member00001",
                GuildActionKind::ScrollNote {
                    name: "Member00001".into(),
                    rows: -1,
                },
            ),
            ("guild:73:motd:1", GuildActionKind::ScrollMotd { rows: 1 }),
        ] {
            assert_eq!(
                UiAction::from_hit(&hit(item)),
                Some(UiAction::Guild(GuildAction { revision: 73, kind }))
            );
        }
        for item in [
            "guild:73:invite",
            "guild:73:leave",
            "guild:73:refresh",
            "guild:73:member:",
            "guild:73:note:1:",
            "guild:73:scroll:999",
            "guild:73:offline:true",
            "guild:x:members",
        ] {
            assert!(GuildAction::from_hit(&hit(item)).is_none(), "{item}");
        }
        assert_eq!(
            guild_note_hit_identity(&hit("guild:73:note_body:Éowyn")),
            Some((73, "Éowyn"))
        );
        assert_eq!(
            guild_note_hit_identity(&hit("guild:73:note:1:Éowyn")),
            Some((73, "Éowyn"))
        );
        assert!(guild_note_hit_identity(&hit("guild:73:motd:1")).is_none());
        let mut covered = hit("guild:73:note:1:Éowyn");
        covered.window_id = Some("raid".into());
        assert!(GuildAction::from_hit(&covered).is_none());
        assert!(guild_note_hit_identity(&covered).is_none());
        covered.window_id = Some("guild".into());
        covered.enabled = false;
        assert!(GuildAction::from_hit(&covered).is_none());
        assert!(guild_note_hit_identity(&covered).is_none());
        assert_ne!(guild_note_scroll_id(73, "A"), guild_note_scroll_id(73, "B"));
        assert_ne!(guild_note_scroll_id(73, "A"), guild_note_scroll_id(74, "A"));
    }

    #[test]
    fn large_roster_emits_only_visible_stable_rows_and_clamps_scroll() {
        let hud = minimal_hud();
        let mut state = GameHudState {
            guild: Some(fixture(16_384)),
            ..Default::default()
        };
        for (viewport, scroll) in [
            ([900, 650], 0),
            ([900, 650], usize::MAX),
            ([350, 400], usize::MAX),
        ] {
            state.guild.as_mut().unwrap().scroll = scroll;
            let frame = hud.gameplay_frame(viewport, &HudState::default(), &state);
            let rows: Vec<_> = frame
                .hit_targets
                .iter()
                .filter(|hit| hit.kind == "GuildMember")
                .collect();
            assert_eq!(rows.len(), guild_visible_rows(viewport[1]));
            assert!(rows.len() <= GUILD_VISIBLE_ROWS);
            let expected = if scroll == 0 {
                "Member00000"
            } else {
                "Member16383"
            };
            assert!(rows.iter().any(|hit| hit.item.ends_with(expected)));
            for row in rows {
                assert_eq!(row.window_id.as_deref(), Some("guild"));
                assert!(matches!(
                    GuildAction::from_hit(row),
                    Some(GuildAction {
                        revision: 73,
                        kind: GuildActionKind::SelectMember { .. }
                    })
                ));
            }
            assert!(
                frame.commands.len() < 150,
                "drawing should be independent of roster size"
            );
        }
        assert_eq!(state.guild.as_ref().unwrap().members.len(), 16_384);
    }

    #[test]
    fn filters_remove_hidden_hits_notes_and_keep_unknown_members() {
        let hud = minimal_hud();
        let mut guild = fixture(6);
        guild.show_offline = false;
        guild.hide_alts = true;
        let state = GameHudState {
            guild: Some(guild),
            ..Default::default()
        };
        let frame = hud.gameplay_frame([900, 650], &HudState::default(), &state);
        let members: Vec<_> = frame
            .hit_targets
            .iter()
            .filter(|hit| hit.kind == "GuildMember")
            .map(|hit| hit.item.as_str())
            .collect();
        assert_eq!(
            members,
            ["guild:73:member:Member00002", "guild:73:member:Member00005"]
        );
        assert!(!frame.hit_targets.iter().any(|hit| hit.kind == "GuildNote"));
        assert!(frame.commands.iter().any(|command| matches!(command, DrawCommand::Text { text, .. } if text == "Showing 2 of 6 received members")));
    }

    #[test]
    fn text_viewports_keep_unknown_empty_received_and_member_identity_separate() {
        let hud = minimal_hud();
        let mut state = GameHudState {
            guild: Some(fixture(2)),
            ..Default::default()
        };
        let member = &mut Arc::make_mut(&mut state.guild.as_mut().unwrap().members)[0];
        member.class = Some(999);
        member.level = Some(300);
        member.public_note = Some("<a href='guild:73:leave'>plain received text</a>".into());
        let frame = hud.gameplay_frame([900, 650], &HudState::default(), &state);
        assert!(frame.commands.iter().any(|command| matches!(command, DrawCommand::TextArea { id, text, .. } if id == &guild_note_scroll_id(73, "Member00000") && text.contains("Level 300 · Class 999") && text.contains("<a href='guild:73:leave'>"))));
        let note_hit = frame
            .hit_targets
            .iter()
            .find(|hit| hit.item == "guild:73:note:1:Member00000")
            .unwrap();
        assert_eq!(guild_note_hit_identity(note_hit), Some((73, "Member00000")));
        state.guild.as_mut().unwrap().page = UiGuildPage::Information;
        for (motd, expected) in [
            (None, "No guild message has been received."),
            (Some(UiGuildMotd::default()), "The guild message is empty."),
            (
                Some(UiGuildMotd {
                    text: "Received message".into(),
                    author: None,
                }),
                "Received message",
            ),
        ] {
            state.guild.as_mut().unwrap().motd = motd;
            let frame = hud.gameplay_frame([900, 650], &HudState::default(), &state);
            assert!(frame.commands.iter().any(|command| matches!(command, DrawCommand::TextArea { id, text, .. } if id == &guild_motd_scroll_id(73) && text == expected)));
            assert!(!frame.hit_targets.iter().any(|hit| matches!(
                hit.kind.as_str(),
                "GuildMember" | "GuildNote" | "GuildFilter"
            )));
        }
    }

    #[test]
    fn guild_window_obeys_saved_position_stack_and_close_ownership() {
        let hud = minimal_hud();
        let mut state = GameHudState {
            guild: Some(fixture(3)),
            inventory_open: true,
            ..Default::default()
        };
        state.window_positions.insert("guild".into(), [0., 0.]);
        state.window_positions.insert("inventory".into(), [0., 0.]);
        let mut stack = WindowStack::default();
        let frame = hud.gameplay_frame_with_windows(
            [900, 650],
            &HudState::default(),
            &state,
            vec![],
            &mut stack,
        );
        let point = [20., 185.];
        assert_eq!(
            frame.hit_test(point).unwrap().window_id.as_deref(),
            Some("guild")
        );
        assert!(valid_window_id("guild"));
        let close = frame
            .hit_targets
            .iter()
            .find(|hit| hit.item == "game:close:guild")
            .unwrap();
        assert_eq!(
            UiAction::from_hit(close),
            Some(UiAction::CloseWindow("guild".into()))
        );
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
        state.guild = None;
        let closed = hud.gameplay_frame([900, 650], &HudState::default(), &state);
        assert!(
            !closed
                .hit_targets
                .iter()
                .any(|hit| hit.window_id.as_deref() == Some("guild"))
        );
    }

    #[test]
    #[ignore = "requires original UI assets and GPU; optional OPENEQ_UI_CAPTURE_DIR"]
    fn original_guild_skin_roster_filters_notes_motd_at_both_scales() {
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
                "filtered",
                "note-bottom",
                "information-top",
                "information-bottom",
                "narrow-members",
                "narrow-note-bottom",
                "unknown",
                "none",
                "empty-roster",
                "empty-message",
            ] {
                let viewport = if mode.starts_with("narrow") {
                    [350, 400]
                } else {
                    [900, 650]
                };
                renderer.resize(viewport[0] * scale, viewport[1] * scale);
                let mut guild = fixture(72);
                if mode.starts_with("narrow") {
                    Arc::make_mut(&mut guild.members)[0].name =
                        "ÉowynAnUnusuallyLongMemberNameForClipping".into();
                    guild.selected_member = Some(guild.members[0].name.clone());
                }
                Arc::make_mut(&mut guild.members)[0].public_note = Some((1..=24).map(|line| format!("{line}. Gather at the library entrance. This received public note is read-only and may span several lines.\n")).collect());
                if mode == "filtered" {
                    guild.show_offline = false;
                    guild.hide_alts = true;
                    guild.selected_member = Some(guild.members[2].name.clone());
                }
                if mode.ends_with("note-bottom") {
                    guild.note_scroll_rows = usize::MAX;
                }
                if mode.starts_with("information") {
                    guild.page = UiGuildPage::Information;
                    guild.motd = Some(UiGuildMotd { text: (1..=45).map(|line| format!("{line}. Welcome to the guild library. Read the received instructions here, and ask in guild chat when ready.\n")).collect(), author: Some("Archivist".into()) });
                    if mode == "information-bottom" {
                        guild.motd_scroll_rows = usize::MAX;
                    }
                }
                if mode == "unknown" {
                    guild = UiGuild {
                        revision: 74,
                        ..Default::default()
                    };
                }
                if mode == "none" {
                    guild = UiGuild {
                        revision: 75,
                        status: "You are not in a guild.".into(),
                        ..Default::default()
                    };
                }
                if mode == "empty-roster" {
                    guild = fixture(0);
                    guild.status = "Received guild roster with no members.".into();
                }
                if mode == "empty-message" {
                    guild.page = UiGuildPage::Information;
                    guild.motd = Some(UiGuildMotd::default());
                }
                let state = GameHudState {
                    guild: Some(guild),
                    ..Default::default()
                };
                let frame = hud.gameplay_frame(viewport, &resources, &state);
                assert!(frame.warnings.is_empty(), "{:?}", frame.warnings);
                for hit in frame
                    .hit_targets
                    .iter()
                    .filter(|hit| hit.item.starts_with("guild:"))
                {
                    assert_eq!(hit.window_id.as_deref(), Some("guild"));
                }
                let close = frame
                    .hit_targets
                    .iter()
                    .find(|hit| hit.item == "game:close:guild")
                    .unwrap();
                assert_eq!(
                    frame
                        .hit_test([close.rect.x + 3., close.rect.y + 3.])
                        .unwrap()
                        .item,
                    close.item
                );
                renderer.set_ui_scaled(&frame, scale as f32);
                if !matches!(mode, "unknown" | "none" | "empty-roster") {
                    let guild = state.guild.as_ref().unwrap();
                    let expected_id = if guild.page == UiGuildPage::Information {
                        guild_motd_scroll_id(73)
                    } else {
                        guild_note_scroll_id(73, guild.selected_member.as_deref().unwrap())
                    };
                    let metrics = renderer
                        .ui_text_scroll_metrics()
                        .iter()
                        .find(|metrics| metrics.id == expected_id)
                        .unwrap();
                    let bottom = mode.ends_with("bottom");
                    assert_eq!(
                        metrics.first_row,
                        if bottom { metrics.max_scroll() } else { 0 }
                    );
                    if bottom {
                        assert!(metrics.first_row > 0);
                    }
                    let thumb = frame
                        .commands
                        .iter()
                        .find_map(|command| match command {
                            DrawCommand::TextArea { thumb, .. } => thumb.as_ref(),
                            _ => None,
                        })
                        .unwrap();
                    assert!(thumb.images.iter().all(Option::is_some));
                }
                renderer.render_ui();
                if let Some(directory) = std::env::var_os("OPENEQ_UI_CAPTURE_DIR") {
                    std::fs::create_dir_all(&directory).unwrap();
                    let (width, height, pixels) = renderer.read_rgba().unwrap();
                    image::save_buffer(
                        std::path::PathBuf::from(directory)
                            .join(format!("guild-{mode}-{scale}x.png")),
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
