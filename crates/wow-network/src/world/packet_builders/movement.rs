use super::*;
use wow_proto::{
    MovementInfoResponse, MovementJumpResponse, MsgMoveTeleportAckResponse, ServerWorldPacket,
    SmsgMonsterMovePathResponse, SmsgMonsterMoveStopResponse, SplineSetSpeedResponse,
    WorldLocationResponse,
};

// CMaNGOS reference: src/game/Handlers/MovementHandler.cpp movement packet builders.

pub(in crate::world) fn build_near_teleport_ack_body(
    character: &ActiveCharacter,
    counter: u32,
) -> anyhow::Result<Vec<u8>> {
    Ok(MsgMoveTeleportAckResponse {
        player: ObjectGuid::new(HighGuid::Player, 0, character.guid),
        counter,
        movement: movement_info_response(
            character.movement_flags,
            character.client_time,
            character.position,
            character.fall_time,
            &character.jump,
        ),
    }
    .body())
}

pub(in crate::world) fn build_monster_move_walk_path_body(
    guid: ObjectGuid,

    start: WorldPosition,

    path: &[WorldPosition],

    spline_id: u32,

    duration_ms: u32,
) -> anyhow::Result<Vec<u8>> {
    build_monster_move_path_body_inner(guid, start, path, spline_id, duration_ms, None, false)
}

pub(in crate::world) fn build_monster_move_run_path_body(
    guid: ObjectGuid,

    start: WorldPosition,

    path: &[WorldPosition],

    spline_id: u32,

    duration_ms: u32,
) -> anyhow::Result<Vec<u8>> {
    build_monster_move_path_body_inner(guid, start, path, spline_id, duration_ms, None, true)
}

pub(in crate::world) fn build_monster_move_facing_target_body(
    guid: ObjectGuid,

    start: WorldPosition,

    destination: WorldPosition,

    spline_id: u32,

    duration_ms: u32,

    target: ObjectGuid,
) -> anyhow::Result<Vec<u8>> {
    build_monster_move_facing_target_path_body(
        guid,
        start,
        &[destination],
        spline_id,
        duration_ms,
        target,
    )
}

pub(in crate::world) fn build_monster_move_facing_spot_body(
    guid: ObjectGuid,
    start: WorldPosition,
    destination: WorldPosition,
    spline_id: u32,
    duration_ms: u32,
    facing_spot: WorldPosition,
) -> anyhow::Result<Vec<u8>> {
    build_monster_move_facing_spot_path_body(
        guid,
        start,
        &[destination],
        spline_id,
        duration_ms,
        facing_spot,
    )
}

pub(in crate::world) fn build_monster_move_stop_body(
    guid: ObjectGuid,

    position: WorldPosition,

    spline_id: u32,
) -> anyhow::Result<Vec<u8>> {
    Ok(SmsgMonsterMoveStopResponse {
        guid,
        position: world_location_response(position),
        spline_id,
        move_type: MONSTER_MOVE_TYPE_STOP,
    }
    .body())
}

pub(in crate::world) fn build_monster_move_facing_spot_path_body(
    guid: ObjectGuid,
    start: WorldPosition,
    path: &[WorldPosition],
    spline_id: u32,
    duration_ms: u32,
    facing_spot: WorldPosition,
) -> anyhow::Result<Vec<u8>> {
    anyhow::ensure!(!path.is_empty(), "monster movement path must not be empty");

    let mut body = Vec::new();
    PackedGuid::write(&mut body, guid)?;
    body.extend_from_slice(&start.x.to_le_bytes());
    body.extend_from_slice(&start.y.to_le_bytes());
    body.extend_from_slice(&start.z.to_le_bytes());
    body.extend_from_slice(&spline_id.to_le_bytes());
    body.push(MONSTER_MOVE_TYPE_FACING_SPOT);
    body.extend_from_slice(&facing_spot.x.to_le_bytes());
    body.extend_from_slice(&facing_spot.y.to_le_bytes());
    body.extend_from_slice(&facing_spot.z.to_le_bytes());
    body.extend_from_slice(&MONSTER_MOVE_SPLINE_FLAG_RUNMODE.to_le_bytes());
    body.extend_from_slice(&duration_ms.to_le_bytes());

    let destination = *path
        .last()
        .expect("non-empty path should still have a destination");
    let mut offsets = Vec::new();
    for point in &path[..path.len().saturating_sub(1)] {
        let offset_x = destination.x - point.x;
        let offset_y = destination.y - point.y;
        let offset_z = destination.z - point.z;
        if (offset_x * offset_x) + (offset_y * offset_y) + (offset_z * offset_z) < 0.5 {
            continue;
        }
        offsets.push(wow_proto::pack_monster_move_xyz_offset(
            offset_x, offset_y, offset_z,
        ));
    }
    body.extend_from_slice(&(1 + offsets.len() as u32).to_le_bytes());
    body.extend_from_slice(&destination.x.to_le_bytes());
    body.extend_from_slice(&destination.y.to_le_bytes());
    body.extend_from_slice(&destination.z.to_le_bytes());
    for offset in offsets {
        body.extend_from_slice(&offset.to_le_bytes());
    }
    Ok(body)
}

pub(in crate::world) fn build_spline_set_speed_body(
    guid: ObjectGuid,
    speed: f32,
) -> anyhow::Result<Vec<u8>> {
    Ok(SplineSetSpeedResponse { guid, speed }.body())
}

pub(in crate::world) fn build_force_run_speed_change_body(
    player: ObjectGuid,
    counter: u32,
    speed: f32,
) -> anyhow::Result<Vec<u8>> {
    let mut body = Vec::with_capacity(PackedGuid::packed_size(player) + 8);
    PackedGuid::write(&mut body, player)?;
    body.extend_from_slice(&counter.to_le_bytes());
    body.extend_from_slice(&speed.to_le_bytes());
    Ok(body)
}

pub(in crate::world) fn build_monster_move_facing_target_path_body(
    guid: ObjectGuid,

    start: WorldPosition,

    path: &[WorldPosition],

    spline_id: u32,

    duration_ms: u32,

    target: ObjectGuid,
) -> anyhow::Result<Vec<u8>> {
    build_monster_move_facing_target_path_body_with_run(
        guid,
        start,
        path,
        spline_id,
        duration_ms,
        target,
        true,
    )
}

pub(in crate::world) fn build_monster_move_facing_target_path_body_with_run(
    guid: ObjectGuid,

    start: WorldPosition,

    path: &[WorldPosition],

    spline_id: u32,

    duration_ms: u32,

    target: ObjectGuid,

    run: bool,
) -> anyhow::Result<Vec<u8>> {
    build_monster_move_path_body_inner(guid, start, path, spline_id, duration_ms, Some(target), run)
}

pub(in crate::world) fn build_monster_move_path_body_inner(
    guid: ObjectGuid,

    start: WorldPosition,

    path: &[WorldPosition],

    spline_id: u32,

    duration_ms: u32,

    facing_target: Option<ObjectGuid>,

    run: bool,
) -> anyhow::Result<Vec<u8>> {
    anyhow::ensure!(!path.is_empty(), "monster movement path must not be empty");

    Ok(SmsgMonsterMovePathResponse {
        guid,
        start: world_location_response(start),
        path: path.iter().copied().map(world_location_response).collect(),
        spline_id,
        duration_ms,
        facing_target,
        move_type_normal: MONSTER_MOVE_TYPE_NORMAL,
        move_type_facing_target: MONSTER_MOVE_TYPE_FACING_TARGET,
        run_spline_flag: MONSTER_MOVE_SPLINE_FLAG_RUNMODE,
        run,
        catmull_rom: false,
    }
    .body())
}

pub(in crate::world) fn build_taxi_flight_spline_body(
    guid: ObjectGuid,
    start: WorldPosition,
    path: &[WorldPosition],
    spline_id: u32,
    duration_ms: u32,
) -> anyhow::Result<Vec<u8>> {
    anyhow::ensure!(!path.is_empty(), "taxi flight path must not be empty");

    Ok(SmsgMonsterMovePathResponse {
        guid,
        start: world_location_response(start),
        path: path.iter().copied().map(world_location_response).collect(),
        spline_id,
        duration_ms,
        facing_target: None,
        move_type_normal: MONSTER_MOVE_TYPE_NORMAL,
        move_type_facing_target: MONSTER_MOVE_TYPE_FACING_TARGET,
        run_spline_flag: MONSTER_MOVE_SPLINE_FLAG_RUNMODE | MONSTER_MOVE_SPLINE_FLAG_FLYING,
        run: true,
        catmull_rom: true,
    }
    .body())
}

#[cfg(test)]
pub(in crate::world) fn pack_monster_move_xyz_offset(x: f32, y: f32, z: f32) -> u32 {
    wow_proto::pack_monster_move_xyz_offset(x, y, z)
}

pub(in crate::world) fn world_location_response(position: WorldPosition) -> WorldLocationResponse {
    WorldLocationResponse {
        map_id: position.map_id,
        x: position.x,
        y: position.y,
        z: position.z,
        orientation: position.orientation,
    }
}

pub(in crate::world) fn movement_info_response(
    flags: u32,
    client_time: u32,
    position: WorldPosition,
    fall_time: u32,
    jump: &JumpInfo,
) -> MovementInfoResponse {
    MovementInfoResponse {
        flags,
        client_time,
        position: world_location_response(position),
        fall_time,
        jump: (flags & MOVEFLAG_JUMPING != 0).then_some(MovementJumpResponse {
            z_speed: jump.z_speed,
            cos_angle: jump.cos_angle,
            sin_angle: jump.sin_angle,
            xy_speed: jump.xy_speed,
        }),
    }
}
