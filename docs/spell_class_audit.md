# Spell Class Audit

Durable tracker for the class-by-class generic spell audit. Keep this file
compact. Record what is done, what required generic runtime work, what is
deferred, and the next in-order blocking family. Do not turn this back into a
run log.

## Global Notes

- Current class: `Mage`
- Current next blocking family: `Blizzard`
- Use local `spell_chain`, live spell rows, and CMaNGOS as the behavior
  reference.
- Local `npc_trainer` / `npc_trainer_template` data is empty in this dump, so
  learn-path confirmation often comes from spell-chain and live-row evidence.
- Known generic follow-up still open:
  target-side spell bonus coeff ownership after `Dampen Magic` /
  `Amplify Magic`.

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

- State: in progress.
- Done:
  `Frost Armor`, `Fireball`, `Conjure Water`, `Frostbolt`,
  `Arcane Intellect`, `Fire Blast`, `Conjure Food`, `Arcane Explosion`,
  `Remove Lesser Curse`, `Blink`, `Frost Nova`, `Polymorph`,
  `Arcane Missiles`, `Counterspell`, `Dampen Magic`, `Amplify Magic`,
  `Evocation`
- Fixed generically during audit:
  `Remove Lesser Curse`, `Blink`, `Polymorph`, `Arcane Missiles`,
  `Dampen Magic`
- Proof-only / small proof closures:
  `Frost Armor`, `Fireball`, `Conjure Water`, `Frostbolt`,
  `Arcane Intellect`, `Fire Blast`, `Conjure Food`, `Arcane Explosion`,
  `Frost Nova`, `Counterspell`, `Amplify Magic`, `Evocation`
- Next:
  `Blizzard`
- Remaining queue after next:
  `Mana Shield`, `Scorch`, `Cone of Cold`, `Detect Magic`, `Fire Ward`,
  `Frost Ward`, `Mage Armor`, `Ice Armor`, `Flamestrike`,
  `Teleport family`, `Portal family`

## Priest

- State: untouched.
- Done: none
- Fixed generically during audit: none
- Deferred: none
- Next:
  `Lesser Heal`

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
