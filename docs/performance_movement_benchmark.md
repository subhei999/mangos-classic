# Movement Benchmark Report

This document tracks the movement-performance benchmark we are using to compare
world-server architecture changes over time.

## Goal

Measure the server cost of many normal player sessions logging in and moving at
the same time, with emphasis on the shared world/runtime path rather than
server-owned playerbot AI.

The immediate architecture question is whether moving from the current
per-session/per-movement handling toward a batched player-movement pipeline
reduces map tick cost and tick lag under concurrent load.

## What We Are Measuring

- `wow_map_tick_duration_average_1m_milliseconds`
- `wow_map_tick_lag_average_1m_milliseconds`
- `wow_map_tick_duration_latest_milliseconds`
- `wow_map_phase_duration_average_1m_milliseconds{phase="player_environment"}`
- `wow_map_phase_duration_average_1m_milliseconds{phase="idle_motion_dispatch"}`
- `wow_world_sessions_connected`
- `wow_map_active_players{map_id="0",instance_id="0"}`
- `wow_map_loaded_grids{map_id="0",instance_id="0"}`

Primary KPI:

- map tick average over 1 minute

Secondary KPIs:

- map tick lag average over 1 minute
- latest tick duration
- dominant phase costs inside the map tick
- connected sessions and active players during the scrape

## Harness

Thin-client benchmark:

- launcher: [start-thin-client-load.ps1](/C:/Users/subhe/Documents/New%20project/scripts/start-thin-client-load.ps1)
- client binary: [main.rs](/C:/Users/subhe/Documents/New%20project/bins/world-load-test/src/main.rs)
- server binaries: `target\release\authserver.exe` and `target\release\worldserver.exe`
- config: [worldserver.local.toml](/C:/Users/subhe/Documents/New%20project/config/worldserver.local.toml)

Behavior:

- seeds dedicated benchmark accounts and characters
- logs in through normal SRP auth and world auth
- performs normal character enum and player login
- synchronizes movement start after login/bootstrap so the steady-state movement
  window is measured without overlapping the tail of the login storm
- sends a more real-client-like movement mix:
  `MSG_MOVE_START_FORWARD`, `MSG_MOVE_HEARTBEAT`, `MSG_MOVE_STOP`,
  `MSG_MOVE_SET_FACING`, occasional `MSG_MOVE_JUMP`, and
  `MSG_MOVE_FALL_LAND`
- uses a Northshire-centered safe spread to avoid combat noise

Default benchmark parameters for the current baseline:

- target clients: `50`
- hold window: `600 s`
- move interval: `500 ms`
- login stagger: `25 ms`
- center: `(-8949, -132, 83.5)`
- spread radius: `150`
- local move radius: `6`

## Methodology

1. Run the benchmark only against release binaries.
2. Restart auth/world through the thin-client launcher so the stack and harness
   use the same settings every time.
3. Let worldserver finish startup and static-cache warmup before interpreting
   metrics.
4. Wait until the target clients are logged in and moving, then take the
   benchmark scrape after a `10 s` steady-state window.
5. Record both the target client count and the observed connected/active count
   at scrape time.
6. Compare future architecture changes against the same scenario first before
   expanding to larger swarms.

Command:

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\start-thin-client-load.ps1 -ClientCount 50 -HoldSeconds 600
```

Observability endpoint:

- [http://127.0.0.1:9091/dashboard](http://127.0.0.1:9091/dashboard)
- [http://127.0.0.1:9091/metrics](http://127.0.0.1:9091/metrics)

## Caveats

- `wow_world_sessions_connected` and `wow_map_active_players` are not always
  identical during ramp-up or disconnect churn.
- worldserver startup includes a large static creature cache load, so early
  scrapes right after restart are not comparable to settled-state runtime.
- this benchmark is intentionally measuring real auth/login/session/movement
  traffic, not packet fanout from all players stacked in one spot and not
  playerbot planner cost.
- the synchronized-movement-start harness revision makes later `200`-client
  reruns cleaner for steady-state movement analysis, but those numbers are not
  directly apples-to-apples with older `200`-client runs where movement began
  while logins were still ramping.
- the long-hold `200`-client harness can still end with
  `STATUS_ACCESS_VIOLATION (0xc0000005)`, so short steady-state captures are
  currently more reliable than waiting for a perfect clean exit.

## Baseline Runs

| Timestamp | Scenario | Observed sessions | Observed active players | Loaded grids | Tick avg 1m | Tick lag avg 1m | Tick latest | Player environment avg 1m | Idle motion dispatch avg 1m | Notes |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | --- |
| 2026-05-17T14:15:54-07:00 | Thin-client release baseline, target `50` | 48 | 48 | 4 | 51.656 ms | 5.079 ms | 70.323 ms | 24.477 ms | 0.009 ms | Settled scrape from live 50-client run. |
| 2026-05-17T14:11 approx. | Thin-client release baseline, target `50` | 51 | 50 | 4 | 44.739 ms | 5.107 ms | not captured | not captured | not captured | Earlier scrape during the same run; useful as a ramp/stability reference. |

## Movement Actor A/B

These runs were designed to answer whether the `codex/g12-movement-actor-proxy`
slice materially improves the thin-client movement benchmark.

Scenario adjustments from the baseline:

- move interval tightened to `50 ms`
- login stagger reduced to `1 ms`
- same release thin-client harness, same Northshire-local spread
- compared `experimental_movement_actor = false` vs `true`
- tested both `50` and `200` client targets

| Date | Scenario | Actor | Sessions | Active players | Tick avg 1m | Tick lag avg 1m | Player environment avg 1m | Actor batch avg | Actor reply avg | Movement mutex wait avg | Movement mutex hold avg | Notes |
| --- | --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | --- |
| 2026-05-17 | Thin-client A/B, `50` clients, `50 ms` movement | Off | 50 | 50 | 74.947 ms | 3.169 ms | 34.351 ms | 0.000 | 0.000 ms | 2.129 ms | 0.021 ms | Control run with tighter movement cadence. |
| 2026-05-17 | Thin-client A/B, `50` clients, `50 ms` movement | On | 50 | 50 | 77.639 ms | 3.721 ms | 35.460 ms | 1.245 | 2.872 ms | 2.210 ms | 0.028 ms | No win; slightly worse in this sample. |
| 2026-05-17 | Thin-client A/B, `200` clients, `50 ms` movement | Off | 200 | 199 | 269.322 ms | 217.383 ms | 121.997 ms | 0.000 | 0.000 ms | 22.038 ms | 0.044 ms | Heavy control run. |
| 2026-05-17 | Thin-client A/B, `200` clients, `50 ms` movement | On | 200 | 200 | 267.320 ms | 218.998 ms | 120.456 ms | 3.089 | 34.275 ms | 22.350 ms | 0.117 ms | Essentially flat; any improvement is within noise. |

## Environment Tick Optimization Follow-Up

This follow-up adds map-owned environment caching and a fast path for safe
players:

- cached `environment_flags` on `PlayerEnvironmentRuntime`
- bounded recheck interval for safe players
- movement/grid/cell invalidation
- at-risk players kept on high-frequency checks
- new player-environment subphase metrics

### `50`-Client Retest

Same harsher shape used in the earlier movement-actor A/B:

- move interval `50 ms`
- login stagger `1 ms`
- same release thin-client harness

| Date | Scenario | Actor | Sessions | Active players | Tick avg 1m | Tick lag avg 1m | Player environment avg 1m | Movement mutex wait avg | Notes |
| --- | --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | --- |
| 2026-05-17 | Thin-client retest after environment optimization, `50` clients | Off | 50 | 50 | 62.888 ms | 2.873 ms | 28.319 ms | 1.427 ms | Improved from prior `74.947 ms` / `34.351 ms`. |
| 2026-05-17 | Thin-client retest after environment optimization, `50` clients | On | 50 | 50 | 62.206 ms | 3.983 ms | 27.583 ms | 1.721 ms | Improved from prior `77.639 ms` / `35.460 ms`. |

### `200`-Client Retest

The original post-optimization retest was blocked by a thin-client harness
crash. The harness now gates movement start until clients finish
login/bootstrap, which removed the `STATUS_ACCESS_VIOLATION (0xc0000005)` and
allowed the `200`-client run to complete long enough for steady-state scrapes.

Important methodology note:

- these reruns use the synchronized movement-start harness revision
- that makes them better steady-state movement measurements
- they should not be treated as direct apples-to-apples replacements for the
  older `200`-client A/B rows where movement overlapped the login storm

| Date | Scenario | Actor | Sessions | Active players | Tick avg 1m | Tick lag avg 1m | Player environment avg 1m | Actor batch avg | Actor reply avg | Movement mutex wait avg | Notes |
| --- | --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | --- |
| 2026-05-17 | Thin-client retest after environment optimization, `200` clients, synchronized movement start | Off | 201 | 201 | 90.000 ms | 5.712 ms | 33.922 ms | 0.000 | 0.000 ms | 7.184 ms | Harness crash fixed; run stayed up for scrape, but finished with `2` exhausted-client failures. |
| 2026-05-17 | Thin-client retest after environment optimization, `200` clients, synchronized movement start | On | 200 | 200 | 95.985 ms | 16.120 ms | 35.369 ms | 3.103 | 10.737 ms | 7.384 ms | Harness crash fixed; run stayed up for scrape, but finished with `3` exhausted-client failures. |

### `200`-Client Apples-To-Apples, `10 s` Steady-State Window

This is the cleaner comparison shape to use going forward:

- synchronized movement start
- target `200` clients
- `50 ms` move interval
- `1 ms` login stagger
- scrape taken `10 s` after the swarm reached the steady-state
  logged-in-and-moving condition
- short `40 s` hold to minimize end-of-run churn

| Date | Scenario | Actor | Sessions | Active players | Tick avg 1m | Tick lag avg 1m | Tick latest | Player environment avg 1m | Actor batch avg | Actor reply avg | Movement mutex wait avg | Notes |
| --- | --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | --- |
| 2026-05-17 | Thin-client steady-state comparison, `200` clients, `10 s` capture | Off | 200 | 200 | 24.861 ms | 5.732 ms | 96.678 ms | 9.270 ms | 0.000 | 0.000 ms | 7.020 ms | Zero client failures; clean short-run steady-state capture. |
| 2026-05-17 | Thin-client steady-state comparison, `200` clients, `10 s` capture | On | 200 | 200 | 25.683 ms | 6.855 ms | 68.478 ms | 9.595 ms | 3.129 | 8.492 ms | 6.234 ms | Zero client failures; actor still does not beat direct path. |

### `200`-Client Actor-On, Old Vs New Environment Path

To isolate the environment optimization itself, we temporarily added a
benchmark-only switch that could disable the cache. That temporary control path
has now been removed again; the cached environment path is the only supported
runtime path going forward. These historical runs used the same
synchronized-start `200`-client steady-state shape with the movement actor
enabled and changed only that temporary cache toggle.

Correction:

- the first attempt at this comparison sampled a `1m` metric after only `10 s`
  of steady state, which under-reported the true steady-state cost
- the corrected comparison below waits `70 s` after the swarm is fully logged
  in and moving, then records both the true `1m` steady-state averages and a
  direct `10 s` sample of the live `latest` tick metric

| Date | Scenario | Env path | Sessions | Active players | Tick avg 1m | Tick lag avg 1m | Tick latest avg 10s | Tick latest max 10s | Player environment avg 1m | Env flags avg | Actor reply avg | Movement mutex wait avg | Notes |
| --- | --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | --- |
| 2026-05-17 | Thin-client steady-state comparison, `200` clients, actor on, corrected long-window capture | Old cache-disabled path | 200 | 200 | 247.922 ms | 200.790 ms | 249.392 ms | 296.479 ms | 110.521 ms | 184.210 ms | 29.549 ms | 19.494 ms | One exhausted-client failure by run end, but the steady-state scrape had all `200` sessions and players present. |
| 2026-05-17 | Thin-client steady-state comparison, `200` clients, actor on, corrected long-window capture | New cache-enabled path | 200 | 200 | 97.240 ms | 20.004 ms | 86.090 ms | 120.405 ms | 35.173 ms | 60.191 ms | 10.169 ms | 6.922 ms | Three exhausted-client failures by run end, but the steady-state scrape had all `200` sessions and players present. |

### Fully Movement-Owned Environment Follow-Up

This follow-up finishes the ownership move so the player-environment tick no
longer walks all non-bot players. Instead:

- movement, login, and teleport paths own environment-flag refresh
- the environment tick advances timers from cached flags
- only a narrow map-owned at-risk set gets periodic geometry revalidation

Current verification shape:

- synchronized movement start
- target `200` clients
- movement actor enabled
- `50 ms` move interval
- `1 ms` login stagger
- short `40 s` hold with a `10 s` steady-state capture

| Date | Scenario | Sessions avg | Active players avg | Tick latest avg 10s | Tick latest max 10s | Player environment latest avg 10s | Players scanned per tick avg 10s | Env flags latest avg 10s | Movement mutex hold latest avg 10s | Movement mutex wait latest avg 10s | Notes |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | --- |
| 2026-05-17 | Thin-client steady-state follow-up, `200` clients, actor on, fully movement-owned env | 201 | 201 | 99.436 ms | 113.233 ms | 7.520 ms | 5.600 | 0.209 ms | 6.997 ms | 1.237 ms | Confirms the environment tick is no longer scanning the whole player set. The long-hold harness still crashes later, so this row uses the short steady-state capture window. |

## Interpretation

- The current release thin-client baseline is roughly `45-52 ms` map tick
  average at a `50`-client target in this Northshire-local movement scenario.
- In the settled scrape, `player_environment` is already a meaningful share of
  the tick budget, while `idle_motion_dispatch` is essentially noise.
- This makes the benchmark a good candidate for evaluating a batched
  player-movement architecture, because the benchmark pressure is coming from
  real session and movement handling rather than planner-driven bot work.
- The movement actor proxy does not materially improve this benchmark by
  itself.
- In the harsher A/B runs, actor batch depth stayed modest:
  about `1.245` at `50` clients and `3.089` at `200` clients.
- The proxy still applies movement against the same `MapRuntime` mutex, so its
  main possible win is lock-acquisition amortization and same-player supersede
  collapsing. That was not enough to shift overall map tick cost in a
  meaningful way here.
- The dominant cost remained `player_environment`, not movement mutex hold
  time, which further limits how much this proxy slice can help the total tick
  budget.
- The environment tick optimization, by contrast, produced a visible win at
  `50` clients under the same harsher load shape:
  roughly `16%` lower tick average with actor off and roughly `20%` lower with
  actor on.
- That supports the earlier hypothesis that environment/liquid checks were a
  meaningful hotspot worth attacking before expecting large gains from the
  movement actor proxy alone.
- The `200`-client rerun is unblocked again, but because the harness now
  synchronizes movement start after login/bootstrap, its lower tick numbers are
  not attributable to the environment optimization alone.
- Even in that cleaner synchronized-start shape, the actor-on `200`-client run
  still did not beat actor-off, so the movement actor proxy remains a flat or
  slightly worse experiment in this benchmark.
- In the newer `10 s` steady-state comparison shape, the result is even
  clearer: actor-on remained slightly worse than actor-off at `200` clients,
  while both runs stayed fully populated and completed without client failures.
- With actor enabled and the harness held constant, the corrected long-window
  comparison shows the environment optimization itself produced a large
  `200`-client win:
  `247.922 ms -> 97.240 ms` on true `1m` tick average and
  `110.521 ms -> 35.173 ms` on the `player_environment` phase average.
- That is roughly a `61%` reduction in total tick average and a `68%`
  reduction in the `player_environment` phase for the actor-on `200`-client
  steady-state benchmark.
- The fully movement-owned follow-up tightens that further in the short
  steady-state view: `player_environment` latest is down around `7.5 ms`, and
  the environment tick only scans about `5.6` players per pass instead of the
  full moving population.
- That is strong evidence that environment checking is no longer the dominant
  scaling term in this benchmark.
- After this ownership move, the next likely bottlenecks are the movement path
  itself, especially actor reply latency and `MapRuntime` mutex hold time,
  rather than native geometry/liquid checks.
- Because that comparison is decisive, the old cache-disabled environment path
  is no longer kept in the codebase or benchmark launcher.

## Next Comparison Target

When batched player movement lands, rerun this exact `50`-client scenario and
append a new row with:

- old architecture result
- batched architecture result
- delta in tick average
- delta in tick lag
- any observed change in dominant phase cost
