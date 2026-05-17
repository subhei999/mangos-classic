use super::*;

// Shared DB-creature combat claim and player-damage authority.
pub(in crate::world) const DB_CREATURE_DEFAULT_PURSUIT_MILLIS: u32 = 15_000;
pub(in crate::world) const DB_CREATURE_SPELL_LIST_UPDATE_MILLIS: u64 = 1_200;
pub(in crate::world) const CREATURE_SPELL_LIST_FLAG_SUPPORT_ACTION: u32 = 0x1;
pub(in crate::world) const CREATURE_SPELL_LIST_FLAG_RANGED_ACTION: u32 = 0x2;
pub(in crate::world) const CREATURE_SPELL_LIST_FLAG_CATEGORY_COOLDOWN: u32 = 0x4;
pub(in crate::world) const CREATURE_SPELL_LIST_FLAG_NON_BLOCKING: u32 = 0x8;
pub(in crate::world) const CREATURE_SPELL_CATEGORY_COOLDOWN_KEY: u32 = 0x8000_0000;
pub(in crate::world) const CREATURE_SPELL_LIST_TARGETING_HARDCODED: u32 = 0;
pub(in crate::world) const CREATURE_SPELL_LIST_TARGETING_ATTACK: u32 = 1;
pub(in crate::world) const CREATURE_SPELL_LIST_TARGETING_SUPPORT: u32 = 2;
pub(in crate::world) const CREATURE_SPELL_LIST_TARGET_CURRENT: u32 = 1;
pub(in crate::world) const CREATURE_SPELL_LIST_TARGET_SELF: u32 = 2;
pub(in crate::world) const CREATURE_SPELL_LIST_TARGET_CURRENT_NOT_ALONE: u32 = 7;
pub(in crate::world) const CREATURE_ATTACKING_TARGET_RANDOM: i32 = 0;
pub(in crate::world) const CREATURE_ATTACKING_TARGET_TOP_AGGRO: i32 = 1;
pub(in crate::world) const CREATURE_ATTACKING_TARGET_BOTTOM_AGGRO: i32 = 2;
pub(in crate::world) const CREATURE_ATTACKING_TARGET_NEAREST: i32 = 3;
pub(in crate::world) const CREATURE_ATTACKING_TARGET_FARTHEST: i32 = 4;
pub(in crate::world) const EVENT_AI_EVENT_TIMER_IN_COMBAT: u8 = 0;
pub(in crate::world) const EVENT_AI_EVENT_TIMER_OOC: u8 = 1;
pub(in crate::world) const EVENT_AI_EVENT_HP: u8 = 2;
pub(in crate::world) const EVENT_AI_EVENT_AGGRO: u8 = 4;
pub(in crate::world) const EVENT_AI_EVENT_SPAWNED: u8 = 11;
pub(in crate::world) const EVENT_AI_EVENT_RANGE: u8 = 9;
pub(in crate::world) const EVENT_AI_EVENT_MISSING_AURA: u8 = 27;
pub(in crate::world) const EVENT_AI_EVENT_FACING_TARGET: u8 = 33;
pub(in crate::world) const EVENT_AI_ACTION_CAST: u8 = 11;
pub(in crate::world) const EVENT_AI_ACTION_FLEE_FOR_ASSIST: u8 = 25;
pub(in crate::world) const EVENT_AI_ACTION_SET_WALK: u8 = 58;
pub(in crate::world) const EVENT_AI_FLAG_REPEATABLE: u32 = 0x01;
pub(in crate::world) const EVENT_AI_WALK_SETTING_RUN_DEFAULT: i32 = 0;
pub(in crate::world) const EVENT_AI_WALK_SETTING_WALK_DEFAULT: i32 = 1;
pub(in crate::world) const EVENT_AI_WALK_SETTING_RUN_CHASE: i32 = 2;
pub(in crate::world) const EVENT_AI_WALK_SETTING_WALK_CHASE: i32 = 3;
pub(in crate::world) const EVENT_AI_TARGET_SELF: i32 = 0;
pub(in crate::world) const EVENT_AI_TARGET_HOSTILE: i32 = 1;
pub(in crate::world) const EVENT_AI_TARGET_HOSTILE_SECOND_AGGRO: i32 = 2;
pub(in crate::world) const EVENT_AI_TARGET_HOSTILE_LAST_AGGRO: i32 = 3;
pub(in crate::world) const EVENT_AI_TARGET_HOSTILE_RANDOM: i32 = 4;
pub(in crate::world) const EVENT_AI_TARGET_HOSTILE_RANDOM_NOT_TOP: i32 = 5;
pub(in crate::world) const EVENT_AI_TARGET_ACTION_INVOKER: i32 = 6;
pub(in crate::world) const EVENT_AI_TARGET_ACTION_INVOKER_OWNER: i32 = 7;
pub(in crate::world) const EVENT_AI_TARGET_HOSTILE_RANDOM_PLAYER: i32 = 8;
pub(in crate::world) const EVENT_AI_TARGET_HOSTILE_RANDOM_NOT_TOP_PLAYER: i32 = 9;
pub(in crate::world) const EVENT_AI_TARGET_PLAYER_INVOKER: i32 = 13;
pub(in crate::world) const EVENT_AI_TARGET_PLAYER_TAPPED: i32 = 14;
pub(in crate::world) const EVENT_AI_TARGET_NONE: i32 = 15;
pub(in crate::world) const EVENT_AI_TARGET_HOSTILE_RANDOM_MANA: i32 = 16;
pub(in crate::world) const EVENT_AI_TARGET_NEAREST_AOE_TARGET: i32 = 17;
pub(in crate::world) const EVENT_AI_TARGET_HOSTILE_FARTHEST_AWAY: i32 = 18;
pub(in crate::world) const EVENT_AI_SPAWNED_ALWAYS: i32 = 0;
pub(in crate::world) const EVENT_AI_SPAWNED_MAP: i32 = 1;
pub(in crate::world) const EVENT_AI_SPAWNED_ZONE: i32 = 2;
pub(in crate::world) const CMANGOS_CREATURE_FAMILY_FLEE_DELAY: Duration =
    Duration::from_millis(10_000);
pub(in crate::world) const UNIT_CONDITION_FLAG_OR: u32 = 0x1;
pub(in crate::world) const CONDITION_LOGIC_NONE: i32 = 0;
pub(in crate::world) const CONDITION_LOGIC_AND: i32 = 1;
pub(in crate::world) const CONDITION_LOGIC_OR: i32 = 2;
pub(in crate::world) const CONDITION_LOGIC_XOR: i32 = 3;
pub(in crate::world) const UNIT_CONDITION_NONE: u32 = 0;
pub(in crate::world) const UNIT_CONDITION_RACE: u32 = 1;
pub(in crate::world) const UNIT_CONDITION_CLASS: u32 = 2;
pub(in crate::world) const UNIT_CONDITION_LEVEL: u32 = 3;
pub(in crate::world) const UNIT_CONDITION_IS_SELF: u32 = 4;
pub(in crate::world) const UNIT_CONDITION_IS_TARGET: u32 = 7;
pub(in crate::world) const UNIT_CONDITION_HEALTH_PERCENT: u32 = 12;
pub(in crate::world) const UNIT_CONDITION_MANA_PERCENT: u32 = 13;
pub(in crate::world) const UNIT_CONDITION_RAGE_PERCENT: u32 = 14;
pub(in crate::world) const UNIT_CONDITION_ENERGY_PERCENT: u32 = 15;
pub(in crate::world) const UNIT_CONDITION_IN_COMBAT: u32 = 31;
pub(in crate::world) const UNIT_CONDITION_NUMBER_OF_MELEE_ATTACKERS: u32 = 37;
pub(in crate::world) const UNIT_CONDITION_IS_ATTACKING_ME: u32 = 38;
pub(in crate::world) const UNIT_CONDITION_RANGE: u32 = 39;
pub(in crate::world) const UNIT_CONDITION_IN_MELEE_RANGE: u32 = 40;
pub(in crate::world) const UNIT_CONDITION_NUMBER_OF_ENEMIES: u32 = 44;
pub(in crate::world) const UNIT_CONDITION_NUMBER_OF_ATTACKERS: u32 = 54;
pub(in crate::world) const UNIT_CONDITION_NUMBER_OF_RANGED_ATTACKERS: u32 = 55;
pub(in crate::world) const UNIT_CONDITION_CREATURE_TYPE: u32 = 56;
pub(in crate::world) const UNIT_CONDITION_IS_MELEE_ATTACKING: u32 = 57;
pub(in crate::world) const UNIT_CONDITION_IS_RANGED_ATTACKING: u32 = 58;
pub(in crate::world) const UNIT_CONDITION_HEALTH: u32 = 59;
pub(in crate::world) const UNIT_CONDITION_IS_INTERRUPTIBLE: u32 = 53;
pub(in crate::world) const UNIT_CONDITION_IS_PLAYER: u32 = 63;
pub(in crate::world) const UNIT_CONDITION_CREATURE_ID: u32 = 74;
pub(in crate::world) const UNIT_CONDITION_IS_ENEMY: u32 = 77;
pub(in crate::world) const UNIT_CONDITION_IS_DYING: u32 = 83;

#[derive(Debug, Clone, Default)]
pub(in crate::world) struct DbCreatureSpellConditionCache {
    pub(in crate::world) unit_conditions:
        std::collections::HashMap<i32, wow_db::UnitConditionQuery>,
    pub(in crate::world) combat_conditions:
        std::collections::HashMap<i32, wow_db::CombatConditionQuery>,
}

#[derive(Debug, Clone, Copy)]
pub(in crate::world) struct DbCreatureCombatConditionCountClause {
    pub(in crate::world) ids: [i32; 2],
    pub(in crate::world) ops: [i32; 2],
    pub(in crate::world) counts: [i32; 2],
    pub(in crate::world) logic: i32,
}

#[derive(Debug, Clone)]
pub(in crate::world) struct DbCreaturePlayerMeleeValidation {
    pub(in crate::world) check: PlayerMeleeCheck,
}

#[derive(Debug, Clone)]
pub(in crate::world) struct PlayerChargeValidation {
    pub(in crate::world) check: PlayerChargeCheck,
}

#[derive(Debug, Clone)]
pub(in crate::world) struct PlayerSpellTargetValidation {
    pub(in crate::world) check: PlayerSpellTargetCheck,
}

#[derive(Debug, Clone)]
pub(in crate::world) struct DbCreatureSpellTargetValidation {
    pub(in crate::world) check: DbCreatureSpellTargetCheck,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::world) enum PlayerChargeCheck {
    Clear,
    NoActiveCharacter,
    MissingTarget,
    TargetNotAlive,
    NavigationBlocked(DbCreatureNavigationResult),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::world) enum PlayerSpellTargetCheck {
    Clear,
    NoActiveCharacter,
    MissingTarget,
    TargetNotAlive,
    NavigationBlocked(DbCreatureNavigationResult),
    OutOfRange,
    TooClose,
    BadFacing,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::world) enum DbCreatureSpellTargetCheck {
    Clear,
    MissingCaster,
    CasterNotAlive,
    MissingTarget,
    TargetNotAlive,
    NavigationBlocked(DbCreatureNavigationResult),
    OutOfRange,
    TooClose,
    NotBehind,
}

pub(in crate::world) fn spell_unit_target_range_bounds(
    range: SpellRangeEntry,
    caster_combat_reach: f32,
    target_combat_reach: f32,
) -> (f32, f32) {
    let caster_combat_reach = caster_combat_reach.max(0.0);
    let target_combat_reach = target_combat_reach.max(0.0);
    let range_mod = caster_combat_reach + target_combat_reach;
    let min_range = if range.min_range > 0.0 {
        if range.flags & SPELL_RANGE_FLAG_RANGED != 0 {
            range.min_range + combined_melee_reach(caster_combat_reach, target_combat_reach)
        } else if range.flags & SPELL_RANGE_FLAG_MELEE == 0 {
            range.min_range + range_mod
        } else {
            range.min_range
        }
    } else {
        range.min_range
    };
    let max_range = if range.max_range > 0.0 {
        range.max_range + range_mod
    } else {
        range.max_range
    };
    (min_range, max_range)
}

#[derive(Debug, Clone)]
pub(in crate::world) struct ActiveDbCreatureCombatSnapshot {
    pub(in crate::world) combat: CreatureCombatState,
    pub(in crate::world) creature: DbCreatureRuntime,
}

#[derive(Debug, Clone)]
pub(in crate::world) struct ReadyDbCreatureSpellCast {
    pub(in crate::world) spell: wow_db::CreatureSpellListQuery,
    pub(in crate::world) target: ObjectGuid,
}

#[derive(Debug, Clone)]
pub(in crate::world) struct ReadyDbCreatureEventAiSpellCast {
    pub(in crate::world) script_id: i32,
    pub(in crate::world) spell_id: u32,
    pub(in crate::world) target: ObjectGuid,
    pub(in crate::world) repeat_min: i32,
    pub(in crate::world) repeat_max: i32,
}

impl MapRuntime {
    fn set_player_in_combat_from_creature_refs(&mut self, player: ObjectGuid) {
        if !player.is_player() {
            return;
        }
        let in_combat = self
            .active_creature_combats
            .values()
            .any(|combat| combat.victim == player);
        if let Some(runtime) = self.players.get_mut(&player.counter()) {
            runtime.in_combat = in_combat;
        }
    }

    pub(in crate::world) fn db_creature_combat_snapshot(
        &self,
        creature_guid: ObjectGuid,
    ) -> Option<DbCreatureRuntime> {
        self.creatures
            .get(&creature_guid.raw())
            .filter(|creature| creature.is_alive() && !creature.is_evading_home())
            .cloned()
    }

    pub(in crate::world) fn validate_player_melee_against_db_creature(
        &self,
        character_guid: u32,
        target: ObjectGuid,
        navigation: &DbCreatureNavigationGuardrail,
    ) -> DbCreaturePlayerMeleeValidation {
        let Some(player) = self.players.get(&character_guid) else {
            return DbCreaturePlayerMeleeValidation {
                check: PlayerMeleeCheck::NoActiveCharacter,
            };
        };
        let Some(creature) = self.creatures.get(&target.raw()).cloned() else {
            return DbCreaturePlayerMeleeValidation {
                check: PlayerMeleeCheck::MissingTarget,
            };
        };
        if !creature.is_alive() {
            return DbCreaturePlayerMeleeValidation {
                check: PlayerMeleeCheck::TargetNotAlive,
            };
        }
        if creature.is_evading_home() {
            return DbCreaturePlayerMeleeValidation {
                check: PlayerMeleeCheck::TargetEvading,
            };
        }
        let reach = combined_melee_reach(PLAYER_COMBAT_REACH_YARDS, creature.combat_reach());
        let dx = player.position.x - creature.current_position.x;
        let dy = player.position.y - creature.current_position.y;
        let dz = player.position.z - creature.current_position.z;
        if dx * dx + dy * dy + dz * dz > reach * reach {
            return DbCreaturePlayerMeleeValidation {
                check: PlayerMeleeCheck::OutOfRange,
            };
        }
        let navigation_check =
            db_creature_navigation_check(navigation, player.position, creature.current_position);
        if !navigation_check.is_clear() {
            return DbCreaturePlayerMeleeValidation {
                check: PlayerMeleeCheck::NavigationBlocked(navigation_check),
            };
        }
        if !has_in_arc(
            player.position,
            creature.current_position,
            PLAYER_MELEE_ARC_RADIANS,
        ) {
            return DbCreaturePlayerMeleeValidation {
                check: PlayerMeleeCheck::BadFacing,
            };
        }
        DbCreaturePlayerMeleeValidation {
            check: PlayerMeleeCheck::Clear,
        }
    }

    pub(in crate::world) fn validate_player_charge_against_db_creature(
        &self,
        character_guid: u32,
        target: ObjectGuid,
        navigation: &DbCreatureNavigationGuardrail,
    ) -> PlayerChargeValidation {
        let Some(player) = self.players.get(&character_guid) else {
            return PlayerChargeValidation {
                check: PlayerChargeCheck::NoActiveCharacter,
            };
        };
        let Some(creature) = self.creatures.get(&target.raw()).cloned() else {
            return PlayerChargeValidation {
                check: PlayerChargeCheck::MissingTarget,
            };
        };
        if !creature.is_alive() || creature.is_evading_home() {
            return PlayerChargeValidation {
                check: PlayerChargeCheck::TargetNotAlive,
            };
        }
        let destination = charge_destination(player.position, &creature);
        let navigation_check =
            player_charge_navigation_check(navigation, player.position, destination);
        if !navigation_check.is_clear() {
            return PlayerChargeValidation {
                check: PlayerChargeCheck::NavigationBlocked(navigation_check),
            };
        }
        PlayerChargeValidation {
            check: PlayerChargeCheck::Clear,
        }
    }

    pub(in crate::world) fn validate_player_spell_against_db_creature(
        &self,
        character_guid: u32,
        target: ObjectGuid,
        navigation: &DbCreatureNavigationGuardrail,
        range: Option<SpellRangeEntry>,
        requires_infront: bool,
    ) -> PlayerSpellTargetValidation {
        let Some(player) = self.players.get(&character_guid) else {
            return PlayerSpellTargetValidation {
                check: PlayerSpellTargetCheck::NoActiveCharacter,
            };
        };
        let Some(creature) = self.creatures.get(&target.raw()).cloned() else {
            return PlayerSpellTargetValidation {
                check: PlayerSpellTargetCheck::MissingTarget,
            };
        };
        if !creature.is_alive() || creature.is_evading_home() {
            return PlayerSpellTargetValidation {
                check: PlayerSpellTargetCheck::TargetNotAlive,
            };
        }
        let navigation_check =
            db_creature_navigation_check(navigation, player.position, creature.current_position);
        if !navigation_check.is_clear() {
            return PlayerSpellTargetValidation {
                check: PlayerSpellTargetCheck::NavigationBlocked(navigation_check),
            };
        }
        if let Some(range) = range {
            let dx = player.position.x - creature.current_position.x;
            let dy = player.position.y - creature.current_position.y;
            let dz = player.position.z - creature.current_position.z;
            let distance_squared = dx * dx + dy * dy + dz * dz;
            let (min_range, max_range) = spell_unit_target_range_bounds(
                range,
                PLAYER_COMBAT_REACH_YARDS,
                creature.combat_reach(),
            );
            if max_range > 0.0 && distance_squared > max_range * max_range {
                return PlayerSpellTargetValidation {
                    check: PlayerSpellTargetCheck::OutOfRange,
                };
            }
            if min_range > 0.0 && distance_squared < min_range * min_range {
                return PlayerSpellTargetValidation {
                    check: PlayerSpellTargetCheck::TooClose,
                };
            }
        }
        if requires_infront
            && !has_in_arc(
                player.position,
                creature.current_position,
                SPELL_CAST_ARC_RADIANS,
            )
        {
            return PlayerSpellTargetValidation {
                check: PlayerSpellTargetCheck::BadFacing,
            };
        }
        PlayerSpellTargetValidation {
            check: PlayerSpellTargetCheck::Clear,
        }
    }

    pub(in crate::world) fn validate_db_creature_spell_against_target(
        &self,
        caster_guid: ObjectGuid,
        target_guid: ObjectGuid,
        navigation: &DbCreatureNavigationGuardrail,
        range: Option<SpellRangeEntry>,
        requires_behind: bool,
    ) -> DbCreatureSpellTargetValidation {
        let Some(caster) = self.creatures.get(&caster_guid.raw()) else {
            return DbCreatureSpellTargetValidation {
                check: DbCreatureSpellTargetCheck::MissingCaster,
            };
        };
        if !caster.is_alive() || caster.is_evading_home() {
            return DbCreatureSpellTargetValidation {
                check: DbCreatureSpellTargetCheck::CasterNotAlive,
            };
        }
        let Some(target_position) = self.db_creature_spell_target_position(target_guid) else {
            return DbCreatureSpellTargetValidation {
                check: DbCreatureSpellTargetCheck::MissingTarget,
            };
        };
        if !self.db_creature_spell_cast_target_alive(target_guid) {
            return DbCreatureSpellTargetValidation {
                check: DbCreatureSpellTargetCheck::TargetNotAlive,
            };
        }
        if caster_guid != target_guid {
            let navigation_check =
                db_creature_navigation_check(navigation, caster.current_position, target_position);
            if !navigation_check.is_clear() {
                return DbCreatureSpellTargetValidation {
                    check: DbCreatureSpellTargetCheck::NavigationBlocked(navigation_check),
                };
            }
        }
        if let Some(range) = range {
            let dx = caster.current_position.x - target_position.x;
            let dy = caster.current_position.y - target_position.y;
            let dz = caster.current_position.z - target_position.z;
            let distance_squared = dx * dx + dy * dy + dz * dz;
            let (min_range, max_range) = spell_unit_target_range_bounds(
                range,
                caster.combat_reach(),
                self.db_creature_spell_target_combat_reach(target_guid),
            );
            if max_range > 0.0 && distance_squared > max_range * max_range {
                return DbCreatureSpellTargetValidation {
                    check: DbCreatureSpellTargetCheck::OutOfRange,
                };
            }
            if min_range > 0.0 && distance_squared < min_range * min_range {
                return DbCreatureSpellTargetValidation {
                    check: DbCreatureSpellTargetCheck::TooClose,
                };
            }
        }
        if requires_behind
            && !db_creature_is_facing_targets_back(caster.current_position, target_position)
        {
            return DbCreatureSpellTargetValidation {
                check: DbCreatureSpellTargetCheck::NotBehind,
            };
        }
        DbCreatureSpellTargetValidation {
            check: DbCreatureSpellTargetCheck::Clear,
        }
    }

    pub(in crate::world) fn active_db_creature_combat_snapshot(
        &mut self,
        attacker: ObjectGuid,
        victim: ObjectGuid,
    ) -> Option<ActiveDbCreatureCombatSnapshot> {
        let combat = self.active_creature_combats.get(&attacker.raw()).copied()?;
        if combat.victim != victim {
            return None;
        }
        let Some(creature) = self.db_creature_combat_snapshot(attacker) else {
            self.clear_db_creature_combat(attacker);
            return None;
        };
        Some(ActiveDbCreatureCombatSnapshot { combat, creature })
    }

    pub(in crate::world) fn process_db_creature_event_ai_hp_actions(
        &mut self,
        navigation: &DbCreatureNavigationGuardrail,
        attacker: ObjectGuid,
        victim: ObjectGuid,
        scripts: &[wow_db::CreatureAiScriptQuery],
        now: Instant,
        exclude_character_guid: Option<u32>,
    ) -> anyhow::Result<Option<DbCreatureEventAiActionsEvent>> {
        let Some(source_position) = victim
            .is_player()
            .then(|| {
                self.players
                    .get(&victim.counter())
                    .map(|player| player.position)
            })
            .flatten()
        else {
            return Ok(None);
        };
        let Some(creature) = self.creatures.get(&attacker.raw()).cloned() else {
            return Ok(None);
        };
        if !creature.is_alive() || creature.is_fleeing() {
            return Ok(None);
        }
        let Some(combat) = self.active_creature_combats.get(&attacker.raw()) else {
            return Ok(None);
        };
        if combat.victim != victim {
            return Ok(None);
        }

        let mut direct_packets = Vec::new();
        let mut observer_packets = Vec::new();
        let mut triggered_script_ids = Vec::new();
        let triggerable_scripts = scripts
            .iter()
            .filter(|script| db_creature_event_ai_hp_script_can_trigger(&creature, script))
            .cloned()
            .collect::<Vec<_>>();
        for script in &triggerable_scripts {
            let mut script_executed = false;
            for action in db_creature_event_ai_actions(script) {
                match action.action_type {
                    EVENT_AI_ACTION_FLEE_FOR_ASSIST => {
                        if self
                            .creatures
                            .get(&attacker.raw())
                            .is_some_and(|c| c.is_fleeing())
                        {
                            continue;
                        }
                        let Some((creature, motion)) = self.start_db_creature_flee_motion(
                            navigation,
                            attacker,
                            victim,
                            source_position,
                            now,
                            CMANGOS_CREATURE_FAMILY_FLEE_DELAY,
                        ) else {
                            continue;
                        };
                        let unit_flags_body = build_unit_flags_update_body(
                            attacker,
                            db_creature_unit_flags(&creature, true),
                        )?;
                        let motion_body = build_monster_move_run_path_body(
                            attacker,
                            motion.start,
                            &motion.path,
                            motion.spline_id,
                            motion.duration.as_millis().max(1) as u32,
                        )?;
                        direct_packets.push(OutboundWorldPacket {
                            opcode: SMSG_UPDATE_OBJECT,
                            body: unit_flags_body.clone(),
                        });
                        direct_packets.push(OutboundWorldPacket {
                            opcode: SMSG_MONSTER_MOVE,
                            body: motion_body.clone(),
                        });
                        observer_packets.extend(self.db_creature_event_ai_observer_packets(
                            creature.current_position,
                            exclude_character_guid,
                            [
                                OutboundWorldPacket {
                                    opcode: SMSG_UPDATE_OBJECT,
                                    body: unit_flags_body,
                                },
                                OutboundWorldPacket {
                                    opcode: SMSG_MONSTER_MOVE,
                                    body: motion_body,
                                },
                            ],
                        ));
                        script_executed = true;
                    }
                    EVENT_AI_ACTION_SET_WALK => {
                        if let Some(packet) =
                            self.apply_db_creature_event_ai_set_walk(attacker, action.param1, now)?
                        {
                            let creature_position = self
                                .creatures
                                .get(&attacker.raw())
                                .map(|creature| creature.current_position)
                                .unwrap_or(source_position);
                            direct_packets.push(packet.clone());
                            observer_packets.extend(self.db_creature_event_ai_observer_packets(
                                creature_position,
                                exclude_character_guid,
                                [packet],
                            ));
                        }
                        script_executed = true;
                    }
                    _ => {}
                }
            }
            if script_executed && script.event_flags & EVENT_AI_FLAG_REPEATABLE == 0 {
                triggered_script_ids.push(script.id);
            }
        }

        if direct_packets.is_empty()
            && observer_packets.is_empty()
            && triggered_script_ids.is_empty()
        {
            return Ok(None);
        }
        let Some(creature) = self.creatures.get_mut(&attacker.raw()) else {
            return Ok(None);
        };
        creature
            .triggered_event_ai_scripts
            .extend(triggered_script_ids);
        Ok(Some(DbCreatureEventAiActionsEvent {
            creature: creature.clone(),
            direct_packets,
            observer_packets,
        }))
    }

    pub(in crate::world) fn ready_db_creature_event_ai_spell_cast(
        &mut self,
        attacker: ObjectGuid,
        victim: ObjectGuid,
        scripts: &[wow_db::CreatureAiScriptQuery],
        now: Instant,
    ) -> Option<ReadyDbCreatureEventAiSpellCast> {
        self.ready_db_creature_event_ai_spell_cast_inner(attacker, Some(victim), scripts, now)
    }

    pub(in crate::world) fn ready_db_creature_event_ai_ooc_spell_cast(
        &mut self,
        attacker: ObjectGuid,
        scripts: &[wow_db::CreatureAiScriptQuery],
        now: Instant,
    ) -> Option<ReadyDbCreatureEventAiSpellCast> {
        self.ready_db_creature_event_ai_spell_cast_inner(attacker, None, scripts, now)
    }

    fn ready_db_creature_event_ai_spell_cast_inner(
        &mut self,
        attacker: ObjectGuid,
        victim: Option<ObjectGuid>,
        scripts: &[wow_db::CreatureAiScriptQuery],
        now: Instant,
    ) -> Option<ReadyDbCreatureEventAiSpellCast> {
        if self
            .active_creature_spell_casts
            .contains_key(&attacker.raw())
        {
            return None;
        }
        match victim {
            Some(victim) => {
                let combat = self.active_creature_combats.get(&attacker.raw()).copied()?;
                if combat.victim != victim {
                    return None;
                }
            }
            None if self.active_creature_combats.contains_key(&attacker.raw()) => return None,
            None => {}
        }

        for script in scripts {
            let Some(action) = db_creature_event_ai_actions(script)
                .into_iter()
                .find(|action| action.action_type == EVENT_AI_ACTION_CAST && action.param1 > 0)
            else {
                continue;
            };
            let Some(target) =
                db_creature_event_ai_action_target(self, attacker, victim, action.param2)
            else {
                continue;
            };
            if !self.db_creature_event_ai_script_ready(attacker, victim, script, now) {
                continue;
            }
            return Some(ReadyDbCreatureEventAiSpellCast {
                script_id: script.id,
                spell_id: action.param1 as u32,
                target,
                repeat_min: script.event_param3,
                repeat_max: script.event_param4,
            });
        }
        None
    }

    pub(in crate::world) fn apply_db_creature_event_ai_spell_cooldown(
        &mut self,
        attacker: ObjectGuid,
        ready: &ReadyDbCreatureEventAiSpellCast,
        now: Instant,
    ) {
        let Some(creature) = self.creatures.get_mut(&attacker.raw()) else {
            return;
        };
        let repeat_millis = random_millis_between_i32(ready.repeat_min, ready.repeat_max);
        if repeat_millis > 0 {
            creature.event_ai_cooldowns_until.insert(
                ready.script_id,
                now + Duration::from_millis(repeat_millis as u64),
            );
        }
        creature.triggered_event_ai_scripts.insert(ready.script_id);
    }

    #[allow(clippy::too_many_arguments)]
    pub(in crate::world) fn prepare_db_creature_spell_cast_from_template(
        &self,
        caster: ObjectGuid,
        target: ObjectGuid,
        template: &wow_db::SpellTemplateQuery,
        duration: Option<SpellDurationEntry>,
        range: Option<SpellRangeEntry>,
        cast_time: Option<SpellCastTimeEntry>,
        now: Instant,
    ) -> Option<ActiveDbCreatureSpellCast> {
        let creature = self.creatures.get(&caster.raw())?;
        if !creature.is_alive() || creature.is_evading_home() {
            return None;
        }
        if (template.attributes_ex & SPELL_ATTR_EX_NO_AUTOCAST_AI) != 0
            || (template.attributes & SPELL_ATTR_PASSIVE) != 0
        {
            return None;
        }
        let spell_info = SpellInfo::from_template(template);
        let caster_level = creature
            .spawn
            .template
            .max_level
            .max(creature.spawn.template.min_level);
        let value_context =
            SpellEffectValueContext::with_spell_rank_level(template, (caster_level / 5) as i32, 0);
        let aura = (spell_info.has_effect(SpellEffectDispatch::ApplyAura)
            && (target.is_player() || target.is_creature()))
        .then(|| build_active_aura(template, caster, caster_level, value_context, now, duration));
        let effect = if spell_info.has_direct_damage_effect() {
            let amount = spell_info.direct_damage_with_context(value_context);
            if amount == 0 {
                return None;
            }
            ActiveDbCreatureSpellEffect::Damage {
                amount,
                school: template.school as u8,
                dmg_class: template.dmg_class,
                attributes_ex2: template.attributes_ex2,
                attributes_ex3: template.attributes_ex3,
            }
        } else if spell_info.has_direct_heal_effect() {
            let amount = spell_info.direct_heal();
            if amount == 0 {
                return None;
            }
            ActiveDbCreatureSpellEffect::Heal { amount }
        } else if aura.is_some() {
            ActiveDbCreatureSpellEffect::None
        } else {
            return None;
        };
        let mana_cost = if template.power_type == POWER_TYPE_MANA {
            template.mana_cost
        } else {
            0
        };
        let cast_time_millis = spell_cast_time_millis(cast_time);
        Some(ActiveDbCreatureSpellCast {
            caster,
            target,
            spell_id: template.id,
            requires_behind: spell_info.requires_behind_target(),
            effect,
            aura,
            range,
            mana_cost,
            cast_time_millis,
            due_at: now + Duration::from_millis(cast_time_millis as u64),
        })
    }

    fn db_creature_event_ai_script_ready(
        &mut self,
        attacker: ObjectGuid,
        victim: Option<ObjectGuid>,
        script: &wow_db::CreatureAiScriptQuery,
        now: Instant,
    ) -> bool {
        let event_ready = match script.event_type {
            EVENT_AI_EVENT_AGGRO => {
                let Some(creature) = self.creatures.get(&attacker.raw()) else {
                    return false;
                };
                db_creature_event_ai_common_ready(creature, script)
            }
            EVENT_AI_EVENT_SPAWNED => {
                let Some(creature) = self.creatures.get(&attacker.raw()) else {
                    return false;
                };
                db_creature_event_ai_common_ready(creature, script)
                    && db_creature_event_ai_spawned_condition(self, creature, script)
            }
            EVENT_AI_EVENT_TIMER_IN_COMBAT => {
                let Some(_victim) = victim else {
                    return false;
                };
                let Some(creature) = self.creatures.get_mut(&attacker.raw()) else {
                    return false;
                };
                db_creature_event_ai_common_ready(creature, script)
                    && db_creature_event_ai_timer_ready(creature, script, now)
            }
            EVENT_AI_EVENT_TIMER_OOC => {
                if victim.is_some() {
                    return false;
                }
                let Some(creature) = self.creatures.get_mut(&attacker.raw()) else {
                    return false;
                };
                db_creature_event_ai_common_ready(creature, script)
                    && db_creature_event_ai_timer_ready(creature, script, now)
            }
            EVENT_AI_EVENT_RANGE => {
                let Some(victim) = victim else {
                    return false;
                };
                let Some(creature) = self.creatures.get(&attacker.raw()) else {
                    return false;
                };
                db_creature_event_ai_common_ready(creature, script)
                    && db_creature_event_ai_repeating_ready(creature, script, now)
                    && db_creature_event_ai_range_condition(self, attacker, victim, script)
            }
            EVENT_AI_EVENT_FACING_TARGET => {
                let Some(victim) = victim else {
                    return false;
                };
                let Some(creature) = self.creatures.get(&attacker.raw()) else {
                    return false;
                };
                db_creature_event_ai_common_ready(creature, script)
                    && db_creature_event_ai_repeating_ready(creature, script, now)
                    && db_creature_event_ai_facing_condition(self, attacker, victim, script)
            }
            EVENT_AI_EVENT_MISSING_AURA => {
                let Some(creature) = self.creatures.get(&attacker.raw()) else {
                    return false;
                };
                db_creature_event_ai_common_ready(creature, script)
                    && db_creature_event_ai_repeating_ready(creature, script, now)
                    && db_creature_event_ai_missing_aura_condition(creature, script)
            }
            _ => false,
        };
        if !event_ready {
            return false;
        }
        script.event_chance >= 100 || rand::thread_rng().gen_range(0..100) < script.event_chance
    }

    fn db_creature_event_ai_observer_packets<const N: usize>(
        &self,
        position: WorldPosition,
        exclude_character_guid: Option<u32>,
        packets: [OutboundWorldPacket; N],
    ) -> Vec<(SessionId, OutboundWorldPacket)> {
        let mut observer_packets = Vec::new();
        for player_guid in self.nearby_player_guids(
            position,
            CREATURE_SPAWN_RADIUS_YARDS,
            exclude_character_guid,
        ) {
            let Some(player) = self.players.get(&player_guid) else {
                continue;
            };
            for packet in &packets {
                if let Some(packet) = player.packet_to_client(packet.clone()) {
                    observer_packets.push(packet);
                }
            }
        }
        observer_packets
    }

    fn apply_db_creature_event_ai_set_walk(
        &mut self,
        creature_guid: ObjectGuid,
        walk_setting: i32,
        now: Instant,
    ) -> anyhow::Result<Option<OutboundWorldPacket>> {
        let Some(creature) = self.creatures.get_mut(&creature_guid.raw()) else {
            return Ok(None);
        };
        match walk_setting {
            EVENT_AI_WALK_SETTING_RUN_DEFAULT => {
                creature.default_movement_run = true;
                return Ok(None);
            }
            EVENT_AI_WALK_SETTING_WALK_DEFAULT => {
                creature.default_movement_run = false;
                return Ok(None);
            }
            EVENT_AI_WALK_SETTING_RUN_CHASE | EVENT_AI_WALK_SETTING_WALK_CHASE => {}
            _ => return Ok(None),
        }

        let new_run = walk_setting == EVENT_AI_WALK_SETTING_RUN_CHASE;
        if creature.chase_run == new_run
            && !matches!(&creature.motion, CreatureMotionState::Chase(chase) if chase.run != new_run)
        {
            return Ok(None);
        }
        creature.chase_run = new_run;
        if let CreatureMotionState::Chase(chase) = &mut creature.motion {
            chase.run = new_run;
        }
        retime_db_creature_motion_for_speed_change(creature, now)
    }

    pub(in crate::world) fn ready_db_creature_spell_cast(
        &mut self,
        attacker: ObjectGuid,
        victim: ObjectGuid,
        spell_list: &[wow_db::CreatureSpellListQuery],
        conditions: &DbCreatureSpellConditionCache,
        now: Instant,
    ) -> Option<ReadyDbCreatureSpellCast> {
        if spell_list.is_empty()
            || self
                .active_creature_spell_casts
                .contains_key(&attacker.raw())
        {
            return None;
        }
        let combat = self.active_creature_combats.get(&attacker.raw()).copied()?;
        if combat.victim != victim {
            return None;
        }
        let (unavailable_positions, cooldowns_until) = {
            let creature = self.creatures.get_mut(&attacker.raw())?;
            if !creature.is_alive() || creature.is_evading_home() {
                return None;
            }
            refresh_db_creature_spell_list_availability(creature, spell_list);
            if creature.next_spell_list_update_at.is_none() {
                creature.next_spell_list_update_at =
                    Some(now + Duration::from_millis(DB_CREATURE_SPELL_LIST_UPDATE_MILLIS));
                initialize_db_creature_spell_cooldowns(creature, spell_list, now);
                return None;
            }
            if creature
                .next_spell_list_update_at
                .is_some_and(|due_at| now < due_at)
            {
                return None;
            }
            creature.next_spell_list_update_at =
                Some(now + Duration::from_millis(DB_CREATURE_SPELL_LIST_UPDATE_MILLIS));
            (
                creature.unavailable_spell_list_positions.clone(),
                creature.spell_cooldowns_until.clone(),
            )
        };

        let support_action_roll = rand::thread_rng().gen_range(0..=100);
        let ranged_action_roll = rand::thread_rng().gen_range(0..=100);
        let current_ranged_mode = !self.db_creature_threat_victim_in_melee(attacker, victim);

        let mut non_blocking = Vec::new();
        let mut eligible = spell_list
            .iter()
            .filter_map(|spell| {
                db_creature_spell_ai_target(self, attacker, spell, victim)
                    .map(|target| (spell, target))
            })
            .filter(|(spell, _)| !unavailable_positions.contains(&spell.position))
            .filter(|(spell, target)| {
                db_creature_spell_conditions_met(self, attacker, *target, spell, conditions)
            })
            .filter(|(spell, _)| {
                if (spell.flags & CREATURE_SPELL_LIST_FLAG_SUPPORT_ACTION) != 0 {
                    support_action_roll <= spell.chance_support_action
                } else {
                    true
                }
            })
            .filter(|(spell, _)| {
                if (spell.flags & CREATURE_SPELL_LIST_FLAG_RANGED_ACTION) != 0 {
                    let chance = if current_ranged_mode {
                        spell.chance_ranged_attack as i32
                    } else {
                        (spell.chance_ranged_attack as i32 - 50).max(0)
                    };
                    (ranged_action_roll as i32) <= chance
                } else {
                    true
                }
            })
            .filter(|(spell, _)| db_creature_spell_cooldown_ready(&cooldowns_until, spell, now))
            .map(|(spell, target)| (spell.clone(), target))
            .inspect(|(spell, target)| {
                if (spell.flags & CREATURE_SPELL_LIST_FLAG_NON_BLOCKING) != 0 {
                    non_blocking.push((spell.clone(), *target));
                }
            })
            .filter(|(spell, _)| (spell.flags & CREATURE_SPELL_LIST_FLAG_NON_BLOCKING) == 0)
            .collect::<Vec<_>>();
        if eligible.is_empty() {
            eligible = non_blocking;
        }
        eligible.sort_by_key(|(spell, _)| spell.position);
        let (spell, target) = choose_db_creature_spell(&eligible)?;
        Some(ReadyDbCreatureSpellCast { spell, target })
    }

    pub(in crate::world) fn apply_db_creature_spell_cooldowns(
        &mut self,
        attacker: ObjectGuid,
        spell: &wow_db::CreatureSpellListQuery,
        template: &wow_db::SpellTemplateQuery,
        now: Instant,
    ) {
        let Some(creature) = self.creatures.get_mut(&attacker.raw()) else {
            return;
        };
        let repeat_cooldown_millis = random_millis_between(spell.repeat_min, spell.repeat_max);
        if repeat_cooldown_millis > 0 {
            let key = db_creature_spell_list_repeat_cooldown_key(spell, template.category);
            db_creature_insert_spell_cooldown(
                creature,
                key,
                now + Duration::from_millis(repeat_cooldown_millis as u64),
            );
        }
        if template.recovery_time > 0 {
            db_creature_insert_spell_cooldown(
                creature,
                template.id,
                now + Duration::from_millis(template.recovery_time as u64),
            );
        }
        if template.category != 0 && template.category_recovery_time > 0 {
            db_creature_insert_spell_cooldown(
                creature,
                db_creature_spell_category_cooldown_key(template.category),
                now + Duration::from_millis(template.category_recovery_time as u64),
            );
        }
    }

    pub(in crate::world) fn start_db_creature_spell_cast(
        &mut self,
        cast: ActiveDbCreatureSpellCast,
    ) -> anyhow::Result<Option<Vec<(SessionId, OutboundWorldPacket)>>> {
        let Some(creature) = self.creatures.get(&cast.caster.raw()) else {
            return Ok(None);
        };
        if !creature.is_alive() || creature.is_evading_home() {
            return Ok(None);
        }
        if creature.power1 < cast.mana_cost {
            return Ok(None);
        }
        if !self.db_creature_spell_cast_target_alive(cast.target) {
            return Ok(None);
        }
        let targets = SpellCastTargets {
            target_mask: SPELL_CAST_TARGET_UNIT,
            unit_target: Some(cast.target),
            gameobject_target: None,
            source_location: None,
            destination: None,
        };
        let start_body =
            build_spell_start_body(cast.caster, cast.spell_id, cast.cast_time_millis, &targets)?;
        let start_packet = OutboundWorldPacket {
            opcode: SMSG_SPELL_START,
            body: start_body,
        };
        let (position, motion_stop_packet, power_update_packet) = {
            let creature = self
                .creatures
                .get_mut(&cast.caster.raw())
                .expect("checked above");
            let position = creature.current_position;
            let motion_stop_packet = if matches!(creature.motion, CreatureMotionState::Idle) {
                None
            } else {
                let stop = stop_db_creature_motion_runtime(creature);
                Some(OutboundWorldPacket {
                    opcode: SMSG_MONSTER_MOVE,
                    body: build_monster_move_stop_body(cast.caster, stop.position, stop.spline_id)?,
                })
            };
            creature.power1 = creature.power1.saturating_sub(cast.mana_cost);
            let power_update_packet = (cast.mana_cost > 0)
                .then(|| {
                    Ok::<_, anyhow::Error>(OutboundWorldPacket {
                        opcode: SMSG_UPDATE_OBJECT,
                        body: build_db_creature_power_update_body(cast.caster, creature.power1)?,
                    })
                })
                .transpose()?;
            (position, motion_stop_packet, power_update_packet)
        };
        self.active_creature_spell_casts
            .insert(cast.caster.raw(), cast);
        Ok(Some(
            self.nearby_player_guids(position, CREATURE_SPAWN_RADIUS_YARDS, None)
                .into_iter()
                .filter_map(|player_guid| {
                    let player = self.players.get(&player_guid)?;
                    let mut packets = Vec::new();
                    if let Some(packet) = &motion_stop_packet {
                        if let Some(packet) = player.packet_to_client(packet.clone()) {
                            packets.push(packet);
                        }
                    }
                    if let Some(packet) = &power_update_packet {
                        if let Some(packet) = player.packet_to_client(packet.clone()) {
                            packets.push(packet);
                        }
                    }
                    if let Some(packet) = player.packet_to_client(start_packet.clone()) {
                        packets.push(packet);
                    }
                    Some(packets)
                })
                .flatten()
                .collect(),
        ))
    }

    #[allow(dead_code)]
    pub(in crate::world) fn complete_ready_db_creature_spell_cast(
        &mut self,
        attacker: ObjectGuid,
        victim: ObjectGuid,
        now: Instant,
    ) -> anyhow::Result<Option<DbCreatureCompletedSpellCastEvent>> {
        self.complete_ready_db_creature_spell_cast_with_navigation(
            attacker,
            victim,
            now,
            &DbCreatureNavigationGuardrail::default(),
        )
    }

    pub(in crate::world) fn complete_ready_db_creature_spell_cast_with_navigation(
        &mut self,
        attacker: ObjectGuid,
        victim: ObjectGuid,
        now: Instant,
        navigation: &DbCreatureNavigationGuardrail,
    ) -> anyhow::Result<Option<DbCreatureCompletedSpellCastEvent>> {
        let Some(cast) = self
            .active_creature_spell_casts
            .get(&attacker.raw())
            .cloned()
        else {
            return Ok(None);
        };
        if now < cast.due_at {
            return Ok(None);
        }
        match self.active_creature_combats.get(&attacker.raw()) {
            Some(combat) if combat.victim != victim => return Ok(None),
            Some(_) => {}
            None if victim == cast.target || (victim.is_player() && cast.target == attacker) => {}
            None => return Ok(None),
        }
        let target_validation = self.validate_db_creature_spell_against_target(
            attacker,
            cast.target,
            navigation,
            cast.range,
            cast.requires_behind,
        );
        if target_validation.check != DbCreatureSpellTargetCheck::Clear {
            self.active_creature_spell_casts.remove(&attacker.raw());
            let failure = db_creature_spell_failure_from_target_check(target_validation.check);
            return Ok(Some(DbCreatureCompletedSpellCastEvent {
                spell_go_body: Vec::new(),
                effect: DbCreatureCompletedSpellEffect::Interrupted(
                    self.db_creature_interrupted_spell_cast_event(
                        attacker,
                        cast.spell_id,
                        failure,
                    )?,
                ),
                aura_event: None,
                creature_aura_event: None,
            }));
        }
        self.active_creature_spell_casts.remove(&attacker.raw());
        let targets = SpellCastTargets {
            target_mask: SPELL_CAST_TARGET_UNIT,
            unit_target: Some(cast.target),
            gameobject_target: None,
            source_location: None,
            destination: None,
        };
        let pending_aura = cast.aura.clone();
        let (effect, spell_go_body) = match cast.effect {
            ActiveDbCreatureSpellEffect::None => (
                DbCreatureCompletedSpellEffect::AuraOnly,
                build_spell_go_body(attacker, cast.spell_id, &targets)?,
            ),
            ActiveDbCreatureSpellEffect::Damage {
                amount,
                school,
                dmg_class,
                attributes_ex2,
                attributes_ex3,
            } => {
                if !cast.target.is_player() {
                    return Ok(None);
                }
                let Some(damage) = self.apply_db_creature_player_spell_damage(
                    attacker,
                    cast.target,
                    cast.spell_id,
                    amount,
                    school,
                    dmg_class,
                    attributes_ex2,
                    attributes_ex3,
                    now,
                )?
                else {
                    return Ok(None);
                };
                let spell_go_body = if let Some(miss_info) = damage.outcome.miss_info {
                    build_spell_go_body_with_miss(attacker, cast.spell_id, &targets, miss_info)?
                } else {
                    build_spell_go_body(attacker, cast.spell_id, &targets)?
                };
                (
                    DbCreatureCompletedSpellEffect::PlayerDamage(damage),
                    spell_go_body,
                )
            }
            ActiveDbCreatureSpellEffect::Heal { amount } => {
                let Some(heal) = self.apply_db_creature_creature_spell_heal(
                    attacker,
                    cast.target,
                    cast.spell_id,
                    amount,
                )?
                else {
                    return Ok(None);
                };
                (
                    DbCreatureCompletedSpellEffect::CreatureHeal(heal),
                    build_spell_go_body(attacker, cast.spell_id, &targets)?,
                )
            }
        };
        let mut creature_aura_event = None;
        let aura_event = match (pending_aura, &effect) {
            (Some(aura), DbCreatureCompletedSpellEffect::PlayerDamage(damage))
                if cast.target.is_player()
                    && damage.victim_health > 0
                    && damage.outcome.miss_info.is_none() =>
            {
                self.apply_player_aura(cast.target.counter(), aura)?
            }
            (Some(aura), DbCreatureCompletedSpellEffect::AuraOnly) if cast.target.is_player() => {
                self.apply_player_aura(cast.target.counter(), aura)?
            }
            (Some(aura), DbCreatureCompletedSpellEffect::AuraOnly) if cast.target.is_creature() => {
                creature_aura_event = self.apply_db_creature_aura(cast.target, 0, aura, now)?;
                None
            }
            _ => None,
        };
        Ok(Some(DbCreatureCompletedSpellCastEvent {
            spell_go_body,
            effect,
            aura_event,
            creature_aura_event,
        }))
    }

    pub(in crate::world) fn db_creature_interrupted_spell_cast_event(
        &self,
        caster: ObjectGuid,
        spell_id: u32,
        failure: u8,
    ) -> anyhow::Result<DbCreatureInterruptedSpellCastEvent> {
        let Some(position) = self
            .creatures
            .get(&caster.raw())
            .map(|creature| creature.current_position)
        else {
            return Ok(DbCreatureInterruptedSpellCastEvent {
                failure,
                observer_packets: Vec::new(),
            });
        };
        let failure_packet = OutboundWorldPacket {
            opcode: SMSG_SPELL_FAILURE,
            body: build_spell_failure_body(caster, spell_id, failure)?,
        };
        let failed_other_packet = OutboundWorldPacket {
            opcode: SMSG_SPELL_FAILED_OTHER,
            body: build_spell_failed_other_body(caster, spell_id),
        };
        let observer_packets = self
            .nearby_player_guids(position, CREATURE_SPAWN_RADIUS_YARDS, None)
            .into_iter()
            .filter_map(|player_guid| {
                let player = self.players.get(&player_guid)?;
                Some([
                    player.packet_to_client(failure_packet.clone()),
                    player.packet_to_client(failed_other_packet.clone()),
                ])
            })
            .flatten()
            .flatten()
            .collect();
        Ok(DbCreatureInterruptedSpellCastEvent {
            failure,
            observer_packets,
        })
    }

    pub(in crate::world) fn active_db_creature_spell_cast_due_at(
        &self,
        attacker: ObjectGuid,
    ) -> Option<Instant> {
        self.active_creature_spell_casts
            .get(&attacker.raw())
            .map(|cast| cast.due_at)
    }

    pub(in crate::world) fn begin_db_creature_combat(
        &mut self,
        attacker: ObjectGuid,
        victim: ObjectGuid,
        now: Instant,
    ) -> Option<CreatureCombatState> {
        if self
            .active_creature_combats
            .get(&attacker.raw())
            .is_some_and(|combat| combat.victim == victim)
        {
            return None;
        }
        if self.active_creature_combats.contains_key(&attacker.raw()) {
            return None;
        }
        let combat = CreatureCombatState {
            attacker,
            victim,
            next_swing_at: now,
        };
        if let Some(creature) = self.creatures.get_mut(&attacker.raw()) {
            creature.aggro_enabled_at = None;
        }
        self.active_creature_combats.insert(attacker.raw(), combat);
        if victim.is_player() {
            if let Some(player) = self.players.get_mut(&victim.counter()) {
                player.in_combat = true;
            }
        }
        self.refresh_db_creature_combat_leash(attacker, now);
        self.add_db_creature_threat(attacker, victim, 0.0);
        if let Some(position) = self
            .creatures
            .get(&attacker.raw())
            .map(|creature| creature.current_position)
        {
            self.refresh_grid_state(grid_coord_for_position(position));
        }
        Some(combat)
    }

    pub(in crate::world) fn clear_db_creature_combat(&mut self, attacker: ObjectGuid) {
        let old_victim = self
            .active_creature_combats
            .remove(&attacker.raw())
            .map(|combat| combat.victim);
        self.active_creature_spell_casts.remove(&attacker.raw());
        self.creature_combat_leash.remove(&attacker.raw());
        self.creature_threats.remove(&attacker.raw());
        if let Some(victim) = old_victim {
            self.set_player_in_combat_from_creature_refs(victim);
        }
        if let Some(position) = self
            .creatures
            .get(&attacker.raw())
            .map(|creature| creature.current_position)
        {
            self.refresh_grid_state(grid_coord_for_position(position));
        }
    }

    pub(in crate::world) fn clear_db_creature_combats_for_victim(&mut self, victim: ObjectGuid) {
        let changed_grids = self
            .active_creature_combats
            .values()
            .filter(|combat| combat.victim == victim)
            .filter_map(|combat| self.creatures.get(&combat.attacker.raw()))
            .map(|creature| grid_coord_for_position(creature.current_position))
            .collect::<HashSet<_>>();
        self.active_creature_combats
            .retain(|_, combat| combat.victim != victim);
        let active_attackers = self
            .active_creature_combats
            .keys()
            .copied()
            .collect::<HashSet<_>>();
        self.active_creature_spell_casts
            .retain(|attacker, cast| active_attackers.contains(attacker) && cast.target != victim);
        self.creature_combat_leash
            .retain(|attacker, _| active_attackers.contains(attacker));
        for threats in self.creature_threats.values_mut() {
            threats.retain(|entry| entry.victim != victim);
        }
        self.creature_threats
            .retain(|attacker, threats| active_attackers.contains(attacker) || !threats.is_empty());
        self.set_player_in_combat_from_creature_refs(victim);
        for grid in changed_grids {
            self.refresh_grid_state(grid);
        }
    }

    pub(in crate::world) fn active_db_creature_combats_for_victim(
        &self,
        victim: ObjectGuid,
    ) -> Vec<CreatureCombatState> {
        let mut combats = self
            .active_creature_combats
            .values()
            .filter(|combat| combat.victim == victim)
            .copied()
            .collect::<Vec<_>>();
        combats.sort_by_key(|combat| combat.attacker.raw());
        combats
    }

    #[allow(dead_code)]
    pub(in crate::world) fn apply_db_creature_player_damage(
        &mut self,
        attacker: ObjectGuid,
        victim: ObjectGuid,
        damage: u32,
        now: Instant,
        next_swing_at: Instant,
    ) -> anyhow::Result<Option<DbCreaturePlayerDamageEvent>> {
        self.apply_db_creature_player_melee_outcome(
            attacker,
            victim,
            MeleeDamageOutcome::normal_hit(damage),
            now,
            next_swing_at,
        )
    }

    pub(in crate::world) fn apply_db_creature_player_melee_outcome(
        &mut self,
        attacker: ObjectGuid,
        victim: ObjectGuid,
        outcome: MeleeDamageOutcome,
        now: Instant,
        next_swing_at: Instant,
    ) -> anyhow::Result<Option<DbCreaturePlayerDamageEvent>> {
        let combat = {
            let Some(combat) = self.active_creature_combats.get_mut(&attacker.raw()) else {
                return Ok(None);
            };
            if combat.victim != victim {
                return Ok(None);
            }
            combat.next_swing_at = next_swing_at;
            *combat
        };
        let creature_motion = self
            .creatures
            .get(&attacker.raw())
            .map(|creature| creature.motion.clone());
        if !self.players.get(&victim.counter()).is_some_and(|player| {
            player.health > 0 && player.death_state == PlayerDeathState::Alive
        }) {
            return Ok(None);
        };
        let damage = outcome.total_damage;
        if damage == 0 {
            let Some(victim_player) = self.players.get(&victim.counter()) else {
                return Ok(None);
            };
            let victim_health = victim_player.health;
            let victim_position = victim_player.position;
            let attacker_state = OutboundWorldPacket {
                opcode: SMSG_ATTACKERSTATEUPDATE,
                body: build_attacker_state_update_body_for_outcome(attacker, victim, outcome, 0)?,
            };
            let health_update_body = build_player_health_update_body(victim, victim_health)?;
            let health_update = OutboundWorldPacket {
                opcode: SMSG_UPDATE_OBJECT,
                body: health_update_body.clone(),
            };
            let mut observer_packets = Vec::new();
            for player_guid in self.nearby_player_guids(
                victim_position,
                PLAYER_VISIBILITY_RADIUS_YARDS,
                Some(victim.counter()),
            ) {
                let Some(player) = self.players.get(&player_guid) else {
                    continue;
                };
                if let Some(packet) = player.packet_to_client(attacker_state.clone()) {
                    observer_packets.push(packet);
                }
                if let Some(packet) = player.packet_to_client(health_update.clone()) {
                    observer_packets.push(packet);
                }
            }
            return Ok(Some(DbCreaturePlayerDamageEvent {
                damage,
                victim_health,
                combat,
                direct_packets: Vec::new(),
                aura_packet: None,
                health_update_body,
                observer_packets,
            }));
        }
        let Some(applied) = self.apply_player_world_damage(
            victim,
            Some(attacker),
            damage,
            WorldDamageKind::Melee,
            now,
        )?
        else {
            return Ok(None);
        };
        let victim_health = applied.remaining_health;
        let victim_position = applied.position;
        let direct_packets = applied.direct_packets;
        let aura_packet = applied.aura_packet;
        let health_update_body = applied.health_packet.body.clone();
        if damage > 0
            && creature_motion
                .as_ref()
                .is_some_and(|motion| !matches!(motion, CreatureMotionState::Chase(_)))
        {
            self.refresh_db_creature_combat_leash(attacker, now);
        }
        let attacker_state = OutboundWorldPacket {
            opcode: SMSG_ATTACKERSTATEUPDATE,
            body: build_attacker_state_update_body_for_outcome(attacker, victim, outcome, 0)?,
        };
        let health_update = OutboundWorldPacket {
            opcode: SMSG_UPDATE_OBJECT,
            body: health_update_body.clone(),
        };
        let mut observer_packets = applied.observer_packets;
        for player_guid in self.nearby_player_guids(
            victim_position,
            PLAYER_VISIBILITY_RADIUS_YARDS,
            Some(victim.counter()),
        ) {
            let Some(player) = self.players.get(&player_guid) else {
                continue;
            };
            if let Some(packet) = player.packet_to_client(attacker_state.clone()) {
                observer_packets.push(packet);
            }
            if let Some(packet) = player.packet_to_client(health_update.clone()) {
                observer_packets.push(packet);
            }
        }
        Ok(Some(DbCreaturePlayerDamageEvent {
            damage,
            victim_health,
            combat,
            direct_packets,
            aura_packet,
            health_update_body,
            observer_packets,
        }))
    }

    #[allow(clippy::too_many_arguments, dead_code)]
    pub(in crate::world) fn apply_db_creature_player_spell_damage(
        &mut self,
        attacker: ObjectGuid,
        victim: ObjectGuid,
        spell_id: u32,
        damage: u32,
        school: u8,
        dmg_class: u32,
        attributes_ex2: u32,
        attributes_ex3: u32,
        now: Instant,
    ) -> anyhow::Result<Option<DbCreaturePlayerSpellDamageEvent>> {
        let Some(combat) = self.active_creature_combats.get(&attacker.raw()).copied() else {
            return Ok(None);
        };
        if combat.victim != victim {
            return Ok(None);
        }
        let Some(creature) = self.creatures.get(&attacker.raw()) else {
            return Ok(None);
        };
        if !creature.is_alive() {
            return Ok(None);
        }
        let Some(victim_player) = self.players.get(&victim.counter()) else {
            return Ok(None);
        };
        if victim_player.health == 0 || victim_player.death_state != PlayerDeathState::Alive {
            return Ok(None);
        }
        let outcome = roll_spell_damage_outcome(spell_damage_outcome_input(
            damage,
            school,
            dmg_class,
            attributes_ex2,
            attributes_ex3,
            db_creature_spell_snapshot(creature),
            player_spell_snapshot(
                victim_player.level,
                victim_player.class,
                &victim_player.combat_stats,
            ),
        ));
        let requested_damage = outcome.final_damage;
        let damage = victim_player.health.min(requested_damage);
        let spell_non_melee_log_body = outcome
            .miss_info
            .is_none()
            .then(|| {
                build_spell_non_melee_damage_log_body(SpellNonMeleeDamageLogPacket {
                    attacker,
                    target: victim,
                    spell_id,
                    damage: requested_damage,
                    school,
                    absorb: outcome.absorb,
                    resist: outcome.resist,
                    periodic: false,
                    blocked: outcome.blocked,
                    hit_info: outcome.hit_info,
                })
            })
            .transpose()?;
        let spell_miss_log_body = outcome
            .miss_info
            .map(|miss_info| build_spell_log_miss_body(attacker, victim, spell_id, miss_info))
            .transpose()?;
        let victim_health_before = victim_player.health;
        let victim_position_before = victim_player.position;
        let (
            victim_health,
            victim_position,
            direct_packets,
            aura_packet,
            health_update_body,
            mut observer_packets,
        ) = if requested_damage == 0 {
            (
                victim_health_before,
                victim_position_before,
                Vec::new(),
                None,
                build_player_health_update_body(victim, victim_health_before)?,
                Vec::new(),
            )
        } else {
            let Some(applied) = self.apply_player_world_damage_with_school_mask(
                victim,
                Some(attacker),
                damage,
                WorldDamageKind::SpellDirect,
                u32::from(school).max(SPELL_SCHOOL_MASK_NORMAL),
                now,
            )?
            else {
                return Ok(None);
            };
            (
                applied.remaining_health,
                applied.position,
                applied.direct_packets,
                applied.aura_packet,
                applied.health_packet.body.clone(),
                applied.observer_packets,
            )
        };
        if damage > 0 {
            self.add_db_creature_threat(attacker, victim, damage as f32);
            self.refresh_db_creature_combat_leash(attacker, now);
        }
        for player_guid in self.nearby_player_guids(
            victim_position,
            PLAYER_VISIBILITY_RADIUS_YARDS,
            Some(victim.counter()),
        ) {
            let Some(player) = self.players.get(&player_guid) else {
                continue;
            };
            if let Some(body) = &spell_miss_log_body {
                if let Some(packet) = player.packet_to_client(OutboundWorldPacket {
                    opcode: SMSG_SPELLLOGMISS,
                    body: body.clone(),
                }) {
                    observer_packets.push(packet);
                }
            }
            if let Some(body) = &spell_non_melee_log_body {
                if let Some(packet) = player.packet_to_client(OutboundWorldPacket {
                    opcode: SMSG_SPELLNONMELEEDAMAGELOG,
                    body: body.clone(),
                }) {
                    observer_packets.push(packet);
                }
            }
            if let Some(packet) = player.packet_to_client(OutboundWorldPacket {
                opcode: SMSG_UPDATE_OBJECT,
                body: health_update_body.clone(),
            }) {
                observer_packets.push(packet);
            }
        }
        Ok(Some(DbCreaturePlayerSpellDamageEvent {
            damage,
            victim_health,
            outcome,
            spell_non_melee_log_body,
            spell_miss_log_body,
            direct_packets,
            aura_packet,
            health_update_body,
            observer_packets,
        }))
    }

    pub(in crate::world) fn apply_db_creature_creature_spell_heal(
        &mut self,
        caster: ObjectGuid,
        target: ObjectGuid,
        spell_id: u32,
        heal: u32,
    ) -> anyhow::Result<Option<DbCreatureSpellHealEvent>> {
        if heal == 0 {
            return Ok(None);
        }
        let Some(caster_creature) = self.creatures.get(&caster.raw()) else {
            return Ok(None);
        };
        if !caster_creature.is_alive() || caster_creature.is_evading_home() {
            return Ok(None);
        }
        let Some(target_creature) = self.creatures.get_mut(&target.raw()) else {
            return Ok(None);
        };
        if !target_creature.is_alive() || target_creature.is_evading_home() {
            return Ok(None);
        }
        let previous_health = target_creature.health;
        target_creature.health = target_creature
            .health
            .saturating_add(heal)
            .min(target_creature.max_health());
        let amount = target_creature.health.saturating_sub(previous_health);
        if amount == 0 {
            return Ok(None);
        }
        let target_health = target_creature.health;
        let dynamic_flags = target_creature.spawn.template.dynamic_flags;
        let target_position = target_creature.current_position;
        let spell_heal_log_body =
            build_spell_heal_log_body(caster, target, spell_id, amount, false)?;
        let health_update_body =
            build_db_creature_state_update_body(target, target_health, dynamic_flags)?;
        let mut observer_packets = Vec::new();
        for player_guid in
            self.nearby_player_guids(target_position, CREATURE_SPAWN_RADIUS_YARDS, None)
        {
            let Some(player) = self.players.get(&player_guid) else {
                continue;
            };
            if let Some(packet) = player.packet_to_client(OutboundWorldPacket {
                opcode: SMSG_SPELLHEALLOG,
                body: spell_heal_log_body.clone(),
            }) {
                observer_packets.push(packet);
            }
            if let Some(packet) = player.packet_to_client(OutboundWorldPacket {
                opcode: SMSG_UPDATE_OBJECT,
                body: health_update_body.clone(),
            }) {
                observer_packets.push(packet);
            }
        }
        Ok(Some(DbCreatureSpellHealEvent {
            target,
            amount,
            target_health,
            spell_heal_log_body,
            health_update_body,
            observer_packets,
        }))
    }

    pub(in crate::world) fn db_creature_spell_cast_target_alive(&self, target: ObjectGuid) -> bool {
        if target.is_player() {
            return self
                .players
                .get(&target.counter())
                .is_some_and(|player| player.health > 0);
        }
        self.creatures
            .get(&target.raw())
            .is_some_and(|creature| creature.is_alive() && !creature.is_evading_home())
    }

    pub(in crate::world) fn db_creature_spell_target_position(
        &self,
        target: ObjectGuid,
    ) -> Option<WorldPosition> {
        if target.is_player() {
            return self
                .players
                .get(&target.counter())
                .map(|player| player.position);
        }
        self.creatures
            .get(&target.raw())
            .map(|creature| creature.current_position)
    }

    pub(in crate::world) fn db_creature_spell_target_combat_reach(
        &self,
        target: ObjectGuid,
    ) -> f32 {
        if target.is_player() {
            return PLAYER_COMBAT_REACH_YARDS;
        }
        self.creatures
            .get(&target.raw())
            .map(DbCreatureRuntime::combat_reach)
            .unwrap_or(0.0)
    }

    pub(in crate::world) fn defer_ready_db_creature_swing_retry(
        &mut self,
        attacker: ObjectGuid,
        victim: ObjectGuid,
        now: Instant,
    ) -> Option<CreatureCombatState> {
        let combat = self.active_creature_combats.get_mut(&attacker.raw())?;
        if combat.attacker == attacker && combat.victim == victim && now >= combat.next_swing_at {
            combat.next_swing_at = now + Duration::from_millis(DB_CREATURE_MELEE_RETRY_MILLIS);
        }
        Some(*combat)
    }

    pub(in crate::world) fn refresh_db_creature_combat_leash(
        &mut self,
        attacker: ObjectGuid,
        now: Instant,
    ) {
        let Some(creature) = self.creatures.get(&attacker.raw()) else {
            return;
        };
        let combat_start_position = self
            .creature_combat_leash
            .get(&attacker.raw())
            .map(|leash| leash.combat_start_position)
            .unwrap_or(creature.current_position);
        self.creature_combat_leash.insert(
            attacker.raw(),
            CreatureCombatLeashState {
                refresh_position: creature.current_position,
                combat_start_position,
                expires_at: now + db_creature_pursuit_duration_millis(creature),
                template_leash_yards: creature.spawn.template.leash as f32,
            },
        );
    }

    pub(in crate::world) fn db_creature_should_evade(
        &self,
        attacker: ObjectGuid,
        now: Instant,
    ) -> bool {
        let Some(creature) = self.creatures.get(&attacker.raw()) else {
            return false;
        };
        if matches!(creature.motion, CreatureMotionState::ReturnHome(_)) {
            return false;
        }
        let Some(combat) = self.active_creature_combats.get(&attacker.raw()) else {
            return false;
        };
        let Some(leash) = self.creature_combat_leash.get(&attacker.raw()) else {
            return false;
        };
        if leash.template_leash_yards > 0.0
            && distance_2d(
                creature.current_position.x,
                creature.current_position.y,
                leash.combat_start_position.x,
                leash.combat_start_position.y,
            ) > leash.template_leash_yards
        {
            return true;
        }
        if now < leash.expires_at {
            return false;
        }
        let Some(victim) = self.players.get(&combat.victim.counter()) else {
            return false;
        };
        distance_2d(
            victim.position.x,
            victim.position.y,
            leash.refresh_position.x,
            leash.refresh_position.y,
        ) > DB_CREATURE_LEASH_RADIUS_YARDS
    }

    pub(in crate::world) fn add_db_creature_threat(
        &mut self,
        attacker: ObjectGuid,
        victim: ObjectGuid,
        threat: f32,
    ) {
        if threat < 0.0 || !threat.is_finite() {
            return;
        }
        let threats = self.creature_threats.entry(attacker.raw()).or_default();
        if let Some(entry) = threats.iter_mut().find(|entry| entry.victim == victim) {
            entry.threat += threat;
        } else {
            threats.push(CreatureThreatEntry { victim, threat });
        }
        threats.sort_by(|left, right| {
            right
                .threat
                .total_cmp(&left.threat)
                .then_with(|| left.victim.raw().cmp(&right.victim.raw()))
        });
    }

    #[cfg(test)]
    pub(in crate::world) fn db_creature_threat_entries(
        &self,
        attacker: ObjectGuid,
    ) -> Vec<CreatureThreatEntry> {
        self.creature_threats
            .get(&attacker.raw())
            .cloned()
            .unwrap_or_default()
    }

    pub(in crate::world) fn select_db_creature_threat_victim(
        &self,
        attacker: ObjectGuid,
        current_victim: Option<ObjectGuid>,
    ) -> Option<ObjectGuid> {
        let threats = self.creature_threats.get(&attacker.raw())?;
        let current_entry =
            current_victim.and_then(|victim| threats.iter().find(|entry| entry.victim == victim));

        for entry in threats {
            if entry.threat <= 0.0 {
                continue;
            }
            let Some(current) = current_entry else {
                return Some(entry.victim);
            };
            if entry.victim == current.victim {
                return Some(current.victim);
            }
            if entry.threat <= DB_CREATURE_THREAT_MELEE_SWITCH_FACTOR * current.threat {
                return Some(current.victim);
            }
            if entry.threat > DB_CREATURE_THREAT_RANGED_SWITCH_FACTOR * current.threat
                || (entry.threat > DB_CREATURE_THREAT_MELEE_SWITCH_FACTOR * current.threat
                    && self.db_creature_threat_victim_in_melee(attacker, entry.victim))
            {
                return Some(entry.victim);
            }
        }
        current_victim
    }

    pub(in crate::world) fn switch_db_creature_threat_victim_if_needed(
        &mut self,
        attacker: ObjectGuid,
        exclude_character_guid: Option<u32>,
    ) -> anyhow::Result<Option<DbCreatureThreatTargetSwitchEvent>> {
        let Some(current_combat) = self.active_creature_combats.get(&attacker.raw()).copied()
        else {
            return Ok(None);
        };
        let Some(new_victim) =
            self.select_db_creature_threat_victim(attacker, Some(current_combat.victim))
        else {
            return Ok(None);
        };
        if new_victim == current_combat.victim {
            return Ok(None);
        }
        let Some(creature_position) = self
            .creatures
            .get(&attacker.raw())
            .map(|creature| creature.current_position)
        else {
            return Ok(None);
        };
        let mut combat = current_combat;
        combat.victim = new_victim;
        self.active_creature_combats.insert(attacker.raw(), combat);
        self.set_player_in_combat_from_creature_refs(current_combat.victim);
        if new_victim.is_player() {
            if let Some(player) = self.players.get_mut(&new_victim.counter()) {
                player.in_combat = true;
            }
        }

        let packets = [
            OutboundWorldPacket {
                opcode: SMSG_ATTACKSTOP,
                body: build_attack_stop_body(attacker, current_combat.victim, false)?,
            },
            OutboundWorldPacket {
                opcode: SMSG_ATTACKSTART,
                body: build_attack_start_body(attacker, new_victim),
            },
        ];
        let direct_packets = if exclude_character_guid
            .is_some_and(|guid| packets_direct_to_character(self, guid, creature_position))
        {
            packets.to_vec()
        } else {
            Vec::new()
        };
        let observer_packets = self
            .nearby_player_guids(
                creature_position,
                CREATURE_SPAWN_RADIUS_YARDS,
                exclude_character_guid,
            )
            .into_iter()
            .filter_map(|player_guid| {
                self.players.get(&player_guid).map(|player| {
                    packets
                        .iter()
                        .cloned()
                        .filter_map(|packet| player.packet_to_client(packet))
                        .collect::<Vec<_>>()
                })
            })
            .flatten()
            .collect();

        Ok(Some(DbCreatureThreatTargetSwitchEvent {
            attacker,
            old_victim: current_combat.victim,
            new_victim,
            combat,
            direct_packets,
            observer_packets,
        }))
    }

    pub(in crate::world) fn db_creature_threat_victim_in_melee(
        &self,
        attacker: ObjectGuid,
        victim: ObjectGuid,
    ) -> bool {
        let Some(creature) = self.creatures.get(&attacker.raw()) else {
            return false;
        };
        let Some(player) = self.players.get(&victim.counter()) else {
            return false;
        };
        let reach = combined_melee_reach(creature.combat_reach(), PLAYER_COMBAT_REACH_YARDS);
        let dx = creature.current_position.x - player.position.x;
        let dy = creature.current_position.y - player.position.y;
        dx * dx + dy * dy <= reach * reach
    }
}

pub(in crate::world) fn db_creature_pursuit_duration_millis(
    creature: &DbCreatureRuntime,
) -> Duration {
    let pursuit_millis = if creature.spawn.template.pursuit == 0 {
        DB_CREATURE_DEFAULT_PURSUIT_MILLIS
    } else {
        creature.spawn.template.pursuit
    };
    Duration::from_millis(pursuit_millis as u64)
}

pub(in crate::world) fn initialize_db_creature_spell_cooldowns(
    creature: &mut DbCreatureRuntime,
    spell_list: &[wow_db::CreatureSpellListQuery],
    now: Instant,
) {
    for spell in spell_list {
        let cooldown_millis = random_millis_between(spell.initial_min, spell.initial_max);
        if cooldown_millis == 0 {
            continue;
        }
        creature.spell_cooldowns_until.insert(
            spell.spell_id,
            now + Duration::from_millis(cooldown_millis as u64),
        );
    }
}

pub(in crate::world) fn db_creature_spell_cooldown_ready(
    cooldowns_until: &std::collections::HashMap<u32, Instant>,
    spell: &wow_db::CreatureSpellListQuery,
    now: Instant,
) -> bool {
    cooldowns_until
        .get(&spell.spell_id)
        .is_none_or(|cooldown| now >= *cooldown)
        && (spell.category == 0
            || cooldowns_until
                .get(&db_creature_spell_category_cooldown_key(spell.category))
                .is_none_or(|cooldown| now >= *cooldown))
}

pub(in crate::world) fn db_creature_spell_list_repeat_cooldown_key(
    spell: &wow_db::CreatureSpellListQuery,
    template_category: u32,
) -> u32 {
    if (spell.flags & CREATURE_SPELL_LIST_FLAG_CATEGORY_COOLDOWN) != 0 {
        let category = if spell.category != 0 {
            spell.category
        } else {
            template_category
        };
        if category != 0 {
            return db_creature_spell_category_cooldown_key(category);
        }
    }
    spell.spell_id
}

pub(in crate::world) fn db_creature_spell_category_cooldown_key(category: u32) -> u32 {
    CREATURE_SPELL_CATEGORY_COOLDOWN_KEY | (category & !CREATURE_SPELL_CATEGORY_COOLDOWN_KEY)
}

pub(in crate::world) fn db_creature_spell_failure_from_target_check(
    check: DbCreatureSpellTargetCheck,
) -> u8 {
    match check {
        DbCreatureSpellTargetCheck::Clear => SPELL_FAILED_INTERRUPTED,
        DbCreatureSpellTargetCheck::CasterNotAlive => SPELL_FAILED_CASTER_DEAD,
        DbCreatureSpellTargetCheck::NavigationBlocked(
            DbCreatureNavigationResult::LineOfSightBlocked,
        ) => SPELL_FAILED_LINE_OF_SIGHT,
        DbCreatureSpellTargetCheck::TooClose => SPELL_FAILED_TOO_CLOSE,
        DbCreatureSpellTargetCheck::NotBehind => SPELL_FAILED_NOT_BEHIND,
        DbCreatureSpellTargetCheck::MissingCaster
        | DbCreatureSpellTargetCheck::MissingTarget
        | DbCreatureSpellTargetCheck::TargetNotAlive
        | DbCreatureSpellTargetCheck::NavigationBlocked(_)
        | DbCreatureSpellTargetCheck::OutOfRange => SPELL_FAILED_OUT_OF_RANGE,
    }
}

pub(in crate::world) fn db_creature_insert_spell_cooldown(
    creature: &mut DbCreatureRuntime,
    key: u32,
    cooldown_until: Instant,
) {
    creature
        .spell_cooldowns_until
        .entry(key)
        .and_modify(|existing| {
            if cooldown_until > *existing {
                *existing = cooldown_until;
            }
        })
        .or_insert(cooldown_until);
}

#[derive(Debug, Clone, Copy)]
#[allow(dead_code)]
pub(in crate::world) struct DbCreatureEventAiAction {
    pub(in crate::world) action_type: u8,
    pub(in crate::world) param1: i32,
    pub(in crate::world) param2: i32,
    pub(in crate::world) param3: i32,
}

pub(in crate::world) fn db_creature_event_ai_actions(
    script: &wow_db::CreatureAiScriptQuery,
) -> [DbCreatureEventAiAction; 3] {
    [
        DbCreatureEventAiAction {
            action_type: script.action1_type,
            param1: script.action1_param1,
            param2: script.action1_param2,
            param3: script.action1_param3,
        },
        DbCreatureEventAiAction {
            action_type: script.action2_type,
            param1: script.action2_param1,
            param2: script.action2_param2,
            param3: script.action2_param3,
        },
        DbCreatureEventAiAction {
            action_type: script.action3_type,
            param1: script.action3_param1,
            param2: script.action3_param2,
            param3: script.action3_param3,
        },
    ]
}

pub(in crate::world) fn db_creature_event_ai_hp_script_can_trigger(
    creature: &DbCreatureRuntime,
    script: &wow_db::CreatureAiScriptQuery,
) -> bool {
    if script.event_type != EVENT_AI_EVENT_HP {
        return false;
    }
    if !db_creature_event_ai_actions(script).iter().any(|action| {
        matches!(
            action.action_type,
            EVENT_AI_ACTION_FLEE_FOR_ASSIST | EVENT_AI_ACTION_SET_WALK
        )
    }) {
        return false;
    }
    if script.event_flags & EVENT_AI_FLAG_REPEATABLE == 0
        && creature.triggered_event_ai_scripts.contains(&script.id)
    {
        return false;
    }
    let max_health = creature.max_health().max(1);
    let health_percent = (creature.health.min(max_health) * 100) / max_health;
    let max_percent = script.event_param1.clamp(0, 100) as u32;
    let min_percent = script.event_param2.clamp(0, 100) as u32;
    if health_percent > max_percent || health_percent < min_percent {
        return false;
    }
    if script.event_chance >= 100 {
        return true;
    }
    if script.event_chance == 0 {
        return false;
    }
    rand::thread_rng().gen_range(0..100) < script.event_chance
}

pub(in crate::world) fn db_creature_event_ai_common_ready(
    creature: &DbCreatureRuntime,
    script: &wow_db::CreatureAiScriptQuery,
) -> bool {
    if !creature.is_alive() || creature.is_evading_home() || creature.is_fleeing() {
        return false;
    }
    if script.event_chance == 0 {
        return false;
    }
    db_creature_event_ai_effectively_repeatable(script)
        || !creature.triggered_event_ai_scripts.contains(&script.id)
}

pub(in crate::world) fn db_creature_event_ai_timer_ready(
    creature: &mut DbCreatureRuntime,
    script: &wow_db::CreatureAiScriptQuery,
    now: Instant,
) -> bool {
    match creature.event_ai_cooldowns_until.get(&script.id).copied() {
        Some(due_at) => now >= due_at,
        None => {
            let initial_millis =
                random_millis_between_i32(script.event_param1, script.event_param2);
            if initial_millis == 0 {
                true
            } else {
                creature.event_ai_cooldowns_until.insert(
                    script.id,
                    now + Duration::from_millis(initial_millis as u64),
                );
                false
            }
        }
    }
}

pub(in crate::world) fn db_creature_event_ai_repeating_ready(
    creature: &DbCreatureRuntime,
    script: &wow_db::CreatureAiScriptQuery,
    now: Instant,
) -> bool {
    creature
        .event_ai_cooldowns_until
        .get(&script.id)
        .is_none_or(|due_at| now >= *due_at)
}

pub(in crate::world) fn db_creature_event_ai_range_condition(
    map: &MapRuntime,
    attacker: ObjectGuid,
    victim: ObjectGuid,
    script: &wow_db::CreatureAiScriptQuery,
) -> bool {
    let Some(creature) = map.creatures.get(&attacker.raw()) else {
        return false;
    };
    let Some(target_position) = map.db_creature_spell_target_position(victim) else {
        return false;
    };
    let dx = creature.current_position.x - target_position.x;
    let dy = creature.current_position.y - target_position.y;
    let dz = creature.current_position.z - target_position.z;
    let distance = (dx * dx + dy * dy + dz * dz).sqrt();
    let min = script.event_param1.max(0) as f32;
    let max = script.event_param2.max(0) as f32;
    distance >= min && (max == 0.0 || distance <= max)
}

pub(in crate::world) fn db_creature_event_ai_facing_condition(
    map: &MapRuntime,
    attacker: ObjectGuid,
    victim: ObjectGuid,
    script: &wow_db::CreatureAiScriptQuery,
) -> bool {
    let Some(creature) = map.creatures.get(&attacker.raw()) else {
        return false;
    };
    let Some(player) = victim
        .is_player()
        .then(|| map.players.get(&victim.counter()))
        .flatten()
    else {
        return false;
    };
    let dx = creature.current_position.x - player.position.x;
    let dy = creature.current_position.y - player.position.y;
    let dz = creature.current_position.z - player.position.z;
    if dx * dx + dy * dy + dz * dz > 25.0 {
        return false;
    }
    let creature_in_player_front = has_in_arc(
        player.position,
        creature.current_position,
        PLAYER_MELEE_ARC_RADIANS,
    );
    match script.event_param1 {
        0 => !creature_in_player_front,
        1 => creature_in_player_front,
        _ => false,
    }
}

pub(in crate::world) fn db_creature_is_facing_targets_back(
    caster_position: WorldPosition,
    target_position: WorldPosition,
) -> bool {
    !has_in_arc(target_position, caster_position, PLAYER_MELEE_ARC_RADIANS)
        && has_in_arc(caster_position, target_position, PLAYER_MELEE_ARC_RADIANS)
}

pub(in crate::world) fn db_creature_event_ai_missing_aura_condition(
    creature: &DbCreatureRuntime,
    script: &wow_db::CreatureAiScriptQuery,
) -> bool {
    let spell_id = script.event_param1.max(0) as u32;
    spell_id != 0
        && !creature
            .active_auras
            .iter()
            .any(|aura| aura.spell_id == spell_id)
}

pub(in crate::world) fn db_creature_event_ai_spawned_condition(
    _map: &MapRuntime,
    creature: &DbCreatureRuntime,
    script: &wow_db::CreatureAiScriptQuery,
) -> bool {
    match script.event_param1 {
        EVENT_AI_SPAWNED_ALWAYS => true,
        EVENT_AI_SPAWNED_MAP => creature.current_position.map_id == script.event_param2 as u32,
        EVENT_AI_SPAWNED_ZONE => false,
        _ => false,
    }
}

pub(in crate::world) fn db_creature_event_ai_effectively_repeatable(
    script: &wow_db::CreatureAiScriptQuery,
) -> bool {
    if script.event_flags & EVENT_AI_FLAG_REPEATABLE == 0 {
        return false;
    }
    match script.event_type {
        EVENT_AI_EVENT_FACING_TARGET => script.event_param3 > 0 || script.event_param4 > 0,
        _ => true,
    }
}

pub(in crate::world) fn db_creature_event_ai_action_target(
    map: &MapRuntime,
    attacker: ObjectGuid,
    victim: Option<ObjectGuid>,
    target_mode: i32,
) -> Option<ObjectGuid> {
    match target_mode {
        EVENT_AI_TARGET_SELF => Some(attacker),
        EVENT_AI_TARGET_HOSTILE
        | EVENT_AI_TARGET_ACTION_INVOKER
        | EVENT_AI_TARGET_ACTION_INVOKER_OWNER
        | EVENT_AI_TARGET_PLAYER_INVOKER
        | EVENT_AI_TARGET_PLAYER_TAPPED => {
            victim.filter(|victim| map.db_creature_spell_cast_target_alive(*victim))
        }
        EVENT_AI_TARGET_HOSTILE_SECOND_AGGRO => {
            db_creature_event_ai_threat_target(map, attacker, 1)
        }
        EVENT_AI_TARGET_HOSTILE_LAST_AGGRO => {
            db_creature_event_ai_threat_target(map, attacker, usize::MAX)
        }
        EVENT_AI_TARGET_HOSTILE_RANDOM
        | EVENT_AI_TARGET_HOSTILE_RANDOM_PLAYER
        | EVENT_AI_TARGET_HOSTILE_RANDOM_MANA => {
            db_creature_event_ai_random_threat_target(map, attacker, 0)
        }
        EVENT_AI_TARGET_HOSTILE_RANDOM_NOT_TOP | EVENT_AI_TARGET_HOSTILE_RANDOM_NOT_TOP_PLAYER => {
            db_creature_event_ai_random_threat_target(map, attacker, 1)
        }
        EVENT_AI_TARGET_NEAREST_AOE_TARGET => {
            db_creature_event_ai_nearest_threat_target(map, attacker)
        }
        EVENT_AI_TARGET_HOSTILE_FARTHEST_AWAY => {
            db_creature_event_ai_farthest_threat_target(map, attacker)
        }
        EVENT_AI_TARGET_NONE => victim
            .filter(|victim| map.db_creature_spell_cast_target_alive(*victim))
            .or(Some(attacker)),
        _ => victim.filter(|victim| map.db_creature_spell_cast_target_alive(*victim)),
    }
}

pub(in crate::world) fn db_creature_event_ai_threat_target(
    map: &MapRuntime,
    attacker: ObjectGuid,
    index: usize,
) -> Option<ObjectGuid> {
    let threats = db_creature_event_ai_living_threats(map, attacker);
    if threats.is_empty() {
        return None;
    }
    if index == usize::MAX {
        threats.last().copied()
    } else {
        threats.get(index).copied()
    }
}

pub(in crate::world) fn db_creature_event_ai_random_threat_target(
    map: &MapRuntime,
    attacker: ObjectGuid,
    skip_top: usize,
) -> Option<ObjectGuid> {
    let threats = db_creature_event_ai_living_threats(map, attacker);
    if threats.len() <= skip_top {
        return None;
    }
    let candidates = &threats[skip_top..];
    candidates
        .get(rand::thread_rng().gen_range(0..candidates.len()))
        .copied()
}

pub(in crate::world) fn db_creature_event_ai_nearest_threat_target(
    map: &MapRuntime,
    attacker: ObjectGuid,
) -> Option<ObjectGuid> {
    let mut threats = db_creature_event_ai_living_threats(map, attacker);
    threats.sort_by(|left, right| {
        db_creature_spell_target_distance_squared(map, attacker, *left).total_cmp(
            &db_creature_spell_target_distance_squared(map, attacker, *right),
        )
    });
    threats.first().copied()
}

pub(in crate::world) fn db_creature_event_ai_farthest_threat_target(
    map: &MapRuntime,
    attacker: ObjectGuid,
) -> Option<ObjectGuid> {
    let mut threats = db_creature_event_ai_living_threats(map, attacker);
    threats.sort_by(|left, right| {
        db_creature_spell_target_distance_squared(map, attacker, *right).total_cmp(
            &db_creature_spell_target_distance_squared(map, attacker, *left),
        )
    });
    threats.first().copied()
}

pub(in crate::world) fn db_creature_event_ai_living_threats(
    map: &MapRuntime,
    attacker: ObjectGuid,
) -> Vec<ObjectGuid> {
    map.creature_threats
        .get(&attacker.raw())
        .into_iter()
        .flatten()
        .filter(|entry| entry.threat > 0.0)
        .filter(|entry| map.db_creature_spell_cast_target_alive(entry.victim))
        .map(|entry| entry.victim)
        .collect()
}

pub(in crate::world) fn random_millis_between_i32(min_millis: i32, max_millis: i32) -> u32 {
    random_millis_between(min_millis.max(0) as u32, max_millis.max(0) as u32)
}

pub(in crate::world) fn refresh_db_creature_spell_list_availability(
    creature: &mut DbCreatureRuntime,
    spell_list: &[wow_db::CreatureSpellListQuery],
) {
    let Some(list_id) = spell_list.first().map(|spell| spell.id) else {
        creature.spell_list_availability_id = None;
        creature.unavailable_spell_list_positions.clear();
        return;
    };
    if creature.spell_list_availability_id == Some(list_id) {
        return;
    }
    creature.spell_list_availability_id = Some(list_id);
    creature.unavailable_spell_list_positions.clear();
    let mut rng = rand::thread_rng();
    for spell in spell_list {
        if spell.availability >= 100 {
            continue;
        }
        let roll = rng.gen_range(0..=100);
        if !db_creature_spell_available_for_lifetime(spell.availability, roll) {
            creature
                .unavailable_spell_list_positions
                .insert(spell.position);
        }
    }
}

pub(in crate::world) fn db_creature_spell_available_for_lifetime(
    availability: u32,
    roll: u32,
) -> bool {
    availability >= 100 || roll <= availability
}

pub(in crate::world) fn db_creature_spell_ai_target(
    map: &MapRuntime,
    attacker: ObjectGuid,
    spell: &wow_db::CreatureSpellListQuery,
    victim: ObjectGuid,
) -> Option<ObjectGuid> {
    match spell.target_type {
        CREATURE_SPELL_LIST_TARGETING_HARDCODED
            if spell.target_id == CREATURE_SPELL_LIST_TARGET_CURRENT =>
        {
            Some(victim)
        }
        CREATURE_SPELL_LIST_TARGETING_HARDCODED
            if spell.target_id == CREATURE_SPELL_LIST_TARGET_SELF =>
        {
            Some(attacker)
        }
        CREATURE_SPELL_LIST_TARGETING_HARDCODED
            if spell.target_id == CREATURE_SPELL_LIST_TARGET_CURRENT_NOT_ALONE =>
        {
            let threat_count = map
                .creature_threats
                .get(&attacker.raw())
                .map(|threats| threats.iter().filter(|entry| entry.threat > 0.0).count())
                .unwrap_or_default();
            (threat_count > 1).then_some(victim)
        }
        CREATURE_SPELL_LIST_TARGETING_ATTACK => {
            db_creature_spell_attack_target(map, attacker, victim, spell)
        }
        CREATURE_SPELL_LIST_TARGETING_SUPPORT => {
            db_creature_spell_support_target(map, attacker, spell)
        }
        _ => None,
    }
}

pub(in crate::world) fn choose_db_creature_spell(
    eligible: &[(wow_db::CreatureSpellListQuery, ObjectGuid)],
) -> Option<(wow_db::CreatureSpellListQuery, ObjectGuid)> {
    let probability_sum = eligible
        .iter()
        .map(|(spell, _)| spell.probability)
        .sum::<u32>();
    if probability_sum == 0 {
        return eligible.first().cloned();
    }
    let mut roll = rand::thread_rng().gen_range(0..probability_sum);
    for (spell, target) in eligible {
        if roll < spell.probability {
            return Some((spell.clone(), *target));
        }
        roll -= spell.probability;
    }
    eligible.first().cloned()
}

pub(in crate::world) fn db_creature_spell_attack_target(
    map: &MapRuntime,
    attacker: ObjectGuid,
    victim: ObjectGuid,
    spell: &wow_db::CreatureSpellListQuery,
) -> Option<ObjectGuid> {
    let threats = map.creature_threats.get(&attacker.raw())?;
    let mut candidates = threats
        .iter()
        .filter(|entry| entry.threat > 0.0)
        .filter(|entry| {
            map.players
                .get(&entry.victim.counter())
                .is_some_and(|player| player.health > 0)
        })
        .map(|entry| entry.victim)
        .collect::<Vec<_>>();
    if candidates.is_empty() {
        return None;
    }
    match spell.target_param1 {
        CREATURE_ATTACKING_TARGET_TOP_AGGRO => candidates.first().copied(),
        CREATURE_ATTACKING_TARGET_BOTTOM_AGGRO => candidates.last().copied(),
        CREATURE_ATTACKING_TARGET_NEAREST => {
            candidates.sort_by(|left, right| {
                db_creature_spell_target_distance_squared(map, attacker, *left).total_cmp(
                    &db_creature_spell_target_distance_squared(map, attacker, *right),
                )
            });
            candidates.first().copied()
        }
        CREATURE_ATTACKING_TARGET_FARTHEST => {
            candidates.sort_by(|left, right| {
                db_creature_spell_target_distance_squared(map, attacker, *right).total_cmp(
                    &db_creature_spell_target_distance_squared(map, attacker, *left),
                )
            });
            candidates.first().copied()
        }
        CREATURE_ATTACKING_TARGET_RANDOM => {
            let index = rand::thread_rng().gen_range(0..candidates.len());
            candidates.get(index).copied()
        }
        _ => Some(victim),
    }
}

pub(in crate::world) fn db_creature_spell_support_target(
    map: &MapRuntime,
    attacker: ObjectGuid,
    spell: &wow_db::CreatureSpellListQuery,
) -> Option<ObjectGuid> {
    let caster = map.creatures.get(&attacker.raw())?;
    let min_missing = spell.target_param1.max(0) as f32;
    let use_percent_missing = spell.target_param2 != 0;
    let include_self = spell.target_param3 != 0;
    let max_range = CREATURE_SPAWN_RADIUS_YARDS;
    let mut candidates = map
        .creatures
        .values()
        .filter(|creature| include_self || creature.guid() != attacker)
        .filter(|creature| creature.is_alive() && !creature.is_evading_home())
        .filter(|creature| creature.spawn.template.faction == caster.spawn.template.faction)
        .filter(|creature| {
            is_position_inside_radius(
                creature.current_position,
                caster.current_position,
                max_range + creature.combat_reach() + caster.combat_reach(),
            )
        })
        .filter_map(|creature| {
            let max_health = creature.max_health().max(1);
            let missing = max_health.saturating_sub(creature.health);
            let missing_score = if use_percent_missing {
                missing as f32 * 100.0 / max_health as f32
            } else {
                missing as f32
            };
            (missing_score >= min_missing).then_some((creature.guid(), missing_score))
        })
        .collect::<Vec<_>>();
    candidates.sort_by(|left, right| {
        right
            .1
            .total_cmp(&left.1)
            .then_with(|| left.0.raw().cmp(&right.0.raw()))
    });
    candidates.first().map(|(guid, _)| *guid)
}

pub(in crate::world) fn db_creature_spell_conditions_met(
    map: &MapRuntime,
    attacker: ObjectGuid,
    target: ObjectGuid,
    spell: &wow_db::CreatureSpellListQuery,
    conditions: &DbCreatureSpellConditionCache,
) -> bool {
    if spell.target_unit_condition != -1
        && !db_creature_unit_condition_met(
            map,
            target,
            attacker,
            spell.target_unit_condition,
            conditions,
        )
    {
        return false;
    }
    match spell.combat_condition {
        -1 | 0 => true,
        id => db_creature_combat_condition_met(map, attacker, target, id, conditions),
    }
}

pub(in crate::world) fn db_creature_combat_condition_met(
    map: &MapRuntime,
    attacker: ObjectGuid,
    target: ObjectGuid,
    id: i32,
    conditions: &DbCreatureSpellConditionCache,
) -> bool {
    let Some(condition) = conditions.combat_conditions.get(&id) else {
        return false;
    };
    if condition.world_state_expression_id != 0 {
        return false;
    }
    if condition.self_condition_id != 0
        && !db_creature_unit_condition_met(
            map,
            attacker,
            attacker,
            condition.self_condition_id,
            conditions,
        )
    {
        return false;
    }
    if condition.target_condition_id != 0
        && !db_creature_unit_condition_met(
            map,
            target,
            attacker,
            condition.target_condition_id,
            conditions,
        )
    {
        return false;
    }
    if condition.friend_condition_id_0 != 0
        && !db_creature_combat_condition_counts_met(
            map,
            attacker,
            conditions,
            true,
            DbCreatureCombatConditionCountClause {
                ids: [
                    condition.friend_condition_id_0,
                    condition.friend_condition_id_1,
                ],
                ops: [
                    condition.friend_condition_op_0,
                    condition.friend_condition_op_1,
                ],
                counts: [
                    condition.friend_condition_count_0,
                    condition.friend_condition_count_1,
                ],
                logic: condition.friend_condition_logic,
            },
        )
    {
        return false;
    }
    if condition.enemy_condition_id_0 != 0
        && !db_creature_combat_condition_counts_met(
            map,
            attacker,
            conditions,
            false,
            DbCreatureCombatConditionCountClause {
                ids: [
                    condition.enemy_condition_id_0,
                    condition.enemy_condition_id_1,
                ],
                ops: [
                    condition.enemy_condition_op_0,
                    condition.enemy_condition_op_1,
                ],
                counts: [
                    condition.enemy_condition_count_0,
                    condition.enemy_condition_count_1,
                ],
                logic: condition.enemy_condition_logic,
            },
        )
    {
        return false;
    }
    true
}

pub(in crate::world) fn db_creature_combat_condition_counts_met(
    map: &MapRuntime,
    attacker: ObjectGuid,
    conditions: &DbCreatureSpellConditionCache,
    friendly: bool,
    clause: DbCreatureCombatConditionCountClause,
) -> bool {
    let units = if friendly {
        db_creature_condition_friendlies(map, attacker)
    } else {
        db_creature_condition_enemies(map, attacker)
    };
    let mut eligible = [0_i32; 2];
    for unit in units {
        for (index, count) in eligible.iter_mut().enumerate() {
            if clause.ids[index] != 0
                && db_creature_unit_condition_met(
                    map,
                    unit,
                    attacker,
                    clause.ids[index],
                    conditions,
                )
            {
                *count += 1;
            }
        }
    }
    match clause.logic {
        CONDITION_LOGIC_NONE => {
            db_creature_condition_compare(clause.ops[0] as u32, eligible[0], clause.counts[0])
        }
        CONDITION_LOGIC_AND => {
            db_creature_condition_compare(clause.ops[0] as u32, eligible[0], clause.counts[0])
                && db_creature_condition_compare(
                    clause.ops[1] as u32,
                    eligible[1],
                    clause.counts[1],
                )
        }
        CONDITION_LOGIC_OR => {
            db_creature_condition_compare(clause.ops[0] as u32, eligible[0], clause.counts[0])
                || db_creature_condition_compare(
                    clause.ops[1] as u32,
                    eligible[1],
                    clause.counts[1],
                )
        }
        CONDITION_LOGIC_XOR => {
            db_creature_condition_compare(clause.ops[0] as u32, eligible[0], clause.counts[0])
                != db_creature_condition_compare(
                    clause.ops[1] as u32,
                    eligible[1],
                    clause.counts[1],
                )
        }
        _ => false,
    }
}

pub(in crate::world) fn db_creature_unit_condition_met(
    map: &MapRuntime,
    source: ObjectGuid,
    target: ObjectGuid,
    id: i32,
    conditions: &DbCreatureSpellConditionCache,
) -> bool {
    if id == 0 {
        return true;
    }
    let Some(condition) = conditions.unit_conditions.get(&id) else {
        return false;
    };
    let variables = condition.variables();
    let operations = condition.operations();
    let values = condition.values();
    for index in 0..8 {
        let result = db_creature_unit_condition_value(map, source, target, variables[index])
            .is_some_and(|condition_value| {
                db_creature_condition_compare(operations[index], condition_value, values[index])
            });
        if result && (condition.flags & UNIT_CONDITION_FLAG_OR) != 0 {
            return true;
        }
        if !result && (condition.flags & UNIT_CONDITION_FLAG_OR) == 0 {
            return false;
        }
    }
    (condition.flags & UNIT_CONDITION_FLAG_OR) == 0
}

pub(in crate::world) fn db_creature_unit_condition_value(
    map: &MapRuntime,
    source: ObjectGuid,
    target: ObjectGuid,
    variable: u32,
) -> Option<i32> {
    match variable {
        UNIT_CONDITION_NONE => Some(1),
        UNIT_CONDITION_RACE => {
            db_creature_condition_player(map, source).map(|player| player.race as i32)
        }
        UNIT_CONDITION_CLASS => {
            db_creature_condition_player(map, source).map(|player| player.class as i32)
        }
        UNIT_CONDITION_LEVEL => db_creature_condition_level(map, source).map(i32::from),
        UNIT_CONDITION_IS_SELF => Some(i32::from(source == target)),
        UNIT_CONDITION_IS_TARGET | UNIT_CONDITION_IS_ATTACKING_ME => Some(i32::from(
            db_creature_condition_unit_target(map, source) == Some(target),
        )),
        UNIT_CONDITION_HEALTH_PERCENT => db_creature_condition_health(map, source)
            .map(|(health, max_health)| (health.saturating_mul(100) / max_health.max(1)) as i32),
        UNIT_CONDITION_HEALTH => {
            db_creature_condition_health(map, source).map(|(health, _)| health as i32)
        }
        UNIT_CONDITION_MANA_PERCENT => db_creature_condition_player(map, source)
            .map(|player| (player.power1.saturating_mul(100) / player.max_power1.max(1)) as i32),
        UNIT_CONDITION_RAGE_PERCENT => db_creature_condition_player(map, source)
            .map(|player| (player.power2.saturating_mul(100) / POWER_RAGE_DEFAULT.max(1)) as i32),
        UNIT_CONDITION_ENERGY_PERCENT => db_creature_condition_player(map, source)
            .map(|player| (player.power4.saturating_mul(100) / player.max_power4.max(1)) as i32),
        UNIT_CONDITION_IN_COMBAT => Some(i32::from(db_creature_condition_in_combat(map, source))),
        UNIT_CONDITION_NUMBER_OF_ENEMIES | UNIT_CONDITION_NUMBER_OF_ATTACKERS => {
            Some(db_creature_condition_enemies(map, source).len() as i32)
        }
        UNIT_CONDITION_NUMBER_OF_MELEE_ATTACKERS => Some(
            db_creature_condition_enemies(map, source)
                .into_iter()
                .filter(|enemy| db_creature_condition_in_melee_range(map, source, *enemy))
                .count() as i32,
        ),
        UNIT_CONDITION_NUMBER_OF_RANGED_ATTACKERS => Some(
            db_creature_condition_enemies(map, source)
                .into_iter()
                .filter(|enemy| !db_creature_condition_in_melee_range(map, source, *enemy))
                .count() as i32,
        ),
        UNIT_CONDITION_RANGE => {
            db_creature_condition_distance(map, source, target).map(|distance| distance as i32)
        }
        UNIT_CONDITION_IN_MELEE_RANGE => Some(i32::from(db_creature_condition_in_melee_range(
            map, source, target,
        ))),
        UNIT_CONDITION_CREATURE_TYPE => db_creature_condition_creature(map, source)
            .map(|creature| creature.spawn.template.creature_type as i32),
        UNIT_CONDITION_CREATURE_ID => db_creature_condition_creature(map, source)
            .map(|creature| creature.spawn.template.entry as i32),
        UNIT_CONDITION_IS_INTERRUPTIBLE => Some(i32::from(
            !source.is_player() && map.active_creature_spell_casts.contains_key(&source.raw()),
        )),
        UNIT_CONDITION_IS_MELEE_ATTACKING => {
            let unit_target = db_creature_condition_unit_target(map, source)?;
            Some(i32::from(db_creature_condition_in_melee_range(
                map,
                source,
                unit_target,
            )))
        }
        UNIT_CONDITION_IS_RANGED_ATTACKING => {
            let unit_target = db_creature_condition_unit_target(map, source)?;
            Some(i32::from(!db_creature_condition_in_melee_range(
                map,
                source,
                unit_target,
            )))
        }
        UNIT_CONDITION_IS_PLAYER => Some(i32::from(source.is_player())),
        UNIT_CONDITION_IS_ENEMY => Some(i32::from(db_creature_condition_is_enemy(
            map, source, target,
        ))),
        UNIT_CONDITION_IS_DYING => {
            db_creature_condition_health(map, source).map(|(health, _)| i32::from(health == 0))
        }
        _ => None,
    }
}

pub(in crate::world) fn db_creature_condition_compare(
    operation: u32,
    condition_value: i32,
    value: i32,
) -> bool {
    match operation {
        1 => condition_value == value,
        2 => condition_value != value,
        3 => condition_value < value,
        4 => condition_value <= value,
        5 => condition_value > value,
        6 => condition_value >= value,
        _ => true,
    }
}

pub(in crate::world) fn db_creature_condition_player(
    map: &MapRuntime,
    guid: ObjectGuid,
) -> Option<&PlayerRuntime> {
    guid.is_player()
        .then(|| map.players.get(&guid.counter()))
        .flatten()
}

pub(in crate::world) fn db_creature_condition_creature(
    map: &MapRuntime,
    guid: ObjectGuid,
) -> Option<&DbCreatureRuntime> {
    (!guid.is_player())
        .then(|| map.creatures.get(&guid.raw()))
        .flatten()
}

pub(in crate::world) fn db_creature_condition_level(
    map: &MapRuntime,
    guid: ObjectGuid,
) -> Option<u8> {
    if let Some(player) = db_creature_condition_player(map, guid) {
        Some(player.level)
    } else {
        db_creature_condition_creature(map, guid).map(|creature| creature.spawn.template.max_level)
    }
}

pub(in crate::world) fn db_creature_condition_health(
    map: &MapRuntime,
    guid: ObjectGuid,
) -> Option<(u32, u32)> {
    if let Some(player) = db_creature_condition_player(map, guid) {
        Some((player.health, player.max_health))
    } else {
        db_creature_condition_creature(map, guid)
            .map(|creature| (creature.health, creature.max_health()))
    }
}

pub(in crate::world) fn db_creature_condition_unit_target(
    map: &MapRuntime,
    guid: ObjectGuid,
) -> Option<ObjectGuid> {
    if let Some(player) = db_creature_condition_player(map, guid) {
        player.unit_target.or(player.active_combat_target)
    } else {
        map.active_creature_combats
            .get(&guid.raw())
            .map(|combat| combat.victim)
    }
}

pub(in crate::world) fn db_creature_condition_in_combat(
    map: &MapRuntime,
    guid: ObjectGuid,
) -> bool {
    if guid.is_player() {
        map.players
            .get(&guid.counter())
            .is_some_and(|player| player.in_combat)
    } else {
        map.active_creature_combats.contains_key(&guid.raw())
    }
}

pub(in crate::world) fn db_creature_condition_friendlies(
    map: &MapRuntime,
    source: ObjectGuid,
) -> Vec<ObjectGuid> {
    let Some(creature) = db_creature_condition_creature(map, source) else {
        return Vec::new();
    };
    map.creatures
        .values()
        .filter(|candidate| candidate.is_alive() && !candidate.is_evading_home())
        .filter(|candidate| candidate.spawn.template.faction == creature.spawn.template.faction)
        .filter(|candidate| {
            is_position_inside_radius(
                candidate.current_position,
                creature.current_position,
                CREATURE_SPAWN_RADIUS_YARDS + candidate.combat_reach() + creature.combat_reach(),
            )
        })
        .map(DbCreatureRuntime::guid)
        .collect()
}

pub(in crate::world) fn db_creature_condition_enemies(
    map: &MapRuntime,
    source: ObjectGuid,
) -> Vec<ObjectGuid> {
    if source.is_player() {
        return map
            .active_creature_combats
            .values()
            .filter(|combat| combat.victim == source)
            .map(|combat| combat.attacker)
            .collect();
    }
    map.creature_threats
        .get(&source.raw())
        .map(|threats| {
            threats
                .iter()
                .filter(|entry| entry.threat > 0.0)
                .map(|entry| entry.victim)
                .collect()
        })
        .unwrap_or_default()
}

pub(in crate::world) fn db_creature_condition_distance(
    map: &MapRuntime,
    source: ObjectGuid,
    target: ObjectGuid,
) -> Option<f32> {
    let source_position = db_creature_condition_position(map, source)?;
    let target_position = db_creature_condition_position(map, target)?;
    let dx = source_position.x - target_position.x;
    let dy = source_position.y - target_position.y;
    let dz = source_position.z - target_position.z;
    Some((dx * dx + dy * dy + dz * dz).sqrt())
}

pub(in crate::world) fn db_creature_condition_in_melee_range(
    map: &MapRuntime,
    source: ObjectGuid,
    target: ObjectGuid,
) -> bool {
    let Some(distance) = db_creature_condition_distance(map, source, target) else {
        return false;
    };
    let reach = db_creature_condition_combat_reach(map, source)
        + db_creature_condition_combat_reach(map, target);
    distance <= reach
}

pub(in crate::world) fn db_creature_condition_position(
    map: &MapRuntime,
    guid: ObjectGuid,
) -> Option<WorldPosition> {
    if let Some(player) = db_creature_condition_player(map, guid) {
        Some(player.position)
    } else {
        db_creature_condition_creature(map, guid).map(|creature| creature.current_position)
    }
}

pub(in crate::world) fn db_creature_condition_combat_reach(
    map: &MapRuntime,
    guid: ObjectGuid,
) -> f32 {
    if guid.is_player() {
        PLAYER_COMBAT_REACH_YARDS
    } else {
        db_creature_condition_creature(map, guid)
            .map(DbCreatureRuntime::combat_reach)
            .unwrap_or(0.0)
    }
}

pub(in crate::world) fn db_creature_condition_is_enemy(
    map: &MapRuntime,
    source: ObjectGuid,
    target: ObjectGuid,
) -> bool {
    if source.is_player() != target.is_player() {
        return true;
    }
    let Some(source_creature) = db_creature_condition_creature(map, source) else {
        return false;
    };
    let Some(target_creature) = db_creature_condition_creature(map, target) else {
        return false;
    };
    source_creature.spawn.template.faction != target_creature.spawn.template.faction
}

pub(in crate::world) fn db_creature_spell_target_distance_squared(
    map: &MapRuntime,
    attacker: ObjectGuid,
    target: ObjectGuid,
) -> f32 {
    let Some(creature) = map.creatures.get(&attacker.raw()) else {
        return f32::MAX;
    };
    let Some(player) = map.players.get(&target.counter()) else {
        return f32::MAX;
    };
    let dx = creature.current_position.x - player.position.x;
    let dy = creature.current_position.y - player.position.y;
    let dz = creature.current_position.z - player.position.z;
    dx * dx + dy * dy + dz * dz
}

pub(in crate::world) fn random_millis_between(min: u32, max: u32) -> u32 {
    let max = max.max(min);
    if min == max {
        min
    } else {
        rand::thread_rng().gen_range(min..=max)
    }
}

pub(in crate::world) fn packets_direct_to_character(
    map: &MapRuntime,
    character_guid: u32,
    creature_position: WorldPosition,
) -> bool {
    map.players.get(&character_guid).is_some_and(|player| {
        is_position_inside_radius(
            player.position,
            creature_position,
            CREATURE_SPAWN_RADIUS_YARDS,
        )
    })
}
