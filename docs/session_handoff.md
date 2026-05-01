# Session Handoff

This is the short operating brief for the next Rust migration session. Keep it
pruned. Do not append a chronological log.

Durable roadmap belongs in `docs/rust_migration_plan.md`; gate status belongs
in `docs/playable_gate_board.md`; detailed G12 design belongs in
`docs/g12_shared_mapruntime_plan.md`; auth setup belongs in
`docs/rust_auth_foundation.md`.

## Handoff Rules

- Keep only current branch state, active goal, current confidence, blockers,
  recommended next task, and key files.
- Replace stale details instead of appending endlessly.
- Use GitHub issues for non-blocking P2/P3/P4 discoveries when available.
- If this file becomes long again, prune it before doing new feature work.

## Current Branch And Worktree

- Branch: `codex/rusty-mangos`.
- Latest pushed checkpoint commit: `6cb0632de` (`Reshape world runtime ownership and shared gameplay systems`).
- Current worktree is intentionally dirty with uncommitted follow-up work across
  G12 shared MapRuntime, G8 combat fidelity, G9 creature movement, harnesses,
  docs, and script updates.
- Important dirty areas include:
  - `crates/wow-network/src/world/combat/` split files plus `combat.rs` facade.
  - `crates/wow-network/src/world/maps/{grid,map,map_manager}.rs` shared
    MapRuntime authority, after deleting the old `maps/runtime.rs` staging
    file.
  - `crates/wow-network/src/world/server/world_session.rs`,
    `entities/update_data.rs`, `fixtures/legacy_npcs.rs`, `inventory.rs`,
    `death.rs`, `spells.rs`, `packet_builders.rs`, and `tests.rs`, after
    deleting the old `bootstrap.rs` staging file.
  - `crates/wow-db/src/world_data.rs` creature template/spawn loader changes.
  - `bins/starter-zone-flow-test` and `bins/world-flow-test` harness updates.
  - `docs/playable_gate_board.md`, `docs/g12_shared_mapruntime_plan.md`, and
    `AGENTS.md`.

Before destructive cleanup, inspect the dirty worktree carefully. Do not revert
unrelated changes unless the user explicitly asks.

## Current Goal

Current milestone: **Northshire Human Warrior playable slice**.

Current user direction: continue toward a faithful, shared, crash-resistant
CMaNGOS-style Rust worldserver. G12 multiplayer/shared MapRuntime is the major
architecture push, and current active implementation work has shifted back into
the G8 combat-fidelity ladder.

Near-term G8 order requested by the user:

1. Real melee roll table.
2. Real swing timers.
3. Real damage formula.
4. Reach/model modifiers.
5. Swing error packets.
6. Vmaps.
7. Full PathFinder and smoothing.
8. Threat model and enemy targeting.

Current principle: no fake or hardcoded gameplay values for combat math. Use DB,
DBC/source-derived CMaNGOS formulas, or explicitly leave the gap unimplemented
and logged.

## Current State

- G1/G2/G3 are regression gates. G3 movement visibility is user-verified in the
  real client.
- G7 death/release/reclaim/spirit-healer flow is harness-proven and
  user-smoked.
- G12 shared MapRuntime is substantially implemented:
  - sessions use outbound channels and a session registry;
  - live players register in shared map state;
  - nearby players see spawn, movement, logout destroy, equipment visuals, and
    player attack/spell packets;
  - `/say` broadcasts through nearby map visibility;
  - DB creature snapshots, damage, death, corpse/respawn lifecycle, loot claims,
    combat claims, chase/facing/evade/return-home packets, and creature-origin
    player damage route through shared `MapRuntime`;
  - DB creatures lazy-load by CMaNGOS-shaped grid rectangles and movement
    visibility no longer depends on DB radius queries every heartbeat.
- G8 combat fidelity now has a first real-data pass:
  - melee outcome core supports miss, dodge, parry, block, glancing, crit,
    crushing, and normal outcomes;
  - attacker-state packets serialize non-hit and blocked outcomes;
  - player auto-attacks use equipped main-hand damage plus class/stat attack
    power and target creature DB template armor;
  - player auto-attack rescheduling uses equipped main-hand delay instead of the
    old Rust dummy 2s fallback;
  - DB creature attack rescheduling uses
    `creature_template.MeleeBaseAttackTime`;
  - player/creature melee reach uses CMaNGOS-style combined combat reach:
    attacker reach + victim reach + base melee offset, clamped to at least 5yd;
  - DB creature create blocks and reach checks use `creature_model_info`
    bounding radius/combat reach through the selected template display id;
  - player melee failures now send empty Vanilla swing-error packets for
    out-of-range, bad-facing, dead-target, and can't-attack cases;
  - CMaNGOS `VMAP_7.0` static LOS can be loaded through a safe Rust wrapper over
    a native CMaNGOS vmap bridge, and DB-creature navigation now gates
    melee/starter melee spell validity on that LOS when compatible vmaps are
    present;
  - local `C:\World of Warcraft Classic\vmaps` has been replaced with generated
    CMaNGOS `VMAP_7.0` data after backing up the previous folder, and server
    startup logs prove `vmap_maps=43` / `vmap_tiles=1249` plus native VMAP loads;
  - starter melee spell failures now return CMaNGOS'
    `SPELL_FAILED_LINE_OF_SIGHT` when the navigation guardrail reports blocked
    LOS;
  - the native MMAP bridge now requests up to 32 Detour straight-path points,
    matching the bridge capacity, and reports explicit Rust path statuses
    instead of flattening native results into only `Some(points)` / `None`;
  - creature path building now carries CMaNGOS-shaped path flags for
    `NORMAL`, `INCOMPLETE`, `NOPATH`, and `NOT_USING_PATH`: missing/disabled
    mmap data keeps the permissive straight fallback as
    `NORMAL | NOT_USING_PATH`, while advertised-but-unloadable mmap data is treated as a
    real `NOPATH` and does not collapse to a through-geometry shortcut;
  - active creature chase path creation no longer requires clear VMAP LOS, so
    mobs can keep pathing around static geometry while LOS still gates aggro,
    melee reach, and starter melee spell validity;
  - creature motion stop now uses the CMaNGOS `MonsterMoveStop` packet shape
    instead of sending a one-point move-to-self path;
  - DB creature template loaders carry combat fields such as armor, AP, damage
    ranges, multipliers, mana, and damage school;
  - incoming creature swings use live player defensive stats from
    `PlayerCombatStats`, including DB/equipment-backed armor and shield block
    value.
- `PlayerCombatStats` now carries `armor` and `shield_block_value`. Armor is
  agility-derived physical resistance plus equipped item armor. Shield block
  value comes from equipped offhand shield `item_template.block` plus the
  CMaNGOS-style strength component from `PlayerWorldStats`.
- Legacy packet-only Rust fixture NPCs are disabled in normal client startup
  unless explicitly requested by script/env. The old `world-flow` harness still
  enables them for fixture tests.
- The native mmap bridge is behind safe Rust wrappers and C++ validation. Direct
  gameplay code should use the safe wrapper only.
- The native vmap bridge is also behind safe Rust wrappers and validates input
  plus catches C++ exceptions. It intentionally activates only for compatible
  CMaNGOS `VMAP_7.0` files.
- Cleanup note: the old live `bootstrap.rs` and `maps/runtime.rs` staging files
  have been drained and deleted. Login/bootstrap packet code now lives in
  `server/world_session.rs`; generic `SMSG_UPDATE_OBJECT` orchestration remains
  in `entities/update_data.rs`; player, creature, corpse, and item update
  builders live in their matching `entities/*` files; shared map ownership is
  split across `maps/grid.rs`, `maps/map.rs`, and `maps/map_manager.rs`; the
  old giant `world/mod.rs` server flow is split into focused files under
  `server/`; `packet_builders.rs` is now an include hub for packet-family
  builders under `packet_builders/`; `maps/map.rs` owns only the shared runtime
  types/new constructor and delegates player, creature, loot, combat, lifecycle,
  and spatial helpers to `maps/map/*`; harness-only Rust Guide / Rust Combat
  Dummy create-block helpers live in `fixtures/legacy_npcs.rs` behind
  `WORLD_ENABLE_LEGACY_FIXTURE_NPCS`.

## Current Confidence

Latest passing checks for the current combat-stat/shared-runtime/pathing work:

- `cargo fmt --check`
- `cargo test -p wow-network db_creature_navigation_guardrail_blocks_aggro_and_melee_but_not_chase_pathing --lib`
- `cargo test -p wow-network monster_move_stop_uses_cmangos_stop_shape --lib`
- `cargo test -p wow-network db_creature_path_uses_straight_fallback_only_when_mmap_unavailable --lib`
- `cargo test -p wow-network db_creature_mmap_path_corner_uses_local_detour_data_when_available --lib`
- `cargo test -p wow-network map_runtime --lib`
- `cargo test -p wow-network rust_guide_create_block_has_gossip_unit_fields --lib`
- `cargo test -p wow-network rust_combat_dummy_create_block_has_hostile_unit_fields --lib`
- `cargo test -p wow-network self_spawn_update_includes_cmangos_player_vitals_and_defaults --lib`
- `cargo test -p wow-network --lib` (`210` tests)
- `cargo clippy -p wow-network --all-targets -- -D warnings`
- `.\scripts\test-rust.cmd` with `CARGO_TARGET_DIR=target\codex-entity-split-test`
- `.\scripts\test-rust.cmd` with `CARGO_TARGET_DIR=target\codex-clean-split-test`
- `cargo clippy -p wow-network -p worldserver --all-targets -- -D warnings`
- `cargo test -p wow-network melee_reach --lib`
- `cargo test -p wow-network swing_error --lib`
- `cargo test -p wow-network player_swing_timer --lib`
- `cargo test -p wow-network db_creature_swing_timer --lib`
- `cargo test -p wow-network db_creature_create_block --lib`
- `cargo test -p wow-network db_creature_player_melee_check --lib`
- `cargo test -p wow-db --lib`
- `.\scripts\test-rust.cmd` with `CARGO_TARGET_DIR=target\codex-test`
- `.\scripts\test-rust.cmd` with `CARGO_TARGET_DIR=target\codex-pathfinder-test`
- `.\scripts\test-rust.cmd` with `CARGO_TARGET_DIR=target\codex-cleanhouse-test`

Baseline runs can fail if a live auto-restarting client stack holds
`target\debug\authserver.exe`; either stop the wrapper/children or use a
separate `CARGO_TARGET_DIR`.

## Known Blockers And Gaps

- G8 combat still lacks full CMaNGOS parity:
  - player offensive dodge/parry/block outcomes need real target state,
    facing/arc rules, and skills/auras;
  - swing timers now use DB/equipment delays, but reset/queue/offhand behavior
    still needs fuller CMaNGOS parity;
  - reach/model modifiers now use DB-backed model reach for creatures, but
    moving melee leeway and full player race/model DBC reach loading are still
    future parity;
  - compatible CMaNGOS vmap LOS is wired and local `VMAP_7.0` data is now
    installed, but real-client wall/terrain LOS behavior still needs a focused
    smoke proof;
  - full CMaNGOS PathFinder smoothing remains future work: Rust now preserves
    the first useful path-result flags, but still uses Detour straight-path
    corners rather than the full CMaNGOS corridor smoothing, smooth-step
    fixups, random-point query parity, and caller-specific flag handling;
  - threat model and enemy targeting are not implemented yet.
- G12 remaining multiplayer polish:
  - grid unload/idle eviction;
  - loot-flag observer polish after claims;
  - broader group/reward eligibility;
  - more real-client confirmation of player-facing visuals and lazy grid
    loading.
- G9 creature fidelity still needs true pathfinder random points, follower
  formation movement, broader real-client zone proof, and additional patrol
  polish.
- G10/G11 remain broader red/yellow areas: NPC interaction fidelity and
  persistence/relog sanity across every major starter-zone action.
- The repo still relies on local `target/classic-db` / Docker content import for
  full ClassicDB Northshire data.

## Recommended Next Task

Next implementation chunk: **G8 full PathFinder/smoothing real-client proof**.

Suggested scope:

- Build on the new Rust path-result flags and audit CMaNGOS `PathFinder`
  corridor smoothing, smooth-step constants, and caller-specific flag handling
  for chase, random movement, waypoints, and return-home.
- Replace the current thin Detour straight-path corner use with a more faithful
  smoothing layer, then smoke wolves around trees/fences in the real client.
- Real-client smoke the current installed `VMAP_7.0` data with an open-terrain
  combat check plus one wall/terrain LOS check.
- Timer parity remains a useful side branch: audit offhand/reset/queued-swing
  behavior and add tests for base/offhand separation plus retry timing.
- Then run `cargo test -p wow-network --lib`, clippy, and `.\scripts\test-rust.cmd`.

Use subagents only if the work splits cleanly. Good split: one 5.3-Codex worker
researches CMaNGOS timer references while the parent implements/reviews, or one
worker writes focused tests in a disjoint file scope. Keep the parent agent as
architect/integrator.

## Key Files

- `AGENTS.md`
- `docs/playable_gate_board.md`
- `docs/g12_shared_mapruntime_plan.md`
- `crates/wow-network/build.rs`
- `crates/wow-network/native/vmap_los.cpp`
- `crates/wow-network/src/world/vmap_los.rs`
- `crates/wow-network/src/world/maps/world_data.rs`
- `crates/wow-network/src/world/maps/grid.rs`
- `crates/wow-network/src/world/maps/map.rs`
- `crates/wow-network/src/world/maps/map/`
- `crates/wow-network/src/world/maps/map_manager.rs`
- `crates/wow-network/src/world/server/world_session.rs`
- `crates/wow-network/src/world/server/session_loop.rs`
- `crates/wow-network/src/world/server/character_screen.rs`
- `crates/wow-network/src/world/server/player_login.rs`
- `crates/wow-network/src/world/server/logout.rs`
- `crates/wow-network/src/world/server/movement.rs`
- `crates/wow-network/src/world/server/visibility.rs`
- `crates/wow-network/src/world/entities/update_data.rs`
- `crates/wow-network/src/world/entities/item.rs`
- `crates/wow-network/src/world/entities/corpse.rs`
- `crates/wow-network/src/world/fixtures/legacy_npcs.rs`
- `crates/wow-network/src/world/combat.rs`
- `crates/wow-network/src/world/combat/`
- `crates/wow-network/src/world/entities/creature.rs`
- `crates/wow-network/src/world/entities/player.rs`
- `crates/wow-network/src/world/inventory.rs`
- `crates/wow-network/src/world/spells.rs`
- `crates/wow-network/src/world/death.rs`
- `crates/wow-network/src/world/packet_builders.rs`
- `crates/wow-network/src/world/packet_builders/`
- `crates/wow-network/src/world/tests.rs`
- `crates/wow-db/src/world_data.rs`
- `bins/starter-zone-flow-test/src/main.rs`
- `bins/world-flow-test/src/main.rs`
- `scripts/test-rust.cmd`
- `scripts/test-starter-zone-flow.cmd`
- `scripts/run-client-stack-18085.cmd`
