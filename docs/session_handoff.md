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
- Current uncommitted investigation edit: high-volume authenticated packet and
  DB-creature visibility movement logs were demoted from `info` to `debug`
  after Defias smoke showed the worldserver spending measurable CPU in a
  packet/status/visibility churn state.
- Current uncommitted combat fix: DB creatures now immediately announce
  retaliation (`SMSG_ATTACKSTART`, combat flags, and chase if needed) when a
  player hit starts shared creature combat, including the 1 HP player case.
- Current uncommitted stall fix: hostile sight-aggro now checks cheap
  detection distance before VMAP/MMAP navigation, avoiding native LOS/path
  calls for every visible out-of-range Defias during camp entry. Spell-cancel
  fixture spam is also debug-only.
- Current uncommitted map-runtime aggro performance fix: production sight
  aggro now asks shared `MapRuntime` for cell-bucketed nearby hostile
  candidates, applies CMaNGOS-shaped detection range before native navigation,
  and keeps VMAP/MMAP checks outside the map mutex. This avoids returning to a
  session-owned full visible-creature scan.
- Current uncommitted creature-ownership fix: idle random/waypoint motion
  candidate selection now comes from shared `MapRuntime` creature state in
  `Active` or `UnloadBlocked` grids instead of `session.db_creatures`. Session
  caches now only remember moved/started creatures when they were already
  tracked locally or are still inside the viewer's visibility radius. This is
  the first deliberate CMaNGOS-close step away from session-owned creature
  runtime ticking and directly targets the Northshire-to-Defias patrol freeze.
- Current uncommitted patrol-start fix: map-owned idle/waypoint start candidates
  now use nearby cell buckets plus an exact player visibility-radius check
  instead of admitting every ready creature in a coarse player-interest grid.
  This prevents far same-grid zero-wait movers from starving visible patrols
  under the per-tick start budget.
- Current uncommitted map-owned patrol tick fix: worldserver startup now spawns
  a background map runtime update loop. Idle/waypoint patrol advancement/start
  runs from that map loop through `MapRuntime`, not from per-session ticks. The
  shared world/map tick interval is now `100ms`, matching CMaNGOS'
  `MapUpdateInterval` default.
  `MapRuntime` also tracks DB-creature visibility in player `visible_objects`
  so the map-owned tick can send create-before-move when needed and plain
  movement when the player already knows the creature. Session ticks only sync
  their local viewer cache from map-owned creature snapshots.
- Current uncommitted map-runtime gameobject ownership fix: DB gameobjects now
  load into shared `MapRuntime` grid/cell buckets, login and movement stream
  snapshots from the map, and `CMSG_GAMEOBJ_USE` prefers the shared snapshot
  before session cache. Consumable quest objects such as Milly's Harvest now
  update a shared consumed timer and broadcast `SMSG_DESTROY_OBJECT` to nearby
  observers. Follow-up visibility fix: DB gameobject create blocks now use the
  positioned `CREATE_OBJECT2` / `UPDATEFLAG_HAS_POSITION` shape so the client
  places Milly's Harvest objects in the field instead of receiving only update
  fields. Follow-up interaction fix: quest chest gameobjects now send
  CMaNGOS-style activation/sparkle dynamic flags and `gameobject_loot_template`
  quest loot can be opened/autostored through the normal loot window path.
  Follow-up Opening spell fix: gameobject clicks now accept the client `Opening`
  spell (`6478`), send `SMSG_SPELL_START` with a cast timer, then send
  `SMSG_SPELL_GO` and the gameobject loot response. Follow-up cast-bar fix:
  outgoing Opening spell targets now force `TARGET_FLAG_GAMEOBJECT` alongside
  `TARGET_FLAG_LOCKED`, matching the resolved CMaNGOS gameobject target mask.
  Follow-up CMaNGOS lifecycle fix: removed the hidden failure cleanup guess and
  changed Opening ordering to CMaNGOS' real sequence: `SMSG_SPELL_START` at
  cast start, then after the timer `SMSG_CAST_RESULT` OK, `SMSG_SPELL_GO`, and
  finally the loot response.
- Local branch is intentionally ahead while C2 workstream merges are being
  real-client smoked.
- Live client stack was restarted after the 100ms map tick change:
  authserver PID `15860` on `127.0.0.1:13724`, worldserver PID `28740` on
  `127.0.0.1:18085`, logs `auth-client-13724.log` and
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
- Session-local creature runtime still exists in important readers:
  `active_combat_target` / melee checks, retaliation/chase/evade helpers,
  movement visibility retention, and parts of combat packet production still
  consult `session.db_creatures` after the new shared idle/patrol start
  selection. The current correction removes session ownership from the idle
  random/waypoint candidate list and the once-per-tick patrol start/advance
  scheduler; that scheduler is now invoked by the background map runtime update
  loop.
- G10/G11 remain broader red/yellow areas: NPC interaction fidelity and
  persistence/relog sanity across every major starter-zone action.
- GitHub issue #58 tracks full CMaNGOS creature loot-table rolling beyond the
  current active quest item-drop bridge.
- GitHub issue #59 tracks moving DB gameobject consumed/respawn state from
  per-session storage into shared `MapRuntime`/world state for multiplayer
  consistency.
- GitHub issue #60 tracks a real-client combat/visibility desync where the
  client attacked visible Defias Thug GUID `0xF130000026013939` but the session
  treated it as unknown or not alive.
- GitHub issue #61 tracks the DB-heavy questgiver status-query path amplified
  by gameobject visibility; repeated `CMSG_QUESTGIVER_STATUS_QUERY` traffic may
  be contributing to sluggish gameplay feel. Live Defias smoke showed
  worldserver CPU around 31% of one core over a 10s sample while recent logs
  contained heavy `0x0182` status-query bursts plus creature visibility
  create/destroy churn in the 40-50 tracked-creature range.
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
  tick-lag/native-call instrumentation and make visible-target attack recovery
  refresh from shared `MapRuntime` when the client attacks a visible DB GUID
  missing from `session.db_creatures`.
- Patrol-start starvation from far same-grid movers is fixed in unit coverage,
  but still needs a restarted real-client Northshire/Defias smoke to verify the
  visible patrol packet stream and long-run stability.

## Recommended Next Task

Continue the CMaNGOS-close creature ownership correction before the next
feature merge:

1. move the remaining chase/evade/combat readers off `session.db_creatures`
   where practical, starting with melee range/facing checks and visible-target
   recovery;
2. keep session creature state as a viewer cache only, not the source of truth;
3. then restart and real-client smoke the Northshire start area plus the run to
   Defias to confirm patrols keep moving after leaving the initial zone.

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
