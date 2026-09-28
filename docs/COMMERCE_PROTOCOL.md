# RoF2 merchants and banking

Implementation and live checks use EQEmu commit
`4aceae18b94ffaafc08e2b17bc41cd72c77f795d`. References are
`common/patches/rof2.cpp`, `common/patches/rof2_structs.h`,
`common/eq_packet_structs.h`, `zone/client_packet.cpp`, and `zone/client.cpp`.
See [GAMEPLAY_PROTOCOL.md](GAMEPLAY_PROTOCOL.md) for shared item serialization,
inventory addresses, movement and connection behavior.

## Commands and events

Send `gameplay::Command` through `ZoneClient::command()`. Submission alone does
not prove a transaction succeeded.

| Command | Wire opcode | Payload bytes | Result |
| --- | --- | --- | --- |
| `MerchantOpen` | `0x4fed` | 24 | `MerchantOpened` and item catalog |
| `MerchantBuy` | `0x0ddd` | 32 | Actual item delivery and `MerchantBought` |
| `MerchantSell` | `0x791b` | 20 | `MerchantSold` and carried `Currency` |
| `MerchantClose` | `0x30a8` | 0 | `MerchantClosed`, opcode `0x3196` |
| `MoveCoin` | `0x0bcf` | 20 | No success echo |
| `BankerChange` | `0x791e` | 4 | `BankerBalances`, 32 bytes |

Merchant stock deletion is `MerchantItemRemoved`, opcode `0x724f`, containing
merchant ID, player ID and merchant slot. A merchant-open result includes the
server command, price rate and supported tabs. The client must honor the result
rather than opening the catalog merely because its request was sent.

## Merchant catalog and transactions

Catalog entries use ordinary serialized item packets with **packet type `0x64`**
and inventory kind 9. `item.slot.slot` is the merchant address used for buying.
It is unrelated to the item's ID or its eventual possessions slot.
`InventoryItem.price` is the server's per-unit purchase quote in copper;
`merchant_count == -1` means unlimited stock. Nonnegative values describe limited
stock. Updated listings and deletion messages update the catalog.

`MerchantBuy.price` is the quoted total. The server recalculates the cost and can
reduce the requested quantity. `MerchantBought.quantity` and `.price` are the
actual quantity and total charged. The confusing upstream names
`Merchant_Sell_Struct` and `Merchant_Purchase_Struct` refer to the merchant's
perspective: the 32-byte structure buys from the merchant; the 20-byte structure
sells to it. A sale uses the RoF2 typeless inventory slot, including its bag index.

The successful packet flows are asymmetric:

- Buying delivers an item or stack update, then acknowledges the purchase.
  `TakeMoneyFromPP` uses `update_client=false`; there is **no currency update**.
  Apply actual item packets and debit the matching acknowledgment's price once.
- Selling acknowledges the sale and sends a carried currency update. The server
  removes the sold units using `client_update=false`; there is **no item deletion
  packet**. Remove units once on the matching sale acknowledgment and take the
  currency balance from the separate server update.

Buying must preserve the server's coin denominations. `TakeMoneyFromPP` spends
copper, then silver, then gold, then platinum, breaking only the lowest higher
denomination required. It does not normalize untouched coin buckets. For example,
spending 10 copper from `99pp 8gp 18sp 12cp` produces `99pp 8gp 18sp 2cp`.
Canonicalizing the total would produce different coin buckets and would cause
subsequent denomination-specific bank transfers to disagree with the server.

A script-vetoed sale returns an invalid main slot (`0xffff`) or zero quantity;
`MerchantSold.rejected` preserves this condition. Do not remove the item or award
money. Certain purchase failures, including lore restrictions, insufficient
money and unavailable inventory space, have no purchase acknowledgment. The
upstream no-space path can run after the money deduction. Keep an unresolved
transaction distinguishable from success and avoid blindly retrying it.

An outstanding transaction must remain identifiable after its window closes so
late acknowledgments cannot apply to a different purchase or sale. Use the
merchant, player/slot, requested quantity and current pending action to reconcile
results. Reconnection supplies fresh authoritative inventory and balances.

Close acknowledgments contain no merchant ID. The client blocks reopening
until an outstanding close returns, suppresses duplicate close requests, and
preserves a provisional new session across trailing old close messages. Only an
acknowledged session accepts catalog entries. This also handles an unsolicited
server close followed by the reply to an explicit close request.

`base_price` and the merchant's open rate support an estimated sale price;
`MerchantSold.price` is the actual payout. `no_drop` preserves the inverted EQ
item flag (`0` means no-drop), and `attuned` comes from the instance header.

Other serialized item packet types have distinct meanings: `0x00` is an item
inspection/link response, `0x65` a trade view, `0x66` a corpse item, `0x67` an
inventory delivery, `0x69` inventory, `0x6a` limbo/cursor, `0x6b` a world container,
and `0x6c` tribute. An inspection or merchant listing must never insert an item
into the player's inventory.

## Banking

There is no bank-open protocol handshake. Initial profile data carries
`bank_currency`, `cursor_currency` and `shared_platinum`. Bank and shared-bank
items arrive through the inventory stream and individual item deliveries; those
deliveries can follow the initial inventory packet.

`InventorySlot::bank(0..23)` uses kind 1 and canonical slots 2000–2023.
`InventorySlot::shared_bank(0..1)` uses kind 2 and canonical slots 2500–2501.
`in_bag(index)` addresses children; moving a container must preserve its children
at their corresponding destination addresses.

The client must require a nearby banker (NPC class 40) for every bank item or
coin operation. This server's `USE_NPC_RANGE2` is 40000: a 200 EQ-unit radius.
Merchant services use NPC class 41 and the same distance. Access must close when
the NPC disappears, the player leaves reach, zones or disconnects. The server can
disconnect a client for a distant bank item move because it is an inventory
desynchronization, not a harmless rejected click.

Successful bank item moves and coin moves have no success echo. Item moves use
the same prediction and correction behavior as possessions moves. `MoveCoin`
supports carried (1), personal bank (2) and shared bank (4); copper (0), silver (1),
gold (2) and platinum (3). Transfers preserve denomination. The encoder rejects
zero amounts, values above `i32::MAX`, identical locations, and non-platinum
shared-bank transfers. Cursor, trade and destruction money locations are omitted.

Shared platinum must be enabled by the server's `Character:SharedBankPlat` rule.
It is enabled and live-tested on Storage2. Do not infer this capability merely
from a server accepting a move packet: servers with the rule disabled can deduct
deposited money without retaining it. No-drop or attuned items, including such
items inside containers, also require client validation before shared deposit.

`BankerChange` consolidates denominations in the carried and personal-bank
balances and returns both balances, in that order. It is an actual change-money
operation, not a passive bank-open or refresh request. Its reply does not contain
shared platinum; the profile is the source for that balance after reconnect.

## Dedicated live proof

The guarded `eqcommerce` probe uses account `openeq_commerce`, character **Broker**,
in Plane of Knowledge. It refuses another host/account/character. Fixture
provisioning followed backup run `20260928T155309Z` in `/srv/eqemu/backups` and did
not modify world content or the Explorer and Arcanist fixtures. The server seeder
is `/home/daeken/eqemu-bootstrap/commerce-seed.py`.

Private configuration has mode 0600 and is never committed:

```sh
cargo run -p openeq-net --bin eqcommerce -- \
  --config "$HOME/.config/openeq/storage2-commerce-credentials.json"
```

Verified against Storage2 on 2026-09-28:

- Amile Pitt opened a 42-item catalog. Iron Ration, item 13005 at merchant slot 5,
  had an authoritative 151-copper quote and unlimited stock.
- Buying two delivered two rations to possessions slot 26 and acknowledged
  302 copper. One delivery and zero currency updates arrived.
- Selling both acknowledged 294 copper and sent the actual carried balance.
  Zero deletion packets and one currency update arrived.
- After closing the merchant, Dogle Pitt provided banker proximity. A cloth cap
  moved to personal-bank slot 0; 20 waters moved to shared-bank slot 0. One
  platinum moved to each bank. Reconnection confirmed both items, both balances,
  and the absence of the sold rations.
- Both items and both coins returned to their original locations. A second
  reconnect proved restoration. Independent database reads confirmed the four
  original items, empty banks and zero shared platinum. The complete roundtrip
  costs eight copper in the merchant spread; final carried funds were
  `99pp 9gp 9sp 2cp` after this run. Broker was logged out afterward.

Network tests cover exact layouts and every truncated prefix of merchant and
bank balance messages, invalid rates and quantities, veto sentinels, typeless bag
slots, coin restrictions, bank address bounds, and merchant item header fields.
All 42 network library tests and all-target network Clippy passed at this
milestone. This probe verifies protocol behavior; client-window rendering and
interaction are validated separately by the integrated client tests.

### Integrated client proof

```sh
cargo run -p openeq --bin commerce_smoke -- \
  "$HOME/.config/openeq/storage2-commerce-credentials.json"
```

This additional guarded Broker probe drives `LiveWorld`, `Interaction`,
`UiAction::Commerce`, the gameplay reducers and the HUD presentation data. It
loads the actual PoK collision geometry, chooses a floor beside each service
NPC, and places the human fixture's center above that floor. It converts scene
coordinates back to server coordinates for the fixture's `#goto` command and
waits for the authoritative position correction. This avoids both large-motion
anti-warp assumptions and copying a smaller NPC's center height.

The probe
opens the merchant through the selected NPC, selects the actual ration quote,
buys two, selects the delivered inventory item and sells both. It checks pending
states, actual delivery/removal, and the reducer's purchase debit and sale balance.

Bank transfers use the same two inventory clicks as the UI: pick up onto the
cursor, then place into a personal/shared-bank slot. Coin transfers use the bank
buttons. The probe checks the rendered presentation data, rejects item movement
without an open banker, closes service access out of reach, and confirms both
deposits and restoration through fresh reconnects. It deliberately does not call
`BankerChange`, so exact nonnormalized denomination buckets are also tested.

The integrated run passed on 2026-09-28: two rations bought for 302 copper and
sold for 294; cap/water and both one-platinum deposits persisted, then their
restoration persisted. Final fresh profile had `99pp 8gp 18sp 4cp`, zero bank
money and zero shared platinum. Broker logged out with its four original items.
Native window rendering is a separate visual check; this executable requires no
window and does not claim GPU verification.

Native verification subsequently exercised bank and shared-bank item/coin
deposits and withdrawals, merchant purchase/sale of two rations, catalog
scrolling and tooltips. The items and bank balances were restored. With the
coordinate boundary correction, the banker window opens inside the actual PoK
bank building, the player remains on its floor and map landmarks align.

After correcting the server/asset coordinate boundary, the integrated roundtrip
passed again. The fresh profile and independent database read agreed on
`99pp 8gp 16sp 8cp`, empty banks and the restored four original items. Broker's
final scene center was `[-305, 934, -92.875]`, over the measured asset floor at
`-96`; the database stored server coordinates `[934, -305, -92.875]`. Thus the
walking client's feet start 0.125 units above the floor, rather than below it.
The fixture was logged out for subsequent native verification.
