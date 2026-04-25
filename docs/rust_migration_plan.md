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
- Local character lifecycle DB smoke entrypoint:
  `scripts/test-character-lifecycle.cmd`
- Local packet-level world flow entrypoint: `scripts/test-world-flow.cmd`
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
   - Current status: `CMSG_CHAR_DELETE` is implemented and manually proven.
     A first Docker-backed `character-lifecycle-test` now creates a Human
     Warrior through the Rust DB path, verifies enum visibility, refreshes the
     `realmcharacters` count, verifies starter inventory, deletes the
     character, and verifies count/item cleanup. A packet-level
     `world-flow-test` now authenticates through the Rust authserver, completes
     world `CMSG_AUTH_SESSION`, sends `CMSG_CHAR_ENUM`, creates a Human
     Warrior through `CMSG_CHAR_CREATE`, verifies enum/count/starter inventory
     refresh, covers duplicate name, invalid name, invalid race/class,
     character-limit, malformed delete, cross-account delete, and guild leader
     delete failures, deletes a non-leader guild member through
     `CMSG_CHAR_DELETE`, verifies `guild_member` / `guild_eventlog` cleanup,
     verifies group membership cleanup plus group-leader transfer and
     `group_instance` bind preservation, verifies social cleanup, pet child
     cleanup, basic received-mail cleanup, COD mail/item return-to-sender, and
     auction cleanup, loaded-character delete rejection, config-dependent
     hard-delete versus unlink/soft-delete behavior, broader race/class
     starter cleanup, and verifies enum/count refresh.
   - Current status: milestone complete. Remaining deeper group/LFG/reset
     nuance and full auction/mail gameplay behavior should be handled in future
     subsystem milestones rather than blocking Character Lifecycle Coverage.
10. Gameplay slices
   - Movement, chat, inventory, combat, spells, NPCs, loot, groups, guilds.
   - Each slice gets packet tests and DB fixture coverage.
11. Slamrock/Hardcore fork behavior
   - Port fork-specific mechanics after baseline classic behavior has coverage.

## Checkpoint Roadmap

The numbered milestones above track implementation history and near-term
vertical slices. The checkpoints below describe demo-quality product states.
Each checkpoint should be proven with a real WoW 1.12.1 client plus scripted
packet/DB harness coverage where practical.

### Checkpoint 1: First Playable World

Goal: the Rust server supports the first real playable loop in the WoW client,
not just authentication, character lifecycle, and a skeletal empty-world login.

Success looks like:

- A real client can authenticate, create/select/delete characters, enter world,
  move, logout, relog, and preserve character state.
- Multiple Classic race/gender combinations render with correct in-world body
  display ids.
- New characters have believable starter state: spawn position, bind point,
  race/class metadata, starter spells, action buttons, skills, starter items,
  equipment visuals, health, power, stats, faction/reputation basics, and
  cinematic/tutorial flags close enough that the client behaves normally.
- Startup packets are quiet and CMaNGOS-shaped enough that the client is not
  repeatedly probing missing account data, tutorial, time, raid, battleground,
  name, faction, or world-state data.
- The player can do first-minute gameplay: move, chat, see/query a simple NPC,
  open a basic gossip/vendor/trainer interaction, fight a simple creature,
  cast at least one starter spell, loot money/items, equip or move starter
  inventory, logout, and relog with durable DB state.
- Scripted tests protect the core loop: auth flow, character lifecycle, world
  character-screen flow, player bootstrap packet shapes, movement persistence,
  basic chat, basic inventory, basic NPC interaction, basic combat/spell, and
  basic loot.

Detailed path:

1. Real-client smoke gate
   - Relaunch with `scripts/run-client-stack-18085.cmd`.
   - Verify auth, enum, create/select, enter world, movement, logout/relog,
     non-loaded delete, and non-human display ids.
   - Capture any fresh client startup opcodes that still look noisy or missing.
2. Player object update parity
   - Expand the minimal `SMSG_UPDATE_OBJECT` self-spawn toward CMaNGOS player
     fields: health, power, max values, stats, faction, bytes, flags, display
     ids, scale, level, money, inventory slots, visible equipment, attack
     speeds, and aura placeholders.
   - Keep field indexes source-derived from `src/game/Entities/Player.cpp` and
     update-field definitions.
   - Add packet-builder tests for representative race/class/gender entries.
3. Starter/default parity completion
   - Replace remaining hardcoded Human Warrior assumptions with DB/DBC-backed
     or source-derived data.
   - Cover health/power/stat initialization, class power type, skill ranges,
     cinematic flags, tutorial flags, homebind, race/class validation, and
     item visual metadata from `item_template` or a shared world-data cache.
   - Expand the lifecycle matrix only after each golden case is proven.
4. Bootstrap packet polish
   - Make account data, tutorial, bind point, time, name query, zone update,
     raid info, battlefield status, GM ticket, active mover, faction/reputation
     init, mail timing, and world-state responses closer to CMaNGOS.
   - Add a packet smoke harness that logs and asserts the expected startup
     sequence after `CMSG_PLAYER_LOGIN`.
5. Movement v1
   - Handle observed walk/run/turn/jump/fall/swim movement packets without
     disconnects.
   - Persist position on logout/disconnect and reject obviously invalid
     character ownership/session cases.
   - Defer full anticheat and map collision, but keep the API ready for it.
6. Chat v1
   - Implement say/yell/whisper/system-message basics with language/faction
     checks sufficient for solo local testing.
   - Add packet tests for accepted/rejected message shapes.
7. World module cleanup gate
   - Completed as a mechanical, no-behavior-change split before real item
     movement: `world/mod.rs` now includes focused `bootstrap.rs`,
     `interactions.rs`, `wire.rs`, and `tests.rs` files.
   - The split is include-based for now so visibility and behavior stay
     unchanged while future Checkpoint 1 slices become easier to review.
   - Verified with `test-rust.cmd` and `test-world-flow.cmd`.
8. Inventory v1
   - Support item query responses, basic equip/unequip, bag/backpack moves,
     destroy item, stack counts, durability fields, and DB persistence.
   - Keep item operations conservative and schema-compatible with
     `characters.item_instance` / `character_inventory`.
   - Current status: first backpack move slice is implemented for present
     bag-0 backpack items via `CMSG_SWAP_INV_ITEM` and bag-0 `CMSG_SWAP_ITEM`.
     Rust persists slot swaps in `character_inventory`, refreshes session
     inventory, and sends changed player inventory-slot fields in
     `SMSG_UPDATE_OBJECT`. `test-world-flow.cmd` proves moving hearthstone
     `6948` from slot 24 to 26 and back persists in the DB.
   - Current status: basic equip/unequip is implemented for starter equipment
     slots 3, 6, 7, 15, and 16 using `item_template.InventoryType` validation.
     Rust persists the move, refreshes `characters.equipmentCache`, and sends
     changed inventory plus visible-equipment fields. `test-world-flow.cmd`
     proves moving shirt `38` from slot 3 to backpack slot 26 and back persists
     in both inventory and equipment cache. Bag containers, destroy,
     split/stacking, durability, and full class/race/equipment validation remain
     future Inventory v1 slices.
9. NPC interaction v1
   - Spawn/query a tiny fixture set of creatures/gameobjects from the world DB
     or a controlled test fixture.
   - Implement enough `CMSG_CREATURE_QUERY`, `CMSG_GAMEOBJECT_QUERY`,
     gossip hello, vendor list, and trainer list for a real client to open
     simple interactions.
10. Combat and spell v1
   - Implement target selection, auto-attack start/stop, swing timing basics,
     health updates, death/respawn basics, and one or two starter instant spell
     casts.
   - Keep combat deterministic in harness tests before adding broader spell
     mechanics.
11. Loot v1
    - Support opening loot, taking money/items, updating inventory, and
      persisting creature loot state enough for a single-player demo.
12. First Playable demo pass
    - Run the real client through the full loop on a fresh account/character.
    - Run `test-rust.cmd`, `test-auth-flow.cmd`,
      `test-character-lifecycle.cmd`, `test-world-flow.cmd`, and any new
      Checkpoint 1 harnesses.
    - Update `docs/session_handoff.md` with exactly what was demonstrated and
      what remains outside Checkpoint 1.

### Checkpoint 2: Starter Zone Playability

Goal: one starter zone can be played as a coherent early-game experience rather
than a handpicked interaction demo.

Success looks like:

- A chosen starter zone has creature spawns, gameobjects, vendors, trainers,
  graveyard/respawn flow, loot tables, basic aggro/leashing, and enough class
  spells to play several opening minutes.
- A player can accept, progress, and complete a small set of starter quests.
- Creature respawn, XP gain, level-up basics, money, inventory, and trainer
  learning persist correctly.
- The implementation uses CMaNGOS DB/script data where possible, with explicit
  fixture shortcuts documented.

### Checkpoint 3: Core Solo Leveling Loop

Goal: the Rust server supports the ordinary solo PvE loop across multiple low
level areas.

Success looks like:

- Quest, creature, gameobject, loot, vendor, trainer, XP, level-up, rest,
  durability, death, resurrection, and hearthstone flows are functional.
- More class starter spell families work, including cooldowns, resource costs,
  aura application/removal, and simple periodic effects.
- Movement, visibility, and object updates are stable enough for longer client
  sessions without frequent protocol surprises.

### Checkpoint 4: Social And Economy Basics

Goal: the world feels persistent and multiplayer-capable beyond solo combat.

Success looks like:

- Friends/ignore, whispers, channels, party invite/leave/leadership, guild
  roster/chat basics, mail send/receive/COD, auction browsing/bid/buyout, and
  trade-window basics work with durable DB state.
- Character lifecycle cleanup remains compatible with these systems.
- Multi-client smoke tests prove at least two real clients can see each other,
  chat, group, trade, and persist state.

### Checkpoint 5: Dungeons And Group PvE

Goal: grouped PvE works well enough for a small dungeon-style demo.

Success looks like:

- Instance creation/binding/reset basics, group visibility, elite creature
  combat, threat, loot methods, party XP, corpse/ghost flow, and basic boss
  scripting are functional.
- Pathing/collision is good enough for controlled dungeon fixtures, with known
  gaps documented.

### Checkpoint 6: Broad Classic Systems Coverage

Goal: most major Classic systems have faithful Rust coverage, even if long-tail
edge cases remain.

Success looks like:

- Talents, trainers, professions, banks, pets, transports/taxis, reputations,
  battleground queue/status basics, weather/time, GM commands needed for
  testing, and broader spell/aura families are implemented with packet and DB
  tests.
- Real-client regression passes cover several races/classes and multiple zones.

### Alpha: Feature-Complete CMaNGOS Parity

Goal: Rust can be treated as a feature-complete replacement candidate for the
Classic CMaNGOS gameplay surface, with known bugs rather than known missing
subsystems.

Success looks like:

- Auth, character lifecycle, world bootstrap, movement, chat, inventory, items,
  spells, auras, combat, creatures, gameobjects, quests, loot, groups, guilds,
  mail, auction house, trade, vendors, trainers, pets, taxis/transports,
  instances, battleground basics, GM/admin operations, persistence, and cleanup
  semantics are implemented against CMaNGOS-compatible schemas.
- Existing CMaNGOS data can boot without Rust-specific migrations beyond
  documented compatibility configuration.
- Automated tests include unit tests, packet-shape tests, DB fixture tests,
  multi-client smoke tests, and long-running real-client regression scripts.
- Remaining gaps are tracked as bugs or fidelity differences, not whole missing
  feature areas.

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

Run when character create/delete/count-refresh behavior changes:

```powershell
.\scripts\test-character-lifecycle.cmd
```

Run when authenticated world character-screen packet behavior changes:

```powershell
.\scripts\test-world-flow.cmd
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

Keep `docs/session_handoff.md` as a concise current-session operating brief,
not a running changelog. Target about 120 lines and keep it under 180 lines.
When handoff detail becomes durable history, summarize it here in the roadmap
or replace it with a one-line pointer to the relevant milestone section.
Prune stale test narratives, old manual-smoke transcripts, and completed
implementation logs instead of appending indefinitely.

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
- Character Lifecycle Coverage is closed for the current milestone, with
  packet-level happy-path, negative create/delete, loaded/guild leader
  rejection, guild/group/social/pet/mail/auction cleanup, COD mail return,
  config-dependent soft-delete/unlink, and broader race/class cleanup coverage.
- Future worldserver work will need a strict packet compatibility harness before
  gameplay code grows.
