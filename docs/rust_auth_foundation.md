# Rust Auth Foundation

This branch adds a parallel Rust authserver workspace while leaving the C++
CMaNGOS server untouched.

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
.\scripts\test-rust.ps1
```

The included unit tests cover auth packet round trips, SRP challenge creation,
configuration defaults, and database helper behavior that does not require a
live database.

The same checks also run in GitHub Actions through the `Rust Auth Foundation`
workflow on pushes to this branch and on pull requests targeting `master`.
