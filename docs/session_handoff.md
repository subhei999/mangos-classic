# Session Handoff

Short operating brief for the next Rust migration session. Durable roadmap
history belongs in `docs/rust_migration_plan.md`, gate status in
`docs/playable_gate_board.md`, and benchmark chronology in
`docs/performance_movement_benchmark.md`.

## Current Branch And State

- Branch: `codex/rusty-mangos`
- Workspace: `C:\Users\subhe\Documents\New project`
- Latest local integration state includes the pushed active-spell teleport
  cleanup checkpoint `02388436c` on `codex/rusty-mangos` and the latest
  aura/target invalidation spell slice.
- Local multiplayer teleport RCA/fix is uncommitted: same-map teleport paths
  (`set_player_position` callers such as near-teleport/hearth-style movement)
  were refreshing environment flags but not immediately rebuilding
  player-player visibility. That could leave stale observer sets and stale
  nearby-player create/destroy state until relog. `MapRuntime::set_player_position`
  now runs the player visibility diff immediately, reuses the same enter/leave
  create/destroy flow as the batched visibility-refresh phase, and has focused
  regression coverage that proves old observers get destroy packets, new
  observers get create packets at the teleported position, and no pending
  player-visibility refresh remains after the teleport reposition.
- Local GM `.go` teleport RCA/fix is uncommitted: the GM path was still routing
  same-map teleports through `update_player_position(..., MSG_MOVE_HEARTBEAT, ...)`
  before sending `MSG_MOVE_TELEPORT_ACK`, unlike CMaNGOS' near-teleport flow.
  That meant `.go` behaved like spoofed movement instead of an instant
  relocation. `.go` now uses the same `set_player_position` relocation path as
  the rest of Rust's same-map teleports, which clears active spell runtime,
  refreshes environment state, and rebuilds immediate player visibility without
  fabricating a heartbeat movement update first.
- Local multiplayer right-click-turn RCA/fix is uncommitted: the movement actor
  was coalescing batched movement updates by player GUID only, so a same-batch
  `MSG_MOVE_SET_FACING` could supersede a heartbeat for that player. That fit
  the live symptom where remote players briefly snapped to a nearby offset with
  a slightly different facing only while holding right-click turn. The actor
  now keeps the latest movement packet per `(player, opcode)` and preserves the
  original order of the last surviving packets, so facing updates no longer
  erase same-batch positional movement.
- Local multiplayer movement-timestamp RCA/fix is uncommitted: the earlier
  assumption about CMaNGOS was wrong. CMaNGOS `MovementInfo::Write()` serializes
  synchronized `stime`, not raw `ctime`, for observer movement packets and
  living create blocks. Rust now writes synchronized movement `server_time` in
  `MSG_MOVE_*` observer broadcasts and in `build_other_player_create_block`,
  matching the actual CMaNGOS relay shape.
- Local multiplayer session-loop coalescing RCA/fix is uncommitted: the world
  session loop still buffered only one pending movement packet for the `10 ms`
  coalesce window and replaced older movement packets wholesale before
  dispatch. That meant a right-click `MSG_MOVE_SET_FACING` could still erase a
  heartbeat before the movement actor saw either packet. Rust now keeps an
  ordered short batch of pending movement packets through the session coalesce
  window instead of replacing the older one.
- Local multiplayer movement RCA/fix is uncommitted: live packet captures and
  direct CMaNGOS contrast resolved the remaining right-click-turn glitch. Rust
  now matches the key CMaNGOS movement shape more closely: synchronized
  movement time is used for observer movement/create packets,
  `MSG_MOVE_SET_FACING` carries its packet position through map-owned
  apply/broadcast again, and the movement actor no longer drops intermediate
  movement packets inside a batch.
- Startup fix after the dialogue merge: `wow-db` now treats missing optional
  local-starter DB tables `unit_condition`, `combat_condition`, and
  `broadcast_text` as empty instead of failing world runtime initialization.
  Full CMaNGOS world DB imports still use the real table data when present.
- Local stack account fix: `scripts/run-client-stack-18085.ps1` now seeds the
  documented `RUSTAUTH` / `RUSTPASS` account before seeding/preserving starter
  characters. The live DB was repaired; `RUSTAUTH` currently has `Rustone` and
  user-created `Twtowto`.
- Live local `mangos` DB was re-imported from ClassicDB using
  `scripts/import-classic-db-world.ps1` after the user saw an empty world. The
  previous local DB had schema only (`creature=0`, `gameobject=0`,
  `quest_template=0`). Fresh worldserver startup now reports
  `static_creature_spawns=59640`, `static_gameobject_spawns=33372`, and the
  Northshire query window has 108 creatures / 26 gameobjects.
- `RUSTAUTH` has GM privileges (`realmd.account.gmlevel=3`) in the live DB, and
  `scripts/run-client-stack-18085.ps1` now preserves/seeds it that way.
- Dialogue regression follow-up: `wow-db::get_vendor_items` and
  `wow-db::get_trainer_spells` now merge CMaNGOS template-backed service rows
  (`npc_vendor_template` via `VendorTemplateId`, `npc_trainer_template` via
  `TrainerTemplateId`) with direct rows. This should restore merchants/trainers
  whose gossip flags were visible but whose service backing looked empty in
  Rust. The release stack was restarted after the fix.
- Trainer gossip live RCA: the mage attempt against Khelden Bremen reached
  Rust as `CMSG_GOSSIP_HELLO` and Rust sent a two-option menu, but the client
  never sent `CMSG_GOSSIP_SELECT_OPTION`. The data was not missing:
  Khelden's text ids `538/539` live in CMaNGOS' `npc_text_broadcast_text`
  overlay and point to `broadcast_text` rows `2502/2503`. Rust now recognizes
  `npc_text_broadcast_text` as valid gossip text backing and resolves primary
  text through `broadcast_text`; missing `CMSG_NPC_TEXT_QUERY` ids still use
  CMaNGOS' `"Greetings $N"` fallback. Release stack was restarted; the next
  real-client mage trainer click is the live proof.
- Existing GitHub issue #75 still tracks remaining non-merchant service actions:
  taxi, innkeeper, bank, auction, stable, tabard, talent reset, POI, gossip
  scripts/locales, and full npc_text parity.
- Bank integration: `codex/banking-parity` was preserved as commit
  `bec18507b` and merged into `codex/rusty-mangos`. The slice adds banker
  activation, bank-slot purchase backed by `BankBagSlotPrices.dbc`, bank item
  storage slots and bank bag slots, autobank/autostore bank packets, persistent
  `playerBytes2` bank-slot count updates, and gossip `GOSSIP_OPTION_BANKER`
  dispatch into the bank opener. The stack still needs a real-client banker
  smoke.
- Live vendor RCA: Brog Hamfist's ClassicDB data is correct (`VendorTemplateId`
  `1100` includes `Small Brown Pouch` and `Brown Leather Satchel`), but Rust
  was filtering `item_template.ContainerSlots != 0` out of
  `wow_db::get_vendor_items`, hiding all bags from vendor lists. That filter is
  removed locally and the release stack was restarted; Brog in Goldshire should
  now list bags.
- Local GM convenience command is uncommitted: `.modify money #copper` adds
  copper to the active character, persists `characters.money`, and sends a live
  `PLAYER_FIELD_COINAGE` update. It requires GM security 3 and `.gm on`.
- Live bank-bag drag RCA/fix is uncommitted: dropping an item onto a bank bag
  icon can arrive as `CMSG_AUTOSTORE_BAG_ITEM` with a bank bag destination, or
  as a `CMSG_SWAP_ITEM` targeting `bag0/slot63..68`. Rust only resolved normal
  inventory bag icons, so bank-bag icon drops could no-op and leave the client
  item gray. Rust now resolves bank bag icons through the CMaNGOS-shaped
  `CanBankItem(bag, NULL_SLOT, ...)` behavior into the first valid contained
  bank-bag slot, and sends an equip failure when autostore has no destination.
  Release stack was restarted; needs live client retry.
- Mail integration: `codex/mail-system-parity` was preserved as commit
  `42e985da2` and merged into `codex/rusty-mangos`. The slice adds mail
  opcodes/packet parsing, mailbox proximity checks, send mail, list mail,
  take money, take item, mark read, return/delete, item text query/copy, COD
  handling, recipient/team/self/cap validation, attachment validation, and DB
  helpers for `mail`, `mail_items`, `item_text`, `item_instance`, and character
  money/inventory state. Follow-up hotfix: money-only player mail now delivers
  immediately (`deliver_time = now`) while item/COD/other player mail continues
  using the default one-hour delay. The next live proof is sending money from
  one `RUSTAUTH` character to another through a mailbox, then logging into the
  recipient and taking the money without manually fast-forwarding the DB row.
  Live mailbox-open disconnect RCA found `mail.stationery` is signed
  `tinyint(3)` in `sql/base/characters.sql`; Rust now decodes that column as
  signed and converts it for packet output instead of ending the session.
- Current protocol cleanup is uncommitted: `wow-proto` is now the single owner
  for world opcode numeric values via `wow_proto::world::WorldOpcode`.
  `wow-network` no longer has `world/opcodes.rs` or parallel `CMSG_`/`SMSG_`/
  `MSG_` constants; the old file was renamed to `world/constants.rs` because it
  now only carries non-opcode constants. Do not reintroduce network-owned
  opcode numbers when resolving older branch conflicts.
- Local world layout cleanup is uncommitted: `map_runtime/map.rs` was renamed
  to `map_runtime/state.rs`, and the nested `map_runtime/map/*` extension
  modules were renamed to `map_runtime/systems/*`. `world/README.md` now
  distinguishes live runtime areas from CMaNGOS parity scaffolds. The same
  cleanup removed the legacy synthetic `Rust Guide` NPC fixture path and old
  session-owned DB-creature combat/spell shims; DB creature queries, gossip,
  vendor inventory, and creature combat now rely on DB/map-owned paths only.
- The worktree is intentionally dirty with the opcode-ownership cleanup plus
  untracked `logs/` RCA captures until that cleanup is reviewed/landed.
- Local playerbots remain disabled in `config/worldserver.local.toml`.
- OOC EventAI is enabled again in
  `crates/wow-network/src/world/server/map_update.rs`; future RCA controls
  should include its map-owned tick cost.

## Current Goal

Immediate user-directed priority is multiplayer visual parity around same-map
teleport plus right-click-turn observation in Northshire. The latest local fixes
make teleport/set-position rebuild player visibility immediately instead of
waiting for relog or later movement, route GM `.go` through the real relocation
path, and keep `MSG_MOVE_SET_FACING` from erasing same-batch heartbeat
movement. The next local release stack restart already includes the additional
  movement-timestamp fix that now preserves CMaNGOS-shaped synchronized
  movement `server_time` in observer broadcasts and late create blocks. The
  currently restarted local release stack also includes
the session-loop pending-movement batch fix so heartbeat plus facing packets now
survive both coalescing layers, plus the new map-owned `MSG_MOVE_SET_FACING`
position clamp so right-click-turn packets rotate without dragging observers
through the packet's tiny client-side XY drift. Next proof should be a
two-client real-client smoke:

- teleport one player into Northshire near another player and confirm the
  observer immediately gets correct create/destroy behavior;
- turn in place on both clients and confirm remote players rotate around their
  correct position without the old pivot/offset symptom;
- hold right-click turn while moving and while stationary, then confirm the
  observer no longer sees the remote player snap to a nearby offset/facing and
  back;
- relog only if needed to compare the old broken state against the fixed one.

After that, resume the prior trainer-gossip verification and spell-system
parity work, starting with Polymorph and generic hard-control aura behavior.

- Polymorph already has transform display, damage break, single-target
  replacement, helper regen, diminishing metadata, combat preservation, and
  confused-motion coverage.
- CMaNGOS check for the real-client Polymorph smoke issues:
  `Aura::HandleModConfuse` calls `SetConfused(...)` and
  `HostileRefManager::HandleSuppressed(...)`; it does not call `CombatStop` or
  erase threat. The Rust slice now follows that: hostile aura application still
  starts/keeps combat, while confuse/fear/damage-break stun suppress sight aggro,
  normal/chase movement starts, and creature reaction until control ends.
- Root/stun movement blocking must still win over confuse motion. A rooted
  Polymorph target keeps the pending confused-wander due time but does not start
  or advance confused splines until the movement-blocking aura ends.
- The current dirty implementation slice makes natural aura expiration follow the
  same map-owned control cleanup expectations as damage-break/manual removal:
  expired Polymorph leaves confused motion, clears transform display, reconciles
  single-target aura trackers, and retires active diminishing aura bookkeeping.
- The same slice now adds CMaNGOS-shaped hard-control action gates:
  player spell-cast failure returns stun/confuse/fear/silence/pacify results,
  player auto attacks pause under hard control, creature spell-list/EventAI
  casts do not schedule while controlled, and in-flight creature casts
  interrupt if hard control lands before completion.
- Latest Polymorph smoke fix:
  CMaNGOS `EnterEvadeMode` removes normal negative auras through
  `RemoveAllAurasOnEvade`, so Rust evade now clears DB-creature active auras,
  sheep display override, active confused motion, single-target aura trackers,
  and active diminishing bookkeeping before return-home motion starts. The
  evade sender also immediately broadcasts aura/display updates so a client
  cannot keep rendering the mob as sheep after the map owner cleared it.
- CMaNGOS classifies Polymorph as `DRTYPE_PLAYER`; ordinary PvE DB creatures do
  not get player-style Polymorph diminishing levels. Rust now uses no
  DB-creature PvE diminishing group for Polymorph, which also removes the
  "re-sheep after evade is still DR immune" symptom for normal mobs.
- Hostile aura casts that fail due aura rank/bounce now still begin
  DB-creature retaliation before sending the spell failure. This covers failed
  sheep-style hostile aura applications instead of only successful applications.
- Confirmed CMaNGOS expectation: Polymorph can be resisted as a hostile magic
  spell. Rust now resolves hostile DB-creature aura-only miss/resist before
  building `SMSG_SPELL_GO`; resisted Polymorph-style casts encode the miss
  target in `SMSG_SPELL_GO`, do not send an extra `SMSG_SPELLLOGMISS`, do not
  apply the aura, and still start creature retaliation.
- Target outcome resolution has started moving toward the CMaNGOS
  `Spell::AddUnitTarget` / `TargetInfo::missCondition` shape. Player-cast
  hostile DB-creature unit-target school damage and hostile aura spells now
  resolve one pre-GO `PlayerSpellTargetOutcome`; `SMSG_SPELL_GO` consumes that
  miss list, missed targets skip all damage/aura effects, and delayed pending
  spell impacts carry the resolved hit outcome so impact code does not reroll a
  second full resist.
- Latest target-outcome extension: item-cast hostile DB-creature unit-target
  school damage now uses the same CMaNGOS-shaped pre-GO outcome. On-use hostile
  school-damage spells prepare as item casts; resisted item casts encode the
  miss target in `SMSG_SPELL_GO`, do not send an extra `SMSG_SPELLLOGMISS`, do
  not apply damage, and still begin DB-creature retaliation. Hit item casts use
  the normal player spell impact path with the item GUID preserved as the packet
  source.
- The "damage log appears but floating damage over the head does not" report is
  still unproven. Rust uses `SMSG_SPELLNONMELEEDAMAGELOG` for spell damage,
  which matches the existing CMaNGOS-shaped packet path; next step is a packet
  capture/settings comparison before adding `SMSG_ATTACKERSTATEUPDATE` for
  spell damage.
- Tentative spell parity roadmap:
  1. active cast interrupt/cancel parity
  2. target outcome generalization for immune/evade/reflect/player/PvP/AoE
     target lists
  3. Polymorph edge polish from real-client smoke and CMaNGOS packet comparison
  4. triggered spell source/outcome/proc architecture
  5. aura interrupt/proc behavior
  6. class spell parity slices for Mage, Warrior, and creature/EventAI spells
- Current next spell slice: active cast interrupt/cancel parity. We are not
  starting from zero: movement opcodes already cancel active player casts,
  explicit cancel opcodes share the same helper, map-owned active casts already
  support damage pushback, channels support damage interrupt/pushback, and
  opening casts have their own cancel path. The first parity gap is that Rust
  currently cancels on movement opcodes without consulting the active spell's
  `SPELL_INTERRUPT_FLAG_MOVEMENT`; CMaNGOS cancels normal non-triggered
  non-auto-repeat casts on movement only when that interrupt flag is present,
  and cancels channels through `ChannelInterruptFlags & AURA_INTERRUPT_FLAG_MOVING`.
- First active-cast interrupt/cancel parity slice is now implemented locally:
  movement-triggered cancellation uses a dedicated helper instead of the
  explicit cancel helper. Active player casts only cancel on movement when their
  `interrupt_flags` include `SPELL_INTERRUPT_FLAG_MOVEMENT`, while player
  channels and dynamic-object channels only cancel on movement when their
  channel interrupt flags include `AURA_INTERRUPT_FLAG_MOVING`. Explicit cancel
  opcodes still use the unconditional cancel path.
- Damage interrupt/pushback parity slice is now implemented locally for the
  player damage paths Rust currently wires: direct creature damage first
  interrupts active player casts with `SPELL_INTERRUPT_FLAG_DAMAGE_CANCELS`;
  otherwise it applies CMaNGOS-style cast delay only when
  `SPELL_INTERRUPT_FLAG_DAMAGE_PUSHBACK` is present. Channel damage handling
  still uses channel interrupt flags: `AURA_INTERRUPT_FLAG_DAMAGE` cancels and
  `AURA_INTERRUPT_FLAG_DAMAGE_CHANNEL_DURATION` shortens channel duration.
- Hard-control active-cast invalidation is now implemented locally for player
  aura application. Applying stun/confuse/fear follows CMaNGOS `CastStop` shape
  and interrupts active player casts; applying silence interrupts only
  silence-prevented active casts; pacify blocks new melee-prevented casts but
  does not retroactively interrupt an existing cast, matching
  `HandleAuraModPacify`.
- Latest active-cast lifecycle slice: map-owned player death, logout/removal,
  and combat-disconnect linger now clear the full active player spell runtime,
  not only cast timers. Cleanup removes active casts, pending spell events,
  active player channels, queued channel impacts, and caster-owned dynamic
  objects, while sending channel/dynamic-object clear packets to observers when
  the caster leaves the map or dies.
- Latest teleport invalidation slice: near-teleport/set-position now clears
  the same map-owned active player spell runtime while ordinary movement still
  preserves non-movement-interrupt casts. GM `.go` now also invokes the
  explicit active-spell cleanup before applying its movement update.
- Latest aura/target invalidation slice: external removal of a DB-creature
  channeled aura now interrupts the matching player channel, clears queued
  channel impacts, and sends channel-clear packets to the caster/observers.
  DB-creature target death or runtime deletion now interrupts active and
  delayed player spell work targeting that unit, mirroring CMaNGOS'
  channeled-aura removal and lost/dead unit target cancellation paths.

Recent RCA/perf work is committed at `b58c6ca81` and pushed to
`origin/codex/rusty-mangos`; keep the detailed benchmark chronology below as
reference, but feature work is now back on spells.

Latest measurement caveat/fix:

- A 10-second live WPA capture during the `500` same-grid real-client playtest
  found the observability endpoint itself hot:
  `run_metrics_endpoint -> render_prometheus -> Histogram::rolling_stats`.
- `Histogram::rolling_stats` now reads maintained one-second rolling buckets
  instead of scanning the full five-minute sample deque on every scrape.
- `/metrics` now serves a cached Prometheus render for `5s`, and the embedded
  dashboard refresh interval is also `5s`, so the dashboard should no longer
  meaningfully perturb the load test.

Latest load-harness change:

- `bins/world-load-test` now supports `--stream-clients`, exposed through
  `scripts/start-thin-client-load.ps1 -StreamClients`.
- In streaming mode, the harness seeds one account/character and immediately
  starts that client thread before seeding the next one. This avoids the old
  behavior where the first visible client had to wait for every load-test
  account to be prepared first.
- Streaming mode also bypasses the all-clients movement start gate by using a
  per-client-open gate, so early clients do not time out waiting for a large
  `2000`-client ramp to finish.
- First use was a live `2000` `creature_grid_scatter` map-0 run with
  `50 ms` movement, `5` stationary mage sentinels, `512 KiB` client thread
  stacks, and `1800s` hold:
  `logs/perf-rca/20260519-101331-2000-creature-grid-scatter-50ms-stream-clients-full-load.summary.prom`.
  At full ramp it reached `2001` connected/active players including the real
  client, `22141` active creatures, `2129` tracked idle-motion creatures, map
  tick avg/max `181.591/256.183 ms`, tick lag avg/max `132.652/253.714 ms`,
  idle-motion avg/max `34.159/90.344 ms`, movement actor queue age avg/max
  `10.014/88.331 ms`, and `CMSG_CAST_SPELL` dispatch max stayed below
  `100 ms` in the captured summary.
- A laggy 10-second WPR/WPA capture from that same live run is:
  `logs/perf-rca/20260519-101815-2000-creature-grid-scatter-50ms-stream-clients-laggy-quick10-wpa.etl`.
  The next hot branch pasted from WPA was:
  `MapRuntime::db_creature_snapshots -> Vec::from_iter -> Creature::clone`.
- The confirmed cause was session combat tick polling for return-home creature
  motions by cloning full visible `DbCreatureRuntime` snapshots only to filter
  `CreatureMotionState::ReturnHome`.
- The local fix adds a map-owned `db_creature_return_home_guids(...)` query and
  makes `advance_db_creature_return_home_motions(...)` fetch only matching
  GUIDs, avoiding the full creature clone path in this hot loop.
- Follow-up WPA expansion on the same `2000` spread run showed no single new
  giant offender. The largest named app work in the top Tokio task was
  `sync_player_gameplay_state`, `expire_disconnected_players`,
  `select_db_creature_sight_aggro_targets`, and the new
  `db_creature_return_home_guids`. This points to distributed per-session
  map interaction pressure rather than one remaining smoking gun.
- Two local fixes were added from that branch:
  - disconnected-player expiry is now map-loop-owned via
    `MapRuntimeManager::expire_all_disconnected_players(...)` and a new
    `disconnected_player_expiration` map phase, instead of scanning a map's
    players from every session combat tick
  - `MapRuntime::sync_player_gameplay_state(...)` now only clones session
    active spells, inventory, and quest-status collections into map state when
    those collections actually changed; `CharacterInventoryItem` and
    `CharacterQuestStatus` now derive `PartialEq/Eq` to support that cheap
    comparison
- Follow-up quick WPA after those changes showed the same top Tokio task shape,
  but smaller:
  `sync_player_gameplay_state` around `288 ms`,
  `select_db_creature_sight_aggro_targets` around `217 ms`,
  `batch_semaphore::add_permits_locked` around `188 ms`, and
  `db_creature_return_home_guids` around `132 ms`; disconnected-player expiry
  disappeared from that hot path.
- A second local pressure-source pass moves return-home motion fully into the
  map-owned creature motion tick and removes runtime session polling for
  `advance_db_creature_return_home_motions(...)`. Return-home creatures now use
  the same active motion advancement queue as random/confused/waypoint motion.
- Sight aggro selection is now map-owned throttled/dirty-gated per player:
  steady players check at most every `250 ms`, while players that move at least
  `2 yd` can check immediately. The selector also sorts candidate
  `(distance, guid)` pairs before cloning final target snapshots, avoiding
  clone work for ordering.
- A rebuilt `1000` `creature_grid_scatter` run with OOC EventAI enabled
  produced a quick WPA hot path in
  `MapRuntime::player_runtime_snapshot -> Vec::clone`. The pressure source was
  high-frequency movement/session tick code asking the map for a full gameplay
  snapshot, including inventory, quest statuses, auras, cooldown maps, and
  active spells, when it only needed position/death/vitals/combat flags.
- The local fix adds `PlayerRuntimeSessionSnapshot` and
  `MapRuntimeManager::player_runtime_session_snapshot(...)`; movement handling,
  character position persistence, and the idle session tick refresh now use the
  narrow snapshot. Full `PlayerRuntimeSnapshot` remains for non-movement
  packet pre-refresh and gameplay handlers that need rich state.

## What Is Proven

- Disabling the map-owned OOC EventAI phase immediately restored:
  - NPC idle patrol motion
  - mana / health regen
  - other timed map systems
- So OOC EventAI was a real regression source, but it is not the whole
  `1000`-client movement-flood problem.
- The earlier session-loop starvation work is already landed:
  - DB-creature lifecycle is map-owned
  - OOC EventAI scans were removed from `handle_combat_tick(...)`
  - active creature attack processing is collapsed into map-owned victim
    transactions
- A real-client hostile caster hang was reproduced against Burning Blade
  Neophyte (`entry=3196`, combat EventAI `348 = Immolate`) and reduced to an
  async mutex lifetime bug in the manager-owned combat wrapper.
- That deadlock is fixed, and the same bug class was audited/fixed in the
  playerbot manager loops.
- The movement actor only coalesces after a movement packet is already inside
  the movement path; it does not reduce the session-side per-packet work.
- Our current movement handler still does far more inline work than CMaNGOS:
  movement map update, creature/gameobject/corpse visibility rescans, aggro
  start checks, area discovery, and session-to-map gameplay-state sync.

## Latest Change

The movement path is now materially thinner and more map-owned:

- authenticated sessions still coalesce same-session movement bursts for `10 ms`
  in `crates/wow-network/src/world/server/session_loop.rs`
- pure movement packets no longer force an immediate
  `sync_active_player_gameplay_state(...)` after dispatch; sync still happens
  for non-movement packets and on the world-tick path
- `crates/wow-network/src/world/server/movement.rs` no longer starts
  DB-creature aggro inline on every successful move; we now rely on the
  existing once-per-world-tick `handle_combat_tick(...)` aggro path instead
- player area discovery checks are now throttled to `100 ms` via
  `MovementSessionState::next_position_status_update_at`, matching the
  CMaNGOS idea of throttled position-status updates instead of per-packet work
- player-to-player enter/leave visibility diffing and
  `sync_db_creature_idle_motion_tracking_for_player_interest_positions(...)`
  were removed from inline `MapRuntime::update_player_position(...)`
- movement now only marks a dirty player-visibility refresh
- the new map-owned `player_visibility_refresh` phase runs once per map tick,
  batches each player once, updates player-player visibility, and then performs
  the deferred creature-interest sync before idle motion
- the thin-client harness now supports `--move-phase-jitter-ms` for a
  deterministic per-client movement start offset after the shared ready gate;
  this keeps the same per-client interval but avoids all clients sharing the
  same movement phase
- observability now splits the new `player_visibility_refresh` phase into:
  - `wow_player_visibility_refresh_visibility_diff_broadcast_time_*`
  - `wow_player_visibility_refresh_creature_interest_sync_time_*`
  and movement packet ownership already had:
  - `wow_movement_map_mutex_wait_*`
  - `wow_movement_map_mutex_hold_*`
- movement observability now also exposes an explicit pipeline split:
  - actor enqueue -> apply start latency:
    `wow_movement_actor_apply_start_latency_*`
  - per-applied-move counts:
    `wow_movement_apply_observers_notified_*`
    `wow_movement_apply_packets_emitted_*`
  - `MapRuntime::update_player_position(...)` subphases:
    - `wow_movement_apply_observer_snapshot_time_*`
    - `wow_movement_apply_movement_broadcast_time_*`
    - `wow_movement_apply_grid_update_time_*`
    - `wow_movement_apply_player_state_environment_time_*`
    - `wow_movement_apply_fall_damage_broadcast_time_*`
    - `wow_movement_apply_death_presentation_time_*`
    - `wow_movement_apply_visibility_refresh_mark_time_*`
    - `wow_movement_apply_total_time_*`
  - the HTML dashboard now has a **Movement Pipeline** panel for these metrics
- movement packets now take a more aggressive session-loop fast path:
  - they skip pre-dispatch `refresh_active_player_session_cache(...)`
  - they skip pre-dispatch death finalization
  - they skip pre/post pending player spell completion checks unless the
    session already has active spells
  - the main session-loop timeout path also skips map `next_pending_player_spell_cast_due_at(...)`
    lookups unless the session already has active spells
  This is an explicit measurement experiment aimed at reducing movement
  `dispatch/service` cost before we decide whether deeper actor/map-thread
  ownership work is still needed.

Creature/gameobject/corpse visibility streaming from `movement.rs` still
remains inline and distance-gated, so it is the next likely movement-side
effect family to revisit if the harness still lags badly.

RCA setup added this session:

- `docs/performance_rca_runbook.md` maps the user's fishbone to current crate
  boundaries, existing metric names, run shapes, jitter matrix, and decision
  rules for identifying the first growing queue or phase.
- `scripts/capture-rca-metrics.ps1` and `.cmd` capture raw Prometheus metrics,
  a filtered RCA summary, git/status metadata, runtime environment, matching
  process command lines, world config snippets, and quick baseline metrics into
  `logs/perf-rca/`.
- Generic channel metrics are now exposed in Prometheus:
  - `wow_channel_queue_age_*{channel=...}`
  - `wow_channel_queue_depth_*{channel=...}`
  - `wow_channel_send_wait_*{channel=...}`
  These are wired for the production action-latency mailboxes:
  `movement_actor`, `world_session_outbound`, and
  `world_session_disconnect`.
- Tokio runtime metrics are now exposed when observability is enabled:
  `wow_tokio_runtime_workers`, `wow_tokio_task_count`,
  `wow_tokio_worker_busy_milliseconds`,
  `wow_tokio_runtime_global_queue_depth`, and, when built with
  `RUSTFLAGS=--cfg tokio_unstable`, task poll duration, local queue depth,
  spawn-blocking queue/thread counts, and cooperative forced-yield counters.
  `scripts/start-thin-client-load.ps1` exposes
  `-EnableTokioUnstableMetrics` for repeatable RCA controls.
- This setup intentionally does not add a new perf crate yet. Existing
  `wow-network` observability already covers the first RCA pass; add new
  metrics only when the runbook's current signals cannot isolate the next
  boundary.

First RCA control run captured:

- Command shape:
  `500` clients, `local_radius`, `MoveIntervalMs=50`,
  `MovePhaseJitterMs=0`, `LoginStaggerMs=1`, `HoldSeconds=90`,
  movement actor enabled.
- Capture files:
  - `logs/perf-rca/20260518-193541-500-local-radius-50ms-jitter0-actor-on.metrics.prom`
  - `logs/perf-rca/20260518-193541-500-local-radius-50ms-jitter0-actor-on.summary.prom`
  - `logs/perf-rca/20260518-193541-500-local-radius-50ms-jitter0-actor-on.metadata.md`
- Harness completed with `clients=500`, `failures=2`,
  `movements_sent=568890`, `packets_drained=5316421`; treat this as usable
  but not perfectly clean.
- Capture window reached `500` connected sessions and roughly `498-500` active
  players.
- First read:
  - multi-second delay is visible on inbound world packet dispatch/service for
    movement-like opcodes
  - outbound queue latency is tiny (`world_session_outbound` queue age average
    `0.036 ms`, max `2.068 ms`)
  - `movement_actor` queue age is non-zero but below the observed client delay
    (average `78.641 ms`, max `247.552 ms`)
  - movement apply itself is small compared with the lag (total average
    `2.057 ms`, max `34.256 ms`)
  - map tick spikes are large (duration max `1080.746 ms`, lag max
    `1059.029 ms`)
  This points the next RCA pass toward session/map scheduling and tick-phase
  spikes before outbound write, not outbound socket backlog and not the small
  per-movement apply subphases alone.

Spell-cast sentinel setup added after the first control:

- `bins/world-load-test` now supports an opt-in self-cast probe:
  - `--sentinel-cast-clients <n>`
  - `--sentinel-cast-spell-id <id>`; default `168` (`Frost Armor Rank 1`)
  - `--sentinel-cast-interval-ms <ms>`; default `5000`
  - `--sentinel-cast-phase-jitter-ms <ms>` to spread sentinel cast starts
  - `--disable-movement` to keep watch/sentinel clients stationary after login
  - `--disable-sentinel-movement` to keep only sentinel clients stationary
    while the remaining load clients keep generating movement pressure
- The harness records `CMSG_CAST_SPELL` to matching `SMSG_CAST_RESULT`
  response latency in the final stdout:
  `casts_sent`, `responses`, `failures`, `pending`, `avg_response_ms`, and
  `max_response_ms`.
- `scripts/start-thin-client-load.ps1` exposes the same options and now also
  exposes character `Race`, `CharacterClass`, and `Gender`, so the sentinel run
  can seed mage clients with `-CharacterClass 8 -SentinelCastSpellId 168`.
- Tiny live smoke passed:
  `2` mage clients, `1` Frost Armor sentinel, `10s` hold, movement actor on.
  Result: `casts_sent=4`, `responses=4`, `failures=1`, `pending=0`,
  `avg_response_ms=129.019`, `max_response_ms=515.492`.
  The one spell failure does not block latency measurement, but a later
  success-only sentinel may need a different self-buff or longer interval.
- User watch group launched after the moving/synchronized first attempt was
  stopped: `5` stationary human mages near Northshire spawn, all self-casting
  Frost Armor with `5000 ms` phase jitter and no movement packets. Current
  harness PID at launch was `63072`; it was stopped before the full control.

Second RCA control with stationary spell sentinels captured:

- Command shape:
  `500` human mage clients, `local_radius`, `MoveIntervalMs=50`,
  `MovePhaseJitterMs=0`, `LoginStaggerMs=1`, `HoldSeconds=90`, movement actor
  enabled, first `5` clients configured as stationary Frost Armor sentinels
  with `SentinelCastIntervalMs=5000`,
  `SentinelCastPhaseJitterMs=5000`, and `DisableSentinelMovement=True`.
- Full-load capture files:
  - `logs/perf-rca/20260518-200247-500-local-radius-50ms-jitter0-actor-on-5-stationary-mage-sentinels.metrics.prom`
  - `logs/perf-rca/20260518-200247-500-local-radius-50ms-jitter0-actor-on-5-stationary-mage-sentinels.summary.prom`
  - `logs/perf-rca/20260518-200247-500-local-radius-50ms-jitter0-actor-on-5-stationary-mage-sentinels.metadata.md`
- Post-run aggregate capture files:
  - `logs/perf-rca/20260518-200440-500-local-radius-50ms-jitter0-actor-on-5-stationary-mage-sentinels-postrun.metrics.prom`
  - `logs/perf-rca/20260518-200440-500-local-radius-50ms-jitter0-actor-on-5-stationary-mage-sentinels-postrun.summary.prom`
  - `logs/perf-rca/20260518-200440-500-local-radius-50ms-jitter0-actor-on-5-stationary-mage-sentinels-postrun.metadata.md`
- Full-load scrape reached `501` connected sessions / `501` active players
  because the user's real client was also connected.
- Full-load `CMSG_CAST_SPELL` (`0x012E`) server timings from the first `15`
  sentinel casts:
  - dispatch delay average `122.445 ms`, max `228.313 ms`
  - handler duration average `403.344 ms`, max `893.445 ms`
  - total service time average `525.792 ms`, max `981.541 ms`
- Post-run aggregate after `82` received spell cast packets:
  - dispatch delay average `124.390 ms`, max `228.313 ms`
  - handler duration average `347.487 ms`, max `907.162 ms`
  - total service time average `471.880 ms`, max `1006.878 ms`
- Queue/tick context from the full-load scrape:
  - `movement_actor` queue age average `69.890 ms`, max `219.645 ms`
  - `world_session_outbound` queue age average `0.034 ms`, max `12.175 ms`
  - map tick duration average `23.997 ms`, max `977.393 ms`
  - map tick lag average `27.537 ms`, max `970.099 ms`
- The load harness exited with `0xc0000005` after the run, so the final
  client-side `sentinel-cast summary` line was not emitted. Treat server-side
  spell opcode metrics as the usable control measurement and the missing
  harness summary as an unproven harness bug.

Harness crash mitigation added:

- Newest Windows crash dump was
  `C:\Users\subhe\AppData\Local\CrashDumps\world-load-test.exe.41676.dmp`.
  Several dumps had the same access-violation shape: faulting read at `0x24`
  from the packet-drain timeout/error classification path.
- `bins/world-load-test` no longer classifies timeout reads by calling
  `anyhow::Error::downcast_ref::<std::io::Error>()` in the hot drain/login/logout
  paths. Packet reads now return a concrete `WorldPacketReadError`, so timeout
  handling is direct and avoids the trait-object downcast path seen in the
  dumps.
- The harness no longer forces client threads onto a `256 KiB` stack by
  default. Default per-client thread stack is now `1024 KiB`, with
  `--client-thread-stack-kb <kb>` exposed for experiments. The PowerShell
  wrapper exposes `-ClientThreadStackKb`.
- Post-fix verification against the existing release server:
  - `500` clients, `20s`, `5` stationary mage sentinels, movement load:
    completed with `failures=0`, `casts_sent=30`, `responses=30`,
    `avg_response_ms=757.459`, `max_response_ms=1387.023`.
  - `500` clients, `90s`, same sentinel shape, `MaxAttempts=1`: completed
    without access violation and printed the sentinel summary; exited through
    normal harness failure handling with `6` client failures. Result:
    `casts_sent=71`, `responses=71`, `avg_response_ms=711.371`,
    `max_response_ms=1230.748`.
  - No newer `world-load-test.exe` crash dump appeared after these patched
    runs. Treat the `0xc0000005` as mitigated unless it reappears under the
    default `MaxAttempts=3` script path.

Runtime-metrics control captured after adding the remaining RCA setup:

- Command shape matched the stationary sentinel control and added
  `-EnableTokioUnstableMetrics`: `500` human mage clients, `local_radius`,
  `MoveIntervalMs=50`, `MovePhaseJitterMs=0`, `LoginStaggerMs=1`,
  `HoldSeconds=90`, movement actor enabled, `5` stationary Frost Armor
  sentinels with `SentinelCastIntervalMs=5000`,
  `SentinelCastPhaseJitterMs=5000`.
- Capture files:
  - `logs/perf-rca/20260518-204613-500-local-radius-50ms-jitter0-actor-on-5-stationary-mage-sentinels-runtime-tokio.metrics.prom`
  - `logs/perf-rca/20260518-204613-500-local-radius-50ms-jitter0-actor-on-5-stationary-mage-sentinels-runtime-tokio.summary.prom`
  - `logs/perf-rca/20260518-204613-500-local-radius-50ms-jitter0-actor-on-5-stationary-mage-sentinels-runtime-tokio.metadata.md`
- Run completed without an access violation and emitted the harness sentinel
  summary: `clients=500`, `failures=5`, `movements_sent=554466`,
  `packets_drained=5227195`; spell sentinel result `casts_sent=89`,
  `responses=89`, `failures=45`, `pending=0`,
  `avg_response_ms=665.147`, `max_response_ms=1393.490`.
- Full-load scrape reached exactly `500` connected sessions and `500` active
  players.
- Runtime health from the scrape:
  - `wow_tokio_runtime_workers=24`, `wow_tokio_task_count=1008`
  - worker busy duration over the latest ~1s interval was `9257.459 ms`
    across all workers, so the runtime was busy but not saturated across
    `24` workers
  - `wow_tokio_runtime_global_queue_depth=0`
  - `wow_tokio_task_poll_duration_milliseconds=0.029`
  - `wow_tokio_spawn_blocking_queue_depth=0`
- Server spell path for `CMSG_CAST_SPELL` (`0x012E`) during the scrape:
  - dispatch delay average `142.706 ms`, max `191.971 ms`
  - handler duration average `668.709 ms`, max `917.284 ms`
  - total service time average `811.418 ms`, max `1030.636 ms`
  - outbound `SMSG_CAST_RESULT` (`0x0130`) queue/write remained tiny:
    queue average `0.019 ms`, max `0.041 ms`; write average `0.011 ms`,
    max `0.014 ms`
- Movement / map context:
  - `movement_actor` queue age average `80.796 ms`, max `416.761 ms`
  - `world_session_outbound` queue age average `0.048 ms`, max `8.101 ms`
  - movement apply total average `1.958 ms`, max `27.935 ms`
  - map tick latest `672.009 ms`, max `860.766 ms`; tick lag latest
    `742.615 ms`, max `839.668 ms`
  - session-loop `packet_dispatch` average `2067.932 ms`, max
    `12211.373 ms`; `packet_branch_total` average `2683.226 ms`, max
    `12371.774 ms`
- Current read: the control captures the spell lag without manual casting.
  The first obvious problem remains session/map scheduling and long
  packet-branch/dispatch phases under movement flood; outbound socket queues,
  spawn-blocking, and Tokio global queue depth are not the current first
  bottleneck.

Core scalability matrix captured:

- New repeatable runner:
  `scripts/run-rca-scalability-matrix.ps1`.
- Core matrix id: `20260518-210239`.
- Matrix output:
  - `logs/perf-rca/matrix-20260518-210239/matrix-results.csv`
  - `logs/perf-rca/matrix-20260518-210239/matrix-analysis.csv`
  - `logs/perf-rca/matrix-20260518-210239/matrix-summary.md`
- Shape:
  - player counts: `50`, `100`, `250`, `500`
  - scenarios per count:
    - `idle-same-grid`
    - `movement-same-grid-sync`
    - `movement-same-grid-jitter250`
    - `movement-spread-sync`
  - all runs used stationary mage sentinels and
    `-EnableTokioUnstableMetrics`.
- Key sentinel spell response averages / maxes:
  - `50` idle: `1.300 ms` avg, `61.780 ms` max
  - `50` same-grid movement sync: `56.262 ms` avg, `152.812 ms` max
  - `50` same-grid movement jitter250: `41.920 ms` avg, `124.061 ms` max
  - `50` spread movement sync: `96.771 ms` avg, `2541.807 ms` max
  - `100` idle: `1.848 ms` avg, `62.107 ms` max
  - `100` same-grid movement sync: `124.034 ms` avg, `252.450 ms` max
  - `100` same-grid movement jitter250: `124.360 ms` avg, `230.526 ms` max
  - `100` spread movement sync: `134.111 ms` avg, `2772.751 ms` max
  - `250` idle: `2.096 ms` avg, `62.852 ms` max
  - `250` same-grid movement sync: `337.648 ms` avg, `544.339 ms` max
  - `250` same-grid movement jitter250: `334.476 ms` avg, `639.148 ms` max
  - `250` spread movement sync: `300.807 ms` avg, `4199.256 ms` max
  - `500` idle: `2.427 ms` avg, `61.617 ms` max
  - `500` same-grid movement sync: `608.477 ms` avg, `1152.261 ms` max
  - `500` same-grid movement jitter250: `634.240 ms` avg, `1256.794 ms` max
  - `500` spread movement sync: `252.782 ms` avg, `3303.231 ms` max
- Matrix interpretation:
  - Idle stays near-zero even at `500` players, so connected session count
    alone is not the root cause.
  - Movement at `50 ms` is the trigger. Same-grid movement response averages
    scale roughly `56 ms -> 124 ms -> 338 ms -> 608 ms` from
    `50 -> 100 -> 250 -> 500` players.
  - `250 ms` movement phase jitter does not help at `100+` players, so this is
    not primarily same-millisecond burst collapse.
  - Spread movement improves the `500` average versus same-grid movement
    (`252.782 ms` vs `608.477 ms`) but still has multi-second tails and still
    degrades badly from idle. That points to both sustained movement-path cost
    and some same-grid/AOI pressure, with sustained path cost first.
  - For the `500` same-grid movement sync scrape, `CMSG_CAST_SPELL` service
    average was `513.990 ms`; dispatch average `129.660 ms`, handler average
    `384.327 ms`. Movement actor queue age max was `269.298 ms`, outbound
    queue max stayed small, and spawn-blocking / Tokio global queue depth
    remained `0`.
- No new `world-load-test.exe` crash dump appeared during the matrix.

Rate-knee follow-up captured:

- Runner preset added: `RateKnee` in
  `scripts/run-rca-scalability-matrix.ps1`.
- Matrix id: `20260518-215725`.
- Output:
  - `logs/perf-rca/matrix-20260518-215725/matrix-results.csv`
  - `logs/perf-rca/matrix-20260518-215725/matrix-analysis.csv`
  - `logs/perf-rca/matrix-20260518-215725/matrix-summary.md`
- Shape: `500` players, same-grid movement, movement actor on, stationary mage
  sentinels, runtime metrics enabled.
- Results:
  - `MoveIntervalMs=250`: sentinel avg `676.117 ms`, max `1217.108 ms`;
    `CMSG_CAST_SPELL` service avg `623.038 ms`; movement apply avg
    `1.754 ms`; movement actor queue age max `219.012 ms`; outbound queue max
    `9.854 ms`; Tokio global queue and spawn-blocking queue both `0`.
  - `MoveIntervalMs=500`: sentinel avg `738.274 ms`, max `1386.574 ms`;
    `CMSG_CAST_SPELL` service avg `542.517 ms`; movement apply avg
    `1.722 ms`; movement actor queue age max `201.248 ms`; outbound queue max
    `2.457 ms`; Tokio global queue and spawn-blocking queue both `0`.
- Interpretation: slowing movement packets from `50 ms` to `250/500 ms` did
  not collapse the lag. The root is no longer best described as simple packet
  rate saturation. It looks more like movement-triggered session/map work or
  lock/scheduling interaction that remains costly once the 500 players are in
  the moving-state path.

Profiling attempt:

- Reproduced the worst case with a longer `500` same-grid `50 ms` movement run
  (`HoldSeconds=210`) and captured matching RCA metrics during the intended
  profile window:
  - `logs/perf-rca/20260518-220807-20260518-220616-wpr-500-same-grid-50ms-during-wpr.metrics.prom`
  - `logs/perf-rca/20260518-220807-20260518-220616-wpr-500-same-grid-50ms-during-wpr.summary.prom`
  - `logs/perf-rca/20260518-220807-20260518-220616-wpr-500-same-grid-50ms-during-wpr.metadata.md`
- The long run reproduced the lag: sentinel avg `745.499 ms`, max
  `1382.070 ms`; metrics window had `CMSG_CAST_SPELL` service avg
  `619.468 ms`, handler avg `533.172 ms`, dispatch avg `86.293 ms`;
  `packet_dispatch` avg `1273.856 ms`, `packet_branch_total` avg
  `1850.037 ms`; movement actor queue age max `413.546 ms`; Tokio global and
  spawn-blocking queues still `0`.
- `cargo flamegraph` / `flamegraph` is now installed in
  `C:\Users\subhe\.cargo\bin`.
- Attaching with `flamegraph --pid <worldserver-pid>` failed because
  `flamegraph` uses `dtrace` for PID attach on Windows, and `dtrace` is not
  installed on this machine.
- Command-mode `flamegraph -- <command>` fell back to the built-in Windows
  `blondie` backend. A non-elevated smoke failed with `NotAnAdmin`, but an
  elevated smoke succeeded and produced:
  `logs/perf-rca/20260518-223922-elevated-flamegraph-smoke.svg`.
- Elevated `flamegraph --pid 62696` still failed because PID attach always
  shells out to `dtrace`; elevation alone does not make attach work without
  installing/enabling Windows DTrace.
- Windows Performance Recorder is installed, but `wpr -start CPU -filemode`
  failed from a non-elevated shell with `0xc5585011` ("Failed to enable the
  policy to profile system performance"), but elevated WPR works.
- First elevated WPR attempt started before the load wrapper restarted the game
  stack and produced an ETL, but the load failed before gameplay
  (`500` failures, `0` movements, `0` packets drained), so treat that trace as
  startup/login noise:
  `logs/perf-rca/20260518-224201-wpr-500-same-grid-50ms-spell-sentinels.etl`.
- Useful elevated WPR profile captured during an already-steady `500` direct
  same-grid `50 ms` movement run with stationary mage spell sentinels:
  - WPR ETL:
    `logs/perf-rca/20260518-224929-wpr-steady-direct-500-same-grid-50ms-spell-sentinels.etl`
    (`14,918,090,752` bytes)
  - paired metrics:
    `logs/perf-rca/20260518-224941-wpr-steady-direct-500-same-grid-50ms-spell-sentinels.summary.prom`
  - post-WPR metrics:
    `logs/perf-rca/20260518-225255-post-wpr-steady-direct-500-same-grid-50ms-spell-sentinels.summary.prom`
- The steady WPR metrics window reached `501` connected sessions and
  `501` active players. Spell opcode `0x012E` had dispatch avg `89.072 ms`,
  handler avg `399.309 ms`, service avg `489.125 ms`, and service max
  `1861.016 ms`. Map tick latest/max were `983.079/1535.937 ms`; map tick lag
  latest/max were `1117.772/1505.345 ms`. Movement apply remained small
  (`1.956 ms` avg, `36.792 ms` max), while movement actor apply-start latency
  averaged `146.502 ms` and reply latency averaged `174.600 ms`.
- The direct load harness process stayed alive past its expected hold window
  and did not flush a final sentinel summary under/after the WPR run, so it was
  stopped manually after post-WPR metrics were captured. Treat this as another
  harness robustness caveat, not as invalidating the paired server metrics.
- WPA stack inspection of the useful ETL found the hottest async task under
  `world_session_writer`, with the hot branch:
  `world_session_writer -> tokio::time::timeout -> TcpStream::poll_write_priv -> std::net::tcp::write -> ws2_32.dll!send -> mswsock.dll!WSPSend`.
  This confirms the profile is showing real outbound socket write work, not
  just timeout/timer overhead.
- WPA stack inspection also found a second confirmed hot branch:
  `worldserver::main -> WorldGeometry::area_entry -> native_map_area_info -> wow_map_area_info -> VMAP::VMapManager2::loadMap`.
  This accounts for roughly `25 s` in view and means movement-driven player
  position status / area discovery is still reaching expensive native terrain
  or vmap lookup/load behavior during the steady load window.
- Current RCA read: movement pressure causes large outbound replication/write
  fanout and repeated terrain/area lookup work. Outbound queue age remains low
  because writers are actively draining work, but that write volume consumes
  scheduler/CPU time and coincides with delayed spell service and map tick
  spikes. The next evidence gaps are per-opcode outbound bytes / write cost /
  recipient fanout plus area-entry/native-vmap lookup counts, timings, and
  cache/load behavior.
- No new `world-load-test.exe` crash dump appeared during the rate-knee or
  profiling-attempt runs.

Attribution metrics added and control rerun:

- Code now emits:
  - `wow_world_packet_outbound_enqueued_bytes_total{opcode}`
  - `wow_world_packet_write_bytes_total{opcode}`
  - `wow_world_outbound_fanout_recipients_*{source,opcode}`
  - `wow_world_position_status_total{result}`
  - `wow_world_geometry_area_entry_*{source}`
  - `wow_world_geometry_wmo_area_*{source}`
  - `wow_world_geometry_area_flag_*{source}`
  - `wow_world_geometry_native_area_info_*{status}`
  - `wow_world_geometry_native_area_flag_*{status}`
  - `wow_world_geometry_lookup_results_total{result}`
- Steady-state capture:
  `logs/perf-rca/20260518-234640-500-same-grid-50ms-5-mage-sentinels-attribution-steady.summary.prom`
- Post-run aggregate capture:
  `logs/perf-rca/20260518-234928-500-same-grid-50ms-5-mage-sentinels-attribution-postrun.summary.prom`
- Harness summary:
  `clients=500`, `failures=5`, `movements_sent=570264`,
  `packets_drained=6130460`; sentinel result `casts_sent=89`,
  `responses=89`, `failures=45`, `avg_response_ms=653.853`,
  `max_response_ms=1160.594`.
- Steady scrape reached `500` connected sessions and `499` active players.
  `CMSG_CAST_SPELL` service average/max over the 1m window were
  `569.275/780.043 ms`.
- Outbound byte attribution at steady scrape:
  - `SMSG_UPDATE_OBJECT` (`0x00A9`) dominated: `77,716,311` queued bytes and
    `77,709,747` written bytes.
  - `MSG_MOVE_HEARTBEAT` (`0x00EE`) was second: `10,196,634` queued bytes and
    `10,191,174` written bytes.
  - Other movement opcodes were much smaller:
    `0x00B5` `2.2 MB`, `0x00DA` `1.9 MB`, `0x00BB` `1.5 MB`,
    `0x00C9` `0.78 MB`.
  - Movement broadcast fanout averaged roughly `153-160` recipients and maxed
    around `224-226`.
- Area lookup attribution at steady scrape:
  - `wow_world_position_status_total{result="attempted"} = 2786`
  - `area_entry` average/max `18.212/355.866 ms`
  - WMO area average/max `17.956/355.860 ms`
  - ADT area-flag average/max `0.252/38.467 ms`
  - native WMO area info `not_found` average/max `17.891/355.859 ms`
  - all resolved area entries went through `area_entry_area_flag_found`, so
    this Northshire control is mostly paying expensive WMO misses before a
    cheap ADT area flag succeeds.

Outbound source attribution added and control rerun:

- Code now also emits:
  - `wow_world_outbound_source_packets_total{source,opcode}`
  - `wow_world_outbound_source_bytes_total{source,opcode}`
- Useful steady scrape:
  `logs/perf-rca/20260519-000158-500-same-grid-50ms-source-attribution-steady2.summary.prom`
- Harness summary:
  `clients=500`, `failures=5`, `movements_sent=560487`,
  `packets_drained=5296001`; sentinel result `casts_sent=89`,
  `responses=89`, `failures=44`, `avg_response_ms=815.632`,
  `max_response_ms=1606.316`.
- Steady scrape reached `500` connected sessions and `500` active players.
  `CMSG_CAST_SPELL` service average/max over the 1m window were
  `455.831/1184.130 ms`.
- Top steady-state source/opcode byte families:
  - `movement_apply` / `0x00EE` (`MSG_MOVE_HEARTBEAT`): `106,284,955`
    bytes
  - `player_visibility_refresh` / `0x00A9` (`SMSG_UPDATE_OBJECT`):
    `56,651,147` bytes
  - `player_add_visibility` / `0x00A9`: `55,911,280` bytes
  - `movement_apply` movement opcodes:
    `0x00BB` `13.3 MB`, `0x00C9` `9.0 MB`, `0x00B5` `8.5 MB`,
    `0x00DA` `8.0 MB`, `0x00B7` `7.2 MB`
- Postrun-minus-steady source deltas show ongoing movement pressure is
  dominated by `movement_apply`, especially `MSG_MOVE_HEARTBEAT`.
  `player_add_visibility` is mostly startup/login visibility cost;
  `player_visibility_refresh` remains the ongoing `SMSG_UPDATE_OBJECT`
  producer.
- Session-writer batching was tried as a fix experiment and rejected:
  - steady scrape:
    `logs/perf-rca/20260519-001018-500-same-grid-50ms-writer-batch-steady.summary.prom`
  - harness result worsened to `avg_response_ms=890.854`,
    `max_response_ms=1618.473`
  - the code path was reverted; keep the source-attribution metrics, but do
    not pursue writer batching as the first fix.

First producer-side movement coalescing experiment:

- Code now coalesces stale observer broadcasts for `movement_apply`
  `MSG_MOVE_HEARTBEAT` (`0x00EE`) to at most once per `100 ms` per mover.
  The server still accepts every movement packet and updates authoritative
  player state. Non-heartbeat movement packets still broadcast immediately.
- New regression test:
  `map_runtime_coalesces_stale_heartbeat_broadcasts_to_observers`.
- Active steady scrape:
  `logs/perf-rca/20260519-003425-500-same-grid-50ms-5-mage-sentinels-heartbeat-coalesce100-active-steady.summary.prom`
- Harness summary:
  `clients=500`, `failures=2`, `movements_sent=575564`,
  `packets_drained=3219278`; sentinel result `casts_sent=89`,
  `responses=89`, `failures=45`, `avg_response_ms=948.112`,
  `max_response_ms=1635.198`.
- The scrape reached `500` connected sessions and `499` active players.
  `CMSG_CAST_SPELL` service average/max were `742.636/1354.926 ms`.
- The intended outbound bucket dropped: `movement_apply` / `0x00EE` was
  `12,995,803` bytes in the active steady scrape, and
  `player_movement_broadcast` fanout for `0x00EE` averaged `87.408`
  recipients. `packets_drained` also fell to `3.2M`.
- Spell latency did not improve, so heartbeat coalescing is a useful volume
  reduction but not the complete root fix. The next evidence gap is inside or
  around `CMSG_CAST_SPELL` service time: map lock wait, spell-handler stages,
  remaining `SMSG_UPDATE_OBJECT` churn, and terrain/area lookup.

VMap tile-load cache guard:

- Static CMaNGOS comparison showed that `TerrainInfo::LoadMapAndVMap` checks
  `IsTileLoaded(map, x, y)` before calling `loadMap(...)`.
- The Rust native bridge did not have that guard in hot height, liquid, area,
  and LOS paths; repeated movement-position status work could reach
  `VMapManager2::loadMap(...)` under the global native bridge mutex.
- Fixed by adding `wow_vmap_ensure_tile_loaded(...)` in
  `crates/wow-network/native/vmap_bridge.cpp` and using it from:
  - `crates/wow-network/native/map_height.cpp`
  - `crates/wow-network/native/vmap_los.cpp`
- Active-polled post-fix control:
  `logs/perf-rca/20260519-010653-500-same-grid-50ms-5-mage-sentinels-vmap-cache-guard-active-steady.summary.prom`
- Harness summary:
  `clients=500`, `failures=20`, `movements_sent=241409`,
  `packets_drained=21814770`; sentinel result `casts_sent=89`,
  `responses=89`, `failures=46`, `avg_response_ms=80.677`,
  `max_response_ms=310.921`.
- The scrape reached `500` connected sessions and `500` active map players.
  `CMSG_CAST_SPELL` service average/max were `245.803/1088.052 ms`.
- The core geometry metric collapsed from about `23 ms` native area-info
  average in the previous control to `0.007 ms` found / `0.002 ms` not-found.
  Movement actor queue age dropped from `90.295 ms` average / `324.114 ms` max
  to `0.589 ms` average / `63.432 ms` max.
- This moves native vmap repeated loading from "secondary hypothesis" to
  "confirmed contributor fixed." The control is not a clean scalability pass
  because of thin-client failures, but it is strong RCA evidence.

Player visibility relocation threshold:

- CMaNGOS reference: `Unit::OnRelocated` only calls
  `UpdateObjectVisibility()` after movement exceeds
  `Visibility.RelocationLowerLimit`, default `10` yards, from
  `m_last_notified_position`.
- The Rust map-owned movement path was marking every accepted movement packet
  for player-player visibility refresh. That made the
  `player_visibility_refresh` phase rebuild create/destroy visibility at the
  `50 ms` movement-packet cadence.
- Fixed by adding
  `PlayerRuntime::last_player_visibility_refresh_position` and only marking
  player-player visibility refreshes after `10` yards of relocation.
- Regression test added:
  `map_runtime_skips_player_visibility_refresh_below_relocation_limit`.
- Active-polled post-fix control:
  `logs/perf-rca/20260519-011936-500-same-grid-50ms-5-mage-sentinels-player-vis-relocation10-active-steady.summary.prom`
- Harness summary:
  `clients=500`, `failures=20`, `movements_sent=258904`,
  `packets_drained=23672092`; sentinel result `casts_sent=89`,
  `responses=88`, `failures=44`, `pending=1`, `avg_response_ms=39.000`,
  `max_response_ms=374.827`.
- The scrape reached `500` connected sessions and `498` active map players.
  `CMSG_CAST_SPELL` service average/max were `81.326/419.431 ms`.
- Intended producer drop versus the vmap-cache-guard control:
  - `player_visibility_refresh/0x00A9` bytes:
    `130,241,624 -> 1,852,928`
  - `player_visibility_refresh/0x00A9` packets:
    `187,980 -> 2,674`
  - refresh players per sample:
    `163.870 avg / 494 max -> 0.393 avg / 6 max`
  - refresh packets per sample:
    `1748.995 avg / 9834 max -> 13.057 avg / 224 max`
- Remaining largest outbound source is now movement broadcasts from
  `movement_apply`, especially `0x00EE`; `player_visibility_refresh` is no
  longer the main ongoing update-object churn source.

## Tests Run

- Current spell/control slice:
  - `cargo test -p wow-network hard_control --lib`
  - `cargo test -p wow-network polymorph --lib`
  - `cargo test -p wow-network confused_creature --lib`
  - `cargo test -p wow-network db_creature_polymorph_uses_no_pve_diminishing_group --lib`
  - `cargo test -p wow-network db_creature_evade_removes_polymorph_aura_display_and_diminishing_tracker --lib`
  - `cargo test -p wow-network failed_hostile_aura_rank_cast_still_pulls_db_creature_aggro --lib`
  - `cargo test -p wow-network resisted_hostile_aura_spell_sends_miss_without_applying_aura_and_pulls_aggro --lib`
  - `cargo test -p wow-network resisted_hostile_direct_damage_spell_sends_go_miss_without_damage_or_miss_log --lib`
  - `cargo test -p wow-network resisted_damage_plus_aura_spell_skips_damage_and_aura_from_same_target_outcome --lib`
  - `cargo test -p wow-network item_hostile_damage_spell --lib`
  - `cargo test -p wow-network --lib -- --test-threads=1`
  - `cargo check -p worldserver`
  - `cargo test -p wow-network polymorphed_creature_keeps_confused_wandering_while_in_combat --lib`
  - `cargo test -p wow-network rooted_polymorph_does_not_start_confused_wandering_until_root_ends --lib`
  - `cargo test -p wow-network db_creature_random_motion_is_blocked_by_root --lib`
  - `cargo check -p worldserver`
  - `.\scripts\test-rust.cmd`
- `cargo fmt`
- `cargo test -p wow-network enqueue_pending_movement_replaces_older_packet --lib`
- `cargo test -p wow-network pending_movement_timeout_uses_coalesce_deadline --lib`
- `cargo test -p wow-network pending_movement_due_only_after_deadline --lib`
- `cargo test -p wow-network player_position_status_update_is_throttled --lib`
- `cargo test -p wow-network movement_packets_skip_immediate_gameplay_sync --lib`
- `cargo test -p wow-network map_runtime_defers_player_visibility_enter_until_refresh_phase --lib`
- `cargo test -p wow-network map_runtime_visibility_refresh_keeps_earliest_old_position_across_multiple_moves --lib`
- `cargo test -p wow-network map_runtime_manager_movement_actor_matches_direct_path_packets --lib`
- `cargo test -p wow-network map_runtime_player_movement_preserves_db_creature_visibility_set --lib`
- `cargo test -p wow-network movement_packets_skip_pre_dispatch_session_refresh --lib`
- `cargo test -p wow-network movement_packets_skip_pending_spell_checks_without_active_spells --lib`
- `cargo test -p wow-network active_spells_keep_pending_spell_checks_enabled_for_movement --lib`
- `cargo test -p wow-network dashboard_renders_live_metrics_page --lib`
- `cargo test -p wow-network prometheus_render_includes_histogram_and_opcode_labels --lib`
- `cargo test -p wow-network map_runtime_manager_movement_actor_matches_direct_path_packets --lib`
- `cargo test -p world-load-test movement_phase_jitter_is_deterministic_and_bounded`
- `cargo check -p worldserver`
- `cargo check -p world-load-test`
- `.\scripts\test-rust.cmd`
- PowerShell parser check for `scripts/capture-rca-metrics.ps1`
- `cargo test -p wow-network session_registry_requests_disconnect_when_bounded_queue_is_full --lib`
- Final `.\scripts\test-rust.cmd` after queue-metric wiring
- Control load run:
  `.\scripts\start-thin-client-load.ps1 -ClientCount 500 -SpawnMode local_radius -MoveIntervalMs 50 -MovePhaseJitterMs 0 -LoginStaggerMs 1 -HoldSeconds 90 -EnableMovementActor`
  captured metrics successfully, but harness exited with `2` client failures.
- `cargo test -p world-load-test`
- `cargo check -p world-load-test`
- `cargo run -p world-load-test -- --help`
- `.\scripts\test-rust.cmd`
- Tiny live sentinel smoke:
  `powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\start-thin-client-load.ps1 -ClientCount 2 -HoldSeconds 10 -MoveIntervalMs 500 -LoginStaggerMs 10 -CharacterClass 8 -Race 1 -SentinelCastClients 1 -SentinelCastSpellId 168 -SentinelCastIntervalMs 3000 -EnableMovementActor`
- After adding stationary/desync sentinel options:
  - `cargo test -p world-load-test`
  - `cargo check -p world-load-test`
  - `cargo build --release -p world-load-test`
  - `cargo run -p world-load-test -- --help`
- live launch of stationary mage sentinels:
    `target\release\world-load-test.exe --client-count 5 --hold-seconds 900 --spawn-mode local_radius --center-x -8949 --center-y -132 --center-z 83.5 --radius 6 --move-radius 0 --race 1 --class 8 --sentinel-cast-clients 5 --sentinel-cast-spell-id 168 --sentinel-cast-interval-ms 5000 --sentinel-cast-phase-jitter-ms 5000 --disable-movement`
- Full 500-client stationary-sentinel control:
  `powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\start-thin-client-load.ps1 -ClientCount 500 -SpawnMode local_radius -MoveIntervalMs 50 -MovePhaseJitterMs 0 -LoginStaggerMs 1 -HoldSeconds 90 -CharacterClass 8 -Race 1 -SentinelCastClients 5 -SentinelCastSpellId 168 -SentinelCastIntervalMs 5000 -SentinelCastPhaseJitterMs 5000 -DisableSentinelMovement -EnableMovementActor`
- Metrics capture during the full 500-client stationary-sentinel control:
  `powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\capture-rca-metrics.ps1 -Scenario "500-local-radius-50ms-jitter0-actor-on-5-stationary-mage-sentinels"`
- Post-run metrics capture after the harness crash:
  `powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\capture-rca-metrics.ps1 -Scenario "500-local-radius-50ms-jitter0-actor-on-5-stationary-mage-sentinels-postrun"`
- Harness crash fix verification:
  - `cargo test -p world-load-test`
  - `cargo check -p world-load-test`
  - `cargo build --release -p world-load-test`
  - `cargo run -p world-load-test -- --help`
  - direct `target\release\world-load-test.exe` run with `500` clients,
    `20s`, `5` stationary mage sentinels
  - direct `target\release\world-load-test.exe` run with `500` clients,
    `90s`, `5` stationary mage sentinels, `MaxAttempts=1`
- Runtime metrics / control setup:
  - `cargo check -p worldserver`
  - `cargo test -p wow-network prometheus_render_includes_histogram_and_opcode_labels --lib`
  - PowerShell parser check for `scripts/capture-rca-metrics.ps1` and
    `scripts/start-thin-client-load.ps1`
  - `RUSTFLAGS="--cfg tokio_unstable" cargo check -p worldserver`
  - `cargo check -p world-load-test`
  - full `500`-client stationary-sentinel control with
    `-EnableTokioUnstableMetrics`, captured at steady state
- Scalability matrix:
  - PowerShell parser check for `scripts/run-rca-scalability-matrix.ps1`
  - smoke matrix: `50` clients, movement same-grid sync, `20s` hold
  - core matrix: `50`, `100`, `250`, `500` clients across idle same-grid,
    movement same-grid sync, movement same-grid jitter250, and movement spread
    sync
  - rate-knee matrix: `500` clients, same-grid movement at `250 ms` and
    `500 ms`
  - WPR CPU profile attempt during a long `500` same-grid `50 ms` run; WPR was
    blocked by Windows profiling policy/privilege, but matching RCA metrics
    were captured
- Source-attribution control:
  - `cargo fmt`
  - `cargo check -p worldserver`
  - `cargo test -p wow-network prometheus_render_includes_histogram_and_opcode_labels --lib`
  - full `500`-client stationary-sentinel control with outbound
    source/opcode metrics, captured at steady state and postrun
  - session-writer batching control, captured at steady state; batching was
    reverted after the control worsened spell latency
  - post-revert validation:
    `cargo check -p worldserver` and
    `cargo test -p wow-network prometheus_render_includes_histogram_and_opcode_labels --lib`
  - final `.\scripts\test-rust.cmd`
- Current item target-outcome slice:
  - `cargo fmt`
  - `cargo test -p wow-network item_hostile_damage_spell --lib`
  - `cargo test -p wow-network resisted_hostile_aura_spell_sends_miss_without_applying_aura_and_pulls_aggro --lib`
  - `cargo test -p wow-network resisted_hostile_direct_damage_spell_sends_go_miss_without_damage_or_miss_log --lib`
  - `cargo test -p wow-network resisted_damage_plus_aura_spell_skips_damage_and_aura_from_same_target_outcome --lib`
  - `cargo test -p wow-network prometheus_render_includes_histogram_and_opcode_labels --lib -- --nocapture`
  - `cargo test -p wow-network --lib -- --test-threads=1`
  - `cargo check -p worldserver`
  - `.\scripts\test-rust.cmd` was attempted; it failed in the parallel
    `wow-network` lib run on
    `observability::tests::prometheus_render_includes_histogram_and_opcode_labels`
    because the rendered global counter did not contain
    `wow_player_environment_geometry_checks_total 1`. The same test passed
    when isolated, and the full `wow-network` lib suite passed serially, so
    this is currently classified as global observability test-order sensitivity
    rather than a spell regression.
- Current active-cast movement-interrupt slice:
  - `cargo fmt`
  - `cargo test -p wow-network movement_interrupt --lib`
  - `cargo test -p wow-network movement_does_not_interrupt --lib`
  - `cargo test -p wow-network moving_during_cast_time_interrupts_spell_before_damage_or_power_spend --lib`
  - `cargo check -p worldserver`
  - `cargo test -p wow-network --lib -- --test-threads=1`
  - `.\scripts\test-rust.cmd`
- Current active-cast damage interrupt/pushback slice:
  - `cargo test -p wow-network map_owned_active_cast_damage --lib`
  - `cargo test -p wow-network map_owned_active_cast_without_damage_flags_ignores_damage_interrupt --lib`
  - `cargo test -p wow-network damage_pushback --lib`
  - `cargo test -p wow-network damage_cancels --lib`
  - `cargo fmt`
  - `cargo check -p worldserver`
  - `cargo test -p wow-network --lib -- --test-threads=1`
  - `.\scripts\test-rust.cmd`
- Current active-cast hard-control invalidation slice:
  - `cargo test -p wow-network applying_hard_control_aura_interrupts_active_player_cast --lib`
  - `cargo test -p wow-network applying_silence_and_pacify_only_interrupt_matching_existing_casts --lib`
  - `cargo test -p wow-network hard_control --lib`
  - `cargo fmt`
  - `cargo check -p worldserver`
  - `cargo test -p wow-network --lib -- --test-threads=1`
  - `.\scripts\test-rust.cmd`
- Current active-cast lifecycle cleanup slice:
  - `cargo fmt`
  - `cargo test -p wow-network player_death_clears_active_spell_channels_and_dynamic_objects --lib`
  - `cargo test -p wow-network removing_player_clears_spell_channels_and_notifies_observers --lib`
  - `cargo test -p wow-network disconnect_in_combat --lib`
  - `cargo test -p wow-network active_cast --lib`
  - `cargo test -p wow-network movement_interrupt --lib`
  - `cargo test -p wow-network hard_control --lib`
  - `cargo check -p worldserver`
- Current active-cast teleport invalidation slice:
  - test-first failure confirmed
    `near_teleport_position_set_clears_active_spell_runtime` left active casts
    alive before implementation
  - `cargo fmt`
  - `cargo test -p wow-network near_teleport_position_set_clears_active_spell_runtime --lib`
  - `cargo test -p wow-network regular_movement_position_update_preserves_non_movement_interrupt_cast --lib`
  - `cargo test -p wow-network teleport --lib`
  - `cargo test -p wow-network active_cast --lib`
  - `cargo test -p wow-network movement_interrupt --lib`
  - `cargo test -p wow-network hard_control --lib`
  - `cargo check -p worldserver`
- Current active-cast aura/target invalidation slice:
  - test-first failure confirmed
    `removing_channeled_creature_aura_interrupts_player_channel` left the
    channel alive before implementation
  - test-first failure confirmed
    `creature_target_death_interrupts_active_player_spell_work_targeting_it`
    left active spell work alive before implementation
  - `cargo fmt`
  - `cargo test -p wow-network removing_channeled_creature_aura_interrupts_player_channel --lib`
  - `cargo test -p wow-network creature_target_death_interrupts_active_player_spell_work_targeting_it --lib`
  - `cargo test -p wow-network deleting_creature_target_interrupts_active_player_spell_work_targeting_it --lib`
  - `cargo test -p wow-network active_cast --lib`
  - `cargo test -p wow-network channel --lib`
  - `cargo test -p wow-network death --lib`
  - `cargo test -p wow-network movement_interrupt --lib`
  - `cargo test -p wow-network hard_control --lib`
  - `cargo check -p worldserver`
- Heartbeat coalescing:
  - `cargo test -p wow-network map_runtime_coalesces_stale_heartbeat_broadcasts_to_observers --lib`
  - `cargo check -p worldserver`
  - `cargo test -p wow-network map_runtime_manager_movement_actor_matches_direct_path_packets --lib`
  - `cargo test -p wow-network map_runtime_broadcasts_stop_with_final_idle_orientation --lib`
  - final `.\scripts\test-rust.cmd`
  - active-polled `500`-client stationary-sentinel control captured with
    `-EnableTokioUnstableMetrics`
- VMap tile-load cache guard:
  - `cargo check -p worldserver`
  - `cargo test -p wow-network db_creature_vmap_los_uses_local_cmangos_data_when_available --lib`
  - `cargo test -p wow-network terrain_height_uses_local_cmangos_map_data_when_available --lib`
  - final `.\scripts\test-rust.cmd`
  - `cargo build --release -p authserver -p worldserver -p world-load-test`
  - active-polled `500`-client stationary-sentinel control captured with
    `-EnableTokioUnstableMetrics`
- Player visibility relocation threshold:
  - `cargo test -p wow-network map_runtime_skips_player_visibility_refresh_below_relocation_limit --lib`
  - `cargo test -p wow-network map_runtime_visibility_refresh_keeps_earliest_old_position_across_multiple_moves --lib`
  - `cargo test -p wow-network map_runtime_defers_player_visibility_enter_until_refresh_phase --lib`
  - `cargo check -p worldserver`
  - `cargo build --release -p authserver -p worldserver -p world-load-test`
  - active-polled `500`-client stationary-sentinel control captured with
    `-EnableTokioUnstableMetrics`
- 2000-player RCA pressure-source fixes:
  - `cargo fmt`
  - `cargo check -p worldserver`
  - `cargo test -p wow-network map_runtime_disconnect_in_combat_lingers_before_removal --lib`
  - `cargo test -p wow-network db_creature_return_home_motion_advances_without_active_combat --lib`
  - `cargo test -p wow-network dashboard_renders_live_metrics_page --lib`
- Return-home / sight-aggro pressure-source fixes:
  - `cargo fmt`
  - `cargo check -p worldserver`
  - `cargo test -p wow-network map_runtime_sight_aggro_uses_cell_buckets_and_detection_range --lib`
  - `cargo test -p wow-network map_runtime_sight_aggro_is_throttled_until_player_moves_enough --lib`
  - `cargo test -p wow-network map_runtime_tick_advances_return_home_motion_without_session_polling --lib`
  - `cargo test -p wow-network db_creature_return_home_motion_advances_without_active_combat --lib`
- OOC EventAI re-enabled:
  - `cargo fmt`
  - `cargo check -p worldserver`
  - `cargo test -p wow-network ooc_event_ai --lib`
- Dialogue branch merge:
  - `cargo test -p wow-network gossip --lib`
  - `cargo check -p worldserver`
  - `.\scripts\test-rust.cmd`
- Local starter DB startup fix:
  - `cargo fmt`
  - `cargo check -p worldserver`
  - `.\scripts\restart-game-stack.cmd --release`
  - `.\scripts\test-rust.cmd` hit the known parallel
    `prometheus_render_includes_histogram_and_opcode_labels` observability
    counter-order failure
  - isolated `cargo test -p wow-network prometheus_render_includes_histogram_and_opcode_labels --lib -- --nocapture`
  - `cargo test -p wow-db --lib`
  - `cargo test -p wow-network gossip --lib`
- Account seed fix:
  - `cargo run -p auth-flow-test`
  - PowerShell parser check for `scripts/run-client-stack-18085.ps1`
  - DB sanity check confirmed `RUSTAUTH` exists and has starter characters.
- Empty-world live DB repair:
  - `powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\import-classic-db-world.ps1`
  - `.\scripts\restart-game-stack.cmd --release`
  - DB sanity counts for full world tables and Northshire creature/gameobject
    rows
- GM account enablement:
  - live DB `UPDATE account SET gmlevel=3 WHERE username='RUSTAUTH'`
  - PowerShell parser check for `scripts/run-client-stack-18085.ps1`
- Dialogue template service backing:
  - `cargo fmt`
  - `cargo test -p wow-db --lib`
  - `cargo test -p wow-network gossip --lib`
  - `cargo check -p worldserver`
  - `.\scripts\test-rust.cmd`
  - `.\scripts\restart-game-stack.cmd --release`
  - DB sanity check confirmed representative template-backed vendors/trainers
    such as Corina Steele, Jessara Cordell, Mogwah, and World Mage Trainer have
    service rows through CMaNGOS template tables.
- Trainer gossip diagnostics:
  - DB sanity check confirmed nearby Northshire class trainers such as Khelden
    Bremen and Llane Beshere use gossip-menu trainer options backed by direct
    `npc_trainer` rows; nearby weapon masters use template rows.
  - Live metrics before the second diagnostic restart showed
    `CMSG_GOSSIP_HELLO=52`, `SMSG_GOSSIP_MESSAGE=52`, and no
    `CMSG_GOSSIP_SELECT_OPTION`, pointing at the menu presented to the client.
  - Live mage attempt against Khelden Bremen showed Rust sending menu `4660`,
    `text_id=538`, `options=2`, but still no `CMSG_GOSSIP_SELECT_OPTION`.
    Follow-up DB RCA showed `538/539` are in `npc_text_broadcast_text`, not
    `npc_text`, and point to `broadcast_text` rows `2502/2503`. Rust now treats
    that overlay as CMaNGOS gossip text backing instead of classifying the rows
    as missing.
  - `cargo fmt`
  - `cargo test -p wow-network gossip --lib`
  - `cargo test -p wow-network trainer --lib`
  - `cargo check -p worldserver`
  - `.\scripts\restart-game-stack.cmd --release`
  - `.\scripts\test-rust.cmd`
  - Added gossip hello / prepared-menu logging, then reran `cargo fmt`,
    `cargo test -p wow-network gossip --lib`, `cargo check -p worldserver`, and
    `.\scripts\restart-game-stack.cmd --release`.
  - Missing-gossip-text fix validation: `cargo fmt`,
    `cargo test -p wow-network gossip --lib`,
    `cargo test -p wow-network trainer --lib`, `cargo check -p worldserver`,
    and `.\scripts\restart-game-stack.cmd --release`.
  - Broadcast-backed gossip text correction: `cargo fmt`,
    `cargo test -p wow-db --lib`, `cargo test -p wow-network gossip --lib`,
    `cargo check -p worldserver`, and
    `.\scripts\restart-game-stack.cmd --release`.
- Bank branch integration:
  - committed `codex/banking-parity` worktree changes as `bec18507b`
  - merged into `codex/rusty-mangos`
  - added integration glue so DB gossip banker selections call the same banker
    access/open-bank path as `CMSG_BANKER_ACTIVATE`
  - resolved the `UNIT_NPC_FLAG_*` constant conflict by keeping the full
    dialogue-service flag set plus banker's `0x0000_0100`
  - `cargo fmt`
  - `cargo test -p wow-db --lib`
  - `cargo test -p wow-network bank --lib`
  - `cargo test -p wow-network inventory --lib`
  - `cargo test -p wow-network gossip --lib`
  - `cargo check -p worldserver`
  - `.\scripts\test-rust.cmd`
- GM money / bag vendor hotfix:
  - `cargo fmt`
  - `cargo test -p wow-network parses_gm_dot_commands_for_creature_spawn_and_die --lib`
  - `cargo test -p wow-network db_vendor_inventory_uses_cmangos_list_shape --lib`
  - `cargo test -p wow-network vendor --lib`
  - `cargo test -p wow-db --lib`
  - `cargo check -p worldserver`
  - `.\scripts\restart-game-stack.cmd --release`
- Bank-bag icon drag hotfix:
  - `cargo fmt`
  - `cargo test -p wow-network bank_bag --lib`
  - `cargo test -p wow-network autostore_to_bank_bag_icon_selects_first_valid_slot_in_that_bank_bag --lib`
  - `cargo test -p wow-network swap_to_bank_bag_icon_resolves_non_bag_item_to_bag_storage --lib`
  - `cargo test -p wow-network inventory --lib`
  - `cargo test -p wow-network bank --lib`
  - `cargo check -p worldserver`
  - `.\scripts\restart-game-stack.cmd --release`
- Mail branch integration:
  - committed `codex/mail-system-parity` worktree code as `42e985da2`
  - resolved merge conflicts in `crates/wow-network/src/world/opcodes.rs` and
    `crates/wow-network/src/world/packets.rs` by keeping both bank and mail
    opcode/packet additions
  - `cargo fmt`
  - `cargo check -p worldserver`
  - `cargo test -p wow-network mail --lib`
  - `cargo test -p wow-db --lib`
  - `cargo test -p wow-network bank --lib`
- Mail money delivery hotfix:
  - money-only player mail now uses zero delivery delay; item/COD/other player
    mail keeps the default one-hour delay
  - `cargo fmt`
  - `cargo test -p wow-network mail --lib`
  - `cargo check -p worldserver`
- Mailbox open disconnect fix:
  - fixed DB decoding for signed `mail.stationery` rows
  - `cargo fmt`
  - `cargo test -p wow-db --lib`
  - `cargo test -p wow-network mail --lib`
  - `cargo check -p worldserver`
  - `.\scripts\restart-game-stack.cmd --release`
- World layout cleanup:
  - renamed `map_runtime/map.rs` to `map_runtime/state.rs`
  - renamed `map_runtime/map/*` extension modules to `map_runtime/systems/*`
  - added `crates/wow-network/src/world/README.md`
  - split the former giant `crates/wow-network/src/world/tests.rs` into
    topic-oriented files under `crates/wow-network/src/world/tests/`, with
    shared helpers in `support.rs`
  - split spell cast orchestration, shared spell definitions, effect-value
    scaling, and skill-backed spell helpers out of
    `crates/wow-network/src/world/spells.rs` into focused
    `crates/wow-network/src/world/spells/{casting,definitions,values,skills}.rs`
  - removed empty top-level CMaNGOS scaffold husks `world/maps/` and
    `world/movement/`, and collapsed the half-real `world/motion/` folder into
    live `world/motion.rs`; future CMaNGOS mappings now live in
    `world/PARITY_LAYOUT.md` instead of empty source files
  - split the former giant
    `crates/wow-network/src/world/map_runtime/map_manager.rs` into
    `crates/wow-network/src/world/map_runtime/map_manager/{mod,spells,players,grids,creatures,ticks}.rs`
    while preserving the existing `MapRuntimeManager` facade and behavior
  - split the remaining large creature manager facade into nested
    `crates/wow-network/src/world/map_runtime/map_manager/creatures/{combat,gameobjects,loot,motion,snapshots,spells}.rs`
  - split the remaining giant spell effect implementation into nested
    `crates/wow-network/src/world/spells/effects/{areas,auras,coverage,damage,dispatch,healing,items,movement,utility}.rs`
    while keeping `spells/effects.rs` as the effect facade
  - moved shared world runtime dependencies out of `world/session.rs` into
    `world/session_runtime.rs`; `world/session.rs` now stays focused on
    per-session mutable state
  - removed the legacy synthetic `Rust Guide` NPC fixture module, constants,
    query/gossip/vendor branches, and fixture-only tests
  - removed the dead session-owned DB-creature combat/spell shim functions from
    `world/combat/aggro.rs`; live creature combat continues through
    map-owned `advance_db_creature_combats_for_victim`
  - `cargo fmt`
  - `cargo check -p worldserver`
  - `cargo test -p wow-network query --lib`
  - `cargo test -p wow-network gossip --lib`
  - `cargo test -p wow-network vendor --lib`
  - `cargo test -p wow-network map_runtime --lib`
  - `cargo test -p wow-network spell --lib`
  - `cargo test -p wow-network motion --lib`
  - `cargo test -p wow-network --lib -- --test-threads=1`
  - `.\scripts\test-rust.cmd`

## Current Confidence

- High that the old standalone OOC due-queue architecture is no longer the
  best explanation for the remaining `1000`-client movement lag.
- High that movement flood currently causes too much per-packet session-side
  work compared with CMaNGOS.
- High that `update_player_position(...)` was still a major hot path because it
  owned player-visibility diffing and creature-interest sync inline.
- Medium-high that the new map-owned `player_visibility_refresh` phase is a
  correct architecture move.
- Medium that this slice alone materially improves the `1000`-client harness;
  the first `500`-client control still shows multi-second packet
  dispatch/service delay and large map tick spikes.
- High that outbound movement/replication fanout is now the leading root-cause
  class for the captured `500` same-grid spell lag: WPR shows the hottest
  resolved async task in `world_session_writer` doing real socket sends.
- High that terrain/area lookup is a second confirmed contributor in the same
  capture: the second-largest resolved branch goes through
  `WorldGeometry::area_entry`, native `wow_map_area_info`, and
  `VMapManager2::loadMap`.
- High that, after the VMap/cache and observability fixes, there is no single
  remaining WPA smoking gun in the `2000` spread capture. The current pressure
  source is distributed per-session map work.
- High that disconnected-player expiration belonged in the map runtime loop,
  not in each session combat tick; this was fixed locally and should remove a
  redundant map-wide player scan under high connected-player counts.
- Medium that conditional `sync_player_gameplay_state(...)` collection updates
  will reduce allocator churn in repeated non-movement/session tick syncs; it
  is low risk but still needs another live `2000` run to quantify.
- High that return-home motion should not be polled from each session; this is
  now map motion tick work.
- Medium-high that sight aggro throttling/dirty gating should reduce repeated
  nearby-cell scans for stationary or tiny-jitter players without delaying
  meaningful movement-triggered aggro by more than the `250 ms` fallback.
- High that repeated native vmap tile loading was a real root-cause
  contributor and is now fixed by the cache guard. The post-fix control reduced
  sentinel spell average from the prior comparable `948.112 ms` to
  `80.677 ms`, and reduced native area-info averages from roughly `23 ms` to
  micro-scale millisecond values.
- High that per-packet player visibility refresh marking was a real
  update-object churn source and is now fixed with the CMaNGOS-shaped
  relocation threshold. The post-fix control reduced
  `player_visibility_refresh/0x00A9` bytes from `130 MB` to `1.85 MB`.
- High that the first attribution pass confirms the concrete split: outbound
  writer volume is dominated by `SMSG_UPDATE_OBJECT`, while position-status
  area discovery is dominated by repeated native WMO area-info misses.
- High that the latest control captures the user's spell-cast lag shape without
  manual casting and includes enough runtime metrics to rule out
  spawn-blocking and Tokio global queue backlog for this specific run.
- High that the scalability trigger is movement pressure, not connected player
  count alone: the `500` idle control stayed at `2.427 ms` average spell
  response while `500` same-grid movement sync rose to `608.477 ms`.
- Medium-high that same-grid AOI contributes to the `500` average, but is not
  the whole root cause because spread movement still has degraded averages and
  multi-second tails.
- OOC EventAI is enabled again after the return-home/sight-aggro ownership
  pass; the next comparison run should measure its map phase instead of
  excluding it.
- High that slower movement intervals alone do not eliminate the lag at `500`
  same-grid players; `250 ms` and `500 ms` movement still averaged
  `676-738 ms` spell responses.
- High that producer-side heartbeat coalescing reduces outbound volume, but it
  is not sufficient to fix spell latency by itself. The latest control dropped
  `movement_apply/0x00EE` traffic and still averaged nearly `1s` spell
  response.
- High that session-writer batching is not the right first fix: the control
  worsened sentinel spell average and did not address producer-side byte
  volume.

## Known Blockers / Unproven Areas

- The currently running live server, if still up from before this change, does
  not include the latest return-home/sight-aggro/OOC EventAI changes until the
  release stack is rebuilt and restarted.
- The teleport/player-visibility fix is unit-test proven but still needs the
  real-client two-player Northshire smoke that originally showed the remote
  pivot/offset symptom.
- The `.go`-specific teleport-path fix is compile/test proven but still needs
  the exact user repro: `.go` away, `.go` back to Northshire, then remote
  turn-in-place and short movement observation from a second client.
- The right-click-turn movement-actor fix is compile/test proven but still
  needs the exact live repro: hold right-click turn with a second client
  observing and confirm the old nearby snap/facing jitter is gone.
- The right-click-turn movement-timestamp fix is compile/test proven and the
  local release stack has been restarted with it, but it still needs the exact
  live repro from a second client observer.
- The right-click-turn session-loop batching fix is compile/test proven and the
  local release stack has been restarted with it, but it still needs the exact
  live repro from a second client observer.
- The right-click-turn `MSG_MOVE_SET_FACING` position-clamp fix is compile/test
  proven and the local release stack has been restarted with it, but it still
  needs the exact live repro from a second client observer.
- Trainer gossip needs one more real-client login and mage trainer click after
  the latest restart. Watch `world-client-18085.log` for Khelden's menu `4660`
  with `text_id=538`, then `Dispatching DB gossip selection` followed by
  `Sending trainer list`.
- Bank integration needs a real-client banker smoke after the next release
  restart: open bank, buy one bank bag slot if DBC prices are available, move
  an item into a bank main slot, move it back, relog, and confirm bank contents
  plus purchased slot count persist.
- The new movement coalescing is compile- and test-proven, but not yet
  benchmark-proven under the thin-client harness.
- The first `500`-client control was not perfectly clean: two clients exhausted
  all attempts.
- The latest runtime-metrics control was not perfectly clean: five clients
  exhausted all attempts, but the harness completed normally and emitted the
  sentinel summary.
- The post-vmap-cache-guard control was also not perfectly clean: twenty
  clients exhausted all attempts. The active scrape still reached `500`
  connected sessions / active players and emitted the sentinel summary, so it
  is useful RCA evidence but not a final scalability acceptance run.
- The post-player-visibility-threshold control also had twenty client
  failures and one pending sentinel cast. It reached an active scrape and is
  useful RCA evidence, but not a final clean scalability acceptance run.
- The core matrix has some client failures in movement scenarios. Every row
  reached steady state and emitted sentinel summaries, so the matrix is useful
  for RCA shape, but exact pass/fail cleanliness is not perfect.
- Local CPU profiling is blocked in a non-elevated Windows shell, but elevated
  WPR works and has produced a useful profile. `flamegraph --pid` still needs
  Windows DTrace; command-mode flamegraph works only elevated and only when it
  launches the process itself.
- `scripts/capture-rca-metrics.ps1` metadata fenced-code formatting was fixed
  after the runtime control capture; the capture's raw/summary metrics are
  intact, but that specific metadata file has malformed fences.
- Long thin-client harness runs previously hit `world-load-test.exe`
  `0xc0000005`. The default `MaxAttempts=3` script-run control now completed
  without an access violation; continue watching for new dumps, but the known
  crash shape is mitigated.

## Recommended Next Task

1. Use the currently restarted local release stack and run a two-client
   Northshire smoke covering `.go` away/back plus stationary and moving
   right-click turn observation from a second client.
2. If the pivot/offset and right-click snap symptoms are gone, update
   `docs/playable_gate_board.md` / multiplayer notes with the real-client proof
   and then return to the trainer-gossip verification.
3. After the user-directed multiplayer proof, resume the prior spell roadmap:
   damage-interrupt coverage, cross-map transfer active-spell cleanup, then the
   broader target-outcome / Polymorph / triggered-spell parity slices.

## Key Files

- `crates/wow-network/src/world/server/session_loop.rs`
- `crates/wow-network/src/world/server/movement.rs`
- `crates/wow-network/src/world/map_runtime/movement_actor.rs`
- `crates/wow-network/src/world/map_runtime/map_manager/mod.rs`
- `crates/wow-network/src/world/map_runtime/map_manager/creatures.rs`
- `crates/wow-network/src/world/map_runtime/map_manager/creatures/combat.rs`
- `crates/wow-network/src/world/map_runtime/map_manager/creatures/motion.rs`
- `crates/wow-network/src/world/map_runtime/map_manager/creatures/spells.rs`
- `crates/wow-network/src/world/map_runtime/map_manager/players.rs`
- `crates/wow-network/src/world/map_runtime/map_manager/spells.rs`
- `crates/wow-network/src/world/map_runtime/map_manager/grids.rs`
- `crates/wow-network/src/world/map_runtime/map_manager/ticks.rs`
- `crates/wow-network/src/world/map_runtime/state.rs`
- `crates/wow-network/src/world/map_runtime/systems/players.rs`
- `crates/wow-network/src/world/spells.rs`
- `crates/wow-network/src/world/spells/effects.rs`
- `crates/wow-network/src/world/spells/effects/damage.rs`
- `crates/wow-network/src/world/spells/effects/auras.rs`
- `crates/wow-network/src/world/spells/effects/items.rs`
- `crates/wow-network/src/world/spells/spell_mgr.rs`
- `crates/wow-network/src/world/session_runtime.rs`
- `crates/wow-network/src/world/map_runtime/systems/creature_motion.rs`
- `crates/wow-network/src/world/combat/evade.rs`
- `crates/wow-network/src/world/tests/mod.rs`
- `docs/performance_rca_runbook.md`
- `scripts/capture-rca-metrics.ps1`
