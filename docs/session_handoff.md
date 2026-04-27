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

- Branch: `codex/rust-auth-foundation`
- Latest committed base before this slice: `7311b1986`
- Remote: `origin/codex/rust-auth-foundation`
- Worktree at handoff: contains the current G3/G8 playable-gate stack in Rust
  world/network, `starter-zone-flow-test`, and this handoff update.

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
client and is now a regression gate. Current active priority is G8 Combat
Agency, then G9 World Creature Fidelity, then G10 NPC Interaction Fidelity,
then G11 Persistence + Relog Sanity, then G5 Combat and Loot real-behavior
fidelity, then G6 Level + Trainer issue #49 polish, then G7 Death + Respawn,
then G12 Multi-client Sanity.

Important scope rule:
We are proving one vertical slice only. Fix P0/P1 bugs that block this slice.
Do not chase unrelated horizontal parity issues. For any non-blocking bug,
mismatch, missing subsystem, or cleanup gap you discover, create a GitHub issue
using the repo's bug triage policy, then continue the requested task.

## What Changed Recently

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

## Tests Run

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

## Non-blocking Backlog

GitHub issues remain the source of truth. No new non-blocking P2/P3/P4 issues
were discovered during the Quest v1 slice. The follow-up sustainability audit
logged #48 as P4 world architecture debt: split gameplay handlers before
XP/combat v2. #48 is now completed/closed. #5 is now completed/closed for the
`wow-db/src/character.rs` split and character lifecycle transaction debt.

Known open directions still include player death/respawn (#44), broader
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
  kill credit, loot release, and respawn in the harness, but not full CMaNGOS
  combat pacing or threat.
- G8 aggro is harness-proven for hostile DB creatures. It still does not
  implement real pathfinding/navmesh, LOS/path validity, leash/evade, social
  aggro, faction DB relationship lookup beyond the narrow Northshire
  hostile/friendly guardrails, player death, or final real-client proof.
  Creature attacks now carry explicit attacker/victim/timer state, chase through
  a timed runtime motion state with 250ms re-pathing and active-combat
  visibility retention, and require melee reach; player attacks and starter
  spell damage still use the older fixture-style range assumptions until the
  range/facing slice is widened. User real-client smoke confirmed terrain
  clipping/glitchy pathing remains; the evidence was appended to GitHub #12
  with `gate:G8-combat-agency` / `cmangos-diff`.
- G3 movement-triggered DB creature streaming is harness-proven and
  user-verified in the real client; keep it as a regression gate.
- The repo still relies on local `target/classic-db` / Docker content import for
  full ClassicDB Northshire data.

## Next Recommended Task

Continue Checkpoint 2 with the next narrow starter gameplay slice:

- G8 Combat Agency: build on the chase re-path foundation with path
  validity/LOS checks, then widen range/facing-gated player swings before
  leash/evade.
- G9 World Creature Fidelity: keep DB spawn/template/loot/respawn/patrol
  fidelity separate from combat agency, so aggro progress does not have to wait
  for full persistent creature behavior.
- G10 NPC Interaction Fidelity: audit Northshire quest givers, vendors,
  trainers, gossip NPCs, and non-interactive NPCs against CMaNGOS affordances.
- G11 Persistence + Relog Sanity: add relog checkpoints after each major
  Northshire action so state bugs cannot hide inside a single live session.
- After G3, tighten G5 corpse/respawn behavior so kill, loot, release, and
  respawn are closer to CMaNGOS instead of instant revive.
- G7 Player death/respawn (#44): starter death state, release spirit, graveyard
  teleport, resurrection, and persistence.
- G12 Multi-client Sanity: add minimal two-session visibility, chat, and shared
  creature-state proof before calling the slice MMO-shaped.

Keep it Human Warrior / Northshire only unless the user explicitly chooses a
broader slice.

## Key Files

- `crates/wow-network/src/world/mod.rs`
- `crates/wow-network/src/world/bootstrap.rs`
- `crates/wow-network/src/world/interactions.rs`
- `crates/wow-network/src/world/combat.rs`
- `crates/wow-network/src/world/quests.rs`
- `crates/wow-network/src/world/trainers.rs`
- `crates/wow-network/src/world/opcodes.rs`
- `crates/wow-network/src/world/session.rs`
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
- `docs/rust_auth_foundation.md`
- `docs/checkpoint2_codebase_audit.md`
- `scripts/test-rust.cmd`
- `scripts/test-starter-zone-flow.cmd`
- `scripts/run-client-stack-18085.cmd`
- `scripts/run-client-stack-18085.ps1`
