use super::*;
use tokio::sync::{mpsc, oneshot};

const DEFAULT_MOVEMENT_ACTOR_QUEUE_CAPACITY: usize = 1024;
const DEFAULT_MOVEMENT_ACTOR_MAX_BATCH_SIZE: usize = 64;

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
    pub(in crate::world) fn with_enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    #[cfg(test)]
    pub(in crate::world) fn for_test(
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
    pub(in crate::world) reply:
        oneshot::Sender<anyhow::Result<Vec<(SessionId, OutboundWorldPacket)>>>,
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
    ) -> anyhow::Result<Vec<(SessionId, OutboundWorldPacket)>> {
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
                crate::observability::record_movement_actor_enqueue_latency(
                    enqueue_started_at.elapsed(),
                );
                crate::observability::record_movement_actor_queue_depth(
                    self.sender
                        .max_capacity()
                        .saturating_sub(self.sender.capacity()),
                );
            }
            Err(mpsc::error::TrySendError::Full(_)) => {
                crate::observability::record_movement_actor_enqueue_latency(
                    enqueue_started_at.elapsed(),
                );
                crate::observability::record_movement_actor_queue_depth(self.sender.max_capacity());
                crate::observability::record_movement_actor_enqueue_failure("full");
                anyhow::bail!("movement actor mailbox is full");
            }
            Err(mpsc::error::TrySendError::Closed(_)) => {
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

#[derive(Debug)]
struct CoalescedMovementBatch {
    latest: Vec<MovementUpdateCommand>,
    superseded: Vec<MovementUpdateCommand>,
}

async fn run_movement_actor(
    map: Arc<Mutex<MapRuntime>>,
    mut receiver: mpsc::Receiver<MovementActorCommand>,
    max_batch_size: usize,
) {
    while let Some(first) = receiver.recv().await {
        let batch = drain_movement_batch(first, &mut receiver, max_batch_size.max(1));
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

fn coalesce_movement_batch(batch: Vec<MovementActorCommand>) -> CoalescedMovementBatch {
    let mut latest = Vec::new();
    let mut latest_by_guid = HashMap::new();
    let mut superseded = Vec::new();

    for command in batch {
        let MovementActorCommand::UpdatePlayerPosition(command) = command;
        if let Some(index) = latest_by_guid.get(&command.character_guid).copied() {
            let replaced = std::mem::replace(&mut latest[index], command);
            superseded.push(replaced);
            continue;
        }
        latest_by_guid.insert(command.character_guid, latest.len());
        latest.push(command);
    }

    CoalescedMovementBatch { latest, superseded }
}

async fn process_movement_batch(map: &Arc<Mutex<MapRuntime>>, batch: Vec<MovementActorCommand>) {
    crate::observability::record_movement_actor_batch_size(batch.len());

    let CoalescedMovementBatch { latest, superseded } = coalesce_movement_batch(batch);
    let processing_started_at = Instant::now();
    let mutex_wait_started_at = Instant::now();
    let mut map = map.lock().await;
    crate::observability::record_movement_map_mutex_wait(mutex_wait_started_at.elapsed());

    let mutex_hold_started_at = Instant::now();
    let mut replies = Vec::with_capacity(latest.len());
    for command in latest {
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

    for command in superseded {
        crate::observability::record_movement_actor_reply_latency(command.enqueued_at.elapsed());
        let _ = command.reply.send(Ok(Vec::new()));
    }

    for (reply, enqueued_at, result) in replies {
        crate::observability::record_movement_actor_reply_latency(enqueued_at.elapsed());
        let _ = reply.send(result);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn movement(character_guid: u32, orientation: f32) -> MovementActorCommand {
        let (reply, _rx) = oneshot::channel();
        MovementActorCommand::UpdatePlayerPosition(MovementUpdateCommand {
            character_guid,
            opcode: MSG_MOVE_STOP as u16,
            movement: MovementInfo {
                flags: 0,
                client_time: character_guid,
                position: WorldPosition::new(0, character_guid as f32, 0.0, 0.0, orientation),
                fall_time: 0,
                jump: JumpInfo::default(),
            },
            server_time: character_guid,
            enqueued_at: Instant::now(),
            reply,
        })
    }

    #[test]
    fn coalesce_movement_batch_keeps_latest_command_per_character() {
        let batch = vec![movement(7, 0.25), movement(8, 0.5), movement(7, 1.5)];

        let coalesced = coalesce_movement_batch(batch);

        assert_eq!(coalesced.latest.len(), 2);
        assert_eq!(coalesced.superseded.len(), 1);
        let command = coalesced
            .latest
            .into_iter()
            .find(|command| command.character_guid == 7)
            .expect("latest command for player 7");
        assert!((command.movement.position.orientation - 1.5).abs() < f32::EPSILON);
    }
}
