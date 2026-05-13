use super::*;
use wow_proto::{
    FactionStandingResponse, ServerWorldPacket, SmsgAccountDataTimesResponse,
    SmsgActionButtonsResponse, SmsgBindpointUpdateResponse, SmsgInitWorldStatesResponse,
    SmsgInitialSpellsResponse, SmsgInitializeFactionsResponse, SmsgLoginSetTimeSpeedResponse,
    SmsgLoginVerifyWorldResponse, SmsgSetProficiencyResponse, SmsgSetRestStartResponse,
    SmsgTriggerCinematicResponse, SmsgTutorialFlagsResponse,
};

// CMaNGOS reference: src/game/Server/WorldSession.cpp login/bootstrap packet path.
pub(in crate::world) struct EnterWorldBootstrap<'a> {
    pub(in crate::world) character_db_pool: &'a MySqlPool,
    pub(in crate::world) world_db_pool: &'a MySqlPool,
    pub(in crate::world) character: &'a CharacterEnumEntry,
    pub(in crate::world) inventory: &'a [CharacterInventoryItem],
    pub(in crate::world) inventory_container_slots: &'a HashMap<u32, u32>,
    pub(in crate::world) base_world_stats: &'a PlayerWorldStats,
    pub(in crate::world) world_stats: &'a PlayerWorldStats,
    pub(in crate::world) equipped_templates: &'a [EquippedItemTemplate],
    pub(in crate::world) ammo_template: Option<&'a ItemTemplateQuery>,
    pub(in crate::world) spells: &'a [CharacterSpell],
    pub(in crate::world) skills: &'a [CharacterSkill],
    pub(in crate::world) reputations: &'a [CharacterReputation],
    pub(in crate::world) quest_statuses: &'a HashMap<u32, CharacterQuestStatus>,
    pub(in crate::world) active_auras: &'a [ActiveAura],
    pub(in crate::world) account_data: &'a HashMap<u32, AccountDataCache>,
    pub(in crate::world) tutorial_flags: &'a [u32; 8],
    pub(in crate::world) cinematic_sequence: Option<u32>,
    pub(in crate::world) nearby_creatures: &'a [DbCreatureRuntime],
    pub(in crate::world) nearby_gameobjects: &'a [DbGameObjectRuntime],
    pub(in crate::world) nearby_player_corpses: &'a [PlayerCorpseRuntime],
}

pub(in crate::world) async fn send_enter_world_bootstrap(
    stream: &mut WorldPacketSink,
    bootstrap: EnterWorldBootstrap<'_>,
    header_crypto: Option<&mut HeaderCrypto>,
) -> anyhow::Result<()> {
    let mut header_crypto = header_crypto;
    send_login_verify_world(stream, bootstrap.character, header_crypto.as_deref_mut()).await?;
    send_account_data_times(stream, bootstrap.account_data, header_crypto.as_deref_mut()).await?;
    send_set_rest_start(stream, header_crypto.as_deref_mut()).await?;
    send_bindpoint_update(stream, bootstrap.character, header_crypto.as_deref_mut()).await?;
    send_known_proficiencies(
        stream,
        bootstrap.world_db_pool,
        bootstrap.spells,
        header_crypto.as_deref_mut(),
    )
    .await?;
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
    send_initial_reputations(stream, bootstrap.reputations, header_crypto.as_deref_mut()).await?;
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
            inventory_container_slots: bootstrap.inventory_container_slots,
            base_world_stats: bootstrap.base_world_stats,
            world_stats: bootstrap.world_stats,
            skills: bootstrap.skills,
            quest_statuses: bootstrap.quest_statuses,
            equipped_templates: bootstrap.equipped_templates,
            ammo_template: bootstrap.ammo_template,
            active_auras: bootstrap.active_auras,
            nearby_creatures: bootstrap.nearby_creatures,
            nearby_gameobjects: bootstrap.nearby_gameobjects,
            nearby_player_corpses: bootstrap.nearby_player_corpses,
        },
        header_crypto,
    )
    .await?;
    Ok(())
}

pub(in crate::world) async fn send_set_rest_start(
    stream: &mut WorldPacketSink,
    header_crypto: Option<&mut HeaderCrypto>,
) -> anyhow::Result<()> {
    send_packet(
        stream,
        SMSG_SET_REST_START,
        &build_set_rest_start_body(),
        header_crypto,
    )
    .await
}

pub(in crate::world) fn build_set_rest_start_body() -> Vec<u8> {
    SmsgSetRestStartResponse { rest_start: 0 }.body()
}

pub(in crate::world) async fn send_login_verify_world(
    stream: &mut WorldPacketSink,
    character: &CharacterEnumEntry,
    header_crypto: Option<&mut HeaderCrypto>,
) -> anyhow::Result<()> {
    let body = build_login_verify_world_body(character);
    send_packet(stream, SMSG_LOGIN_VERIFY_WORLD, &body, header_crypto).await
}

pub(in crate::world) async fn send_account_data_times(
    stream: &mut WorldPacketSink,
    account_data: &HashMap<u32, AccountDataCache>,
    header_crypto: Option<&mut HeaderCrypto>,
) -> anyhow::Result<()> {
    let body = build_account_data_times_body(account_data);
    send_packet(stream, SMSG_ACCOUNT_DATA_TIMES, &body, header_crypto).await
}

pub(in crate::world) async fn send_bindpoint_update(
    stream: &mut WorldPacketSink,
    character: &CharacterEnumEntry,
    header_crypto: Option<&mut HeaderCrypto>,
) -> anyhow::Result<()> {
    let body = build_bindpoint_update_body(character);
    send_packet(stream, SMSG_BINDPOINTUPDATE, &body, header_crypto).await
}

pub(in crate::world) fn build_login_verify_world_body(character: &CharacterEnumEntry) -> Vec<u8> {
    SmsgLoginVerifyWorldResponse {
        map: character.map,
        x: character.position_x,
        y: character.position_y,
        z: character.position_z,
        orientation: character.orientation,
    }
    .body()
}

pub(in crate::world) fn build_bindpoint_update_body(character: &CharacterEnumEntry) -> Vec<u8> {
    SmsgBindpointUpdateResponse {
        x: character.position_x,
        y: character.position_y,
        z: character.position_z,
        map: character.map,
        zone: character.zone,
    }
    .body()
}

pub(in crate::world) fn build_account_data_times_body(
    account_data: &HashMap<u32, AccountDataCache>,
) -> Vec<u8> {
    let mut digests = Vec::with_capacity(ACCOUNT_DATA_TYPES);
    for data_type in 0..ACCOUNT_DATA_TYPES as u32 {
        if let Some(entry) = account_data
            .get(&data_type)
            .filter(|entry| !entry.data.is_empty())
        {
            let mut digest = Md5::new();
            digest.update(&entry.data);
            let finalized = digest.finalize();
            let mut bytes = [0u8; MD5_DIGEST_LEN];
            bytes.copy_from_slice(&finalized);
            digests.push(bytes);
        } else {
            digests.push([0; MD5_DIGEST_LEN]);
        }
    }
    SmsgAccountDataTimesResponse { digests }.body()
}

pub(in crate::world) async fn send_tutorial_flags(
    stream: &mut WorldPacketSink,
    tutorial_flags: &[u32; 8],
    header_crypto: Option<&mut HeaderCrypto>,
) -> anyhow::Result<()> {
    let body = build_tutorial_flags_body(tutorial_flags);
    send_packet(stream, SMSG_TUTORIAL_FLAGS, &body, header_crypto).await
}

pub(in crate::world) fn build_tutorial_flags_body(tutorial_flags: &[u32; 8]) -> Vec<u8> {
    SmsgTutorialFlagsResponse {
        flags: *tutorial_flags,
    }
    .body()
}

pub(in crate::world) async fn handle_tutorial_flag(
    character_db_pool: &MySqlPool,
    account_id: u32,
    request: wow_proto::TutorialFlagRequest,
) -> anyhow::Result<()> {
    let flag = request.flag;
    let mut tutorials = wow_db::get_tutorial_flags(character_db_pool, account_id).await?;
    if !apply_tutorial_flag(&mut tutorials, flag) {
        warn!(account_id, flag, "Ignoring out-of-range tutorial flag");
        return Ok(());
    }

    wow_db::save_tutorial_flags(character_db_pool, account_id, tutorials).await?;
    Ok(())
}

pub(in crate::world) fn apply_tutorial_flag(tutorials: &mut [u32; 8], flag: u32) -> bool {
    let index = (flag / 32) as usize;
    if index >= tutorials.len() {
        return false;
    }

    tutorials[index] |= 1u32 << (flag % 32);
    true
}

pub(in crate::world) async fn handle_tutorial_clear(
    character_db_pool: &MySqlPool,
    account_id: u32,
) -> anyhow::Result<()> {
    wow_db::save_tutorial_flags(character_db_pool, account_id, [u32::MAX; 8]).await?;
    Ok(())
}

pub(in crate::world) async fn handle_tutorial_reset(
    character_db_pool: &MySqlPool,
    account_id: u32,
) -> anyhow::Result<()> {
    wow_db::save_tutorial_flags(character_db_pool, account_id, [0; 8]).await?;
    Ok(())
}

pub(in crate::world) async fn send_initial_spells(
    stream: &mut WorldPacketSink,
    spells: &[CharacterSpell],
    header_crypto: Option<&mut HeaderCrypto>,
) -> anyhow::Result<()> {
    let body = build_initial_spells_body(spells);
    send_packet(stream, SMSG_INITIAL_SPELLS, &body, header_crypto).await
}

pub(in crate::world) async fn send_known_proficiencies(
    stream: &mut WorldPacketSink,
    world_db_pool: &MySqlPool,
    spells: &[CharacterSpell],
    mut header_crypto: Option<&mut HeaderCrypto>,
) -> anyhow::Result<()> {
    let (weapon_mask, armor_mask) = known_proficiency_masks(world_db_pool, spells).await?;
    if weapon_mask != 0 {
        send_packet(
            stream,
            SMSG_SET_PROFICIENCY,
            &build_set_proficiency_body(ITEM_CLASS_WEAPON, weapon_mask),
            reborrow_header_crypto(&mut header_crypto),
        )
        .await?;
    }
    if armor_mask != 0 {
        send_packet(
            stream,
            SMSG_SET_PROFICIENCY,
            &build_set_proficiency_body(ITEM_CLASS_ARMOR, armor_mask),
            reborrow_header_crypto(&mut header_crypto),
        )
        .await?;
    }
    Ok(())
}

pub(in crate::world) fn reborrow_header_crypto<'a>(
    header_crypto: &'a mut Option<&mut HeaderCrypto>,
) -> Option<&'a mut HeaderCrypto> {
    match header_crypto {
        Some(crypto) => Some(&mut **crypto),
        None => None,
    }
}

pub(in crate::world) async fn known_proficiency_masks(
    world_db_pool: &MySqlPool,
    spells: &[CharacterSpell],
) -> anyhow::Result<(u32, u32)> {
    let mut weapon_mask = 0u32;
    let mut armor_mask = 0u32;
    for spell in spells
        .iter()
        .filter(|spell| spell.active != 0 && spell.disabled == 0)
    {
        let Some(template) = wow_db::get_spell_template_query(world_db_pool, spell.spell).await?
        else {
            continue;
        };
        if !spell_template_has_proficiency_effect(&template) {
            continue;
        }
        add_template_proficiency_masks(&template, &mut weapon_mask, &mut armor_mask);
    }
    Ok((weapon_mask, armor_mask))
}

pub(in crate::world) fn add_template_proficiency_masks(
    template: &wow_db::SpellTemplateQuery,
    weapon_mask: &mut u32,
    armor_mask: &mut u32,
) {
    match template.equipped_item_class {
        class if class == ITEM_CLASS_WEAPON as i32 => {
            *weapon_mask |= template.equipped_item_subclass_mask as u32;
        }
        class if class == ITEM_CLASS_ARMOR as i32 => {
            *armor_mask |= template.equipped_item_subclass_mask as u32;
        }
        _ => {}
    }
}

pub(in crate::world) fn spell_template_has_proficiency_effect(
    template: &wow_db::SpellTemplateQuery,
) -> bool {
    const SPELL_EFFECT_PROFICIENCY: u32 = 60;
    [template.effect1, template.effect2, template.effect3].contains(&SPELL_EFFECT_PROFICIENCY)
}

pub(in crate::world) fn build_set_proficiency_body(
    item_class: u32,
    item_subclass_mask: u32,
) -> Vec<u8> {
    SmsgSetProficiencyResponse {
        item_class: item_class as u8,
        item_subclass_mask,
    }
    .body()
}

pub(in crate::world) fn build_initial_spells_body(spells: &[CharacterSpell]) -> Vec<u8> {
    let spells = spells
        .iter()
        .filter(|spell| spell.active != 0 && spell.disabled == 0)
        .map(|spell| spell.spell)
        .collect();
    SmsgInitialSpellsResponse { spells }.body()
}

pub(in crate::world) async fn send_action_buttons(
    stream: &mut WorldPacketSink,
    actions: &[CharacterAction],
    header_crypto: Option<&mut HeaderCrypto>,
) -> anyhow::Result<()> {
    let body = build_action_buttons_body(actions);
    send_packet(stream, SMSG_ACTION_BUTTONS, &body, header_crypto).await
}

pub(in crate::world) fn build_action_buttons_body(actions: &[CharacterAction]) -> Vec<u8> {
    let mut buttons = vec![0u32; MAX_ACTION_BUTTONS];
    for action in actions {
        if (action.button as usize) < MAX_ACTION_BUTTONS {
            buttons[action.button as usize] = pack_action_button(action.action, action.action_type);
        }
    }

    SmsgActionButtonsResponse { buttons }.body()
}

pub(in crate::world) fn pack_action_button(action: u32, action_type: u8) -> u32 {
    action | ((action_type as u32) << 24)
}

pub(in crate::world) async fn send_initial_reputations(
    stream: &mut WorldPacketSink,
    reputations: &[CharacterReputation],
    header_crypto: Option<&mut HeaderCrypto>,
) -> anyhow::Result<()> {
    let body = build_initial_reputations_body(reputations);
    send_packet(stream, SMSG_INITIALIZE_FACTIONS, &body, header_crypto).await
}

pub(in crate::world) fn build_initial_reputations_body(
    reputations: &[CharacterReputation],
) -> Vec<u8> {
    let mut slots = vec![(0u8, 0i32); REPUTATION_LIST_SLOTS];
    for reputation in reputations {
        let Some(slot) = reputation_list_slot_for_faction(reputation.faction) else {
            continue;
        };
        if slot < REPUTATION_LIST_SLOTS {
            slots[slot] = (reputation.flags as u8, reputation.standing);
        }
    }

    SmsgInitializeFactionsResponse {
        slots: slots
            .into_iter()
            .map(|(flags, standing)| FactionStandingResponse { flags, standing })
            .collect(),
    }
    .body()
}

pub(in crate::world) async fn send_trigger_cinematic(
    stream: &mut WorldPacketSink,
    sequence: u32,
    header_crypto: Option<&mut HeaderCrypto>,
) -> anyhow::Result<()> {
    let body = build_trigger_cinematic_body(sequence);
    send_packet(stream, SMSG_TRIGGER_CINEMATIC, &body, header_crypto).await
}

pub(in crate::world) fn build_trigger_cinematic_body(sequence: u32) -> Vec<u8> {
    SmsgTriggerCinematicResponse { sequence }.body()
}

pub(in crate::world) fn cinematic_sequence_for_race(race: u8) -> Option<u32> {
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

pub(in crate::world) async fn send_login_set_time_speed(
    stream: &mut WorldPacketSink,
    header_crypto: Option<&mut HeaderCrypto>,
) -> anyhow::Result<()> {
    let body = SmsgLoginSetTimeSpeedResponse {
        packed_server_time: 0,
        game_speed: 0.01666667,
    }
    .body();
    send_packet(stream, SMSG_LOGIN_SETTIMESPEED, &body, header_crypto).await
}

pub(in crate::world) async fn send_init_world_states(
    stream: &mut WorldPacketSink,
    character: &CharacterEnumEntry,
    header_crypto: Option<&mut HeaderCrypto>,
) -> anyhow::Result<()> {
    let body = SmsgInitWorldStatesResponse {
        map: character.map,
        zone: character.zone,
        area: 0,
        states: Vec::new(),
    }
    .body();
    send_packet(stream, SMSG_INIT_WORLD_STATES, &body, header_crypto).await
}
