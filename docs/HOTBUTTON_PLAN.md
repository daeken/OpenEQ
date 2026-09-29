# Small command hotbar implementation plan

Investigation and implementation, 2026-09-29. This refines the command-bar
part of [PROGRESSION_UI_PLAN.md](PROGRESSION_UI_PLAN.md). The first slice is now
implemented: saved twelve-slot bar, original-art editor, ordered native input,
and dispatch through existing actions. No protocol extensions or server changes.
The verification record is in `OVERNIGHT_2026-09-29.md`.

Implement one original-skin bar with twelve locally saved command buttons and
a two-field editor. A button stores a label and one supported slash command.
It resolves its target, gem, inspected item and service at activation through
the existing dispatcher. There is no script interpreter, timer, repeat mode,
inventory binding, keyboard shortcut or server hotbutton packet in this slice.

## What the current code already provides

| Source | Reuse / constraint |
| --- | --- |
| `chat.rs::parse`, `Action` | Supported commands and aliases; plain text becomes Say, so hotbutton validation must require a slash first. Unknown commands error. |
| `Interaction::submit` / `action` | Existing gameplay guards and target resolution. The return value is **true only for Quit**, including `/camp`. |
| `Interaction::ui_action` | Returns `()`. Sending a hotbutton through this path and discarding the command result would silently break Quit. Prefer a dedicated activation method returning the existing bool. |
| `LiveWorld::command` | Enforces ready/error, movement/recovery, trade/item-use and command-specific guards. Do not construct/send packets from the hotbar. |
| `main.rs::handle_gameplay_input` | Uses ordered native events for chat, but ordinary UI actions use whole-frame `mouse.just_pressed`. Hotbuttons need their own ordered press/release path, excluded from that later generic path. |
| `input.rs::ChatInput` | Physical-key ownership, session repeat suppression and same-frame release/fresh-press replay. It also opens chat on slash/Enter, handles history and submits on Enter; those behaviors cannot be used unchanged for a two-field editor. |
| `account_ui.rs::AccountInput` | Concrete pattern for field focus, composition ownership/cancellation, matching pointer press/release, stale frame checks, and captured-key handoff. Account data and types need not move into the hotbar. |
| `ui_layout.rs::LayoutStore` | Version 1 JSON; world/character identity check, 64 KiB read limit, 750 ms debounce, retry delay, private atomic writes. Extend this store instead of adding another preference file. |
| `main.rs::persist_ui_layout` | Captures the entire layout every frame; flushes on mouse release or app exit. Every capture must include hotbuttons or moving a window will erase them. |
| `Interaction` / `LiveWorld::zone_generation` | Interaction survives zone changes. Clearing `GameplayState` does not invalidate an editor or mouse press stored in Interaction. |

The dispatcher permits `/quit` and `/camp` now; `/camp` is an immediate Quit
alias, **not a timed campsite action**. Main handles a true result by writing
`AppExit::Success`; reuse that path so ordinary shutdown/logout and layout
flush remain responsible for cleanup.

## Saved configuration and validation

Keep configuration separate from drafts and interaction state:

```rust
pub const HOTBUTTON_COUNT: usize = 12;
pub const MAX_HOTBUTTON_LABEL_BYTES: usize = 64;
pub const MAX_HOTBUTTON_LABEL_CHARS: usize = 32;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SavedHotbutton {
    pub label: String,
    pub command: String,
}

pub type HotbuttonBindings = [Option<SavedHotbutton>; HOTBUTTON_COUNT];

// Both empty means clear. One empty is a validation error.
pub fn validate_hotbutton(
    label: &str,
    command: &str,
) -> Result<Option<SavedHotbutton>, HotbuttonError>;

// Also called immediately before activation. No network or state mutation.
pub fn parse_hotbutton_command(command: &str) -> Result<chat::Action, HotbuttonError>;
```

Validation order is significant:

1. Reject control characters on the original strings **before trimming**.
   Newlines, CR, NUL and tabs must not become a silently different command.
2. Trim surrounding ordinary whitespace. Both empty clears; exactly one empty
   is an error. Enforce label byte and scalar limits and command length
   `chat::MAX_CHAT_BYTES` (512 bytes). Preserve case and internal spacing.
3. Require exactly the ordinary slash-command form accepted by `chat::parse`;
   specifically reject non-slash text before parsing. Do not split on commas,
   semicolons or a second slash. Such characters inside chat text are text.
4. Parse through `chat::parse`, then match the permitted Action variants
   exhaustively. A future Action variant must require a conscious decision;
   do not add a wildcard that automatically permits new mutation kinds.

Do not store the parsed Action, a target entity, zone, item identity or spell
ID. Revalidate the saved command when loading, when saving and when activating.
Do not execute it during validation, preview, restoration or Save.

### Exact initial command vocabulary

These are the current aliases in `chat.rs`, grouped by resulting Action. The
existing parser remains authoritative for argument interpretation and errors.
No-argument commands currently ignore extra text in that parser; this plan
does not reinterpret extra text as a second action.

| Purpose | Accepted spellings and required arguments |
| --- | --- |
| Say | `/say MESSAGE`, `/s MESSAGE` |
| Group | `/group MESSAGE`, `/g MESSAGE`, `/gsay MESSAGE` |
| Guild | `/guild MESSAGE`, `/gu MESSAGE` |
| Raid chat | `/rsay MESSAGE`, `/rs MESSAGE` |
| Other channels | `/ooc MESSAGE`; `/shout MESSAGE`, `/sh MESSAGE`; `/auction MESSAGE`, `/auc MESSAGE` |
| Direct messages | `/tell NAME MESSAGE`, `/t NAME MESSAGE`; `/reply MESSAGE`, `/r MESSAGE` |
| Emote | `/emote MESSAGE`, `/em MESSAGE`, `/me MESSAGE` |
| Combat / posture | `/attack [on\|off]`, `/sit`, `/stand`, `/hail`, `/loot`, `/con`, `/consider` |
| Target selection | `/assist [NAME]`, `/target NAME`, `/tar NAME`; target `clear` retains its existing meaning |
| Items / services | `/inventory`, `/inv`, `/trade`, `/canceltrade`, `/scribe`, `/useitem`, `/use`, `/merchant`, `/bank` |
| Group membership | `/invite [NAME]`, `/accept`, `/acceptinvite`, `/decline`, `/declineinvite`, `/leavegroup`, `/disband`, `/makeleader NAME` |
| Raid membership / windows | `/raid`, `/raidinvite [NAME]`, `/raidaccept`, `/raiddecline`, `/raidleave`, `/raidleader NAME`, `/guildwindow`, `/skills` |
| Spells | `/book`, `/spellbook`, `/cast 1..12`, `/stopcast` |
| Local information / exit | `/loc`, `/help`, `/quit`, `/camp` |

This is every current Action variant. Explicitly exclude the proposed
`/hotbuttons` and `/hotbutton 1..12` management actions after they are added;
they must not recursively open or replace an editor from a saved binding.
`/audio` is currently handled separately by main before `Interaction::submit`,
so it is excluded initially. `/pause`, macros, slash-delimited scripts, arbitrary
GM commands, `/map` and camera commands are not supported by this dispatcher.

Clarify in the editor help that `/useitem` and `/scribe` operate on the currently
inspected owned item. `/cast 1` uses the current first gem. `/reply` uses the
current last-tell sender. A saved `/sit` sits and `/stand` stands; an argumentless
`/attack` uses the existing toggle behavior. Do not freeze those contexts in
configuration.

### Integrating version 1 layouts without collateral loss

Add `#[serde(default)] pub hotbuttons: HotbuttonBindings` to `Layout`; twelve
nulls is the canonical serialized empty value. Keep Document version 1 because
the old optional field is backward compatible. `Layout::capture` should accept
`&HotbuttonBindings` explicitly, and `Layout::apply` should restore into
`&mut HotbuttonBindings` or call the state replacement method described below.

Update all actual callers: CLI initialization around main.rs:219; interactive
account handoff around main.rs:1694; per-frame/exit capture around main.rs:295;
`Layout::default`; and existing store tests. Never use an implicit default in a
capture call that runs after the user has configured buttons.

A derived `[Option<SavedHotbutton>; 12]` Deserialize alone is insufficient:
one wrong type or wrong array length would fail the **entire** layout. A small
bounded JSON normalization step in `LayoutStore::open_at` is practical:

- Preserve the existing whole-file limit and document version/identity checks.
- Read into `serde_json::Value`, normalize only `layout.hotbuttons`, then
  deserialize the regular Document and apply ordinary layout validation.
- Missing hotbuttons: twelve empty slots, without warning.
- Array: process indices 0–11 independently through SavedHotbutton decoding
  and the shared validator. `null` is empty. Preserve a valid slot at its exact
  index; never compact valid entries around a bad one.
- Pad missing indices; ignore excess indices with a diagnostic. The next
  legitimate save always writes exactly twelve slots. A non-array field resets
  only the bar, with a diagnostic.
- A wrong entry type, missing field, malformed command or over-limit string
  clears only that slot. Return or log a bounded diagnostic containing slot
  numbers/counts, not the saved command text. Report once on load, not per frame.
- Malformed JSON, unsupported document version, identity mismatch, or invalid
  pre-existing layout fields still fail opening and preserve the original file
  as today. Slot recovery must never relax identity validation.

An alternative small custom field deserializer is acceptable if it achieves
the same independent-slot recovery and can report the discarded slots. Avoid
a generic configuration migration framework for this field.

Keep invalid-configuration diagnostics outside persisted state/equality so
they do not trigger writes every frame. Loading must not immediately rewrite
the file merely to normalize it. Explicit Save can request `LayoutStore` flush;
otherwise mouse release and the existing debounce already handle persistence.
If writing fails, retain the in-memory binding and the old file, show the save
failure, and use the existing bounded retry; do not report disk persistence
as successful.

## State and activation API

Keep the pure saved bindings in a small hotbutton state owned by Interaction.
Use a monotonically changing interaction revision plus the existing zone
generation; do not reset the revision to zero when adopting a new character.
Suggested shapes, adjustable to the UI contract:

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HotbuttonToken {
    pub revision: u64,
    pub zone_generation: u64,
}

pub struct HotbuttonState {
    bindings: HotbuttonBindings,
    pub open: bool,
    revision: u64,
    zone_generation: Option<u64>,
    // Draft and press are transient; neither is serialized.
    editor: Option<HotbuttonDraft>,
    pressed: Option<PressedHotbutton>,
}

impl HotbuttonState {
    pub fn bindings(&self) -> &HotbuttonBindings;
    pub fn begin_session(&mut self, bindings: HotbuttonBindings);
    pub fn sync_context(&mut self, zone_generation: u64, available: bool);
    pub fn open_editor(&mut self, token: HotbuttonToken, slot: u8);
    pub fn save_editor(&mut self) -> Result<bool, HotbuttonError>; // changed
    pub fn cancel_editor(&mut self);
    pub fn command_for_activation(
        &self, token: HotbuttonToken, slot: u8,
    ) -> Option<&str>;
}

// Called by the ordered pointer path, once after a validated release.
// Returns the existing true-for-quit result for main to propagate.
impl Interaction {
    pub fn activate_hotbutton(
        &mut self, token: HotbuttonToken, slot: u8,
        live: &mut LiveWorld, position: [f32; 3],
    ) -> bool;
}
```

`activate_hotbutton` checks the current context and token, copies the bounded
command from the unchanged slot, parses/validates again, and calls
`Interaction::action` with the current LiveWorld and position. It returns
the result to main. There is no need to change every existing `ui_action`
caller just to support Quit: handle hotbar activation in its own raw-input
branch and write `AppExit::Success` there when true.

Bump revision and clear pending press when a binding changes, a new editor
opens, the editor closes, the bar closes/reopens, a zone boundary occurs or a
new live session is adopted. The editor carries the slot and revision it was
opened against; Save must recheck both. Field focus/text changes need not bump
the binding revision, but must invalidate a pending editor button press if its
meaning changes. Keep a separate draft epoch if replacing a draft in-place is
simpler than using the same revision consistently.

The input state records `{token, slot, mouse_button}` on Pressed. Released
consumes that record with `take()` and acts only if the same current topmost
slot is under the pointer, the button matches, and the current token still
matches. Left release on a populated slot activates; left release on empty
or right release on any slot opens the editor. Moving away, focus loss,
cursor leaving the window, a stale frame, editor activation, another session
or binding replacement cancels. A release without a captured press is inert.
Do not duplicate this handling in the later `mouse.just_pressed` UI dispatch.

The logical hit IDs should include revision and zone generation, e.g.
`hotbutton:<revision>:<zone_generation>:slot:<0..11>`, with window `hotbuttons`.
Editor controls use window `hotbutton_editor` and the current draft token.
The XML's duplicate ScreenIDs are never action identity.

## Input owner and lifetime details

Use one dedicated hotbutton input controller with two bounded UTF-8 editors,
focus `Label | Command | Clear | Save | Cancel`, ordered modifiers, captured
physical keys, IME composition owner, pointer and pending press. Reuse
ChatEditor editing primitives if useful, but do not call `submit()` or its
history for draft Save. ChatEditor::open is not itself bounded, so restore only
validated definitions and bound any inserted/preedit text for its field.

The editor is modal for gameplay input while open. It may consume clicks
outside itself without committing/clicking through. Cancel is explicit via
Escape/close; Clear empties the **draft**, and Save with both fields empty
clears the slot. Draft changes are never autosaved. Enter saves only once;
holding Enter cannot resave, reopen chat or activate a newly shown button.
Tab/Shift+Tab moves editor focus, not world targeting. Existing Alt+digits gem
keys and movement controls remain owned by gameplay only when no editor owns
the events.

Main must process the combined `WindowEvent` stream in this order:

1. Existing account handoff filter.
2. Retired editor key/composition ownership (including chat-to-hotbutton and
   hotbutton-to-chat), so an old key repeat cannot enter the new editor.
3. Current hotbutton editor/slot pointer routing. Apply a returned intent
   immediately, before the next native event in the batch.
4. Existing chat router for events that remain unowned.
5. Gameplay key/mouse handling after suppressing every still-owned key.

ChatInput currently has no captured-only filter: its normal event function can
open chat on slash/Enter. Add a narrow `filter_handoff_event` hook modeled on
AccountInput for when a different editor is active. It should retire chat's
old session, consume held/repeated/released old keys, suppress their Bevy
held/edge state, and reconstruct a genuinely fresh press after release within
the same batch. It must not open chat, edit text or submit a command. Avoid
feeding the same owned event to both routers. The hotbutton input needs the
same inactive-owner filtering after its own editor closes.

Before the raw loop, initialize each owner's handoff reconstruction from the
whole-frame Bevy state. After routing, suppress owned keys before targeting,
cursor capture and camera movement. `controls_blocked` stays true for the
entire frame if an editor owned any part of it, including a frame where Save
or Escape closed it. Mark consumed Escape so the later uncaptured-cursor
handler does not also quit. Synchronize `window.ime_enabled` only after the
final input owner is chosen, for either active chat or the hotbutton field.

Track composition's field and draft/session identity. While composing, Enter
must not Save and Tab must not move to the other field. A field click/close,
focus loss or session change cancels composition. Drop the corresponding late
commit even if a different field or chat editor has opened; a fresh preedit
starts a new composition. AccountInput provides a working cancellation pattern.
Preedit display is bounded too, not just committed strings.

Lifecycle rules:

- **Zone transition, including same-zone forced re-entry:** preserve bindings
  and bar position/open preference in memory; cancel draft/press, bump revision
  and adopt the new zone generation. Do not queue an activation for Ready.
- **Disconnected/loading/recovery overlay:** cancel draft/press and disable
  activation. Keep bindings. Route old key releases/focus events through the
  handoff filter even while the main gameplay branch returns early. Repeated
  pre-transition Enter/W/Alt must not reappear as a new gameplay press later.
- **New character/session:** call `begin_session` on the existing state to
  invalidate tokens and clear prior definitions before attempting the new
  identity's layout restore. A missing/bad new layout must yield an empty bar,
  never the previous character's commands. Flush the old identity's store
  before replacing it if an in-process switch is added; do not write the new
  character's definitions through the old store.
- Current main has one Interaction for its lifetime and assigns the account
  result directly to `runtime.live`; do not rely on a nonexistent implicit
  Interaction reset there. Both CLI and interactive restore paths need explicit
  binding adoption.
- Do not serialize open state, draft text, selection, input ownership, press,
  revision, zone generation or server-derived context. Current UI layout stores
  only positions/order and the new confirmed bindings.

## Tests and bounded verification

Use pure tests and the existing `live::tests::command_world`; no server
mutation is needed to verify the first hotbar slice.

1. **Validation table:** every allowed Action/alias parses; `/cast 0/13`,
   malformed tell/raidleader and unknown commands fail; non-slash text fails;
   management/audio/script commands fail; label scalar and byte boundaries,
   multibyte command boundary, controls before trim, both-empty clear and
   half-empty error. Validate `/quit` and `/camp` as Quit explicitly.
2. **Persistence:** old version 1 JSON without hotbuttons loads empty; all
   twelve slots round-trip; valid neighbors survive malformed type/command;
   short/long array normalization retains indices; wrong shape affects only
   bar; identity mismatch and oversized/future-version files remain rejected
   and untouched. A window move/capture retains bindings. Existing debounce,
   failed-write and shutdown flush tests cover the added field.
3. **Pointer/reducer:** press/release executes once; repeat release is inert;
   release on another slot/outside/over an occluder is inert; right click
   opens editor only; empty left click edits; stale revision, changed binding,
   close/reopen, zone generation, new session and disconnect cancel; Clear is
   draft-only until Save; invalid Save preserves old binding and draft.
4. **Actual dispatcher:** a saved `/attack on` without a target sends nothing;
   `/cast 1` uses the current gem; `/reply` without sender errors; `/useitem`
   retains the inspected-owned-item checks; `/skills` only changes local state.
   Quit and Camp return true without network traffic, and the main-facing
   activation path retains that result. Do not merely unit-test parse and
   assume the UI propagates exit.
5. **Ordered input:** same-frame pointer focus then text; Save/Cancel then held
   text/Enter/Alt repeats; release then fresh press in the same batch; chat
   `/hotbutton 1` Enter ownership across editor opening; hotbutton Save then
   chat opening; Tab and Alt+digit suppression; losing/regaining focus in one
   batch; foreign-window events ignored; composing Enter/Tab; field change
   plus late IME commit; zone/disconnect with held keys; no click-through on
   editor close. Reuse the real WindowEvent test constructors and sequencing
   patterns in input.rs/account_ui.rs.
6. **UI:** original HotButton template renders all twelve slots, hover/pressed
   state and bounded labels/tooltips; editor fields/buttons clip at narrow
   viewports; no old page/Make Hotkey/color controls are actionable. One local
   render/screenshot after integration is sufficient for initial visual proof.

Suggested ownership: root owns the hotbutton reducer/Interaction/main lifecycle
and dispatcher integration; UI agent owns painter/contracts and the narrow
HotButton skin support; a bounded input/config task may own the isolated input
controller and layout extension after the contracts are agreed. Keep the
existing account/chat behaviors covered while adding the new owner.
