# Session Handoff

Short operating brief for the next Rust migration session. Keep this pruned;
durable roadmap details belong in `docs/rust_migration_plan.md`, gate status in
`docs/playable_gate_board.md`, and branch split details in
`docs/playable_execution_roadmap.md`.

## Current Branch And Worktree

- Branch: `codex/server-observability-foundation`.
- Current state: uncommitted observability foundation changes are present.
- Purpose: add a local server performance monitoring surface that CMaNGOS did
  not have, starting with Prometheus-compatible `/metrics` output for
  worldserver loop, session, packet, map population, and DB query pressure.
- Base branch: `codex/rusty-mangos`.
- Re-run `git status --short --branch` before editing.
- Live client stack was rebuilt/restarted after this slice:
  - current live stack is running release binaries for performance comparison;
  - authserver PID `47144` on `127.0.0.1:13724`;
  - worldserver PID `51528` on `127.0.0.1:18085`;
  - metrics endpoint: `http://127.0.0.1:9091/metrics`;
  - logs: `auth-client-13724-release.log`, `world-client-18085-release.log`;
  - auto-restart is disabled.

## Current Goal

Current milestone remains **Checkpoint 2 Northshire Human Warrior playable
slice with shared multiplayer state**, but the active user-directed side quest
is now **server observability**: build performance monitoring alongside the
server without adding or maintaining a Northshire grading harness.

Important scope rule: stay focused on the current goal, but use judgment. Fix
blockers and safety/data-integrity guardrails when practical. Log useful
follow-ups when they should not be handled immediately.

Gameplay data rule: do not fake or hardcode gameplay values for parity work.
Use DB data, DBC/source-derived values, or CMaNGOS formulas. If the real data
source is not wired yet, leave behavior unimplemented or narrowly guarded and
log the follow-up.

## Recently Changed

- Added `ObservabilityConfig` to `wow-config`.
- `worldserver` starts a localhost-only observability endpoint when enabled.
- `config/worldserver.local.toml` explicitly enables:
  - `127.0.0.1:9091`;
  - `/metrics` for Prometheus-compatible text;
  - `/healthz` for a simple liveness probe.
- Added `wow-network::observability`, a dependency-light in-process metrics
  registry and local HTTP endpoint.
- Map runtime update loop now records:
  - `wow_map_ticks_total`;
  - `wow_map_tick_duration_seconds`;
  - `wow_map_tick_duration_average_milliseconds`;
  - `wow_map_tick_duration_latest_milliseconds`;
  - `wow_map_tick_duration_max_milliseconds`;
  - `wow_map_tick_lag_seconds`;
  - `wow_map_tick_lag_average_milliseconds`;
  - `wow_map_tick_lag_latest_milliseconds`;
  - `wow_map_tick_lag_max_milliseconds`;
  - `wow_map_tick_over_budget_total`;
  - `wow_map_tick_errors_total`.
- Map update phase timing now records bounded phase gauges:
  - `wow_map_phase_duration_average_milliseconds{phase="idle_motion"}`;
  - `wow_map_phase_duration_latest_milliseconds{phase="idle_motion"}`;
  - `wow_map_phase_duration_max_milliseconds{phase="idle_motion"}`;
  - same metric families for `idle_motion_dispatch`, `player_regen`,
    `player_regen_dispatch`, `aura_expiration`, and
    `aura_expiration_dispatch`.
- World session/socket paths now record:
  - connected/authenticated world sessions;
  - total registered/unregistered sessions;
  - packets in/out by opcode;
  - unhandled authenticated opcodes by opcode.
- `MapRuntime` now exposes a read-only observability snapshot sampled by
  `MapRuntimeManager` once per world tick. `/metrics` includes per-map,
  per-instance gauges for:
  - `wow_map_active_players`;
  - `wow_map_active_creatures`;
  - `wow_map_active_gameobjects`;
  - `wow_map_loaded_grids`;
  - `wow_map_loaded_creature_grids`;
  - `wow_map_loaded_gameobject_grids`;
  - `wow_map_loaded_player_corpse_grids`;
  - `wow_map_active_creature_combats`;
  - `wow_map_corpses`.
- Added `wow-db::observability` with scoped DB query timers. `/metrics` now
  appends:
  - `wow_db_query_total{family="..."}`;
  - `wow_db_query_duration_average_milliseconds{family="..."}`;
  - `wow_db_query_duration_latest_milliseconds{family="..."}`;
  - `wow_db_query_duration_max_milliseconds{family="..."}`.
- Initial DB timing families cover auth account checks, character list/login
  state, tutorial/spell/action/skill/quest/reputation/corpse helpers, world
  template/quest/vendor/trainer/loot helpers, grid spawn loads, respawn loads,
  and waypoint path loads. Labels are bounded static family names, not SQL or
  object IDs.

## Tests Run

- Baseline before edits: `.\scripts\test-rust.cmd` passed.
- `cargo fmt`
- `cargo test -p wow-config observability` passed.
- `cargo test -p wow-network observability` passed.
- `cargo test -p worldserver` passed.
- First post-change `.\scripts\test-rust.cmd` reached checks/tests but failed
  the final `authserver` build because an old running authserver process locked
  `target\debug\authserver.exe` on Windows.
- Stopped old authserver/worldserver processes, then reran
  `.\scripts\test-rust.cmd`; it passed.
- `.\scripts\run-client-stack-18085.cmd -NoAutoRestart` restarted the client
  stack successfully.
- `Invoke-WebRequest http://127.0.0.1:9091/metrics` confirmed live metrics:
  map tick counters/histograms were increasing and over-budget/error counters
  were zero before a real client connected.
- Added direct average millisecond gauges for the browser-readable map tick
  duration and map tick lag values.
- `cargo fmt` and `cargo test -p wow-network observability` passed for the
  average millisecond gauge update.
- Restarted the local client stack again after the average gauge update and
  confirmed `/metrics` shows `wow_map_tick_duration_average_milliseconds` and
  `wow_map_tick_lag_average_milliseconds`.
- Added direct latest/max millisecond gauges for map tick duration and lag.
- `cargo fmt`, `cargo test -p wow-network observability`, and
  `.\scripts\test-rust.cmd` passed for the latest/max gauge update.
- Restarted the local client stack after the latest/max gauge update and
  confirmed `/metrics` shows `wow_map_tick_duration_latest_milliseconds`,
  `wow_map_tick_duration_max_milliseconds`,
  `wow_map_tick_lag_latest_milliseconds`, and
  `wow_map_tick_lag_max_milliseconds`.
- Built release binaries with `cargo build --release -p authserver -p
  worldserver`, stopped the debug stack, and started `target\release`
  authserver/worldserver manually on the same local ports.
- Idle dev-vs-release comparison from `/metrics`:
  - debug idle tick duration average was about `0.020ms`, max about `0.093ms`;
  - release idle tick duration average was about `0.004ms`, max about `0.013ms`;
  - lag average stayed around `9ms` in both builds, suggesting the lag metric is
    mostly OS/runtime wake scheduling at idle rather than game-loop work.
- Added map update phase duration metrics so future whole-tick spikes can be
  attributed to idle motion, player regen, aura expiration, or packet dispatch.
- `cargo fmt`, `cargo test -p wow-network observability`, and
  `.\scripts\test-rust.cmd` passed for the phase timing update.
- Rebuilt release binaries, restarted `target\release` authserver/worldserver,
  and confirmed `/metrics` shows `wow_map_phase_duration_*_milliseconds`
  families.
- Added per-map runtime gauges and DB query-family timing.
- `cargo fmt`
- `cargo test -p wow-db observability` passed.
- `cargo test -p wow-network observability` passed.
- Stopped the release stack, then `.\scripts\test-rust.cmd` passed.
- `cargo build --release -p authserver -p worldserver` passed.
- Restarted release `authserver`/`worldserver` manually and confirmed
  `/metrics` includes:
  - `wow_map_active_players` / `wow_map_active_creatures`;
  - `wow_db_query_total`;
  - `wow_db_query_duration_average_milliseconds`.
- After a small respawn-timer cleanup, reran `cargo fmt`,
  `cargo test -p wow-db observability`, rebuilt release binaries, and restarted
  the release stack.

## Known Follow-Ups

- Add combat/spell/loot/quest counters after the metric naming conventions
  settle.
- Add a lightweight Grafana/Prometheus setup later; this slice intentionally
  only makes the server emit trustworthy measurements.
- Consider a bounded metric reset/test hook if future tests need stronger
  assertions than checking rendered metric families.
- Consider a simple browser dashboard on top of `/metrics`; raw Prometheus text
  is useful but not friendly once the metric list grows.
- Existing playable follow-ups still apply: quest eligibility, quest item
  drops, gameobject quest pickup, broader warrior spells, combat log feedback,
  regen/rage, skill/weapon-skill polish, aggro/chase/leash parity, and patrol
  stability.

## Key Files

- `crates/wow-network/src/observability.rs`
- `crates/wow-db/src/observability.rs`
- `crates/wow-db/src/account.rs`
- `crates/wow-db/src/character/queries.rs`
- `crates/wow-db/src/character/state.rs`
- `crates/wow-db/src/world_data.rs`
- `bins/worldserver/src/main.rs`
- `crates/wow-config/src/lib.rs`
- `config/worldserver.local.toml`
- `crates/wow-network/src/world/server/map_update.rs`
- `crates/wow-network/src/world/maps/map.rs`
- `crates/wow-network/src/world/maps/map_manager.rs`
- `crates/wow-network/src/world/session.rs`
- `crates/wow-network/src/world/wire.rs`
- `crates/wow-network/src/world/server/session_loop.rs`
