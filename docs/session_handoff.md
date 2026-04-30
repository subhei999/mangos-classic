# Session Handoff

This file is the current operating brief for the next Rust migration session.
Durable roadmap belongs in `docs/rust_migration_plan.md`; the playable gate
dashboard belongs in `docs/playable_gate_board.md`; auth-specific setup belongs
in `docs/rust_auth_foundation.md`.

## Handoff Rules

- Keep only current branch state, active goal, recent meaningful changes, exact
  tests run, local blockers, and the next recommended task.
- Do not append a full chronological log. Prune stale detail as it becomes
  durable roadmap history.
- For non-blocking P2/P3/P4 discoveries, use GitHub issues as the primary
  tracker. Only record a handoff fallback if GitHub logging fails.

## Current Branch

- Branch: `codex/rusty-mangos`
- Latest checkpoint commit: `8119c7448` (`Implement player death corpse flow`),
  branched before the later world architecture split.
- Remote: not pushed yet.
- Worktree at handoff: contains an uncommitted, behavior-preserving
  CMaNGOS-shaped runtime type/file split in `crates/wow-network/src/world`.

## Current Goal

Checkpoint 2: **Starter Zone Playability**.

Northshire Valley / fresh Human Warrior remains the golden path. The current
trainer v1 slice proves one real ClassicDB training loop after XP/level-up:
accept Kobold Camp Cleanup, kill Kobold Vermin for creature XP and quest
progress, complete the quest, turn it in to Marshal McBride, grant reward money
and quest XP, level up, open the Warrior trainer Llane Beshere, learn one
available spell, charge money, and persist character progression, quest state,
money, and `character_spell`.

Use `docs/playable_gate_board.md` as the executive dashboard before selecting
work. G3 Movement Visibility Streaming has been user-verified in the real
client and is now a regression gate. G7 Player Death + Respawn is now
core-flow harness-proven and user-smoked in the real client. User-directed next
priority is to derisk multiplayer before continuing deeper G8/G9 work: keep one
monolithic worldserver, introduce a shared in-process `MapRuntime` / grid layer
inside `WorldRuntimeState`, and route player visibility, movement, `/say`, and
DB creature state through it. The user has a detailed implementation plan and
will walk the next agent through it. Follow `docs/g12_shared_mapruntime_plan.md`
as the implementation document for this G12 slice.

Important scope rule:
Stay focused on the requested multiplayer derisking step, but keep the agent
free to make small local safety, protocol, test, or data-integrity fixes needed
to prove the step. Log larger follow-ups rather than turning the task into a
broad parity sweep.

## What Changed Recently

- Completed G12 Shared MapRuntime Phase 1. World sessions now get a `SessionId`,
  register a `SessionHandle` in `WorldRuntimeState.sessions`, split the socket
  into a packet reader plus one outbound writer task, and route existing
  gameplay responses through `WorldPacketSink` / `OutboundWorldPacket` without
  adding multiplayer behavior yet.
- Completed G12 Shared MapRuntime Phase 2. `WorldRuntimeState` now owns a shared
  `MapRuntimeManager`; player login registers a `PlayerRuntime` into the shared
  map after self bootstrap, nearby already-online players receive the new
  player's create block, the new player receives nearby existing-player create
  blocks, and logout/disconnect removes the player and broadcasts
  `SMSG_DESTROY_OBJECT` to nearby players. The `world-flow-test` harness now
  proves two simultaneous clients can observe login/logout visibility.
- Completed G12 Shared MapRuntime Phase 3. Movement handlers now update the
  shared `MapRuntime`, compute nearby-player visibility diffs, send
  `SMSG_UPDATE_OBJECT` create packets when players re-enter range, send
  `SMSG_DESTROY_OBJECT` when players leave range or logout, and broadcast
  player movement packets to nearby sessions through `SessionRegistry`. The
  `world-flow-test` harness now proves two clients can see each other spawn,
  receive movement, lose visibility out of range, regain visibility on return,
  and observe logout destroy.
- User real-client smoke confirmed G12 Phase 3 movement with three players
  online at once.
- Completed G12 Shared MapRuntime Phase 4. Player-player visibility now uses
  CMaNGOS-shaped 64-grid / 16-cell coordinate primitives, cell-area radius
  lookup, and nearby cell visiting over the map runtime's cell buckets instead
  of scanning every live player on each visibility update. Distance filtering
  remains after cell candidate lookup.
- Completed the G12 nearby chat slice. `CMSG_MESSAGECHAT` `/say` still echoes
  to the sender as before, but also asks `MapRuntime` for nearby visible
  players and dispatches `SMSG_MESSAGECHAT` through `SessionRegistry` after the
  map lock is released. The `world-flow-test` two-client proof now verifies a
  far client does not receive `/say`, then receives it after moving back into
  visibility range.
- Started the G12 shared DB-creature runtime slice. `MapRuntime` now owns a
  shared DB-creature snapshot map keyed by creature object GUID; login,
  movement, and repop visibility load DB spawns through that shared map so a
  later session reuses existing corpse/dead/respawn state instead of creating a
  fresh private runtime copy. Player melee and supported starter spell damage
  now write the changed DB-creature snapshot back into `MapRuntime` and
  broadcast the health/death `SMSG_UPDATE_OBJECT` to nearby player sessions
  through `SessionRegistry` after releasing the map lock. DB-creature loot open,
  money claim, item claim, item restore on failed autostore, and loot release
  now mutate the shared map creature snapshot so money/items cannot be claimed
  from independent per-session copies. DB-creature combat claims now go through
  `MapRuntime` too, so one creature cannot start separate private combat loops
  for multiple sessions. The per-session combat tick now mirrors active
  attacker/victim/next-swing state from `MapRuntime`, and ready-swing retry plus
  next-swing timing are written back to the shared map. Player melee and
  supported starter spell retaliation both use the shared combat claim path.
  When a player is no longer alive, shared map combat claims for that victim are
  cleared so stale attackers cannot stay reserved after death.
  Creature combat start, creature in-combat flag updates, facing turns, chase
  movement, evade attack-stop/state reset, and return-home movement are now
  dispatched through `MapRuntime` to nearby observer sessions after the direct
  victim packet send. Creature-origin damage packet execution and lifecycle
  finalization still run in the owning session tick until a later slice moves
  those operations behind map events. Lazy grid-loaded DB creature discovery is
  also still pending.
- Fixed the first real-client shared-mob desync blocker reported during G12
  testing: observers could keep a stale session-local DB creature copy moving
  through patrol/chase after another player killed the creature, producing a
  dead mob that still walked and sometimes looked lootable without valid loot.
  Session-local DB creatures now sync from `MapRuntime` snapshots before local
  creature ticks, movement visibility refreshes existing local entries from the
  shared snapshot, shared dead snapshots destroy stale visible local creatures,
  and DB creature death emits a motion-stop `SMSG_MONSTER_MOVE` to nearby
  observers. The `world-flow-test` harness now skips legitimate interleaved
  `SMSG_MONSTER_MOVE` packets around DB gossip/vendor responses.
- Fixed the follow-up real-client blocker where shared state was more correct
  but random/waypoint patrols stopped working: idle/random/waypoint and
  return-home motion now write their updated `DbCreatureRuntime` back to
  `MapRuntime` instead of being overwritten by the next shared snapshot sync.
  New idle motion starts also broadcast `SMSG_MONSTER_MOVE` through the shared
  map so nearby clients see the same patrol spline. The exact 5-yard melee
  boundary is now accepted (`<=`) for player and creature melee reach, avoiding
  the real-client "slightly out of range" feel at the reach threshold.
- Fixed the Phase 1 harness blockers exposed by queued outbound packets:
  self-spawn `SMSG_UPDATE_OBJECT` bodies are chunked by byte size so full
  backpacks stay under the vanilla server-packet cap, `world-flow-test` drains
  legal extra login update/monster-move packets, and `starter-zone-flow-test`
  waits for all expected login creature create blocks instead of assuming a
  fixed update-packet count.
- Hardened the Rust/native mmap path boundary used by G8/G9 Detour-backed
  movement. The only Rust `unsafe` mmap call now lives in
  `crates/wow-network/src/world/mmap_path.rs` behind finite-position,
  tile-range, buffer-length, and output validation; gameplay code and tests use
  the safe wrapper instead of calling FFI directly. The C++ bridge now rejects
  invalid tile ids/non-finite coordinates, caps `.mmtile` data allocation, uses
  RAII for Detour queries, and catches native exceptions before they can cross
  FFI.
- Added `docs/g12_shared_mapruntime_plan.md` as the dedicated plan for the
  user-directed G12 Derisk Multiplayer / Shared MapRuntime milestone.
- Split `crates/wow-network/src/world/session.rs` into CMaNGOS-shaped runtime
  type files without changing behavior: `entities/player.rs`,
  `entities/creature.rs`, `entities/corpse.rs`, `motion/motion_master.rs`,
  `maps/world_data.rs`, and `maps/navigation.rs`.
- Renamed the underlying runtime types toward CMaNGOS vocabulary while keeping
  compatibility aliases for existing call sites: `Player` backs
  `ActiveCharacter`, `Creature` backs `DbCreatureRuntime`, `Corpse` backs
  `PlayerCorpseRuntime`, and `CreatureLoot` backs `DbCreatureLootRuntime`.
- Added an inert CMaNGOS parity scaffold under `crates/wow-network/src/world`
  for subsystems that do not collide with existing live flat modules. See
  `crates/wow-network/src/world/PARITY_LAYOUT.md` for the mapping and future
  split targets for live files like `combat.rs`, `chat.rs`, `quests.rs`,
  `loot.rs`, and `spells.rs`.
- Implemented Quest System v1 for the Northshire golden path.
- Confirmed ClassicDB quest ids: `7` is `Kobold Camp Cleanup`;
  `783` is `A Threat Within`.
- Added DB query helpers for `quest_template`, `creature_questrelation`,
  `creature_involvedrelation`, `item_template` display ids, and
  `character_queststatus`.
- Added character quest DB operations for load, accept, mob-count progress, and
  reward persistence with money grant.
- Added world handlers for quest query, quest giver status, quest list, quest
  details, accept, complete/request reward, choose reward, kill-credit update,
  and quest-log update fields.
- Added player quest-log serialization to enter-world bootstrap and live
  `SMSG_UPDATE_OBJECT` updates.
- Wired DB creature death to grant kill credit for active incomplete quests and
  emit `SMSG_QUESTUPDATE_ADD_KILL` / `SMSG_QUESTUPDATE_COMPLETE`.
- Extended `bins/starter-zone-flow-test` to prove the real ClassicDB
  Northshire quest flow through Marshal McBride (`197`), Kobold Vermin (`6`),
  quest `7`, ten kills, turn-in, reward packet, and
  `character_queststatus.rewarded = 1`.
- Fixed the real-client Marshal McBride interaction path: McBride has
  `NpcFlags = 3` (`GOSSIP | QUESTGIVER`) while Deputy Willem is only
  `QUESTGIVER`, so the client opens McBride with `CMSG_GOSSIP_HELLO`. Rust now
  returns the visible quest list from DB gossip hello when a creature has
  start quests, and `starter-zone-flow-test` covers that path.
- Fixed quest-log progress slotting for real-client testing with multiple
  accepted Northshire quests. Quest accept, kill progress, login bootstrap, and
  reward clear now select the actual quest's deterministic log slot instead of
  always updating slot 0.
- Fixed DB-creature spell/queued-strike kill credit. If a supported starter
  spell such as Human Warrior Heroic Strike is the killing blow, Rust now grants
  quest kill credit and sends attack stop from that path too.
- Fixed the completed-quest McBride click path. After Kobold Camp Cleanup is
  complete, `CMSG_GOSSIP_HELLO` now prioritizes an available turn-in/reward
  offer over the normal start-quest list, which matches the real client's
  yellow-question-mark click path.
- Added `docs/checkpoint2_codebase_audit.md` after Quest v1. The audit
  recommends a behavior-preserving world gameplay module split before XP,
  combat v2, death/respawn, and trainers, followed by a shared DB-creature
  death finalization path.
- Completed the first sustainability split pass from #48. `world/interactions.rs`
  is now an include hub for focused gameplay files (`chat.rs`, `spells.rs`,
  `inventory.rs`, `creatures.rs`, `gossip.rs`, `quests.rs`, `vendors.rs`,
  `combat.rs`, `loot.rs`, and `packet_builders.rs`); `opcodes.rs` owns opcode
  constants; `session.rs` owns runtime/session structs.
- Added `finalize_db_creature_death(...)` so DB creature deaths from melee and
  supported starter spell damage converge before quest kill credit and attack
  stop. XP should hook into this finalizer next.
- Added CMaNGOS-like DB creature corpse/respawn runtime state. Rust now selects
  respawn delay from `creature.spawntimesecsmin/max`, corpse delay from
  `creature_template.CorpseDecay` or CMaNGOS rank defaults, keeps creatures in
  alive/corpse/dead state, destroys expired corpses, recreates respawned
  creatures, and no longer respawns DB creatures immediately on loot release.
- Extended `starter-zone-flow-test` so the RealClassicDb Kobold Camp Cleanup
  proof kills ten distinct Kobold Vermin targets. This keeps the golden path
  green without depending on same-creature instant respawn.
- Completed the DB-side sustainability split from #5. `wow-db/src/character.rs`
  now includes focused files under `wow-db/src/character/` for types, queries,
  lifecycle, creation, state, inventory, progression, starter data, and tests.
  The split preserves the existing public API with no wrapper or dispatch
  overhead.
- Added transaction boundaries to multi-table character creation and hard-delete
  cleanup. The single-row unlink/soft-delete path remains a single update, and
  hot gameplay inventory/money paths were intentionally left out of this
  transaction pass to avoid runtime overhead.
- Fixed the manual real-client stack preserving-data guardrail. By default,
  `scripts/run-client-stack-18085.ps1` now preserves existing `RUSTAUTH`
  characters and only seeds `Rustone` when that account is empty; passing
  `-ResetCharacters` is now required to intentionally wipe and reseed that
  account. The helper DB Guide spawn now anchors near `Rustone` when present or
  the first available `RUSTAUTH` character otherwise.
- Added a mandatory startup performance reminder in `AGENTS.md`: when doing
  CMaNGOS parity, each agent should look for behavior-preserving algorithmic or
  data-structure wins that matter for eventually running thousands of bots, but
  fall back to CMaNGOS behavior when unsure.
- Implemented XP/level-up v1 for the Northshire Quest v1 path.
- Character enum/login bootstrap now carry persisted XP; player self-spawn sets
  `PLAYER_XP` from the character row.
- DB creature templates now load `ExperienceMultiplier`; creature kill XP hooks
  into the shared DB-creature death finalizer after quest kill credit.
- Added CMaNGOS-derived starter XP formulas for gray level, zero difference,
  base creature XP, elite multiplier, creature `ExperienceMultiplier`, and
  quest XP from `RewMoneyMaxLevel`.
- Quest reward now sends `SMSG_QUESTGIVER_QUEST_COMPLETE` with reward XP, then
  grants XP through the same progression path as creature XP.
- Added `SMSG_LOG_XPGAIN`, `SMSG_LEVELUP_INFO`, and player progression
  `SMSG_UPDATE_OBJECT` serialization for level, XP, next-level XP, health,
  powers, and stats.
- Added DB progression persistence for level, XP, health, and powers.
- Extended unit coverage for XP formulas, packet shapes, and progression update
  values.
- Extended `starter-zone-flow-test` to observe creature XP, quest XP,
  `SMSG_LEVELUP_INFO`, player progression updates, and persisted level/XP
  against RealClassicDb Northshire content.
- Implemented Trainer v1 for the Northshire Human Warrior path.
- Corrected the trainer target for real ClassicDB: Brother Paxton (`951`) is a
  priest quest giver, while Llane Beshere (`911`) is the Warrior trainer.
- Added `npc_trainer` DB loading, trainer metadata from `creature_template`,
  `SMSG_TRAINER_LIST`, `CMSG_TRAINER_BUY_SPELL`,
  `SMSG_TRAINER_BUY_SUCCEEDED`, and conservative failure handling.
- Trainer list state now uses CMaNGOS' trainer-row spell versus learned-spell
  split. Rust derives `learned_spell` from `spell_template.EffectTriggerSpell`
  for `SPELL_EFFECT_LEARN_SPELL` rows, so Llane's `6674` trainer cast teaches
  persisted spell `6673` (`Battle Shout`) instead of saving the non-spellbook
  trainer cast.
- Trainer buy now atomically charges money and inserts the learned spell into
  `character_spell`, then sends buy success for the original trainer-row spell,
  `SMSG_LEARNED_SPELL` for immediate spellbook feedback, refreshed initial
  spells, and money update.
- Added `docs/playable_gate_board.md` as the current Northshire Human Warrior
  playable milestone dashboard, updated `AGENTS.md` startup order to require it,
  and aligned `docs/rust_migration_plan.md` agent startup guidance with the new
  playable-gate rule.
- Updated the playable gate board from real-client/user-known status: G1 and
  G2 are green, G3 only has login-radius creature loading rather than movement
  streaming, G4 looks good, G5 is basic but instant-respawn/non-CMaNGOS-like,
  G6 works with #49 polish remaining, and G7 has no progress.
- Split new gates for creature work: G8 Combat Agency tracks whether mobs can
  aggro, chase or enter range, swing, damage, kill, or die; G9 World Creature
  Fidelity tracks DB spawn/template/loot, persistent world-object behavior,
  CMaNGOS-like respawn, and patrol/movement.
- Added MMO-slice guardrail gates: G10 NPC Interaction Fidelity for quest,
  vendor, trainer, gossip, flags, cursor/status, menu text, and failure
  behavior; G11 Persistence + Relog Sanity for state restoration after each
  major Northshire action; and G12 Multi-client Sanity so the world cannot pass
  as a single-player packet demo.
- Extended `starter-zone-flow-test` to request Llane Beshere's trainer list,
  buy `6674`, verify the live learned-spell packet names `6673`, and verify
  `6673` persisted active/enabled.
- Implemented G3 Movement Visibility Streaming v1. Movement packets now trigger
  a throttled DB creature visibility rescan, stage only newly visible creature
  GUIDs in the session, and send chunked `SMSG_UPDATE_OBJECT` create packets
  using the same DB creature create block shape as login bootstrap.
- Added the matching G3 out-of-range cleanup. Movement visibility now compares
  the current DB creature set against the newly visible query, removes creatures
  that left the 100-yard bubble, clears that target if it was the active combat
  target, and sends `SMSG_DESTROY_OBJECT` for each removed creature.
- Tightened the G3 visibility radius from the previous oversized 220-yard
  login bubble to CMaNGOS' normal continent visibility distance of 100 yards
  (`src/game/Entities/ObjectDefines.h`), making real-client pop-in/streaming
  visible during the Northshire walking smoke.
- Extended `starter-zone-flow-test` with a movement heartbeat step that proves a
  creature outside the login visibility set streams after movement against
  RealClassicDb, then drains immediate streaming chunks before continuing the
  existing wolf, quest, XP, and trainer proof.
- Implemented G8 Combat Agency v1. Hostile DB creatures now use a
  CMaNGOS-derived level-delta aggro radius, engage from movement/idle ticks,
  send creature-origin `SMSG_ATTACKSTART`, keep independent creature-attacker
  state separate from the player's active swing target, and deal melee damage
  to the player before the player attacks. This is still stationary starter
  agency, not pathfinding/chase/leash/death.
- Extended `starter-zone-flow-test` to stream a RealClassicDb Kobold Vermin,
  move into melee range, and require kobold-origin attack start,
  attacker-state damage, and player health update before the existing
  kill/loot/quest/XP/trainer proof continues.
- Added a G8 aggro guardrail after real-client smoke showed a friendly guard
  could enter the early aggro path. Until Rust has CMaNGOS faction-template
  reactions, auto-aggro is restricted to known hostile starter entries:
  ClassicDB Kobold Vermin (`6`), Defias Thug (`38`), and the matching
  starter-zone fixture kobold entry. Real-client observation confirmed Young
  Wolf (`299`) is neutral, so wolves are attackable but do not auto-aggro.
- Logged the broader faction-reaction parity gap as GitHub #50 and added the
  missing combat-state/AI observation to existing combat issue #12.
- User confirmed G3 real-client movement visibility streaming is good. Updated
  the playable gate board to mark G3 Green and make G8 the active top
  priority.
- Expanded G8 requirements in `docs/playable_gate_board.md`: faction/reaction
  aggro rules, aggro radius/leash/timings, threat/combat ownership, movement to
  player, melee/ranged/spell range, facing/arc rules, line of sight/path
  validity, swing timers/GCD, combat roll outcomes, and damage formulas.
- Continued the G8 combat-overhaul foundation. Active DB creature combat now
  uses `CreatureCombatState` with attacker GUID, player victim GUID, and
  `next_swing_at` timing instead of a loose `active_creature_attacker` GUID.
- Creature-origin melee damage is now gated by a narrow server-side melee reach
  check before applying damage. The starter-zone aggro proof moves the player
  inside that reach so the current stationary-combat slice remains honest until
  chase/move-into-range is implemented.
- Visibility cleanup now clears active creature combat state when a DB creature
  leaves the 100-yard movement bubble.
- Recommended next implementation ladder for G8:
  1. faction reaction gate;
  2. creature combat state and threat/victim ownership;
  3. melee chase / move-into-range v1;
  4. range and facing-gated swing timers;
  5. leash, evade, and return home;
  6. melee roll table;
  7. damage formula v1;
  8. spell, GCD, and queued melee integration.
- Pivoted the next G8 chase work away from a synthetic straight-line
  `SMSG_MONSTER_MOVE` shortcut after comparing the CMaNGOS reference paths:
  `CreatureAI::AttackStart`, `Unit::Attack`, `UnitAI::HandleMovementOnAttackStart`,
  `MotionMaster::MoveChase`, `ChaseMovementGenerator`, and
  `Unit::UpdateMeleeAttackingState`. The fake chase packet/runtime-position
  mutation was removed before it became foundation; the next movement slice
  should introduce a CMaNGOS-shaped motion/chase/spline state instead.
- Implemented the first CMaNGOS-shaped G8 chase foundation. DB creature runtime
  now keeps home/current position separate from immutable spawn data, owns an
  `Idle`/`Chase` motion state with target, start, destination, start time,
  and duration, sends `SMSG_MONSTER_MOVE` from aggro `AttackStart`,
  advances current position by elapsed time on combat ticks, and only allows
  creature-origin melee damage after the timed chase reaches melee range.
- Extended the starter-zone RealClassicDb harness so Kobold Vermin aggro is
  proven from just outside melee range: the harness now requires kobold-origin
  `SMSG_ATTACKSTART`, `SMSG_MONSTER_MOVE`, later creature damage, and a player
  health update before continuing the existing kill/loot/quest/trainer proof.
- Fixed the first chase real-client regression report. The world loop now runs
  a 250ms tick like the CMaNGOS `ChaseMovementGenerator` recheck cadence, while
  player auto-swings keep their own 2s `active_combat_next_swing_at` timer so
  the faster world tick does not create machine-gun melee.
- DB creature chase now rechecks the active player position every 250ms and
  sends a fresh `SMSG_MONSTER_MOVE` when the destination moved far enough. This
  keeps an aggroed mob following a moving player instead of committing forever
  to the player's position at initial aggro.
- Fixed the follow-up chase regression from real-client smoke. Movement
  visibility cleanup no longer removes an active combat creature just because
  its DB spawn point fell out of the normal nearby-spawn query while chasing,
  so kiting out of the spawn radius does not delete the attacker and clear
  combat. Chase re-pathing also now uses the CMaNGOS-style melee stop distance
  as the destination-change threshold instead of refreshing splines for tiny
  sub-yard target shifts.
- Fixed the next chase stall report from real-client smoke. The world loop no
  longer depends on socket-read timeouts alone to run combat/chase ticks; it
  tracks the next world tick deadline and runs due ticks after packet handling
  too, so continuous movement packets while kiting do not starve
  `handle_combat_tick`.
- Adjusted `starter-zone-flow-test` so G3 destroy cleanup is proven before
  kobold aggro/combat, then the same kobold is streamed again for the G8 aggro
  proof. This keeps the harness aligned with the new rule: non-combat
  out-of-range creatures are destroyed, active combat creatures are retained.
- Added the G8 path validity / LOS / navigation guardrail slice. Aggro target
  selection, chase destination generation, and creature melee reach now all
  pass through a single DB-creature navigation check. The current backend is an
  explicit permissive terrain/path placeholder until real map or navmesh data
  is wired, but the combat API no longer bakes in distance-only assumptions.
- Added the first G8 leash/evade/return-home slice. Active DB creatures now use
  the CMaNGOS default 30-yard leash radius as a starter guardrail; when they
  exceed it, Rust clears player/creature combat state, resets the creature to
  max health, sends attack stop plus a creature health update, and starts a
  timed `ReturnHome` motion using the same monster-move packet builder as
  chase.
- Fixed the first return-home real-client smoke regression. A creature that was
  already evading home could immediately reacquire the player from the normal
  aggro scan when the player walked toward it, causing chase/home flip-flopping.
  `ReturnHome` now acts as an evade state: DB creatures in that motion cannot
  auto-aggro and ignore player damage until they finish returning home.
- Fixed the next return-home smoke regression where creatures could sometimes
  appear to stop instead of fully running home. Return-home motion now advances
  from the normal world tick even after combat has been cleared, and home
  movement no longer reuses the combat LOS/path guardrail; like CMaNGOS'
  `HomeMovementGenerator`, it still attempts to go home even when combat
  pathing is imperfect.
- Tightened G8 aggro parity against the CMaNGOS reference path:
  `CreatureInfo::Detection` -> `Creature::GetDetectionRange()` ->
  `Unit::GetAttackDistance()`. Rust now loads `creature_template.Detection`
  through `CreatureTemplateQuery`, uses it as the base aggro distance before
  applying the existing level delta/minimum clamp, and seeds starter-zone
  fixture creatures with a 20-yard detection value so fixture and RealClassicDb
  paths exercise the same field.
- Added the G8 range/facing-gated player melee slice after comparing CMaNGOS
  `Unit::UpdateMeleeAttackingState`, `Unit::CanReachWithMeleeAttack`, and
  `WorldObject::HasInArc`. DB-creature player swings now start auto-attack but
  only apply damage when the active player is same-map, navigation-clear,
  within the CMaNGOS minimum 5-yard 3D melee reach, and inside the 120-degree
  forward melee arc. Supported starter melee spell fixtures now use the same
  DB-creature melee validity check before applying direct DB-creature damage.
- Updated `starter-zone-flow-test` to move into melee range and face the real
  ClassicDB wolf/kobold before scripted melee proof steps, so the harness
  proves the new server-side range/facing gates instead of relying on
  distance-free fixture damage.
- Fixed two follow-up real-client observations from the range/facing smoke:
  far right-clicking a DB creature no longer starts creature retaliation/chase
  before a valid landed player hit, and supported starter melee spells now
  return a CMaNGOS-shaped `SMSG_CAST_RESULT` failure before power spend,
  `SMSG_SPELL_GO`, or damage when range/facing is invalid.

## Tests Run

- G12 shared-mob stale observer fix: `cargo fmt --check` passed.
- G12 shared-mob stale observer focused visibility test:
  `cargo test -p wow-network
  movement_visibility_refreshes_existing_creature_from_shared_dead_snapshot --lib`
  passed.
- G12 shared-mob stale observer movement regression:
  `cargo test -p wow-network movement_visibility --lib` passed (`8` tests).
- G12 shared-mob stale observer death-stop test:
  `cargo test -p wow-network db_creature_death_motion_stop_clears_active_motion
  --lib` passed.
- G12 shared-mob stale observer map-runtime regression:
  `cargo test -p wow-network map_runtime --lib` passed (`9` tests).
- G12 shared-mob stale observer compile/clippy:
  `cargo check -p wow-network -p world-flow-test` and
  `cargo clippy --workspace --all-targets -- -D warnings` passed.
- G12 shared-mob stale observer Rust baseline:
  `.\scripts\test-rust.cmd` passed after stopping a manual
  `run-client-stack-18085.cmd` auth/worldserver process that had locked
  `target\debug\authserver.exe`; `wow-network` now has `175` unit tests.
- G12 shared-mob stale observer world-flow regression:
  `.\scripts\test-world-flow.cmd` passed after teaching the harness to drain
  legal interleaved `SMSG_MONSTER_MOVE` packets during DB vendor sellback.
- G12 shared-mob patrol restoration checks: `cargo fmt --check` passed.
- G12 shared-mob patrol restoration focused tests:
  `cargo test -p wow-network
  shared_db_creature_idle_motion_updates_map_and_observers --lib`,
  `cargo test -p wow-network db_creature_melee_reach_is_position_gated --lib`,
  and `cargo test -p wow-network
  db_creature_return_home_motion_advances_without_active_combat --lib` passed.
- G12 shared-mob patrol restoration unit baseline:
  `cargo test -p wow-network --lib` passed (`176` tests).
- G12 shared-mob patrol restoration compile/clippy:
  `cargo check -p wow-network -p world-flow-test` and
  `cargo clippy --workspace --all-targets -- -D warnings` passed.
- G12 shared-mob patrol restoration harness/baseline:
  `.\scripts\test-world-flow.cmd` passed after stopping stale local
  auth/worldserver processes from manual client testing, and
  `.\scripts\test-rust.cmd` passed.
- G12 shared DB-creature tick timing checks: `cargo fmt --check` passed.
- G12 shared DB-creature tick timing focused test: `cargo test -p wow-network
  db_creature_combat_state_tracks_victim_and_next_swing --lib` passed.
- G12 shared DB-creature tick timing map-runtime regression:
  `cargo test -p wow-network map_runtime --lib` passed (`9` tests).
- G12 shared DB-creature tick timing compile check:
  `cargo check -p wow-network -p world-flow-test` passed.
- G12 shared DB-creature tick timing clippy/baseline:
  `.\scripts\test-rust.cmd` passed (`wow-network` now has `172` unit tests).
- G12 shared DB-creature tick timing world-flow regression:
  `.\scripts\test-world-flow.cmd` passed.
- G12 shared DB-creature observer broadcast check: `cargo fmt --check` passed.
- G12 shared DB-creature observer broadcast focused test:
  `cargo test -p wow-network
  shared_creature_combat_start_broadcasts_to_nearby_observer --lib` passed.
- G12 shared DB-creature observer broadcast map-runtime regression:
  `cargo test -p wow-network map_runtime --lib` passed (`9` tests).
- G12 shared DB-creature observer broadcast compile/clippy:
  `cargo check -p wow-network -p world-flow-test` and
  `cargo clippy --workspace --all-targets -- -D warnings` passed.
- G12 shared DB-creature observer broadcast baseline:
  `.\scripts\test-rust.cmd` passed (`wow-network` now has `173` unit tests).
- G12 shared DB-creature observer broadcast world-flow regression:
  `.\scripts\test-world-flow.cmd` passed.
- G12 Phase 1 baseline before code changes: `.\scripts\test-rust.cmd` passed.
- G12 Phase 1 implementation check: `cargo check -p wow-network` passed.
- G12 Phase 1 world flow gate: `.\scripts\test-world-flow.cmd` passed.
- G12 Phase 1 starter-zone/G3 regression gate:
  `.\scripts\test-starter-zone-flow.cmd` passed:
  `starter-zone RealClassicDb lock passed for account STARTZONE, character Startzone`.
- G12 Phase 1 final check after all edits: `.\scripts\test-rust.cmd` passed.
- G12 Phase 2 implementation check: `cargo check -p wow-network -p
  world-flow-test -p starter-zone-flow-test` passed.
- G12 Phase 2 Rust baseline: `.\scripts\test-rust.cmd` passed.
- G12 Phase 2 two-client proof: `.\scripts\test-world-flow.cmd` passed and now
  reports `two-client login/logout visibility`.
- G12 Phase 2 starter-zone/G3 regression: `.\scripts\test-starter-zone-flow.cmd`
  passed: `starter-zone RealClassicDb lock passed for account STARTZONE,
  character Startzone`.
- G12 nearby chat implementation check: `cargo fmt --check` passed.
- G12 nearby chat implementation check: `cargo check -p wow-network -p
  world-flow-test` passed.
- G12 nearby chat two-client proof: `.\scripts\test-world-flow.cmd` passed and
  now covers nearby `/say` delivery plus out-of-range non-delivery.
- G12 nearby chat baseline: `.\scripts\test-rust.cmd` passed.
- G12 shared DB-creature snapshot checks: `cargo fmt --check` passed.
- G12 shared DB-creature snapshot checks: `cargo test -p wow-network
  map_runtime --lib` passed (`5` tests).
- G12 shared DB-creature snapshot compile check: `cargo check -p wow-network -p
  world-flow-test` passed.
- G12 shared DB-creature world-flow regression:
  `.\scripts\test-world-flow.cmd` passed after stopping the manual
  `run-client-stack-18085.cmd` auth/worldserver processes that had locked
  `target\debug\authserver.exe`.
- G12 shared DB-creature baseline: `.\scripts\test-rust.cmd` passed.
- G12 shared DB-creature loot authority checks: `cargo fmt --check` passed.
- G12 shared DB-creature loot authority checks: `cargo test -p wow-network
  map_runtime --lib` passed (`7` tests).
- G12 shared DB-creature loot compile check: `cargo check -p wow-network -p
  world-flow-test` passed.
- G12 shared DB-creature loot world-flow regression:
  `.\scripts\test-world-flow.cmd` passed.
- G12 shared DB-creature loot baseline: `.\scripts\test-rust.cmd` passed.
- G12 shared DB-creature combat-claim checks: `cargo fmt --check` passed.
- G12 shared DB-creature combat-claim checks: `cargo test -p wow-network
  map_runtime --lib` passed (`8` tests).
- G12 shared DB-creature combat-claim world-flow regression:
  `.\scripts\test-world-flow.cmd` passed.
- G12 shared DB-creature combat-claim baseline: `.\scripts\test-rust.cmd`
  passed.
- Mmap safety hardening baseline before changes: `.\scripts\test-rust.cmd`
  passed.
- Mmap safety hardening focused check: `cargo test -p wow-network mmap --
  --nocapture` passed (`3` tests).
- Mmap safety hardening final check: `.\scripts\test-rust.cmd` passed.
- CMaNGOS-shaped type split: `cargo fmt --check` passed.
- CMaNGOS-shaped type split: `cargo check -p wow-network` passed.
- CMaNGOS-shaped type split: `cargo test -p wow-network --lib` passed
  (`163` tests).
- CMaNGOS-shaped parity scaffold: `cargo fmt --check` and
  `cargo check -p wow-network` passed after adding the placeholder files.
- CMaNGOS-shaped type split: `.\scripts\test-rust.cmd` reached green
  formatting/check/unit/doc-test coverage, then failed only at the final
  `cargo build -p authserver` because `target/debug/authserver.exe` was locked
  by a running `authserver` process (`Access is denied`, pid `39576`).
- `git status --short --branch`
- G8 guardrail follow-up: `cargo fmt` passed; it still prints the known
  `could not canonicalize path C:\Users\subhe` warning.
- G8 guardrail follow-up: `cargo test -p wow-network db_creature --lib` passed
  with 12 tests, including the new non-starter hostile guard test.
- G8 guardrail follow-up: `cargo check -p wow-network -p
  starter-zone-flow-test` passed.
- G8 guardrail follow-up: first `.\scripts\test-rust.cmd` run passed the Rust
  unit/doc-test portions but failed rebuilding `authserver.exe` because local
  `authserver`/`worldserver` processes held executable locks; after stopping
  those processes, `.\scripts\test-rust.cmd` passed.
- G8 guardrail follow-up: `.\scripts\test-starter-zone-flow.cmd` required
  elevated Docker access and passed:
  `starter-zone RealClassicDb lock passed for account STARTZONE, character Startzone`.
- Baseline before changes: `.\scripts\test-rust.cmd` passed.
- `cargo fmt` passed; it still prints the known
  `could not canonicalize path C:\Users\subhe` warning.
- `cargo check -p wow-db -p wow-network -p starter-zone-flow-test` passed.
- `.\scripts\test-starter-zone-flow.cmd` passed in 23 seconds:
  `starter-zone RealClassicDb lock passed for account STARTZONE, character Startzone`.
- Final `.\scripts\test-rust.cmd` passed: clippy, tests, doc-tests, and builds.
- After the McBride gossip-hello fix, `.\scripts\test-starter-zone-flow.cmd`
  passed again in 28 seconds, and `.\scripts\test-rust.cmd` passed again.
- After the quest-log slot fix, `.\scripts\test-starter-zone-flow.cmd` passed
  again in 27 seconds, and `.\scripts\test-rust.cmd` passed again.
- After the spell-kill credit fix, `.\scripts\test-starter-zone-flow.cmd`
  passed again in 27 seconds with Heroic Strike covering the first Vermin kill,
  and `.\scripts\test-rust.cmd` passed again.
- After the completed-quest McBride click fix, `cargo check -p wow-network -p
  starter-zone-flow-test` passed, `.\scripts\test-starter-zone-flow.cmd` passed
  again in 26 seconds with the harness opening the reward offer through
  `CMSG_GOSSIP_HELLO`, and `.\scripts\test-rust.cmd` passed again.
- Codebase audit commands were read-only: file line-count scan, Rust handler
  symbol scan, and CMaNGOS reference-path search. No Rust tests were rerun for
  the docs-only audit addition.
- Baseline before the split: `.\scripts\test-rust.cmd` passed.
- After the split/finalizer pass: `cargo check -p wow-network -p
  starter-zone-flow-test` passed; first `.\scripts\test-rust.cmd` rerun passed
  tests but hit the known Windows stale `authserver.exe` file lock during final
  build; after stopping stale local server processes, `.\scripts\test-rust.cmd`
  passed; elevated `.\scripts\test-starter-zone-flow.cmd` passed against
  RealClassicDb content in 23 seconds.
- Before the DB character split: `.\scripts\test-rust.cmd` passed.
- After the DB character split/transaction pass: `cargo check -p wow-db`
  passed; first `.\scripts\test-rust.cmd` rerun passed tests but hit the known
  Windows stale `authserver.exe` file lock during final build; after stopping
  stale local server processes, `.\scripts\test-rust.cmd` passed; elevated
  `.\scripts\test-character-lifecycle.cmd` passed; elevated
  `.\scripts\test-starter-zone-flow.cmd` passed against RealClassicDb content
  in 25 seconds.
- After the manual stack persistence fix:
  `[System.Management.Automation.Language.Parser]::ParseFile(...)` passed for
  `scripts/run-client-stack-18085.ps1`.
- `git diff --check` passed with only normal LF-to-CRLF working-copy warnings.
- Baseline before XP/level-up v1: `.\scripts\test-rust.cmd` passed.
- During XP/level-up v1: `cargo check -p wow-db -p wow-network -p
  starter-zone-flow-test` passed; `cargo test -p wow-db -p wow-network --lib`
  passed; `cargo fmt` passed with the known `could not canonicalize path
  C:\Users\subhe` warning.
- After XP/level-up v1: `.\scripts\test-rust.cmd` passed.
- During starter-zone verification, the first elevated
  `.\scripts\test-starter-zone-flow.cmd` attempts exposed harness packet-drain
  assumptions around XP/level-up packets; after making the harness observe
  progression packets while waiting for expected quest/loot packets, elevated
  `.\scripts\test-starter-zone-flow.cmd` passed against RealClassicDb content:
  `starter-zone RealClassicDb lock passed for account STARTZONE, character
  Startzone`.
- Final after the harness fix: `.\scripts\test-rust.cmd` passed again.
- Baseline before Trainer v1: `.\scripts\test-rust.cmd` passed.
- During Trainer v1: `cargo check -p wow-db -p wow-network -p
  starter-zone-flow-test` passed; `cargo test -p wow-network trainer --lib`
  passed; `cargo test -p wow-db -p wow-network --lib` passed with 101
  `wow-network` tests.
- Elevated `.\scripts\test-starter-zone-flow.cmd` first hit the known local
  stale `authserver.exe` / `worldserver.exe` file lock; after stopping those
  local real-client stack processes, the next run exposed a DB tinyint signedness
  mismatch for trainer metadata, which was fixed locally.
- Final elevated `.\scripts\test-starter-zone-flow.cmd` passed against
  RealClassicDb content: `starter-zone RealClassicDb lock passed for account
  STARTZONE, character Startzone`.
- Final `.\scripts\test-rust.cmd` passed again.
- Baseline before G8 aggro/mob behavior: `.\scripts\test-rust.cmd` passed.
- During G8: `cargo fmt` passed with the known `could not canonicalize path
  C:\Users\subhe` warning; `cargo check -p wow-network -p
  starter-zone-flow-test` passed; `cargo test -p wow-network db_creature --lib`
  passed; `cargo test -p wow-network movement_visibility --lib` passed.
- First elevated `.\scripts\test-starter-zone-flow.cmd` hit the known stale
  `authserver.exe` / `worldserver.exe` file lock; after stopping those local
  processes, the next run proved the new wolf aggro step but exposed that the
  harness had moved away from the kobold streaming area before quest kills.
  Reordering the harness to prove wolf aggro first, then movement-stream the
  kobold, fixed the harness flow.
- Elevated `.\scripts\test-starter-zone-flow.cmd` passed against RealClassicDb:
  `starter-zone RealClassicDb lock passed for account STARTZONE, character
  Startzone`.
- Final `.\scripts\test-rust.cmd` passed again.
- After lowering the visibility radius to 100 yards: elevated
  `.\scripts\test-starter-zone-flow.cmd` passed against RealClassicDb after
  fixing the harness login packet-count and fixture availability assumptions;
  final `.\scripts\test-rust.cmd` passed again.
- After the real-client report that streamed creatures never disappeared:
  `cargo test -p wow-network movement_visibility --lib` passed with new destroy
  staging coverage; first elevated `.\scripts\test-starter-zone-flow.cmd`
  rerun hit the known stale `authserver.exe` file lock, then exposed a harness
  midpoint issue after out-of-range cleanup; after adjusting the movement proof,
  elevated `.\scripts\test-starter-zone-flow.cmd` passed against RealClassicDb;
  final `.\scripts\test-rust.cmd` passed again.
- After the real-client trainer bug report, `cargo check -p wow-db -p
  wow-network -p starter-zone-flow-test` passed; `cargo test -p wow-network
  trainer --lib` passed; `cargo test -p wow-db -p wow-network --lib` passed;
  elevated `.\scripts\test-starter-zone-flow.cmd` passed against RealClassicDb;
  final `.\scripts\test-rust.cmd` passed.
- After the real-client "only appears after relog" trainer report, `cargo fmt`
  passed; `cargo check -p wow-network -p starter-zone-flow-test` passed;
  `cargo test -p wow-network trainer --lib` passed; elevated
  `.\scripts\test-starter-zone-flow.cmd` passed with a live
  `SMSG_LEARNED_SPELL` assertion; final `.\scripts\test-rust.cmd` passed.
- Docs-only playable gate board/protocol update: `git status --short --branch`
  and docs diffs reviewed. Rust tests were not rerun because no Rust code or
  harness behavior changed.
- Baseline before G3 movement streaming: `.\scripts\test-rust.cmd` passed.
- During G3: `cargo check -p wow-network` passed; `cargo check -p wow-network
  -p starter-zone-flow-test` passed; `cargo test -p wow-network
  movement_visibility --lib` passed.
- First non-elevated `.\scripts\test-starter-zone-flow.cmd` failed because
  Docker access was denied by the Windows sandbox. Elevated rerun exposed a
  harness packet-drain blocker after movement streaming; after draining
  immediate movement-stream packets, elevated
  `.\scripts\test-starter-zone-flow.cmd` passed against RealClassicDb:
  `starter-zone RealClassicDb lock passed for account STARTZONE, character
  Startzone`.
- Final `.\scripts\test-rust.cmd` passed again.
- G8 combat-state follow-up: `cargo fmt` passed with the known
  `could not canonicalize path C:\Users\subhe` warning.
- G8 combat-state follow-up: `cargo test -p wow-network db_creature --lib`
  passed with 15 targeted tests, including creature combat-state, melee reach,
  and neutral Young Wolf no-aggro coverage.
- G8 combat-state follow-up: `cargo check -p wow-network -p
  starter-zone-flow-test` passed.
- G8 combat-state follow-up: `.\scripts\test-rust.cmd` passed.
- G8 combat-state follow-up: first non-elevated
  `.\scripts\test-starter-zone-flow.cmd` failed because Docker access was
  denied by the Windows sandbox; elevated rerun passed against RealClassicDb:
  `starter-zone RealClassicDb lock passed for account STARTZONE, character
  Startzone`.
- G8 neutral-wolf correction: elevated `.\scripts\test-starter-zone-flow.cmd`
  initially exposed a harness parser assumption while skipping movement-stream
  create packets, then passed after tightening the packet wait:
  `starter-zone RealClassicDb lock passed for account STARTZONE, character
  Startzone`.
- G8 Defias Thug correction: real-client observation showed Defias Thugs were
  not aggroing. Local ClassicDB/ACID data identifies Defias Thug as entry `38`;
  the temporary starter-hostile gate now includes entry `38` with targeted unit
  coverage.
- G8 retaliation cleanup: removed the old hardcoded immediate DB-creature
  retaliation from the player swing path. Creature-origin damage now comes from
  active creature combat ticks, which use attacker/victim state, creature base
  attack timing, and the current melee reach gate.
- G8 retaliation cleanup tests: `cargo fmt` passed; `cargo test -p wow-network
  db_creature --lib` passed; `cargo check -p wow-network -p
  starter-zone-flow-test` passed; elevated
  `.\scripts\test-starter-zone-flow.cmd` passed against RealClassicDb; final
  `.\scripts\test-rust.cmd` passed.
- G8 attackback fix: after removing immediate retaliation, the combat tick
  still returned early from the player auto-swing path when
  `active_combat_target` was a DB creature, starving the creature's own
  reach-gated attack tick. The DB creature player-swing tick now falls through
  to `send_active_db_creature_attack(...)`, so mobs can hit back while the
  player is auto-attacking.
- G8 attackback tests: `cargo fmt` passed; `cargo test -p wow-network
  db_creature --lib` passed; `cargo check -p wow-network -p
  starter-zone-flow-test` passed; first elevated
  `.\scripts\test-starter-zone-flow.cmd` rerun hit the known local
  `authserver.exe` file lock, then passed after stopping stale server
  processes; final `.\scripts\test-rust.cmd` passed.
- G8 CMaNGOS-parity pivot: removed the synthetic straight-line chase shortcut
  before landing it. `cargo fmt` passed with the known canonicalize warning;
  `cargo test -p wow-network db_creature --lib` passed with 16 targeted tests;
  `cargo check -p wow-network -p starter-zone-flow-test` passed;
  `.\scripts\test-rust.cmd` first hit the known local `authserver.exe` file
  lock during the final build step, then passed after stopping stale local
  server/test processes; elevated `.\scripts\test-starter-zone-flow.cmd` passed
  against RealClassicDb.
- G8 chase foundation: `cargo fmt` passed with the known canonicalize warning;
  `cargo test -p wow-network db_creature --lib` passed with 18 targeted tests,
  including runtime-home/current-position separation and timed chase movement;
  `cargo check -p wow-network -p starter-zone-flow-test` passed; elevated
  `.\scripts\test-starter-zone-flow.cmd` passed against RealClassicDb with a
  kobold `SMSG_MONSTER_MOVE` assertion; final `.\scripts\test-rust.cmd` passed
  with 114 `wow-network` tests.
- G8 chase re-path follow-up: `cargo fmt` passed with the known canonicalize
  warning; `cargo test -p wow-network db_creature --lib` passed with 20
  targeted tests, including no-repath-before-recheck and
  repath-after-player-move coverage; `cargo check -p wow-network -p
  starter-zone-flow-test` passed; first non-elevated
  `.\scripts\test-starter-zone-flow.cmd` failed because Docker access was
  denied, and the first elevated rerun hit the known local `authserver.exe`
  file lock; after stopping stale local server/test processes, elevated
  `.\scripts\test-starter-zone-flow.cmd` passed against RealClassicDb; final
  `.\scripts\test-rust.cmd` passed with 116 `wow-network` tests.
- G8 chase visibility/jitter follow-up: `cargo fmt` passed with the known
  canonicalize warning; `cargo test -p wow-network db_creature --lib` passed
  with 21 targeted tests; `cargo test -p wow-network movement_visibility --lib`
  passed with 4 targeted tests, including active-combat retention while outside
  the spawn visibility query; `cargo check -p wow-network -p
  starter-zone-flow-test` passed; first elevated
  `.\scripts\test-starter-zone-flow.cmd` hit the known local `authserver.exe`
  file lock, then passed after stopping stale local server/test processes;
  final `.\scripts\test-rust.cmd` passed with 118 `wow-network` tests.
- G8 chase tick-starvation follow-up: `cargo fmt` passed with the known
  canonicalize warning; `cargo test -p wow-network world_tick --lib` passed
  with 2 targeted tests; `cargo test -p wow-network db_creature --lib` passed
  with 21 targeted tests; `cargo check -p wow-network -p
  starter-zone-flow-test` passed; first elevated
  `.\scripts\test-starter-zone-flow.cmd` hit the known local `authserver.exe`
  file lock, then the rerun exposed a stale harness expectation that final
  post-combat movement must destroy the kobold; after moving that destroy proof
  before combat, elevated `.\scripts\test-starter-zone-flow.cmd` passed against
  RealClassicDb; final `.\scripts\test-rust.cmd` passed with 120
  `wow-network` tests.
- G8 navigation guardrail: baseline `.\scripts\test-rust.cmd` passed. After
  changes, `cargo fmt` passed with the known canonicalize warning; `cargo test
  -p wow-network db_creature --lib` passed with 23 targeted tests; `cargo check
  -p wow-network -p starter-zone-flow-test` passed; first non-elevated
  `.\scripts\test-starter-zone-flow.cmd` failed because Docker access was
  denied, and the elevated rerun passed against RealClassicDb:
  `starter-zone RealClassicDb lock passed for account STARTZONE, character
  Startzone`.
- G8 leash/evade/return-home: `cargo fmt` passed with the known canonicalize
  warning; `cargo test -p wow-network db_creature --lib` passed with 25
  targeted tests; `cargo check -p wow-network -p starter-zone-flow-test`
  passed; first non-elevated `.\scripts\test-starter-zone-flow.cmd` failed
  because Docker access was denied, and the elevated rerun passed against
  RealClassicDb: `starter-zone RealClassicDb lock passed for account STARTZONE,
  character Startzone`.
- G8 return-home flip-flop fix: `cargo fmt` passed with the known canonicalize
  warning; `cargo test -p wow-network db_creature --lib` passed with 26
  targeted tests; `cargo check -p wow-network -p starter-zone-flow-test`
  passed; first non-elevated `.\scripts\test-starter-zone-flow.cmd` failed
  because Docker access was denied, the first elevated rerun hit the known
  local `authserver.exe` / `worldserver.exe` file lock, and the elevated rerun
  after stopping stale local server processes passed against RealClassicDb:
  `starter-zone RealClassicDb lock passed for account STARTZONE, character
  Startzone`.
- G8 return-home stall fix: `cargo fmt` passed with the known canonicalize
  warning; `cargo test -p wow-network db_creature --lib` passed with 28
  targeted tests; `cargo check -p wow-network -p starter-zone-flow-test`
  passed; first non-elevated `.\scripts\test-starter-zone-flow.cmd` failed
  because Docker access was denied, the first elevated rerun hit the known
  local `authserver.exe` / `worldserver.exe` file lock, and the elevated rerun
  after stopping stale local server processes passed against RealClassicDb:
  `starter-zone RealClassicDb lock passed for account STARTZONE, character
  Startzone`.
- G8 aggro detection parity: `cargo fmt` passed with the known canonicalize
  warning; `cargo test -p wow-network db_creature --lib` passed with 29
  targeted tests; `cargo check -p wow-network -p starter-zone-flow-test`
  passed; first non-elevated `.\scripts\test-starter-zone-flow.cmd` failed
  because Docker access was denied, and the elevated rerun passed against
  RealClassicDb: `starter-zone RealClassicDb lock passed for account STARTZONE,
  character Startzone`; final `.\scripts\test-rust.cmd` passed with 128
  `wow-network` tests.
- G8 range/facing player melee: baseline `.\scripts\test-rust.cmd` passed.
  After changes, `cargo fmt` passed with the known canonicalize warning;
  `cargo test -p wow-network db_creature --lib` passed with 31 targeted tests;
  `cargo check -p wow-network -p starter-zone-flow-test` passed; first
  non-elevated `.\scripts\test-starter-zone-flow.cmd` failed because Docker
  access was denied, the first elevated rerun hit the known local
  `authserver.exe` file lock, and the elevated rerun after clearing stale
  local processes passed against RealClassicDb:
  `starter-zone RealClassicDb lock passed for account STARTZONE, character
  Startzone`; final `.\scripts\test-rust.cmd` passed with 130 `wow-network`
  tests.
- G8 far attack / melee spell follow-up: `cargo fmt` passed with the known
  canonicalize warning; `cargo test -p wow-network db_creature --lib` passed
  with 31 targeted tests; `cargo test -p wow-network starter_spell --lib`
  passed with 3 targeted tests; `cargo test -p wow-network melee --lib` passed
  with 6 targeted tests; `cargo check -p wow-network -p starter-zone-flow-test`
  passed; first non-elevated `.\scripts\test-starter-zone-flow.cmd` failed
  because Docker access was denied, the first elevated rerun hit the known
  local `authserver.exe` file lock, and the elevated rerun after clearing stale
  local processes passed against RealClassicDb:
  `starter-zone RealClassicDb lock passed for account STARTZONE, character
  Startzone`; final `.\scripts\test-rust.cmd` passed with 131 `wow-network`
  tests.
- G8 chase stop/repath parity: baseline `.\scripts\test-rust.cmd` passed.
  Rust creature melee reach now uses the CMaNGOS combined melee reach floor of
  5 yards for creature-origin reach checks, chase destinations stop at half
  that range, and chase re-pathing waits for the full melee-reach window before
  sending a refreshed spline. The starter-zone kobold aggro proof now starts
  just outside that 5-yard reach so it still proves `SMSG_MONSTER_MOVE`.
  `cargo fmt` passed with the known canonicalize warning; `cargo test -p
  wow-network db_creature_chase --lib` passed with 4 targeted tests; `cargo
  test -p wow-network db_creature --lib` passed with 31 targeted tests; `cargo
  check -p wow-network -p starter-zone-flow-test` passed; first non-elevated
  `.\scripts\test-starter-zone-flow.cmd` failed because Docker access was
  denied, the first elevated rerun hit the known local `authserver.exe` file
  lock, and the elevated rerun after stopping stale local server processes
  passed against RealClassicDb; final `.\scripts\test-rust.cmd` passed with
  131 `wow-network` tests.
- G8 real-client combat-state follow-up: creature aggro now sends CMaNGOS-style
  `UNIT_FIELD_FLAGS` updates with `UNIT_FLAG_IN_COMBAT` for the player and
  aggroing creature, and evade/death clear the flag again. This targets the
  real-client issue where crossed swords only appeared after the player
  right-clicked even though a hostile creature already had aggro.
- G8 creature-facing follow-up: creature-origin melee now requires the player
  to be inside the creature's forward melee arc before damage lands. When the
  player is in reach but behind the creature, Rust turns the creature server
  position toward the player and uses the same 100ms ready-swing retry cadence
  shape as the existing out-of-range retry path.
- G8 real-client combat-state tests: `cargo fmt` passed with the known
  canonicalize warning; `cargo test -p wow-network
  combat_unit_flag_updates_include_cmangos_in_combat_bit --lib` passed; `cargo
  test -p wow-network db_creature --lib` passed with 31 targeted tests; `cargo
  check -p wow-network -p starter-zone-flow-test` passed; first
  `.\scripts\test-rust.cmd` hit the known local `authserver.exe` file lock
  from the running client stack, and the rerun passed after stopping the stale
  auth/world server processes.
- G8 CMaNGOS chase-facing follow-up: compared CMaNGOS
  `ChaseMovementGenerator::DispatchSplineToPosition`, which calls
  `MoveSplineInit::SetFacing(i_target)`, and
  `PacketBuilder::WriteCommonMonsterMovePart`, which serializes that as
  `MonsterMoveFacingTarget`. Rust chase `SMSG_MONSTER_MOVE` packets now carry
  the facing-target variant for player chase splines instead of plain normal
  monster movement.
- G8 chase-facing tests: `cargo fmt` passed with the known canonicalize
  warning; `cargo test -p wow-network
  chase_monster_move_can_face_target_like_cmangos_spline --lib` passed; `cargo
  test -p wow-network db_creature --lib` passed with 31 targeted tests; `cargo
  check -p wow-network -p starter-zone-flow-test` passed; `.\scripts\test-rust.cmd`
  passed with 133 `wow-network` tests after stopping the running local
  auth/world processes to avoid executable locks.
- G8 in-place facing follow-up: real-client smoke showed mobs still did not
  visibly need to turn around when already inside melee range. The bad-facing
  branch now sends a same-position `SMSG_MONSTER_MOVE` with
  `MonsterMoveFacingTarget`, increments the creature spline id, then uses the
  short ready-swing retry instead of silently rotating only server-side.
- G8 in-place facing tests: `cargo fmt` passed with the known canonicalize
  warning; `cargo test -p wow-network
  db_creature_melee_reach_is_position_gated --lib` passed; `cargo test -p
  wow-network db_creature --lib` passed with 31 targeted tests; `cargo check -p
  wow-network -p starter-zone-flow-test` passed; first `.\scripts\test-rust.cmd`
  failed on a clippy `question_mark` style lint in the new optional helper,
  and the rerun passed after applying the clippy fix.
- G8 multi-creature combat follow-up: session creature combat state is now
  keyed by attacker GUID instead of a single `Option<CreatureCombatState>`.
  Aggro selection starts every eligible nearby hostile that is not already in
  creature combat, combat ticks iterate each active creature attacker, and
  visibility retention keeps all active combat creatures instead of just one.
  This is still not full CMaNGOS threat/social aggro, but it removes the
  one-mob chase ceiling for the current real-client slice.
- G8 multi-creature combat tests: `cargo fmt` passed with the known
  canonicalize warning; `cargo test -p wow-network
  db_creature_combat_can_track_multiple_attackers --lib` passed; `cargo test
  -p wow-network db_creature --lib` passed with 32 targeted tests; `cargo check
  -p wow-network -p starter-zone-flow-test` passed; `.\scripts\test-rust.cmd`
  passed with 134 `wow-network` tests after stopping the running local
  auth/world processes to avoid executable locks.
- G8/G9 CMaNGOS-like movement/social follow-up: Rust now loads DB
  `creature.spawndist`, `creature.MovementType`,
  `creature_template.MovementType`, and `creature_template.CallForHelp`.
  Non-combat random-movement creatures start CMaNGOS-shaped walk splines inside
  their spawn radius after 3-10 second pauses, return-home completion restores
  idle/random scheduling, and creature aggro can call nearby same-faction
  eligible hostile assists once using `CallForHelp` or the CMaNGOS default
  5-yard assistance radius. This is still not waypoint/pathfinder parity.
- G8/G9 CMaNGOS-like movement/social tests: `cargo fmt` passed with the known
  canonicalize warning; `cargo test -p wow-network db_creature_random --lib`
  passed with 2 targeted tests; `cargo test -p wow-network
  db_creature_assistance --lib` passed; `cargo test -p wow-network
  db_creature --lib` passed with 35 targeted tests; `cargo check -p
  wow-network -p starter-zone-flow-test` passed; first `.\scripts\test-rust.cmd`
  failed on a clippy simplification in the new random-movement guard, and the
  rerun passed with 137 `wow-network` tests after applying the clippy fix.
- G8 faction-reaction follow-up: auto-aggro no longer uses the temporary
  starter entry allowlist. Rust now routes creature/player sight aggro through
  a CMaNGOS-shaped faction-template reaction bridge using local ClassicDB
  Northshire faction ids: friendly Northshire NPC factions remain friendly,
  Young Wolf faction `32` remains neutral, Defias `17` and Kobold `25` are
  hostile, unknown faction templates default neutral, and the existing creature
  sanity gates still block civilians, critters, vendors/trainers, lootable
  corpses, wrong-map creatures, and evading-home creatures. This is still a
  narrow bridge until Rust has a full `FactionTemplate.dbc` loader, but the
  live aggro decision is no longer keyed on creature entry ids.
- G8 faction-reaction tests: `cargo fmt` passed with the known canonicalize
  warning; `cargo test -p wow-network db_creature_aggro --lib` passed with 7
  targeted tests; `cargo test -p wow-network db_creature --lib` passed with 36
  targeted tests; `cargo check -p wow-network -p starter-zone-flow-test`
  passed; the first `.\scripts\test-rust.cmd` hit clippy's `manual_contains`
  lint in the new faction helper, and the rerun passed with 138
  `wow-network` tests after applying the cleanup.
- G8/G9 mmap groundwork: generated Eastern Kingdoms map 0 mmaps in
  `C:\World of Warcraft Classic\mmaps`, pointed `config/worldserver.local.toml`
  at `C:/World of Warcraft Classic`, and wired Rust world startup to inspect
  configured `maps`, `vmaps`, and `mmaps`. DB-creature navigation now keeps a
  startup-scanned in-memory set of mmap headers/tiles and gates aggro/chase/
  melee path availability on CMaNGOS-style tile presence instead of the old
  unconditional path-success placeholder. If no mmap tiles are configured it
  still falls back permissively for non-pathing test environments. This is
  tile availability validation, not full Recast/Detour path solving yet.
- G8/G9 mmap tests: `cargo fmt` passed with the known canonicalize warning;
  `cargo test -p wow-network db_creature_navigation --lib` passed with 3
  targeted tests; `cargo test -p wow-network mmap_tile --lib` passed with 2
  targeted tests; `cargo test -p wow-network db_creature --lib` passed with 37
  targeted tests; `cargo check -p wow-network -p worldserver -p
  starter-zone-flow-test` passed; first `.\scripts\test-rust.cmd` hit clippy's
  `excessive_precision` lint for the CMaNGOS grid-size constant, and the rerun
  passed with 140 `wow-network` tests after applying the cleanup. Restarted
  the real-client stack; `world-client-18085.log` reports
  `maps=true`, `vmaps=true`, `mmap_maps=1`, and `mmap_tiles=513`.
- G8/G9 native mmap path generation follow-up: added a small C++ Detour bridge
  in `crates/wow-network/native/mmap_path.cpp`, compiled from
  `crates/wow-network/build.rs` against the bundled RecastNavigation Detour
  sources with `DT_POLYREF64`, matching the generated mmap data's poly-ref
  expectations. DB-creature chase destination generation now asks Detour for a
  real mmap-backed next path corner when configured `mmaps` contain the start
  and target tiles, and falls back to the existing straight-line destination
  only when no native path is available.
- G8/G9 native mmap tests: `cargo test -p wow-network db_creature_mmap --lib
  -- --nocapture` passed against local `C:/World of Warcraft Classic/mmaps`;
  `cargo fmt` passed; `cargo test -p wow-network db_creature_navigation --lib`
  passed with 3 targeted tests; `cargo test -p wow-network db_creature --lib`
  passed with 38 targeted tests; `cargo check -p wow-network -p worldserver -p
  starter-zone-flow-test` passed; `.\scripts\test-rust.cmd` passed with 141
  `wow-network` tests after stopping local auth/world processes to avoid
  executable locks. Restarted the real-client stack; `world-client-18085.log`
  reports `maps=true`, `vmaps=true`, `mmap_maps=1`, and `mmap_tiles=513`.
- G8/G9 multi-point path-following follow-up: `SMSG_MONSTER_MOVE` generation
  now serializes multi-point paths instead of hardcoding a single destination,
  and DB creature `Random`, `Chase`, and `ReturnHome` motion states store the
  same path the client receives. Server-side runtime position now interpolates
  across those path corners, chase trims Detour paths to the CMaNGOS-style
  melee stop distance, return-home can reuse the native mmap path generator,
  and random movement asks the same path layer to reach its DB-backed
  `spawndist` destination when local mmaps cover the area.
- G8/G9 multi-point path tests: `cargo fmt` passed; `cargo test -p
  wow-network db_creature --lib` passed with 40 targeted tests; `cargo test -p
  wow-network monster_move --lib` passed with 2 targeted tests; `cargo test -p
  wow-network db_creature_mmap --lib -- --nocapture` passed against the local
  generated mmap data; `cargo check -p wow-network -p worldserver -p
  starter-zone-flow-test` passed; `.\scripts\test-rust.cmd` passed with 144
  `wow-network` tests after stopping local auth/world processes to avoid
  executable locks. Restarted the real-client stack; `world-client-18085.log`
  reports `maps=true`, `vmaps=true`, `mmap_maps=1`, and `mmap_tiles=513`.
- G9 DB waypoint/patrol movement v1: Rust now loads waypoint paths for
  `MovementType` 2/4 creatures from `creature_movement` by creature guid, then
  falls back to `creature_movement_template` by entry/path 0 like CMaNGOS'
  default path lookup. Non-combat waypoint creatures send timed multi-point
  `SMSG_MONSTER_MOVE` walk splines, interpolate server-side along the same
  path, wait at DB nodes, loop normal waypoint paths, and reverse at ends for
  linear waypoint movement type 4. `waypoint_path` / spawn-group indirection is
  intentionally not guessed and is tracked as #51.
- G9 waypoint tests in this slice: `.\scripts\test-rust.cmd` baseline passed
  before edits with 144 `wow-network` tests; `cargo test -p wow-network
  db_creature_waypoint --lib` passed; `cargo test -p wow-network
  db_creature_linear_waypoint --lib` passed; `cargo fmt` passed; `cargo test
  -p wow-network db_creature --lib` passed with 42 targeted tests; `cargo
  check -p wow-db -p wow-network -p worldserver -p starter-zone-flow-test`
  passed; `cargo test -p wow-network monster_move --lib` passed with 2 tests;
  final `.\scripts\test-rust.cmd` passed with 146 `wow-network` tests.
- Fixed the real-client report that some moving mobs appeared to fly away after
  multi-point pathing. Rust's `SMSG_MONSTER_MOVE` linear path writer now matches
  CMaNGOS' Vanilla layout: point count, raw final destination, then packed
  quarter-yard XYZ offsets from the destination for intermediate points. The
  packet also carries the CMaNGOS fake `Runmode` flag for monster move splines.
  This was a P1 protocol/visual fix for the current movement slice.
- Fixed the real-client report that Kobold Vermin were still hostile. Local
  Vanilla `FactionTemplate.dbc` and RealClassicDb rows show faction template
  `25` has no player enemy mask, so Rust now treats Kobold Vermin like neutral
  Young Wolves for auto-aggro while preserving Defias faction `17` as hostile.
  The starter-zone harness now proves walking near Vermin does not start
  combat, then moves into melee range and attacks them explicitly for Kobold
  Camp Cleanup.
- Monster-move encoding tests in this slice: `cargo fmt` passed; `cargo test
  -p wow-network monster_move --lib` passed; `cargo test -p wow-network
  db_creature --lib` passed with 42 targeted tests; `cargo check -p wow-network
  -p worldserver -p starter-zone-flow-test` passed; first `.\scripts\test-rust.cmd`
  run hit the known Windows executable lock because local `authserver` and
  `worldserver` were still running, then passed after stopping those processes.
- Kobold-neutral faction tests in this slice: `cargo fmt` passed; `cargo test
  -p wow-network db_creature_aggro --lib` passed with 8 targeted tests; `cargo
  test -p wow-network db_creature --lib` passed with 43 targeted tests; `cargo
  check -p wow-network -p worldserver -p starter-zone-flow-test` passed;
  `.\scripts\test-rust.cmd` passed after stopping local auth/world processes
  that held Windows executable locks; `.\scripts\test-starter-zone-flow.cmd`
  passed against the Docker-backed RealClassicDb harness after elevated Docker
  access was allowed.
- G9 creature corpse/respawn tests in this slice: `cargo fmt` passed with the
  known canonicalize warning; `cargo test -p wow-network db_creature --lib`
  passed with 45 targeted tests; `cargo check -p wow-db -p wow-network -p
  worldserver -p starter-zone-flow-test` passed; elevated
  `.\scripts\test-starter-zone-flow.cmd` passed against RealClassicDb after the
  harness moved from same-creature instant respawn to ten distinct Kobold Vermin
  kills; final elevated `.\scripts\test-rust.cmd` passed.
- Fixed the repeatable real-client FPS drop that appeared 30-60 seconds after
  login once DB idle movement was enabled. Rust now advances already-moving DB
  creature splines every tick but paces new idle random/waypoint starts to
  `DB_CREATURE_IDLE_MOTION_STARTS_PER_TICK`, avoiding large login-area
  `SMSG_MONSTER_MOVE` bursts when many Northshire creatures become due at once.
  The starter-zone harness attack sweep was widened so it remains valid while
  real DB creatures wander.
- Idle-movement pacing tests in this slice: `cargo fmt` passed; `cargo test -p
  wow-network db_creature_idle_motion_start_guids_are_paced_per_tick --lib`
  passed; `cargo test -p wow-network db_creature --lib` passed with 46 targeted
  tests; `cargo check -p wow-db -p wow-network -p worldserver -p
  starter-zone-flow-test` passed; elevated `.\scripts\test-starter-zone-flow.cmd`
  passed against RealClassicDb; final elevated `.\scripts\test-rust.cmd`
  passed.
- Started the multiplayer-ready creature respawn persistence turn before player
  death/respawn. Rust now writes DB creature deaths to CMaNGOS'
  `characters.creature_respawn` table for instance `0`, clears the row when the
  runtime respawn happens, cleans expired rows while loading nearby creatures,
  and restores future-dead creatures as tracked runtime state without sending
  create blocks to the client. This keeps relog/restart from resurrecting killed
  mobs early and gives future multi-client work a shared DB-backed creature
  truth to build on.
- Creature-respawn persistence tests in this slice: `cargo fmt` passed; `cargo
  test -p wow-network movement_visibility --lib` passed with 5 targeted tests;
  `cargo test -p wow-network db_creature --lib` passed with 46 targeted tests;
  `cargo check -p wow-db -p wow-network -p worldserver -p starter-zone-flow-test`
  passed; `.\scripts\test-rust.cmd` passed with 151 `wow-network` tests. One
  elevated RealClassicDb starter-zone run passed before the final wolf moving
  target harness/assertion tweak; the final elevated rerun was blocked by the
  Codex app usage-limit approval gate, not by a Rust/server failure.
- Fixed the fresh-login real-client FPS drop follow-up after the user reproduced
  it on a clean server without killing mobs. CMaNGOS'
  `src/game/Movement/packet_builder.cpp::WriteLinearPath` skips packed
  intermediate monster-move offsets whose squared distance to the destination is
  less than `0.5f` because tiny offsets can freeze the client. Rust
  `SMSG_MONSTER_MOVE` path serialization now applies the same skip/count logic,
  and per-client movement packet logs were demoted from `INFO` to `DEBUG` to
  avoid host-side log pressure while testing.
- FPS follow-up tests in this slice: `cargo fmt` passed with the known
  canonicalize warning; `cargo test -p wow-network monster_move_path --lib`
  passed with 2 targeted tests; `cargo check -p wow-network -p worldserver`
  passed; `git diff --check` passed with only CRLF warnings. The local client
  stack was restarted on `127.0.0.1:18085` for real-client verification.
- Fixed the real-client corpse visibility gap found after the FPS fix. When a
  corpse leaves visibility before respawn, Rust now destroys only the client
  object, keeps the corpse runtime hidden server-side, and recreates the corpse
  with zero health, lootable dynamic flags, and no NPC flags if the player
  returns before the DB respawn timer. Dead future-respawn creatures still stay
  hidden until their `creature_respawn` row is due.
- Corpse visibility follow-up tests in this slice: `cargo fmt` passed with the
  known canonicalize warning; `cargo test -p wow-network
  movement_visibility_recreates_unloaded_corpse_before_respawn --lib` passed;
  `cargo test -p wow-network movement_visibility --lib` passed with 7 targeted
  tests; `cargo check -p wow-network -p worldserver` passed. The local client
  stack was restarted on `127.0.0.1:18085` for real-client verification.
- Started G7 Player Death + Respawn v1 from the CMaNGOS references:
  `Unit::DealDamage`, `Player::KillPlayer`, `BuildPlayerRepop`,
  `RepopAtGraveyard`, `HandleRepopRequestOpcode`, and
  `HandleReclaimCorpseOpcode`.
- Lethal DB-creature melee can now reduce player health to zero, mark an
  in-session corpse state, clear combat, publish a player update with health
  `0` plus the CMaNGOS release-timer byte, and persist the current health/flags
  state to `characters`.
- Added `CMSG_REPOP_REQUEST` handling. Releasing spirit sets
  `PLAYER_FLAGS_GHOST`, sets health to `1`, clears combat state, looks up the
  nearest DB-backed Alliance graveyard from `game_graveyard_zone` /
  `world_safe_locs`, falls back to a much closer spirit healer when local
  ClassicDB graveyard links point far away from the corpse, sends a same-map
  teleport movement packet, sends `SMSG_CORPSE_RECLAIM_DELAY`, force-rescans
  nearby DB creature visibility, and persists ghost position/flags.
- Added `CMSG_RECLAIM_CORPSE` handling for the first corpse-reclaim path. A
  ghost near the stored corpse position resurrects at 50% max health, clears
  the ghost flag, teleports/places back at the corpse position, and persists
  the alive state. `CMSG_RECLAIM_CORPSE` now ignores the client-sent corpse
  GUID value like CMaNGOS instead of incorrectly requiring it to equal the
  player GUID.
- Added the real-client recovery follow-up for G7 after the user reported
  permanent ghost state: `MSG_CORPSE_QUERY` now points ghosts back to the stored
  corpse position, `CMSG_SPIRIT_HEALER_ACTIVATE` resurrects ghosts at a nearby
  loaded spirit healer, and the shared resurrection path clears ghost flags,
  restores 50% health, and persists the alive state.
- Added CMaNGOS-shaped player corpse world objects for G7. Releasing spirit now
  creates a `TYPEID_CORPSE` / `HighGuid::Corpse` object, saves the resurrectable
  corpse to `characters.corpse`, fills owner, position, display, equipment,
  bytes, guild, and flags from the character/session visual state, streams
  nearby player corpses on login and movement, answers corpse query/reclaim from
  that persistent corpse row, and deletes the row when resurrection succeeds.
  The shared runtime corpse map also carries post-resurrection `CORPSE_BONES`
  objects so bones can remain visible to nearby sessions during the same world
  lifetime without incorrectly making bones permanent DB state.
- Added `wow_db::get_closest_graveyard` and
  `wow_db::get_closest_spirit_healer` and
  `wow_db::update_character_death_state` helpers, player corpse DB helpers, plus
  packet tests for lethal creature damage, death/update fields, corpse query
  body shape, spirit healer detection, player corpse create blocks, bones flag
  updates, and same-map teleport shape.
- Fixed a starter-zone harness expectation found by the Docker-backed run: in
  RealClassicDb mode the wolf smoke only damages a Young Wolf, so the
  `creature_respawn` assertion now expects the ten killed Kobold Vermin rows
  instead of incorrectly requiring an un-killed wolf respawn row.
- G7 tests in this slice: `cargo fmt` passed with the known canonicalize
  warning; `cargo check -p wow-db -p wow-network -p worldserver -p
  starter-zone-flow-test` passed; `cargo test -p wow-network --lib` passed with
  163 tests after the corpse/bones follow-up; `.\scripts\test-rust.cmd` passed;
  elevated `.\scripts\test-starter-zone-flow.cmd` passed against RealClassicDb
  after fixing the corpse visual query's guild join.
- Added the G7 death flow to `starter-zone-flow-test`. The harness now seeds a
  private hostile death-proof creature, lets creature-origin aggro damage kill
  the Human Warrior, releases spirit, verifies the ghost update, persisted
  corpse object, reclaim delay, release teleport, corpse-query arrow data,
  corpse reclaim, bones conversion, final teleport, corpse row deletion,
  restored health, cleared ghost flags, and persisted corpse-position
  resurrection.
- Final G7 flow tests: `cargo fmt` passed with the known canonicalize warning;
  `cargo check -p starter-zone-flow-test` passed; elevated
  `.\scripts\test-starter-zone-flow.cmd` passed against RealClassicDb; final
  `.\scripts\test-rust.cmd` passed with 163 `wow-network` tests.

## P0/P1 Fixes In This Slice

- Fixed the Quest v1 automated harness timing blocker. A one-second packet-drain
  loop made the real ClassicDB ten-kill quest proof take minutes. The harness now
  drives repeated client swings and drains immediately available packets with a
  short timeout, keeping the proof fast without changing production combat.
- Fixed clippy/test regressions caused by adding quest-log state to the player
  update serializer.
- Fixed the real-client McBride click blocker by routing DB gossip hello to the
  Quest v1 quest-list response when the creature has visible start quests.
- Fixed the real-client quest progress display blocker where progress updates
  could target slot 0 even when Kobold Camp Cleanup lived in a later quest-log
  slot because other Northshire quests were accepted.
- Fixed the DB-creature spell-kill credit blocker: melee deaths granted quest
  credit, but starter spell deaths did not.
- Fixed the completed Quest v1 turn-in blocker: McBride's gossip click returned
  the quest list even when the accepted quest was complete; it now returns the
  reward offer first.
- Completed the P4 sustainability split before XP/combat and added the shared
  DB-creature death finalizer. This was architecture cleanup, not a gameplay
  parity fix.
- Completed the P4 DB character split and lifecycle transaction cleanup from
  #5. This improves maintainability and atomicity for create/hard-delete
  without changing gameplay hot paths.
- Fixed a P1 manual-test data-loss guardrail: `run-client-stack-18085.ps1` no
  longer deletes all `RUSTAUTH` characters on every restart unless
  `-ResetCharacters` is explicitly passed.
- Fixed XP/level-up v1 harness blockers where `starter-zone-flow-test` could
  miss `SMSG_LOG_XPGAIN` or `SMSG_LEVELUP_INFO` by stopping at quest kill
  credit or by consuming progression packets inside generic `read_until` waits.
  The harness now records progression evidence across combat, loot release,
  quest status, offer, and reward waits.
- Fixed the fresh-login FPS blocker by matching CMaNGOS' monster-move packed
  offset guard and reducing movement opcode logging to debug-level diagnostics.
- Fixed a Trainer v1 data-reference blocker: the previous fixture-oriented
  assumption that Brother Paxton was the Warrior trainer was wrong for
  ClassicDB; the Rust trainer proof now targets Llane Beshere (`911`).
- Fixed a Trainer v1 DB compatibility blocker where `TrainerType` and
  `TrainerClass` tinyint signedness differs across the local schema shape.
- Fixed the real-client trainer learning blocker: Rust was charging for and
  persisting the trainer-cast row (`6674`) instead of the DBC-triggered learned
  spell (`6673`), so the client spent copper but saw no new spellbook ability.
  The buy path now checks/charges/persists the learned spell while preserving
  the CMaNGOS packet shape for the requested trainer-row spell.
- Fixed the live trainer notification blocker: after persistence was corrected,
  the client still only showed Battle Shout after relog because Rust skipped
  CMaNGOS' in-world `SMSG_LEARNED_SPELL` packet. Trainer buy now sends that
  packet immediately after buy success.
- Fixed the G3 harness packet-drain blocker: movement streaming can legitimately
  send extra creature create chunks, so the starter-zone harness now drains
  immediate movement-stream packets before assertions that expect combat values
  updates.
- Fixed the G3 real-client visibility blocker where creatures streamed in but
  never disappeared. Rust now destroys DB creature objects that leave the
  movement visibility query.
- Fixed a G8 parity blocker from real-client observation: Young Wolves are
  neutral and must not auto-aggro. The temporary starter auto-aggro allowlist
  now includes Kobold Vermin and Defias Thug, and the harness proves kobold
  aggro after movement-streaming the kobold into visibility.
- No new P0/P1 bugs were discovered during the G8 combat-state follow-up.
- No new P0/P1 bugs were discovered during the G8 CMaNGOS-parity pivot; the
  synthetic chase shortcut was removed before it became active behavior.
- Fixed a G8 harness blocker in the chase proof: standing ten yards past the
  selected kobold could let another nearby hostile win nearest-target aggro.
  The proof now stands just outside melee range so the expected RealClassicDb
  Kobold Vermin deterministically owns the aggro/chase/damage sequence.
- Fixed the G8 chase blocker reported from real-client smoke: initial
  `SMSG_MONSTER_MOVE` chased only the target's position at aggro time. The
  active chase state now re-paths from the creature's current interpolated
  position toward the player's current position on a 250ms recheck cadence.
- Fixed the G8 combat-retention blocker reported from real-client smoke:
  kiting outside the spawn-driven visibility query could remove the active
  creature runtime and clear combat. Active combat creatures are now retained by
  visibility cleanup even when their DB spawn point is no longer in the nearby
  query.
- Fixed the G8 chase tick-starvation blocker reported from real-client smoke:
  continuous client movement packets could prevent the timeout-driven world
  tick from firing, so chase updates appeared to stop after running for a while.
  Combat/chase ticks now run whenever the world tick deadline is due, including
  immediately after handling a packet.
- No new P0/P1 bugs were discovered during the G8 navigation guardrail slice.
- No new P0/P1 bugs were discovered during the G8 leash/evade/return-home
  slice.
- Fixed a P1 return-home state blocker reported from real-client smoke:
  returning-home creatures could reacquire and chase again before reaching home.
  They now remain non-aggroable/non-damageable while in `ReturnHome`.
- Fixed a P1 return-home motion blocker reported from real-client smoke:
  returning creatures could sometimes appear to stop because non-combat
  return-home motion was not advanced after combat cleared. Return-home motion
  now ticks independently of active combat.
- Fixed a G8 parity blocker behind the slow-aggro feel: Rust was using a
  hardcoded aggro base instead of the creature template `Detection` field that
  CMaNGOS feeds into `GetAttackDistance`. DB-backed detection now drives
  aggro radius.
- No new P0/P1 bugs were discovered during the G8 range/facing player melee
  slice.
- Fixed the P1 follow-up from real-client smoke where far player attack intent
  could immediately start the creature's retaliation state before melee
  validity was satisfied.
- Fixed the P1 follow-up from real-client smoke where far Heroic Strike could
  look castable even though no damage landed. Starter melee spells now fail
  before power spend/spell-go/damage when melee validity fails.
- No new P0/P1 bugs were discovered during the G8 chase stop/repath parity
  slice.
- Fixed the P1 real-client combat-status blocker where creature aggro did not
  put the player into the client's combat state until the player manually
  right-clicked. The client-facing in-combat flag is now sent on creature aggro
  and cleared on evade/death.
- Fixed a P1 visual-facing parity blocker where chase splines did not include
  the CMaNGOS `MonsterMoveFacingTarget` payload, so mobs could arrive in range
  without visibly turning toward the player.
- Fixed the follow-up P1 visual-facing blocker where mobs already inside melee
  range could silently rotate server-side and hit before the client saw an
  in-place turn packet.
- Fixed the P1 real-client combat agency blocker where only one DB creature
  could own combat/chase state at a time. Multiple nearby hostile creatures can
  now enter creature combat and tick their chase/attack state independently.
- No new P0/P1 bugs were discovered during the G8/G9 random movement and
  assistance slice. The remaining pathfinder/waypoint/threat gaps are already
  tracked as broader G8/G9 parity work.
- Fixed the native mmap bridge blocker discovered during implementation:
  Detour rejected the local generated `000.mmap` header until the Rust native
  bridge compiled bundled Detour with `DT_POLYREF64`, matching the extracted
  mmap data's large `maxPolys` shape.
- Fixed a P1 memory-safety guardrail in the mmap bridge: Rust gameplay/tests no
  longer perform direct unsafe FFI calls, and the C++ bridge now validates FFI
  inputs, bounds tile-data allocation, uses RAII cleanup, and returns errors
  instead of throwing across the Rust boundary.
- Fixed a P1 shared-combat guardrail during the G12 tick timing slice:
  `MapRuntime` now clears all active DB-creature combat claims for a player
  victim when that player is no longer alive, preventing stale shared attackers
  from remaining reserved after death.
- No new P0/P1 bugs were discovered during the G8/G9 multi-point path-following
  slice.
- No new P0/P1 bugs were discovered during the G9 DB waypoint/patrol movement
  slice.
- Fixed a P1 real-client monster-move visual/protocol bug where multi-point
  paths were serialized as raw XYZ points instead of CMaNGOS' destination plus
  packed intermediate offsets, which could make mobs appear to fly away.
- Fixed a P1 real-client faction-reaction bug where Kobold Vermin faction
  template `25` was incorrectly treated as hostile in Rust's narrow
  CMaNGOS-shaped faction bridge. Vermin no longer auto-aggro Alliance players,
  but remain attackable for the quest flow.
- Fixed the P1 creature-lifecycle parity blocker where loot release respawned a
  DB creature immediately. Loot release now only clears looting/lootable state
  and optionally shortens corpse decay; corpse removal and respawn happen from
  CMaNGOS-like runtime timers.
- Fixed a P1 real-client movement throughput regression where all idle DB
  creatures with due random/waypoint timers could start movement in one world
  tick, producing a burst of `SMSG_MONSTER_MOVE` packets after login. New
  idle-motion starts are now paced per tick while existing spline advancement
  remains uncapped.
- Fixed the P1 mob-death persistence gap for the current single-process world:
  DB creature deaths now persist future respawn time in `creature_respawn`, and
  login/movement visibility suppresses creatures with future persisted respawn
  rows instead of recreating them alive.
- Fixed a P1 G7/golden-path harness mismatch discovered during the
  Docker-backed starter-zone run: RealClassicDb mode does not kill the initial
  Young Wolf smoke target, so the persisted creature-respawn assertion now
  counts the ten actually killed Kobold Vermin rows and leaves the wolf out of
  the expected-death set.
- Fixed the P1 G7 real-client recovery blocker reported after the first death
  slice: release could leave the player as a ghost at an unhelpful graveyard
  with no visible healer, no corpse arrow response, and no successful reclaim.
  Rust now prefers a nearby spirit healer over a far linked graveyard when the
  local graveyard links are incomplete, streams graveyard creatures after
  release, answers `MSG_CORPSE_QUERY`, ignores the corpse GUID mismatch on
  reclaim like CMaNGOS, and handles spirit healer activation.
- Fixed the immediate follow-up P1 disconnect/lockout on release: local
  ClassicDB `world_safe_locs` and spirit-healer coordinates are decoded with
  explicit numeric casts, avoiding the MySQL DECIMAL-to-`f32` decode error in
  `CMSG_REPOP_REQUEST`. World sessions now also persist best-effort state and
  unregister the active character on handler errors, so a disconnect does not
  leave the character stuck as "already loaded" until a server restart.
- Fixed the next P1 ghost-state gaps from real-client smoke: release now sends
  the CMaNGOS ghost spell `8326` in the visible aura update fields and login
  bootstrap preserves that aura while `PLAYER_FLAGS_GHOST` is set; resurrect
  updates clear those aura fields. DB creature aggro, assistance, and combat
  start now refuse non-alive players, so ghosts should not pull mobs. Spirit
  healer create blocks force `UNIT_NPC_FLAG_SPIRITHEALER`, and spirit healer
  `CMSG_GOSSIP_HELLO` / gossip option selection now route to the shared 50%
  health resurrection path.
- Fixed the P1 corpse visual query bug caught by the Docker-backed
  starter-zone harness: the new corpse loader initially selected
  `characters.guildid`, which does not exist in the local character schema.
  Corpse visual loading now left-joins `guild_member.guildid` like the rest of
  the character visual path, and the RealClassicDb starter-zone proof passes.
- Fixed a P1 G7 harness cleanup/flow blocker while adding death to the golden
  path: stale `creature_respawn` rows could suppress the private death-proof
  creature from runtime visibility, and the first harness shape let the player
  kill that creature instead of proving creature-origin lethal damage. The
  harness now explicitly clears the death fixture respawn row, places the
  fixture in a deterministic visibility area, gives it enough health to survive
  incidental combat, and waits for hostile creature aggro to kill the player.

## Non-blocking Backlog

GitHub issues remain the source of truth. No new non-blocking P2/P3/P4 issues
were discovered during the Quest v1 slice. The follow-up sustainability audit
logged #48 as P4 world architecture debt: split gameplay handlers before
XP/combat v2. #48 is now completed/closed. #5 is now completed/closed for the
`wow-db/src/character.rs` split and character lifecycle transaction debt.
User real-client smoke noted that aggro response can still feel slow compared
with CMaNGOS after the return-home fixes; this was added as fresh evidence on
GitHub #12 for future G8 cadence/AI-notify parity work. The missing
`waypoint_path` / spawn-group indirection fallback for DB patrol movement is
tracked as GitHub #51. The first waypoint loader's per-creature DB fallback
queries are tracked as P4 performance debt in GitHub #52. The fallback
starter-zone fixture duplicate GUID/counter issue discovered while restoring
the real ClassicDB test data is tracked as GitHub #54; the real ClassicDB
starter-zone proof was green after running the repo import script. The
repeatable starter-zone death proof failure observed during the G12 Phase 4
verification pass is tracked as GitHub #55.

Known open directions still include final player death/respawn proof and polish
(#44), broader
DB-backed gossip/trainer/vendor parity, exact combat/stat formulas, map
exploration discovery/persistence, broader quest types beyond a single
kill-count objective, and XP/trainer follow-ups outside the starter solo path
such as rested XP, group XP, pet XP, max-level money conversion, talent points,
skill-cap updates, spell-chain/DBC class filtering, profession trainer rules,
and passive/aura effects from learned spells.

## Known Blockers And Gaps

- Quest v1 only covers one required-creature kill-count quest with no item
  requirements and no reward item selection.
- XP/level-up v1 covers solo starter creature kills and quest reward XP only.
  Rested XP, group XP, pet XP, max-level money conversion, talent points, and
  skill-cap updates are intentionally deferred.
- Trainer v1 covers one DB-backed Warrior trainer spell in the Northshire path
  and now maps trainer-cast rows to learned spells through `spell_template`.
  It does not yet implement full DBC spell-chain filtering, profession limits,
  trainer templates, spell visual casts, passive/aura side effects, talent
  spells, or action-bar auto-placement.
- Quest-log serialization now uses deterministic active-quest slotting for this
  narrow path; broader abandon/share/fail timers remain future work.
- DB creature combat is still a starter-slice model. It is good enough to prove
  kill credit, loot release, CMaNGOS-like runtime corpse timing, and distinct
  creature kills in the harness, but not full CMaNGOS combat pacing, threat, or
  persistent creature respawn state across worldserver restart.
- G8 aggro is harness-proven for hostile DB creatures. It now has first-slice
  Recast/Detour mmap path generation for configured local `mmaps`, multi-point
  monster-move splines, and matching server-side interpolation for chase,
  return-home, and random movement paths. It still does not implement vmap LOS,
  full CMaNGOS `PathFinder` smooth-path flags, full
  DBC-backed faction-template loading beyond the narrow Northshire
  hostile/friendly bridge, or final G8 real-client proof.
  Creature attacks now carry explicit attacker/victim/timer state, chase through
  a timed runtime motion state with 250ms re-pathing and active-combat
  visibility retention, and require melee reach; player DB-creature swings and
  supported starter melee spell fixtures now require server-side range, facing,
  and the explicit navigation guardrail before damage lands. Combat rolls,
  exact combat reach/model modifiers, swing-error packets, damage formulas,
  CMaNGOS path flags, `waypoint_path` indirection, and vmap LOS remain future
  G8/G9 work. User real-client smoke confirmed terrain clipping/glitchy pathing before
  the native mmap bridge; evidence and the mmap follow-ups were appended to
  GitHub #12 with `gate:G8-combat-agency` / `cmangos-diff`.
- G3 movement-triggered DB creature streaming is harness-proven and
  user-verified in the real client; keep it as a regression gate.
- G7 corpse/bones objects and the core die/release/reclaim path are
  unit/harness-proven, and user real-client smoke confirmed the flow works.
  Durability loss, resurrection sickness, corpse/bones expiry timers,
  relog-dead/relog-ghost edge checks, and cross-worldserver persistence are
  still future parity.
- The repo still relies on local `target/classic-db` / Docker content import for
  full ClassicDB Northshire data.

## Next Recommended Task

User-directed next slice: **G12 shared DB creature event/broadcast authority or
lazy creature grid loading, depending on what the user wants to derisk first**.

Phases 1-4 plus nearby `/say` and the first DB-creature shared-state slice are
done: the outbound channel/session registry exists, live players register into
shared map state with login/logout create/destroy visibility, movement updates
`MapRuntime` with nearby-player movement broadcast plus range create/destroy
visibility diffs, player-player visibility now uses CMaNGOS-shaped grid/cell
primitives instead of full player scans, local `/say` broadcasts to nearby
players through shared map visibility, and DB-creature snapshots,
player-caused health/death updates, and DB-creature loot claims are shared
through `MapRuntime`; DB-creature combat ownership claims are also shared so one
creature cannot start separate private combat loops for different sessions.
The tick loop now pulls active creature combats from `MapRuntime` and stores
next-swing/retry timing back into the shared map, including starter-spell
retaliation and victim-wide cleanup on death. Combat start, creature
in-combat-flag updates, chase, facing turns, evade, and return-home packets now
broadcast to nearby observers through `MapRuntime`. The next multiplayer slice
should move creature-origin damage and lifecycle updates behind `MapRuntime`
events with observer broadcasts, or replace movement-time creature DB radius
queries with lazy grid-loaded creature visibility.

Near-term done definition:

- two clients can log into Northshire at once;
- both clients see each other spawn;
- both clients see each other move;
- one client logging out destroys that player for the other;
- `/say` works between nearby players;
- both clients observe the same DB creature state;
- one client killing/looting a mob cannot duplicate or desync that mob for the
  other;
- existing G3 movement visibility and starter-zone flow tests remain green;
- creature visibility no longer depends on DB radius queries per movement
  heartbeat.

After this user-directed slice, return to the default playable-gate order:
G8 Combat Agency, G9 World Creature Fidelity, G10 NPC Interaction Fidelity, G11
Persistence + Relog Sanity, then G12 follow-up polish.

Keep it Human Warrior / Northshire only unless the user explicitly chooses a
broader slice.

## Key Files

- `crates/wow-network/src/world/mod.rs`
- `crates/wow-network/src/world/bootstrap.rs`
- `crates/wow-network/src/world/interactions.rs`
- `crates/wow-network/src/world/combat.rs`
- `crates/wow-network/src/world/death.rs`
- `crates/wow-network/build.rs`
- `crates/wow-network/native/mmap_path.cpp`
- `crates/wow-network/src/world/quests.rs`
- `crates/wow-network/src/world/trainers.rs`
- `crates/wow-network/src/world/opcodes.rs`
- `crates/wow-network/src/world/session.rs`
- `crates/wow-network/src/world/maps/runtime.rs`
- `crates/wow-network/src/world/tests.rs`
- `crates/wow-db/src/character.rs`
- `crates/wow-db/src/character/lifecycle.rs`
- `crates/wow-db/src/character/creation.rs`
- `crates/wow-db/src/character/inventory.rs`
- `crates/wow-db/src/character/starter.rs`
- `crates/wow-db/src/world_data.rs`
- `bins/starter-zone-flow-test/src/main.rs`
- `docs/rust_migration_plan.md`
- `docs/playable_gate_board.md`
- `docs/g12_shared_mapruntime_plan.md`
- `docs/rust_auth_foundation.md`
- `docs/checkpoint2_codebase_audit.md`
- `scripts/test-rust.cmd`
- `scripts/test-starter-zone-flow.cmd`
- `scripts/run-client-stack-18085.cmd`
- `scripts/run-client-stack-18085.ps1`
