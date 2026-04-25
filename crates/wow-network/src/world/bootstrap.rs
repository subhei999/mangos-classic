struct EnterWorldBootstrap<'a> {
    character_db_pool: &'a MySqlPool,
    character: &'a CharacterEnumEntry,
    inventory: &'a [CharacterInventoryItem],
    world_stats: &'a PlayerWorldStats,
    tutorial_flags: &'a [u32; 8],
    cinematic_sequence: Option<u32>,
}

async fn send_enter_world_bootstrap(
    stream: &mut TcpStream,
    bootstrap: EnterWorldBootstrap<'_>,
    header_crypto: Option<&mut HeaderCrypto>,
) -> anyhow::Result<()> {
    let mut header_crypto = header_crypto;
    send_login_verify_world(stream, bootstrap.character, header_crypto.as_deref_mut()).await?;
    send_account_data_times(stream, header_crypto.as_deref_mut()).await?;
    send_bindpoint_update(stream, bootstrap.character, header_crypto.as_deref_mut()).await?;
    send_tutorial_flags(
        stream,
        bootstrap.tutorial_flags,
        header_crypto.as_deref_mut(),
    )
    .await?;
    let spells =
        wow_db::get_character_spells(bootstrap.character_db_pool, bootstrap.character.guid).await?;
    send_initial_spells(stream, &spells, header_crypto.as_deref_mut()).await?;
    let actions =
        wow_db::get_character_actions(bootstrap.character_db_pool, bootstrap.character.guid)
            .await?;
    send_action_buttons(stream, &actions, header_crypto.as_deref_mut()).await?;
    let reputations =
        wow_db::get_character_reputations(bootstrap.character_db_pool, bootstrap.character.guid)
            .await?;
    send_initial_reputations(stream, &reputations, header_crypto.as_deref_mut()).await?;
    send_login_set_time_speed(stream, header_crypto.as_deref_mut()).await?;
    send_init_world_states(stream, bootstrap.character, header_crypto.as_deref_mut()).await?;
    if let Some(cinematic_sequence) = bootstrap.cinematic_sequence {
        send_trigger_cinematic(stream, cinematic_sequence, header_crypto.as_deref_mut()).await?;
    }
    send_self_spawn_update(
        stream,
        bootstrap.character,
        bootstrap.inventory,
        bootstrap.world_stats,
        header_crypto,
    )
    .await?;
    Ok(())
}

async fn send_login_verify_world(
    stream: &mut TcpStream,
    character: &CharacterEnumEntry,
    header_crypto: Option<&mut HeaderCrypto>,
) -> anyhow::Result<()> {
    let body = build_login_verify_world_body(character);
    send_packet(stream, SMSG_LOGIN_VERIFY_WORLD, &body, header_crypto).await
}

async fn send_account_data_times(
    stream: &mut TcpStream,
    header_crypto: Option<&mut HeaderCrypto>,
) -> anyhow::Result<()> {
    let body = build_account_data_times_body();
    send_packet(stream, SMSG_ACCOUNT_DATA_TIMES, &body, header_crypto).await
}

async fn send_bindpoint_update(
    stream: &mut TcpStream,
    character: &CharacterEnumEntry,
    header_crypto: Option<&mut HeaderCrypto>,
) -> anyhow::Result<()> {
    let body = build_bindpoint_update_body(character);
    send_packet(stream, SMSG_BINDPOINTUPDATE, &body, header_crypto).await
}

fn build_login_verify_world_body(character: &CharacterEnumEntry) -> Vec<u8> {
    let mut body = Vec::with_capacity(20);
    body.extend_from_slice(&character.map.to_le_bytes());
    body.extend_from_slice(&character.position_x.to_le_bytes());
    body.extend_from_slice(&character.position_y.to_le_bytes());
    body.extend_from_slice(&character.position_z.to_le_bytes());
    body.extend_from_slice(&character.orientation.to_le_bytes());
    body
}

fn build_bindpoint_update_body(character: &CharacterEnumEntry) -> Vec<u8> {
    let mut body = Vec::with_capacity(20);
    body.extend_from_slice(&character.position_x.to_le_bytes());
    body.extend_from_slice(&character.position_y.to_le_bytes());
    body.extend_from_slice(&character.position_z.to_le_bytes());
    body.extend_from_slice(&character.map.to_le_bytes());
    body.extend_from_slice(&character.zone.to_le_bytes());
    body
}

fn build_account_data_times_body() -> Vec<u8> {
    vec![0u8; ACCOUNT_DATA_TYPES * MD5_DIGEST_LEN]
}

async fn send_tutorial_flags(
    stream: &mut TcpStream,
    tutorial_flags: &[u32; 8],
    header_crypto: Option<&mut HeaderCrypto>,
) -> anyhow::Result<()> {
    let body = build_tutorial_flags_body(tutorial_flags);
    send_packet(stream, SMSG_TUTORIAL_FLAGS, &body, header_crypto).await
}

fn build_tutorial_flags_body(tutorial_flags: &[u32; 8]) -> Vec<u8> {
    let mut body = Vec::with_capacity(tutorial_flags.len() * 4);
    for flag in tutorial_flags {
        body.extend_from_slice(&flag.to_le_bytes());
    }
    body
}

async fn handle_tutorial_flag(
    character_db_pool: &MySqlPool,
    account_id: u32,
    body: &[u8],
) -> anyhow::Result<()> {
    if body.len() < 4 {
        warn!(
            account_id,
            bytes = body.len(),
            "Ignoring malformed tutorial flag"
        );
        return Ok(());
    }

    let flag = u32::from_le_bytes(body[0..4].try_into()?);
    let mut tutorials = wow_db::get_tutorial_flags(character_db_pool, account_id).await?;
    if !apply_tutorial_flag(&mut tutorials, flag) {
        warn!(account_id, flag, "Ignoring out-of-range tutorial flag");
        return Ok(());
    }

    wow_db::save_tutorial_flags(character_db_pool, account_id, tutorials).await?;
    Ok(())
}

fn apply_tutorial_flag(tutorials: &mut [u32; 8], flag: u32) -> bool {
    let index = (flag / 32) as usize;
    if index >= tutorials.len() {
        return false;
    }

    tutorials[index] |= 1u32 << (flag % 32);
    true
}

async fn handle_tutorial_clear(
    character_db_pool: &MySqlPool,
    account_id: u32,
) -> anyhow::Result<()> {
    wow_db::save_tutorial_flags(character_db_pool, account_id, [u32::MAX; 8]).await?;
    Ok(())
}

async fn handle_tutorial_reset(
    character_db_pool: &MySqlPool,
    account_id: u32,
) -> anyhow::Result<()> {
    wow_db::save_tutorial_flags(character_db_pool, account_id, [0; 8]).await?;
    Ok(())
}

async fn send_initial_spells(
    stream: &mut TcpStream,
    spells: &[CharacterSpell],
    header_crypto: Option<&mut HeaderCrypto>,
) -> anyhow::Result<()> {
    let body = build_initial_spells_body(spells);
    send_packet(stream, SMSG_INITIAL_SPELLS, &body, header_crypto).await
}

fn build_initial_spells_body(spells: &[CharacterSpell]) -> Vec<u8> {
    let active_spells = spells
        .iter()
        .filter(|spell| spell.active != 0 && spell.disabled == 0)
        .count();
    let mut body = Vec::with_capacity(5 + active_spells * 4);
    body.push(0); // unknown flags byte
    body.extend_from_slice(&(active_spells as u16).to_le_bytes());
    for spell in spells
        .iter()
        .filter(|spell| spell.active != 0 && spell.disabled == 0)
    {
        body.extend_from_slice(&(spell.spell as u16).to_le_bytes());
        body.extend_from_slice(&0u16.to_le_bytes()); // CMaNGOS writes zero, not an action slot.
    }
    body.extend_from_slice(&0u16.to_le_bytes()); // cooldown count
    body
}

async fn send_action_buttons(
    stream: &mut TcpStream,
    actions: &[CharacterAction],
    header_crypto: Option<&mut HeaderCrypto>,
) -> anyhow::Result<()> {
    let body = build_action_buttons_body(actions);
    send_packet(stream, SMSG_ACTION_BUTTONS, &body, header_crypto).await
}

fn build_action_buttons_body(actions: &[CharacterAction]) -> Vec<u8> {
    let mut buttons = vec![0u32; MAX_ACTION_BUTTONS];
    for action in actions {
        if (action.button as usize) < MAX_ACTION_BUTTONS {
            buttons[action.button as usize] = pack_action_button(action.action, action.action_type);
        }
    }

    let mut body = Vec::with_capacity(MAX_ACTION_BUTTONS * 4);
    for button in buttons {
        body.extend_from_slice(&button.to_le_bytes());
    }
    body
}

fn pack_action_button(action: u32, action_type: u8) -> u32 {
    action | ((action_type as u32) << 24)
}

async fn send_initial_reputations(
    stream: &mut TcpStream,
    reputations: &[CharacterReputation],
    header_crypto: Option<&mut HeaderCrypto>,
) -> anyhow::Result<()> {
    let body = build_initial_reputations_body(reputations);
    send_packet(stream, SMSG_INITIALIZE_FACTIONS, &body, header_crypto).await
}

fn build_initial_reputations_body(reputations: &[CharacterReputation]) -> Vec<u8> {
    let mut slots = vec![(0u8, 0i32); REPUTATION_LIST_SLOTS];
    for reputation in reputations {
        let Some(slot) = reputation_list_slot_for_faction(reputation.faction) else {
            continue;
        };
        if slot < REPUTATION_LIST_SLOTS {
            slots[slot] = (reputation.flags as u8, reputation.standing);
        }
    }

    let mut body = Vec::with_capacity(4 + REPUTATION_LIST_SLOTS * 5);
    body.extend_from_slice(&(REPUTATION_LIST_SLOTS as u32).to_le_bytes());
    for (flags, standing) in slots {
        body.push(flags);
        body.extend_from_slice(&standing.to_le_bytes());
    }
    body
}

fn reputation_list_slot_for_faction(_faction: u32) -> Option<usize> {
    // Faction.dbc IDs are not the same as the client's 0..63 reputationListID
    // slots. Keep saved reputation rows quiet until the DBC-backed mapping is
    // ported; otherwise the client displays unrelated factions such as
    // Bloodsail Buccaneers for starter city reputations.
    None
}

async fn send_trigger_cinematic(
    stream: &mut TcpStream,
    sequence: u32,
    header_crypto: Option<&mut HeaderCrypto>,
) -> anyhow::Result<()> {
    let body = build_trigger_cinematic_body(sequence);
    send_packet(stream, SMSG_TRIGGER_CINEMATIC, &body, header_crypto).await
}

fn build_trigger_cinematic_body(sequence: u32) -> Vec<u8> {
    sequence.to_le_bytes().to_vec()
}

fn cinematic_sequence_for_race(race: u8) -> Option<u32> {
    match race {
        1 => Some(81),  // Human
        2 => Some(21),  // Orc
        3 => Some(41),  // Dwarf
        4 => Some(61),  // Night Elf
        5 => Some(2),   // Undead
        6 => Some(141), // Tauren
        7 => Some(101), // Gnome
        8 => Some(121), // Troll
        _ => None,
    }
}

async fn send_login_set_time_speed(
    stream: &mut TcpStream,
    header_crypto: Option<&mut HeaderCrypto>,
) -> anyhow::Result<()> {
    let mut body = Vec::with_capacity(8);
    body.extend_from_slice(&0u32.to_le_bytes()); // packed server time placeholder
    body.extend_from_slice(&0.01666667f32.to_le_bytes());
    send_packet(stream, SMSG_LOGIN_SETTIMESPEED, &body, header_crypto).await
}

async fn send_init_world_states(
    stream: &mut TcpStream,
    character: &CharacterEnumEntry,
    header_crypto: Option<&mut HeaderCrypto>,
) -> anyhow::Result<()> {
    let mut body = Vec::with_capacity(16);
    body.extend_from_slice(&character.map.to_le_bytes());
    body.extend_from_slice(&character.zone.to_le_bytes());
    body.extend_from_slice(&0u32.to_le_bytes()); // area id, unknown for this skeleton
    body.extend_from_slice(&0u32.to_le_bytes()); // world state count
    send_packet(stream, SMSG_INIT_WORLD_STATES, &body, header_crypto).await
}

async fn send_self_spawn_update(
    stream: &mut TcpStream,
    character: &CharacterEnumEntry,
    inventory: &[CharacterInventoryItem],
    world_stats: &PlayerWorldStats,
    header_crypto: Option<&mut HeaderCrypto>,
) -> anyhow::Result<()> {
    let body = build_self_spawn_update_body(character, inventory, world_stats)?;
    info!(
        guid = character.guid,
        name = %character.name,
        bytes = body.len(),
        "Sending minimal self spawn update"
    );
    send_packet(stream, SMSG_UPDATE_OBJECT, &body, header_crypto).await
}

fn build_self_spawn_update_body(
    character: &CharacterEnumEntry,
    inventory: &[CharacterInventoryItem],
    world_stats: &PlayerWorldStats,
) -> anyhow::Result<Vec<u8>> {
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

    write_minimal_player_update_values(&mut block, guid, character, inventory, world_stats)?;

    let npc_block = build_rust_guide_create_block(character)?;
    let combat_dummy_block = build_rust_combat_dummy_create_block(character)?;
    let item_blocks = build_inventory_item_create_blocks(character, inventory)?;
    let block_count = 3 + item_blocks.len() as u32;
    let mut body = Vec::with_capacity(5 + block.len());
    body.extend_from_slice(&block_count.to_le_bytes());
    body.push(0); // has transport
    body.extend_from_slice(&block);
    body.extend_from_slice(&npc_block);
    body.extend_from_slice(&combat_dummy_block);
    for item_block in item_blocks {
        body.extend_from_slice(&item_block);
    }
    Ok(body)
}

fn build_rust_guide_create_block(character: &CharacterEnumEntry) -> anyhow::Result<Vec<u8>> {
    let guid = rust_guide_guid();
    let mut block = Vec::new();
    block.push(UPDATE_TYPE_CREATE_OBJECT2);
    PackedGuid::write(&mut block, guid)?;
    block.push(TYPEID_UNIT);

    block.push(UPDATEFLAG_ALL | UPDATEFLAG_LIVING | UPDATEFLAG_HAS_POSITION);
    block.extend_from_slice(&0u32.to_le_bytes()); // movement flags
    block.extend_from_slice(&0u32.to_le_bytes()); // server time placeholder
    block.extend_from_slice(&(character.position_x + 4.0).to_le_bytes());
    block.extend_from_slice(&(character.position_y + 2.0).to_le_bytes());
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

    write_rust_guide_update_values(&mut block, guid)?;
    Ok(block)
}

fn write_rust_guide_update_values(body: &mut Vec<u8>, guid: ObjectGuid) -> anyhow::Result<()> {
    let mut values = vec![None; PLAYER_END_FIELDS];
    set_update_value(&mut values, 0x000, guid.raw() as u32)?;
    set_update_value(&mut values, 0x001, (guid.raw() >> 32) as u32)?;
    set_update_value(&mut values, 0x002, TYPEMASK_OBJECT_UNIT)?;
    set_update_value(&mut values, 0x003, RUST_GUIDE_ENTRY)?;
    set_update_value(&mut values, 0x004, 1.0f32.to_bits())?;
    set_update_value(&mut values, UNIT_FIELD_HEALTH, 42)?;
    set_update_value(&mut values, UNIT_FIELD_MAXHEALTH, 42)?;
    set_update_value(&mut values, UNIT_FIELD_LEVEL, 1)?;
    set_update_value(
        &mut values,
        UNIT_FIELD_FACTIONTEMPLATE,
        RUST_GUIDE_FACTION_TEMPLATE,
    )?;
    set_update_value(&mut values, UNIT_FIELD_BYTES_0, 0)?;
    set_update_value(&mut values, UNIT_FIELD_BASEATTACKTIME, 2000)?;
    set_update_value(&mut values, UNIT_FIELD_BASEATTACKTIME + 1, 2000)?;
    set_update_value(&mut values, UNIT_FIELD_RANGEDATTACKTIME, 2000)?;
    set_update_value(&mut values, UNIT_FIELD_BOUNDINGRADIUS, 0.389f32.to_bits())?;
    set_update_value(&mut values, UNIT_FIELD_COMBATREACH, 1.5f32.to_bits())?;
    set_update_value(&mut values, UNIT_FIELD_DISPLAYID, RUST_GUIDE_DISPLAY_ID)?;
    set_update_value(
        &mut values,
        UNIT_FIELD_NATIVEDISPLAYID,
        RUST_GUIDE_DISPLAY_ID,
    )?;
    set_update_value(&mut values, UNIT_FIELD_MINDAMAGE, 0.0f32.to_bits())?;
    set_update_value(&mut values, UNIT_FIELD_MAXDAMAGE, 0.0f32.to_bits())?;
    set_update_value(&mut values, UNIT_FIELD_BYTES_1, 0)?;
    set_update_value(&mut values, UNIT_MOD_CAST_SPEED, 1.0f32.to_bits())?;
    set_update_value(
        &mut values,
        UNIT_NPC_FLAGS,
        UNIT_NPC_FLAG_GOSSIP | UNIT_NPC_FLAG_VENDOR,
    )?;
    write_update_values(body, &values)
}

fn rust_guide_guid() -> ObjectGuid {
    ObjectGuid::new(HighGuid::Unit, RUST_GUIDE_ENTRY, RUST_GUIDE_COUNTER)
}

fn build_rust_combat_dummy_create_block(character: &CharacterEnumEntry) -> anyhow::Result<Vec<u8>> {
    let guid = rust_combat_dummy_guid();
    let mut block = Vec::new();
    block.push(UPDATE_TYPE_CREATE_OBJECT2);
    PackedGuid::write(&mut block, guid)?;
    block.push(TYPEID_UNIT);

    block.push(UPDATEFLAG_ALL | UPDATEFLAG_LIVING | UPDATEFLAG_HAS_POSITION);
    block.extend_from_slice(&0u32.to_le_bytes());
    block.extend_from_slice(&0u32.to_le_bytes());
    block.extend_from_slice(&(character.position_x + 8.0).to_le_bytes());
    block.extend_from_slice(&(character.position_y + 2.0).to_le_bytes());
    block.extend_from_slice(&character.position_z.to_le_bytes());
    block.extend_from_slice(&character.orientation.to_le_bytes());
    block.extend_from_slice(&0u32.to_le_bytes());
    block.extend_from_slice(&2.5f32.to_le_bytes());
    block.extend_from_slice(&7.0f32.to_le_bytes());
    block.extend_from_slice(&4.5f32.to_le_bytes());
    block.extend_from_slice(&4.722222f32.to_le_bytes());
    block.extend_from_slice(&2.5f32.to_le_bytes());
    block.extend_from_slice(&std::f32::consts::PI.to_le_bytes());
    block.extend_from_slice(&1u32.to_le_bytes());

    write_rust_combat_dummy_update_values(&mut block, guid, RUST_COMBAT_DUMMY_HEALTH)?;
    Ok(block)
}

fn write_rust_combat_dummy_update_values(
    body: &mut Vec<u8>,
    guid: ObjectGuid,
    health: u32,
) -> anyhow::Result<()> {
    let mut values = vec![None; PLAYER_END_FIELDS];
    set_update_value(&mut values, 0x000, guid.raw() as u32)?;
    set_update_value(&mut values, 0x001, (guid.raw() >> 32) as u32)?;
    set_update_value(&mut values, 0x002, TYPEMASK_OBJECT_UNIT)?;
    set_update_value(&mut values, 0x003, RUST_COMBAT_DUMMY_ENTRY)?;
    set_update_value(&mut values, 0x004, 1.0f32.to_bits())?;
    set_update_value(&mut values, UNIT_FIELD_HEALTH, health)?;
    set_update_value(&mut values, UNIT_FIELD_MAXHEALTH, RUST_COMBAT_DUMMY_HEALTH)?;
    set_update_value(&mut values, UNIT_FIELD_LEVEL, 1)?;
    set_update_value(
        &mut values,
        UNIT_FIELD_FACTIONTEMPLATE,
        RUST_COMBAT_DUMMY_FACTION_TEMPLATE,
    )?;
    set_update_value(&mut values, UNIT_FIELD_BYTES_0, 0)?;
    set_update_value(&mut values, UNIT_FIELD_BASEATTACKTIME, 2000)?;
    set_update_value(&mut values, UNIT_FIELD_BASEATTACKTIME + 1, 2000)?;
    set_update_value(&mut values, UNIT_FIELD_RANGEDATTACKTIME, 2000)?;
    set_update_value(&mut values, UNIT_FIELD_BOUNDINGRADIUS, 0.389f32.to_bits())?;
    set_update_value(&mut values, UNIT_FIELD_COMBATREACH, 1.5f32.to_bits())?;
    set_update_value(
        &mut values,
        UNIT_FIELD_DISPLAYID,
        RUST_COMBAT_DUMMY_DISPLAY_ID,
    )?;
    set_update_value(
        &mut values,
        UNIT_FIELD_NATIVEDISPLAYID,
        RUST_COMBAT_DUMMY_DISPLAY_ID,
    )?;
    set_update_value(&mut values, UNIT_FIELD_MINDAMAGE, 1.0f32.to_bits())?;
    set_update_value(
        &mut values,
        UNIT_FIELD_MAXDAMAGE,
        (RUST_COMBAT_DUMMY_HIT_DAMAGE as f32).to_bits(),
    )?;
    set_update_value(&mut values, UNIT_FIELD_BYTES_1, 0)?;
    set_update_value(&mut values, UNIT_MOD_CAST_SPEED, 1.0f32.to_bits())?;
    write_update_values(body, &values)
}

fn rust_combat_dummy_guid() -> ObjectGuid {
    ObjectGuid::new(
        HighGuid::Unit,
        RUST_COMBAT_DUMMY_ENTRY,
        RUST_COMBAT_DUMMY_COUNTER,
    )
}

fn write_minimal_player_update_values(
    body: &mut Vec<u8>,
    guid: ObjectGuid,
    character: &CharacterEnumEntry,
    inventory: &[CharacterInventoryItem],
    world_stats: &PlayerWorldStats,
) -> anyhow::Result<()> {
    let mut values = vec![None; PLAYER_END_FIELDS];
    set_update_value(&mut values, 0x000, guid.raw() as u32)?;
    set_update_value(&mut values, 0x001, (guid.raw() >> 32) as u32)?;
    set_update_value(&mut values, 0x002, TYPEMASK_OBJECT_UNIT_PLAYER)?;
    set_update_value(&mut values, 0x004, 1.0f32.to_bits())?;
    set_player_vital_update_values(&mut values, character, world_stats)?;
    set_update_value(&mut values, UNIT_FIELD_LEVEL, character.level as u32)?;
    set_update_value(
        &mut values,
        UNIT_FIELD_FACTIONTEMPLATE,
        faction_for_race(character.race),
    )?;
    set_update_value(&mut values, UNIT_FIELD_BYTES_0, unit_bytes_0(character))?;
    set_update_value(&mut values, UNIT_FIELD_FLAGS, UNIT_FLAG_PLAYER_CONTROLLED)?;
    set_update_value(&mut values, UNIT_FIELD_BASEATTACKTIME, 2000)?;
    set_update_value(&mut values, UNIT_FIELD_BASEATTACKTIME + 1, 2000)?;
    set_update_value(&mut values, UNIT_FIELD_RANGEDATTACKTIME, 2000)?;
    set_update_value(&mut values, UNIT_FIELD_BOUNDINGRADIUS, 0.389f32.to_bits())?;
    set_update_value(&mut values, UNIT_FIELD_COMBATREACH, 1.5f32.to_bits())?;
    set_update_value(
        &mut values,
        UNIT_FIELD_DISPLAYID,
        display_id_for_character(character),
    )?;
    set_update_value(
        &mut values,
        UNIT_FIELD_NATIVEDISPLAYID,
        display_id_for_character(character),
    )?;
    set_update_value(&mut values, UNIT_FIELD_MINDAMAGE, 0.0f32.to_bits())?;
    set_update_value(&mut values, UNIT_FIELD_MAXDAMAGE, 0.0f32.to_bits())?;
    set_update_value(&mut values, UNIT_FIELD_BYTES_1, unit_bytes_1(character))?;
    set_update_value(&mut values, UNIT_MOD_CAST_SPEED, 1.0f32.to_bits())?;
    set_player_stat_update_values(&mut values, world_stats)?;
    set_update_value(&mut values, UNIT_FIELD_BYTES_2, unit_bytes_2())?;
    set_update_value(&mut values, UNIT_FIELD_ATTACK_POWER, 0)?;
    set_update_value(
        &mut values,
        UNIT_FIELD_ATTACK_POWER_MULTIPLIER,
        0.0f32.to_bits(),
    )?;
    set_update_value(&mut values, UNIT_FIELD_RANGED_ATTACK_POWER, 0)?;
    set_update_value(
        &mut values,
        UNIT_FIELD_RANGED_ATTACK_POWER_MULTIPLIER,
        0.0f32.to_bits(),
    )?;
    for index in UNIT_FIELD_POWER_COST_MULTIPLIER..UNIT_FIELD_POWER_COST_MULTIPLIER + 7 {
        set_update_value(&mut values, index, 0.0f32.to_bits())?;
    }
    set_update_value(&mut values, PLAYER_FLAGS_FIELD, character.player_flags)?;
    set_update_value(&mut values, PLAYER_BYTES, character.player_bytes)?;
    set_update_value(&mut values, PLAYER_BYTES_2, character.player_bytes2)?;
    set_update_value(&mut values, PLAYER_BYTES_3, 0)?;
    set_visible_item_update_values(&mut values, character, inventory)?;
    set_inventory_slot_update_values(&mut values, inventory)?;
    set_update_value(&mut values, PLAYER_XP, 0)?;
    set_update_value(&mut values, PLAYER_NEXT_LEVEL_XP, world_stats.next_level_xp)?;
    set_update_value(&mut values, PLAYER_FIELD_COINAGE, character.money)?;
    set_player_damage_mod_update_values(&mut values)?;
    set_update_value(&mut values, PLAYER_FIELD_BYTES, 0)?;
    set_update_value(
        &mut values,
        PLAYER_FIELD_WATCHED_FACTION_INDEX,
        character.watched_faction,
    )?;

    write_update_values(body, &values)?;

    Ok(())
}

fn set_player_vital_update_values(
    values: &mut [Option<u32>],
    character: &CharacterEnumEntry,
    world_stats: &PlayerWorldStats,
) -> anyhow::Result<()> {
    let max_health = character.health.max(world_stats.max_health());
    let max_mana = world_stats.max_mana();
    let power1 = if character.power1 > 0 {
        character.power1
    } else {
        max_mana
    };
    let power2 = character
        .power2
        .min(create_power_for_class_power(character.class, POWER_RAGE));
    let power4 = if character.power4 > 0 {
        character.power4
    } else {
        create_power_for_class_power(character.class, POWER_ENERGY)
    };

    set_update_value(values, UNIT_FIELD_HEALTH, max_health)?;
    set_update_value(values, UNIT_FIELD_POWER1, power1)?;
    set_update_value(values, UNIT_FIELD_POWER2, power2)?;
    set_update_value(
        values,
        UNIT_FIELD_POWER3,
        create_power_for_class_power(character.class, POWER_FOCUS),
    )?;
    set_update_value(values, UNIT_FIELD_POWER4, power4)?;
    set_update_value(
        values,
        UNIT_FIELD_POWER5,
        create_power_for_class_power(character.class, POWER_HAPPINESS),
    )?;
    set_update_value(values, UNIT_FIELD_MAXHEALTH, max_health)?;
    set_update_value(values, UNIT_FIELD_MAXPOWER1, max_mana)?;
    set_update_value(
        values,
        UNIT_FIELD_MAXPOWER2,
        create_power_for_class_power(character.class, POWER_RAGE),
    )?;
    set_update_value(
        values,
        UNIT_FIELD_MAXPOWER3,
        create_power_for_class_power(character.class, POWER_FOCUS),
    )?;
    set_update_value(
        values,
        UNIT_FIELD_MAXPOWER4,
        create_power_for_class_power(character.class, POWER_ENERGY),
    )?;
    set_update_value(
        values,
        UNIT_FIELD_MAXPOWER5,
        create_power_for_class_power(character.class, POWER_HAPPINESS),
    )?;
    set_update_value(values, UNIT_FIELD_BASE_MANA, world_stats.base_mana)?;
    set_update_value(values, UNIT_FIELD_BASE_HEALTH, world_stats.base_health)?;

    Ok(())
}

fn set_player_stat_update_values(
    values: &mut [Option<u32>],
    world_stats: &PlayerWorldStats,
) -> anyhow::Result<()> {
    for (offset, stat) in world_stats.stats.into_iter().enumerate() {
        set_update_value(values, UNIT_FIELD_STAT0 + offset, stat)?;
    }

    Ok(())
}

fn set_player_damage_mod_update_values(values: &mut [Option<u32>]) -> anyhow::Result<()> {
    for index in PLAYER_FIELD_MOD_DAMAGE_DONE_POS..PLAYER_FIELD_MOD_DAMAGE_DONE_POS + 7 {
        set_update_value(values, index, 0)?;
    }
    for index in PLAYER_FIELD_MOD_DAMAGE_DONE_NEG..PLAYER_FIELD_MOD_DAMAGE_DONE_NEG + 7 {
        set_update_value(values, index, 0)?;
    }
    for index in PLAYER_FIELD_MOD_DAMAGE_DONE_PCT..PLAYER_FIELD_MOD_DAMAGE_DONE_PCT + 7 {
        set_update_value(values, index, 1.0f32.to_bits())?;
    }

    Ok(())
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

fn unit_bytes_0(character: &CharacterEnumEntry) -> u32 {
    let power_type = match character.class {
        1 => 1, // warrior rage
        4 => 3, // rogue energy
        _ => 0, // mana
    };
    character.race as u32
        | ((character.class as u32) << 8)
        | ((character.gender as u32) << 16)
        | (power_type << 24)
}

fn unit_bytes_1(character: &CharacterEnumEntry) -> u32 {
    let pet_loyalty = match character.class {
        1 | 8 => 0xEE, // CMaNGOS initializes this for rage and mana users.
        _ => 0,
    };
    let shapeshift_form = match character.class {
        1 => FORM_BATTLESTANCE,
        _ => 0,
    };

    ((pet_loyalty as u32) << 8) | ((shapeshift_form as u32) << 16)
}

fn unit_bytes_2() -> u32 {
    (0x08 | 0x20) << 8
}

fn create_power_for_class_power(class: u8, power: u8) -> u32 {
    match (class, power) {
        (_, POWER_MANA) => 0,
        (1, POWER_RAGE) => POWER_RAGE_DEFAULT,
        (4, POWER_ENERGY) => POWER_ENERGY_DEFAULT,
        _ => 0,
    }
}

fn set_visible_item_update_values(
    values: &mut [Option<u32>],
    character: &CharacterEnumEntry,
    inventory: &[CharacterInventoryItem],
) -> anyhow::Result<()> {
    let mut equipment = parse_equipment_cache(character.equipment_cache.as_deref());
    for item in inventory {
        if item.bag == INVENTORY_SLOT_BAG_0 as u32 && item.slot < EQUIPMENT_SLOT_END {
            equipment[item.slot as usize] = item.item_template;
        }
    }

    for (slot, item_id) in equipment
        .iter()
        .take(EQUIPMENT_SLOT_END as usize)
        .enumerate()
    {
        if *item_id == 0 {
            continue;
        }

        let visible_base = 0x104 + slot * 12;
        set_update_value(values, visible_base, *item_id)?;
    }

    Ok(())
}

fn set_inventory_slot_update_values(
    values: &mut [Option<u32>],
    inventory: &[CharacterInventoryItem],
) -> anyhow::Result<()> {
    for item in inventory {
        if item.bag != INVENTORY_SLOT_BAG_0 as u32 {
            continue;
        }

        let Some(field) = inventory_slot_update_field(item.slot) else {
            continue;
        };
        let guid = ObjectGuid::new(HighGuid::Item, 0, item.item);
        set_update_value(values, field, guid.raw() as u32)?;
        set_update_value(values, field + 1, (guid.raw() >> 32) as u32)?;
    }

    Ok(())
}

fn build_inventory_slots_update_body(
    character_guid: u32,
    inventory: &[CharacterInventoryItem],
    slots: &[u8],
) -> anyhow::Result<Vec<u8>> {
    let block = build_inventory_slots_update_block(character_guid, inventory, slots)?;

    let mut body = Vec::with_capacity(5 + block.len());
    body.extend_from_slice(&1u32.to_le_bytes());
    body.push(0);
    body.extend_from_slice(&block);
    Ok(body)
}

fn build_inventory_slots_update_block(
    character_guid: u32,
    inventory: &[CharacterInventoryItem],
    slots: &[u8],
) -> anyhow::Result<Vec<u8>> {
    let player_guid = ObjectGuid::new(HighGuid::Player, 0, character_guid);
    let mut block = Vec::new();
    block.push(UPDATE_TYPE_VALUES);
    PackedGuid::write(&mut block, player_guid)?;

    let mut values = vec![None; PLAYER_END_FIELDS];
    for slot in slots {
        let Some(field) = inventory_slot_update_field(*slot) else {
            continue;
        };
        let item = inventory
            .iter()
            .find(|item| item.bag == INVENTORY_SLOT_BAG_0 as u32 && item.slot == *slot);
        let item_guid = item
            .map(|item| ObjectGuid::new(HighGuid::Item, 0, item.item).raw())
            .unwrap_or(0);
        set_update_value(&mut values, field, item_guid as u32)?;
        set_update_value(&mut values, field + 1, (item_guid >> 32) as u32)?;

        if *slot < EQUIPMENT_SLOT_END {
            let visible_base = 0x104 + *slot as usize * 12;
            set_update_value(
                &mut values,
                visible_base,
                item.map(|item| item.item_template).unwrap_or(0),
            )?;
        }
    }
    write_update_values(&mut block, &values)?;

    Ok(block)
}

fn build_item_stack_count_update_body(item_guid: u32, count: u32) -> anyhow::Result<Vec<u8>> {
    build_item_stack_counts_update_body(&[(item_guid, count)])
}

fn build_item_stack_counts_update_body(items: &[(u32, u32)]) -> anyhow::Result<Vec<u8>> {
    let mut body = Vec::new();
    body.extend_from_slice(&(items.len() as u32).to_le_bytes());
    body.push(0);

    for (item_guid, count) in items {
        body.extend_from_slice(&build_item_stack_count_update_block(*item_guid, *count)?);
    }

    Ok(body)
}

fn build_item_stack_count_update_block(item_guid: u32, count: u32) -> anyhow::Result<Vec<u8>> {
    let item_guid = ObjectGuid::new(HighGuid::Item, 0, item_guid);
    let mut block = Vec::new();
    block.push(UPDATE_TYPE_VALUES);
    PackedGuid::write(&mut block, item_guid)?;

    let mut values = vec![None; ITEM_END_FIELDS];
    set_update_value(&mut values, 0x00E, count)?;
    write_update_values(&mut block, &values)?;

    Ok(block)
}

fn build_player_money_update_body(character_guid: u32, money: u32) -> anyhow::Result<Vec<u8>> {
    let player_guid = ObjectGuid::new(HighGuid::Player, 0, character_guid);
    let mut block = Vec::new();
    block.push(UPDATE_TYPE_VALUES);
    PackedGuid::write(&mut block, player_guid)?;
    let mut values = vec![None; PLAYER_END_FIELDS];
    set_update_value(&mut values, PLAYER_FIELD_COINAGE, money)?;
    write_update_values(&mut block, &values)?;
    Ok(build_update_object_body(&[block]))
}

fn build_destroy_object_body(item_guid: u32) -> Vec<u8> {
    ObjectGuid::new(HighGuid::Item, 0, item_guid)
        .raw()
        .to_le_bytes()
        .to_vec()
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

fn inventory_slot_update_field(slot: u8) -> Option<usize> {
    match slot {
        0..INVENTORY_SLOT_ITEM_START => Some(PLAYER_FIELD_INV_SLOT_HEAD + slot as usize * 2),
        INVENTORY_SLOT_ITEM_START..INVENTORY_SLOT_ITEM_END => {
            Some(PLAYER_FIELD_PACK_SLOT_1 + (slot - INVENTORY_SLOT_ITEM_START) as usize * 2)
        }
        _ => None,
    }
}

fn build_inventory_item_create_blocks(
    character: &CharacterEnumEntry,
    inventory: &[CharacterInventoryItem],
) -> anyhow::Result<Vec<Vec<u8>>> {
    let owner_guid = ObjectGuid::new(HighGuid::Player, 0, character.guid);
    let mut blocks = Vec::new();

    for item in inventory {
        if item.bag != INVENTORY_SLOT_BAG_0 as u32 {
            continue;
        }

        if item.slot >= INVENTORY_SLOT_ITEM_END {
            continue;
        }

        let contained_guid = item_contained_guid(owner_guid, inventory, item);
        blocks.push(build_item_create_update_block(owner_guid, contained_guid, item, None)?);
    }

    Ok(blocks)
}

fn build_item_create_update_block(
    owner_guid: ObjectGuid,
    contained_guid: ObjectGuid,
    item: &CharacterInventoryItem,
    container_slots: Option<u32>,
) -> anyhow::Result<Vec<u8>> {
    let item_guid = ObjectGuid::new(HighGuid::Item, 0, item.item);
    let is_container = container_slots.unwrap_or(0) > 0;
    let mut block = Vec::new();
    block.push(UPDATE_TYPE_CREATE_OBJECT);
    PackedGuid::write(&mut block, item_guid)?;
    block.push(if is_container {
        TYPEID_CONTAINER
    } else {
        TYPEID_ITEM
    });
    block.push(UPDATEFLAG_ALL);
    block.extend_from_slice(&1u32.to_le_bytes());

    let mut values = vec![None; if is_container { CONTAINER_END_FIELDS } else { ITEM_END_FIELDS }];
    set_update_value(&mut values, 0x000, item_guid.raw() as u32)?;
    set_update_value(&mut values, 0x001, (item_guid.raw() >> 32) as u32)?;
    set_update_value(
        &mut values,
        0x002,
        if is_container {
            TYPEMASK_OBJECT_CONTAINER
        } else {
            TYPEMASK_OBJECT_ITEM
        },
    )?;
    set_update_value(&mut values, 0x003, item.item_template)?;
    set_update_value(&mut values, 0x004, 1.0f32.to_bits())?;
    set_update_value(&mut values, 0x006, owner_guid.raw() as u32)?;
    set_update_value(&mut values, 0x007, (owner_guid.raw() >> 32) as u32)?;
    set_update_value(&mut values, 0x008, contained_guid.raw() as u32)?;
    set_update_value(&mut values, 0x009, (contained_guid.raw() >> 32) as u32)?;
    set_update_value(&mut values, 0x00E, item.count)?;
    set_update_value(&mut values, 0x02E, item.durability)?;
    set_update_value(&mut values, 0x02F, item.durability)?;
    if let Some(container_slots) = container_slots.filter(|slots| *slots > 0) {
        set_update_value(&mut values, CONTAINER_FIELD_NUM_SLOTS, container_slots)?;
    }
    write_update_values(&mut block, &values)?;

    Ok(block)
}

fn build_item_contained_update_block(
    owner_guid: ObjectGuid,
    inventory: &[CharacterInventoryItem],
    item: &CharacterInventoryItem,
) -> anyhow::Result<Vec<u8>> {
    let item_guid = ObjectGuid::new(HighGuid::Item, 0, item.item);
    let contained_guid = item_contained_guid(owner_guid, inventory, item);
    let mut block = Vec::new();
    block.push(UPDATE_TYPE_VALUES);
    PackedGuid::write(&mut block, item_guid)?;

    let mut values = vec![None; ITEM_END_FIELDS];
    set_update_value(&mut values, 0x008, contained_guid.raw() as u32)?;
    set_update_value(&mut values, 0x009, (contained_guid.raw() >> 32) as u32)?;
    write_update_values(&mut block, &values)?;
    Ok(block)
}

fn build_container_slot_update_block(
    inventory: &[CharacterInventoryItem],
    bag_slot: u8,
    container_slot: u8,
) -> anyhow::Result<Option<Vec<u8>>> {
    let Some(container_item) = inventory
        .iter()
        .find(|item| item.bag == INVENTORY_SLOT_BAG_0 as u32 && item.slot == bag_slot)
    else {
        return Ok(None);
    };
    let container_guid = ObjectGuid::new(HighGuid::Item, 0, container_item.item);
    let contained_guid = inventory
        .iter()
        .find(|item| item.bag == bag_slot as u32 && item.slot == container_slot)
        .map(|item| ObjectGuid::new(HighGuid::Item, 0, item.item).raw())
        .unwrap_or(0);

    let mut block = Vec::new();
    block.push(UPDATE_TYPE_VALUES);
    PackedGuid::write(&mut block, container_guid)?;
    let mut values = vec![None; CONTAINER_END_FIELDS];
    let field = CONTAINER_FIELD_SLOT_1 + container_slot as usize * 2;
    set_update_value(&mut values, field, contained_guid as u32)?;
    set_update_value(&mut values, field + 1, (contained_guid >> 32) as u32)?;
    write_update_values(&mut block, &values)?;
    Ok(Some(block))
}

fn item_contained_guid(
    owner_guid: ObjectGuid,
    inventory: &[CharacterInventoryItem],
    item: &CharacterInventoryItem,
) -> ObjectGuid {
    if item.bag == INVENTORY_SLOT_BAG_0 as u32 {
        return owner_guid;
    }
    inventory
        .iter()
        .find(|container| {
            container.bag == INVENTORY_SLOT_BAG_0 as u32 && container.slot == item.bag as u8
        })
        .map(|container| ObjectGuid::new(HighGuid::Item, 0, container.item))
        .unwrap_or(owner_guid)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct StarterItemVisual {
    display_id: u32,
    inventory_type: u8,
}

fn parse_equipment_cache(cache: Option<&str>) -> [u32; ENUM_EQUIPMENT_SLOTS] {
    let mut equipment = [0u32; ENUM_EQUIPMENT_SLOTS];
    let Some(cache) = cache else {
        return equipment;
    };

    for (slot, chunk) in cache
        .split_whitespace()
        .filter_map(|value| value.parse::<u32>().ok())
        .collect::<Vec<_>>()
        .chunks(2)
        .take(ENUM_EQUIPMENT_SLOTS)
        .enumerate()
    {
        if let Some(item_id) = chunk.first() {
            equipment[slot] = *item_id;
        }
    }

    equipment
}

fn starter_item_visual(item_id: u32) -> Option<StarterItemVisual> {
    match item_id {
        25 => Some(StarterItemVisual {
            display_id: 1542,
            inventory_type: 21,
        }),
        38 => Some(StarterItemVisual {
            display_id: 9891,
            inventory_type: 4,
        }),
        39 => Some(StarterItemVisual {
            display_id: 9892,
            inventory_type: 7,
        }),
        40 => Some(StarterItemVisual {
            display_id: 10141,
            inventory_type: 8,
        }),
        2362 => Some(StarterItemVisual {
            display_id: 18730,
            inventory_type: 14,
        }),
        _ => None,
    }
}

fn faction_for_race(race: u8) -> u32 {
    match race {
        1 | 3 | 4 | 7 => 1,
        2 | 5 | 6 | 8 => 2,
        _ => 1,
    }
}

fn display_id_for_character(character: &CharacterEnumEntry) -> u32 {
    match (character.race, character.gender) {
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

