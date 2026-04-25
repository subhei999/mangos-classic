use anyhow::{ensure, Context};
use bytes::BytesMut;
use sha1::{Digest, Sha1};
use sqlx::mysql::{MySqlPool, MySqlPoolOptions};
use std::io::{ErrorKind, Read, Write};
use std::net::TcpStream;
use std::thread;
use std::time::Duration;
use wow_common::guid::{HighGuid, ObjectGuid, PackedGuid};
use wow_crypto::HeaderCrypto;
use wow_proto::{
    AuthCommand, LogonChallengeRequest, LogonChallengeResponse, LogonProofRequest,
    LogonProofResponse,
};
use wow_srp::client::SrpClientChallenge;
use wow_srp::normalized_string::NormalizedString;
use wow_srp::server::SrpVerifier;
use wow_srp::PublicKey;

const LOGIN_DATABASE_URL: &str = "mysql://mangos:mangos@127.0.0.1:3307/realmd";
const CHARACTER_DATABASE_URL: &str = "mysql://mangos:mangos@127.0.0.1:3307/characters";
const WORLD_DATABASE_URL: &str = "mysql://mangos:mangos@127.0.0.1:3307/mangos";
const AUTH_ADDR: &str = "127.0.0.1:13724";
const WORLD_ADDR: &str = "127.0.0.1:18085";
const USERNAME: &str = "WORLDLIFE";
const OTHER_USERNAME: &str = "WORLDOTHER";
const PASSWORD: &str = "WORLDPASS";
const CHARACTER_NAME: &str = "Worldlife";
const OTHER_CHARACTER_NAME: &str = "Worldother";
const BUILD_1121: u16 = 5875;
const CLIENT_SEED: u32 = 0x1234_5678;
const REALM_ID: u32 = 1;

const CMSG_CHAR_CREATE: u32 = 0x0036;
const CMSG_CHAR_ENUM: u32 = 0x0037;
const CMSG_CHAR_DELETE: u32 = 0x0038;
const CMSG_PLAYER_LOGIN: u32 = 0x003D;
const CMSG_CREATURE_QUERY: u32 = 0x0060;
const CMSG_LOGOUT_REQUEST: u32 = 0x004B;
const CMSG_SWAP_INV_ITEM: u32 = 0x010D;
const CMSG_SWAP_ITEM: u32 = 0x010C;
const CMSG_SPLIT_ITEM: u32 = 0x010E;
const CMSG_DESTROYITEM: u32 = 0x0111;
const CMSG_CAST_SPELL: u32 = 0x012E;
const CMSG_GOSSIP_HELLO: u32 = 0x017B;
const CMSG_GOSSIP_SELECT_OPTION: u32 = 0x017C;
const CMSG_LIST_INVENTORY: u32 = 0x019E;
const CMSG_SELL_ITEM: u32 = 0x01A0;
const CMSG_BUY_ITEM: u32 = 0x01A2;
const CMSG_AUTOSTORE_LOOT_ITEM: u32 = 0x0108;
const CMSG_ATTACKSWING: u32 = 0x0141;
const CMSG_LOOT: u32 = 0x015D;
const CMSG_LOOT_RELEASE: u32 = 0x015F;
const CMSG_AUTH_SESSION: u32 = 0x01ED;
const SMSG_CHAR_CREATE: u32 = 0x003A;
const SMSG_CHAR_ENUM: u32 = 0x003B;
const SMSG_CHAR_DELETE: u32 = 0x003C;
const SMSG_CREATURE_QUERY_RESPONSE: u32 = 0x0061;
const SMSG_UPDATE_OBJECT: u32 = 0x00A9;
const SMSG_DESTROY_OBJECT: u32 = 0x00AA;
const SMSG_INVENTORY_CHANGE_FAILURE: u32 = 0x0112;
const SMSG_CAST_RESULT: u32 = 0x0130;
const SMSG_SPELL_GO: u32 = 0x0132;
const SMSG_GOSSIP_MESSAGE: u32 = 0x017D;
const SMSG_NPC_TEXT_UPDATE: u32 = 0x0180;
const SMSG_LIST_INVENTORY: u32 = 0x019F;
const SMSG_BUY_ITEM: u32 = 0x01A4;
const SMSG_BUY_FAILED: u32 = 0x01A5;
const SMSG_ATTACKSTART: u32 = 0x0143;
const SMSG_ATTACKSTOP: u32 = 0x0144;
const SMSG_ATTACKERSTATEUPDATE: u32 = 0x014A;
const SMSG_LOOT_RESPONSE: u32 = 0x0160;
const SMSG_LOOT_RELEASE_RESPONSE: u32 = 0x0161;
const SMSG_LOOT_REMOVED: u32 = 0x0162;
const SMSG_AUTH_CHALLENGE: u32 = 0x01EC;
const SMSG_AUTH_RESPONSE: u32 = 0x01EE;
const AUTH_OK: u8 = 0x0C;
const CHAR_CREATE_SUCCESS: u8 = 0x2E;
const CHAR_CREATE_FAILED: u8 = 0x30;
const CHAR_CREATE_NAME_IN_USE: u8 = 0x31;
const CHAR_CREATE_SERVER_LIMIT: u8 = 0x34;
const CHAR_DELETE_SUCCESS: u8 = 0x39;
const CHAR_DELETE_FAILED: u8 = 0x3A;
const CHAR_NAME_TOO_SHORT: u8 = 0x44;
const CHAR_NAME_INVALID_CHARACTER: u8 = 0x46;
const AT_LOGIN_FIRST: u32 = 0x20;
const ITEM_FLAG_NO_USER_DESTROY: u32 = 0x0000_0020;
const EQUIP_ERR_CANT_DROP_SOULBOUND: u8 = 24;
const EQUIP_ERR_COULDNT_SPLIT_ITEMS: u8 = 27;
const RUST_GUIDE_ENTRY: u32 = 900_001;
const RUST_GUIDE_COUNTER: u32 = 1;
const RUST_VENDOR_BAG_ITEM: u32 = 2102;
const RUST_VENDOR_STACK_ITEM: u32 = 117;
const WARRIOR_HEROIC_STRIKE_RANK_1: u32 = 78;
const SPELL_CAST_TARGET_UNIT: u16 = 0x0002;
const DB_CREATURE_FIXTURE_GUID: u32 = 96_001;
const DB_CREATURE_FIXTURE_ENTRY: u32 = 1;
const DB_CREATURE_FIXTURE_NAME: &str = "Waypoint (Only GM can see it)";
const EMPTY_DB_VENDOR_FIXTURE_GUID: u32 = 96_002;
const EMPTY_DB_VENDOR_FIXTURE_ENTRY: u32 = 900_011;
const RUST_COMBAT_DUMMY_ENTRY: u32 = 900_002;
const RUST_COMBAT_DUMMY_COUNTER: u32 = 2;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let login_pool = connect(LOGIN_DATABASE_URL).await?;
    let character_pool = connect(CHARACTER_DATABASE_URL).await?;
    let world_pool = connect(WORLD_DATABASE_URL).await?;
    audit_starter_item_templates_exist(&world_pool).await?;

    let account_id = seed_account(&login_pool, USERNAME, PASSWORD).await?;
    let other_account_id = seed_account(&login_pool, OTHER_USERNAME, PASSWORD).await?;
    cleanup_account(&login_pool, &character_pool, account_id).await?;
    cleanup_account(&login_pool, &character_pool, other_account_id).await?;

    complete_auth_flow()?;
    let session_key = fetch_session_key(&login_pool).await?;

    let mut world = WorldClient::connect(&session_key)?;
    let initial = world.char_enum()?;
    ensure!(
        !initial
            .iter()
            .any(|character| character.name == CHARACTER_NAME),
        "test character was visible before create"
    );

    world.expect_create_result("A", human_warrior_attributes(), CHAR_NAME_TOO_SHORT)?;
    assert_count_row(&login_pool, account_id, 0).await?;
    world.expect_create_result(
        "Bad1",
        human_warrior_attributes(),
        CHAR_NAME_INVALID_CHARACTER,
    )?;
    assert_count_row(&login_pool, account_id, 0).await?;
    world.expect_create_result("Badcombo", [1, 7, 0, 0, 0, 0, 0, 0, 0], CHAR_CREATE_FAILED)?;
    assert_count_row(&login_pool, account_id, 0).await?;
    world.expect_delete_body_result(&[1, 2, 3], CHAR_DELETE_FAILED)?;
    assert_count_row(&login_pool, account_id, 0).await?;

    world.expect_create_result(
        CHARACTER_NAME,
        human_warrior_attributes(),
        CHAR_CREATE_SUCCESS,
    )?;
    let after_create = world.char_enum()?;
    let created = after_create
        .iter()
        .find(|character| character.name == CHARACTER_NAME)
        .context("created character was missing from SMSG_CHAR_ENUM")?;
    let created_db = wow_db::get_character_enum_entries(&character_pool, account_id)
        .await?
        .into_iter()
        .find(|character| character.guid == created.guid)
        .context("created character was missing from DB enum rows")?;
    let expected_stats = wow_db::get_player_world_stats(
        &world_pool,
        created_db.race,
        created_db.class,
        created_db.level,
    )
    .await?;
    ensure!(
        created_db.health == expected_stats.max_health(),
        "packet-created character health did not match derived player_classlevelstats/player_levelstats health"
    );
    ensure!(
        created_db.power1 == expected_stats.max_mana(),
        "packet-created character mana did not match derived player_classlevelstats/player_levelstats mana"
    );
    ensure!(
        wow_db::character_count_for_account(&character_pool, account_id).await? == 1,
        "character DB count did not refresh after packet create"
    );
    assert_count_row(&login_pool, account_id, 1).await?;
    ensure!(
        wow_db::get_character_inventory_items(&character_pool, created.guid)
            .await?
            .iter()
            .any(|item| item.item_template == 6948),
        "packet-created character did not receive starter inventory"
    );
    assert_starter_main_hand_state(&character_pool, &world_pool, created.guid).await?;
    assert_heroic_strike_known(&character_pool, created.guid).await?;
    seed_inventory_edge_fixture(&character_pool, created.guid).await?;
    seed_db_creature_fixture(&world_pool, &created_db).await?;

    world.expect_create_result(
        CHARACTER_NAME,
        human_warrior_attributes(),
        CHAR_CREATE_NAME_IN_USE,
    )?;
    assert_count_row(&login_pool, account_id, 1).await?;

    let other_character = wow_db::create_character(
        &character_pool,
        &world_pool,
        wow_db::NewCharacter {
            account_id: other_account_id,
            name: OTHER_CHARACTER_NAME.to_string(),
            race: 1,
            class: 1,
            gender: 0,
            skin: 0,
            face: 0,
            hair_style: 0,
            hair_color: 0,
            facial_hair: 0,
        },
    )
    .await?;
    wow_db::refresh_realm_character_count(&login_pool, &character_pool, other_account_id, REALM_ID)
        .await?;
    world.expect_delete_character_result(other_character.guid, CHAR_DELETE_FAILED)?;
    ensure!(
        wow_db::get_character_enum_entries(&character_pool, other_account_id)
            .await?
            .iter()
            .any(|character| character.guid == other_character.guid),
        "other account character was deleted by packet delete"
    );
    assert_count_row(&login_pool, account_id, 1).await?;

    let no_space_character = wow_db::create_character(
        &character_pool,
        &world_pool,
        wow_db::NewCharacter {
            account_id,
            name: "Lootfull".to_string(),
            race: 1,
            class: 1,
            gender: 0,
            skin: 0,
            face: 0,
            hair_style: 0,
            hair_color: 0,
            facial_hair: 0,
        },
    )
    .await?;
    seed_full_backpack_fixture(&character_pool, no_space_character.guid).await?;
    let no_space_stack_rows_before = inventory_item_row_count(
        &character_pool,
        no_space_character.guid,
        RUST_VENDOR_STACK_ITEM,
    )
    .await?;
    let mut no_space_world = WorldClient::connect(&session_key)?;
    no_space_world.login_character(no_space_character.guid)?;
    no_space_world.kill_combat_dummy_with_autoattacks()?;
    no_space_world.open_combat_dummy_loot()?;
    no_space_world.autostore_combat_dummy_loot_item_expect_no_space()?;
    assert_inventory_item_row_count(
        &character_pool,
        no_space_character.guid,
        RUST_VENDOR_STACK_ITEM,
        no_space_stack_rows_before,
    )
    .await?;
    no_space_world.release_combat_dummy_loot()?;
    drop(no_space_world);
    ensure!(
        wow_db::delete_character(&character_pool, account_id, no_space_character.guid).await?,
        "failed to delete temporary no-space loot character"
    );
    wow_db::refresh_realm_character_count(&login_pool, &character_pool, account_id, REALM_ID)
        .await?;
    assert_count_row(&login_pool, account_id, 1).await?;

    let mut loaded_world = WorldClient::connect(&session_key)?;
    loaded_world.login_character(created.guid)?;
    assert_first_login_state_seen(&character_pool, account_id, created.guid).await?;
    loaded_world.query_db_creature_template()?;
    loaded_world.reject_invalid_db_vendor_gossip_option()?;
    loaded_world.open_db_vendor_gossip_inventory()?;
    loaded_world.list_db_vendor_inventory()?;
    loaded_world.list_empty_db_vendor_inventory()?;
    let (db_vendor_price, db_vendor_sell_price, db_vendor_buy_count) =
        item_prices_and_buy_count(&world_pool, RUST_VENDOR_STACK_ITEM).await?;
    let db_vendor_stack_count_before =
        inventory_item_row_count(&character_pool, created.guid, RUST_VENDOR_STACK_ITEM).await?;
    loaded_world.buy_db_vendor_item_expect_insufficient_money(RUST_VENDOR_STACK_ITEM, 1)?;
    assert_inventory_item_row_count(
        &character_pool,
        created.guid,
        RUST_VENDOR_STACK_ITEM,
        db_vendor_stack_count_before,
    )
    .await?;
    set_character_money(&character_pool, created.guid, db_vendor_price).await?;
    loaded_world.buy_db_vendor_item(RUST_VENDOR_STACK_ITEM, 1)?;
    assert_character_money_eq(&character_pool, created.guid, 0).await?;
    assert_inventory_item_row_count(
        &character_pool,
        created.guid,
        RUST_VENDOR_STACK_ITEM,
        db_vendor_stack_count_before + 1,
    )
    .await?;
    let db_bought_stack_guid = inventory_item_guid_with_count(
        &character_pool,
        created.guid,
        RUST_VENDOR_STACK_ITEM,
        db_vendor_buy_count,
    )
    .await?;
    loaded_world.sell_db_vendor_item(db_bought_stack_guid, 1)?;
    assert_character_money_eq(&character_pool, created.guid, db_vendor_sell_price).await?;
    assert_inventory_item_instance_count(
        &character_pool,
        db_bought_stack_guid,
        db_vendor_buy_count - 1,
    )
    .await?;
    loaded_world.sell_db_vendor_item(db_bought_stack_guid, 0)?;
    assert_character_money_eq(
        &character_pool,
        created.guid,
        db_vendor_sell_price.saturating_mul(db_vendor_buy_count),
    )
    .await?;
    assert_inventory_guid_absent(&character_pool, db_bought_stack_guid).await?;
    loaded_world.swap_inventory_slots(24, 26)?;
    assert_inventory_slot(&character_pool, created.guid, 6948, 26).await?;
    loaded_world.swap_inventory_slots(26, 24)?;
    assert_inventory_slot(&character_pool, created.guid, 6948, 24).await?;
    loaded_world.swap_inventory_slots(3, 26)?;
    assert_inventory_slot(&character_pool, created.guid, 38, 26).await?;
    assert_equipment_cache_slot(&character_pool, created.guid, 3, 0).await?;
    loaded_world.swap_inventory_slots(26, 3)?;
    assert_inventory_slot(&character_pool, created.guid, 38, 3).await?;
    assert_equipment_cache_slot(&character_pool, created.guid, 3, 38).await?;
    loaded_world.list_rust_guide_inventory()?;
    loaded_world.buy_rust_guide_item(RUST_VENDOR_BAG_ITEM, 1)?;
    assert_inventory_item_present(&character_pool, created.guid, RUST_VENDOR_BAG_ITEM).await?;
    loaded_world.buy_rust_guide_item(RUST_VENDOR_STACK_ITEM, 2)?;
    assert_inventory_item_present(&character_pool, created.guid, RUST_VENDOR_STACK_ITEM).await?;
    let bought_stack_guid = inventory_item_guid_at(&character_pool, created.guid, 0, 26).await?;
    loaded_world.sell_item(bought_stack_guid, 1)?;
    assert_character_money_at_least(&character_pool, created.guid, 1).await?;
    loaded_world.buy_rust_guide_item(RUST_VENDOR_STACK_ITEM, 2)?;
    assert_inventory_item_present(&character_pool, created.guid, RUST_VENDOR_STACK_ITEM).await?;
    loaded_world.swap_item(255, 27, 19, 3)?;
    assert_inventory_count_at(&character_pool, created.guid, 19, 3, 3).await?;
    assert_inventory_position_empty(&character_pool, created.guid, 0, 27).await?;
    loaded_world.swap_item(19, 3, 19, 4)?;
    assert_inventory_count_at(&character_pool, created.guid, 19, 4, 3).await?;
    assert_inventory_position_empty(&character_pool, created.guid, 19, 3).await?;
    loaded_world.swap_item(19, 4, 19, 2)?;
    assert_inventory_count_at(&character_pool, created.guid, 19, 2, 7).await?;
    assert_inventory_position_empty(&character_pool, created.guid, 19, 4).await?;
    let loot_stack_rows_before =
        inventory_item_row_count(&character_pool, created.guid, RUST_VENDOR_STACK_ITEM).await?;
    let loot_stack_count_before =
        inventory_item_total_count(&character_pool, created.guid, RUST_VENDOR_STACK_ITEM).await?;
    loaded_world.kill_combat_dummy_with_autoattacks()?;
    loaded_world.open_combat_dummy_loot()?;
    loaded_world.autostore_combat_dummy_loot_item()?;
    assert_inventory_item_total_count(
        &character_pool,
        created.guid,
        RUST_VENDOR_STACK_ITEM,
        loot_stack_count_before + 2,
    )
    .await?;
    assert_inventory_item_row_count(
        &character_pool,
        created.guid,
        RUST_VENDOR_STACK_ITEM,
        loot_stack_rows_before,
    )
    .await?;
    loaded_world.release_combat_dummy_loot()?;
    for item_guid in
        inventory_item_guids_for_template(&character_pool, created.guid, RUST_VENDOR_STACK_ITEM)
            .await?
    {
        loaded_world.sell_item(item_guid, 0)?;
    }
    assert_inventory_item_row_count(&character_pool, created.guid, RUST_VENDOR_STACK_ITEM, 0)
        .await?;
    let full_stack_rows_before =
        inventory_item_row_count(&character_pool, created.guid, RUST_VENDOR_STACK_ITEM).await?;
    loaded_world.kill_combat_dummy_with_autoattacks()?;
    loaded_world.open_combat_dummy_loot()?;
    loaded_world.autostore_combat_dummy_loot_item()?;
    assert_inventory_item_row_count(
        &character_pool,
        created.guid,
        RUST_VENDOR_STACK_ITEM,
        full_stack_rows_before + 1,
    )
    .await?;
    loaded_world.release_combat_dummy_loot()?;
    loaded_world.cast_heroic_strike_at_combat_dummy()?;
    loaded_world.destroy_item(255, 24, 1, SMSG_UPDATE_OBJECT)?;
    assert_inventory_count_at(&character_pool, created.guid, 0, 24, 2).await?;
    loaded_world.split_item(255, 24, 19, 1, 1)?;
    assert_inventory_count_at(&character_pool, created.guid, 0, 24, 1).await?;
    assert_inventory_count_at(&character_pool, created.guid, 19, 1, 1).await?;
    loaded_world.destroy_item(19, 1, 0, SMSG_DESTROY_OBJECT)?;
    assert_inventory_position_empty(&character_pool, created.guid, 19, 1).await?;
    loaded_world.destroy_item(19, 0, 0, SMSG_DESTROY_OBJECT)?;
    assert_inventory_position_empty(&character_pool, created.guid, 19, 0).await?;
    loaded_world.destroy_backpack_item(24)?;
    assert_inventory_item_absent(&character_pool, created.guid, 6948).await?;
    add_item_template_flags(&world_pool, 38, ITEM_FLAG_NO_USER_DESTROY).await?;
    loaded_world.expect_destroy_failure(255, 3, EQUIP_ERR_CANT_DROP_SOULBOUND)?;
    assert_inventory_slot(&character_pool, created.guid, 38, 3).await?;
    remove_item_template_flags(&world_pool, 38, ITEM_FLAG_NO_USER_DESTROY).await?;
    loaded_world.destroy_bag0_item(3)?;
    assert_inventory_item_absent(&character_pool, created.guid, 38).await?;
    assert_equipment_cache_slot(&character_pool, created.guid, 3, 0).await?;
    let mut delete_world = WorldClient::connect(&session_key)?;
    delete_world.expect_delete_character_result(created.guid, CHAR_DELETE_FAILED)?;
    ensure!(
        wow_db::get_character_enum_entries(&character_pool, account_id)
            .await?
            .iter()
            .any(|character| character.guid == created.guid),
        "loaded character was deleted by packet delete"
    );
    loaded_world.logout()?;
    drop(loaded_world);
    drop(delete_world);
    thread::sleep(Duration::from_millis(50));

    seed_limit_characters(&character_pool, &world_pool, account_id).await?;
    wow_db::refresh_realm_character_count(&login_pool, &character_pool, account_id, REALM_ID)
        .await?;
    assert_count_row(&login_pool, account_id, 10).await?;
    world.expect_create_result(
        "Limitfull",
        human_warrior_attributes(),
        CHAR_CREATE_SERVER_LIMIT,
    )?;
    assert_count_row(&login_pool, account_id, 10).await?;

    clear_guild_fixture(&character_pool).await?;
    seed_guild_leader_fixture(&character_pool, created.guid).await?;
    world.expect_delete_character_result(created.guid, CHAR_DELETE_FAILED)?;
    ensure!(
        world
            .char_enum()?
            .iter()
            .any(|character| character.guid == created.guid),
        "guild leader disappeared after rejected delete"
    );
    ensure!(
        wow_db::character_count_for_account(&character_pool, account_id).await? == 10,
        "guild leader delete rejection changed character count"
    );
    assert_count_row(&login_pool, account_id, 10).await?;
    clear_guild_fixture(&character_pool).await?;

    seed_guild_member_fixture(&character_pool, created.guid).await?;
    seed_group_leader_fixture(&character_pool, account_id, created.guid).await?;
    seed_social_fixture(&character_pool, account_id, created.guid).await?;
    seed_pet_fixture(&character_pool, created.guid).await?;
    seed_mail_fixture(&character_pool, created.guid).await?;
    let cod_sender_guid =
        seed_cod_mail_return_fixture(&character_pool, account_id, created.guid).await?;
    seed_auction_fixture(&character_pool, created.guid).await?;
    world.expect_delete_character_result(created.guid, CHAR_DELETE_SUCCESS)?;
    let after_delete = world.char_enum()?;
    ensure!(
        !after_delete
            .iter()
            .any(|character| character.name == CHARACTER_NAME),
        "deleted character was still present in SMSG_CHAR_ENUM"
    );
    ensure!(
        wow_db::character_count_for_account(&character_pool, account_id).await? == 9,
        "character DB count did not refresh after packet delete"
    );
    assert_count_row(&login_pool, account_id, 9).await?;

    let leaked: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM characters WHERE name = ?")
        .bind(CHARACTER_NAME)
        .fetch_one(&character_pool)
        .await?;
    ensure!(leaked == 0, "deleted packet-flow character row remained");
    assert_guild_member_cleanup(&character_pool, created.guid).await?;
    assert_group_leader_cleanup(&character_pool, created.guid).await?;
    assert_social_cleanup(&character_pool, created.guid).await?;
    assert_pet_cleanup(&character_pool, 93_001).await?;
    assert_mail_cleanup(&character_pool, created.guid, 94_001, 94_101).await?;
    assert_cod_mail_return(
        &character_pool,
        created.guid,
        cod_sender_guid,
        94_002,
        94_201,
    )
    .await?;
    assert_auction_cleanup(&character_pool, created.guid, 95_001, 95_101).await?;

    cleanup_account(&login_pool, &character_pool, account_id).await?;
    cleanup_account(&login_pool, &character_pool, other_account_id).await?;

    drop(world_pool);
    println!(
        "world flow check passed: auth session, starter item template audit, create/delete happy path, negative create/delete cases, loaded/guild leader rejection, DB creature query/gossip/vendor list, DB gossip invalid-option guard, empty DB vendor marker, DB vendor BuyPrice charge, sellback, and insufficient-money guard, Heroic Strike known-spell/main-hand/target guard, loot autostore no-space guard, backpack item move persistence, equip/unequip persistence, Rust Guide vendor buys/sell, bag-contained moves, stack merge, loot autostore stack merge and empty-slot fallback, destroy guardrails, partial destroy, split, bag-contained destroy persistence, guild/group/social/pet/mail/auction cleanup, COD mail return, enum/count refresh"
    );
    Ok(())
}

async fn connect(url: &str) -> anyhow::Result<MySqlPool> {
    Ok(MySqlPoolOptions::new()
        .max_connections(2)
        .connect(url)
        .await?)
}

async fn seed_account(
    login_pool: &MySqlPool,
    username: &str,
    password: &str,
) -> anyhow::Result<u32> {
    let verifier = SrpVerifier::from_username_and_password(
        NormalizedString::new(username)?,
        NormalizedString::new(password)?,
    );

    sqlx::query(
        "INSERT INTO account (username, gmlevel, sessionkey, v, s, email, locked, expansion, locale, os) \
         VALUES (?, 0, '', ?, ?, '', 0, 0, '', 'Win') \
         ON DUPLICATE KEY UPDATE sessionkey = '', v = VALUES(v), s = VALUES(s), locked = 0, os = 'Win'",
    )
    .bind(username)
    .bind(bytes_to_hex(verifier.password_verifier()))
    .bind(bytes_to_hex(verifier.salt()))
    .execute(login_pool)
    .await
    .context("seed world-flow account")?;

    let account_id = sqlx::query_scalar("SELECT id FROM account WHERE username = ?")
        .bind(username)
        .fetch_one(login_pool)
        .await?;
    Ok(account_id)
}

async fn cleanup_account(
    login_pool: &MySqlPool,
    character_pool: &MySqlPool,
    account_id: u32,
) -> anyhow::Result<()> {
    let characters = wow_db::get_character_enum_entries(character_pool, account_id).await?;
    for character in characters {
        wow_db::delete_character(character_pool, account_id, character.guid).await?;
    }
    wow_db::refresh_realm_character_count(login_pool, character_pool, account_id, REALM_ID).await?;
    Ok(())
}

async fn seed_limit_characters(
    character_pool: &MySqlPool,
    world_pool: &MySqlPool,
    account_id: u32,
) -> anyhow::Result<()> {
    for name in [
        "Limita", "Limitb", "Limitc", "Limitd", "Limite", "Limitf", "Limitg", "Limith", "Limiti",
    ] {
        wow_db::create_character(
            character_pool,
            world_pool,
            wow_db::NewCharacter {
                account_id,
                name: name.to_string(),
                race: 1,
                class: 1,
                gender: 0,
                skin: 0,
                face: 0,
                hair_style: 0,
                hair_color: 0,
                facial_hair: 0,
            },
        )
        .await?;
    }
    Ok(())
}

async fn assert_first_login_state_seen(
    character_pool: &MySqlPool,
    account_id: u32,
    guid: u32,
) -> anyhow::Result<()> {
    let character = wow_db::get_character_enum_entries(character_pool, account_id)
        .await?
        .into_iter()
        .find(|character| character.guid == guid)
        .context("logged-in character was missing while checking first-login state")?;

    ensure!(
        character.cinematic == 1,
        "first-login character cinematic flag was not marked seen"
    );
    ensure!(
        character.at_login & AT_LOGIN_FIRST == 0,
        "first-login AT_LOGIN_FIRST flag remained set"
    );
    Ok(())
}

async fn audit_starter_item_templates_exist(world_pool: &MySqlPool) -> anyhow::Result<()> {
    let mut missing = Vec::new();
    for item_ref in wow_db::starter_item_template_refs() {
        let exists: Option<u32> =
            sqlx::query_scalar("SELECT entry FROM item_template WHERE entry = ?")
                .bind(item_ref.item_id)
                .fetch_optional(world_pool)
                .await?;
        if exists.is_none() {
            missing.push(format!(
                "race={} class={} slot={} item={} amount={}",
                item_ref.race, item_ref.class, item_ref.slot, item_ref.item_id, item_ref.amount
            ));
        }
    }

    if !missing.is_empty() {
        eprintln!(
            "starter item template audit warning: {} refs missing from world fixture:\n{}",
            missing.len(),
            missing.join("\n")
        );
    }
    Ok(())
}

async fn assert_starter_main_hand_state(
    character_pool: &MySqlPool,
    world_pool: &MySqlPool,
    guid: u32,
) -> anyhow::Result<()> {
    let Some((item_guid, item_template, item_entry, count)): Option<(u32, u32, u32, u32)> =
        sqlx::query_as(
            "SELECT ci.item, ci.item_template, ii.itemEntry, ii.count \
             FROM character_inventory ci \
             JOIN item_instance ii ON ii.guid = ci.item \
             WHERE ci.guid = ? AND ci.bag = 0 AND ci.slot = 15",
        )
        .bind(guid)
        .fetch_optional(character_pool)
        .await?
    else {
        anyhow::bail!("fresh Human Warrior missing main-hand inventory row");
    };
    ensure!(
        item_template == 25 && item_entry == 25 && count == 1,
        "main-hand row was item={item_guid} template={item_template} entry={item_entry} count={count}, expected item 25 x1"
    );
    assert_equipment_cache_slot(character_pool, guid, 15, 25).await?;

    let (class, subclass, inventory_type, allowable_class, allowable_race): (
        u32,
        u32,
        u32,
        i32,
        i32,
    ) = sqlx::query_as(
        "SELECT class, subclass, InventoryType, AllowableClass, AllowableRace \
         FROM item_template WHERE entry = 25",
    )
    .fetch_one(world_pool)
    .await?;
    ensure!(
        class == 2 && matches!(inventory_type, 13 | 17 | 21),
        "starter main-hand item 25 class/inventory type was class={class} inventory_type={inventory_type}, expected weapon/main-hand-compatible inventory type"
    );
    ensure!(
        subclass > 0,
        "starter main-hand item 25 had invalid weapon subclass 0"
    );
    ensure!(
        allowable_class == -1 || (allowable_class & (1 << 0)) != 0,
        "starter main-hand item 25 does not allow warriors: AllowableClass={allowable_class}"
    );
    ensure!(
        allowable_race == -1 || (allowable_race & (1 << 0)) != 0,
        "starter main-hand item 25 does not allow humans: AllowableRace={allowable_race}"
    );
    Ok(())
}

async fn assert_heroic_strike_known(character_pool: &MySqlPool, guid: u32) -> anyhow::Result<()> {
    let row: Option<(u32, u8, u8)> = sqlx::query_as(
        "SELECT spell, active, disabled FROM character_spell WHERE guid = ? AND spell = ?",
    )
    .bind(guid)
    .bind(WARRIOR_HEROIC_STRIKE_RANK_1)
    .fetch_optional(character_pool)
    .await?;
    let Some((spell, active, disabled)) = row else {
        anyhow::bail!(
            "fresh Human Warrior missing Heroic Strike rank 1 spell {WARRIOR_HEROIC_STRIKE_RANK_1}"
        );
    };
    ensure!(
        spell == WARRIOR_HEROIC_STRIKE_RANK_1 && active == 1 && disabled == 0,
        "Heroic Strike starter spell row was spell={spell} active={active} disabled={disabled}"
    );
    Ok(())
}

async fn seed_inventory_edge_fixture(character_pool: &MySqlPool, guid: u32) -> anyhow::Result<()> {
    sqlx::query(
        "UPDATE item_instance ii \
         JOIN character_inventory ci ON ii.guid = ci.item \
         SET ii.count = 3 \
         WHERE ci.guid = ? AND ci.bag = 0 AND ci.slot = 24 AND ci.item_template = 6948",
    )
    .bind(guid)
    .execute(character_pool)
    .await?;

    let base_guid: u32 = sqlx::query_scalar(
        "SELECT CAST(COALESCE(MAX(guid), 0) + 1 AS UNSIGNED) FROM item_instance",
    )
    .fetch_one(character_pool)
    .await?;
    for (item_guid, bag, slot, item_template, count) in [
        (base_guid, 0u32, 19u8, 4496u32, 1u32),
        (base_guid + 1, 19u32, 0u8, 6948u32, 1u32),
        (base_guid + 2, 0u32, 27u8, 117u32, 3u32),
        (base_guid + 3, 19u32, 2u8, 117u32, 4u32),
    ] {
        sqlx::query(
            "INSERT INTO item_instance \
             (guid, owner_guid, itemEntry, creatorGuid, giftCreatorGuid, count, duration, \
              charges, flags, enchantments, randomPropertyId, durability, itemTextId) \
             VALUES (?, ?, ?, 0, 0, ?, 0, '0 0 0 0 0 ', 0, \
                     '0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 ', 0, 0, 0)",
        )
        .bind(item_guid)
        .bind(guid)
        .bind(item_template)
        .bind(count)
        .execute(character_pool)
        .await?;
        sqlx::query(
            "INSERT INTO character_inventory (guid, bag, slot, item, item_template) \
             VALUES (?, ?, ?, ?, ?)",
        )
        .bind(guid)
        .bind(bag)
        .bind(slot)
        .bind(item_guid)
        .bind(item_template)
        .execute(character_pool)
        .await?;
    }

    Ok(())
}

async fn seed_full_backpack_fixture(character_pool: &MySqlPool, guid: u32) -> anyhow::Result<()> {
    sqlx::query(
        "DELETE ci, ii FROM character_inventory ci \
         JOIN item_instance ii ON ii.guid = ci.item \
         WHERE ci.guid = ? AND ii.itemEntry = ?",
    )
    .bind(guid)
    .bind(RUST_VENDOR_STACK_ITEM)
    .execute(character_pool)
    .await?;

    let occupied_slots: Vec<u8> = sqlx::query_scalar(
        "SELECT slot FROM character_inventory WHERE guid = ? AND bag = 0 AND slot BETWEEN 23 AND 38",
    )
    .bind(guid)
    .fetch_all(character_pool)
    .await?;
    let mut next_item_guid: u32 = sqlx::query_scalar(
        "SELECT CAST(COALESCE(MAX(guid), 0) + 1 AS UNSIGNED) FROM item_instance",
    )
    .fetch_one(character_pool)
    .await?;

    for slot in 23u8..=38u8 {
        if occupied_slots.contains(&slot) {
            continue;
        }
        sqlx::query(
            "INSERT INTO item_instance \
             (guid, owner_guid, itemEntry, creatorGuid, giftCreatorGuid, count, duration, \
              charges, flags, enchantments, randomPropertyId, durability, itemTextId) \
             VALUES (?, ?, 6948, 0, 0, 1, 0, '0 0 0 0 0 ', 0, \
                     '0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 ', 0, 0, 0)",
        )
        .bind(next_item_guid)
        .bind(guid)
        .execute(character_pool)
        .await?;
        sqlx::query(
            "INSERT INTO character_inventory (guid, bag, slot, item, item_template) \
             VALUES (?, 0, ?, ?, 6948)",
        )
        .bind(guid)
        .bind(slot)
        .bind(next_item_guid)
        .execute(character_pool)
        .await?;
        next_item_guid += 1;
    }

    Ok(())
}

async fn seed_db_creature_fixture(
    world_pool: &MySqlPool,
    character: &wow_db::CharacterEnumEntry,
) -> anyhow::Result<()> {
    sqlx::query("DELETE FROM creature WHERE guid = ?")
        .bind(DB_CREATURE_FIXTURE_GUID)
        .execute(world_pool)
        .await?;
    sqlx::query("DELETE FROM npc_vendor WHERE entry = ? AND item IN (?, ?)")
        .bind(DB_CREATURE_FIXTURE_ENTRY)
        .bind(RUST_VENDOR_STACK_ITEM)
        .bind(RUST_VENDOR_BAG_ITEM)
        .execute(world_pool)
        .await?;
    sqlx::query("DELETE FROM npc_vendor WHERE entry = ?")
        .bind(EMPTY_DB_VENDOR_FIXTURE_ENTRY)
        .execute(world_pool)
        .await?;
    sqlx::query(
        "INSERT INTO npc_vendor (entry, item, maxcount, incrtime, slot, condition_id) \
         VALUES (?, ?, 0, 0, 1, 0)",
    )
    .bind(DB_CREATURE_FIXTURE_ENTRY)
    .bind(RUST_VENDOR_STACK_ITEM)
    .execute(world_pool)
    .await?;
    sqlx::query(
        "INSERT INTO npc_vendor (entry, item, maxcount, incrtime, slot, condition_id) \
         VALUES (?, ?, 0, 0, 2, 0)",
    )
    .bind(DB_CREATURE_FIXTURE_ENTRY)
    .bind(RUST_VENDOR_BAG_ITEM)
    .execute(world_pool)
    .await?;
    sqlx::query(
        "INSERT INTO creature \
         (guid, id, map, spawnMask, position_x, position_y, position_z, orientation, \
          spawntimesecsmin, spawntimesecsmax, spawndist, MovementType) \
         VALUES (?, ?, ?, 1, ?, ?, ?, ?, 120, 120, 0, 0)",
    )
    .bind(DB_CREATURE_FIXTURE_GUID)
    .bind(DB_CREATURE_FIXTURE_ENTRY)
    .bind(character.map)
    .bind(character.position_x + 6.0)
    .bind(character.position_y + 1.0)
    .bind(character.position_z)
    .bind(character.orientation)
    .execute(world_pool)
    .await?;
    Ok(())
}

async fn add_item_template_flags(
    world_pool: &MySqlPool,
    entry: u32,
    flags: u32,
) -> anyhow::Result<()> {
    sqlx::query("UPDATE item_template SET Flags = Flags | ? WHERE entry = ?")
        .bind(flags)
        .bind(entry)
        .execute(world_pool)
        .await?;
    Ok(())
}

async fn remove_item_template_flags(
    world_pool: &MySqlPool,
    entry: u32,
    flags: u32,
) -> anyhow::Result<()> {
    sqlx::query("UPDATE item_template SET Flags = Flags & ? WHERE entry = ?")
        .bind(u32::MAX ^ flags)
        .bind(entry)
        .execute(world_pool)
        .await?;
    Ok(())
}

async fn assert_inventory_slot(
    character_pool: &MySqlPool,
    guid: u32,
    item_template: u32,
    expected_slot: u8,
) -> anyhow::Result<()> {
    let actual: Option<u8> = sqlx::query_scalar(
        "SELECT slot FROM character_inventory \
         WHERE guid = ? AND item_template = ? AND bag = 0",
    )
    .bind(guid)
    .bind(item_template)
    .fetch_optional(character_pool)
    .await?;
    ensure!(
        actual == Some(expected_slot),
        "item {item_template} for character {guid} was in slot {:?}, expected {expected_slot}",
        actual
    );
    Ok(())
}

async fn assert_inventory_count_at(
    character_pool: &MySqlPool,
    guid: u32,
    bag: u32,
    slot: u8,
    expected_count: u32,
) -> anyhow::Result<()> {
    let actual: Option<u32> = sqlx::query_scalar(
        "SELECT ii.count FROM character_inventory ci \
         JOIN item_instance ii ON ci.item = ii.guid \
         WHERE ci.guid = ? AND ci.bag = ? AND ci.slot = ?",
    )
    .bind(guid)
    .bind(bag)
    .bind(slot)
    .fetch_optional(character_pool)
    .await?;
    ensure!(
        actual == Some(expected_count),
        "item count at bag {bag} slot {slot} for character {guid} was {:?}, expected {expected_count}",
        actual
    );
    Ok(())
}

async fn inventory_item_guid_at(
    character_pool: &MySqlPool,
    guid: u32,
    bag: u32,
    slot: u8,
) -> anyhow::Result<ObjectGuid> {
    let item_guid: u32 = sqlx::query_scalar(
        "SELECT item FROM character_inventory WHERE guid = ? AND bag = ? AND slot = ?",
    )
    .bind(guid)
    .bind(bag)
    .bind(slot)
    .fetch_one(character_pool)
    .await?;
    Ok(ObjectGuid::new(HighGuid::Item, 0, item_guid))
}

async fn assert_inventory_position_empty(
    character_pool: &MySqlPool,
    guid: u32,
    bag: u32,
    slot: u8,
) -> anyhow::Result<()> {
    let count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM character_inventory WHERE guid = ? AND bag = ? AND slot = ?",
    )
    .bind(guid)
    .bind(bag)
    .bind(slot)
    .fetch_one(character_pool)
    .await?;
    ensure!(
        count == 0,
        "inventory position bag {bag} slot {slot} for character {guid} was not empty"
    );
    Ok(())
}

async fn assert_equipment_cache_slot(
    character_pool: &MySqlPool,
    guid: u32,
    slot: usize,
    expected_item: u32,
) -> anyhow::Result<()> {
    let cache: String = sqlx::query_scalar("SELECT equipmentCache FROM characters WHERE guid = ?")
        .bind(guid)
        .fetch_one(character_pool)
        .await?;
    let values: Vec<u32> = cache
        .split_whitespace()
        .filter_map(|value| value.parse::<u32>().ok())
        .collect();
    let actual = values
        .get(slot * 2)
        .copied()
        .context("equipment cache slot was missing")?;
    ensure!(
        actual == expected_item,
        "equipmentCache slot {slot} for character {guid} was {actual}, expected {expected_item}"
    );
    Ok(())
}

async fn assert_inventory_item_absent(
    character_pool: &MySqlPool,
    guid: u32,
    item_template: u32,
) -> anyhow::Result<()> {
    let inventory_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM character_inventory \
         WHERE guid = ? AND item_template = ?",
    )
    .bind(guid)
    .bind(item_template)
    .fetch_one(character_pool)
    .await?;
    let instance_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM item_instance \
         WHERE owner_guid = ? AND itemEntry = ?",
    )
    .bind(guid)
    .bind(item_template)
    .fetch_one(character_pool)
    .await?;
    ensure!(
        inventory_count == 0 && instance_count == 0,
        "item {item_template} for character {guid} remained after destroy: inventory={inventory_count}, instances={instance_count}"
    );
    Ok(())
}

async fn assert_inventory_item_present(
    character_pool: &MySqlPool,
    guid: u32,
    item_template: u32,
) -> anyhow::Result<()> {
    let inventory_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM character_inventory \
         WHERE guid = ? AND item_template = ?",
    )
    .bind(guid)
    .bind(item_template)
    .fetch_one(character_pool)
    .await?;
    ensure!(
        inventory_count > 0,
        "item {item_template} for character {guid} was not present"
    );
    Ok(())
}

async fn inventory_item_row_count(
    character_pool: &MySqlPool,
    guid: u32,
    item_template: u32,
) -> anyhow::Result<i64> {
    let inventory_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM character_inventory \
         WHERE guid = ? AND item_template = ?",
    )
    .bind(guid)
    .bind(item_template)
    .fetch_one(character_pool)
    .await?;
    Ok(inventory_count)
}

async fn assert_inventory_item_row_count(
    character_pool: &MySqlPool,
    guid: u32,
    item_template: u32,
    expected_count: i64,
) -> anyhow::Result<()> {
    let inventory_count = inventory_item_row_count(character_pool, guid, item_template).await?;
    ensure!(
        inventory_count == expected_count,
        "item {item_template} row count for character {guid} was {inventory_count}, expected {expected_count}"
    );
    Ok(())
}

async fn inventory_item_total_count(
    character_pool: &MySqlPool,
    guid: u32,
    item_template: u32,
) -> anyhow::Result<u32> {
    let total: u64 = sqlx::query_scalar(
        "SELECT CAST(COALESCE(SUM(ii.count), 0) AS UNSIGNED) FROM character_inventory ci \
         JOIN item_instance ii ON ii.guid = ci.item \
         WHERE ci.guid = ? AND ci.item_template = ?",
    )
    .bind(guid)
    .bind(item_template)
    .fetch_one(character_pool)
    .await?;
    Ok(total as u32)
}

async fn assert_inventory_item_total_count(
    character_pool: &MySqlPool,
    guid: u32,
    item_template: u32,
    expected_count: u32,
) -> anyhow::Result<()> {
    let total_count = inventory_item_total_count(character_pool, guid, item_template).await?;
    ensure!(
        total_count == expected_count,
        "item {item_template} total count for character {guid} was {total_count}, expected {expected_count}"
    );
    Ok(())
}

async fn inventory_item_guid_with_count(
    character_pool: &MySqlPool,
    guid: u32,
    item_template: u32,
    count: u32,
) -> anyhow::Result<ObjectGuid> {
    let item_guid: u32 = sqlx::query_scalar(
        "SELECT ci.item FROM character_inventory ci \
         JOIN item_instance ii ON ii.guid = ci.item \
         WHERE ci.guid = ? AND ci.item_template = ? AND ii.count = ? \
         ORDER BY ci.bag, ci.slot \
         LIMIT 1",
    )
    .bind(guid)
    .bind(item_template)
    .bind(count)
    .fetch_one(character_pool)
    .await
    .with_context(|| {
        format!("item {item_template} with count {count} was missing for character {guid}")
    })?;
    Ok(ObjectGuid::new(HighGuid::Item, 0, item_guid))
}

async fn inventory_item_guids_for_template(
    character_pool: &MySqlPool,
    guid: u32,
    item_template: u32,
) -> anyhow::Result<Vec<ObjectGuid>> {
    let item_guids: Vec<u32> = sqlx::query_scalar(
        "SELECT item FROM character_inventory \
         WHERE guid = ? AND item_template = ? \
         ORDER BY bag, slot",
    )
    .bind(guid)
    .bind(item_template)
    .fetch_all(character_pool)
    .await?;
    Ok(item_guids
        .into_iter()
        .map(|item_guid| ObjectGuid::new(HighGuid::Item, 0, item_guid))
        .collect())
}

async fn item_prices_and_buy_count(
    world_pool: &MySqlPool,
    item_template: u32,
) -> anyhow::Result<(u32, u32, u32)> {
    let (buy_price, sell_price, buy_count): (u32, u32, u32) =
        sqlx::query_as("SELECT BuyPrice, SellPrice, BuyCount FROM item_template WHERE entry = ?")
            .bind(item_template)
            .fetch_one(world_pool)
            .await?;
    ensure!(buy_price > 0, "item {item_template} has zero BuyPrice");
    ensure!(sell_price > 0, "item {item_template} has zero SellPrice");
    Ok((buy_price, sell_price, buy_count.max(1)))
}

async fn set_character_money(
    character_pool: &MySqlPool,
    guid: u32,
    money: u32,
) -> anyhow::Result<()> {
    sqlx::query("UPDATE characters SET money = ? WHERE guid = ?")
        .bind(money)
        .bind(guid)
        .execute(character_pool)
        .await?;
    Ok(())
}

async fn assert_character_money_eq(
    character_pool: &MySqlPool,
    guid: u32,
    expected_money: u32,
) -> anyhow::Result<()> {
    let money: u32 = sqlx::query_scalar("SELECT money FROM characters WHERE guid = ?")
        .bind(guid)
        .fetch_one(character_pool)
        .await?;
    ensure!(
        money == expected_money,
        "character money was {money}, expected {expected_money}"
    );
    Ok(())
}

async fn assert_inventory_item_instance_count(
    character_pool: &MySqlPool,
    item_guid: ObjectGuid,
    expected_count: u32,
) -> anyhow::Result<()> {
    let count: u32 = sqlx::query_scalar("SELECT count FROM item_instance WHERE guid = ?")
        .bind(item_guid.counter())
        .fetch_one(character_pool)
        .await?;
    ensure!(
        count == expected_count,
        "item instance {} count was {count}, expected {expected_count}",
        item_guid.counter()
    );
    Ok(())
}

async fn assert_inventory_guid_absent(
    character_pool: &MySqlPool,
    item_guid: ObjectGuid,
) -> anyhow::Result<()> {
    let inventory_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM character_inventory WHERE item = ?")
            .bind(item_guid.counter())
            .fetch_one(character_pool)
            .await?;
    let instance_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM item_instance WHERE guid = ?")
            .bind(item_guid.counter())
            .fetch_one(character_pool)
            .await?;
    ensure!(
        inventory_count == 0 && instance_count == 0,
        "sold item {} remained after full sell: inventory={inventory_count}, instances={instance_count}",
        item_guid.counter()
    );
    Ok(())
}

async fn assert_character_money_at_least(
    character_pool: &MySqlPool,
    guid: u32,
    minimum_money: u32,
) -> anyhow::Result<()> {
    let money: u32 = sqlx::query_scalar("SELECT money FROM characters WHERE guid = ?")
        .bind(guid)
        .fetch_one(character_pool)
        .await?;
    ensure!(
        money >= minimum_money,
        "character money was {money}, expected at least {minimum_money}"
    );
    Ok(())
}

async fn clear_guild_fixture(character_pool: &MySqlPool) -> anyhow::Result<()> {
    sqlx::query("DELETE FROM guild_member WHERE guildid = ?")
        .bind(90_001u32)
        .execute(character_pool)
        .await?;
    sqlx::query("DELETE FROM guild WHERE guildid = ?")
        .bind(90_001u32)
        .execute(character_pool)
        .await?;
    Ok(())
}

async fn seed_guild_leader_fixture(
    character_pool: &MySqlPool,
    leader_guid: u32,
) -> anyhow::Result<()> {
    sqlx::query(
        "INSERT INTO guild \
         (guildid, name, leaderguid, EmblemStyle, EmblemColor, BorderStyle, BorderColor, \
          BackgroundColor, info, motd, createdate) \
         VALUES (?, 'World Flow Guild', ?, 0, 0, 0, 0, 0, '', '', UNIX_TIMESTAMP())",
    )
    .bind(90_001u32)
    .bind(leader_guid)
    .execute(character_pool)
    .await?;
    sqlx::query(
        "INSERT INTO guild_member (guildid, guid, rank, pnote, offnote) VALUES (?, ?, 0, '', '')",
    )
    .bind(90_001u32)
    .bind(leader_guid)
    .execute(character_pool)
    .await?;
    Ok(())
}

async fn seed_guild_member_fixture(
    character_pool: &MySqlPool,
    member_guid: u32,
) -> anyhow::Result<()> {
    sqlx::query(
        "INSERT INTO guild \
         (guildid, name, leaderguid, EmblemStyle, EmblemColor, BorderStyle, BorderColor, \
          BackgroundColor, info, motd, createdate) \
         VALUES (?, 'World Flow Guild', 999999, 0, 0, 0, 0, 0, '', '', UNIX_TIMESTAMP())",
    )
    .bind(90_001u32)
    .execute(character_pool)
    .await?;
    sqlx::query(
        "INSERT INTO guild_member (guildid, guid, rank, pnote, offnote) VALUES (?, ?, 1, '', '')",
    )
    .bind(90_001u32)
    .bind(member_guid)
    .execute(character_pool)
    .await?;
    sqlx::query(
        "INSERT INTO guild_eventlog \
         (guildid, LogGuid, EventType, PlayerGuid1, PlayerGuid2, NewRank, TimeStamp) \
         VALUES (?, 1, 1, ?, 0, 1, UNIX_TIMESTAMP()), \
                (?, 2, 2, 0, ?, 1, UNIX_TIMESTAMP())",
    )
    .bind(90_001u32)
    .bind(member_guid)
    .bind(90_001u32)
    .bind(member_guid)
    .execute(character_pool)
    .await?;
    Ok(())
}

async fn seed_group_leader_fixture(
    character_pool: &MySqlPool,
    account_id: u32,
    leader_guid: u32,
) -> anyhow::Result<()> {
    let member_guids: Vec<u32> = sqlx::query_scalar(
        "SELECT guid FROM characters WHERE account = ? AND guid <> ? ORDER BY guid LIMIT 2",
    )
    .bind(account_id)
    .bind(leader_guid)
    .fetch_all(character_pool)
    .await?;
    ensure!(
        member_guids.len() == 2,
        "group fixture needs two extra characters"
    );

    sqlx::query("DELETE FROM group_instance WHERE leaderGuid IN (?, ?, ?)")
        .bind(leader_guid)
        .bind(member_guids[0])
        .bind(member_guids[1])
        .execute(character_pool)
        .await?;
    sqlx::query("DELETE FROM group_member WHERE groupId = ?")
        .bind(91_001u32)
        .execute(character_pool)
        .await?;
    sqlx::query("DELETE FROM `groups` WHERE groupId = ?")
        .bind(91_001u32)
        .execute(character_pool)
        .await?;

    sqlx::query(
        "INSERT INTO `groups` \
         (groupId, leaderGuid, mainTank, mainAssistant, lootMethod, looterGuid, lootThreshold, \
          icon1, icon2, icon3, icon4, icon5, icon6, icon7, icon8, isRaid) \
         VALUES (?, ?, 0, 0, 0, ?, 2, 0, 0, 0, 0, 0, 0, 0, 0, 0)",
    )
    .bind(91_001u32)
    .bind(leader_guid)
    .bind(leader_guid)
    .execute(character_pool)
    .await?;

    for (subgroup, member_guid) in [leader_guid, member_guids[0], member_guids[1]]
        .into_iter()
        .enumerate()
    {
        sqlx::query(
            "INSERT INTO group_member (groupId, memberGuid, assistant, subgroup) VALUES (?, ?, 0, ?)",
        )
        .bind(91_001u32)
        .bind(member_guid)
        .bind(subgroup as u16)
        .execute(character_pool)
        .await?;
    }

    sqlx::query("INSERT INTO group_instance (leaderGuid, instance, permanent) VALUES (?, ?, 0)")
        .bind(leader_guid)
        .bind(92_001u32)
        .execute(character_pool)
        .await?;

    Ok(())
}

async fn assert_guild_member_cleanup(character_pool: &MySqlPool, guid: u32) -> anyhow::Result<()> {
    let member_rows: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM guild_member WHERE guid = ?")
        .bind(guid)
        .fetch_one(character_pool)
        .await?;
    let event_rows: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM guild_eventlog WHERE PlayerGuid1 = ? OR PlayerGuid2 = ?",
    )
    .bind(guid)
    .bind(guid)
    .fetch_one(character_pool)
    .await?;

    ensure!(member_rows == 0, "guild_member row remained after delete");
    ensure!(event_rows == 0, "guild_eventlog rows remained after delete");
    Ok(())
}

async fn assert_group_leader_cleanup(character_pool: &MySqlPool, guid: u32) -> anyhow::Result<()> {
    let member_rows: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM group_member WHERE memberGuid = ?")
            .bind(guid)
            .fetch_one(character_pool)
            .await?;
    let old_leader_instances: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM group_instance WHERE leaderGuid = ?")
            .bind(guid)
            .fetch_one(character_pool)
            .await?;
    let new_leader: Option<u32> =
        sqlx::query_scalar("SELECT leaderGuid FROM `groups` WHERE groupId = ?")
            .bind(91_001u32)
            .fetch_optional(character_pool)
            .await?;
    let transferred_instances: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM group_instance WHERE instance = ?")
            .bind(92_001u32)
            .fetch_one(character_pool)
            .await?;

    ensure!(member_rows == 0, "group_member row remained after delete");
    ensure!(
        old_leader_instances == 0,
        "group_instance row remained on deleted leader"
    );
    ensure!(
        new_leader.is_some_and(|leader| leader != guid),
        "group leader was not transferred away from deleted character"
    );
    ensure!(
        transferred_instances == 1,
        "group instance bind was not preserved during leader transfer"
    );
    Ok(())
}

async fn seed_social_fixture(
    character_pool: &MySqlPool,
    account_id: u32,
    deleted_guid: u32,
) -> anyhow::Result<()> {
    let friend_guid: u32 = sqlx::query_scalar(
        "SELECT guid FROM characters WHERE account = ? AND guid <> ? ORDER BY guid LIMIT 1",
    )
    .bind(account_id)
    .bind(deleted_guid)
    .fetch_one(character_pool)
    .await?;

    sqlx::query("DELETE FROM character_social WHERE guid IN (?, ?) OR friend IN (?, ?)")
        .bind(deleted_guid)
        .bind(friend_guid)
        .bind(deleted_guid)
        .bind(friend_guid)
        .execute(character_pool)
        .await?;
    sqlx::query("INSERT INTO character_social (guid, friend, flags) VALUES (?, ?, 1), (?, ?, 1)")
        .bind(deleted_guid)
        .bind(friend_guid)
        .bind(friend_guid)
        .bind(deleted_guid)
        .execute(character_pool)
        .await?;

    Ok(())
}

async fn seed_pet_fixture(character_pool: &MySqlPool, owner_guid: u32) -> anyhow::Result<()> {
    sqlx::query("DELETE FROM pet_aura WHERE guid = ?")
        .bind(93_001u32)
        .execute(character_pool)
        .await?;
    sqlx::query("DELETE FROM pet_spell WHERE guid = ?")
        .bind(93_001u32)
        .execute(character_pool)
        .await?;
    sqlx::query("DELETE FROM pet_spell_cooldown WHERE guid = ?")
        .bind(93_001u32)
        .execute(character_pool)
        .await?;
    sqlx::query("DELETE FROM character_pet WHERE id = ?")
        .bind(93_001u32)
        .execute(character_pool)
        .await?;

    sqlx::query(
        "INSERT INTO character_pet (id, entry, owner, modelid, PetType, level, name, slot) \
         VALUES (?, 416, ?, 416, 1, 1, 'Worldflow', 0)",
    )
    .bind(93_001u32)
    .bind(owner_guid)
    .execute(character_pool)
    .await?;
    sqlx::query(
        "INSERT INTO pet_aura (guid, caster_guid, item_guid, spell, stackcount) \
         VALUES (?, 0, 0, 197, 1)",
    )
    .bind(93_001u32)
    .execute(character_pool)
    .await?;
    sqlx::query("INSERT INTO pet_spell (guid, spell, active) VALUES (?, 172, 1)")
        .bind(93_001u32)
        .execute(character_pool)
        .await?;
    sqlx::query("INSERT INTO pet_spell_cooldown (guid, spell, time) VALUES (?, 172, 1)")
        .bind(93_001u32)
        .execute(character_pool)
        .await?;

    Ok(())
}

async fn seed_mail_fixture(character_pool: &MySqlPool, receiver_guid: u32) -> anyhow::Result<()> {
    sqlx::query("DELETE FROM mail_items WHERE mail_id = ? OR item_guid = ?")
        .bind(94_001u32)
        .bind(94_101u32)
        .execute(character_pool)
        .await?;
    sqlx::query("DELETE FROM mail WHERE id = ?")
        .bind(94_001u32)
        .execute(character_pool)
        .await?;
    sqlx::query("DELETE FROM item_instance WHERE guid = ?")
        .bind(94_101u32)
        .execute(character_pool)
        .await?;

    sqlx::query(
        "INSERT INTO item_instance \
         (guid, owner_guid, itemEntry, creatorGuid, giftCreatorGuid, count, duration, \
          charges, flags, enchantments, randomPropertyId, durability, itemTextId) \
         VALUES (?, ?, 6948, 0, 0, 1, 0, '0 0 0 0 0 ', 0, \
                 '0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 ', 0, 0, 0)",
    )
    .bind(94_101u32)
    .bind(receiver_guid)
    .execute(character_pool)
    .await?;
    sqlx::query(
        "INSERT INTO mail \
         (id, messageType, sender, receiver, subject, itemTextId, has_items, expire_time, \
          deliver_time, money, cod, checked) \
         VALUES (?, 0, 0, ?, 'world-flow-delete', 0, 1, UNIX_TIMESTAMP() + 2592000, \
                 UNIX_TIMESTAMP(), 0, 0, 0)",
    )
    .bind(94_001u32)
    .bind(receiver_guid)
    .execute(character_pool)
    .await?;
    sqlx::query(
        "INSERT INTO mail_items (mail_id, item_guid, item_template, receiver) VALUES (?, ?, 6948, ?)",
    )
    .bind(94_001u32)
    .bind(94_101u32)
    .bind(receiver_guid)
    .execute(character_pool)
    .await?;

    Ok(())
}

async fn seed_cod_mail_return_fixture(
    character_pool: &MySqlPool,
    account_id: u32,
    receiver_guid: u32,
) -> anyhow::Result<u32> {
    let sender_guid: u32 = sqlx::query_scalar(
        "SELECT guid FROM characters WHERE account = ? AND guid <> ? ORDER BY guid DESC LIMIT 1",
    )
    .bind(account_id)
    .bind(receiver_guid)
    .fetch_one(character_pool)
    .await?;

    sqlx::query("DELETE FROM mail_items WHERE mail_id = ? OR item_guid = ?")
        .bind(94_002u32)
        .bind(94_201u32)
        .execute(character_pool)
        .await?;
    sqlx::query(
        "DELETE FROM mail WHERE id = ? OR (sender = ? AND receiver = ? AND subject = 'world-flow-cod-return')",
    )
    .bind(94_002u32)
    .bind(receiver_guid)
    .bind(sender_guid)
    .execute(character_pool)
    .await?;
    sqlx::query("DELETE FROM item_instance WHERE guid = ?")
        .bind(94_201u32)
        .execute(character_pool)
        .await?;

    sqlx::query(
        "INSERT INTO item_instance \
         (guid, owner_guid, itemEntry, creatorGuid, giftCreatorGuid, count, duration, \
          charges, flags, enchantments, randomPropertyId, durability, itemTextId) \
         VALUES (?, ?, 6948, 0, 0, 1, 0, '0 0 0 0 0 ', 0, \
                 '0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 ', 0, 0, 0)",
    )
    .bind(94_201u32)
    .bind(receiver_guid)
    .execute(character_pool)
    .await?;
    sqlx::query(
        "INSERT INTO mail \
         (id, messageType, sender, receiver, subject, itemTextId, has_items, expire_time, \
          deliver_time, money, cod, checked) \
         VALUES (?, 0, ?, ?, 'world-flow-cod-return', 0, 1, UNIX_TIMESTAMP() + 259200, \
                 UNIX_TIMESTAMP(), 1234, 5678, 0)",
    )
    .bind(94_002u32)
    .bind(sender_guid)
    .bind(receiver_guid)
    .execute(character_pool)
    .await?;
    sqlx::query(
        "INSERT INTO mail_items (mail_id, item_guid, item_template, receiver) VALUES (?, ?, 6948, ?)",
    )
    .bind(94_002u32)
    .bind(94_201u32)
    .bind(receiver_guid)
    .execute(character_pool)
    .await?;

    Ok(sender_guid)
}

async fn seed_auction_fixture(character_pool: &MySqlPool, owner_guid: u32) -> anyhow::Result<()> {
    sqlx::query("DELETE FROM auction WHERE id = ?")
        .bind(95_001u32)
        .execute(character_pool)
        .await?;
    sqlx::query("DELETE FROM item_instance WHERE guid = ?")
        .bind(95_101u32)
        .execute(character_pool)
        .await?;

    sqlx::query(
        "INSERT INTO item_instance \
         (guid, owner_guid, itemEntry, creatorGuid, giftCreatorGuid, count, duration, \
          charges, flags, enchantments, randomPropertyId, durability, itemTextId) \
         VALUES (?, ?, 6948, 0, 0, 1, 0, '0 0 0 0 0 ', 0, \
                 '0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 ', 0, 0, 0)",
    )
    .bind(95_101u32)
    .bind(owner_guid)
    .execute(character_pool)
    .await?;
    sqlx::query(
        "INSERT INTO auction \
         (id, houseid, itemguid, item_template, item_count, itemowner, buyoutprice, \
          time, buyguid, lastbid, startbid, deposit) \
         VALUES (?, 1, ?, 6948, 1, ?, 0, UNIX_TIMESTAMP() + 3600, 0, 0, 1, 0)",
    )
    .bind(95_001u32)
    .bind(95_101u32)
    .bind(owner_guid)
    .execute(character_pool)
    .await?;

    Ok(())
}

async fn assert_social_cleanup(character_pool: &MySqlPool, guid: u32) -> anyhow::Result<()> {
    let rows: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM character_social WHERE guid = ? OR friend = ?")
            .bind(guid)
            .bind(guid)
            .fetch_one(character_pool)
            .await?;

    ensure!(rows == 0, "character_social rows remained after delete");
    Ok(())
}

async fn assert_pet_cleanup(character_pool: &MySqlPool, pet_id: u32) -> anyhow::Result<()> {
    let pet_rows: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM character_pet WHERE id = ?")
        .bind(pet_id)
        .fetch_one(character_pool)
        .await?;
    let aura_rows: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM pet_aura WHERE guid = ?")
        .bind(pet_id)
        .fetch_one(character_pool)
        .await?;
    let spell_rows: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM pet_spell WHERE guid = ?")
        .bind(pet_id)
        .fetch_one(character_pool)
        .await?;
    let cooldown_rows: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM pet_spell_cooldown WHERE guid = ?")
            .bind(pet_id)
            .fetch_one(character_pool)
            .await?;

    ensure!(pet_rows == 0, "character_pet row remained after delete");
    ensure!(aura_rows == 0, "pet_aura rows remained after delete");
    ensure!(spell_rows == 0, "pet_spell rows remained after delete");
    ensure!(
        cooldown_rows == 0,
        "pet_spell_cooldown rows remained after delete"
    );
    Ok(())
}

async fn assert_mail_cleanup(
    character_pool: &MySqlPool,
    receiver_guid: u32,
    mail_id: u32,
    item_guid: u32,
) -> anyhow::Result<()> {
    let mail_rows: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM mail WHERE receiver = ?")
        .bind(receiver_guid)
        .fetch_one(character_pool)
        .await?;
    let mail_item_rows: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM mail_items WHERE receiver = ? OR mail_id = ?")
            .bind(receiver_guid)
            .bind(mail_id)
            .fetch_one(character_pool)
            .await?;
    let item_rows: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM item_instance WHERE guid = ?")
        .bind(item_guid)
        .fetch_one(character_pool)
        .await?;

    ensure!(mail_rows == 0, "mail rows remained after delete");
    ensure!(mail_item_rows == 0, "mail_items rows remained after delete");
    ensure!(
        item_rows == 0,
        "owned mail item_instance row remained after delete"
    );
    Ok(())
}

async fn assert_cod_mail_return(
    character_pool: &MySqlPool,
    deleted_guid: u32,
    sender_guid: u32,
    old_mail_id: u32,
    item_guid: u32,
) -> anyhow::Result<()> {
    let old_mail_rows: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM mail WHERE id = ?")
        .bind(old_mail_id)
        .fetch_one(character_pool)
        .await?;
    let returned_mail: Option<(u32, u32, u32, u8, u8)> = sqlx::query_as(
        "SELECT id, money, cod, checked, has_items \
         FROM mail WHERE sender = ? AND receiver = ? AND subject = 'world-flow-cod-return'",
    )
    .bind(deleted_guid)
    .bind(sender_guid)
    .fetch_optional(character_pool)
    .await?;
    let returned_item_receiver: Option<u32> =
        sqlx::query_scalar("SELECT receiver FROM mail_items WHERE item_guid = ?")
            .bind(item_guid)
            .fetch_optional(character_pool)
            .await?;
    let item_owner: Option<u32> =
        sqlx::query_scalar("SELECT owner_guid FROM item_instance WHERE guid = ?")
            .bind(item_guid)
            .fetch_optional(character_pool)
            .await?;

    ensure!(
        old_mail_rows == 0,
        "original COD mail remained after delete"
    );
    let Some((returned_id, money, cod, checked, has_items)) = returned_mail else {
        anyhow::bail!("COD mail was not returned to sender");
    };
    ensure!(
        returned_id != old_mail_id,
        "COD return reused the original mail id"
    );
    ensure!(money == 1234, "returned COD mail did not preserve money");
    ensure!(cod == 0, "returned COD mail kept COD charge");
    ensure!(checked == 0x02, "returned COD mail was not marked returned");
    ensure!(has_items == 1, "returned COD mail lost has_items flag");
    ensure!(
        returned_item_receiver == Some(sender_guid),
        "returned COD mail item receiver was not sender"
    );
    ensure!(
        item_owner == Some(sender_guid),
        "returned COD item owner was not sender"
    );
    Ok(())
}

async fn assert_auction_cleanup(
    character_pool: &MySqlPool,
    owner_guid: u32,
    auction_id: u32,
    item_guid: u32,
) -> anyhow::Result<()> {
    let auction_rows: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM auction WHERE id = ? OR itemowner = ? OR buyguid = ?",
    )
    .bind(auction_id)
    .bind(owner_guid)
    .bind(owner_guid)
    .fetch_one(character_pool)
    .await?;
    let item_rows: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM item_instance WHERE guid = ?")
        .bind(item_guid)
        .fetch_one(character_pool)
        .await?;

    ensure!(auction_rows == 0, "auction rows remained after delete");
    ensure!(item_rows == 0, "auction item remained after delete");
    Ok(())
}

fn complete_auth_flow() -> anyhow::Result<()> {
    let mut stream = connect_blocking(AUTH_ADDR)?;
    let (challenge, client) = perform_challenge(&mut stream)?;
    ensure!(
        challenge.error == 0,
        "auth challenge failed with {}",
        challenge.error
    );

    let proof = send_proof(&mut stream, &client)?;
    ensure!(proof.cmd == AuthCommand::LogonProof);
    ensure!(proof.error == 0, "auth proof failed with {}", proof.error);
    client.verify_server_proof(proof.m2)?;
    Ok(())
}

fn perform_challenge(
    stream: &mut TcpStream,
) -> anyhow::Result<(LogonChallengeResponse, SrpClientChallenge)> {
    stream.write_all(&logon_challenge_request())?;

    let challenge_bytes = read_exact_vec(stream, LogonChallengeResponse::SIZE)?;
    let challenge = LogonChallengeResponse::read(&mut &challenge_bytes[..])?;
    ensure!(challenge.cmd == AuthCommand::LogonChallenge);
    ensure!(challenge.g_len == 1, "unexpected generator length");
    ensure!(challenge.n_len == 32, "unexpected safe-prime length");

    let client = SrpClientChallenge::new(
        NormalizedString::new(USERNAME)?,
        NormalizedString::new(PASSWORD)?,
        challenge.g,
        challenge.n,
        PublicKey::from_le_bytes(challenge.server_public)?,
        challenge.salt,
    );

    Ok((challenge, client))
}

fn send_proof(
    stream: &mut TcpStream,
    client: &SrpClientChallenge,
) -> anyhow::Result<LogonProofResponse> {
    let proof_request = LogonProofRequest {
        cmd: AuthCommand::LogonProof,
        client_public: *client.client_public_key(),
        m1: *client.client_proof(),
        crc_hash: [0; 20],
        num_keys: 0,
        security_flags: 0,
    };
    let mut proof_bytes = BytesMut::new();
    proof_request.write(&mut proof_bytes);
    stream.write_all(&proof_bytes)?;

    let response = read_exact_vec(stream, LogonProofResponse::SIZE)?;
    Ok(LogonProofResponse::read(&mut &response[..])?)
}

async fn fetch_session_key(login_pool: &MySqlPool) -> anyhow::Result<[u8; 40]> {
    let session_key: String =
        sqlx::query_scalar("SELECT sessionkey FROM account WHERE username = ?")
            .bind(USERNAME)
            .fetch_one(login_pool)
            .await?;
    hex_to_array40(&session_key)
}

struct WorldClient {
    stream: TcpStream,
    crypto: HeaderCrypto,
}

impl WorldClient {
    fn connect(session_key: &[u8; 40]) -> anyhow::Result<Self> {
        let mut stream = connect_blocking(WORLD_ADDR)?;
        let (opcode, body) = read_server_packet(&mut stream, None)?;
        ensure!(opcode == SMSG_AUTH_CHALLENGE, "expected auth challenge");
        ensure!(body.len() == 4, "world auth challenge body was malformed");
        let server_seed = u32::from_le_bytes(body.as_slice().try_into()?);

        let auth_body = auth_session_body(session_key, server_seed);
        write_client_packet(&mut stream, CMSG_AUTH_SESSION, &auth_body, None)?;

        let mut crypto = HeaderCrypto::new(session_key);
        let (opcode, body) = read_server_packet(&mut stream, Some(&mut crypto))?;
        ensure!(opcode == SMSG_AUTH_RESPONSE, "expected SMSG_AUTH_RESPONSE");
        ensure!(
            body.first() == Some(&AUTH_OK),
            "world auth failed with body {:02X?}",
            body
        );

        Ok(Self { stream, crypto })
    }

    fn char_enum(&mut self) -> anyhow::Result<Vec<EnumCharacter>> {
        write_client_packet(
            &mut self.stream,
            CMSG_CHAR_ENUM,
            &[],
            Some(&mut self.crypto),
        )?;
        let (opcode, body) = read_server_packet(&mut self.stream, Some(&mut self.crypto))?;
        ensure!(opcode == SMSG_CHAR_ENUM, "expected SMSG_CHAR_ENUM");
        parse_char_enum(&body)
    }

    fn login_character(&mut self, guid: u32) -> anyhow::Result<()> {
        let guid = ObjectGuid::new(HighGuid::Player, 0, guid);
        write_client_packet(
            &mut self.stream,
            CMSG_PLAYER_LOGIN,
            &guid.raw().to_le_bytes(),
            Some(&mut self.crypto),
        )?;

        for _ in 0..11 {
            let _ = read_server_packet(&mut self.stream, Some(&mut self.crypto))?;
        }
        Ok(())
    }

    fn swap_inventory_slots(&mut self, src_slot: u8, dst_slot: u8) -> anyhow::Result<()> {
        write_client_packet(
            &mut self.stream,
            CMSG_SWAP_INV_ITEM,
            &[src_slot, dst_slot],
            Some(&mut self.crypto),
        )?;
        let (opcode, _) = read_server_packet(&mut self.stream, Some(&mut self.crypto))?;
        ensure!(
            opcode == SMSG_UPDATE_OBJECT,
            "expected SMSG_UPDATE_OBJECT after inventory move, got 0x{opcode:04X}"
        );
        Ok(())
    }

    fn swap_item(
        &mut self,
        src_bag: u8,
        src_slot: u8,
        dst_bag: u8,
        dst_slot: u8,
    ) -> anyhow::Result<()> {
        write_client_packet(
            &mut self.stream,
            CMSG_SWAP_ITEM,
            &[dst_bag, dst_slot, src_bag, src_slot],
            Some(&mut self.crypto),
        )?;
        let (opcode, _) = read_server_packet(&mut self.stream, Some(&mut self.crypto))?;
        ensure!(
            opcode == SMSG_UPDATE_OBJECT,
            "expected SMSG_UPDATE_OBJECT after item swap, got 0x{opcode:04X}"
        );
        Ok(())
    }

    fn list_rust_guide_inventory(&mut self) -> anyhow::Result<()> {
        write_client_packet(
            &mut self.stream,
            CMSG_LIST_INVENTORY,
            &rust_guide_guid().raw().to_le_bytes(),
            Some(&mut self.crypto),
        )?;
        let (opcode, body) = read_server_packet(&mut self.stream, Some(&mut self.crypto))?;
        ensure!(
            opcode == SMSG_LIST_INVENTORY,
            "expected SMSG_LIST_INVENTORY, got 0x{opcode:04X}"
        );
        ensure!(
            body.get(8) == Some(&2),
            "vendor inventory body was malformed"
        );
        Ok(())
    }

    fn list_db_vendor_inventory(&mut self) -> anyhow::Result<()> {
        let guid = ObjectGuid::new(
            HighGuid::Unit,
            DB_CREATURE_FIXTURE_ENTRY,
            DB_CREATURE_FIXTURE_GUID,
        );
        write_client_packet(
            &mut self.stream,
            CMSG_LIST_INVENTORY,
            &guid.raw().to_le_bytes(),
            Some(&mut self.crypto),
        )?;
        let (opcode, body) = read_server_packet(&mut self.stream, Some(&mut self.crypto))?;
        ensure!(
            opcode == SMSG_LIST_INVENTORY,
            "expected DB SMSG_LIST_INVENTORY, got 0x{opcode:04X}"
        );
        ensure_available(&body, 37)?;
        ensure!(
            &body[0..8] == guid.raw().to_le_bytes().as_slice(),
            "DB vendor guid mismatch"
        );
        ensure!(body[8] == 1, "DB vendor item count was {}", body[8]);
        let item = u32::from_le_bytes(body[13..17].try_into()?);
        ensure!(
            item == RUST_VENDOR_STACK_ITEM,
            "DB vendor item was {item}, expected {RUST_VENDOR_STACK_ITEM}"
        );
        ensure!(
            item != RUST_VENDOR_BAG_ITEM,
            "DB vendor exposed filtered container item {RUST_VENDOR_BAG_ITEM}"
        );
        let available = u32::from_le_bytes(body[21..25].try_into()?);
        ensure!(
            available == u32::MAX,
            "DB vendor unlimited count marker was {available}"
        );
        Ok(())
    }

    fn list_empty_db_vendor_inventory(&mut self) -> anyhow::Result<()> {
        let guid = ObjectGuid::new(
            HighGuid::Unit,
            EMPTY_DB_VENDOR_FIXTURE_ENTRY,
            EMPTY_DB_VENDOR_FIXTURE_GUID,
        );
        write_client_packet(
            &mut self.stream,
            CMSG_LIST_INVENTORY,
            &guid.raw().to_le_bytes(),
            Some(&mut self.crypto),
        )?;
        let (opcode, body) = read_server_packet(&mut self.stream, Some(&mut self.crypto))?;
        ensure!(
            opcode == SMSG_LIST_INVENTORY,
            "expected empty DB SMSG_LIST_INVENTORY, got 0x{opcode:04X}"
        );
        ensure_available(&body, 10)?;
        ensure!(
            body.len() == 10,
            "empty DB vendor body length was {}, expected 10",
            body.len()
        );
        ensure!(
            &body[0..8] == guid.raw().to_le_bytes().as_slice(),
            "empty DB vendor guid mismatch"
        );
        ensure!(body[8] == 0, "empty DB vendor item count was {}", body[8]);
        ensure!(
            body[9] == 0,
            "empty DB vendor no-inventory marker was {}",
            body[9]
        );
        Ok(())
    }

    fn open_db_vendor_gossip_inventory(&mut self) -> anyhow::Result<()> {
        let guid = ObjectGuid::new(
            HighGuid::Unit,
            DB_CREATURE_FIXTURE_ENTRY,
            DB_CREATURE_FIXTURE_GUID,
        );
        write_client_packet(
            &mut self.stream,
            CMSG_GOSSIP_HELLO,
            &guid.raw().to_le_bytes(),
            Some(&mut self.crypto),
        )?;
        let (opcode, _) = read_server_packet(&mut self.stream, Some(&mut self.crypto))?;
        ensure!(
            opcode == SMSG_NPC_TEXT_UPDATE,
            "expected SMSG_NPC_TEXT_UPDATE after DB gossip hello, got 0x{opcode:04X}"
        );
        let (opcode, body) = read_server_packet(&mut self.stream, Some(&mut self.crypto))?;
        ensure!(
            opcode == SMSG_GOSSIP_MESSAGE,
            "expected SMSG_GOSSIP_MESSAGE after DB gossip hello, got 0x{opcode:04X}"
        );
        ensure_available(&body, 17)?;
        ensure!(
            &body[0..8] == guid.raw().to_le_bytes().as_slice(),
            "DB gossip guid mismatch"
        );
        ensure!(
            u32::from_le_bytes(body[12..16].try_into()?) == 1,
            "DB gossip option count was not one"
        );

        let mut select = Vec::with_capacity(12);
        select.extend_from_slice(&guid.raw().to_le_bytes());
        select.extend_from_slice(&0u32.to_le_bytes());
        write_client_packet(
            &mut self.stream,
            CMSG_GOSSIP_SELECT_OPTION,
            &select,
            Some(&mut self.crypto),
        )?;
        self.expect_db_vendor_inventory_response(guid)
    }

    fn reject_invalid_db_vendor_gossip_option(&mut self) -> anyhow::Result<()> {
        let guid = ObjectGuid::new(
            HighGuid::Unit,
            DB_CREATURE_FIXTURE_ENTRY,
            DB_CREATURE_FIXTURE_GUID,
        );
        write_client_packet(
            &mut self.stream,
            CMSG_GOSSIP_HELLO,
            &guid.raw().to_le_bytes(),
            Some(&mut self.crypto),
        )?;
        let (opcode, _) = read_server_packet(&mut self.stream, Some(&mut self.crypto))?;
        ensure!(
            opcode == SMSG_NPC_TEXT_UPDATE,
            "expected SMSG_NPC_TEXT_UPDATE after DB gossip hello, got 0x{opcode:04X}"
        );
        let (opcode, _) = read_server_packet(&mut self.stream, Some(&mut self.crypto))?;
        ensure!(
            opcode == SMSG_GOSSIP_MESSAGE,
            "expected SMSG_GOSSIP_MESSAGE after DB gossip hello, got 0x{opcode:04X}"
        );

        let mut select = Vec::with_capacity(12);
        select.extend_from_slice(&guid.raw().to_le_bytes());
        select.extend_from_slice(&1u32.to_le_bytes());
        write_client_packet(
            &mut self.stream,
            CMSG_GOSSIP_SELECT_OPTION,
            &select,
            Some(&mut self.crypto),
        )?;

        self.stream
            .set_read_timeout(Some(Duration::from_millis(250)))?;
        let read_result = read_server_packet(&mut self.stream, Some(&mut self.crypto));
        self.stream.set_read_timeout(Some(Duration::from_secs(5)))?;
        match read_result {
            Ok((opcode, _)) => anyhow::bail!(
                "invalid DB gossip option unexpectedly produced packet 0x{opcode:04X}"
            ),
            Err(error) => {
                if let Some(io_error) = error.downcast_ref::<std::io::Error>() {
                    ensure!(
                        matches!(io_error.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut),
                        "invalid DB gossip option failed with unexpected IO error: {io_error}"
                    );
                    Ok(())
                } else {
                    Err(error)
                }
            }
        }
    }

    fn expect_db_vendor_inventory_response(&mut self, guid: ObjectGuid) -> anyhow::Result<()> {
        let (opcode, body) = read_server_packet(&mut self.stream, Some(&mut self.crypto))?;
        ensure!(
            opcode == SMSG_LIST_INVENTORY,
            "expected DB SMSG_LIST_INVENTORY, got 0x{opcode:04X}"
        );
        ensure_available(&body, 37)?;
        ensure!(
            &body[0..8] == guid.raw().to_le_bytes().as_slice(),
            "DB vendor guid mismatch"
        );
        ensure!(body[8] == 1, "DB vendor item count was {}", body[8]);
        let item = u32::from_le_bytes(body[13..17].try_into()?);
        ensure!(
            item == RUST_VENDOR_STACK_ITEM,
            "DB vendor item was {item}, expected {RUST_VENDOR_STACK_ITEM}"
        );
        ensure!(
            item != RUST_VENDOR_BAG_ITEM,
            "DB vendor gossip list exposed filtered container item {RUST_VENDOR_BAG_ITEM}"
        );
        let available = u32::from_le_bytes(body[21..25].try_into()?);
        ensure!(
            available == u32::MAX,
            "DB vendor unlimited count marker was {available}"
        );
        Ok(())
    }

    fn query_db_creature_template(&mut self) -> anyhow::Result<()> {
        let guid = ObjectGuid::new(
            HighGuid::Unit,
            DB_CREATURE_FIXTURE_ENTRY,
            DB_CREATURE_FIXTURE_GUID,
        );
        let mut body = Vec::with_capacity(12);
        body.extend_from_slice(&DB_CREATURE_FIXTURE_ENTRY.to_le_bytes());
        body.extend_from_slice(&guid.raw().to_le_bytes());
        write_client_packet(
            &mut self.stream,
            CMSG_CREATURE_QUERY,
            &body,
            Some(&mut self.crypto),
        )?;
        let (opcode, body) = read_server_packet(&mut self.stream, Some(&mut self.crypto))?;
        ensure!(
            opcode == SMSG_CREATURE_QUERY_RESPONSE,
            "expected SMSG_CREATURE_QUERY_RESPONSE, got 0x{opcode:04X}"
        );
        let mut cursor = 0;
        ensure_available(&body, 4)?;
        let entry = u32::from_le_bytes(body[0..4].try_into()?);
        cursor += 4;
        let name = read_c_string(&body, &mut cursor)?;
        ensure!(
            entry == DB_CREATURE_FIXTURE_ENTRY,
            "creature query response entry was {entry}, expected {DB_CREATURE_FIXTURE_ENTRY}"
        );
        ensure!(
            name == DB_CREATURE_FIXTURE_NAME,
            "creature query response name was {name:?}, expected {DB_CREATURE_FIXTURE_NAME:?}"
        );
        Ok(())
    }

    fn buy_rust_guide_item(&mut self, item: u32, count: u8) -> anyhow::Result<()> {
        let mut body = Vec::with_capacity(14);
        body.extend_from_slice(&rust_guide_guid().raw().to_le_bytes());
        body.extend_from_slice(&item.to_le_bytes());
        body.push(count);
        body.push(1);
        write_client_packet(
            &mut self.stream,
            CMSG_BUY_ITEM,
            &body,
            Some(&mut self.crypto),
        )?;
        let (opcode, _) = read_server_packet(&mut self.stream, Some(&mut self.crypto))?;
        ensure!(
            opcode == SMSG_BUY_ITEM,
            "expected SMSG_BUY_ITEM after vendor buy, got 0x{opcode:04X}"
        );
        let (opcode, _) = read_server_packet(&mut self.stream, Some(&mut self.crypto))?;
        ensure!(
            opcode == SMSG_UPDATE_OBJECT,
            "expected SMSG_UPDATE_OBJECT after vendor buy, got 0x{opcode:04X}"
        );
        Ok(())
    }

    fn buy_db_vendor_item_expect_insufficient_money(
        &mut self,
        item: u32,
        count: u8,
    ) -> anyhow::Result<()> {
        self.buy_db_vendor_item_with_result(item, count, false)
    }

    fn buy_db_vendor_item(&mut self, item: u32, count: u8) -> anyhow::Result<()> {
        self.buy_db_vendor_item_with_result(item, count, true)
    }

    fn buy_db_vendor_item_with_result(
        &mut self,
        item: u32,
        count: u8,
        expect_success: bool,
    ) -> anyhow::Result<()> {
        let vendor_guid = ObjectGuid::new(
            HighGuid::Unit,
            DB_CREATURE_FIXTURE_ENTRY,
            DB_CREATURE_FIXTURE_GUID,
        );
        let mut body = Vec::with_capacity(14);
        body.extend_from_slice(&vendor_guid.raw().to_le_bytes());
        body.extend_from_slice(&item.to_le_bytes());
        body.push(count);
        body.push(1);
        write_client_packet(
            &mut self.stream,
            CMSG_BUY_ITEM,
            &body,
            Some(&mut self.crypto),
        )?;
        let (opcode, body) = read_server_packet(&mut self.stream, Some(&mut self.crypto))?;
        if expect_success {
            ensure!(
                opcode == SMSG_BUY_ITEM,
                "expected SMSG_BUY_ITEM after affordable DB vendor buy, got 0x{opcode:04X}"
            );
            let (opcode, _) = read_server_packet(&mut self.stream, Some(&mut self.crypto))?;
            ensure!(
                opcode == SMSG_UPDATE_OBJECT,
                "expected SMSG_UPDATE_OBJECT after affordable DB vendor buy item update, got 0x{opcode:04X}"
            );
            let (opcode, _) = read_server_packet(&mut self.stream, Some(&mut self.crypto))?;
            ensure!(
                opcode == SMSG_UPDATE_OBJECT,
                "expected SMSG_UPDATE_OBJECT after affordable DB vendor buy money update, got 0x{opcode:04X}"
            );
            return Ok(());
        }
        ensure!(
            opcode == SMSG_BUY_FAILED,
            "expected SMSG_BUY_FAILED after unaffordable DB vendor buy, got 0x{opcode:04X}"
        );
        ensure_available(&body, 13)?;
        ensure!(
            &body[0..8] == vendor_guid.raw().to_le_bytes().as_slice(),
            "buy failure vendor guid mismatch"
        );
        ensure!(
            u32::from_le_bytes(body[8..12].try_into()?) == item,
            "buy failure item mismatch"
        );
        ensure!(body[12] == 2, "buy failure code was {}", body[12]);
        Ok(())
    }

    fn sell_item(&mut self, item_guid: ObjectGuid, count: u8) -> anyhow::Result<()> {
        self.sell_item_to_vendor(rust_guide_guid(), item_guid, count)
    }

    fn sell_db_vendor_item(&mut self, item_guid: ObjectGuid, count: u8) -> anyhow::Result<()> {
        let vendor_guid = ObjectGuid::new(
            HighGuid::Unit,
            DB_CREATURE_FIXTURE_ENTRY,
            DB_CREATURE_FIXTURE_GUID,
        );
        self.sell_item_to_vendor(vendor_guid, item_guid, count)
    }

    fn sell_item_to_vendor(
        &mut self,
        vendor_guid: ObjectGuid,
        item_guid: ObjectGuid,
        count: u8,
    ) -> anyhow::Result<()> {
        let mut body = Vec::with_capacity(17);
        body.extend_from_slice(&vendor_guid.raw().to_le_bytes());
        body.extend_from_slice(&item_guid.raw().to_le_bytes());
        body.push(count);
        write_client_packet(
            &mut self.stream,
            CMSG_SELL_ITEM,
            &body,
            Some(&mut self.crypto),
        )?;
        let (opcode, _) = read_server_packet(&mut self.stream, Some(&mut self.crypto))?;
        ensure!(
            opcode == SMSG_UPDATE_OBJECT || opcode == SMSG_DESTROY_OBJECT,
            "expected item/count update after vendor sell, got 0x{opcode:04X}"
        );
        let (opcode, _) = read_server_packet(&mut self.stream, Some(&mut self.crypto))?;
        ensure!(
            opcode == SMSG_UPDATE_OBJECT,
            "expected SMSG_UPDATE_OBJECT after vendor sell money update, got 0x{opcode:04X}"
        );
        Ok(())
    }

    fn destroy_backpack_item(&mut self, slot: u8) -> anyhow::Result<()> {
        self.destroy_bag0_item(slot)
    }

    fn destroy_bag0_item(&mut self, slot: u8) -> anyhow::Result<()> {
        self.destroy_item(255, slot, 0, SMSG_UPDATE_OBJECT)
    }

    fn destroy_item(
        &mut self,
        bag: u8,
        slot: u8,
        count: u8,
        expected_opcode: u32,
    ) -> anyhow::Result<()> {
        write_client_packet(
            &mut self.stream,
            CMSG_DESTROYITEM,
            &[bag, slot, count, 0, 0, 0],
            Some(&mut self.crypto),
        )?;
        let (opcode, _) = read_server_packet(&mut self.stream, Some(&mut self.crypto))?;
        ensure!(
            opcode == expected_opcode,
            "expected 0x{expected_opcode:04X} after item destroy, got 0x{opcode:04X}"
        );
        Ok(())
    }

    fn expect_destroy_failure(
        &mut self,
        bag: u8,
        slot: u8,
        expected_error: u8,
    ) -> anyhow::Result<()> {
        write_client_packet(
            &mut self.stream,
            CMSG_DESTROYITEM,
            &[bag, slot, 0, 0, 0, 0],
            Some(&mut self.crypto),
        )?;
        let (opcode, body) = read_server_packet(&mut self.stream, Some(&mut self.crypto))?;
        ensure!(
            opcode == SMSG_INVENTORY_CHANGE_FAILURE,
            "expected SMSG_INVENTORY_CHANGE_FAILURE after rejected item destroy, got 0x{opcode:04X}"
        );
        ensure!(
            body.first().copied() == Some(expected_error),
            "destroy failure error was {:?}, expected {expected_error}",
            body.first()
        );
        Ok(())
    }

    fn split_item(
        &mut self,
        src_bag: u8,
        src_slot: u8,
        dst_bag: u8,
        dst_slot: u8,
        count: u8,
    ) -> anyhow::Result<()> {
        write_client_packet(
            &mut self.stream,
            CMSG_SPLIT_ITEM,
            &[src_bag, src_slot, dst_bag, dst_slot, count],
            Some(&mut self.crypto),
        )?;
        let (opcode, _) = read_server_packet(&mut self.stream, Some(&mut self.crypto))?;
        ensure!(
            opcode == SMSG_UPDATE_OBJECT,
            "expected SMSG_UPDATE_OBJECT after item split, got 0x{opcode:04X}"
        );
        Ok(())
    }

    fn kill_combat_dummy_with_autoattacks(&mut self) -> anyhow::Result<()> {
        let target = rust_combat_dummy_guid();
        for swing_index in 0..3 {
            write_client_packet(
                &mut self.stream,
                CMSG_ATTACKSWING,
                &target.raw().to_le_bytes(),
                Some(&mut self.crypto),
            )?;
            for expected in [
                SMSG_ATTACKSTART,
                SMSG_ATTACKERSTATEUPDATE,
                SMSG_UPDATE_OBJECT,
                SMSG_UPDATE_OBJECT,
            ] {
                let (opcode, _) = read_server_packet(&mut self.stream, Some(&mut self.crypto))?;
                ensure!(
                    opcode == expected,
                    "expected 0x{expected:04X} during combat dummy swing, got 0x{opcode:04X}"
                );
            }
            if swing_index == 2 {
                for expected in [SMSG_ATTACKSTOP, SMSG_UPDATE_OBJECT] {
                    let (opcode, _) = read_server_packet(&mut self.stream, Some(&mut self.crypto))?;
                    ensure!(
                        opcode == expected,
                        "expected 0x{expected:04X} after combat dummy death, got 0x{opcode:04X}"
                    );
                }
            }
        }
        Ok(())
    }

    fn open_combat_dummy_loot(&mut self) -> anyhow::Result<()> {
        let target = rust_combat_dummy_guid();
        write_client_packet(
            &mut self.stream,
            CMSG_LOOT,
            &target.raw().to_le_bytes(),
            Some(&mut self.crypto),
        )?;
        let (opcode, body) = read_server_packet(&mut self.stream, Some(&mut self.crypto))?;
        ensure!(
            opcode == SMSG_LOOT_RESPONSE,
            "expected SMSG_LOOT_RESPONSE, got 0x{opcode:04X}"
        );
        ensure_available(&body, 14)?;
        ensure!(
            &body[0..8] == target.raw().to_le_bytes().as_slice(),
            "loot response target guid mismatch"
        );
        ensure!(
            body[13] == 1,
            "combat dummy loot item count was {}",
            body[13]
        );
        Ok(())
    }

    fn autostore_combat_dummy_loot_item(&mut self) -> anyhow::Result<()> {
        write_client_packet(
            &mut self.stream,
            CMSG_AUTOSTORE_LOOT_ITEM,
            &[0],
            Some(&mut self.crypto),
        )?;
        let (opcode, body) = read_server_packet(&mut self.stream, Some(&mut self.crypto))?;
        ensure!(
            opcode == SMSG_LOOT_REMOVED,
            "expected SMSG_LOOT_REMOVED, got 0x{opcode:04X}"
        );
        ensure!(body == [0], "loot removed body was {body:02X?}");
        let (opcode, _) = read_server_packet(&mut self.stream, Some(&mut self.crypto))?;
        ensure!(
            opcode == SMSG_UPDATE_OBJECT,
            "expected SMSG_UPDATE_OBJECT after autostore loot, got 0x{opcode:04X}"
        );
        Ok(())
    }

    fn autostore_combat_dummy_loot_item_expect_no_space(&mut self) -> anyhow::Result<()> {
        write_client_packet(
            &mut self.stream,
            CMSG_AUTOSTORE_LOOT_ITEM,
            &[0],
            Some(&mut self.crypto),
        )?;
        let (opcode, body) = read_server_packet(&mut self.stream, Some(&mut self.crypto))?;
        ensure!(
            opcode == SMSG_INVENTORY_CHANGE_FAILURE,
            "expected SMSG_INVENTORY_CHANGE_FAILURE for no-space autostore, got 0x{opcode:04X}"
        );
        ensure_available(&body, 18)?;
        ensure!(
            body[0] == EQUIP_ERR_COULDNT_SPLIT_ITEMS,
            "no-space autostore failure code was {}, expected {EQUIP_ERR_COULDNT_SPLIT_ITEMS}",
            body[0]
        );
        Ok(())
    }

    fn release_combat_dummy_loot(&mut self) -> anyhow::Result<()> {
        let target = rust_combat_dummy_guid();
        write_client_packet(
            &mut self.stream,
            CMSG_LOOT_RELEASE,
            &target.raw().to_le_bytes(),
            Some(&mut self.crypto),
        )?;
        let (opcode, body) = read_server_packet(&mut self.stream, Some(&mut self.crypto))?;
        ensure!(
            opcode == SMSG_LOOT_RELEASE_RESPONSE,
            "expected SMSG_LOOT_RELEASE_RESPONSE, got 0x{opcode:04X}"
        );
        ensure_available(&body, 9)?;
        ensure!(
            &body[0..8] == target.raw().to_le_bytes().as_slice(),
            "loot release target guid mismatch"
        );
        ensure!(body[8] == 1, "loot release success byte was {}", body[8]);
        let (opcode, _) = read_server_packet(&mut self.stream, Some(&mut self.crypto))?;
        ensure!(
            opcode == SMSG_UPDATE_OBJECT,
            "expected SMSG_UPDATE_OBJECT after loot release, got 0x{opcode:04X}"
        );
        Ok(())
    }

    fn cast_heroic_strike_at_combat_dummy(&mut self) -> anyhow::Result<()> {
        let target = rust_combat_dummy_guid();
        let mut body = Vec::new();
        body.extend_from_slice(&WARRIOR_HEROIC_STRIKE_RANK_1.to_le_bytes());
        body.extend_from_slice(&SPELL_CAST_TARGET_UNIT.to_le_bytes());
        PackedGuid::write(&mut body, target)?;
        write_client_packet(
            &mut self.stream,
            CMSG_CAST_SPELL,
            &body,
            Some(&mut self.crypto),
        )?;

        let (opcode, body) = read_server_packet(&mut self.stream, Some(&mut self.crypto))?;
        ensure!(
            opcode == SMSG_CAST_RESULT,
            "expected SMSG_CAST_RESULT for Heroic Strike, got 0x{opcode:04X}"
        );
        ensure_available(&body, 5)?;
        ensure!(
            &body[0..4] == WARRIOR_HEROIC_STRIKE_RANK_1.to_le_bytes().as_slice(),
            "Heroic Strike cast result returned spell bytes {:02X?}",
            &body[0..4]
        );
        ensure!(body[4] == 0, "Heroic Strike cast result was {}", body[4]);

        for expected in [
            SMSG_SPELL_GO,
            SMSG_ATTACKERSTATEUPDATE,
            SMSG_UPDATE_OBJECT,
            SMSG_UPDATE_OBJECT,
        ] {
            let (opcode, _) = read_server_packet(&mut self.stream, Some(&mut self.crypto))?;
            ensure!(
                opcode == expected,
                "expected 0x{expected:04X} during Heroic Strike fixture cast, got 0x{opcode:04X}"
            );
        }
        Ok(())
    }

    fn logout(&mut self) -> anyhow::Result<()> {
        write_client_packet(
            &mut self.stream,
            CMSG_LOGOUT_REQUEST,
            &[],
            Some(&mut self.crypto),
        )?;
        for _ in 0..2 {
            let _ = read_server_packet(&mut self.stream, Some(&mut self.crypto))?;
        }
        Ok(())
    }

    fn expect_create_result(
        &mut self,
        name: &str,
        attributes: [u8; 9],
        expected: u8,
    ) -> anyhow::Result<()> {
        let mut body = Vec::new();
        body.extend_from_slice(name.as_bytes());
        body.push(0);
        body.extend_from_slice(&attributes);
        write_client_packet(
            &mut self.stream,
            CMSG_CHAR_CREATE,
            &body,
            Some(&mut self.crypto),
        )?;
        let (opcode, body) = read_server_packet(&mut self.stream, Some(&mut self.crypto))?;
        ensure!(opcode == SMSG_CHAR_CREATE, "expected SMSG_CHAR_CREATE");
        ensure!(
            body == [expected],
            "character create returned {:02X?}, expected 0x{expected:02X}",
            body
        );
        Ok(())
    }

    fn expect_delete_character_result(&mut self, guid: u32, expected: u8) -> anyhow::Result<()> {
        let guid = ObjectGuid::new(HighGuid::Player, 0, guid);
        self.expect_delete_body_result(&guid.raw().to_le_bytes(), expected)
    }

    fn expect_delete_body_result(&mut self, body: &[u8], expected: u8) -> anyhow::Result<()> {
        write_client_packet(
            &mut self.stream,
            CMSG_CHAR_DELETE,
            body,
            Some(&mut self.crypto),
        )?;
        let (opcode, body) = read_server_packet(&mut self.stream, Some(&mut self.crypto))?;
        ensure!(opcode == SMSG_CHAR_DELETE, "expected SMSG_CHAR_DELETE");
        ensure!(
            body == [expected],
            "character delete returned {:02X?}, expected 0x{expected:02X}",
            body
        );
        Ok(())
    }
}

fn human_warrior_attributes() -> [u8; 9] {
    [1, 1, 0, 0, 0, 0, 0, 0, 0]
}

fn rust_guide_guid() -> ObjectGuid {
    ObjectGuid::new(HighGuid::Unit, RUST_GUIDE_ENTRY, RUST_GUIDE_COUNTER)
}

fn rust_combat_dummy_guid() -> ObjectGuid {
    ObjectGuid::new(
        HighGuid::Unit,
        RUST_COMBAT_DUMMY_ENTRY,
        RUST_COMBAT_DUMMY_COUNTER,
    )
}

#[derive(Debug)]
struct EnumCharacter {
    guid: u32,
    name: String,
}

fn parse_char_enum(body: &[u8]) -> anyhow::Result<Vec<EnumCharacter>> {
    ensure!(!body.is_empty(), "empty SMSG_CHAR_ENUM body");
    let count = body[0] as usize;
    let mut cursor = 1;
    let mut characters = Vec::with_capacity(count);

    for _ in 0..count {
        ensure_available(body, cursor + 8)?;
        let raw_guid = u64::from_le_bytes(body[cursor..cursor + 8].try_into()?);
        cursor += 8;

        let name_end = body[cursor..]
            .iter()
            .position(|b| *b == 0)
            .ok_or_else(|| anyhow::anyhow!("character enum name is not NUL-terminated"))?
            + cursor;
        let name = String::from_utf8(body[cursor..name_end].to_vec())?;
        cursor = name_end + 1;

        ensure_available(
            body,
            cursor + 3 + 5 + 1 + 4 + 4 + 12 + 4 + 4 + 1 + 12 + 20 * 5,
        )?;
        cursor += 3; // race, class, gender
        cursor += 5; // appearance bytes
        cursor += 1; // level
        cursor += 4; // zone
        cursor += 4; // map
        cursor += 12; // position
        cursor += 4; // guild
        cursor += 4; // flags
        cursor += 1; // first login
        cursor += 12; // pet display, level, family
        cursor += 20 * 5; // equipment display id + inventory type

        characters.push(EnumCharacter {
            guid: ObjectGuid::from_raw(raw_guid).counter(),
            name,
        });
    }

    ensure!(
        cursor == body.len(),
        "SMSG_CHAR_ENUM had trailing bytes: parsed {cursor}, len {}",
        body.len()
    );
    Ok(characters)
}

fn auth_session_body(session_key: &[u8; 40], server_seed: u32) -> Vec<u8> {
    let mut hasher = Sha1::new();
    hasher.update(USERNAME.as_bytes());
    hasher.update(0u32.to_le_bytes());
    hasher.update(CLIENT_SEED.to_le_bytes());
    hasher.update(server_seed.to_le_bytes());
    hasher.update(session_key);
    let digest: [u8; 20] = hasher.finalize().into();

    let mut body = Vec::new();
    body.extend_from_slice(&(BUILD_1121 as u32).to_le_bytes());
    body.extend_from_slice(&0u32.to_le_bytes());
    body.extend_from_slice(USERNAME.as_bytes());
    body.push(0);
    body.extend_from_slice(&CLIENT_SEED.to_le_bytes());
    body.extend_from_slice(&digest);
    body
}

fn write_client_packet(
    stream: &mut TcpStream,
    opcode: u32,
    body: &[u8],
    crypto: Option<&mut HeaderCrypto>,
) -> anyhow::Result<()> {
    let size = (body.len() + 4) as u16;
    let mut header = [0u8; 6];
    header[0..2].copy_from_slice(&size.to_be_bytes());
    header[2..6].copy_from_slice(&opcode.to_le_bytes());
    if let Some(crypto) = crypto {
        crypto.encrypt(&mut header);
    }
    stream.write_all(&header)?;
    stream.write_all(body)?;
    Ok(())
}

fn read_server_packet(
    stream: &mut TcpStream,
    crypto: Option<&mut HeaderCrypto>,
) -> anyhow::Result<(u32, Vec<u8>)> {
    let mut header = [0u8; 4];
    stream.read_exact(&mut header)?;
    if let Some(crypto) = crypto {
        crypto.decrypt(&mut header);
    }

    let size = u16::from_be_bytes([header[0], header[1]]) as usize;
    let opcode = u16::from_le_bytes([header[2], header[3]]) as u32;
    ensure!(
        (2..=0x2800).contains(&size),
        "malformed server packet size {size}"
    );
    let body_len = size - 2;
    let body = read_exact_vec(stream, body_len)?;
    Ok((opcode, body))
}

async fn assert_count_row(
    login_pool: &MySqlPool,
    account_id: u32,
    expected: u8,
) -> anyhow::Result<()> {
    let actual: Option<u8> =
        sqlx::query_scalar("SELECT numchars FROM realmcharacters WHERE realmid = ? AND acctid = ?")
            .bind(REALM_ID)
            .bind(account_id)
            .fetch_optional(login_pool)
            .await?;
    ensure!(
        actual == Some(expected),
        "realmcharacters count for acctid={account_id} was {:?}, expected {expected}",
        actual
    );
    Ok(())
}

fn connect_blocking(addr: &str) -> anyhow::Result<TcpStream> {
    let stream = TcpStream::connect(addr).with_context(|| format!("connect to {addr}"))?;
    stream.set_read_timeout(Some(Duration::from_secs(5)))?;
    stream.set_write_timeout(Some(Duration::from_secs(5)))?;
    Ok(stream)
}

fn logon_challenge_request() -> Vec<u8> {
    let request = LogonChallengeRequest {
        cmd: AuthCommand::LogonChallenge,
        error: 0,
        size: 30 + USERNAME.len() as u16,
        game_name: *b"WoW\0",
        version_major: 1,
        version_minor: 12,
        version_patch: 1,
        build: BUILD_1121,
        platform: *b"x86\0",
        os: *b"Win\0",
        country: *b"enUS",
        timezone_bias: 0,
        ip: [127, 0, 0, 1],
        account_name: USERNAME.to_string(),
    };

    let mut bytes = BytesMut::new();
    request.write(&mut bytes);
    bytes.to_vec()
}

fn read_exact_vec(stream: &mut TcpStream, len: usize) -> anyhow::Result<Vec<u8>> {
    let mut bytes = vec![0u8; len];
    stream.read_exact(&mut bytes)?;
    Ok(bytes)
}

fn ensure_available(body: &[u8], end: usize) -> anyhow::Result<()> {
    ensure!(
        end <= body.len(),
        "packet truncated: need {end} bytes, got {}",
        body.len()
    );
    Ok(())
}

fn read_c_string(body: &[u8], cursor: &mut usize) -> anyhow::Result<String> {
    let end = body[*cursor..]
        .iter()
        .position(|b| *b == 0)
        .ok_or_else(|| anyhow::anyhow!("string is not NUL-terminated"))?
        + *cursor;
    let value = String::from_utf8(body[*cursor..end].to_vec())?;
    *cursor = end + 1;
    Ok(value)
}

fn bytes_to_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn hex_to_array40(hex: &str) -> anyhow::Result<[u8; 40]> {
    let bytes = hex_to_vec(hex)?;
    ensure!(bytes.len() == 40, "expected 40-byte session key");
    let mut out = [0u8; 40];
    out.copy_from_slice(&bytes);
    Ok(out)
}

fn hex_to_vec(hex: &str) -> anyhow::Result<Vec<u8>> {
    let hex = hex.trim();
    ensure!(hex.len().is_multiple_of(2), "hex string has odd length");
    let mut out = Vec::with_capacity(hex.len() / 2);
    for chunk in hex.as_bytes().chunks(2) {
        out.push((hex_nibble(chunk[0])? << 4) | hex_nibble(chunk[1])?);
    }
    Ok(out)
}

fn hex_nibble(c: u8) -> anyhow::Result<u8> {
    match c {
        b'0'..=b'9' => Ok(c - b'0'),
        b'a'..=b'f' => Ok(c - b'a' + 10),
        b'A'..=b'F' => Ok(c - b'A' + 10),
        _ => anyhow::bail!("invalid hex character 0x{c:02X}"),
    }
}
