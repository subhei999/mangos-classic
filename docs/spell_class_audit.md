# Spell Class Audit

Durable tracker for the class-by-class generic spell audit. Keep this file
compact. Record what is done, what required generic runtime work, what is
deferred, and the next in-order blocking family. Do not turn this back into a
run log.

## Global Notes

- Current class: `Priest`
- Current next blocking family: `Prayer of Healing`
- Use local `spell_chain`, live spell rows, and CMaNGOS as the behavior
  reference.
- Local `npc_trainer` / `npc_trainer_template` data is empty in this dump, so
  learn-path confirmation often comes from spell-chain and live-row evidence.
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

- State: in progress.
- Done:
  `Lesser Heal`, `Heal`, `Power Word: Fortitude`, `Shadow Word: Pain`,
  `Smite`, `Renew`, `Power Word: Shield`, `Fade`, `Mind Blast`,
  `Cure Disease`, `Dispel Magic`, `Inner Fire`, `Levitate`,
  `Shackle Undead`, `Mana Burn`, `Holy Fire`
- Fixed generically during audit:
  `Power Word: Shield`, `Fade`, `Dispel Magic`, `Inner Fire`, `Levitate`,
  `Shackle Undead`, `Mana Burn`
- Proof-only / small proof closures:
  `Lesser Heal`, `Heal`, `Power Word: Fortitude`, `Shadow Word: Pain`,
  `Smite`, `Renew`, `Mind Blast`, `Cure Disease`, `Holy Fire`
- Deferred: none
- Next:
  `Prayer of Healing`

## Rogue

- State: untouched.
- Done: none
- Fixed generically during audit: none
- Deferred: none
- Next:
  `Sinister Strike`

## Warlock

- State: untouched.
- Done: none
- Fixed generically during audit: none
- Deferred: none
- Next:
  `Shadow Bolt`

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
