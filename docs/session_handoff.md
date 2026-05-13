# Session Handoff

Short operating brief for the next Rust migration session. Durable roadmap
history belongs in `docs/rust_migration_plan.md`, gate status in
`docs/playable_gate_board.md`, and branch split details in
`docs/playable_execution_roadmap.md`.

## Current Branch And State

- Branch: `codex/rusty-mangos`, in the main checkout at
  `C:\Users\subhe\Documents\New project`.
- Current user-directed priority: auditor hardening/refactor sequence before
  more gameplay. P1 network/session hardening is implemented locally; current
  active slice is the P2 typed protocol/world dispatch refactor.
- Current state: uncommitted changes in `wow-network` and `wow-proto` replace
  auth whole-read dispatch with exact auth frame reads, add bounded world
  outbound queues/lifecycle controls, finish the first typed world protocol
  dispatch pass, replace the broad include-based world surface with real Rust
  modules, split `WorldSessionState` into typed session components, and move
  the main outbound world response packet serializers into `wow-proto`.
  Dispatch tracking is explicit: 78 main world dispatch arms, 78 typed dispatch
  arms, and 0 dispatch arms still on the raw body path. Module tracking is also
  explicit: `rg "include!\(" crates\wow-network\src\world -n` returns no
  matches.
- A local `target\debug\authserver.exe --config config\authserver.local.toml`
  process was already running and holding the default target binary during
  verification. Use an isolated `CARGO_TARGET_DIR` or stop that process before
  rebuilding `target\debug\authserver.exe`.
- Playerbots are disabled by default for normal multiplayer/Northshire testing:
  `config/worldserver.local.toml` has `[playerbots] enabled = false` and
  `[playerbots.random] enabled = false`.

## Recent Implemented Work

- Authserver now reads command-framed auth packets with exact reads:
  challenge/reconnect-challenge honor the declared body length, proof/reconnect
  proof/realm-list read fixed sizes, malformed challenge lengths are rejected,
  and coalesced TCP bytes remain available for the next frame.
- Auth I/O now has a 10 second packet/write timeout instead of waiting
  indefinitely on stalled clients.
- World sessions now use bounded outbound queues for live session handles
  instead of `mpsc::UnboundedSender`. Queue-full sends record metrics and request
  session disconnect; direct session sink sends fail fast when the queue is full.
- World session lifecycle now includes a 30 second login timeout, 15 minute idle
  timeout, 10 second writer timeout, and writer error/timeout disconnect
  signalling back to the owning session loop.
- Observability now exports world session disconnect counters by reason,
  outbound queue-full totals, and latest/max observed outbound queue depth.
- `wow-proto` now has a first world-packet module with stable typed
  `WorldOpcode` values plus read/write helpers for auth-session, ping/pong,
  control/session, account data, tutorial, selection/target/mover, basic item
  queries, creature/gameobject/quest queries, gossip hello, NPC text,
  questgiver status/hello, gameobject use, vendor list, and trainer list.
- `wow-network::world::packets` now parses known client world packets into
  typed request objects before handler logic. Unknown opcodes remain available
  for the existing dispatch path.
- World session login auth now uses the typed auth-session parser from
  `wow-proto`, and ping handling now receives a typed `PingRequest` and emits a
  typed `PongResponse`. This removes the duplicate local auth-session parser
  from `world/wire.rs`.
- The session loop now routes these additional typed request structs into their
  handlers: name/item/creature/gameobject/quest queries, account data,
  tutorials, stand state, action buttons, selection/target/mover, gossip hello,
  gameobject use, questgiver status/hello, NPC text, vendor list, and trainer
  list. Gameplay-heavy handlers such as movement, combat/spells, inventory
  mutation, quest accept/reward, loot, group, and chat remain on the old raw
  body path.
- The remaining 35 main dispatch arms are now rewired through typed
  `wow-proto`/`world::packets` requests: character create/enum/delete/login,
  chat/channel/text-emote, spell cast/use item/cancel, inventory move/destroy/
  split, gossip select, quest query/accept/complete/reward/abandon, vendor sell/
  buyback/buy, trainer buy, attack start/stop, death/corpse lifecycle, and loot
  open/store/money/release/roll/master-give.
- Test helpers that used to call private raw parsers now read through the typed
  request structures, keeping the unit suite aligned with the new dispatch path.
- `wow-network::world` now has real module roots for server/session state,
  handlers, entities, globals, fixtures, map runtime, motion, social, combat,
  spells, and packet builders. The old `world/interactions.rs` include hub was
  removed.
- The nested include hubs in combat, packet builders, spells, session-owned
  entity/runtime files, and `maps/map.rs` were replaced with real child modules
  plus `crate::world`-scoped compatibility re-exports. This is intended as a
  behavior-preserving boundary move; `WorldServer` and the public
  `PlayerbotSpawnConfig` API remain available at `wow_network::world`.
- `WorldSessionState` is now componentized into `account`, `character`,
  `movement`, `combat`, `auras`, `inventory`, `quests`, `death`, `social`, and
  `visibility`. The old 36 flat fields are reduced to 10 component fields, and
  the 88 test constructors now use component literals. The intentionally
  retained compatibility helpers are the map/test mirror helpers in
  `session.rs`, now forwarding into the `combat` and `visibility` components.
- `wow-proto` now owns outbound world response serialization through the
  `ServerWorldPacket` trait. Existing `wow-network` builder names are retained
  as compatibility shims, but the converted shims construct protocol DTOs and
  call `.body()` instead of hand-writing canonical packet bytes locally.
- Converted outbound families now include NPC interaction/vendor/trainer,
  questgiver/quest-update, character screen/auth/login bootstrap, account data,
  tutorial/action buttons/initial spells/reputations/time/mail, name/chat/text
  emote/channel, group/social/party stats, combat and spell log packets, spell
  start/go/cast-result/failure/delay, aura duration, loot response/release/
  master/roll packets, death/corpse/root/unroot, monster movement/spline speed,
  XP/level-up, and the `SMSG_UPDATE_OBJECT` envelope.
- `SMSG_UPDATE_OBJECT` inner update blocks remain in `wow-network` intentionally
  because they depend on runtime entity state, update-field indices, and map
  authority. `wow-proto` owns only the envelope/count layout for now.

## Protocol Dispatch Tracking

- Count basis: main `session_loop.rs` dispatch arms, not every individual
  opcode hidden inside grouped arms or movement/no-op helper predicates.
- Total main dispatch arms: 78.
- Typed main dispatch arms: 78.
- Untyped main dispatch arms remaining: 0.
- Remaining untyped arms: none.

## Outbound Protocol Tracking

- Baseline before this pass: 174 high-level `build_*` functions under
  `crates\wow-network\src\world`.
- Current typed outbound baseline: 90 `ServerWorldPacket` implementations in
  `crates\wow-proto\src\world_packets.rs`.
- Converted response families: pong; gossip/NPC text; vendor buy/sell/list and
  inventory-change failure; trainer list/buy/learned/visual helpers; quest
  query/status/list/details/request-items/offer-reward/complete/add-kill;
  character create/delete/enum/login failure; auth response; logout response/
  complete/cancel-ack; login verify/bindpoint/rest/time/world-state bootstrap;
  account data times/update; tutorial flags; proficiencies, initial spells,
  action buttons, and initial reputations; query time/mail; name/chat/channel/
  text emote/emote; group invite/list/leader/command result and party member
  stats; attack start/stop/attacker-state; spell start/go/cast result/failure/
  failed-other/delayed/log-miss/non-melee/heal/energize; aura duration;
  loot response/error/release/master list/start roll/roll/won/all-passed;
  force root/unroot, corpse reclaim/query; monster movement, teleport ack,
  spline speed body; XP gain/level-up; and update-object envelope.
- Remaining raw or intentionally local families: item query, creature query,
  gameobject query, item push result, loot removed/money-notify/clear-money,
  destroy object, account/character/item/equipment update-field blocks, and all
  entity create/update block internals. These are still better owned by
  `wow-network` until the runtime/update-field DTO boundary is designed.
- Player death lifecycle is now map-owned with `PlayerDeathState::JustDied` as
  a runtime-only transition. Lethal melee, direct spell, DoT, environmental,
  fall, and GM-style damage paths flow through the same cleanup/presentation
  machinery.
- Death cleanup now clears combat flags, auto attack, queued melee spells,
  active player casts, pending player spell events, active creature casts
  targeting the dead player, looting/combo state, death-cleared auras, and
  creature threat/combat. Release Spirit and resurrection refresh from
  `MapRuntime` and restore movement defensively.
- Fixed the DoT death pop-up/relog class of bugs by preserving zero health in
  world-stat refresh packets, clearing stale fall tracking on normal grounded
  movement, and forcing pending `JustDied` presentation before repop.
- NPC periodic damage auras keep ticking from stored caster combat snapshots if
  the live caster runtime disappears while the debuff remains active.
- DB creature spell AI now validates selected targets through `MapRuntime` at
  cast start and completion. Validation covers caster/target liveness, range,
  LOS, and path guardrails.
- Creature spell-list cooldowns now combine initial delay, repeat cooldown,
  spell recovery, and category recovery. Category cooldown sharing honors
  `SPELL_LIST_FLAG_CATEGORY_COOLDOWN`.
- Creature casts interrupted by target death, range, or LOS now broadcast the
  CMaNGOS-shaped `SMSG_SPELL_FAILURE` and `SMSG_SPELL_FAILED_OTHER` cleanup
  packets instead of silently dropping the cast.
- Resisted creature spells now encode the miss in `SMSG_SPELL_GO` itself
  instead of listing the target as a hit and only sending a separate
  `SMSG_SPELLLOGMISS`. This is important for real-client spell visual/log
  parity on resisted Immolate.

## Tests Run

- `cargo fmt --package wow-network`
- `cargo check -p wow-network`
- `cargo test -p wow-network auth_frame_reader --lib`
- `cargo test -p wow-network bounded_queue --lib`
- `cargo test -p wow-network packet_dispatch_tests --lib`
- `cargo test -p wow-network parses_auth_session_packet --lib`
- `cargo test -p wow-network handle_join_channel_sends_you_joined_notify_packet --lib`
- `cargo test -p wow-network --lib` passed, including 622 `wow-network`
  tests.
- `$env:CARGO_TARGET_DIR='target\codex-network-hardening'; cargo test -p wow-proto --lib`
  passed, including 10 `wow-proto` tests.
- `.\scripts\test-rust.cmd` mostly passed but failed at the final
  `cargo build -p authserver` because `target\debug\authserver.exe` was already
  running and Windows denied replacing it.
- `$env:CARGO_TARGET_DIR='target\codex-network-hardening'; .\scripts\test-rust.cmd`
  passed fully after the expanded typed-dispatch batch.
- `$env:CARGO_TARGET_DIR='target\codex-network-hardening'; cargo check -p wow-network`
  passed with no warnings after the remaining-35 typed dispatch rewiring.
- `$env:CARGO_TARGET_DIR='target\codex-network-hardening'; cargo test -p wow-network --lib --no-run`
  passed after updating test helpers to typed request parsing.
- `$env:CARGO_TARGET_DIR='target\codex-network-hardening'; .\scripts\test-rust.cmd`
  passed fully after the remaining-35 typed dispatch rewiring, including 622
  `wow-network` tests and 10 `wow-proto` tests.
- Dispatch count verification after the final pass: 78 total main dispatch arms,
  78 typed, 0 untyped.
- `$env:CARGO_TARGET_DIR='target\codex-network-hardening'; .\scripts\test-rust-db.cmd`
  and `.\scripts\test-auth-flow.cmd` were attempted but blocked because
  `127.0.0.1:13724` was already held by the existing local authserver process
  (`os error 10048`).
- `$env:CARGO_TARGET_DIR='target\codex-real-modules'; cargo check -p wow-network`
  passed with no warnings after the real-module rewiring.
- `rg "include!\(" crates\wow-network\src\world -n` returns no matches after
  the module split.
- `$env:CARGO_TARGET_DIR='target\codex-real-modules'; .\scripts\test-rust.cmd`
  passed fully after the real-module split, including 622 `wow-network` tests
  and 10 `wow-proto` tests.
- `$env:CARGO_TARGET_DIR='target\codex-session-components'; cargo check -p wow-network`
  passed after the `WorldSessionState` component split.
- `$env:CARGO_TARGET_DIR='target\codex-session-components'; cargo test -p wow-network --lib --no-run`
  passed after updating the test-only constructors/mirrors.
- `rg -P "\bsession\s*\.\s*(...legacy fields...)" crates\wow-network\src\world -n`
  returned no legacy direct session-field hits after the split; the
  `WorldSessionState` direct component-field count is 10.
- `$env:CARGO_TARGET_DIR='target\codex-session-components'; .\scripts\test-rust.cmd`
  passed fully after the component split, including 622 `wow-network` tests and
  10 `wow-proto` tests.
- `$env:CARGO_TARGET_DIR='target\codex-outbound-proto'; cargo check -p wow-proto`
  passed after the outbound protocol DTO additions.
- `$env:CARGO_TARGET_DIR='target\codex-outbound-proto'; cargo check -p wow-network`
  passed after the proto-backed shim conversions.
- `$env:CARGO_TARGET_DIR='target\codex-outbound-proto'; cargo test -p wow-proto --lib`
  passed with 23 tests, including new outbound golden layout tests.
- Focused outbound family tests passed:
  `cargo test -p wow-network quest --lib`,
  `cargo test -p wow-network loot --lib`,
  `cargo test -p wow-network combat --lib`,
  `cargo test -p wow-network spell --lib`,
  `cargo test -p wow-network movement --lib`,
  `cargo test -p wow-network chat --lib`,
  `cargo test -p wow-network party --lib`,
  `cargo test -p wow-network char_enum --lib`,
  `cargo test -p wow-network initial_spells --lib`, and
  `cargo test -p wow-network packet_dispatch_tests --lib`.
- `$env:CARGO_TARGET_DIR='target\codex-outbound-proto'; cargo test -p wow-network --lib --no-run`
  passed after the outbound conversion.
- `$env:CARGO_TARGET_DIR='target\codex-outbound-proto'; .\scripts\test-rust.cmd`
  passed fully after the outbound conversion, including 622 `wow-network`
  tests and 23 `wow-proto` tests.

## Real-Client Verification Needed

- Re-run `.\scripts\test-rust-db.cmd` and `.\scripts\test-auth-flow.cmd` after
  stopping the existing local authserver process or moving it out of the
  default auth port. This should prove TCP startup and the patched auth framing
  with the seeded SRP flow over TCP.
- Spawn Burning Blade Neophyte `3196` and test Immolate at level 10 and level
  20. At level 20, expect frequent resists from the level 9-10 caster; proof is
  that the cast completes with visible spell miss/log behavior and does not
  disappear silently.
- Confirm the client receives/uses the `SMSG_SPELL_START` path for NPC casts:
  Immolate has `CastingTimeIndex = 5`, which resolves to a 2000 ms cast in the
  local `SpellCastTimes.dbc`. If the default 1.12 UI still shows no enemy cast
  bar, verify with a combat-log/addon/packet view before changing server
  behavior again.
- Repeat caster death tests: direct spell death, Immolate DoT death, melee
  death, and airborne/jump death. Expected: collapse/release UI, DoT icon clears,
  combat and auto attack stop, creature evades/leashes, and relog preserves
  corpse/release state.
- With a second client nearby, confirm observers see spell start/go, spell
  damage or resist logs, target health changes, death updates, and creature
  evade/leash cleanup.

## Current Follow-Ups

- Network P1 remaining: add graceful shutdown entrypoints for auth/world accept
  loops, map runtime update loop, and playerbot planner loop. Current change
  hardens per-session lifecycle/backpressure but does not yet add process-level
  shutdown orchestration.
- Consider making queue capacity and session timeout values configurable once
  real-client/load-test defaults are proven.
- Typed protocol P2 remaining: design the next DTO boundary for item/creature/
  gameobject query responses and update-field/entity block internals. Optional
  cleanup after the component split is to reduce handler signatures and remove
  map/test mirror helpers that stop carrying their weight once the new session
  boundaries settle.
- PvE spell outcome parity still needs broader `spell_proc_event` data support,
  absorbs, immunities/vulnerabilities, and aura modifiers for spell hit/crit,
  damage taken/done, healing, and threat.
- Creature spell parity still needs broader CMaNGOS target selector coverage,
  interrupt/death/leash real-client proof while mid-cast, and dbscript success
  hooks.
- Combat-log polish remains: if generic "enemy hits/crits" lines persist for
  spells, compare exact `SMSG_SPELLNONMELEEDAMAGELOG`, `SMSG_PERIODICAURALOG`,
  `SMSG_SPELLLOGMISS`, and `SMSG_SPELL_GO` payloads against CMaNGOS.
- Starter-zone integration follow-up: GitHub issue #69 tracks the red Kobold
  Camp Cleanup kill-credit smoke. Treat it as separate unless a future death or
  combat-credit change directly touches that path.
- Continue Northshire missing criteria from the playable board: quest
  restrictions, quest item drops from real loot tables, gameobject quest pickup,
  remaining warrior level 1-6 spell parity, combat log feedback, health/rage
  regen, skills/weapon skills, aggro/chase/leash behavior, and patrol runtime
  stability.

## Key Files

- `crates/wow-network/src/auth/mod.rs`
- `crates/wow-network/src/world/mod.rs`
- `crates/wow-network/src/world/handlers/mod.rs`
- `crates/wow-network/src/world/map_runtime/mod.rs`
- `crates/wow-network/src/world/session.rs`
- `crates/wow-network/src/world/server/session_loop.rs`
- `crates/wow-network/src/world/packets.rs`
- `crates/wow-proto/src/world_packets.rs`
- `crates/wow-network/src/observability.rs`
- `crates/wow-network/src/world/tests.rs`
