# Session Handoff

Update this file before ending any substantial Rust migration session.

## Current Branch

- Branch: `codex/rust-auth-foundation`
- Latest commit: `fac4f2ff7`
- Uncommitted session changes: character lifecycle coverage in progress; not
  committed yet.
- Remote: `origin/codex/rust-auth-foundation`

## Current Goal

Expand Character Lifecycle Coverage while keeping the client-proven create ->
enum -> enter world -> move -> logout/delete path intact.

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
- Added world-session state for the active character, CMaNGOS-shaped movement
  packet decoding, in-memory position/flag/time updates for observed movement
  opcodes, and minimal logout request/cancel handling back to character select.
- Added DB persistence for the active character position on logout/disconnect.
  Manual real-client testing proved `Rustone` logs back in at the position
  where he logged out.
- Added a minimal `CMSG_CHAR_CREATE` happy path and common failure result
  handling. The worldserver now parses the 1.12.1 create packet, normalizes and
  validates names/race/class/gender, rejects duplicate names and realm limits,
  inserts a CMaNGOS-schema `characters` row plus `character_homebind`, updates
  `realmd.realmcharacters`, and returns `SMSG_CHAR_CREATE`.
- Added low-risk cleanup for common post-login probes: `CMSG_NAME_QUERY` now
  returns `SMSG_NAME_QUERY_RESPONSE`, `CMSG_QUERY_TIME` returns server time,
  `CMSG_REQUEST_ACCOUNT_DATA` returns an empty account-data response,
  `CMSG_GMTICKET_GETTICKET` returns no-ticket status, account-data updates are
  explicitly ignored, and known bootstrap chatter is logged as expected instead
  of unhandled warnings.
- Completed the first real-client CMaNGOS starter-default parity slice for newly
  created Human Warriors:
  - `worldserver` now opens a separate world DB pool and local config points it
    at the Docker `mangos` database.
  - `wow_db::create_character` now reads `playercreateinfo` from the world DB
    instead of using hardcoded Rust fallback spawn data.
  - New characters now get starter rows in `character_spell`,
    `character_action`, and `character_skills` from
    `playercreateinfo_spell`, `playercreateinfo_action`, and
    `playercreateinfo_skills`.
  - Login bootstrap now loads `character_spell` and `character_action` and
    sends non-empty CMaNGOS-shaped `SMSG_INITIAL_SPELLS` and
    `SMSG_ACTION_BUTTONS`.
  - Fixed Human Warrior starter action bar visibility by setting Battle Stance
    in `UNIT_FIELD_BYTES_1` during the minimal self-spawn update; CMaNGOS
    stores warrior starter buttons on stance-bar slots `72`, `73`, and `83`.
  - Added first starter outfit/item slice:
    - source-derived starter item rows from archived CMaNGOS
      `playercreateinfo_item` data,
    - new `item_instance` and `character_inventory` rows for newly created
      characters,
    - `equipmentCache` item/enchant pairs for character enum,
    - Human Warrior enum visual metadata for starter shirt/pants/boots/sword
      and shield,
    - visible equipped item update fields in the minimal self-spawn packet,
    - equipment/backpack item GUID update fields in the minimal self-spawn
      packet,
    - minimal `UPDATETYPE_CREATE_OBJECT` item blocks appended to the login
      update so the client receives owned item objects for starter inventory.
  - `scripts/run-client-stack-18085.ps1` now creates/imports the local `mangos`
    schema when needed so the worldserver can read starter defaults.
- Fixed Rust worldserver delete count refresh to update
  `realmcharacters(realmid=1, acctid=<account>)`; the delete path previously
  passed realm/account arguments in the wrong order.
- Added `wow_db::refresh_realm_character_count` so create/delete share one
  source of truth for character count refresh.
- Added `bins/character-lifecycle-test` plus
  `scripts/test-character-lifecycle.cmd` / `.ps1`. The Docker-backed smoke
  creates a Human Warrior through the Rust DB path, verifies enum visibility,
  verifies `realmcharacters` count and absence of the reversed row, verifies
  starter hearthstone/weapon inventory, deletes the character, then verifies
  count and starter item cleanup.
- Added `bins/world-flow-test` plus `scripts/test-world-flow.cmd` / `.ps1`.
  The packet-level smoke seeds and authenticates an SRP account through the
  Rust authserver, connects to Rust worldserver, completes
  `CMSG_AUTH_SESSION`, sends `CMSG_CHAR_ENUM`, creates `Worldlife` through
  `CMSG_CHAR_CREATE`, verifies `SMSG_CHAR_CREATE`, enum visibility,
  `realmcharacters` count, and starter inventory, deletes through
  `CMSG_CHAR_DELETE`, then verifies the final enum/count cleanup.
- Expanded `world-flow-test` with negative character-screen packet coverage:
  one-character-too-short name, invalid-character name, invalid race/class
  combo, duplicate name, character-limit response, malformed delete packet, and
  deleting another account's character. These cases assert response codes and
  count/ownership invariants.
- Added guild leader delete parity: Rust worldserver now rejects
  `CMSG_CHAR_DELETE` when `guild.leaderguid` matches the selected character,
  mirroring CMaNGOS `sGuildMgr.GetGuildByLeader(guid)`. `world-flow-test`
  seeds a temporary guild/guild_member row, asserts delete failure, verifies
  the character remains in enum, and checks count stability.
- Added non-leader guild delete cleanup parity: `wow_db::delete_character`
  removes `guild_member` rows for the deleted character and `guild_eventlog`
  rows where the character appears as `PlayerGuid1` or `PlayerGuid2`.
  `world-flow-test` seeds those rows before the successful delete and asserts
  they are gone afterward.
- Updated `scripts/test-world-flow.ps1` to tear down child `authserver.exe` and
  `worldserver.exe` processes after the smoke test finishes.

## Tests Last Run

Passing locally:

```powershell
$env:PATH = "$env:USERPROFILE\.cargo\bin;$env:PATH"; .\scripts\test-rust.cmd
.\scripts\test-rust-db.cmd
```

Previously passing locally:

```powershell
.\scripts\test-rust-db.cmd
.\scripts\test-auth-flow.cmd
```

World stack helper smoke after starter-default slice:

```powershell
.\scripts\run-client-stack-18085.cmd
```

Verified logs:

```text
Authserver listening on 127.0.0.1:13724
Configuration loaded bind=127.0.0.1:18085 login_database=realmd character_database=characters world_database=mangos
World server listening on 127.0.0.1:18085
```

Real-client starter-default smoke:

```text
Fresh Human Warrior created through the WoW 1.12.1 client.
Spellbook showed the expected starter spells.
Initial action bar was empty before the Battle Stance self-spawn fix.
After restarting the patched stack and re-entering the character, the starter
action bar looked good.
Follow-up client smoke confirmed pants, boots, hearthstone, and gift voucher
visible after the inventory GUID and item create-block fixes.
```

Starter outfit stack smoke:

```text
.\scripts\run-client-stack-18085.cmd
Authserver listening on 127.0.0.1:13724
World server listening on 127.0.0.1:18085
```

Unit/build coverage added for Human Warrior starter outfit rows,
`equipmentCache` formatting, equipment-cache parsing, and Human Warrior
starter visual metadata. User real-client smoke confirmed pants/boots after the
equipment/backpack GUID update-field fix and confirmed hearthstone plus the
starter gift voucher after minimal owned item create blocks were added during
self-spawn.

Inventory GUID follow-up:

```powershell
cargo test -p wow-db -p wow-network
$env:PATH = "$env:USERPROFILE\.cargo\bin;$env:PATH"; .\scripts\test-rust.cmd
```

Both passed locally after adding `character_inventory` loading for login,
player update-field coverage for equipment/backpack slots, and minimal item
create blocks with owner/contained GUIDs, item entry, stack count, and
durability.

Character lifecycle coverage:

```powershell
$env:PATH = "$env:USERPROFILE\.cargo\bin;$env:PATH"; .\scripts\test-rust.cmd
$env:PATH = "$env:USERPROFILE\.cargo\bin;$env:PATH"; .\scripts\test-character-lifecycle.cmd
$env:PATH = "$env:USERPROFILE\.cargo\bin;$env:PATH"; .\scripts\test-rust-db.cmd
$env:PATH = "$env:USERPROFILE\.cargo\bin;$env:PATH"; .\scripts\test-world-flow.cmd
```

All passed locally after stopping stale `authserver.exe` / `worldserver.exe`
processes that were holding `target\debug\authserver.exe`. Docker-backed tests
needed elevated Docker access in the Codex app shell. The lifecycle smoke
printed:

```text
character lifecycle check passed: create, enum, count refresh, starter items, delete cleanup
world flow check passed: auth session, create/delete happy path, negative create/delete cases, guild leader rejection, guild cleanup, enum/count refresh
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

Movement/logout persistence manual client test passed:

```text
Rustone moved away from the seed spawn, logged out instantly to character
select, then entered world again at the logged-out position. World log shows
movement updates, SMSG_LOGOUT_RESPONSE/SMSG_LOGOUT_COMPLETE flow, and
"Persisted character position" for guid=1.
```

Character creation manual client test passed:

```text
WoW 1.12.1 client created a new Human Warrior named Rusttwo.
World log shows CMSG_CHAR_CREATE bytes=17, Created character guid=2 name=Rusttwo
race=1 class=1 count=2, followed by CMSG_CHAR_ENUM count=2. Rusttwo appeared
on the character list, entered the world, moved, logged out, and persisted
position like Rustone.
```

Post-login probe cleanup:

```text
Unit-tested after implementation. Not yet rerun through the real client after
the cleanup patch, but the previous real-client logs identified the covered
opcodes: CMSG_NAME_QUERY, CMSG_ZONEUPDATE, CMSG_UPDATE_ACCOUNT_DATA,
CMSG_GMTICKET_GETTICKET, CMSG_QUERY_TIME, CMSG_SET_ACTIVE_MOVER,
CMSG_REQUEST_RAID_INFO, CMSG_BATTLEFIELD_STATUS, tutorial flags, and related
startup chatter.
```

Note: the app shell did not have Cargo on PATH, so the workspace test was run
with Cargo from `%USERPROFILE%\.cargo\bin`. The project scripts pass after
prepending that directory to PATH.

Note: Docker access needed elevated approval in the app shell. The initial
non-elevated DB smoke failed with access denied on Docker config/pipe, then
passed when rerun with elevated Docker access.

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

Next recommended milestone: Character Lifecycle Coverage.

- Important: character deletion is not complete CMaNGOS parity yet. Current
  coverage proves happy-path packet create/delete, negative character-screen
  responses, guild leader rejection, and guild member/eventlog cleanup only.
  Keep documenting delete behavior as partial until the remaining gaps below
  are resolved.
- Exercise fuller CMaNGOS delete semantics: loaded character rejection, group
  cleanup, social cleanup, mail/item cleanup, and config-dependent hard-delete
  versus unlink behavior.
- Broaden lifecycle coverage beyond the first Human Warrior happy path,
  especially starter equipment/backpack cleanup for more race/class rows.
- Keep the client-proven vertical path intact: create character -> enum refresh
  -> enter world -> starter gear/items visible -> move -> logout/delete.

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
- `bins/character-lifecycle-test/src/main.rs`
- `bins/world-flow-test/src/main.rs`
- `bins/worldserver/src/main.rs`
- `scripts/test-auth-flow.ps1`
- `scripts/test-character-lifecycle.ps1`
- `scripts/test-world-flow.ps1`
- `scripts/run-auth-client-13724.cmd`
- `scripts/run-client-stack-18085.ps1`
- `scripts/run-client-stack-18085.cmd`
- `config/worldserver.local.toml`
- `crates/wow-network/src/world/mod.rs`
- `sql/base/mangos.sql`
- `src/game/Globals/ObjectMgr.cpp`
- `src/game/Entities/Player.cpp`
- `auth-client-13724.log`
- `world-client-18085.log`

## Known Blockers

- Real WoW 1.12.1 client login/authentication is proven for `RUSTAUTH`, build
  `5875`, through the Rust authserver on `127.0.0.1:13724`.
- Packet behavior is checked against C++ source-derived shapes, but not yet
  against a live CMaNGOS `realmd` capture.
- Enter-world is intentionally skeletal. Movement packets update in-memory
  session state and logout/disconnect persists position to the database, but
  broader world state, validation, and delayed logout semantics are not yet
  implemented.
- Character creation now persists starter spells, starter skills, and action
  buttons from the CMaNGOS world DB tables. A real-client Human Warrior smoke
  confirmed starter spellbook entries and visible starter action bar after the
  Battle Stance update-field fix.
- Starter inventory/equipment now has a first implementation, but it is only
  unit/build and stack-start tested after the latest inventory GUID fix. It
  needs real-client visual verification. The enum visual bridge currently
  covers Human Warrior starter gear metadata;
  the DB rows are source-derived for all classic race/class starter outfits,
  but broader enum visual metadata should come from `item_template` or a shared
  world-data cache instead of hardcoded item visuals.
- Exact DBC skill ranges, health/power/stat initialization, cinematic flags,
  and DBC-backed appearance validation remain open.
- Character delete parity remains partial. Known remaining gaps: group cleanup
  (`group_member`, `group_instance`, leader behavior), explicit social cleanup
  tests, CMaNGOS mail/COD/item-return behavior, pet cleanup coverage,
  auction-related cleanup, config-dependent hard-delete versus
  unlink/soft-delete behavior, and loaded-character delete rejection once
  online character tracking exists.
- Post-login probe cleanup is unit-tested, but should still get one quick
  real-client smoke pass at the start of the next session.
- The minimal self-spawn `SMSG_UPDATE_OBJECT` is enough for the real client to
  enter world, but it is not a complete CMaNGOS player object update yet.
- Port `8085` is blocked on this Windows machine with socket error `10013`.
  Manual world testing needs a local alternate port and matching `realmlist`
  row update, or an OS-level port fix.
- Port `3724` was blocked on this Windows machine during local smoke testing,
  so local config uses `13724`.
