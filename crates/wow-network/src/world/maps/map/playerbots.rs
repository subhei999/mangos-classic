use super::*;

// Map-owned playerbot actor updates. Bots emit movement intent; MapRuntime owns
// the player-like position, visibility, cell buckets, and observer packets.

#[derive(Debug, Default)]
pub(in crate::world) struct PlayerbotMovementTick {
    pub(in crate::world) advanced_bots: u32,
    pub(in crate::world) budget_exhausted: bool,
    pub(in crate::world) packets: Vec<(SessionId, OutboundWorldPacket)>,
}

#[derive(Debug, Default)]
pub(in crate::world) struct PlayerbotCombatTick {
    pub(in crate::world) advanced_bots: u32,
    pub(in crate::world) creature_swings: u32,
    pub(in crate::world) budget_exhausted: bool,
    pub(in crate::world) packets: Vec<(SessionId, OutboundWorldPacket)>,
}

#[derive(Debug, Clone)]
pub(in crate::world) struct PlayerbotMovementUpdate {
    pub(in crate::world) opcode: u16,
    pub(in crate::world) movement: MovementInfo,
}

#[derive(Debug, Default)]
pub(in crate::world) struct PlayerbotPlannerBudget {
    pub(in crate::world) route_plans_remaining: usize,
    pub(in crate::world) combat_thinks_remaining: usize,
    pub(in crate::world) route_budget_exhausted: bool,
    pub(in crate::world) combat_budget_exhausted: bool,
}

pub(in crate::world) const PLAYERBOT_ROAM_THINK_INTERVAL: Duration = Duration::from_millis(1_250);
pub(in crate::world) const PLAYERBOT_COMBAT_THINK_INTERVAL: Duration = Duration::from_millis(500);
pub(in crate::world) const PLAYERBOT_PLANNER_TICK_INTERVAL: Duration = Duration::from_millis(100);
pub(in crate::world) const PLAYERBOT_MAX_MOVES_PER_MAP_TICK: usize = 1028;
pub(in crate::world) const PLAYERBOT_MAX_UNOBSERVED_MOVES_PER_MAP_TICK: usize = 128;
pub(in crate::world) const PLAYERBOT_MAX_ACTIVE_COMBAT_ACTIONS_PER_MAP_TICK: usize = 16;
pub(in crate::world) const PLAYERBOT_MAX_COMBAT_THINKS_PER_MAP_TICK: usize = 4;
pub(in crate::world) const PLAYERBOT_MAX_CREATURE_SWINGS_PER_MAP_TICK: usize = 128;
pub(in crate::world) const PLAYERBOT_MAX_ROUTE_PLANS_PER_MAP_TICK: usize = 2;
pub(in crate::world) const PLAYERBOT_TARGET_SEARCH_RADIUS_YARDS: f32 = 35.0;
pub(in crate::world) const PLAYERBOT_WANDER_MIN_RADIUS_YARDS: f32 = 35.0;
pub(in crate::world) const PLAYERBOT_WANDER_MAX_RADIUS_YARDS: f32 = 180.0;
pub(in crate::world) const PLAYERBOT_RUN_SPEED_YARDS_PER_SECOND: f32 = 7.0;
pub(in crate::world) const PLAYERBOT_DESTINATION_EPSILON_YARDS: f32 = 0.05;
pub(in crate::world) const PLAYERBOT_ROUTE_MIN_SEGMENT_YARDS: f32 = 14.0;
pub(in crate::world) const PLAYERBOT_ROUTE_TURN_ANGLE_RADIANS: f32 = 0.35;
pub(in crate::world) const PLAYERBOT_TRAVEL_ROUTE_LEG_YARDS: f32 = 180.0;
pub(in crate::world) const PLAYERBOT_TRAVEL_ROUTE_LEG_CANDIDATES_YARDS: [f32; 5] =
    [180.0, 120.0, 80.0, 40.0, 20.0];
pub(in crate::world) const PLAYERBOT_ROUTE_REANCHOR_YARDS: f32 = 4.0;
pub(in crate::world) const PLAYERBOT_TRAVEL_ARRIVED_RECHECK_INTERVAL: Duration =
    Duration::from_secs(30);
pub(in crate::world) const PLAYERBOT_ROUTE_PLAN_DEFER_BASE_MILLIS: u64 = 100;
pub(in crate::world) const PLAYERBOT_ROUTE_PLAN_FAILED_RETRY_MILLIS: u64 = 5_000;
pub(in crate::world) const PLAYERBOT_MISSING_INTENT_DEFER_BASE_MILLIS: u64 = 150;
pub(in crate::world) const PLAYERBOT_ENGAGE_FAILED_BACKOFF_MILLIS: u64 = 7_500;

impl MapRuntime {
    #[cfg(test)]
    pub(in crate::world) fn plan_playerbot_intents_for_test(
        &mut self,
        faction_templates: &FactionTemplateStore,
        navigation: &DbCreatureNavigationGuardrail,
        now: Instant,
    ) -> PlayerbotPlanningTick {
        let inputs = self.collect_playerbot_plan_inputs(now);
        let mut budget = PlayerbotPlannerBudget {
            route_plans_remaining: PLAYERBOT_MAX_ROUTE_PLANS_PER_MAP_TICK,
            combat_thinks_remaining: PLAYERBOT_MAX_COMBAT_THINKS_PER_MAP_TICK,
            ..PlayerbotPlannerBudget::default()
        };
        let planned = plan_playerbot_intents(inputs, faction_templates, navigation, &mut budget);
        let planned_bots = planned.len() as u32;
        let intents = planned
            .into_iter()
            .map(|(_, bot_guid, intent)| (bot_guid, intent))
            .collect();
        self.queue_playerbot_intents(intents);
        PlayerbotPlanningTick {
            planned_bots,
            route_budget_exhausted: budget.route_budget_exhausted,
            combat_budget_exhausted: budget.combat_budget_exhausted,
        }
    }

    pub(in crate::world) fn collect_playerbot_plan_inputs(
        &self,
        now: Instant,
    ) -> Vec<PlayerbotPlanInput> {
        self.players
            .iter()
            .filter_map(|(guid, player)| {
                let bot = player.bot_runtime.as_ref()?;
                if player.health == 0 || !self.playerbot_is_in_active_grid(player) {
                    return None;
                }

                let position = bot
                    .active_leg
                    .map(|leg| playerbot_position_on_leg(leg, now))
                    .unwrap_or(player.position);
                let movement_due_at =
                    (bot.active_leg.is_none() && bot.route.is_empty() && now >= bot.next_think_at)
                        .then_some(bot.next_think_at);
                let combat_due_at = (player.active_combat_target.is_none()
                    && now >= bot.next_combat_think_at)
                    .then_some(bot.next_combat_think_at);
                if movement_due_at.is_none() && combat_due_at.is_none() {
                    return None;
                }

                let engage_target_creature = bot
                    .engage_target
                    .and_then(|target| self.db_creature_snapshot(target));
                let nearby_creatures = if combat_due_at.is_some() && bot.engage_target.is_none() {
                    self.nearby_db_creature_snapshots(
                        position,
                        PLAYERBOT_TARGET_SEARCH_RADIUS_YARDS,
                        16,
                    )
                } else {
                    Vec::new()
                };

                Some(PlayerbotPlanInput {
                    map_id: self.map_id,
                    instance_id: self.instance_id,
                    bot_guid: *guid,
                    position,
                    home_position: bot.home_position,
                    travel_destination: bot.travel_destination,
                    roam_step: bot.roam_step,
                    player_race: player.race,
                    movement_due_at,
                    combat_due_at,
                    engage_target: bot.engage_target,
                    engage_target_creature,
                    nearby_creatures,
                    geometry: self.geometry.clone(),
                })
            })
            .collect()
    }

    pub(in crate::world) fn queue_playerbot_intents(
        &mut self,
        intents: Vec<(u32, PlayerbotQueuedIntents)>,
    ) {
        for (bot_guid, intent) in intents {
            if intent.is_empty()
                || self
                    .players
                    .get(&bot_guid)
                    .is_none_or(|player| player.bot_runtime.is_none())
            {
                continue;
            }
            let queued = self.playerbot_intents.entry(bot_guid).or_default();
            if intent.movement.is_some() {
                queued.movement = intent.movement;
            }
            if intent.combat.is_some() {
                queued.combat = intent.combat;
            }
        }
    }

    pub(in crate::world) fn take_playerbot_movement_intent(
        &mut self,
        bot_guid: u32,
    ) -> Option<PlayerbotMovementIntent> {
        let intent = self
            .playerbot_intents
            .get_mut(&bot_guid)
            .and_then(|queued| queued.movement.take());
        if self
            .playerbot_intents
            .get(&bot_guid)
            .is_some_and(PlayerbotQueuedIntents::is_empty)
        {
            self.playerbot_intents.remove(&bot_guid);
        }
        intent
    }

    pub(in crate::world) fn take_playerbot_combat_intent(
        &mut self,
        bot_guid: u32,
    ) -> Option<PlayerbotCombatIntent> {
        let intent = self
            .playerbot_intents
            .get_mut(&bot_guid)
            .and_then(|queued| queued.combat.take());
        if self
            .playerbot_intents
            .get(&bot_guid)
            .is_some_and(PlayerbotQueuedIntents::is_empty)
        {
            self.playerbot_intents.remove(&bot_guid);
        }
        intent
    }

    pub(in crate::world) fn playerbot_debug_snapshots(
        &self,
        now: Instant,
    ) -> Vec<crate::observability::PlayerbotDebugSnapshot> {
        self.players
            .values()
            .filter_map(|player| {
                let bot = player.bot_runtime.as_ref()?;
                let active_leg_remaining_millis = bot.active_leg.map(|leg| {
                    leg.arrival_time
                        .saturating_duration_since(now)
                        .as_millis()
                        .min(u64::MAX as u128) as u64
                });
                let state = if player.active_combat_target.is_some() {
                    "combat"
                } else if bot.engage_target.is_some() {
                    "engaging"
                } else if bot.active_leg.is_some() {
                    "moving"
                } else if !bot.route.is_empty() {
                    "queued_route"
                } else if bot.travel_destination.is_some_and(|destination| {
                    player.position.distance_2d(&destination) <= PLAYERBOT_DESTINATION_EPSILON_YARDS
                }) {
                    "travel_arrived"
                } else if now >= bot.next_think_at {
                    "planning_due"
                } else if bot.travel_destination.is_some() {
                    "waiting_retry_or_budget"
                } else {
                    "idle"
                };
                Some(crate::observability::PlayerbotDebugSnapshot {
                    guid: player.guid,
                    map_id: self.map_id,
                    instance_id: self.instance_id,
                    x: player.position.x,
                    y: player.position.y,
                    z: player.position.z,
                    travel_x: bot.travel_destination.map(|position| position.x),
                    travel_y: bot.travel_destination.map(|position| position.y),
                    travel_z: bot.travel_destination.map(|position| position.z),
                    distance_to_travel: bot
                        .travel_destination
                        .map(|destination| player.position.distance_2d(&destination)),
                    active_leg_destination_x: bot.active_leg.map(|leg| leg.destination.x),
                    active_leg_destination_y: bot.active_leg.map(|leg| leg.destination.y),
                    active_leg_destination_z: bot.active_leg.map(|leg| leg.destination.z),
                    active_leg_remaining_millis,
                    route_len: bot.route.len(),
                    next_think_in_millis: bot
                        .next_think_at
                        .saturating_duration_since(now)
                        .as_millis()
                        .min(u64::MAX as u128) as u64,
                    movement_flags: player.movement_flags,
                    state,
                })
            })
            .collect()
    }

    pub(in crate::world) fn advance_playerbot_movement_tick(
        &mut self,
        _navigation: &DbCreatureNavigationGuardrail,
        now: Instant,
    ) -> anyhow::Result<PlayerbotMovementTick> {
        #[cfg(test)]
        self.plan_playerbot_intents_for_test(
            &FactionTemplateStore::fallback_bridge(),
            _navigation,
            now,
        );

        let has_client_players = self
            .players
            .values()
            .any(PlayerRuntime::is_client_controlled);
        let move_budget = if has_client_players {
            PLAYERBOT_MAX_MOVES_PER_MAP_TICK
        } else {
            PLAYERBOT_MAX_UNOBSERVED_MOVES_PER_MAP_TICK
        };

        let mut due_bot_guids = self
            .players
            .iter()
            .filter_map(|(guid, player)| {
                let bot = player.bot_runtime.as_ref()?;
                let due_at = playerbot_movement_due_at(bot);
                if now >= due_at && self.playerbot_is_in_active_grid(player) {
                    Some((*guid, due_at))
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();
        due_bot_guids.sort_by_key(|(guid, due_at)| (*due_at, *guid));

        let budget_exhausted = due_bot_guids.len() > move_budget;
        due_bot_guids.truncate(move_budget);

        let mut packets = Vec::new();
        let mut advanced_bots = 0;
        for (bot_guid, _) in due_bot_guids {
            let Some(update) = self.prepare_playerbot_roam_movement(bot_guid, now) else {
                continue;
            };
            let server_time = update.movement.client_time;
            if has_client_players {
                packets.extend(self.update_player_position(
                    bot_guid,
                    update.opcode,
                    &update.movement,
                    server_time,
                )?);
            } else {
                self.commit_unobserved_playerbot_movement(bot_guid, &update.movement, server_time);
            }
            advanced_bots += 1;
        }

        Ok(PlayerbotMovementTick {
            advanced_bots,
            budget_exhausted,
            packets,
        })
    }

    pub(in crate::world) fn advance_playerbot_combat_tick(
        &mut self,
        faction_templates: &FactionTemplateStore,
        navigation: &DbCreatureNavigationGuardrail,
        now: Instant,
    ) -> anyhow::Result<PlayerbotCombatTick> {
        #[cfg(test)]
        self.plan_playerbot_intents_for_test(faction_templates, navigation, now);

        let mut due_active_bot_guids = Vec::new();
        let mut due_think_bot_guids = Vec::new();
        for (guid, player) in &self.players {
            let Some(bot) = player.bot_runtime.as_ref() else {
                continue;
            };
            if player.health == 0 || !self.playerbot_is_in_active_grid(player) {
                continue;
            }
            let due_at = playerbot_combat_due_at(player, bot, now);
            if now < due_at {
                continue;
            }
            if player.active_combat_target.is_some() {
                due_active_bot_guids.push((*guid, due_at));
            } else {
                due_think_bot_guids.push((*guid, due_at));
            }
        }
        due_active_bot_guids.sort_by_key(|(guid, due_at)| (*due_at, *guid));
        due_think_bot_guids.sort_by_key(|(guid, due_at)| (*due_at, *guid));

        let active_budget_exhausted =
            due_active_bot_guids.len() > PLAYERBOT_MAX_ACTIVE_COMBAT_ACTIONS_PER_MAP_TICK;
        let think_budget_exhausted =
            due_think_bot_guids.len() > PLAYERBOT_MAX_COMBAT_THINKS_PER_MAP_TICK;
        due_active_bot_guids.truncate(PLAYERBOT_MAX_ACTIVE_COMBAT_ACTIONS_PER_MAP_TICK);
        due_think_bot_guids.truncate(PLAYERBOT_MAX_COMBAT_THINKS_PER_MAP_TICK);
        let due_bot_guids = due_active_bot_guids
            .into_iter()
            .chain(due_think_bot_guids)
            .collect::<Vec<_>>();

        let mut due_creature_combats = self
            .active_creature_combats
            .values()
            .filter(|combat| now >= combat.next_swing_at)
            .filter(|combat| {
                self.players
                    .get(&combat.victim.counter())
                    .is_some_and(|player| player.bot_runtime.is_some() && player.health > 0)
            })
            .copied()
            .collect::<Vec<_>>();
        due_creature_combats.sort_by_key(|combat| combat.attacker.raw());

        let creature_budget_exhausted =
            due_creature_combats.len() > PLAYERBOT_MAX_CREATURE_SWINGS_PER_MAP_TICK;
        due_creature_combats.truncate(PLAYERBOT_MAX_CREATURE_SWINGS_PER_MAP_TICK);

        let budget_exhausted =
            active_budget_exhausted || think_budget_exhausted || creature_budget_exhausted;

        let mut packets = Vec::new();
        let mut advanced_bots = 0;
        for (bot_guid, _) in due_bot_guids {
            let action_packets =
                self.advance_single_playerbot_combat(bot_guid, faction_templates, navigation, now)?;
            if !action_packets.is_empty() {
                advanced_bots += 1;
                packets.extend(action_packets);
            }
        }

        let mut creature_swings = 0;
        for combat in due_creature_combats {
            let action_packets = self.advance_playerbot_creature_swing(combat, navigation, now)?;
            if !action_packets.is_empty() {
                creature_swings += 1;
                packets.extend(action_packets);
            }
        }

        Ok(PlayerbotCombatTick {
            advanced_bots,
            creature_swings,
            budget_exhausted,
            packets,
        })
    }

    pub(in crate::world) fn advance_single_playerbot_combat(
        &mut self,
        bot_guid: u32,
        faction_templates: &FactionTemplateStore,
        navigation: &DbCreatureNavigationGuardrail,
        now: Instant,
    ) -> anyhow::Result<Vec<(SessionId, OutboundWorldPacket)>> {
        let Some(player) = self.players.get(&bot_guid) else {
            return Ok(Vec::new());
        };
        if player.health == 0 {
            return self.clear_playerbot_auto_attack(bot_guid);
        }
        if let Some(target) = player.active_combat_target {
            return self.advance_playerbot_auto_attack(bot_guid, target, navigation, now);
        }
        let Some(intent) = self.take_playerbot_combat_intent(bot_guid) else {
            return Ok(Vec::new());
        };
        self.apply_playerbot_combat_intent(bot_guid, intent, faction_templates, navigation, now)
    }

    pub(in crate::world) fn apply_playerbot_combat_intent(
        &mut self,
        bot_guid: u32,
        intent: PlayerbotCombatIntent,
        faction_templates: &FactionTemplateStore,
        navigation: &DbCreatureNavigationGuardrail,
        now: Instant,
    ) -> anyhow::Result<Vec<(SessionId, OutboundWorldPacket)>> {
        match intent {
            PlayerbotCombatIntent::NoTarget => {
                if let Some(bot) = self
                    .players
                    .get_mut(&bot_guid)
                    .and_then(|player| player.bot_runtime.as_mut())
                {
                    bot.engage_target = None;
                    bot.route.clear();
                    bot.next_combat_think_at = now + playerbot_next_combat_think_delay(bot_guid);
                }
                Ok(Vec::new())
            }
            PlayerbotCombatIntent::Target { target, route } => self
                .advance_playerbot_planned_target(
                    bot_guid,
                    target,
                    route,
                    faction_templates,
                    navigation,
                    now,
                ),
        }
    }

    pub(in crate::world) fn advance_playerbot_planned_target(
        &mut self,
        bot_guid: u32,
        target: ObjectGuid,
        route: Option<Vec<WorldPosition>>,
        faction_templates: &FactionTemplateStore,
        navigation: &DbCreatureNavigationGuardrail,
        now: Instant,
    ) -> anyhow::Result<Vec<(SessionId, OutboundWorldPacket)>> {
        let Some(creature) = self.db_creature_snapshot(target) else {
            return self.clear_playerbot_engage_target(bot_guid);
        };
        let Some(player) = self.players.get(&bot_guid) else {
            return Ok(Vec::new());
        };
        if !playerbot_can_attack_db_creature(faction_templates, &creature, player.race) {
            return self.clear_playerbot_engage_target(bot_guid);
        }

        let bot_position = player
            .bot_runtime
            .as_ref()
            .and_then(|bot| bot.active_leg)
            .map(|leg| playerbot_position_on_leg(leg, now))
            .unwrap_or(player.position);
        let has_active_leg = player
            .bot_runtime
            .as_ref()
            .is_some_and(|bot| bot.active_leg.is_some());
        let can_melee_from_position =
            playerbot_can_melee_db_creature_from_position(bot_position, &creature, navigation);
        let mut packets = if has_active_leg {
            self.stop_playerbot_active_leg_for_combat(bot_guid, now)?
        } else {
            Vec::new()
        };
        self.face_playerbot_toward(bot_guid, creature.current_position);
        if can_melee_from_position
            && self
                .validate_player_melee_against_db_creature(bot_guid, target, navigation)
                .check
                == PlayerMeleeCheck::Clear
        {
            packets.extend(self.start_playerbot_attack(bot_guid, target, now)?);
            return Ok(packets);
        }

        packets.extend(self.set_playerbot_engage_target(bot_guid, target, now)?);
        if let Some(bot) = self
            .players
            .get_mut(&bot_guid)
            .and_then(|player| player.bot_runtime.as_mut())
        {
            if let Some(route) = route {
                bot.route = route;
                bot.next_think_at = now;
            } else if bot.route.is_empty() && bot.active_leg.is_none() {
                bot.next_think_at = now + playerbot_failed_route_retry_delay(bot_guid);
            }
        }
        Ok(packets)
    }

    #[allow(dead_code)]
    pub(in crate::world) fn start_playerbot_attack_if_target_available(
        &mut self,
        bot_guid: u32,
        faction_templates: &FactionTemplateStore,
        navigation: &DbCreatureNavigationGuardrail,
        now: Instant,
    ) -> anyhow::Result<Vec<(SessionId, OutboundWorldPacket)>> {
        let Some(player) = self.players.get(&bot_guid) else {
            return Ok(Vec::new());
        };
        let bot_position = player
            .bot_runtime
            .as_ref()
            .and_then(|bot| bot.active_leg)
            .map(|leg| playerbot_position_on_leg(leg, now))
            .unwrap_or(player.position);
        let player_race = player.race;
        let candidates = self.nearby_db_creature_snapshots(
            bot_position,
            PLAYERBOT_TARGET_SEARCH_RADIUS_YARDS,
            16,
        );

        for creature in candidates {
            if !playerbot_can_attack_db_creature(faction_templates, &creature, player_race) {
                continue;
            }
            let target = creature.guid();
            let has_active_leg = self
                .players
                .get(&bot_guid)
                .and_then(|player| player.bot_runtime.as_ref())
                .is_some_and(|bot| bot.active_leg.is_some());
            let can_melee_from_position =
                playerbot_can_melee_db_creature_from_position(bot_position, &creature, navigation);
            let mut packets = if has_active_leg {
                self.stop_playerbot_active_leg_for_combat(bot_guid, now)?
            } else {
                Vec::new()
            };
            self.face_playerbot_toward(bot_guid, creature.current_position);
            if can_melee_from_position
                && self
                    .validate_player_melee_against_db_creature(bot_guid, target, navigation)
                    .check
                    == PlayerMeleeCheck::Clear
            {
                packets.extend(self.start_playerbot_attack(bot_guid, target, now)?);
                return Ok(packets);
            }
            packets.extend(self.set_playerbot_engage_target(bot_guid, target, now)?);
            return Ok(packets);
        }

        if let Some(bot) = self
            .players
            .get_mut(&bot_guid)
            .and_then(|player| player.bot_runtime.as_mut())
        {
            bot.engage_target = None;
            bot.route.clear();
            bot.next_combat_think_at = now + playerbot_next_combat_think_delay(bot_guid);
        }
        Ok(Vec::new())
    }

    #[allow(dead_code)]
    pub(in crate::world) fn advance_playerbot_engagement(
        &mut self,
        bot_guid: u32,
        target: ObjectGuid,
        navigation: &DbCreatureNavigationGuardrail,
        now: Instant,
    ) -> anyhow::Result<Vec<(SessionId, OutboundWorldPacket)>> {
        let Some(creature) = self.db_creature_snapshot(target) else {
            return self.clear_playerbot_engage_target(bot_guid);
        };
        if !creature.is_alive() || creature.is_evading_home() {
            return self.clear_playerbot_engage_target(bot_guid);
        }

        let Some(player) = self.players.get(&bot_guid) else {
            return Ok(Vec::new());
        };
        let bot_position = player
            .bot_runtime
            .as_ref()
            .and_then(|bot| bot.active_leg)
            .map(|leg| playerbot_position_on_leg(leg, now))
            .unwrap_or(player.position);
        let has_active_leg = player
            .bot_runtime
            .as_ref()
            .is_some_and(|bot| bot.active_leg.is_some());
        let can_melee_from_position =
            playerbot_can_melee_db_creature_from_position(bot_position, &creature, navigation);
        if can_melee_from_position {
            let mut packets = if has_active_leg {
                self.stop_playerbot_active_leg_for_combat(bot_guid, now)?
            } else {
                Vec::new()
            };
            self.face_playerbot_toward(bot_guid, creature.current_position);
            if self
                .validate_player_melee_against_db_creature(bot_guid, target, navigation)
                .check
                == PlayerMeleeCheck::Clear
            {
                packets.extend(self.start_playerbot_attack(bot_guid, target, now)?);
                return Ok(packets);
            }
            return Ok(packets);
        }

        if let Some(bot) = self
            .players
            .get_mut(&bot_guid)
            .and_then(|player| player.bot_runtime.as_mut())
        {
            if !has_active_leg && bot.route.is_empty() {
                bot.next_think_at = now;
            }
            bot.next_combat_think_at = now + playerbot_next_combat_think_delay(bot_guid);
        }
        Ok(Vec::new())
    }

    pub(in crate::world) fn set_playerbot_engage_target(
        &mut self,
        bot_guid: u32,
        target: ObjectGuid,
        now: Instant,
    ) -> anyhow::Result<Vec<(SessionId, OutboundWorldPacket)>> {
        if let Some(bot) = self
            .players
            .get_mut(&bot_guid)
            .and_then(|player| player.bot_runtime.as_mut())
        {
            bot.engage_target = Some(target);
            bot.route.clear();
            bot.next_think_at = now;
            bot.next_combat_think_at = now + playerbot_next_combat_think_delay(bot_guid);
        }
        self.update_player_selection(bot_guid, Some(target))
    }

    pub(in crate::world) fn clear_playerbot_engage_target(
        &mut self,
        bot_guid: u32,
    ) -> anyhow::Result<Vec<(SessionId, OutboundWorldPacket)>> {
        if let Some(bot) = self
            .players
            .get_mut(&bot_guid)
            .and_then(|player| player.bot_runtime.as_mut())
        {
            bot.engage_target = None;
            bot.route.clear();
        }
        self.update_player_selection(bot_guid, None)
    }

    pub(in crate::world) fn stop_playerbot_active_leg_for_combat(
        &mut self,
        bot_guid: u32,
        now: Instant,
    ) -> anyhow::Result<Vec<(SessionId, OutboundWorldPacket)>> {
        let has_client_players = self
            .players
            .values()
            .any(PlayerRuntime::is_client_controlled);
        let geometry = self.geometry.clone();
        let Some(player) = self.players.get_mut(&bot_guid) else {
            return Ok(Vec::new());
        };
        let Some(bot) = player.bot_runtime.as_mut() else {
            return Ok(Vec::new());
        };
        let Some(leg) = bot.active_leg else {
            return Ok(Vec::new());
        };

        let stop_position =
            playerbot_grounded_position(Some(&geometry), playerbot_position_on_leg(leg, now));
        bot.active_leg = None;
        bot.route.clear();
        bot.next_think_at = now + playerbot_next_roam_delay(bot_guid, bot.roam_step);
        bot.next_combat_think_at = now + playerbot_next_combat_think_delay(bot_guid);
        bot.movement_start_retries_remaining = 0;
        let server_time = player.server_time.wrapping_add(
            now.saturating_duration_since(leg.start_time)
                .as_millis()
                .min(u32::MAX as u128) as u32,
        );
        let movement = MovementInfo {
            flags: 0,
            client_time: server_time,
            position: stop_position,
            fall_time: 0,
            jump: JumpInfo::default(),
        };
        if has_client_players {
            self.update_player_position(bot_guid, MSG_MOVE_STOP as u16, &movement, server_time)
        } else {
            self.commit_unobserved_playerbot_movement(bot_guid, &movement, server_time);
            Ok(Vec::new())
        }
    }

    pub(in crate::world) fn start_playerbot_attack(
        &mut self,
        bot_guid: u32,
        target: ObjectGuid,
        now: Instant,
    ) -> anyhow::Result<Vec<(SessionId, OutboundWorldPacket)>> {
        let bot_object = ObjectGuid::new(HighGuid::Player, 0, bot_guid);
        self.set_player_auto_attack(bot_guid, Some(target), Some(now));
        if let Some(bot) = self
            .players
            .get_mut(&bot_guid)
            .and_then(|player| player.bot_runtime.as_mut())
        {
            bot.next_combat_think_at = now + playerbot_next_combat_think_delay(bot_guid);
        }

        let mut packets = self.update_player_selection(bot_guid, Some(target))?;
        packets.extend(self.broadcast_nearby_player_packet(
            bot_guid,
            PLAYER_VISIBILITY_RADIUS_YARDS,
            OutboundWorldPacket {
                opcode: SMSG_ATTACKSTART,
                body: build_attack_start_body(bot_object, target),
            },
        ));
        packets.extend(self.broadcast_nearby_player_packet(
            bot_guid,
            PLAYER_VISIBILITY_RADIUS_YARDS,
            OutboundWorldPacket {
                opcode: SMSG_UPDATE_OBJECT,
                body: build_unit_flags_update_body(bot_object, player_unit_flags(true))?,
            },
        ));

        if let Some(creature) = self.db_creature_combat_snapshot(target) {
            if self
                .begin_db_creature_combat(target, bot_object, now)
                .is_some()
            {
                packets.extend(self.broadcast_packet_near_position(
                    creature.current_position,
                    CREATURE_SPAWN_RADIUS_YARDS,
                    None,
                    OutboundWorldPacket {
                        opcode: SMSG_ATTACKSTART,
                        body: build_attack_start_body(target, bot_object),
                    },
                ));
                packets.extend(self.broadcast_packet_near_position(
                    creature.current_position,
                    CREATURE_SPAWN_RADIUS_YARDS,
                    None,
                    OutboundWorldPacket {
                        opcode: SMSG_UPDATE_OBJECT,
                        body: build_unit_flags_update_body(
                            target,
                            db_creature_unit_flags(&creature, true),
                        )?,
                    },
                ));
            }
        }
        Ok(packets)
    }

    pub(in crate::world) fn advance_playerbot_auto_attack(
        &mut self,
        bot_guid: u32,
        target: ObjectGuid,
        navigation: &DbCreatureNavigationGuardrail,
        now: Instant,
    ) -> anyhow::Result<Vec<(SessionId, OutboundWorldPacket)>> {
        let Some(target_creature) = self.db_creature_snapshot(target) else {
            return self.clear_playerbot_auto_attack(bot_guid);
        };
        if !target_creature.is_alive() {
            return self.clear_playerbot_auto_attack(bot_guid);
        }

        self.face_playerbot_toward(bot_guid, target_creature.current_position);
        let validation =
            self.validate_player_melee_against_db_creature(bot_guid, target, navigation);
        if validation.check != PlayerMeleeCheck::Clear {
            if matches!(
                validation.check,
                PlayerMeleeCheck::MissingTarget | PlayerMeleeCheck::TargetNotAlive
            ) {
                return self.clear_playerbot_auto_attack(bot_guid);
            }
            if let Some(bot) = self
                .players
                .get_mut(&bot_guid)
                .and_then(|player| player.bot_runtime.as_mut())
            {
                bot.engage_target = Some(target);
                if bot.active_leg.is_none() && bot.route.is_empty() {
                    bot.next_think_at = now;
                }
            }
            self.set_player_next_swing_at(
                bot_guid,
                Some(now + Duration::from_millis(DB_CREATURE_MELEE_RETRY_MILLIS)),
            );
            return Ok(Vec::new());
        }

        let Some(player) = self.players.get(&bot_guid) else {
            return Ok(Vec::new());
        };
        let bot_object = ObjectGuid::new(HighGuid::Player, 0, bot_guid);
        let combat_stats = player.combat_stats;
        let player_level = player.level;
        let melee_outcome = player_main_hand_melee_outcome_against_db_creature(
            &combat_stats,
            player_level,
            0,
            &target_creature,
        );
        let Some(event) = self.apply_db_creature_damage(DbCreatureDamageRequest {
            creature_guid: target,
            killer: bot_object,
            damage: melee_outcome.total_damage,
            melee_outcome: Some(melee_outcome),
            spell_damage_outcome: None,
            spell_id: None,
            spell_school: 0,
            suppress_attacker_state: false,
            now,
            now_epoch_secs: current_unix_epoch_secs(),
            exclude_character_guid: None,
            corpse_loot: None,
        })?
        else {
            return self.clear_playerbot_auto_attack(bot_guid);
        };

        let is_dead = event.death_finalization.is_some();
        let damage = event.damage;
        let mut packets = event.observer_packets;
        if let Some(target_switch) = event.target_switch {
            packets.extend(target_switch.observer_packets);
        }
        if let Some(death_finalization) = event.death_finalization {
            packets.extend(death_finalization.observer_packets);
        }

        if is_dead {
            self.set_player_auto_attack(bot_guid, None, None);
            if let Some(bot) = self
                .players
                .get_mut(&bot_guid)
                .and_then(|player| player.bot_runtime.as_mut())
            {
                bot.engage_target = None;
                bot.route.clear();
            }
            packets.extend(self.update_player_selection(bot_guid, None)?);
        } else {
            self.set_player_next_swing_at(
                bot_guid,
                Some(now + Duration::from_millis(combat_stats.main_attack_time_ms.max(1) as u64)),
            );
        }
        if let Some(bot) = self
            .players
            .get_mut(&bot_guid)
            .and_then(|player| player.bot_runtime.as_mut())
        {
            bot.next_combat_think_at = now + playerbot_next_combat_think_delay(bot_guid);
        }

        let rage_gain = rage_gain_from_main_hand_white_damage(
            damage,
            player_level,
            combat_stats.main_attack_time_ms,
            melee_outcome.outcome,
        );
        if rage_gain > 0 {
            let new_rage = self
                .players
                .get(&bot_guid)
                .map(|player| {
                    player
                        .power2
                        .saturating_add(rage_gain)
                        .min(POWER_RAGE_DEFAULT)
                })
                .unwrap_or(0);
            self.set_player_power2(bot_guid, new_rage);
            packets.extend(self.broadcast_nearby_player_packet(
                bot_guid,
                PLAYER_VISIBILITY_RADIUS_YARDS,
                OutboundWorldPacket {
                    opcode: SMSG_UPDATE_OBJECT,
                    body: build_player_rage_update_body(bot_object, new_rage)?,
                },
            ));
        }

        Ok(packets)
    }

    pub(in crate::world) fn advance_playerbot_creature_swing(
        &mut self,
        combat: CreatureCombatState,
        navigation: &DbCreatureNavigationGuardrail,
        now: Instant,
    ) -> anyhow::Result<Vec<(SessionId, OutboundWorldPacket)>> {
        let bot_guid = combat.victim.counter();
        let Some(player) = self.players.get(&bot_guid) else {
            self.clear_db_creature_combat(combat.attacker);
            return Ok(Vec::new());
        };
        if player.bot_runtime.is_none() || player.health == 0 {
            self.clear_db_creature_combat(combat.attacker);
            return Ok(Vec::new());
        }

        self.advance_db_creature_motion(combat.attacker, now);
        let Some(active) = self.active_db_creature_combat_snapshot(combat.attacker, combat.victim)
        else {
            return Ok(Vec::new());
        };
        let Some(victim) = self.players.get(&bot_guid) else {
            return Ok(Vec::new());
        };
        if !playerbot_creature_can_reach_player(&active.creature, victim, navigation) {
            self.defer_ready_db_creature_swing_retry(combat.attacker, combat.victim, now);
            return Ok(Vec::new());
        }
        if !has_in_arc(
            active.creature.current_position,
            victim.position,
            PLAYER_MELEE_ARC_RADIANS,
        ) {
            self.defer_ready_db_creature_swing_retry(combat.attacker, combat.victim, now);
            return Ok(Vec::new());
        }

        let victim_level = victim.level;
        let defense = PlayerMeleeDefenseInput {
            level: victim_level.max(1),
            defense_skill: 0,
            armor: victim.combat_stats.armor,
            block_value: victim.combat_stats.shield_block_value,
            dodge_percent: victim.combat_stats.dodge_percent,
            parry_percent: victim.combat_stats.parry_percent,
            block_percent: victim.combat_stats.block_percent,
        };
        let outcome = active.creature.melee_outcome_against_player(defense);
        let Some(event) = self.apply_db_creature_player_melee_outcome(
            combat.attacker,
            combat.victim,
            outcome,
            now,
            now + active.creature.base_attack_duration(),
        )?
        else {
            self.clear_db_creature_combat(combat.attacker);
            return Ok(Vec::new());
        };

        let mut packets = event.observer_packets;
        let rage_gain = rage_gain_from_damage_taken(event.damage, victim_level);
        if rage_gain > 0 {
            let new_rage = self
                .players
                .get(&bot_guid)
                .map(|player| {
                    player
                        .power2
                        .saturating_add(rage_gain)
                        .min(POWER_RAGE_DEFAULT)
                })
                .unwrap_or(0);
            self.set_player_power2(bot_guid, new_rage);
            packets.extend(self.broadcast_nearby_player_packet(
                bot_guid,
                PLAYER_VISIBILITY_RADIUS_YARDS,
                OutboundWorldPacket {
                    opcode: SMSG_UPDATE_OBJECT,
                    body: build_player_rage_update_body(combat.victim, new_rage)?,
                },
            ));
        }
        if event.victim_health == 0 {
            packets.extend(self.clear_playerbot_auto_attack(bot_guid)?);
            self.clear_db_creature_combats_for_victim(combat.victim);
        }
        Ok(packets)
    }

    pub(in crate::world) fn clear_playerbot_auto_attack(
        &mut self,
        bot_guid: u32,
    ) -> anyhow::Result<Vec<(SessionId, OutboundWorldPacket)>> {
        let Some(target) = self
            .players
            .get(&bot_guid)
            .and_then(|player| player.active_combat_target)
        else {
            return Ok(Vec::new());
        };
        let bot_object = ObjectGuid::new(HighGuid::Player, 0, bot_guid);
        self.set_player_auto_attack(bot_guid, None, None);
        if let Some(bot) = self
            .players
            .get_mut(&bot_guid)
            .and_then(|player| player.bot_runtime.as_mut())
        {
            bot.engage_target = None;
            bot.route.clear();
        }
        let mut packets = self.update_player_selection(bot_guid, None)?;
        packets.extend(self.broadcast_nearby_player_packet(
            bot_guid,
            PLAYER_VISIBILITY_RADIUS_YARDS,
            OutboundWorldPacket {
                opcode: SMSG_ATTACKSTOP,
                body: build_attack_stop_body(bot_object, target, false)?,
            },
        ));
        packets.extend(self.broadcast_nearby_player_packet(
            bot_guid,
            PLAYER_VISIBILITY_RADIUS_YARDS,
            OutboundWorldPacket {
                opcode: SMSG_UPDATE_OBJECT,
                body: build_unit_flags_update_body(bot_object, player_unit_flags(false))?,
            },
        ));
        Ok(packets)
    }

    pub(in crate::world) fn face_playerbot_toward(&mut self, bot_guid: u32, target: WorldPosition) {
        if let Some(player) = self.players.get_mut(&bot_guid) {
            player.position = WorldPosition::new(
                player.position.map_id,
                player.position.x,
                player.position.y,
                player.position.z,
                playerbot_orientation_toward(player.position, target),
            );
        }
    }

    pub(in crate::world) fn broadcast_packet_near_position(
        &self,
        position: WorldPosition,
        radius: f32,
        exclude_guid: Option<u32>,
        packet: OutboundWorldPacket,
    ) -> Vec<(SessionId, OutboundWorldPacket)> {
        self.nearby_player_guids(position, radius, exclude_guid)
            .into_iter()
            .filter_map(|player_guid| {
                self.players
                    .get(&player_guid)
                    .and_then(|player| player.packet_to_client(packet.clone()))
            })
            .collect()
    }

    pub(in crate::world) fn playerbot_is_in_active_grid(&self, player: &PlayerRuntime) -> bool {
        if player
            .bot_runtime
            .as_ref()
            .is_some_and(|bot| bot.travel_destination.is_some())
        {
            return true;
        }
        let grid = grid_coord_for_position(player.position);
        self.grids.get(&grid).is_some_and(|grid| {
            grid.active_player_count > 0 && matches!(grid.state, GridState::Active)
        })
    }

    pub(in crate::world) fn prepare_playerbot_roam_movement(
        &mut self,
        bot_guid: u32,
        now: Instant,
    ) -> Option<PlayerbotMovementUpdate> {
        let geometry = self.geometry.clone();
        let needs_route_intent = self
            .players
            .get(&bot_guid)
            .and_then(|player| player.bot_runtime.as_ref())
            .is_some_and(|bot| {
                bot.active_leg.is_none() && bot.route.is_empty() && now >= bot.next_think_at
            });
        let movement_intent = needs_route_intent
            .then(|| self.take_playerbot_movement_intent(bot_guid))
            .flatten();
        let player = self.players.get_mut(&bot_guid)?;
        let bot = player.bot_runtime.as_mut()?;
        if let Some(leg) = bot.active_leg {
            let stop_position =
                playerbot_grounded_position(Some(&geometry), playerbot_position_on_leg(leg, now));
            bot.active_leg = None;
            bot.next_think_at = if bot.route.is_empty() {
                now + playerbot_next_roam_delay(bot_guid, bot.roam_step)
            } else {
                now
            };
            bot.movement_start_retries_remaining = 0;
            let server_time = player.server_time.wrapping_add(
                leg.arrival_time
                    .saturating_duration_since(leg.start_time)
                    .as_millis()
                    .min(u32::MAX as u128) as u32,
            );
            return Some(PlayerbotMovementUpdate {
                opcode: MSG_MOVE_STOP as u16,
                movement: MovementInfo {
                    flags: 0,
                    client_time: server_time,
                    position: stop_position,
                    fall_time: 0,
                    jump: JumpInfo::default(),
                },
            });
        }

        if now >= bot.next_think_at {
            if bot.route.is_empty() {
                let Some(movement_intent) = movement_intent else {
                    bot.next_think_at = now + playerbot_missing_intent_defer_delay(bot_guid);
                    crate::observability::record_playerbot_event("missing_movement_intent_defer");
                    return None;
                };
                let route = match movement_intent {
                    PlayerbotMovementIntent::Defer => {
                        bot.next_think_at = now + playerbot_route_plan_defer_delay(bot_guid);
                        crate::observability::record_playerbot_event("route_budget_defer");
                        return None;
                    }
                    PlayerbotMovementIntent::Route { route } => route,
                };
                let Some(route) = route else {
                    if bot.engage_target.is_some() {
                        bot.engage_target = None;
                        bot.route.clear();
                        bot.next_think_at =
                            now + playerbot_next_roam_delay(bot_guid, bot.roam_step);
                        bot.next_combat_think_at =
                            now + playerbot_failed_engage_backoff_delay(bot_guid);
                        crate::observability::record_playerbot_event("engage_route_failed_backoff");
                    } else {
                        bot.next_think_at = now + playerbot_failed_route_retry_delay(bot_guid);
                        crate::observability::record_playerbot_event("route_failed_retry");
                    }
                    return None;
                };
                if route.is_empty() {
                    bot.next_think_at = if bot.travel_destination.is_some() {
                        now + playerbot_travel_arrived_recheck_delay(bot_guid)
                    } else {
                        now + playerbot_next_roam_delay(bot_guid, bot.roam_step)
                    };
                    return None;
                }
                bot.route = route;
            }
            let next_destination = bot.route.remove(0);
            let leg = playerbot_movement_leg(player.position, next_destination, now);
            bot.active_leg = Some(leg);
            bot.movement_start_retries_remaining = 0;
            if bot.travel_destination.is_none() {
                bot.roam_step = bot.roam_step.wrapping_add(1) % PLAYERBOT_ROAM_STEPS;
            }
            let server_time = player.server_time;
            return Some(PlayerbotMovementUpdate {
                opcode: MSG_MOVE_START_FORWARD as u16,
                movement: MovementInfo {
                    flags: MOVEFLAG_FORWARD,
                    client_time: server_time,
                    position: WorldPosition::new(
                        player.position.map_id,
                        player.position.x,
                        player.position.y,
                        player.position.z,
                        playerbot_orientation_toward(player.position, leg.destination),
                    ),
                    fall_time: 0,
                    jump: JumpInfo::default(),
                },
            });
        }

        None
    }

    pub(in crate::world) fn commit_unobserved_playerbot_movement(
        &mut self,
        bot_guid: u32,
        movement: &MovementInfo,
        server_time: u32,
    ) {
        let Some(current_player) = self.players.get(&bot_guid) else {
            return;
        };
        debug_assert!(matches!(
            current_player.controller,
            PlayerController::Bot { .. }
        ));
        let old_cell = current_player.cell;
        let old_grid = grid_coord_for_position(current_player.position);
        let new_cell = cell_coord_for_position(movement.position);
        let new_grid = grid_coord_for_position(movement.position);

        if old_grid != new_grid || old_cell != new_cell {
            if let Some(grid) = self.grids.get_mut(&old_grid) {
                if let Some(cell) = grid.cells.get_mut(&old_cell) {
                    cell.players.remove(&bot_guid);
                }
                grid.last_touched = Instant::now();
            }
            let grid = self.grids.entry(new_grid).or_default();
            grid.last_touched = Instant::now();
            grid.cells
                .entry(new_cell)
                .or_default()
                .players
                .insert(bot_guid);
            if old_grid != new_grid {
                self.refresh_grid_state(old_grid);
            }
            self.refresh_grid_state(new_grid);
        }

        if let Some(player) = self.players.get_mut(&bot_guid) {
            player.position = movement.position;
            player.movement_flags = movement.flags;
            player.client_time = movement.client_time;
            player.server_time = server_time;
            player.fall_time = movement.fall_time;
            player.jump = movement.jump.clone();
            player.cell = new_cell;
        }
        self.invalidate_idle_motion_start_schedule();
    }
}

pub(in crate::world) const PLAYERBOT_ROAM_STEPS: u8 = u8::MAX;

pub(in crate::world) fn plan_playerbot_intents(
    mut inputs: Vec<PlayerbotPlanInput>,
    faction_templates: &FactionTemplateStore,
    navigation: &DbCreatureNavigationGuardrail,
    budget: &mut PlayerbotPlannerBudget,
) -> Vec<((u32, u32), u32, PlayerbotQueuedIntents)> {
    inputs.sort_by_key(|input| {
        (
            input
                .movement_due_at
                .or(input.combat_due_at)
                .unwrap_or_else(Instant::now),
            input.bot_guid,
        )
    });

    let mut planned = Vec::new();
    for input in inputs {
        let mut intents = PlayerbotQueuedIntents::default();
        if input.movement_due_at.is_some() {
            if playerbot_movement_route_uses_budget(&input) && budget.route_plans_remaining == 0 {
                budget.route_budget_exhausted = true;
                intents.movement = Some(PlayerbotMovementIntent::Defer);
            } else {
                if playerbot_movement_route_uses_budget(&input) {
                    budget.route_plans_remaining -= 1;
                }
                intents.movement = Some(PlayerbotMovementIntent::Route {
                    route: plan_playerbot_movement_route(&input, navigation),
                });
            }
        }

        if input.combat_due_at.is_some() {
            if budget.combat_thinks_remaining == 0 {
                budget.combat_budget_exhausted = true;
            } else {
                budget.combat_thinks_remaining -= 1;
                intents.combat = Some(plan_playerbot_combat_intent(
                    &input,
                    faction_templates,
                    navigation,
                    budget,
                ));
            }
        }

        if !intents.is_empty() {
            planned.push(((input.map_id, input.instance_id), input.bot_guid, intents));
        }
    }
    planned
}

pub(in crate::world) fn playerbot_movement_route_uses_budget(input: &PlayerbotPlanInput) -> bool {
    input.travel_destination.is_some() || input.engage_target_creature.is_some()
}

pub(in crate::world) fn plan_playerbot_movement_route(
    input: &PlayerbotPlanInput,
    navigation: &DbCreatureNavigationGuardrail,
) -> Option<Vec<WorldPosition>> {
    let geometry = Some(input.geometry.as_ref());
    if let Some(creature) = input
        .engage_target_creature
        .as_ref()
        .filter(|creature| creature.is_alive() && !creature.is_evading_home())
    {
        return playerbot_route_points_to_target(
            navigation,
            geometry,
            input.position,
            creature.current_position,
            false,
        );
    }
    if input.travel_destination.is_none() {
        return playerbot_roam_route_points(
            geometry,
            input.position,
            input.home_position,
            input.roam_step,
            input.bot_guid,
        );
    }
    playerbot_route_points(
        navigation,
        geometry,
        input.position,
        input.travel_destination,
        input.home_position,
        input.roam_step,
        input.bot_guid,
    )
}

pub(in crate::world) fn playerbot_roam_route_points(
    geometry: Option<&WorldGeometry>,
    start: WorldPosition,
    home_position: WorldPosition,
    roam_step: u8,
    bot_guid: u32,
) -> Option<Vec<WorldPosition>> {
    let target = playerbot_route_target(bot_guid, None, home_position, roam_step);
    let target = playerbot_grounded_position(geometry, target);
    if start.distance_2d(&target) <= PLAYERBOT_DESTINATION_EPSILON_YARDS {
        return Some(Vec::new());
    }
    Some(playerbot_local_grounded_route_points(
        geometry, start, target,
    ))
}

pub(in crate::world) fn playerbot_local_grounded_route_points(
    geometry: Option<&WorldGeometry>,
    start: WorldPosition,
    target: WorldPosition,
) -> Vec<WorldPosition> {
    let distance = start.distance_2d(&target);
    let steps = (distance / PLAYERBOT_ROUTE_MIN_SEGMENT_YARDS)
        .ceil()
        .max(1.0) as u32;
    let mut points = Vec::new();
    for step in 1..=steps {
        let ratio = step as f32 / steps as f32;
        let candidate = WorldPosition::new(
            start.map_id,
            start.x + (target.x - start.x) * ratio,
            start.y + (target.y - start.y) * ratio,
            start.z + (target.z - start.z) * ratio,
            playerbot_orientation_toward(start, target),
        );
        push_playerbot_route_point(
            &mut points,
            playerbot_grounded_position(geometry, candidate),
        );
    }
    points
}

pub(in crate::world) fn plan_playerbot_combat_intent(
    input: &PlayerbotPlanInput,
    faction_templates: &FactionTemplateStore,
    navigation: &DbCreatureNavigationGuardrail,
    budget: &mut PlayerbotPlannerBudget,
) -> PlayerbotCombatIntent {
    if let Some(creature) = input.engage_target_creature.as_ref() {
        if playerbot_can_attack_db_creature(faction_templates, creature, input.player_race) {
            return PlayerbotCombatIntent::Target {
                target: creature.guid(),
                route: plan_playerbot_combat_route(input, creature, navigation, budget),
            };
        }
    }

    for creature in &input.nearby_creatures {
        if !playerbot_can_attack_db_creature(faction_templates, creature, input.player_race) {
            continue;
        }
        return PlayerbotCombatIntent::Target {
            target: creature.guid(),
            route: plan_playerbot_combat_route(input, creature, navigation, budget),
        };
    }
    PlayerbotCombatIntent::NoTarget
}

pub(in crate::world) fn plan_playerbot_combat_route(
    input: &PlayerbotPlanInput,
    creature: &DbCreatureRuntime,
    navigation: &DbCreatureNavigationGuardrail,
    budget: &mut PlayerbotPlannerBudget,
) -> Option<Vec<WorldPosition>> {
    if playerbot_can_melee_db_creature_from_position(input.position, creature, navigation) {
        return Some(Vec::new());
    }
    if budget.route_plans_remaining == 0 {
        budget.route_budget_exhausted = true;
        return None;
    }
    budget.route_plans_remaining -= 1;
    playerbot_route_points_to_target(
        navigation,
        Some(input.geometry.as_ref()),
        input.position,
        creature.current_position,
        false,
    )
}

pub(in crate::world) fn playerbot_route_points(
    navigation: &DbCreatureNavigationGuardrail,
    geometry: Option<&WorldGeometry>,
    start: WorldPosition,
    travel_destination: Option<WorldPosition>,
    home_position: WorldPosition,
    roam_step: u8,
    bot_guid: u32,
) -> Option<Vec<WorldPosition>> {
    let target = playerbot_route_target(bot_guid, travel_destination, home_position, roam_step);
    playerbot_route_points_to_target(
        navigation,
        geometry,
        start,
        target,
        travel_destination.is_some(),
    )
}

pub(in crate::world) fn playerbot_route_points_to_target(
    navigation: &DbCreatureNavigationGuardrail,
    geometry: Option<&WorldGeometry>,
    start: WorldPosition,
    target: WorldPosition,
    allow_travel_partials: bool,
) -> Option<Vec<WorldPosition>> {
    let target = geometry
        .and_then(|geometry| geometry.ground_position(target))
        .unwrap_or(target);
    if start.distance_2d(&target) <= PLAYERBOT_DESTINATION_EPSILON_YARDS {
        return Some(Vec::new());
    }
    let route_start = playerbot_grounded_route_start(geometry, start);
    let mut points = playerbot_route_points_from_anchor(
        navigation,
        geometry,
        route_start,
        target,
        allow_travel_partials,
    )
    .or_else(|| {
        allow_travel_partials
            .then(|| playerbot_reanchored_route_points(navigation, geometry, start, target))
            .flatten()
    })?;
    playerbot_prefix_route_anchor(start, route_start, &mut points);
    (!points.is_empty()).then_some(points)
}

pub(in crate::world) fn playerbot_grounded_route_start(
    geometry: Option<&WorldGeometry>,
    start: WorldPosition,
) -> WorldPosition {
    playerbot_grounded_position(geometry, start)
}

pub(in crate::world) fn playerbot_grounded_position(
    geometry: Option<&WorldGeometry>,
    position: WorldPosition,
) -> WorldPosition {
    geometry
        .and_then(|geometry| geometry.ground_position(position))
        .unwrap_or(position)
}

pub(in crate::world) fn playerbot_route_points_from_anchor(
    navigation: &DbCreatureNavigationGuardrail,
    geometry: Option<&WorldGeometry>,
    anchor: WorldPosition,
    target: WorldPosition,
    allow_travel_partials: bool,
) -> Option<Vec<WorldPosition>> {
    let path =
        playerbot_path_to_destination(navigation, geometry, anchor, target, CreaturePathMode::Full)
            .or_else(|| {
                allow_travel_partials
                    .then(|| playerbot_partial_travel_path(navigation, geometry, anchor, target))
                    .flatten()
            })?;
    let points = playerbot_compact_route_points(anchor, path.points);
    if allow_travel_partials && !playerbot_route_makes_progress(anchor, target, &points) {
        return None;
    }
    (!points.is_empty()).then_some(points)
}

pub(in crate::world) fn playerbot_path_to_destination(
    navigation: &DbCreatureNavigationGuardrail,
    geometry: Option<&WorldGeometry>,
    start: WorldPosition,
    target_position: WorldPosition,
    mode: CreaturePathMode,
) -> Option<DbCreaturePath> {
    let target_position = db_creature_ground_destination(geometry, target_position)?;
    match playerbot_mmap_path(navigation, start, target_position, mode) {
        DbCreaturePathBuild::Path(path) => Some(path),
        DbCreaturePathBuild::NoPath(_) => None,
        DbCreaturePathBuild::Unavailable => {
            if db_creature_uses_unit_fixture_pathing(navigation) {
                db_creature_straight_path(start, target_position, mode).map(|points| {
                    DbCreaturePath {
                        flags: DbCreaturePathFlags(
                            DbCreaturePathFlags::NORMAL.0 | DbCreaturePathFlags::NOT_USING_PATH.0,
                        ),
                        points,
                    }
                })
            } else {
                None
            }
        }
    }
}

pub(in crate::world) fn playerbot_mmap_path(
    navigation: &DbCreatureNavigationGuardrail,
    start: WorldPosition,
    target_position: WorldPosition,
    mode: CreaturePathMode,
) -> DbCreaturePathBuild {
    let Some(data_dir) = navigation.world_data_files.data_dir_for_native.as_ref() else {
        return DbCreaturePathBuild::Unavailable;
    };
    let Some((start_tile_x, start_tile_y)) = mmap_tile_for_position(start) else {
        return DbCreaturePathBuild::Unavailable;
    };
    let Some((target_tile_x, target_tile_y)) = mmap_tile_for_position(target_position) else {
        return DbCreaturePathBuild::Unavailable;
    };
    if !navigation
        .world_data_files
        .has_mmap_support_for_map(start.map_id)
        || !navigation
            .world_data_files
            .has_mmap_tile(start.map_id, start_tile_x, start_tile_y)
        || !navigation
            .world_data_files
            .has_mmap_tile(start.map_id, target_tile_x, target_tile_y)
    {
        return DbCreaturePathBuild::Unavailable;
    }

    let native_path = native_mmap_find_path(
        data_dir,
        start,
        target_position,
        (start_tile_x, start_tile_y),
        (target_tile_x, target_tile_y),
        NativeMmapPathFilter {
            include_flags: NativeMmapPathFilter::NAV_GROUND,
            exclude_flags: 0,
        },
    );
    let flags = match native_path.status {
        NativeMmapPathStatus::Normal => DbCreaturePathFlags::NORMAL,
        NativeMmapPathStatus::Incomplete => DbCreaturePathFlags::INCOMPLETE,
        NativeMmapPathStatus::NoPath
        | NativeMmapPathStatus::Unavailable
        | NativeMmapPathStatus::InvalidInput
        | NativeMmapPathStatus::NativeError => {
            return DbCreaturePathBuild::NoPath(DbCreaturePathFlags::NOPATH);
        }
    };
    if native_path.points.len() < 2 {
        return DbCreaturePathBuild::NoPath(DbCreaturePathFlags::NOPATH);
    }
    let Some(path) = native_mmap_points_to_world_path(start, &native_path.points) else {
        return DbCreaturePathBuild::NoPath(DbCreaturePathFlags::NOPATH);
    };
    match db_creature_trim_path_for_mode(start, path, mode) {
        Some(points) => DbCreaturePathBuild::Path(DbCreaturePath { flags, points }),
        None => DbCreaturePathBuild::NoPath(DbCreaturePathFlags::NOPATH),
    }
}

pub(in crate::world) fn playerbot_partial_travel_path(
    navigation: &DbCreatureNavigationGuardrail,
    geometry: Option<&WorldGeometry>,
    anchor: WorldPosition,
    target: WorldPosition,
) -> Option<DbCreaturePath> {
    for leg_yards in PLAYERBOT_TRAVEL_ROUTE_LEG_CANDIDATES_YARDS {
        let partial_target = playerbot_partial_travel_target(geometry, anchor, target, leg_yards)?;
        if let Some(path) = playerbot_path_to_destination(
            navigation,
            geometry,
            anchor,
            partial_target,
            CreaturePathMode::Full,
        ) {
            return Some(path);
        }
    }
    None
}

pub(in crate::world) fn playerbot_reanchored_route_points(
    navigation: &DbCreatureNavigationGuardrail,
    geometry: Option<&WorldGeometry>,
    start: WorldPosition,
    target: WorldPosition,
) -> Option<Vec<WorldPosition>> {
    for anchor in playerbot_reanchor_candidates(geometry, start, target) {
        let partial_target = playerbot_partial_travel_target(
            geometry,
            anchor,
            target,
            PLAYERBOT_TRAVEL_ROUTE_LEG_CANDIDATES_YARDS
                .last()
                .copied()
                .unwrap_or(PLAYERBOT_TRAVEL_ROUTE_LEG_YARDS),
        )?;
        let Some(path) = playerbot_path_to_destination(
            navigation,
            geometry,
            anchor,
            partial_target,
            CreaturePathMode::Full,
        ) else {
            continue;
        };
        let mut points = playerbot_compact_route_points(anchor, path.points);
        playerbot_prefix_route_anchor(start, anchor, &mut points);
        if !playerbot_route_makes_progress(start, target, &points) {
            continue;
        }
        if !points.is_empty() {
            return Some(points);
        }
    }
    None
}

pub(in crate::world) fn playerbot_reanchor_candidates(
    geometry: Option<&WorldGeometry>,
    start: WorldPosition,
    target: WorldPosition,
) -> Vec<WorldPosition> {
    let facing = playerbot_orientation_toward(start, target);
    [
        facing + std::f32::consts::FRAC_PI_2,
        facing - std::f32::consts::FRAC_PI_2,
        facing,
        facing + std::f32::consts::PI,
    ]
    .into_iter()
    .filter_map(|angle| {
        let candidate = WorldPosition::new(
            start.map_id,
            start.x + PLAYERBOT_ROUTE_REANCHOR_YARDS * angle.cos(),
            start.y + PLAYERBOT_ROUTE_REANCHOR_YARDS * angle.sin(),
            start.z,
            normalize_orientation(angle),
        );
        let grounded = playerbot_grounded_route_start(geometry, candidate);
        (start.distance_2d(&grounded) > PLAYERBOT_DESTINATION_EPSILON_YARDS).then_some(grounded)
    })
    .collect()
}

pub(in crate::world) fn playerbot_prefix_route_anchor(
    start: WorldPosition,
    anchor: WorldPosition,
    points: &mut Vec<WorldPosition>,
) {
    if start.distance_2d(&anchor) <= PLAYERBOT_DESTINATION_EPSILON_YARDS {
        return;
    }
    points.insert(0, anchor);
}

pub(in crate::world) fn playerbot_route_makes_progress(
    start: WorldPosition,
    target: WorldPosition,
    points: &[WorldPosition],
) -> bool {
    let Some(last) = points.last() else {
        return false;
    };
    last.distance_2d(&target) + PLAYERBOT_DESTINATION_EPSILON_YARDS < start.distance_2d(&target)
}

pub(in crate::world) fn playerbot_route_target(
    bot_guid: u32,
    travel_destination: Option<WorldPosition>,
    home_position: WorldPosition,
    roam_step: u8,
) -> WorldPosition {
    match travel_destination {
        Some(destination) => destination,
        None => playerbot_roam_destination(home_position, bot_guid, roam_step),
    }
}

pub(in crate::world) fn playerbot_movement_due_at(bot: &PlayerbotRuntimeState) -> Instant {
    bot.active_leg
        .map(|leg| leg.arrival_time)
        .unwrap_or(bot.next_think_at)
}

pub(in crate::world) fn playerbot_combat_due_at(
    player: &PlayerRuntime,
    bot: &PlayerbotRuntimeState,
    now: Instant,
) -> Instant {
    if player.active_combat_target.is_some() {
        return player.active_combat_next_swing_at.unwrap_or(now);
    }
    bot.next_combat_think_at
}

pub(in crate::world) fn playerbot_next_roam_delay(bot_guid: u32, roam_step: u8) -> Duration {
    let stagger_window_ms = PLAYERBOT_ROAM_THINK_INTERVAL.as_millis() as u64;
    let stagger_ms = (u64::from(bot_guid).wrapping_mul(37)
        + u64::from(roam_step).wrapping_mul(149))
        % stagger_window_ms;
    PLAYERBOT_ROAM_THINK_INTERVAL + Duration::from_millis(stagger_ms)
}

pub(in crate::world) fn playerbot_next_combat_think_delay(bot_guid: u32) -> Duration {
    let stagger_window_ms = PLAYERBOT_COMBAT_THINK_INTERVAL.as_millis() as u64;
    let stagger_ms = u64::from(bot_guid).wrapping_mul(43) % stagger_window_ms;
    PLAYERBOT_COMBAT_THINK_INTERVAL + Duration::from_millis(stagger_ms)
}

pub(in crate::world) fn playerbot_route_plan_defer_delay(bot_guid: u32) -> Duration {
    Duration::from_millis(
        PLAYERBOT_ROUTE_PLAN_DEFER_BASE_MILLIS
            + (u64::from(bot_guid).wrapping_mul(17)
                % PLAYERBOT_ROAM_THINK_INTERVAL.as_millis() as u64),
    )
}

pub(in crate::world) fn playerbot_missing_intent_defer_delay(bot_guid: u32) -> Duration {
    Duration::from_millis(
        PLAYERBOT_MISSING_INTENT_DEFER_BASE_MILLIS
            + (u64::from(bot_guid).wrapping_mul(23)
                % PLAYERBOT_ROAM_THINK_INTERVAL.as_millis() as u64),
    )
}

pub(in crate::world) fn playerbot_failed_route_retry_delay(bot_guid: u32) -> Duration {
    Duration::from_millis(
        PLAYERBOT_ROUTE_PLAN_FAILED_RETRY_MILLIS
            + (u64::from(bot_guid).wrapping_mul(37)
                % PLAYERBOT_ROAM_THINK_INTERVAL.as_millis() as u64),
    )
}

pub(in crate::world) fn playerbot_failed_engage_backoff_delay(bot_guid: u32) -> Duration {
    Duration::from_millis(
        PLAYERBOT_ENGAGE_FAILED_BACKOFF_MILLIS
            + (u64::from(bot_guid).wrapping_mul(41)
                % PLAYERBOT_ROAM_THINK_INTERVAL.as_millis() as u64),
    )
}

pub(in crate::world) fn playerbot_travel_arrived_recheck_delay(bot_guid: u32) -> Duration {
    PLAYERBOT_TRAVEL_ARRIVED_RECHECK_INTERVAL
        + Duration::from_millis(
            u64::from(bot_guid).wrapping_mul(53) % PLAYERBOT_ROAM_THINK_INTERVAL.as_millis() as u64,
        )
}

pub(in crate::world) fn playerbot_partial_travel_target(
    geometry: Option<&WorldGeometry>,
    start: WorldPosition,
    target: WorldPosition,
    leg_yards: f32,
) -> Option<WorldPosition> {
    if start.map_id != target.map_id {
        return None;
    }
    let dx = target.x - start.x;
    let dy = target.y - start.y;
    let distance = (dx.mul_add(dx, dy * dy)).sqrt();
    if distance <= PLAYERBOT_DESTINATION_EPSILON_YARDS {
        return None;
    }
    let travel = leg_yards
        .clamp(
            PLAYERBOT_DESTINATION_EPSILON_YARDS,
            PLAYERBOT_TRAVEL_ROUTE_LEG_YARDS,
        )
        .min(distance);
    let ratio = travel / distance;
    let partial_target = WorldPosition::new(
        start.map_id,
        start.x + dx * ratio,
        start.y + dy * ratio,
        target.z,
        playerbot_orientation_toward(start, target),
    );
    Some(
        geometry
            .and_then(|geometry| geometry.ground_position(partial_target))
            .unwrap_or(partial_target),
    )
}

pub(in crate::world) fn playerbot_compact_route_points(
    start: WorldPosition,
    points: Vec<WorldPosition>,
) -> Vec<WorldPosition> {
    let Some(mut previous) = points.first().copied() else {
        return Vec::new();
    };
    let Some(final_point) = points.last().copied() else {
        return Vec::new();
    };

    let mut compact = Vec::new();
    let mut anchor = start;
    for point in points.into_iter().skip(1) {
        if playerbot_turn_angle(anchor, previous, point) >= PLAYERBOT_ROUTE_TURN_ANGLE_RADIANS
            && anchor.distance_2d(&previous) > PLAYERBOT_DESTINATION_EPSILON_YARDS
        {
            push_playerbot_route_point(&mut compact, previous);
            anchor = previous;
        }

        if anchor.distance_2d(&point) >= PLAYERBOT_ROUTE_MIN_SEGMENT_YARDS {
            push_playerbot_route_point(&mut compact, point);
            anchor = point;
        }
        previous = point;
    }
    push_playerbot_route_point(&mut compact, final_point);
    compact
}

pub(in crate::world) fn push_playerbot_route_point(
    points: &mut Vec<WorldPosition>,
    point: WorldPosition,
) {
    let should_push = match points.last() {
        Some(last) => last.distance_2d(&point) > PLAYERBOT_DESTINATION_EPSILON_YARDS,
        None => true,
    };
    if should_push {
        points.push(point);
    }
}

pub(in crate::world) fn playerbot_turn_angle(
    anchor: WorldPosition,
    corner: WorldPosition,
    next: WorldPosition,
) -> f32 {
    let ax = corner.x - anchor.x;
    let ay = corner.y - anchor.y;
    let bx = next.x - corner.x;
    let by = next.y - corner.y;
    let a_len = (ax.mul_add(ax, ay * ay)).sqrt();
    let b_len = (bx.mul_add(bx, by * by)).sqrt();
    if a_len <= PLAYERBOT_DESTINATION_EPSILON_YARDS || b_len <= PLAYERBOT_DESTINATION_EPSILON_YARDS
    {
        return 0.0;
    }
    let dot = (ax * bx + ay * by) / (a_len * b_len);
    dot.clamp(-1.0, 1.0).acos()
}

pub(in crate::world) fn playerbot_roam_destination(
    home: WorldPosition,
    bot_guid: u32,
    step: u8,
) -> WorldPosition {
    let angle_seed = playerbot_hash64(u64::from(bot_guid) << 32 | u64::from(step));
    let radius_seed = playerbot_hash64(angle_seed ^ 0x9E37_79B9_7F4A_7C15);
    let angle = playerbot_unit_float(angle_seed) * std::f32::consts::TAU;
    let radius = PLAYERBOT_WANDER_MIN_RADIUS_YARDS
        + playerbot_unit_float(radius_seed)
            * (PLAYERBOT_WANDER_MAX_RADIUS_YARDS - PLAYERBOT_WANDER_MIN_RADIUS_YARDS);
    WorldPosition::new(
        home.map_id,
        home.x + radius * angle.cos(),
        home.y + radius * angle.sin(),
        home.z,
        angle,
    )
}

pub(in crate::world) fn playerbot_hash64(mut value: u64) -> u64 {
    value = value.wrapping_add(0x9E37_79B9_7F4A_7C15);
    value = (value ^ (value >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    value = (value ^ (value >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    value ^ (value >> 31)
}

pub(in crate::world) fn playerbot_unit_float(value: u64) -> f32 {
    ((value >> 40) as f32) / ((1u64 << 24) as f32)
}

pub(in crate::world) fn playerbot_movement_leg(
    start_position: WorldPosition,
    destination: WorldPosition,
    start_time: Instant,
) -> PlayerbotMovementLeg {
    let distance = start_position.distance_2d(&destination);
    let travel_time = if distance <= PLAYERBOT_DESTINATION_EPSILON_YARDS {
        Duration::ZERO
    } else {
        Duration::from_secs_f32(distance / PLAYERBOT_RUN_SPEED_YARDS_PER_SECOND)
    };
    PlayerbotMovementLeg {
        start_position,
        destination,
        start_time,
        arrival_time: start_time + travel_time,
        speed_yards_per_second: PLAYERBOT_RUN_SPEED_YARDS_PER_SECOND,
    }
}

pub(in crate::world) fn playerbot_position_on_leg(
    leg: PlayerbotMovementLeg,
    now: Instant,
) -> WorldPosition {
    if now >= leg.arrival_time {
        return WorldPosition::new(
            leg.destination.map_id,
            leg.destination.x,
            leg.destination.y,
            leg.destination.z,
            playerbot_orientation_toward(leg.start_position, leg.destination),
        );
    }
    let elapsed = now.saturating_duration_since(leg.start_time).as_secs_f32();
    let max_distance = leg.speed_yards_per_second * elapsed;
    playerbot_step_toward(leg.start_position, leg.destination, max_distance).0
}

pub(in crate::world) fn playerbot_orientation_toward(
    start: WorldPosition,
    destination: WorldPosition,
) -> f32 {
    let dx = destination.x - start.x;
    let dy = destination.y - start.y;
    if dx.abs() <= f32::EPSILON && dy.abs() <= f32::EPSILON {
        return start.orientation;
    }
    normalize_orientation(dy.atan2(dx))
}

pub(in crate::world) fn playerbot_step_toward(
    start: WorldPosition,
    destination: WorldPosition,
    max_distance: f32,
) -> (WorldPosition, bool) {
    if start.map_id != destination.map_id {
        return (start, true);
    }
    let dx = destination.x - start.x;
    let dy = destination.y - start.y;
    let dz = destination.z - start.z;
    let distance = (dx.mul_add(dx, dy * dy)).sqrt();
    if distance <= PLAYERBOT_DESTINATION_EPSILON_YARDS || distance <= max_distance.max(0.0) {
        return (
            WorldPosition::new(
                destination.map_id,
                destination.x,
                destination.y,
                destination.z,
                playerbot_orientation_toward(start, destination),
            ),
            true,
        );
    }

    let ratio = max_distance.max(0.0) / distance;
    let next = WorldPosition::new(
        start.map_id,
        start.x + dx * ratio,
        start.y + dy * ratio,
        start.z + dz * ratio,
        playerbot_orientation_toward(start, destination),
    );
    (next, false)
}

pub(in crate::world) fn playerbot_can_attack_db_creature(
    faction_templates: &FactionTemplateStore,
    creature: &DbCreatureRuntime,
    player_race: u8,
) -> bool {
    let is_critter = creature.spawn.template.creature_type == CREATURE_TYPE_CRITTER;
    creature.is_alive()
        && !creature.is_evading_home()
        && (creature.spawn.template.civilian == 0 || is_critter)
        && creature.spawn.template.npc_flags == 0
        && (is_critter
            || faction_reaction_to(
                faction_templates,
                creature.spawn.template.faction,
                faction_for_race(player_race),
            ) != FactionReaction::Friendly)
}

pub(in crate::world) fn playerbot_creature_can_reach_player(
    creature: &DbCreatureRuntime,
    player: &PlayerRuntime,
    navigation: &DbCreatureNavigationGuardrail,
) -> bool {
    if creature.current_position.map_id != player.position.map_id {
        return false;
    }
    let reach = combined_melee_reach(creature.combat_reach(), PLAYER_COMBAT_REACH_YARDS);
    let dx = creature.current_position.x - player.position.x;
    let dy = creature.current_position.y - player.position.y;
    let dz = creature.current_position.z - player.position.z;
    if dx * dx + dy * dy + dz * dz > reach * reach {
        return false;
    }
    db_creature_navigation_check(navigation, creature.current_position, player.position).is_clear()
}

pub(in crate::world) fn playerbot_can_melee_db_creature_from_position(
    position: WorldPosition,
    creature: &DbCreatureRuntime,
    navigation: &DbCreatureNavigationGuardrail,
) -> bool {
    if !creature.is_alive() || creature.is_evading_home() {
        return false;
    }
    let reach = combined_melee_reach(PLAYER_COMBAT_REACH_YARDS, creature.combat_reach());
    let dx = position.x - creature.current_position.x;
    let dy = position.y - creature.current_position.y;
    let dz = position.z - creature.current_position.z;
    if dx * dx + dy * dy + dz * dz > reach * reach {
        return false;
    }
    db_creature_navigation_check(navigation, position, creature.current_position).is_clear()
}
