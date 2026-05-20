use super::*;
use wow_proto::world::WorldOpcode;
use wow_proto::{
    ServerWorldPacket, SmsgAuthResponse, SmsgCharCreateResponse, SmsgCharDeleteResponse,
};

// CMaNGOS reference: src/game/Handlers/CharacterHandler.cpp character screen flow.

pub(in crate::world) fn normalize_character_name(name: &str) -> Result<String, u8> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err(CHAR_NAME_NO_NAME);
    }
    if trimmed.len() < 2 {
        return Err(CHAR_NAME_TOO_SHORT);
    }
    if trimmed.len() > 12 {
        return Err(CHAR_NAME_TOO_LONG);
    }
    if !trimmed.bytes().all(|b| b.is_ascii_alphabetic()) {
        return Err(CHAR_NAME_INVALID_CHARACTER);
    }

    let mut chars = trimmed.chars();
    let first = chars.next().expect("empty name checked above");
    let normalized = first.to_ascii_uppercase().to_string() + &chars.as_str().to_ascii_lowercase();
    Ok(normalized)
}

pub(in crate::world) fn is_valid_race_class(race: u8, class: u8) -> bool {
    matches!(
        (race, class),
        (1, 1 | 2 | 4 | 5 | 8 | 9)
            | (2, 1 | 3 | 4 | 7 | 9)
            | (3, 1..=5)
            | (4, 1 | 3 | 4 | 5 | 11)
            | (5, 1 | 4 | 5 | 8 | 9)
            | (6, 1 | 3 | 7 | 11)
            | (7, 1 | 4 | 8 | 9)
            | (8, 1 | 3 | 4 | 5 | 7 | 8)
    )
}

pub(in crate::world) async fn send_auth_response(
    stream: &mut TcpStream,
    response: u8,
) -> anyhow::Result<()> {
    send_packet_direct(
        stream,
        WorldOpcode::SmsgAuthResponse as u16,
        &SmsgAuthResponse {
            result: response,
            billing_time_remaining: 0,
            billing_plan_flags: 0,
            billing_time_rested: 0,
            expansion: 0,
        }
        .body(),
        None,
    )
    .await
}

pub(in crate::world) async fn send_auth_ok(
    stream: &mut WorldPacketSink,
    header_crypto: Option<&mut HeaderCrypto>,
) -> anyhow::Result<()> {
    let body = SmsgAuthResponse::ok().body();
    send_packet(
        stream,
        WorldOpcode::SmsgAuthResponse as u16,
        &body,
        header_crypto,
    )
    .await
}

pub(in crate::world) async fn send_char_enum(
    stream: &mut WorldPacketSink,
    characters: &[CharacterEnumEntry],
    header_crypto: Option<&mut HeaderCrypto>,
) -> anyhow::Result<()> {
    let body = build_char_enum_body(characters)?;
    send_packet(
        stream,
        WorldOpcode::SmsgCharEnum as u16,
        &body,
        header_crypto,
    )
    .await
}

pub(in crate::world) async fn handle_char_delete(
    stream: &mut WorldPacketSink,
    login_db_pool: &MySqlPool,
    character_db_pool: &MySqlPool,
    account_id: u32,
    request: wow_proto::CharDeleteRequest,
    header_crypto: &mut HeaderCrypto,
    runtime_state: &WorldRuntimeState,
) -> anyhow::Result<()> {
    let raw_guid = request.raw_guid;
    let guid = ObjectGuid::from_raw(raw_guid).counter();
    if runtime_state.online_characters.lock().await.contains(&guid) {
        warn!(account_id, guid, "Rejected loaded character delete");
        return send_char_delete_result(stream, CHAR_DELETE_FAILED, Some(header_crypto)).await;
    }
    if wow_db::is_guild_leader(character_db_pool, guid).await? {
        warn!(account_id, guid, "Rejected guild leader character delete");
        return send_char_delete_result(stream, CHAR_DELETE_FAILED, Some(header_crypto)).await;
    }

    let deleted = wow_db::delete_character_with_options(
        character_db_pool,
        account_id,
        guid,
        runtime_state.delete_options,
    )
    .await?;
    if deleted {
        let count = wow_db::refresh_realm_character_count(
            login_db_pool,
            character_db_pool,
            account_id,
            REALM_ID,
        )
        .await?;
        info!(account_id, guid, count, "Deleted character");
        send_char_delete_result(stream, CHAR_DELETE_SUCCESS, Some(header_crypto)).await
    } else {
        warn!(account_id, guid, "Rejected character delete");
        send_char_delete_result(stream, CHAR_DELETE_FAILED, Some(header_crypto)).await
    }
}

pub(in crate::world) async fn send_char_delete_result(
    stream: &mut WorldPacketSink,
    result: u8,
    header_crypto: Option<&mut HeaderCrypto>,
) -> anyhow::Result<()> {
    send_packet(
        stream,
        WorldOpcode::SmsgCharDelete as u16,
        &SmsgCharDeleteResponse { result }.body(),
        header_crypto,
    )
    .await
}

pub(in crate::world) async fn handle_char_create(
    stream: &mut WorldPacketSink,
    login_db_pool: &MySqlPool,
    character_db_pool: &MySqlPool,
    world_db_pool: &MySqlPool,
    account_id: u32,
    create: wow_proto::CharCreateRequest,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    let name = match normalize_character_name(&create.name) {
        Ok(name) => name,
        Err(code) => {
            send_char_create_result(stream, code, Some(header_crypto)).await?;
            return Ok(());
        }
    };

    if !is_valid_race_class(create.race, create.class) || create.gender > 1 {
        warn!(
            account_id,
            race = create.race,
            class = create.class,
            gender = create.gender,
            "Rejected invalid character create attributes"
        );
        send_char_create_result(stream, CHAR_CREATE_FAILED, Some(header_crypto)).await?;
        return Ok(());
    }

    if wow_db::character_name_exists(character_db_pool, &name).await? {
        send_char_create_result(stream, CHAR_CREATE_NAME_IN_USE, Some(header_crypto)).await?;
        return Ok(());
    }

    let char_count = wow_db::character_count_for_account(character_db_pool, account_id).await?;
    if char_count >= MAX_CHARACTERS_PER_REALM {
        send_char_create_result(stream, CHAR_CREATE_SERVER_LIMIT, Some(header_crypto)).await?;
        return Ok(());
    }

    let created = wow_db::create_character(
        character_db_pool,
        world_db_pool,
        NewCharacter {
            account_id,
            name,
            race: create.race,
            class: create.class,
            gender: create.gender,
            skin: create.skin,
            face: create.face,
            hair_style: create.hair_style,
            hair_color: create.hair_color,
            facial_hair: create.facial_hair,
        },
    )
    .await?;

    let new_count = wow_db::refresh_realm_character_count(
        login_db_pool,
        character_db_pool,
        account_id,
        REALM_ID,
    )
    .await?;

    info!(
        account_id,
        guid = created.guid,
        name = %created.name,
        race = created.race,
        class = created.class,
        count = new_count,
        "Created character"
    );

    send_char_create_result(stream, CHAR_CREATE_SUCCESS, Some(header_crypto)).await
}

pub(in crate::world) async fn send_char_create_result(
    stream: &mut WorldPacketSink,
    result: u8,
    header_crypto: Option<&mut HeaderCrypto>,
) -> anyhow::Result<()> {
    send_packet(
        stream,
        WorldOpcode::SmsgCharCreate as u16,
        &SmsgCharCreateResponse { result }.body(),
        header_crypto,
    )
    .await
}
