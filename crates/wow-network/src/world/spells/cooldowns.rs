use super::*;

#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::world) struct SpellCooldownKey {
    pub(in crate::world) spell_id: u32,
    pub(in crate::world) category: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::world) struct ItemSpellCooldown {
    pub(in crate::world) recovery_millis: u64,
    pub(in crate::world) category: u32,
    pub(in crate::world) category_recovery_millis: u64,
}

pub(in crate::world) fn item_spell_cooldown(
    item_spell: wow_db::ItemTemplateSpell,
    spell_template: &wow_db::SpellTemplateQuery,
) -> ItemSpellCooldown {
    let recovery_millis = if item_spell.spell_cooldown >= 0 {
        item_spell.spell_cooldown as u64
    } else {
        spell_template.recovery_time as u64
    };
    let category = if item_spell.spell_category != 0 {
        item_spell.spell_category
    } else {
        spell_template.category
    };
    let category_recovery_millis = if item_spell.spell_category_cooldown >= 0 {
        item_spell.spell_category_cooldown as u64
    } else {
        spell_template.category_recovery_time as u64
    };

    ItemSpellCooldown {
        recovery_millis,
        category,
        category_recovery_millis,
    }
}

pub(in crate::world) fn item_spell_cast_profile_with_cooldown(
    mut profile: SpellCastProfile,
    item_spell: wow_db::ItemTemplateSpell,
    spell_template: &wow_db::SpellTemplateQuery,
) -> (SpellCastProfile, ItemSpellCooldown) {
    let cooldown = item_spell_cooldown(item_spell, spell_template);
    profile.cooldown_millis = cooldown.recovery_millis;
    profile.cooldown_category = cooldown.category;
    profile.category_cooldown_millis = cooldown.category_recovery_millis;
    (profile, cooldown)
}

#[allow(clippy::too_many_arguments)]
pub(in crate::world) async fn apply_item_use_spell_cooldowns(
    maps: &MapRuntimeManager,
    map_id: u32,
    character_guid: u32,
    item_id: u32,
    item_spell: &SpellCastProfile,
    now: Instant,
    skip_spell_cooldown: bool,
    category: u32,
    category_cooldown_millis: u64,
) {
    maps.apply_player_item_spell_cooldowns(
        map_id,
        character_guid,
        item_spell,
        now,
        skip_spell_cooldown,
        item_id,
        category,
        category_cooldown_millis,
    )
    .await;
}

pub(in crate::world) async fn item_use_spell_failure(
    maps: &MapRuntimeManager,
    map_id: u32,
    character_guid: u32,
    item_spell: &SpellCastProfile,
    now: Instant,
    ignore_spell_cooldown: bool,
) -> Option<u8> {
    let refreshing_active_aura = item_spell.kind == SpellCastKind::AuraApplication
        && maps
            .player_runtime_snapshot(map_id, character_guid)
            .await
            .is_some_and(|snapshot| {
                snapshot
                    .active_auras
                    .iter()
                    .any(|aura| aura.spell_id == item_spell.spell_id)
            });
    if refreshing_active_aura {
        return None;
    }
    if ignore_spell_cooldown {
        let mut gcd_only = *item_spell;
        gcd_only.cooldown_millis = 0;
        gcd_only.cooldown_category = 0;
        gcd_only.category_cooldown_millis = 0;
        return maps
            .player_spell_cast_failure(map_id, character_guid, &gcd_only, now)
            .await;
    }
    maps.player_spell_cast_failure(map_id, character_guid, item_spell, now)
        .await
}
