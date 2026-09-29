# Interactive account, server, and character selection

Source investigation, bounded protocol prerequisites, and staged controller,
2026-09-29. The protocol and controller changes described below are implemented;
dedicated-account live validation is recorded separately below. The integration
outline records the original design and the constraints still used by the UI.

The existing transport can already authenticate an account, list worlds, choose
a world, list its characters, and enter an existing character. The first usable
UI needs a staged session worker and screen/input binding, not another login
protocol implementation. Keep the existing private-file `--connect` path for
headless probes and direct entry.

## Current path and reusable APIs

`crates/openeq-net/src/session.rs::ConnectionConfig::connect` currently performs
every stage in one call:

1. Resolve the configured login endpoint to IPv4 and connect.
2. Authenticate, receiving account ID and the transient world session key.
3. Fetch worlds. With a configured `server_id`, select that ID; otherwise select
   the first entry whose `is_up()` is true (status 0 or 2).
4. Request access to that world, then connect to its advertised IP and the
   configured world port (default 9000).
5. Read the character roster and require the configured character name to exist,
   comparing names case-insensitively.
6. Enter that character, connect to the supplied zone address, and install the
   authenticated world handoff used by later zoning.

The selected world/character are decisions embedded in that convenience method;
they are not protocol restrictions. The lower-level public APIs already expose
the necessary pause points:

| Stage | Existing API | Data available to the UI |
| --- | --- | --- |
| Login transport | `LoginClient::connect(SocketAddr)` | Typed connection failure |
| Authenticate | `LoginClient::login(username, password)` | `Session` with account ID and private transient key |
| List/refresh worlds | `LoginClient::server_list()` | ID, name, address, type, country, language, status, player count |
| Join chosen world | `LoginClient::play(server_id)` | Accepted or rejected, stream/malformed/timeout errors |
| World transport | `WorldClient::connect(address, account_id, key)` | Authenticated world socket |
| Character roster | `WorldClient::characters()` | Name, level, class, race, gender, last zone/instance, Enabled flag |
| Enter chosen character | `WorldClient::enter_world(name)` | Actual zone IP and port, or zone unavailable/error |
| Selected-character handoff | Public `session::enter_character(&mut WorldClient, world_address, &Session, character)` | Connected `ZoneClient` with authenticated later zoning enabled |

`WorldClient::connect_zoning` is specifically for an existing character's zone
handoff. Returning to character selection should use the non-zoning connection
mode and a valid authenticated session instead.

`session::enter_character` shares the existing `enter_world` → zone connect →
authenticated handoff setup with `ConnectionConfig::connect`. It borrows the
authenticated session and clones its private transient key into the zone's
handoff; raw `ZoneClient::enable_zoning` remains crate-private. Call it only for
the selected enabled entry in the current roster and on the same live runtime
as the world connection.

`LiveWorld::start(config)` retains the automatic private-file connection path.
Interactive entry uses `AccountController`, whose worker performs authentication,
publishes world and character rows, and waits for token-checked user actions.
Both paths run the same `NetworkIo` event/command/movement loop. Login, world,
and zone sockets stay on their original worker's current-thread Tokio runtime;
the foreground receives a nonsecret `SessionIdentity` and `LiveWorld`, not a
socket moved into a fresh runtime. The server's environment name selects the
actual zone assets.

## Exact protocol evidence

The current opcodes are in `crates/openeq-net/src/opcodes.rs`; this path uses the
SoD/RoF2 login dialect and RoF2 world/zone layouts.

| Exchange | Existing representation | EQEmu source |
| --- | --- | --- |
| Session-ready/login | `SessionReady=0x0001`; `ChatMessage=0x0017`; `Login=0x0002`; `LoginAccepted=0x0018`. Credentials are NUL-separated and DES-encrypted after a 10-byte header. The accepted reply decrypts 80 bytes; account ID is at decrypted byte 8 and the 11-byte key at byte 12. | `loginserver/client_manager.cpp` SoD opcode configuration; `loginserver/login_types.h::LoginBaseMessage`, `PlayerLoginReply`; `loginserver/client.cpp::SendFailedLogin` and successful reply handling |
| Server list | Request `0x0004`, reply `0x0019`; 10-byte base header + 6-byte reply header, count at byte 16, entries at byte 20. Entry is address C-string, type u32, ID u32, name/country/language C-strings, status u32, players u32. | `loginserver/world_server_manager.cpp::CreateServerListPacket`; `loginserver/world_server.cpp::SerializeForClientServerList` |
| Join world | Request `0x000d`, response `0x0022`; request is 10-byte header + selected world ID u32. Response contains success at byte 10, error string ID at 11, string byte at 15, and selected world ID at 16. | `loginserver/login_types.h::PlayEverquestRequest`, `PlayEverquestResponse`; `loginserver/world_server.cpp` user-to-world response handling |
| World login | `SendLoginInfo=0x7a09`; exactly 464 bytes, account text + NUL + session key in `login_info[64]`; zoning flag at byte 188. The packed fields total 464 despite the stale terminal offset comment in the C++ header. | `common/patches/rof2_structs.h::LoginInfo_Struct` |
| Character roster | `SendCharInfo=0x00d2`; u32 count followed by NUL-terminated name and a 274-byte fixed tail per entry. Current fields read at tail offsets class 0, race 1, level 5, zone 11, gender 15. | `common/patches/rof2.cpp::ENCODE(OP_SendCharInfo)`; `rof2_structs.h::CharacterSelect_Struct`, `CharacterSelectEntry_Struct` |
| Enter character | `WorldClientReady=0x23c1`, then `EnterWorld=0x578f`, 72-byte payload: name[64], tutorial u32, return-home u32. Current client sends both flags zero. | `rof2_structs.h::EnterWorld_Struct`; `world/client.cpp::HandleEnterWorldPacket` |
| Zone handoff | `ZoneServerInfo=0x4c44`, at least 130 bytes: IP string[128], port u16; `ZoneUnavailable=0x4cb4` is already a typed error. | Existing `WorldClient::enter_world` and EQEmu world handoff |

The login server's newer Larion dialect inserts additional address/ID fields;
do not reinterpret the existing RoF2 server list as that dialect. Likewise the
listed world port is not present in the current SoD entry: retain the explicit
world-port setting rather than guessing a port from list bytes.

EQEmu checks that the selected character belongs to the authenticated account in
`world/client.cpp::HandleEnterWorldPacket`; local row selection must use the
current roster, but it never grants ownership. That handler also validates
return-home/tutorial eligibility. Those buttons must stay unavailable until
their roster flags and request behavior are implemented.

## Original XML windows available now

The installed original assets contain both login and character-selection skins,
and `openeq-ui` already loads them. The asset opt-in tests in
`crates/openeq-ui/tests/client_assets.rs` verify the login controls, password
masking, atlas images, and a character-list window with hit targets.

| Stage | Root document / window | Controls to bind first |
| --- | --- | --- |
| Credentials | `EQLSUI.xml` / `connect` from `EQLSUI_ConnectWnd.xml` | `UsernameEdit`, `PasswordEdit`, `ConnectButton`, `CancelButton` |
| World selection | `EQLSUI.xml` / `serverselect` from `EQLSUI_ServerSelectWnd.xml` | `ServerList`, `PlayButton`, `ExitButton`, optional last-server label |
| Character selection | `EQUI.xml` / `CharacterListWnd` | `Character_List`, `Play_Button`, `Quit_Button`, `Count_Label` |

`EQUI_ServerListWnd.xml` is a character-transfer dialog with name-in-use/free-
transfer columns. It is **not** the login server picker. The older
`EQUI_CharacterSelect.xml` also exists, but its eight hard-coded character buttons
are a worse fit for the current RoF2 roster than `CharacterListWnd`.

`UiBindings::widget_mut` already supplies text, visibility, enabled state,
selection styling, and password masking. XML layout supplies control rectangles
and hit targets. Listboxes currently provide their enclosing rectangle/hit
target, not an application-owned dynamic row model. For the first slice, render
clipped server/character rows inside those XML rectangles and add per-row logical
hit targets, following the existing commerce presentation pattern. Bind rows by
world ID / current character identity, never by a stale display index. Clamp
scrolling and support keyboard up/down/Enter as well as clicks.

Login needs two editable fields and focus traversal. The current ordered
`WindowEvent`/IME and focus-loss work in `input.rs` is useful, but `ChatInput`
is tied to chat. Add a small field editor or extract its text-editing core without
allowing login keystrokes to reach gameplay bindings. Password raw text should
remain in private input state; draw only masking characters and never put secret
state into diagnostic captures/debug output.

Hide or disable account purchase, expansion purchase, marketplace, character
creation/deletion, heroic upgrades, tutorial, return home, and model rotation in
the first slice. Supply an honest empty-roster message. A character's textual
name/level/class/race/last-zone is enough to select and enter it; a full 3D
character preview can follow when the remaining appearance fields are decoded.

## Smallest usable integration

1. **Staged background connection worker.** Extract the existing automatic
   sequence into a worker-owned connector with states `Credentials`,
   `Authenticating`, `Worlds`, `JoiningWorld`, `Characters`, `EnteringZone`,
   `Playing`, and recoverable failure. UI commands choose a world ID or character
   from that state's current list. Attach a request generation to commands and
   results so Cancel/Back/new login invalidates late results and double-clicks
   cannot send a second play/enter request. Keep `ConnectionConfig::connect` as
   the automatic consumer of the same stage functions for existing probes.
2. **Preserve socket/runtime ownership.** The same background Tokio runtime must
   own login, world, and eventual zone streams. `EqStream` spawns reader/ticker
   tasks on its creation runtime and aborts them on drop. Do not prepare a
   `ZoneClient` in a short-lived asset-job runtime and move it into a new live
   worker. Extract the current live command/event loop to an async helper and
   call it after interactive handoff on the same runtime. Publish selected
   character/world identity to the foreground before any zone event. The normal
   `LiveWorld` readiness, movement authority, zoning, and asset loader then apply.
3. **Screen controller plus one presentation module.** Add a small application
   reducer and `account_ui.rs`, with loading/progress/error text and actions.
   Start the window without a zone argument for interactive mode; preserve
   explicit offline zone viewing and automatic `--connect` behavior. Load the
   login skin early. Do not load a guessed zone or start gameplay input until
   zone Ready and the server-selected assets are installed.
4. **Endpoint and last-choice preferences.** The original login window has no
   EQEmu host control. Add a compact connection-settings affordance for login
   host/port and world port, or accept a reusable nonsecret endpoint profile.
   Persist endpoint and last selected world/character only; first implementation
   can keep the password in memory for the active login flow and clear it on
   logout/cancel. Preserve the existing private-file path without silently
   rewriting its selected character. Defer password persistence to an explicit
   OS credential-store implementation.
5. **Bind existing per-character state to the chosen identity.** Current
   `ui_layout.rs::Identity::from_config` includes endpoint, world ID, and character
   and intentionally excludes credentials. Introduce the equivalent nonsecret
   session identity for interactive play, then load its layout only after the
   player chooses a character. Do not invent a temporary config containing
   credentials merely to address layout preferences.

Back from a character roster closes that world socket and returns to the
still-valid login/world list. If that login session has expired, return to the
credentials screen with a clear reason. Before EnterWorld is sent, Cancel drops
owned streams and abandons in-flight futures. Once entry may have committed,
cancellation completes the bounded zone handshake solely to send normal logout;
it never publishes the cancelled foreground world. Initial scope does not
implement camping from a running character into the roster: that is a separate
logout/camp lifecycle that must await the existing zone logout path.

## Controller and cancellation contract

`AccountController::sign_in` starts a fresh attempt from `Stage::Credentials`.
`poll()` applies world/character/error replies and returns `Ready` only for the
current attempt. `action(Token, Action)` accepts world IDs and enabled character
names from the current rows. Tokens carry attempt and list revision; stale rows,
wrong-stage actions, and double submissions do not send requests. Refresh advances
the list revision and preserves a selection by identity when it remains present.
The returned `Ready` contains endpoint, selected world ID, selected character,
and `LiveWorld`. Destroying the controller after consuming it does not destroy
the runtime still serving that world.

`cancel()` invalidates the attempt immediately and resets the view to credentials.
The worker marks entry committed before calling `enter_character`. Cancellation
before that point drops the pending future; after it, the worker allows at most
45 seconds to complete entry and uses `live::logout_zone` to finish the zone
handshake, with a further 20-second bound, then send Logout. A cancellation flag
check also covers a ready connection racing the cancellation notification.
Dropping a queued stale `Ready` closes its live channels and follows that same
cleanup helper. Thread-spawn failure restores credentials so sign-in can retry.

Waiting for zone Ready during cleanup is protocol-required. EQEmu registers
`OP_Logout` in `ConnectedOpcodes`, not `ConnectingOpcodes`; `HandlePacket` dispatches
by connection state. `Handle_Connect_OP_ClientReady` calls `CompleteConnect`, which
sets `CLIENT_CONNECTED`. OpenEQ emits its local `ZoneEvent::Ready` after sending
ClientReady on the reliable stream, so a following Logout reaches the connected
handler in order. Sending Logout immediately after ZoneClient creation would be
ignored by the connecting handler. The helper gives the stream a short drain
after Logout, while smoke tests still require database-confirmed offline state.

## Implemented protocol prerequisites

- `parse_server_list` uses checked cursors and rejects incomplete or trailing
  data as a whole. It bounds count before allocation (1024 entries plus a
  minimum remaining-byte check), bounds names to 200 bytes and address/locale
  fields, requires terminated strings, checks reply success, rejects duplicate
  or zero server IDs, and reports invalid addresses instead of substituting
  loopback. Its request sequence/header checks match EQEmu's echoed sequence.
- `parse_characters` now returns `Result`, distinguishing a genuine empty list
  from a malformed one. It requires complete UTF-8 names of 1–63 bytes,
  case-insensitively unique names, complete 274-byte tails, the RoF2 source limit
  of 12 characters, and no trailing bytes. `Character` now exposes
  `instance_id: u16` (tail offset 13) and `enabled: bool` (tail offset 268),
  validating Enabled as 0/1. Tests explicitly distinguish it from GoHome at 261.
- `LoginError::Rejected { context, reason }` preserves a typed
  `RejectionReason`: invalid credentials (105), already-online character (111),
  unavailable world (326), suspended account (337), banned account (338), full
  world (339), or `Unknown(code)`. `reason.code()` preserves the original number;
  Display provides a useful message without private packet contents. These are
  EQEmu `LS::ErrStr` constants plus `Client::SendFailedLogin`'s code 105.
- Play replies must be exactly 20 bytes with complete base headers, a valid
  success flag/string terminator, expected sequence, and the requested server
  ID. A mismatched response cannot grant access even if its success byte is set.
  Authentication replies must be the source-defined 90 bytes, contain a valid
  header/decrypted base reply, a nonzero account ID, and a terminated nonempty
  ASCII key. `Session` Debug is explicitly redacted.
- The automatic `ConnectionConfig::connect` API remains available with the same
  configuration format and selection behavior. It now refuses a selected
  disabled roster entry with an actionable error. Lower-level stage API method
  signatures remain unchanged; `characters()` already returned `Result` and now
  propagates checked roster-decoding failures instead of accepting partial rows.
  Both automatic and interactive entry can use the public `enter_character`
  helper without duplicating handoff setup or exposing raw zoning credentials.
- Portable tests cover valid empty/multiple lists, all byte-prefix truncations,
  excessive counts, malformed fields/headers, trailers, duplicate identities,
  disabled/instanced characters, exact/mismatched play responses, source-backed
  and unknown rejection codes, malformed authentication identity, and debug
  redaction. All 76 network library tests and strict all-target Clippy passed.

## Verified selection-screen idle behavior

The bounded `account_idle_smoke` probe used only the existing disposable recovery
account and Reviver, with a private pre-login snapshot. It sent no invented
application Poll, added no keepalive, changed no rules, and created no accounts.
Its only normal login actions were authentication, server-list refresh, world
selection, roster retrieval, and existing-character selection.

`/tmp/openeq-account-idle-1.log` passed on 2026-09-29:

- The initial world list contained one entry. After **35.110 seconds** without
  another application request, refreshing returned the current list successfully
  on the same authenticated login client. No fresh authentication was necessary.
- World access passed the complete 20-byte reply/server-ID check. The checked
  roster contained Reviver, `enabled=true`, zone **77**, instance **0**.
- After **35.123 seconds** on that roster, selecting Reviver succeeded on the
  same world connection. The zone supplied Reviver's own living spawn, profile,
  and Ready. No outgoing movement or gameplay actions were sent.
- The probe sent normal zone logout and verified Reviver was offline. Original
  pose/resources were restored after logout; inventory, cash, all binds, spell
  book/gems, buffs, corpse count, level/XP/stats, and identity matched the saved
  state. No other character was used.

```sh
cargo run -p openeq-net --bin account_idle_smoke -- \
  "$HOME/.config/openeq/storage2-recovery-credentials.json" \
  /tmp/openeq-account-idle-next-run.restore.sql
```

Use a new snapshot filename each run. It is created mode 0600 before login and
never overwritten. The probe recognizes closed-session outcomes explicitly,
re-authenticates at most once for the server-list stage, and can verify clean
entry through the automatic path if the world stage expires. Neither fallback
was needed in this observed run. The test establishes that **35-second pauses
work with this EQEmu deployment**; it does not promise indefinite idle survival
or silently change the 30-second transport inactivity/retransmission deadlines.
Controller cancellation is covered by the later probe below. UI Back navigation,
longer idle periods, and other login deployments remain separate checks.

## Verified interactive-controller flow

`crates/openeq/src/bin/account_smoke.rs` drives only public
`AccountController`/`Action`/`Stage` APIs. It loads the private recovery config for
credentials and endpoint, but never calls `ConnectionConfig::connect`. The
fixture guard requires `storage2.daeken.dev`, account `openeq_recovery`, and
character `Reviver`; the character must start offline with no items, buffs, or
corpses. A mode-0600, create-new restoration snapshot is written before login.

`/tmp/openeq-account-controller-1.log` passed on 2026-09-29, with restoration
snapshot `/tmp/openeq-account-controller-1.restore.sql`:

- Sign-in published the world list. After **35.012 seconds**, refresh succeeded
  on the same authenticated session and advanced the list revision. The probe
  selected advertised world **1**, then verified enabled Reviver in Arena
  **77**, instance **0**. After **35.006 seconds** on the character screen,
  choosing Reviver produced controller `Ready` with the correct endpoint,
  server ID, and character identity.
- The probe destroyed the controller while retaining `Ready.live`. That live
  state still received Reviver's profile, own living spawn **30**, and zone
  Ready, then stayed healthy for another two seconds. This exercises the real
  worker/runtime lifetime transfer. Normal live drop logged cleanup with
  `ready=true`, sent Logout, and reached database-confirmed offline state.
- A second controller entry waited for the `entering zone` diagnostic after
  the world's EnterWorld reply, then cancelled without polling a foreground
  Ready. The old token was rejected, the view stayed at Credentials, and no
  stale Ready escaped. Cleanup started with **`ready=false`**, completed the
  handshake, and sent Logout. Only after the completion diagnostic did the
  probe accept the database's offline result. This covers actual server-
  committed entry cancellation, not just discarding an unsent selection.
- Duplicate sign-in/refresh/world/character actions, an old attempt, an old
  list revision, unadvertised world ID, unlisted character, wrong-stage character
  choice, and Back during entry were rejected without changing the active
  selection. The logged session identity matched the actual chosen rows.
- No player position or gameplay requests were sent. Reviver ended offline;
  pose/resources were restored exactly. Level, XP, stats, inventory, cash, all
  binds, spellbook/gems, buffs, and corpse count matched the snapshot. Neither
  the cleanup reconnect fallback nor any other character was needed.

```sh
cargo run -p openeq --bin account_smoke -- \
  "$HOME/.config/openeq/storage2-recovery-credentials.json" \
  /tmp/openeq-account-controller-next-run.log
```

Use a fresh log name; the probe refuses to replace its log or restoration file.
Strict Clippy for this binary passed. This is a live controller and protocol
check; visual account-screen captures and keyboard/mouse behavior have separate
UI checks. It does not claim indefinite idle, unavailable-zone recovery, or
character creation/deletion support.

## Remaining integration considerations

- `Character` still omits return-home/tutorial flags, appearance, equipment,
  and last-login fields present in the tail. Keep richer appearance and special
  entry actions for later. No new server request is needed for these fields.
- `EqStream` enforces 30 seconds of incoming inactivity and 30 seconds of
  unacknowledged reliable data. Its ticker retransmits/ACKs but does not generate
  outbound keepalives. Incoming valid server keepalives do refresh it. Human
  selection screens therefore require explicit long-idle testing and closed-
  session handling; immediate automated connect success proves neither. Do not
  invent an application Poll heartbeat: the inspected login server switch only
  handles session-ready, login, server-list, and play.
- Status is a bit field: Up 0, Down 1, compatibility/unused 2, Locked 4 (possibly
  combined with Down). Show status honestly. Locked-world access may depend on
  server-side account privileges; current explicit `server_id` selection still
  attempts it, while automatic selection uses `is_up()`. Preserve server
  authority instead of inferring account privilege from a row.

## Acceptance checks for implementation

- Portable codec fixtures: successful/failed login and play replies; several
  worlds with distinct statuses; empty/multiple/disabled character rows; maximum
  and malformed counts, missing terminators, truncated tails, invalid addresses,
  and selected-world mismatch. Verify no credential/key Debug output.
- Pure controller tests: generation changes, stale row/click rejection,
  double-submit suppression, Cancel/Back while a stage is pending, empty roster,
  error/retry, selection surviving a refresh by stable identity, and a closed
  session returning to an actionable screen.
- Original-asset captures: credentials, busy/error states, scrolled server list,
  character list, empty roster, small viewport; password text absent from draw
  commands/captures. Test focus traversal, IME, scrolling, and keyboard capture.
- The dedicated-account controller smoke above covers selection-screen pauses,
  refresh, advertised world/character selection, identity, live profile/Ready,
  stale/double rejection, committed-entry cancellation, and normal logout.
  Unavailable-zone and longer-idle retry scenarios remain separate acceptance
  checks. Reuse disposable accounts only with fixture coordination.

This first slice delivers credentials → server list → existing character list →
the current playable world. Character creation/deletion, entitlement emulation,
return-home/tutorial entry, 3D previews, and camping back to selection remain
separate, source-backed additions.
