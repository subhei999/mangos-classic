# CMaNGOS Parity Layout

This directory is intentionally moving toward a CMaNGOS-shaped Rust monolith.
Some subsystems are still live flat files from the early Rust world slice.
Do not create a duplicate subfolder for one of those modules until you are
ready to move the live code mechanically.

`README.md` is the quick orientation map for what is live today. This file is
the CMaNGOS parity map for where behavior should eventually settle.

Keep these mappings easy to follow:

- `server/*`: `src/game/Server/*`
- `entities/*`: `src/game/Entities/*`
- `combat.rs`: transitional live home for `src/game/Combat/*`,
  `src/game/Entities/Unit.*` melee sections, and movement-driven creature
  combat. Future split targets: `combat/combat_handler.rs`,
  `combat/combat_manager.rs`, `combat/threat_manager.rs`, `combat/melee.rs`,
  and `combat/xp.rs`.
- `motion.rs`: current live creature motion-state structs. Future CMaNGOS
  targets include `src/game/MotionGenerators/*`, but do not keep empty Rust
  scaffold files for each generator.
- Movement packet handling currently lives in `handlers/movement.rs`,
  `server/movement.rs`, `packet_builders/movement.rs`, and
  `map_runtime/movement_actor.rs`. Future CMaNGOS targets include
  `src/game/Movement/*`, especially MoveSpline/path serialization.
- Map persistence/spawn behavior currently lives in `map_runtime/*` and DB
  loading helpers. Future CMaNGOS targets include `src/game/Maps/*`, but do not
  keep empty Rust scaffold files for map/spawn managers.
- `map_runtime/state.rs`: live Rust-only owner for per-map runtime state.
- `map_runtime/systems/*`: live Rust-only `MapRuntime` systems while map-owned
  behavior is still being migrated toward the source-shaped modules above.
- `loot.rs`: transitional live home for `src/game/Loot/*`. Future split
  targets: `loot/loot_handler.rs` and `loot/loot_mgr.rs`.
- `quests.rs`: transitional live home for `src/game/Quests/*`. Future split
  targets: `quests/quest_handler.rs` and `quests/quest_def.rs`.
- `spells.rs`: transitional live home for `src/game/Spells/*`. Future split
  targets: `spells/spell_handler.rs`, `spells/spell.rs`,
  `spells/spell_auras.rs`, and `spells/spell_mgr.rs`.
- `chat.rs`: transitional live home for `src/game/Chat/*`. Future split
  targets: `chat/chat_handler.rs`, `chat/channel.rs`, and
  `chat/channel_mgr.rs`.
- `globals/*`: `src/game/Globals/*`
- `reputation/*`: `src/game/Reputation/*`

Prefer behavior-preserving moves first. When porting behavior, keep the CMaNGOS
reference path in the Rust file header or nearby tests so future agents can
trace the parity decision quickly.

Current rule of thumb:

- If a live file exists, add behavior there or first move that behavior into a
  named future split target.
- If a future target only exists in this document, create the file during the
  vertical slice that needs it. Avoid empty source husks; they age badly and
  make the live tree harder to read.
