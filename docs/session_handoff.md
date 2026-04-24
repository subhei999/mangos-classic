# Session Handoff

Update this file before ending any substantial Rust migration session.

## Current Branch

- Branch: `codex/rust-auth-foundation`
- Latest commit: `9c5fa0a7d Add TCP auth flow compatibility harness`
- Uncommitted session changes: expanded auth-flow negative coverage, schema
  compatibility fixes, real 1.12.1 client smoke-test support, worldserver
  skeleton, DB-backed character enum support, and the first enter-world
  skeleton.
- Remote: `origin/codex/rust-auth-foundation`

## Current Goal

Move from first real-client enter-world success into movement/world-session
hardening.

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
- Expanded `auth-flow-test` to cover unknown account, bad proof/no session key,
  banned account, pre-auth realm-list rejection, and unsupported build response.
- Matched unsupported-build proof behavior to CMaNGOS by returning
  `CMD_AUTH_LOGON_CHALLENGE` / `AUTH_LOGON_FAILED_VERSION_INVALID` and closing.
- Matched vanilla bad-proof response shape for build `5875` by returning the
  two-byte CMaNGOS-style `CMD_AUTH_LOGON_PROOF` failure response.
- Added minimal reconnect-challenge handling so unsupported reconnect attempts
  do not leave a real client waiting forever.
- Fixed `account_banned` model signedness to match `sql/base/realmd.sql`.
- Proved real WoW 1.12.1 client authentication against Rust authserver:
  `RUSTAUTH` / `RUSTPASS`, build `5875`, `realmlist.wtf` set to
  `127.0.0.1:13724`. The client authenticated successfully, then stopped before
  character screen because no worldserver is listening on realm port `8085`.
- Added helper script `scripts/run-auth-client-13724.cmd` to start the Rust
  authserver for manual client testing and log to `auth-client-13724.log`.
- Added `bins/worldserver` as the first Rust worldserver skeleton binary.
- Added `crates/wow-network::world` with a TCP listener, CMaNGOS-shaped
  `SMSG_AUTH_CHALLENGE`, `CMSG_AUTH_SESSION` parsing, login DB session-key
  loading, world auth digest verification, and initial `SMSG_AUTH_RESPONSE`
  auth-ok response.
- Added `config/worldserver.local.toml` for local worldserver smoke testing.
- Added encrypted world header handling after `CMSG_AUTH_SESSION`, matching
  CMaNGOS `AuthCrypt` behavior for post-auth world packets.
- Added `CMSG_CHAR_ENUM` handling with a valid empty `SMSG_CHAR_ENUM` response.
- Proved a real WoW 1.12.1 client reaches the character screen through Rust
  authserver plus Rust worldserver. Logs show verified `CMSG_AUTH_SESSION`,
  handled `CMSG_CHAR_ENUM`, then the client sending unimplemented
  `CMSG_CHAR_CREATE` (`0x0036`).
- Added `wow_db::character` query support for the CMaNGOS `characters`
  database and changed worldserver character enum to serialize DB-backed
  `SMSG_CHAR_ENUM` rows using the C++ `Player::BuildEnumData` field order.
- Updated local worldserver config to use a separate `characters` database for
  `character_database`.
- Added `scripts/run-client-stack-18085.cmd` / `.ps1` to start local MariaDB,
  point the realm row at unblocked world port `18085`, and run authserver plus
  worldserver for manual real-client testing.
- Updated the client-stack helper to create/import the `characters` schema,
  grant access to the `mangos` DB user, and seed `RUSTAUTH` with one visible
  test character named `Rustone` when the account exists.
- Added `CMSG_PLAYER_LOGIN` handling. The worldserver validates that the
  selected character belongs to the account and sends the initial
  `SMSG_LOGIN_VERIFY_WORLD` response.
- Added the early enter-world packet burst after character selection:
  `SMSG_ACCOUNT_DATA_TIMES`, `SMSG_BINDPOINTUPDATE`, `SMSG_TUTORIAL_FLAGS`,
  `SMSG_INITIAL_SPELLS`, `SMSG_ACTION_BUTTONS`,
  `SMSG_LOGIN_SETTIMESPEED`, and `SMSG_INIT_WORLD_STATES`.
- Added a minimal `SMSG_UPDATE_OBJECT` self-spawn packet for `Rustone`, using
  source-derived CMaNGOS update header/movement/update-mask structure.
- Proved the Enter World Skeleton milestone with a real WoW 1.12.1 client:
  `RUSTAUTH` can select `Rustone`, leave loading screen, enter the world, and
  walk around. Logs show movement opcodes arriving after self spawn.

## Tests Last Run

Passing locally:

```powershell
$env:PATH = "$env:USERPROFILE\.cargo\bin;$env:PATH"; .\scripts\test-rust.cmd
.\scripts\test-rust-db.cmd
.\scripts\test-auth-flow.cmd
```

The DB smoke test starts MariaDB through Docker and verifies the Rust authserver
can start against it.

Worldserver skeleton checks:

```powershell
cargo build -p worldserver
$env:WORLD_BIND_PORT = "18085"; target\debug\worldserver.exe --config config\worldserver.local.toml
.\scripts\run-client-stack-18085.cmd
```

The direct worldserver startup reached `World server listening on
127.0.0.1:18085`. Port `8085` is blocked locally with Windows socket error
`10013`, so the default realm port still needs a local workaround before manual
client world testing on this machine.

The client-stack helper was verified to start both services:

- Authserver: `127.0.0.1:13724`
- Worldserver: `127.0.0.1:18085`
- Realm row: `127.0.0.1:18085`

Manual client test also passed:

```text
WoW 1.12.1 client -> 127.0.0.1:13724
Account: RUSTAUTH
Password: RUSTPASS
Observed: Account 'RUSTAUTH' authenticated successfully
```

Follow-up manual client test reached character screen:

```text
World auth session verified account=RUSTAUTH account_id=5
Received world packet after auth opcode=0x0037 bytes=0
Received world packet after auth opcode=0x0036 bytes=12
```

After DB-backed enum work, `.\scripts\run-client-stack-18085.cmd` was rerun
successfully and Docker verified:

```text
characters.characters: guid=1 name=Rustone account=5 username=RUSTAUTH
realmd.realmcharacters: realmid=1 acctid=5 numchars=1
```

Enter-world manual client test passed:

```text
Account 'RUSTAUTH' authenticated successfully
World auth session verified account=RUSTAUTH account_id=5
Sending character enum account=RUSTAUTH count=1
Character login selected account_id=5 guid=1 name=Rustone map=0
Sending minimal self spawn update guid=1 name=Rustone
Received movement opcodes including MSG_MOVE_START_FORWARD, MSG_MOVE_STOP,
MSG_MOVE_SET_FACING, MSG_MOVE_HEARTBEAT, MSG_MOVE_JUMP, and CMSG_PING.
Observed in client: Rustone logged into the world and could walk around.
```

Note: the app shell did not have Cargo on PATH, so the workspace test was run
with Cargo from `%USERPROFILE%\.cargo\bin`. The project scripts pass after
prepending that directory to PATH.

## Local Environment Notes

- Rust installed with rustup.
- Docker Desktop is installed and was started successfully.
- MariaDB test container name: `cmangos-rust-realmd`.
- MariaDB local port: `3307`.
- Authserver local smoke-test port: `13724`.
- Worldserver local config port: `8085`, matching the seeded `realmlist` row,
  but this Windows machine blocks binding to `8085`; use
  `WORLD_BIND_PORT=18085` for local process smoke tests.
- Current manual client stack logs: `auth-client-13724.log` and
  `world-client-18085.log`.
- Real client auth test used `scripts/run-auth-client-13724.cmd`.
- Normal WoW auth port `3724` is blocked on this Windows machine with socket
  error `10013`, so manual testing uses `realmlist.wtf` value
  `set realmlist 127.0.0.1:13724`.

Stop the local DB container with:

```powershell
docker compose -f docker-compose.local.yml down
```

## Next Recommended Task

Harden the in-world skeleton:

- Decode and acknowledge/log movement packets as movement instead of generic
  unhandled warnings. Current observed opcodes include `0x00B5`,
  `0x00B7`, `0x00B8`, `0x00B9`, `0x00BA`, `0x00BB`, `0x00BD`,
  `0x00BE`, `0x00C9`, `0x00DA`, and `0x00EE`.
- Persist/update the in-memory character position from movement packets.
- Add minimal handlers for logout/exit-to-character-screen paths.
- After movement/logging is calmer, implement `CMSG_CHAR_CREATE` or start
  DB-backed character creation.

Optional auth follow-up: compare captured packet bytes against a live CMaNGOS
`realmd` run for extra confidence.

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
- `crates/wow-db/src/character.rs`
- `crates/wow-db/src/realm.rs`
- `bins/auth-flow-test/src/main.rs`
- `bins/worldserver/src/main.rs`
- `scripts/test-auth-flow.ps1`
- `scripts/run-auth-client-13724.cmd`
- `scripts/run-client-stack-18085.ps1`
- `scripts/run-client-stack-18085.cmd`
- `config/worldserver.local.toml`
- `crates/wow-network/src/world/mod.rs`
- `auth-client-13724.log`
- `world-client-18085.log`

## Known Blockers

- Real WoW 1.12.1 client login/authentication is proven for `RUSTAUTH`, build
  `5875`, through the Rust authserver on `127.0.0.1:13724`.
- Packet behavior is checked against C++ source-derived shapes, but not yet
  against a live CMaNGOS `realmd` capture.
- Enter-world is intentionally skeletal. The client can move, but movement
  packets are currently only logged as unhandled and no position is persisted.
- The minimal self-spawn `SMSG_UPDATE_OBJECT` is enough for the real client to
  enter world, but it is not a complete CMaNGOS player object update yet.
- Port `8085` is blocked on this Windows machine with socket error `10013`.
  Manual world testing needs a local alternate port and matching `realmlist`
  row update, or an OS-level port fix.
- Port `3724` was blocked on this Windows machine during local smoke testing,
  so local config uses `13724`.
