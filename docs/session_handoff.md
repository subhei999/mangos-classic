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

Complete a root cause analysis for simulated-player action latency:

- corrected problem statement: at roughly `500` simulated players, player
  actions take multiple seconds to execute
- RCA target: find where action latency accumulates, not just whether CPU is
  high
- first pass should separate session dispatch delay, movement actor/mailbox
  delay, map mutex wait/hold, movement apply cost, map tick lag, visibility /
  idle-motion phases, outbound queue latency, and socket write time

The important live observation is:

- even with OOC EventAI disabled, `1000` scattered clients moving every `50 ms`
  still cause unacceptable lag
- slowing the harness movement interval dramatically improves feel

Earlier controls pointed at the movement packet path itself, not creature idle
motion. The latest WPR/WPA evidence narrows the leading root-cause class
further: movement load creates enough outbound replication/write fanout that
per-session socket writers consume major runtime/CPU capacity and delay
unrelated actions such as spell casts.

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

RCA setup added this session:

- `docs/performance_rca_runbook.md` maps the user's fishbone to current crate
  boundaries, existing metric names, run shapes, jitter matrix, and decision
  rules for identifying the first growing queue or phase.
- `scripts/capture-rca-metrics.ps1` and `.cmd` capture raw Prometheus metrics,
  a filtered RCA summary, git/status metadata, runtime environment, matching
  process command lines, world config snippets, and quick baseline metrics into
  `logs/perf-rca/`.
- Generic channel metrics are now exposed in Prometheus:
  - `wow_channel_queue_age_*{channel=...}`
  - `wow_channel_queue_depth_*{channel=...}`
  - `wow_channel_send_wait_*{channel=...}`
  These are wired for the production action-latency mailboxes:
  `movement_actor`, `world_session_outbound`, and
  `world_session_disconnect`.
- Tokio runtime metrics are now exposed when observability is enabled:
  `wow_tokio_runtime_workers`, `wow_tokio_task_count`,
  `wow_tokio_worker_busy_milliseconds`,
  `wow_tokio_runtime_global_queue_depth`, and, when built with
  `RUSTFLAGS=--cfg tokio_unstable`, task poll duration, local queue depth,
  spawn-blocking queue/thread counts, and cooperative forced-yield counters.
  `scripts/start-thin-client-load.ps1` exposes
  `-EnableTokioUnstableMetrics` for repeatable RCA controls.
- This setup intentionally does not add a new perf crate yet. Existing
  `wow-network` observability already covers the first RCA pass; add new
  metrics only when the runbook's current signals cannot isolate the next
  boundary.

First RCA control run captured:

- Command shape:
  `500` clients, `local_radius`, `MoveIntervalMs=50`,
  `MovePhaseJitterMs=0`, `LoginStaggerMs=1`, `HoldSeconds=90`,
  movement actor enabled.
- Capture files:
  - `logs/perf-rca/20260518-193541-500-local-radius-50ms-jitter0-actor-on.metrics.prom`
  - `logs/perf-rca/20260518-193541-500-local-radius-50ms-jitter0-actor-on.summary.prom`
  - `logs/perf-rca/20260518-193541-500-local-radius-50ms-jitter0-actor-on.metadata.md`
- Harness completed with `clients=500`, `failures=2`,
  `movements_sent=568890`, `packets_drained=5316421`; treat this as usable
  but not perfectly clean.
- Capture window reached `500` connected sessions and roughly `498-500` active
  players.
- First read:
  - multi-second delay is visible on inbound world packet dispatch/service for
    movement-like opcodes
  - outbound queue latency is tiny (`world_session_outbound` queue age average
    `0.036 ms`, max `2.068 ms`)
  - `movement_actor` queue age is non-zero but below the observed client delay
    (average `78.641 ms`, max `247.552 ms`)
  - movement apply itself is small compared with the lag (total average
    `2.057 ms`, max `34.256 ms`)
  - map tick spikes are large (duration max `1080.746 ms`, lag max
    `1059.029 ms`)
  This points the next RCA pass toward session/map scheduling and tick-phase
  spikes before outbound write, not outbound socket backlog and not the small
  per-movement apply subphases alone.

Spell-cast sentinel setup added after the first control:

- `bins/world-load-test` now supports an opt-in self-cast probe:
  - `--sentinel-cast-clients <n>`
  - `--sentinel-cast-spell-id <id>`; default `168` (`Frost Armor Rank 1`)
  - `--sentinel-cast-interval-ms <ms>`; default `5000`
  - `--sentinel-cast-phase-jitter-ms <ms>` to spread sentinel cast starts
  - `--disable-movement` to keep watch/sentinel clients stationary after login
  - `--disable-sentinel-movement` to keep only sentinel clients stationary
    while the remaining load clients keep generating movement pressure
- The harness records `CMSG_CAST_SPELL` to matching `SMSG_CAST_RESULT`
  response latency in the final stdout:
  `casts_sent`, `responses`, `failures`, `pending`, `avg_response_ms`, and
  `max_response_ms`.
- `scripts/start-thin-client-load.ps1` exposes the same options and now also
  exposes character `Race`, `CharacterClass`, and `Gender`, so the sentinel run
  can seed mage clients with `-CharacterClass 8 -SentinelCastSpellId 168`.
- Tiny live smoke passed:
  `2` mage clients, `1` Frost Armor sentinel, `10s` hold, movement actor on.
  Result: `casts_sent=4`, `responses=4`, `failures=1`, `pending=0`,
  `avg_response_ms=129.019`, `max_response_ms=515.492`.
  The one spell failure does not block latency measurement, but a later
  success-only sentinel may need a different self-buff or longer interval.
- User watch group launched after the moving/synchronized first attempt was
  stopped: `5` stationary human mages near Northshire spawn, all self-casting
  Frost Armor with `5000 ms` phase jitter and no movement packets. Current
  harness PID at launch was `63072`; it was stopped before the full control.

Second RCA control with stationary spell sentinels captured:

- Command shape:
  `500` human mage clients, `local_radius`, `MoveIntervalMs=50`,
  `MovePhaseJitterMs=0`, `LoginStaggerMs=1`, `HoldSeconds=90`, movement actor
  enabled, first `5` clients configured as stationary Frost Armor sentinels
  with `SentinelCastIntervalMs=5000`,
  `SentinelCastPhaseJitterMs=5000`, and `DisableSentinelMovement=True`.
- Full-load capture files:
  - `logs/perf-rca/20260518-200247-500-local-radius-50ms-jitter0-actor-on-5-stationary-mage-sentinels.metrics.prom`
  - `logs/perf-rca/20260518-200247-500-local-radius-50ms-jitter0-actor-on-5-stationary-mage-sentinels.summary.prom`
  - `logs/perf-rca/20260518-200247-500-local-radius-50ms-jitter0-actor-on-5-stationary-mage-sentinels.metadata.md`
- Post-run aggregate capture files:
  - `logs/perf-rca/20260518-200440-500-local-radius-50ms-jitter0-actor-on-5-stationary-mage-sentinels-postrun.metrics.prom`
  - `logs/perf-rca/20260518-200440-500-local-radius-50ms-jitter0-actor-on-5-stationary-mage-sentinels-postrun.summary.prom`
  - `logs/perf-rca/20260518-200440-500-local-radius-50ms-jitter0-actor-on-5-stationary-mage-sentinels-postrun.metadata.md`
- Full-load scrape reached `501` connected sessions / `501` active players
  because the user's real client was also connected.
- Full-load `CMSG_CAST_SPELL` (`0x012E`) server timings from the first `15`
  sentinel casts:
  - dispatch delay average `122.445 ms`, max `228.313 ms`
  - handler duration average `403.344 ms`, max `893.445 ms`
  - total service time average `525.792 ms`, max `981.541 ms`
- Post-run aggregate after `82` received spell cast packets:
  - dispatch delay average `124.390 ms`, max `228.313 ms`
  - handler duration average `347.487 ms`, max `907.162 ms`
  - total service time average `471.880 ms`, max `1006.878 ms`
- Queue/tick context from the full-load scrape:
  - `movement_actor` queue age average `69.890 ms`, max `219.645 ms`
  - `world_session_outbound` queue age average `0.034 ms`, max `12.175 ms`
  - map tick duration average `23.997 ms`, max `977.393 ms`
  - map tick lag average `27.537 ms`, max `970.099 ms`
- The load harness exited with `0xc0000005` after the run, so the final
  client-side `sentinel-cast summary` line was not emitted. Treat server-side
  spell opcode metrics as the usable control measurement and the missing
  harness summary as an unproven harness bug.

Harness crash mitigation added:

- Newest Windows crash dump was
  `C:\Users\subhe\AppData\Local\CrashDumps\world-load-test.exe.41676.dmp`.
  Several dumps had the same access-violation shape: faulting read at `0x24`
  from the packet-drain timeout/error classification path.
- `bins/world-load-test` no longer classifies timeout reads by calling
  `anyhow::Error::downcast_ref::<std::io::Error>()` in the hot drain/login/logout
  paths. Packet reads now return a concrete `WorldPacketReadError`, so timeout
  handling is direct and avoids the trait-object downcast path seen in the
  dumps.
- The harness no longer forces client threads onto a `256 KiB` stack by
  default. Default per-client thread stack is now `1024 KiB`, with
  `--client-thread-stack-kb <kb>` exposed for experiments. The PowerShell
  wrapper exposes `-ClientThreadStackKb`.
- Post-fix verification against the existing release server:
  - `500` clients, `20s`, `5` stationary mage sentinels, movement load:
    completed with `failures=0`, `casts_sent=30`, `responses=30`,
    `avg_response_ms=757.459`, `max_response_ms=1387.023`.
  - `500` clients, `90s`, same sentinel shape, `MaxAttempts=1`: completed
    without access violation and printed the sentinel summary; exited through
    normal harness failure handling with `6` client failures. Result:
    `casts_sent=71`, `responses=71`, `avg_response_ms=711.371`,
    `max_response_ms=1230.748`.
  - No newer `world-load-test.exe` crash dump appeared after these patched
    runs. Treat the `0xc0000005` as mitigated unless it reappears under the
    default `MaxAttempts=3` script path.

Runtime-metrics control captured after adding the remaining RCA setup:

- Command shape matched the stationary sentinel control and added
  `-EnableTokioUnstableMetrics`: `500` human mage clients, `local_radius`,
  `MoveIntervalMs=50`, `MovePhaseJitterMs=0`, `LoginStaggerMs=1`,
  `HoldSeconds=90`, movement actor enabled, `5` stationary Frost Armor
  sentinels with `SentinelCastIntervalMs=5000`,
  `SentinelCastPhaseJitterMs=5000`.
- Capture files:
  - `logs/perf-rca/20260518-204613-500-local-radius-50ms-jitter0-actor-on-5-stationary-mage-sentinels-runtime-tokio.metrics.prom`
  - `logs/perf-rca/20260518-204613-500-local-radius-50ms-jitter0-actor-on-5-stationary-mage-sentinels-runtime-tokio.summary.prom`
  - `logs/perf-rca/20260518-204613-500-local-radius-50ms-jitter0-actor-on-5-stationary-mage-sentinels-runtime-tokio.metadata.md`
- Run completed without an access violation and emitted the harness sentinel
  summary: `clients=500`, `failures=5`, `movements_sent=554466`,
  `packets_drained=5227195`; spell sentinel result `casts_sent=89`,
  `responses=89`, `failures=45`, `pending=0`,
  `avg_response_ms=665.147`, `max_response_ms=1393.490`.
- Full-load scrape reached exactly `500` connected sessions and `500` active
  players.
- Runtime health from the scrape:
  - `wow_tokio_runtime_workers=24`, `wow_tokio_task_count=1008`
  - worker busy duration over the latest ~1s interval was `9257.459 ms`
    across all workers, so the runtime was busy but not saturated across
    `24` workers
  - `wow_tokio_runtime_global_queue_depth=0`
  - `wow_tokio_task_poll_duration_milliseconds=0.029`
  - `wow_tokio_spawn_blocking_queue_depth=0`
- Server spell path for `CMSG_CAST_SPELL` (`0x012E`) during the scrape:
  - dispatch delay average `142.706 ms`, max `191.971 ms`
  - handler duration average `668.709 ms`, max `917.284 ms`
  - total service time average `811.418 ms`, max `1030.636 ms`
  - outbound `SMSG_CAST_RESULT` (`0x0130`) queue/write remained tiny:
    queue average `0.019 ms`, max `0.041 ms`; write average `0.011 ms`,
    max `0.014 ms`
- Movement / map context:
  - `movement_actor` queue age average `80.796 ms`, max `416.761 ms`
  - `world_session_outbound` queue age average `0.048 ms`, max `8.101 ms`
  - movement apply total average `1.958 ms`, max `27.935 ms`
  - map tick latest `672.009 ms`, max `860.766 ms`; tick lag latest
    `742.615 ms`, max `839.668 ms`
  - session-loop `packet_dispatch` average `2067.932 ms`, max
    `12211.373 ms`; `packet_branch_total` average `2683.226 ms`, max
    `12371.774 ms`
- Current read: the control captures the spell lag without manual casting.
  The first obvious problem remains session/map scheduling and long
  packet-branch/dispatch phases under movement flood; outbound socket queues,
  spawn-blocking, and Tokio global queue depth are not the current first
  bottleneck.

Core scalability matrix captured:

- New repeatable runner:
  `scripts/run-rca-scalability-matrix.ps1`.
- Core matrix id: `20260518-210239`.
- Matrix output:
  - `logs/perf-rca/matrix-20260518-210239/matrix-results.csv`
  - `logs/perf-rca/matrix-20260518-210239/matrix-analysis.csv`
  - `logs/perf-rca/matrix-20260518-210239/matrix-summary.md`
- Shape:
  - player counts: `50`, `100`, `250`, `500`
  - scenarios per count:
    - `idle-same-grid`
    - `movement-same-grid-sync`
    - `movement-same-grid-jitter250`
    - `movement-spread-sync`
  - all runs used stationary mage sentinels and
    `-EnableTokioUnstableMetrics`.
- Key sentinel spell response averages / maxes:
  - `50` idle: `1.300 ms` avg, `61.780 ms` max
  - `50` same-grid movement sync: `56.262 ms` avg, `152.812 ms` max
  - `50` same-grid movement jitter250: `41.920 ms` avg, `124.061 ms` max
  - `50` spread movement sync: `96.771 ms` avg, `2541.807 ms` max
  - `100` idle: `1.848 ms` avg, `62.107 ms` max
  - `100` same-grid movement sync: `124.034 ms` avg, `252.450 ms` max
  - `100` same-grid movement jitter250: `124.360 ms` avg, `230.526 ms` max
  - `100` spread movement sync: `134.111 ms` avg, `2772.751 ms` max
  - `250` idle: `2.096 ms` avg, `62.852 ms` max
  - `250` same-grid movement sync: `337.648 ms` avg, `544.339 ms` max
  - `250` same-grid movement jitter250: `334.476 ms` avg, `639.148 ms` max
  - `250` spread movement sync: `300.807 ms` avg, `4199.256 ms` max
  - `500` idle: `2.427 ms` avg, `61.617 ms` max
  - `500` same-grid movement sync: `608.477 ms` avg, `1152.261 ms` max
  - `500` same-grid movement jitter250: `634.240 ms` avg, `1256.794 ms` max
  - `500` spread movement sync: `252.782 ms` avg, `3303.231 ms` max
- Matrix interpretation:
  - Idle stays near-zero even at `500` players, so connected session count
    alone is not the root cause.
  - Movement at `50 ms` is the trigger. Same-grid movement response averages
    scale roughly `56 ms -> 124 ms -> 338 ms -> 608 ms` from
    `50 -> 100 -> 250 -> 500` players.
  - `250 ms` movement phase jitter does not help at `100+` players, so this is
    not primarily same-millisecond burst collapse.
  - Spread movement improves the `500` average versus same-grid movement
    (`252.782 ms` vs `608.477 ms`) but still has multi-second tails and still
    degrades badly from idle. That points to both sustained movement-path cost
    and some same-grid/AOI pressure, with sustained path cost first.
  - For the `500` same-grid movement sync scrape, `CMSG_CAST_SPELL` service
    average was `513.990 ms`; dispatch average `129.660 ms`, handler average
    `384.327 ms`. Movement actor queue age max was `269.298 ms`, outbound
    queue max stayed small, and spawn-blocking / Tokio global queue depth
    remained `0`.
- No new `world-load-test.exe` crash dump appeared during the matrix.

Rate-knee follow-up captured:

- Runner preset added: `RateKnee` in
  `scripts/run-rca-scalability-matrix.ps1`.
- Matrix id: `20260518-215725`.
- Output:
  - `logs/perf-rca/matrix-20260518-215725/matrix-results.csv`
  - `logs/perf-rca/matrix-20260518-215725/matrix-analysis.csv`
  - `logs/perf-rca/matrix-20260518-215725/matrix-summary.md`
- Shape: `500` players, same-grid movement, movement actor on, stationary mage
  sentinels, runtime metrics enabled.
- Results:
  - `MoveIntervalMs=250`: sentinel avg `676.117 ms`, max `1217.108 ms`;
    `CMSG_CAST_SPELL` service avg `623.038 ms`; movement apply avg
    `1.754 ms`; movement actor queue age max `219.012 ms`; outbound queue max
    `9.854 ms`; Tokio global queue and spawn-blocking queue both `0`.
  - `MoveIntervalMs=500`: sentinel avg `738.274 ms`, max `1386.574 ms`;
    `CMSG_CAST_SPELL` service avg `542.517 ms`; movement apply avg
    `1.722 ms`; movement actor queue age max `201.248 ms`; outbound queue max
    `2.457 ms`; Tokio global queue and spawn-blocking queue both `0`.
- Interpretation: slowing movement packets from `50 ms` to `250/500 ms` did
  not collapse the lag. The root is no longer best described as simple packet
  rate saturation. It looks more like movement-triggered session/map work or
  lock/scheduling interaction that remains costly once the 500 players are in
  the moving-state path.

Profiling attempt:

- Reproduced the worst case with a longer `500` same-grid `50 ms` movement run
  (`HoldSeconds=210`) and captured matching RCA metrics during the intended
  profile window:
  - `logs/perf-rca/20260518-220807-20260518-220616-wpr-500-same-grid-50ms-during-wpr.metrics.prom`
  - `logs/perf-rca/20260518-220807-20260518-220616-wpr-500-same-grid-50ms-during-wpr.summary.prom`
  - `logs/perf-rca/20260518-220807-20260518-220616-wpr-500-same-grid-50ms-during-wpr.metadata.md`
- The long run reproduced the lag: sentinel avg `745.499 ms`, max
  `1382.070 ms`; metrics window had `CMSG_CAST_SPELL` service avg
  `619.468 ms`, handler avg `533.172 ms`, dispatch avg `86.293 ms`;
  `packet_dispatch` avg `1273.856 ms`, `packet_branch_total` avg
  `1850.037 ms`; movement actor queue age max `413.546 ms`; Tokio global and
  spawn-blocking queues still `0`.
- `cargo flamegraph` / `flamegraph` is now installed in
  `C:\Users\subhe\.cargo\bin`.
- Attaching with `flamegraph --pid <worldserver-pid>` failed because
  `flamegraph` uses `dtrace` for PID attach on Windows, and `dtrace` is not
  installed on this machine.
- Command-mode `flamegraph -- <command>` fell back to the built-in Windows
  `blondie` backend. A non-elevated smoke failed with `NotAnAdmin`, but an
  elevated smoke succeeded and produced:
  `logs/perf-rca/20260518-223922-elevated-flamegraph-smoke.svg`.
- Elevated `flamegraph --pid 62696` still failed because PID attach always
  shells out to `dtrace`; elevation alone does not make attach work without
  installing/enabling Windows DTrace.
- Windows Performance Recorder is installed, but `wpr -start CPU -filemode`
  failed from a non-elevated shell with `0xc5585011` ("Failed to enable the
  policy to profile system performance"), but elevated WPR works.
- First elevated WPR attempt started before the load wrapper restarted the game
  stack and produced an ETL, but the load failed before gameplay
  (`500` failures, `0` movements, `0` packets drained), so treat that trace as
  startup/login noise:
  `logs/perf-rca/20260518-224201-wpr-500-same-grid-50ms-spell-sentinels.etl`.
- Useful elevated WPR profile captured during an already-steady `500` direct
  same-grid `50 ms` movement run with stationary mage spell sentinels:
  - WPR ETL:
    `logs/perf-rca/20260518-224929-wpr-steady-direct-500-same-grid-50ms-spell-sentinels.etl`
    (`14,918,090,752` bytes)
  - paired metrics:
    `logs/perf-rca/20260518-224941-wpr-steady-direct-500-same-grid-50ms-spell-sentinels.summary.prom`
  - post-WPR metrics:
    `logs/perf-rca/20260518-225255-post-wpr-steady-direct-500-same-grid-50ms-spell-sentinels.summary.prom`
- The steady WPR metrics window reached `501` connected sessions and
  `501` active players. Spell opcode `0x012E` had dispatch avg `89.072 ms`,
  handler avg `399.309 ms`, service avg `489.125 ms`, and service max
  `1861.016 ms`. Map tick latest/max were `983.079/1535.937 ms`; map tick lag
  latest/max were `1117.772/1505.345 ms`. Movement apply remained small
  (`1.956 ms` avg, `36.792 ms` max), while movement actor apply-start latency
  averaged `146.502 ms` and reply latency averaged `174.600 ms`.
- The direct load harness process stayed alive past its expected hold window
  and did not flush a final sentinel summary under/after the WPR run, so it was
  stopped manually after post-WPR metrics were captured. Treat this as another
  harness robustness caveat, not as invalidating the paired server metrics.
- WPA stack inspection of the useful ETL found the hottest async task under
  `world_session_writer`, with the hot branch:
  `world_session_writer -> tokio::time::timeout -> TcpStream::poll_write_priv -> std::net::tcp::write -> ws2_32.dll!send -> mswsock.dll!WSPSend`.
  This confirms the profile is showing real outbound socket write work, not
  just timeout/timer overhead.
- WPA stack inspection also found a second confirmed hot branch:
  `worldserver::main -> WorldGeometry::area_entry -> native_map_area_info -> wow_map_area_info -> VMAP::VMapManager2::loadMap`.
  This accounts for roughly `25 s` in view and means movement-driven player
  position status / area discovery is still reaching expensive native terrain
  or vmap lookup/load behavior during the steady load window.
- Current RCA read: movement pressure causes large outbound replication/write
  fanout and repeated terrain/area lookup work. Outbound queue age remains low
  because writers are actively draining work, but that write volume consumes
  scheduler/CPU time and coincides with delayed spell service and map tick
  spikes. The next evidence gaps are per-opcode outbound bytes / write cost /
  recipient fanout plus area-entry/native-vmap lookup counts, timings, and
  cache/load behavior.
- No new `world-load-test.exe` crash dump appeared during the rate-knee or
  profiling-attempt runs.

Attribution metrics added and control rerun:

- Code now emits:
  - `wow_world_packet_outbound_enqueued_bytes_total{opcode}`
  - `wow_world_packet_write_bytes_total{opcode}`
  - `wow_world_outbound_fanout_recipients_*{source,opcode}`
  - `wow_world_position_status_total{result}`
  - `wow_world_geometry_area_entry_*{source}`
  - `wow_world_geometry_wmo_area_*{source}`
  - `wow_world_geometry_area_flag_*{source}`
  - `wow_world_geometry_native_area_info_*{status}`
  - `wow_world_geometry_native_area_flag_*{status}`
  - `wow_world_geometry_lookup_results_total{result}`
- Steady-state capture:
  `logs/perf-rca/20260518-234640-500-same-grid-50ms-5-mage-sentinels-attribution-steady.summary.prom`
- Post-run aggregate capture:
  `logs/perf-rca/20260518-234928-500-same-grid-50ms-5-mage-sentinels-attribution-postrun.summary.prom`
- Harness summary:
  `clients=500`, `failures=5`, `movements_sent=570264`,
  `packets_drained=6130460`; sentinel result `casts_sent=89`,
  `responses=89`, `failures=45`, `avg_response_ms=653.853`,
  `max_response_ms=1160.594`.
- Steady scrape reached `500` connected sessions and `499` active players.
  `CMSG_CAST_SPELL` service average/max over the 1m window were
  `569.275/780.043 ms`.
- Outbound byte attribution at steady scrape:
  - `SMSG_UPDATE_OBJECT` (`0x00A9`) dominated: `77,716,311` queued bytes and
    `77,709,747` written bytes.
  - `MSG_MOVE_HEARTBEAT` (`0x00EE`) was second: `10,196,634` queued bytes and
    `10,191,174` written bytes.
  - Other movement opcodes were much smaller:
    `0x00B5` `2.2 MB`, `0x00DA` `1.9 MB`, `0x00BB` `1.5 MB`,
    `0x00C9` `0.78 MB`.
  - Movement broadcast fanout averaged roughly `153-160` recipients and maxed
    around `224-226`.
- Area lookup attribution at steady scrape:
  - `wow_world_position_status_total{result="attempted"} = 2786`
  - `area_entry` average/max `18.212/355.866 ms`
  - WMO area average/max `17.956/355.860 ms`
  - ADT area-flag average/max `0.252/38.467 ms`
  - native WMO area info `not_found` average/max `17.891/355.859 ms`
  - all resolved area entries went through `area_entry_area_flag_found`, so
    this Northshire control is mostly paying expensive WMO misses before a
    cheap ADT area flag succeeds.

Outbound source attribution added and control rerun:

- Code now also emits:
  - `wow_world_outbound_source_packets_total{source,opcode}`
  - `wow_world_outbound_source_bytes_total{source,opcode}`
- Useful steady scrape:
  `logs/perf-rca/20260519-000158-500-same-grid-50ms-source-attribution-steady2.summary.prom`
- Harness summary:
  `clients=500`, `failures=5`, `movements_sent=560487`,
  `packets_drained=5296001`; sentinel result `casts_sent=89`,
  `responses=89`, `failures=44`, `avg_response_ms=815.632`,
  `max_response_ms=1606.316`.
- Steady scrape reached `500` connected sessions and `500` active players.
  `CMSG_CAST_SPELL` service average/max over the 1m window were
  `455.831/1184.130 ms`.
- Top steady-state source/opcode byte families:
  - `movement_apply` / `0x00EE` (`MSG_MOVE_HEARTBEAT`): `106,284,955`
    bytes
  - `player_visibility_refresh` / `0x00A9` (`SMSG_UPDATE_OBJECT`):
    `56,651,147` bytes
  - `player_add_visibility` / `0x00A9`: `55,911,280` bytes
  - `movement_apply` movement opcodes:
    `0x00BB` `13.3 MB`, `0x00C9` `9.0 MB`, `0x00B5` `8.5 MB`,
    `0x00DA` `8.0 MB`, `0x00B7` `7.2 MB`
- Postrun-minus-steady source deltas show ongoing movement pressure is
  dominated by `movement_apply`, especially `MSG_MOVE_HEARTBEAT`.
  `player_add_visibility` is mostly startup/login visibility cost;
  `player_visibility_refresh` remains the ongoing `SMSG_UPDATE_OBJECT`
  producer.
- Session-writer batching was tried as a fix experiment and rejected:
  - steady scrape:
    `logs/perf-rca/20260519-001018-500-same-grid-50ms-writer-batch-steady.summary.prom`
  - harness result worsened to `avg_response_ms=890.854`,
    `max_response_ms=1618.473`
  - the code path was reverted; keep the source-attribution metrics, but do
    not pursue writer batching as the first fix.

First producer-side movement coalescing experiment:

- Code now coalesces stale observer broadcasts for `movement_apply`
  `MSG_MOVE_HEARTBEAT` (`0x00EE`) to at most once per `100 ms` per mover.
  The server still accepts every movement packet and updates authoritative
  player state. Non-heartbeat movement packets still broadcast immediately.
- New regression test:
  `map_runtime_coalesces_stale_heartbeat_broadcasts_to_observers`.
- Active steady scrape:
  `logs/perf-rca/20260519-003425-500-same-grid-50ms-5-mage-sentinels-heartbeat-coalesce100-active-steady.summary.prom`
- Harness summary:
  `clients=500`, `failures=2`, `movements_sent=575564`,
  `packets_drained=3219278`; sentinel result `casts_sent=89`,
  `responses=89`, `failures=45`, `avg_response_ms=948.112`,
  `max_response_ms=1635.198`.
- The scrape reached `500` connected sessions and `499` active players.
  `CMSG_CAST_SPELL` service average/max were `742.636/1354.926 ms`.
- The intended outbound bucket dropped: `movement_apply` / `0x00EE` was
  `12,995,803` bytes in the active steady scrape, and
  `player_movement_broadcast` fanout for `0x00EE` averaged `87.408`
  recipients. `packets_drained` also fell to `3.2M`.
- Spell latency did not improve, so heartbeat coalescing is a useful volume
  reduction but not the complete root fix. The next evidence gap is inside or
  around `CMSG_CAST_SPELL` service time: map lock wait, spell-handler stages,
  remaining `SMSG_UPDATE_OBJECT` churn, and terrain/area lookup.

VMap tile-load cache guard:

- Static CMaNGOS comparison showed that `TerrainInfo::LoadMapAndVMap` checks
  `IsTileLoaded(map, x, y)` before calling `loadMap(...)`.
- The Rust native bridge did not have that guard in hot height, liquid, area,
  and LOS paths; repeated movement-position status work could reach
  `VMapManager2::loadMap(...)` under the global native bridge mutex.
- Fixed by adding `wow_vmap_ensure_tile_loaded(...)` in
  `crates/wow-network/native/vmap_bridge.cpp` and using it from:
  - `crates/wow-network/native/map_height.cpp`
  - `crates/wow-network/native/vmap_los.cpp`
- Active-polled post-fix control:
  `logs/perf-rca/20260519-010653-500-same-grid-50ms-5-mage-sentinels-vmap-cache-guard-active-steady.summary.prom`
- Harness summary:
  `clients=500`, `failures=20`, `movements_sent=241409`,
  `packets_drained=21814770`; sentinel result `casts_sent=89`,
  `responses=89`, `failures=46`, `avg_response_ms=80.677`,
  `max_response_ms=310.921`.
- The scrape reached `500` connected sessions and `500` active map players.
  `CMSG_CAST_SPELL` service average/max were `245.803/1088.052 ms`.
- The core geometry metric collapsed from about `23 ms` native area-info
  average in the previous control to `0.007 ms` found / `0.002 ms` not-found.
  Movement actor queue age dropped from `90.295 ms` average / `324.114 ms` max
  to `0.589 ms` average / `63.432 ms` max.
- This moves native vmap repeated loading from "secondary hypothesis" to
  "confirmed contributor fixed." The control is not a clean scalability pass
  because of thin-client failures, but it is strong RCA evidence.

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
- PowerShell parser check for `scripts/capture-rca-metrics.ps1`
- `cargo test -p wow-network session_registry_requests_disconnect_when_bounded_queue_is_full --lib`
- Final `.\scripts\test-rust.cmd` after queue-metric wiring
- Control load run:
  `.\scripts\start-thin-client-load.ps1 -ClientCount 500 -SpawnMode local_radius -MoveIntervalMs 50 -MovePhaseJitterMs 0 -LoginStaggerMs 1 -HoldSeconds 90 -EnableMovementActor`
  captured metrics successfully, but harness exited with `2` client failures.
- `cargo test -p world-load-test`
- `cargo check -p world-load-test`
- `cargo run -p world-load-test -- --help`
- `.\scripts\test-rust.cmd`
- Tiny live sentinel smoke:
  `powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\start-thin-client-load.ps1 -ClientCount 2 -HoldSeconds 10 -MoveIntervalMs 500 -LoginStaggerMs 10 -CharacterClass 8 -Race 1 -SentinelCastClients 1 -SentinelCastSpellId 168 -SentinelCastIntervalMs 3000 -EnableMovementActor`
- After adding stationary/desync sentinel options:
  - `cargo test -p world-load-test`
  - `cargo check -p world-load-test`
  - `cargo build --release -p world-load-test`
  - `cargo run -p world-load-test -- --help`
- live launch of stationary mage sentinels:
    `target\release\world-load-test.exe --client-count 5 --hold-seconds 900 --spawn-mode local_radius --center-x -8949 --center-y -132 --center-z 83.5 --radius 6 --move-radius 0 --race 1 --class 8 --sentinel-cast-clients 5 --sentinel-cast-spell-id 168 --sentinel-cast-interval-ms 5000 --sentinel-cast-phase-jitter-ms 5000 --disable-movement`
- Full 500-client stationary-sentinel control:
  `powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\start-thin-client-load.ps1 -ClientCount 500 -SpawnMode local_radius -MoveIntervalMs 50 -MovePhaseJitterMs 0 -LoginStaggerMs 1 -HoldSeconds 90 -CharacterClass 8 -Race 1 -SentinelCastClients 5 -SentinelCastSpellId 168 -SentinelCastIntervalMs 5000 -SentinelCastPhaseJitterMs 5000 -DisableSentinelMovement -EnableMovementActor`
- Metrics capture during the full 500-client stationary-sentinel control:
  `powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\capture-rca-metrics.ps1 -Scenario "500-local-radius-50ms-jitter0-actor-on-5-stationary-mage-sentinels"`
- Post-run metrics capture after the harness crash:
  `powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\capture-rca-metrics.ps1 -Scenario "500-local-radius-50ms-jitter0-actor-on-5-stationary-mage-sentinels-postrun"`
- Harness crash fix verification:
  - `cargo test -p world-load-test`
  - `cargo check -p world-load-test`
  - `cargo build --release -p world-load-test`
  - `cargo run -p world-load-test -- --help`
  - direct `target\release\world-load-test.exe` run with `500` clients,
    `20s`, `5` stationary mage sentinels
  - direct `target\release\world-load-test.exe` run with `500` clients,
    `90s`, `5` stationary mage sentinels, `MaxAttempts=1`
- Runtime metrics / control setup:
  - `cargo check -p worldserver`
  - `cargo test -p wow-network prometheus_render_includes_histogram_and_opcode_labels --lib`
  - PowerShell parser check for `scripts/capture-rca-metrics.ps1` and
    `scripts/start-thin-client-load.ps1`
  - `RUSTFLAGS="--cfg tokio_unstable" cargo check -p worldserver`
  - `cargo check -p world-load-test`
  - full `500`-client stationary-sentinel control with
    `-EnableTokioUnstableMetrics`, captured at steady state
- Scalability matrix:
  - PowerShell parser check for `scripts/run-rca-scalability-matrix.ps1`
  - smoke matrix: `50` clients, movement same-grid sync, `20s` hold
  - core matrix: `50`, `100`, `250`, `500` clients across idle same-grid,
    movement same-grid sync, movement same-grid jitter250, and movement spread
    sync
  - rate-knee matrix: `500` clients, same-grid movement at `250 ms` and
    `500 ms`
  - WPR CPU profile attempt during a long `500` same-grid `50 ms` run; WPR was
    blocked by Windows profiling policy/privilege, but matching RCA metrics
    were captured
- Source-attribution control:
  - `cargo fmt`
  - `cargo check -p worldserver`
  - `cargo test -p wow-network prometheus_render_includes_histogram_and_opcode_labels --lib`
  - full `500`-client stationary-sentinel control with outbound
    source/opcode metrics, captured at steady state and postrun
  - session-writer batching control, captured at steady state; batching was
    reverted after the control worsened spell latency
  - post-revert validation:
    `cargo check -p worldserver` and
    `cargo test -p wow-network prometheus_render_includes_histogram_and_opcode_labels --lib`
  - final `.\scripts\test-rust.cmd`
- Heartbeat coalescing:
  - `cargo test -p wow-network map_runtime_coalesces_stale_heartbeat_broadcasts_to_observers --lib`
  - `cargo check -p worldserver`
  - `cargo test -p wow-network map_runtime_manager_movement_actor_matches_direct_path_packets --lib`
  - `cargo test -p wow-network map_runtime_broadcasts_stop_with_final_idle_orientation --lib`
  - final `.\scripts\test-rust.cmd`
  - active-polled `500`-client stationary-sentinel control captured with
    `-EnableTokioUnstableMetrics`
- VMap tile-load cache guard:
  - `cargo check -p worldserver`
  - `cargo test -p wow-network db_creature_vmap_los_uses_local_cmangos_data_when_available --lib`
  - `cargo test -p wow-network terrain_height_uses_local_cmangos_map_data_when_available --lib`
  - final `.\scripts\test-rust.cmd`
  - `cargo build --release -p authserver -p worldserver -p world-load-test`
  - active-polled `500`-client stationary-sentinel control captured with
    `-EnableTokioUnstableMetrics`

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
  the first `500`-client control still shows multi-second packet
  dispatch/service delay and large map tick spikes.
- High that outbound movement/replication fanout is now the leading root-cause
  class for the captured `500` same-grid spell lag: WPR shows the hottest
  resolved async task in `world_session_writer` doing real socket sends.
- High that terrain/area lookup is a second confirmed contributor in the same
  capture: the second-largest resolved branch goes through
  `WorldGeometry::area_entry`, native `wow_map_area_info`, and
  `VMapManager2::loadMap`.
- High that repeated native vmap tile loading was a real root-cause
  contributor and is now fixed by the cache guard. The post-fix control reduced
  sentinel spell average from the prior comparable `948.112 ms` to
  `80.677 ms`, and reduced native area-info averages from roughly `23 ms` to
  micro-scale millisecond values.
- High that the first attribution pass confirms the concrete split: outbound
  writer volume is dominated by `SMSG_UPDATE_OBJECT`, while position-status
  area discovery is dominated by repeated native WMO area-info misses.
- High that the latest control captures the user's spell-cast lag shape without
  manual casting and includes enough runtime metrics to rule out
  spawn-blocking and Tokio global queue backlog for this specific run.
- High that the scalability trigger is movement pressure, not connected player
  count alone: the `500` idle control stayed at `2.427 ms` average spell
  response while `500` same-grid movement sync rose to `608.477 ms`.
- Medium-high that same-grid AOI contributes to the `500` average, but is not
  the whole root cause because spread movement still has degraded averages and
  multi-second tails.
- High that slower movement intervals alone do not eliminate the lag at `500`
  same-grid players; `250 ms` and `500 ms` movement still averaged
  `676-738 ms` spell responses.
- High that producer-side heartbeat coalescing reduces outbound volume, but it
  is not sufficient to fix spell latency by itself. The latest control dropped
  `movement_apply/0x00EE` traffic and still averaged nearly `1s` spell
  response.
- High that session-writer batching is not the right first fix: the control
  worsened sentinel spell average and did not address producer-side byte
  volume.

## Known Blockers / Unproven Areas

- OOC EventAI remains disabled for isolation, so current live perf runs are not
  exercising that subsystem.
- The new movement coalescing is compile- and test-proven, but not yet
  benchmark-proven under the thin-client harness.
- The first `500`-client control was not perfectly clean: two clients exhausted
  all attempts.
- The latest runtime-metrics control was not perfectly clean: five clients
  exhausted all attempts, but the harness completed normally and emitted the
  sentinel summary.
- The post-vmap-cache-guard control was also not perfectly clean: twenty
  clients exhausted all attempts. The active scrape still reached `500`
  connected sessions / active players and emitted the sentinel summary, so it
  is useful RCA evidence but not a final scalability acceptance run.
- The core matrix has some client failures in movement scenarios. Every row
  reached steady state and emitted sentinel summaries, so the matrix is useful
  for RCA shape, but exact pass/fail cleanliness is not perfect.
- Local CPU profiling is blocked in a non-elevated Windows shell, but elevated
  WPR works and has produced a useful profile. `flamegraph --pid` still needs
  Windows DTrace; command-mode flamegraph works only elevated and only when it
  launches the process itself.
- `scripts/capture-rca-metrics.ps1` metadata fenced-code formatting was fixed
  after the runtime control capture; the capture's raw/summary metrics are
  intact, but that specific metadata file has malformed fences.
- Long thin-client harness runs previously hit `world-load-test.exe`
  `0xc0000005`. The default `MaxAttempts=3` script-run control now completed
  without an access violation; continue watching for new dumps, but the known
  crash shape is mitigated.

## Recommended Next Task

1. Read `docs/performance_rca_runbook.md`.
2. Keep OOC EventAI disabled for the next comparison run.
3. Treat repeated native vmap tile loading as a confirmed contributor that is
   fixed by the current cache guard.
4. Treat outbound replication/write fanout and `player_visibility_refresh`
   update-object churn as the next leading RCA hypothesis. The post-fix control
   still emitted very large movement/update-object packet volume even though
   spell latency improved dramatically.
5. Reduce `player_visibility_refresh`
   `SMSG_UPDATE_OBJECT` churn. Keep `player_add_visibility` separate because
   the source metrics show it is mostly startup/login visibility cost.
6. Add finer `CMSG_CAST_SPELL` handler timing before another broad
   optimization: separate map lock wait, spell validation, map mutation,
   response build, and observer broadcast. The average is much better after
   the vmap fix, but the packet service max still reached about `1.1 s`.
7. Reduce area lookup frequency with a CMaNGOS-shaped rule: recompute on
   meaningful cell/tile/area transitions or cache invalidation, preserving
   exploration and WMO override correctness.
8. Add native vmap/map load/cache hit/miss counts if the bridge can expose
   them cheaply; the guard fixed the repeated-load bug, but counters would make
   future regressions obvious.
9. During each steady-state window, run:
   - `powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\capture-rca-metrics.ps1 -Scenario "<scenario-name>"`
   Use `-EnableTokioUnstableMetrics` on `scripts/start-thin-client-load.ps1`
   when collecting runtime scheduler metrics.
10. If a smaller profile artifact is needed, start `worldserver.exe` under
   elevated `flamegraph`, install/enable Windows DTrace for
   `flamegraph --pid`, or move the same run to Linux and use
   `perf`/`cargo flamegraph`.
11. Keep same-grid AOI/visibility on the suspect list, but the current data now
   says the concrete producer to attack first is `movement_apply`, followed by
   `player_visibility_refresh`.

## Key Files

- `crates/wow-network/src/world/server/session_loop.rs`
- `crates/wow-network/src/world/server/movement.rs`
- `crates/wow-network/src/world/map_runtime/movement_actor.rs`
- `crates/wow-network/src/world/map_runtime/map_manager.rs`
- `crates/wow-network/src/world/map_runtime/map/players.rs`
- `crates/wow-network/src/world/tests.rs`
- `docs/performance_rca_runbook.md`
- `scripts/capture-rca-metrics.ps1`
