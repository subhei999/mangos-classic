#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MeleeHitOutcome {
    Miss,
    Dodge,
    Block,
    Parry,
    Glancing,
    Crit,
    Crushing,
    Normal,
}

const SKILL_DEFENSE: u16 = 95;
const SKILL_SWORDS: u16 = 43;
const SKILL_AXES: u16 = 44;
const SKILL_BOWS: u16 = 45;
const SKILL_GUNS: u16 = 46;
const SKILL_MACES: u16 = 54;
const SKILL_TWO_HANDED_SWORDS: u16 = 55;
const SKILL_STAVES: u16 = 136;
const SKILL_TWO_HANDED_MACES: u16 = 160;
const SKILL_UNARMED: u16 = 162;
const SKILL_TWO_HANDED_AXES: u16 = 172;
const SKILL_DAGGERS: u16 = 173;
const SKILL_THROWN: u16 = 176;
const SKILL_CROSSBOWS: u16 = 226;
const SKILL_WANDS: u16 = 228;
const SKILL_POLEARMS: u16 = 229;
const SKILL_SPEARS: u16 = 253;
const SKILL_FISHING: u16 = 356;
const SKILL_FIST_WEAPONS: u16 = 473;

#[derive(Debug, Clone, Copy)]
struct MeleeRollChances {
    miss: f32,
    dodge: f32,
    parry: f32,
    block: f32,
    glancing: f32,
    crit: f32,
    crushing: f32,
}

#[derive(Debug, Clone, Copy)]
struct MeleeDamageInput {
    attacker_level: u8,
    attacker_skill: u16,
    victim_defense: u16,
    min_damage: f32,
    max_damage: f32,
    victim_armor: u32,
    victim_block_value: u32,
    chances: MeleeRollChances,
}

#[derive(Debug, Clone, Copy)]
struct PlayerMeleeDefenseInput {
    level: u8,
    defense_skill: u16,
    armor: u32,
    block_value: u32,
    dodge_percent: f32,
    parry_percent: f32,
    block_percent: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct MeleeDamageOutcome {
    hit_info: u32,
    victim_state: u32,
    outcome: MeleeHitOutcome,
    total_damage: u32,
    school_damage: u32,
    absorbed: u32,
    resisted: i32,
    blocked: u32,
}

impl MeleeDamageOutcome {
    fn normal_hit(damage: u32) -> Self {
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
}

fn roll_melee_damage(input: MeleeDamageInput) -> MeleeDamageOutcome {
    let mut rng = rand::thread_rng();
    let damage_roll = rng.gen_range(1..=10_000);
    let outcome_roll = rng.gen_range(1..=10_000);
    calculate_melee_damage(input, damage_roll, outcome_roll)
}

fn calculate_melee_damage(
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

fn roll_melee_outcome(chances: MeleeRollChances, roll: u32) -> MeleeHitOutcome {
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

fn chance_to_basis_points(chance_percent: f32) -> u32 {
    (chance_percent.clamp(0.0, 100.0) * 100.0).round() as u32
}

fn roll_damage_between(min_damage: f32, max_damage: f32, roll: u32) -> f32 {
    let min_damage = min_damage.max(1.0);
    let max_damage = max_damage.max(min_damage);
    let t = (roll.clamp(1, 10_000) - 1) as f32 / 9_999.0;
    min_damage + (max_damage - min_damage) * t
}

fn armor_reduced_damage(attacker_level: u8, victim_armor: u32, damage: f32) -> u32 {
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

fn glancing_multiplier(input: MeleeDamageInput) -> f32 {
    let difference = input.victim_defense as i32 - input.attacker_skill as i32;
    if difference < 0 {
        return 1.0;
    }
    let high_end = (1.2 - 0.03 * difference as f32).clamp(0.2, 0.99);
    let low_end = (1.3 - 0.05 * difference as f32).clamp(0.01, high_end.min(0.91));
    (low_end + high_end) / 2.0
}

fn creature_melee_input_against_player(
    creature: &DbCreatureRuntime,
    defense: PlayerMeleeDefenseInput,
) -> MeleeDamageInput {
    let level = creature.spawn.template.max_level.max(creature.spawn.template.min_level);
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
fn calculate_player_main_hand_melee_damage(
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

fn player_main_hand_melee_outcome_against_db_creature(
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

fn calculate_player_main_hand_melee_outcome_against_db_creature(
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

fn player_main_hand_chances_against_db_creature(
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

fn starter_player_defense_chances(
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

fn player_melee_defense_input(
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

fn cmangos_melee_miss_chance(attacker_skill: i32, victim_defense: i32, victim_is_player: bool) -> f32 {
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

fn current_skill_value(character_skills: &[CharacterSkill], skill_id: u16) -> u16 {
    character_skills
        .iter()
        .find(|skill| skill.skill == skill_id)
        .map(|skill| skill.value)
        .unwrap_or(0)
}
