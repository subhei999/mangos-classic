# Session Handoff

Update this file before ending any substantial Rust migration session.

## Current Branch

- Branch: `codex/rust-auth-foundation`
- Latest pushed commit: this commit (run git log -1 --oneline for exact hash)
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
- Added TCP auth compatibility harness: `bins/auth-flow-test`.
- Added auth flow script: `scripts/test-auth-flow.cmd`.
- Proved seeded-account SRP challenge/proof and realm-list over TCP.

## Tests Last Run

Passing locally:

```powershell
.\scripts\test-rust.cmd
.\scripts\test-rust-db.cmd
.\scripts\test-auth-flow.cmd
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

Expand auth compatibility harness failure coverage:

- Unknown account should return the expected CMaNGOS failure packet.
- Bad proof should fail without authenticating.
- Banned account should return the expected banned response.
- Unsupported build behavior should match `src/realmd/AuthSocket.cpp`.

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
- `bins/auth-flow-test/src/main.rs`
- `scripts/test-auth-flow.ps1`

## Known Blockers

- Full real-client login has not been proven yet.
- SRP byte-order compatibility is proven with the local TCP harness, but still
  needs validation against a real 1.12.1 client.
- Port `3724` was blocked on this Windows machine during local smoke testing,
  so local config uses `13724`.
