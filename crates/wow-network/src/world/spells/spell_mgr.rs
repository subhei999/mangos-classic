#[derive(Debug, Clone, Copy)]
struct SpellInfo<'a> {
    template: &'a wow_db::SpellTemplateQuery,
    effects: [SpellInfoEffect; 3],
}

#[derive(Debug, Clone, Copy)]
#[allow(dead_code)]
struct SpellInfoEffect {
    effect_id: u32,
    aura_name: u32,
    base_points: i32,
    amplitude: u32,
    implicit_target_a: u32,
    misc_value: i32,
    dispatch: SpellEffectDispatch,
}

impl<'a> SpellInfo<'a> {
    fn from_template(template: &'a wow_db::SpellTemplateQuery) -> Self {
        let effects = [
            SpellInfoEffect::from_template_slot(
                template.effect1,
                template.effect_apply_aura_name1,
                template.effect_base_points1,
                template.effect_amplitude1,
                template.effect_implicit_target_a1,
                template.effect_misc_value1,
            ),
            SpellInfoEffect::from_template_slot(
                template.effect2,
                template.effect_apply_aura_name2,
                template.effect_base_points2,
                template.effect_amplitude2,
                template.effect_implicit_target_a2,
                template.effect_misc_value2,
            ),
            SpellInfoEffect::from_template_slot(
                template.effect3,
                template.effect_apply_aura_name3,
                template.effect_base_points3,
                template.effect_amplitude3,
                template.effect_implicit_target_a3,
                template.effect_misc_value3,
            ),
        ];
        Self { template, effects }
    }

    fn prepare_player_cast(&self) -> Option<PreparedSpellCast> {
        self.player_cast_profile()
            .map(|profile| PreparedSpellCast::new(self.template.id, SpellCastSource::Player, profile))
    }

    fn prepare_item_cast(&self, item_guid: ObjectGuid) -> Option<PreparedSpellCast> {
        self.item_cast_profile().map(|profile| {
            PreparedSpellCast::new(self.template.id, SpellCastSource::Item { item_guid }, profile)
        })
    }

    fn player_cast_profile(&self) -> Option<SpellCastProfile> {
        let kind = if self.has_on_next_swing_attribute() {
            SpellCastKind::NextMeleeSwing
        } else if self.has_effect(SpellEffectDispatch::Charge) {
            SpellCastKind::Charge
        } else if self.has_direct_heal_effect() {
            SpellCastKind::DirectHeal
        } else if self.has_effect(SpellEffectDispatch::ApplyAura) {
            SpellCastKind::AuraApplication
        } else if self.has_direct_damage_effect() {
            SpellCastKind::InstantDamage
        } else {
            return None;
        };
        Some(self.build_cast_profile(kind))
    }

    fn item_cast_profile(&self) -> Option<SpellCastProfile> {
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

    fn build_cast_profile(&self, kind: SpellCastKind) -> SpellCastProfile {
        SpellCastProfile {
            spell_id: self.template.id,
            kind,
            aura_target: self.aura_target(),
            bonus_damage: self.bonus_damage(),
            damage: if matches!(
                kind,
                SpellCastKind::NextMeleeSwing | SpellCastKind::Charge
            ) {
                0
            } else {
                self.direct_damage()
            },
            power: self.power(),
            requires_melee: kind == SpellCastKind::NextMeleeSwing
                || (self.template.dmg_class == 2 && kind != SpellCastKind::Charge),
            global_cooldown_category: self.template.start_recovery_category,
            global_cooldown_millis: self.template.start_recovery_time as u64,
            cooldown_millis: self
                .template
                .recovery_time
                .max(self.template.category_recovery_time) as u64,
        }
    }

    fn has_effect(&self, dispatch: SpellEffectDispatch) -> bool {
        self.effects
            .iter()
            .any(|effect| effect.dispatch == dispatch)
    }

    fn has_on_next_swing_attribute(&self) -> bool {
        (self.template.attributes & (SPELL_ATTR_ON_NEXT_SWING_NO_DAMAGE | SPELL_ATTR_ON_NEXT_SWING))
            != 0
    }

    fn has_item_direct_effect(&self) -> bool {
        self.effects.iter().any(|effect| {
            matches!(
                effect.dispatch,
                SpellEffectDispatch::Heal | SpellEffectDispatch::Energize
            ) && spell_effect_simple_value(effect.base_points).is_some()
        })
    }

    fn has_direct_damage_effect(&self) -> bool {
        self.effects.iter().any(|effect| {
            matches!(
                effect.dispatch,
                SpellEffectDispatch::SchoolDamage | SpellEffectDispatch::WeaponDamage
            ) && spell_effect_simple_value(effect.base_points).is_some()
        })
    }

    fn has_direct_heal_effect(&self) -> bool {
        self.effects.iter().any(|effect| {
            effect.dispatch == SpellEffectDispatch::Heal
                && spell_effect_simple_value(effect.base_points).is_some()
        })
    }

    fn aura_target(&self) -> SpellAuraTarget {
        if let Some(effect) = self
            .effects
            .iter()
            .find(|effect| effect.dispatch == SpellEffectDispatch::ApplyAura)
        {
            return match effect.implicit_target_a {
                TARGET_UNIT_CASTER => SpellAuraTarget::Caster,
                TARGET_UNIT_ENEMY | TARGET_UNIT => SpellAuraTarget::UnitTarget,
                _ => SpellAuraTarget::Caster,
            };
        }
        self.effects
            .iter()
            .find(|effect| effect.dispatch == SpellEffectDispatch::Heal)
            .map(|effect| match effect.implicit_target_a {
                TARGET_UNIT_CASTER => SpellAuraTarget::Caster,
                TARGET_UNIT => SpellAuraTarget::UnitTarget,
                _ => SpellAuraTarget::UnitTarget,
            })
            .unwrap_or(SpellAuraTarget::Caster)
    }

    fn unit_target_kind(&self, kind: SpellCastKind) -> SpellTargetKind {
        match kind {
            SpellCastKind::Charge | SpellCastKind::NextMeleeSwing => SpellTargetKind::HostileUnit,
            SpellCastKind::DirectHeal => {
                if self.aura_target() == SpellAuraTarget::Caster {
                    SpellTargetKind::Caster
                } else {
                    SpellTargetKind::FriendlyUnit
                }
            }
            SpellCastKind::AuraApplication => {
                if self.aura_target() == SpellAuraTarget::Caster {
                    SpellTargetKind::Caster
                } else if self.effects.iter().any(|effect| {
                    effect.implicit_target_a == TARGET_UNIT_ENEMY
                        || effect.aura_name == SPELL_AURA_PERIODIC_DAMAGE
                }) {
                    SpellTargetKind::HostileUnit
                } else {
                    SpellTargetKind::Unit
                }
            }
            SpellCastKind::InstantDamage => {
                if self.effects.iter().any(|effect| {
                    matches!(
                        effect.dispatch,
                        SpellEffectDispatch::SchoolDamage | SpellEffectDispatch::WeaponDamage
                    ) && effect.implicit_target_a == TARGET_UNIT_CASTER
                }) {
                    SpellTargetKind::Caster
                } else {
                    SpellTargetKind::HostileUnit
                }
            }
            SpellCastKind::Teleport => SpellTargetKind::Caster,
        }
    }

    fn bonus_damage(&self) -> u32 {
        self.effects
            .iter()
            .filter(|effect| effect.dispatch == SpellEffectDispatch::WeaponDamage)
            .filter_map(|effect| spell_effect_simple_value(effect.base_points))
            .max()
            .unwrap_or(0)
    }

    fn direct_damage(&self) -> u32 {
        if self.has_on_next_swing_attribute() || self.has_effect(SpellEffectDispatch::Charge) {
            return 0;
        }
        self.effects
            .iter()
            .filter(|effect| {
                matches!(
                    effect.dispatch,
                    SpellEffectDispatch::SchoolDamage | SpellEffectDispatch::WeaponDamage
                )
            })
            .filter_map(|effect| spell_effect_simple_value(effect.base_points))
            .sum()
    }

    fn power(&self) -> SpellPowerCost {
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
    fn from_template_slot(
        effect_id: u32,
        aura_name: u32,
        base_points: i32,
        amplitude: u32,
        implicit_target_a: u32,
        misc_value: i32,
    ) -> Self {
        Self {
            effect_id,
            aura_name,
            base_points,
            amplitude,
            implicit_target_a,
            misc_value,
            dispatch: SpellEffectDispatch::from_effect_id(effect_id),
        }
    }
}
