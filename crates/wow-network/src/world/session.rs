use super::*;

pub(in crate::world) type OnlineCharacters = Arc<Mutex<HashSet<u32>>>;

pub(in crate::world) type ActiveCharacter = Player;
pub(in crate::world) type DbCreatureRuntime = Creature;
pub(in crate::world) type DbCreatureLifeState = CreatureLifeState;
pub(in crate::world) type DbCreatureLootRuntime = CreatureLoot;
pub(in crate::world) type DbGameObjectRuntime = GameObjectRuntime;
pub(in crate::world) type PlayerCorpseRuntime = Corpse;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(in crate::world) struct SessionId(pub(in crate::world) u64);

impl SessionId {
    pub(in crate::world) fn next() -> Self {
        static NEXT_SESSION_ID: AtomicU64 = AtomicU64::new(1);
        Self(NEXT_SESSION_ID.fetch_add(1, Ordering::Relaxed))
    }
}

#[derive(Debug, Clone)]
pub(in crate::world) struct OutboundWorldPacket {
    pub(in crate::world) opcode: u16,
    pub(in crate::world) body: Vec<u8>,
}

pub(in crate::world) const WORLD_OUTBOUND_QUEUE_CAPACITY: usize = 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::world) enum WorldSessionDisconnectReason {
    OutboundQueueFull,
    LoginTimeout,
    IdleTimeout,
    WriteTimeout,
    WriteError,
}

impl WorldSessionDisconnectReason {
    pub(in crate::world) fn metric_label(self) -> &'static str {
        match self {
            Self::OutboundQueueFull => "outbound_queue_full",
            Self::LoginTimeout => "login_timeout",
            Self::IdleTimeout => "idle_timeout",
            Self::WriteTimeout => "write_timeout",
            Self::WriteError => "write_error",
        }
    }
}

#[derive(Debug)]
pub(in crate::world) enum WorldPacketSendError {
    Closed,
    Full,
}

#[derive(Debug, Clone)]
pub(in crate::world) enum WorldPacketSender {
    Bounded(mpsc::Sender<OutboundWorldPacket>),
    #[cfg(test)]
    Unbounded(mpsc::UnboundedSender<OutboundWorldPacket>),
}

impl From<mpsc::Sender<OutboundWorldPacket>> for WorldPacketSender {
    fn from(sender: mpsc::Sender<OutboundWorldPacket>) -> Self {
        Self::Bounded(sender)
    }
}

#[cfg(test)]
impl From<mpsc::UnboundedSender<OutboundWorldPacket>> for WorldPacketSender {
    fn from(sender: mpsc::UnboundedSender<OutboundWorldPacket>) -> Self {
        Self::Unbounded(sender)
    }
}

impl WorldPacketSender {
    pub(in crate::world) fn send(
        &self,
        packet: OutboundWorldPacket,
    ) -> Result<(), WorldPacketSendError> {
        match self {
            Self::Bounded(sender) => match sender.try_send(packet) {
                Ok(()) => {
                    crate::observability::record_world_outbound_queue_depth(
                        sender.max_capacity().saturating_sub(sender.capacity()),
                    );
                    Ok(())
                }
                Err(mpsc::error::TrySendError::Full(_)) => {
                    crate::observability::record_world_outbound_queue_full();
                    crate::observability::record_world_outbound_queue_depth(sender.max_capacity());
                    Err(WorldPacketSendError::Full)
                }
                Err(mpsc::error::TrySendError::Closed(_)) => Err(WorldPacketSendError::Closed),
            },
            #[cfg(test)]
            Self::Unbounded(sender) => sender
                .send(packet)
                .map_err(|_| WorldPacketSendError::Closed),
        }
    }

    pub(in crate::world) fn is_closed(&self) -> bool {
        match self {
            Self::Bounded(sender) => sender.is_closed(),
            #[cfg(test)]
            Self::Unbounded(sender) => sender.is_closed(),
        }
    }
}

#[derive(Clone)]
pub(in crate::world) struct WorldPacketSink {
    pub(in crate::world) outbound: WorldPacketSender,
}

impl WorldPacketSink {
    pub(in crate::world) fn new(outbound: impl Into<WorldPacketSender>) -> Self {
        Self {
            outbound: outbound.into(),
        }
    }

    pub(in crate::world) fn send(&mut self, opcode: u16, body: &[u8]) -> anyhow::Result<()> {
        self.outbound
            .send(OutboundWorldPacket {
                opcode,
                body: body.to_vec(),
            })
            .map_err(|error| match error {
                WorldPacketSendError::Closed => {
                    anyhow::anyhow!("world session outbound channel closed")
                }
                WorldPacketSendError::Full => {
                    crate::observability::record_world_session_disconnect(
                        WorldSessionDisconnectReason::OutboundQueueFull.metric_label(),
                    );
                    anyhow::anyhow!("world session outbound queue full")
                }
            })
    }
}

#[derive(Debug, Clone)]
pub(in crate::world) struct SessionHandle {
    pub(in crate::world) account_id: u32,
    pub(in crate::world) character_guid: Option<u32>,
    pub(in crate::world) character_name: Option<String>,
    pub(in crate::world) outbound: WorldPacketSender,
    pub(in crate::world) disconnect: Option<mpsc::Sender<WorldSessionDisconnectReason>>,
}

#[derive(Debug, Default)]
pub(in crate::world) struct SessionRegistry {
    pub(in crate::world) sessions: Mutex<HashMap<SessionId, SessionHandle>>,
}

impl SessionRegistry {
    pub(in crate::world) async fn register(
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

    pub(in crate::world) async fn unregister(
        &self,
        session_id: SessionId,
    ) -> Option<SessionHandle> {
        let removed = self.sessions.lock().await.remove(&session_id);
        if removed.is_some() {
            crate::observability::record_world_session_unregistered();
        }
        removed
    }

    pub(in crate::world) async fn set_active_character(
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

    pub(in crate::world) async fn session_for_character(
        &self,
        character_guid: u32,
    ) -> Option<SessionId> {
        self.sessions
            .lock()
            .await
            .iter()
            .find_map(|(session_id, handle)| {
                (handle.character_guid == Some(character_guid)).then_some(*session_id)
            })
    }

    pub(in crate::world) async fn online_character_by_name(
        &self,
        name: &str,
    ) -> Option<(u32, String, SessionId)> {
        let needle = name.to_ascii_lowercase();
        self.sessions
            .lock()
            .await
            .iter()
            .find_map(|(session_id, handle)| {
                let character_guid = handle.character_guid?;
                let character_name = handle.character_name.as_ref()?;
                (character_name.to_ascii_lowercase() == needle)
                    .then(|| (character_guid, character_name.clone(), *session_id))
            })
    }

    pub(in crate::world) async fn character_name_for_guid(
        &self,
        character_guid: u32,
    ) -> Option<String> {
        self.sessions.lock().await.iter().find_map(|(_, handle)| {
            (handle.character_guid == Some(character_guid))
                .then(|| handle.character_name.clone())
                .flatten()
        })
    }

    pub(in crate::world) async fn send_packet(
        &self,
        session_id: SessionId,
        packet: OutboundWorldPacket,
    ) {
        let (outbound, disconnect) = {
            let sessions = self.sessions.lock().await;
            sessions
                .get(&session_id)
                .map(|handle| (handle.outbound.clone(), handle.disconnect.clone()))
                .unzip()
        };
        if let Some(outbound) = outbound {
            if matches!(outbound.send(packet), Err(WorldPacketSendError::Full)) {
                if let Some(Some(disconnect)) = disconnect {
                    let _ = disconnect.try_send(WorldSessionDisconnectReason::OutboundQueueFull);
                }
            }
        }
    }

    pub(in crate::world) async fn dispatch(&self, packets: Vec<(SessionId, OutboundWorldPacket)>) {
        for (session_id, packet) in packets {
            self.send_packet(session_id, packet).await;
        }
    }
}

#[derive(Clone)]
pub(in crate::world) struct WorldRuntimeState {
    pub(in crate::world) online_characters: OnlineCharacters,
    pub(in crate::world) delete_options: CharacterDeleteOptions,
    pub(in crate::world) character_db_pool: MySqlPool,
    pub(in crate::world) world_data_files: Arc<WorldDataFiles>,
    pub(in crate::world) world_tick_interval: Duration,
    pub(in crate::world) game_event_schedules: Arc<Vec<wow_db::GameEventScheduleQuery>>,
    pub(in crate::world) sessions: Arc<SessionRegistry>,
    pub(in crate::world) maps: Arc<MapRuntimeManager>,
    pub(in crate::world) parties: Arc<PartyManager>,
    pub(in crate::world) object_mgr: Arc<ObjectMgr>,
    pub(in crate::world) playerbots: Arc<PlayerbotRoster>,
}

#[derive(Clone, Copy)]
pub(in crate::world) struct SharedWorldDeps<'a> {
    pub(in crate::world) object_mgr: &'a ObjectMgr,
    pub(in crate::world) maps: &'a Arc<MapRuntimeManager>,
    pub(in crate::world) sessions: &'a Arc<SessionRegistry>,
}

#[derive(Debug, Default)]
pub(in crate::world) struct WorldSessionState {
    pub(in crate::world) account: AccountSessionState,
    pub(in crate::world) character: CharacterSessionState,
    pub(in crate::world) movement: MovementSessionState,
    pub(in crate::world) combat: CombatSessionState,
    pub(in crate::world) auras: AuraSessionState,
    pub(in crate::world) inventory: InventorySessionState,
    pub(in crate::world) quests: QuestSessionState,
    pub(in crate::world) death: DeathSessionState,
    #[allow(dead_code)]
    pub(in crate::world) social: SocialSessionState,
    #[allow(dead_code)]
    pub(in crate::world) visibility: VisibilitySessionState,
}

#[derive(Debug, Default)]
pub(in crate::world) struct AccountSessionState {
    pub(in crate::world) account_security: u8,
    pub(in crate::world) gm_mode: bool,
    pub(in crate::world) account_data: HashMap<u32, AccountDataCache>,
}

#[derive(Debug, Default)]
pub(in crate::world) struct CharacterSessionState {
    pub(in crate::world) active_character: Option<ActiveCharacter>,
    pub(in crate::world) selected_target: Option<ObjectGuid>,
    pub(in crate::world) player_visual: Option<PlayerVisualState>,
    pub(in crate::world) player_flags: u32,
    pub(in crate::world) player_health: u32,
    pub(in crate::world) player_rage: u32,
    pub(in crate::world) player_mana: u32,
    pub(in crate::world) player_energy: u32,
    pub(in crate::world) player_ammo_id: u32,
    pub(in crate::world) player_stand_state: u8,
    pub(in crate::world) active_spells: HashSet<u32>,
    pub(in crate::world) character_skills: Vec<CharacterSkill>,
    pub(in crate::world) character_reputations: Vec<CharacterReputation>,
}

#[derive(Debug, Default)]
pub(in crate::world) struct MovementSessionState {
    pub(in crate::world) movement_client_time_delay: Option<u32>,
    pub(in crate::world) db_creature_navigation: DbCreatureNavigationGuardrail,
}

#[derive(Debug, Default)]
pub(in crate::world) struct CombatSessionState {
    pub(in crate::world) last_player_melee_swing_error: Option<PlayerMeleeSwingError>,
    pub(in crate::world) player_in_combat: bool,
    #[cfg(test)]
    pub(in crate::world) active_combat_target: Option<ObjectGuid>,
    #[cfg(test)]
    pub(in crate::world) active_combat_next_swing_at: Option<Instant>,
    #[cfg(test)]
    pub(in crate::world) active_creature_combats: HashMap<u64, CreatureCombatState>,
}

#[derive(Debug, Default)]
pub(in crate::world) struct AuraSessionState {
    pub(in crate::world) active_auras: Vec<ActiveAura>,
}

#[derive(Debug, Default)]
pub(in crate::world) struct InventorySessionState {
    pub(in crate::world) items: Vec<CharacterInventoryItem>,
    pub(in crate::world) buyback_items: Vec<BuybackItem>,
    pub(in crate::world) next_buyback_slot: u8,
}

#[derive(Debug, Default)]
pub(in crate::world) struct QuestSessionState {
    pub(in crate::world) quest_statuses: HashMap<u32, CharacterQuestStatus>,
    pub(in crate::world) quest_log_slots: [u32; MAX_QUEST_LOG_SIZE],
}

#[derive(Debug, Default)]
pub(in crate::world) struct DeathSessionState {
    pub(in crate::world) player_death_state: PlayerDeathState,
    pub(in crate::world) player_death_presentation_pending: bool,
    pub(in crate::world) player_corpse: Option<PlayerCorpseRuntime>,
}

#[derive(Debug, Default)]
pub(in crate::world) struct SocialSessionState {}

#[derive(Debug, Default)]
pub(in crate::world) struct VisibilitySessionState {
    #[cfg(test)]
    pub(in crate::world) db_creatures: HashMap<u64, DbCreatureRuntime>,
    #[cfg(test)]
    pub(in crate::world) db_gameobjects: HashMap<u64, DbGameObjectRuntime>,
    #[cfg(test)]
    pub(in crate::world) last_creature_visibility_position: Option<WorldPosition>,
    #[cfg(test)]
    pub(in crate::world) last_gameobject_visibility_position: Option<WorldPosition>,
    #[cfg(test)]
    #[allow(dead_code)]
    pub(in crate::world) last_player_corpse_visibility_position: Option<WorldPosition>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::world) struct BuybackItem {
    pub(in crate::world) slot: u8,
    pub(in crate::world) item: u32,
    pub(in crate::world) price: u32,
    pub(in crate::world) timestamp: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::world) struct ActiveAura {
    pub(in crate::world) spell_id: u32,
    pub(in crate::world) caster: ObjectGuid,
    pub(in crate::world) level: u8,
    pub(in crate::world) positive: bool,
    pub(in crate::world) visible: bool,
    pub(in crate::world) duration_millis: Option<u32>,
    pub(in crate::world) expires_at: Option<Instant>,
    pub(in crate::world) periodic_damage: Option<PeriodicDamageAura>,
    pub(in crate::world) periodic_regen: Option<PeriodicRegenAura>,
    pub(in crate::world) stat_modifiers: Vec<AuraStatModifier>,
    pub(in crate::world) proc_triggers: Vec<AuraProcTrigger>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(in crate::world) struct AccountDataCache {
    pub(in crate::world) time: u64,
    pub(in crate::world) data: Vec<u8>,
}

impl ActiveAura {
    pub(in crate::world) fn remaining_duration_millis(&self, now: Instant) -> Option<u32> {
        self.expires_at
            .map(|expires_at| expires_at.saturating_duration_since(now).as_millis() as u32)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::world) struct PeriodicDamageAura {
    pub(in crate::world) aura_name: u32,
    pub(in crate::world) school: u32,
    pub(in crate::world) damage_class: u32,
    pub(in crate::world) attributes_ex2: u32,
    pub(in crate::world) attributes_ex3: u32,
    pub(in crate::world) caster_snapshot: SpellCombatUnitSnapshot,
    pub(in crate::world) amount: u32,
    pub(in crate::world) tick_millis: u32,
    pub(in crate::world) next_tick_at: Instant,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::world) struct PeriodicRegenAura {
    pub(in crate::world) health_amount: u32,
    pub(in crate::world) mana_amount: u32,
    pub(in crate::world) tick_millis: u32,
    pub(in crate::world) next_tick_at: Instant,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::world) struct AuraProcTrigger {
    pub(in crate::world) triggered_spell_id: u32,
    pub(in crate::world) proc_flags: u32,
    pub(in crate::world) proc_chance: u32,
    pub(in crate::world) remaining_charges: Option<u32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::world) enum AuraStatModifier {
    AttackPower {
        amount: i32,
    },
    Resistance {
        school_mask: u32,
        amount: i32,
    },
    Skill {
        skill_id: u16,
        amount: i16,
        permanent: bool,
    },
    MoveSpeedPercent {
        percent: i32,
    },
    MeleeAttackTimePercent {
        percent: i32,
    },
    Stat {
        stat: Option<usize>,
        amount: i32,
    },
    TotalStatPercent {
        stat: usize,
        percent: i32,
    },
    ReputationGainPercent {
        percent: i32,
    },
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(in crate::world) struct UnitMoveSpeeds {
    pub(in crate::world) walk: f32,
    pub(in crate::world) run: f32,
    pub(in crate::world) run_back: f32,
    pub(in crate::world) swim: f32,
    pub(in crate::world) swim_back: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::world) struct QueuedNextMeleeSpell {
    pub(in crate::world) spell_id: u32,
    pub(in crate::world) target: ObjectGuid,
    pub(in crate::world) bonus_damage: u32,
    pub(in crate::world) rage_cost: u32,
    pub(in crate::world) mana_cost: u32,
}

#[cfg(test)]
pub(in crate::world) fn mirror_session_player_auto_attack(
    session: &mut WorldSessionState,
    target: Option<ObjectGuid>,
    next_swing_at: Option<Instant>,
) {
    session.combat.active_combat_target = target;
    session.combat.active_combat_next_swing_at = next_swing_at;
}

#[cfg(not(test))]
pub(in crate::world) fn mirror_session_player_auto_attack(
    _session: &mut WorldSessionState,
    _target: Option<ObjectGuid>,
    _next_swing_at: Option<Instant>,
) {
}

#[cfg(test)]
pub(in crate::world) fn mirror_session_player_next_swing_at(
    session: &mut WorldSessionState,
    next_swing_at: Option<Instant>,
) {
    session.combat.active_combat_next_swing_at = next_swing_at;
}

#[cfg(not(test))]
pub(in crate::world) fn mirror_session_player_next_swing_at(
    _session: &mut WorldSessionState,
    _next_swing_at: Option<Instant>,
) {
}

#[cfg(test)]
pub(in crate::world) fn clear_session_active_creature_combats(session: &mut WorldSessionState) {
    session.combat.active_creature_combats.clear();
}

#[cfg(not(test))]
pub(in crate::world) fn clear_session_active_creature_combats(_session: &mut WorldSessionState) {}

#[cfg(test)]
pub(in crate::world) fn mirror_session_active_creature_combats(
    session: &mut WorldSessionState,
    combats: &[CreatureCombatState],
) {
    session.combat.active_creature_combats = combats
        .iter()
        .map(|combat| (combat.attacker.raw(), *combat))
        .collect();
}

#[cfg(not(test))]
pub(in crate::world) fn mirror_session_active_creature_combats(
    _session: &mut WorldSessionState,
    _combats: &[CreatureCombatState],
) {
}

#[cfg(test)]
pub(in crate::world) fn mirror_session_active_creature_combat(
    session: &mut WorldSessionState,
    combat: CreatureCombatState,
) {
    session
        .combat
        .active_creature_combats
        .insert(combat.attacker.raw(), combat);
}

#[cfg(not(test))]
pub(in crate::world) fn mirror_session_active_creature_combat(
    _session: &mut WorldSessionState,
    _combat: CreatureCombatState,
) {
}

#[cfg(test)]
pub(in crate::world) fn remove_session_active_creature_combat(
    session: &mut WorldSessionState,
    attacker: ObjectGuid,
) {
    session
        .combat
        .active_creature_combats
        .remove(&attacker.raw());
}

#[cfg(not(test))]
pub(in crate::world) fn remove_session_active_creature_combat(
    _session: &mut WorldSessionState,
    _attacker: ObjectGuid,
) {
}

#[cfg(test)]
pub(in crate::world) fn mirror_session_db_creature(
    session: &mut WorldSessionState,
    guid: u64,
    creature: DbCreatureRuntime,
) {
    session.visibility.db_creatures.insert(guid, creature);
}

#[cfg(not(test))]
pub(in crate::world) fn mirror_session_db_creature(
    _session: &mut WorldSessionState,
    _guid: u64,
    _creature: DbCreatureRuntime,
) {
}
