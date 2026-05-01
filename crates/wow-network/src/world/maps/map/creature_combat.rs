// Shared DB-creature combat claim and player-damage authority.

impl MapRuntime {
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
        Some(combat)
    }

    fn clear_db_creature_combat(&mut self, attacker: ObjectGuid) {
        self.active_creature_combats.remove(&attacker.raw());
    }

    fn clear_db_creature_combats_for_victim(&mut self, victim: ObjectGuid) {
        self.active_creature_combats
            .retain(|_, combat| combat.victim != victim);
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
        next_swing_at: Instant,
    ) -> anyhow::Result<Option<DbCreaturePlayerDamageEvent>> {
        self.apply_db_creature_player_melee_outcome(
            attacker,
            victim,
            MeleeDamageOutcome::normal_hit(damage),
            next_swing_at,
        )
    }

    fn apply_db_creature_player_melee_outcome(
        &mut self,
        attacker: ObjectGuid,
        victim: ObjectGuid,
        outcome: MeleeDamageOutcome,
        next_swing_at: Instant,
    ) -> anyhow::Result<Option<DbCreaturePlayerDamageEvent>> {
        let Some(combat) = self.active_creature_combats.get_mut(&attacker.raw()) else {
            return Ok(None);
        };
        if combat.victim != victim {
            return Ok(None);
        }
        let Some(victim_player) = self.players.get_mut(&victim.counter()) else {
            return Ok(None);
        };
        let damage = outcome.total_damage;
        victim_player.health = victim_player.health.saturating_sub(damage);
        combat.next_swing_at = next_swing_at;
        let combat = *combat;
        let victim_health = victim_player.health;
        let victim_position = victim_player.position;
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
            observer_packets.push((player.session_id, attacker_state.clone()));
            observer_packets.push((player.session_id, health_update.clone()));
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
}
