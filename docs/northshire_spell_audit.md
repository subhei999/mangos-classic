# Northshire Reachable Spell Audit

This audit narrows spell work to spells a human warrior can reasonably see in
Northshire through level 6, plus nearby creature, quest, item, loot, and game
object spell surfaces from the imported CMaNGOS database.

Coordinate envelope used for DB discovery:

```sql
map = 0
position_x BETWEEN -9200 AND -8700
position_y BETWEEN -700 AND 300
```

`creature_zone` and `gameobject_zone` had no `ZoneId = 12` rows in the local DB,
so the coordinate envelope is the current reliable source for this starter-zone
slice.

## Sources

- Human warrior starting spells:
  `78,81,107,196,198,201,203,204,522,668,2382,2457,2479,3050,3365,5301,6233,6246,6247,6477,6478,6603,7266,7267,7355,8386,8737,9077,9078,9116,9125,20597,20598,20599,20600,20864,21651,21652,22027,22810`.
- Northshire warrior trainer `911 Llane Beshere`, through level 6:
  `6674,1423,1738,1343,3128`.
- Northshire creature template spell slots:
  `53,133,12544,6016,744,12023,8260`.
- Northshire EventAI `ACTION_T_CAST = 11` spells:
  `6016,11959,53,7164,11919,12023,12544,20793,18950,11939,10848,9036`.
- Quest reward/source spell fields from local quest givers/GOs:
  `688,7763`.
- Quest, chest, food/drink, potion, and welcome-item use spells:
  `430,433,439,17707,17708,17709`.
- Triggered spells reached from those rows:
  `100,772,3127,5302,6136,6673,7922,11918,11920`.

Gameobject template `data*` values were not treated as spell IDs generically.
For example chest `data1` is a loot template, not a spell. Type-specific GO
spell fields should be audited only when the GO type definition says the field
is a spell.

## Covered Now

These Northshire-visible mechanics are already covered by generic Rust spell
effect or aura support:

- Direct spell damage and weapon damage: Heroic Strike, Backstab, Fireball.
- Healing: Healing Potion and simple heal rows.
- Food/drink periodic recovery auras.
- Damage-over-time auras: Rend and poison periodic damage.
- Rage energize and trigger-spell execution: Charge.
- Charge movement and Charge Stun.
- Root/decrease-speed auras: Web root and Chilled slow.
- Attack power, damage-done, melee haste, stat, skill, resistance,
  resistance-percent, speed, and reputation auras when applied through the
  generic aura path.
- Tracking and utility visibility aura metadata: stealth detection,
  invisibility detection, creature/resource tracking fields, dummy utility
  auras, creature ghost visual flags, and water-walk aura state.
- Open-lock/opening effects used by chest and GO interaction spells.
- Learn-spell effect `36` for player targets.
- Armor proficiencies and passive/no-op starter spell rows classified as
  implemented or CMaNGOS no-op.

## Blocking Gaps By Chunk

### Chunk A: Creature EventAI Spell Casting

This chunk is now implemented for the Northshire-visible `ACTION_T_CAST = 11`
surface: timer-in-combat, timer-OOC, aggro, range, facing-target, missing-aura,
and spawned events can select casts from loaded DB scripts and route them
through the shared creature spell pipeline.

Relevant Northshire rows:

- Kobold Miner: `6016 Pierce Armor` from timer-in-combat.
- Defias Cutpurse: `53 Backstab` from facing-target.
- Garrick Padfoot: `7164 Defensive Stance` on aggro.
- Mine Spider: `11959 Poison Proc` OOC.
- Mother Fang: `11919 Poison Proc`, `12023 Web`.
- Kobold Geomancer: `12544 Frost Armor`, `20793 Fireball`, range-mode setup.
- Northshire Guard: `18950 Invisibility and Stealth Detection` OOC.
- Dane Winslow: `11939 Summon Imp` OOC.
- Spirit Healer: `10848 Shroud of Death`, `9036 Ghost` on spawn.

Remaining EventAI breadth is outside the immediate Northshire combat smoke:
zone-conditioned spawned events, evade/reached-home/kill/death events, richer
cast flags, and non-cast actions beyond the HP flee/set-walk slice.

### Chunk B: Learn-Spell Cast Effect

Effect `36` is implemented for player targets and persists the learned spell,
updates the session spell set, and sends the learned-spell/proficiency/initial
spell updates. It appears in spell rows that teach or unlock spells:

- `1423 Rend` triggers `772`.
- `1738 Charge` triggers `100`.
- `3128 Parry` triggers `3127`.
- `6674 Battle Shout` triggers `6673`.
- `7763 Teach Summon Imp` triggers `688`.

Trainer code still learns spells through the trainer path, but the generic
effect now covers quest/class reward spell teaching and avoids future one-off
trainer/quest fixes.

### Chunk C: Creature Combat Modifier Auras

These are now covered for the Northshire-visible generic buckets:

- `6016 Pierce Armor`: aura `101` resistance-percent armor reduction.
- `8260 Rushing Charge`: aura `13` physical damage modifier and aura `31` speed
  modifier.
- `6136 Chilled`: aura `138` melee haste and aura `33` speed decrease.
- `12544 Frost Armor`: aura `22` resistance and trigger aura `42`.

The next modifier work should be driven by newly audited spells, not by
Northshire's current level 1-6 warrior path.

### Chunk D: Summons And Pets

Summon effects are still pending and are present in reachable but lower-priority
Northshire rows:

- `688 Summon Imp`, `7763 Teach Summon Imp`.
- `11939 Summon Imp` from Dane Winslow.
- `17707 Summon Panda`, `17708 Summon Diablo`, `17709 Summon Zergling`.

This needs the real summon/pet owner model rather than a fake visual-only spawn.

### Chunk E: Tracking, Visibility, Duel, Stuck, And Corpse Utility

The generic aura side of this chunk is now covered for Northshire-visible rows:

- `18950 Invisibility and Stealth Detection`, `20600 Perception`.
- `10848 Shroud of Death`, `9036 Ghost`.

Tracking/detection auras now preserve their CMaNGOS modifier metadata on the
active aura, tracking auras update `PLAYER_TRACK_CREATURES` and
`PLAYER_TRACK_RESOURCES`, and ghost auras on DB creatures update the unit
visibility byte used by Spirit Healer-style spawned auras.

The remaining effect handlers are intentionally still pending because CMaNGOS
routes them through larger owner systems, not isolated spell math:

- `7266 Duel`, `7355 Stuck`, `22027 Remove Insignia`.

`Duel` needs duel flag gameobject creation, duel request/accept/cancel state,
area checks, and end conditions. `Stuck` needs graveyard/hearthstone/safe
teleport ownership. `Remove Insignia` needs player corpse/PvP loot conversion.

They should stay behind the current combat/EventAI and utility-aura work, and
ahead of pet/summon work only if the next playtest specifically needs them.

## Recommended Next Slice

Run the real-client Northshire spell smoke before adding more speculative spell
surface. Focus on visible combat behavior: EventAI casts firing, aura icons and
unit flags updating, Thunder Clap AoE, Charge stun/rage, wounded slowdown, and
creatures resuming normal combat after flee/stun/root effects expire.
