# Session Handoff

Short operating brief for the next Rust migration session. Durable roadmap
history belongs in `docs/rust_migration_plan.md`, gate status in
`docs/playable_gate_board.md`, and branch split details in
`docs/playable_execution_roadmap.md`.

## Current Branch And State

- Branch: `codex/rusty-mangos`, in the main checkout at
  `C:\Users\subhe\Documents\New project`.
- Latest landing on this branch combines the spell-effect-value/conjure scaling
  slice, spell/auto-attack timing follow-up, combat-state/regen fixes, level-up
  stat propagation fix, and the random-wander Detour pathfinder merge from the
  `codex/random-wander-pathfinder` worktree. Touched files include spell
  metadata/effects, DBC world-data loading, skill-cap sync, map-owned
  auto-attack/combat-state ownership, level-up stat sync, native mmap pathing,
  creature random motion, and focused `wow-network` tests.
- Current user-directed priority: Checkpoint 2 real-client playtest triage for
  Northshire. The first implementation slice fixed the Garrick-style quest
  reward inventory transaction, chest gameobject use opening loot, and combat
  logout denial. The next slice added the CMaNGOS 20-second non-rest logout
  timer and cancel path. A later slice moved Opening/gameobject interaction
  work out of blocking session sleeps and added DB-backed quest-objective GO
  gating. The corpse/respawn slice caps corpse decay to 90% of DB respawn delay
  and advances map-loaded creature lifecycle timers instead of only
  player-visible GUIDs. Another pass added CMaNGOS-style
  disconnected-in-combat body linger. The latest stacked pass addresses the
  user's failed smoke checks: quest chests now use the quest-objective GO gate,
  gameobject chest loot keeps normal non-quest drops and supports reference
  loot rows, timed logout applies sit/stun-style movement lock and clears it on
  cancel, item spell cooldowns carry source item ids into
  `SMSG_INITIAL_SPELLS`, and attacks against returning-home evaders now resolve
  as evade feedback instead of dead-target stop. The newest local pass fixes
  relogged buff icons by preserving visible aura fields in the login create
  block and sending aura durations after login, repairs item-instance spell
  charges from item templates so charge-based tooltips can render correctly,
  adds CMaNGOS-style 5 second respawn sight-aggro delay, and sends queued
  Heroic Strike rage updates before spell-go/combat logs. The newest discovery
  slice loads `AreaTable.dbc`, reads `.map` area flags through the native map
  bridge, stores map-owned explored-zone bits, persists `characters.exploredZones`,
  and sends exploration XP/update packets on movement using CMaNGOS
  `exploration_basexp` values. The latest follow-up adds the CMaNGOS WMO
  indoor override path: native VMAP `getAreaInfo` is checked before `.map`
  fallback, `WMOAreaTable.dbc` maps `(rootId, adtId, groupId)` to `AreaTable`,
  and terrain-between-player-and-WMO-floor matches the CMaNGOS guard.
- Playerbots remain disabled by default for normal multiplayer/Northshire
  testing: `config/worldserver.local.toml` has `[playerbots] enabled = false`
  and `[playerbots.random] enabled = false`.

## Current Goal And Recommended Next Task

- Goal: make the Northshire Checkpoint 2 playtest loop stable enough for the
  user to grade in the real client without disconnects, broken quest-object
  interactions, corpse/respawn stalls, or persistence surprises. The user
  clarified that the occasional stuck quest-completion screen is a visual
  desync because the underlying quest turn-in succeeds, so treat it below P0.
- Recommended next task: restart the worldserver and real-client smoke the
  latest failed checks again, especially relogged buff icons, Hearthstone
  cooldown icon state, food/water charge tooltip text, respawn aggro grace, and
  queued Heroic Strike/rage display. Non-rest logout movement lock, Battered
  Chest loot, and Milly-style quest-only chest gating should remain in the
  regression pass. The current GO follow-up moved closer to CMaNGOS by evaluating
  dynamic flags per player at visibility-create/quest-refresh time and by
  refreshing visible GO flags after quest abandon. The Battered Chest follow-up
  identified the remaining lock mismatch: the client casts DB-backed
  `SPELL_EFFECT_OPEN_LOCK` spell 3365 for lock id 57, while Rust previously
  special-cased only spell 6478. The Opening flow now accepts gameobject-target
  open-lock spells from `spell_template` and echoes the actual spell id through
  start/go packets. The chest multi-loot follow-up fixed another GO bug where
  successful autostore immediately consumed/destroyed the chest after the first
  item; GO chests now consume only once the shared chest loot is empty. The
  latest follow-up adds `CMSG_READ_ITEM` / `CMSG_PAGE_TEXT_QUERY`, generic
  DB-backed NPC text, merged trainer/vendor plus quest gossip, movement cancel
  for `/dance`/state emotes, and visible positive buff save/load through
  CMaNGOS `character_aura`. Exploration discovery/XP now has an outdoor
  CMaNGOS-shaped data path from WMO/VMAP area info, `.map` area flags,
  `WMOAreaTable.dbc`, and `AreaTable.dbc`. Secondary-bag deletion, Garrick turn-in, GO
  cancel/interruption, GO combat ticking, corpse/respawn, right-click
  out-of-range combat state, post-kill regen/rage decay, and evade feedback
  passed the user's current real-client smoke.

## Recent Implemented Work

- Extended world spell metadata loading from `spell_template` with dispel,
  mechanic, stack amount, per-effect mechanic, implicit target B, radius index,
  and item type fields. Added `spell_chain` DB lookup/cache in `ObjectMgr`.
- Added `SpellRadius.dbc` loading into `WorldDataFiles`/`MapRuntimeManager` so
  AoE spell radius comes from DBC data rather than constants.
- Extended `SpellInfoEffect` and spell profile derivation for
  `SPELL_EFFECT_CREATE_ITEM`, effect item type, secondary implicit targets, and
  caster-centered hostile AoE targets (`15` / `36`).
- Implemented generic create-item spell handling. Item id comes from
  `EffectItemTypeN`; count comes from the CMaNGOS effect roll value with a
  minimum of 1 and item stack-size cap. Cast preflight checks item-template
  existence and storage space, then the effect uses the existing inventory store
  plan, stack merge/add paths, persistence helpers, item push result, and update
  packets.
- Implemented `SPELL_AURA_MOD_ROOT` as a generic aura modifier. Players now send
  force-root/unroot packets when root state changes or expires. Creatures stop
  active motion when newly rooted, and chase/random/waypoint/return-home motion
  will not start while root aura state is active.
- Added caster-centered hostile AoE aura application for player spells. The
  effect resolves DBC radius metadata, finds nearby hostile DB creatures from
  map-owned spatial/faction state, applies the aura to each target, and starts
  retaliation.
- Added rank-aware aura conflict checks backed by `spell_chain`. Same spell from
  the same caster refreshes; higher rank in the same chain replaces lower-rank
  auras; lower/equal different-rank recasts bounce with
  `SPELL_FAILED_AURA_BOUNCED`; replacement paths avoid duplicate stat
  application in session and map-owned aura state.
- Added `spell_group` / `spell_group_spell` DB lookup and ObjectMgr caching.
  Aura conflict preflight now honors CMaNGOS group rules: `UNIQUE` replaces
  matching aura groups regardless of caster, while `UNIQUE_PER_CASTER` only
  replaces the caster's own matching group. Rank checks also bounce stronger
  positive auras from other casters and replace weaker positive ranks across
  casters.
- Broadened generic implicit target handling for direct friendly unit aura
  targets (`TARGET_UNIT_FRIEND`, party/raid unit variants, chain-heal target).
  Friendly player-target aura casts now update map-owned target aura state and
  dispatch direct/observer aura packets instead of silently doing nothing or
  falling back to self.
- Kept unsupported player spell effects visible with warning logs so new spell
  families are easier to triage.
- Fixed a parity wrinkle found during tests: caster-centered hostile root auras
  are classified as debuffs, not positive self buffs.
- Fixed a critical player power timing issue: spell mana/rage/energy is still
  spent from map-owned state at cast completion, but the client-visible power
  `SMSG_UPDATE_OBJECT` now goes out immediately before cast result/`SMSG_SPELL_GO`
  instead of waiting for delayed projectile impact. This matches the CMaNGOS
  `Spell::cast` ordering where `TakePower()` happens before `SendSpellGo()`.
- Strengthened the map-owned regen/session-cache regression so food/drink-style
  mana ticks, health regen, and rage decay survive refresh/sync without stale
  session state pushing bars backwards.
- Added an uncommitted follow-up for normal spell casts and auto-attack timers:
  spells with CMaNGOS `SPELL_INTERRUPT_FLAG_COMBAT` now reset active melee
  swing timers on cast release, delay active Auto Shot by at least the normal
  500 ms post-cast windup without shortening a longer ranged weapon cooldown,
  and cancel wand-style auto-repeat when the active ranged spell has
  `SPELL_ATTR_EX3_CASTING_CANCELS_AUTOREPEAT`.
- Added a combat-state parity fix for issue #72. Map-owned `PlayerRuntime` now
  has an explicit `in_combat` bit driven by creature combat ownership instead
  of `active_combat_target`; right-click/`CMSG_ATTACKSWING` remains auto-attack
  intent only. Player update/create flags, looting flag updates, regen/rage
  decisions, and DB script `UNIT_CONDITION_IN_COMBAT` now use the explicit
  combat bit. Creature combat begin, target switch, clear, death, evade, and
  victim cleanup paths refresh the bit from active creature combat refs.
- Fixed the follow-up regen regression from the first combat-state slice: the
  lethal DB-creature damage path was directly removing combat/threat/leash
  state and bypassing the combat-owner clear helper, leaving `PlayerRuntime` in
  combat after a kill. Creature death now uses `clear_db_creature_combat`, and
  player-death cleanup explicitly clears the player's map-owned combat bit.
  Session-side active creature attack refresh also clears the client combat flag
  even if auto-attack intent remains queued.
- Fixed the user-observed right-click regen/rage persistence bug. The remaining
  issue was not another combat flag source: ordinary session sync still pushed
  cached health/mana/rage/energy back into `MapRuntime` after client packets.
  If a map-owned regen tick happened between session-cache refresh and the final
  sync, the stale session values could undo the tick while auto-attack intent
  was active. `sync_player_gameplay_state` now treats alive player health and
  powers as map-owned state and only applies session health during non-alive
  death/ghost transitions.
- Added the spell-effect-value/conjure scaling slice. `spell_template` loading
  now includes CMaNGOS level-scaling fields (`MaxLevel`, `BaseLevel`,
  `SpellLevel`, `ManaCostPerlevel`, `EffectDicePerLevel*`,
  `EffectRealPointsPerLevel*`). `WorldDataFiles`/`MapRuntimeManager` now load
  minimal `SkillLineAbility.dbc`, `SkillLine.dbc`, and
  `SkillRaceClassInfo.dbc` data for spell rank and level-backed skill sync.
- Added a CMaNGOS-shaped player spell rank context from
  `SkillLineAbility` spell-to-skill mapping plus current character skill value,
  capped by `MaxLevel * 5`. Missing DBC remains degraded/unranked rather than
  inventing constants.
- Replaced the simple effect amount paths with a generic signed effect-value
  calculator covering base points, base dice, die sides,
  `EffectRealPointsPerLevel`, `EffectDicePerLevel`, base/max/spell level
  clamps, combo-point additions, and scaled mana cost. It is now used for
  create-item counts/preflight, direct damage/heal/energize, periodic
  damage/regen, aura stat amounts, item-use auras, and queued on-next-swing
  bonus damage.
- Player level-backed skill rows are now synced on login, real level-up, and
  GM level changes using DBC-backed skill category/race/class info when
  available. Maximized class skills become `level * 5`, weapon skill maximums
  rise without forcing current value, and mono armor/languages stay unchanged.
- Fixed level-up stat propagation into map-owned runtime state. Direct XP awards
  now call `MapRuntimeManager::update_player_level_progression_state` with the
  new DB-backed `PlayerWorldStats` and recomputed equipped-item combat stats,
  so map-owned regen, max health, mana, combat stats, and aura-derived effective
  stats move to the new level immediately. Party/member reward updates now
  carry optional world/combat stats for level-up cases instead of only raw
  health/power values, fixing the path where regen could still cap at the
  previous level's max HP.
- Merged the random-wander pathfinder work from the
  `codex/random-wander-pathfinder` worktree. The native mmap bridge now exposes
  `wow_mmap_find_random_path`, samples a deterministic point within the DB
  wander radius, loads start/target neighbor tiles, finds nearest Detour polys,
  smooths the path, and returns grounded world points. Rust random creature
  motion now uses that native random path when real mmap data is advertised,
  while the old straight-path random fixture behavior remains limited to
  unit-test fixture navigation. Advertised-but-unloadable mmap data no longer
  falls back to fake through-geometry random movement.
- Added CMaNGOS-shaped server time for day/night on login. The
  `SMSG_LOGIN_SETTIMESPEED` bootstrap packet now sends current local server time
  packed with the same `secsToTimeBitFields` layout CMaNGOS uses, plus the
  Classic game speed `0.01666667`, instead of the old zero placeholder.
- Fixed the first Checkpoint 2 playtest triage slice. Quest reward turn-in now
  preflights required-item consumption plus all reward grants through the
  bag-aware store planner, so Garrick-style full-backpack/open-equipped-bag
  cases can grant into container slots without consuming the quest item first.
  Reward item creation now sends the correct player/container slot updates.
- Gameobject chest use now opens shared DB gameobject loot through the existing
  loot authority instead of doing nothing when the GO has a chest loot id.
- Logout requests while the session is in combat now return the CMaNGOS failure
  response (`failure_reason = 1`, non-instant) instead of immediately completing
  logout.
- Logout requests outside combat now follow the CMaNGOS request flow more
  closely: no active character or resting players complete instantly, normal
  non-rest logout sends a non-instant success response and completes after 20
  seconds, `CMSG_LOGOUT_CANCEL` clears the pending timer and returns
  `SMSG_LOGOUT_CANCEL_ACK`, and jumping/falling-far logout is denied like
  combat.
- Opening/gameobject interactions no longer sleep inside the session handler.
  The special Opening spell now starts immediately, records a map-owned active
  player cast, and completes through the normal pending-cast wakeup path after
  the Classic 5 second Opening cast time. Movement cancel uses the existing
  active-cast cancellation path; damaging hits cancel pending Opening casts; and
  player removal/log out clears active/pending spell work from the map owner.
- Quest-objective gameobjects now use a DB-backed ObjectMgr index over
  negative `ReqCreatureOrGOId*` quest objectives. Generic/spell-focus/goober
  objective GOs are denied unless the player has an active incomplete objective
  for that exact GO entry and has not already reached the required count.
- Creature corpse/respawn lifecycle now follows the CMaNGOS timing shape more
  closely. `DbCreatureRuntime::begin_corpse` caps corpse decay to 90% of the
  chosen DB respawn delay, matching `Creature::Create`, so starter mobs with
  short DB respawns do not wait for the generic 5-minute corpse default before
  they can respawn.
- Creature lifecycle advancement now scans map-loaded creature runtimes with
  due corpse/respawn timers instead of only the active player's visible
  creature GUID set. Corpse removal updates map-owned per-player visibility
  state before sending destroys, and respawn creation re-adds nearby eligible
  players with per-player create blocks.
- Combat disconnects now leave the player runtime in the map as a disconnected
  body for CMaNGOS's 60 second offline disconnect timeout instead of removing
  the body immediately. The lingering body is no longer client-controlled, keeps
  active creature combat references, can still take map-owned damage, expires
  through the map lifecycle, dispatches destroy packets to observers, and
  persists final position/vitals or death state when the linger expires.
- Character spell cooldowns now load from and save to CMaNGOS
  `character_spell_cooldown`. Future spell cooldowns are restored into
  session/map-owned runtime state on login, included in the initial spells
  cooldown list sent to the client, saved on logout, and saved when a
  disconnected-in-combat lingering body expires. This covers the Hearthstone
  relog-reset symptom without item-specific special casing. Standalone category
  cooldown persistence is still deferred until runtime cooldown ownership
  tracks spell, category, and item as one record.
- Deleting an item from an equipped secondary bag now sends the container-slot
  `SMSG_UPDATE_OBJECT` clear before destroying the item object, so the client
  should not keep a gray ghost icon until relog or another inventory update.
- Latest failed-smoke fix pass: quest chest gameobjects now participate in the
  DB-backed quest-objective gate, so abandoning the related quest should deny
  use immediately rather than allowing a progress bar with no loot. Gameobject
  chest loot selection no longer strips non-quest drops, and DB loading now
  supports negative `mincountOrRef` reference rows for normal chest loot such
  as Battered Chest.
- The 20-second non-rest logout path now applies the CMaNGOS-shaped
  sit/stunned presentation, ignores movement packets while the logout timer is
  pending, and clears the sit/stun state on logout cancel.
- Item-use spell cooldown runtime state now tracks the source item id alongside
  the spell cooldown. Login `SMSG_INITIAL_SPELLS` now includes that item id, so
  Hearthstone-style item cooldowns have the metadata the client needs to draw
  the item cooldown overlay after relog.
- Attacking a returning-home evading creature now remains an attack attempt and
  sends melee evade feedback (`SPELL_MISS_EVADE` / `VICTIMSTATE_EVADES`)
  instead of treating the target as dead and stopping auto-attack intent.

## Tests Run

- AreaTable/exploration discovery slice:
  `cargo test -p wow-network area_ -- --nocapture` passed, including the new
  `area_table_dbc_parser_indexes_explore_flags_by_map` and
  `map_owned_area_discovery_sets_explored_zone_bit_once` tests.
  `cargo check -p wow-network --tests` passed.
  `cargo clippy --workspace --all-targets -- -D warnings` passed.
  `cargo test --workspace -- --nocapture` passed, including 691 `wow-network`
  tests and 23 `wow-proto` tests.
  `.\scripts\test-rust.cmd` passed fmt, clippy, and all workspace tests, then
  failed only at final `cargo build -p authserver` because running
  `authserver.exe` / `worldserver.exe` processes held the target binaries open
  on Windows.
- WMO indoor area override follow-up:
  `cargo test -p wow-network area_ -- --nocapture` passed, including
  `wmo_area_table_dbc_parser_maps_vmap_triple_to_area_table_entry`.
  `cargo check -p wow-network --tests` passed.
  `cargo clippy --workspace --all-targets -- -D warnings` passed.
- Latest failed-smoke fix pass:
  `cargo fmt --check` passed.
  `cargo check --tests -p wow-network` passed.
  `cargo test -p wow-network --lib initial_spells_include_active_spell_cooldowns -- --nocapture`
  passed.
  `cargo test -p wow-network --lib quest_objective_gameobject_requires_active_incomplete_objective -- --nocapture`
  passed.
  `cargo test -p wow-network --lib db_creature_player_melee_check_allows_evade_feedback_for_returning_creature -- --nocapture`
  passed.
  `cargo test -p wow-network --lib logout -- --nocapture` passed with 5
  focused logout tests.
  `git diff --check` passed with only CRLF normalization warnings. Full
  `.\scripts\test-rust.cmd` was not run in this pass because the user is doing
  the next real-client validation.
- Corpse/respawn follow-up:
  `cargo check -p wow-network` passed. No test suite was run for this pass
  because the user explicitly asked to skip testing and do the real-client
  testing later.
- Disconnected body linger follow-up:
  `cargo check -p wow-network` passed. Full test suite not run in this pass;
  focused map-runtime coverage was added for combat disconnect linger and
  expiry.
- Cooldown/secondary-bag persistence follow-up:
  `cargo test -p wow-network initial_spells_include_active_spell_cooldowns`
  passed.
  `cargo test -p wow-network equipped_bag_destroy_update_clears_container_slot`
  passed.
  `cargo check -p wow-network` passed.
  `git diff --check` passed with only CRLF normalization warnings. Full
  `.\scripts\test-rust.cmd` was not run because the user asked to keep moving
  and handle real-client testing later.
- Baseline before spell changes:
  `$env:CARGO_TARGET_DIR='target\codex-spells-baseline'; .\scripts\test-rust.cmd`
  passed fully.
- During implementation:
  `$env:CARGO_TARGET_DIR='target\codex-spells-dev'; cargo test -p wow-network --lib`
  initially exposed a DB access bug in aura-rank conflict preflight when no
  same-caster different-rank aura was active; after the early-return fix it
  passed with 634 tests, and later with 640 tests after focused spell tests were
  added.
- Added focused tests for SpellRadius DBC parsing, create-item metadata/stack
  cap, full-backpack storage planning, Frost-Nova-style caster-centered hostile
  root targeting/radius/debuff classification, root movement/root packet
  expiration, and ranked aura replacement/bounce/stat behavior.
- Final verification:
  `$env:CARGO_TARGET_DIR='target\codex-spells-final'; .\scripts\test-rust.cmd`
  passed fully after clippy cleanup, including fmt, clippy, workspace unit/doc
  tests, `wow-network` 640 tests, `wow-proto` 23 tests, and authserver/worldserver
  builds in the isolated target dir.
- Regen/power timing investigation:
  baseline `$env:CARGO_TARGET_DIR='target\codex-regen-baseline'; .\scripts\test-rust.cmd`
  initially failed on a pre-existing `cargo fmt --check` mismatch in
  `crates/wow-network/src/world/spells.rs`; `cargo fmt` fixed it. Focused
  tests passed for
  `session_cache_refresh_preserves_map_owned_regen_before_session_sync` and
  `cast_time_spell_sends_start_before_delayed_go_and_effects`. Final
  verification `$env:CARGO_TARGET_DIR='target\codex-regen-final'; .\scripts\test-rust.cmd`
  passed fully, including `wow-network` 643 tests.
- Baseline for the follow-up stacker/target slice:
  `$env:CARGO_TARGET_DIR='target\codex-spellstack-baseline'; cargo test -p wow-network --lib`
  passed with 640 tests.
- Focused follow-up checks:
  `$env:CARGO_TARGET_DIR='target\codex-spellstack-dev'; cargo test -p wow-network --lib conflict`
  passed with the rank/group conflict tests, and
  `$env:CARGO_TARGET_DIR='target\codex-spellstack-dev'; cargo test -p wow-network --lib direct_friendly_unit`
  passed. Full crate rerun:
  `$env:CARGO_TARGET_DIR='target\codex-spellstack-dev'; cargo test -p wow-network --lib`
  passed with 643 tests.
- Final verification after the stacker/target follow-up:
  `$env:CARGO_TARGET_DIR='target\codex-spellstack-final'; .\scripts\test-rust.cmd`
  passed fully, including fmt, clippy, workspace unit/doc tests,
  `wow-network` 643 tests, `wow-proto` 23 tests, and authserver/worldserver
  builds in the isolated target dir.
- Spell/auto-attack timing follow-up:
  `$env:CARGO_TARGET_DIR='target\codex-attackspell-timing'; cargo test -p wow-network --lib combat_flag_spell_completion -- --nocapture`
  passed with 3 focused tests.
  `$env:CARGO_TARGET_DIR='target\codex-attackspell-timing'; cargo test -p wow-network --lib combat_interrupt_flag_drives_spell_cast_attack_timer_reset -- --nocapture`
  passed.
  `$env:CARGO_TARGET_DIR='target\codex-attackspell-timing'; cargo test -p wow-network --lib repeated_auto_attack_input_preserves_swing_timer_and_uses_normal_due_tick -- --nocapture`
  passed.
  `$env:CARGO_TARGET_DIR='target\codex-attackspell-timing'; cargo test -p wow-network --lib ranged_auto_attack_uses_cmangos_windup_before_weapon_timer -- --nocapture`
  passed.
  `$env:CARGO_TARGET_DIR='target\codex-attackspell-timing'; cargo test -p wow-network --lib`
  passed with 647 tests.
  Final verification:
  `$env:CARGO_TARGET_DIR='target\codex-attackspell-final'; .\scripts\test-rust.cmd`
  passed fully, including fmt, clippy, workspace unit/doc tests,
  `wow-network` 647 tests, `wow-proto` 23 tests, and authserver/worldserver
  builds in the isolated target dir.
- Combat-state parity fix:
  `$env:CARGO_TARGET_DIR='target\codex-combat-state'; cargo test -p wow-network --lib in_combat -- --nocapture`
  passed with the focused combat-flag tests.
  `$env:CARGO_TARGET_DIR='target\codex-combat-state'; cargo test -p wow-network --lib far_attack_swing_starts_intent_without_in_combat_flag -- --nocapture`
  passed.
  `$env:CARGO_TARGET_DIR='target\codex-combat-state'; cargo test -p wow-network --lib`
  passed with 659 tests.
  `$env:CARGO_TARGET_DIR='target\codex-combat-state'; .\scripts\test-rust.cmd`
  progressed through workspace tests, doc tests, `wow-network` 659 tests, and
  `wow-proto` 23 tests, then failed locally while building final binaries
  because `target` ran out of disk space writing `sqlx-mysql`/PDB artifacts
  (`os error 112` / `LNK1201`). Earlier fresh-target attempt also failed from
  no disk space while compiling native bridge objects. This is an environment
  capacity blocker, not a known test assertion failure.
- Creature-death regen regression after combat-state fix:
  `cargo fmt --check` passed.
  `cargo test -p wow-network --lib map_runtime_db_creature_death_clears_player_combat_for_regen -- --nocapture`
  passed.
  `cargo test -p wow-network --lib regen -- --nocapture` passed with 13 tests,
  including the creature-death combat-clear regression.
  `cargo test -p wow-network --lib in_combat -- --nocapture` passed with 4
  focused combat-state tests.
  `cargo test -p wow-network --lib` passed with 660 tests.
- Right-click/attack-intent regen persistence follow-up:
  `cargo fmt --check` passed.
  `cargo test -p wow-network --lib stale_session_sync_does_not_undo_attack_intent_regen -- --nocapture`
  passed.
  `cargo test -p wow-network --lib map_runtime_player_gameplay_sync_owns_session_mutable_state -- --nocapture`
  passed.
  `cargo test -p wow-network --lib session_cache_refresh_preserves_map_owned_regen_before_session_sync -- --nocapture`
  passed.
  `cargo test -p wow-network --lib regen -- --nocapture` passed with 14 tests.
  `cargo test -p wow-network --lib in_combat -- --nocapture` passed with 4 tests.
  `cargo test -p wow-network --lib` passed with 661 tests.
- Level-up stat propagation follow-up:
  `cargo fmt --check` passed.
  `cargo test -p wow-network --lib player_reward_level_up_refreshes_world_stats_for_regen_cap -- --nocapture`
  passed.
  `cargo test -p wow-network --lib level -- --nocapture` passed with 16 tests.
  `cargo test -p wow-network --lib regen -- --nocapture` passed with 15 tests.
  `cargo test -p wow-network --lib` passed with 662 tests.
  `.\scripts\test-rust.cmd` progressed through workspace tests, doc tests,
  `wow-network` 662 tests, `wow-proto` 23 tests, and binary test builds, then
  failed at the final `cargo build -p authserver` because Windows could not
  replace `target\debug\authserver.exe` (`Access is denied`, os error 5). The
  running processes were `authserver` PID 38896 and `worldserver` PID 23460 from
  `target\debug`, so this is an executable lock, not a known test failure.
- Random-wander pathfinder merge:
  `cargo fmt --check` passed after formatting.
  `cargo test -p wow-network --lib db_creature_random_motion -- --nocapture`
  passed with 5 focused tests.
  `cargo test -p wow-network --lib db_creature_mmap_path -- --nocapture`
  passed with 4 focused tests.
  `cargo test -p wow-network --lib db_creature_random_path -- --nocapture`
  passed.
  `cargo test -p wow-network --lib random_mmap_path -- --nocapture` passed.
  `cargo test -p wow-network --lib` passed with 665 tests.
  `$env:CARGO_TARGET_DIR='target\codex-merge-final'; .\scripts\test-rust.cmd`
  passed fully, including workspace unit/doc tests, `wow-network` 665 tests,
  `wow-proto` 23 tests, and final authserver/auth-flow builds.
- Server time/day-night login slice:
  `cargo fmt --check` passed.
  `cargo test -p wow-network --lib login_set_time_speed -- --nocapture`
  passed with 3 focused packet/time-layout tests.
  `cargo test -p wow-network --lib` passed with 668 tests.
  `$env:CARGO_TARGET_DIR='target\codex-server-time-final'; .\scripts\test-rust.cmd`
  passed fully, including fmt, clippy, workspace unit/doc tests, `wow-network`
  668 tests, `wow-proto` 23 tests, and final authserver/auth-flow builds.
- Checkpoint 2 playtest triage first slice:
  `$env:CARGO_TARGET_DIR='target\codex-cp2-quest-dev'; cargo test -p wow-network --lib quest_reward_storage -- --nocapture`
  passed with 3 quest reward storage planning tests.
  `$env:CARGO_TARGET_DIR='target\codex-cp2-quest-dev'; cargo test -p wow-network --lib logout_response_uses_cmangos_combat_failure_shape -- --nocapture`
  passed.
  `$env:CARGO_TARGET_DIR='target\codex-cp2-quest-dev'; cargo test -p wow-network --lib login_set_time_speed -- --nocapture`
  passed with 3 login time tests after the baseline commit.
- Checkpoint 2 logout timer slice:
  `cargo fmt --check` passed.
  `$env:CARGO_TARGET_DIR='target\codex-logout-p1'; cargo test -p wow-network --lib logout -- --nocapture`
  passed with 5 focused logout tests.
  `$env:CARGO_TARGET_DIR='target\codex-logout-p1-final'; .\scripts\test-rust.cmd`
  passed fully, including workspace unit/doc tests, `wow-network` 676 tests,
  `wow-proto` 23 tests, and authserver/auth-flow builds.
- Checkpoint 2 gameobject authority slice:
  `cargo fmt --check` passed.
  `$env:CARGO_TARGET_DIR='target\codex-go-p1'; cargo test -p wow-network --lib gameobject -- --nocapture`
  passed with 16 focused gameobject tests.
  `$env:CARGO_TARGET_DIR='target\codex-go-p1'; cargo test -p wow-network --lib opening_spell -- --nocapture`
  passed with 3 focused Opening tests.
  `$env:CARGO_TARGET_DIR='target\codex-go-p1-final'; .\scripts\test-rust.cmd`
  passed fully, including workspace unit/doc tests, `wow-network` 679 tests,
  `wow-proto` 23 tests, and authserver/auth-flow builds.
- Spell-effect-value/conjure scaling slice:
  baseline `$env:CARGO_TARGET_DIR='target\codex-effectvalue-baseline'; .\scripts\test-rust.cmd`
  passed fully before changes, including `wow-network` 647 tests.
  Focused checks passed for `spell_effect`, `skill_line`,
  `skill_race_class_info`, `player_spell_rank_context`, and
  `level_backed_skill_sync`. Full crate check
  `$env:CARGO_TARGET_DIR='target\codex-effectvalue-dev'; cargo test -p wow-network --lib`
  passed with 656 tests. Final verification
  `$env:CARGO_TARGET_DIR='target\codex-effectvalue-final'; .\scripts\test-rust.cmd`
  passed fully, including fmt, clippy, workspace unit/doc tests,
  `wow-network` 656 tests, `wow-proto` 23 tests, and authserver/worldserver
  builds in the isolated target dir.

## Real-Client Verification Needed

- Latest Checkpoint 2 smoke results from the user:
  Garrick turn-in passed; GO cancel/interruption passed; enemies attack during
  GO interaction passed; corpse/respawn passed; right-click out-of-range combat
  state passed; post-kill HP regen/rage decay passed; item deletion from a
  secondary bag passed. The latest code pass attempted fixes for the failed
  smoke items: quest GO use after abandon, Battered Chest loot, timed logout
  sit/move lock, Hearthstone cooldown UI metadata, and evade attack feedback.
  These all need a new real-client pass. Buff persistence now reloads
  server-side and the latest local fix keeps the visible aura slots/durations in
  the login packet path, so verify the buff icon survives relog. Respawn aggro
  grace now follows the CMaNGOS 5 second `CanAggro` delay locally and needs
  client proof. Quest completion visual, trainer gossip, and generic gossip
  were not tested in the latest pass.
- Conjure Food/Water live cast: DBC/skill-rank scaled quantity, inventory
  creation, stack merge, item push/update packets, bag-full failure,
  missing-template logging if DB data is absent, and resource/cooldown behavior
  around failed preflight.
- Frost Nova live cast: root animation/state, hostile-only AoE selection,
  creature movement stop/resume, expiration unroot, combat retaliation, and no
  friendly/self accidental roots.
- Ranked/grouped buffs live cast: higher/lower rank interactions across casters,
  `spell_group` unique category replacement, visible aura slot replacement,
  bounce failure text, and no doubled character-panel stats.
- Friendly unit buffs live cast: Arcane Intellect or similar direct friendly
  target auras should apply to the selected friendly player, not self or hostile
  creatures.
- Regression smoke: existing damage, heal, DoT, Battle Shout, Heroic Strike,
  Auto Shot, item-use, and aura tick behavior.
- Spell/auto-attack timing smoke: while melee auto-attack is active, casting a
  normal spell such as Fireball should not release an immediate queued white
  swing at cast completion; while Auto Shot is active, normal spell casts should
  delay the next shot by at least the 500 ms windup and preserve longer weapon
  cooldowns; while wand Shoot is active, starting another normal spell should
  cancel the wand auto-repeat rather than resume an immediate wand shot.
- Combat-state smoke: right-clicking an out-of-range hostile should start
  attack intent/retry behavior without the player entering combat; creature
  aggro or landed retaliation should still set `UNIT_FLAG_IN_COMBAT`, and
  death/evade/leash cleanup should clear it only after no combat refs remain.
  After killing a creature, verify HP regen and warrior rage degeneration resume
  while standing out of combat. Also keep moving/right-clicking while out of
  combat and verify map-owned HP regen/rage decay continue instead of snapping
  back to stale session values.
- Level-up smoke: after leveling from quest or creature XP, verify the client
  max HP/stat packet, map-owned max health, and passive HP regen cap all advance
  to the new level; party XP level-ups should behave the same for non-killer
  members.
- Random-wander smoke: with local mmap data enabled, idle DB creatures with
  random movement should choose grounded Detour paths inside spawn radius and
  avoid fake straight-line movement through unavailable mmap geometry. Rooted
  random-movement creatures should remain idle until root expires.
- Day/night smoke: log in at visibly different local server times or adjust the
  host clock in a throwaway test environment and verify the client receives the
  expected day/night lighting from `SMSG_LOGIN_SETTIMESPEED`.
- HP/mana real-client smoke: verify food/drink bars only increase while seated
  and out of interrupting actions, no stale lower-value snapback occurs after
  client input, normal mana regen resumes after the five-second rule, and
  projectile spell mana visibly drops on cast launch rather than on impact.
- Gameobject smoke: Milly buckets/objective goobers should not be usable when
  the player lacks the matching incomplete objective; moving during Opening
  should cancel without delayed loot/credit; taking creature damage during
  Opening should cancel; and nearby hostile creatures should continue attacking
  during the progress bar.

## Current Follow-Ups

- The create-item path now has focused metadata/planner/effect-value tests for
  CMaNGOS level-scaled conjure quantities via `EffectRealPointsPerLevel` and
  spell-level bounds, but still needs real DB/client proof for actual persisted
  inventory creation, packet sequencing, and bag-full preflight behavior.
- The AoE target resolver currently covers the caster-centered hostile target
  families needed by Frost Nova. Direct friendly player targets are now
  classified/applied, but party/raid area targets, chain jumps, cone targets,
  destination-location AoE, and gameobject/unit-location target payloads remain
  future spell-engine work.
- The DB-backed `spell_group` foundation is in, but full Classic SpellStacker
  parity still needs the CMaNGOS per-aura stackability matrix, exclusive dispel
  categories, diminishing-return interaction, and special proc/aura rules.
- Custom spell script hooks are intentionally deferred; the user wants generic
  systems 2 and 3 first and script architecture later.
- The spell/auto-attack timing follow-up is tested in unit coverage but still
  needs real-client visual proof for melee swing delay, Auto Shot resume timing,
  and wand Shoot cancellation/animation state.
- The combat-state issue #72, creature-death regen follow-up, and stale session
  sync fix are covered by focused and full `wow-network` unit tests, but still
  need real-client proof.
- The random-wander Detour merge is covered by focused and full `wow-network`
  unit tests, including the local Northshire mmap path test when data is
  present, but still needs real-client observation of idle creatures wandering
  naturally under server tick load.
- The logout timer now follows the CMaNGOS 20-second non-rest path in unit
  coverage, and combat disconnect now leaves a 60-second map-owned body, but
  real-client logout cancellation/interruption and combat-disconnect linger
  still need live proof.
- The Opening/gameobject authority slice is unit-tested for pending-cast shape,
  cleanup, and quest-objective eligibility, but it still needs real-client
  proof against Milly buckets, chest/opening progress bars, and combat
  interruption timing.
- The corpse/respawn slice was compile-checked only at the user's request to
  skip testing this pass. It needs a later `.\scripts\test-rust.cmd` run plus
  real-client proof that unlooted corpses decay and respawn on the expected DB
  timing.
- Buff persistence now has a first CMaNGOS-shaped `character_aura` path:
  visible positive player auras are rebuilt on login, included in the player's
  self create block, sent with duration packets after bootstrap, and saved on
  normal logout or expired disconnect linger. This intentionally does not yet
  persist charges/basepoints/periodic timers or CMaNGOS offline-expiration
  nuances.
- Spell cooldown persistence currently saves spell-level cooldowns plus the
  source item id for item-use cooldowns, which should cover Hearthstone-style
  relog cooldown UI. It still logs/skips standalone category cooldown
  persistence until runtime cooldown state keeps the full CMaNGOS
  spell/category/item tuple together.
- Item instance charges are now loaded from `item_instance.charges`, repaired on
  login from real `item_template` spell charges when the instance has no
  charges, and emitted in item create blocks. Real-client food/water tooltip
  proof is still needed.
- Respawn aggro grace now uses map-owned creature runtime state: respawned DB
  creatures suppress sight aggro until CMaNGOS' default
  `CreatureRespawnAggroDelay` of 5000 ms elapses, while player-initiated combat
  clears the grace like CMaNGOS `SetCanAggro(true)`.
- Queued next-melee rage spending now sends the player `UNIT_FIELD_POWER2`
  update immediately after the map-owned spend and before queued
  `SMSG_SPELL_GO`, matching the normal spell-cast power ordering. This still
  needs real-client proof for the transient rage UI zero.
- Remaining original playtest gaps that still need source/data-backed slices:
  exploration discovery and XP require AreaTable/map area-flag ownership;
  Garrick Padfoot respawn timing should be verified from the active world DB
  row; rage UI zero flicker may need packet-order tracing in a real client
  capture if the latest ordering fix is not enough.

## Key Files

- `crates/wow-db/src/world_data.rs`
- `crates/wow-network/src/world/globals/object_mgr.rs`
- `crates/wow-network/src/world/map_runtime/world_data.rs`
- `crates/wow-network/src/world/map_runtime/map_manager.rs`
- `crates/wow-network/src/world/map_runtime/map/{players.rs,creature_damage.rs,creature_lifecycle.rs,spatial.rs}`
- `crates/wow-network/src/world/combat/{lifecycle.rs,runtime.rs}`
- `crates/wow-network/native/mmap_path.cpp`
- `crates/wow-network/src/world/combat/motion.rs`
- `crates/wow-network/src/world/handlers/mmap_path.rs`
- `crates/wow-network/src/world/spells.rs`
- `crates/wow-network/src/world/spells/{effects.rs,spell_mgr.rs,targets.rs}`
- `crates/wow-network/src/world/handlers/gameobject.rs`
- `crates/wow-network/src/world/globals/object_mgr.rs`
- `crates/wow-network/src/world/map_runtime/map_manager.rs`
- `crates/wow-network/src/world/map_runtime/map/players.rs`
- `crates/wow-db/src/world_data.rs`
- `crates/wow-network/src/world/session.rs`
- `crates/wow-network/src/world/server/logout.rs`
- `crates/wow-network/src/world/server/player_login.rs`
- `crates/wow-network/src/world/handlers/{gossip.rs,inventory.rs,chat.rs,vendor.rs}`
- `crates/wow-network/src/world/packet_builders/gossip.rs`
- `crates/wow-proto/src/world_packets.rs`
- `crates/wow-network/src/world/server/world_session.rs`
- `crates/wow-network/src/world/server/session_loop.rs`
- `crates/wow-network/src/world/handlers/inventory.rs`
- `crates/wow-db/src/character/{queries.rs,state.rs,types.rs}`
- `crates/wow-network/src/world/opcodes.rs`
- `crates/wow-network/src/world/tests.rs`
