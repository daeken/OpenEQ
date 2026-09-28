# EverQuest XML UI foundation

`openeq-ui` reads the client's SIDL XML files and builds a draw list independently
of Bevy or wgpu. It does not execute scripts or client commands from UI files.
Original client assets must be supplied by the user; none are redistributed here.

```rust,no_run
use openeq_ui::{Rect, UiBindings, UiDocument};

let ui = UiDocument::load("/path/to/EverQuest/uifiles/default", "EQUI.xml")?;
let window = ui.window("PlayerWindow")?;
let mut bindings = UiBindings::default();
bindings.widget_mut("Player_HP").text = Some("Adventurer".into());
bindings.eq_gauges.insert("1".into(), 0.73); // HP fraction
bindings.eq_text.insert("19".into(), "73".into()); // HP percent label
bindings.widget_mut("PlayerWindow").rect = Some(Rect::new(16., 16., 160., 95.));
let frame = window.layout(Rect::new(0., 0., 1280., 720.), &bindings);
// Send frame.commands to a renderer, or use frame.hit_test([x, y]) for input.
# Ok::<(), openeq_ui::UiError>(())
```

For the login UI, load `EQLSUI.xml` and instantiate `connect`. Controls retain
both `item` and `ScreenID`; bindings accept either, with `item` taking precedence.
The application must set `WidgetState.password = true` for a password field and
handle edit focus, keyboard input, clicks, and networking itself.

Supported presentation features:

- Composite includes, case-insensitive filenames, UTF-8/BOM and Windows-1252.
- Named definitions, texture atlases, timed animations, grid cells, and explicit frame index.
- Screens and nested pieces, labels, buttons with state art, edit boxes, gauges,
  static images/frames, inventory backgrounds, listbox frames, and basic tab page selection.
- Relative/absolute placement, stretch anchors (with SIDL defaults), min/max
  bounds, clipping, text alignment, frame/title art, and gauge fill clipping.
- Dynamic text, visibility, enabled/pressed/hovered/checked state, gauge fractions,
  rectangle overrides, EQType bindings, hit testing, and missing-art diagnostics.

`DrawCommand::Image` includes an absolute texture path, pixel source rectangle,
and authored atlas dimensions. Renderers should use the decoded image dimensions
when normalizing UVs; a zero source dimension means the whole texture. Command
colors are sRGB RGBA bytes; clips and geometry use the same pixel coordinate space
as the viewport. Commands are in drawing order. `openeq-render::ui::UiRenderer`
provides the wgpu implementation, including true TGA/DDS textures and cached text.

This is a foundation rather than a complete SIDL client. It does not implement
list/tree data models, rich STML markup, drag-and-drop inventory, spell gem logic,
scrollbars, resizing input, tiled backgrounds, exact client font metrics, every
frame overlap rule, layout strategies, or schema-driven custom inheritance.
`StaticTintedBlendAnimation` is not implemented.
Unknown properties remain in `Element` for later support; unsupported widgets
are reported in `UiFrame.warnings`. Live indicator visibility and values are
explicit application state and are not guessed from widget names.

Run the portable regression suite with `cargo test -p openeq-ui`.
Run the real client compatibility tests with:

```
EQ_UI_DIR=/path/to/EverQuest/uifiles/default cargo test -p openeq-ui --test client_assets -- --ignored --nocapture
```

The installed default skin used during development loads 167 files, 6,842
named definitions and 1,219 animations. Both `PlayerWindow` and the `connect`
login window produce draw lists without missing-art warnings; the tests also
validate the character selection window, texture paths, hit tests, and masking.


The application layer's `Hud::gameplay_frame` composes the original window,
button, equipment-slot and bag-slot templates with live state. `GameHudState`
supplies colored chat, editing focus/caret/scroll position, inventory slots, open
bags, loot, cursor items, and window positions. `UiAction::from_hit` maps the
namespaced hit IDs to client actions; the application owns the input and protocol
handling. Inventory addresses are always supplied by the protocol adapter,
including RoF2 bag addresses such as 4010; XML widget names are presentation only.
RoF2 power-source slot 21 and ammo slot 22 match the authored XML order.

`DrawCommand::TextLog` keeps one color per source line, wraps using the renderer's
font metrics, and treats `scroll_rows` as wrapped display rows above the newest
message. Zero follows the newest line; excessive offsets clamp to the oldest
page. The input caret offset is a UTF-8 byte boundary. Original `A_DragItem`
grid cells use EQ icon ID minus 500, preserve each DDS atlas's padding, and
honor the XML `Vertical` column-first traversal flag.

To verify and capture the complete gameplay layout against a local installation:

```
OPENEQ_UI_CAPTURE_DIR=/tmp cargo test -p openeq --lib gameplay_ui::tests -- --include-ignored --nocapture
```

The gameplay overlay also supports the original spell gems, spell icons, book
art, and casting gauge. `UiSpell` uses the zero-based `A_SpellIcons` cell (the
spell table's new-icon field); unlike drag-item icons this grid is horizontal.
Spellbook pages expose explicit spell-ID/gem-ID actions. Known/memorized spells,
selected gem, availability and cast progress remain application state.
`UiBuff` displays server buff slots and remaining time using the original buff
frames. `RemoveBuff(slot)` hit actions must be dispatched only on right-click;
left-click never implicitly cancels an effect. Persistent item inspection blocks
clicks through its panel and exposes a close action; hover tooltips stay passive.

For HiDPI windows, build draw lists and hit-test pointers in logical pixels, then
call `Renderer::set_ui_scaled(frame, window_scale)` or
`UiRenderer::prepare_scaled(device, queue, frame, physical_size, window_scale)`.
Geometry and clipping are scaled together; glyphs are rasterized at the physical
pixel density. Existing `set_ui` and `prepare` continue to mean scale 1.

`DrawCommand::Line` supplies a batched screen-space segment. The client map module
uses it to render the installed `maps/<zone>.txt` and layers `_1` through `_3`,
including labels, height fading, player heading, target and waypoint markers.
Map records convert `(-x,-y,z)` into the engine's EQ world coordinates, with north
(+world_y) at the top. Map click/zoom/drag actions remain application-owned;
`MapState::world_at` converts a canvas click into a waypoint without network I/O.

```
OPENEQ_UI_CAPTURE_DIR=/tmp cargo test -p openeq --lib map::tests -- --include-ignored --nocapture
```

Merchant and bank overlays use the original `MerchantWnd`, `BankWnd`, stock list,
button, coin, and bank-slot artwork. `UiMerchantStock.slot` is the server's stock
row address, not the item's ID. Buy prices are integer copper per unit; the
application supplies availability and action enablement. `sell_price_copper` is
an estimated **total for the selected quantity**, labeled as estimated until the
server confirms the sale. Neither prices nor item transfers are applied by the
presentation layer. Stock supports explicit row selection, quantity controls,
scroll arrows, and a wheel hit region. Pending actions disable buy/sell controls.
Bank slots retain canonical addresses 2000–2023 and shared slots 2500–2501;
coin controls use wire order copper, silver, gold, platinum. Optional balances
and shared-bank capability remain application-owned.

`UiGroup` renders the original group window and gauges, membership, leader,
invitations, and invite/accept/decline/leave actions. Level, HP, and mana are
optional values from live entities; unknown values are omitted. Members outside
the current zone remain visible with disabled targeting.

Each `TextLine` can include `TextLink { range, id }`, where `range` addresses
visible UTF-8 bytes and `id` is an application-owned payload key. The renderer
underlines and colors links, splits their hit regions at wrapped rows, clips
them to the visible log, and returns logical coordinates from `ui_link_hits()`.
Invalid ranges and scrolled-out rows produce no clickable regions. The caller
must only dispatch those regions when the frame's topmost hit is `game:chat_log`,
so overlaid windows block clicks. Visible link labels are never interpreted as
commands; the application parses and sends the server's typed link metadata.

Verify merchant/bank/group layout, disabled states, stock scrolling, sale-total
semantics, and chat links at both 1× and 2× pixel density with:

```
OPENEQ_UI_CAPTURE_DIR=/tmp cargo test -p openeq --lib commerce_ui::tests -- --include-ignored --nocapture
cargo test -p openeq-render ui::tests
```

The first command writes `openeq-commerce-ui.png` and
`openeq-commerce-ui-retina.png` when the capture directory is set.

`UiTrade` uses the original `TradeWnd`, its sixteen inventory backgrounds,
coin controls, and accept/cancel button art. Each participant has eight explicitly
addressed offer indices (0–7); a partner slot produces an inspection action, never
an inventory move. Clicking an occupied own-offer slot with an empty cursor
inspects it; individual item retraction is not supported. Offered amounts and
the two acceptance flags remain separate.
Invitation, waiting, active, confirmation-pending, and ended states have distinct
controls. An ended exchange displays the application's outcome text; the UI does
not infer success from a generic server finish message. Offered coins cannot be
removed individually, so the window explains that cancellation returns them.
The application owns invitation, transfer, acceptance, cancellation, and item
inspection behavior through `UiAction::Trade(TradeAction)`.

Persistent item inspection can expose `UiItemUse` controls for explicit Scribe
and Use actions. The application supplies the labels, readiness, reason text,
and a revalidated owned slot/instance; linked items and other players' offers
must not provide this state. Hovering never exposes these action buttons, and
holding an item does not hide an already-open persistent inspection. Rendering
does not consume an item, scribe a spell, or start a cast.

```
OPENEQ_UI_CAPTURE_DIR=/tmp cargo test -p openeq --lib trade_ui::tests -- --include-ignored --nocapture
cargo test -p openeq --lib trade_interaction::tests
```

This verifies original artwork, explicit slot hits, disabled actions, pending
and failed exchanges, and item-use visibility. With a capture directory, it
writes Retina captures `openeq-trade-ui.png`, `openeq-scribe-ui.png`, and
`openeq-item-use-ui.png` using verified client icon IDs.
The binding tests use local command channels to verify player/session addresses,
separate offer inventories, inspection-only offered items, currency enqueueing,
and single cancellation without connecting a character to the server.
