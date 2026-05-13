use super::*;

pub(in crate::world) async fn dispatch_social_packet(
    ctx: &mut WorldPacketDispatchContext<'_>,
    packet: &packets::ParsedWorldClientPacket,
) -> anyhow::Result<()> {
    match packet {
        packets::ParsedWorldClientPacket::GroupInvite(_) => {
            handle_group_invite(
                &mut *ctx.stream,
                &ctx.runtime_state.parties,
                &ctx.runtime_state.sessions,
                packet.group_invite()?,
                &*ctx.session,
                &mut *ctx.header_crypto,
            )
            .await
        }
        packets::ParsedWorldClientPacket::GroupCancel(_)
        | packets::ParsedWorldClientPacket::GroupDecline(_) => {
            handle_group_decline(
                &ctx.runtime_state.parties,
                &ctx.runtime_state.sessions,
                &*ctx.session,
            )
            .await
        }
        packets::ParsedWorldClientPacket::GroupAccept(_) => {
            handle_group_accept(
                &mut *ctx.stream,
                &ctx.runtime_state.parties,
                &ctx.runtime_state.sessions,
                &*ctx.session,
                &mut *ctx.header_crypto,
            )
            .await
        }
        packets::ParsedWorldClientPacket::GroupUninvite(_) => {
            handle_group_uninvite(
                &mut *ctx.stream,
                &ctx.runtime_state.parties,
                &ctx.runtime_state.sessions,
                packet.group_uninvite()?,
                &*ctx.session,
                &mut *ctx.header_crypto,
            )
            .await
        }
        packets::ParsedWorldClientPacket::GroupUninviteGuid(_) => {
            handle_group_uninvite_guid(
                &mut *ctx.stream,
                &ctx.runtime_state.parties,
                &ctx.runtime_state.sessions,
                packet.group_uninvite_guid()?,
                &*ctx.session,
                &mut *ctx.header_crypto,
            )
            .await
        }
        packets::ParsedWorldClientPacket::GroupSetLeader(_) => {
            handle_group_set_leader(
                &ctx.runtime_state.parties,
                &ctx.runtime_state.sessions,
                packet.group_set_leader()?,
                &*ctx.session,
            )
            .await
        }
        packets::ParsedWorldClientPacket::GroupRaidConvert(_) => {
            handle_group_raid_convert(
                &mut *ctx.stream,
                &ctx.runtime_state.parties,
                &ctx.runtime_state.sessions,
                &*ctx.session,
                &mut *ctx.header_crypto,
            )
            .await
        }
        packets::ParsedWorldClientPacket::GroupChangeSubGroup(_) => {
            handle_group_change_subgroup(
                &ctx.runtime_state.parties,
                &ctx.runtime_state.sessions,
                packet.group_change_subgroup()?,
                &*ctx.session,
            )
            .await
        }
        packets::ParsedWorldClientPacket::GroupAssistantLeader(_) => {
            handle_group_assistant_leader(
                &ctx.runtime_state.parties,
                &ctx.runtime_state.sessions,
                packet.group_assistant_leader()?,
                &*ctx.session,
            )
            .await
        }
        packets::ParsedWorldClientPacket::GroupDisband(_) => {
            handle_group_disband(
                &mut *ctx.stream,
                &ctx.runtime_state.parties,
                &ctx.runtime_state.sessions,
                &*ctx.session,
                &mut *ctx.header_crypto,
            )
            .await
        }
        packets::ParsedWorldClientPacket::RequestPartyMemberStats(_) => {
            handle_request_party_member_stats(
                &mut *ctx.stream,
                &ctx.runtime_state.maps,
                packet.request_party_member_stats()?,
                &*ctx.session,
                &mut *ctx.header_crypto,
            )
            .await
        }
        packets::ParsedWorldClientPacket::LootMethod(_) => {
            handle_loot_method(
                &ctx.runtime_state.parties,
                &ctx.runtime_state.sessions,
                packet.loot_method()?,
                &*ctx.session,
            )
            .await
        }
        other => anyhow::bail!("social router received opcode 0x{:04X}", other.opcode()),
    }
}
