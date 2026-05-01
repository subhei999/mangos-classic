// CMaNGOS reference: src/game/Handlers/ItemHandler.cpp item query packet builders.

fn build_item_query_single_response(
    item: u32,

    template: Option<&wow_db::ItemTemplateQuery>,
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

    for _ in 0..10 {
        write_u32(&mut body, 0);

        write_u32(&mut body, 0);
    }

    for _ in 0..5 {
        write_f32(&mut body, 0.0);

        write_f32(&mut body, 0.0);

        write_u32(&mut body, 0);
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

    for _ in 0..5 {
        write_u32(&mut body, 0);

        write_u32(&mut body, 0);

        write_u32(&mut body, 0);

        write_u32(&mut body, u32::MAX);

        write_u32(&mut body, 0);

        write_u32(&mut body, u32::MAX);
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

fn item_query_subclass(template: &wow_db::ItemTemplateQuery) -> u32 {
    if template.class == 0 {
        0
    } else {
        template.subclass
    }
}
