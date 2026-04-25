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
- Latest commit: this commit, `Add fixture loot and vendor item flow` (use
  `git log -1 --oneline` for the exact hash)
- Remote: `origin/codex/rust-auth-foundation`
- Worktree: expected clean after the fixture loot/vendor inventory slice.

## Current Goal

Checkpoint 1: **First Playable World**.

The Rust auth/world stack can authenticate a real WoW 1.12.1 client, show
character select, create/select/delete characters, enter a minimal world, move,
logout/relog, persist position, seed starter state, open a fixture NPC gossip
dialogue, fight a fixture combat dummy, move/equip/destroy/split/merge basic
inventory items, and exercise fixture loot/vendor item flows in the packet DB
harness.

Important scope rule:
We are proving one vertical slice only. Fix P0/P1 bugs that block this slice.
Do not chase unrelated horizontal parity issues. For any non-blocking bug,
mismatch, missing subsystem, or cleanup gap you discover, create a GitHub issue
using the repo's bug triage policy, then continue the requested task.

## What Changed Recently

- Split `crates/wow-network/src/world/mod.rs` into focused include files for
  bootstrap, interactions, wire helpers, and tests so Checkpoint 1 slices are
  cheaper to read and review.
- Added Inventory v1 support for backpack moves, equip/unequip, destroy,
  partial destroy, split, equipped-bag storage positions, bag-internal moves,
  and simple same-template stack merges, with DB persistence and packet-harness
  coverage.
- Improved inventory update packet fidelity for bag containers: create/update
  blocks now distinguish item versus container objects, include container slot
  counts where needed, update player inventory slots, update container slot
  fields, and send item contained-guid changes for supported moves/splits.
- Added a fixture combat-dummy loot loop: killing the dummy exposes money and
  Tough Jerky `117` x2, `CMSG_LOOT_MONEY` persists coinage, and
  `CMSG_AUTOSTORE_LOOT_ITEM` stores the item in the backpack and sends update
  packets.
- Extended the `Rust Guide` fixture to also be a vendor. It lists and sells a
  source-backed 6-slot container item `2102` plus Tough Jerky `117`, inserts
  purchases into the first empty backpack slot, refreshes inventory, and is
  covered by `world-flow-test`.

## Tests Last Run

Passing locally:

```powershell
$env:PATH = "$env:USERPROFILE\.cargo\bin;$env:PATH"; cargo fmt
$env:PATH = "$env:USERPROFILE\.cargo\bin;$env:PATH"; cargo test -p wow-network combat_dummy_loot -- --nocapture
$env:PATH = "$env:USERPROFILE\.cargo\bin;$env:PATH"; cargo test -p wow-network inventory -- --nocapture
$env:PATH = "$env:USERPROFILE\.cargo\bin;$env:PATH"; cargo test -p wow-network vendor -- --nocapture
$env:PATH = "$env:USERPROFILE\.cargo\bin;$env:PATH"; cargo test -p wow-network parses_rust_guide_buy_item_packet -- --nocapture
$env:PATH = "$env:USERPROFILE\.cargo\bin;$env:PATH"; cargo check -p world-flow-test
$env:PATH = "$env:USERPROFILE\.cargo\bin;$env:PATH"; .\scripts\test-rust.cmd
$env:PATH = "$env:USERPROFILE\.cargo\bin;$env:PATH"; .\scripts\test-world-flow.cmd
git diff --check
```

`test-rust.cmd` passed with 73 `wow-network` tests. `test-world-flow.cmd`
passed with auth session, create/delete cases, loaded/guild leader rejection,
backpack moves, equip/unequip, Rust Guide vendor buys, bag-contained moves,
stack merge, destroy guardrails, partial destroy, split, bag-contained destroy,
cleanup checks, COD mail return, and enum/count refresh.

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

Last reported manual smoke:

- `Rust Combat Dummy` is visible and targetable.
- First right-click attack and continued attacks work without retargeting.
- Empty corpse-loot fixture smoke passed before item/money loot was added.
- Heroic Strike now reaches the Rust cast path, consumes fixture rage, and
  applies fixture damage; full next-swing spell parity remains GitHub #13.
- Inventory smoke confirmed backpack movement, equip persistence, and basic
  destroy behavior; stack splitting had no good manual fixture before this
  vendor/loot slice.
- Fixture loot/vendor smoke is good overall. Minor issue found: looted Tough
  Jerky creates a separate stack instead of merging into an existing stack, but
  manual stacking works afterward; tracked as GitHub #19.

Next manual smoke should verify:

- Launch `scripts/run-client-stack-18085.cmd`.
- Login, create/select a character, enter world, move, and confirm logout/relog.
- Open `Rust Guide` vendor, buy the container item `2102` and Tough Jerky `117`.
- Equip the bought container if the client accepts it, move jerky into and
  within the bag, split/merge stacks, destroy a backpack item, and relog.
- Kill `Rust Combat Dummy`, loot money and jerky, and confirm no disconnects or
  stale loot-window behavior.

## Non-blocking Backlog

GitHub issues are the source of truth:

- #3 `[Rust Rewrite][P3][Reputation] Initial reputation packet uses zeroed DBC state placeholder`
- #4 `[Rust Rewrite][P3][WorldBootstrap] First-login cinematic playback is not source-derived`
- #5 `[Rust Rewrite][P4][DB] Split character lifecycle module and add transactions`
- #11 `[Rust Rewrite][P2][NPC] Checkpoint fixture NPC is hardcoded instead of DB-backed`
- #12 `[Rust Rewrite][P2][Combat] Fixture combat lacks AI timers, death, XP, and loot parity`
- #13 `[Rust Rewrite][P2][Spells] Starter spell cast path lacks real spell mechanics`
- #14 `[Rust Rewrite][P2][Equipment] Starter character cannot cast Heroic Strike: melee weapon not equipped`
- #15 `[Rust Rewrite][P2][Inventory] Custom starter item templates are absent from Docker world fixture`
- #16 `[Rust Rewrite][P2][Inventory] Split item updates lack full client-visible destination create fidelity`
- #17 `[Rust Rewrite][P2][NPC] Hearthstone replacement from innkeepers is not implemented`
- #18 `[Rust Rewrite][P2][Inventory] Bag-container moves lack full container slot update fidelity`
- #19 `[Rust Rewrite][P2][Loot] Autostore loot does not merge into existing item stacks`

The current slice improves #16 and #18 but does not close them until real-client
smoke proves split and container visuals are correct. Fixture NPC/vendor gaps
remain under #11. Full combat, XP, death, respawn, and DB-backed loot remain
under #12. Loot autostore stack merging remains under #19.

## Known Blockers And Gaps

- The fixture NPC, vendor, and combat dummy remain hardcoded pending #11.
- Loot v1 is fixture-only: no loot tables, corpse state persistence, XP,
  respawn, group loot, or DB-backed creature loot yet.
- Vendor v1 is fixture-only on `Rust Guide`, not `npc_vendor` DB-backed.
- Bought container item `2102` is source-backed and has 6 container slots, but
  it is an ammo pouch template; real-client smoke must prove whether it accepts
  the item movement we need for manual bag testing.
- Inventory v1 still lacks full durability changes, complete equipment rules,
  broader item-template/class/race validation, and real-client closure of all
  split/container visual update cases.
- Loot autostore does not merge into existing compatible stacks before choosing
  an empty slot; manual stacking works and the issue is tracked as #19.

## Next Recommended Task

Run the real-client smoke for the new Rust Guide vendor plus combat-dummy
item/money loot. If the client-visible inventory updates are good, continue
Checkpoint 1 with a small DB-backed vendor/loot data slice or close out the
remaining Inventory v1 visual gaps found by smoke.

## Key Files

- `crates/wow-network/src/world/mod.rs`
- `crates/wow-network/src/world/bootstrap.rs`
- `crates/wow-network/src/world/interactions.rs`
- `crates/wow-network/src/world/wire.rs`
- `crates/wow-network/src/world/tests.rs`
- `crates/wow-db/src/character.rs`
- `bins/world-flow-test/src/main.rs`
- `docs/rust_migration_plan.md`
- `docs/rust_auth_foundation.md`
- `scripts/test-rust.cmd`
- `scripts/test-world-flow.cmd`
- `scripts/run-client-stack-18085.cmd`
