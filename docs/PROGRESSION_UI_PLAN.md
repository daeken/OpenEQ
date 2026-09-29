# Skills, experience and command hotbuttons

Read-only investigation of the installed client XML and current OpenEQ code,
2026-09-29. This document proposes milestone 8; it does not implement UI,
send gameplay commands, or establish training support. Packet semantics belong
to the companion `PROGRESSION_PROTOCOL_PLAN.md`.

## Smallest useful slice

1. A receive-only Skills window opened by `/skills` or the inventory Skills
   button, with Skills/Languages pages showing server values and original names.
2. An inventory progression strip showing the authoritative level and normal
   experience progress when known. Keep AA, vitality and training separate.
3. A subsequent slice: one draggable bar with twelve user-configured commands.
   Clicking executes the existing command dispatcher once; editing and loading
   preferences never execute a command. Initial slots are empty.

This does not require skill-use packets, trainer interactions, multi-line
socials, automatic macros, new keyboard bindings, or spell/item drag-and-drop.

## Original controls and art

Sources below are under `/Users/daeken/EverQuest/uifiles/default`. XML sample
text is presentation data, not authoritative character state.

| Source | Relevant controls | Authored geometry/art |
| --- | --- | --- |
| `EQUI_SkillsWindow.xml` | `SkillsWindow`, `SKLW_SkillList`, `SKLW_DoneButton`, `SKLW_MakeHotKeyButton` | 300×200, `WDT_Filigree`; list columns Skill Name 130, Rank 80, Value 60; normal buttons |
| `EQUI_SkillsSelectWindow.xml` | `SkillsSelectWindow`, `SKSW_SkillSelectorList` | 200×240, `WDT_Def`, one 200-wide name column; selection UI only |
| `EQUI_Inventory.xml` | `IW_Level`, `IW_NextLevel`, `IW_ExpGauge`, `IW_Skills` | Level/class EQType 2; normal XP EQType 4, 118×11; Skills button 45×20 |
| `EQUI_HotButtonWnd.xml` | `HotButtonWnd`, `HB_Button1`…`HB_Button12` | 525×53, `WDT_RoundedNoTitle`; each `HotButton` is 37×34 |
| `EQUI_SocialEditWnd.xml` | `SocialEditWnd`, `SEW_NameInput`, `SEW_Line0Input`…`SEW_Line4Input`, `SEW_Clear_Button`, `SEW_Accept_Button` | 355×240, `WDT_Def`; five command lines and twenty name-color buttons |
| `EQUI_TrainWindow.xml` | `TrainWindow`, `TRNW_SkillList`, `TRNW_ExpGauge`, `TRNW_PracticeCount`, `TRNW_TrainButton` | 580×280, `WDT_Rounded`; skills and four coin columns; training mutation deferred |

`IW_ExpGauge` uses `A_GaugeBackground`, `A_GaugeFill`, `A_GaugeLines`,
`A_GaugeLinesFill` and end caps. Fill tint is gold `(220,150,0)`; line-fill
tint is blue `(0,80,220)`. Reuse these assets through the existing gauge path.
The installed PlayerWindow has no XP gauge. Current inventory rendering hides
and repositions original pieces, so simply loading its XML will not make the
progression controls visible; add them deliberately to its application layout.

Remove the `IW_Level` example “60 Shadow Knight” before displaying the window.
Likewise, `HB_HorizontalCurrentPageLabel` and `HB_VerticalCurrentPageLabel`
contain sample “10”, and training includes sample practice count 255 and coin
amounts. None may survive as character data. Hide `IW_AltAdvGauge` (EQType 5),
`IW_VitalityGauge` (147), and `IW_AAVitalityGauge` (148) in this slice.

## Skill identity and names

Use explicit numeric skill IDs as row identity. The authoritative enumeration
is `/Users/daeken/projects/EQEmu/common/skills.h`, `EQ::skills::SkillType`.
Its comments map skill IDs to the installed `eqstr_us.txt` string IDs. The
mapping is not `13855 + skill_id`, and `dbstr_us.txt` is unnecessary here.

The complete 78-entry string-ID catalog, ordered by skill ID 0 through 77:

```text
 0– 9: 13855,13856,13857,13858,13859,13861,13862,13863,13864,13866
10–19: 13867,13871,13872,13874,13875,13876,13877,13878,13879,13880
20–29: 13881,13882,13883,13884,13885,13886,13888,13889,13890,13891
30–39: 13893,13894,13895,13896,13897,13899,13900,13903,13904,13905
40–49: 13906,13908,13909,13910,13911,13912,13913,13914,13915,13916
50–59: 13917,13919,13920,13921,13923,13854,13853,13852,13851,13850
60–69: 13865,13918,13907,13870,13887,13873,13860,13868,13892,13901
70–77: 13898,13922,13869,13902, 5837, 3670,13049,  789
```

Useful noncontiguous checks are Fishing `55→13854`, Alchemy `59→13850`,
Frenzy `74→5837`, Remove Traps `75→3670` (installed text “Remove Trap”),
Triple Attack `76→13049`, and 2H Piercing `77→789`. `SkillCount=78` is
not a skill. Tail Rake is an alias for Dragon Punch at ID 21, with alternate
string 13924; do not insert an extra row and shift subsequent IDs. Retain the
canonical Dragon Punch name until race-specific alias behavior is verified.

Add a plain read-only lookup to `game::StringTable`, or a small skill-name
catalog using it. Its existing loader handles Windows-1252 and the
`eqstr_us.txt`/`eqstr_en.txt` fallback. Missing strings show `Skill N`; they
must not silently select a neighboring string. Do not commit original tables.

Language IDs come from EQEmu `common/eq_constants.h::Language`, checked
against `common/emu_constants.cpp::GetLanguageMap` and installed text. Their
explicit string-ID catalog, ordered by language ID 0 through 26, is:

```text
 0– 9: 3114,3200,3201,3202,3203,3204,3205,3206,3207,3208
10–19: 3217,3219,3220,3209,3210,3211,3212,3213,3214,3215
20–26: 3216,3218,3221,3222,3223,7658,7659
```

Language 27 is explicitly unknown in EQEmu; show `Language 27`, not a
guessed name. Missing other labels fall back to `Language N`. Alaran and Hadal
are IDs 25 and 26, strings 7658 and 7659. These names do not require outgoing
chat-language selection. Reuse the SkillsWindow shell with local Skills and
Languages page buttons, two columns and one scroll position reset on page
change. This switch is an application addition, not an authored XML tab.

## Receive-only state and presentation

`PlayerProfile` already retains `level: u8` and `skills: Vec<u32>`. Current
profile parsing bounds the counted skill array, while the RoF2 encoder sends
100 slots. Named skills are only IDs 0–77. Retain bounded unknown/padding slots
for diagnostics, but do not manufacture a “None” or “SkillCount” row from them.

Profile and SkillUpdate values are base `m_pp.skills` values. They do not
include item modifiers and do not establish class eligibility, current caps,
maximum attainable values, or rank names such as Master. The first view uses
two columns, Skill Name and Base Value; hide the unsupported Rank column.
Show a received zero as zero. Missing profile values display unavailable;
neither missing nor zero means “this class cannot learn this skill.”

Populate the catalog in numeric ID order and allow local scrolling. Bound
visible rows and clamp scroll after snapshot replacement or resize. A row's
identity is its numeric ID and page, never its displayed index. Done closes.
Hide Make Hotkey until a selected skill has a verified action path; existing
`/cast` and `/attack` do not establish a generic skill-use command.

Protocol routing must distinguish skill updates from language updates:
SkillUpdate IDs `100 + language_id` represent language gains. They must never
index the skills display. The inspected server supports language IDs 0–27;
the RoF2 encoder pads the profile's remaining four language slots with zero.
Display the same raw-value/unknown distinction on the Languages page, without
inferring a proficiency rank or duplicating the server's improvement messages.

For normal experience, keep absolute profile XP and level-progress ratio as
different types. Profile XP is an absolute `u64`; ExpUpdate and LevelUpdate
carry a progress ratio scaled to 330. Do not derive a percentage from absolute
XP using an assumed client XP table. Until a valid ratio is received, show an
unfilled/disabled gauge with “Experience unavailable”, not 0%.

For a valid ratio `r` in `0..=330`, gauge fraction is `r / 330`, and percentage
is that fraction times 100. Preserve out-of-range wire values in state, but
present progress as unavailable; do not reject the whole packet or retain an
old percentage as current. A server-confirmed zero is a valid empty bar.
The second ExpUpdate word is unassigned by inspected senders; it is not a
verified AA percentage. A LevelUpdate's `oldlevel` can be the character's
highest-ever level, so never use it to invent a level gain/loss notification.
Its ratio can be transient; later ExpUpdate replaces it in packet order.

Use a small progression reducer owned by character/session state. Profile
replacement seeds base values and authoritative level; subsequent skill/XP
events update that state. Clear previous-character values on a fresh session,
disconnect/recovery boundary, and zone profile replacement as appropriate to
the existing lifecycle. An update received without a valid current profile
must not rehydrate a stale character. Do not persist any progression values.

## Minimal shared receive-only UI contract

Add `GameHudState.progression: Option<UiProgression>`; `Some` can represent
an open, unavailable view before its profile arrives. Keep full-width levels
from the receive state instead of truncating a LevelUpdate to profile `u8`.

```rust
pub enum UiProgressionPage { Skills, Languages } // default Skills
pub struct UiProgressionRow {
    pub id: u32,
    pub name: String,
    pub value: Option<u32>,
}
pub struct UiProgression {
    pub revision: u64,
    pub status: String,
    pub current: bool,
    pub level: Option<u32>,
    pub experience_bar_units: Option<u32>,
    pub skills: Arc<[UiProgressionRow]>,
    pub languages: Arc<[UiProgressionRow]>,
    pub open: bool,
    pub page: UiProgressionPage,
    pub scroll: usize,
}
pub struct ProgressionAction {
    pub revision: u64,
    pub kind: ProgressionActionKind,
}
pub enum ProgressionActionKind {
    Page(UiProgressionPage),
    Scroll { rows: i32 },
}
```

The adapter supplies names, bounded rows sorted by ID, and a ratio in 0–330
or `None`. `current=false` visibly marks retained rows/level stale and disables
current XP progress. Status distinguishes loading, stale and available state.
Revision changes across session/snapshot replacement and invalidates old page
or scroll hits. UI code still bounds/clips supplied data and ratio defensively.
No selection field is needed while Make Hotkey and training are absent.

Use `UiAction::OpenSkills` for the inventory opener and
`UiAction::Progression(ProgressionAction)` for page/scroll controls. Existing
`CloseWindow("skills")` handles Done/close, and `BeginWindowDrag("skills")`
handles dragging. Proposed hit prefix is `progression:<revision>:...`, with
logical window ID `skills`. Only position/order persists. Page, scroll, open
state and server data do not. Root owns the reducer, names and interaction
adapter; UI owns this contract, painter, hit parsing and inventory strip.

## Command bar and editor

The original hotbar has ten root windows, reusing inner item names. Start with
`HotButtonWnd` only, one page with twelve slots and stable slot IDs 0–11.
Hide all page controls and labels. Later bars need their own logical identity;
XML `ScreenID` alone cannot distinguish their slots.

`HB_ButtonN` elements are `HotButton`, currently unsupported by the generic
layout renderer. Add the narrow Button-compatible painting/hit behavior before
using them. Their five-state templates are `A_HotButtonNNormal`, `Pressed`,
`Flyby`, `Disabled`, and `PressedFlyby`; 40×40 frames come from
`window_pieces06.tga` (1–6) and `window_pieces07.tga` (7–12). Honor the
existing hover/pressed/disabled path and draw the user label within each slot.

`HB_HotButtonLayout` is an unsupported `TileLayoutBox`, horizontal-first with
four-pixel spacing and `SnapToChildren`. Manually place twelve slots for this
slice and suppress the automatic tile children; a complete tile engine is not
required. Ignore the nested `HB_SpellGem` and `HB_InvSlot`; the latter's EQType
-2 is not a real inventory slot. Show a tooltip with the full label and command
when the small button cannot display them completely.

Left-click on a populated slot activates it once. Left-click on an empty slot,
or right-click on any slot, opens its editor. `/hotbuttons` reopens a closed
bar; `/hotbutton 1..12` opens an editor without relying on secondary-click
support. Add these as local presentation commands and explicitly prohibit
binding them recursively. This command syntax is proposed, not implemented.

Reuse `SocialEditWnd` art with title “Edit Hotbutton”, Name and Command fields,
Clear and Save. Hide lines 1–4 and all twenty color buttons. Save validates the
draft and changes only local configuration. Clear empties the draft; Save of
two empty fields clears the slot. Close or Escape cancels without saving.
Require both fields for a populated slot. An invalid draft stays open with a
plain error. Never execute a command as a preview or consequence of Save.

Validate a populated command as one slash-prefixed line, at most the existing
`MAX_CHAT_BYTES` (512 bytes), with no control characters. Run `chat::parse`
and require a supported action; a missing slash must not become a Say message.
Support existing dispatcher actions, excluding editor/bar management. `/audio`
is intercepted separately in main and is not part of this dispatcher, so it
is unsupported until routed explicitly. Map/camera keyboard actions likewise
do not become hotbutton commands merely because gameplay exposes them.

On activation, parse and validate again, then invoke the same
`Interaction::submit`/`action` path as chat with the current `LiveWorld`, target
and player position. Preserve its target/service/session/transaction guards.
Propagate the dispatcher's quit result to the same exit handler if `/quit` or
`/camp` is bound. Never construct packets independently or save spawn IDs,
target snapshots, gem contents, or inventory transaction state in a button.
`/useitem` means the currently inspected owned item, not a saved item binding;
its tooltip/help must say so. `/sit` sits; `/stand` stands; neither is a toggle.

Snapshot slot ID and a configuration revision for mouse-down/up validation.
Editing, clearing, reconnecting, window closure or slot replacement invalidates
a pending activation. A release over a changed slot must not run its newly
saved command. Existing gameplay readiness decides whether an action is sent.

## Focus, persistence and code ownership

Use mouse activation initially. Preserve existing Alt+1…0 gem shortcuts and
gameplay keys. A future number-key binding editor needs explicit conflict,
held-key and focus rules before it can own those inputs.

The XML Editbox rendering does not provide a text editor by itself. Give this
editor an explicit input owner and bounded UTF-8 editing using the existing
chat-editor primitives. Ordered raw window events, IME and repeat handling
must follow the current chat/account ownership model. Enter saves the draft;
Escape cancels the editor. Neither event may also enter chat, cast, target or
close an underlying window. Account/loading/recovery UI retains priority.
Opening or closing the editor must not leak held keys or releases to gameplay.

Use logical window IDs `skills`, `hotbuttons`, and `hotbutton_editor` with the
current WindowStack. Persist their position/order, not open state. Keep scroll,
selection, draft text, editing focus and activation revisions in memory only.

Concrete persistence choice: extend `ui_layout::Layout` version 1 with a
`#[serde(default)] hotbuttons` field containing exactly twelve optional
`SavedHotbutton { label, command }` entries. Older files default to empty slots.
Labels are bounded to 32 Unicode scalar values and 64 UTF-8 bytes, with no
control characters; commands use the bounds above. Validate on save and load.
An invalid stored slot becomes empty with one local diagnostic; it must not
execute or discard unrelated valid layout preferences.

Thread this field through `Layout::capture`/`apply` and all restore/update
callers so moving a window cannot accidentally erase saved commands. Reuse
the existing full identity check (host, login/world ports, server ID and
character), hashed filename, 64 KiB document limit, atomic write and 750 ms
debounce. Explicit Save may flush. Each restored command stays inert until
clicked; switching identities clears the old bar before applying the new one.

Suggested ownership: a progression reducer/catalog in the application crate;
`progression_ui.rs` for the Skills window and inventory strip; `hotbuttons.rs`
for configuration, draft editor and validation; existing `gameplay_ui`,
`interaction`, `main` and `ui_layout` for their current integration boundaries.
`openeq-ui` owns only the narrow generic HotButton rendering addition. Network
decoders and receive-event additions follow the protocol plan independently.

## Training is a separate mutation

Do not turn SkillsWindow selection into training. Training needs a verified
trainer/session identity, range and class restrictions, server-provided skill
availability and costs, caps, remaining practices, request encoding and result
handling. The profile's training-point count is only a snapshot; no recurring
authoritative count update has been established. Do not infer extra points
from level changes or show the XML sample count/coin values. A future training
window must handle server rejection and refresh its authoritative state.

## Acceptance before runtime completion

- CPU state fixtures: partial/zero profile skills, catalog exceptions and
  missing strings; base versus language routing; skill update after reset;
  profile absolute XP versus unknown ratio; ratio 0/165/330 and invalid values;
  LevelUpdate followed by correcting ExpUpdate and highest-ever oldlevel.
- Command fixtures: save/load executes nothing; one click dispatches once;
  updated target/gem context is used; unsupported commands, control characters,
  excessive UTF-8 and recursion are rejected; stale slot revisions cancel;
  unavailable sessions retain existing guards; quit return reaches its owner.
- Preference/input fixtures: old layout loads empty bar; round-trip and identity
  separation; invalid slots do not erase layout; drag/save preserves commands;
  editor IME/Enter/Escape/held-release ownership; existing chat and gem shortcuts.
- Original-asset CPU layout checks: real names/templates, nonempty clickable
  slots, hidden samples/rank/training/page controls, no duplicate hotbar children,
  scroll clipping and stable row/slot IDs under overlap and window movement.
- After implementation, capture and inspect original-art UI at normal and
  increased scale: Skills populated/empty/scrolled, XP unknown/zero/half/full,
  empty/populated/pressed/disabled bar, editor valid/error, long labels, clipping
  and overlapping windows. Synthetic fixtures suffice before a live session.
- Live acceptance then verifies a real profile, a received skill/XP change,
  reconnect freshness, and an explicitly activated existing command. Trainer
  packets and unimplemented skill actions remain outside this milestone.
