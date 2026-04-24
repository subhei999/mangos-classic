# Rust Migration Plan

This document is the long-running memory map for the CMaNGOS Classic Rust
rewrite. Keep it current whenever architecture, milestone order, test commands,
or crate responsibilities change.

## North Star

Build a faithful Rust implementation of CMaNGOS Classic for WoW 1.12.1 while
keeping the existing C++ server available as the behavioral reference.

The rewrite should progress through working vertical slices, not broad rewrites
that cannot be run. Each milestone must leave the repo in a testable state.

## Current State

- Branch: `codex/rust-auth-foundation`
- Base: `master`
- C++ tree: untouched and still the canonical behavior reference
- Rust status: authserver foundation exists and builds locally; worldserver
  skeleton can carry a real 1.12.1 client into a minimal in-world state
- Local unit/lint/build entrypoint: `scripts/test-rust.cmd`
- Local MariaDB smoke entrypoint: `scripts/test-rust-db.cmd`
- Local auth flow entrypoint: `scripts/test-auth-flow.cmd`
- Local DB config: `config/authserver.local.toml`
- Local world skeleton config: `config/worldserver.local.toml`
- Docker DB harness: `docker-compose.local.yml`

## Crate Map

- `bins/authserver`
  - Runnable Rust login/auth server.
  - Owns CLI parsing, config loading, tracing setup, DB pool creation, and
    server startup.
- `bins/worldserver`
  - Runnable Rust worldserver skeleton.
  - Current focus is accepting the real client's world TCP connection,
    bootstrapping auth, character enum, character create/login, minimal
    self-spawn, movement position persistence, and post-login probe cleanup.
- `crates/wow-config`
  - TOML and environment configuration.
  - `AUTH_` environment variables override authserver TOML.
- `crates/wow-crypto`
  - SRP6 auth, header crypto, and crypto placeholders.
  - Must stay protocol-faithful and heavily tested.
- `crates/wow-proto`
  - Wire packet structs and encode/decode helpers.
  - Current focus is realmd/auth packets only.
- `crates/wow-db`
  - Database models and queries against existing CMaNGOS schemas.
  - Current focus is `realmd`: accounts, bans, realms, character counts, plus
    enough `characters` schema access for character select, creation, login,
    and logout position persistence.
- `crates/wow-network`
  - Async TCP servers and per-connection session state machines.
  - Current focus is auth handshake, realm-list flow, and early world session
    bootstrap.
- `crates/wow-common`
  - Shared enums, GUIDs, positions, and cross-crate primitives.

## Migration Rules

- Preserve C++ behavior first; improve design only where Rust requires it or a
  test proves the behavior.
- Port from the existing C++ source before inventing protocol behavior.
- Keep C++ and Rust side by side until a full replacement path is proven.
- Favor small vertical milestones that can be run locally and in CI.
- Every new subsystem needs:
  - a crate or module owner,
  - a C++ reference path,
  - packet/schema compatibility notes,
  - tests that can run without private data.
- Do not add broad world/gameplay modules to the workspace until the milestone
  needs them.

## Milestones

1. Auth foundation
   - Rust authserver starts.
   - Connects to CMaNGOS `realmd` schema.
   - Handles SRP challenge/proof and realm-list packet construction.
   - Local and CI tests pass.
2. Auth compatibility harness
   - Scripted TCP test client exists for the happy path.
   - Verify known failure responses against seeded DB data.
   - Compare packet shapes against C++ `realmd`.
3. Worldserver skeleton
   - Add `bins/worldserver` only after auth compatibility is stable.
   - Accept world TCP connections and perform header/session bootstrap.
   - Current status: binary and TCP skeleton exist; it sends
     `SMSG_AUTH_CHALLENGE`, parses/verifies `CMSG_AUTH_SESSION`, and returns
     initial auth-ok. It also decrypts/encrypts post-auth world headers and
     responds to `CMSG_CHAR_ENUM`.
4. Character list vertical slice
   - Load account session key from `realmd`.
   - Read character database enough to answer character list requests.
   - Current status: worldserver reads `characters.characters` joined to
     current pet/guild rows and serializes the CMaNGOS character enum field
     order. The local client-stack helper imports `sql/base/characters.sql` and
     seeds `RUSTAUTH` with `Rustone` for manual testing.
5. Enter-world vertical slice
   - Load player, map, position, and minimum update packets.
   - Client can enter a static world state.
   - Current status: real client can select seeded `Rustone` or a newly created
     character, leave loading screen, enter the world, walk around, logout, and
     relog at the persisted position. The server sends an early login packet
     burst plus a minimal self-spawn `SMSG_UPDATE_OBJECT`.
6. Character creation vertical slice
   - Handle `CMSG_CHAR_CREATE` from the real client.
   - Insert CMaNGOS-schema `characters` and `character_homebind` rows.
   - Update `realmd.realmcharacters`.
   - Current status: manually proven with a real 1.12.1 client creating a
     Human Warrior named `Rusttwo`; the character appeared in enum, entered the
     world, moved, logged out, and persisted position. This is still a minimal
     vertical slice, not full `Player::Create` parity.
7. CMaNGOS starter-default parity
   - Populate newly created characters with C++-matching starter spells,
     skills, action buttons, starter items/equipment, health/power/stat
     defaults, cinematic flags, and race/class create info from source data
     instead of hardcoded Rust fallback values.
   - Add negative/manual coverage for duplicate names, invalid names, invalid
     race/class combos, and character-count limits.
   - Current status: first real-client Human Warrior slice is complete and
     pushed in `fac4f2ff7`. Rust reads `playercreateinfo` from the world DB,
     persists starter spells/skills/action buttons from CMaNGOS source tables,
     sends those spell/action rows during login bootstrap, and persists starter
     outfit/items from archived CMaNGOS `playercreateinfo_item` rows. Real
     client verification confirmed starter spellbook, action bar, visible
     shirt/pants/boots/sword/shield, hearthstone, and gift voucher. Broader
     race/class item visuals, exact DBC skill ranges, stats, power/health,
     cinematic flags, and DBC-backed validation remain open.
8. World bootstrap packet parity
   - Expand the current minimal post-login responses toward CMaNGOS behavior.
   - Keep `CMSG_NAME_QUERY`, account-data, tutorial state, channels,
     zone-update, raid/battleground probes, mail timing, and initial faction
     behavior quiet, tested, and source-derived.
9. Character lifecycle coverage
   - Add `CMSG_CHAR_DELETE`, stronger character-screen negative cases,
     rename/delete cleanup semantics, and a scripted world/character harness so
     character-screen behavior is not only manually tested through the WoW
     client.
   - Current status: `CMSG_CHAR_DELETE` is implemented and manually proven
     after fixing `character_tutorial` cleanup; next work is automated coverage
     for create/delete/count refresh and negative character-screen behavior.
10. Gameplay slices
   - Movement, chat, inventory, combat, spells, NPCs, loot, groups, guilds.
   - Each slice gets packet tests and DB fixture coverage.
11. Slamrock/Hardcore fork behavior
   - Port fork-specific mechanics after baseline classic behavior has coverage.

## Testing Contract

Run before committing Rust work:

```powershell
.\scripts\test-rust.cmd
```

Run when DB/authserver behavior changes:

```powershell
.\scripts\test-rust-db.cmd
```

Run when auth protocol behavior changes:

```powershell
.\scripts\test-auth-flow.cmd
```

Expected local services:

- MariaDB container: `cmangos-rust-realmd`
- DB port: `127.0.0.1:3307`
- Smoke-test auth port: `127.0.0.1:13724`
- World skeleton local config port: `127.0.0.1:8085` (blocked on the current
  Windows machine; `WORLD_BIND_PORT=18085` was used for process smoke testing)
- Manual client-stack helper creates a `characters` schema in the same MariaDB
  container and grants the `mangos` user access.

CI currently runs the Rust workflow on pushes to this branch and pull requests
targeting `master`.

## Reference Paths

- Auth command codes: `src/realmd/AuthCodes.h`
- Auth session behavior: `src/realmd/AuthSocket.cpp`
- World session bootstrap: `src/game/Server/WorldSocket.cpp`
- Realm list behavior: `src/realmd/RealmList.*`
- Login schema: `sql/base/realmd.sql`
- Rust auth notes: `docs/rust_auth_foundation.md`

## Agent Handoff Protocol

At the end of meaningful work, update `docs/session_handoff.md` with:

- branch and latest commit,
- what changed,
- exact tests run and results,
- current blockers,
- recommended next task,
- files most likely relevant next.

New AI agents should start by reading, in order:

1. `docs/session_handoff.md`
2. `docs/rust_migration_plan.md`
3. `docs/rust_auth_foundation.md`
4. `git status --short --branch`
5. the C++ reference files for the active milestone

## Open Technical Risks

- SRP verifier byte order is now proven through the local compatibility test
  client; it still needs validation against a real 1.12.1 client.
- The authserver now proves successful TCP login and realm-list flow against
  local DB fixtures, including common negative/failure cases.
- CMaNGOS schema variants may differ across forks; keep DB queries close to
  `sql/base/realmd.sql` unless a migration is explicitly added.
- Character enum packet shape is source-derived and unit-tested; DB-backed
  character select, creation, enter-world, logout, and position persistence
  have now been manually proven with the real client.
- Movement is decoded into in-memory session state and persisted on
  logout/disconnect, but it is not yet validated, broadcast, or backed by full
  map/physics/anticheat behavior.
- Character creation is schema-compatible enough for the current enum/login
  path, but not yet full CMaNGOS `Player::Create` parity. Starter spells,
  skills, items/equipment, and action bars now have a first source-derived
  bridge; stats, DBC-backed appearance validation, broader item visual metadata,
  and fuller create-info parity remain open.
- Future worldserver work will need a strict packet compatibility harness before
  gameplay code grows.
