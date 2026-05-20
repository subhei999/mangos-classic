use super::*;
use wow_proto::world::WorldOpcode;
use wow_proto::ServerWorldPacket;

pub(in crate::world) const BANKER_INTERACTION_DISTANCE_YARDS: f32 = 5.0;

pub(in crate::world) async fn dispatch_bank_packet(
    ctx: &mut WorldPacketDispatchContext<'_>,
    packet: &packets::ParsedWorldClientPacket,
) -> anyhow::Result<()> {
    match packet {
        packets::ParsedWorldClientPacket::BankerActivate(_) => {
            handle_banker_activate(
                &mut *ctx.stream,
                &ctx.runtime_state.maps,
                packet.banker_activate()?,
                &*ctx.session,
                &mut *ctx.header_crypto,
            )
            .await
        }
        packets::ParsedWorldClientPacket::BuyBankSlot(_) => {
            handle_buy_bank_slot(
                &mut *ctx.stream,
                ctx.character_db_pool,
                &ctx.runtime_state.world_data_files,
                &ctx.runtime_state.maps,
                packet.buy_bank_slot()?,
                &mut *ctx.session,
                &mut *ctx.header_crypto,
            )
            .await
        }
        other => anyhow::bail!("bank router received opcode 0x{:04X}", other.opcode()),
    }
}

pub(in crate::world) async fn handle_banker_activate(
    stream: &mut WorldPacketSink,
    maps: &Arc<MapRuntimeManager>,
    request: wow_proto::BankerActivateRequest,
    session: &WorldSessionState,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    let banker = ObjectGuid::from_raw(request.banker_raw_guid);
    if !check_banker_access(maps, session, banker).await {
        warn!(
            banker = format_args!("0x{:016X}", banker.raw()),
            "Ignoring bank open request for inaccessible non-banker"
        );
        return Ok(());
    }
    send_packet(
        stream,
        WorldOpcode::SmsgShowBank as u16,
        &wow_proto::SmsgShowBankResponse { banker }.body(),
        Some(header_crypto),
    )
    .await
}

pub(in crate::world) async fn handle_buy_bank_slot(
    stream: &mut WorldPacketSink,
    character_db_pool: &MySqlPool,
    world_data_files: &WorldDataFiles,
    maps: &Arc<MapRuntimeManager>,
    request: wow_proto::BuyBankSlotRequest,
    session: &mut WorldSessionState,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    let Some(character) = session.character.active_character.as_ref() else {
        warn!("Ignoring bank slot purchase before character login");
        return Ok(());
    };
    let character_guid = character.guid;
    let banker = ObjectGuid::from_raw(request.banker_raw_guid);
    if !check_banker_access(maps, session, banker).await {
        return send_buy_bank_slot_result(stream, ERR_BANKSLOT_NOTBANKER, header_crypto).await;
    }

    let current_count = bank_bag_slot_count(session);
    let next_slot = u32::from(current_count) + 1;
    let Some(price) = world_data_files
        .bank_bag_slot_prices
        .get(&next_slot)
        .copied()
    else {
        return send_buy_bank_slot_result(stream, ERR_BANKSLOT_FAILED_TOO_MANY, header_crypto)
            .await;
    };
    let old_player_bytes2 = session
        .character
        .player_visual
        .as_ref()
        .map(|visual| visual.player_bytes2)
        .unwrap_or(0);
    let new_player_bytes2 = with_bank_bag_slot_count(old_player_bytes2, current_count + 1);
    let Some(new_money) = wow_db::purchase_character_bank_slot(
        character_db_pool,
        character_guid,
        price,
        new_player_bytes2,
    )
    .await?
    else {
        return send_buy_bank_slot_result(stream, ERR_BANKSLOT_INSUFFICIENT_FUNDS, header_crypto)
            .await;
    };
    if let Some(visual) = session.character.player_visual.as_mut() {
        visual.player_bytes2 = new_player_bytes2;
    }

    send_buy_bank_slot_result(stream, ERR_BANKSLOT_OK, &mut *header_crypto).await?;
    send_packet(
        stream,
        WorldOpcode::SmsgUpdateObject as u16,
        &build_player_money_update_body(character_guid, new_money)?,
        Some(&mut *header_crypto),
    )
    .await?;
    send_packet(
        stream,
        WorldOpcode::SmsgUpdateObject as u16,
        &build_player_bytes2_update_body(character_guid, new_player_bytes2)?,
        Some(header_crypto),
    )
    .await
}

pub(in crate::world) async fn send_buy_bank_slot_result(
    stream: &mut WorldPacketSink,
    result: u32,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    send_packet(
        stream,
        WorldOpcode::SmsgBuyBankSlotResult as u16,
        &wow_proto::SmsgBuyBankSlotResultResponse { result }.body(),
        Some(header_crypto),
    )
    .await
}

pub(in crate::world) async fn check_banker_access(
    maps: &Arc<MapRuntimeManager>,
    session: &WorldSessionState,
    banker: ObjectGuid,
) -> bool {
    let Some(character) = session.character.active_character.as_ref() else {
        return false;
    };
    if banker == ObjectGuid::new(HighGuid::Player, 0, character.guid) {
        return session.account.gm_mode;
    }
    if !banker.is_creature() {
        return false;
    }
    let Some(creature) = maps
        .db_creature_snapshot(character.position.map_id, banker)
        .await
    else {
        return false;
    };
    creature.spawn.template.npc_flags & UNIT_NPC_FLAG_BANKER != 0
        && is_position_inside_radius(
            creature.current_position,
            character.position,
            BANKER_INTERACTION_DISTANCE_YARDS,
        )
}
