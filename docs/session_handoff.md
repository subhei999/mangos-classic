# Session Handoff

This file is the current operating brief for the next Rust migration session.
Durable roadmap belongs in `docs/rust_migration_plan.md`; auth-specific setup
belongs in `docs/rust_auth_foundation.md`.

## Handoff Rules

- Keep only current branch state, active goal, recent meaningful changes, exact
  tests run, local blockers, and the next recommended task.
- Do not append a full chronological log. Prune stale detail as it becomes
  durable roadmap history.
- For non-blocking P2/P3/P4 discoveries, use GitHub issues as the primary
  tracker. Only record a handoff fallback if GitHub logging fails.

## Current Branch

- Branch: `codex/rust-auth-foundation`
- Latest committed base before this slice: `820c138d8`
- Remote: `origin/codex/rust-auth-foundation`
- Worktree at handoff: contains Quest System v1 changes in Rust world/network,
  DB helpers, `starter-zone-flow-test`, and this handoff update.

## Current Goal

Checkpoint 2: **Starter Zone Playability**.

Northshire Valley / fresh Human Warrior remains the golden path. The current
trainer v1 slice proves one real ClassicDB training loop after XP/level-up:
accept Kobold Camp Cleanup, kill Kobold Vermin for creature XP and quest
progress, complete the quest, turn it in to Marshal McBride, grant reward money
and quest XP, level up, open the Warrior trainer Llane Beshere, learn one
available spell, charge money, and persist character progression, quest state,
money, and `character_spell`.

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
- Extended `starter-zone-flow-test` to request Llane Beshere's trainer list,
  buy `6674`, verify the live learned-spell packet names `6673`, and verify
  `6673` persisted active/enabled.

## Tests Run

- `git status --short --branch`
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
- The repo still relies on local `target/classic-db` / Docker content import for
  full ClassicDB Northshire data.

## Next Recommended Task

Continue Checkpoint 2 with the next narrow starter gameplay slice:

- Player death/respawn (#44): starter death state, release spirit, graveyard
  teleport, resurrection, and persistence.
- Or deepen the just-landed trainer slice only if real-client smoke shows a
  P0/P1 issue opening the trainer window or seeing the learned spell.

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
- `docs/rust_auth_foundation.md`
- `docs/checkpoint2_codebase_audit.md`
- `scripts/test-rust.cmd`
- `scripts/test-starter-zone-flow.cmd`
- `scripts/run-client-stack-18085.cmd`
- `scripts/run-client-stack-18085.ps1`
