# Session Handoff

This file is the current operating brief for the next Rust migration session.
Keep it short. Durable roadmap and milestone history belong in
`docs/rust_migration_plan.md`; auth-specific setup belongs in
`docs/rust_auth_foundation.md`.

## Handoff Rules

- Target length: about 120 lines. Hard cap: 180 lines.
- Keep only current branch state, active goal, last meaningful changes, exact
  tests run, local blockers, and the next recommended task.
- Replace old session detail with a one-line summary or move durable roadmap
  detail into `docs/rust_migration_plan.md`.
- Do not append a full chronological log. Each update should prune stale
  bullets from the same section.
- Keep "What Changed Recently" focused on the last one or two substantial
  slices.

## Current Branch

- Branch: `codex/rust-auth-foundation`
- Latest commit: see `git log -1 --oneline`
- Remote: `origin/codex/rust-auth-foundation`
- Latest local commit includes:
  - Checkpoint 1 DB-backed and derived player vitals/stats/XP slice.
  - `AGENTS.md` bug triage / GitHub logging policy.
  - Checkpoint 1 initial faction, class power, tutorial flag, and first-login
    cleanup slice.

## Current Goal

Checkpoint 1: **First Playable World**.

The Rust auth/world stack can already authenticate a real WoW 1.12.1 client,
show character select, create/select/delete characters, enter a skeletal world,
move, logout/relog, persist position, seed starter spells/actions/skills/items,
and pass packet-level character lifecycle coverage.

Current Checkpoint 1 focus:

- Continue player `SMSG_UPDATE_OBJECT` parity and starter/default cleanup.
- Expand DB/DBC-backed starter/default loading beyond health/mana/stats/XP into
  class power defaults, faction/reputation, and fuller create-info parity.
- Keep real-client smoke passes and `world-flow-test` green after each slice.

## What Changed Recently

- Replaced hardcoded level-1 health/mana/stat/next-XP fallbacks with
  `wow_db::get_player_world_stats` sourced from CMaNGOS
  `player_classlevelstats`, `player_levelstats`, and `player_xp_for_level`.
  New character creation now seeds derived max health/mana using the CMaNGOS
  stamina/intellect formulas, and `world-flow-test` verifies those values.
- User ran a real-client smoke after the derived vitals/stat/XP slice and
  reported it works; level 1 Human Warrior HP is no longer stuck at 20.
- Added repo-local bug triage / GitHub logging policy to `AGENTS.md`.
- Began the next bootstrap slice: Rust now sends a basic CMaNGOS-shaped
  `SMSG_INITIALIZE_FACTIONS` packet after initial spells/action buttons, class
  power defaults use explicit CMaNGOS `GetCreatePowers` constants for warrior
  rage and rogue energy, and packet tests cover both.
- Finished the tutorial/first-login cleanup slice: Rust loads/saves
  `character_tutorial`, handles tutorial flag/clear/reset opcodes, sends
  account tutorial flags during login, and marks `cinematic = 1` plus clears
  `AT_LOGIN_FIRST` once a character login is accepted. `world-flow-test` now
  verifies the first-login DB cleanup.

## Tests Last Run

Passing locally for this slice:

```powershell
$env:PATH = "$env:USERPROFILE\.cargo\bin;$env:PATH"; .\scripts\test-rust.cmd
$env:PATH = "$env:USERPROFILE\.cargo\bin;$env:PATH"; .\scripts\test-world-flow.cmd
$env:PATH = "$env:USERPROFILE\.cargo\bin;$env:PATH"; cargo test -p wow-db -p wow-network
$env:PATH = "$env:USERPROFILE\.cargo\bin;$env:PATH"; cargo test -p wow-network
```

Notes:

- Docker-backed tests need elevated Docker access in the Codex app shell.
- `cargo fmt` was run.
- During this session, stale `authserver.exe` / `worldserver.exe` processes
  had to be stopped once because they were locking `target\debug\authserver.exe`.

Last manual real-client smoke:

```powershell
.\scripts\run-client-stack-18085.cmd
```

Stack started successfully:

- Authserver: `127.0.0.1:13724`
- Worldserver: `127.0.0.1:18085`
- Client realmlist: `set realmlist 127.0.0.1:13724`

The user reported the derived vitals/stat/XP smoke works.

## Local Environment Notes

- Rust is available through `%USERPROFILE%\.cargo\bin`.
- Docker-backed tests may require elevated Docker access.
- MariaDB test container: `cmangos-rust-realmd` on local port `3307`.
- Normal WoW ports are blocked locally; manual client stack uses auth `13724`
  and world `18085`.

## Next Recommended Task

Continue Checkpoint 1 player update-object / starter-default parity:

1. Run a short real-client smoke for the tutorial/first-login slice if desired:
   create/login a fresh character, confirm normal login, logout/relog, and
   optionally toggle tutorial hints.
2. Continue starter-default cleanup: DBC-backed reputation defaults,
   faction/reputation persistence, and source-derived cinematic playback.
3. Rerun after the next code slice:

```powershell
$env:PATH = "$env:USERPROFILE\.cargo\bin;$env:PATH"; .\scripts\test-rust.cmd
$env:PATH = "$env:USERPROFILE\.cargo\bin;$env:PATH"; .\scripts\test-world-flow.cmd
```


## Key Files

- `docs/rust_migration_plan.md`
- `docs/rust_auth_foundation.md`
- `crates/wow-network/src/world/mod.rs`
- `crates/wow-db/src/character.rs`
- `bins/world-flow-test/src/main.rs`
- `scripts/test-rust.cmd`
- `scripts/test-world-flow.cmd`
- `scripts/run-client-stack-18085.cmd`
- `sql/base/mangos.sql`
- `sql/base/characters.sql`
- C++ references:
  - `src/game/Entities/UpdateFields.h`
  - `src/game/Entities/UpdateFields.cpp`
  - `src/game/Entities/Player.cpp`
  - `src/game/Globals/ObjectMgr.cpp`
  - `src/game/Reputation/ReputationMgr.cpp`
  - `src/game/Entities/CharacterHandler.cpp`
  - `src/game/Server/WorldSession.cpp`

## Non-blocking Backlog

- GitHub Issues are now enabled and should be the source of truth for
  non-blocking P2/P3/P4 work.
- Current logged issues:
  - #3 `[Rust Rewrite][P3][Reputation] Initial reputation packet uses zeroed DBC state placeholder`
  - #4 `[Rust Rewrite][P3][WorldBootstrap] First-login cinematic playback is not source-derived`
  - #5 `[Rust Rewrite][P4][DB] Split character lifecycle module and add transactions`

## Known Blockers And Gaps

- The self-spawn update is improved but still not full CMaNGOS player object
  parity.
- Health/mana/stats/next-XP now come from world DB tables with CMaNGOS-style
  stamina/intellect derivation, but broader derived player fields still need
  source-derived parity work.
- Broader world gameplay remains skeletal: movement persistence works, but
  validation, visibility, NPCs, combat, spells, inventory actions, loot, and
  chat still need Checkpoint 1 slices.
