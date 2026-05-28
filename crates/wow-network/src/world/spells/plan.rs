use super::*;

impl<'a> SpellInfo<'a> {
    pub(in crate::world) fn player_spell_plan(&self) -> Option<SpellPlan> {
        let kind = self.player_cast_kind()?;
        let profile = self.build_cast_profile(kind);
        let target = self.plan_target(kind, profile.aura_target);
        let channel = self.plan_channel();
        let effects = self
            .effects
            .iter()
            .copied()
            .enumerate()
            .filter(|(_, effect)| effect.dispatch != SpellEffectDispatch::Empty)
            .map(|(index, effect)| SpellPlanEffect {
                index,
                dispatch: effect.dispatch,
                aura_name: effect.aura_name,
                target: plan_effect_target(effect),
            })
            .collect();

        Some(SpellPlan {
            spell_id: self.template.id,
            profile,
            target,
            channel,
            effects,
            behavior: self.plan_behavior(kind),
            flags: self.plan_flags(),
        })
    }

    pub(in crate::world) fn item_spell_plan(&self, _item_guid: ObjectGuid) -> Option<SpellPlan> {
        if self.has_effect(SpellEffectDispatch::Charge) || self.has_on_next_swing_attribute() {
            return None;
        }
        let kind = if self.has_effect(SpellEffectDispatch::ApplyAura) {
            SpellCastKind::AuraApplication
        } else if self.has_effect(SpellEffectDispatch::Teleport) {
            SpellCastKind::Teleport
        } else if self.has_direct_heal_effect() {
            SpellCastKind::DirectHeal
        } else if self.has_item_direct_effect() {
            SpellCastKind::InstantDamage
        } else {
            return None;
        };
        let mut profile = self.build_cast_profile(kind);
        profile.bonus_damage = 0;
        if kind != SpellCastKind::InstantDamage {
            profile.damage = 0;
        }
        profile.requires_melee = false;

        Some(SpellPlan {
            spell_id: self.template.id,
            target: self.plan_target(kind, profile.aura_target),
            channel: None,
            effects: self
                .effects
                .iter()
                .copied()
                .enumerate()
                .filter(|(_, effect)| effect.dispatch != SpellEffectDispatch::Empty)
                .map(|(index, effect)| SpellPlanEffect {
                    index,
                    dispatch: effect.dispatch,
                    aura_name: effect.aura_name,
                    target: plan_effect_target(effect),
                })
                .collect(),
            profile,
            behavior: self.plan_behavior(kind),
            flags: self.plan_flags(),
        })
    }

    pub(in crate::world) fn db_creature_spell_plan(
        &self,
        target: ObjectGuid,
        value_context: SpellEffectValueContext,
    ) -> Option<DbCreatureSpellPlan> {
        if !self.can_db_creature_autocast() {
            return None;
        }
        let aura = self.has_effect(SpellEffectDispatch::ApplyAura)
            && (target.is_player() || target.is_creature());
        let effect = if self.has_direct_damage_effect() {
            let amount = self.direct_damage_with_context(value_context);
            if !target.is_player() || amount == 0 {
                return None;
            }
            DbCreatureSpellPlanEffect::Damage {
                amount,
                school: self.template.school as u8,
                dmg_class: self.template.dmg_class,
                attributes_ex2: self.template.attributes_ex2,
                attributes_ex3: self.template.attributes_ex3,
            }
        } else if self.has_direct_heal_effect() {
            let amount = self.direct_heal_with_context(value_context);
            if target.is_player() || amount == 0 {
                return None;
            }
            DbCreatureSpellPlanEffect::Heal { amount }
        } else if aura {
            DbCreatureSpellPlanEffect::AuraOnly
        } else {
            return None;
        };
        let mana_cost = if self.template.power_type == POWER_TYPE_MANA {
            self.template.mana_cost
        } else {
            0
        };
        Some(DbCreatureSpellPlan {
            spell_id: self.template.id,
            requires_behind: self.requires_behind_target(),
            mana_cost,
            aura,
            effect,
        })
    }

    pub(in crate::world) fn can_db_creature_autocast(&self) -> bool {
        (self.template.attributes_ex & SPELL_ATTR_EX_NO_AUTOCAST_AI) == 0
            && (self.template.attributes & SPELL_ATTR_PASSIVE) == 0
    }

    pub(in crate::world) fn needs_passive_cast_at_learn(&self) -> bool {
        self.template.attributes & SPELL_ATTR_PASSIVE != 0
            && self.has_effect(SpellEffectDispatch::ApplyAura)
    }

    pub(in crate::world) fn plan_behavior(&self, kind: SpellCastKind) -> SpellPlanBehavior {
        SpellPlanBehavior {
            resets_auto_attack_timers: self.template.interrupt_flags & SPELL_INTERRUPT_FLAG_COMBAT
                != 0
                && !matches!(
                    kind,
                    SpellCastKind::AutoRepeatRanged | SpellCastKind::NextMeleeSwing
                ),
            cancels_auto_repeat_when_casting: self.template.attributes_ex3
                & SPELL_ATTR_EX3_CASTING_CANCELS_AUTOREPEAT
                != 0,
            blocks_mana_regen: self.template.power_type == POWER_TYPE_MANA
                && self.template.mana_cost > 0
                && (self.template.attributes_ex2 & SPELL_ATTR_EX2_DONT_BLOCK_MANA_REGEN) == 0,
        }
    }

    pub(in crate::world) fn plan_flags(&self) -> Vec<SpellPlanFlag> {
        let mut flags = Vec::new();
        collect_spell_plan_flags(
            &mut flags,
            SpellPlanFlagField::Attributes,
            self.template.attributes,
            &ATTRIBUTE_FLAG_SPECS,
        );
        collect_spell_plan_flags(
            &mut flags,
            SpellPlanFlagField::AttributesEx,
            self.template.attributes_ex,
            &ATTRIBUTE_EX_FLAG_SPECS,
        );
        collect_spell_plan_flags(
            &mut flags,
            SpellPlanFlagField::AttributesEx2,
            self.template.attributes_ex2,
            &ATTRIBUTE_EX2_FLAG_SPECS,
        );
        collect_spell_plan_flags(
            &mut flags,
            SpellPlanFlagField::AttributesEx3,
            self.template.attributes_ex3,
            &ATTRIBUTE_EX3_FLAG_SPECS,
        );
        collect_spell_plan_flags(
            &mut flags,
            SpellPlanFlagField::AttributesServerSide,
            self.template.attributes_serverside,
            &ATTRIBUTE_SERVERSIDE_FLAG_SPECS,
        );
        flags
    }

    fn player_cast_kind(&self) -> Option<SpellCastKind> {
        if self.is_auto_repeat_ranged() {
            Some(SpellCastKind::AutoRepeatRanged)
        } else if self.has_on_next_swing_attribute() {
            Some(SpellCastKind::NextMeleeSwing)
        } else if self.has_effect(SpellEffectDispatch::Charge) {
            Some(SpellCastKind::Charge)
        } else if self.has_effect(SpellEffectDispatch::Teleport)
            || self.has_effect(SpellEffectDispatch::Leap)
        {
            Some(SpellCastKind::Teleport)
        } else if self.has_direct_heal_effect() {
            Some(SpellCastKind::DirectHeal)
        } else if self.has_effect(SpellEffectDispatch::Dispel)
            || self.has_effect(SpellEffectDispatch::Threat)
            || self.has_effect(SpellEffectDispatch::Distract)
            || self.has_effect(SpellEffectDispatch::PickPocket)
            || self.has_effect(SpellEffectDispatch::SummonPet)
            || self.has_effect(SpellEffectDispatch::SummonPossessed)
        {
            Some(SpellCastKind::AuraApplication)
        } else if self.has_effect(SpellEffectDispatch::CreateItem) {
            Some(SpellCastKind::CreateItem)
        } else if self.has_effect(SpellEffectDispatch::TransportDoor) {
            Some(SpellCastKind::AuraApplication)
        } else if self.has_effect(SpellEffectDispatch::InterruptCast) {
            Some(SpellCastKind::Interrupt)
        } else if self.has_effect(SpellEffectDispatch::ApplyAura) {
            Some(SpellCastKind::AuraApplication)
        } else if self.has_power_burn_effect() || self.has_direct_damage_effect() {
            Some(SpellCastKind::InstantDamage)
        } else if self.has_effect(SpellEffectDispatch::PersistentAreaAura)
            || self.has_effect(SpellEffectDispatch::SpellScript)
        {
            Some(SpellCastKind::AuraApplication)
        } else {
            None
        }
    }

    pub(in crate::world) fn plan_channel(&self) -> Option<SpellPlanChannel> {
        if !self.is_channeled() {
            return None;
        }
        if self.has_effect(SpellEffectDispatch::PersistentAreaAura) {
            return Some(SpellPlanChannel::PersistentArea {
                duration_index: self.template.duration_index,
                interrupt_flags: self.template.channel_interrupt_flags,
            });
        }
        if let Some(effect) = self.effects.iter().find(|effect| {
            effect.dispatch == SpellEffectDispatch::ApplyAura
                && effect.aura_name == SPELL_AURA_PERIODIC_TRIGGER_SPELL
                && effect.trigger_spell != 0
        }) {
            return Some(SpellPlanChannel::UnitPeriodicTrigger {
                trigger_spell: effect.trigger_spell,
                tick_millis: effect.amplitude,
                duration_index: self.template.duration_index,
                interrupt_flags: self.template.channel_interrupt_flags,
            });
        }
        if self.effects.iter().any(|effect| {
            effect.dispatch == SpellEffectDispatch::ApplyAura
                && matches!(
                    plan_effect_target(*effect),
                    SpellPlanEffectTarget::Unit
                        | SpellPlanEffectTarget::HostileUnit
                        | SpellPlanEffectTarget::FriendlyUnit
                )
        }) {
            return Some(SpellPlanChannel::UnitAura {
                duration_index: self.template.duration_index,
                interrupt_flags: self.template.channel_interrupt_flags,
            });
        }
        Some(SpellPlanChannel::SelfAura {
            duration_index: self.template.duration_index,
            interrupt_flags: self.template.channel_interrupt_flags,
        })
    }

    pub(in crate::world) fn is_channeled(&self) -> bool {
        self.template.attributes_ex & (SPELL_ATTR_EX_IS_CHANNELED | SPELL_ATTR_EX_IS_SELF_CHANNELED)
            != 0
    }

    pub(in crate::world) fn plan_target(
        &self,
        kind: SpellCastKind,
        aura_target: SpellAuraTarget,
    ) -> SpellPlanTarget {
        match kind {
            SpellCastKind::AutoRepeatRanged
            | SpellCastKind::Charge
            | SpellCastKind::NextMeleeSwing
            | SpellCastKind::Interrupt => SpellPlanTarget::HostileUnit,
            SpellCastKind::DirectHeal => {
                if aura_target == SpellAuraTarget::Caster {
                    SpellPlanTarget::Caster
                } else {
                    SpellPlanTarget::FriendlyUnit
                }
            }
            SpellCastKind::AuraApplication => {
                if matches!(
                    self.plan_channel(),
                    Some(SpellPlanChannel::UnitPeriodicTrigger { .. })
                ) {
                    return SpellPlanTarget::HostileUnit;
                }
                match aura_target {
                    SpellAuraTarget::Caster => SpellPlanTarget::Caster,
                    SpellAuraTarget::CasterAreaEnemy => SpellPlanTarget::CasterAreaEnemy {
                        cone: self.effects.iter().any(|effect| {
                            effect.dispatch == SpellEffectDispatch::ApplyAura
                                && effect_targets_caster_centered_hostile_cone(*effect)
                        }),
                    },
                    SpellAuraTarget::DestinationAreaEnemy => SpellPlanTarget::DestinationAreaEnemy,
                    SpellAuraTarget::UnitTarget => {
                        if self.effects.iter().any(|effect| {
                            effect.dispatch == SpellEffectDispatch::ApplyAura
                                && effect_targets_caster_centered_friendly_area(*effect)
                        }) {
                            SpellPlanTarget::FriendlyUnit
                        } else if self.effects.iter().any(|effect| {
                            effect.dispatch == SpellEffectDispatch::PickPocket
                                || effect_targets_direct_hostile_unit(*effect)
                                || effect.aura_name == SPELL_AURA_PERIODIC_DAMAGE
                                || effect.aura_name == SPELL_AURA_PERIODIC_TRIGGER_SPELL
                        }) {
                            SpellPlanTarget::HostileUnit
                        } else if self
                            .effects
                            .iter()
                            .any(|effect| effect_targets_direct_friendly_unit(*effect))
                        {
                            SpellPlanTarget::FriendlyUnit
                        } else {
                            SpellPlanTarget::Unit
                        }
                    }
                }
            }
            SpellCastKind::InstantDamage => self.instant_damage_target(),
            SpellCastKind::CreateItem
            | SpellCastKind::OpeningGameObject
            | SpellCastKind::Teleport => SpellPlanTarget::Caster,
        }
    }

    fn instant_damage_target(&self) -> SpellPlanTarget {
        if self.effects.iter().any(|effect| {
            matches!(
                effect.dispatch,
                SpellEffectDispatch::SchoolDamage
                    | SpellEffectDispatch::WeaponDamage
                    | SpellEffectDispatch::WeaponPercentDamage
            ) && effect_targets_destination_hostile_area(*effect)
        }) {
            SpellPlanTarget::Destination
        } else if self.effects.iter().any(|effect| {
            let target = plan_effect_target(*effect);
            matches!(
                effect.dispatch,
                SpellEffectDispatch::SchoolDamage
                    | SpellEffectDispatch::WeaponDamage
                    | SpellEffectDispatch::WeaponPercentDamage
            ) && matches!(
                target,
                SpellPlanEffectTarget::Caster
                    | SpellPlanEffectTarget::CasterAreaEnemy { .. }
                    | SpellPlanEffectTarget::DestinationAreaEnemy
            )
        }) {
            SpellPlanTarget::Caster
        } else {
            SpellPlanTarget::HostileUnit
        }
    }
}

pub(in crate::world) fn plan_effect_target(effect: SpellInfoEffect) -> SpellPlanEffectTarget {
    if effect_targets_target_party_friendly_area(effect) {
        return SpellPlanEffectTarget::TargetPartyFriendly;
    }
    if effect_targets_caster_centered_friendly_area(effect) {
        return SpellPlanEffectTarget::CasterAreaFriendly;
    }
    if effect_targets_caster_centered_hostile_area(effect) {
        return SpellPlanEffectTarget::CasterAreaEnemy {
            cone: effect_targets_caster_centered_hostile_cone(effect),
        };
    }
    if effect_targets_destination_hostile_area(effect) {
        return SpellPlanEffectTarget::DestinationAreaEnemy;
    }
    if [effect.implicit_target_a, effect.implicit_target_b]
        .into_iter()
        .any(|target| target == TARGET_UNIT_ENEMY)
    {
        return SpellPlanEffectTarget::HostileUnit;
    }
    if [effect.implicit_target_a, effect.implicit_target_b]
        .into_iter()
        .any(|target| {
            matches!(
                target,
                TARGET_UNIT_FRIEND | TARGET_UNIT_PARTY | TARGET_UNIT_RAID
            )
        })
    {
        return SpellPlanEffectTarget::FriendlyUnit;
    }
    if [effect.implicit_target_a, effect.implicit_target_b]
        .into_iter()
        .any(is_direct_unit_target)
    {
        return SpellPlanEffectTarget::Unit;
    }
    if [effect.implicit_target_a, effect.implicit_target_b]
        .into_iter()
        .any(|target| target == TARGET_LOCATION_CASTER_FRONT_LEAP)
    {
        return SpellPlanEffectTarget::CasterFrontLeap;
    }
    if [effect.implicit_target_a, effect.implicit_target_b]
        .into_iter()
        .any(|target| target == TARGET_UNIT_CASTER)
    {
        return SpellPlanEffectTarget::Caster;
    }
    SpellPlanEffectTarget::None
}

#[derive(Debug, Clone, Copy)]
struct SpellPlanFlagSpec {
    bit: u32,
    name: &'static str,
    support: SpellPlanFlagSupport,
}

const ATTRIBUTE_FLAG_SPECS: [SpellPlanFlagSpec; 6] = [
    SpellPlanFlagSpec {
        bit: SPELL_ATTR_USES_RANGED_SLOT,
        name: "SPELL_ATTR_USES_RANGED_SLOT",
        support: SpellPlanFlagSupport::ImplementedGeneric("auto-repeat ranged cast profile"),
    },
    SpellPlanFlagSpec {
        bit: SPELL_ATTR_ON_NEXT_SWING_NO_DAMAGE,
        name: "SPELL_ATTR_ON_NEXT_SWING_NO_DAMAGE",
        support: SpellPlanFlagSupport::ImplementedGeneric("queued next melee swing"),
    },
    SpellPlanFlagSpec {
        bit: SPELL_ATTR_PASSIVE,
        name: "SPELL_ATTR_PASSIVE",
        support: SpellPlanFlagSupport::ImplementedGeneric(
            "passive learn and creature autocast gate",
        ),
    },
    SpellPlanFlagSpec {
        bit: SPELL_ATTR_ON_NEXT_SWING,
        name: "SPELL_ATTR_ON_NEXT_SWING",
        support: SpellPlanFlagSupport::ImplementedGeneric("queued next melee swing"),
    },
    SpellPlanFlagSpec {
        bit: SPELL_ATTR_ONLY_STEALTHED,
        name: "SPELL_ATTR_ONLY_STEALTHED",
        support: SpellPlanFlagSupport::ImplementedGeneric("caster stealth cast validation"),
    },
    SpellPlanFlagSpec {
        bit: SPELL_ATTR_HEARTBEAT_RESIST,
        name: "SPELL_ATTR_HEARTBEAT_RESIST",
        support: SpellPlanFlagSupport::ImplementedGeneric("generic heartbeat early-break runtime"),
    },
];

const ATTRIBUTE_EX_FLAG_SPECS: [SpellPlanFlagSpec; 8] = [
    SpellPlanFlagSpec {
        bit: SPELL_ATTR_EX_IS_CHANNELED,
        name: "SPELL_ATTR_EX_IS_CHANNELED",
        support: SpellPlanFlagSupport::ImplementedGeneric("generic channel lifecycle"),
    },
    SpellPlanFlagSpec {
        bit: 0x0000_0008,
        name: "ATTRIBUTES_EX_BIT_0x00000008",
        support: SpellPlanFlagSupport::PendingGeneric(
            "target/cast exception needs CMaNGOS parity mapping",
        ),
    },
    SpellPlanFlagSpec {
        bit: SPELL_ATTR_EX_IS_SELF_CHANNELED,
        name: "SPELL_ATTR_EX_IS_SELF_CHANNELED",
        support: SpellPlanFlagSupport::ImplementedGeneric("generic self-channel lifecycle"),
    },
    SpellPlanFlagSpec {
        bit: SPELL_ATTR_EX_ONLY_PEACEFUL_TARGETS,
        name: "SPELL_ATTR_EX_ONLY_PEACEFUL_TARGETS",
        support: SpellPlanFlagSupport::ImplementedGeneric("out-of-combat target validation"),
    },
    SpellPlanFlagSpec {
        bit: 0x0000_0200,
        name: "ATTRIBUTES_EX_BIT_0x00000200",
        support: SpellPlanFlagSupport::ScriptRequired(
            "special spell-family behavior belongs in script layer",
        ),
    },
    SpellPlanFlagSpec {
        bit: SPELL_ATTR_EX_NO_AUTOCAST_AI,
        name: "SPELL_ATTR_EX_NO_AUTOCAST_AI",
        support: SpellPlanFlagSupport::ImplementedGeneric("DB-creature autocast rejection"),
    },
    SpellPlanFlagSpec {
        bit: SPELL_ATTR_EX_FINISHING_MOVE_DAMAGE,
        name: "SPELL_ATTR_EX_FINISHING_MOVE_DAMAGE",
        support: SpellPlanFlagSupport::ImplementedGeneric("combo-point finisher validation/clear"),
    },
    SpellPlanFlagSpec {
        bit: SPELL_ATTR_EX_FINISHING_MOVE_DURATION,
        name: "SPELL_ATTR_EX_FINISHING_MOVE_DURATION",
        support: SpellPlanFlagSupport::ImplementedGeneric("combo-point finisher validation/clear"),
    },
];

const ATTRIBUTE_EX2_FLAG_SPECS: [SpellPlanFlagSpec; 4] = [
    SpellPlanFlagSpec {
        bit: SPELL_ATTR_EX2_AUTO_REPEAT,
        name: "SPELL_ATTR_EX2_AUTO_REPEAT",
        support: SpellPlanFlagSupport::ImplementedGeneric("auto-repeat ranged cast profile"),
    },
    SpellPlanFlagSpec {
        bit: SPELL_ATTR_EX2_DONT_BLOCK_MANA_REGEN,
        name: "SPELL_ATTR_EX2_DONT_BLOCK_MANA_REGEN",
        support: SpellPlanFlagSupport::ImplementedGeneric("mana-regeneration cast behavior"),
    },
    SpellPlanFlagSpec {
        bit: 0x2000_0000,
        name: "SPELL_ATTR_EX2_CANT_CRIT",
        support: SpellPlanFlagSupport::ExecutionPayload("spell damage outcome calculation"),
    },
    SpellPlanFlagSpec {
        bit: 0x0000_0004,
        name: "SPELL_ATTR_EX2_UNK2",
        support: SpellPlanFlagSupport::KnownNoOp(
            "observed on shout-style fixtures without generic runtime effect",
        ),
    },
];

const ATTRIBUTE_EX3_FLAG_SPECS: [SpellPlanFlagSpec; 3] = [
    SpellPlanFlagSpec {
        bit: SPELL_ATTR_EX3_REQUIRES_MAIN_HAND_WEAPON,
        name: "SPELL_ATTR_EX3_REQUIRES_MAIN_HAND_WEAPON",
        support: SpellPlanFlagSupport::ImplementedGeneric("queued melee spell weapon validation"),
    },
    SpellPlanFlagSpec {
        bit: 0x0004_0000,
        name: "SPELL_ATTR_EX3_ALWAYS_HIT",
        support: SpellPlanFlagSupport::ExecutionPayload("spell hit outcome calculation"),
    },
    SpellPlanFlagSpec {
        bit: SPELL_ATTR_EX3_CASTING_CANCELS_AUTOREPEAT,
        name: "SPELL_ATTR_EX3_CASTING_CANCELS_AUTOREPEAT",
        support: SpellPlanFlagSupport::ImplementedGeneric("auto-repeat cancellation on cast"),
    },
];

const ATTRIBUTE_SERVERSIDE_FLAG_SPECS: [SpellPlanFlagSpec; 1] = [SpellPlanFlagSpec {
    bit: SPELL_ATTR_SS_FACING_BACK,
    name: "SPELL_ATTR_SS_FACING_BACK",
    support: SpellPlanFlagSupport::ImplementedGeneric("behind-target cast validation"),
}];

fn collect_spell_plan_flags(
    flags: &mut Vec<SpellPlanFlag>,
    field: SpellPlanFlagField,
    value: u32,
    specs: &[SpellPlanFlagSpec],
) {
    let mut known_bits = 0;
    for spec in specs {
        known_bits |= spec.bit;
        if value & spec.bit != 0 {
            flags.push(SpellPlanFlag {
                field,
                bit: spec.bit,
                name: Some(spec.name),
                support: spec.support,
            });
        }
    }

    let mut unknown_bits = value & !known_bits;
    while unknown_bits != 0 {
        let bit = unknown_bits & unknown_bits.wrapping_neg();
        flags.push(SpellPlanFlag {
            field,
            bit,
            name: None,
            support: SpellPlanFlagSupport::Unknown,
        });
        unknown_bits &= !bit;
    }
}
