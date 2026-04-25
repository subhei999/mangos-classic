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
- Worktree at handoff: contains uncommitted #34-#38 guardrail edits in
  `crates/wow-network/src/world/tests.rs`,
  `crates/wow-network/src/world/interactions.rs`, and
  `bins/world-flow-test/src/main.rs`.

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

## What Changed Recently

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
$env:PATH = "$env:USERPROFILE\.cargo\bin;$env:PATH"; .\scripts\test-rust.cmd
$env:PATH = "$env:USERPROFILE\.cargo\bin;$env:PATH"; .\scripts\test-world-flow.cmd
git diff --check
```

Focused #34 split packet test passed. Focused #35 equipped-bag packet tests
passed. Focused #36 stack-merge, #37 unknown creature query, and #38 invalid
DB gossip tests passed. `test-rust.cmd` passed with 87 `wow-network` tests and
10 `wow-db` tests. Elevated `test-world-flow.cmd` passed with the DB gossip
invalid-option guard. `git diff --check` passed with only the known LF-to-CRLF
working-copy warning. `cargo fmt` passed with the existing `could not
canonicalize path C:\Users\subhe` warning.

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

Next manual smoke should verify:

- Launch `scripts/run-client-stack-18085.cmd`.
- Login, create/select a character, enter world, move, and confirm logout/relog.
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

For unattended overnight work, continue after #38 in numeric issue order. #31,
#32, and #33 are closed; #34-#36 have packet-shape coverage but still want
real-client visual smoke before closing #16/#18. Do not require real-client
visual testing overnight; leave the `scripts/run-client-stack-18085.cmd` smoke
pass for morning. Start with the smallest issue slice available, run focused
packet/DB tests first, and use `test-rust.cmd` / `test-world-flow.cmd` when
practical for Rust world-flow changes.

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
