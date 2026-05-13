use super::*;

pub(in crate::world) async fn dispatch_combat_packet(
    ctx: &mut WorldPacketDispatchContext<'_>,
    packet: &packets::ParsedWorldClientPacket,
) -> anyhow::Result<()> {
    match packet {
        packets::ParsedWorldClientPacket::CastSpell(_) => {
            handle_cast_spell(
                &mut *ctx.stream,
                SpellCastDeps {
                    character_db_pool: ctx.character_db_pool,
                    world_db_pool: ctx.world_db_pool,
                    account_id: ctx.account_id,
                    shared_world: SharedWorldDeps {
                        object_mgr: ctx.runtime_state.object_mgr.as_ref(),
                        maps: &ctx.runtime_state.maps,
                        sessions: &ctx.runtime_state.sessions,
                    },
                    parties: ctx.runtime_state.parties.as_ref(),
                },
                packet.cast_spell()?,
                &mut *ctx.session,
                &mut *ctx.header_crypto,
            )
            .await
        }
        packets::ParsedWorldClientPacket::UseItem(_) => {
            handle_use_item(
                &mut *ctx.stream,
                SpellCastDeps {
                    character_db_pool: ctx.character_db_pool,
                    world_db_pool: ctx.world_db_pool,
                    account_id: ctx.account_id,
                    shared_world: SharedWorldDeps {
                        object_mgr: ctx.runtime_state.object_mgr.as_ref(),
                        maps: &ctx.runtime_state.maps,
                        sessions: &ctx.runtime_state.sessions,
                    },
                    parties: ctx.runtime_state.parties.as_ref(),
                },
                packet.use_item()?,
                &mut *ctx.session,
                &mut *ctx.header_crypto,
            )
            .await
        }
        packets::ParsedWorldClientPacket::CancelCast(_) => {
            if !cancel_pending_player_spell_cast(
                &mut *ctx.stream,
                ctx.runtime_state.maps.as_ref(),
                ctx.runtime_state.sessions.as_ref(),
                &mut *ctx.session,
                SPELL_FAILED_INTERRUPTED,
                &mut *ctx.header_crypto,
            )
            .await?
            {
                debug!(
                    opcode = expected_noop_opcode_name(packet.opcode()),
                    "Ignoring spell cancel opcode with no pending spell cast"
                );
            }
            Ok(())
        }
        packets::ParsedWorldClientPacket::CancelAutoRepeatSpell(_) => {
            if let Some(character) = ctx.session.character.active_character.as_ref() {
                let map_id = character.position.map_id;
                let character_guid = character.guid;
                if let Some(snapshot) = ctx
                    .runtime_state
                    .maps
                    .player_runtime_snapshot(map_id, character_guid)
                    .await
                {
                    if matches!(
                        snapshot.active_combat_attack_kind,
                        PlayerAutoAttackKind::Ranged { .. }
                    ) {
                        let transitioned = if let Some(target) = snapshot.active_combat_target {
                            try_transition_ranged_auto_repeat_to_melee(
                                &mut *ctx.stream,
                                SharedWorldDeps {
                                    object_mgr: ctx.runtime_state.object_mgr.as_ref(),
                                    maps: &ctx.runtime_state.maps,
                                    sessions: &ctx.runtime_state.sessions,
                                },
                                &mut *ctx.session,
                                &mut *ctx.header_crypto,
                                target,
                            )
                            .await?
                        } else {
                            false
                        };
                        if !transitioned {
                            ctx.runtime_state
                                .maps
                                .set_player_auto_attack(map_id, character_guid, None, None)
                                .await;
                            mirror_session_player_auto_attack(ctx.session, None, None);
                        }
                    }
                }
            }
            if !cancel_pending_player_spell_cast(
                &mut *ctx.stream,
                ctx.runtime_state.maps.as_ref(),
                ctx.runtime_state.sessions.as_ref(),
                &mut *ctx.session,
                SPELL_FAILED_INTERRUPTED,
                &mut *ctx.header_crypto,
            )
            .await?
            {
                debug!(
                    opcode = expected_noop_opcode_name(packet.opcode()),
                    "Ignoring auto-repeat cancel opcode with no pending spell cast"
                );
            }
            Ok(())
        }
        packets::ParsedWorldClientPacket::AttackSwing(_) => {
            handle_attack_swing(
                &mut *ctx.stream,
                SharedWorldDeps {
                    object_mgr: ctx.runtime_state.object_mgr.as_ref(),
                    maps: &ctx.runtime_state.maps,
                    sessions: &ctx.runtime_state.sessions,
                },
                &ctx.runtime_state.parties,
                packet.attack_swing()?,
                &mut *ctx.session,
                &mut *ctx.header_crypto,
            )
            .await
        }
        packets::ParsedWorldClientPacket::AttackStop(_) => {
            let _ = packet.attack_stop()?;
            handle_attack_stop(
                &mut *ctx.stream,
                SharedWorldDeps {
                    object_mgr: ctx.runtime_state.object_mgr.as_ref(),
                    maps: &ctx.runtime_state.maps,
                    sessions: &ctx.runtime_state.sessions,
                },
                &mut *ctx.session,
                &mut *ctx.header_crypto,
            )
            .await
        }
        other => anyhow::bail!("combat router received opcode 0x{:04X}", other.opcode()),
    }
}
