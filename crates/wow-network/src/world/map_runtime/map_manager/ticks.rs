use super::*;

impl MapRuntimeManager {
    pub(in crate::world) async fn advance_all_player_regen_ticks(
        &self,
        now: Instant,
    ) -> anyhow::Result<Vec<(SessionId, OutboundWorldPacket)>> {
        let maps = { self.maps.lock().await.values().cloned().collect::<Vec<_>>() };
        let mut packets = Vec::new();
        for map in maps {
            packets.extend(map.lock().await.advance_player_regen_tick(now)?);
        }
        Ok(packets)
    }

    pub(in crate::world) async fn advance_all_db_creature_lifecycle_ticks(
        &self,
        now: Instant,
    ) -> anyhow::Result<DbCreatureLifecycleTick> {
        let maps = { self.maps.lock().await.values().cloned().collect::<Vec<_>>() };
        let mut aggregate = DbCreatureLifecycleTick::default();
        for map in maps {
            let tick = map.lock().await.advance_db_creature_lifecycle_tick(now)?;
            aggregate.packets.extend(tick.packets);
            aggregate.respawn_updates.extend(tick.respawn_updates);
        }
        Ok(aggregate)
    }

    pub(in crate::world) async fn advance_all_player_visibility_refreshes(
        &self,
    ) -> anyhow::Result<PlayerVisibilityRefreshTick> {
        let maps = { self.maps.lock().await.values().cloned().collect::<Vec<_>>() };
        let mut aggregate = PlayerVisibilityRefreshTick::default();
        for map in maps {
            let tick = map.lock().await.advance_player_visibility_refresh_tick()?;
            aggregate.packets.extend(tick.packets);
            aggregate.refreshed_players = aggregate
                .refreshed_players
                .saturating_add(tick.refreshed_players);
            aggregate.budget_exhausted |= tick.budget_exhausted;
        }
        Ok(aggregate)
    }

    pub(in crate::world) async fn advance_all_db_creature_ooc_event_ai_spell_ticks(
        &self,
        world_db_pool: &MySqlPool,
        object_mgr: &ObjectMgr,
        navigation: &DbCreatureNavigationGuardrail,
        now: Instant,
        diff: Duration,
    ) -> anyhow::Result<DbCreatureOocEventAiTick> {
        let maps = { self.maps.lock().await.values().cloned().collect::<Vec<_>>() };
        let mut aggregate = DbCreatureOocEventAiTick::default();
        for map in maps {
            aggregate.packets.extend(
                self.advance_map_db_creature_ooc_event_ai_spell_tick(
                    map,
                    world_db_pool,
                    object_mgr,
                    navigation,
                    now,
                    diff,
                )
                .await?,
            );
        }
        Ok(aggregate)
    }

    async fn advance_map_db_creature_ooc_event_ai_spell_tick(
        &self,
        map: Arc<Mutex<MapRuntime>>,
        world_db_pool: &MySqlPool,
        object_mgr: &ObjectMgr,
        navigation: &DbCreatureNavigationGuardrail,
        now: Instant,
        diff: Duration,
    ) -> anyhow::Result<Vec<(SessionId, OutboundWorldPacket)>> {
        let mut packets = Vec::new();
        let candidates = { map.lock().await.db_creature_ooc_event_ai_candidate_guids() };
        if candidates.is_empty() {
            return Ok(packets);
        }

        let unknown_entries = {
            map.lock()
                .await
                .db_creature_ooc_event_ai_unknown_entries(&candidates)
        };
        if !unknown_entries.is_empty() {
            let mut classified = Vec::with_capacity(unknown_entries.len());
            for entry in unknown_entries {
                let scripts = object_mgr.creature_ai_scripts(world_db_pool, entry).await?;
                let scripts = scripts
                    .into_iter()
                    .filter(|script| {
                        matches!(
                            script.event_type,
                            EVENT_AI_EVENT_TIMER_OOC | EVENT_AI_EVENT_SPAWNED
                        ) && db_creature_event_ai_actions(script).iter().any(|action| {
                            action.action_type == EVENT_AI_ACTION_CAST && action.param1 > 0
                        })
                    })
                    .collect::<Vec<_>>();
                let capability = if scripts.is_empty() {
                    DbCreatureOocEventAiCapability::None
                } else {
                    DbCreatureOocEventAiCapability::OocCast(Arc::from(scripts))
                };
                classified.push((entry, capability));
            }

            let mut map_guard = map.lock().await;
            for (entry, capability) in classified {
                map_guard.set_db_creature_ooc_event_ai_capability(entry, capability);
            }
        }

        for (guid, _) in candidates {
            let action = map
                .lock()
                .await
                .prepare_ready_db_creature_ooc_event_ai_action(guid, now, diff);
            let Some(action) = action else {
                continue;
            };
            match action {
                ReadyDbCreatureOocEventAiAction::Complete { attacker, victim } => {
                    let mut map_guard = map.lock().await;
                    let Some(event) = map_guard
                        .complete_ready_db_creature_spell_cast_with_navigation(
                            attacker, victim, now, navigation,
                        )?
                    else {
                        continue;
                    };
                    packets.extend(
                        map_guard.materialize_db_creature_completed_spell_cast_packets(
                            attacker, victim, event,
                        ),
                    );
                }
                ReadyDbCreatureOocEventAiAction::Start { attacker, ready } => {
                    let Some(template) = object_mgr
                        .spell_template(world_db_pool, ready.spell_id)
                        .await?
                    else {
                        continue;
                    };
                    let spell_range = self.spell_range(template.range_index);
                    let spell_duration = self.spell_duration(template.duration_index);
                    let spell_cast_time = self.spell_cast_time(template.casting_time_index);
                    let spell_info = SpellInfo::from_template(&template);
                    let mut map_guard = map.lock().await;
                    if ready.target != attacker
                        && map_guard
                            .validate_db_creature_spell_against_target(
                                attacker,
                                ready.target,
                                navigation,
                                spell_range,
                                spell_info.requires_behind_target(),
                            )
                            .check
                            != DbCreatureSpellTargetCheck::Clear
                    {
                        continue;
                    }
                    let Some(cast) = map_guard.prepare_db_creature_spell_cast_from_template(
                        attacker,
                        ready.target,
                        &template,
                        spell_duration,
                        spell_range,
                        spell_cast_time,
                        now,
                    ) else {
                        continue;
                    };
                    let cast_time_millis = cast.cast_time_millis;
                    let target = cast.target;
                    let Some(start_packets) = map_guard.start_db_creature_spell_cast(cast)? else {
                        continue;
                    };
                    map_guard.apply_db_creature_event_ai_spell_cooldown(attacker, &ready, now);
                    packets.extend(start_packets);
                    if cast_time_millis == 0 {
                        if let Some(event) = map_guard
                            .complete_ready_db_creature_spell_cast_with_navigation(
                                attacker, target, now, navigation,
                            )?
                        {
                            packets.extend(
                                map_guard.materialize_db_creature_completed_spell_cast_packets(
                                    attacker, target, event,
                                ),
                            );
                        }
                    }
                }
            }
        }
        Ok(packets)
    }

    pub(in crate::world) async fn advance_all_player_environment_ticks(
        &self,
        now: Instant,
    ) -> anyhow::Result<Vec<(SessionId, OutboundWorldPacket)>> {
        let maps = { self.maps.lock().await.values().cloned().collect::<Vec<_>>() };
        let mut packets = Vec::new();
        for map in maps {
            packets.extend(map.lock().await.advance_player_environment_tick(now)?);
        }
        Ok(packets)
    }

    pub(in crate::world) async fn advance_all_playerbot_movement_ticks(
        &self,
        navigation: &DbCreatureNavigationGuardrail,
        now: Instant,
    ) -> anyhow::Result<PlayerbotMovementTick> {
        if self.active_playerbot_count.load(Ordering::Relaxed) == 0 {
            return Ok(PlayerbotMovementTick::default());
        }
        let maps = { self.maps.lock().await.values().cloned().collect::<Vec<_>>() };
        let mut aggregate = PlayerbotMovementTick::default();
        for map in maps {
            let has_playerbots = { map.lock().await.has_playerbots() };
            if !has_playerbots {
                continue;
            }
            let tick = map
                .lock()
                .await
                .advance_playerbot_movement_tick(navigation, now)?;
            aggregate.advanced_bots += tick.advanced_bots;
            aggregate.budget_exhausted |= tick.budget_exhausted;
            aggregate.packets.extend(tick.packets);
        }
        Ok(aggregate)
    }

    pub(in crate::world) async fn plan_all_playerbot_intents(
        &self,
        navigation: &DbCreatureNavigationGuardrail,
        now: Instant,
    ) -> anyhow::Result<PlayerbotPlanningTick> {
        if !self.has_async_playerbot_planner_work() {
            return Ok(PlayerbotPlanningTick::default());
        }
        let maps = {
            self.maps
                .lock()
                .await
                .iter()
                .map(|(key, map)| (*key, map.clone()))
                .collect::<Vec<_>>()
        };
        let mut inputs = Vec::new();
        for (_, map) in &maps {
            inputs.extend(map.lock().await.collect_playerbot_plan_inputs(now));
        }

        let map_count = maps.len().max(1);
        let mut budget = PlayerbotPlannerBudget {
            route_plans_remaining: PLAYERBOT_MAX_ROUTE_PLANS_PER_MAP_TICK * map_count,
            combat_thinks_remaining: PLAYERBOT_MAX_COMBAT_THINKS_PER_MAP_TICK * map_count,
            ..PlayerbotPlannerBudget::default()
        };
        let planned =
            plan_playerbot_intents(inputs, &self.faction_templates, navigation, &mut budget);
        let planned_bots = planned.len() as u32;

        let mut by_map: HashMap<(u32, u32), Vec<(u32, PlayerbotQueuedIntents)>> = HashMap::new();
        for (map_key, bot_guid, intent) in planned {
            by_map.entry(map_key).or_default().push((bot_guid, intent));
        }

        for (map_key, map) in maps {
            let Some(intents) = by_map.remove(&map_key) else {
                continue;
            };
            map.lock().await.queue_playerbot_intents(intents);
        }

        Ok(PlayerbotPlanningTick {
            planned_bots,
            route_budget_exhausted: budget.route_budget_exhausted,
            combat_budget_exhausted: budget.combat_budget_exhausted,
        })
    }

    pub(in crate::world) async fn advance_all_playerbot_combat_ticks(
        &self,
        navigation: &DbCreatureNavigationGuardrail,
        now: Instant,
    ) -> anyhow::Result<PlayerbotCombatTick> {
        if self.active_playerbot_count.load(Ordering::Relaxed) == 0 {
            return Ok(PlayerbotCombatTick::default());
        }
        let maps = { self.maps.lock().await.values().cloned().collect::<Vec<_>>() };
        let mut aggregate = PlayerbotCombatTick::default();
        for map in maps {
            let has_playerbots = { map.lock().await.has_playerbots() };
            if !has_playerbots {
                continue;
            }
            let tick = map.lock().await.advance_playerbot_combat_tick(
                &self.faction_templates,
                navigation,
                now,
            )?;
            aggregate.advanced_bots += tick.advanced_bots;
            aggregate.creature_swings += tick.creature_swings;
            aggregate.budget_exhausted |= tick.budget_exhausted;
            aggregate.packets.extend(tick.packets);
        }
        Ok(aggregate)
    }

    pub(in crate::world) async fn advance_all_player_aura_expirations(
        &self,
        now: Instant,
    ) -> anyhow::Result<Vec<(SessionId, OutboundWorldPacket)>> {
        let maps = { self.maps.lock().await.values().cloned().collect::<Vec<_>>() };
        let mut packets = Vec::new();
        for map in maps {
            packets.extend(map.lock().await.advance_player_aura_expirations(now)?);
        }
        Ok(packets)
    }

    pub(in crate::world) async fn advance_all_player_death_presentations(
        &self,
        now: Instant,
    ) -> anyhow::Result<Vec<(SessionId, OutboundWorldPacket)>> {
        let maps = { self.maps.lock().await.values().cloned().collect::<Vec<_>>() };
        let mut packets = Vec::new();
        for map in maps {
            packets.extend(map.lock().await.advance_player_death_presentations(now)?);
        }
        Ok(packets)
    }

    pub(in crate::world) async fn force_player_death_presentation(
        &self,
        map_id: u32,
        character_guid: u32,
        now: Instant,
    ) -> anyhow::Result<Vec<(SessionId, OutboundWorldPacket)>> {
        let map = { self.maps.lock().await.get(&(map_id, 0)).cloned() };
        let Some(map) = map else {
            return Ok(Vec::new());
        };
        let packets = map
            .lock()
            .await
            .force_player_death_presentation(character_guid, now)?;
        Ok(packets)
    }

    pub(in crate::world) async fn advance_all_db_creature_auras(
        &self,
        now: Instant,
        now_epoch_secs: u64,
    ) -> anyhow::Result<Vec<(SessionId, OutboundWorldPacket)>> {
        let maps = { self.maps.lock().await.values().cloned().collect::<Vec<_>>() };
        let mut packets = Vec::new();
        for map in maps {
            packets.extend(
                map.lock()
                    .await
                    .advance_db_creature_auras(now, now_epoch_secs)?,
            );
        }
        Ok(packets)
    }

    pub(in crate::world) async fn advance_all_dynamic_objects(
        &self,
        now: Instant,
        now_epoch_secs: u64,
    ) -> anyhow::Result<Vec<(SessionId, OutboundWorldPacket)>> {
        let maps = { self.maps.lock().await.values().cloned().collect::<Vec<_>>() };
        let mut packets = Vec::new();
        for map in maps {
            packets.extend(map.lock().await.advance_dynamic_objects(
                &self.faction_templates,
                now,
                now_epoch_secs,
            )?);
        }
        Ok(packets)
    }

    pub(in crate::world) async fn advance_all_player_channels(
        &self,
        now: Instant,
        now_epoch_secs: u64,
    ) -> anyhow::Result<Vec<(SessionId, OutboundWorldPacket)>> {
        let maps = { self.maps.lock().await.values().cloned().collect::<Vec<_>>() };
        let mut packets = Vec::new();
        for map in maps {
            packets.extend(
                map.lock()
                    .await
                    .advance_player_channels(now, now_epoch_secs)?,
            );
        }
        Ok(packets)
    }

    pub(in crate::world) async fn record_observability_snapshots(&self) {
        let maps = { self.maps.lock().await.values().cloned().collect::<Vec<_>>() };
        let mut snapshots = Vec::with_capacity(maps.len());
        let mut playerbot_snapshots = Vec::new();
        let now = Instant::now();
        for map in maps {
            let map = map.lock().await;
            snapshots.push(map.observability_snapshot());
            playerbot_snapshots.extend(map.playerbot_debug_snapshots(now));
        }
        crate::observability::record_map_runtime_snapshots(snapshots);
        crate::observability::record_playerbot_debug_snapshots(playerbot_snapshots);
    }
}
