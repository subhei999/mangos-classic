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
- Latest pushed commit: `b994ed02e` (`[g12] Track MapRuntime grid load state`),
  pushed to `origin/codex/rusty-mangos`.
- Latest local unpushed commit: current `HEAD` after this task
  (`[c2] Fix gameobject login decode`).
- Local branch is intentionally ahead while C2 workstream merges are being
  real-client smoked.
- Always re-run `git status --short --branch` before editing; this handoff may
  lag behind the live worktree.

## Current Goal

Current milestone: **Northshire Human Warrior playable slice with shared
multiplayer state**.

Current user direction: **merge C2 workstreams one at a time, restart the
client stack, and use real-client smoke feedback before taking the next
branch**.

User-observed missing criteria:

1. Quest availability is not filtered by level/class/race/chain/prerequisite.
2. Quest item drops are missing because real loot-table quest drop eligibility
   is not wired.
3. Gameobject quest pickup is missing.
4. Warrior gameplay through level 6 is incomplete: no global cooldown, Heroic
   Strike is toy-shaped, and other warrior spells are not functional.
5. Combat log output is missing.
6. Health regeneration and rage degeneration are missing.
7. Weapon skills and general skill state are missing or stuck at level 1.
8. Aggro/chase/leash behavior is not CMaNGOS-like enough when mobs are hit and
   the player runs beyond the initial radius.
9. NPC patrols start at server launch but stop working after a while.

Recommended branch split now lives in `docs/playable_execution_roadmap.md` and
uses low-overlap branches:

- `codex/c2-northshire-grade`
- `codex/c2-quest-eligibility`
- `codex/c2-quest-loot-drops`
- `codex/c2-gameobject-quests`
- `codex/c2-warrior-spells-gcd`
- `codex/c2-combat-log`
- `codex/c2-regen-rage-ticks`
- `codex/c2-skills-weapon-skill`
- `codex/c2-aggro-leash-parity`
- `codex/c2-patrol-stability`
- `codex/c2-npc-relog-polish`

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
- C2 integration has landed the Northshire grading checklist, combat log packet
  helpers, quest eligibility, and active quest item loot drops. The quest
  eligibility merge needed follow-up fixes for real-client quest flow:
  - active/incomplete/reward quests are separated from newly available quests;
  - abandon clears the quest-log slot and allows reaccepting the abandoned row;
  - `SrcItemId/SrcItemCount` source items are granted on accept;
  - objective-free and source-item delivery quests can complete on accept;
  - quest reward completion packets now match the vanilla success/reward shape.
- Quest loot now reads negative `ChanceOrQuestChance` rows and selects a quest
  drop only when the player has an active incomplete item objective needing
  that item. Looted quest items now re-check active item objectives, mark newly
  satisfied quests complete, and allow already-satisfied item quests to show and
  turn in even if the DB row was still incomplete. Full CMaNGOS loot-table
  rolling remains issue #58.
- Quest reward turn-in now resolves reward item display IDs from
  `item_template`, grants the selected choice reward plus fixed reward items,
  checks backpack space including stacks freed by required-item turn-in, and
  consumes required quest items on successful reward.
- Gameobject quest interaction has been merged:
  - nearby DB gameobjects stream on login and movement;
  - `CMSG_GAMEOBJECT_QUERY` returns DB-backed template data;
  - `CMSG_GAMEOBJ_USE` gates interaction by map/range/flags/required active
    quest;
  - gameobject questgivers reuse the shared quest list/reward helpers;
  - negative `ReqCreatureOrGOId` objectives award gameobject-use credit and
    encode the high-bit gameobject objective in quest progress packets.
- Follow-up login kick fix: gameobject visibility SQL now explicitly casts
  `gameobject_addon.state` / `animprogress` COALESCE expressions so MySQL
  does not decode the fallback as `DECIMAL` during character login.

## Recently Landed G8/G9 Context

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

- Retargeted `docs/playable_execution_roadmap.md` around the user's nine
  Northshire playability gaps.
- Replaced broad branch buckets with narrower low-overlap branches for quest
  eligibility, quest loot, gameobjects, warrior spells/GCD, combat log,
  regen/rage, skills, aggro/leash, patrol stability, NPC/relog polish, and a
  Northshire grading harness.
- Updated `docs/playable_gate_board.md` priority order to match this plan.

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
- `cargo test -p wow-network quest --lib` passed (`12` tests) for the quest
  reaccept/completion fix.
- `cargo test -p wow-network --lib` passed (`257` tests).
- `.\scripts\test-rust.cmd` with
  `CARGO_TARGET_DIR=target\codex-quest-reaccept-fix-test` passed.
- `cargo test -p wow-network quest_loot --lib` passed after merging
  `codex/c2-quest-loot-drops`.
- `cargo test -p wow-network loot --lib` passed.
- `.\scripts\test-rust.cmd` with
  `CARGO_TARGET_DIR=target\codex-merge-quest-loot-test` passed.
- `cargo test -p wow-network quest --lib` passed (`15` tests) after fixing
  item-loot quest completion.
- `cargo test -p wow-network loot --lib` passed.
- `.\scripts\test-rust.cmd` with
  `CARGO_TARGET_DIR=target\codex-quest-item-complete-test` passed.
- `cargo fmt --check` passed after the quest reward item fix.
- `cargo test -p wow-network quest --lib` passed (`17` tests) after the quest
  reward item fix.
- `cargo test -p wow-network inventory --lib` passed (`16` tests).
- `.\scripts\test-rust.cmd` with
  `CARGO_TARGET_DIR=target\codex-quest-reward-items-test` passed.
- `cargo test -p wow-network gameobject --lib` passed (`4` tests) after
  merging `codex/c2-gameobject-quests`.
- `cargo test -p wow-network quest --lib` passed (`18` tests) after resolving
  quest/gameobject merge overlap.
- `cargo test -p wow-network --lib` passed (`266` tests).
- `cargo test -p wow-db gameobject --lib` passed (`0` filtered tests).
- `.\scripts\test-rust.cmd` with
  `CARGO_TARGET_DIR=target\codex-merge-gameobjects-test` passed.
- `cargo fmt --check` passed after fixing gameobject login decode.
- `cargo test -p wow-network gameobject --lib` passed (`4` tests) after
  fixing gameobject login decode.

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
- GitHub issue #58 tracks full CMaNGOS creature loot-table rolling beyond the
  current active quest item-drop bridge.
- GitHub issue #59 tracks moving DB gameobject consumed/respawn state from
  per-session storage into shared `MapRuntime`/world state for multiplayer
  consistency.

## Recommended Next Task

Restart the client stack and real-client smoke gameobject quests plus the
reward regressions: confirm nearby Northshire gameobjects render/query, an
object-use quest such as Milly's Harvest advances from object clicks, reward
items still appear with real icons, and required quest items still disappear on
turn-in. If that passes, continue with `codex/c2-skills-weapon-skill` before
`codex/c2-warrior-spells-gcd`.

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
