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
- Local unit/lint/build entrypoint: `scripts/test-rust.cmd` (Windows) or `scripts/test-rust.sh` (Linux/macOS)
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

Definition of done:

Checkpoint 1 is complete only when the final real-client grade is `PASS`, all
required automated scripts pass, and every remaining nearby gap is either fixed
or explicitly logged as P2/P3/P4 follow-up outside Checkpoint 1. Do not close
Checkpoint 1 from implementation status alone.

Required automated gate:

- `scripts/test-rust.cmd`
- `scripts/test-auth-flow.cmd`
- `scripts/test-character-lifecycle.cmd`
- `scripts/test-world-flow.cmd`
- Any newer Checkpoint 1 packet/DB harness added before the final pass.

Real-client grading pass:

Run `scripts/run-client-stack-18085.cmd` against a clean local DB fixture and a
WoW 1.12.1 build 5875 client. Record the result in `docs/session_handoff.md`
using `PASS`, `PARTIAL`, `FAIL`, or `DEFERRED` for each row. `PASS` means the
behavior was observed in the real client without disconnects, crashes, protocol
desync, or obviously corrupt DB state. `PARTIAL` means the main flow works but
has a visible limitation that must be linked to a GitHub issue before closure.
`DEFERRED` is allowed only when the behavior is intentionally moved to
Checkpoint 2+ and logged.

| Gate | Grade | Required observation |
| --- | --- | --- |
| Auth and realm list | PASS required | Login with `RUSTAUTH` / `RUSTPASS`, reach realm list, select the Rust realm. |
| Character screen | PASS required | Enum seeded and newly created characters; duplicate/invalid create failures do not break the screen. |
| Character create/select/delete | PASS required | Create a fresh character, enter world, logout to character select, delete a different non-loaded character, and see counts refresh. |
| Enter world and relog | PASS required | Fresh character leaves loading screen, appears in-world, moves, logs out, relogs, and keeps position/basic state. |
| Race/gender display ids | PASS required | At least Human male/female and one non-human male/female render with correct in-world body display ids. |
| Starter state | PASS required | Spellbook, action bar, skills UI, equipment visuals, backpack items, health, power, stats, money, bind point, tutorial/cinematic behavior, and faction basics are sane for the demo characters. |
| Startup packet quietness | PASS required | Server logs show no repeated unknown/missing startup opcode loop that affects the demo flow; any harmless probes are logged as follow-up issues. |
| Movement | PASS required | Walk, run, turn, jump/fall if observed, logout/disconnect persistence, and relog position work without disconnect. |
| Chat | PASS required | At least local say/yell or equivalent solo-visible chat path works and rejected forms fail cleanly. |
| NPC visibility/query | PASS required | A simple creature appears, can be selected/queried, and unknown creature queries fail with the expected marker. |
| Gossip/vendor/trainer | PASS or logged DEFERRED | Basic gossip and vendor open in the real client; trainer must either open with a minimal sane response or be explicitly deferred to Checkpoint 2 with an issue. |
| Inventory and equipment | PASS required | Move, equip/unequip, destroy, split, stack merge, and equipped-bag movement render correctly enough in the real client and persist after relog. |
| Combat/spell | PASS required | Select a simple creature, start/stop attack, cast one starter spell, see health/resource updates, and avoid combat packet desync. |
| Loot | PASS required | Loot money and one item, see inventory/money update, relog, and keep DB state. |
| Death/respawn | PASS or logged DEFERRED | Either prove simple death/respawn in the real client or explicitly move it to Checkpoint 2 with an issue. |
| Final fresh-character demo | PASS required | One fresh character completes auth, create, enter world, starter inspection, movement, chat, NPC interaction, vendor/trainer gate, combat/spell, loot, inventory action, logout, and relog in one uninterrupted session. |

Closure rule:

- No `FAIL` rows may remain.
- No `PARTIAL` row may remain without a linked GitHub issue and an explicit
  reason it does not block Checkpoint 1.
- Required `PASS` rows cannot be downgraded to `DEFERRED`.
- The final handoff must include the real-client grading table, automated test
  commands and results, P0/P1 fixes made during the final pass, and P2/P3/P4
  issues logged or updated.

Current grading snapshot, 2026-04-25:

Overall grade: `PARTIAL`, roughly 65-70% through Checkpoint 1. The checkpoint
is not closeable yet, but the previous `Combat/spell` blocker is cleared by a
real-client Raptor Strike retest.

- `PASS`: auth/realm list, character screen after fixture reset, Night Elf
  female Hunter race/gender display after the starter boots fix, and
  walk/turn/jump/fall/land movement without disconnect.
- `PARTIAL`: starter state, because equipment visuals now look sane and Raptor
  Strike works, but broad starter spell coverage remains future parity;
  chat/emote, because `/hello` has audio/text feedback but no physical wave
  animation.
- `PASS`: combat/spell. The real client previously tried active Night Elf
  Hunter starter spell `2973`, and Rust logged `Ignoring unsupported spell cast
  in starter spell fixture slice`; after the #13 fixture fix, the user
  confirmed Raptor Strike works in the real client.
- `DEFERRED`: trainer behavior is deferred to #39 because meaningful trainer
  verification needs leveling/trainer-learning context. Player death/respawn is
  deferred to #44 because it needs ghost/corpse/graveyard/resurrection behavior
  beyond the first playable world loop.

Plan impact: no grading-table row remains ungraded. Next closure work should
rerun the required automated gate and update the final Checkpoint 1 handoff.

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
     in both inventory and equipment cache.
   - Current status: destroy/split guardrails are implemented for present
     bag-0 backpack/equipment items and basic bag-contained positions via
     `CMSG_DESTROYITEM` and `CMSG_SPLIT_ITEM`. Rust validates
     `ITEM_FLAG_NO_USER_DESTROY`, supports partial stack destroy by updating
     `item_instance.count`, supports splitting part of a stack into an empty
     supported storage position, destroys items stored inside equipped bag
     slots 19-22, refreshes session inventory, sends stack-count or slot-clear
     updates, and refreshes equipment cache when an equipped item is destroyed.
     `test-world-flow.cmd` proves partial hearthstone destroy, split into bag
     slot 19:1, bag-contained destroys, no-destroy rejection for equipped shirt
     `38`, and full equipped destroy. Full new-item create/update fidelity for
     real-client split visuals, durability, and full class/race/equipment
     validation remain future Inventory v1 slices.
   - Current status: generic bag-container movement and simple stack merge are
     implemented for supported storage positions via `CMSG_SWAP_ITEM`. Rust
     persists backpack-to-equipped-bag moves, bag-internal moves, and
     same-template stack merges in `character_inventory` / `item_instance`.
     `test-world-flow.cmd` proves moving source-backed Tough Jerky `117` from
     backpack slot 27 into bag 19:3, moving it within bag 19, and merging it
     into an existing bag stack. Full client-visible container slot update
     fidelity remains tracked separately from the DB slice.
   - Current status: supported split, bag move, and stack-merge responses now
     send more complete `SMSG_UPDATE_OBJECT` blocks for client visuals. Rust
     distinguishes item versus container create blocks, writes container slot
     counts, updates player inventory fields, updates container slot fields,
     and sends contained-guid updates when items move into or out of equipped
     bags. Real-client smoke still needs to prove closure for all split and
     bag-container visual cases.
9. NPC interaction v1
   - Spawn/query a tiny fixture set of creatures/gameobjects from the world DB
     or a controlled test fixture.
   - Implement enough `CMSG_CREATURE_QUERY`, `CMSG_GAMEOBJECT_QUERY`,
     gossip hello, vendor list, and trainer list for a real client to open
     simple interactions.
   - Current status: the hardcoded `Rust Guide` fixture supports gossip and a
     small vendor list. It answers `CMSG_LIST_INVENTORY` and sells source-backed
     items `2102` and `117`, inserts purchases into the first empty backpack
     slot, and sends buy plus inventory update packets. This is intentionally a
     fixture path, not DB-backed `npc_vendor` parity.
   - Current status: DB-backed vendor-list v1 is implemented for DB creature
     GUIDs. Rust reads `npc_vendor` joined to `item_template`, serializes
     CMaNGOS-shaped `SMSG_LIST_INVENTORY` rows, sends the vanilla no-inventory
     marker for empty vendors, and the packet DB harness proves a seeded DB
     creature can answer a vendor-list request. DB creatures with vendor rows
     now also answer `CMSG_GOSSIP_HELLO` with a simple vendor gossip option, and
     `CMSG_GOSSIP_SELECT_OPTION` opens the DB-backed vendor list. The one-option
     DB vendor gossip menu uses zero-based option id `0` to match the vanilla
     client menu. Trainer lists, full vendor validation, and buyback remain
     future slices.
   - Current status: vendor money/sell v1 charges DB vendor `BuyPrice`, returns
     `SMSG_BUY_FAILED` when the player lacks money, updates player coinage
     after paid buys, and supports conservative selling of owned sellable items
     for `SellPrice * count`.
   - Current status: DB-backed vendor lists filter out container items until the
     DB container purchase update shape is proven; a real WoW 5875 client crash
     was observed when shift-right-click buying the DB guide's Small Brown
     Pouch. Container purchase coverage remains on the Rust Guide fixture path.
10. Combat and spell v1
   - Implement target selection, auto-attack start/stop, swing timing basics,
     health updates, death/respawn basics, and one or two starter instant spell
     casts.
   - Keep combat deterministic in harness tests before adding broader spell
     mechanics.
11. Loot v1
    - Support opening loot, taking money/items, updating inventory, and
      persisting creature loot state enough for a single-player demo.
    - Current status: the fixture combat dummy exposes money and Tough Jerky
      `117` x2 after death. Rust handles `CMSG_LOOT_MONEY` by persisting
      character coinage and sending money update packets, and handles
      `CMSG_AUTOSTORE_LOOT_ITEM` by inserting the item into the first empty
      backpack slot and sending loot/inventory updates. This is fixture-only;
      loot tables, corpse persistence, XP, respawn, and group loot remain future
      slices.
12. First Playable demo pass
    - Run the real-client grading pass above through the full loop on a fresh
      account/character and record the table in `docs/session_handoff.md`.
    - Run `test-rust.cmd`, `test-auth-flow.cmd`,
      `test-character-lifecycle.cmd`, `test-world-flow.cmd`, and any new
      Checkpoint 1 harnesses.
    - Update `docs/session_handoff.md` with exactly what was demonstrated, the
      final grades, and the GitHub issue numbers for anything intentionally
      left outside Checkpoint 1.

### Checkpoint 2: Starter Zone Playability

Goal: one starter zone can be played as a coherent early-game experience rather
than a handpicked interaction demo.

Recommended target:

- Use Northshire Valley with a Human Warrior as the golden path. The current
  Rust starter coverage is strongest for Human Warrior, and Northshire gives a
  compact set of early quests, wolves, kobolds, vendors, trainers, loot,
  graveyard behavior, and level-up expectations.
- Keep other races/classes out of the closure gate until the golden path is
  stable. Add small matrix coverage only after the Northshire flow is proven.

Success looks like:

- A chosen starter zone has creature spawns, gameobjects, vendors, trainers,
  graveyard/respawn flow, loot tables, basic aggro/leashing, and enough class
  spells to play several opening minutes.
- A player can accept, progress, and complete a small set of starter quests.
- Creature respawn, XP gain, level-up basics, money, inventory, and trainer
  learning persist correctly.
- The implementation uses CMaNGOS DB/script data where possible, with explicit
  fixture shortcuts documented.

Detailed path:

1. Starter-zone fixture lock
   - Seed/import only the CMaNGOS DB rows needed for the Northshire slice:
     creatures, gameobjects, quest givers, vendors, trainers, graveyard,
     loot templates, and quest templates.
   - Add `scripts/test-starter-zone-flow.cmd` and a focused Rust harness
     instead of growing `world-flow-test` into a full gameplay suite.
   - Prove the harness can create a clean Human Warrior, enter Northshire, and
     observe the expected DB rows without requiring the real client.
   - Current status: first harness skeleton is implemented in
     `bins/starter-zone-flow-test`. It seeds a narrow Rust Northshire fixture
     range (`910xxx`) into CMaNGOS-shaped world tables, creates a clean Human
     Warrior through `wow_db::create_character`, and proves the Northshire
     spawn boundary, DB creature/template joins, quest giver/completer rows,
     vendor/trainer rows, loot rows with valid source-backed item templates,
     gameobject rows, graveyard link, and `realmcharacters` count. This is a
     fixture lock only; DB-backed combat, quest packets/state, trainer
     learning, XP, death, and real-client grading remain later Checkpoint 2
     slices.
2. Creature visibility and lifecycle
   - Load nearby DB creatures by map/position/range for the starter zone.
   - Track alive, corpse, looted, and respawn states in a way that survives the
     single-player demo flow.
   - Keep creatures mostly static for this checkpoint unless movement becomes
     necessary for combat or client stability.
   - Current status: DB-backed Northshire fixture creatures are proven visible
     through the Rust auth/world packet path. `test-starter-zone-flow.cmd` now
     starts authserver/worldserver, authenticates the `STARTZONE` account,
     enters the clean Human Warrior, asserts the five seeded Northshire DB
     creature GUIDs are present in `SMSG_UPDATE_OBJECT`, and drives the DB
     Young Wolf through alive -> damaged/dead lootable corpse -> looted ->
     respawned-alive runtime state.
3. Combat v2
   - Move from fixture dummy combat to DB-backed creature combat.
   - Implement target selection, auto-attack start/stop, swing timing basics,
     player and creature health updates, creature death, and conservative
     evade/leash behavior.
   - Keep spell support narrow at first: Human Warrior melee and Heroic Strike
     are enough for the golden path.
   - Current status: one DB-backed Northshire hostile can be attacked through
     `CMSG_ATTACKSWING`; Rust sends attack start, attacker-state damage,
     creature health/dynamic-flag updates, rage updates, and attack stop on
     death. This is single-player/static-spawn combat only; creature retaliation,
     aggro, leash/evade, XP, and quest kill credit remain future slices.
4. Loot tables v1
   - Use `creature_loot_template` plus `item_template` data for DB-backed
     creature loot.
   - Support money, normal item drops, quest item drops, full-inventory
     failure, and corpse loot state.
   - Persist money/inventory updates and verify them after relog.
   - Current status: Rust can read one normal item from
     `creature_loot_template` joined to `item_template`, expose
     `MinLootGold`/`MaxLootGold` as corpse money, autostore the item through
     existing inventory insertion/stacking behavior, clear looted money/item
     state, and immediately respawn the single DB creature on loot release for
     the harness. Broader drop chances/groups, quest drops, no-space DB loot
     rollback, and relog durability remain future loot-table work.
5. Quest system v1
   - Implement quest status query, accept, progress update, complete, reward
     grant, and quest-log persistence.
   - Start with two or three Northshire quests covering at least one kill-count
     quest and one item/progress quest. Add talk-to or delivery only after the
     first two quest shapes are proven.
   - Use CMaNGOS quest relation/template data where possible and document any
     fixture shortcut.
6. XP and level-up v1
   - Award creature XP and quest XP.
   - Implement level-up packet/update fields, health/power/stat refresh, and
     persisted level/XP state.
   - Prove levels 1-2, or 1-3 if the selected quest set reaches it naturally.
7. Trainer v1
   - Open a real trainer list from DB/source-derived trainer data.
   - Show available and unavailable spells sanely enough for the starter path.
   - Learn one valid ability/spell, charge money if applicable, and persist the
     learned spell in `character_spell`.
8. Death, corpse, graveyard, and respawn
   - Implement player death from creature combat, release spirit, ghost/corpse
     basics, nearest graveyard selection, and resurrection.
   - Defer long-tail durability, resurrection sickness, and map-collision
     nuance unless the real client flow requires them.
9. Gameobject v1
   - Add DB-backed gameobject spawn/query and interaction if the selected
     Northshire quest set requires it.
   - If no selected quest requires gameobjects, explicitly defer richer
     gameobject behavior to Checkpoint 3 with a GitHub issue.
10. Real-client demo pass
    - Run the full Northshire flow with a WoW 1.12.1 build 5875 client.
    - Fix P0/P1 blockers only. Log P2/P3/P4 parity gaps as GitHub issues per
      the repo triage policy.

Required automated gate:

- `scripts/test-rust.cmd`
- `scripts/test-auth-flow.cmd`
- `scripts/test-character-lifecycle.cmd`
- `scripts/test-world-flow.cmd`
- `scripts/test-starter-zone-flow.cmd`

`test-starter-zone-flow.cmd` should prove, without the real client:

- clean Human Warrior creation and Northshire entry;
- DB-backed starter-zone creature/NPC/gameobject availability;
- creature query and visibility packet shape;
- DB creature combat kill;
- DB loot-table money/item/quest-drop handling;
- quest accept/progress/complete persistence;
- XP and level-up persistence;
- trainer list and spell learning persistence;
- death/release/respawn state transitions;
- logout/relog durability for position, inventory, money, level, XP, spells,
  quest state, and completed quest rewards.

Real-client grading pass:

Run `scripts/run-client-stack-18085.cmd` against a clean local DB fixture and a
WoW 1.12.1 build 5875 client. Record the result in `docs/session_handoff.md`
using `PASS`, `PARTIAL`, `FAIL`, or `DEFERRED` for each row.

| Gate | Grade | Required observation |
| --- | --- | --- |
| Northshire spawn set | PASS required | Human Warrior enters Northshire and sees DB-backed starter NPCs, creatures, vendors, trainers, and any selected quest objects. |
| Creature lifecycle | PASS required | DB creatures can be selected, queried, killed, looted, despawned/corpse-tracked, and respawned without corrupting session state. |
| Combat v2 | PASS required | Player and DB creature exchange melee attacks, Heroic Strike or equivalent starter action works, health/resource updates render, and combat ends cleanly. |
| Loot tables | PASS required | DB-backed money, normal item, and quest item loot can be taken, fail safely with no space, and persist after relog. |
| Quest accept/progress/complete | PASS required | At least one kill-count quest and one item/progress quest can be accepted, progressed, completed, rewarded, and persisted. |
| XP and level-up | PASS required | Creature or quest XP updates render, at least one level-up occurs, and level/XP/stat state persists after relog. |
| Trainer learning | PASS required | Trainer window opens, one valid ability/spell can be learned when requirements are met, money/spell state updates, and learned state persists. |
| Vendor loop | PASS required | Starter-zone DB vendor buy/sell works with money and inventory persistence. |
| Death and respawn | PASS required | Player can die to a starter-zone creature, release, resurrect at corpse or graveyard, and continue playing. |
| Gameobject interaction | PASS or logged DEFERRED | Required quest gameobjects work, or richer gameobject behavior is explicitly deferred with a GitHub issue if not used by the selected quest set. |
| Logout/relog durability | PASS required | Position, inventory, money, level, XP, spells, quest state, completed rewards, creature state expectations, and death/respawn state remain sane after relog. |
| Final fresh-character zone demo | PASS required | One fresh Human Warrior completes auth, create/select, Northshire entry, quests, combat, loot, XP/level-up, trainer, vendor, death/respawn, logout, and relog in one coherent session. |

Definition of done:

- No `FAIL` rows may remain in the Checkpoint 2 real-client grading table.
- No `PARTIAL` or `DEFERRED` row may remain without a linked GitHub issue and a
  clear reason it does not block Checkpoint 2.
- All required automated gate scripts pass.
- The final handoff records the real-client grading table, exact tests run,
  P0/P1 bugs fixed immediately, P2/P3/P4 issues logged or updated, and any
  intentionally unfixed discoveries.

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
./scripts/test-rust.sh  # Linux/macOS
.\scripts\test-rust.cmd # Windows
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
