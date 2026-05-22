use super::*;

pub(in crate::world) async fn dispatch_item_query_packet(
    ctx: &mut WorldPacketDispatchContext<'_>,
    packet: &packets::ParsedWorldClientPacket,
) -> anyhow::Result<()> {
    match packet {
        packets::ParsedWorldClientPacket::ItemQuerySingle(_) => {
            handle_item_query_single(
                &mut *ctx.stream,
                ctx.world_db_pool,
                packet.item_query_single()?,
                &mut *ctx.header_crypto,
            )
            .await
        }
        packets::ParsedWorldClientPacket::ItemNameQuery(_) => {
            handle_item_name_query(
                &mut *ctx.stream,
                ctx.world_db_pool,
                packet.item_name_query()?,
                &mut *ctx.header_crypto,
            )
            .await
        }
        packets::ParsedWorldClientPacket::PageTextQuery(_) => {
            handle_page_text_query(
                &mut *ctx.stream,
                ctx.world_db_pool,
                packet.page_text_query()?,
                &mut *ctx.header_crypto,
            )
            .await
        }
        other => anyhow::bail!("item query router received opcode 0x{:04X}", other.opcode()),
    }
}

pub(in crate::world) async fn dispatch_misc_packet(
    ctx: &mut WorldPacketDispatchContext<'_>,
    packet: &packets::ParsedWorldClientPacket,
) -> anyhow::Result<()> {
    match packet {
        packets::ParsedWorldClientPacket::QueryTime(_) => {
            handle_query_time(&mut *ctx.stream, &mut *ctx.header_crypto).await
        }
        packets::ParsedWorldClientPacket::RequestAccountData(_) => {
            handle_request_account_data(
                &mut *ctx.stream,
                packet.request_account_data()?,
                &*ctx.session,
                &mut *ctx.header_crypto,
            )
            .await
        }
        packets::ParsedWorldClientPacket::UpdateAccountData(_) => {
            handle_update_account_data(
                ctx.character_db_pool,
                ctx.account_id,
                packet.update_account_data()?,
                &mut *ctx.session,
            )
            .await
        }
        packets::ParsedWorldClientPacket::TutorialFlag(_) => {
            handle_tutorial_flag(
                ctx.character_db_pool,
                ctx.account_id,
                packet.tutorial_flag()?,
            )
            .await
        }
        packets::ParsedWorldClientPacket::TutorialClear(_) => {
            handle_tutorial_clear(ctx.character_db_pool, ctx.account_id).await
        }
        packets::ParsedWorldClientPacket::TutorialReset(_) => {
            handle_tutorial_reset(ctx.character_db_pool, ctx.account_id).await
        }
        packets::ParsedWorldClientPacket::StandStateChange(_) => {
            handle_stand_state_change(
                &mut *ctx.stream,
                SharedWorldDeps {
                    object_mgr: ctx.runtime_state.object_mgr.as_ref(),
                    maps: &ctx.runtime_state.maps,
                    sessions: &ctx.runtime_state.sessions,
                },
                packet.stand_state_change()?,
                &mut *ctx.session,
                &mut *ctx.header_crypto,
            )
            .await
        }
        packets::ParsedWorldClientPacket::SetActionButton(_) => {
            handle_set_action_button(
                ctx.character_db_pool,
                packet.set_action_button()?,
                &*ctx.session,
            )
            .await
        }
        packets::ParsedWorldClientPacket::SetSelection(_) => {
            handle_set_selection(
                SharedWorldDeps {
                    object_mgr: ctx.runtime_state.object_mgr.as_ref(),
                    maps: &ctx.runtime_state.maps,
                    sessions: &ctx.runtime_state.sessions,
                },
                packet.set_selection()?,
                &mut *ctx.session,
            )
            .await
        }
        packets::ParsedWorldClientPacket::SetTargetObsolete(_) => {
            handle_set_target_obsolete(
                SharedWorldDeps {
                    object_mgr: ctx.runtime_state.object_mgr.as_ref(),
                    maps: &ctx.runtime_state.maps,
                    sessions: &ctx.runtime_state.sessions,
                },
                packet.set_target_obsolete()?,
                &*ctx.session,
            )
            .await
        }
        packets::ParsedWorldClientPacket::GmTicketGetTicket(_) => {
            handle_gmticket_getticket(&mut *ctx.stream, &mut *ctx.header_crypto).await
        }
        packets::ParsedWorldClientPacket::QueryNextMailTime(_) => {
            handle_query_next_mail_time(
                &mut *ctx.stream,
                ctx.character_db_pool,
                &*ctx.session,
                &mut *ctx.header_crypto,
            )
            .await
        }
        packets::ParsedWorldClientPacket::AreaTrigger(_) => {
            handle_area_trigger(
                &mut *ctx.stream,
                ctx.world_db_pool,
                SharedWorldDeps {
                    object_mgr: ctx.runtime_state.object_mgr.as_ref(),
                    maps: &ctx.runtime_state.maps,
                    sessions: &ctx.runtime_state.sessions,
                },
                packet.area_trigger()?,
                &mut *ctx.session,
                &mut *ctx.header_crypto,
            )
            .await
        }
        packets::ParsedWorldClientPacket::ZoneUpdate(_) => {
            handle_zone_update(
                &mut *ctx.stream,
                &ctx.runtime_state.world_data_files,
                &ctx.runtime_state.maps,
                packet.zone_update()?,
                &mut *ctx.session,
                &mut *ctx.header_crypto,
            )
            .await
        }
        packets::ParsedWorldClientPacket::LogoutRequest(_) => {
            handle_logout_request(
                &mut *ctx.stream,
                LogoutDeps {
                    character_db_pool: ctx.character_db_pool,
                    online_characters: &ctx.runtime_state.online_characters,
                    maps: &ctx.runtime_state.maps,
                    sessions: &ctx.runtime_state.sessions,
                    account_id: ctx.account_id,
                    session_id: ctx.session_id,
                },
                &mut *ctx.header_crypto,
                &mut *ctx.session,
            )
            .await
        }
        packets::ParsedWorldClientPacket::LogoutCancel(_) => {
            handle_logout_cancel(
                &mut *ctx.stream,
                &ctx.runtime_state.maps,
                &ctx.runtime_state.sessions,
                &mut *ctx.header_crypto,
                &mut *ctx.session,
            )
            .await
        }
        packets::ParsedWorldClientPacket::PlayerLogout(_) => {
            info!("Received client-side player logout notification");
            Ok(())
        }
        other => anyhow::bail!("misc router received opcode 0x{:04X}", other.opcode()),
    }
}

pub(in crate::world) async fn handle_area_trigger(
    stream: &mut WorldPacketSink,
    world_db_pool: &MySqlPool,
    shared_world: SharedWorldDeps<'_>,
    request: wow_proto::AreaTriggerRequest,
    session: &mut WorldSessionState,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    if session.character.active_character.is_none() {
        return Ok(());
    }
    if !wow_db::is_tavern_area_trigger(world_db_pool, request.trigger_id).await? {
        return Ok(());
    }
    if session.rest.rest_type == RestType::InCity {
        return Ok(());
    }
    set_rest_type(
        stream,
        shared_world.maps,
        session,
        header_crypto,
        RestType::InTavern,
        Some(request.trigger_id),
    )
    .await
}

pub(in crate::world) async fn handle_zone_update(
    stream: &mut WorldPacketSink,
    world_data_files: &Arc<WorldDataFiles>,
    maps: &Arc<MapRuntimeManager>,
    request: wow_proto::ZoneUpdateRequest,
    session: &mut WorldSessionState,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    let Some(area) = world_data_files.area_tables.entry(request.zone_id) else {
        return Ok(());
    };
    if area.flags & AREA_FLAG_CAPITAL != 0 {
        return set_rest_type(stream, maps, session, header_crypto, RestType::InCity, None).await;
    }
    if session.character.player_flags & PLAYER_FLAGS_RESTING != 0
        && session.rest.rest_type != RestType::InTavern
    {
        return set_rest_type(stream, maps, session, header_crypto, RestType::No, None).await;
    }
    Ok(())
}
