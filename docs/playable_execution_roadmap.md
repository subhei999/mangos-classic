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

The next planning merge should retarget the parallel branch list around the
user-observed gaps that still keep Northshire from feeling playable.

1. Keep `codex/rusty-mangos` as the clean integration branch.
2. Branch workers from the latest green commit.
3. Prefer branches whose owned files do not overlap.
4. Merge by dependency order, not by who finishes first.

Proof to collect before broad feature work:

- a real-client Northshire checklist exists for the user-observed missing
  criteria below;
- the harness can prove at least the packet/DB side for each new system before
  real-client smoke;
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

## User-Observed Missing Criteria

These are the current practical blockers for a believable playable Northshire:

1. Quest availability is too broad: the client sees available quests without
   CMaNGOS level, class, race, chain, prerequisite, and repeatability filters.
2. Quest item drops are missing: kill quests work, but real loot tables and
   quest-drop enablement are not wired.
3. Gameobject quest pickup is missing.
4. Warrior gameplay is not real enough through level 6: no global cooldown,
   Heroic Strike is still toy-shaped, and broader warrior spell behavior is not
   functional.
5. Combat log output is missing.
6. Health regeneration and rage degeneration are missing.
7. Weapon skills and broader skill state are missing or stuck at level 1.
8. Aggro/chase/leash behavior is not CMaNGOS-like enough: hit/assist/combat
   activity should affect leash persistence, not simply reset at the initial
   radius.
9. NPC patrols start at server launch but stop working after a while.

Treat these as the next branch subjects. The split below is designed to reduce
cross-talk while still landing meaningful vertical slices.

## Phase Plan

### Phase A: Pin The Northshire Grade

Goal: turn the user-observed missing criteria into a repeatable harness and
real-client checklist.

Primary gates: all Checkpoint 2 gates.

Deliverables:

- a `Northshire Playability Grade` checklist in docs or harness output;
- focused harness scenarios for quest visibility, quest drops, gameobjects,
  warrior spells, combat log packet emission, regen/rage decay, skill state,
  aggro/leash, and patrol continuity;
- no gameplay implementation beyond test helpers unless the fix is tiny and
  local.

Merge proof:

- focused harness/unit tests for newly added checks;
- `.\scripts\test-rust.cmd`;
- written manual real-client checklist for anything not automatable yet.

### Phase B: Quest And Objective Fidelity

Goal: quests should appear, progress, and complete only when CMaNGOS says they
should.

Primary gates: G4, G5, G10, G11.

Deliverables:

- quest giver availability filters by level, class, race, prerequisite,
  exclusive group, chain state, repeatability, and quest status;
- quest item drops through real CMaNGOS loot tables and quest-drop eligibility;
- DB-backed gameobject visibility, query, activation, quest item pickup, and
  respawn/availability where the selected Northshire route needs it;
- relog proof for quest progress, quest item inventory, and completed state.

Merge proof:

- `.\scripts\test-starter-zone-flow.cmd`;
- focused quest/loot/gameobject tests;
- real-client smoke for yellow/gray markers, drop behavior, and pickup objects.

### Phase C: Warrior Level 1-6 Playability

Goal: starter creatures and the player should fight with CMaNGOS-shaped rules,
with enough warrior systems to play to level 6.

Primary gates: G6, G8, G11.

Deliverables:

- global cooldown and per-spell cooldown state;
- real warrior spell behavior needed through level 6, including a proper
  Heroic Strike next-swing shape instead of fixture damage;
- combat log packets for melee, spell, miss, damage, and resource-relevant
  events;
- health regeneration and rage degeneration on CMaNGOS-like ticks;
- skill and weapon-skill state loaded, shown, persisted, and advanced by real
  actions where needed.

Merge proof:

- focused warrior spell, GCD, combat log, regen/rage, and skill tests;
- `.\scripts\test-starter-zone-flow.cmd`;
- `.\scripts\test-rust.cmd`;
- real-client smoke from level 1 through level 6 warrior actions.

### Phase D: Creature Agency And Patrol Stability

Goal: creatures should move and live in the world like DB-backed CMaNGOS
creatures, not as local approximations.

Primary gates: G9, G8.

Deliverables:

- CMaNGOS-shaped random movement path selection using navmesh height/query
  behavior;
- waypoint pre-send behavior, movement informs, and script hooks where needed;
- return-home force-destination, shortcut, and high-velocity behavior;
- CMaNGOS-like aggro, assistance, hit-reactivation, leash, evade, and chase
  persistence rules;
- patrol continuity over time, including after combat, death, respawn, grid
  loading, and observer churn;
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

### Phase E: Integration, Trainer, Vendor, And Relog

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

## Parallel Workstreams

Use these streams from the clean integration branch. Each stream
should use a separate branch and avoid touching another stream's owned files
unless the parent integrator explicitly coordinates it.

| Stream | Suggested Branch | Primary Owner Scope | Good Worker Task | Merge Dependency |
| --- | --- | --- | --- | --- |
| Northshire grading harness | `codex/c2-northshire-grade` | `bins/starter-zone-flow-test/`, `scripts/test-starter-zone-flow.*`, docs checklist | Add explicit pass/fail checks for the nine user-observed missing criteria | Can start immediately; should avoid gameplay code |
| Quest availability | `codex/c2-quest-eligibility` | `crates/wow-network/src/world/quests.rs`, quest DB reads, quest packet tests | Filter quest markers/status/list by level, class, race, prerequisites, chains, repeatability, and current status | Can start immediately; avoid loot and gameobject logic |
| Quest loot drops | `codex/c2-quest-loot-drops` | `crates/wow-network/src/world/loot.rs`, world loot DB helpers, inventory insertion tests | Use real loot tables and enable quest item drops only for eligible active quests | Needs stable quest-status read API; otherwise can run beside eligibility with a narrow interface |
| Gameobject quest objectives | `codex/c2-gameobject-quests` | new/focused `world/gameobjects` module, gameobject DB helpers, object update/query tests | Spawn/query/use quest gameobjects and grant pickup objective/items with respawn rules | Can run beside quest loot; depends on quest-status API before final merge |
| Warrior spells and GCD | `codex/c2-warrior-spells-gcd` | `world/spells.rs`, spell cooldown/GCD state, warrior spell tests | Implement global cooldown, spell validation, and real warrior actions through level 6 | Rebase after combat-log packet helpers if they touch shared builders |
| Combat log packets | `codex/c2-combat-log` | combat packet builders/broadcast helpers, combat tests | Emit real combat log feedback for melee/spell damage, misses, failures, resource events | Can start immediately if it owns packet builders only |
| Health and rage ticks | `codex/c2-regen-rage-ticks` | player runtime/session tick state, health/rage update builders, persistence tests | Add health regen and rage degeneration with CMaNGOS-like timing and packet updates | Coordinate with warrior spell branch on rage spend/gain fields |
| Skills and weapon skills | `codex/c2-skills-weapon-skill` | character skill DB helpers, skill update packets, narrow combat skillup hook | Load/show/persist skills and weapon skills; advance weapon skill from real actions | Can run beside spells if combat hook is one small interface |
| Aggro, chase, and leash parity | `codex/c2-aggro-leash-parity` | `world/combat/{aggro,motion,evade}`, MapRuntime combat events, chase tests | Compare CMaNGOS aggro/leash rules and fix reset/chase persistence while in combat or recently hit | Avoid patrol files except shared motion interface |
| Patrol runtime stability | `codex/c2-patrol-stability` | DB creature waypoint/random motion, MapRuntime motion/lifecycle tests | Find why patrols stop after time; keep patrols alive across ticks, combat, death, respawn, and grid activity | Should merge after or carefully rebase around aggro/leash if both touch motion |
| NPC services and relog polish | `codex/c2-npc-relog-polish` | `world/gossip.rs`, `world/vendors.rs`, trainer module, relog harness | Trainer/vendor/gossip polish plus relog checks after progression systems land | Merge after quest/spell/progression state exists |
| Codebase sustainability | `codex/c2-world-split-followup` | mechanical module splits, no behavior changes | Split large tests or DB character modules along established boundaries | Run between feature branches, not during hot behavior merges |

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

Suggested merge order:

1. `codex/c2-northshire-grade`, because it gives every branch a target.
2. `codex/c2-combat-log`, if it is limited to packet builders/helpers.
3. `codex/c2-quest-eligibility`, because quest status is the dependency for
   quest drops and gameobjects.
4. `codex/c2-quest-loot-drops` and `codex/c2-gameobject-quests`, rebased onto
   quest eligibility when needed.
5. `codex/c2-regen-rage-ticks`, then `codex/c2-warrior-spells-gcd`, because
   warrior spell behavior needs resource timing to be real.
6. `codex/c2-skills-weapon-skill`, before deeper combat math depends on skill
   values.
7. `codex/c2-aggro-leash-parity`.
8. `codex/c2-patrol-stability`, after aggro/leash if both touch creature
   motion.
9. `codex/c2-npc-relog-polish`, once quest/spell/progression state exists.
10. mechanical split branches between feature merges when the tree is green.

Conflict hot spots:

- `crates/wow-network/src/world/tests.rs`;
- `bins/starter-zone-flow-test/src/main.rs`;
- `crates/wow-network/src/world/maps/map.rs`;
- `crates/wow-network/src/world/maps/map_manager.rs`;
- `crates/wow-network/src/world/combat/`;
- `crates/wow-network/src/world/quests.rs`;
- `crates/wow-network/src/world/loot.rs`;
- `crates/wow-network/src/world/spells.rs`;
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
