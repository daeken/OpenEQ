# XML widget slice: readable spell inspection

Audit and implementation: 2026-09-29. The bounded slice below is implemented;
the original investigation and constraints are retained after this status.

## Implemented behavior

Right-click a learned spellbook entry to open the original `SpellDisplayWindow`.
It uses `SDW_IconButton`, `SDW_SpellDescription`, the original inner frame and
vertical scrollbar textures. The read-only description begins at the top and
scrolls with the wheel or arrow buttons. The scrollbar thumb is positioned
using the same measured rows that the renderer draws. Thumb dragging, rich
links/colors and window resizing are outside this slice.

The window is an ordinary `spell_inspection` layer: close/title/body hits keep
their owner, overlap order matches hit order, and position/order use existing
layout persistence. Selection and scrolling are transient. Reopening selects
the requested spell and resets its scroll. Spellbook left-click, gem
right-click memorization and buff right-click removal retain their behavior.
Inspection sends no gameplay command.

`spells.rs` now separates `landing_message`, `description_id` and `description`.
The optional Windows-1252 dbstr catalog supplies real type-6 descriptions using
the compact/fixed ID fields. Malformed, empty and oversized descriptions are
skipped. Missing descriptions have an explicit fallback; dynamic effect values
become `?` with one explanatory sentence. Hover details remain concise metadata.

`WidgetState::scroll_rows` opts a read-only STMLbox into `DrawCommand::TextArea`.
`scroll_id` identifies its content; `Renderer::ui_text_scroll_metrics` reports
total/visible/first rows from the current font, width and display scale.
`SpellInspection::update_metrics` rejects another spell's stale measurements.
The renderer positions the thumb in the same preparation pass, so it does not
wait a frame for layout feedback. Missing optional thumb/arrow art retains
functional scrolling with simple fallback visuals.

Main integration hooks are deliberately small: after UI preparation, pass the
renderer metrics to `interaction.spell_inspection.update_metrics`; route wheel
events for the displayed topmost `spell_inspection` hit through its `wheel`
method after recovery ownership; let `close()` participate in ordinary Escape
handling. Root integrated these `main.rs` hooks; their ordering was reviewed
after the all-target check passed. The wheel helper consumes chrome
events too, ignores nonfinite input and bounds its accumulated scroll offset.

## Validation

- Eight spell catalog/description tests pass, including the installed client.
- Two inspection state/interaction tests pass; recorded command queues prove
  inspection emits no packets and preserve memorization/removal gestures.
- Ten portable XML layout tests pass, including scroll hits, clipping, disabled
  controls and missing optional scrollbar art.
- The measured-text GPU test passes at 1× and 2×: top/bottom clamping, UTF-8,
  blank lines, long words, width changes, clipping and thumb placement.
- The original-skin GPU inspection test passes with no layout warnings. Six
  captures under `/tmp/openeq-spell-inspection` were visually inspected: short
  Complete Heal text and the top/bottom of the 960-character Cryomancy text at
  1× and 2×. Both final lines are visible, the close control remains accessible,
  and overlap ownership/closed-frame removal are asserted.

No live-world fixture is needed for this read-only slice. The final missing-spell
fallback test also passed after account integration: absent metadata displays
“Description unavailable” without inventing zero-valued spell attributes.
Root performs combined workspace and native main-loop integration checks.

## Original problem

The highest-impact remaining text-widget gap is a persistent, scrollable spell
description window. Today hovering a spell shows its short landing message
(for example, “You feel a little better.” for Minor Healing), plus range and
recast time. The original client contains both a dedicated XML window and the
actual explanatory text. Make that information readable before attempting a
general reconstruction of every XML list, tab and rich-text feature.

## Evidence from the current implementation

`crates/openeq/src/spells.rs::parse_spell` assigns field 6 to `description`.
That field is a landing message, not the spell description. `Spell::view`
appends range/recast to it. `gameplay_ui.rs::spell_tooltip` places that string
in a 286 × 60 logical-pixel area of a transient tooltip; there is no persistent
spell inspection or scrolling.

The installed `spells_us.txt` has 47,134 records, and no field-6 message exceeds
63 characters. The immediate problem is therefore incorrect content, rather
than frequent overflow of those existing messages. Loading the real text makes
scrolling necessary: 5,920 matched spell records have descriptions longer than
150 characters, 113 exceed 400, and the longest is 960 characters.

`crates/openeq-ui/src/layout.rs` recognizes `STMLbox` but emits ordinary text.
It has no scroll state or content metrics, ignores the scroll style flags, and
does not create an interactive hit for an STMLbox. The renderer already has
UTF-8-aware measured wrapping, clipped drawing and scrolling in its chat
`TextLog` path. Reuse those measurements for inspection; do not estimate line
lengths from character counts or copy chat's bottom anchoring.

## Installed skin and data contract

Evidence was read from `/Users/daeken/EverQuest`, especially
`uifiles/default/SIDL.xml`, `EQUI_SpellDisplay.xml`, `EQUI_ItemDisplay.xml` and
`EQUI_ChatWindow.xml`.

| Source | Verified behavior/data |
| --- | --- |
| `SIDL.xml`, `Control` | Defines `Style_VScroll`, `Style_HScroll`, `Style_AutoVScroll`, and `Style_AutoHScroll`; all default false. |
| `SIDL.xml`, `ScrollbarDrawTemplate` | Defines up/down buttons, thumb frame, middle texture and tint; window templates reference vertical/horizontal templates. |
| `EQUI_SpellDisplay.xml` | `SpellDisplayWindow` is 400 × 190, with title/close/border/sizing styles and `WDT_Filigree`. |
| `SDW_SpellDescription` | An anchored `STMLbox` using `WDT_Inner`, `Style_VScroll=true` and a border. |
| `SDW_IconButton` | Original spell icon control in the same window. |
| `EQUI_ItemDisplay.xml` | `IDW_ItemDescription` enables vertical and automatic vertical scrolling; `IDW_ItemLore` enables vertical scrolling. |
| `EQUI_ChatWindow.xml` | `CW_ChatOutput` requests vertical scrolling, but current gameplay already supplies a functional custom text log. |
| Installed compact `spells_us.txt` | Description ID is field **95**, with zero-based indexing. |
| Fixed EQEmu spell layout | Description ID is field **155**, confirmed by `common/spdat.h` and `common/shareddb.cpp`. |
| Installed `dbstr_us.txt` | Lookup key is **(description ID, type 6)**, not spell ID. |

All 56,248 installed dbstr rows have five caret-separated fields shaped
`id^type^text^0^`. Decode this file as Windows-1252, as with the spell catalog.
Validate the record shape and numeric key; preserve text whitespace and
ordinary percent signs. A format from another client with additional fields
must not silently shift the text column. A missing or malformed optional dbstr
file must leave the client and spell catalog usable.

The description-ID lookup matches 38,310 spell records, referencing 30,097
distinct descriptions. Spell 200 uses description 200, but that coincidence is
not a general mapping: spell 41711, Cryomancy XXIV, uses description 13679.
The only markup observed in those matched texts was case-insensitive `<br>`
(339 occurrences, counting descriptions referenced by multiple spell records).
These counts describe the installed assets, not every EverQuest client.

## Keep the first implementation narrow

1. Add `description_id` and a separate landing-message field to `Spell`.
   Extend the existing compact/fixed layout distinction rather than inventing
   another format detector. Load the optional dbstr description catalog once,
   keyed by both ID and type. Keep short hover details concise and provide a
   distinct full inspection presentation.
2. Add a conservative plain-text description formatter. Convert `<br>`,
   `<BR>` and their conventional self-closing forms to newlines. Do not treat
   client text as executable markup, navigation, image paths or UI actions.
   Unknown markup should remain harmless visible text in this first slice.
3. Open one `SpellDisplayWindow` on right-click of a learned spellbook entry.
   Bind the original icon and description controls, and include the name,
   class level, mana, cast time, range and recast fields already available.
   Use the same details with a “Description unavailable” fallback when the
   description lookup fails. Opening another spell replaces this presentation
   and resets its scroll position; it does not open an unbounded set of windows.
4. Implement a top-anchored, read-only text viewport for this STMLbox, with
   wheel scrolling and the original vertical scrollbar parts when present.
   Give scrollable STMLboxes explicit clipped hits. Store scroll position in
   widget/presentation state, and reuse the renderer's measured row ranges.
   Return content/visible-row metrics with the rendered widget identity so
   the scrollbar thumb, hit locations and clamping share the actual wrapping.
   Bound the accepted scroll offset and clamp it after a width/font/scale
   change. A missing optional scrollbar texture must not disable scrolling.
5. Register the window as ordinary logical window `spell_inspection`, with
   the current title dragging, body capture, close handling and window stack.
   Save its position/order through the existing layout system; do not persist
   selected spell, open state or scroll offset. A fixed initial size is enough;
   the XML's sizing flag does not require general window resizing in this slice.

Suggested ownership: `spells.rs` owns ID lookup/description presentation;
`openeq-ui::layout` owns read-only scrollable text bindings/commands;
`openeq-render::ui` owns measured wrapping, clipped text and returned metrics;
`gameplay_ui.rs` owns the original window; `interaction.rs` owns selection and
scroll state. `main.rs` only routes the already-owned pointer/wheel event.
Do not change packet serialization, spell calculations or server state.

## Dynamic descriptions: an explicit boundary

20,417 of the 38,310 matched spell records contain candidate substitutions.
Observed forms include `#1`–`#12`, `@1`–`@12`, `$1` and other numbered `$`
tokens, `%z`, `%H`, `%L`, `%l`, `%1`, and `#x`. These are not safely resolved
by replacing every number with an effect's base value. Results can depend on
level, formula, limits and server behavior.

For this first slice, replace recognized substitution tokens with `?` and add
one unobtrusive sentence: “Some effect values are unavailable.” For example,
Minor Healing becomes “Mends minor wounds, healing between ? and ? hit
points.” Preserve literal percentages, so a token followed by `%` becomes
`?%`. Tokenization must consume a complete multi-digit token: `#12` must not
be partially consumed as `#1`. Any unrecognized token-like sequence must be
reported as unavailable too, rather than presented as a computed value.
The substitution scan should be bounded and preserve UTF-8 boundaries.

Descriptions without substitutions can be shown intact immediately. This
delivers the original explanatory text and a useful long-text widget without
claiming correct effect arithmetic. Verified level-aware substitution can be
a separate spell-presentation change; authoritative costs, timing and effects
continue to come from the server. Do not silently replace missing descriptions
with the landing message and label that a complete description.

## Input and ownership rules

Current right-click behavior is intentional: a gem opens memorization and
selects that gem; a buff requests removal. Preserve both. Spellbook entries
currently ignore right-click because `Interaction::ui_action` rejects
`MemorizeSpell` in its right-click gate. Handle inspection before memorization
dispatch so it produces **no** posture or memorize command. Left-click keeps
its existing behavior. Other inspection gestures can follow later.

The displayed frame's topmost hit determines ownership. Inspection wheel
events must be consumed before map zoom, chat scroll or world input; a
covered inspection must not scroll. Read-only inspection may remain usable
during recovery if consistent with existing read-only windows, but recovery
hits stay above it and suppress underlying hover/input. Do not exempt casting,
memorization or other gameplay actions from the existing death/recovery gate.
Clicking the description, scrollbar or blank window body raises the complete
window without clicking through. Hovering or scrolling alone does not raise it.

## Acceptance tests

- Portable catalog fixtures: fixed field 155 and compact field 95 produce
  the same description ID; one description shared by multiple spell IDs;
  same ID under another dbstr type is not selected; missing/invalid/truncated
  rows retain a usable metadata fallback; Windows-1252 punctuation survives.
- Portable formatting fixtures: case variants of line breaks, empty text,
  unknown tags, ordinary percentages, adjacent/multi-digit substitutions,
  the uncommon forms listed above, and multibyte text. Assert unavailable
  values never become invented numeric effects or executable markup.
- Interaction fixtures: right-click spellbook opens inspection without a
  command; left-click still memorizes; gem right-click and buff right-click
  retain their existing semantics; changing the inspected ID resets scroll;
  closing removes pixels/hits; covered and recovery-owned wheel input cannot
  reach inspection or the world.
- Renderer fixtures: top/bottom scroll clamping, a long paragraph, explicit
  blank lines, UTF-8 wrap boundaries, a long unbroken word, clipping, width
  changes and a huge requested offset. Thumb position and returned metrics
  must agree with the actual drawn rows at both 1× and 2× scale.
- Original-skin GPU capture: use `SpellDisplayWindow` and its named children,
  render one short literal description and the 960-character Cryomancy text,
  capture top and bottom at 1× and 2×, and inspect all captures. Require no
  missing-template warnings, no text outside the inner viewport, a visible
  last line at the bottom, correct overlap ownership and accessible close.

The first slice does not need a live-world fixture: inspection is read-only,
and packet non-emission belongs in the interaction tests. Verify the normal
workspace checks after integration. Full STML colors/links, selection/copy,
resizable windows, generic list/tab behavior and the large modern item-display
layout remain subsequent work. The present item tooltip's 16-row cap is not
the strongest immediate issue: current `game.rs::item_view` supplies at most
ten rows, although long unwrapped item text still deserves a later pass.
