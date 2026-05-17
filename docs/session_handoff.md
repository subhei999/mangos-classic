# Session Handoff

Short operating brief for the next Rust migration session. Durable roadmap
history belongs in `docs/rust_migration_plan.md`, gate status in
`docs/playable_gate_board.md`, and branch split details in
`docs/playable_execution_roadmap.md`.

## Current Branch And State

- Branch: `codex/rusty-mangos`, main checkout:
  `C:\Users\subhe\Documents\New project`.
- Latest pushed commit: `0230c2fc9 Implement creature EventAI and wounded
  slowdown`.
- Current local work is uncommitted and extends the Mage/parity slice with a
  more CMaNGOS-shaped `Polymorph` foundation: spell-facing lookup from
  `spell_facing`, caster-owned single-target creature aura tracking, map-owned
  polymorph diminishing state, polymorph helper regen plumbing, centralized
  creature damage-break handling, and now a dedicated confused-motion owner and
  scheduler instead of routing sheep wander through the ambient idle-motion
  queue. It now also adds a reproducible release-mode playerbot performance
  scenario: 500 deterministic map-0 idle bots using a Northshire-centered
  1000-yard radius spread, combat-disabled bot runtime, force-active roaming
  without client-interest gating, a dedicated `config/worldserver.perf.toml`, and
  `scripts/start-playerbot-idle-perf.ps1` to boot the stack. The perf swarm now
  uses a benchmark-only `local_roam_only` mode so idle movement starts directly
  from the map movement tick instead of consuming the async playerbot planner.
  It now also adds a thin-client load harness in `bins/world-load-test` plus
  `scripts/start-thin-client-load.ps1` so release auth/world servers can seed
  dedicated accounts/characters and drive hundreds of normal SRP/world logins
  with movement heartbeats.
- The user remains the Northshire Checkpoint 2 grader through real-client
  playtesting. Do not add or maintain a Northshire grading harness.
- Playerbots are disabled for normal testing in
  `config/worldserver.local.toml`.

## Current Goal

- The new user-directed performance setups are ready:
  `scripts/start-playerbot-idle-perf.ps1` for the planner-free 500-bot
  lower-bound scenario, and `scripts/start-thin-client-load.ps1` for a real
  auth/world login swarm that seeds dedicated accounts/characters and drives
  normal movement packets through release `authserver`/`worldserver`.
- The thin-client launcher now defaults to a Northshire-centered safe spread
  (`150` yards, `25 ms` login stagger, `3` attempts per client) that avoids
  creature-combat noise while still exercising normal session bootstrap,
  player visibility, and movement packet handling.
- A dedicated benchmark report now lives in
  `docs/performance_movement_benchmark.md` and should be the append-only home
  for thin-client and playerbot perf baselines as architecture changes land.
- Immediate recommended follow-up for performance work: capture observability
  baselines from both the lower-bound playerbot scenario and the thin-client
  release scenario, then compare future batched player-movement changes
  against the same `50`-client benchmark first before scaling back up.
- After perf baseline capture, return to the Mage/parity real-client checks,
  especially the open `Polymorph` combat-state regression and then the pending
  Mana Shield / Ward / utility spell smoke list.

## What Changed Recently

- Player hostile spell-facing checks now use cached `spell_facing` DB data
  through `ObjectMgr`, matching the CMaNGOS ownership boundary instead of
  forcing all hostile spells to require front arc. `Polymorph` can now cast
  while the caster is not facing the target, while `Fireball`-style flagged
  spells still fail with `SPELL_FAILED_UNIT_NOT_INFRONT`.
- Creature aura application now carries optional caster-owned single-target
  descriptors and diminishing groups. `Polymorph` can replace the same caster's
  prior sheep target instead of allowing multiple concurrent sheeped creatures.
- MapRuntime now tracks active single-target creature auras and per-target
  diminishing state for polymorph-like CC. Tracker cleanup runs on normal aura
  removal and on break-on-damage removal.
- Periodic creature damage and dynamic-object periodic damage now break
  damage-interrupt creature auras too, so `Polymorph` drops from DoT ticks and
  Blizzard-style periodic AoE paths instead of only direct damage.
- Periodic regen metadata now distinguishes consumable-style sit/move/damage
  interruption from unsuppressed helper regen, which allows polymorph helper
  healing without inheriting food/drink semantics.
- Confused creature movement now has its own map-owned start schedule and
  runtime state. Sheep wander no longer depends on the ambient idle/random
  start queue, keeps a fixed confused origin like the CMaNGOS generator, and
  rearms its own pause timer between short walk splines.
- `Polymorph` now clears creature combat through the shared map combat owner
  instead of raw-removing only `active_creature_combats`. That keeps player
  `in_combat`, leash, threat, and combat re-entry bookkeeping in sync after
  sheep drops combat.
- A focused unit test now proves the map-owned combat state can clear on sheep
  and later restart for the same creature, but the user still reports a
  critical real-client bug where sheep can leave the player out of combat even
  after the mob resumes attacking. Treat the local test as insufficient proof
  until the live repro is resolved.
- Playerbot config now supports `combat_enabled`, `force_active`, and random
  distributions including `cell_scatter` and `grid_scatter`, while the perf
  launcher can also drive a simple `radius` spread through env overrides. The
  force-active path bypasses client-interest sleeping only for explicitly
  configured bot load tests, while combat-disabled bots skip combat
  planning/ticks so the 500-bot benchmark is dominated by idle roaming and
  shared map ownership instead of creature AI.
- The perf benchmark now also supports `local_roam_only`, which suppresses
  planner-owned idle roam inputs and synthesizes local roam routes directly in
  the movement tick. On the refreshed 500-bot release run, latest planner cost
  dropped to roughly `0.023 ms`, while movement remained the dominant phase
  (`157.556 ms` latest sample, `90.412 ms` 1-minute average during early warmup).
- A follow-up gate now tracks whether any planner-driven bots exist at all. If
  the world only contains `local_roam_only` + combat-disabled perf bots, the
  async planner loop returns before touching maps or contending on the map
  mutex. After restarting the release benchmark with that gate, planner
  observability fell to effectively noise (`0.001 ms` latest, `0.000 ms`
  1-minute average in the first settled scrape) while movement stayed dominant.
- `scripts/run-client-stack-18085.ps1` and `scripts/restart-game-stack.ps1`
  now accept release-mode starts. The dedicated perf wrapper uses
  `target\release\*.exe` plus `config/worldserver.perf.toml` so the benchmark
  does not include the old single visibility bot from `worldserver.local.toml`.
- `bins/world-load-test` now reuses the in-tree SRP/world protocol helpers to
  seed dedicated load-test accounts and characters, log them in normally
  through `authserver` + `worldserver`, and drive `MSG_MOVE_HEARTBEAT` for a
  bounded hold window. Login bootstrap now waits for self-spawn updates instead
  of assuming a fixed packet count, and each client gets a small retry budget
  so burst runs survive transient socket aborts.
- The thin-client harness now emits a more real-client-like movement mix
  instead of pure heartbeats: quiet periods, `MSG_MOVE_START_FORWARD`,
  `MSG_MOVE_HEARTBEAT`, `MSG_MOVE_STOP`, `MSG_MOVE_SET_FACING`, and occasional
  `MSG_MOVE_JUMP` / `MSG_MOVE_FALL_LAND` packets with matching movement flags
  and jump payloads, while still staying inside the configured local roam
  radius.
- A new benchmark report now records the current methodology, observability
  metrics of interest, and baseline numbers for the `50`-client release thin
  client scenario. The first settled scrape recorded about `51.656 ms`
  1-minute map tick average, `5.079 ms` 1-minute tick lag average, `48`
  connected/active clients at scrape time, and `24.477 ms`
  `player_environment` phase average.
- `scripts/start-thin-client-load.ps1` now restarts the release stack against
  `config/worldserver.local.toml`, then runs the thin-client harness with safe
  defaults (`500` clients, `25 ms` stagger, Northshire `150`-yard spread,
  `3` max attempts). Verified runs completed cleanly at `50`, `200`, and `500`
  clients with `10 s` hold windows.

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
- `cargo test -p wow-network --lib` passed: 743 tests.
- `.\scripts\test-rust.cmd` passed fmt/clippy/check/unit/integration coverage
  again, then failed only at the final `cargo build -p authserver` step because
  Windows could not overwrite a running `target\debug\authserver.exe`
  (`Access is denied`, os error 5).
- `powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\start-playerbot-idle-perf.ps1`
  successfully launched the release stack. Verified:
  `target\release\worldserver.exe`, observability dashboard `200`, exactly
  500 `Loaded playerbot actor into MapRuntime` log lines, and Northshire
  placement inside the requested 1000-yard radius (`max_distance=999.75` from
  center `(-8949, -132)`).
- `cargo build -p world-load-test` passed.
- `cargo build --release -p world-load-test` passed.
- `cargo test -p world-load-test` passed (`3` unit tests covering opcode mix,
  radius envelope, and jump payloads).
- `powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\start-thin-client-load.ps1 -ClientCount 5 -HoldSeconds 10 -CenterX -8949 -CenterY -132 -CenterZ 83.5 -Radius 150`
  passed: `5/5` clients completed normal auth/world login plus movement.
- `.\target\release\world-load-test.exe --client-count 5 --hold-seconds 10 ...`
  passed against the live release auth/world stack after the richer movement
  signal upgrade: `5/5` clients completed, `75` movement packets sent.
- `powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\start-thin-client-load.ps1 -ClientCount 50 -HoldSeconds 10`
  passed with the new safe defaults: `50/50` clients completed, `1000`
  movements sent, `16300` packets drained.
- `powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\start-thin-client-load.ps1 -ClientCount 200 -HoldSeconds 10`
  passed: `200/200` clients completed, `4000` movements sent, `89899`
  packets drained.
- `powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\start-thin-client-load.ps1 -ClientCount 500 -HoldSeconds 10`
  passed: `500/500` clients completed, `9832` movements sent, `555140`
  packets drained.
- Live observability capture for the `50`-client thin-client release baseline
  is now recorded in `docs/performance_movement_benchmark.md`, including a
  settled scrape at `2026-05-17T14:15:54-07:00`.

## Real-Client Verification Needed

- `Polymorph` should show sheep display, restore health while active, allow
  casts without facing, wander convincingly while controlled, break from
  melee, spell direct damage, DoTs, and Blizzard ticks, and cleanly drop then
  re-enter player combat when sheeped creatures resume hostility.
- Current real-client blocker: after `Polymorph`, the player may remain out of
  combat while the mob attacks, allowing food/drink usage. The recent
  map-owned combat-clear patch did not yet earn user confidence and should be
  treated as still open until replayed in client with packet/state inspection.
- Confirm polymorph diminishing returns timing against repeated re-sheep
  sequences, including whether evade/home reset should clear the DR chain.
- Blink should teleport forward roughly 20 yards from the caster using terrain
  ground placement; no target should be required.
- Mana Shield should absorb melee damage and consume mana; Fire/Frost Ward
  should absorb only matching school damage.
- Remove Curse/Detect Magic/Dampen Magic need live-client checks for correct
  aura visibility, dispel result, and failure feedback.
- Arcane Missiles should keep the caster in channel pose through all three rank
  1 missile launches, aggro only when impact damage lands, and stop dead target
  motion.
- Blizzard should cancel on movement/damage interruption, aggro on periodic
  damage, and only affect hostile targets in the selected ground area.
- Flamestrike should cast at a destination without unit target and hurt only
  attackable hostile creatures.

## Known Follow-Ups

- `Polymorph` helper regen is wired, but there is not yet a dedicated
  regression that proves the helper aura heals exactly like CMaNGOS; current
  confidence is from code-path inspection and integration plumbing.
- Diminishing returns still need a real-client parity check around evade/home
  reset. Current DR state is target-owned and time-based, but evade does not
  yet explicitly clear the polymorph DR chain the way CMaNGOS aura cleanup may.
- The sheep combat-state regression may still involve a second session-facing
  or retaliation/chase edge beyond the map-owned clear path. If the bug
  reproduces after the latest patch, inspect live combat-flag packets,
  retaliation re-entry, and session cache refresh around sheep break/resume
  instead of assuming the map-side owner fix was sufficient.
- Full dynamic-object aura semantics are still incomplete beyond current
  create/destroy/channel/periodic-damage support.
- Map-owned periodic player spell kills still need DB-backed corpse loot prep
  before relying on Blizzard/Flamestrike/Arcane Missiles as common loot-bearing
  killing blows.
- Utility effects still pending: duel ownership, stuck/graveyard/hearth flow,
  and remove-insignia/player-corpse logic.
- The full script may fail to rebuild while local auth/world binaries are
  running because Windows locks `target\debug\*.exe` and `target\release\*.exe`;
  stop the stack before verification builds.

## Key Files

- `crates/wow-db/src/world_data.rs`
- `bins/world-load-test/src/main.rs`
- `crates/wow-network/src/world/globals/object_mgr.rs`
- `crates/wow-network/src/world/session.rs`
- `crates/wow-network/src/world/spells.rs`
- `crates/wow-network/src/world/spells/effects.rs`
- `crates/wow-network/src/world/map_runtime/map.rs`
- `crates/wow-network/src/world/playerbots.rs`
- `config/worldserver.perf.toml`
- `scripts/start-playerbot-idle-perf.ps1`
- `scripts/start-thin-client-load.ps1`
- `docs/performance_movement_benchmark.md`
- `scripts/run-client-stack-18085.ps1`
- `crates/wow-network/src/world/map_runtime/map_manager.rs`
- `crates/wow-network/src/world/map_runtime/map/creature_damage.rs`
- `crates/wow-network/src/world/map_runtime/map/dynamic_objects.rs`
- `crates/wow-network/src/world/map_runtime/map/players.rs`
- `crates/wow-network/src/world/tests.rs`
- `docs/northshire_spell_audit.md`
