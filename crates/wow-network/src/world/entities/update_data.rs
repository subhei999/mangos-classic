use super::*;
use wow_proto::world::WorldOpcode;
use wow_proto::{ServerWorldPacket, SmsgUpdateObjectResponse};

// CMaNGOS reference: src/game/Entities/UpdateData.* and object update builders.
pub(in crate::world) async fn send_self_spawn_update(
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
            WorldOpcode::SmsgUpdateObject as u16,
            &body,
            header_crypto.as_deref_mut(),
        )
        .await?;
    }
    Ok(())
}

pub(in crate::world) struct SelfSpawnUpdate<'a> {
    pub(in crate::world) character: &'a CharacterEnumEntry,
    pub(in crate::world) inventory: &'a [CharacterInventoryItem],
    pub(in crate::world) inventory_container_slots: &'a HashMap<u32, u32>,
    pub(in crate::world) base_world_stats: &'a PlayerWorldStats,
    pub(in crate::world) world_stats: &'a PlayerWorldStats,
    pub(in crate::world) skills: &'a [CharacterSkill],
    pub(in crate::world) active_spells: &'a HashSet<u32>,
    pub(in crate::world) quest_statuses: &'a HashMap<u32, CharacterQuestStatus>,
    pub(in crate::world) equipped_templates: &'a [EquippedItemTemplate],
    pub(in crate::world) ammo_template: Option<&'a ItemTemplateQuery>,
    pub(in crate::world) active_auras: &'a [ActiveAura],
    pub(in crate::world) nearby_creatures: &'a [DbCreatureRuntime],
    pub(in crate::world) nearby_gameobjects: &'a [DbGameObjectRuntime],
    pub(in crate::world) nearby_player_corpses: &'a [PlayerCorpseRuntime],
}

pub(in crate::world) async fn load_equipped_item_templates(
    world_db_pool: &MySqlPool,
    inventory: &[CharacterInventoryItem],
) -> anyhow::Result<Vec<EquippedItemTemplate>> {
    load_equipped_item_templates_with_enchantments(world_db_pool, inventory, &HashMap::new()).await
}

pub(in crate::world) async fn load_equipped_item_templates_with_enchantments(
    world_db_pool: &MySqlPool,
    inventory: &[CharacterInventoryItem],
    spell_item_enchantments: &HashMap<u32, SpellItemEnchantmentEntry>,
) -> anyhow::Result<Vec<EquippedItemTemplate>> {
    let mut templates = Vec::new();
    for item in inventory {
        if item.bag != INVENTORY_SLOT_BAG_0 as u32 || item.slot >= EQUIPMENT_SLOT_END {
            continue;
        }
        let Some(template) =
            wow_db::get_item_template_query(world_db_pool, item.item_template).await?
        else {
            continue;
        };
        let (enchantment_stat_bonuses, enchantment_resistance_bonuses) =
            item_enchantment_bonuses(&item.enchantments, spell_item_enchantments);
        templates.push(EquippedItemTemplate {
            slot: item.slot,
            template,
            enchantment_stat_bonuses,
            enchantment_resistance_bonuses,
        });
    }
    Ok(templates)
}

pub(in crate::world) fn build_self_spawn_update_bodies(
    update: &SelfSpawnUpdate<'_>,
) -> anyhow::Result<Vec<Vec<u8>>> {
    let mut blocks = build_self_spawn_update_blocks(update)?;
    let creature_start = 1;
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

pub(in crate::world) fn chunk_update_blocks_by_body_size(
    blocks: &[Vec<u8>],
) -> anyhow::Result<Vec<Vec<u8>>> {
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

        if !current_blocks.is_empty() && current_len + block_len > MAX_SERVER_PACKET_BODY_BYTES {
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

pub(in crate::world) fn build_self_spawn_update_blocks(
    update: &SelfSpawnUpdate<'_>,
) -> anyhow::Result<Vec<Vec<u8>>> {
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
        update.active_spells,
        update.quest_statuses,
        update.equipped_templates,
        update.ammo_template,
        update.active_auras,
    )?;

    let creature_blocks =
        build_db_creature_create_blocks_for_player(update.nearby_creatures, Some(character.guid))?;
    let gameobject_blocks =
        build_db_gameobject_create_blocks(update.nearby_gameobjects, update.quest_statuses)?;
    let corpse_blocks = build_player_corpse_create_blocks(update.nearby_player_corpses)?;
    let item_blocks = build_inventory_item_create_blocks(
        character,
        update.inventory,
        update.inventory_container_slots,
    )?;
    let mut blocks = Vec::with_capacity(
        1 + creature_blocks.len()
            + gameobject_blocks.len()
            + corpse_blocks.len()
            + item_blocks.len(),
    );
    blocks.push(block);
    blocks.extend(creature_blocks);
    blocks.extend(gameobject_blocks);
    blocks.extend(corpse_blocks);
    blocks.extend(item_blocks);
    Ok(blocks)
}

pub(in crate::world) fn write_update_values(
    body: &mut Vec<u8>,
    values: &[Option<u32>],
) -> anyhow::Result<()> {
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

pub(in crate::world) fn set_update_value(
    values: &mut [Option<u32>],
    index: usize,
    value: u32,
) -> anyhow::Result<()> {
    if index >= values.len() {
        anyhow::bail!("update field index {index} exceeds player field count");
    }
    values[index] = Some(value);
    Ok(())
}

pub(in crate::world) fn make_pair32(low: u16, high: u16) -> u32 {
    low as u32 | ((high as u32) << 16)
}

pub(in crate::world) fn build_update_object_body(blocks: &[Vec<u8>]) -> Vec<u8> {
    SmsgUpdateObjectResponse {
        blocks: blocks.to_vec(),
    }
    .body()
}
