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
- Immediate performance focus: keep the new movement-owned
  `player_environment` path in place, verify it against the `200`-client
  thin-client scenario, and shift investigation toward the next bottleneck in
  the movement/runtime path.
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
  - now synchronizes movement start after login/bootstrap so the steady-state
    movement phase does not overlap the login storm.
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
- Finished the remaining ownership move for player environment:
  - movement/login/teleport paths now own environment-flag refresh;
  - the environment tick advances cached timers and only periodically
    revalidates a small at-risk player set;
  - new observability tracks geometry refreshes, timer-only processing, packets
    emitted, and subphase timings inside the environment tick.

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
- Post-merge movement-actor A/B runs are now also recorded there. With a more
  batch-friendly shape (`50 ms` move interval, `1 ms` login stagger), the
  feature-flagged movement actor proxy was still effectively flat versus the
  direct path:
  - `50` clients: `74.947 ms` off vs `77.639 ms` on
  - `200` clients: `269.322 ms` off vs `267.320 ms` on
  - actor batch depth remained modest (`1.245` at `50`, `3.089` at `200`)
  - `player_environment` stayed dominant, so the proxy slice alone does not
    materially improve the benchmark
- A follow-up optimization on the player-environment tick now caches safe
  players' environment flags, adds a bounded recheck window, and keeps only
  at-risk players on high-frequency checks. In the same harsher `50`-client
  scenario, tick average improved to `62.888 ms` with actor off and `62.206 ms`
  with actor on, down from `74.947 ms` and `77.639 ms` respectively.
- The `200`-client retest is unblocked. The harness no longer crashes with
  `STATUS_ACCESS_VIOLATION (0xc0000005)` under the `50 ms` / `1 ms` shape now
  that movement waits for login/bootstrap completion.
- Updated `200`-client synchronized-start scrapes:
  - actor off: `90.000 ms` tick avg, `5.712 ms` lag avg,
    `33.922 ms` player environment avg
  - actor on: `95.985 ms` tick avg, `16.120 ms` lag avg,
    `35.369 ms` player environment avg
  - these are not apples-to-apples replacements for the older `200`-client A/B
    rows because the benchmark methodology changed with the synchronized-start
    fix
  - both runs still ended with a few exhausted-client failures, so the harness
    is stable enough to measure but not yet perfectly clean at `200`
- A cleaner apples-to-apples `200`-client steady-state comparison is now
  recorded too:
  - actor off: `24.861 ms` tick avg, `5.732 ms` lag avg,
    `9.270 ms` player environment avg
  - actor on: `25.683 ms` tick avg, `6.855 ms` lag avg,
    `9.595 ms` player environment avg
  - both used the synchronized-start harness, a `10 s` steady-state capture
    window, and a short `40 s` hold
  - both completed with zero client failures
- We now also have the direct actor-on old-vs-new environment comparison on
  that same `200`-client steady-state shape:
  - initial short-window scrape was misleading because it read a `1m` metric
    after only `10 s` of steady state
  - corrected long-window actor-on comparison:
    old env path with cache disabled: `247.922 ms` tick avg,
    `200.790 ms` lag avg, `110.521 ms` player environment avg
  - corrected long-window actor-on comparison:
    new env path with cache enabled: `97.240 ms` tick avg,
    `20.004 ms` lag avg, `35.173 ms` player environment avg
  - direct `10 s` average of the live latest tick during that corrected run:
    about `249.392 ms` old env vs `86.090 ms` new env
  - that isolates the environment optimization itself at roughly `61%` lower
    total tick average and `68%` lower `player_environment` phase cost with
    actor on
  - the temporary old-env benchmark control has been removed from the code and
    launcher again; the cached environment path is now the only supported path
- A fresh `200`-client actor-on retest on the fully movement-owned path now
  shows:
  - short `10 s` steady-state capture with `201` sessions / `201` active
    players on average
  - `99.436 ms` average latest tick and `7.520 ms` average latest
    `player_environment`
  - only `5.6` players scanned per environment tick on average
  - `0.209 ms` average latest environment-flags subphase
  - `6.997 ms` average latest movement-mutex hold time
  - this strongly suggests environment checking is no longer the dominant
    scaling term and the next bottleneck has moved toward the movement apply
    path itself
- Long-hold `200`-client runs can still end with
  `STATUS_ACCESS_VIOLATION (0xc0000005)` in `world-load-test`, so short
  steady-state captures are currently the more trustworthy benchmark shape for
  this slice.

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
- `target\release\world-load-test.exe --client-count 120 --hold-seconds 0 --move-interval-ms 50 --login-stagger-ms 1`
  passed.
- `target\release\world-load-test.exe --client-count 120 --hold-seconds 15 --move-interval-ms 500 --login-stagger-ms 1`
  passed.
- `target\release\world-load-test.exe --client-count 120 --hold-seconds 15 --move-interval-ms 50 --login-stagger-ms 1`
  passed after the synchronized-start fix.
- `target\release\world-load-test.exe --client-count 200 --hold-seconds 15 --move-interval-ms 50 --login-stagger-ms 1`
  passed after the synchronized-start fix.
- `powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\start-thin-client-load.ps1 -ClientCount 200 -HoldSeconds 40 -MoveIntervalMs 50 -LoginStaggerMs 1`
  completed with zero client failures.
- `powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\start-thin-client-load.ps1 -ClientCount 200 -HoldSeconds 40 -MoveIntervalMs 50 -LoginStaggerMs 1 -EnableMovementActor`
  completed with zero client failures.
- `cargo test -p wow-network map_runtime_player_environment --lib` passed.
- `cargo test -p wow-network map_runtime_add_player_refreshes_environment_cache_on_login --lib`
  passed.
- `cargo test -p wow-network map_runtime_update_player_position_refreshes_environment_cache --lib`
  passed.
- `cargo test -p wow-network map_runtime_set_player_position_refreshes_environment_cache --lib`
  passed.
- `cargo test -p wow-network map_runtime_underwater_breath_timer_applies_drowning_damage_and_log --lib`
  passed.
- `cargo test -p wow-network map_runtime_magma_environmental_timer_applies_lava_damage_without_client_timer --lib`
  passed.
- `cargo build --release -p worldserver` passed.
- `cargo build --release -p world-load-test` passed.
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
- If the thin-client harness remains part of the long-term perf workflow,
  investigate the remaining `world-load-test` end-of-run
  `STATUS_ACCESS_VIOLATION (0xc0000005)` on longer `200`-client runs.
- Decide whether the movement actor proxy remains an experiment or becomes the
  stepping stone toward a fuller map-owned actor model.
- Now that `player_environment` is mostly out of the way, profile the movement
  apply path itself, especially actor reply latency and `MapRuntime` mutex hold
  time, before attempting the next batching architecture.
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
