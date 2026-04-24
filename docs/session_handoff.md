# Session Handoff

This file is the current operating brief for the next Rust migration session.
Keep it short. Durable roadmap and milestone history belong in
`docs/rust_migration_plan.md`; auth-specific setup belongs in
`docs/rust_auth_foundation.md`.

## Handoff Rules

- Target length: about 120 lines. Hard cap: 180 lines.
- Keep only current branch state, active goal, last meaningful changes, exact
  tests run, local blockers, and the next recommended task.
- Replace old session detail with a one-line summary or move durable roadmap
  detail into `docs/rust_migration_plan.md`.
- Do not append a full chronological log. Each update should prune stale
  bullets from the same section.
- Keep "What Changed Recently" focused on the last one or two substantial
  slices.

## Current Branch

- Branch: `codex/rust-auth-foundation`
- Latest commit: see `git log -1 --oneline`
- Remote: `origin/codex/rust-auth-foundation`
- Latest local commit includes:
  - Checkpoint 1 DB-backed and derived player vitals/stats/XP slice.
  - `AGENTS.md` bug triage / GitHub logging policy.
  - Checkpoint 1 initial faction, class power, tutorial flag, and first-login
    cleanup slice.

## Current Goal

Checkpoint 1: **First Playable World**.

The Rust auth/world stack can already authenticate a real WoW 1.12.1 client,
show character select, create/select/delete characters, enter a skeletal world,
move, logout/relog, persist position, seed starter spells/actions/skills/items,
and pass packet-level character lifecycle coverage.

Current Checkpoint 1 focus:

- Continue player `SMSG_UPDATE_OBJECT` parity and starter/default cleanup.
- Expand DB/DBC-backed starter/default loading beyond health/mana/stats/XP into
  class power defaults, faction/reputation, and fuller create-info parity.
- Keep real-client smoke passes and `world-flow-test` green after each slice.

## What Changed Recently

- Replaced hardcoded level-1 health/mana/stat/next-XP fallbacks with
  `wow_db::get_player_world_stats` sourced from CMaNGOS
  `player_classlevelstats`, `player_levelstats`, and `player_xp_for_level`.
  New character creation now seeds derived max health/mana using the CMaNGOS
  stamina/intellect formulas, and `world-flow-test` verifies those values.
- User ran a real-client smoke after the derived vitals/stat/XP slice and
  reported it works; level 1 Human Warrior HP is no longer stuck at 20.
- Added repo-local bug triage / GitHub logging policy to `AGENTS.md`.
- Began the next bootstrap slice: Rust now sends a basic CMaNGOS-shaped
  `SMSG_INITIALIZE_FACTIONS` packet after initial spells/action buttons, class
  power defaults use explicit CMaNGOS `GetCreatePowers` constants for warrior
  rage and rogue energy, and packet tests cover both.
- Finished the tutorial/first-login cleanup slice: Rust loads/saves
  `character_tutorial`, handles tutorial flag/clear/reset opcodes, sends
  account tutorial flags during login, and marks `cinematic = 1` plus clears
  `AT_LOGIN_FIRST` once a character login is accepted. `world-flow-test` now
  verifies the first-login DB cleanup.
- Attempted a first starter reputation persistence slice, then rolled back the
  unsafe visible portion after real-client smoke showed raw faction IDs were
  being mistaken for `Faction.dbc` reputation-list slots. Rust now keeps the
  packet quiet again until the true DBC-backed mapping is ported.
- Added source-referenced first-login cinematic playback: Rust sends
  `SMSG_TRIGGER_CINEMATIC` for `cinematic = 0` characters using vanilla
  playable-race `ChrRaces.dbc` cinematic sequence IDs, then preserves the
  existing first-login cleanup that marks `cinematic = 1` and clears
  `AT_LOGIN_FIRST`.
- Added a small player update-object parity slice for watched reputation:
  new Rust-created characters now persist `watchedFaction = -1`, enum loading
  carries it, and the self-spawn `SMSG_UPDATE_OBJECT` includes
  `PLAYER_FIELD_WATCHED_FACTION_INDEX`.
- Added a combined Checkpoint 1 v1 polish pass:
  - visible item update values now prefer live equipped `character_inventory`
    rows over stale `equipmentCache` fallback data;
  - `CMSG_SET_ACTIVE_MOVER` is parsed and guarded instead of treated as a
    generic ignored bootstrap opcode;
  - `MSG_QUERY_NEXT_MAIL_TIME` now returns the CMaNGOS-shaped float response
    based on unread delivered mail.
- Added basic solo chat v1: Rust handles `CMSG_MESSAGECHAT` for say, yell, and
  emote, then echoes a CMaNGOS-shaped `SMSG_MESSAGECHAT` packet back to the
  sender for real-client smoke testing. Party, guild, channel, whisper,
  language skill checks, chat range, and multi-client broadcast remain future
  slices.
- Added basic slash text-emote v1 after real-client smoke showed most slash
  emotes were not covered by chat packets. Rust now handles `CMSG_TEXT_EMOTE`
  and echoes a CMaNGOS-shaped `SMSG_TEXT_EMOTE` packet back to the sender with
  an empty target name for solo smoke testing.
- Added text-emote animation support after smoke showed emote chat text without
  avatar animation. Rust now maps common slash text emotes (`/wave`, `/point`,
  `/dance`, `/sleep`) to CMaNGOS emote animation IDs, sends `SMSG_EMOTE` for
  one-shot animations, and sends an update-object `UNIT_NPC_EMOTESTATE` update
  for state emotes.

## Tests Last Run

Passing locally for this slice:

```powershell
$env:PATH = "$env:USERPROFILE\.cargo\bin;$env:PATH"; .\scripts\test-rust.cmd
$env:PATH = "$env:USERPROFILE\.cargo\bin;$env:PATH"; .\scripts\test-world-flow.cmd
$env:PATH = "$env:USERPROFILE\.cargo\bin;$env:PATH"; cargo test -p wow-db -p wow-network
$env:PATH = "$env:USERPROFILE\.cargo\bin;$env:PATH"; cargo test -p wow-network
```

Notes:

- Docker-backed tests need elevated Docker access in the Codex app shell.
- Latest `test-rust.cmd` passed after stopping the running real-client stack
  that had locked `target\debug\authserver.exe`.
- Latest `test-world-flow.cmd` passed with elevated Docker access after the
  cinematic packet, watched-faction update-object slice, and the visible
  equipment / active mover / mail-time v1 polish pass, plus basic chat v1,
  text-emote v1, and text-emote animation support were added.
- `cargo fmt` was run.
- Real-client smoke after the attempted reputation slice showed incorrect
  Bloodsail Buccaneers / Gelkis / Magram / Syndicate entries; the visible
  mapping was rolled back and a follow-up smoke showed an empty reputation page
  again.
- During this session, stale `authserver.exe` / `worldserver.exe` processes
  had to be stopped once because they were locking `target\debug\authserver.exe`.

Last manual real-client smoke:

```powershell
.\scripts\run-client-stack-18085.cmd
```

Stack started successfully:

- Authserver: `127.0.0.1:13724`
- Worldserver: `127.0.0.1:18085`
- Client realmlist: `set realmlist 127.0.0.1:13724`

The user reported the derived vitals/stat/XP smoke works.

## Local Environment Notes

- Rust is available through `%USERPROFILE%\.cargo\bin`.
- Docker-backed tests may require elevated Docker access.
- MariaDB test container: `cmangos-rust-realmd` on local port `3307`.
- Normal WoW ports are blocked locally; manual client stack uses auth `13724`
  and world `18085`.

## Next Recommended Task

Continue Checkpoint 1 player update-object / starter-default parity:

1. Run a short real-client smoke for the tutorial/first-login slice if desired:
   create/login a fresh character, confirm normal login, logout/relog, and
   optionally toggle tutorial hints.
2. If the current manual smoke stack is no longer needed, stop stale
   `authserver.exe` / `worldserver.exe` processes and rerun `test-rust.cmd`
   plus `test-world-flow.cmd` to fully close the starter reputation slice.
3. Real-client smoke basic chat/text emotes: `/say`, `/yell`, `/emote`, and a
   few slash emotes such as `/wave`, `/dance`, `/sleep` should display locally
   without disconnecting.
4. Continue starter-default cleanup: full DBC-backed reputation defaults, or
   proceed into NPC query/interaction v1.
5. Rerun after the next code slice:

```powershell
$env:PATH = "$env:USERPROFILE\.cargo\bin;$env:PATH"; .\scripts\test-rust.cmd
$env:PATH = "$env:USERPROFILE\.cargo\bin;$env:PATH"; .\scripts\test-world-flow.cmd
```


## Key Files

- `docs/rust_migration_plan.md`
- `docs/rust_auth_foundation.md`
- `crates/wow-network/src/world/mod.rs`
- `crates/wow-db/src/character.rs`
- `bins/world-flow-test/src/main.rs`
- `scripts/test-rust.cmd`
- `scripts/test-world-flow.cmd`
- `scripts/run-client-stack-18085.cmd`
- `sql/base/mangos.sql`
- `sql/base/characters.sql`
- C++ references:
  - `src/game/Entities/UpdateFields.h`
  - `src/game/Entities/UpdateFields.cpp`
  - `src/game/Entities/Player.cpp`
  - `src/game/Globals/ObjectMgr.cpp`
  - `src/game/Reputation/ReputationMgr.cpp`
  - `src/game/Entities/CharacterHandler.cpp`
  - `src/game/Server/WorldSession.cpp`

## Non-blocking Backlog

- GitHub Issues are now enabled and should be the source of truth for
  non-blocking P2/P3/P4 work.
- Current logged issues:
  - #3 `[Rust Rewrite][P3][Reputation] Initial reputation packet uses zeroed DBC state placeholder`
    - Updated local finding: real-client smoke confirmed raw faction IDs cannot
      be used as 64-slot reputation-list indexes; port `Faction.dbc`
      `reputationListID` before sending visible starter reputations.
  - #4 `[Rust Rewrite][P3][WorldBootstrap] First-login cinematic playback is not source-derived`
    - Updated with the first Rust implementation. Remaining caveat: replace
      hardcoded vanilla playable-race cinematic constants with shared DBC-backed
      loading when the broader DBC path exists.
  - #5 `[Rust Rewrite][P4][DB] Split character lifecycle module and add transactions`

## Known Blockers And Gaps

- The self-spawn update is improved but still not full CMaNGOS player object
  parity.
- Health/mana/stats/next-XP now come from world DB tables with CMaNGOS-style
  stamina/intellect derivation, but broader derived player fields still need
  source-derived parity work.
- Broader world gameplay remains skeletal: movement persistence works, but
  validation, visibility, NPCs, combat, spells, inventory actions, loot, and
  chat still need Checkpoint 1 slices.

## 2026-04-24 - Checkpoint 1 NPC query/gossip fixture v1

Latest commit: not checked in this task.

What changed:
- Added a minimal `Rust Guide` fixture creature to the initial world `SMSG_UPDATE_OBJECT` so the real 1.12.1 client has a visible, selectable NPC for Checkpoint 1 smoke testing.
- Added `CMSG_CREATURE_QUERY` / `SMSG_CREATURE_QUERY_RESPONSE` handling for the fixture using the CMaNGOS packet layout from `src/game/Entities/QueryHandler.cpp`.
- Added `CMSG_GOSSIP_HELLO` / `SMSG_GOSSIP_MESSAGE` handling for the fixture using the no-option menu shape from `src/game/Entities/GossipDef.cpp` and `src/game/Entities/NPCHandler.cpp`.
- Added focused packet/update tests for creature query responses, missing creature query responses, empty gossip menu shape, and fixture unit gossip flags.

Tests run:
- `cargo fmt`
- `scripts/test-rust.cmd` - passed after stopping the previously running local stack that was locking `authserver.exe`.
- `scripts/test-world-flow.cmd` - passed.

P0/P1 fixed immediately:
- None.

P2/P3/P4 issues logged:
- GitHub #11: `[Rust Rewrite][P2][NPC] Checkpoint fixture NPC is hardcoded instead of DB-backed`.

Discovered issues intentionally not fixed:
- The fixture NPC is intentionally hardcoded for this proof slice; DB-backed creature/template spawning is deferred to GitHub #11.

Recommended next task:
- Real-client smoke the fixture NPC: enter world, confirm `Rust Guide` appears near the player, select/click it, and verify no disconnect or weird popup behavior.

Key files for next agent:
- `crates/wow-network/src/world/mod.rs`
- `src/game/Entities/QueryHandler.cpp`
- `src/game/Entities/GossipDef.cpp`
- `src/game/Entities/NPCHandler.cpp`

## 2026-04-24 - Checkpoint 1 NPC gossip dialogue P0 fix

Latest commit: not checked in this task.

What changed:
- Fixed the visible `Rust Guide` fixture NPC interaction path after real-client smoke showed `CMSG_GOSSIP_HELLO` was received but no dialogue frame opened.
- Added `CMSG_NPC_TEXT_QUERY` / `SMSG_NPC_TEXT_UPDATE` support using the CMaNGOS eight-option NPC text shape from `src/game/Entities/QueryHandler.cpp`.
- Updated the fixture gossip message to use a non-zero text id, include one simple gossip option, and send NPC text before `SMSG_GOSSIP_MESSAGE` on hello.
- Added `CMSG_GOSSIP_SELECT_OPTION` handling that sends `SMSG_GOSSIP_COMPLETE` for the fixture option.
- Added focused unit coverage for the NPC text update packet shape.

Tests run:
- `cargo fmt`
- `scripts/test-rust.cmd` - passed.
- `scripts/test-world-flow.cmd` - passed.

P0/P1 fixed immediately:
- P0: The fixture NPC could be selected/clicked but did not open a dialogue frame because the gossip response lacked the NPC text cache/update path and concrete menu content.

P2/P3/P4 issues logged:
- Existing GitHub #11 still tracks replacing the hardcoded fixture NPC with DB-backed creature/template loading.

Discovered issues intentionally not fixed:
- The fixture NPC remains hardcoded pending GitHub #11.

Recommended next task:
- Real-client retry: right-click `Rust Guide`, confirm the gossip frame opens with text and the `Keep going.` option, select the option, and confirm it closes without disconnect.

Key files for next agent:
- `crates/wow-network/src/world/mod.rs`
- `src/game/Entities/QueryHandler.cpp`
- `src/game/Entities/GossipDef.cpp`
- `src/game/Entities/NPCHandler.cpp`

## 2026-04-24 - Checkpoint 1 basic combat fixture v1

Latest commit: not checked in this task.

What changed:
- Added a hostile `Rust Combat Dummy` fixture creature near the player alongside the friendly `Rust Guide` NPC.
- Added `CMSG_ATTACKSWING` / `CMSG_ATTACKSTOP` handling for the fixture dummy.
- On attack swing, the Rust world server now sends CMaNGOS-shaped `SMSG_ATTACKSTART`, `SMSG_ATTACKERSTATEUPDATE`, and `SMSG_UPDATE_OBJECT` health update packets.
- On attack stop, the Rust world server sends `SMSG_ATTACKSTOP`.
- Added focused tests for the combat dummy create block, melee attack packet shapes, and dummy health update object shape.

Tests run:
- `cargo fmt`
- `scripts/test-rust.cmd` - passed.
- `scripts/test-world-flow.cmd` - passed.

P0/P1 fixed immediately:
- None.

P2/P3/P4 issues logged:
- GitHub #12: `[Rust Rewrite][P2][Combat] Fixture combat lacks AI timers, death, XP, and loot parity`.
- Existing GitHub #11 still tracks replacing hardcoded fixture NPCs with DB-backed creature/template loading.

Discovered issues intentionally not fixed:
- Combat remains a deterministic fixture packet loop only: no AI, no periodic swing timers, no full death/respawn, no XP, no loot, no DB-backed creature combat yet.

Recommended next task:
- Real-client smoke: target `Rust Combat Dummy`, start attacking, verify attack animation/combat feedback/health movement/no disconnect, then stop attacking.

Key files for next agent:
- `crates/wow-network/src/world/mod.rs`
- `src/game/Combat/CombatHandler.cpp`
- `src/game/Entities/Unit.cpp`
- `src/game/Entities/Unit.h`

## 2026-04-24 - Checkpoint 1 combat auto-swing P0 fix

Latest commit: not checked in this task.

What changed:
- Fixed the basic combat fixture loop after real-client smoke showed only the first attack worked unless the player untargeted/retargeted and right-clicked again.
- Added a server-side 2-second combat tick around the active `Rust Combat Dummy` target so auto-attack continues producing melee swings while the client remains in combat range.
- Tracked the active combat target in `WorldSessionState` and clears it on attack stop or when the dummy reaches zero health.
- Added focused coverage for active dummy combat target session tracking.

Tests run:
- `cargo fmt`
- `scripts/test-rust.cmd` - passed.
- `scripts/test-world-flow.cmd` - passed.

P0/P1 fixed immediately:
- P0: Auto-attack did not continue after the initial `CMSG_ATTACKSWING`; server now drives follow-up fixture swings on a 2-second tick.

P2/P3/P4 issues logged:
- Existing GitHub #12 tracks full combat parity work: AI timers, death, XP, loot, and related systems.
- Existing GitHub #11 tracks replacing hardcoded fixture creatures with DB-backed creature/template loading.

Discovered issues intentionally not fixed:
- The 2-second tick is a fixture/server loop only, not the full CMaNGOS melee timer system; full parity remains tracked in GitHub #12.

Recommended next task:
- Real-client retest: target `Rust Combat Dummy`, right-click once, stay in range, and verify repeated swings/health updates occur without retargeting.

Key files for next agent:
- `crates/wow-network/src/world/mod.rs`
- `src/game/Combat/CombatHandler.cpp`
- `src/game/Entities/Unit.cpp`
- `src/game/Entities/Unit.h`

## 2026-04-24 - Real-client combat fixture smoke result

Latest commit: pending commit.

Real-client result:
- `Rust Combat Dummy` is visible and targetable.
- First right-click attack works.
- Continued attacks now repeat without untargeting/retargeting.
- No disconnect or weird popup reported.
- Swing cadence feels faster than the intended fixture 2-second tick; this is treated as non-blocking combat timing/parity under GitHub #12.

P0/P1 fixed immediately:
- P0 auto-attack continuation blocker fixed by server-side fixture combat tick.

P2/P3/P4 issues logged:
- Existing GitHub #12 covers full combat parity, including proper melee timers/cadence, AI, death, XP, and loot.

Recommended next task after commit:
- Either commit this Checkpoint 1 playable-world batch, or continue with a narrow dummy death/reset slice if desired.
