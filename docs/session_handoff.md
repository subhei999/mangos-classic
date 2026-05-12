# Session Handoff

Short operating brief for the next Rust migration session. Durable roadmap
history belongs in `docs/rust_migration_plan.md`, gate status in
`docs/playable_gate_board.md`, and branch split details in
`docs/playable_execution_roadmap.md`.

## Current Branch And Worktree

- Branch: `codex/rusty-mangos`, in the main checkout at
  `C:\Users\subhe\Documents\New project`.
- Current state: uncommitted bulletproof player life/death lifecycle changes are
  present in `wow-network`, including the walking DoT death fix and the
  level-gap creature spell resist/miss completion fix for caster NPCs, plus the
  release-spirit stale-session/`JustDied` race fixes.
- Current user-directed priority: finish the CMaNGOS-shaped map-owned player
  death lifecycle, then real-client test spell/DoT/melee/fall deaths before
  continuing broader Northshire/playable parity.
- Playerbots are disabled by default for normal multiplayer/Northshire testing:
  `config/worldserver.local.toml` has `[playerbots] enabled = false` and
  `[playerbots.random] enabled = false`.
- The local authserver/worldserver were running from `target\debug` during this
  pass, so the first normal `.\scripts\test-rust.cmd` could not overwrite
  `target\debug\authserver.exe`. The verification rerun used
  `CARGO_TARGET_DIR=target\codex-test-rust` and passed.

## Recent Implemented Work

- Added `PlayerDeathState::JustDied` as a runtime-only transition. `JustDied`,
  `Corpse`, and `Ghost` are treated as non-alive for combat, spells, aura ticks,
  AI targeting, movement, and regen.
- Moved authoritative lethal damage handling into `MapRuntime` for creature
  melee, creature spell damage, periodic player aura damage, environmental
  damage, and fall damage. Session code now mirrors map-owned death state instead
  of deciding death presentation or creature evade.
- Added one map-owned death presentation state machine:
  grounded deaths send root, corpse/dead-stand/release-timer update, aura clear,
  and combat cleanup immediately; airborne deaths enter `JustDied`, stop combat
  and input, then present on landing or after a 3 second fallback.
- Centralized death cleanup in `MapRuntime`: clears combat flags, auto attack,
  queued melee spell, active player casts, pending player spell events, creature
  casts targeting the dead player, looting/combo state, death-cleared auras, and
  creature threat/combat. Active creatures evade/leash from the map owner.
- Tightened sync/login/repop behavior so stale session `Alive` state cannot
  resurrect a map-owned corpse, relog mirrors `JustDied`/`Corpse` as corpse or
  release state, releasing spirit clears combat and unroots movement, and
  resurrection restores normal movement and alive state.
- Fixed a related combat regression found during the full suite: zero-damage
  creature melee outcomes now still emit the attacker-state combat event, so
  misses/avoids do not disappear from map-owned creature combat tests.
- Fixed the user-observed walking DoT death pop-up: lethal periodic aura ticks
  clear the aura and then send stat refresh packets, and the generic player
  world-stat update builder was flooring `health = 0` to `health = 1`. That
  post-death aura cleanup packet could briefly resurrect the client visually
  before the death state caught up. World-stat refresh packets now preserve
  zero health, and the Immolate DoT death regression test asserts no direct
  player update sends `UNIT_FIELD_HEALTH = 1` after death.
- Kept the airborne landing guardrail as well: landing packets with nonzero
  `fall_time` now present pending `JustDied` corpses immediately instead of
  waiting for the 3 second fallback.
- Fixed the level-20-vs-entry-3196 "mob does not cast" symptom: the cast was
  usually fully resisted because a level 10 caster has a very high miss/resist
  chance against a level 20 player, and our direct spell completion path treated
  zero final damage as "no event." Creature spells now still consume the active
  cast and emit `SMSG_SPELL_GO` plus `SMSG_SPELLLOGMISS`; resisted Immolate no
  longer applies its DoT, and lethal direct Immolate no longer briefly applies a
  DoT to a dead player.
- Fixed an intermittent Release Spirit no-op: delayed/airborne death
  presentation can be completed by the map update loop while the live session
  cache still says `JustDied`. `CMSG_REPOP_REQUEST` now refreshes death state
  from `MapRuntime` before deciding whether the player is a releasable corpse,
  so a valid Release Spirit click no longer has to wait for relog to enter
  ghost state.
- Tightened the remaining spell/DoT death edge: normal grounded movement now
  clears stale fall tracking in both session and map state, so a later periodic
  tick cannot be misclassified as airborne and delayed for the 3 second fallback.
  Release Spirit also force-completes a lingering map-owned `JustDied`
  presentation before deciding whether to repop, and stale alive session sync no
  longer copies movement/fall state over a map-owned corpse.

## Tests Run

- `cargo fmt --package wow-network`
- `cargo check -p wow-network`
- `git diff --check`
- `cargo test -p wow-network map_runtime_player_world_damage --lib`
- `cargo test -p wow-network death --lib`
- `cargo test -p wow-network aura --lib`
- `cargo test -p wow-network movement --lib`
- `cargo test -p wow-network spell --lib`
- `cargo test -p wow-network combat --lib`
- `cargo test -p wow-network corpse --lib`
- `cargo test -p wow-network ghost --lib`
- `cargo test -p wow-network creature_spell --lib`
- `cargo test -p wow-network map_runtime_creature_dot_death_presents_release_and_clears_combat --lib`
- `cargo test -p wow-network map_runtime_db_creature_lethal_immolate_does_not_apply_dot --lib`
- `cargo test -p wow-network map_runtime_db_creature_immolate_full_resist_still_sends_go_without_dot --lib`
- `cargo test -p wow-network map_runtime_db_creature_immolate_applies_player_dot_ticks --lib`
- `cargo test -p wow-network player_death_evades_active_db_creature_and_starts_return_home --lib`
- `cargo test -p wow-network map_runtime_playerbot_creature_retaliation_damages_bot_runtime --lib`
- `cargo test -p wow-network db_creature_retaliation_can_kill_player --lib`
- `cargo test -p wow-network map_runtime_airborne_death_presentation_fallback_forces_after_timeout --lib`
- `cargo test -p wow-network map_runtime_dot_death_landing_presents_even_with_nonzero_fall_time --lib`
- `cargo test -p wow-network grounded_movement_clears_stale_fall_tracking_before_dot_death --lib`
- `cargo test -p wow-network repop_forces_pending_just_died_presentation_before_refresh --lib`
- `cargo test -p wow-network map_runtime_gameplay_sync_preserves_dead_player_zero_health --lib`
- `cargo test -p wow-network repop_refreshes_session_after_map_presents_delayed_death --lib`
- `cargo test -p wow-network spell_aura_mod_stat_and_resistance_use_generic_template_metadata --lib`
- `cargo test -p wow-network immolate --lib`
- `cargo test -p wow-network --lib` passed with 604 tests.
- `.\scripts\test-rust.cmd` first attempt passed tests/checks but failed final
  `cargo build -p authserver` because `target\debug\authserver.exe` was locked
  by the running local stack.
- `$env:CARGO_TARGET_DIR='target\codex-test-rust'; .\scripts\test-rust.cmd`
  passed.
- `.\scripts\restart-game-stack.cmd` rebuilt and restarted auth/worldserver;
  stack is listening on realmd `127.0.0.1:13724`, world `127.0.0.1:18085`, and
  dashboard `http://127.0.0.1:9091/dashboard`.

## Real-Client Verification Needed

- Spawn Burning Blade Neophyte `3196`, let Immolate/DoT kill the player:
  player should collapse, Release Spirit should appear, DoT icon should clear,
  combat and auto attack should stop, the creature should evade/leash, and relog
  should preserve corpse/release state.
- Repeat death by direct spell, creature melee, and fall/environment damage.
- Test airborne death by jumping while lethal damage lands: the player should
  not regain control, combat should stop immediately, corpse/release should
  present on landing or within the 3 second fallback, and resurrection should
  restore normal movement.
- With a second client nearby, confirm observers see the death update, health
  state, spell/periodic damage logs, and creature evade/leash cleanup.

## Current Follow-Ups

- Real-client proof is still required before calling the death lifecycle fully
  parity-safe. Pay special attention to DoT deaths, jump/airborne deaths, relog
  after death, Release Spirit timing, and movement after resurrection.
- PvE spell outcome follow-ups remain: broader `spell_proc_event`, absorbs,
  immunities/vulnerabilities, aura hit/crit/damage/healing/threat modifiers,
  richer creature spell target selectors, interrupt/death/leash proof, and
  dbscript success hooks.
- Creature caster real-client follow-up: retest `.npc add 3196` and confirm the
  Immolate cast bar plus combat-log spell/periodic damage names. At level 20,
  expect frequent resists from the level 9-10 caster; the important proof is
  that `SMSG_SPELL_GO`/combat-log resist appears and the active cast does not
  disappear silently. If generic hit lines remain, compare exact
  `SMSG_SPELLNONMELEEDAMAGELOG`, `SMSG_PERIODICAURALOG`, and
  `SMSG_SPELLLOGMISS` payloads against CMaNGOS.
- Starter-zone integration follow-up: GitHub issue #69 tracks the red Kobold
  Camp Cleanup kill-credit smoke. Treat it as separate unless a future death or
  combat-credit change directly touches that path.
- Continue Northshire missing criteria from the playable board: quest
  availability restrictions, quest item drops from real loot tables,
  gameobject quest pickup, remaining warrior level 1-6 spell parity, combat log
  feedback, health/rage regen behavior, skills/weapon skills,
  CMaNGOS-like aggro/chase/leash behavior, and patrol runtime stability.

## Key Files

- `crates/wow-network/src/world/entities/player.rs`
- `crates/wow-network/src/world/maps/map.rs`
- `crates/wow-network/src/world/maps/map/damage.rs`
- `crates/wow-network/src/world/maps/map/players.rs`
- `crates/wow-network/src/world/maps/map/creature_combat.rs`
- `crates/wow-network/src/world/maps/map_manager.rs`
- `crates/wow-network/src/world/combat/aggro.rs`
- `crates/wow-network/src/world/death.rs`
- `crates/wow-network/src/world/server/map_update.rs`
- `crates/wow-network/src/world/server/movement.rs`
- `crates/wow-network/src/world/server/session_loop.rs`
- `crates/wow-network/src/world/tests.rs`
- `docs/playable_gate_board.md`
- `docs/playable_execution_roadmap.md`
