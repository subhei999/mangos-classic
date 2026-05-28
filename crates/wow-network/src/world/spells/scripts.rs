use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::world) enum SpellScriptId {
    WarlockEyeOfKilrogg,
    WarlockLifeTap,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::world) enum AuraScriptId {
    WarlockCurseOfAgony,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::world) struct AuraPeriodicAmountContext {
    pub(in crate::world) tick_number: u32,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Copy)]
pub(in crate::world) struct SpellScriptCastContext<'a> {
    pub(in crate::world) spell_template: &'a wow_db::SpellTemplateQuery,
    pub(in crate::world) spell_profile: &'a SpellCastProfile,
    pub(in crate::world) targets: &'a SpellCastTargets,
    pub(in crate::world) active_auras: &'a [ActiveAura],
    pub(in crate::world) caster: ObjectGuid,
    pub(in crate::world) character_guid: u32,
    pub(in crate::world) map_id: u32,
    pub(in crate::world) caster_health: u32,
    pub(in crate::world) caster_mana: u32,
    pub(in crate::world) now: Instant,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Copy)]
pub(in crate::world) struct SpellScriptEffectContext<'a> {
    pub(in crate::world) cast: SpellScriptCastContext<'a>,
    pub(in crate::world) effect_index: usize,
    pub(in crate::world) effect: SpellInfoEffect,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(in crate::world) struct SpellScriptCastOverrides {
    pub(in crate::world) power: Option<SpellPowerCost>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(in crate::world) struct SpellScriptEffectResult {
    pub(in crate::world) handled: bool,
    pub(in crate::world) action: Option<SpellScriptEffectAction>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::world) enum SpellScriptEffectAction {
    LifeTap {
        health_cost: u32,
        mana_spell_id: u32,
        mana_amount: u32,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::world) enum SpellScriptFinishAction {
    ApplyHiddenAuraToOwnedSummonCreatedBySpell {
        summon_spell_id: u32,
        aura_spell_id: u32,
    },
}

#[allow(dead_code)]
#[derive(Debug, Clone, Copy)]
pub(in crate::world) struct AuraScriptDurationContext {
    pub(in crate::world) caster: ObjectGuid,
    pub(in crate::world) level: u8,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Copy)]
pub(in crate::world) struct AuraScriptApplyContext<'a> {
    pub(in crate::world) aura: &'a ActiveAura,
    pub(in crate::world) apply: bool,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Copy)]
pub(in crate::world) struct AuraScriptProcContext {
    pub(in crate::world) proc_flags: u32,
    pub(in crate::world) proc_ex: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::world) enum AuraScriptProcResult {
    Continue,
    PreventDefault,
}

const WARLOCK_CURSE_OF_AGONY_SPELL_IDS: [u32; 6] = [980, 1014, 6217, 11711, 11712, 11713];
const WARLOCK_EYE_OF_KILROGG_SPELL_IDS: [u32; 1] = [126];
const WARLOCK_LIFE_TAP_SPELL_IDS: [u32; 6] = [1454, 1455, 1456, 11687, 11688, 11689];

pub(in crate::world) fn spell_script_for_spell_id(spell_id: u32) -> Option<SpellScriptId> {
    if WARLOCK_EYE_OF_KILROGG_SPELL_IDS.contains(&spell_id) {
        return Some(SpellScriptId::WarlockEyeOfKilrogg);
    }

    if WARLOCK_LIFE_TAP_SPELL_IDS.contains(&spell_id) {
        return Some(SpellScriptId::WarlockLifeTap);
    }

    None
}

pub(in crate::world) fn aura_script_for_spell_id(spell_id: u32) -> Option<AuraScriptId> {
    if WARLOCK_CURSE_OF_AGONY_SPELL_IDS.contains(&spell_id) {
        return Some(AuraScriptId::WarlockCurseOfAgony);
    }

    None
}

#[allow(dead_code)]
pub(in crate::world) fn spell_script_for_name(name: &str) -> Option<SpellScriptId> {
    match name {
        "spell_eye_of_kilrogg" => Some(SpellScriptId::WarlockEyeOfKilrogg),
        "spell_life_tap" => Some(SpellScriptId::WarlockLifeTap),
        _ => None,
    }
}

#[allow(dead_code)]
pub(in crate::world) fn aura_script_for_name(name: &str) -> Option<AuraScriptId> {
    match name {
        "spell_curse_of_agony" => Some(AuraScriptId::WarlockCurseOfAgony),
        _ => None,
    }
}

pub(in crate::world) fn spell_script_handles_effect(spell_id: u32, effect_id: u32) -> bool {
    spell_script_for_spell_id(spell_id).is_some()
        && matches!(effect_id, SPELL_EFFECT_DUMMY | SPELL_EFFECT_SCRIPT_EFFECT)
}

pub(in crate::world) fn spell_script_on_init(
    script: SpellScriptId,
    context: SpellScriptCastContext<'_>,
) -> SpellScriptCastOverrides {
    match script {
        SpellScriptId::WarlockEyeOfKilrogg => {
            let _ = context;
            SpellScriptCastOverrides::default()
        }
        SpellScriptId::WarlockLifeTap => {
            let _ = context;
            SpellScriptCastOverrides::default()
        }
    }
}

pub(in crate::world) fn spell_script_on_successful_start(
    script: SpellScriptId,
    context: SpellScriptCastContext<'_>,
) {
    match script {
        SpellScriptId::WarlockEyeOfKilrogg => {
            let _ = context;
        }
        SpellScriptId::WarlockLifeTap => {
            let _ = context;
        }
    }
}

pub(in crate::world) fn spell_script_on_successful_finish(
    script: SpellScriptId,
    context: SpellScriptCastContext<'_>,
) -> Option<SpellScriptFinishAction> {
    match script {
        SpellScriptId::WarlockEyeOfKilrogg => {
            let _ = context;
            Some(
                SpellScriptFinishAction::ApplyHiddenAuraToOwnedSummonCreatedBySpell {
                    summon_spell_id: 126,
                    aura_spell_id: 2585,
                },
            )
        }
        SpellScriptId::WarlockLifeTap => {
            let _ = context;
            None
        }
    }
}

pub(in crate::world) fn spell_script_on_check_cast(
    script: SpellScriptId,
    context: SpellScriptCastContext<'_>,
    strict: bool,
) -> Option<u8> {
    match script {
        SpellScriptId::WarlockEyeOfKilrogg => {
            let _ = (context, strict);
            None
        }
        SpellScriptId::WarlockLifeTap => life_tap_on_check_cast(context, strict),
    }
}

pub(in crate::world) fn spell_script_on_cast(
    script: SpellScriptId,
    context: SpellScriptCastContext<'_>,
) {
    match script {
        SpellScriptId::WarlockEyeOfKilrogg => {
            let _ = context;
        }
        SpellScriptId::WarlockLifeTap => {
            let _ = context;
        }
    }
}

pub(in crate::world) fn spell_script_on_hit(
    script: SpellScriptId,
    context: SpellScriptCastContext<'_>,
    miss_info: Option<u8>,
) {
    match script {
        SpellScriptId::WarlockEyeOfKilrogg => {
            let _ = (context, miss_info);
        }
        SpellScriptId::WarlockLifeTap => {
            let _ = (context, miss_info);
        }
    }
}

pub(in crate::world) fn spell_script_on_after_hit(
    script: SpellScriptId,
    context: SpellScriptCastContext<'_>,
) {
    match script {
        SpellScriptId::WarlockEyeOfKilrogg => {
            let _ = context;
        }
        SpellScriptId::WarlockLifeTap => {
            let _ = context;
        }
    }
}

pub(in crate::world) fn spell_script_on_effect_execute(
    script: SpellScriptId,
    context: SpellScriptEffectContext<'_>,
) -> SpellScriptEffectResult {
    match script {
        SpellScriptId::WarlockEyeOfKilrogg => {
            let _ = context;
            SpellScriptEffectResult::default()
        }
        SpellScriptId::WarlockLifeTap => life_tap_on_effect_execute(context),
    }
}

pub(in crate::world) fn aura_script_on_holder_init(
    script: AuraScriptId,
    context: AuraScriptApplyContext<'_>,
) {
    match script {
        AuraScriptId::WarlockCurseOfAgony => {
            let _ = context;
        }
    }
}

pub(in crate::world) fn aura_script_on_aura_init(script: AuraScriptId, aura: &ActiveAura) {
    match script {
        AuraScriptId::WarlockCurseOfAgony => {
            let _ = aura;
        }
    }
}

pub(in crate::world) fn aura_script_duration(
    script: AuraScriptId,
    duration_millis: i32,
    context: AuraScriptDurationContext,
) -> i32 {
    match script {
        AuraScriptId::WarlockCurseOfAgony => {
            let _ = context;
            duration_millis
        }
    }
}

pub(in crate::world) fn aura_script_on_apply(
    script: AuraScriptId,
    context: AuraScriptApplyContext<'_>,
) {
    match script {
        AuraScriptId::WarlockCurseOfAgony => {
            let _ = context;
        }
    }
}

pub(in crate::world) fn aura_script_on_after_apply(
    script: AuraScriptId,
    context: AuraScriptApplyContext<'_>,
) {
    match script {
        AuraScriptId::WarlockCurseOfAgony => {
            let _ = context;
        }
    }
}

pub(in crate::world) fn aura_script_periodic_amount(
    script: AuraScriptId,
    amount: u32,
    context: AuraPeriodicAmountContext,
) -> u32 {
    match script {
        AuraScriptId::WarlockCurseOfAgony => curse_of_agony_periodic_amount(amount, context),
    }
}

pub(in crate::world) fn aura_script_on_periodic_trigger(script: AuraScriptId, aura: &ActiveAura) {
    match script {
        AuraScriptId::WarlockCurseOfAgony => {
            let _ = aura;
        }
    }
}

#[cfg(test)]
pub(in crate::world) fn aura_script_on_periodic_dummy(script: AuraScriptId, aura: &ActiveAura) {
    match script {
        AuraScriptId::WarlockCurseOfAgony => {
            let _ = aura;
        }
    }
}

pub(in crate::world) fn aura_script_on_periodic_tick_end(script: AuraScriptId, aura: &ActiveAura) {
    match script {
        AuraScriptId::WarlockCurseOfAgony => {
            let _ = aura;
        }
    }
}

#[cfg(test)]
pub(in crate::world) fn aura_script_on_heartbeat(script: AuraScriptId, aura: &ActiveAura) {
    match script {
        AuraScriptId::WarlockCurseOfAgony => {
            let _ = aura;
        }
    }
}

pub(in crate::world) fn aura_script_on_check_proc(
    script: AuraScriptId,
    aura: &ActiveAura,
    context: AuraScriptProcContext,
) -> bool {
    match script {
        AuraScriptId::WarlockCurseOfAgony => {
            let _ = (aura, context);
            true
        }
    }
}

pub(in crate::world) fn aura_script_on_proc(
    script: AuraScriptId,
    aura: &ActiveAura,
    context: AuraScriptProcContext,
) -> AuraScriptProcResult {
    match script {
        AuraScriptId::WarlockCurseOfAgony => {
            let _ = (aura, context);
            AuraScriptProcResult::Continue
        }
    }
}

fn curse_of_agony_periodic_amount(amount: u32, context: AuraPeriodicAmountContext) -> u32 {
    match context.tick_number {
        1..=4 => amount / 2,
        9..=12 => amount.saturating_add(amount.div_ceil(2)),
        _ => amount,
    }
}

fn life_tap_on_check_cast(context: SpellScriptCastContext<'_>, strict: bool) -> Option<u8> {
    let health_cost = life_tap_health_cost(context.spell_template);
    if health_cost > context.caster_health {
        return Some(SPELL_FAILED_FIZZLE);
    }
    let _ = strict;
    None
}

fn life_tap_on_effect_execute(context: SpellScriptEffectContext<'_>) -> SpellScriptEffectResult {
    let health_cost = life_tap_health_cost(context.cast.spell_template);
    if health_cost == 0 {
        return SpellScriptEffectResult {
            handled: true,
            action: None,
        };
    }
    SpellScriptEffectResult {
        handled: true,
        action: Some(SpellScriptEffectAction::LifeTap {
            health_cost,
            mana_spell_id: 31_818,
            mana_amount: life_tap_mana_amount(health_cost, context.cast.active_auras),
        }),
    }
}

fn life_tap_health_cost(template: &wow_db::SpellTemplateQuery) -> u32 {
    let effect = SpellInfo::from_template(template).effects[0];
    cmangos_simple_effect_value(effect)
}

fn life_tap_mana_amount(health_cost: u32, active_auras: &[ActiveAura]) -> u32 {
    let _ = active_auras;
    health_cost
}

fn cmangos_simple_effect_value(effect: SpellInfoEffect) -> u32 {
    let value = i64::from(effect.base_points) + i64::from(effect.base_dice);
    value.clamp(0, i64::from(u32::MAX)) as u32
}
