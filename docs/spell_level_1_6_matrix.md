# Level 1-6 Spell Matrix

Current operating matrix for moving all early class spells through the generic,
map-owned spell engine. This is intentionally focused on class starter/trainer
spells, not every generic language, skill, opening, duel, stuck, racial, or
profession row that appears in `playercreateinfo_spell`.

Generated from the local CMaNGOS-derived world DB on 2026-05-08:

- Starter source: `playercreateinfo_spell` joined to `spell_template`.
- Trainer source: `npc_trainer` joined through CMaNGOS-style learn-spell
  wrappers, using the actual `EffectTriggerSpell*` learned spell row.
- Reference enum source: `src/game/Spells/SpellEffectDefines.h`.
- Current Rust engine source: `crates/wow-network/src/world/spells.rs` and
  `crates/wow-network/src/world/spells/`.

## Current Engine Profiles

`SpellInfo::prepare_player_cast` currently admits these player profiles:

- `NextMeleeSwing`: on-next-swing attributes, currently used by Heroic Strike.
- `Charge`: `SPELL_EFFECT_CHARGE`.
- `DirectHeal`: `SPELL_EFFECT_HEAL`.
- `AuraApplication`: `SPELL_EFFECT_APPLY_AURA`.
- `InstantDamage`: `SPELL_EFFECT_SCHOOL_DAMAGE`, `SPELL_EFFECT_WEAPON_DAMAGE`,
  and the current Rust weapon-damage aliases.

The profile being admitted only means the cast enters the generic lifecycle. The
individual effect or aura modifier can still be incomplete.

## Class Matrix

| Class | Level | Spell | DB effect shape | Current status | Needed parity work |
|---|---:|---|---|---|---|
| Warrior | 1 | `78 Heroic Strike` | weapon damage, on-next-swing | Supported profile | Continue real-client validation of queued swing state, power recheck, and combat log order. |
| Warrior | 1 | `6673 Battle Shout` | apply aura, attack power | Supported profile/effect | Keep as regression baseline for friendly/self aura and GCD. |
| Warrior | 4 | `100 Charge` | charge, energize, trigger spell | Supported primary profile | Generic secondary `Energize`/`TriggerSpell` execution is still not complete for player spells. |
| Warrior | 4 | `772 Rend` | periodic damage aura | Supported profile/effect | Keep as regression baseline for negative aura cleanup on death/respawn. |
| Warrior | 6 | `3127 Parry` | `SPELL_EFFECT_PARRY` | Missing passive effect | Add passive combat capability effects: dodge/parry/block/defense/proficiency bootstrap. |
| Warrior | 6 | `6343 Thunder Clap` | school damage plus aura 138 | Partially admitted | Needs area target/radius handling and aura 138 attack-speed modifier. |
| Paladin | 1 | `635 Holy Light` | `SPELL_EFFECT_SCRIPT_EFFECT` | Missing | Implement CMaNGOS script-effect path for Paladin heal rows or source-derived mapping to heal behavior. |
| Paladin | 1 | `20154 Seal of Righteousness` | aura 4 proc/seal | Admitted, effect incomplete | Implement seal/proc aura family and triggered strike behavior. |
| Paladin | 1 | `465 Devotion Aura` | `SPELL_EFFECT_APPLY_AREA_AURA_PARTY` | Missing | Add party/self area aura profile and armor/resistance aura modifiers. |
| Paladin | 4 | `19740 Blessing of Might` | attack power aura | Supported profile/effect | Keep as friendly aura regression. |
| Paladin | 4 | `20271 Judgement` | `SPELL_EFFECT_SCRIPT_EFFECT` | Missing | Implement Judgement script-effect dispatch and seal interaction. |
| Paladin | 6 | `498 Divine Protection` | immunity auras 25/39 | Admitted, effect incomplete | Add immunity/school/mechanic aura handling and lockout rules. |
| Paladin | 6 | `639 Holy Light` | `SPELL_EFFECT_SCRIPT_EFFECT` | Missing | Same script-heal work as rank 1. |
| Paladin | 6 | `21082 Seal of the Crusader` | attack power, proc/seal auras | Admitted, effect incomplete | Same seal/proc aura work as Seal of Righteousness. |
| Hunter | 1 | `2973 Raptor Strike` | weapon percent damage, on-next-swing | Partially supported | Fix CMaNGOS weapon effect IDs and apply percent/bonus damage through queued swing. |
| Hunter | 1 | `1494 Track Beasts` | track creature aura | Supported profile/effect | Keep as non-combat self aura regression. |
| Hunter | 4 | `1978 Serpent Sting` | periodic damage aura | Supported profile/effect | Keep as ranged DoT/death cleanup regression. |
| Hunter | 4 | `13163 Aspect of the Monkey` | dodge aura 49 | Admitted, effect incomplete | Add dodge/stat-derived combat modifier aura. |
| Hunter | 6 | `1130 Hunter's Mark` | mark/attack-power auras 68/127 | Admitted, effect incomplete | Add hunter mark aura family and ranged AP bonus behavior. |
| Hunter | 6 | `3044 Arcane Shot` | school damage | Supported profile | Needs ranged validation/projectile coverage in smoke tests. |
| Rogue | 1 | `1752 Sinister Strike` | normalized weapon damage 121, combo point 80 | Supported profile/effect | Uses Energy, spell-tagged melee outcome, and map-owned combo point gain. |
| Rogue | 1 | `2098 Eviscerate` | school damage plus combo-point scaling | Supported profile/effect | Uses finishing-move attributes, `EffectPointsPerComboPoint`, Energy, and clears map-owned combo points on hit. |
| Rogue | 1 | `1784 Stealth` | stealth/speed auras 36/16/33 | Admitted, effect incomplete | Add stealth visibility, movement speed, and aura interrupt rules. |
| Rogue | 4 | `53 Backstab` | normalized weapon damage, weapon percent damage, combo point | Supported profile/effect | Uses corrected CMaNGOS weapon effect ids, Energy, combo point gain, and `AttributesServerside` behind-target validation. |
| Rogue | 4 | `921 Pick Pocket` | `SPELL_EFFECT_PICKPOCKET` | Missing | Add pickpocket target validation and loot path later. |
| Rogue | 6 | `1757 Sinister Strike` | normalized weapon damage 121, combo point 80 | Supported profile/effect | Same generic path as rank 1. |
| Rogue | 6 | `1776 Gouge` | school damage, combo point, incapacitate aura | Partially supported | Damage and combo-point gain use generic effects; incapacitate aura 12 behavior still needs movement/action lock rules. |
| Priest | 1 | `585 Smite` | school damage | Supported profile/effect | Keep as cast-time direct damage regression. |
| Priest | 1 | `2050 Lesser Heal` | heal | Supported profile/effect | Keep as direct heal regression. |
| Priest | 1 | `1243 Power Word: Fortitude` | stat aura | Supported profile/effect | Keep as friendly buff regression. |
| Priest | 4 | `589 Shadow Word: Pain` | periodic damage aura | Supported profile/effect | Keep as DoT cleanup regression. |
| Priest | 4 | `2052 Lesser Heal` | heal | Supported profile/effect | Keep as rank/cast-time heal regression. |
| Priest | 6 | `17 Power Word: Shield` | absorb aura 69 | Admitted, effect incomplete | Add absorb aura, damage consumption, and weakened-soul follow-up behavior. |
| Priest | 6 | `591 Smite` | school damage | Supported profile/effect | Keep as ranked direct damage regression. |
| Shaman | 1 | `403 Lightning Bolt` | school damage | Supported profile | Needs ranged/projectile validation coverage. |
| Shaman | 1 | `331 Healing Wave` | heal | Supported profile/effect | Keep as direct heal regression. |
| Shaman | 1 | `8017 Rockbiter Weapon` | temporary weapon enchant 54 | Missing | Add held/weapon temporary enchant effect and melee damage modifier. |
| Shaman | 4 | `8042 Earth Shock` | school damage, interrupt cast 68 | Partially supported | Add interrupt effect and school-lockout behavior. |
| Shaman | 6 | `332 Healing Wave` | heal | Supported profile/effect | Keep as rank/cast-time heal regression. |
| Shaman | 6 | `2484 Earthbind Totem` | summon totem slot 2 effect 88 | Missing | Add totem summon/object lifecycle and area aura tick behavior. |
| Mage | 1 | `133 Fireball` | school damage plus periodic damage aura | Supported profile/effect | Keep projectile impact, cast interruption, facing/LOS, and DoT cleanup as smoke regressions. |
| Mage | 1 | `168 Frost Armor` | resistance aura 22, power-cost aura | Admitted, effect incomplete | Add resistance/armor aura modifiers and attacker slow proc later. |
| Mage | 1 | `1459 Arcane Intellect` | stat aura | Supported profile/effect | Keep as friendly buff regression. |
| Mage | 4 | `116 Frostbolt` | slow aura 33 plus school damage | Partially supported | Add movement slow aura and projectile validation. |
| Mage | 4 | `5504 Conjure Water` | create item 24 | Missing | Add create-item spell effect, inventory insertion, and failure handling. |
| Mage | 6 | `143 Fireball` | school damage plus periodic damage aura | Supported profile/effect | Same as rank 1. |
| Mage | 6 | `587 Conjure Food` | create item 24 | Missing | Same create-item effect work. |
| Mage | 6 | `2136 Fire Blast` | school damage | Supported profile/effect | Keep as instant hostile damage regression. |
| Warlock | 1 | `686 Shadow Bolt` | school damage | Supported profile | Needs ranged/projectile validation coverage. |
| Warlock | 1 | `687 Demon Skin` | resistance/regen auras 22/161 | Admitted, effect incomplete | Add armor/resistance and regen aura modifiers. |
| Warlock | 1 | `348 Immolate` | periodic damage aura plus school damage | Supported profile/effect | Keep as direct-plus-DoT regression. |
| Warlock | 4 | `172 Corruption` | periodic damage aura | Supported profile/effect | Keep as DoT cleanup regression. |
| Warlock | 4 | `702 Curse of Weakness` | aura 13 | Admitted, effect incomplete | Add attack-power/damage reduction aura family. |
| Warlock | 6 | `695 Shadow Bolt` | school damage | Supported profile | Needs ranged/projectile validation coverage. |
| Warlock | 6 | `1454 Life Tap` | dummy effect 3 | Missing | Implement source-derived dummy effect dispatch for Life Tap. |
| Druid | 1 | `5176 Wrath` | school damage | Supported profile | Needs ranged/projectile validation coverage. |
| Druid | 1 | `5185 Healing Touch` | heal | Supported profile/effect | Keep as direct heal regression. |
| Druid | 1 | `1126 Mark of the Wild` | aura 22 | Admitted, effect incomplete | Add armor/resistance/stat aura modifiers. |
| Druid | 4 | `774 Rejuvenation` | periodic heal aura | Supported profile/effect | Keep as HoT periodic regression. |
| Druid | 4 | `8921 Moonfire` | periodic damage aura plus school damage | Supported profile/effect | Keep as instant direct-plus-DoT regression. |
| Druid | 6 | `467 Thorns` | damage shield aura 15 | Admitted, effect incomplete | Add damage shield proc on attackers. |
| Druid | 6 | `5177 Wrath` | school damage | Supported profile | Needs ranged/projectile validation coverage. |

## Effect Families To Add Next

Highest value implementation order for class-spell parity:

1. Script-effect bridge:
   `SPELL_EFFECT_SCRIPT_EFFECT` 77 for Paladin Holy Light/Judgement and other
   early scripted rows. Each supported script must be source-derived from
   CMaNGOS instead of guessed.
2. Create-item spells:
   `SPELL_EFFECT_CREATE_ITEM` 24 for Conjure Water/Food, using map-owned cast
   completion plus character inventory authority.
3. Aura modifier expansion:
   absorb 69, resistance/armor 22, stealth 16/36, slow 33, dodge 49, damage
   shield 15, attack-speed 138, hunter mark 68/127, seal/proc auras 4/9, and
   immunity auras 25/39.
4. Totem and temporary enchant effects:
   `SPELL_EFFECT_ENCHANT_ITEM_TEMPORARY` 54 for Rockbiter and summon totem slot
   effects 87-90 for Shaman totems.
5. Interrupt and utility effects:
   `SPELL_EFFECT_INTERRUPT_CAST` 68 for Earth Shock, `SPELL_EFFECT_PICKPOCKET`
   71, and passive capability effects such as dodge/parry/block/defense/
   proficiency.
6. Ranged/projectile proof:
   keep Fireball/Shadow Bolt/Wrath/Lightning Bolt/Arcane Shot/Frostbolt under
   real-client smoke so cast-start validation, completion revalidation,
   projectile impact, facing/LOS, and DoT cleanup stay together.

## Acceptance Definition

A row is production-ready only when all of these are true:

- `SpellInfo` admits it from DB/DBC metadata without a spell-id allowlist.
- Session code only parses/sends; map runtime owns power, health, auras,
  cooldowns, cast completion, and effects.
- Start/result/go packet order is covered by a focused test.
- The effect family has a CMaNGOS source reference or DB/DBC-backed formula.
- Real-client smoke confirms the button behavior, cast bar/GCD, power spend,
  target validation, combat log, and aura/cooldown UI.
