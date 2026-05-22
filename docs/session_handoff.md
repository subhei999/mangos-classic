# Session Handoff

Short operating brief for the next Rust migration session. Durable roadmap
history belongs in `docs/rust_migration_plan.md`, gate status in
`docs/playable_gate_board.md`, and focused feature plans in their own docs.

## Current Branch And State

- Branch: `codex/rusty-mangos`
- Workspace: `C:\Users\subhe\Documents\New project`
- Latest pushed checkpoint before the current uncommitted work:
  `0311b79b0 Implement trainer and rest parity fixes`
- Current uncommitted state fixes a live Stormwind area-trigger disconnect:
  - `is_tavern_area_trigger` now decodes `SELECT 1` as `i32` instead of `u8`
    because MySQL returns integer literals as `INT`.

## Current Goal

Latest user-directed priority: fix a live-client disconnect when entering
Stormwind after the tavern area-trigger rest implementation.

## What Changed Recently

- Trainer buys now do more than add the learned spell:
  - trainer list state checks required skill value instead of marking every
    `reqskill` row red,
  - trainer spell visibility/buy checks consult DBC `SkillLineAbility` and
    `SkillRaceClassInfo` race/class masks when available,
  - buying a proficiency spell now creates or updates the corresponding
    `character_skills` row using CMaNGOS skill range rules, then sends a player
    skill update.
- Equipment moves now reject final states where a two-handed main-hand weapon
  and any offhand item would both be equipped, returning the CMaNGOS
  `EQUIP_ERR_CANT_EQUIP_WITH_TWOHANDED` result.
- Player corpse state now arms a six-minute server-side auto-repop deadline.
  The world session loop wakes on that deadline and reuses the normal Release
  Spirit flow to create the corpse, move the ghost to the graveyard, send corpse
  packets, and persist death state.
- `CMSG_AREATRIGGER` is now parsed and routed. Tavern triggers from
  `areatrigger_tavern` set `PLAYER_FLAGS_RESTING`, send `SMSG_SET_REST_START`,
  update the visible player flags, and sync map-owned player state so logout is
  instant in designated tavern rest areas.
- The previous inventory/bag model and vendor fixes were committed and pushed as
  `e3142f260`.
- Live Stormwind testing found that the tavern area-trigger DB lookup decoded
  `SELECT 1` into `u8`, but MySQL exposes that literal as `INT`, ending the
  world session with a SQLx type mismatch. The lookup now decodes into `i32`.

## Tests Run

- `cargo fmt -p wow-network -p wow-proto -p wow-db`
- `cargo check -p wow-network`
- `.\scripts\restart-game-stack.cmd --release`
- Focused Rust tests:
  - `cargo test -p wow-network trainer_spell_state_marks_known_level_and_requirement_gates -- --nocapture`
  - `cargo test -p wow-network two_handed_weapons_conflict_with_offhand_equipment_like_cmangos -- --nocapture`
  - `cargo test -p wow-network corpse_state_arms_auto_repop_deadline -- --nocapture`
  - `cargo test -p wow-network trained_skill_initial_values_follow_cmangos_range_types -- --nocapture`
  - `cargo test -p wow-network parse_world_client_packet_decodes_control_requests -- --nocapture`
- `.\scripts\test-rust.cmd`
  - clippy/check portions pass,
  - broad test run reaches the same two pre-existing local DB-auth failures:
    - `world::tests::map_runtime_manager_advances_3196_event_ai_immolate_with_delayed_completion`
    - `world::tests::map_runtime_direct_completion_after_manager_started_3196_immolate_does_not_hang`
  - failure cause remains local MySQL auth:
    `1698 (28000): Access denied for user 'root'@'localhost'`

## Known Blockers / Unproven Areas

- Full workspace green is still blocked locally by the unrelated MySQL auth
  issue on the two EventAI immolate tests above.
- Tavern rest handling trusts the client area-trigger opcode plus
  `areatrigger_tavern`; Rust still does not validate the trigger geometry
  against `AreaTrigger.dbc` coordinates like CMaNGOS.
- Rest state clearing outside taverns/cities remains incomplete because the
  broader zone/area rest-type system is not wired yet.
- Two-hander/offhand conflict is intentionally conservative: Rust rejects the
  invalid final equipment state instead of auto-unequipping/storing offhand gear
  when equipping a two-hander.
- Trainer skill unlocks rely on loaded DBC skill maps for race/class skill
  eligibility. If those maps are absent, non-skill trainer spells continue to
  work, but proficiency skill creation cannot be inferred.

## Recommended Next Task

Recommended next task: live-client smoke this pass:

- buy a weapon skill from a valid trainer and confirm the skill appears and the
  weapon type equips,
- verify a class that should not learn that weapon type cannot buy it,
- attempt shield/held/offhand equipment with a two-handed weapon in both orders,
- die and wait for the six-minute release timer to expire,
- enter a tavern, log out, and confirm logout is instant.

After that, wire the remaining rest-type clearing/city rest detection and add
AreaTrigger DBC geometry validation if area-trigger spoofing or stale rest
flags show up in live testing.

## Key Files

- `crates/wow-network/src/world/handlers/trainer.rs`
- `crates/wow-network/src/world/handlers/inventory.rs`
- `crates/wow-network/src/world/handlers/death.rs`
- `crates/wow-network/src/world/handlers/misc.rs`
- `crates/wow-network/src/world/server/session_loop.rs`
- `crates/wow-network/src/world/spells/skills.rs`
- `crates/wow-network/src/world/packets.rs`
- `crates/wow-proto/src/world_packets.rs`
- `crates/wow-db/src/world_data.rs`
- `src/game/Entities/MiscHandler.cpp`
- `src/game/Entities/Player.cpp`
- `src/game/Globals/ObjectMgr.cpp`
