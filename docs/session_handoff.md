# Session Handoff

Short operating brief for the next Rust migration session. Durable roadmap
history belongs in `docs/rust_migration_plan.md`, gate status in
`docs/playable_gate_board.md`, and focused feature plans in their own docs.

## Current Branch And State

- Branch: `codex/rusty-mangos`
- Workspace: `C:\Users\subhe\Documents\New project`
- Latest pushed checkpoint before this task: `042fe910c Fix gameplay parity issues from playtest`
- Current uncommitted state includes the live-proven quest reward UI
  chain-advance fix plus the next playtest parity slice for death/rage,
  combat-cast swing suppression, consumable regen refresh, and evading-target
  spell miss parity, CMaNGOS-shaped near-barrier/tree chase pathing fixes, and
  the vendor buyback cursor/timestamp parity fix:
  - `crates/wow-network/src/world/handlers/quest.rs`
  - `crates/wow-network/src/world/tests/quests_reputation.rs`
  - `bins/starter-zone-flow-test/src/main.rs`
  - `crates/wow-network/native/mmap_path.cpp`
  - `crates/wow-network/src/world/combat/melee.rs`
  - `crates/wow-network/src/world/combat/motion.rs`
  - `crates/wow-network/src/world/handlers/death.rs`
  - `crates/wow-network/src/world/map_runtime/systems/creature_combat.rs`
  - `crates/wow-network/src/world/map_runtime/systems/creature_motion.rs`
  - `crates/wow-network/src/world/map_runtime/systems/damage.rs`
  - `crates/wow-network/src/world/map_runtime/systems/players.rs`
  - `crates/wow-network/src/world/spells.rs`
  - `crates/wow-network/src/world/spells/casting.rs`
  - `crates/wow-network/src/world/handlers/vendor.rs`
  - `crates/wow-network/src/world/packet_builders/death.rs`
  - `crates/wow-network/src/world/tests/death_aggro.rs`
  - `crates/wow-network/src/world/tests/character_inventory_social.rs`
  - `crates/wow-network/src/world/tests/map_runtime_grids_playerbots.rs`
  - `crates/wow-network/src/world/tests/navigation_motion.rs`
  - `crates/wow-network/src/world/tests/player_runtime_auras.rs`
  - `crates/wow-network/src/world/tests/spells.rs`
  - `docs/session_handoff.md`
- Untracked `logs/` still exists locally and should not be committed unless the
  user explicitly asks after a size review.

## Current Goal

Latest user-directed priority in progress/completed this session: creature AI
spell scheduling parity. Gameplay testing reported creatures casting spells too
immediately on combat start and sometimes appearing to cast twice, with Webwood
Silkspinner as the useful hint. Follow-up smoke found Defias Cutpurse spamming
Backstab. CMaNGOS comparison found the relevant owner split:

- `src/game/AI/BaseAI/UnitAI.cpp` runs generic creature spell lists every 1200ms
  through `GENERIC_ACTION_SPELL_LIST` and seeds initial spell cooldowns on
  `EnterCombat`.
- `src/game/AI/EventAI/CreatureEventAI.cpp` runs EventAI timer-executed events
  only on `EVENT_UPDATE_TIME = 500ms` pulses. `EnterCombat` schedules the first
  EventAI update and anchors timer-in-combat initial delays from the combat
  start, not from an arbitrary later spell check.
- EventAI chance failures call `ResetEvent`, so timer-executed events retry on
  their repeat timer instead of rolling again every server tick.
- Legacy `creature_template_spells` are loaded into a disabled compatibility
  spell list (`LoadCreatureTemplateSpells` sets `Disabled = true`) and do not
  drive generic `UnitAI::UpdateSpellLists()` autocasts. Defias Cutpurse entry
  `94` has no modern `creature_spell_list` rows for `9400`; it only has legacy
  `creature_template_spells` Backstab `53`, so Rust must not autocast it from
  the active spell-list path.

Rust now mirrors that shape for map-owned creature EventAI:

- DB creature combat state records `started_at`.
- Runtime creatures track `next_event_ai_update_at`.
- In-combat timer/range/facing/missing-aura EventAI casts are gated by the
  CMaNGOS 500ms update pulse, while aggro casts still bypass that pulse.
- Timer-in-combat initial delays are anchored to combat start.
- Failed EventAI chance rolls reset the event repeat cooldown.
- Active Rust creature spell-list loading now only returns modern
  `creature_spell_list` rows. The previous fallback to
  `creature_template_spells` was removed from `wow-db::get_creature_spell_list`
  because CMaNGOS marks those legacy rows disabled for generic AI. This should
  stop Defias Cutpurse from spamming Backstab without inventing an energy or
  cooldown bandaid.

Previously completed: vendor buyback slot parity. Gameplay test reports said
buyback items were confusing, did not behave like a normal 12-slot list, and the
final slot could appear unusable. CMaNGOS comparison found
the relevant owner in `src/game/Entities/Player.cpp`:

- `BUYBACK_SLOT_START = 69`, `BUYBACK_SLOT_END = 81`; slots `69..80` are the
  12 buyback slots.
- `Player::AddItemToBuyBackSlot` leaves `m_currentBuybackSlot` parked on slot
  `80` after the list reaches the final slot. It does not wrap the cursor back
  to `69`; once full, replacement is driven by free/oldest slot selection.
- `Player::RemoveItemFromBuyBackSlot` retargets the cursor to a cleared slot
  only when the current cursor slot is occupied, which is what lets slot `80`
  be reused if it is bought back/cleared from a full list.
- Buyback timestamps are increasing session-relative values, so "oldest slot"
  replacement has stable ordering.

Rust now mirrors that shape in `crates/wow-network/src/world/handlers/vendor.rs`:

- The buyback cursor no longer wraps after slot `80`.
- `next_buyback_slot` prefers the cursor when free, then a free slot, then the
  oldest timestamp when the list is full.
- Clearing a buyback slot updates the cursor when the cursor still points at an
  occupied slot.
- Buyback entries now receive monotonic session-local timestamps instead of all
  entries using the same `30h` value.

Previously completed: enemy cry-for-help / family assistance on combat
initiation and while creatures move through an existing fight.
CMaNGOS source comparison found two separate ownership points:

- `src/game/Entities/Unit.cpp`: when a creature enters combat,
  `creature->CallAssistance()` runs from the creature combat-entry path, not
  only from sight aggro. `Creature::CallAssistance()` uses DB `CallForHelp`
  when set, falls back to the configured family-assistance radius, marks the
  caller as already called, and sends a delayed assistance AI event.
- `src/game/AI/BaseAI/UnitAI.cpp`: `MoveInLineOfSight` calls `CheckForHelp`
  when a creature sees another creature already in combat. `CheckForHelp` uses
  `CreatureCheckForHelpRadius = 5y`, requires both creatures' check-for-help
  gate to be enabled, requires LOS between the helper and fighting creature,
  and requires the helper to be able to attack the fighting creature's victim.
  `src/game/Entities/Unit.cpp` disables `CanCheckForHelp` for
  `CreatureCheckForHelpAggroDelay = 2000ms` after aggro.

Rust now routes player-initiated creature combat through CMaNGOS-shaped
assistance helpers:

- Session combat entry (`begin_db_creature_combat_with_assistance`) starts the
  primary creature, sends the attack-start/flag packets, selects nearby helpers,
  starts them against the same player, and sends matching packets.
- Melee/hostile aura retaliation and direct spell damage both use that helper,
  so pulling by hit or spell can bring nearby same-faction mobs.
- Map-runtime channel/dynamic-object combat entry now uses
  `begin_db_creature_combat_packets_with_assistance`, so periodic spell-owned
  combat startup does not keep a separate primary-only implementation.
- Map-owned creature relocation now also runs `CheckForHelp` parity:
  dragging a fighting mob past another eligible hostile can pull the helper
  after the 2s aggro delay, and an idle/patrol creature walking by an existing
  fight can join that fight.

Previously completed pathfinding/barrier task: compare and fix mob pathfinding
around barriers/trees with CMaNGOS-shaped ownership. CMaNGOS source comparison
found these relevant mismatches:

- `PathFinder::getPolyByLocation` tries a 5 yard `findNearestPoly` box, then
  retries a 10 yard box before treating a point as off-mesh. Rust's native mmap
  bridge only used the 5 yard lookup, so chase destinations near
  walls/fences/barriers could return no path instead of snapping to the
  reachable navmesh like CMaNGOS.
- `WorldObject::GetNearPointAt`, used by
  `ChaseMovementGenerator::_getLocation`, does not blindly use the first
  target-ring point. If the original melee slot has LOS/collision trouble, it
  rotates through nearby angles and uses the first LOS-valid candidate. Rust's
  map-owned chase-slot destination used one raw `target + angle * distance`
  point, so kiting around a tree could pick a slot on the obstructed side and
  leave the mob stuck.
- `Unit::CanReachWithMeleeAttack` is reach/distance-only for NPCs. It does not
  fold LOS, mmap path availability, or evade state into melee reach. Rust had
  navigation guardrails inside both session and map-owned reach checks, so a mob
  close to a tree/object could remain in a chase/repath loop instead of letting
  normal reach or motion ownership decide the next action.
- `PathFinder` preserves `PATHFIND_INCOMPLETE` when Detour only returns a
  partial poly path or the requested point had to be snapped far onto the mesh.
  Rust's native mmap bridge only returned a point count, so partial endpoints
  near barriers were misclassified as normal paths and could become stable stuck
  chase destinations.

Recently addressed playtest parity bugs:

- Death package: rage must clear on death/revive, ghost/death aura must be a
  negative debuff aura, and Night Elf ghosts should receive Wisp Form.
- Combat-cast timing: white swings must not fire during a combat-interruptible
  cast such as Hearthstone, and overdue swing timers should be reset instead of
  released when the cast is active.
- Consumable regen refresh: eating/drinking over an existing food/drink aura
  should not stand the player up and immediately sit them back down.
- Evading-target spell parity: hostile spells cast at an evading creature should
  follow CMaNGOS range validation first, then report `SPELL_MISS_EVADE` through
  `SMSG_SPELL_GO` instead of failing cast validation as out of range.

Previously completed and user live-confirmed: the quest completion UI bug where
rewarding a chain quest completed server state but left the client dialogue
stuck instead of advancing to the next quest.

CMaNGOS reference shape:

- `WorldSession::HandleQuestgiverChooseRewardOpcode` rewards the quest.
- It sends `SMSG_QUESTGIVER_QUEST_COMPLETE`.
- It then calls `Player::GetNextQuest(guid, pQuest)` and, when the same
  questgiver starts `Quest::GetNextQuestInChain()`, sends
  `SMSG_QUESTGIVER_QUEST_DETAILS` for the next quest.
- It does not rely on a blind gossip close for this path.

Rust now follows that shape: after a successful reward, the reward handler
checks `next_quest_in_chain`, verifies the same questgiver starts that quest,
loads the next template, checks the player can now take it, and sends
`SMSG_QUESTGIVER_QUEST_DETAILS`. The user live-smoked this with the Northshire
chain and confirmed it fixed the issue.

CMaNGOS death reference shape:

- `Player::BuildPlayerRepop()` casts Night Elf Wisp Form spell `20584` before
  ghost spell `8326`.
- `Player::ResurrectPlayer()` removes ghost/wisp auras and sets rage to `0`.
- Rust now clears rage in map-owned lethal damage, deferred death presentation,
  release spirit, resurrection, and session finalization paths; death/revive
  update packets also write `UNIT_FIELD_POWER2 = 0`.

CMaNGOS spell/evade reference shape:

- `Spell::CheckRange()` validates distance/facing for the target without
  treating evade mode as an out-of-range cast failure.
- `Unit::SpellHitResult()` then returns `SPELL_MISS_EVADE` for units in evade
  mode.
- Rust now follows that split: map-owned hostile spell validation allows an
  in-range evading creature through, while `player_db_creature_spell_target_outcome`
  emits `SPELL_MISS_EVADE` before damage/aura effects run.

CMaNGOS mmap/path reference shape:

- `src/game/MotionGenerators/PathFinder.cpp` defines `NearPolySearchBound` as
  `{ 5.0f, 5.0f, 5.0f }` and `FarPolySearchBound` as
  `{ 10.0f, 10.0f, 10.0f }`.
- `PathFinder::getPolyByLocation()` retries the far search bound before giving
  up on start/end points.
- Rust now mirrors that in `crates/wow-network/native/mmap_path.cpp` for both
  direct path and random path entry points.

CMaNGOS chase near-point reference shape:

- `ChaseMovementGenerator::_getLocation` calls `target->GetNearPoint(...)`.
- `WorldObject::GetNearPointAt` tests the original angle first; when LOS fails,
  it scans adjacent angles using `ObjectPosSelector`'s step shape based on
  `atan(1.8 * searcher_radius / distance)`.
- Rust now mirrors that angle retry before building the mmap chase path, using
  map-owned geometry/LOS as the candidate validity check.

CMaNGOS melee reach reference shape:

- `Unit::CanReachWithMeleeAttack` checks combat reach and distance; NPC reach is
  not gated by LOS or path availability.
- Rust now keeps navigation guardrails on generated aggro/chase motion, while
  session and map-owned melee reach checks are distance-only like CMaNGOS.

CMaNGOS incomplete-path reference shape:

- `PathFinder::BuildPolyPath()` marks paths `PATHFIND_INCOMPLETE` when the
  start/end point is far from its navmesh polygon or the final Detour poly is
  not the requested end poly.
- `ChaseMovementGenerator::DispatchSplineToPosition(..., checkReachable=true)`
  allows incomplete paths only when the final path point can still reach the
  target by combat reach.
- Rust's native mmap bridge now returns CMaNGOS-style path flags alongside path
  points, and chase motion rejects incomplete endpoints that are outside melee
  reach instead of treating them as normal destinations.

CMaNGOS combat-leash refresh reference shape:

- `src/game/Combat/CombatManager.cpp` owns the pursuit/combat leash timer, not
  `CreatureAI::DamageTaken`.
- `TriggerCombatTimer(bool)` sets the timer to `m_owner->GetPursuit()` and
  refreshes `m_lastRefreshPos` to the creature/owner position.
- The file comment and implementation say the timer is refreshed by direct
  player damage to the creature and direct creature damage to the player; periodic
  aura/dynamic-object ticks should not refresh it.
- Rust now keeps `CreatureCombatLeashState` map-owned and follows the direct-only
  refresh policy.

## What Changed Recently

- Audited CMaNGOS creature AI scheduling against Rust. Rust already had generic
  spell-list initial/repeat cooldowns, but in-combat EventAI was checked every
  Rust world tick instead of CMaNGOS' 500ms EventAI pulse. Rust now gates
  timer-executed EventAI casts with `next_event_ai_update_at`, anchors
  timer-in-combat initial delays to combat start, and resets repeat cooldowns
  when EventAI chance rolls fail. This should reduce too-immediate combat casts
  and over-frequent retries without special-casing Webwood Silkspinners.
- Audited Defias Cutpurse Backstab spam against local CMaNGOS DB/source. Entry
  `94` has legacy `creature_template_spells` Backstab `53`, no
  `creature_cooldowns`, and no modern `creature_spell_list_entry`/`9400`.
  CMaNGOS marks legacy template spell lists disabled for generic UnitAI, while
  Rust was using them as active spell-list rows. Removed that fallback from the
  active Rust creature spell-list query.
- Added `send_next_chain_quest_details_after_reward(...)` in the quest handler.
- The normal reward path now sends `SMSG_QUESTGIVER_QUEST_COMPLETE` first, then
  attempts the CMaNGOS-style next-chain details packet.
- Added a focused unit test proving rewarded quest `783` opens quest `7`
  details from the same questgiver when the previous quest is rewarded.
- Updated the starter-zone fixture seeding to include `PrevQuestId` and
  `NextQuestInChain`.
- Updated the starter-zone flow check so rewarding `A Threat Within` must be
  followed by `SMSG_QUESTGIVER_QUEST_DETAILS` containing `Kobold Camp Cleanup`.
- Death update packets now include `UNIT_FIELD_POWER2 = 0`; map/session death
  transitions clear rage authoritatively instead of relying on regen.
- Night Elf ghost presentation now places Wisp Form `20584` and Ghost `8326` in
  negative/debuff aura slots.
- Map-owned active casts suppress overdue melee swings without mutating the
  stored weapon timer; cancelled long casts resume from the real swing timer
  instead of waiting for the full cast duration.
- Cast-time spells retime the auto-attack timer when the cast starts, not only
  after spell completion.
- Food/drink item uses skip the standing-cancels path when the item is a
  refreshable periodic regen consumable, preventing the stand/sit flicker.
- Player spell validation no longer aliases `creature.is_evading_home()` to
  target-not-alive/out-of-range. In-range spells now cast and send
  `SMSG_SPELL_GO` with `SPELL_MISS_EVADE`; far evading targets still fail the
  normal range check.
- The test-only melee validator now checks melee reach before navigation, matching
  the map-owned validator and CMaNGOS' reach-before-evade feedback shape.
- Native mmap path lookup now uses the CMaNGOS near-then-far polygon search
  bounds (`5y` then `10y`) instead of returning no path after only the near
  lookup. This is aimed at mobs stopping near barriers when the desired chase
  point is slightly off the navmesh.
- Map-owned chase destination selection now retries nearby target-ring angles
  when the primary melee slot has LOS trouble, matching CMaNGOS
  `GetNearPointAt`. This is aimed at mobs stopping when kited around trees.
- Session and map-owned DB-creature melee reach now ignore LOS/path guardrails,
  matching CMaNGOS `CanReachWithMeleeAttack`. Navigation still gates generated
  aggro and chase movement; it no longer redefines whether a close target is in
  melee reach.
- Native mmap direct/random path calls now return CMaNGOS path status flags
  (`NORMAL`, `INCOMPLETE`, `NOPATH`) through the Rust FFI instead of inferring
  status from point count. Chase startup now refuses incomplete paths whose
  final endpoint is still outside target combat reach, matching
  `DispatchSplineToPosition`'s reachable-endpoint guard.
- Audited CMaNGOS `CombatManager` leash refresh against Rust. Rust already had
  map-owned combat leash state, but one policy was inverted: creature melee hits
  while still in chase motion did not refresh the timer, while periodic
  aura/dynamic-object ticks did. Rust now refreshes on direct creature melee,
  direct creature spell damage, and direct player damage to creatures; periodic
  aura and dynamic-object tick damage add threat/damage without refreshing the
  leash timer.
- Audited CMaNGOS creature assistance against Rust. Rust already loaded
  `CallForHelp` and tracked `already_called_assistance`, but only sight aggro
  used it. Player-hit retaliation, direct hostile spell damage, and map-runtime
  channel/dynamic-object combat startup now all call nearby same-faction
  combat-capable helpers once and announce each helper's combat start.
- Added CMaNGOS `CheckForHelp` parity on creature relocation. Creatures now
  track the 2s post-aggro check-for-help delay, active combat creatures can pull
  nearby same-faction helpers after that delay, and idle/patrol creatures moving
  past an existing fight can join when they are within 5y, in LOS, and hostile
  to the victim.
- Audited CMaNGOS vendor buyback against Rust. Rust used the correct slot range
  (`69..80`) and player update fields, but wrapped the buyback cursor after slot
  `80` and assigned the same timestamp to every entry. Rust now keeps the cursor
  parked at the final slot like CMaNGOS, reuses cleared slots when appropriate,
  and assigns monotonic timestamps so full-list replacement follows oldest-slot
  ordering.

## Tests Run

- `cargo fmt`
- `cargo test -p wow-network map_runtime_event_ai_zero_initial_timer_waits_for_cmangos_update_pulse --lib`
- `cargo test -p wow-network map_runtime_event_ai_timer_in_combat_schedules_cast --lib`
- `cargo test -p wow-network map_runtime_event_ai_cast_respects_hard_control --lib`
- `cargo test -p wow-network map_runtime_event_ai_aggro_cast_targets_self --lib`
- `cargo test -p wow-network map_runtime_event_ai_range_and_missing_aura_select_casts --lib`
- `cargo test -p wow-network map_runtime_event_ai_facing_target_matches_cmangos_position_and_repeat_rules --lib`
- `docker exec cmangos-rust-realmd mariadb -uroot -proot -N -B mangos -e "SELECT COUNT(*) FROM creature_spell_list_entry WHERE Id=9400; SELECT COUNT(*) FROM creature_template_spells WHERE entry=94 AND setId=0;"`
- `cargo check -p wow-db`
- `cargo test -p wow-network map_runtime_db_creature_spell_list_schedules_direct_damage_cast --lib`
- `cargo test -p wow-network rewarded_chain_quest_opens_next_quest_details_from_same_questgiver --lib`
- `cargo check -p starter-zone-flow-test`
- `cargo check -p worldserver`
- `.\scripts\test-rust.cmd`
- `cargo test -p wow-network player_death_update_sets_health_flags_and_release_timer --lib`
- `cargo test -p wow-network night_elf_ghost_update_includes_wisp_form_as_negative_aura --lib`
- `cargo test -p wow-network lethal_player_world_damage_clears_rage_immediately --lib`
- `cargo test -p wow-network active_cast_suppresses_melee_without_extending_resume_timer_to_cast_end --lib`
- `cargo test -p wow-network combat_flag_spell_completion_resets_ready_melee_swing_timer --lib`
- `cargo test -p wow-network map_runtime_player_spell_allows_in_range_evading_target_for_spell_miss_resolution --lib`
- `cargo test -p wow-network fireball_against_evading_creature_casts_then_reports_evade_miss --lib`
- `cargo test -p wow-network --lib`
- `cargo test -p wow-network db_creature_mmap_path_uses_cmangos_smooth_steps_when_available --lib`
- `cargo test -p wow-network db_creature_mmap_path_corner_uses_local_detour_data_when_available --lib`
- `cargo test -p wow-network db_creature_mmap_path_uses_kalimdor_teldrassil_data_when_available --lib`
- `cargo test -p wow-network db_creature_chase_near_point_retries_adjacent_angles_when_primary_los_fails --lib`
- `cargo test -p wow-network db_creature_chase_near_point_keeps_primary_when_no_los_candidate_exists --lib`
- `cargo test -p wow-network db_creature_melee_reach_ignores_los_and_path_like_cmangos --lib`
- `cargo test -p wow-network db_creature_chase_motion_advances_position_over_time_before_reach --lib`
- `cargo test -p wow-network db_creature_melee_reach_is_position_gated --lib`
- `cargo test -p wow-network map_runtime_chase_destination_fans_out_same_victim_attackers --lib`
- `cargo test -p wow-network db_creature_navigation_guardrail_blocks_aggro_and_chase_not_reach --lib`
- `cargo test -p wow-network native_mmap_path_status_preserves_cmangos_incomplete_flag --lib`
- `cargo test -p wow-network incomplete_chase_endpoint_must_reach_target_like_cmangos --lib`
- `cargo test -p wow-network db_creature_random_mmap_path_uses_local_detour_data_when_available --lib`
- `cargo test -p wow-network map_runtime_db_creature_direct_melee_refreshes_leash_timer_while_chasing --lib`
- `cargo test -p wow-network map_runtime_db_creature_periodic_aura_damage_does_not_refresh_leash_timer --lib`
- `cargo test -p wow-network player_hit_calls_nearby_db_creature_assistance --lib`
- `cargo test -p wow-network map_runtime_db_creature_combat_packets_call_assistance --lib`
- `cargo test -p wow-network player_hit_announces_db_creature_retaliation_start --lib`
- `cargo test -p wow-network map_runtime_db_creature_assistance_call_is_shared_once --lib`
- `cargo test -p wow-network db_creature_assistance_calls_nearby_same_faction_hostiles_once --lib`
- `cargo test -p wow-network blizzard_creates_channel_dynamic_object_and_ticks_area_damage --lib`
- `cargo test -p wow-network arcane_missiles_starts_unit_channel_and_ticks_triggered_damage --lib`
- `cargo test -p wow-network map_runtime_active_combat_creature_pulls_help_after_aggro_delay --lib`
- `cargo test -p wow-network map_runtime_idle_patrol_walking_by_active_fight_joins_combat --lib`
- `cargo test -p wow-network buyback_fills_all_twelve_slots_including_last --lib`
- `cargo test -p wow-network buyback_reuses_last_slot_after_it_is_cleared --lib`
- `cargo test -p wow-network buyback_full_list_replaces_oldest_slot_in_order --lib`
- `cargo test -p wow-network buyback_slot_update_writes_guid_price_and_timestamp_fields --lib`
- `cargo test -p wow-network buyback --lib`
- `git diff --check`
- `.\scripts\test-rust.cmd` attempted after the creature AI scheduler change.
  The gate reached the DB-backed tests and failed on local MySQL auth:
  `database error: 1698 (28000): Access denied for user 'root'@'localhost'`.
  The failing tests were
  `world::tests::map_runtime_direct_completion_after_manager_started_3196_immolate_does_not_hang`
  and
  `world::tests::map_runtime_manager_advances_3196_event_ai_immolate_with_delayed_completion`.

Focused creature AI tests, formatting, and diff whitespace checks passed. The
full Rust gate is blocked locally by DB credentials rather than a creature AI
assertion failure.

## Known Blockers / Unproven Areas

- Creature AI spell scheduling is source- and unit-test-backed, but still needs
  live smoke on Webwood Silkspinner and another early caster. Expected behavior:
  aggro EventAI casts may still happen quickly when DB scripts say so, but
  timer/range/facing/missing-aura EventAI should not fire before the CMaNGOS
  500ms pulse or retry every 100ms world tick.
- Defias Cutpurse Backstab spam should be live-smoked after rebuild/restart.
  Expected behavior: entry `94` should no longer autocast Backstab from legacy
  `creature_template_spells` unless/until a proper modern CMaNGOS spell-list or
  script owner is wired by DB data.
- The standard `.\scripts\test-rust.cmd` gate is currently blocked in this
  environment by local MySQL `root@localhost` access for two DB-backed map
  runtime tests. Fix credentials or run against the expected local DB setup
  before treating the full gate as green.
- No remaining blocker is known for the quest reward chain-advance bug; the
  focused test gate passed and the user confirmed the real client advances
  from `A Threat Within` into `Kobold Camp Cleanup`.
- Night Elf Wisp Form is now represented in negative aura slots using
  CMaNGOS/DBC spell id `20584`, but live-client visual transform still needs a
  quick smoke test because Rust does not yet build player transform display
  updates from active auras.
- The food/drink stand/sit fix is covered by behavior-level code review and the
  full `wow-network` unit suite, but not yet by a focused `handle_use_item`
  integration test because that path currently depends on DB-backed item
  templates.
- The earlier target portrait aura timer issue remains unresolved and should be
  packet-capture compared against CMaNGOS before further Rust patches. CMaNGOS
  only sends `SMSG_UPDATE_AURA_DURATION` directly to player aura targets, so do
  not add caster-directed creature debuff duration packets without a capture.
- The near-barrier/tree pathing fixes are source- and test-backed, but still
  need live smoke at known tree/barrier repros. The latest close-to-object stall
  report led to the CMaNGOS reach-ownership correction and native incomplete
  path-status preservation above. If mobs still stop at the same coordinate,
  capture the creature/player coordinates and compare whether CMaNGOS generates
  a complete alternate path there or also marks it unreachable. Do not blindly
  tune chase slot offsets.
- Enemy cry-for-help assistance is unit-tested for initial pull, player-hit
  retaliation, map-runtime packet combat startup, active-combat drag-through,
  and idle/patrol walk-by assistance. It still needs a live client smoke with a
  clustered same-faction hostile pull, a drag-through-another-mob pull, and a
  patrol walking by an existing fight.
- Vendor buyback cursor/timestamp behavior is unit-tested and full Rust-gated,
  but should still get a quick live smoke: sell 12 distinct stacks/items, confirm
  the final buyback slot fills, buy back the final slot, sell another item, and
  confirm the final slot can be occupied again.

## Recommended Next Task

Rebuild/restart and live-smoke creature AI with Webwood Silkspinner and Defias
Cutpurse. For Cutpurse, confirm Backstab is no longer spammed from legacy
`creature_template_spells`. Then live-smoke vendor buyback using the 12-slot
repro above, enemy cry-for-help with clustered/drag-through/patrol cases, and
the tree/barrier mob pathing repro. If those pass, the next useful
playtest-parity slice is the aura refresh/timer issue, but it should start from
a CMaNGOS source/packet compare rather than a UI-only patch.

## Key Files

- `crates/wow-network/src/world/handlers/quest.rs`
- `crates/wow-network/src/world/handlers/vendor.rs`
- `crates/wow-db/src/world_data.rs`
- `crates/wow-network/native/mmap_path.cpp`
- `crates/wow-network/src/world/handlers/mmap_path.rs`
- `crates/wow-network/src/world/handlers/death.rs`
- `crates/wow-network/src/world/combat/aggro.rs`
- `crates/wow-network/src/world/combat/lifecycle.rs`
- `crates/wow-network/src/world/combat/melee.rs`
- `crates/wow-network/src/world/combat/motion.rs`
- `crates/wow-network/src/world/combat/runtime.rs`
- `crates/wow-network/src/world/entities/creature.rs`
- `crates/wow-network/src/world/map_runtime/map_manager/creatures/combat.rs`
- `crates/wow-network/src/world/map_runtime/systems/creature_combat.rs`
- `crates/wow-network/src/world/map_runtime/systems/creature_damage.rs`
- `crates/wow-network/src/world/map_runtime/systems/creature_motion.rs`
- `crates/wow-network/src/world/map_runtime/systems/dynamic_objects.rs`
- `crates/wow-network/src/world/map_runtime/systems/player_channels.rs`
- `crates/wow-network/src/world/spells.rs`
- `crates/wow-network/src/world/spells/casting.rs`
- `crates/wow-network/src/world/map_runtime/systems/damage.rs`
- `crates/wow-network/src/world/map_runtime/systems/players.rs`
- `crates/wow-network/src/world/tests/quests_reputation.rs`
- `crates/wow-network/src/world/tests/character_inventory_social.rs`
- `crates/wow-network/src/world/tests/death_aggro.rs`
- `crates/wow-network/src/world/tests/map_runtime_grids_playerbots.rs`
- `crates/wow-network/src/world/tests/navigation_motion.rs`
- `crates/wow-network/src/world/tests/player_runtime_auras.rs`
- `crates/wow-network/src/world/tests/spells.rs`
- `bins/starter-zone-flow-test/src/main.rs`
- `docs/playable_gate_board.md`
- `docs/playable_execution_roadmap.md`
