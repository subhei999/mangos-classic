# Session Handoff

Short operating brief for the next Rust migration session. Durable roadmap
history belongs in `docs/rust_migration_plan.md`, gate status in
`docs/playable_gate_board.md`, and focused feature plans in their own docs.

## Current Branch And State

- Branch: `codex/rusty-mangos`
- Workspace: `C:\Users\subhe\Documents\New project`
- Latest pushed checkpoint before the current uncommitted work:
  `58851c5fd Fix tavern area trigger lookup decode`
- Current uncommitted state continues the generic spell system with mage-facing
  fixes for Cone of Cold, absorb combat-log packets, regular melee absorb
  presentation, Evocation-style mana regeneration modifiers, and the
  `SpellPlan` DBC flag audit. No spellscript or aurascript engine has been
  added.

## Current Goal

Latest user-directed priority: finish the generic DBC-driven spell system as far
as it can go without spellscript/aurascript, using CMaNGOS behavior as the
reference. Scripts should remain a later layer for dummy/script effects, special
class/talent cases, and advanced spell/aura custom behavior.

## What Changed Recently

- Cone target ids `24` and `54` are now classified as caster-centered hostile
  cone targets instead of generic self/caster aura behavior.
- `SpellCone.sql` is loaded from CMaNGOS SQL data and copied into
  `MapRuntimeManager`. Cone angle lookup uses the exact spell id when present,
  otherwise resolves through `spell_chain.first_spell`, matching CMaNGOS'
  first-rank `spell_cone` lookup. Missing rows fall back to the default 60
  degree cone.
- Player direct damage and hostile aura application now filter caster-centered
  cone effects by facing arc as well as radius/reaction, so Cone of Cold damages
  and slows only attackable creatures in front of the caster.
- Runtime school/mana-shield absorbs now flow into combat packet presentation:
  - `SMSG_SPELLNONMELEEDAMAGELOG` sends post-absorb damage plus the absorb
    amount.
  - melee attacker-state packets set `HITINFO_ABSORB` and include absorbed
    amount, so the client can show absorb feedback instead of plain hit text.
  - the map-manager victim/direct regular-hit path now forwards the adjusted
    map-runtime attacker-state body instead of rebuilding a pre-absorb packet
    for the active player.
  - the direct regular-hit absorb regression test now uses a deterministic
    manager outcome path that shares the same tick packet materialization,
    avoiding random melee miss/dodge noise in broad runs.
- Generic aura stat modifiers now include:
  - `SPELL_AURA_MOD_POWER_REGEN_PERCENT` (`110`),
  - `SPELL_AURA_MOD_MANA_REGEN_INTERRUPT` (`134`).
  Player mana regen applies these to spirit regen, allowing Evocation to restore
  mana during the normal recent-cast regen interruption window.
- Self-channeled caster auras now use the generic player channel lifecycle:
  DBC `SPELL_ATTR_EX_IS_SELF_CHANNELED`, duration, and
  `ChannelInterruptFlags` start `MSG_CHANNEL_START`, set `UNIT_CHANNEL_SPELL`,
  clear naturally at expiry, and cancel on movement. Channeled aura interrupt
  flags include the DBC channel flags so Evocation's mana aura is also removed
  when movement breaks the channel.
- `SpellPlan` now compiles raw DBC/template fields into typed runtime intent:
  cast profile, target plan, per-effect targets, channel kind, cast behavior,
  hostile/retaliation targeting, passive-learn ownership, and DB-creature
  autocast/effect shape. Player profile selection, target normalization and
  validation, failed hostile-cast retaliation, active aura channel interrupt
  ownership, self-aura channel start, periodic trigger channels, persistent
  area channels, leap fallback targeting, player aura application routing,
  mana/auto-attack cast behavior, and DB-creature spell preparation now consult
  the plan instead of each rechecking raw DBC flags.
- `SpellPlan` now also audits DBC attribute flags in one place. Known bits are
  classified as generic runtime behavior, execution-payload behavior, pending
  generic work, script-required behavior, known no-ops, or unknown bits. This
  lets new DBC flag support land as a plan-classification change plus a focused
  plan shape test before any runtime wiring. Payload-only flags such as
  spell-cant-crit and always-hit do not block runtime support, while Arcane
  Missiles now visibly surfaces currently unmapped `Attributes` bits for the
  next parity pass.
- Arcane Missiles now demonstrates the intended SpellPlan debugging flow:
  `SpellPlan` correctly classifies it as a hostile unit channel, and player
  spell target validation now enforces faction attackability for DB-creature
  targets. Friendly NPC targets return bad-targets before channel start or mana
  spend.
- Older hostile spell tests that used the shared default test creature now mark
  their local spell target spawns as hostile. The default test creature remains
  faction `35` for gossip/query compatibility; spell tests should opt into
  hostile faction explicitly when exercising enemy-target casts.
- Counterspell/effect `68` is now generic instead of unsupported:
  `SpellPlan` classifies `SPELL_EFFECT_INTERRUPT_CAST` as an interrupt against
  a hostile unit, player casts use normal hostile target outcome/validation, and
  the map runtime removes an active DB-creature spell cast with interrupted
  failure packets. Interrupted DB-creature casts now store a CMaNGOS-style
  school lockout through `GetSchoolMask(School)` semantics and future
  DB-creature spell preparation rejects same-school casts until Counterspell's
  DBC duration expires.
- Spell coverage marks aura ids `110` and `134` implemented as generic
  stat/combat modifiers.

## Generic Spell Boundary

Expected to work without spellscript/aurascript:

- DBC-driven target selection: direct caster/unit targets, destination AoE,
  caster-source hostile AoE, cone targets, range/LOS/facing gates.
- Generic effect dispatch for school damage, weapon damage, heals, energize,
  item creation, teleport/leap movement, dynamic-object/persistent-area damage,
  aura apply/remove, and periodic damage/heal/energize.
- Generic aura modifiers that can be expressed directly from DBC fields:
  movement speed, roots, stuns/control, absorbs, mana shield, stat/resistance
  modifiers, regen modifiers, interrupt/pacify/silence, shapeshift/display
  transforms already represented in `AuraStatModifier`.

Should wait for spellscript/aurascript:

- Dummy/script effects and triggered spell chains with spell-family special
  cases.
- Talent-specific proc math and class exceptions not expressible from DBC rows.
- Boss/event scripts and one-off behaviors that CMaNGOS owns in script hooks.
- Advanced aura scripts where the generic aura id alone is insufficient.

## Tests Run

- `cargo fmt -p wow-network`
- `cargo check -p wow-network`
- Focused Rust tests:
  - `cargo test -p wow-network cone_of_cold_uses_caster_cone_targeting_not_caster_self_aura -- --nocapture`
  - `cargo test -p wow-network cone_of_cold_damages_and_debuffs_only_hostiles_in_front_cone -- --nocapture`
  - `cargo test -p wow-network spell_cone_metadata_uses_chain_root_for_higher_ranks -- --nocapture`
  - `cargo test -p wow-network evocation_builds_generic_mana_regen_aura_modifiers -- --nocapture`
  - `cargo test -p wow-network evocation_starts_self_channel_and_movement_cancels_it -- --nocapture`
  - `cargo test -p wow-network map_runtime_evocation_modifiers_regen_mana_during_interrupt -- --nocapture`
  - `cargo test -p wow-network spell_plan -- --nocapture`
  - `cargo test -p wow-network spell_plan_owns_cast_behavior_and_creature_spell_shape -- --nocapture`
  - `cargo test -p wow-network spell_plan_audits_dbc_attribute_flags_in_one_place -- --nocapture`
  - `cargo test -p wow-network arcane_missiles_rejects_friendly_creature_target_before_channel_start -- --nocapture`
  - `cargo test -p wow-network counterspell -- --nocapture`
  - `cargo test -p wow-network arcane_missiles_starts_unit_channel_and_ticks_triggered_damage -- --nocapture`
  - `cargo test -p wow-network blizzard_creates_channel_dynamic_object_and_ticks_area_damage -- --nocapture`
  - `cargo test -p wow-network caster_centered_hostile_root_spell_uses_aoe_target_and_radius_metadata -- --nocapture`
  - `cargo test -p wow-network thunder_clap_uses_caster_source_aoe_damage_and_attack_speed_debuff -- --nocapture`
  - `cargo test -p wow-network flamestrike_uses_destination_hostile_aoe_targeting -- --nocapture`
  - `cargo test -p wow-network direct_friendly_unit_aura_targets_require_a_friendly_unit -- --nocapture`
  - `cargo test -p wow-network blink_missing_client_destination_uses_front_leap_radius -- --nocapture`
  - `cargo test -p wow-network movement_interrupts_channel_with_moving_interrupt_flag -- --nocapture`
  - `cargo test -p wow-network map_runtime_db_creature_spell_damage_log_reports_runtime_absorb -- --nocapture`
  - `cargo test -p wow-network map_runtime_db_creature_melee_damage_packet_reports_runtime_absorb -- --nocapture`
  - `cargo test -p wow-network map_runtime_manager_direct_melee_packet_reports_runtime_absorb -- --nocapture`
  - `cargo test -p wow-network player_spell_target -- --nocapture`
  - `cargo test -p wow-network spell_effect_coverage_classifies_every_cmangos_effect_id -- --nocapture`
- Latest focused follow-up after adding Counterspell school lockout:
  - `cargo fmt -p wow-network`
  - `cargo check -p wow-network`
  - `cargo test -p wow-network counterspell -- --nocapture`
  - `cargo test -p wow-network map_runtime_db_creature_spell -- --nocapture`
  - `cargo test -p wow-network spell_plan -- --nocapture`
  - `git diff --check`
- `.\scripts\test-rust.cmd`
  - passes after temporarily removing the two DB-backed EventAI immolate tests
    that depended on passwordless local MySQL `root` access.

## Known Blockers / Unproven Areas

- Two DB-backed EventAI immolate tests were removed temporarily because they
  depended on passwordless local MySQL `root` access. Re-add equivalent coverage
  with primed/in-memory data or a proper test DB fixture before relying on that
  path as fully covered.
- Live-client mage smoke is still needed for:
  - Cone of Cold all ranks and creature/player facing,
  - Mana Shield and Ice Barrier combat text showing absorbs for both special
    abilities and regular white hits,
  - Evocation visual channel start, movement cancellation, and mana tick feel.
- Cone filtering currently covers DB creatures in the map runtime path. If
  player-vs-player spell damage/aura targeting is added later, the same cone
  target contract should be reused there.
- Script/dummy effects are still intentionally out of scope until the scripting
  engines are introduced.
- Raw DBC spell interpretation should now stay inside `SpellPlan`/`SpellInfo`
  construction or low-level combat payloads such as crit/resist/always-hit
  fields. Keep new routing decisions out of effect/session/map-runtime callers.
- The new unsupported flag audit is diagnostic and does not yet implement every
  remaining DBC bit. Classify and implement unmapped bits from CMaNGOS one at a
  time, preserving script-required cases for the later spellscript/aurascript
  layers.
- Counterspell school lockout is currently proven for interrupted DB-creature
  casts and DB-creature recast preparation. Player-vs-player/player-target
  school lockout and client cooldown packet presentation are not implemented or
  proven yet.
- Proc ownership, triggered spell chains, and future script/aura script dispatch
  boundaries still need fuller plan representation as those systems are
  expanded.

## Recommended Next Task

Recommended next task: use the `SpellPlan` flag audit to continue the generic
spell-system sweep with CMaNGOS-backed DBC flag classifications:

- classify Arcane Missiles' currently unmapped `Attributes` bits and either map
  them to generic behavior, execution payloads, known no-ops, or script-required
  ownership,
- keep each new DBC flag implementation plan-first with a focused shape test,
- live-smoke the three fixed mage repros,
- add/verify remaining DBC-generic mage spells such as Counterspell
  player-target school lockout/client cooldown presentation, Polymorph
  control/regeneration, Blink movement edge cases, and Blizzard/Frost Nova rank
  behavior,
- keep logging any spell that needs script ownership rather than forcing
  hardcoded behavior into the generic path.

## Key Files

- `crates/wow-network/src/world/spells/effects.rs`
- `crates/wow-network/src/world/spells/effects/damage.rs`
- `crates/wow-network/src/world/spells/effects/auras.rs`
- `crates/wow-network/src/world/spells/effects/coverage.rs`
- `crates/wow-network/src/world/spells/plan.rs`
- `crates/wow-network/src/world/spells/spell_mgr.rs`
- `crates/wow-network/src/world/spells/definitions.rs`
- `crates/wow-network/src/world/map_runtime/world_data.rs`
- `crates/wow-network/src/world/map_runtime/map_manager/mod.rs`
- `crates/wow-network/src/world/map_runtime/map_manager/spells.rs`
- `crates/wow-network/src/world/map_runtime/systems/spatial.rs`
- `crates/wow-network/src/world/map_runtime/systems/creature_combat.rs`
- `crates/wow-network/src/world/map_runtime/systems/damage.rs`
- `crates/wow-network/src/world/map_runtime/systems/players.rs`
- `crates/wow-network/src/world/session.rs`
- `crates/wow-network/src/world/tests/spells.rs`
- `crates/wow-network/src/world/tests/player_runtime_auras.rs`
- `crates/wow-network/src/world/tests/map_runtime_creatures.rs`
- `sql/base/dbc/original_data/SpellCone.sql`
- `src/game/Spells/Spell.cpp`
- `src/game/Entities/Unit.cpp`
- `src/game/Entities/Player.cpp`
