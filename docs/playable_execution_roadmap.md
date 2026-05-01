# Playable Server Execution Roadmap

This is the working execution plan for turning the current Northshire slice
into a server that is genuinely playable and increasingly close to CMaNGOS
Classic behavior. Use it with:

- `docs/session_handoff.md` for current state;
- `docs/playable_gate_board.md` for gate health;
- `docs/rust_migration_plan.md` for durable milestone history;
- focused briefs such as `docs/g12_shared_mapruntime_plan.md` and
  `docs/briefs/combat.md` for subsystem detail.

Keep this document practical. It should answer:

- what order work should happen in;
- what can be split across workers;
- what each worker branch owns;
- what proof is required before merging.

## Current Strategic Read

The project has crossed the most important early threshold: the server is no
longer just an auth or packet bootstrap demo. It has real-client proof for the
first playable world, a Northshire starter-zone slice, a shared in-process
`MapRuntime`, DB-backed creatures, combat/corpse/loot state, movement
visibility, local `/say`, death/release/reclaim, and a growing CMaNGOS-shaped
combat/pathing foundation.

The main risk is now integration drag. Many remaining playable features touch
the same worldserver surfaces: `MapRuntime`, DB creature lifecycle, combat
events, player state updates, persistence, and the starter-zone harness. The
roadmap should therefore prefer narrow vertical slices that land behind shared
events and packet builders, with clear branch ownership.

## Roadmap Principles

- Real-client proof decides whether a gate is playable; harness proof protects
  it from regression.
- CMaNGOS source, DB rows, or DBC-derived values are the only accepted sources
  for gameplay parity. Do not invent constants to make a demo pass.
- Shared-world state belongs in `MapRuntime` unless it is truly session-local.
  Avoid new per-session creature, loot, threat, or movement authority.
- Every branch should own a small set of files or one subsystem boundary.
  Parallel branches are useful only when they minimize merge conflicts.
- Prefer adding a test seam before broadening behavior. The test seam is often
  the real unlock for later workers.
- Performance notes are welcome, but parity comes first unless the current
  implementation would obviously collapse with multiple players.

## Immediate Integration Lane

The next merge should stabilize the current uncommitted G8/G9 work before new
parallel branches diverge.

1. Real-client smoke current movement/threat changes.
2. Commit the slice if the smoke is acceptable.
3. Update `docs/session_handoff.md` with the exact result and tests.
4. Branch the next workers from that commit.

Proof to collect before branching:

- wolves chase on smooth MMAP-backed paths without hover or vertical snapping;
- stop-on-reach works for normal and larger-reach creatures;
- waypoint creatures no longer move as one tiny no-wait spline per node;
- two clients observe death-time motion stop and no stale local patrol/chase;
- threat switching transfers creature attacks from player A to player B without
  duplicate attack-start/stop spam;
- `.\scripts\test-rust.cmd` passes, or any failure is explained as a local
  locked-target artifact.

## Checkpoint 2 Finish Line

Checkpoint 2 is done when one fresh Human Warrior can play Northshire as a
coherent early-game loop in the real client:

- enter Northshire with DB-backed NPCs, creatures, vendors, trainers, and any
  selected quest objects visible;
- accept, progress, complete, and persist at least one kill-count quest and one
  item/progress quest;
- fight DB creatures with believable aggro, chase, swing, damage, death,
  corpse, loot, respawn, and shared multiplayer state;
- loot money/items, use inventory, buy/sell with a vendor, gain XP, level up,
  learn one trainer ability, die, release, resurrect, logout, and relog without
  corrupting state;
- keep two clients sharing player visibility, movement, chat, and DB creature
  state through the same `MapRuntime`;
- pass the required scripts in `docs/rust_migration_plan.md`.

## Phase Plan

### Phase A: Stabilize Shared World Combat

Goal: make multiplayer creature combat feel authoritative instead of
session-local.

Primary gates: G8, G9, G12.

Deliverables:

- finish the current movement/threat slice;
- add or extend two-client torture coverage around combat, corpse, loot,
  logout, relog, and respawn;
- close the remaining stale-observer classes of bugs before adding broader
  combat features;
- keep lazy grid loading from regressing into movement-time DB radius queries.

Merge proof:

- focused `cargo test -p wow-network map_runtime_ --lib`;
- focused `cargo test -p wow-network db_creature_ --lib`;
- `.\scripts\test-rust.cmd`;
- real-client two-client smoke for shared death/loot and threat switch when the
  branch touches player-visible multiplayer state.

### Phase B: Make Combat Feel Like Classic

Goal: starter creatures and the player should fight with CMaNGOS-shaped rules,
not packet-demo rules.

Primary gate: G8.

Deliverables:

- player offensive miss/dodge/parry/block/crit/glancing/crushing eligibility
  where Classic allows it;
- swing timer reset, offhand, queued next-swing, and retry behavior;
- player and creature damage formulas tied to DB, DBC, and source-derived
  stats;
- moving melee leeway and fuller reach/model data;
- threat expansion for healing, taunt/fixate-style behavior, group ownership,
  pets, and edge-case victim selection.

Merge proof:

- `cargo test -p wow-network melee --lib`;
- `cargo test -p wow-network creature_melee --lib`;
- `cargo test -p wow-network player_main_hand --lib`;
- `cargo test -p wow-network db_creature_threat --lib`;
- `.\scripts\test-rust.cmd`;
- real-client combat smoke when packet shapes or timing are changed.

### Phase C: World Creature Fidelity And Navigation

Goal: creatures should move and live in the world like DB-backed CMaNGOS
creatures, not as local approximations.

Primary gates: G9, G8.

Deliverables:

- CMaNGOS-shaped random movement path selection using navmesh height/query
  behavior;
- waypoint pre-send behavior, movement informs, and script hooks where needed;
- return-home force-destination, shortcut, and high-velocity behavior;
- grid unload and idle eviction rules that preserve combat, corpse, loot, and
  respawn correctness;
- query-count instrumentation proving grid loading is the hot path, not DB
  scans per movement heartbeat.

Merge proof:

- `cargo test -p wow-network db_creature_random_motion --lib`;
- `cargo test -p wow-network db_creature_waypoint --lib`;
- `cargo test -p wow-network db_creature_mmap_path --lib`;
- `cargo test -p wow-network map_runtime_ --lib`;
- `.\scripts\test-rust.cmd`;
- real-client motion smoke in Northshire open space and around obstacles.

### Phase D: Quest, XP, Level, Trainer, Vendor, Inventory Loop

Goal: the player can make durable character progress through normal starter
zone actions.

Primary gates: G4, G6, G10, G11.

Deliverables:

- quest status/query/accept/progress/complete/reward parity for selected
  Northshire quests;
- creature XP and quest XP through shared kill/reward finalizers;
- level-up packets, stat refresh, health/power refresh, and persistence;
- trainer list, one valid learn path, unavailable spell states, and persisted
  learned spell;
- vendor buy/sell durability, money checks, inventory full checks, and relog
  proof;
- relog checkpoints after each major state mutation.

Merge proof:

- `.\scripts\test-starter-zone-flow.cmd`;
- `.\scripts\test-world-flow.cmd`;
- `.\scripts\test-rust.cmd`;
- focused unit tests for packet builders and DB writes;
- real-client smoke for every newly player-visible interaction.

### Phase E: Death, Relog, And Long-Run Sanity

Goal: the playable loop survives failure, disconnect, and persistence edges.

Primary gates: G7, G11, G12.

Deliverables:

- logout/relog during combat, corpse, loot, ghost, and resurrection states;
- player corpse/bones expiry behavior;
- durability loss and resurrection sickness if needed for Classic fidelity;
- reconnect behavior for visible players and shared creatures;
- longer two-client smoke script or documented manual checklist.

Merge proof:

- `.\scripts\test-starter-zone-flow.cmd`;
- `.\scripts\test-rust.cmd`;
- dedicated two-client logout/relog torture coverage;
- real-client smoke for death/release/reclaim/healer and relog edge cases.

## Parallel Workstreams

Use these streams once the immediate integration lane is committed. Each stream
should use a separate branch and avoid touching another stream's owned files
unless the parent integrator explicitly coordinates it.

| Stream | Suggested Branch | Primary Owner Scope | Good Worker Task | Merge Dependency |
| --- | --- | --- | --- | --- |
| Shared runtime hardening | `codex/c2-mapruntime-hardening` | `crates/wow-network/src/world/maps/`, shared creature lifecycle tests, multi-client harness | Grid unload/idle eviction, logout/relog torture, query-count assertions | Start after current G8/G9 movement-threat commit |
| Combat math and timing | `codex/c2-combat-parity` | `crates/wow-network/src/world/combat/`, combat packet builders, combat tests | Offensive rolls, swing timer reset/queue, damage formula parity | Rebase after shared runtime changes that alter damage events |
| Creature movement/navigation | `codex/c2-creature-navigation` | `crates/wow-network/native/`, `mmap_path.rs`, `combat/motion.rs`, DB creature motion tests | Random point pathing, waypoint pre-send, return-home fidelity | Needs current MMAP/chase slice committed |
| Quest/progression | `codex/c2-progression-loop` | `world/quests.rs`, XP/level helpers, progression DB writes, starter harness steps | Kill credit to XP, quest reward XP/money/items, level-up packet/state | Should consume shared death finalizer and avoid combat internals |
| NPC services | `codex/c2-npc-services` | `world/gossip.rs`, `world/vendors.rs`, trainer module if split, NPC interaction tests | Trainer list/learn path, vendor buy/sell polish, gossip affordance audit | Can run beside combat if DB mutation helpers are coordinated |
| Persistence/relog | `codex/c2-relog-sanity` | character DB mutation helpers, starter harness relog checkpoints, session logout/login state | Relog matrix for quest, XP, inventory, trainer, death, corpse, creature respawn | Best after progression and death state APIs settle |
| Harness and tooling | `codex/c2-harness-multiclient` | `bins/*flow-test`, `scripts/test-*.cmd`, packet test helpers | Dedicated multiclient world-flow test and scenario helpers | Can start early if it only adds helpers and tests |
| Codebase sustainability | `codex/c2-world-split-followup` | mechanical module splits, no behavior changes | Split large tests or DB character modules along existing boundaries | Run between feature branches, not during hot behavior merges |

## Branch And Merge Strategy

Recommended branch rules:

- branch from the latest green integration commit on `codex/rusty-mangos`;
- use the `codex/` prefix and name the gate or subsystem in the branch;
- keep each branch to one vertical slice and one proof story;
- avoid mixing mechanical splits with gameplay behavior;
- update docs only when the branch changes the plan, gate status, or next-agent
  operating context;
- merge branches through the parent integrator in dependency order, not by who
  finishes first.

Suggested merge order after the current uncommitted slice:

1. `codex/c2-harness-multiclient`, if it adds test infrastructure without
   gameplay behavior.
2. `codex/c2-mapruntime-hardening`, because many later branches depend on
   authoritative shared state.
3. `codex/c2-creature-navigation`, if it changes motion packets or path state.
4. `codex/c2-combat-parity`, rebased onto shared runtime and navigation.
5. `codex/c2-progression-loop`, because XP and quest reward hooks should attach
   to settled death/reward finalizers.
6. `codex/c2-npc-services`, if trainer/vendor changes are mostly independent.
7. `codex/c2-relog-sanity`, once the state it verifies exists.
8. mechanical split branches between feature merges when the tree is green.

Conflict hot spots:

- `crates/wow-network/src/world/tests.rs`;
- `bins/starter-zone-flow-test/src/main.rs`;
- `crates/wow-network/src/world/maps/map.rs`;
- `crates/wow-network/src/world/maps/map_manager.rs`;
- `crates/wow-network/src/world/combat/`;
- `crates/wow-network/src/world/quests.rs`;
- `crates/wow-db/src/character.rs`.

Avoid assigning two active workers to the same hot spot unless their write
sets are explicitly disjoint.

## Worker Contract

Every worker branch should start with:

- current gate/subsystem;
- CMaNGOS source or DB/DBC backing path;
- owned files/modules;
- files/modules explicitly out of scope;
- required tests;
- real-client proof needed, if any;
- stop condition.

Standard worker stop conditions:

- stay in the assigned gate and write scope;
- do not invent gameplay constants;
- do not broaden architecture without parent approval;
- do not revert unrelated worktree changes;
- run focused tests and report exact results;
- leave docs touched only when behavior, plan, or proof changed.

## Issue Logging Targets

Log rather than silently carrying these if discovered outside the active branch:

- P0/P1: crash, panic, protocol desync, DB corruption, duplicated loot/reward,
  or shared creature/player state divergence.
- P2: missing behavior needed for Checkpoint 2 closure but not blocking the
  current branch.
- P3: visible CMaNGOS fidelity mismatch that does not block the current proof.
- P4: performance, refactor, test infrastructure, or future architecture work.

Use the issue labels from `docs/playable_gate_board.md`. If GitHub is not
available, add a compact fallback note to `docs/session_handoff.md`.

## Final Real-Client Closure Pass

When the streams above converge, run one explicit closure pass:

1. reset to a clean local DB fixture;
2. run all required automated scripts;
3. run one fresh Human Warrior through the full Northshire route;
4. run two clients through player visibility, movement, chat, shared combat,
   loot, logout, relog, death/corpse, and respawn;
5. update `docs/session_handoff.md` with the grading table, test results,
   fixed P0/P1 issues, and logged P2/P3/P4 follow-ups.

Checkpoint 2 should not close until the final real-client table has no `FAIL`
rows and every `PARTIAL` or `DEFERRED` row has a linked issue and a clear reason
it does not block the milestone.
