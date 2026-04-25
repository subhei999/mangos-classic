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
- Latest committed base before this slice: `fe3a51cb8 Cover starter spell and item audits`
- Remote: `origin/codex/rust-auth-foundation`
- Worktree at handoff: contains uncommitted Checkpoint 1 grading/guardrail
  edits plus the candidate #13 Raptor Strike slice in
  `crates/wow-network/src/world/{mod.rs,bootstrap.rs,interactions.rs,tests.rs}`,
  `crates/wow-db/src/character.rs`, `docs/rust_migration_plan.md`,
  `docs/session_handoff.md`, and `scripts/run-client-stack-18085.ps1`.

## Current Goal

Checkpoint 1: **First Playable World**.

The Rust auth/world stack can authenticate a real WoW 1.12.1 client, manage
characters, enter a minimal world, move/logout/relog, seed starter state, render
DB-backed creature spawns, query creature templates, open fixture NPC
gossip/vendor flows, list a DB-backed vendor inventory, fight a fixture combat
dummy, and exercise basic inventory and loot/vendor item flows in the packet DB
harness.

Important scope rule:
We are proving one vertical slice only. Fix P0/P1 bugs that block this slice.
Do not chase unrelated horizontal parity issues. For any non-blocking bug,
mismatch, missing subsystem, or cleanup gap you discover, create a GitHub issue
using the repo's bug triage policy, then continue the requested task.

Checkpoint 1 now has a durable definition of done in
`docs/rust_migration_plan.md`: close it only after the required automated gate
passes and the real-client grading table has no unresolved `FAIL` rows. Future
"what is left" answers should grade against that table instead of inventing a
new open-ended checklist.

Current Checkpoint 1 grade: `PARTIAL`, roughly 65-70% complete. The checkpoint
is no longer blocked by the previous `Combat/spell` FAIL: active Night Elf
Hunter starter spell `2973` now works in the real client through the Rust
fixture path. The checkpoint still needs the remaining explicit grading gaps
closed or deferred before it is closeable.

## What Changed Recently

- Added a Checkpoint 1 definition of done and real-client grading pass to
  `docs/rust_migration_plan.md`, including required automated scripts, pass /
  partial / fail / deferred semantics, and closure rules for logged follow-up
  issues.
- Real-client grading found a character-list blocker caused by accumulated
  local `RUSTAUTH` smoke characters. `scripts/run-client-stack-18085.ps1` now
  resets the local grading account's character rows and common per-character
  state before seeding `Rustone`, so future manual passes start from one clean
  seeded character.
- Real-client grading found Night Elf female Hunter starter boots rendering as
  pants because archived missing item `129` was translated to pants item `147`.
  Rust now translates item `129` to source-backed `Trapper's Boots` `6127`,
  and `wow-db` tests assert Dwarf/Night Elf Hunter boot slots use `6127`.
- Real-client grading found the `Combat/spell` gate is still failing: Night
  Elf Hunter starter spell `2973` was active in `character_spell`, but Rust
  logged `Ignoring unsupported spell cast in starter spell fixture slice`. #13
  was updated with this evidence and should drive the next narrow spell/combat
  slice.
- Real-client grading found `/hello` has audio/text feedback but no physical
  wave animation. Logged as #43 P3 emote visual parity; do not block the
  current spell/combat slice on it.
- Candidate #13 fix added: Rust now loads active character spells into the
  world session on login, validates fixture spell casts against that active
  set, supports Hunter Raptor Strike rank 1 (`2973`) alongside Warrior Heroic
  Strike rank 1 (`78`), sends `SMSG_CAST_RESULT` / `SMSG_SPELL_GO`, applies
  fixture dummy damage, and updates mana (`UNIT_FIELD_POWER1`) or rage
  (`UNIT_FIELD_POWER2`) as appropriate.
- Real-client retest confirmed Raptor Strike works in the WoW 1.12.1 client.
  The old ignored-spell blocker for `2973` is cleared for Checkpoint 1.
- Inventory v1 supports backpack moves, equip/unequip, destroy, partial
  destroy, split, equipped-bag storage positions, bag-internal moves, and simple
  same-template stack merges, with DB persistence and packet-harness coverage.
- Fixture combat-dummy loot and `Rust Guide` vendor loops cover money, Tough
  Jerky `117`, and container item `2102` in `world-flow-test`.
- Started player `SMSG_UPDATE_OBJECT` parity by adding source-derived CMaNGOS
  default fields to the self-spawn create block: aura state, mount display,
  offhand/ranged damage placeholders, attack-power mods, power-cost modifiers,
  armor/resistances, secondary combat percentages, stat/resistance buff mods,
  profession points, rest XP, ammo/self-res/PvP placeholders, and
  `PLAYER_FIELD_BYTES2`.
- Added DB-backed starter skill serialization to the player self-spawn update:
  Rust now loads `character_skills` on enter-world and writes
  `PLAYER_SKILL_INFO_1_1` triplets with skill id, value/max, and zero bonus
  fields using the CMaNGOS packed two-`u16` layout.
- Added first-pass equipment-derived combat stats to the player self-spawn
  update: Rust now fills armor/resistances, base class attack power, weapon
  damage, shield block, and agility-derived dodge/crit fields instead of broad
  zero placeholders.
- Added DB-backed explored-zone serialization to the player self-spawn update:
  Rust now reads `characters.exploredZones` and writes all 64
  `PLAYER_EXPLORED_ZONES_*` fields into the update block.
- Added DB-backed creature spawn/query v1: Rust loads nearby `creature` rows
  joined to `creature_template`, appends unit create blocks during enter-world,
  and answers `CMSG_CREATURE_QUERY` from `creature_template` before falling
  back to the Rust Guide / combat dummy fixtures.
- Added DB-backed vendor-list v1: Rust reads `npc_vendor` rows joined to
  `item_template`, serializes CMaNGOS-shaped `SMSG_LIST_INVENTORY`, returns the
  vanilla no-inventory marker for empty DB vendors, and accepts supported DB
  vendor buys through the existing conservative item insertion path.
- Fixed the real-client `Rust DB Guide` interaction path: DB creature
  `CMSG_GOSSIP_HELLO` now returns a small vendor gossip menu when `npc_vendor`
  rows exist, and `CMSG_GOSSIP_SELECT_OPTION` opens the DB-backed vendor list.
  The DB gossip option id is zero-based to match the one-option client menu and
  avoid a WoW 5875 client crash on selection.
- Vendor money/sell v1 now charges `BuyPrice`, returns `SMSG_BUY_FAILED` when
  the player cannot afford a DB vendor item, updates `PLAYER_FIELD_COINAGE`
  after paid buys, and handles conservative sellback of owned sellable items by
  reducing/removing the stack and adding `SellPrice * count`.
- DB-backed vendor lists intentionally filter out container items for now after
  a WoW 5875 client crash was observed when shift-right-click buying the DB
  guide's Small Brown Pouch. Keep container purchases on the Rust Guide fixture
  path until DB container create/update fidelity is proven by real-client smoke.
- `scripts/run-client-stack-18085.ps1` now seeds local DB-spawn fixture
  `Rust DB Guide` (`creature` / `creature_template` `900010`) near `Rustone`
  with gossip/vendor NPC flags and Tough Jerky `117` plus Small Brown Pouch
  `2102` vendor rows.
- Overnight issues #23-#30 closed: handoff current-state wording was refreshed;
  `world-flow-test` now proves DB vendor `BuyPrice` charging,
  insufficient-money no-grant behavior, DB vendor sellback money,
  stack-decrement/full-removal behavior, empty DB vendor no-inventory marker,
  DB container-item filtering for `2102`, and combat-dummy loot autostore
  merging into existing compatible Tough Jerky stacks before empty-slot
  fallback. It also proves loot autostore no-space failure leaves DB state
  unchanged and that fresh Human Warriors have a valid item `25` main-hand DB
  row, equipment cache entry, and player update-field coverage.
- Issue #31 closed: `world-flow-test` now proves a fresh Human Warrior knows
  Heroic Strike rank 1 (`spell=78`), has a valid starter main-hand weapon
  precondition, and can send a fixture `CMSG_CAST_SPELL` with a combat-dummy
  unit target through `SMSG_CAST_RESULT`, `SMSG_SPELL_GO`, attacker-state,
  dummy, and rage update packets without adding full spell mechanics.
- Issue #32 closed: `wow_db::starter_item_template_refs()` exposes starter
  item refs with race/class/slot/amount context, and `world-flow-test` audits
  those refs against Docker `mangos.item_template`. The audit warns that 75
  refs are absent, mostly archived custom starter IDs `65020`-`65027`, plus
  item `129` for Dwarf/Night Elf hunter pants. This remains the #15 P2 data
  fixture gap, #15 was updated with the audit evidence, and the warning does
  not block current world-flow tests.
- Issue #33 closed: Rust starter item seeding now translates the
  archived missing starter IDs to source-backed templates present in
  `sql/base/mangos.sql`: food IDs `65020/65022/65025/65026/65027` use Tough
  Jerky `117`, water `65021` uses Refreshing Spring Water `159`, thrown IDs
  `65023/65024` use Small Throwing Knife `2947` / Crude Throwing Axe `25861`,
  and missing pants `129` use Rugged Trapper's Pants `147`. The starter item
  template audit is now quiet in `test-world-flow.cmd`, and #15 was closed as
  resolved by this slice.
- Issue #34 packet coverage added: `wow-network` now decodes the split
  `SMSG_UPDATE_OBJECT` response blocks and asserts the source stack count,
  destination item create state, equipped-bag container slot GUID, and
  destination contained GUID needed for immediate client rendering after
  `CMSG_SPLIT_ITEM`.
- Issue #35 packet coverage added: `wow-network` now asserts
  `CMSG_SWAP_ITEM` equipped-bag moves update the source/destination player
  inventory slot fields, the equipped bag's `CONTAINER_FIELD_SLOT_*` value,
  and the moved item's contained GUID when moving into and out of an equipped
  bag.
- Issue #36 packet coverage added: `wow-network` now asserts full stack merges
  clear the source backpack/player slot or equipped-bag container slot and
  update the destination item stack count.
- Issue #37 guardrail coverage added: `wow-network` now parses an unknown DB
  creature query and asserts the unknown-entry response uses the high-bit
  missing-template marker.
- Issue #38 guardrail added: DB/Rust Guide gossip select now only accepts the
  supported browse option `0`; invalid DB gossip option coverage was added to
  `world-flow-test` and the packet/unit tests.

## Tests Last Run

Passing locally:

```powershell
$env:PATH = "$env:USERPROFILE\.cargo\bin;$env:PATH"; cargo fmt
$env:PATH = "$env:USERPROFILE\.cargo\bin;$env:PATH"; cargo test -p wow-network equipped_bag -- --nocapture
$env:PATH = "$env:USERPROFILE\.cargo\bin;$env:PATH"; cargo test -p wow-network split_into_equipped_bag_update_body_contains_renderable_destination_stack -- --nocapture
$env:PATH = "$env:USERPROFILE\.cargo\bin;$env:PATH"; cargo test -p wow-network stack_merge_update -- --nocapture
$env:PATH = "$env:USERPROFILE\.cargo\bin;$env:PATH"; cargo test -p wow-network unknown_db_creature -- --nocapture
$env:PATH = "$env:USERPROFILE\.cargo\bin;$env:PATH"; cargo test -p wow-network invalid_db_vendor_gossip -- --nocapture
$env:PATH = "$env:USERPROFILE\.cargo\bin;$env:PATH"; cargo test -p wow-db starter_item_template_refs_replace_archived_custom_ids -- --nocapture
$env:PATH = "$env:USERPROFILE\.cargo\bin;$env:PATH"; cargo test -p wow-network raptor_strike -- --nocapture
$env:PATH = "$env:USERPROFILE\.cargo\bin;$env:PATH"; cargo test -p wow-network player_mana_update -- --nocapture
$env:PATH = "$env:USERPROFILE\.cargo\bin;$env:PATH"; cargo test -p wow-network -- --nocapture
$env:PATH = "$env:USERPROFILE\.cargo\bin;$env:PATH"; .\scripts\test-rust.cmd
$env:PATH = "$env:USERPROFILE\.cargo\bin;$env:PATH"; .\scripts\test-world-flow.cmd
git diff --check
```

Focused #34 split packet test passed. Focused #35 equipped-bag packet tests
passed. Focused #36 stack-merge, #37 unknown creature query, and #38 invalid
DB gossip tests passed. Focused `wow-db` archived starter replacement test
passed after the `129` -> `6127` boots fix. `test-rust.cmd` passed with 87
`wow-network` tests and 10 `wow-db` tests after stopping the manual auth/world
processes that were locking `authserver.exe`. Elevated `test-world-flow.cmd`
passed after the starter boots fix. `git diff --check` passed with only the
known LF-to-CRLF working-copy warning. `cargo fmt` passed with the existing
`could not canonicalize path C:\Users\subhe` warning.

Latest local verification after the Raptor Strike candidate fix:

- `cargo fmt` passed with the existing `could not canonicalize path C:\Users\subhe`
  warning.
- `cargo test -p wow-network raptor_strike -- --nocapture` passed.
- `cargo test -p wow-network player_mana_update -- --nocapture` passed.
- `cargo test -p wow-network -- --nocapture` passed with 91 tests.
- First `test-rust.cmd` rerun hit the known local stale-process lock on
  `target\debug\authserver.exe`; stopping local `authserver.exe` /
  `worldserver.exe` fixed it, and the second `test-rust.cmd` passed.
- Default `test-world-flow.cmd` lacked Docker access; elevated
  `test-world-flow.cmd` passed.
- `git diff --check` passed with only normal LF-to-CRLF working-copy warnings.

Checkpoint 1 closure gate, 2026-04-25:

- `.\scripts\test-rust.cmd` passed with 91 `wow-network` tests and 10
  `wow-db` tests.
- Default `.\scripts\test-auth-flow.cmd` lacked Docker access; elevated retry
  initially found stale local `authserver.exe` / `worldserver.exe` processes on
  the test ports. After stopping those, elevated `test-auth-flow.cmd` passed.
- Default `.\scripts\test-character-lifecycle.cmd` lacked Docker access;
  elevated `test-character-lifecycle.cmd` passed.
- Default `.\scripts\test-world-flow.cmd` lacked Docker access; elevated
  `test-world-flow.cmd` passed.

Notes:

- Docker-backed `test-world-flow.cmd` requires elevated Docker access locally.
- `git diff --check` only reported normal LF-to-CRLF working-copy warnings.

## Local Environment Notes

- Rust is available through `%USERPROFILE%\.cargo\bin`.
- Docker-backed tests may require elevated Docker access.
- MariaDB test container: `cmangos-rust-realmd` on local port `3307`.
- Manual client stack uses auth `127.0.0.1:13724` and world
  `127.0.0.1:18085` because the normal world port is blocked locally.
- If builds fail removing `authserver.exe` or `worldserver.exe`, stop stale
  local Rust server processes and rerun.

## Real-Client Smoke Notes

Last reported manual smoke: user confirmed the real-client world, combat dummy,
fixture loot/vendor, inventory, starter Skills UI, character pane, and map
gates are good enough to continue Checkpoint 1.

Current grading pass, 2026-04-25:

| Gate | Grade | Evidence / next action |
| --- | --- | --- |
| Auth and realm list | PASS | Real client logged in with `RUSTAUTH` and reached the Rust realm. |
| Character screen | PASS | First pass failed because stale local smoke characters made Rust send `SMSG_CHAR_ENUM count=11`; fixed by resetting the grading fixture, then retest looked good. |
| Character create/select/delete | PASS | User confirmed the remaining row looks good. |
| Enter world and relog | PASS | User confirmed the remaining row looks good. |
| Race/gender display ids | PASS | Night Elf female Hunter rendered correctly after starter boots fix. |
| Starter state | PARTIAL | Boots bug fixed by mapping archived item `129` to `6127`; Raptor Strike works, but broad starter spell coverage remains future parity outside the current gate. |
| Startup packet quietness | PARTIAL | No fatal loop reported; needs final log review after the spell/combat slice. |
| Movement | PASS | Logs show walk/turn/jump/fall/land/heartbeat packets parsed without disconnect. |
| Chat/emote | PARTIAL | `/hello` audio/text works, but no physical wave animation; #43 tracks non-blocking visual parity. |
| NPC visibility/query | PASS | User confirmed NPC interaction with Rust Guide and Rust DB Guide is good. |
| Gossip/vendor/trainer | DEFERRED for trainer | Rust Guide and Rust DB Guide interactions are good; trainer is deferred to #39 because meaningful trainer testing needs leveling/trainer-learning context. |
| Inventory and equipment | PASS | User confirmed the remaining row looks good. |
| Combat/spell | PASS | Real-client retest confirmed active Hunter spell `2973` / Raptor Strike works on the Rust stack. |
| Loot | PASS | User confirmed the remaining row looks good. |
| Death/respawn | DEFERRED | Player-character death/respawn is deferred to #44; NPC death/loot is covered by the combat dummy fixture. |
| Final fresh-character demo | PASS | User said OK after Raptor Strike and remaining-row review; trainer and player death/respawn are explicitly deferred to #39/#44. |

Next manual smoke should verify:

- Launch `scripts/run-client-stack-18085.cmd`.
- Login, create/select a character, enter world, move, and confirm logout/relog.
- Recreate a Night Elf female Hunter and confirm the boots slot renders
  Trapper's Boots instead of pants.
- Run the required automated gate for Checkpoint 1 closure and record final
  results: `test-rust.cmd`, `test-auth-flow.cmd`,
  `test-character-lifecycle.cmd`, and `test-world-flow.cmd`.
- Open the character pane and Skills UI; confirm armor/damage/block/crit/dodge
  and starter skills look sane and there is no disconnect.

## Non-blocking Backlog

GitHub issues are the source of truth:

- #3 reputation DBC placeholder; #4 first-login cinematic; #5 character
  lifecycle transactions/refactor.
- #11 fixture NPC/vendor hardcoding; #12 fixture combat/loot/XP/death gaps;
  #13 starter spell mechanics; #14 starter weapon equipment gap.
- #16 split update visual fidelity;
  #17 hearthstone replacement; #18 bag-container update fidelity;
  #19 loot autostore stack merging.
- #20 skill tier-step placeholder; #21 first-pass combat stat formulas.
- #22 explored-zone fields are serialized from DB but not yet discovered from
  map area flags or persisted on movement.
- #11 was updated with the DB creature spawn/query v1 evidence; DB vendor-list
  routing now has first packet/DB coverage, while real DB-backed gossip,
  trainer, combat, loot-table, and richer vendor validation remain future work.

#34-#36 add packet-shape evidence for #16/#18, but those remain open until
real-client smoke proves split and container visuals are correct. #37/#38 add
DB creature/gossip guardrails under #11. Fixture NPC/vendor gaps remain under
#11. Full combat, XP, death, respawn, and DB-backed loot remain under #12. Loot
autostore stack merging remains under #19. Exact combat stat formula parity
beyond the first equipment-derived pass remains under #21. Full area
exploration discovery/persistence remains under #22.

## Known Blockers And Gaps

- Previous Checkpoint 1 blocker cleared: Raptor Strike (`2973`) now works in
  the real client. Trainer is deferred to #39. Player death/respawn is deferred
  to #44. No grading-table row remains ungraded; Checkpoint 1 closure now needs
  the required automated gate rerun and final bookkeeping.
- Fixture NPC gossip/combat/loot remain hardcoded or fixture-only pending #11
  and #12; DB-backed creature spawns/query and vendor lists exist, but DB-backed
  gossip, trainers, loot tables, XP, respawn, and full vendor rules do not yet.
- Inventory v1 still lacks full durability changes, complete equipment rules,
  broader item-template/class/race validation, and final split/container visual
  parity closure.
- Full starter item parity still needs a source-data pass beyond the
  source-backed replacements used for #33, but the Docker fixture no longer has
  missing starter item template refs and the audit is quiet.
- Player self-spawn update now has more CMaNGOS default fields and first-pass
  equipment-derived combat stats, but full item stat bonuses, aura modifiers,
  ammo DPS, skill/defense adjustments, durability checks, exact DBC-derived
  skill tier steps, map area exploration discovery, and real aura state are
  still future parity work.

## Next Recommended Task

Continue Checkpoint 1 closure from the grading table, not broad feature work.
Checkpoint 1 is ready for commit/PR packaging from the grading-table and
automated-gate perspective. Next, review the uncommitted diff, decide whether
to split commits, then publish the closure slice with trainer deferred to #39
and player death/respawn deferred to #44.

## Key Files

- `crates/wow-network/src/world/mod.rs`
- `crates/wow-network/src/world/bootstrap.rs`
- `crates/wow-network/src/world/interactions.rs`
- `crates/wow-network/src/world/wire.rs`
- `crates/wow-network/src/world/tests.rs`
- `crates/wow-db/src/character.rs`
- `crates/wow-db/src/world_data.rs`
- `bins/world-flow-test/src/main.rs`
- `docs/rust_migration_plan.md`
- `docs/rust_auth_foundation.md`
- `scripts/test-rust.cmd`
- `scripts/test-world-flow.cmd`
- `scripts/run-client-stack-18085.cmd`
