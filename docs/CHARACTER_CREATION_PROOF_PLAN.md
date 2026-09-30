# Dedicated character creation proof

Prepared and executed September 30, 2026 for the user-authorized client parity
work. The bounded live creation and fresh-roster proof below passed. This does
not claim first zone entry or a complete race/model-family creation matrix.

## Dedicated account and expected footprint

Use a new ordinary account named `openeq_create1` and a single dedicated
character named `Trailborn`. Before provisioning or submission, verify those
identities do not already exist in `login_accounts`, `account` or
`character_data`. A read-only availability check is not an approval packet.
Generate a fresh credential into a private, exclusively created local file;
invoke the source-audited loginserver account CLI without displaying its
arguments or output. Do not assign administrator status.

The new account and successful character will remain as test fixtures. Do not
delete or replace any existing account or character. Explorer and all existing
dedicated characters are outside this creation operation.

Pinned EQEmu source is `4aceae18b94ffaafc08e2b17bc41cd72c77f795d`. Relevant
paths are `Client::HandleApproveNamePacket`, `HandleCharacterCreatePacket`,
`OPCharCreate`, `StoreCharacter`, `Database::ReserveName` and
`Database::SaveCharacterCreate`. Approval reserves a durable character row.
Successful creation populates character data, five binds, starting skills and
languages, optional granted leadership ability and starting inventory. World
authentication may create the ordinary account row and login bookkeeping.

The deployed rules were read without modification: SoF start zone202, tutorial
disabled, starting bind equals start zone, starting swimming100 and sense
heading200. Revalidate these before the proof. Use an advertised permitted human
warrior combination and its source default stats; do not infer a requested
origin is the resulting zone when the server overrides it.

## Client sequence

1. Authenticate through the production account controller and receive the empty
   roster, catalog and capabilities on the retained world connection. Save
   private nonsecret evidence and verify there is no character reservation.
2. Exercise browsing and local cancellation without submitting. No approval,
   character-create, character-entry or gameplay request is allowed here.
3. Build one immutable draft from the permitted combination, actual original
   model-family policy and a matching successfully loaded preview receipt.
   Use the editor's explicit final Create action. Replaying the same local
   action must not dispatch a second transaction.
4. Require the production worker's same-socket approval/create pair and a newer
   roster matching the chosen name, race, class and gender. Respect the received
   enabled flag and actual zone. Compare the exact submitted stat and appearance
   fields with the new character's persisted rows; inspect starting inventory,
   binds, skills and languages separately.
5. Cancel/sign out, reconnect without another creation request, and require the
   same single character from a fresh roster. Do not enter a zone during this
   initial creation proof. Any later entry/camp proof starts from a separate
   private snapshot of this newly created fixture.

No source-asset audio, movement, combat, commerce or other-character actions are
part of the test. Credentials, password hashes, session keys, raw packets and
database snapshots stay in private files outside the repository.

## Interruption and uncertainty

After approval dispatch, UI cancellation detaches while the worker finishes its
bounded transaction. If transport fails or a receipt is ambiguous, stop further
creation and inspect the dedicated account/name read-only. Do not retry the
name, guess success, delete a reservation or repair rows automatically. Preserve
the private journal and report the actual outcome. A rejected or interrupted
case must not be described as a successful complete lifecycle.

## Verification status

- Source audit and localhost transaction tests passed in the prior checkpoint.
- Read-only production catalog proof on the existing trade fixture passed and
  left all29 tracked character tables exactly unchanged.
- New-account/name availability checks passed. The dedicated login account has
  been provisioned through the audited CLI with a private generated credential;
  read-only sign-in created ordinary world account14 (login account12/status0).
  This initial provision did not create/reserve a character or enter a zone.
  Private provision evidence is under `/tmp/openeq-creation-proof`.
- The first operator-gated attempt lost its idle character-selection connection
  before the authorization marker existed. No approval/create request was sent.
  Read-only verification found no reservation/character and empty tracked
  character tables. That attempt's evidence is preserved separately.

## Completed bounded live proof

Fresh evidence is under `/tmp/openeq-creation-proof-v2`. The production
`creation_smoke` adapter used the actual Editor, AccountController and GPU
Preview, never raw approval packets or manufactured preview receipts.

- The same empty-roster world connection survived120,020ms of idle before
  name approval. Only the source-backed transport keepalives maintain it;
  no application polling or character request is used to extend the session.
- Read-only preflight confirmed ordinary account14/status0, no reservation and
  no tracked character rows. The intended draft was human warrior/male,
  deity201, requested origin45, face1 and source default attributes
  STR85/STA110/AGI80/DEX75/WIS75/INT75/CHA75. Actual Classic face1 textures were
  independently resolved and decoded; the full-avatar capture was inspected.
- One explicit Create passed. Replaying the same local action was rejected.
  The newer roster reported **Trailborn**, level1/race1/class1/gender0/face1,
  enabled, zone202 as required by the server's start-zone override.
- Read-only SQL matched every submitted stat/appearance field, ordinary/offline
  identity, five binds,100 skill rows,28 language rows and five practices.
  Six starting item IDs/charges matched the source-filtered database templates:
  9979×1,9990×20,9991×20,9998×1,21779×20,32601×1. Primary/offhand slots matched.
  No unexpected progression, currency, quest or social tables were populated.
- Sign-out and a fresh authenticated world connection returned the same single
  character with no inherited creation operation. All **30 tracked character
  tables were exactly unchanged** between post-create and post-reconnect
  snapshots. The initial helper's textual label said31; its dictionary actually
  contains30, and the fresh-attempt helper/report uses the corrected count.
- Trailborn remains an ordinary, offline fixture. There was no zone entry,
  gameplay action, deletion, restoration, other-character action or audio.

The first failed pause also exposed missing client idle keepalives. Independent
transport tests verify exact EQEmu framing and nine-second idle cadence, raw
ping acceptance, CRC/trailing-body rejection,180s synthetic idle and real local
UDP reception. Sending keepalives does not extend dead-peer receive deadlines;
receiving them does not extend reliable unacknowledged-message deadlines.

Independent appearance review separately reproduced a missing Luclin hair
archive silently rendering bald while allowing a receipt. That broader case
does not apply to the verified Classic face1 fixture. Loaders now retain whether
the requested parts/materials actually resolved, and creation requires both that
signal and decoded diffuse textures. Ordinary rendering retains its graceful
fallback. A provisional invalid draft can still receive the actual family's
policy/defaults without a receipt, so missing resources cannot strand policy
normalization or authorize an unrendered choice.

Independent CPU review checked2,299 original-asset appearance cases:2,210
resolved and89 were withheld. Every withheld case was compared with its default
and had identical geometry/materials under the current renderer. This verifies
the refusal of unrendered choices; it is not a complete cosmetic cross-product,
GPU comparison of all2,299 cases or a live race/class creation matrix. Targeted
omission tests cover Luclin hair/beard, Classic/Luclin face parts, eye textures
and Drakkin modules while accepting authored shared pieces and bald aliases.
