use super::*;

// CMaNGOS reference: src/game/Entities/Unit.cpp Unit::DealDamage/Kill/SetDeathState.
pub(in crate::world) const AIRBORNE_DEATH_PRESENTATION_FALLBACK: Duration = Duration::from_secs(3);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub(in crate::world) enum WorldDamageKind {
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
pub(in crate::world) struct AppliedPlayerWorldDamage {
    pub(in crate::world) target: ObjectGuid,
    pub(in crate::world) source: Option<ObjectGuid>,
    pub(in crate::world) kind: WorldDamageKind,
    pub(in crate::world) requested_damage: u32,
    pub(in crate::world) applied_damage: u32,
    pub(in crate::world) remaining_health: u32,
    pub(in crate::world) died: bool,
    pub(in crate::world) position: WorldPosition,
    pub(in crate::world) direct_session_id: Option<SessionId>,
    pub(in crate::world) direct_packets: Vec<OutboundWorldPacket>,
    pub(in crate::world) observer_packets: Vec<(SessionId, OutboundWorldPacket)>,
    pub(in crate::world) health_packet: OutboundWorldPacket,
    pub(in crate::world) aura_packet: Option<OutboundWorldPacket>,
    pub(in crate::world) death_presentation_deferred: bool,
}

#[derive(Debug, Clone, Copy)]
#[allow(dead_code)]
pub(in crate::world) struct AppliedCreatureWorldDamage {
    pub(in crate::world) target: ObjectGuid,
    pub(in crate::world) source: ObjectGuid,
    pub(in crate::world) kind: WorldDamageKind,
    pub(in crate::world) requested_damage: u32,
    pub(in crate::world) applied_damage: u32,
    pub(in crate::world) remaining_health: u32,
    pub(in crate::world) died: bool,
}

impl MapRuntime {
    pub(in crate::world) fn apply_player_world_damage(
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
        let mut applied =
            apply_player_runtime_world_damage(player, target, source, requested_damage, kind, now)?;
        if let Some(damage) = applied.as_mut().filter(|damage| damage.died) {
            if damage.death_presentation_deferred {
                self.pending_player_death_presentations.insert(
                    target.counter(),
                    PlayerDeathPresentationRuntime { waiting_since: now },
                );
            } else {
                self.pending_player_death_presentations
                    .remove(&target.counter());
            }
            let cleanup = self.finalize_player_death_cleanup(target, now)?;
            damage.direct_packets.extend(cleanup.direct_packets);
            damage.observer_packets.extend(cleanup.observer_packets);
        }
        Ok(applied)
    }

    pub(in crate::world) fn finalize_player_death_cleanup(
        &mut self,
        player_guid: ObjectGuid,
        now: Instant,
    ) -> anyhow::Result<PlayerDeathCleanupPackets> {
        let character_guid = player_guid.counter();
        self.active_player_spell_casts.remove(&character_guid);
        self.pending_spell_events
            .retain(|event| event.caster_character_guid != character_guid);
        self.active_creature_spell_casts
            .retain(|_, cast| cast.target != player_guid);

        let Some(player) = self.players.get_mut(&character_guid) else {
            return Ok(PlayerDeathCleanupPackets::default());
        };
        player.active_combat_target = None;
        player.active_combat_next_swing_at = None;
        player.ranged_auto_attack_next_shot_at = None;
        player.queued_next_melee_spell = None;
        player.combo_target = None;
        player.combo_points = 0;
        player.in_combat = false;
        player.looting = false;
        let player_position = player.position;
        let direct_session_id = player.client_session_id();
        let _ = player;

        let mut direct_packets = Vec::new();
        let mut observer_packets = Vec::new();
        let player_combat_flags = OutboundWorldPacket {
            opcode: SMSG_UPDATE_OBJECT,
            body: build_unit_flags_update_body(player_guid, player_unit_flags(false))?,
        };
        direct_packets.push(player_combat_flags.clone());
        observer_packets.extend(
            self.nearby_player_guids(
                player_position,
                PLAYER_VISIBILITY_RADIUS_YARDS,
                Some(character_guid),
            )
            .into_iter()
            .filter_map(|observer_guid| {
                self.players
                    .get(&observer_guid)
                    .and_then(|observer| observer.packet_to_client(player_combat_flags.clone()))
            }),
        );

        let active_combats = self.active_db_creature_combats_for_victim(player_guid);
        for combat in active_combats {
            let attacker = combat.attacker;
            let Some(mut creature) = self.prepare_db_creature_evade(attacker) else {
                continue;
            };
            let mut creature_packets = vec![
                OutboundWorldPacket {
                    opcode: SMSG_ATTACKSTOP,
                    body: build_attack_stop_body(attacker, player_guid, false)?,
                },
                OutboundWorldPacket {
                    opcode: SMSG_UPDATE_OBJECT,
                    body: build_unit_flags_update_body(
                        attacker,
                        db_creature_unit_flags(&creature, false),
                    )?,
                },
                OutboundWorldPacket {
                    opcode: SMSG_UPDATE_OBJECT,
                    body: build_db_creature_state_update_body(attacker, creature.health, 0)?,
                },
            ];
            if let Some((returned, motion)) = self.start_db_creature_return_home_motion(
                &DbCreatureNavigationGuardrail::default(),
                attacker,
                now,
            ) {
                creature = returned;
                creature_packets.push(OutboundWorldPacket {
                    opcode: SMSG_MONSTER_MOVE,
                    body: build_monster_move_path_body_inner(
                        attacker,
                        motion.start,
                        &motion.path,
                        motion.spline_id,
                        motion.duration.as_millis().max(1) as u32,
                        None,
                        true,
                    )?,
                });
            }
            direct_packets.extend(creature_packets.iter().cloned());
            observer_packets.extend(
                self.nearby_player_guids(
                    creature.current_position,
                    CREATURE_SPAWN_RADIUS_YARDS,
                    Some(character_guid),
                )
                .into_iter()
                .flat_map(|observer_guid| {
                    self.players
                        .get(&observer_guid)
                        .map(|observer| {
                            creature_packets
                                .iter()
                                .cloned()
                                .filter_map(|packet| observer.packet_to_client(packet))
                                .collect::<Vec<_>>()
                        })
                        .unwrap_or_default()
                }),
            );
        }

        self.clear_db_creature_combats_for_victim(player_guid);
        self.creature_threats.retain(|_, threats| {
            threats.retain(|entry| entry.victim != player_guid);
            !threats.is_empty()
        });
        if direct_session_id.is_none() {
            direct_packets.clear();
        }
        Ok(PlayerDeathCleanupPackets {
            direct_packets,
            observer_packets,
        })
    }

    pub(in crate::world) fn advance_player_death_presentations(
        &mut self,
        now: Instant,
    ) -> anyhow::Result<Vec<(SessionId, OutboundWorldPacket)>> {
        let pending = self
            .pending_player_death_presentations
            .iter()
            .map(|(guid, presentation)| (*guid, *presentation))
            .collect::<Vec<_>>();
        let mut packets = Vec::new();
        for (character_guid, presentation) in pending {
            let force = now.saturating_duration_since(presentation.waiting_since)
                >= AIRBORNE_DEATH_PRESENTATION_FALLBACK;
            packets.extend(self.present_player_death_if_ready(character_guid, now, force)?);
        }
        Ok(packets)
    }

    pub(in crate::world) fn force_player_death_presentation(
        &mut self,
        character_guid: u32,
        now: Instant,
    ) -> anyhow::Result<Vec<(SessionId, OutboundWorldPacket)>> {
        self.present_player_death_if_ready(character_guid, now, true)
    }

    pub(in crate::world) fn present_player_death_if_ready(
        &mut self,
        character_guid: u32,
        _now: Instant,
        force: bool,
    ) -> anyhow::Result<Vec<(SessionId, OutboundWorldPacket)>> {
        let player_guid = ObjectGuid::new(HighGuid::Player, 0, character_guid);
        let Some(player) = self.players.get_mut(&character_guid) else {
            self.pending_player_death_presentations
                .remove(&character_guid);
            return Ok(Vec::new());
        };
        if player.health != 0 || player.death_state == PlayerDeathState::Alive {
            self.pending_player_death_presentations
                .remove(&character_guid);
            return Ok(Vec::new());
        }
        if player.death_state == PlayerDeathState::Corpse {
            self.pending_player_death_presentations
                .remove(&character_guid);
            return Ok(Vec::new());
        }
        let airborne = player_runtime_is_airborne(player);
        if airborne && !force {
            return Ok(Vec::new());
        }
        if force && airborne {
            if let Some(ground_position) = self.geometry.ground_position(player.position) {
                player.position = ground_position;
            }
        }
        clear_player_fall_state_for_death_presentation(player);
        player.death_state = PlayerDeathState::Corpse;
        player.stand_state = PLAYER_STAND_STATE_DEAD;
        let position = player.position;
        let flags = player.flags;
        let class = player.class;
        let direct_session_id = player.client_session_id();
        self.pending_player_death_presentations
            .remove(&character_guid);

        let death_packet = OutboundWorldPacket {
            opcode: SMSG_UPDATE_OBJECT,
            body: build_player_death_update_body(
                player_guid,
                0,
                flags,
                PLAYER_FIELD_BYTE_RELEASE_TIMER,
                player_unit_flags(false),
                class,
                PLAYER_STAND_STATE_DEAD,
            )?,
        };
        let mut packets = Vec::new();
        if let Some(session_id) = direct_session_id {
            packets.push((
                session_id,
                OutboundWorldPacket {
                    opcode: SMSG_FORCE_MOVE_ROOT,
                    body: build_force_move_root_body(player_guid, 0)?,
                },
            ));
            packets.push((session_id, death_packet.clone()));
        }
        packets.extend(
            self.nearby_player_guids(
                position,
                PLAYER_VISIBILITY_RADIUS_YARDS,
                Some(character_guid),
            )
            .into_iter()
            .filter_map(|observer_guid| {
                self.players
                    .get(&observer_guid)
                    .and_then(|observer| observer.packet_to_client(death_packet.clone()))
            }),
        );
        Ok(packets)
    }
}

pub(in crate::world) fn apply_player_runtime_world_damage(
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
        || player.flags & PLAYER_FLAGS_GM != 0
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
    let death_presentation_deferred = died && player_runtime_is_airborne(player);
    let mut direct_packets = Vec::new();
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
        player.death_state = if death_presentation_deferred {
            PlayerDeathState::JustDied
        } else {
            PlayerDeathState::Corpse
        };
        player.active_combat_target = None;
        player.active_combat_next_swing_at = None;
        player.ranged_auto_attack_next_shot_at = None;
        player.queued_next_melee_spell = None;
        player.combo_target = None;
        player.combo_points = 0;
        player.looting = false;
        if !death_presentation_deferred {
            player.stand_state = PLAYER_STAND_STATE_DEAD;
            direct_packets.push(OutboundWorldPacket {
                opcode: SMSG_FORCE_MOVE_ROOT,
                body: build_force_move_root_body(target, 0)?,
            });
        }
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
    let health_packet = if died && !death_presentation_deferred {
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
        observer_packets: Vec::new(),
        health_packet,
        aura_packet,
        death_presentation_deferred,
    }))
}

pub(in crate::world) fn player_runtime_is_airborne(player: &PlayerRuntime) -> bool {
    player.movement_flags & MOVEFLAG_JUMPING != 0
        || (player.fall_time > 0 && player.last_fall_z.is_some())
}

pub(in crate::world) fn clear_player_fall_state_for_death_presentation(player: &mut PlayerRuntime) {
    player.movement_flags &= !MOVEFLAG_JUMPING;
    player.fall_time = 0;
    player.last_fall_z = None;
    player.last_fall_time = 0;
    player.jump = JumpInfo::default();
}

#[derive(Default)]
pub(in crate::world) struct PlayerDeathCleanupPackets {
    pub(in crate::world) direct_packets: Vec<OutboundWorldPacket>,
    pub(in crate::world) observer_packets: Vec<(SessionId, OutboundWorldPacket)>,
}

impl MapRuntime {
    pub(in crate::world) fn apply_creature_world_damage(
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

pub(in crate::world) fn apply_creature_runtime_world_damage(
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
