# Session Handoff

Short operating brief for the next Rust migration session. Durable roadmap
history belongs in `docs/rust_migration_plan.md`, gate status in
`docs/playable_gate_board.md`, and branch split details in
`docs/playable_execution_roadmap.md`.

## Current Branch And State

- Branch: `codex/rusty-mangos`, in the main checkout at
  `C:\Users\subhe\Documents\New project`.
- Current user-directed priority: continue NPC spell-system parity after the
  death lifecycle work. Real-client death proof looked good in the latest user
  pass; caster spell visuals/combat-log fidelity remain the active follow-up.
- Playerbots are disabled by default for normal multiplayer/Northshire testing:
  `config/worldserver.local.toml` has `[playerbots] enabled = false` and
  `[playerbots.random] enabled = false`.

## Recent Implemented Work

- Player death lifecycle is now map-owned with `PlayerDeathState::JustDied` as
  a runtime-only transition. Lethal melee, direct spell, DoT, environmental,
  fall, and GM-style damage paths flow through the same cleanup/presentation
  machinery.
- Death cleanup now clears combat flags, auto attack, queued melee spells,
  active player casts, pending player spell events, active creature casts
  targeting the dead player, looting/combo state, death-cleared auras, and
  creature threat/combat. Release Spirit and resurrection refresh from
  `MapRuntime` and restore movement defensively.
- Fixed the DoT death pop-up/relog class of bugs by preserving zero health in
  world-stat refresh packets, clearing stale fall tracking on normal grounded
  movement, and forcing pending `JustDied` presentation before repop.
- NPC periodic damage auras keep ticking from stored caster combat snapshots if
  the live caster runtime disappears while the debuff remains active.
- DB creature spell AI now validates selected targets through `MapRuntime` at
  cast start and completion. Validation covers caster/target liveness, range,
  LOS, and path guardrails.
- Creature spell-list cooldowns now combine initial delay, repeat cooldown,
  spell recovery, and category recovery. Category cooldown sharing honors
  `SPELL_LIST_FLAG_CATEGORY_COOLDOWN`.
- Creature casts interrupted by target death, range, or LOS now broadcast the
  CMaNGOS-shaped `SMSG_SPELL_FAILURE` and `SMSG_SPELL_FAILED_OTHER` cleanup
  packets instead of silently dropping the cast.
- Resisted creature spells now encode the miss in `SMSG_SPELL_GO` itself
  instead of listing the target as a hit and only sending a separate
  `SMSG_SPELLLOGMISS`. This is important for real-client spell visual/log
  parity on resisted Immolate.

## Tests Run

- `cargo fmt --package wow-network`
- `cargo check -p wow-network`
- `cargo test -p wow-network spell_packets_match_cmangos_success_shapes --lib`
- `cargo test -p wow-network map_runtime_db_creature_immolate_full_resist_still_sends_go_without_dot --lib`
- `cargo test -p wow-network creature_spell --lib`
- `cargo test -p wow-network spell --lib`
- `$env:CARGO_TARGET_DIR='target\codex-test-rust'; .\scripts\test-rust.cmd`
  passed, including 609 `wow-network` tests.

## Real-Client Verification Needed

- Spawn Burning Blade Neophyte `3196` and test Immolate at level 10 and level
  20. At level 20, expect frequent resists from the level 9-10 caster; proof is
  that the cast completes with visible spell miss/log behavior and does not
  disappear silently.
- Confirm the client receives/uses the `SMSG_SPELL_START` path for NPC casts:
  Immolate has `CastingTimeIndex = 5`, which resolves to a 2000 ms cast in the
  local `SpellCastTimes.dbc`. If the default 1.12 UI still shows no enemy cast
  bar, verify with a combat-log/addon/packet view before changing server
  behavior again.
- Repeat caster death tests: direct spell death, Immolate DoT death, melee
  death, and airborne/jump death. Expected: collapse/release UI, DoT icon clears,
  combat and auto attack stop, creature evades/leashes, and relog preserves
  corpse/release state.
- With a second client nearby, confirm observers see spell start/go, spell
  damage or resist logs, target health changes, death updates, and creature
  evade/leash cleanup.

## Current Follow-Ups

- PvE spell outcome parity still needs broader `spell_proc_event` data support,
  absorbs, immunities/vulnerabilities, and aura modifiers for spell hit/crit,
  damage taken/done, healing, and threat.
- Creature spell parity still needs broader CMaNGOS target selector coverage,
  interrupt/death/leash real-client proof while mid-cast, and dbscript success
  hooks.
- Combat-log polish remains: if generic "enemy hits/crits" lines persist for
  spells, compare exact `SMSG_SPELLNONMELEEDAMAGELOG`, `SMSG_PERIODICAURALOG`,
  `SMSG_SPELLLOGMISS`, and `SMSG_SPELL_GO` payloads against CMaNGOS.
- Starter-zone integration follow-up: GitHub issue #69 tracks the red Kobold
  Camp Cleanup kill-credit smoke. Treat it as separate unless a future death or
  combat-credit change directly touches that path.
- Continue Northshire missing criteria from the playable board: quest
  restrictions, quest item drops from real loot tables, gameobject quest pickup,
  remaining warrior level 1-6 spell parity, combat log feedback, health/rage
  regen, skills/weapon skills, aggro/chase/leash behavior, and patrol runtime
  stability.

## Key Files

- `crates/wow-db/src/world_data.rs`
- `crates/wow-network/src/world/combat/aggro.rs`
- `crates/wow-network/src/world/maps/map.rs`
- `crates/wow-network/src/world/maps/map/creature_combat.rs`
- `crates/wow-network/src/world/maps/map/creature_damage.rs`
- `crates/wow-network/src/world/maps/map/players.rs`
- `crates/wow-network/src/world/maps/map_manager.rs`
- `crates/wow-network/src/world/spells.rs`
- `crates/wow-network/src/world/spells/packets.rs`
- `crates/wow-network/src/world/spells/spell.rs`
- `crates/wow-network/src/world/tests.rs`
