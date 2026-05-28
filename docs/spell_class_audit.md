# Spell Class Audit

Durable tracker for the class-by-class generic spell audit. Keep this file
compact. Record what is done, what required generic runtime work, what is
deferred, and the next in-order blocking family. Do not turn this back into a
run log.

## Global Notes

- Current class: `Warlock`
- Current next blocking family: `Hellfire`
- Use local `spell_chain`, live spell rows, and CMaNGOS as the behavior
  reference.
- Local `npc_trainer` / `npc_trainer_template` data is empty in this dump, so
  learn-path confirmation often comes from spell-chain and live-row evidence.
- Rogue `Stealth` has no local `spell_chain` row in this dump; rank/level
  backing comes from live `spell_template` rows
  `1784 -> 1785 -> 1786 -> 1787`, and local `playercreateinfo_spell` /
  `playercreateinfo_action` contain no Rogue starter row for this family.
- Rogue `Backstab` also has no local `spell_chain` row in this dump; rank
  backing comes from live `spell_template` rows
  `53 -> 2589 -> 2590 -> 2591 -> 8721 -> 11279 -> 11280 -> 11281`.
  The live row carries `SPELL_ATTR_SS_FACING_BACK`, a dagger-only
  `equipped_item_subclass_mask`, and a main-hand weapon gate, all of which the
  generic cast path now proves through focused tests.
- Rogue `Sprint` also has no local `spell_chain` row in this dump; rank/level
  backing comes from live `spell_template` rows
  `2983 -> 8696 -> 11305` at levels `10 -> 34 -> 58`. Duration backing comes
  from local `SpellDuration.dbc` entry `15_000 ms`, and the live row uses a
  generic self-targeted `SPELL_AURA_MOD_INCREASE_SPEED` aura plus a 300s spell
  cooldown.
- Rogue `Slice and Dice` cast rows `5171 -> 6774` are not generic spell rows in
  this dump: the live family entry point is still a dummy/script wrapper around
  hidden self-aura rows. The generic runtime beneath it now scales finisher
  aura durations from DBC base/max duration by combo points, accepts stored
  combo targets for self-target finishers, and clears combo points after
  successful non-damaging finisher aura application.
- Rogue `Evasion` rank/level backing comes from the packed
  `sql/base/dbc/original_data/Spell.sql` rows `5277 -> 15087` at levels
  `8 -> 50`; CMaNGOS handles it through generic aura `49`
  (`SPELL_AURA_MOD_DODGE_PERCENT`) in `src/game/Spells/SpellAuras.cpp`.
  The local `mangos` test DB row for `5277` omits the expected rank text and
  cooldown metadata, so focused live proof should key on the self-targeted
  dodge aura fields and `SpellDuration.dbc`, not the incomplete row metadata.
- Rogue `Kick` has no local `spell_chain` row in this dump; rank/level backing
  comes from live `spell_template` rows `1766 -> 1767 -> 1768 -> 1769` at
  levels `12 -> 26 -> 42 -> 58`. The live row is a generic hostile mixed melee
  interrupt: direct `SPELL_EFFECT_SCHOOL_DAMAGE` plus
  `SPELL_EFFECT_INTERRUPT_CAST`, DBC melee range index `2`, and duration index
  `28` for the interrupted-school lockout. Focused live proof now covers the
  generic interrupt plan/profile on the real row plus out-of-range failure
  before any interrupt impact, while the existing `Counterspell` runtime test
  remains the adjacent proof for the shared creature-cast lockout lane.
- Rogue `Expose Armor` has no local `spell_chain` row in this dump; rank/level
  backing comes from live `spell_template` rows
  `8647 -> 8649 -> 8650 -> 11197 -> 11198` at levels `14 -> 26 -> 36 -> 46 -> 56`.
  The live row closes on the generic hostile finisher-aura lane: effect 1 is a
  normal-school `SPELL_AURA_MOD_RESISTANCE` armor debuff that scales by combo
  points, while effect 2 remains a `SPELL_EFFECT_DUMMY` tail. CMaNGOS leaves
  `SPELLFAMILY_ROGUE` `Spell::EffectDummy` empty here, so that tail stays
  classified as inert/script-owned and does not block the generic closure.
  Focused live proof now covers the real row/profile/plan, the exact mixed
  coverage shape, and runtime combo-point spend/clear plus hostile armor-debuff
  aura application.
- Rogue `Rupture` uses the live rank chain
  `1943 -> 8639 -> 8640 -> 11273 -> 11274 -> 11275` at levels
  `20 -> 28 -> 36 -> 44 -> 52 -> 60`. The live row closes on the generic
  hostile finisher bleed lane: a melee-range `SPELL_AURA_PERIODIC_DAMAGE` aura
  with DBC combo-duration scaling, DBC base tick scaling by combo points, and
  a CMaNGOS family formula that adds `melee AP * min(combo points, 3) / 100`
  damage per tick. The generic aura value context now carries player melee
  attack power into aura construction, and focused live proof covers the real
  row/profile/plan, the capped AP bonus, combo-point spend/clear, and the
  no-direct-damage cast path.
- Mage `Mana Shield` rank 1 uses `SPELL_SCHOOL_MASK_NORMAL` in the local
  CMaNGOS spell row; keep the live row, not the broader synthetic helper, as
  the parity reference for this family.
- Local `spell_facing` rows for Mage direct-damage nukes can matter even when
  no serverside facing attribute is present; `Fire Blast` and `Scorch` both use
  facing flag `1`, and the generic cast-failure path already enforces it.
- Mage `Detect Magic` (`2855`) has no `spell_chain` row in this dump; treat it
  as a single-rank spell backed by aura `100` (`SPELL_AURA_AURAS_VISIBLE`).
- Local spell bonus coefficient ownership now supplements the pre-`z2817`
  `spell_template` schema by loading checked-in
  `sql/base/dbc/cmangos_fixes/Spell.sql` effect-coefficient fixes into live
  spell queries.
- Mage `Frost Ward` shares the same generic absorb + school-reflect runtime path
  as `Fire Ward`; the `Frost Warding` talent script in
  `src/game/Spells/Scripts/Scripting/ClassScripts/Mage.cpp` stays out of scope
  for this non-talent audit.
- Mage `Mage Armor` uses the live all-magic resistance mask `126` plus generic
  aura `134` for 30% interrupted mana regeneration; do not narrow it to arcane
  resistance.
- Mage `Ice Armor` uses generic self-targeted resistance auras for physical
  armor plus frost resistance and a live rank-specific chilled proc trigger; it
  does not require a Mage-family script for the non-talent baseline.
- Mage `Flamestrike` closes on the generic destination-AoE path as an
  instant-damage burst plus a persistent-area periodic-damage dynamic object;
  local trainer tables remain empty, so learn-path confirmation comes from the
  live rank chain `2120 -> 2121 -> 8422 -> 8423 -> 10215 -> 10216`.
- Mage `Teleport family` uses live `spell_target_position` rows plus
  `SPELL_EFFECT_TELEPORT_UNITS`; same-map rows resolve those database
  destinations on the near-teleport lane, and cross-map rows now emit generic
  worldport packets while moving the player runtime across maps.
- Mage `Portal family` uses live `SPELL_EFFECT_TRANS_DOOR` rows to spawn
  temporary `GAMEOBJECT_TYPE_SPELLCASTER` portals, and generic gameobject-use
  handling now casts the linked portal-effect teleport spell from the spawned
  portal object.
- Priest `Fade` uses the live rank chain
  `586 -> 9578 -> 9579 -> 9592 -> 10941 -> 10942` plus generic aura `103`
  (`SPELL_AURA_MOD_TOTAL_THREAT`) to apply a temporary per-creature threat
  reduction that is restored when the aura expires or is otherwise removed.
- Priest `Mind Blast` uses the live rank chain
  `8092 -> 8102 -> 8103 -> 8104 -> 8105 -> 8106 -> 10945 -> 10946 -> 10947`
  plus generic hostile `SPELL_EFFECT_SCHOOL_DAMAGE`; the live row carries an
  8s category cooldown on category `19`, and focused proof now covers both the
  generic spell-plan lane and runtime cooldown/damage packets.
- Priest `Power Word: Shield` uses the live rank chain
  `17 -> 592 -> 600 -> 3747 -> 6065 -> 6066 -> 10898 -> 10899 -> 10900 -> 10901`,
  checked-in 0.1 absorb coefficients from `sql/base/dbc/cmangos_fixes/Spell.sql`,
  and a CMaNGOS `AddPrecastSpell(6788)` follow-up; the Rust generic path now
  mirrors that with a linked `Weakened Soul` aura application plus explicit
  recast bounce while the lockout aura is active.
- Priest `Cure Disease` is a single-rank live row (`528`, level `14`) with no
  local `spell_chain` entry in this dump; the generic friendly dispel path now
  has focused proof for disease-only removal and `SPELL_FAILED_NOTHING_TO_DISPEL`.
- Priest `Dispel Magic` uses the live rank chain `527 -> 988`, a neutral
  `TARGET_UNIT` row, and the generic dispel effect; the mixed unit-target cast
  lane now validates friendly player targets and hostile creature
  range/attackability before the shared dispel execution path runs.
- Priest `Inner Fire` uses the live rank chain
  `588 -> 7128 -> 602 -> 1006 -> 10951 -> 10952`, a self-targeted
  `SPELL_AURA_MOD_RESISTANCE` armor aura, and live `spell_template` proc
  metadata (`procFlags = 680`, `procChance = 100`, `procCharges = 20`) with no
  local `spell_proc_event` override row; the generic aura builder must
  materialize the charge-only proc from the spell row itself, and runtime melee
  damage now consumes charges only on real damage, not on zero-damage hits.
- Priest `Levitate` (`1706`) has no local `spell_chain` row in this dump and
  uses a generic self-targeted aura package:
  `SPELL_AURA_FEATHER_FALL`, `SPELL_AURA_HOVER`, and
  `SPELL_AURA_WATER_WALK`. The generic aura builder now materializes hover as an
  explicit runtime modifier instead of leaving aura `106` pending.
- Priest `Shackle Undead` uses the live rank chain
  `9484 -> 9485 -> 10955` and a live `TargetCreatureType` mask of `32`
  (undead). The generic hostile-creature cast validation path now loads and
  enforces `spell_template.TargetCreatureType` before range and LOS checks.
- Priest `Mana Burn` uses the live rank chain
  `8129 -> 8131 -> 10874 -> 10875 -> 10876`, live `SPELL_EFFECT_POWER_BURN`
  effect `62`, `EffectMiscValue = 0` (mana), and `EffectMultipleValue = 0.5`.
  The generic hostile-creature path now classifies pure power-burn casts as
  hostile instant spells, validates target power type before cast completion,
  burns current creature mana first, broadcasts the power update, and routes
  the resulting shadow damage through the shared spell-damage lane.
- Priest `Holy Fire` uses the live rank chain
  `14914 -> 15262 -> 15263 -> 15264 -> 15265 -> 15266 -> 15267 -> 15261`
  and a generic hostile mixed spell path: direct `SPELL_EFFECT_SCHOOL_DAMAGE`
  followed by hostile `SPELL_AURA_PERIODIC_DAMAGE`. Focused live proof now
  covers the Priest spell-plan lane plus the DBC-backed cast-time/range/duration
  runtime, including direct damage, debuff aura application, and periodic tick
  packets against hostile creatures.
- Priest `Prayer of Healing` uses the live rank chain
  `596 -> 996 -> 10960 -> 10961 -> 25316`, local `cmangos_fixes` priest-heal
  `AttributesEx2` override rows, and a live direct-heal target of
  `TARGET_ENUM_UNITS_PARTY_WITHIN_CASTER_RANGE` (`20`). CMaNGOS still requires
  a friendly in-group cast target for validation, but the generic heal effect
  fans out over the caster-centered party radius from `SpellRadius.dbc`, heals
  the caster, and respects cast-time completion before impact.
- Priest `Prayer of Fortitude` uses the live rank chain `21562 -> 21564`,
  `SPELL_EFFECT_APPLY_AURA`, `SPELL_AURA_MOD_STAT`, and a live
  `TARGET_UNIT_FRIEND_AND_PARTY` (`37`) target. The generic aura lane now
  keeps friendly-unit cast validation while fanning the aura across the selected
  target's party membership using the live `SpellRadius.dbc` row, which is
  large enough to include a distant same-party member in the focused proof.
- Known generic follow-ups still open:
  caster-side spell healing bonus ownership still comes only from active auras;
  item/stat-backed healing bonus sources are not wired yet.
- Known generic follow-ups still open:
  target-side spell bonus coeff ownership after `Dampen Magic` /
  `Amplify Magic`.
- Known generic follow-ups still open:
  offensive `spell_bonus_data` rows such as `Holy Fire` are not yet surfaced
  through live `effect_bonus_coefficient*` query fields, so these audit
  closures currently prove base spell behavior rather than stat-scaled bonus
  damage.

## Warrior

- State: scoped audit complete for the current non-talent pass.
- Done:
  `Battle Shout`, `Heroic Strike`, `Rend`, `Charge`, `Thunder Clap`,
  `Hamstring`, `Defensive Stance`, `Sunder Armor`, `Revenge`, `Taunt`,
  `Shield Bash`, `Overpower`, `Demoralizing Shout`, `Cleave`, `Disarm`,
  `Shield Block`, `Berserker Stance`, `Intercept`, `Berserker Rage`, `Slam`,
  `Retaliation`, `Recklessness`, `Shield Wall`
- Fixed generically during audit:
  `Defensive Stance`, `Sunder Armor`, `Revenge`, `Taunt`, `Shield Bash`,
  `Overpower`, `Demoralizing Shout`, `Cleave`, `Disarm`, `Shield Block`,
  `Berserker Stance`, `Intercept`, `Berserker Rage`, `Retaliation`,
  `Recklessness`
- Proof-only / small proof closures:
  `Battle Shout`, `Heroic Strike`, `Rend`, `Charge`, `Thunder Clap`,
  `Hamstring`, `Slam`, `Shield Wall`
- Deferred:
  `Execute` (script-owned), `Berserker Rage` damage-taken rage bonus
  (family-specific follow-up)
- Next: none

## Mage

- State: scoped audit complete for the current non-talent pass.
- Done:
  `Frost Armor`, `Fireball`, `Conjure Water`, `Frostbolt`,
  `Arcane Intellect`, `Fire Blast`, `Conjure Food`, `Arcane Explosion`,
  `Remove Lesser Curse`, `Blink`, `Frost Nova`, `Polymorph`,
  `Arcane Missiles`, `Counterspell`, `Dampen Magic`, `Amplify Magic`,
  `Evocation`, `Blizzard`, `Mana Shield`, `Scorch`, `Cone of Cold`,
  `Detect Magic`, `Fire Ward`, `Frost Ward`, `Mage Armor`, `Ice Armor`,
  `Flamestrike`, `Teleport family`, `Portal family`
- Fixed generically during audit:
  `Remove Lesser Curse`, `Blink`, `Polymorph`, `Arcane Missiles`,
  `Dampen Magic`, `Detect Magic`, `Fire Ward`, `Teleport family`,
  `Portal family`
- Proof-only / small proof closures:
  `Frost Armor`, `Fireball`, `Conjure Water`, `Frostbolt`,
  `Arcane Intellect`, `Fire Blast`, `Conjure Food`, `Arcane Explosion`,
  `Frost Nova`, `Counterspell`, `Amplify Magic`, `Evocation`, `Blizzard`,
  `Mana Shield`, `Scorch`, `Cone of Cold`, `Frost Ward`, `Mage Armor`,
  `Ice Armor`, `Flamestrike`
- Next: none

## Priest

- State: scoped audit complete for the current non-talent pass.
- Done:
  `Lesser Heal`, `Heal`, `Power Word: Fortitude`, `Shadow Word: Pain`,
  `Smite`, `Renew`, `Power Word: Shield`, `Fade`, `Mind Blast`,
  `Cure Disease`, `Dispel Magic`, `Inner Fire`, `Levitate`,
  `Shackle Undead`, `Mana Burn`, `Holy Fire`, `Prayer of Healing`,
  `Flash Heal`, `Greater Heal`, `Prayer of Fortitude`
- Fixed generically during audit:
  `Power Word: Shield`, `Fade`, `Dispel Magic`, `Inner Fire`, `Levitate`,
  `Shackle Undead`, `Mana Burn`, `Prayer of Healing`,
  `Prayer of Fortitude`
- Proof-only / small proof closures:
  `Lesser Heal`, `Heal`, `Power Word: Fortitude`, `Shadow Word: Pain`,
  `Smite`, `Renew`, `Mind Blast`, `Cure Disease`, `Holy Fire`,
  `Flash Heal`, `Greater Heal`
- Deferred: none
- Next: none

## Rogue

- State: scoped audit complete for the current non-talent pass.
- Done:
  `Sinister Strike`, `Eviscerate`, `Stealth`, `Backstab`, `Pick Pocket`,
  `Gouge`, `Sap`, `Sprint`, `Evasion`, `Kick`, `Expose Armor`, `Garrote`,
  `Feint`, `Rupture`, `Blind`, `Cheap Shot`, `Kidney Shot`, `Distract`
- Fixed generically during audit:
  `Stealth`, `Pick Pocket`, `Sap`, `Evasion`, `Garrote`, `Feint`, `Rupture`,
  `Blind`, `Cheap Shot`, `Kidney Shot`, `Distract`
- Proof-only / small proof closures:
  `Sinister Strike`, `Eviscerate`, `Backstab`, `Gouge`, `Sprint`, `Kick`,
  `Expose Armor`
- Notes:
  `Pick Pocket` uses live rank chain `921 -> 5167`, DBC range index `2`
  (`0..5 yd`, melee flag), `SPELL_EFFECT_PICKPOCKET` (`71`), and
  `PickpocketLootId` / `pickpocketing_loot_template` backing. The generic
  runtime now loads pickpocket loot rows, opens `CLIENT_LOOT_PICKPOCKETING`,
  applies the CMaNGOS money formula and 600s restock cooldown, and returns
  `LOOT_ERROR_ALREADY_PICKPOCKETED` on recast before restock.
  `Gouge` has no local `spell_chain` rows in this dump; rank/level backing
  comes from live `spell_template` rows
  `1776 -> 1777 -> 8629 -> 11285 -> 11286` at levels
  `6 -> 18 -> 32 -> 46 -> 60`. The live row uses a generic hostile mixed lane:
  direct damage, a break-on-damage hard-control aura, and combo-point gain. It
  also carries a generic main-hand weapon gate, which the focused live proof
  now covers with an equipped dagger plus melee-range DBC data.
  `Sap` uses live rank/level backing from
  `spell_template` rows `6770 -> 2070 -> 11297` at levels `10 -> 28 -> 48`;
  the row uses `SPELL_AURA_MOD_STUN`, humanoid-only `TargetCreatureType = 64`,
  `AURA_INTERRUPT_FLAG_DAMAGE`, `SPELL_ATTR_ONLY_STEALTHED`, and
  `SPELL_ATTR_EX_ONLY_PEACEFUL_TARGETS`. The generic cast path now enforces the
  stealth and peaceful-target flags even for melee hostile-unit spells, and the
  focused live proof covers stealth failure, target-in-combat failure, energy
  spend, hostile knockout aura application, and generic
  `SPELL_ATTR_HEARTBEAT_RESIST` early-break runtime. Heartbeat chance now seeds
  from the live hostile target outcome, ticks on the map-owned aura runtime,
  and cleans up with the aura. The CMaNGOS target-stealth strip in
  `src/game/Spells/Scripts/Scripting/ClassScripts/Rogue.cpp` stays classified
  as script-owned.
  `Sprint` uses live rank/level backing from `spell_template` rows
  `2983 -> 8696 -> 11305` at levels `10 -> 34 -> 58`; the local dump exposes
  no `spell_chain` rows for this family. The live row is a generic self-cast
  `SPELL_EFFECT_APPLY_AURA` with `SPELL_AURA_MOD_INCREASE_SPEED`, and focused
  proof now covers the DBC `SpellDuration.dbc` 15s duration, positive
  movement-speed multiplier, owner run-speed packet, and 300s spell cooldown.
  `Slice and Dice` is not a fully generic cast row in the local dump. The
  player-facing cast rows `5171 -> 6774` are still dummy/script wrappers, while
  hidden helper rows carry the actual self-aura payload. This run corrected the
  stale Rogue ordering that had skipped the family, then landed the missing
  generic finisher self-aura support underneath it: DBC base/max aura duration
  now scales by combo points, self-target finishers accept the stored combo
  target, and non-damaging finisher aura casts now clear combo points on
  success. Focused proof covers the generic scaling rule, the combo-clear cast
  path, the adjacent `Eviscerate` damage finisher regression, and live-row
  classification that keeps the cast wrapper itself out of scope.
  `Evasion` closes on the generic self-aura lane: the runtime now maps
  `SPELL_AURA_MOD_DODGE_PERCENT` into a typed aura stat modifier, applies that
  dodge delta when recomputing effective combat stats, and marks aura `49` as
  implemented in spell coverage. Focused proof covers the generic stat
  application directly plus the live rank-1 row `5277`, its self-targeted
  apply-aura plan, and the DBC-backed 15s duration from `SpellDuration.dbc`.
  The local `mangos` test DB row still leaves rank text empty and cooldown at
  `0`, so packed `Spell.sql` remains the authoritative parity source for those
  metadata fields in this family.
  `Kick` closes as a proof-backed generic family with no runtime changes. The
  live rank-1 row `1766` is not a pure interrupt row: it still carries direct
  `SPELL_EFFECT_SCHOOL_DAMAGE` in effect 1 and
  `SPELL_EFFECT_INTERRUPT_CAST` in effect 2, so the shared generic lane is the
  same mixed melee-interrupt path already used by `Shield Bash`. Focused proof
  now covers the live row/profile/plan, confirms the missing local
  `spell_chain` row, and proves that an out-of-range cast fails before spending
  energy or interrupting the creature's active cast. The adjacent
  `Counterspell` runtime regression remains the proof for the shared
  creature-cast school-lockout execution path.
  `Expose Armor` closes as a proof-backed generic family with no runtime
  changes. The live rank-1 row `8647` is a hostile melee finisher whose real
  gameplay payload is effect 1: a generic `SPELL_AURA_MOD_RESISTANCE` armor
  debuff on the normal-school mask, with `-80` armor per combo point and a
  fixed 30s DBC duration. The same live row still carries effect 2 as
  `SPELL_EFFECT_DUMMY`, but CMaNGOS does not implement a Rogue `EffectDummy`
  branch for this family, so the audit treats that tail as inert/script-owned
  while closing the generic finisher aura path. Focused live proof now covers
  the real rank chain, mixed coverage classification, and runtime combo-point
  spend/clear plus hostile debuff aura application.
  `Garrote` has no local `spell_chain` row in this dump; rank/level backing
  comes from live `spell_template` rows
  `703 -> 8631 -> 8632 -> 8633 -> 11289 -> 11290` at levels
  `14 -> 22 -> 30 -> 38 -> 46 -> 54`. The live row is a stealth-only hostile
  bleed opener, not a silence spell: it uses a generic periodic-damage aura
  plus `SPELL_EFFECT_ADD_COMBO_POINTS`, with the behind-target requirement
  carried by the CMaNGOS flag pair
  `SPELL_ATTR_EX_INITIATES_COMBAT_ENABLES_AUTO_ATTACK` +
  `SPELL_ATTR_EX2_INITIATE_COMBAT_POST_CAST` rather than only the
  serverside-facing bit. The generic cast path now derives that behind-target
  rule from the live row and only dispatches combo-point gain after a landed
  hostile damage or aura application while still taking the combo-point count
  from the effect's own calculated value. Focused live proof covers the real
  row/profile, behind-target failure, stealth-energy spend, hostile bleed aura
  application, and combo-point gain.
  `Feint` closes on the generic hostile threat lane. The live rank backing
  comes from `spell_template` rows
  `1966 -> 6768 -> 8637 -> 11303 -> 25302` at levels
  `16 -> 28 -> 40 -> 52 -> 60`; this local DB still exposes no `spell_chain`
  row for rank 1, so rank proof comes from the live rows plus packed
  `Spell.sql`. The live row is a pure `SPELL_EFFECT_THREAT` (`63`) melee-range
  energy ability with a calculated rank-1 threat delta of `-150`. The generic
  spell runtime now dispatches hostile threat-only effects through the shared
  creature threat table, and that table now clamps negative deltas at zero
  instead of dropping them. Focused proof covers the real row/profile/plan,
  zero-damage hostile cast execution, threat reduction on the caster's entry,
  and aggro switching to a higher-threat rival, with Priest `Fade` kept green
  as the adjacent threat regression.
  `Blind` uses packed CMaNGOS `Spell.sql` rank backing
  `2094 -> 21060` at levels `34 -> 42`; this local `mangos` dump keeps the
  `21060` row present but leaves its metadata degraded, so focused live proof
  keys on the real rank-1 row `2094` plus rank-2 id presence. It closes on the
  generic hostile control-aura lane with `AURA_INTERRUPT_FLAG_DAMAGE`, no
  direct damage payload, and Rogue family flag `0x0100_0000`. The real generic gap
  here was diminishing-return ownership. CMaNGOS classifies Rogue `Blind` into
  `DIMINISHING_BLIND`; the Rust runtime now maps that Rogue family flag to its
  own `Blind` diminishing bucket instead of incorrectly sharing the Mage
  `Polymorph` bucket. Focused live proof covers the real row/profile/plan and
  verifies that a live `Blind` cast on a creature keeps full duration even when
  an existing Polymorph DR state is already primed on the target.
  `Cheap Shot` uses live rank ids `1833 -> 8621 -> 11293 -> 11294`, but this
  local `mangos` dump degrades the later rank metadata to
  `26 -> 38 -> 46 -> 54` with blank / stale rank text and drops the standard
  Rogue opener attr pair from the live `spell_template` row. Packed CMaNGOS
  `Spell.sql` still keeps those generic opener bits, so spell-template loading
  now restores
  `SPELL_ATTR_EX_INITIATES_COMBAT_ENABLES_AUTO_ATTACK` +
  `SPELL_ATTR_EX2_INITIATE_COMBAT_POST_CAST` for the full live Cheap Shot rank
  family. That lets the shared melee cast path enforce `SPELL_FAILED_NOT_BEHIND`
  through the normal generic behind-target gate instead of treating Cheap Shot
  like an unrestricted front-facing stun. Focused live proof now covers the
  degraded row/profile, missing-weapon failure, behind-target failure, and
  landed stealth stun/combo-point execution.
  `Kidney Shot` closes on the generic hostile finisher stun lane. CMaNGOS
  classifies Rogue family flag `0x0020_0000` into its own Kidney Shot
  diminishing-return bucket, and the Rust runtime now maps that flag to
  `DiminishingGroupRuntime::KidneyShot` instead of leaving the family unowned.
  This local `mangos` dump exposes no `spell_chain` row, empty trainer tables,
  duplicate/degraded name rows `6735 -> 8644 -> 408 -> 8643 -> 27615`, and a
  variant DBC duration row, so focused proof keys on the real live rank-1 row
  `408`, confirms the level-50 rank-2 presence, and exercises the actual cast
  runtime instead of overfitting to dump-specific metadata drift. Focused live
  proof now covers the generic hostile stun plan/profile, weapon gate, combo
  point spend/clear, landed stun aura duration through the loaded DBC row, and
  target DR registration/clear. The same live row still carries one secondary
  `SPELL_EFFECT_DUMMY` tail, so that side effect remains classified as
  script-owned rather than a generic spell blocker.
  `Distract` uses the live row `1725` at level `22` plus a hidden helper row
  `1728`; this dump exposes no `spell_chain` or trainer rows for the family.
  CMaNGOS treats effect `69` as a generic destination-area utility spell: the
  cast is allowed when the destination has no nearby attackable units or at
  least one nearby peaceful unit, and it fails with `SPELL_FAILED_TARGET_IN_COMBAT`
  only when every nearby attackable target is already in combat. The Rust
  generic runtime now classifies `SPELL_EFFECT_DISTRACT`, resolves its hostile
  destination area from DBC targets and `SpellRadius.dbc`, stops only
  out-of-combat random/waypoint creature motion, turns those creatures toward
  the chosen point with classic monster-move facing-spot packets, and delays
  their next idle motion by the live effect duration.
- Deferred / script-owned:
  `Slice and Dice` cast wrapper rows `5171 -> 6774`,
  `Vanish` cast wrapper rows `1856 -> 1857`
- Next: none

## Warlock

- State: in progress.
- Done:
  `Shadow Bolt`, `Demon Skin`, `Immolate`, `Corruption`,
  `Curse of Weakness`, `Curse of Agony`, `Summon Imp`, `Drain Soul`, `Fear`,
  `Healthstone use path`, `Eye of Kilrogg`, `Summon Voidwalker`,
  `Searing Pain`, `Create Firestone`, `Unending Breath`
- Fixed generically during audit:
  `Demon Skin`, `Curse of Agony`, `Summon Imp`, `Drain Soul`, `Fear`,
  `Healthstone use path`, `Eye of Kilrogg`, `Unending Breath`
- Proof-only / small proof closures:
  `Shadow Bolt`, `Immolate`, `Corruption`, `Curse of Weakness`,
  `Summon Voidwalker`, `Searing Pain`, `Create Firestone`
- Notes:
  `Shadow Bolt` uses the live rank chain
  `686 -> 695 -> 705 -> 1088 -> 1106 -> 7641 -> 11659 -> 11660 -> 11661 -> 25307`
  with local `spell_chain` backing and starter learn/action rows in
  `playercreateinfo_spell` / `playercreateinfo_action`. Rank 1 stays a generic
  hostile `SPELL_EFFECT_SCHOOL_DAMAGE` cast-time nuke with no aura tail, and
  focused live proof now covers both its player spell-plan classification and
  its runtime projectile timing: mana spend and `SMSG_SPELL_GO` happen at cast
  completion, while the actual shadow damage lands later on the missile-impact
  event without falling back to melee attacker-state packets.
  `Demon Skin` uses the live two-rank family `687 -> 696`, with starter learn
  and action rows for rank 1 and no trainer backing in this local dump. The
  live row is a generic self-aura pair: physical armor via
  `SPELL_AURA_MOD_RESISTANCE` and flat health regeneration through aura `161`
  (`SPELL_AURA_MOD_HEALTH_REGEN_IN_COMBAT`). The generic runtime now maps aura
  `161` into the shared player health-regen tick instead of misclassifying it
  as a periodic HoT, and focused proof covers both the live spell-plan closure
  and in-combat health gain from the applied aura.
  `Immolate` uses the live rank chain
  `348 -> 707 -> 1094 -> 2941 -> 11665 -> 11667 -> 11668 -> 25309` at levels
  `1 -> 8 -> 16 -> 24 -> 32 -> 40 -> 48 -> 58`. This local dump has no
  `playercreateinfo_spell`, `playercreateinfo_action`, `npc_trainer`, or
  `npc_trainer_template` rows for the family, so learn-path proof comes from
  the live chain plus spell rows. Rank 1 closes on the generic hostile mixed
  spell lane: direct `SPELL_EFFECT_SCHOOL_DAMAGE` plus hostile
  `SPELL_AURA_PERIODIC_DAMAGE`, with live DBC cast-time, range, duration, and
  3000 ms tick amplitude data. Focused live proof now covers both the generic
  spell-plan closure and the runtime sequence where cast completion spends
  mana, emits direct spell damage, applies the hostile debuff aura, and then
  ticks periodic damage against a creature target.
  `Corruption` uses the live rank chain
  `172 -> 6222 -> 6223 -> 7648 -> 11671 -> 11672 -> 25311`. This local dump
  has no Warlock starter or trainer rows for the family, so learn-path proof
  comes from `spell_chain`, live `spell_template` rows, and the local
  `spell_bonus_data` coefficient row for spell `172`. Rank 1 closes on the
  generic pure hostile periodic-damage aura lane: a single
  `SPELL_EFFECT_APPLY_AURA` with `SPELL_AURA_PERIODIC_DAMAGE`,
  DBC-backed duration/range, 3000 ms tick amplitude, no direct-damage payload,
  and hostile debuff presentation. Focused live proof now covers both the
  generic spell-plan closure and the runtime sequence where the cast spends
  mana, applies only the hostile debuff aura, and deals damage on later
  periodic ticks.
  `Curse of Weakness` uses the live rank chain
  `702 -> 1108 -> 6205 -> 7646 -> 11707 -> 11708`. Rank 1 closes on an
  existing generic hostile debuff lane with no runtime changes: effect 1 is a
  plain `SPELL_AURA_MOD_DAMAGE_DONE` aura carrying negative physical-damage
  deltas, and the Rust runtime already routes that modifier through shared
  attack-power / damage-done recomputation. The existing `Demoralizing Shout`
  focused proof remains the adjacent coverage for this shared lane.
  `Curse of Agony` uses the live rank chain
  `980 -> 1014 -> 6217 -> 11711 -> 11712 -> 11713` at levels
  `8 -> 18 -> 28 -> 38 -> 48 -> 58`. This local dump has no Warlock starter
  or trainer rows for the family, so learn-path proof comes from
  `spell_chain`, live `spell_template` rows, and the CMaNGOS
  `CurseOfAgony` aura script in
  `src/game/Spells/Scripts/Scripting/ClassScripts/Warlock.cpp`. Rank 1 closes
  on a now-shared phased hostile periodic-damage lane: a single
  `SPELL_EFFECT_APPLY_AURA` with `SPELL_AURA_PERIODIC_DAMAGE`,
  DBC-backed duration/range, 2000 ms tick amplitude, and runtime-owned tick
  phasing where ticks `1..4` use half damage, ticks `5..8` use normal damage,
  and ticks `9..12` use `3/2` damage. Focused live proof now covers both the
  generic spell-plan closure and the runtime sequence where later periodic
  ticks ramp through the low / normal / high phases.
  `Summon Imp` uses live spell `688`, effect `56`
  (`SPELL_EFFECT_SUMMON_PET`), and creature entry `416`, while this local dump
  exposes no `spell_chain` row for the family. CMaNGOS handles it through the
  generic summon-pet effect path in `Spell::EffectSummonPet`; the Rust runtime
  now does the same by classifying effect `56`, routing it through a generic
  player summon-pet dispatch, materializing a player-owned `HighGuid::Pet`
  runtime creature with owner/created-by update fields, replacing any existing
  owned summon first, and broadcasting the create/destroy object packets
  through the shared map/session path.
  `Drain Soul` uses the live rank chain `1120 -> 8288 -> 8289 -> 11675`; this
  local dump also keeps starter and trainer tables empty for the family, so
  learn-path confirmation comes from `spell_chain`, live `spell_template`
  rows, and CMaNGOS `SPELL_AURA_CHANNEL_DEATH_ITEM` handling in
  `src/game/Spells/SpellAuras.cpp`. Rank 1 stays on the generic hostile-unit
  channel lane: effect 1 is `SPELL_EFFECT_APPLY_AURA` with aura `86`
  (`SPELL_AURA_CHANNEL_DEATH_ITEM`), `effect_item_type1 = 6265` (Soul Shard),
  and hostile unit targeting. The generic blockers were target-channel
  ownership plus reward ownership for lethal channel ticks. The Rust runtime
  now classifies channeled hostile target auras onto a dedicated generic
  player target-channel lane, starts the classic channel packets/update fields,
  removes the target aura when movement cancels the channel, queues channel-kill
  `DbCreatureDeathFinalizationEvent`s in map runtime, drains them from the
  session loop into the existing session-owned `finalize_db_creature_death`
  path, parses aura `86` as a generic channel-death-item effect, awards one
  inventory item per caster/item pair when the tapped kill passes a dedicated
  CMaNGOS-style honor/XP-target gate derived from the effective creature level
  (currently `max_level` in this runtime) instead of piggybacking on the
  broader XP-reward formula, and suppresses duplicate same-caster shard awards
  from stacked identical auras. Focused proof covers the live rank-1 row,
  target-channel start plus movement cleanup, the live-cast Soul Shard create
  flow on a variable-level creature, the deduped shard reward case, and the
  deferred death-finalization packet handoff for lethal player channels.
  `Fear` uses the live rank chain `5782 -> 6213 -> 6215`; this local dump has
  no trainer rows for the family, so learn-path confirmation comes from
  `spell_chain`, live `spell_template` rows, and the CMaNGOS
  `SPELL_AURA_MOD_FEAR` runtime in `src/game/Spells/SpellAuras.cpp` plus the
  Warlock fear diminishing classification in `src/game/Spells/SpellMgr.cpp`.
  Rank 1 stays a generic hostile control aura with `SPELL_ATTR_HEARTBEAT_RESIST`,
  not a script-owned family. The missing lane was map-owned creature control:
  fear already blocked spellcasts and suppressed hostile refs, but it never
  claimed creature flee motion. The generic runtime now treats fear as a
  control-state sibling to confuse, starts the existing flee-motion lane on
  hostile creature aura application, updates creature fleeing flags from either
  active fear auras or flee motion, and resumes normal idle scheduling when the
  control aura clears. Focused proof covers the live rank-1 row plus immediate
  generic flee-motion ownership on a feared creature, with Berserker Rage fear
  immunity kept green as the adjacent regression slice.
  `Create Healthstone` uses the live rank chain
  `6201 -> 6202 -> 5699 -> 11729 -> 11730`, but the player-facing create rows
  are not generic spell rows in this dump: rank 1 `6201` is
  `SPELL_EFFECT_SCRIPT_EFFECT` (`77`) with no direct `CreateItem` payload, and
  CMaNGOS owns the item-selection / duplicate-item checks in
  `src/game/Spells/Scripts/Scripting/ClassScripts/Warlock.cpp`
  (`CreateHealthStoneWarlock`). The family stays classified as script-owned for
  this audit pass.
  `Healthstone use path` closes on a small generic item-use planning fix, not a
  Warlock-family script. The live `Minor Healthstone` item `5512` exposes
  on-use spell `6262`, a pure `SPELL_EFFECT_HEAL` row targeting the caster.
  The generic item spell plan now classifies pure heal consumables as
  `DirectHeal` instead of folding them into `InstantDamage`, and item target
  normalization now keeps caster-target self-heal items on the same self-target
  lane as player direct heals. Focused live proof covers the actual Healthstone
  item-use row, self-heal runtime, item consumption, and heal-log packets,
  while the hostile item-damage regression stays green.
  `Eye of Kilrogg` uses live spell `126`, effect `73`
  (`SPELL_EFFECT_SUMMON_POSSESSED`), creature entry `4277`, and the CMaNGOS
  `Spell::EffectSummonPossessed` path in `src/game/Spells/SpellEffects.cpp`.
  The generic Rust runtime now classifies effect `73`, routes it through a
  shared player-owned runtime-creature summon helper, and spawns a
  player-owned `HighGuid::Unit` creature with owner / created-by update fields,
  player-controlled flags, and existing-owned-summon replacement semantics.
  The generic runtime also now closes the first private viewpoint lane under
  hidden passive `2585`: aura `1` (`SPELL_AURA_BIND_SIGHT`) / `76`
  (`SPELL_AURA_FAR_SIGHT`) map to a generic farsight modifier, creature-target
  aura apply / remove / despawn update per-player `PLAYER_FARSIGHT`, and
  focused proof covers both the modifier mapping and private update packets.
  The spell-family finish hook that actually casts hidden passive `2585` is
  now wired too: successful Eye summons apply the hidden bind-sight aura onto
  the owned summon through the same live creature-aura path, so the eye picks
  up private farsight ownership without a spell-specific aura lane. The
  generic possession handshake is now partially closed as well: possessed
  summons stamp `UNIT_FIELD_CHARMEDBY`, gain `UNIT_FLAG_POSSESSED`, mirror the
  summoned eye through the player's `UNIT_FIELD_CHARM`, emit classic
  `SMSG_CLIENT_CONTROL_UPDATE`, and allow `CMSG_SET_ACTIVE_MOVER` to accept
  the controlled unit guid while session state tracks that mover ownership.
  The shared movement path now routes client movement packets for the active
  controlled mover into the possessed eye's runtime creature position instead
  of mutating the Warlock player body, matching the next CMaNGOS
  `HandleMoverRelocation` ownership step for charmed creatures. This pass also
  closes the first generic camera-origin visibility lane under the same family:
  farsight target ownership now refreshes owner-only nearby-player visibility,
  and creature / gameobject / corpse streaming now uses the active farsight
  origin during controlled Eye movement instead of the Warlock body position.
  The final generic blocker was cleanup / unpossess parity underneath
  `Unit::Uncharm`: session-side reconciliation now clears stale controlled-unit
  ownership when the possessed eye despawns or loses its charmer, restores the
  active mover to the player body, clears the player's charm field, revokes
  `SMSG_CLIENT_CONTROL_UPDATE`, and reuses the shared farsight-clear path so
  owner visibility returns to the player body without a spell-specific hook.
  `Summon Voidwalker` uses live spell `697`, effect `56`
  (`SPELL_EFFECT_SUMMON_PET`), creature entry `1860`, and no local
  `spell_chain` row in this dump. CMaNGOS handles it through the same generic
  summon-pet effect path as `Summon Imp`; the existing Rust summon-pet runtime
  already covers the player-owned pet create/replace ownership boundary, so
  this family closes as proof-only on the established lane.
  `Searing Pain` uses local `spell_chain` rank backing
  `5676 -> 17919 -> 17920 -> 17921 -> 17922 -> 17923` with a generic hostile
  `SPELL_EFFECT_SCHOOL_DAMAGE` row. It closes as proof-only on the same direct
  hostile spell-damage lane already covered by adjacent Warlock/Mage nukes.
  `Create Firestone` uses local `spell_chain` rank backing
  `6366 -> 17951 -> 17952 -> 17953` and stays on the generic
  `SPELL_EFFECT_CREATE_ITEM` lane already proven by `Conjure Water` /
  `Conjure Food`; no Warlock-family script owns the create flow in CMaNGOS, so
  the family closes as proof-only for this audit.
  `Unending Breath` uses live spell `5697`, has no local `spell_chain` row in
  this dump, and applies aura `82` (`SPELL_AURA_WATER_BREATHING`). CMaNGOS
  treats that as a generic player mirror-timer modifier in
  `SpellAuras.cpp` / `Player.cpp`; the Rust runtime now maps aura `82` into an
  explicit water-breathing modifier, marks the aura implemented in coverage,
  and stops the drowning mirror timer while the aura is active.
  `Hellfire` uses the live rank chain `1949 -> 11683 -> 11684`, and rank 1 is
  not a direct persistent-area row in this dump: `1949` is a self
  `SPELL_AURA_PERIODIC_TRIGGER_SPELL` wrapper that ticks trigger spell `5857`
  every 1000 ms. A separate generic fix now lets caster-centered
  persistent-area effects derive their origin from the caster position instead
  of requiring a client destination, and focused synthetic proof for that lane
  is green. Live `Hellfire` remains the first in-scope blocker because the
  missing generic lane is still the self periodic-trigger hostile-AoE channel
  path owned by the wrapper row, not the persistent-area origin path.
- Deferred:
  `Life Tap` (`1454 -> 1455 -> 1456 -> 11687 -> 11688 -> 11689`) remains
  script-owned for this audit pass because the live family entry point still
  uses `SPELL_EFFECT_DUMMY`, and CMaNGOS owns the conversion behavior in
  `src/game/Spells/Scripts/Scripting/ClassScripts/Warlock.cpp`.
  `Create Healthstone` (`6201 -> 6202 -> 5699 -> 11729 -> 11730`) remains
  script-owned for this audit pass because the live family entry point still
  uses `SPELL_EFFECT_SCRIPT_EFFECT`, and CMaNGOS owns the item-selection and
  duplicate-item checks in
  `src/game/Spells/Scripts/Scripting/ClassScripts/Warlock.cpp`.
- Next:
  `Hellfire`

## Paladin

- State: untouched.
- Done: none
- Fixed generically during audit: none
- Deferred: talent-only exclusions still apply
- Next:
  `Devotion Aura`

## Druid

- State: untouched.
- Done: none
- Fixed generically during audit: none
- Deferred: none
- Next:
  `Wrath`

## Shaman

- State: untouched.
- Done: none
- Fixed generically during audit: none
- Deferred: none
- Next:
  `Healing Wave`

## Hunter

- State: untouched.
- Done: none
- Fixed generically during audit: none
- Deferred:
  pet talents and deep pet-family combat behavior stay out of scope for now
- Next:
  `Auto Shot / Shoot path`
