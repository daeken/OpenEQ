# Raid and guild UI: first usable slice

Initial read-only audit, 2026-09-29. The raid presentation implementation is
recorded below, followed by the receive-only guild checkpoint. The audit sections
record the original proposal; checkpoint sections describe shipped scope.
The XML observations below come from the installed default skin under
`/Users/daeken/EverQuest/uifiles/default`. Protocol constraints were coordinated
with the [parallel EQEmu protocol audit](SOCIAL_PARITY_PLAN.md); source verification is not a substitute
for that slice's planned packet captures and live acceptance checks.

## Recommended scope

Add two ordinary windows using the original `RaidWindow` and
`GuildManagementWnd` art. The raid window shows confirmed members, subgroups,
leadership, lock state and message of the day; it supports a separate raid
invitation prompt, acceptance, local dismissal and self-leave. The guild window
starts with identity, a selectable roster and read-only information/MOTD.
Enable only the controls whose packet and state behavior have been verified.

Opening and closing either window is a local presentation action. Receiving an
invitation is sufficient to open the raid window, but sending an acceptance
does not create membership. Closing a raid window never leaves the raid.
Guild promotion, removal, rank/permission editing, tribute, guild banking,
banners and UCS channel joining remain separate slices. Raid group management,
leader transfer, loot policy and delegate-role changes also remain deferred.
Guild invitations/replies/self-leave can follow once their captures validate
the protocol plan; leave those controls hidden in the initial read-only UI.

## Existing code to retain

- `social_ui.rs` currently draws the six-member group and invitation window.
  `social_interaction.rs` binds it to `game.group` and visible zone entities;
  `group.rs` reconciles server events. Keep this path distinct from a raid.
- `openeq-net/src/social.rs` handles group membership/invitations and chat item
  links; it does not yet supply a raid or guild roster. The current
  `PlayerProfile` does not expose guild membership fields.
- Guild chat already works through `/guild` and `/gu` on channel 0. Incoming
  channel 15 is already labeled Raid, and `ChatChannel::Raid` exists; outgoing
  `/rsay` needs both a parser alias and the channel-15 case in
  `Interaction::action`. Do not report raid chat as already complete.
- `Painter::shell`, named window composition, title dragging and layout storage
  already provide the required window behavior. Read-only STML scrolling now
  uses `WidgetState::scroll_rows`, `scroll_id`, `DrawCommand::TextArea` and actual
  renderer metrics. Reuse it for MOTDs and selected-member notes.
- Generic `Listbox` still lacks row/column data rendering. `TabBox` selects a
  page through bindings but does not provide a complete interactive tab strip.
  Existing account/commerce lists offer bounded row/hit patterns, but their
  action identities and input state must not be reused for guild or raid data.

## Original XML controls

All three source files are included by the installed `EQUI.xml`.

| Original control | First binding |
| --- | --- |
| `RaidWindow` | Logical window `raid`; original `WDT_Rounded` shell. Authored size 380 × 400. |
| `RAID_PlayerList` | Members with a verified subgroup; columns Grp, Player Name, Lvl, Class, Role and Raid Rank. |
| `RAID_NotInGroupPlayerList` | Members whose subgroup is the verified ungrouped sentinel; no fabricated “group 0.” |
| `RAID_PlayerCountLabel` | Number of currently known roster members, including ungrouped members. |
| `RAID_LevelAverageLabel` | Average only when level data and roster completeness justify it; hide while completeness is unknown. |
| `RAID_InviteButton` | Invite the current valid player target, if the verified command rules permit it. Named invitations use the chat command path. |
| `RAID_AcceptButton` | Accept the current raid invitation, with captured invitation identity. |
| `RAID_DeclineButton` | Label **Dismiss**: clear this local prompt; do not send an unsupported raid-decline packet. |
| `RAID_DisbandButton` | Label **Leave raid** and replace its original target-disband tooltip. The first action is self-only. |
| `RAID_MOTDViewer` | Read-only, scrollable text on an Information page. |
| `RAID_NoteList` | Optional selected-member notes once the corresponding server events are supported; no note editor. |
| `GuildManagementWnd` | Logical window `guild`; original `WDT_Rounded` shell. Authored size 600 × 410. |
| `GT_GuildNameLabel`, `GT_PlayerCountLabel` | Verified guild name and received roster count; filters show visible/total counts. |
| `GT_MemberList` | Name, level, class, rank and source-qualified presence/location. Alt/public-note data can appear in row details. |
| `GT_ShowOfflineButton` | Local filter; exclude only members known to be offline, retaining unknown presence. |
| `GT_HideAltButton` | Local filter using the verified alt bit; unknown alt state stays visible. |
| `GT_MOTDViewer`, `GT_MOTDAuthorLabel` | Scrollable message and author from the guild event, including an explicitly empty MOTD. |
| `GT_ProfilePage` | Information presentation. Add **Refresh message** using original button art and the verified MOTD-read request. |
| `GT_NoteList` | Public note as read-only selected-member detail; personal notes are not provided by the guild roster packet. |

The raid member lists also contain expedition/shared-task/flagging/voice
columns. Hide these until their data is supported. The guild list contains
last-on, public note and two narrow icon columns; do not infer what the
unlabeled columns mean. Its six pages cover members, notes, tribute,
information, banners and access. Only Members and Information need working
local page buttons in this slice.

`EQUI_RaidOptionsWindow.xml` supplies `RaidOptionsWindow`,
`RAIDOPTIONS_CurrentLootType`, `RAIDOPTIONS_LooterList`, a loot-type combo and
sixteen class-color controls. Keep this entire management window out of the
first slice rather than exposing nonfunctional settings.

Bind **unique XML item names**, not shared ScreenIDs. For example,
`RAID_MOTDViewer` and `GT_MOTDViewer` both have ScreenID `MOTDViewer`;
`MemberPage` and `Subwindows` also repeat. Neither ScreenID nor XML template
name is the logical window identity or a protocol address.

## State required from the protocol layer

Use separate `RaidState`/`GuildState` resources and renderer-neutral views. Each
needs a session/membership identity and a monotonically changing presentation
revision, plus explicit readiness. A missing packet is not evidence of no
membership. Render “Waiting for raid information” or “Waiting for guild
information” until absence or membership is actually established.

Raid view fields: membership/readiness, leader name, known member records,
optional lock state, optional MOTD, pending inviter/invitee identity, pending
local command status and any validated action capabilities. A member needs a
stable canonical name, subgroup, real level/class and group-leader/raid-leader
flags; optional roles/notes must remain unknown until their events arrive.

The verified wire subgroup is 0–11 or `0xffffffff`; display groups 1–12 and
Ungrouped. Raid add records carry real class/level but **no online flag**.
Visible player spawns can support an “In this zone” hint and targeting, but
absence from the spawn map is never “Offline.” Match only non-NPC, non-corpse
player spawns by validated name when resolving a target.

EQEmu sends bulk raid rosters as repeated additions, not the advertised action-6
full-list structure. Create action 8 can start a rebuild, including during a
subgroup move, and there is no explicit end marker. Remove action 1 may be
followed by an add for a subgroup move; the actually removed client also gets
disband action 5. The UI must not announce global departure, erase the raid
identity or optimistically declare a complete snapshot from those partial
events. Count known rows while rebuilding; do not invent a completion timer.

Guild view fields: independently verified identity/name, membership readiness,
roster revision, member records, MOTD/author, rank names and received rank
permissions. A member can expose name, level, class, numeric rank, optional rank
label, banker/alt flags, last-seen timestamp, public note and presence/location
with source semantics preserved. Use the server's rank name when available;
otherwise display `Rank N`, not a guessed officer/leader title.

The RoF2 full-roster guild-ID slot is uninitialized in the EQEmu encoder and
must not select the current guild. Obtain identity from verified profile,
appearance/dynamic events and directory/name data as the protocol plan defines.
The full roster's first string may be a guild name; it is not inherently the
local character name. Banker bit 0 and alt bit 1 are verified roster flags.
Profile/full-roster rank and received permissions are the capability sources;
do not depend on a display-only appearance value to grant actions.

Guild presence also needs careful decoding: full snapshots use zone 0 for
offline, while opcode `0x69b9` has modern and legacy-translated layouts with
different zone/instance/offline semantics. Bind the normalized, verified state,
not a UI test of the last integer or an inference from zone visibility. Show
unknown locations/timestamps as unavailable; no fabricated last-login dates.

The verified `GetGuildMOTD` request can refresh the message and associated
information. There is no verified full-roster refresh request in this slice:
`GetGuildsList` means the guild directory, and opening a guild window is not
proven to request its roster. Membership rows remain receive-driven.

## Bounded table and page implementation

Use the original shells and the named inner list/STML controls, hiding unused
children explicitly. Enlarge the raid's initial presentation to roughly
680 × 500 logical pixels when the viewport permits; its authored list columns
already exceed the 380-pixel root width. A guild window around 680 × 480 permits
readable names/ranks. Clamp windows to the viewport using the existing helper;
on narrow screens prioritize name/subgroup/status and put other fields in
selected-member details. Do not squeeze text to illegibility.

Add a narrow shared read-only roster helper for these windows: fixed-height
single-line rows, clipped cells/headings, stable row keys, selected-row highlight,
vertical scroll and visible-row hit targets only. Keep presentation work bounded
to the visible page (for example, at most 24 rows), while retaining the complete
protocol-validated roster. The raid cap is 72; the guild's accepted record bound
belongs to the protocol decoder, not an arbitrary UI truncation.

The SIDL schema defines `Listbox.Columns` with Heading, Width, Sortable, DataType
and Tooltip, but no runtime rows. A small row binding/owner-draw adapter is
sufficient; full editable lists, movable columns and drag/drop group assignment
are not prerequisites. Use the installed column labels/art and explicit data
bindings. Local name/subgroup sorting and guild offline/alt filters are enough;
reset or clamp scroll when the result set changes. Long cell values remain
available in a read-only detail pane or tooltip instead of disappearing.

Reuse the measured `TextArea` path for guild/raid MOTDs and long public notes.
Give each text viewport a distinct content identity, including membership and
content revision, so old renderer feedback cannot clamp another guild's text.
Treat all server text as display data; it cannot provide UI actions, executable
markup, asset paths or navigation. URLs/channel names, if displayed, are plain
read-only text; UCS Join and browser View remain hidden here.

## Input, command and persistence integration

Add local `/raidwindow` and `/guildwindow` commands and help entries. Preserve
`/guild MESSAGE` and the existing group `/invite`, `/accept`, `/decline` and
`/leavegroup` meanings. Raid invitations need distinct actions/commands; an
ambiguous `/accept` must not silently choose between a group and a raid prompt.
Add `/rsay` and `/rs` for channel 15. New hotkeys are unnecessary for this first
slice; Alt-based hotkeys would need explicit protection against existing
gameplay shortcuts and chat ownership.

Row clicks select a stable member key, not a row index. Bind mutation hits to
the displayed membership/revision and revalidate target, pending invitation,
capability and connection state at dispatch. An incoming sort, removal or guild
change must not redirect the user's click to another person. Selection and
viewing send no command. If a Target button is supplied, disable it unless the
selected member resolves to a current player spawn; it cannot target someone
in another zone. Raid acceptance/self-leave keeps the received membership
until server confirmation, with a pending status instead of an optimistic roster.

Keep every roster/MOTD/button/title/body hit inside logical windows `raid` and
`guild`, add both to `valid_window_id`, and compose them before tooltips/cursor
presentation. Clicking raises the complete window; scrolling alone does not.
The displayed topmost hit owns the wheel, so roster/MOTD scrolling cannot also
move chat or the map behind it. Hidden pages and filtered-out rows emit no hits.

Opening either view does not focus chat or capture keyboard movement. Reading,
scrolling and closing remain local; mutating actions follow the existing live
connection/death/recovery gates. Recovery keeps final hit ownership and Escape
must not accept an invitation or send a leave/disband request. Close/Escape only
hides the selected local window. Reuse existing window-position/stack persistence
for the same world/character; membership, invitations, selected member, open
state, scroll offset, MOTD and permissions are never restored from that file.
Session changes invalidate all transient social state and row actions. Zoning
reconciliation belongs to the protocol state machine, not window visibility.

## Acceptance criteria

- Synthetic UI/state fixtures cover unknown, confirmed-none, active/rebuilding,
  invitation, pending acceptance/leave, subgroup move and real disband states.
  No partial raid rebuild is rendered as a confirmed global departure.
- Snapshot/delta updates, filtered selection, sorting, duplicate/case-variant
  names and member removal preserve identity. A stale row/action revision sends
  no packet and cannot act on the new occupant of a recycled row.
- Capture command queues: opening/closing/selecting/scrolling and local raid
  Dismiss send nothing; Accept/self-leave use only the verified wire command;
  guild Refresh message cannot accidentally request or fabricate a roster.
- Table tests cover 72 raid members, the guild decoder's supported bound,
  grouped/ungrouped rows, unknown presence, filters, long names/notes, Unicode,
  clipping, scroll clamping and rows disappearing during a click. Only visible
  enabled controls receive hits.
- Original-asset tests load the named raid/guild definitions from `EQUI.xml`.
  Capture populated Members and Information views at 1× and 2×, including the
  last scroll page and overlapping windows, with no missing-template warnings.
  Inspect readability, target ownership, close controls and MOTD clipping.
- Root's later live social fixture verifies that actual server updates drive
  the same views across invitation/acceptance, movement between subgroups,
  zoning, self-leave and guild roster/MOTD delivery. Screenshots or synthetic
  fixtures alone cannot establish protocol completion.

## Raid presentation checkpoint

`crates/openeq/src/raid_ui.rs` now supplies the original raid shell, responsive
member table (at most 12 visible rows), stable name selection, invite/accept/local
dismiss/self-leave controls and a measured, scrollable MOTD page. Every action
carries the displayed revision; invitation actions also carry their token.
Pending acceptance disables both invitation buttons. Membership and capabilities
come from the live adapter. The logical `raid` window participates in the normal
saved position and stack order; close is separate from leave.

Four presentation tests cover action identity/ownership, disabled controls,
roster bounds and scrolling, hidden pages, close and overlapping window order.
An original-asset GPU test covers Members, invitation, pending and MOTD top/bottom
at 1× and 2×, plus narrow views with long Unicode names. Captures are written
when `OPENEQ_UI_CAPTURE_DIR` is supplied. These validate presentation only;
the coordinated reducer, protocol and later live fixture establish behavior.


## Guild presentation checkpoint

`guild_ui.rs` and `guild_interaction.rs` implement the original
`GuildManagementWnd` shell with Members and Information pages. `/guildwindow`
toggles it; `/guild` and `/gu` remain chat. The received roster displays names,
levels, classes, numeric ranks and source-qualified presence. A selected member's
public note, last-seen age and alt/banker flags use a measured scrolling detail
pane; unknown values remain unavailable. Received-empty MOTD and an absent MOTD
are different states. No guild command API or refresh control is exposed.

Local offline/alt filters retain unknown rows. Revision and member-name identities
protect selection and note scrolling when the roster changes. The adapter caches
transformed rows with shared storage; draw/hit output is bounded to visible rows.
Window positions and stacking use existing character/world layout persistence.
Closing, reading, selecting and filtering send no gameplay packets.

Six presentation tests and 22 original-skin GPU captures cover normal/Retina/narrow
views, filtering, long Unicode names, long notes and MOTDs at both ends, unknown
membership, guildless state and empty rosters/messages. Reducer/adapter tests cover
identity changes, travel, own departure, duplicate appearance, partial metadata
and stale selection/scroll feedback. Captures: `/tmp/openeq-guild-ui`.
Live fixture results and final combined checks are recorded in
`SOCIAL_PARITY_PLAN.md` and `OVERNIGHT_2026-09-29.md`.
