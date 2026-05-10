# Playerbot Architecture Plan

This document defines the Rust playerbot direction for the CMaNGOS Classic
migration. Playerbots are a fork feature, but their world interactions must use
the same CMaNGOS-shaped authority boundaries as real players: map-owned state,
DB/DBC/source-derived gameplay values, shared creature/loot/combat ownership,
and packet output built from normal world events.

## Goal

Build high-performance server-side playerbots that can inhabit the world,
roam, fight creatures, loot, and eventually participate in PvPvE without
becoming a parallel gameplay implementation.

The first useful target is intentionally small:

- one or more bots can appear in the real client as player objects;
- bots are advanced by the map/world update loop, not by fake TCP sessions;
- bots can roam inside active map grids using the same geometry/pathing
  guardrails as creatures and players;
- bots can acquire hostile DB creatures, start melee, apply damage, receive
  damage, die or disengage, and loot through the same map-owned systems real
  players use;
- real clients observe bot movement, combat, death, and loot-visible state via
  normal nearby broadcast packets.

## Non-Goals For The First Slice

- Do not build a TCP client swarm as the primary bot architecture. Protocol
  bots are useful later for load and compatibility testing, but world
  population bots should not pay per-socket/per-session costs or duplicate
  `WorldSessionState`.
- Do not add per-bot private creature, loot, threat, movement, quest, or combat
  caches.
- Do not hardcode gameplay stats, spells, loot outcomes, factions, or combat
  constants for production bot behavior.
- Do not make bots a Northshire-only demo. The first proof can run in
  Northshire, but the architecture must be map/gate agnostic.
- Do not let bot AI bypass CMaNGOS parity systems. A bot can choose an intent;
  existing world systems decide whether the action is legal and what happens.

## Core Design

Use in-process, server-side bot actors. A bot is not a network connection. A
bot is a controlled player-like world actor whose controller is the bot runtime
instead of a client socket.

High-level ownership:

| Area | Owner |
| --- | --- |
| Bot roster and population policy | `BotDirector` |
| Bot decision making | `BotBrain` |
| Bot command emission | `BotController` |
| Position, visibility, health, power, combat target, auras | `MapRuntime` |
| Creature combat, threat, motion, corpse, respawn | `MapRuntime` |
| Creature/gameobject loot open/claim/release | `MapRuntime` |
| Gameplay values | DB, DBC, CMaNGOS-derived formulas, existing object managers |
| Packets to real clients | normal nearby map/session dispatch |

The architectural rule is:

```text
bot brain -> world actor command -> map/world authority -> observer packets
```

The bot brain must not directly mutate creature health, loot vectors, quest
status, player inventory, or shared visibility state.

## Player Runtime Shape

The current `MapRuntime` has `PlayerRuntime` entries keyed by character GUID and
each entry carries a `SessionId`. Bots should not be modeled as fake sessions.
Instead, split the controlling endpoint from the visible player object:

```rust
enum PlayerController {
    Client { session_id: SessionId },
    Bot { bot_id: BotId },
}
```

Then `PlayerRuntime` can represent both real players and bots:

```rust
struct PlayerRuntime {
    guid: u32,
    account_id: Option<u32>,
    controller: PlayerController,
    // existing visual, movement, combat, inventory, quest, aura, stat fields
}
```

Direct packets only go to `PlayerController::Client`. Nearby observer packets
go to real client sessions that can see the actor. Bots do not need encrypted
socket output, ping handling, addon data, or `WorldSessionState`.

This avoids the wrong ownership edge where a bot would be a synthetic
`WorldSessionState` with private cached gameplay state.

## Bot Runtime Modules

Initial module layout can stay inside `crates/wow-network/src/world/` until the
feature proves its boundaries:

```text
world/playerbots.rs
world/playerbots/director.rs
world/playerbots/brain.rs
world/playerbots/commands.rs
world/playerbots/runtime.rs
world/playerbots/roster.rs
```

If the implementation grows large, move decision logic into a separate crate
later, but keep map mutations in `wow-network` where `MapRuntime` and packet
builders already live.

### `BotDirector`

Owns population policy:

- which maps/zones can host bots;
- how many bots are desired per active grid or region;
- spawn/despawn eligibility;
- per-map and global CPU/pathing budgets;
- aggression profile distribution;
- debug/admin controls.

The director should be called from the map runtime update loop or a sibling
world update phase. It should avoid full-world scans by using active map/grid
state and cheap counters.

### `BotBrain`

Chooses intent from snapshots, not mutable world references.

Inputs:

- bot snapshot: position, level, health, power, class, combat state;
- nearby creature/player/gameobject summaries;
- route/roam state;
- configured personality/aggression state;
- cooldown/debounce timestamps.

Outputs:

- one or more `WorldActorCommand` values;
- next desired think time.

The brain should be deterministic under injected RNG for tests.

### `BotController`

Converts bot intent into the same command entrypoints available to real player
sessions. It may enrich commands with bot identity, but it must not implement a
separate combat or loot engine.

## Command Layer

Add a shared action boundary that both client handlers and bot controllers can
call over time.

Proposed command enum:

```rust
enum WorldActorCommand {
    MoveTo {
        actor: ObjectGuid,
        destination: WorldPosition,
    },
    StopMoving {
        actor: ObjectGuid,
    },
    AttackStart {
        actor: ObjectGuid,
        target: ObjectGuid,
    },
    AttackStop {
        actor: ObjectGuid,
        target: Option<ObjectGuid>,
    },
    CastSpell {
        actor: ObjectGuid,
        spell_id: u32,
        target: Option<ObjectGuid>,
    },
    LootOpen {
        actor: ObjectGuid,
        target: ObjectGuid,
    },
    LootMoney {
        actor: ObjectGuid,
        target: ObjectGuid,
    },
    LootItem {
        actor: ObjectGuid,
        target: ObjectGuid,
        slot: u8,
    },
    LootRelease {
        actor: ObjectGuid,
        target: ObjectGuid,
    },
    UseGameObject {
        actor: ObjectGuid,
        target: ObjectGuid,
    },
}
```

Early implementation can expose narrower functions first, but the direction is
to remove session-specific gameplay entrypoints and keep socket handlers as
packet decoders plus response emitters.

## Tick And Scheduling Model

Bots must not all think every world tick.

Recommended scheduling:

- map update loop owns bot advancement as a phase after normal lifecycle/motion
  cleanup and before observability snapshots;
- each bot has `next_think_at`;
- thinking cadence depends on state:
  - sleeping/off-grid: no active thinking;
  - roaming: 750-1500 ms;
  - alert/target search: 250-500 ms;
  - combat: align with swing/cast/cooldown deadlines;
  - looting: short burst until loot is resolved, then backoff;
- add jitter to avoid synchronized spikes;
- each map tick has a bot CPU budget and optional max bot count advanced;
- pathfinding requests are budgeted separately and cached/debounced.

The runtime should use `Skip` behavior like the current map ticker so bot load
does not cause unbounded catch-up work after stalls.

## Active Grid Behavior

Bot simulation should follow map activity:

- bots inside active grids are eligible for full AI;
- bots in idle/loaded grids should either sleep or run coarse movement only
  when a population policy needs them to migrate toward active areas;
- bots must count as world actors for visibility and packet broadcasting, but
  whether they keep a grid active is a policy choice.

Initial recommendation: bots should not indefinitely pin grids active. A bot
near real players is active because real players activate the grid. Remote
population can be represented as sleeping roster entries until a real player
approaches or the director explicitly activates a limited region.

## Data And Persistence

Use real gameplay data for bot actors.

Minimum production data:

- bot identity and enabled state;
- character-like race/class/gender/level/name;
- spawn/home/roam region;
- aggression/personality profile;
- optional linked character inventory/equipment source.

Preferred path:

- introduce a small bot roster table or config-backed roster that references
  character-like data;
- hydrate bot runtime through the same stat, spell, skill, equipment, and
  inventory loading helpers used for real players where possible;
- persist only durable player-like state initially: position if desired,
  inventory changes from loot, money, death/recovery state once those systems
  exist.

Do not persist every transient bot movement point. CMaNGOS-style runtime
movement belongs to map state, not durable DB rows.

## Movement

Bot movement should use a player movement authority path, not creature motion
shortcuts.

First slice:

- server chooses destination;
- `MapRuntime` validates/corrects destination through `WorldGeometry`;
- movement is stored as an interruptible time-based leg with start position,
  destination, start time, arrival time, and speed;
- movement is emitted through player movement start/stop packets visible to
  nearby clients, avoiding periodic correction spam for normal straight legs;
- gameplay that needs the bot position during an active leg must first resolve
  the leg to `now`, commit that authoritative position, and then continue.

Future fidelity:

- reuse mmap path generation where appropriate;
- smooth paths into player-like movement updates;
- add stuck handling and destination replanning;
- eventually support mounted speed, swimming, falling, water, transports, and
  path cost preferences.

## Combat

Bots should use the same player combat path as real players.

First creature-combat slice:

- brain selects a hostile DB creature using faction/reaction, distance, alive
  state, and navigation reachability;
- controller emits `AttackStart`;
- map stores bot active combat target and next swing timer;
- map-owned tick processes bot auto attacks through the same melee validation,
  damage outcome, threat, death-finalization, XP/quest-credit eligibility, and
  observer packet flow as real player attacks;
- creature-origin damage can target bots through the same shared player health
  path used for real players.

Important follow-up:

- decide reward/quest credit rules for bots. For early roaming/fighting bots,
  do not grant quest progress unless the bot is explicitly a questing bot with
  a real quest log and inventory state.

PvPvE rules from the fork README come later:

- playerbot aggression chance;
- all bots defend themselves;
- bot-vs-bot combat requires both bots to be aggressive;
- anti-grief level band;
- death drops and recovery.

Those should be implemented as policy checks on top of the shared player-vs-
player combat path once that path exists.

## Loot

Bots should loot through the shared map loot owner.

First slice:

- bot can open a lootable corpse;
- map decides whether loot is still available;
- bot claims money/items through stable loot slots;
- inventory storage uses existing inventory capacity and stack rules;
- loot release updates observers exactly like real clients.

Avoid a bot-only loot claim path. If existing loot handlers are session-shaped,
extract reusable actor-level helpers first.

## Quests And Gameobjects

Do not start with full questing. Questing bots require more durable inventory,
quest log, and route planning than the first architecture proof needs.

When added:

- quest availability uses existing DB-backed restrictions;
- gameobject use emits `UseGameObject`;
- kill/gameobject/item progress uses the same quest mutation code as real
  players;
- quest reward selection uses real reward data and inventory guards.

## Observability

Add lightweight metrics before scaling:

- active bots per map/grid;
- sleeping bots;
- bot think count and duration;
- bot command count by kind;
- path requests, failures, and cache hits;
- target acquisition failures by reason;
- bot combat starts/stops/deaths;
- bot loot opens/claims/releases;
- per-tick bot budget exhaustion.

These belong beside existing world/map runtime observability, not in a separate
debug-only logger.

## Testing Strategy

Focused automated tests should prove architecture boundaries before behavior
gets fancy:

- bot visible player create block is sent to nearby real client sessions;
- bot movement updates cell/grid position and observer visibility;
- bot does not require a `SessionRegistry` entry;
- bot cannot mutate creature health outside `MapRuntime`;
- bot attack uses the same melee validation as real players;
- bot kill creates the same shared corpse/loot lifecycle as a real player kill;
- two bots cannot duplicate a corpse loot claim;
- sleeping bots do not increase DB grid load/query counts;
- bot thinking cadence respects `next_think_at` and jitter/budget limits.

Real-client smoke for the first vertical slice:

- log into Northshire;
- observe one bot spawn as a player;
- watch it roam without snapping or freezing;
- watch it engage a hostile creature;
- verify creature health/death/corpse/loot state matches what the real client
  sees for a real player;
- verify no duplicate loot or stale moving corpse visuals.

## Implementation Phases

### Phase 0: Design And Branch Setup

- Add this architecture doc.
- Update session handoff so playerbots are the active user-directed priority.
- Keep Checkpoint 2 playable gates as regression constraints.

### Phase 1: Player Runtime Controller Split

- Replace mandatory `session_id` in `PlayerRuntime` with `PlayerController`.
- Keep client behavior identical.
- Prove real players still spawn, move, broadcast, attack, and receive direct
  packets.

Status: implemented on `codex/playerbots-map-actor-foundation`.
`PlayerRuntime` now carries `PlayerController::Client { session_id }` or
`PlayerController::Bot { bot_id }`, and map packet fan-out only sends direct or
observer packets to client-controlled players. A focused unit test proves a
bot-controlled player-like runtime can be visible to a nearby client without a
direct session.

### Phase 2: Bot Actor Visibility

- Add minimal `BotId`, bot roster fixture, and map insertion path.
- Spawn one configured bot as a player-like map actor.
- Broadcast create/destroy/movement to real clients.
- No combat yet.

Status: implemented on `codex/playerbots-map-actor-foundation`.
`WorldServer` now accepts `WorldServerOptions` with a configured playerbot
roster, hydrates each bot through DB-backed player level stats, inserts the
bot-controlled `PlayerRuntime` into `MapRuntime`, and answers name queries from
the bot roster. `config/worldserver.local.toml` enables one Northshire
visibility bot; the default `config/worldserver.toml` keeps playerbots
disabled and documents the roster shape.

Load-test extension: implemented as a deterministic config-backed swarm under
`[playerbots.random]`. It appends generated bot spawn configs to the explicit
roster and still hydrates every bot through the normal DB-backed runtime path.
This is a development/test population tool, not the final production director
policy from Phase 6.

### Phase 3: Bot Movement Tick

- Add bot brain cadence and simple roam intent.
- Route destination correction through `WorldGeometry`.
- Update map cell/grid buckets as bots move.
- Add observability and budget counters.

Status: implemented on `codex/playerbots-map-actor-foundation`.
`PlayerRuntime` now carries a small `PlayerbotRuntimeState` with home position,
active movement leg, `next_think_at`, and roam step. The map update loop starts
due bot legs only in real-client-active grids, corrects simple roam destinations
through `WorldGeometry`, and stores a time-based leg at normal player run speed
of 7 yards per second across a 10-yard square. Normal roam legs now emit
`MSG_MOVE_START_FORWARD` at leg start and `MSG_MOVE_STOP` at arrival instead of
periodic heartbeat correction packets. Arrival commits the bot's authoritative
position through the existing map-owned `update_player_position` path so cell
buckets, visible player sets, and normal observer packets stay shared. Bot
movement is budgeted per map tick, timed under playerbot movement observability
phases, and counted in the map runtime snapshot without making bots client grid
interest.

### Phase 4: Bot Creature Combat

- Add hostile target acquisition from active map cells.
- Emit shared attack-start and auto-swing commands.
- Reuse player melee validation/damage/threat/death-finalization.
- Route creature-origin damage to bot health through shared player runtime.

### Phase 5: Bot Loot

- Extract actor-level loot helpers from session handlers where needed.
- Let bots open, claim, and release DB creature loot.
- Prove stable slot semantics and no duplicate claims.

### Phase 6: Scaling And Population Policy

- Add director-driven spawn/despawn policy.
- Add grid-aware sleeping.
- Add pathing budgets and cached route attempts.
- Add configurable population caps.

Pre-work: local deterministic swarm generation exists for load testing hundreds
of map-owned bot actors in one configured area. Phase 6 should replace that
with a director that can activate, sleep, and migrate populations without
pinning arbitrary remote grids active.

### Phase 7: PvPvE Personality

- Add aggression/defense policy.
- Add player-vs-bot and bot-vs-bot engagement rules.
- Add hardcore death-drop behavior only after the shared player-vs-player and
  inventory/drop systems can support it correctly.

## Current Recommended Next Task

Start Phase 4: bot creature combat. Phase 3 added a map-owned bot movement
cadence and shared movement broadcast path. The next architecture-correct move
is target acquisition plus attack start/auto-swing for bot-controlled players,
reusing the existing player melee validation, creature threat, damage,
death-finalization, and observer packet paths.

Suggested branch:

```text
codex/playerbots-map-actor-foundation
```

Suggested first proof:

- pick one nearby hostile DB creature in active map cells;
- emit a shared attack-start/auto-swing command for the bot actor;
- reuse player melee validation and `MapRuntime` creature damage/threat
  ownership;
- broadcast normal attack/combat/damage updates to nearby real clients;
- keep reward, loot, PvP, quest credit, and fake-session behavior out of this
  slice.
