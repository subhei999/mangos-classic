// CMaNGOS reference: src/game/Handlers/MovementHandler.cpp movement packet builders.

fn build_near_teleport_ack_body(
    character: &ActiveCharacter,
    counter: u32,
) -> anyhow::Result<Vec<u8>> {
    let mut body = Vec::with_capacity(41);

    let player = ObjectGuid::new(HighGuid::Player, 0, character.guid);

    PackedGuid::write(&mut body, player)?;

    body.extend_from_slice(&counter.to_le_bytes());

    write_movement_info(
        &mut body,
        character.movement_flags,
        character.client_time,
        character.position,
        character.fall_time,
        &character.jump,
    );

    Ok(body)
}

fn build_monster_move_walk_path_body(
    guid: ObjectGuid,

    start: WorldPosition,

    path: &[WorldPosition],

    spline_id: u32,

    duration_ms: u32,
) -> anyhow::Result<Vec<u8>> {
    build_monster_move_path_body_inner(guid, start, path, spline_id, duration_ms, None, false)
}

fn build_monster_move_run_path_body(
    guid: ObjectGuid,

    start: WorldPosition,

    path: &[WorldPosition],

    spline_id: u32,

    duration_ms: u32,
) -> anyhow::Result<Vec<u8>> {
    build_monster_move_path_body_inner(guid, start, path, spline_id, duration_ms, None, true)
}

fn build_monster_move_facing_target_body(
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

fn build_monster_move_stop_body(
    guid: ObjectGuid,

    position: WorldPosition,

    spline_id: u32,
) -> anyhow::Result<Vec<u8>> {
    let mut body = Vec::with_capacity(32);

    PackedGuid::write(&mut body, guid)?;

    body.extend_from_slice(&position.x.to_le_bytes());

    body.extend_from_slice(&position.y.to_le_bytes());

    body.extend_from_slice(&position.z.to_le_bytes());

    body.extend_from_slice(&spline_id.to_le_bytes());

    body.push(MONSTER_MOVE_TYPE_STOP);

    Ok(body)
}

fn build_spline_set_speed_body(guid: ObjectGuid, speed: f32) -> anyhow::Result<Vec<u8>> {
    let mut body = Vec::with_capacity(16);
    PackedGuid::write(&mut body, guid)?;
    body.extend_from_slice(&speed.to_le_bytes());
    Ok(body)
}

fn build_monster_move_facing_target_path_body(
    guid: ObjectGuid,

    start: WorldPosition,

    path: &[WorldPosition],

    spline_id: u32,

    duration_ms: u32,

    target: ObjectGuid,
) -> anyhow::Result<Vec<u8>> {
    build_monster_move_path_body_inner(
        guid,
        start,
        path,
        spline_id,
        duration_ms,
        Some(target),
        true,
    )
}

fn build_monster_move_path_body_inner(
    guid: ObjectGuid,

    start: WorldPosition,

    path: &[WorldPosition],

    spline_id: u32,

    duration_ms: u32,

    facing_target: Option<ObjectGuid>,

    run: bool,
) -> anyhow::Result<Vec<u8>> {
    anyhow::ensure!(!path.is_empty(), "monster movement path must not be empty");

    let mut body = Vec::with_capacity(52 + path.len() * 12);

    PackedGuid::write(&mut body, guid)?;

    body.extend_from_slice(&start.x.to_le_bytes());

    body.extend_from_slice(&start.y.to_le_bytes());

    body.extend_from_slice(&start.z.to_le_bytes());

    body.extend_from_slice(&spline_id.to_le_bytes());

    if let Some(target) = facing_target {
        body.push(MONSTER_MOVE_TYPE_FACING_TARGET);

        body.extend_from_slice(&target.raw().to_le_bytes());
    } else {
        body.push(MONSTER_MOVE_TYPE_NORMAL);
    }

    let spline_flags = if run {
        MONSTER_MOVE_SPLINE_FLAG_RUNMODE
    } else {
        0
    };

    body.extend_from_slice(&spline_flags.to_le_bytes());

    body.extend_from_slice(&duration_ms.to_le_bytes());

    let destination = path[path.len() - 1];

    let count_pos = body.len();

    body.extend_from_slice(&0u32.to_le_bytes());

    body.extend_from_slice(&destination.x.to_le_bytes());

    body.extend_from_slice(&destination.y.to_le_bytes());

    body.extend_from_slice(&destination.z.to_le_bytes());

    let mut offset_count = 1u32;

    for point in &path[..path.len() - 1] {
        let offset_x = destination.x - point.x;

        let offset_y = destination.y - point.y;

        let offset_z = destination.z - point.z;

        if (offset_x * offset_x) + (offset_y * offset_y) + (offset_z * offset_z) < 0.5 {
            continue;
        }

        body.extend_from_slice(
            &pack_monster_move_xyz_offset(offset_x, offset_y, offset_z).to_le_bytes(),
        );

        offset_count += 1;
    }

    body[count_pos..count_pos + 4].copy_from_slice(&offset_count.to_le_bytes());

    Ok(body)
}

fn pack_monster_move_xyz_offset(x: f32, y: f32, z: f32) -> u32 {
    let mut packed = 0;

    packed |= ((x / 0.25) as i32 as u32) & 0x7FF;

    packed |= (((y / 0.25) as i32 as u32) & 0x7FF) << 11;

    packed |= (((z / 0.25) as i32 as u32) & 0x3FF) << 22;

    packed
}
