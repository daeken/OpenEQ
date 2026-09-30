# Account screens and input

`account_ui::AccountUi` loads the original default `EQLSUI.xml` login/server
artwork and `EQUI.xml` character-select controls. Optional missing skin files use
plain local controls. The main menu exposes Play, Connection settings, and Exit.
The account flow exposes sign-in, selection, appearance preview, character
creation, play, refresh, back/cancel, and exit. Delete, tutorial, upgrades, store,
and chat controls are hidden. Character-select buttons retain their original frame art, replacing
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
creation has its own local editor below; roster preview does not edit an existing
character.

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

## Character creation

**Create** on the roster (**New** in compact layouts) opens a local five-page
editor: name/identity, starting choices, attributes, appearance, and review.
Opening or browsing it sends no name approval or character creation request.
The roster button is enabled only when the retained world connection has a
creation catalog, complete capability advertisements, an available character
slot, and at least one permitted combination. A pending creation disables a
second creation and entry into the world.

`account_creation::editor::Editor` derives race, class, deity and starting-zone
choices from the server's actual combinations, filtered by expansion and
membership capabilities. Changing a parent choice selects another advertised
combination; it never invents a cross-product of independently valid values.
Standard race/class/deity/city names are display labels only; unfamiliar start
IDs remain explicitly labeled server starting zones. The final server-selected
zone can differ from the requested start. Gender is limited to the two supported
player model variants.

Attributes start at the server's default allocation. Minus/plus controls allow
redistribution within the advertised base and total point budget, and Reset
restores the source defaults. The server permits unspent points. Every draft
edit advances its revision and retires the previous appearance receipt; catalog
or roster updates revalidate the choices and require a fresh preview. Changing
the session or world socket closes the local editor.

The name editor has its own buffer and composition state, separate from account
credentials and chat. It accepts up to 15 ASCII letters, normalizes capitalization,
and requires the source-backed 4–15-letter name contract before final submission.
Only the server decides name availability. Escape cancels composition before
navigating. A cancelled composition cannot commit later into the reopened name
editor or a credential field. Page transitions invalidate stale pointer actions
and keep held Enter/Escape keys inert until release.

Appearance controls come from the model family that actually loaded, rather
than the requested Classic/Luclin preference. Classic offers faces; Luclin adds
the supported hair, beard and eye choices; Drakkin offers the intersection of
authored customization metadata, class/gender support and EQEmu race limits,
including verified heritage, palette, tattoo and facial detail choices. A
provisional unsupported default is corrected locally and must be previewed again.
Classic/Luclin hair and beard palettes are not reproduced: those colors stay at
fixed source-valid defaults, and the appearance page explains that limitation.
Unsupported or unpreviewable features are not offered as sliders. Missing model
assets or required diffuse textures block submission with a visible explanation;
there is no fallback receipt for an unloaded model.

Review shows the full standing avatar with rotation controls, Back, Close and
the only final **Create** action. Submission requires a receipt matching the
exact draft revision, session/socket, catalog/roster revisions, loaded family,
race/class/gender and appearance. The controller and worker independently
revalidate the frozen submission. Name approval can reserve durable server state,
so closing after dispatch only detaches the screen while the same bounded
transaction finishes. Pending, completed, name-rejected, creation-rejected,
cancelled-before-dispatch and unknown outcomes have distinct presentations.
Explicit rejection offers **Edit draft**; unknown outcomes direct the player to
sign in again and inspect the roster. No automatic retry or delete is attempted.
After closing the editor, the roster retains the operation's pending or final
status, including rejections, above its controls. An unknown outcome disables
new creation until the player reconnects to inspect the roster. The pending
message explains that creation continues while the screen is closed.

Pages reuse original XML skins at logical display scale. Narrow layouts stack
field text above its controls and scroll the rows; Tab/Shift-Tab and up/down
keep focused rows visible. The review buttons remain outside the avatar area at
800×600, 320×240 and 180×640. This is a new layout using original artwork, not a
claim of native-client layout or appearance fidelity.

## Integration contract

- Call `AccountInput::sync(&View)` after polling account state and before each
  presentation, including frames without native input or an appearance request.
  Session failures must retire the old editor and expose the current account
  notice immediately. Paint also rejects an editor from a different session.
- Feed `AccountInput::event` the combined native `WindowEvent` stream in order.
  While account UI is active, **all events are captured**, even when the method
  returns `None`. Do not also route them to chat, movement, or generic HUD input.
- `Intent::SelectWorld`/`SelectCharacter` update the current local selection.
  `Play`, `Refresh`, `Back`, and row intents carry the account token; controller
  actions must validate it. Hit IDs include the token and stage, and input rejects
  frames drawn for a different account revision, stage, or local menu revision.
- `creation_request` supplies the current preview request and draft context;
  pass its matching result to `apply_creation_preview`. If policy defaults change
  the draft, retire the old model request immediately. `Intent::Create` carries
  the frozen submission; `CancelCreation` carries the exact active operation.
- Set window IME availability from `ime_enabled(&View)` and native focus.
- After successful handoff, retain the input object and call
  `begin_handoff_frame` once before routing each gameplay event batch. Feed every
  event to `filter_handoff_event` in native order; a true result must be skipped
  before chat sees it. This also suppresses physical shortcut edges when an owned
  repeat and release arrive together, while allowing a fresh press after release.
  Drain through the same filter during loading so releases retire ownership.
  Do not call `reset` at handoff: it clears ownership for cancellation/shutdown.
- The UI opens no connection, persists no credentials, and initializes no audio.

UI tests exercise Unicode/IME secrecy, focus/modifier ordering, bounds,
port validation, disabled/stale row handling, scrolling, key ownership across
handoff, fallback frames, menu navigation, endpoint draft validation, and stale
menu input, preview navigation/rotation, disabled selections and stale preview
input. Eight preview CPU tests cover roster-to-model conversion, camera
framing, bounded cancelled workers, terminal load failures, loaded-family
creation policy, exact request matching and missing diffuse textures; five world tests
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

The creation editor adds five CPU tests for dependent combinations, stat bounds,
missing capabilities/capacity, snapshot invalidation, actual-family defaults and
exact receipt matching. Eight UI tests cover local-only browsing, the final
submission gate, isolated/cancelled name IME, held keys, a catalog/roster refresh
between mouse press and release, pending/unknown outcomes, stale presentation
without input events, detached results, and compact control bounds. Account
tests and the follow-up UI tests passed; strict app all-target Clippy also passed.

The separately run original creation GPU test loads a real Classic preview,
obtains its matching receipt, checks the enabled final Create control, and
captures all five pages at 800×600 logical pixels at 1x/2x plus 320×240 and
180×640. Pending screens add 1x/2x captures, for 22 images under
`OPENEQ_CREATION_CAPTURE_DIR`. Avatar visibility and missing-texture colors are
checked. The test passed and normal, Retina and narrow images were inspected in
`/tmp/openeq-creation-ui`; narrow deity text and the Create button were corrected
after inspection and recaptured. These creation tests use no live account,
reserve no names, create no server characters and play no audio.

A separate outcome GPU test adds 40 captures at the same four sizes/scales:
pending, completed, both rejection types, unknown, cancelled, and detached
pending/rejected/unknown states, including a pending operation with an existing
roster character. Inspected captures confirmed that result instructions remain
readable and do not imply remote cancellation or retry of an unknown outcome.
The detached roster now preserves rejection messages and keeps its compact
pending message above the buttons; these corrections have CPU regression checks.
Outcome captures use the `outcome-` filename prefix in the same directory.

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
cargo test -p openeq --lib --no-default-features account
cargo clippy -p openeq --all-targets --no-default-features -- -D warnings
OPENEQ_CREATION_CAPTURE_DIR=/tmp/openeq-creation-ui \
  cargo test -p openeq --lib --no-default-features \
  account_ui::creation::tests::original_creation_pages_at_both_scales_and_compact \
  -- --ignored
OPENEQ_CREATION_CAPTURE_DIR=/tmp/openeq-creation-ui \
  cargo test -p openeq --lib --no-default-features \
  account_ui::creation::tests::original_creation_outcomes_at_both_scales_and_compact \
  -- --ignored
```

The original-asset test uses `EQ_CLIENT_DIR` when set, otherwise
`$HOME/EverQuest`. Captures and proprietary artwork remain outside the repository.
