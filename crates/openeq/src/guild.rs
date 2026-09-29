//! Guild data is bound to independently confirmed local membership. The roster
//! packet's reserved bytes and prefix never establish which guild we belong to.
use openeq_net::guild::{GuildEvent, GuildMember, MAX_MEMBERS};
use std::collections::BTreeMap;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Motd {
    pub author: String,
    pub text: String,
}

#[derive(Default)]
pub struct GuildState {
    pub confirmed: bool,
    pub guild_id: Option<u32>,
    pub rank: Option<u32>,
    pub name: Option<String>,
    pub roster_received: bool,
    pub members: BTreeMap<String, GuildMember>,
    pub motd: Option<Motd>,
    pub revision: u64,
    directory: BTreeMap<u32, String>,
}

impl GuildState {
    fn changed(&mut self) {
        self.revision = self.revision.wrapping_add(1);
    }

    /// Keep a visibly stale snapshot during travel/disconnection. No old guild
    /// event can refresh it before this zone establishes local identity again.
    pub fn begin_zone(&mut self) {
        self.confirmed = false;
        self.roster_received = false;
        self.changed();
    }

    pub fn identity(&mut self, guild_id: Option<u32>, rank: Option<u32>) {
        if !self.confirmed || self.guild_id != guild_id {
            self.members.clear();
            self.motd = None;
            self.roster_received = false;
            self.guild_id = guild_id;
            self.rank = rank;
            self.name = guild_id.and_then(|id| self.directory.get(&id).cloned());
        } else if rank.is_some() {
            self.rank = rank;
        }
        self.confirmed = true;
        self.changed();
    }

    pub fn appearance_rank(&mut self, rank: u32) {
        if self.confirmed && self.guild_id.is_some() {
            self.rank = Some(rank);
            self.changed();
        }
    }

    pub fn member(&self, name: &str) -> Option<&GuildMember> {
        self.members.get(&name.to_ascii_lowercase())
    }

    fn belongs(&self, guild_id: u32) -> bool {
        self.confirmed && self.guild_id == Some(guild_id)
    }

    pub fn apply(&mut self, event: GuildEvent, own_name: &str) {
        match event {
            GuildEvent::Directory(entries) => {
                self.directory = entries
                    .into_iter()
                    .map(|entry| (entry.id, entry.name))
                    .collect();
                self.name = self
                    .guild_id
                    .and_then(|id| self.directory.get(&id).cloned());
            }
            GuildEvent::Roster { members } if self.confirmed && self.guild_id.is_some() => {
                if members.len() > MAX_MEMBERS {
                    return;
                }
                let count = members.len();
                let members: BTreeMap<_, _> = members
                    .into_iter()
                    .map(|member| (member.name.to_ascii_lowercase(), member))
                    .collect();
                if members.len() != count {
                    return;
                }
                self.members = members;
                self.roster_received = true;
            }
            GuildEvent::Motd {
                recipient,
                author,
                text,
            } if self.confirmed
                && self.guild_id.is_some()
                && recipient.eq_ignore_ascii_case(own_name) =>
            {
                self.motd = Some(Motd { author, text });
            }
            GuildEvent::MemberAdded { guild_id, member } if self.belongs(guild_id) => {
                let key = member.name.to_ascii_lowercase();
                if self.members.len() >= MAX_MEMBERS && !self.members.contains_key(&key) {
                    return;
                }
                let mut member = member;
                if let Some(previous) = self.members.get(&key) {
                    // Add records omit these fields; a repeated add must not
                    // erase metadata from the earlier full roster.
                    member.banker = member.banker.or(previous.banker);
                    member.alt = member.alt.or(previous.alt);
                    if member.public_note.is_none() {
                        member.public_note.clone_from(&previous.public_note);
                    }
                }
                self.members.insert(key, member);
            }
            GuildEvent::MemberRemoved { guild_id, name } if self.belongs(guild_id) => {
                if name.eq_ignore_ascii_case(own_name) {
                    self.identity(None, None);
                    return;
                }
                self.members.remove(&name.to_ascii_lowercase());
            }
            GuildEvent::MemberRenamed {
                guild_id,
                old_name,
                new_name,
            } if self.belongs(guild_id) => {
                let key = new_name.to_ascii_lowercase();
                if !old_name.eq_ignore_ascii_case(&new_name) && self.members.contains_key(&key) {
                    return;
                }
                let Some(mut member) = self.members.remove(&old_name.to_ascii_lowercase()) else {
                    return;
                };
                member.name = new_name;
                self.members.insert(key, member);
            }
            GuildEvent::MemberLevel {
                guild_id,
                name,
                level,
            } if self.belongs(guild_id) => {
                let Some(member) = self.members.get_mut(&name.to_ascii_lowercase()) else {
                    return;
                };
                member.level = level;
            }
            GuildEvent::MemberRank {
                guild_id,
                name,
                rank,
                banker,
                alt,
            } if self.belongs(guild_id) => {
                let Some(member) = self.members.get_mut(&name.to_ascii_lowercase()) else {
                    return;
                };
                member.rank = rank;
                member.banker = Some(banker);
                member.alt = Some(alt);
                if name.eq_ignore_ascii_case(own_name) {
                    self.rank = Some(rank);
                }
            }
            GuildEvent::MemberNote {
                guild_id,
                name,
                note,
            } if self.belongs(guild_id) => {
                let Some(member) = self.members.get_mut(&name.to_ascii_lowercase()) else {
                    return;
                };
                member.public_note = Some(note);
            }
            GuildEvent::MemberDetails {
                guild_id,
                name,
                last_seen,
                presence,
            } if self.belongs(guild_id) => {
                let Some(member) = self.members.get_mut(&name.to_ascii_lowercase()) else {
                    return;
                };
                member.last_seen = last_seen;
                member.presence = presence;
            }
            GuildEvent::Renamed { guild_id, name } => {
                // A rename may update known directory entries but cannot grow
                // an unbounded second directory from unrelated events.
                if let Some(entry) = self.directory.get_mut(&guild_id) {
                    entry.clone_from(&name);
                }
                if self.guild_id == Some(guild_id) {
                    self.name = Some(name);
                }
            }
            GuildEvent::Deleted { guild_id } => {
                self.directory.remove(&guild_id);
                if self.belongs(guild_id) {
                    self.identity(None, None);
                    return;
                }
            }
            _ => return,
        }
        self.changed();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use openeq_net::guild::{GuildDirectoryEntry, GuildPresence};

    pub(crate) fn member(name: &str) -> GuildMember {
        GuildMember {
            name: name.into(),
            level: 10,
            class: 1,
            rank: 5,
            banker: Some(false),
            alt: Some(false),
            presence: GuildPresence::Offline,
            last_seen: None,
            public_note: Some(String::new()),
        }
    }
    #[test]
    fn roster_and_motd_cannot_establish_membership_or_survive_identity_change() {
        let mut state = GuildState::default();
        let roster = || GuildEvent::Roster {
            members: vec![member("Alice")],
        };
        state.apply(roster(), "Alice");
        assert!(state.members.is_empty() && !state.confirmed);
        state.identity(Some(42), Some(5));
        state.apply(roster(), "Alice");
        state.apply(
            GuildEvent::Motd {
                recipient: "Other".into(),
                author: "Officer".into(),
                text: "wrong".into(),
            },
            "Alice",
        );
        assert!(state.motd.is_none());
        state.apply(
            GuildEvent::Motd {
                recipient: "ALICE".into(),
                author: String::new(),
                text: String::new(),
            },
            "Alice",
        );
        assert_eq!(state.motd.as_ref().unwrap().text, "");
        state.begin_zone();
        assert_eq!(state.members.len(), 1);
        assert!(!state.confirmed && !state.roster_received);
        state.apply(
            GuildEvent::MemberAdded {
                guild_id: 42,
                member: member("Old zone"),
            },
            "Alice",
        );
        assert_eq!(state.members.len(), 1);
        state.identity(Some(43), Some(8));
        assert!(state.members.is_empty() && state.motd.is_none());
        state.apply(
            GuildEvent::MemberAdded {
                guild_id: 42,
                member: member("Old guild"),
            },
            "Alice",
        );
        assert!(state.members.is_empty());
        state.identity(None, None);
        state.apply(roster(), "Alice");
        assert!(state.confirmed && state.guild_id.is_none() && state.members.is_empty());
    }

    #[test]
    fn incremental_rows_preserve_identity_and_unknown_presence() {
        let mut state = GuildState::default();
        state.apply(
            GuildEvent::Directory(vec![GuildDirectoryEntry {
                id: 42,
                name: "Test Guild".into(),
            }]),
            "Alice",
        );
        state.identity(Some(42), Some(5));
        assert_eq!(state.name.as_deref(), Some("Test Guild"));
        state.apply(
            GuildEvent::Roster {
                members: vec![member("Alice"), member("Bob")],
            },
            "Alice",
        );
        state.apply(
            GuildEvent::MemberLevel {
                guild_id: 41,
                name: "Bob".into(),
                level: 40,
            },
            "Alice",
        );
        assert_eq!(state.member("BOB").unwrap().level, 10);
        state.apply(
            GuildEvent::MemberLevel {
                guild_id: 42,
                name: "Unknown".into(),
                level: 40,
            },
            "Alice",
        );
        assert_eq!(state.members.len(), 2);
        state.apply(
            GuildEvent::MemberDetails {
                guild_id: 42,
                name: "BOB".into(),
                last_seen: Some(123),
                presence: GuildPresence::Unknown,
            },
            "Alice",
        );
        assert_eq!(state.member("Bob").unwrap().last_seen, Some(123));
        assert!(matches!(
            state.member("Bob").unwrap().presence,
            GuildPresence::Unknown
        ));
        state.apply(
            GuildEvent::MemberRenamed {
                guild_id: 42,
                old_name: "BOB".into(),
                new_name: "Alice".into(),
            },
            "Alice",
        );
        assert_eq!(
            state.members.len(),
            2,
            "rename must not overwrite another member"
        );
        state.apply(
            GuildEvent::MemberRenamed {
                guild_id: 42,
                old_name: "BOB".into(),
                new_name: "Robert".into(),
            },
            "Alice",
        );
        assert!(state.member("Bob").is_none());
        assert_eq!(state.member("Robert").unwrap().last_seen, Some(123));
        state.apply(
            GuildEvent::MemberRemoved {
                guild_id: 42,
                name: "ALICE".into(),
            },
            "Alice",
        );
        assert!(state.guild_id.is_none() && state.members.is_empty());
        state.apply(
            GuildEvent::MemberAdded {
                guild_id: 42,
                member: member("Alice"),
            },
            "Alice",
        );
        assert!(state.members.is_empty());
    }

    #[test]
    fn duplicate_identity_and_partial_member_add_keep_confirmed_metadata() {
        let mut state = GuildState::default();
        state.identity(Some(42), Some(1));
        let mut alice = member("Alice");
        alice.public_note = Some("Keep this roster note".into());
        alice.alt = Some(true);
        state.apply(
            GuildEvent::Roster {
                members: vec![alice],
            },
            "Alice",
        );
        state.apply(
            GuildEvent::Motd {
                recipient: "Alice".into(),
                author: "Alice".into(),
                text: "Welcome".into(),
            },
            "Alice",
        );
        // EQEmu's world callback repeats the earlier local guild appearance.
        state.identity(Some(42), None);
        assert!(state.roster_received);
        assert_eq!(state.motd.as_ref().unwrap().text, "Welcome");
        let mut addition = member("ALICE");
        addition.alt = None;
        addition.banker = None;
        addition.public_note = None;
        addition.level = 11;
        state.apply(
            GuildEvent::MemberAdded {
                guild_id: 42,
                member: addition,
            },
            "Alice",
        );
        let alice = state.member("Alice").unwrap();
        assert_eq!(alice.level, 11);
        assert_eq!(alice.alt, Some(true));
        assert_eq!(alice.public_note.as_deref(), Some("Keep this roster note"));
    }

    #[test]
    fn unrelated_guild_rename_stream_does_not_expand_directory() {
        let mut state = GuildState::default();
        state.apply(
            GuildEvent::Directory(vec![GuildDirectoryEntry {
                id: 42,
                name: "Original".into(),
            }]),
            "Alice",
        );
        state.identity(Some(42), Some(1));
        for guild_id in 1000..2000 {
            state.apply(
                GuildEvent::Renamed {
                    guild_id,
                    name: "Unrelated".into(),
                },
                "Alice",
            );
        }
        assert_eq!(state.directory.len(), 1);
        state.apply(
            GuildEvent::Renamed {
                guild_id: 42,
                name: "Renamed".into(),
            },
            "Alice",
        );
        assert_eq!(state.name.as_deref(), Some("Renamed"));
        state.begin_zone();
        state.identity(Some(42), Some(1));
        assert_eq!(state.name.as_deref(), Some("Renamed"));
    }
}
