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
- Latest pushed/local `HEAD`: `4f5ceaf23` (`[g12] Move patrol tick to map
  loop`).
- Current uncommitted ownership/performance migration: production gameplay
  ownership is now moved to shared `MapRuntime`/`PlayerRuntime`/`ObjectMgr`
  surfaces rather than `WorldSessionState`:
  - DB-creature runtime, combat claims/timers, threat, damage, evade/chase,
    facing, death, loot, and lifecycle reads/writes are map-owned;
  - player auto-attack target/timer and mutable player gameplay snapshots are
    map-owned `PlayerRuntime` state;
  - patrol/lifecycle motion already runs from the 100ms map tick, and this
    pass preserves DB-creature `visible_objects` during player movement so
    map-owned patrol moves can send the correct create-before-move or movement
    update;
  - DB gameobject loot claim/open/autostore/release state is now owned by
    `MapRuntime`, and the old session gameobject-loot fields were removed;
  - player mutable gameplay state is mirrored into `PlayerRuntime` after
    opcodes/ticks and read back from the map snapshot for logout/movement
    persistence;
  - quest templates, questgiver relations, quest-chain checks, and loot
    templates are cached in shared `ObjectMgr` instead of queried per session;
  - player corpse visibility loads DB corpse grids into `MapRuntime` once and
    movement/login stream from map snapshots;
  - DB creature corpse loot generation is guarded by map-owned corpse state so
    the first opener generates from real loot data and later opens reuse the
    map-owned loot;
  - player combat stats/equipment-derived damage are cached in `PlayerRuntime`
    and refreshed on equipment changes instead of reloading templates during
    combat swings;
  - DB-creature visibility create/destroy staging now mutates the map-owned
    player visible-object set;
  - the old `WorldSessionState` creature/combat mirrors
    (`db_creatures`, `active_creature_combats`, `active_combat_target`,
    `active_combat_next_swing_at`) are `#[cfg(test)]` compatibility shims only.
- Local branch is intentionally ahead while C2 workstream merges are being
  real-client smoked.
- Live client stack was rebuilt/restarted after the final session-eradication
  patch and the login-load-screen fix: authserver PID `38596` on
  `127.0.0.1:13724`, worldserver PID `20568`
  on `127.0.0.1:18085`, logs `auth-client-13724.log` and
  `world-client-18085.log`. Auto-restart is disabled for this run.
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
  - nearby DB gameobjects stream on login and movement from shared
    `MapRuntime` grid/cell buckets;
  - `CMSG_GAMEOBJECT_QUERY` returns DB-backed template data;
  - `CMSG_GAMEOBJ_USE` gates interaction by shared map state, map/range/flags,
    and required active quest;
  - gameobject questgivers reuse the shared quest list/reward helpers;
  - negative `ReqCreatureOrGOId` objectives award gameobject-use credit and
    encode the high-bit gameobject objective in quest progress packets.
  - consumable quest objects update shared consumed/respawn state and destroy
    for nearby observers instead of staying session-owned.
- Follow-up login kick fix: gameobject visibility SQL now explicitly casts
  `gameobject_addon.state` / `animprogress` COALESCE expressions so MySQL
  does not decode the fallback as `DECIMAL` during character login.
- Session-to-map ownership correction completed locally for production code:
  - combat decisions now query map-owned DB-creature state for liveness,
    melee range/LOS/facing validation, and active attacker snapshots;
  - gameobject loot windows and available quest-loot items are shared
    map-owned state, so two sessions cannot independently own the same chest
    loot item;
  - player corpse visibility now loads DB-derived corpse rows into shared
    `MapRuntime` grid snapshots, so login/movement visibility reads the map
    cache instead of querying nearby corpses from the character DB on each
    movement rescan; death/reclaim updates the same map corpse snapshot when a
    corpse is created or converted to bones;
  - player health/power/spells/inventory/quest status are copied into
    `PlayerRuntime` after session mutations, and movement/logout persistence
    prefers the map snapshot;
  - `MapRuntime::update_player_position` now preserves non-player
    `visible_objects`, fixing a real patrol visibility hole where map-owned
    creature movement could lose the knowledge needed to send movement instead
    of a fresh create;
  - production `WorldSessionState` no longer owns creature runtime/combat maps;
    the remaining same-named fields are test-only shims for legacy unit tests.

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

- Completed the six user-requested map/shared ownership performance fixes:
  quest/template relation reads through shared `ObjectMgr`, player corpse
  visibility through map-owned corpse grids, DB creature corpse loot generated
  once from map-owned corpse state, player combat stats cached in
  `PlayerRuntime`, player auto-attack timing checked through map-owned runtime,
  and DB-creature visibility diffs staged from map-owned player visibility.
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
- `cargo fmt --check` passed after the Defias workload logging demotion.
- `cargo test -p wow-network --lib` passed (`266` tests) after the Defias
  workload logging demotion.
- `cargo test -p wow-network db_creature_combat --lib` passed after the
  retaliation-start fix.
- `cargo test -p wow-network player_hit_announces --lib` passed.
- `cargo test -p wow-network --lib` passed (`267` tests) after the
  retaliation-start fix.
- `cargo test -p wow-network db_creature_aggro --lib` passed after moving
  aggro distance filtering before native navigation.
- `cargo test -p wow-network --lib` passed (`267` tests) after the aggro
  stall fix.
- `cargo test -p wow-network map_runtime_sight_aggro_uses_cell_buckets_and_detection_range --lib`
  passed.
- `cargo test -p wow-network db_creature_aggro --lib` passed (`9` tests).
- `cargo test -p wow-network --lib` passed (`268` tests) after moving
  production sight aggro to shared MapRuntime cell-bucket candidate selection.
- `.\scripts\test-rust.cmd` with
  `CARGO_TARGET_DIR=target\codex-mapruntime-aggro-test` passed.
- `cargo test -p wow-db gameobject --lib` passed (`0` filtered tests) after
  adding DB gameobject rectangle loading.
- `cargo fmt --check` passed after map-owned gameobject changes.
- `cargo test -p wow-network gameobject --lib` passed (`7` tests).
- `cargo test -p wow-network --lib` passed (`271` tests).
- `.\scripts\test-rust.cmd` with
  `CARGO_TARGET_DIR=target\codex-mapruntime-gameobjects-test` passed.
- `cargo fmt --check`, `cargo test -p wow-network gameobject --lib`,
  `cargo test -p wow-network --lib`, and `.\scripts\test-rust.cmd` with
  `CARGO_TARGET_DIR=target\codex-gameobject-position-test` passed after
  switching DB gameobject creates to positioned create blocks.
- `cargo fmt --check`, `cargo test -p wow-network gameobject --lib`,
  `cargo test -p wow-network loot --lib`, `cargo test -p wow-network --lib`,
  and `.\scripts\test-rust.cmd` with
  `CARGO_TARGET_DIR=target\codex-gameobject-loot-test` passed after wiring
  Milly-style quest chest dynamic flags and gameobject loot.
- `.\scripts\test-rust.cmd` with
  `CARGO_TARGET_DIR=target\codex-merge-gameobjects-test` passed.
- `cargo fmt --check` passed after fixing gameobject login decode.
- `cargo test -p wow-network gameobject --lib` passed (`4` tests) after
  fixing gameobject login decode.
- `cargo fmt --check`, `cargo test -p wow-network gameobject --lib`,
  `cargo test -p wow-network spell --lib`, `cargo test -p wow-network loot --lib`,
  `cargo test -p wow-network --lib`, and `.\scripts\test-rust.cmd` with
  `CARGO_TARGET_DIR=target\codex-gameobject-opening-test` passed after wiring
  the client `Opening` spell for gameobject loot.
- `cargo fmt --check`, `cargo test -p wow-network opening_spell_packets_include_gameobject_target_mask --lib`,
  `cargo test -p wow-network gameobject --lib`, `cargo test -p wow-network spell --lib`,
  `cargo test -p wow-network loot --lib`, and `cargo test -p wow-network --lib`
  passed after forcing `TARGET_FLAG_GAMEOBJECT` into the outgoing Opening spell
  target mask.
- `cargo fmt --check`, `cargo test -p wow-network opening_spell --lib`,
  `cargo test -p wow-network gameobject --lib`, `cargo test -p wow-network spell --lib`,
  and `cargo test -p wow-network --lib` passed after changing Opening to the
  CMaNGOS packet order and removing the hidden failure cleanup guess.
- `cargo test -p wow-network map_runtime_idle_motion_start_guids_include_timer_blocked_grids_without_players --lib`
  passed after moving idle random/waypoint motion candidate selection to
  shared `MapRuntime`.
- `cargo test -p wow-network shared_db_creature_idle_motion_updates_map_and_observers --lib`
  passed after the shared idle-motion ownership shift.
- `cargo test -p wow-network map_runtime_grid_states_prepare_idle_and_unload_blockers --lib`
  passed after the shared idle-motion ownership shift.
- `cargo test -p wow-network db_creature_ --lib` passed (`78` tests) after
  the shared idle-motion ownership shift.
- `.\scripts\test-rust.cmd` with
  `CARGO_TARGET_DIR=target\codex-mapruntime-idle-motion-test` passed after the
  shared idle-motion ownership shift.
- `cargo test -p wow-network map_runtime_idle_motion_start_guids_ignore_far_same_grid_creatures --lib`
  failed before the patrol-start radius fix and passed after it.
- `cargo test -p wow-network idle_motion --lib` passed (`6` tests) after the
  patrol-start radius fix.
- `cargo fmt --check` passed after the patrol-start radius fix.
- `cargo test -p wow-network db_creature_ --lib` passed (`80` tests) after the
  patrol-start radius fix.
- `.\scripts\test-rust.cmd` with
  `CARGO_TARGET_DIR=target\codex-patrol-investigation-test` passed after the
  patrol-start radius fix.
- `cargo fmt --check` and `cargo test -p wow-network idle_motion --lib` passed
  (`7` tests) after moving idle/waypoint patrol advancement to a map-owned tick
  surface.
- `cargo test -p wow-network db_creature_ --lib` passed (`80` tests) after the
  map-owned patrol tick change.
- `.\scripts\test-rust.cmd` with
  `CARGO_TARGET_DIR=target\codex-map-owned-motion-test` passed after the
  map-owned patrol tick change.
- `cargo fmt --check`, `cargo test -p wow-network idle_motion --lib`,
  `cargo test -p wow-network db_creature_ --lib`, and `.\scripts\test-rust.cmd`
  with `CARGO_TARGET_DIR=target\codex-map-loop-motion-test` passed after moving
  idle/waypoint patrol scheduling from session-triggered calls into the
  background map runtime update loop.
- `cargo fmt --check`, `cargo test -p wow-network idle_motion --lib`,
  `cargo test -p wow-network world_tick --lib`, and `.\scripts\test-rust.cmd`
  with `CARGO_TARGET_DIR=target\codex-100ms-map-tick-test` passed after changing
  `WORLD_TICK_MILLIS` from `250` to the CMaNGOS-default `100`.
- `cargo check -p wow-network --lib` passed after the session-to-map ownership
  migration.
- `cargo fmt --check` passed after the session-to-map ownership migration.
- Focused ownership tests passed:
  `map_runtime_player_gameplay_sync_owns_session_mutable_state`,
  `map_runtime_db_gameobject_loot_item_is_shared_between_characters`,
  `map_runtime_db_gameobject_loot_item_can_restore_after_failed_autostore`,
  `begin_shared_db_creature_combat_uses_mapruntime_liveness_without_session_cache`,
  `player_melee_validation_refreshes_stale_session_cache_from_mapruntime`,
  `active_db_creature_combat_snapshot_uses_mapruntime_without_session_cache`,
  `starter_melee_spell_failure_uses_melee_validity_before_damage`, and
  `map_runtime_player_movement_preserves_db_creature_visibility_set`.
- `cargo test -p wow-network db_creature_ --lib` passed (`82` tests).
- `cargo test -p wow-network gameobject --lib` passed (`11` tests).
- `cargo test -p wow-network spell --lib` passed (`14` tests).
- `cargo test -p wow-network combat --lib` passed (`26` tests).
- `cargo test -p wow-network --lib` passed (`285` tests).
- `cargo clippy -p wow-network --all-targets -- -D warnings` passed.
- `.\scripts\test-rust.cmd` with
  `CARGO_TARGET_DIR=target\codex-session-ownership-test` passed.
- `.\scripts\run-client-stack-18085.cmd -NoAutoRestart` rebuilt and restarted
  auth/world after stopping the stale locked binaries.
- `cargo check -p wow-network --lib`, `cargo test -p wow-network player_corpse
  --lib`, and `cargo clippy -p wow-network --all-targets -- -D warnings`
  passed after moving player corpse visibility to shared `MapRuntime`
  snapshots.
- Six-path ownership verification:
  - `cargo fmt --check`
  - `cargo test -p wow-network object_mgr --lib`
  - `cargo test -p wow-network map_runtime_player_corpse --lib`
  - `cargo test -p wow-network map_runtime_db_creature_loot_item_is_generated_once --lib`
  - `cargo test -p wow-network map_runtime_stages_db_creature_visibility_from_player_visible_set --lib`
  - `cargo test -p wow-network combat --lib` (`26` tests)
  - `cargo test -p wow-network --lib` (`291` tests)
  - `cargo clippy -p wow-network --all-targets -- -D warnings`
  - `.\scripts\test-rust.cmd` with
    `CARGO_TARGET_DIR=target\codex-six-map-owned-test` passed.
- `.\scripts\run-client-stack-18085.cmd -NoAutoRestart` rebuilt and restarted
  auth/world after the six-path migration; listeners verified on
  `127.0.0.1:13724` and `127.0.0.1:18085`.
- Final session-eradication verification:
  - `cargo check -p wow-network --lib`
  - `cargo fmt --check`
  - `cargo clippy -p wow-network --all-targets -- -D warnings`
  - `cargo test -p wow-network --lib` (`291` tests)
  - `.\scripts\test-rust.cmd` with
    `CARGO_TARGET_DIR=target\codex-total-mapruntime-test` passed.
- `.\scripts\run-client-stack-18085.cmd -NoAutoRestart` rebuilt/restarted
  auth/world after stopping the stale supervising wrapper that was locking
  `target\debug\authserver.exe`; listeners verified on `127.0.0.1:13724` and
  `127.0.0.1:18085`.
- Login-load-screen fix verification:
  - `cargo fmt --check`
  - `cargo test -p wow-network self_spawn_update_chunks_without_legacy_fixture_blocks --lib`
  - `cargo test -p wow-network gameobject --lib` (`11` tests)
  - `cargo clippy -p wow-network --all-targets -- -D warnings`
  - `cargo test -p wow-network --lib` (`292` tests)
  - `.\scripts\run-client-stack-18085.cmd -NoAutoRestart` rebuilt/restarted
    auth/world; listeners verified on `127.0.0.1:13724` and
    `127.0.0.1:18085`.

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
- The production session-owned creature/combat mirror is gone. Remaining
  `session.db_creatures` / active-combat references are `#[cfg(test)]` legacy
  unit-test helpers and should be converted to map-runtime test harnesses as
  cleanup, not treated as runtime ownership.
- G10/G11 remain broader red/yellow areas: NPC interaction fidelity and
  persistence/relog sanity across every major starter-zone action.
- GitHub issue #58 tracks full CMaNGOS creature loot-table rolling beyond the
  current active quest item-drop bridge.
- GitHub issue #59 tracks moving DB gameobject consumed/respawn state from
  per-session storage into shared `MapRuntime`/world state for multiplayer
  consistency; this is fixed locally for consumed state and gameobject loot
  ownership, pending commit/push.
- GitHub issue #60 tracks a real-client combat/visibility desync where the
  client attacked visible Defias Thug GUID `0xF130000026013939` but the session
  treated it as unknown or not alive.
- GitHub issue #61 tracked the DB-heavy questgiver status-query path amplified
  by gameobject visibility; the repeated production quest/template/relation
  reads now go through shared `ObjectMgr` locally, pending commit/push.
- Real-client Defias smoke found player attacks could damage a hostile DB
  creature without the creature visibly engaging. The immediate fix now sends
  the same creature-side combat-start/chase path used by proximity aggro when
  retaliation combat begins.
- Defias camp smoke also showed a broader stall/freezing pattern: the
  worldserver log contained a long synchronous VMAP load burst before gameplay
  packets resumed, followed by ability cancel spam and unknown-target attacks
  for Defias GUID `0xF13000002601396E` (entry 38 / spawn 80238). The aggro
  path was doing native navigation before cheap distance filtering, which could
  block the single session loop for many visible but out-of-range hostiles.
  The immediate mitigation is merged locally; remaining follow-up is to add
  tick-lag/native-call instrumentation.
- Patrol-start starvation from far same-grid movers is fixed in unit coverage
  and the stack has been rebuilt/restarted; it still needs a real-client
  Northshire/Defias smoke to verify visible patrol packets over time.

## Recommended Next Task

Real-client smoke the rebuilt stack in the Northshire start area plus the run
to Defias:

1. confirm patrols keep moving after leaving the initial zone and returning;
2. confirm player-vs-creature melee still starts creature retaliation/chase;
3. confirm Milly-style gameobject loot opens, autostores once, and releases
   cleanly;
4. confirm repeated questgiver/status-query traffic stays responsive with the
   shared `ObjectMgr` cache;
5. if smoke passes, commit this ownership migration before taking the next C2
   feature branch.

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
