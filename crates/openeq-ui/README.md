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
- Named definitions, texture atlases, timed animations and explicit frame index.
- Screens and nested pieces, labels, buttons with state art, edit boxes, gauges,
  static images/frames, inventory backgrounds, and basic tab page selection.
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
Grid animation cells and `StaticTintedBlendAnimation` are not implemented.
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
