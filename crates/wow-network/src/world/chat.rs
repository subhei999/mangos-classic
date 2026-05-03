async fn handle_ping(
    stream: &mut WorldPacketSink,
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
    stream: &mut WorldPacketSink,
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
    stream: &mut WorldPacketSink,
    deps: ChatDeps<'_>,
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
    send_packet(stream, SMSG_MESSAGECHAT, &body, Some(header_crypto)).await?;

    let radius = chat_radius_yards(chat.chat_type);
    if radius > 0.0 {
        let packets = deps
            .maps
            .broadcast_nearby_player_packet(
                character.position.map_id,
                character.guid,
                radius,
                OutboundWorldPacket {
                    opcode: SMSG_MESSAGECHAT,
                    body,
                },
            )
            .await;
        deps.sessions.dispatch(packets).await;
    }
    Ok(())
}

struct ChatDeps<'a> {
    maps: &'a Arc<MapRuntimeManager>,
    sessions: &'a Arc<SessionRegistry>,
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

fn build_system_message_chat_body(message: &str) -> Vec<u8> {
    let mut body = Vec::with_capacity(1 + 4 + 8 + 4 + message.len() + 2);
    body.push(CHAT_MSG_SYSTEM as u8);
    body.extend_from_slice(&LANG_UNIVERSAL.to_le_bytes());
    body.extend_from_slice(&0u64.to_le_bytes());
    body.extend_from_slice(&((message.len() + 1) as u32).to_le_bytes());
    write_c_string(&mut body, message);
    body.push(CHAT_TAG_NONE);
    body
}

fn chat_radius_yards(chat_type: u32) -> f32 {
    match chat_type {
        CHAT_MSG_SAY => CHAT_SAY_RADIUS_YARDS,
        CHAT_MSG_YELL => CHAT_YELL_RADIUS_YARDS,
        CHAT_MSG_EMOTE => CHAT_EMOTE_RADIUS_YARDS,
        _ => 0.0,
    }
}

async fn handle_text_emote(
    stream: &mut WorldPacketSink,
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
    gameobject_target: Option<ObjectGuid>,
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
        let gameobject_target =
            if target_mask & (SPELL_CAST_TARGET_GAMEOBJECT | SPELL_CAST_TARGET_LOCKED) != 0 {
                Some(read_packed_guid(body, cursor)?)
            } else {
                None
            };

        Ok(Self {
            target_mask,
            unit_target,
            gameobject_target,
        })
    }

    fn write(&self, body: &mut Vec<u8>) -> anyhow::Result<()> {
        let target_mask = self.target_mask & !SPELL_CAST_TARGET_UNIT_ENEMY;
        body.extend_from_slice(&target_mask.to_le_bytes());
        if target_mask & SPELL_CAST_TARGET_UNIT != 0 {
            PackedGuid::write(body, self.unit_target.unwrap_or(ObjectGuid::EMPTY))?;
        }
        if target_mask & (SPELL_CAST_TARGET_GAMEOBJECT | SPELL_CAST_TARGET_LOCKED) != 0 {
            PackedGuid::write(body, self.gameobject_target.unwrap_or(ObjectGuid::EMPTY))?;
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

