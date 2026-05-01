impl DbCreatureRuntime {
    fn new(spawn: CreatureSpawnQuery) -> Self {
        let health = creature_health(&spawn.template);
        let home_position = db_creature_spawn_position(&spawn);
        let next_random_move_at = Self::initial_random_move_at(&spawn);
        let next_waypoint_move_at = Self::initial_waypoint_move_at(&spawn);
        Self {
            spawn,
            home_position,
            current_position: home_position,
            motion: CreatureMotionState::Idle,
            next_random_move_at,
            next_waypoint_move_at,
            waypoint_next_index: 0,
            waypoint_forward: true,
            already_called_assistance: false,
            next_spline_id: 0,
            health,
            life_state: DbCreatureLifeState::Alive,
            corpse_expires_at: None,
            respawn_at: None,
            respawn_epoch_secs: None,
            client_visible: true,
            lootable: false,
            looting: false,
            loot_money_available: false,
            loot_item: None,
        }
    }

    fn guid(&self) -> ObjectGuid {
        creature_spawn_guid(&self.spawn)
    }

    fn is_alive(&self) -> bool {
        self.life_state == DbCreatureLifeState::Alive && self.health > 0
    }

    fn is_evading_home(&self) -> bool {
        matches!(self.motion, CreatureMotionState::ReturnHome(_))
    }

    fn default_movement_type(&self) -> u8 {
        if self.spawn.movement_type != DB_MOTION_TYPE_IDLE {
            self.spawn.movement_type
        } else {
            self.spawn.template.movement_type
        }
    }

    fn new_with_persisted_respawn(
        spawn: CreatureSpawnQuery,
        now: Instant,
        now_epoch_secs: u64,
        respawn_epoch_secs: Option<u64>,
    ) -> Self {
        let mut creature = Self::new(spawn);
        if let Some(respawn_epoch_secs) = respawn_epoch_secs {
            if respawn_epoch_secs > now_epoch_secs {
                creature.health = 0;
                creature.life_state = DbCreatureLifeState::Dead;
                creature.corpse_expires_at = None;
                creature.respawn_at =
                    Some(now + Duration::from_secs(respawn_epoch_secs - now_epoch_secs));
                creature.respawn_epoch_secs = Some(respawn_epoch_secs);
                creature.client_visible = false;
                creature.lootable = false;
                creature.looting = false;
                creature.loot_money_available = false;
                creature.loot_item = None;
                creature.motion = CreatureMotionState::Idle;
                creature.next_random_move_at = None;
                creature.next_waypoint_move_at = None;
            }
        }
        creature
    }

    fn random_wander_radius(&self) -> f32 {
        if self.default_movement_type() == DB_MOTION_TYPE_RANDOM {
            self.spawn.spawn_dist.max(0.0)
        } else {
            0.0
        }
    }

    fn has_waypoint_movement(&self) -> bool {
        matches!(
            self.default_movement_type(),
            DB_MOTION_TYPE_WAYPOINT | DB_MOTION_TYPE_LINEAR_WAYPOINT
        ) && !self.spawn.waypoint_path.is_empty()
    }

    fn initial_random_move_at(spawn: &CreatureSpawnQuery) -> Option<Instant> {
        let movement_type = if spawn.movement_type != DB_MOTION_TYPE_IDLE {
            spawn.movement_type
        } else {
            spawn.template.movement_type
        };
        (movement_type == DB_MOTION_TYPE_RANDOM && spawn.spawn_dist > 0.0).then(|| {
            Instant::now()
                + Duration::from_millis(db_creature_random_pause_millis(creature_spawn_guid(spawn).raw(), 0))
        })
    }

    fn initial_waypoint_move_at(spawn: &CreatureSpawnQuery) -> Option<Instant> {
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

    fn max_health(&self) -> u32 {
        creature_health(&self.spawn.template)
    }

    #[allow(dead_code)]
    fn hit_damage(&self) -> u32 {
        self.spawn.template.max_melee_dmg.ceil().max(1.0) as u32
    }

    fn melee_outcome_against_player(
        &self,
        defense: PlayerMeleeDefenseInput,
    ) -> MeleeDamageOutcome {
        roll_melee_damage(creature_melee_input_against_player(self, defense))
    }

    fn base_attack_duration(&self) -> Duration {
        Duration::from_millis(self.spawn.template.melee_base_attack_time.max(1) as u64)
    }

    fn combat_reach(&self) -> f32 {
        creature_combat_reach(&self.spawn.template)
    }

    fn loot_money(&self) -> u32 {
        self.spawn
            .template
            .max_loot_gold
            .max(self.spawn.template.min_loot_gold)
    }

    fn dynamic_flags(&self) -> u32 {
        if self.life_state == DbCreatureLifeState::Corpse && self.lootable {
            UNIT_DYNFLAG_LOOTABLE
        } else {
            self.spawn.template.dynamic_flags
        }
    }

    fn begin_corpse(&mut self, now: Instant, now_epoch_secs: u64) {
        let respawn_delay = db_creature_respawn_delay(&self.spawn);
        self.health = 0;
        self.life_state = DbCreatureLifeState::Corpse;
        self.corpse_expires_at = Some(now + db_creature_corpse_decay_duration(&self.spawn.template));
        self.respawn_at = Some(now + respawn_delay);
        self.respawn_epoch_secs = Some(now_epoch_secs + respawn_delay.as_secs());
        self.client_visible = true;
        self.lootable = true;
        self.looting = false;
        self.loot_money_available = self.loot_money() > 0;
        self.loot_item = None;
        self.motion = CreatureMotionState::Idle;
        self.next_random_move_at = None;
        self.next_waypoint_move_at = None;
        self.already_called_assistance = false;
    }

    fn reduce_corpse_decay_after_loot(&mut self, now: Instant) {
        if self.life_state != DbCreatureLifeState::Corpse {
            return;
        }
        if self.loot_money_available || self.loot_item.is_some() {
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

    fn is_corpse_expired(&self, now: Instant) -> bool {
        self.life_state == DbCreatureLifeState::Corpse
            && self.corpse_expires_at.is_some_and(|expires_at| now >= expires_at)
    }

    fn remove_corpse(&mut self) {
        self.life_state = DbCreatureLifeState::Dead;
        self.corpse_expires_at = None;
        self.health = 0;
        self.client_visible = false;
        self.lootable = false;
        self.looting = false;
        self.loot_money_available = false;
        self.loot_item = None;
        self.current_position = self.home_position;
        self.motion = CreatureMotionState::Idle;
        self.next_random_move_at = None;
        self.next_waypoint_move_at = None;
        self.waypoint_next_index = 0;
        self.waypoint_forward = true;
        self.already_called_assistance = false;
    }

    fn is_ready_to_respawn(&self, now: Instant) -> bool {
        self.life_state == DbCreatureLifeState::Dead
            && self.respawn_at.is_none_or(|respawn_at| now >= respawn_at)
    }

    fn respawn(&mut self) {
        self.health = self.max_health();
        self.life_state = DbCreatureLifeState::Alive;
        self.corpse_expires_at = None;
        self.respawn_at = None;
        self.respawn_epoch_secs = None;
        self.client_visible = true;
        self.lootable = false;
        self.looting = false;
        self.loot_money_available = false;
        self.loot_item = None;
        self.current_position = self.home_position;
        self.motion = CreatureMotionState::Idle;
        self.next_random_move_at = Self::initial_random_move_at(&self.spawn);
        self.next_waypoint_move_at = Self::initial_waypoint_move_at(&self.spawn);
        self.waypoint_next_index = 0;
        self.waypoint_forward = true;
        self.already_called_assistance = false;
    }

    fn can_aggro_player(&self, character: &ActiveCharacter) -> bool {
        self.is_alive()
            && !self.is_evading_home()
            && self.spawn.map == character.position.map_id
            && self.spawn.template.civilian == 0
            && self.spawn.template.creature_type != CREATURE_TYPE_CRITTER
            && self.spawn.template.npc_flags == 0
            && can_creature_attack_player_on_sight(self.spawn.template.faction, character.race)
    }

    fn distance_to_player_squared(&self, character: &ActiveCharacter) -> Option<f32> {
        (self.current_position.map_id == character.position.map_id).then(|| {
            let dx = self.current_position.x - character.position.x;
            let dy = self.current_position.y - character.position.y;
            dx * dx + dy * dy
        })
    }
}

const CMANGOS_MINIMUM_LOOTING_TIME_MILLIS: u64 = 2 * 60 * 1000;
const CMANGOS_CORPSE_DECAY_NORMAL_SECS: u64 = 300;
const CMANGOS_CORPSE_DECAY_RARE_SECS: u64 = 900;
const CMANGOS_CORPSE_DECAY_ELITE_SECS: u64 = 600;
const CMANGOS_CORPSE_DECAY_RARE_ELITE_SECS: u64 = 1200;
const CMANGOS_CORPSE_DECAY_WORLD_BOSS_SECS: u64 = 3600;

fn current_unix_epoch_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or_default()
}

fn db_creature_corpse_decay_duration(template: &CreatureTemplateQuery) -> Duration {
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
    Duration::from_secs(seconds)
}

fn db_creature_respawn_delay(spawn: &CreatureSpawnQuery) -> Duration {
    let min = spawn.spawn_time_secs_min;
    let max = spawn.spawn_time_secs_max.max(min);
    let seconds = if min == max {
        min
    } else {
        rand::thread_rng().gen_range(min..=max)
    };
    Duration::from_secs(seconds as u64)
}

fn db_creature_spawn_position(spawn: &CreatureSpawnQuery) -> WorldPosition {
    WorldPosition::new(
        spawn.map,
        spawn.position_x,
        spawn.position_y,
        spawn.position_z,
        spawn.orientation,
    )
}

