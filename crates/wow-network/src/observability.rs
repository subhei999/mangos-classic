use std::collections::{HashMap, VecDeque};
use std::net::SocketAddr;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tracing::{info, warn};

const MAP_TICK_BUCKETS_SECONDS: [f64; 10] = [
    0.001, 0.005, 0.010, 0.025, 0.050, 0.100, 0.250, 0.500, 1.000, 2.500,
];
const ROLLING_ONE_MINUTE: Duration = Duration::from_secs(60);
const ROLLING_FIVE_MINUTES: Duration = Duration::from_secs(300);

static REGISTRY: OnceLock<MetricsRegistry> = OnceLock::new();

fn registry() -> &'static MetricsRegistry {
    REGISTRY.get_or_init(MetricsRegistry::default)
}

#[derive(Default)]
struct MetricsRegistry {
    world_sessions_connected: AtomicU64,
    world_sessions_registered_total: AtomicU64,
    world_sessions_unregistered_total: AtomicU64,
    map_ticks_total: AtomicU64,
    map_tick_over_budget_total: AtomicU64,
    map_tick_errors_total: AtomicU64,
    map_tick_duration: Histogram,
    map_tick_lag: Histogram,
    map_phase_duration: MapPhaseDurations,
    map_runtime_snapshots: Mutex<HashMap<(u32, u32), MapRuntimeSnapshot>>,
    playerbot_names: Mutex<HashMap<u32, String>>,
    playerbot_debug_snapshots: Mutex<HashMap<u32, PlayerbotDebugSnapshot>>,
    playerbot_events: Mutex<HashMap<&'static str, u64>>,
    static_world_cache_loads: Mutex<HashMap<&'static str, StaticWorldCacheLoadStats>>,
    static_world_cache_lookups: Mutex<HashMap<&'static str, DurationStats>>,
    static_world_cache_instantiations: Mutex<HashMap<&'static str, DurationStats>>,
    monitoring_session_started_unix_seconds: AtomicU64,
    monitoring_session_marks_total: AtomicU64,
    world_packets_in: Mutex<HashMap<u32, u64>>,
    world_packets_out: Mutex<HashMap<u32, u64>>,
    world_unknown_opcodes: Mutex<HashMap<u32, u64>>,
    world_packet_dispatch_delay: Mutex<HashMap<u32, Histogram>>,
    world_packet_handler_duration: Mutex<HashMap<u32, Histogram>>,
    world_packet_service_time: Mutex<HashMap<u32, Histogram>>,
    world_packet_outbound_queue_latency: Mutex<HashMap<u32, Histogram>>,
    world_packet_write_duration: Mutex<HashMap<u32, Histogram>>,
    world_session_loop_phase_duration: WorldSessionLoopPhaseDurations,
    world_session_disconnects: Mutex<HashMap<&'static str, u64>>,
    world_outbound_queue_full_total: AtomicU64,
    world_outbound_queue_depth_latest: AtomicU64,
    world_outbound_queue_depth_max: AtomicU64,
    movement_actor_queue_depth_latest: AtomicU64,
    movement_actor_queue_depth_max: AtomicU64,
    movement_actor_enqueue_failures: Mutex<HashMap<&'static str, u64>>,
    movement_actor_enqueue_latency: Histogram,
    movement_actor_processing_time: Histogram,
    movement_actor_reply_latency: Histogram,
    movement_actor_batch_size: NumericStats,
    movement_map_mutex_wait: Histogram,
    movement_map_mutex_hold: Histogram,
    native_mmap: NativeMmapMetrics,
    player_environment_geometry_checks_total: AtomicU64,
    player_environment_cached_skips_total: AtomicU64,
    player_environment_login_refreshes_total: AtomicU64,
    player_environment_movement_refreshes_total: AtomicU64,
    player_environment_teleport_refreshes_total: AtomicU64,
    player_environment_periodic_revalidations_total: AtomicU64,
    player_environment_timer_only_processing_total: AtomicU64,
    idle_motion_subphases: IdleMotionSubphaseMetrics,
    player_environment_subphases: PlayerEnvironmentSubphaseMetrics,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct MapRuntimeSnapshot {
    pub map_id: u32,
    pub instance_id: u32,
    pub active_players: u64,
    pub active_playerbots: u64,
    pub tracked_idle_motion_creatures: u64,
    pub tracked_idle_motion_start_candidates: u64,
    pub active_creatures: u64,
    pub active_gameobjects: u64,
    pub loaded_grids: u64,
    pub loaded_creature_grids: u64,
    pub loaded_gameobject_grids: u64,
    pub loaded_player_corpse_grids: u64,
    pub active_creature_combats: u64,
    pub corpses: u64,
}

#[derive(Debug, Clone)]
pub struct PlayerbotDebugSnapshot {
    pub guid: u32,
    pub map_id: u32,
    pub instance_id: u32,
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub travel_x: Option<f32>,
    pub travel_y: Option<f32>,
    pub travel_z: Option<f32>,
    pub distance_to_travel: Option<f32>,
    pub active_leg_destination_x: Option<f32>,
    pub active_leg_destination_y: Option<f32>,
    pub active_leg_destination_z: Option<f32>,
    pub active_leg_remaining_millis: Option<u64>,
    pub route_len: usize,
    pub next_think_in_millis: u64,
    pub movement_flags: u32,
    pub state: &'static str,
}

#[derive(Debug, Clone, Copy)]
pub enum StaticWorldCacheKind {
    Creature,
    GameObject,
}

impl StaticWorldCacheKind {
    fn label(self) -> &'static str {
        match self {
            Self::Creature => "creature",
            Self::GameObject => "gameobject",
        }
    }
}

#[derive(Debug, Clone, Copy, Default)]
struct StaticWorldCacheLoadStats {
    spawns: u64,
    grids: u64,
    duration_micros: u64,
}

impl StaticWorldCacheLoadStats {
    fn duration_milliseconds(self) -> f64 {
        self.duration_micros as f64 / 1_000.0
    }
}

#[derive(Debug, Clone, Copy, Default)]
struct DurationStats {
    count: u64,
    rows: u64,
    sum_micros: u64,
    latest_micros: u64,
    max_micros: u64,
}

impl DurationStats {
    fn record(&mut self, rows: u64, duration: Duration) {
        let micros = duration.as_micros() as u64;
        self.count += 1;
        self.rows += rows;
        self.sum_micros += micros;
        self.latest_micros = micros;
        self.max_micros = self.max_micros.max(micros);
    }

    fn average_milliseconds(self) -> f64 {
        if self.count == 0 {
            return 0.0;
        }
        self.sum_micros as f64 / self.count as f64 / 1_000.0
    }

    fn latest_milliseconds(self) -> f64 {
        self.latest_micros as f64 / 1_000.0
    }

    fn max_milliseconds(self) -> f64 {
        self.max_micros as f64 / 1_000.0
    }
}

#[derive(Default)]
struct NumericStats {
    count: AtomicU64,
    sum: AtomicU64,
    latest: AtomicU64,
    max: AtomicU64,
}

impl NumericStats {
    fn record(&self, value: u64) {
        self.count.fetch_add(1, Ordering::Relaxed);
        self.sum.fetch_add(value, Ordering::Relaxed);
        self.latest.store(value, Ordering::Relaxed);
        let _ = self
            .max
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |current| {
                (value > current).then_some(value)
            });
    }

    fn count(&self) -> u64 {
        self.count.load(Ordering::Relaxed)
    }

    fn average(&self) -> f64 {
        let count = self.count();
        if count == 0 {
            return 0.0;
        }
        self.sum.load(Ordering::Relaxed) as f64 / count as f64
    }

    fn latest(&self) -> u64 {
        self.latest.load(Ordering::Relaxed)
    }

    fn max(&self) -> u64 {
        self.max.load(Ordering::Relaxed)
    }
}

#[derive(Default)]
struct PlayerEnvironmentSubphaseMetrics {
    players_scanned: NumericStats,
    packets_emitted: NumericStats,
    environment_flags_time: Histogram,
    timer_update_time: Histogram,
    damage_application_time: Histogram,
    nearby_fanout_time: Histogram,
}

#[derive(Default)]
struct IdleMotionSubphaseMetrics {
    due_creatures: NumericStats,
    started_creatures: NumericStats,
    packets_emitted: NumericStats,
    advancement_queue_pop_time: Histogram,
    advancement_validation_time: Histogram,
    motion_advance_time: Histogram,
    spatial_update_time: Histogram,
    start_queue_pop_time: Histogram,
    motion_start_time: Histogram,
    motion_start_path_build_time: Histogram,
    motion_start_snapshot_clone_time: Histogram,
    motion_start_post_start_tracking_time: Histogram,
    motion_start_broadcast_time: Histogram,
    motion_script_schedule_time: Histogram,
    pending_script_execution_time: Histogram,
    start_schedule_rebuild_time: Histogram,
}

#[derive(Default)]
struct NativeMmapMetrics {
    path_calls_total: AtomicU64,
    random_path_calls_total: AtomicU64,
    path: NativeMmapStageMetrics,
    random_path: NativeMmapStageMetrics,
}

#[derive(Default)]
struct NativeMmapStageMetrics {
    lock_and_tile_load_time: Histogram,
    query_alloc_init_time: Histogram,
    find_nearest_poly_time: Histogram,
    find_path_time: Histogram,
    find_smooth_path_time: Histogram,
}

#[derive(Debug, Clone, Copy)]
pub(crate) enum NativeMmapQueryKind {
    Path,
    RandomPath,
}

#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct NativeMmapQueryTimings {
    pub(crate) lock_and_tile_load: Duration,
    pub(crate) query_alloc_init: Duration,
    pub(crate) find_nearest_poly: Duration,
    pub(crate) find_path: Duration,
    pub(crate) find_smooth_path: Duration,
}

#[derive(Debug, Clone, Copy)]
pub enum MapTickPhase {
    StaticGameEventRefresh,
    StaticGameEventRefreshDispatch,
    DbCreatureLifecycle,
    DbCreatureLifecycleDispatch,
    DbCreatureOocEventAi,
    DbCreatureOocEventAiDispatch,
    IdleMotion,
    IdleMotionDispatch,
    PlayerEnvironment,
    PlayerEnvironmentDispatch,
    PlayerbotPlanner,
    PlayerbotMovement,
    PlayerbotMovementDispatch,
    PlayerbotCombat,
    PlayerbotCombatDispatch,
    PlayerRegen,
    PlayerRegenDispatch,
    AuraExpiration,
    AuraExpirationDispatch,
    DynamicObjects,
    DynamicObjectsDispatch,
    PlayerChannels,
    PlayerChannelsDispatch,
    DbCreatureAuras,
    DbCreatureAurasDispatch,
    PlayerDeathPresentation,
    PlayerDeathPresentationDispatch,
    ObservabilitySnapshot,
}

impl MapTickPhase {
    fn label(self) -> &'static str {
        match self {
            Self::StaticGameEventRefresh => "static_game_event_refresh",
            Self::StaticGameEventRefreshDispatch => "static_game_event_refresh_dispatch",
            Self::DbCreatureLifecycle => "db_creature_lifecycle",
            Self::DbCreatureLifecycleDispatch => "db_creature_lifecycle_dispatch",
            Self::DbCreatureOocEventAi => "db_creature_ooc_event_ai",
            Self::DbCreatureOocEventAiDispatch => "db_creature_ooc_event_ai_dispatch",
            Self::IdleMotion => "idle_motion",
            Self::IdleMotionDispatch => "idle_motion_dispatch",
            Self::PlayerEnvironment => "player_environment",
            Self::PlayerEnvironmentDispatch => "player_environment_dispatch",
            Self::PlayerbotPlanner => "playerbot_planner",
            Self::PlayerbotMovement => "playerbot_movement",
            Self::PlayerbotMovementDispatch => "playerbot_movement_dispatch",
            Self::PlayerbotCombat => "playerbot_combat",
            Self::PlayerbotCombatDispatch => "playerbot_combat_dispatch",
            Self::PlayerRegen => "player_regen",
            Self::PlayerRegenDispatch => "player_regen_dispatch",
            Self::AuraExpiration => "aura_expiration",
            Self::AuraExpirationDispatch => "aura_expiration_dispatch",
            Self::DynamicObjects => "dynamic_objects",
            Self::DynamicObjectsDispatch => "dynamic_objects_dispatch",
            Self::PlayerChannels => "player_channels",
            Self::PlayerChannelsDispatch => "player_channels_dispatch",
            Self::DbCreatureAuras => "db_creature_auras",
            Self::DbCreatureAurasDispatch => "db_creature_auras_dispatch",
            Self::PlayerDeathPresentation => "player_death_presentation",
            Self::PlayerDeathPresentationDispatch => "player_death_presentation_dispatch",
            Self::ObservabilitySnapshot => "observability_snapshot",
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum WorldSessionLoopPhase {
    RefreshActivePlayerCache,
    FinalizePlayerDeath,
    PendingSpellCompletion,
    PacketDispatch,
    SyncGameplayState,
    CombatTick,
    LootRollTimeouts,
    PacketBranchTotal,
    TimeoutBranchTotal,
}

impl WorldSessionLoopPhase {
    fn label(self) -> &'static str {
        match self {
            Self::RefreshActivePlayerCache => "refresh_active_player_cache",
            Self::FinalizePlayerDeath => "finalize_player_death",
            Self::PendingSpellCompletion => "pending_spell_completion",
            Self::PacketDispatch => "packet_dispatch",
            Self::SyncGameplayState => "sync_gameplay_state",
            Self::CombatTick => "combat_tick",
            Self::LootRollTimeouts => "loot_roll_timeouts",
            Self::PacketBranchTotal => "packet_branch_total",
            Self::TimeoutBranchTotal => "timeout_branch_total",
        }
    }
}

const WORLD_SESSION_LOOP_PHASES: [WorldSessionLoopPhase; 9] = [
    WorldSessionLoopPhase::RefreshActivePlayerCache,
    WorldSessionLoopPhase::FinalizePlayerDeath,
    WorldSessionLoopPhase::PendingSpellCompletion,
    WorldSessionLoopPhase::PacketDispatch,
    WorldSessionLoopPhase::SyncGameplayState,
    WorldSessionLoopPhase::CombatTick,
    WorldSessionLoopPhase::LootRollTimeouts,
    WorldSessionLoopPhase::PacketBranchTotal,
    WorldSessionLoopPhase::TimeoutBranchTotal,
];

const MAP_TICK_PHASES: [MapTickPhase; 28] = [
    MapTickPhase::StaticGameEventRefresh,
    MapTickPhase::StaticGameEventRefreshDispatch,
    MapTickPhase::DbCreatureLifecycle,
    MapTickPhase::DbCreatureLifecycleDispatch,
    MapTickPhase::DbCreatureOocEventAi,
    MapTickPhase::DbCreatureOocEventAiDispatch,
    MapTickPhase::IdleMotion,
    MapTickPhase::IdleMotionDispatch,
    MapTickPhase::PlayerEnvironment,
    MapTickPhase::PlayerEnvironmentDispatch,
    MapTickPhase::PlayerbotPlanner,
    MapTickPhase::PlayerbotMovement,
    MapTickPhase::PlayerbotMovementDispatch,
    MapTickPhase::PlayerbotCombat,
    MapTickPhase::PlayerbotCombatDispatch,
    MapTickPhase::PlayerRegen,
    MapTickPhase::PlayerRegenDispatch,
    MapTickPhase::AuraExpiration,
    MapTickPhase::AuraExpirationDispatch,
    MapTickPhase::DynamicObjects,
    MapTickPhase::DynamicObjectsDispatch,
    MapTickPhase::PlayerChannels,
    MapTickPhase::PlayerChannelsDispatch,
    MapTickPhase::DbCreatureAuras,
    MapTickPhase::DbCreatureAurasDispatch,
    MapTickPhase::PlayerDeathPresentation,
    MapTickPhase::PlayerDeathPresentationDispatch,
    MapTickPhase::ObservabilitySnapshot,
];

#[derive(Default)]
struct MapPhaseDurations {
    static_game_event_refresh: Histogram,
    static_game_event_refresh_dispatch: Histogram,
    db_creature_lifecycle: Histogram,
    db_creature_lifecycle_dispatch: Histogram,
    db_creature_ooc_event_ai: Histogram,
    db_creature_ooc_event_ai_dispatch: Histogram,
    idle_motion: Histogram,
    idle_motion_dispatch: Histogram,
    player_environment: Histogram,
    player_environment_dispatch: Histogram,
    playerbot_planner: Histogram,
    playerbot_movement: Histogram,
    playerbot_movement_dispatch: Histogram,
    playerbot_combat: Histogram,
    playerbot_combat_dispatch: Histogram,
    player_regen: Histogram,
    player_regen_dispatch: Histogram,
    aura_expiration: Histogram,
    aura_expiration_dispatch: Histogram,
    dynamic_objects: Histogram,
    dynamic_objects_dispatch: Histogram,
    player_channels: Histogram,
    player_channels_dispatch: Histogram,
    db_creature_auras: Histogram,
    db_creature_auras_dispatch: Histogram,
    player_death_presentation: Histogram,
    player_death_presentation_dispatch: Histogram,
    observability_snapshot: Histogram,
}

#[derive(Default)]
struct WorldSessionLoopPhaseDurations {
    refresh_active_player_cache: Histogram,
    finalize_player_death: Histogram,
    pending_spell_completion: Histogram,
    packet_dispatch: Histogram,
    sync_gameplay_state: Histogram,
    combat_tick: Histogram,
    loot_roll_timeouts: Histogram,
    packet_branch_total: Histogram,
    timeout_branch_total: Histogram,
}

impl WorldSessionLoopPhaseDurations {
    fn get(&self, phase: WorldSessionLoopPhase) -> &Histogram {
        match phase {
            WorldSessionLoopPhase::RefreshActivePlayerCache => &self.refresh_active_player_cache,
            WorldSessionLoopPhase::FinalizePlayerDeath => &self.finalize_player_death,
            WorldSessionLoopPhase::PendingSpellCompletion => &self.pending_spell_completion,
            WorldSessionLoopPhase::PacketDispatch => &self.packet_dispatch,
            WorldSessionLoopPhase::SyncGameplayState => &self.sync_gameplay_state,
            WorldSessionLoopPhase::CombatTick => &self.combat_tick,
            WorldSessionLoopPhase::LootRollTimeouts => &self.loot_roll_timeouts,
            WorldSessionLoopPhase::PacketBranchTotal => &self.packet_branch_total,
            WorldSessionLoopPhase::TimeoutBranchTotal => &self.timeout_branch_total,
        }
    }
}

impl MapPhaseDurations {
    fn get(&self, phase: MapTickPhase) -> &Histogram {
        match phase {
            MapTickPhase::StaticGameEventRefresh => &self.static_game_event_refresh,
            MapTickPhase::StaticGameEventRefreshDispatch => {
                &self.static_game_event_refresh_dispatch
            }
            MapTickPhase::DbCreatureLifecycle => &self.db_creature_lifecycle,
            MapTickPhase::DbCreatureLifecycleDispatch => &self.db_creature_lifecycle_dispatch,
            MapTickPhase::DbCreatureOocEventAi => &self.db_creature_ooc_event_ai,
            MapTickPhase::DbCreatureOocEventAiDispatch => &self.db_creature_ooc_event_ai_dispatch,
            MapTickPhase::IdleMotion => &self.idle_motion,
            MapTickPhase::IdleMotionDispatch => &self.idle_motion_dispatch,
            MapTickPhase::PlayerEnvironment => &self.player_environment,
            MapTickPhase::PlayerEnvironmentDispatch => &self.player_environment_dispatch,
            MapTickPhase::PlayerbotPlanner => &self.playerbot_planner,
            MapTickPhase::PlayerbotMovement => &self.playerbot_movement,
            MapTickPhase::PlayerbotMovementDispatch => &self.playerbot_movement_dispatch,
            MapTickPhase::PlayerbotCombat => &self.playerbot_combat,
            MapTickPhase::PlayerbotCombatDispatch => &self.playerbot_combat_dispatch,
            MapTickPhase::PlayerRegen => &self.player_regen,
            MapTickPhase::PlayerRegenDispatch => &self.player_regen_dispatch,
            MapTickPhase::AuraExpiration => &self.aura_expiration,
            MapTickPhase::AuraExpirationDispatch => &self.aura_expiration_dispatch,
            MapTickPhase::DynamicObjects => &self.dynamic_objects,
            MapTickPhase::DynamicObjectsDispatch => &self.dynamic_objects_dispatch,
            MapTickPhase::PlayerChannels => &self.player_channels,
            MapTickPhase::PlayerChannelsDispatch => &self.player_channels_dispatch,
            MapTickPhase::DbCreatureAuras => &self.db_creature_auras,
            MapTickPhase::DbCreatureAurasDispatch => &self.db_creature_auras_dispatch,
            MapTickPhase::PlayerDeathPresentation => &self.player_death_presentation,
            MapTickPhase::PlayerDeathPresentationDispatch => {
                &self.player_death_presentation_dispatch
            }
            MapTickPhase::ObservabilitySnapshot => &self.observability_snapshot,
        }
    }
}

#[derive(Default)]
struct Histogram {
    buckets: [AtomicU64; MAP_TICK_BUCKETS_SECONDS.len()],
    infinite_bucket: AtomicU64,
    count: AtomicU64,
    sum_micros: AtomicU64,
    latest_micros: AtomicU64,
    max_micros: AtomicU64,
    recent: Mutex<VecDeque<TimedSample>>,
}

#[derive(Debug, Clone, Copy)]
struct TimedSample {
    at: Instant,
    micros: u64,
}

#[derive(Debug, Clone, Copy, Default)]
struct RollingStats {
    count: u64,
    sum_micros: u64,
    max_micros: u64,
}

impl RollingStats {
    fn average_milliseconds(self) -> f64 {
        if self.count == 0 {
            return 0.0;
        }
        self.sum_micros as f64 / self.count as f64 / 1_000.0
    }

    fn max_milliseconds(self) -> f64 {
        self.max_micros as f64 / 1_000.0
    }
}

impl Histogram {
    fn record(&self, value: Duration) {
        let micros = value.as_micros() as u64;
        let seconds = value.as_secs_f64();
        let bucket_index = MAP_TICK_BUCKETS_SECONDS
            .iter()
            .position(|bucket| seconds <= *bucket);
        if let Some(index) = bucket_index {
            self.buckets[index].fetch_add(1, Ordering::Relaxed);
        } else {
            self.infinite_bucket.fetch_add(1, Ordering::Relaxed);
        }
        self.count.fetch_add(1, Ordering::Relaxed);
        self.sum_micros.fetch_add(micros, Ordering::Relaxed);
        self.latest_micros.store(micros, Ordering::Relaxed);
        self.max_micros.fetch_max(micros, Ordering::Relaxed);
        let now = Instant::now();
        let mut recent = self
            .recent
            .lock()
            .expect("metrics rolling sample window poisoned");
        recent.push_back(TimedSample { at: now, micros });
        prune_samples(&mut recent, now, ROLLING_FIVE_MINUTES);
    }

    fn average_milliseconds(&self) -> f64 {
        let count = self.count.load(Ordering::Relaxed);
        if count == 0 {
            return 0.0;
        }

        let sum_micros = self.sum_micros.load(Ordering::Relaxed);
        sum_micros as f64 / count as f64 / 1_000.0
    }

    fn latest_milliseconds(&self) -> f64 {
        self.latest_micros.load(Ordering::Relaxed) as f64 / 1_000.0
    }

    fn max_milliseconds(&self) -> f64 {
        self.max_micros.load(Ordering::Relaxed) as f64 / 1_000.0
    }

    fn rolling_stats(&self, window: Duration) -> RollingStats {
        let now = Instant::now();
        let mut recent = self
            .recent
            .lock()
            .expect("metrics rolling sample window poisoned");
        prune_samples(&mut recent, now, ROLLING_FIVE_MINUTES);
        recent
            .iter()
            .filter(|sample| now.saturating_duration_since(sample.at) <= window)
            .fold(RollingStats::default(), |mut stats, sample| {
                stats.count += 1;
                stats.sum_micros += sample.micros;
                stats.max_micros = stats.max_micros.max(sample.micros);
                stats
            })
    }
}

fn prune_samples(samples: &mut VecDeque<TimedSample>, now: Instant, window: Duration) {
    while samples
        .front()
        .is_some_and(|sample| now.saturating_duration_since(sample.at) > window)
    {
        samples.pop_front();
    }
}

pub fn record_world_session_registered() {
    let metrics = registry();
    metrics
        .world_sessions_connected
        .fetch_add(1, Ordering::Relaxed);
    metrics
        .world_sessions_registered_total
        .fetch_add(1, Ordering::Relaxed);
}

pub fn record_world_session_unregistered() {
    let metrics = registry();
    let _ = metrics.world_sessions_connected.fetch_update(
        Ordering::Relaxed,
        Ordering::Relaxed,
        |value| Some(value.saturating_sub(1)),
    );
    metrics
        .world_sessions_unregistered_total
        .fetch_add(1, Ordering::Relaxed);
}

pub fn record_world_packet_in(opcode: u32) {
    increment_opcode(&registry().world_packets_in, opcode);
}

pub fn record_world_packet_dispatch_delay(opcode: u32, duration: Duration) {
    let mut durations = registry()
        .world_packet_dispatch_delay
        .lock()
        .expect("metrics world packet dispatch delay poisoned");
    durations.entry(opcode).or_default().record(duration);
}

pub fn record_world_packet_handler_duration(opcode: u32, duration: Duration) {
    let mut durations = registry()
        .world_packet_handler_duration
        .lock()
        .expect("metrics world packet handler duration poisoned");
    durations.entry(opcode).or_default().record(duration);
}

pub fn record_world_packet_service_time(opcode: u32, duration: Duration) {
    let mut durations = registry()
        .world_packet_service_time
        .lock()
        .expect("metrics world packet service time poisoned");
    durations.entry(opcode).or_default().record(duration);
}

pub fn record_world_packet_out(opcode: u16) {
    increment_opcode(&registry().world_packets_out, opcode as u32);
}

pub fn record_world_packet_outbound_queue_latency(opcode: u16, duration: Duration) {
    let mut durations = registry()
        .world_packet_outbound_queue_latency
        .lock()
        .expect("metrics world packet outbound queue latency poisoned");
    durations.entry(opcode as u32).or_default().record(duration);
}

pub fn record_world_packet_write_duration(opcode: u16, duration: Duration) {
    let mut durations = registry()
        .world_packet_write_duration
        .lock()
        .expect("metrics world packet write duration poisoned");
    durations.entry(opcode as u32).or_default().record(duration);
}

pub fn record_world_session_loop_phase_duration(phase: WorldSessionLoopPhase, duration: Duration) {
    registry()
        .world_session_loop_phase_duration
        .get(phase)
        .record(duration);
}

pub fn record_world_unknown_opcode(opcode: u32) {
    increment_opcode(&registry().world_unknown_opcodes, opcode);
}

pub fn record_world_session_disconnect(reason: &'static str) {
    let mut counters = registry()
        .world_session_disconnects
        .lock()
        .expect("metrics world session disconnect counter poisoned");
    *counters.entry(reason).or_insert(0) += 1;
}

pub fn record_world_outbound_queue_depth(depth: usize) {
    let metrics = registry();
    let depth = depth as u64;
    metrics
        .world_outbound_queue_depth_latest
        .store(depth, Ordering::Relaxed);
    let _ = metrics.world_outbound_queue_depth_max.fetch_update(
        Ordering::Relaxed,
        Ordering::Relaxed,
        |current| (depth > current).then_some(depth),
    );
}

pub fn record_world_outbound_queue_full() {
    registry()
        .world_outbound_queue_full_total
        .fetch_add(1, Ordering::Relaxed);
}

pub fn record_movement_actor_queue_depth(depth: usize) {
    let metrics = registry();
    let depth = depth as u64;
    metrics
        .movement_actor_queue_depth_latest
        .store(depth, Ordering::Relaxed);
    let _ = metrics.movement_actor_queue_depth_max.fetch_update(
        Ordering::Relaxed,
        Ordering::Relaxed,
        |current| (depth > current).then_some(depth),
    );
}

pub fn record_movement_actor_enqueue_failure(reason: &'static str) {
    let mut counters = registry()
        .movement_actor_enqueue_failures
        .lock()
        .expect("metrics movement actor enqueue failures poisoned");
    *counters.entry(reason).or_insert(0) += 1;
}

pub fn record_movement_actor_enqueue_latency(duration: Duration) {
    registry().movement_actor_enqueue_latency.record(duration);
}

pub fn record_movement_actor_processing_time(duration: Duration) {
    registry().movement_actor_processing_time.record(duration);
}

pub fn record_movement_actor_reply_latency(duration: Duration) {
    registry().movement_actor_reply_latency.record(duration);
}

pub fn record_movement_actor_batch_size(batch_size: usize) {
    registry()
        .movement_actor_batch_size
        .record(batch_size as u64);
}

pub fn record_movement_map_mutex_wait(duration: Duration) {
    registry().movement_map_mutex_wait.record(duration);
}

pub fn record_movement_map_mutex_hold(duration: Duration) {
    registry().movement_map_mutex_hold.record(duration);
}

pub(crate) fn record_native_mmap_query(kind: NativeMmapQueryKind, timings: NativeMmapQueryTimings) {
    let metrics = &registry().native_mmap;
    let stage_metrics = match kind {
        NativeMmapQueryKind::Path => {
            metrics.path_calls_total.fetch_add(1, Ordering::Relaxed);
            &metrics.path
        }
        NativeMmapQueryKind::RandomPath => {
            metrics
                .random_path_calls_total
                .fetch_add(1, Ordering::Relaxed);
            &metrics.random_path
        }
    };
    if !timings.lock_and_tile_load.is_zero() {
        stage_metrics
            .lock_and_tile_load_time
            .record(timings.lock_and_tile_load);
    }
    if !timings.query_alloc_init.is_zero() {
        stage_metrics
            .query_alloc_init_time
            .record(timings.query_alloc_init);
    }
    if !timings.find_nearest_poly.is_zero() {
        stage_metrics
            .find_nearest_poly_time
            .record(timings.find_nearest_poly);
    }
    if !timings.find_path.is_zero() {
        stage_metrics.find_path_time.record(timings.find_path);
    }
    if !timings.find_smooth_path.is_zero() {
        stage_metrics
            .find_smooth_path_time
            .record(timings.find_smooth_path);
    }
}

pub fn record_player_environment_geometry_check() {
    registry()
        .player_environment_geometry_checks_total
        .fetch_add(1, Ordering::Relaxed);
}

pub fn record_player_environment_cached_skip() {
    registry()
        .player_environment_cached_skips_total
        .fetch_add(1, Ordering::Relaxed);
}

pub fn record_player_environment_login_refresh() {
    registry()
        .player_environment_login_refreshes_total
        .fetch_add(1, Ordering::Relaxed);
}

pub fn record_player_environment_movement_refresh() {
    registry()
        .player_environment_movement_refreshes_total
        .fetch_add(1, Ordering::Relaxed);
}

pub fn record_player_environment_teleport_refresh() {
    registry()
        .player_environment_teleport_refreshes_total
        .fetch_add(1, Ordering::Relaxed);
}

pub fn record_player_environment_periodic_revalidation() {
    registry()
        .player_environment_periodic_revalidations_total
        .fetch_add(1, Ordering::Relaxed);
}

pub fn record_player_environment_timer_only_processing() {
    registry()
        .player_environment_timer_only_processing_total
        .fetch_add(1, Ordering::Relaxed);
}

pub fn record_idle_motion_due_creatures(creature_count: usize) {
    registry()
        .idle_motion_subphases
        .due_creatures
        .record(creature_count as u64);
}

pub fn record_idle_motion_started_creatures(creature_count: usize) {
    registry()
        .idle_motion_subphases
        .started_creatures
        .record(creature_count as u64);
}

pub fn record_idle_motion_packets_emitted(packet_count: usize) {
    registry()
        .idle_motion_subphases
        .packets_emitted
        .record(packet_count as u64);
}

pub fn record_idle_motion_advancement_queue_pop_time(duration: Duration) {
    registry()
        .idle_motion_subphases
        .advancement_queue_pop_time
        .record(duration);
}

pub fn record_idle_motion_advancement_validation_time(duration: Duration) {
    registry()
        .idle_motion_subphases
        .advancement_validation_time
        .record(duration);
}

pub fn record_idle_motion_motion_advance_time(duration: Duration) {
    registry()
        .idle_motion_subphases
        .motion_advance_time
        .record(duration);
}

pub fn record_idle_motion_spatial_update_time(duration: Duration) {
    registry()
        .idle_motion_subphases
        .spatial_update_time
        .record(duration);
}

pub fn record_idle_motion_start_queue_pop_time(duration: Duration) {
    registry()
        .idle_motion_subphases
        .start_queue_pop_time
        .record(duration);
}

pub fn record_idle_motion_motion_start_time(duration: Duration) {
    registry()
        .idle_motion_subphases
        .motion_start_time
        .record(duration);
}

pub fn record_idle_motion_motion_start_path_build_time(duration: Duration) {
    registry()
        .idle_motion_subphases
        .motion_start_path_build_time
        .record(duration);
}

pub fn record_idle_motion_motion_start_snapshot_clone_time(duration: Duration) {
    registry()
        .idle_motion_subphases
        .motion_start_snapshot_clone_time
        .record(duration);
}

pub fn record_idle_motion_motion_start_post_start_tracking_time(duration: Duration) {
    registry()
        .idle_motion_subphases
        .motion_start_post_start_tracking_time
        .record(duration);
}

pub fn record_idle_motion_motion_start_broadcast_time(duration: Duration) {
    registry()
        .idle_motion_subphases
        .motion_start_broadcast_time
        .record(duration);
}

pub fn record_idle_motion_motion_script_schedule_time(duration: Duration) {
    registry()
        .idle_motion_subphases
        .motion_script_schedule_time
        .record(duration);
}

pub fn record_idle_motion_pending_script_execution_time(duration: Duration) {
    registry()
        .idle_motion_subphases
        .pending_script_execution_time
        .record(duration);
}

pub fn record_idle_motion_start_schedule_rebuild_time(duration: Duration) {
    registry()
        .idle_motion_subphases
        .start_schedule_rebuild_time
        .record(duration);
}

pub fn record_player_environment_players_scanned(players_scanned: usize) {
    registry()
        .player_environment_subphases
        .players_scanned
        .record(players_scanned as u64);
}

pub fn record_player_environment_packets_emitted(packet_count: usize) {
    registry()
        .player_environment_subphases
        .packets_emitted
        .record(packet_count as u64);
}

pub fn record_player_environment_flags_time(duration: Duration) {
    registry()
        .player_environment_subphases
        .environment_flags_time
        .record(duration);
}

pub fn record_player_environment_timer_update_time(duration: Duration) {
    registry()
        .player_environment_subphases
        .timer_update_time
        .record(duration);
}

pub fn record_player_environment_damage_application_time(duration: Duration) {
    registry()
        .player_environment_subphases
        .damage_application_time
        .record(duration);
}

pub fn record_player_environment_nearby_fanout_time(duration: Duration) {
    registry()
        .player_environment_subphases
        .nearby_fanout_time
        .record(duration);
}

pub fn record_map_tick(duration: Duration, lag: Duration, budget: Duration) {
    let metrics = registry();
    metrics.map_ticks_total.fetch_add(1, Ordering::Relaxed);
    metrics.map_tick_duration.record(duration);
    metrics.map_tick_lag.record(lag);
    if duration > budget || lag > budget {
        metrics
            .map_tick_over_budget_total
            .fetch_add(1, Ordering::Relaxed);
    }
}

pub fn record_map_phase_duration(phase: MapTickPhase, duration: Duration) {
    registry().map_phase_duration.get(phase).record(duration);
}

pub fn record_map_runtime_snapshots(snapshots: impl IntoIterator<Item = MapRuntimeSnapshot>) {
    let mut gauges = registry()
        .map_runtime_snapshots
        .lock()
        .expect("metrics map runtime snapshot gauge poisoned");
    gauges.clear();
    for snapshot in snapshots {
        gauges.insert((snapshot.map_id, snapshot.instance_id), snapshot);
    }
}

pub fn record_playerbot_name(guid: u32, name: impl Into<String>) {
    registry()
        .playerbot_names
        .lock()
        .expect("metrics playerbot names poisoned")
        .insert(guid, name.into());
}

pub fn record_playerbot_debug_snapshots(
    snapshots: impl IntoIterator<Item = PlayerbotDebugSnapshot>,
) {
    let mut gauges = registry()
        .playerbot_debug_snapshots
        .lock()
        .expect("metrics playerbot debug snapshots poisoned");
    gauges.clear();
    for snapshot in snapshots {
        gauges.insert(snapshot.guid, snapshot);
    }
}

pub fn record_playerbot_event(kind: &'static str) {
    let mut counters = registry()
        .playerbot_events
        .lock()
        .expect("metrics playerbot event counters poisoned");
    *counters.entry(kind).or_insert(0) += 1;
}

pub fn record_static_world_cache_load(
    kind: StaticWorldCacheKind,
    spawns: u64,
    grids: u64,
    duration: Duration,
) {
    let mut loads = registry()
        .static_world_cache_loads
        .lock()
        .expect("metrics static world cache load registry poisoned");
    loads.insert(
        kind.label(),
        StaticWorldCacheLoadStats {
            spawns,
            grids,
            duration_micros: duration.as_micros() as u64,
        },
    );
}

pub fn record_static_world_cache_lookup(kind: StaticWorldCacheKind, duration: Duration) {
    let mut lookups = registry()
        .static_world_cache_lookups
        .lock()
        .expect("metrics static world cache lookup registry poisoned");
    lookups.entry(kind.label()).or_default().record(0, duration);
}

pub fn record_static_world_cache_instantiation(
    kind: StaticWorldCacheKind,
    rows: u64,
    duration: Duration,
) {
    let mut instantiations = registry()
        .static_world_cache_instantiations
        .lock()
        .expect("metrics static world cache instantiation registry poisoned");
    instantiations
        .entry(kind.label())
        .or_default()
        .record(rows, duration);
}

pub fn record_map_tick_error() {
    registry()
        .map_tick_errors_total
        .fetch_add(1, Ordering::Relaxed);
}

pub fn mark_monitoring_session() -> u64 {
    let started_at = current_unix_seconds();
    let metrics = registry();
    metrics
        .monitoring_session_started_unix_seconds
        .store(started_at, Ordering::Relaxed);
    metrics
        .monitoring_session_marks_total
        .fetch_add(1, Ordering::Relaxed);
    started_at
}

fn monitoring_session_started_unix_seconds(metrics: &MetricsRegistry) -> u64 {
    let started_at = metrics
        .monitoring_session_started_unix_seconds
        .load(Ordering::Relaxed);
    if started_at != 0 {
        return started_at;
    }

    let now = current_unix_seconds();
    match metrics
        .monitoring_session_started_unix_seconds
        .compare_exchange(0, now, Ordering::Relaxed, Ordering::Relaxed)
    {
        Ok(_) => now,
        Err(existing) => existing,
    }
}

fn current_unix_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn increment_opcode(counters: &Mutex<HashMap<u32, u64>>, opcode: u32) {
    let mut counters = counters.lock().expect("metrics opcode counter poisoned");
    *counters.entry(opcode).or_insert(0) += 1;
}

pub fn render_prometheus() -> String {
    let metrics = registry();
    let mut body = String::with_capacity(8192);

    write_counter(
        &mut body,
        "wow_world_sessions_registered_total",
        "Total authenticated world sessions registered.",
        metrics
            .world_sessions_registered_total
            .load(Ordering::Relaxed),
    );
    write_counter(
        &mut body,
        "wow_world_sessions_unregistered_total",
        "Total authenticated world sessions unregistered.",
        metrics
            .world_sessions_unregistered_total
            .load(Ordering::Relaxed),
    );
    write_gauge(
        &mut body,
        "wow_world_sessions_connected",
        "Currently connected authenticated world sessions.",
        metrics.world_sessions_connected.load(Ordering::Relaxed),
    );
    write_gauge(
        &mut body,
        "wow_monitoring_session_started_unix_seconds",
        "Unix timestamp for the current monitoring session marker.",
        monitoring_session_started_unix_seconds(metrics),
    );
    write_counter(
        &mut body,
        "wow_monitoring_session_marks_total",
        "Total monitoring session markers requested from the dashboard.",
        metrics
            .monitoring_session_marks_total
            .load(Ordering::Relaxed),
    );
    write_counter(
        &mut body,
        "wow_map_ticks_total",
        "Total map runtime update ticks.",
        metrics.map_ticks_total.load(Ordering::Relaxed),
    );
    write_counter(
        &mut body,
        "wow_map_tick_over_budget_total",
        "Map ticks whose duration or start lag exceeded the world tick budget.",
        metrics.map_tick_over_budget_total.load(Ordering::Relaxed),
    );
    write_counter(
        &mut body,
        "wow_map_tick_errors_total",
        "Map runtime update phases that returned an error.",
        metrics.map_tick_errors_total.load(Ordering::Relaxed),
    );
    write_histogram(
        &mut body,
        "wow_map_tick_duration_seconds",
        "Time spent executing map runtime update work.",
        &metrics.map_tick_duration,
    );
    write_float_gauge(
        &mut body,
        "wow_map_tick_duration_average_milliseconds",
        "Average time spent executing map runtime update work.",
        metrics.map_tick_duration.average_milliseconds(),
    );
    write_float_gauge(
        &mut body,
        "wow_map_tick_duration_latest_milliseconds",
        "Most recent time spent executing map runtime update work.",
        metrics.map_tick_duration.latest_milliseconds(),
    );
    write_float_gauge(
        &mut body,
        "wow_map_tick_duration_max_milliseconds",
        "Maximum observed time spent executing map runtime update work since server start.",
        metrics.map_tick_duration.max_milliseconds(),
    );
    write_rolling_histogram_gauges(
        &mut body,
        "wow_map_tick_duration",
        "time spent executing map runtime update work",
        &metrics.map_tick_duration,
    );
    write_histogram(
        &mut body,
        "wow_map_tick_lag_seconds",
        "How late the map runtime update loop started relative to its scheduled tick.",
        &metrics.map_tick_lag,
    );
    write_float_gauge(
        &mut body,
        "wow_map_tick_lag_average_milliseconds",
        "Average delay between the scheduled map tick time and the actual start time.",
        metrics.map_tick_lag.average_milliseconds(),
    );
    write_float_gauge(
        &mut body,
        "wow_map_tick_lag_latest_milliseconds",
        "Most recent delay between the scheduled map tick time and the actual start time.",
        metrics.map_tick_lag.latest_milliseconds(),
    );
    write_float_gauge(
        &mut body,
        "wow_map_tick_lag_max_milliseconds",
        "Maximum observed delay between the scheduled map tick time and the actual start time since server start.",
        metrics.map_tick_lag.max_milliseconds(),
    );
    write_rolling_histogram_gauges(
        &mut body,
        "wow_map_tick_lag",
        "delay between the scheduled map tick time and the actual start time",
        &metrics.map_tick_lag,
    );
    write_map_phase_duration_summaries(&mut body, &metrics.map_phase_duration);
    write_map_runtime_gauges(&mut body, &metrics.map_runtime_snapshots);
    write_playerbot_event_counters(&mut body, &metrics.playerbot_events);
    write_static_world_cache_metrics(
        &mut body,
        &metrics.static_world_cache_loads,
        &metrics.static_world_cache_lookups,
        &metrics.static_world_cache_instantiations,
    );
    write_opcode_counter(
        &mut body,
        "wow_world_packets_in_total",
        "Total world packets received by opcode.",
        &metrics.world_packets_in,
    );
    write_opcode_counter(
        &mut body,
        "wow_world_packets_out_total",
        "Total world packets sent by opcode.",
        &metrics.world_packets_out,
    );
    write_opcode_counter(
        &mut body,
        "wow_world_unknown_opcodes_total",
        "Total authenticated world packets with no handler.",
        &metrics.world_unknown_opcodes,
    );
    write_opcode_histogram_metric_summaries(
        &mut body,
        "wow_world_packet_dispatch_delay",
        "authenticated world packet dispatch delay",
        "Time from authenticated world packet receipt until handler dispatch began.",
        &metrics.world_packet_dispatch_delay,
    );
    write_opcode_histogram_metric_summaries(
        &mut body,
        "wow_world_packet_handler_duration",
        "authenticated world packet handler duration",
        "Time spent handling an authenticated world packet by opcode.",
        &metrics.world_packet_handler_duration,
    );
    write_opcode_histogram_metric_summaries(
        &mut body,
        "wow_world_packet_service_time",
        "authenticated world packet service time",
        "Time from authenticated world packet receipt until handler completion.",
        &metrics.world_packet_service_time,
    );
    write_opcode_histogram_metric_summaries(
        &mut body,
        "wow_world_packet_outbound_queue_latency",
        "authenticated world packet outbound queue latency",
        "Time a world packet spent waiting in the session outbound queue before socket write.",
        &metrics.world_packet_outbound_queue_latency,
    );
    write_opcode_histogram_metric_summaries(
        &mut body,
        "wow_world_packet_write_duration",
        "authenticated world packet socket write duration",
        "Time spent writing a world packet to the session socket.",
        &metrics.world_packet_write_duration,
    );
    write_world_session_loop_phase_duration_summaries(
        &mut body,
        &metrics.world_session_loop_phase_duration,
    );
    write_label_counter(
        &mut body,
        "wow_world_session_disconnects_total",
        "Total world session disconnects by reason.",
        "reason",
        &metrics.world_session_disconnects,
    );
    write_counter(
        &mut body,
        "wow_world_outbound_queue_full_total",
        "Total outbound world packets rejected because a session queue was full.",
        metrics
            .world_outbound_queue_full_total
            .load(Ordering::Relaxed),
    );
    write_gauge(
        &mut body,
        "wow_world_outbound_queue_depth_latest",
        "Most recently observed world session outbound queue depth.",
        metrics
            .world_outbound_queue_depth_latest
            .load(Ordering::Relaxed),
    );
    write_gauge(
        &mut body,
        "wow_world_outbound_queue_depth_max",
        "Maximum observed world session outbound queue depth since server start.",
        metrics
            .world_outbound_queue_depth_max
            .load(Ordering::Relaxed),
    );
    write_gauge(
        &mut body,
        "wow_movement_actor_queue_depth_latest",
        "Most recently observed movement actor mailbox depth.",
        metrics
            .movement_actor_queue_depth_latest
            .load(Ordering::Relaxed),
    );
    write_gauge(
        &mut body,
        "wow_movement_actor_queue_depth_max",
        "Maximum observed movement actor mailbox depth since server start.",
        metrics
            .movement_actor_queue_depth_max
            .load(Ordering::Relaxed),
    );
    write_label_counter(
        &mut body,
        "wow_movement_actor_enqueue_failures_total",
        "Total movement actor enqueue failures by reason.",
        "reason",
        &metrics.movement_actor_enqueue_failures,
    );
    write_histogram(
        &mut body,
        "wow_movement_actor_enqueue_latency_seconds",
        "Time spent enqueueing movement work into the movement actor mailbox.",
        &metrics.movement_actor_enqueue_latency,
    );
    write_float_gauge(
        &mut body,
        "wow_movement_actor_enqueue_latency_average_milliseconds",
        "Average time spent enqueueing movement work into the movement actor mailbox.",
        metrics
            .movement_actor_enqueue_latency
            .average_milliseconds(),
    );
    write_float_gauge(
        &mut body,
        "wow_movement_actor_enqueue_latency_latest_milliseconds",
        "Most recent time spent enqueueing movement work into the movement actor mailbox.",
        metrics.movement_actor_enqueue_latency.latest_milliseconds(),
    );
    write_float_gauge(
        &mut body,
        "wow_movement_actor_enqueue_latency_max_milliseconds",
        "Maximum observed movement actor enqueue time since server start.",
        metrics.movement_actor_enqueue_latency.max_milliseconds(),
    );
    write_rolling_histogram_gauges(
        &mut body,
        "wow_movement_actor_enqueue_latency",
        "time spent enqueueing movement work into the movement actor mailbox",
        &metrics.movement_actor_enqueue_latency,
    );
    write_histogram(
        &mut body,
        "wow_movement_actor_processing_time_seconds",
        "Time spent processing one drained movement actor batch.",
        &metrics.movement_actor_processing_time,
    );
    write_float_gauge(
        &mut body,
        "wow_movement_actor_processing_time_average_milliseconds",
        "Average time spent processing one drained movement actor batch.",
        metrics
            .movement_actor_processing_time
            .average_milliseconds(),
    );
    write_float_gauge(
        &mut body,
        "wow_movement_actor_processing_time_latest_milliseconds",
        "Most recent time spent processing one drained movement actor batch.",
        metrics.movement_actor_processing_time.latest_milliseconds(),
    );
    write_float_gauge(
        &mut body,
        "wow_movement_actor_processing_time_max_milliseconds",
        "Maximum observed movement actor batch processing time since server start.",
        metrics.movement_actor_processing_time.max_milliseconds(),
    );
    write_rolling_histogram_gauges(
        &mut body,
        "wow_movement_actor_processing_time",
        "time spent processing one drained movement actor batch",
        &metrics.movement_actor_processing_time,
    );
    write_histogram(
        &mut body,
        "wow_movement_actor_reply_latency_seconds",
        "Time from movement actor enqueue until the caller reply is completed.",
        &metrics.movement_actor_reply_latency,
    );
    write_float_gauge(
        &mut body,
        "wow_movement_actor_reply_latency_average_milliseconds",
        "Average time from movement actor enqueue until the caller reply is completed.",
        metrics.movement_actor_reply_latency.average_milliseconds(),
    );
    write_float_gauge(
        &mut body,
        "wow_movement_actor_reply_latency_latest_milliseconds",
        "Most recent time from movement actor enqueue until the caller reply is completed.",
        metrics.movement_actor_reply_latency.latest_milliseconds(),
    );
    write_float_gauge(
        &mut body,
        "wow_movement_actor_reply_latency_max_milliseconds",
        "Maximum observed movement actor reply latency since server start.",
        metrics.movement_actor_reply_latency.max_milliseconds(),
    );
    write_rolling_histogram_gauges(
        &mut body,
        "wow_movement_actor_reply_latency",
        "time from movement actor enqueue until the caller reply is completed",
        &metrics.movement_actor_reply_latency,
    );
    write_counter(
        &mut body,
        "wow_movement_actor_batches_total",
        "Total drained movement actor batches.",
        metrics.movement_actor_batch_size.count(),
    );
    write_float_gauge(
        &mut body,
        "wow_movement_actor_batch_size_average",
        "Average number of commands drained per movement actor batch.",
        metrics.movement_actor_batch_size.average(),
    );
    write_gauge(
        &mut body,
        "wow_movement_actor_batch_size_latest",
        "Most recent number of commands drained in one movement actor batch.",
        metrics.movement_actor_batch_size.latest(),
    );
    write_gauge(
        &mut body,
        "wow_movement_actor_batch_size_max",
        "Maximum observed number of commands drained in one movement actor batch since server start.",
        metrics.movement_actor_batch_size.max(),
    );
    write_histogram(
        &mut body,
        "wow_movement_map_mutex_wait_seconds",
        "Time movement work spent waiting to acquire the MapRuntime mutex.",
        &metrics.movement_map_mutex_wait,
    );
    write_float_gauge(
        &mut body,
        "wow_movement_map_mutex_wait_average_milliseconds",
        "Average time movement work spent waiting to acquire the MapRuntime mutex.",
        metrics.movement_map_mutex_wait.average_milliseconds(),
    );
    write_float_gauge(
        &mut body,
        "wow_movement_map_mutex_wait_latest_milliseconds",
        "Most recent time movement work spent waiting to acquire the MapRuntime mutex.",
        metrics.movement_map_mutex_wait.latest_milliseconds(),
    );
    write_float_gauge(
        &mut body,
        "wow_movement_map_mutex_wait_max_milliseconds",
        "Maximum observed movement mutex wait time since server start.",
        metrics.movement_map_mutex_wait.max_milliseconds(),
    );
    write_rolling_histogram_gauges(
        &mut body,
        "wow_movement_map_mutex_wait",
        "time movement work spent waiting to acquire the MapRuntime mutex",
        &metrics.movement_map_mutex_wait,
    );
    write_histogram(
        &mut body,
        "wow_movement_map_mutex_hold_seconds",
        "Time movement work held the MapRuntime mutex while applying updates.",
        &metrics.movement_map_mutex_hold,
    );
    write_float_gauge(
        &mut body,
        "wow_movement_map_mutex_hold_average_milliseconds",
        "Average time movement work held the MapRuntime mutex while applying updates.",
        metrics.movement_map_mutex_hold.average_milliseconds(),
    );
    write_float_gauge(
        &mut body,
        "wow_movement_map_mutex_hold_latest_milliseconds",
        "Most recent time movement work held the MapRuntime mutex while applying updates.",
        metrics.movement_map_mutex_hold.latest_milliseconds(),
    );
    write_float_gauge(
        &mut body,
        "wow_movement_map_mutex_hold_max_milliseconds",
        "Maximum observed movement mutex hold time since server start.",
        metrics.movement_map_mutex_hold.max_milliseconds(),
    );
    write_rolling_histogram_gauges(
        &mut body,
        "wow_movement_map_mutex_hold",
        "time movement work held the MapRuntime mutex while applying updates",
        &metrics.movement_map_mutex_hold,
    );
    write_native_mmap_metrics(&mut body, &metrics.native_mmap);
    write_counter(
        &mut body,
        "wow_player_environment_geometry_checks_total",
        "Total player-environment cache refreshes that sampled world geometry.",
        metrics
            .player_environment_geometry_checks_total
            .load(Ordering::Relaxed),
    );
    write_counter(
        &mut body,
        "wow_player_environment_cached_skips_total",
        "Total player-environment cache hits that skipped world geometry sampling.",
        metrics
            .player_environment_cached_skips_total
            .load(Ordering::Relaxed),
    );
    write_counter(
        &mut body,
        "wow_player_environment_login_refreshes_total",
        "Total player-environment geometry refreshes forced by player login or add-player ownership.",
        metrics
            .player_environment_login_refreshes_total
            .load(Ordering::Relaxed),
    );
    write_counter(
        &mut body,
        "wow_player_environment_movement_refreshes_total",
        "Total player-environment geometry refreshes forced by applied player movement ownership.",
        metrics
            .player_environment_movement_refreshes_total
            .load(Ordering::Relaxed),
    );
    write_counter(
        &mut body,
        "wow_player_environment_teleport_refreshes_total",
        "Total player-environment geometry refreshes forced by teleport or authoritative set-position ownership.",
        metrics
            .player_environment_teleport_refreshes_total
            .load(Ordering::Relaxed),
    );
    write_counter(
        &mut body,
        "wow_player_environment_periodic_revalidations_total",
        "Total player-environment geometry refreshes performed by the narrow periodic active-state revalidation path.",
        metrics
            .player_environment_periodic_revalidations_total
            .load(Ordering::Relaxed),
    );
    write_counter(
        &mut body,
        "wow_player_environment_timer_only_processing_total",
        "Total player-environment tick passes that advanced cached timers without re-sampling world geometry.",
        metrics
            .player_environment_timer_only_processing_total
            .load(Ordering::Relaxed),
    );
    write_idle_motion_subphase_metrics(&mut body, &metrics.idle_motion_subphases);
    write_player_environment_subphase_metrics(&mut body, &metrics.player_environment_subphases);
    body.push_str(&wow_db::render_db_metrics_prometheus());

    body
}

fn write_label_counter(
    body: &mut String,
    name: &str,
    help: &str,
    label_name: &str,
    counters: &Mutex<HashMap<&'static str, u64>>,
) {
    let values = counters
        .lock()
        .expect("metrics label counter registry poisoned");
    body.push_str("# HELP ");
    body.push_str(name);
    body.push(' ');
    body.push_str(help);
    body.push('\n');
    body.push_str("# TYPE ");
    body.push_str(name);
    body.push_str(" counter\n");
    let mut rows = values.iter().collect::<Vec<_>>();
    rows.sort_by(|left, right| left.0.cmp(right.0));
    for (label, value) in rows {
        body.push_str(name);
        body.push('{');
        body.push_str(label_name);
        body.push_str("=\"");
        body.push_str(label);
        body.push_str("\"} ");
        body.push_str(&value.to_string());
        body.push('\n');
    }
}

fn write_playerbot_event_counters(body: &mut String, counters: &Mutex<HashMap<&'static str, u64>>) {
    let values = counters
        .lock()
        .expect("metrics playerbot event counter registry poisoned");
    body.push_str("# HELP wow_playerbot_events_total Total playerbot runtime events by kind.\n");
    body.push_str("# TYPE wow_playerbot_events_total counter\n");
    let mut rows = values.iter().collect::<Vec<_>>();
    rows.sort_by(|left, right| left.0.cmp(right.0));
    for (kind, value) in rows {
        body.push_str("wow_playerbot_events_total{kind=\"");
        body.push_str(kind);
        body.push_str("\"} ");
        body.push_str(&value.to_string());
        body.push('\n');
    }
}

fn write_counter(body: &mut String, name: &str, help: &str, value: u64) {
    body.push_str("# HELP ");
    body.push_str(name);
    body.push(' ');
    body.push_str(help);
    body.push('\n');
    body.push_str("# TYPE ");
    body.push_str(name);
    body.push_str(" counter\n");
    body.push_str(name);
    body.push(' ');
    body.push_str(&value.to_string());
    body.push('\n');
}

fn write_gauge(body: &mut String, name: &str, help: &str, value: u64) {
    body.push_str("# HELP ");
    body.push_str(name);
    body.push(' ');
    body.push_str(help);
    body.push('\n');
    body.push_str("# TYPE ");
    body.push_str(name);
    body.push_str(" gauge\n");
    body.push_str(name);
    body.push(' ');
    body.push_str(&value.to_string());
    body.push('\n');
}

fn write_float_gauge(body: &mut String, name: &str, help: &str, value: f64) {
    body.push_str("# HELP ");
    body.push_str(name);
    body.push(' ');
    body.push_str(help);
    body.push('\n');
    body.push_str("# TYPE ");
    body.push_str(name);
    body.push_str(" gauge\n");
    body.push_str(name);
    body.push(' ');
    body.push_str(&format!("{value:.3}"));
    body.push('\n');
}

fn write_numeric_summary(
    body: &mut String,
    prefix: &str,
    help_prefix: &str,
    values: &NumericStats,
) {
    write_counter(
        body,
        &format!("{prefix}_samples_total"),
        &format!("Total recorded samples for {help_prefix}."),
        values.count(),
    );
    write_float_gauge(
        body,
        &format!("{prefix}_average"),
        &format!("Average {help_prefix}."),
        values.average(),
    );
    write_gauge(
        body,
        &format!("{prefix}_latest"),
        &format!("Most recent {help_prefix}."),
        values.latest(),
    );
    write_gauge(
        body,
        &format!("{prefix}_max"),
        &format!("Maximum observed {help_prefix} since server start."),
        values.max(),
    );
}

fn write_histogram(body: &mut String, name: &str, help: &str, histogram: &Histogram) {
    body.push_str("# HELP ");
    body.push_str(name);
    body.push(' ');
    body.push_str(help);
    body.push('\n');
    body.push_str("# TYPE ");
    body.push_str(name);
    body.push_str(" histogram\n");

    let mut cumulative = 0u64;
    for (index, bucket) in MAP_TICK_BUCKETS_SECONDS.iter().enumerate() {
        cumulative += histogram.buckets[index].load(Ordering::Relaxed);
        body.push_str(name);
        body.push_str("_bucket{le=\"");
        body.push_str(&format_bucket(*bucket));
        body.push_str("\"} ");
        body.push_str(&cumulative.to_string());
        body.push('\n');
    }

    cumulative += histogram.infinite_bucket.load(Ordering::Relaxed);
    body.push_str(name);
    body.push_str("_bucket{le=\"+Inf\"} ");
    body.push_str(&cumulative.to_string());
    body.push('\n');
    body.push_str(name);
    body.push_str("_sum ");
    let sum_seconds = histogram.sum_micros.load(Ordering::Relaxed) as f64 / 1_000_000.0;
    body.push_str(&format!("{sum_seconds:.6}"));
    body.push('\n');
    body.push_str(name);
    body.push_str("_count ");
    body.push_str(&histogram.count.load(Ordering::Relaxed).to_string());
    body.push('\n');
}

fn write_rolling_histogram_gauges(
    body: &mut String,
    prefix: &str,
    subject: &str,
    histogram: &Histogram,
) {
    let one_minute = histogram.rolling_stats(ROLLING_ONE_MINUTE);
    let five_minutes = histogram.rolling_stats(ROLLING_FIVE_MINUTES);
    write_float_gauge(
        body,
        &format!("{prefix}_average_1m_milliseconds"),
        &format!("Average {subject} over the last minute."),
        one_minute.average_milliseconds(),
    );
    write_float_gauge(
        body,
        &format!("{prefix}_max_1m_milliseconds"),
        &format!("Maximum observed {subject} over the last minute."),
        one_minute.max_milliseconds(),
    );
    write_float_gauge(
        body,
        &format!("{prefix}_average_5m_milliseconds"),
        &format!("Average {subject} over the last five minutes."),
        five_minutes.average_milliseconds(),
    );
    write_float_gauge(
        body,
        &format!("{prefix}_max_5m_milliseconds"),
        &format!("Maximum observed {subject} over the last five minutes."),
        five_minutes.max_milliseconds(),
    );
}

fn write_map_phase_duration_summaries(body: &mut String, phases: &MapPhaseDurations) {
    write_phase_float_gauges(
        body,
        "wow_map_phase_duration_average_milliseconds",
        "Average map update phase duration.",
        phases,
        Histogram::average_milliseconds,
    );
    write_phase_float_gauges(
        body,
        "wow_map_phase_duration_latest_milliseconds",
        "Most recent map update phase duration.",
        phases,
        Histogram::latest_milliseconds,
    );
    write_phase_float_gauges(
        body,
        "wow_map_phase_duration_max_milliseconds",
        "Maximum observed map update phase duration since server start.",
        phases,
        Histogram::max_milliseconds,
    );
    write_phase_rolling_gauges(
        body,
        "wow_map_phase_duration_average_1m_milliseconds",
        "Average map update phase duration over the last minute.",
        phases,
        ROLLING_ONE_MINUTE,
        RollingStats::average_milliseconds,
    );
    write_phase_rolling_gauges(
        body,
        "wow_map_phase_duration_max_1m_milliseconds",
        "Maximum map update phase duration over the last minute.",
        phases,
        ROLLING_ONE_MINUTE,
        RollingStats::max_milliseconds,
    );
    write_phase_rolling_gauges(
        body,
        "wow_map_phase_duration_average_5m_milliseconds",
        "Average map update phase duration over the last five minutes.",
        phases,
        ROLLING_FIVE_MINUTES,
        RollingStats::average_milliseconds,
    );
    write_phase_rolling_gauges(
        body,
        "wow_map_phase_duration_max_5m_milliseconds",
        "Maximum map update phase duration over the last five minutes.",
        phases,
        ROLLING_FIVE_MINUTES,
        RollingStats::max_milliseconds,
    );
}

fn write_world_session_loop_phase_duration_summaries(
    body: &mut String,
    phases: &WorldSessionLoopPhaseDurations,
) {
    body.push_str(
        "# HELP wow_world_session_loop_phase_duration_average_milliseconds Average world session loop phase duration.\n",
    );
    body.push_str("# TYPE wow_world_session_loop_phase_duration_average_milliseconds gauge\n");
    body.push_str(
        "# HELP wow_world_session_loop_phase_duration_latest_milliseconds Most recent world session loop phase duration.\n",
    );
    body.push_str("# TYPE wow_world_session_loop_phase_duration_latest_milliseconds gauge\n");
    body.push_str(
        "# HELP wow_world_session_loop_phase_duration_max_milliseconds Maximum observed world session loop phase duration since server start.\n",
    );
    body.push_str("# TYPE wow_world_session_loop_phase_duration_max_milliseconds gauge\n");
    body.push_str(
        "# HELP wow_world_session_loop_phase_duration_average_1m_milliseconds Average world session loop phase duration over the last minute.\n",
    );
    body.push_str("# TYPE wow_world_session_loop_phase_duration_average_1m_milliseconds gauge\n");
    body.push_str(
        "# HELP wow_world_session_loop_phase_duration_max_1m_milliseconds Maximum world session loop phase duration over the last minute.\n",
    );
    body.push_str("# TYPE wow_world_session_loop_phase_duration_max_1m_milliseconds gauge\n");
    body.push_str(
        "# HELP wow_world_session_loop_phase_duration_average_5m_milliseconds Average world session loop phase duration over the last five minutes.\n",
    );
    body.push_str("# TYPE wow_world_session_loop_phase_duration_average_5m_milliseconds gauge\n");
    body.push_str(
        "# HELP wow_world_session_loop_phase_duration_max_5m_milliseconds Maximum world session loop phase duration over the last five minutes.\n",
    );
    body.push_str("# TYPE wow_world_session_loop_phase_duration_max_5m_milliseconds gauge\n");

    for phase in WORLD_SESSION_LOOP_PHASES {
        let histogram = phases.get(phase);
        let label = phase.label();
        let rolling_1m = histogram.rolling_stats(ROLLING_ONE_MINUTE);
        let rolling_5m = histogram.rolling_stats(ROLLING_FIVE_MINUTES);
        body.push_str("wow_world_session_loop_phase_duration_average_milliseconds{phase=\"");
        body.push_str(label);
        body.push_str("\"} ");
        body.push_str(&format!("{:.3}", histogram.average_milliseconds()));
        body.push('\n');

        body.push_str("wow_world_session_loop_phase_duration_latest_milliseconds{phase=\"");
        body.push_str(label);
        body.push_str("\"} ");
        body.push_str(&format!("{:.3}", histogram.latest_milliseconds()));
        body.push('\n');

        body.push_str("wow_world_session_loop_phase_duration_max_milliseconds{phase=\"");
        body.push_str(label);
        body.push_str("\"} ");
        body.push_str(&format!("{:.3}", histogram.max_milliseconds()));
        body.push('\n');

        body.push_str("wow_world_session_loop_phase_duration_average_1m_milliseconds{phase=\"");
        body.push_str(label);
        body.push_str("\"} ");
        body.push_str(&format!("{:.3}", rolling_1m.average_milliseconds()));
        body.push('\n');

        body.push_str("wow_world_session_loop_phase_duration_max_1m_milliseconds{phase=\"");
        body.push_str(label);
        body.push_str("\"} ");
        body.push_str(&format!("{:.3}", rolling_1m.max_milliseconds()));
        body.push('\n');

        body.push_str("wow_world_session_loop_phase_duration_average_5m_milliseconds{phase=\"");
        body.push_str(label);
        body.push_str("\"} ");
        body.push_str(&format!("{:.3}", rolling_5m.average_milliseconds()));
        body.push('\n');

        body.push_str("wow_world_session_loop_phase_duration_max_5m_milliseconds{phase=\"");
        body.push_str(label);
        body.push_str("\"} ");
        body.push_str(&format!("{:.3}", rolling_5m.max_milliseconds()));
        body.push('\n');
    }
}

fn write_histogram_metric_summary(
    body: &mut String,
    prefix: &str,
    subject: &str,
    help: &str,
    histogram: &Histogram,
) {
    write_histogram(body, &format!("{prefix}_seconds"), help, histogram);
    write_float_gauge(
        body,
        &format!("{prefix}_average_milliseconds"),
        &format!("Average {subject}."),
        histogram.average_milliseconds(),
    );
    write_float_gauge(
        body,
        &format!("{prefix}_latest_milliseconds"),
        &format!("Most recent {subject}."),
        histogram.latest_milliseconds(),
    );
    write_float_gauge(
        body,
        &format!("{prefix}_max_milliseconds"),
        &format!("Maximum observed {subject} since server start."),
        histogram.max_milliseconds(),
    );
    write_rolling_histogram_gauges(body, prefix, subject, histogram);
}

fn write_idle_motion_subphase_metrics(body: &mut String, metrics: &IdleMotionSubphaseMetrics) {
    write_numeric_summary(
        body,
        "wow_idle_motion_due_creatures",
        "due creatures advanced per idle-motion tick",
        &metrics.due_creatures,
    );
    write_numeric_summary(
        body,
        "wow_idle_motion_started_creatures",
        "creatures whose idle/confused motion started per idle-motion tick",
        &metrics.started_creatures,
    );
    write_numeric_summary(
        body,
        "wow_idle_motion_packets_emitted",
        "packets emitted per idle-motion tick",
        &metrics.packets_emitted,
    );
    write_histogram_metric_summary(
        body,
        "wow_idle_motion_advancement_queue_pop_time",
        "time spent popping due idle-motion advancement entries during one idle-motion tick",
        "Time spent popping due idle-motion advancement entries during one idle-motion tick.",
        &metrics.advancement_queue_pop_time,
    );
    write_histogram_metric_summary(
        body,
        "wow_idle_motion_advancement_validation_time",
        "time spent validating due idle-motion advancement entries during one idle-motion tick",
        "Time spent validating due idle-motion advancement entries during one idle-motion tick.",
        &metrics.advancement_validation_time,
    );
    write_histogram_metric_summary(
        body,
        "wow_idle_motion_motion_advance_time",
        "time spent advancing tracked idle-motion creature runtime during one idle-motion tick",
        "Time spent advancing tracked idle-motion creature runtime during one idle-motion tick.",
        &metrics.motion_advance_time,
    );
    write_histogram_metric_summary(
        body,
        "wow_idle_motion_spatial_update_time",
        "time spent refreshing idle-motion creature spatial state during one idle-motion tick",
        "Time spent refreshing idle-motion creature spatial state during one idle-motion tick.",
        &metrics.spatial_update_time,
    );
    write_histogram_metric_summary(
        body,
        "wow_idle_motion_start_queue_pop_time",
        "time spent collecting due idle/confused motion starts during one idle-motion tick",
        "Time spent collecting due idle/confused motion starts during one idle-motion tick.",
        &metrics.start_queue_pop_time,
    );
    write_histogram_metric_summary(
        body,
        "wow_idle_motion_motion_start_time",
        "time spent starting idle/confused motion during one idle-motion tick",
        "Time spent starting idle/confused motion during one idle-motion tick.",
        &metrics.motion_start_time,
    );
    write_histogram_metric_summary(
        body,
        "wow_idle_motion_motion_start_path_build_time",
        "time spent building idle/confused motion starts during one idle-motion tick",
        "Time spent building idle/confused motion starts during one idle-motion tick.",
        &metrics.motion_start_path_build_time,
    );
    write_histogram_metric_summary(
        body,
        "wow_idle_motion_motion_start_snapshot_clone_time",
        "time spent cloning creature snapshots for idle/confused motion starts during one idle-motion tick",
        "Time spent cloning creature snapshots for idle/confused motion starts during one idle-motion tick.",
        &metrics.motion_start_snapshot_clone_time,
    );
    write_histogram_metric_summary(
        body,
        "wow_idle_motion_motion_start_post_start_tracking_time",
        "time spent resyncing idle-motion tracking after idle/confused motion starts during one idle-motion tick",
        "Time spent resyncing idle-motion tracking after idle/confused motion starts during one idle-motion tick.",
        &metrics.motion_start_post_start_tracking_time,
    );
    write_histogram_metric_summary(
        body,
        "wow_idle_motion_motion_start_broadcast_time",
        "time spent broadcasting newly started idle/confused creature motion during one idle-motion tick",
        "Time spent broadcasting newly started idle/confused creature motion during one idle-motion tick.",
        &metrics.motion_start_broadcast_time,
    );
    write_histogram_metric_summary(
        body,
        "wow_idle_motion_motion_script_schedule_time",
        "time spent scheduling DB creature movement scripts during one idle-motion tick",
        "Time spent scheduling DB creature movement scripts during one idle-motion tick.",
        &metrics.motion_script_schedule_time,
    );
    write_histogram_metric_summary(
        body,
        "wow_idle_motion_pending_script_execution_time",
        "time spent executing due DB creature movement scripts during one idle-motion tick",
        "Time spent executing due DB creature movement scripts during one idle-motion tick.",
        &metrics.pending_script_execution_time,
    );
    write_histogram_metric_summary(
        body,
        "wow_idle_motion_start_schedule_rebuild_time",
        "time spent rebuilding idle-motion start schedules during one idle-motion tick",
        "Time spent rebuilding idle-motion start schedules during one idle-motion tick.",
        &metrics.start_schedule_rebuild_time,
    );
}

fn write_native_mmap_metrics(body: &mut String, metrics: &NativeMmapMetrics) {
    write_counter(
        body,
        "wow_native_mmap_path_calls_total",
        "Total native mmap direct path queries.",
        metrics.path_calls_total.load(Ordering::Relaxed),
    );
    write_counter(
        body,
        "wow_native_mmap_random_path_calls_total",
        "Total native mmap random path queries.",
        metrics.random_path_calls_total.load(Ordering::Relaxed),
    );
    write_native_mmap_stage_metrics(
        body,
        "wow_native_mmap_path",
        "native mmap direct path query",
        &metrics.path,
    );
    write_native_mmap_stage_metrics(
        body,
        "wow_native_mmap_random_path",
        "native mmap random path query",
        &metrics.random_path,
    );
}

fn write_native_mmap_stage_metrics(
    body: &mut String,
    prefix: &str,
    subject: &str,
    metrics: &NativeMmapStageMetrics,
) {
    write_histogram_metric_summary(
        body,
        &format!("{prefix}_lock_and_tile_load_time"),
        &format!("time spent loading native mmap cache state for one {subject}"),
        &format!("Time spent loading native mmap cache state for one {subject}."),
        &metrics.lock_and_tile_load_time,
    );
    write_histogram_metric_summary(
        body,
        &format!("{prefix}_query_alloc_init_time"),
        &format!("time spent allocating and initializing Detour query state for one {subject}"),
        &format!("Time spent allocating and initializing Detour query state for one {subject}."),
        &metrics.query_alloc_init_time,
    );
    write_histogram_metric_summary(
        body,
        &format!("{prefix}_find_nearest_poly_time"),
        &format!("time spent finding start and target nav polys for one {subject}"),
        &format!("Time spent finding start and target nav polys for one {subject}."),
        &metrics.find_nearest_poly_time,
    );
    write_histogram_metric_summary(
        body,
        &format!("{prefix}_find_path_time"),
        &format!("time spent running Detour corridor pathfinding for one {subject}"),
        &format!("Time spent running Detour corridor pathfinding for one {subject}."),
        &metrics.find_path_time,
    );
    write_histogram_metric_summary(
        body,
        &format!("{prefix}_find_smooth_path_time"),
        &format!("time spent smoothing Detour corridor output for one {subject}"),
        &format!("Time spent smoothing Detour corridor output for one {subject}."),
        &metrics.find_smooth_path_time,
    );
}

fn write_player_environment_subphase_metrics(
    body: &mut String,
    metrics: &PlayerEnvironmentSubphaseMetrics,
) {
    write_numeric_summary(
        body,
        "wow_player_environment_players_scanned",
        "non-bot players scanned per player-environment tick",
        &metrics.players_scanned,
    );
    write_numeric_summary(
        body,
        "wow_player_environment_packets_emitted",
        "packets emitted per player-environment tick",
        &metrics.packets_emitted,
    );
    write_histogram(
        body,
        "wow_player_environment_flags_time_seconds",
        "Time spent sampling environment flags during one player-environment tick.",
        &metrics.environment_flags_time,
    );
    write_float_gauge(
        body,
        "wow_player_environment_flags_time_average_milliseconds",
        "Average time spent sampling environment flags during one player-environment tick.",
        metrics.environment_flags_time.average_milliseconds(),
    );
    write_float_gauge(
        body,
        "wow_player_environment_flags_time_latest_milliseconds",
        "Most recent time spent sampling environment flags during one player-environment tick.",
        metrics.environment_flags_time.latest_milliseconds(),
    );
    write_float_gauge(
        body,
        "wow_player_environment_flags_time_max_milliseconds",
        "Maximum observed environment flag sampling time since server start.",
        metrics.environment_flags_time.max_milliseconds(),
    );
    write_rolling_histogram_gauges(
        body,
        "wow_player_environment_flags_time",
        "time spent sampling environment flags during one player-environment tick",
        &metrics.environment_flags_time,
    );
    write_histogram(
        body,
        "wow_player_environment_timer_update_time_seconds",
        "Time spent advancing environment timers during one player-environment tick.",
        &metrics.timer_update_time,
    );
    write_float_gauge(
        body,
        "wow_player_environment_timer_update_time_average_milliseconds",
        "Average time spent advancing environment timers during one player-environment tick.",
        metrics.timer_update_time.average_milliseconds(),
    );
    write_float_gauge(
        body,
        "wow_player_environment_timer_update_time_latest_milliseconds",
        "Most recent time spent advancing environment timers during one player-environment tick.",
        metrics.timer_update_time.latest_milliseconds(),
    );
    write_float_gauge(
        body,
        "wow_player_environment_timer_update_time_max_milliseconds",
        "Maximum observed environment timer update time since server start.",
        metrics.timer_update_time.max_milliseconds(),
    );
    write_rolling_histogram_gauges(
        body,
        "wow_player_environment_timer_update_time",
        "time spent advancing environment timers during one player-environment tick",
        &metrics.timer_update_time,
    );
    write_histogram(
        body,
        "wow_player_environment_damage_application_time_seconds",
        "Time spent applying environmental damage during one player-environment tick.",
        &metrics.damage_application_time,
    );
    write_float_gauge(
        body,
        "wow_player_environment_damage_application_time_average_milliseconds",
        "Average time spent applying environmental damage during one player-environment tick.",
        metrics.damage_application_time.average_milliseconds(),
    );
    write_float_gauge(
        body,
        "wow_player_environment_damage_application_time_latest_milliseconds",
        "Most recent time spent applying environmental damage during one player-environment tick.",
        metrics.damage_application_time.latest_milliseconds(),
    );
    write_float_gauge(
        body,
        "wow_player_environment_damage_application_time_max_milliseconds",
        "Maximum observed environmental damage application time since server start.",
        metrics.damage_application_time.max_milliseconds(),
    );
    write_rolling_histogram_gauges(
        body,
        "wow_player_environment_damage_application_time",
        "time spent applying environmental damage during one player-environment tick",
        &metrics.damage_application_time,
    );
    write_histogram(
        body,
        "wow_player_environment_nearby_fanout_time_seconds",
        "Time spent broadcasting nearby player-environment updates during one player-environment tick.",
        &metrics.nearby_fanout_time,
    );
    write_float_gauge(
        body,
        "wow_player_environment_nearby_fanout_time_average_milliseconds",
        "Average time spent broadcasting nearby player-environment updates during one player-environment tick.",
        metrics.nearby_fanout_time.average_milliseconds(),
    );
    write_float_gauge(
        body,
        "wow_player_environment_nearby_fanout_time_latest_milliseconds",
        "Most recent time spent broadcasting nearby player-environment updates during one player-environment tick.",
        metrics.nearby_fanout_time.latest_milliseconds(),
    );
    write_float_gauge(
        body,
        "wow_player_environment_nearby_fanout_time_max_milliseconds",
        "Maximum observed nearby player-environment fanout time since server start.",
        metrics.nearby_fanout_time.max_milliseconds(),
    );
    write_rolling_histogram_gauges(
        body,
        "wow_player_environment_nearby_fanout_time",
        "time spent broadcasting nearby player-environment updates during one player-environment tick",
        &metrics.nearby_fanout_time,
    );
}

fn write_map_runtime_gauges(
    body: &mut String,
    snapshots: &Mutex<HashMap<(u32, u32), MapRuntimeSnapshot>>,
) {
    let mut values: Vec<MapRuntimeSnapshot> = snapshots
        .lock()
        .expect("metrics map runtime snapshot gauge poisoned")
        .values()
        .copied()
        .collect();
    values.sort_by_key(|snapshot| (snapshot.map_id, snapshot.instance_id));

    write_map_runtime_gauge_family(
        body,
        "wow_map_active_players",
        "Currently active players by map runtime.",
        &values,
        |snapshot| snapshot.active_players,
    );
    write_map_runtime_gauge_family(
        body,
        "wow_map_active_playerbots",
        "Currently active playerbot actors by map runtime.",
        &values,
        |snapshot| snapshot.active_playerbots,
    );
    write_map_runtime_gauge_family(
        body,
        "wow_map_tracked_idle_motion_creatures",
        "DB creatures currently tracked in the idle-motion advancement set by map runtime.",
        &values,
        |snapshot| snapshot.tracked_idle_motion_creatures,
    );
    write_map_runtime_gauge_family(
        body,
        "wow_map_tracked_idle_motion_start_candidates",
        "DB creatures queued in idle/confused motion start schedules by map runtime.",
        &values,
        |snapshot| snapshot.tracked_idle_motion_start_candidates,
    );
    write_map_runtime_gauge_family(
        body,
        "wow_map_active_creatures",
        "Currently active DB creatures by map runtime.",
        &values,
        |snapshot| snapshot.active_creatures,
    );
    write_map_runtime_gauge_family(
        body,
        "wow_map_active_gameobjects",
        "Currently active DB gameobjects by map runtime.",
        &values,
        |snapshot| snapshot.active_gameobjects,
    );
    write_map_runtime_gauge_family(
        body,
        "wow_map_loaded_grids",
        "Loaded spatial grids by map runtime.",
        &values,
        |snapshot| snapshot.loaded_grids,
    );
    write_map_runtime_gauge_family(
        body,
        "wow_map_loaded_creature_grids",
        "Loaded creature grids by map runtime.",
        &values,
        |snapshot| snapshot.loaded_creature_grids,
    );
    write_map_runtime_gauge_family(
        body,
        "wow_map_loaded_gameobject_grids",
        "Loaded gameobject grids by map runtime.",
        &values,
        |snapshot| snapshot.loaded_gameobject_grids,
    );
    write_map_runtime_gauge_family(
        body,
        "wow_map_loaded_player_corpse_grids",
        "Loaded player corpse grids by map runtime.",
        &values,
        |snapshot| snapshot.loaded_player_corpse_grids,
    );
    write_map_runtime_gauge_family(
        body,
        "wow_map_active_creature_combats",
        "Active creature combat states by map runtime.",
        &values,
        |snapshot| snapshot.active_creature_combats,
    );
    write_map_runtime_gauge_family(
        body,
        "wow_map_corpses",
        "Tracked player corpses by map runtime.",
        &values,
        |snapshot| snapshot.corpses,
    );
}

fn write_map_runtime_gauge_family(
    body: &mut String,
    name: &str,
    help: &str,
    snapshots: &[MapRuntimeSnapshot],
    value: fn(&MapRuntimeSnapshot) -> u64,
) {
    body.push_str("# HELP ");
    body.push_str(name);
    body.push(' ');
    body.push_str(help);
    body.push('\n');
    body.push_str("# TYPE ");
    body.push_str(name);
    body.push_str(" gauge\n");

    for snapshot in snapshots {
        body.push_str(name);
        body.push_str("{map_id=\"");
        body.push_str(&snapshot.map_id.to_string());
        body.push_str("\",instance_id=\"");
        body.push_str(&snapshot.instance_id.to_string());
        body.push_str("\"} ");
        body.push_str(&value(snapshot).to_string());
        body.push('\n');
    }
}

fn write_static_world_cache_metrics(
    body: &mut String,
    loads: &Mutex<HashMap<&'static str, StaticWorldCacheLoadStats>>,
    lookups: &Mutex<HashMap<&'static str, DurationStats>>,
    instantiations: &Mutex<HashMap<&'static str, DurationStats>>,
) {
    let mut load_values = loads
        .lock()
        .expect("metrics static world cache load registry poisoned")
        .iter()
        .map(|(kind, stats)| (*kind, *stats))
        .collect::<Vec<_>>();
    load_values.sort_by_key(|(kind, _)| *kind);
    write_static_cache_load_gauge(
        body,
        "wow_static_world_cache_load_spawns",
        "Static world cache spawn rows loaded at startup.",
        &load_values,
        |stats| stats.spawns as f64,
    );
    write_static_cache_load_gauge(
        body,
        "wow_static_world_cache_load_grids",
        "Static world cache populated grids at startup.",
        &load_values,
        |stats| stats.grids as f64,
    );
    write_static_cache_load_gauge(
        body,
        "wow_static_world_cache_load_duration_milliseconds",
        "Static world cache startup load duration.",
        &load_values,
        StaticWorldCacheLoadStats::duration_milliseconds,
    );

    write_static_cache_duration_metrics(
        body,
        "wow_static_world_cache_lookup",
        "Static world cache grid lookup",
        lookups,
        false,
    );
    write_static_cache_duration_metrics(
        body,
        "wow_static_world_cache_instantiation",
        "Static world cache runtime instantiation",
        instantiations,
        true,
    );
}

fn write_static_cache_load_gauge(
    body: &mut String,
    name: &str,
    help: &str,
    values: &[(&'static str, StaticWorldCacheLoadStats)],
    value: fn(StaticWorldCacheLoadStats) -> f64,
) {
    body.push_str("# HELP ");
    body.push_str(name);
    body.push(' ');
    body.push_str(help);
    body.push('\n');
    body.push_str("# TYPE ");
    body.push_str(name);
    body.push_str(" gauge\n");
    for (kind, stats) in values {
        body.push_str(name);
        body.push_str("{kind=\"");
        body.push_str(kind);
        body.push_str("\"} ");
        body.push_str(&format!("{:.3}", value(*stats)));
        body.push('\n');
    }
}

fn write_static_cache_duration_metrics(
    body: &mut String,
    name_prefix: &str,
    help_prefix: &str,
    stats: &Mutex<HashMap<&'static str, DurationStats>>,
    include_rows: bool,
) {
    let mut values = stats
        .lock()
        .expect("metrics static world cache duration registry poisoned")
        .iter()
        .map(|(kind, stats)| (*kind, *stats))
        .collect::<Vec<_>>();
    values.sort_by_key(|(kind, _)| *kind);

    write_static_cache_duration_counter(
        body,
        &format!("{name_prefix}_total"),
        &format!("{help_prefix} calls."),
        &values,
        |stats| stats.count,
    );
    if include_rows {
        write_static_cache_duration_counter(
            body,
            &format!("{name_prefix}_rows_total"),
            &format!("{help_prefix} spawn rows."),
            &values,
            |stats| stats.rows,
        );
    }
    write_static_cache_duration_gauge(
        body,
        &format!("{name_prefix}_duration_average_milliseconds"),
        &format!("Average {help_prefix} duration."),
        &values,
        DurationStats::average_milliseconds,
    );
    write_static_cache_duration_gauge(
        body,
        &format!("{name_prefix}_duration_latest_milliseconds"),
        &format!("Most recent {help_prefix} duration."),
        &values,
        DurationStats::latest_milliseconds,
    );
    write_static_cache_duration_gauge(
        body,
        &format!("{name_prefix}_duration_max_milliseconds"),
        &format!("Maximum observed {help_prefix} duration since server start."),
        &values,
        DurationStats::max_milliseconds,
    );
}

fn write_static_cache_duration_counter(
    body: &mut String,
    name: &str,
    help: &str,
    values: &[(&'static str, DurationStats)],
    value: fn(DurationStats) -> u64,
) {
    body.push_str("# HELP ");
    body.push_str(name);
    body.push(' ');
    body.push_str(help);
    body.push('\n');
    body.push_str("# TYPE ");
    body.push_str(name);
    body.push_str(" counter\n");
    for (kind, stats) in values {
        body.push_str(name);
        body.push_str("{kind=\"");
        body.push_str(kind);
        body.push_str("\"} ");
        body.push_str(&value(*stats).to_string());
        body.push('\n');
    }
}

fn write_static_cache_duration_gauge(
    body: &mut String,
    name: &str,
    help: &str,
    values: &[(&'static str, DurationStats)],
    value: fn(DurationStats) -> f64,
) {
    body.push_str("# HELP ");
    body.push_str(name);
    body.push(' ');
    body.push_str(help);
    body.push('\n');
    body.push_str("# TYPE ");
    body.push_str(name);
    body.push_str(" gauge\n");
    for (kind, stats) in values {
        body.push_str(name);
        body.push_str("{kind=\"");
        body.push_str(kind);
        body.push_str("\"} ");
        body.push_str(&format!("{:.3}", value(*stats)));
        body.push('\n');
    }
}

fn write_phase_float_gauges(
    body: &mut String,
    name: &str,
    help: &str,
    phases: &MapPhaseDurations,
    value: fn(&Histogram) -> f64,
) {
    body.push_str("# HELP ");
    body.push_str(name);
    body.push(' ');
    body.push_str(help);
    body.push('\n');
    body.push_str("# TYPE ");
    body.push_str(name);
    body.push_str(" gauge\n");
    for phase in MAP_TICK_PHASES {
        body.push_str(name);
        body.push_str("{phase=\"");
        body.push_str(phase.label());
        body.push_str("\"} ");
        body.push_str(&format!("{:.3}", value(phases.get(phase))));
        body.push('\n');
    }
}

fn write_phase_rolling_gauges(
    body: &mut String,
    name: &str,
    help: &str,
    phases: &MapPhaseDurations,
    window: Duration,
    value: fn(RollingStats) -> f64,
) {
    body.push_str("# HELP ");
    body.push_str(name);
    body.push(' ');
    body.push_str(help);
    body.push('\n');
    body.push_str("# TYPE ");
    body.push_str(name);
    body.push_str(" gauge\n");
    for phase in MAP_TICK_PHASES {
        body.push_str(name);
        body.push_str("{phase=\"");
        body.push_str(phase.label());
        body.push_str("\"} ");
        body.push_str(&format!(
            "{:.3}",
            value(phases.get(phase).rolling_stats(window))
        ));
        body.push('\n');
    }
}

fn write_opcode_counter(
    body: &mut String,
    name: &str,
    help: &str,
    counters: &Mutex<HashMap<u32, u64>>,
) {
    body.push_str("# HELP ");
    body.push_str(name);
    body.push(' ');
    body.push_str(help);
    body.push('\n');
    body.push_str("# TYPE ");
    body.push_str(name);
    body.push_str(" counter\n");

    let mut values: Vec<(u32, u64)> = counters
        .lock()
        .expect("metrics opcode counter poisoned")
        .iter()
        .map(|(opcode, count)| (*opcode, *count))
        .collect();
    values.sort_by_key(|(opcode, _)| *opcode);

    for (opcode, count) in values {
        body.push_str(name);
        body.push_str("{opcode=\"");
        body.push_str(&format_opcode(opcode));
        body.push_str("\"} ");
        body.push_str(&count.to_string());
        body.push('\n');
    }
}

fn write_opcode_histogram_metric_summaries(
    body: &mut String,
    prefix: &str,
    subject: &str,
    _help: &str,
    histograms: &Mutex<HashMap<u32, Histogram>>,
) {
    write_opcode_histogram_gauge(
        body,
        &format!("{prefix}_average_milliseconds"),
        &format!("Average {subject}."),
        histograms,
        Histogram::average_milliseconds,
    );
    write_opcode_histogram_gauge(
        body,
        &format!("{prefix}_latest_milliseconds"),
        &format!("Most recent {subject}."),
        histograms,
        Histogram::latest_milliseconds,
    );
    write_opcode_histogram_gauge(
        body,
        &format!("{prefix}_max_milliseconds"),
        &format!("Maximum observed {subject} since server start."),
        histograms,
        Histogram::max_milliseconds,
    );
    write_opcode_histogram_rolling_gauge(
        body,
        &format!("{prefix}_average_1m_milliseconds"),
        &format!("Average {subject} over the last minute."),
        histograms,
        ROLLING_ONE_MINUTE,
        RollingStats::average_milliseconds,
    );
    write_opcode_histogram_rolling_gauge(
        body,
        &format!("{prefix}_max_1m_milliseconds"),
        &format!("Maximum observed {subject} over the last minute."),
        histograms,
        ROLLING_ONE_MINUTE,
        RollingStats::max_milliseconds,
    );
    write_opcode_histogram_rolling_gauge(
        body,
        &format!("{prefix}_average_5m_milliseconds"),
        &format!("Average {subject} over the last five minutes."),
        histograms,
        ROLLING_FIVE_MINUTES,
        RollingStats::average_milliseconds,
    );
    write_opcode_histogram_rolling_gauge(
        body,
        &format!("{prefix}_max_5m_milliseconds"),
        &format!("Maximum observed {subject} over the last five minutes."),
        histograms,
        ROLLING_FIVE_MINUTES,
        RollingStats::max_milliseconds,
    );
}

fn write_opcode_histogram_gauge(
    body: &mut String,
    name: &str,
    help: &str,
    histograms: &Mutex<HashMap<u32, Histogram>>,
    value: fn(&Histogram) -> f64,
) {
    body.push_str("# HELP ");
    body.push_str(name);
    body.push(' ');
    body.push_str(help);
    body.push('\n');
    body.push_str("# TYPE ");
    body.push_str(name);
    body.push_str(" gauge\n");

    let mut values = histograms
        .lock()
        .expect("metrics opcode histogram poisoned")
        .iter()
        .map(|(opcode, histogram)| (*opcode, value(histogram)))
        .collect::<Vec<_>>();
    values.sort_by_key(|(opcode, _)| *opcode);

    for (opcode, metric_value) in values {
        body.push_str(name);
        body.push_str("{opcode=\"");
        body.push_str(&format_opcode(opcode));
        body.push_str("\"} ");
        body.push_str(&format!("{metric_value:.3}"));
        body.push('\n');
    }
}

fn write_opcode_histogram_rolling_gauge(
    body: &mut String,
    name: &str,
    help: &str,
    histograms: &Mutex<HashMap<u32, Histogram>>,
    window: Duration,
    value: fn(RollingStats) -> f64,
) {
    body.push_str("# HELP ");
    body.push_str(name);
    body.push(' ');
    body.push_str(help);
    body.push('\n');
    body.push_str("# TYPE ");
    body.push_str(name);
    body.push_str(" gauge\n");

    let mut values = histograms
        .lock()
        .expect("metrics opcode histogram poisoned")
        .iter()
        .map(|(opcode, histogram)| (*opcode, value(histogram.rolling_stats(window))))
        .collect::<Vec<_>>();
    values.sort_by_key(|(opcode, _)| *opcode);

    for (opcode, metric_value) in values {
        body.push_str(name);
        body.push_str("{opcode=\"");
        body.push_str(&format_opcode(opcode));
        body.push_str("\"} ");
        body.push_str(&format!("{metric_value:.3}"));
        body.push('\n');
    }
}

fn format_bucket(bucket: f64) -> String {
    format!("{bucket:.3}")
}

fn format_opcode(opcode: u32) -> String {
    format!("0x{opcode:04X}")
}

fn render_playerbot_diagnostics_from_request(first_line: &str) -> String {
    let path = first_line
        .split_whitespace()
        .nth(1)
        .unwrap_or("/playerbots");
    let requested_names = playerbot_diagnostic_requested_names(path);
    render_playerbot_diagnostics(requested_names.as_deref())
}

fn playerbot_diagnostic_requested_names(path: &str) -> Option<Vec<String>> {
    let query = path.split_once('?')?.1;
    for part in query.split('&') {
        let (key, value) = part.split_once('=')?;
        if key != "names" {
            continue;
        }
        let names = value
            .split(',')
            .map(percent_decode_query_component)
            .map(|name| name.trim().to_string())
            .filter(|name| !name.is_empty())
            .collect::<Vec<_>>();
        return Some(names);
    }
    None
}

fn percent_decode_query_component(value: &str) -> String {
    let bytes = value.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        match bytes[index] {
            b'+' => {
                out.push(b' ');
                index += 1;
            }
            b'%' if index + 2 < bytes.len() => {
                let hex = &value[index + 1..index + 3];
                if let Ok(byte) = u8::from_str_radix(hex, 16) {
                    out.push(byte);
                    index += 3;
                } else {
                    out.push(bytes[index]);
                    index += 1;
                }
            }
            byte => {
                out.push(byte);
                index += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn render_playerbot_diagnostics(requested_names: Option<&[String]>) -> String {
    let metrics = registry();
    let names = metrics
        .playerbot_names
        .lock()
        .expect("metrics playerbot names poisoned");
    let snapshots = metrics
        .playerbot_debug_snapshots
        .lock()
        .expect("metrics playerbot debug snapshots poisoned");

    let mut name_to_guid = names
        .iter()
        .map(|(guid, name)| (name.to_ascii_lowercase(), *guid))
        .collect::<HashMap<_, _>>();
    let mut rows = Vec::new();
    match requested_names {
        Some(requested_names) => {
            for name in requested_names {
                let key = name.to_ascii_lowercase();
                let Some(guid) = name_to_guid.remove(&key) else {
                    rows.push(format!("{name}: not found"));
                    continue;
                };
                match snapshots.get(&guid) {
                    Some(snapshot) => rows.push(format_playerbot_debug_row(
                        names.get(&guid).map(String::as_str).unwrap_or(name),
                        snapshot,
                    )),
                    None => rows.push(format!("{name} ({guid}): no runtime snapshot")),
                }
            }
        }
        None => {
            let mut entries = snapshots
                .iter()
                .map(|(guid, snapshot)| {
                    (
                        names.get(guid).map(String::as_str).unwrap_or("<unnamed>"),
                        snapshot,
                    )
                })
                .collect::<Vec<_>>();
            entries.sort_by(|left, right| left.0.cmp(right.0));
            for (name, snapshot) in entries {
                rows.push(format_playerbot_debug_row(name, snapshot));
            }
        }
    }

    if rows.is_empty() {
        return "no playerbot diagnostics available\n".to_string();
    }
    rows.join("\n") + "\n"
}

fn format_playerbot_debug_row(name: &str, snapshot: &PlayerbotDebugSnapshot) -> String {
    let travel = match (
        snapshot.travel_x,
        snapshot.travel_y,
        snapshot.travel_z,
        snapshot.distance_to_travel,
    ) {
        (Some(x), Some(y), Some(z), Some(distance)) => {
            format!(" target=({x:.2},{y:.2},{z:.2}) target_dist={distance:.1}yd")
        }
        _ => " target=<none>".to_string(),
    };
    let leg = match (
        snapshot.active_leg_destination_x,
        snapshot.active_leg_destination_y,
        snapshot.active_leg_destination_z,
        snapshot.active_leg_remaining_millis,
    ) {
        (Some(x), Some(y), Some(z), Some(remaining)) => {
            format!(" leg_dest=({x:.2},{y:.2},{z:.2}) leg_remaining={remaining}ms")
        }
        _ => " leg_dest=<none>".to_string(),
    };
    format!(
        "{} ({}) state={} map={} pos=({:.2},{:.2},{:.2}){}{} route_len={} next_think={}ms flags=0x{:X}",
        name,
        snapshot.guid,
        snapshot.state,
        snapshot.map_id,
        snapshot.x,
        snapshot.y,
        snapshot.z,
        travel,
        leg,
        snapshot.route_len,
        snapshot.next_think_in_millis,
        snapshot.movement_flags,
    )
}

pub fn render_dashboard_html() -> &'static str {
    DASHBOARD_HTML
}

const DASHBOARD_HTML: &str = r##"<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>Worldserver Monitor</title>
<style>
:root {
  color-scheme: dark;
  --bg: #101214;
  --panel: #181c20;
  --panel-2: #20262b;
  --text: #eef2f4;
  --muted: #97a3ad;
  --line: #303941;
  --good: #48c78e;
  --warn: #f2b84b;
  --bad: #ef6b73;
  --info: #64b5f6;
}
* { box-sizing: border-box; }
body {
  margin: 0;
  background: var(--bg);
  color: var(--text);
  font-family: "Segoe UI", system-ui, -apple-system, BlinkMacSystemFont, sans-serif;
  letter-spacing: 0;
}
button {
  border: 1px solid var(--line);
  border-radius: 6px;
  background: var(--panel-2);
  color: var(--text);
  padding: 7px 10px;
  font: inherit;
  cursor: pointer;
}
button:hover { border-color: var(--info); }
main {
  width: min(1440px, calc(100vw - 32px));
  margin: 0 auto;
  padding: 18px 0 24px;
}
header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 16px;
  padding: 0 0 14px;
  border-bottom: 1px solid var(--line);
}
h1 {
  margin: 0;
  font-size: 22px;
  font-weight: 650;
}
h2 {
  margin: 0 0 10px;
  font-size: 14px;
  font-weight: 650;
  color: var(--muted);
  text-transform: uppercase;
}
.toolbar {
  display: flex;
  align-items: center;
  gap: 10px;
  color: var(--muted);
  font-size: 13px;
  white-space: nowrap;
}
.status-dot {
  width: 10px;
  height: 10px;
  border-radius: 50%;
  background: var(--warn);
  display: inline-block;
}
.status-dot.good { background: var(--good); }
.status-dot.bad { background: var(--bad); }
.grid {
  display: grid;
  gap: 12px;
}
.kpis {
  grid-template-columns: repeat(6, minmax(0, 1fr));
  margin-top: 14px;
}
.card, .section {
  background: var(--panel);
  border: 1px solid var(--line);
  border-radius: 8px;
}
.card {
  min-height: 92px;
  padding: 12px;
}
.label {
  color: var(--muted);
  font-size: 12px;
  line-height: 1.25;
}
.value {
  margin-top: 8px;
  font-size: 26px;
  font-weight: 700;
  line-height: 1.05;
}
.sub {
  margin-top: 7px;
  color: var(--muted);
  font-size: 12px;
}
.section {
  padding: 14px;
  min-width: 0;
}
.layout {
  grid-template-columns: minmax(0, 1.35fr) minmax(360px, 0.65fr);
  margin-top: 12px;
}
.two {
  grid-template-columns: repeat(2, minmax(0, 1fr));
  margin-top: 12px;
}
.chart {
  width: 100%;
  height: 190px;
  border: 1px solid var(--line);
  border-radius: 6px;
  background: #0c0e10;
  display: block;
}
.solo-chart {
  margin-top: 12px;
}
table {
  width: 100%;
  border-collapse: collapse;
  font-size: 13px;
}
th, td {
  padding: 7px 8px;
  border-bottom: 1px solid var(--line);
  text-align: right;
  white-space: nowrap;
}
th:first-child, td:first-child {
  text-align: left;
  white-space: normal;
}
th {
  color: var(--muted);
  font-size: 12px;
  font-weight: 600;
}
tr:last-child td { border-bottom: 0; }
.phase-row, .map-row {
  display: grid;
  grid-template-columns: minmax(130px, 0.9fr) minmax(0, 1.3fr) 76px 76px 76px;
  gap: 8px;
  align-items: center;
  padding: 7px 0;
  border-bottom: 1px solid var(--line);
  font-size: 13px;
}
.phase-row:last-child, .map-row:last-child { border-bottom: 0; }
.bar {
  height: 9px;
  background: #0c0e10;
  border-radius: 4px;
  overflow: hidden;
}
.fill {
  height: 100%;
  width: 0%;
  background: var(--info);
}
.fill.warn { background: var(--warn); }
.fill.bad { background: var(--bad); }
.metric-good { color: var(--good); }
.metric-warn { color: var(--warn); }
.metric-bad { color: var(--bad); }
.empty {
  color: var(--muted);
  font-size: 13px;
  padding: 16px 0;
}
@media (max-width: 1080px) {
  .kpis { grid-template-columns: repeat(3, minmax(0, 1fr)); }
  .layout, .two { grid-template-columns: 1fr; }
}
@media (max-width: 640px) {
  main { width: min(100vw - 20px, 1440px); padding-top: 12px; }
  header { align-items: flex-start; flex-direction: column; }
  .toolbar { flex-wrap: wrap; white-space: normal; }
  .kpis { grid-template-columns: repeat(2, minmax(0, 1fr)); }
  .value { font-size: 22px; }
  .phase-row, .map-row {
    grid-template-columns: minmax(100px, 1fr) 64px 64px 64px;
  }
  .phase-row .bar, .map-row .bar { display: none; }
}
</style>
</head>
<body>
<main>
  <header>
    <div>
      <h1>Worldserver Monitor</h1>
      <div class="sub">localhost:9091</div>
    </div>
    <div class="toolbar">
      <span id="statusDot" class="status-dot"></span>
      <span id="statusText">connecting</span>
      <span id="lastUpdated">never</span>
      <button id="pauseButton" type="button">Pause</button>
      <button id="resetButton" type="button">Mark Session</button>
      <a href="/metrics" style="color: var(--info); text-decoration: none;">Metrics</a>
    </div>
  </header>

  <section class="grid kpis">
    <div class="card"><div class="label">Sessions</div><div id="sessions" class="value">0</div><div id="sessionSub" class="sub">registered 0</div></div>
    <div class="card"><div class="label">Loop Avg 1m</div><div id="loopAvg" class="value">0.000 ms</div><div id="loopLatest" class="sub">latest 0.000 ms</div></div>
    <div class="card"><div class="label">Loop Max 1m</div><div id="loopMax" class="value">0.000 ms</div><div id="loopBudget" class="sub">lifetime max 0.000 ms</div></div>
    <div class="card"><div class="label">Lag Avg 1m</div><div id="lagAvg" class="value">0.000 ms</div><div id="lagLatest" class="sub">latest 0.000 ms</div></div>
    <div class="card"><div class="label">DB Slowest 1m</div><div id="dbSlowest" class="value">0.000 ms</div><div id="dbSlowestFamily" class="sub">no samples</div></div>
    <div class="card"><div class="label">Unknown Opcodes</div><div id="unknownTotal" class="value">0</div><div id="unknownSub" class="sub">families 0</div></div>
  </section>

  <section class="grid layout">
    <div class="section">
      <h2>Loop Timing</h2>
      <canvas id="loopChart" class="chart" width="900" height="260"></canvas>
    </div>
    <div class="section">
      <h2>Map Runtime</h2>
      <div id="mapTable"></div>
    </div>
  </section>

  <section class="section solo-chart">
    <h2>Loop Avg 10s</h2>
    <canvas id="loopAvg10sChart" class="chart" width="900" height="220"></canvas>
  </section>

  <section class="grid two">
    <div class="section">
      <h2>Map Phases</h2>
      <div id="phaseTable"></div>
    </div>
    <div class="section">
      <h2>Static World Cache</h2>
      <div id="cacheTable"></div>
    </div>
    <div class="section">
      <h2>DB Query Families</h2>
      <div id="dbTable"></div>
    </div>
  </section>

  <section class="grid two">
    <div class="section">
      <h2>Packets In</h2>
      <div id="packetsIn"></div>
    </div>
    <div class="section">
      <h2>Unknown Opcodes</h2>
      <div id="unknownTable"></div>
    </div>
  </section>

  <section class="section solo-chart">
    <h2>Packet Latency</h2>
    <div id="packetLatencyTable"></div>
  </section>

  <section class="section solo-chart">
    <h2>Session Loop</h2>
    <div id="sessionLoopTable"></div>
  </section>
</main>

<script>
const state = {
  paused: false,
  history: [],
  avg10sHistory: [],
  previous: new Map(),
  lastTickCount: 0
};

const $ = (id) => document.getElementById(id);
const metricKey = (name, labels) => {
  const parts = Object.keys(labels).sort().map((key) => `${key}=${labels[key]}`).join(",");
  return parts ? `${name}{${parts}}` : name;
};
const fmt = (value, digits = 3) => Number.isFinite(value) ? value.toFixed(digits) : "0.000";
const intFmt = (value) => Number.isFinite(value) ? Math.round(value).toLocaleString() : "0";
const classForMs = (value, warn, bad) => value >= bad ? "metric-bad" : value >= warn ? "metric-warn" : "metric-good";
const watchedPacketOpcodes = [
  { opcode: "0x012E", label: "CastSpell" },
  { opcode: "0x00EE", label: "MoveHeartbeat" },
  { opcode: "0x00B5", label: "MoveStartForward" },
  { opcode: "0x00B7", label: "MoveStop" },
  { opcode: "0x00BB", label: "MoveJump" },
  { opcode: "0x00C9", label: "MoveLand" },
  { opcode: "0x00DA", label: "MoveSetFacing" }
];

function parseMetrics(text) {
  const metrics = new Map();
  for (const raw of text.split("\n")) {
    const line = raw.trim();
    if (!line || line.startsWith("#")) continue;
    const match = line.match(/^([^\s{]+)(?:\{([^}]*)\})?\s+([-+0-9.eE]+)$/);
    if (!match) continue;
    const labels = {};
    if (match[2]) {
      for (const part of match[2].matchAll(/([^=,]+)="([^"]*)"/g)) {
        labels[part[1]] = part[2];
      }
    }
    const name = match[1];
    const value = Number(match[3]);
    metrics.set(metricKey(name, labels), { name, labels, value });
  }
  return metrics;
}

function get(metrics, key) {
  return metrics.get(key)?.value ?? 0;
}

function getLabeled(metrics, name, labels) {
  return metrics.get(metricKey(name, labels))?.value ?? 0;
}

function series(metrics, name) {
  return [...metrics.values()].filter((item) => item.name === name);
}

function renderKpis(metrics) {
  const connected = get(metrics, "wow_world_sessions_connected");
  const registered = get(metrics, "wow_world_sessions_registered_total");
  const unregistered = get(metrics, "wow_world_sessions_unregistered_total");
  const durationAvg = get(metrics, "wow_map_tick_duration_average_milliseconds");
  const durationAvg1m = get(metrics, "wow_map_tick_duration_average_1m_milliseconds") || durationAvg;
  const durationLatest = get(metrics, "wow_map_tick_duration_latest_milliseconds");
  const durationMax = get(metrics, "wow_map_tick_duration_max_milliseconds");
  const durationMax1m = get(metrics, "wow_map_tick_duration_max_1m_milliseconds") || durationMax;
  const lagAvg = get(metrics, "wow_map_tick_lag_average_milliseconds");
  const lagAvg1m = get(metrics, "wow_map_tick_lag_average_1m_milliseconds") || lagAvg;
  const lagLatest = get(metrics, "wow_map_tick_lag_latest_milliseconds");
  const overBudget = get(metrics, "wow_map_tick_over_budget_total");
  const sessionStarted = get(metrics, "wow_monitoring_session_started_unix_seconds");
  const dbMaxRows = (series(metrics, "wow_db_query_duration_max_1m_milliseconds").length
    ? series(metrics, "wow_db_query_duration_max_1m_milliseconds")
    : series(metrics, "wow_db_query_duration_max_milliseconds"))
    .sort((a, b) => b.value - a.value);
  const unknownRows = series(metrics, "wow_world_unknown_opcodes_total")
    .sort((a, b) => b.value - a.value);
  const unknownTotal = unknownRows.reduce((sum, row) => sum + row.value, 0);

  $("sessions").textContent = intFmt(connected);
  const sessionText = sessionStarted > 0 ? new Date(sessionStarted * 1000).toLocaleTimeString() : "startup";
  $("sessionSub").textContent = `registered ${intFmt(registered)} / marked ${sessionText} / left ${intFmt(unregistered)}`;
  $("loopAvg").textContent = `${fmt(durationAvg1m)} ms`;
  $("loopAvg").className = `value ${classForMs(durationAvg1m, 25, 100)}`;
  $("loopLatest").textContent = `latest ${fmt(durationLatest)} ms`;
  $("loopMax").textContent = `${fmt(durationMax1m)} ms`;
  $("loopMax").className = `value ${classForMs(durationMax1m, 50, 100)}`;
  $("loopBudget").textContent = `lifetime max ${fmt(durationMax)} ms / over budget ${intFmt(overBudget)}`;
  $("lagAvg").textContent = `${fmt(lagAvg1m)} ms`;
  $("lagLatest").textContent = `latest ${fmt(lagLatest)} ms`;
  $("dbSlowest").textContent = `${fmt(dbMaxRows[0]?.value ?? 0)} ms`;
  $("dbSlowest").className = `value ${classForMs(dbMaxRows[0]?.value ?? 0, 25, 100)}`;
  $("dbSlowestFamily").textContent = dbMaxRows[0]?.labels.family ?? "no samples";
  $("unknownTotal").textContent = intFmt(unknownTotal);
  $("unknownSub").textContent = `families ${unknownRows.length}`;
}

function renderLoopChart(metrics) {
  const ticks = get(metrics, "wow_map_ticks_total");
  const durationLatest = get(metrics, "wow_map_tick_duration_latest_milliseconds");
  const lagLatest = get(metrics, "wow_map_tick_lag_latest_milliseconds");
  if (ticks !== state.lastTickCount) {
    const now = Date.now();
    state.history.push({ at: now, durationLatest, lagLatest });
    state.history = state.history.slice(-120);
    const recent = state.history.filter((point) => now - point.at <= 10000);
    const durationAvg10s = recent.length
      ? recent.reduce((sum, point) => sum + point.durationLatest, 0) / recent.length
      : durationLatest;
    state.avg10sHistory.push({ at: now, durationAvg10s });
    state.avg10sHistory = state.avg10sHistory.slice(-120);
    state.lastTickCount = ticks;
  }

  const canvas = $("loopChart");
  const ctx = canvas.getContext("2d");
  const width = canvas.width;
  const height = canvas.height;
  ctx.clearRect(0, 0, width, height);
  ctx.fillStyle = "#0c0e10";
  ctx.fillRect(0, 0, width, height);
  ctx.strokeStyle = "#303941";
  ctx.lineWidth = 1;
  for (let i = 1; i < 5; i++) {
    const y = (height / 5) * i;
    ctx.beginPath();
    ctx.moveTo(0, y);
    ctx.lineTo(width, y);
    ctx.stroke();
  }
  const max = Math.max(25, ...state.history.flatMap((point) => [point.durationLatest, point.lagLatest]));
  drawSeries(ctx, state.history.map((point) => point.durationLatest), max, width, height, "#48c78e");
  drawSeries(ctx, state.history.map((point) => point.lagLatest), max, width, height, "#64b5f6");
  ctx.fillStyle = "#97a3ad";
  ctx.font = "12px Segoe UI, sans-serif";
  ctx.fillText(`latest duration ${fmt(durationLatest)} ms`, 12, 20);
  ctx.fillText(`latest lag ${fmt(lagLatest)} ms`, 12, 38);
  ctx.fillText(`scale ${fmt(max)} ms`, width - 105, 20);
}

function renderLoopAvg10sChart(metrics) {
  const durationLatest = get(metrics, "wow_map_tick_duration_latest_milliseconds");
  const fallbackAvg = get(metrics, "wow_map_tick_duration_average_milliseconds") || durationLatest;
  const values = state.avg10sHistory.map((point) => point.durationAvg10s);
  const latest = values.length ? values[values.length - 1] : fallbackAvg;
  const scale = Math.max(1, ...values) * 1.15;

  const canvas = $("loopAvg10sChart");
  const ctx = canvas.getContext("2d");
  const width = canvas.width;
  const height = canvas.height;
  ctx.clearRect(0, 0, width, height);
  ctx.fillStyle = "#0c0e10";
  ctx.fillRect(0, 0, width, height);
  ctx.strokeStyle = "#303941";
  ctx.lineWidth = 1;
  for (let i = 1; i < 5; i++) {
    const y = (height / 5) * i;
    ctx.beginPath();
    ctx.moveTo(0, y);
    ctx.lineTo(width, y);
    ctx.stroke();
  }
  drawSeries(ctx, values, scale, width, height, "#f7c948");
  ctx.fillStyle = "#97a3ad";
  ctx.font = "12px Segoe UI, sans-serif";
  ctx.fillText(`avg 10s ${fmt(latest)} ms`, 12, 20);
  ctx.fillText(`scale ${fmt(scale)} ms`, width - 105, 20);
}

function drawSeries(ctx, values, max, width, height, color) {
  if (values.length < 2) return;
  ctx.strokeStyle = color;
  ctx.lineWidth = 2;
  ctx.beginPath();
  values.forEach((value, index) => {
    const x = values.length === 1 ? 0 : (index / (values.length - 1)) * width;
    const y = height - Math.min(value / max, 1) * (height - 28) - 12;
    if (index === 0) ctx.moveTo(x, y);
    else ctx.lineTo(x, y);
  });
  ctx.stroke();
}

function renderPhases(metrics) {
  const averages = byLabel(
    series(metrics, "wow_map_phase_duration_average_1m_milliseconds").length
      ? series(metrics, "wow_map_phase_duration_average_1m_milliseconds")
      : series(metrics, "wow_map_phase_duration_average_milliseconds"),
    "phase"
  );
  const latest = byLabel(series(metrics, "wow_map_phase_duration_latest_milliseconds"), "phase");
  const maxes = byLabel(
    series(metrics, "wow_map_phase_duration_max_1m_milliseconds").length
      ? series(metrics, "wow_map_phase_duration_max_1m_milliseconds")
      : series(metrics, "wow_map_phase_duration_max_milliseconds"),
    "phase"
  );
  const phases = [...new Set([...averages.keys(), ...latest.keys(), ...maxes.keys()])].sort();
  const maxValue = Math.max(1, ...[...maxes.values()]);
  $("phaseTable").innerHTML = phases.length ? phases.map((phase) => {
    const avg = averages.get(phase) ?? 0;
    const now = latest.get(phase) ?? 0;
    const max = maxes.get(phase) ?? 0;
    const pct = Math.min((max / maxValue) * 100, 100);
    const barClass = max >= 100 ? "bad" : max >= 25 ? "warn" : "";
    return `<div class="phase-row"><div>${phase}</div><div class="bar"><div class="fill ${barClass}" style="width:${pct}%"></div></div><div>${fmt(avg)}</div><div>${fmt(now)}</div><div>${fmt(max)}</div></div>`;
  }).join("") : `<div class="empty">No phase samples yet.</div>`;
}

function renderMaps(metrics) {
  const players = series(metrics, "wow_map_active_players");
  if (!players.length) {
    $("mapTable").innerHTML = `<div class="empty">No loaded maps yet.</div>`;
    return;
  }
  const rows = players.map((row) => {
    const labels = { map_id: row.labels.map_id, instance_id: row.labels.instance_id };
    return {
      map: `${row.labels.map_id}/${row.labels.instance_id}`,
      players: row.value,
      creatures: getLabeled(metrics, "wow_map_active_creatures", labels),
      gameobjects: getLabeled(metrics, "wow_map_active_gameobjects", labels),
      grids: getLabeled(metrics, "wow_map_loaded_grids", labels),
      combats: getLabeled(metrics, "wow_map_active_creature_combats", labels)
    };
  }).sort((a, b) => b.players - a.players || Number(a.map.split("/")[0]) - Number(b.map.split("/")[0]));
  $("mapTable").innerHTML = table(["Map", "Players", "Creatures", "Gameobjects", "Grids", "Combats"], rows.map((row) => [
    row.map, intFmt(row.players), intFmt(row.creatures), intFmt(row.gameobjects), intFmt(row.grids), intFmt(row.combats)
  ]));
}

function renderDb(metrics) {
  const totals = byLabel(series(metrics, "wow_db_query_total"), "family");
  const avg = byLabel(
    series(metrics, "wow_db_query_duration_average_1m_milliseconds").length
      ? series(metrics, "wow_db_query_duration_average_1m_milliseconds")
      : series(metrics, "wow_db_query_duration_average_milliseconds"),
    "family"
  );
  const latest = byLabel(series(metrics, "wow_db_query_duration_latest_milliseconds"), "family");
  const maxes = byLabel(
    series(metrics, "wow_db_query_duration_max_1m_milliseconds").length
      ? series(metrics, "wow_db_query_duration_max_1m_milliseconds")
      : series(metrics, "wow_db_query_duration_max_milliseconds"),
    "family"
  );
  const families = [...new Set([...totals.keys(), ...maxes.keys()])];
  const rows = families.map((family) => ({
    family,
    count: totals.get(family) ?? 0,
    avg: avg.get(family) ?? 0,
    latest: latest.get(family) ?? 0,
    max: maxes.get(family) ?? 0
  })).sort((a, b) => b.max - a.max).slice(0, 12);
  $("dbTable").innerHTML = rows.length ? table(["Family", "Count", "Avg", "Latest", "Max"], rows.map((row) => [
    row.family, intFmt(row.count), fmt(row.avg), fmt(row.latest), fmt(row.max)
  ])) : `<div class="empty">No DB samples yet.</div>`;
}

function renderCache(metrics) {
  const loads = series(metrics, "wow_static_world_cache_load_spawns");
  if (!loads.length) {
    $("cacheTable").innerHTML = `<div class="empty">Cache has not loaded yet.</div>`;
    return;
  }
  const rows = loads.map((row) => {
    const labels = { kind: row.labels.kind };
    return [
      row.labels.kind,
      intFmt(row.value),
      intFmt(getLabeled(metrics, "wow_static_world_cache_load_grids", labels)),
      fmt(getLabeled(metrics, "wow_static_world_cache_load_duration_milliseconds", labels)),
      intFmt(getLabeled(metrics, "wow_static_world_cache_lookup_total", labels)),
      intFmt(getLabeled(metrics, "wow_static_world_cache_instantiation_rows_total", labels)),
      fmt(getLabeled(metrics, "wow_static_world_cache_instantiation_duration_max_milliseconds", labels))
    ];
  }).sort((a, b) => a[0].localeCompare(b[0]));
  $("cacheTable").innerHTML = table(["Kind", "Spawns", "Grids", "Load ms", "Lookups", "Rows", "Max ms"], rows);
}

function renderPackets(metrics) {
  const rows = series(metrics, "wow_world_packets_in_total")
    .sort((a, b) => b.value - a.value)
    .slice(0, 12)
    .map((row) => [row.labels.opcode, intFmt(row.value), deltaText(metrics, row)]);
  $("packetsIn").innerHTML = rows.length ? table(["Opcode", "Total", "Delta"], rows) : `<div class="empty">No packets yet.</div>`;

  const unknownRows = series(metrics, "wow_world_unknown_opcodes_total")
    .sort((a, b) => b.value - a.value)
    .slice(0, 12)
    .map((row) => [row.labels.opcode, intFmt(row.value), deltaText(metrics, row)]);
  $("unknownTable").innerHTML = unknownRows.length ? table(["Opcode", "Total", "Delta"], unknownRows) : `<div class="empty">No unknown opcodes.</div>`;
}

function renderPacketLatency(metrics) {
  const rows = watchedPacketOpcodes.map(({ opcode, label }) => {
    const dispatchAvg = getLabeled(metrics, "wow_world_packet_dispatch_delay_average_1m_milliseconds", { opcode });
    const dispatchMax = getLabeled(metrics, "wow_world_packet_dispatch_delay_max_1m_milliseconds", { opcode });
    const handlerAvg = getLabeled(metrics, "wow_world_packet_handler_duration_average_1m_milliseconds", { opcode });
    const handlerMax = getLabeled(metrics, "wow_world_packet_handler_duration_max_1m_milliseconds", { opcode });
    const serviceAvg = getLabeled(metrics, "wow_world_packet_service_time_average_1m_milliseconds", { opcode });
    const serviceLatest = getLabeled(metrics, "wow_world_packet_service_time_latest_milliseconds", { opcode });
    const serviceMax = getLabeled(metrics, "wow_world_packet_service_time_max_1m_milliseconds", { opcode });
    const outboundAvg = getLabeled(metrics, "wow_world_packet_outbound_queue_latency_average_1m_milliseconds", { opcode });
    const writeAvg = getLabeled(metrics, "wow_world_packet_write_duration_average_1m_milliseconds", { opcode });
    return [
      opcode,
      label,
      fmt(dispatchAvg),
      fmt(dispatchMax),
      fmt(handlerAvg),
      fmt(handlerMax),
      fmt(serviceAvg),
      fmt(serviceLatest),
      fmt(serviceMax),
      fmt(outboundAvg),
      fmt(writeAvg)
    ];
  }).filter((row) =>
    Number(row[2]) > 0 || Number(row[3]) > 0 || Number(row[4]) > 0 ||
    Number(row[5]) > 0 || Number(row[6]) > 0 || Number(row[7]) > 0 || Number(row[8]) > 0
  );

  $("packetLatencyTable").innerHTML = rows.length
    ? table(
        ["Opcode", "Name", "Dispatch 1m", "Dispatch Max 1m", "Handler 1m", "Handler Max 1m", "Service 1m", "Service Latest", "Service Max 1m", "OutQ 1m", "Write 1m"],
        rows
      )
    : `<div class="empty">No watched packet latency samples yet.</div>`;
}

function renderSessionLoop(metrics) {
  const averages = byLabel(
    series(metrics, "wow_world_session_loop_phase_duration_average_1m_milliseconds").length
      ? series(metrics, "wow_world_session_loop_phase_duration_average_1m_milliseconds")
      : series(metrics, "wow_world_session_loop_phase_duration_average_milliseconds"),
    "phase"
  );
  const latest = byLabel(series(metrics, "wow_world_session_loop_phase_duration_latest_milliseconds"), "phase");
  const maxes = byLabel(
    series(metrics, "wow_world_session_loop_phase_duration_max_1m_milliseconds").length
      ? series(metrics, "wow_world_session_loop_phase_duration_max_1m_milliseconds")
      : series(metrics, "wow_world_session_loop_phase_duration_max_milliseconds"),
    "phase"
  );
  const phases = [...new Set([...averages.keys(), ...latest.keys(), ...maxes.keys()])].sort();
  $("sessionLoopTable").innerHTML = phases.length
    ? table(
        ["Phase", "Avg 1m", "Latest", "Max 1m"],
        phases.map((phase) => [phase, fmt(averages.get(phase) ?? 0), fmt(latest.get(phase) ?? 0), fmt(maxes.get(phase) ?? 0)])
      )
    : `<div class="empty">No session loop phase samples yet.</div>`;
}

function deltaText(metrics, row) {
  const key = metricKey(row.name, row.labels);
  const previous = state.previous.get(key) ?? row.value;
  const delta = row.value - previous;
  return delta > 0 ? `+${intFmt(delta)}` : "0";
}

function byLabel(rows, label) {
  const map = new Map();
  for (const row of rows) map.set(row.labels[label] ?? "", row.value);
  return map;
}

function table(headers, rows) {
  return `<table><thead><tr>${headers.map((h) => `<th>${h}</th>`).join("")}</tr></thead><tbody>${rows.map((row) => `<tr>${row.map((cell) => `<td>${cell}</td>`).join("")}</tr>`).join("")}</tbody></table>`;
}

async function refresh() {
  if (state.paused) return;
  try {
    const response = await fetch("/metrics", { cache: "no-store" });
    const text = await response.text();
    const metrics = parseMetrics(text);
    renderKpis(metrics);
    renderLoopChart(metrics);
    renderLoopAvg10sChart(metrics);
    renderPhases(metrics);
    renderMaps(metrics);
    renderCache(metrics);
    renderDb(metrics);
    renderPackets(metrics);
    renderPacketLatency(metrics);
    renderSessionLoop(metrics);
    state.previous = metrics;
    $("statusDot").className = "status-dot good";
    $("statusText").textContent = "live";
    $("lastUpdated").textContent = new Date().toLocaleTimeString();
  } catch (error) {
    $("statusDot").className = "status-dot bad";
    $("statusText").textContent = "offline";
  }
}

$("pauseButton").addEventListener("click", () => {
  state.paused = !state.paused;
  $("pauseButton").textContent = state.paused ? "Resume" : "Pause";
  $("statusText").textContent = state.paused ? "paused" : "live";
});
$("resetButton").addEventListener("click", async () => {
  state.history = [];
  state.avg10sHistory = [];
  state.previous = new Map();
  try {
    await fetch("/dashboard/mark", { method: "POST", cache: "no-store" });
    await refresh();
  } catch (error) {
    $("statusDot").className = "status-dot bad";
    $("statusText").textContent = "mark failed";
  }
});

refresh();
setInterval(refresh, 1000);
</script>
</body>
</html>"##;

pub async fn run_metrics_endpoint(bind_addr: SocketAddr) -> anyhow::Result<()> {
    let listener = TcpListener::bind(bind_addr).await?;
    info!(%bind_addr, "Observability metrics endpoint listening");

    loop {
        let (mut socket, peer) = listener.accept().await?;
        tokio::spawn(async move {
            let mut request = [0u8; 1024];
            let read = match socket.read(&mut request).await {
                Ok(read) => read,
                Err(error) => {
                    warn!(%peer, "Failed to read metrics request: {error}");
                    return;
                }
            };
            let request = String::from_utf8_lossy(&request[..read]);
            let first_line = request.lines().next().unwrap_or_default();
            let (status, content_type, body) = if first_line.starts_with("GET /metrics ") {
                (
                    "200 OK",
                    "text/plain; version=0.0.4; charset=utf-8",
                    render_prometheus(),
                )
            } else if first_line.starts_with("GET /playerbots")
                || first_line.starts_with("GET /playerbot")
            {
                (
                    "200 OK",
                    "text/plain; charset=utf-8",
                    render_playerbot_diagnostics_from_request(first_line),
                )
            } else if first_line.starts_with("GET /dashboard ") || first_line.starts_with("GET / ")
            {
                (
                    "200 OK",
                    "text/html; charset=utf-8",
                    render_dashboard_html().to_string(),
                )
            } else if first_line.starts_with("POST /dashboard/mark ") {
                let started_at = mark_monitoring_session();
                (
                    "200 OK",
                    "text/plain; charset=utf-8",
                    format!("marked {started_at}\n"),
                )
            } else if first_line.starts_with("GET /healthz ") {
                ("200 OK", "text/plain; charset=utf-8", "ok\n".to_string())
            } else {
                (
                    "404 Not Found",
                    "text/plain; charset=utf-8",
                    "not found\n".to_string(),
                )
            };
            let response = format!(
                "HTTP/1.1 {status}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
            if let Err(error) = socket.write_all(response.as_bytes()).await {
                warn!(%peer, "Failed to write metrics response: {error}");
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prometheus_render_includes_histogram_and_opcode_labels() {
        record_map_tick(
            Duration::from_millis(12),
            Duration::from_millis(3),
            Duration::from_millis(100),
        );
        record_map_phase_duration(MapTickPhase::PlayerRegen, Duration::from_millis(7));
        record_map_runtime_snapshots([MapRuntimeSnapshot {
            map_id: 0,
            instance_id: 0,
            active_players: 1,
            active_playerbots: 1,
            tracked_idle_motion_creatures: 9,
            tracked_idle_motion_start_candidates: 11,
            active_creatures: 42,
            active_gameobjects: 3,
            loaded_grids: 4,
            loaded_creature_grids: 5,
            loaded_gameobject_grids: 6,
            loaded_player_corpse_grids: 7,
            active_creature_combats: 2,
            corpses: 1,
        }]);
        record_static_world_cache_load(
            StaticWorldCacheKind::Creature,
            10,
            2,
            Duration::from_millis(15),
        );
        record_static_world_cache_lookup(StaticWorldCacheKind::Creature, Duration::from_millis(1));
        record_static_world_cache_instantiation(
            StaticWorldCacheKind::Creature,
            3,
            Duration::from_millis(2),
        );
        record_world_packet_in(0x01E0);
        record_world_packet_out(0x00DD);
        record_world_unknown_opcode(0x01E0);
        record_world_packet_handler_duration(0x012E, Duration::from_millis(33));
        record_movement_actor_queue_depth(3);
        record_movement_actor_enqueue_failure("full");
        record_movement_actor_enqueue_latency(Duration::from_millis(1));
        record_movement_actor_processing_time(Duration::from_millis(2));
        record_movement_actor_reply_latency(Duration::from_millis(4));
        record_movement_actor_batch_size(5);
        record_movement_map_mutex_wait(Duration::from_millis(6));
        record_movement_map_mutex_hold(Duration::from_millis(8));
        record_native_mmap_query(
            NativeMmapQueryKind::Path,
            NativeMmapQueryTimings {
                lock_and_tile_load: Duration::from_millis(22),
                query_alloc_init: Duration::from_millis(23),
                find_nearest_poly: Duration::from_millis(24),
                find_path: Duration::from_millis(25),
                find_smooth_path: Duration::from_millis(26),
            },
        );
        record_native_mmap_query(
            NativeMmapQueryKind::RandomPath,
            NativeMmapQueryTimings {
                lock_and_tile_load: Duration::from_millis(27),
                query_alloc_init: Duration::from_millis(28),
                find_nearest_poly: Duration::from_millis(29),
                find_path: Duration::from_millis(30),
                find_smooth_path: Duration::from_millis(31),
            },
        );
        record_player_environment_geometry_check();
        record_player_environment_cached_skip();
        record_player_environment_login_refresh();
        record_player_environment_movement_refresh();
        record_player_environment_teleport_refresh();
        record_player_environment_periodic_revalidation();
        record_player_environment_timer_only_processing();
        record_idle_motion_due_creatures(13);
        record_idle_motion_started_creatures(4);
        record_idle_motion_packets_emitted(17);
        record_idle_motion_advancement_queue_pop_time(Duration::from_millis(13));
        record_idle_motion_advancement_validation_time(Duration::from_millis(14));
        record_idle_motion_motion_advance_time(Duration::from_millis(15));
        record_idle_motion_spatial_update_time(Duration::from_millis(16));
        record_idle_motion_start_queue_pop_time(Duration::from_millis(17));
        record_idle_motion_motion_start_time(Duration::from_millis(18));
        record_idle_motion_motion_start_path_build_time(Duration::from_millis(19));
        record_idle_motion_motion_start_snapshot_clone_time(Duration::from_millis(20));
        record_idle_motion_motion_start_post_start_tracking_time(Duration::from_millis(21));
        record_idle_motion_motion_start_broadcast_time(Duration::from_millis(22));
        record_idle_motion_motion_script_schedule_time(Duration::from_millis(23));
        record_idle_motion_pending_script_execution_time(Duration::from_millis(24));
        record_idle_motion_start_schedule_rebuild_time(Duration::from_millis(25));
        record_player_environment_players_scanned(48);
        record_player_environment_packets_emitted(96);
        record_player_environment_flags_time(Duration::from_millis(9));
        record_player_environment_timer_update_time(Duration::from_millis(10));
        record_player_environment_damage_application_time(Duration::from_millis(11));
        record_player_environment_nearby_fanout_time(Duration::from_millis(12));

        let rendered = render_prometheus();

        assert!(rendered.contains("# TYPE wow_map_tick_duration_seconds histogram"));
        assert!(rendered.contains("wow_map_tick_duration_seconds_bucket{le=\"0.025\"}"));
        assert!(rendered.contains("wow_map_tick_duration_seconds_count "));
        assert!(rendered.contains("wow_map_tick_duration_average_milliseconds 12.000"));
        assert!(rendered.contains("wow_map_tick_duration_latest_milliseconds 12.000"));
        assert!(rendered.contains("wow_map_tick_duration_max_milliseconds 12.000"));
        assert!(rendered.contains("wow_map_tick_duration_average_1m_milliseconds 12.000"));
        assert!(rendered.contains("wow_map_tick_duration_max_1m_milliseconds 12.000"));
        assert!(rendered.contains("wow_map_tick_duration_average_5m_milliseconds 12.000"));
        assert!(rendered.contains("wow_map_tick_duration_max_5m_milliseconds 12.000"));
        assert!(rendered.contains("wow_map_tick_lag_average_milliseconds 3.000"));
        assert!(rendered.contains("wow_map_tick_lag_latest_milliseconds 3.000"));
        assert!(rendered.contains("wow_map_tick_lag_max_milliseconds 3.000"));
        assert!(rendered.contains("wow_map_tick_lag_average_1m_milliseconds 3.000"));
        assert!(rendered.contains("wow_map_tick_lag_max_1m_milliseconds 3.000"));
        assert!(rendered
            .contains("wow_map_phase_duration_max_milliseconds{phase=\"player_regen\"} 7.000"));
        assert!(rendered.contains(
            "wow_map_phase_duration_average_1m_milliseconds{phase=\"player_regen\"} 7.000"
        ));
        assert!(rendered
            .contains("wow_map_phase_duration_max_1m_milliseconds{phase=\"player_regen\"} 7.000"));
        assert!(rendered
            .contains("wow_map_phase_duration_latest_milliseconds{phase=\"idle_motion\"} 0.000"));
        assert!(rendered.contains("wow_monitoring_session_started_unix_seconds "));
        assert!(rendered.contains("wow_monitoring_session_marks_total "));
        assert!(rendered.contains("wow_map_active_players{map_id=\"0\",instance_id=\"0\"} 1"));
        assert!(rendered.contains("wow_map_active_playerbots{map_id=\"0\",instance_id=\"0\"} 1"));
        assert!(rendered
            .contains("wow_map_tracked_idle_motion_creatures{map_id=\"0\",instance_id=\"0\"} 9"));
        assert!(rendered.contains(
            "wow_map_tracked_idle_motion_start_candidates{map_id=\"0\",instance_id=\"0\"} 11"
        ));
        assert!(rendered.contains("wow_map_active_creatures{map_id=\"0\",instance_id=\"0\"} 42"));
        assert!(
            rendered.contains("wow_map_active_creature_combats{map_id=\"0\",instance_id=\"0\"} 2")
        );
        assert!(rendered.contains("wow_static_world_cache_load_spawns{kind=\"creature\"} 10.000"));
        assert!(rendered.contains(
            "wow_static_world_cache_load_duration_milliseconds{kind=\"creature\"} 15.000"
        ));
        assert!(rendered.contains("wow_static_world_cache_lookup_total{kind=\"creature\"} 1"));
        assert!(rendered
            .contains("wow_static_world_cache_instantiation_rows_total{kind=\"creature\"} 3"));
        assert!(rendered.contains("wow_world_packets_in_total{opcode=\"0x01E0\"}"));
        assert!(rendered.contains("wow_world_packets_out_total{opcode=\"0x00DD\"}"));
        assert!(rendered.contains("wow_world_unknown_opcodes_total{opcode=\"0x01E0\"}"));
        assert!(rendered.contains(
            "wow_world_packet_handler_duration_average_milliseconds{opcode=\"0x012E\"} 33.000"
        ));
        assert!(rendered.contains("wow_movement_actor_queue_depth_latest "));
        assert!(rendered.contains("wow_movement_actor_enqueue_failures_total{reason=\"full\"} 1"));
        assert!(rendered.contains("wow_movement_actor_enqueue_latency_average_milliseconds "));
        assert!(rendered.contains("wow_movement_actor_processing_time_average_milliseconds "));
        assert!(rendered.contains("wow_movement_actor_reply_latency_average_milliseconds "));
        assert!(rendered.contains("wow_movement_actor_batch_size_latest "));
        assert!(rendered.contains("wow_movement_map_mutex_wait_average_milliseconds "));
        assert!(rendered.contains("wow_movement_map_mutex_hold_average_milliseconds "));
        assert!(rendered.contains("wow_native_mmap_path_calls_total 1"));
        assert!(rendered.contains("wow_native_mmap_random_path_calls_total 1"));
        assert!(rendered
            .contains("wow_native_mmap_path_query_alloc_init_time_average_milliseconds 23.000"));
        assert!(rendered.contains(
            "wow_native_mmap_random_path_find_smooth_path_time_average_milliseconds 31.000"
        ));
        assert!(rendered.contains("wow_player_environment_geometry_checks_total 1"));
        assert!(rendered.contains("wow_player_environment_cached_skips_total 1"));
        assert!(rendered.contains("wow_player_environment_login_refreshes_total 1"));
        assert!(rendered.contains("wow_player_environment_movement_refreshes_total 1"));
        assert!(rendered.contains("wow_player_environment_teleport_refreshes_total 1"));
        assert!(rendered.contains("wow_player_environment_periodic_revalidations_total 1"));
        assert!(rendered.contains("wow_player_environment_timer_only_processing_total 1"));
        assert!(rendered.contains("wow_idle_motion_due_creatures_latest 13"));
        assert!(rendered.contains("wow_idle_motion_started_creatures_latest 4"));
        assert!(rendered.contains("wow_idle_motion_packets_emitted_latest 17"));
        assert!(rendered
            .contains("wow_idle_motion_advancement_queue_pop_time_average_milliseconds 13.000"));
        assert!(rendered
            .contains("wow_idle_motion_advancement_validation_time_average_milliseconds 14.000"));
        assert!(
            rendered.contains("wow_idle_motion_motion_advance_time_average_milliseconds 15.000")
        );
        assert!(
            rendered.contains("wow_idle_motion_spatial_update_time_average_milliseconds 16.000")
        );
        assert!(
            rendered.contains("wow_idle_motion_start_queue_pop_time_average_milliseconds 17.000")
        );
        assert!(rendered.contains("wow_idle_motion_motion_start_time_average_milliseconds 18.000"));
        assert!(rendered
            .contains("wow_idle_motion_motion_start_path_build_time_average_milliseconds 19.000"));
        assert!(rendered.contains(
            "wow_idle_motion_motion_start_snapshot_clone_time_average_milliseconds 20.000"
        ));
        assert!(rendered.contains(
            "wow_idle_motion_motion_start_post_start_tracking_time_average_milliseconds 21.000"
        ));
        assert!(rendered
            .contains("wow_idle_motion_motion_start_broadcast_time_average_milliseconds 22.000"));
        assert!(rendered
            .contains("wow_idle_motion_motion_script_schedule_time_average_milliseconds 23.000"));
        assert!(rendered
            .contains("wow_idle_motion_pending_script_execution_time_average_milliseconds 24.000"));
        assert!(rendered
            .contains("wow_idle_motion_start_schedule_rebuild_time_average_milliseconds 25.000"));
        assert!(rendered.contains("wow_player_environment_players_scanned_latest 48"));
        assert!(rendered.contains("wow_player_environment_packets_emitted_latest 96"));
        assert!(rendered.contains("wow_player_environment_flags_time_average_milliseconds 9.000"));
        assert!(rendered
            .contains("wow_player_environment_timer_update_time_average_milliseconds 10.000"));
        assert!(rendered.contains(
            "wow_player_environment_damage_application_time_average_milliseconds 11.000"
        ));
        assert!(rendered
            .contains("wow_player_environment_nearby_fanout_time_average_milliseconds 12.000"));
        assert!(rendered.contains(
            "wow_map_phase_duration_average_milliseconds{phase=\"dynamic_objects\"} 0.000"
        ));
        assert!(rendered.contains(
            "wow_map_phase_duration_average_milliseconds{phase=\"player_death_presentation\"} 0.000"
        ));
        assert!(rendered.contains(
            "wow_world_session_loop_phase_duration_average_milliseconds{phase=\"packet_branch_total\"} 0.000"
        ));
    }

    #[test]
    fn monitoring_session_marker_updates_prometheus_value() {
        let started_at = mark_monitoring_session();
        let rendered = render_prometheus();

        assert!(started_at > 0);
        assert!(rendered.contains(&format!(
            "wow_monitoring_session_started_unix_seconds {started_at}"
        )));
    }

    #[test]
    fn dashboard_renders_live_metrics_page() {
        let rendered = render_dashboard_html();

        assert!(rendered.contains("<title>Worldserver Monitor</title>"));
        assert!(rendered.contains("fetch(\"/metrics\""));
        assert!(rendered.contains("fetch(\"/dashboard/mark\""));
        assert!(rendered.contains("Loop Avg 10s"));
        assert!(rendered.contains("loopAvg10sChart"));
        assert!(rendered.contains("Packet Latency"));
        assert!(rendered.contains("Session Loop"));
        assert!(rendered.contains("sessionLoopTable"));
        assert!(rendered.contains("wow_map_tick_duration_average_milliseconds"));
        assert!(rendered.contains("wow_map_tick_duration_average_1m_milliseconds"));
        assert!(rendered.contains("wow_static_world_cache_load_spawns"));
        assert!(rendered.contains("wow_db_query_duration_max_milliseconds"));
    }

    #[test]
    fn playerbot_diagnostics_can_filter_by_name() {
        record_playerbot_name(42_424_242, "Diagbot");
        record_playerbot_debug_snapshots([PlayerbotDebugSnapshot {
            guid: 42_424_242,
            map_id: 0,
            instance_id: 0,
            x: -1.0,
            y: 2.0,
            z: 3.0,
            travel_x: Some(4.0),
            travel_y: Some(5.0),
            travel_z: Some(6.0),
            distance_to_travel: Some(7.0),
            active_leg_destination_x: None,
            active_leg_destination_y: None,
            active_leg_destination_z: None,
            active_leg_remaining_millis: None,
            route_len: 0,
            next_think_in_millis: 5000,
            movement_flags: 0,
            state: "waiting_retry_or_budget",
        }]);

        let rendered =
            render_playerbot_diagnostics_from_request("GET /playerbots?names=Diagbot HTTP/1.1");

        assert!(rendered.contains("Diagbot (42424242)"));
        assert!(rendered.contains("state=waiting_retry_or_budget"));
        assert!(rendered.contains("target_dist=7.0yd"));
    }
}
