#[derive(Debug, Clone, PartialEq)]
struct Corpse {
    guid: ObjectGuid,
    owner: ObjectGuid,
    position: WorldPosition,
    corpse_type: u8,
    race: u8,
    class: u8,
    gender: u8,
    player_bytes: u32,
    player_bytes2: u32,
    equipment_cache: Option<String>,
    guildid: Option<u32>,
    player_flags: u32,
}

// CMaNGOS reference: src/game/Entities/Corpse.* player corpse update builders.
fn build_player_corpse_create_blocks(
    corpses: &[PlayerCorpseRuntime],
) -> anyhow::Result<Vec<Vec<u8>>> {
    corpses.iter().map(build_player_corpse_create_block).collect()
}

fn build_player_corpse_create_block(corpse: &PlayerCorpseRuntime) -> anyhow::Result<Vec<u8>> {
    let mut block = Vec::new();
    block.push(UPDATE_TYPE_CREATE_OBJECT2);
    PackedGuid::write(&mut block, corpse.guid)?;
    block.push(TYPEID_CORPSE);

    block.push(UPDATEFLAG_ALL | UPDATEFLAG_HAS_POSITION);
    block.extend_from_slice(&corpse.position.x.to_le_bytes());
    block.extend_from_slice(&corpse.position.y.to_le_bytes());
    block.extend_from_slice(&corpse.position.z.to_le_bytes());
    block.extend_from_slice(&corpse.position.orientation.to_le_bytes());
    block.extend_from_slice(&1u32.to_le_bytes());

    let mut values = vec![None; CORPSE_END_FIELDS];
    set_update_value(&mut values, 0x000, corpse.guid.raw() as u32)?;
    set_update_value(&mut values, 0x001, (corpse.guid.raw() >> 32) as u32)?;
    set_update_value(&mut values, 0x002, TYPEMASK_OBJECT_CORPSE)?;
    set_update_value(&mut values, 0x004, 1.0f32.to_bits())?;
    set_update_value(&mut values, CORPSE_FIELD_OWNER, corpse.owner.raw() as u32)?;
    set_update_value(
        &mut values,
        CORPSE_FIELD_OWNER + 1,
        (corpse.owner.raw() >> 32) as u32,
    )?;
    set_update_value(
        &mut values,
        CORPSE_FIELD_FACING,
        corpse.position.orientation.to_bits(),
    )?;
    set_update_value(&mut values, CORPSE_FIELD_POS_X, corpse.position.x.to_bits())?;
    set_update_value(&mut values, CORPSE_FIELD_POS_Y, corpse.position.y.to_bits())?;
    set_update_value(&mut values, CORPSE_FIELD_POS_Z, corpse.position.z.to_bits())?;
    set_update_value(
        &mut values,
        CORPSE_FIELD_DISPLAY_ID,
        display_id_for_corpse(corpse),
    )?;
    set_corpse_item_update_values(&mut values, corpse)?;
    set_update_value(&mut values, CORPSE_FIELD_BYTES_1, corpse_bytes_1(corpse))?;
    set_update_value(&mut values, CORPSE_FIELD_BYTES_2, corpse_bytes_2(corpse))?;
    set_update_value(&mut values, CORPSE_FIELD_GUILD, corpse.guildid.unwrap_or(0))?;
    set_update_value(&mut values, CORPSE_FIELD_FLAGS, corpse_flags(corpse))?;
    write_update_values(&mut block, &values)?;
    Ok(block)
}

fn build_player_corpse_bones_update_body(corpse: &PlayerCorpseRuntime) -> anyhow::Result<Vec<u8>> {
    let mut block = Vec::new();
    block.push(UPDATE_TYPE_VALUES);
    PackedGuid::write(&mut block, corpse.guid)?;
    let mut values = vec![None; CORPSE_END_FIELDS];
    set_update_value(&mut values, CORPSE_FIELD_FLAGS, corpse_flags(corpse))?;
    write_update_values(&mut block, &values)?;
    Ok(build_update_object_body(&[block]))
}

fn set_corpse_item_update_values(
    values: &mut [Option<u32>],
    corpse: &PlayerCorpseRuntime,
) -> anyhow::Result<()> {
    let equipment = parse_equipment_cache(corpse.equipment_cache.as_deref());
    for (slot, item_id) in equipment
        .iter()
        .copied()
        .take(EQUIPMENT_SLOT_END as usize)
        .enumerate()
    {
        let Some(visual) = starter_item_visual(item_id) else {
            continue;
        };
        set_update_value(
            values,
            CORPSE_FIELD_ITEM + slot,
            visual.display_id | ((visual.inventory_type as u32) << 24),
        )?;
    }
    Ok(())
}

fn display_id_for_corpse(corpse: &PlayerCorpseRuntime) -> u32 {
    match (corpse.race, corpse.gender) {
        (1, 0) => 49,
        (1, 1) => 50,
        (2, 0) => 51,
        (2, 1) => 52,
        (3, 0) => 53,
        (3, 1) => 54,
        (4, 0) => 55,
        (4, 1) => 56,
        (5, 0) => 57,
        (5, 1) => 58,
        (6, 0) => 59,
        (6, 1) => 60,
        (7, 0) => 1563,
        (7, 1) => 1564,
        (8, 0) => 1478,
        (8, 1) => 1479,
        _ => 49,
    }
}

fn corpse_bytes_1(corpse: &PlayerCorpseRuntime) -> u32 {
    let skin = (corpse.player_bytes & 0xFF) as u8;
    ((corpse.race as u32) << 8) | ((corpse.gender as u32) << 16) | ((skin as u32) << 24)
}

fn corpse_bytes_2(corpse: &PlayerCorpseRuntime) -> u32 {
    let face = (corpse.player_bytes >> 8) & 0xFF;
    let hairstyle = (corpse.player_bytes >> 16) & 0xFF;
    let haircolor = (corpse.player_bytes >> 24) & 0xFF;
    let facialhair = corpse.player_bytes2 & 0xFF;
    facialhair | (face << 8) | (hairstyle << 16) | (haircolor << 24)
}

fn corpse_flags(corpse: &PlayerCorpseRuntime) -> u32 {
    if corpse.corpse_type == PLAYER_CORPSE_TYPE_BONES {
        return CORPSE_FLAG_BONES;
    }

    let mut flags = CORPSE_FLAG_UNK2;
    if corpse.player_flags & PLAYER_FLAGS_HIDE_HELM != 0 {
        flags |= CORPSE_FLAG_HIDE_HELM;
    }
    if corpse.player_flags & PLAYER_FLAGS_HIDE_CLOAK != 0 {
        flags |= CORPSE_FLAG_HIDE_CLOAK;
    }
    flags
}

