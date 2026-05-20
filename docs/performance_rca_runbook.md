# Server Action Latency RCA Runbook

This runbook is for the current Rust worldserver performance RCA:

> At roughly 500 simulated players, player actions take multiple seconds to
> execute.

The target is to locate where latency accumulates, not to prove that CPU is
high. For each bad run, separate queue wait from handler/runtime work:

- socket read and packet decode
- session-loop dispatch delay
- movement actor enqueue and apply-start latency
- map mutex wait and hold time
- map tick duration and lag
- movement apply subphases
- player visibility refresh
- idle motion and other map phases
- outbound queue wait and socket write

## Current Crate Map

The top-level workspace members relevant to this RCA are:

- `bins/worldserver`: starts auth/world runtime config and the observability
  endpoint.
- `bins/world-load-test`: thin-client auth/login/world movement harness.
- `crates/wow-network`: packet/session/map runtime, movement, visibility,
  combat, outbound writer, and metrics.
- `crates/wow-db`: SQL access used by auth, world, and character paths.
- `crates/wow-proto`, `crates/wow-common`, `crates/wow-config`: protocol,
  shared types, and config support.

The current performance hot-path files are:

- `crates/wow-network/src/world/server/session_loop.rs`
- `crates/wow-network/src/world/server/movement.rs`
- `crates/wow-network/src/world/map_runtime/movement_actor.rs`
- `crates/wow-network/src/world/map_runtime/map_manager.rs`
- `crates/wow-network/src/world/map_runtime/systems/players.rs`
- `crates/wow-network/src/world/server/map_update.rs`
- `crates/wow-network/src/observability.rs`
- `bins/world-load-test/src/main.rs`
- `scripts/start-thin-client-load.ps1`

## Existing Signals

Do not add new instrumentation until these existing signals have been captured
for the failing shape.

Session and packet pipeline:

- `wow_channel_queue_age_*{channel="world_session_disconnect"}`
- `wow_channel_queue_age_*{channel="world_session_outbound"}`
- `wow_channel_queue_depth_*{channel="world_session_disconnect"}`
- `wow_channel_queue_depth_*{channel="world_session_outbound"}`
- `wow_channel_send_wait_*{channel="world_session_disconnect"}`
- `wow_channel_send_wait_*{channel="world_session_outbound"}`
- `wow_world_packet_dispatch_delay_*`
- `wow_world_packet_handler_duration_*`
- `wow_world_packet_service_time_*`
- `wow_world_session_loop_phase_duration_*`

Movement actor and map ownership:

- `wow_channel_queue_age_*{channel="movement_actor"}`
- `wow_channel_queue_depth_*{channel="movement_actor"}`
- `wow_channel_send_wait_*{channel="movement_actor"}`
- `wow_movement_actor_queue_depth_latest`
- `wow_movement_actor_queue_depth_max`
- `wow_movement_actor_enqueue_latency_*`
- `wow_movement_actor_processing_time_*`
- `wow_movement_actor_reply_latency_*`
- `wow_movement_actor_batch_size_*`
- `wow_movement_actor_apply_start_latency_*`
- `wow_movement_map_mutex_wait_*`
- `wow_movement_map_mutex_hold_*`

Movement apply:

- `wow_movement_apply_total_time_*`
- `wow_movement_apply_observers_notified_*`
- `wow_movement_apply_packets_emitted_*`
- `wow_movement_apply_observer_snapshot_time_*`
- `wow_movement_apply_movement_broadcast_time_*`
- `wow_movement_apply_grid_update_time_*`
- `wow_movement_apply_player_state_environment_time_*`
- `wow_movement_apply_visibility_refresh_mark_time_*`

Map tick and world phases:

- `wow_map_tick_duration_*`
- `wow_map_tick_lag_*`
- `wow_map_phase_duration_*`
- `wow_map_active_players`
- `wow_map_loaded_grids`
- `wow_map_tracked_idle_motion_creatures`
- `wow_map_tracked_idle_motion_start_candidates`

Visibility and idle motion:

- `wow_player_visibility_refresh_*`
- `wow_idle_motion_*`

Outbound:

- `wow_world_packet_outbound_queue_latency_*`
- `wow_world_packet_write_duration_*`
- `wow_world_outbound_queue_depth_latest`
- `wow_world_outbound_queue_depth_max`
- `wow_world_outbound_queue_full_total`

## Current RCA Finding

Latest useful profile:

- WPR ETL:
  `logs/perf-rca/20260518-224929-wpr-steady-direct-500-same-grid-50ms-spell-sentinels.etl`
- paired metrics:
  `logs/perf-rca/20260518-224941-wpr-steady-direct-500-same-grid-50ms-spell-sentinels.summary.prom`

The paired metrics window reached `501` connected sessions / active players.
`CMSG_CAST_SPELL` (`0x012E`) service time averaged `489.125 ms` and maxed at
`1861.016 ms`. Map tick duration and lag also spiked past `1.5 s`.

WPA CPU sampled stacks resolved the hottest async task as:

```text
wow_network::world::server::session_loop::world_session_writer
  -> tokio::time::timeout::Timeout<T>::poll
    -> tokio::net::tcp::TcpStream::poll_write_priv
      -> mio::sys::windows::IoSourceState::do_io
        -> std::net::tcp::write
          -> ws2_32.dll!send
            -> mswsock.dll!WSPSend
```

This means the leading root-cause class is now outbound replication/write
fanout under movement load, not spell-handler code in isolation. Outbound queue
age stays low because session writers are actively draining work, but doing so
consumes enough runtime/CPU capacity to delay unrelated action handling and map
ticks.

The second-largest resolved branch in the same WPA view is also actionable:

```text
worldserver::main
  -> wow_network::world::map_runtime::world_geometry::WorldGeometry::area_entry
    -> wow_network::world::handlers::terrain_height::native_map_area_info
      -> wow_map_area_info
        -> VMAP::VMapManager2::loadMap
```

That branch accounts for roughly `25 s` in view under the sampled profile. This
means player position status / area discovery is still pulling expensive native
terrain or vmap area lookup and map-load behavior into the movement-pressure
path. The current Rust path calls WMO area lookup before falling back to ADT
area flags, so even outdoor movement can pay native WMO lookup unless this is
cached, warmed, or skipped by a CMaNGOS-shaped owner.

Narrowed fishbone status:

- Likely: interest management / replication fanout, outbound packet volume,
  per-session socket writer work, movement update coalescing/drop policy.
- Likely secondary: terrain / area lookup in `WorldGeometry::area_entry`,
  especially native `wow_map_area_info` and `VMapManager2::loadMap` reached
  from position-status updates during movement load.
- Contributing: same-grid AOI/visibility amplification and map tick spikes.
- Mostly ruled out for this run: DB on spell response path, spawn-blocking
  saturation, Tokio global queue backlog, outbound queue backlog, raw movement
  apply cost, and spell logic as the primary isolated bottleneck.

The remaining evidence gaps are:

- map-owned producer-side coalescing proof. We now know which producers and
  opcodes own the byte volume, but still need a fix/control comparison that
  reduces stale movement replication before packets reach session writers.
- native vmap/map cache/load attribution. Rust metrics now show repeated native
  WMO area-info misses, while WPA showed `VMapManager2::loadMap`; expose
  hit/miss/load counts if the bridge can do so cheaply.

Attribution control after adding the first metric pass:

- Capture:
  `logs/perf-rca/20260518-234640-500-same-grid-50ms-5-mage-sentinels-attribution-steady.summary.prom`
- Harness summary:
  `casts_sent=89`, `responses=89`, `avg_response_ms=653.853`,
  `max_response_ms=1160.594`, with `5` client failures after retries.
- Steady scrape reached `500` connected sessions and `499` active map players.
- Top outbound byte families at steady scrape:
  - `0x00A9` (`SMSG_UPDATE_OBJECT`): `77,716,311` queued bytes /
    `77,709,747` written bytes
  - `0x00EE` (`MSG_MOVE_HEARTBEAT`): `10,196,634` queued bytes /
    `10,191,174` written bytes
  - next movement opcodes were much smaller:
    `0x00B5` `2.2 MB`, `0x00DA` `1.9 MB`, `0x00BB` `1.5 MB`
- Movement broadcast fanout averaged roughly `153-160` recipients per sampled
  movement broadcast, with max around `224-226`.
- Position-status attribution at steady scrape:
  - `wow_world_position_status_total{result="attempted"} = 2786`
  - `area_entry` average `18.212 ms`, max `355.866 ms`
  - WMO area lookup average `17.956 ms`
  - ADT area-flag lookup average `0.252 ms`
  - native area info `not_found` average `17.891 ms`, max `355.859 ms`
  - all area entries resolved through ADT area flag after WMO miss:
    `area_entry_area_flag_found = 2772`

Updated RCA read: `SMSG_UPDATE_OBJECT` fanout is the dominant outbound byte
source, and movement-position status is paying a repeated WMO/native lookup
that usually misses before the cheap ADT area-flag lookup succeeds. The next
fix should target both: reduce/coalesce redundant update-object replication
and avoid native WMO area lookup on every throttled outdoor position-status
check.

Source-attribution control after splitting outbound producers:

- Useful steady scrape:
  `logs/perf-rca/20260519-000158-500-same-grid-50ms-source-attribution-steady2.summary.prom`
- Harness summary:
  `casts_sent=89`, `responses=89`, `avg_response_ms=815.632`,
  `max_response_ms=1606.316`, with `5` client failures after retries.
- Top steady-state source/opcode byte families:
  - `movement_apply` / `0x00EE` (`MSG_MOVE_HEARTBEAT`): `106,284,955`
    bytes
  - `player_visibility_refresh` / `0x00A9` (`SMSG_UPDATE_OBJECT`):
    `56,651,147` bytes
  - `player_add_visibility` / `0x00A9`: `55,911,280` bytes
  - other `movement_apply` movement opcodes:
    `0x00BB` `13.3 MB`, `0x00C9` `9.0 MB`, `0x00B5` `8.5 MB`,
    `0x00DA` `8.0 MB`, `0x00B7` `7.2 MB`
- The postrun-minus-steady delta shows ongoing load is dominated by
  `movement_apply` movement broadcasts, especially `MSG_MOVE_HEARTBEAT`.
  `player_add_visibility` is mostly startup/login visibility cost, while
  `player_visibility_refresh` remains the ongoing update-object producer.

Rejected fix experiment:

- Session-writer batching was tried in
  `logs/perf-rca/20260519-001018-500-same-grid-50ms-writer-batch-steady.summary.prom`
  and was reverted.
- Harness spell latency worsened versus the source-attribution control:
  `avg_response_ms=890.854`, `max_response_ms=1618.473`.
- Write and outbound queue timings were already small, so batching at the
  session writer did not attack the byte volume or producer fanout. Do not
  retry this as the first fix; coalesce stale movement replication before
  packets are enqueued to per-session writers.

First producer-side coalescing experiment:

- Code now coalesces stale `MSG_MOVE_HEARTBEAT` (`0x00EE`) observer broadcasts
  in the map-owned movement path. The server still accepts every movement
  packet and updates authoritative player state; observers receive heartbeat
  broadcasts at most once per `100 ms` per mover. Non-heartbeat movement
  packets still broadcast immediately.
- Active steady scrape:
  `logs/perf-rca/20260519-003425-500-same-grid-50ms-5-mage-sentinels-heartbeat-coalesce100-active-steady.summary.prom`
- Harness summary:
  `clients=500`, `failures=2`, `movements_sent=575564`,
  `packets_drained=3219278`; sentinel result `casts_sent=89`,
  `responses=89`, `avg_response_ms=948.112`,
  `max_response_ms=1635.198`.
- The scrape reached `500` connected sessions and `499` active map players.
  `CMSG_CAST_SPELL` service average/max were `742.636/1354.926 ms`.
- The intended byte bucket dropped sharply in the active steady scrape:
  `movement_apply` / `0x00EE` was `12,995,803` bytes, while
  `player_movement_broadcast` fanout for `0x00EE` averaged `87.408`
  recipients.
- However, spell latency did not improve. Treat heartbeat coalescing as a
  useful outbound-volume reduction, not as the complete root fix. The next
  investigation should focus on why `CMSG_CAST_SPELL` handler/service time
  remains high after the largest movement heartbeat byte source is reduced:
  map lock wait inside spell handling, remaining `SMSG_UPDATE_OBJECT` churn,
  and position-status/terrain lookup are the next suspects.

VMap tile-load cache guard:

- Static comparison against CMaNGOS showed that `TerrainInfo::LoadMapAndVMap`
  checks `IsTileLoaded(map, x, y)` before calling `loadMap(...)`, while the
  Rust native bridge called `VMapManager2::loadMap(...)` from hot query paths
  for height, liquid, area, and LOS.
- The bridge now routes those paths through `wow_vmap_ensure_tile_loaded(...)`,
  which returns immediately when the tile is already loaded and only calls
  `loadMap(...)` on a miss.
- Active steady scrape after the fix:
  `logs/perf-rca/20260519-010653-500-same-grid-50ms-5-mage-sentinels-vmap-cache-guard-active-steady.summary.prom`
- Harness summary:
  `clients=500`, `failures=20`, `movements_sent=241409`,
  `packets_drained=21814770`; sentinel result `casts_sent=89`,
  `responses=89`, `avg_response_ms=80.677`,
  `max_response_ms=310.921`.
- The scrape reached `500` connected sessions and `500` active map players.
  `CMSG_CAST_SPELL` service average/max were `245.803/1088.052 ms`.
- The geometry signal collapsed from the previous control's native WMO area
  lookup average of roughly `23 ms` to:
  - `area_entry` average `0.006 ms`
  - `wmo_area` average `0.003 ms`
  - native area info `found` average `0.007 ms`
  - native area info `not_found` average `0.002 ms`
- Movement actor queue age also dropped from `90.295 ms` average /
  `324.114 ms` max in the heartbeat-coalesce control to `0.589 ms` average /
  `63.432 ms` max. This confirms repeated native vmap load/check work was a
  real map-side movement-path latency source.
- The run still had thin-client failures, so do not treat it as a clean
  scalability pass. Treat it as valid root-cause evidence because the active
  scrape and sentinel summaries were captured.

Player visibility relocation threshold:

- CMaNGOS only updates object visibility after relocation crosses
  `Visibility.RelocationLowerLimit`, default `10` yards. Reference:
  `Unit::OnRelocated` compares current position against
  `m_last_notified_position` and only then calls
  `UpdateObjectVisibility()`.
- The Rust map-owned movement path was marking every accepted movement packet
  for `player_visibility_refresh`, so the refresh phase could rebuild
  player-player create/destroy visibility at `50 ms` movement cadence.
- The Rust map runtime now tracks
  `PlayerRuntime::last_player_visibility_refresh_position` and only marks a
  player-player visibility refresh after `10` yards of relocation, preserving
  the current visible-object set between relocation notifications.
- Active steady scrape after the fix:
  `logs/perf-rca/20260519-011936-500-same-grid-50ms-5-mage-sentinels-player-vis-relocation10-active-steady.summary.prom`
- Harness summary:
  `clients=500`, `failures=20`, `movements_sent=258904`,
  `packets_drained=23672092`; sentinel result `casts_sent=89`,
  `responses=88`, `avg_response_ms=39.000`,
  `max_response_ms=374.827`.
- The scrape reached `500` connected sessions and `498` active map players.
  `CMSG_CAST_SPELL` service average/max were `81.326/419.431 ms`.
- The intended producer dropped sharply compared with the vmap-cache-guard
  control:
  - `player_visibility_refresh/0x00A9` bytes:
    `130,241,624 -> 1,852,928`
  - `player_visibility_refresh/0x00A9` packets:
    `187,980 -> 2,674`
  - refresh players per sample:
    `163.870 avg / 494 max -> 0.393 avg / 6 max`
  - refresh packets per sample:
    `1748.995 avg / 9834 max -> 13.057 avg / 224 max`
- This confirms that per-packet player visibility refresh marking was a major
  update-object churn source. Remaining high volume is now mostly movement
  broadcast opcodes from `movement_apply`, especially `0x00EE`.

## Baseline Capture

Use release binaries and record the exact shape before each run:

- branch and commit
- dirty files
- player count
- spawn mode: `local_radius` or `creature_grid_scatter`
- movement interval
- movement phase jitter
- login stagger
- sentinel cast clients, spell id, interval, race, and class when enabled
- actor enabled/disabled
- actor queue capacity and max batch size
- hold duration and scrape timing
- world config path
- OOC EventAI isolation status

Useful commands:

```powershell
git status --short --branch
cargo build --release -p authserver -p worldserver -p world-load-test
```

Start a clean local-radius control:

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\start-thin-client-load.ps1 `
  -ClientCount 500 `
  -SpawnMode local_radius `
  -MoveIntervalMs 50 `
  -MovePhaseJitterMs 0 `
  -LoginStaggerMs 1 `
  -HoldSeconds 90 `
  -EnableMovementActor
```

Add a spell-latency sentinel without manual casting by seeding mage clients and
having one client self-cast `Frost Armor Rank 1` (`168`) every five seconds:

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\start-thin-client-load.ps1 `
  -ClientCount 500 `
  -SpawnMode local_radius `
  -MoveIntervalMs 50 `
  -MovePhaseJitterMs 0 `
  -LoginStaggerMs 1 `
  -HoldSeconds 90 `
  -CharacterClass 8 `
  -Race 1 `
  -SentinelCastClients 1 `
  -SentinelCastSpellId 168 `
  -SentinelCastIntervalMs 5000 `
  -EnableMovementActor
```

The harness stdout will include `sentinel-cast summary` with cast response
count, failure count, pending count, average response latency, and max response
latency. This is the sustainable replacement for manual real-client spell
spam. Use packet metrics for `0x012E` plus the sentinel summary to determine
whether spell lag is packet dispatch delay or spell handler/combat work.

During the steady-state window, capture metrics:

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\capture-rca-metrics.ps1 `
  -Scenario "500-local-radius-50ms-jitter0-actor-on"
```

Then repeat with only one variable changed.

## Scaling Matrix

Run enough points to identify the shape of collapse. For the first RCA pass, do
not optimize during the matrix.

| Players | Spread idle | Same-grid idle | Spread movement | Same-grid movement |
| ---: | --- | --- | --- | --- |
| 50 | optional | optional | required | required |
| 100 | optional | optional | required | required |
| 250 | optional | optional | required | required |
| 500 | optional | optional | required | required |
| 1000 | optional | optional | current sparse stress shape | optional |

Current harness modes:

- spread: `-SpawnMode creature_grid_scatter`
- same-grid/local swarm: `-SpawnMode local_radius`
- movement pressure: `-MoveIntervalMs 50`
- slower comparison: `-MoveIntervalMs 250` or `500`

## Jitter Matrix

For the first bad player count, repeat the same scenario with:

- `-MovePhaseJitterMs 0`
- `-MovePhaseJitterMs 50`
- `-MovePhaseJitterMs 250`
- `-MovePhaseJitterMs 500`

Interpretation:

- If jitter fixes the run, suspect synchronized burst collapse, actor/mailbox
  batching, map mutex contention, or timer alignment.
- If jitter does not help, suspect steady-state throughput: map tick work,
  movement apply cost, idle motion, visibility fanout, DB, outbound writes, or
  scheduler starvation.

## Decision Rules

First growing latency signal wins:

- High `wow_channel_queue_age_*` means the named mailbox is buffering old work.
- High `wow_channel_queue_depth_*` means the named mailbox is accumulating
  backlog, even if handler timings look small.
- High `wow_channel_send_wait_*` means bounded-channel send pressure is visible
  on the hot path.
- High `wow_world_packet_dispatch_delay_*` with low handler time means packets
  are waiting in the session loop before dispatch.
- High `wow_movement_actor_apply_start_latency_*` means movement is waiting in
  the movement actor mailbox.
- High `wow_movement_actor_reply_latency_*` with low apply-start and low apply
  time means the caller is waiting after actor work, usually reply/outbound
  path or scheduler delay.
- High `wow_movement_map_mutex_wait_*` means sessions or actor work are
  contending for the same map owner.
- High `wow_movement_map_mutex_hold_*` or high `wow_movement_apply_total_time_*`
  means map-owned movement application is expensive.
- High `wow_map_tick_lag_*` with high `wow_map_phase_duration_*` means map tick
  phases are falling behind.
- High outbound queue latency or depth means the server built state updates
  faster than socket writers can send them.
- Low outbound queue latency with high CPU in `world_session_writer` means the
  writers are keeping up by spending runtime capacity on socket writes. Treat
  this as an outbound volume / fanout problem even though backlog is not
  accumulating.

Specific shape interpretation:

- Same-grid bad, scatter acceptable: AOI, visibility, replication fanout, or
  repeated per-observer serialization.
- Scatter bad too: global/session/map throughput, broad map phases, lock
  contention, runtime starvation, or DB/external I/O.
- Handler time small but queue age large: throughput/backlog, not an individual
  handler bug.
- CPU low but latency high: locks, bounded-channel stalls, DB pool wait,
  outbound backpressure, or scheduler/off-CPU time.

## Capture Template

Append confirmed runs to `docs/performance_movement_benchmark.md` only after
the run shape and scrape timing are clear.

Use this scratch template while investigating:

```md
## RCA Capture: <scenario>

- Date:
- Branch/commit:
- Dirty state:
- Clients:
- Spawn mode:
- Move interval:
- Move phase jitter:
- Actor:
- Hold/scrape timing:
- Sessions / active players:
- Loaded grids:
- Client-visible latency:

Key signals:

- Packet dispatch delay:
- Packet handler duration:
- Packet service time:
- Movement actor queue depth:
- Movement apply-start latency:
- Movement actor reply latency:
- Map mutex wait / hold:
- Movement apply total:
- Map tick duration / lag:
- Dominant map phases:
- Visibility refresh:
- Idle motion:
- Outbound queue latency/depth:

Interpretation:

- First growing queue or phase:
- Queue wait vs handler time:
- Same-grid vs spread conclusion:
- Jitter conclusion:
- Current root-cause hypothesis:
- Next measurement:
```

## When To Add Instrumentation

Add new instrumentation only if the current metrics cannot distinguish the
next boundary. Likely additions, in order:

1. Per-opcode outbound byte counters:
   `wow_world_packet_outbound_enqueued_bytes_total{opcode}` and
   `wow_world_packet_write_bytes_total{opcode}`.
2. Per-opcode outbound write cost / count split for session writers:
   `wow_world_packet_write_duration_*{opcode}` and
   `wow_world_packets_out_total{opcode}`.
3. Per-opcode / per-source recipient fanout counters:
   `wow_world_outbound_fanout_recipients_*{source,opcode}` for movement
   broadcast and nearby-player broadcast sources.
4. Area / geometry lookup attribution:
   `wow_world_geometry_area_entry_*{source}`,
   `wow_world_geometry_wmo_area_*{source}`,
   `wow_world_geometry_area_flag_*{source}`,
   `wow_world_geometry_native_area_info_*{status}`,
   `wow_world_geometry_native_area_flag_*{status}`,
   `wow_world_geometry_lookup_results_total{result}`, and
   `wow_world_position_status_total{result}`. Native map/vmap load/cache
   hit/miss counts still need a C++ bridge hook if repeated `loadMap` remains
   visible after this pass.
5. Per-session stale movement replication coalescing counters:
   queued, replaced, dropped, and flushed by opcode.
6. Per-session pending movement coalesce age at dispatch.
7. Per-session packet-read to `process_authenticated_world_packet` entry age.
8. Movement visibility streaming subphase metrics for creature, gameobject, and
   corpse streaming still inline in `movement.rs`.
9. DB pool wait/query duration labels by caller if a run points at DB awaits.
10. Lock wait/hold wrappers for any newly identified hot `Mutex` or `RwLock`.

Keep any new metrics in `crates/wow-network/src/observability.rs` unless a
cross-crate boundary truly needs a shared helper.

## Next Fix Experiments

Do these in small, measurable steps and rerun the same `500` same-grid
stationary-sentinel control after each one:

1. Reduce remaining `movement_apply` movement-broadcast volume. After the
   player visibility relocation threshold, `player_visibility_refresh` is no
   longer the dominant ongoing `SMSG_UPDATE_OBJECT` producer; movement opcodes
   from `movement_apply`, especially `0x00EE`, are again the largest byte
   bucket.
2. Add finer spell-handler stage timing for `CMSG_CAST_SPELL`: separate
   session dispatch wait, map lock wait, spell lookup/validation, map mutation,
   direct response build, and observer broadcast. The latest controls brought
   the average down, but service tails still need an exact stage split.
3. Reduce area lookup frequency with a CMaNGOS-shaped rule: only recompute on
   meaningful cell/tile/area transitions or after a cached state invalidates,
   while preserving exploration and WMO override correctness.
4. Add native vmap/map load/cache hit/miss counts if the bridge can expose
   them cheaply; the guard fixed repeated loads, but counters would make
   regressions obvious.
5. Revisit same-grid AOI/visibility after movement coalescing and
   `player_visibility_refresh` reduction have fresh control data.

Do not start with session-writer batching. That experiment was measured and
reverted because it worsened the sentinel spell average and did not reduce the
producer-side outbound byte volume.
