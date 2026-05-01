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
- Current worktree has uncommitted G8 PathFinder/smoothing work:
  - `crates/wow-network/native/mmap_path.cpp`
  - `crates/wow-network/src/world/mmap_path.rs`
  - `crates/wow-network/src/world/packet_builders/movement.rs`
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
  loot claims, chase/facing/evade/return-home packets, player damage from
  creatures, and lazy DB creature grid loading.
- G8 has real-data combat foundations: melee outcome ordering, attacker-state
  packet shapes, equipped weapon damage, DB/equipment-backed armor and shield
  block, DB-backed creature attack timers, CMaNGOS-style combined melee reach,
  swing error packets, compatible `VMAP_7.0` LOS guardrails, and explicit MMAP
  path result flags.
- Legacy Rust Guide / Rust Combat Dummy fixture NPCs are disabled in normal
  client startup unless `WORLD_ENABLE_LEGACY_FIXTURE_NPCS` is set.
- Native VMAP/MMAP code is isolated behind safe Rust wrappers and C++ input
  validation/catch-all boundaries. Gameplay code should use those wrappers only.

## Current Uncommitted G8 PathFinder Slice

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

## Tests Run For Current Uncommitted Slice

- `cargo test -p wow-network db_creature_mmap_path_uses_cmangos_smooth_steps_when_available --lib`
- `cargo test -p wow-network monster_move --lib`
- `cargo test -p wow-network --lib` (`212` tests)
- `cargo fmt --check`
- `cargo clippy -p wow-network --all-targets -- -D warnings`
- `.\scripts\test-rust.cmd` with
  `CARGO_TARGET_DIR=target\codex-path-smoothing-test`

Baseline runs can fail if a live auto-restarting client stack holds
`target\debug\authserver.exe`; stop the wrapper/children or use a separate
`CARGO_TARGET_DIR`.

## Known Blockers And Gaps

- Full CMaNGOS PathFinder parity is not done yet:
  - `BuildPolyPath` missing-poly/same-poly/incomplete/force-destination decision
    behavior is still only partially represented.
  - Chase lacks the CMaNGOS cheap straight-line LOS attempt, fallback full path,
    LOS-valid `CutPath`, premature stop before overshooting the target,
    fanning/backpedal, and moving melee leeway.
  - Random movement still chooses a raw XY point at home Z instead of
    CMaNGOS `ComputePathToRandomPoint` height/query behavior.
  - Waypoint movement still sends one node at a time instead of buffering at
    least 6s of path and pre-sending the next segment.
  - Return-home does not yet implement the CMaNGOS force-destination plus
    shortcut/high-velocity behavior.
- G8 still needs player offensive dodge/parry/block outcomes, offhand/reset/
  queued swing parity, fuller race/model DBC reach, and threat/enemy targeting.
- G12 polish remains: grid unload/idle eviction, loot-flag observer polish after
  claims, broader group/reward eligibility, and more real-client confirmation.
- G10/G11 remain broader red/yellow areas: NPC interaction fidelity and
  persistence/relog sanity across every major starter-zone action.

## Recommended Next Task

Finish this G8 PathFinder/smoothing slice:

1. Real-client smoke with two clients in Northshire:
   - wolves walk/chase around trees or fences instead of cutting or stopping;
   - random/waypoint walking looks like walking, not run-flag animation;
   - chase still reaches melee without jittery repath spam;
   - death stop and return-home are observed consistently by both clients;
   - VMAP LOS still blocks can-see/can-hit while MMAP handles how to walk.
2. If the smoke is good, commit this slice.
3. Next implementation chunk after the smoke: CMaNGOS chase caller behavior
   (`CutPath`, straight-line fast path, fallback full path, overshoot stop).

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
