# Session Handoff

This is the short operating brief for the next Rust migration session. Keep it
pruned. Do not append a chronological log.

Durable roadmap belongs in `docs/rust_migration_plan.md`; gate status belongs
in `docs/playable_gate_board.md`; branchable Checkpoint 2 execution planning
belongs in `docs/playable_execution_roadmap.md`; detailed G12 design belongs in
`docs/g12_shared_mapruntime_plan.md`; auth setup belongs in
`docs/rust_auth_foundation.md`.

## Current Branch And Worktree

- Branch: `codex/rusty-mangos`.
- Latest pushed commit reported by prior handoff: `3134dadc4`
  (`[opcode] Handle selection and channel joins`), pushed to
  `origin/codex/rusty-mangos`.
- Worktree at this handoff update:
  - current G12 grid-scalability Rust slice:
    `crates/wow-network/src/world/maps/grid.rs`,
    `crates/wow-network/src/world/maps/map/creature_combat.rs`,
    `crates/wow-network/src/world/maps/map/creature_damage.rs`,
    `crates/wow-network/src/world/maps/map/creature_lifecycle.rs`,
    `crates/wow-network/src/world/maps/map/creature_loot.rs`,
    `crates/wow-network/src/world/maps/map/creature_motion.rs`,
    `crates/wow-network/src/world/maps/map/creature_snapshots.rs`,
    `crates/wow-network/src/world/maps/map/players.rs`,
    `crates/wow-network/src/world/maps/map_manager.rs`, and
    `crates/wow-network/src/world/tests.rs`;
  - roadmap/doc edits in progress:
    `docs/playable_execution_roadmap.md`,
    `docs/playable_gate_board.md`, `docs/rust_migration_plan.md`, and this
    file.
- Always re-run `git status --short --branch` before editing; this handoff may
  lag behind the live worktree.

## Current Goal

Current milestone: **Northshire Human Warrior playable slice with shared
multiplayer state**.

Current user direction: **turn lazy DB creature grid loading into a measured
G12 scalability gate**. Movement visibility must stay grid-load-driven instead
of silently regressing to DB-query-per-heartbeat, and `MapRuntime` should be
ready for safe idle/unload lifecycle work.

Near-term integration lane:

1. Finish and verify the current G12 grid-load measurement slice.
2. Commit it with the existing active-mover test additions in
   `crates/wow-network/src/world/tests.rs` if desired.
3. Next implementation target: actual idle-grid unload/eviction policy that
   respects combat, loot, corpse, respawn, and motion timers.
4. Keep shared `MapRuntime` authority intact while later G8/G9/G10/G11
   fidelity branches build on this.

Important scope rule: stay focused on the current goal, but use judgment. Fix
blockers and safety or data-integrity guardrails when practical. Log useful
follow-ups when they should not be handled immediately.

Gameplay data rule: do not fake or hardcode gameplay values for parity work.
Use DB data, DBC/source-derived values, or CMaNGOS formulas. If the real data
source is not wired yet, leave the behavior unimplemented or narrowly guarded
and log the follow-up.

## Current State

- G1/G2/G3 are regression gates; G3 movement visibility has been user-verified
  in the real client.
- G7 death/release/reclaim/spirit-healer flow is harness-proven and user-smoked.
- G12 shared `MapRuntime` is substantially implemented:
  - live player visibility/movement/logout;
  - nearby `/say`;
  - shared DB creature snapshots;
  - shared creature combat/death/corpse/respawn;
  - loot claims/release updates;
  - lazy DB creature grid loading;
  - observer broadcasts for movement, combat, death, loot, and player visuals.
- Current G12 grid scalability slice adds:
  - regression-tested grid-load counters proving repeated movement in a loaded
    area stays at one DB rectangle load;
  - regression-tested crossing into one unloaded grid adds exactly one DB query;
  - regression-tested nearby players reuse already loaded grids;
  - explicit grid states for `Loaded`, `Active`, `Idle`, and
    `UnloadBlocked(Combat/Loot/Corpse/Timer)`;
  - grid-state refreshes on player movement/logout, combat claim/clear,
    creature damage/death, loot state changes, and lifecycle/motion updates;
  - cell-bucket reindex coverage for creature movement, return-home, corpse
    expiry, and respawn-style home repositioning.
- G8 has real-data combat foundations:
  - melee outcome ordering and attacker-state packet shapes;
  - equipped weapon damage;
  - DB/equipment-backed armor and shield block;
  - DB-backed creature attack timers;
  - CMaNGOS-style combined melee reach;
  - swing error packets;
  - compatible `VMAP_7.0` LOS guardrails;
  - explicit MMAP path result flags and smoothing.
- `CMSG_SET_SELECTION` and minimal `CMSG_JOIN_CHANNEL` are implemented with
  known follow-ups for full reputation/mover mirroring and full channel state.
- Legacy Rust Guide / Rust Combat Dummy fixture NPCs are disabled in normal
  client startup unless `WORLD_ENABLE_LEGACY_FIXTURE_NPCS` is set.
- Native VMAP/MMAP code is isolated behind safe Rust wrappers and C++ input
  validation/catch-all boundaries. Gameplay code should use those wrappers only.

## Current Uncommitted G8/G9 Slice To Smoke

- Chase stop distance uses CMaNGOS-shaped combined melee reach instead of a
  fixed attack distance.
- The VMAP LOS-only straight chase fast path is disabled because real-client
  smoke showed it caused hover/jitter on uneven Northshire terrain.
- Full MMAP chase paths are built to the target, then cut back to the first
  LOS-valid point within melee reach.
- Creatures already chasing send a monster-move stop packet when they reach
  melee range before returning to swing handling.
- Waypoint movement buffers short zero-wait DB waypoint legs into one longer
  path until the CMaNGOS 6s minimum path time is reached or a wait node appears.
- MapRuntime owns an initial DB-creature threat table:
  - aggro creates a zero-threat entry;
  - player damage adds threat;
  - death and clear-combat remove threat;
  - victim switching follows CMaNGOS 110% melee / 130% ranged thresholds.
- Shared-mob hardening includes a torture test shape for A/B damage observation,
  corpse/death, no post-death damage, one loot claim, loot-release update,
  single corpse expiry, and single respawn.
- Remaining motion/AI transitions were moved behind MapRuntime operations:
  idle random/waypoint start and advance, chase start, stop-on-reach,
  return-home start/advance, evade reset, and assistance-call flags.

## Roadmap Update From This Session

- Added `docs/playable_execution_roadmap.md`.
- Updated `docs/playable_gate_board.md` to point to the execution roadmap and
  reflect the current priority: stabilize the in-flight G8/G9 slice, then use
  branchable workstreams.
- Updated `docs/rust_migration_plan.md` with a Checkpoint 2 pointer to the new
  execution roadmap.

The new roadmap defines:

- the immediate integration lane;
- Checkpoint 2 finish line;
- phases A-E from shared combat stabilization through relog sanity;
- parallel worker streams with suggested `codex/` branch names;
- branch/merge strategy and conflict hot spots;
- worker contracts and proof requirements;
- final real-client closure pass.

## Tests Run

- `cargo test -p wow-network map_runtime_lazy_creature_grid_tracks_loaded_grids_and_nearby_snapshots --lib`
- `cargo test -p wow-network map_runtime_grid_load_counters --lib`
- `cargo test -p wow-network map_runtime_creature_cell_buckets_follow_move_return_home_and_lifecycle --lib`
- `cargo test -p wow-network map_runtime_grid_states_prepare_idle_and_unload_blockers --lib`
- `cargo test -p wow-network map_runtime_ --lib` (`30` tests)
- `cargo test -p wow-network db_creature_ --lib` (`76` tests)
- `cargo fmt --check`
- `cargo fmt --check` during commit/push prep.
- `cargo test -p wow-network --lib` (`246` tests)
- `cargo clippy -p wow-network --all-targets -- -D warnings`
- `.\scripts\test-rust.cmd` first reached the known Windows locked
  `target\debug\authserver.exe` failure after tests passed.
- `.\scripts\test-rust.cmd` with
  `CARGO_TARGET_DIR=target\codex-grid-load-test` passed.
- `.\scripts\test-rust.cmd` with
  `CARGO_TARGET_DIR=target\codex-commit-push-test` passed during commit/push
  prep.

Baseline runs can fail if a live auto-restarting client stack holds
`target\debug\authserver.exe`; stop the wrapper/children or use a separate
`CARGO_TARGET_DIR`.

## Known Blockers And Gaps

- Full CMaNGOS PathFinder parity is not done:
  - missing-poly/same-poly/incomplete/force-destination behavior is partial;
  - chase still lacks a safe straight-line height-sampling fast path,
    fanning/backpedal, and moving melee leeway;
  - random movement still needs CMaNGOS-style navmesh height/query behavior;
  - waypoint movement lacks 1.5s pre-send, scripts, and movement informs;
  - return-home lacks force-destination plus shortcut/high-velocity behavior.
- G8 still needs fuller offensive outcome rules, offhand/reset/queued swing
  parity, race/model DBC reach, and expanded threat/enemy targeting.
- G12 polish remains: actual grid unload/idle eviction, broader group/reward
  eligibility, dedicated two-client logout/relog torture coverage, and more
  real-client confirmation. The unload blockers now have an explicit state
  shape, but no eviction loop has been implemented yet.
- G10/G11 remain broader red/yellow areas: NPC interaction fidelity and
  persistence/relog sanity across every major starter-zone action.

## Recommended Next Task

Implement the first real idle-grid unload/eviction slice:

1. Decide the CMaNGOS-shaped idle delay and unload eligibility boundary.
2. Add a map/runtime sweep that marks loaded grids idle, then unloads only when
   no active players and no `UnloadBlocked` reason remains.
3. Preserve or persist state correctly for combat, corpse, loot, respawn, and
   active motion/timer cases.
4. Add tests proving an idle clean grid unloads, blocked grids do not unload,
   and returning players reload from DB/runtime state without duplicate spawns.
5. Run `cargo test -p wow-network map_runtime_ --lib`,
   `cargo test -p wow-network db_creature_ --lib`, and
   `.\scripts\test-rust.cmd`.

## Key Files

- `AGENTS.md`
- `docs/playable_execution_roadmap.md`
- `docs/playable_gate_board.md`
- `docs/g12_shared_mapruntime_plan.md`
- `docs/briefs/combat.md`
- `crates/wow-network/native/mmap_path.cpp`
- `crates/wow-network/src/world/mmap_path.rs`
- `crates/wow-network/src/world/vmap_los.rs`
- `crates/wow-network/src/world/combat/`
- `crates/wow-network/src/world/maps/map/`
- `crates/wow-network/src/world/maps/map.rs`
- `crates/wow-network/src/world/maps/map_manager.rs`
- `crates/wow-network/src/world/tests.rs`
- `bins/starter-zone-flow-test/src/main.rs`
- `scripts/run-client-stack-18085.cmd`
- `scripts/test-rust.cmd`
