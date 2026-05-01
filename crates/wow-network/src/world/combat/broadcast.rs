#[derive(Clone, Copy)]
struct CreatureCombatBroadcast<'a> {
    shared_world: SharedWorldDeps<'a>,
    map_id: u32,
    player: ObjectGuid,
}

async fn broadcast_db_creature_packet(
    broadcast: CreatureCombatBroadcast<'_>,
    session: &WorldSessionState,
    creature_guid: ObjectGuid,
    opcode: u16,
    body: Vec<u8>,
) {
    let Some(creature) = session.db_creatures.get(&creature_guid.raw()).cloned() else {
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

async fn broadcast_db_creature_snapshot_packet(
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
