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
- Latest commit: `fac4f2ff7`
- Remote: `origin/codex/rust-auth-foundation`
- Uncommitted changes:
  - Checkpoint 1 first-pass player `SMSG_UPDATE_OBJECT` parity slice.
  - Handoff compaction / maintenance rules.

## Current Goal

Checkpoint 1: **First Playable World**.

The Rust auth/world stack can already authenticate a real WoW 1.12.1 client,
show character select, create/select/delete characters, enter a skeletal world,
move, logout/relog, persist position, seed starter spells/actions/skills/items,
and pass packet-level character lifecycle coverage.

Current Checkpoint 1 focus:

- Continue player `SMSG_UPDATE_OBJECT` parity and starter/default cleanup.
- Replace remaining hardcoded player stat/health/power fallback data with
  DB/DBC-backed world-data loading.
- Keep real-client smoke passes and `world-flow-test` green after each slice.

## What Changed Recently

- Began the first Checkpoint 1 player self-spawn update-object parity slice:
  - `wow_db::CharacterEnumEntry` now carries saved `characters.money`,
    `health`, and `power1..power5` into the world login path.
  - `wow-network::world` now writes named vanilla update-field indexes for
    health/power, class power maxima, level-1 stats for the currently covered
    race/class matrix, player-controlled flags, attack timers, base
    health/mana, bytes2 support flags, zeroed attack power, damage modifier
    baselines, and coinage.
  - Added a serialized update-mask unit test for the new player vitals and
    defaults.
- Manual real-client smoke after this slice was successful: user reported the
  in-client player state "looks better now."
- This handoff was compacted so it no longer acts as an ever-growing log.

## Tests Last Run

Passing locally for this slice:

```powershell
$env:PATH = "$env:USERPROFILE\.cargo\bin;$env:PATH"; cargo test -p wow-network
$env:PATH = "$env:USERPROFILE\.cargo\bin;$env:PATH"; .\scripts\test-rust.cmd
$env:PATH = "$env:USERPROFILE\.cargo\bin;$env:PATH"; .\scripts\test-world-flow.cmd
```

`test-world-flow.cmd` passed with:

```text
world flow check passed: auth session, create/delete happy path, negative create/delete cases, loaded/guild leader rejection, guild/group/social/pet/mail/auction cleanup, COD mail return, enum/count refresh
```

Notes:

- Docker-backed tests need elevated Docker access in the Codex app shell.
- Before the final successful `test-world-flow.cmd`, stale
  `authserver.exe` / `worldserver.exe` processes had to be stopped because
  they were locking `target\debug\authserver.exe`.
- `cargo fmt` was run.

Manual real-client smoke:

```powershell
.\scripts\run-client-stack-18085.cmd
```

Stack started successfully:

- Authserver: `127.0.0.1:13724`
- Worldserver: `127.0.0.1:18085`
- Client realmlist: `set realmlist 127.0.0.1:13724`

The user tested the WoW 1.12.1 client and considered the new player
update-object slice successful.

## Local Environment Notes

- Rust is available through `%USERPROFILE%\.cargo\bin`.
- Docker Desktop is installed; Docker commands may require elevated access.
- MariaDB test container: `cmangos-rust-realmd`
- MariaDB local port: `3307`
- Normal WoW auth/world ports are blocked on this machine:
  - `3724` blocked, so local auth uses `13724`.
  - `8085` blocked, so local world uses `18085`.
- Current stack logs:
  - `auth-client-13724.log`
  - `world-client-18085.log`

Stop the local DB container when needed:

```powershell
docker compose -f docker-compose.local.yml down
```

## Next Recommended Task

Continue Checkpoint 1 player update-object / starter-default parity:

1. Replace hardcoded level-1 health/mana/stat fallback data in
   `crates/wow-network/src/world/mod.rs` with DB/DBC-backed loading from the
   CMaNGOS world data (`player_classlevelstats`, `player_levelstats`, or a
   shared world-data cache).
2. Keep the serialized update-mask test, but make it assert values loaded from
   fixture data rather than local hardcoded tables.
3. Rerun:

```powershell
$env:PATH = "$env:USERPROFILE\.cargo\bin;$env:PATH"; .\scripts\test-rust.cmd
$env:PATH = "$env:USERPROFILE\.cargo\bin;$env:PATH"; .\scripts\test-world-flow.cmd
```

4. Run `scripts/run-client-stack-18085.cmd` and smoke the real client again.

## Key Files

- `docs/rust_migration_plan.md`
- `docs/rust_auth_foundation.md`
- `crates/wow-network/src/world/mod.rs`
- `crates/wow-db/src/character.rs`
- `bins/world-flow-test/src/main.rs`
- `bins/character-lifecycle-test/src/main.rs`
- `scripts/test-rust.cmd`
- `scripts/test-world-flow.cmd`
- `scripts/run-client-stack-18085.cmd`
- `config/worldserver.local.toml`
- `sql/base/mangos.sql`
- `sql/base/characters.sql`
- C++ references:
  - `src/game/Entities/UpdateFields.h`
  - `src/game/Entities/UpdateFields.cpp`
  - `src/game/Entities/Player.cpp`
  - `src/game/Globals/ObjectMgr.cpp`

## Known Blockers And Gaps

- The self-spawn update is improved but still not full CMaNGOS player object
  parity.
- Level-1 stat/health/power values are currently first-pass hardcoded fallback
  data for a small covered matrix; replace with world-data loading next.
- Broader world gameplay remains skeletal: movement persistence works, but
  validation, visibility, NPCs, combat, spells, inventory actions, loot, and
  chat still need Checkpoint 1 slices.
- Packet behavior is source-derived and harness-tested, but not yet compared
  against a fresh live CMaNGOS packet capture.
