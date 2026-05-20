# World Module Layout

`wow-network/src/world` is the Rust worldserver implementation. It is still a
monolith, but it has three different kinds of modules:

- Live runtime code: compiled modules that own current server behavior.
- Parity scaffolds: tiny CMaNGOS-shaped files that mark future homes.
- Packet/data helpers: protocol builders, decoded packet shapes, and shared
  constants that support the runtime.

## Live Runtime Areas

- `server/`: world socket/session loop, login/logout flow, dispatch, and
  session-side visibility helpers.
- `session.rs` and `session_runtime.rs`: per-session mutable state and shared
  runtime dependencies passed to server loops and handlers.
- `handlers/`: client opcode handlers grouped by gameplay topic.
- `map_runtime/`: map-owned simulation state and ticking.
- `combat/`: combat, threat, aggro, evade, melee, and creature motion helpers.
- `entities/`: update-object builders and active entity packet shapes.
- `spells/`: spell metadata, targeting, aura, cooldown, and effect behavior.
- `packet_builders/`: server packet serialization helpers.
- `globals/` and `social/`: shared world managers.

## Map Runtime

`map_runtime/state.rs` defines `MapRuntime` and the runtime state structs owned
by a single map/instance.

`map_runtime/systems/` contains focused `impl MapRuntime` files for the live
systems that mutate or query that state, such as players, creature combat,
creature motion, dynamic objects, loot, and spatial lookups.

`map_runtime/map_manager/` owns access to all `MapRuntime` instances and
provides the async facade used by sessions, handlers, and server ticks. Its
submodules group the facade by player, creature, grid, spell, and tick entry
points while keeping `MapRuntimeManager` as the caller-facing type. Creature
facade entry points are further grouped under `map_manager/creatures/` by
combat, loot, motion, snapshots, spells, and gameobjects.

## Spells

`spells.rs` is the spell-system facade. Large implementation areas live under
`spells/`, and spell effect handling is further grouped under
`spells/effects/` by dispatch, coverage, damage, auras, items, area/channel
effects, movement, healing, and utility effects.

## Parity Scaffolds

`PARITY_LAYOUT.md` tracks CMaNGOS reference mappings and future split targets.
Do not keep empty source files only to reserve a future home. When a vertical
slice needs one of those targets, create it with live behavior in a mechanical
move first, then make the parity change in a separate focused step.
