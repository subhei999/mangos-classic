// CMaNGOS reference: src/game/Server/WorldSession.cpp login/bootstrap packet path.
struct EnterWorldBootstrap<'a> {
    character_db_pool: &'a MySqlPool,
    character: &'a CharacterEnumEntry,
    inventory: &'a [CharacterInventoryItem],
    world_stats: &'a PlayerWorldStats,
    equipped_templates: &'a [EquippedItemTemplate],
    spells: &'a [CharacterSpell],
    skills: &'a [CharacterSkill],
    quest_statuses: &'a HashMap<u32, CharacterQuestStatus>,
    tutorial_flags: &'a [u32; 8],
    cinematic_sequence: Option<u32>,
    nearby_creatures: &'a [DbCreatureRuntime],
    nearby_gameobjects: &'a [DbGameObjectRuntime],
    nearby_player_corpses: &'a [PlayerCorpseRuntime],
}

async fn send_enter_world_bootstrap(
    stream: &mut WorldPacketSink,
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
    send_initial_spells(stream, bootstrap.spells, header_crypto.as_deref_mut()).await?;
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
        SelfSpawnUpdate {
            character: bootstrap.character,
            inventory: bootstrap.inventory,
            world_stats: bootstrap.world_stats,
            skills: bootstrap.skills,
            quest_statuses: bootstrap.quest_statuses,
            equipped_templates: bootstrap.equipped_templates,
            nearby_creatures: bootstrap.nearby_creatures,
            nearby_gameobjects: bootstrap.nearby_gameobjects,
            nearby_player_corpses: bootstrap.nearby_player_corpses,
        },
        header_crypto,
    )
    .await?;
    Ok(())
}

async fn send_login_verify_world(
    stream: &mut WorldPacketSink,
    character: &CharacterEnumEntry,
    header_crypto: Option<&mut HeaderCrypto>,
) -> anyhow::Result<()> {
    let body = build_login_verify_world_body(character);
    send_packet(stream, SMSG_LOGIN_VERIFY_WORLD, &body, header_crypto).await
}

async fn send_account_data_times(
    stream: &mut WorldPacketSink,
    header_crypto: Option<&mut HeaderCrypto>,
) -> anyhow::Result<()> {
    let body = build_account_data_times_body();
    send_packet(stream, SMSG_ACCOUNT_DATA_TIMES, &body, header_crypto).await
}

async fn send_bindpoint_update(
    stream: &mut WorldPacketSink,
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
    stream: &mut WorldPacketSink,
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
    stream: &mut WorldPacketSink,
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
    stream: &mut WorldPacketSink,
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
    stream: &mut WorldPacketSink,
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

async fn send_trigger_cinematic(
    stream: &mut WorldPacketSink,
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
    stream: &mut WorldPacketSink,
    header_crypto: Option<&mut HeaderCrypto>,
) -> anyhow::Result<()> {
    let mut body = Vec::with_capacity(8);
    body.extend_from_slice(&0u32.to_le_bytes()); // packed server time placeholder
    body.extend_from_slice(&0.01666667f32.to_le_bytes());
    send_packet(stream, SMSG_LOGIN_SETTIMESPEED, &body, header_crypto).await
}

async fn send_init_world_states(
    stream: &mut WorldPacketSink,
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

