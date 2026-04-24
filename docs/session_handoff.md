# Session Handoff

Update this file before ending any substantial Rust migration session.

## Current Branch

- Branch: `codex/rust-auth-foundation`
- Latest pushed commit: `d481038fd`
- Remote: `origin/codex/rust-auth-foundation`

## Current Goal

Make the Rust authserver foundation testable and ready for long-running,
multi-session development.

## What Changed Recently

- Added Rust workspace for authserver foundation.
- Added local Rust test script: `scripts/test-rust.cmd`.
- Added Docker-backed MariaDB smoke test: `scripts/test-rust-db.cmd`.
- Added local DB config: `config/authserver.local.toml`.
- Fixed Rust DB mappings to match `sql/base/realmd.sql`.
- Added this long-running migration map: `docs/rust_migration_plan.md`.

## Tests Last Run

Passing locally:

```powershell
.\scripts\test-rust.cmd
.\scripts\test-rust-db.cmd
```

The DB smoke test starts MariaDB through Docker and verifies the Rust authserver
can start against it.

## Local Environment Notes

- Rust installed with rustup.
- Docker Desktop is installed and was started successfully.
- MariaDB test container name: `cmangos-rust-realmd`.
- MariaDB local port: `3307`.
- Authserver local smoke-test port: `13724`.

Stop the local DB container with:

```powershell
docker compose -f docker-compose.local.yml down
```

## Next Recommended Task

Build an auth compatibility harness:

- Add a small Rust integration test or test client that connects to the
  authserver over TCP.
- Seed/use a known account from `sql/base/realmd.sql`.
- Verify the login challenge response, proof response, and realm-list response.
- Keep this test runnable from `scripts/test-rust-db.cmd` or a new
  `scripts/test-auth-flow.cmd`.

## Key Files

- `src/realmd/AuthSocket.cpp`
- `src/realmd/AuthCodes.h`
- `src/realmd/RealmList.h`
- `sql/base/realmd.sql`
- `bins/authserver/src/main.rs`
- `crates/wow-network/src/auth/session.rs`
- `crates/wow-proto/src/auth_packets.rs`
- `crates/wow-crypto/src/srp.rs`
- `crates/wow-db/src/account.rs`
- `crates/wow-db/src/realm.rs`

## Known Blockers

- Full real-client login has not been proven yet.
- SRP byte-order compatibility still needs a TCP-level auth flow test.
- Port `3724` was blocked on this Windows machine during local smoke testing,
  so local config uses `13724`.
