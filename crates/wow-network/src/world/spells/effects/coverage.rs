use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::world) enum SpellEffectDispatch {
    Empty,

    SchoolDamage,

    WeaponDamage,

    WeaponPercentDamage,

    ApplyAura,

    CreateItem,

    Heal,

    Energize,

    PowerBurn,

    Teleport,

    Leap,

    Charge,

    Taunt,

    OpenLock,

    Dispel,

    DispelMechanic,

    InterruptCast,

    PersistentAreaAura,

    LearnSpell,

    LearnSkill,

    TriggerSpell,

    TriggerMissile,

    TransportDoor,

    AddComboPoints,

    Unsupported(u32),
}

pub(in crate::world) const CMANGOS_MAX_SPELL_EFFECTS: u32 = 130;

#[allow(dead_code)]
pub(in crate::world) const CMANGOS_TOTAL_AURAS: u32 = 192;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::world) enum SpellMechanicSupport {
    Implemented,

    KnownNoOp,

    Pending(&'static str),

    Unknown,
}

impl SpellMechanicSupport {
    #[allow(dead_code)]
    pub(in crate::world) fn blocks_runtime(self) -> bool {
        matches!(self, Self::Pending(_) | Self::Unknown)
    }
}

#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::world) enum SpellCoverageMechanic {
    Effect,

    Aura,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::world) struct SpellCoverageIssue {
    pub(in crate::world) spell_id: u32,

    pub(in crate::world) effect_index: usize,

    pub(in crate::world) mechanic: SpellCoverageMechanic,

    pub(in crate::world) mechanic_id: u32,

    pub(in crate::world) mechanic_name: &'static str,

    pub(in crate::world) support: SpellMechanicSupport,
}

impl SpellEffectDispatch {
    pub(in crate::world) fn from_effect_id(effect_id: u32) -> Self {
        match effect_id {
            0 => Self::Empty,

            SPELL_EFFECT_SCHOOL_DAMAGE => Self::SchoolDamage,

            SPELL_EFFECT_WEAPON_DAMAGE_NOSCHOOL
            | SPELL_EFFECT_WEAPON_DAMAGE
            | SPELL_EFFECT_NORMALIZED_WEAPON_DMG => Self::WeaponDamage,

            SPELL_EFFECT_WEAPON_PERCENT_DAMAGE => Self::WeaponPercentDamage,

            SPELL_EFFECT_APPLY_AURA => Self::ApplyAura,

            SPELL_EFFECT_CREATE_ITEM => Self::CreateItem,

            SPELL_EFFECT_HEAL => Self::Heal,

            SPELL_EFFECT_ENERGIZE => Self::Energize,

            SPELL_EFFECT_POWER_BURN => Self::PowerBurn,

            SPELL_EFFECT_TELEPORT_UNITS | SPELL_EFFECT_TELEPORT_UNITS_FACE_CASTER => Self::Teleport,

            SPELL_EFFECT_LEAP => Self::Leap,

            SPELL_EFFECT_CHARGE => Self::Charge,

            SPELL_EFFECT_ATTACK_ME => Self::Taunt,

            SPELL_EFFECT_DISPEL => Self::Dispel,

            SPELL_EFFECT_DISPEL_MECHANIC => Self::DispelMechanic,

            SPELL_EFFECT_INTERRUPT_CAST => Self::InterruptCast,

            SPELL_EFFECT_PERSISTENT_AREA_AURA => Self::PersistentAreaAura,

            33 | 59 => Self::OpenLock,

            36 => Self::LearnSpell,

            44 => Self::LearnSkill,

            SPELL_EFFECT_TRIGGER_MISSILE => Self::TriggerMissile,

            50 => Self::TransportDoor,

            64 => Self::TriggerSpell,

            SPELL_EFFECT_ADD_COMBO_POINTS => Self::AddComboPoints,

            other => Self::Unsupported(other),
        }
    }
}

pub(in crate::world) fn spell_effect_support(effect_id: u32) -> SpellMechanicSupport {
    if effect_id >= CMANGOS_MAX_SPELL_EFFECTS {
        return SpellMechanicSupport::Unknown;
    }

    match effect_id {
        0 => SpellMechanicSupport::KnownNoOp,

        SPELL_EFFECT_SCHOOL_DAMAGE
        | SPELL_EFFECT_TELEPORT_UNITS
        | SPELL_EFFECT_APPLY_AURA
        | SPELL_EFFECT_HEAL
        | SPELL_EFFECT_WEAPON_DAMAGE_NOSCHOOL
        | SPELL_EFFECT_CREATE_ITEM
        | SPELL_EFFECT_ENERGIZE
        | SPELL_EFFECT_LEAP
        | SPELL_EFFECT_WEAPON_PERCENT_DAMAGE
        | SPELL_EFFECT_TRIGGER_MISSILE
        | 33
        | SPELL_EFFECT_TELEPORT_UNITS_FACE_CASTER
        | SPELL_EFFECT_WEAPON_DAMAGE
        | 36
        | 59
        | 60
        | 64
        | SPELL_EFFECT_ADD_COMBO_POINTS
        | SPELL_EFFECT_CHARGE
        | SPELL_EFFECT_ATTACK_ME
        | SPELL_EFFECT_DISPEL
        | SPELL_EFFECT_INTERRUPT_CAST
        | SPELL_EFFECT_PERSISTENT_AREA_AURA
        | SPELL_EFFECT_NORMALIZED_WEAPON_DMG => SpellMechanicSupport::Implemented,

        4 | 12 | 13 | 14 | 15 | 20 | 21 | 23 | 25 | 26 | 37 | 39 | 48 | 49 | 51 | 52 | 65 | 66
        | 78 | 81 | 91 | 122 | 126 | 127 => SpellMechanicSupport::KnownNoOp,

        1 => SpellMechanicSupport::Pending("instant kill"),

        3 | 77 => SpellMechanicSupport::Pending("dummy/script effect"),

        7 => SpellMechanicSupport::Pending("environmental damage"),

        8 => SpellMechanicSupport::Pending("power drain"),

        SPELL_EFFECT_POWER_BURN => SpellMechanicSupport::Implemented,

        9 => SpellMechanicSupport::Pending("health leech"),

        11 => SpellMechanicSupport::Pending("bind"),

        16 => SpellMechanicSupport::Pending("quest complete"),

        18 | 94 | 113 | 117 => SpellMechanicSupport::Pending("resurrection"),

        19 => SpellMechanicSupport::Pending("extra attacks"),

        22 => SpellMechanicSupport::Pending("parry state"),

        35 | 119 | 128 | 129 => SpellMechanicSupport::Pending("area aura"),

        28 | 34 | 41 | 42 | 73 | 76 | 85 | 93 | 97 | 104 | 105 | 106 | 107 | 112 => {
            SpellMechanicSupport::Pending("summon")
        }

        70 | 124 => SpellMechanicSupport::Pending("movement displacement"),

        SPELL_EFFECT_DISPEL_MECHANIC => SpellMechanicSupport::Implemented,

        40 => SpellMechanicSupport::Pending("dual wield"),

        44 | 118 => SpellMechanicSupport::Pending("skill modification"),

        45 => SpellMechanicSupport::Pending("honor"),

        46 => SpellMechanicSupport::Pending("spawn visual"),

        47 => SpellMechanicSupport::Pending("trade skill"),

        50 => SpellMechanicSupport::Implemented,

        53 | 54 | 92 => SpellMechanicSupport::Pending("item enchant"),

        55 => SpellMechanicSupport::Pending("tame creature"),

        56 | 57 | 101 | 102 | 109 => SpellMechanicSupport::Pending("pet"),

        61 => SpellMechanicSupport::Pending("game event"),

        63 | 125 => SpellMechanicSupport::Pending("threat"),

        67 | 75 => SpellMechanicSupport::Pending("special heal"),

        69 => SpellMechanicSupport::Pending("distract"),

        71 => SpellMechanicSupport::Pending("pick pocket"),

        72 | 82 => SpellMechanicSupport::Pending("bind/farsight"),

        74 | 87 | 88 | 89 | 90 | 110 => SpellMechanicSupport::Pending("totem"),

        79 => SpellMechanicSupport::Pending("sanctuary"),

        SPELL_EFFECT_DUEL => SpellMechanicSupport::Pending("duel"),

        SPELL_EFFECT_STUCK => SpellMechanicSupport::Pending("stuck"),

        86 => SpellMechanicSupport::Pending("activate object"),

        95 | 99 | SPELL_EFFECT_SKIN_PLAYER_CORPSE => {
            SpellMechanicSupport::Pending("corpse/skinning/disenchant")
        }

        98 => SpellMechanicSupport::Pending("knockback"),

        100 => SpellMechanicSupport::Pending("inebriate"),

        103 => SpellMechanicSupport::Pending("reputation effect"),

        111 | 115 => SpellMechanicSupport::Pending("durability"),

        120 => SpellMechanicSupport::Pending("graveyard teleport"),

        123 => SpellMechanicSupport::Pending("taxi"),

        _ => SpellMechanicSupport::Unknown,
    }
}

#[allow(dead_code)]
pub(in crate::world) fn spell_aura_support(aura_type: u32) -> SpellMechanicSupport {
    if aura_type >= CMANGOS_TOTAL_AURAS {
        return SpellMechanicSupport::Unknown;
    }

    match aura_type {
        0 => SpellMechanicSupport::KnownNoOp,

        SPELL_AURA_PERIODIC_DAMAGE
        | SPELL_AURA_PERIODIC_HEAL
        | SPELL_AURA_MOD_STUN
        | SPELL_AURA_MOD_BLOCK_PERCENT
        | SPELL_AURA_MOD_CRIT_PERCENT
        | SPELL_AURA_MOD_THREAT
        | SPELL_AURA_MOD_TOTAL_THREAT
        | SPELL_AURA_MOD_TAUNT
        | SPELL_AURA_MOD_DAMAGE_DONE
        | SPELL_AURA_MOD_DAMAGE_TAKEN
        | SPELL_AURA_MOD_DAMAGE_PERCENT_DONE
        | SPELL_AURA_DUMMY
        | SPELL_AURA_MOD_STEALTH_DETECT
        | SPELL_AURA_MOD_INVISIBILITY_DETECTION
        | SPELL_AURA_OBS_MOD_HEALTH
        | SPELL_AURA_PERIODIC_TRIGGER_SPELL
        | SPELL_AURA_MOD_RESISTANCE
        | SPELL_AURA_PERIODIC_ENERGIZE
        | SPELL_AURA_MOD_ROOT
        | SPELL_AURA_MOD_STAT
        | SPELL_AURA_MOD_SKILL
        | SPELL_AURA_MOD_INCREASE_SPEED
        | SPELL_AURA_MOD_DECREASE_SPEED
        | SPELL_AURA_MOD_SHAPESHIFT
        | SPELL_AURA_PROC_TRIGGER_SPELL
        | SPELL_AURA_MOD_RESISTANCE_PCT
        | SPELL_AURA_MOD_REGEN
        | SPELL_AURA_MOD_POWER_REGEN
        | SPELL_AURA_MOD_POWER_REGEN_PERCENT
        | SPELL_AURA_MOD_HEALING_DONE
        | SPELL_AURA_MOD_HEALING
        | SPELL_AURA_MOD_MANA_REGEN_INTERRUPT
        | SPELL_AURA_MOD_SKILL_TALENT
        | SPELL_AURA_MOD_ATTACK_POWER
        | SPELL_AURA_MOD_TOTAL_STAT_PERCENTAGE
        | SPELL_AURA_MOD_MELEE_HASTE
        | SPELL_AURA_MOD_DAMAGE_PERCENT_TAKEN
        | SPELL_AURA_MOD_REPUTATION_GAIN
        | SPELL_AURA_TRACK_CREATURES
        | SPELL_AURA_TRACK_RESOURCES
        | SPELL_AURA_GHOST
        | SPELL_AURA_WATER_WALK
        | SPELL_AURA_HOVER
        | SPELL_AURA_MOD_CONFUSE
        | SPELL_AURA_MOD_FEAR
        | SPELL_AURA_MOD_DISARM
        | SPELL_AURA_MECHANIC_IMMUNITY
        | SPELL_AURA_MOD_PACIFY
        | SPELL_AURA_MOD_SILENCE
        | SPELL_AURA_MOD_PACIFY_SILENCE
        | SPELL_AURA_SCHOOL_ABSORB
        | SPELL_AURA_MANA_SHIELD
        | SPELL_AURA_AURAS_VISIBLE
        | SPELL_AURA_FEATHER_FALL => SpellMechanicSupport::Implemented,

        46 | 48 | 164 => SpellMechanicSupport::KnownNoOp,

        1 | 76 | 82 | 144 => SpellMechanicSupport::Pending("movement/visibility state"),

        2 | 6 | 16 | 18 | 66 | 78 | 128 | 176 | 177 => {
            SpellMechanicSupport::Pending("control state")
        }

        43 | 107 | 108 | 109 | 111 | 112 => SpellMechanicSupport::Pending("trigger/script aura"),

        9 | 15 | 28 | 32 | 34 | 35 | 47 | 49 | 54 | 55 | 57 | 58 | 59 | 65 | 70 | 71 | 72 | 73
        | 80 | 83 | 88 | 89 | 90 | 91 | 102 | 113 | 114 | 116 | 117 | 118 | 122 | 123 | 124
        | 125 | 126 | 127 | 129 | 130 | 131 | 132 | 133 | 136 | 140 | 141 | 142 | 143 | 147
        | 149 | 150 | 152 | 153 | 154 | 155 | 157 | 158 | 160 | 161 | 163 | 165 | 166 | 167
        | 168 | 169 | 171 | 172 | 174 | 175 | 178 | 179 | 180 | 181 | 182 | 183 | 184 | 185
        | 186 | 187 | 188 | 189 | 190 | 191 => {
            SpellMechanicSupport::Pending("stat/combat modifier")
        }

        68 | 75 | 119 | 120 | 121 | 139 | 145 | 146 | 151 | 159 | 170 | 173 => {
            SpellMechanicSupport::Pending("tracking/reaction/client state")
        }

        21 | 53 | 62 | 63 | 64 | 81 | 86 | 96 | 162 => {
            SpellMechanicSupport::Pending("resource shield/funnel")
        }

        37 | 38 | 39 | 40 | 41 | 92 | 93 | 94 | 148 => {
            SpellMechanicSupport::Pending("immunity/special state")
        }

        56 => SpellMechanicSupport::Implemented,

        50 | 61 => SpellMechanicSupport::Pending("visual/model/client state"),

        SPELL_AURA_REFLECT_SPELLS_SCHOOL => SpellMechanicSupport::Implemented,

        _ => SpellMechanicSupport::Unknown,
    }
}

#[allow(dead_code)]
pub(in crate::world) fn spell_template_coverage_issues(
    template: &wow_db::SpellTemplateQuery,
) -> Vec<SpellCoverageIssue> {
    let spell_info = SpellInfo::from_template(template);

    let mut issues = Vec::new();

    for (index, effect) in spell_info.effects.into_iter().enumerate() {
        if effect.effect_id != 0 {
            let support = spell_effect_support(effect.effect_id);

            if support.blocks_runtime() {
                issues.push(SpellCoverageIssue {
                    spell_id: template.id,

                    effect_index: index,

                    mechanic: SpellCoverageMechanic::Effect,

                    mechanic_id: effect.effect_id,

                    mechanic_name: spell_effect_coverage_name(effect.effect_id),

                    support,
                });
            }
        }

        if effect.dispatch == SpellEffectDispatch::ApplyAura && effect.aura_name != 0 {
            let support = spell_aura_support(effect.aura_name);

            if support.blocks_runtime() {
                issues.push(SpellCoverageIssue {
                    spell_id: template.id,

                    effect_index: index,

                    mechanic: SpellCoverageMechanic::Aura,

                    mechanic_id: effect.aura_name,

                    mechanic_name: spell_aura_coverage_name(effect.aura_name),

                    support,
                });
            }
        }
    }

    issues
}

#[allow(dead_code)]
pub(in crate::world) async fn spell_coverage_issues_for_spell_ids(
    object_mgr: &ObjectMgr,

    world_db_pool: &MySqlPool,

    spell_ids: impl IntoIterator<Item = u32>,
) -> anyhow::Result<Vec<SpellCoverageIssue>> {
    let mut issues = Vec::new();

    for spell_id in spell_ids {
        let Some(template) = object_mgr.spell_template(world_db_pool, spell_id).await? else {
            warn!(
                spell_id,
                "Skipping spell coverage audit for missing spell_template row"
            );

            continue;
        };

        issues.extend(spell_template_coverage_issues(&template));
    }

    Ok(issues)
}

pub(in crate::world) fn spell_effect_coverage_name(effect_id: u32) -> &'static str {
    match effect_id {
        0 => "SPELL_EFFECT_NONE",

        SPELL_EFFECT_SCHOOL_DAMAGE => "SPELL_EFFECT_SCHOOL_DAMAGE",

        SPELL_EFFECT_TELEPORT_UNITS => "SPELL_EFFECT_TELEPORT_UNITS",

        SPELL_EFFECT_APPLY_AURA => "SPELL_EFFECT_APPLY_AURA",

        SPELL_EFFECT_HEAL => "SPELL_EFFECT_HEAL",

        SPELL_EFFECT_WEAPON_DAMAGE_NOSCHOOL => "SPELL_EFFECT_WEAPON_DAMAGE_NOSCHOOL",

        SPELL_EFFECT_CREATE_ITEM => "SPELL_EFFECT_CREATE_ITEM",

        SPELL_EFFECT_ENERGIZE => "SPELL_EFFECT_ENERGIZE",

        SPELL_EFFECT_LEAP => "SPELL_EFFECT_LEAP",

        SPELL_EFFECT_WEAPON_PERCENT_DAMAGE => "SPELL_EFFECT_WEAPON_PERCENT_DAMAGE",

        SPELL_EFFECT_TRIGGER_MISSILE => "SPELL_EFFECT_TRIGGER_MISSILE",

        33 => "SPELL_EFFECT_OPEN_LOCK",

        SPELL_EFFECT_TELEPORT_UNITS_FACE_CASTER => "SPELL_EFFECT_TELEPORT_UNITS_FACE_CASTER",

        SPELL_EFFECT_WEAPON_DAMAGE => "SPELL_EFFECT_WEAPON_DAMAGE",

        59 => "SPELL_EFFECT_OPEN_LOCK_ITEM",

        60 => "SPELL_EFFECT_PROFICIENCY",

        64 => "SPELL_EFFECT_TRIGGER_SPELL",

        SPELL_EFFECT_ADD_COMBO_POINTS => "SPELL_EFFECT_ADD_COMBO_POINTS",

        SPELL_EFFECT_CHARGE => "SPELL_EFFECT_CHARGE",

        SPELL_EFFECT_ATTACK_ME => "SPELL_EFFECT_ATTACK_ME",

        SPELL_EFFECT_DISPEL => "SPELL_EFFECT_DISPEL",

        SPELL_EFFECT_DISPEL_MECHANIC => "SPELL_EFFECT_DISPEL_MECHANIC",

        SPELL_EFFECT_POWER_BURN => "SPELL_EFFECT_POWER_BURN",

        SPELL_EFFECT_INTERRUPT_CAST => "SPELL_EFFECT_INTERRUPT_CAST",

        SPELL_EFFECT_PERSISTENT_AREA_AURA => "SPELL_EFFECT_PERSISTENT_AREA_AURA",

        SPELL_EFFECT_NORMALIZED_WEAPON_DMG => "SPELL_EFFECT_NORMALIZED_WEAPON_DMG",

        _ if effect_id < CMANGOS_MAX_SPELL_EFFECTS => "CMANGOS_SPELL_EFFECT",

        _ => "UNKNOWN_SPELL_EFFECT",
    }
}

#[allow(dead_code)]
pub(in crate::world) fn spell_aura_coverage_name(aura_type: u32) -> &'static str {
    match aura_type {
        0 => "SPELL_AURA_NONE",

        SPELL_AURA_PERIODIC_DAMAGE => "SPELL_AURA_PERIODIC_DAMAGE",

        SPELL_AURA_PERIODIC_HEAL => "SPELL_AURA_PERIODIC_HEAL",

        SPELL_AURA_MOD_CONFUSE => "SPELL_AURA_MOD_CONFUSE",

        SPELL_AURA_MOD_FEAR => "SPELL_AURA_MOD_FEAR",

        SPELL_AURA_MOD_STUN => "SPELL_AURA_MOD_STUN",

        SPELL_AURA_MOD_TAUNT => "SPELL_AURA_MOD_TAUNT",

        SPELL_AURA_MOD_BLOCK_PERCENT => "SPELL_AURA_MOD_BLOCK_PERCENT",

        SPELL_AURA_MOD_CRIT_PERCENT => "SPELL_AURA_MOD_CRIT_PERCENT",

        SPELL_AURA_MOD_DAMAGE_DONE => "SPELL_AURA_MOD_DAMAGE_DONE",

        SPELL_AURA_MOD_DAMAGE_TAKEN => "SPELL_AURA_MOD_DAMAGE_TAKEN",

        SPELL_AURA_DUMMY => "SPELL_AURA_DUMMY",

        SPELL_AURA_MOD_STEALTH_DETECT => "SPELL_AURA_MOD_STEALTH_DETECT",

        SPELL_AURA_MOD_INVISIBILITY_DETECTION => "SPELL_AURA_MOD_INVISIBILITY_DETECTION",

        SPELL_AURA_OBS_MOD_HEALTH => "SPELL_AURA_OBS_MOD_HEALTH",

        SPELL_AURA_PERIODIC_TRIGGER_SPELL => "SPELL_AURA_PERIODIC_TRIGGER_SPELL",

        SPELL_AURA_MOD_RESISTANCE => "SPELL_AURA_MOD_RESISTANCE",

        SPELL_AURA_PERIODIC_ENERGIZE => "SPELL_AURA_PERIODIC_ENERGIZE",

        SPELL_AURA_MOD_PACIFY => "SPELL_AURA_MOD_PACIFY",

        SPELL_AURA_MOD_ROOT => "SPELL_AURA_MOD_ROOT",

        SPELL_AURA_MOD_SILENCE => "SPELL_AURA_MOD_SILENCE",

        SPELL_AURA_MOD_STAT => "SPELL_AURA_MOD_STAT",

        SPELL_AURA_MOD_SKILL => "SPELL_AURA_MOD_SKILL",

        SPELL_AURA_MOD_INCREASE_SPEED => "SPELL_AURA_MOD_INCREASE_SPEED",

        SPELL_AURA_MOD_DECREASE_SPEED => "SPELL_AURA_MOD_DECREASE_SPEED",

        SPELL_AURA_PROC_TRIGGER_SPELL => "SPELL_AURA_PROC_TRIGGER_SPELL",

        SPELL_AURA_MOD_PACIFY_SILENCE => "SPELL_AURA_MOD_PACIFY_SILENCE",

        SPELL_AURA_MOD_DISARM => "SPELL_AURA_MOD_DISARM",

        SPELL_AURA_SCHOOL_ABSORB => "SPELL_AURA_SCHOOL_ABSORB",

        SPELL_AURA_REFLECT_SPELLS_SCHOOL => "SPELL_AURA_REFLECT_SPELLS_SCHOOL",

        SPELL_AURA_MANA_SHIELD => "SPELL_AURA_MANA_SHIELD",

        SPELL_AURA_AURAS_VISIBLE => "SPELL_AURA_AURAS_VISIBLE",

        SPELL_AURA_MOD_RESISTANCE_PCT => "SPELL_AURA_MOD_RESISTANCE_PCT",

        SPELL_AURA_MOD_REGEN => "SPELL_AURA_MOD_REGEN",

        SPELL_AURA_MOD_POWER_REGEN => "SPELL_AURA_MOD_POWER_REGEN",

        SPELL_AURA_MOD_HEALING_DONE => "SPELL_AURA_MOD_HEALING_DONE",

        SPELL_AURA_MOD_HEALING => "SPELL_AURA_MOD_HEALING",

        SPELL_AURA_MOD_SKILL_TALENT => "SPELL_AURA_MOD_SKILL_TALENT",

        SPELL_AURA_MOD_ATTACK_POWER => "SPELL_AURA_MOD_ATTACK_POWER",

        SPELL_AURA_MOD_TOTAL_STAT_PERCENTAGE => "SPELL_AURA_MOD_TOTAL_STAT_PERCENTAGE",

        SPELL_AURA_MOD_MELEE_HASTE => "SPELL_AURA_MOD_MELEE_HASTE",

        SPELL_AURA_MOD_REPUTATION_GAIN => "SPELL_AURA_MOD_REPUTATION_GAIN",

        SPELL_AURA_TRACK_CREATURES => "SPELL_AURA_TRACK_CREATURES",

        SPELL_AURA_TRACK_RESOURCES => "SPELL_AURA_TRACK_RESOURCES",

        SPELL_AURA_GHOST => "SPELL_AURA_GHOST",

        SPELL_AURA_WATER_WALK => "SPELL_AURA_WATER_WALK",

        SPELL_AURA_FEATHER_FALL => "SPELL_AURA_FEATHER_FALL",

        SPELL_AURA_HOVER => "SPELL_AURA_HOVER",

        _ if aura_type < CMANGOS_TOTAL_AURAS => "CMANGOS_SPELL_AURA",

        _ => "UNKNOWN_SPELL_AURA",
    }
}
