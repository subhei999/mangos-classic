use super::*;
use wow_proto::world::WorldOpcode;
use wow_proto::{
    ServerWorldPacket, SmsgActivateTaxiReplyResponse, SmsgBinderConfirmResponse,
    SmsgBindpointUpdateResponse, SmsgNewTaxiPathResponse, SmsgPlayerBoundResponse,
    SmsgTaxiNodeStatusResponse, SmsgTrainerBuySucceededResponse, SpellCastTargets,
    SPELL_CAST_TARGET_UNIT,
};

const TAXI_FLIGHT_SPEED: f32 = 32.0;
const ERR_TAXIOK: u32 = 0;
const ERR_TAXIUNSPECIFIEDSERVERERROR: u32 = 1;
const ERR_TAXINOSUCHPATH: u32 = 2;
const ERR_TAXINOTENOUGHMONEY: u32 = 3;
const ERR_TAXINOTVISITED: u32 = 6;
const SPELL_BIND: u32 = 3286;

#[derive(Clone, Copy)]
pub(in crate::world) struct TaxiDeps<'a> {
    pub(in crate::world) character_db_pool: &'a MySqlPool,
    pub(in crate::world) world_db_pool: &'a MySqlPool,
    pub(in crate::world) world_data_files: &'a WorldDataFiles,
    pub(in crate::world) maps: &'a Arc<MapRuntimeManager>,
    pub(in crate::world) sessions: &'a Arc<SessionRegistry>,
    pub(in crate::world) account_id: u32,
}

pub(in crate::world) async fn handle_taxi_node_status_query(
    stream: &mut WorldPacketSink,
    character_db_pool: &MySqlPool,
    world_db_pool: &MySqlPool,
    world_data_files: &WorldDataFiles,
    request: wow_proto::TaxiNodeStatusQueryRequest,
    session: &WorldSessionState,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    let guid = ObjectGuid::from_raw(request.raw_guid);
    let Some(character) = session.character.active_character.as_ref() else {
        return Ok(());
    };
    let Some(node) =
        nearest_creature_taxi_node(world_db_pool, world_data_files, guid, character).await?
    else {
        return Ok(());
    };
    let taximask = wow_db::get_character_taximask(character_db_pool, character.guid).await?;
    let body = build_taxi_node_status_body(guid, world_data_files.taxi_node_known(taximask, node));
    send_packet(
        stream,
        WorldOpcode::SmsgTaxiNodeStatus as u16,
        &body,
        Some(header_crypto),
    )
    .await
}

pub(in crate::world) async fn handle_taxi_query_available_nodes(
    stream: &mut WorldPacketSink,
    character_db_pool: &MySqlPool,
    world_db_pool: &MySqlPool,
    world_data_files: &WorldDataFiles,
    request: wow_proto::TaxiQueryAvailableNodesRequest,
    session: &mut WorldSessionState,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    let guid = ObjectGuid::from_raw(request.raw_guid);
    let Some(character) = session.character.active_character.as_ref() else {
        return Ok(());
    };
    let Some(current_node) =
        nearest_creature_taxi_node(world_db_pool, world_data_files, guid, character).await?
    else {
        return Ok(());
    };
    let (taximask, learned) = load_and_learn_taxi_node(
        character_db_pool,
        world_data_files,
        character.guid,
        current_node,
    )
    .await?;
    if learned {
        send_new_taxi_path_feedback(stream, guid, header_crypto).await?;
    }

    send_taxi_menu(stream, guid, current_node, taximask, header_crypto).await
}

pub(in crate::world) async fn discover_taxi_node_on_gossip_hello(
    stream: &mut WorldPacketSink,
    character_db_pool: &MySqlPool,
    world_db_pool: &MySqlPool,
    world_data_files: &WorldDataFiles,
    guid: ObjectGuid,
    session: &mut WorldSessionState,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    let Some(character) = session.character.active_character.as_ref() else {
        return Ok(());
    };
    let Some(node) =
        nearest_creature_taxi_node(world_db_pool, world_data_files, guid, character).await?
    else {
        return Ok(());
    };
    let (_taximask, learned) =
        load_and_learn_taxi_node(character_db_pool, world_data_files, character.guid, node).await?;
    if learned {
        send_new_taxi_path_feedback(stream, guid, header_crypto).await?;
    }
    Ok(())
}

pub(in crate::world) async fn handle_activate_taxi(
    stream: &mut WorldPacketSink,
    deps: TaxiDeps<'_>,
    request: wow_proto::ActivateTaxiRequest,
    session: &mut WorldSessionState,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    let guid = ObjectGuid::from_raw(request.raw_guid);
    let Some(character) = session.character.active_character.as_ref() else {
        return Ok(());
    };
    let Some(current_node) =
        nearest_creature_taxi_node(deps.world_db_pool, deps.world_data_files, guid, character)
            .await?
    else {
        return Ok(());
    };
    if request.source_node != current_node {
        return send_activate_taxi_reply(stream, ERR_TAXINOSUCHPATH, header_crypto).await;
    }

    let taximask = wow_db::get_character_taximask(deps.character_db_pool, character.guid).await?;
    if !deps
        .world_data_files
        .taxi_node_known(taximask, request.source_node)
        || !deps
            .world_data_files
            .taxi_node_known(taximask, request.destination_node)
    {
        return send_activate_taxi_reply(stream, ERR_TAXINOTVISITED, header_crypto).await;
    }

    let Some(path) = deps
        .world_data_files
        .taxi_path(request.source_node, request.destination_node)
    else {
        return send_activate_taxi_reply(stream, ERR_TAXINOSUCHPATH, header_crypto).await;
    };
    let Some(destination) = deps.world_data_files.taxi_node(request.destination_node) else {
        return send_activate_taxi_reply(stream, ERR_TAXINOSUCHPATH, header_crypto).await;
    };
    if destination.map_id != character.position.map_id {
        return send_activate_taxi_reply(stream, ERR_TAXIUNSPECIFIEDSERVERERROR, header_crypto)
            .await;
    }
    let flight_nodes = deps.world_data_files.taxi_path_nodes(path.id);
    if flight_nodes.len() < 2
        || flight_nodes
            .iter()
            .any(|node| node.map_id != character.position.map_id)
    {
        return send_activate_taxi_reply(stream, ERR_TAXINOSUCHPATH, header_crypto).await;
    }

    let character_guid = character.guid;
    let old_map_id = character.position.map_id;
    let player_guid = ObjectGuid::new(HighGuid::Player, 0, character_guid);
    let flight_positions = flight_nodes
        .iter()
        .map(|node| node.position())
        .collect::<Vec<_>>();
    let start_position = flight_positions
        .first()
        .copied()
        .unwrap_or(character.position);
    let mut destination_position = flight_positions
        .last()
        .copied()
        .unwrap_or_else(|| destination.position());
    destination_position.orientation = character.position.orientation;
    let mount_display_id = taxi_mount_display_id(
        deps.world_db_pool,
        deps.world_data_files,
        request.source_node,
        character.race,
    )
    .await?;
    if mount_display_id == 0 {
        return send_activate_taxi_reply(stream, ERR_TAXIUNSPECIFIEDSERVERERROR, header_crypto)
            .await;
    }
    let money = if path.price == 0 {
        None
    } else {
        match wow_db::spend_character_money(deps.character_db_pool, character_guid, path.price)
            .await?
        {
            Some(money) => Some(money),
            None => {
                return send_activate_taxi_reply(stream, ERR_TAXINOTENOUGHMONEY, header_crypto)
                    .await
            }
        }
    };

    send_activate_taxi_reply(stream, ERR_TAXIOK, header_crypto).await?;
    let unit_flags = UNIT_FLAG_CLIENT_CONTROL_LOST | UNIT_FLAG_TAXI_FLIGHT;
    send_packet(
        stream,
        WorldOpcode::SmsgUpdateObject as u16,
        &build_taxi_player_values_update_body(player_guid, unit_flags, mount_display_id)?,
        Some(&mut *header_crypto),
    )
    .await?;
    let spline_id = path.id;
    let duration_ms = taxi_flight_duration_ms(&flight_positions);
    send_packet(
        stream,
        WorldOpcode::SmsgMonsterMove as u16,
        &build_taxi_flight_spline_body(
            player_guid,
            start_position,
            &flight_positions[1..],
            spline_id,
            duration_ms,
        )?,
        Some(&mut *header_crypto),
    )
    .await?;
    let observer_packets = deps
        .maps
        .set_player_position(old_map_id, character_guid, start_position)
        .await?;
    if let Some(character) = session.character.active_character.as_mut() {
        character.position = start_position;
        character.movement_flags = 0;
        character.fall_time = 0;
        character.jump = JumpInfo::default();
    }
    session.movement.active_taxi = Some(TaxiFlightSession {
        spline_id,
        destination_position,
    });
    deps.sessions.dispatch(observer_packets).await;
    deps.maps
        .reset_player_visibility_scan_positions(old_map_id, character_guid)
        .await;
    deps.maps
        .sync_player_gameplay_state(old_map_id, character_guid, session)
        .await;
    wow_db::update_character_position(
        deps.character_db_pool,
        deps.account_id,
        character_guid,
        start_position,
    )
    .await?;
    if let Some(money) = money {
        send_packet(
            stream,
            WorldOpcode::SmsgUpdateObject as u16,
            &build_player_money_update_body(character_guid, money)?,
            Some(&mut *header_crypto),
        )
        .await?;
    }
    stream_newly_visible_db_creatures(
        stream,
        deps.character_db_pool,
        deps.world_db_pool,
        deps.maps,
        session,
        header_crypto,
    )
    .await
}

pub(in crate::world) async fn handle_taxi_spline_done(
    stream: &mut WorldPacketSink,
    deps: TaxiDeps<'_>,
    body: &[u8],
    session: &mut WorldSessionState,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    let Some(flight) = session.movement.active_taxi.clone() else {
        return Ok(());
    };
    let (_movement, mut cursor) = MovementInfo::read_with_len(body)?;
    ensure_available(body, cursor + 8)?;
    let spline_id = read_u32(body, &mut cursor)?;
    let _unused = read_u32(body, &mut cursor)?;
    if spline_id != flight.spline_id {
        return Ok(());
    }
    let Some(character) = session.character.active_character.as_ref() else {
        return Ok(());
    };
    let character_guid = character.guid;
    let map_id = character.position.map_id;
    let player_guid = ObjectGuid::new(HighGuid::Player, 0, character_guid);
    let observer_packets = deps
        .maps
        .set_player_position(map_id, character_guid, flight.destination_position)
        .await?;
    if let Some(character) = session.character.active_character.as_mut() {
        character.position = flight.destination_position;
        character.movement_flags = 0;
        character.fall_time = 0;
        character.jump = JumpInfo::default();
    }
    session.movement.active_taxi = None;
    deps.sessions.dispatch(observer_packets).await;
    deps.maps
        .reset_player_visibility_scan_positions(map_id, character_guid)
        .await;
    deps.maps
        .sync_player_gameplay_state(map_id, character_guid, session)
        .await;
    wow_db::update_character_position(
        deps.character_db_pool,
        deps.account_id,
        character_guid,
        flight.destination_position,
    )
    .await?;
    send_packet(
        stream,
        WorldOpcode::SmsgUpdateObject as u16,
        &build_taxi_player_values_update_body(player_guid, 0, 0)?,
        Some(&mut *header_crypto),
    )
    .await?;
    stream_newly_visible_db_creatures(
        stream,
        deps.character_db_pool,
        deps.world_db_pool,
        deps.maps,
        session,
        header_crypto,
    )
    .await
}

pub(in crate::world) async fn send_bind_confirmation(
    stream: &mut WorldPacketSink,
    guid: ObjectGuid,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    let body = SmsgBinderConfirmResponse { innkeeper: guid }.body();
    send_packet(
        stream,
        WorldOpcode::SmsgBinderConfirm as u16,
        &body,
        Some(header_crypto),
    )
    .await
}

pub(in crate::world) async fn handle_binder_activate(
    stream: &mut WorldPacketSink,
    character_db_pool: &MySqlPool,
    world_db_pool: &MySqlPool,
    maps: &Arc<MapRuntimeManager>,
    request: wow_proto::BinderActivateRequest,
    session: &mut WorldSessionState,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    let guid = ObjectGuid::from_raw(request.raw_guid);
    if !creature_has_npc_flag(world_db_pool, guid, UNIT_NPC_FLAG_INNKEEPER).await? {
        return Ok(());
    }
    let Some(character) = session.character.active_character.as_ref() else {
        return Ok(());
    };
    let position = character.position;
    let area = homebind_area_id(maps, position, session);
    let player_guid = ObjectGuid::new(HighGuid::Player, 0, character.guid);
    let targets = SpellCastTargets {
        target_mask: SPELL_CAST_TARGET_UNIT,
        unit_target: Some(player_guid),
        ..SpellCastTargets::empty()
    };
    send_packet(
        stream,
        WorldOpcode::SmsgSpellStart as u16,
        &build_spell_start_body(guid, SPELL_BIND, 0, &targets)?,
        Some(&mut *header_crypto),
    )
    .await?;
    send_packet(
        stream,
        WorldOpcode::SmsgSpellGo as u16,
        &build_spell_go_body(guid, SPELL_BIND, &targets)?,
        Some(&mut *header_crypto),
    )
    .await?;
    wow_db::update_character_homebind(
        character_db_pool,
        character.guid,
        position.map_id,
        area,
        position,
    )
    .await?;
    send_packet(
        stream,
        WorldOpcode::SmsgTrainerBuySucceeded as u16,
        &SmsgTrainerBuySucceededResponse {
            trainer: guid,
            spell: SPELL_BIND,
        }
        .body(),
        Some(&mut *header_crypto),
    )
    .await?;
    send_packet(
        stream,
        WorldOpcode::SmsgBindpointUpdate as u16,
        &SmsgBindpointUpdateResponse {
            x: position.x,
            y: position.y,
            z: position.z,
            map: position.map_id,
            zone: area,
        }
        .body(),
        Some(&mut *header_crypto),
    )
    .await?;
    send_packet(
        stream,
        WorldOpcode::SmsgPlayerBound as u16,
        &SmsgPlayerBoundResponse { caster: guid, area }.body(),
        Some(&mut *header_crypto),
    )
    .await?;
    send_packet(
        stream,
        WorldOpcode::SmsgGossipComplete as u16,
        &[],
        Some(header_crypto),
    )
    .await
}

fn homebind_area_id(
    maps: &Arc<MapRuntimeManager>,
    position: WorldPosition,
    session: &WorldSessionState,
) -> u32 {
    maps.geometry
        .area_entry_with_source(position, "binder_homebind")
        .map(|(_, area)| area.id)
        .or(session.character.current_zone)
        .unwrap_or_default()
}

async fn send_taxi_menu(
    stream: &mut WorldPacketSink,
    guid: ObjectGuid,
    current_node: u32,
    taximask: [u32; 8],
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    let body = wow_proto::SmsgShowTaxiNodesResponse {
        taxi_master: guid,
        current_node,
        taximask,
    }
    .body();
    send_packet(
        stream,
        WorldOpcode::SmsgShowTaxiNodes as u16,
        &body,
        Some(header_crypto),
    )
    .await
}

async fn load_and_learn_taxi_node(
    character_db_pool: &MySqlPool,
    world_data_files: &WorldDataFiles,
    character_guid: u32,
    node: u32,
) -> anyhow::Result<([u32; 8], bool)> {
    let mut taximask = wow_db::get_character_taximask(character_db_pool, character_guid).await?;
    let learned = set_taxi_mask_node(&mut taximask, node);
    if learned {
        wow_db::save_character_taximask(character_db_pool, character_guid, taximask).await?;
        debug!(character_guid, node, "Learned taxi node");
    } else if !world_data_files.taxi_node_known(taximask, node) {
        warn!(
            character_guid,
            node, "Taxi node could not be marked as known"
        );
    }
    Ok((taximask, learned))
}

async fn send_new_taxi_path_feedback(
    stream: &mut WorldPacketSink,
    guid: ObjectGuid,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    send_packet(
        stream,
        WorldOpcode::SmsgNewTaxiPath as u16,
        &SmsgNewTaxiPathResponse.body(),
        Some(&mut *header_crypto),
    )
    .await?;
    send_packet(
        stream,
        WorldOpcode::SmsgTaxiNodeStatus as u16,
        &build_taxi_node_status_body(guid, true),
        Some(header_crypto),
    )
    .await
}

fn build_taxi_node_status_body(guid: ObjectGuid, known: bool) -> Vec<u8> {
    SmsgTaxiNodeStatusResponse {
        taxi_master: guid,
        known,
    }
    .body()
}

async fn send_activate_taxi_reply(
    stream: &mut WorldPacketSink,
    reply: u32,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    let body = SmsgActivateTaxiReplyResponse { reply }.body();
    send_packet(
        stream,
        WorldOpcode::SmsgActivateTaxiReply as u16,
        &body,
        Some(header_crypto),
    )
    .await
}

async fn nearest_creature_taxi_node(
    world_db_pool: &MySqlPool,
    world_data_files: &WorldDataFiles,
    guid: ObjectGuid,
    character: &ActiveCharacter,
) -> anyhow::Result<Option<u32>> {
    if !creature_has_npc_flag(world_db_pool, guid, UNIT_NPC_FLAG_TAXIVENDOR).await? {
        return Ok(None);
    }
    let Some(template) = wow_db::get_creature_template_query(world_db_pool, guid.entry()).await?
    else {
        return Ok(None);
    };
    if template.npc_flags & UNIT_NPC_FLAG_TAXIVENDOR == 0 {
        return Ok(None);
    }
    Ok(world_data_files.nearest_taxi_node(character.position, is_alliance_race(character.race)))
}

async fn taxi_mount_display_id(
    world_db_pool: &MySqlPool,
    world_data_files: &WorldDataFiles,
    source_node: u32,
    race: u8,
) -> anyhow::Result<u32> {
    let Some(node) = world_data_files.taxi_node(source_node) else {
        return Ok(0);
    };
    let mount_entry = node.mount_creature_id(is_alliance_race(race));
    if mount_entry == 0 {
        return Ok(0);
    }
    Ok(
        wow_db::get_creature_template_query(world_db_pool, mount_entry)
            .await?
            .map(|template| creature_display_id(&template))
            .unwrap_or(0),
    )
}

fn taxi_flight_duration_ms(path: &[WorldPosition]) -> u32 {
    let distance = path
        .windows(2)
        .map(|segment| {
            let left = segment[0];
            let right = segment[1];
            let dx = right.x - left.x;
            let dy = right.y - left.y;
            let dz = right.z - left.z;
            ((dx * dx) + (dy * dy) + (dz * dz)).sqrt()
        })
        .sum::<f32>();
    ((distance / TAXI_FLIGHT_SPEED) * 1000.0).ceil().max(1.0) as u32
}

fn build_taxi_player_values_update_body(
    player: ObjectGuid,
    unit_flags: u32,
    mount_display_id: u32,
) -> anyhow::Result<Vec<u8>> {
    let mut block = Vec::new();
    block.push(UPDATE_TYPE_VALUES);
    PackedGuid::write(&mut block, player)?;
    let mut values = vec![None; PLAYER_END_FIELDS];
    set_update_value(&mut values, UNIT_FIELD_FLAGS, unit_flags)?;
    set_update_value(&mut values, UNIT_FIELD_MOUNTDISPLAYID, mount_display_id)?;
    write_update_values(&mut block, &values)?;
    Ok(build_update_object_body(&[block]))
}

async fn creature_has_npc_flag(
    world_db_pool: &MySqlPool,
    guid: ObjectGuid,
    flag: u32,
) -> anyhow::Result<bool> {
    if !guid.is_creature() {
        return Ok(false);
    }
    Ok(
        wow_db::get_creature_template_query(world_db_pool, guid.entry())
            .await?
            .is_some_and(|template| template.npc_flags & flag != 0),
    )
}

fn is_alliance_race(race: u8) -> bool {
    matches!(race, 1 | 3 | 4 | 7)
}
