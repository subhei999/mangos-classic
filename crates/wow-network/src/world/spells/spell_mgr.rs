use super::*;

#[derive(Debug, Clone, Copy)]
pub(in crate::world) struct SpellInfo<'a> {
    pub(in crate::world) template: &'a wow_db::SpellTemplateQuery,
    pub(in crate::world) effects: [SpellInfoEffect; 3],
}

pub(in crate::world) const SPELL_ATTR_EX_NO_AUTOCAST_AI: u32 = 0x0002_0000;

#[derive(Debug, Clone, Copy)]
#[allow(dead_code)]
pub(in crate::world) struct SpellInfoEffect {
    pub(in crate::world) effect_id: u32,
    pub(in crate::world) aura_name: u32,
    pub(in crate::world) base_points: i32,
    pub(in crate::world) die_sides: i32,
    pub(in crate::world) base_dice: u32,
    pub(in crate::world) points_per_combo_point: f32,
    pub(in crate::world) amplitude: u32,
    pub(in crate::world) implicit_target_a: u32,
    pub(in crate::world) implicit_target_b: u32,
    pub(in crate::world) radius_index: u32,
    pub(in crate::world) misc_value: i32,
    pub(in crate::world) mechanic: u32,
    pub(in crate::world) trigger_spell: u32,
    pub(in crate::world) item_type: u32,
    pub(in crate::world) dispatch: SpellEffectDispatch,
}

#[derive(Debug, Clone, Copy)]
pub(in crate::world) struct SpellInfoEffectSlot {
    pub(in crate::world) effect_id: u32,
    pub(in crate::world) aura_name: u32,
    pub(in crate::world) base_points: i32,
    pub(in crate::world) roll: (i32, u32, f32),
    pub(in crate::world) amplitude: u32,
    pub(in crate::world) implicit_target_a: u32,
    pub(in crate::world) implicit_target_b: u32,
    pub(in crate::world) radius_index: u32,
    pub(in crate::world) misc_value: i32,
    pub(in crate::world) mechanic: u32,
    pub(in crate::world) trigger_spell: u32,
    pub(in crate::world) item_type: u32,
}

impl<'a> SpellInfo<'a> {
    pub(in crate::world) fn from_template(template: &'a wow_db::SpellTemplateQuery) -> Self {
        let effects = [
            SpellInfoEffect::from_template_slot(SpellInfoEffectSlot {
                effect_id: template.effect1,
                aura_name: template.effect_apply_aura_name1,
                base_points: template.effect_base_points1,
                roll: (
                    template.effect_die_sides1,
                    template.effect_base_dice1,
                    template.effect_points_per_combo_point1,
                ),
                amplitude: template.effect_amplitude1,
                implicit_target_a: template.effect_implicit_target_a1,
                implicit_target_b: template.effect_implicit_target_b1,
                radius_index: template.effect_radius_index1,
                misc_value: template.effect_misc_value1,
                mechanic: template.effect_mechanic1,
                trigger_spell: template.effect_trigger_spell1,
                item_type: template.effect_item_type1,
            }),
            SpellInfoEffect::from_template_slot(SpellInfoEffectSlot {
                effect_id: template.effect2,
                aura_name: template.effect_apply_aura_name2,
                base_points: template.effect_base_points2,
                roll: (
                    template.effect_die_sides2,
                    template.effect_base_dice2,
                    template.effect_points_per_combo_point2,
                ),
                amplitude: template.effect_amplitude2,
                implicit_target_a: template.effect_implicit_target_a2,
                implicit_target_b: template.effect_implicit_target_b2,
                radius_index: template.effect_radius_index2,
                misc_value: template.effect_misc_value2,
                mechanic: template.effect_mechanic2,
                trigger_spell: template.effect_trigger_spell2,
                item_type: template.effect_item_type2,
            }),
            SpellInfoEffect::from_template_slot(SpellInfoEffectSlot {
                effect_id: template.effect3,
                aura_name: template.effect_apply_aura_name3,
                base_points: template.effect_base_points3,
                roll: (
                    template.effect_die_sides3,
                    template.effect_base_dice3,
                    template.effect_points_per_combo_point3,
                ),
                amplitude: template.effect_amplitude3,
                implicit_target_a: template.effect_implicit_target_a3,
                implicit_target_b: template.effect_implicit_target_b3,
                radius_index: template.effect_radius_index3,
                misc_value: template.effect_misc_value3,
                mechanic: template.effect_mechanic3,
                trigger_spell: template.effect_trigger_spell3,
                item_type: template.effect_item_type3,
            }),
        ];
        Self { template, effects }
    }

    pub(in crate::world) fn prepare_player_cast(&self) -> Option<PreparedSpellCast> {
        self.player_cast_profile().map(|profile| {
            PreparedSpellCast::new(self.template.id, SpellCastSource::Player, profile)
        })
    }

    pub(in crate::world) fn prepare_item_cast(
        &self,
        item_guid: ObjectGuid,
    ) -> Option<PreparedSpellCast> {
        self.item_cast_profile().map(|profile| {
            PreparedSpellCast::new(
                self.template.id,
                SpellCastSource::Item { item_guid },
                profile,
            )
        })
    }

    pub(in crate::world) fn player_cast_profile(&self) -> Option<SpellCastProfile> {
        let kind = if self.is_auto_repeat_ranged() {
            SpellCastKind::AutoRepeatRanged
        } else if self.has_on_next_swing_attribute() {
            SpellCastKind::NextMeleeSwing
        } else if self.has_effect(SpellEffectDispatch::Charge) {
            SpellCastKind::Charge
        } else if self.has_direct_heal_effect() {
            SpellCastKind::DirectHeal
        } else if self.has_effect(SpellEffectDispatch::CreateItem) {
            SpellCastKind::CreateItem
        } else if self.has_effect(SpellEffectDispatch::ApplyAura) {
            SpellCastKind::AuraApplication
        } else if self.has_direct_damage_effect() {
            SpellCastKind::InstantDamage
        } else {
            return None;
        };
        Some(self.build_cast_profile(kind))
    }

    pub(in crate::world) fn item_cast_profile(&self) -> Option<SpellCastProfile> {
        if self.has_effect(SpellEffectDispatch::Charge) || self.has_on_next_swing_attribute() {
            return None;
        }
        let kind = if self.has_effect(SpellEffectDispatch::ApplyAura) {
            SpellCastKind::AuraApplication
        } else if self.has_effect(SpellEffectDispatch::Teleport) {
            SpellCastKind::Teleport
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
        Some(profile)
    }

    pub(in crate::world) fn build_cast_profile(&self, kind: SpellCastKind) -> SpellCastProfile {
        SpellCastProfile {
            spell_id: self.template.id,
            kind,
            aura_target: self.aura_target(),
            bonus_damage: self.bonus_damage(),
            weapon_damage_percent: self.weapon_damage_percent(),
            damage: if matches!(
                kind,
                SpellCastKind::AutoRepeatRanged
                    | SpellCastKind::NextMeleeSwing
                    | SpellCastKind::Charge
            ) {
                0
            } else {
                self.direct_damage()
            },
            power: self.power(),
            requires_melee: kind == SpellCastKind::NextMeleeSwing
                || (self.template.dmg_class == 2
                    && !matches!(
                        kind,
                        SpellCastKind::AutoRepeatRanged | SpellCastKind::Charge
                    )),
            requires_behind: self.requires_behind_target(),
            needs_combo_points: self.needs_combo_points(),
            global_cooldown_category: self.template.start_recovery_category,
            global_cooldown_millis: self.template.start_recovery_time as u64,
            cooldown_millis: self
                .template
                .recovery_time
                .max(self.template.category_recovery_time) as u64,
        }
    }

    pub(in crate::world) fn has_effect(&self, dispatch: SpellEffectDispatch) -> bool {
        self.effects
            .iter()
            .any(|effect| effect.dispatch == dispatch)
    }

    pub(in crate::world) fn has_on_next_swing_attribute(&self) -> bool {
        (self.template.attributes & (SPELL_ATTR_ON_NEXT_SWING_NO_DAMAGE | SPELL_ATTR_ON_NEXT_SWING))
            != 0
    }

    pub(in crate::world) fn is_auto_repeat_ranged(&self) -> bool {
        (self.template.attributes & SPELL_ATTR_USES_RANGED_SLOT) != 0
            && (self.template.attributes_ex2 & SPELL_ATTR_EX2_AUTO_REPEAT) != 0
    }

    pub(in crate::world) fn has_item_direct_effect(&self) -> bool {
        self.effects.iter().any(|effect| {
            matches!(
                effect.dispatch,
                SpellEffectDispatch::Heal | SpellEffectDispatch::Energize
            ) && spell_effect_simple_value(effect.base_points).is_some()
        })
    }

    pub(in crate::world) fn has_direct_damage_effect(&self) -> bool {
        self.effects.iter().any(|effect| {
            matches!(
                effect.dispatch,
                SpellEffectDispatch::SchoolDamage
                    | SpellEffectDispatch::WeaponDamage
                    | SpellEffectDispatch::WeaponPercentDamage
            ) && spell_effect_simple_value(effect.base_points).is_some()
        })
    }

    pub(in crate::world) fn has_direct_heal_effect(&self) -> bool {
        self.effects.iter().any(|effect| {
            effect.dispatch == SpellEffectDispatch::Heal
                && spell_effect_simple_value(effect.base_points).is_some()
        })
    }

    pub(in crate::world) fn direct_heal(&self) -> u32 {
        self.effects
            .iter()
            .filter(|effect| effect.dispatch == SpellEffectDispatch::Heal)
            .filter_map(|effect| spell_effect_simple_value(effect.base_points))
            .sum()
    }

    pub(in crate::world) fn aura_target(&self) -> SpellAuraTarget {
        if let Some(effect) = self
            .effects
            .iter()
            .find(|effect| effect.dispatch == SpellEffectDispatch::ApplyAura)
        {
            if effect_targets_caster_centered_hostile_area(*effect) {
                return SpellAuraTarget::CasterAreaEnemy;
            }
            return match effect.implicit_target_a {
                TARGET_UNIT_CASTER => SpellAuraTarget::Caster,
                target if is_direct_unit_target(target) => SpellAuraTarget::UnitTarget,
                _ => SpellAuraTarget::Caster,
            };
        }
        self.effects
            .iter()
            .find(|effect| effect.dispatch == SpellEffectDispatch::Heal)
            .map(|effect| match effect.implicit_target_a {
                TARGET_UNIT_CASTER => SpellAuraTarget::Caster,
                target if is_direct_unit_target(target) => SpellAuraTarget::UnitTarget,
                _ => SpellAuraTarget::UnitTarget,
            })
            .unwrap_or(SpellAuraTarget::Caster)
    }

    pub(in crate::world) fn unit_target_kind(&self, kind: SpellCastKind) -> SpellTargetKind {
        match kind {
            SpellCastKind::AutoRepeatRanged
            | SpellCastKind::Charge
            | SpellCastKind::NextMeleeSwing => SpellTargetKind::HostileUnit,
            SpellCastKind::DirectHeal => {
                if self.aura_target() == SpellAuraTarget::Caster {
                    SpellTargetKind::Caster
                } else {
                    SpellTargetKind::FriendlyUnit
                }
            }
            SpellCastKind::AuraApplication => {
                let aura_target = self.aura_target();
                if matches!(
                    aura_target,
                    SpellAuraTarget::Caster | SpellAuraTarget::CasterAreaEnemy
                ) {
                    SpellTargetKind::Caster
                } else if self.effects.iter().any(|effect| {
                    effect_targets_direct_hostile_unit(*effect)
                        || effect.aura_name == SPELL_AURA_PERIODIC_DAMAGE
                }) {
                    SpellTargetKind::HostileUnit
                } else if self
                    .effects
                    .iter()
                    .any(|effect| effect_targets_direct_friendly_unit(*effect))
                {
                    SpellTargetKind::FriendlyUnit
                } else {
                    SpellTargetKind::Unit
                }
            }
            SpellCastKind::InstantDamage => {
                if self.effects.iter().any(|effect| {
                    matches!(
                        effect.dispatch,
                        SpellEffectDispatch::SchoolDamage
                            | SpellEffectDispatch::WeaponDamage
                            | SpellEffectDispatch::WeaponPercentDamage
                    ) && effect.implicit_target_a == TARGET_UNIT_CASTER
                }) {
                    SpellTargetKind::Caster
                } else {
                    SpellTargetKind::HostileUnit
                }
            }
            SpellCastKind::CreateItem | SpellCastKind::Teleport => SpellTargetKind::Caster,
        }
    }

    pub(in crate::world) fn bonus_damage(&self) -> u32 {
        let fixed_bonus: u32 = self
            .effects
            .iter()
            .filter(|effect| effect.dispatch == SpellEffectDispatch::WeaponDamage)
            .filter_map(|effect| spell_effect_simple_value(effect.base_points))
            .sum();
        fixed_bonus.saturating_mul(self.weapon_damage_percent()) / 100
    }

    pub(in crate::world) fn weapon_damage_percent(&self) -> u32 {
        self.effects
            .iter()
            .filter(|effect| effect.dispatch == SpellEffectDispatch::WeaponPercentDamage)
            .filter_map(|effect| spell_effect_simple_value(effect.base_points))
            .fold(100u32, |percent, effect_percent| {
                percent.saturating_mul(effect_percent) / 100
            })
    }

    pub(in crate::world) fn direct_damage(&self) -> u32 {
        if self.has_on_next_swing_attribute() || self.has_effect(SpellEffectDispatch::Charge) {
            return 0;
        }
        let school_damage: u32 = self
            .effects
            .iter()
            .filter(|effect| matches!(effect.dispatch, SpellEffectDispatch::SchoolDamage))
            .filter_map(|effect| spell_effect_simple_value(effect.base_points))
            .sum();
        let weapon_damage = if self.has_effect(SpellEffectDispatch::WeaponDamage) {
            self.bonus_damage()
        } else {
            0
        };
        school_damage.saturating_add(weapon_damage)
    }

    pub(in crate::world) fn requires_behind_target(&self) -> bool {
        (self.template.attributes_serverside & SPELL_ATTR_SS_FACING_BACK) != 0
    }

    pub(in crate::world) fn needs_combo_points(&self) -> bool {
        (self.template.attributes_ex & SPELL_ATTR_EX_FINISHING_MOVE_DAMAGE) != 0
            || (self.template.attributes_ex & SPELL_ATTR_EX_FINISHING_MOVE_DURATION) != 0
    }

    pub(in crate::world) fn power(&self) -> SpellPowerCost {
        match self.template.power_type {
            POWER_TYPE_RAGE => SpellPowerCost::Rage {
                cost: self.template.mana_cost,
            },
            POWER_TYPE_MANA => SpellPowerCost::Mana {
                cost: self.template.mana_cost,
            },
            POWER_TYPE_ENERGY => SpellPowerCost::Energy {
                cost: self.template.mana_cost,
            },
            _ => SpellPowerCost::Mana {
                cost: self.template.mana_cost,
            },
        }
    }
}

impl SpellInfoEffect {
    pub(in crate::world) fn from_template_slot(slot: SpellInfoEffectSlot) -> Self {
        Self {
            effect_id: slot.effect_id,
            aura_name: slot.aura_name,
            base_points: slot.base_points,
            die_sides: slot.roll.0,
            base_dice: slot.roll.1,
            points_per_combo_point: slot.roll.2,
            amplitude: slot.amplitude,
            implicit_target_a: slot.implicit_target_a,
            implicit_target_b: slot.implicit_target_b,
            radius_index: slot.radius_index,
            misc_value: slot.misc_value,
            mechanic: slot.mechanic,
            trigger_spell: slot.trigger_spell,
            item_type: slot.item_type,
            dispatch: SpellEffectDispatch::from_effect_id(slot.effect_id),
        }
    }
}

pub(in crate::world) fn effect_targets_caster_centered_hostile_area(
    effect: SpellInfoEffect,
) -> bool {
    is_caster_centered_hostile_area_target(effect.implicit_target_a)
        || is_caster_centered_hostile_area_target(effect.implicit_target_b)
}

pub(in crate::world) fn effect_targets_direct_hostile_unit(effect: SpellInfoEffect) -> bool {
    is_direct_hostile_unit_target(effect.implicit_target_a)
        || is_direct_hostile_unit_target(effect.implicit_target_b)
}

pub(in crate::world) fn effect_targets_direct_friendly_unit(effect: SpellInfoEffect) -> bool {
    is_direct_friendly_unit_target(effect.implicit_target_a)
        || is_direct_friendly_unit_target(effect.implicit_target_b)
}

pub(in crate::world) fn is_direct_unit_target(target: u32) -> bool {
    is_direct_hostile_unit_target(target)
        || is_direct_friendly_unit_target(target)
        || matches!(target, TARGET_UNIT)
}

pub(in crate::world) fn is_direct_hostile_unit_target(target: u32) -> bool {
    matches!(target, TARGET_UNIT_ENEMY)
}

pub(in crate::world) fn is_direct_friendly_unit_target(target: u32) -> bool {
    matches!(
        target,
        TARGET_UNIT_FRIEND
            | TARGET_UNIT_PARTY
            | TARGET_UNIT_FRIEND_AND_PARTY
            | TARGET_UNIT_FRIEND_CHAIN_HEAL
            | TARGET_UNIT_RAID
            | TARGET_UNIT_RAID_NEAR_CASTER
            | TARGET_UNIT_RAID_AND_CLASS
    )
}

pub(in crate::world) fn is_caster_centered_hostile_area_target(target: u32) -> bool {
    matches!(
        target,
        TARGET_ENUM_UNITS_ENEMY_AOE_AT_SRC_LOC | TARGET_ENUM_UNITS_ENEMY_WITHIN_CASTER_RANGE
    )
}
