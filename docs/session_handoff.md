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
- Latest commit: see `git log -1 --oneline`
- Remote: `origin/codex/rust-auth-foundation`
- Worktree: expected clean after the no-behavior world module split commit.

## Current Goal

Checkpoint 1: **First Playable World**.

The Rust auth/world stack can authenticate a real WoW 1.12.1 client, show
character select, create/select/delete characters, enter a minimal world, move,
logout/relog, persist position, seed starter state, open a fixture NPC gossip
dialogue, and fight a fixture combat dummy.

Current Checkpoint 1 focus:

- Continue first-minute gameplay slices without widening into full subsystem
  ports.
- Keep `test-rust.cmd` and `test-world-flow.cmd` green after each Rust world
  packet slice.
- Real-client smoke any new interaction: gossip, combat, corpse loot, starter
  spell, then proceed toward inventory and fuller loot v1.

## What Changed Recently

- Added a visible friendly `Rust Guide` fixture and hostile `Rust Combat Dummy`
  fixture to the initial world update.
- Added creature query, NPC text query, gossip hello/select, basic say/yell/
  emote chat, slash text emotes, and common emote animations for solo smoke.
- Added basic melee fixture combat: attack start/stop, attacker-state updates,
  dummy health updates, and a 2-second server-side auto-swing tick.
- Committed empty corpse-loot fixture slice: when the combat dummy reaches 0
  health, Rust marks it lootable, answers `CMSG_LOOT` with an empty
  CMaNGOS-shaped `SMSG_LOOT_RESPONSE`, handles empty money clear, and resets on
  `CMSG_LOOT_RELEASE`.
- Committed starter-spell/equipment fixture slice: the self-spawn
  `SMSG_UPDATE_OBJECT` now creates equipped bag-0 item objects, Rust grants 15
  stored rage on each fixture dummy auto-swing, Heroic Strike consumes rage in
  the fixture cast path, and Rust applies a small visible fixture damage update
  to the dummy. Real-client smoke confirmed Heroic Strike now casts and
  subtracts rage; full next-swing spell mechanics are intentionally deferred to
  GitHub #13.
- Completed the planned no-behavior cleanup gate before real item movement:
  split oversized `crates/wow-network/src/world/mod.rs` into focused included
  files for bootstrap, interactions, wire helpers, and tests. The split is
  deliberately include-based to preserve the existing module/visibility shape
  while making future slices cheaper to read and review.

## Tests Last Run

Passing locally:

```powershell
$env:PATH = "$env:USERPROFILE\.cargo\bin;$env:PATH"; cargo fmt
$env:PATH = "$env:USERPROFILE\.cargo\bin;$env:PATH"; cargo test -p wow-network builds_create_blocks_for_equipped_and_backpack_items -- --nocapture
$env:PATH = "$env:USERPROFILE\.cargo\bin;$env:PATH"; cargo test -p wow-network inventory -- --nocapture
$env:PATH = "$env:USERPROFILE\.cargo\bin;$env:PATH"; cargo test -p wow-network heroic -- --nocapture
$env:PATH = "$env:USERPROFILE\.cargo\bin;$env:PATH"; cargo test -p wow-network combat -- --nocapture
$env:PATH = "$env:USERPROFILE\.cargo\bin;$env:PATH"; cargo test -p wow-network spell -- --nocapture
$env:PATH = "$env:USERPROFILE\.cargo\bin;$env:PATH"; .\scripts\test-rust.cmd
$env:PATH = "$env:USERPROFILE\.cargo\bin;$env:PATH"; .\scripts\test-world-flow.cmd
```

Notes:

- First broad-script rerun after the module split hit the local
  `target\debug\authserver.exe` lock. Stopped local `authserver`/`worldserver`
  processes and reran `test-rust.cmd`, then `test-world-flow.cmd`, sequentially.
- Docker-backed `test-world-flow.cmd` requires elevated Docker access locally.
- `test-world-flow.cmd` result: auth session, create/delete happy path,
  negative create/delete cases, loaded/guild leader rejection, guild/group/
  social/pet/mail/auction cleanup, COD mail return, enum/count refresh.

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
- First right-click attack works.
- Continued attacks repeat without untargeting/retargeting.
- No disconnect or weird popup reported.
- Swing cadence feels faster than intended; tracked as non-blocking combat
  timing/parity under GitHub #12.
- Empty corpse-loot fixture smoke passed: after killing the dummy, the client
  handled the loot interaction without disconnect or visible regression.
- Starter spell fixture smoke now proves starter weapon recognition moved past
  GitHub #14: after equipped item create blocks, Human Warrior Heroic Strike on
  `Rust Combat Dummy` reports "not enough rage" instead of "melee weapon not
  equipped".
- Rust now grants fixture rage after dummy auto-swings. Real-client smoke
  confirmed Heroic Strike now casts and subtracts rage. A small fixture damage
  update was added afterward; user reported Heroic Strike lands but still looks
  like white damage, which is expected for the temporary fixture path. Full
  Heroic Strike parity remains covered by GitHub #13.

Next manual smoke should verify:

- Rerun the first-minute loop from login through movement, NPC gossip, combat,
  corpse loot, starter spell, and logout/relog.

## Non-blocking Backlog

GitHub issues are the source of truth:

- #3 `[Rust Rewrite][P3][Reputation] Initial reputation packet uses zeroed DBC state placeholder`
- #4 `[Rust Rewrite][P3][WorldBootstrap] First-login cinematic playback is not source-derived`
- #5 `[Rust Rewrite][P4][DB] Split character lifecycle module and add transactions`
- #11 `[Rust Rewrite][P2][NPC] Checkpoint fixture NPC is hardcoded instead of DB-backed`
- #12 `[Rust Rewrite][P2][Combat] Fixture combat lacks AI timers, death, XP, and loot parity`
- #13 `[Rust Rewrite][P2][Spells] Starter spell cast path lacks real spell mechanics`
- #14 `[Rust Rewrite][P2][Equipment] Starter character cannot cast Heroic Strike: melee weapon not equipped`

Full DB-backed loot, item/money persistence, death/respawn, XP, and combat
timing remain covered by #12. Full spellbook validation, Heroic Strike
next-swing behavior, rage/cooldown/aura/effect execution, and DBC/DB-backed
spell mechanics are covered by #13.
Starter equipped-weapon recognition for client-side Heroic Strike validation is
covered by #14.

## Known Blockers And Gaps

- The fixture NPC and combat dummy remain hardcoded pending #11.
- Combat remains deterministic fixture logic, not full CMaNGOS melee/death/AI
  behavior.
- Empty corpse loot is packet-shape-only; no loot table, item, money, XP,
  corpse decay, or DB persistence yet.
- Starter spell cast v1 is packet-shape/resource-only for Heroic Strike rank 1;
  it reaches the Rust cast handler, consumes fixture rage, and applies a small
  immediate fixture damage update. It is not true CMaNGOS next-swing spell
  behavior yet.
- Broader world gameplay remains skeletal: movement persistence works, but
  validation, visibility, spells, inventory actions, loot, vendors/trainers,
  quests, and multi-client behavior remain future Checkpoint 1+ slices.

## Next Recommended Task

Continue with inventory item query/move v1 or fuller loot money/item v1, using
CMaNGOS packet references before adding behavior. Item movement is still not
implemented; the module split only prepared the codebase so that slice is less
painful to work in.

## Key Files

- `crates/wow-network/src/world/mod.rs`
- `crates/wow-network/src/world/bootstrap.rs`
- `crates/wow-network/src/world/interactions.rs`
- `crates/wow-network/src/world/wire.rs`
- `crates/wow-network/src/world/tests.rs`
- `bins/world-flow-test/src/main.rs`
- `docs/rust_migration_plan.md`
- `docs/rust_auth_foundation.md`
- `scripts/test-rust.cmd`
- `scripts/test-world-flow.cmd`
- `scripts/run-client-stack-18085.cmd`
- C++ references:
  - `src/game/Loot/LootHandler.cpp`
  - `src/game/Loot/LootMgr.cpp`
  - `src/game/Loot/LootMgr.h`
  - `src/game/Server/Opcodes.h`
  - `src/game/Entities/UpdateFields.h`
  - `src/game/Entities/Object.cpp`
  - `src/game/Spells/SpellHandler.cpp`
  - `src/game/Spells/Spell.cpp`
  - `src/game/Spells/Spell.h`
  - `src/game/Spells/SpellTargetDefines.h`
