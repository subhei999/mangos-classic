use super::*;

pub(in crate::world) async fn dispatch_movement_packet(
    ctx: &mut WorldPacketDispatchContext<'_>,
    packet: &packets::ParsedWorldClientPacket,
    body: &[u8],
) -> anyhow::Result<()> {
    if matches!(packet, packets::ParsedWorldClientPacket::SetActiveMover(_)) {
        return handle_set_active_mover(packet.set_active_mover()?, &*ctx.session);
    }
    if matches!(packet, packets::ParsedWorldClientPacket::MoveTeleportAck(_)) {
        return handle_move_teleport_ack(packet.move_teleport_ack()?, &*ctx.session);
    }

    let opcode = packet.opcode();
    handle_movement(
        &mut *ctx.stream,
        MovementDeps {
            character_db_pool: ctx.character_db_pool,
            world_db_pool: ctx.world_db_pool,
            object_mgr: ctx.runtime_state.object_mgr.as_ref(),
            maps: &ctx.runtime_state.maps,
            sessions: &ctx.runtime_state.sessions,
        },
        opcode,
        body,
        &mut *ctx.session,
        &mut *ctx.header_crypto,
    )
    .await
}

pub(in crate::world) fn handle_move_teleport_ack(
    request: wow_proto::MoveTeleportAckRequest,
    session: &WorldSessionState,
) -> anyhow::Result<()> {
    let Some(character) = session.character.active_character.as_ref() else {
        debug!("Ignoring teleport ack before character login");
        return Ok(());
    };
    let expected_player = ObjectGuid::new(HighGuid::Player, 0, character.guid);
    if request.player != expected_player {
        warn!(
            expected = format_args!("0x{:016X}", expected_player.raw()),
            received = format_args!("0x{:016X}", request.player.raw()),
            "Ignoring teleport ack for non-active mover"
        );
        return Ok(());
    }
    debug!(
        counter = request.counter,
        client_time = request.client_time,
        guid = character.guid,
        "Received near teleport ack"
    );
    Ok(())
}
