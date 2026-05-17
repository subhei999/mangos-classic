# Session Handoff

Short operating brief for the next Rust migration session. Durable roadmap
history belongs in `docs/rust_migration_plan.md`, gate status in
`docs/playable_gate_board.md`, and branch split details in
`docs/playable_execution_roadmap.md`.

## Current Branch And State

- Branch: `codex/rusty-mangos`, main checkout:
  `C:\Users\subhe\Documents\New project`.
- Latest pushed commit: `0230c2fc9 Implement creature EventAI and wounded
  slowdown`.
- Current local work is uncommitted and extends the Mage/parity slice with a
  more CMaNGOS-shaped `Polymorph` foundation: spell-facing lookup from
  `spell_facing`, caster-owned single-target creature aura tracking, map-owned
  polymorph diminishing state, polymorph helper regen plumbing, centralized
  creature damage-break handling, and now a dedicated confused-motion owner and
  scheduler instead of routing sheep wander through the ambient idle-motion
  queue.
- The user remains the Northshire Checkpoint 2 grader through real-client
  playtesting. Do not add or maintain a Northshire grading harness.
- Playerbots are disabled for normal testing in
  `config/worldserver.local.toml`.

## Current Goal

- Keep building CMaNGOS-shaped spell behavior generically rather than
  special-casing individual Mage spell IDs.
- Immediate recommended follow-up: real-client verify the `Polymorph`
  combat-state regression reported after the latest sheep/combat-owner change,
  then continue helper-heal proof and diminishing-return reset/timing,
  especially evade/reset behavior.
- Immediate real-client focus: verify `Polymorph` sheep/heal/confuse/damage
  break on direct damage, DoTs, and Blizzard ticks; confirm no-facing casts;
  then continue Mana Shield, Fire/Frost Ward, Remove Curse, Blink, Arcane
  Missiles, and Flamestrike smoke.

## What Changed Recently

- Player hostile spell-facing checks now use cached `spell_facing` DB data
  through `ObjectMgr`, matching the CMaNGOS ownership boundary instead of
  forcing all hostile spells to require front arc. `Polymorph` can now cast
  while the caster is not facing the target, while `Fireball`-style flagged
  spells still fail with `SPELL_FAILED_UNIT_NOT_INFRONT`.
- Creature aura application now carries optional caster-owned single-target
  descriptors and diminishing groups. `Polymorph` can replace the same caster's
  prior sheep target instead of allowing multiple concurrent sheeped creatures.
- MapRuntime now tracks active single-target creature auras and per-target
  diminishing state for polymorph-like CC. Tracker cleanup runs on normal aura
  removal and on break-on-damage removal.
- Periodic creature damage and dynamic-object periodic damage now break
  damage-interrupt creature auras too, so `Polymorph` drops from DoT ticks and
  Blizzard-style periodic AoE paths instead of only direct damage.
- Periodic regen metadata now distinguishes consumable-style sit/move/damage
  interruption from unsuppressed helper regen, which allows polymorph helper
  healing without inheriting food/drink semantics.
- Confused creature movement now has its own map-owned start schedule and
  runtime state. Sheep wander no longer depends on the ambient idle/random
  start queue, keeps a fixed confused origin like the CMaNGOS generator, and
  rearms its own pause timer between short walk splines.
- `Polymorph` now clears creature combat through the shared map combat owner
  instead of raw-removing only `active_creature_combats`. That keeps player
  `in_combat`, leash, threat, and combat re-entry bookkeeping in sync after
  sheep drops combat.
- A focused unit test now proves the map-owned combat state can clear on sheep
  and later restart for the same creature, but the user still reports a
  critical real-client bug where sheep can leave the player out of combat even
  after the mob resumes attacking. Treat the local test as insufficient proof
  until the live repro is resolved.

## Tests Run

- `cargo test -p wow-network polymorph --lib` passed.
- `cargo test -p wow-network hostile_spell_cast_failure_checks_range_los_and_facing_from_map --lib`
  passed.
- `cargo test -p wow-network cast_time_spell_sends_start_before_delayed_go_and_effects --lib`
  passed.
- `cargo test -p wow-network cast_time_spell_rechecks_facing_before_completion_go --lib`
  passed.
- `cargo test -p wow-network map_runtime_environmental_damage_interrupts_regen --lib`
  passed.
- `cargo test -p wow-network stand_state_change_to_stand_cancels_consumable_regen_aura --lib`
  passed.
- `cargo test -p wow-network db_creature_confused_motion_uses_dedicated_state_and_pause_timer --lib`
  passed.
- `cargo test -p wow-network map_runtime_confused_motion_start_guids_use_control_scheduler --lib`
  passed.
- `cargo test -p wow-network map_runtime_tick_starts_confused_motion_from_control_scheduler --lib`
  passed.
- `cargo test -p wow-network polymorph_clears_creature_combat_through_map_owner_and_allows_reentry --lib`
  passed.
- `cargo test -p wow-network creature_combat_ownership_marks_player_in_combat --lib`
  passed.
- `cargo test -p wow-network begin_shared_db_creature_combat_uses_mapruntime_liveness_without_session_cache --lib`
  passed.
- `cargo test -p wow-network --lib` passed: 743 tests.
- `.\scripts\test-rust.cmd` passed fmt/clippy/check/unit/integration coverage
  again, then failed only at the final `cargo build -p authserver` step because
  Windows could not overwrite a running `target\debug\authserver.exe`
  (`Access is denied`, os error 5).

## Real-Client Verification Needed

- `Polymorph` should show sheep display, restore health while active, allow
  casts without facing, wander convincingly while controlled, break from
  melee, spell direct damage, DoTs, and Blizzard ticks, and cleanly drop then
  re-enter player combat when sheeped creatures resume hostility.
- Current real-client blocker: after `Polymorph`, the player may remain out of
  combat while the mob attacks, allowing food/drink usage. The recent
  map-owned combat-clear patch did not yet earn user confidence and should be
  treated as still open until replayed in client with packet/state inspection.
- Confirm polymorph diminishing returns timing against repeated re-sheep
  sequences, including whether evade/home reset should clear the DR chain.
- Blink should teleport forward roughly 20 yards from the caster using terrain
  ground placement; no target should be required.
- Mana Shield should absorb melee damage and consume mana; Fire/Frost Ward
  should absorb only matching school damage.
- Remove Curse/Detect Magic/Dampen Magic need live-client checks for correct
  aura visibility, dispel result, and failure feedback.
- Arcane Missiles should keep the caster in channel pose through all three rank
  1 missile launches, aggro only when impact damage lands, and stop dead target
  motion.
- Blizzard should cancel on movement/damage interruption, aggro on periodic
  damage, and only affect hostile targets in the selected ground area.
- Flamestrike should cast at a destination without unit target and hurt only
  attackable hostile creatures.

## Known Follow-Ups

- `Polymorph` helper regen is wired, but there is not yet a dedicated
  regression that proves the helper aura heals exactly like CMaNGOS; current
  confidence is from code-path inspection and integration plumbing.
- Diminishing returns still need a real-client parity check around evade/home
  reset. Current DR state is target-owned and time-based, but evade does not
  yet explicitly clear the polymorph DR chain the way CMaNGOS aura cleanup may.
- The sheep combat-state regression may still involve a second session-facing
  or retaliation/chase edge beyond the map-owned clear path. If the bug
  reproduces after the latest patch, inspect live combat-flag packets,
  retaliation re-entry, and session cache refresh around sheep break/resume
  instead of assuming the map-side owner fix was sufficient.
- Full dynamic-object aura semantics are still incomplete beyond current
  create/destroy/channel/periodic-damage support.
- Map-owned periodic player spell kills still need DB-backed corpse loot prep
  before relying on Blizzard/Flamestrike/Arcane Missiles as common loot-bearing
  killing blows.
- Utility effects still pending: duel ownership, stuck/graveyard/hearth flow,
  and remove-insignia/player-corpse logic.
- The full script may fail to rebuild while local auth/world binaries are
  running because Windows locks `target\debug\*.exe`; stop the stack before
  verification builds.

## Key Files

- `crates/wow-db/src/world_data.rs`
- `crates/wow-network/src/world/globals/object_mgr.rs`
- `crates/wow-network/src/world/session.rs`
- `crates/wow-network/src/world/spells.rs`
- `crates/wow-network/src/world/spells/effects.rs`
- `crates/wow-network/src/world/map_runtime/map.rs`
- `crates/wow-network/src/world/map_runtime/map_manager.rs`
- `crates/wow-network/src/world/map_runtime/map/creature_damage.rs`
- `crates/wow-network/src/world/map_runtime/map/dynamic_objects.rs`
- `crates/wow-network/src/world/map_runtime/map/players.rs`
- `crates/wow-network/src/world/tests.rs`
- `docs/northshire_spell_audit.md`
