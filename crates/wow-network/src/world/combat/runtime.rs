use super::*;

// CMaNGOS default `CreatureRespawnAggroDelay` from mangosd.conf.dist.in.
pub(in crate::world) const CMANGOS_CREATURE_RESPAWN_AGGRO_DELAY: Duration =
    Duration::from_millis(5_000);
pub(in crate::world) const CREATURE_STATIC_FLAGS2_NO_WOUNDED_SLOWDOWN: u32 = 0x0000_0040;
pub(in crate::world) const CMANGOS_WOUNDED_SLOWDOWN_HEALTH_PERCENT: f32 = 30.0;
pub(in crate::world) const CMANGOS_WOUNDED_SLOWDOWN_PER_PERCENT: f32 = 1.67;

impl DbCreatureRuntime {
    pub(in crate::world) fn new(spawn: CreatureSpawnQuery) -> Self {
        let health = creature_health(&spawn.template);
        let power1 = creature_mana(&spawn.template);
        let home_position = db_creature_spawn_position(&spawn);
        let next_random_move_at = Self::initial_random_move_at(&spawn);
        let next_waypoint_move_at = Self::initial_waypoint_move_at(&spawn);
        let native_display = choose_creature_display(&spawn.template);
        let move_speeds = db_creature_move_speeds(&spawn.template, &[]);
        Self {
            spawn,
            home_position,
            current_position: home_position,
            motion: CreatureMotionState::Idle,
            next_random_move_at,
            next_waypoint_move_at,
            waypoint_next_index: 0,
            waypoint_forward: true,
            waypoint_resume_position: None,
            already_called_assistance: false,
            next_spline_id: 0,
            move_speeds,
            default_movement_run: false,
            chase_run: true,
            health,
            power1,
            life_state: DbCreatureLifeState::Alive,
            corpse_expires_at: None,
            respawn_at: None,
            respawn_epoch_secs: None,
            aggro_enabled_at: None,
            life_generation: 0,
            client_visible: true,
            lootable: false,
            looting: false,
            loot_money: 0,
            loot_money_available: false,
            loot_items: Vec::new(),
            loot_items_generated: false,
            loot_roll_released_slots: HashSet::new(),
            loot_current_looter_pass_slots: HashSet::new(),
            loot_owner: None,
            loot_current_looter: None,
            loot_allowed_players: HashSet::new(),
            loot_method: None,
            active_auras: Vec::new(),
            next_spell_list_update_at: None,
            spell_cooldowns_until: HashMap::new(),
            spell_list_availability_id: None,
            unavailable_spell_list_positions: HashSet::new(),
            triggered_event_ai_scripts: HashSet::new(),
            event_ai_cooldowns_until: HashMap::new(),
            native_display,
            display_id_override: None,
            pending_movement_scripts: Vec::new(),
        }
    }

    pub(in crate::world) fn guid(&self) -> ObjectGuid {
        creature_spawn_guid(&self.spawn)
    }

    pub(in crate::world) fn is_alive(&self) -> bool {
        self.life_state == DbCreatureLifeState::Alive && self.health > 0
    }

    pub(in crate::world) fn is_evading_home(&self) -> bool {
        matches!(self.motion, CreatureMotionState::ReturnHome(_))
    }

    pub(in crate::world) fn is_fleeing(&self) -> bool {
        matches!(self.motion, CreatureMotionState::Flee(_))
    }

    pub(in crate::world) fn default_movement_type(&self) -> u8 {
        if self.spawn.movement_type != DB_MOTION_TYPE_IDLE {
            self.spawn.movement_type
        } else {
            self.spawn.template.movement_type
        }
    }

    pub(in crate::world) fn new_with_persisted_respawn(
        spawn: CreatureSpawnQuery,
        now: Instant,
        now_epoch_secs: u64,
        respawn_epoch_secs: Option<u64>,
    ) -> Self {
        let mut creature = Self::new(spawn);
        if let Some(respawn_epoch_secs) = respawn_epoch_secs {
            if respawn_epoch_secs > now_epoch_secs {
                creature.health = 0;
                creature.power1 = 0;
                creature.life_state = DbCreatureLifeState::Dead;
                creature.corpse_expires_at = None;
                creature.respawn_at =
                    Some(now + Duration::from_secs(respawn_epoch_secs - now_epoch_secs));
                creature.respawn_epoch_secs = Some(respawn_epoch_secs);
                creature.aggro_enabled_at = None;
                creature.client_visible = false;
                creature.lootable = false;
                creature.looting = false;
                creature.loot_money = 0;
                creature.loot_money_available = false;
                creature.loot_items.clear();
                creature.loot_items_generated = false;
                creature.loot_roll_released_slots.clear();
                creature.loot_current_looter_pass_slots.clear();
                creature.loot_owner = None;
                creature.loot_current_looter = None;
                creature.loot_allowed_players.clear();
                creature.loot_method = None;
                creature.next_spell_list_update_at = None;
                creature.spell_cooldowns_until.clear();
                creature.spell_list_availability_id = None;
                creature.unavailable_spell_list_positions.clear();
                creature.triggered_event_ai_scripts.clear();
                creature.event_ai_cooldowns_until.clear();
                creature.motion = CreatureMotionState::Idle;
                creature.next_random_move_at = None;
                creature.next_waypoint_move_at = None;
                creature.waypoint_resume_position = None;
            }
        }
        creature
    }

    pub(in crate::world) fn random_wander_radius(&self) -> f32 {
        if self.default_movement_type() == DB_MOTION_TYPE_RANDOM {
            self.spawn.spawn_dist.max(0.0)
        } else {
            0.0
        }
    }

    pub(in crate::world) fn has_waypoint_movement(&self) -> bool {
        matches!(
            self.default_movement_type(),
            DB_MOTION_TYPE_WAYPOINT | DB_MOTION_TYPE_LINEAR_WAYPOINT
        ) && !self.spawn.waypoint_path.is_empty()
    }

    pub(in crate::world) fn initial_random_move_at(spawn: &CreatureSpawnQuery) -> Option<Instant> {
        let movement_type = if spawn.movement_type != DB_MOTION_TYPE_IDLE {
            spawn.movement_type
        } else {
            spawn.template.movement_type
        };
        (movement_type == DB_MOTION_TYPE_RANDOM && spawn.spawn_dist > 0.0).then(|| {
            Instant::now()
                + Duration::from_millis(db_creature_random_pause_millis(
                    creature_spawn_guid(spawn).raw(),
                    0,
                ))
        })
    }

    pub(in crate::world) fn initial_waypoint_move_at(
        spawn: &CreatureSpawnQuery,
    ) -> Option<Instant> {
        let movement_type = if spawn.movement_type != DB_MOTION_TYPE_IDLE {
            spawn.movement_type
        } else {
            spawn.template.movement_type
        };
        matches!(
            movement_type,
            DB_MOTION_TYPE_WAYPOINT | DB_MOTION_TYPE_LINEAR_WAYPOINT
        )
        .then_some(Instant::now())
        .filter(|_| !spawn.waypoint_path.is_empty())
    }

    pub(in crate::world) fn max_health(&self) -> u32 {
        creature_health(&self.spawn.template)
    }

    #[allow(dead_code)]
    pub(in crate::world) fn hit_damage(&self) -> u32 {
        self.spawn.template.max_melee_dmg.ceil().max(1.0) as u32
    }

    pub(in crate::world) fn melee_outcome_against_player(
        &self,
        defense: PlayerMeleeDefenseInput,
    ) -> MeleeDamageOutcome {
        roll_melee_damage(creature_melee_input_against_player(self, defense))
    }

    pub(in crate::world) fn base_attack_duration(&self) -> Duration {
        let base_millis = self.spawn.template.melee_base_attack_time.max(1) as f32;
        let multiplier = active_aura_melee_attack_time_multiplier(&self.active_auras);
        Duration::from_millis((base_millis * multiplier).round().max(1.0) as u64)
    }

    pub(in crate::world) fn combat_reach(&self) -> f32 {
        creature_combat_reach(&self.spawn.template)
    }

    pub(in crate::world) fn walk_speed(&self) -> f32 {
        self.move_speeds.walk
    }

    pub(in crate::world) fn run_speed(&self) -> f32 {
        self.move_speeds.run
    }

    pub(in crate::world) fn health_percent(&self) -> f32 {
        let max_health = self.max_health();
        if max_health == 0 {
            return 0.0;
        }
        ((self.health as f32 / max_health as f32) * 100.0).clamp(0.0, 100.0)
    }

    pub(in crate::world) fn is_wounded_slowed_in_combat(&self) -> bool {
        self.spawn.template.static_flags2 & CREATURE_STATIC_FLAGS2_NO_WOUNDED_SLOWDOWN == 0
            && self.health_percent() < CMANGOS_WOUNDED_SLOWDOWN_HEALTH_PERCENT
    }

    pub(in crate::world) fn wounded_combat_speed_multiplier(&self) -> f32 {
        if !self.is_wounded_slowed_in_combat() {
            return 1.0;
        }
        let missing_from_threshold =
            CMANGOS_WOUNDED_SLOWDOWN_HEALTH_PERCENT - self.health_percent().min(30.0);
        (1.0 - (missing_from_threshold * CMANGOS_WOUNDED_SLOWDOWN_PER_PERCENT) / 100.0)
            .clamp(0.1, 1.0)
    }

    pub(in crate::world) fn random_motion_speed(&self, run: bool) -> f32 {
        let base = if run {
            self.run_speed()
        } else {
            self.walk_speed()
        };
        base * self.wounded_combat_speed_multiplier()
    }

    pub(in crate::world) fn targeted_motion_speed(&self, run: bool) -> f32 {
        if run {
            self.run_speed() * self.wounded_combat_speed_multiplier()
        } else {
            self.walk_speed()
        }
    }

    pub(in crate::world) fn refresh_move_speeds(&mut self) -> UnitMoveSpeeds {
        let previous = self.move_speeds;
        self.move_speeds = db_creature_move_speeds(&self.spawn.template, &self.active_auras);
        previous
    }

    pub(in crate::world) fn loot_money(&self) -> u32 {
        self.loot_money
    }

    pub(in crate::world) fn roll_loot_money(&self) -> u32 {
        let min = self.spawn.template.min_loot_gold;
        let max = self.spawn.template.max_loot_gold.max(min);
        if min == max {
            min
        } else {
            rand::thread_rng().gen_range(min..=max)
        }
    }

    pub(in crate::world) fn dynamic_flags(&self) -> u32 {
        self.dynamic_flags_for_player(None)
    }

    pub(in crate::world) fn dynamic_flags_for_player(&self, character_guid: Option<u32>) -> u32 {
        if self.life_state == DbCreatureLifeState::Corpse && self.lootable {
            if self.can_loot_for_player(character_guid) {
                UNIT_DYNFLAG_LOOTABLE
            } else {
                0
            }
        } else {
            self.spawn.template.dynamic_flags
        }
    }

    pub(in crate::world) fn can_loot_for_player(&self, character_guid: Option<u32>) -> bool {
        if self.life_state != DbCreatureLifeState::Corpse || !self.lootable {
            return false;
        }
        let Some(character_guid) = character_guid else {
            if !self.loot_items_generated {
                return true;
            }
            return self.loot_money_available || !self.loot_items.is_empty();
        };
        if !self.loot_owner_allows_character(character_guid) {
            return false;
        }
        if !self.loot_items_generated {
            return true;
        }
        if self.loot_money_available {
            return true;
        }
        self.loot_items
            .iter()
            .any(|loot| self.can_loot_item_for_player(character_guid, loot))
    }

    pub(in crate::world) fn loot_owner_allows_character(&self, character_guid: u32) -> bool {
        match self.loot_owner {
            None => true,
            Some(CreatureLootOwner::Player(owner)) => owner == character_guid,
            Some(CreatureLootOwner::Party(_)) => {
                self.loot_allowed_players.is_empty()
                    || self.loot_allowed_players.contains(&character_guid)
            }
        }
    }

    pub(in crate::world) fn can_loot_item_for_player(
        &self,
        character_guid: u32,
        loot: &DbCreatureLootRuntime,
    ) -> bool {
        if loot.free_for_all {
            return true;
        }
        let Some(loot_method) = self.loot_method else {
            return true;
        };
        let under_threshold = loot.quest_drop || loot.quality < loot_method.threshold;
        match loot_method.method {
            0 => true,
            1 | 4 => {
                !under_threshold
                    || self.loot_current_looter == Some(character_guid)
                    || self.loot_current_looter_pass_slots.contains(&loot.slot)
                    || self.loot_roll_released_slots.contains(&loot.slot)
            }
            3 => {
                under_threshold
                    && (self.loot_current_looter == Some(character_guid)
                        || self.loot_current_looter_pass_slots.contains(&loot.slot))
                    || self.loot_roll_released_slots.contains(&loot.slot)
            }
            2 if under_threshold => {
                self.loot_current_looter == Some(character_guid)
                    || self.loot_current_looter_pass_slots.contains(&loot.slot)
                    || self.loot_roll_released_slots.contains(&loot.slot)
            }
            2 => true,
            _ => true,
        }
    }

    pub(in crate::world) fn begin_corpse(&mut self, now: Instant, now_epoch_secs: u64) {
        let respawn_delay = db_creature_respawn_delay(&self.spawn);
        self.health = 0;
        self.power1 = 0;
        self.life_state = DbCreatureLifeState::Corpse;
        self.life_generation = self.life_generation.saturating_add(1);
        self.active_auras.clear();
        self.next_spell_list_update_at = None;
        self.spell_cooldowns_until.clear();
        self.spell_list_availability_id = None;
        self.unavailable_spell_list_positions.clear();
        self.triggered_event_ai_scripts.clear();
        self.event_ai_cooldowns_until.clear();
        self.refresh_move_speeds();
        self.corpse_expires_at =
            Some(now + db_creature_corpse_decay_duration(&self.spawn.template, respawn_delay));
        self.respawn_at = Some(now + respawn_delay);
        self.respawn_epoch_secs = Some(now_epoch_secs + respawn_delay.as_secs());
        self.aggro_enabled_at = None;
        self.client_visible = true;
        self.lootable = true;
        self.looting = false;
        self.loot_money = self.roll_loot_money();
        self.loot_money_available = self.loot_money > 0;
        self.loot_items.clear();
        self.loot_items_generated = false;
        self.loot_roll_released_slots.clear();
        self.loot_current_looter_pass_slots.clear();
        self.loot_current_looter = None;
        self.loot_allowed_players.clear();
        self.loot_method = None;
        self.motion = CreatureMotionState::Idle;
        self.next_random_move_at = None;
        self.next_waypoint_move_at = None;
        self.waypoint_resume_position = None;
        self.already_called_assistance = false;
    }

    pub(in crate::world) fn reduce_corpse_decay_after_loot(&mut self, now: Instant) {
        if self.life_state != DbCreatureLifeState::Corpse {
            return;
        }
        if self.loot_money_available || !self.loot_items.is_empty() {
            return;
        }
        self.lootable = false;
        let reduced_expires_at = now + Duration::from_millis(CMANGOS_MINIMUM_LOOTING_TIME_MILLIS);
        if self
            .corpse_expires_at
            .is_none_or(|expires_at| expires_at > reduced_expires_at)
        {
            self.corpse_expires_at = Some(reduced_expires_at);
        }
    }

    pub(in crate::world) fn is_corpse_expired(&self, now: Instant) -> bool {
        self.life_state == DbCreatureLifeState::Corpse
            && self
                .corpse_expires_at
                .is_some_and(|expires_at| now >= expires_at)
    }

    pub(in crate::world) fn remove_corpse(&mut self) {
        self.life_state = DbCreatureLifeState::Dead;
        self.active_auras.clear();
        self.next_spell_list_update_at = None;
        self.spell_cooldowns_until.clear();
        self.spell_list_availability_id = None;
        self.unavailable_spell_list_positions.clear();
        self.triggered_event_ai_scripts.clear();
        self.event_ai_cooldowns_until.clear();
        self.refresh_move_speeds();
        self.corpse_expires_at = None;
        self.health = 0;
        self.power1 = 0;
        self.aggro_enabled_at = None;
        self.client_visible = false;
        self.lootable = false;
        self.looting = false;
        self.loot_money_available = false;
        self.loot_money = 0;
        self.loot_items.clear();
        self.loot_items_generated = false;
        self.loot_roll_released_slots.clear();
        self.loot_current_looter_pass_slots.clear();
        self.loot_owner = None;
        self.loot_current_looter = None;
        self.loot_allowed_players.clear();
        self.loot_method = None;
        self.native_display = choose_creature_display(&self.spawn.template);
        self.display_id_override = None;
        self.current_position = self.home_position;
        self.motion = CreatureMotionState::Idle;
        self.next_random_move_at = None;
        self.next_waypoint_move_at = None;
        self.waypoint_next_index = 0;
        self.waypoint_forward = true;
        self.waypoint_resume_position = None;
        self.already_called_assistance = false;
    }

    pub(in crate::world) fn is_ready_to_respawn(&self, now: Instant) -> bool {
        self.life_state == DbCreatureLifeState::Dead
            && self.respawn_at.is_none_or(|respawn_at| now >= respawn_at)
    }

    pub(in crate::world) fn respawn(&mut self, now: Instant) {
        self.health = self.max_health();
        self.power1 = creature_mana(&self.spawn.template);
        self.life_state = DbCreatureLifeState::Alive;
        self.life_generation = self.life_generation.saturating_add(1);
        self.active_auras.clear();
        self.next_spell_list_update_at = None;
        self.spell_cooldowns_until.clear();
        self.spell_list_availability_id = None;
        self.unavailable_spell_list_positions.clear();
        self.triggered_event_ai_scripts.clear();
        self.event_ai_cooldowns_until.clear();
        self.refresh_move_speeds();
        self.corpse_expires_at = None;
        self.respawn_at = None;
        self.respawn_epoch_secs = None;
        self.aggro_enabled_at = Some(now + CMANGOS_CREATURE_RESPAWN_AGGRO_DELAY);
        self.client_visible = true;
        self.lootable = false;
        self.looting = false;
        self.loot_money_available = false;
        self.loot_money = 0;
        self.loot_items.clear();
        self.loot_items_generated = false;
        self.loot_roll_released_slots.clear();
        self.loot_current_looter_pass_slots.clear();
        self.loot_owner = None;
        self.loot_current_looter = None;
        self.loot_allowed_players.clear();
        self.loot_method = None;
        self.current_position = self.home_position;
        self.motion = CreatureMotionState::Idle;
        self.next_random_move_at = Self::initial_random_move_at(&self.spawn);
        self.next_waypoint_move_at = Self::initial_waypoint_move_at(&self.spawn);
        self.waypoint_next_index = 0;
        self.waypoint_forward = true;
        self.waypoint_resume_position = None;
        self.already_called_assistance = false;
    }

    pub(in crate::world) fn can_aggro_player(
        &self,
        faction_templates: &FactionTemplateStore,
        character: &ActiveCharacter,
        now: Instant,
    ) -> bool {
        self.is_alive()
            && self
                .aggro_enabled_at
                .is_none_or(|enabled_at| now >= enabled_at)
            && !self.is_evading_home()
            && self.spawn.map == character.position.map_id
            && self.spawn.template.civilian == 0
            && self.spawn.template.creature_type != CREATURE_TYPE_CRITTER
            && self.spawn.template.npc_flags == 0
            && can_creature_attack_player_on_sight(
                faction_templates,
                self.spawn.template.faction,
                character.race,
            )
    }

    pub(in crate::world) fn distance_to_player_squared(
        &self,
        character: &ActiveCharacter,
    ) -> Option<f32> {
        (self.current_position.map_id == character.position.map_id).then(|| {
            let dx = self.current_position.x - character.position.x;
            let dy = self.current_position.y - character.position.y;
            dx * dx + dy * dy
        })
    }
}

pub(in crate::world) fn db_creature_move_speeds(
    template: &CreatureTemplateQuery,
    active_auras: &[ActiveAura],
) -> UnitMoveSpeeds {
    let walk_base = if template.speed_walk > 0.0 {
        DB_CREATURE_WALK_SPEED_YARDS_PER_SEC * template.speed_walk
    } else {
        DB_CREATURE_WALK_SPEED_YARDS_PER_SEC
    };
    let run_base = if template.speed_run > 0.0 {
        DB_CREATURE_RUN_SPEED_YARDS_PER_SEC * template.speed_run
    } else {
        DB_CREATURE_RUN_SPEED_YARDS_PER_SEC
    };
    let slow = active_aura_movement_speed_multiplier(active_auras);
    UnitMoveSpeeds {
        walk: walk_base.max(f32::EPSILON),
        run: (run_base * slow).max(f32::EPSILON),
        run_back: (DB_CREATURE_RUN_BACK_SPEED_YARDS_PER_SEC * slow).max(f32::EPSILON),
        swim: (DB_CREATURE_SWIM_SPEED_YARDS_PER_SEC * slow).max(f32::EPSILON),
        swim_back: DB_CREATURE_SWIM_BACK_SPEED_YARDS_PER_SEC,
    }
}

pub(in crate::world) const CMANGOS_MINIMUM_LOOTING_TIME_MILLIS: u64 = 2 * 60 * 1000;
pub(in crate::world) const CMANGOS_CORPSE_DECAY_NORMAL_SECS: u64 = 300;
pub(in crate::world) const CMANGOS_CORPSE_DECAY_RARE_SECS: u64 = 900;
pub(in crate::world) const CMANGOS_CORPSE_DECAY_ELITE_SECS: u64 = 600;
pub(in crate::world) const CMANGOS_CORPSE_DECAY_RARE_ELITE_SECS: u64 = 1200;
pub(in crate::world) const CMANGOS_CORPSE_DECAY_WORLD_BOSS_SECS: u64 = 3600;

pub(in crate::world) fn current_unix_epoch_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or_default()
}

pub(in crate::world) fn db_creature_corpse_decay_duration(
    template: &CreatureTemplateQuery,
    respawn_delay: Duration,
) -> Duration {
    let seconds = if template.corpse_decay != 0 {
        template.corpse_decay as u64
    } else {
        match template.rank {
            1 => CMANGOS_CORPSE_DECAY_ELITE_SECS,
            2 => CMANGOS_CORPSE_DECAY_RARE_ELITE_SECS,
            3 => CMANGOS_CORPSE_DECAY_WORLD_BOSS_SECS,
            4 => CMANGOS_CORPSE_DECAY_RARE_SECS,
            _ => CMANGOS_CORPSE_DECAY_NORMAL_SECS,
        }
    };
    let rank_or_template_delay = Duration::from_secs(seconds);
    let respawn_capped_delay = Duration::from_secs(respawn_delay.as_secs().saturating_mul(9) / 10);
    rank_or_template_delay.min(respawn_capped_delay)
}

pub(in crate::world) fn db_creature_respawn_delay(spawn: &CreatureSpawnQuery) -> Duration {
    let min = spawn.spawn_time_secs_min;
    let max = spawn.spawn_time_secs_max.max(min);
    let seconds = if min == max {
        min
    } else {
        rand::thread_rng().gen_range(min..=max)
    };
    Duration::from_secs(seconds as u64)
}

pub(in crate::world) fn db_creature_spawn_position(spawn: &CreatureSpawnQuery) -> WorldPosition {
    WorldPosition::new(
        spawn.map,
        spawn.position_x,
        spawn.position_y,
        spawn.position_z,
        spawn.orientation,
    )
}
