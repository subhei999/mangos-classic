# Session Handoff

This is the short operating brief for the next Rust migration session. Keep it
pruned. Do not append a chronological log.

Durable roadmap belongs in `docs/rust_migration_plan.md`; gate status belongs
in `docs/playable_gate_board.md`; detailed G12 design belongs in
`docs/g12_shared_mapruntime_plan.md`; auth setup belongs in
`docs/rust_auth_foundation.md`.

## Current Branch And Worktree

- Branch: `codex/rusty-mangos`.
- Latest pushed commit: `30094c12a` (`Update session handoff checkpoint hash`).
- Latest local commit: `3b03a4572` (`Add CMaNGOS-style mmap path smoothing`);
  local branch is ahead of origin by 1.
- Current worktree has uncommitted G8/G9 movement and threat work:
  - `crates/wow-network/src/world/combat/motion.rs`
  - `crates/wow-network/src/world/combat/broadcast.rs`
  - `crates/wow-network/src/world/combat/melee.rs`
  - `crates/wow-network/src/world/combat/evade.rs`
  - `crates/wow-network/src/world/combat/aggro.rs`
  - `crates/wow-network/src/world/combat/lifecycle.rs`
  - `crates/wow-network/src/world/entities/creature.rs`
  - `crates/wow-network/src/world/loot.rs`
  - `crates/wow-network/src/world/maps/map.rs`
  - `crates/wow-network/src/world/maps/map/creature_combat.rs`
  - `crates/wow-network/src/world/maps/map/creature_damage.rs`
  - `crates/wow-network/src/world/maps/map/creature_loot.rs`
  - `crates/wow-network/src/world/maps/map/creature_motion.rs`
  - `crates/wow-network/src/world/maps/map/players.rs`
  - `crates/wow-network/src/world/maps/map_manager.rs`
  - `crates/wow-network/src/world/opcodes.rs`
  - `crates/wow-network/src/world/spells.rs`
  - `crates/wow-network/src/world/tests.rs`
  - this handoff file
- Do not revert unrelated user changes if more dirty files appear.

## Current Goal

Current milestone: **Northshire Human Warrior playable slice**.

Current user direction: continue toward a faithful, shared, crash-resistant
CMaNGOS-style Rust worldserver. G12 multiplayer/shared MapRuntime is
substantially implemented; current active work is the G8 combat-fidelity ladder.

Near-term G8 order requested by the user:

1. Real melee roll table.
2. Real swing timers.
3. Real damage formula.
4. Reach/model modifiers.
5. Swing error packets.
6. Vmaps.
7. Full PathFinder and smoothing.
8. Threat model and enemy targeting.

Important scope rule: stay focused on the current goal, but use judgment. Fix
blockers and safety or data-integrity guardrails when practical. Log useful
follow-ups when they should not be handled immediately.

Gameplay data rule: do not fake or hardcode gameplay values for parity work. Use
DB data, DBC/source-derived values, or CMaNGOS formulas. If the real data source
is not wired yet, leave the behavior unimplemented or narrowly guarded and log
the follow-up.

## Current State

- G1/G2/G3 are regression gates; G3 movement visibility has been user-verified
  in the real client.
- G7 death/release/reclaim/spirit-healer flow is harness-proven and user-smoked.
- G12 shared MapRuntime now owns live player visibility/movement/logout,
  nearby `/say`, shared DB creature snapshots, combat/death/corpse/respawn,
  loot claims, idle/random/waypoint/chase/return-home motion transitions, evade
  prep, assistance-call flags, chase/facing/evade/return-home packets, player
  damage from creatures, and lazy DB creature grid loading.
- G8 has real-data combat foundations: melee outcome ordering, attacker-state
  packet shapes, equipped weapon damage, DB/equipment-backed armor and shield
  block, DB-backed creature attack timers, CMaNGOS-style combined melee reach,
  swing error packets, compatible `VMAP_7.0` LOS guardrails, and explicit MMAP
  path result flags.
- Legacy Rust Guide / Rust Combat Dummy fixture NPCs are disabled in normal
  client startup unless `WORLD_ENABLE_LEGACY_FIXTURE_NPCS` is set.
- Native VMAP/MMAP code is isolated behind safe Rust wrappers and C++ input
  validation/catch-all boundaries. Gameplay code should use those wrappers only.

## Recently Landed G8 PathFinder Slice

- The native MMAP bridge now uses a CMaNGOS-style smooth corridor pass instead
  of returning raw Detour `findStraightPath` corners:
  - `SMOOTH_PATH_STEP_SIZE = 4.0`
  - `SMOOTH_PATH_SLOP = 0.3`
  - steer target lookup
  - `moveAlongSurface`
  - corridor `fixupCorridor`
  - `getPolyHeight` refresh
  - max smooth points raised to 74
- Rust mmap output capacity is now 74 points, matching CMaNGOS
  `MAX_POINT_PATH_LENGTH`.
- Walk monster-move packets no longer set the run-mode spline flag; chase
  facing-target packets still serialize run mode.
- Added tests proving local Northshire mmap data returns multiple smooth
  4-yard-ish path points and that walk/run monster-move flags serialize
  separately.
- User real-client smoke passed for this slice.

## Current Uncommitted G8/G9 Slice

- Chase stop distance now uses the CMaNGOS-shaped combined melee reach between
  the creature model reach and player combat reach, instead of a fixed attack
  distance.
- The VMAP LOS-only straight chase fast path is disabled behind
  `DB_CREATURE_CHASE_STRAIGHT_FAST_PATH_ENABLED = false` because real-client
  smoke showed it made Northshire wolves hover/jitter on uneven terrain by
  reusing the creature's start Z. Keep chase on MMAP-backed paths until the
  fast path can sample terrain/nav height like CMaNGOS.
- Full MMAP chase paths are built to the target, then cut back to the first
  LOS-valid point within melee reach so creatures do not run through or past the
  player.
- When a creature already chasing reaches melee range, the server sends a
  monster-move stop packet before returning to swing handling.
- Added focused tests for dynamic chase stop distance, LOS-valid path cutting,
  and keeping the LOS-backed straight fast path disabled for chase.
- Waypoint movement now buffers short zero-wait DB waypoint legs into one
  longer path until the CMaNGOS 6s minimum path time is reached or a wait node is
  encountered. This makes patrol motion less one-node-at-a-time without adding
  zone-specific behavior.
- MapRuntime now owns an initial DB-creature threat table:
  - aggro/engagement creates a zero-threat entry;
  - player damage adds applied damage as threat;
  - death and clear-combat paths remove threat;
  - tests cover multiple players on one threat list and CMaNGOS 110% melee /
    130% ranged selection thresholds.
- Player damage now applies the MapRuntime threat selection to active creature
  combat. When a different player passes the CMaNGOS melee/ranged switch
  threshold, the creature's authoritative victim changes and the server emits
  attack stop/start packets to the directly affected session and nearby
  observers.
- The current-session combat flag is refreshed on threat switch. Broader threat
  choreography is still intentionally narrow: taunt, healing threat, pets,
  group threat, suppression, and real-client verification are follow-ups.
- Shared-mob desync hardening now has a focused MapRuntime torture test for one
  DB creature: A damages, B observes; B damages, A observes; A kills; B cannot
  damage the corpse; loot money claims once; loot release clears/broadcasts
  corpse flags; corpse expiry and respawn each fire once. MapRuntime player
  removal clears creature combats/threat entries for the logging-out player, and
  DB-creature loot release now returns a shared event with direct and observer
  update packets.
- Follow-up shared authority hardening moved the remaining motion/AI transitions
  behind MapRuntime operations: idle random/waypoint start and advance,
  chase start, stop-on-reach, return-home start/advance, evade reset, and
  assistance-call flags. The old session-local helpers are now test-only. Death
  finalization now carries a map-owned monster-move stop packet when a moving
  creature dies, so observers do not see a dead/corpse creature keep moving.

## Tests Run

- `cargo test -p wow-network db_creature_mmap_path_uses_cmangos_smooth_steps_when_available --lib`
- `cargo test -p wow-network monster_move --lib`
- `cargo test -p wow-network db_creature_chase --lib`
- `cargo test -p wow-network db_creature_waypoint --lib`
- `cargo test -p wow-network db_creature_threat --lib`
- `cargo test -p wow-network map_runtime_db_creature_damage --lib`
- `cargo test -p wow-network db_creature_aggro --lib`
- `cargo test -p wow-network starter_spell --lib`
- `cargo test -p wow-network map_runtime_db_creature_combat --lib`
- `cargo test -p wow-network db_creature_random_motion --lib`
- `cargo test -p wow-network --lib` (`221` tests)
- `cargo test -p wow-network map_runtime_ --lib` (`22` tests)
- `cargo test -p wow-network map_runtime_ --lib` (`25` tests) with
  `CARGO_TARGET_DIR=target\codex-map-motion-authority`
- `cargo test -p wow-network db_creature_ --lib` (`76` tests) with
  `CARGO_TARGET_DIR=target\codex-map-motion-authority`
- `cargo fmt --check`
- `cargo clippy -p wow-network --all-targets -- -D warnings`
- `cargo clippy -p wow-network --all-targets -- -D warnings` with
  `CARGO_TARGET_DIR=target\codex-map-motion-authority`
- `.\scripts\test-rust.cmd` with
  `CARGO_TARGET_DIR=target\codex-path-smoothing-test`
- `.\scripts\test-rust.cmd` with
  `CARGO_TARGET_DIR=target\codex-chase-cutpath-test`
- `.\scripts\test-rust.cmd` with
  `CARGO_TARGET_DIR=target\codex-g9-threat-test`
- `.\scripts\test-rust.cmd` with
  `CARGO_TARGET_DIR=target\codex-threat-switch-test`
- `.\scripts\test-rust.cmd` with
  `CARGO_TARGET_DIR=target\codex-shared-mob-baseline`
- `.\scripts\test-rust.cmd` with
  `CARGO_TARGET_DIR=target\codex-shared-mob-fix`
- `.\scripts\test-rust.cmd` with
  `CARGO_TARGET_DIR=target\codex-map-motion-authority`

Baseline runs can fail if a live auto-restarting client stack holds
`target\debug\authserver.exe`; stop the wrapper/children or use a separate
`CARGO_TARGET_DIR`.

## Known Blockers And Gaps

- Full CMaNGOS PathFinder parity is not done yet:
  - `BuildPolyPath` missing-poly/same-poly/incomplete/force-destination decision
    behavior is still only partially represented.
  - Chase now has fallback full path, LOS-valid path cut, and simple
    stop-on-reach behavior, but still lacks a safe straight-line height-sampling
    fast path, fanning/backpedal, and moving melee leeway.
  - Random movement still chooses a raw XY point at home Z instead of
    CMaNGOS `ComputePathToRandomPoint` height/query behavior.
  - Waypoint movement now buffers short zero-wait paths to the 6s CMaNGOS
    minimum, but it does not yet implement the 1.5s pre-send behavior,
    waypoint scripts, or movement informs.
  - Return-home does not yet implement the CMaNGOS force-destination plus
    shortcut/high-velocity behavior.
- G8 still needs player offensive dodge/parry/block outcomes, offhand/reset/
  queued swing parity, fuller race/model DBC reach, and expanded threat/enemy
  targeting parity beyond direct damage victim switching.
- G12 polish remains: grid unload/idle eviction, broader group/reward
  eligibility, a dedicated two-client logout/relog torture harness around
  combat/corpse/loot states, and more real-client confirmation.
- G10/G11 remain broader red/yellow areas: NPC interaction fidelity and
  persistence/relog sanity across every major starter-zone action.

## Recommended Next Task

Real-client smoke the current uncommitted movement and threat slice:

1. In open Northshire space, wolves should take a direct smooth chase path and
   stop at melee reach instead of running into or past the player.
2. Around trees/fences/buildings, wolves should use MMAP pathing rather than
   cutting through blocked geometry or giving up early.
3. Waypoint/patrol creatures should move through short no-wait waypoint chains
   more smoothly, without one tiny spline per node.
4. Large-reach creatures should stop farther out than normal wolves.
5. When the player steps into reach during an active chase, observers should see
   a stop instead of an overshoot/jitter loop.
6. Regression smoke: Northshire wolves should not hover or snap vertically
   while chasing on uneven terrain; the old VMAP LOS-only straight fast path was
   confirmed as the culprit and is currently disabled.
7. For threat switching: player A pulls a wolf, player B deals enough damage to
   exceed the 130% ranged or 110% melee threshold, and both clients should see
   the wolf stop attacking/chasing A and start attacking/chasing B.
8. After the switch, A should stop taking creature swings, B should receive
   creature swings, and observers should not see duplicate attack start/stop
   spam.
9. Watch logs and clients for excessive stop-packet spam.
10. Kill a moving/chasing creature with another client watching; the observer
   should see the monster-move stop and corpse/death update, with no continued
   patrol/chase motion.

If the smoke is good, commit this slice. The next implementation chunk should be
one of:

- G9 random wander parity: native `ComputePathToRandomPoint`-style helper,
  no-path retry timing, and navmesh height selection; or
- G12 harness polish: dedicated two-client logout/relog torture coverage during
  combat/corpse/loot states plus grid unload/idle eviction; or
- G8 threat parity expansion: healing threat, taunt/fixate-style effects,
  group/pet threat ownership, and closer CMaNGOS victim-selection edge cases.

## Key Files

- `AGENTS.md`
- `docs/playable_gate_board.md`
- `docs/g12_shared_mapruntime_plan.md`
- `crates/wow-network/native/mmap_path.cpp`
- `crates/wow-network/src/world/mmap_path.rs`
- `crates/wow-network/src/world/vmap_los.rs`
- `crates/wow-network/src/world/combat/motion.rs`
- `crates/wow-network/src/world/combat/evade.rs`
- `crates/wow-network/src/world/combat/lifecycle.rs`
- `crates/wow-network/src/world/packet_builders/movement.rs`
- `crates/wow-network/src/world/maps/map/`
- `crates/wow-network/src/world/tests.rs`
- `bins/starter-zone-flow-test/src/main.rs`
- `scripts/run-client-stack-18085.cmd`
- `scripts/test-rust.cmd`
