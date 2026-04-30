# G12 Derisk Multiplayer / Shared MapRuntime Plan

This document is the working plan for the user-directed G12 milestone. It is
the reference to follow before implementing shared multiplayer world state.

## Current Diagnosis

The repo is ready for this, but the multiplayer/grid seam is currently in the
wrong place.

The branch already has a strong CMaNGOS-shaped world split: player, creature,
corpse, motion, world-data, and navigation modules are included from
`world/session.rs`. But `WorldRuntimeState` only shares online characters,
player corpses, delete options, and data-file inspection. Meanwhile
`WorldSessionState` owns the active player, visible DB creatures, creature
combat state, session-local creature map, quest state, inventory, visibility
scan positions, and navigation guardrail. That means creature visibility and
combat are still largely per-session, not one shared world.

The server itself is still a per-connection async loop: `WorldServer::run`
accepts a socket, clones DB pools and runtime state, then `tokio::spawn`s
`handle_client`. Inside `handle_client`, each session owns its
`WorldSessionState` and runs its own `handle_combat_tick`. That is fine for a
monolith, but multiplayer needs shared map state between those session loops.

The playable gate board confirms exactly where this lands: G3 movement
visibility streaming is green, while G12 multi-client sanity is still red. G12's
minimum proof is two clients in Northshire seeing each other enter, move,
logout, share local chat, and observe one shared creature state without
loot/quest/combat divergence.

The current bootstrap creates the self player, fixture NPCs, nearby DB
creatures, player corpses, and inventory item objects, but it does not include
nearby live players. Chat is similarly solo-shaped today:
`handle_message_chat` builds `SMSG_MESSAGECHAT`, but sends it only back to the
same stream instead of broadcasting to nearby players.

For load control, the current DB-side creature lookup is a spatial query by
map, x/y, radius, and limit, joined to `creature_template`. That is fine for
early slices, but it is not the right hot path for many players because
movement visibility should not hit the DB on every player's heartbeat.

## CMaNGOS Shape To Copy

CMaNGOS' core shape is the right reference:

- CMaNGOS uses 64 map grids.
- Each grid is 533.33333 yards.
- Each grid has 16 cells.
- Each cell is roughly 33.333 yards.
- It computes grid/cell pairs from world coordinates.
- It stores world objects versus grid objects in separate containers.

`Map::Add(Player*)` ensures the grid is loaded, adds the player to the
grid/world, sends initial self state, triggers the player viewpoint, and updates
object visibility. CMaNGOS also has `Cell::CalculateCellArea` and cell visitor
helpers for visiting nearby grid/world objects by radius. For broadcast,
CMaNGOS uses map/cell visitors like `MessageBroadcast` and
`MessageDistBroadcast` rather than asking every session to discover recipients
itself.

For Rust, copy the shape, not the pointer-heavy implementation.

## Target Architecture

Keep the existing monolithic worldserver. Add one shared world runtime under
`WorldRuntimeState`:

```rust
struct WorldRuntimeState {
    online_characters: OnlineCharacters,
    player_corpses: PlayerCorpses,
    delete_options: CharacterDeleteOptions,
    world_data_files: Arc<WorldDataFiles>,

    maps: Arc<MapRuntimeManager>,
    sessions: Arc<SessionRegistry>,
}
```

Key new pieces:

```rust
struct SessionRegistry {
    sessions: HashMap<SessionId, SessionHandle>,
}

struct SessionHandle {
    account_id: u32,
    character_guid: Option<u32>,
    outbound: mpsc::UnboundedSender<OutboundWorldPacket>,
}

struct MapRuntimeManager {
    maps: HashMap<(u32, u32), Arc<Mutex<MapRuntime>>>, // (map_id, instance_id)
}

struct MapRuntime {
    map_id: u32,
    instance_id: u32,

    grids: HashMap<GridCoord, GridRuntime>,
    players: HashMap<u32, PlayerRuntime>,
    creatures: HashMap<u32, DbCreatureRuntime>,
    corpses: HashMap<u64, PlayerCorpseRuntime>,
}

struct GridRuntime {
    state: GridState,
    cells: HashMap<CellCoord, CellRuntime>,
    active_player_count: u32,
    last_touched: Instant,
}

struct CellRuntime {
    players: HashSet<u32>,
    creatures: HashSet<u32>,
    corpses: HashSet<u64>,
}

struct PlayerRuntime {
    guid: u32,
    account_id: u32,
    session_id: SessionId,
    position: WorldPosition,
    cell: CellCoord,
    visible_objects: HashSet<ObjectGuid>,
    visual: PlayerVisualState,
    flags: u32,
    level: u8,
    race: u8,
    class: u8,
    gender: u8,
}
```

The session stays responsible for account auth, character ownership, inventory,
quest status, packet parsing, and DB persistence. The map runtime owns the
shared spatial world: live players, loaded creatures, corpses, cell membership,
and which sessions should receive world packets.

Most important rule: never hold the map lock while awaiting socket writes. Map
operations should collect `Vec<(SessionId, OutboundWorldPacket)>`, release the
lock, then dispatch through `SessionRegistry`.

## Implementation Plan

### Phase 1: Add Outbound Session Channels Without Gameplay Changes

Today, every handler directly calls `send_packet(stream, ...)`. That makes it
hard for another player's session or the map runtime to send packets to this
client.

First split each world connection into:

- reader loop: reads client packets and calls handlers;
- writer task: receives `OutboundWorldPacket` and calls `send_packet(...)`.

The current session loop can still call a helper like:

```rust
session_outbound.send(opcode, body)?;
```

Do this as a behavior-preserving refactor. The world should work exactly as
before, but now the shared map can send packets to any connected client.

Deliverable:

- `SessionId`
- `SessionRegistry`
- `OutboundWorldPacket`
- one writer task per connected client
- no multiplayer behavior yet

Tests:

- existing `test-rust`
- existing `test-world-flow`
- existing `test-starter-zone-flow`

### Phase 2: Register Live Players In MapRuntime

On `CMSG_PLAYER_LOGIN`, after loading the character and sending the existing
bootstrap, register the player in `MapRuntime`.

On login:

- compute grid/cell from `WorldPosition`;
- ensure the grid exists;
- add `PlayerRuntime`;
- find nearby already-online players;
- send their create blocks to the new player;
- send the new player's create block to those nearby players.

This needs a non-self player create block:

```rust
fn build_other_player_create_block(player: &PlayerRuntime) -> anyhow::Result<Vec<u8>>
```

It should reuse the self-spawn player field logic where possible, but must not
set `UPDATEFLAG_SELF`. For the first slice, it only needs enough fields for a
nearby player to render, select, name-query, and move: GUID, type mask,
display/race/class/gender bytes, level, faction, health/power, movement speeds,
position, visible equipment fields, and flags.

On logout/disconnect:

- persist the character as today;
- remove player from `MapRuntime`;
- send `SMSG_DESTROY_OBJECT` for that player to sessions that had the player
  visible.

Deliverable:

- two players can see each other appear and disappear;
- no movement forwarding yet.

Automated proof:

- add a two-client harness, probably `bins/multiclient-world-flow-test`;
- create/login account A and account B;
- A logs in;
- B logs in;
- B receives A create block;
- A receives B create block;
- B logs out;
- A receives `SMSG_DESTROY_OBJECT`.

This is the first real G12 step.

### Phase 3: Broadcast Player Movement And Maintain Player Visibility Diffs

Movement packets already parse into `MovementInfo`, and the opcode set is
centralized. After the current movement handler updates the active character
position, it should call:

```rust
map_runtime.update_player_position(character_guid, new_position)
```

That operation should:

- recompute grid/cell;
- update player cell membership if changed;
- compute nearby player visibility from cells;
- emit create packets for newly visible players;
- emit destroy packets for players no longer visible;
- emit a movement packet to currently visible players.

Be careful with packet shape here. Do not blindly forward the client's exact
movement payload until it is proven against the Vanilla/CMaNGOS movement
broadcast format. Build a server movement-broadcast helper and compare it
against the CMaNGOS movement path before relying on real-client smoke.

Deliverable:

- if B walks or turns near A, A sees B move;
- if B crosses the visibility radius, A gets create/destroy updates;
- movement persistence remains unchanged.

Automated proof:

- two-client harness sends movement from B;
- A observes the movement opcode/update;
- B walks outside visibility range;
- A receives destroy;
- B walks back in;
- A receives create.

### Phase 4: Implement CMaNGOS-Shaped Grid/Cell Primitives

Add Rust modules shaped like:

- `crates/wow-network/src/world/maps/grid.rs`
- `crates/wow-network/src/world/maps/cell.rs`
- `crates/wow-network/src/world/maps/map_runtime.rs`
- `crates/wow-network/src/world/maps/map_manager.rs`

Constants should match CMaNGOS:

```rust
const MAX_NUMBER_OF_GRIDS: u32 = 64;
const SIZE_OF_GRIDS: f32 = 533.33333;
const MAX_NUMBER_OF_CELLS: u32 = 16;
const SIZE_OF_GRID_CELL: f32 = SIZE_OF_GRIDS / MAX_NUMBER_OF_CELLS;
```

Implement:

- `compute_grid_pair(x, y)`
- `compute_cell_pair(x, y)`
- `calculate_cell_area(x, y, radius)`
- `visit_nearby_cells(map, position, radius, visitor)`
- distance-squared filtering after cell candidate lookup

Use the same conceptual distinction as CMaNGOS:

- players, pets, resurrectable corpses: world objects;
- creatures, gameobjects, bones: grid objects.

Deliverable:

- all player-player visibility uses cells, not full scans;
- player create/destroy remains green;
- no DB-grid loading yet.

### Phase 5: Replace Movement-Time DB Creature Scans With Lazy Grid Loading

Right now, nearby creatures are discovered with a DB spatial radius query. That
is okay for G3, but not for many players.

Add DB helpers:

- `get_creature_spawns_in_grid_rect(map, min_x, max_x, min_y, max_y)`
- `get_creature_respawn_times_for_grid(...)`

When a player enters a grid or a cell area requiring neighboring grids:

- `MapRuntime::ensure_grid_loaded(grid)`;
- load all DB creatures whose spawn positions fall in that grid bounds;
- join template data once;
- apply `creature_respawn` rows;
- create `DbCreatureRuntime` entries;
- insert creature GUIDs into cell buckets.

Movement visibility should then compute nearby creatures from loaded cells, not
query the DB.

Grid states:

```rust
enum GridState {
    Unloaded,
    Loading,
    Loaded,
    Active,
    Idle { unload_after: Instant },
}
```

Keep a grid active while:

- at least one player is in or near it;
- any creature in it is in combat;
- any object has an active timer requiring updates;
- a corpse/loot state is still relevant to visible players.

Unload only idle grids. Persist respawn state exactly as the existing creature
respawn code already does; do not persist every transient creature position yet
unless needed for patrol/combat parity.

Deliverable:

- G3 still passes;
- creature streaming uses loaded grid/cell state;
- DB query count drops from movement-driven to grid-load-driven.

Automated proof:

- existing starter-zone movement streaming test still passes;
- add instrumentation: repeated heartbeats inside one loaded grid should not
  call `get_nearby_creature_spawns`;
- moving across a grid boundary triggers one grid load.

### Phase 6: Move DB Creature Runtime And Combat State Out Of WorldSessionState

This is the most important correctness step for shared mobs.

Status: started. `MapRuntime` now preserves shared DB-creature snapshots across
sessions and player-caused melee/starter-spell health or death updates are
written back to the map and broadcast to nearby sessions. DB-creature loot
open, money claim, item claim, item restore on failed autostore, and release
now mutate the shared map creature snapshot. DB-creature combat claims are also
exclusive in `MapRuntime`, preventing one creature from entering separate
private combat loops for different sessions. The active creature tick mirrors
attacker/victim/next-swing state from `MapRuntime`, and ready-swing retry plus
next-swing timing are written back to the shared map. Player melee and
supported starter spell damage both enter retaliation through this shared claim
path, including victim-wide cleanup on player death. Creature-origin damage
packet execution and lifecycle finalization still run in the owning session
tick, but creature combat-start, in-combat flag, facing, chase, evade, and
return-home packets now broadcast through `MapRuntime` to nearby observers.
Session-local DB creature caches refresh from shared snapshots before local
ticks, and movement visibility refreshes existing local creatures from shared
snapshots instead of letting stale session copies keep patrolling. Creature
death also emits a motion-stop packet to nearby observers so a killed mob does
not continue a previously queued chase or patrol spline in another client.
Idle/random/waypoint and return-home motion now write their updated creature
snapshot back into `MapRuntime`, and new idle motion starts broadcast
`SMSG_MONSTER_MOVE` through the shared map to nearby observers so clients share
the same patrol spline instead of rebuilding private per-session patrols. Lazy
grid loading, observer loot-flag polish after claims, and full two-client
shared-mob harness proof remain follow-up work.

Move these from `WorldSessionState` into `MapRuntime`:

- `db_creatures: HashMap<u64, DbCreatureRuntime>`
- `active_creature_combats: HashMap<u64, CreatureCombatState>`
- creature life state
- creature loot state
- creature movement/chase/home state

The session still owns:

- player inventory;
- quest statuses;
- learned spells;
- active selected target;
- player-specific combat intent;
- player health/rage/mana until full `PlayerRuntime` centralization is needed.

Creature events should become map events:

```rust
enum MapEvent {
    CreatureValuesChanged { guid, update_body },
    CreatureMoved { guid, movement_body },
    CreatureDied { guid, update_body },
    CreatureRespawned { guid, create_body },
    CreatureDestroyed { guid },
}
```

When a creature takes damage or dies:

- mutate the one authoritative `DbCreatureRuntime`;
- build update packets;
- broadcast to all sessions whose `visible_objects` contains that creature;
- grant quest/XP only to eligible participants;
- expose loot through one shared loot state.

This prevents:

- client A killing a wolf while client B still sees it alive;
- duplicated loot;
- duplicated respawn timers;
- per-session creature combat divergence.

Deliverable:

- two clients see the same creature health, death, corpse, lootable state,
  corpse despawn, and respawn.

Automated proof:

- A and B see the same wolf;
- A damages wolf;
- B receives health update;
- A kills wolf;
- B receives death/dynamic flag update;
- A loots money/item;
- B cannot loot already-taken money/item;
- respawn is observed by both if both are nearby.

### Phase 7: Broadcast Local Chat And Emotes Through The Map

Change `handle_message_chat` from direct echo to:

```rust
map_runtime.broadcast_chat_radius(
    sender_guid,
    chat_radius(chat_type),
    build_message_chat_body(...),
)
```

For the first slice:

- SAY: nearby players only;
- YELL: larger radius;
- EMOTE: nearby players;
- still send to self if CMaNGOS does for that chat type;
- keep unsupported chat types ignored as today.

Text emotes should follow the same pattern: animation packet plus text emote
packet to nearby visible players, not just self.

Deliverable:

- A says `/say hello`;
- B sees it if nearby;
- C does not see it if outside radius.

This is a quick G12 win once the session registry and map broadcast path exist.

Status: `/say` is implemented and harness-proven. `YELL`, emotes, and text
emotes still need follow-up slices if they become the next multiplayer chat
priority.

### Phase 8: Add The Multi-Client Gate Harness

The current starter-zone harness is rich but single-client-shaped. It logs in
one `WorldClient`, runs movement streaming, combat, quest, trainer, death, and
persistence checks. Add a separate multi-client harness instead of bloating the
existing one too much:

- `bins/multiclient-world-flow-test`
- `scripts/test-multiclient-world-flow.cmd`
- `scripts/test-multiclient-world-flow.sh`

Minimum scripted flow:

- seed two accounts and two Human Warriors;
- connect both clients through auth/world;
- login A to Northshire;
- login B to Northshire;
- assert mutual player create blocks;
- send B movement, assert A observes it;
- send A `/say`, assert B observes it;
- have both clients observe the same DB creature;
- A damages/kills it, B observes health/death;
- A loots, B cannot duplicate loot;
- logout B, A receives destroy;
- relog B, A receives create again.

This directly maps to G12's definition of done.

## Suggested Commit Ladder

1. Outbound packet channel refactor.
   - No behavior change.
   - Direct `TcpStream` writes replaced by session outbound sender where needed.
2. `MapRuntime` skeleton.
   - `MapRuntimeManager`.
   - Grid/cell coordinate types.
   - Player registration/unregistration.
   - No gameplay behavior change yet.
3. Player enter/logout visibility.
   - Nearby player create blocks.
   - Logout destroy packets.
   - Two-client login/logout harness.
4. Player movement broadcast.
   - Map position updates.
   - Player visibility diff.
   - Movement packet builder.
   - Two-client movement harness.
5. Grid constants and cell-area visitor.
   - CMaNGOS grid/cell constants.
   - Radius candidate lookup.
   - No DB hot-path changes yet.
6. Lazy DB creature grid loading.
   - Grid-load DB query.
   - Loaded-grid creature buckets.
   - Movement visibility uses cells.
   - G3 regression test still green.
7. Shared creature runtime.
   - Move DB creature state/combat/loot to `MapRuntime`.
   - Broadcast creature updates.
   - Two-client shared mob proof.
8. Local chat broadcast.
   - Done for `/say` via map radius.
   - Two-client chat proof covers nearby delivery and out-of-range
     non-delivery.
   - YELL/EMOTE remain follow-up chat coverage.
9. Real-client G12 pass.
   - Two real WoW clients in Northshire.
   - See each other, move, chat, share one creature state.
   - Record result in the gate board/handoff.

## Design Guardrails

Avoid building multiplayer as packet mirroring between sessions. The
authoritative world should be:

```text
WorldServer
  └── WorldRuntimeState
        ├── SessionRegistry
        └── MapRuntimeManager
              └── MapRuntime
                    └── GridRuntime
                          └── CellRuntime
```

Sessions are clients of the map. They should not own the map.

Also avoid holding one giant lock across I/O. A simple
`Arc<Mutex<MapRuntime>>` per map is good enough for this milestone if every
operation follows:

1. lock map;
2. mutate state;
3. collect outbound packets;
4. unlock map;
5. send packets through session channels.

That keeps the architecture monolithic, keeps the implementation
understandable, and leaves a clean path to per-map or per-grid sharding later
without changing the external gameplay model.

## Near-Term Done Definition

For this milestone, call it done when:

- two clients can log into Northshire at once;
- both clients see each other spawn;
- both clients see each other move;
- one client logging out destroys that player for the other;
- `/say` works between nearby players;
- both clients observe the same DB creature state;
- one client killing/looting a mob cannot duplicate or desync the mob for the
  other;
- the existing G3 movement visibility and starter-zone flow tests remain green;
- creature visibility no longer depends on DB radius queries per movement
  heartbeat.
