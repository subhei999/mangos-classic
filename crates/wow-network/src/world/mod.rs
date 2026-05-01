use rand::Rng;
use sha1::{Digest, Sha1};
use sqlx::mysql::MySqlPool;
use std::collections::{HashMap, HashSet};
use std::io::Cursor;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
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
    CharacterNameQuery, CharacterQuestStatus, CharacterReputation, CharacterSkill, CharacterSpell,
    CreatureLootQuery, CreatureSpawnQuery, CreatureTemplateQuery, ItemTemplateQuery, NewCharacter,
    NewPlayerCorpse, PlayerCorpseQuery, PlayerWorldStats, QuestTemplateQuery,
};

include!("opcodes.rs");
include!("session.rs");
include!("fixtures/legacy_npcs.rs");
include!("server/world_session.rs");
include!("entities/update_data.rs");
include!("server/runtime_helpers.rs");
include!("server/session_loop.rs");
include!("server/character_screen.rs");
include!("server/player_login.rs");
include!("server/logout.rs");
include!("server/movement.rs");
include!("server/visibility.rs");
include!("server/action_buttons.rs");

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
        data_dir: impl Into<std::path::PathBuf>,
    ) -> Self {
        let world_data_files = Arc::new(WorldDataFiles::inspect(data_dir));
        info!(
            data_dir = %world_data_files.data_dir.display(),
            maps = world_data_files.maps_available,
            vmaps = world_data_files.vmaps_available,
            mmap_maps = world_data_files.mmap_headers.len(),
            mmap_tiles = world_data_files.mmap_tiles.len(),
            vmap_maps = world_data_files.vmap_trees.len(),
            vmap_tiles = world_data_files.vmap_tiles.len(),
            "World data files inspected",
        );
        if world_data_files.mmap_tiles.is_empty() {
            warn!(
                data_dir = %world_data_files.data_dir.display(),
                "No mmap tiles found; DB creature pathing will use the permissive fallback",
            );
        }
        if world_data_files.vmaps_available && world_data_files.vmap_tiles.is_empty() {
            warn!(
                data_dir = %world_data_files.data_dir.display(),
                "No compatible CMaNGOS VMAP_7.0 tiles found; DB creature line-of-sight will use the permissive fallback",
            );
        }
        Self {
            bind_addr,
            login_db_pool,
            character_db_pool,
            world_db_pool,
            runtime_state: WorldRuntimeState {
                online_characters: Arc::new(Mutex::new(HashSet::new())),
                player_corpses: Arc::new(Mutex::new(HashMap::new())),
                delete_options,
                world_data_files,
                sessions: Arc::new(SessionRegistry::default()),
                maps: Arc::new(MapRuntimeManager::default()),
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

include!("interactions.rs");
include!("wire.rs");
#[cfg(test)]
mod tests;
