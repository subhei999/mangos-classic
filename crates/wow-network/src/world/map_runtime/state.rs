use std::cmp::Reverse;
use std::collections::{BTreeSet, BinaryHeap};

use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[allow(dead_code)]
pub(in crate::world) struct BotId(pub(in crate::world) u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub(in crate::world) enum PlayerController {
    Client { session_id: SessionId },
    Disconnected { remove_at: Instant },
    Bot { bot_id: BotId },
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub(in crate::world) struct PlayerbotRuntimeState {
    pub(in crate::world) bot_id: BotId,
    pub(in crate::world) home_position: WorldPosition,
    pub(in crate::world) next_think_at: Instant,
    pub(in crate::world) next_combat_think_at: Instant,
    pub(in crate::world) active_leg: Option<PlayerbotMovementLeg>,
    pub(in crate::world) route: Vec<WorldPosition>,
    pub(in crate::world) combat_enabled: bool,
    pub(in crate::world) local_roam_only: bool,
    pub(in crate::world) force_active: bool,
    pub(in crate::world) travel_destination: Option<WorldPosition>,
    pub(in crate::world) engage_target: Option<ObjectGuid>,
    pub(in crate::world) movement_start_retries_remaining: u8,
    pub(in crate::world) roam_step: u8,
}

pub(in crate::world) fn playerbot_runtime_requires_async_planner(
    bot: &PlayerbotRuntimeState,
) -> bool {
    bot.combat_enabled || bot.travel_destination.is_some() || !bot.local_roam_only
}

#[derive(Debug, Clone, Copy)]
#[allow(dead_code)]
pub(in crate::world) struct PlayerbotMovementLeg {
    pub(in crate::world) start_position: WorldPosition,
    pub(in crate::world) destination: WorldPosition,
    pub(in crate::world) start_time: Instant,
    pub(in crate::world) arrival_time: Instant,
    pub(in crate::world) speed_yards_per_second: f32,
}

#[derive(Debug, Clone, Default)]
#[allow(dead_code)]
pub(in crate::world) struct PlayerbotQueuedIntents {
    pub(in crate::world) movement: Option<PlayerbotMovementIntent>,
    pub(in crate::world) combat: Option<PlayerbotCombatIntent>,
}

impl PlayerbotQueuedIntents {
    pub(in crate::world) fn is_empty(&self) -> bool {
        self.movement.is_none() && self.combat.is_none()
    }
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub(in crate::world) enum PlayerbotMovementIntent {
    Defer,
    Route { route: Option<Vec<WorldPosition>> },
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub(in crate::world) enum PlayerbotCombatIntent {
    Target {
        target: ObjectGuid,
        route: Option<Vec<WorldPosition>>,
    },
    NoTarget,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::world) enum PlayerAutoAttackKind {
    Melee,
    Ranged {
        spell_id: u32,
        phase: PlayerRangedAutoAttackPhase,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::world) enum PlayerRangedAutoAttackPhase {
    Windup,
    Shooting,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::world) struct PlayerAutoAttackDue {
    pub(in crate::world) target: ObjectGuid,
    pub(in crate::world) kind: PlayerAutoAttackKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::world) enum PlayerAutoAttackAfterSpellCast {
    None,
    MeleeRetimed {
        target: ObjectGuid,
        next_swing_at: Instant,
    },
    RangedRetimed {
        target: ObjectGuid,
        spell_id: u32,
        next_shot_at: Instant,
    },
    RangedCanceled {
        target: ObjectGuid,
        spell_id: u32,
    },
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub(in crate::world) struct PlayerbotPlanInput {
    pub(in crate::world) map_id: u32,
    pub(in crate::world) instance_id: u32,
    pub(in crate::world) bot_guid: u32,
    pub(in crate::world) position: WorldPosition,
    pub(in crate::world) home_position: WorldPosition,
    pub(in crate::world) travel_destination: Option<WorldPosition>,
    pub(in crate::world) roam_step: u8,
    pub(in crate::world) player_race: u8,
    pub(in crate::world) movement_due_at: Option<Instant>,
    pub(in crate::world) combat_due_at: Option<Instant>,
    pub(in crate::world) engage_target: Option<ObjectGuid>,
    pub(in crate::world) engage_target_creature: Option<DbCreatureRuntime>,
    pub(in crate::world) nearby_creatures: Vec<DbCreatureRuntime>,
    pub(in crate::world) geometry: Arc<WorldGeometry>,
}

#[derive(Debug, Default)]
#[allow(dead_code)]
pub(in crate::world) struct PlayerbotPlanningTick {
    pub(in crate::world) planned_bots: u32,
    pub(in crate::world) route_budget_exhausted: bool,
    pub(in crate::world) combat_budget_exhausted: bool,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub(in crate::world) struct PlayerRuntime {
    pub(in crate::world) guid: u32,
    pub(in crate::world) account_id: Option<u32>,
    pub(in crate::world) controller: PlayerController,
    pub(in crate::world) bot_runtime: Option<PlayerbotRuntimeState>,
    pub(in crate::world) selected_target: Option<ObjectGuid>,
    pub(in crate::world) unit_target: Option<ObjectGuid>,
    pub(in crate::world) active_combat_target: Option<ObjectGuid>,
    pub(in crate::world) active_combat_attack_kind: PlayerAutoAttackKind,
    pub(in crate::world) active_combat_next_swing_at: Option<Instant>,
    pub(in crate::world) ranged_auto_attack_next_shot_at: Option<Instant>,
    pub(in crate::world) in_combat: bool,
    pub(in crate::world) looting: bool,
    pub(in crate::world) position: WorldPosition,
    pub(in crate::world) movement_flags: u32,
    pub(in crate::world) client_time: u32,
    pub(in crate::world) server_time: u32,
    pub(in crate::world) fall_time: u32,
    pub(in crate::world) last_fall_z: Option<f32>,
    pub(in crate::world) last_fall_time: u32,
    pub(in crate::world) environment: PlayerEnvironmentRuntime,
    pub(in crate::world) jump: JumpInfo,
    pub(in crate::world) cell: CellCoord,
    pub(in crate::world) visible_objects: HashSet<ObjectGuid>,
    pub(in crate::world) next_sight_aggro_check_at: Option<Instant>,
    pub(in crate::world) last_sight_aggro_check_position: Option<WorldPosition>,
    pub(in crate::world) last_player_visibility_refresh_position: Option<WorldPosition>,
    pub(in crate::world) last_creature_visibility_position: Option<WorldPosition>,
    pub(in crate::world) last_gameobject_visibility_position: Option<WorldPosition>,
    pub(in crate::world) last_player_corpse_visibility_position: Option<WorldPosition>,
    pub(in crate::world) visual: PlayerVisualState,
    pub(in crate::world) visible_equipment: [u32; ENUM_EQUIPMENT_SLOTS],
    pub(in crate::world) flags: u32,
    pub(in crate::world) death_state: PlayerDeathState,
    pub(in crate::world) level: u8,
    pub(in crate::world) race: u8,
    pub(in crate::world) class: u8,
    pub(in crate::world) spirit: u32,
    pub(in crate::world) gender: u8,
    pub(in crate::world) base_world_stats: PlayerWorldStats,
    pub(in crate::world) effective_world_stats: PlayerWorldStats,
    pub(in crate::world) health: u32,
    pub(in crate::world) max_health: u32,
    pub(in crate::world) xp: u32,
    pub(in crate::world) power1: u32,
    pub(in crate::world) max_power1: u32,
    pub(in crate::world) last_mana_use_at: Option<Instant>,
    pub(in crate::world) power2: u32,
    pub(in crate::world) power4: u32,
    pub(in crate::world) max_power4: u32,
    pub(in crate::world) player_bytes: u32,
    pub(in crate::world) player_bytes2: u32,
    pub(in crate::world) combo_target: Option<ObjectGuid>,
    pub(in crate::world) combo_points: u8,
    pub(in crate::world) stand_state: u8,
    pub(in crate::world) active_spells: HashSet<u32>,
    pub(in crate::world) inventory: Vec<CharacterInventoryItem>,
    pub(in crate::world) quest_statuses: HashMap<u32, CharacterQuestStatus>,
    pub(in crate::world) explored_zones: [u32; PLAYER_EXPLORED_ZONES_SIZE],
    pub(in crate::world) active_auras: Vec<ActiveAura>,
    pub(in crate::world) spell_global_cooldowns_until: HashMap<u32, Instant>,
    pub(in crate::world) spell_cooldowns_until: HashMap<u32, Instant>,
    pub(in crate::world) spell_cooldown_categories: HashMap<u32, u32>,
    pub(in crate::world) spell_cooldown_item_ids: HashMap<u32, u32>,
    pub(in crate::world) queued_next_melee_spell: Option<QueuedNextMeleeSpell>,
    pub(in crate::world) base_combat_stats: PlayerCombatStats,
    pub(in crate::world) combat_stats: PlayerCombatStats,
}

#[derive(Debug, Clone)]
pub(in crate::world) struct ExpiredDisconnectedPlayer {
    pub(in crate::world) player: PlayerRuntime,
    pub(in crate::world) observer_packets: Vec<(SessionId, OutboundWorldPacket)>,
}

impl PlayerRuntime {
    pub(in crate::world) fn is_client_controlled(&self) -> bool {
        matches!(self.controller, PlayerController::Client { .. })
    }

    pub(in crate::world) fn disconnected_remove_at(&self) -> Option<Instant> {
        match self.controller {
            PlayerController::Disconnected { remove_at } => Some(remove_at),
            _ => None,
        }
    }

    pub(in crate::world) fn client_session_id(&self) -> Option<SessionId> {
        match self.controller {
            PlayerController::Client { session_id } => Some(session_id),
            PlayerController::Disconnected { .. } => None,
            PlayerController::Bot { .. } => None,
        }
    }

    pub(in crate::world) fn packet_to_client(
        &self,
        packet: OutboundWorldPacket,
    ) -> Option<(SessionId, OutboundWorldPacket)> {
        self.client_session_id()
            .map(|session_id| (session_id, packet))
    }
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub(in crate::world) struct PlayerRuntimeSnapshot {
    pub(in crate::world) position: WorldPosition,
    pub(in crate::world) movement_flags: u32,
    pub(in crate::world) client_time: u32,
    pub(in crate::world) fall_time: u32,
    pub(in crate::world) jump: JumpInfo,
    pub(in crate::world) flags: u32,
    pub(in crate::world) death_state: PlayerDeathState,
    pub(in crate::world) stand_state: u8,
    pub(in crate::world) level: u8,
    pub(in crate::world) race: u8,
    pub(in crate::world) class: u8,
    pub(in crate::world) xp: u32,
    pub(in crate::world) health: u32,
    pub(in crate::world) max_health: u32,
    pub(in crate::world) power1: u32,
    pub(in crate::world) max_power1: u32,
    pub(in crate::world) last_mana_use_at: Option<Instant>,
    pub(in crate::world) power2: u32,
    pub(in crate::world) power4: u32,
    pub(in crate::world) max_power4: u32,
    pub(in crate::world) combo_target: Option<ObjectGuid>,
    pub(in crate::world) combo_points: u8,
    pub(in crate::world) active_spells: HashSet<u32>,
    pub(in crate::world) inventory: Vec<CharacterInventoryItem>,
    pub(in crate::world) quest_statuses: HashMap<u32, CharacterQuestStatus>,
    pub(in crate::world) active_auras: Vec<ActiveAura>,
    pub(in crate::world) spell_global_cooldowns_until: HashMap<u32, Instant>,
    pub(in crate::world) spell_cooldowns_until: HashMap<u32, Instant>,
    pub(in crate::world) spell_cooldown_categories: HashMap<u32, u32>,
    pub(in crate::world) spell_cooldown_item_ids: HashMap<u32, u32>,
    pub(in crate::world) queued_next_melee_spell: Option<QueuedNextMeleeSpell>,
    pub(in crate::world) base_combat_stats: PlayerCombatStats,
    pub(in crate::world) combat_stats: PlayerCombatStats,
    pub(in crate::world) in_combat: bool,
    pub(in crate::world) active_combat_target: Option<ObjectGuid>,
    pub(in crate::world) active_combat_attack_kind: PlayerAutoAttackKind,
    pub(in crate::world) active_combat_next_swing_at: Option<Instant>,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub(in crate::world) struct PlayerRuntimeSessionSnapshot {
    pub(in crate::world) position: WorldPosition,
    pub(in crate::world) movement_flags: u32,
    pub(in crate::world) client_time: u32,
    pub(in crate::world) fall_time: u32,
    pub(in crate::world) jump: JumpInfo,
    pub(in crate::world) flags: u32,
    pub(in crate::world) death_state: PlayerDeathState,
    pub(in crate::world) stand_state: u8,
    pub(in crate::world) level: u8,
    pub(in crate::world) xp: u32,
    pub(in crate::world) health: u32,
    pub(in crate::world) max_health: u32,
    pub(in crate::world) power1: u32,
    pub(in crate::world) power2: u32,
    pub(in crate::world) power4: u32,
    pub(in crate::world) in_combat: bool,
}

#[derive(Debug, Clone, Copy)]
pub(in crate::world) struct PlayerDeathPresentationRuntime {
    pub(in crate::world) waiting_since: Instant,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::world) struct TrackedSingleTargetAuraRuntime {
    pub(in crate::world) target: ObjectGuid,
    pub(in crate::world) descriptor: SingleTargetAuraDescriptor,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(in crate::world) struct DiminishingStateRuntime {
    pub(in crate::world) active_stack_count: u16,
    pub(in crate::world) next_level: u8,
    pub(in crate::world) last_hit_at: Option<Instant>,
}

#[derive(Debug)]
pub(in crate::world) struct PlayerRewardRuntimeUpdate {
    pub(in crate::world) level: u8,
    pub(in crate::world) xp: u32,
    pub(in crate::world) health: u32,
    pub(in crate::world) max_health: u32,
    pub(in crate::world) power1: u32,
    pub(in crate::world) max_power1: u32,
    pub(in crate::world) power2: u32,
    pub(in crate::world) world_stats: Option<PlayerWorldStats>,
    pub(in crate::world) combat_stats: Option<PlayerCombatStats>,
    pub(in crate::world) quest_statuses: HashMap<u32, CharacterQuestStatus>,
}

#[derive(Debug, Clone, Copy)]
pub(in crate::world) struct PlayerLevelProgressionRuntimeUpdate {
    pub(in crate::world) level: u8,
    pub(in crate::world) xp: u32,
    pub(in crate::world) health: u32,
    pub(in crate::world) power1: u32,
    pub(in crate::world) power2: u32,
    pub(in crate::world) power4: u32,
    pub(in crate::world) world_stats: PlayerWorldStats,
    pub(in crate::world) combat_stats: PlayerCombatStats,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::world) struct PlayerComboPointsEvent {
    pub(in crate::world) combo_target: ObjectGuid,
    pub(in crate::world) combo_points: u8,
    pub(in crate::world) player_bytes: u32,
}

#[derive(Debug)]
pub(in crate::world) struct PlayerHealEvent {
    pub(in crate::world) healed_character_guid: u32,
    pub(in crate::world) amount_healed: u32,
    pub(in crate::world) health: u32,
    pub(in crate::world) direct_session_id: SessionId,
    pub(in crate::world) direct_packets: Vec<OutboundWorldPacket>,
    pub(in crate::world) observer_packets: Vec<(SessionId, OutboundWorldPacket)>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(in crate::world) struct ScheduledDbCreatureMotionStart {
    pub(in crate::world) due_at: Instant,
    pub(in crate::world) guid: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(in crate::world) struct ScheduledDbCreatureMotionAdvance {
    pub(in crate::world) due_at: Instant,
    pub(in crate::world) guid: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(in crate::world) struct ScheduledDbCreatureLifecycle {
    pub(in crate::world) due_at: Instant,
    pub(in crate::world) guid: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(in crate::world) struct ScheduledDbCreatureCombat {
    pub(in crate::world) due_at: Instant,
    pub(in crate::world) guid: u64,
}

#[derive(Debug, Clone)]
pub(in crate::world) enum DbCreatureOocEventAiCapability {
    Unknown,
    None,
    OocCast(Arc<[wow_db::CreatureAiScriptQuery]>),
}

#[derive(Debug)]
pub(in crate::world) enum ReadyDbCreatureOocEventAiAction {
    Complete {
        attacker: ObjectGuid,
        victim: ObjectGuid,
    },
    Start {
        attacker: ObjectGuid,
        ready: ReadyDbCreatureEventAiSpellCast,
    },
}

#[derive(Debug, Default)]
pub(in crate::world) struct DbCreatureOocEventAiTick {
    pub(in crate::world) packets: Vec<(SessionId, OutboundWorldPacket)>,
}

#[derive(Debug, Default)]
pub(in crate::world) struct PlayerVisibilityRefreshTick {
    pub(in crate::world) packets: Vec<(SessionId, OutboundWorldPacket)>,
    pub(in crate::world) refreshed_players: u32,
    pub(in crate::world) budget_exhausted: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::world) enum DbCreatureVictimCombatLocalEffect {
    Melee {
        attacker: ObjectGuid,
        damage_taken: u32,
        victim_health: u32,
        rage_gain: u32,
        player_died: bool,
    },
    SpellDamage {
        victim_health: u32,
        player_died: bool,
    },
}

#[derive(Debug, Default)]
pub(in crate::world) struct DbCreatureVictimCombatAdvanceTick {
    pub(in crate::world) direct_packets: Vec<OutboundWorldPacket>,
    pub(in crate::world) observer_packets: Vec<(SessionId, OutboundWorldPacket)>,
    pub(in crate::world) local_effects: Vec<DbCreatureVictimCombatLocalEffect>,
    pub(in crate::world) active_combats: Vec<CreatureCombatState>,
    pub(in crate::world) player_in_combat: bool,
}

#[derive(Debug)]
#[allow(dead_code)]
pub(in crate::world) struct MapRuntime {
    pub(in crate::world) map_id: u32,
    pub(in crate::world) instance_id: u32,
    pub(in crate::world) geometry: Arc<WorldGeometry>,
    pub(in crate::world) db_scripts: Arc<DbScriptRegistry>,
    pub(in crate::world) grids: HashMap<GridCoord, GridRuntime>,
    pub(in crate::world) loaded_creature_grids: HashSet<GridCoord>,
    pub(in crate::world) loaded_gameobject_grids: HashSet<GridCoord>,
    pub(in crate::world) loaded_player_corpse_grids: HashSet<GridCoord>,
    pub(in crate::world) players: HashMap<u32, PlayerRuntime>,
    pub(in crate::world) pending_player_visibility_refreshes: BTreeSet<u32>,
    pub(in crate::world) pending_player_visibility_refresh_old_positions:
        HashMap<u32, WorldPosition>,
    pub(in crate::world) player_movement_heartbeat_broadcast_server_time: HashMap<u32, u32>,
    pub(in crate::world) creatures: HashMap<u64, DbCreatureRuntime>,
    pub(in crate::world) creature_looting_by_character: HashMap<u32, u64>,
    pub(in crate::world) gameobjects: HashMap<u64, DbGameObjectRuntime>,
    pub(in crate::world) gameobject_loots: HashMap<u64, DbGameObjectLootState>,
    pub(in crate::world) gameobject_looting_by_character: HashMap<u32, u64>,
    pub(in crate::world) active_creature_combats: HashMap<u64, CreatureCombatState>,
    pub(in crate::world) creature_combats_by_victim: HashMap<u64, BTreeSet<u64>>,
    pub(in crate::world) active_creature_combat_due: BinaryHeap<Reverse<ScheduledDbCreatureCombat>>,
    pub(in crate::world) active_creature_spell_casts: HashMap<u64, ActiveDbCreatureSpellCast>,
    pub(in crate::world) creature_combat_leash: HashMap<u64, CreatureCombatLeashState>,
    pub(in crate::world) creature_threats: HashMap<u64, Vec<CreatureThreatEntry>>,
    pub(in crate::world) corpses: HashMap<u64, PlayerCorpseRuntime>,
    pub(in crate::world) dynamic_objects: HashMap<u64, DynamicObjectRuntime>,
    pub(in crate::world) next_dynamic_object_counter: u32,
    pub(in crate::world) active_playerbot_count: usize,
    pub(in crate::world) playerbot_intents: HashMap<u32, PlayerbotQueuedIntents>,
    pub(in crate::world) next_idle_motion_tick_at: Option<Instant>,
    pub(in crate::world) next_confused_motion_start_check_at: Option<Instant>,
    pub(in crate::world) next_idle_motion_start_check_at: Option<Instant>,
    pub(in crate::world) idle_motion_start_schedule_dirty: bool,
    pub(in crate::world) active_db_creature_motion_guids: BTreeSet<u64>,
    pub(in crate::world) db_creature_motion_advance_due_at: HashMap<u64, Instant>,
    pub(in crate::world) idle_db_creature_motion_advances:
        BinaryHeap<Reverse<ScheduledDbCreatureMotionAdvance>>,
    pub(in crate::world) confused_db_creature_motion_start_due_at: HashMap<u64, Instant>,
    pub(in crate::world) confused_db_creature_motion_starts:
        BinaryHeap<Reverse<ScheduledDbCreatureMotionStart>>,
    pub(in crate::world) idle_db_creature_motion_start_due_at: HashMap<u64, Instant>,
    pub(in crate::world) idle_db_creature_motion_starts:
        BinaryHeap<Reverse<ScheduledDbCreatureMotionStart>>,
    pub(in crate::world) db_creature_corpse_expiry_due_at: HashMap<u64, Instant>,
    pub(in crate::world) db_creature_corpse_expiries:
        BinaryHeap<Reverse<ScheduledDbCreatureLifecycle>>,
    pub(in crate::world) db_creature_respawn_due_at: HashMap<u64, Instant>,
    pub(in crate::world) db_creature_respawns: BinaryHeap<Reverse<ScheduledDbCreatureLifecycle>>,
    pub(in crate::world) db_creature_ooc_event_ai_capabilities:
        HashMap<u32, DbCreatureOocEventAiCapability>,
    pub(in crate::world) active_player_environment_guids: HashSet<u32>,
    pub(in crate::world) pending_db_scripts: BinaryHeap<Reverse<ScheduledPendingDbScriptAction>>,
    pub(in crate::world) next_pending_db_script_sequence: u64,
    pub(in crate::world) next_player_regen_tick_at: Option<Instant>,
    pub(in crate::world) active_player_spell_casts: HashMap<u32, ActivePlayerSpellCast>,
    pub(in crate::world) active_player_channels: HashMap<u32, ActivePlayerChannel>,
    pub(in crate::world) pending_player_channel_impacts: Vec<PendingPlayerChannelImpact>,
    pub(in crate::world) pending_spell_events: Vec<PendingSpellEvent>,
    pub(in crate::world) next_spell_event_id: u64,
    pub(in crate::world) pending_player_death_presentations:
        HashMap<u32, PlayerDeathPresentationRuntime>,
    pub(in crate::world) tracked_single_target_auras:
        HashMap<u64, Vec<TrackedSingleTargetAuraRuntime>>,
    pub(in crate::world) active_diminishing_auras:
        HashMap<(u64, u64, u32), DiminishingGroupRuntime>,
    pub(in crate::world) diminishing_states:
        HashMap<u64, HashMap<DiminishingGroupRuntime, DiminishingStateRuntime>>,
}

#[derive(Debug, Clone)]
pub(in crate::world) struct ActivePlayerSpellCast {
    pub(in crate::world) spell_id: u32,
    pub(in crate::world) source: ActivePlayerSpellCastSource,
    pub(in crate::world) profile: SpellCastProfile,
    pub(in crate::world) targets: PendingSpellCastTargets,
    pub(in crate::world) due_at: Instant,
    pub(in crate::world) cast_time_millis: u32,
    pub(in crate::world) interrupt_flags: u32,
    pub(in crate::world) damage_pushback_count: u8,
}

#[derive(Debug, Clone)]
pub(in crate::world) struct ActivePlayerChannel {
    pub(in crate::world) caster: ObjectGuid,
    pub(in crate::world) caster_character_guid: u32,
    pub(in crate::world) spell_id: u32,
    pub(in crate::world) target: ObjectGuid,
    pub(in crate::world) expires_at: Instant,
    pub(in crate::world) next_tick_at: Instant,
    pub(in crate::world) tick_millis: u32,
    pub(in crate::world) ticks_remaining: u32,
    pub(in crate::world) channel_interrupt_flags: u32,
    pub(in crate::world) damage_delay_count: u8,
    pub(in crate::world) triggered_spell_speed: f32,
    pub(in crate::world) damage_effect: PlayerDirectDamageEffect,
}

#[derive(Debug, Default)]
pub(in crate::world) struct PlayerSpellRuntimeCleanupPackets {
    pub(in crate::world) direct_packets: Vec<OutboundWorldPacket>,
    pub(in crate::world) observer_packets: Vec<(SessionId, OutboundWorldPacket)>,
}

#[derive(Debug, Clone)]
pub(in crate::world) struct PendingPlayerChannelImpact {
    pub(in crate::world) caster: ObjectGuid,
    pub(in crate::world) caster_character_guid: u32,
    pub(in crate::world) target: ObjectGuid,
    pub(in crate::world) impact_at: Instant,
    pub(in crate::world) damage_effect: PlayerDirectDamageEffect,
    pub(in crate::world) outcome: SpellDamageOutcome,
}

#[derive(Debug, Clone)]
pub(in crate::world) struct ActiveDbCreatureSpellCast {
    pub(in crate::world) caster: ObjectGuid,
    pub(in crate::world) target: ObjectGuid,
    pub(in crate::world) spell_id: u32,
    pub(in crate::world) requires_behind: bool,
    pub(in crate::world) effect: ActiveDbCreatureSpellEffect,
    pub(in crate::world) aura: Option<ActiveAura>,
    pub(in crate::world) range: Option<SpellRangeEntry>,
    pub(in crate::world) mana_cost: u32,
    pub(in crate::world) cast_time_millis: u32,
    pub(in crate::world) due_at: Instant,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::world) struct DbCreatureRespawnPersistenceUpdate {
    pub(in crate::world) creature_spawn_guid: u32,
}

#[derive(Debug, Default)]
pub(in crate::world) struct DbCreatureLifecycleTick {
    pub(in crate::world) packets: Vec<(SessionId, OutboundWorldPacket)>,
    pub(in crate::world) respawn_updates: Vec<DbCreatureRespawnPersistenceUpdate>,
}

#[derive(Debug, Clone)]
pub(in crate::world) enum ActiveDbCreatureSpellEffect {
    None,
    Damage {
        amount: u32,
        school: u8,
        dmg_class: u32,
        attributes_ex2: u32,
        attributes_ex3: u32,
    },
    Heal {
        amount: u32,
    },
}

#[derive(Debug, Clone)]
pub(in crate::world) enum ActivePlayerSpellCastSource {
    Player,
    OpeningGameObject,
    Item {
        item_guid: ObjectGuid,
        source_item: CharacterInventoryItem,
        spell_charges: i32,
    },
}

#[derive(Debug, Clone)]
pub(in crate::world) enum PendingSpellEventKind {
    Spell {
        targets: PendingSpellCastTargets,
        target_outcome: Option<PlayerSpellTargetOutcome>,
    },
    RangedAutoAttack {
        target: ObjectGuid,
        outcome: MeleeDamageOutcome,
        weapon_skill_id: Option<u16>,
    },
}

#[derive(Debug, Clone, Copy)]
pub(in crate::world) struct PendingRangedAutoAttackImpact {
    pub(in crate::world) spell_id: u32,
    pub(in crate::world) target: ObjectGuid,
    pub(in crate::world) outcome: MeleeDamageOutcome,
    pub(in crate::world) weapon_skill_id: Option<u16>,
    pub(in crate::world) due_at: Instant,
}

#[derive(Debug, Clone)]
pub(in crate::world) struct PendingSpellEvent {
    pub(in crate::world) event_id: u64,
    pub(in crate::world) caster_character_guid: u32,
    pub(in crate::world) spell_id: u32,
    pub(in crate::world) kind: PendingSpellEventKind,
    pub(in crate::world) unit_target_generation: Option<(ObjectGuid, u64)>,
    pub(in crate::world) due_at: Instant,
}

#[derive(Debug, Clone)]
pub(in crate::world) struct PendingSpellCastTargets {
    pub(in crate::world) target_mask: u16,
    pub(in crate::world) unit_target: Option<ObjectGuid>,
    pub(in crate::world) gameobject_target: Option<ObjectGuid>,
    pub(in crate::world) source_location: Option<wow_proto::SpellTargetLocation>,
    pub(in crate::world) destination: Option<wow_proto::SpellTargetLocation>,
}

#[derive(Debug, Clone, Copy)]
pub(in crate::world) struct PlayerEnvironmentRuntime {
    pub(in crate::world) flags: u32,
    pub(in crate::world) last_flags_position: Option<WorldPosition>,
    pub(in crate::world) next_flags_refresh_at: Option<Instant>,
    pub(in crate::world) last_tick_at: Option<Instant>,
    pub(in crate::world) last_damage_at: Option<Instant>,
    pub(in crate::world) fatigue: MirrorTimerRuntime,
    pub(in crate::world) breath: MirrorTimerRuntime,
    pub(in crate::world) environmental: MirrorTimerRuntime,
}

impl Default for PlayerEnvironmentRuntime {
    fn default() -> Self {
        Self {
            flags: 0,
            last_flags_position: None,
            next_flags_refresh_at: None,
            last_tick_at: None,
            last_damage_at: None,
            fatigue: MirrorTimerRuntime::new(MIRROR_TIMER_FATIGUE, MIRROR_TIMER_FATIGUE_MAX_MILLIS),
            breath: MirrorTimerRuntime::new(MIRROR_TIMER_BREATH, MIRROR_TIMER_BREATH_MAX_MILLIS),
            environmental: MirrorTimerRuntime::new(
                MIRROR_TIMER_ENVIRONMENTAL,
                MIRROR_TIMER_ENVIRONMENTAL_MAX_MILLIS,
            ),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub(in crate::world) struct MirrorTimerRuntime {
    pub(in crate::world) timer_type: u32,
    pub(in crate::world) active: bool,
    pub(in crate::world) elapsed_millis: u32,
    pub(in crate::world) pulse_millis: u32,
    pub(in crate::world) duration_millis: u32,
    pub(in crate::world) scale: i32,
}

impl MirrorTimerRuntime {
    pub(in crate::world) const fn new(timer_type: u32, duration_millis: u32) -> Self {
        Self {
            timer_type,
            active: false,
            elapsed_millis: 0,
            pulse_millis: 0,
            duration_millis,
            scale: -1,
        }
    }
}

#[derive(Debug)]
#[allow(dead_code)]
pub(in crate::world) struct DbCreaturePlayerDamageEvent {
    pub(in crate::world) damage: u32,
    pub(in crate::world) victim_health: u32,
    pub(in crate::world) combat: CreatureCombatState,
    pub(in crate::world) direct_packets: Vec<OutboundWorldPacket>,
    pub(in crate::world) aura_packet: Option<OutboundWorldPacket>,
    pub(in crate::world) health_update_body: Vec<u8>,
    pub(in crate::world) observer_packets: Vec<(SessionId, OutboundWorldPacket)>,
}

#[derive(Debug)]
#[allow(dead_code)]
pub(in crate::world) struct DbCreaturePlayerSpellDamageEvent {
    pub(in crate::world) damage: u32,
    pub(in crate::world) victim_health: u32,
    pub(in crate::world) outcome: SpellDamageOutcome,
    pub(in crate::world) spell_non_melee_log_body: Option<Vec<u8>>,
    pub(in crate::world) spell_miss_log_body: Option<Vec<u8>>,
    pub(in crate::world) direct_packets: Vec<OutboundWorldPacket>,
    pub(in crate::world) aura_packet: Option<OutboundWorldPacket>,
    pub(in crate::world) health_update_body: Vec<u8>,
    pub(in crate::world) observer_packets: Vec<(SessionId, OutboundWorldPacket)>,
}

#[derive(Debug)]
pub(in crate::world) struct DbCreatureCompletedSpellCastEvent {
    pub(in crate::world) spell_go_body: Vec<u8>,
    pub(in crate::world) effect: DbCreatureCompletedSpellEffect,
    pub(in crate::world) aura_event: Option<PlayerAuraUpdateEvent>,
    pub(in crate::world) creature_aura_event: Option<DbCreatureAuraUpdateEvent>,
}

#[derive(Debug)]
pub(in crate::world) enum DbCreatureCompletedSpellEffect {
    AuraOnly,
    PlayerDamage(DbCreaturePlayerSpellDamageEvent),
    CreatureHeal(DbCreatureSpellHealEvent),
    Interrupted(DbCreatureInterruptedSpellCastEvent),
}

#[derive(Debug)]
#[allow(dead_code)]
pub(in crate::world) struct DbCreatureInterruptedSpellCastEvent {
    pub(in crate::world) failure: u8,
    pub(in crate::world) observer_packets: Vec<(SessionId, OutboundWorldPacket)>,
}

#[derive(Debug)]
#[allow(dead_code)]
pub(in crate::world) struct DbCreatureSpellHealEvent {
    pub(in crate::world) target: ObjectGuid,
    pub(in crate::world) amount: u32,
    pub(in crate::world) target_health: u32,
    pub(in crate::world) spell_heal_log_body: Vec<u8>,
    pub(in crate::world) health_update_body: Vec<u8>,
    pub(in crate::world) observer_packets: Vec<(SessionId, OutboundWorldPacket)>,
}

#[derive(Debug, Clone, Copy)]
pub(in crate::world) struct CreatureCombatLeashState {
    pub(in crate::world) refresh_position: WorldPosition,
    pub(in crate::world) combat_start_position: WorldPosition,
    pub(in crate::world) expires_at: Instant,
    pub(in crate::world) template_leash_yards: f32,
}

#[derive(Debug)]
pub(in crate::world) struct DbCreatureDamageEvent {
    #[allow(dead_code)]
    pub(in crate::world) damage: u32,
    pub(in crate::world) attacker_rage_damage: u32,
    pub(in crate::world) creature: DbCreatureRuntime,
    pub(in crate::world) attacker_state_body: Option<Vec<u8>>,
    pub(in crate::world) spell_non_melee_log_body: Option<Vec<u8>>,
    pub(in crate::world) spell_miss_log_body: Option<Vec<u8>>,
    pub(in crate::world) update_body: Vec<u8>,
    pub(in crate::world) direct_packets: Vec<OutboundWorldPacket>,
    pub(in crate::world) death_finalization: Option<DbCreatureDeathFinalizationEvent>,
    pub(in crate::world) target_switch: Option<DbCreatureThreatTargetSwitchEvent>,
    pub(in crate::world) observer_packets: Vec<(SessionId, OutboundWorldPacket)>,
}

#[derive(Debug)]
pub(in crate::world) struct DbCreatureAuraUpdateEvent {
    pub(in crate::world) update_body: Vec<u8>,
    pub(in crate::world) direct_packets: Vec<OutboundWorldPacket>,
    pub(in crate::world) observer_packets: Vec<(SessionId, OutboundWorldPacket)>,
}

#[derive(Debug)]
pub(in crate::world) struct DbCreatureAuraDispelEvent {
    pub(in crate::world) removed_spell_ids: Vec<u32>,
    pub(in crate::world) aura_update: DbCreatureAuraUpdateEvent,
}

#[derive(Debug, Clone)]
pub(in crate::world) struct DbCreatureDamageRequest {
    pub(in crate::world) creature_guid: ObjectGuid,
    pub(in crate::world) killer: ObjectGuid,
    pub(in crate::world) damage: u32,
    pub(in crate::world) melee_outcome: Option<MeleeDamageOutcome>,
    pub(in crate::world) spell_damage_outcome: Option<SpellDamageOutcome>,
    pub(in crate::world) spell_id: Option<u32>,
    pub(in crate::world) spell_school: u8,
    pub(in crate::world) suppress_attacker_state: bool,
    pub(in crate::world) now: Instant,
    pub(in crate::world) now_epoch_secs: u64,
    pub(in crate::world) exclude_character_guid: Option<u32>,
    pub(in crate::world) corpse_loot: Option<DbCreatureCorpseLootInit>,
}

#[derive(Debug)]
pub(in crate::world) struct DbCreatureDeleteEvent {
    pub(in crate::world) creature: DbCreatureRuntime,
    pub(in crate::world) direct_packet: OutboundWorldPacket,
    pub(in crate::world) observer_packets: Vec<(SessionId, OutboundWorldPacket)>,
}

#[derive(Debug, Clone)]
pub(in crate::world) struct DbCreatureCorpseLootInit {
    pub(in crate::world) owner: CreatureLootOwner,
    pub(in crate::world) allowed_players: Vec<u32>,
    pub(in crate::world) current_looter: Option<u32>,
    pub(in crate::world) loot_method: Option<CreatureLootMethod>,
    pub(in crate::world) loot_items: Vec<DbCreatureLootRuntime>,
}

#[derive(Debug)]
pub(in crate::world) struct DbCreatureDeathFinalizationEvent {
    pub(in crate::world) killed: ObjectGuid,
    pub(in crate::world) respawn_epoch_secs: Option<u64>,
    pub(in crate::world) motion_stop_packet: Option<OutboundWorldPacket>,
    pub(in crate::world) attack_stop_packet: OutboundWorldPacket,
    pub(in crate::world) combat_flag_packet: OutboundWorldPacket,
    pub(in crate::world) observer_packets: Vec<(SessionId, OutboundWorldPacket)>,
}

#[derive(Debug)]
pub(in crate::world) struct DbCreatureThreatTargetSwitchEvent {
    pub(in crate::world) attacker: ObjectGuid,
    pub(in crate::world) old_victim: ObjectGuid,
    pub(in crate::world) new_victim: ObjectGuid,
    pub(in crate::world) combat: CreatureCombatState,
    pub(in crate::world) direct_packets: Vec<OutboundWorldPacket>,
    pub(in crate::world) observer_packets: Vec<(SessionId, OutboundWorldPacket)>,
}

#[derive(Debug)]
pub(in crate::world) struct DbCreatureEventAiActionsEvent {
    pub(in crate::world) creature: DbCreatureRuntime,
    pub(in crate::world) direct_packets: Vec<OutboundWorldPacket>,
    pub(in crate::world) observer_packets: Vec<(SessionId, OutboundWorldPacket)>,
}

#[cfg(test)]
#[derive(Debug)]
pub(in crate::world) struct DbCreatureLifecycleEvent {
    #[allow(dead_code)]
    pub(in crate::world) creature: DbCreatureRuntime,
    pub(in crate::world) direct_packets: Vec<OutboundWorldPacket>,
    pub(in crate::world) observer_packets: Vec<(SessionId, OutboundWorldPacket)>,
    pub(in crate::world) clear_respawn_guid: Option<u32>,
}

#[derive(Debug)]
pub(in crate::world) struct DbCreatureLootReleaseEvent {
    pub(in crate::world) creature: DbCreatureRuntime,
    pub(in crate::world) direct_packet: OutboundWorldPacket,
    pub(in crate::world) observer_packets: Vec<(SessionId, OutboundWorldPacket)>,
}

#[derive(Debug)]
pub(in crate::world) struct PlayerAuraUpdateEvent {
    pub(in crate::world) direct_packets: Vec<OutboundWorldPacket>,
    pub(in crate::world) observer_packets: Vec<(SessionId, OutboundWorldPacket)>,
}

#[derive(Debug)]
pub(in crate::world) struct PlayerAuraDispelEvent {
    pub(in crate::world) removed_spell_ids: Vec<u32>,
    pub(in crate::world) aura_update: PlayerAuraUpdateEvent,
}

#[derive(Debug, Default)]
pub(in crate::world) struct MapDbCreatureVisibilityStage {
    pub(in crate::world) nearby_creatures: Vec<DbCreatureRuntime>,
    pub(in crate::world) create_guids: Vec<ObjectGuid>,
    pub(in crate::world) destroy_guids: Vec<ObjectGuid>,
}

#[derive(Debug, Default)]
pub(in crate::world) struct MapDbGameObjectVisibilityStage {
    pub(in crate::world) nearby_gameobjects: Vec<DbGameObjectRuntime>,
    pub(in crate::world) create_guids: Vec<ObjectGuid>,
    pub(in crate::world) destroy_guids: Vec<ObjectGuid>,
}

#[derive(Debug, Default)]
pub(in crate::world) struct MapPlayerCorpseVisibilityStage {
    pub(in crate::world) nearby_corpses: Vec<PlayerCorpseRuntime>,
    pub(in crate::world) create_guids: Vec<ObjectGuid>,
    pub(in crate::world) destroy_guids: Vec<ObjectGuid>,
}

#[derive(Debug, Default)]
pub(in crate::world) struct DbGameObjectLootState {
    pub(in crate::world) open_characters: HashSet<u32>,
    pub(in crate::world) loot_items: Vec<DbCreatureLootRuntime>,
}

impl MapRuntime {
    #[cfg(test)]
    pub(in crate::world) fn new(map_id: u32, instance_id: u32) -> Self {
        Self::with_geometry(
            map_id,
            instance_id,
            Arc::new(WorldGeometry::default()),
            Arc::new(DbScriptRegistry::default()),
        )
    }

    #[allow(dead_code)]
    pub(in crate::world) fn with_geometry(
        map_id: u32,
        instance_id: u32,
        geometry: Arc<WorldGeometry>,
        db_scripts: Arc<DbScriptRegistry>,
    ) -> Self {
        Self {
            map_id,
            instance_id,
            geometry,
            db_scripts,
            grids: HashMap::new(),
            loaded_creature_grids: HashSet::new(),
            loaded_gameobject_grids: HashSet::new(),
            loaded_player_corpse_grids: HashSet::new(),
            players: HashMap::new(),
            pending_player_visibility_refreshes: BTreeSet::new(),
            pending_player_visibility_refresh_old_positions: HashMap::new(),
            player_movement_heartbeat_broadcast_server_time: HashMap::new(),
            creatures: HashMap::new(),
            creature_looting_by_character: HashMap::new(),
            gameobjects: HashMap::new(),
            gameobject_loots: HashMap::new(),
            gameobject_looting_by_character: HashMap::new(),
            active_creature_combats: HashMap::new(),
            creature_combats_by_victim: HashMap::new(),
            active_creature_combat_due: BinaryHeap::new(),
            active_creature_spell_casts: HashMap::new(),
            creature_combat_leash: HashMap::new(),
            creature_threats: HashMap::new(),
            corpses: HashMap::new(),
            dynamic_objects: HashMap::new(),
            next_dynamic_object_counter: 1,
            active_playerbot_count: 0,
            playerbot_intents: HashMap::new(),
            next_idle_motion_tick_at: None,
            next_confused_motion_start_check_at: None,
            next_idle_motion_start_check_at: None,
            idle_motion_start_schedule_dirty: true,
            active_db_creature_motion_guids: BTreeSet::new(),
            db_creature_motion_advance_due_at: HashMap::new(),
            idle_db_creature_motion_advances: BinaryHeap::new(),
            confused_db_creature_motion_start_due_at: HashMap::new(),
            confused_db_creature_motion_starts: BinaryHeap::new(),
            idle_db_creature_motion_start_due_at: HashMap::new(),
            idle_db_creature_motion_starts: BinaryHeap::new(),
            db_creature_corpse_expiry_due_at: HashMap::new(),
            db_creature_corpse_expiries: BinaryHeap::new(),
            db_creature_respawn_due_at: HashMap::new(),
            db_creature_respawns: BinaryHeap::new(),
            db_creature_ooc_event_ai_capabilities: HashMap::new(),
            active_player_environment_guids: HashSet::new(),
            pending_db_scripts: BinaryHeap::new(),
            next_pending_db_script_sequence: 0,
            next_player_regen_tick_at: None,
            active_player_spell_casts: HashMap::new(),
            active_player_channels: HashMap::new(),
            pending_player_channel_impacts: Vec::new(),
            pending_spell_events: Vec::new(),
            next_spell_event_id: 1,
            pending_player_death_presentations: HashMap::new(),
            tracked_single_target_auras: HashMap::new(),
            active_diminishing_auras: HashMap::new(),
            diminishing_states: HashMap::new(),
        }
    }

    pub(in crate::world) fn observability_snapshot(
        &self,
    ) -> crate::observability::MapRuntimeSnapshot {
        crate::observability::MapRuntimeSnapshot {
            map_id: self.map_id,
            instance_id: self.instance_id,
            active_players: self.players.len() as u64,
            active_creatures: self.creatures.len() as u64,
            active_gameobjects: self.gameobjects.len() as u64,
            loaded_grids: self.grids.len() as u64,
            loaded_creature_grids: self.loaded_creature_grids.len() as u64,
            loaded_gameobject_grids: self.loaded_gameobject_grids.len() as u64,
            loaded_player_corpse_grids: self.loaded_player_corpse_grids.len() as u64,
            active_creature_combats: self.active_creature_combats.len() as u64,
            corpses: self.corpses.len() as u64,
            active_playerbots: self.active_playerbot_count as u64,
            tracked_idle_motion_creatures: self.active_db_creature_motion_guids.len() as u64,
            tracked_idle_motion_start_candidates: (self.confused_db_creature_motion_starts.len()
                + self.idle_db_creature_motion_starts.len())
                as u64,
        }
    }

    pub(in crate::world) fn current_diminishing_level(
        &mut self,
        target: ObjectGuid,
        group: DiminishingGroupRuntime,
        now: Instant,
    ) -> DiminishingLevelRuntime {
        let Some(groups) = self.diminishing_states.get_mut(&target.raw()) else {
            return DiminishingLevelRuntime::Level1;
        };
        let Some(state) = groups.get_mut(&group) else {
            return DiminishingLevelRuntime::Level1;
        };
        if state.active_stack_count == 0
            && state.last_hit_at.is_some_and(|last_hit_at| {
                now.saturating_duration_since(last_hit_at) > Duration::from_secs(15)
            })
        {
            state.next_level = 0;
        }
        match state.next_level {
            0 => DiminishingLevelRuntime::Level1,
            1 => DiminishingLevelRuntime::Level2,
            2 => DiminishingLevelRuntime::Level3,
            _ => DiminishingLevelRuntime::Immune,
        }
    }

    pub(in crate::world) fn register_diminishing_aura(
        &mut self,
        target: ObjectGuid,
        caster: ObjectGuid,
        spell_id: u32,
        group: DiminishingGroupRuntime,
        now: Instant,
    ) {
        self.active_diminishing_auras
            .insert((target.raw(), caster.raw(), spell_id), group);
        let state = self
            .diminishing_states
            .entry(target.raw())
            .or_default()
            .entry(group)
            .or_default();
        state.active_stack_count = state.active_stack_count.saturating_add(1);
        state.last_hit_at = Some(now);
        state.next_level = match state.next_level {
            0 => 1,
            1 => 2,
            _ => 3,
        };
    }

    pub(in crate::world) fn reconcile_target_aura_trackers(
        &mut self,
        target: ObjectGuid,
        active_auras: &[ActiveAura],
        now: Instant,
    ) {
        let active_pairs = active_auras
            .iter()
            .map(|aura| (aura.caster.raw(), aura.spell_id))
            .collect::<HashSet<_>>();
        self.tracked_single_target_auras
            .retain(|caster_raw, entries| {
                entries.retain(|entry| {
                    entry.target != target
                        || active_pairs.contains(&(*caster_raw, entry.descriptor.spell_id))
                });
                !entries.is_empty()
            });
        let removed = self
            .active_diminishing_auras
            .iter()
            .filter_map(|(key, group)| {
                (key.0 == target.raw() && !active_pairs.contains(&(key.1, key.2)))
                    .then_some((*key, *group))
            })
            .collect::<Vec<_>>();
        for (key, group) in removed {
            self.active_diminishing_auras.remove(&key);
            if let Some(state) = self
                .diminishing_states
                .get_mut(&target.raw())
                .and_then(|groups| groups.get_mut(&group))
            {
                state.active_stack_count = state.active_stack_count.saturating_sub(1);
                state.last_hit_at = Some(now);
            }
        }
    }
}

#[path = "systems/creature_combat.rs"]
mod creature_combat;
#[path = "systems/creature_damage.rs"]
mod creature_damage;
#[path = "systems/creature_lifecycle.rs"]
mod creature_lifecycle;
#[path = "systems/creature_loot.rs"]
mod creature_loot;
#[path = "systems/creature_motion.rs"]
mod creature_motion;
#[path = "systems/creature_snapshots.rs"]
mod creature_snapshots;
#[path = "systems/damage.rs"]
mod damage;
#[path = "systems/dynamic_objects.rs"]
mod dynamic_objects;
#[path = "systems/gameobject_loot.rs"]
mod gameobject_loot;
#[path = "systems/gameobject_snapshots.rs"]
mod gameobject_snapshots;
#[path = "systems/player_channels.rs"]
mod player_channels;
#[path = "systems/player_corpses.rs"]
mod player_corpses;
#[path = "systems/playerbots.rs"]
mod playerbots;
#[path = "systems/players.rs"]
mod players;
#[path = "systems/spatial.rs"]
mod spatial;

pub(in crate::world) use self::creature_combat::*;
pub(in crate::world) use self::creature_damage::*;
pub(in crate::world) use self::creature_loot::*;
pub(in crate::world) use self::creature_motion::*;
pub(in crate::world) use self::damage::*;
pub(in crate::world) use self::dynamic_objects::*;
pub(in crate::world) use self::player_channels::*;
pub(in crate::world) use self::playerbots::*;
pub(in crate::world) use self::players::*;
