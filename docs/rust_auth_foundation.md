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
./scripts/test-rust.sh  # Linux/macOS
.\scripts\test-rust.cmd # Windows
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

For a TCP-level auth compatibility check, run:

```powershell
.\scripts\test-auth-flow.cmd
```

This seeds a known SRP account into the local `realmd` fixture, starts the Rust
authserver, completes logon challenge/proof over TCP, verifies the server proof,
requests the realm list, and covers common failure responses.

For manual real-client smoke testing on this Windows machine:

```powershell
.\scripts\run-auth-client-13724.cmd
```

Then set the WoW 1.12.1 client's `realmlist.wtf` to:

```text
set realmlist 127.0.0.1:13724
```

Use the seeded harness account `RUSTAUTH` / `RUSTPASS`. This has been proven to
authenticate a real build `5875` client through the Rust authserver. The client
does not reach character screen yet because the Rust worldserver skeleton is not
implemented and nothing listens on realm port `8085`.

For the current combined auth/world manual smoke path, run:

```powershell
.\scripts\run-client-stack-18085.cmd
```

This creates/imports the local `characters` schema when needed, seeds the
`RUSTAUTH` account with one visible test character named `Rustone`, updates the
local Docker `realmlist` row to `127.0.0.1:18085`, starts the authserver on
`127.0.0.1:13724`, and starts the worldserver skeleton on
`127.0.0.1:18085`. Keep the client `realmlist.wtf` pointed at:

```text
set realmlist 127.0.0.1:13724
```

Port `8085` is blocked on this Windows machine, so `18085` is used for manual
worldserver testing.

The real client has reached character select and entered the world through Rust
authserver and Rust worldserver. `Rustone` appears on the character screen, can
be selected, leaves the loading screen, and can walk around in a minimal empty
world. Movement packets are currently logged but not yet applied to persistent
world/session state.
