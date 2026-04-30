# CMaNGOS Parity Layout

This directory is intentionally moving toward a CMaNGOS-shaped Rust monolith.
Some subsystems are still live flat files from the early Rust world slice.
Do not create a duplicate subfolder for one of those modules until you are
ready to move the live code mechanically.

Keep these mappings easy to follow:

- `server/*`: `src/game/Server/*`
- `entities/*`: `src/game/Entities/*`
- `combat.rs`: transitional live home for `src/game/Combat/*`,
  `src/game/Entities/Unit.*` melee sections, and movement-driven creature
  combat. Future split targets: `combat/combat_handler.rs`,
  `combat/combat_manager.rs`, `combat/threat_manager.rs`, `combat/melee.rs`,
  and `combat/xp.rs`.
- `motion/*`: `src/game/MotionGenerators/*`
- `movement/*`: `src/game/Movement/*`
- `maps/*`: `src/game/Maps/*`
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

- If a live flat file exists, add behavior there or first move that behavior
  into the named future split target.
- If only a scaffold file exists, it is safe to fill it during the vertical
  slice that needs that CMaNGOS subsystem.
