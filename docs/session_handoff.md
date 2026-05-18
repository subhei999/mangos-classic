# Session Handoff

Short operating brief for the next Rust migration session. Durable roadmap
history belongs in `docs/rust_migration_plan.md`, gate status in
`docs/playable_gate_board.md`, and detailed benchmark history in
`docs/performance_movement_benchmark.md`.

## Current Branch And State

- Branch: `codex/rusty-mangos`
- Workspace: `C:\Users\subhe\Documents\New project`
- The worktree is intentionally dirty with local perf investigation changes in:
  - thin-client harness / restart script
  - idle-motion scheduling and metrics
  - native mmap timing instrumentation
  - related tests and benchmark docs
- Playerbots remain disabled for normal local testing in
  `config/worldserver.local.toml`.

## Current Goal

Use the thin-client benchmark as the baseline for session-loop starvation work.

Immediate next target:
- continue moving shared combat/world maintenance off the per-session combat
  tick and into map-owned ticking
- measure whether the remaining bad `combat_tick` time is concentrated in:
  - active DB-creature attack processing
  - disconnected-player expiry
- keep packet-latency/session-loop observability as the truth source while
  doing these ownership moves

## What Is Proven

- `player_environment` used to be a major bottleneck and is now much cheaper.
  The movement-owned cached environment path is a real win and should stay.
- Sparse `1000` proved packet fanout is not the only scaling problem.
- No-playerbot benchmark tax is removed.
- Idle-motion breadth was reduced from broad rediscovery scans to tracked
  active/due sets and due heaps.
- Native mmap timing is now instrumented and shows the Detour query itself is
  cheap:
  - query alloc/init, `findPath`, and `findSmoothPath` are all tiny per call
  - the remaining cost is above the FFI layer, not Rust->C++ overhead
- Pending DB movement scripts are not the hidden idle-motion bottleneck:
  - `motion_script_schedule_time` is about `0.001-0.002 ms`
  - `pending_script_execution_time` is about `0.014-0.021 ms`
- Replacing whole idle-motion start-schedule invalidation on player movement
  with incremental nearby-creature resync was a real win:
  - `start_schedule_rebuild_time` dropped from roughly `9-13 ms` to under
    `1 ms`
  - matched sparse `1000` full-load samples also lowered `idle_motion`
    materially
- Remaining dominant idle-motion start cost is still
  `motion_start_path_build_time`, roughly `14-21 ms` in the live sparse `1000`
  window
- The old map-phase view had real blind spots:
  - `dynamic_objects`, `player_channels`, `db_creature_auras`, and
    `player_death_presentation` were still being collapsed into the wrong phase
    buckets
  - `refresh_static_game_event_spawns` and `record_observability_snapshots`
    were included in tick time but not labeled as their own phases
  - authenticated world packet handler latency was not recorded at all, so
    real-player spell/action stalls could look much worse than the map tick
    numbers implied
- The next layer of packet-latency observability is now wired:
  - `wow_world_packet_dispatch_delay_*{opcode="0x...."}`
  - `wow_world_packet_handler_duration_*{opcode="0x...."}`
  - `wow_world_packet_service_time_*{opcode="0x...."}`
  - `wow_world_packet_outbound_queue_latency_*{opcode="0x...."}`
  - `wow_world_packet_write_duration_*{opcode="0x...."}`
- The remaining starvation blind spot is now instrumented in source:
  - `wow_world_session_loop_phase_duration_*{phase="refresh_active_player_cache|finalize_player_death|pending_spell_completion|packet_dispatch|sync_gameplay_state|combat_tick|loot_roll_timeouts|packet_branch_total|timeout_branch_total"}`
  - this should reveal whether packets are sitting unread because the session
    loop is busy in combat ticks / sync work rather than in the cast handler
  - the live `500` run that showed `3s+` spell feel was still on the older
    binary and did not expose these new metrics yet
- Live session-loop metrics now show the real starvation source:
  - `CMSG_CAST_SPELL` service time is tens of milliseconds, not seconds
  - outbound queue/write latency is negligible
  - `combat_tick` and `packet_branch_total` can spike into the multi-hundred
    millisecond to multi-second range
- DB-creature lifecycle is now map-owned:
  - corpse expiry / respawn due work is scheduled in `MapRuntime` via due-at
  maps and heaps instead of per-session loaded-creature scans
  - the map update loop now advances lifecycle once per map tick, dispatches
  lifecycle packets, and persists respawn clears outside the map owner
  - the session loop no longer drives DB-creature lifecycle from
    `handle_combat_tick(...)`
- OOC EventAI spell ticking is now map-owned:
  - session/viewer-side nearby-creature OOC scans were removed from
    `handle_combat_tick(...)`
  - `MapRuntime` now tracks OOC-capable creatures and due times, resolves
    cached EventAI capabilities by creature entry, advances starts/completions
    once per map tick, and emits addressed packets outside the map owner
  - focused tests prove the OOC tick runs without a viewer session and still
    dispatches `SMSG_SPELL_START` / `SMSG_SPELL_GO` to nearby players
- Active DB-creature attack processing is now collapsed into one map-manager
  transaction per victim:
  - session-side broad victim combat scans were replaced with map-owned victim
    indexes and due scheduling
  - the session loop no longer walks the full shared-world creature attack
    state machine attacker by attacker through repeated `maps.*` calls
  - the manager path now advances motion, spell completion/start, facing/range
    checks, melee outcomes, and combat-clear transitions once, then returns a
    compact event bundle for tiny session-local follow-up
  - focused tests prove the new path advances active creature combat without
    the old session-owned per-attacker loop

## Latest Read

Most useful recent live evidence came from the restarted `500`-player sparse
run:

- map tick avg `1m`: about `77 ms`
- `CMSG_CAST_SPELL` service avg `1m`: about `74 ms`
- outbound queue latency: negligible
- session-loop `combat_tick` avg `1m`: about `165 ms`
- session-loop `combat_tick` max `1m`: about `1383 ms`
- `packet_branch_total` max `1m`: about `1509 ms`

Interpretation:
- the bad feel is not explained by the measured cast handler or socket writes
- packets are waiting to be read because the session loop is busy doing shared
  combat/world work

## Recommended Next Task

Continue the ownership move inside `handle_combat_tick(...)`.

Best next slice:
1. statically split the remaining session-loop `combat_tick` work into:
   - disconnected-player expiry
2. rerun the `500` sparse test and compare:
   - `wow_world_session_loop_phase_duration_*{phase="combat_tick"}`
   - `wow_world_packet_dispatch_delay_*{opcode="0x012E"}`
   - `wow_world_packet_service_time_*{opcode="0x012E"}`
   - whether the new active-creature attack ownership move materially lowers
     session starvation
3. if combat tick is still the dominant session blocker, move
   disconnected-player expiry off the session loop next

Do not spend the next pass on:
- native mmap micro-optimizing
- DB movement script scheduling
- reworking packet writes or outbound queueing

## Tests And Confidence

Most relevant recent verification:
- `cargo fmt`
- `cargo test -p wow-network map_runtime_manager_advances_db_creature_combats_for_victim_without_session_side_loop --lib`
- `cargo test -p wow-network map_runtime_db_creature_combats_clear_by_victim --lib`
- `cargo test -p wow-network active_db_creature_combat_snapshot_uses_mapruntime_without_session_cache --lib`
- `cargo test -p wow-network map_runtime_manager_ooc_event_ai --lib`
- `cargo test -p wow-network map_runtime_db_creature_lifecycle --lib`
- `cargo test -p wow-network prometheus_render_includes_histogram_and_opcode_labels --lib`
- `cargo check -p worldserver`
- `.\scripts\test-rust.cmd`

Confidence:
- high that enemy-spell hot paths are using cached object-manager data rather
  than live DB loads in the warm run
- high that session-loop starvation is real and that `combat_tick` is the main
  remaining blind spot
- medium-high that moving shared combat/world maintenance off the session loop
  is the right next architecture direction

## Known Follow-Ups

- long/larger thin-client runs can still end with
  `world-load-test.exe` `STATUS_ACCESS_VIOLATION (0xc0000005)`
- idle-motion/path-start cost still matters for sparse `1000`, but it is no
  longer the best immediate fix for the player-visible spell/action lag

## Key Files

- `crates/wow-network/src/world/combat/motion.rs`
- `crates/wow-network/src/world/combat/lifecycle.rs`
- `crates/wow-network/src/world/server/map_update.rs`
- `crates/wow-network/src/world/server/session_loop.rs`
- `crates/wow-network/src/world/map_runtime/map/creature_combat.rs`
- `crates/wow-network/src/world/map_runtime/map/creature_lifecycle.rs`
- `crates/wow-network/src/world/map_runtime/map/creature_motion.rs`
- `crates/wow-network/src/world/map_runtime/map/players.rs`
- `crates/wow-network/src/world/map_runtime/map.rs`
- `crates/wow-network/src/observability.rs`
- `crates/wow-network/src/world/tests.rs`
- `docs/performance_movement_benchmark.md`
