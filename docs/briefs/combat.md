# Combat Brief

This is the short G8 combat context for agents. Keep it compact and update it
when combat architecture or test commands change.

## Goal

Implement CMaNGOS/Classic combat behavior without fake gameplay values. Combat
math should come from DB data, DBC/source-derived values, or CMaNGOS formulas.
If a backing system is missing, leave the gap explicit and logged.

Current requested order:

1. Real melee roll table.
2. Real swing timers.
3. Real damage formula.
4. Reach/model modifiers.
5. Swing error packets.
6. Vmaps.
7. Full PathFinder and smoothing.
8. Threat model and enemy targeting.

## Current Rust Shape

- `crates/wow-network/src/world/combat.rs` is now a facade over
  `crates/wow-network/src/world/combat/`.
- `combat/outcome.rs` owns melee outcomes, armor reduction, player offensive
  outcome helpers, and player defense snapshots.
- `combat/aggro.rs`, `combat/lifecycle.rs`, `combat/runtime.rs`, `combat/melee.rs`,
  `combat/motion.rs`, `combat/evade.rs`, and `combat/broadcast.rs` own the
  active combat flow.
- `maps/map.rs` and `maps/map_manager.rs` own shared DB-creature/player damage
  events and observer packet fanout.
- `entities/update_data.rs` derives `PlayerCombatStats` from DB-backed player
  world stats plus equipped item templates.
- `wow-db/src/world_data.rs` loads DB creature template combat fields.

## Current Implemented Pieces

- DB creature swings use min/max template damage and CMaNGOS-shaped melee
  outcome ordering.
- `SMSG_ATTACKERSTATEUPDATE` serializes miss, dodge, parry, block, glancing,
  crit, crushing, and normal outcomes.
- Physical damage runs through the current CMaNGOS-shaped armor reduction
  helper.
- Player auto-attacks use equipped main-hand damage plus class/stat attack power
  and target creature DB template armor.
- Player and DB-creature melee reach use the CMaNGOS combined combat reach
  shape: attacker reach + victim reach + 1.33yd base melee offset, clamped to at
  least 5yd. DB creature reach/bounding radius is loaded from
  `creature_model_info` for the selected template display id.
- Player melee failures send empty Vanilla `SMSG_ATTACKSWING_*` packets for
  not-in-range, bad-facing, dead-target, and can't-attack cases.
- CMaNGOS `VMAP_7.0` static LOS is wired through a native vmap bridge with a
  safe Rust wrapper. DB-creature navigation uses it for melee and starter melee
  spell validity when compatible vmap files are present.
- Starter melee spells now report `SPELL_FAILED_LINE_OF_SIGHT` for navigation
  LOS failures.
- Incoming creature swings use `PlayerMeleeDefenseInput` built from live player
  stats and equipped item templates:
  - armor = agility-derived physical resistance plus equipped item armor;
  - shield block value = equipped offhand shield `item_template.block` plus the
    CMaNGOS-style strength component.
- Shared `MapRuntime` applies player/creature damage and broadcasts observer
  packets.

## Main Gaps

- Player offensive dodge/parry/block eligibility needs real target state,
  facing/arc rules, skills, and auras.
- Swing timer reset/queue behavior needs a dedicated parity pass.
- Moving melee leeway, player race/model DBC reach loading, and offhand/reset
  timer parity are not implemented.
- Current local vmaps under `C:\World of Warcraft Classic\vmaps` are
  `VMAP_4.0`, while the CMaNGOS source bridge expects `VMAP_7.0`; Rust detects
  this and keeps LOS permissive until compatible files are available.
- Full PathFinder smoothing is not implemented.
- Threat model and enemy targeting are not implemented.

## CMaNGOS Reference Paths

- `src/game/Entities/Unit.cpp`
  - `Unit::UpdateMeleeAttackingState` around line 648.
  - `Unit::CalculateMeleeDamage` around line 1923.
  - `Unit::RollMeleeOutcomeAgainst` around line 2762.
  - `Unit::CalculateEffectiveDodgeChance` around line 3282.
  - `Unit::CalculateEffectiveParryChance` around line 3306.
  - `Unit::CalculateEffectiveBlockChance` around line 3338.
  - `Unit::CalculateEffectiveCrushChance` around line 3362.
  - `Unit::CalculateEffectiveGlanceChance` around line 3376.
  - `Unit::CalculateEffectiveCritChance` around line 3786.
  - `Unit::CalculateEffectiveMissChance` around line 3823.
- `src/game/Entities/StatSystem.cpp`
  - `Player::UpdateBlockPercentage` around line 397.
  - `Creature::UpdateArmor` around line 664.
- `src/game/Entities/Player.cpp`
  - `Player::GetShieldBlockValue` around line 5128.
- `src/game/Entities/Creature.cpp`
  - `Creature::SelectLevel` around line 1265.
- `src/game/Globals/ObjectMgr.cpp`
  - `ObjectMgr::LoadCreatureClassLvlStats` around line 720.
- `src/game/AI/BaseAI/UnitAI.cpp`
  - `UnitAI::UpdateAI` / melee update and `MoveChase` calls.
- `src/game/MotionGenerators/MotionMaster.cpp`
  - `MotionMaster::MoveChase` around line 318.
- `src/game/MotionGenerators/TargetedMovementGenerator.cpp`
  - `ChaseMovementGenerator` behavior, dynamic target distance, cut path, and
    reachability.
- `src/game/Entities/Object.cpp`
  - `WorldObject::IsWithinLOS` and `IsWithinLOSInMap`.
- `src/game/Maps/Map.cpp`
  - `Map::IsInLineOfSight`.
- `src/game/vmap/VMapManager2.cpp`
  - `loadMap`, coordinate conversion, and `isInLineOfSight`.

Use `rg` to confirm line numbers before citing or porting behavior.

## Focused Tests

Useful unit filters:

- `cargo test -p wow-network melee --lib`
- `cargo test -p wow-network player_main_hand --lib`
- `cargo test -p wow-network creature_melee --lib`
- `cargo test -p wow-network db_creature_chase_motion --lib`
- `cargo test -p wow-network db_creature_navigation --lib`
- `cargo test -p wow-network world_data_parses_cmangos_vmap_file_names --lib`
- `cargo test -p wow-network map_runtime_db_creature_damage --lib`
- `cargo test -p wow-network --lib`

Baseline:

- `cargo fmt --check`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `.\scripts\test-rust.cmd`

Harnesses when combat flow changes:

- `.\scripts\test-starter-zone-flow.cmd`
- `.\scripts\test-world-flow.cmd`

## Real-Client Success Criteria For G8

- Hostile DB creatures aggro only when CMaNGOS reaction/range rules allow.
- Creatures chase, face, stop in reach, and swing at believable cadence.
- Player attacks/spells fail visibly when out of range or facing is invalid.
- Weapon/equipment changes affect stats and combat results without relog.
- Multiple clients see the same creature health/death/loot state.
- No fake NPCs, fixture-only damage, or hardcoded starter creature special cases
  are required for the normal client path.
