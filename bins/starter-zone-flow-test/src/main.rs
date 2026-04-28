use anyhow::{ensure, Context};
use bytes::BytesMut;
use sha1::{Digest, Sha1};
use sqlx::mysql::{MySqlPool, MySqlPoolOptions};
use sqlx::QueryBuilder;
use std::collections::HashSet;
use std::io::{ErrorKind, Read, Write};
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
const REAL_LLANE_BESHERE_ENTRY: u32 = 911;
const REAL_YOUNG_WOLF_ENTRY: u32 = 299;
const REAL_KOBOLD_VERMIN_ENTRY: u32 = 6;
const REAL_NORTHSHIRE_VISIBLE_RADIUS_YARDS: f32 = 100.0;
const REAL_NORTHSHIRE_VISIBLE_LIMIT: u32 = 128;
const FIXTURE_PREFIX: u32 = 910_000;
const MARSHAL_MCBRIDE_ENTRY: u32 = FIXTURE_PREFIX + 1;
const DEPUTY_WILLEM_ENTRY: u32 = FIXTURE_PREFIX + 2;
const BROTHER_PAXTON_ENTRY: u32 = FIXTURE_PREFIX + 3;
const LLANE_BESHERE_ENTRY: u32 = FIXTURE_PREFIX + 6;
const YOUNG_WOLF_ENTRY: u32 = FIXTURE_PREFIX + 4;
const KOBOLD_VERMIN_ENTRY: u32 = FIXTURE_PREFIX + 5;
const DEATH_TESTER_ENTRY: u32 = FIXTURE_PREFIX + 7;
const DEATH_TESTER_GUID: u32 = FIXTURE_PREFIX + 907;
const YOUNG_WOLF_DISPLAY_ID: u32 = 372;
const KOBOLD_VERMIN_DISPLAY_ID: u32 = 365;
const NORTHSHIRE_CRATE_ENTRY: u32 = FIXTURE_PREFIX + 101;
const A_THREAT_WITHIN_QUEST: u32 = FIXTURE_PREFIX + 201;
const KOBOLD_CAMP_CLEANUP_QUEST: u32 = FIXTURE_PREFIX + 202;
const FIXTURE_GRAVEYARD_ID: u32 = FIXTURE_PREFIX + 301;
const CMSG_CHAR_ENUM: u32 = 0x0037;
const CMSG_PLAYER_LOGIN: u32 = 0x003D;
const CMSG_CAST_SPELL: u32 = 0x012E;
const CMSG_GOSSIP_HELLO: u32 = 0x017B;
const CMSG_QUESTGIVER_STATUS_QUERY: u32 = 0x0182;
const CMSG_QUESTGIVER_QUERY_QUEST: u32 = 0x0186;
const CMSG_QUESTGIVER_ACCEPT_QUEST: u32 = 0x0189;
const CMSG_QUESTGIVER_CHOOSE_REWARD: u32 = 0x018E;
const CMSG_TRAINER_LIST: u32 = 0x01B0;
const CMSG_TRAINER_BUY_SPELL: u32 = 0x01B2;
const CMSG_ATTACKSWING: u32 = 0x0141;
const CMSG_REPOP_REQUEST: u32 = 0x015A;
const CMSG_LOOT: u32 = 0x015D;
const CMSG_LOOT_MONEY: u32 = 0x015E;
const CMSG_LOOT_RELEASE: u32 = 0x015F;
const CMSG_AUTOSTORE_LOOT_ITEM: u32 = 0x0108;
const CMSG_AUTH_SESSION: u32 = 0x01ED;
const CMSG_RECLAIM_CORPSE: u32 = 0x01D2;
const CMSG_MOVE_HEARTBEAT: u32 = 0x00EE;
const MSG_CORPSE_QUERY: u32 = 0x0216;
const SMSG_CHAR_ENUM: u32 = 0x003B;
const SMSG_UPDATE_OBJECT: u32 = 0x00A9;
const SMSG_DESTROY_OBJECT: u32 = 0x00AA;
const SMSG_QUESTGIVER_STATUS: u32 = 0x0183;
const SMSG_QUESTGIVER_QUEST_LIST: u32 = 0x0185;
const SMSG_QUESTGIVER_QUEST_DETAILS: u32 = 0x0188;
const SMSG_QUESTGIVER_OFFER_REWARD: u32 = 0x018D;
const SMSG_QUESTGIVER_QUEST_COMPLETE: u32 = 0x0191;
const SMSG_QUESTUPDATE_COMPLETE: u32 = 0x0198;
const SMSG_QUESTUPDATE_ADD_KILL: u32 = 0x0199;
const SMSG_TRAINER_LIST: u32 = 0x01B1;
const SMSG_TRAINER_BUY_SUCCEEDED: u32 = 0x01B3;
const SMSG_LEARNED_SPELL: u32 = 0x012B;
const SMSG_MONSTER_MOVE: u32 = 0x00DD;
const SMSG_ATTACKSTART: u32 = 0x0143;
const SMSG_ATTACKERSTATEUPDATE: u32 = 0x014A;
const SMSG_LOOT_RESPONSE: u32 = 0x0160;
const SMSG_LOOT_RELEASE_RESPONSE: u32 = 0x0161;
const SMSG_LOG_XPGAIN: u32 = 0x01D0;
const SMSG_LEVELUP_INFO: u32 = 0x01D4;
const SMSG_AUTH_CHALLENGE: u32 = 0x01EC;
const SMSG_AUTH_RESPONSE: u32 = 0x01EE;
const SMSG_CORPSE_RECLAIM_DELAY: u32 = 0x0269;
const MSG_MOVE_TELEPORT_ACK: u32 = 0x00C7;
const AUTH_OK: u8 = 0x0C;
const UNIT_FIELD_HEALTH: usize = 0x016;
const UNIT_FIELD_LEVEL: usize = 0x022;
const UNIT_DYNAMIC_FLAGS: usize = 0x08F;
const PLAYER_FLAGS_FIELD: usize = 0x0BE;
const PLAYER_NEXT_LEVEL_XP: usize = 0x2CD;
const CORPSE_FIELD_FLAGS: usize = 0x023;
const UNIT_DYNFLAG_LOOTABLE: u32 = 0x0000_0001;
const PLAYER_FLAGS_GHOST: u32 = 0x0000_0010;
const CORPSE_FLAG_BONES: u32 = 0x01;
const DIALOG_STATUS_AVAILABLE: u32 = 5;
const DIALOG_STATUS_REWARD2: u32 = 7;
const QUEST_STATUS_COMPLETE: u32 = 1;
const HEROIC_STRIKE_RANK_1: u32 = 78;
const WARRIOR_BATTLE_SHOUT_RANK_1: u32 = 6673;
const WARRIOR_BATTLE_SHOUT_TRAINER_CAST: u32 = 6674;
const SPELL_CAST_TARGET_UNIT: u16 = 0x0002;

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
struct ExpectedCreatureAt {
    creature: ExpectedCreature,
    position: WorldPosition,
}

#[derive(Debug, Clone)]
struct StarterZoneContent {
    source: StarterZoneSource,
    visible_creatures: Vec<ExpectedCreature>,
    kobold_quest: u32,
    quest_giver: ExpectedCreature,
    trainer: ExpectedCreature,
    trainer_spell: u32,
    streaming_creature: ExpectedCreature,
    streaming_position: WorldPosition,
    kobold: ExpectedCreature,
    kobold_position: WorldPosition,
    kobold_targets: Vec<ExpectedCreatureAt>,
    kobold_required_count: u32,
    wolf: ExpectedCreature,
    wolf_position: WorldPosition,
    wolf_health: u32,
    wolf_loot_money: u32,
    wolf_loot_item: Option<u32>,
    death_creature: ExpectedCreature,
    death_position: WorldPosition,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let login_pool = connect(LOGIN_DATABASE_URL).await?;
    let character_pool = connect(CHARACTER_DATABASE_URL).await?;
    let world_pool = connect(WORLD_DATABASE_URL).await?;

    let account_id = seed_account(&login_pool).await?;
    cleanup_account_characters(&character_pool, account_id).await?;
    let starter_zone = prepare_northshire_content(&world_pool).await?;
    cleanup_starter_zone_creature_respawns(&character_pool, &starter_zone).await?;

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
    world.kill_loot_and_release_young_wolf(&starter_zone)?;
    world.move_to_expect_streamed_creature(&starter_zone)?;
    world.move_to_expect_destroyed_creature(&starter_zone)?;
    world.move_to_expect_streamed_creature(&starter_zone)?;
    world.move_near_kobold_expect_no_aggro(&starter_zone)?;
    world.complete_kobold_camp_cleanup(&starter_zone)?;
    world.learn_warrior_trainer_spell(&starter_zone)?;
    let death_proof = world.die_release_and_reclaim_corpse(&starter_zone)?;
    assert_kobold_camp_cleanup_persisted(&character_pool, created.guid, starter_zone.kobold_quest)
        .await?;
    assert_warrior_trainer_spell_persisted(
        &character_pool,
        created.guid,
        WARRIOR_BATTLE_SHOUT_RANK_1,
    )
    .await?;
    assert_starter_zone_creature_respawns_persisted(&character_pool, &starter_zone).await?;
    assert_player_death_reclaim_persisted(&character_pool, created.guid, death_proof).await?;

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
    for guid in &guids {
        sqlx::query("DELETE FROM corpse WHERE player = ?")
            .bind(*guid)
            .execute(character_pool)
            .await?;
    }
    for guid in guids {
        let deleted = wow_db::delete_character(character_pool, account_id, guid).await?;
        ensure!(
            deleted,
            "failed to delete stale starter-zone character {guid}"
        );
    }
    Ok(())
}

async fn cleanup_starter_zone_creature_respawns(
    character_pool: &MySqlPool,
    content: &StarterZoneContent,
) -> anyhow::Result<()> {
    let mut guids = vec![
        content.streaming_creature.counter,
        content.kobold.counter,
        content.wolf.counter,
    ];
    guids.extend(
        content
            .visible_creatures
            .iter()
            .map(|creature| creature.counter),
    );
    guids.extend(
        content
            .kobold_targets
            .iter()
            .map(|target| target.creature.counter),
    );
    guids.push(content.death_creature.counter);
    guids.sort_unstable();
    guids.dedup();

    if guids.is_empty() {
        return Ok(());
    }

    let mut builder =
        QueryBuilder::new("DELETE FROM creature_respawn WHERE instance = 0 AND guid IN (");
    let mut separated = builder.separated(", ");
    for guid in guids {
        separated.push_bind(guid);
    }
    separated.push_unseparated(")");
    builder.build().execute(character_pool).await?;
    sqlx::query("DELETE FROM creature_respawn WHERE instance = 0 AND guid = ?")
        .bind(content.death_creature.counter)
        .execute(character_pool)
        .await?;
    Ok(())
}

async fn prepare_northshire_content(world_pool: &MySqlPool) -> anyhow::Result<StarterZoneContent> {
    cleanup_northshire_fixture(world_pool).await?;
    if let Some(real) = load_real_northshire_content(world_pool).await? {
        seed_death_test_fixture(world_pool).await?;
        return Ok(real);
    }

    seed_northshire_fixture(world_pool).await?;
    seed_death_test_fixture(world_pool).await?;
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
        REAL_NORTHSHIRE_VISIBLE_RADIUS_YARDS * 3.0,
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
    let wider_northshire = wow_db::get_nearby_creature_spawns(
        world_pool,
        EASTERN_KINGDOMS_MAP,
        HUMAN_START_X,
        HUMAN_START_Y,
        REAL_NORTHSHIRE_VISIBLE_RADIUS_YARDS * 3.0,
        REAL_NORTHSHIRE_VISIBLE_LIMIT * 4,
    )
    .await?;

    let required_nearby = [
        REAL_MARSHAL_MCBRIDE_ENTRY,
        REAL_DEPUTY_WILLEM_ENTRY,
        REAL_BROTHER_PAXTON_ENTRY,
        REAL_LLANE_BESHERE_ENTRY,
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
        REAL_LLANE_BESHERE_ENTRY,
        REAL_YOUNG_WOLF_ENTRY,
    ];
    ensure!(
        required_visible
            .iter()
            .all(|entry| visible.iter().any(|spawn| spawn.entry == *entry)),
        "real ClassicDB Northshire core NPC/wolf rows exist, but not all are visible within the current worldserver spawn radius"
    );
    let visible_guids = visible
        .iter()
        .map(|spawn| spawn.guid)
        .collect::<HashSet<_>>();
    let streaming_spawn = wider_northshire
        .iter()
        .find(|spawn| spawn.entry == REAL_KOBOLD_VERMIN_ENTRY && !visible_guids.contains(&spawn.guid))
        .context("real ClassicDB Kobold Vermin was not outside the login visibility radius for movement streaming proof")?;

    let wolf_spawn = visible
        .iter()
        .find(|spawn| spawn.entry == REAL_YOUNG_WOLF_ENTRY)
        .context("real ClassicDB Young Wolf was not visible near the Human Warrior start")?;
    let kobold_spawn = streaming_spawn;
    let quest_giver_spawn = visible
        .iter()
        .find(|spawn| spawn.entry == REAL_MARSHAL_MCBRIDE_ENTRY)
        .context("real ClassicDB Marshal McBride was not visible near the Human Warrior start")?;
    let trainer_spawn = visible
        .iter()
        .find(|spawn| spawn.entry == REAL_LLANE_BESHERE_ENTRY)
        .context("real ClassicDB Llane Beshere was not visible near the Human Warrior start")?;
    let kobold_required_count: u32 =
        sqlx::query_scalar("SELECT ReqCreatureOrGOCount1 FROM quest_template WHERE entry = 7")
            .fetch_one(world_pool)
            .await?;
    let mut quest_kobold_spawns = wider_northshire
        .iter()
        .filter(|spawn| spawn.entry == REAL_KOBOLD_VERMIN_ENTRY)
        .cloned()
        .collect::<Vec<_>>();
    quest_kobold_spawns.sort_by(|left, right| {
        let left_moves = left.spawn_dist > 0.0 || left.movement_type != 0;
        let right_moves = right.spawn_dist > 0.0 || right.movement_type != 0;
        left_moves
            .cmp(&right_moves)
            .then_with(|| {
                let left_distance = (left.position_x - HUMAN_START_X).powi(2)
                    + (left.position_y - HUMAN_START_Y).powi(2);
                let right_distance = (right.position_x - HUMAN_START_X).powi(2)
                    + (right.position_y - HUMAN_START_Y).powi(2);
                left_distance.total_cmp(&right_distance)
            })
            .then_with(|| left.guid.cmp(&right.guid))
    });
    quest_kobold_spawns.truncate(kobold_required_count as usize);
    ensure!(
        quest_kobold_spawns.len() >= kobold_required_count as usize,
        "real ClassicDB Northshire did not have enough Kobold Vermin spawns near the starter path for non-instant-respawn quest proof: found={} required={}",
        quest_kobold_spawns.len(),
        kobold_required_count
    );
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
        kobold_quest: 7,
        quest_giver: ExpectedCreature {
            entry: quest_giver_spawn.entry,
            counter: quest_giver_spawn.guid,
        },
        trainer: ExpectedCreature {
            entry: trainer_spawn.entry,
            counter: trainer_spawn.guid,
        },
        trainer_spell: WARRIOR_BATTLE_SHOUT_TRAINER_CAST,
        streaming_creature: ExpectedCreature {
            entry: streaming_spawn.entry,
            counter: streaming_spawn.guid,
        },
        streaming_position: WorldPosition::new(
            streaming_spawn.map,
            (HUMAN_START_X + streaming_spawn.position_x) / 2.0,
            (HUMAN_START_Y + streaming_spawn.position_y) / 2.0,
            streaming_spawn.position_z,
            streaming_spawn.orientation,
        ),
        kobold: ExpectedCreature {
            entry: kobold_spawn.entry,
            counter: kobold_spawn.guid,
        },
        kobold_position: WorldPosition::new(
            kobold_spawn.map,
            kobold_spawn.position_x,
            kobold_spawn.position_y,
            kobold_spawn.position_z,
            kobold_spawn.orientation,
        ),
        kobold_targets: quest_kobold_spawns
            .iter()
            .map(|spawn| ExpectedCreatureAt {
                creature: ExpectedCreature {
                    entry: spawn.entry,
                    counter: spawn.guid,
                },
                position: WorldPosition::new(
                    spawn.map,
                    spawn.position_x,
                    spawn.position_y,
                    spawn.position_z,
                    spawn.orientation,
                ),
            })
            .collect(),
        kobold_required_count,
        wolf: ExpectedCreature {
            entry: wolf_spawn.entry,
            counter: wolf_spawn.guid,
        },
        wolf_position: WorldPosition::new(
            wolf_spawn.map,
            wolf_spawn.position_x,
            wolf_spawn.position_y,
            wolf_spawn.position_z,
            wolf_spawn.orientation,
        ),
        wolf_health: wolf_spawn.template.max_level_health,
        wolf_loot_money: wolf_spawn
            .template
            .max_loot_gold
            .max(wolf_spawn.template.min_loot_gold),
        wolf_loot_item: wolf_loot.map(|loot| loot.item),
        death_creature: death_test_creature(),
        death_position: death_test_position(),
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
                entry: LLANE_BESHERE_ENTRY,
                counter: FIXTURE_PREFIX + 6,
            },
            ExpectedCreature {
                entry: YOUNG_WOLF_ENTRY,
                counter: FIXTURE_PREFIX + 4,
            },
        ],
        kobold_quest: KOBOLD_CAMP_CLEANUP_QUEST,
        quest_giver: ExpectedCreature {
            entry: MARSHAL_MCBRIDE_ENTRY,
            counter: FIXTURE_PREFIX + 1,
        },
        trainer: ExpectedCreature {
            entry: LLANE_BESHERE_ENTRY,
            counter: FIXTURE_PREFIX + 6,
        },
        trainer_spell: WARRIOR_BATTLE_SHOUT_TRAINER_CAST,
        streaming_creature: ExpectedCreature {
            entry: KOBOLD_VERMIN_ENTRY,
            counter: FIXTURE_PREFIX + 5,
        },
        streaming_position: WorldPosition::new(EASTERN_KINGDOMS_MAP, -8855.0, -126.0, 81.9, 3.4),
        kobold: ExpectedCreature {
            entry: KOBOLD_VERMIN_ENTRY,
            counter: FIXTURE_PREFIX + 5,
        },
        kobold_position: WorldPosition::new(EASTERN_KINGDOMS_MAP, -8760.0, -121.0, 81.9, 3.4),
        kobold_targets: (0..5)
            .map(|index| ExpectedCreatureAt {
                creature: ExpectedCreature {
                    entry: KOBOLD_VERMIN_ENTRY,
                    counter: FIXTURE_PREFIX + 5 + index,
                },
                position: WorldPosition::new(
                    EASTERN_KINGDOMS_MAP,
                    -8760.0 + (index as f32 * 3.0),
                    -121.0,
                    81.9,
                    3.4,
                ),
            })
            .collect(),
        kobold_required_count: 5,
        wolf: ExpectedCreature {
            entry: YOUNG_WOLF_ENTRY,
            counter: FIXTURE_PREFIX + 4,
        },
        wolf_position: WorldPosition::new(EASTERN_KINGDOMS_MAP, -8908.0, -145.0, 82.2, 3.4),
        wolf_health: 4,
        wolf_loot_money: 3,
        wolf_loot_item: wolf_loot.map(|loot| loot.item),
        death_creature: death_test_creature(),
        death_position: death_test_position(),
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
        "Priest Fixture",
        5,
        53,
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
        LLANE_BESHERE_ENTRY,
        "Rust Llane Beshere",
        "Warrior Trainer Fixture",
        5,
        53,
        35,
        7,
        0x0000_0013,
        40,
        3.0,
        5.0,
        0,
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
        25,
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
            FIXTURE_PREFIX + 6,
            LLANE_BESHERE_ENTRY,
            -8920.0,
            -205.0,
            82.0,
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
    ] {
        seed_creature_spawn(world_pool, guid, entry, x, y, z, orientation).await?;
    }
    for index in 0..5 {
        seed_creature_spawn(
            world_pool,
            FIXTURE_PREFIX + 5 + index,
            KOBOLD_VERMIN_ENTRY,
            -8760.0 + (index as f32 * 3.0),
            -121.0,
            81.9,
            3.4,
        )
        .await?;
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
         VALUES (?, ?, 10, 0, 0, 1, 0)",
    )
    .bind(LLANE_BESHERE_ENTRY)
    .bind(WARRIOR_BATTLE_SHOUT_TRAINER_CAST)
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

async fn seed_death_test_fixture(world_pool: &MySqlPool) -> anyhow::Result<()> {
    seed_creature_template(
        world_pool,
        DEATH_TESTER_ENTRY,
        "Rust Death Proof",
        "Northshire Harness",
        3,
        KOBOLD_VERMIN_DISPLAY_ID,
        14,
        7,
        0,
        50_000,
        250.0,
        300.0,
        0,
        0,
        0,
        0,
    )
    .await?;
    let position = death_test_position();
    seed_creature_spawn(
        world_pool,
        DEATH_TESTER_GUID,
        DEATH_TESTER_ENTRY,
        position.x,
        position.y,
        position.z,
        position.orientation,
    )
    .await
}

fn death_test_creature() -> ExpectedCreature {
    ExpectedCreature {
        entry: DEATH_TESTER_ENTRY,
        counter: DEATH_TESTER_GUID,
    }
}

fn death_test_position() -> WorldPosition {
    WorldPosition::new(EASTERN_KINGDOMS_MAP, -9025.0, -132.0, 83.5, 0.0)
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
          Faction, Scale, Detection, Family, CreatureType, NpcFlags, UnitFlags, DynamicFlags, \
          Rank, MinLevelHealth, MaxLevelHealth, MinMeleeDmg, MaxMeleeDmg, \
          MeleeBaseAttackTime, RangedBaseAttackTime, TrainerType, TrainerClass, MinLootGold, MaxLootGold) \
         VALUES (?, ?, ?, ?, ?, ?, 100, ?, 1, 20, 0, ?, ?, 0, 0, 0, ?, ?, ?, ?, 2000, 2000, ?, ?, ?, ?)",
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
          RewMoneyMaxLevel, RewOrReqMoney) \
         VALUES (?, 2, ?, 1, 1, 1, ?, \
                 'Rust fixture quest detail for the Northshire starter-zone harness.', \
                 'Prove the Northshire quest data boundary exists.', \
                 'Good. Keep the fixture narrow until quest v1 lands.', \
                 'The Rust fixture is ready for the next quest slice.', \
                 ?, ?, ?, ?, 210, 25)",
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
        let streamed_spawns = wow_db::get_nearby_creature_spawns(
            world_pool,
            content.streaming_position.map_id,
            content.streaming_position.x,
            content.streaming_position.y,
            REAL_NORTHSHIRE_VISIBLE_RADIUS_YARDS,
            REAL_NORTHSHIRE_VISIBLE_LIMIT,
        )
        .await?;
        ensure!(
            streamed_spawns
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
        sqlx::query_scalar("SELECT spell FROM npc_trainer WHERE entry = ? AND spell = ?")
            .bind(LLANE_BESHERE_ENTRY)
            .bind(WARRIOR_BATTLE_SHOUT_TRAINER_CAST)
            .fetch_optional(world_pool)
            .await?;
    ensure!(
        trainer_spell == Some(WARRIOR_BATTLE_SHOUT_TRAINER_CAST),
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

async fn assert_kobold_camp_cleanup_persisted(
    character_pool: &MySqlPool,
    character_guid: u32,
    quest: u32,
) -> anyhow::Result<()> {
    let row: (u32, u8) = sqlx::query_as(
        "SELECT status, rewarded FROM character_queststatus WHERE guid = ? AND quest = ?",
    )
    .bind(character_guid)
    .bind(quest)
    .fetch_one(character_pool)
    .await?;
    ensure!(
        row.0 == QUEST_STATUS_COMPLETE && row.1 == 1,
        "Kobold Camp Cleanup did not persist as rewarded complete"
    );
    let progression: (u8, u32) = sqlx::query_as("SELECT level, xp FROM characters WHERE guid = ?")
        .bind(character_guid)
        .fetch_one(character_pool)
        .await?;
    ensure!(
        progression.0 >= 2 && progression.1 > 0,
        "Kobold Camp Cleanup flow did not persist a level-up with in-level XP: level={} xp={}",
        progression.0,
        progression.1
    );
    Ok(())
}

async fn assert_warrior_trainer_spell_persisted(
    character_pool: &MySqlPool,
    character_guid: u32,
    spell: u32,
) -> anyhow::Result<()> {
    let row: Option<(u8, u8)> =
        sqlx::query_as("SELECT active, disabled FROM character_spell WHERE guid = ? AND spell = ?")
            .bind(character_guid)
            .bind(spell)
            .fetch_optional(character_pool)
            .await?;
    ensure!(
        row == Some((1, 0)),
        "trainer spell {spell} was not persisted as active/enabled"
    );
    Ok(())
}

async fn assert_starter_zone_creature_respawns_persisted(
    character_pool: &MySqlPool,
    content: &StarterZoneContent,
) -> anyhow::Result<()> {
    let mut expected_guids = if content.source == StarterZoneSource::Fixture {
        vec![content.wolf.counter]
    } else {
        Vec::new()
    };
    expected_guids.extend(
        content
            .kobold_targets
            .iter()
            .take(content.kobold_required_count as usize)
            .map(|target| target.creature.counter),
    );
    expected_guids.sort_unstable();
    expected_guids.dedup();

    let now_epoch_secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_secs();
    let mut builder = QueryBuilder::new(
        "SELECT COUNT(DISTINCT guid) FROM creature_respawn WHERE instance = 0 AND respawntime > ",
    );
    builder.push_bind(now_epoch_secs);
    builder.push(" AND guid IN (");
    let mut separated = builder.separated(", ");
    for guid in &expected_guids {
        separated.push_bind(*guid);
    }
    separated.push_unseparated(")");

    let persisted_count: i64 = builder
        .build_query_scalar()
        .fetch_one(character_pool)
        .await?;
    ensure!(
        persisted_count as usize == expected_guids.len(),
        "starter-zone creature deaths did not persist respawn rows: expected={} actual={}",
        expected_guids.len(),
        persisted_count
    );
    Ok(())
}

async fn assert_player_death_reclaim_persisted(
    character_pool: &MySqlPool,
    character_guid: u32,
    proof: DeathReclaimProof,
) -> anyhow::Result<()> {
    let corpse_rows: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM corpse WHERE player = ?")
        .bind(character_guid)
        .fetch_one(character_pool)
        .await?;
    ensure!(
        corpse_rows == 0,
        "corpse row was not deleted after corpse reclaim: rows={corpse_rows}"
    );

    let row: (u32, u32, f32, f32, f32) = sqlx::query_as(
        "SELECT health, playerFlags, position_x, position_y, position_z \
         FROM characters WHERE guid = ?",
    )
    .bind(character_guid)
    .fetch_one(character_pool)
    .await?;
    ensure!(row.0 > 0, "reclaimed character health did not persist");
    ensure!(
        row.1 & PLAYER_FLAGS_GHOST == 0,
        "ghost flag persisted after corpse reclaim"
    );
    ensure!(
        (row.2 - proof.corpse_position.x).abs() < 0.25
            && (row.3 - proof.corpse_position.y).abs() < 0.25
            && (row.4 - proof.corpse_position.z).abs() < 0.25,
        "reclaimed character position did not persist at corpse: db=({}, {}, {}) corpse=({}, {}, {})",
        row.2,
        row.3,
        row.4,
        proof.corpse_position.x,
        proof.corpse_position.y,
        proof.corpse_position.z
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
    character_guid: u32,
}

#[derive(Debug, Default)]
struct XpProgressionEvidence {
    saw_creature_xp_log: bool,
    saw_quest_xp_log: bool,
    saw_levelup: bool,
    saw_progression_update: bool,
}

#[derive(Debug, Clone, Copy)]
struct DeathReclaimProof {
    corpse_position: WorldPosition,
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

        Ok(Self {
            stream,
            crypto,
            character_guid: 0,
        })
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
        self.character_guid = guid;
        let guid = ObjectGuid::new(HighGuid::Player, 0, guid);
        write_client_packet(
            &mut self.stream,
            CMSG_PLAYER_LOGIN,
            &guid.raw().to_le_bytes(),
            Some(&mut self.crypto),
        )?;

        let mut update_bodies = Vec::new();
        let expected_update_packets = if content.source == StarterZoneSource::RealClassicDb {
            3
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

    fn move_to_expect_streamed_creature(
        &mut self,
        content: &StarterZoneContent,
    ) -> anyhow::Result<()> {
        let streamed = ObjectGuid::new(
            HighGuid::Unit,
            content.streaming_creature.entry,
            content.streaming_creature.counter,
        );
        write_client_packet(
            &mut self.stream,
            CMSG_MOVE_HEARTBEAT,
            &movement_body(content.streaming_position),
            Some(&mut self.crypto),
        )?;

        let streamed_guid = streamed.raw().to_le_bytes();
        for _ in 0..80 {
            let (opcode, body) = read_server_packet(&mut self.stream, Some(&mut self.crypto))?;
            if opcode == SMSG_UPDATE_OBJECT
                && body
                    .windows(streamed_guid.len())
                    .any(|window| window == streamed_guid)
            {
                self.drain_immediate_packets()?;
                return Ok(());
            }
        }
        anyhow::bail!(
            "movement to x={} y={} did not stream creature entry={} counter={}",
            content.streaming_position.x,
            content.streaming_position.y,
            content.streaming_creature.entry,
            content.streaming_creature.counter
        )
    }

    fn move_to_expect_destroyed_creature(
        &mut self,
        content: &StarterZoneContent,
    ) -> anyhow::Result<()> {
        let streamed = ObjectGuid::new(
            HighGuid::Unit,
            content.streaming_creature.entry,
            content.streaming_creature.counter,
        );
        let start_position = WorldPosition::new(
            EASTERN_KINGDOMS_MAP,
            HUMAN_START_X,
            HUMAN_START_Y,
            83.5,
            0.0,
        );
        write_client_packet(
            &mut self.stream,
            CMSG_MOVE_HEARTBEAT,
            &movement_body(start_position),
            Some(&mut self.crypto),
        )?;

        for _ in 0..80 {
            let (opcode, body) = read_server_packet(&mut self.stream, Some(&mut self.crypto))?;
            if opcode == SMSG_DESTROY_OBJECT && body == streamed.raw().to_le_bytes() {
                self.drain_immediate_packets()?;
                return Ok(());
            }
        }
        anyhow::bail!(
            "movement back to start did not destroy streamed creature entry={} counter={}",
            content.streaming_creature.entry,
            content.streaming_creature.counter
        )
    }

    fn move_near_kobold_expect_no_aggro(
        &mut self,
        content: &StarterZoneContent,
    ) -> anyhow::Result<()> {
        let kobold = ObjectGuid::new(HighGuid::Unit, content.kobold.entry, content.kobold.counter);
        let player = ObjectGuid::new(HighGuid::Player, 0, self.character_guid);
        let aggro_position = WorldPosition::new(
            content.kobold_position.map_id,
            content.kobold_position.x + 6.0,
            content.kobold_position.y,
            content.kobold_position.z,
            std::f32::consts::PI,
        );
        write_client_packet(
            &mut self.stream,
            CMSG_MOVE_HEARTBEAT,
            &movement_body(aggro_position),
            Some(&mut self.crypto),
        )?;

        self.stream
            .set_read_timeout(Some(Duration::from_millis(75)))?;
        for _ in 0..12 {
            while let Some((opcode, body)) =
                try_read_server_packet(&mut self.stream, &mut self.crypto)?
            {
                if opcode == SMSG_ATTACKSTART {
                    ensure!(
                        body.len() != 16
                            || u64::from_le_bytes(body[0..8].try_into()?) != kobold.raw()
                            || u64::from_le_bytes(body[8..16].try_into()?) != player.raw(),
                        "neutral Kobold Vermin auto-aggroed the player"
                    );
                }
                ensure!(
                    opcode != SMSG_MONSTER_MOVE || !monster_move_matches(&body, kobold)?,
                    "neutral Kobold Vermin started a chase move without being attacked"
                );
                if opcode == SMSG_ATTACKERSTATEUPDATE
                    && attacker_state_update_matches(&body, kobold, player)?
                {
                    anyhow::bail!(
                        "neutral Kobold Vermin damaged the player without being attacked"
                    );
                }
            }
        }
        self.stream.set_read_timeout(Some(Duration::from_secs(5)))?;
        self.drain_immediate_packets()?;
        Ok(())
    }

    fn kill_loot_and_release_young_wolf(
        &mut self,
        content: &StarterZoneContent,
    ) -> anyhow::Result<()> {
        let wolf = ObjectGuid::new(HighGuid::Unit, content.wolf.entry, content.wolf.counter);
        let attack_positions = nearby_attack_positions(content.wolf_position);
        write_client_packet(
            &mut self.stream,
            CMSG_MOVE_HEARTBEAT,
            &movement_body(attack_positions[0]),
            Some(&mut self.crypto),
        )?;
        self.drain_immediate_packets()?;

        let mut saw_damage = false;
        let mut saw_dead_lootable = false;
        for attempt in 0..48 {
            let attack_position = attack_positions[attempt % attack_positions.len()];
            write_client_packet(
                &mut self.stream,
                CMSG_MOVE_HEARTBEAT,
                &movement_body(attack_position),
                Some(&mut self.crypto),
            )?;
            write_client_packet(
                &mut self.stream,
                CMSG_ATTACKSWING,
                &wolf.raw().to_le_bytes(),
                Some(&mut self.crypto),
            )?;
            self.stream
                .set_read_timeout(Some(Duration::from_millis(125)))?;
            while let Some((opcode, body)) =
                try_read_server_packet(&mut self.stream, &mut self.crypto)?
            {
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
            self.stream.set_read_timeout(Some(Duration::from_secs(5)))?;
            if saw_dead_lootable {
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

        let release_update = self.read_until(SMSG_UPDATE_OBJECT, 6)?;
        ensure!(
            update_packet_has_values(
                &release_update,
                wolf,
                &[(UNIT_FIELD_HEALTH, 0), (UNIT_DYNAMIC_FLAGS, 0)],
            )?,
            "Young Wolf loot release did not leave a non-lootable corpse"
        );

        self.stream
            .set_read_timeout(Some(Duration::from_millis(250)))?;
        while let Some((opcode, body)) = try_read_server_packet(&mut self.stream, &mut self.crypto)?
        {
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
                anyhow::bail!("Young Wolf respawned immediately after loot release");
            }
        }
        self.stream.set_read_timeout(Some(Duration::from_secs(5)))?;
        Ok(())
    }

    fn complete_kobold_camp_cleanup(&mut self, content: &StarterZoneContent) -> anyhow::Result<()> {
        let giver = ObjectGuid::new(
            HighGuid::Unit,
            content.quest_giver.entry,
            content.quest_giver.counter,
        );
        let player = ObjectGuid::new(HighGuid::Player, 0, self.character_guid);
        let mut xp_evidence = XpProgressionEvidence::default();

        write_client_packet(
            &mut self.stream,
            CMSG_QUESTGIVER_STATUS_QUERY,
            &giver.raw().to_le_bytes(),
            Some(&mut self.crypto),
        )?;
        let status = self.read_until(SMSG_QUESTGIVER_STATUS, 8)?;
        assert_questgiver_status(&status, giver, DIALOG_STATUS_AVAILABLE)?;

        write_client_packet(
            &mut self.stream,
            CMSG_GOSSIP_HELLO,
            &giver.raw().to_le_bytes(),
            Some(&mut self.crypto),
        )?;
        let quest_list = self.read_until(SMSG_QUESTGIVER_QUEST_LIST, 8)?;
        ensure!(
            quest_list
                .windows(4)
                .any(|window| window == content.kobold_quest.to_le_bytes()),
            "Kobold Camp Cleanup was missing from quest list"
        );

        write_client_packet(
            &mut self.stream,
            CMSG_QUESTGIVER_QUERY_QUEST,
            &questgiver_request_body(giver, content.kobold_quest),
            Some(&mut self.crypto),
        )?;
        let details = self.read_until(SMSG_QUESTGIVER_QUEST_DETAILS, 8)?;
        ensure!(
            details
                .windows(4)
                .any(|window| window == content.kobold_quest.to_le_bytes()),
            "Kobold Camp Cleanup details did not reference the quest id"
        );

        write_client_packet(
            &mut self.stream,
            CMSG_QUESTGIVER_ACCEPT_QUEST,
            &questgiver_request_body(giver, content.kobold_quest),
            Some(&mut self.crypto),
        )?;
        let accept_update = self.read_until(SMSG_UPDATE_OBJECT, 8)?;
        ensure!(
            update_packet_has_values(&accept_update, ObjectGuid::new(HighGuid::Player, 0, 0), &[],)
                .is_ok(),
            "accepted quest update was not parseable"
        );
        ensure!(
            content.kobold_targets.len() >= content.kobold_required_count as usize,
            "not enough distinct Kobold Vermin targets for non-instant-respawn quest proof"
        );

        for expected_count in 1..=content.kobold_required_count {
            let target = &content.kobold_targets[(expected_count - 1) as usize];
            let kobold = ObjectGuid::new(
                HighGuid::Unit,
                target.creature.entry,
                target.creature.counter,
            );
            let attack_positions = nearby_attack_positions(target.position);
            write_client_packet(
                &mut self.stream,
                CMSG_MOVE_HEARTBEAT,
                &movement_body(attack_positions[0]),
                Some(&mut self.crypto),
            )?;
            self.drain_immediate_packets()?;

            let mut saw_kill_credit = false;
            let mut saw_complete = false;
            for attempt in 0..96 {
                let attack_position = attack_positions[attempt % attack_positions.len()];
                write_client_packet(
                    &mut self.stream,
                    CMSG_MOVE_HEARTBEAT,
                    &movement_body(attack_position),
                    Some(&mut self.crypto),
                )?;
                write_client_packet(
                    &mut self.stream,
                    CMSG_ATTACKSWING,
                    &kobold.raw().to_le_bytes(),
                    Some(&mut self.crypto),
                )?;
                write_client_packet(
                    &mut self.stream,
                    CMSG_CAST_SPELL,
                    &cast_spell_body(HEROIC_STRIKE_RANK_1, kobold)?,
                    Some(&mut self.crypto),
                )?;
                self.stream
                    .set_read_timeout(Some(Duration::from_millis(125)))?;
                while let Some((opcode, body)) =
                    try_read_server_packet(&mut self.stream, &mut self.crypto)?
                {
                    observe_xp_progression_packet(opcode, &body, player, &mut xp_evidence)?;
                    if opcode == SMSG_QUESTUPDATE_ADD_KILL {
                        assert_quest_kill_update(
                            &body,
                            content.kobold_quest,
                            target.creature.entry,
                            expected_count,
                            content.kobold_required_count,
                            kobold,
                        )?;
                        saw_kill_credit = true;
                    }
                    if opcode == SMSG_QUESTUPDATE_COMPLETE {
                        ensure!(
                            u32::from_le_bytes(body[0..4].try_into()?) == content.kobold_quest,
                            "quest complete packet used wrong quest id"
                        );
                        saw_complete = true;
                    }
                    let saw_expected_quest_packets = saw_kill_credit
                        && (expected_count < content.kobold_required_count || saw_complete);
                    if saw_expected_quest_packets && xp_evidence.saw_creature_xp_log {
                        break;
                    }
                }
                self.stream.set_read_timeout(Some(Duration::from_secs(5)))?;
                let saw_expected_quest_packets = saw_kill_credit
                    && (expected_count < content.kobold_required_count || saw_complete);
                if saw_expected_quest_packets && xp_evidence.saw_creature_xp_log {
                    break;
                }
            }
            ensure!(
                saw_kill_credit,
                "Kobold kill {expected_count} did not grant quest credit"
            );
            write_client_packet(
                &mut self.stream,
                CMSG_LOOT_RELEASE,
                &kobold.raw().to_le_bytes(),
                Some(&mut self.crypto),
            )?;
            let _ = self.read_until_observing_xp(
                SMSG_LOOT_RELEASE_RESPONSE,
                8,
                player,
                &mut xp_evidence,
            )?;
        }

        write_client_packet(
            &mut self.stream,
            CMSG_QUESTGIVER_STATUS_QUERY,
            &giver.raw().to_le_bytes(),
            Some(&mut self.crypto),
        )?;
        let status =
            self.read_until_observing_xp(SMSG_QUESTGIVER_STATUS, 8, player, &mut xp_evidence)?;
        assert_questgiver_status(&status, giver, DIALOG_STATUS_REWARD2)?;

        write_client_packet(
            &mut self.stream,
            CMSG_GOSSIP_HELLO,
            &giver.raw().to_le_bytes(),
            Some(&mut self.crypto),
        )?;
        let offer = self.read_until_observing_xp(
            SMSG_QUESTGIVER_OFFER_REWARD,
            8,
            player,
            &mut xp_evidence,
        )?;
        ensure!(
            offer
                .windows(4)
                .any(|window| window == content.kobold_quest.to_le_bytes()),
            "Kobold Camp Cleanup offer reward did not reference the quest id"
        );

        let mut reward_body = questgiver_request_body(giver, content.kobold_quest);
        reward_body.extend_from_slice(&0u32.to_le_bytes());
        write_client_packet(
            &mut self.stream,
            CMSG_QUESTGIVER_CHOOSE_REWARD,
            &reward_body,
            Some(&mut self.crypto),
        )?;
        let complete = self.read_until_observing_xp(
            SMSG_QUESTGIVER_QUEST_COMPLETE,
            8,
            player,
            &mut xp_evidence,
        )?;
        ensure!(
            u32::from_le_bytes(complete[0..4].try_into()?) == content.kobold_quest,
            "quest reward completion packet used wrong quest id"
        );
        let reward_xp = u32::from_le_bytes(complete[4..8].try_into()?);
        ensure!(
            reward_xp > 0,
            "quest reward completion packet did not include XP"
        );

        self.stream
            .set_read_timeout(Some(Duration::from_millis(250)))?;
        for _ in 0..16 {
            let Some((opcode, body)) = try_read_server_packet(&mut self.stream, &mut self.crypto)?
            else {
                break;
            };
            observe_xp_progression_packet(opcode, &body, player, &mut xp_evidence)?;
            if xp_evidence.saw_creature_xp_log
                && xp_evidence.saw_quest_xp_log
                && xp_evidence.saw_levelup
                && xp_evidence.saw_progression_update
            {
                break;
            }
        }
        self.stream.set_read_timeout(Some(Duration::from_secs(5)))?;
        ensure!(
            xp_evidence.saw_creature_xp_log,
            "kobold kills did not send creature SMSG_LOG_XPGAIN"
        );
        ensure!(
            xp_evidence.saw_quest_xp_log,
            "quest reward did not send quest SMSG_LOG_XPGAIN"
        );
        ensure!(
            xp_evidence.saw_levelup,
            "Kobold Camp Cleanup flow did not send SMSG_LEVELUP_INFO"
        );
        ensure!(
            xp_evidence.saw_progression_update,
            "Kobold Camp Cleanup flow did not send level/next-XP player update"
        );
        Ok(())
    }

    fn learn_warrior_trainer_spell(&mut self, content: &StarterZoneContent) -> anyhow::Result<()> {
        let trainer = ObjectGuid::new(
            HighGuid::Unit,
            content.trainer.entry,
            content.trainer.counter,
        );
        write_client_packet(
            &mut self.stream,
            CMSG_TRAINER_LIST,
            &trainer.raw().to_le_bytes(),
            Some(&mut self.crypto),
        )?;
        let list = self.read_until(SMSG_TRAINER_LIST, 8)?;
        assert_trainer_list_has_green_spell(&list, trainer, content.trainer_spell)?;

        let mut buy = Vec::with_capacity(12);
        buy.extend_from_slice(&trainer.raw().to_le_bytes());
        buy.extend_from_slice(&content.trainer_spell.to_le_bytes());
        write_client_packet(
            &mut self.stream,
            CMSG_TRAINER_BUY_SPELL,
            &buy,
            Some(&mut self.crypto),
        )?;
        let success = self.read_until(SMSG_TRAINER_BUY_SUCCEEDED, 8)?;
        ensure!(
            success.len() == 12,
            "trainer buy success packet had wrong size"
        );
        ensure!(
            u64::from_le_bytes(success[0..8].try_into()?) == trainer.raw(),
            "trainer buy success packet used wrong trainer guid"
        );
        ensure!(
            u32::from_le_bytes(success[8..12].try_into()?) == content.trainer_spell,
            "trainer buy success packet used wrong spell"
        );
        let learned = self.read_until(SMSG_LEARNED_SPELL, 8)?;
        ensure!(learned.len() == 4, "learned spell packet had wrong size");
        ensure!(
            u32::from_le_bytes(learned[0..4].try_into()?) == WARRIOR_BATTLE_SHOUT_RANK_1,
            "learned spell packet used wrong spell"
        );
        Ok(())
    }

    fn die_release_and_reclaim_corpse(
        &mut self,
        content: &StarterZoneContent,
    ) -> anyhow::Result<DeathReclaimProof> {
        let player = ObjectGuid::new(HighGuid::Player, 0, self.character_guid);
        let killer = ObjectGuid::new(
            HighGuid::Unit,
            content.death_creature.entry,
            content.death_creature.counter,
        );
        let corpse = ObjectGuid::new(HighGuid::Corpse, 0, self.character_guid);
        let attack_positions = nearby_attack_positions(content.death_position);
        let corpse_position = attack_positions[0];

        write_client_packet(
            &mut self.stream,
            CMSG_MOVE_HEARTBEAT,
            &movement_body(corpse_position),
            Some(&mut self.crypto),
        )?;
        self.drain_immediate_packets()?;

        let mut saw_death_damage = false;
        let mut saw_player_dead = false;
        for attempt in 0..80 {
            let attack_position = attack_positions[attempt % attack_positions.len()];
            write_client_packet(
                &mut self.stream,
                CMSG_MOVE_HEARTBEAT,
                &movement_body(attack_position),
                Some(&mut self.crypto),
            )?;
            self.stream
                .set_read_timeout(Some(Duration::from_millis(150)))?;
            while let Some((opcode, body)) =
                try_read_server_packet(&mut self.stream, &mut self.crypto)?
            {
                if opcode == SMSG_ATTACKERSTATEUPDATE
                    && attacker_state_update_matches(&body, killer, player)?
                {
                    saw_death_damage = true;
                }
                if opcode == SMSG_UPDATE_OBJECT
                    && update_packet_has_values_or_false(&body, player, &[(UNIT_FIELD_HEALTH, 0)])
                {
                    saw_player_dead = true;
                    break;
                }
            }
            if saw_player_dead {
                break;
            }
        }
        self.stream.set_read_timeout(Some(Duration::from_secs(5)))?;
        ensure!(
            saw_death_damage,
            "death proof creature did not damage the player"
        );
        ensure!(
            saw_player_dead,
            "death proof creature did not trigger player death"
        );

        write_client_packet(
            &mut self.stream,
            CMSG_REPOP_REQUEST,
            &[],
            Some(&mut self.crypto),
        )?;
        let mut saw_ghost_update = false;
        let mut saw_corpse_object = false;
        let mut saw_reclaim_delay = false;
        let mut saw_release_teleport = false;
        for _ in 0..48 {
            let (opcode, body) = read_server_packet(&mut self.stream, Some(&mut self.crypto))?;
            match opcode {
                SMSG_UPDATE_OBJECT => {
                    if update_packet_has_values_or_false(
                        &body,
                        player,
                        &[
                            (UNIT_FIELD_HEALTH, 1),
                            (PLAYER_FLAGS_FIELD, PLAYER_FLAGS_GHOST),
                        ],
                    ) {
                        saw_ghost_update = true;
                    }
                    let corpse_bytes = corpse.raw().to_le_bytes();
                    if body
                        .windows(corpse_bytes.len())
                        .any(|window| window == corpse_bytes)
                    {
                        saw_corpse_object = true;
                    }
                }
                SMSG_CORPSE_RECLAIM_DELAY => saw_reclaim_delay = true,
                MSG_MOVE_TELEPORT_ACK => saw_release_teleport = true,
                _ => {}
            }
            if saw_ghost_update && saw_corpse_object && saw_reclaim_delay && saw_release_teleport {
                break;
            }
        }
        ensure!(saw_ghost_update, "release did not send ghost player update");
        ensure!(
            saw_corpse_object,
            "release did not create a player corpse object"
        );
        ensure!(
            saw_reclaim_delay,
            "release did not send corpse reclaim delay"
        );
        ensure!(
            saw_release_teleport,
            "release did not teleport the ghost to a graveyard"
        );

        write_client_packet(
            &mut self.stream,
            MSG_CORPSE_QUERY,
            &[],
            Some(&mut self.crypto),
        )?;
        let query = self.read_until(MSG_CORPSE_QUERY, 12)?;
        assert_corpse_query_points_to(&query, corpse_position)?;

        write_client_packet(
            &mut self.stream,
            CMSG_MOVE_HEARTBEAT,
            &movement_body(corpse_position),
            Some(&mut self.crypto),
        )?;
        self.drain_immediate_packets()?;
        write_client_packet(
            &mut self.stream,
            CMSG_RECLAIM_CORPSE,
            &corpse.raw().to_le_bytes(),
            Some(&mut self.crypto),
        )?;

        let mut saw_alive_update = false;
        let mut saw_bones_update = false;
        let mut saw_reclaim_teleport = false;
        for _ in 0..48 {
            let (opcode, body) = read_server_packet(&mut self.stream, Some(&mut self.crypto))?;
            match opcode {
                SMSG_UPDATE_OBJECT => {
                    if update_packet_has_values_or_false(&body, player, &[(PLAYER_FLAGS_FIELD, 0)])
                    {
                        saw_alive_update = true;
                    }
                    if update_packet_has_values_or_false(
                        &body,
                        corpse,
                        &[(CORPSE_FIELD_FLAGS, CORPSE_FLAG_BONES)],
                    ) {
                        saw_bones_update = true;
                    }
                }
                MSG_MOVE_TELEPORT_ACK => saw_reclaim_teleport = true,
                _ => {}
            }
            if saw_alive_update && saw_bones_update && saw_reclaim_teleport {
                break;
            }
        }
        ensure!(saw_alive_update, "corpse reclaim did not clear ghost flags");
        ensure!(
            saw_bones_update,
            "corpse reclaim did not convert corpse to bones"
        );
        ensure!(
            saw_reclaim_teleport,
            "corpse reclaim did not send final teleport/movement ack"
        );
        Ok(DeathReclaimProof { corpse_position })
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

    fn read_until_observing_xp(
        &mut self,
        expected_opcode: u32,
        max_packets: usize,
        player: ObjectGuid,
        evidence: &mut XpProgressionEvidence,
    ) -> anyhow::Result<Vec<u8>> {
        for _ in 0..max_packets {
            let (opcode, body) = read_server_packet(&mut self.stream, Some(&mut self.crypto))?;
            observe_xp_progression_packet(opcode, &body, player, evidence)?;
            if opcode == expected_opcode {
                return Ok(body);
            }
        }
        anyhow::bail!("did not receive expected opcode 0x{expected_opcode:04X}");
    }

    fn drain_immediate_packets(&mut self) -> anyhow::Result<()> {
        self.stream
            .set_read_timeout(Some(Duration::from_millis(25)))?;
        while try_read_server_packet(&mut self.stream, &mut self.crypto)?.is_some() {}
        self.stream.set_read_timeout(Some(Duration::from_secs(5)))?;
        Ok(())
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

fn assert_corpse_query_points_to(body: &[u8], expected: WorldPosition) -> anyhow::Result<()> {
    ensure!(body.len() == 21, "corpse query response had wrong size");
    ensure!(body[0] == 1, "corpse query did not report a corpse");
    ensure!(
        i32::from_le_bytes(body[1..5].try_into()?) == expected.map_id as i32,
        "corpse query used wrong map"
    );
    let x = f32::from_le_bytes(body[5..9].try_into()?);
    let y = f32::from_le_bytes(body[9..13].try_into()?);
    let z = f32::from_le_bytes(body[13..17].try_into()?);
    ensure!(
        (x - expected.x).abs() < 0.25
            && (y - expected.y).abs() < 0.25
            && (z - expected.z).abs() < 0.25,
        "corpse query did not point to the death position: query=({x}, {y}, {z}) expected=({}, {}, {})",
        expected.x,
        expected.y,
        expected.z
    );
    ensure!(
        u32::from_le_bytes(body[17..21].try_into()?) == expected.map_id,
        "corpse query used wrong corpse map id"
    );
    Ok(())
}

fn questgiver_request_body(giver: ObjectGuid, quest: u32) -> Vec<u8> {
    let mut body = Vec::with_capacity(12);
    body.extend_from_slice(&giver.raw().to_le_bytes());
    body.extend_from_slice(&quest.to_le_bytes());
    body
}

fn movement_body(position: WorldPosition) -> Vec<u8> {
    let mut body = Vec::with_capacity(28);
    body.extend_from_slice(&0u32.to_le_bytes());
    body.extend_from_slice(&1_000u32.to_le_bytes());
    body.extend_from_slice(&position.x.to_le_bytes());
    body.extend_from_slice(&position.y.to_le_bytes());
    body.extend_from_slice(&position.z.to_le_bytes());
    body.extend_from_slice(&position.orientation.to_le_bytes());
    body.extend_from_slice(&0u32.to_le_bytes());
    body
}

fn nearby_attack_positions(target: WorldPosition) -> Vec<WorldPosition> {
    let mut positions = Vec::with_capacity(64);
    for radius in [3.0_f32, 5.0, 7.0, 10.0] {
        for index in 0..8 {
            let angle = index as f32 * std::f32::consts::FRAC_PI_4;
            let x = target.x + angle.cos() * radius;
            let y = target.y + angle.sin() * radius;
            let inward_facing = (target.y - y).atan2(target.x - x);
            positions.push(WorldPosition::new(
                target.map_id,
                x,
                y,
                target.z,
                inward_facing,
            ));
            positions.push(WorldPosition::new(target.map_id, x, y, target.z, angle));
        }
    }
    positions
}

fn cast_spell_body(spell_id: u32, target: ObjectGuid) -> anyhow::Result<Vec<u8>> {
    let mut body = Vec::with_capacity(16);
    body.extend_from_slice(&spell_id.to_le_bytes());
    body.extend_from_slice(&SPELL_CAST_TARGET_UNIT.to_le_bytes());
    write_packed_guid(&mut body, target)?;
    Ok(body)
}

fn write_packed_guid(body: &mut Vec<u8>, guid: ObjectGuid) -> anyhow::Result<()> {
    let raw = guid.raw();
    let mut mask = 0u8;
    let mut bytes = Vec::new();
    for index in 0..8 {
        let byte = ((raw >> (index * 8)) & 0xFF) as u8;
        if byte != 0 {
            mask |= 1 << index;
            bytes.push(byte);
        }
    }
    body.push(mask);
    body.extend_from_slice(&bytes);
    Ok(())
}

fn assert_questgiver_status(body: &[u8], giver: ObjectGuid, expected: u32) -> anyhow::Result<()> {
    ensure!(
        body.len() == 12,
        "questgiver status response had wrong size"
    );
    ensure!(
        u64::from_le_bytes(body[0..8].try_into()?) == giver.raw(),
        "questgiver status response used wrong guid"
    );
    ensure!(
        u32::from_le_bytes(body[8..12].try_into()?) == expected,
        "questgiver status response had wrong dialog status"
    );
    Ok(())
}

fn assert_quest_kill_update(
    body: &[u8],
    quest: u32,
    entry: u32,
    count: u32,
    required_count: u32,
    killed: ObjectGuid,
) -> anyhow::Result<()> {
    ensure!(body.len() == 24, "quest kill update had wrong size");
    ensure!(
        u32::from_le_bytes(body[0..4].try_into()?) == quest,
        "quest kill update used wrong quest"
    );
    ensure!(
        u32::from_le_bytes(body[4..8].try_into()?) == entry,
        "quest kill update used wrong creature entry"
    );
    ensure!(
        u32::from_le_bytes(body[8..12].try_into()?) == count,
        "quest kill update used wrong current count"
    );
    ensure!(
        u32::from_le_bytes(body[12..16].try_into()?) == required_count,
        "quest kill update used wrong required count"
    );
    ensure!(
        u64::from_le_bytes(body[16..24].try_into()?) == killed.raw(),
        "quest kill update used wrong killed guid"
    );
    Ok(())
}

fn attacker_state_update_matches(
    body: &[u8],
    attacker: ObjectGuid,
    victim: ObjectGuid,
) -> anyhow::Result<bool> {
    ensure_available(body, 4)?;
    let mut cursor = 4;
    let parsed_attacker = read_packed_update_guid(body, &mut cursor)?;
    let parsed_victim = read_packed_update_guid(body, &mut cursor)?;
    Ok(parsed_attacker == attacker.raw() && parsed_victim == victim.raw())
}

fn monster_move_matches(body: &[u8], mover: ObjectGuid) -> anyhow::Result<bool> {
    let mut cursor = 0;
    let parsed_mover = read_packed_update_guid(body, &mut cursor)?;
    ensure_available(body, cursor + 12 + 4 + 1 + 4 + 4 + 4 + 12)?;
    let move_type = body[cursor + 12 + 4];
    let point_count_offset = cursor + 12 + 4 + 1 + 4 + 4;
    let point_count =
        u32::from_le_bytes(body[point_count_offset..point_count_offset + 4].try_into()?);
    Ok(parsed_mover == mover.raw() && move_type == 0 && point_count >= 1)
}

fn observe_xp_progression_packet(
    opcode: u32,
    body: &[u8],
    player: ObjectGuid,
    evidence: &mut XpProgressionEvidence,
) -> anyhow::Result<()> {
    match opcode {
        SMSG_LOG_XPGAIN => {
            ensure!(body.len() >= 13, "XP gain log was too short");
            ensure!(
                u32::from_le_bytes(body[8..12].try_into()?) > 0,
                "XP gain log did not report positive XP"
            );
            let has_source_guid = u64::from_le_bytes(body[0..8].try_into()?) != 0;
            if has_source_guid {
                evidence.saw_creature_xp_log = true;
            } else {
                evidence.saw_quest_xp_log = true;
            }
        }
        SMSG_LEVELUP_INFO => {
            ensure!(body.len() == 48, "level-up info packet had wrong size");
            ensure!(
                u32::from_le_bytes(body[0..4].try_into()?) >= 2,
                "level-up info did not report level 2+"
            );
            evidence.saw_levelup = true;
        }
        SMSG_UPDATE_OBJECT
            if update_packet_has_values(
                body,
                player,
                &[(UNIT_FIELD_LEVEL, 2), (PLAYER_NEXT_LEVEL_XP, 900)],
            )? =>
        {
            evidence.saw_progression_update = true;
        }
        _ => {}
    }
    Ok(())
}

fn assert_trainer_list_has_green_spell(
    body: &[u8],
    trainer: ObjectGuid,
    expected_spell: u32,
) -> anyhow::Result<()> {
    ensure!(body.len() >= 16, "trainer list was too short");
    ensure!(
        u64::from_le_bytes(body[0..8].try_into()?) == trainer.raw(),
        "trainer list used wrong trainer guid"
    );
    let count = u32::from_le_bytes(body[12..16].try_into()?) as usize;
    let mut cursor = 16;
    let mut found = false;
    for _ in 0..count {
        ensure_available(body, cursor + 38)?;
        let spell = u32::from_le_bytes(body[cursor..cursor + 4].try_into()?);
        let state = body[cursor + 4];
        if spell == expected_spell {
            ensure!(
                state == 0,
                "trainer spell {expected_spell} was not learnable"
            );
            found = true;
        }
        cursor += 38;
    }
    ensure_available(body, cursor + 1)?;
    ensure!(
        body[cursor..].contains(&0),
        "trainer list greeting was not NUL-terminated"
    );
    ensure!(found, "trainer list did not contain spell {expected_spell}");
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
            _ => return Ok(false),
        }
    }
    Ok(false)
}

fn update_packet_has_values_or_false(
    body: &[u8],
    guid: ObjectGuid,
    expected: &[(usize, u32)],
) -> bool {
    update_packet_has_values(body, guid, expected).unwrap_or(false)
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

fn try_read_server_packet(
    stream: &mut TcpStream,
    crypto: &mut HeaderCrypto,
) -> anyhow::Result<Option<(u32, Vec<u8>)>> {
    match read_server_packet(stream, Some(crypto)) {
        Ok(packet) => Ok(Some(packet)),
        Err(error) => {
            if error
                .downcast_ref::<std::io::Error>()
                .is_some_and(|io| matches!(io.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut))
            {
                Ok(None)
            } else {
                Err(error)
            }
        }
    }
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
