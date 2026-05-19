# Session Handoff

Short operating brief for the next Rust migration session. Durable roadmap
history belongs in `docs/rust_migration_plan.md`, gate status in
`docs/playable_gate_board.md`, and benchmark chronology in
`docs/performance_movement_benchmark.md`.

## Current Branch And State

- Branch: `codex/rusty-mangos`
- Workspace: `C:\Users\subhe\Documents\New project`
- Current state is intentionally dirty with uncommitted movement-path work plus
  earlier combat/OOC perf investigation changes.
- Local playerbots remain disabled in `config/worldserver.local.toml`.
- OOC EventAI is temporarily disabled again in
  `crates/wow-network/src/world/server/map_update.rs` for testing isolation.

## Current Goal

Reduce thin-client movement-flood lag by making authenticated movement packet
handling more CMaNGOS-shaped.

The important live observation is:

- even with OOC EventAI disabled, `1000` scattered clients moving every `50 ms`
  still cause unacceptable lag
- slowing the harness movement interval dramatically improves feel

That points at the movement packet path itself, especially
`MapRuntime::update_player_position(...)`, not creature idle motion, as the
current first-order problem.

## What Is Proven

- Disabling the map-owned OOC EventAI phase immediately restored:
  - NPC idle patrol motion
  - mana / health regen
  - other timed map systems
- So OOC EventAI was a real regression source, but it is not the whole
  `1000`-client movement-flood problem.
- The earlier session-loop starvation work is already landed:
  - DB-creature lifecycle is map-owned
  - OOC EventAI scans were removed from `handle_combat_tick(...)`
  - active creature attack processing is collapsed into map-owned victim
    transactions
- A real-client hostile caster hang was reproduced against Burning Blade
  Neophyte (`entry=3196`, combat EventAI `348 = Immolate`) and reduced to an
  async mutex lifetime bug in the manager-owned combat wrapper.
- That deadlock is fixed, and the same bug class was audited/fixed in the
  playerbot manager loops.
- The movement actor only coalesces after a movement packet is already inside
  the movement path; it does not reduce the session-side per-packet work.
- Our current movement handler still does far more inline work than CMaNGOS:
  movement map update, creature/gameobject/corpse visibility rescans, aggro
  start checks, area discovery, and session-to-map gameplay-state sync.

## Latest Change

The movement path is now materially thinner and more map-owned:

- authenticated sessions still coalesce same-session movement bursts for `10 ms`
  in `crates/wow-network/src/world/server/session_loop.rs`
- pure movement packets no longer force an immediate
  `sync_active_player_gameplay_state(...)` after dispatch; sync still happens
  for non-movement packets and on the world-tick path
- `crates/wow-network/src/world/server/movement.rs` no longer starts
  DB-creature aggro inline on every successful move; we now rely on the
  existing once-per-world-tick `handle_combat_tick(...)` aggro path instead
- player area discovery checks are now throttled to `100 ms` via
  `MovementSessionState::next_position_status_update_at`, matching the
  CMaNGOS idea of throttled position-status updates instead of per-packet work
- player-to-player enter/leave visibility diffing and
  `sync_db_creature_idle_motion_tracking_for_player_interest_positions(...)`
  were removed from inline `MapRuntime::update_player_position(...)`
- movement now only marks a dirty player-visibility refresh
- the new map-owned `player_visibility_refresh` phase runs once per map tick,
  batches each player once, updates player-player visibility, and then performs
  the deferred creature-interest sync before idle motion
- the thin-client harness now supports `--move-phase-jitter-ms` for a
  deterministic per-client movement start offset after the shared ready gate;
  this keeps the same per-client interval but avoids all clients sharing the
  same movement phase
- observability now splits the new `player_visibility_refresh` phase into:
  - `wow_player_visibility_refresh_visibility_diff_broadcast_time_*`
  - `wow_player_visibility_refresh_creature_interest_sync_time_*`
  and movement packet ownership already had:
  - `wow_movement_map_mutex_wait_*`
  - `wow_movement_map_mutex_hold_*`
- movement observability now also exposes an explicit pipeline split:
  - actor enqueue -> apply start latency:
    `wow_movement_actor_apply_start_latency_*`
  - per-applied-move counts:
    `wow_movement_apply_observers_notified_*`
    `wow_movement_apply_packets_emitted_*`
  - `MapRuntime::update_player_position(...)` subphases:
    - `wow_movement_apply_observer_snapshot_time_*`
    - `wow_movement_apply_movement_broadcast_time_*`
    - `wow_movement_apply_grid_update_time_*`
    - `wow_movement_apply_player_state_environment_time_*`
    - `wow_movement_apply_fall_damage_broadcast_time_*`
    - `wow_movement_apply_death_presentation_time_*`
    - `wow_movement_apply_visibility_refresh_mark_time_*`
    - `wow_movement_apply_total_time_*`
  - the HTML dashboard now has a **Movement Pipeline** panel for these metrics
- movement packets now take a more aggressive session-loop fast path:
  - they skip pre-dispatch `refresh_active_player_session_cache(...)`
  - they skip pre-dispatch death finalization
  - they skip pre/post pending player spell completion checks unless the
    session already has active spells
  - the main session-loop timeout path also skips map `next_pending_player_spell_cast_due_at(...)`
    lookups unless the session already has active spells
  This is an explicit measurement experiment aimed at reducing movement
  `dispatch/service` cost before we decide whether deeper actor/map-thread
  ownership work is still needed.

Creature/gameobject/corpse visibility streaming from `movement.rs` still
remains inline and distance-gated, so it is the next likely movement-side
effect family to revisit if the harness still lags badly.

## Tests Run

- `cargo fmt`
- `cargo test -p wow-network enqueue_pending_movement_replaces_older_packet --lib`
- `cargo test -p wow-network pending_movement_timeout_uses_coalesce_deadline --lib`
- `cargo test -p wow-network pending_movement_due_only_after_deadline --lib`
- `cargo test -p wow-network player_position_status_update_is_throttled --lib`
- `cargo test -p wow-network movement_packets_skip_immediate_gameplay_sync --lib`
- `cargo test -p wow-network map_runtime_defers_player_visibility_enter_until_refresh_phase --lib`
- `cargo test -p wow-network map_runtime_visibility_refresh_keeps_earliest_old_position_across_multiple_moves --lib`
- `cargo test -p wow-network map_runtime_manager_movement_actor_matches_direct_path_packets --lib`
- `cargo test -p wow-network map_runtime_player_movement_preserves_db_creature_visibility_set --lib`
- `cargo test -p wow-network movement_packets_skip_pre_dispatch_session_refresh --lib`
- `cargo test -p wow-network movement_packets_skip_pending_spell_checks_without_active_spells --lib`
- `cargo test -p wow-network active_spells_keep_pending_spell_checks_enabled_for_movement --lib`
- `cargo test -p wow-network dashboard_renders_live_metrics_page --lib`
- `cargo test -p wow-network prometheus_render_includes_histogram_and_opcode_labels --lib`
- `cargo test -p wow-network map_runtime_manager_movement_actor_matches_direct_path_packets --lib`
- `cargo test -p world-load-test movement_phase_jitter_is_deterministic_and_bounded`
- `cargo check -p worldserver`
- `cargo check -p world-load-test`
- `.\scripts\test-rust.cmd`

## Current Confidence

- High that the old standalone OOC due-queue architecture is no longer the
  best explanation for the remaining `1000`-client movement lag.
- High that movement flood currently causes too much per-packet session-side
  work compared with CMaNGOS.
- High that `update_player_position(...)` was still a major hot path because it
  owned player-visibility diffing and creature-interest sync inline.
- Medium-high that the new map-owned `player_visibility_refresh` phase is a
  correct architecture move.
- Medium that this slice alone materially improves the `1000`-client harness;
  live rerun is still required.

## Known Blockers / Unproven Areas

- OOC EventAI remains disabled for isolation, so current live perf runs are not
  exercising that subsystem.
- The new movement coalescing is compile- and test-proven, but not yet
  benchmark-proven under the thin-client harness.
- Long thin-client harness runs may still hit the existing
  `world-load-test.exe` `0xc0000005` issue.

## Recommended Next Task

1. Rebuild and restart release binaries with OOC EventAI still disabled.
2. Rerun the thin-client harness with OOC EventAI still disabled:
   - `1000` clients
   - `creature_grid_scatter`
   - compare `MoveIntervalMs=50` vs a slower interval
   - compare `MovePhaseJitterMs=0` vs a nonzero phase jitter such as `50`
3. Check whether `player_visibility_refresh` materially reduces session-loop
   and map-tick pressure at `MoveIntervalMs=50`.
4. If movement flood is still the main limiter, the next architecture-correct
   move should be to pull the remaining inline `movement.rs` side effects off
   the packet path, especially:
   - creature visibility streaming
   - gameobject visibility streaming
   - corpse visibility streaming
   - broader packet-queue / map-thread ownership work if those trims are not
     enough

## Key Files

- `crates/wow-network/src/world/server/session_loop.rs`
- `crates/wow-network/src/world/server/movement.rs`
- `crates/wow-network/src/world/map_runtime/movement_actor.rs`
- `crates/wow-network/src/world/map_runtime/map_manager.rs`
- `crates/wow-network/src/world/map_runtime/map/players.rs`
- `crates/wow-network/src/world/tests.rs`
