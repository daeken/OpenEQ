# Account screens and input

`account_ui::AccountUi` loads the original default `EQLSUI.xml` login/server
artwork and `EQUI.xml` character-select controls. Optional missing skin files use
plain local controls. The main menu exposes Play, Connection settings, and Exit.
The account flow exposes sign-in, selection, appearance preview, play, refresh, back/cancel, and exit;
create/delete, tutorial, upgrades, store, and chat controls are hidden. Character-select buttons retain their original frame art, replacing
the baked label decals in memory so they do not overlap application labels.

The screen uses logical pixels; the renderer applies display scale. Lists draw
at most eight clipped rows, bounded to 2,048 worlds or 256 characters. Unavailable
worlds/characters cannot be selected or played. Wheel/Page Up/Page Down scroll,
arrow keys select available rows, Tab/Shift-Tab move focus, Enter activates the
focused control, and Escape cancels composition before navigating back.

The credential editor is separate from `ChatEditor`. Its buffers and IME state
are private and have no Debug, Display, or serialization implementation. Password
and password composition are masked before constructing any draw commands or
bindings. `take_credentials()` moves the password out and clears its editor.
Cancellation clears the password while retaining nonsecret connection fields.
Late commits from a cancelled composition cannot move password text to another
field. Editing respects UTF-8 character boundaries and per-field byte limits;
ports accept only ASCII digits and are validated before sign-in. The editor
supports Home/End, Backspace/Delete, arrows, Command/Ctrl-A, and Command/Ctrl-U.
Clipboard integration and drag text selection are not implemented.

## Main menu

No-argument startup and `--login HOST` use `AccountInput::with_main_menu`. Play
opens the existing sign-in screen with username focus; Connection settings edits
the hostname and two ports. Done validates and applies the local edits, while
Back/Escape discards that settings draft. Endpoint preferences still persist only
after successful world entry. No authentication or network connection starts
from either local page. The original `main` background and `MAIN_*` button skins
are reused without exposing unsupported account/web/help controls.

Back from credentials clears the password and IME composition before returning
to the menu. A local navigation revision rejects stale frames, pointer releases
and composition events; keys held across navigation remain suppressed until
release. The welcome page disables text input. `AccountInput::new` retains direct
credentials behavior for existing callers. Network cancellation and roster back
behavior are unchanged.

## Selected-character appearance preview

Select an enabled roster character and choose **Preview**. The local inspection
screen shows a standing model, with Rotate left/right buttons and arrow keys;
Back or Escape returns to the same roster selection. Tab/Enter also operate the
controls. Opening, rotating and closing the preview emit no network command,
enter no zone, persist no character change and initialize no audio. Character
creation, appearance editing and camp-to-roster remain separate work.

The appearance is decoded from the existing RoF2 SendCharInfo roster, using
EQEmu `common/patches/rof2_structs.h::CharacterSelectEntry_Struct` and
`rof2.cpp::ENCODE(OP_SendCharInfo)`, at source revision
`4aceae18b94ffaafc08e2b17bc41cd72c77f795d`. After the variable name, the 274-byte
tail contains Face at16; nine 24-byte equipment records at17; Drakkin tattoo and
details at235/239; primary/secondary IDFile models at247/251; six hair/beard/eye
bytes at255–260; and heritage at263. The existing Enabled flag remains at268.
Equipment preserves material, unknown1, elite material, Hero's Forge, material2
and packed tint separately. EQEmu `world/worlddb.cpp` writes each dedicated held
model and its corresponding equipment material from the same item/ornament;
the preview uses those explicit primary/secondary models.

Rendering uses the configured `--models classic|luclin` preference and the
existing character pipeline, including its fallback behavior. Supported local
features include classic armor/tint/face/robes and rigid held models, Luclin
modular armor/hair/beard/faces/eyes, and Drakkin modular appearance plus the
verified customization palette. Models use a fixed standing pose and normalized
inspection scale, not live character height. Camera framing includes equipment
and all rotations, leaving the header and controls accessible on narrow views.

Preserving a wire field does not imply rendering it: elite materials, Hero's
Forge models, unknown1/material2, Luclin hair/beard tint palettes and animated
held-item skeletons/effects remain unsupported. The preview states that some
appearance details are unavailable; it makes no native-client fidelity claim.
Missing character assets show a recoverable preview message and do not disable
Enter world on the roster.

`account_preview::Preview` owns at most one prepared appearance and one asset
job. Cancellation invalidates publication immediately but retains the job until
it exits, so rapid Back/reopen/selection changes cannot spawn parallel loaders.
Worker results match the full account token, character data, model preference
and asset directory. Changing the roster or session invalidates the old result;
failure does not automatically retry every frame. A separate local input
revision rejects stale preview frames and preserves held-key ownership until
release, including Escape/Enter across the return to the roster.

## Integration contract

- Feed `AccountInput::event` the combined native `WindowEvent` stream in order.
  While account UI is active, **all events are captured**, even when the method
  returns `None`. Do not also route them to chat, movement, or generic HUD input.
- `Intent::SelectWorld`/`SelectCharacter` update the current local selection.
  `Play`, `Refresh`, `Back`, and row intents carry the account token; controller
  actions must validate it. Hit IDs include the token and stage, and input rejects
  frames drawn for a different account revision, stage, or local menu revision.
- Set window IME availability from `ime_enabled(&View)` and native focus.
- After successful handoff, retain the input object and call
  `begin_handoff_frame` once before routing each gameplay event batch. Feed every
  event to `filter_handoff_event` in native order; a true result must be skipped
  before chat sees it. This also suppresses physical shortcut edges when an owned
  repeat and release arrive together, while allowing a fresh press after release.
  Drain through the same filter during loading so releases retire ownership.
  Do not call `reset` at handoff: it clears ownership for cancellation/shutdown.
- The UI opens no connection, persists no credentials, and initializes no audio.

Nineteen UI tests exercise Unicode/IME secrecy, focus/modifier ordering, bounds,
port validation, disabled/stale row handling, scrolling, key ownership across
handoff, fallback frames, menu navigation, endpoint draft validation, and stale
menu input, preview navigation/rotation, disabled selections and stale preview
input. Four additional preview tests cover roster-to-model conversion, camera
framing, bounded cancelled workers and terminal load failures; five world tests
cover the roster parser, including all appearance offsets and truncated input.
Ignored original-asset/GPU tests check login, world, character, and busy frames
at 1x and 2x. The main-menu/settings test adds six captures at 1x, 2x and compact
320×240. With `OPENEQ_UI_CAPTURE_DIR` set, these fourteen captures are written
there. These tests perform no real login and no audio playback.

The appearance test renders Classic, Luclin and Drakkin fixtures from decoded
roster-shaped data, at 800×600 logical pixels with 1x/2x scale and at 320×240.
Front/back views produce 18 images under `OPENEQ_PREVIEW_CAPTURE_DIR`; model
visibility, missing-texture colors and changed rotation are checked. The actual
Preview/Back hit targets are exercised without logging in. The targeted run
passed, and ordinary, Retina and compact captures were visually inspected in
`/tmp/openeq-account-preview`. This is local protocol/asset/UI verification, not
a claim of live roster capture or native-client pixel parity.

Native macOS keyboard QA also passed menu → settings → back → credentials,
username/password typing and masking, credentials → menu → credentials with the
password cleared, and normal exit. Only dummy strings were entered; sign-in was
never submitted and the process had no network sockets. Captures use the prefix
`/tmp/openeq-menu-native-`. The final compact padding/footer wording is covered
by the subsequent original-art GPU captures.

```sh
cargo test -p openeq --lib account_ui --no-default-features
OPENEQ_UI_CAPTURE_DIR=/tmp/openeq-account-ui \
  cargo test -p openeq --lib account_ui --no-default-features -- --include-ignored
OPENEQ_PREVIEW_CAPTURE_DIR=/tmp/openeq-account-preview \
  cargo test -p openeq --lib original_roster_previews --no-default-features -- --ignored
```

The original-asset test uses `EQ_CLIENT_DIR` when set, otherwise
`$HOME/EverQuest`. Captures and proprietary artwork remain outside the repository.
