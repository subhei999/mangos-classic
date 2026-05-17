# Session Handoff

Short operating brief for the next Rust migration session. Durable roadmap
history belongs in `docs/rust_migration_plan.md`, gate status in
`docs/playable_gate_board.md`, and branch split details in
`docs/playable_execution_roadmap.md`.

## Current Branch And State

- Branch: `codex/rusty-mangos`, main checkout:
  `C:\Users\subhe\Documents\New project`.
- Current local work merges the `codex/g12-movement-actor-proxy` slice into the
  main integration branch on top of the existing performance harness work.
- The branch now contains:
  - the Mage/parity `Polymorph` and confused-motion groundwork already in
    `codex/rusty-mangos`;
  - the release-mode playerbot perf scenario in
    `scripts/start-playerbot-idle-perf.ps1`;
  - the real thin-client load harness in `bins/world-load-test` plus
    `scripts/start-thin-client-load.ps1`;
  - the feature-flagged movement actor proxy path from
    `codex/g12-movement-actor-proxy`, including explicit superseded movement
    replies and movement-actor observability.
- Playerbots remain disabled for normal local testing in
  `config/worldserver.local.toml`.
- The user remains the Northshire Checkpoint 2 grader through real-client
  playtesting. Do not add or maintain a Northshire grading harness.

## Current Goal

- Use the thin-client benchmark as the baseline for movement/runtime
  architecture work.
- Immediate performance focus: compare the current movement path against future
  batched player-movement work using the documented `50`-client release
  scenario first.
- Immediate gameplay follow-up after perf work: return to Mage/parity live
  verification, especially the open `Polymorph` combat-state regression and the
  pending Mana Shield / Ward / utility spell smoke list.

## What Changed Recently

- Added a reproducible release-mode playerbot perf scenario:
  `config/worldserver.perf.toml` and
  `scripts/start-playerbot-idle-perf.ps1`.
- Playerbot perf setup now supports `combat_enabled`, `force_active`,
  `local_roam_only`, and deterministic scatter/radius placement.
- Added a gate so the async playerbot planner skips map work entirely when the
  world only contains planner-free perf bots.
- Added a real thin-client load harness in `bins/world-load-test` that:
  - seeds dedicated accounts and characters;
  - performs normal SRP auth, world auth, character enum, and player login;
  - emits a more real-client-like movement mix instead of pure heartbeats.
- Added release-mode stack support to the restart scripts so benchmark runs use
  `target\release\*.exe`.
- Added `docs/performance_movement_benchmark.md` as the append-only home for
  benchmark methodology and baseline results.
- Merged in the feature-flagged movement actor proxy:
  - `world.experimental_movement_actor`;
  - bounded mailbox and batch drain support for movement updates;
  - explicit `MovementUpdateOutcome::Superseded` semantics for deduped movement;
  - movement actor queue/latency/batch observability alongside direct mutex
    timing.

## Performance Baseline

- The current baseline scenario is the `50`-client thin-client release run
  documented in `docs/performance_movement_benchmark.md`.
- First settled baseline scrape:
  - `wow_map_tick_duration_average_1m_milliseconds`: `51.656`
  - `wow_map_tick_lag_average_1m_milliseconds`: `5.079`
  - `wow_map_tick_duration_latest_milliseconds`: `70.323`
  - `wow_map_phase_duration_average_1m_milliseconds{phase="player_environment"}`:
    `24.477`
  - observed `48` sessions / `48` active players at scrape time
- Earlier same-run reference scrape:
  - tick avg `44.739 ms`
  - lag avg `5.107 ms`
  - observed `51` sessions / `50` active players

## Tests Run

- `cargo test -p wow-network polymorph --lib` passed.
- `cargo test -p wow-network hostile_spell_cast_failure_checks_range_los_and_facing_from_map --lib`
  passed.
- `cargo test -p wow-network cast_time_spell_sends_start_before_delayed_go_and_effects --lib`
  passed.
- `cargo test -p wow-network cast_time_spell_rechecks_facing_before_completion_go --lib`
  passed.
- `cargo test -p wow-network map_runtime_environmental_damage_interrupts_regen --lib`
  passed.
- `cargo test -p wow-network stand_state_change_to_stand_cancels_consumable_regen_aura --lib`
  passed.
- `cargo test -p wow-network db_creature_confused_motion_uses_dedicated_state_and_pause_timer --lib`
  passed.
- `cargo test -p wow-network map_runtime_confused_motion_start_guids_use_control_scheduler --lib`
  passed.
- `cargo test -p wow-network map_runtime_tick_starts_confused_motion_from_control_scheduler --lib`
  passed.
- `cargo test -p wow-network polymorph_clears_creature_combat_through_map_owner_and_allows_reentry --lib`
  passed.
- `cargo test -p wow-network creature_combat_ownership_marks_player_in_combat --lib`
  passed.
- `cargo test -p wow-network begin_shared_db_creature_combat_uses_mapruntime_liveness_without_session_cache --lib`
  passed.
- `cargo test -p wow-config world_config_accepts_playerbot_roster --lib`
  passed.
- `cargo test -p worldserver playerbot_spawn_configs --bin worldserver`
  passed.
- `cargo test -p wow-network map_runtime_force_active_playerbot_moves_without_client_interest --lib`
  passed.
- `cargo test -p wow-network map_runtime_combat_disabled_playerbot_skips_combat_planning --lib`
  passed.
- `cargo test -p wow-network map_runtime_local_roam_only_playerbot_skips_planner_inputs_and_still_moves --lib`
  passed.
- `cargo test -p wow-network map_runtime_manager_skips_async_planner_for_local_roam_only_perf_bots --lib`
  passed.
- `cargo test -p wow-network movement_actor --lib` passed.
- `cargo test -p wow-network prometheus_render_includes_histogram_and_opcode_labels --lib`
  passed.
- `cargo test -p wow-network map_runtime_manager_movement_actor --lib` passed.
- `cargo test -p wow-network --lib` passed earlier on both slices before merge.
- `cargo build -p world-load-test` passed.
- `cargo build --release -p world-load-test` passed.
- `cargo test -p world-load-test` passed.
- `cargo build --release -p worldserver` passed.
- `.\scripts\test-rust.cmd` passed earlier apart from Windows binary-lock cases
  when auth/world executables were still running.
- Verified launcher runs:
  - `powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\start-playerbot-idle-perf.ps1`
  - `powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\start-thin-client-load.ps1 -ClientCount 50 -HoldSeconds 10`
  - `powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\start-thin-client-load.ps1 -ClientCount 200 -HoldSeconds 10`
  - `powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\start-thin-client-load.ps1 -ClientCount 500 -HoldSeconds 10`

## Real-Client Verification Needed

- `Polymorph` still needs live-client confirmation for:
  - sheep display and helper healing;
  - no-facing cast allowance;
  - confused wander;
  - break on melee, direct spell damage, DoTs, and Blizzard ticks;
  - clean combat drop and re-entry behavior.
- Current user-reported blocker remains possible: after `Polymorph`, the player
  may remain out of combat while the mob attacks, allowing food/drink usage.
- Blink, Mana Shield, Fire/Frost Ward, Remove Curse, Detect Magic, Dampen
  Magic, Arcane Missiles, Blizzard, and Flamestrike still need the planned
  real-client smoke checks.

## Known Follow-Ups

- Compare the current direct movement path and the feature-flagged movement
  actor path against future batched player-movement work using the same
  `50`-client benchmark scenario.
- Decide whether the movement actor proxy remains an experiment or becomes the
  stepping stone toward a fuller map-owned actor model.
- Preserve the explicit superseded-movement contract if batching semantics keep
  evolving: deduped updates should not masquerade as applied empty results.
- Full dynamic-object aura semantics and periodic player-kill corpse/loot prep
  are still incomplete beyond the current spell slice.
- Windows binary locks still affect local rebuild loops if auth/world processes
  are left running during verification.

## Key Files

- `bins/world-load-test/src/main.rs`
- `bins/worldserver/src/main.rs`
- `config/worldserver.local.toml`
- `config/worldserver.perf.toml`
- `crates/wow-config/src/lib.rs`
- `crates/wow-network/src/observability.rs`
- `crates/wow-network/src/world/map_runtime/map.rs`
- `crates/wow-network/src/world/map_runtime/map/playerbots.rs`
- `crates/wow-network/src/world/map_runtime/map_manager.rs`
- `crates/wow-network/src/world/map_runtime/movement_actor.rs`
- `crates/wow-network/src/world/mod.rs`
- `crates/wow-network/src/world/playerbots.rs`
- `crates/wow-network/src/world/server/movement.rs`
- `crates/wow-network/src/world/spells.rs`
- `crates/wow-network/src/world/spells/effects.rs`
- `crates/wow-network/src/world/tests.rs`
- `docs/performance_movement_benchmark.md`
- `docs/northshire_spell_audit.md`
- `scripts/restart-game-stack.ps1`
- `scripts/run-client-stack-18085.ps1`
- `scripts/start-playerbot-idle-perf.ps1`
- `scripts/start-thin-client-load.ps1`
