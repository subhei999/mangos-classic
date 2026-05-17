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
4. Wait for the client ramp to stabilize before taking a baseline scrape.
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

## Baseline Runs

| Timestamp | Scenario | Observed sessions | Observed active players | Loaded grids | Tick avg 1m | Tick lag avg 1m | Tick latest | Player environment avg 1m | Idle motion dispatch avg 1m | Notes |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | --- |
| 2026-05-17T14:15:54-07:00 | Thin-client release baseline, target `50` | 48 | 48 | 4 | 51.656 ms | 5.079 ms | 70.323 ms | 24.477 ms | 0.009 ms | Settled scrape from live 50-client run. |
| 2026-05-17T14:11 approx. | Thin-client release baseline, target `50` | 51 | 50 | 4 | 44.739 ms | 5.107 ms | not captured | not captured | not captured | Earlier scrape during the same run; useful as a ramp/stability reference. |

## Interpretation

- The current release thin-client baseline is roughly `45-52 ms` map tick
  average at a `50`-client target in this Northshire-local movement scenario.
- In the settled scrape, `player_environment` is already a meaningful share of
  the tick budget, while `idle_motion_dispatch` is essentially noise.
- This makes the benchmark a good candidate for evaluating a batched
  player-movement architecture, because the benchmark pressure is coming from
  real session and movement handling rather than planner-driven bot work.

## Next Comparison Target

When batched player movement lands, rerun this exact `50`-client scenario and
append a new row with:

- old architecture result
- batched architecture result
- delta in tick average
- delta in tick lag
- any observed change in dominant phase cost
