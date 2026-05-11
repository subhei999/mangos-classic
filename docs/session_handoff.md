# Session Handoff

Short operating brief for the next Rust migration session. Durable roadmap
history belongs in `docs/rust_migration_plan.md`, gate status in
`docs/playable_gate_board.md`, and branch split details in
`docs/playable_execution_roadmap.md`.

## Current Branch And Worktree

- Branch: `codex/multiplayer-cross-action-parity`, tracking
  `origin/codex/multiplayer-cross-action-parity`.
- Integration target remains `codex/rusty-mangos`.
- Playerbots are disabled by default for normal multiplayer/Northshire testing:
  `config/worldserver.local.toml` has `[playerbots] enabled = false` and
  `[playerbots.random] enabled = false`; the stack launcher keeps them off
  unless explicitly passed a playerbot flag.
- Current user-directed priority: finish Northshire multiplayer/gameplay parity
  issues found by real-client testing, using CMaNGOS as the behavior reference
  and keeping shared world authority in `MapRuntime`.
- Cast-from-sitting remains deprioritized after packet-trace investigation.
  The client eventually sent `CMSG_CAST_SPELL` after auto-standing, but then
  immediately sent `CMSG_CANCEL_CAST`; the root cause is likely a subtle
  client/server acknowledgement or state-ordering edge. Runtime packet tracing
  was removed again to keep normal playtest logs clean. Keep the useful
  CMaNGOS-shaped account-data support and client-originated stand-state
  acknowledgement cleanup, but do not block the Northshire demo on this bug.

## Recent Implemented Work

- Multiplayer cross-player parity, already committed and pushed, fixed nearby
  text-emote delivery, observer emote animations/states, looting visual flags,
  stop/turn orientation broadcasting, stand/sit/sleep/kneel observer state, and
  stale auto-attack cleanup after DB-creature death.
- Current inventory slice adds equipped secondary bag capacity, auto-equipping
  bags into available bag slots, dropping gear onto a bag icon to place it in
  the first valid contained slot, vendor/autostore stack merging, and tests for
  stack merge plus equipped bag storage.
- Current spell lifecycle slice adds CMaNGOS-style cast pushback from creature
  melee hits (`SMSG_SPELL_DELAYED`), auto-standing when casting from sit/sleep,
  and nearby observer cleanup packets when an active cast is interrupted by
  movement/cancel so lingering hand/fireball animations clear.
- Current real-client feedback slice fixes three follow-up issues from testing:
  cast-from-sitting now performs the stand transition before spell/item cast
  failure checks and now sends the self `SMSG_UPDATE_OBJECT` stand-state field
  update that the client needs to stop treating the caster as seated,
  client-driven standing now cancels stand-cancel regen auras before syncing
  player state,
  `SMSG_SPELL_DELAYED` now serializes a full 8-byte caster guid plus `uint32`
  delay like CMaNGOS `Spell::Delayed`, and login inventory create blocks now
  build equipped bag objects as `TYPEID_CONTAINER` with slot fields for visible
  contained items.
- Current account-data diagnostic slice adds persistent global/per-character
  account data loading, compressed `SMSG_UPDATE_ACCOUNT_DATA` responses, and
  compressed `CMSG_UPDATE_ACCOUNT_DATA` storage in `account_data` and
  `character_account_data`. `SMSG_ACCOUNT_DATA_TIMES` now sends MD5 digests for
  cached data instead of all-zero placeholders, matching the WoW 1.12.1 cache
  contract.
- Current stand-state cleanup aligns `CMSG_STANDSTATECHANGE` with CMaNGOS more
  closely: client-originated sit/stand changes update `MapRuntime` and nearby
  observers but no longer echo an immediate self `SMSG_UPDATE_OBJECT`; the
  server-forced auto-stand path used by spells/items still sends the acting
  client its explicit stand update.
- Current spell proc slice loads DB-backed proc trigger metadata from
  `spell_template` and applies triggered aura spells from successful creature
  melee hits against players. Frost Armor-style `SPELL_AURA_PROC_TRIGGER_SPELL`
  now applies its triggered effect to the attacker when proc flags/chance allow.
- Current aura stat slice parses DB-backed movement slow and melee attack-time
  aura modifiers (`SPELL_AURA_MOD_DECREASE_SPEED`,
  `SPELL_AURA_MOD_MELEE_HASTE`) and routes DB-creature apply/expiration through
  CMaNGOS-shaped runtime updates: map-owned move speeds are recomputed
  immediately, `SMSG_SPLINE_SET_*_SPEED` packets are sent for changed run,
  run-back, and swim speeds, active chase/return-home motion is retimed from
  the current position, and active creature swing timers are adjusted by the
  old/new base-attack-time delta. Chilled/Frost Armor should no longer be only
  a debuff icon or a next-path-only slowdown.
- Arcane Intellect-style `SPELL_AURA_MOD_STAT` now refreshes map-owned
  effective `PlayerWorldStats`, sends the acting client stat/max-power update
  fields on aura apply/expiration, and feeds intellect bonuses into
  `PlayerCombatStats` so spell crit math sees the buffed intellect.
- Current creature display slice loads `DisplayIdProbability*`,
  `creature_model_info.gender`, and `modelid_other_gender`, then serializes the
  selected display id/gender in DB-creature create blocks. Respawn reselects a
  native display and movement-script morph reset returns to that native display.
- Local addon cleanup removed
  `C:\World of Warcraft Classic\Interface\AddOns\NorthshireAuraTimers`. No repo
  commit is needed for that file-system-only cleanup.
- Current spell outcome slice ports the CMaNGOS low-level formulas that matter
  for Northshire spell damage: `Unit::CalculateSpellMissChance`,
  `Unit::CalculateEffectiveMagicResistancePercent`,
  `SPELL_PARTIAL_RESIST_DISTRIBUTION`, and
  `Player::GetSpellCritFromIntellect`. The Rust path currently applies this to
  non-weapon direct player spell damage against shared DB creatures; melee
  ability damage still uses the existing melee outcome path.
- Current proc slice now treats `proc_charges = 0` as unlimited and decrements
  finite `proc_charges` only after a proc successfully fires.
- Server-start hotfix: `CREATURE_SPAWN_SELECT`,
  `get_creature_template_query`, and `get_nearby_creature_spawns` now cast
  creature model gender, other-gender model id, and fallback radius/reach
  expressions. This fixes the real startup failure:
  `template_model_gender1` decoded as `DECIMAL` instead of `u8`.
- Current `cargo test -p wow-network --lib` passes with 564 tests. A previous
  full `.\scripts\test-rust.cmd` run reached successful crate tests/checks,
  then failed final binary rebuild because the running local `authserver.exe`
  held `target\debug\authserver.exe`; `.\scripts\restart-game-stack.cmd` then
  stopped/rebuilt/restarted the stack successfully.

## Tests Run

- `cargo fmt --package wow-db --package wow-network --check`
- `cargo check -p wow-network`
- `cargo fmt --package wow-network --check`
- `cargo check -p wow-db`
- `cargo test -p wow-network inventory --lib`
- `cargo test -p wow-network spell --lib`
- `cargo test -p wow-network combat --lib`
- `cargo test -p wow-network creature --lib`
- `cargo test -p wow-network proc_trigger --lib`
- `cargo test -p wow-network spell_damage_outcome --lib`
- `cargo test -p wow-network spell_delayed_packet_uses_full_caster_guid_for_client_cast_bar --lib`
- `cargo test -p wow-network map_owned_active_cast_damage_pushback_extends_remaining_cast_time --lib`
- `cargo test -p wow-network login_create_blocks_make_equipped_bags_openable_containers --lib`
- `cargo test -p wow-network spell_cast_from_sitting_auto_stands_player_and_observers --lib`
- `cargo test -p wow-network stand_state_change_to_stand_cancels_consumable_regen_aura --lib`
- `cargo test -p wow-network account_data --lib`
- `cargo test -p wow-network stand_state --lib`
- `cargo check -p wow-db`
- `cargo check -p wow-network`
- `cargo test -p wow-network spell --lib`
- `cargo test -p wow-network chilled --lib`
- `cargo test -p wow-network db_creature_swing_timer --lib`
- `cargo test -p wow-network db_creature_chase_motion_duration_applies_temporary_run_speed_slow --lib`
- `cargo test -p wow-network db_creature_slow_aura_retimes_active_chase_and_adjusts_swing_timer --lib`
- `cargo test -p wow-network db_creature_slow_aura_expiration_restores_speed_and_attack_timer --lib`
- `cargo test -p wow-network spell_aura_mod_stat_and_resistance_use_generic_template_metadata --lib`
- `cargo test -p wow-network aura --lib`
- `cargo test -p wow-network combat --lib`
- `cargo test -p wow-network inventory --lib`
- `cargo test -p wow-network --lib`
- `cargo test -p wow-db world_data --lib`
- `.\scripts\restart-game-stack.cmd`
- `git diff --check`
- `.\scripts\test-rust.cmd` reached successful crate tests/checks, then failed
  final binary rebuild because the running local `authserver.exe` held
  `target\debug\authserver.exe`; `.\scripts\restart-game-stack.cmd` then
  stopped/rebuilt/restarted the stack successfully.

## Current Follow-Ups

- Park cast-from-sitting as a non-blocking real-client follow-up. Known trace:
  after `/console autoStand 1`, the client can send `CMSG_CAST_SPELL` but then
  immediately sends `CMSG_CANCEL_CAST`; this needs a focused future parity pass
  against CMaNGOS stand acknowledgement/cast-start ordering. Runtime packet
  tracing is currently removed.
- Continue real-client smoke from the prior slices: buy duplicate bread stacks,
  drag equipped gear onto a bag icon, cancel/move during a cast while another
  player watches, and test Frost Armor or another proc-on-hit aura against a
  creature.
- Debuff timers on hostile creature target portraits remain unimplemented or
  client-limited until proven otherwise. CMaNGOS sends aura duration updates to
  player aura targets; do not fake hostile portrait timers with an addon.
- Spell outcome follow-ups: wire the outcome layer into creature-cast spells,
  player-vs-player spell damage, periodic damage, binary spell full-resist
  behavior, absorbs/vulnerabilities, spell hit/crit aura modifiers, and broader
  proc event metadata (`spell_proc_event`, procEx, cooldowns, PPM, equipment
  requirements). Do not branch more ad hoc spell damage paths before extending
  the shared outcome structs.
- Creature radius/reach still mostly use first-display template fallback. A
  later visual-size slice should derive radius/reach from the selected model row
  where CMaNGOS does.
- Loot autostore paths still have backpack-heavy legacy branches. Move them
  onto the shared inventory store planner when the next loot/inventory slice is
  opened.
- Continue Northshire missing criteria from the playable board: quest
  availability restrictions, quest item drops from real loot tables,
  gameobject quest pickup, remaining warrior level 1-6 spell parity, combat log
  feedback, health/rage regen behavior, skills/weapon skills,
  CMaNGOS-like aggro/chase/leash behavior, and patrol runtime stability.

## Key Files

- `crates/wow-db/src/world_data.rs`
- `crates/wow-network/src/world/inventory.rs`
- `crates/wow-network/src/world/vendors.rs`
- `crates/wow-network/src/world/spells.rs`
- `crates/wow-network/src/world/spells/spell_mgr.rs`
- `crates/wow-network/src/world/combat/aggro.rs`
- `crates/wow-network/src/world/combat/runtime.rs`
- `crates/wow-network/src/world/entities/creature.rs`
- `crates/wow-network/src/world/maps/map.rs`
- `crates/wow-network/src/world/maps/map_manager.rs`
- `crates/wow-network/src/world/maps/map/creature_motion.rs`
- `crates/wow-network/src/world/server/session_loop.rs`
- `crates/wow-network/src/world/server/movement.rs`
- `crates/wow-network/src/world/session.rs`
- `crates/wow-network/src/world/opcodes.rs`
- `crates/wow-network/src/world/packet_builders/combat.rs`
- `crates/wow-network/src/world/tests.rs`
- `docs/playable_gate_board.md`
- `docs/playable_execution_roadmap.md`
