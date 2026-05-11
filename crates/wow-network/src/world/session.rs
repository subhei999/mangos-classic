include!("entities/player.rs");
include!("entities/creature.rs");
include!("entities/gameobject.rs");
include!("entities/corpse.rs");
include!("entities/item.rs");
include!("motion/motion_master.rs");
include!("maps/world_data.rs");
include!("maps/world_geometry.rs");
include!("maps/navigation.rs");
include!("maps/grid.rs");
include!("maps/static_world_cache.rs");
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
    character_name: Option<String>,
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
        let previous = self.sessions.lock().await.insert(session_id, handle);
        if previous.is_none() {
            crate::observability::record_world_session_registered();
        }
        previous
    }

    async fn unregister(&self, session_id: SessionId) -> Option<SessionHandle> {
        let removed = self.sessions.lock().await.remove(&session_id);
        if removed.is_some() {
            crate::observability::record_world_session_unregistered();
        }
        removed
    }

    async fn set_active_character(
        &self,
        session_id: SessionId,
        character_guid: Option<u32>,
        character_name: Option<String>,
    ) {
        if let Some(handle) = self.sessions.lock().await.get_mut(&session_id) {
            handle.character_guid = character_guid;
            handle.character_name = character_name;
        }
    }

    async fn session_for_character(&self, character_guid: u32) -> Option<SessionId> {
        self.sessions
            .lock()
            .await
            .iter()
            .find_map(|(session_id, handle)| {
                (handle.character_guid == Some(character_guid)).then_some(*session_id)
            })
    }

    async fn online_character_by_name(&self, name: &str) -> Option<(u32, String, SessionId)> {
        let needle = name.to_ascii_lowercase();
        self.sessions.lock().await.iter().find_map(|(session_id, handle)| {
            let character_guid = handle.character_guid?;
            let character_name = handle.character_name.as_ref()?;
            (character_name.to_ascii_lowercase() == needle).then(|| {
                (character_guid, character_name.clone(), *session_id)
            })
        })
    }

    async fn character_name_for_guid(&self, character_guid: u32) -> Option<String> {
        self.sessions.lock().await.iter().find_map(|(_, handle)| {
            (handle.character_guid == Some(character_guid))
                .then(|| handle.character_name.clone())
                .flatten()
        })
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

include!("social/party.rs");

#[derive(Clone)]
struct WorldRuntimeState {
    online_characters: OnlineCharacters,
    delete_options: CharacterDeleteOptions,
    character_db_pool: MySqlPool,
    world_data_files: Arc<WorldDataFiles>,
    world_tick_interval: Duration,
    game_event_schedules: Arc<Vec<wow_db::GameEventScheduleQuery>>,
    sessions: Arc<SessionRegistry>,
    maps: Arc<MapRuntimeManager>,
    parties: Arc<PartyManager>,
    object_mgr: Arc<ObjectMgr>,
    playerbots: Arc<PlayerbotRoster>,
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
    account_security: u8,
    gm_mode: bool,
    selected_target: Option<ObjectGuid>,
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
    #[cfg(test)]
    db_creatures: HashMap<u64, DbCreatureRuntime>,
    #[cfg(test)]
    db_gameobjects: HashMap<u64, DbGameObjectRuntime>,
    player_health: u32,
    player_rage: u32,
    player_mana: u32,
    player_energy: u32,
    player_stand_state: u8,
    movement_client_time_delay: Option<u32>,
    active_auras: Vec<ActiveAura>,
    active_spells: HashSet<u32>,
    inventory: Vec<CharacterInventoryItem>,
    buyback_items: Vec<BuybackItem>,
    next_buyback_slot: u8,
    character_skills: Vec<CharacterSkill>,
    character_reputations: Vec<CharacterReputation>,
    quest_statuses: HashMap<u32, CharacterQuestStatus>,
    quest_log_slots: [u32; MAX_QUEST_LOG_SIZE],
    account_data: HashMap<u32, AccountDataCache>,
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
struct BuybackItem {
    slot: u8,
    item: u32,
    price: u32,
    timestamp: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ActiveAura {
    spell_id: u32,
    caster: ObjectGuid,
    level: u8,
    positive: bool,
    visible: bool,
    duration_millis: Option<u32>,
    expires_at: Option<Instant>,
    periodic_damage: Option<PeriodicDamageAura>,
    periodic_regen: Option<PeriodicRegenAura>,
    stat_modifiers: Vec<AuraStatModifier>,
    proc_triggers: Vec<AuraProcTrigger>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct AccountDataCache {
    time: u64,
    data: Vec<u8>,
}

impl ActiveAura {
    fn remaining_duration_millis(&self, now: Instant) -> Option<u32> {
        self.expires_at
            .map(|expires_at| expires_at.saturating_duration_since(now).as_millis() as u32)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct PeriodicDamageAura {
    aura_name: u32,
    school: u32,
    damage_class: u32,
    attributes_ex2: u32,
    attributes_ex3: u32,
    amount: u32,
    tick_millis: u32,
    next_tick_at: Instant,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct PeriodicRegenAura {
    health_amount: u32,
    mana_amount: u32,
    tick_millis: u32,
    next_tick_at: Instant,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct AuraProcTrigger {
    triggered_spell_id: u32,
    proc_flags: u32,
    proc_chance: u32,
    remaining_charges: Option<u32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AuraStatModifier {
    AttackPower { amount: i32 },
    Resistance { school_mask: u32, amount: i32 },
    Skill {
        skill_id: u16,
        amount: i16,
        permanent: bool,
    },
    MoveSpeedPercent { percent: i32 },
    MeleeAttackTimePercent { percent: i32 },
    Stat { stat: Option<usize>, amount: i32 },
    TotalStatPercent { stat: usize, percent: i32 },
    ReputationGainPercent { percent: i32 },
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct UnitMoveSpeeds {
    walk: f32,
    run: f32,
    run_back: f32,
    swim: f32,
    swim_back: f32,
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
