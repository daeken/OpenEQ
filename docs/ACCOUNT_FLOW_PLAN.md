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

- `Character` now decodes appearance and equipment for the selected-character
  preview. Return-home/tutorial flags and last-login still need explicit
  handling before special entry actions are exposed. No new server request is
  needed for those fields.
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
return-home/tutorial entry and camping back to selection remain
separate, source-backed additions.

Selected-character appearance previews were subsequently implemented from the
received roster fields, with local rotation and cancellable asset loading.
See `ACCOUNT_UI.md` for original-asset/GPU verification and remaining material,
tint and animated-equipment limitations.

## Next lifecycle slice: camp to the same world's fresh roster

Source audit: EQEmu revision `4aceae18b94ffaafc08e2b17bc41cd72c77f795d`,
2026-09-29. This section specifies the next implementation and its proof; it
does not claim a live camp or character-creation test. The installed 2016 client
assets remain a presentation reference, not proof of RoF2 wire layouts.

Camp-to-roster is smaller than creation and has a complete server path. Current
Before this implementation OpenEQ `/camp` was an immediate `/quit` alias (`chat.rs`, `hotbuttons.rs` and
`HOTBUTTON_PLAN.md`). `connect_stages` returns after entry, dropping its account
request receiver/session; `AccountController::poll` retires its channels and
`main::present_account` discards the controller. The live worker owns the zone
on that same runtime and `logout_zone` only sends Logout and drains 200ms. None
of those actions currently restores a character list.

### Camp protocol and cancellation

| Step | Source-backed behavior |
| --- | --- |
| Sit | Existing `Command::Posture` sends `OP_SpawnAppearance=0x0971`, 8 bytes: own spawn ID u16, type u16=14, parameter u32=110. Standing is parameter100. `zone/client_packet.cpp::Handle_OP_SpawnAppearance` handles both and standing disables `camp_timer` and `bot_camp_timer`. |
| Begin camp | `OP_Camp=0x28ec` (`utils/patches/patch_RoF2.conf:236`). No RoF2 translation or payload struct is registered; `Handle_OP_Camp` reads no payload. An empty application packet is sufficient for this EQEmu handler. It starts a 29,000ms server timer, stops LFP, and optionally starts a bot timer. Parcel-merchant engagement rejects camping by standing the player and emitting a message. |
| Timer | `zone/client_process.cpp:193` saves, leaves the group, updates raid/guild and mercenary state, sets `instalog=true`, and disables the timer. Expiry does **not** itself close the ordinary player's stream. A client countdown should wait 30 seconds from Camp dispatch; it must continue receiving zone events throughout. Do not infer a shorter countdown from account privileges. |
| Cancel | Send the existing standing appearance before expiry. Local Escape/Cancel/stand and server-directed standing, damage, death or travel retire the local camp attempt. Send no standing pose for an already-dead/replaced player. Countdown completion and cancel must be serialized on the worker so only one wins. |
| Complete | `OP_Logout=0x4ac6` is connected-only. `Handle_OP_Logout` calls `SendLogoutPackets`, queues LogoutReply and calls `Disconnect`. Both `OP_PreLogoutReply` and `OP_LogoutReply` map to **0** for RoF2; neither is a usable application success acknowledgment. CancelTrade(action7), also sent by death, is not camp completion. |
| Peer close | `common/net/reliable_stream_connection.cpp::Close` flushes and sends session `0x0005`; `ReliableStreamDisconnect::serialize` writes zero byte, opcode byte, and big-endian u32 connection code (six decoded bytes; its `size()` comment/helper says8). Compression/CRC follow negotiated encoding. Distinguish this from OutOfSession, malformed data, transport failure and 30-second timeout; the transport now retains the first close reason separately from `EqStream::recv(None)`. |
| GM early close | With the default `EnableHackedFastCampForGM=false`, `Handle_OP_Camp` calls `OnDisconnect(true)` immediately for a GM. A matching peer close after Camp but before intentional Logout is `DisconnectedWhileCamping`, never `Camped`: retire the zone and attempt one fresh nonzoning world reconnect with “Connection closed while camping; returning to character selection.” This also safely treats an indistinguishable kick/shutdown as disconnect recovery. The fresh roster is authoritative; no claim of saved state is made. |
| Return to world | After the old zone has actually closed, use a **new non-zoning** `WorldClient::connect` with the retained account ID/key and selected world's address. `world/client.cpp::HandleSendLoginInfoPacket:479` explicitly handles “Game -> Char Select”, sets `CLE_Status::CharSelect`, and sends membership/expansion/roster data. `connect_zoning` skips that roster and is wrong here. |

The session key remains usable while the world retains its `ClientListEntry`:
`CheckAuth` compares the login-server account ID/key; `Camp/ClearVars(false)`
clear character/zone state while retaining those fields. World/zone keepalives
maintain that entry; `CheckStale` removes it after more than20 ten-second checks.
This supports an immediate authenticated return, not indefinite session reuse.
A rejected/expired return must end at sign-in with a clear notice. Do not retain
the account password, invent a new key, or automatically replay authentication.

### Bounded implementation contract

1. Keep one private account worker/runtime and its authenticated session through
   repeated `Characters -> EnteringZone -> Playing -> Returning -> Characters`.
   Keep the controller hidden during Playing rather than destroying it. A fresh
   roster comes from the newly authenticated world connection and advances the
   account revision. The foreground still receives only nonsecret identity and
   live channels. A first slice may make Back after returning end the session at
   sign-in; it must not assume the old login-server socket survives a long play.
2. Give camping an explicit request token and source movement/action context.
   Reject stale/double requests and camp during death, zoning, casting or pending
   inventory/services. Keep state/event processing alive while the countdown
   runs; block gameplay that conflicts with sitting, and offer Cancel until the
   worker commits Logout. Retire the timer on recovery/zone boundaries. Final
   logout sends no further target, gameplay or position packets. No stale
   callback may revive a camp attempt or a subsequent character's pending work.
3. Await bounded peer close after Logout. A timeout, malformed transport or
   OutOfSession is a failed return with sign-in/exit recovery. A matching peer
   close while counting is the explicit uncertain disconnect-recovery path
   described above, with exactly one fresh world reconnect and no success claim.
   Existing shutdown/committed-entry cleanup
   remains available; dropping a process is not the ordinary camp operation.
4. Returning invalidates old zone loaders, preview jobs, hit targets and held
   input. Save the old character's layout before clearing live state, entities,
   doors, map/travel/collision/liquid/atmosphere state and gameplay interaction.
   Reuse only account-independent client asset caches. Destination currently
   keys by zone name and per-LiveWorld generation, which restarts at0: clear it
   or add session identity, or a second character in the same zone can inherit
   the first character's scene/arrival. Stop zone audio on leaving play.
5. Preserve direct `--connect` and offline startup. A path without a retained
   account controller must explicitly say camp-to-roster is unavailable; it
   must neither create a second login nor claim that a roster will follow.
   Keep `/quit` as application exit. Update `/camp` chat/hotbutton parsing and
   tests together so a saved Camp button no longer takes the Quit path.

Required proof before a live fixture:

- Transport tests distinguish valid peer disconnect, truncated/wrong-session
  disconnect, OutOfSession, CRC/encoding errors, silence, unacknowledged sends
  and malformed fragmentation. A timeout must wake a waiting receiver.
- Pure camp tests cover countdown from dispatch, double/stale requests,
  cancellation immediately before expiry, cancellation after Logout commits,
  authoritative stand/own damage/death/bind/travel, old timer callbacks, GM early
  close, and no movement/gameplay sends during final logout.
- A loopback fake server records sit/Camp/stand and sit/Camp/Logout order, emits
  peer close or stays silent, verifies `SendLoginInfo.zoning=0` and a fresh
  roster, then permits a second selected-character entry on the same runtime.
  Cancel/drop at each boundary must neither publish stale Ready/roster nor
  leave a committed entry without the existing bounded cleanup.
- UI checks cover camp progress/cancel, held Escape/Enter across return, late
  pointer releases, re-entry into the same zone with a different character,
  expired-session sign-in recovery, and direct-connect unavailability. Original
  artwork captures require no login and no audio playback.
- Later live proof needs one coordinated disposable account/character, initially
  offline and alive, with no combat/cast/trade/loot/bank/merchant operation in
  progress, plus a private pre-login snapshot. Observe actual timed camp, fresh
  roster, re-entry and ordinary logout; verify offline state and restore/check
  pose/resources, items/cash, spells/buffs, binds, progression and corpses. No
  creation, other account or server-rule changes are needed for this slice.

## Character creation audit and safe next boundary

Creation should follow camp. Its packet data is known, but it changes durable
server state **at name approval**, earlier than its name suggests. Begin with
bounded catalog/capability decoding and a local draft/preview; do not expose a
standalone “Check name” network button.

| Exchange | RoF2 payload and source |
| --- | --- |
| ExpansionInfo `0x590d` | Exactly68 bytes; expansion bit mask at64 after64 unknown bytes (`rof2_structs.h:4699`, `rof2.cpp::ENCODE(OP_ExpansionInfo)`). The common server struct is only4 bytes and must not be used as the wire layout. `SendExpansionInfo` uses CharacterSelectExpansionSettings, client-based settings, or World.ExpansionSettings in that order. |
| SendMaxCharacters `0x5475` |12 bytes: u32 maximum then two unknown u32s. Server limits by client/common creation limit; RoF2 is12. Roster count is not a capacity advertisement. |
| SendMembership `0x7acc` |116 bytes: u32 membership, race mask, class mask, entry count=25, then25 signed u32-sized settings. `rof2.cpp` expands the common21 settings; its header's terminal offset comment is stale. Pinned server advertises gold/all races/classes. Do not infer eligibility from that hardcoded deployment behavior on another server. MembershipDetails=`0x057b` has separate settings and purchase mappings; never open purchase URLs. |
| CharacterCreateRequest `0x6773` | Empty request is accepted (handler does not read payload). Reply: u8=0, u32 allocation count, N×60-byte allocations, u32 combination count, M×24-byte combinations. `world/sof_char_create_data.h` packs an allocation as index +7 base stats +7 default increments, all u32; a combination is expansion requirement, race, class, deity, allocation index, start zone, all u32. Bound counts/checked arithmetic before allocating; require exact length, known referenced allocation and unambiguous IDs. |
| ApproveName `0x56a2` |72-byte request: NUL-terminated name[64], race u32 at64, class u32 at68 (`common/eq_packet_structs.h::NameApproval_Struct`, passthrough RoF2). Reply is exactly one byte0/1 in this server. The similarly named old `NameApproval` structure in `rof2_structs.h` is unrelated. Preserve unknown reply values as rejection, not success. |
| CharacterCreate `0x6bbf` |96 bytes,24 little-endian u32s: gender0, race4, class8, deity12, start zone16; hair color20, beard24, beard color28, hairstyle32, face36, eye1/eye2 at40/44; Drakkin heritage/tattoo/details48/52/56; STR60, STA64, AGI68, DEX72, WIS76, INT80, CHA84; tutorial88; unknown92 zero. `rof2.cpp::DECODE(OP_CharacterCreate)` copies all except unknown92. The DEX offset comment73 is a typo; packed u32 layout gives72. There is **no name field**: the server uses this connection's last approved `char_name`. |
| Creation outcome | `HandleCharacterCreatePacket` sends a fresh `SendCharInfo` on success; on failure it deletes the reserved name and sends `ApproveName(0)`. A transport send or approved name alone is not successful creation. Match the fresh roster to the immutable submitted name and expected identity before offering Play. |
| RandomNameGenerator `0x5954` |72 bytes: race u32, gender u32, name[64]. Server replaces the name and echoes the packet; generation is not reservation and does not establish availability for a later request. Optional after the basic creation flow. |

`WorldClient::characters()` now retains capability packets while awaiting
SendCharInfo in a bounded `CharacterSelection` snapshot. The snapshot also
retains a strictly parsed creation catalog requested by the account selection
pump. Missing or malformed capability data never means all-enabled.
`world/worlddb.cpp::LoadCharacterCreateAllocations/Combos` obtains the creation
catalog from the database; no fixed race/class/deity/start-zone table belongs in
the client. `ExpansionRequired` is an expansion bit mask (e.g. historical
`utils/sql/svn/2024_required_update.sql` uses2048 for Crescent Reach/Drakkin,
matching `common/emu_versions.h::bitTSS=0x800`); require all requested bits from
ExpansionInfo. Membership race masks need the explicit player-race mapping
(`common/races.cpp::GetPlayerRaceBit`), not `1 << (race_id-1)` for128/130/330/522.

The stat arrays use **STR, DEX, AGI, STA, INT, WIS, CHA**, whereas the outgoing
96-byte request uses STR, STA, AGI, DEX, WIS, INT, CHA. `CheckCharCreateInfoSoF`
requires the exact advertised race/class/deity/start-zone combination, resolves
its allocation index, bounds every stat between base and base+sum(default
increments), and rejects total increments above that sum. It permits unspent
points. Start with the server's default allocation, validate sums with checked
arithmetic, and label individual stats rather than copying array order. The
validator does not enforce expansion/membership or cosmetic ranges. Server
start-zone/tutorial rules can change the actual result; first creation should
send tutorial=0 and use the returned roster/zone, without predicting a final
spawn or inventing tutorial eligibility.

Appearance limits require separate evidence. `common/races.cpp::RaceAppearance`
contains race/gender/model-specific helpers, but **OPCharCreate does not call
them**; it narrows/copies appearance fields into the profile. Helpers accept
255/u32::MAX sentinels used elsewhere, which are not normal selectable choices.
The local renderer's normalization is a fallback, not a creation validator.
Examples that preclude a universal range:

- Faces are0–7 for most playable races,0–9 for Froglok,0–6 for Drakkin in the
  helpers. Barbarian composite face/woad selection is separate; current Luclin
  rendering uses decimal face/10 and face%10. Do not flatten that into a guessed
  universal face slider.
- Luclin hair is0–3 for supported ordinary race/gender pairs, Erudite male0–5
  and female0–8, Drakkin male0–8 and female0–7. Beard options vary from Dwarf
  female0–1 through Drakkin male0–11; unsupported parts must be hidden.
- Hair/beard palettes vary by race (Dark Elf13–18, Gnome0–24, Drakkin0–3, etc.).
  Existing Luclin preview does not reproduce these palettes; do not offer a
  color picker whose preview silently normalizes the selected value away.
- Drakkin helpers allow heritage0–7, but the installed
  `Resources/playercustomization.txt` has authored parents0–5 and per-parent
  class lists, both genders, four colors, seven faces, twelve eyes, eight
  tattoos and eight facial attachments. Intersect supported assets, that
  metadata and server-advertised combinations. The same2016 file's Human
  entries describe newer feature counts while most older race rows are zero
  placeholders; it cannot replace a verified RoF2/classic/Luclin capability
  table. Unsupported choices need to remain unavailable, not invented defaults.

### Creation commit and cancellation requirements

`HandleNameApprovalPacket` validates a4–15-letter name, uppercase first letter,
no later uppercase, and no spaces; `Database::CheckNameFilter` also rejects
nonletters, more than two identical consecutive letters and configured banned
substrings. It checks playable race/class and then calls `ReserveName`.
`ReserveName` rejects existing character/bot/NPC/pet names and **inserts a real
`character_data` row with level0**, optionally also adding default guild
membership. No name-reservation rollback is present in the world client
destructor. A subsequent name approval can overwrite this connection's selected
`char_name`; the client must never have two approval/create attempts in flight.

The final Create action therefore freezes a complete locally validated draft
and locks its revision before sending ApproveName. On approval, immediately send
exactly that draft's CharacterCreate on the same authenticated world socket,
then await the fresh roster/failure. Cancel before approval sends nothing;
navigation or app cancellation after approval has potentially reserved durable
state, so finish the bounded committed transaction without publishing stale UI.
An uncertain network failure must not retry approval/create or issue an
automatic DeleteCharacter. Reconnect to inspect the roster and report the
uncertain result; resolving a leftover reserved row requires a separate,
explicitly scoped action. The normal DeleteCharacter opcode is not a safe
generic rollback: it deletes an account-owned character by name, not a token-
identified reservation, and can soft-delete all of its associated state.

Creation tests should first cover every packet prefix/trailer, count overflow,
duplicate IDs, missing allocations, capability absence, nonmatching masks,
wire/stat-order mapping, checked stat sums, immutable approval/create pairing,
single-flight behavior, rejection/unknown reply, socket failure after approval,
cancel before/after commit, stale roster and changed session. A fake server can
prove that no retry or deletion follows ambiguous success. Live creation and
deletion need their own disposable-account fixture and explicit test scope;
camp proof requires neither.

### Creation transport implementation status

The strict RoF2 codec and immutable transaction state are implemented in
`openeq-net/src/creation.rs` and `openeq/src/account_creation.rs`. Named stat
fields preserve the different catalog/create orders. Validation checks the
advertised exact combination, referenced allocation, checked point budget,
expansion and membership masks, capacity, source-backed appearance limits and
the current successful preview receipt before claiming name approval. Classic
and Luclin palettes remain unavailable as controls; Drakkin choices require
authored heritage/gender/class metadata. No renderer normalization is accepted
as creation validation.

`WorldClient` retains socket-identified capabilities/catalog/roster snapshots;
roster revisions remain fresh across new connections. Catalog requests are
read-only and sent once per selection connection. Capability/catalog changes
invalidate older drafts and malformed replacements remove old permissive
state. Existing characters remain playable when creation data is unavailable.

Both initial login and camp return use `account/selection.rs`. Its single
receive pump handles UI commands, socket packets and an absolute transaction
deadline. One immutable draft owns name approval and the exact create payload
on the same socket. Foreground and worker checks reject double submissions;
Play is disabled while creation is pending. Cancelled panels detach, while
Back/sign-out/closed UI finish an already claimed transaction before releasing
the socket. Outer account cancellation retains that future for its bounded
completion. Unexpected/malformed replies, timeout or connection loss produce an
uncertain result; no retry, auto-delete or replacement transaction is sent.
Success requires a newer roster matching the submitted identity. Actual zone
and enabled state come from that roster, including disabled new characters.

Local UDP fake-server tests exercise the production pump, including retained
pre-roster capabilities, stale context, foreground and worker duplicate
submissions, approval-to-frozen-create pairing, capability refresh before the
success roster, detached completion, stale callbacks, Back before submission,
rejection, malformed/unknown/duplicate approvals and socket failure. The
production twenty-second deadline is verified while unrelated packets keep
arriving. The test peer asserts that uncertain outcomes cause no retries or
other application mutations. Pure codec/transaction tests also cover bounded
parsing, stat/capability validation and appearance intersection.

This transport checkpoint exposes internal account view/actions only. No
creation button is activated yet: draft controls and a receipt from the actual
loaded preview model family still need integration. No name reservation or
character creation has been attempted live.

The read-only `creation_catalog_smoke` passed against storage2 using only the
dedicated Barterer account. The production controller retained all advertised
capabilities and parsed641 combinations, with340 permitted for that account,
capacity12, expansion mask522239 and membership tier2. Every advertised default
stat allocation validated. The probe cancelled without selecting a character,
approving a name or sending a creation packet. Private before/after snapshots
confirmed all29 character tables exactly unchanged, including pose/resources
and bookkeeping. Evidence is under `/tmp/openeq-training-proof/creation-catalog.log`
and the adjacent private snapshot files; no credentials are recorded here.


### Camp implementation and verification status

Implemented the retained account worker/controller, explicit countdown tokens,
validated current-pose stop before sitting, empty Camp, standing cancellation,
authoritative damage/death/standing/bind/travel interruption, bounded final
Logout confirmation, distinct early-close recovery, fresh nonzoning roster,
and same-runtime reentry. The foreground retires character scene/load keys,
actors, doors, map/travel/collision/liquids/atmosphere, old interaction/hits and
layout ownership before the next character. Zone audio receives no destination;
held world keys cannot trigger account controls before release. A release-aware
raw-key ledger preserves that ownership across cleared Bevy input frames.
Immutable text/spell catalogs survive reentry, while spell simulation starts
fresh and shares only its decoded assets. `/quit` still exits, while direct
sessions explain the unavailable roster return.

Local fake-server tests cover ready-only Camp, exact sitting/Camp/standing/
Logout order, matching intentional close, silence, wrong connection code,
OutOfSession, and early close that cannot retroactively become Logout success.
Pure tests cover valid motion stopping without invented coordinates, rejected
stale/double camp, stale timer callbacks, blocked gameplay and interruptions,
retained controller cancellation handles, fresh roster revisions, old callback
rejection, and input/scene retirement. The production input selector is tested
with ready authority, queued Cancel and expired deadline simultaneously: it
chooses them in that order. A camp-specific interruption revision rejects starts
queued before damage/standing. Both worker and foreground use the living
MovementAuthority ID; corpse events cannot desynchronize later camp requests.

The root task's first guarded live proofs succeeded: Reviver cancelled once,
then returned after30,230ms with `uncertain_close=false` and reentered; Barterer
returned after235ms with `uncertain_close=true` and reentered. Both requested
graceful shutdown and root reported exact restoration/offline verification.
Private logs are `/tmp/openeq-camp-reviver-proof/live.log` and
`/tmp/openeq-camp-barterer-proof/live.log`. Those runs preceded the final queue
ordering/stamp and held-input fixes. The root task's final rebuilt Reviver rerun
passed after fresh source/rules/fixture revalidation: cancel, full30,257ms camp,
fresh roster, same-character reentry and graceful shutdown. The final private
log is `/tmp/openeq-camp-reviver-proof/live-final.log`; all29-table invariants
and baseline pose/resources were restored with Reviver offline. No live login
or database change was performed by xml_ui.

Original-art countdown and leaving captures passed at1×/2× and were visually
inspected in `/tmp/openeq-camp-ui`; Cancel becomes disabled during final Logout.
All-target application/network clippy passes with warnings denied.

`camp_smoke PRIVATE_CONFIG EXPECTED_CHARACTER [--early-close]` drives the
production AccountController, ordinary cancellation/countdown or GM early-close
recovery, fresh roster and same-character reentry, then requests graceful
shutdown. It does not snapshot or write the database; the operator must perform
preflight, offline verification and guarded restoration. It never logs private
configuration contents, raw packets or session keys and opens no audio device.
