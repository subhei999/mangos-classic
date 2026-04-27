use anyhow::{ensure, Context};
use bytes::BytesMut;
use sha1::{Digest, Sha1};
use sqlx::mysql::{MySqlPool, MySqlPoolOptions};
use std::io::{Read, Write};
use std::net::TcpStream;
use std::time::Duration;
use wow_common::guid::{HighGuid, ObjectGuid};
use wow_common::position::WorldPosition;
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
const USERNAME: &str = "STARTZONE";
const PASSWORD: &str = "STARTPASS";
const CHARACTER_NAME: &str = "Startzone";
const BUILD_1121: u16 = 5875;
const CLIENT_SEED: u32 = 0x5A17_0201;
const REALM_ID: u32 = 1;
const NORTHSHIRE_ZONE: u32 = 12;
const ELWYNN_FOREST_ZONE: u32 = 12;
const EASTERN_KINGDOMS_MAP: u32 = 0;
const ALLIANCE_FACTION: u32 = 469;
const HUMAN_START_X: f32 = -8949.95;
const HUMAN_START_Y: f32 = -132.493;
const REAL_MARSHAL_MCBRIDE_ENTRY: u32 = 197;
const REAL_DEPUTY_WILLEM_ENTRY: u32 = 823;
const REAL_BROTHER_PAXTON_ENTRY: u32 = 951;
const REAL_YOUNG_WOLF_ENTRY: u32 = 299;
const REAL_KOBOLD_VERMIN_ENTRY: u32 = 6;
const REAL_NORTHSHIRE_VISIBLE_RADIUS_YARDS: f32 = 220.0;
const REAL_NORTHSHIRE_VISIBLE_LIMIT: u32 = 128;
const FIXTURE_PREFIX: u32 = 910_000;
const MARSHAL_MCBRIDE_ENTRY: u32 = FIXTURE_PREFIX + 1;
const DEPUTY_WILLEM_ENTRY: u32 = FIXTURE_PREFIX + 2;
const BROTHER_PAXTON_ENTRY: u32 = FIXTURE_PREFIX + 3;
const YOUNG_WOLF_ENTRY: u32 = FIXTURE_PREFIX + 4;
const KOBOLD_VERMIN_ENTRY: u32 = FIXTURE_PREFIX + 5;
const YOUNG_WOLF_DISPLAY_ID: u32 = 372;
const KOBOLD_VERMIN_DISPLAY_ID: u32 = 365;
const NORTHSHIRE_CRATE_ENTRY: u32 = FIXTURE_PREFIX + 101;
const A_THREAT_WITHIN_QUEST: u32 = FIXTURE_PREFIX + 201;
const KOBOLD_CAMP_CLEANUP_QUEST: u32 = FIXTURE_PREFIX + 202;
const FIXTURE_GRAVEYARD_ID: u32 = FIXTURE_PREFIX + 301;
const CMSG_CHAR_ENUM: u32 = 0x0037;
const CMSG_PLAYER_LOGIN: u32 = 0x003D;
const CMSG_ATTACKSWING: u32 = 0x0141;
const CMSG_LOOT: u32 = 0x015D;
const CMSG_LOOT_MONEY: u32 = 0x015E;
const CMSG_LOOT_RELEASE: u32 = 0x015F;
const CMSG_AUTOSTORE_LOOT_ITEM: u32 = 0x0108;
const CMSG_AUTH_SESSION: u32 = 0x01ED;
const SMSG_CHAR_ENUM: u32 = 0x003B;
const SMSG_UPDATE_OBJECT: u32 = 0x00A9;
const SMSG_ATTACKERSTATEUPDATE: u32 = 0x014A;
const SMSG_LOOT_RESPONSE: u32 = 0x0160;
const SMSG_LOOT_RELEASE_RESPONSE: u32 = 0x0161;
const SMSG_AUTH_CHALLENGE: u32 = 0x01EC;
const SMSG_AUTH_RESPONSE: u32 = 0x01EE;
const AUTH_OK: u8 = 0x0C;
const UNIT_FIELD_HEALTH: usize = 0x016;
const UNIT_DYNAMIC_FLAGS: usize = 0x08F;
const UNIT_DYNFLAG_LOOTABLE: u32 = 0x0000_0001;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StarterZoneSource {
    RealClassicDb,
    Fixture,
}

#[derive(Debug, Clone)]
struct ExpectedCreature {
    entry: u32,
    counter: u32,
}

#[derive(Debug, Clone)]
struct StarterZoneContent {
    source: StarterZoneSource,
    visible_creatures: Vec<ExpectedCreature>,
    wolf: ExpectedCreature,
    wolf_health: u32,
    wolf_loot_money: u32,
    wolf_loot_item: Option<u32>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let login_pool = connect(LOGIN_DATABASE_URL).await?;
    let character_pool = connect(CHARACTER_DATABASE_URL).await?;
    let world_pool = connect(WORLD_DATABASE_URL).await?;

    let account_id = seed_account(&login_pool).await?;
    cleanup_account_characters(&character_pool, account_id).await?;
    let starter_zone = prepare_northshire_content(&world_pool).await?;

    let created = wow_db::create_character(
        &character_pool,
        &world_pool,
        wow_db::NewCharacter {
            account_id,
            name: CHARACTER_NAME.to_string(),
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
    wow_db::refresh_realm_character_count(&login_pool, &character_pool, account_id, REALM_ID)
        .await?;

    assert_human_warrior_enters_northshire(&character_pool, account_id, created.guid).await?;
    assert_starter_zone_creatures_available(&world_pool, created.position, &starter_zone).await?;
    match starter_zone.source {
        StarterZoneSource::RealClassicDb => assert_real_starter_zone_rows(&world_pool).await?,
        StarterZoneSource::Fixture => {
            assert_starter_zone_interaction_rows(&world_pool).await?;
            assert_starter_zone_loot_and_quest_rows(&world_pool).await?;
            assert_starter_zone_gameobject_and_graveyard_rows(&world_pool).await?;
        }
    }
    assert_count_row(&login_pool, account_id, 1).await?;

    complete_auth_flow()?;
    let session_key = fetch_session_key(&login_pool).await?;
    let mut world = WorldClient::connect(&session_key)?;
    let enum_rows = world.char_enum()?;
    ensure!(
        enum_rows
            .iter()
            .any(|character| character.guid == created.guid && character.name == CHARACTER_NAME),
        "fresh Northshire character was missing from SMSG_CHAR_ENUM"
    );
    world.login_character_expect_northshire_creatures(created.guid, &starter_zone)?;
    world.kill_loot_and_respawn_young_wolf(&starter_zone)?;

    println!(
        "starter-zone {:?} lock passed for account {USERNAME}, character {CHARACTER_NAME}",
        starter_zone.source
    );
    Ok(())
}

async fn connect(url: &str) -> anyhow::Result<MySqlPool> {
    MySqlPoolOptions::new()
        .max_connections(5)
        .connect(url)
        .await
        .with_context(|| format!("connect to {url}"))
}

async fn seed_account(login_pool: &MySqlPool) -> anyhow::Result<u32> {
    let verifier = SrpVerifier::from_username_and_password(
        NormalizedString::new(USERNAME)?,
        NormalizedString::new(PASSWORD)?,
    );
    sqlx::query(
        "DELETE FROM realmcharacters WHERE acctid IN (SELECT id FROM account WHERE username = ?)",
    )
    .bind(USERNAME)
    .execute(login_pool)
    .await?;
    sqlx::query("DELETE FROM account WHERE username = ?")
        .bind(USERNAME)
        .execute(login_pool)
        .await?;
    sqlx::query(
        "INSERT INTO account (username, gmlevel, sessionkey, v, s, email, locked, expansion, locale, os) \
         VALUES (?, 0, '', ?, ?, '', 0, 0, '', 'Win')",
    )
    .bind(USERNAME)
    .bind(bytes_to_hex(verifier.password_verifier()))
    .bind(bytes_to_hex(verifier.salt()))
    .execute(login_pool)
    .await?;

    let account_id: u32 = sqlx::query_scalar("SELECT id FROM account WHERE username = ?")
        .bind(USERNAME)
        .fetch_one(login_pool)
        .await?;
    Ok(account_id)
}

async fn cleanup_account_characters(
    character_pool: &MySqlPool,
    account_id: u32,
) -> anyhow::Result<()> {
    let guids: Vec<u32> = sqlx::query_scalar("SELECT guid FROM characters WHERE account = ?")
        .bind(account_id)
        .fetch_all(character_pool)
        .await?;
    for guid in guids {
        let deleted = wow_db::delete_character(character_pool, account_id, guid).await?;
        ensure!(
            deleted,
            "failed to delete stale starter-zone character {guid}"
        );
    }
    Ok(())
}

async fn prepare_northshire_content(world_pool: &MySqlPool) -> anyhow::Result<StarterZoneContent> {
    cleanup_northshire_fixture(world_pool).await?;
    if let Some(real) = load_real_northshire_content(world_pool).await? {
        return Ok(real);
    }

    seed_northshire_fixture(world_pool).await?;
    load_fixture_northshire_content(world_pool).await
}

async fn load_real_northshire_content(
    world_pool: &MySqlPool,
) -> anyhow::Result<Option<StarterZoneContent>> {
    let nearby = wow_db::get_nearby_creature_spawns(
        world_pool,
        EASTERN_KINGDOMS_MAP,
        HUMAN_START_X,
        HUMAN_START_Y,
        220.0,
        128,
    )
    .await?;
    let visible = wow_db::get_nearby_creature_spawns(
        world_pool,
        EASTERN_KINGDOMS_MAP,
        HUMAN_START_X,
        HUMAN_START_Y,
        REAL_NORTHSHIRE_VISIBLE_RADIUS_YARDS,
        REAL_NORTHSHIRE_VISIBLE_LIMIT,
    )
    .await?;

    let required_nearby = [
        REAL_MARSHAL_MCBRIDE_ENTRY,
        REAL_DEPUTY_WILLEM_ENTRY,
        REAL_BROTHER_PAXTON_ENTRY,
        REAL_YOUNG_WOLF_ENTRY,
        REAL_KOBOLD_VERMIN_ENTRY,
    ];
    if !required_nearby
        .iter()
        .all(|entry| nearby.iter().any(|spawn| spawn.entry == *entry))
    {
        return Ok(None);
    }

    let required_visible = [
        REAL_MARSHAL_MCBRIDE_ENTRY,
        REAL_DEPUTY_WILLEM_ENTRY,
        REAL_BROTHER_PAXTON_ENTRY,
        REAL_YOUNG_WOLF_ENTRY,
        REAL_KOBOLD_VERMIN_ENTRY,
    ];
    ensure!(
        required_visible
            .iter()
            .all(|entry| visible.iter().any(|spawn| spawn.entry == *entry)),
        "real ClassicDB Northshire core NPC/wolf rows exist, but not all are visible within the current worldserver spawn radius"
    );

    let wolf_spawn = visible
        .iter()
        .find(|spawn| spawn.entry == REAL_YOUNG_WOLF_ENTRY)
        .context("real ClassicDB Young Wolf was not visible near the Human Warrior start")?;
    let wolf_loot = wow_db::get_creature_loot_items(world_pool, REAL_YOUNG_WOLF_ENTRY)
        .await?
        .into_iter()
        .next();

    Ok(Some(StarterZoneContent {
        source: StarterZoneSource::RealClassicDb,
        visible_creatures: required_visible
            .iter()
            .map(|entry| {
                let spawn = visible
                    .iter()
                    .find(|spawn| spawn.entry == *entry)
                    .expect("required visible spawn checked above");
                ExpectedCreature {
                    entry: spawn.entry,
                    counter: spawn.guid,
                }
            })
            .collect(),
        wolf: ExpectedCreature {
            entry: wolf_spawn.entry,
            counter: wolf_spawn.guid,
        },
        wolf_health: wolf_spawn.template.max_level_health,
        wolf_loot_money: wolf_spawn
            .template
            .max_loot_gold
            .max(wolf_spawn.template.min_loot_gold),
        wolf_loot_item: wolf_loot.map(|loot| loot.item),
    }))
}

async fn load_fixture_northshire_content(
    world_pool: &MySqlPool,
) -> anyhow::Result<StarterZoneContent> {
    let wolf_loot = wow_db::get_creature_loot_items(world_pool, YOUNG_WOLF_ENTRY)
        .await?
        .into_iter()
        .next();
    Ok(StarterZoneContent {
        source: StarterZoneSource::Fixture,
        visible_creatures: vec![
            ExpectedCreature {
                entry: MARSHAL_MCBRIDE_ENTRY,
                counter: FIXTURE_PREFIX + 1,
            },
            ExpectedCreature {
                entry: DEPUTY_WILLEM_ENTRY,
                counter: FIXTURE_PREFIX + 2,
            },
            ExpectedCreature {
                entry: BROTHER_PAXTON_ENTRY,
                counter: FIXTURE_PREFIX + 3,
            },
            ExpectedCreature {
                entry: YOUNG_WOLF_ENTRY,
                counter: FIXTURE_PREFIX + 4,
            },
            ExpectedCreature {
                entry: KOBOLD_VERMIN_ENTRY,
                counter: FIXTURE_PREFIX + 5,
            },
        ],
        wolf: ExpectedCreature {
            entry: YOUNG_WOLF_ENTRY,
            counter: FIXTURE_PREFIX + 4,
        },
        wolf_health: 4,
        wolf_loot_money: 3,
        wolf_loot_item: wolf_loot.map(|loot| loot.item),
    })
}

async fn seed_northshire_fixture(world_pool: &MySqlPool) -> anyhow::Result<()> {
    seed_creature_template(
        world_pool,
        MARSHAL_MCBRIDE_ENTRY,
        "Rust Marshal McBride",
        "Northshire Fixture",
        5,
        49,
        35,
        7,
        0x0000_0003,
        40,
        3.0,
        5.0,
        0,
        0,
        0,
        0,
    )
    .await?;
    seed_creature_template(
        world_pool,
        DEPUTY_WILLEM_ENTRY,
        "Rust Deputy Willem",
        "Northshire Fixture",
        5,
        51,
        35,
        7,
        0x0000_0083,
        40,
        3.0,
        5.0,
        0,
        0,
        0,
        0,
    )
    .await?;
    seed_creature_template(
        world_pool,
        BROTHER_PAXTON_ENTRY,
        "Rust Brother Paxton",
        "Warrior Trainer Fixture",
        5,
        53,
        35,
        7,
        0x0000_0013,
        40,
        3.0,
        5.0,
        1,
        1,
        0,
        0,
    )
    .await?;
    seed_creature_template(
        world_pool,
        YOUNG_WOLF_ENTRY,
        "Rust Young Wolf",
        "",
        1,
        YOUNG_WOLF_DISPLAY_ID,
        14,
        1,
        0,
        4,
        1.0,
        4.0,
        0,
        0,
        1,
        3,
    )
    .await?;
    seed_creature_template(
        world_pool,
        KOBOLD_VERMIN_ENTRY,
        "Rust Kobold Vermin",
        "",
        2,
        KOBOLD_VERMIN_DISPLAY_ID,
        14,
        7,
        0,
        35,
        1.0,
        2.0,
        0,
        0,
        0,
        0,
    )
    .await?;

    for (guid, entry, x, y, z, orientation) in [
        (
            FIXTURE_PREFIX + 1,
            MARSHAL_MCBRIDE_ENTRY,
            -8933.3,
            -136.2,
            82.1,
            5.2,
        ),
        (
            FIXTURE_PREFIX + 2,
            DEPUTY_WILLEM_ENTRY,
            -8921.8,
            -119.1,
            82.0,
            2.9,
        ),
        (
            FIXTURE_PREFIX + 3,
            BROTHER_PAXTON_ENTRY,
            -8942.5,
            -150.0,
            83.5,
            0.2,
        ),
        (
            FIXTURE_PREFIX + 4,
            YOUNG_WOLF_ENTRY,
            -8908.0,
            -145.0,
            82.2,
            3.4,
        ),
        (
            FIXTURE_PREFIX + 5,
            KOBOLD_VERMIN_ENTRY,
            -8897.0,
            -121.0,
            81.9,
            3.4,
        ),
    ] {
        seed_creature_spawn(world_pool, guid, entry, x, y, z, orientation).await?;
    }

    sqlx::query(
        "INSERT INTO npc_vendor (entry, item, maxcount, incrtime, slot, condition_id) \
         VALUES (?, 117, 0, 0, 1, 0)",
    )
    .bind(DEPUTY_WILLEM_ENTRY)
    .execute(world_pool)
    .await?;
    sqlx::query(
        "INSERT INTO npc_trainer (entry, spell, spellcost, reqskill, reqskillvalue, reqlevel, condition_id) \
         VALUES (?, 772, 10, 0, 0, 4, 0)",
    )
    .bind(BROTHER_PAXTON_ENTRY)
    .execute(world_pool)
    .await?;

    seed_quest_template(
        world_pool,
        A_THREAT_WITHIN_QUEST,
        "Rust A Threat Within",
        0,
        0,
        0,
        0,
    )
    .await?;
    seed_quest_template(
        world_pool,
        KOBOLD_CAMP_CLEANUP_QUEST,
        "Rust Kobold Camp Cleanup",
        KOBOLD_VERMIN_ENTRY as i32,
        5,
        0,
        0,
    )
    .await?;
    sqlx::query("INSERT INTO creature_questrelation (id, quest) VALUES (?, ?), (?, ?)")
        .bind(MARSHAL_MCBRIDE_ENTRY)
        .bind(A_THREAT_WITHIN_QUEST)
        .bind(DEPUTY_WILLEM_ENTRY)
        .bind(KOBOLD_CAMP_CLEANUP_QUEST)
        .execute(world_pool)
        .await?;
    sqlx::query("INSERT INTO creature_involvedrelation (id, quest) VALUES (?, ?), (?, ?)")
        .bind(DEPUTY_WILLEM_ENTRY)
        .bind(A_THREAT_WITHIN_QUEST)
        .bind(MARSHAL_MCBRIDE_ENTRY)
        .bind(KOBOLD_CAMP_CLEANUP_QUEST)
        .execute(world_pool)
        .await?;
    sqlx::query(
        "INSERT INTO creature_loot_template \
         (entry, item, ChanceOrQuestChance, groupid, mincountOrRef, maxcount, condition_id, comments) \
         VALUES (?, 117, 100, 0, 1, 1, 0, 'Rust Northshire wolf food'), \
                (?, 159, 100, 0, 1, 1, 0, 'Rust Northshire kobold quest-progress stand-in')",
    )
    .bind(YOUNG_WOLF_ENTRY)
    .bind(KOBOLD_VERMIN_ENTRY)
    .execute(world_pool)
    .await?;

    seed_gameobject_fixture(world_pool).await?;
    sqlx::query(
        "INSERT INTO world_safe_locs (id, map, x, y, z, o, name) \
         VALUES (?, ?, -8910.0, -140.0, 82.0, 0.0, 'Rust Northshire Graveyard')",
    )
    .bind(FIXTURE_GRAVEYARD_ID)
    .bind(EASTERN_KINGDOMS_MAP)
    .execute(world_pool)
    .await?;
    sqlx::query(
        "INSERT INTO game_graveyard_zone (id, ghost_loc, link_kind, faction) VALUES (?, ?, 0, ?)",
    )
    .bind(ELWYNN_FOREST_ZONE)
    .bind(FIXTURE_GRAVEYARD_ID)
    .bind(ALLIANCE_FACTION)
    .execute(world_pool)
    .await?;

    Ok(())
}

async fn cleanup_northshire_fixture(world_pool: &MySqlPool) -> anyhow::Result<()> {
    for table in [
        "creature_questrelation",
        "creature_involvedrelation",
        "gameobject_questrelation",
        "gameobject_involvedrelation",
    ] {
        sqlx::query(&format!(
            "DELETE FROM {table} WHERE id BETWEEN ? AND ? OR quest BETWEEN ? AND ?"
        ))
        .bind(FIXTURE_PREFIX)
        .bind(FIXTURE_PREFIX + 999)
        .bind(FIXTURE_PREFIX)
        .bind(FIXTURE_PREFIX + 999)
        .execute(world_pool)
        .await?;
    }

    for table in ["npc_vendor", "npc_trainer", "creature_loot_template"] {
        sqlx::query(&format!("DELETE FROM {table} WHERE entry BETWEEN ? AND ?"))
            .bind(FIXTURE_PREFIX)
            .bind(FIXTURE_PREFIX + 999)
            .execute(world_pool)
            .await?;
    }

    sqlx::query("DELETE FROM creature WHERE guid BETWEEN ? AND ? OR id BETWEEN ? AND ?")
        .bind(FIXTURE_PREFIX)
        .bind(FIXTURE_PREFIX + 999)
        .bind(FIXTURE_PREFIX)
        .bind(FIXTURE_PREFIX + 999)
        .execute(world_pool)
        .await?;
    sqlx::query("DELETE FROM creature_template WHERE Entry BETWEEN ? AND ?")
        .bind(FIXTURE_PREFIX)
        .bind(FIXTURE_PREFIX + 999)
        .execute(world_pool)
        .await?;
    sqlx::query("DELETE FROM quest_template WHERE entry BETWEEN ? AND ?")
        .bind(FIXTURE_PREFIX)
        .bind(FIXTURE_PREFIX + 999)
        .execute(world_pool)
        .await?;
    sqlx::query("DELETE FROM gameobject WHERE guid BETWEEN ? AND ? OR id BETWEEN ? AND ?")
        .bind(FIXTURE_PREFIX)
        .bind(FIXTURE_PREFIX + 999)
        .bind(FIXTURE_PREFIX)
        .bind(FIXTURE_PREFIX + 999)
        .execute(world_pool)
        .await?;
    sqlx::query("DELETE FROM gameobject_template WHERE entry BETWEEN ? AND ?")
        .bind(FIXTURE_PREFIX)
        .bind(FIXTURE_PREFIX + 999)
        .execute(world_pool)
        .await?;
    sqlx::query("DELETE FROM game_graveyard_zone WHERE id = ? OR ghost_loc = ?")
        .bind(ELWYNN_FOREST_ZONE)
        .bind(FIXTURE_GRAVEYARD_ID)
        .execute(world_pool)
        .await?;
    sqlx::query("DELETE FROM world_safe_locs WHERE id = ?")
        .bind(FIXTURE_GRAVEYARD_ID)
        .execute(world_pool)
        .await?;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
async fn seed_creature_template(
    world_pool: &MySqlPool,
    entry: u32,
    name: &str,
    subname: &str,
    level: u8,
    display_id: u32,
    faction: u32,
    creature_type: u32,
    npc_flags: u32,
    health: u32,
    min_damage: f32,
    max_damage: f32,
    trainer_type: u8,
    trainer_class: u8,
    min_loot_gold: u32,
    max_loot_gold: u32,
) -> anyhow::Result<()> {
    sqlx::query(
        "INSERT INTO creature_template \
         (Entry, Name, SubName, MinLevel, MaxLevel, DisplayId1, DisplayIdProbability1, \
          Faction, Scale, Family, CreatureType, NpcFlags, UnitFlags, DynamicFlags, \
          Rank, MinLevelHealth, MaxLevelHealth, MinMeleeDmg, MaxMeleeDmg, \
          MeleeBaseAttackTime, RangedBaseAttackTime, TrainerType, TrainerClass, MinLootGold, MaxLootGold) \
         VALUES (?, ?, ?, ?, ?, ?, 100, ?, 1, 0, ?, ?, 0, 0, 0, ?, ?, ?, ?, 2000, 2000, ?, ?, ?, ?)",
    )
    .bind(entry)
    .bind(name)
    .bind(if subname.is_empty() {
        None
    } else {
        Some(subname)
    })
    .bind(level)
    .bind(level)
    .bind(display_id)
    .bind(faction)
    .bind(creature_type)
    .bind(npc_flags)
    .bind(health)
    .bind(health)
    .bind(min_damage)
    .bind(max_damage)
    .bind(trainer_type)
    .bind(trainer_class)
    .bind(min_loot_gold)
    .bind(max_loot_gold)
    .execute(world_pool)
    .await?;
    Ok(())
}

async fn seed_creature_spawn(
    world_pool: &MySqlPool,
    guid: u32,
    entry: u32,
    x: f32,
    y: f32,
    z: f32,
    orientation: f32,
) -> anyhow::Result<()> {
    sqlx::query(
        "INSERT INTO creature \
         (guid, id, map, spawnMask, position_x, position_y, position_z, orientation, \
          spawntimesecsmin, spawntimesecsmax, spawndist, MovementType) \
         VALUES (?, ?, ?, 1, ?, ?, ?, ?, 120, 120, 5, 0)",
    )
    .bind(guid)
    .bind(entry)
    .bind(EASTERN_KINGDOMS_MAP)
    .bind(x)
    .bind(y)
    .bind(z)
    .bind(orientation)
    .execute(world_pool)
    .await?;
    Ok(())
}

async fn seed_quest_template(
    world_pool: &MySqlPool,
    entry: u32,
    title: &str,
    req_creature: i32,
    req_count: u16,
    req_item: u32,
    req_item_count: u16,
) -> anyhow::Result<()> {
    sqlx::query(
        "INSERT INTO quest_template \
         (entry, Method, ZoneOrSort, MinLevel, QuestLevel, RequiredRaces, \
          Title, Details, Objectives, OfferRewardText, RequestItemsText, \
          ReqCreatureOrGOId1, ReqCreatureOrGOCount1, ReqItemId1, ReqItemCount1, \
          RewOrReqMoney) \
         VALUES (?, 2, ?, 1, 1, 1, ?, \
                 'Rust fixture quest detail for the Northshire starter-zone harness.', \
                 'Prove the Northshire quest data boundary exists.', \
                 'Good. Keep the fixture narrow until quest v1 lands.', \
                 'The Rust fixture is ready for the next quest slice.', \
                 ?, ?, ?, ?, 25)",
    )
    .bind(entry)
    .bind(NORTHSHIRE_ZONE as i16)
    .bind(title)
    .bind(req_creature)
    .bind(req_count)
    .bind(req_item)
    .bind(req_item_count)
    .execute(world_pool)
    .await?;
    Ok(())
}

async fn seed_gameobject_fixture(world_pool: &MySqlPool) -> anyhow::Result<()> {
    sqlx::query(
        "INSERT INTO gameobject_template \
         (entry, type, displayId, name, IconName, faction, flags, size, data0, data1, data2) \
         VALUES (?, 3, 0, 'Rust Northshire Supply Crate', '', 0, 0, 1, 57, 0, 1)",
    )
    .bind(NORTHSHIRE_CRATE_ENTRY)
    .execute(world_pool)
    .await?;
    sqlx::query(
        "INSERT INTO gameobject \
         (guid, id, map, spawnMask, position_x, position_y, position_z, orientation, \
          rotation0, rotation1, rotation2, rotation3, spawntimesecsmin, spawntimesecsmax) \
         VALUES (?, ?, ?, 1, -8918.0, -145.0, 82.0, 0.0, 0, 0, 0, 1, 120, 120)",
    )
    .bind(FIXTURE_PREFIX + 101)
    .bind(NORTHSHIRE_CRATE_ENTRY)
    .bind(EASTERN_KINGDOMS_MAP)
    .execute(world_pool)
    .await?;
    Ok(())
}

async fn assert_human_warrior_enters_northshire(
    character_pool: &MySqlPool,
    account_id: u32,
    guid: u32,
) -> anyhow::Result<()> {
    let character = wow_db::get_character_enum_entries(character_pool, account_id)
        .await?
        .into_iter()
        .find(|character| character.guid == guid)
        .context("fresh Human Warrior is missing from character enum rows")?;
    ensure!(
        character.name == CHARACTER_NAME,
        "unexpected character name"
    );
    ensure!(character.race == 1, "fresh character race was not Human");
    ensure!(
        character.class == 1,
        "fresh character class was not Warrior"
    );
    ensure!(
        character.map == EASTERN_KINGDOMS_MAP,
        "Human Warrior map was not Eastern Kingdoms"
    );
    ensure!(
        character.zone == NORTHSHIRE_ZONE,
        "Human Warrior zone was not Northshire/Elwynn"
    );
    ensure!(
        (character.position_x - HUMAN_START_X).abs() < 0.1
            && (character.position_y - HUMAN_START_Y).abs() < 0.1,
        "Human Warrior did not spawn at the CMaNGOS playercreateinfo Northshire start"
    );
    Ok(())
}

async fn assert_starter_zone_creatures_available(
    world_pool: &MySqlPool,
    position: WorldPosition,
    content: &StarterZoneContent,
) -> anyhow::Result<()> {
    let spawns = wow_db::get_nearby_creature_spawns(
        world_pool,
        position.map_id,
        position.x,
        position.y,
        REAL_NORTHSHIRE_VISIBLE_RADIUS_YARDS,
        REAL_NORTHSHIRE_VISIBLE_LIMIT,
    )
    .await?;
    for expected in &content.visible_creatures {
        ensure!(
            spawns
                .iter()
                .any(|spawn| spawn.entry == expected.entry && spawn.guid == expected.counter),
            "Northshire spawn entry={} guid={} was not visible near the Human Warrior start",
            expected.entry,
            expected.counter
        );
    }
    if content.source == StarterZoneSource::Fixture {
        ensure!(
            spawns
                .iter()
                .any(|spawn| spawn.entry == KOBOLD_VERMIN_ENTRY && spawn.template.min_level == 2),
            "Kobold fixture did not carry joined creature_template data"
        );
    }
    Ok(())
}

async fn assert_real_starter_zone_rows(world_pool: &MySqlPool) -> anyhow::Result<()> {
    let quest_rows: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) \
         FROM creature_questrelation \
         WHERE id IN (?, ?) \
           AND quest IN (6, 7, 15, 18, 21, 783)",
    )
    .bind(REAL_MARSHAL_MCBRIDE_ENTRY)
    .bind(REAL_DEPUTY_WILLEM_ENTRY)
    .fetch_one(world_pool)
    .await?;
    ensure!(
        quest_rows >= 4,
        "real ClassicDB Northshire quest giver rows were missing"
    );

    let involved_rows: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) \
         FROM creature_involvedrelation \
         WHERE id IN (?, ?) \
           AND quest IN (6, 7, 15, 18, 21, 783)",
    )
    .bind(REAL_MARSHAL_MCBRIDE_ENTRY)
    .bind(REAL_DEPUTY_WILLEM_ENTRY)
    .fetch_one(world_pool)
    .await?;
    ensure!(
        involved_rows >= 4,
        "real ClassicDB Northshire quest completer rows were missing"
    );

    let wolf_loot_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) \
         FROM creature_loot_template \
         JOIN item_template ON creature_loot_template.item = item_template.entry \
         WHERE creature_loot_template.entry = ? \
           AND creature_loot_template.condition_id = 0 \
           AND creature_loot_template.ChanceOrQuestChance > 0 \
           AND creature_loot_template.groupid = 0 \
           AND creature_loot_template.mincountOrRef > 0",
    )
    .bind(REAL_YOUNG_WOLF_ENTRY)
    .fetch_one(world_pool)
    .await?;
    ensure!(
        wolf_loot_count > 0,
        "real ClassicDB Young Wolf had no normal loot rows with valid item templates"
    );

    Ok(())
}

async fn assert_starter_zone_interaction_rows(world_pool: &MySqlPool) -> anyhow::Result<()> {
    let vendor_items = wow_db::get_vendor_items(world_pool, DEPUTY_WILLEM_ENTRY).await?;
    ensure!(
        vendor_items.iter().any(|item| item.item == 117),
        "Northshire DB vendor fixture did not expose source-backed Tough Jerky"
    );
    let trainer_spell: Option<u32> =
        sqlx::query_scalar("SELECT spell FROM npc_trainer WHERE entry = ? AND spell = 772")
            .bind(BROTHER_PAXTON_ENTRY)
            .fetch_optional(world_pool)
            .await?;
    ensure!(
        trainer_spell == Some(772),
        "Northshire trainer fixture did not expose a warrior trainer spell row"
    );
    Ok(())
}

async fn assert_starter_zone_loot_and_quest_rows(world_pool: &MySqlPool) -> anyhow::Result<()> {
    let quest_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM quest_template WHERE entry IN (?, ?) \
         AND ZoneOrSort = ? AND RequiredRaces = 1",
    )
    .bind(A_THREAT_WITHIN_QUEST)
    .bind(KOBOLD_CAMP_CLEANUP_QUEST)
    .bind(NORTHSHIRE_ZONE as i16)
    .fetch_one(world_pool)
    .await?;
    ensure!(
        quest_count == 2,
        "expected two Northshire starter quest templates"
    );

    let relation_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM creature_questrelation WHERE quest IN (?, ?)")
            .bind(A_THREAT_WITHIN_QUEST)
            .bind(KOBOLD_CAMP_CLEANUP_QUEST)
            .fetch_one(world_pool)
            .await?;
    ensure!(
        relation_count == 2,
        "expected two Northshire creature quest-start relation rows"
    );

    let loot_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) \
         FROM creature_loot_template \
         JOIN item_template ON creature_loot_template.item = item_template.entry \
         WHERE creature_loot_template.entry IN (?, ?)",
    )
    .bind(YOUNG_WOLF_ENTRY)
    .bind(KOBOLD_VERMIN_ENTRY)
    .fetch_one(world_pool)
    .await?;
    ensure!(
        loot_count == 2,
        "expected two Northshire loot rows with valid source-backed item templates"
    );
    Ok(())
}

async fn assert_starter_zone_gameobject_and_graveyard_rows(
    world_pool: &MySqlPool,
) -> anyhow::Result<()> {
    let gameobject_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) \
         FROM gameobject \
         JOIN gameobject_template ON gameobject.id = gameobject_template.entry \
         WHERE gameobject.id = ? AND gameobject.map = ?",
    )
    .bind(NORTHSHIRE_CRATE_ENTRY)
    .bind(EASTERN_KINGDOMS_MAP)
    .fetch_one(world_pool)
    .await?;
    ensure!(
        gameobject_count == 1,
        "expected one DB-backed Northshire gameobject fixture"
    );

    let graveyard_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) \
         FROM game_graveyard_zone \
         JOIN world_safe_locs ON game_graveyard_zone.ghost_loc = world_safe_locs.id \
         WHERE game_graveyard_zone.id = ? AND game_graveyard_zone.faction = ?",
    )
    .bind(ELWYNN_FOREST_ZONE)
    .bind(ALLIANCE_FACTION)
    .fetch_one(world_pool)
    .await?;
    ensure!(
        graveyard_count == 1,
        "expected one Northshire/Elwynn Alliance graveyard link"
    );
    Ok(())
}

async fn fetch_session_key(login_pool: &MySqlPool) -> anyhow::Result<[u8; 40]> {
    let session_key: String =
        sqlx::query_scalar("SELECT sessionkey FROM account WHERE username = ?")
            .bind(USERNAME)
            .fetch_one(login_pool)
            .await?;
    hex_to_array40(&session_key)
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

    fn login_character_expect_northshire_creatures(
        &mut self,
        guid: u32,
        content: &StarterZoneContent,
    ) -> anyhow::Result<()> {
        let guid = ObjectGuid::new(HighGuid::Player, 0, guid);
        write_client_packet(
            &mut self.stream,
            CMSG_PLAYER_LOGIN,
            &guid.raw().to_le_bytes(),
            Some(&mut self.crypto),
        )?;

        let mut update_bodies = Vec::new();
        let expected_update_packets = if content.source == StarterZoneSource::RealClassicDb {
            5
        } else {
            2
        };
        for _ in 0..24 {
            let (opcode, body) = read_server_packet(&mut self.stream, Some(&mut self.crypto))?;
            if opcode == SMSG_UPDATE_OBJECT {
                update_bodies.push(body);
                if update_bodies.len() >= expected_update_packets {
                    break;
                }
            }
        }
        ensure!(
            !update_bodies.is_empty(),
            "login did not produce any SMSG_UPDATE_OBJECT packet"
        );

        for expected in &content.visible_creatures {
            let guid = ObjectGuid::new(HighGuid::Unit, expected.entry, expected.counter);
            let guid_bytes = guid.raw().to_le_bytes();
            ensure!(
                update_bodies.iter().any(|body| body
                    .windows(guid_bytes.len())
                    .any(|window| window == guid_bytes)),
                "Northshire DB creature entry={} counter={} was missing from login update object",
                expected.entry,
                expected.counter
            );
        }
        Ok(())
    }

    fn kill_loot_and_respawn_young_wolf(
        &mut self,
        content: &StarterZoneContent,
    ) -> anyhow::Result<()> {
        let wolf = ObjectGuid::new(HighGuid::Unit, content.wolf.entry, content.wolf.counter);
        write_client_packet(
            &mut self.stream,
            CMSG_ATTACKSWING,
            &wolf.raw().to_le_bytes(),
            Some(&mut self.crypto),
        )?;

        let mut saw_damage = false;
        let mut saw_dead_lootable = false;
        for _ in 0..8 {
            let (opcode, body) = read_server_packet(&mut self.stream, Some(&mut self.crypto))?;
            if opcode == SMSG_ATTACKERSTATEUPDATE {
                ensure!(body.len() >= 20, "wolf damage packet was too short");
                saw_damage = true;
            }
            if opcode == SMSG_UPDATE_OBJECT
                && update_packet_has_values(
                    &body,
                    wolf,
                    &[
                        (
                            UNIT_FIELD_HEALTH,
                            if content.source == StarterZoneSource::RealClassicDb {
                                content.wolf_health.saturating_sub(2)
                            } else {
                                0
                            },
                        ),
                        (
                            UNIT_DYNAMIC_FLAGS,
                            if content.source == StarterZoneSource::RealClassicDb {
                                0
                            } else {
                                UNIT_DYNFLAG_LOOTABLE
                            },
                        ),
                    ],
                )?
            {
                saw_dead_lootable = true;
                break;
            }
        }
        ensure!(saw_damage, "Young Wolf did not receive combat damage");
        if content.source == StarterZoneSource::RealClassicDb {
            ensure!(
                saw_dead_lootable,
                "real ClassicDB Young Wolf did not transition to damaged runtime state"
            );
            return Ok(());
        }
        ensure!(
            saw_dead_lootable,
            "Young Wolf did not transition to a lootable corpse"
        );

        write_client_packet(
            &mut self.stream,
            CMSG_LOOT,
            &wolf.raw().to_le_bytes(),
            Some(&mut self.crypto),
        )?;
        let loot_body = self.read_until(SMSG_LOOT_RESPONSE, 6)?;
        assert_wolf_loot_response(
            &loot_body,
            wolf,
            content.wolf_loot_money,
            content.wolf_loot_item,
        )?;

        write_client_packet(
            &mut self.stream,
            CMSG_LOOT_MONEY,
            &[],
            Some(&mut self.crypto),
        )?;
        let _ = self.read_until(SMSG_UPDATE_OBJECT, 6)?;

        write_client_packet(
            &mut self.stream,
            CMSG_AUTOSTORE_LOOT_ITEM,
            &[0],
            Some(&mut self.crypto),
        )?;
        let _ = self.read_until(SMSG_UPDATE_OBJECT, 6)?;

        write_client_packet(
            &mut self.stream,
            CMSG_LOOT_RELEASE,
            &wolf.raw().to_le_bytes(),
            Some(&mut self.crypto),
        )?;
        let release = self.read_until(SMSG_LOOT_RELEASE_RESPONSE, 6)?;
        ensure!(
            release == [wolf.raw().to_le_bytes().as_slice(), &[1]].concat(),
            "Young Wolf loot release response was malformed"
        );

        let mut saw_respawn = false;
        for _ in 0..6 {
            let (opcode, body) = read_server_packet(&mut self.stream, Some(&mut self.crypto))?;
            if opcode == SMSG_UPDATE_OBJECT
                && update_packet_has_values(
                    &body,
                    wolf,
                    &[
                        (UNIT_FIELD_HEALTH, content.wolf_health),
                        (UNIT_DYNAMIC_FLAGS, 0),
                    ],
                )?
            {
                saw_respawn = true;
                break;
            }
        }
        ensure!(
            saw_respawn,
            "Young Wolf did not respawn alive after loot release"
        );
        Ok(())
    }

    fn read_until(&mut self, expected_opcode: u32, max_packets: usize) -> anyhow::Result<Vec<u8>> {
        for _ in 0..max_packets {
            let (opcode, body) = read_server_packet(&mut self.stream, Some(&mut self.crypto))?;
            if opcode == expected_opcode {
                return Ok(body);
            }
        }
        anyhow::bail!("did not receive expected opcode 0x{expected_opcode:04X}");
    }
}

fn assert_wolf_loot_response(
    body: &[u8],
    wolf: ObjectGuid,
    expected_money: u32,
    expected_item: Option<u32>,
) -> anyhow::Result<()> {
    ensure!(body.len() >= 36, "wolf loot response was too short");
    ensure!(
        &body[0..8] == wolf.raw().to_le_bytes().as_slice(),
        "wolf loot response used the wrong target guid"
    );
    ensure!(body[8] == 1, "wolf loot response was not corpse loot");
    ensure!(
        u32::from_le_bytes(body[9..13].try_into()?) == expected_money,
        "wolf loot response did not expose DB-backed copper"
    );
    let expected_item = expected_item.context("expected wolf loot item was missing")?;
    ensure!(body[13] == 1, "wolf loot response did not expose one item");
    ensure!(body[14] == 0, "wolf loot slot was not zero");
    ensure!(
        u32::from_le_bytes(body[15..19].try_into()?) == expected_item,
        "wolf loot item was not the expected DB-backed item"
    );
    ensure!(
        u32::from_le_bytes(body[19..23].try_into()?) == 1,
        "wolf loot item count was not one"
    );
    Ok(())
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
        cursor += 3;
        cursor += 5;
        cursor += 1;
        cursor += 4;
        cursor += 4;
        cursor += 12;
        cursor += 4;
        cursor += 4;
        cursor += 1;
        cursor += 12;
        cursor += 20 * 5;

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

fn update_packet_has_values(
    body: &[u8],
    guid: ObjectGuid,
    expected: &[(usize, u32)],
) -> anyhow::Result<bool> {
    ensure_available(body, 5)?;
    let block_count = u32::from_le_bytes(body[0..4].try_into()?) as usize;
    let mut cursor = 5;
    for _ in 0..block_count {
        ensure_available(body, cursor + 1)?;
        let update_type = body[cursor];
        cursor += 1;
        let block_guid = read_packed_update_guid(body, &mut cursor)?;

        match update_type {
            0 => {
                let values_start = cursor;
                let values_len = update_values_encoded_len(&body[values_start..])?;
                let values = decode_update_values(&body[values_start..values_start + values_len])?;
                cursor += values_len;
                if block_guid == guid.raw()
                    && expected
                        .iter()
                        .all(|(field, value)| values.get(*field) == Some(&Some(*value)))
                {
                    return Ok(true);
                }
            }
            2 | 3 => {
                ensure_available(body, cursor + 1)?;
                cursor += 1; // type id
                ensure_available(body, cursor + 1)?;
                let update_flags = body[cursor];
                cursor += 1;
                if update_flags & 0x20 != 0 {
                    ensure_available(body, cursor + 56)?;
                    cursor += 56;
                }
                if update_flags & 0x10 != 0 {
                    ensure_available(body, cursor + 4)?;
                    cursor += 4;
                }
                let values_start = cursor;
                let values_len = update_values_encoded_len(&body[values_start..])?;
                cursor += values_len;
            }
            other => anyhow::bail!("unsupported update block type {other}"),
        }
    }
    Ok(false)
}

fn read_packed_update_guid(body: &[u8], cursor: &mut usize) -> anyhow::Result<u64> {
    ensure_available(body, *cursor + 1)?;
    let mask = body[*cursor];
    *cursor += 1;
    let mut raw = 0u64;
    for index in 0..8 {
        if mask & (1 << index) != 0 {
            ensure_available(body, *cursor + 1)?;
            raw |= (body[*cursor] as u64) << (index * 8);
            *cursor += 1;
        }
    }
    Ok(raw)
}

fn update_values_encoded_len(body: &[u8]) -> anyhow::Result<usize> {
    ensure_available(body, 1)?;
    let block_count = body[0] as usize;
    let mask_start = 1;
    let mask_len = block_count * 4;
    ensure_available(body, mask_start + mask_len)?;
    let mut value_count = 0usize;
    for block in 0..block_count {
        let offset = mask_start + block * 4;
        value_count +=
            u32::from_le_bytes(body[offset..offset + 4].try_into()?).count_ones() as usize;
    }
    let len = mask_start + mask_len + value_count * 4;
    ensure_available(body, len)?;
    Ok(len)
}

fn decode_update_values(body: &[u8]) -> anyhow::Result<Vec<Option<u32>>> {
    let block_count = body[0] as usize;
    let mask_start = 1;
    let mut value_cursor = mask_start + block_count * 4;
    let mut values = vec![None; block_count * 32];

    for (index, value_slot) in values.iter_mut().enumerate() {
        let mask_offset = mask_start + (index / 32) * 4;
        let mask = u32::from_le_bytes(body[mask_offset..mask_offset + 4].try_into()?);
        if mask & (1 << (index % 32)) == 0 {
            continue;
        }
        *value_slot = Some(u32::from_le_bytes(
            body[value_cursor..value_cursor + 4].try_into()?,
        ));
        value_cursor += 4;
    }
    Ok(values)
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
        (2..=0xFFFF).contains(&size),
        "malformed server packet size {size}"
    );
    let body_len = size - 2;
    let body = read_exact_vec(stream, body_len)?;
    Ok((opcode, body))
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
