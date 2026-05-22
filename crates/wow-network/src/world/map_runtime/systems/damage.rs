use super::*;
use wow_proto::world::WorldOpcode;

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
    pub(in crate::world) absorbed_damage: u32,
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
        self.apply_player_world_damage_with_school_mask(
            target,
            source,
            requested_damage,
            kind,
            SPELL_SCHOOL_MASK_NORMAL,
            now,
        )
    }

    pub(in crate::world) fn apply_player_world_damage_with_school_mask(
        &mut self,
        target: ObjectGuid,
        source: Option<ObjectGuid>,
        requested_damage: u32,
        kind: WorldDamageKind,
        school_mask: u32,
        now: Instant,
    ) -> anyhow::Result<Option<AppliedPlayerWorldDamage>> {
        let Some(player) = self.players.get_mut(&target.counter()) else {
            return Ok(None);
        };
        let mut applied = apply_player_runtime_world_damage_with_school_mask(
            player,
            target,
            source,
            requested_damage,
            kind,
            school_mask,
            now,
        )?;
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
        let spell_cleanup = self.clear_player_active_spell_runtime(character_guid)?;
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
        direct_packets.extend(spell_cleanup.direct_packets);
        observer_packets.extend(spell_cleanup.observer_packets);
        let player_combat_flags = OutboundWorldPacket {
            opcode: WorldOpcode::SmsgUpdateObject as u16,
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
                    opcode: WorldOpcode::SmsgAttackStop as u16,
                    body: build_attack_stop_body(attacker, player_guid, false)?,
                },
                OutboundWorldPacket {
                    opcode: WorldOpcode::SmsgUpdateObject as u16,
                    body: build_unit_flags_update_body(
                        attacker,
                        db_creature_unit_flags(&creature, false),
                    )?,
                },
                OutboundWorldPacket {
                    opcode: WorldOpcode::SmsgUpdateObject as u16,
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
                    opcode: WorldOpcode::SmsgMonsterMove as u16,
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
        player.power2 = 0;
        let position = player.position;
        let flags = player.flags;
        let race = player.race;
        let class = player.class;
        let direct_session_id = player.client_session_id();
        self.pending_player_death_presentations
            .remove(&character_guid);

        let death_packet = OutboundWorldPacket {
            opcode: WorldOpcode::SmsgUpdateObject as u16,
            body: build_player_death_update_body(PlayerDeathUpdate {
                player: player_guid,
                health: 0,
                player_flags: flags,
                field_bytes: PLAYER_FIELD_BYTE_RELEASE_TIMER,
                unit_flags: player_unit_flags(false),
                race,
                class,
                stand_state: PLAYER_STAND_STATE_DEAD,
            })?,
        };
        let mut packets = Vec::new();
        if let Some(session_id) = direct_session_id {
            packets.push((
                session_id,
                OutboundWorldPacket {
                    opcode: WorldOpcode::SmsgForceMoveRoot as u16,
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
    apply_player_runtime_world_damage_with_school_mask(
        player,
        target,
        source,
        requested_damage,
        kind,
        SPELL_SCHOOL_MASK_NORMAL,
        now,
    )
}

pub(in crate::world) fn apply_player_runtime_world_damage_with_school_mask(
    player: &mut PlayerRuntime,
    target: ObjectGuid,
    source: Option<ObjectGuid>,
    requested_damage: u32,
    kind: WorldDamageKind,
    school_mask: u32,
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

    let absorb = absorb_player_runtime_damage(player, requested_damage, school_mask);
    let requested_damage_after_absorb = requested_damage.saturating_sub(absorb.absorbed);
    let previous_health = player.health;
    let applied_damage = requested_damage_after_absorb.min(previous_health);
    player.health = previous_health.saturating_sub(applied_damage);
    player.environment.last_damage_at = Some(now);
    let died = player.health == 0;
    let position = player.position;
    let direct_session_id = player.client_session_id();
    let death_presentation_deferred = died && player_runtime_is_airborne(player);
    let mut direct_packets = Vec::new();
    if absorb.mana_spent > 0 {
        direct_packets.push(OutboundWorldPacket {
            opcode: WorldOpcode::SmsgUpdateObject as u16,
            body: build_player_mana_update_body(target, player.power1)?,
        });
    }
    let aura_packet = if died && !player.active_auras.is_empty() {
        player.active_auras.clear();
        Some(OutboundWorldPacket {
            opcode: WorldOpcode::SmsgUpdateObject as u16,
            body: build_player_aura_update_body(target, &player.active_auras)?,
        })
    } else if absorb.aura_changed {
        Some(OutboundWorldPacket {
            opcode: WorldOpcode::SmsgUpdateObject as u16,
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
        player.power2 = 0;
        if !death_presentation_deferred {
            player.stand_state = PLAYER_STAND_STATE_DEAD;
            direct_packets.push(OutboundWorldPacket {
                opcode: WorldOpcode::SmsgForceMoveRoot as u16,
                body: build_force_move_root_body(target, 0)?,
            });
        }
        let source_guid = source.map(|guid| format!("0x{:016X}", guid.raw()));
        warn!(
            target = format_args!("0x{:016X}", target.raw()),
            source = source_guid.as_deref(),
            ?kind,
            requested_damage,
            absorbed_damage = absorb.absorbed,
            applied_damage,
            old_health = previous_health,
            "MapRuntime finalized player death from world damage"
        );
    }
    let remaining_health = player.health;
    let health_packet = if died && !death_presentation_deferred {
        OutboundWorldPacket {
            opcode: WorldOpcode::SmsgUpdateObject as u16,
            body: build_player_death_update_body(PlayerDeathUpdate {
                player: target,
                health: 0,
                player_flags: player.flags,
                field_bytes: PLAYER_FIELD_BYTE_RELEASE_TIMER,
                unit_flags: player_unit_flags(false),
                race: player.race,
                class: player.class,
                stand_state: PLAYER_STAND_STATE_DEAD,
            })?,
        }
    } else {
        OutboundWorldPacket {
            opcode: WorldOpcode::SmsgUpdateObject as u16,
            body: build_player_health_update_body(target, remaining_health)?,
        }
    };
    Ok(Some(AppliedPlayerWorldDamage {
        target,
        source,
        kind,
        requested_damage,
        applied_damage,
        absorbed_damage: absorb.absorbed,
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

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(in crate::world) struct PlayerDamageAbsorb {
    pub(in crate::world) absorbed: u32,
    pub(in crate::world) mana_spent: u32,
    pub(in crate::world) aura_changed: bool,
}

pub(in crate::world) fn absorb_player_runtime_damage(
    player: &mut PlayerRuntime,
    requested_damage: u32,
    school_mask: u32,
) -> PlayerDamageAbsorb {
    let mut remaining_damage = requested_damage;
    let mut result = PlayerDamageAbsorb::default();
    for aura in &mut player.active_auras {
        if remaining_damage == 0 {
            break;
        }
        for modifier in &mut aura.stat_modifiers {
            let AuraStatModifier::SchoolAbsorb {
                school_mask: aura_school_mask,
                amount,
            } = modifier
            else {
                continue;
            };
            if *amount <= 0 || (*aura_school_mask & school_mask) == 0 {
                continue;
            }
            let absorbed = remaining_damage.min(*amount as u32);
            *amount -= absorbed as i32;
            remaining_damage -= absorbed;
            result.absorbed = result.absorbed.saturating_add(absorbed);
            result.aura_changed = true;
            if remaining_damage == 0 {
                break;
            }
        }
    }

    for aura in &mut player.active_auras {
        if remaining_damage == 0 {
            break;
        }
        for modifier in &mut aura.stat_modifiers {
            let AuraStatModifier::ManaShield {
                school_mask: aura_school_mask,
                amount,
                mana_multiplier_millis,
            } = modifier
            else {
                continue;
            };
            if *amount <= 0 || (*aura_school_mask & school_mask) == 0 {
                continue;
            }
            let mut absorbed = remaining_damage.min(*amount as u32);
            if *mana_multiplier_millis > 0 {
                let max_absorb = ((u64::from(player.power1) * 1000)
                    / u64::from(*mana_multiplier_millis))
                .min(u64::from(u32::MAX)) as u32;
                absorbed = absorbed.min(max_absorb);
                let mana_spent = ((u64::from(absorbed) * u64::from(*mana_multiplier_millis)) / 1000)
                    .min(u64::from(player.power1)) as u32;
                player.power1 = player.power1.saturating_sub(mana_spent);
                result.mana_spent = result.mana_spent.saturating_add(mana_spent);
            }
            if absorbed == 0 {
                continue;
            }
            *amount -= absorbed as i32;
            remaining_damage -= absorbed;
            result.absorbed = result.absorbed.saturating_add(absorbed);
            result.aura_changed = true;
            if remaining_damage == 0 {
                break;
            }
        }
    }

    if result.aura_changed {
        player.active_auras.retain(|aura| {
            !aura.stat_modifiers.iter().any(|modifier| {
                matches!(
                    modifier,
                    AuraStatModifier::SchoolAbsorb { amount, .. }
                        | AuraStatModifier::ManaShield { amount, .. }
                        if *amount <= 0
                )
            })
        });
    }

    result
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
        let applied = apply_creature_runtime_world_damage(
            creature,
            target,
            source,
            requested_damage,
            kind,
            now,
            now_epoch_secs,
        )?;
        if applied.is_some() {
            self.sync_db_creature_lifecycle_tracking(target.raw());
            self.sync_db_creature_ooc_event_ai_tracking(target.raw(), now);
        }
        Ok(applied)
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
