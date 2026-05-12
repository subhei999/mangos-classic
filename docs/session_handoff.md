# Session Handoff

Short operating brief for the next Rust migration session. Durable roadmap
history belongs in `docs/rust_migration_plan.md`, gate status in
`docs/playable_gate_board.md`, and branch split details in
`docs/playable_execution_roadmap.md`.

## Current Branch And Worktree

- Branch: `codex/rusty-mangos`, in the main checkout at
  `C:\Users\subhe\Documents\New project`.
- The former `codex/pve-spell-combat-parity` worktree has been integrated into
  this branch: committed spell outcome base plus the previously uncommitted
  creature spell AI, condition/proc/aura follow-ups, and GM level command
  helper are now present in the main integration branch.
- Playerbots are disabled by default for normal multiplayer/Northshire testing:
  `config/worldserver.local.toml` has `[playerbots] enabled = false` and
  `[playerbots.random] enabled = false`; the stack launcher keeps them off
  unless explicitly passed a playerbot flag.
- Current user-directed priority: continue Northshire/playable parity on the
  integrated `codex/rusty-mangos` branch, with PvE spell/combat parity and the
  newly landed inventory/bag and death-state fixes all present together. Use
  CMaNGOS as the behavior reference and keep shared world authority in
  `MapRuntime`.
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
- Current inventory parity follow-up fixes user-observed bag issues:
  CMaNGOS-style rejection for dragging an equipped bag into its own contained
  slots now sends the item GUID context that the client needs to clear the
  cursor/gray drag state, looted bags are created as containers immediately
  instead of only after relog, loot autostore now uses the shared inventory
  store planner so equipped bags count as valid space, selling from equipped
  bags clears the container slot instead of leaving the client one item behind,
  and vendor buyback now handles `CMSG_BUYBACK_ITEM` with Classic buyback
  player fields. A second real-client follow-up now also sends explicit
  `SMSG_INVENTORY_CHANGE_FAILURE` packets when a non-empty equipped bag is
  moved into another bag/storage slot and when right-click auto-equip has no
  free bag slot, preventing gray stuck items after those rejected actions.
- Current P0 death-state fix addresses the real-client overkill/jump repro
  where a player could appear alive after death while mobs ignored them. The
  root cause was `MapRuntime` gameplay sync applying session health `0` and
  then stat/aura refresh flooring health back to `1`. Dead players now stay at
  zero health through map sync, `CMSG_ATTACKSWING` is ignored for dead players,
  and spell casts fail with CMaNGOS `SPELL_FAILED_CASTER_DEAD` (`0x13`).
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
- Post-merge stabilization fixed a flaky spell timing regression fixture: the
  synthetic always-hit Fireball used by
  `cast_time_spell_sends_start_before_delayed_go_and_effects` now also carries
  the CMaNGOS "can't crit" spell attribute so the test continues to prove
  delayed missile ordering instead of randomly observing a crit.
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
- Current PvE spell/combat parity slice keeps the existing CMaNGOS low-level
  spell outcome formulas and adds a shared `SpellCombatUnitSnapshot` input
  boundary for player and DB-creature casters/targets. Direct player spell
  damage against DB creatures now uses the snapshot helper instead of building
  outcome inputs ad hoc, periodic DB-creature aura damage now rolls through the
  same spell outcome path for partial/full resist and crit handling, and
  `MapRuntime` has a map-owned creature-to-player spell damage application path
  that produces `SMSG_SPELLNONMELEEDAMAGELOG`/`SMSG_SPELLLOGMISS`, health
  updates, threat, and observer packets.
- Current creature spell AI slice loads `creature_template.SpellList` and
  DB-backed `creature_spell_list` rows through `ObjectMgr`, keeps per-creature
  spell-list update timers, repeat cooldowns, and active casts in `MapRuntime`,
  and lets active DB creatures select current-victim direct damage spells,
  send `SMSG_SPELL_START`, complete delayed `SMSG_SPELL_GO`, and apply damage
  through the shared creature-to-player spell outcome path. This is intentionally
  limited to hostile direct damage/current-victim targeting; support spells,
  combat/unit conditions, scripts, non-blocking spells, and richer target
  selectors still need follow-up.
- Current creature spell AI follow-up now mirrors more of the CMaNGOS spell-list
  lifecycle: `Availability` is rolled once for the creature spell-list lifetime,
  repeat cooldowns are committed only after a cast is accepted, old
  `creature_template_spells` rows fall back to list id `entry * 100 + setId`
  with `creature_cooldowns`, and delayed creature spells drop cleanly without a
  fake damage log if the victim dies before `SMSG_SPELL_GO`.
- Current creature spell AI target/heal follow-up adds more of the CMaNGOS
  `UnitAI::UpdateSpellLists` shape: support/ranged action rolls happen once per
  spell-list tick, unsupported positive `CombatCondition` ids fail closed,
  hardcoded self/current/current-not-alone and attack target selectors can
  choose from the map-owned threat list, support target selection can choose the
  lowest-health friendly creature by missing health/percent semantics, and
  DB-creature direct heals now run through creature `SMSG_SPELL_START`,
  `SMSG_SPELL_GO`, `SMSG_SPELLHEALLOG`, and health update packets. This is still
  a scoped creature-caster slice, not full CMaNGOS spell parity.
- Current creature spell AI condition follow-up loads DB-backed
  `unit_condition` and `combat_condition` rows through `ObjectMgr`, passes the
  relevant condition cache into map-owned creature spell selection, and filters
  target `UnitCondition`, combat self/target conditions, and friend/enemy count
  predicates before cooldowns or cast packets are committed. The implemented
  unit variables cover common creature-caster checks such as race/class/level,
  health/power percent, combat/enemy counts, melee/ranged range, creature type,
  creature entry, player checks, enemy checks, and dying state. Unsupported
  variables and nonzero world-state expressions deliberately fail closed rather
  than inventing state.
- Current proc slice now treats `proc_charges = 0` as unlimited and decrements
  finite `proc_charges` only after a proc successfully fires.
- Current GM testing helper slice adds CMaNGOS-style level commands for the
  active character: `.levelup`, `.levelup <delta>`, `.level <delta>`,
  `.level +/-<delta>`, and absolute `.character level <level>`. Command parsing
  is case-insensitive for these dot commands. The command persists
  level/XP/vitals, refreshes stat and combat-stat update packets, updates
  map-owned player runtime stats, and adjusts level-capped combat/weapon skill
  caps so higher-level spell and skill testing is easier.
- Server-start hotfix: `CREATURE_SPAWN_SELECT`,
  `get_creature_template_query`, and `get_nearby_creature_spawns` now cast
  creature model gender, other-gender model id, and fallback radius/reach
  expressions. This fixes the real startup failure:
  `template_model_gender1` decoded as `DECIMAL` instead of `u8`.
- Server-start hotfix follow-up: after importing a full ClassicDB world, the
  live stack exposed another decode mismatch where `creature_template.SpellList`
  is signed in SQL but loaded as `u32`. The creature spawn/template queries now
  cast `SpellList` unsigned before decoding.
- GM spawn hotfix: real-client `.npc add 3196` disconnected/logged out the
  session because the single-template GM spawn query omitted the `spell_list`
  alias while `CreatureTemplateQuery` expected it. Static creature loading was
  already correct. `get_creature_template_query` now selects
  `CAST(creature_template.SpellList AS UNSIGNED) AS spell_list`, matching the
  bulk spawn query and allowing DB caster templates to be GM-spawned.
- GM caster spawn hotfix follow-up: the next `.npc add 3196` retry progressed
  past template loading and then disconnected on `creature_spell_list_entry.Id`
  decoding as signed `INT` into Rust `u32`. `get_creature_spell_list` now casts
  nonnegative ids, chances, positions, flags, timers, and target type fields to
  unsigned for ClassicDB's signed spell-list tables.
- Current creature caster real-client fix addresses the first Burning Blade
  Neophyte `.npc add 3196` parity findings: mana-class DB creatures now carry
  runtime mana in create/update blocks, spend mana on accepted casts, restore it
  on respawn/evade, stop active chase motion when starting a spell cast, expose
  the DBC cast time in `SMSG_SPELL_START` for the client cast bar, and apply
  hostile periodic auras such as Immolate to player targets so map-owned aura
  ticks produce real periodic damage logs and health updates.
- Current creature caster cast-bar follow-up fixes a live-socket delivery gap
  for the current player: creature `SMSG_SPELL_START` packets now use the
  active session socket even when the session registry lookup has not yet
  associated that character with the current `SessionId`, while observer packets
  still dispatch through the shared registry. This specifically targets the
  real-client report that Burning Blade Neophyte Immolate applied/spent mana but
  showed no visible cast bar.
- Current creature DoT expiration follow-up fixes the user-observed Immolate
  debuff stuck at `0 sec`: `MapRuntime` remains the owner of live player auras
  after login/apply, so normal session gameplay sync no longer clobbers
  map-owned creature debuffs without an aura-removal packet. Explicit aura
  interruption paths now clear the map-owned player aura list directly instead
  of relying on sync side effects.
- Current death-certainty pass replaces the prior spell/DoT catch-up fix with a
  CMaNGOS-shaped map-owned damage boundary. `MapRuntime` now tracks live player
  death state, direct player/creature HP subtraction in melee, direct spell,
  periodic aura, environmental, and fall damage paths is routed through shared
  world-damage helpers, and overkill immediately produces `Corpse + 0 HP`
  instead of allowing `Alive + 0 HP`. Login no longer silently heals a legacy
  `Alive + 0 HP` row; it preserves the invariant for death handling and logs
  the corrupted state.
- Current death-presentation hotfix adds the missing CMaNGOS
  `KillPlayer()->SendMoveRoot(true)` behavior to Rust player death. Map-owned
  spell/DoT/environmental/fall deaths now produce a direct
  `SMSG_FORCE_MOVE_ROOT` for the controlling client before the death update,
  and the active creature-spell path sends that packet alongside spell damage
  logs. This targets the real-client report that spell death made the character
  technically dead but still upright and controllable with no release flow.
- Current death-presentation follow-up fixes the remaining real-client report
  after force-root: corpse-state player updates now set the CMaNGOS dead stand
  state (`UNIT_FIELD_BYTES_1` stand state 7), login self create blocks preserve
  `health = 0` plus the release-timer byte for non-ghost corpses, observer
  create blocks no longer floor dead players back to 1 HP, and login now
  reconstructs `health = 0`/non-ghost rows as `PlayerDeathState::Corpse`
  instead of `Alive`.
- Current spell/DoT death cleanup pass is based on a direct CMaNGOS death
  lifecycle read: `Unit::DealDamage` lethal damage calls `Kill`, `Kill` sets
  health 0 and player `JUST_DIED`, `SetDeathState(JUST_DIED)` removes all
  non-death-persistent auras and stops movement/combat/casts, and
  `Player::Update` later runs `KillPlayer()` to send root/release/corpse
  presentation. Rust now mirrors that split more closely: map-owned lethal
  damage first records 0 HP/corpse/dead stand state and clears auras, direct
  creature spell death suppresses stale prebuilt aura-apply packets and sends
  the clear packet instead, session sync inherits map-owned dead stand/aura
  state, and the active session finalizer sends the root/release death
  presentation for map-owned DoT/environment/fall deaths.
- Current death/repop follow-up fixes the user-observed post-resurrection
  movement lock. CMaNGOS sends `SendMoveRoot(false)` both when converting the
  corpse player into a ghost in `BuildPlayerRepop()` and when reviving in
  `ResurrectPlayer()`. Rust now sends `SMSG_FORCE_MOVE_UNROOT` during both
  release-to-ghost and corpse/spirit-healer resurrection, accepts the matching
  `CMSG_FORCE_MOVE_UNROOT_ACK` as an expected client acknowledgement, and
  resets session stand state back to standing so map sync does not keep the
  dead stand state after release or resurrection.
- Current death presentation/aura-clear follow-up addresses two remaining
  real-client artifacts: release UI/collapse could lag because map-owned
  lethal damage sometimes sent only `health = 0` and waited for the session
  finalizer to send corpse/release fields, and hostile DoT icons could remain
  visually stuck if the client missed a separate aura-clear packet. Map-owned
  lethal damage now emits the full corpse/dead-stand/release-timer update as
  its first health update, while the later finalizer still handles root,
  persistence, and evade cleanup. Death, ghost, and alive recovery updates now
  clear all visible aura slots before applying the ghost aura, so stale hostile
  debuff slots are cleared even during release/resurrection.
- Current airborne-death follow-up matches the CMaNGOS `KillPlayer()` comment
  that corpse creation is delayed because the player might still be falling.
  Map-owned lethal damage no longer wipes jump/fall movement state, and the
  movement handler now accepts only corpse falling/landing/swim-fall packets
  while still ignoring ordinary dead-player walking/control packets. This
  targets the real-client report that dying while jumping froze the player in
  midair instead of letting the body fall to the ground.
- Test reliability cleanup: the synthetic Fireball-with-DoT fixture now carries
  the same always-hit/cannot-crit flags used by adjacent spell timing tests, so
  random spell outcome rolls no longer make the full Rust suite fail while
  verifying unrelated hotfixes.
- After rebasing onto `d1829107e`, `cargo test -p wow-network --lib` passes
  with 575 tests and `.\scripts\test-rust.cmd` is green, proving the combined
  inventory, death-state, and PvE spell changes. `.\scripts\test-starter-zone-flow.cmd`
  remains tracked separately in GitHub issue #69 for `Kobold kill 2 did not
  grant quest credit`; focused quest/kill unit tests are green and a detached
  clean `origin/codex/rusty-mangos` baseline worktree also failed under the
  same Docker setup, so do not attribute that smoke failure to this PvE spell
  outcome slice unless future evidence links it directly.

## Tests Run

- `cargo fmt --package wow-db --package wow-network --check`
- `cargo check -p wow-network`
- `cargo fmt --package wow-network --check`
- `cargo check -p wow-db`
- `cargo test -p wow-network inventory --lib`
- `cargo test -p wow-network buyback --lib`
- `cargo test -p wow-network loot --lib`
- `cargo test -p wow-network item_create_block_for_looted_bag_is_container_immediately --lib`
- `cargo test -p wow-network autoequip_bag --lib`
- `cargo test -p wow-network --lib`
- `cargo test -p wow-network map_runtime_gameplay_sync_preserves_dead_player_zero_health --lib`
- `cargo test -p wow-network dead_player_attack_swing_does_not_start_map_auto_attack --lib`
- `cargo test -p wow-network spell_cast_failure_rejects_missing_power_gcd_and_duplicate_queue --lib`
- `cargo test -p wow-network death --lib`
- `cargo test -p wow-network combat --lib`
- `cargo test -p wow-network spell --lib`
- `cargo test -p wow-network combat --lib`
- `cargo test -p wow-network creature --lib`
- `cargo test -p wow-network proc_trigger --lib`
- `cargo test -p wow-network db_creature_runtime_create_block_uses_runtime_mana_for_mana_creatures --lib`
- `cargo test -p wow-network map_runtime_db_creature_spell --lib`
- `cargo test -p wow-network map_runtime_db_creature_immolate_applies_player_dot_ticks --lib`
- `cargo test -p wow-network creature_spell_start_packets_use_current_session_socket_without_registry_lookup --lib`
- `cargo test -p wow-network map_runtime_db_creature_dot_survives_session_sync_and_sends_expire_update --lib`
- `cargo test -p wow-network --lib`
- `cargo test -p wow-network map_runtime_player_world_damage --lib`
- `cargo test -p wow-network death --lib`
- `cargo test -p wow-network spell --lib`
- `cargo test -p wow-network player_death_update_sets_health_flags_and_release_timer --lib`
- `cargo test -p wow-network other_player_create_block_preserves_dead_corpse_state --lib`
- `cargo test -p wow-network login_player_create_values_preserve_zero_health_corpse_state --lib`
- `cargo test -p wow-network aura --lib`
- `cargo test -p wow-network map_runtime_player_world_damage --lib`
- `cargo test -p wow-network map_runtime_db_creature_lethal_immolate_clears_preapplied_dot --lib`
- `cargo test -p wow-network death --lib`
- `cargo test -p wow-network aura --lib`
- `cargo test -p wow-network spell --lib`
- `cargo test -p wow-network creature_spell --lib`
- `cargo test -p wow-network map_runtime_gameplay_sync_preserves_dead_player_zero_health --lib`
- `cargo test -p wow-network force_move_unroot_body_matches_root_ack_shape --lib`
- `cargo test -p wow-network death --lib`
- `cargo test -p wow-network corpse --lib`
- `cargo test -p wow-network ghost --lib`
- `cargo test -p wow-network player_death_update_sets_health_flags_and_release_timer --lib`
- `cargo test -p wow-network player_alive_recovery_update_clears_all_visible_aura_slots --lib`
- `cargo test -p wow-network map_runtime_player_world_damage --lib`
- `cargo test -p wow-network map_runtime_db_creature_lethal_immolate_clears_preapplied_dot --lib`
- `cargo test -p wow-network map_runtime_db_creature_immolate_applies_player_dot_ticks --lib`
- `cargo test -p wow-network aura --lib`
- `cargo test -p wow-network spell --lib`
- `cargo test -p wow-network creature_spell --lib`
- `cargo test -p wow-network corpse --lib`
- `cargo test -p wow-network movement --lib`
- `cargo test -p wow-network corpse_falling_movement --lib`
- `.\scripts\test-rust.cmd`
- First `.\scripts\test-rust.cmd` attempt passed tests but failed the final
  `cargo build -p authserver` because the running local stack had
  `target\debug\authserver.exe` locked. After stopping `authserver` and
  `worldserver`, rerunning `.\scripts\test-rust.cmd` passed.
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
- `cargo test -p wow-network spell_damage_outcome --lib`
- `cargo test -p wow-network periodic_damage_tick --lib`
- `cargo test -p wow-network db_creature_spell_damage_to_player --lib`
- `cargo test -p wow-network quest --lib`
- `cargo test -p wow-network kill --lib`
- `cargo test -p wow-network spell --lib`
- `cargo test -p wow-network chilled --lib`
- `cargo test -p wow-network db_creature_swing_timer --lib`
- `cargo test -p wow-network db_creature_chase_motion_duration_applies_temporary_run_speed_slow --lib`
- `cargo test -p wow-network db_creature_slow_aura_retimes_active_chase_and_adjusts_swing_timer --lib`
- `cargo test -p wow-network db_creature_slow_aura_expiration_restores_speed_and_attack_timer --lib`
- `cargo test -p wow-network cast_time_spell_sends_start_before_delayed_go_and_effects --lib`
- `cargo test -p wow-network spell_aura_mod_stat_and_resistance_use_generic_template_metadata --lib`
- `cargo check -p wow-network`
- `cargo test -p wow-network db_creature_spell --lib`
- `cargo test -p wow-network creature_spell_list --lib`
- `cargo test -p wow-network creature_spell --lib`
- `cargo test -p wow-network gm_ --lib`
- `cargo fmt --package wow-db --package wow-network --check`
- `cargo test -p wow-network gm_ --lib`
- `cargo check -p wow-network`
- `powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\import-classic-db-world.ps1 -ClassicDbPath "C:\Users\subhe\Documents\New project\target\classic-db"`
- `.\scripts\restart-game-stack.cmd`
- `.\scripts\test-rust.cmd`
- `cargo test -p wow-network combat_skill_progression --lib`
- `cargo test -p wow-network spell --lib`
- `cargo test -p wow-network combat --lib`
- `cargo test -p wow-db world_data --lib`
- `cargo test -p wow-network aura --lib`
- `cargo test -p wow-network combat --lib`
- `cargo test -p wow-network inventory --lib`
- `cargo test -p wow-network --lib`
- `cargo test -p wow-db world_data --lib`
- `.\scripts\restart-game-stack.cmd`
- `git diff --check`
- `.\scripts\test-rust.cmd`
- `.\scripts\test-starter-zone-flow.cmd` failed with `Kobold kill 2 did not
  grant quest credit`. A reset without full world SQL left the local `mangos`
  schema missing full-world tables; the DB was repaired by importing
  `ClassicDB_1_12_1_z2815.sql.gz` plus post-dump mangos updates `z2816+`, and
  the starter-zone flow still failed with the same Kobold kill-credit symptom.
  A detached clean `origin/codex/rusty-mangos` baseline smoke also failed under
  the same Docker setup, so issue #69 tracks the integration follow-up.
- `.\scripts\restart-game-stack.cmd`

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
- PvE spell outcome follow-ups: finish the non-demo CMaNGOS spell backing that
  this slice intentionally does not fake: world-state expression support and
  broader unit-condition variables for creature spell lists, dbscript success
  hooks, true non-blocking multi-cast execution in the same AI tick, range/LOS
  validation for non-player creature spell targets, friendly dispel and
  missing-buff target selectors, broader interrupt/death/leash cleanup proof,
  absorbs/vulnerabilities/immunities, spell hit/crit aura modifiers,
  healing/energize threat, and broader proc event metadata (`spell_proc_event`,
  procEx, cooldowns, PPM, equipment requirements). Do not branch more ad hoc
  spell damage paths before extending the shared outcome structs.
- Creature caster real-client follow-up: after the cast-start delivery fix,
  retest `.npc add 3196` and confirm the Immolate cast bar. If combat log still
  reports only generic melee hit/crit lines instead of spell damage/periodic
  spell events, do a focused CMaNGOS packet comparison around
  `SMSG_SPELLNONMELEEDAMAGELOG`, `SMSG_PERIODICAURALOG`, `SMSG_SPELLLOGMISS`,
  and the exact target/caster GUID ordering seen by the client.
- Starter-zone integration follow-up: GitHub issue #69 tracks the currently
  red Kobold Camp Cleanup kill-credit smoke. Treat it as a separate quest/combat
  lifecycle investigation unless a future PvE change directly touches the same
  death-credit path.
- Creature radius/reach still mostly use first-display template fallback. A
  later visual-size slice should derive radius/reach from the selected model row
  where CMaNGOS does.
- Loot autostore paths still have backpack-heavy legacy branches. Move them
  onto the shared inventory store planner for any remaining non-creature or
  older helper paths if they show the same backpack-only behavior.
- Real-client smoke needed for the current inventory parity patch: sell full
  and partial stacks from an equipped bag, buy back both, loot into an empty
  equipped bag with a full backpack, and drag an equipped bag onto one of its
  own slots, drag a non-empty equipped bag into another bag, and right-click an
  extra bag while all four bag slots are occupied to confirm the client clears
  the cursor/gray state.
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
- `crates/wow-network/src/world/combat/outcome.rs`
- `crates/wow-network/src/world/combat/aggro.rs`
- `crates/wow-network/src/world/combat/runtime.rs`
- `crates/wow-network/src/world/entities/creature.rs`
- `crates/wow-network/src/world/maps/map.rs`
- `crates/wow-network/src/world/maps/map/creature_combat.rs`
- `crates/wow-network/src/world/maps/map/creature_damage.rs`
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
