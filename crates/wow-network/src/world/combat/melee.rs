use super::*;
use wow_proto::world::WorldOpcode;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::world) enum PlayerMeleeCheck {
    Clear,
    NoActiveCharacter,
    MissingTarget,
    TargetNotAlive,
    TargetEvading,
    NavigationBlocked(DbCreatureNavigationResult),
    OutOfRange,
    BadFacing,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::world) enum PlayerMeleeSwingError {
    NotInRange,
    BadFacing,
    DeadTarget,
    CantAttack,
}

impl PlayerMeleeSwingError {
    pub(in crate::world) fn opcode(self) -> u16 {
        match self {
            Self::NotInRange => WorldOpcode::SmsgAttackSwingNotInRange as u16,
            Self::BadFacing => WorldOpcode::SmsgAttackSwingBadFacing as u16,
            Self::DeadTarget => WorldOpcode::SmsgAttackSwingDeadTarget as u16,
            Self::CantAttack => WorldOpcode::SmsgAttackSwingCantAttack as u16,
        }
    }

    pub(in crate::world) fn packet(self) -> OutboundWorldPacket {
        OutboundWorldPacket {
            opcode: self.opcode(),
            body: Vec::new(),
        }
    }
}

#[cfg(test)]
pub(in crate::world) fn db_creature_player_melee_check(
    session: &WorldSessionState,
    target: ObjectGuid,
) -> PlayerMeleeCheck {
    let Some(character) = &session.character.active_character else {
        return PlayerMeleeCheck::NoActiveCharacter;
    };
    let Some(creature) = session.visibility.db_creatures.get(&target.raw()) else {
        return PlayerMeleeCheck::MissingTarget;
    };
    if !creature.is_alive() {
        return PlayerMeleeCheck::TargetNotAlive;
    }
    if !player_can_reach_with_melee_attack(character, creature) {
        return PlayerMeleeCheck::OutOfRange;
    }
    let navigation = db_creature_navigation_check(
        &session.movement.db_creature_navigation,
        character.position,
        creature.current_position,
    );
    if !navigation.is_clear() {
        return PlayerMeleeCheck::NavigationBlocked(navigation);
    }
    if !has_in_arc(
        character.position,
        creature.current_position,
        PLAYER_MELEE_ARC_RADIANS,
    ) {
        return PlayerMeleeCheck::BadFacing;
    }
    if creature.is_evading_home() {
        return PlayerMeleeCheck::TargetEvading;
    }
    PlayerMeleeCheck::Clear
}

pub(in crate::world) async fn db_creature_player_melee_check_from_map(
    shared_world: SharedWorldDeps<'_>,
    session: &mut WorldSessionState,
    target: ObjectGuid,
) -> PlayerMeleeCheck {
    let Some(character) = session.character.active_character.as_ref() else {
        return PlayerMeleeCheck::NoActiveCharacter;
    };
    let character_guid = character.guid;
    let character_position = character.position;
    let validation = shared_world
        .maps
        .validate_player_melee_against_db_creature(
            character_position.map_id,
            character_guid,
            target,
            &session.movement.db_creature_navigation,
        )
        .await;
    #[cfg(test)]
    if let Some(creature) = shared_world
        .maps
        .db_creature_combat_snapshot(character_position.map_id, target)
        .await
    {
        let guid = creature.guid().raw();
        session.visibility.db_creatures.insert(guid, creature);
    }
    validation.check
}

pub(in crate::world) fn db_creature_attack_distance(
    player_level: u8,
    creature_level: u8,
    detection_range: u32,
) -> f32 {
    if detection_range == 0 {
        return 0.0;
    }
    let mut level_diff = player_level as i32 - creature_level as i32;
    if level_diff < -25 {
        level_diff = -25;
    }
    (detection_range as f32 - level_diff as f32).max(5.0)
}

#[cfg(test)]
pub(in crate::world) fn player_can_reach_with_melee_attack(
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

pub(in crate::world) fn combined_melee_reach(
    attacker_combat_reach: f32,
    victim_combat_reach: f32,
) -> f32 {
    (attacker_combat_reach.max(0.0) + victim_combat_reach.max(0.0) + BASE_MELEE_RANGE_OFFSET_YARDS)
        .max(ATTACK_DISTANCE_YARDS)
}

pub(in crate::world) fn creature_bounding_radius(template: &CreatureTemplateQuery) -> f32 {
    let scale = creature_scale(template);
    let radius = template.model_bounding_radius * scale;
    if radius > 0.0 {
        radius
    } else {
        DEFAULT_WORLD_OBJECT_SIZE
    }
}

pub(in crate::world) fn creature_combat_reach(template: &CreatureTemplateQuery) -> f32 {
    let scale = creature_scale(template);
    let reach = template.model_combat_reach * scale;
    if reach > 0.0 {
        reach
    } else {
        PLAYER_COMBAT_REACH_YARDS
    }
}

pub(in crate::world) fn has_in_arc(source: WorldPosition, target: WorldPosition, arc: f32) -> bool {
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

pub(in crate::world) fn normalize_orientation(angle: f32) -> f32 {
    angle.rem_euclid(2.0 * std::f32::consts::PI)
}

#[cfg(test)]
pub(in crate::world) fn db_creature_can_reach_player(
    session: &WorldSessionState,
    attacker: ObjectGuid,
) -> bool {
    let Some(character) = &session.character.active_character else {
        return false;
    };
    let Some(creature) = session.visibility.db_creatures.get(&attacker.raw()) else {
        return false;
    };
    creature
        .distance_to_player_squared(character)
        .is_some_and(|distance_sq| {
            let reach = combined_melee_reach(creature.combat_reach(), PLAYER_COMBAT_REACH_YARDS);
            distance_sq <= reach * reach
        })
}

pub(in crate::world) async fn db_creature_can_reach_player_from_map(
    shared_world: SharedWorldDeps<'_>,
    session: &WorldSessionState,
    map_id: u32,
    attacker: ObjectGuid,
) -> bool {
    let Some(character) = &session.character.active_character else {
        return false;
    };
    let Some(creature) = shared_world
        .maps
        .db_creature_snapshot(map_id, attacker)
        .await
    else {
        return false;
    };
    creature
        .distance_to_player_squared(character)
        .is_some_and(|distance_sq| {
            let reach = combined_melee_reach(creature.combat_reach(), PLAYER_COMBAT_REACH_YARDS);
            distance_sq <= reach * reach
        })
}

#[cfg(test)]
pub(in crate::world) fn db_creature_has_player_in_arc(
    session: &WorldSessionState,
    attacker: ObjectGuid,
) -> bool {
    let Some(character) = &session.character.active_character else {
        return false;
    };
    let Some(creature) = session.visibility.db_creatures.get(&attacker.raw()) else {
        return false;
    };
    has_in_arc(
        creature.current_position,
        character.position,
        PLAYER_MELEE_ARC_RADIANS,
    )
}

#[allow(dead_code)]
pub(in crate::world) async fn db_creature_has_player_in_arc_from_map(
    shared_world: SharedWorldDeps<'_>,
    session: &WorldSessionState,
    map_id: u32,
    attacker: ObjectGuid,
) -> bool {
    let Some(character) = &session.character.active_character else {
        return false;
    };
    let Some(creature) = shared_world
        .maps
        .db_creature_snapshot(map_id, attacker)
        .await
    else {
        return false;
    };
    has_in_arc(
        creature.current_position,
        character.position,
        PLAYER_MELEE_ARC_RADIANS,
    )
}

#[allow(dead_code)]
pub(in crate::world) async fn send_db_creature_face_target(
    stream: &mut WorldPacketSink,
    broadcast: CreatureCombatBroadcast<'_>,
    session: &mut WorldSessionState,
    attacker: ObjectGuid,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    let Some((position, spline_id)) =
        face_db_creature_toward_player_from_map(broadcast.shared_world, session, attacker).await
    else {
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
        WorldOpcode::SmsgMonsterMove as u16,
        &body,
        Some(header_crypto),
    )
    .await?;
    broadcast_db_creature_packet(
        broadcast,
        session,
        attacker,
        WorldOpcode::SmsgMonsterMove as u16,
        body,
    )
    .await;
    Ok(())
}

pub(in crate::world) async fn send_db_creature_motion_stop(
    stream: &mut WorldPacketSink,
    broadcast: CreatureCombatBroadcast<'_>,
    session: &mut WorldSessionState,
    creature_guid: ObjectGuid,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    let Some((creature, stop)) = broadcast
        .shared_world
        .maps
        .stop_db_creature_motion(broadcast.map_id, creature_guid)
        .await
    else {
        return Ok(());
    };
    mirror_session_db_creature(session, creature_guid.raw(), creature.clone());
    let body = build_monster_move_stop_body(creature_guid, stop.position, stop.spline_id)?;
    send_packet(
        stream,
        WorldOpcode::SmsgMonsterMove as u16,
        &body,
        Some(&mut *header_crypto),
    )
    .await?;
    broadcast_db_creature_snapshot_packet(
        broadcast,
        creature,
        WorldOpcode::SmsgMonsterMove as u16,
        body,
    )
    .await;
    Ok(())
}

#[cfg(test)]
pub(in crate::world) fn build_db_creature_motion_stop_body(
    session: &mut WorldSessionState,
    creature_guid: ObjectGuid,
) -> anyhow::Result<Option<Vec<u8>>> {
    let Some(creature) = session
        .visibility
        .db_creatures
        .get_mut(&creature_guid.raw())
    else {
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

#[cfg(test)]
pub(in crate::world) fn face_db_creature_toward_player(
    session: &mut WorldSessionState,
    attacker: ObjectGuid,
) -> Option<(WorldPosition, u32)> {
    let character_position = session
        .character
        .active_character
        .as_ref()
        .map(|character| character.position)?;
    let creature = session.visibility.db_creatures.get_mut(&attacker.raw())?;
    let dx = character_position.x - creature.current_position.x;
    let dy = character_position.y - creature.current_position.y;
    creature.current_position.orientation = normalize_orientation(dy.atan2(dx));
    let spline_id = creature.next_spline_id;
    creature.next_spline_id = creature.next_spline_id.wrapping_add(1);
    Some((creature.current_position, spline_id))
}

#[allow(dead_code)]
pub(in crate::world) async fn face_db_creature_toward_player_from_map(
    shared_world: SharedWorldDeps<'_>,
    session: &WorldSessionState,
    attacker: ObjectGuid,
) -> Option<(WorldPosition, u32)> {
    let character_position = session
        .character
        .active_character
        .as_ref()
        .map(|character| character.position)?;
    shared_world
        .maps
        .face_db_creature_toward_position(character_position.map_id, attacker, character_position)
        .await
        .map(|(_, position, spline_id)| (position, spline_id))
}
