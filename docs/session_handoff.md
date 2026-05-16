# Session Handoff

Short operating brief for the next Rust migration session. Durable roadmap
history belongs in `docs/rust_migration_plan.md`, gate status in
`docs/playable_gate_board.md`, and branch split details in
`docs/playable_execution_roadmap.md`.

## Current Branch And State

- Branch: `codex/rusty-mangos`, in the main checkout at
  `C:\Users\subhe\Documents\New project`.
- Current user-directed priority: Northshire Checkpoint 2 real-client parity.
  The user remains the grader through live playtesting; do not add a
  Northshire grading harness.
- Latest pushed commit is `0230c2fc9 Implement creature EventAI and wounded
  slowdown`.
- Current local work is uncommitted and expands the Northshire spell/CreatureAI
  slice: warrior Thunder Clap/Charge mechanics, CMaNGOS-shaped spell coverage,
  EventAI cast dispatch for Northshire combat/OOC/spawn cases, aura-only
  creature casts, generic learn-spell effect handling, and combat modifier
  auras used by nearby creature spells. Latest local additions cover the
  non-pet utility aura chunk: stealth/invisibility detection metadata,
  creature/resource tracking update fields, dummy utility aura metadata, DB
  creature ghost visual flags, and water-walk aura state. A GM command slice is
  also uncommitted: `.gm on` now sets CMaNGOS-like GM player flags/faction and
  blocks world damage/aggro, non-`.gm` dot commands require active GM mode,
  `.die` runs the normal creature death finalizer for kill credit/XP, `.go`
  supports same-map coordinates/common waypoints, and `.modify speed` sends a
  forced run-speed update.
- Playerbots remain disabled by default for normal multiplayer/Northshire
  testing: `config/worldserver.local.toml` has `[playerbots] enabled = false`
  and `[playerbots.random] enabled = false`.

## Current Goal And Recommended Next Task

- Goal: make the Northshire Checkpoint 2 playtest loop stable enough for the
  user to grade in the real client without disconnects, broken quest/object
  interactions, corpse/respawn stalls, or obvious CMaNGOS behavior gaps.
- Recommended next task: real-client smoke the Northshire combat/EventAI spell
  slice before adding broader spell systems. Kobold Miner should cast
  `Pierce Armor`, Defias Cutpurse should be able to cast `Backstab` when behind
  the player, Garrick should cast `Defensive Stance` on aggro, Kobold Geomancer
  should cast `Fireball/Frost Armor`, Mother Fang should cast `Web`, and nearby
  OOC/spawn self-cast scripts should no longer be skipped by the map runtime.
- If continuing spell parity, keep filling source/DBC-backed effect handlers
  rather than special-casing individual Northshire spells. Use
  `docs/northshire_spell_audit.md` plus the coverage helpers to group work by
  generic mechanic. Do not start pet/summon ownership until the user asks for
  that slice; duel, stuck, and Remove Insignia remain pending utility
  subsystems rather than safe one-off spell handlers.

## Recent Implemented Work

- Added `CreatureAiScriptQuery` and `get_creature_ai_scripts_for_entry` to
  `crates/wow-db/src/world_data.rs`.
- Added `ObjectMgr::creature_ai_scripts` caching and test cache stats in
  `crates/wow-network/src/world/globals/object_mgr.rs`.
- Added `CreatureMotionState::Flee` and `CreatureFleeMotion`, plus
  `start_db_creature_flee_motion_runtime`, run-speed retiming, and timed
  completion back to idle.
- Added `UNIT_FLAG_FLEEING` and included it in DB creature `UNIT_FIELD_FLAGS`
  while a creature is fleeing.
- Added map-owned EventAI HP evaluation for CMaNGOS:
  `EVENT_T_HP = 2`, `ACTION_T_FLEE_FOR_ASSIST = 25`, one-shot script tracking,
  `event_chance`, and CMaNGOS default flee delay of 10 seconds.
- Generalized HP EventAI dispatch so supported actions execute in script order.
  `ACTION_T_SET_WALK = 58` now toggles default movement walk/run and chase
  walk/run from DB params; active chase splines are retimed and resent with the
  correct run flag.
- Fixed a P0 real-client disconnect on attacking EventAI creatures by matching
  `creature_ai_scripts` DB column types: `id` is signed `INT` and
  `event_chance` is unsigned `INT`, not `u32`/`u8` respectively.
- Wired player melee, ranged auto attack, and player spell damage paths to
  evaluate loaded EventAI HP scripts after non-lethal damage.
- Active creature AI now pauses chasing, casting, and swinging while the
  creature is fleeing, then clears the flee flag and resumes normal combat once
  the flee timer ends.
- Corrected the weak-wolf parity assumption: CMaNGOS has generic wounded
  slowdown in `Creature::IsSlowedInCombat` and movement generators, independent
  of EventAI rows. Rust now loads `creature_template.StaticFlags2`, honors
  `NO_WOUNDED_SLOWDOWN`, applies the CMaNGOS linear under-30%-health speed
  multiplier to random and targeted chase movement, and retimes active movement
  when damage crosses the threshold so the real client sees the new spline.
- Added a generic player spell-effect pass for warrior abilities: trigger
  effects now execute triggered aura/damage spells, energize effects can grant
  rage, `TARGET_LOCATION_CASTER_SRC` plus radius is treated as caster-centered
  hostile AoE for damage/aura effects, and AoE direct damage iterates nearby
  hostile DB creatures.
- Added stun aura support for DB creatures: `SPELL_AURA_MOD_STUN` maps to an
  active aura modifier, sets `UNIT_FLAG_STUNNED`, stops/blocks creature motion,
  and pauses creature AI swings/casts/chase while stunned.
- Added a spell coverage registry matching the CMaNGOS Classic surface:
  `130` spell effect IDs and `192` aura IDs are now classified as implemented,
  known no-op, pending a subsystem, or unknown. Unsupported player spell-effect
  logs now include the CMaNGOS coverage name and support status.
- Added spell coverage audit helpers that report per-spell unsupported effect
  and aura mechanics. Current focused tests prove all CMaNGOS IDs are
  classified and the starter warrior spell fixture set has no coverage gaps.
- Added `docs/northshire_spell_audit.md`, a DB-backed inventory of
  Northshire-reachable human warrior, trainer, creature, EventAI, quest, item,
  chest, and triggered spells. The biggest actionable finding is that several
  Northshire spell failures are blocked by missing CMaNGOS EventAI cast/timer
  dispatch before individual effect handlers are even reached.
- Added the first map-owned EventAI spell-cast slice:
  timer-in-combat, aggro, range, facing-target, and missing-aura events can
  select `ACTION_T_CAST = 11` spells with CMaNGOS target modes used in
  Northshire (`self`, hostile current, default). EventAI casts now route through
  the existing creature spell start/go packet path.
- Added creature spell completion for aura-only casts, so creature-cast auras
  such as `Frost Armor`, `Defensive Stance`, `Web`, and `Pierce Armor` can
  apply instead of being skipped because there was no direct damage or heal
  effect.
- Extended the EventAI spell-cast slice with OOC timer and spawned events,
  broader CMaNGOS target modes backed by the map threat list, and OOC cast
  ticking from the combat lifecycle for nearby loaded creatures.
- Implemented generic `SPELL_EFFECT_LEARN_SPELL` for player targets: learned
  spells are persisted, inserted into the live session spell set, and followed
  by learned-spell/proficiency/initial-spells updates.
- Added Northshire-visible modifier aura buckets: resistance percent for armor
  reduction, physical damage-done, positive speed modifiers, and existing
  resistance/proc/melee-haste handling now cover the nearby creature spell rows.
- Fixed two real-client spell/EventAI regressions found during the first smoke:
  caster-centered hostile AoE spells such as Thunder Clap no longer normalize
  their cast targets to the player, and `EVENT_T_FACING_TARGET` now honors the
  CMaNGOS front/back parameter, 5-yard positional check, and no-repeat-without-
  timers load rule that prevents Backstab rows from firing every AI tick.
- Follow-up Backstab fix: generic DB creature spell casts now honor
  `SPELL_ATTR_SS_FACING_BACK` through shared target validation, so Backstab is
  rejected unless the creature is facing the target's back even when the spell
  comes from creature spell slots rather than EventAI.
- Added the non-pet Chunk E utility-aura slice: tracking auras now update
  `PLAYER_TRACK_CREATURES`/`PLAYER_TRACK_RESOURCES`; stealth and invisibility
  detection auras preserve their CMaNGOS modifier kind and amount; dummy utility
  auras are retained as typed active-aura metadata; ghost auras set DB creature
  unit visibility flags in create/aura/state update blocks; and water-walk
  auras are represented as active aura state for the movement subsystem.
- Improved GM commands for real-client playtesting: GM mode now applies the GM
  player flag and friendly faction template, map-owned player damage ignores GM
  players, sight aggro skips GM players, all non-`.gm` dot commands require GM
  mode to be active, `.die` now uses the same death finalization path as normal
  player kills so quest credit and XP can be awarded, `.go` supports `x y`
  coordinates plus common same-map waypoints, and `.modify speed #rate` sends
  the Classic forced run-speed-change packet.
- Left `SPELL_EFFECT_DUEL`, `SPELL_EFFECT_STUCK`, and
  `SPELL_EFFECT_SKIN_PLAYER_CORPSE` pending in the coverage registry. CMaNGOS
  routes them through duel state/gameobjects, graveyard/hearthstone/safe
  teleport, and player corpse/PvP loot ownership respectively, so they should
  not be implemented as spell-only shims.

## Tests Run

- `cargo test -p wow-network map_runtime_event_ai_hp --lib` passed with four
  focused EventAI HP tests covering flee and set-walk chase behavior after the
  DB type fix.
- `cargo test -p wow-network wounded_slowdown --lib` passed with two focused
  tests for the generic CMaNGOS wounded slowdown rule and opt-out flag.
- `cargo test -p wow-network db_creature_damage_crossing_wounded_threshold_retimes_active_chase --lib`
  passed.
- `cargo test -p wow-network thunder_clap --lib` passed with focused tests for
  Thunder Clap AoE metadata plus real damage/debuff application to nearby
  hostiles.
- `cargo test -p wow-network charge --lib` passed with focused tests covering
  Charge movement, no fake remote damage, rage grant, triggered Charge Stun,
  and blocked-navigation failure.
- `cargo test -p wow-network coverage --lib` passed with four focused tests for
  complete CMaNGOS effect/aura classification, pending-mechanic audit reporting,
  and starter warrior spell coverage.
- `cargo test -p wow-network event_ai --lib` passed with nine focused EventAI
  tests covering HP flee/set-walk plus combat timer, OOC timer, spawned, aggro,
  range, missing-aura, and threat-backed target selection.
- `cargo test -p wow-network event_ai_facing --lib` passed with the focused
  Backstab/facing-target repeat guard test.
- `cargo test -p wow-network backstab_validation --lib` passed with the shared
  creature spell `SPELL_ATTR_SS_FACING_BACK` validation regression.
- `cargo test -p wow-network db_creature_spell_cast --lib` passed after adding
  the shared behind-target validation to active creature casts.
- `cargo test -p wow-network caster_centered_hostile_aoe_spell_packets_do_not_self_target --lib`
  passed.
- `cargo test -p wow-network creature_aura_only --lib` passed.
- `cargo test -p wow-network db_creature_spell_cast --lib` passed.
- `cargo test -p wow-network thunder_clap --lib` passed.
- `cargo test -p wow-network charge --lib` passed.
- `cargo test -p wow-network wounded_slowdown --lib` passed.
- `cargo test -p wow-network spell_aura --lib` passed.
- `cargo check -p wow-network` passed after formatting.
- `cargo test -p wow-network utility_visibility --lib` passed.
- `cargo test -p wow-network tracking_auras --lib` passed.
- `cargo test -p wow-network ghost_and_water_walk --lib` passed.
- `cargo test -p wow-network coverage --lib` passed after the utility-aura
  classification update.
- `cargo check -p wow-network` passed after the utility-aura slice.
- `cargo test -p wow-network gm --lib` passed after the GM command slice.
- `cargo check -p wow-network` passed after the GM command slice.
- `.\scripts\test-rust.cmd` was rerun after the GM command slice and still
  fails in clippy on existing uncommitted spell/EventAI lint issues outside the
  GM command change: `prepare_db_creature_spell_cast_from_template` has too
  many arguments, two `spells/effects.rs` blocks need clippy reshaping, and four
  EventAI tests use clone-to-slice patterns.
- Restarted the local game stack successfully after the DB type fix:
  authserver on `127.0.0.1:13724`, worldserver on `127.0.0.1:18085`,
  dashboard on `127.0.0.1:9091`.
- `cargo test -p wow-network --lib` ran 696 tests; 691 passed and five
  existing DB-backed spell tests failed with local DB pool timeouts while
  waiting for an open connection. The failing names were
  `eviscerate_uses_combo_points_for_damage_and_clears_them_on_hit`,
  `fireball_with_periodic_aura_applies_direct_damage_and_dot`,
  `player_damage_spell_executes_each_damage_effect_slot`,
  `sinister_strike_cast_uses_energy_and_spell_damage_log_result`, and
  `cast_time_spell_sends_start_before_delayed_go_and_effects`.
- `.\scripts\test-rust.cmd` was not rerun for the set-walk follow-up. Last
  known full-script attempt progressed through most workspace tests but failed
  in existing DB-backed spell tests because local MySQL root auth is denied:
  `1698 (28000): Access denied for user 'root'@'localhost'`.

## Real-Client Verification Needed

- Weak-creature movement:
  Young Wolf and other normal starter mobs should visibly slow below 30% health
  and not flee unless EventAI says to flee; mobs with HP set-walk rows should
  still switch to scripted walk/run behavior.
- Flee combat behavior:
  while fleeing, the creature should not swing/cast/chase; after the CMaNGOS
  flee delay, it should resume normal combat if still alive/in combat.
- Warrior spells:
  Thunder Clap should damage every nearby hostile creature in radius and apply
  the attack-speed debuff; Charge should visibly stun the target after movement
  and grant its rage.
- Spell coverage:
  when adding new class, creature, item, or quest spells, run the coverage audit
  against the reachable spell IDs first and implement pending mechanics
  generically.
- Northshire spell audit:
  EventAI `ACTION_T_CAST = 11` is implemented for timer-in-combat, timer-OOC,
  aggro, range, facing-target, missing-aura, spawned, and common threat-backed
  target modes. Verify the real DB rows actually fire in client.
- Existing Checkpoint 2 regression smoke still matters:
  quest GO gating after abandon, Battered Chest loot including multi-item loot,
  Milly bucket cancel/interruption, enemies attacking during GO interaction,
  logout movement lock/cancel, Hearthstone cooldown icon after relog, visible
  buff icons after relog, corpse/respawn, respawn aggro grace, trainer/generic
  gossip, and Heroic Strike/rage display.

## Current Follow-Ups

- This is still not a full CMaNGOS EventAI port. The Northshire cast surface is
  covered, but zone-conditioned spawned events, evade/reached-home/kill/death
  events, richer cast flags, summons/pet ownership, and broader non-cast
  actions remain future slices.
- Utility effect handlers still pending before full parity: duel needs the duel
  flag gameobject plus request/accept/cancel state; stuck needs graveyard,
  Hearthstone, and safe-teleport ownership; Remove Insignia needs player corpse
  and PvP loot conversion. Keep these out of the generic spell dispatcher until
  their owner systems exist.
- GM `.go` currently uses near-teleport support and is intentionally limited to
  same-map destinations. Cross-map named teleports should be wired through a
  proper CMaNGOS-style map transfer path before adding Kalimdor/outland-style
  waypoints.
- Flee movement currently uses the CMaNGOS 30-yard run-away shape and the
  project pathing guardrail. If real-client behavior exposes pathing oddities,
  compare against CMaNGOS `FleeingMovementGenerator` / `PanicMovementGenerator`
  before tuning.
- If the full Rust test script is required locally, fix the MySQL root auth
  environment or rerun with the expected DB credentials/container.

## Key Files

- `crates/wow-db/src/world_data.rs`
- `crates/wow-network/src/world/globals/object_mgr.rs`
- `crates/wow-network/src/world/motion/motion_master.rs`
- `crates/wow-network/src/world/combat/{aggro.rs,lifecycle.rs,motion.rs,runtime.rs}`
- `crates/wow-network/src/world/entities/creature.rs`
- `crates/wow-network/src/world/map_runtime/map/{creature_combat.rs,creature_damage.rs,creature_motion.rs}`
- `crates/wow-network/src/world/map_runtime/{map.rs,map_manager.rs}`
- `crates/wow-network/src/world/spells/effects.rs`
- `crates/wow-network/src/world/opcodes.rs`
- `crates/wow-network/src/world/tests.rs`
- `docs/spell_effect_coverage.md`
- `docs/northshire_spell_audit.md`
