use rand::Rng;
use sha1::{Digest, Sha1};
use sqlx::mysql::MySqlPool;
use std::collections::{HashMap, HashSet};
use std::io::Cursor;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, RwLock};
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::net::tcp::OwnedWriteHalf;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{mpsc, Mutex};
use tokio::time::{timeout, Duration, Instant};
use tracing::{debug, error, info, warn};
use wow_common::guid::{write_guid, HighGuid, ObjectGuid, PackedGuid};
use wow_common::position::WorldPosition;
use wow_crypto::HeaderCrypto;
use wow_db::{
    CharacterAction, CharacterDeleteOptions, CharacterEnumEntry, CharacterInventoryItem,
    CharacterNameQuery, CharacterQuestStatus, CharacterReputation, CharacterReputationChange,
    CharacterSkill, CharacterSpell, CreatureLootQuery, CreatureSpawnQuery, CreatureTemplateQuery,
    ItemTemplateQuery, NewCharacter, NewPlayerCorpse, PlayerCorpseQuery, PlayerWorldStats,
    QuestTemplateQuery,
};

include!("opcodes.rs");
include!("game_events.rs");
include!("scripts.rs");
include!("globals/object_mgr.rs");
include!("globals/conditions.rs");
include!("session.rs");
include!("fixtures/legacy_npcs.rs");
include!("server/world_session.rs");
include!("entities/update_data.rs");
include!("server/runtime_helpers.rs");
include!("server/map_update.rs");
include!("server/session_loop.rs");
include!("server/character_screen.rs");
include!("server/player_login.rs");
include!("server/logout.rs");
include!("server/movement.rs");
include!("server/visibility.rs");
include!("server/action_buttons.rs");
include!("playerbots.rs");

pub struct WorldServer {
    bind_addr: SocketAddr,
    login_db_pool: MySqlPool,
    character_db_pool: MySqlPool,
    world_db_pool: MySqlPool,
    runtime_state: WorldRuntimeState,
}

pub struct WorldServerOptions {
    pub data_dir: PathBuf,
    pub world_tick_interval: Duration,
    pub playerbots: Vec<PlayerbotSpawnConfig>,
}

impl WorldServer {
    pub async fn new(
        bind_addr: SocketAddr,
        login_db_pool: MySqlPool,
        character_db_pool: MySqlPool,
        world_db_pool: MySqlPool,
        delete_options: CharacterDeleteOptions,
        options: WorldServerOptions,
    ) -> anyhow::Result<Self> {
        let WorldServerOptions {
            data_dir,
            world_tick_interval,
            playerbots,
        } = options;
        if world_tick_interval.is_zero() {
            anyhow::bail!("world tick interval must be greater than 0");
        }
        let world_data_files = Arc::new(WorldDataFiles::inspect(data_dir));
        let game_event_schedules = wow_db::get_game_event_schedules(&world_db_pool).await?;
        let now_unix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;
        let game_events = GameEventState::from_schedules_at(&game_event_schedules, now_unix);
        let creature_cache_load_started_at = Instant::now();
        let mut creature_spawns = wow_db::get_all_static_creature_spawns(&world_db_pool).await?;
        apply_creature_display_scale_fallbacks(
            &mut creature_spawns,
            &world_data_files.creature_display_scales,
        );
        let creature_cache_load_duration = creature_cache_load_started_at.elapsed();
        let gameobject_cache_load_started_at = Instant::now();
        let gameobject_spawns = wow_db::get_all_static_gameobject_spawns(&world_db_pool).await?;
        let gameobject_cache_load_duration = gameobject_cache_load_started_at.elapsed();
        let db_scripts = Arc::new(DbScriptRegistry::load(&world_db_pool).await?);
        let next_gm_creature_guid = creature_spawns
            .iter()
            .map(|spawn| spawn.guid as u64)
            .max()
            .unwrap_or(0)
            .saturating_add(1);
        let static_world_cache = Arc::new(StaticWorldSpawnCache::from_spawns_for_game_events(
            creature_spawns,
            gameobject_spawns,
            &game_events,
        ));
        let static_cache_counts = static_world_cache.counts();
        crate::observability::record_static_world_cache_load(
            crate::observability::StaticWorldCacheKind::Creature,
            static_cache_counts.creature_spawns,
            static_cache_counts.creature_grids,
            creature_cache_load_duration,
        );
        crate::observability::record_static_world_cache_load(
            crate::observability::StaticWorldCacheKind::GameObject,
            static_cache_counts.gameobject_spawns,
            static_cache_counts.gameobject_grids,
            gameobject_cache_load_duration,
        );
        let maps = Arc::new(
            MapRuntimeManager::with_world_data_files_static_cache_and_next_gm_guid(
                &world_data_files,
                static_world_cache,
                next_gm_creature_guid,
                db_scripts,
            ),
        );
        let playerbots = Arc::new(initialize_playerbots(&maps, &world_db_pool, &playerbots).await?);
        info!(
            data_dir = %world_data_files.data_dir.display(),
            maps = world_data_files.maps_available,
            vmaps = world_data_files.vmaps_available,
            creature_display_scales = world_data_files.creature_display_scales.len(),
            faction_templates = world_data_files.faction_templates.len(),
            faction_templates_dbc_backed = world_data_files.faction_templates.is_dbc_backed(),
            item_random_properties = world_data_files.item_random_properties.len(),
            mmap_maps = world_data_files.mmap_headers.len(),
            mmap_tiles = world_data_files.mmap_tiles.len(),
            vmap_maps = world_data_files.vmap_trees.len(),
            vmap_tiles = world_data_files.vmap_tiles.len(),
            game_event_schedules = game_event_schedules.len(),
            active_game_events = game_events.active_count(),
            static_creature_spawns = static_cache_counts.creature_spawns,
            static_creature_grids = static_cache_counts.creature_grids,
            static_gameobject_spawns = static_cache_counts.gameobject_spawns,
            static_gameobject_grids = static_cache_counts.gameobject_grids,
            static_creature_cache_load_ms = creature_cache_load_duration.as_secs_f64() * 1_000.0,
            static_gameobject_cache_load_ms = gameobject_cache_load_duration.as_secs_f64() * 1_000.0,
            "World data files inspected",
        );
        if world_data_files.mmap_tiles.is_empty() {
            warn!(
                data_dir = %world_data_files.data_dir.display(),
                "No mmap tiles found; DB creature generated movement and path-gated aggro checks will stay unavailable",
            );
        }
        if !world_data_files.faction_templates.is_dbc_backed() {
            warn!(
                data_dir = %world_data_files.data_dir.display(),
                "FactionTemplate.dbc was not loaded; creature sight aggro will use the limited test/bootstrap faction bridge",
            );
        }
        if world_data_files.vmaps_available && world_data_files.vmap_tiles.is_empty() {
            warn!(
                data_dir = %world_data_files.data_dir.display(),
                "No compatible CMaNGOS VMAP_7.0 tiles found; DB creature line-of-sight will use the permissive fallback",
            );
        }
        let object_mgr = Arc::new(ObjectMgr::default());
        object_mgr.load_conditions(&world_db_pool).await?;
        object_mgr
            .set_game_event_schedules(game_event_schedules.clone())
            .await;
        Ok(Self {
            bind_addr,
            login_db_pool,
            character_db_pool: character_db_pool.clone(),
            world_db_pool,
            runtime_state: WorldRuntimeState {
                online_characters: Arc::new(Mutex::new(HashSet::new())),
                delete_options,
                character_db_pool: character_db_pool.clone(),
                world_data_files,
                world_tick_interval,
                game_event_schedules: Arc::new(game_event_schedules),
                sessions: Arc::new(SessionRegistry::default()),
                maps,
                parties: Arc::new(PartyManager::default()),
                object_mgr,
                playerbots,
            },
        })
    }

    pub async fn run(&self) -> anyhow::Result<()> {
        let listener = TcpListener::bind(self.bind_addr).await?;
        info!("World server listening on {}", self.bind_addr);
        let playerbot_planner_state = self.runtime_state.clone();
        tokio::spawn(async move {
            run_playerbot_planner_loop(playerbot_planner_state).await;
        });
        let map_update_state = self.runtime_state.clone();
        tokio::spawn(async move {
            run_map_runtime_update_loop(map_update_state).await;
        });

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

include!("interactions.rs");
include!("wire.rs");
#[cfg(test)]
mod tests;
