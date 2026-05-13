use super::*;

// CMaNGOS reference: src/game/Handlers/ItemHandler.cpp item query packet builders.

#[cfg(test)]
pub(in crate::world) fn build_item_query_single_response(
    item: u32,

    template: Option<&wow_db::ItemTemplateQuery>,
) -> Vec<u8> {
    build_item_query_single_response_with_spell_cooldowns(item, template, None)
}

#[derive(Clone, Copy)]
pub(in crate::world) struct ItemQuerySpellCooldown {
    pub(in crate::world) recovery_time: i32,
    pub(in crate::world) category: u32,
    pub(in crate::world) category_recovery_time: i32,
}

pub(in crate::world) fn build_item_query_single_response_with_spell_cooldowns(
    item: u32,

    template: Option<&wow_db::ItemTemplateQuery>,

    spell_cooldowns: Option<&[Option<ItemQuerySpellCooldown>; 5]>,
) -> Vec<u8> {
    let Some(template) = template else {
        return (item | 0x8000_0000).to_le_bytes().to_vec();
    };

    let mut body = Vec::with_capacity(600);

    write_u32(&mut body, template.entry);

    write_u32(&mut body, template.class);

    write_u32(&mut body, item_query_subclass(template));

    write_c_string(&mut body, &template.name);

    body.push(0);

    body.push(0);

    body.push(0);

    write_u32(&mut body, template.displayid);

    write_u32(&mut body, template.quality);

    write_u32(&mut body, template.flags);

    write_u32(&mut body, template.buy_price);

    write_u32(&mut body, template.sell_price);

    write_u32(&mut body, template.inventory_type);

    write_i32(&mut body, template.allowable_class);

    write_i32(&mut body, template.allowable_race);

    write_u32(&mut body, template.item_level);

    write_u32(&mut body, template.required_level);

    write_u32(&mut body, template.required_skill);

    write_u32(&mut body, template.required_skill_rank);

    write_u32(&mut body, template.required_spell);

    write_u32(&mut body, template.required_honor_rank);

    write_u32(&mut body, template.required_city_rank);

    write_u32(&mut body, template.required_reputation_faction);

    write_u32(
        &mut body,
        if template.required_reputation_faction > 0 {
            template.required_reputation_rank
        } else {
            0
        },
    );

    write_u32(&mut body, template.max_count);

    write_u32(&mut body, template.stackable);

    write_u32(&mut body, template.container_slots);

    for stat in template.stats {
        write_u32(&mut body, stat.stat_type);

        write_i32(&mut body, stat.stat_value);
    }

    for damage in template.damage {
        write_f32(&mut body, damage.damage_min);

        write_f32(&mut body, damage.damage_max);

        write_u32(&mut body, damage.damage_type);
    }

    write_u32(&mut body, template.armor);

    write_u32(&mut body, template.holy_res);

    write_u32(&mut body, template.fire_res);

    write_u32(&mut body, template.nature_res);

    write_u32(&mut body, template.frost_res);

    write_u32(&mut body, template.shadow_res);

    write_u32(&mut body, template.arcane_res);

    write_u32(&mut body, template.delay);

    write_u32(&mut body, template.ammo_type);

    write_f32(&mut body, template.ranged_mod_range);

    for (index, spell) in template.spells.into_iter().enumerate() {
        let cooldown = item_query_spell_cooldown(spell, spell_cooldowns.and_then(|c| c[index]));
        write_u32(&mut body, spell.spell_id);

        write_u32(&mut body, spell.spell_trigger);

        write_i32(&mut body, spell.spell_charges);

        write_i32(&mut body, cooldown.recovery_time);

        write_u32(&mut body, cooldown.category);

        write_i32(&mut body, cooldown.category_recovery_time);
    }

    write_u32(&mut body, template.bonding);

    write_c_string(&mut body, &template.description);

    write_u32(&mut body, template.page_text);

    write_u32(&mut body, template.language_id);

    write_u32(&mut body, template.page_material);

    write_u32(&mut body, template.start_quest);

    write_u32(&mut body, template.lock_id);

    write_i32(&mut body, template.material);

    write_u32(&mut body, template.sheath);

    write_u32(&mut body, template.random_property);

    write_u32(&mut body, template.block);

    write_u32(&mut body, template.itemset);

    write_u32(&mut body, template.max_durability);

    write_u32(&mut body, template.area);

    write_i32(&mut body, template.map);

    write_i32(&mut body, template.bag_family);

    body
}

pub(in crate::world) fn item_query_spell_cooldown(
    spell: wow_db::ItemTemplateSpell,
    spell_template_cooldown: Option<ItemQuerySpellCooldown>,
) -> ItemQuerySpellCooldown {
    if spell.spell_cooldown >= 0 || spell.spell_category_cooldown >= 0 {
        ItemQuerySpellCooldown {
            recovery_time: spell.spell_cooldown,
            category: spell.spell_category,
            category_recovery_time: spell.spell_category_cooldown,
        }
    } else {
        spell_template_cooldown.unwrap_or(ItemQuerySpellCooldown {
            recovery_time: spell.spell_cooldown,
            category: spell.spell_category,
            category_recovery_time: spell.spell_category_cooldown,
        })
    }
}

pub(in crate::world) fn build_item_name_query_response(
    template: &wow_db::ItemTemplateQuery,
) -> Vec<u8> {
    let mut body = Vec::with_capacity(9 + template.name.len());
    write_u32(&mut body, template.entry);
    write_c_string(&mut body, &template.name);
    write_u32(&mut body, template.inventory_type);
    body
}

pub(in crate::world) fn item_query_subclass(template: &wow_db::ItemTemplateQuery) -> u32 {
    if template.class == 0 {
        0
    } else {
        template.subclass
    }
}
