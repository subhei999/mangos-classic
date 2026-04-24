# Rust Auth Foundation

This branch adds a parallel Rust authserver workspace while leaving the C++
CMaNGOS server untouched.

For the long-running migration map and AI-agent handoff protocol, see
`docs/rust_migration_plan.md` and `docs/session_handoff.md`.

## Local configuration

The Rust authserver reads TOML configuration from `config/authserver.toml` by
default:

```powershell
cargo run -p authserver -- --config config/authserver.toml
```

Environment variables with the `AUTH_` prefix override TOML values. Nested keys
use double underscores, for example:

```powershell
$env:AUTH_DATABASE__HOST = "127.0.0.1"
$env:AUTH_DATABASE__DATABASE = "realmd"
```

## Database expectations

The first milestone targets the existing CMaNGOS `realmd` schema on
MySQL/MariaDB. The auth path currently reads:

- `account`: `id`, `username`, `gmlevel`, `sessionkey`, `v`, `s`, and related
  account metadata.
- `account_banned`: active account bans.
- `realmlist`: realm display and network endpoint data.
- `realmcharacters`: per-account character counts per realm.

No migration is required for this branch. It expects the normal CMaNGOS schema
and SRP verifier/salt columns to already exist.

## Verification

Once Rust is installed, run:

```powershell
.\scripts\test-rust.cmd
```

The included unit tests cover auth packet round trips, SRP challenge creation,
configuration defaults, and database helper behavior that does not require a
live database.

The same checks also run in GitHub Actions through the `Rust Auth Foundation`
workflow on pushes to this branch and on pull requests targeting `master`.

For a local database smoke test, start Docker Desktop and run:

```powershell
.\scripts\test-rust-db.cmd
```

This starts MariaDB on local port `3307`, initializes it with
`sql/base/realmd.sql`, and verifies the Rust authserver can start on
`127.0.0.1:13724` with `config/authserver.local.toml`. Use
`.\scripts\test-rust-db.cmd -KeepRunning` to leave the authserver process
attached for manual client testing.
