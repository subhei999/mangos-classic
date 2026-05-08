// CMaNGOS reference: src/game/Entities/UpdateData.* and object update builders.
async fn send_self_spawn_update(
    stream: &mut WorldPacketSink,
    update: SelfSpawnUpdate<'_>,
    header_crypto: Option<&mut HeaderCrypto>,
) -> anyhow::Result<()> {
    let bodies = build_self_spawn_update_bodies(&update)?;
    info!(
        guid = update.character.guid,
        name = %update.character.name,
        packets = bodies.len(),
        bytes = bodies.iter().map(Vec::len).sum::<usize>(),
        max_packet_bytes = bodies.iter().map(Vec::len).max().unwrap_or(0),
        "Sending minimal self spawn update"
    );
    let mut header_crypto = header_crypto;
    for body in bodies {
        send_packet(
            stream,
            SMSG_UPDATE_OBJECT,
            &body,
            header_crypto.as_deref_mut(),
        )
        .await?;
    }
    Ok(())
}

struct SelfSpawnUpdate<'a> {
    character: &'a CharacterEnumEntry,
    inventory: &'a [CharacterInventoryItem],
    base_world_stats: &'a PlayerWorldStats,
    world_stats: &'a PlayerWorldStats,
    skills: &'a [CharacterSkill],
    quest_statuses: &'a HashMap<u32, CharacterQuestStatus>,
    equipped_templates: &'a [EquippedItemTemplate],
    active_auras: &'a [ActiveAura],
    nearby_creatures: &'a [DbCreatureRuntime],
    nearby_gameobjects: &'a [DbGameObjectRuntime],
    nearby_player_corpses: &'a [PlayerCorpseRuntime],
}

async fn load_equipped_item_templates(
    world_db_pool: &MySqlPool,
    inventory: &[CharacterInventoryItem],
) -> anyhow::Result<Vec<EquippedItemTemplate>> {
    let mut templates = Vec::new();
    for item in inventory {
        if item.bag != INVENTORY_SLOT_BAG_0 as u32 || item.slot >= EQUIPMENT_SLOT_END {
            continue;
        }
        let Some(template) = wow_db::get_item_template_query(world_db_pool, item.item_template).await?
        else {
            continue;
        };
        templates.push(EquippedItemTemplate {
            slot: item.slot,
            template,
        });
    }
    Ok(templates)
}

fn build_self_spawn_update_bodies(update: &SelfSpawnUpdate<'_>) -> anyhow::Result<Vec<Vec<u8>>> {
    let mut blocks = build_self_spawn_update_blocks(update)?;
    let leading_block_count = 1 + if legacy_fixture_npcs_enabled() { 2 } else { 0 };
    let creature_start = leading_block_count;
    let item_start = creature_start
        + update.nearby_creatures.len()
        + update.nearby_gameobjects.len()
        + update.nearby_player_corpses.len();
    let item_blocks = blocks.split_off(item_start);
    let creature_blocks = blocks.split_off(creature_start);

    let mut first_blocks = blocks;
    first_blocks.extend(item_blocks);
    let mut bodies = chunk_update_blocks_by_body_size(&first_blocks)?;
    for chunk in creature_blocks.chunks(CREATURE_UPDATE_CHUNK_SIZE) {
        bodies.extend(chunk_update_blocks_by_body_size(chunk)?);
    }
    Ok(bodies)
}

fn chunk_update_blocks_by_body_size(blocks: &[Vec<u8>]) -> anyhow::Result<Vec<Vec<u8>>> {
    const UPDATE_OBJECT_BODY_PREFIX_BYTES: usize = 5;
    const MAX_SERVER_PACKET_BODY_BYTES: usize = 0x2800 - 2;

    let mut bodies = Vec::new();
    let mut current_blocks = Vec::new();
    let mut current_len = UPDATE_OBJECT_BODY_PREFIX_BYTES;

    for block in blocks {
        let block_len = block.len();
        if UPDATE_OBJECT_BODY_PREFIX_BYTES + block_len > MAX_SERVER_PACKET_BODY_BYTES {
            anyhow::bail!(
                "single SMSG_UPDATE_OBJECT block exceeds packet body limit: {} bytes",
                block_len
            );
        }

        if !current_blocks.is_empty()
            && current_len + block_len > MAX_SERVER_PACKET_BODY_BYTES
        {
            bodies.push(build_update_object_body(&current_blocks));
            current_blocks.clear();
            current_len = UPDATE_OBJECT_BODY_PREFIX_BYTES;
        }

        current_blocks.push(block.clone());
        current_len += block_len;
    }

    if !current_blocks.is_empty() {
        bodies.push(build_update_object_body(&current_blocks));
    }

    Ok(bodies)
}

fn build_self_spawn_update_blocks(update: &SelfSpawnUpdate<'_>) -> anyhow::Result<Vec<Vec<u8>>> {
    let character = update.character;
    let guid = ObjectGuid::new(HighGuid::Player, 0, character.guid);
    let mut block = Vec::new();
    block.push(UPDATE_TYPE_CREATE_OBJECT2);
    PackedGuid::write(&mut block, guid)?;
    block.push(TYPEID_PLAYER);

    block.push(UPDATEFLAG_SELF | UPDATEFLAG_ALL | UPDATEFLAG_LIVING | UPDATEFLAG_HAS_POSITION);
    block.extend_from_slice(&0u32.to_le_bytes()); // movement flags
    block.extend_from_slice(&0u32.to_le_bytes()); // server time placeholder
    block.extend_from_slice(&character.position_x.to_le_bytes());
    block.extend_from_slice(&character.position_y.to_le_bytes());
    block.extend_from_slice(&character.position_z.to_le_bytes());
    block.extend_from_slice(&character.orientation.to_le_bytes());
    block.extend_from_slice(&0u32.to_le_bytes()); // fall time
    block.extend_from_slice(&2.5f32.to_le_bytes()); // walk
    block.extend_from_slice(&7.0f32.to_le_bytes()); // run
    block.extend_from_slice(&4.5f32.to_le_bytes()); // run back
    block.extend_from_slice(&4.722222f32.to_le_bytes()); // swim
    block.extend_from_slice(&2.5f32.to_le_bytes()); // swim back
    block.extend_from_slice(&std::f32::consts::PI.to_le_bytes()); // turn rate
    block.extend_from_slice(&1u32.to_le_bytes()); // UPDATEFLAG_ALL payload

    write_minimal_player_update_values(
        &mut block,
        guid,
        character,
        update.inventory,
        update.base_world_stats,
        update.world_stats,
        update.skills,
        update.quest_statuses,
        update.equipped_templates,
        update.active_auras,
    )?;

    let creature_blocks = build_db_creature_create_blocks(update.nearby_creatures)?;
    let gameobject_blocks =
        build_db_gameobject_create_blocks(update.nearby_gameobjects, update.quest_statuses)?;
    let corpse_blocks = build_player_corpse_create_blocks(update.nearby_player_corpses)?;
    let item_blocks = build_inventory_item_create_blocks(character, update.inventory)?;
    let legacy_fixture_count = if legacy_fixture_npcs_enabled() { 1 } else { 0 };
    let mut blocks = Vec::with_capacity(
        1 + legacy_fixture_count
            + creature_blocks.len()
            + gameobject_blocks.len()
            + corpse_blocks.len()
            + item_blocks.len(),
    );
    blocks.push(block);
    if legacy_fixture_npcs_enabled() {
        blocks.push(build_rust_guide_create_block(character)?);
    }
    blocks.extend(creature_blocks);
    blocks.extend(gameobject_blocks);
    blocks.extend(corpse_blocks);
    blocks.extend(item_blocks);
    Ok(blocks)
}

fn write_update_values(body: &mut Vec<u8>, values: &[Option<u32>]) -> anyhow::Result<()> {
    let block_count = values.len().div_ceil(32);
    body.push(block_count as u8);
    let mask_start = body.len();
    body.resize(mask_start + block_count * 4, 0);

    for (index, value) in values.iter().enumerate() {
        if let Some(value) = value {
            let block = index / 32;
            let bit = index % 32;
            let offset = mask_start + block * 4;
            let mut mask = u32::from_le_bytes(body[offset..offset + 4].try_into()?);
            mask |= 1u32 << bit;
            body[offset..offset + 4].copy_from_slice(&mask.to_le_bytes());
            body.extend_from_slice(&value.to_le_bytes());
        }
    }

    Ok(())
}

fn set_update_value(values: &mut [Option<u32>], index: usize, value: u32) -> anyhow::Result<()> {
    if index >= values.len() {
        anyhow::bail!("update field index {index} exceeds player field count");
    }
    values[index] = Some(value);
    Ok(())
}

fn make_pair32(low: u16, high: u16) -> u32 {
    low as u32 | ((high as u32) << 16)
}


fn build_update_object_body(blocks: &[Vec<u8>]) -> Vec<u8> {
    let body_len = 5 + blocks.iter().map(Vec::len).sum::<usize>();
    let mut body = Vec::with_capacity(body_len);
    body.extend_from_slice(&(blocks.len() as u32).to_le_bytes());
    body.push(0);
    for block in blocks {
        body.extend_from_slice(block);
    }
    body
}

