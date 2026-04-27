fn build_attack_start_body(attacker: ObjectGuid, victim: ObjectGuid) -> Vec<u8> {
    let mut body = Vec::with_capacity(16);
    body.extend_from_slice(&attacker.raw().to_le_bytes());
    body.extend_from_slice(&victim.raw().to_le_bytes());
    body
}

fn build_questgiver_status_body(guid: ObjectGuid, status: u32) -> Vec<u8> {
    let mut body = Vec::with_capacity(12);
    body.extend_from_slice(&guid.raw().to_le_bytes());
    body.extend_from_slice(&status.to_le_bytes());
    body
}

fn build_questgiver_quest_list_body(guid: ObjectGuid, quests: &[QuestTemplateQuery]) -> Vec<u8> {
    let mut body = Vec::with_capacity(64 + quests.len() * 24);
    body.extend_from_slice(&guid.raw().to_le_bytes());
    push_cstring(&mut body, "Greetings.");
    body.extend_from_slice(&0u32.to_le_bytes()); // player emote delay
    body.extend_from_slice(&0u32.to_le_bytes()); // NPC emote
    body.push(quests.len().min(u8::MAX as usize) as u8);
    for quest in quests.iter().take(u8::MAX as usize) {
        body.extend_from_slice(&quest.entry.to_le_bytes());
        body.extend_from_slice(&2u32.to_le_bytes()); // yellow exclamation mark
        body.extend_from_slice(&quest.quest_level.to_le_bytes());
        push_cstring(&mut body, &quest.title);
    }
    body
}

fn build_quest_details_body(guid: ObjectGuid, quest: &QuestTemplateQuery) -> Vec<u8> {
    let mut body = Vec::with_capacity(256);
    body.extend_from_slice(&guid.raw().to_le_bytes());
    body.extend_from_slice(&quest.entry.to_le_bytes());
    push_cstring(&mut body, &quest.title);
    push_cstring(&mut body, &quest.details);
    push_cstring(&mut body, &quest.objectives);
    body.extend_from_slice(&1u32.to_le_bytes()); // activate accept
    write_quest_reward_items(&mut body, &quest.rew_choice_item_id, &quest.rew_choice_item_count);
    write_quest_reward_items(&mut body, &quest.rew_item_id, &quest.rew_item_count);
    body.extend_from_slice(&(quest.rew_or_req_money.max(0) as u32).to_le_bytes());
    body.extend_from_slice(&quest.rew_spell.to_le_bytes());
    let emote_count = quest
        .details_emote
        .iter()
        .take_while(|emote| **emote != 0)
        .count();
    body.extend_from_slice(&(emote_count as u32).to_le_bytes());
    for index in 0..emote_count {
        body.extend_from_slice(&quest.details_emote[index].to_le_bytes());
        body.extend_from_slice(&quest.details_emote_delay[index].to_le_bytes());
    }
    body
}

fn build_quest_query_response_body(quest: &QuestTemplateQuery) -> Vec<u8> {
    let mut body = Vec::with_capacity(512);
    body.extend_from_slice(&quest.entry.to_le_bytes());
    body.extend_from_slice(&quest.method.to_le_bytes());
    body.extend_from_slice(&quest.quest_level.to_le_bytes());
    body.extend_from_slice(&(quest.zone_or_sort as i32 as u32).to_le_bytes());
    body.extend_from_slice(&quest.quest_type.to_le_bytes());
    body.extend_from_slice(&quest.rep_objective_faction.to_le_bytes());
    body.extend_from_slice(&(quest.rep_objective_value as u32).to_le_bytes());
    body.extend_from_slice(&0u32.to_le_bytes());
    body.extend_from_slice(&0u32.to_le_bytes());
    body.extend_from_slice(&quest.next_quest_in_chain.to_le_bytes());
    body.extend_from_slice(&(quest.rew_or_req_money.max(0) as u32).to_le_bytes());
    body.extend_from_slice(&quest.rew_money_max_level.to_le_bytes());
    body.extend_from_slice(&quest.rew_spell.to_le_bytes());
    body.extend_from_slice(&quest.src_item_id.to_le_bytes());
    body.extend_from_slice(&quest.quest_flags.to_le_bytes());
    for index in 0..4 {
        body.extend_from_slice(&quest.rew_item_id[index].to_le_bytes());
        body.extend_from_slice(&quest.rew_item_count[index].to_le_bytes());
    }
    for index in 0..6 {
        body.extend_from_slice(&quest.rew_choice_item_id[index].to_le_bytes());
        body.extend_from_slice(&quest.rew_choice_item_count[index].to_le_bytes());
    }
    body.extend_from_slice(&quest.point_map_id.to_le_bytes());
    body.extend_from_slice(&quest.point_x.to_le_bytes());
    body.extend_from_slice(&quest.point_y.to_le_bytes());
    body.extend_from_slice(&quest.point_opt.to_le_bytes());
    push_cstring(&mut body, &quest.title);
    push_cstring(&mut body, &quest.objectives);
    push_cstring(&mut body, &quest.details);
    push_cstring(&mut body, &quest.end_text);
    for index in 0..4 {
        let entry = quest.req_creature_or_go_id[index];
        let wire_entry = if entry < 0 {
            ((-entry) as u32) | 0x8000_0000
        } else {
            entry as u32
        };
        body.extend_from_slice(&wire_entry.to_le_bytes());
        body.extend_from_slice(&quest.req_creature_or_go_count[index].to_le_bytes());
        body.extend_from_slice(&quest.req_item_id[index].to_le_bytes());
        body.extend_from_slice(&quest.req_item_count[index].to_le_bytes());
    }
    for text in &quest.objective_text {
        push_cstring(&mut body, text);
    }
    body
}

fn build_quest_request_items_body(
    guid: ObjectGuid,
    quest: &QuestTemplateQuery,
    complete: bool,
) -> Vec<u8> {
    let mut body = Vec::with_capacity(128);
    body.extend_from_slice(&guid.raw().to_le_bytes());
    body.extend_from_slice(&quest.entry.to_le_bytes());
    push_cstring(&mut body, &quest.title);
    push_cstring(&mut body, &quest.request_items_text);
    let (delay, emote) = if complete {
        (quest.complete_emote_delay, quest.complete_emote)
    } else {
        (quest.incomplete_emote_delay, quest.incomplete_emote)
    };
    body.extend_from_slice(&delay.to_le_bytes());
    body.extend_from_slice(&emote.to_le_bytes());
    body.extend_from_slice(&0u32.to_le_bytes()); // close on cancel
    body.extend_from_slice(&0u32.to_le_bytes()); // required money
    body.extend_from_slice(&0u32.to_le_bytes()); // required item count
    body.extend_from_slice(&2u32.to_le_bytes());
    body.extend_from_slice(&(if complete { 3u32 } else { 0u32 }).to_le_bytes());
    body.extend_from_slice(&4u32.to_le_bytes());
    body.extend_from_slice(&8u32.to_le_bytes());
    body
}

fn build_quest_offer_reward_body(guid: ObjectGuid, quest: &QuestTemplateQuery) -> Vec<u8> {
    let mut body = Vec::with_capacity(192);
    body.extend_from_slice(&guid.raw().to_le_bytes());
    body.extend_from_slice(&quest.entry.to_le_bytes());
    push_cstring(&mut body, &quest.title);
    push_cstring(&mut body, &quest.offer_reward_text);
    body.extend_from_slice(&1u32.to_le_bytes()); // enable next
    let emote_count = quest
        .offer_reward_emote
        .iter()
        .take_while(|emote| **emote != 0)
        .count();
    body.extend_from_slice(&(emote_count as u32).to_le_bytes());
    for index in 0..emote_count {
        body.extend_from_slice(&quest.offer_reward_emote_delay[index].to_le_bytes());
        body.extend_from_slice(&quest.offer_reward_emote[index].to_le_bytes());
    }
    write_quest_reward_items(&mut body, &quest.rew_choice_item_id, &quest.rew_choice_item_count);
    write_quest_reward_items(&mut body, &quest.rew_item_id, &quest.rew_item_count);
    body.extend_from_slice(&(quest.rew_or_req_money.max(0) as u32).to_le_bytes());
    body.extend_from_slice(&quest.rew_spell.to_le_bytes());
    body.extend_from_slice(&quest.rew_spell_cast.to_le_bytes());
    body
}

fn build_questgiver_quest_complete_body_with_xp(
    quest: u32,
    reward_xp: u32,
    reward_money: u32,
) -> Vec<u8> {
    let mut body = Vec::with_capacity(16);
    body.extend_from_slice(&quest.to_le_bytes());
    body.extend_from_slice(&reward_xp.to_le_bytes());
    body.extend_from_slice(&0u32.to_le_bytes());
    body.extend_from_slice(&reward_money.to_le_bytes());
    body
}

fn build_quest_update_add_kill_body(
    quest: &QuestTemplateQuery,
    killed_guid: ObjectGuid,
    objective_index: usize,
    count: u32,
) -> Vec<u8> {
    let mut body = Vec::with_capacity(24);
    body.extend_from_slice(&quest.entry.to_le_bytes());
    body.extend_from_slice(&(quest.req_creature_or_go_id[objective_index] as u32).to_le_bytes());
    body.extend_from_slice(&count.to_le_bytes());
    body.extend_from_slice(&quest.req_creature_or_go_count[objective_index].to_le_bytes());
    body.extend_from_slice(&killed_guid.raw().to_le_bytes());
    body
}

fn build_player_quest_log_update_body(
    character_guid: u32,
    slot: usize,
    status: &CharacterQuestStatus,
) -> anyhow::Result<Vec<u8>> {
    let player = ObjectGuid::new(HighGuid::Player, 0, character_guid);
    let mut block = Vec::new();
    block.push(UPDATE_TYPE_VALUES);
    PackedGuid::write(&mut block, player)?;
    let mut values = vec![None; PLAYER_END_FIELDS];
    let base = PLAYER_QUEST_LOG_1_1 + slot * MAX_QUEST_OFFSET;
    set_update_value(&mut values, base + QUEST_LOG_QUEST_ID_OFFSET, status.quest)?;
    set_update_value(
        &mut values,
        base + QUEST_LOG_COUNT_STATE_OFFSET,
        quest_log_count_state(status),
    )?;
    set_update_value(&mut values, base + QUEST_LOG_TIME_OFFSET, 0)?;
    write_update_values(&mut block, &values)?;
    Ok(build_update_object_body(&[block]))
}

fn build_player_quest_log_clear_body(character_guid: u32, slot: usize) -> anyhow::Result<Vec<u8>> {
    let player = ObjectGuid::new(HighGuid::Player, 0, character_guid);
    let mut block = Vec::new();
    block.push(UPDATE_TYPE_VALUES);
    PackedGuid::write(&mut block, player)?;
    let mut values = vec![None; PLAYER_END_FIELDS];
    let base = PLAYER_QUEST_LOG_1_1 + slot * MAX_QUEST_OFFSET;
    set_update_value(&mut values, base + QUEST_LOG_QUEST_ID_OFFSET, 0)?;
    set_update_value(&mut values, base + QUEST_LOG_COUNT_STATE_OFFSET, 0)?;
    set_update_value(&mut values, base + QUEST_LOG_TIME_OFFSET, 0)?;
    write_update_values(&mut block, &values)?;
    Ok(build_update_object_body(&[block]))
}

fn quest_log_count_state(status: &CharacterQuestStatus) -> u32 {
    let count = status.mobcount1 & 0x3F;
    let complete = if status.status == QUEST_STATUS_COMPLETE {
        QUEST_STATE_COMPLETE << 24
    } else {
        0
    };
    count | complete
}

fn write_quest_reward_items<const N: usize>(body: &mut Vec<u8>, ids: &[u32; N], counts: &[u32; N]) {
    let non_zero: Vec<_> = ids
        .iter()
        .zip(counts.iter())
        .filter(|(id, count)| **id != 0 && **count != 0)
        .collect();
    body.extend_from_slice(&(non_zero.len() as u32).to_le_bytes());
    for (id, count) in non_zero {
        body.extend_from_slice(&id.to_le_bytes());
        body.extend_from_slice(&(*count).to_le_bytes());
        body.extend_from_slice(&0u32.to_le_bytes());
    }
}

fn push_cstring(body: &mut Vec<u8>, value: &str) {
    body.extend_from_slice(value.as_bytes());
    body.push(0);
}

fn build_attack_stop_body(
    attacker: ObjectGuid,
    victim: ObjectGuid,
    dead: bool,
) -> anyhow::Result<Vec<u8>> {
    let mut body = Vec::with_capacity(20);
    PackedGuid::write(&mut body, attacker)?;
    PackedGuid::write(&mut body, victim)?;
    body.extend_from_slice(&(dead as u32).to_le_bytes());
    Ok(body)
}

fn build_combat_dummy_loot_response_body(session: &WorldSessionState) -> Vec<u8> {
    let item_count = u8::from(session.combat_dummy_loot_item_available);
    let mut body = Vec::with_capacity(14 + item_count as usize * 22);
    body.extend_from_slice(&rust_combat_dummy_guid().raw().to_le_bytes());
    body.push(CLIENT_LOOT_CORPSE);
    body.extend_from_slice(
        &(if session.combat_dummy_loot_money_available {
            RUST_COMBAT_DUMMY_LOOT_MONEY
        } else {
            0
        })
        .to_le_bytes(),
    );
    body.push(item_count);
    if session.combat_dummy_loot_item_available {
        body.push(0); // loot slot
        body.extend_from_slice(&RUST_COMBAT_DUMMY_LOOT_ITEM.to_le_bytes());
        body.extend_from_slice(&RUST_COMBAT_DUMMY_LOOT_ITEM_COUNT.to_le_bytes());
        body.extend_from_slice(&RUST_COMBAT_DUMMY_LOOT_ITEM_DISPLAY.to_le_bytes());
        body.extend_from_slice(&0u32.to_le_bytes()); // random suffix factor
        body.extend_from_slice(&0u32.to_le_bytes()); // random property id
        body.push(LOOT_SLOT_NORMAL);
    }
    body
}

impl From<CreatureLootQuery> for DbCreatureLootRuntime {
    fn from(loot: CreatureLootQuery) -> Self {
        Self {
            item: loot.item,
            count: loot.max_count.max(loot.min_count).max(1),
            display_id: loot.display_id,
        }
    }
}

struct LootAutostoreContext<'a> {
    stream: &'a mut TcpStream,
    character_db_pool: &'a MySqlPool,
    world_db_pool: &'a MySqlPool,
    session: &'a mut WorldSessionState,
    header_crypto: &'a mut HeaderCrypto,
    character_guid: u32,
}

async fn autostore_loot_item(
    context: LootAutostoreContext<'_>,
    creature_guid: u64,
    loot: DbCreatureLootRuntime,
    loot_slot: u8,
) -> anyhow::Result<()> {
    let LootAutostoreContext {
        stream,
        character_db_pool,
        world_db_pool,
        session,
        header_crypto,
        character_guid,
    } = context;
    let max_stack = wow_db::get_item_template_query(world_db_pool, loot.item)
        .await?
        .map(|template| template.stackable.max(1))
        .unwrap_or(1);
    let mut remaining_count = loot.count;
    let mut update_blocks = Vec::new();

    if max_stack > 1 {
        if let Some(existing_stack) = session
            .inventory
            .iter()
            .filter(|item| {
                item.item_template == loot.item
                    && item.count < max_stack
                    && remaining_count <= max_stack - item.count
                    && u8::try_from(item.bag)
                        .ok()
                        .is_some_and(|bag| is_supported_storage_position(bag, item.slot))
            })
            .min_by_key(|item| {
                let bag_order = if item.bag == INVENTORY_SLOT_BAG_0 as u32 {
                    0
                } else {
                    1
                };
                (bag_order, item.bag, item.slot)
            })
            .cloned()
        {
            let merged_count = existing_stack.count + remaining_count;
            if wow_db::update_character_inventory_item_count(
                character_db_pool,
                character_guid,
                existing_stack.item,
                merged_count,
            )
            .await?
            {
                remaining_count = 0;
                update_blocks.push(build_item_stack_count_update_block(
                    existing_stack.item,
                    merged_count,
                )?);
            }
        }
    }

    if remaining_count > 0 {
        let Some(dst_slot) = first_empty_backpack_slot(&session.inventory) else {
            send_inventory_change_failure(
                stream,
                EQUIP_ERR_COULDNT_SPLIT_ITEMS,
                None,
                None,
                header_crypto,
            )
            .await?;
            return Ok(());
        };

        wow_db::add_character_inventory_item(
            character_db_pool,
            character_guid,
            INVENTORY_SLOT_BAG_0 as u32,
            dst_slot,
            loot.item,
            remaining_count,
            0,
        )
        .await?;
        session.inventory =
            wow_db::get_character_inventory_items(character_db_pool, character_guid).await?;
        if let Some(new_item) = session.inventory.iter().find(|item| {
            item.bag == INVENTORY_SLOT_BAG_0 as u32
                && item.slot == dst_slot
                && item.item_template == loot.item
        }) {
            let owner_guid = ObjectGuid::new(HighGuid::Player, 0, character_guid);
            update_blocks.push(build_item_create_update_block(
                owner_guid, owner_guid, new_item, None,
            )?);
            update_blocks.push(build_inventory_slots_update_block(
                character_guid,
                &session.inventory,
                &[dst_slot],
            )?);
        }
    } else {
        session.inventory =
            wow_db::get_character_inventory_items(character_db_pool, character_guid).await?;
    }

    if let Some(creature) = session.db_creatures.get_mut(&creature_guid) {
        creature.loot_item = None;
    }
    send_packet(
        stream,
        SMSG_LOOT_REMOVED,
        &[loot_slot],
        Some(&mut *header_crypto),
    )
    .await?;
    let body = build_update_object_body(&update_blocks);
    send_packet(stream, SMSG_UPDATE_OBJECT, &body, Some(header_crypto)).await
}

fn build_db_creature_loot_response_body(
    target: ObjectGuid,
    creature: &DbCreatureRuntime,
) -> Vec<u8> {
    let item_count = u8::from(creature.loot_item.is_some());
    let mut body = Vec::with_capacity(14 + item_count as usize * 22);
    body.extend_from_slice(&target.raw().to_le_bytes());
    body.push(CLIENT_LOOT_CORPSE);
    body.extend_from_slice(
        &(if creature.loot_money_available {
            creature.loot_money()
        } else {
            0
        })
        .to_le_bytes(),
    );
    body.push(item_count);
    if let Some(loot) = &creature.loot_item {
        body.push(0);
        body.extend_from_slice(&loot.item.to_le_bytes());
        body.extend_from_slice(&loot.count.to_le_bytes());
        body.extend_from_slice(&loot.display_id.to_le_bytes());
        body.extend_from_slice(&0u32.to_le_bytes());
        body.extend_from_slice(&0u32.to_le_bytes());
        body.push(LOOT_SLOT_NORMAL);
    }
    body
}

fn build_loot_release_response_body(target: ObjectGuid, released: bool) -> Vec<u8> {
    let mut body = Vec::with_capacity(9);
    body.extend_from_slice(&target.raw().to_le_bytes());
    body.push(released as u8);
    body
}

fn build_attacker_state_update_body(
    attacker: ObjectGuid,
    victim: ObjectGuid,
    damage: u32,
) -> anyhow::Result<Vec<u8>> {
    build_attacker_state_update_body_with_spell_id(attacker, victim, damage, 0)
}

fn build_attacker_state_update_body_with_spell_id(
    attacker: ObjectGuid,
    victim: ObjectGuid,
    damage: u32,
    spell_id: u32,
) -> anyhow::Result<Vec<u8>> {
    let mut body = Vec::with_capacity(42);
    body.extend_from_slice(&HITINFO_NORMALSWING2.to_le_bytes());
    PackedGuid::write(&mut body, attacker)?;
    PackedGuid::write(&mut body, victim)?;
    body.extend_from_slice(&damage.to_le_bytes());
    body.push(1);
    body.extend_from_slice(&0u32.to_le_bytes()); // normal school
    body.extend_from_slice(&(damage as f32).to_le_bytes());
    body.extend_from_slice(&damage.to_le_bytes());
    body.extend_from_slice(&0u32.to_le_bytes()); // absorb
    body.extend_from_slice(&0i32.to_le_bytes()); // resist
    body.extend_from_slice(&VICTIMSTATE_NORMAL.to_le_bytes());
    body.extend_from_slice(&0u32.to_le_bytes()); // unknown
    body.extend_from_slice(&spell_id.to_le_bytes());
    body.extend_from_slice(&0u32.to_le_bytes()); // blocked
    Ok(body)
}

fn build_combat_dummy_state_update_body(
    health: u32,
    dynamic_flags: u32,
) -> anyhow::Result<Vec<u8>> {
    let guid = rust_combat_dummy_guid();
    let mut block = Vec::new();
    block.push(UPDATE_TYPE_VALUES);
    PackedGuid::write(&mut block, guid)?;
    let mut values = vec![None; PLAYER_END_FIELDS];
    set_update_value(&mut values, UNIT_FIELD_HEALTH, health)?;
    set_update_value(&mut values, UNIT_DYNAMIC_FLAGS, dynamic_flags)?;
    write_update_values(&mut block, &values)?;

    let mut body = Vec::with_capacity(5 + block.len());
    body.extend_from_slice(&1u32.to_le_bytes());
    body.push(0);
    body.extend_from_slice(&block);
    Ok(body)
}

fn build_player_rage_update_body(player: ObjectGuid, rage: u32) -> anyhow::Result<Vec<u8>> {
    let mut block = Vec::new();
    block.push(UPDATE_TYPE_VALUES);
    PackedGuid::write(&mut block, player)?;
    let mut values = vec![None; PLAYER_END_FIELDS];
    set_update_value(&mut values, UNIT_FIELD_POWER2, rage.min(POWER_RAGE_DEFAULT))?;
    write_update_values(&mut block, &values)?;

    let mut body = Vec::with_capacity(5 + block.len());
    body.extend_from_slice(&1u32.to_le_bytes());
    body.push(0);
    body.extend_from_slice(&block);
    Ok(body)
}

fn build_db_creature_state_update_body(
    guid: ObjectGuid,
    health: u32,
    dynamic_flags: u32,
) -> anyhow::Result<Vec<u8>> {
    let mut block = Vec::new();
    block.push(UPDATE_TYPE_VALUES);
    PackedGuid::write(&mut block, guid)?;
    let mut values = vec![None; PLAYER_END_FIELDS];
    set_update_value(&mut values, UNIT_FIELD_HEALTH, health)?;
    set_update_value(&mut values, UNIT_DYNAMIC_FLAGS, dynamic_flags)?;
    write_update_values(&mut block, &values)?;

    Ok(build_update_object_body(&[block]))
}

fn build_player_mana_update_body(player: ObjectGuid, mana: u32) -> anyhow::Result<Vec<u8>> {
    let mut block = Vec::new();
    block.push(UPDATE_TYPE_VALUES);
    PackedGuid::write(&mut block, player)?;
    let mut values = vec![None; PLAYER_END_FIELDS];
    set_update_value(&mut values, UNIT_FIELD_POWER1, mana)?;
    write_update_values(&mut block, &values)?;

    let mut body = Vec::with_capacity(5 + block.len());
    body.extend_from_slice(&1u32.to_le_bytes());
    body.push(0);
    body.extend_from_slice(&block);
    Ok(body)
}

fn build_player_health_update_body(player: ObjectGuid, health: u32) -> anyhow::Result<Vec<u8>> {
    let mut block = Vec::new();
    block.push(UPDATE_TYPE_VALUES);
    PackedGuid::write(&mut block, player)?;
    let mut values = vec![None; PLAYER_END_FIELDS];
    set_update_value(&mut values, UNIT_FIELD_HEALTH, health.max(PLAYER_SURVIVOR_HEALTH_FLOOR))?;
    write_update_values(&mut block, &values)?;

    let mut body = Vec::with_capacity(5 + block.len());
    body.extend_from_slice(&1u32.to_le_bytes());
    body.push(0);
    body.extend_from_slice(&block);
    Ok(body)
}

fn build_log_xp_gain_body(source: Option<ObjectGuid>, given_xp: u32) -> Vec<u8> {
    let mut body = Vec::with_capacity(21);
    body.extend_from_slice(&source.map_or(0, |guid| guid.raw()).to_le_bytes());
    body.extend_from_slice(&given_xp.to_le_bytes());
    body.push(u8::from(source.is_none()));
    if source.is_some() {
        body.extend_from_slice(&given_xp.to_le_bytes());
        body.extend_from_slice(&1.0f32.to_le_bytes());
    }
    body
}

fn build_levelup_info_body(
    new_level: u8,
    previous_stats: &PlayerWorldStats,
    new_stats: &PlayerWorldStats,
) -> Vec<u8> {
    let mut body = Vec::with_capacity(48);
    body.extend_from_slice(&(new_level as u32).to_le_bytes());
    body.extend_from_slice(
        &(new_stats.base_health as i32 - previous_stats.base_health as i32)
            .to_le_bytes(),
    );
    body.extend_from_slice(
        &(new_stats.base_mana as i32 - previous_stats.base_mana as i32).to_le_bytes(),
    );
    for _ in 0..4 {
        body.extend_from_slice(&0u32.to_le_bytes());
    }
    for index in 0..MAX_STATS {
        body.extend_from_slice(
            &(new_stats.stats[index] as i32 - previous_stats.stats[index] as i32)
                .to_le_bytes(),
        );
    }
    body
}

#[derive(Debug, Clone, Copy)]
struct PlayerProgressionUpdate<'a> {
    character_guid: u32,
    level: u8,
    xp: u32,
    health: u32,
    power1: u32,
    power2: u32,
    power3: u32,
    power4: u32,
    power5: u32,
    world_stats: &'a PlayerWorldStats,
}

fn build_player_progression_update_body(update: PlayerProgressionUpdate<'_>) -> anyhow::Result<Vec<u8>> {
    let PlayerProgressionUpdate {
        character_guid,
        level,
        xp,
        health,
        power1,
        power2,
        power3,
        power4,
        power5,
        world_stats,
    } = update;
    let player = ObjectGuid::new(HighGuid::Player, 0, character_guid);
    let mut block = Vec::new();
    block.push(UPDATE_TYPE_VALUES);
    PackedGuid::write(&mut block, player)?;
    let mut values = vec![None; PLAYER_END_FIELDS];
    let max_health = world_stats.max_health().max(1);
    let max_mana = world_stats.max_mana();
    set_update_value(&mut values, UNIT_FIELD_HEALTH, health.max(1).min(max_health))?;
    set_update_value(&mut values, UNIT_FIELD_POWER1, power1.min(max_mana))?;
    set_update_value(&mut values, UNIT_FIELD_POWER2, power2.min(POWER_RAGE_DEFAULT))?;
    set_update_value(&mut values, UNIT_FIELD_POWER3, power3)?;
    set_update_value(&mut values, UNIT_FIELD_POWER4, power4.min(POWER_ENERGY_DEFAULT))?;
    set_update_value(&mut values, UNIT_FIELD_POWER5, power5)?;
    set_update_value(&mut values, UNIT_FIELD_MAXHEALTH, max_health)?;
    set_update_value(&mut values, UNIT_FIELD_MAXPOWER1, max_mana)?;
    set_update_value(
        &mut values,
        UNIT_FIELD_MAXPOWER2,
        if power2 > 0 { POWER_RAGE_DEFAULT } else { 0 },
    )?;
    set_update_value(&mut values, UNIT_FIELD_MAXPOWER3, 0)?;
    set_update_value(
        &mut values,
        UNIT_FIELD_MAXPOWER4,
        if power4 > 0 { POWER_ENERGY_DEFAULT } else { 0 },
    )?;
    set_update_value(&mut values, UNIT_FIELD_MAXPOWER5, 0)?;
    set_update_value(&mut values, UNIT_FIELD_LEVEL, level as u32)?;
    set_update_value(&mut values, UNIT_FIELD_BASE_MANA, max_mana)?;
    set_update_value(&mut values, UNIT_FIELD_BASE_HEALTH, max_health)?;
    set_player_stat_update_values(&mut values, world_stats)?;
    set_update_value(&mut values, PLAYER_XP, xp)?;
    set_update_value(&mut values, PLAYER_NEXT_LEVEL_XP, world_stats.next_level_xp)?;
    write_update_values(&mut block, &values)?;

    Ok(build_update_object_body(&[block]))
}

fn retaliation_damage_for_db_creature(session: &mut WorldSessionState, target: ObjectGuid) -> u32 {
    let Some(creature) = session.db_creatures.get(&target.raw()) else {
        return 0;
    };
    let retaliation_damage = creature.hit_damage().max(1);
    session.player_health = if session.player_health <= retaliation_damage {
        PLAYER_SURVIVOR_HEALTH_FLOOR
    } else {
        session.player_health - retaliation_damage
    };
    retaliation_damage
}

fn read_packet_guid(body: &[u8], packet_name: &str) -> anyhow::Result<ObjectGuid> {
    if body.len() < 8 {
        anyhow::bail!("{packet_name} payload must include an 8-byte GUID");
    }
    Ok(ObjectGuid::from_raw(u64::from_le_bytes(
        body[0..8].try_into()?,
    )))
}

#[cfg(test)]
fn build_rust_guide_gossip_message() -> Vec<u8> {
    build_gossip_message(
        rust_guide_guid(),
        RUST_GUIDE_GOSSIP_TEXT_ID,
        &[(0, RUST_GUIDE_GOSSIP_OPTION)],
    )
}

#[cfg(test)]
fn build_rust_guide_npc_text_update(text_id: u32) -> Vec<u8> {
    build_npc_text_update(text_id, RUST_GUIDE_GOSSIP_TEXT)
}

fn build_gossip_message(guid: ObjectGuid, text_id: u32, options: &[(u32, &str)]) -> Vec<u8> {
    let option_text_len: usize = options.iter().map(|(_, text)| text.len() + 1).sum();
    let mut body = Vec::with_capacity(16 + options.len() * 6 + option_text_len);
    body.extend_from_slice(&guid.raw().to_le_bytes());
    body.extend_from_slice(&text_id.to_le_bytes());
    body.extend_from_slice(&(options.len() as u32).to_le_bytes());
    for (option_index, option_text) in options {
        body.extend_from_slice(&option_index.to_le_bytes());
        body.push(0); // icon
        body.push(0); // coded
        write_c_string(&mut body, option_text);
    }
    body.extend_from_slice(&0u32.to_le_bytes()); // quest option count
    body
}

fn build_npc_text_update(text_id: u32, primary_text: &str) -> Vec<u8> {
    let mut body = Vec::with_capacity(220);
    body.extend_from_slice(&text_id.to_le_bytes());
    for index in 0..8 {
        body.extend_from_slice(&(if index == 0 { 1.0f32 } else { 0.0f32 }).to_le_bytes());
        let text = if index == 0 { primary_text } else { "" };
        write_c_string(&mut body, text);
        write_c_string(&mut body, text);
        body.extend_from_slice(&0u32.to_le_bytes()); // language
        for _ in 0..3 {
            body.extend_from_slice(&0u32.to_le_bytes()); // emote delay
            body.extend_from_slice(&0u32.to_le_bytes()); // emote id
        }
    }
    body
}

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
