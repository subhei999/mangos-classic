# Session Handoff

Short operating brief for the next Rust migration session. Durable roadmap
history belongs in `docs/rust_migration_plan.md`, gate status in
`docs/playable_gate_board.md`, and branch split details in
`docs/playable_execution_roadmap.md`.

## Current Branch And State

- Branch: `codex/rusty-mangos`, in the main checkout at
  `C:\Users\subhe\Documents\New project`.
- Base commit: `9fc8a4c2b`.
- Current user-directed priority: Northshire/playable gameplay parity. The most
  recent implementation pass tightened hunter ranged Auto Shot toward CMaNGOS
  parity: selected ammo, 500 ms shoot wind-up, ranged weapon-delay cadence,
  ammo visual packet payloads, map-owned repeated ranged swings, preserving
  ranged auto-repeat when the client sends melee `CMSG_ATTACKSTOP`, CMaNGOS-like
  triggered `SMSG_SPELL_START` / `SMSG_SPELL_GO` packet order at each shot
  release, delayed projectile impact damage based on Auto Shot missile speed,
  CMaNGOS-shaped spell weapon-damage impact logs that avoid melee
  attacker-state updates, and ranged-to-melee auto attack handoff when the
  target is in melee reach. A follow-up fix also preserves the ranged weapon
  cooldown when Auto Shot is canceled/restarted, preventing toggle spam from
  shortening the timer.
- Worktree is intentionally dirty. In addition to the ranged/ammo changes, it
  still contains the pre-existing handler/router and map-runtime file moves
  under `crates/wow-network/src/world/{handlers,map_runtime}` plus related
  typed-dispatch edits. Do not revert those unrelated changes.
- A local `target\debug\authserver.exe --config config\authserver.local.toml`
  and `worldserver` process were already running from the default target during
  verification. Use an isolated `CARGO_TARGET_DIR` or stop those processes
  before rebuilding the default `target\debug` binaries.
- Playerbots are disabled by default for normal multiplayer/Northshire testing:
  `config/worldserver.local.toml` has `[playerbots] enabled = false` and
  `[playerbots.random] enabled = false`.

## Current Goal And Recommended Next Task

- Goal: continue closing the user-observed missing Northshire/playable systems,
  with real-client playtesting as the Checkpoint 2 grader. Do not add or
  maintain a Northshire grading harness.
- Recommended next task: real-client smoke the hunter flow with a low-level bow
  user and real arrow stacks. Verify `CMSG_SET_AMMO`, `PLAYER_AMMO_ID`, Auto
  Shot spell id `75`, 500 ms initial shoot delay, repeated shot animation with
  the triggered start/result/go packet trio at release, weapon-speed repeat
  cadence, ammo projectile visuals, ammo consumption, damage landing when the
  projectile reaches the target instead of at release, ranged white-hit logs,
  skill-ups, target death/loot, no-ammo failure, and Auto Shot
  cancellation/close-range failure handing off to melee auto attack without
  requiring another right-click. Also verify that toggling Auto Shot off and
  back on after a shot does not allow shots faster than ranged weapon speed.
- After hunter proof, continue the remaining board items: quest restrictions,
  quest item drops from real loot tables, gameobject quest pickup, combat log
  polish, health/rage regeneration, skills and weapon skills, aggro/chase/leash,
  and patrol runtime stability.

## Recent Implemented Work

- Added typed `CMSG_SET_AMMO` support in `wow-proto` and `wow-network`, including
  world opcode mapping, packet parsing, dispatch, and wire-name reporting.
- Added character ammo persistence through `characters.ammoId`: enum loading,
  session state, login bootstrap, and `update_character_ammo_id`.
- Implemented CMaNGOS-shaped ammo selection in inventory handling. Ammo can be
  cleared, must exist in the player's inventory, must be projectile ammo
  (`class = 6`, `inventory_type = 24`), must match bow/crossbow arrow or gun
  bullet requirements, and must pass normal item-use checks.
- Initial player updates and ammo changes now send `PLAYER_AMMO_ID` and refresh
  derived combat stats. Ranged weapon damage includes compatible ammo DPS scaled
  by ranged weapon attack time.
- Spell profile derivation now recognizes Auto Shot as an auto-repeat ranged
  spell from CMaNGOS spell attributes (`SPELL_ATTR_USES_RANGED_SLOT` and
  `SPELL_ATTR_EX2_AUTO_REPEAT`) rather than hardcoding combat behavior by name.
- Map runtime now stores player auto-attack kind: melee or ranged with spell id
  and ranged phase. Ranged auto attack is scheduled by the map owner, uses a
  CMaNGOS-shaped 500 ms initial wind-up, resets that wind-up while the player is
  moving, and then repeats from `ranged_attack_time_ms`.
- `SMSG_SPELL_START` and `SMSG_SPELL_GO` can now carry CMaNGOS `CAST_FLAG_AMMO`
  plus projectile display id/inventory type. Auto Shot now treats the 500 ms
  wind-up as CMaNGOS' internal auto-repeat delay, then sends the triggered shot
  packets at release in CMaNGOS order: `SMSG_SPELL_START`, `SMSG_CAST_RESULT`,
  and `SMSG_SPELL_GO`. This replaced the earlier split that sent start 500 ms
  before go, which still made repeat arrows appear to come from the player body
  in the real client.
- Ranged swing execution validates live target, ranged weapon, compatible ammo,
  range, facing, and LOS; computes ranged outcome from ranged stats and weapon
  skill; consumes one ammo at release; emits spell start/go immediately; then
  queues a map-owned ranged impact event using Auto Shot projectile speed.
  Attacker-state/miss logs, creature health updates, retaliation, skill advance,
  and death/loot finalization now happen at projectile impact.
- `CMSG_CANCEL_AUTO_REPEAT_SPELL` now attempts to transition ranged auto-repeat
  to melee auto attack when the current ranged target is in melee reach; only
  failed transitions clear map-owned auto attack state. Ranged wind-up/release
  validation failures use the same close-range melee handoff path.
- Fixed a real-client repeat blocker where `CMSG_ATTACKSTOP` used the generic
  auto-attack clear path and erased ranged Auto Shot after the opener. The map
  runtime now has a melee-only stop path, matching CMaNGOS' separation between
  melee attack state and `CURRENT_AUTOREPEAT_SPELL`.
- Auto Shot pending impact events now share the existing map-owned delayed spell
  event scheduler. Stale events are dropped if the target dies/respawns before
  impact, and the delay uses the spell projectile speed with CMaNGOS' 5-yard
  minimum travel distance.
- Auto Shot projectile impact now suppresses `SMSG_ATTACKERSTATEUPDATE` and
  sends `SMSG_SPELLNONMELEEDAMAGELOG`, matching CMaNGOS' spell weapon-damage
  path. This avoids telling the client that each arrow impact was a melee swing,
  which was making repeat shots fall back to melee/dagger-looking stance.
- Spell range validation now treats `SPELL_RANGE_FLAG_RANGED` as the complement
  of melee reach and returns `SPELL_FAILED_TOO_CLOSE` for the minimum-range side.
  Auto Shot close-range failures still use the existing ranged-to-melee handoff
  path, so a target entering melee reach should start melee auto attack without
  another right-click.
- Auto Shot now stores a map-owned ranged auto-repeat next-shot timer separate
  from active target/kind. Starting or restarting Auto Shot uses
  `max(existing_ranged_timer, now + 500ms)`, matching CMaNGOS'
  `_UpdateAutoRepeatSpell` behavior where interrupting
  `CURRENT_AUTOREPEAT_SPELL` does not clear the spell cooldown. Canceling,
  target clearing, or switching to melee no longer resets the ranged weapon
  cooldown, while player death clears it with the rest of combat state.

## Tests Run

- Baseline attempt before edits:
  `$env:CARGO_TARGET_DIR='target\codex-ranged-baseline'; .\scripts\test-rust.cmd`
  timed out after about 124 seconds without a useful failure signal.
- `cargo check -p wow-network` passed after the ranged/ammo implementation.
- `cargo test -p wow-network ranged_weapon_damage_adds_compatible_ammo_dps_for_weapon_speed --lib`
  passed.
- `cargo test -p wow-network spell_cast_profiles_are_derived_from_cmangos_spell_template_fields --lib`
  passed.
- `cargo test -p wow-network --lib` passed with 625 tests.
- `.\scripts\test-rust.cmd` against the default target mostly passed but failed
  at the final `cargo build -p authserver` because Windows could not replace
  the running `target\debug\authserver.exe`.
- `$env:CARGO_TARGET_DIR='target\codex-ranged-script-final'; .\scripts\test-rust.cmd`
  passed fully.
- `cargo check -p wow-network` passed after the Auto Shot wind-up/projectile
  packet pass.
- `cargo test -p wow-network ranged_auto_attack_uses_cmangos_windup_before_weapon_timer --lib`
  passed.
- `cargo test -p wow-network ranged_auto_attack_movement_does_not_shortcut_long_weapon_cooldown --lib`
  passed.
- `cargo test -p wow-network ranged_spell_packets_include_cmangos_ammo_visual_payload --lib`
  passed.
- `cargo test -p wow-proto --lib` passed with 23 tests.
- `cargo test -p wow-network --lib` passed with 628 tests.
- `$env:CARGO_TARGET_DIR='target\codex-ranged-parity-final'; .\scripts\test-rust.cmd`
  passed fully, including clippy, unit/doc tests, and authserver/worldserver
  builds in the isolated target dir.
- Baseline for the repeat fix:
  `$env:CARGO_TARGET_DIR='target\codex-ranged-repeat-baseline'; .\scripts\test-rust.cmd`
  passed fully before the `CMSG_ATTACKSTOP` change.
- `cargo test -p wow-network player_attack_stop_preserves_ranged_auto_repeat_spell --lib`
  passed.
- `cargo test -p wow-network player_attack_stop_broadcasts_to_nearby_observer --lib`
  passed.
- `cargo test -p wow-network player_attack_stop_clears_queued_next_melee_spell_without_active_target --lib`
  passed.
- `cargo test -p wow-network ranged_auto_attack_uses_cmangos_windup_before_weapon_timer --lib`
  passed.
- `cargo test -p wow-network ranged_auto_attack_movement_does_not_shortcut_long_weapon_cooldown --lib`
  passed.
- `cargo test -p wow-proto --lib` passed with 23 tests.
- `cargo test -p wow-network --lib` passed with 629 tests.
- `$env:CARGO_TARGET_DIR='target\codex-ranged-repeat-final'; .\scripts\test-rust.cmd`
  passed fully, including clippy, unit/doc tests, and authserver/worldserver
  builds in the isolated target dir.
- Latest focused repeat-animation/transition tests:
  `cargo test -p wow-network ranged_auto_attack_uses_cmangos_windup_before_weapon_timer --lib`,
  `cargo test -p wow-network ranged_auto_attack_movement_does_not_shortcut_long_weapon_cooldown --lib`,
  `cargo test -p wow-network player_attack_stop_preserves_ranged_auto_repeat_spell --lib`,
  and
  `cargo test -p wow-network ranged_auto_repeat_cancel_transitions_to_melee_when_target_is_in_reach --lib`
  all passed.
- `cargo test -p wow-network --lib` passed with 630 tests.
- `cargo test -p wow-proto --lib` passed with 23 tests.
- First run of
  `$env:CARGO_TARGET_DIR='target\codex-ranged-draw-transition-final'; .\scripts\test-rust.cmd`
  failed at `cargo fmt --check`; after `cargo fmt --package wow-network`, the
  same script passed fully, including clippy, unit/doc tests, and
  authserver/worldserver builds in the isolated target dir.
- Latest repeat-animation correction:
  `cargo test -p wow-network ranged_auto_attack_uses_cmangos_windup_before_weapon_timer --lib`,
  `cargo test -p wow-network ranged_auto_attack_movement_does_not_shortcut_long_weapon_cooldown --lib`,
  `cargo test -p wow-network player_attack_stop_preserves_ranged_auto_repeat_spell --lib`,
  `cargo test -p wow-network ranged_auto_repeat_cancel_transitions_to_melee_when_target_is_in_reach --lib`,
  and
  `cargo test -p wow-network ranged_spell_packets_include_cmangos_ammo_visual_payload --lib`
  all passed.
- `cargo test -p wow-network --lib` passed with 630 tests.
- `$env:CARGO_TARGET_DIR='target\codex-ranged-triggered-start-final'; .\scripts\test-rust.cmd`
  passed fully, including clippy, unit/doc tests, and authserver/worldserver
  builds in the isolated target dir.
- Latest projectile-impact correction:
  `cargo test -p wow-network ranged_auto` passed with 5 ranged auto-repeat
  tests, and
  `cargo test -p wow-network auto_shot_pending_impact_delays_damage_until_projectile_due`
  passed.
- `$env:CARGO_TARGET_DIR='target\codex-ranged-projectile-full'; .\scripts\test-rust.cmd`
  passed fully, including clippy, 632 `wow-network` tests, doc tests, and
  authserver/worldserver builds in the isolated target dir.
- Latest Auto Shot stance/too-close correction:
  `cargo test -p wow-network auto_shot_pending_impact_delays_damage_until_projectile_due --lib`,
  `cargo test -p wow-network map_runtime_player_spell_target_validation_treats_ranged_min_range_as_melee_complement --lib`,
  `cargo test -p wow-network ranged_auto_repeat_cancel_transitions_to_melee_when_target_is_in_reach --lib`,
  and `cargo test -p wow-network ranged_auto --lib` all passed.
- `cargo test -p wow-network --lib` passed with 633 tests.
- `$env:CARGO_TARGET_DIR='target\codex-ranged-autoshot-parity-final'; .\scripts\test-rust.cmd`
  passed fully, including fmt, clippy, unit/doc tests, and authserver/worldserver
  builds in the isolated target dir.
- Latest Auto Shot cooldown toggle exploit correction:
  `cargo test -p wow-network ranged_auto_repeat_restart_preserves_weapon_cooldown_after_cancel --lib`,
  `cargo test -p wow-network ranged_auto --lib`,
  `cargo test -p wow-network player_attack_stop_preserves_ranged_auto_repeat_spell --lib`,
  `cargo test -p wow-network ranged_auto_repeat_cancel_transitions_to_melee_when_target_is_in_reach --lib`,
  and
  `cargo test -p wow-network auto_shot_pending_impact_delays_damage_until_projectile_due --lib`
  all passed.
- `cargo test -p wow-network --lib` passed with 634 tests.
- `$env:CARGO_TARGET_DIR='target\codex-ranged-cooldown-final'; .\scripts\test-rust.cmd`
  passed fully, including fmt, clippy, unit/doc tests, and authserver/worldserver
  builds in the isolated target dir.

## Real-Client Verification Needed

- Hunter ranged flow still needs live-client proof. Verify arrows can be chosen
  as ammo, Auto Shot starts and repeats, every repeat now animates from the bow
  instead of appearing to launch from the player body or switching to dagger
  stance, arrows are consumed, damage/logs appear at projectile impact rather
  than at release, weapon skill advances, target death/loot still works, and the
  client receives clear failures for missing ammo and target-too-close. Verify
  specifically that canceling/restarting Auto Shot after a release keeps the
  ranged weapon cooldown instead of allowing rapid-fire toggle shots.
- Verify switching out of Auto Shot at close range starts melee auto attack
  without another right-click. If repeat animation is still wrong, capture or
  compare real CMaNGOS packet order/fields around repeat Auto Shot; the current
  Rust path now avoids the known bad melee attacker-state packet on arrow
  impact, but real packets may expose another client-facing state update.
- Verify selected ammo persistence across logout/relogin through
  `characters.ammoId`.
- Re-run `.\scripts\test-rust-db.cmd` only if the next pass broadens character
  DB behavior beyond the narrow `ammoId` update; it was not run in this pass.
- Thrown and wand parity are not complete. This slice targets bow/gun/crossbow
  Auto Shot and projectile ammo requirements.
- Quiver/ammo-pouch ranged haste is intentionally not complete; GitHub issue
  #70 tracks adding `SPELL_AURA_MOD_RANGED_AMMO_HASTE` to ranged attack time.

## Current Follow-Ups

- If real-client testing shows selected ammo remains after the final stack is
  consumed, decide whether CMaNGOS clears `ammoId` immediately or leaves the
  selected ammo id and fails the next shot. The current implementation leaves
  the selection and sends `SPELL_FAILED_NO_AMMO` on the next invalid shot.
- Broaden ranged combat parity after hunter proof: exact ranged combat-log
  payloads, animation timing, shoot/cast state UI behavior, PvP/duel target
  support, and any class/race-specific weapon-skill wrinkles.
- GitHub issue #70 tracks quiver/ammo-pouch haste not yet adjusting
  `ranged_attack_time_ms`.
- Existing P2 protocol/router cleanup remains in the same dirty worktree. Keep
  future changes focused and avoid mixing unrelated gameplay work into that
  refactor unless it is required for correctness.

## Key Files

- `crates/wow-proto/src/world_packets.rs`
- `crates/wow-db/src/character/{queries.rs,state.rs,types.rs}`
- `crates/wow-network/src/world/packets.rs`
- `crates/wow-network/src/world/opcodes.rs`
- `crates/wow-network/src/world/wire.rs`
- `crates/wow-network/src/world/session.rs`
- `crates/wow-network/src/world/server/{dispatch.rs,player_login.rs,world_session.rs}`
- `crates/wow-network/src/world/handlers/inventory.rs`
- `crates/wow-network/src/world/spells.rs`
- `crates/wow-network/src/world/spells/spell_mgr.rs`
- `crates/wow-network/src/world/combat/{lifecycle.rs,outcome.rs}`
- `crates/wow-network/src/world/entities/{player.rs,update_data.rs}`
- `crates/wow-network/src/world/map_runtime/{map.rs,map/players.rs,map_manager.rs}`
- `crates/wow-network/src/world/tests.rs`
