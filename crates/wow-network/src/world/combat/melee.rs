#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PlayerMeleeCheck {
    Clear,
    NoActiveCharacter,
    MissingTarget,
    TargetNotAlive,
    NavigationBlocked(DbCreatureNavigationResult),
    OutOfRange,
    BadFacing,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PlayerMeleeSwingError {
    NotInRange,
    BadFacing,
    DeadTarget,
    CantAttack,
}

impl PlayerMeleeSwingError {
    fn opcode(self) -> u16 {
        match self {
            Self::NotInRange => SMSG_ATTACKSWING_NOTINRANGE,
            Self::BadFacing => SMSG_ATTACKSWING_BADFACING,
            Self::DeadTarget => SMSG_ATTACKSWING_DEADTARGET,
            Self::CantAttack => SMSG_ATTACKSWING_CANT_ATTACK,
        }
    }

    fn packet(self) -> OutboundWorldPacket {
        OutboundWorldPacket {
            opcode: self.opcode(),
            body: Vec::new(),
        }
    }
}

fn db_creature_player_melee_check(
    session: &WorldSessionState,
    target: ObjectGuid,
) -> PlayerMeleeCheck {
    let Some(character) = &session.active_character else {
        return PlayerMeleeCheck::NoActiveCharacter;
    };
    let Some(creature) = session.db_creatures.get(&target.raw()) else {
        return PlayerMeleeCheck::MissingTarget;
    };
    if !creature.is_alive() || creature.is_evading_home() {
        return PlayerMeleeCheck::TargetNotAlive;
    }
    let navigation = db_creature_navigation_check(
        &session.db_creature_navigation,
        character.position,
        creature.current_position,
    );
    if !navigation.is_clear() {
        return PlayerMeleeCheck::NavigationBlocked(navigation);
    }
    if !player_can_reach_with_melee_attack(character, creature) {
        return PlayerMeleeCheck::OutOfRange;
    }
    if !has_in_arc(
        character.position,
        creature.current_position,
        PLAYER_MELEE_ARC_RADIANS,
    ) {
        return PlayerMeleeCheck::BadFacing;
    }
    PlayerMeleeCheck::Clear
}

fn db_creature_attack_distance(player_level: u8, creature_level: u8, detection_range: u32) -> f32 {
    if detection_range == 0 {
        return 0.0;
    }
    let mut level_diff = player_level as i32 - creature_level as i32;
    if level_diff < -25 {
        level_diff = -25;
    }
    (detection_range as f32 - level_diff as f32).max(5.0)
}

fn player_can_reach_with_melee_attack(
    character: &ActiveCharacter,
    target: &DbCreatureRuntime,
) -> bool {
    if character.position.map_id != target.current_position.map_id {
        return false;
    }
    let reach = combined_melee_reach(PLAYER_COMBAT_REACH_YARDS, target.combat_reach());
    let dx = character.position.x - target.current_position.x;
    let dy = character.position.y - target.current_position.y;
    let dz = character.position.z - target.current_position.z;
    dx * dx + dy * dy + dz * dz <= reach * reach
}

fn combined_melee_reach(attacker_combat_reach: f32, victim_combat_reach: f32) -> f32 {
    (attacker_combat_reach.max(0.0)
        + victim_combat_reach.max(0.0)
        + BASE_MELEE_RANGE_OFFSET_YARDS)
        .max(ATTACK_DISTANCE_YARDS)
}

fn creature_bounding_radius(template: &CreatureTemplateQuery) -> f32 {
    let scale = creature_scale(template);
    let radius = template.model_bounding_radius * scale;
    if radius > 0.0 {
        radius
    } else {
        DEFAULT_WORLD_OBJECT_SIZE
    }
}

fn creature_combat_reach(template: &CreatureTemplateQuery) -> f32 {
    let scale = creature_scale(template);
    let reach = template.model_combat_reach * scale;
    if reach > 0.0 {
        reach
    } else {
        PLAYER_COMBAT_REACH_YARDS
    }
}

fn has_in_arc(source: WorldPosition, target: WorldPosition, arc: f32) -> bool {
    if source.map_id != target.map_id {
        return false;
    }
    let angle = normalize_orientation((target.y - source.y).atan2(target.x - source.x));
    let mut delta = normalize_orientation(angle - source.orientation);
    if delta > std::f32::consts::PI {
        delta -= 2.0 * std::f32::consts::PI;
    }
    delta >= -(arc / 2.0) && delta <= arc / 2.0
}

fn normalize_orientation(angle: f32) -> f32 {
    angle.rem_euclid(2.0 * std::f32::consts::PI)
}

fn db_creature_can_reach_player(session: &WorldSessionState, attacker: ObjectGuid) -> bool {
    let Some(character) = &session.active_character else {
        return false;
    };
    let Some(creature) = session.db_creatures.get(&attacker.raw()) else {
        return false;
    };
    if !db_creature_navigation_check(
        &session.db_creature_navigation,
        creature.current_position,
        character.position,
    )
    .is_clear()
    {
        return false;
    }
    creature
        .distance_to_player_squared(character)
        .is_some_and(|distance_sq| {
            let reach = combined_melee_reach(creature.combat_reach(), PLAYER_COMBAT_REACH_YARDS);
            distance_sq <= reach * reach
        })
}

fn db_creature_has_player_in_arc(session: &WorldSessionState, attacker: ObjectGuid) -> bool {
    let Some(character) = &session.active_character else {
        return false;
    };
    let Some(creature) = session.db_creatures.get(&attacker.raw()) else {
        return false;
    };
    has_in_arc(
        creature.current_position,
        character.position,
        PLAYER_MELEE_ARC_RADIANS,
    )
}

async fn send_db_creature_face_target(
    stream: &mut WorldPacketSink,
    broadcast: CreatureCombatBroadcast<'_>,
    session: &mut WorldSessionState,
    attacker: ObjectGuid,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    let Some((position, spline_id)) = face_db_creature_toward_player(session, attacker) else {
        return Ok(());
    };
    let body = build_monster_move_facing_target_body(
        attacker,
        position,
        position,
        spline_id,
        1,
        broadcast.player,
    )?;
    send_packet(
        stream,
        SMSG_MONSTER_MOVE,
        &body,
        Some(header_crypto),
    )
    .await?;
    broadcast_db_creature_packet(
        broadcast,
        session,
        attacker,
        SMSG_MONSTER_MOVE,
        body,
    )
    .await;
    Ok(())
}

async fn send_db_creature_motion_stop(
    stream: &mut WorldPacketSink,
    broadcast: CreatureCombatBroadcast<'_>,
    session: &mut WorldSessionState,
    creature_guid: ObjectGuid,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    let Some(body) = build_db_creature_motion_stop_body(session, creature_guid)? else {
        return Ok(());
    };
    send_packet(
        stream,
        SMSG_MONSTER_MOVE,
        &body,
        Some(&mut *header_crypto),
    )
    .await?;
    broadcast_db_creature_packet(
        broadcast,
        session,
        creature_guid,
        SMSG_MONSTER_MOVE,
        body,
    )
    .await;
    Ok(())
}

fn build_db_creature_motion_stop_body(
    session: &mut WorldSessionState,
    creature_guid: ObjectGuid,
) -> anyhow::Result<Option<Vec<u8>>> {
    let Some(creature) = session.db_creatures.get_mut(&creature_guid.raw()) else {
        return Ok(None);
    };
    let position = creature.current_position;
    let spline_id = creature.next_spline_id;
    creature.next_spline_id = creature.next_spline_id.wrapping_add(1);
    creature.motion = CreatureMotionState::Idle;
    Ok(Some(build_monster_move_stop_body(
        creature_guid,
        position,
        spline_id,
    )?))
}

fn face_db_creature_toward_player(
    session: &mut WorldSessionState,
    attacker: ObjectGuid,
) -> Option<(WorldPosition, u32)> {
    let character_position = session
        .active_character
        .as_ref()
        .map(|character| character.position)?;
    let creature = session.db_creatures.get_mut(&attacker.raw())?;
    let dx = character_position.x - creature.current_position.x;
    let dy = character_position.y - creature.current_position.y;
    creature.current_position.orientation = normalize_orientation(dy.atan2(dx));
    let spline_id = creature.next_spline_id;
    creature.next_spline_id = creature.next_spline_id.wrapping_add(1);
    Some((creature.current_position, spline_id))
}


