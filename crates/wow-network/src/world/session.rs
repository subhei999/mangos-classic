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
    delete_options: CharacterDeleteOptions,
    world_data_files: Arc<WorldDataFiles>,
    sessions: Arc<SessionRegistry>,
    maps: Arc<MapRuntimeManager>,
    object_mgr: Arc<ObjectMgr>,
}

#[derive(Clone, Copy)]
struct SharedWorldDeps<'a> {
    object_mgr: &'a ObjectMgr,
    maps: &'a Arc<MapRuntimeManager>,
    sessions: &'a Arc<SessionRegistry>,
}

#[derive(Debug, Default)]
struct WorldSessionState {
    active_character: Option<ActiveCharacter>,
    selected_target: Option<ObjectGuid>,
    combat_dummy_health: u32,
    #[cfg(test)]
    active_combat_target: Option<ObjectGuid>,
    #[cfg(test)]
    active_combat_next_swing_at: Option<Instant>,
    last_player_melee_swing_error: Option<PlayerMeleeSwingError>,
    #[cfg(test)]
    active_creature_combats: HashMap<u64, CreatureCombatState>,
    player_in_combat: bool,
    player_death_state: PlayerDeathState,
    player_corpse: Option<PlayerCorpseRuntime>,
    player_visual: Option<PlayerVisualState>,
    player_flags: u32,
    combat_dummy_lootable: bool,
    combat_dummy_looting: bool,
    combat_dummy_loot_money_available: bool,
    combat_dummy_loot_item_available: bool,
    #[cfg(test)]
    db_creatures: HashMap<u64, DbCreatureRuntime>,
    #[cfg(test)]
    db_gameobjects: HashMap<u64, DbGameObjectRuntime>,
    player_health: u32,
    player_rage: u32,
    player_mana: u32,
    starter_global_cooldown_until: Option<Instant>,
    starter_spell_cooldowns_until: HashMap<u32, Instant>,
    queued_next_melee_spell: Option<QueuedNextMeleeSpell>,
    active_spells: HashSet<u32>,
    inventory: Vec<CharacterInventoryItem>,
    character_skills: Vec<CharacterSkill>,
    quest_statuses: HashMap<u32, CharacterQuestStatus>,
    #[cfg(test)]
    last_creature_visibility_position: Option<WorldPosition>,
    #[cfg(test)]
    last_gameobject_visibility_position: Option<WorldPosition>,
    #[cfg(test)]
    #[allow(dead_code)]
    last_player_corpse_visibility_position: Option<WorldPosition>,
    db_creature_navigation: DbCreatureNavigationGuardrail,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct QueuedNextMeleeSpell {
    spell_id: u32,
    target: ObjectGuid,
    bonus_damage: u32,
    rage_cost: u32,
    mana_cost: u32,
}

#[cfg(test)]
fn mirror_session_player_auto_attack(
    session: &mut WorldSessionState,
    target: Option<ObjectGuid>,
    next_swing_at: Option<Instant>,
) {
    session.active_combat_target = target;
    session.active_combat_next_swing_at = next_swing_at;
}

#[cfg(not(test))]
fn mirror_session_player_auto_attack(
    _session: &mut WorldSessionState,
    _target: Option<ObjectGuid>,
    _next_swing_at: Option<Instant>,
) {
}

#[cfg(test)]
fn mirror_session_player_next_swing_at(
    session: &mut WorldSessionState,
    next_swing_at: Option<Instant>,
) {
    session.active_combat_next_swing_at = next_swing_at;
}

#[cfg(not(test))]
fn mirror_session_player_next_swing_at(
    _session: &mut WorldSessionState,
    _next_swing_at: Option<Instant>,
) {
}

#[cfg(test)]
fn clear_session_active_creature_combats(session: &mut WorldSessionState) {
    session.active_creature_combats.clear();
}

#[cfg(not(test))]
fn clear_session_active_creature_combats(_session: &mut WorldSessionState) {}

#[cfg(test)]
fn mirror_session_active_creature_combats(
    session: &mut WorldSessionState,
    combats: &[CreatureCombatState],
) {
    session.active_creature_combats = combats
        .iter()
        .map(|combat| (combat.attacker.raw(), *combat))
        .collect();
}

#[cfg(not(test))]
fn mirror_session_active_creature_combats(
    _session: &mut WorldSessionState,
    _combats: &[CreatureCombatState],
) {
}

#[cfg(test)]
fn mirror_session_active_creature_combat(
    session: &mut WorldSessionState,
    combat: CreatureCombatState,
) {
    session
        .active_creature_combats
        .insert(combat.attacker.raw(), combat);
}

#[cfg(not(test))]
fn mirror_session_active_creature_combat(
    _session: &mut WorldSessionState,
    _combat: CreatureCombatState,
) {
}

#[cfg(test)]
fn remove_session_active_creature_combat(session: &mut WorldSessionState, attacker: ObjectGuid) {
    session.active_creature_combats.remove(&attacker.raw());
}

#[cfg(not(test))]
fn remove_session_active_creature_combat(
    _session: &mut WorldSessionState,
    _attacker: ObjectGuid,
) {
}

#[cfg(test)]
fn mirror_session_db_creature(
    session: &mut WorldSessionState,
    guid: u64,
    creature: DbCreatureRuntime,
) {
    session.db_creatures.insert(guid, creature);
}

#[cfg(not(test))]
fn mirror_session_db_creature(
    _session: &mut WorldSessionState,
    _guid: u64,
    _creature: DbCreatureRuntime,
) {
}
