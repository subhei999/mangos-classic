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

    if packet.spell_id != WARRIOR_HEROIC_STRIKE_RANK_1 {
        warn!(
            spell_id = packet.spell_id,
            "Ignoring unsupported spell cast in starter spell fixture slice"
        );
        return Ok(());
    }

    let caster = ObjectGuid::new(HighGuid::Player, 0, character_guid);
    let targets = normalize_fixture_spell_targets(packet.targets);
    session.player_rage = session.player_rage.saturating_sub(HEROIC_STRIKE_RAGE_COST);
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
        let damage = session
            .combat_dummy_health
            .min(HEROIC_STRIKE_FIXTURE_DAMAGE);
        session.combat_dummy_health = session.combat_dummy_health.saturating_sub(damage);
        if session.combat_dummy_health == 0 {
            session.combat_dummy_lootable = true;
            session.combat_dummy_looting = false;
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
    }
    send_packet(
        stream,
        SMSG_UPDATE_OBJECT,
        &build_player_rage_update_body(caster, session.player_rage)?,
        Some(header_crypto),
    )
    .await
}

fn normalize_fixture_spell_targets(mut targets: SpellCastTargets) -> SpellCastTargets {
    targets.target_mask =
        (targets.target_mask | SPELL_CAST_TARGET_UNIT) & !SPELL_CAST_TARGET_UNIT_ENEMY;
    targets.unit_target = Some(targets.unit_target.unwrap_or_else(rust_combat_dummy_guid));
    targets
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

async fn handle_creature_query(
    stream: &mut TcpStream,
    body: &[u8],
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    let query = CreatureQuery::read(body)?;
    info!(
        entry = query.entry,
        guid = format_args!("0x{:016X}", query.guid.raw()),
        found = matches!(query.entry, RUST_GUIDE_ENTRY | RUST_COMBAT_DUMMY_ENTRY),
        "Answering creature template query"
    );
    let response = build_creature_query_response(query.entry);
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

fn build_creature_query_response(entry: u32) -> Vec<u8> {
    let Some(template) = fixture_creature_template(entry) else {
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
    body.extend_from_slice(&7u32.to_le_bytes()); // humanoid
    body.extend_from_slice(&0u32.to_le_bytes()); // family
    body.extend_from_slice(&0u32.to_le_bytes()); // rank
    body.extend_from_slice(&0u32.to_le_bytes()); // unknown
    body.extend_from_slice(&0u32.to_le_bytes()); // pet spell data id
    body.extend_from_slice(&template.display_id.to_le_bytes());
    body.extend_from_slice(&0u16.to_le_bytes()); // civilian
    body
}

struct FixtureCreatureTemplate {
    name: &'static str,
    subname: &'static str,
    display_id: u32,
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
    body: &[u8],
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    let guid = read_packet_guid(body, "CMSG_GOSSIP_HELLO")?;
    if guid != rust_guide_guid() {
        warn!(
            guid = format_args!("0x{:016X}", guid.raw()),
            "Ignoring gossip hello for unknown creature"
        );
        return Ok(());
    }

    let text_update = build_rust_guide_npc_text_update(RUST_GUIDE_GOSSIP_TEXT_ID);
    send_packet(
        stream,
        SMSG_NPC_TEXT_UPDATE,
        &text_update,
        Some(&mut *header_crypto),
    )
    .await?;
    let response = build_rust_guide_gossip_message();
    send_packet(stream, SMSG_GOSSIP_MESSAGE, &response, Some(header_crypto)).await
}

async fn handle_gossip_select_option(
    stream: &mut TcpStream,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    send_packet(stream, SMSG_GOSSIP_COMPLETE, &[], Some(header_crypto)).await
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
    let response = build_rust_guide_npc_text_update(text_id);
    send_packet(stream, SMSG_NPC_TEXT_UPDATE, &response, Some(header_crypto)).await
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

    if target != rust_combat_dummy_guid() {
        warn!(
            target = format_args!("0x{:016X}", target.raw()),
            "Ignoring attack swing against unknown target"
        );
        return Ok(());
    }
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
    send_combat_dummy_swing(stream, session, header_crypto).await
}

async fn handle_combat_tick(
    stream: &mut TcpStream,
    session: &mut WorldSessionState,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    if session.active_combat_target != Some(rust_combat_dummy_guid()) {
        return Ok(());
    }
    send_combat_dummy_swing(stream, session, header_crypto).await
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
    session.active_combat_target = None;
    let attacker = ObjectGuid::new(HighGuid::Player, 0, character.guid);
    send_packet(
        stream,
        SMSG_ATTACKSTOP,
        &build_attack_stop_body(attacker, rust_combat_dummy_guid(), false)?,
        Some(header_crypto),
    )
    .await
}

async fn handle_loot(
    stream: &mut TcpStream,
    body: &[u8],
    session: &mut WorldSessionState,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    let target = read_packet_guid(body, "CMSG_LOOT")?;
    if target != rust_combat_dummy_guid() {
        warn!(
            target = format_args!("0x{:016X}", target.raw()),
            "Ignoring loot request for unknown target"
        );
        return Ok(());
    }
    if !session.combat_dummy_lootable {
        warn!("Ignoring loot request for combat dummy before it is lootable");
        return Ok(());
    }

    session.combat_dummy_looting = true;
    let response = build_combat_dummy_loot_response_body();
    send_packet(stream, SMSG_LOOT_RESPONSE, &response, Some(header_crypto)).await
}

async fn handle_loot_money(
    stream: &mut TcpStream,
    session: &mut WorldSessionState,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    if !session.combat_dummy_looting {
        warn!("Ignoring loot money request without an open combat dummy loot window");
        return Ok(());
    }

    send_packet(stream, SMSG_LOOT_CLEAR_MONEY, &[], Some(header_crypto)).await
}

async fn handle_loot_release(
    stream: &mut TcpStream,
    body: &[u8],
    session: &mut WorldSessionState,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    let target = read_packet_guid(body, "CMSG_LOOT_RELEASE")?;
    if target != rust_combat_dummy_guid() {
        warn!(
            target = format_args!("0x{:016X}", target.raw()),
            "Ignoring loot release for unknown target"
        );
        return Ok(());
    }

    session.combat_dummy_looting = false;
    session.combat_dummy_lootable = false;
    session.combat_dummy_health = RUST_COMBAT_DUMMY_HEALTH;
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
        &build_combat_dummy_state_update_body(RUST_COMBAT_DUMMY_HEALTH, 0)?,
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

fn build_combat_dummy_loot_response_body() -> Vec<u8> {
    let mut body = Vec::with_capacity(14);
    body.extend_from_slice(&rust_combat_dummy_guid().raw().to_le_bytes());
    body.push(CLIENT_LOOT_CORPSE);
    body.extend_from_slice(&0u32.to_le_bytes()); // gold
    body.push(0); // item count
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

fn read_packet_guid(body: &[u8], packet_name: &str) -> anyhow::Result<ObjectGuid> {
    if body.len() < 8 {
        anyhow::bail!("{packet_name} payload must include an 8-byte GUID");
    }
    Ok(ObjectGuid::from_raw(u64::from_le_bytes(
        body[0..8].try_into()?,
    )))
}

fn build_rust_guide_gossip_message() -> Vec<u8> {
    let guid = rust_guide_guid();
    let mut body = Vec::with_capacity(32 + RUST_GUIDE_GOSSIP_OPTION.len());
    body.extend_from_slice(&guid.raw().to_le_bytes());
    body.extend_from_slice(&RUST_GUIDE_GOSSIP_TEXT_ID.to_le_bytes());
    body.extend_from_slice(&1u32.to_le_bytes()); // gossip option count
    body.extend_from_slice(&0u32.to_le_bytes()); // option index
    body.push(0); // icon
    body.push(0); // coded
    write_c_string(&mut body, RUST_GUIDE_GOSSIP_OPTION);
    body.extend_from_slice(&0u32.to_le_bytes()); // quest option count
    body
}

fn build_rust_guide_npc_text_update(text_id: u32) -> Vec<u8> {
    let mut body = Vec::with_capacity(220);
    body.extend_from_slice(&text_id.to_le_bytes());
    for index in 0..8 {
        body.extend_from_slice(&(if index == 0 { 1.0f32 } else { 0.0f32 }).to_le_bytes());
        let text = if index == 0 {
            RUST_GUIDE_GOSSIP_TEXT
        } else {
            ""
        };
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

