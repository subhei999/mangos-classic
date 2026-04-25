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
- Latest committed base before this slice: `17c6f0dfb Add DB-backed vendor interactions`
- Remote: `origin/codex/rust-auth-foundation`
- Worktree at handoff: clean against `origin/codex/rust-auth-foundation`.

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
- Overnight issues #23-#28 closed: handoff current-state wording was refreshed;
  `world-flow-test` now proves DB vendor `BuyPrice` charging,
  insufficient-money no-grant behavior, DB vendor sellback money,
  stack-decrement/full-removal behavior, empty DB vendor no-inventory marker,
  DB container-item filtering for `2102`, and combat-dummy loot autostore
  merging into existing compatible Tough Jerky stacks before empty-slot
  fallback.

## Tests Last Run

Passing locally:

```powershell
$env:PATH = "$env:USERPROFILE\.cargo\bin;$env:PATH"; cargo fmt
$env:PATH = "$env:USERPROFILE\.cargo\bin;$env:PATH"; cargo test -p wow-network vendor_inventory -- --nocapture
$env:PATH = "$env:USERPROFILE\.cargo\bin;$env:PATH"; .\scripts\test-rust.cmd
$env:PATH = "$env:USERPROFILE\.cargo\bin;$env:PATH"; .\scripts\test-world-flow.cmd
git diff --check
```

`cargo test -p wow-network vendor -- --nocapture` passed 4 focused vendor/gossip
packet-shape tests. `cargo test -p wow-network gossip -- --nocapture` passed 4
focused gossip tests. `test-rust.cmd` passed with 79 `wow-network` tests. The first full rerun
hit the usual final-build binary lock; stopping stale local `authserver.exe` /
`worldserver.exe` processes and rerunning passed. `test-world-flow.cmd` passed
with auth session, create/delete cases, DB creature query/gossip/vendor list,
DB vendor insufficient-money guard, inventory/vendor buy/sell/loot flows,
cleanup checks, COD mail return, and enum/count refresh.
Latest overnight reruns:
`test-world-flow.cmd` passed with DB vendor BuyPrice charge, sellback,
empty-vendor, container-filter, and loot-autostore stack-merge coverage.
`test-rust.cmd` passed with 79 `wow-network` tests. `git diff --check` passed
with only the known LF-to-CRLF working-copy warnings. `cargo fmt` passed with
the existing `could not canonicalize path C:\Users\subhe` warning.

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
- #15 custom starter item templates; #16 split update visual fidelity;
  #17 hearthstone replacement; #18 bag-container update fidelity;
  #19 loot autostore stack merging.
- #20 skill tier-step placeholder; #21 first-pass combat stat formulas.
- #22 explored-zone fields are serialized from DB but not yet discovered from
  map area flags or persisted on movement.
- #11 was updated with the DB creature spawn/query v1 evidence; DB vendor-list
  routing now has first packet/DB coverage, while real DB-backed gossip,
  trainer, combat, loot-table, and richer vendor validation remain future work.

The current slice improves #16 and #18 but does not close them until real-client
smoke proves split and container visuals are correct. Fixture NPC/vendor gaps
remain under #11. Full combat, XP, death, respawn, and DB-backed loot remain
under #12. Loot autostore stack merging remains under #19. Exact combat stat
formula parity beyond the first equipment-derived pass remains under #21. Full
area exploration discovery/persistence remains under #22.

## Known Blockers And Gaps

- Fixture NPC gossip/combat/loot remain hardcoded or fixture-only pending #11
  and #12; DB-backed creature spawns/query and vendor lists exist, but DB-backed
  gossip, trainers, loot tables, XP, respawn, and full vendor rules do not yet.
- Inventory v1 still lacks full durability changes, complete equipment rules,
  broader item-template/class/race validation, and final split/container visual
  parity closure.
- Loot autostore does not merge into existing compatible stacks before choosing
  an empty slot; manual stacking works and the issue is tracked as #19.
- Player self-spawn update now has more CMaNGOS default fields and first-pass
  equipment-derived combat stats, but full item stat bonuses, aura modifiers,
  ammo DPS, skill/defense adjustments, durability checks, exact DBC-derived
  skill tier steps, map area exploration discovery, and real aura state are
  still future parity work.

## Next Recommended Task

For unattended overnight work, continue Checkpoint 1 at GitHub issue #29 and
proceed through the remaining numbered issues in order. Do not require
real-client visual testing overnight; leave the
`scripts/run-client-stack-18085.cmd` smoke pass for morning. Start with the
smallest issue slice available, run focused packet/DB tests first, and use
`test-rust.cmd` / `test-world-flow.cmd` when practical for Rust world-flow
changes.

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
