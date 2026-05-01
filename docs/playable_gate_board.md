# Rust Rewrite Playable Gate Board

This board defines the current real-client playable milestone and the next
engineering bet. Use it as a dashboard, not a cage: user direction can reorder
work when it reduces project risk.

Current milestone: Northshire Human Warrior playable slice
Client: WoW 1.12.1 / build 5875
Branch: codex/rust-auth-foundation

## Rules

- Real-client proof beats harness proof.
- Harness proof is required before real-client proof when practical.
- Prefer tasks that advance one of the gates below or reduce crash/desync risk.
- If a bug or parity gap is discovered, use judgment: fix blockers and safety or
  data-integrity guardrails when practical; log useful follow-ups when they
  should wait.
- G3 is green and remains a regression gate, not a reason to block broader
  world-runtime work.

## Current Priority Order

1. G12 Derisk Multiplayer / Shared MapRuntime.
2. G8 Combat Agency.
3. G9 World Creature Fidelity.
4. G10 NPC Interaction Fidelity.
5. G11 Persistence + Relog Sanity.
6. G5 Combat + Loot real-behavior fidelity.
7. G6 Level + Trainer issue #49 polish.
8. G7 Death + Respawn polish.
9. G8/G9 Pathing + Movement Fidelity follow-ups.

Current user-directed milestone: **G12 Derisk Multiplayer / Shared MapRuntime**.
Keep one monolithic worldserver, but stop treating each TCP session as its own
mini-world. Introduce a shared in-process `MapRuntime` / grid layer inside
`WorldRuntimeState`, then route player visibility, movement, `/say`, and DB
creature state through it. The user has a detailed implementation plan and will
walk the next agent through it before coding. Follow
`docs/g12_shared_mapruntime_plan.md` as the G12 implementation document.

Recently verified:

- G3 Movement Visibility Streaming
## Gate Status

| Gate | Status | Definition of Done | Evidence | Blocking Issues | Next Action |
| --- | --- | --- | --- | --- | --- |
| G1 Login + Character Lifecycle | Green | Fresh account can create Human Warrior, enter world, logout, relog, delete non-loaded char | `test-world-flow`, real-client smoke | None | Keep as regression gate |
| G2 Visual World Sanity | Green | Human start area shows correct NPCs/creatures with correct display IDs/models | Real-client observation | None | Keep as regression gate |
| G3 Movement Visibility Streaming | Green | Walking from Human start to kobold area streams newly nearby creatures without relog and removes out-of-range creatures | Login-radius creature loading now uses CMaNGOS normal continent visibility distance of 100 yards; harness proves movement-triggered DB creature create and destroy updates against RealClassicDb; user verified real-client walking proof | #45 follow-up only if more parity gaps are found | Keep as regression gate |
| G4 Quest Loop | Green-ish | Accept, progress, complete Kobold Camp Cleanup in real client | Harness green; user reports flow looks good | TBD | Keep as regression gate and collect final real-client proof after G3 |
| G5 Combat + Loot | Yellow-Green | Kill, loot, release, respawn starter mobs in real client with CMaNGOS-like corpse/respawn timing | Rust now uses DB `creature.spawntimesecsmin/max`, template `CorpseDecay`, CMaNGOS rank defaults, corpse/dead/alive runtime state, corpse destroy, and respawn create updates. Loot release no longer respawns the creature immediately; the RealClassicDb starter-zone harness proves ten distinct Kobold Vermin quest kills without relying on same-creature instant revive | TBD | Keep as regression gate; remaining G5 work is broader loot/drop fidelity and combat math, not instant respawn |
| G6 Level + Trainer | Yellow | Level up and train Battle Shout, spell appears immediately, persists after relog | Level/trainer flow works; #49 remains visual polish | #49 visual polish | Resolve or explicitly defer #49 after G3/G5 priority |
| G7 Death + Respawn | Yellow-Green | Player can die, release spirit, see/reclaim their corpse or use a spirit healer, resurrect, leave bones, and keep DB state sane | Rust now follows the CMaNGOS death/repop/reclaim/healer path for the starter slice: lethal DB-creature melee sets corpse state and release timer, release sets ghost flag plus spell `8326` aura fields, graveyard selection prefers a nearby spirit healer when local links are incomplete, graveyard creatures stream after release, `MSG_CORPSE_QUERY` points to the stored corpse, reclaim and spirit-healer activation resurrect at 50% health, and ghost players do not aggro mobs. The corpse foundation creates real `TYPEID_CORPSE` / `HighGuid::Corpse` world objects, persists resurrectable corpses in `characters.corpse`, streams nearby player corpses on login/movement, deletes the row on resurrection, and converts the runtime object to `CORPSE_BONES`. User smoke confirmed the core real-client death/release/reclaim flow works. Unit tests cover lethal damage, ghost aura fields, no ghost aggro, spirit-healer flags, corpse query, player corpse create blocks, and bones flag updates; `test-rust.cmd` passes, and elevated `test-starter-zone-flow.cmd` now proves creature-origin death, release, corpse query, corpse object creation, reclaim, bones update, corpse row deletion, ghost flag clearing, restored health, and persisted position against RealClassicDb. | #44 | Keep as a regression gate; remaining G7 work is durability loss, resurrection sickness, corpse/bones expiry, relog-dead/relog-ghost edge checks, and broader multiplayer corpse/bones broadcasting |
| G8 Combat Agency | Yellow | Starter mobs can aggro, chase or enter range, swing, damage the player, and either kill the player or die with CMaNGOS-like combat rules | Harness now proves a RealClassicDb Kobold Vermin streams in, aggro-starts from movement proximity using DB-backed `creature_template.Detection` plus the CMaNGOS level-delta attack-distance shape, sends timed `SMSG_MONSTER_MOVE` chase splines from just outside melee range, advances runtime creature position separately from DB spawn/home position, rechecks/re-paths chase destination on a 250ms CMaNGOS-style cadence even while movement packets are flowing, retains active combat creatures even when their DB spawn point leaves the normal nearby-spawn visibility query, deals creature-origin melee damage only after the motion reaches melee range, and updates player health before the player attacks. Aggro eligibility now uses a CMaNGOS-shaped faction-template reaction bridge over local ClassicDB faction IDs instead of a starter creature entry allowlist: Northshire hostile factions aggro, Young Wolf faction stays neutral, and friendly NPC factions stay friendly. Aggro, chase, creature melee reach, and player DB-creature melee hits now pass through explicit range/facing/navigation guardrails; Rust now inspects configured `maps`/`vmaps`/`mmaps` at startup, gates DB-creature path availability on CMaNGOS-style mmap tile presence, and can ask a native Detour bridge for multi-point mmap paths when local generated mmap data covers the start and target tiles. Far player right-click no longer starts creature retaliation before a valid landed hit, far starter melee spells fail before power spend/spell-go/damage, creature aggro now toggles the client-facing in-combat unit flag instead of requiring a player right-click, chase splines now use the CMaNGOS `MonsterMoveFacingTarget` shape, in-range bad-facing swings now publish an in-place facing-target turn before retrying, and multiple nearby hostile DB creatures can own combat/chase state at the same time. A first CMaNGOS-shaped leash/evade/home slice uses the default 30-yard leash radius, clears combat, resets creature health, and starts timed return-home motion; chase stop/re-path now uses the CMaNGOS combined 5-yard melee reach floor and half-range chase destination, trimming Detour paths to that stop distance. Aggro can now call same-faction nearby hostile assists once using `CallForHelp` or the CMaNGOS 5-yard default radius. DB-creature melee now uses a first CMaNGOS-shaped melee outcome path for creature swings: min/max template damage rolls, armor-reduction helper, miss/dodge/parry/block/glancing/crit/crushing outcome ordering, and `SMSG_ATTACKERSTATEUPDATE` serialization for non-hit and blocked outcomes. Player auto-attacks now use equipped main-hand damage plus class/stat attack-power scaling, and successful swings schedule the next auto-swing from the equipped main-hand delay. Real-client observation confirmed Young Wolf is neutral, Defias Thug should aggro, chase response is faster, and pre-mmap-bridge terrain/pathing was still glitchy/non-parity | #12, #50 | Continue G8 combat fidelity: player offensive outcome rolls, live armor/block stats, exact reach/model modifiers, swing error packets, vmap LOS, then full PathFinder smoothing |
| G9 World Creature Fidelity | Yellow | Starter mobs come from real DB spawn/template data, expose DB loot, persist enough world state, respawn with CMaNGOS-like timing, and support generic DB-backed idle/random/waypoint/patrol movement | DB spawn/template/loot basics work; Rust now loads DB `MovementType`/`spawndist` and runs generic random walk splines with CMaNGOS-like 3-10 second pauses for random-movement creatures. Rust also loads CMaNGOS-style `creature_movement` GUID paths, `creature_movement_template` entry/path 0 fallback, and spawn-group formation `waypoint_path` indirection for `MovementType` 2/4, sends timed multi-point patrol `SMSG_MONSTER_MOVE` splines, waits at DB nodes, supports linear back-and-forth waypoint movement, and keeps DB creatures in alive/corpse/dead/respawn state with DB/template-derived timers. Creature deaths now write CMaNGOS-shaped `characters.creature_respawn` rows for instance `0`; login/movement visibility restores future-dead creatures as tracked runtime state without creating them client-side, unloaded corpses are recreated as corpses when the player returns before respawn, and runtime respawn clears the row. True pathfinder random points, follower formation movement, broader real-client zone proof, and remaining multi-client polish remain missing | #51, #52 | Continue after G8 if movement/world fidelity is the highest remaining blocker; Northshire is proof only, not a source of starter-specific creature logic |
| G10 NPC Interaction Fidelity | Red | Quest givers, vendors, trainers, gossip NPCs, and non-interactive NPCs expose the correct cursor/status, menus, flags, text, and failure behavior in the real client | Real-client NPC interaction pass plus harness | TBD | Audit Northshire NPC flags/status/menu flows against CMaNGOS |
| G11 Persistence + Relog Sanity | Red | After quest progress, XP, level-up, loot, inventory changes, trainer learning, death/respawn, and position changes, logout/relog restores correct state with no dupes/loss/corruption | Harness plus real-client relog checklist | TBD | Add relog checkpoints after each major Northshire action |
| G12 Derisk Multiplayer / Shared MapRuntime | Red / active | Two clients can log into Northshire, see each other spawn/move/logout, exchange nearby `/say`, observe shared DB creature state, and avoid duplicated/desynced kill or loot state | Harness now proves two clients can log in together, receive mutual player create blocks, receive movement broadcast, get destroy when the other player leaves visibility range, receive create again on return, observe logout destroy, and exchange nearby `/say` without leaking it to an out-of-range player. The starter-zone harness now also runs two simultaneous Northshire clients through shared wolf combat: the observer sees the primary player, movement, `/say`, shared wolf damage/death state, and cannot duplicate loot after the primary claims the corpse. User real-client smoke confirmed movement with three players online. Player-player visibility now uses CMaNGOS-shaped grid/cell buckets instead of full player scans. `MapRuntime` now preserves shared DB-creature snapshots across sessions, applies player melee and supported starter spell creature damage through shared map events, broadcasts shared creature health/death updates to nearby sessions, owns DB-creature loot open/money/item/release claims, owns exclusive DB-creature combat claims, is authoritative for active creature combat attacker/victim plus retry timing and victim-wide cleanup on death, applies creature-origin melee damage to the shared player snapshot before victim-session packet sends, advances DB-creature corpse expiry/respawn through shared map lifecycle events, dispatches creature combat-start/chase/facing/evade/return-home packets to nearby observer sessions, and gates DB respawn persistence plus killer quest credit/XP/final attack-stop/combat-flag cleanup behind a `MapRuntime` death-finalization event. A real-client shared-mob bug where observers could keep ticking stale local patrol/chase state after another player killed the mob is fixed by syncing session-local creatures from shared map snapshots before creature ticks and by broadcasting a death-time motion stop. A follow-up patrol regression is fixed by writing random/waypoint/return-home motion back into `MapRuntime` and broadcasting idle motion starts to nearby observers; exact 5-yard melee reach is now accepted. Other-player create blocks now carry movement timing and visible equipped item fields, equipment changes broadcast visible item updates to nearby players, equip/unequip refreshes the local player's derived combat-stat fields, resurrection refreshes shared map player health, and player attack/spell packets now broadcast `SMSG_ATTACKSTART`, `SMSG_SPELL_GO`, and attacker-state updates to observers. DB creatures now lazy-load by CMaNGOS-shaped grid rectangle into shared `MapRuntime` cell buckets, login/movement visibility stages nearby creatures from loaded map state instead of DB radius queries, and grid-load counters/logs expose actual DB rectangle loads. | Grid unload/idle eviction, loot-flag observer polish after claims, broader group/reward eligibility, and real-client confirmation of the player-facing visual fix are still pending | Next: real-client smoke lazy grid loading/player visuals, then expose grid-load counters in a harness or continue G8/G9 fidelity |

## Gate Detail

### G8 Combat Agency

G8 is the core combat foundation for the whole game. Do not treat this as a
starter-only packet demo. The implementation should move in narrow vertical
slices, but the target behavior is CMaNGOS/Classic combat agency.

Requirements:

- Faction/reaction eligibility: replace entry allowlists with
  CMaNGOS-style `CanAttackOnSight` behavior using faction-template/reputation
  reactions. Hostile creatures can aggro; neutral creatures do not aggro unless
  attacked or scripted; friendly/allied NPCs, guards, trainers, vendors, and
  quest NPCs must not aggro friendly players.
- Aggro radius and timing: use CMaNGOS' DB-backed creature detection range
  (`creature_template.Detection`) as the base for the level-delta detection
  shape, minimum roughly 5 yards when non-zero, and run checks from
  world/creature ticks, not only player movement.
- Combat ownership: track creature victim, threat/attackers, player in-combat
  state, combat start/stop, home position, and invalid target cleanup.
- Movement to player: aggroed melee creatures must move toward the target,
  stop in melee range, face the target, continue following if the player moves,
  and publish movement updates visible to the client.
- Movement fidelity priority: because real-client feel is currently limited
  more by fake-looking creature motion than by damage math, prioritize
  CMaNGOS chase stop distance, re-path cadence, evade/home movement cleanup,
  and later idle/random/patrol movement before melee roll/damage parity.
- Parity guardrail: do not implement chase as an isolated packet shortcut. The
  Rust shape should follow CMaNGOS' chain: `CreatureAI::AttackStart` /
  `Unit::Attack` / combat ownership and threat, `MotionMaster::MoveChase`,
  `ChaseMovementGenerator`/spline movement, then
  `Unit::UpdateMeleeAttackingState` for reach/facing/timer-gated damage.
- Leash and evade: creatures need home/leash distance, threat clear, attack
  stop, evade state, health/reset behavior, and return-home behavior.
- Range and positioning: melee attacks require valid range. Player and creature
  attacks must not apply damage from spawn point or out of range. Ranged and
  spell ranges must be separate from melee range. Player DB-creature melee now
  uses the CMaNGOS minimum melee reach shape; combat reach/model-specific
  modifiers remain future fidelity work.
- Facing and arc rules: melee validity and defensive outcomes depend on
  orientation. Behind-target attacks must alter parry/block/dodge eligibility
  according to Classic rules; spell casts may require facing where applicable.
- Line of sight and path validity: aggro, melee, ranged attacks, and spell
  casts must eventually respect LOS/pathing. Initial slices may stub terrain
  checks only behind an explicit interface and must not hardcode distance-only
  assumptions into the combat API.
- Swing timers: player and creature melee need independent timers derived from
  weapon speed / creature base attack time. No machine-gun server tick damage.
- Combat rolls: melee resolution needs CMaNGOS-derived miss, dodge, parry,
  block, hit, crit, and evade outcomes, with packet results matching
  `SMSG_ATTACKERSTATEUPDATE`.
- Damage formulas: creature min/max melee damage, player weapon/base damage,
  armor mitigation, block value, and later absorb/resist handling should be
  source-derived and tested.
- Spell/GCD/queued melee integration: global cooldowns, target/range/facing
  validation, power/cooldown checks, and queued Heroic Strike-style next-swing
  behavior must integrate with melee combat rather than remain fixture damage.

Recommended slice order:

1. G8.1 Faction reaction gate.
2. G8.2 Creature combat state and threat/victim ownership.
3. G8.3 Melee chase / move-into-range v1.
4. G8.4 Range and facing-gated swing timers.
5. G8.5 Leash, evade, and return home.
6. G8.6 CMaNGOS chase stop-distance, re-path, and return-home movement feel.
7. G8.7 G9 idle/random/patrol movement v1 for starter creatures.
8. G8.8 Melee roll table.
9. G8.9 Damage formula v1.
10. G8.10 Spell, GCD, and queued melee integration.

Next movement slice:

- Compare `MotionMaster`, `ChaseMovementGenerator`,
  `HomeMovementGenerator`, and `Unit::CanReachWithMeleeAttack`.
- Chase stop/re-path now uses the CMaNGOS combined melee reach floor: creatures
  can hit at 5 yards, chase to half that range, and only refresh chase splines
  when the target leaves the full melee-reach window.
- Chase, return-home, and random movement can now store and send multi-point
  paths from local generated mmap data, with server-side interpolation across
  the same corners.
- Real-client tune chase and return-home transitions so mobs do not jitter,
  stall, or appear to run through/around the target in obviously fake ways.
- Keep vmap LOS, CMaNGOS path flags, and DB waypoint/patrol movement explicit
  until each piece is wired and proven honestly.

### G9 World Creature Fidelity

G9 owns non-combat creature behavior and world-state fidelity. It should be
generic CMaNGOS/DB-backed creature movement, not Northshire-specific scripting.
Northshire is the first proof area because it is the current playable slice.

Requirements:

- Use CMaNGOS source and DB behavior as the reference for `MovementType`,
  `spawndist`, home position, waypoint/path tables, idle/random movement,
  patrol movement, respawn timing, and AI update cadence.
- Avoid hardcoded starter creature movement rules. Harness-only fixtures are
  acceptable when clearly marked as proof data, but production behavior should
  come from DB/source-derived creature state.
- Keep responsibilities clear: G8 owns combat chase, melee reach, leash/evade,
  and return-home combat cleanup; G9 owns idle, random, waypoint, patrol,
  respawn, and persistent world-object behavior outside combat.
- Prove the generic behavior first in Northshire, then leave the shape ready
  for other zones without per-zone branches.

### G10 NPC Interaction Fidelity

NPC affordance is part of the playable feel. This gate covers yellow
exclamation marks, gray question marks, trainer affordance, vendor affordance,
gossip text, correct `NpcFlags`, wrong-class trainer behavior, unavailable menu
states, non-interactive NPC behavior, and clean failure handling.

### G11 Persistence + Relog Sanity

Relog checks should cover:

- position;
- health and power;
- XP and level;
- learned spells;
- quest accepted/progress/rewarded state;
- inventory, loot, and money;
- dead/alive/corpse state;
- creature respawn state, if it is persisted or intentionally runtime-scoped.

### G12 Derisk Multiplayer / Shared MapRuntime

Implementation document: `docs/g12_shared_mapruntime_plan.md`.

User-directed near-term goal:

- Keep the server one monolithic worldserver.
- Stop treating each TCP session as its own mini-world.
- Introduce a shared in-process `MapRuntime` / grid layer inside
  `WorldRuntimeState`.
- Route player visibility, movement, chat, and creature state through the shared
  map runtime.
- Creature visibility should no longer depend on DB radius queries per movement
  heartbeat.

Suggested implementation ladder:

1. Done: add shared `MapRuntime` ownership under `WorldRuntimeState` without
   changing gameplay behavior.
2. Done: register/unregister logged-in players in the shared map runtime.
3. Done: broadcast player spawn, movement, and logout destroy updates to nearby
   players through the shared map runtime.
4. Done: implement CMaNGOS-shaped grid/cell primitives for player-player
   visibility candidate lookup.
5. Done: route nearby `/say` through shared player visibility.
6. In progress: move DB creature live state into the shared runtime so all
   sessions observe the same alive/corpse/loot/respawn state. Shared snapshots,
   player-caused health/death update broadcast, DB-creature loot claims,
   exclusive DB-creature combat claims, and shared next-swing/retry timing are
   in, including victim-wide cleanup on player death. Creature combat-start,
   chase, facing, evade, and return-home packets now broadcast to nearby
   observers through `MapRuntime`. Session-local creature caches now refresh
   from shared snapshots before local ticks, and creature death sends a
   death-time motion stop to prevent observer clients from seeing a corpse keep
   patrolling or chasing. Idle/random/waypoint and return-home motion now write
   updated creature snapshots back into `MapRuntime`, and new idle motion
   splines broadcast through the shared map to nearby observers. Creature-origin
   damage packet execution and
   lifecycle event broadcast authority remain.
7. Replace movement-heartbeat DB radius visibility as the live creature source
   with grid/runtime visibility backed by loaded DB spawns.
8. Add a two-session harness or real-client smoke proof before resuming deeper
   G8/G9 tuning.

Minimum proof:

- Two separate clients can log into Northshire at the same time.
- Both players can see each other spawn.
- Both players can see each other move.
- One client logging out destroys that player for the other.
- Local `/say` is visible to the other nearby client.
- Both clients observe the same shared creature state for at least one starter
  mob.
- Loot, quest credit, and combat state do not duplicate or diverge between
  clients.
- Existing G3 movement visibility and `starter-zone-flow-test` flows remain
  green.

## GitHub Issue Labels

Use GitHub Issues as the detailed tracker. This board should only summarize
gate state and link blockers by issue number.

Every issue that supports this milestone should have one gate label:

- `gate:G1-login`
- `gate:G2-visual-world`
- `gate:G3-visibility-streaming`
- `gate:G4-quest-loop`
- `gate:G5-combat-loot`
- `gate:G6-level-trainer`
- `gate:G7-death-respawn`
- `gate:G8-combat-agency`
- `gate:G9-creature-fidelity`
- `gate:G10-npc-interaction`
- `gate:G11-relog-persistence`
- `gate:G12-multiclient`

Use these proof labels when applicable:

- `real-client-required`
- `harness-required`
- `gate-blocker`

## Issue Template Example

Title:

```text
[Gate G2][World] Friendly Northshire NPCs do not render correctly in real client
```

Body:

```md
## Gate

G2 Visual World Sanity

## Definition of done

Fresh Human Warrior login shows:
- Deputy Willem
- Marshal McBride
- Brother Paxton
- Llane Beshere
- Young Wolves
- Kobold Vermin if within visibility range

All have correct models/display IDs and can be targeted/interacted with where appropriate.

## Current evidence

Harness sees GUIDs in update packet, but real client does not render friendly NPCs correctly.

## Required proof

- Rust packet capture/log for one spawn
- CMaNGOS reference packet/source comparison
- Real-client screenshot or written smoke result
- `starter-zone-flow-test` still passes
```
