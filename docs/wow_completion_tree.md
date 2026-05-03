# WoW Completion Tree

This document is the whole-game completion map for the Rust rewrite. It is
intended to answer two questions at the same time:

- What are the systems of WoW Classic that eventually need parity?
- What are the smallest testable leaf requirements whose status can roll up to
  show parent completion?

The current playable milestone remains the Northshire Human Warrior slice. This
tree is broader than that milestone on purpose: it gives every narrow branch a
home and makes remaining work visible without turning `docs/session_handoff.md`
or `docs/playable_gate_board.md` into a giant backlog.

## Status Model

Only leaf requirements should normally be marked by hand.

| Status | Meaning |
| --- | --- |
| Red | Default. Missing, unstarted, unproven, or known broken. |
| Yellow | Partially implemented or harness-proven but missing real-client proof, CMaNGOS parity, persistence, multiplayer coverage, or edge cases. |
| Green | Implemented with CMaNGOS/DB/DBC/source-derived behavior and enough automated plus real-client proof for its risk. |

Parent node status is derived from children:

- Green: every required child is Green.
- Yellow: at least one required child is Yellow or Green, and at least one
  required child is not Green.
- Red: every required child is Red, or the node has an unresolved blocker that
  invalidates its children.

Completion percentage can be computed from leaves:

```text
leaf score: Red = 0.0, Yellow = 0.5, Green = 1.0
parent completion = sum(required leaf scores) / required leaf count
```

Optional or explicitly deferred leaves should stay in the tree, but they should
not count toward a milestone rollup unless the milestone names them as required.
Use `Deferred` in notes, not as a status color.

## Node Format

Use stable IDs so harness rows, GitHub issues, and docs can point at the same
requirement.

```text
WOW.<SYSTEM>.<SUBSYSTEM>.<REQUIREMENT>
```

Recommended row shape:

| ID | Status | Requirement | Proof |
| --- | --- | --- | --- |
| `WOW.EXAMPLE.SUBSYSTEM.LEAF` | Red | Testable behavior. | Harness, unit, DB fixture, CMaNGOS source path, packet proof, and/or real-client smoke. |

## Milestone Tags

Use these tags in notes or future machine-readable rows:

- `CP1`: auth, character, and basic world entry checkpoint.
- `CP2`: Northshire Human Warrior playable slice.
- `G1` through `G12`: gate labels from `docs/playable_gate_board.md`.
- `Classic`: required for faithful WoW 1.12.1 behavior beyond the current
  Northshire slice.
- `Deferred`: known valid WoW behavior intentionally outside the current
  milestone.

## Root

| ID | Status | Requirement | Proof |
| --- | --- | --- | --- |
| `WOW` | Yellow | WoW 1.12.1 Classic server parity root. | Rolls up all required leaf requirements below. Current project proof is strongest for auth, character lifecycle, Northshire world entry, movement visibility, shared `MapRuntime`, starter combat, death, loot, and selected quest/trainer paths. |

## 1. Account, Auth, And Realm

Parent: `WOW.AUTH`

| ID | Status | Requirement | Proof |
| --- | --- | --- | --- |
| `WOW.AUTH.SRP.LOGON_CHALLENGE` | Green | Client can complete 1.12.1 SRP logon challenge against `realmd.account`. | `test-auth-flow.cmd`; CP1 real-client proof. |
| `WOW.AUTH.SRP.LOGON_PROOF` | Green | Client proof is validated and a session key is stored for world login. | `test-auth-flow.cmd`; auth DB assertions. |
| `WOW.AUTH.REALMLIST.BUILD_5875` | Green | Build 5875 client receives a usable realm list and can select the Rust realm. | CP1 real-client proof. |
| `WOW.AUTH.REALMLIST_CHARACTER_COUNTS` | Green | Realm character counts refresh after create/delete. | `test-world-flow.cmd`; character screen proof. |
| `WOW.AUTH.DB_SCHEMA_REALMD` | Green | Auth behavior stays compatible with `sql/base/realmd.sql`. | DB integration tests. |
| `WOW.AUTH.ERRORS.INVALID_LOGIN` | Yellow | Invalid password, locked, banned, incompatible build, and bad proof fail with CMaNGOS-like codes. | Needs focused negative auth matrix. |
| `WOW.AUTH.SESSION_RECONNECT` | Red | Reconnect, duplicate login, and stale session handling are CMaNGOS-like. | Needs real-client reconnect proof. |

## 2. World Session And Protocol

Parent: `WOW.PROTOCOL`

| ID | Status | Requirement | Proof |
| --- | --- | --- | --- |
| `WOW.PROTOCOL.AUTH_SESSION` | Green | Worldserver validates `CMSG_AUTH_SESSION` and sends accepted auth response. | `test-world-flow.cmd`; starter-zone harness. |
| `WOW.PROTOCOL.OPCODE_TABLE` | Yellow | Supported opcode table routes known client startup/gameplay opcodes without desync. | Harness coverage exists; unsupported opcode audit remains. |
| `WOW.PROTOCOL.PACKET_SERIALIZATION` | Yellow | Packet builders serialize Classic 1.12.1 shapes for implemented systems. | Unit tests and real-client smoke by subsystem. |
| `WOW.PROTOCOL.UNKNOWN_OPCODE_HANDLING` | Yellow | Unsupported opcodes are logged, ignored, or rejected safely without disconnect loops. | CP1 quietness proof; broader audit needed. |
| `WOW.PROTOCOL.SESSION_LOOP_BACKPRESSURE` | Red | Slow clients, packet bursts, disconnects, and socket errors do not corrupt world state. | Needs stress harness. |
| `WOW.PROTOCOL.CRYPTO_HEADER` | Green | World packet header crypto works after auth handoff. | Auth/world flow tests. |

## 3. Character Lifecycle

Parent: `WOW.CHARACTER`

| ID | Status | Requirement | Proof |
| --- | --- | --- | --- |
| `WOW.CHARACTER.ENUM.LIST` | Green | Character screen enumerates existing characters with correct core fields. | `test-world-flow.cmd`; real-client CP1. |
| `WOW.CHARACTER.CREATE.VALID_HUMAN_WARRIOR` | Green | Fresh Human Warrior can be created with DB-backed starter state. | Starter-zone harness. |
| `WOW.CHARACTER.CREATE.RACE_CLASS_GENDER_DISPLAY` | Yellow | Race, class, gender, and appearance choices map to correct display IDs and restrictions. | CP1 partial race/gender proof; full matrix needed. |
| `WOW.CHARACTER.CREATE.INVALID_NAMES` | Yellow | Invalid, duplicate, reserved, and malformed names fail with correct client result. | Some create failures covered; full name policy needed. |
| `WOW.CHARACTER.DELETE.NON_LOADED` | Green | Non-loaded characters can be deleted and counts update. | CP1 flow proof. |
| `WOW.CHARACTER.LOGIN.ENTER_WORLD` | Green | Fresh character enters world without loading-screen hang. | `test-world-flow.cmd`; starter-zone harness. |
| `WOW.CHARACTER.LOGOUT.SELECT_SCREEN` | Green | Logout returns safely to character screen with persisted basics. | CP1 proof. |
| `WOW.CHARACTER.RELOG.BASIC_STATE` | Green | Position and basic state survive relog. | `test-world-flow.cmd`. |
| `WOW.CHARACTER.CUSTOMIZATION.ALL_RACES` | Red | All Classic race/class/gender appearance options are validated and persisted. | Needs matrix harness. |
| `WOW.CHARACTER.TUTORIAL_CINEMATIC` | Yellow | Cinematic/tutorial flags are sane for starter characters. | Starter state proof exists; parity audit needed. |

## 4. World Runtime, Maps, And Visibility

Parent: `WOW.WORLD`

| ID | Status | Requirement | Proof |
| --- | --- | --- | --- |
| `WOW.WORLD.MAPRUNTIME.SHARED_OWNER` | Green | Live player and creature state belongs to shared `MapRuntime`, not per-session mini-worlds. | G12 unit/harness coverage. |
| `WOW.WORLD.PLAYER_VISIBILITY.SPAWN` | Green | Nearby players receive create blocks on login. | Two-client starter-zone harness. |
| `WOW.WORLD.PLAYER_VISIBILITY.MOVEMENT` | Green | Nearby players receive movement and out-of-range destroy/create updates. | G12 harness; real-client smoke. |
| `WOW.WORLD.CHAT.SAY_LOCAL` | Green | Nearby `/say` broadcasts only to nearby players. | Two-client harness. |
| `WOW.WORLD.CREATURE_VISIBILITY.LOGIN` | Green | Login streams nearby DB-backed creatures from map state. | Starter-zone harness. |
| `WOW.WORLD.CREATURE_VISIBILITY.MOVEMENT` | Green | Movement streams new nearby creatures and destroys out-of-range ones. | G3 harness and real-client proof. |
| `WOW.WORLD.GRID.LOAD_RECTANGLE` | Green | DB creatures lazy-load by CMaNGOS-shaped grid rectangle into runtime cells. | `map_runtime_` tests. |
| `WOW.WORLD.GRID.REUSE_LOADED` | Green | Nearby players reuse loaded grids instead of repeating DB radius scans. | Query-count tests. |
| `WOW.WORLD.GRID.IDLE_UNLOAD` | Red | Idle grids unload/evict safely while preserving combat, corpse, loot, and respawn invariants. | Current G12 next action. |
| `WOW.WORLD.OBJECT_CREATE_FIELDS` | Yellow | Player, creature, corpse, item, and gameobject create blocks include correct Classic update fields. | Strong starter coverage; full object matrix needed. |
| `WOW.WORLD.INSTANCE_MAPS` | Red | Instance map ownership, reset, bind, and group visibility are implemented. | Deferred beyond Northshire. |
| `WOW.WORLD.TRANSPORTS` | Red | Boats, zeppelins, elevators, and moving transports are implemented. | Deferred. |
| `WOW.WORLD.WEATHER_ZONE_STATE` | Red | Weather and zone environmental state are sent and updated. | Deferred. |

## 5. Movement, Navigation, And Physics

Parent: `WOW.MOVEMENT`

| ID | Status | Requirement | Proof |
| --- | --- | --- | --- |
| `WOW.MOVEMENT.PLAYER.WALK_RUN_TURN` | Green | Player walking, running, turning, and position persistence work without disconnect. | CP1/G3 proof. |
| `WOW.MOVEMENT.PLAYER.JUMP_FALL` | Yellow | Jump/fall packets are accepted and persistence remains sane. | Needs focused real-client fall proof. |
| `WOW.MOVEMENT.PLAYER.SPEED_MODIFIERS` | Red | Walk, run, swim, mount, slow, root, and speed aura changes are authoritative. | Needs spell/aura integration. |
| `WOW.MOVEMENT.CREATURE.SPLINES` | Yellow | Creature movement publishes timed `SMSG_MONSTER_MOVE` splines visible to clients. | G8/G9 tests; real-client feel still under tuning. |
| `WOW.MOVEMENT.CREATURE.RANDOM` | Yellow | DB `MovementType` random movement uses CMaNGOS-like timing and safe positions. | Tests exist; true pathfinder random points remain. |
| `WOW.MOVEMENT.CREATURE.WAYPOINT` | Yellow | DB waypoint/path movement runs with waits and linear back-and-forth behavior. | Tests exist; script hooks and long stability pending. |
| `WOW.MOVEMENT.CREATURE.PATROL_LONG_RUNNING` | Red | Patrols stay alive over time, combat, death, respawn, grid activity, and observer churn. | Current Northshire missing criterion. |
| `WOW.MOVEMENT.PATHFINDING.MMAP` | Yellow | Generated mmap tiles are detected and Detour paths can be used where present. | Navigation tests; smoothing and flags pending. |
| `WOW.MOVEMENT.PATHFINDING.VMAP_LOS` | Red | LOS and terrain checks gate aggro, spell, and ranged interactions. | Interface exists; full parity pending. |
| `WOW.MOVEMENT.COLLISION.WATER_LIQUID` | Red | Water, swimming, fatigue, liquid status, and collision state are Classic-like. | Deferred. |

## 6. Creatures, NPCs, And Gameobjects

Parent: `WOW.OBJECTS`

| ID | Status | Requirement | Proof |
| --- | --- | --- | --- |
| `WOW.OBJECTS.CREATURE.DB_SPAWNS` | Green | Starter creatures load from real DB spawn/template rows when available. | Starter-zone harness. |
| `WOW.OBJECTS.CREATURE.QUERY_DETAILS` | Green | Client can query creature name/display data for implemented DB creatures. | CP1/G2 proof. |
| `WOW.OBJECTS.CREATURE.FACTION_REACTION` | Green | Hostile, neutral, and friendly starter reactions use faction-template bridge instead of entry allowlists. | G8 tests and real-client observation. |
| `WOW.OBJECTS.CREATURE.LIFECYCLE` | Green | DB creatures can be alive, corpse, dead, despawned, and respawned with DB/template timers. | G5/G9 tests. |
| `WOW.OBJECTS.CREATURE.RESPAWN_PERSISTENCE` | Green | Creature respawn state persists in `characters.creature_respawn` where CMaNGOS expects it. | Starter-zone harness. |
| `WOW.OBJECTS.NPC.QUEST_GIVER_STATUS` | Yellow | Quest givers expose correct available/reward/unavailable status icons. | Basic quest loop works; eligibility filters pending. |
| `WOW.OBJECTS.NPC.GOSSIP` | Yellow | Gossip hello and menu flows are correct for implemented starter NPCs. | Harness coverage partial. |
| `WOW.OBJECTS.NPC.TRAINER` | Yellow | Trainer list, buy, learned spell packet, and persistence work for selected starter warrior spell. | Starter-zone harness; full trainer states pending. |
| `WOW.OBJECTS.NPC.VENDOR` | Red | Vendor buy/sell, price, inventory, stack, money, and persistence are Classic-like. | CP2 required later. |
| `WOW.OBJECTS.NPC.FLAGS_AND_CURSORS` | Red | NPC flags, cursor affordances, wrong-class trainer behavior, and non-interactive NPC behavior match CMaNGOS. | G10 pending. |
| `WOW.OBJECTS.GAMEOBJECT.SPAWN_QUERY` | Yellow | Gameobject DB rows can exist and be queried for starter fixtures. | Fixture validation only. |
| `WOW.OBJECTS.GAMEOBJECT.QUEST_USE` | Red | Gameobject quest pickup/activation grants objective or quest item with respawn/availability rules. | Current Northshire missing criterion. |
| `WOW.OBJECTS.CORPSE.PLAYER_CREATE` | Green | Player corpse objects stream, query, reclaim, and become bones. | G7 proof. |
| `WOW.OBJECTS.DYNAMIC_OBJECTS` | Red | Dynamic objects, area triggers, and spell visuals are represented. | Spell/aura future work. |

## 7. Combat

Parent: `WOW.COMBAT`

| ID | Status | Requirement | Proof |
| --- | --- | --- | --- |
| `WOW.COMBAT.START_STOP.PLAYER_ATTACK` | Green | Player can start and stop attacking DB creatures safely. | Starter-zone harness. |
| `WOW.COMBAT.START_STOP.CREATURE_ATTACK` | Green | DB creatures can aggro, claim victims, and stop on invalid targets/death. | G8/G12 tests. |
| `WOW.COMBAT.THREAT.BASIC_VICTIM` | Yellow | Creature victim and attacker state are map-owned and cleaned up on death/logout. | Shared tests; full threat table pending. |
| `WOW.COMBAT.AGGRO.ON_SIGHT` | Green | Hostile starter creatures aggro from movement proximity with DB detection range. | G8 proof. |
| `WOW.COMBAT.AGGRO.ASSIST` | Yellow | Same-faction nearby assists are called with CMaNGOS-like radius. | Unit coverage; broader faction/AI proof pending. |
| `WOW.COMBAT.CHASE.MOVE_INTO_RANGE` | Yellow | Aggroed melee creatures chase and stop near melee reach. | G8 proof; motion polish pending. |
| `WOW.COMBAT.LEASH.EVADE_HOME` | Yellow | Creatures leash, clear combat, reset health, and return home. | First slice exists; hit-reactivation and persistence parity pending. |
| `WOW.COMBAT.MELEE.RANGE_FACING` | Yellow | Player and creature melee require valid range and facing guardrails. | Tests exist; exact reach/model modifiers pending. |
| `WOW.COMBAT.MELEE.SWING_TIMERS` | Yellow | Player and creature melee use independent timers from weapon/base attack time. | Starter coverage; broad weapon/aura modifiers pending. |
| `WOW.COMBAT.MELEE.CREATURE_OUTCOME_TABLE` | Yellow | Creature melee rolls miss/dodge/parry/block/glancing/crit/crushing and serializes attacker-state updates. | First outcome path exists; live stats pending. |
| `WOW.COMBAT.MELEE.PLAYER_OUTCOME_TABLE` | Red | Player offensive outcome rolls use weapon skill, defense, target level, and positional rules. | Current G8 follow-up. |
| `WOW.COMBAT.DAMAGE.CREATURE_FORMULA` | Yellow | Creature melee damage uses DB min/max, armor mitigation, and block outcome. | First slice exists; broader stats pending. |
| `WOW.COMBAT.DAMAGE.PLAYER_FORMULA` | Yellow | Player auto-attack damage uses equipped weapon and class/stat attack power. | First slice exists; full combat math pending. |
| `WOW.COMBAT.LOG.MELEE` | Red | Client combat log receives melee hit, miss, dodge, parry, block, crit, and damage feedback. | Current Northshire missing criterion. |
| `WOW.COMBAT.LOG.SPELL` | Red | Client combat log receives spell cast, damage, miss, resist, and resource feedback. | Current Northshire missing criterion. |
| `WOW.COMBAT.DEATH.CREATURE` | Green | Creature death broadcasts state, stops motion, opens loot, and later respawns. | G5/G12 proof. |
| `WOW.COMBAT.DEATH.PLAYER` | Green | Player can die, release, become ghost, reclaim, and resurrect. | G7 proof. |
| `WOW.COMBAT.PVP_DUEL` | Red | Player-vs-player duels and hostile player combat are implemented. | Deferred. |

## 8. Spells, Auras, Classes, And Resources

Parent: `WOW.SPELLS`

| ID | Status | Requirement | Proof |
| --- | --- | --- | --- |
| `WOW.SPELLS.CAST.START_GO_RESULT` | Yellow | Implemented casts send correct cast result/start/go packet shapes. | Heroic Strike/Battle Shout coverage; broad matrix pending. |
| `WOW.SPELLS.GCD.GLOBAL` | Red | Global cooldown starts, blocks, expires, and is visible to the client. | Current Northshire missing criterion. |
| `WOW.SPELLS.COOLDOWN.PER_SPELL` | Red | Per-spell cooldowns, categories, and failure packets are Classic-like. | Needed for warrior level 1-6. |
| `WOW.SPELLS.POWER.COSTS` | Yellow | Supported starter spells spend/validate power correctly. | Partial starter spell tests. |
| `WOW.SPELLS.WARRIOR.HEROIC_STRIKE` | Yellow | Heroic Strike queues as next melee, delays spell-go until swing, and reports yellow damage. | Packet-shape tests; real-client queued-state smoke pending. |
| `WOW.SPELLS.WARRIOR.BATTLE_SHOUT` | Yellow | Battle Shout can be learned and appears in the spellbook. | Trainer persistence proof; aura/effect parity pending. |
| `WOW.SPELLS.WARRIOR.LEVEL_1_6` | Red | All Human Warrior level 1-6 actions needed for Northshire use real spell data/effects. | Current Northshire missing criterion. |
| `WOW.SPELLS.AURAS.APPLY_REMOVE` | Red | Aura apply/remove, durations, stacking, periodic ticks, and stat modifiers are generic. | Future spell foundation. |
| `WOW.SPELLS.RESOURCES.HEALTH_REGEN` | Red | Health regeneration ticks with CMaNGOS-like timing/rates and packet updates. | Current Northshire missing criterion. |
| `WOW.SPELLS.RESOURCES.RAGE_DECAY` | Red | Warrior rage degenerates out of combat with Classic timing and packet updates. | Current Northshire missing criterion. |
| `WOW.SPELLS.RESOURCES.MANA_ENERGY` | Red | Mana, energy, focus, and other power regeneration rules are implemented. | Deferred beyond warrior slice. |
| `WOW.SPELLS.TARGETING.RANGE_FACING_LOS` | Red | Spell target validation uses range, facing, LOS, target type, and failure packets. | Needed before broad spell parity. |
| `WOW.SPELLS.SPELLBOOK.PERSISTENCE` | Yellow | Learned spells load, display, and persist. | Starter trainer proof; full class/race matrix pending. |

## 9. Quests And Objectives

Parent: `WOW.QUESTS`

| ID | Status | Requirement | Proof |
| --- | --- | --- | --- |
| `WOW.QUESTS.ACCEPT.BASIC` | Green | Client can accept supported starter quests. | Starter-zone harness. |
| `WOW.QUESTS.PROGRESS.KILL_COUNT` | Green | Kill-count quest progress updates and completes. | Kobold Camp Cleanup proof. |
| `WOW.QUESTS.COMPLETE.REWARD` | Yellow | Supported starter quest can be completed/rewarded in client. | Harness green; real-client final proof still useful. |
| `WOW.QUESTS.ELIGIBILITY.LEVEL` | Red | Quest availability filters by level. | Current Northshire missing criterion. |
| `WOW.QUESTS.ELIGIBILITY.RACE_CLASS` | Red | Quest availability filters by race and class. | Current Northshire missing criterion. |
| `WOW.QUESTS.ELIGIBILITY.PREREQUISITE_CHAIN` | Red | Quest availability filters by prerequisite, chain, exclusive group, and previous completion. | Current Northshire missing criterion. |
| `WOW.QUESTS.ELIGIBILITY.REPEATABLE_DAILY` | Red | Repeatability and completed-state availability follow CMaNGOS. | Current Northshire missing criterion. |
| `WOW.QUESTS.OBJECTIVES.ITEM_DROPS` | Red | Quest item objectives drop only from real loot tables and only for eligible active quests. | Current Northshire missing criterion. |
| `WOW.QUESTS.OBJECTIVES.GAMEOBJECT` | Red | Gameobject objectives and pickup quests work with DB-backed gameobjects. | Current Northshire missing criterion. |
| `WOW.QUESTS.OBJECTIVES.EXPLORE_SPELL_EMOTE` | Red | Explore, spellcast, escort, reputation, and script objectives are represented. | Deferred. |
| `WOW.QUESTS.LOG_LIMITS` | Red | Quest log capacity, failure result codes, abandon, and share restrictions are Classic-like. | Deferred. |
| `WOW.QUESTS.RELOG_PERSISTENCE` | Yellow | Accepted/progress/completed quest state persists after relog. | Starter proof for one quest; broader matrix pending. |
| `WOW.QUESTS.REWARDS.XP_MONEY_ITEMS` | Yellow | Quest rewards grant XP, money, items, spell/faction where applicable. | Partial; CP2 closure needed. |

## 10. Loot, Inventory, Items, And Economy

Parent: `WOW.ITEMS`

| ID | Status | Requirement | Proof |
| --- | --- | --- | --- |
| `WOW.ITEMS.INVENTORY.STARTER_ITEMS` | Green | Fresh character receives starter inventory/equipment state. | CP1/starter proof. |
| `WOW.ITEMS.INVENTORY.MOVE_EQUIP_DESTROY` | Yellow | Move, equip, unequip, destroy, split, stack merge, and equipped-bag movement work and persist. | CP1 proof; broader item edge cases pending. |
| `WOW.ITEMS.EQUIPMENT.VISIBLE_FIELDS` | Green | Nearby players see equipment visual updates for implemented equip/unequip paths. | G12 proof. |
| `WOW.ITEMS.LOOT.MONEY` | Green | Creature money loot can be claimed once and persists. | Starter-zone shared wolf proof. |
| `WOW.ITEMS.LOOT.NORMAL_ITEMS` | Yellow | Normal item loot opens, autostores, handles inventory, and persists. | Harness partial; full CMaNGOS loot table issue remains. |
| `WOW.ITEMS.LOOT.QUEST_ITEMS` | Red | Quest item loot rolls from real tables and respects quest eligibility. | Current Northshire missing criterion. |
| `WOW.ITEMS.LOOT.MULTICLIENT_EXCLUSIVE` | Green | Two clients cannot duplicate the same creature loot claim. | G12 harness. |
| `WOW.ITEMS.LOOT.CORPSE_RELEASE` | Green | Loot release updates corpse flags for observers and respawn lifecycle. | G12 harness. |
| `WOW.ITEMS.ITEM_QUERY` | Yellow | Client item queries return correct DB-backed item data for implemented items. | Existing item query builders; broader matrix pending. |
| `WOW.ITEMS.VENDOR.BUY_SELL` | Red | Vendor buy/sell, inventory availability, money checks, refund-like edge cases, and persistence work. | CP2 pending. |
| `WOW.ITEMS.MAIL` | Red | Mail attachments, money, COD, expiry, and mailbox UI work. | Deferred. |
| `WOW.ITEMS.AUCTION_HOUSE` | Red | Auction house listing, bidding, buyout, deposits, and mail delivery work. | Deferred. |
| `WOW.ITEMS.BANK_BAGS` | Red | Bank, bags, slots, and bag-specific constraints are Classic-like. | Deferred. |
| `WOW.ITEMS.DURABILITY_REPAIR` | Red | Durability loss, repair, and death durability rules work. | G7 follow-up/deferred. |

## 11. Progression, Stats, Skills, And Reputation

Parent: `WOW.PROGRESSION`

| ID | Status | Requirement | Proof |
| --- | --- | --- | --- |
| `WOW.PROGRESSION.XP.CREATURE` | Yellow | Creature XP grants render and persist. | Starter proof exists; formula parity pending. |
| `WOW.PROGRESSION.XP.QUEST` | Yellow | Quest XP grants render and persist. | Partial CP2 proof needed. |
| `WOW.PROGRESSION.LEVEL_UP.PACKETS` | Yellow | Level-up packets, stat refresh, health/power refresh, and persistence are correct. | Trainer/level gate partial. |
| `WOW.PROGRESSION.STATS.BASE_CLASS_RACE` | Yellow | Base stats derive from source/DBC/DB rather than invented constants. | Starter state sane; full derivation audit pending. |
| `WOW.PROGRESSION.STATS.DERIVED_COMBAT` | Yellow | Attack power, armor, defense, block, crit, and damage derive from stats/equipment. | First combat slice; full parity pending. |
| `WOW.PROGRESSION.SKILLS.LOAD_SHOW` | Red | Skills and weapon skills load and display in the client. | Current Northshire missing criterion. |
| `WOW.PROGRESSION.SKILLS.WEAPON_ADVANCE` | Red | Weapon skills advance from real actions and persist. | Current Northshire missing criterion. |
| `WOW.PROGRESSION.SKILLS.TRADE_LANGUAGE_DEFENSE` | Red | Defense, languages, professions, class skills, and trade skills are represented. | Deferred. |
| `WOW.PROGRESSION.REPUTATION.FACTION_STANDING` | Yellow | Reputation manager supports faction standing enough for hostile/friendly reactions. | Combat reaction bridge exists; full rep UI/rewards pending. |
| `WOW.PROGRESSION.REPUTATION.GAINS` | Red | Reputation gains, spillover, ranks, and UI updates work. | Deferred. |
| `WOW.PROGRESSION.TALENTS` | Red | Talent points, learn/unlearn, prerequisites, and reset costs work. | Deferred beyond level 6. |

## 12. Persistence And Relog Durability

Parent: `WOW.PERSISTENCE`

| ID | Status | Requirement | Proof |
| --- | --- | --- | --- |
| `WOW.PERSISTENCE.POSITION` | Green | Position persists across logout/relog and movement. | CP1/G11 partial proof. |
| `WOW.PERSISTENCE.HEALTH_POWER` | Yellow | Health and power persist/reset according to Classic rules across relog/death. | Death proof exists; regen/resource rules pending. |
| `WOW.PERSISTENCE.INVENTORY_MONEY` | Yellow | Inventory and money persist after loot, move, buy/sell, and relog. | Loot proof partial; vendor pending. |
| `WOW.PERSISTENCE.QUEST_STATE` | Yellow | Quest accepted/progress/rewarded state persists after relog. | One starter quest proof; broader matrix pending. |
| `WOW.PERSISTENCE.SPELLS_SKILLS` | Yellow | Learned spells and skill state persist. | Learned spell proof; skills pending. |
| `WOW.PERSISTENCE.DEATH_CORPSE` | Green | Dead, ghost, corpse, reclaim, bones, and corpse row deletion remain sane. | G7 harness. |
| `WOW.PERSISTENCE.CREATURE_RESPAWN` | Green | Creature respawn persistence works for DB creatures. | Starter-zone harness. |
| `WOW.PERSISTENCE.MULTICLIENT_CONSISTENCY` | Yellow | Logout/relog during combat, corpse, loot, and grid-load states cannot fork shared state. | Shared state proof exists; dedicated torture coverage pending. |
| `WOW.PERSISTENCE.CRASH_RECOVERY` | Red | Server restart restores durable character, creature, quest, corpse, and mail state correctly. | Deferred. |

## 13. Social, Group, Guild, And Communication

Parent: `WOW.SOCIAL`

| ID | Status | Requirement | Proof |
| --- | --- | --- | --- |
| `WOW.SOCIAL.CHAT.SAY` | Green | Local say works for nearby players. | G12 harness. |
| `WOW.SOCIAL.CHAT.YELL_WHISPER_PARTY_GUILD` | Red | Yell, whisper, party, guild, officer, raid, and channel chat work with errors. | Deferred. |
| `WOW.SOCIAL.FRIEND_IGNORE` | Red | Friend, ignore, who, played, and online notifications work. | Deferred. |
| `WOW.SOCIAL.PARTY.BASIC` | Red | Invite, accept, leave, leader, loot method, XP/quest sharing, and minimap member state work. | Deferred. |
| `WOW.SOCIAL.GUILD.BASIC` | Red | Guild create, roster, ranks, permissions, MOTD, and chat work. | Deferred. |
| `WOW.SOCIAL.TRADE` | Red | Player trade, money/items, cancel, confirmation, and anti-dupe safeguards work. | Deferred. |

## 14. Dungeons, Instances, Raids, And Encounters

Parent: `WOW.INSTANCES`

| ID | Status | Requirement | Proof |
| --- | --- | --- | --- |
| `WOW.INSTANCES.MAP_CREATION` | Red | Instance maps create isolated runtime state per group/reset. | Deferred. |
| `WOW.INSTANCES.BIND_RESET` | Red | Bind, soft reset, hard reset, lockout, and corpse entrance behavior work. | Deferred. |
| `WOW.INSTANCES.BOSS_AI` | Red | Boss combat, threat, scripts, spells, loot locks, and reset behavior are CMaNGOS-like. | Deferred. |
| `WOW.INSTANCES.DUNGEON_PORTALS` | Red | Dungeon portals, meeting stones, entrance requirements, and teleport handling work. | Deferred. |
| `WOW.INSTANCES.RAID_GROUPS` | Red | Raid groups, conversions, markers, assist, loot rules, and lockouts work. | Deferred. |

## 15. PvP, Honor, Battlegrounds, And World PvP

Parent: `WOW.PVP`

| ID | Status | Requirement | Proof |
| --- | --- | --- | --- |
| `WOW.PVP.FLAGS` | Red | PvP flags, sanctuary/contested zones, faction attackability, and timers work. | Deferred. |
| `WOW.PVP.HONOR_KILLS` | Red | Honorable kills, dishonorable kills, contribution points, ranks, and decay work. | Deferred. |
| `WOW.PVP.BATTLEGROUNDS.QUEUE` | Red | Battleground queue, invite, join, leave, deserter, and scoreboard work. | Deferred. |
| `WOW.PVP.BATTLEGROUNDS.WSG_AB_AV` | Red | Warsong Gulch, Arathi Basin, and Alterac Valley objectives and rewards work. | Deferred. |
| `WOW.PVP.DUELS` | Red | Duel request, boundary, victory, cancel, and class combat integration work. | Deferred. |

## 16. AI, Scripts, Events, And World Logic

Parent: `WOW.SCRIPTS`

| ID | Status | Requirement | Proof |
| --- | --- | --- | --- |
| `WOW.SCRIPTS.CREATURE_AI.BASIC_AGGRO` | Yellow | Creature AI can aggro, chase, melee, assist, evade, and return home for starter mobs. | G8 partial. |
| `WOW.SCRIPTS.CREATURE_AI.SPELL_CASTING` | Red | Creature spell casting, cooldowns, target selection, and interrupt behavior work. | Deferred. |
| `WOW.SCRIPTS.EVENTS.DB_SCRIPTS` | Red | DB script commands for quests, gossip, gameobjects, movement, and events are executed. | Deferred. |
| `WOW.SCRIPTS.ESCORTS` | Red | Escort quests and waypoint script hooks are implemented. | Deferred. |
| `WOW.SCRIPTS.GAME_EVENTS` | Red | Holiday/world events spawn, despawn, and change NPC/gameobject behavior. | Deferred. |
| `WOW.SCRIPTS.SMARTAI_EQUIVALENT` | Red | CMaNGOS creature/gameobject AI scripts used by Classic content are represented. | Deferred. |

## 17. Data Fidelity And Content Loading

Parent: `WOW.DATA`

| ID | Status | Requirement | Proof |
| --- | --- | --- | --- |
| `WOW.DATA.REALMD_SCHEMA` | Green | Auth schema remains compatible with Classic `realmd`. | Auth DB tests. |
| `WOW.DATA.CHARACTERS_SCHEMA` | Yellow | Character DB reads/writes remain compatible with CMaNGOS character schema for implemented systems. | Tests by subsystem; full schema audit pending. |
| `WOW.DATA.WORLD_DB_CREATURES` | Green | Creature templates/spawns for starter zones load from real world DB when present. | Starter-zone harness. |
| `WOW.DATA.WORLD_DB_QUESTS` | Yellow | Quest templates and starter quest rows load enough for supported flows. | Quest loop proof; eligibility fields pending. |
| `WOW.DATA.WORLD_DB_LOOT` | Yellow | DB-backed loot basics exist. | Harness partial; real loot-table rolling pending. |
| `WOW.DATA.WORLD_DB_GAMEOBJECTS` | Yellow | Gameobject rows are recognized for fixture/startup checks. | Activation/objective behavior pending. |
| `WOW.DATA.DBC.SPELLS` | Red | Spell effects, ranges, schools, costs, cooldowns, aura data, and ranks derive from DBC/source. | Needed for real spell parity. |
| `WOW.DATA.DBC.CLASSES_RACES_STATS` | Red | Race/class stats, skills, display data, and level progression derive from DBC/source. | Needed before broad character parity. |
| `WOW.DATA.MAPS_VMAPS_MMAPS` | Yellow | Map/vmap/mmap paths are detected and available to movement/combat systems. | Startup/navigation checks; full LOS/pathing pending. |

## 18. Operations, Tooling, And Test Harnesses

Parent: `WOW.TOOLING`

| ID | Status | Requirement | Proof |
| --- | --- | --- | --- |
| `WOW.TOOLING.TEST_RUST` | Green | `.\scripts\test-rust.cmd` verifies the Rust workspace baseline. | Current standard gate. |
| `WOW.TOOLING.TEST_DB` | Green | `.\scripts\test-rust-db.cmd` verifies auth DB/TCP startup paths. | Auth foundation gate. |
| `WOW.TOOLING.TEST_AUTH_FLOW` | Green | `.\scripts\test-auth-flow.cmd` verifies auth protocol behavior. | Auth foundation gate. |
| `WOW.TOOLING.STARTER_ZONE_FLOW` | Yellow | `.\scripts\test-starter-zone-flow.cmd` proves Northshire starter flows. | Strong CP2 coverage; user real-client proof remains the closure authority. |
| `WOW.TOOLING.REAL_CLIENT_SMOKE` | Yellow | Manual real-client checklists are kept for player-visible behavior that harnesses cannot yet prove. | Needs final CP2 closure table. |
| `WOW.TOOLING.PACKET_CAPTURE_DIFF` | Red | Packet captures can be compared against CMaNGOS for high-risk protocol parity. | Future useful tooling. |
| `WOW.TOOLING.PERF_COUNTERS` | Yellow | Query counts, grid loads, and runtime counters expose performance-sensitive world behavior. | G12 grid counters; broader counters pending. |

## Northshire CP2 Overlay

These are the current user-observed missing criteria mapped onto the whole-game
tree. Use this table to decide which leaf requirements matter for the active
milestone before marking broader Classic-only leaves.

| Current Criterion | Tree Leaves | Current Expected Status |
| --- | --- | --- |
| Quest availability restrictions | `WOW.QUESTS.ELIGIBILITY.LEVEL`, `WOW.QUESTS.ELIGIBILITY.RACE_CLASS`, `WOW.QUESTS.ELIGIBILITY.PREREQUISITE_CHAIN`, `WOW.QUESTS.ELIGIBILITY.REPEATABLE_DAILY` | Red until harness rejects unavailable quests and real client shows correct markers/lists. |
| Quest item drops from real loot tables | `WOW.QUESTS.OBJECTIVES.ITEM_DROPS`, `WOW.ITEMS.LOOT.QUEST_ITEMS`, `WOW.DATA.WORLD_DB_LOOT` | Red/Yellow until active-quest-gated real loot drops are proven. |
| Gameobject quest pickup | `WOW.OBJECTS.GAMEOBJECT.QUEST_USE`, `WOW.QUESTS.OBJECTIVES.GAMEOBJECT` | Red until CMSG gameobject activation grants the objective/item with DB rules. |
| Warrior level 1-6 spells, GCD, Heroic Strike | `WOW.SPELLS.GCD.GLOBAL`, `WOW.SPELLS.COOLDOWN.PER_SPELL`, `WOW.SPELLS.WARRIOR.HEROIC_STRIKE`, `WOW.SPELLS.WARRIOR.BATTLE_SHOUT`, `WOW.SPELLS.WARRIOR.LEVEL_1_6` | Yellow/Red until source-derived spell behavior and client proof are complete. |
| Combat log feedback | `WOW.COMBAT.LOG.MELEE`, `WOW.COMBAT.LOG.SPELL` | Red until explicit packet assertions and real-client log proof exist. |
| Health regeneration and rage degeneration | `WOW.SPELLS.RESOURCES.HEALTH_REGEN`, `WOW.SPELLS.RESOURCES.RAGE_DECAY`, `WOW.PERSISTENCE.HEALTH_POWER` | Red until timed ticks and packet updates are proven. |
| Skills and weapon skills | `WOW.PROGRESSION.SKILLS.LOAD_SHOW`, `WOW.PROGRESSION.SKILLS.WEAPON_ADVANCE`, `WOW.PERSISTENCE.SPELLS_SKILLS` | Red until UI load, gain, and persistence are proven. |
| CMaNGOS-like aggro/chase/leash | `WOW.COMBAT.AGGRO.ON_SIGHT`, `WOW.COMBAT.AGGRO.ASSIST`, `WOW.COMBAT.CHASE.MOVE_INTO_RANGE`, `WOW.COMBAT.LEASH.EVADE_HOME`, `WOW.MOVEMENT.PATHFINDING.MMAP` | Yellow until the remaining leash persistence and movement feel match CMaNGOS. |
| Patrol runtime stability | `WOW.MOVEMENT.CREATURE.PATROL_LONG_RUNNING`, `WOW.MOVEMENT.CREATURE.RANDOM`, `WOW.MOVEMENT.CREATURE.WAYPOINT`, `WOW.WORLD.GRID.IDLE_UNLOAD` | Red/Yellow until long-duration patrol continuity is harnessed. |

## Next Automation Step

The editable source is now `docs/wow_completion_tree.toml`. Regenerate the
static dashboard with:

```powershell
.\scripts\render-wow-tree.cmd
```

Then open:

```text
docs/generated/wow_completion_tree.html
```

The dashboard supports:

- compact click-to-expand parent/child nodes, starting at the `WoW` root;
- one expanded pathway at a time, so opening one second-tier system closes the
  other second-tier branch;
- children are centered under the node that was expanded;
- computed Red/Yellow/Green parent rollups;
- completion percentages and leaf counts;
- CP2 filtering;
- Red/Yellow/Green filtering;
- text search for systems like `Heroic Strike`, `loot`, or `relog`;
- system cards for quick scan of the worst remaining areas.
- drag-to-pan and wheel/button zoom in tree view.

When the tree structure changes, edit `docs/wow_completion_tree.toml` first and
rerender the dashboard. The Markdown tree can remain the human-readable design
brief, but TOML should be the source of truth for visual rollups.

Future useful hardening:

- add a CI check that runs `.\scripts\render-wow-tree.cmd`;
- fail if the generated HTML is stale;
- reject hand-edited parent statuses when a node has children;
- emit CP2/Gate-specific Markdown tables from the same TOML.

Example TOML row:

```toml
[[node]]
id = "WOW.QUESTS.ELIGIBILITY.LEVEL"
status = "red"
required_for = ["CP2", "G4", "G10"]
requirement = "Quest availability filters by level."
proof = "starter-zone-flow focused probes; user real-client quest marker smoke."
```
