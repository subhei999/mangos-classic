# Session Handoff

Short operating brief for the next Rust gameplay-parity session. Keep this file
concise; durable audit state belongs in focused roadmap/audit docs.

## Current Branch And State

- Branch: `codex/rusty-mangos`
- Workspace: `C:\Users\subhe\Documents\New project`
- Current state: dirty worktree with focused combat fixes for Heroic Strike
  next-swing queue cleanup and DB-creature melee facing/overlap handling.
- Latest known clean upstream before these edits was synced with
  `origin/codex/rusty-mangos`.
- Melee leeway is currently `7.0 / 3.0` in `crates/wow-network/src/world/constants.rs`.

## Current Goal

User-directed gameplay fix: remove regressions where Heroic Strike can wedge in
the next-swing slot after no-rage spam, and where nearby DB creatures can get
stuck repeatedly facing instead of landing melee swings.

- Gate/subsystem: world combat, player spell cast failure, next melee swing
  queueing, DB-creature chase/facing, melee arc validation.
- CMaNGOS reference remains the behavior source for melee/chase/facing.

## What Changed Recently

- `MapRuntimeManager::player_spell_cast_failure` now uses a mutable validation
  path so a same-spell queued next-swing action is cleared when validation
  fails for `SPELL_FAILED_NO_POWER`.
- Heroic Strike no-rage retries no longer leave a stale queued
  `QueuedNextMeleeSpell` that blocks later casts until relog.
- `has_in_arc` treats overlapping source/target positions as valid facing, and
  DB-creature in-place facing skips meaningless near-overlap updates instead of
  emitting a new facing spline.
- Added focused regression tests for stale Heroic cleanup, overlap arc validity,
  and suppressed overlap-facing packets.

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
- These fixes have focused synthetic coverage, but still need live client
  verification against the reported Heroic Strike no-rage spam and close-range
  creature facing scenarios.

## Recommended Next Task

- Playtest on `codex/rusty-mangos`: spam Heroic Strike at zero rage, gain rage,
  and confirm it can be queued without relogging; then stand inside/near a
  hostile creature and verify it stops turning every frame and can melee.
- If live creature behavior still jitters outside true overlap, compare against
  CMaNGOS `TargetedMovementGenerator` final facing and adjust the arrived-chase
  swing retry path rather than adding client-facing movement heuristics.

## Key Files

- `crates/wow-network/src/world/map_runtime/systems/players.rs`
- `crates/wow-network/src/world/map_runtime/map_manager/spells.rs`
- `crates/wow-network/src/world/combat/melee.rs`
- `crates/wow-network/src/world/map_runtime/systems/creature_motion.rs`
- `crates/wow-network/src/world/map_runtime/map_manager/creatures/combat.rs`
- `crates/wow-network/src/world/tests/spells.rs`
- `crates/wow-network/src/world/tests/navigation_motion.rs`
- `crates/wow-network/src/world/tests/death_aggro.rs`
