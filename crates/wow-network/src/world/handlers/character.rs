use super::*;

pub(in crate::world) async fn dispatch_character_packet(
    ctx: &mut WorldPacketDispatchContext<'_>,
    packet: &packets::ParsedWorldClientPacket,
) -> anyhow::Result<()> {
    match packet {
        packets::ParsedWorldClientPacket::CharCreate(_) => {
            handle_char_create(
                &mut *ctx.stream,
                ctx.login_db_pool,
                ctx.character_db_pool,
                ctx.world_db_pool,
                ctx.account_id,
                packet.char_create()?,
                &mut *ctx.header_crypto,
            )
            .await
        }
        packets::ParsedWorldClientPacket::CharEnum(_) => {
            let _ = packet.char_enum()?;
            let characters =
                wow_db::get_character_enum_entries(ctx.character_db_pool, ctx.account_id).await?;
            info!(
                account = %ctx.account_name,
                count = characters.len(),
                "Sending character enum"
            );
            send_char_enum(&mut *ctx.stream, &characters, Some(&mut *ctx.header_crypto)).await
        }
        packets::ParsedWorldClientPacket::CharDelete(_) => {
            handle_char_delete(
                &mut *ctx.stream,
                ctx.login_db_pool,
                ctx.character_db_pool,
                ctx.account_id,
                packet.char_delete()?,
                &mut *ctx.header_crypto,
                ctx.runtime_state,
            )
            .await
        }
        packets::ParsedWorldClientPacket::PlayerLogin(_) => {
            handle_player_login(
                &mut *ctx.stream,
                PlayerLoginDeps {
                    character_db_pool: ctx.character_db_pool,
                    world_db_pool: ctx.world_db_pool,
                    object_mgr: ctx.runtime_state.object_mgr.as_ref(),
                    online_characters: &ctx.runtime_state.online_characters,
                    maps: &ctx.runtime_state.maps,
                    sessions: &ctx.runtime_state.sessions,
                    parties: &ctx.runtime_state.parties,
                    session_id: ctx.session_id,
                },
                ctx.account_id,
                packet.player_login()?,
                &mut *ctx.header_crypto,
                &mut *ctx.session,
            )
            .await
        }
        other => anyhow::bail!("character router received opcode 0x{:04X}", other.opcode()),
    }
}
