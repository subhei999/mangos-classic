use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::world) enum MeleeHitOutcome {
    Miss,
    Dodge,
    Block,
    Parry,
    Glancing,
    Crit,
    Crushing,
    Normal,
}

pub(in crate::world) const SKILL_DEFENSE: u16 = 95;
pub(in crate::world) const SKILL_SWORDS: u16 = 43;
pub(in crate::world) const SKILL_AXES: u16 = 44;
pub(in crate::world) const SKILL_BOWS: u16 = 45;
pub(in crate::world) const SKILL_GUNS: u16 = 46;
pub(in crate::world) const SKILL_MACES: u16 = 54;
pub(in crate::world) const SKILL_TWO_HANDED_SWORDS: u16 = 55;
pub(in crate::world) const SKILL_STAVES: u16 = 136;
pub(in crate::world) const SKILL_TWO_HANDED_MACES: u16 = 160;
pub(in crate::world) const SKILL_UNARMED: u16 = 162;
pub(in crate::world) const SKILL_TWO_HANDED_AXES: u16 = 172;
pub(in crate::world) const SKILL_DAGGERS: u16 = 173;
pub(in crate::world) const SKILL_THROWN: u16 = 176;
pub(in crate::world) const SKILL_CROSSBOWS: u16 = 226;
pub(in crate::world) const SKILL_WANDS: u16 = 228;
pub(in crate::world) const SKILL_POLEARMS: u16 = 229;
pub(in crate::world) const SKILL_SPEARS: u16 = 253;
pub(in crate::world) const SKILL_FISHING: u16 = 356;
pub(in crate::world) const SKILL_FIST_WEAPONS: u16 = 473;

#[derive(Debug, Clone, Copy)]
pub(in crate::world) struct MeleeRollChances {
    pub(in crate::world) miss: f32,
    pub(in crate::world) dodge: f32,
    pub(in crate::world) parry: f32,
    pub(in crate::world) block: f32,
    pub(in crate::world) glancing: f32,
    pub(in crate::world) crit: f32,
    pub(in crate::world) crushing: f32,
}

#[derive(Debug, Clone, Copy)]
pub(in crate::world) struct MeleeDamageInput {
    pub(in crate::world) attacker_level: u8,
    pub(in crate::world) attacker_skill: u16,
    pub(in crate::world) victim_defense: u16,
    pub(in crate::world) min_damage: f32,
    pub(in crate::world) max_damage: f32,
    pub(in crate::world) victim_armor: u32,
    pub(in crate::world) victim_block_value: u32,
    pub(in crate::world) chances: MeleeRollChances,
}

#[derive(Debug, Clone, Copy)]
pub(in crate::world) struct PlayerMeleeDefenseInput {
    pub(in crate::world) level: u8,
    pub(in crate::world) defense_skill: u16,
    pub(in crate::world) armor: u32,
    pub(in crate::world) block_value: u32,
    pub(in crate::world) dodge_percent: f32,
    pub(in crate::world) parry_percent: f32,
    pub(in crate::world) block_percent: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(in crate::world) struct MeleeDamageOutcome {
    pub(in crate::world) hit_info: u32,
    pub(in crate::world) victim_state: u32,
    pub(in crate::world) outcome: MeleeHitOutcome,
    pub(in crate::world) total_damage: u32,
    pub(in crate::world) school_damage: u32,
    pub(in crate::world) absorbed: u32,
    pub(in crate::world) resisted: i32,
    pub(in crate::world) blocked: u32,
}

impl MeleeDamageOutcome {
    pub(in crate::world) fn normal_hit(damage: u32) -> Self {
        Self {
            hit_info: HITINFO_NORMALSWING2,
            victim_state: VICTIMSTATE_NORMAL,
            outcome: MeleeHitOutcome::Normal,
            total_damage: damage,
            school_damage: damage,
            absorbed: 0,
            resisted: 0,
            blocked: 0,
        }
    }

    pub(in crate::world) fn with_next_melee_spell_bonus(mut self, bonus_damage: u32) -> Self {
        if self.total_damage > 0 {
            self.total_damage = self.total_damage.saturating_add(bonus_damage);
            self.school_damage = self.school_damage.saturating_add(bonus_damage);
        }
        self
    }

    pub(in crate::world) fn with_weapon_spell_modifier(
        mut self,
        bonus_damage: u32,
        weapon_damage_percent: u32,
    ) -> Self {
        if self.total_damage > 0 {
            self.total_damage = self.total_damage.saturating_mul(weapon_damage_percent) / 100;
            self.school_damage = self.school_damage.saturating_mul(weapon_damage_percent) / 100;
            self.total_damage = self.total_damage.saturating_add(bonus_damage);
            self.school_damage = self.school_damage.saturating_add(bonus_damage);
        }
        self
    }

    pub(in crate::world) fn spell_miss_info(self) -> Option<u8> {
        if self.total_damage > 0 {
            return None;
        }
        match self.outcome {
            MeleeHitOutcome::Miss => Some(SPELL_MISS_MISS),
            MeleeHitOutcome::Dodge => Some(SPELL_MISS_DODGE),
            MeleeHitOutcome::Parry => Some(SPELL_MISS_PARRY),
            MeleeHitOutcome::Block => Some(SPELL_MISS_BLOCK),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::world) struct SpellDamageOutcome {
    pub(in crate::world) original_damage: u32,
    pub(in crate::world) final_damage: u32,
    pub(in crate::world) absorb: u32,
    pub(in crate::world) resist: i32,
    pub(in crate::world) blocked: u32,
    pub(in crate::world) hit_info: u32,
    pub(in crate::world) miss_info: Option<u8>,
}

impl SpellDamageOutcome {
    pub(in crate::world) fn normal_hit(damage: u32) -> Self {
        Self {
            original_damage: damage,
            final_damage: damage,
            absorb: 0,
            resist: 0,
            blocked: 0,
            hit_info: 0,
            miss_info: None,
        }
    }

    pub(in crate::world) fn full_resist(damage: u32) -> Self {
        Self {
            original_damage: damage,
            final_damage: 0,
            absorb: 0,
            resist: damage as i32,
            blocked: 0,
            hit_info: 0,
            miss_info: Some(SPELL_MISS_RESIST),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub(in crate::world) struct SpellDamageOutcomeInput {
    pub(in crate::world) damage: u32,
    pub(in crate::world) school: u8,
    pub(in crate::world) dmg_class: u32,
    pub(in crate::world) attributes_ex2: u32,
    pub(in crate::world) attributes_ex3: u32,
    pub(in crate::world) caster_class: u8,
    pub(in crate::world) caster_level: u8,
    pub(in crate::world) caster_intellect: u32,
    pub(in crate::world) target_level: u8,
    pub(in crate::world) target_resistances: [i16; MAX_SPELL_SCHOOL],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::world) struct SpellCombatUnitSnapshot {
    pub(in crate::world) level: u8,
    pub(in crate::world) class: u8,
    pub(in crate::world) intellect: u32,
    pub(in crate::world) resistances: [i16; MAX_SPELL_SCHOOL],
}

#[derive(Debug, Clone, Copy)]
pub(in crate::world) struct SpellDamageOutcomeRolls {
    pub(in crate::world) hit_roll: u32,
    pub(in crate::world) crit_roll: u32,
    pub(in crate::world) partial_resist_roll: u32,
}

pub(in crate::world) const SPELL_DAMAGE_CLASS_NONE: u32 = 0;
pub(in crate::world) const SPELL_DAMAGE_CLASS_MAGIC: u32 = 1;
pub(in crate::world) const SPELL_DAMAGE_CLASS_MELEE: u32 = 2;
pub(in crate::world) const SPELL_ATTR_EX2_CANT_CRIT: u32 = 0x2000_0000;
pub(in crate::world) const SPELL_ATTR_EX3_ALWAYS_HIT: u32 = 0x0004_0000;
pub(in crate::world) const SPELL_ATTR_EX3_IGNORE_CASTER_MODIFIERS: u32 = 0x2000_0000;
pub(in crate::world) const PARTIAL_RESIST_DISTRIBUTION: [[u32; 5]; 101] = [
    [10000, 0, 0, 0, 0],
    [9700, 200, 100, 0, 0],
    [9400, 400, 200, 0, 0],
    [9000, 800, 200, 0, 0],
    [8700, 1000, 300, 0, 0],
    [8400, 1200, 400, 0, 0],
    [8200, 1300, 400, 100, 0],
    [7900, 1500, 500, 100, 0],
    [7600, 1700, 600, 100, 0],
    [7300, 1900, 700, 100, 0],
    [6900, 2300, 700, 100, 0],
    [6600, 2500, 800, 100, 0],
    [6300, 2700, 900, 100, 0],
    [6000, 2900, 1000, 100, 0],
    [5800, 3000, 1000, 200, 0],
    [5400, 3300, 1100, 200, 0],
    [5100, 3600, 1100, 200, 0],
    [4800, 3800, 1200, 200, 0],
    [4400, 4200, 1200, 200, 0],
    [4100, 4400, 1300, 200, 0],
    [3700, 4800, 1300, 200, 0],
    [3400, 5000, 1400, 200, 0],
    [3100, 5200, 1500, 200, 0],
    [3000, 5200, 1500, 200, 100],
    [2800, 5300, 1500, 300, 100],
    [2500, 5500, 1600, 300, 100],
    [2400, 5400, 1700, 400, 100],
    [2300, 5300, 1800, 500, 100],
    [2200, 5100, 2100, 500, 100],
    [2100, 5000, 2200, 600, 100],
    [2000, 4900, 2400, 600, 100],
    [1900, 4700, 2600, 700, 100],
    [1800, 4600, 2700, 800, 100],
    [1700, 4400, 3000, 800, 100],
    [1600, 4300, 3100, 900, 100],
    [1500, 4200, 3200, 1000, 100],
    [1400, 4100, 3300, 1100, 100],
    [1300, 3900, 3600, 1100, 100],
    [1300, 3600, 3800, 1200, 100],
    [1200, 3500, 3900, 1300, 100],
    [1100, 3400, 4000, 1400, 100],
    [1000, 3300, 4100, 1500, 100],
    [900, 3100, 4400, 1500, 100],
    [800, 3000, 4500, 1600, 100],
    [800, 2700, 4700, 1700, 100],
    [700, 2600, 4800, 1800, 100],
    [600, 2500, 4900, 1900, 100],
    [600, 2300, 5000, 1900, 200],
    [500, 2200, 5100, 2000, 200],
    [300, 2200, 5300, 2000, 200],
    [200, 2100, 5400, 2100, 200],
    [200, 2000, 5300, 2200, 300],
    [200, 2000, 5100, 2200, 500],
    [200, 1900, 5000, 2300, 600],
    [100, 1900, 4900, 2500, 600],
    [100, 1800, 4800, 2600, 700],
    [100, 1700, 4700, 2700, 800],
    [100, 1600, 4500, 3000, 800],
    [100, 1500, 4400, 3100, 900],
    [100, 1500, 4100, 3300, 1000],
    [100, 1400, 4000, 3400, 1100],
    [100, 1300, 3900, 3500, 1200],
    [100, 1200, 3800, 3600, 1300],
    [100, 1100, 3600, 3900, 1300],
    [100, 1100, 3300, 4100, 1400],
    [100, 1000, 3200, 4200, 1500],
    [100, 900, 3100, 4300, 1600],
    [100, 800, 3000, 4400, 1700],
    [100, 800, 2700, 4600, 1800],
    [100, 700, 2600, 4700, 1900],
    [100, 600, 2400, 4900, 2000],
    [100, 600, 2200, 5000, 2100],
    [100, 500, 2100, 5100, 2200],
    [100, 500, 1800, 5300, 2300],
    [100, 400, 1700, 5400, 2400],
    [100, 300, 1600, 5500, 2500],
    [100, 300, 1500, 5300, 2800],
    [100, 200, 1500, 5200, 3000],
    [0, 200, 1500, 5200, 3100],
    [0, 200, 1400, 5000, 3400],
    [0, 200, 1300, 4800, 3700],
    [0, 200, 1300, 4400, 4100],
    [0, 200, 1200, 4200, 4400],
    [0, 200, 1200, 3800, 4800],
    [0, 200, 1100, 3600, 5100],
    [0, 200, 1100, 3300, 5400],
    [0, 200, 1000, 3000, 5800],
    [0, 100, 1000, 2900, 6000],
    [0, 100, 900, 2700, 6300],
    [0, 100, 800, 2500, 6600],
    [0, 100, 700, 2300, 6900],
    [0, 100, 700, 1900, 7300],
    [0, 100, 600, 1700, 7600],
    [0, 100, 500, 1500, 7900],
    [0, 100, 400, 1300, 8200],
    [0, 0, 400, 1200, 8400],
    [0, 0, 300, 1000, 8700],
    [0, 0, 200, 800, 9000],
    [0, 0, 200, 400, 9400],
    [0, 0, 100, 200, 9700],
    [0, 0, 0, 0, 10000],
];

pub(in crate::world) fn roll_spell_damage_outcome(
    input: SpellDamageOutcomeInput,
) -> SpellDamageOutcome {
    let mut rng = rand::thread_rng();
    calculate_spell_damage_outcome(
        input,
        SpellDamageOutcomeRolls {
            hit_roll: rng.gen_range(1..=10_000),
            crit_roll: rng.gen_range(1..=10_000),
            partial_resist_roll: rng.gen_range(1..=10_000),
        },
    )
}

pub(in crate::world) fn spell_damage_outcome_input(
    damage: u32,
    school: u8,
    dmg_class: u32,
    attributes_ex2: u32,
    attributes_ex3: u32,
    caster: SpellCombatUnitSnapshot,
    target: SpellCombatUnitSnapshot,
) -> SpellDamageOutcomeInput {
    SpellDamageOutcomeInput {
        damage,
        school,
        dmg_class,
        attributes_ex2,
        attributes_ex3,
        caster_class: caster.class,
        caster_level: caster.level,
        caster_intellect: caster.intellect,
        target_level: target.level,
        target_resistances: target.resistances,
    }
}

pub(in crate::world) fn calculate_spell_damage_outcome(
    input: SpellDamageOutcomeInput,
    rolls: SpellDamageOutcomeRolls,
) -> SpellDamageOutcome {
    if input.damage == 0 {
        return SpellDamageOutcome::normal_hit(0);
    }
    if spell_full_resist_succeeds(input, rolls.hit_roll) {
        return SpellDamageOutcome::full_resist(input.damage);
    }

    let mut damage = input.damage;
    let mut hit_info = 0;
    if spell_crit_succeeds(input, rolls.crit_roll) {
        hit_info |= SPELL_HIT_TYPE_CRIT;
        damage = spell_crit_amount(input, damage);
    }

    let resist = spell_partial_resist_amount(input, damage, rolls.partial_resist_roll);
    let final_damage = damage.saturating_sub(resist);
    SpellDamageOutcome {
        original_damage: input.damage,
        final_damage,
        absorb: 0,
        resist: resist as i32,
        blocked: 0,
        hit_info,
        miss_info: None,
    }
}

pub(in crate::world) fn spell_full_resist_succeeds(
    input: SpellDamageOutcomeInput,
    roll: u32,
) -> bool {
    if input.attributes_ex3 & SPELL_ATTR_EX3_ALWAYS_HIT != 0 {
        return false;
    }
    if !matches!(
        input.dmg_class,
        SPELL_DAMAGE_CLASS_MAGIC | SPELL_DAMAGE_CLASS_NONE
    ) {
        return false;
    }
    let mut chance = spell_miss_chance_percent(input);
    if input.dmg_class == SPELL_DAMAGE_CLASS_MAGIC && is_resistable_spell_school(input.school) {
        let percent = effective_magic_resistance_percent(input);
        if percent > 0.0 {
            let full_resist_chance = partial_resist_chances(percent)[4] as f32 * 0.01;
            chance += full_resist_chance;
        }
    }
    roll <= chance_to_basis_points(chance)
}

pub(in crate::world) fn spell_miss_chance_percent(input: SpellDamageOutcomeInput) -> f32 {
    let difference = input.target_level as i32 - input.caster_level as i32;
    let chance = if difference > 2 {
        2 + (difference - 2) * 11
    } else {
        difference
    };
    (chance as f32).clamp(1.0, 100.0)
}

pub(in crate::world) fn spell_crit_succeeds(input: SpellDamageOutcomeInput, roll: u32) -> bool {
    if input.attributes_ex2 & SPELL_ATTR_EX2_CANT_CRIT != 0 {
        return false;
    }
    if !matches!(
        input.dmg_class,
        SPELL_DAMAGE_CLASS_MAGIC | SPELL_DAMAGE_CLASS_NONE
    ) {
        return false;
    }
    roll <= chance_to_basis_points(spell_crit_chance_percent(input))
}

pub(in crate::world) fn spell_crit_amount(input: SpellDamageOutcomeInput, damage: u32) -> u32 {
    if input.attributes_ex3 & SPELL_ATTR_EX3_IGNORE_CASTER_MODIFIERS != 0 {
        damage
    } else {
        damage.saturating_add(damage / 2).max(1)
    }
}

pub(in crate::world) fn spell_crit_chance_percent(input: SpellDamageOutcomeInput) -> f32 {
    spell_crit_from_intellect(
        input.caster_class,
        input.caster_level,
        input.caster_intellect,
    )
    .clamp(0.0, 100.0)
}

pub(in crate::world) fn spell_crit_from_intellect(class: u8, level: u8, intellect: u32) -> f32 {
    let (base, rate0, rate1) = match class {
        2 => (3.70, 14.77, 0.65),
        5 => (2.97, 10.03, 0.82),
        7 => (3.54, 11.51, 0.80),
        8 => (3.70, 14.77, 0.65),
        9 => (3.18, 11.30, 0.82),
        11 => (3.33, 12.41, 0.79),
        _ => return 0.0,
    };
    let ratio = rate0 + rate1 * level.max(1) as f32;
    if ratio <= 0.0 {
        0.0
    } else {
        base + intellect as f32 / ratio
    }
}

pub(in crate::world) fn spell_partial_resist_amount(
    input: SpellDamageOutcomeInput,
    damage: u32,
    roll: u32,
) -> u32 {
    if input.dmg_class != SPELL_DAMAGE_CLASS_MAGIC || !is_resistable_spell_school(input.school) {
        return 0;
    }
    let percent = effective_magic_resistance_percent(input);
    if percent <= 0.0 {
        return 0;
    }
    let chances = partial_resist_chances(percent);
    let mut threshold = 0u32;
    for (portion, chance) in chances.into_iter().enumerate() {
        threshold = threshold.saturating_add(chance);
        if roll <= threshold {
            let portion = portion.min(4) as u32;
            return damage.saturating_mul(portion) / 5;
        }
    }
    0
}

pub(in crate::world) fn effective_magic_resistance_percent(input: SpellDamageOutcomeInput) -> f32 {
    if input.school as usize >= MAX_SPELL_SCHOOL || input.school == 0 || input.school == 1 {
        return 0.0;
    }
    let resistance = input.target_resistances[input.school as usize].max(0) as f32;
    if resistance <= 0.0 {
        return 0.0;
    }
    let caster_skill = (input.caster_level.max(1) as f32 * 5.0).max(100.0);
    let target_skill = (input.target_level.max(1) as f32 * 5.0).max(100.0);
    let mut percent = (resistance / caster_skill) * 100.0 * 0.75;
    if input.dmg_class == SPELL_DAMAGE_CLASS_MAGIC {
        percent += 0.4 * (target_skill - caster_skill).max(0.0);
    }
    percent.clamp(0.0, 75.0)
}

pub(in crate::world) fn is_resistable_spell_school(school: u8) -> bool {
    school != 0 && school != 1
}

pub(in crate::world) fn creature_spell_resistances(
    template: &CreatureTemplateQuery,
) -> [i16; MAX_SPELL_SCHOOL] {
    [
        template.armor.min(i16::MAX as u32) as i16,
        template.resistance_holy,
        template.resistance_fire,
        template.resistance_nature,
        template.resistance_frost,
        template.resistance_shadow,
        template.resistance_arcane,
    ]
}

pub(in crate::world) fn db_creature_spell_snapshot(
    creature: &DbCreatureRuntime,
) -> SpellCombatUnitSnapshot {
    SpellCombatUnitSnapshot {
        level: creature
            .spawn
            .template
            .max_level
            .max(creature.spawn.template.min_level)
            .max(1),
        class: 0,
        intellect: 0,
        resistances: creature_spell_resistances(&creature.spawn.template),
    }
}

pub(in crate::world) fn player_spell_snapshot(
    level: u8,
    class: u8,
    combat_stats: &PlayerCombatStats,
) -> SpellCombatUnitSnapshot {
    let mut resistances = [0i16; MAX_SPELL_SCHOOL];
    for (index, value) in combat_stats.resistances.iter().copied().enumerate() {
        resistances[index] = value.min(i16::MAX as u32) as i16;
    }
    SpellCombatUnitSnapshot {
        level: level.max(1),
        class,
        intellect: combat_stats.intellect,
        resistances,
    }
}

pub(in crate::world) fn partial_resist_chances(percent: f32) -> [u32; 5] {
    let basis = (percent.clamp(0.0, 100.0) * 100.0).round() as usize;
    let row = (basis / 100).min(PARTIAL_RESIST_DISTRIBUTION.len() - 1);
    let intermediate = basis % 100;
    if row + 1 >= PARTIAL_RESIST_DISTRIBUTION.len() || intermediate == 0 {
        return PARTIAL_RESIST_DISTRIBUTION[row];
    }
    let base = PARTIAL_RESIST_DISTRIBUTION[row];
    let next = PARTIAL_RESIST_DISTRIBUTION[row + 1];
    let mut values = [0; 5];
    for index in 0..5 {
        let diff = next[index] as i64 - base[index] as i64;
        values[index] = (base[index] as i64
            + ((diff as f64 * intermediate as f64 / 100.0).round() as i64))
            .max(0) as u32;
    }
    values
}

pub(in crate::world) fn roll_melee_damage(input: MeleeDamageInput) -> MeleeDamageOutcome {
    let mut rng = rand::thread_rng();
    let damage_roll = rng.gen_range(1..=10_000);
    let outcome_roll = rng.gen_range(1..=10_000);
    calculate_melee_damage(input, damage_roll, outcome_roll)
}

pub(in crate::world) fn calculate_melee_damage(
    input: MeleeDamageInput,
    damage_roll: u32,
    outcome_roll: u32,
) -> MeleeDamageOutcome {
    let base_damage = roll_damage_between(input.min_damage, input.max_damage, damage_roll);
    let damage = armor_reduced_damage(input.attacker_level, input.victim_armor, base_damage);
    let outcome = roll_melee_outcome(input.chances, outcome_roll);

    match outcome {
        MeleeHitOutcome::Miss => MeleeDamageOutcome {
            hit_info: HITINFO_NORMALSWING2 | HITINFO_MISS,
            victim_state: VICTIMSTATE_UNAFFECTED,
            outcome,
            total_damage: 0,
            school_damage: 0,
            absorbed: 0,
            resisted: 0,
            blocked: 0,
        },
        MeleeHitOutcome::Dodge => MeleeDamageOutcome {
            hit_info: HITINFO_NORMALSWING2,
            victim_state: VICTIMSTATE_DODGE,
            outcome,
            total_damage: 0,
            school_damage: 0,
            absorbed: 0,
            resisted: 0,
            blocked: 0,
        },
        MeleeHitOutcome::Parry => MeleeDamageOutcome {
            hit_info: HITINFO_NORMALSWING2,
            victim_state: VICTIMSTATE_PARRY,
            outcome,
            total_damage: 0,
            school_damage: 0,
            absorbed: 0,
            resisted: 0,
            blocked: 0,
        },
        MeleeHitOutcome::Block => {
            let blocked = input.victim_block_value.min(damage);
            let total_damage = damage.saturating_sub(blocked);
            MeleeDamageOutcome {
                hit_info: HITINFO_NORMALSWING2 | HITINFO_BLOCK,
                victim_state: if total_damage == 0 {
                    VICTIMSTATE_BLOCKS
                } else {
                    VICTIMSTATE_NORMAL
                },
                outcome,
                total_damage,
                school_damage: total_damage,
                absorbed: 0,
                resisted: 0,
                blocked,
            }
        }
        MeleeHitOutcome::Glancing => {
            let total_damage = ((damage as f32) * glancing_multiplier(input)).round() as u32;
            MeleeDamageOutcome {
                hit_info: HITINFO_NORMALSWING2 | HITINFO_GLANCING,
                victim_state: VICTIMSTATE_NORMAL,
                outcome,
                total_damage: total_damage.max(1),
                school_damage: total_damage.max(1),
                absorbed: 0,
                resisted: 0,
                blocked: 0,
            }
        }
        MeleeHitOutcome::Crit => {
            let total_damage = damage.saturating_mul(2).max(1);
            MeleeDamageOutcome {
                hit_info: HITINFO_NORMALSWING2 | HITINFO_CRITICALHIT,
                victim_state: VICTIMSTATE_NORMAL,
                outcome,
                total_damage,
                school_damage: total_damage,
                absorbed: 0,
                resisted: 0,
                blocked: 0,
            }
        }
        MeleeHitOutcome::Crushing => {
            let total_damage = damage + damage / 2;
            MeleeDamageOutcome {
                hit_info: HITINFO_NORMALSWING2 | HITINFO_CRUSHING,
                victim_state: VICTIMSTATE_NORMAL,
                outcome,
                total_damage: total_damage.max(1),
                school_damage: total_damage.max(1),
                absorbed: 0,
                resisted: 0,
                blocked: 0,
            }
        }
        MeleeHitOutcome::Normal => MeleeDamageOutcome::normal_hit(damage),
    }
}

pub(in crate::world) fn roll_melee_outcome(
    chances: MeleeRollChances,
    roll: u32,
) -> MeleeHitOutcome {
    let mut threshold = chance_to_basis_points(chances.miss);
    if roll <= threshold {
        return MeleeHitOutcome::Miss;
    }
    threshold += chance_to_basis_points(chances.dodge);
    if roll <= threshold {
        return MeleeHitOutcome::Dodge;
    }
    threshold += chance_to_basis_points(chances.parry);
    if roll <= threshold {
        return MeleeHitOutcome::Parry;
    }
    threshold += chance_to_basis_points(chances.block);
    if roll <= threshold {
        return MeleeHitOutcome::Block;
    }
    threshold += chance_to_basis_points(chances.glancing);
    if roll <= threshold {
        return MeleeHitOutcome::Glancing;
    }
    threshold += chance_to_basis_points(chances.crit);
    if roll <= threshold {
        return MeleeHitOutcome::Crit;
    }
    threshold += chance_to_basis_points(chances.crushing);
    if roll <= threshold {
        return MeleeHitOutcome::Crushing;
    }
    MeleeHitOutcome::Normal
}

pub(in crate::world) fn chance_to_basis_points(chance_percent: f32) -> u32 {
    (chance_percent.clamp(0.0, 100.0) * 100.0).round() as u32
}

pub(in crate::world) fn roll_damage_between(min_damage: f32, max_damage: f32, roll: u32) -> f32 {
    let min_damage = min_damage.max(1.0);
    let max_damage = max_damage.max(min_damage);
    let t = (roll.clamp(1, 10_000) - 1) as f32 / 9_999.0;
    min_damage + (max_damage - min_damage) * t
}

pub(in crate::world) fn armor_reduced_damage(
    attacker_level: u8,
    victim_armor: u32,
    damage: f32,
) -> u32 {
    let armor = victim_armor as f32;
    let level_modifier = attacker_level.max(1) as f32;
    let reduction = if armor <= 0.0 {
        0.0
    } else {
        let value = 0.1 * armor / (8.5 * level_modifier + 40.0);
        (value / (1.0 + value)).clamp(0.0, 0.75)
    };
    (damage - damage * reduction).round().max(1.0) as u32
}

pub(in crate::world) fn glancing_multiplier(input: MeleeDamageInput) -> f32 {
    let difference = input.victim_defense as i32 - input.attacker_skill as i32;
    if difference < 0 {
        return 1.0;
    }
    let high_end = (1.2 - 0.03 * difference as f32).clamp(0.2, 0.99);
    let low_end = (1.3 - 0.05 * difference as f32).clamp(0.01, high_end.min(0.91));
    (low_end + high_end) / 2.0
}

pub(in crate::world) fn creature_melee_input_against_player(
    creature: &DbCreatureRuntime,
    defense: PlayerMeleeDefenseInput,
) -> MeleeDamageInput {
    let level = creature
        .spawn
        .template
        .max_level
        .max(creature.spawn.template.min_level);
    let attacker_skill = u16::from(level.max(1)).saturating_mul(5);
    MeleeDamageInput {
        attacker_level: level.max(1),
        attacker_skill,
        victim_defense: defense.defense_skill,
        min_damage: creature.spawn.template.min_melee_dmg,
        max_damage: creature.spawn.template.max_melee_dmg,
        victim_armor: defense.armor,
        victim_block_value: defense.block_value,
        chances: starter_player_defense_chances(level, defense),
    }
}

#[cfg(test)]
pub(in crate::world) fn calculate_player_main_hand_melee_damage(
    combat_stats: &PlayerCombatStats,
    attacker_level: u8,
    victim_armor: u32,
    damage_roll: u32,
) -> u32 {
    let damage = roll_damage_between(
        combat_stats.main_min_damage.max(1.0),
        combat_stats.main_max_damage.max(1.0),
        damage_roll,
    );
    armor_reduced_damage(attacker_level, victim_armor, damage)
}

pub(in crate::world) fn player_main_hand_melee_outcome_against_db_creature(
    combat_stats: &PlayerCombatStats,
    attacker_level: u8,
    attacker_skill: u16,
    creature: &DbCreatureRuntime,
) -> MeleeDamageOutcome {
    let mut rng = rand::thread_rng();
    calculate_player_main_hand_melee_outcome_against_db_creature(
        combat_stats,
        attacker_level,
        attacker_skill,
        creature,
        rng.gen_range(1..=10_000),
        rng.gen_range(1..=10_000),
    )
}

pub(in crate::world) fn calculate_player_main_hand_melee_outcome_against_db_creature(
    combat_stats: &PlayerCombatStats,
    attacker_level: u8,
    attacker_skill: u16,
    creature: &DbCreatureRuntime,
    damage_roll: u32,
    outcome_roll: u32,
) -> MeleeDamageOutcome {
    let level = creature
        .spawn
        .template
        .max_level
        .max(creature.spawn.template.min_level)
        .max(1);
    let creature_defense = u16::from(level).saturating_mul(5);
    calculate_melee_damage(
        MeleeDamageInput {
            attacker_level: attacker_level.max(1),
            attacker_skill,
            victim_defense: creature_defense,
            min_damage: combat_stats.main_min_damage,
            max_damage: combat_stats.main_max_damage,
            victim_armor: creature.spawn.template.armor,
            victim_block_value: 0,
            chances: player_main_hand_chances_against_db_creature(
                combat_stats,
                attacker_skill,
                creature_defense,
                level,
            ),
        },
        damage_roll,
        outcome_roll,
    )
}

pub(in crate::world) fn player_ranged_outcome_against_db_creature(
    combat_stats: &PlayerCombatStats,
    attacker_level: u8,
    attacker_skill: u16,
    creature: &DbCreatureRuntime,
) -> MeleeDamageOutcome {
    let level = creature
        .spawn
        .template
        .max_level
        .max(creature.spawn.template.min_level)
        .max(1);
    let creature_defense = u16::from(level).saturating_mul(5);
    let skill_delta = i32::from(attacker_skill) - i32::from(creature_defense);
    calculate_melee_damage(
        MeleeDamageInput {
            attacker_level: attacker_level.max(1),
            attacker_skill,
            victim_defense: creature_defense,
            min_damage: combat_stats.ranged_min_damage,
            max_damage: combat_stats.ranged_max_damage,
            victim_armor: creature.spawn.template.armor,
            victim_block_value: 0,
            chances: MeleeRollChances {
                miss: cmangos_melee_miss_chance(
                    i32::from(attacker_skill),
                    i32::from(creature_defense),
                    false,
                ),
                dodge: 0.0,
                parry: 0.0,
                block: 0.0,
                glancing: 0.0,
                crit: (combat_stats.ranged_crit_percent + skill_delta as f32 * 0.2)
                    .clamp(0.0, 100.0),
                crushing: 0.0,
            },
        },
        rand::thread_rng().gen_range(1..=10_000),
        rand::thread_rng().gen_range(1..=10_000),
    )
}

pub(in crate::world) fn player_main_hand_chances_against_db_creature(
    combat_stats: &PlayerCombatStats,
    attacker_skill: u16,
    creature_defense: u16,
    creature_level: u8,
) -> MeleeRollChances {
    let attacker_skill = i32::from(attacker_skill);
    let creature_defense = i32::from(creature_defense);
    let miss = cmangos_melee_miss_chance(attacker_skill, creature_defense, false);
    let crit_difference = attacker_skill - creature_defense;
    let crit = combat_stats.crit_percent + crit_difference as f32 * 0.2;
    MeleeRollChances {
        miss: miss.clamp(0.0, 100.0),
        dodge: 0.0,
        parry: 0.0,
        block: 0.0,
        glancing: if creature_level > 10 {
            (10.0 + ((creature_defense - attacker_skill) as f32 * 2.0)).clamp(0.0, 100.0)
        } else {
            0.0
        },
        crit: crit.clamp(0.0, 100.0),
        crushing: 0.0,
    }
}

pub(in crate::world) fn starter_player_defense_chances(
    creature_level: u8,
    defense: PlayerMeleeDefenseInput,
) -> MeleeRollChances {
    let creature_skill = i32::from(u16::from(creature_level.max(1)).saturating_mul(5));
    let defense_skill = i32::from(defense.defense_skill);
    let skill_difference = defense_skill - creature_skill;
    let skill_adjust = skill_difference as f32 * 0.04;
    MeleeRollChances {
        miss: cmangos_melee_miss_chance(creature_skill, defense_skill, true),
        dodge: (defense.dodge_percent + skill_adjust).clamp(0.0, 100.0),
        parry: (defense.parry_percent + skill_adjust).clamp(0.0, 100.0),
        block: (defense.block_percent + skill_adjust).clamp(0.0, 100.0),
        glancing: 0.0,
        crit: 5.0,
        crushing: if creature_level >= defense.level.saturating_add(3) {
            15.0
        } else {
            0.0
        },
    }
}

pub(in crate::world) fn player_melee_defense_input(
    character: &ActiveCharacter,
    combat_stats: &PlayerCombatStats,
    character_skills: &[CharacterSkill],
    active_auras: &[ActiveAura],
) -> PlayerMeleeDefenseInput {
    PlayerMeleeDefenseInput {
        level: character.level.max(1),
        defense_skill: current_skill_value_with_active_auras(
            character_skills,
            active_auras,
            SKILL_DEFENSE,
        ),
        armor: combat_stats.armor,
        block_value: combat_stats.shield_block_value,
        dodge_percent: combat_stats.dodge_percent,
        parry_percent: combat_stats.parry_percent,
        block_percent: combat_stats.block_percent,
    }
}

pub(in crate::world) fn cmangos_melee_miss_chance(
    attacker_skill: i32,
    victim_defense: i32,
    victim_is_player: bool,
) -> f32 {
    let mut chance = 5.0;
    let mut difference = victim_defense - attacker_skill;
    let mut factor = 0.04;
    if !victim_is_player && difference > 0 {
        if difference > 10 {
            chance += 10.0 * 0.1;
            difference -= 10;
            factor = 0.4;
            chance += difference as f32 * 0.2;
        } else {
            factor = 0.1;
        }
    }
    chance += difference as f32 * factor;
    chance.clamp(0.0, 100.0)
}

pub(in crate::world) fn current_skill_value(
    character_skills: &[CharacterSkill],
    skill_id: u16,
) -> u16 {
    character_skills
        .iter()
        .find(|skill| skill.skill == skill_id)
        .map(|skill| skill.value)
        .unwrap_or(0)
}
