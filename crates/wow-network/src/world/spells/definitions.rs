use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::world) struct SpellCastProfile {
    pub(in crate::world) spell_id: u32,
    pub(in crate::world) kind: SpellCastKind,
    pub(in crate::world) aura_target: SpellAuraTarget,
    pub(in crate::world) bonus_damage: u32,
    pub(in crate::world) weapon_damage_percent: u32,
    pub(in crate::world) damage: u32,
    pub(in crate::world) power: SpellPowerCost,
    pub(in crate::world) requires_melee: bool,
    pub(in crate::world) requires_behind: bool,
    pub(in crate::world) needs_combo_points: bool,
    pub(in crate::world) global_cooldown_category: u32,
    pub(in crate::world) global_cooldown_millis: u64,
    pub(in crate::world) cooldown_category: u32,
    pub(in crate::world) category_cooldown_millis: u64,
    pub(in crate::world) cooldown_millis: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::world) struct PlayerSpellTargetOutcome {
    pub(in crate::world) target: ObjectGuid,
    pub(in crate::world) miss_info: Option<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::world) struct SpellPlan {
    pub(in crate::world) spell_id: u32,
    pub(in crate::world) profile: SpellCastProfile,
    pub(in crate::world) target: SpellPlanTarget,
    pub(in crate::world) channel: Option<SpellPlanChannel>,
    pub(in crate::world) effects: Vec<SpellPlanEffect>,
    pub(in crate::world) behavior: SpellPlanBehavior,
    pub(in crate::world) flags: Vec<SpellPlanFlag>,
}

impl SpellPlan {
    pub(in crate::world) fn effect_target(&self, index: usize) -> SpellPlanEffectTarget {
        self.effects
            .iter()
            .find(|effect| effect.index == index)
            .map(|effect| effect.target)
            .unwrap_or(SpellPlanEffectTarget::None)
    }

    pub(in crate::world) fn has_hostile_direct_effect(&self) -> bool {
        self.effects.iter().any(|effect| {
            matches!(
                effect.dispatch,
                SpellEffectDispatch::SchoolDamage
                    | SpellEffectDispatch::WeaponDamage
                    | SpellEffectDispatch::WeaponPercentDamage
            ) && effect.target.is_hostile()
        })
    }

    pub(in crate::world) fn has_hostile_unit_damage(&self) -> bool {
        self.effects.iter().any(|effect| {
            effect.dispatch == SpellEffectDispatch::SchoolDamage
                && effect.target == SpellPlanEffectTarget::HostileUnit
        })
    }

    pub(in crate::world) fn has_hostile_unit_aura(&self) -> bool {
        self.effects.iter().any(|effect| {
            effect.dispatch == SpellEffectDispatch::ApplyAura
                && effect.target == SpellPlanEffectTarget::HostileUnit
        })
    }

    pub(in crate::world) fn has_hostile_unit_interrupt(&self) -> bool {
        self.effects.iter().any(|effect| {
            effect.dispatch == SpellEffectDispatch::InterruptCast
                && effect.target == SpellPlanEffectTarget::HostileUnit
        })
    }

    pub(in crate::world) fn uses_db_creature_unit_target_outcome(&self) -> bool {
        matches!(
            self.profile.kind,
            SpellCastKind::InstantDamage
                | SpellCastKind::AuraApplication
                | SpellCastKind::Interrupt
        ) && (self.has_hostile_unit_damage()
            || self.has_hostile_unit_aura()
            || self.has_hostile_unit_interrupt())
    }

    pub(in crate::world) fn should_retaliate_on_failed_hostile_cast(&self) -> bool {
        self.has_hostile_unit_damage()
            || self.has_hostile_unit_aura()
            || self.has_hostile_unit_interrupt()
    }

    #[allow(dead_code)]
    pub(in crate::world) fn unsupported_flags(&self) -> Vec<SpellPlanFlag> {
        self.flags
            .iter()
            .copied()
            .filter(|flag| flag.support.blocks_runtime())
            .collect()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::world) struct SpellPlanBehavior {
    pub(in crate::world) resets_auto_attack_timers: bool,
    pub(in crate::world) cancels_auto_repeat_when_casting: bool,
    pub(in crate::world) blocks_mana_regen: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::world) struct SpellPlanFlag {
    pub(in crate::world) field: SpellPlanFlagField,
    pub(in crate::world) bit: u32,
    pub(in crate::world) name: Option<&'static str>,
    pub(in crate::world) support: SpellPlanFlagSupport,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::world) enum SpellPlanFlagField {
    Attributes,
    AttributesEx,
    AttributesEx2,
    AttributesEx3,
    AttributesServerSide,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::world) enum SpellPlanFlagSupport {
    ImplementedGeneric(&'static str),
    ExecutionPayload(&'static str),
    PendingGeneric(&'static str),
    ScriptRequired(&'static str),
    KnownNoOp(&'static str),
    Unknown,
}

impl SpellPlanFlagSupport {
    #[allow(dead_code)]
    pub(in crate::world) fn blocks_runtime(self) -> bool {
        matches!(
            self,
            SpellPlanFlagSupport::PendingGeneric(_)
                | SpellPlanFlagSupport::ScriptRequired(_)
                | SpellPlanFlagSupport::Unknown
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::world) enum SpellPlanTarget {
    Caster,
    Unit,
    HostileUnit,
    FriendlyUnit,
    Destination,
    CasterAreaEnemy { cone: bool },
    DestinationAreaEnemy,
}

impl SpellPlanTarget {
    pub(in crate::world) fn target_kind(self) -> SpellTargetKind {
        match self {
            SpellPlanTarget::Caster | SpellPlanTarget::CasterAreaEnemy { .. } => {
                SpellTargetKind::Caster
            }
            SpellPlanTarget::Unit => SpellTargetKind::Unit,
            SpellPlanTarget::HostileUnit => SpellTargetKind::HostileUnit,
            SpellPlanTarget::FriendlyUnit => SpellTargetKind::FriendlyUnit,
            SpellPlanTarget::Destination | SpellPlanTarget::DestinationAreaEnemy => {
                SpellTargetKind::Destination
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::world) enum SpellPlanChannel {
    SelfAura {
        duration_index: u32,
        interrupt_flags: u32,
    },
    UnitPeriodicTrigger {
        trigger_spell: u32,
        tick_millis: u32,
        duration_index: u32,
        interrupt_flags: u32,
    },
    PersistentArea {
        duration_index: u32,
        interrupt_flags: u32,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::world) struct SpellPlanEffect {
    pub(in crate::world) index: usize,
    pub(in crate::world) dispatch: SpellEffectDispatch,
    pub(in crate::world) aura_name: u32,
    pub(in crate::world) target: SpellPlanEffectTarget,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::world) enum SpellPlanEffectTarget {
    None,
    Caster,
    Unit,
    HostileUnit,
    FriendlyUnit,
    CasterAreaEnemy { cone: bool },
    DestinationAreaEnemy,
    CasterFrontLeap,
}

impl SpellPlanEffectTarget {
    pub(in crate::world) fn is_hostile(self) -> bool {
        matches!(
            self,
            SpellPlanEffectTarget::HostileUnit
                | SpellPlanEffectTarget::CasterAreaEnemy { .. }
                | SpellPlanEffectTarget::DestinationAreaEnemy
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::world) struct DbCreatureSpellPlan {
    pub(in crate::world) spell_id: u32,
    pub(in crate::world) requires_behind: bool,
    pub(in crate::world) mana_cost: u32,
    pub(in crate::world) aura: bool,
    pub(in crate::world) effect: DbCreatureSpellPlanEffect,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::world) enum DbCreatureSpellPlanEffect {
    Damage {
        amount: u32,
        school: u8,
        dmg_class: u32,
        attributes_ex2: u32,
        attributes_ex3: u32,
    },
    Heal {
        amount: u32,
    },
    AuraOnly,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::world) enum SpellCastKind {
    InstantDamage,
    DirectHeal,
    AuraApplication,
    CreateItem,
    OpeningGameObject,
    AutoRepeatRanged,
    Charge,
    NextMeleeSwing,
    Teleport,
    Interrupt,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::world) enum SpellAuraTarget {
    Caster,
    UnitTarget,
    CasterAreaEnemy,
    DestinationAreaEnemy,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::world) enum SpellTargetKind {
    Caster,
    Unit,
    HostileUnit,
    FriendlyUnit,
    Destination,
}

impl SpellTargetKind {
    pub(in crate::world) fn requires_unit_target(self) -> bool {
        matches!(
            self,
            SpellTargetKind::Unit | SpellTargetKind::HostileUnit | SpellTargetKind::FriendlyUnit
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::world) enum SpellPowerCost {
    Rage { cost: u32 },
    Mana { cost: u32 },
    Energy { cost: u32 },
}

pub(in crate::world) const SPELL_ATTR_ON_NEXT_SWING_NO_DAMAGE: u32 = 0x0000_0004;
pub(in crate::world) const SPELL_ATTR_USES_RANGED_SLOT: u32 = 0x0000_0002;
pub(in crate::world) const SPELL_ATTR_PASSIVE: u32 = 0x0000_0040;
pub(in crate::world) const SPELL_ATTR_ON_NEXT_SWING: u32 = 0x0000_0400;
pub(in crate::world) const SPELL_INTERRUPT_FLAG_MOVEMENT: u32 = 0x01;
pub(in crate::world) const SPELL_INTERRUPT_FLAG_DAMAGE_PUSHBACK: u32 = 0x02;
pub(in crate::world) const SPELL_INTERRUPT_FLAG_DAMAGE_CANCELS: u32 = 0x10;
pub(in crate::world) const SPELL_ATTR_EX_IS_CHANNELED: u32 = 0x0000_0004;
pub(in crate::world) const SPELL_ATTR_EX_IS_SELF_CHANNELED: u32 = 0x0000_0040;
pub(in crate::world) const SPELL_ATTR_EX_IMMUNITY_PURGES_EFFECT: u32 = 0x0000_8000;
pub(in crate::world) const SPELL_ATTR_EX_NO_AUTOCAST_AI: u32 = 0x0002_0000;
pub(in crate::world) const SPELL_ATTR_EX_FINISHING_MOVE_DAMAGE: u32 = 0x0010_0000;
pub(in crate::world) const SPELL_ATTR_EX_FINISHING_MOVE_DURATION: u32 = 0x0040_0000;
pub(in crate::world) const SPELL_ATTR_EX3_REQUIRES_MAIN_HAND_WEAPON: u32 = 0x0000_0400;
pub(in crate::world) const SPELL_ATTR_EX3_REQUIRES_OFFHAND_WEAPON: u32 = 0x0100_0000;
pub(in crate::world) const SPELL_EFFECT_SCHOOL_DAMAGE: u32 = 2;
pub(in crate::world) const SPELL_EFFECT_PERSISTENT_AREA_AURA: u32 = 27;
pub(in crate::world) const SPELL_EFFECT_TRIGGER_MISSILE: u32 = 32;
pub(in crate::world) const SPELL_EFFECT_WEAPON_DAMAGE_NOSCHOOL: u32 = 17;
pub(in crate::world) const SPELL_EFFECT_CREATE_ITEM: u32 = 24;
pub(in crate::world) const SPELL_EFFECT_LEAP: u32 = 29;
pub(in crate::world) const SPELL_EFFECT_WEAPON_PERCENT_DAMAGE: u32 = 31;
pub(in crate::world) const SPELL_EFFECT_WEAPON_DAMAGE: u32 = 58;
pub(in crate::world) const SPELL_EFFECT_ADD_COMBO_POINTS: u32 = 80;
pub(in crate::world) const SPELL_EFFECT_NORMALIZED_WEAPON_DMG: u32 = 121;
pub(in crate::world) const SPELL_EFFECT_APPLY_AURA: u32 = 6;
pub(in crate::world) const SPELL_EFFECT_TELEPORT_UNITS: u32 = 5;
pub(in crate::world) const SPELL_EFFECT_TELEPORT_UNITS_FACE_CASTER: u32 = 43;
pub(in crate::world) const SPELL_EFFECT_DISPEL: u32 = 38;
pub(in crate::world) const SPELL_EFFECT_DISPEL_MECHANIC: u32 = 108;
pub(in crate::world) const SPELL_EFFECT_INTERRUPT_CAST: u32 = 68;
pub(in crate::world) const SPELL_EFFECT_HEAL: u32 = 10;
pub(in crate::world) const SPELL_EFFECT_ENERGIZE: u32 = 30;
pub(in crate::world) const SPELL_EFFECT_CHARGE: u32 = 96;
pub(in crate::world) const SPELL_EFFECT_ATTACK_ME: u32 = 114;
pub(in crate::world) const SPELL_EFFECT_DUEL: u32 = 83;
pub(in crate::world) const SPELL_EFFECT_STUCK: u32 = 84;
pub(in crate::world) const SPELL_EFFECT_SKIN_PLAYER_CORPSE: u32 = 116;
pub(in crate::world) const SPELL_AURA_PERIODIC_DAMAGE: u32 = 3;
pub(in crate::world) const SPELL_AURA_DUMMY: u32 = 4;
pub(in crate::world) const SPELL_AURA_MOD_CONFUSE: u32 = 5;
pub(in crate::world) const SPELL_AURA_MOD_FEAR: u32 = 7;
pub(in crate::world) const SPELL_AURA_PERIODIC_HEAL: u32 = 8;
pub(in crate::world) const SPELL_AURA_MOD_THREAT: u32 = 10;
pub(in crate::world) const SPELL_AURA_MOD_TAUNT: u32 = 11;
pub(in crate::world) const SPELL_AURA_MOD_STUN: u32 = 12;
pub(in crate::world) const SPELL_AURA_MOD_DAMAGE_DONE: u32 = 13;
pub(in crate::world) const SPELL_AURA_MOD_DAMAGE_TAKEN: u32 = 14;
pub(in crate::world) const SPELL_AURA_MOD_STEALTH_DETECT: u32 = 17;
pub(in crate::world) const SPELL_AURA_MOD_INVISIBILITY_DETECTION: u32 = 19;
pub(in crate::world) const SPELL_AURA_OBS_MOD_HEALTH: u32 = 20;
pub(in crate::world) const SPELL_AURA_PERIODIC_TRIGGER_SPELL: u32 = 23;
pub(in crate::world) const SPELL_AURA_PERIODIC_ENERGIZE: u32 = 24;
pub(in crate::world) const SPELL_AURA_MOD_PACIFY: u32 = 25;
pub(in crate::world) const SPELL_AURA_MOD_ROOT: u32 = 26;
pub(in crate::world) const SPELL_AURA_MOD_SILENCE: u32 = 27;
pub(in crate::world) const SPELL_AURA_MOD_STAT: u32 = 29;
pub(in crate::world) const SPELL_AURA_MOD_SHAPESHIFT: u32 = 36;
pub(in crate::world) const SPELL_AURA_MOD_RESISTANCE: u32 = 22;
pub(in crate::world) const SPELL_AURA_MOD_INCREASE_SPEED: u32 = 31;
pub(in crate::world) const SPELL_AURA_MOD_DECREASE_SPEED: u32 = 33;
pub(in crate::world) const SPELL_AURA_PROC_TRIGGER_SPELL: u32 = 42;
pub(in crate::world) const SPELL_AURA_MOD_BLOCK_PERCENT: u32 = 51;
pub(in crate::world) const SPELL_AURA_MOD_CRIT_PERCENT: u32 = 52;
pub(in crate::world) const SPELL_AURA_MOD_PACIFY_SILENCE: u32 = 60;
pub(in crate::world) const SPELL_AURA_MOD_DISARM: u32 = 67;
pub(in crate::world) const SPELL_AURA_MOD_STALKED: u32 = 68;
pub(in crate::world) const SPELL_AURA_SCHOOL_ABSORB: u32 = 69;
pub(in crate::world) const SPELL_AURA_MECHANIC_IMMUNITY: u32 = 77;
pub(in crate::world) const SPELL_AURA_MANA_SHIELD: u32 = 97;
pub(in crate::world) const SPELL_AURA_MOD_RESISTANCE_PCT: u32 = 101;
pub(in crate::world) const SPELL_AURA_MOD_POWER_REGEN_PERCENT: u32 = 110;
pub(in crate::world) const SPELL_AURA_MOD_HEALING: u32 = 115;
pub(in crate::world) const SPELL_AURA_MOD_MANA_REGEN_INTERRUPT: u32 = 134;
pub(in crate::world) const SPELL_AURA_MOD_SKILL_TALENT: u32 = 98;
pub(in crate::world) const SPELL_AURA_MOD_SKILL: u32 = 30;
pub(in crate::world) const SPELL_AURA_MOD_REGEN: u32 = 84;
pub(in crate::world) const SPELL_AURA_MOD_POWER_REGEN: u32 = 85;
pub(in crate::world) const SPELL_AURA_MOD_DAMAGE_PERCENT_TAKEN: u32 = 87;
pub(in crate::world) const SPELL_AURA_MOD_ATTACK_POWER: u32 = 99;
pub(in crate::world) const SPELL_AURA_MOD_TOTAL_STAT_PERCENTAGE: u32 = 137;
pub(in crate::world) const SPELL_AURA_MOD_MELEE_HASTE: u32 = 138;
pub(in crate::world) const SPELL_AURA_MOD_REPUTATION_GAIN: u32 = 156;
pub(in crate::world) const SPELL_AURA_MOD_DAMAGE_PERCENT_DONE: u32 = 79;
pub(in crate::world) const SPELL_AURA_TRACK_CREATURES: u32 = 44;
pub(in crate::world) const SPELL_AURA_TRACK_RESOURCES: u32 = 45;
pub(in crate::world) const SPELL_AURA_TRANSFORM: u32 = 56;
pub(in crate::world) const SPELL_AURA_GHOST: u32 = 95;
pub(in crate::world) const SPELL_AURA_WATER_WALK: u32 = 104;
pub(in crate::world) const SPELL_AURA_FEATHER_FALL: u32 = 105;
pub(in crate::world) const AURA_INTERRUPT_FLAG_DAMAGE: u32 = 0x0000_0002;
pub(in crate::world) const AURA_INTERRUPT_FLAG_MOVING: u32 = 0x0000_0008;
pub(in crate::world) const AURA_INTERRUPT_FLAG_DAMAGE_CHANNEL_DURATION: u32 = 0x0000_4000;
pub(in crate::world) const AURA_INTERRUPT_FLAG_STANDING_CANCELS: u32 = 0x0004_0000;
pub(in crate::world) const PLAYER_STAND_STATE_STAND: u8 = 0;
pub(in crate::world) const PLAYER_STAND_STATE_SIT: u8 = 1;
pub(in crate::world) const PLAYER_STAND_STATE_SLEEP: u8 = 3;
pub(in crate::world) const PLAYER_STAND_STATE_DEAD: u8 = 7;
pub(in crate::world) const PLAYER_STAND_STATE_KNEEL: u8 = 8;
pub(in crate::world) const POWER_TYPE_MANA: u32 = 0;
pub(in crate::world) const POWER_TYPE_RAGE: u32 = 1;
pub(in crate::world) const POWER_TYPE_ENERGY: u32 = 3;
pub(in crate::world) const DISPEL_ALL: u32 = 7;
pub(in crate::world) const POSITIVE_AURA_FLAGS: u32 = 0x05;
pub(in crate::world) const NEGATIVE_AURA_FLAGS: u32 = 0x08;
pub(in crate::world) const TARGET_UNIT_CASTER: u32 = 1;
pub(in crate::world) const TARGET_UNIT_ENEMY: u32 = 6;
pub(in crate::world) const TARGET_ENUM_UNITS_ENEMY_AOE_AT_SRC_LOC: u32 = 15;
pub(in crate::world) const TARGET_ENUM_UNITS_ENEMY_AOE_AT_DEST_LOC: u32 = 16;
pub(in crate::world) const TARGET_LOCATION_CASTER_SRC: u32 = 22;
pub(in crate::world) const TARGET_ENUM_UNITS_ENEMY_IN_CONE_24: u32 = 24;
pub(in crate::world) const TARGET_ENUM_UNITS_ENEMY_AOE_AT_DYNOBJ_LOC: u32 = 28;
pub(in crate::world) const TARGET_UNIT_FRIEND: u32 = 21;
pub(in crate::world) const TARGET_UNIT: u32 = 25;
pub(in crate::world) const TARGET_UNIT_PARTY: u32 = 35;
pub(in crate::world) const TARGET_ENUM_UNITS_ENEMY_WITHIN_CASTER_RANGE: u32 = 36;
pub(in crate::world) const TARGET_LOCATION_CASTER_TARGET_POSITION: u32 = 53;
pub(in crate::world) const TARGET_ENUM_UNITS_ENEMY_IN_CONE_54: u32 = 54;
pub(in crate::world) const TARGET_LOCATION_CASTER_FRONT_LEAP: u32 = 55;
pub(in crate::world) const TARGET_UNIT_FRIEND_AND_PARTY: u32 = 37;
pub(in crate::world) const TARGET_UNIT_FRIEND_CHAIN_HEAL: u32 = 45;
pub(in crate::world) const TARGET_UNIT_RAID: u32 = 57;
pub(in crate::world) const TARGET_UNIT_RAID_NEAR_CASTER: u32 = 58;
pub(in crate::world) const TARGET_UNIT_RAID_AND_CLASS: u32 = 61;
pub(in crate::world) const SPELL_GROUP_RULE_UNIQUE: u32 = 1;
pub(in crate::world) const SPELL_GROUP_RULE_UNIQUE_PER_CASTER: u32 = 2;
pub(in crate::world) const PROC_FLAG_TAKE_MELEE_SWING: u32 = 0x0000_0008;
pub(in crate::world) const PROC_EX_BLOCK: u32 = 0x0000_0040;
pub(in crate::world) const ITEM_SPELLTRIGGER_ON_USE: u32 = 0;
pub(in crate::world) const ITEM_SPELLTRIGGER_ON_NO_DELAY_USE: u32 = 5;
pub(in crate::world) const SPELL_ATTR_EX2_DONT_BLOCK_MANA_REGEN: u32 = 0x0200_0000;
pub(in crate::world) const SPELL_ATTR_EX2_AUTO_REPEAT: u32 = 0x0000_0020;
pub(in crate::world) const SPELL_ATTR_EX3_CASTING_CANCELS_AUTOREPEAT: u32 = 0x0040_0000;
pub(in crate::world) const SPELL_ATTR_SS_FACING_BACK: u32 = 0x0000_0008;
pub(in crate::world) const SPELL_FACING_FLAG_INFRONT: u32 = 0x0000_0001;
pub(in crate::world) const SPELL_INTERRUPT_FLAG_COMBAT: u32 = 0x08;
pub(in crate::world) const SPELL_RANGE_FLAG_MELEE: u32 = 0x1;
pub(in crate::world) const SPELL_RANGE_FLAG_RANGED: u32 = 0x2;
pub(in crate::world) const SPELL_CAST_ARC_RADIANS: f32 = std::f32::consts::PI;
pub(in crate::world) const SPELL_DEFAULT_CONE_RADIANS: f32 = std::f32::consts::PI / 3.0;
pub(in crate::world) const BASE_CHARGE_SPEED: f32 = 27.0;
pub(in crate::world) const SPELL_SCHOOL_MASK_NORMAL: u32 = 0x01;
pub(in crate::world) const SPELL_FAMILY_GENERIC: u32 = 0;
pub(in crate::world) const SPELL_FAMILY_MAGE: u32 = 3;
pub(in crate::world) const SPELL_FAMILY_HUNTER: u32 = 9;
pub(in crate::world) const MECHANIC_FEAR: u32 = 5;
pub(in crate::world) const MECHANIC_ROOT: u32 = 7;
pub(in crate::world) const MECHANIC_SLEEP: u32 = 10;
pub(in crate::world) const MECHANIC_KNOCKOUT: u32 = 14;
pub(in crate::world) const MECHANIC_POLYMORPH: u32 = 17;
pub(in crate::world) const MECHANIC_BANISH: u32 = 18;
pub(in crate::world) const MECHANIC_SHACKLE: u32 = 20;
pub(in crate::world) const MECHANIC_TURN: u32 = 23;
pub(in crate::world) const POLYMORPH_HELPER_REGEN_SPELL_ID: u32 = 12_939;
pub(in crate::world) const MAX_AURA_SLOTS: usize = 48;
pub(in crate::world) const MAX_POSITIVE_AURA_SLOTS: usize = 32;
pub(in crate::world) const MAX_AURA_FLAG_FIELDS: usize = 6;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(in crate::world) enum DiminishingGroupRuntime {
    Polymorph,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::world) enum DiminishingLevelRuntime {
    Level1,
    Level2,
    Level3,
    Immune,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::world) struct SingleTargetAuraDescriptor {
    pub(in crate::world) spell_id: u32,
    pub(in crate::world) chain_root: u32,
    pub(in crate::world) spell_family_name: u32,
    pub(in crate::world) spell_family_flags: u64,
    pub(in crate::world) mechanic: u32,
}

pub(in crate::world) fn spell_damage_pushback_delay_millis(pushback_count: u8) -> u32 {
    match pushback_count {
        0 => 1000,
        1 => 800,
        2 => 600,
        3 => 400,
        _ => 200,
    }
}
pub(in crate::world) const MAX_AURA_LEVEL_FIELDS: usize = 12;
