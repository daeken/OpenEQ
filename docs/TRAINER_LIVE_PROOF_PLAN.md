# Dedicated trainer purchase proof

Prepared 2026-09-29 under the user's authorization to implement and verify
progression. This is a bounded fixture plan; no purchase is claimed until the
verification record is appended. Explorer is excluded.

## Baseline and mutation budget

Use only Barterer, character 8/account 10 (`openeq_trade1`), human warrior level
10, GM flag 1. The private 29-table snapshot and guarded helper are under
`/tmp/openeq-training-proof`. Credentials remain outside the repository.

The current baseline is offline in Plane of Knowledge, practice points 0,
skill 0 (1H Blunt) 55 and carried money 100 platinum. No specialization skill
43–47 exceeds 50. The installed class-1/skill-0/level-10 cap is 75. No original
inventory, bind, spell, language, AA, quest, faction or social state is seeded.

Immediately before login, revalidate the deployed source/binary hashes, opcode
map, relevant class rule, skill cap and exact fixture snapshot. Under an
offline/full-row guard, seed only two practice points and a pose near the
existing class-20 trainer Warlord Welorf (NPC type 202251). Do not change NPCs,
rules, scripts, GM status, zone processes or other characters.

The production client should explicitly open that trainer, select skill 0 and
perform exactly two ordinary sequential purchases:

| Step | Received skill | Server-assessed copper cost | Expected carried copper | Session practices |
| --- | ---: | ---: | ---: | ---: |
| Baseline | 55 | — | 100,000 | 2 |
| First | 56 | 911 | 99,089 | 1 |
| Second | 57 | 973 | 98,116 | 0 |

Read-only SQL must agree after each purchase. Training points may remain
unsaved until normal character save; reconnect must establish the final zero
from a new profile and database state. Reject duplicate UI actions and stale
stamps locally without sending extra purchases. The probe must not move,
attack, cast, hand in items or play audio. Complete normal logout and reconnect
using the same dedicated account, then log out again.

## Restoration

After confirmed offline status, inspect the complete table snapshot. Permit
only the two specified skill increments, assessed carried-money totals,
practice-point states 0/1/2, ordinary resources/pose and login bookkeeping.
Inventory GUID regeneration is recorded separately, as in prior Barterer
proofs. Any other mutation fails restoration preflight and requires inspection.

Restore the baseline skill value, exact carried denominations, practice count,
pose and resources only under a predicate matching the observed fixture and
all related rows. Compare all 29 tables after restoration, preserving private
before/after journals. The helper's baseline operation is read-only; do not
seed until the production interaction path and probe are ready.

## Current verification

- Baseline snapshot/preflight passed with fixture offline; no seed or login yet.
- Source audit and standalone protocol/state tests are documented separately in
  `TRAINER_TRANSACTION_PLAN.md`. These do not prove deployed purchases.
