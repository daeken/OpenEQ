# Interactive account, server, and character selection

Read-only source investigation, 2026-09-29. No account/character records were
changed and no new login sessions were opened for this investigation.

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
| Character roster | `WorldClient::characters()` | Name, level, class, race, gender, last zone ID |
| Enter chosen character | `WorldClient::enter_world(name)` | Actual zone IP and port, or zone unavailable/error |
| Zone transport | `ZoneClient::connect(address, name)` and `enable_zoning(...)` | Existing zone/profile/spawn/Ready lifecycle |

`WorldClient::connect_zoning` is specifically for an existing character's zone
handoff. Returning to character selection should use the non-zoning connection
mode and a valid authenticated session instead.

The graphical executable accepts only `--connect CONFIG` for live play today.
`main.rs::prepare_world` waits for the client asset job, calls
`LiveWorld::start(config)`, and lets the server's environment name select the
actual zone assets. `LiveWorld::start` owns a background thread with a current-
thread Tokio runtime, calls the automatic `config.connect()`, then starts the
existing event/command/movement loop. Errors become the current loading error;
there is no interactive retry/selection state yet.

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

Back from a character roster can close that world socket and return to the
still-valid login/world list. If that login session has expired, return to the
credentials screen with a clear reason. Cancel drops owned streams and abandons
in-flight futures; a later stale result cannot enter a character. Initial scope
need not implement camping from a running character into the roster: that is a
separate logout/camp lifecycle that must await the existing zone logout path.

## Gaps to close before exposing arbitrary server/roster input

- `parse_server_list` allocates directly from an unbounded wire count, accepts
  nonterminated strings, ignores the reply success field, and substitutes
  loopback for an invalid address. Use bounded checked decoding, explicit errors,
  count/string limits, and reject invalid addresses; never redirect silently.
- `parse_characters` currently returns an empty or partial vector on truncation.
  A genuine zero-character roster must be distinguishable from malformed data.
  Return `Result`, bound/validate each name and tail, reject unsupported counts
  and trailing/truncated entries. The current RoF2 encoder caps at 12 characters;
  the Rust parser currently caps its loop at 32. Preserve a deliberate documented
  protocol bound rather than treating partial parsing as success.
- `Character` omits instance, Enabled, return-home/tutorial flags, appearance,
  equipment, and last-login fields that are already present in the tail. At
  minimum decode Enabled and instance for correct row/action state; keep richer
  appearance and special entry actions for later. No new server request is
  needed to obtain these existing fields.
- Login/world failures lose useful reason codes. `play()` only reads success
  and currently accepts a 15-byte minimum even though its complete reply is 20
  bytes. Validate complete reply shape and selected server ID; surface the
  server error string ID (unavailable, suspended/banned, full, already online)
  without exposing private payloads. Authentication errors similarly have
  `LoginBaseReplyMessage` reason data currently collapsed into `Rejected`.
- `login::Session` derives `Debug` while containing the transient key. Remove
  that derivation or redact it before adding worker/state diagnostics. Existing
  `ConnectionConfig` and the zone's authenticated handoff already avoid Debug.
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
- A future authorized dedicated-account smoke should pause on both selection
  screens for longer than 30 seconds, refresh, choose the advertised world and
  existing character, observe actual profile/Ready and correct zone, then exit
  cleanly. Include reject/retry/cancel and unavailable-zone outcomes. Reuse
  disposable accounts only with fixture coordination; this investigation did
  not perform such logins or mutate any accounts.

This first slice delivers credentials → server list → existing character list →
the current playable world. Character creation/deletion, entitlement emulation,
return-home/tutorial entry, 3D previews, and camping back to selection remain
separate, source-backed additions.
