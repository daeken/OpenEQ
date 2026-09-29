# Raid and guild parity: protocol and fixture plan

Source investigation, raid/guild implementation, and dedicated live validation,
2026-09-29. Research was read-only; subsequent fixture logins and guarded
restoration are recorded below. EQEmu source revision:
`4aceae18b94ffaafc08e2b17bc41cd72c77f795d`; OpenEQ base at investigation:
`d8fb83dd7f9b22bf6debec7272aa8397f56e4916`. EQEmu paths below refer to the sibling
checkout `/Users/daeken/projects/EQEmu`. Wire opcode numbers come from
`utils/patches/patch_RoF2.conf`; the live probe compares the deployed mapping
before sending anything. Source evidence is distinct from live proof.

Current implemented scope is **raid invitations, roster, self-leave, leadership
transfer and raid chat; plus receive-only guild identity, directory, roster,
MOTD and server-driven updates**. `/rsay` and the raid membership decoder/reducer
are implemented. Guild profile identity and own appearance transitions now feed
the guild decoder/reducer and original XML guild window. No outgoing guild
protocol command is included, **not even MOTD refresh**. Existing guild chat is
unchanged. Guild invitations, self-leave, rank/permission management and the
source-known MOTD refresh remain later slices. Use `SOCIAL_UI_PLAN.md` for
original XML controls and presentation constraints.

The original audit and proposed acceptance order below are historical research;
the completed raid and guild validation sections record the shipped scope and
its limits. Source-known capabilities are not claims that a command is enabled.

## Original audit: existing coverage and reuse

- `openeq-net/src/social.rs`, `openeq/src/group.rs`, and `SOCIAL_PROTOCOL.md`
  already cover six-member groups, invite/accept/decline, server-driven roster,
  leader changes, leave, and quest links. Keep those behaviors intact. Raid
  subgroups are not a replacement for that implementation; EQEmu also sends
  existing group updates while assigning raid subgroups.
- `gameplay::ChatChannel` includes Guild=0 and Raid=15, and incoming chat labels
  both correctly. `/guild` and `/gu` are wired. `/rsay` is absent from `chat.rs`,
  and `interaction.rs` rejects outgoing channel15. Adding the parser alias and
  dispatch mapping completes this existing chat transport path.
- `WorldOp::GuildsList` is named, but world selection discards its contents.
  `WorldClient::enter_world` also discards chat-server setup packets. No raid or
  guild membership reducer, roster decoder, or management command exists.
- Player profile and spawn parsing skip guild identity/rank. Generic
  `SpawnAppearance` decoding exists; live handling currently ignores guild
  appearance types. Extend those known fields instead of inferring membership
  from a guild-tag string or an invitation.

Recommended separation: `openeq-net::raid` and `openeq-net::guild` for checked
wire events/commands; corresponding foreground reducers for membership and
pending choices. Reuse `Command`/`GameplayEvent`, existing chat rendering, list
widgets, and normal `LiveWorld` transport. A sent command only changes pending
UI state; server events change membership. Scope stale-choice tokens to session,
zone generation where applicable, membership generation, and selected name.

Implemented raid Rust contract (existing group types stay unchanged):

```rust
pub struct RaidMember {
    pub name: String,
    pub class: u8,
    pub level: u8,
    pub group: Option<u8>, // Some(0..=11), None = 0xffffffff
    pub group_leader: bool,
}
pub enum RaidEvent {
    Invitation { inviter: String, invitee: String },
    Created { leader: String },
    MemberAdded(RaidMember),
    MemberRemoved { name: String },
    Disbanded,
    NoRaid, // Explicit136-byte zone-entry action10, passed through unchanged.
    LeaderChanged { leader: String },
    LockChanged { locked: bool },
    Motd { text: String },
    Note { member: String, text: String },
    LeadershipData, // Consume known layout; no invented AA behavior.
}
pub enum RaidCommand {
    Invite { inviter: String, invitee: String },
    Accept { inviter: String, invitee: String },
    Leave { character: String },
    MakeLeader { character: String, leader: String },
}
pub fn parse_packet(opcode: u16, data: &[u8])
    -> Option<Result<RaidEvent, ZoneError>>;
pub fn encode_command(command: RaidCommand) -> Result<AppPacket, ZoneError>;
```

Wrap these as `GameplayEvent::Raid(RaidEvent)` and `Command::Raid(RaidCommand)`.
Unknown action values return no recognized event; malformed recognized actions
return a checked error. MakeLeader's wire command is bounded and source-backed,
but its UI can remain disabled until leader-transfer fixture proof is complete.

`crates/openeq-net/src/raid.rs` now implements this contract. Public constants
expose72 members,12 subgroups and6 members per subgroup. Action3 is used for
outgoing invite,1 for accept,5 for self-leave,30 for MakeLeader. There is no
wire decline command. `gameplay.rs` routes raid events/commands independently
of group social types and includes Raid in the existing chat wire-value test.

Initial validation: all **83 network library tests** passed, including seven new raid
tests covering real encoder offsets, subgroup sentinel/boolean validation,
action-specific identity, text bounds, every byte-prefix truncation and trailer
for supported layouts, ignored opaque actions/reserved bytes, and outgoing
direction/name/self-leave rules. Strict all-target network Clippy and formatting
checks passed. The live zone-entry correction below adds one regression,
bringing the network library total to **84 passed**. Foreground reducer/UI
integration is documented by its owner. The subsequent receive-only guild
implementation is recorded at the end of this document; no guild mutation API
was added.

## Raid wire evidence

Sources: `common/raid.h`; `common/patches/rof2_structs.h:4370`;
`common/patches/rof2.cpp::ENCODE(OP_RaidJoin)`, `ENCODE(OP_RaidUpdate)`, and
`DECODE(OP_RaidInvite)`; `zone/client_packet.cpp::Handle_OP_RaidCommand`;
`zone/raids.cpp::SendRaid*`, `SendBulkRaid`, `RemoveMember`, `MoveMember`.

`OP_RaidInvite=0x55ac` is the client command carrier;
`OP_RaidUpdate=0x3973` is the server event carrier. `OP_RaidJoin=0x0000` is **not**
a missing wire mapping to repair: its encoder deliberately emits RaidUpdate,
action8, parameter1, with both names set to the raid leader.

Integers below are little-endian. The general RoF2 record is **140 bytes**:

| Offset | Width | Field |
| --- | --- | --- |
| 0 | 4 | action |
| 4 | 64 | player_name, terminated fixed string |
| 68 | 4 | unknown; ignore on receive, initialize on send |
| 72 | 64 | leader_name, meaning depends on action |
| 136 | 4 | parameter, meaning depends on action |

Do not use stale comments that put the next fields at byte136: the declaration
and encoder put the added member's class at **140**.

| Server action | Actual RoF2 bytes | Meaning and reducer input |
| --- | --- | --- |
| 0, add | 148 | General + class u8@140, level u8@141, group-leader u8@142, five uninterpreted flag bytes@143. Both names identify the member; parameter is subgroup0–11 or `0xffffffff` for unassigned. Class/level come from real raid member data. |
| 1, remove | 140 | Both names identify the removed row. Also sent during subgroup moves; this alone does not mean the local player left the raid. |
| 5, disband | 140 | Clear local membership. `RemoveMember` sends this to the removed client in addition to the row-removal broadcast. |
| 8, create/rebuild | 140 | Leader identity and beginning of a server-driven roster sequence. Also used to rebuild the moved client's view. |
| 10, no raid / zone reset | 136 | `Client::SendZoneInPackets` sends `ZoneInSendName_Struct`: action@0, self name[64]@4, self name[64]@68, unused u32@132. RoF2 passes this record through unchanged. It precedes any current raid rebuild. |
| 17 / 18, lock/unlock | 140 | Authoritative lock flag. Names can be leader or recipient; do not change leadership from these names. |
| 20, invitation | 140 | `player_name` is invitee, `leader_name` inviter, parameter0 in the current handler. Keep a pending invitation; do not create membership. |
| 30, leader / 14, leadership data | 392 | action@0, player[64]@4, unknown u32@68, leader[64]@72, group-AA block64@136, raid-AA block64@200, unknown128@264. Only action30 establishes a new leader; action14 can be subgroup AA data. |
| 35, MOTD | 1164 | General + terminated text[1024]@140. The sender's player_name is leader; leader_name is a recipient field and is not reliable as a leader identity. |
| 36, note | 204 | General + terminated note[64]@140; leader_name identifies the noted member. No need to interpret player_name as authoritative. |

Other action constants are not proof of an implemented event. In particular,
`RaidMembers_Struct`/action6 exists in headers, but the inspected server's
`SendBulkRaid` emits repeated action0 records. Do not invent a full-roster parser
or request based on that unused declaration. Action10 is a distinct136-byte
zone-entry record, confirmed in the live probe and in `zone/client.cpp:859`
and `common/eq_packet_structs.h:4060`; it is not a140-byte general record.
Unrecognized actions must not mutate state.

On zone entry, `Client::CompleteConnect` obtains raid membership from the
database and sends create, local add, bulk adds, subgroup update, MOTD,
leader/AA data, and lock state when locked. There is no explicit final-row
marker/count. Treat the roster as an incrementally rebuilt set, not an atomic
snapshot. A subgroup move may remove a row, recreate the local view, replay
other members, re-add the moved member, then repeat leadership. Preserve
idempotence and tolerate this ordering. `zone/raids.h` supplies the real limits:
72 members, 12 groups, at most six members per group; unassigned is `u32::MAX`.
No online/offline flag is provided in the add record. A missing local spawn
means only that no in-zone spawn is currently known.

### Raid command meanings

The common command and RoF2 general layouts both total140. The patch explicitly
copies action, both names, and parameter. Names in these commands are not the
same roles for every action:

| Action | Request fields and server behavior | First-slice treatment |
| --- | --- | --- |
| 0 or 3, invite | player_name=target, leader_name=inviter. Handler locates target in this zone; rejects already-raided players and nonleader members of ordinary groups. | Choose one source-supported action, validate same-zone target/player identity, send once; await invitation/roster. Start fixtures ungrouped. |
| 1, accept | player_name=inviter, leader_name=accepting self. Handler resolves both online in the zone; may import entire existing groups and enforce72-member capacity. | Only construct from a current received invitation, never arbitrary entered names. |
| 5, disband/remove | leader_name=member to remove, player_name=actor. Handler removes that member, updates leader/subgroup, and disbands an empty raid. | Expose **self-leave only** with both names=self initially. A button labelled Disband must not silently remove another selected row. |
| 6, move subgroup | leader_name=member, parameter=destination0–11; out-of-range uses unassigned branch. | Later leader-only control; do not guess a subgroup-leader opcode. |
| 8 / 9, lock/unlock | Acts on caller's raid. | Later leader control with pending state until17/18. |
| 30, make leader | leader_name=new leader. Handler explicitly checks caller is current raid leader. | Later control using current roster identity. |
| 20/32, loot type; 21/33 add looter; 22/34 remove looter | parameter is loot type, or leader_name is looter. | Defer until source-backed loot policy and capture coverage exist. |
| 35 / 36, set MOTD/note | 1164/204-byte records; note target in leader_name. | Display-only initially. Current server does not broadcast empty MOTD/note values, so empty-edit confirmation needs separate handling. |

There is **no implemented raid decline handler**. `RaidCommandInviteFail=31`
exists only in the enum; unhandled actions return NYI. The first Decline/Dismiss
control therefore dismisses that invitation locally without sending a guessed
packet or altering membership. A new invite gets a new local token.

Some raid administrative handler branches lack the explicit permission check
present in MakeLeader. This is not authorization to expose them to every
member. Use conservative local leader gating and retain server authority; keep
removing others, subgroup management, loot policy and leadership abilities out
of the first slice. No tests should probe permissive branches against others.

## Guild wire evidence

Sources: `common/patches/rof2.cpp:1764` and `rof2_ops.h`;
`common/eq_packet_structs.h` guild structs; `common/guilds.h`;
`zone/guild.cpp`, `zone/guild_mgr.cpp::MakeGuildMembers`;
`zone/client_packet.cpp::Handle_OP_Guild*` and `Handle_OP_GetGuildMOTD`.
RoF2 defaults to pass-through for opcodes without a registered translator
(`StructStrategy` and RoF2 `Strategy::Strategy`). Therefore common structs and
actual senders override conflicting historical RoF2 header comments.

### Identity, full roster, and descriptive state

`PlayerProfile`'s variable encoder provides the local guild ID u32 and rank u8:
after the counted language bytes, skip zone/instance4 + position16 + four flag
bytes, then read guild ID and rank. Continue with the encoder's remaining
unknown9 + experience8 + eye-height1 before bank currency. This replaces a
portion of the existing aggregate skip, not a guessed global profile offset.
Spawn encoding likewise provides guild ID/rank u32/u32 after the holding byte
and deity. NPCs receive guild ID `0xffffffff` and rank0. Appearance types22/23
update guild ID/rank; type52 controls displayed tag. `GUILD_NONE=0xffffffff`.
Guild rank0 means none; ranks1–8 run Leader, Senior Officer, Officer, Senior
Member, Member, Junior Member, Initiate, Recruit. Do not reuse Titanium0/1/2.

| Message | Wire opcode / layout | Interpretation |
| --- | --- | --- |
| Guild directory | `0x507a`; zero prefix64, count u32LE@64, repeated ID u32LE + terminated name | Maps IDs to names. Current encoder filters IDs to below50000; legacy1500-entry struct is not this wire format. Directory membership says nothing about the local player. |
| Guild roster | `0x12a6`; terminated prefix, four reserved bytes, count u32BE, variable rows | Prefix is the guild name on current `SendGuildMembersList`, and can be player name on older `SendGuildMembers`. The alleged guild-ID four bytes are skipped without initialization by the RoF2 encoder. **Never trust, validate as zero, log, or bind identity from them.** Bind roster to independently known current guild/session. |
| MOTD / requested MOTD | `0x3e13` / `0x4f1f`; 648 bytes from common sender: unknown u32, recipient[64]@4, setter[64]@68, unknown u32@132, MOTD[512]@136 | No RoF2 translation registered. Header's variable-length MOTD declaration does not describe this sender. An unguilded player receives empty setter/MOTD. |
| URL/channel | `0x2958`, action0/1; 4176 bytes, text[512]@80, reserved tail3584@592 | Display data only; a guild's stored channel name does not establish UCS channel membership. |
| Rank names | `0x2958`, action4; common union packet592 bytes, rank u32@80, name[76]@84 | Sent once per configured rank. Preserve server text, do not hardcode customization away. |
| Rank permissions | `0x2958`, action5; 92 bytes, guildID u32@76, rank u32@80, function u32@84, enabled **u8**@88, reserved3@89 | Sender includes trailing bytes `2c 01 00`; do not decode permission as a u32 boolean. 8 ranks ×30 functions can arrive on entry. |

Each full roster row is two terminated strings plus52 numeric bytes:
name; ten u32BE values (level, banker/alt bits, class, rank, last-online Unix
time, tribute enabled, unknown0, total tribute, last tribute, unknown1); public
note; instance u16BE, zone u16BE, unknown u32BE, unknown u32BE. The encoder sets
instance0; this is not proof of a real instance. `MakeGuildMembers` packs banker
in bit0 and alt in bit1, and sets zone0 when the database member is offline.
This is server snapshot status, not a guarantee that it remains current.

Use bounded strings and checked count/remaining-length arithmetic. No guild
member-cap constant was established here: set a documented client resource
ceiling independently of the72-person raid limit, and label it as a client
limit. Do not conflate maximum guild ID, directory count, and member count.
Validate known semantic fields and complete records while consuming reserved
bytes without inventing meanings. Never expose raw ignored roster bytes in
diagnostics; the sender leaves some uninitialized.

### Guild incremental events

The following modern common structs pass through unchanged; integers are LE.
Updates must match current guild ID and stable member name, and stale updates
after leave/session change must not reconstitute membership.

| Opcode | Bytes | Fields |
| --- | --- | --- |
| Member add `0x2925` | 104 | guildID@0; reserved12@4; level/class/rank/tag/zone/last-on u32@16/20/24/28/32/36; name[64]@40 |
| Member delete `0x3141` | 68 | guildID@0, name[64]@4; own removal is followed by guild-none appearance |
| Member rename `0x3b26` | 132 | guildID@0, old[64]@4, new[64]@68 |
| Level `0x1bd3` | 72 | guildID@0, name[64]@4, level@68 |
| Rank/alt/banker `0x0b9c` | 80 | guildID@0, rank@4, name[64]@8, flags@72 (bit0 banker, bit1 alt), reserved/offline@76 |
| Public note `0x01f9` | 324 | guildID@0, name[64]@4, note[256]@68 |
| Member details `0x69b9` | 80 | modern: guildID@0, name[64]@4, zone u32@68, last-on@72, offline mode@76 (1 offline) |
| Guild rename `0x61db` | 68 | guildID@0, name[64]@4 |
| Guild deleted `0x6dab` | 4 | guildID@0 |

Two shared-opcode cautions need captured coverage. `0x69b9` is also the older
translated GuildMemberUpdate: zone u16@68, instance u16@70, last-seen@72,
zero@76. The current legacy sender sets instance0, so the zone value agrees
with modern u32 under this source, but a future nonzero instance cannot be
silently combined into a zone ID. `0x0b9c` also names SetGuildRank, whose encoder
sets final u32 to1. Do not read that final field as authoritative online status.
Modern rank updates leave it unset/zero and use the bit flags at72. A known
member's details/update must not invent a new guild affiliation.

Profile and roster rank are suitable capability inputs. Appearance rank goes
through `GetDisplayedRank` (currently simply returns the same rank), but remains
presentation data. Prefer explicit rank-permission events for allowed actions.
Permission7 is invite,8 promote,9 demote,10 remove,12 public-note editing,
18 MOTD,19 read guild chat,20 speak guild chat. Do not grant all capabilities
merely because a row says Officer. Missing permission data means unknown.

### Guild requests and size discrepancies

- **MOTD refresh:** send empty `OP_GetGuildMOTD=0x36e0`. Handler does not read
  payload and sends GetGuildMOTDReply plus URL/channel if guilded. This is a
  verified read-only control.
- **Roster refresh:** no verified client request in this investigation.
  `OP_GetGuildsList` requests the directory, not membership, and is absent from
  the shipped RoF2 mapping. `GuildOpenGuildWindow=0x0276` has no located handler.
  Do not wire a Refresh Roster button to either by guess; initial roster is
  receive-driven on entry and membership changes.
- **Invite `0x7099`:** common handler requires136 bytes: other[64]@0,
  self[64]@64, guildID u16@128, reserved2, proposed rank u32@132. It also forwards
  that packet as the invitation. RoF2 header declares an additional u32 (140
  total), but no RoF2 GuildInvite decoder/encoder is registered. This checked-in
  server therefore expects136. Capture and validate deployed behavior before
  enabling it. The same opcode can promote/demote an existing guildmate;
  initial invitation UI must reject already-guilded targets locally.
- **Reply `0x7053`:** common/RoF2 both136: inviter[64], invitee[64], response
  u32@128, guildID u32@132. Response is the offered rank1–8 when accepting,
  **9** for decline (`GUILD_INVITE_DECLINE`), not a boolean. The handler expects
  both players in the same zone. Use only fields from the current pending
  invitation. Wait for authoritative roster/appearance changes rather than
  assuming the reply grants membership; the inspected handler calls the invite
  verification helper without checking its return value, so local stale-reply
  rejection remains necessary.
- **Self-leave `0x1444`:** RoF2 decoder explicitly requires140 and translates
  to the common136-byte GuildCommand. Put self in both name fields. Never reuse
  the136-byte invite encoder for leave. Wait for own delete/guild-none event.
- **Status `0x7326`:** RoF2 request140, name[64] then reserved76; translated to
  common136. Returns chat status text, not a structured roster/membership ack.
- **MOTD edit `0x0b0b`:** current common648-byte layout, permission18 required.
  Guild leader transfer `0x7e09` uses two64-byte names and requires current
  leader plus same-zone target. Public note `0x5053` uses common388 bytes, not
  the stale100-byte note in RoF2 header. These and promote/demote, rank names,
  permission mutation, guild creation/deletion, tribute and guild bank are
  later slices with separate authorization/fixture coverage.

## First implementation and acceptance order

1. Add portable checked codecs and reducers without exposing administrative
   buttons. Cover all prefixes/truncations, unterminated names, action-specific
   lengths, wrong byte order, duplicate/stale member updates, unknown actions,
   capacities, reserved bytes, guild-none and same-opcode variants. Decode
   guild identity from the existing profile/spawn cursor rather than altering
   unrelated profile offsets. Unknown or malformed social packets must not
   replace a valid roster with an invented empty one.
2. Show raid member name, actual class/level, subgroup, group leader, raid leader,
   lock and MOTD; show guild identity, roster, server status, ranks and MOTD.
   Distinguish not-yet-known, rebuilding and confirmed absence. Never show an
   invented complete-roster count, offline state, raid ID or privilege. A raid
   session can use a local generation because these updates carry no raid ID.
3. Bind raid invite/current-invite accept/local dismiss, self-leave, and `/rsay`
   using the existing chat packet and channel15. Add guild MOTD refresh and
   preserve existing `/guild`. Keep command completion driven by events, with
   pending/error/timeout presentation; double clicks cannot queue duplicates.
4. Run the dedicated two-character raid proof, then guild read proof. Resolve
   invitation size and shared-opcode cases before guild invite/decline/accept/
   self-leave. Only then extend management controls with their own proofs.

EQEmu routes guild channel0 and raid channel15 through zone/world; this does not
require UCS. `zone/worldserver.cpp::ServerOP_RaidSay` deliberately excludes the
sender, so local sender echo is a client presentation behavior, not a server
delivery acknowledgment. Live tests send in both directions and require each
other member's inbound line. Group channel2 inside a raid routes to the caller's raid subgroup,
so preserve it. Custom numbered channels are a separate UCS session milestone:
world emits SetChatServer/SetChatServer2 as a terminated
`host,port,server.character,type+hex-session-key` string; OpenEQ currently drops
it. UCS has separate login/opcodes/channel lifecycle and private transient keys.
Do not simulate `/join` through ordinary Say or treat a guild channel label as
an active subscription. This plan makes no claim of UCS availability or support.

## Dedicated live fixture safeguards

Use only two newly isolated social-parity accounts/characters, or the existing
Fellowship/Companion social fixtures after explicit coordination and a fresh
state check. Do not use Explorer, Reviver, Rezzer, established player characters,
or another task's active fixtures. No need for items, combat, currency, spells,
quests, movement, bots, or normal groups in the first proof.

Before any future fixture setup, record the deployed RoF2 opcode mapping and
source/build identity. Snapshot offline character pose/resources, inventory,
currency, binds, spells/buffs, XP/stats, corpse counts, group membership, guild
membership and all related raid/guild rows in private create-new files. Check
table columns against the deployed schema rather than assuming the local
generated repository is an exact schema dump. Abort if a selected social group
contains any nonfixture member; never restore/delete such a shared group.

For raids, start both fixtures ungrouped and unraided in the same quiet zone.
Invite B from A; B locally dismisses; verify no raid was created. Invite again,
accept once, and reject a stale/double accept. Both reducers must show the same
two members/leader and agree with `raid_members` (charid/name, raidid, groupid,
israidleader/isgroupleader). Send one explicit fixture-only raid chat message;
the other client must receive it on channel15; repeat in the opposite direction.
Reconnect one fixture normally to
exercise create/add/rebuild and retained membership. Request self-leave from B,
observe remove and disband, then self-leave from A; verify no fixture membership
remains. Do not claim a single self-leave disbands everyone.

For guild receive proof, provision only an isolated fixture guild through the
server's supported administration path in a later implementation task; keep
its ID and exact member set as cleanup guards. Give A leader rank and B a
nonprivileged rank; no guild bank assets/tribute. Enter both and compare roster,
ID/name, MOTD/setter, eight rank names and received permission matrix against
`guilds`, `guild_members`, `guild_ranks`, and `guild_permissions`. Refresh MOTD
through the client and verify the reply. Log one member out and back in to check
status/update behavior; capture only social payloads, excluding login/world/UCS
credentials and ignored uninitialized bytes. A later mutation proof starts B
outside the guild, captures136/140-byte invite behavior, tests decline then new
invite/accept, confirms server membership and guild chat, then B self-leaves.
Guild permission and rank-change tests require explicit scenarios beyond that.

Normal logout must complete before restoration. Confirm every fixture offline;
then restore exact saved character state and verify independent gameplay
invariants. Raid cleanup must cover `raid_members`, `raid_details`, and any
leadership rows created for the **recorded fixture-only raid ID**. Guild cleanup
must cover only the recorded isolated guild and dependent rows. Use supported
disband/delete paths and verify caches and database agree; do not directly
delete live guild/raid rows or perform global table cleanup. Preserve any fixture
guild deliberately retained for future tests and document that choice.

Evidence to record: sanitized social event sequence/lengths, both foreground
reducers, selected action identities/tokens, server database membership and
offline results, and restored invariant checks. XML/GPU captures must use
synthetic or dedicated-fixture social text. No acceptance claim is complete
from fabricated packet fixtures alone; live results and unresolved server
quirks must be recorded separately when that work is performed.

## Live raid proof, 2026-09-29

`raid_smoke` exercised the real foreground `LiveWorld` reducer and
`LiveWorld::raid_request`, with `/rsay` submitted through `Interaction::submit`.
It was restricted to `storage2.daeken.dev`, accounts `openeq_social1` and
`openeq_social2`, characters **Fellowship** and **Companion**, in PoK202 instance0.
No Explorer, Reviver, Rezzer, or other characters were used. The probe checked
the deployed `/srv/eqemu/patch_RoF2.conf` entries before login: RaidInvite0x55ac,
RaidUpdate0x3973, RaidJoin0x0000, ChannelMessage0x2b2d.

The successful fourth run used the final request-token and worker membership
generation safeguards, with the corrected136-byte action10 decoder. It ran at
07:54–07:55 UTC and exited0. Observations:

- Fellowship invited Companion; a repeated invite was rejected locally.
  Companion dismissed the current invitation locally; no SQL raid appeared,
  and accepting that dismissed token failed. After the real inviter debounce,
  a new invitation arrived; the old token failed and the current token was
  accepted once, with a second accept rejected.
- Both reducers showed the same two level10 warriors, Fellowship as leader,
  and unassigned subgroups. The new raid was ID3004; its exact two-member set
  and leader agreed with `raid_members`.
- Each character submitted one `/rsay` message, and the other character
  received the exact channel15 line in its foreground chat history. This
  proves two actual remote deliveries independently of sender echo.
- Companion could not transfer leadership while a nonleader. Fellowship
  transferred leadership to Companion once; a duplicate transfer and an old
  roster-revision leave failed. Both reducers and SQL confirmed Companion.
- Companion logged out normally and reconnected. SQL membership persisted,
  the fresh reducer first saw the136-byte zone-entry reset, then rebuilt the
  two-member roster with Companion still leader.
- Fellowship self-left, receiving disband while Companion retained a confirmed
  one-member roster. Companion then self-left. SQL membership was empty.
  Repeated leave requests were rejected. No movement packets were sent.

Private mode0600 evidence is in `/tmp/openeq-raid-foreground-4.log`, with
`.baseline.txt` and `.restore.sql` siblings. The journal contains deployed
opcodes, original raid IDs, recorded fixture raid ID, full inventory evidence,
and each offline gameplay baseline. Reproduce only after coordination and
using a fresh output path:

```sh
cargo run -p openeq --bin raid_smoke -- \
  /Users/daeken/.config/openeq/storage2-social1-credentials.json \
  /Users/daeken/.config/openeq/storage2-social2-credentials.json \
  /tmp/openeq-raid-foreground-NEW.log
```

The successful run left both fixtures offline, ungrouped, unraided, unguilded,
and restored exact pose, HP, mana, endurance, hunger, and thirst. Inventory
items/slots/charges/augments/custom data/ornaments, currency, binds, spells,
buffs, corpses, levels, XP, and stats compared unchanged. Inventory `guid` is a
session item serial regenerated by `SharedDatabase::GetInventory`
(`common/shareddb.cpp:810`); full before rows are recorded, but this one field
is excluded from gameplay equality rather than overwritten. No gameplay
inventory data is omitted from comparison.

Cleanup uses normal foreground self-leave, with a guarded reconnect/self-leave
fallback on failure. EQEmu leaves `raid_details` after an empty raid, so only
the recorded new raid ID's metadata is removed, after both fixtures are
offline and the raid has no members. IDs3003 (earlier attempt) and3004 were
independently confirmed absent from `raid_members`, `raid_details`, and
`raid_leaders`; no global tables or preexisting raid IDs were cleared. Exact
restored poses were Fellowship `(1005,-15,389)` and Companion
`(-280,-148,-159)`, heading0, HP338, mana0, endurance225, hunger/thirst6000.

Earlier attempts exposed two assumptions worth retaining as evidence:

- Runs1/2 stopped on the zone-entry action10 record before a raid was formed.
  The narrow run2 diagnostic recorded action10 length136 and the adjacent
  name layout. Source verification located `Client::SendZoneInPackets` and
  `ZoneInSendName_Struct`; the decoder and exact-size regression were corrected.
  Temporary packet diagnostics were removed. Run1 also found regenerated
  inventory serials; all actual gameplay data was unchanged, both fixtures
  were independently restored offline, and restoration now attempts both
  fixtures even if one invariant check fails.
- Run3 formed raid3003, with both rosters correct, and delivered Fellowship's
  chat to Companion. Its expectation of server echo to Fellowship timed out.
  `zone/worldserver.cpp:1599` explicitly excludes the sender; the probe now
  proves inbound delivery in each direction. Run3's guarded reconnect,
  self-leave, metadata cleanup, and baseline restoration all passed.

After the correction, all84 network library tests and strict Clippy for the
raid smoke passed; the probe was rebuilt before run4. UI/GPU validation and
sender-echo presentation are separate checks. This proof does not cover raid
subgroup moves, cross-zone invites, loot policy, leadership abilities, guild
mutation, or UCS custom channels.

## Completed receive-only guild slice and live proof

`openeq-net/src/guild.rs` implements the directory, full roster, MOTD and requested
MOTD response, member add/delete/rename/level/rank/flags/note/details, guild rename
and guild deletion. `GameplayEvent::Guild` carries checked typed events; profile
parsing now exposes `guild_id: Option<u32>` (only `0xffffffff` maps to `None`)
and raw `guild_rank: u8`. The existing own `SpawnAppearance` types22/23 establish
subsequent identity/rank changes. Zone entry already sends a directory for guilded
clients (`zone/client_packet.cpp:892`), so no world/session persistence was added.

Client safety limits are explicit and independent: directory50,000 entries and
4MiB, roster16,384 members and8MiB, names/prefix63 bytes, public notes255 and
MOTD511. These are client resource ceilings, not asserted server guild capacities.
Numeric member level/class/rank remain u32. Duplicate directory IDs and ASCII
case-insensitive member names, incomplete records, invalid names, and unexpected
trailers fail decoding without producing a replacement empty roster.

Neither the roster prefix nor its four uninitialized bytes is exposed, logged,
or used to establish membership. Missing add-packet flags/notes remain unknown.
Full-roster zone0 means offline snapshot; a nonzero zone is an online snapshot,
without an invented instance. Shared `0x0b9c` ignores its final field entirely.
Shared `0x69b9` exposes the common last-seen timestamp; mode0 leaves presence and
location unknown, mode1 is unambiguously modern offline, and other modes remain
opaque. Timestamp0 remains unknown. No `0x2958` URL/rank/permission variant is
implemented, and no outgoing guild API exists.

Portable validation passed **95 network library tests** and strict network
Clippy. Ten new guild tests cover source-shaped records, every prefix/trailer,
big-endian rosters, wide numeric values, name/note/count/byte limits, duplicate
keys, ignored reserved bytes, shared-opcode uncertainty and both MOTD opcodes.
An additional profile test varies language-array size and guild identity while
verifying currency/resource alignment and truncation rejection. Nonempty MOTD
and nonempty public-note decoding are portable-tested, not claimed as live proof.

The bounded `crates/openeq/src/bin/guild_smoke.rs` foreground proof passed on
2026-09-29, run2, exit0. It uses only Fellowship/Companion, existing dedicated
GM accounts, no movement/items/combat/spells/currency, and no outgoing guild
packet. Supported `#guild create`, `#guild set` and `#guild rename` commands
provisioned isolated guild1 with Fellowship rank1 and Companion rank5. Setup
name `OpenEQ Receive Setup` became `OpenEQ Receive Proof` to demonstrate the
server-driven directory change. Both fresh foreground reducers then matched
SQL identity/name, both level10/class1 member rows, rank, banker/alt flags and
empty notes. Both received the server's valid empty MOTD and author. Companion
logged out normally and reconnected, retaining server membership and rebuilding
its full guild state while Fellowship remained connected.

The sanitized trace established the real initial order:

```text
Profile guild identity → MOTD → full roster → MOTD → directory
→ shared member-details update (presence unknown)
```

The GM membership assignment sent own GuildID/Rank before its roster; later
duplicate appearance updates preserved that roster. The local source explains
why: `ZoneGuildManager::SetGuild` sends appearance immediately after `MemberAdd`,
before the world round trip later sends its directory/roster and duplicate
appearance. No speculative staging or binding to reserved roster bytes was
needed. The newly created leader initially received identity without a full
roster in this run; the probe deliberately validated the complete fresh-login
snapshot rather than inventing a complete membership view from incrementals.

Guarded `#guild delete 1` sent guild deletion, guild-none appearance and empty
directory events. Both reducers confirmed no membership. After normal logout,
the probe restored exact saved pose/resources and independently compared
inventory gameplay fields, currency, binds, spells, buffs, corpse counts,
level/XP/stats, group/raid/guild membership. Inventory GUID is excluded from
gameplay equality because EQEmu regenerates it during login; complete original
rows are privately recorded. Independent post-run SQL confirmed both offline,
Fellowship `(1005,-15,389)`, Companion `(-280,-148,-159)`, headings0, HP338,
mana0, endurance225 and hunger/thirst6000, with all seven guild-related tables
empty and no fixture group/raid memberships. No movement packet was sent.

Private evidence is `/tmp/openeq-guild-foreground-2.log` and its `.baseline.txt`
and `.restore.sql` siblings, all0600. It includes deployed opcode checks and
world/zone binary hashes, initial full character/inventory rows, guild rows,
sanitized typed events and foreground state. Temporary per-event runtime
diagnostics were removed after the capture; the permanent probe logs its
foreground guild state and never raw packet bytes.

Two fixture-only findings from the first attempt are documented rather than
hidden by the successful rerun:

- Run1 failed only its nonempty-MOTD expectation. A guarded SQL MOTD update
  followed by supported rename was overwritten by EQEmu's cached full-row
  `UpdateDbRenameGuild` (`common/guild_base.cpp:606`). The decoder and identity
  sequence were correct; the retry removed SQL MOTD setup and proves the valid
  empty server message. No outbound setter/refresh was added to broaden scope.
- EQEmu `_StoreGuildDB` (`common/guild_base.cpp:342–349`) constructs a zeroed
  `GuildTributes` row but never sets `gt.guild_id` before `ReplaceOne`. Creating
  the fixture guild consequently left an empty tribute sentinel under ID0,
  outside the recorded guild1. Baseline absence was recorded, and cleanup removed
  only the exact new row `(0,4294967295,0,4294967295,0,600000,0)`, after confirming
  no guild0/members and both fixtures offline. The probe now guards and reconciles
  that exact source-backed side effect. No global or live-guild deletion occurs.

This live proof does not cover guild invitations, leaving through a client
guild command, permission/rank configuration, guild bank/tribute, or UCS. Its
administrative setup and deletion are fixture preparation, not a shipped player
mutation interface.
