# Session Handoff

Short operating brief for the next Rust migration session. Durable roadmap
history belongs in `docs/rust_migration_plan.md`, gate status in
`docs/playable_gate_board.md`, and branch split details in
`docs/playable_execution_roadmap.md`.

## Current Branch And State

- Branch: `codex/rusty-mangos`, in the main checkout at
  `C:\Users\subhe\Documents\New project`.
- Current user-directed priority: Northshire Checkpoint 2 real-client parity.
  The user remains the grader through live playtesting; do not add a
  Northshire grading harness.
- Latest local work is uncommitted and adds the first CMaNGOS-shaped Creature
  EventAI slice plus source-backed wounded slowdown: DB-backed
  `creature_ai_scripts` loading through `ObjectMgr`, map-owned HP EventAI
  evaluation, `ACTION_T_FLEE_FOR_ASSIST`, `ACTION_T_SET_WALK`, and CMaNGOS
  generic under-30%-health movement slowdown honoring `StaticFlags2`
  `NO_WOUNDED_SLOWDOWN`.
- Playerbots remain disabled by default for normal multiplayer/Northshire
  testing: `config/worldserver.local.toml` has `[playerbots] enabled = false`
  and `[playerbots.random] enabled = false`.

## Current Goal And Recommended Next Task

- Goal: make the Northshire Checkpoint 2 playtest loop stable enough for the
  user to grade in the real client without disconnects, broken quest/object
  interactions, corpse/respawn stalls, or obvious CMaNGOS behavior gaps.
- Recommended next task: real-client smoke wounded movement in Northshire.
  Young Wolf and other non-opted-out starter mobs should slow below 30% health
  without fleeing. Mobs with CMaNGOS HP flee rows, such as Riverpaw Runt,
  Goldtooth, or Riverpaw Scout, should still flee at their scripted threshold.
- If continuing CreatureAI parity, the next source-backed step is broader
  EventAI support: more event types/actions from CMaNGOS
  `src/game/AI/EventAI`, not hardcoded per-creature behavior.

## Recent Implemented Work

- Added `CreatureAiScriptQuery` and `get_creature_ai_scripts_for_entry` to
  `crates/wow-db/src/world_data.rs`.
- Added `ObjectMgr::creature_ai_scripts` caching and test cache stats in
  `crates/wow-network/src/world/globals/object_mgr.rs`.
- Added `CreatureMotionState::Flee` and `CreatureFleeMotion`, plus
  `start_db_creature_flee_motion_runtime`, run-speed retiming, and timed
  completion back to idle.
- Added `UNIT_FLAG_FLEEING` and included it in DB creature `UNIT_FIELD_FLAGS`
  while a creature is fleeing.
- Added map-owned EventAI HP evaluation for CMaNGOS:
  `EVENT_T_HP = 2`, `ACTION_T_FLEE_FOR_ASSIST = 25`, one-shot script tracking,
  `event_chance`, and CMaNGOS default flee delay of 10 seconds.
- Generalized HP EventAI dispatch so supported actions execute in script order.
  `ACTION_T_SET_WALK = 58` now toggles default movement walk/run and chase
  walk/run from DB params; active chase splines are retimed and resent with the
  correct run flag.
- Fixed a P0 real-client disconnect on attacking EventAI creatures by matching
  `creature_ai_scripts` DB column types: `id` is signed `INT` and
  `event_chance` is unsigned `INT`, not `u32`/`u8` respectively.
- Wired player melee, ranged auto attack, and player spell damage paths to
  evaluate loaded EventAI HP scripts after non-lethal damage.
- Active creature AI now pauses chasing, casting, and swinging while the
  creature is fleeing, then clears the flee flag and resumes normal combat once
  the flee timer ends.
- Corrected the weak-wolf parity assumption: CMaNGOS has generic wounded
  slowdown in `Creature::IsSlowedInCombat` and movement generators, independent
  of EventAI rows. Rust now loads `creature_template.StaticFlags2`, honors
  `NO_WOUNDED_SLOWDOWN`, applies the CMaNGOS linear under-30%-health speed
  multiplier to random and targeted chase movement, and retimes active movement
  when damage crosses the threshold so the real client sees the new spline.

## Tests Run

- `cargo test -p wow-network map_runtime_event_ai_hp --lib` passed with four
  focused EventAI HP tests covering flee and set-walk chase behavior after the
  DB type fix.
- `cargo test -p wow-network wounded_slowdown --lib` passed with two focused
  tests for the generic CMaNGOS wounded slowdown rule and opt-out flag.
- `cargo test -p wow-network db_creature_damage_crossing_wounded_threshold_retimes_active_chase --lib`
  passed.
- `cargo check -p wow-network` passed after formatting.
- Restarted the local game stack successfully after the DB type fix:
  authserver on `127.0.0.1:13724`, worldserver on `127.0.0.1:18085`,
  dashboard on `127.0.0.1:9091`.
- `cargo test -p wow-network --lib` ran 696 tests; 691 passed and five
  existing DB-backed spell tests failed with local DB pool timeouts while
  waiting for an open connection. The failing names were
  `eviscerate_uses_combo_points_for_damage_and_clears_them_on_hit`,
  `fireball_with_periodic_aura_applies_direct_damage_and_dot`,
  `player_damage_spell_executes_each_damage_effect_slot`,
  `sinister_strike_cast_uses_energy_and_spell_damage_log_result`, and
  `cast_time_spell_sends_start_before_delayed_go_and_effects`.
- `.\scripts\test-rust.cmd` was not rerun for the set-walk follow-up. Last
  known full-script attempt progressed through most workspace tests but failed
  in existing DB-backed spell tests because local MySQL root auth is denied:
  `1698 (28000): Access denied for user 'root'@'localhost'`.

## Real-Client Verification Needed

- Weak-creature movement:
  Young Wolf and other normal starter mobs should visibly slow below 30% health
  and not flee unless EventAI says to flee; mobs with HP set-walk rows should
  still switch to scripted walk/run behavior.
- Flee combat behavior:
  while fleeing, the creature should not swing/cast/chase; after the CMaNGOS
  flee delay, it should resume normal combat if still alive/in combat.
- Existing Checkpoint 2 regression smoke still matters:
  quest GO gating after abandon, Battered Chest loot including multi-item loot,
  Milly bucket cancel/interruption, enemies attacking during GO interaction,
  logout movement lock/cancel, Hearthstone cooldown icon after relog, visible
  buff icons after relog, corpse/respawn, respawn aggro grace, trainer/generic
  gossip, and Heroic Strike/rage display.

## Current Follow-Ups

- This is not a full CMaNGOS EventAI port yet. It establishes the DB-backed,
  map-owned foundation and implements HP flee plus set-walk actions; generic
  wounded slowdown is source-backed and does not require `creature_ai_scripts`
  rows.
- Broader CreatureAI parity still needs CMaNGOS EventAI event/action coverage,
  script action dispatch, assist/fear nuances, and any spell-cast EventAI rows
  that starter-zone creatures rely on.
- Flee movement currently uses the CMaNGOS 30-yard run-away shape and the
  project pathing guardrail. If real-client behavior exposes pathing oddities,
  compare against CMaNGOS `FleeingMovementGenerator` / `PanicMovementGenerator`
  before tuning.
- If the full Rust test script is required locally, fix the MySQL root auth
  environment or rerun with the expected DB credentials/container.

## Key Files

- `crates/wow-db/src/world_data.rs`
- `crates/wow-network/src/world/globals/object_mgr.rs`
- `crates/wow-network/src/world/motion/motion_master.rs`
- `crates/wow-network/src/world/combat/{aggro.rs,lifecycle.rs,motion.rs,runtime.rs}`
- `crates/wow-network/src/world/entities/creature.rs`
- `crates/wow-network/src/world/map_runtime/map/{creature_combat.rs,creature_damage.rs,creature_motion.rs}`
- `crates/wow-network/src/world/map_runtime/{map.rs,map_manager.rs}`
- `crates/wow-network/src/world/spells/effects.rs`
- `crates/wow-network/src/world/opcodes.rs`
- `crates/wow-network/src/world/tests.rs`
