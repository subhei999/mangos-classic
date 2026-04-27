# Checkpoint 2 Codebase Sustainability Audit

Date: 2026-04-27

Scope: Rust world/gameplay code after Quest System v1, before XP/level-up,
combat v2, death/respawn, and trainer learning.

## Summary

Quest v1 proved a real client loop, but it also confirmed that the current Rust
world layer has outgrown the early include-based split. The code is still
serviceable, but the next Checkpoint 2 slices will touch the same few files from
several directions unless we split subsystem ownership first.

Completed before XP/combat work: a behavior-preserving world module split and a
shared DB-creature death finalization path.

Tracked issue: <https://github.com/subhei999/mangos-classic/issues/48>
completed in the first split pass.

Related existing issue: <https://github.com/subhei999/mangos-classic/issues/5>
for `wow-db/src/character.rs` split and transaction debt.

## Current Pressure Points

Line-count audit:

- `crates/wow-network/src/world/interactions.rs`: about 3.6k lines.
- `crates/wow-network/src/world/tests.rs`: about 2.3k lines.
- `crates/wow-db/src/character.rs`: about 2.4k lines.
- `bins/starter-zone-flow-test/src/main.rs`: about 1.8k lines.
- `crates/wow-network/src/world/bootstrap.rs`: about 1.6k lines.
- `crates/wow-network/src/world/mod.rs`: about 1.5k lines.

`interactions.rs` currently mixes:

- chat and text emotes;
- starter spell casting;
- item query and inventory movement;
- creature query;
- gossip, NPC text, vendor routing;
- quest status/details/accept/progress/reward;
- vendor buy/sell;
- melee combat and creature retaliation;
- loot open/autostore/money/release;
- packet builders for all of the above.

`mod.rs` still owns opcode constants, session/runtime structs, character
create/delete/login/logout, authenticated dispatch, movement, and test imports.
That was useful while proving the first world, but it now makes gameplay slices
harder to review.

## CMaNGOS Reference Shape

The Rust rewrite should not copy C++ inheritance structure directly, but the
source tree gives good subsystem boundaries:

- Quest packets and turn-in: `src/game/Quests/QuestHandler.cpp`.
- Quest state/reward/kill credit API: `src/game/Entities/Player.h` and
  `src/game/Entities/Player.cpp`.
- XP and level-up: `Player::GiveXP` / `Player::GiveLevel` in
  `src/game/Entities/Player.cpp`.
- Player kill credit: `Player::KilledMonster` /
  `Player::KilledMonsterCredit` in `src/game/Entities/Player.*`.
- Creature runtime state: `src/game/Entities/Creature.*`.
- Combat routing: `src/game/Combat/CombatHandler.cpp` and
  `src/game/Combat/CombatManager.*`.
- Loot packets and loot state: `src/game/Loot/LootHandler.cpp` and
  `src/game/Loot/LootMgr.*`.
- Gossip menu packet shape: `src/game/Entities/GossipDef.*`.

This maps cleanly to focused Rust modules without requiring a broad object
model port.

## Completed Split

The first pass stayed mechanical and behavior-preserving by keeping the
include-based parent-module visibility model. No protocol behavior changes were
intended.

Added `crates/wow-network/src/world/` modules:

- `opcodes.rs`: opcode constants and expected no-op metadata.
- `session.rs`: `WorldSessionState`, `ActiveCharacter`,
  `DbCreatureRuntime`, `DbCreatureLootRuntime`, and world runtime state.
- `chat.rs`: chat and text emote handlers/builders.
- `spells.rs`: starter spell parsing, supported spell table, cast result,
  spell-go builders, and spell-to-combat bridge.
- `inventory.rs`: item query, inventory move/split/destroy, bag-slot helpers,
  and inventory update builders.
- `creatures.rs`: creature query and DB creature create/query helpers that are
  not bootstrap-specific.
- `gossip.rs`: gossip hello/select, NPC text, and gossip message builders.
- `quests.rs`: quest query/status/details/accept/complete/reward, quest-log
  update builders, and kill-credit service entry point.
- `vendors.rs`: vendor list, buy, sell, and vendor packet builders.
- `combat.rs`: attack swing/stop, combat ticks, DB creature damage,
  retaliation, attacker-state updates, rage/health/power combat updates.
- `loot.rs`: loot open, autostore, money, release, loot response builders.
- Later, split `tests.rs` by subsystem once the modules are stable.

The starter-zone harness can stay single-file for now, but if it grows during
XP/death/trainer work, split it into protocol helpers, DB fixture/content
helpers, and scenario steps.

## Most Important Design Correction

Before adding XP and combat v2, create one shared DB-creature death finalizer.
This is now in place as `finalize_db_creature_death(...)`.

Current behavior:

- melee death and supported starter spell death both grant quest credit, but
  they reach that by calling the shared credit helper from separate damage
  paths;
- loot/corpse flags, attack stop, quest updates, and future XP are not yet
  routed through one finalization point.

Checkpoint 2 target shape:

```text
damage source
  -> apply damage to DB creature runtime
  -> if health reaches zero:
       finalize_db_creature_death(...)
          -> stop combat / send attack stop
          -> set corpse and loot runtime state
          -> grant quest kill credit
          -> award creature XP
          -> send required object/player update packets
```

That keeps future spell, pet, DoT, group, tagging, and script paths from
duplicating kill side effects.

## DB Layer Notes

`wow-db/src/character.rs` is already tracked by issue #5. Do not fold gameplay
policy deeper into it. Prefer:

- DB modules own schema-compatible reads/writes.
- World/gameplay modules decide when a mutation should happen.
- Multi-table reward, XP, inventory, death, and lifecycle updates should move
  toward transactions as soon as they can fail halfway.

Near-term DB split candidates:

- `character_lifecycle.rs`: create/delete and cleanup orchestration.
- `character_inventory.rs`: inventory, item instance, money mutations.
- `character_quests.rs`: quest status load/accept/progress/reward.
- `character_progression.rs`: XP, level, stats, spells/skills.
- `world_creatures.rs`, `world_loot.rs`, `world_quests.rs`, `world_vendors.rs`
  if `world_data.rs` grows beyond simple query helpers.

## Risk Ranking

P1 status:

- Shared DB-creature death finalization is in place for melee and supported
  starter spell kills. XP/level-up should now hook into this finalizer rather
  than into individual damage sources.

P2/P3/P4 tracked:

- #48: split world gameplay handlers before XP/combat v2. Completed and closed.
- #5: split `wow-db/src/character.rs` and add transaction boundaries.

No new P2/P3 gameplay parity bugs were found during this audit.

## Recommended Next Order

1. XP/level-up v1 using `finalize_db_creature_death(...)` and the quest reward
   path.
2. Combat v2: retaliation timing, combat state, damage rolls, rage, death risk.
3. Death/respawn v1.
4. Trainer v1, once leveling has a real reason to train.

Keep Northshire / Human Warrior as the golden path throughout.
