// CMaNGOS reference: src/game/Entities/Unit.cpp Unit::DealDamage/Kill/SetDeathState.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
enum WorldDamageKind {
    Melee,
    SpellDirect,
    PeriodicAura,
    Environmental,
    Fall,
    SelfDamage,
    Instakill,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
struct AppliedPlayerWorldDamage {
    target: ObjectGuid,
    source: Option<ObjectGuid>,
    kind: WorldDamageKind,
    requested_damage: u32,
    applied_damage: u32,
    remaining_health: u32,
    died: bool,
    position: WorldPosition,
    direct_session_id: Option<SessionId>,
    direct_packets: Vec<OutboundWorldPacket>,
    health_packet: OutboundWorldPacket,
    aura_packet: Option<OutboundWorldPacket>,
}

#[derive(Debug, Clone, Copy)]
#[allow(dead_code)]
struct AppliedCreatureWorldDamage {
    target: ObjectGuid,
    source: ObjectGuid,
    kind: WorldDamageKind,
    requested_damage: u32,
    applied_damage: u32,
    remaining_health: u32,
    died: bool,
}

impl MapRuntime {
    fn apply_player_world_damage(
        &mut self,
        target: ObjectGuid,
        source: Option<ObjectGuid>,
        requested_damage: u32,
        kind: WorldDamageKind,
        now: Instant,
    ) -> anyhow::Result<Option<AppliedPlayerWorldDamage>> {
        let Some(player) = self.players.get_mut(&target.counter()) else {
            return Ok(None);
        };
        let applied =
            apply_player_runtime_world_damage(player, target, source, requested_damage, kind, now)?;
        if applied.as_ref().is_some_and(|damage| damage.died) {
            self.active_player_spell_casts.remove(&target.counter());
        }
        Ok(applied)
    }
}

fn apply_player_runtime_world_damage(
    player: &mut PlayerRuntime,
    target: ObjectGuid,
    source: Option<ObjectGuid>,
    requested_damage: u32,
    kind: WorldDamageKind,
    now: Instant,
) -> anyhow::Result<Option<AppliedPlayerWorldDamage>> {
    if requested_damage == 0
        || player.health == 0
        || player.death_state != PlayerDeathState::Alive
        || player.flags & PLAYER_FLAGS_GHOST != 0
    {
        return Ok(None);
    }

    let previous_health = player.health;
    let applied_damage = requested_damage.min(previous_health);
    player.health = previous_health.saturating_sub(applied_damage);
    player.environment.last_damage_at = Some(now);
    let died = player.health == 0;
    let position = player.position;
    let direct_session_id = player.client_session_id();
    let direct_packets = Vec::new();
    let aura_packet = if died && !player.active_auras.is_empty() {
        player.active_auras.clear();
        Some(OutboundWorldPacket {
            opcode: SMSG_UPDATE_OBJECT,
            body: build_player_aura_update_body(target, &player.active_auras)?,
        })
    } else {
        None
    };
    if died {
        player.death_state = PlayerDeathState::Corpse;
        player.active_combat_target = None;
        player.active_combat_next_swing_at = None;
        player.queued_next_melee_spell = None;
        player.stand_state = PLAYER_STAND_STATE_DEAD;
        player.movement_flags = 0;
        player.fall_time = 0;
        player.jump = JumpInfo::default();
        let source_guid = source.map(|guid| format!("0x{:016X}", guid.raw()));
        warn!(
            target = format_args!("0x{:016X}", target.raw()),
            source = source_guid.as_deref(),
            ?kind,
            requested_damage,
            applied_damage,
            old_health = previous_health,
            "MapRuntime finalized player death from world damage"
        );
    }
    let remaining_health = player.health;
    let health_packet = if died {
        OutboundWorldPacket {
            opcode: SMSG_UPDATE_OBJECT,
            body: build_player_death_update_body(
                target,
                0,
                player.flags,
                PLAYER_FIELD_BYTE_RELEASE_TIMER,
                player_unit_flags(false),
                player.class,
                PLAYER_STAND_STATE_DEAD,
            )?,
        }
    } else {
        OutboundWorldPacket {
            opcode: SMSG_UPDATE_OBJECT,
            body: build_player_health_update_body(target, remaining_health)?,
        }
    };
    Ok(Some(AppliedPlayerWorldDamage {
        target,
        source,
        kind,
        requested_damage,
        applied_damage,
        remaining_health,
        died,
        position,
        direct_session_id,
        direct_packets,
        health_packet,
        aura_packet,
    }))
}

impl MapRuntime {
    fn apply_creature_world_damage(
        &mut self,
        target: ObjectGuid,
        source: ObjectGuid,
        requested_damage: u32,
        kind: WorldDamageKind,
        now: Instant,
        now_epoch_secs: u64,
    ) -> anyhow::Result<Option<AppliedCreatureWorldDamage>> {
        let Some(creature) = self.creatures.get_mut(&target.raw()) else {
            return Ok(None);
        };
        apply_creature_runtime_world_damage(
            creature,
            target,
            source,
            requested_damage,
            kind,
            now,
            now_epoch_secs,
        )
    }
}

fn apply_creature_runtime_world_damage(
    creature: &mut DbCreatureRuntime,
    target: ObjectGuid,
    source: ObjectGuid,
    requested_damage: u32,
    kind: WorldDamageKind,
    now: Instant,
    now_epoch_secs: u64,
) -> anyhow::Result<Option<AppliedCreatureWorldDamage>> {
    if !creature.is_alive() || creature.is_evading_home() {
        return Ok(None);
    }

        let previous_health = creature.health;
        let applied_damage = requested_damage.min(previous_health);
        creature.health = previous_health.saturating_sub(applied_damage);
        let died = creature.health == 0;
        if died {
            creature.begin_corpse(now, now_epoch_secs);
            warn!(
                target = format_args!("0x{:016X}", target.raw()),
                source = format_args!("0x{:016X}", source.raw()),
                ?kind,
                requested_damage,
                applied_damage,
                old_health = previous_health,
                "MapRuntime finalized creature death from world damage"
            );
        }
        Ok(Some(AppliedCreatureWorldDamage {
            target,
            source,
            kind,
            requested_damage,
            applied_damage,
            remaining_health: creature.health,
            died,
        }))
}
