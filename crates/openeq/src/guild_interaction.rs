//! Read-only guild presentation. Opening, filtering and selecting never queue
//! gameplay commands; every hit retains its displayed roster/member identity.
use crate::{
    gameplay_ui::GameHudState,
    guild_ui::{
        GuildAction, GuildActionKind, UiGuild, UiGuildMember, UiGuildMotd, UiGuildPage,
        UiGuildPresence, guild_member_visible,
    },
    interaction::Interaction,
    live::LiveWorld,
};
use openeq_net::guild::GuildPresence;
use std::{
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};

pub struct GuildWindowState {
    pub open: bool,
    pub page: UiGuildPage,
    pub show_offline: bool,
    pub hide_alts: bool,
    pub scroll: usize,
    pub selection: Option<String>,
    pub motd_scroll: usize,
    pub note_scroll: usize,
    pub visible_rows: usize,
    motd_max_scroll: usize,
    note_max_scroll: usize,
    seen_identity: Option<Option<u32>>,
    cached_revision: Option<u64>,
    cached_minute: u64,
    cached_members: Arc<[UiGuildMember]>,
}
impl Default for GuildWindowState {
    fn default() -> Self {
        Self {
            open: false,
            page: UiGuildPage::Members,
            show_offline: true,
            hide_alts: false,
            scroll: 0,
            selection: None,
            motd_scroll: 0,
            note_scroll: 0,
            visible_rows: 1,
            motd_max_scroll: 0,
            note_max_scroll: 0,
            seen_identity: None,
            cached_revision: None,
            cached_minute: 0,
            cached_members: Arc::from([]),
        }
    }
}

fn timestamp_age(timestamp: u32, now: u64) -> Option<String> {
    let seconds = now.checked_sub(u64::from(timestamp))?;
    Some(if seconds < 60 {
        "Just now".into()
    } else if seconds < 3600 {
        format!("{} minutes ago", seconds / 60)
    } else if seconds < 86400 {
        format!("{} hours ago", seconds / 3600)
    } else {
        format!("{} days ago", seconds / 86400)
    })
}
fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |time| time.as_secs())
}

fn member_views(live: &LiveWorld, now: u64) -> Arc<[UiGuildMember]> {
    live.game
        .guild
        .members
        .values()
        .map(|member| UiGuildMember {
            name: member.name.clone(),
            level: Some(member.level),
            class: Some(member.class),
            rank: format!("Rank {}", member.rank),
            alt: member.alt,
            banker: member.banker,
            presence: match member.presence {
                GuildPresence::Unknown => UiGuildPresence::Unknown,
                GuildPresence::Offline => UiGuildPresence::ReportedOffline,
                GuildPresence::Online { zone_id } => UiGuildPresence::ReportedOnline {
                    location: live
                        .environment
                        .as_ref()
                        .filter(|zone| u32::from(zone.zone_id) == zone_id)
                        .map(|zone| zone.long_name.clone()),
                },
            },
            last_seen: member.last_seen.and_then(|time| timestamp_age(time, now)),
            public_note: member.public_note.clone(),
        })
        .collect()
}

impl Interaction {
    pub(crate) fn guild_view(&self, live: &LiveWorld, view: &mut GameHudState) {
        let window = &self.guild_window;
        if !window.open {
            return;
        }
        let guild = &live.game.guild;
        let status = if !live.ready || live.error.is_some() || !guild.confirmed {
            "Waiting for guild information; any displayed roster is the last known snapshot.".into()
        } else if guild.guild_id.is_none() {
            "You are not in a guild.".into()
        } else if !guild.roster_received {
            "Waiting for the guild roster.".into()
        } else {
            format!(
                "{} members · Presence shows the latest received report",
                guild.members.len()
            )
        };
        view.guild = Some(UiGuild {
            revision: guild.revision,
            status,
            name: guild.name.clone(),
            members: if window.cached_revision == Some(guild.revision) {
                window.cached_members.clone()
            } else {
                member_views(live, now())
            },
            motd: guild.motd.as_ref().map(|motd| UiGuildMotd {
                text: motd.text.clone(),
                author: (!motd.author.is_empty()).then(|| motd.author.clone()),
            }),
            page: window.page,
            show_offline: window.show_offline,
            hide_alts: window.hide_alts,
            scroll: window.scroll,
            selected_member: window.selection.clone(),
            motd_scroll_rows: window.motd_scroll,
            note_scroll_rows: window.note_scroll,
        });
    }

    pub(crate) fn guild_tick(&mut self, live: &LiveWorld) {
        let guild = &live.game.guild;
        let window = &mut self.guild_window;
        if guild.confirmed && window.seen_identity != Some(guild.guild_id) {
            window.seen_identity = Some(guild.guild_id);
            window.selection = None;
            window.scroll = 0;
            window.motd_scroll = 0;
            window.note_scroll = 0;
            window.motd_max_scroll = 0;
            window.note_max_scroll = 0;
        }
        let time = now();
        if window.cached_revision != Some(guild.revision) || window.cached_minute != time / 60 {
            window.cached_members = member_views(live, time);
            window.cached_revision = Some(guild.revision);
            window.cached_minute = time / 60;
        }
        self.clamp_guild_view();
    }

    fn clamp_guild_view(&mut self) {
        let window = &mut self.guild_window;
        let mut count = 0usize;
        let mut selected_visible = false;
        for member in window
            .cached_members
            .iter()
            .filter(|member| guild_member_visible(member, window.show_offline, window.hide_alts))
        {
            count += 1;
            selected_visible |= window
                .selection
                .as_ref()
                .is_some_and(|name| name.eq_ignore_ascii_case(&member.name));
        }
        window.scroll = window
            .scroll
            .min(count.saturating_sub(window.visible_rows.max(1)));
        if !selected_visible {
            window.selection = None;
            window.note_scroll = 0;
            window.note_max_scroll = 0;
        }
    }

    pub(crate) fn guild_action(&mut self, action: GuildAction, live: &LiveWorld) {
        if !self.guild_window.open || action.revision != live.game.guild.revision {
            return;
        }
        self.guild_tick(live);
        let window = &mut self.guild_window;
        match action.kind {
            GuildActionKind::SelectMember { name } => {
                if let Some(member) = window.cached_members.iter().find(|member| {
                    member.name.eq_ignore_ascii_case(&name)
                        && guild_member_visible(member, window.show_offline, window.hide_alts)
                }) {
                    window.selection = Some(member.name.clone());
                    window.note_scroll = 0;
                    window.note_max_scroll = 0;
                }
            }
            GuildActionKind::SetShowOffline(value) => {
                window.show_offline = value;
                window.scroll = 0;
            }
            GuildActionKind::SetHideAlts(value) => {
                window.hide_alts = value;
                window.scroll = 0;
            }
            GuildActionKind::Scroll { rows } => {
                window.scroll = window.scroll.saturating_add_signed(rows as isize);
            }
            GuildActionKind::Page(page) => {
                window.page = page;
            }
            GuildActionKind::ScrollMotd { rows } => {
                window.motd_scroll = window
                    .motd_scroll
                    .min(window.motd_max_scroll)
                    .saturating_add_signed(rows as isize)
                    .min(window.motd_max_scroll);
            }
            GuildActionKind::ScrollNote { name, rows } => {
                if window
                    .selection
                    .as_ref()
                    .is_some_and(|selected| selected.eq_ignore_ascii_case(&name))
                {
                    window.note_scroll = window
                        .note_scroll
                        .min(window.note_max_scroll)
                        .saturating_add_signed(rows as isize)
                        .min(window.note_max_scroll);
                }
            }
        }
        self.clamp_guild_view();
    }

    pub fn guild_wheel(&mut self, live: &LiveWorld, hit: &openeq_ui::HitTarget, delta: f32) {
        if !delta.is_finite()
            || delta == 0.
            || !hit
                .item
                .starts_with(&format!("guild:{}:", live.game.guild.revision))
        {
            return;
        }
        let rows = (-delta * 3.).clamp(-72., 72.) as i32;
        let kind = match hit.kind.as_str() {
            "GuildNote" => {
                let Some((revision, name)) = crate::guild_ui::guild_note_hit_identity(hit) else {
                    return;
                };
                if revision != live.game.guild.revision {
                    return;
                }
                GuildActionKind::ScrollNote {
                    name: name.to_owned(),
                    rows,
                }
            }
            "GuildMotd" => GuildActionKind::ScrollMotd { rows },
            "GuildRoster" | "GuildMember" => GuildActionKind::Scroll { rows },
            _ => return,
        };
        self.guild_action(
            GuildAction {
                revision: live.game.guild.revision,
                kind,
            },
            live,
        );
    }

    pub fn guild_text_metrics(&mut self, revision: u64, metrics: &[openeq_ui::TextScrollMetrics]) {
        let window = &mut self.guild_window;
        if let Some(metric) = metrics
            .iter()
            .find(|metric| metric.id == crate::guild_ui::guild_motd_scroll_id(revision))
        {
            window.motd_max_scroll = metric.total_rows.saturating_sub(metric.visible_rows);
            window.motd_scroll = window.motd_scroll.min(window.motd_max_scroll);
        }
        if let Some(name) = &window.selection {
            let id = crate::guild_ui::guild_note_scroll_id(revision, name);
            if let Some(metric) = metrics.iter().find(|metric| metric.id == id) {
                window.note_max_scroll = metric.total_rows.saturating_sub(metric.visible_rows);
                window.note_scroll = window.note_scroll.min(window.note_max_scroll);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::live::{NetworkCommand, tests::command_world};
    use openeq_net::{
        gameplay::{ChatChannel, Command},
        guild::{GuildEvent, GuildMember},
    };

    fn member(name: &str, presence: GuildPresence, alt: Option<bool>) -> GuildMember {
        GuildMember {
            name: name.into(),
            level: 10,
            class: 1,
            rank: 5,
            banker: None,
            alt,
            presence,
            last_seen: None,
            public_note: Some(name.repeat(100)),
        }
    }

    #[test]
    fn guild_window_filtering_and_closing_are_local_and_preserve_guild_chat() {
        let (mut live, mut wire) = command_world(1, 1.);
        live.game.guild.identity(Some(42), Some(5));
        live.game.guild.apply(
            GuildEvent::Roster {
                members: vec![
                    member("Alice", GuildPresence::Online { zone_id: 202 }, Some(false)),
                    member("Offline", GuildPresence::Offline, Some(false)),
                    member("Uncertain", GuildPresence::Unknown, None),
                    member("Alternate", GuildPresence::Unknown, Some(true)),
                ],
            },
            "Player",
        );
        let mut ui = Interaction::default();
        ui.submit("/guildwindow", &mut live, [0.; 3]);
        assert!(ui.guild_window.open);
        ui.guild_tick(&live);
        let action = |kind| GuildAction {
            revision: live.game.guild.revision,
            kind,
        };
        ui.guild_action(
            action(GuildActionKind::SelectMember {
                name: "Offline".into(),
            }),
            &live,
        );
        ui.guild_action(action(GuildActionKind::SetShowOffline(false)), &live);
        assert!(ui.guild_window.selection.is_none());
        ui.guild_action(action(GuildActionKind::SetHideAlts(true)), &live);
        let view = ui.view(&live).guild.unwrap();
        assert_eq!(
            view.members
                .iter()
                .filter(|member| guild_member_visible(member, view.show_offline, view.hide_alts))
                .map(|member| member.name.as_str())
                .collect::<Vec<_>>(),
            vec!["Alice", "Uncertain"]
        );
        assert!(wire.try_recv().is_err());
        ui.close_window("guild", &mut live);
        assert!(!ui.guild_window.open);
        assert_eq!(live.game.guild.members.len(), 4);
        ui.submit("/guild hello", &mut live, [0.; 3]);
        assert!(
            matches!(wire.try_recv().unwrap(), NetworkCommand::Gameplay(Command::Chat { channel: ChatChannel::Guild, text, .. }) if text == "hello")
        );
        assert!(wire.try_recv().is_err());
    }

    #[test]
    fn old_guild_and_old_member_hits_cannot_redirect_selection_or_note_scroll() {
        let (mut live, mut wire) = command_world(1, 1.);
        live.game.guild.identity(Some(42), Some(5));
        live.game.guild.apply(
            GuildEvent::Roster {
                members: vec![
                    member("Alice", GuildPresence::Unknown, None),
                    member("Bob", GuildPresence::Unknown, None),
                ],
            },
            "Player",
        );
        let mut ui = Interaction::default();
        ui.guild_window.open = true;
        let revision = live.game.guild.revision;
        let action = |kind| GuildAction { revision, kind };
        ui.guild_action(
            action(GuildActionKind::SelectMember {
                name: "Alice".into(),
            }),
            &live,
        );
        ui.guild_action(
            action(GuildActionKind::SelectMember { name: "Bob".into() }),
            &live,
        );
        ui.guild_window.note_max_scroll = 100;
        ui.guild_action(
            action(GuildActionKind::ScrollNote {
                name: "Alice".into(),
                rows: 5,
            }),
            &live,
        );
        assert_eq!(ui.guild_window.note_scroll, 0);
        ui.guild_action(
            action(GuildActionKind::ScrollNote {
                name: "Bob".into(),
                rows: 5,
            }),
            &live,
        );
        assert_eq!(ui.guild_window.note_scroll, 5);
        live.game.guild.identity(Some(43), Some(5));
        live.game.guild.apply(
            GuildEvent::Roster {
                members: vec![member("Bob", GuildPresence::Unknown, None)],
            },
            "Player",
        );
        ui.guild_tick(&live);
        ui.guild_action(
            action(GuildActionKind::SelectMember { name: "Bob".into() }),
            &live,
        );
        assert!(ui.guild_window.selection.is_none());
        assert_eq!(ui.guild_window.note_scroll, 0);
        assert!(wire.try_recv().is_err());
    }

    #[test]
    fn guild_scroll_clamps_before_reverse_scroll_and_caches_shared_rows() {
        let (mut live, _) = command_world(1, 1.);
        live.game.guild.identity(Some(42), Some(5));
        live.game.guild.apply(
            GuildEvent::Roster {
                members: (0..30)
                    .map(|i| member(&format!("Player{i:02}"), GuildPresence::Unknown, None))
                    .collect(),
            },
            "Player",
        );
        let mut ui = Interaction::default();
        ui.guild_window.open = true;
        ui.guild_window.visible_rows = 12;
        ui.guild_tick(&live);
        let first = ui.view(&live).guild.unwrap().members;
        let second = ui.view(&live).guild.unwrap().members;
        assert!(Arc::ptr_eq(&first, &second));
        ui.guild_window.scroll = 200;
        ui.guild_action(
            GuildAction {
                revision: live.game.guild.revision,
                kind: GuildActionKind::Scroll { rows: -3 },
            },
            &live,
        );
        assert_eq!(ui.guild_window.scroll, 15);
        ui.guild_window.visible_rows = 28;
        ui.guild_tick(&live);
        assert_eq!(ui.guild_window.scroll, 2);
    }
}
