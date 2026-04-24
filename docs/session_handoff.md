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
- Latest commit: `0abb208ad Advance checkpoint 1 world interactions`
- Remote: `origin/codex/rust-auth-foundation`
- Worktree: currently has uncommitted Checkpoint 1 empty corpse-loot fixture
  changes in `crates/wow-network/src/world/mod.rs` and this handoff update.

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
- Real-client smoke any new interaction: gossip, combat, corpse loot, then
  proceed toward spell, inventory, and fuller loot v1.

## What Changed Recently

- Added a visible friendly `Rust Guide` fixture and hostile `Rust Combat Dummy`
  fixture to the initial world update.
- Added creature query, NPC text query, gossip hello/select, basic say/yell/
  emote chat, slash text emotes, and common emote animations for solo smoke.
- Added basic melee fixture combat: attack start/stop, attacker-state updates,
  dummy health updates, and a 2-second server-side auto-swing tick.
- Latest uncommitted slice: when the combat dummy reaches 0 health, Rust now
  marks it lootable with `UNIT_DYNAMIC_FLAGS`, answers `CMSG_LOOT` with an
  empty CMaNGOS-shaped `SMSG_LOOT_RESPONSE`, handles `CMSG_LOOT_MONEY` as an
  empty-money clear, and handles `CMSG_LOOT_RELEASE` with
  `SMSG_LOOT_RELEASE_RESPONSE` plus a fixture reset to full health.

## Tests Last Run

Passing locally:

```powershell
$env:PATH = "$env:USERPROFILE\.cargo\bin;$env:PATH"; cargo fmt
$env:PATH = "$env:USERPROFILE\.cargo\bin;$env:PATH"; cargo test -p wow-network combat_dummy -- --nocapture
$env:PATH = "$env:USERPROFILE\.cargo\bin;$env:PATH"; .\scripts\test-rust.cmd
$env:PATH = "$env:USERPROFILE\.cargo\bin;$env:PATH"; .\scripts\test-world-flow.cmd
```

Notes:

- First `test-rust.cmd` rerun failed at the final authserver build because
  Windows had `target\debug\authserver.exe` locked. Stopped local
  `authserver`/`worldserver` processes and reran successfully.
- First `test-world-flow.cmd` failed because Docker access was denied in the
  default shell. Reran with elevated Docker access and it passed.
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

Next manual smoke should verify:

- After the next interaction slice, rerun the first-minute loop from login
  through movement, NPC gossip, combat, corpse loot, and logout/relog.

## Non-blocking Backlog

GitHub issues are the source of truth:

- #3 `[Rust Rewrite][P3][Reputation] Initial reputation packet uses zeroed DBC state placeholder`
- #4 `[Rust Rewrite][P3][WorldBootstrap] First-login cinematic playback is not source-derived`
- #5 `[Rust Rewrite][P4][DB] Split character lifecycle module and add transactions`
- #11 `[Rust Rewrite][P2][NPC] Checkpoint fixture NPC is hardcoded instead of DB-backed`
- #12 `[Rust Rewrite][P2][Combat] Fixture combat lacks AI timers, death, XP, and loot parity`

No new non-blocking issue was created for the empty corpse-loot fixture slice;
full DB-backed loot, item/money persistence, death/respawn, XP, and combat
timing remain covered by #12.

## Known Blockers And Gaps

- The fixture NPC and combat dummy remain hardcoded pending #11.
- Combat remains deterministic fixture logic, not full CMaNGOS melee/death/AI
  behavior.
- Empty corpse loot is packet-shape-only; no loot table, item, money, XP,
  corpse decay, or DB persistence yet.
- Broader world gameplay remains skeletal: movement persistence works, but
  validation, visibility, spells, inventory actions, loot, vendors/trainers,
  quests, and multi-client behavior remain future Checkpoint 1+ slices.

## Next Recommended Task

1. Commit the current Checkpoint 1 corpse-loot fixture slice.
2. Continue with a narrow starter-spell cast v1 or inventory item query/move v1
   slice, using CMaNGOS packet references before adding behavior.

## Key Files

- `crates/wow-network/src/world/mod.rs`
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
