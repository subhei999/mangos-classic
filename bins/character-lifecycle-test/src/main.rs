use anyhow::{ensure, Context};
use sqlx::mysql::{MySqlPool, MySqlPoolOptions};
use wow_db::{CharacterDeleteMethod, CharacterDeleteOptions, CreatedCharacter, NewCharacter};

const LOGIN_DATABASE_URL: &str = "mysql://mangos:mangos@127.0.0.1:3307/realmd";
const CHARACTER_DATABASE_URL: &str = "mysql://mangos:mangos@127.0.0.1:3307/characters";
const WORLD_DATABASE_URL: &str = "mysql://mangos:mangos@127.0.0.1:3307/mangos";
const USERNAME: &str = "CODEXLIFE";
const CHARACTER_NAME: &str = "Codexlife";
const REALM_ID: u32 = 1;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let login_pool = connect(LOGIN_DATABASE_URL)
        .await
        .context("connect to realmd")?;
    let character_pool = connect(CHARACTER_DATABASE_URL)
        .await
        .context("connect to characters")?;
    let world_pool = connect(WORLD_DATABASE_URL)
        .await
        .context("connect to mangos")?;

    let account_id = seed_account(&login_pool).await?;
    cleanup_account(&login_pool, &character_pool, account_id).await?;

    let created = create_lifecycle_character(&character_pool, &world_pool, account_id).await?;
    wow_db::refresh_realm_character_count(&login_pool, &character_pool, account_id, REALM_ID)
        .await?;

    assert_count_row(&login_pool, account_id, 1).await?;
    assert_no_reversed_count_row(&login_pool, account_id).await?;
    assert_character_visible(&character_pool, account_id, created.guid).await?;
    assert_starter_inventory_present(&character_pool, created.guid).await?;

    ensure!(
        wow_db::delete_character(&character_pool, account_id, created.guid).await?,
        "delete_character returned false for owned test character"
    );
    wow_db::refresh_realm_character_count(&login_pool, &character_pool, account_id, REALM_ID)
        .await?;

    assert_count_row(&login_pool, account_id, 0).await?;
    assert_character_deleted(&character_pool, account_id, created.guid).await?;
    assert_starter_inventory_deleted(&character_pool, created.guid).await?;

    assert_soft_delete_unlinks_character(&character_pool, &world_pool, account_id).await?;
    assert_race_class_matrix_cleanup(&character_pool, &world_pool, account_id).await?;

    println!("character lifecycle check passed: create, enum, count refresh, starter items, delete cleanup, soft delete, race/class cleanup");
    Ok(())
}

async fn connect(url: &str) -> anyhow::Result<MySqlPool> {
    Ok(MySqlPoolOptions::new()
        .max_connections(2)
        .connect(url)
        .await?)
}

async fn seed_account(login_pool: &MySqlPool) -> anyhow::Result<u32> {
    sqlx::query(
        "INSERT INTO account (username, gmlevel, sessionkey, v, s, email, locked, expansion, locale, os) \
         VALUES (?, 0, '', '', '', '', 0, 0, '', 'Win') \
         ON DUPLICATE KEY UPDATE username = VALUES(username), locked = 0, os = 'Win'",
    )
    .bind(USERNAME)
    .execute(login_pool)
    .await
    .context("seed lifecycle account")?;

    let account_id = sqlx::query_scalar("SELECT id FROM account WHERE username = ?")
        .bind(USERNAME)
        .fetch_one(login_pool)
        .await
        .context("fetch lifecycle account id")?;
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

    sqlx::query("DELETE FROM realmcharacters WHERE acctid = ?")
        .bind(account_id)
        .execute(login_pool)
        .await
        .context("clear lifecycle realm character rows")?;

    if account_id != REALM_ID {
        sqlx::query("DELETE FROM realmcharacters WHERE realmid = ? AND acctid = ?")
            .bind(account_id)
            .bind(REALM_ID)
            .execute(login_pool)
            .await
            .context("clear stale reversed lifecycle count row")?;
    }

    Ok(())
}

async fn create_lifecycle_character(
    character_pool: &MySqlPool,
    world_pool: &MySqlPool,
    account_id: u32,
) -> anyhow::Result<CreatedCharacter> {
    wow_db::create_character(
        character_pool,
        world_pool,
        NewCharacter {
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
    .await
    .context("create lifecycle character")
}

async fn create_character(
    character_pool: &MySqlPool,
    world_pool: &MySqlPool,
    account_id: u32,
    name: &str,
    race: u8,
    class: u8,
) -> anyhow::Result<CreatedCharacter> {
    wow_db::create_character(
        character_pool,
        world_pool,
        NewCharacter {
            account_id,
            name: name.to_string(),
            race,
            class,
            gender: 0,
            skin: 0,
            face: 0,
            hair_style: 0,
            hair_color: 0,
            facial_hair: 0,
        },
    )
    .await
    .with_context(|| format!("create matrix character {name}"))
}

async fn assert_soft_delete_unlinks_character(
    character_pool: &MySqlPool,
    world_pool: &MySqlPool,
    account_id: u32,
) -> anyhow::Result<()> {
    let created =
        create_character(character_pool, world_pool, account_id, "Softlife", 1, 1).await?;
    ensure!(
        wow_db::delete_character_with_options(
            character_pool,
            account_id,
            created.guid,
            CharacterDeleteOptions {
                method: CharacterDeleteMethod::Unlink,
                min_level_for_unlink: 1,
                force_hard_delete: false,
            },
        )
        .await?,
        "soft delete returned false for owned test character"
    );

    let row: (u32, String, Option<u32>, Option<String>, Option<u64>) = sqlx::query_as(
        "SELECT account, name, deleteInfos_Account, deleteInfos_Name, deleteDate \
         FROM characters WHERE guid = ?",
    )
    .bind(created.guid)
    .fetch_one(character_pool)
    .await?;
    ensure!(
        row.0 == 0,
        "soft-deleted character account was not unlinked"
    );
    ensure!(
        row.1.is_empty(),
        "soft-deleted character name was not cleared"
    );
    ensure!(
        row.2 == Some(account_id),
        "soft-deleted character account backup was wrong"
    );
    ensure!(
        row.3.as_deref() == Some("Softlife"),
        "soft-deleted character name backup was wrong"
    );
    ensure!(row.4.is_some(), "soft-deleted character deleteDate missing");

    ensure!(
        wow_db::delete_character_with_options(
            character_pool,
            0,
            created.guid,
            CharacterDeleteOptions::hard_delete(),
        )
        .await?,
        "hard cleanup of soft-deleted character failed"
    );
    Ok(())
}

async fn assert_race_class_matrix_cleanup(
    character_pool: &MySqlPool,
    world_pool: &MySqlPool,
    account_id: u32,
) -> anyhow::Result<()> {
    for (name, race, class, expected_items) in [
        ("Matrixorc", 2, 3, &[6948, 2512, 2101][..]),
        ("Matrixdruid", 4, 11, &[6948, 3661][..]),
        ("Matrixmage", 7, 8, &[6948, 35][..]),
    ] {
        let created =
            create_character(character_pool, world_pool, account_id, name, race, class).await?;
        let inventory = wow_db::get_character_inventory_items(character_pool, created.guid).await?;
        for expected in expected_items {
            ensure!(
                inventory.iter().any(|item| item.item_template == *expected),
                "starter item {expected} missing for {name}"
            );
        }
        ensure!(
            wow_db::delete_character(character_pool, account_id, created.guid).await?,
            "matrix delete failed for {name}"
        );
        assert_starter_inventory_deleted(character_pool, created.guid).await?;
    }
    Ok(())
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
            .await
            .context("fetch lifecycle realm character count")?;

    ensure!(
        actual == Some(expected),
        "realmcharacters count for acctid={account_id} was {:?}, expected {expected}",
        actual
    );
    Ok(())
}

async fn assert_no_reversed_count_row(
    login_pool: &MySqlPool,
    account_id: u32,
) -> anyhow::Result<()> {
    if account_id == REALM_ID {
        return Ok(());
    }

    let reversed: Option<u8> =
        sqlx::query_scalar("SELECT numchars FROM realmcharacters WHERE realmid = ? AND acctid = ?")
            .bind(account_id)
            .bind(REALM_ID)
            .fetch_optional(login_pool)
            .await
            .context("fetch reversed lifecycle realm character count")?;

    ensure!(
        reversed.is_none(),
        "found reversed realmcharacters row realmid={account_id} acctid={REALM_ID}: {:?}",
        reversed
    );
    Ok(())
}

async fn assert_character_visible(
    character_pool: &MySqlPool,
    account_id: u32,
    guid: u32,
) -> anyhow::Result<()> {
    let characters = wow_db::get_character_enum_entries(character_pool, account_id).await?;
    ensure!(
        characters
            .iter()
            .any(|character| character.guid == guid && character.name == CHARACTER_NAME),
        "created character was not visible in enum rows"
    );
    Ok(())
}

async fn assert_character_deleted(
    character_pool: &MySqlPool,
    account_id: u32,
    guid: u32,
) -> anyhow::Result<()> {
    let characters = wow_db::get_character_enum_entries(character_pool, account_id).await?;
    ensure!(
        characters.iter().all(|character| character.guid != guid),
        "deleted character was still visible in enum rows"
    );
    Ok(())
}

async fn assert_starter_inventory_present(
    character_pool: &MySqlPool,
    guid: u32,
) -> anyhow::Result<()> {
    let inventory = wow_db::get_character_inventory_items(character_pool, guid).await?;
    ensure!(
        inventory.iter().any(|item| item.item_template == 6948),
        "starter hearthstone was not created"
    );
    ensure!(
        inventory
            .iter()
            .any(|item| item.slot == 15 && item.item_template == 25),
        "starter weapon was not created"
    );
    Ok(())
}

async fn assert_starter_inventory_deleted(
    character_pool: &MySqlPool,
    guid: u32,
) -> anyhow::Result<()> {
    let inventory_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM character_inventory WHERE guid = ?")
            .bind(guid)
            .fetch_one(character_pool)
            .await
            .context("count deleted character inventory rows")?;
    let item_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM item_instance WHERE owner_guid = ?")
            .bind(guid)
            .fetch_one(character_pool)
            .await
            .context("count deleted character item instances")?;

    ensure!(inventory_count == 0, "inventory rows remained after delete");
    ensure!(item_count == 0, "item instances remained after delete");
    Ok(())
}
