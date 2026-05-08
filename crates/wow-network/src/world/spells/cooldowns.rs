#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct SpellCooldownKey {
    spell_id: u32,
    category: u32,
}

async fn apply_item_use_spell_cooldowns(
    maps: &MapRuntimeManager,
    map_id: u32,
    character_guid: u32,
    item_spell: &SpellCastProfile,
    now: Instant,
    skip_spell_cooldown: bool,
) {
    maps.apply_player_spell_cooldowns(
        map_id,
        character_guid,
        item_spell,
        now,
        skip_spell_cooldown,
    )
    .await;
}

async fn item_use_spell_failure(
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
        return maps
            .player_spell_cast_failure(map_id, character_guid, &gcd_only, now)
            .await;
    }
    maps.player_spell_cast_failure(map_id, character_guid, item_spell, now)
        .await
}
