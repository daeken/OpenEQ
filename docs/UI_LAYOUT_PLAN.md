# Movable window stacking and persistence

Design audit: 2026-09-29. Implementation is held until the current death/recovery
milestone is published. This document describes the next bounded UI slice.

## Outcome and ownership

Clicking a visible window brings the entire window forward while preserving the
clicked control's action. Its pixels and hit targets use the same order. The
order survives restart for the same world/character, alongside the positions
already saved by `ui_layout.rs`. Opening or raising windows never restores a
merchant, trade, loot, or other server-owned session.

- UI agent: logical window layers, composition and hit ownership in
  `gameplay_ui.rs`/Painter, optional metadata in `openeq-ui::HitTarget`, focused
  tests and original-skin overlap captures. Coordinate any helper changes in
  commerce/social/trade drawing rather than inferring ownership from actions.
- Root: `Interaction`/`GameHudState` order plumbing as agreed, persisted order in
  `ui_layout.rs`, main input hooks, and map insertion into the shared order.
- Recovery dialogs remain above ordinary windows. Their actions and lifecycle
  do not change in this slice.

## Current behavior and source pitfalls

`Hud::gameplay_frame` in `gameplay_ui.rs` emits a fixed order: player, target,
buffs, spellbar, group, chat, actions, inventory, bags, loot, spellbook,
merchant/bank, trade and casting. `UiFrame::hit_test` scans enabled hits in
reverse, so current control precedence follows emission order, not click order.
Dragging only updates a position. An inventory window cannot be raised above a
later loot window, for example.

`Painter::shell(template, key, ...)` already receives stable logical keys for
nearly every window. Player and target use `widget` plus a manual title hit and
need explicit layer boundaries. `commerce` contains two separately keyed
windows and must preserve that distinction. Bags reuse one XML template but
have distinct `bag:<parent_slot>` keys. Gameplay action IDs such as
`game:slot:<slot>` are server addresses, not window identifiers.

The XML `ScreenID` identifies an individual control, often falling back to the
widget item name. Preserve it. Neither it nor an XML template name can identify
an owning logical window: `ChatWindow` is reused by several windows and all bags
share `ContainerWindow`. Likewise, do not infer ownership from action prefixes;
inventory slot actions occur in inventory, bags and bank windows.

XML Screen roots currently supply body hit coverage when a template loads, but
that is incidental. Add an explicit full-body capture hit for every logical
window before its interactive controls. This also covers missing optional skin
templates and disabled controls without allowing clicks through to lower UI or
world targeting. Clip that hit to the window's visible rectangle and viewport.

The map is currently appended in `main.rs` after all gameplay drawing, including
tooltips and cursor items. It must become an ordinary named layer before those
overlays are assembled. Sorting only inside the HUD while leaving map appended
would retain an unraiseable map and hide cursor items beneath it.

Hover item/spell details are selected using `draw.frame.hit_test` before they
are drawn. Sorting after that lookup is incorrect: it can show details for a
covered control. Sort ordinary windows, including the map, first; then derive
hover presentation from that ordered hit list.

Persistent item inspection currently has an interactive close/use panel but no
saved position or drag title. Keep its present overlay behavior in this slice
and give its hits no ordinary-window owner. Making it movable can be a separate
change. Hover tips and cursor icons have no interactive hits. Nameplates and
combat feedback currently prepend their commands to HUD drawing; retain their
placement behind ordinary windows. Recovery remains the last interactive layer.

## Bounded implementation

Use the existing position keys as logical IDs: `player`, `target`, `buffs`,
`spellbar`, `group`, `chat`, `actions`, `inventory`, `bag:<parent_slot>`, `loot`,
`spellbook`, `merchant`, `bank`, `trade`, `casting`, and `map`.

Introduce a small per-window layer record holding its logical ID and `UiFrame`.
Painter can delimit groups at `shell` calls, with explicit boundaries for
player/target and before non-window overlays. Each group must own both its draw
commands and hit targets; individual commands must never be sorted separately.
An alternative internal representation using completed command/hit ranges is
fine if both vectors are moved together without cloning texture data each frame.
Do not add nested window semantics or a general widget tree to this slice.

Add optional `window_id: Option<String>` to `HitTarget`. XML layout and unrelated
standalone frames initialize it to `None`. When completing a logical layer,
stamp **every** hit in it with the layer ID, including native XML controls,
explicit buttons, title/close hits, and the body capture. This preserves widget
and protocol IDs. Disabled controls fall back to a body hit with the same owner.

Add `window_order: Vec<String>` to presentation state, ordered back to front.
Composition uses this order to append complete layers. Missing IDs receive a
deterministic fallback order; duplicate/stale order entries cannot duplicate or
drop a layer. Each currently visible logical ID contributes at most one layer.
Closed windows produce no pixels or hits while retaining their saved position
and relative ordering preference.

Keep the existing `Hud::gameplay_frame` API as a compatibility wrapper for
tests, captures and non-map callers. Add a variant accepting additional named
window frames, for example `Vec<(String, UiFrame)>`. Root builds the map frame
and passes it as `map`; the HUD composes all ordinary layers, resolves hover,
then appends inspection/tooltips/cursor presentation. Main can still prepend
nameplates/combat feedback and append the recovery overlay afterward. Rendering
continues to consume an ordinary `UiFrame`; no GPU renderer sorting is needed.

## Main input and persistence contract

Raise the owner of the **original topmost hit** on a focused, uncaptured left or
right press, before dispatching that hit's existing action. Use the stored hit
from the displayed frame; do not raise, re-hit-test, and accidentally activate a
different control during the same click. Clicking blank chrome or a disabled
control raises its window but sends no gameplay action. Recovery/inspection
hits have no ordinary owner, so they cannot raise a covered window. Hovering,
wheel scrolling and dragging across another window do not raise that window.

Chat handles its pointer press early in the raw event loop and may suppress the
later generic UI click. Raise chat at that same ownership decision, or use one
shared press handler which records the owner before either path consumes it.
Rendered chat-link hits are a separate renderer-generated list; retain the
current requirement that the ordinary top hit is `game:chat_log` before a link
can activate. Raise chat from that ordinary hit, not from the link metadata.

A title press raises the window once and establishes the existing drag owner.
Dragging retains that owner until release/focus loss; crossing other windows
does not switch owners. Preserve logical-pixel coordinates at Retina scale.

Root adds `#[serde(default)] window_order: Vec<String>` to version-1 layout
documents. Old position-only files load with the default order. Keep the current
identity scope, atomic writes and debounce/release flushing. Bound the list and
ID lengths, reject control characters and duplicates or canonicalize them
deterministically, and validate bag IDs without treating them as filesystem
paths. A practical list cap is the existing 256-window bound.

At initial layout restoration, fill missing visible IDs in deterministic order
without overriding the saved relative order. Subsequently, newly opened windows
should come forward; distinguish first-frame restoration from later visibility
changes so every startup does not erase the saved stacking. Retain hidden IDs
within the bounded order list, and move an ID to the end on an explicit raise.
Persist order only, never current visibility or session data.

## Acceptance checks

Portable tests should establish observable behavior:

- Two overlapping windows show and hit the same top owner; swapping their order
  swaps both the final drawing and winning control. Raising one is idempotent.
- Clicking a control retains its action and protocol address while raising its
  owner. Two bag windows sharing a template retain distinct IDs.
- Blank body and disabled controls block lower windows/world actions. Native
  XML controls and explicit controls receive the same layer ownership.
- Unknown/duplicate/sparse order entries preserve every visible layer once;
  hidden windows produce no hits and reopening does not lose their positions.
- Map participates in the same order. A covered item does not show its hover
  details through the map or another window; tooltips/cursor stay above it.
- Recovery and persistent inspection capture their own areas without raising
  lower windows. Nameplates remain behind ordinary UI.
- Chat links require visible chat ownership. Title dragging keeps its owner
  across overlaps and focus loss releases it.
- Version-1 files without order still load; order/positions survive restart,
  and world/character isolation plus invalid-file preservation remain intact.

Run relevant UI/persistence tests, strict Clippy, and an original-skin GPU
capture with inventory, two bags, loot and map deliberately overlapping before
and after a raise. Inspect ordinary and Retina captures. The render sequence and
hit order must agree at a point that was visibly covered before the raise.
