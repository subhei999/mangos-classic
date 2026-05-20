use super::*;
use wow_proto::world::WorldOpcode;
use wow_proto::{
    ServerWorldPacket, SmsgEmoteResponse, SmsgMessageChatResponse, SmsgNameQueryResponse,
    SmsgTextEmoteResponse,
};

pub(in crate::world) async fn dispatch_chat_packet(
    ctx: &mut WorldPacketDispatchContext<'_>,
    packet: &packets::ParsedWorldClientPacket,
) -> anyhow::Result<()> {
    match packet {
        packets::ParsedWorldClientPacket::Ping(_) => {
            handle_ping(
                &mut *ctx.stream,
                packet.ping()?,
                Some(&mut *ctx.header_crypto),
            )
            .await
        }
        packets::ParsedWorldClientPacket::NameQuery(_) => {
            handle_name_query(
                &mut *ctx.stream,
                ctx.character_db_pool,
                &ctx.runtime_state.playerbots,
                packet.name_query()?,
                &mut *ctx.header_crypto,
            )
            .await
        }
        packets::ParsedWorldClientPacket::MessageChat(_) => {
            handle_message_chat(
                &mut *ctx.stream,
                ChatDeps {
                    character_db_pool: ctx.character_db_pool,
                    world_db_pool: ctx.world_db_pool,
                    object_mgr: &ctx.runtime_state.object_mgr,
                    maps: &ctx.runtime_state.maps,
                    sessions: &ctx.runtime_state.sessions,
                    parties: &ctx.runtime_state.parties,
                },
                packet.message_chat()?,
                &mut *ctx.session,
                &mut *ctx.header_crypto,
            )
            .await
        }
        packets::ParsedWorldClientPacket::JoinChannel(_) => {
            handle_join_channel(
                &mut *ctx.stream,
                packet.join_channel()?,
                &mut *ctx.header_crypto,
            )
            .await
        }
        packets::ParsedWorldClientPacket::TextEmote(_) => {
            handle_text_emote(
                &mut *ctx.stream,
                TextEmoteDeps {
                    maps: &ctx.runtime_state.maps,
                    sessions: &ctx.runtime_state.sessions,
                },
                packet.text_emote()?,
                &mut *ctx.session,
                &mut *ctx.header_crypto,
            )
            .await
        }
        other => anyhow::bail!("chat router received opcode 0x{:04X}", other.opcode()),
    }
}

pub(in crate::world) async fn handle_ping(
    stream: &mut WorldPacketSink,
    ping: wow_proto::PingRequest,
    header_crypto: Option<&mut HeaderCrypto>,
) -> anyhow::Result<()> {
    let pong = wow_proto::PongResponse::from(ping);
    send_packet(
        stream,
        u32::from(WorldOpcode::SmsgPong) as u16,
        &pong.to_body(),
        header_crypto,
    )
    .await
}

pub(in crate::world) async fn handle_name_query(
    stream: &mut WorldPacketSink,
    character_db_pool: &MySqlPool,
    playerbots: &PlayerbotRoster,
    request: wow_proto::NameQueryRequest,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    let raw_guid = request.raw_guid;
    let guid = ObjectGuid::from_raw(raw_guid);
    let character_guid = guid.counter();
    let character = wow_db::get_character_name_query(character_db_pool, character_guid).await?;
    let bot_character = character
        .is_none()
        .then(|| playerbots.name_query(character_guid))
        .flatten();
    let response = build_name_query_response(raw_guid, character.as_ref());
    let response = if character.is_some() {
        response
    } else {
        build_name_query_response(raw_guid, bot_character.as_ref())
    };
    send_packet(
        stream,
        WorldOpcode::SmsgNameQueryResponse as u16,
        &response,
        Some(header_crypto),
    )
    .await
}

pub(in crate::world) fn build_name_query_response(
    requested_guid: u64,
    character: Option<&CharacterNameQuery>,
) -> Vec<u8> {
    let response = if let Some(character) = character {
        SmsgNameQueryResponse {
            guid: ObjectGuid::new(HighGuid::Player, 0, character.guid),
            name: character.name.clone(),
            realm_name: String::new(),
            race: character.race as u32,
            gender: character.gender as u32,
            class: character.class as u32,
        }
    } else {
        SmsgNameQueryResponse {
            guid: ObjectGuid::from_raw(requested_guid),
            name: "Unknown".to_string(),
            realm_name: String::new(),
            race: 0,
            gender: 0,
            class: 0,
        }
    };
    response.body()
}

pub(in crate::world) async fn handle_message_chat(
    stream: &mut WorldPacketSink,
    deps: ChatDeps<'_>,
    chat: wow_proto::MessageChatRequest,
    session: &mut WorldSessionState,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    if !matches!(
        chat.chat_type,
        CHAT_MSG_SAY | CHAT_MSG_PARTY | CHAT_MSG_YELL | CHAT_MSG_EMOTE
    ) {
        info!(
            chat_type = chat.chat_type,
            language = chat.language,
            "Ignoring unsupported chat message type"
        );
        return Ok(());
    }

    let Some(character) = session.character.active_character.clone() else {
        warn!(
            chat_type = chat.chat_type,
            "Ignoring chat before character login"
        );
        return Ok(());
    };
    if chat.message.is_empty() {
        return Ok(());
    }
    if chat.message.starts_with('.') {
        handle_gm_dot_command(stream, deps, &chat.message, session, header_crypto).await?;
        return Ok(());
    }

    let body = build_message_chat_body(chat.chat_type, chat.language, &chat.message, &character);
    send_packet(
        stream,
        WorldOpcode::SmsgMessageChat as u16,
        &body,
        Some(header_crypto),
    )
    .await?;

    if chat.chat_type == CHAT_MSG_PARTY {
        let party_members = deps.parties.party_members(character.guid).await;
        let mut packets = Vec::new();
        for member in party_members {
            if member.guid == character.guid {
                continue;
            }
            if let Some(session_id) = deps.sessions.session_for_character(member.guid).await {
                packets.push((
                    session_id,
                    OutboundWorldPacket {
                        opcode: WorldOpcode::SmsgMessageChat as u16,
                        body: body.clone(),
                    },
                ));
            }
        }
        deps.sessions.dispatch(packets).await;
        return Ok(());
    }

    let radius = chat_radius_yards(chat.chat_type);
    if radius > 0.0 {
        let packets = deps
            .maps
            .broadcast_nearby_player_packet(
                character.position.map_id,
                character.guid,
                radius,
                OutboundWorldPacket {
                    opcode: WorldOpcode::SmsgMessageChat as u16,
                    body,
                },
            )
            .await;
        deps.sessions.dispatch(packets).await;
    }
    Ok(())
}

pub(in crate::world) struct ChatDeps<'a> {
    pub(in crate::world) character_db_pool: &'a MySqlPool,
    pub(in crate::world) world_db_pool: &'a MySqlPool,
    pub(in crate::world) object_mgr: &'a ObjectMgr,
    pub(in crate::world) maps: &'a Arc<MapRuntimeManager>,
    pub(in crate::world) sessions: &'a Arc<SessionRegistry>,
    pub(in crate::world) parties: &'a Arc<PartyManager>,
}

pub(in crate::world) fn build_message_chat_body(
    chat_type: u32,
    language: u32,
    message: &str,
    character: &ActiveCharacter,
) -> Vec<u8> {
    let sender = ObjectGuid::new(HighGuid::Player, 0, character.guid);
    let target = match chat_type {
        CHAT_MSG_SAY | CHAT_MSG_PARTY | CHAT_MSG_YELL => Some(sender),
        _ => None,
    };
    SmsgMessageChatResponse {
        chat_type: chat_type as u8,
        language,
        sender,
        target,
        message: message.to_string(),
        tag: CHAT_TAG_NONE,
    }
    .body()
}

pub(in crate::world) fn build_system_message_chat_body(message: &str) -> Vec<u8> {
    SmsgMessageChatResponse {
        chat_type: CHAT_MSG_SYSTEM as u8,
        language: LANG_UNIVERSAL,
        sender: ObjectGuid::EMPTY,
        target: None,
        message: message.to_string(),
        tag: CHAT_TAG_NONE,
    }
    .body()
}

pub(in crate::world) fn chat_radius_yards(chat_type: u32) -> f32 {
    match chat_type {
        CHAT_MSG_SAY => CHAT_SAY_RADIUS_YARDS,
        CHAT_MSG_YELL => CHAT_YELL_RADIUS_YARDS,
        CHAT_MSG_EMOTE => CHAT_EMOTE_RADIUS_YARDS,
        _ => 0.0,
    }
}

pub(in crate::world) async fn handle_text_emote(
    stream: &mut WorldPacketSink,
    deps: TextEmoteDeps<'_>,
    emote: wow_proto::TextEmoteRequest,
    session: &mut WorldSessionState,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    let Some(character) = &session.character.active_character else {
        warn!(
            text_emote = emote.text_emote,
            "Ignoring text emote before character login"
        );
        return Ok(());
    };
    let target_name =
        text_emote_target_name(deps, character.position.map_id, emote.target_raw_guid)
            .await
            .unwrap_or_default();
    if let Some(animation) = animation_emote_for_text_emote(emote.text_emote) {
        if matches!(emote.text_emote, TEXTEMOTE_DANCE | TEXTEMOTE_SLEEP) {
            session.character.player_emote_state = animation;
            let body = build_emote_state_update_body(character, animation)?;
            send_packet(
                stream,
                WorldOpcode::SmsgUpdateObject as u16,
                &body,
                Some(header_crypto),
            )
            .await?;
            dispatch_nearby_text_emote_packet(
                deps,
                character,
                WorldOpcode::SmsgUpdateObject as u16,
                body,
            )
            .await;
        } else {
            let body = build_emote_body(character, animation);
            send_packet(
                stream,
                WorldOpcode::SmsgEmote as u16,
                &body,
                Some(header_crypto),
            )
            .await?;
            dispatch_nearby_text_emote_packet(deps, character, WorldOpcode::SmsgEmote as u16, body)
                .await;
        }
    }
    let body = build_text_emote_body(
        character,
        emote.text_emote,
        emote.emote_num,
        (!target_name.is_empty()).then_some(target_name.as_str()),
    );
    send_packet(
        stream,
        WorldOpcode::SmsgTextEmote as u16,
        &body,
        Some(header_crypto),
    )
    .await?;
    dispatch_nearby_text_emote_packet(deps, character, WorldOpcode::SmsgTextEmote as u16, body)
        .await;
    Ok(())
}

#[derive(Clone, Copy)]
pub(in crate::world) struct TextEmoteDeps<'a> {
    pub(in crate::world) maps: &'a Arc<MapRuntimeManager>,
    pub(in crate::world) sessions: &'a Arc<SessionRegistry>,
}

pub(in crate::world) async fn dispatch_nearby_text_emote_packet(
    deps: TextEmoteDeps<'_>,
    character: &ActiveCharacter,
    opcode: u16,
    body: Vec<u8>,
) {
    let packets = deps
        .maps
        .broadcast_nearby_player_packet(
            character.position.map_id,
            character.guid,
            CHAT_EMOTE_RADIUS_YARDS,
            OutboundWorldPacket { opcode, body },
        )
        .await;
    deps.sessions.dispatch(packets).await;
}

pub(in crate::world) async fn text_emote_target_name(
    deps: TextEmoteDeps<'_>,
    map_id: u32,
    target_guid: u64,
) -> Option<String> {
    if target_guid == ObjectGuid::EMPTY.raw() {
        return None;
    }
    let target = ObjectGuid::from_raw(target_guid);
    match target.high_type() {
        Some(HighGuid::Player) => {
            deps.sessions
                .character_name_for_guid(target.counter())
                .await
        }
        Some(HighGuid::Unit) => deps
            .maps
            .db_creature_snapshot(map_id, target)
            .await
            .map(|creature| creature.spawn.template.name),
        _ => None,
    }
}

pub(in crate::world) fn build_text_emote_body(
    character: &ActiveCharacter,
    text_emote: u32,
    emote_num: u32,
    target_name: Option<&str>,
) -> Vec<u8> {
    let sender = ObjectGuid::new(HighGuid::Player, 0, character.guid);
    let target_name = target_name.unwrap_or("");
    SmsgTextEmoteResponse {
        sender,
        text_emote,
        emote_num,
        target_name: target_name.to_string(),
    }
    .body()
}

pub(in crate::world) fn animation_emote_for_text_emote(text_emote: u32) -> Option<u32> {
    match text_emote {
        TEXTEMOTE_WAVE => Some(EMOTE_ONESHOT_WAVE),
        TEXTEMOTE_POINT => Some(EMOTE_ONESHOT_POINT),
        TEXTEMOTE_DANCE => Some(EMOTE_STATE_DANCE),
        TEXTEMOTE_SLEEP => Some(EMOTE_STATE_SLEEP),
        _ => None,
    }
}

pub(in crate::world) fn build_emote_body(character: &ActiveCharacter, emote: u32) -> Vec<u8> {
    let sender = ObjectGuid::new(HighGuid::Player, 0, character.guid);
    SmsgEmoteResponse { emote, sender }.body()
}

pub(in crate::world) fn build_emote_state_update_body(
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
