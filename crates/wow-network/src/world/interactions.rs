async fn handle_ping(
    stream: &mut TcpStream,
    body: &[u8],
    header_crypto: Option<&mut HeaderCrypto>,
) -> anyhow::Result<()> {
    if body.len() < 4 {
        anyhow::bail!("CMSG_PING payload too short: {} bytes", body.len());
    }

    let ping = u32::from_le_bytes(body[0..4].try_into()?);
    send_packet(stream, SMSG_PONG, &ping.to_le_bytes(), header_crypto).await
}

async fn handle_name_query(
    stream: &mut TcpStream,
    character_db_pool: &MySqlPool,
    body: &[u8],
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    if body.len() != 8 {
        anyhow::bail!(
            "CMSG_NAME_QUERY payload must be 8 bytes, got {}",
            body.len()
        );
    }

    let raw_guid = u64::from_le_bytes(body.try_into()?);
    let guid = ObjectGuid::from_raw(raw_guid);
    let character_guid = guid.counter();
    let character = wow_db::get_character_name_query(character_db_pool, character_guid).await?;
    let response = build_name_query_response(raw_guid, character.as_ref());
    send_packet(
        stream,
        SMSG_NAME_QUERY_RESPONSE,
        &response,
        Some(header_crypto),
    )
    .await
}

fn build_name_query_response(
    requested_guid: u64,
    character: Option<&CharacterNameQuery>,
) -> Vec<u8> {
    let mut body = Vec::with_capacity(8 + 1 + 1 + 12);
    match character {
        Some(character) => {
            let guid = ObjectGuid::new(HighGuid::Player, 0, character.guid);
            body.extend_from_slice(&guid.raw().to_le_bytes());
            write_c_string(&mut body, &character.name);
            body.push(0); // realm name
            body.extend_from_slice(&(character.race as u32).to_le_bytes());
            body.extend_from_slice(&(character.gender as u32).to_le_bytes());
            body.extend_from_slice(&(character.class as u32).to_le_bytes());
        }
        None => {
            body.extend_from_slice(&requested_guid.to_le_bytes());
            write_c_string(&mut body, "Unknown");
            body.push(0); // realm name
            body.extend_from_slice(&0u32.to_le_bytes());
            body.extend_from_slice(&0u32.to_le_bytes());
            body.extend_from_slice(&0u32.to_le_bytes());
        }
    }
    body
}

async fn handle_message_chat(
    stream: &mut TcpStream,
    body: &[u8],
    session: &WorldSessionState,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    let chat = ChatMessage::read(body)?;
    if !matches!(
        chat.chat_type,
        CHAT_MSG_SAY | CHAT_MSG_YELL | CHAT_MSG_EMOTE
    ) {
        info!(
            chat_type = chat.chat_type,
            language = chat.language,
            "Ignoring unsupported chat message type"
        );
        return Ok(());
    }

    let Some(character) = &session.active_character else {
        warn!(
            chat_type = chat.chat_type,
            "Ignoring chat before character login"
        );
        return Ok(());
    };
    if chat.message.is_empty() {
        return Ok(());
    }

    let body = build_message_chat_body(chat.chat_type, chat.language, &chat.message, character);
    send_packet(stream, SMSG_MESSAGECHAT, &body, Some(header_crypto)).await
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ChatMessage {
    chat_type: u32,
    language: u32,
    message: String,
}

impl ChatMessage {
    fn read(body: &[u8]) -> anyhow::Result<Self> {
        let mut cursor = 0;
        let chat_type = read_u32(body, &mut cursor)?;
        let language = read_u32(body, &mut cursor)?;
        let message = read_c_string(body, &mut cursor)?;
        Ok(Self {
            chat_type,
            language,
            message,
        })
    }
}

fn build_message_chat_body(
    chat_type: u32,
    language: u32,
    message: &str,
    character: &ActiveCharacter,
) -> Vec<u8> {
    let sender = ObjectGuid::new(HighGuid::Player, 0, character.guid);
    let mut body = Vec::with_capacity(1 + 4 + 16 + 4 + message.len() + 2);
    body.push(chat_type as u8);
    body.extend_from_slice(&language.to_le_bytes());
    match chat_type {
        CHAT_MSG_SAY | CHAT_MSG_YELL => {
            body.extend_from_slice(&sender.raw().to_le_bytes());
            body.extend_from_slice(&sender.raw().to_le_bytes());
        }
        _ => {
            body.extend_from_slice(&sender.raw().to_le_bytes());
        }
    }
    body.extend_from_slice(&((message.len() + 1) as u32).to_le_bytes());
    write_c_string(&mut body, message);
    body.push(CHAT_TAG_NONE);
    body
}

async fn handle_text_emote(
    stream: &mut TcpStream,
    body: &[u8],
    session: &WorldSessionState,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    let emote = TextEmote::read(body)?;
    let Some(character) = &session.active_character else {
        warn!(
            text_emote = emote.text_emote,
            "Ignoring text emote before character login"
        );
        return Ok(());
    };
    if let Some(animation) = animation_emote_for_text_emote(emote.text_emote) {
        if matches!(emote.text_emote, TEXTEMOTE_DANCE | TEXTEMOTE_SLEEP) {
            let body = build_emote_state_update_body(character, animation)?;
            send_packet(stream, SMSG_UPDATE_OBJECT, &body, Some(header_crypto)).await?;
        } else {
            let body = build_emote_body(character, animation);
            send_packet(stream, SMSG_EMOTE, &body, Some(header_crypto)).await?;
        }
    }
    let body = build_text_emote_body(character, emote.text_emote, emote.emote_num);
    send_packet(stream, SMSG_TEXT_EMOTE, &body, Some(header_crypto)).await
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct TextEmote {
    text_emote: u32,
    emote_num: u32,
    target_guid: u64,
}

impl TextEmote {
    fn read(body: &[u8]) -> anyhow::Result<Self> {
        let mut cursor = 0;
        let text_emote = read_u32(body, &mut cursor)?;
        let emote_num = read_u32(body, &mut cursor)?;
        ensure_available(body, cursor + 8)?;
        let target_guid = u64::from_le_bytes(body[cursor..cursor + 8].try_into()?);
        Ok(Self {
            text_emote,
            emote_num,
            target_guid,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CastSpellPacket {
    spell_id: u32,
    targets: SpellCastTargets,
}

impl CastSpellPacket {
    fn read(body: &[u8]) -> anyhow::Result<Self> {
        let mut cursor = 0;
        let spell_id = read_u32(body, &mut cursor)?;
        let targets = SpellCastTargets::read(body, &mut cursor)?;
        Ok(Self { spell_id, targets })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SpellCastTargets {
    target_mask: u16,
    unit_target: Option<ObjectGuid>,
}

impl SpellCastTargets {
    fn read(body: &[u8], cursor: &mut usize) -> anyhow::Result<Self> {
        let target_mask = read_u16(body, cursor)?;
        let unit_target =
            if target_mask & (SPELL_CAST_TARGET_UNIT | SPELL_CAST_TARGET_UNIT_ENEMY) != 0 {
                Some(read_packed_guid(body, cursor)?)
            } else {
                None
            };

        Ok(Self {
            target_mask,
            unit_target,
        })
    }

    fn write(&self, body: &mut Vec<u8>) -> anyhow::Result<()> {
        let target_mask = self.target_mask & !SPELL_CAST_TARGET_UNIT_ENEMY;
        body.extend_from_slice(&target_mask.to_le_bytes());
        if target_mask & SPELL_CAST_TARGET_UNIT != 0 {
            PackedGuid::write(body, self.unit_target.unwrap_or(ObjectGuid::EMPTY))?;
        }
        Ok(())
    }
}

fn build_text_emote_body(character: &ActiveCharacter, text_emote: u32, emote_num: u32) -> Vec<u8> {
    let sender = ObjectGuid::new(HighGuid::Player, 0, character.guid);
    let mut body = Vec::with_capacity(8 + 4 + 4 + 4 + 1);
    body.extend_from_slice(&sender.raw().to_le_bytes());
    body.extend_from_slice(&text_emote.to_le_bytes());
    body.extend_from_slice(&emote_num.to_le_bytes());
    body.extend_from_slice(&1u32.to_le_bytes());
    body.push(0);
    body
}

fn animation_emote_for_text_emote(text_emote: u32) -> Option<u32> {
    match text_emote {
        TEXTEMOTE_WAVE => Some(EMOTE_ONESHOT_WAVE),
        TEXTEMOTE_POINT => Some(EMOTE_ONESHOT_POINT),
        TEXTEMOTE_DANCE => Some(EMOTE_STATE_DANCE),
        TEXTEMOTE_SLEEP => Some(EMOTE_STATE_SLEEP),
        _ => None,
    }
}

fn build_emote_body(character: &ActiveCharacter, emote: u32) -> Vec<u8> {
    let sender = ObjectGuid::new(HighGuid::Player, 0, character.guid);
    let mut body = Vec::with_capacity(12);
    body.extend_from_slice(&emote.to_le_bytes());
    body.extend_from_slice(&sender.raw().to_le_bytes());
    body
}

fn build_emote_state_update_body(
    character: &ActiveCharacter,
    emote: u32,
) -> anyhow::Result<Vec<u8>> {
    let guid = ObjectGuid::new(HighGuid::Player, 0, character.guid);
    let mut block = Vec::new();
    block.push(UPDATE_TYPE_VALUES);
    PackedGuid::write(&mut block, guid)?;
    let mut values = vec![None; PLAYER_END_FIELDS];
    set_update_value(&mut values, UNIT_NPC_EMOTESTATE, emote)?;
    write_update_values(&mut block, &values)?;

    let mut body = Vec::with_capacity(5 + block.len());
    body.extend_from_slice(&1u32.to_le_bytes());
    body.push(0);
    body.extend_from_slice(&block);
    Ok(body)
}

async fn handle_cast_spell(
    stream: &mut TcpStream,
    body: &[u8],
    session: &mut WorldSessionState,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    let packet = CastSpellPacket::read(body)?;
    let Some(character) = &session.active_character else {
        warn!(
            spell_id = packet.spell_id,
            "Ignoring spell cast before character login"
        );
        return Ok(());
    };
    let character_guid = character.guid;

    let Some(starter_spell) = supported_starter_spell(packet.spell_id) else {
        warn!(
            spell_id = packet.spell_id,
            "Ignoring unsupported spell cast in starter spell fixture slice"
        );
        return Ok(());
    };
    if !session.active_spells.contains(&packet.spell_id) {
        warn!(
            spell_id = packet.spell_id,
            character_guid,
            "Ignoring starter spell cast for spell not active on character"
        );
        return Ok(());
    }

    let caster = ObjectGuid::new(HighGuid::Player, 0, character_guid);
    let targets = normalize_fixture_spell_targets(packet.targets);
    match starter_spell.power {
        StarterSpellPower::Rage { cost } => {
            session.player_rage = session.player_rage.saturating_sub(cost);
        }
        StarterSpellPower::Mana { cost } => {
            session.player_mana = session.player_mana.saturating_sub(cost);
        }
    }
    send_packet(
        stream,
        SMSG_CAST_RESULT,
        &build_cast_result_ok_body(packet.spell_id),
        Some(&mut *header_crypto),
    )
    .await?;
    send_packet(
        stream,
        SMSG_SPELL_GO,
        &build_spell_go_body(caster, packet.spell_id, &targets)?,
        Some(&mut *header_crypto),
    )
    .await?;
    if targets.unit_target == Some(rust_combat_dummy_guid())
        && !session.combat_dummy_lootable
        && session.combat_dummy_health > 0
    {
        let damage = session.combat_dummy_health.min(starter_spell.damage);
        session.combat_dummy_health = session.combat_dummy_health.saturating_sub(damage);
        if session.combat_dummy_health == 0 {
            session.combat_dummy_lootable = true;
            session.combat_dummy_looting = false;
            session.combat_dummy_loot_money_available = true;
            session.combat_dummy_loot_item_available = true;
            session.active_combat_target = None;
        }
        send_packet(
            stream,
            SMSG_ATTACKERSTATEUPDATE,
            &build_attacker_state_update_body_with_spell_id(
                caster,
                rust_combat_dummy_guid(),
                damage,
                packet.spell_id,
            )?,
            Some(&mut *header_crypto),
        )
        .await?;
        send_packet(
            stream,
            SMSG_UPDATE_OBJECT,
            &build_combat_dummy_state_update_body(
                session.combat_dummy_health,
                if session.combat_dummy_health == 0 {
                    UNIT_DYNFLAG_LOOTABLE
                } else {
                    0
                },
            )?,
            Some(&mut *header_crypto),
        )
        .await?;
    } else if let Some(target) = targets.unit_target {
        if let Some(damage) = apply_db_creature_damage(session, target, starter_spell.damage) {
            let (health, dynamic_flags) = session
                .db_creatures
                .get(&target.raw())
                .map(|creature| (creature.health, creature.dynamic_flags()))
                .expect("creature damage target checked above");
            send_packet(
                stream,
                SMSG_ATTACKERSTATEUPDATE,
                &build_attacker_state_update_body_with_spell_id(
                    caster,
                    target,
                    damage,
                    packet.spell_id,
                )?,
                Some(&mut *header_crypto),
            )
            .await?;
            send_packet(
                stream,
                SMSG_UPDATE_OBJECT,
                &build_db_creature_state_update_body(target, health, dynamic_flags)?,
                Some(&mut *header_crypto),
            )
            .await?;
        }
    }
    let power_update = match starter_spell.power {
        StarterSpellPower::Rage { .. } => build_player_rage_update_body(caster, session.player_rage)?,
        StarterSpellPower::Mana { .. } => build_player_mana_update_body(caster, session.player_mana)?,
    };
    send_packet(stream, SMSG_UPDATE_OBJECT, &power_update, Some(header_crypto)).await
}

fn normalize_fixture_spell_targets(mut targets: SpellCastTargets) -> SpellCastTargets {
    targets.target_mask =
        (targets.target_mask | SPELL_CAST_TARGET_UNIT) & !SPELL_CAST_TARGET_UNIT_ENEMY;
    targets.unit_target = Some(targets.unit_target.unwrap_or_else(rust_combat_dummy_guid));
    targets
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct SupportedStarterSpell {
    damage: u32,
    power: StarterSpellPower,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StarterSpellPower {
    Rage { cost: u32 },
    Mana { cost: u32 },
}

fn supported_starter_spell(spell_id: u32) -> Option<SupportedStarterSpell> {
    match spell_id {
        WARRIOR_HEROIC_STRIKE_RANK_1 => Some(SupportedStarterSpell {
            damage: HEROIC_STRIKE_FIXTURE_DAMAGE,
            power: StarterSpellPower::Rage {
                cost: HEROIC_STRIKE_RAGE_COST,
            },
        }),
        HUNTER_RAPTOR_STRIKE_RANK_1 => Some(SupportedStarterSpell {
            damage: RAPTOR_STRIKE_FIXTURE_DAMAGE,
            power: StarterSpellPower::Mana {
                cost: RAPTOR_STRIKE_MANA_COST,
            },
        }),
        _ => None,
    }
}

fn build_cast_result_ok_body(spell_id: u32) -> Vec<u8> {
    let mut body = Vec::with_capacity(5);
    body.extend_from_slice(&spell_id.to_le_bytes());
    body.push(0);
    body
}

fn build_spell_go_body(
    caster: ObjectGuid,
    spell_id: u32,
    targets: &SpellCastTargets,
) -> anyhow::Result<Vec<u8>> {
    let mut body = Vec::with_capacity(40);
    PackedGuid::write(&mut body, caster)?;
    PackedGuid::write(&mut body, caster)?;
    body.extend_from_slice(&spell_id.to_le_bytes());
    body.extend_from_slice(&CAST_FLAG_SPELL_GO.to_le_bytes());

    if let Some(target) = targets.unit_target {
        body.push(1);
        body.extend_from_slice(&target.raw().to_le_bytes());
    } else {
        body.push(0);
    }
    body.push(0); // miss count
    targets.write(&mut body)?;
    Ok(body)
}

async fn handle_item_query_single(
    stream: &mut TcpStream,
    world_db_pool: &MySqlPool,
    body: &[u8],
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    if body.len() < 4 {
        anyhow::bail!(
            "CMSG_ITEM_QUERY_SINGLE payload too short: {} bytes",
            body.len()
        );
    }

    let item = u32::from_le_bytes(body[0..4].try_into()?);
    let template = wow_db::get_item_template_query(world_db_pool, item).await?;
    info!(
        item,
        found = template.is_some(),
        "Answering item template query"
    );
    let response = build_item_query_single_response(item, template.as_ref());
    send_packet(
        stream,
        SMSG_ITEM_QUERY_SINGLE_RESPONSE,
        &response,
        Some(header_crypto),
    )
    .await
}

async fn handle_inventory_swap(
    stream: &mut TcpStream,
    character_db_pool: &MySqlPool,
    world_db_pool: &MySqlPool,
    opcode: u32,
    body: &[u8],
    session: &mut WorldSessionState,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    let Some(character) = &session.active_character else {
        warn!(
            opcode = inventory_opcode_name(opcode),
            "Ignoring inventory move before character login"
        );
        return Ok(());
    };
    let character_guid = character.guid;
    let Some(move_request) = (if opcode == CMSG_AUTOEQUIP_ITEM {
        InventoryMoveRequest::read_auto_equip(body, world_db_pool, session).await?
    } else {
        Some(InventoryMoveRequest::read(opcode, body)?)
    }) else {
        info!(
            opcode = inventory_opcode_name(opcode),
            "Ignoring unsupported inventory auto-equip source"
        );
        return Ok(());
    };

    if !move_request.is_supported_inventory_move() {
        info!(
            opcode = inventory_opcode_name(opcode),
            src_bag = move_request.src_bag,
            src_slot = move_request.src_slot,
            dst_bag = move_request.dst_bag,
            dst_slot = move_request.dst_slot,
            "Ignoring unsupported inventory move outside bag-0 or equipped bag storage"
        );
        return Ok(());
    }

    if move_request.src_bag == move_request.dst_bag && move_request.src_slot == move_request.dst_slot {
        return Ok(());
    }

    let Some(src_item) = session
        .inventory
        .iter()
        .find(|item| item.bag == move_request.src_bag as u32 && item.slot == move_request.src_slot)
    else {
        warn!(
            opcode = inventory_opcode_name(opcode),
            guid = character_guid,
            src_bag = move_request.src_bag,
            src_slot = move_request.src_slot,
            dst_bag = move_request.dst_bag,
            dst_slot = move_request.dst_slot,
            "Rejected inventory move without source item"
        );
        return Ok(());
    };

    if move_request.dst_bag == INVENTORY_SLOT_BAG_0
        && (move_request.dst_slot < EQUIPMENT_SLOT_END || is_bag_slot(move_request.dst_slot))
    {
        let Some(template) =
            wow_db::get_item_template_query(world_db_pool, src_item.item_template).await?
        else {
            warn!(
                opcode = inventory_opcode_name(opcode),
                guid = character_guid,
                item_template = src_item.item_template,
                "Rejected equip move for missing item template"
            );
            return Ok(());
        };
        let fits_destination = if move_request.dst_slot < EQUIPMENT_SLOT_END {
            item_fits_equipment_slot(template.inventory_type, move_request.dst_slot)
        } else {
            template.container_slots > 0
        };
        if !fits_destination {
            info!(
                opcode = inventory_opcode_name(opcode),
                guid = character_guid,
                item_template = src_item.item_template,
                inventory_type = template.inventory_type,
                dst_slot = move_request.dst_slot,
                "Rejected inventory move for incompatible equipment/bag slot"
            );
            return Ok(());
        }
    }

    if move_request.src_bag == INVENTORY_SLOT_BAG_0
        && is_bag_slot(move_request.src_slot)
        && !is_bag_slot(move_request.dst_slot)
        && session
            .inventory
            .iter()
            .any(|item| item.bag == move_request.src_slot as u32)
    {
        info!(
            opcode = inventory_opcode_name(opcode),
            guid = character_guid,
            src_slot = move_request.src_slot,
            "Rejected moving non-empty equipped bag into non-bag storage"
        );
        return Ok(());
    }

    let dst_item = session
        .inventory
        .iter()
        .find(|item| item.bag == move_request.dst_bag as u32 && item.slot == move_request.dst_slot);
    let max_stack = if let Some(dst_item) = dst_item.filter(|item| {
        item.item_template == src_item.item_template && item.item != src_item.item
    }) {
        let Some(template) =
            wow_db::get_item_template_query(world_db_pool, dst_item.item_template).await?
        else {
            return Ok(());
        };
        Some(template.stackable)
    } else {
        None
    };

    let moved = wow_db::swap_character_inventory_slots_with_stack(
        character_db_pool,
        character_guid,
        move_request.src_bag as u32,
        move_request.src_slot,
        move_request.dst_bag as u32,
        move_request.dst_slot,
        max_stack,
    )
    .await?;
    let Some(moved) = moved else {
        warn!(
            opcode = inventory_opcode_name(opcode),
            guid = character_guid,
            src_bag = move_request.src_bag,
            src_slot = move_request.src_slot,
            dst_bag = move_request.dst_bag,
            dst_slot = move_request.dst_slot,
            "Rejected inventory move without source item"
        );
        return Ok(());
    };

    session.inventory = wow_db::get_character_inventory_items(character_db_pool, character_guid)
        .await?;
    match moved {
        wow_db::InventoryMoveResult::Swapped => {
            let blocks =
                build_inventory_move_update_blocks(character_guid, &session.inventory, &move_request)?;
            let body = build_update_object_body(&blocks);
            send_packet(stream, SMSG_UPDATE_OBJECT, &body, Some(header_crypto)).await
        }
        wow_db::InventoryMoveResult::Merged {
            source_item,
            source_count,
            destination_item,
            destination_count,
        } => {
            let mut blocks = Vec::new();
            if let Some(source_count) = source_count {
                blocks.push(build_item_stack_count_update_block(source_item, source_count)?);
            } else {
                blocks.extend(build_inventory_position_update_blocks(
                    character_guid,
                    &session.inventory,
                    move_request.src_bag,
                    move_request.src_slot,
                )?);
            }
            blocks.push(build_item_stack_count_update_block(
                destination_item,
                destination_count,
            )?);
            let body = build_update_object_body(&blocks);
            send_packet(stream, SMSG_UPDATE_OBJECT, &body, Some(header_crypto)).await
        }
    }
}

async fn handle_destroy_item(
    stream: &mut TcpStream,
    character_db_pool: &MySqlPool,
    world_db_pool: &MySqlPool,
    body: &[u8],
    session: &mut WorldSessionState,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    let Some(character) = &session.active_character else {
        warn!("Ignoring item destroy before character login");
        return Ok(());
    };
    let character_guid = character.guid;
    let request = DestroyItemRequest::read(body)?;

    if !request.is_supported_destroy() {
        info!(
            bag = request.bag,
            slot = request.slot,
            count = request.count,
            "Ignoring unsupported item destroy outside bag-0 or equipped bag storage"
        );
        return Ok(());
    }

    let Some(source_item) = session
        .inventory
        .iter()
        .find(|item| item.bag == request.bag as u32 && item.slot == request.slot)
    else {
        warn!(
            guid = character_guid,
            bag = request.bag,
            slot = request.slot,
            "Rejected item destroy without source item"
        );
        return Ok(());
    };

    let Some(template) = wow_db::get_item_template_query(world_db_pool, source_item.item_template).await?
    else {
        warn!(
            guid = character_guid,
            item_template = source_item.item_template,
            "Rejected item destroy for missing item template"
        );
        return Ok(());
    };
    if template.flags & ITEM_FLAG_NO_USER_DESTROY != 0 {
        info!(
            guid = character_guid,
            item_template = source_item.item_template,
            "Rejected no-user-destroy item destroy"
        );
        return send_inventory_change_failure(
            stream,
            EQUIP_ERR_CANT_DROP_SOULBOUND,
            None,
            None,
            header_crypto,
        )
        .await;
    }

    let destroyed = wow_db::destroy_character_inventory_item_count(
        character_db_pool,
        character_guid,
        request.bag as u32,
        request.slot,
        request.count as u32,
    )
    .await?;
    let Some(destroyed) = destroyed else {
        warn!(
            guid = character_guid,
            bag = request.bag,
            slot = request.slot,
            "Rejected item destroy without DB source item"
        );
        return Ok(());
    };

    session.inventory = wow_db::get_character_inventory_items(character_db_pool, character_guid)
        .await?;
    match destroyed {
        wow_db::InventoryDestroyResult::CountChanged { item, count } => {
            let body = build_item_stack_count_update_body(item, count)?;
            send_packet(stream, SMSG_UPDATE_OBJECT, &body, Some(header_crypto)).await
        }
        wow_db::InventoryDestroyResult::Removed { item } => {
            if request.bag == INVENTORY_SLOT_BAG_0 {
                let body =
                    build_inventory_slots_update_body(character_guid, &session.inventory, &[request.slot])?;
                send_packet(stream, SMSG_UPDATE_OBJECT, &body, Some(header_crypto)).await
            } else {
                let body = build_destroy_object_body(item);
                send_packet(stream, SMSG_DESTROY_OBJECT, &body, Some(header_crypto)).await
            }
        }
    }
}

async fn handle_split_item(
    stream: &mut TcpStream,
    character_db_pool: &MySqlPool,
    body: &[u8],
    session: &mut WorldSessionState,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    let Some(character) = &session.active_character else {
        warn!("Ignoring item split before character login");
        return Ok(());
    };
    let character_guid = character.guid;
    let request = SplitItemRequest::read(body)?;
    if !request.is_supported_split() || request.src_bag == request.dst_bag && request.src_slot == request.dst_slot {
        info!(
            src_bag = request.src_bag,
            src_slot = request.src_slot,
            dst_bag = request.dst_bag,
            dst_slot = request.dst_slot,
            count = request.count,
            "Ignoring unsupported item split outside bag-0 or equipped bag storage"
        );
        return Ok(());
    }

    let split = wow_db::split_character_inventory_item(
        character_db_pool,
        character_guid,
        request.src_bag as u32,
        request.src_slot,
        request.dst_bag as u32,
        request.dst_slot,
        request.count as u32,
    )
    .await?;
    let Some(split) = split else {
        warn!(
            guid = character_guid,
            src_bag = request.src_bag,
            src_slot = request.src_slot,
            dst_bag = request.dst_bag,
            dst_slot = request.dst_slot,
            "Rejected item split"
        );
        return send_inventory_change_failure(
            stream,
            EQUIP_ERR_COULDNT_SPLIT_ITEMS,
            None,
            None,
            header_crypto,
        )
        .await;
    };

    session.inventory = wow_db::get_character_inventory_items(character_db_pool, character_guid)
        .await?;
    let mut blocks = vec![build_item_stack_count_update_block(
        split.source_item,
        split.source_count,
    )?];
    if let Some(new_item) = session.inventory.iter().find(|item| item.item == split.new_item) {
        let owner_guid = ObjectGuid::new(HighGuid::Player, 0, character_guid);
        let contained_guid = item_contained_guid(owner_guid, &session.inventory, new_item);
        blocks.push(build_item_create_update_block(
            owner_guid,
            contained_guid,
            new_item,
            None,
        )?);
        blocks.extend(build_inventory_position_update_blocks(
            character_guid,
            &session.inventory,
            new_item.bag as u8,
            new_item.slot,
        )?);
    }
    let body = build_update_object_body(&blocks);
    send_packet(stream, SMSG_UPDATE_OBJECT, &body, Some(header_crypto)).await
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct InventoryMoveRequest {
    src_bag: u8,
    src_slot: u8,
    dst_bag: u8,
    dst_slot: u8,
}

impl InventoryMoveRequest {
    fn read(opcode: u32, body: &[u8]) -> anyhow::Result<Self> {
        match opcode {
            CMSG_SWAP_INV_ITEM => {
                if body.len() < 2 {
                    anyhow::bail!("CMSG_SWAP_INV_ITEM payload too short: {} bytes", body.len());
                }
                Ok(Self {
                    src_bag: INVENTORY_SLOT_BAG_0,
                    src_slot: body[0],
                    dst_bag: INVENTORY_SLOT_BAG_0,
                    dst_slot: body[1],
                })
            }
            CMSG_SWAP_ITEM => {
                if body.len() < 4 {
                    anyhow::bail!("CMSG_SWAP_ITEM payload too short: {} bytes", body.len());
                }
                Ok(Self {
                    dst_bag: normalize_client_bag(body[0]),
                    dst_slot: body[1],
                    src_bag: normalize_client_bag(body[2]),
                    src_slot: body[3],
                })
            }
            _ => anyhow::bail!("unsupported inventory opcode 0x{opcode:04X}"),
        }
    }

    async fn read_auto_equip(
        body: &[u8],
        world_db_pool: &MySqlPool,
        session: &WorldSessionState,
    ) -> anyhow::Result<Option<Self>> {
        if body.len() < 2 {
            anyhow::bail!("CMSG_AUTOEQUIP_ITEM payload too short: {} bytes", body.len());
        }
        let src_bag = normalize_client_bag(body[0]);
        let src_slot = body[1];
        let Some(src_item) = session
            .inventory
            .iter()
            .find(|item| item.bag == src_bag as u32 && item.slot == src_slot)
        else {
            return Ok(None);
        };
        let Some(template) =
            wow_db::get_item_template_query(world_db_pool, src_item.item_template).await?
        else {
            return Ok(None);
        };
        let Some(dst_slot) = preferred_equipment_slot(template.inventory_type) else {
            return Ok(None);
        };
        Ok(Some(Self {
            src_bag,
            src_slot,
            dst_bag: INVENTORY_SLOT_BAG_0,
            dst_slot,
        }))
    }

    fn is_supported_inventory_move(&self) -> bool {
        is_supported_move_position(self.src_bag, self.src_slot)
            && is_supported_move_position(self.dst_bag, self.dst_slot)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct DestroyItemRequest {
    bag: u8,
    slot: u8,
    count: u8,
}

impl DestroyItemRequest {
    fn read(body: &[u8]) -> anyhow::Result<Self> {
        if body.len() < 6 {
            anyhow::bail!("CMSG_DESTROYITEM payload too short: {} bytes", body.len());
        }
        Ok(Self {
            bag: normalize_client_bag(body[0]),
            slot: body[1],
            count: body[2],
        })
    }

    fn is_supported_destroy(&self) -> bool {
        is_supported_storage_position(self.bag, self.slot)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct SplitItemRequest {
    src_bag: u8,
    src_slot: u8,
    dst_bag: u8,
    dst_slot: u8,
    count: u8,
}

impl SplitItemRequest {
    fn read(body: &[u8]) -> anyhow::Result<Self> {
        if body.len() < 5 {
            anyhow::bail!("CMSG_SPLIT_ITEM payload too short: {} bytes", body.len());
        }
        Ok(Self {
            src_bag: normalize_client_bag(body[0]),
            src_slot: body[1],
            dst_bag: normalize_client_bag(body[2]),
            dst_slot: body[3],
            count: body[4],
        })
    }

    fn is_supported_split(&self) -> bool {
        self.count != 0
            && is_supported_storage_position(self.src_bag, self.src_slot)
            && is_supported_storage_position(self.dst_bag, self.dst_slot)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct BuyItemRequest {
    vendor_guid: ObjectGuid,
    item: u32,
    count: u8,
}

impl BuyItemRequest {
    fn read(body: &[u8]) -> anyhow::Result<Self> {
        if body.len() < 14 {
            anyhow::bail!("CMSG_BUY_ITEM payload too short: {} bytes", body.len());
        }
        Ok(Self {
            vendor_guid: ObjectGuid::from_raw(u64::from_le_bytes(body[0..8].try_into()?)),
            item: u32::from_le_bytes(body[8..12].try_into()?),
            count: body[12],
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct SellItemRequest {
    vendor_guid: ObjectGuid,
    item_guid: ObjectGuid,
    count: u8,
}

impl SellItemRequest {
    fn read(body: &[u8]) -> anyhow::Result<Self> {
        if body.len() < 17 {
            anyhow::bail!("CMSG_SELL_ITEM payload too short: {} bytes", body.len());
        }
        Ok(Self {
            vendor_guid: ObjectGuid::from_raw(u64::from_le_bytes(body[0..8].try_into()?)),
            item_guid: ObjectGuid::from_raw(u64::from_le_bytes(body[8..16].try_into()?)),
            count: body[16],
        })
    }
}

fn normalize_client_bag(bag: u8) -> u8 {
    if bag == CLIENT_INVENTORY_SLOT_BAG_0 {
        INVENTORY_SLOT_BAG_0
    } else {
        bag
    }
}

fn is_backpack_item_slot(slot: u8) -> bool {
    (INVENTORY_SLOT_ITEM_START..INVENTORY_SLOT_ITEM_END).contains(&slot)
}

fn is_bag_slot(slot: u8) -> bool {
    (INVENTORY_SLOT_BAG_START..INVENTORY_SLOT_BAG_END).contains(&slot)
}

fn is_supported_storage_position(bag: u8, slot: u8) -> bool {
    (bag == INVENTORY_SLOT_BAG_0 && slot < INVENTORY_SLOT_ITEM_END)
        || (is_bag_slot(bag) && slot < MAX_BAG_SIZE)
}

fn is_supported_move_position(bag: u8, slot: u8) -> bool {
    (bag == INVENTORY_SLOT_BAG_0
        && (slot < EQUIPMENT_SLOT_END || is_bag_slot(slot) || is_backpack_item_slot(slot)))
        || (is_bag_slot(bag) && slot < MAX_BAG_SIZE)
}

fn bag0_changed_slots(request: &InventoryMoveRequest) -> Vec<u8> {
    let mut slots = Vec::with_capacity(2);
    if request.src_bag == INVENTORY_SLOT_BAG_0 {
        slots.push(request.src_slot);
    }
    if request.dst_bag == INVENTORY_SLOT_BAG_0 && request.dst_slot != request.src_slot {
        slots.push(request.dst_slot);
    }
    slots
}

fn build_inventory_move_update_blocks(
    character_guid: u32,
    inventory: &[CharacterInventoryItem],
    request: &InventoryMoveRequest,
) -> anyhow::Result<Vec<Vec<u8>>> {
    let mut blocks = Vec::new();
    let bag0_slots = bag0_changed_slots(request);
    if !bag0_slots.is_empty() {
        blocks.push(build_inventory_slots_update_block(
            character_guid,
            inventory,
            &bag0_slots,
        )?);
    }
    blocks.extend(build_container_position_update_blocks(
        character_guid,
        inventory,
        request.src_bag,
        request.src_slot,
    )?);
    if request.dst_bag != request.src_bag || request.dst_slot != request.src_slot {
        blocks.extend(build_container_position_update_blocks(
            character_guid,
            inventory,
            request.dst_bag,
            request.dst_slot,
        )?);
    }

    Ok(blocks)
}

fn build_inventory_position_update_blocks(
    character_guid: u32,
    inventory: &[CharacterInventoryItem],
    bag: u8,
    slot: u8,
) -> anyhow::Result<Vec<Vec<u8>>> {
    if bag == INVENTORY_SLOT_BAG_0 {
        return Ok(vec![build_inventory_slots_update_block(
            character_guid,
            inventory,
            &[slot],
        )?]);
    }
    build_container_position_update_blocks(character_guid, inventory, bag, slot)
}

fn build_container_position_update_blocks(
    character_guid: u32,
    inventory: &[CharacterInventoryItem],
    bag: u8,
    slot: u8,
) -> anyhow::Result<Vec<Vec<u8>>> {
    if !is_bag_slot(bag) {
        return Ok(Vec::new());
    }
    let mut blocks = Vec::new();
    if let Some(block) = build_container_slot_update_block(inventory, bag, slot)? {
        blocks.push(block);
    }
    if let Some(item) = inventory
        .iter()
        .find(|item| item.bag == bag as u32 && item.slot == slot)
    {
        let owner_guid = ObjectGuid::new(HighGuid::Player, 0, character_guid);
        blocks.push(build_item_contained_update_block(owner_guid, inventory, item)?);
    }
    Ok(blocks)
}

fn first_empty_backpack_slot(inventory: &[CharacterInventoryItem]) -> Option<u8> {
    (INVENTORY_SLOT_ITEM_START..INVENTORY_SLOT_ITEM_END).find(|slot| {
        inventory
            .iter()
            .all(|item| item.bag != INVENTORY_SLOT_BAG_0 as u32 || item.slot != *slot)
    })
}

fn build_rust_guide_vendor_inventory() -> Vec<u8> {
    build_vendor_inventory_body(
        rust_guide_guid(),
        &[
            VendorListItem {
                item: RUST_VENDOR_BAG_ITEM,
                display: RUST_VENDOR_BAG_DISPLAY,
                max_count: 0,
                price: 0,
                durability: 0,
                buy_count: 1,
            },
            VendorListItem {
                item: RUST_COMBAT_DUMMY_LOOT_ITEM,
                display: RUST_COMBAT_DUMMY_LOOT_ITEM_DISPLAY,
                max_count: 0,
                price: 0,
                durability: 0,
                buy_count: 1,
            },
        ],
    )
}

#[derive(Debug, Clone, Copy)]
struct VendorListItem {
    item: u32,
    display: u32,
    max_count: u32,
    price: u32,
    durability: u32,
    buy_count: u32,
}

impl From<&wow_db::VendorItemQuery> for VendorListItem {
    fn from(item: &wow_db::VendorItemQuery) -> Self {
        Self {
            item: item.item,
            display: item.display_id,
            max_count: item.max_count,
            price: item.buy_price,
            durability: item.max_durability,
            buy_count: item.buy_count,
        }
    }
}

fn build_vendor_inventory_body(vendor_guid: ObjectGuid, items: &[VendorListItem]) -> Vec<u8> {
    if items.is_empty() {
        let mut body = Vec::with_capacity(10);
        body.extend_from_slice(&vendor_guid.raw().to_le_bytes());
        body.push(0);
        body.push(0);
        return body;
    }

    let mut body = Vec::with_capacity(8 + 1 + items.len().min(128) * 28);
    body.extend_from_slice(&vendor_guid.raw().to_le_bytes());
    body.push(items.len().min(128) as u8);
    for (index, item) in items.iter().take(128).enumerate() {
        let available_count = if item.max_count == 0 {
            u32::MAX
        } else {
            item.max_count
        };
        write_vendor_item(
            &mut body,
            (index + 1) as u32,
            available_count,
            *item,
        );
    }
    body
}

fn write_vendor_item(body: &mut Vec<u8>, slot: u32, available_count: u32, item: VendorListItem) {
    body.extend_from_slice(&slot.to_le_bytes());
    body.extend_from_slice(&item.item.to_le_bytes());
    body.extend_from_slice(&item.display.to_le_bytes());
    body.extend_from_slice(&available_count.to_le_bytes());
    body.extend_from_slice(&item.price.to_le_bytes());
    body.extend_from_slice(&item.durability.to_le_bytes());
    body.extend_from_slice(&item.buy_count.to_le_bytes());
}

fn build_buy_item_body(vendor_guid: ObjectGuid, vendor_slot: u32, count: u8) -> Vec<u8> {
    let mut body = Vec::with_capacity(20);
    body.extend_from_slice(&vendor_guid.raw().to_le_bytes());
    body.extend_from_slice(&vendor_slot.to_le_bytes());
    body.extend_from_slice(&u32::MAX.to_le_bytes());
    body.extend_from_slice(&(count as u32).to_le_bytes());
    body
}

fn build_buy_failed_body(vendor_guid: ObjectGuid, item: u32, result: u8) -> Vec<u8> {
    let mut body = Vec::with_capacity(13);
    body.extend_from_slice(&vendor_guid.raw().to_le_bytes());
    body.extend_from_slice(&item.to_le_bytes());
    body.push(result);
    body
}

fn build_sell_item_error_body(vendor_guid: ObjectGuid, item_guid: ObjectGuid, result: u8) -> Vec<u8> {
    let mut body = Vec::with_capacity(17);
    body.extend_from_slice(&vendor_guid.raw().to_le_bytes());
    body.extend_from_slice(&item_guid.raw().to_le_bytes());
    body.push(result);
    body
}

fn rust_guide_vendor_slot(item: u32) -> Option<u32> {
    match item {
        RUST_VENDOR_BAG_ITEM => Some(1),
        RUST_COMBAT_DUMMY_LOOT_ITEM => Some(2),
        _ => None,
    }
}

fn preferred_equipment_slot(inventory_type: u32) -> Option<u8> {
    match inventory_type {
        4 => Some(3),   // INVTYPE_BODY
        7 => Some(6),   // INVTYPE_LEGS
        8 => Some(7),   // INVTYPE_FEET
        13 | 17 | 21 => Some(15), // one-hand/two-hand/main-hand weapon
        14 => Some(16), // shield
        _ => None,
    }
}

fn item_fits_equipment_slot(inventory_type: u32, slot: u8) -> bool {
    match slot {
        3 => inventory_type == 4,
        6 => inventory_type == 7,
        7 => inventory_type == 8,
        15 => matches!(inventory_type, 13 | 17 | 21),
        16 => inventory_type == 14,
        _ => false,
    }
}

fn inventory_opcode_name(opcode: u32) -> &'static str {
    match opcode {
        CMSG_AUTOEQUIP_ITEM => "CMSG_AUTOEQUIP_ITEM",
        CMSG_SWAP_INV_ITEM => "CMSG_SWAP_INV_ITEM",
        CMSG_SWAP_ITEM => "CMSG_SWAP_ITEM",
        CMSG_SPLIT_ITEM => "CMSG_SPLIT_ITEM",
        CMSG_DESTROYITEM => "CMSG_DESTROYITEM",
        _ => "UNKNOWN_INVENTORY_OPCODE",
    }
}

async fn send_inventory_change_failure(
    stream: &mut TcpStream,
    result: u8,
    item: Option<ObjectGuid>,
    item2: Option<ObjectGuid>,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    let mut body = Vec::with_capacity(18);
    body.push(result);
    body.extend_from_slice(&item.map(|guid| guid.raw()).unwrap_or(0).to_le_bytes());
    body.extend_from_slice(&item2.map(|guid| guid.raw()).unwrap_or(0).to_le_bytes());
    body.push(0);
    send_packet(
        stream,
        SMSG_INVENTORY_CHANGE_FAILURE,
        &body,
        Some(header_crypto),
    )
    .await
}

async fn handle_creature_query(
    stream: &mut TcpStream,
    world_db_pool: &MySqlPool,
    body: &[u8],
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    let query = CreatureQuery::read(body)?;
    let db_template = wow_db::get_creature_template_query(world_db_pool, query.entry).await?;
    info!(
        entry = query.entry,
        guid = format_args!("0x{:016X}", query.guid.raw()),
        found = db_template.is_some()
            || matches!(query.entry, RUST_GUIDE_ENTRY | RUST_COMBAT_DUMMY_ENTRY),
        "Answering creature template query"
    );
    let response = build_creature_query_response(query.entry, db_template.as_ref());
    send_packet(
        stream,
        SMSG_CREATURE_QUERY_RESPONSE,
        &response,
        Some(header_crypto),
    )
    .await
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct CreatureQuery {
    entry: u32,
    guid: ObjectGuid,
}

impl CreatureQuery {
    fn read(body: &[u8]) -> anyhow::Result<Self> {
        let mut cursor = 0;
        let entry = read_u32(body, &mut cursor)?;
        ensure_available(body, cursor + 8)?;
        let guid = ObjectGuid::from_raw(u64::from_le_bytes(body[cursor..cursor + 8].try_into()?));
        Ok(Self { entry, guid })
    }
}

fn build_creature_query_response(entry: u32, db_template: Option<&CreatureTemplateQuery>) -> Vec<u8> {
    let Some(template) = creature_query_template(entry, db_template) else {
        return (entry | 0x8000_0000).to_le_bytes().to_vec();
    };

    let mut body = Vec::with_capacity(100);
    body.extend_from_slice(&entry.to_le_bytes());
    write_c_string(&mut body, template.name);
    body.push(0);
    body.push(0);
    body.push(0);
    write_c_string(&mut body, template.subname);
    body.extend_from_slice(&0u32.to_le_bytes()); // type flags
    body.extend_from_slice(&template.creature_type.to_le_bytes());
    body.extend_from_slice(&(template.family as u32).to_le_bytes());
    body.extend_from_slice(&template.rank.to_le_bytes());
    body.extend_from_slice(&0u32.to_le_bytes()); // unknown
    body.extend_from_slice(&template.pet_spell_data_id.to_le_bytes());
    body.extend_from_slice(&template.display_id.to_le_bytes());
    body.extend_from_slice(&(template.civilian as u16).to_le_bytes());
    body
}

struct FixtureCreatureTemplate {
    name: &'static str,
    subname: &'static str,
    display_id: u32,
}

struct CreatureQueryTemplate<'a> {
    name: &'a str,
    subname: &'a str,
    creature_type: u32,
    family: i32,
    rank: u32,
    pet_spell_data_id: u32,
    display_id: u32,
    civilian: u8,
}

fn creature_query_template<'a>(
    entry: u32,
    db_template: Option<&'a CreatureTemplateQuery>,
) -> Option<CreatureQueryTemplate<'a>> {
    if let Some(template) = db_template {
        return Some(CreatureQueryTemplate {
            name: &template.name,
            subname: template.subname.as_deref().unwrap_or(""),
            creature_type: template.creature_type,
            family: template.family,
            rank: template.rank,
            pet_spell_data_id: template.pet_spell_data_id,
            display_id: creature_display_id(template),
            civilian: template.civilian,
        });
    }

    let template = fixture_creature_template(entry)?;
    Some(CreatureQueryTemplate {
        name: template.name,
        subname: template.subname,
        creature_type: 7,
        family: 0,
        rank: 0,
        pet_spell_data_id: 0,
        display_id: template.display_id,
        civilian: 0,
    })
}

fn fixture_creature_template(entry: u32) -> Option<FixtureCreatureTemplate> {
    match entry {
        RUST_GUIDE_ENTRY => Some(FixtureCreatureTemplate {
            name: RUST_GUIDE_NAME,
            subname: RUST_GUIDE_SUBNAME,
            display_id: RUST_GUIDE_DISPLAY_ID,
        }),
        RUST_COMBAT_DUMMY_ENTRY => Some(FixtureCreatureTemplate {
            name: RUST_COMBAT_DUMMY_NAME,
            subname: RUST_COMBAT_DUMMY_SUBNAME,
            display_id: RUST_COMBAT_DUMMY_DISPLAY_ID,
        }),
        _ => None,
    }
}

async fn handle_gossip_hello(
    stream: &mut TcpStream,
    world_db_pool: &MySqlPool,
    body: &[u8],
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    let guid = read_packet_guid(body, "CMSG_GOSSIP_HELLO")?;
    if guid == rust_guide_guid() {
        let text_update =
            build_npc_text_update(RUST_GUIDE_GOSSIP_TEXT_ID, RUST_GUIDE_GOSSIP_TEXT);
        send_packet(
            stream,
            SMSG_NPC_TEXT_UPDATE,
            &text_update,
            Some(&mut *header_crypto),
        )
        .await?;
        let response = build_gossip_message(
            guid,
            RUST_GUIDE_GOSSIP_TEXT_ID,
            &[(0, RUST_GUIDE_GOSSIP_OPTION)],
        );
        return send_packet(stream, SMSG_GOSSIP_MESSAGE, &response, Some(header_crypto)).await;
    }

    if guid.is_creature() {
        let vendor_items = wow_db::get_vendor_items(world_db_pool, guid.entry()).await?;
        if !vendor_items.is_empty() {
            let text_update =
                build_npc_text_update(DB_VENDOR_GOSSIP_TEXT_ID, DB_VENDOR_GOSSIP_TEXT);
            send_packet(
                stream,
                SMSG_NPC_TEXT_UPDATE,
                &text_update,
                Some(&mut *header_crypto),
            )
            .await?;
            let response = build_gossip_message(
                guid,
                DB_VENDOR_GOSSIP_TEXT_ID,
                &[(0, DB_VENDOR_GOSSIP_OPTION)],
            );
            return send_packet(stream, SMSG_GOSSIP_MESSAGE, &response, Some(header_crypto)).await;
        }
    }

    warn!(
        guid = format_args!("0x{:016X}", guid.raw()),
        "Ignoring gossip hello for unknown creature"
    );
    Ok(())
}

async fn handle_gossip_select_option(
    stream: &mut TcpStream,
    world_db_pool: &MySqlPool,
    body: &[u8],
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    let selection = GossipSelectOption::read(body)?;
    if selection.guid == rust_guide_guid() {
        if selection.is_supported_browse_option() {
            return send_packet(stream, SMSG_GOSSIP_COMPLETE, &[], Some(header_crypto)).await;
        }
        warn!(
            option = selection.option,
            "Ignoring unsupported Rust Guide gossip option"
        );
        return Ok(());
    }

    if selection.guid.is_creature() {
        if !selection.is_supported_browse_option() {
            warn!(
                guid = format_args!("0x{:016X}", selection.guid.raw()),
                option = selection.option,
                "Ignoring unsupported DB vendor gossip option"
            );
            return Ok(());
        }
        let vendor_items = wow_db::get_vendor_items(world_db_pool, selection.guid.entry()).await?;
        if !vendor_items.is_empty() {
            let list_items: Vec<VendorListItem> = vendor_items.iter().map(Into::into).collect();
            let response = build_vendor_inventory_body(selection.guid, &list_items);
            return send_packet(stream, SMSG_LIST_INVENTORY, &response, Some(header_crypto)).await;
        }
    }

    warn!(
        guid = format_args!("0x{:016X}", selection.guid.raw()),
        option = selection.option,
        "Ignoring gossip select for unknown creature"
    );
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct GossipSelectOption {
    guid: ObjectGuid,
    option: u32,
}

impl GossipSelectOption {
    fn read(body: &[u8]) -> anyhow::Result<Self> {
        if body.len() < 12 {
            anyhow::bail!(
                "CMSG_GOSSIP_SELECT_OPTION payload too short: {} bytes",
                body.len()
            );
        }
        Ok(Self {
            guid: ObjectGuid::from_raw(u64::from_le_bytes(body[0..8].try_into()?)),
            option: u32::from_le_bytes(body[8..12].try_into()?),
        })
    }

    fn is_supported_browse_option(&self) -> bool {
        self.option == 0
    }
}

async fn handle_npc_text_query(
    stream: &mut TcpStream,
    body: &[u8],
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    let mut cursor = 0;
    let text_id = read_u32(body, &mut cursor)?;
    ensure_available(body, cursor + 8)?;
    let guid = ObjectGuid::from_raw(u64::from_le_bytes(body[cursor..cursor + 8].try_into()?));
    info!(
        text_id,
        guid = format_args!("0x{:016X}", guid.raw()),
        "Answering NPC text query"
    );
    let text = if text_id == DB_VENDOR_GOSSIP_TEXT_ID {
        DB_VENDOR_GOSSIP_TEXT
    } else {
        RUST_GUIDE_GOSSIP_TEXT
    };
    let response = build_npc_text_update(text_id, text);
    send_packet(stream, SMSG_NPC_TEXT_UPDATE, &response, Some(header_crypto)).await
}

async fn handle_list_inventory(
    stream: &mut TcpStream,
    world_db_pool: &MySqlPool,
    body: &[u8],
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    let guid = read_packet_guid(body, "CMSG_LIST_INVENTORY")?;
    let response = if guid == rust_guide_guid() {
        build_rust_guide_vendor_inventory()
    } else if guid.is_creature() {
        let vendor_items = wow_db::get_vendor_items(world_db_pool, guid.entry()).await?;
        let list_items: Vec<VendorListItem> = vendor_items.iter().map(Into::into).collect();
        info!(
            entry = guid.entry(),
            guid = format_args!("0x{:016X}", guid.raw()),
            count = list_items.len(),
            "Answering DB-backed vendor inventory request"
        );
        build_vendor_inventory_body(guid, &list_items)
    } else {
        warn!(
            guid = format_args!("0x{:016X}", guid.raw()),
            "Ignoring vendor inventory request for unknown creature"
        );
        return Ok(());
    };
    send_packet(stream, SMSG_LIST_INVENTORY, &response, Some(header_crypto)).await
}

async fn handle_buy_item(
    stream: &mut TcpStream,
    character_db_pool: &MySqlPool,
    world_db_pool: &MySqlPool,
    body: &[u8],
    session: &mut WorldSessionState,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    let Some(character) = &session.active_character else {
        warn!("Ignoring vendor buy before character login");
        return Ok(());
    };
    let character_guid = character.guid;
    let buy = BuyItemRequest::read(body)?;
    let vendor_item = vendor_buy_item(world_db_pool, buy).await?;
    let Some(vendor_item) = vendor_item else {
        warn!(
            item = buy.item,
            vendor = format_args!("0x{:016X}", buy.vendor_guid.raw()),
            "Ignoring unsupported vendor buy request"
        );
        return Ok(());
    };
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

    let count = buy.count.max(1);
    let total_count = vendor_item.buy_count.max(1).saturating_mul(count as u32);
    let price = vendor_item.price.saturating_mul(count as u32);
    let money = if price == 0 {
        None
    } else {
        match wow_db::spend_character_money(character_db_pool, character_guid, price).await? {
            Some(money) => Some(money),
            None => {
                return send_packet(
                    stream,
                    SMSG_BUY_FAILED,
                    &build_buy_failed_body(buy.vendor_guid, buy.item, BUY_ERR_NOT_ENOUGHT_MONEY),
                    Some(header_crypto),
                )
                .await;
            }
        }
    };
    wow_db::add_character_inventory_item(
        character_db_pool,
        character_guid,
        INVENTORY_SLOT_BAG_0 as u32,
        dst_slot,
        buy.item,
        total_count,
        0,
    )
    .await?;
    session.inventory = wow_db::get_character_inventory_items(character_db_pool, character_guid).await?;
    let Some(new_item) = session
        .inventory
        .iter()
        .find(|item| item.bag == INVENTORY_SLOT_BAG_0 as u32 && item.slot == dst_slot)
    else {
        return Ok(());
    };

    send_packet(
        stream,
        SMSG_BUY_ITEM,
        &build_buy_item_body(buy.vendor_guid, vendor_item.slot, count),
        Some(&mut *header_crypto),
    )
    .await?;
    let owner_guid = ObjectGuid::new(HighGuid::Player, 0, character_guid);
    let container_slots = if vendor_item.container_slots > 0 {
        Some(vendor_item.container_slots)
    } else {
        None
    };
    let create_block =
        build_item_create_update_block(owner_guid, owner_guid, new_item, container_slots)?;
    let slot_block = build_inventory_slots_update_block(character_guid, &session.inventory, &[dst_slot])?;
    let body = build_update_object_body(&[create_block, slot_block]);
    send_packet(stream, SMSG_UPDATE_OBJECT, &body, Some(&mut *header_crypto)).await?;
    if let Some(money) = money {
        send_packet(
            stream,
            SMSG_UPDATE_OBJECT,
            &build_player_money_update_body(character_guid, money)?,
            Some(header_crypto),
        )
        .await?;
    }
    Ok(())
}

#[derive(Debug, Clone, Copy)]
struct VendorBuyItem {
    slot: u32,
    container_slots: u32,
    buy_count: u32,
    price: u32,
}

async fn vendor_buy_item(
    world_db_pool: &MySqlPool,
    buy: BuyItemRequest,
) -> anyhow::Result<Option<VendorBuyItem>> {
    if buy.vendor_guid == rust_guide_guid() {
        return Ok(rust_guide_vendor_slot(buy.item).map(|slot| VendorBuyItem {
            slot,
            container_slots: if buy.item == RUST_VENDOR_BAG_ITEM { 6 } else { 0 },
            buy_count: 1,
            price: 0,
        }));
    }

    if !buy.vendor_guid.is_creature() {
        return Ok(None);
    }

    let vendor_items = wow_db::get_vendor_items(world_db_pool, buy.vendor_guid.entry()).await?;
    Ok(vendor_items
        .iter()
        .enumerate()
        .find(|(_, item)| item.item == buy.item)
        .map(|(index, item)| VendorBuyItem {
            slot: (index + 1) as u32,
            container_slots: item.container_slots,
            buy_count: item.buy_count,
            price: item.buy_price,
        }))
}

async fn handle_sell_item(
    stream: &mut TcpStream,
    character_db_pool: &MySqlPool,
    world_db_pool: &MySqlPool,
    body: &[u8],
    session: &mut WorldSessionState,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    let Some(character) = &session.active_character else {
        warn!("Ignoring vendor sell before character login");
        return Ok(());
    };
    let character_guid = character.guid;
    let request = SellItemRequest::read(body)?;
    let vendor_valid = if request.vendor_guid == rust_guide_guid() {
        true
    } else if request.vendor_guid.is_creature() {
        !wow_db::get_vendor_items(world_db_pool, request.vendor_guid.entry())
            .await?
            .is_empty()
    } else {
        false
    };
    if !vendor_valid {
        return send_packet(
            stream,
            SMSG_SELL_ITEM,
            &build_sell_item_error_body(
                request.vendor_guid,
                request.item_guid,
                SELL_ERR_CANT_FIND_VENDOR,
            ),
            Some(header_crypto),
        )
        .await;
    }

    let Some(source_item) = session
        .inventory
        .iter()
        .find(|item| item.item == request.item_guid.counter())
        .cloned()
    else {
        return Ok(());
    };
    let Some(template) = wow_db::get_item_template_query(world_db_pool, source_item.item_template).await?
    else {
        return send_packet(
            stream,
            SMSG_SELL_ITEM,
            &build_sell_item_error_body(
                request.vendor_guid,
                request.item_guid,
                SELL_ERR_CANT_SELL_ITEM,
            ),
            Some(header_crypto),
        )
        .await;
    };
    let count = if request.count == 0 {
        source_item.count
    } else {
        request.count as u32
    };
    if count == 0
        || count > source_item.count
        || template.sell_price == 0
        || (template.container_slots > 0
            && session
                .inventory
                .iter()
                .any(|item| item.bag == source_item.slot as u32))
    {
        return send_packet(
            stream,
            SMSG_SELL_ITEM,
            &build_sell_item_error_body(
                request.vendor_guid,
                request.item_guid,
                SELL_ERR_CANT_SELL_ITEM,
            ),
            Some(header_crypto),
        )
        .await;
    }

    let sold = wow_db::destroy_character_inventory_item_count(
        character_db_pool,
        character_guid,
        source_item.bag,
        source_item.slot,
        count,
    )
    .await?;
    let Some(sold) = sold else {
        return Ok(());
    };
    let money = wow_db::add_character_money(
        character_db_pool,
        character_guid,
        template.sell_price.saturating_mul(count),
    )
    .await?;
    session.inventory = wow_db::get_character_inventory_items(character_db_pool, character_guid).await?;

    match sold {
        wow_db::InventoryDestroyResult::CountChanged { item, count } => {
            let body = build_item_stack_count_update_body(item, count)?;
            send_packet(stream, SMSG_UPDATE_OBJECT, &body, Some(&mut *header_crypto)).await?;
        }
        wow_db::InventoryDestroyResult::Removed { item } => {
            let body = if source_item.bag == INVENTORY_SLOT_BAG_0 as u32 {
                build_inventory_slots_update_body(
                    character_guid,
                    &session.inventory,
                    &[source_item.slot],
                )?
            } else {
                build_destroy_object_body(item)
            };
            let opcode = if source_item.bag == INVENTORY_SLOT_BAG_0 as u32 {
                SMSG_UPDATE_OBJECT
            } else {
                SMSG_DESTROY_OBJECT
            };
            send_packet(stream, opcode, &body, Some(&mut *header_crypto)).await?;
        }
    }
    send_packet(
        stream,
        SMSG_UPDATE_OBJECT,
        &build_player_money_update_body(character_guid, money)?,
        Some(header_crypto),
    )
    .await
}

async fn handle_attack_swing(
    stream: &mut TcpStream,
    body: &[u8],
    session: &mut WorldSessionState,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    let target = read_packet_guid(body, "CMSG_ATTACKSWING")?;
    let Some(character) = &session.active_character else {
        warn!("Ignoring attack swing before character login");
        return Ok(());
    };

    if target == rust_combat_dummy_guid() {
        if session.combat_dummy_lootable || session.combat_dummy_health == 0 {
            warn!("Ignoring attack swing against dead combat dummy");
            return Ok(());
        }

        session.active_combat_target = Some(target);
        let attacker = ObjectGuid::new(HighGuid::Player, 0, character.guid);
        send_packet(
            stream,
            SMSG_ATTACKSTART,
            &build_attack_start_body(attacker, target),
            Some(&mut *header_crypto),
        )
        .await?;
        return send_combat_dummy_swing(stream, session, header_crypto).await;
    }

    if !session
        .db_creatures
        .get(&target.raw())
        .is_some_and(DbCreatureRuntime::is_alive)
    {
        warn!(
            target = format_args!("0x{:016X}", target.raw()),
            "Ignoring attack swing against unknown target"
        );
        return Ok(());
    }

    session.active_combat_target = Some(target);
    let attacker = ObjectGuid::new(HighGuid::Player, 0, character.guid);
    send_packet(
        stream,
        SMSG_ATTACKSTART,
        &build_attack_start_body(attacker, target),
        Some(&mut *header_crypto),
    )
    .await?;
    send_db_creature_swing(stream, session, header_crypto, target).await
}

impl DbCreatureRuntime {
    fn new(spawn: CreatureSpawnQuery) -> Self {
        let health = creature_health(&spawn.template);
        Self {
            spawn,
            health,
            lootable: false,
            looting: false,
            loot_money_available: false,
            loot_item: None,
        }
    }

    fn guid(&self) -> ObjectGuid {
        creature_spawn_guid(&self.spawn)
    }

    fn is_alive(&self) -> bool {
        self.health > 0 && !self.lootable
    }

    fn max_health(&self) -> u32 {
        creature_health(&self.spawn.template)
    }

    fn hit_damage(&self) -> u32 {
        self.spawn.template.max_melee_dmg.ceil().max(1.0) as u32
    }

    fn loot_money(&self) -> u32 {
        self.spawn
            .template
            .max_loot_gold
            .max(self.spawn.template.min_loot_gold)
    }

    fn dynamic_flags(&self) -> u32 {
        if self.lootable {
            UNIT_DYNFLAG_LOOTABLE
        } else {
            self.spawn.template.dynamic_flags
        }
    }

    fn respawn(&mut self) {
        self.health = self.max_health();
        self.lootable = false;
        self.looting = false;
        self.loot_money_available = false;
        self.loot_item = None;
    }
}

fn apply_db_creature_damage(
    session: &mut WorldSessionState,
    target: ObjectGuid,
    requested_damage: u32,
) -> Option<u32> {
    let creature = session.db_creatures.get_mut(&target.raw())?;
    if !creature.is_alive() {
        return None;
    }

    let damage = creature.health.min(requested_damage.max(1));
    creature.health = creature.health.saturating_sub(damage);
    if creature.health == 0 {
        creature.lootable = true;
        creature.looting = false;
        creature.loot_money_available = creature.loot_money() > 0;
        creature.loot_item = None;
        session.active_combat_target = None;
    }
    Some(damage)
}

async fn handle_combat_tick(
    stream: &mut TcpStream,
    session: &mut WorldSessionState,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    let Some(target) = session.active_combat_target else {
        return Ok(());
    };
    if target == rust_combat_dummy_guid() {
        return send_combat_dummy_swing(stream, session, header_crypto).await;
    }
    send_db_creature_swing(stream, session, header_crypto, target).await
}

async fn send_db_creature_swing(
    stream: &mut TcpStream,
    session: &mut WorldSessionState,
    header_crypto: &mut HeaderCrypto,
    target: ObjectGuid,
) -> anyhow::Result<()> {
    let Some(character) = &session.active_character else {
        return Ok(());
    };
    let attacker = ObjectGuid::new(HighGuid::Player, 0, character.guid);
    let requested_damage = session
        .db_creatures
        .get(&target.raw())
        .map(DbCreatureRuntime::hit_damage)
        .unwrap_or(1);
    let Some(damage) = apply_db_creature_damage(session, target, requested_damage) else {
        session.active_combat_target = None;
        return Ok(());
    };
    session.player_rage =
        (session.player_rage + RUST_COMBAT_DUMMY_RAGE_GAIN).min(POWER_RAGE_DEFAULT);
    let (health, dynamic_flags, is_dead) = session
        .db_creatures
        .get(&target.raw())
        .map(|creature| {
            (
                creature.health,
                creature.dynamic_flags(),
                creature.health == 0,
            )
        })
        .expect("DB creature existed before damage");

    send_packet(
        stream,
        SMSG_ATTACKERSTATEUPDATE,
        &build_attacker_state_update_body(attacker, target, damage)?,
        Some(&mut *header_crypto),
    )
    .await?;
    send_packet(
        stream,
        SMSG_UPDATE_OBJECT,
        &build_db_creature_state_update_body(target, health, dynamic_flags)?,
        Some(&mut *header_crypto),
    )
    .await?;
    send_packet(
        stream,
        SMSG_UPDATE_OBJECT,
        &build_player_rage_update_body(attacker, session.player_rage)?,
        Some(&mut *header_crypto),
    )
    .await?;

    if is_dead {
        send_packet(
            stream,
            SMSG_ATTACKSTOP,
            &build_attack_stop_body(attacker, target, true)?,
            Some(header_crypto),
        )
        .await?;
    }

    Ok(())
}

async fn send_combat_dummy_swing(
    stream: &mut TcpStream,
    session: &mut WorldSessionState,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    let Some(character) = &session.active_character else {
        return Ok(());
    };
    let attacker = ObjectGuid::new(HighGuid::Player, 0, character.guid);
    let target = rust_combat_dummy_guid();

    let damage = session
        .combat_dummy_health
        .min(RUST_COMBAT_DUMMY_HIT_DAMAGE);
    session.combat_dummy_health = session.combat_dummy_health.saturating_sub(damage);
    session.player_rage =
        (session.player_rage + RUST_COMBAT_DUMMY_RAGE_GAIN).min(POWER_RAGE_DEFAULT);

    send_packet(
        stream,
        SMSG_ATTACKERSTATEUPDATE,
        &build_attacker_state_update_body(attacker, target, damage)?,
        Some(&mut *header_crypto),
    )
    .await?;
    send_packet(
        stream,
        SMSG_UPDATE_OBJECT,
        &build_combat_dummy_state_update_body(session.combat_dummy_health, 0)?,
        Some(&mut *header_crypto),
    )
    .await?;
    send_packet(
        stream,
        SMSG_UPDATE_OBJECT,
        &build_player_rage_update_body(attacker, session.player_rage)?,
        Some(&mut *header_crypto),
    )
    .await?;

    if session.combat_dummy_health == 0 {
        session.combat_dummy_lootable = true;
        session.combat_dummy_looting = false;
        session.combat_dummy_loot_money_available = true;
        session.combat_dummy_loot_item_available = true;
        send_packet(
            stream,
            SMSG_ATTACKSTOP,
            &build_attack_stop_body(attacker, target, true)?,
            Some(&mut *header_crypto),
        )
        .await?;
        send_packet(
            stream,
            SMSG_UPDATE_OBJECT,
            &build_combat_dummy_state_update_body(0, UNIT_DYNFLAG_LOOTABLE)?,
            Some(header_crypto),
        )
        .await?;
        session.active_combat_target = None;
    }

    Ok(())
}

async fn handle_attack_stop(
    stream: &mut TcpStream,
    session: &mut WorldSessionState,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    let Some(character) = &session.active_character else {
        return Ok(());
    };
    let attacker = ObjectGuid::new(HighGuid::Player, 0, character.guid);
    let victim = session.active_combat_target.unwrap_or_else(rust_combat_dummy_guid);
    session.active_combat_target = None;
    send_packet(
        stream,
        SMSG_ATTACKSTOP,
        &build_attack_stop_body(attacker, victim, false)?,
        Some(header_crypto),
    )
    .await
}

async fn handle_loot(
    stream: &mut TcpStream,
    world_db_pool: &MySqlPool,
    body: &[u8],
    session: &mut WorldSessionState,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    let target = read_packet_guid(body, "CMSG_LOOT")?;
    if target == rust_combat_dummy_guid() {
        if !session.combat_dummy_lootable {
            warn!("Ignoring loot request for combat dummy before it is lootable");
            return Ok(());
        }

        session.combat_dummy_looting = true;
        let response = build_combat_dummy_loot_response_body(session);
        return send_packet(stream, SMSG_LOOT_RESPONSE, &response, Some(header_crypto)).await;
    }

    let Some(creature) = session.db_creatures.get(&target.raw()) else {
        warn!(
            target = format_args!("0x{:016X}", target.raw()),
            "Ignoring loot request for unknown target"
        );
        return Ok(());
    };
    if !creature.lootable {
        warn!("Ignoring loot request for DB creature before it is lootable");
        return Ok(());
    }
    let needs_loot_item = creature.loot_item.is_none();
    let entry = creature.spawn.entry;
    if needs_loot_item {
        let loot_item = wow_db::get_creature_loot_items(world_db_pool, entry)
            .await?
            .into_iter()
            .next()
            .map(DbCreatureLootRuntime::from);
        if let Some(creature) = session.db_creatures.get_mut(&target.raw()) {
            creature.loot_item = loot_item;
        }
    }
    let creature = session
        .db_creatures
        .get_mut(&target.raw())
        .expect("DB creature existed before loot query");
    creature.looting = true;
    let response = build_db_creature_loot_response_body(target, creature);
    send_packet(stream, SMSG_LOOT_RESPONSE, &response, Some(header_crypto)).await
}

async fn handle_autostore_loot_item(
    stream: &mut TcpStream,
    character_db_pool: &MySqlPool,
    world_db_pool: &MySqlPool,
    body: &[u8],
    session: &mut WorldSessionState,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    let Some(character) = &session.active_character else {
        warn!("Ignoring loot item request before character login");
        return Ok(());
    };
    let character_guid = character.guid;
    if body.is_empty() {
        anyhow::bail!("CMSG_AUTOSTORE_LOOT_ITEM payload too short: {} bytes", body.len());
    }
    let loot_slot = body[0];
    let db_loot = session
        .db_creatures
        .iter()
        .find_map(|(guid, creature)| {
            creature
                .looting
                .then(|| creature.loot_item.as_ref().map(|loot| (*guid, loot.clone())))
                .flatten()
        });
    if let Some((creature_guid, loot)) = db_loot {
        if loot_slot != 0 {
            warn!(loot_slot, "Ignoring unsupported DB creature loot slot");
            return Ok(());
        }
        return autostore_loot_item(
            LootAutostoreContext {
                stream,
                character_db_pool,
                world_db_pool,
                session,
                header_crypto,
                character_guid,
            },
            creature_guid,
            loot,
            loot_slot,
        )
        .await;
    }

    if !session.combat_dummy_looting || loot_slot != 0 || !session.combat_dummy_loot_item_available {
        warn!(
            loot_slot,
            "Ignoring loot item request without available combat dummy loot"
        );
        return Ok(());
    }

    let max_stack = wow_db::get_item_template_query(world_db_pool, RUST_COMBAT_DUMMY_LOOT_ITEM)
        .await?
        .map(|template| template.stackable.max(1))
        .unwrap_or(1);
    let mut remaining_count = RUST_COMBAT_DUMMY_LOOT_ITEM_COUNT;
    let mut update_blocks = Vec::new();

    if max_stack > 1 {
        if let Some(existing_stack) = session
            .inventory
            .iter()
            .filter(|item| {
                item.item_template == RUST_COMBAT_DUMMY_LOOT_ITEM
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

    if remaining_count == 0 {
        session.combat_dummy_loot_item_available = false;
        session.inventory =
            wow_db::get_character_inventory_items(character_db_pool, character_guid).await?;
        send_packet(
            stream,
            SMSG_LOOT_REMOVED,
            &[loot_slot],
            Some(&mut *header_crypto),
        )
        .await?;
        let body = build_update_object_body(&update_blocks);
        return send_packet(stream, SMSG_UPDATE_OBJECT, &body, Some(header_crypto)).await;
    }

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
        RUST_COMBAT_DUMMY_LOOT_ITEM,
        remaining_count,
        0,
    )
    .await?;
    session.combat_dummy_loot_item_available = false;
    session.inventory = wow_db::get_character_inventory_items(character_db_pool, character_guid).await?;
    let Some(new_item) = session.inventory.iter().find(|item| {
        item.bag == INVENTORY_SLOT_BAG_0 as u32
            && item.slot == dst_slot
            && item.item_template == RUST_COMBAT_DUMMY_LOOT_ITEM
    }) else {
        return Ok(());
    };

    send_packet(stream, SMSG_LOOT_REMOVED, &[loot_slot], Some(&mut *header_crypto)).await?;
    let owner_guid = ObjectGuid::new(HighGuid::Player, 0, character.guid);
    let create_block = build_item_create_update_block(owner_guid, owner_guid, new_item, None)?;
    let slot_block = build_inventory_slots_update_block(character_guid, &session.inventory, &[dst_slot])?;
    update_blocks.push(create_block);
    update_blocks.push(slot_block);
    let body = build_update_object_body(&update_blocks);
    send_packet(stream, SMSG_UPDATE_OBJECT, &body, Some(header_crypto)).await
}

async fn handle_loot_money(
    stream: &mut TcpStream,
    character_db_pool: &MySqlPool,
    session: &mut WorldSessionState,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    let Some(character) = &session.active_character else {
        warn!("Ignoring loot money request before character login");
        return Ok(());
    };
    if let Some((creature_guid, money)) = session
        .db_creatures
        .iter()
        .find(|(_, creature)| creature.looting && creature.loot_money_available)
        .map(|(guid, creature)| (*guid, creature.loot_money()))
    {
        let gained_money = money;
        let money =
            wow_db::add_character_money(character_db_pool, character.guid, gained_money).await?;
        if let Some(creature) = session.db_creatures.get_mut(&creature_guid) {
            creature.loot_money_available = false;
        }
        send_packet(
            stream,
            SMSG_LOOT_MONEY_NOTIFY,
            &gained_money.to_le_bytes(),
            Some(&mut *header_crypto),
        )
        .await?;
        send_packet(stream, SMSG_LOOT_CLEAR_MONEY, &[], Some(&mut *header_crypto)).await?;
        return send_packet(
            stream,
            SMSG_UPDATE_OBJECT,
            &build_player_money_update_body(character.guid, money)?,
            Some(header_crypto),
        )
        .await;
    }

    if !session.combat_dummy_looting {
        warn!("Ignoring loot money request without an open combat dummy loot window");
        return Ok(());
    }
    if !session.combat_dummy_loot_money_available {
        return Ok(());
    }

    let money = wow_db::add_character_money(
        character_db_pool,
        character.guid,
        RUST_COMBAT_DUMMY_LOOT_MONEY,
    )
    .await?;
    session.combat_dummy_loot_money_available = false;
    send_packet(
        stream,
        SMSG_LOOT_MONEY_NOTIFY,
        &RUST_COMBAT_DUMMY_LOOT_MONEY.to_le_bytes(),
        Some(&mut *header_crypto),
    )
    .await?;
    send_packet(stream, SMSG_LOOT_CLEAR_MONEY, &[], Some(&mut *header_crypto)).await?;
    send_packet(
        stream,
        SMSG_UPDATE_OBJECT,
        &build_player_money_update_body(character.guid, money)?,
        Some(header_crypto),
    )
    .await
}

async fn handle_loot_release(
    stream: &mut TcpStream,
    body: &[u8],
    session: &mut WorldSessionState,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    let target = read_packet_guid(body, "CMSG_LOOT_RELEASE")?;
    if target == rust_combat_dummy_guid() {
        session.combat_dummy_looting = false;
        session.combat_dummy_lootable = false;
        session.combat_dummy_loot_money_available = false;
        session.combat_dummy_loot_item_available = false;
        session.combat_dummy_health = RUST_COMBAT_DUMMY_HEALTH;
        send_packet(
            stream,
            SMSG_LOOT_RELEASE_RESPONSE,
            &build_loot_release_response_body(target, true),
            Some(&mut *header_crypto),
        )
        .await?;
        return send_packet(
            stream,
            SMSG_UPDATE_OBJECT,
            &build_combat_dummy_state_update_body(RUST_COMBAT_DUMMY_HEALTH, 0)?,
            Some(header_crypto),
        )
        .await;
    }

    let Some(creature) = session.db_creatures.get_mut(&target.raw()) else {
        warn!(
            target = format_args!("0x{:016X}", target.raw()),
            "Ignoring loot release for unknown target"
        );
        return Ok(());
    };
    creature.respawn();
    send_packet(
        stream,
        SMSG_LOOT_RELEASE_RESPONSE,
        &build_loot_release_response_body(target, true),
        Some(&mut *header_crypto),
    )
    .await?;
    send_packet(
        stream,
        SMSG_UPDATE_OBJECT,
        &build_db_creature_state_update_body(target, creature.health, 0)?,
        Some(header_crypto),
    )
    .await
}

fn build_attack_start_body(attacker: ObjectGuid, victim: ObjectGuid) -> Vec<u8> {
    let mut body = Vec::with_capacity(16);
    body.extend_from_slice(&attacker.raw().to_le_bytes());
    body.extend_from_slice(&victim.raw().to_le_bytes());
    body
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

