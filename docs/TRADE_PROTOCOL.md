# RoF2 player trading

Audited against EQEmu `4aceae18b94ffaafc08e2b17bc41cd72c77f795d` and the Storage2 server. Protocol implementation is `crates/openeq-net/src/trade.rs`; `Command::Trade` and `GameplayEvent::Trade` expose it to the client. NPC quest hand-ins and bazaar trading are separate features.

## Session and acceptance

| Wire opcode | Message | Payload |
| --- | --- | --- |
| `77b5` | Request | `u32 recipient, u32 sender` |
| `14bf` | Acknowledge | `u32 recipient, u32 sender` |
| `5505` | Busy/decline | Same IDs plus `01 ef ff ff` |
| `69e2` | Accept | `u32 sender, u32 unused` |
| `354c` | Cancel | `u32 player, u32 action` |
| `4206` | Coins added | `u32 recipient, u8 denomination, 3 unused bytes, u32 amount` |
| `3993` | Finish trade | Empty |
| `7349`, `40ef` | Finish windows | Empty |

Request only forwards an invitation. Acknowledgment starts **both** server trade sessions and is forwarded only to the initiator: the acknowledging client gets no self-echo. Reject invalid/stale invitations and require a nearby player and an otherwise idle client before acknowledging. The upstream request/ack handlers do not enforce these UI constraints.

Accept is forwarded to the partner. Any offered item or coin addition resets **both** server acceptance states, without a separate reset packet; the client must do the same. A finished packet means the session ended, including lore/no-drop rejection. Item deliveries and final currency are authoritative; the finish packet alone must not say the exchange succeeded.

After local acceptance, freeze item and coin mutations until a remote offer changes or the session ends; cancellation remains available. The partner may already have completed the exchange on the server before its packets reach this client. Sending another coin offer during that gap can debit money outside any trade.

## Item escrow

Trade addresses are `InventorySlot::trade(0..7)`, wire kind3, canonical server3000..3007. The **only** valid source of an offered item is the cursor. Moving directly from general inventory into trade causes EQEmu to kick the client. Move/split the desired item onto the cursor first, then move the whole cursor item into an empty trade slot with count0.

An occupied destination cannot be replaced. Count greater than zero has special stack-onto-existing semantics and kicks for invalid combinations; the initial client should offer into empty slots only. Do not withdraw or modify trade items through ordinary inventory moves: the upstream path does not update the partner's offer or reset acceptance correctly. Cancel and start another trade to revise those offers. Check no-drop and attuned metadata recursively, including bag contents, before offering.

Successful offers have no ordinary move/delete self-echo. Track the sent cursor-to-escrow move once, preserving item ownership in a separate escrow state until the server ends the session. A queued next cursor item can independently arrive afterward.

The partner gets `GameplayEvent::Item { packet_type:0x65, ... }` (`ItemPacketTradeView`), with **kind0, slot0..7**, because EQEmu serializes the trade slot minus3000. These are display-only offers; they must never enter the receiving player's owned inventory. Bag items carry recursively serialized children; do not confuse extra child display packets with owned items.

Actual deliveries and refunds use packet type **0x67** (`ItemPacketTrade`), with real possession/bag destinations. Cursor deliveries use **0x6a** (`ItemPacketLimbo`). These constants are distinct from0x65. EQEmu chooses available slots and may merge stacks or return undeliverable items to the giver; do not invent a destination or blindly restore the original slot.

## Coins

`TradeCommand::OfferCoin` sends `OP_MoveCoin(0bcf)` with five little-endian dwords: carried location1, trade location3, source denomination, destination denomination, amount. Denominations are copper0, silver1, gold2, platinum3. Preserve the exact buckets; they can exceed9.

Require an acknowledged active session, sufficient carried money, and positive amount at most `i32::MAX` before sending. The server can clamp an excessive request and gives the sender no immediate currency echo. The sender tracks a validated local debit once; completion/refund currency replaces it with authoritative state.

EQEmu also limits each resulting coin bucket to signed32-bit range when crediting a refund or receipt. Bound cumulative offers and validate both possible refund and receipt balances before accepting; otherwise the server can silently omit an overflowing credit.

The partner receives `CoinsAdded`, a **delta**, whose ID unexpectedly identifies the recipient/local player, not the offering player. Match against the local ID and apply it to the active partner's offer. EQEmu prohibits moving coins back from trade: cancel to recover them. `CoinLocation` intentionally continues to exclude trade; the dedicated command cannot accidentally withdraw or bank escrowed money. `OP_TradeMoneyUpdate(68c2)` exists in the opcode table but the audited trade implementation never emits it.

## Cancel, disconnect, and zoning

Cancel is asymmetric. The caller is refunded and reset, and the server forwards `CancelTrade` to the partner with the ID rewritten to the partner's own ID. That partner must send **one** reciprocal cancel to recover its own escrow. Mark the local session as closing before sending; ignore the later reciprocal cancellation once closed. Without this guard, clients can bounce cancel packets or reset a new session.

Observed ordering for each canceling client: authoritative currency, item refund0x67, `WindowClosed`, `WindowClosed2`. The initiator may receive the peer's reciprocal cancel after those close packets. Treat the two window-close packets idempotently.

Withdrawing an outgoing invitation before ACK also keeps a cancellation barrier: wait for local `WindowClosed2` **and** the peer's Busy/Cancel response, or its departure. OpenEQ sends Busy to withdraw the peer's invitation; an untouched invitation replies Busy once. A late ACK requires a fresh cancellation and close response. Do not allow a new trade after the local close alone; a late ACK could otherwise reset the new server session. An unresponsive peer leaves the barrier in place until disconnection/reconnect.

An unexpected ACK during an existing trade freezes client mutations, invalidates the displayed currency, cancels the newly acknowledged server session, and requires reconnect. This limits further damage from another client's stale or duplicate ACK. EQEmu's `Trade::Start` has already reset its trade counters before sending that ACK; reconnect reveals authoritative ownership but cannot recover coins discarded by that server reset. Preventing arbitrary foreign ACK resets requires server-side request/session validation.

`Client::OnDisconnect` refunds both players and resets both trade objects, but does not send a trade-close packet to the surviving partner. On partner despawn, the client requests a final cancel/close without waiting for a reply from the departed peer. Local zoning/disconnect resets trade state; preserve authoritative refunded deliveries. Outgoing escrow must not become usable inventory merely because a transport operation failed; reconnect inventory is authoritative.

## Live proof and isolated fixtures

The fixtures were created after database backup `/srv/eqemu/backups` timestamp `20260928T171531Z`:

| Account | Character | Initial possessions | Initial money |
| --- | --- | --- | --- |
| `openeq_trade1` | Barterer | Slot23 Short Sword5001, slot24 Water Flask13006×20 | 100pp |
| `openeq_trade2` | Swapper | Slot23 Cloth Cap1001, slot24 Water Flask13006×20 | 100pp |

Both are in Plane of Knowledge, near the bank at server934/938,-305,-92.875. Configs and passwords remain outside the checkout with mode0600. Do not concurrently connect a native client and the probe to the same fixture.

```sh
cargo run -p openeq-net --bin eqtrade -- \
  "$HOME/.config/openeq/storage2-trade1-credentials.json" \
  "$HOME/.config/openeq/storage2-trade2-credentials.json"
```

The probe is restricted to these two accounts/characters on Storage2. It verifies request/ack/decline, item and coin views, two-sided cancellation and its persistence, acceptance reset when an accepted offer changes, exchange and reconnect persistence, reverse exchange restoring the original items and money, and disconnect refunds with reconnect persistence. Its diagnostics include actual item packet types and destination slots. It never prints credentials.

The integrated proof exercises `LiveWorld`, `Interaction`, `/trade`, inventory clicks, and the same `TradeAction` buttons used by the windowed client:

```sh
cargo run -p openeq --bin trade_smoke -- \
  "$HOME/.config/openeq/storage2-trade1-credentials.json" \
  "$HOME/.config/openeq/storage2-trade2-credentials.json"
```

It additionally checks that coins change only after a sent callback, repeated clicks while pending cannot double-debit money, cancellation retains escrow while awaiting both close and reciprocal cancel, a new invitation cannot bypass that barrier, and remote item views never overwrite worn slot0. It withdraws an invitation before ACK and immediately opens a fresh trade, and verifies accepted players cannot add coins, move or destroy items. It runs cancellation, changed-offer acceptance, actual exchange, reverse exchange, and partner disconnect through the client reducers, reconnecting after each transaction to compare exact possessions and currency.

Client inventory validation permits only a whole cursor item into an empty top-level trade slot. It rejects partial direct offers, occupied destinations, trade child operations, withdrawal/destruction, and recursively no-drop or attuned contents. `Inventory::clear_trade()` removes only the separate escrow tree, preserving server-delivered refund slots and their bag children.

Tests exercise exact packet sizes and IDs, every truncation of recognized packets, trailing bytes, invalid coin denominations, and safe amount bounds. Inventory tests verify packed item click/scroll/recast metadata and trade-slot canonical addressing.
