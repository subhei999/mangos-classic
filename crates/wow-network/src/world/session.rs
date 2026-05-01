include!("entities/player.rs");
include!("entities/creature.rs");
include!("entities/gameobject.rs");
include!("entities/corpse.rs");
include!("entities/item.rs");
include!("motion/motion_master.rs");
include!("maps/world_data.rs");
include!("maps/navigation.rs");
include!("maps/grid.rs");
include!("maps/map.rs");
include!("maps/map_manager.rs");

type OnlineCharacters = Arc<Mutex<HashSet<u32>>>;
type PlayerCorpses = Arc<Mutex<HashMap<u32, PlayerCorpseRuntime>>>;

type ActiveCharacter = Player;
type DbCreatureRuntime = Creature;
type DbCreatureLifeState = CreatureLifeState;
type DbCreatureLootRuntime = CreatureLoot;
type DbGameObjectRuntime = GameObjectRuntime;
type PlayerCorpseRuntime = Corpse;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct SessionId(u64);

impl SessionId {
    fn next() -> Self {
        static NEXT_SESSION_ID: AtomicU64 = AtomicU64::new(1);
        Self(NEXT_SESSION_ID.fetch_add(1, Ordering::Relaxed))
    }
}

#[derive(Debug, Clone)]
struct OutboundWorldPacket {
    opcode: u16,
    body: Vec<u8>,
}

#[derive(Clone)]
struct WorldPacketSink {
    outbound: mpsc::UnboundedSender<OutboundWorldPacket>,
}

impl WorldPacketSink {
    fn new(outbound: mpsc::UnboundedSender<OutboundWorldPacket>) -> Self {
        Self { outbound }
    }

    fn send(&mut self, opcode: u16, body: &[u8]) -> anyhow::Result<()> {
        self.outbound
            .send(OutboundWorldPacket {
                opcode,
                body: body.to_vec(),
            })
            .map_err(|_| anyhow::anyhow!("world session outbound channel closed"))
    }
}

#[derive(Debug, Clone)]
struct SessionHandle {
    account_id: u32,
    character_guid: Option<u32>,
    outbound: mpsc::UnboundedSender<OutboundWorldPacket>,
}

#[derive(Debug, Default)]
struct SessionRegistry {
    sessions: Mutex<HashMap<SessionId, SessionHandle>>,
}

impl SessionRegistry {
    async fn register(
        &self,
        session_id: SessionId,
        handle: SessionHandle,
    ) -> Option<SessionHandle> {
        self.sessions.lock().await.insert(session_id, handle)
    }

    async fn unregister(&self, session_id: SessionId) -> Option<SessionHandle> {
        self.sessions.lock().await.remove(&session_id)
    }

    async fn set_character_guid(&self, session_id: SessionId, character_guid: Option<u32>) {
        if let Some(handle) = self.sessions.lock().await.get_mut(&session_id) {
            handle.character_guid = character_guid;
        }
    }

    async fn send_packet(&self, session_id: SessionId, packet: OutboundWorldPacket) {
        let outbound = {
            self.sessions
                .lock()
                .await
                .get(&session_id)
                .map(|handle| handle.outbound.clone())
        };
        if let Some(outbound) = outbound {
            let _ = outbound.send(packet);
        }
    }

    async fn dispatch(&self, packets: Vec<(SessionId, OutboundWorldPacket)>) {
        for (session_id, packet) in packets {
            self.send_packet(session_id, packet).await;
        }
    }
}

#[derive(Clone)]
struct WorldRuntimeState {
    online_characters: OnlineCharacters,
    player_corpses: PlayerCorpses,
    delete_options: CharacterDeleteOptions,
    world_data_files: Arc<WorldDataFiles>,
    sessions: Arc<SessionRegistry>,
    maps: Arc<MapRuntimeManager>,
}

#[derive(Clone, Copy)]
struct SharedWorldDeps<'a> {
    maps: &'a Arc<MapRuntimeManager>,
    sessions: &'a Arc<SessionRegistry>,
}

#[derive(Debug, Default)]
struct WorldSessionState {
    active_character: Option<ActiveCharacter>,
    selected_target: Option<ObjectGuid>,
    combat_dummy_health: u32,
    active_combat_target: Option<ObjectGuid>,
    active_combat_next_swing_at: Option<Instant>,
    last_player_melee_swing_error: Option<PlayerMeleeSwingError>,
    active_creature_combats: HashMap<u64, CreatureCombatState>,
    player_in_combat: bool,
    player_death_state: PlayerDeathState,
    player_corpse: Option<PlayerCorpseRuntime>,
    visible_player_corpses: HashMap<u64, PlayerCorpseRuntime>,
    player_visual: Option<PlayerVisualState>,
    player_flags: u32,
    combat_dummy_lootable: bool,
    combat_dummy_looting: bool,
    combat_dummy_loot_money_available: bool,
    combat_dummy_loot_item_available: bool,
    db_creatures: HashMap<u64, DbCreatureRuntime>,
    db_gameobjects: HashMap<u64, DbGameObjectRuntime>,
    player_health: u32,
    player_rage: u32,
    player_mana: u32,
    active_spells: HashSet<u32>,
    inventory: Vec<CharacterInventoryItem>,
    quest_statuses: HashMap<u32, CharacterQuestStatus>,
    last_creature_visibility_position: Option<WorldPosition>,
    last_gameobject_visibility_position: Option<WorldPosition>,
    last_player_corpse_visibility_position: Option<WorldPosition>,
    db_creature_navigation: DbCreatureNavigationGuardrail,
}
