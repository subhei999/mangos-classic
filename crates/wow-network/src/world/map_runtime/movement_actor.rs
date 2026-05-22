use super::*;
use tokio::sync::{mpsc, oneshot};
#[cfg(test)]
use wow_proto::world::WorldOpcode;

const DEFAULT_MOVEMENT_ACTOR_QUEUE_CAPACITY: usize = 1024;
const DEFAULT_MOVEMENT_ACTOR_MAX_BATCH_SIZE: usize = 64;
const MOVEMENT_ACTOR_CHANNEL: &str = "movement_actor";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::world) struct MovementActorSettings {
    pub(in crate::world) enabled: bool,
    pub(in crate::world) queue_capacity: usize,
    pub(in crate::world) max_batch_size: usize,
}

impl Default for MovementActorSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            queue_capacity: DEFAULT_MOVEMENT_ACTOR_QUEUE_CAPACITY,
            max_batch_size: DEFAULT_MOVEMENT_ACTOR_MAX_BATCH_SIZE,
        }
    }
}

impl MovementActorSettings {
    pub(in crate::world) fn new(
        enabled: bool,
        queue_capacity: usize,
        max_batch_size: usize,
    ) -> Self {
        Self {
            enabled,
            queue_capacity,
            max_batch_size,
        }
    }

    #[cfg(test)]
    pub(in crate::world) fn for_test(
        enabled: bool,
        queue_capacity: usize,
        max_batch_size: usize,
    ) -> Self {
        Self::new(enabled, queue_capacity, max_batch_size)
    }
}

#[derive(Debug, Clone)]
pub(in crate::world) enum MovementUpdateOutcome {
    Applied {
        packets: Vec<(SessionId, OutboundWorldPacket)>,
    },
}

#[derive(Debug)]
pub(in crate::world) enum MovementActorCommand {
    UpdatePlayerPosition(MovementUpdateCommand),
}

#[derive(Debug)]
pub(in crate::world) struct MovementUpdateCommand {
    pub(in crate::world) character_guid: u32,
    pub(in crate::world) opcode: u16,
    pub(in crate::world) movement: MovementInfo,
    pub(in crate::world) server_time: u32,
    pub(in crate::world) enqueued_at: Instant,
    pub(in crate::world) reply: oneshot::Sender<anyhow::Result<MovementUpdateOutcome>>,
}

#[derive(Debug, Clone)]
pub(in crate::world) struct MovementActorHandle {
    pub(in crate::world) sender: mpsc::Sender<MovementActorCommand>,
}

impl MovementActorHandle {
    pub(in crate::world) fn spawn_proxy(
        map: Arc<Mutex<MapRuntime>>,
        settings: MovementActorSettings,
    ) -> Self {
        let queue_capacity = settings.queue_capacity.max(1);
        let max_batch_size = settings.max_batch_size.max(1);
        let (sender, receiver) = mpsc::channel(queue_capacity);
        tokio::spawn(run_movement_actor(map, receiver, max_batch_size));
        Self { sender }
    }

    pub(in crate::world) async fn update_player_position(
        &self,
        character_guid: u32,
        opcode: u16,
        movement: &MovementInfo,
        server_time: u32,
    ) -> anyhow::Result<MovementUpdateOutcome> {
        let enqueue_started_at = Instant::now();
        let (reply_tx, reply_rx) = oneshot::channel();
        let command = MovementActorCommand::UpdatePlayerPosition(MovementUpdateCommand {
            character_guid,
            opcode,
            movement: movement.clone(),
            server_time,
            enqueued_at: Instant::now(),
            reply: reply_tx,
        });
        match self.sender.try_send(command) {
            Ok(()) => {
                crate::observability::record_channel_send_wait(
                    MOVEMENT_ACTOR_CHANNEL,
                    enqueue_started_at.elapsed(),
                );
                crate::observability::record_movement_actor_enqueue_latency(
                    enqueue_started_at.elapsed(),
                );
                let depth = self
                    .sender
                    .max_capacity()
                    .saturating_sub(self.sender.capacity());
                crate::observability::record_channel_queue_depth(MOVEMENT_ACTOR_CHANNEL, depth);
                crate::observability::record_movement_actor_queue_depth(depth);
            }
            Err(mpsc::error::TrySendError::Full(_)) => {
                crate::observability::record_channel_send_wait(
                    MOVEMENT_ACTOR_CHANNEL,
                    enqueue_started_at.elapsed(),
                );
                crate::observability::record_movement_actor_enqueue_latency(
                    enqueue_started_at.elapsed(),
                );
                crate::observability::record_channel_queue_depth(
                    MOVEMENT_ACTOR_CHANNEL,
                    self.sender.max_capacity(),
                );
                crate::observability::record_movement_actor_queue_depth(self.sender.max_capacity());
                crate::observability::record_movement_actor_enqueue_failure("full");
                anyhow::bail!("movement actor mailbox is full");
            }
            Err(mpsc::error::TrySendError::Closed(_)) => {
                crate::observability::record_channel_send_wait(
                    MOVEMENT_ACTOR_CHANNEL,
                    enqueue_started_at.elapsed(),
                );
                crate::observability::record_movement_actor_enqueue_latency(
                    enqueue_started_at.elapsed(),
                );
                crate::observability::record_movement_actor_enqueue_failure("closed");
                anyhow::bail!("movement actor mailbox is closed");
            }
        }

        reply_rx
            .await
            .map_err(|_| anyhow::anyhow!("movement actor reply channel closed"))?
    }
}

async fn run_movement_actor(
    map: Arc<Mutex<MapRuntime>>,
    mut receiver: mpsc::Receiver<MovementActorCommand>,
    max_batch_size: usize,
) {
    while let Some(first) = receiver.recv().await {
        let batch = drain_movement_batch(first, &mut receiver, max_batch_size.max(1));
        crate::observability::record_channel_queue_depth(MOVEMENT_ACTOR_CHANNEL, receiver.len());
        process_movement_batch(&map, batch).await;
    }
}

fn drain_movement_batch(
    first: MovementActorCommand,
    receiver: &mut mpsc::Receiver<MovementActorCommand>,
    max_batch_size: usize,
) -> Vec<MovementActorCommand> {
    let mut batch = vec![first];
    while batch.len() < max_batch_size {
        match receiver.try_recv() {
            Ok(command) => batch.push(command),
            Err(mpsc::error::TryRecvError::Empty)
            | Err(mpsc::error::TryRecvError::Disconnected) => break,
        }
    }
    batch
}

async fn process_movement_batch(map: &Arc<Mutex<MapRuntime>>, batch: Vec<MovementActorCommand>) {
    crate::observability::record_movement_actor_batch_size(batch.len());
    let processing_started_at = Instant::now();
    let mutex_wait_started_at = Instant::now();
    let mut map = map.lock().await;
    crate::observability::record_movement_map_mutex_wait(mutex_wait_started_at.elapsed());

    let mutex_hold_started_at = Instant::now();
    let mut replies = Vec::new();
    for command in batch {
        let MovementActorCommand::UpdatePlayerPosition(command) = command;
        crate::observability::record_channel_queue_age(
            MOVEMENT_ACTOR_CHANNEL,
            command.enqueued_at.elapsed(),
        );
        crate::observability::record_movement_actor_apply_start_latency(
            command.enqueued_at.elapsed(),
        );
        let result = map.update_player_position(
            command.character_guid,
            command.opcode,
            &command.movement,
            command.server_time,
        );
        replies.push((command.reply, command.enqueued_at, result));
    }
    crate::observability::record_movement_map_mutex_hold(mutex_hold_started_at.elapsed());
    drop(map);

    crate::observability::record_movement_actor_processing_time(processing_started_at.elapsed());

    for (reply, enqueued_at, result) in replies {
        crate::observability::record_movement_actor_reply_latency(enqueued_at.elapsed());
        let _ = reply.send(result.map(|packets| MovementUpdateOutcome::Applied { packets }));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_player_runtime(
        guid: u32,
        session_id: SessionId,
        position: WorldPosition,
    ) -> PlayerRuntime {
        let world_stats = PlayerWorldStats {
            base_health: 20,
            base_mana: 0,
            stats: [23, 20, 22, 20, 20],
            next_level_xp: 400,
        };
        PlayerRuntime {
            guid,
            account_id: Some(guid),
            controller: PlayerController::Client { session_id },
            bot_runtime: None,
            selected_target: None,
            unit_target: None,
            active_combat_target: None,
            active_combat_attack_kind: PlayerAutoAttackKind::Melee,
            active_combat_next_swing_at: None,
            ranged_auto_attack_next_shot_at: None,
            in_combat: false,
            looting: false,
            position,
            movement_flags: 0,
            client_time: 0,
            server_time: 0,
            fall_time: 0,
            last_fall_z: None,
            last_fall_time: 0,
            environment: PlayerEnvironmentRuntime::default(),
            jump: JumpInfo::default(),
            cell: cell_coord_for_position(position),
            visible_objects: HashSet::new(),
            next_sight_aggro_check_at: None,
            last_sight_aggro_check_position: None,
            last_player_visibility_refresh_position: None,
            last_creature_visibility_position: None,
            last_gameobject_visibility_position: None,
            last_player_corpse_visibility_position: None,
            visual: PlayerVisualState {
                gender: 0,
                player_bytes: 0,
                player_bytes2: 0,
                equipment_cache: None,
                guildid: None,
            },
            visible_equipment: [0; ENUM_EQUIPMENT_SLOTS],
            flags: 0,
            death_state: PlayerDeathState::Alive,
            level: 1,
            race: 1,
            class: 1,
            spirit: 20,
            gender: 0,
            base_world_stats: world_stats,
            effective_world_stats: world_stats,
            health: 20,
            max_health: 20,
            xp: 0,
            rest_bonus: 0.0,
            power1: 0,
            max_power1: 0,
            last_mana_use_at: None,
            power2: 0,
            power4: 0,
            max_power4: POWER_ENERGY_DEFAULT,
            player_bytes: 0,
            player_bytes2: 0,
            combo_target: None,
            combo_points: 0,
            stand_state: PLAYER_STAND_STATE_STAND,
            active_spells: HashSet::new(),
            inventory: Vec::new(),
            quest_statuses: HashMap::new(),
            explored_zones: [0; PLAYER_EXPLORED_ZONES_SIZE],
            active_auras: Vec::new(),
            spell_global_cooldowns_until: HashMap::new(),
            spell_cooldowns_until: HashMap::new(),
            spell_cooldown_categories: HashMap::new(),
            spell_cooldown_item_ids: HashMap::new(),
            queued_next_melee_spell: None,
            base_combat_stats: test_player_combat_stats(),
            combat_stats: test_player_combat_stats(),
        }
    }

    fn test_player_combat_stats() -> PlayerCombatStats {
        let world_stats = PlayerWorldStats {
            base_health: 20,
            base_mana: 0,
            stats: [23, 20, 22, 20, 20],
            next_level_xp: 400,
        };
        player_combat_stats_for_values(1, 1, &world_stats, &[])
    }

    #[tokio::test]
    async fn process_movement_batch_applies_older_and_newer_updates_in_order() {
        let mut runtime = MapRuntime::new(0, 0);
        runtime
            .add_player(test_player_runtime(
                7,
                SessionId(7),
                WorldPosition::new(0, -8950.0, -130.0, 83.5, 0.0),
            ))
            .expect("add mover");
        let map = Arc::new(Mutex::new(runtime));

        let (first_reply_tx, first_reply_rx) = oneshot::channel();
        let (latest_reply_tx, latest_reply_rx) = oneshot::channel();
        let batch = vec![
            MovementActorCommand::UpdatePlayerPosition(MovementUpdateCommand {
                character_guid: 7,
                opcode: WorldOpcode::MsgMoveStop as u16,
                movement: MovementInfo {
                    flags: 0,
                    client_time: 1,
                    position: WorldPosition::new(0, -8950.0, -130.0, 83.5, 0.25),
                    fall_time: 0,
                    jump: JumpInfo::default(),
                },
                server_time: 1,
                enqueued_at: Instant::now(),
                reply: first_reply_tx,
            }),
            MovementActorCommand::UpdatePlayerPosition(MovementUpdateCommand {
                character_guid: 7,
                opcode: WorldOpcode::MsgMoveStop as u16,
                movement: MovementInfo {
                    flags: 0,
                    client_time: 2,
                    position: WorldPosition::new(0, -8950.0, -130.0, 83.5, 1.5),
                    fall_time: 0,
                    jump: JumpInfo::default(),
                },
                server_time: 2,
                enqueued_at: Instant::now(),
                reply: latest_reply_tx,
            }),
        ];

        process_movement_batch(&map, batch).await;

        match first_reply_rx
            .await
            .expect("first reply")
            .expect("first result")
        {
            MovementUpdateOutcome::Applied { packets } => assert!(packets.is_empty()),
        }
        let latest = latest_reply_rx
            .await
            .expect("latest reply")
            .expect("latest result");
        match latest {
            MovementUpdateOutcome::Applied { packets } => assert!(packets.is_empty()),
        }

        let map = map.lock().await;
        let player = map.players.get(&7).expect("mover stored in map");
        assert_eq!(player.position.orientation, 1.5);
    }

    #[tokio::test]
    async fn process_movement_batch_preserves_facing_updates_alongside_latest_heartbeat() {
        let mut runtime = MapRuntime::new(0, 0);
        runtime
            .add_player(test_player_runtime(
                7,
                SessionId(7),
                WorldPosition::new(0, -8950.0, -130.0, 83.5, 0.0),
            ))
            .expect("add mover");
        runtime
            .add_player(test_player_runtime(
                8,
                SessionId(8),
                WorldPosition::new(0, -8952.0, -130.0, 83.5, 0.0),
            ))
            .expect("add observer");
        let map = Arc::new(Mutex::new(runtime));

        let (first_heartbeat_reply_tx, first_heartbeat_reply_rx) = oneshot::channel();
        let (facing_reply_tx, facing_reply_rx) = oneshot::channel();
        let (latest_heartbeat_reply_tx, latest_heartbeat_reply_rx) = oneshot::channel();
        let batch = vec![
            MovementActorCommand::UpdatePlayerPosition(MovementUpdateCommand {
                character_guid: 7,
                opcode: WorldOpcode::MsgMoveHeartbeat as u16,
                movement: MovementInfo {
                    flags: MOVEFLAG_FORWARD,
                    client_time: 1,
                    position: WorldPosition::new(0, -8949.5, -130.0, 83.5, 0.25),
                    fall_time: 0,
                    jump: JumpInfo::default(),
                },
                server_time: 1,
                enqueued_at: Instant::now(),
                reply: first_heartbeat_reply_tx,
            }),
            MovementActorCommand::UpdatePlayerPosition(MovementUpdateCommand {
                character_guid: 7,
                opcode: WorldOpcode::MsgMoveSetFacing as u16,
                movement: MovementInfo {
                    flags: MOVEFLAG_FORWARD,
                    client_time: 2,
                    position: WorldPosition::new(0, -8949.4, -130.0, 83.5, 0.75),
                    fall_time: 0,
                    jump: JumpInfo::default(),
                },
                server_time: 2,
                enqueued_at: Instant::now(),
                reply: facing_reply_tx,
            }),
            MovementActorCommand::UpdatePlayerPosition(MovementUpdateCommand {
                character_guid: 7,
                opcode: WorldOpcode::MsgMoveHeartbeat as u16,
                movement: MovementInfo {
                    flags: MOVEFLAG_FORWARD,
                    client_time: 3,
                    position: WorldPosition::new(0, -8949.0, -130.0, 83.5, 1.0),
                    fall_time: 0,
                    jump: JumpInfo::default(),
                },
                server_time: 3,
                enqueued_at: Instant::now(),
                reply: latest_heartbeat_reply_tx,
            }),
        ];

        process_movement_batch(&map, batch).await;

        let first_heartbeat_packets = match first_heartbeat_reply_rx
            .await
            .expect("first heartbeat reply")
            .expect("first heartbeat result")
        {
            MovementUpdateOutcome::Applied { packets } => packets,
        };
        assert!(first_heartbeat_packets.iter().any(|(session, packet)| {
            *session == SessionId(8) && packet.opcode == WorldOpcode::MsgMoveHeartbeat as u16
        }));

        let facing_packets = match facing_reply_rx
            .await
            .expect("facing reply")
            .expect("facing result")
        {
            MovementUpdateOutcome::Applied { packets } => packets,
        };
        assert!(facing_packets.iter().any(|(session, packet)| {
            *session == SessionId(8) && packet.opcode == WorldOpcode::MsgMoveSetFacing as u16
        }));

        let heartbeat_packets = match latest_heartbeat_reply_rx
            .await
            .expect("latest heartbeat reply")
            .expect("latest heartbeat result")
        {
            MovementUpdateOutcome::Applied { packets } => packets,
        };
        assert!(heartbeat_packets.iter().any(|(session, packet)| {
            *session == SessionId(8) && packet.opcode == WorldOpcode::MsgMoveHeartbeat as u16
        }));

        let map = map.lock().await;
        let player = map.players.get(&7).expect("mover stored in map");
        assert_eq!(player.position.x, -8949.0);
        assert_eq!(player.position.orientation, 1.0);
    }
}
