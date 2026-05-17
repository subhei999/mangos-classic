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
    Teleport,
    Leap,
    Charge,
    OpenLock,
    Dispel,
    DispelMechanic,
    PersistentAreaAura,
    LearnSpell,
    LearnSkill,
    TriggerSpell,
    TriggerMissile,
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
            SPELL_EFFECT_TELEPORT_UNITS | SPELL_EFFECT_TELEPORT_UNITS_FACE_CASTER => Self::Teleport,
            SPELL_EFFECT_LEAP => Self::Leap,
            SPELL_EFFECT_CHARGE => Self::Charge,
            SPELL_EFFECT_DISPEL => Self::Dispel,
            SPELL_EFFECT_DISPEL_MECHANIC => Self::DispelMechanic,
            SPELL_EFFECT_PERSISTENT_AREA_AURA => Self::PersistentAreaAura,
            33 | 59 => Self::OpenLock,
            36 => Self::LearnSpell,
            44 => Self::LearnSkill,
            SPELL_EFFECT_TRIGGER_MISSILE => Self::TriggerMissile,
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
        | SPELL_EFFECT_DISPEL
        | SPELL_EFFECT_PERSISTENT_AREA_AURA
        | SPELL_EFFECT_NORMALIZED_WEAPON_DMG => SpellMechanicSupport::Implemented,
        4 | 12 | 13 | 14 | 15 | 20 | 21 | 23 | 25 | 26 | 37 | 39 | 48 | 49 | 51 | 52 | 65 | 66
        | 78 | 81 | 91 | 122 | 126 | 127 => SpellMechanicSupport::KnownNoOp,
        1 => SpellMechanicSupport::Pending("instant kill"),
        3 | 77 => SpellMechanicSupport::Pending("dummy/script effect"),
        7 => SpellMechanicSupport::Pending("environmental damage"),
        8 | 62 => SpellMechanicSupport::Pending("power drain/burn"),
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
        SPELL_EFFECT_DISPEL_MECHANIC => SpellMechanicSupport::Pending("dispel mechanic"),
        40 => SpellMechanicSupport::Pending("dual wield"),
        44 | 118 => SpellMechanicSupport::Pending("skill modification"),
        45 => SpellMechanicSupport::Pending("honor"),
        46 => SpellMechanicSupport::Pending("spawn visual"),
        47 => SpellMechanicSupport::Pending("trade skill"),
        50 => SpellMechanicSupport::Pending("transport door"),
        53 | 54 | 92 => SpellMechanicSupport::Pending("item enchant"),
        55 => SpellMechanicSupport::Pending("tame creature"),
        56 | 57 | 101 | 102 | 109 => SpellMechanicSupport::Pending("pet"),
        61 => SpellMechanicSupport::Pending("game event"),
        63 | 125 => SpellMechanicSupport::Pending("threat"),
        67 | 75 => SpellMechanicSupport::Pending("special heal"),
        68 => SpellMechanicSupport::Pending("interrupt cast"),
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
        114 => SpellMechanicSupport::Pending("taunt"),
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
        | SPELL_AURA_MOD_DAMAGE_DONE
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
        | SPELL_AURA_PROC_TRIGGER_SPELL
        | SPELL_AURA_MOD_RESISTANCE_PCT
        | SPELL_AURA_MOD_REGEN
        | SPELL_AURA_MOD_POWER_REGEN
        | SPELL_AURA_MOD_SKILL_TALENT
        | SPELL_AURA_MOD_ATTACK_POWER
        | SPELL_AURA_MOD_TOTAL_STAT_PERCENTAGE
        | SPELL_AURA_MOD_MELEE_HASTE
        | SPELL_AURA_MOD_REPUTATION_GAIN
        | SPELL_AURA_TRACK_CREATURES
        | SPELL_AURA_TRACK_RESOURCES
        | SPELL_AURA_GHOST
        | SPELL_AURA_WATER_WALK
        | SPELL_AURA_MOD_CONFUSE
        | SPELL_AURA_MOD_FEAR
        | SPELL_AURA_MOD_PACIFY
        | SPELL_AURA_MOD_SILENCE
        | SPELL_AURA_MOD_PACIFY_SILENCE
        | SPELL_AURA_SCHOOL_ABSORB
        | SPELL_AURA_MANA_SHIELD
        | SPELL_AURA_FEATHER_FALL => SpellMechanicSupport::Implemented,
        46 | 48 | 164 => SpellMechanicSupport::KnownNoOp,
        1 | 76 | 82 | 106 | 144 => SpellMechanicSupport::Pending("movement/visibility state"),
        2 | 6 | 16 | 18 | 36 | 66 | 67 | 78 | 128 | 176 | 177 => {
            SpellMechanicSupport::Pending("control state")
        }
        43 | 107 | 108 | 109 | 111 | 112 => SpellMechanicSupport::Pending("trigger/script aura"),
        9 | 10 | 11 | 14 | 15 | 28 | 32 | 34 | 35 | 47 | 49 | 51 | 52 | 54 | 55 | 57 | 58 | 59
        | 65 | 70 | 71 | 72 | 73 | 79 | 80 | 83 | 87 | 88 | 89 | 90 | 91 | 102 | 103 | 110
        | 113 | 114 | 115 | 116 | 117 | 118 | 122 | 123 | 124 | 125 | 126 | 127 | 129 | 130
        | 131 | 132 | 133 | 134 | 135 | 136 | 140 | 141 | 142 | 143 | 147 | 149 | 150 | 152
        | 153 | 154 | 155 | 157 | 158 | 160 | 161 | 163 | 165 | 166 | 167 | 168 | 169 | 171
        | 172 | 174 | 175 | 178 | 179 | 180 | 181 | 182 | 183 | 184 | 185 | 186 | 187 | 188
        | 189 | 190 | 191 => SpellMechanicSupport::Pending("stat/combat modifier"),
        68 | 75 | 119 | 120 | 121 | 139 | 145 | 146 | 151 | 159 | 170 | 173 => {
            SpellMechanicSupport::Pending("tracking/reaction/client state")
        }
        21 | 53 | 62 | 63 | 64 | 81 | 86 | 96 | 162 => {
            SpellMechanicSupport::Pending("resource shield/funnel")
        }
        37 | 38 | 39 | 40 | 41 | 77 | 92 | 93 | 94 | 148 => {
            SpellMechanicSupport::Pending("immunity/special state")
        }
        50 | 56 | 61 | 74 | 100 => SpellMechanicSupport::Pending("visual/model/client state"),
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
        SPELL_EFFECT_DISPEL => "SPELL_EFFECT_DISPEL",
        SPELL_EFFECT_DISPEL_MECHANIC => "SPELL_EFFECT_DISPEL_MECHANIC",
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
        SPELL_AURA_MOD_DAMAGE_DONE => "SPELL_AURA_MOD_DAMAGE_DONE",
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
        SPELL_AURA_SCHOOL_ABSORB => "SPELL_AURA_SCHOOL_ABSORB",
        SPELL_AURA_MANA_SHIELD => "SPELL_AURA_MANA_SHIELD",
        SPELL_AURA_MOD_RESISTANCE_PCT => "SPELL_AURA_MOD_RESISTANCE_PCT",
        SPELL_AURA_MOD_REGEN => "SPELL_AURA_MOD_REGEN",
        SPELL_AURA_MOD_POWER_REGEN => "SPELL_AURA_MOD_POWER_REGEN",
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
        _ if aura_type < CMANGOS_TOTAL_AURAS => "CMANGOS_SPELL_AURA",
        _ => "UNKNOWN_SPELL_AURA",
    }
}

#[allow(clippy::too_many_arguments)]
pub(in crate::world) async fn apply_player_spell_effects(
    stream: &mut WorldPacketSink,
    deps: SpellCastDeps<'_>,
    session: &mut WorldSessionState,
    caster: ObjectGuid,
    character_guid: u32,
    character_level: u8,
    map_id: u32,
    spell_template: &wow_db::SpellTemplateQuery,
    spell_profile: &SpellCastProfile,
    targets: &SpellCastTargets,
    now: Instant,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    let spell_info = SpellInfo::from_template(spell_template);
    let mut charge_applied = false;
    let mut direct_heal_applied = false;
    let mut direct_energize_applied = false;
    let mut aura_applied = false;
    let mut create_item_applied = false;
    let mut weapon_damage_applied = false;
    let mut landed_damage = false;
    let mut direct_damage_processed = false;
    let mut deferred_hostile_aura = false;
    let mut learned_spells = HashSet::new();
    let spell_has_hostile_direct_damage = spell_info.effects.iter().any(|effect| {
        matches!(
            effect.dispatch,
            SpellEffectDispatch::SchoolDamage
                | SpellEffectDispatch::WeaponDamage
                | SpellEffectDispatch::WeaponPercentDamage
        ) && spell_info_effect_targets_hostile(*effect)
    });
    let combo_points_for_effects = spell_combo_points_for_effects(
        deps.shared_world,
        caster,
        character_guid,
        map_id,
        spell_profile,
        targets,
    )
    .await;
    let effect_value_context = player_spell_effect_value_context(
        deps.shared_world.maps,
        spell_template,
        &session.character.character_skills,
        combo_points_for_effects,
    );

    for (effect_index, effect) in spell_info.effects.into_iter().enumerate() {
        match effect.dispatch {
            SpellEffectDispatch::Empty => {}
            SpellEffectDispatch::Charge
                if spell_profile.kind == SpellCastKind::Charge && !charge_applied =>
            {
                apply_player_charge_effect(
                    stream,
                    deps.shared_world,
                    session,
                    caster,
                    map_id,
                    spell_template,
                    spell_profile,
                    targets,
                    header_crypto,
                )
                .await?;
                charge_applied = true;
            }
            SpellEffectDispatch::SchoolDamage
                if spell_profile.kind != SpellCastKind::Charge
                    && spell_profile.kind != SpellCastKind::NextMeleeSwing =>
            {
                if let Some(damage_effect) = player_direct_damage_effect(
                    spell_template,
                    spell_profile,
                    effect,
                    effect_value_context,
                ) {
                    landed_damage |= apply_player_direct_damage_effect(
                        stream,
                        deps,
                        session,
                        caster,
                        character_guid,
                        map_id,
                        damage_effect,
                        targets,
                        header_crypto,
                    )
                    .await?;
                    direct_damage_processed = true;
                }
            }
            SpellEffectDispatch::WeaponDamage | SpellEffectDispatch::WeaponPercentDamage
                if spell_profile.kind != SpellCastKind::Charge
                    && spell_profile.kind != SpellCastKind::NextMeleeSwing
                    && !weapon_damage_applied =>
            {
                landed_damage |= apply_player_direct_damage_effect(
                    stream,
                    deps,
                    session,
                    caster,
                    character_guid,
                    map_id,
                    player_weapon_damage_effect(spell_profile),
                    targets,
                    header_crypto,
                )
                .await?;
                direct_damage_processed = true;
                weapon_damage_applied = true;
            }
            SpellEffectDispatch::AddComboPoints if landed_damage => {
                apply_player_combo_points_effect(
                    stream,
                    deps.shared_world,
                    caster,
                    character_guid,
                    map_id,
                    effect,
                    targets,
                    header_crypto,
                )
                .await?;
            }
            SpellEffectDispatch::Heal
                if spell_profile.kind == SpellCastKind::DirectHeal && !direct_heal_applied =>
            {
                apply_player_direct_heal_effect(
                    stream,
                    deps,
                    session,
                    caster,
                    map_id,
                    &spell_info,
                    effect_value_context,
                    targets,
                    header_crypto,
                )
                .await?;
                direct_heal_applied = true;
            }
            SpellEffectDispatch::Energize if !direct_energize_applied => {
                apply_player_direct_energize_effect(
                    stream,
                    deps,
                    session,
                    caster,
                    character_guid,
                    map_id,
                    &spell_info,
                    effect_value_context,
                    header_crypto,
                )
                .await?;
                direct_energize_applied = true;
            }
            SpellEffectDispatch::CreateItem
                if spell_profile.kind == SpellCastKind::CreateItem && !create_item_applied =>
            {
                apply_player_create_item_effects(
                    stream,
                    deps,
                    session,
                    character_guid,
                    &spell_info,
                    effect_value_context,
                    header_crypto,
                )
                .await?;
                create_item_applied = true;
            }
            SpellEffectDispatch::Leap | SpellEffectDispatch::Teleport
                if spell_profile.kind == SpellCastKind::Teleport =>
            {
                apply_player_near_teleport_effect(
                    stream,
                    deps,
                    session,
                    character_guid,
                    map_id,
                    spell_template,
                    effect,
                    targets,
                    header_crypto,
                )
                .await?;
            }
            SpellEffectDispatch::ApplyAura
                if effect.aura_name == SPELL_AURA_PERIODIC_TRIGGER_SPELL
                    && (spell_template.attributes_ex
                        & (SPELL_ATTR_EX_IS_CHANNELED | SPELL_ATTR_EX_IS_SELF_CHANNELED))
                        != 0 =>
            {
                apply_player_periodic_trigger_channel_effect(
                    stream,
                    deps,
                    caster,
                    character_guid,
                    character_level,
                    map_id,
                    spell_template,
                    effect,
                    effect_value_context,
                    targets,
                    now,
                    header_crypto,
                )
                .await?;
                aura_applied = true;
            }
            SpellEffectDispatch::ApplyAura
                if matches!(
                    spell_profile.kind,
                    SpellCastKind::AuraApplication | SpellCastKind::DirectHeal
                ) && !aura_applied
                    && {
                        if spell_has_hostile_direct_damage
                            && spell_info_effect_targets_hostile(effect)
                            && !landed_damage
                        {
                            if !direct_damage_processed {
                                deferred_hostile_aura = true;
                            }
                            false
                        } else {
                            true
                        }
                    } =>
            {
                apply_player_spell_aura(
                    stream,
                    deps,
                    session,
                    caster,
                    character_guid,
                    character_level,
                    map_id,
                    spell_template,
                    spell_profile,
                    targets,
                    effect_value_context,
                    now,
                    header_crypto,
                )
                .await?;
                aura_applied = true;
            }
            SpellEffectDispatch::PersistentAreaAura => {
                apply_player_persistent_area_aura_effect(
                    stream,
                    deps,
                    session,
                    caster,
                    character_guid,
                    character_level,
                    map_id,
                    spell_template,
                    effect_index,
                    effect,
                    effect_value_context,
                    targets,
                    now,
                    header_crypto,
                )
                .await?;
            }
            SpellEffectDispatch::TriggerSpell if effect.trigger_spell != 0 => {
                apply_player_trigger_spell_effect(
                    stream,
                    deps,
                    session,
                    caster,
                    character_guid,
                    character_level,
                    map_id,
                    effect.trigger_spell,
                    targets,
                    now,
                    header_crypto,
                )
                .await?;
            }
            SpellEffectDispatch::Dispel => {
                apply_player_dispel_effect(
                    stream,
                    deps,
                    session,
                    caster,
                    character_guid,
                    map_id,
                    spell_template.id,
                    effect,
                    effect_value_context,
                    targets,
                    now,
                    header_crypto,
                )
                .await?;
            }
            SpellEffectDispatch::DispelMechanic => {
                warn!(
                    spell_id = spell_template.id,
                    mechanic = effect.misc_value,
                    "Skipping SPELL_EFFECT_DISPEL_MECHANIC until aura mechanic ownership is represented"
                );
            }
            SpellEffectDispatch::LearnSpell
                if effect.trigger_spell != 0 && learned_spells.insert(effect.trigger_spell) =>
            {
                apply_player_learn_spell_effect(
                    stream,
                    deps,
                    session,
                    character_guid,
                    effect.trigger_spell,
                    header_crypto,
                )
                .await?;
            }
            SpellEffectDispatch::Unsupported(effect_id) => {
                let support = spell_effect_support(effect_id);
                warn!(
                    spell_id = spell_template.id,
                    effect_id,
                    effect_name = spell_effect_coverage_name(effect_id),
                    ?support,
                    "Skipping unsupported player spell effect"
                );
            }
            _ => {}
        }
    }

    if deferred_hostile_aura && landed_damage && !aura_applied {
        apply_player_spell_aura(
            stream,
            deps,
            session,
            caster,
            character_guid,
            character_level,
            map_id,
            spell_template,
            spell_profile,
            targets,
            effect_value_context,
            now,
            header_crypto,
        )
        .await?;
    }

    if spell_profile.needs_combo_points && landed_damage {
        clear_player_combo_points_after_finisher(
            stream,
            deps.shared_world,
            caster,
            character_guid,
            map_id,
            header_crypto,
        )
        .await?;
    }

    Ok(())
}

fn spell_info_effect_targets_hostile(effect: SpellInfoEffect) -> bool {
    [effect.implicit_target_a, effect.implicit_target_b]
        .into_iter()
        .any(|target| {
            matches!(
                target,
                TARGET_UNIT_ENEMY
                    | TARGET_ENUM_UNITS_ENEMY_AOE_AT_SRC_LOC
                    | TARGET_ENUM_UNITS_ENEMY_AOE_AT_DEST_LOC
                    | TARGET_ENUM_UNITS_ENEMY_AOE_AT_DYNOBJ_LOC
                    | TARGET_LOCATION_CASTER_SRC
            )
        })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::world) struct CreateItemSpellEffect {
    pub(in crate::world) item_template: u32,
    pub(in crate::world) requested_count: u32,
}

pub(in crate::world) fn create_item_spell_effect(
    effect: SpellInfoEffect,
    value_context: SpellEffectValueContext,
) -> Option<CreateItemSpellEffect> {
    if effect.dispatch != SpellEffectDispatch::CreateItem || effect.item_type == 0 {
        return None;
    }
    Some(CreateItemSpellEffect {
        item_template: effect.item_type,
        requested_count: spell_effect_calculated_u32(effect, value_context)
            .unwrap_or(1)
            .max(1),
    })
}

pub(in crate::world) fn create_item_spell_effects(
    spell_info: &SpellInfo<'_>,
    value_context: SpellEffectValueContext,
) -> Vec<CreateItemSpellEffect> {
    spell_info
        .effects
        .into_iter()
        .filter_map(|effect| create_item_spell_effect(effect, value_context))
        .collect()
}

pub(in crate::world) fn create_item_count_for_template(
    effect: CreateItemSpellEffect,
    template: &ItemTemplateQuery,
) -> u32 {
    effect.requested_count.min(template.stackable.max(1)).max(1)
}

pub(in crate::world) async fn player_create_item_cast_inventory_failure(
    deps: SpellCastDeps<'_>,
    session: &WorldSessionState,
    spell_template: &wow_db::SpellTemplateQuery,
) -> anyhow::Result<Option<u8>> {
    let spell_info = SpellInfo::from_template(spell_template);
    let value_context = player_spell_effect_value_context(
        deps.shared_world.maps,
        spell_template,
        &session.character.character_skills,
        0,
    );
    let effects = create_item_spell_effects(&spell_info, value_context);
    if effects.is_empty() {
        return Ok(None);
    }
    let equipped_bags =
        load_equipped_bag_infos(deps.world_db_pool, &session.inventory.items).await?;
    for effect in effects {
        let Some(template) =
            wow_db::get_item_template_query(deps.world_db_pool, effect.item_template).await?
        else {
            warn!(
                spell_id = spell_template.id,
                item_template = effect.item_template,
                "Create-item spell references missing item_template row"
            );
            return Ok(Some(EQUIP_ERR_ITEM_NOT_FOUND));
        };
        let count = create_item_count_for_template(effect, &template);
        if plan_store_item(
            &session.inventory.items,
            &template,
            count,
            &equipped_bags,
            None,
            None,
        )
        .is_none()
        {
            return Ok(Some(EQUIP_ERR_INVENTORY_FULL));
        }
    }
    Ok(None)
}

pub(in crate::world) async fn apply_player_create_item_effects(
    stream: &mut WorldPacketSink,
    deps: SpellCastDeps<'_>,
    session: &mut WorldSessionState,
    character_guid: u32,
    spell_info: &SpellInfo<'_>,
    value_context: SpellEffectValueContext,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    let effects = create_item_spell_effects(spell_info, value_context);
    if effects.is_empty() {
        return Ok(());
    }
    let owner_guid = ObjectGuid::new(HighGuid::Player, 0, character_guid);
    let equipped_bags =
        load_equipped_bag_infos(deps.world_db_pool, &session.inventory.items).await?;
    let mut update_blocks = Vec::new();
    let mut push_results = Vec::new();

    for effect in effects {
        let Some(template) =
            wow_db::get_item_template_query(deps.world_db_pool, effect.item_template).await?
        else {
            warn!(
                spell_id = spell_info.template.id,
                item_template = effect.item_template,
                "Skipping create-item spell effect with missing item_template row"
            );
            send_inventory_change_failure(
                stream,
                EQUIP_ERR_ITEM_NOT_FOUND,
                None,
                None,
                header_crypto,
            )
            .await?;
            return Ok(());
        };
        let count = create_item_count_for_template(effect, &template);
        let Some(store_plan) = plan_store_item(
            &session.inventory.items,
            &template,
            count,
            &equipped_bags,
            None,
            None,
        ) else {
            send_inventory_change_failure(
                stream,
                EQUIP_ERR_INVENTORY_FULL,
                None,
                None,
                header_crypto,
            )
            .await?;
            return Ok(());
        };

        let random_properties = generate_item_instance_random_properties_for_template(
            deps.world_db_pool,
            &session.movement.db_creature_navigation.world_data_files,
            &template,
        )
        .await?;
        for slot in &store_plan {
            if let Some(item_guid) = slot.existing_item {
                let existing_count = session
                    .inventory
                    .items
                    .iter()
                    .find(|item| item.item == item_guid)
                    .map(|item| item.count)
                    .unwrap_or(0);
                wow_db::update_character_inventory_item_count(
                    deps.character_db_pool,
                    character_guid,
                    item_guid,
                    existing_count.saturating_add(slot.count),
                )
                .await?;
            } else {
                wow_db::add_character_inventory_item_with_random_properties(
                    deps.character_db_pool,
                    wow_db::AddCharacterInventoryItemRequest {
                        guid: character_guid,
                        bag: slot.bag as u32,
                        slot: slot.slot,
                        item_template: template.entry,
                        count: slot.count,
                        durability: template.max_durability,
                        random_properties: random_properties.as_ref(),
                    },
                )
                .await?;
            }
        }

        session.inventory.items =
            wow_db::get_character_inventory_items(deps.character_db_pool, character_guid).await?;
        for slot in &store_plan {
            if let Some(item_guid) = slot.existing_item {
                if let Some(item) = session
                    .inventory
                    .items
                    .iter()
                    .find(|item| item.item == item_guid)
                {
                    update_blocks.push(build_item_stack_count_update_block(item.item, item.count)?);
                    push_results.push(build_item_push_result_body(
                        character_guid,
                        item,
                        slot.count,
                        true,
                        true,
                        true,
                    ));
                }
                continue;
            }
            if let Some(new_item) = session
                .inventory
                .items
                .iter()
                .find(|item| item.bag == slot.bag as u32 && item.slot == slot.slot)
            {
                let contained_guid =
                    item_contained_guid(owner_guid, &session.inventory.items, new_item);
                update_blocks.push(build_item_create_update_block(
                    owner_guid,
                    contained_guid,
                    new_item,
                    (template.container_slots > 0).then_some(template.container_slots),
                )?);
                update_blocks.extend(build_inventory_position_update_blocks(
                    character_guid,
                    &session.inventory.items,
                    slot.bag,
                    slot.slot,
                )?);
                push_results.push(build_item_push_result_body(
                    character_guid,
                    new_item,
                    slot.count,
                    true,
                    true,
                    true,
                ));
            }
        }
    }

    for body in push_results {
        send_packet(
            stream,
            SMSG_ITEM_PUSH_RESULT,
            &body,
            Some(&mut *header_crypto),
        )
        .await?;
    }
    if !update_blocks.is_empty() {
        let body = build_update_object_body(&update_blocks);
        send_packet(stream, SMSG_UPDATE_OBJECT, &body, Some(header_crypto)).await?;
    }
    Ok(())
}

#[derive(Debug, Clone, Copy)]
pub(in crate::world) struct PlayerDirectDamageEffect {
    pub(in crate::world) spell_id: u32,
    pub(in crate::world) damage: u32,
    pub(in crate::world) weapon_damage_percent: u32,
    pub(in crate::world) school: u8,
    pub(in crate::world) dmg_class: u32,
    pub(in crate::world) attributes_ex2: u32,
    pub(in crate::world) attributes_ex3: u32,
    pub(in crate::world) requires_melee: bool,
    pub(in crate::world) uses_weapon_outcome: bool,
    pub(in crate::world) suppress_attacker_state: bool,
    pub(in crate::world) caster_centered_hostile_area: bool,
    pub(in crate::world) destination_hostile_area: bool,
    pub(in crate::world) radius_index: u32,
}

pub(in crate::world) fn player_direct_damage_effect(
    spell_template: &wow_db::SpellTemplateQuery,
    spell_profile: &SpellCastProfile,
    effect: SpellInfoEffect,
    value_context: SpellEffectValueContext,
) -> Option<PlayerDirectDamageEffect> {
    let damage = spell_effect_calculated_u32(effect, value_context)?;
    let school = match effect.dispatch {
        SpellEffectDispatch::SchoolDamage => spell_template.school as u8,
        _ => return None,
    };
    Some(PlayerDirectDamageEffect {
        spell_id: spell_profile.spell_id,
        damage,
        weapon_damage_percent: 100,
        school,
        dmg_class: spell_template.dmg_class,
        attributes_ex2: spell_template.attributes_ex2,
        attributes_ex3: spell_template.attributes_ex3,
        requires_melee: spell_profile.requires_melee,
        uses_weapon_outcome: false,
        suppress_attacker_state: effect.dispatch == SpellEffectDispatch::SchoolDamage,
        caster_centered_hostile_area: effect_targets_caster_centered_hostile_area(effect),
        destination_hostile_area: effect_targets_destination_hostile_area(effect),
        radius_index: effect.radius_index,
    })
}

pub(in crate::world) fn player_weapon_damage_effect(
    spell_profile: &SpellCastProfile,
) -> PlayerDirectDamageEffect {
    PlayerDirectDamageEffect {
        spell_id: spell_profile.spell_id,
        damage: spell_profile.bonus_damage,
        weapon_damage_percent: spell_profile.weapon_damage_percent,
        school: 0,
        dmg_class: SPELL_DAMAGE_CLASS_MELEE,
        attributes_ex2: 0,
        attributes_ex3: 0,
        requires_melee: spell_profile.requires_melee,
        uses_weapon_outcome: true,
        suppress_attacker_state: true,
        caster_centered_hostile_area: false,
        destination_hostile_area: false,
        radius_index: 0,
    }
}

pub(in crate::world) async fn spell_combo_points_for_effects(
    shared_world: SharedWorldDeps<'_>,
    caster: ObjectGuid,
    character_guid: u32,
    map_id: u32,
    spell_profile: &SpellCastProfile,
    targets: &SpellCastTargets,
) -> u8 {
    if !spell_profile.needs_combo_points {
        return 0;
    }
    let Some(target) = targets.unit_target else {
        return 0;
    };
    shared_world
        .maps
        .player_runtime_snapshot(map_id, character_guid)
        .await
        .filter(|snapshot| snapshot.combo_target == Some(target) || target == caster)
        .map(|snapshot| snapshot.combo_points)
        .unwrap_or(0)
}

#[allow(clippy::too_many_arguments)]
pub(in crate::world) async fn apply_player_charge_effect(
    stream: &mut WorldPacketSink,
    shared_world: SharedWorldDeps<'_>,
    session: &mut WorldSessionState,
    caster: ObjectGuid,
    map_id: u32,
    spell_template: &wow_db::SpellTemplateQuery,
    spell_profile: &SpellCastProfile,
    targets: &SpellCastTargets,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    let Some(target) = targets.unit_target else {
        return Ok(());
    };
    apply_charge_movement(
        stream,
        shared_world,
        session,
        caster,
        target,
        spell_template.speed,
        spell_profile.spell_id,
        header_crypto,
    )
    .await?;
    begin_db_creature_retaliation_if_needed(
        stream,
        shared_world,
        map_id,
        session,
        target,
        caster,
        header_crypto,
    )
    .await
}

#[allow(clippy::too_many_arguments)]
pub(in crate::world) async fn apply_player_direct_damage_effect(
    stream: &mut WorldPacketSink,
    deps: SpellCastDeps<'_>,
    session: &mut WorldSessionState,
    caster: ObjectGuid,
    character_guid: u32,
    map_id: u32,
    damage_effect: PlayerDirectDamageEffect,
    targets: &SpellCastTargets,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<bool> {
    if damage_effect.caster_centered_hostile_area {
        let Some(radius) = deps
            .shared_world
            .maps
            .spell_radius(damage_effect.radius_index)
            .map(|entry| entry.radius)
            .filter(|radius| *radius > 0.0)
        else {
            warn!(
                spell_id = damage_effect.spell_id,
                radius_index = damage_effect.radius_index,
                "Skipping caster-centered AoE damage with missing SpellRadius.dbc row"
            );
            return Ok(false);
        };
        let targets = deps
            .shared_world
            .maps
            .nearby_attackable_db_creature_guids_for_player_spell(map_id, character_guid, radius)
            .await;
        let mut landed = false;
        for target in targets {
            let area_targets = SpellCastTargets {
                target_mask: SPELL_CAST_TARGET_UNIT,
                unit_target: Some(target),
                gameobject_target: None,
                source_location: None,
                destination: None,
            };
            landed |= apply_db_creature_spell_damage(
                stream,
                deps,
                session,
                caster,
                character_guid,
                map_id,
                damage_effect,
                &area_targets,
                header_crypto,
            )
            .await?;
        }
        return Ok(landed);
    }
    if damage_effect.destination_hostile_area {
        let Some(radius) = deps
            .shared_world
            .maps
            .spell_radius(damage_effect.radius_index)
            .map(|entry| entry.radius)
            .filter(|radius| *radius > 0.0)
        else {
            warn!(
                spell_id = damage_effect.spell_id,
                radius_index = damage_effect.radius_index,
                "Skipping destination AoE damage with missing SpellRadius.dbc row"
            );
            return Ok(false);
        };
        let Some(destination) = spell_target_destination_position(map_id, targets) else {
            warn!(
                spell_id = damage_effect.spell_id,
                "Skipping destination AoE damage with missing target destination"
            );
            return Ok(false);
        };
        let targets = deps
            .shared_world
            .maps
            .nearby_attackable_db_creature_guids_for_player_spell_at_position(
                map_id,
                character_guid,
                destination,
                radius,
            )
            .await;
        let mut landed = false;
        for target in targets {
            let area_targets = SpellCastTargets {
                target_mask: SPELL_CAST_TARGET_UNIT,
                unit_target: Some(target),
                gameobject_target: None,
                source_location: None,
                destination: None,
            };
            landed |= apply_db_creature_spell_damage(
                stream,
                deps,
                session,
                caster,
                character_guid,
                map_id,
                damage_effect,
                &area_targets,
                header_crypto,
            )
            .await?;
        }
        return Ok(landed);
    }
    apply_db_creature_spell_damage(
        stream,
        deps,
        session,
        caster,
        character_guid,
        map_id,
        damage_effect,
        targets,
        header_crypto,
    )
    .await
}

pub(in crate::world) fn spell_target_destination_position(
    map_id: u32,
    targets: &SpellCastTargets,
) -> Option<WorldPosition> {
    let destination = targets.destination?;
    Some(WorldPosition::new(
        map_id,
        destination.x,
        destination.y,
        destination.z,
        0.0,
    ))
}

#[allow(clippy::too_many_arguments)]
pub(in crate::world) async fn apply_player_direct_energize_effect(
    stream: &mut WorldPacketSink,
    deps: SpellCastDeps<'_>,
    session: &mut WorldSessionState,
    caster: ObjectGuid,
    character_guid: u32,
    map_id: u32,
    spell_info: &SpellInfo<'_>,
    value_context: SpellEffectValueContext,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    let energize = spell_direct_energize(spell_info, value_context);
    if energize == 0 {
        return Ok(());
    }
    if spell_info.template.power_type == POWER_TYPE_RAGE {
        let old_rage = session.character.player_rage;
        session.character.player_rage = session
            .character
            .player_rage
            .saturating_add(energize)
            .min(POWER_RAGE_DEFAULT);
        let amount = session.character.player_rage.saturating_sub(old_rage);
        if amount == 0 {
            return Ok(());
        }
        deps.shared_world
            .maps
            .set_player_power2(map_id, character_guid, session.character.player_rage)
            .await;
        send_packet(
            stream,
            SMSG_SPELLENERGIZELOG,
            &build_spell_energize_log_body(
                caster,
                caster,
                spell_info.template.id,
                POWER_TYPE_RAGE,
                amount,
            )?,
            Some(&mut *header_crypto),
        )
        .await?;
        send_packet(
            stream,
            SMSG_UPDATE_OBJECT,
            &build_player_rage_update_body(caster, session.character.player_rage)?,
            Some(header_crypto),
        )
        .await?;
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub(in crate::world) async fn apply_player_dispel_effect(
    stream: &mut WorldPacketSink,
    deps: SpellCastDeps<'_>,
    session: &mut WorldSessionState,
    caster: ObjectGuid,
    character_guid: u32,
    map_id: u32,
    spell_id: u32,
    effect: SpellInfoEffect,
    value_context: SpellEffectValueContext,
    targets: &SpellCastTargets,
    now: Instant,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    let Ok(dispel_type) = u32::try_from(effect.misc_value) else {
        return Ok(());
    };
    if dispel_type == 0 {
        return Ok(());
    }
    let count = spell_effect_calculated_u32(effect, value_context)
        .unwrap_or(1)
        .max(1);
    let target = targets.unit_target.unwrap_or(caster);
    if target.is_player() {
        let target_character_guid = target.counter();
        if target_character_guid == character_guid {
            remove_session_auras_by_dispel_type(
                &mut session.auras.active_auras,
                dispel_type,
                count,
            );
        }
        let Some(event) = deps
            .shared_world
            .maps
            .remove_player_auras_by_dispel_type(
                map_id,
                target_character_guid,
                dispel_type,
                count,
                now,
            )
            .await?
        else {
            return Ok(());
        };
        send_packet(
            stream,
            SMSG_SPELLDISPELLOG,
            &build_spell_dispel_log_body(target, caster, &event.removed_spell_ids)?,
            Some(&mut *header_crypto),
        )
        .await?;
        send_or_dispatch_player_aura_event(
            stream,
            deps.shared_world,
            character_guid,
            target_character_guid,
            event.aura_update,
            header_crypto,
        )
        .await?;
    } else if target.is_creature() {
        let Some(event) = deps
            .shared_world
            .maps
            .remove_db_creature_auras_by_dispel_type(
                map_id,
                target,
                character_guid,
                dispel_type,
                count,
                now,
            )
            .await?
        else {
            return Ok(());
        };
        send_packet(
            stream,
            SMSG_SPELLDISPELLOG,
            &build_spell_dispel_log_body(target, caster, &event.removed_spell_ids)?,
            Some(&mut *header_crypto),
        )
        .await?;
        send_packet(
            stream,
            SMSG_UPDATE_OBJECT,
            &event.aura_update.update_body,
            Some(&mut *header_crypto),
        )
        .await?;
        for packet in event.aura_update.direct_packets {
            send_packet(
                stream,
                packet.opcode,
                &packet.body,
                Some(&mut *header_crypto),
            )
            .await?;
        }
        deps.shared_world
            .sessions
            .dispatch(event.aura_update.observer_packets)
            .await;
    }
    debug!(spell_id, dispel_type, count, "Applied player dispel effect");
    Ok(())
}

pub(in crate::world) fn remove_session_auras_by_dispel_type(
    active_auras: &mut Vec<ActiveAura>,
    dispel_type: u32,
    count: u32,
) -> Vec<u32> {
    let mut remaining = count.max(1) as usize;
    let mut removed = Vec::new();
    active_auras.retain(|aura| {
        if remaining == 0 || !active_aura_matches_dispel_type(aura, dispel_type) {
            return true;
        }
        removed.push(aura.spell_id);
        remaining -= 1;
        false
    });
    removed
}

pub(in crate::world) fn build_spell_dispel_log_body(
    target: ObjectGuid,
    caster: ObjectGuid,
    removed_spell_ids: &[u32],
) -> anyhow::Result<Vec<u8>> {
    let mut body = Vec::with_capacity(8 + 8 + 4 + removed_spell_ids.len() * 4);
    body.extend_from_slice(&target.raw().to_le_bytes());
    body.extend_from_slice(&caster.raw().to_le_bytes());
    body.extend_from_slice(&(removed_spell_ids.len() as u32).to_le_bytes());
    for spell_id in removed_spell_ids {
        body.extend_from_slice(&spell_id.to_le_bytes());
    }
    Ok(body)
}

#[allow(clippy::too_many_arguments)]
pub(in crate::world) async fn apply_player_trigger_spell_effect(
    stream: &mut WorldPacketSink,
    deps: SpellCastDeps<'_>,
    session: &mut WorldSessionState,
    caster: ObjectGuid,
    character_guid: u32,
    character_level: u8,
    map_id: u32,
    triggered_spell_id: u32,
    targets: &SpellCastTargets,
    now: Instant,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    let Some(triggered_template) = deps
        .shared_world
        .object_mgr
        .spell_template(deps.world_db_pool, triggered_spell_id)
        .await?
    else {
        warn!(
            triggered_spell_id,
            "Skipping trigger-spell effect with missing spell_template row"
        );
        return Ok(());
    };
    let triggered_info = SpellInfo::from_template(&triggered_template);
    let Some(triggered_profile) = triggered_info.player_cast_profile() else {
        warn!(
            triggered_spell_id,
            "Skipping trigger-spell effect with unsupported triggered spell shape"
        );
        return Ok(());
    };
    let triggered_value_context = player_spell_effect_value_context(
        deps.shared_world.maps,
        &triggered_template,
        &session.character.character_skills,
        0,
    );
    match triggered_profile.kind {
        SpellCastKind::AuraApplication | SpellCastKind::DirectHeal => {
            apply_player_spell_aura(
                stream,
                deps,
                session,
                caster,
                character_guid,
                character_level,
                map_id,
                &triggered_template,
                &triggered_profile,
                targets,
                triggered_value_context,
                now,
                header_crypto,
            )
            .await?;
        }
        SpellCastKind::InstantDamage => {
            for effect in triggered_info.effects {
                if let Some(damage_effect) = player_direct_damage_effect(
                    &triggered_template,
                    &triggered_profile,
                    effect,
                    triggered_value_context,
                ) {
                    apply_player_direct_damage_effect(
                        stream,
                        deps,
                        session,
                        caster,
                        character_guid,
                        map_id,
                        damage_effect,
                        targets,
                        header_crypto,
                    )
                    .await?;
                }
            }
        }
        _ => {}
    }
    Ok(())
}

pub(in crate::world) async fn apply_player_learn_spell_effect(
    stream: &mut WorldPacketSink,
    deps: SpellCastDeps<'_>,
    session: &mut WorldSessionState,
    character_guid: u32,
    learned_spell_id: u32,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    if learned_spell_id == 0 || session.character.active_spells.contains(&learned_spell_id) {
        return Ok(());
    }
    let Some(_) =
        wow_db::learn_character_spell(deps.character_db_pool, character_guid, learned_spell_id, 0)
            .await?
    else {
        return Ok(());
    };
    session.character.active_spells.insert(learned_spell_id);
    send_packet(
        stream,
        SMSG_LEARNED_SPELL,
        &build_learned_spell_body(learned_spell_id),
        Some(&mut *header_crypto),
    )
    .await?;
    let known_spells = wow_db::get_character_spells(deps.character_db_pool, character_guid).await?;
    send_known_proficiencies(
        stream,
        deps.world_db_pool,
        &known_spells,
        Some(&mut *header_crypto),
    )
    .await?;
    send_packet(
        stream,
        SMSG_INITIAL_SPELLS,
        &build_initial_spells_body(&known_spells),
        Some(header_crypto),
    )
    .await
}

#[allow(clippy::too_many_arguments)]
pub(in crate::world) async fn apply_player_direct_heal_effect(
    stream: &mut WorldPacketSink,
    deps: SpellCastDeps<'_>,
    session: &mut WorldSessionState,
    caster: ObjectGuid,
    map_id: u32,
    spell_info: &SpellInfo<'_>,
    value_context: SpellEffectValueContext,
    targets: &SpellCastTargets,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    let heal = spell_direct_heal(spell_info, value_context);
    if heal == 0 {
        return Ok(());
    }
    let Some(target) = targets.unit_target.filter(|target| target.is_player()) else {
        return Ok(());
    };
    let Some(event) = deps
        .shared_world
        .maps
        .apply_player_heal(map_id, target.counter(), heal)
        .await?
    else {
        return Ok(());
    };
    send_player_spell_log_to_target_set(
        stream,
        deps.shared_world,
        character_guid_from_caster(caster),
        event.healed_character_guid,
        event.direct_session_id,
        &event.observer_packets,
        OutboundWorldPacket {
            opcode: SMSG_SPELLHEALLOG,
            body: build_spell_heal_log_body(
                caster,
                target,
                spell_info.template.id,
                event.amount_healed,
                false,
            )?,
        },
        header_crypto,
    )
    .await?;
    if event.healed_character_guid == caster.counter() {
        session.character.player_health = event.health;
        for packet in event.direct_packets {
            send_packet(
                stream,
                packet.opcode,
                &packet.body,
                Some(&mut *header_crypto),
            )
            .await?;
        }
    } else {
        deps.shared_world
            .sessions
            .dispatch(
                event
                    .direct_packets
                    .into_iter()
                    .map(|packet| (event.direct_session_id, packet))
                    .collect(),
            )
            .await;
    }
    deps.shared_world
        .sessions
        .dispatch(event.observer_packets)
        .await;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub(in crate::world) async fn send_player_spell_log_to_target_set(
    stream: &mut WorldPacketSink,
    shared_world: SharedWorldDeps<'_>,
    caster_character_guid: u32,
    target_character_guid: u32,
    target_session_id: SessionId,
    observer_packets: &[(SessionId, OutboundWorldPacket)],
    packet: OutboundWorldPacket,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    send_packet(
        stream,
        packet.opcode,
        &packet.body,
        Some(&mut *header_crypto),
    )
    .await?;

    let caster_session_id = shared_world
        .sessions
        .session_for_character(caster_character_guid)
        .await;
    let mut dispatch = Vec::new();
    let mut seen = HashSet::new();
    if Some(target_session_id) != caster_session_id
        || target_character_guid != caster_character_guid
    {
        seen.insert(target_session_id);
        dispatch.push((target_session_id, packet.clone()));
    }
    for (session_id, _) in observer_packets {
        if Some(*session_id) == caster_session_id || !seen.insert(*session_id) {
            continue;
        }
        dispatch.push((*session_id, packet.clone()));
    }
    shared_world.sessions.dispatch(dispatch).await;
    Ok(())
}

pub(in crate::world) async fn send_or_dispatch_player_aura_event(
    stream: &mut WorldPacketSink,
    shared_world: SharedWorldDeps<'_>,
    current_character_guid: u32,
    target_character_guid: u32,
    event: PlayerAuraUpdateEvent,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    let current_session_id = shared_world
        .sessions
        .session_for_character(current_character_guid)
        .await;
    let mut dispatch = Vec::new();

    if target_character_guid == current_character_guid {
        for packet in event.direct_packets {
            send_packet(
                stream,
                packet.opcode,
                &packet.body,
                Some(&mut *header_crypto),
            )
            .await?;
        }
    } else if let Some(target_session_id) = shared_world
        .sessions
        .session_for_character(target_character_guid)
        .await
    {
        dispatch.extend(
            event
                .direct_packets
                .into_iter()
                .map(|packet| (target_session_id, packet)),
        );
    }

    for (session_id, packet) in event.observer_packets {
        if Some(session_id) == current_session_id {
            send_packet(
                stream,
                packet.opcode,
                &packet.body,
                Some(&mut *header_crypto),
            )
            .await?;
        } else {
            dispatch.push((session_id, packet));
        }
    }

    shared_world.sessions.dispatch(dispatch).await;
    Ok(())
}

pub(in crate::world) fn character_guid_from_caster(caster: ObjectGuid) -> u32 {
    caster.counter()
}

#[allow(clippy::too_many_arguments)]
pub(in crate::world) async fn apply_db_creature_spell_damage(
    stream: &mut WorldPacketSink,
    deps: SpellCastDeps<'_>,
    session: &mut WorldSessionState,
    caster: ObjectGuid,
    character_guid: u32,
    map_id: u32,
    damage_effect: PlayerDirectDamageEffect,
    targets: &SpellCastTargets,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<bool> {
    let Some(target) = targets.unit_target else {
        return Ok(false);
    };
    let can_apply_damage = if damage_effect.requires_melee {
        db_creature_player_melee_check_from_map(deps.shared_world, session, target).await
            == PlayerMeleeCheck::Clear
    } else {
        true
    };
    if !can_apply_damage {
        return Ok(false);
    }

    let Some(target_creature) = deps
        .shared_world
        .maps
        .db_creature_snapshot(map_id, target)
        .await
    else {
        return Ok(false);
    };
    let melee_outcome = if damage_effect.uses_weapon_outcome {
        let combat_stats = deps
            .shared_world
            .maps
            .player_combat_stats(map_id, character_guid)
            .await
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "map-owned player combat stats missing for character {}",
                    character_guid
                )
            })?;
        let weapon_skill_id =
            main_hand_weapon_skill_id(deps.world_db_pool, &session.inventory.items).await?;
        let attacker_skill = weapon_skill_id
            .map(|skill_id| {
                current_skill_value_with_active_auras(
                    &session.character.character_skills,
                    &session.auras.active_auras,
                    skill_id,
                )
            })
            .unwrap_or(0);
        let character_level = session
            .character
            .active_character
            .as_ref()
            .map(|character| character.level)
            .unwrap_or(1);
        Some(
            player_main_hand_melee_outcome_against_db_creature(
                &combat_stats,
                character_level,
                attacker_skill,
                &target_creature,
            )
            .with_weapon_spell_modifier(damage_effect.damage, damage_effect.weapon_damage_percent),
        )
    } else {
        None
    };
    let spell_damage_outcome = if melee_outcome.is_none() {
        let combat_stats = deps
            .shared_world
            .maps
            .player_combat_stats(map_id, character_guid)
            .await
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "map-owned player combat stats missing for character {}",
                    character_guid
                )
            })?;
        let character = session.character.active_character.as_ref();
        Some(roll_spell_damage_outcome(spell_damage_outcome_input(
            damage_effect.damage,
            damage_effect.school,
            damage_effect.dmg_class,
            damage_effect.attributes_ex2,
            damage_effect.attributes_ex3,
            player_spell_snapshot(
                character.map(|character| character.level).unwrap_or(1),
                character.map(|character| character.class).unwrap_or(1),
                &combat_stats,
            ),
            db_creature_spell_snapshot(&target_creature),
        )))
    } else {
        None
    };
    let requested_damage = melee_outcome
        .map(|outcome| outcome.total_damage)
        .or_else(|| spell_damage_outcome.map(|outcome| outcome.final_damage))
        .unwrap_or(damage_effect.damage);

    let corpse_loot = if requested_damage >= target_creature.health {
        Some(
            prepare_db_creature_corpse_loot(
                deps.shared_world.object_mgr,
                deps.world_db_pool,
                deps.parties,
                session,
                character_guid,
                target_creature.spawn.entry,
            )
            .await?,
        )
    } else {
        None
    };
    if let Some(event) = deps
        .shared_world
        .maps
        .apply_db_creature_damage(
            map_id,
            DbCreatureDamageRequest {
                creature_guid: target,
                killer: caster,
                damage: requested_damage,
                melee_outcome,
                spell_damage_outcome,
                spell_id: Some(damage_effect.spell_id),
                spell_school: damage_effect.school,
                suppress_attacker_state: damage_effect.suppress_attacker_state,
                now: Instant::now(),
                now_epoch_secs: current_unix_epoch_secs(),
                exclude_character_guid: Some(character_guid),
                corpse_loot,
            },
        )
        .await?
    {
        let death_finalization = event.death_finalization;
        let target_switch = event.target_switch;
        let is_dead = death_finalization.is_some();
        mirror_session_db_creature(session, target.raw(), event.creature.clone());
        if is_dead {
            mirror_session_player_auto_attack(session, None, None);
            deps.shared_world
                .maps
                .set_player_auto_attack(map_id, character_guid, None, None)
                .await;
            clear_db_creature_combat_if_attacker(session, target);
        }
        if let Some(spell_non_melee_log_body) = &event.spell_non_melee_log_body {
            send_packet(
                stream,
                SMSG_SPELLNONMELEEDAMAGELOG,
                spell_non_melee_log_body,
                Some(&mut *header_crypto),
            )
            .await?;
        }
        if let Some(spell_miss_log_body) = &event.spell_miss_log_body {
            send_packet(
                stream,
                SMSG_SPELLLOGMISS,
                spell_miss_log_body,
                Some(&mut *header_crypto),
            )
            .await?;
        }
        if let Some(attacker_state_body) = &event.attacker_state_body {
            send_packet(
                stream,
                SMSG_ATTACKERSTATEUPDATE,
                attacker_state_body,
                Some(&mut *header_crypto),
            )
            .await?;
        }
        let creature_update_body = event.update_body.clone();
        send_packet(
            stream,
            SMSG_UPDATE_OBJECT,
            &creature_update_body,
            Some(&mut *header_crypto),
        )
        .await?;
        for packet in event.direct_packets {
            send_packet(
                stream,
                packet.opcode,
                &packet.body,
                Some(&mut *header_crypto),
            )
            .await?;
        }
        let broadcast = CreatureCombatBroadcast {
            shared_world: deps.shared_world,
            map_id,
            player: caster,
        };
        deps.shared_world
            .sessions
            .dispatch(event.observer_packets)
            .await;
        if is_dead {
            send_db_creature_motion_stop(stream, broadcast, session, target, header_crypto).await?;
            finalize_db_creature_death(
                stream,
                CombatRewardDeps {
                    character_db_pool: deps.character_db_pool,
                    world_db_pool: deps.world_db_pool,
                    shared_world: deps.shared_world,
                    parties: deps.parties,
                },
                session,
                death_finalization,
                header_crypto,
            )
            .await?;
        } else {
            send_db_creature_threat_target_switch(
                stream,
                deps.shared_world,
                session,
                target_switch,
                header_crypto,
            )
            .await?;
            begin_shared_db_creature_combat(deps.shared_world, session, target, Instant::now())
                .await;
            try_process_db_creature_event_ai_hp_actions(
                stream,
                deps.shared_world,
                deps.world_db_pool,
                session,
                map_id,
                target,
                caster,
                Instant::now(),
                header_crypto,
            )
            .await?;
        }
        return Ok(requested_damage > 0);
    }
    Ok(false)
}

#[allow(clippy::too_many_arguments)]
pub(in crate::world) async fn apply_player_combo_points_effect(
    stream: &mut WorldPacketSink,
    shared_world: SharedWorldDeps<'_>,
    caster: ObjectGuid,
    character_guid: u32,
    map_id: u32,
    effect: SpellInfoEffect,
    targets: &SpellCastTargets,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    let Some(target) = targets.unit_target else {
        return Ok(());
    };
    let Some(points) = spell_effect_simple_value(effect.base_points) else {
        return Ok(());
    };
    let Some(event) = shared_world
        .maps
        .add_player_combo_points(map_id, character_guid, target, points as u8)
        .await
    else {
        return Ok(());
    };
    let body = build_player_combo_points_update_body(
        caster,
        event.combo_target,
        event.combo_points,
        event.player_bytes,
    )?;
    send_packet(stream, SMSG_UPDATE_OBJECT, &body, Some(header_crypto)).await
}

pub(in crate::world) async fn clear_player_combo_points_after_finisher(
    stream: &mut WorldPacketSink,
    shared_world: SharedWorldDeps<'_>,
    caster: ObjectGuid,
    character_guid: u32,
    map_id: u32,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    let Some(event) = shared_world
        .maps
        .clear_player_combo_points(map_id, character_guid)
        .await
    else {
        return Ok(());
    };
    let body = build_player_combo_points_update_body(
        caster,
        event.combo_target,
        event.combo_points,
        event.player_bytes,
    )?;
    send_packet(stream, SMSG_UPDATE_OBJECT, &body, Some(header_crypto)).await
}

#[allow(clippy::too_many_arguments)]
pub(in crate::world) async fn apply_player_spell_aura(
    stream: &mut WorldPacketSink,
    deps: SpellCastDeps<'_>,
    session: &mut WorldSessionState,
    caster: ObjectGuid,
    character_guid: u32,
    character_level: u8,
    map_id: u32,
    spell_template: &wow_db::SpellTemplateQuery,
    spell_profile: &SpellCastProfile,
    targets: &SpellCastTargets,
    value_context: SpellEffectValueContext,
    now: Instant,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    let mut aura = build_active_aura(
        spell_template,
        caster,
        character_level,
        value_context,
        now,
        deps.shared_world
            .maps
            .spell_duration(spell_template.duration_index),
    );
    resolve_active_aura_transform_displays(
        deps.shared_world.object_mgr,
        deps.world_db_pool,
        &mut aura,
    )
    .await?;
    let suppress_hostile_refs = active_aura_suppresses_hostile_refs(&aura);
    match spell_profile.aura_target {
        SpellAuraTarget::Caster => {
            let resolution = aura_rank_conflict_resolution(
                deps.shared_world.object_mgr,
                deps.world_db_pool,
                spell_template.id,
                caster,
                &session.auras.active_auras,
            )
            .await?;
            if resolution.failure.is_some() {
                return Ok(());
            }
            apply_player_aura_replacing_conflicts(session, aura.clone(), &resolution);
            if let Some(event) = deps
                .shared_world
                .maps
                .apply_player_aura_replacing_conflicts(map_id, character_guid, aura, &resolution)
                .await?
            {
                send_or_dispatch_player_aura_event(
                    stream,
                    deps.shared_world,
                    character_guid,
                    character_guid,
                    event,
                    header_crypto,
                )
                .await?;
            } else {
                send_packet(
                    stream,
                    SMSG_UPDATE_OBJECT,
                    &build_player_aura_update_body(caster, &session.auras.active_auras)?,
                    Some(&mut *header_crypto),
                )
                .await?;
                for packet in
                    build_player_aura_duration_update_packets(&session.auras.active_auras, now)
                {
                    send_packet(
                        stream,
                        packet.opcode,
                        &packet.body,
                        Some(&mut *header_crypto),
                    )
                    .await?;
                }
            }
        }
        SpellAuraTarget::UnitTarget => {
            if let Some(target) = targets.unit_target {
                if target.is_player() {
                    let target_character_guid = target.counter();
                    let active_auras = if target_character_guid == character_guid {
                        session.auras.active_auras.clone()
                    } else {
                        let Some(snapshot) = deps
                            .shared_world
                            .maps
                            .player_runtime_snapshot(map_id, target_character_guid)
                            .await
                        else {
                            return Ok(());
                        };
                        snapshot.active_auras
                    };
                    let resolution = aura_rank_conflict_resolution(
                        deps.shared_world.object_mgr,
                        deps.world_db_pool,
                        spell_template.id,
                        caster,
                        &active_auras,
                    )
                    .await?;
                    if resolution.failure.is_some() {
                        return Ok(());
                    }
                    if target_character_guid == character_guid {
                        apply_player_aura_replacing_conflicts(session, aura.clone(), &resolution);
                    }
                    if let Some(event) = deps
                        .shared_world
                        .maps
                        .apply_player_aura_replacing_conflicts(
                            map_id,
                            target_character_guid,
                            aura,
                            &resolution,
                        )
                        .await?
                    {
                        send_or_dispatch_player_aura_event(
                            stream,
                            deps.shared_world,
                            character_guid,
                            target_character_guid,
                            event,
                            header_crypto,
                        )
                        .await?;
                    }
                } else if target.is_creature() {
                    let Some(target_creature) = deps
                        .shared_world
                        .maps
                        .db_creature_snapshot(map_id, target)
                        .await
                    else {
                        return Ok(());
                    };
                    augment_mage_polymorph_regen_from_helper_spell(
                        deps.shared_world.object_mgr,
                        deps.world_db_pool,
                        spell_template,
                        &mut aura,
                        now,
                        target_creature.max_health(),
                    )
                    .await?;
                    let single_target_descriptor = single_target_aura_descriptor(
                        deps.shared_world.object_mgr,
                        deps.world_db_pool,
                        spell_template,
                    )
                    .await?;
                    let diminishing_group = spell_diminishing_group(spell_template);
                    if let Some(group) = diminishing_group {
                        let level = deps
                            .shared_world
                            .maps
                            .current_diminishing_level(map_id, target, group, now)
                            .await
                            .unwrap_or(DiminishingLevelRuntime::Level1);
                        let adjusted_duration =
                            diminishing_duration_millis(aura.duration_millis, level).unwrap_or(0);
                        if adjusted_duration == 0 {
                            return Ok(());
                        }
                        aura.duration_millis = Some(adjusted_duration);
                        aura.expires_at =
                            Some(now + Duration::from_millis(adjusted_duration as u64));
                    }
                    let resolution = aura_rank_conflict_resolution(
                        deps.shared_world.object_mgr,
                        deps.world_db_pool,
                        spell_template.id,
                        caster,
                        &target_creature.active_auras,
                    )
                    .await?;
                    if resolution.failure.is_some() {
                        return Ok(());
                    }
                    if let Some(event) = deps
                        .shared_world
                        .maps
                        .apply_db_creature_aura_replacing_conflicts(
                            map_id,
                            target,
                            character_guid,
                            aura,
                            &resolution,
                            single_target_descriptor,
                            diminishing_group,
                            now,
                        )
                        .await?
                    {
                        send_packet(
                            stream,
                            SMSG_UPDATE_OBJECT,
                            &event.update_body,
                            Some(&mut *header_crypto),
                        )
                        .await?;
                        for packet in event.direct_packets {
                            send_packet(
                                stream,
                                packet.opcode,
                                &packet.body,
                                Some(&mut *header_crypto),
                            )
                            .await?;
                        }
                        deps.shared_world
                            .sessions
                            .dispatch(event.observer_packets)
                            .await;
                    }
                    if !suppress_hostile_refs {
                        begin_db_creature_retaliation_if_needed(
                            stream,
                            deps.shared_world,
                            map_id,
                            session,
                            target,
                            caster,
                            header_crypto,
                        )
                        .await?;
                    }
                }
            }
        }
        SpellAuraTarget::CasterAreaEnemy => {
            let spell_info = SpellInfo::from_template(spell_template);
            let Some(effect) = spell_info.effects.into_iter().find(|effect| {
                effect.dispatch == SpellEffectDispatch::ApplyAura
                    && effect_targets_caster_centered_hostile_area(*effect)
            }) else {
                return Ok(());
            };
            let Some(radius) = spell_effect_radius_yards(deps.shared_world.maps, effect) else {
                warn!(
                    spell_id = spell_template.id,
                    radius_index = effect.radius_index,
                    "Skipping caster-centered AoE aura with missing SpellRadius.dbc row"
                );
                return Ok(());
            };
            let targets = deps
                .shared_world
                .maps
                .nearby_attackable_db_creature_guids_for_player_spell(
                    map_id,
                    character_guid,
                    radius,
                )
                .await;
            for target in targets {
                let Some(target_creature) = deps
                    .shared_world
                    .maps
                    .db_creature_snapshot(map_id, target)
                    .await
                else {
                    continue;
                };
                let resolution = aura_rank_conflict_resolution(
                    deps.shared_world.object_mgr,
                    deps.world_db_pool,
                    spell_template.id,
                    caster,
                    &target_creature.active_auras,
                )
                .await?;
                if resolution.failure.is_some() {
                    continue;
                }
                if let Some(event) = deps
                    .shared_world
                    .maps
                    .apply_db_creature_aura_replacing_conflicts(
                        map_id,
                        target,
                        character_guid,
                        aura.clone(),
                        &resolution,
                        None,
                        None,
                        now,
                    )
                    .await?
                {
                    send_packet(
                        stream,
                        SMSG_UPDATE_OBJECT,
                        &event.update_body,
                        Some(&mut *header_crypto),
                    )
                    .await?;
                    for packet in event.direct_packets {
                        send_packet(
                            stream,
                            packet.opcode,
                            &packet.body,
                            Some(&mut *header_crypto),
                        )
                        .await?;
                    }
                    deps.shared_world
                        .sessions
                        .dispatch(event.observer_packets)
                        .await;
                }
                if !suppress_hostile_refs {
                    begin_db_creature_retaliation_if_needed(
                        stream,
                        deps.shared_world,
                        map_id,
                        session,
                        target,
                        caster,
                        header_crypto,
                    )
                    .await?;
                }
            }
        }
        SpellAuraTarget::DestinationAreaEnemy => {
            let spell_info = SpellInfo::from_template(spell_template);
            let Some(effect) = spell_info.effects.into_iter().find(|effect| {
                effect.dispatch == SpellEffectDispatch::ApplyAura
                    && effect_targets_destination_hostile_area(*effect)
            }) else {
                return Ok(());
            };
            let Some(radius) = spell_effect_radius_yards(deps.shared_world.maps, effect) else {
                warn!(
                    spell_id = spell_template.id,
                    radius_index = effect.radius_index,
                    "Skipping destination AoE aura with missing SpellRadius.dbc row"
                );
                return Ok(());
            };
            let Some(destination) = spell_target_destination_position(map_id, targets) else {
                warn!(
                    spell_id = spell_template.id,
                    "Skipping destination AoE aura with missing target destination"
                );
                return Ok(());
            };
            let targets = deps
                .shared_world
                .maps
                .nearby_attackable_db_creature_guids_for_player_spell_at_position(
                    map_id,
                    character_guid,
                    destination,
                    radius,
                )
                .await;
            for target in targets {
                let Some(target_creature) = deps
                    .shared_world
                    .maps
                    .db_creature_snapshot(map_id, target)
                    .await
                else {
                    continue;
                };
                let resolution = aura_rank_conflict_resolution(
                    deps.shared_world.object_mgr,
                    deps.world_db_pool,
                    spell_template.id,
                    caster,
                    &target_creature.active_auras,
                )
                .await?;
                if resolution.failure.is_some() {
                    continue;
                }
                if let Some(event) = deps
                    .shared_world
                    .maps
                    .apply_db_creature_aura_replacing_conflicts(
                        map_id,
                        target,
                        character_guid,
                        aura.clone(),
                        &resolution,
                        None,
                        None,
                        now,
                    )
                    .await?
                {
                    send_packet(
                        stream,
                        SMSG_UPDATE_OBJECT,
                        &event.update_body,
                        Some(&mut *header_crypto),
                    )
                    .await?;
                    for packet in event.direct_packets {
                        send_packet(
                            stream,
                            packet.opcode,
                            &packet.body,
                            Some(&mut *header_crypto),
                        )
                        .await?;
                    }
                    deps.shared_world
                        .sessions
                        .dispatch(event.observer_packets)
                        .await;
                }
                if !suppress_hostile_refs {
                    begin_db_creature_retaliation_if_needed(
                        stream,
                        deps.shared_world,
                        map_id,
                        session,
                        target,
                        caster,
                        header_crypto,
                    )
                    .await?;
                }
            }
        }
    }
    Ok(())
}

async fn augment_mage_polymorph_regen_from_helper_spell(
    object_mgr: &ObjectMgr,
    world_db_pool: &MySqlPool,
    spell_template: &wow_db::SpellTemplateQuery,
    aura: &mut ActiveAura,
    now: Instant,
    target_max_health: u32,
) -> anyhow::Result<()> {
    if !spell_is_mage_polymorph(spell_template) {
        return Ok(());
    }
    let Some(helper_template) = object_mgr
        .spell_template(world_db_pool, POLYMORPH_HELPER_REGEN_SPELL_ID)
        .await?
    else {
        warn!(
            spell_id = spell_template.id,
            helper_spell_id = POLYMORPH_HELPER_REGEN_SPELL_ID,
            "Mage polymorph helper regen spell_template row is missing"
        );
        return Ok(());
    };
    let helper_context = SpellEffectValueContext::unranked(&helper_template, 0);
    let Some(mut regen) = spell_periodic_regen_aura(
        &SpellInfo::from_template(&helper_template),
        helper_context,
        now,
    ) else {
        warn!(
            spell_id = spell_template.id,
            helper_spell_id = POLYMORPH_HELPER_REGEN_SPELL_ID,
            "Mage polymorph helper regen spell has no periodic regen aura payload"
        );
        return Ok(());
    };
    regen.health_amount = (target_max_health / 10).max(1);
    aura.periodic_regen = Some(regen);
    Ok(())
}

pub(in crate::world) fn spell_effect_radius_yards(
    maps: &MapRuntimeManager,
    effect: SpellInfoEffect,
) -> Option<f32> {
    maps.spell_radius(effect.radius_index)
        .map(|entry| entry.radius)
        .filter(|radius| *radius > 0.0)
}

pub(in crate::world) fn spell_direct_heal(
    spell_info: &SpellInfo<'_>,
    value_context: SpellEffectValueContext,
) -> u32 {
    spell_info
        .effects
        .into_iter()
        .filter(|effect| effect.dispatch == SpellEffectDispatch::Heal)
        .filter_map(|effect| spell_effect_calculated_u32(effect, value_context))
        .sum()
}

pub(in crate::world) fn spell_direct_energize(
    spell_info: &SpellInfo<'_>,
    value_context: SpellEffectValueContext,
) -> u32 {
    spell_info
        .effects
        .into_iter()
        .filter(|effect| effect.dispatch == SpellEffectDispatch::Energize)
        .filter_map(|effect| spell_effect_calculated_u32(effect, value_context))
        .sum()
}

#[allow(clippy::too_many_arguments)]
pub(in crate::world) async fn apply_player_persistent_area_aura_effect(
    stream: &mut WorldPacketSink,
    deps: SpellCastDeps<'_>,
    _session: &mut WorldSessionState,
    caster: ObjectGuid,
    character_guid: u32,
    character_level: u8,
    map_id: u32,
    spell_template: &wow_db::SpellTemplateQuery,
    effect_index: usize,
    effect: SpellInfoEffect,
    value_context: SpellEffectValueContext,
    targets: &SpellCastTargets,
    now: Instant,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    let Some(destination) = spell_target_destination_position(map_id, targets) else {
        warn!(
            spell_id = spell_template.id,
            "Skipping persistent area aura with missing target destination"
        );
        return Ok(());
    };
    let Some(radius) = spell_effect_radius_yards(deps.shared_world.maps, effect) else {
        warn!(
            spell_id = spell_template.id,
            radius_index = effect.radius_index,
            "Skipping persistent area aura with missing SpellRadius.dbc row"
        );
        return Ok(());
    };
    let Some(duration) = deps
        .shared_world
        .maps
        .spell_duration(spell_template.duration_index)
        .map(|duration| duration.duration_millis)
        .filter(|duration| *duration > 0)
    else {
        warn!(
            spell_id = spell_template.id,
            duration_index = spell_template.duration_index,
            "Skipping persistent area aura with missing positive SpellDuration.dbc row"
        );
        return Ok(());
    };
    let periodic_damage = persistent_area_periodic_damage(
        spell_template,
        effect,
        character_level,
        value_context,
        now,
    );
    let channeled = (spell_template.attributes_ex
        & (SPELL_ATTR_EX_IS_CHANNELED | SPELL_ATTR_EX_IS_SELF_CHANNELED))
        != 0;
    let Some(event) = deps
        .shared_world
        .maps
        .create_persistent_area_dynamic_object(
            map_id,
            caster,
            character_guid,
            spell_template.id,
            effect_index,
            destination,
            radius,
            duration as u32,
            periodic_damage,
            channeled,
            spell_template.channel_interrupt_flags,
            now,
        )
        .await?
    else {
        return Ok(());
    };
    for packet in event.direct_packets {
        send_packet(
            stream,
            packet.opcode,
            &packet.body,
            Some(&mut *header_crypto),
        )
        .await?;
    }
    deps.shared_world
        .sessions
        .dispatch(event.observer_packets)
        .await;
    Ok(())
}

pub(in crate::world) fn persistent_area_periodic_damage(
    spell_template: &wow_db::SpellTemplateQuery,
    effect: SpellInfoEffect,
    caster_level: u8,
    value_context: SpellEffectValueContext,
    now: Instant,
) -> Option<PeriodicDamageAura> {
    if effect.aura_name != SPELL_AURA_PERIODIC_DAMAGE || effect.amplitude == 0 {
        return None;
    }
    let damage = spell_effect_calculated_u32(effect, value_context)?;
    Some(PeriodicDamageAura {
        aura_name: effect.aura_name,
        school: spell_template.school,
        damage_class: spell_template.dmg_class,
        attributes_ex2: spell_template.attributes_ex2,
        attributes_ex3: spell_template.attributes_ex3,
        caster_snapshot: spell_periodic_damage_fallback_caster_snapshot(caster_level),
        amount: damage,
        tick_millis: effect.amplitude,
        next_tick_at: now + Duration::from_millis(effect.amplitude as u64),
    })
}

#[allow(clippy::too_many_arguments)]
pub(in crate::world) async fn apply_player_periodic_trigger_channel_effect(
    stream: &mut WorldPacketSink,
    deps: SpellCastDeps<'_>,
    caster: ObjectGuid,
    character_guid: u32,
    character_level: u8,
    map_id: u32,
    spell_template: &wow_db::SpellTemplateQuery,
    effect: SpellInfoEffect,
    _value_context: SpellEffectValueContext,
    targets: &SpellCastTargets,
    now: Instant,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    let Some(target) = targets.unit_target else {
        warn!(
            spell_id = spell_template.id,
            "Skipping periodic trigger channel with missing unit target"
        );
        return Ok(());
    };
    if effect.trigger_spell == 0 || effect.amplitude == 0 {
        warn!(
            spell_id = spell_template.id,
            effect_trigger_spell = effect.trigger_spell,
            effect_amplitude = effect.amplitude,
            "Skipping periodic trigger channel with incomplete trigger data"
        );
        return Ok(());
    }
    let Some(duration) = deps
        .shared_world
        .maps
        .spell_duration(spell_template.duration_index)
        .map(|duration| duration.duration_millis)
        .filter(|duration| *duration > 0)
    else {
        warn!(
            spell_id = spell_template.id,
            duration_index = spell_template.duration_index,
            "Skipping periodic trigger channel with missing positive SpellDuration.dbc row"
        );
        return Ok(());
    };
    let Some(triggered_template) = deps
        .shared_world
        .object_mgr
        .spell_template(deps.world_db_pool, effect.trigger_spell)
        .await?
    else {
        warn!(
            spell_id = spell_template.id,
            triggered_spell_id = effect.trigger_spell,
            "Skipping periodic trigger channel with missing triggered spell_template row"
        );
        return Ok(());
    };
    let triggered_info = SpellInfo::from_template(&triggered_template);
    let Some(triggered_profile) = triggered_info.player_cast_profile() else {
        warn!(
            spell_id = spell_template.id,
            triggered_spell_id = effect.trigger_spell,
            "Skipping periodic trigger channel with unsupported triggered spell shape"
        );
        return Ok(());
    };
    let triggered_value_context = SpellEffectValueContext::with_spell_rank_level(
        &triggered_template,
        character_level as i32,
        0,
    );
    let Some(damage_effect) = triggered_info
        .effects
        .into_iter()
        .find_map(|triggered_effect| {
            player_direct_damage_effect(
                &triggered_template,
                &triggered_profile,
                triggered_effect,
                triggered_value_context,
            )
        })
    else {
        warn!(
            spell_id = spell_template.id,
            triggered_spell_id = effect.trigger_spell,
            "Skipping periodic trigger channel whose triggered spell has no direct damage effect"
        );
        return Ok(());
    };
    let Some(event) = deps
        .shared_world
        .maps
        .start_player_periodic_trigger_channel(
            map_id,
            caster,
            character_guid,
            spell_template.id,
            target,
            duration as u32,
            effect.amplitude,
            damage_effect,
            spell_template.channel_interrupt_flags,
            triggered_template.speed,
            now,
        )
        .await?
    else {
        return Ok(());
    };
    for packet in event.direct_packets {
        send_packet(
            stream,
            packet.opcode,
            &packet.body,
            Some(&mut *header_crypto),
        )
        .await?;
    }
    deps.shared_world
        .sessions
        .dispatch(event.observer_packets)
        .await;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub(in crate::world) async fn apply_item_use_spell_effects(
    stream: &mut WorldPacketSink,
    deps: SpellCastDeps<'_>,
    session: &mut WorldSessionState,
    caster: ObjectGuid,
    spell_template: &wow_db::SpellTemplateQuery,
    item_spell: &SpellCastProfile,
    now: Instant,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    let Some(character) = session.character.active_character.as_ref() else {
        return Ok(());
    };
    let map_id = character.position.map_id;
    let character_guid = character.guid;
    let character_level = character.level;
    let character_snapshot = character.clone();
    let mut update_bodies = Vec::new();
    let spell_info = SpellInfo::from_template(spell_template);

    if spell_info
        .effects
        .iter()
        .any(|effect| effect.dispatch == SpellEffectDispatch::Teleport)
        && item_spell.kind == SpellCastKind::Teleport
    {
        for effect in spell_info.effects {
            if effect.dispatch == SpellEffectDispatch::Teleport {
                return apply_item_teleport_spell_effect(
                    stream,
                    deps,
                    session,
                    character_guid,
                    map_id,
                    header_crypto,
                )
                .await;
            }
        }
    }

    let world_stats = wow_db::get_player_world_stats(
        deps.world_db_pool,
        character.race,
        character.class,
        character.level,
    )
    .await?;
    let effective_world_stats =
        player_world_stats_with_active_auras(world_stats, &session.auras.active_auras);
    let max_health = effective_world_stats.max_health().max(1);
    let max_mana = effective_world_stats.max_mana();

    let mut direct_heal_applied = false;
    let mut direct_energize_applied = false;
    let mut aura_applied = false;
    let value_context = player_spell_effect_value_context(
        deps.shared_world.maps,
        spell_template,
        &session.character.character_skills,
        0,
    );
    for effect in spell_info.effects {
        match effect.dispatch {
            SpellEffectDispatch::Heal if !direct_heal_applied => {
                let heal = spell_direct_heal(&spell_info, value_context);
                if heal != 0 {
                    let old_health = session.character.player_health;
                    session.character.player_health = session
                        .character
                        .player_health
                        .saturating_add(heal)
                        .min(max_health);
                    let amount_healed = session.character.player_health.saturating_sub(old_health);
                    if amount_healed > 0 {
                        let log = build_spell_heal_log_body(
                            caster,
                            caster,
                            spell_template.id,
                            amount_healed,
                            false,
                        )?;
                        send_packet(stream, SMSG_SPELLHEALLOG, &log, Some(&mut *header_crypto))
                            .await?;
                    }
                    update_bodies.push(build_player_health_update_body(
                        caster,
                        session.character.player_health,
                    )?);
                }
                direct_heal_applied = true;
            }
            SpellEffectDispatch::Energize if !direct_energize_applied => {
                let energize = spell_direct_energize(&spell_info, value_context);
                if energize != 0 && max_mana != 0 {
                    let old_mana = session.character.player_mana;
                    session.character.player_mana = session
                        .character
                        .player_mana
                        .saturating_add(energize)
                        .min(max_mana);
                    let amount_energized = session.character.player_mana.saturating_sub(old_mana);
                    if amount_energized > 0 {
                        let log = build_spell_energize_log_body(
                            caster,
                            caster,
                            spell_template.id,
                            POWER_TYPE_MANA,
                            amount_energized,
                        )?;
                        send_packet(
                            stream,
                            SMSG_SPELLENERGIZELOG,
                            &log,
                            Some(&mut *header_crypto),
                        )
                        .await?;
                    }
                    update_bodies.push(build_player_mana_update_body(
                        caster,
                        session.character.player_mana,
                    )?);
                }
                direct_energize_applied = true;
            }
            SpellEffectDispatch::ApplyAura
                if item_spell.kind == SpellCastKind::AuraApplication && !aura_applied =>
            {
                apply_item_aura_effect(
                    stream,
                    deps,
                    session,
                    caster,
                    character_guid,
                    character_level,
                    map_id,
                    spell_template,
                    &character_snapshot,
                    value_context,
                    now,
                    &mut update_bodies,
                    header_crypto,
                )
                .await?;
                aura_applied = true;
            }
            _ => {}
        }
    }

    for body in update_bodies {
        send_packet(stream, SMSG_UPDATE_OBJECT, &body, Some(&mut *header_crypto)).await?;
    }
    deps.shared_world
        .maps
        .sync_player_gameplay_state(map_id, character_guid, session)
        .await;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub(in crate::world) async fn apply_item_aura_effect(
    stream: &mut WorldPacketSink,
    deps: SpellCastDeps<'_>,
    session: &mut WorldSessionState,
    caster: ObjectGuid,
    character_guid: u32,
    character_level: u8,
    map_id: u32,
    spell_template: &wow_db::SpellTemplateQuery,
    character_snapshot: &Player,
    value_context: SpellEffectValueContext,
    now: Instant,
    update_bodies: &mut Vec<Vec<u8>>,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    let aura = build_active_aura(
        spell_template,
        caster,
        character_level,
        value_context,
        now,
        deps.shared_world
            .maps
            .spell_duration(spell_template.duration_index),
    );
    let mut aura = aura;
    mark_active_aura_periodic_regen_as_consumable(&mut aura);
    let makes_player_sit = aura
        .periodic_regen
        .is_some_and(|regen| regen.makes_player_sit);
    apply_player_aura(session, aura.clone());
    if makes_player_sit {
        session.character.player_stand_state = PLAYER_STAND_STATE_SIT;
        update_bodies.push(build_player_stand_state_update_body(
            character_snapshot,
            session.character.player_stand_state,
        )?);
    }
    if let Some(event) = deps
        .shared_world
        .maps
        .apply_player_aura(map_id, character_guid, aura)
        .await?
    {
        for packet in event.direct_packets {
            send_packet(
                stream,
                packet.opcode,
                &packet.body,
                Some(&mut *header_crypto),
            )
            .await?;
        }
        deps.shared_world
            .sessions
            .dispatch(event.observer_packets)
            .await;
    } else {
        update_bodies.push(build_player_aura_update_body(
            caster,
            &session.auras.active_auras,
        )?);
        for packet in build_player_aura_duration_update_packets(&session.auras.active_auras, now) {
            send_packet(
                stream,
                packet.opcode,
                &packet.body,
                Some(&mut *header_crypto),
            )
            .await?;
        }
    }
    if makes_player_sit {
        let observer_packets = deps
            .shared_world
            .maps
            .broadcast_nearby_player_packet(
                map_id,
                character_guid,
                PLAYER_VISIBILITY_RADIUS_YARDS,
                OutboundWorldPacket {
                    opcode: SMSG_UPDATE_OBJECT,
                    body: build_player_stand_state_update_body(
                        character_snapshot,
                        session.character.player_stand_state,
                    )?,
                },
            )
            .await;
        deps.shared_world.sessions.dispatch(observer_packets).await;
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub(in crate::world) async fn apply_player_near_teleport_effect(
    stream: &mut WorldPacketSink,
    deps: SpellCastDeps<'_>,
    session: &mut WorldSessionState,
    character_guid: u32,
    map_id: u32,
    spell_template: &wow_db::SpellTemplateQuery,
    effect: SpellInfoEffect,
    targets: &SpellCastTargets,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    let Some(destination) = spell_target_destination_position(map_id, targets).or_else(|| {
        player_near_teleport_forward_destination(
            deps.shared_world.maps,
            session,
            spell_template,
            effect,
            map_id,
        )
    }) else {
        warn!(
            character_guid,
            "Skipping near teleport spell with missing destination"
        );
        return Ok(());
    };
    let position = {
        let Some(character) = session.character.active_character.as_mut() else {
            return Ok(());
        };
        character.position = WorldPosition::new(
            destination.map_id,
            destination.x,
            destination.y,
            destination.z,
            character.position.orientation,
        );
        character.movement_flags = 0;
        character.fall_time = 0;
        character.position
    };
    let old_map_id = map_id;
    deps.shared_world
        .maps
        .set_player_position(old_map_id, character_guid, position)
        .await;
    deps.shared_world
        .maps
        .reset_player_visibility_scan_positions(old_map_id, character_guid)
        .await;
    deps.shared_world
        .maps
        .sync_player_gameplay_state(old_map_id, character_guid, session)
        .await;
    wow_db::update_character_position(
        deps.character_db_pool,
        deps.account_id,
        character_guid,
        position,
    )
    .await?;
    send_packet(
        stream,
        MSG_MOVE_TELEPORT_ACK,
        &build_near_teleport_ack_body(session.character.active_character.as_ref().unwrap(), 0)?,
        Some(&mut *header_crypto),
    )
    .await?;
    stream_newly_visible_db_creatures(
        stream,
        deps.character_db_pool,
        deps.world_db_pool,
        deps.shared_world.maps,
        session,
        header_crypto,
    )
    .await?;
    Ok(())
}

pub(in crate::world) fn player_near_teleport_forward_destination(
    maps: &MapRuntimeManager,
    session: &WorldSessionState,
    spell_template: &wow_db::SpellTemplateQuery,
    effect: SpellInfoEffect,
    map_id: u32,
) -> Option<WorldPosition> {
    if effect.dispatch != SpellEffectDispatch::Leap {
        return None;
    }
    if effect.implicit_target_a != TARGET_LOCATION_CASTER_FRONT_LEAP
        && effect.implicit_target_b != TARGET_LOCATION_CASTER_FRONT_LEAP
    {
        return None;
    }
    let character = session.character.active_character.as_ref()?;
    let distance = maps
        .spell_radius(effect.radius_index)
        .map(|radius| radius.radius)
        .or_else(|| {
            maps.spell_range(spell_template.range_index)
                .map(|range| range.max_range)
        })?;
    if distance <= 0.0 || !distance.is_finite() {
        return None;
    }
    let orientation = character.position.orientation;
    let destination = WorldPosition::new(
        map_id,
        character.position.x + orientation.cos() * distance,
        character.position.y + orientation.sin() * distance,
        character.position.z,
        orientation,
    );
    Some(
        maps.geometry
            .ground_position(destination)
            .unwrap_or(destination),
    )
}

pub(in crate::world) async fn apply_item_teleport_spell_effect(
    stream: &mut WorldPacketSink,
    deps: SpellCastDeps<'_>,
    session: &mut WorldSessionState,
    character_guid: u32,
    old_map_id: u32,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    let Some(homebind) =
        wow_db::get_character_homebind(deps.character_db_pool, character_guid).await?
    else {
        warn!(
            character_guid,
            "Ignoring teleport item spell without character_homebind row"
        );
        return Ok(());
    };
    let Some(character) = session.character.active_character.as_mut() else {
        return Ok(());
    };

    character.position = homebind;
    character.movement_flags = 0;
    character.fall_time = 0;
    deps.shared_world
        .maps
        .set_player_position(old_map_id, character_guid, homebind)
        .await;
    deps.shared_world
        .maps
        .reset_player_visibility_scan_positions(old_map_id, character_guid)
        .await;
    deps.shared_world
        .maps
        .sync_player_gameplay_state(old_map_id, character_guid, session)
        .await;
    wow_db::update_character_position(
        deps.character_db_pool,
        deps.account_id,
        character_guid,
        homebind,
    )
    .await?;

    send_packet(
        stream,
        MSG_MOVE_TELEPORT_ACK,
        &build_near_teleport_ack_body(session.character.active_character.as_ref().unwrap(), 0)?,
        Some(&mut *header_crypto),
    )
    .await?;
    stream_newly_visible_db_creatures(
        stream,
        deps.character_db_pool,
        deps.world_db_pool,
        deps.shared_world.maps,
        session,
        header_crypto,
    )
    .await?;
    Ok(())
}
