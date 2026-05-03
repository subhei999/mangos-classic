# Session Handoff

Short operating brief for the next Rust migration session. Keep this pruned;
durable roadmap details belong in `docs/rust_migration_plan.md`, gate status in
`docs/playable_gate_board.md`, and branch split details in
`docs/playable_execution_roadmap.md`.

## Current Branch And Worktree

- Branch: `codex/c2-idle-motion-scheduler`.
- Base branch: `codex/rusty-mangos`.
- Current state: uncommitted G9/G12 idle-motion scheduler fix is present, plus
  a small observability dashboard loop-average plot and a CMaNGOS-shaped world
  config tick-rate knob.
- Live client stack is currently running release binaries:
  - authserver PID `43224` from `target\release\authserver.exe` on
    `127.0.0.1:13724`;
  - worldserver PID `41968` from `target\release\worldserver.exe` on
    `127.0.0.1:18085`;
  - observability is on `127.0.0.1:9091`;
  - logs: `auth-client-13724.log`, `world-client-18085.log`;
  - auto-restart is disabled.

Run `git status --short --branch` before editing.

## Current Goal

Current milestone remains **Checkpoint 2 Northshire Human Warrior playable
slice with shared multiplayer state**. The active user-directed task is
**patrol/runtime stability and CMaNGOS-like idle motion**, especially making
random walk and waypoint patrol starts consistent while moving away from the
old global start-budget queue toward creature-owned due timers.

Important scope rule: stay focused on the current goal, but use judgment. Fix
blockers and safety/data-integrity guardrails when practical. Log useful
follow-ups when they should not be handled immediately.

Gameplay data rule: do not fake or hardcode gameplay values for parity work.
Use DB data, DBC/source-derived values, or CMaNGOS formulas. If the real data
source is not wired yet, leave behavior unimplemented or narrowly guarded and
log the follow-up.

## Recently Changed

- Restored the temporary idle-motion start cap experiment:
  `DB_CREATURE_IDLE_MOTION_STARTS_PER_TICK = usize::MAX`, because the next
  architecture direction is per-creature motion ownership instead of a shared
  capped start queue.
- Added `MapRuntime::next_idle_motion_start_check_at`, so idle-motion starts
  are scheduled from the next due creature timer instead of scanning ready
  starts every 100ms when all active creatures are still waiting.
- The shared `MapRuntime` idle-motion start path no longer applies
  `take(DB_CREATURE_IDLE_MOTION_STARTS_PER_TICK)`. The legacy constant remains
  set to `usize::MAX`, but live shared runtime motion now follows creature due
  timers.
- Player add/move, loaded creature-grid insertion, and creature respawn now
  invalidate the idle-motion start schedule so newly active due creatures wake
  the start scanner immediately.
- Added first idle-grid unload/eviction behavior:
  - ordinary future random/waypoint idle-motion timers no longer permanently
    block grid unload;
  - idle grids are no longer scanned for moving idle-motion advancement;
  - grids with no nearby player interest and no combat/loot/corpse/respawn or
    active motion blocker unload after the CMaNGOS `MIN_GRID_DELAY` shape
    (`60_000ms`);
  - unload removes the grid from loaded creature/gameobject/corpse grid sets and
    evicts static runtime objects from the shared map, so map-runtime creature
    and grid counters can recover after players leave an area.
- Added a CMaNGOS-shaped 500ms retry delay for failed DB-creature idle-motion
  starts, based on the non-player `RandomMovementGenerator` path-failure retry
  cadence.
- Random walk start failures now defer `next_random_move_at` instead of staying
  immediately eligible and repeatedly consuming the limited start budget.
- Waypoint start path failures now defer `next_waypoint_move_at` instead of
  retrying every 100ms map tick.
- Waypoint creatures whose next node is already at their current position now
  treat that as a node arrival: update current position, advance
  `waypoint_next_index`, apply the DB node wait time, and stay idle until the
  next node is due. This directly addresses Northshire Guard-style paths where
  the first waypoint can equal the spawn position and wait 60 seconds.
- Added regression coverage proving:
  - zero-distance waypoint arrival waits and then starts the next node;
  - failed random path starts are backed off;
  - a zero-distance waypoint candidate no longer blocks other unbounded
    MapRuntime idle-motion starts;
  - a newly loaded due creature wakes a previously sleeping idle-motion start
    schedule;
  - future idle-motion timers do not unload-lock inactive grids;
  - expired idle grids evict their static creatures and grid index;
  - nearby player interest prevents idle grid unload.
- Added an observability dashboard `Loop Avg 10s` plot with its own autoscale,
  computed client-side from the already-polled latest map loop duration
  samples.
- Added `config/worldserver.toml` and wired `[world].MapUpdateInterval = 100`
  from the world config into the shared map update loop and the legacy
  per-session world tick deadline logic. The Rust-style
  `map_update_interval_ms` and old `update_interval_ms` names are accepted as
  aliases, but the visible config key matches CMaNGOS naming.

## Tests Run

- `cargo fmt` passed.
- `cargo test -p wow-network db_creature_waypoint_motion --lib` passed.
- `cargo test -p wow-network db_creature_random_motion --lib` passed.
- `cargo test -p wow-network map_runtime_idle_motion --lib` passed.
- After per-creature due scheduling:
  - `cargo fmt` passed.
  - `cargo test -p wow-network map_runtime_idle_motion --lib` passed.
  - `cargo test -p wow-network db_creature_waypoint_motion --lib` passed.
  - `cargo test -p wow-network db_creature_random_motion --lib` passed.
  - `.\scripts\test-rust.cmd` passed.
- After idle-grid unload:
  - `cargo fmt` passed.
  - `cargo test -p wow-network map_runtime_grid --lib` passed.
  - `cargo test -p wow-network map_runtime_idle_motion --lib` passed.
  - `cargo test -p wow-network map_runtime_lazy --lib` passed.
  - `cargo test -p wow-network map_runtime_expired_idle_grid --lib` passed.
  - `cargo test -p wow-network map_runtime_player_interest_prevents_idle_grid_unload --lib` passed.
  - `.\scripts\test-rust.cmd` passed.
- First `.\scripts\test-rust.cmd` run passed tests/checks but failed final
  `authserver` rebuild because the running debug authserver locked
  `target\debug\authserver.exe` on Windows.
- Stopped the old authserver/worldserver processes and reran
  `.\scripts\test-rust.cmd`; it passed.
- `.\scripts\run-client-stack-18085.cmd -NoAutoRestart` rebuilt/restarted the
  local client stack.
- Verified authserver is listening on `127.0.0.1:13724` and worldserver is
  listening on `127.0.0.1:18085`.
- After per-creature due scheduling, restarted the local client stack and
  verified authserver/worldserver were listening on the same ports.
- After idle-grid unload, restarted the local client stack again and verified
  authserver/worldserver are listening on the same ports.
- After the dashboard plot:
  - `cargo fmt` passed.
  - `cargo test -p wow-network observability --lib` passed.
  - `.\scripts\run-client-stack-18085.cmd -NoAutoRestart` rebuilt/restarted
    the local debug client stack.
  - Verified authserver listens on `127.0.0.1:13724`, worldserver listens on
    `127.0.0.1:18085`, observability listens on `127.0.0.1:9091`, and
    `/dashboard` contains the `loopAvg10sChart` markup.
- Then switched the local client stack from debug to release binaries:
  - `cargo build --release -p authserver -p worldserver` passed.
  - Stopped the debug authserver/worldserver pair.
  - Started `target\release\authserver.exe` and `target\release\worldserver.exe`
    with the same local configs and ports.
  - Verified listeners on `127.0.0.1:13724`, `127.0.0.1:18085`, and
    `127.0.0.1:9091`.
  - Verified `/dashboard` returns 200 and still contains `loopAvg10sChart`.
- After the world tick config:
  - `cargo fmt` passed.
  - `cargo test -p wow-config` passed.
  - `cargo test -p wow-network world_tick --lib` passed.
  - `cargo test -p wow-network map_runtime_idle_motion_tick --lib` passed.
  - `cargo build -p worldserver` passed.
  - `.\scripts\test-rust.cmd` passed.
  - Stopped the previous release authserver/worldserver, rebuilt with
    `cargo build --release -p authserver -p worldserver`, and restarted release
    binaries on the same local ports.
  - Verified the worldserver log reports `map_update_interval_ms=100`,
    listeners are up on `127.0.0.1:13724`, `127.0.0.1:18085`, and
    `127.0.0.1:9091`, and `/dashboard` returns 200.

## Known Follow-Ups

- Real-client smoke Northshire random walkers and Northshire Guard patrols with
  the temporary unbounded legacy constant restored and the shared runtime using
  creature due timers.
- Confirm in the dashboard that map-runtime active creature/grid counters and
  idle-motion phase time recover after leaving an area for at least 60 seconds.
- Watch `/metrics` or the dashboard, especially the new `Loop Avg 10s` plot,
  while moving through dense creature areas to measure idle-motion phase
  duration under the unbounded-cap experiment.
- Investigate broader CMaNGOS parity for per-creature movement generators and
  movement-owner scheduling if capped starts still feel visibly delayed in very
  dense areas.
- Existing playable follow-ups still apply: quest eligibility, quest item
  drops, gameobject quest pickup, broader warrior spells, combat log feedback,
  regen/rage, skill/weapon-skill polish, aggro/chase/leash parity, and patrol
  stability.

## Key Files

- `crates/wow-network/src/world/opcodes.rs`
- `crates/wow-network/src/world/combat/motion.rs`
- `crates/wow-network/src/world/maps/map/creature_motion.rs`
- `crates/wow-network/src/observability.rs`
- `crates/wow-config/src/lib.rs`
- `config/worldserver.toml`
- `config/worldserver.local.toml`
- `crates/wow-network/src/world/tests.rs`
- `docs/playable_gate_board.md`
