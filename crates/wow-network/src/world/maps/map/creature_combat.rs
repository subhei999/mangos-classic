// Shared DB-creature combat claim and player-damage authority.
const DB_CREATURE_DEFAULT_PURSUIT_MILLIS: u32 = 15_000;

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
        let Some(victim_player) = self.players.get_mut(&victim.counter()) else {
            return Ok(None);
        };
        let damage = outcome.total_damage;
        victim_player.health = victim_player.health.saturating_sub(damage);
        let victim_health = victim_player.health;
        let victim_position = victim_player.position;
        let _ = victim_player;
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
            body: build_player_health_update_body(victim, victim_health)?,
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

fn packets_direct_to_character(
    map: &MapRuntime,
    character_guid: u32,
    creature_position: WorldPosition,
) -> bool {
    map.players
        .get(&character_guid)
        .is_some_and(|player| is_position_inside_radius(player.position, creature_position, CREATURE_SPAWN_RADIUS_YARDS))
}
