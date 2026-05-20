use super::*;

#[allow(clippy::too_many_arguments)]
pub(in crate::world) async fn apply_player_direct_heal_effect(
    stream: &mut WorldPacketSink,

    deps: SpellCastDeps<'_>,

    session: &mut WorldSessionState,

    caster: ObjectGuid,

    map_id: u32,

    spell_info: &SpellInfo<'_>,

    value_context: SpellEffectValueContext,

    targets: &SpellCastTargets,

    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    let heal = spell_direct_heal(spell_info, value_context);

    if heal == 0 {
        return Ok(());
    }

    let Some(target) = targets.unit_target.filter(|target| target.is_player()) else {
        return Ok(());
    };

    let Some(event) = deps
        .shared_world
        .maps
        .apply_player_heal(map_id, target.counter(), heal)
        .await?
    else {
        return Ok(());
    };

    send_player_spell_log_to_target_set(
        stream,
        deps.shared_world,
        character_guid_from_caster(caster),
        event.healed_character_guid,
        event.direct_session_id,
        &event.observer_packets,
        OutboundWorldPacket {
            opcode: WorldOpcode::SmsgSpellHealLog as u16,

            body: build_spell_heal_log_body(
                caster,
                target,
                spell_info.template.id,
                event.amount_healed,
                false,
            )?,
        },
        header_crypto,
    )
    .await?;

    if event.healed_character_guid == caster.counter() {
        session.character.player_health = event.health;

        for packet in event.direct_packets {
            send_packet(
                stream,
                packet.opcode,
                &packet.body,
                Some(&mut *header_crypto),
            )
            .await?;
        }
    } else {
        deps.shared_world
            .sessions
            .dispatch(
                event
                    .direct_packets
                    .into_iter()
                    .map(|packet| (event.direct_session_id, packet))
                    .collect(),
            )
            .await;
    }

    deps.shared_world
        .sessions
        .dispatch(event.observer_packets)
        .await;

    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub(in crate::world) async fn send_player_spell_log_to_target_set(
    stream: &mut WorldPacketSink,

    shared_world: SharedWorldDeps<'_>,

    caster_character_guid: u32,

    target_character_guid: u32,

    target_session_id: SessionId,

    observer_packets: &[(SessionId, OutboundWorldPacket)],

    packet: OutboundWorldPacket,

    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    send_packet(
        stream,
        packet.opcode,
        &packet.body,
        Some(&mut *header_crypto),
    )
    .await?;

    let caster_session_id = shared_world
        .sessions
        .session_for_character(caster_character_guid)
        .await;

    let mut dispatch = Vec::new();

    let mut seen = HashSet::new();

    if Some(target_session_id) != caster_session_id
        || target_character_guid != caster_character_guid
    {
        seen.insert(target_session_id);

        dispatch.push((target_session_id, packet.clone()));
    }

    for (session_id, _) in observer_packets {
        if Some(*session_id) == caster_session_id || !seen.insert(*session_id) {
            continue;
        }

        dispatch.push((*session_id, packet.clone()));
    }

    shared_world.sessions.dispatch(dispatch).await;

    Ok(())
}

pub(in crate::world) async fn send_or_dispatch_player_aura_event(
    stream: &mut WorldPacketSink,

    shared_world: SharedWorldDeps<'_>,

    current_character_guid: u32,

    target_character_guid: u32,

    event: PlayerAuraUpdateEvent,

    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    let current_session_id = shared_world
        .sessions
        .session_for_character(current_character_guid)
        .await;

    let mut dispatch = Vec::new();

    if target_character_guid == current_character_guid {
        for packet in event.direct_packets {
            send_packet(
                stream,
                packet.opcode,
                &packet.body,
                Some(&mut *header_crypto),
            )
            .await?;
        }
    } else if let Some(target_session_id) = shared_world
        .sessions
        .session_for_character(target_character_guid)
        .await
    {
        dispatch.extend(
            event
                .direct_packets
                .into_iter()
                .map(|packet| (target_session_id, packet)),
        );
    }

    for (session_id, packet) in event.observer_packets {
        if Some(session_id) == current_session_id {
            send_packet(
                stream,
                packet.opcode,
                &packet.body,
                Some(&mut *header_crypto),
            )
            .await?;
        } else {
            dispatch.push((session_id, packet));
        }
    }

    shared_world.sessions.dispatch(dispatch).await;

    Ok(())
}

pub(in crate::world) fn character_guid_from_caster(caster: ObjectGuid) -> u32 {
    caster.counter()
}
