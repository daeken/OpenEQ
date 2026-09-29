# Reviver same-total XP and same-level receive proof

Completed guarded validation, 2026-09-29. The integration/fixture owner reviewed
and authorized the recipe below before either login. The completed Fellowship
proof is recorded in
[PROGRESSION_PROTOCOL_PLAN.md](PROGRESSION_PROTOCOL_PLAN.md).

## Completed result

The proof passed, with Reviver independently verified offline and restored
after both the command session and the fresh-login check. Revalidation found
the documented source revision, deployed binary/script hashes, rules, opcode
values, account access, fixture identity and gameplay baseline unchanged.
Explorer was neither logged in nor changed.

The external probe used the actual `ConnectionConfig`/`ZoneClient` connection
and decoded typed events, then called the production `GameplayState::apply`
method used by the foreground client. It counted received events before that
call so identical duplicates remained observable without changing reducer
semantics. This bypassed LiveWorld's private mailbox adapter and did not render
the graphical UI; it is live decoder-to-foreground-state evidence, not a new
end-to-end mailbox or visual check. No temporary product tracing or repository
runtime changes were needed.

| Step | Actual typed receipt | Foreground result |
| --- | --- | --- |
| Initial login | `Experience { bar_units: 0 }` | Revision1→2, level1, bar0, fraction0.0. |
| `#set exp exp 0` | `Experience { bar_units: 0 }` | Revision stays2, level1/bar0; profile totalXP0/training0 unchanged. |
| `#set level 1` | `Level { level: 1, reported_old_level: 1, bar_units: 0 }` | Revision stays2; profile and foreground level remain1. |
| Fresh login, no commands | `Experience { bar_units: 0 }` | Restored profile reproduced; fresh revision1→2, level1/bar0. |

Each profile contained100 zero normal skills and32 languages, with only
language0=100. No SkillValue event arrived. The command session received two
Experience events and one Level event; the reconnect received one Experience
event and no Level event. The only gameplay commands sent were the two listed
above, each once, with no target. Both sessions explicitly logged out. No
movement, target selection, combat, trainer, travel or other-character command
was sent.

Private snapshots and comparisons covered the full character row and28 related
tables before login, after each command, and after each disconnect. All gameplay
invariants matched, including empty skills/inventory/buffs/mercenaries/factions/
tasks/social memberships, language0=100,14 veteran AA allocations, five binds,
two inert pet-info rows and the two existing XP-modifier rows (zones2 and77,
all modifiers0). Only ordinary login/time bookkeeping (`last_login`,
`time_played`, `mailkey`, `ingame`) was excluded from gameplay equality; it was
journaled and not rewritten.

The same-level command's save recorded HP36/endurance21, versus the inherited
HP100/endurance100 baseline. The reconnect's ordinary logout save produced the
same resource adjustment without a level command. Both are within the reviewed
resource footprint. Each offline restoration matched exactly one guarded
character row, changed that row once, and restored only the approved
pose/resources/hunger/thirst columns. The predicates included the freshly
observed resource values, all unchanged character gameplay fields and the exact
related-row invariants. No progression, skill, AA, faction or related table was
repaired or replaced.

Independent final SQL verified Reviver(id10/account12) offline, GM0/PvP0,
level1/level2=1, absoluteXP0, training0 and all AA bookkeeping0, Arena77/instance0
at(146,-1009,51), heading0, HP100/mana0/endurance100, hunger/thirst6000, language0
100, the same14 veteran AA rows, two inert pet rows, two XP-modifier rows and
zero skills/inventory/buffs/mercenaries/factions/corpses/group/guild/raid rows.

Artifacts are in `/tmp/openeq-reviver-proof-1/` (directory mode0700):
`baseline.json`, `preflight.json`, `observations.jsonl`, `commands.log`,
`reconnect.log` and `restore.sql` are mode0600. The standalone `probe.rs`,
`fixture.py` and `build.json` retain the bounded recipe and exact linked-library
provenance outside the repository. No credentials or raw packet buffers were
logged. Both probe runs exited0; all preflight, invariant and restoration
checks passed.

This establishes actual same-total XP and same-level packet receipt with stable
duplicate reduction. Earned XP, a real level gain/loss, trainer purchases and
ordinary skill gain chance remain outside this validation.

## Reviewed recipe

The remaining sections preserve the concrete recipe approved before execution.

## Exact fixture and permitted commands

Use only Reviver, character10/account12 `openeq_recovery`, with the existing
private recovery connection config. Read-only inspection found the character
offline, account status250, GM0, human warrior, deity396, level1/level2=1,
absolute XP0, training points0, all AA XP/point/spent/old bookkeeping0,
AA percentage/effects/expended bookkeeping0 and PvP0. Do not change GM status.
`command_settings` permits `set` at status50, so no privilege change is needed.

The allowed self commands, each sent once in sequence, are:

```text
#set exp exp 0
#set level 1
```

Both require no current target; require this before every poll and send. No
movement, attack, cast, target, item, trainer, bind, pet, mercenary, AA purchase,
travel or other-character command belongs in this proof. Explorer is excluded.
Do not turn a failed test into an actual level gain/loss or a fixture reset.

## Read-only source and deployment evidence

The deployed and inspected source revision is
`4aceae18b94ffaafc08e2b17bc41cd72c77f795d`. Arena77/version0 uses ruleset1
(`default`). Recheck these exact stored rules immediately before login:

| Rule | Required value | Why it matters |
| --- | --- | --- |
| `Expansion:AutoGrantAAExpansion` | -1 | `AutoGrantAAPoints` returns immediately, including during command SetLevel and login. |
| `Character:HealOnLevel` | false | SetLevel only clamps over-max HP; it still recalculates bonuses and sets mana to maximum. |
| `World:PVPMinLevel` | 0 | SetLevel cannot enable PvP through this rule. |
| `Bots:Enabled` | false | Neither SetLevel's wrapper nor Save can update owned bots. No bot tables are deployed. |
| `Bots:BotLevelsWithOwner` | false | Additional guard against the wrapper's bot leveling branch. |
| `Character:ActiveInvSnapshots` | false | Save cannot create an inventory snapshot. |
| `AA:ExpPerPoint` | 23976503 | The AA divisor is nonzero; preserved AA XP0 cannot grant an AA point. |
| `Character:MaxLevel`, `Character:MaxExpLevel` | 70,70 | Level1 is valid and below the cap. |
| `Expansion:CurrentExpansion` | 9 | The deployed global DoN login hooks are enabled and must be considered. |

`zone/exp.cpp:877–999` sends LevelUpdate `{level:1, level_old:1, exp:0}`
for command SetLevel. Training points are granted only when the new level is
greater than level2; level-up/down hooks run only when it differs from the
current profile level. Therefore neither branch should run at1/1. The command
still invokes AA auto-grants, bonuses, HP/mana adjustment, tribute update,
mercenary update and Save; it is not treated as a no-op.

`zone/gm_commands/set/set_level.cpp:28–57` then clamps all effective skills to
current caps. Reviver has no skill rows and no inventory, buffs or active
tribute. Its profile must contain only zero base skills before proceeding.
`zone/client.cpp:12730–12740` calculates item-modified skills from the base
value, so the all-zero/no-item baseline cannot exceed any nonnegative cap.
Do not create skill rows or restore unexpected clamp changes automatically.

`zone/exp.cpp:572–869` preserves AA XP when `#set exp exp 0` is called through
`zone/gm_commands/set/set_exp.cpp`. With current XP0/level1 and the default
curve, SetEXP stays at level1 and sends ExpUpdate0. Level1's default threshold
is0; this recipe does not fabricate a new XP total. Below51, SetEXP sets the
AA allocation percentage to0, already the baseline. Normal/AA gain hooks need
a changed stored total, so neither should fire.

The server was built with Lua and Perl enabled. `common/path_manager.cpp`
resolves Lua overrides to `/srv/eqemu/mods`; that directory is absent, so there
is no `load_order.txt` override for GetEXPForLevel, GetRequiredAAExperience,
SetEXP or SetAAEXP. Configured quest path is `/srv/eqemu/quests`. Arena has no
player or script-init file. The global initializer imports the ordinary helper
modules; its command dispatcher has no `set` override. Lua is registered before
Perl, and `GetQIByGlobalPlayerQuest` selects the first existing file, so the
deployed `global_player.lua` is the global player script.

The global connect hook requests14 veteran AA grants. Reviver already owns
exactly their first-rank allocations, all value1/charges0:
1371–1377,4665,4700,5006,9000,9031,9032,9033. `CanPurchaseAlternateAdvancementRank`
rejects an already purchased rank (`zone/aa.cpp:1643–1657`). The DoN faction
repair hook requires both factions to be at least indifferent. Reviver has no
faction rows/items/buffs; Dark Reign1021 and Norrath's Keepers1023 each have
base0 and deity396 modifier-100, with no applicable race/class modifier, so
neither satisfies the condition. Snapshot and compare faction rows anyway.
The global level-up hook grants starter skills, but same-level SetLevel does
not call it. There are no global XP-gain hooks in the inspected player script.

Recheck the hashes of relevant scripts rather than relying on this historic
inspection. SHA256 values at preparation time:

```text
global/global_player.lua  1e09300dcbf990308f5f860344634f3eecff560626840df6724161ea899556e8
global/script_init.lua   17b72fe55b9c7d0fd978fc3980ebc527a5a85a6629deeebc4536aebbad1ef42e
lua_modules/dragons_of_norrath.lua  08eb602757a3b8386bab874023f236166046d30602e747956832ba54bddfb68b
lua_modules/command.lua  fb7c357935c64aa4cba304e22a4e0fff17f66ac06f0c702c47bb738d3dcd0681
```

## Snapshot and restoration footprint

Before login, save a mode0600 journal with the complete character row and exact
rows from skills, languages, AA allocations, currency, inventory, binds, spells,
memorized spells, buffs, pet info/buffs/inventory, tribute, mercenaries, factions,
timers, tasks/task timers/enabled tasks, quest globals, corpses and social
membership. Preserve hashes of server binaries, scripts and deployed opcodes.
Do not journal passwords or raw network packets.

The current pose/resources baseline is Arena77/instance0 at(146,-1009,51),
heading0, HP100/mana0/endurance100, hunger/thirst6000. All five binds remain at
(160,-1009,51) in Arena77/instance0. Currency is all zero; language0 is100.
Inventory, skills, spells, memorized spells, buffs, mercenaries, tribute,
factions, timers, tasks, quest globals, corpses and group/raid/guild membership
are empty. Two existing pet-info rows (pet0 and1) have blank names and zero
spell/HP/mana/power/size/taunting; no pet buffs/inventory exist. Do not delete
those inert rows or invent a pet cleanup step.

`Client::Save` (`zone/client.cpp:981–1120`) saves resources/pose, currency,
binds, buffs, pet info, tribute, timers, tasks and XP modifiers, plus normal
login/time-played bookkeeping. `DoTributeUpdate` is not merely a notification:
it removes equipped tribute items when inactive. Empty inventory/tribute and
tribute_active0 avoid that footprint. `UpdateMercLevel` only affects a live
mercenary; require no owned mercenary rows and none in the received session.

Compare all gameplay invariants after each command and after disconnect.
Allow normal timestamps/time-played bookkeeping in the journal without
rewriting it. Resources may be clamped or regenerate even with no movement.
Only pose/resources/hunger/thirst may be restored, to the captured baseline,
after the session is dropped and Reviver is confirmed offline. Make that a
single guarded update requiring exact id10/name/account12/account-name,
offline, unchanged level1/level2=1/XP0/training0/AA0/PvP0/GM0 and all other
gameplay invariants. Include the freshly observed post-session pose/resource
values in the update predicate so an intervening edit is not overwritten.
Require exactly one matched character and verify the result independently.

No generic row replacement, table restore, insertion/deletion, XP repair,
training-point reset, AA reset, faction reset or skill reset is authorized by
this proposal. If an invariant changes, disconnect, preserve evidence and
report the mismatch for a separate decision. Do not broaden cleanup silently.

## Receipt evidence and sequence

The reducer correctly ignores identical duplicate events for revision changes.
Consequently a wait for a larger progression revision cannot prove either
command. After review, the integration owner can add a temporary typed trace
immediately before applying a received Progression event in GameState. Capture
only the enum fields and the foreground before/after values, never packet
buffers. The probe can record receipt counters through a tracing subscriber;
event counts are evidence only, not product progression state.

1. Revalidate all preconditions; create the private snapshot/restore journal.
2. Login once, receive profile/Ready and initial ExpUpdate0. Check level1,
   training0/totalXP0, all skills0, language0=100, no target and no errors.
   Verify all persistent gameplay invariants before sending a command.
3. Record the current received-XP-event counter. Send `#set exp exp 0`; await
   a later typed Experience{bar_units:0}. Verify foreground level/bar1/0,
   unchanged snapshots/banks and exact persistent progression/AA state.
4. Record the current received-Level-event counter. Send `#set level 1`;
   await typed Level{level:1,reported_old_level:1,bar_units:0}. Verify unchanged
   reducer revision, skills/languages/training/XP/AA/PvP and all other gameplay
   invariants. Record any resource adjustment without assuming HP stays100.
5. Disconnect, wait for offline, compare invariants, restore only the guarded
   pose/resources footprint and independently verify the complete baseline.
6. Fresh login with no commands must reproduce profile1/XP0/training0 and
   ExpUpdate0. Disconnect, apply the same guarded resource restoration and
   independently verify offline/full baseline again.
7. Remove temporary runtime tracing. Keep the bounded probe and private
   evidence if the integration owner wants a repeatable fixture artifact.

Success proves receipt of an actual same-total ExpUpdate and same-level
LevelUpdate through the foreground path while duplicate values remain stable.
It does not establish live level gain/loss, earned XP, trainer purchases or
ordinary skill gain chance.
