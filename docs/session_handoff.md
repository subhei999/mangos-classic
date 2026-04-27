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
- Latest committed base before this slice: `98efe4a04 Add Northshire starter zone DB slice`
- Remote: `origin/codex/rust-auth-foundation`
- Worktree at handoff: contains Checkpoint 2 starter-zone fixture-lock work:
  new `bins/starter-zone-flow-test`, new
  `scripts/test-starter-zone-flow.cmd` / `.ps1`, workspace lockfile/member
  updates, and doc updates.

## Current Goal

Checkpoint 2: **Starter Zone Playability**.

Checkpoint 1 is closed. The next milestone is to make one starter zone playable
as a coherent early-game experience instead of a handpicked interaction demo.
Use Northshire Valley with a fresh Human Warrior as the Checkpoint 2 golden
path unless the user explicitly changes the target.

Important scope rule:
We are proving one vertical slice only. Fix P0/P1 bugs that block this slice.
Do not chase unrelated horizontal parity issues. For any non-blocking bug,
mismatch, missing subsystem, or cleanup gap you discover, create a GitHub issue
using the repo's bug triage policy, then continue the requested task.

Checkpoint 2 now has a durable plan and definition of done in
`docs/rust_migration_plan.md`. The core closure gate is a real-client
Northshire grading pass plus a new planned `scripts/test-starter-zone-flow.cmd`
automated harness covering DB-backed zone spawns, creature combat, loot tables,
quests, XP/level-up, trainers, death/respawn, and relog persistence.

## What Changed Recently

- Added cross-platform Rust test entrypoint `scripts/test-rust.sh` and updated the Rust GitHub workflow to run Rust checks on both Ubuntu and Windows, so remote Linux/macOS environments can run the same baseline script without `.cmd` wrappers.
- Added first Checkpoint 2 creature-retaliation guardrail for DB creatures: when a living DB target is attacked in the world tick loop, Rust now emits a creature->player `SMSG_ATTACKERSTATEUPDATE`, updates `UNIT_FIELD_HEALTH` for the player via `SMSG_UPDATE_OBJECT`, and keeps the player above a 1-HP survivor floor so death/respawn remains deferred to the dedicated Checkpoint 2 death slice (#44).
- Added focused `wow-network` unit coverage for the new player-health update packet body and DB-creature retaliation health-floor behavior.
- Fixed a real-client new-character loading-screen blocker observed in
  `world-client-18085.log`: auth and character create succeeded, but the client
  stalled after Rust sent a single 44 KB `SMSG_UPDATE_OBJECT` login burst for
  full ClassicDB Northshire density. Login bootstrap now sends the player /
  fixture / inventory create blocks first and DB creature create blocks in
  smaller follow-up `SMSG_UPDATE_OBJECT` chunks while keeping the 128-spawn
  Northshire visibility cap.
- Cemented the Checkpoint 2 plan in `docs/rust_migration_plan.md`: Northshire
  Valley / Human Warrior golden path, detailed slice order, required automated
  gate, real-client grading table, and definition of done.
- Added the first Checkpoint 2 starter-zone fixture-lock harness:
  `bins/starter-zone-flow-test` plus `scripts/test-starter-zone-flow.cmd`.
  The harness seeds a narrow Rust Northshire fixture range (`910xxx`) into
  CMaNGOS-shaped world tables and proves a clean Human Warrior starts in
  Northshire with DB-backed creature/template joins, quest giver/completer
  rows, vendor/trainer rows, loot rows with valid item templates, a gameobject,
  a graveyard link, and `realmcharacters` count.
- Extended `test-starter-zone-flow.cmd` into a Rust auth/world packet smoke:
  it starts authserver/worldserver, authenticates the `STARTZONE` account,
  enters the clean Human Warrior, and asserts the login `SMSG_UPDATE_OBJECT`
  includes all five seeded Northshire DB creature GUIDs. This proves the
  Northshire fixture is worldserver-visible, not just present in SQL.
- Added the first DB-backed Northshire hostile lifecycle slice. The Rust world
  session now tracks nearby DB creature runtime state, supports attacking a DB
  Young Wolf, transitions it from alive to damaged/dead lootable corpse, opens
  DB-backed corpse loot from `creature_loot_template` plus template gold,
  autostores the loot item, clears money/item loot state, and respawns it alive
  on loot release for the single-player harness.
- Added the first full-world-DB bridge for Checkpoint 2. CMaNGOS loads creature
  templates, creature spawn metadata, all creature rows, gameobjects, and then
  persistent respawn state into global ObjectMgr/map state at startup; map/grid
  visibility decides what the player sees. The Rust scripts now keep the tiny
  repo schema as the default but accept `-WorldSqlPath <dump.sql>` or
  `$env:CMANGOS_WORLD_SQL`, plus `-ResetWorldDatabase`, so the same Rust stack
  can import a full CMaNGOS world dump and then assert only the nearby
  Northshire slice.
- Added repeatable real ClassicDB import support with
  `scripts/import-classic-db-world.cmd`. The script expects
  `target/classic-db` cloned from `https://github.com/cmangos/classic-db`,
  imports `Full_DB/ClassicDB_1_12_1_z2815.sql.gz`, replays ClassicDB content
  and instance updates, then applies the remaining local CMaNGOS core world
  schema updates. The local Docker `mangos` DB was rebuilt this way and now has
  real Northshire creature entries/spawns.
- `starter-zone-flow-test` now detects real ClassicDB Northshire content first:
  real entries `197` Marshal McBride, `823` Deputy Willem, `951` Brother
  Paxton, `299` Young Wolf, and `6` Kobold Vermin. When present, it skips
  synthetic `910xxx` seeding, asserts the real nearby/visible creature rows,
  real quest relation rows, real Young Wolf loot-template rows, logs into the
  Rust worldserver, verifies the real spawn GUIDs are in `SMSG_UPDATE_OBJECT`,
  and proves a real Young Wolf can enter damaged runtime state. The synthetic
  fallback still covers the full dead corpse -> lootable/looted -> respawn
  lifecycle until real combat damage/kill pacing is widened.
- Raised the worldserver DB creature spawn cap from 32 to 64. With real
  ClassicDB Northshire, Brother Paxton was the 33rd nearest spawn because
  ambient rabbits/guards/peasants fill the area; the lower cap prevented the
  real-start-zone proof from seeing all required golden-path NPCs.
- Relaxed the starter-zone packet harness maximum server packet size because
  the real Northshire `SMSG_UPDATE_OBJECT` burst is about 22 KB, larger than
  the earlier tiny-fixture guard.
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
$env:PATH = "$env:USERPROFILE\.cargo\bin;$env:PATH"; .\scripts\test-rust.cmd # baseline before Checkpoint 2 harness
$env:PATH = "$env:USERPROFILE\.cargo\bin;$env:PATH"; cargo fmt
$env:PATH = "$env:USERPROFILE\.cargo\bin;$env:PATH"; cargo build -p starter-zone-flow-test
.\scripts\test-starter-zone-flow.cmd # default attempt failed due Docker access
.\scripts\test-starter-zone-flow.cmd # elevated Docker access passed
$env:PATH = "$env:USERPROFILE\.cargo\bin;$env:PATH"; .\scripts\test-rust.cmd
git diff --check
```

Latest Checkpoint 2 fixture-lock verification:

- `test-rust.cmd` passed on the current uncommitted slice with 93
  `wow-network` tests and 10 `wow-db` tests; only the known
  `could not canonicalize path C:\Users\subhe` warning appeared.
- `bash -n scripts/test-rust.sh` initially exposed a CRLF-sensitive syntax
  issue in the new shell entrypoint; the script was rewritten with LF endings
  and the syntax check now passes.
- Elevated `test-starter-zone-flow.cmd` passed after Docker Desktop's
  `desktop-linux` context became available, printing
  `starter-zone RealClassicDb lock passed for account STARTZONE, character Startzone`.
- After the split login-update fix, elevated `test-starter-zone-flow.cmd`
  passed again. A brief 64-spawn cap experiment reproduced the older small
  packet behavior but failed the real ClassicDB harness by hiding a required
  Kobold Vermin spawn, so the committed fix keeps 128 spawns and chunks the
  packet output instead.
- Final `test-rust.cmd` passed with 93 `wow-network` tests and 10 `wow-db`
  tests; only the known `could not canonicalize path C:\Users\subhe` warning
  appeared.
- `cargo fmt` passed.
- Baseline `test-rust.cmd` passed before Rust changes.
- `cargo fmt` passed with the existing `could not canonicalize path C:\Users\subhe`
  warning.
- `cargo build -p starter-zone-flow-test` passed.
- Default `test-starter-zone-flow.cmd` failed because Docker config/pipe access
  was denied; elevated retry passed and printed
  `starter-zone fixture lock passed for account STARTZONE, character Startzone`.
- Final `test-rust.cmd` passed with the new workspace member included.
- `git diff --check` passed with only normal LF-to-CRLF working-copy warnings.
- After the packet-visibility extension, elevated `test-starter-zone-flow.cmd`
  passed again with authserver/worldserver running.
- Final `test-rust.cmd` passed again after the packet-visibility extension.
- After the DB creature lifecycle slice, default `test-starter-zone-flow.cmd`
  again failed on Docker config/pipe access; elevated retry first exposed a
  MariaDB/SQLx count decode mismatch in the new loot query, which was fixed by
  casting the computed loot counts to unsigned. Elevated retry then passed,
  including Young Wolf kill, DB corpse loot, item autostore, money loot, loot
  release, and respawn. `test-rust.cmd` passed after a clippy helper-shape fix.
  Default `test-world-flow.cmd` failed on Docker access; elevated
  `test-world-flow.cmd` passed as a regression check.
- The local Docker `mangos` DB currently has only 7 creature templates and 7
  creature spawns from the tiny fixture/base import; there are no real
  Northshire rows for Young Wolf, Kobold Vermin, Marshal McBride, Deputy
  Willem, or Brother Paxton until a full CMaNGOS world dump is imported.
- PowerShell parser validation passed for `scripts/test-starter-zone-flow.ps1`
  and `scripts/run-client-stack-18085.ps1` after adding the full-world-DB import
  parameters.
- `scripts/import-classic-db-world.cmd` passed end to end against Docker
  MariaDB after cloning `cmangos/classic-db` into `target/classic-db`.
- Elevated `scripts/test-starter-zone-flow.cmd` passed against the rebuilt real
  ClassicDB world DB and printed
  `starter-zone RealClassicDb lock passed for account STARTZONE, character Startzone`.
- `scripts/test-rust.cmd` passed after the real-DB harness changes and the
  spawn-cap increase.

Previous Checkpoint 1 closure and focused checks:

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
- To test against a full CMaNGOS content DB instead of the tiny repo base,
  provide a world dump with either
  `.\scripts\test-starter-zone-flow.cmd -ResetWorldDatabase -WorldSqlPath C:\path\to\world.sql`
  or set `$env:CMANGOS_WORLD_SQL` before launching
  `scripts/run-client-stack-18085.cmd`.
- Preferred real-content rebuild path:
  `git clone --depth 1 https://github.com/cmangos/classic-db.git target\classic-db`
  if it is not already present, then run
  `.\scripts\import-classic-db-world.cmd`, then
  `.\scripts\test-starter-zone-flow.cmd`.
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

- Checkpoint 2 now has a DB fixture/harness boundary, packet proof that seeded
  Northshire creatures are worldserver-visible on Human Warrior login, and one
  DB-backed hostile creature lifecycle/combat path for Young Wolf. The next
  blocker is turning this into real starter-zone gameplay: creature retaliation
  / threat basics or quest kill-credit/loot progression, depending on the next
  selected vertical slice.
- The repo does not include the full CMaNGOS world content dump. The local
  repeatable path uses `target/classic-db`, which is ignored build/cache space,
  as the external content source.
- Trainer behavior was deferred from Checkpoint 1 to #39 and should be handled
  as part of Checkpoint 2 trainer v1 after XP/level requirements are available.
- Player death/respawn was deferred from Checkpoint 1 to #44 and should be
  handled as part of Checkpoint 2 death/corpse/graveyard/respawn.
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

Continue Checkpoint 2 with the next Northshire gameplay vertical slice:

- Keep Northshire Valley / Human Warrior as the golden path.
- Reuse `bins/starter-zone-flow-test` instead of expanding `world-flow-test`.
- Recommended next slice: DB-backed quest kill credit/progress for Kobold Camp
  Cleanup or creature retaliation/threat basics for the same Young Wolf path.
- Keep spell support narrow to Human Warrior melee/Heroic Strike.
- Do not add XP/level-up, trainer learning, or player death unless a P0/P1
  blocker requires it; log broader parity gaps as GitHub issues.

## Key Files

- `crates/wow-network/src/world/mod.rs`
- `crates/wow-network/src/world/bootstrap.rs`
- `crates/wow-network/src/world/interactions.rs`
- `crates/wow-network/src/world/wire.rs`
- `crates/wow-network/src/world/tests.rs`
- `crates/wow-db/src/character.rs`
- `crates/wow-db/src/world_data.rs`
- `bins/world-flow-test/src/main.rs`
- `bins/starter-zone-flow-test/src/main.rs`
- `docs/rust_migration_plan.md`
- `docs/rust_auth_foundation.md`
- `scripts/test-rust.cmd`
- `scripts/test-world-flow.cmd`
- `scripts/test-starter-zone-flow.cmd`
- `scripts/run-client-stack-18085.cmd`
