// Shared DB-creature combat claim and player-damage authority.
const DB_CREATURE_DEFAULT_PURSUIT_MILLIS: u32 = 15_000;
const DB_CREATURE_SPELL_LIST_UPDATE_MILLIS: u64 = 1_200;
const CREATURE_SPELL_LIST_FLAG_SUPPORT_ACTION: u32 = 0x1;
const CREATURE_SPELL_LIST_FLAG_RANGED_ACTION: u32 = 0x2;
const CREATURE_SPELL_LIST_FLAG_NON_BLOCKING: u32 = 0x8;
const CREATURE_SPELL_LIST_TARGETING_HARDCODED: u32 = 0;
const CREATURE_SPELL_LIST_TARGETING_ATTACK: u32 = 1;
const CREATURE_SPELL_LIST_TARGETING_SUPPORT: u32 = 2;
const CREATURE_SPELL_LIST_TARGET_CURRENT: u32 = 1;
const CREATURE_SPELL_LIST_TARGET_SELF: u32 = 2;
const CREATURE_SPELL_LIST_TARGET_CURRENT_NOT_ALONE: u32 = 7;
const CREATURE_ATTACKING_TARGET_RANDOM: i32 = 0;
const CREATURE_ATTACKING_TARGET_TOP_AGGRO: i32 = 1;
const CREATURE_ATTACKING_TARGET_BOTTOM_AGGRO: i32 = 2;
const CREATURE_ATTACKING_TARGET_NEAREST: i32 = 3;
const CREATURE_ATTACKING_TARGET_FARTHEST: i32 = 4;
const UNIT_CONDITION_FLAG_OR: u32 = 0x1;
const CONDITION_LOGIC_NONE: i32 = 0;
const CONDITION_LOGIC_AND: i32 = 1;
const CONDITION_LOGIC_OR: i32 = 2;
const CONDITION_LOGIC_XOR: i32 = 3;
const UNIT_CONDITION_NONE: u32 = 0;
const UNIT_CONDITION_RACE: u32 = 1;
const UNIT_CONDITION_CLASS: u32 = 2;
const UNIT_CONDITION_LEVEL: u32 = 3;
const UNIT_CONDITION_IS_SELF: u32 = 4;
const UNIT_CONDITION_IS_TARGET: u32 = 7;
const UNIT_CONDITION_HEALTH_PERCENT: u32 = 12;
const UNIT_CONDITION_MANA_PERCENT: u32 = 13;
const UNIT_CONDITION_RAGE_PERCENT: u32 = 14;
const UNIT_CONDITION_ENERGY_PERCENT: u32 = 15;
const UNIT_CONDITION_IN_COMBAT: u32 = 31;
const UNIT_CONDITION_NUMBER_OF_MELEE_ATTACKERS: u32 = 37;
const UNIT_CONDITION_IS_ATTACKING_ME: u32 = 38;
const UNIT_CONDITION_RANGE: u32 = 39;
const UNIT_CONDITION_IN_MELEE_RANGE: u32 = 40;
const UNIT_CONDITION_NUMBER_OF_ENEMIES: u32 = 44;
const UNIT_CONDITION_NUMBER_OF_ATTACKERS: u32 = 54;
const UNIT_CONDITION_NUMBER_OF_RANGED_ATTACKERS: u32 = 55;
const UNIT_CONDITION_CREATURE_TYPE: u32 = 56;
const UNIT_CONDITION_IS_MELEE_ATTACKING: u32 = 57;
const UNIT_CONDITION_IS_RANGED_ATTACKING: u32 = 58;
const UNIT_CONDITION_HEALTH: u32 = 59;
const UNIT_CONDITION_IS_INTERRUPTIBLE: u32 = 53;
const UNIT_CONDITION_IS_PLAYER: u32 = 63;
const UNIT_CONDITION_CREATURE_ID: u32 = 74;
const UNIT_CONDITION_IS_ENEMY: u32 = 77;
const UNIT_CONDITION_IS_DYING: u32 = 83;

#[derive(Debug, Clone, Default)]
struct DbCreatureSpellConditionCache {
    unit_conditions: std::collections::HashMap<i32, wow_db::UnitConditionQuery>,
    combat_conditions: std::collections::HashMap<i32, wow_db::CombatConditionQuery>,
}

#[derive(Debug, Clone, Copy)]
struct DbCreatureCombatConditionCountClause {
    ids: [i32; 2],
    ops: [i32; 2],
    counts: [i32; 2],
    logic: i32,
}

#[derive(Debug, Clone)]
struct DbCreaturePlayerMeleeValidation {
    check: PlayerMeleeCheck,
}

#[derive(Debug, Clone)]
struct PlayerChargeValidation {
    check: PlayerChargeCheck,
}

#[derive(Debug, Clone)]
struct PlayerSpellTargetValidation {
    check: PlayerSpellTargetCheck,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PlayerChargeCheck {
    Clear,
    NoActiveCharacter,
    MissingTarget,
    TargetNotAlive,
    NavigationBlocked(DbCreatureNavigationResult),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PlayerSpellTargetCheck {
    Clear,
    NoActiveCharacter,
    MissingTarget,
    TargetNotAlive,
    NavigationBlocked(DbCreatureNavigationResult),
    OutOfRange,
    BadFacing,
}

#[derive(Debug, Clone)]
struct ActiveDbCreatureCombatSnapshot {
    combat: CreatureCombatState,
    creature: DbCreatureRuntime,
}

#[derive(Debug, Clone)]
struct ReadyDbCreatureSpellCast {
    spell: wow_db::CreatureSpellListQuery,
    target: ObjectGuid,
}

impl MapRuntime {
    fn db_creature_combat_snapshot(&self, creature_guid: ObjectGuid) -> Option<DbCreatureRuntime> {
        self.creatures
            .get(&creature_guid.raw())
            .filter(|creature| creature.is_alive() && !creature.is_evading_home())
            .cloned()
    }

    fn validate_player_melee_against_db_creature(
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
        if !creature.is_alive() || creature.is_evading_home() {
            return DbCreaturePlayerMeleeValidation {
                check: PlayerMeleeCheck::TargetNotAlive,
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

    fn validate_player_charge_against_db_creature(
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

    fn validate_player_spell_against_db_creature(
        &self,
        character_guid: u32,
        target: ObjectGuid,
        navigation: &DbCreatureNavigationGuardrail,
        range: Option<SpellRangeEntry>,
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
            let range_mod = PLAYER_COMBAT_REACH_YARDS + creature.combat_reach();
            let min_range = if range.min_range > 0.0
                && (range.flags & SPELL_RANGE_FLAG_RANGED) == 0
            {
                range.min_range + range_mod
            } else {
                range.min_range
            };
            let max_range = if range.max_range > 0.0 {
                range.max_range + range_mod
            } else {
                range.max_range
            };
            if max_range > 0.0 && distance_squared > max_range * max_range {
                return PlayerSpellTargetValidation {
                    check: PlayerSpellTargetCheck::OutOfRange,
                };
            }
            if min_range > 0.0 && distance_squared < min_range * min_range {
                return PlayerSpellTargetValidation {
                    check: PlayerSpellTargetCheck::OutOfRange,
                };
            }
        }
        if !has_in_arc(player.position, creature.current_position, SPELL_CAST_ARC_RADIANS) {
            return PlayerSpellTargetValidation {
                check: PlayerSpellTargetCheck::BadFacing,
            };
        }
        PlayerSpellTargetValidation {
            check: PlayerSpellTargetCheck::Clear,
        }
    }

    fn active_db_creature_combat_snapshot(
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

    fn ready_db_creature_spell_cast(
        &mut self,
        attacker: ObjectGuid,
        victim: ObjectGuid,
        spell_list: &[wow_db::CreatureSpellListQuery],
        conditions: &DbCreatureSpellConditionCache,
        now: Instant,
    ) -> Option<ReadyDbCreatureSpellCast> {
        if spell_list.is_empty() || self.active_creature_spell_casts.contains_key(&attacker.raw()) {
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
            .filter(|(spell, _)| {
                cooldowns_until
                    .get(&spell.spell_id)
                    .is_none_or(|cooldown| now >= *cooldown)
            })
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

    fn set_db_creature_spell_repeat_cooldown(
        &mut self,
        attacker: ObjectGuid,
        spell_id: u32,
        repeat_min: u32,
        repeat_max: u32,
        now: Instant,
    ) {
        let cooldown_millis = random_millis_between(repeat_min, repeat_max);
        if cooldown_millis == 0 {
            return;
        }
        if let Some(creature) = self.creatures.get_mut(&attacker.raw()) {
            creature
                .spell_cooldowns_until
                .insert(spell_id, now + Duration::from_millis(cooldown_millis as u64));
        }
    }

    fn start_db_creature_spell_cast(
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
        };
        let start_body = build_spell_start_body(
            cast.caster,
            cast.spell_id,
            cast.cast_time_millis,
            &targets,
        )?;
        let start_packet = OutboundWorldPacket {
            opcode: SMSG_SPELL_START,
            body: start_body,
        };
        let (position, motion_stop_packet, power_update_packet) = {
            let creature = self.creatures.get_mut(&cast.caster.raw()).expect("checked above");
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
            let power_update_packet = (cast.mana_cost > 0).then(|| {
                Ok::<_, anyhow::Error>(OutboundWorldPacket {
                    opcode: SMSG_UPDATE_OBJECT,
                    body: build_db_creature_power_update_body(cast.caster, creature.power1)?,
                })
            }).transpose()?;
            (position, motion_stop_packet, power_update_packet)
        };
        self.active_creature_spell_casts.insert(cast.caster.raw(), cast);
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

    fn complete_ready_db_creature_spell_cast(
        &mut self,
        attacker: ObjectGuid,
        victim: ObjectGuid,
        now: Instant,
    ) -> anyhow::Result<Option<DbCreatureCompletedSpellCastEvent>> {
        let Some(cast) = self.active_creature_spell_casts.get(&attacker.raw()).cloned() else {
            return Ok(None);
        };
        if now < cast.due_at {
            return Ok(None);
        }
        if self
            .active_creature_combats
            .get(&attacker.raw())
            .is_none_or(|combat| combat.victim != victim)
        {
            return Ok(None);
        }
        self.active_creature_spell_casts.remove(&attacker.raw());
        let targets = SpellCastTargets {
            target_mask: SPELL_CAST_TARGET_UNIT,
            unit_target: Some(cast.target),
            gameobject_target: None,
        };
        let spell_go_body = build_spell_go_body(attacker, cast.spell_id, &targets)?;
        let aura_event = if let Some(aura) = cast.aura {
            if cast.target.is_player() {
                self.apply_player_aura(cast.target.counter(), aura)?
            } else {
                None
            }
        } else {
            None
        };
        let effect = match cast.effect {
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
                DbCreatureCompletedSpellEffect::PlayerDamage(damage)
            }
            ActiveDbCreatureSpellEffect::Heal { amount } => {
                let Some(heal) =
                    self.apply_db_creature_creature_spell_heal(attacker, cast.target, cast.spell_id, amount)?
                else {
                    return Ok(None);
                };
                DbCreatureCompletedSpellEffect::CreatureHeal(heal)
            }
        };
        Ok(Some(DbCreatureCompletedSpellCastEvent {
            spell_go_body,
            effect,
            aura_event,
        }))
    }

    fn active_db_creature_spell_cast_due_at(&self, attacker: ObjectGuid) -> Option<Instant> {
        self.active_creature_spell_casts
            .get(&attacker.raw())
            .map(|cast| cast.due_at)
    }

    fn begin_db_creature_combat(
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
        self.active_creature_combats.insert(attacker.raw(), combat);
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

    fn clear_db_creature_combat(&mut self, attacker: ObjectGuid) {
        self.active_creature_combats.remove(&attacker.raw());
        self.active_creature_spell_casts.remove(&attacker.raw());
        self.creature_combat_leash.remove(&attacker.raw());
        self.creature_threats.remove(&attacker.raw());
        if let Some(position) = self
            .creatures
            .get(&attacker.raw())
            .map(|creature| creature.current_position)
        {
            self.refresh_grid_state(grid_coord_for_position(position));
        }
    }

    fn clear_db_creature_combats_for_victim(&mut self, victim: ObjectGuid) {
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
        for grid in changed_grids {
            self.refresh_grid_state(grid);
        }
    }

    fn active_db_creature_combats_for_victim(
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
    fn apply_db_creature_player_damage(
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

    fn apply_db_creature_player_melee_outcome(
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
        if !self
            .players
            .get(&victim.counter())
            .is_some_and(|player| player.health > 0 && player.death_state == PlayerDeathState::Alive)
        {
            return Ok(None);
        };
        let damage = outcome.total_damage;
        let Some(applied) = self.apply_player_world_damage(
            victim,
            Some(attacker),
            damage,
            WorldDamageKind::Melee,
            now,
        )? else {
            return Ok(None);
        };
        let victim_health = applied.remaining_health;
        let victim_position = applied.position;
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
            body: applied.health_packet.body.clone(),
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
        Ok(Some(DbCreaturePlayerDamageEvent {
            damage,
            victim_health,
            combat,
            observer_packets,
        }))
    }

    #[allow(clippy::too_many_arguments, dead_code)]
    fn apply_db_creature_player_spell_damage(
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
            player_spell_snapshot(victim_player.level, victim_player.class, &victim_player.combat_stats),
        ));
        let requested_damage = outcome.final_damage;
        let damage = victim_player.health.min(requested_damage);
        let Some(applied) = self.apply_player_world_damage(
            victim,
            Some(attacker),
            damage,
            WorldDamageKind::SpellDirect,
            now,
        )? else {
            return Ok(None);
        };
        let victim_health = applied.remaining_health;
        let victim_position = applied.position;
        if damage > 0 {
            self.add_db_creature_threat(attacker, victim, damage as f32);
            self.refresh_db_creature_combat_leash(attacker, now);
        }
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
        let health_update_body = applied.health_packet.body.clone();
        let mut observer_packets = Vec::new();
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
            direct_packets: applied.direct_packets,
            health_update_body,
            observer_packets,
        }))
    }

    fn apply_db_creature_creature_spell_heal(
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
        let spell_heal_log_body = build_spell_heal_log_body(caster, target, spell_id, amount, false)?;
        let health_update_body = build_db_creature_state_update_body(target, target_health, dynamic_flags)?;
        let mut observer_packets = Vec::new();
        for player_guid in self.nearby_player_guids(target_position, CREATURE_SPAWN_RADIUS_YARDS, None)
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

    fn db_creature_spell_cast_target_alive(&self, target: ObjectGuid) -> bool {
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

    fn defer_ready_db_creature_swing_retry(
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

    fn refresh_db_creature_combat_leash(&mut self, attacker: ObjectGuid, now: Instant) {
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

    fn db_creature_should_evade(&self, attacker: ObjectGuid, now: Instant) -> bool {
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

    fn add_db_creature_threat(
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
    fn db_creature_threat_entries(&self, attacker: ObjectGuid) -> Vec<CreatureThreatEntry> {
        self.creature_threats
            .get(&attacker.raw())
            .cloned()
            .unwrap_or_default()
    }

    fn select_db_creature_threat_victim(
        &self,
        attacker: ObjectGuid,
        current_victim: Option<ObjectGuid>,
    ) -> Option<ObjectGuid> {
        let threats = self.creature_threats.get(&attacker.raw())?;
        let current_entry = current_victim
            .and_then(|victim| threats.iter().find(|entry| entry.victim == victim));

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

    fn switch_db_creature_threat_victim_if_needed(
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
        let Some(creature) = self.creatures.get(&attacker.raw()) else {
            return Ok(None);
        };
        let mut combat = current_combat;
        combat.victim = new_victim;
        self.active_creature_combats.insert(attacker.raw(), combat);

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
        let direct_packets = if exclude_character_guid.is_some_and(|guid| {
            packets_direct_to_character(self, guid, creature.current_position)
        }) {
            packets.to_vec()
        } else {
            Vec::new()
        };
        let observer_packets = self
            .nearby_player_guids(
                creature.current_position,
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

    fn db_creature_threat_victim_in_melee(
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

fn db_creature_pursuit_duration_millis(creature: &DbCreatureRuntime) -> Duration {
    let pursuit_millis = if creature.spawn.template.pursuit == 0 {
        DB_CREATURE_DEFAULT_PURSUIT_MILLIS
    } else {
        creature.spawn.template.pursuit
    };
    Duration::from_millis(pursuit_millis as u64)
}

fn initialize_db_creature_spell_cooldowns(
    creature: &mut DbCreatureRuntime,
    spell_list: &[wow_db::CreatureSpellListQuery],
    now: Instant,
) {
    for spell in spell_list {
        let cooldown_millis = random_millis_between(spell.initial_min, spell.initial_max);
        if cooldown_millis == 0 {
            continue;
        }
        creature
            .spell_cooldowns_until
            .insert(spell.spell_id, now + Duration::from_millis(cooldown_millis as u64));
    }
}

fn refresh_db_creature_spell_list_availability(
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

fn db_creature_spell_available_for_lifetime(availability: u32, roll: u32) -> bool {
    availability >= 100 || roll <= availability
}

fn db_creature_spell_ai_target(
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

fn choose_db_creature_spell(
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

fn db_creature_spell_attack_target(
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
                db_creature_spell_target_distance_squared(map, attacker, *left)
                    .total_cmp(&db_creature_spell_target_distance_squared(map, attacker, *right))
            });
            candidates.first().copied()
        }
        CREATURE_ATTACKING_TARGET_FARTHEST => {
            candidates.sort_by(|left, right| {
                db_creature_spell_target_distance_squared(map, attacker, *right)
                    .total_cmp(&db_creature_spell_target_distance_squared(map, attacker, *left))
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

fn db_creature_spell_support_target(
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

fn db_creature_spell_conditions_met(
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

fn db_creature_combat_condition_met(
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

fn db_creature_combat_condition_counts_met(
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
                && db_creature_unit_condition_met(map, unit, attacker, clause.ids[index], conditions)
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
                && db_creature_condition_compare(clause.ops[1] as u32, eligible[1], clause.counts[1])
        }
        CONDITION_LOGIC_OR => {
            db_creature_condition_compare(clause.ops[0] as u32, eligible[0], clause.counts[0])
                || db_creature_condition_compare(clause.ops[1] as u32, eligible[1], clause.counts[1])
        }
        CONDITION_LOGIC_XOR => {
            db_creature_condition_compare(clause.ops[0] as u32, eligible[0], clause.counts[0])
                != db_creature_condition_compare(clause.ops[1] as u32, eligible[1], clause.counts[1])
        }
        _ => false,
    }
}

fn db_creature_unit_condition_met(
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

fn db_creature_unit_condition_value(
    map: &MapRuntime,
    source: ObjectGuid,
    target: ObjectGuid,
    variable: u32,
) -> Option<i32> {
    match variable {
        UNIT_CONDITION_NONE => Some(1),
        UNIT_CONDITION_RACE => db_creature_condition_player(map, source).map(|player| player.race as i32),
        UNIT_CONDITION_CLASS => {
            db_creature_condition_player(map, source).map(|player| player.class as i32)
        }
        UNIT_CONDITION_LEVEL => db_creature_condition_level(map, source).map(i32::from),
        UNIT_CONDITION_IS_SELF => Some(i32::from(source == target)),
        UNIT_CONDITION_IS_TARGET | UNIT_CONDITION_IS_ATTACKING_ME => {
            Some(i32::from(db_creature_condition_unit_target(map, source) == Some(target)))
        }
        UNIT_CONDITION_HEALTH_PERCENT => db_creature_condition_health(map, source)
            .map(|(health, max_health)| (health.saturating_mul(100) / max_health.max(1)) as i32),
        UNIT_CONDITION_HEALTH => db_creature_condition_health(map, source).map(|(health, _)| health as i32),
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
        UNIT_CONDITION_IN_MELEE_RANGE => {
            Some(i32::from(db_creature_condition_in_melee_range(map, source, target)))
        }
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
        UNIT_CONDITION_IS_ENEMY => Some(i32::from(db_creature_condition_is_enemy(map, source, target))),
        UNIT_CONDITION_IS_DYING => db_creature_condition_health(map, source).map(|(health, _)| i32::from(health == 0)),
        _ => None,
    }
}

fn db_creature_condition_compare(operation: u32, condition_value: i32, value: i32) -> bool {
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

fn db_creature_condition_player(map: &MapRuntime, guid: ObjectGuid) -> Option<&PlayerRuntime> {
    guid.is_player()
        .then(|| map.players.get(&guid.counter()))
        .flatten()
}

fn db_creature_condition_creature(
    map: &MapRuntime,
    guid: ObjectGuid,
) -> Option<&DbCreatureRuntime> {
    (!guid.is_player())
        .then(|| map.creatures.get(&guid.raw()))
        .flatten()
}

fn db_creature_condition_level(map: &MapRuntime, guid: ObjectGuid) -> Option<u8> {
    if let Some(player) = db_creature_condition_player(map, guid) {
        Some(player.level)
    } else {
        db_creature_condition_creature(map, guid).map(|creature| creature.spawn.template.max_level)
    }
}

fn db_creature_condition_health(map: &MapRuntime, guid: ObjectGuid) -> Option<(u32, u32)> {
    if let Some(player) = db_creature_condition_player(map, guid) {
        Some((player.health, player.max_health))
    } else {
        db_creature_condition_creature(map, guid)
            .map(|creature| (creature.health, creature.max_health()))
    }
}

fn db_creature_condition_unit_target(map: &MapRuntime, guid: ObjectGuid) -> Option<ObjectGuid> {
    if let Some(player) = db_creature_condition_player(map, guid) {
        player.unit_target.or(player.active_combat_target)
    } else {
        map.active_creature_combats
            .get(&guid.raw())
            .map(|combat| combat.victim)
    }
}

fn db_creature_condition_in_combat(map: &MapRuntime, guid: ObjectGuid) -> bool {
    if guid.is_player() {
        map.players
            .get(&guid.counter())
            .is_some_and(|player| player.active_combat_target.is_some())
    } else {
        map.active_creature_combats.contains_key(&guid.raw())
    }
}

fn db_creature_condition_friendlies(map: &MapRuntime, source: ObjectGuid) -> Vec<ObjectGuid> {
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

fn db_creature_condition_enemies(map: &MapRuntime, source: ObjectGuid) -> Vec<ObjectGuid> {
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

fn db_creature_condition_distance(
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

fn db_creature_condition_in_melee_range(
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

fn db_creature_condition_position(
    map: &MapRuntime,
    guid: ObjectGuid,
) -> Option<WorldPosition> {
    if let Some(player) = db_creature_condition_player(map, guid) {
        Some(player.position)
    } else {
        db_creature_condition_creature(map, guid).map(|creature| creature.current_position)
    }
}

fn db_creature_condition_combat_reach(map: &MapRuntime, guid: ObjectGuid) -> f32 {
    if guid.is_player() {
        PLAYER_COMBAT_REACH_YARDS
    } else {
        db_creature_condition_creature(map, guid)
            .map(DbCreatureRuntime::combat_reach)
            .unwrap_or(0.0)
    }
}

fn db_creature_condition_is_enemy(
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

fn db_creature_spell_target_distance_squared(
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

fn random_millis_between(min: u32, max: u32) -> u32 {
    let max = max.max(min);
    if min == max {
        min
    } else {
        rand::thread_rng().gen_range(min..=max)
    }
}

fn packets_direct_to_character(
    map: &MapRuntime,
    character_guid: u32,
    creature_position: WorldPosition,
) -> bool {
    map.players
        .get(&character_guid)
        .is_some_and(|player| is_position_inside_radius(player.position, creature_position, CREATURE_SPAWN_RADIUS_YARDS))
}
