use sha1::{Digest, Sha1};
use sqlx::mysql::MySqlPool;
use std::collections::{HashMap, HashSet};
use std::io::Cursor;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::Mutex;
use tokio::time::{timeout, Duration};
use tracing::{error, info, warn};
use wow_common::guid::{write_guid, HighGuid, ObjectGuid, PackedGuid};
use wow_common::position::WorldPosition;
use wow_crypto::HeaderCrypto;
use wow_db::{
    CharacterAction, CharacterDeleteOptions, CharacterEnumEntry, CharacterInventoryItem,
    CharacterNameQuery, CharacterQuestStatus, CharacterReputation, CharacterSkill, CharacterSpell,
    CreatureLootQuery, CreatureSpawnQuery, CreatureTemplateQuery, ItemTemplateQuery, NewCharacter,
    PlayerWorldStats, QuestTemplateQuery,
};

include!("opcodes.rs");
include!("session.rs");

pub struct WorldServer {
    bind_addr: SocketAddr,
    login_db_pool: MySqlPool,
    character_db_pool: MySqlPool,
    world_db_pool: MySqlPool,
    runtime_state: WorldRuntimeState,
}

impl WorldServer {
    pub fn new(
        bind_addr: SocketAddr,
        login_db_pool: MySqlPool,
        character_db_pool: MySqlPool,
        world_db_pool: MySqlPool,
        delete_options: CharacterDeleteOptions,
    ) -> Self {
        Self {
            bind_addr,
            login_db_pool,
            character_db_pool,
            world_db_pool,
            runtime_state: WorldRuntimeState {
                online_characters: Arc::new(Mutex::new(HashSet::new())),
                delete_options,
            },
        }
    }

    pub async fn run(&self) -> anyhow::Result<()> {
        let listener = TcpListener::bind(self.bind_addr).await?;
        info!("World server listening on {}", self.bind_addr);

        loop {
            match listener.accept().await {
                Ok((socket, peer)) => {
                    info!(%peer, "Accepted world connection");
                    let login_pool = self.login_db_pool.clone();
                    let character_pool = self.character_db_pool.clone();
                    let world_pool = self.world_db_pool.clone();
                    let runtime_state = self.runtime_state.clone();
                    tokio::spawn(async move {
                        if let Err(e) = handle_client(
                            socket,
                            login_pool,
                            character_pool,
                            world_pool,
                            runtime_state,
                        )
                        .await
                        {
                            warn!(%peer, "World session ended with error: {}", e);
                        }
                    });
                }
                Err(e) => error!("Failed to accept world connection: {}", e),
            }
        }
    }
}

async fn handle_client(
    mut stream: TcpStream,
    login_db_pool: MySqlPool,
    character_db_pool: MySqlPool,
    world_db_pool: MySqlPool,
    runtime_state: WorldRuntimeState,
) -> anyhow::Result<()> {
    send_packet(
        &mut stream,
        SMSG_AUTH_CHALLENGE,
        &SERVER_SEED.to_le_bytes(),
        None,
    )
    .await?;

    let (opcode, payload) = read_client_packet(&mut stream, None).await?;
    if opcode != CMSG_AUTH_SESSION {
        anyhow::bail!("expected CMSG_AUTH_SESSION, got 0x{opcode:04X}");
    }

    let auth = AuthSessionPacket::read(&payload)?;
    info!(
        account = %auth.account,
        build = auth.client_build,
        client_seed = format_args!("0x{:08X}", auth.client_seed),
        addon_bytes = auth.addon_data.len(),
        "Received CMSG_AUTH_SESSION"
    );

    if !matches!(auth.client_build, 5875 | 6005 | 6141) {
        send_auth_response(&mut stream, AUTH_VERSION_MISMATCH).await?;
        anyhow::bail!("unsupported world client build {}", auth.client_build);
    }

    let account = wow_db::account::get_account_by_username(&login_db_pool, &auth.account)
        .await?
        .ok_or_else(|| anyhow::anyhow!("unknown account {}", auth.account))?;

    if account.sessionkey.trim().is_empty() {
        send_auth_response(&mut stream, AUTH_UNKNOWN_ACCOUNT).await?;
        anyhow::bail!("account {} has no auth session key", auth.account);
    }

    let session_key = hex_to_array40(&account.sessionkey)?;
    if !verify_world_digest(&auth, &session_key) {
        send_auth_response(&mut stream, AUTH_FAILED).await?;
        anyhow::bail!("world auth digest mismatch for account {}", auth.account);
    }

    info!(
        account = %auth.account,
        account_id = account.id,
        "World auth session verified"
    );

    let mut header_crypto = HeaderCrypto::new(&session_key);
    send_auth_ok(&mut stream, Some(&mut header_crypto)).await?;
    let mut session = WorldSessionState::default();

    loop {
        match timeout(
            Duration::from_millis(RUST_COMBAT_SWING_MILLIS),
            read_client_packet(&mut stream, Some(&mut header_crypto)),
        )
        .await
        {
            Ok(Ok((opcode, body))) => {
                info!(
                    opcode = format_args!("0x{opcode:04X}"),
                    bytes = body.len(),
                    "Received world packet after auth"
                );

                match opcode {
                    CMSG_CHAR_CREATE => {
                        handle_char_create(
                            &mut stream,
                            &login_db_pool,
                            &character_db_pool,
                            &world_db_pool,
                            account.id,
                            &body,
                            &mut header_crypto,
                        )
                        .await?;
                    }
                    CMSG_CHAR_ENUM => {
                        let characters =
                            wow_db::get_character_enum_entries(&character_db_pool, account.id)
                                .await?;
                        info!(
                            account = %auth.account,
                            count = characters.len(),
                            "Sending character enum"
                        );
                        send_char_enum(&mut stream, &characters, Some(&mut header_crypto)).await?;
                    }
                    CMSG_CHAR_DELETE => {
                        handle_char_delete(
                            &mut stream,
                            &login_db_pool,
                            &character_db_pool,
                            account.id,
                            &body,
                            &mut header_crypto,
                            &runtime_state,
                        )
                        .await?;
                    }
                    CMSG_PLAYER_LOGIN => {
                        handle_player_login(
                            &mut stream,
                            PlayerLoginDeps {
                                character_db_pool: &character_db_pool,
                                world_db_pool: &world_db_pool,
                                online_characters: &runtime_state.online_characters,
                            },
                            account.id,
                            &body,
                            &mut header_crypto,
                            &mut session,
                        )
                        .await?;
                    }
                    CMSG_PING => {
                        handle_ping(&mut stream, &body, Some(&mut header_crypto)).await?;
                    }
                    CMSG_NAME_QUERY => {
                        handle_name_query(
                            &mut stream,
                            &character_db_pool,
                            &body,
                            &mut header_crypto,
                        )
                        .await?;
                    }
                    CMSG_ITEM_QUERY_SINGLE => {
                        handle_item_query_single(
                            &mut stream,
                            &world_db_pool,
                            &body,
                            &mut header_crypto,
                        )
                        .await?;
                    }
                    CMSG_CREATURE_QUERY => {
                        handle_creature_query(
                            &mut stream,
                            &world_db_pool,
                            &body,
                            &mut header_crypto,
                        )
                        .await?;
                    }
                    CMSG_QUEST_QUERY => {
                        handle_quest_query(&mut stream, &world_db_pool, &body, &mut header_crypto)
                            .await?;
                    }
                    CMSG_MESSAGECHAT => {
                        handle_message_chat(&mut stream, &body, &session, &mut header_crypto)
                            .await?;
                    }
                    CMSG_QUERY_TIME => {
                        handle_query_time(&mut stream, &mut header_crypto).await?;
                    }
                    CMSG_REQUEST_ACCOUNT_DATA => {
                        handle_request_account_data(&mut stream, &body, &mut header_crypto).await?;
                    }
                    CMSG_UPDATE_ACCOUNT_DATA => {
                        handle_update_account_data(&body);
                    }
                    CMSG_TUTORIAL_FLAG => {
                        handle_tutorial_flag(&character_db_pool, account.id, &body).await?;
                    }
                    CMSG_TUTORIAL_CLEAR => {
                        handle_tutorial_clear(&character_db_pool, account.id).await?;
                    }
                    CMSG_TUTORIAL_RESET => {
                        handle_tutorial_reset(&character_db_pool, account.id).await?;
                    }
                    CMSG_TEXT_EMOTE => {
                        handle_text_emote(&mut stream, &body, &session, &mut header_crypto).await?;
                    }
                    CMSG_CAST_SPELL => {
                        handle_cast_spell(
                            &mut stream,
                            &character_db_pool,
                            &world_db_pool,
                            &body,
                            &mut session,
                            &mut header_crypto,
                        )
                        .await?;
                    }
                    CMSG_AUTOEQUIP_ITEM | CMSG_SWAP_ITEM | CMSG_SWAP_INV_ITEM => {
                        handle_inventory_swap(
                            &mut stream,
                            &character_db_pool,
                            &world_db_pool,
                            opcode,
                            &body,
                            &mut session,
                            &mut header_crypto,
                        )
                        .await?;
                    }
                    CMSG_DESTROYITEM => {
                        handle_destroy_item(
                            &mut stream,
                            &character_db_pool,
                            &world_db_pool,
                            &body,
                            &mut session,
                            &mut header_crypto,
                        )
                        .await?;
                    }
                    CMSG_SPLIT_ITEM => {
                        handle_split_item(
                            &mut stream,
                            &character_db_pool,
                            &body,
                            &mut session,
                            &mut header_crypto,
                        )
                        .await?;
                    }
                    CMSG_CANCEL_CAST | CMSG_CANCEL_AUTO_REPEAT_SPELL => {
                        info!(
                            opcode = expected_noop_opcode_name(opcode),
                            "Ignoring spell cancel opcode for fixture spell slice"
                        );
                    }
                    CMSG_GOSSIP_HELLO => {
                        handle_gossip_hello(
                            &mut stream,
                            &world_db_pool,
                            &body,
                            &session,
                            &mut header_crypto,
                        )
                        .await?;
                    }
                    CMSG_GOSSIP_SELECT_OPTION => {
                        handle_gossip_select_option(
                            &mut stream,
                            &character_db_pool,
                            &world_db_pool,
                            &body,
                            &mut session,
                            &mut header_crypto,
                        )
                        .await?;
                    }
                    CMSG_QUESTGIVER_STATUS_QUERY => {
                        handle_questgiver_status_query(
                            &mut stream,
                            &world_db_pool,
                            &body,
                            &session,
                            &mut header_crypto,
                        )
                        .await?;
                    }
                    CMSG_QUESTGIVER_HELLO => {
                        handle_questgiver_hello(
                            &mut stream,
                            &world_db_pool,
                            &body,
                            &session,
                            &mut header_crypto,
                        )
                        .await?;
                    }
                    CMSG_QUESTGIVER_QUERY_QUEST => {
                        handle_questgiver_query_quest(
                            &mut stream,
                            &world_db_pool,
                            &body,
                            &mut header_crypto,
                        )
                        .await?;
                    }
                    CMSG_QUESTGIVER_ACCEPT_QUEST => {
                        handle_questgiver_accept_quest(
                            &mut stream,
                            &character_db_pool,
                            &world_db_pool,
                            &body,
                            &mut session,
                            &mut header_crypto,
                        )
                        .await?;
                    }
                    CMSG_QUESTGIVER_COMPLETE_QUEST | CMSG_QUESTGIVER_REQUEST_REWARD => {
                        handle_questgiver_complete_quest(
                            &mut stream,
                            &world_db_pool,
                            &body,
                            &session,
                            &mut header_crypto,
                        )
                        .await?;
                    }
                    CMSG_QUESTGIVER_CHOOSE_REWARD => {
                        handle_questgiver_choose_reward(
                            &mut stream,
                            &character_db_pool,
                            &world_db_pool,
                            &body,
                            &mut session,
                            &mut header_crypto,
                        )
                        .await?;
                    }
                    CMSG_NPC_TEXT_QUERY => {
                        handle_npc_text_query(&mut stream, &body, &mut header_crypto).await?;
                    }
                    CMSG_LIST_INVENTORY => {
                        handle_list_inventory(
                            &mut stream,
                            &world_db_pool,
                            &body,
                            &mut header_crypto,
                        )
                        .await?;
                    }
                    CMSG_SELL_ITEM => {
                        handle_sell_item(
                            &mut stream,
                            &character_db_pool,
                            &world_db_pool,
                            &body,
                            &mut session,
                            &mut header_crypto,
                        )
                        .await?;
                    }
                    CMSG_BUY_ITEM => {
                        handle_buy_item(
                            &mut stream,
                            &character_db_pool,
                            &world_db_pool,
                            &body,
                            &mut session,
                            &mut header_crypto,
                        )
                        .await?;
                    }
                    CMSG_TRAINER_LIST => {
                        handle_trainer_list(
                            &mut stream,
                            &character_db_pool,
                            &world_db_pool,
                            &body,
                            &mut session,
                            &mut header_crypto,
                        )
                        .await?;
                    }
                    CMSG_TRAINER_BUY_SPELL => {
                        handle_trainer_buy_spell(
                            &mut stream,
                            &character_db_pool,
                            &world_db_pool,
                            &body,
                            &mut session,
                            &mut header_crypto,
                        )
                        .await?;
                    }
                    CMSG_ATTACKSWING => {
                        handle_attack_swing(
                            &mut stream,
                            &character_db_pool,
                            &world_db_pool,
                            &body,
                            &mut session,
                            &mut header_crypto,
                        )
                        .await?;
                    }
                    CMSG_ATTACKSTOP => {
                        handle_attack_stop(&mut stream, &mut session, &mut header_crypto).await?;
                    }
                    CMSG_LOOT => {
                        handle_loot(
                            &mut stream,
                            &world_db_pool,
                            &body,
                            &mut session,
                            &mut header_crypto,
                        )
                        .await?;
                    }
                    CMSG_AUTOSTORE_LOOT_ITEM => {
                        handle_autostore_loot_item(
                            &mut stream,
                            &character_db_pool,
                            &world_db_pool,
                            &body,
                            &mut session,
                            &mut header_crypto,
                        )
                        .await?;
                    }
                    CMSG_LOOT_MONEY => {
                        handle_loot_money(
                            &mut stream,
                            &character_db_pool,
                            &mut session,
                            &mut header_crypto,
                        )
                        .await?;
                    }
                    CMSG_LOOT_RELEASE => {
                        handle_loot_release(&mut stream, &body, &mut session, &mut header_crypto)
                            .await?;
                    }
                    CMSG_GMTICKET_GETTICKET => {
                        handle_gmticket_getticket(&mut stream, &mut header_crypto).await?;
                    }
                    CMSG_SET_ACTIVE_MOVER => {
                        handle_set_active_mover(&body, &session)?;
                    }
                    MSG_QUERY_NEXT_MAIL_TIME => {
                        handle_query_next_mail_time(
                            &mut stream,
                            &character_db_pool,
                            &session,
                            &mut header_crypto,
                        )
                        .await?;
                    }
                    CMSG_LOGOUT_REQUEST => {
                        handle_logout_request(
                            &mut stream,
                            &character_db_pool,
                            account.id,
                            &mut header_crypto,
                            &mut session,
                            &runtime_state.online_characters,
                        )
                        .await?;
                    }
                    CMSG_LOGOUT_CANCEL => {
                        handle_logout_cancel(&mut stream, &mut header_crypto).await?;
                    }
                    CMSG_PLAYER_LOGOUT => {
                        info!("Received client-side player logout notification");
                    }
                    _ if is_movement_opcode(opcode) => {
                        handle_movement(opcode, &body, &mut session)?;
                    }
                    _ if is_expected_noop_opcode(opcode) => {
                        info!(
                            opcode = expected_noop_opcode_name(opcode),
                            bytes = body.len(),
                            "Ignoring expected world bootstrap opcode"
                        );
                    }
                    _ => {
                        warn!(
                            opcode = format_args!("0x{opcode:04X}"),
                            "Unhandled authenticated world opcode"
                        );
                    }
                }
            }
            Ok(Err(e)) => {
                persist_active_character_position(&character_db_pool, account.id, &session).await?;
                unregister_active_character(&runtime_state.online_characters, &mut session).await;
                info!("World client disconnected or read failed: {}", e);
                return Ok(());
            }
            Err(_) => {
                handle_combat_tick(
                    &mut stream,
                    &character_db_pool,
                    &world_db_pool,
                    &mut session,
                    &mut header_crypto,
                )
                .await?;
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
struct CharCreatePacket {
    name: String,
    race: u8,
    class: u8,
    gender: u8,
    skin: u8,
    face: u8,
    hair_style: u8,
    hair_color: u8,
    facial_hair: u8,
    outfit_id: u8,
}

impl CharCreatePacket {
    fn read(body: &[u8]) -> anyhow::Result<Self> {
        let name_end = body
            .iter()
            .position(|b| *b == 0)
            .ok_or_else(|| anyhow::anyhow!("CMSG_CHAR_CREATE name is not NUL-terminated"))?;
        let name = String::from_utf8(body[..name_end].to_vec())?;
        let cursor = name_end + 1;
        ensure_available(body, cursor + 9)?;

        Ok(Self {
            name,
            race: body[cursor],
            class: body[cursor + 1],
            gender: body[cursor + 2],
            skin: body[cursor + 3],
            face: body[cursor + 4],
            hair_style: body[cursor + 5],
            hair_color: body[cursor + 6],
            facial_hair: body[cursor + 7],
            outfit_id: body[cursor + 8],
        })
    }
}

fn normalize_character_name(name: &str) -> Result<String, u8> {
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

fn is_valid_race_class(race: u8, class: u8) -> bool {
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

async fn send_auth_response(stream: &mut TcpStream, response: u8) -> anyhow::Result<()> {
    send_packet(stream, SMSG_AUTH_RESPONSE, &[response], None).await
}

async fn send_auth_ok(
    stream: &mut TcpStream,
    header_crypto: Option<&mut HeaderCrypto>,
) -> anyhow::Result<()> {
    let mut body = Vec::with_capacity(11);
    body.push(AUTH_OK);
    body.extend_from_slice(&0u32.to_le_bytes()); // BillingTimeRemaining
    body.push(0); // BillingPlanFlags
    body.extend_from_slice(&0u32.to_le_bytes()); // BillingTimeRested
    body.push(0); // expansion
    send_packet(stream, SMSG_AUTH_RESPONSE, &body, header_crypto).await
}

async fn send_char_enum(
    stream: &mut TcpStream,
    characters: &[CharacterEnumEntry],
    header_crypto: Option<&mut HeaderCrypto>,
) -> anyhow::Result<()> {
    let body = build_char_enum_body(characters)?;
    send_packet(stream, SMSG_CHAR_ENUM, &body, header_crypto).await
}

async fn handle_char_delete(
    stream: &mut TcpStream,
    login_db_pool: &MySqlPool,
    character_db_pool: &MySqlPool,
    account_id: u32,
    body: &[u8],
    header_crypto: &mut HeaderCrypto,
    runtime_state: &WorldRuntimeState,
) -> anyhow::Result<()> {
    if body.len() != 8 {
        warn!("Rejected malformed CMSG_CHAR_DELETE bytes={}", body.len());
        return send_char_delete_result(stream, CHAR_DELETE_FAILED, Some(header_crypto)).await;
    }

    let raw_guid = u64::from_le_bytes(body.try_into()?);
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

async fn send_char_delete_result(
    stream: &mut TcpStream,
    result: u8,
    header_crypto: Option<&mut HeaderCrypto>,
) -> anyhow::Result<()> {
    send_packet(stream, SMSG_CHAR_DELETE, &[result], header_crypto).await
}

async fn handle_char_create(
    stream: &mut TcpStream,
    login_db_pool: &MySqlPool,
    character_db_pool: &MySqlPool,
    world_db_pool: &MySqlPool,
    account_id: u32,
    body: &[u8],
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    let create = match CharCreatePacket::read(body) {
        Ok(create) => create,
        Err(e) => {
            warn!("Rejected malformed CMSG_CHAR_CREATE: {}", e);
            send_char_create_result(stream, CHAR_CREATE_FAILED, Some(header_crypto)).await?;
            return Ok(());
        }
    };

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

async fn send_char_create_result(
    stream: &mut TcpStream,
    result: u8,
    header_crypto: Option<&mut HeaderCrypto>,
) -> anyhow::Result<()> {
    send_packet(stream, SMSG_CHAR_CREATE, &[result], header_crypto).await
}

async fn handle_player_login(
    stream: &mut TcpStream,
    deps: PlayerLoginDeps<'_>,
    account_id: u32,
    body: &[u8],
    header_crypto: &mut HeaderCrypto,
    session: &mut WorldSessionState,
) -> anyhow::Result<()> {
    if body.len() != 8 {
        anyhow::bail!(
            "CMSG_PLAYER_LOGIN payload must be 8 bytes, got {}",
            body.len()
        );
    }

    let guid_raw = u64::from_le_bytes(body.try_into()?);
    let guid = ObjectGuid::from_raw(guid_raw);
    let character_guid = guid.counter();
    let characters = wow_db::get_character_enum_entries(deps.character_db_pool, account_id).await?;
    let Some(character) = characters
        .iter()
        .find(|character| character.guid == character_guid)
    else {
        warn!(
            account_id,
            guid = format_args!("0x{guid_raw:016X}"),
            "Character login rejected: character not found for account"
        );
        send_packet(
            stream,
            SMSG_CHARACTER_LOGIN_FAILED,
            &[CHAR_LOGIN_NO_CHARACTER],
            Some(header_crypto),
        )
        .await?;
        return Ok(());
    };

    if deps
        .online_characters
        .lock()
        .await
        .contains(&character.guid)
    {
        warn!(
            account_id,
            guid = character.guid,
            "Character login rejected: character already loaded"
        );
        send_packet(
            stream,
            SMSG_CHARACTER_LOGIN_FAILED,
            &[CHAR_LOGIN_NO_CHARACTER],
            Some(header_crypto),
        )
        .await?;
        return Ok(());
    }

    info!(
        account_id,
        guid = character.guid,
        name = %character.name,
        map = character.map,
        "Character login selected"
    );
    unregister_active_character(deps.online_characters, session).await;
    deps.online_characters.lock().await.insert(character.guid);
    session.active_character = Some(ActiveCharacter {
        guid: character.guid,
        name: character.name.clone(),
        race: character.race,
        class: character.class,
        level: character.level,
        xp: character.xp,
        position: WorldPosition::new(
            character.map,
            character.position_x,
            character.position_y,
            character.position_z,
            character.orientation,
        ),
        movement_flags: 0,
        client_time: 0,
        fall_time: 0,
    });
    session.combat_dummy_health = RUST_COMBAT_DUMMY_HEALTH;
    session.combat_dummy_lootable = false;
    session.combat_dummy_looting = false;
    session.combat_dummy_loot_money_available = false;
    session.combat_dummy_loot_item_available = false;
    let nearby_creatures = wow_db::get_nearby_creature_spawns(
        deps.world_db_pool,
        character.map,
        character.position_x,
        character.position_y,
        CREATURE_SPAWN_RADIUS_YARDS,
        CREATURE_SPAWN_LIMIT,
    )
    .await?;
    session.db_creatures = nearby_creatures
        .into_iter()
        .map(DbCreatureRuntime::new)
        .map(|creature| (creature.guid().raw(), creature))
        .collect();
    session.player_health = character.health;
    session.player_rage = character.power2.min(POWER_RAGE_DEFAULT);
    session.player_mana = character.power1;
    session.inventory =
        wow_db::get_character_inventory_items(deps.character_db_pool, character.guid).await?;
    session.quest_statuses =
        wow_db::get_character_quest_statuses(deps.character_db_pool, character.guid)
            .await?
            .into_iter()
            .map(|status| (status.quest, status))
            .collect();
    let world_stats = wow_db::get_player_world_stats(
        deps.world_db_pool,
        character.race,
        character.class,
        character.level,
    )
    .await?;
    if session.player_mana == 0 {
        session.player_mana = world_stats.max_mana();
    }
    if session.player_health == 0 {
        session.player_health = world_stats.max_health().max(1);
    }
    let spells = wow_db::get_character_spells(deps.character_db_pool, character.guid).await?;
    session.active_spells = spells
        .iter()
        .filter(|spell| spell.active != 0 && spell.disabled == 0)
        .map(|spell| spell.spell)
        .collect();
    let tutorial_flags = wow_db::get_tutorial_flags(deps.character_db_pool, account_id).await?;
    let cinematic_sequence = if character.cinematic == 0 {
        cinematic_sequence_for_race(character.race)
    } else {
        None
    };
    if character.cinematic == 0 || character.at_login & AT_LOGIN_FIRST != 0 {
        let rows = wow_db::mark_character_first_login_seen(
            deps.character_db_pool,
            account_id,
            character.guid,
        )
        .await?;
        if rows == 0 {
            warn!(
                account_id,
                guid = character.guid,
                "No character row updated while marking first-login state seen"
            );
        }
    }

    send_enter_world_bootstrap(
        stream,
        EnterWorldBootstrap {
            character_db_pool: deps.character_db_pool,
            world_db_pool: deps.world_db_pool,
            character,
            inventory: &session.inventory,
            world_stats: &world_stats,
            spells: &spells,
            quest_statuses: &session.quest_statuses,
            tutorial_flags: &tutorial_flags,
            cinematic_sequence,
        },
        Some(header_crypto),
    )
    .await?;

    Ok(())
}

struct PlayerLoginDeps<'a> {
    character_db_pool: &'a MySqlPool,
    world_db_pool: &'a MySqlPool,
    online_characters: &'a OnlineCharacters,
}

async fn handle_logout_request(
    stream: &mut TcpStream,
    character_db_pool: &MySqlPool,
    account_id: u32,
    header_crypto: &mut HeaderCrypto,
    session: &mut WorldSessionState,
    online_characters: &OnlineCharacters,
) -> anyhow::Result<()> {
    if let Some(character) = &session.active_character {
        info!(
            guid = character.guid,
            name = %character.name,
            x = character.position.x,
            y = character.position.y,
            z = character.position.z,
            o = character.position.orientation,
            "Completing instant logout to character selection"
        );
    } else {
        info!("Completing logout request before character login");
    }

    let mut body = Vec::with_capacity(5);
    body.extend_from_slice(&0u32.to_le_bytes()); // no logout failure reason
    body.push(1); // instant logout, matching rested/GM-style response shape
    send_packet(
        stream,
        SMSG_LOGOUT_RESPONSE,
        &body,
        Some(&mut *header_crypto),
    )
    .await?;
    send_packet(stream, SMSG_LOGOUT_COMPLETE, &[], Some(header_crypto)).await?;
    persist_active_character_position(character_db_pool, account_id, session).await?;
    unregister_active_character(online_characters, session).await;
    Ok(())
}

async fn unregister_active_character(
    online_characters: &OnlineCharacters,
    session: &mut WorldSessionState,
) {
    if let Some(character) = session.active_character.take() {
        online_characters.lock().await.remove(&character.guid);
    }
    session.active_spells.clear();
}

async fn persist_active_character_position(
    character_db_pool: &MySqlPool,
    account_id: u32,
    session: &WorldSessionState,
) -> anyhow::Result<()> {
    let Some(character) = &session.active_character else {
        return Ok(());
    };

    let rows = wow_db::update_character_position(
        character_db_pool,
        account_id,
        character.guid,
        character.position,
    )
    .await?;

    if rows == 0 {
        warn!(
            account_id,
            guid = character.guid,
            "No character row updated while persisting position"
        );
    } else {
        info!(
            account_id,
            guid = character.guid,
            name = %character.name,
            x = character.position.x,
            y = character.position.y,
            z = character.position.z,
            o = character.position.orientation,
            "Persisted character position"
        );
    }

    Ok(())
}

async fn handle_logout_cancel(
    stream: &mut TcpStream,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    send_packet(stream, SMSG_LOGOUT_CANCEL_ACK, &[], Some(header_crypto)).await
}

fn handle_movement(
    opcode: u32,
    body: &[u8],
    session: &mut WorldSessionState,
) -> anyhow::Result<()> {
    let movement = MovementInfo::read(body)?;
    if let Some(character) = &mut session.active_character {
        character.position.x = movement.position.x;
        character.position.y = movement.position.y;
        character.position.z = movement.position.z;
        character.position.orientation = movement.position.orientation;
        character.movement_flags = movement.flags;
        character.client_time = movement.client_time;
        character.fall_time = movement.fall_time;
        info!(
            opcode = movement_opcode_name(opcode),
            guid = character.guid,
            name = %character.name,
            flags = format_args!("0x{:08X}", movement.flags),
            client_time = movement.client_time,
            x = movement.position.x,
            y = movement.position.y,
            z = movement.position.z,
            o = movement.position.orientation,
            "Updated in-memory character movement"
        );
    } else {
        warn!(
            opcode = movement_opcode_name(opcode),
            "Received movement packet before character login"
        );
    }
    Ok(())
}

include!("bootstrap.rs");
include!("interactions.rs");
include!("wire.rs");
#[cfg(test)]
mod tests;
