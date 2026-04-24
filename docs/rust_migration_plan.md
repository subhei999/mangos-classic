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
- Rust status: authserver foundation exists and builds locally
- Local unit/lint/build entrypoint: `scripts/test-rust.cmd`
- Local MariaDB smoke entrypoint: `scripts/test-rust-db.cmd`
- Local DB config: `config/authserver.local.toml`
- Docker DB harness: `docker-compose.local.yml`

## Crate Map

- `bins/authserver`
  - Runnable Rust login/auth server.
  - Owns CLI parsing, config loading, tracing setup, DB pool creation, and
    server startup.
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
  - Current focus is `realmd`: accounts, bans, realms, character counts.
- `crates/wow-network`
  - Async TCP servers and per-connection session state machines.
  - Current focus is auth handshake and realm-list flow.
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
   - Add scripted test client or captured-packet tests for 1.12.1 login flow.
   - Verify successful login and known failure responses against seeded DB data.
3. Worldserver skeleton
   - Add `bins/worldserver` only after auth compatibility is stable.
   - Accept world TCP connections and perform header/session bootstrap.
4. Character list vertical slice
   - Load account session key from `realmd`.
   - Read character database enough to answer character list requests.
5. Enter-world vertical slice
   - Load player, map, position, and minimum update packets.
   - Client can enter a static world state.
6. Gameplay slices
   - Movement, chat, inventory, combat, spells, NPCs, loot, groups, guilds.
   - Each slice gets packet tests and DB fixture coverage.
7. Slamrock/Hardcore fork behavior
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

Expected local services:

- MariaDB container: `cmangos-rust-realmd`
- DB port: `127.0.0.1:3307`
- Smoke-test auth port: `127.0.0.1:13724`

CI currently runs the Rust workflow on pushes to this branch and pull requests
targeting `master`.

## Reference Paths

- Auth command codes: `src/realmd/AuthCodes.h`
- Auth session behavior: `src/realmd/AuthSocket.cpp`
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

- SRP verifier byte order must be proven against a real 1.12.1 client or a
  compatibility test client.
- The authserver currently proves startup against the DB, not a full login.
- CMaNGOS schema variants may differ across forks; keep DB queries close to
  `sql/base/realmd.sql` unless a migration is explicitly added.
- Future worldserver work will need a strict packet compatibility harness before
  gameplay code grows.
