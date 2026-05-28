# Session Handoff

Short operating brief for the next Rust gameplay-parity session. Keep this file
concise; durable audit state belongs in focused roadmap/audit docs.

## Current Branch And State

- Branch: `codex/rusty-mangos`
- Workspace: `C:\Users\subhe\Documents\New project`
- Current state: latest HEAD contains the arrived-chase orientation follow-up
  for the DB-creature facing regression.
- Previous pushed baseline before this follow-up was `e42c3fd71`
  (`Fix next-swing and creature facing stalls`).
- Melee leeway is currently `7.0 / 3.0` in `crates/wow-network/src/world/constants.rs`.

## Current Goal

User-directed gameplay fix: finish the DB-creature facing/motion regression
that can make an enemy turn back and forth every tick and stop landing melee
swings. Heroic Strike stale next-swing cleanup is already committed in
`e42c3fd71`.

- Gate/subsystem: world combat, player spell cast failure, next melee swing
  queueing, DB-creature chase/facing, melee arc validation.
- CMaNGOS reference remains the behavior source for melee/chase/facing.

## What Changed Recently

- `MapRuntimeManager::player_spell_cast_failure` now uses a mutable validation
  path so a same-spell queued next-swing action is cleared when validation
  fails for `SPELL_FAILED_NO_POWER`.
- Heroic Strike no-rage retries no longer leave a stale queued
  `QueuedNextMeleeSpell` that blocks later casts until relog.
- Commit-history investigation points at `7da5b04b4` as the likely regression:
  chase completion stopped converting `CreatureMotionState::Chase` back to
  `Idle`, while `face_db_creature_toward_position` began allowing facing on an
  arrived chase. Since `advance_db_creature_motion_runtime` keeps copying
  `chase.destination` into `current_position` after arrival, a face update that
  only changed `current_position.orientation` was overwritten on the next tick.
- The arrived-chase follow-up persists facing into
  `chase.destination.orientation`, and narrows the previous overlap heuristic
  from object-radius distance to exact coordinate overlap only.
- Added focused regression coverage proving an arrived chase face update
  survives a later motion advance.

## Tests Run

- `cargo test -p wow-network player_spell_cast_failure_clears_stale_next_melee_queue_when_power_missing -- --nocapture`
  - passed
- `cargo test -p wow-network map_runtime_skips_in_place_facing_for_overlapping_target -- --nocapture`
  - passed
- `cargo test -p wow-network melee_arc_treats_overlapping_positions_as_valid_facing -- --nocapture`
  - passed
- `cargo test -p wow-network spell_cast_failure_rejects_missing_power_gcd_and_duplicate_queue -- --nocapture`
  - passed
- `cargo test -p wow-network map_runtime_refuses_in_place_facing_while_creature_is_moving -- --nocapture`
  - passed
- `cargo test -p wow-network map_runtime_manager_does_not_snap_face_before_pending_swing -- --nocapture`
  - passed
- `cargo test -p wow-network map_runtime_arrived_chase_facing_survives_motion_advance -- --nocapture`
  - passed
- `cargo test -p wow-network starter_melee_spell_failure_uses_melee_validity_before_damage -- --nocapture`
  - failed with existing assertion mismatch: left `None`, right
    `Some(SPELL_FAILED_OUT_OF_RANGE)`. This appears unrelated to the narrowed
    facing fix and also showed up during the broad gate failure.
- `cargo check -p wow-network --lib`
  - passed
- `.\scripts\test-rust.cmd`
  - failed in the existing broad `cargo test --workspace` suite. The run still
    showed the workspace compiles, but 26 `wow-network` tests failed, including
    local MySQL access failures for `root@localhost` and pre-existing gameplay
    expectation failures such as Battle Shout rage cost.

## Known Blockers / Unproven Areas

- The full Rust gate is not green in this local environment. Before treating
  this as release-ready, either fix/triage the existing red tests or rerun with
  the expected local DB/vmap setup.
- The arrived-chase orientation fix has focused synthetic coverage, but still
  needs live client verification against the reported close-range creature
  turn-loop scenario.

## Recommended Next Task

- Playtest on `codex/rusty-mangos`: stand inside/near a hostile creature and
  verify it stops turning every frame and can melee.
- If live creature behavior still jitters, compare against CMaNGOS
  `TargetedMovementGenerator` final facing and the Rust arrived-chase retry path
  before adding any client-facing movement heuristics.

## Key Files

- `crates/wow-network/src/world/map_runtime/systems/players.rs`
- `crates/wow-network/src/world/map_runtime/map_manager/spells.rs`
- `crates/wow-network/src/world/combat/melee.rs`
- `crates/wow-network/src/world/map_runtime/systems/creature_motion.rs`
- `crates/wow-network/src/world/map_runtime/map_manager/creatures/combat.rs`
- `crates/wow-network/src/world/tests/spells.rs`
- `crates/wow-network/src/world/tests/navigation_motion.rs`
- `crates/wow-network/src/world/tests/death_aggro.rs`
