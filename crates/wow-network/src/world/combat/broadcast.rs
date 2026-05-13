use super::*;

#[derive(Clone, Copy)]
pub(in crate::world) struct CreatureCombatBroadcast<'a> {
    pub(in crate::world) shared_world: SharedWorldDeps<'a>,
    pub(in crate::world) map_id: u32,
    pub(in crate::world) player: ObjectGuid,
}

pub(in crate::world) async fn broadcast_db_creature_packet(
    broadcast: CreatureCombatBroadcast<'_>,
    _session: &WorldSessionState,
    creature_guid: ObjectGuid,
    opcode: u16,
    body: Vec<u8>,
) {
    let Some(creature) = broadcast
        .shared_world
        .maps
        .db_creature_snapshot(broadcast.map_id, creature_guid)
        .await
    else {
        return;
    };
    let packets = broadcast
        .shared_world
        .maps
        .update_db_creature_snapshot_and_broadcast(
            broadcast.map_id,
            creature,
            Some(broadcast.player.counter()),
            OutboundWorldPacket { opcode, body },
        )
        .await;
    broadcast.shared_world.sessions.dispatch(packets).await;
}

pub(in crate::world) async fn broadcast_db_creature_snapshot_packet(
    broadcast: CreatureCombatBroadcast<'_>,
    creature: DbCreatureRuntime,
    opcode: u16,
    body: Vec<u8>,
) {
    let packets = broadcast
        .shared_world
        .maps
        .update_db_creature_snapshot_and_broadcast(
            broadcast.map_id,
            creature,
            Some(broadcast.player.counter()),
            OutboundWorldPacket { opcode, body },
        )
        .await;
    broadcast.shared_world.sessions.dispatch(packets).await;
}
