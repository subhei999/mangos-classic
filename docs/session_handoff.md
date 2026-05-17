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
- Current local work is uncommitted and covers the spell/EventAI slices for
  Mage and Northshire parity: destination hostile AoE, dynamic-object channels,
  unit-target channels, absorb/dispel/control aura metadata, Blink/Leap,
  Polymorph-style transform/confuse behavior, EventAI casting, wounded
  slowdown, and supporting tests.
- The user remains the Northshire Checkpoint 2 grader through real-client
  playtesting. Do not add or maintain a Northshire grading harness.
- Playerbots are disabled for normal testing in
  `config/worldserver.local.toml`.

## Current Goal

- Keep building CMaNGOS-shaped spell behavior generically rather than
  special-casing individual Mage spell IDs.
- Recommended next implementation follow-up: do a deeper Polymorph parity dive
  against CMaNGOS control-aura, confused movement, damage-break, regen, and
  threat/hostility behavior after the current client smoke.
- Immediate real-client focus: verify Mana Shield melee absorb, Fire/Frost Ward
  school absorbs, Remove Curse dispel, Polymorph sheep/confuse/damage break,
  Blink forward teleport, Arcane Missiles channel visuals, Blizzard cancel and
  aggro, and Flamestrike hostile-only destination AoE.

## What Changed Recently

- `spell_template` now loads `AuraInterruptFlags`, `ChannelInterruptFlags`, and
  `EffectMultipleValue` needed by control, channel, and absorb behavior.
- Active aura metadata now tracks school absorbs, Mana Shield, dispel type,
  confuse/fear/pacify/silence, feather fall, and transform display source.
- Mana Shield and school absorbs are applied in the map-owned player damage
  path before health loss, with mana/aura updates when shields are spent.
- `SPELL_EFFECT_DISPEL` removes matching player or creature auras by dispel
  type and emits the Classic dispel log packet.
- Blink/Leap is implemented from the real DBC shape: `Effect=LEAP`, implicit
  target `TARGET_LOCATION_CASTER_FRONT_LEAP`, and `SpellRadius.dbc` radius.
  This fixes Blink when the client does not send a destination.
- Polymorph transform now resolves `EffectMiscValue` creature entry `16372`
  through `creature_template` display data instead of trying to invent a sheep
  display from base points.
- Polymorph break-on-damage now uses `AuraInterruptFlags &
  AURA_INTERRUPT_FLAG_DAMAGE`, matching the CMaNGOS owner field.
- Confuse/fear auras suppress hostile reference setup, remove active creature
  combat when applied, and start confused random motion while the aura is
  active.
- Channeled spell follow-ups remain in the same local work: Arcane Missiles
  ticks from map-owned channels, damage waits for missile impact, final rank 1
  missile launches before channel clear, damage can push back/cancel channels,
  and lethal deferred damage stops target motion before corpse state.

## Tests Run

- `cargo test -p wow-network blink_missing_client_destination_uses_front_leap_radius --lib`
  passed.
- `cargo test -p wow-network polymorph_transform_updates_creature_display_and_breaks_on_damage --lib`
  passed.
- `cargo test -p wow-network control_absorb_and_dispel_aura_metadata_comes_from_spell_template --lib`
  passed.
- `cargo test -p wow-network transform_aura_resolves_creature_entry_to_display_id --lib`
  passed.
- `cargo test -p wow-network --lib` passed: 737 tests.
- `.\scripts\test-rust.cmd` passed after stopping the running local
  `authserver.exe` and `worldserver.exe` that initially locked Cargo rebuilds.

## Real-Client Verification Needed

- Blink should teleport forward roughly 20 yards from the caster using terrain
  ground placement; no target should be required.
- Polymorph should show sheep display, make the target confused instead of
  attacking, wander while controlled, and break on incoming damage.
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

- Full CMaNGOS dynamic-object aura semantics are not complete; current ground
  AoE covers create/destroy, channel updates, expiry, and direct periodic
  damage.
- Map-owned periodic player spell kills still need DB-backed corpse loot prep
  before relying on Blizzard/Flamestrike/Arcane Missiles as common loot-bearing
  killing blows.
- Utility effects still pending: duel ownership, stuck/graveyard/hearth flow,
  and remove-insignia/player-corpse logic.
- If Polymorph wander looks wrong in client, compare against CMaNGOS
  `ConfusedMovementGenerator` before tuning constants.
- The full script may fail to rebuild while local auth/world binaries are
  running because Windows locks `target\debug\*.exe`; stop the stack before
  verification builds.

## Key Files

- `crates/wow-db/src/world_data.rs`
- `crates/wow-network/src/world/globals/object_mgr.rs`
- `crates/wow-network/src/world/session.rs`
- `crates/wow-network/src/world/spells.rs`
- `crates/wow-network/src/world/spells/effects.rs`
- `crates/wow-network/src/world/spells/targets.rs`
- `crates/wow-network/src/world/map_runtime/map/dynamic_objects.rs`
- `crates/wow-network/src/world/map_runtime/map/player_channels.rs`
- `crates/wow-network/src/world/map_runtime/map/creature_damage.rs`
- `crates/wow-network/src/world/map_runtime/map/creature_motion.rs`
- `crates/wow-network/src/world/tests.rs`
- `docs/northshire_spell_audit.md`
