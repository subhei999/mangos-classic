# Session Handoff

Short operating brief for the next Rust migration session. Durable roadmap
history belongs in `docs/rust_migration_plan.md`, gate status in
`docs/playable_gate_board.md`, and focused feature plans in their own docs.

## Current Branch And State

- Branch: `codex/rusty-mangos`
- Workspace: `C:\Users\subhe\Documents\New project`
- Latest pushed checkpoint before the current uncommitted work:
  `58851c5fd Fix tavern area trigger lookup decode`
- Current uncommitted state implements the broader CMaNGOS-style rest system:
  session-owned rest type/state, city/tavern rest entry, offline/online rested
  XP accrual, rested XP consumption on creature kills, DB persistence, and
  AreaTrigger DBC geometry validation for leaving taverns.

## Current Goal

Latest user-directed priority: finish the full rest area system, including
rested XP gain, in CMaNGOS parity.

## What Changed Recently

- Rest state is now modeled explicitly instead of treating
  `PLAYER_FLAGS_RESTING` as the only source of truth:
  - `RestType` tracks no rest, tavern rest, and city rest.
  - `rest_bonus`, `time_inn_enter`, `inn_trigger_id`, and `next_level_xp` live
    in session state.
  - `characters.rest_bonus`, `logout_time`, and `is_logout_resting` are loaded
    and saved.
- CMaNGOS rest math is wired:
  - online rest ticks every 10 seconds while resting,
  - offline rest uses full tavern/city rate or wilderness divided by 4,
  - bonus is capped at `next_level_xp * 1.5 / 2.0`,
  - rest state byte and `PLAYER_REST_STATE_EXPERIENCE` update with the bonus.
- `CMSG_ZONEUPDATE` is parsed and routed. AreaTable entries with
  `AREA_FLAG_CAPITAL` set city rest; non-capital zone updates clear non-tavern
  rest like CMaNGOS.
- Tavern `CMSG_AREATRIGGER` now delegates to the same rest-type setter, and
  city rest takes precedence over tavern triggers.
- `AreaTrigger.dbc` is loaded and checked with CMaNGOS radius/box geometry on
  movement; if a tavern-resting player leaves the stored inn trigger, Rust now
  clears `PLAYER_FLAGS_RESTING` and syncs the player state update so the client
  portrait ZZZ icon drops.
- Online rested XP is now updated from both idle world ticks and packet-driven
  world ticks, so active/moving players still cross the CMaNGOS rested bonus
  threshold and receive the blue XP bar state update.
- Map/session gameplay sync now mirrors rested XP and `PLAYER_BYTES_2` back to
  map-owned player runtime state. Without this, live session refreshes could
  pull stale map rest data back over the session and keep the XP bar from ever
  crossing the rested threshold.
- Creature kill XP consumes rested bonus for both solo and group kill rewards.
  Quest/exploration XP remains non-rested, matching CMaNGOS.
- The previous trainer, two-hander/offhand, auto-release, tavern-entry, and
  Stormwind area-trigger SQL decode fixes are already committed and pushed.

## Tests Run

- `cargo fmt -p wow-network -p wow-proto -p wow-db`
- `cargo check -p wow-network`
- Focused Rust tests:
  - `cargo test -p wow-network area_trigger_dbc_parser_reads_cmangos_geometry -- --nocapture`
  - `cargo test -p wow-network rest_bonus_math_follows_cmangos_bubble_rate_and_cap -- --nocapture`
  - `cargo test -p wow-network rest_update_packet_marks_xp_bar_rested_when_bonus_crosses_threshold -- --nocapture`
  - `cargo test -p wow-network map_runtime_session_sync_persists_rested_xp_visual_state -- --nocapture`
  - `cargo test -p wow-network xp_gain_packets_match_vanilla_shapes -- --nocapture`
  - `cargo test -p wow-network parse_world_client_packet_decodes_control_requests -- --nocapture`
  - `cargo test -p wow-network self_spawn_update_includes_cmangos_player_vitals_and_defaults -- --nocapture`
- `.\scripts\test-rust.cmd`
  - clippy/check portions pass,
  - broad test run now reaches only the same two pre-existing local DB-auth
    failures:
    - `world::tests::map_runtime_manager_advances_3196_event_ai_immolate_with_delayed_completion`
    - `world::tests::map_runtime_direct_completion_after_manager_started_3196_immolate_does_not_hang`
  - failure cause remains local MySQL auth:
    `1698 (28000): Access denied for user 'root'@'localhost'`

## Known Blockers / Unproven Areas

- Full workspace green is still blocked locally by the unrelated MySQL auth
  issue on the two EventAI immolate tests above.
- City rest depends on client `CMSG_ZONEUPDATE` and AreaTable DBC flags. Live
  smoke should confirm Stormwind/Ironforge/etc. enter instant logout and leave
  rest state correctly when changing zones.
- Tavern rest now validates against `AreaTrigger.dbc` on movement, but should
  still be live-smoked in a real inn to confirm the portrait icon clears at the
  same boundary the vanilla client expects.
- CMaNGOS only sets `REST_STATE_RESTED` for the XP bar when `rest_bonus > 10`.
  Entering a rest area with no stored bonus shows the portrait/resting flag
  first; the XP bar turns blue after enough rested XP accrues or immediately on
  login if stored rested XP already exceeds that threshold.
- Rest rates currently use CMaNGOS default `1.0` behavior in code; config keys
  for custom rest rates are not wired yet.

## Recommended Next Task

Recommended next task: live-client smoke the rest system:

- enter a tavern and confirm instant logout plus rested state,
- enter a capital city and confirm instant logout without a tavern trigger,
- log out in a rest area, wait briefly, log in, and confirm rest bonus persists,
- kill a creature while rested and confirm XP log shows total XP with base kill
  XP preserved,
- leave a city/non-tavern rest area and confirm delayed logout returns.
- leave a tavern and confirm the portrait ZZZ icon clears immediately after
  crossing the inn trigger boundary.
- stay active in a rest area until rested bonus exceeds 10 XP, or log in with
  stored rested XP, and confirm the XP bar turns blue.

After that, wire configurable rest rates if custom server rates are needed.

## Key Files

- `crates/wow-network/src/world/server/rest.rs`
- `crates/wow-network/src/world/server/player_login.rs`
- `crates/wow-network/src/world/server/session_loop.rs`
- `crates/wow-network/src/world/server/logout.rs`
- `crates/wow-network/src/world/handlers/misc.rs`
- `crates/wow-network/src/world/combat/lifecycle.rs`
- `crates/wow-network/src/world/session.rs`
- `crates/wow-network/src/world/map_runtime/state.rs`
- `crates/wow-network/src/world/packet_builders/progression.rs`
- `crates/wow-network/src/world/entities/player.rs`
- `crates/wow-network/src/world/packets.rs`
- `crates/wow-proto/src/world_packets.rs`
- `crates/wow-db/src/character/queries.rs`
- `crates/wow-db/src/character/state.rs`
- `crates/wow-db/src/character/types.rs`
- `src/game/Entities/Player.cpp`
