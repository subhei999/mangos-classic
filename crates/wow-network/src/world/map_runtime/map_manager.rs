use super::*;

#[derive(Debug, Default)]
pub(in crate::world) struct MapRuntimeManager {
    pub(in crate::world) maps: Mutex<MapRuntimeHandles>,
    pub(in crate::world) movement_actors: Mutex<MovementActorHandles>,
    pub(in crate::world) movement_actor_settings: MovementActorSettings,
    pub(in crate::world) static_world_cache: Arc<StaticWorldSpawnCache>,
    pub(in crate::world) geometry: Arc<WorldGeometry>,
    pub(in crate::world) db_scripts: Arc<DbScriptRegistry>,
    pub(in crate::world) creature_display_scales: HashMap<u32, f32>,
    pub(in crate::world) spell_cast_times: HashMap<u32, SpellCastTimeEntry>,
    pub(in crate::world) spell_durations: HashMap<u32, SpellDurationEntry>,
    pub(in crate::world) spell_radii: HashMap<u32, SpellRadiusEntry>,
    pub(in crate::world) spell_ranges: HashMap<u32, SpellRangeEntry>,
    pub(in crate::world) skill_line_abilities_by_spell: HashMap<u32, Vec<SkillLineAbilityEntry>>,
    pub(in crate::world) skill_lines: HashMap<u32, SkillLineEntry>,
    pub(in crate::world) skill_race_class_infos_by_skill:
        HashMap<u32, Vec<SkillRaceClassInfoEntry>>,
    pub(in crate::world) faction_templates: FactionTemplateStore,
    pub(in crate::world) active_playerbot_count: AtomicUsize,
    pub(in crate::world) planner_driven_playerbot_count: AtomicUsize,
    pub(in crate::world) next_gm_creature_guid: AtomicU64,
    pub(in crate::world) creature_grid_load_ensure_calls: AtomicU64,
    pub(in crate::world) creature_grid_load_cache_hits: AtomicU64,
    pub(in crate::world) creature_grid_load_db_queries: AtomicU64,
    pub(in crate::world) creature_grid_load_rows: AtomicU64,
}

pub(in crate::world) type MapRuntimeHandles = HashMap<(u32, u32), Arc<Mutex<MapRuntime>>>;
pub(in crate::world) type MovementActorHandles = HashMap<(u32, u32), MovementActorHandle>;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
#[allow(dead_code)]
pub(in crate::world) struct CreatureGridLoadStats {
    pub(in crate::world) ensure_calls: u64,
    pub(in crate::world) cache_hits: u64,
    pub(in crate::world) db_queries: u64,
    pub(in crate::world) rows_loaded: u64,
}

pub(in crate::world) fn apply_creature_display_scale_fallbacks(
    spawns: &mut [CreatureSpawnQuery],
    display_scales: &HashMap<u32, f32>,
) {
    for spawn in spawns {
        if spawn.template.scale > 0.0 {
            continue;
        }
        let Some(scale) = [
            spawn.template.display_id1,
            spawn.template.display_id2,
            spawn.template.display_id3,
            spawn.template.display_id4,
        ]
        .into_iter()
        .find_map(|display_id| {
            display_scales
                .get(&display_id)
                .copied()
                .filter(|scale| *scale > 0.0)
        }) else {
            continue;
        };
        spawn.template.scale = scale;
    }
}

impl MapRuntimeManager {
    pub(in crate::world) fn with_movement_actor_settings(
        mut self,
        settings: MovementActorSettings,
    ) -> Self {
        self.movement_actor_settings = settings;
        self
    }

    #[cfg(test)]
    pub(in crate::world) fn with_movement_actor_settings_for_test(
        mut self,
        settings: MovementActorSettings,
    ) -> Self {
        self.movement_actor_settings = settings;
        self
    }

    #[allow(dead_code)]
    pub(in crate::world) fn with_world_data_files(world_data_files: &WorldDataFiles) -> Self {
        Self::with_world_data_files_and_static_cache(
            world_data_files,
            Arc::new(StaticWorldSpawnCache::default()),
        )
    }

    pub(in crate::world) fn with_world_data_files_and_static_cache(
        world_data_files: &WorldDataFiles,
        static_world_cache: Arc<StaticWorldSpawnCache>,
    ) -> Self {
        Self::with_world_data_files_static_cache_and_next_gm_guid(
            world_data_files,
            static_world_cache,
            1,
            Arc::new(DbScriptRegistry::default()),
        )
    }

    pub(in crate::world) fn with_world_data_files_static_cache_and_next_gm_guid(
        world_data_files: &WorldDataFiles,
        static_world_cache: Arc<StaticWorldSpawnCache>,
        next_gm_creature_guid: u64,
        db_scripts: Arc<DbScriptRegistry>,
    ) -> Self {
        let world_data_files = Arc::new(world_data_files.clone());
        Self {
            static_world_cache,
            geometry: Arc::new(WorldGeometry::new(world_data_files.clone())),
            db_scripts,
            creature_display_scales: world_data_files.creature_display_scales.clone(),
            spell_cast_times: world_data_files.spell_cast_times.clone(),
            spell_durations: world_data_files.spell_durations.clone(),
            spell_radii: world_data_files.spell_radii.clone(),
            spell_ranges: world_data_files.spell_ranges.clone(),
            skill_line_abilities_by_spell: world_data_files.skill_line_abilities_by_spell.clone(),
            skill_lines: world_data_files.skill_lines.clone(),
            skill_race_class_infos_by_skill: world_data_files
                .skill_race_class_infos_by_skill
                .clone(),
            faction_templates: world_data_files.faction_templates.clone(),
            next_gm_creature_guid: AtomicU64::new(next_gm_creature_guid.max(1)),
            ..Self::default()
        }
    }

    #[cfg(test)]
    pub(in crate::world) fn with_static_world_cache(
        static_world_cache: StaticWorldSpawnCache,
    ) -> Self {
        Self {
            static_world_cache: Arc::new(static_world_cache),
            ..Self::default()
        }
    }

    pub(in crate::world) fn spell_duration(
        &self,
        duration_index: u32,
    ) -> Option<SpellDurationEntry> {
        self.spell_durations.get(&duration_index).copied()
    }

    pub(in crate::world) fn spell_cast_time(
        &self,
        casting_time_index: u32,
    ) -> Option<SpellCastTimeEntry> {
        self.spell_cast_times.get(&casting_time_index).copied()
    }

    pub(in crate::world) fn spell_range(&self, range_index: u32) -> Option<SpellRangeEntry> {
        self.spell_ranges.get(&range_index).copied()
    }

    pub(in crate::world) fn spell_radius(&self, radius_index: u32) -> Option<SpellRadiusEntry> {
        self.spell_radii.get(&radius_index).copied()
    }

    pub(in crate::world) fn has_async_playerbot_planner_work(&self) -> bool {
        self.planner_driven_playerbot_count.load(Ordering::Relaxed) > 0
    }

    pub(in crate::world) fn skill_line_ability_for_spell(
        &self,
        spell_id: u32,
    ) -> Option<SkillLineAbilityEntry> {
        self.skill_line_abilities_by_spell
            .get(&spell_id)
            .and_then(|abilities| abilities.first())
            .copied()
    }

    pub(in crate::world) fn skill_line(&self, skill_id: u32) -> Option<SkillLineEntry> {
        self.skill_lines.get(&skill_id).copied()
    }

    pub(in crate::world) fn skill_race_class_info(
        &self,
        skill_id: u32,
        race: u8,
        class: u8,
    ) -> Option<SkillRaceClassInfoEntry> {
        let race_mask = 1u32.checked_shl(u32::from(race.saturating_sub(1)))?;
        let class_mask = 1u32.checked_shl(u32::from(class.saturating_sub(1)))?;
        self.skill_race_class_infos_by_skill
            .get(&skill_id)?
            .iter()
            .copied()
            .find(|entry| {
                (entry.race_mask == 0 || (entry.race_mask & race_mask) != 0)
                    && (entry.class_mask == 0 || (entry.class_mask & class_mask) != 0)
            })
    }

    pub(in crate::world) async fn set_active_player_spell_cast(
        &self,
        map_id: u32,
        character_guid: u32,
        active_cast: ActivePlayerSpellCast,
    ) {
        let map = self.get_or_create_map(map_id, 0).await;
        map.lock()
            .await
            .active_player_spell_casts
            .insert(character_guid, active_cast);
    }

    pub(in crate::world) async fn take_due_active_player_spell_cast(
        &self,
        map_id: u32,
        character_guid: u32,
        now: Instant,
    ) -> Option<ActivePlayerSpellCast> {
        let map = { self.maps.lock().await.get(&(map_id, 0)).cloned() }?;
        let mut map = map.lock().await;
        if map
            .active_player_spell_casts
            .get(&character_guid)
            .is_none_or(|active_cast| now < active_cast.due_at)
        {
            return None;
        }
        map.active_player_spell_casts.remove(&character_guid)
    }

    pub(in crate::world) async fn cancel_active_player_spell_cast(
        &self,
        map_id: u32,
        character_guid: u32,
    ) -> Option<ActivePlayerSpellCast> {
        let map = { self.maps.lock().await.get(&(map_id, 0)).cloned() }?;
        let active_cast = map
            .lock()
            .await
            .active_player_spell_casts
            .remove(&character_guid);
        active_cast
    }

    pub(in crate::world) async fn cancel_active_player_channel(
        &self,
        map_id: u32,
        character_guid: u32,
    ) -> anyhow::Result<Option<PlayerChannelEvent>> {
        let Some(map) = self.maps.lock().await.get(&(map_id, 0)).cloned() else {
            return Ok(None);
        };
        let mut map = map.lock().await;
        let event = if let Some(event) = map.cancel_player_channel(character_guid)? {
            Some(event)
        } else {
            map.cancel_player_dynamic_object_channel(character_guid)?
        };
        Ok(event)
    }

    #[allow(clippy::too_many_arguments)]
    pub(in crate::world) async fn start_player_periodic_trigger_channel(
        &self,
        map_id: u32,
        caster: ObjectGuid,
        caster_character_guid: u32,
        spell_id: u32,
        target: ObjectGuid,
        duration_millis: u32,
        tick_millis: u32,
        damage_effect: PlayerDirectDamageEffect,
        channel_interrupt_flags: u32,
        triggered_spell_speed: f32,
        now: Instant,
    ) -> anyhow::Result<Option<PlayerChannelEvent>> {
        let map = self.get_or_create_map(map_id, 0).await;
        let event = map.lock().await.start_player_periodic_trigger_channel(
            caster,
            caster_character_guid,
            spell_id,
            target,
            duration_millis,
            tick_millis,
            channel_interrupt_flags,
            triggered_spell_speed,
            damage_effect,
            now,
        )?;
        Ok(event)
    }

    pub(in crate::world) async fn interrupt_active_player_channel_for_damage(
        &self,
        map_id: u32,
        character_guid: u32,
        now: Instant,
    ) -> anyhow::Result<Option<PlayerChannelEvent>> {
        let Some(map) = self.maps.lock().await.get(&(map_id, 0)).cloned() else {
            return Ok(None);
        };
        let mut map = map.lock().await;
        let event =
            if let Some(event) = map.interrupt_player_channel_for_damage(character_guid, now)? {
                Some(event)
            } else {
                map.interrupt_dynamic_object_channel_for_damage(character_guid, now)?
            };
        Ok(event)
    }

    pub(in crate::world) async fn cancel_active_player_opening_spell_cast(
        &self,
        map_id: u32,
        character_guid: u32,
    ) -> Option<ActivePlayerSpellCast> {
        let map = { self.maps.lock().await.get(&(map_id, 0)).cloned() }?;
        let mut map = map.lock().await;
        if !map
            .active_player_spell_casts
            .get(&character_guid)
            .is_some_and(|active_cast| {
                matches!(
                    active_cast.source,
                    ActivePlayerSpellCastSource::OpeningGameObject
                )
            })
        {
            return None;
        }
        map.active_player_spell_casts.remove(&character_guid)
    }

    pub(in crate::world) async fn delay_active_player_spell_cast_for_damage(
        &self,
        map_id: u32,
        character_guid: u32,
        now: Instant,
    ) -> Option<u32> {
        let map = { self.maps.lock().await.get(&(map_id, 0)).cloned() }?;
        let mut map = map.lock().await;
        let active_cast = map.active_player_spell_casts.get_mut(&character_guid)?;
        if active_cast.interrupt_flags & SPELL_INTERRUPT_FLAG_DAMAGE_PUSHBACK == 0 {
            return None;
        }
        if active_cast.cast_time_millis == 0 || now >= active_cast.due_at {
            return None;
        }

        let next_delay = spell_damage_pushback_delay_millis(active_cast.damage_pushback_count);
        active_cast.damage_pushback_count = active_cast.damage_pushback_count.saturating_add(1);
        let remaining = active_cast
            .due_at
            .saturating_duration_since(now)
            .as_millis()
            .min(u128::from(u32::MAX)) as u32;
        let delay = next_delay.min(active_cast.cast_time_millis.saturating_sub(remaining));
        if delay == 0 {
            return None;
        }
        active_cast.due_at += Duration::from_millis(delay as u64);
        Some(delay)
    }

    pub(in crate::world) async fn push_pending_spell_event(
        &self,
        map_id: u32,
        caster_character_guid: u32,
        spell_id: u32,
        targets: PendingSpellCastTargets,
        due_at: Instant,
    ) {
        let map = self.get_or_create_map(map_id, 0).await;
        let mut map = map.lock().await;
        let event_id = map.next_spell_event_id;
        map.next_spell_event_id = map.next_spell_event_id.saturating_add(1).max(1);
        let kind = PendingSpellEventKind::Spell { targets };
        let unit_target_generation = pending_spell_event_unit_target_generation(&map, &kind);
        map.pending_spell_events.push(PendingSpellEvent {
            event_id,
            caster_character_guid,
            spell_id,
            kind,
            unit_target_generation,
            due_at,
        });
    }

    pub(in crate::world) async fn push_pending_ranged_auto_attack_event(
        &self,
        map_id: u32,
        caster_character_guid: u32,
        impact: PendingRangedAutoAttackImpact,
    ) {
        let map = self.get_or_create_map(map_id, 0).await;
        let mut map = map.lock().await;
        let event_id = map.next_spell_event_id;
        map.next_spell_event_id = map.next_spell_event_id.saturating_add(1).max(1);
        let kind = PendingSpellEventKind::RangedAutoAttack {
            target: impact.target,
            outcome: impact.outcome,
            weapon_skill_id: impact.weapon_skill_id,
        };
        let unit_target_generation = pending_spell_event_unit_target_generation(&map, &kind);
        map.pending_spell_events.push(PendingSpellEvent {
            event_id,
            caster_character_guid,
            spell_id: impact.spell_id,
            kind,
            unit_target_generation,
            due_at: impact.due_at,
        });
    }

    pub(in crate::world) async fn take_due_pending_spell_event(
        &self,
        map_id: u32,
        caster_character_guid: u32,
        now: Instant,
    ) -> Option<PendingSpellEvent> {
        let map = { self.maps.lock().await.get(&(map_id, 0)).cloned() }?;
        let mut map = map.lock().await;
        loop {
            let event_index = map
                .pending_spell_events
                .iter()
                .enumerate()
                .filter(|(_, event)| {
                    event.caster_character_guid == caster_character_guid && now >= event.due_at
                })
                .min_by_key(|(_, event)| (event.due_at, event.event_id))
                .map(|(index, _)| index)?;
            let event = map.pending_spell_events.remove(event_index);
            let stale = event
                .unit_target_generation
                .is_some_and(|(target, generation)| {
                    !map.creatures.get(&target.raw()).is_some_and(|creature| {
                        creature.is_alive() && creature.life_generation == generation
                    })
                });
            if !stale {
                return Some(event);
            }
        }
    }

    pub(in crate::world) async fn next_pending_player_spell_cast_due_at(
        &self,
        map_id: u32,
        character_guid: u32,
    ) -> Option<Instant> {
        let map = { self.maps.lock().await.get(&(map_id, 0)).cloned() }?;
        let map = map.lock().await;
        let active_due_at = map
            .active_player_spell_casts
            .get(&character_guid)
            .map(|active_cast| active_cast.due_at);
        let event_due_at = map
            .pending_spell_events
            .iter()
            .filter(|event| event.caster_character_guid == character_guid)
            .map(|event| event.due_at)
            .min();
        active_due_at.into_iter().chain(event_due_at).min()
    }

    pub(in crate::world) async fn add_player(
        &self,
        player: PlayerRuntime,
    ) -> anyhow::Result<Vec<(SessionId, OutboundWorldPacket)>> {
        let map_key = (player.position.map_id, 0);
        let new_requires_async_planner = player
            .bot_runtime
            .as_ref()
            .is_some_and(playerbot_runtime_requires_async_planner);
        let new_is_playerbot = player.bot_runtime.is_some();
        let map = {
            let mut maps = self.maps.lock().await;
            maps.entry(map_key)
                .or_insert_with(|| {
                    Arc::new(Mutex::new(MapRuntime::with_geometry(
                        map_key.0,
                        map_key.1,
                        self.geometry.clone(),
                        self.db_scripts.clone(),
                    )))
                })
                .clone()
        };
        let mut map = map.lock().await;
        let old_requires_async_planner = map.player_guid_requires_async_planner(player.guid);
        let old_is_playerbot = map
            .players
            .get(&player.guid)
            .is_some_and(|existing| existing.bot_runtime.is_some());
        let packets = map.add_player(player);
        drop(map);
        match (old_is_playerbot, new_is_playerbot) {
            (false, true) => {
                self.active_playerbot_count.fetch_add(1, Ordering::Relaxed);
            }
            (true, false) => {
                self.active_playerbot_count.fetch_sub(1, Ordering::Relaxed);
            }
            _ => {}
        }
        match (old_requires_async_planner, new_requires_async_planner) {
            (false, true) => {
                self.planner_driven_playerbot_count
                    .fetch_add(1, Ordering::Relaxed);
            }
            (true, false) => {
                self.planner_driven_playerbot_count
                    .fetch_sub(1, Ordering::Relaxed);
            }
            _ => {}
        }
        packets
    }

    pub(in crate::world) async fn remove_player(
        &self,
        map_id: u32,
        character_guid: u32,
    ) -> Vec<(SessionId, OutboundWorldPacket)> {
        let map = { self.maps.lock().await.get(&(map_id, 0)).cloned() };
        let Some(map) = map else {
            return Vec::new();
        };
        let mut map = map.lock().await;
        let removed_requires_async_planner = map.player_guid_requires_async_planner(character_guid);
        let removed_is_playerbot = map
            .players
            .get(&character_guid)
            .is_some_and(|player| player.bot_runtime.is_some());
        let packets = map.remove_player(character_guid);
        drop(map);
        if removed_is_playerbot {
            self.active_playerbot_count.fetch_sub(1, Ordering::Relaxed);
        }
        if removed_requires_async_planner {
            self.planner_driven_playerbot_count
                .fetch_sub(1, Ordering::Relaxed);
        }
        packets
    }

    pub(in crate::world) async fn disconnect_player_for_linger(
        &self,
        map_id: u32,
        character_guid: u32,
        now: Instant,
    ) -> bool {
        let map = { self.maps.lock().await.get(&(map_id, 0)).cloned() };
        let Some(map) = map else {
            return false;
        };
        let disconnected = map
            .lock()
            .await
            .disconnect_player_for_linger(character_guid, now);
        disconnected
    }

    pub(in crate::world) async fn expire_disconnected_players(
        &self,
        map_id: u32,
        now: Instant,
    ) -> Vec<ExpiredDisconnectedPlayer> {
        let map = { self.maps.lock().await.get(&(map_id, 0)).cloned() };
        let Some(map) = map else {
            return Vec::new();
        };
        let expired = map.lock().await.expire_disconnected_players(now);
        expired
    }

    pub(in crate::world) async fn update_player_position(
        &self,
        map_id: u32,
        character_guid: u32,
        opcode: u16,
        movement: &MovementInfo,
        server_time: u32,
    ) -> anyhow::Result<MovementUpdateOutcome> {
        let map_key = (map_id, 0);
        let map = { self.maps.lock().await.get(&map_key).cloned() };
        let Some(map) = map else {
            return Ok(MovementUpdateOutcome::Applied {
                packets: Vec::new(),
            });
        };

        if let Some(actor) = self.movement_actor_for_map(map_key, map.clone()).await {
            return actor
                .update_player_position(character_guid, opcode, movement, server_time)
                .await;
        }

        let mutex_wait_started_at = Instant::now();
        let mut map = map.lock().await;
        crate::observability::record_movement_map_mutex_wait(mutex_wait_started_at.elapsed());
        let mutex_hold_started_at = Instant::now();
        let packets = map.update_player_position(character_guid, opcode, movement, server_time)?;
        crate::observability::record_movement_map_mutex_hold(mutex_hold_started_at.elapsed());
        Ok(MovementUpdateOutcome::Applied { packets })
    }

    pub(in crate::world) async fn discover_player_area(
        &self,
        map_id: u32,
        character_guid: u32,
        area_flag: u16,
    ) -> anyhow::Result<Option<PlayerAreaDiscoveryEvent>> {
        let map = { self.maps.lock().await.get(&(map_id, 0)).cloned() };
        let Some(map) = map else {
            return Ok(None);
        };
        let event = map
            .lock()
            .await
            .discover_player_area(character_guid, area_flag)?;
        Ok(event)
    }

    pub(in crate::world) fn allocate_gm_creature_guid(&self) -> u32 {
        loop {
            let stored = self.next_gm_creature_guid.load(Ordering::Relaxed);
            let current = stored.clamp(1, 0x00FF_FFFF);
            let next = current.saturating_add(1).min(0x00FF_FFFF);
            if self
                .next_gm_creature_guid
                .compare_exchange(stored, next, Ordering::Relaxed, Ordering::Relaxed)
                .is_ok()
            {
                return current as u32;
            }
        }
    }

    pub(in crate::world) async fn spawn_gm_db_creature(
        &self,
        mut spawn: CreatureSpawnQuery,
        exclude_character_guid: Option<u32>,
    ) -> anyhow::Result<(DbCreatureRuntime, Vec<(SessionId, OutboundWorldPacket)>)> {
        spawn.guid = self.allocate_gm_creature_guid();
        apply_creature_display_scale_fallbacks(
            std::slice::from_mut(&mut spawn),
            &self.creature_display_scales,
        );
        let creature = DbCreatureRuntime::new(spawn);
        let body = build_update_object_body(&[build_db_creature_runtime_create_block(&creature)?]);
        let map = self
            .get_or_create_map(creature.current_position.map_id, 0)
            .await;
        let packets = map.lock().await.spawn_db_creature_and_broadcast(
            creature.clone(),
            exclude_character_guid,
            body,
        );
        Ok((creature, packets))
    }

    pub(in crate::world) async fn delete_db_creature_runtime(
        &self,
        map_id: u32,
        creature_guid: Option<ObjectGuid>,
        db_guid: Option<u32>,
        exclude_character_guid: Option<u32>,
    ) -> anyhow::Result<Option<DbCreatureDeleteEvent>> {
        let map = { self.maps.lock().await.get(&(map_id, 0)).cloned() };
        let Some(map) = map else {
            return Ok(None);
        };
        let event = map.lock().await.delete_db_creature_runtime(
            creature_guid,
            db_guid,
            exclude_character_guid,
        )?;
        Ok(event)
    }

    pub(in crate::world) async fn update_player_visible_equipment(
        &self,
        map_id: u32,
        character_guid: u32,
        visible_equipment: [u32; ENUM_EQUIPMENT_SLOTS],
        changed_slots: &[u8],
    ) -> anyhow::Result<Vec<(SessionId, OutboundWorldPacket)>> {
        let map = { self.maps.lock().await.get(&(map_id, 0)).cloned() };
        let Some(map) = map else {
            return Ok(Vec::new());
        };
        let packets = map.lock().await.update_player_visible_equipment(
            character_guid,
            visible_equipment,
            changed_slots,
        );
        packets
    }

    pub(in crate::world) async fn update_player_health(
        &self,
        map_id: u32,
        character_guid: u32,
        health: u32,
    ) -> anyhow::Result<Vec<(SessionId, OutboundWorldPacket)>> {
        let map = { self.maps.lock().await.get(&(map_id, 0)).cloned() };
        let Some(map) = map else {
            return Ok(Vec::new());
        };
        let packets = map
            .lock()
            .await
            .update_player_health(character_guid, health);
        packets
    }

    pub(in crate::world) async fn apply_player_heal(
        &self,
        map_id: u32,
        target_character_guid: u32,
        amount: u32,
    ) -> anyhow::Result<Option<PlayerHealEvent>> {
        let map = { self.maps.lock().await.get(&(map_id, 0)).cloned() };
        let Some(map) = map else {
            return Ok(None);
        };
        let event = map
            .lock()
            .await
            .apply_player_heal(target_character_guid, amount);
        event
    }

    pub(in crate::world) async fn sync_player_gameplay_state(
        &self,
        map_id: u32,
        character_guid: u32,
        session: &WorldSessionState,
    ) {
        let map = { self.maps.lock().await.get(&(map_id, 0)).cloned() };
        let Some(map) = map else {
            return;
        };
        map.lock()
            .await
            .sync_player_gameplay_state(character_guid, session);
    }

    pub(in crate::world) async fn remove_player_auras_with_interrupt_flag(
        &self,
        map_id: u32,
        character_guid: u32,
        interrupt_flag: u32,
    ) -> bool {
        let map = { self.maps.lock().await.get(&(map_id, 0)).cloned() };
        let Some(map) = map else {
            return false;
        };
        let removed = map
            .lock()
            .await
            .remove_player_auras_with_interrupt_flag(character_guid, interrupt_flag);
        removed
    }

    pub(in crate::world) async fn player_runtime_snapshot(
        &self,
        map_id: u32,
        character_guid: u32,
    ) -> Option<PlayerRuntimeSnapshot> {
        let map = { self.maps.lock().await.get(&(map_id, 0)).cloned() };
        let map = map?;
        let snapshot = map.lock().await.player_runtime_snapshot(character_guid);
        snapshot
    }

    pub(in crate::world) async fn update_player_reward_state(
        &self,
        map_id: u32,
        character_guid: u32,
        reward: PlayerRewardRuntimeUpdate,
    ) {
        let map = { self.maps.lock().await.get(&(map_id, 0)).cloned() };
        let Some(map) = map else {
            return;
        };
        map.lock()
            .await
            .update_player_reward_state(character_guid, reward);
    }

    pub(in crate::world) async fn update_player_level_progression_state(
        &self,
        map_id: u32,
        character_guid: u32,
        progression: PlayerLevelProgressionRuntimeUpdate,
    ) {
        let map = { self.maps.lock().await.get(&(map_id, 0)).cloned() };
        let Some(map) = map else {
            return;
        };
        map.lock()
            .await
            .update_player_level_progression_state(character_guid, progression);
    }

    pub(in crate::world) async fn update_player_inventory(
        &self,
        map_id: u32,
        character_guid: u32,
        inventory: Vec<CharacterInventoryItem>,
    ) {
        let map = { self.maps.lock().await.get(&(map_id, 0)).cloned() };
        let Some(map) = map else {
            return;
        };
        map.lock()
            .await
            .update_player_inventory(character_guid, inventory);
    }

    pub(in crate::world) async fn player_visible_db_creature_guids(
        &self,
        map_id: u32,
        character_guid: u32,
    ) -> Vec<u64> {
        let map = { self.maps.lock().await.get(&(map_id, 0)).cloned() };
        let Some(map) = map else {
            return Vec::new();
        };
        let guids = map
            .lock()
            .await
            .player_visible_db_creature_guids(character_guid);
        guids
    }

    pub(in crate::world) async fn player_visible_db_gameobject_guids(
        &self,
        map_id: u32,
        character_guid: u32,
    ) -> Vec<u64> {
        let map = { self.maps.lock().await.get(&(map_id, 0)).cloned() };
        let Some(map) = map else {
            return Vec::new();
        };
        let guids = map
            .lock()
            .await
            .player_visible_db_gameobject_guids(character_guid);
        guids
    }

    pub(in crate::world) async fn should_rescan_player_creature_visibility(
        &self,
        map_id: u32,
        character_guid: u32,
        position: WorldPosition,
    ) -> bool {
        let map = { self.maps.lock().await.get(&(map_id, 0)).cloned() };
        let Some(map) = map else {
            return true;
        };
        let should_rescan = map
            .lock()
            .await
            .should_rescan_player_creature_visibility(character_guid, position);
        should_rescan
    }

    pub(in crate::world) async fn should_rescan_player_gameobject_visibility(
        &self,
        map_id: u32,
        character_guid: u32,
        position: WorldPosition,
    ) -> bool {
        let map = { self.maps.lock().await.get(&(map_id, 0)).cloned() };
        let Some(map) = map else {
            return true;
        };
        let should_rescan = map
            .lock()
            .await
            .should_rescan_player_gameobject_visibility(character_guid, position);
        should_rescan
    }

    pub(in crate::world) async fn should_rescan_player_corpse_visibility(
        &self,
        map_id: u32,
        character_guid: u32,
        position: WorldPosition,
    ) -> bool {
        let map = { self.maps.lock().await.get(&(map_id, 0)).cloned() };
        let Some(map) = map else {
            return true;
        };
        let should_rescan = map
            .lock()
            .await
            .should_rescan_player_corpse_visibility(character_guid, position);
        should_rescan
    }

    pub(in crate::world) async fn reset_player_visibility_scan_positions(
        &self,
        map_id: u32,
        character_guid: u32,
    ) {
        let map = { self.maps.lock().await.get(&(map_id, 0)).cloned() };
        let Some(map) = map else {
            return;
        };
        map.lock()
            .await
            .reset_player_visibility_scan_positions(character_guid);
    }

    pub(in crate::world) async fn update_player_combat_stats(
        &self,
        map_id: u32,
        character_guid: u32,
        combat_stats: PlayerCombatStats,
    ) -> anyhow::Result<Vec<(SessionId, OutboundWorldPacket)>> {
        let map = { self.maps.lock().await.get(&(map_id, 0)).cloned() };
        let Some(map) = map else {
            return Ok(Vec::new());
        };
        let packets = map
            .lock()
            .await
            .update_player_combat_stats(character_guid, combat_stats);
        packets
    }

    pub(in crate::world) async fn player_combat_stats(
        &self,
        map_id: u32,
        character_guid: u32,
    ) -> Option<PlayerCombatStats> {
        let map = { self.maps.lock().await.get(&(map_id, 0)).cloned() };
        let map = map?;
        let combat_stats = map.lock().await.player_combat_stats(character_guid);
        combat_stats
    }

    pub(in crate::world) async fn set_player_auto_attack(
        &self,
        map_id: u32,
        character_guid: u32,
        target: Option<ObjectGuid>,
        next_swing_at: Option<Instant>,
    ) {
        let map = { self.maps.lock().await.get(&(map_id, 0)).cloned() };
        let Some(map) = map else {
            return;
        };
        map.lock()
            .await
            .set_player_auto_attack(character_guid, target, next_swing_at);
    }

    #[cfg(test)]
    pub(in crate::world) async fn set_player_ranged_auto_attack(
        &self,
        map_id: u32,
        character_guid: u32,
        target: Option<ObjectGuid>,
        next_swing_at: Option<Instant>,
        spell_id: u32,
    ) {
        let map = { self.maps.lock().await.get(&(map_id, 0)).cloned() };
        let Some(map) = map else {
            return;
        };
        map.lock().await.set_player_ranged_auto_attack(
            character_guid,
            target,
            next_swing_at,
            spell_id,
        );
    }

    pub(in crate::world) async fn set_player_ranged_auto_attack_started(
        &self,
        map_id: u32,
        character_guid: u32,
        target: Option<ObjectGuid>,
        requested_next_shot_at: Instant,
        spell_id: u32,
    ) -> Instant {
        let map = { self.maps.lock().await.get(&(map_id, 0)).cloned() };
        let Some(map) = map else {
            return requested_next_shot_at;
        };
        let next_shot_at = map
            .lock()
            .await
            .set_player_ranged_auto_attack_started(
                character_guid,
                target,
                requested_next_shot_at,
                spell_id,
            )
            .unwrap_or(requested_next_shot_at);
        next_shot_at
    }

    pub(in crate::world) async fn set_player_ranged_next_shot_at(
        &self,
        map_id: u32,
        character_guid: u32,
        next_shot_at: Instant,
    ) {
        let map = { self.maps.lock().await.get(&(map_id, 0)).cloned() };
        let Some(map) = map else {
            return;
        };
        map.lock()
            .await
            .set_player_ranged_next_shot_at(character_guid, next_shot_at);
    }

    pub(in crate::world) async fn stop_player_melee_auto_attack(
        &self,
        map_id: u32,
        character_guid: u32,
    ) -> Option<(ObjectGuid, Option<Instant>)> {
        let map = { self.maps.lock().await.get(&(map_id, 0)).cloned() };
        let map = map?;
        let stopped = map
            .lock()
            .await
            .stop_player_melee_auto_attack(character_guid);
        stopped
    }

    pub(in crate::world) async fn player_auto_attack_due(
        &self,
        map_id: u32,
        character_guid: u32,
        now: Instant,
    ) -> Option<PlayerAutoAttackDue> {
        let map = { self.maps.lock().await.get(&(map_id, 0)).cloned() };
        let map = map?;
        let mut map = map.lock().await;
        map.player_auto_attack_due(character_guid, now)
    }

    pub(in crate::world) async fn retime_player_auto_attack_after_spell_cast(
        &self,
        map_id: u32,
        character_guid: u32,
        now: Instant,
        melee_delay: Duration,
        ranged_windup: Duration,
        cancel_ranged_auto_repeat: bool,
    ) -> PlayerAutoAttackAfterSpellCast {
        let map = { self.maps.lock().await.get(&(map_id, 0)).cloned() };
        let Some(map) = map else {
            return PlayerAutoAttackAfterSpellCast::None;
        };
        let mut map = map.lock().await;
        map.retime_player_auto_attack_after_spell_cast(
            character_guid,
            now,
            melee_delay,
            ranged_windup,
            cancel_ranged_auto_repeat,
        )
    }

    #[allow(dead_code)]
    pub(in crate::world) async fn player_auto_attack_target(
        &self,
        map_id: u32,
        character_guid: u32,
    ) -> Option<ObjectGuid> {
        let map = { self.maps.lock().await.get(&(map_id, 0)).cloned() };
        let map = map?;
        let target = map.lock().await.player_auto_attack_target(character_guid);
        target
    }

    pub(in crate::world) async fn set_player_next_swing_at(
        &self,
        map_id: u32,
        character_guid: u32,
        next_swing_at: Option<Instant>,
    ) {
        let map = { self.maps.lock().await.get(&(map_id, 0)).cloned() };
        let Some(map) = map else {
            return;
        };
        map.lock()
            .await
            .set_player_next_swing_at(character_guid, next_swing_at);
    }

    pub(in crate::world) async fn set_player_power2(
        &self,
        map_id: u32,
        character_guid: u32,
        power2: u32,
    ) {
        let map = { self.maps.lock().await.get(&(map_id, 0)).cloned() };
        let Some(map) = map else {
            return;
        };
        map.lock().await.set_player_power2(character_guid, power2);
    }

    pub(in crate::world) async fn player_selected_target(
        &self,
        map_id: u32,
        character_guid: u32,
    ) -> Option<ObjectGuid> {
        let map = { self.maps.lock().await.get(&(map_id, 0)).cloned() };
        let map = map?;
        let selected_target = map.lock().await.player_selected_target(character_guid);
        selected_target
    }

    pub(in crate::world) async fn player_spell_cast_failure(
        &self,
        map_id: u32,
        character_guid: u32,
        spell_profile: &SpellCastProfile,
        now: Instant,
    ) -> Option<u8> {
        let map = { self.maps.lock().await.get(&(map_id, 0)).cloned() };
        let map = map?;
        let failure =
            map.lock()
                .await
                .player_spell_cast_failure(character_guid, spell_profile, now);
        failure
    }

    pub(in crate::world) async fn apply_player_spell_cooldowns(
        &self,
        map_id: u32,
        character_guid: u32,
        spell_profile: &SpellCastProfile,
        now: Instant,
        skip_spell_cooldown: bool,
    ) {
        let map = { self.maps.lock().await.get(&(map_id, 0)).cloned() };
        let Some(map) = map else {
            return;
        };
        map.lock().await.apply_player_spell_cooldowns(
            character_guid,
            spell_profile,
            now,
            skip_spell_cooldown,
        );
    }

    #[allow(clippy::too_many_arguments)]
    pub(in crate::world) async fn apply_player_item_spell_cooldowns(
        &self,
        map_id: u32,
        character_guid: u32,
        spell_profile: &SpellCastProfile,
        now: Instant,
        skip_spell_cooldown: bool,
        item_id: u32,
        category: u32,
        category_cooldown_millis: u64,
    ) {
        let map = { self.maps.lock().await.get(&(map_id, 0)).cloned() };
        let Some(map) = map else {
            return;
        };
        map.lock()
            .await
            .apply_player_spell_cooldowns_with_item_category(
                character_guid,
                spell_profile,
                now,
                skip_spell_cooldown,
                item_id,
                category,
                category_cooldown_millis,
            );
    }

    pub(in crate::world) async fn clear_player_spell_recovery(
        &self,
        map_id: u32,
        character_guid: u32,
        spell_profile: &SpellCastProfile,
    ) {
        let map = { self.maps.lock().await.get(&(map_id, 0)).cloned() };
        let Some(map) = map else {
            return;
        };
        map.lock()
            .await
            .clear_player_spell_recovery(character_guid, spell_profile);
    }

    pub(in crate::world) async fn spend_player_spell_power(
        &self,
        map_id: u32,
        character_guid: u32,
        spell_profile: &SpellCastProfile,
        now: Instant,
        blocks_mana_regen: bool,
    ) -> Result<(), u8> {
        let map = { self.maps.lock().await.get(&(map_id, 0)).cloned() };
        let Some(map) = map else {
            return Ok(());
        };
        let result = map.lock().await.spend_player_spell_power(
            character_guid,
            spell_profile,
            now,
            blocks_mana_regen,
        );
        result
    }

    pub(in crate::world) async fn queue_player_next_melee_spell(
        &self,
        map_id: u32,
        character_guid: u32,
        queued: QueuedNextMeleeSpell,
    ) {
        let map = { self.maps.lock().await.get(&(map_id, 0)).cloned() };
        let Some(map) = map else {
            return;
        };
        map.lock()
            .await
            .queue_player_next_melee_spell(character_guid, queued);
    }

    pub(in crate::world) async fn queued_player_next_melee_spell(
        &self,
        map_id: u32,
        character_guid: u32,
        target: ObjectGuid,
    ) -> Option<QueuedNextMeleeSpell> {
        let map = { self.maps.lock().await.get(&(map_id, 0)).cloned() };
        let map = map?;
        let queued = map
            .lock()
            .await
            .queued_player_next_melee_spell(character_guid, target);
        queued
    }

    pub(in crate::world) async fn clear_player_next_melee_spell(
        &self,
        map_id: u32,
        character_guid: u32,
    ) {
        let map = { self.maps.lock().await.get(&(map_id, 0)).cloned() };
        let Some(map) = map else {
            return;
        };
        map.lock()
            .await
            .clear_player_next_melee_spell(character_guid);
    }

    pub(in crate::world) async fn spend_queued_player_next_melee_spell_power(
        &self,
        map_id: u32,
        character_guid: u32,
        queued: QueuedNextMeleeSpell,
    ) -> Result<(), u8> {
        let map = { self.maps.lock().await.get(&(map_id, 0)).cloned() };
        let Some(map) = map else {
            return Ok(());
        };
        let result = map
            .lock()
            .await
            .spend_queued_player_next_melee_spell_power(character_guid, queued);
        result
    }

    pub(in crate::world) async fn update_player_selection(
        &self,
        map_id: u32,
        character_guid: u32,
        selected_target: Option<ObjectGuid>,
    ) -> anyhow::Result<Vec<(SessionId, OutboundWorldPacket)>> {
        let map = { self.maps.lock().await.get(&(map_id, 0)).cloned() };
        let Some(map) = map else {
            return Ok(Vec::new());
        };
        let packets = map
            .lock()
            .await
            .update_player_selection(character_guid, selected_target);
        packets
    }

    pub(in crate::world) async fn update_player_target(
        &self,
        map_id: u32,
        character_guid: u32,
        unit_target: Option<ObjectGuid>,
    ) -> anyhow::Result<Vec<(SessionId, OutboundWorldPacket)>> {
        let map = { self.maps.lock().await.get(&(map_id, 0)).cloned() };
        let Some(map) = map else {
            return Ok(Vec::new());
        };
        let packets = map
            .lock()
            .await
            .update_player_target(character_guid, unit_target);
        packets
    }

    pub(in crate::world) async fn add_player_combo_points(
        &self,
        map_id: u32,
        character_guid: u32,
        target: ObjectGuid,
        points: u8,
    ) -> Option<PlayerComboPointsEvent> {
        let map = { self.maps.lock().await.get(&(map_id, 0)).cloned() };
        let map = map?;
        let event = map
            .lock()
            .await
            .add_player_combo_points(character_guid, target, points);
        event
    }

    pub(in crate::world) async fn clear_player_combo_points(
        &self,
        map_id: u32,
        character_guid: u32,
    ) -> Option<PlayerComboPointsEvent> {
        let map = { self.maps.lock().await.get(&(map_id, 0)).cloned() };
        let map = map?;
        let event = map.lock().await.clear_player_combo_points(character_guid);
        event
    }

    #[cfg(test)]
    pub(in crate::world) async fn update_player_db_creature_visibility(
        &self,
        map_id: u32,
        character_guid: u32,
        create_guids: &[ObjectGuid],
        destroy_guids: &[ObjectGuid],
    ) {
        let map = { self.maps.lock().await.get(&(map_id, 0)).cloned() };
        let Some(map) = map else {
            return;
        };
        map.lock().await.update_player_db_creature_visibility(
            character_guid,
            create_guids,
            destroy_guids,
        );
    }

    pub(in crate::world) async fn broadcast_nearby_player_packet(
        &self,
        map_id: u32,
        character_guid: u32,
        radius: f32,
        packet: OutboundWorldPacket,
    ) -> Vec<(SessionId, OutboundWorldPacket)> {
        let map = { self.maps.lock().await.get(&(map_id, 0)).cloned() };
        let Some(map) = map else {
            return Vec::new();
        };
        let packets =
            map.lock()
                .await
                .broadcast_nearby_player_packet(character_guid, radius, packet);
        packets
    }

    pub(in crate::world) async fn set_player_looting_state(
        &self,
        map_id: u32,
        character_guid: u32,
        looting: bool,
    ) -> anyhow::Result<Vec<(SessionId, OutboundWorldPacket)>> {
        let map = { self.maps.lock().await.get(&(map_id, 0)).cloned() };
        let Some(map) = map else {
            return Ok(Vec::new());
        };
        let packets = map
            .lock()
            .await
            .set_player_looting_state(character_guid, looting)?;
        Ok(packets)
    }

    pub(in crate::world) async fn set_player_stand_state(
        &self,
        map_id: u32,
        character_guid: u32,
        stand_state: u8,
    ) -> anyhow::Result<Vec<(SessionId, OutboundWorldPacket)>> {
        let map = { self.maps.lock().await.get(&(map_id, 0)).cloned() };
        let Some(map) = map else {
            return Ok(Vec::new());
        };
        let packets = map
            .lock()
            .await
            .set_player_stand_state(character_guid, stand_state)?;
        Ok(packets)
    }

    pub(in crate::world) async fn set_player_gm_flags(
        &self,
        map_id: u32,
        character_guid: u32,
        player_flags: u32,
    ) -> anyhow::Result<Vec<(SessionId, OutboundWorldPacket)>> {
        let map = { self.maps.lock().await.get(&(map_id, 0)).cloned() };
        let Some(map) = map else {
            return Ok(Vec::new());
        };
        let packets = map
            .lock()
            .await
            .set_player_gm_flags(character_guid, player_flags)?;
        Ok(packets)
    }

    #[allow(dead_code)]
    pub(in crate::world) async fn share_db_creature_snapshots(
        &self,
        map_id: u32,
        creatures: Vec<DbCreatureRuntime>,
    ) -> Vec<DbCreatureRuntime> {
        let map = self.get_or_create_map(map_id, 0).await;
        let creatures = map.lock().await.share_db_creature_snapshots(creatures);
        creatures
    }

    pub(in crate::world) async fn ensure_db_creature_grids_loaded(
        &self,
        character_db_pool: &MySqlPool,
        _world_db_pool: &MySqlPool,
        map_id: u32,
        position: WorldPosition,
        radius: f32,
    ) -> anyhow::Result<()> {
        self.creature_grid_load_ensure_calls
            .fetch_add(1, Ordering::Relaxed);
        let map = self.get_or_create_map(map_id, 0).await;
        let grids = {
            map.lock()
                .await
                .unloaded_creature_grids_for_area(position, radius)
        };
        if grids.is_empty() {
            let cache_hits = self
                .creature_grid_load_cache_hits
                .fetch_add(1, Ordering::Relaxed)
                + 1;
            debug!(
                map_id,
                x = position.x,
                y = position.y,
                radius,
                cache_hits,
                "DB creature visibility served from already loaded grids"
            );
            return Ok(());
        }
        for grid in grids {
            let (min_x, max_x, min_y, max_y) = grid_world_bounds(grid);
            let lookup_started_at = Instant::now();
            let spawns = self
                .static_world_cache
                .creature_spawns_for_grid(map_id, grid);
            crate::observability::record_static_world_cache_lookup(
                crate::observability::StaticWorldCacheKind::Creature,
                lookup_started_at.elapsed(),
            );
            let mut spawns = spawns;
            apply_creature_display_scale_fallbacks(&mut spawns, &self.creature_display_scales);
            let spawn_count = spawns.len() as u64;
            let cache_lookups = self
                .creature_grid_load_db_queries
                .fetch_add(1, Ordering::Relaxed)
                + 1;
            let rows_loaded = self
                .creature_grid_load_rows
                .fetch_add(spawn_count, Ordering::Relaxed)
                + spawn_count;
            let instantiation_started_at = Instant::now();
            let runtimes =
                build_db_creature_runtimes_with_respawns(character_db_pool, spawns).await?;
            crate::observability::record_static_world_cache_instantiation(
                crate::observability::StaticWorldCacheKind::Creature,
                spawn_count,
                instantiation_started_at.elapsed(),
            );
            map.lock().await.insert_loaded_creature_grid(grid, runtimes);
            info!(
                map_id,
                grid_x = grid.x,
                grid_y = grid.y,
                min_x,
                max_x,
                min_y,
                max_y,
                spawn_count,
                cache_lookups,
                rows_loaded,
                "Loaded static creature grid into MapRuntime"
            );
        }
        Ok(())
    }

    pub(in crate::world) async fn refresh_static_game_event_spawns(
        &self,
        character_db_pool: &MySqlPool,
        game_events: GameEventState,
        now: Instant,
    ) -> anyhow::Result<Vec<(SessionId, OutboundWorldPacket)>> {
        if !self
            .static_world_cache
            .replace_active_game_events(game_events)
        {
            return Ok(Vec::new());
        }

        let map_handles = {
            self.maps
                .lock()
                .await
                .iter()
                .map(|(key, map)| (*key, map.clone()))
                .collect::<Vec<_>>()
        };
        let mut packets = Vec::new();
        for ((map_id, _instance_id), map) in map_handles {
            let (creature_grids, gameobject_grids) = {
                let map = map.lock().await;
                (map.loaded_creature_grids(), map.loaded_gameobject_grids())
            };

            for grid in creature_grids {
                let mut spawns = self
                    .static_world_cache
                    .creature_spawns_for_grid(map_id, grid);
                apply_creature_display_scale_fallbacks(&mut spawns, &self.creature_display_scales);
                let runtimes =
                    build_db_creature_runtimes_with_respawns(character_db_pool, spawns).await?;
                packets.extend(
                    map.lock()
                        .await
                        .refresh_static_event_creature_grid(grid, runtimes)?,
                );
            }

            for grid in gameobject_grids {
                let spawns = self
                    .static_world_cache
                    .gameobject_spawns_for_grid(map_id, grid);
                let runtimes = spawns.into_iter().map(DbGameObjectRuntime::new).collect();
                packets.extend(
                    map.lock()
                        .await
                        .refresh_static_event_gameobject_grid(grid, runtimes, now)?,
                );
            }
        }
        Ok(packets)
    }

    pub(in crate::world) async fn apply_player_aura(
        &self,
        map_id: u32,
        character_guid: u32,
        aura: ActiveAura,
    ) -> anyhow::Result<Option<PlayerAuraUpdateEvent>> {
        self.apply_player_aura_replacing_spell_ids(map_id, character_guid, aura, &[])
            .await
    }

    pub(in crate::world) async fn apply_player_aura_replacing_spell_ids(
        &self,
        map_id: u32,
        character_guid: u32,
        aura: ActiveAura,
        replace_spell_ids: &[u32],
    ) -> anyhow::Result<Option<PlayerAuraUpdateEvent>> {
        let resolution = AuraRankConflictResolution {
            failure: None,
            replace_spell_ids: replace_spell_ids.to_vec(),
            replace_any_caster_spell_ids: Vec::new(),
        };
        self.apply_player_aura_replacing_conflicts(map_id, character_guid, aura, &resolution)
            .await
    }

    pub(in crate::world) async fn apply_player_aura_replacing_conflicts(
        &self,
        map_id: u32,
        character_guid: u32,
        aura: ActiveAura,
        resolution: &AuraRankConflictResolution,
    ) -> anyhow::Result<Option<PlayerAuraUpdateEvent>> {
        let map = { self.maps.lock().await.get(&(map_id, 0)).cloned() };
        let Some(map) = map else {
            return Ok(None);
        };
        let event = map.lock().await.apply_player_aura_replacing_conflicts(
            character_guid,
            aura,
            resolution,
        );
        event
    }

    pub(in crate::world) async fn remove_player_auras_by_dispel_type(
        &self,
        map_id: u32,
        character_guid: u32,
        dispel_type: u32,
        count: u32,
        now: Instant,
    ) -> anyhow::Result<Option<PlayerAuraDispelEvent>> {
        let map = { self.maps.lock().await.get(&(map_id, 0)).cloned() };
        let Some(map) = map else {
            return Ok(None);
        };
        let event = map.lock().await.remove_player_auras_by_dispel_type(
            character_guid,
            dispel_type,
            count,
            now,
        );
        event
    }

    pub(in crate::world) async fn apply_db_creature_aura(
        &self,
        map_id: u32,
        creature_guid: ObjectGuid,
        caster_character_guid: u32,
        aura: ActiveAura,
        now: Instant,
    ) -> anyhow::Result<Option<DbCreatureAuraUpdateEvent>> {
        self.apply_db_creature_aura_replacing_spell_ids(
            map_id,
            creature_guid,
            caster_character_guid,
            aura,
            &[],
            None,
            None,
            now,
        )
        .await
    }

    #[allow(clippy::too_many_arguments)]
    pub(in crate::world) async fn apply_db_creature_aura_replacing_spell_ids(
        &self,
        map_id: u32,
        creature_guid: ObjectGuid,
        caster_character_guid: u32,
        aura: ActiveAura,
        replace_spell_ids: &[u32],
        single_target_descriptor: Option<SingleTargetAuraDescriptor>,
        diminishing_group: Option<DiminishingGroupRuntime>,
        now: Instant,
    ) -> anyhow::Result<Option<DbCreatureAuraUpdateEvent>> {
        let resolution = AuraRankConflictResolution {
            failure: None,
            replace_spell_ids: replace_spell_ids.to_vec(),
            replace_any_caster_spell_ids: Vec::new(),
        };
        self.apply_db_creature_aura_replacing_conflicts(
            map_id,
            creature_guid,
            caster_character_guid,
            aura,
            &resolution,
            single_target_descriptor,
            diminishing_group,
            now,
        )
        .await
    }

    #[allow(clippy::too_many_arguments)]
    pub(in crate::world) async fn apply_db_creature_aura_replacing_conflicts(
        &self,
        map_id: u32,
        creature_guid: ObjectGuid,
        caster_character_guid: u32,
        aura: ActiveAura,
        resolution: &AuraRankConflictResolution,
        single_target_descriptor: Option<SingleTargetAuraDescriptor>,
        diminishing_group: Option<DiminishingGroupRuntime>,
        now: Instant,
    ) -> anyhow::Result<Option<DbCreatureAuraUpdateEvent>> {
        let map = { self.maps.lock().await.get(&(map_id, 0)).cloned() };
        let Some(map) = map else {
            return Ok(None);
        };
        let event = {
            let mut map = map.lock().await;
            map.apply_db_creature_aura_replacing_conflicts(
                creature_guid,
                caster_character_guid,
                aura,
                resolution,
                single_target_descriptor,
                diminishing_group,
                now,
            )
        };
        event
    }

    pub(in crate::world) async fn current_diminishing_level(
        &self,
        map_id: u32,
        target: ObjectGuid,
        group: DiminishingGroupRuntime,
        now: Instant,
    ) -> Option<DiminishingLevelRuntime> {
        let map = { self.maps.lock().await.get(&(map_id, 0)).cloned() }?;
        let level = map
            .lock()
            .await
            .current_diminishing_level(target, group, now);
        Some(level)
    }

    pub(in crate::world) async fn remove_db_creature_auras_by_dispel_type(
        &self,
        map_id: u32,
        creature_guid: ObjectGuid,
        caster_character_guid: u32,
        dispel_type: u32,
        count: u32,
        now: Instant,
    ) -> anyhow::Result<Option<DbCreatureAuraDispelEvent>> {
        let map = { self.maps.lock().await.get(&(map_id, 0)).cloned() };
        let Some(map) = map else {
            return Ok(None);
        };
        let event = map.lock().await.remove_db_creature_auras_by_dispel_type(
            creature_guid,
            caster_character_guid,
            dispel_type,
            count,
            now,
        );
        event
    }

    #[allow(clippy::too_many_arguments)]
    pub(in crate::world) async fn create_persistent_area_dynamic_object(
        &self,
        map_id: u32,
        caster: ObjectGuid,
        caster_character_guid: u32,
        spell_id: u32,
        effect_index: usize,
        position: WorldPosition,
        radius: f32,
        duration_millis: u32,
        periodic_damage: Option<PeriodicDamageAura>,
        channeled: bool,
        channel_interrupt_flags: u32,
        now: Instant,
    ) -> anyhow::Result<Option<DynamicObjectCreateEvent>> {
        let map = { self.maps.lock().await.get(&(map_id, 0)).cloned() };
        let Some(map) = map else {
            return Ok(None);
        };
        let event = map.lock().await.create_persistent_area_dynamic_object(
            caster,
            caster_character_guid,
            spell_id,
            effect_index,
            position,
            radius,
            duration_millis,
            periodic_damage,
            channeled,
            channel_interrupt_flags,
            now,
        )?;
        Ok(event)
    }

    pub(in crate::world) async fn nearby_attackable_db_creature_guids_for_player_spell(
        &self,
        map_id: u32,
        character_guid: u32,
        radius: f32,
    ) -> Vec<ObjectGuid> {
        let map = { self.maps.lock().await.get(&(map_id, 0)).cloned() };
        let Some(map) = map else {
            return Vec::new();
        };
        let targets = map
            .lock()
            .await
            .nearby_attackable_db_creature_guids_for_player_spell(
                &self.faction_templates,
                character_guid,
                radius,
            );
        targets
    }

    pub(in crate::world) async fn nearby_attackable_db_creature_guids_for_player_spell_at_position(
        &self,
        map_id: u32,
        character_guid: u32,
        position: WorldPosition,
        radius: f32,
    ) -> Vec<ObjectGuid> {
        let map = { self.maps.lock().await.get(&(map_id, 0)).cloned() };
        let Some(map) = map else {
            return Vec::new();
        };
        let targets = map
            .lock()
            .await
            .nearby_attackable_db_creature_guids_for_player_spell_at_position(
                &self.faction_templates,
                character_guid,
                position,
                radius,
            );
        targets
    }

    pub(in crate::world) async fn set_player_position(
        &self,
        map_id: u32,
        character_guid: u32,
        position: WorldPosition,
    ) -> anyhow::Result<Vec<(SessionId, OutboundWorldPacket)>> {
        let map = { self.maps.lock().await.get(&(map_id, 0)).cloned() };
        let Some(map) = map else {
            return Ok(Vec::new());
        };
        let mut map = map.lock().await;
        map.set_player_position(character_guid, position)
    }

    #[cfg(test)]
    pub(in crate::world) async fn ensure_db_creature_grids_loaded_for_test(
        &self,
        map_id: u32,
        position: WorldPosition,
        radius: f32,
        mut runtimes_for_grid: impl FnMut(GridCoord) -> Vec<DbCreatureRuntime>,
    ) {
        self.creature_grid_load_ensure_calls
            .fetch_add(1, Ordering::Relaxed);
        let map = self.get_or_create_map(map_id, 0).await;
        let grids = {
            map.lock()
                .await
                .unloaded_creature_grids_for_area(position, radius)
        };
        if grids.is_empty() {
            self.creature_grid_load_cache_hits
                .fetch_add(1, Ordering::Relaxed);
            return;
        }
        for grid in grids {
            let runtimes = runtimes_for_grid(grid);
            self.creature_grid_load_db_queries
                .fetch_add(1, Ordering::Relaxed);
            self.creature_grid_load_rows
                .fetch_add(runtimes.len() as u64, Ordering::Relaxed);
            map.lock().await.insert_loaded_creature_grid(grid, runtimes);
        }
    }

    #[cfg(test)]
    pub(in crate::world) async fn ensure_static_creature_grids_loaded_for_test(
        &self,
        map_id: u32,
        position: WorldPosition,
        radius: f32,
    ) {
        self.creature_grid_load_ensure_calls
            .fetch_add(1, Ordering::Relaxed);
        let map = self.get_or_create_map(map_id, 0).await;
        let grids = {
            map.lock()
                .await
                .unloaded_creature_grids_for_area(position, radius)
        };
        if grids.is_empty() {
            self.creature_grid_load_cache_hits
                .fetch_add(1, Ordering::Relaxed);
            return;
        }
        for grid in grids {
            let lookup_started_at = Instant::now();
            let spawns = self
                .static_world_cache
                .creature_spawns_for_grid(map_id, grid);
            crate::observability::record_static_world_cache_lookup(
                crate::observability::StaticWorldCacheKind::Creature,
                lookup_started_at.elapsed(),
            );
            let spawn_count = spawns.len() as u64;
            self.creature_grid_load_db_queries
                .fetch_add(1, Ordering::Relaxed);
            self.creature_grid_load_rows
                .fetch_add(spawn_count, Ordering::Relaxed);
            let instantiation_started_at = Instant::now();
            let runtimes = spawns.into_iter().map(DbCreatureRuntime::new).collect();
            crate::observability::record_static_world_cache_instantiation(
                crate::observability::StaticWorldCacheKind::Creature,
                spawn_count,
                instantiation_started_at.elapsed(),
            );
            map.lock().await.insert_loaded_creature_grid(grid, runtimes);
        }
    }

    #[allow(dead_code)]
    pub(in crate::world) fn creature_grid_load_stats(&self) -> CreatureGridLoadStats {
        CreatureGridLoadStats {
            ensure_calls: self.creature_grid_load_ensure_calls.load(Ordering::Relaxed),
            cache_hits: self.creature_grid_load_cache_hits.load(Ordering::Relaxed),
            db_queries: self.creature_grid_load_db_queries.load(Ordering::Relaxed),
            rows_loaded: self.creature_grid_load_rows.load(Ordering::Relaxed),
        }
    }

    pub(in crate::world) async fn nearby_db_creature_snapshots(
        &self,
        map_id: u32,
        position: WorldPosition,
        radius: f32,
        limit: u32,
    ) -> Vec<DbCreatureRuntime> {
        let map = { self.maps.lock().await.get(&(map_id, 0)).cloned() };
        let Some(map) = map else {
            return Vec::new();
        };
        let snapshots = map
            .lock()
            .await
            .nearby_db_creature_snapshots(position, radius, limit);
        snapshots
    }

    pub(in crate::world) async fn stage_player_db_creature_visibility(
        &self,
        map_id: u32,
        character_guid: u32,
        position: WorldPosition,
        nearby_creatures: Vec<DbCreatureRuntime>,
    ) -> MapDbCreatureVisibilityStage {
        let map = { self.maps.lock().await.get(&(map_id, 0)).cloned() };
        let Some(map) = map else {
            return MapDbCreatureVisibilityStage {
                nearby_creatures,
                ..Default::default()
            };
        };
        let stage = map.lock().await.stage_player_db_creature_visibility(
            character_guid,
            position,
            nearby_creatures,
        );
        stage
    }

    pub(in crate::world) async fn stage_player_db_gameobject_visibility(
        &self,
        map_id: u32,
        character_guid: u32,
        position: WorldPosition,
        nearby_gameobjects: Vec<DbGameObjectRuntime>,
        now: Instant,
    ) -> MapDbGameObjectVisibilityStage {
        let map = { self.maps.lock().await.get(&(map_id, 0)).cloned() };
        let Some(map) = map else {
            return MapDbGameObjectVisibilityStage {
                nearby_gameobjects,
                ..Default::default()
            };
        };
        let stage = map.lock().await.stage_player_db_gameobject_visibility(
            character_guid,
            position,
            nearby_gameobjects,
            now,
        );
        stage
    }

    pub(in crate::world) async fn ensure_db_gameobject_grids_loaded(
        &self,
        _world_db_pool: &MySqlPool,
        map_id: u32,
        position: WorldPosition,
        radius: f32,
    ) -> anyhow::Result<()> {
        let map = self.get_or_create_map(map_id, 0).await;
        let grids = {
            map.lock()
                .await
                .unloaded_gameobject_grids_for_area(position, radius)
        };
        for grid in grids {
            let (min_x, max_x, min_y, max_y) = grid_world_bounds(grid);
            let lookup_started_at = Instant::now();
            let spawns = self
                .static_world_cache
                .gameobject_spawns_for_grid(map_id, grid);
            crate::observability::record_static_world_cache_lookup(
                crate::observability::StaticWorldCacheKind::GameObject,
                lookup_started_at.elapsed(),
            );
            let spawn_count = spawns.len() as u64;
            let instantiation_started_at = Instant::now();
            let runtimes = spawns.into_iter().map(DbGameObjectRuntime::new).collect();
            crate::observability::record_static_world_cache_instantiation(
                crate::observability::StaticWorldCacheKind::GameObject,
                spawn_count,
                instantiation_started_at.elapsed(),
            );
            map.lock()
                .await
                .insert_loaded_gameobject_grid(grid, runtimes);
            info!(
                map_id,
                grid_x = grid.x,
                grid_y = grid.y,
                min_x,
                max_x,
                min_y,
                max_y,
                spawn_count,
                "Loaded static gameobject grid into MapRuntime"
            );
        }
        Ok(())
    }

    #[cfg(test)]
    pub(in crate::world) async fn ensure_db_gameobject_grids_loaded_for_test(
        &self,
        map_id: u32,
        position: WorldPosition,
        radius: f32,
        mut runtimes_for_grid: impl FnMut(GridCoord) -> Vec<DbGameObjectRuntime>,
    ) {
        let map = self.get_or_create_map(map_id, 0).await;
        let grids = {
            map.lock()
                .await
                .unloaded_gameobject_grids_for_area(position, radius)
        };
        for grid in grids {
            let runtimes = runtimes_for_grid(grid);
            map.lock()
                .await
                .insert_loaded_gameobject_grid(grid, runtimes);
        }
    }

    #[cfg(test)]
    pub(in crate::world) async fn ensure_static_gameobject_grids_loaded_for_test(
        &self,
        map_id: u32,
        position: WorldPosition,
        radius: f32,
    ) {
        let map = self.get_or_create_map(map_id, 0).await;
        let grids = {
            map.lock()
                .await
                .unloaded_gameobject_grids_for_area(position, radius)
        };
        for grid in grids {
            let lookup_started_at = Instant::now();
            let spawns = self
                .static_world_cache
                .gameobject_spawns_for_grid(map_id, grid);
            crate::observability::record_static_world_cache_lookup(
                crate::observability::StaticWorldCacheKind::GameObject,
                lookup_started_at.elapsed(),
            );
            let spawn_count = spawns.len() as u64;
            let instantiation_started_at = Instant::now();
            let runtimes = spawns.into_iter().map(DbGameObjectRuntime::new).collect();
            crate::observability::record_static_world_cache_instantiation(
                crate::observability::StaticWorldCacheKind::GameObject,
                spawn_count,
                instantiation_started_at.elapsed(),
            );
            map.lock()
                .await
                .insert_loaded_gameobject_grid(grid, runtimes);
        }
    }

    pub(in crate::world) async fn nearby_db_gameobject_snapshots(
        &self,
        map_id: u32,
        position: WorldPosition,
        radius: f32,
        limit: u32,
    ) -> Vec<DbGameObjectRuntime> {
        let map = { self.maps.lock().await.get(&(map_id, 0)).cloned() };
        let Some(map) = map else {
            return Vec::new();
        };
        let snapshots = map
            .lock()
            .await
            .nearby_db_gameobject_snapshots(position, radius, limit);
        snapshots
    }

    pub(in crate::world) async fn ensure_player_corpse_grids_loaded(
        &self,
        character_db_pool: &MySqlPool,
        map_id: u32,
        position: WorldPosition,
        radius: f32,
    ) -> anyhow::Result<()> {
        let map = self.get_or_create_map(map_id, 0).await;
        let grids = {
            map.lock()
                .await
                .unloaded_player_corpse_grids_for_area(position, radius)
        };
        for grid in grids {
            let (center_x, center_y) = grid_world_center(grid);
            let corpses = wow_db::get_nearby_player_corpses(
                character_db_pool,
                map_id,
                center_x,
                center_y,
                player_corpse_grid_query_radius(),
                u32::MAX,
            )
            .await?
            .into_iter()
            .map(player_corpse_runtime_from_query)
            .collect::<Vec<_>>();
            map.lock()
                .await
                .insert_loaded_player_corpse_grid(grid, corpses);
            let corpse_count = map.lock().await.corpses.len();
            debug!(
                map_id,
                grid_x = grid.x,
                grid_y = grid.y,
                corpse_count,
                "Loaded player corpse grid into MapRuntime"
            );
        }
        Ok(())
    }

    pub(in crate::world) async fn nearby_player_corpse_snapshots(
        &self,
        map_id: u32,
        position: WorldPosition,
        radius: f32,
        limit: u32,
    ) -> Vec<PlayerCorpseRuntime> {
        let map = { self.maps.lock().await.get(&(map_id, 0)).cloned() };
        let Some(map) = map else {
            return Vec::new();
        };
        let snapshots = map
            .lock()
            .await
            .nearby_player_corpse_snapshots(position, radius, limit);
        snapshots
    }

    pub(in crate::world) async fn stage_player_corpse_visibility(
        &self,
        map_id: u32,
        character_guid: u32,
        position: WorldPosition,
        nearby_corpses: Vec<PlayerCorpseRuntime>,
    ) -> MapPlayerCorpseVisibilityStage {
        let map = { self.maps.lock().await.get(&(map_id, 0)).cloned() };
        let Some(map) = map else {
            return MapPlayerCorpseVisibilityStage {
                nearby_corpses,
                ..Default::default()
            };
        };
        let stage = map.lock().await.stage_player_corpse_visibility(
            character_guid,
            position,
            nearby_corpses,
        );
        stage
    }

    pub(in crate::world) async fn upsert_player_corpse(
        &self,
        map_id: u32,
        corpse: PlayerCorpseRuntime,
    ) {
        let map = self.get_or_create_map(map_id, 0).await;
        map.lock().await.upsert_player_corpse(corpse);
    }

    pub(in crate::world) async fn db_gameobject_snapshot(
        &self,
        map_id: u32,
        gameobject_guid: ObjectGuid,
    ) -> Option<DbGameObjectRuntime> {
        let map = { self.maps.lock().await.get(&(map_id, 0)).cloned() };
        let map = map?;
        let snapshot = map.lock().await.db_gameobject_snapshot(gameobject_guid);
        snapshot
    }

    pub(in crate::world) async fn consume_db_gameobject(
        &self,
        map_id: u32,
        gameobject_guid: ObjectGuid,
        now: Instant,
        exclude_character_guid: Option<u32>,
    ) -> Option<(DbGameObjectRuntime, Vec<(SessionId, OutboundWorldPacket)>)> {
        let map = { self.maps.lock().await.get(&(map_id, 0)).cloned() };
        let map = map?;
        let consumed =
            map.lock()
                .await
                .consume_db_gameobject(gameobject_guid, now, exclude_character_guid);
        consumed
    }

    pub(in crate::world) async fn open_db_gameobject_loot(
        &self,
        map_id: u32,
        gameobject_guid: u64,
        character_guid: u32,
        loot_items: Vec<DbCreatureLootRuntime>,
    ) -> Option<(DbGameObjectRuntime, Vec<DbCreatureLootRuntime>)> {
        let map = { self.maps.lock().await.get(&(map_id, 0)).cloned() };
        let map = map?;
        let opened =
            map.lock()
                .await
                .open_db_gameobject_loot(gameobject_guid, character_guid, loot_items);
        opened
    }

    pub(in crate::world) async fn db_gameobject_loot_guid_for_character(
        &self,
        map_id: u32,
        character_guid: u32,
    ) -> Option<u64> {
        let map = { self.maps.lock().await.get(&(map_id, 0)).cloned() };
        let map = map?;
        let gameobject_guid = map
            .lock()
            .await
            .db_gameobject_loot_guid_for_character(character_guid);
        gameobject_guid
    }

    pub(in crate::world) async fn take_db_gameobject_loot_item(
        &self,
        map_id: u32,
        character_guid: u32,
        loot_slot: u8,
    ) -> Option<(u64, u8, DbCreatureLootRuntime)> {
        let map = { self.maps.lock().await.get(&(map_id, 0)).cloned() };
        let map = map?;
        let loot = map
            .lock()
            .await
            .take_db_gameobject_loot_item(character_guid, loot_slot);
        loot
    }

    pub(in crate::world) async fn restore_db_gameobject_loot_item(
        &self,
        map_id: u32,
        gameobject_guid: u64,
        loot_slot: u8,
        loot: DbCreatureLootRuntime,
    ) -> Option<Vec<DbCreatureLootRuntime>> {
        let map = { self.maps.lock().await.get(&(map_id, 0)).cloned() };
        let map = map?;
        let restored =
            map.lock()
                .await
                .restore_db_gameobject_loot_item(gameobject_guid, loot_slot, loot);
        restored
    }

    pub(in crate::world) async fn db_gameobject_loot_is_empty(
        &self,
        map_id: u32,
        gameobject_guid: u64,
    ) -> bool {
        let map = { self.maps.lock().await.get(&(map_id, 0)).cloned() };
        let Some(map) = map else {
            return true;
        };
        let is_empty = map
            .lock()
            .await
            .db_gameobject_loot_is_empty(gameobject_guid);
        is_empty
    }

    pub(in crate::world) async fn release_db_gameobject_loot(
        &self,
        map_id: u32,
        gameobject_guid: u64,
        character_guid: u32,
    ) -> Option<()> {
        let map = { self.maps.lock().await.get(&(map_id, 0)).cloned() };
        let map = map?;
        let released = map
            .lock()
            .await
            .release_db_gameobject_loot(gameobject_guid, character_guid);
        released
    }

    pub(in crate::world) async fn db_creature_snapshots(
        &self,
        map_id: u32,
        creature_guids: &[u64],
    ) -> Vec<DbCreatureRuntime> {
        let map = self.get_or_create_map(map_id, 0).await;
        let snapshots = map.lock().await.db_creature_snapshots(creature_guids);
        snapshots
    }

    pub(in crate::world) async fn db_gameobject_snapshots(
        &self,
        map_id: u32,
        gameobject_guids: &[u64],
    ) -> Vec<DbGameObjectRuntime> {
        let map = self.get_or_create_map(map_id, 0).await;
        let snapshots = map.lock().await.db_gameobject_snapshots(gameobject_guids);
        snapshots
    }

    pub(in crate::world) async fn db_creature_snapshot(
        &self,
        map_id: u32,
        creature_guid: ObjectGuid,
    ) -> Option<DbCreatureRuntime> {
        let map = { self.maps.lock().await.get(&(map_id, 0)).cloned() };
        let map = map?;
        let snapshot = map.lock().await.db_creature_snapshot(creature_guid);
        snapshot
    }

    #[allow(dead_code)]
    pub(in crate::world) async fn db_creature_combat_snapshot(
        &self,
        map_id: u32,
        creature_guid: ObjectGuid,
    ) -> Option<DbCreatureRuntime> {
        let map = { self.maps.lock().await.get(&(map_id, 0)).cloned() };
        let map = map?;
        let creature = map.lock().await.db_creature_combat_snapshot(creature_guid);
        creature
    }

    pub(in crate::world) async fn validate_player_melee_against_db_creature(
        &self,
        map_id: u32,
        character_guid: u32,
        target: ObjectGuid,
        navigation: &DbCreatureNavigationGuardrail,
    ) -> DbCreaturePlayerMeleeValidation {
        let map = { self.maps.lock().await.get(&(map_id, 0)).cloned() };
        let Some(map) = map else {
            return DbCreaturePlayerMeleeValidation {
                check: PlayerMeleeCheck::MissingTarget,
            };
        };
        let validation = map.lock().await.validate_player_melee_against_db_creature(
            character_guid,
            target,
            navigation,
        );
        validation
    }

    pub(in crate::world) async fn validate_player_charge_against_db_creature(
        &self,
        map_id: u32,
        character_guid: u32,
        target: ObjectGuid,
        navigation: &DbCreatureNavigationGuardrail,
    ) -> PlayerChargeValidation {
        let map = { self.maps.lock().await.get(&(map_id, 0)).cloned() };
        let Some(map) = map else {
            return PlayerChargeValidation {
                check: PlayerChargeCheck::MissingTarget,
            };
        };
        let validation = map.lock().await.validate_player_charge_against_db_creature(
            character_guid,
            target,
            navigation,
        );
        validation
    }

    pub(in crate::world) async fn validate_player_spell_against_db_creature(
        &self,
        map_id: u32,
        character_guid: u32,
        target: ObjectGuid,
        navigation: &DbCreatureNavigationGuardrail,
        range: Option<SpellRangeEntry>,
        requires_infront: bool,
    ) -> PlayerSpellTargetValidation {
        let map = { self.maps.lock().await.get(&(map_id, 0)).cloned() };
        let Some(map) = map else {
            return PlayerSpellTargetValidation {
                check: PlayerSpellTargetCheck::MissingTarget,
            };
        };
        let validation = map.lock().await.validate_player_spell_against_db_creature(
            character_guid,
            target,
            navigation,
            range,
            requires_infront,
        );
        validation
    }

    #[allow(dead_code)]
    pub(in crate::world) async fn validate_db_creature_spell_against_target(
        &self,
        map_id: u32,
        caster: ObjectGuid,
        target: ObjectGuid,
        navigation: &DbCreatureNavigationGuardrail,
        range: Option<SpellRangeEntry>,
        requires_behind: bool,
    ) -> DbCreatureSpellTargetValidation {
        let map = { self.maps.lock().await.get(&(map_id, 0)).cloned() };
        let Some(map) = map else {
            return DbCreatureSpellTargetValidation {
                check: DbCreatureSpellTargetCheck::MissingCaster,
            };
        };
        let validation = map.lock().await.validate_db_creature_spell_against_target(
            caster,
            target,
            navigation,
            range,
            requires_behind,
        );
        validation
    }

    #[allow(dead_code)]
    pub(in crate::world) async fn update_db_creature_snapshot(
        &self,
        map_id: u32,
        creature: DbCreatureRuntime,
    ) {
        let map = self.get_or_create_map(map_id, 0).await;
        map.lock().await.update_db_creature_snapshot(creature);
    }

    pub(in crate::world) async fn update_db_creature_snapshot_and_broadcast(
        &self,
        map_id: u32,
        creature: DbCreatureRuntime,
        exclude_character_guid: Option<u32>,
        packet: OutboundWorldPacket,
    ) -> Vec<(SessionId, OutboundWorldPacket)> {
        let map = self.get_or_create_map(map_id, 0).await;
        let packets = map.lock().await.update_db_creature_snapshot_and_broadcast(
            creature,
            exclude_character_guid,
            packet,
        );
        packets
    }

    pub(in crate::world) async fn apply_db_creature_damage(
        &self,
        map_id: u32,
        request: DbCreatureDamageRequest,
    ) -> anyhow::Result<Option<DbCreatureDamageEvent>> {
        let map = self.get_or_create_map(map_id, 0).await;
        let event = map.lock().await.apply_db_creature_damage(request);
        event
    }

    pub(in crate::world) async fn open_db_creature_loot(
        &self,
        map_id: u32,
        creature_guid: u64,
        character_guid: u32,
        access_owner: CreatureLootOwner,
        current_looter: Option<u32>,
        loot_items: Vec<DbCreatureLootRuntime>,
    ) -> Option<DbCreatureRuntime> {
        let map = self.get_or_create_map(map_id, 0).await;
        let creature = map.lock().await.open_db_creature_loot(
            creature_guid,
            character_guid,
            access_owner,
            current_looter,
            loot_items,
        );
        creature
    }

    pub(in crate::world) async fn set_db_creature_loot_owner(
        &self,
        map_id: u32,
        creature_guid: ObjectGuid,
        owner: CreatureLootOwner,
    ) -> Option<DbCreatureRuntime> {
        let map = self.get_or_create_map(map_id, 0).await;
        let creature = map
            .lock()
            .await
            .set_db_creature_loot_owner(creature_guid, owner);
        creature
    }

    pub(in crate::world) async fn force_db_creature_loot_owner(
        &self,
        map_id: u32,
        creature_guid: ObjectGuid,
        owner: CreatureLootOwner,
    ) -> Option<DbCreatureRuntime> {
        let map = self.get_or_create_map(map_id, 0).await;
        let creature = map
            .lock()
            .await
            .force_db_creature_loot_owner(creature_guid, owner);
        creature
    }

    pub(in crate::world) async fn db_creature_loot_guid_for_character(
        &self,
        map_id: u32,
        character_guid: u32,
    ) -> Option<u64> {
        let map = { self.maps.lock().await.get(&(map_id, 0)).cloned() };
        let map = map?;
        let creature_guid = map
            .lock()
            .await
            .db_creature_loot_guid_for_character(character_guid);
        creature_guid
    }

    pub(in crate::world) async fn db_creature_looting_characters(
        &self,
        map_id: u32,
        creature_guid: u64,
    ) -> Vec<u32> {
        let Some(map) = self.maps.lock().await.get(&(map_id, 0)).cloned() else {
            return Vec::new();
        };
        let characters = map
            .lock()
            .await
            .db_creature_looting_characters(creature_guid);
        characters
    }

    pub(in crate::world) async fn db_creature_needs_loot_item(
        &self,
        map_id: u32,
        creature_guid: u64,
    ) -> Option<bool> {
        let map = { self.maps.lock().await.get(&(map_id, 0)).cloned() };
        let map = map?;
        let needs_loot_item = map.lock().await.db_creature_needs_loot_item(creature_guid);
        needs_loot_item
    }

    pub(in crate::world) async fn take_db_creature_loot_money(
        &self,
        map_id: u32,
        character_guid: u32,
    ) -> Option<(u32, DbCreatureRuntime)> {
        let map = self.get_or_create_map(map_id, 0).await;
        let loot = map.lock().await.take_db_creature_loot_money(character_guid);
        loot
    }

    pub(in crate::world) async fn take_db_creature_loot_item(
        &self,
        map_id: u32,
        character_guid: u32,
        loot_slot: u8,
    ) -> Option<(u64, u8, DbCreatureLootRuntime, DbCreatureRuntime)> {
        let map = self.get_or_create_map(map_id, 0).await;
        let loot = map
            .lock()
            .await
            .take_db_creature_loot_item(character_guid, loot_slot);
        loot
    }

    pub(in crate::world) async fn take_db_creature_loot_item_by_guid(
        &self,
        map_id: u32,
        creature_guid: u64,
        loot_slot: u8,
    ) -> Option<(u8, DbCreatureLootRuntime, DbCreatureRuntime)> {
        let map = self.get_or_create_map(map_id, 0).await;
        let loot = map
            .lock()
            .await
            .take_db_creature_loot_item_by_guid(creature_guid, loot_slot);
        loot
    }

    pub(in crate::world) async fn restore_db_creature_loot_item(
        &self,
        map_id: u32,
        creature_guid: u64,
        loot_slot: u8,
        loot: DbCreatureLootRuntime,
    ) -> Option<DbCreatureRuntime> {
        let map = self.get_or_create_map(map_id, 0).await;
        let creature =
            map.lock()
                .await
                .restore_db_creature_loot_item(creature_guid, loot_slot, loot);
        creature
    }

    pub(in crate::world) async fn release_db_creature_loot_roll_item(
        &self,
        map_id: u32,
        creature_guid: u64,
        loot_slot: u8,
    ) -> Option<DbCreatureRuntime> {
        let map = self.get_or_create_map(map_id, 0).await;
        let creature = map
            .lock()
            .await
            .release_db_creature_loot_roll_item(creature_guid, loot_slot);
        creature
    }

    pub(in crate::world) async fn release_db_creature_current_looter_pass_item(
        &self,
        map_id: u32,
        creature_guid: u64,
        loot_slot: u8,
    ) -> Option<DbCreatureRuntime> {
        let map = self.get_or_create_map(map_id, 0).await;
        let creature = map
            .lock()
            .await
            .release_db_creature_current_looter_pass_item(creature_guid, loot_slot);
        creature
    }

    pub(in crate::world) async fn release_db_creature_loot(
        &self,
        map_id: u32,
        creature_guid: u64,
        now: Instant,
        exclude_character_guid: Option<u32>,
    ) -> anyhow::Result<Option<DbCreatureLootReleaseEvent>> {
        let map = self.get_or_create_map(map_id, 0).await;
        let event =
            map.lock()
                .await
                .release_db_creature_loot(creature_guid, now, exclude_character_guid);
        event
    }

    pub(in crate::world) async fn begin_db_creature_combat(
        &self,
        map_id: u32,
        attacker: ObjectGuid,
        victim: ObjectGuid,
        now: Instant,
    ) -> Option<(CreatureCombatState, DbCreatureRuntime)> {
        let map = self.get_or_create_map(map_id, 0).await;
        let mut map = map.lock().await;
        let creature = map.db_creature_combat_snapshot(attacker)?;
        let combat = map.begin_db_creature_combat(attacker, victim, now)?;
        Some((combat, creature))
    }

    #[allow(dead_code)]
    pub(in crate::world) async fn clear_db_creature_combat(
        &self,
        map_id: u32,
        attacker: ObjectGuid,
    ) {
        let map = self.get_or_create_map(map_id, 0).await;
        map.lock().await.clear_db_creature_combat(attacker);
    }

    #[allow(dead_code)]
    pub(in crate::world) async fn clear_db_creature_combats_for_victim(
        &self,
        map_id: u32,
        victim: ObjectGuid,
    ) {
        let map = self.get_or_create_map(map_id, 0).await;
        map.lock()
            .await
            .clear_db_creature_combats_for_victim(victim);
    }

    pub(in crate::world) async fn active_db_creature_combats_for_victim(
        &self,
        map_id: u32,
        victim: ObjectGuid,
    ) -> Vec<CreatureCombatState> {
        let map = self.get_or_create_map(map_id, 0).await;
        let combats = map
            .lock()
            .await
            .active_db_creature_combats_for_victim(victim);
        combats
    }

    #[allow(clippy::too_many_arguments)]
    pub(in crate::world) async fn advance_db_creature_combats_for_victim(
        &self,
        world_db_pool: &MySqlPool,
        object_mgr: &ObjectMgr,
        map_id: u32,
        victim: ObjectGuid,
        current_session_id: SessionId,
        defense: PlayerMeleeDefenseInput,
        navigation: &DbCreatureNavigationGuardrail,
        now: Instant,
    ) -> anyhow::Result<DbCreatureVictimCombatAdvanceTick> {
        let map = self.get_or_create_map(map_id, 0).await;
        {
            let mut map_guard = map.lock().await;
            let player_alive = map_guard
                .players
                .get(&victim.counter())
                .is_some_and(|player| {
                    player.health > 0 && player.death_state == PlayerDeathState::Alive
                });
            if !player_alive {
                map_guard.clear_db_creature_combats_for_victim(victim);
                return Ok(DbCreatureVictimCombatAdvanceTick::default());
            }
        }

        let attackers = map
            .lock()
            .await
            .active_db_creature_combat_attackers_for_victim(victim);
        let mut tick = DbCreatureVictimCombatAdvanceTick::default();
        for attacker in attackers {
            let victim_died = self
                .advance_db_creature_attack_for_victim(
                    map.clone(),
                    world_db_pool,
                    object_mgr,
                    victim,
                    current_session_id,
                    attacker,
                    defense,
                    navigation,
                    now,
                    &mut tick,
                )
                .await?;
            if victim_died {
                break;
            }
        }
        let map_guard = map.lock().await;
        tick.active_combats = map_guard.active_db_creature_combats_for_victim(victim);
        tick.player_in_combat = map_guard
            .players
            .get(&victim.counter())
            .is_some_and(|player| player.in_combat);
        Ok(tick)
    }

    #[allow(dead_code)]
    pub(in crate::world) async fn active_db_creature_combat_snapshot(
        &self,
        map_id: u32,
        attacker: ObjectGuid,
        victim: ObjectGuid,
    ) -> Option<ActiveDbCreatureCombatSnapshot> {
        let map = { self.maps.lock().await.get(&(map_id, 0)).cloned() };
        let map = map?;
        let snapshot = map
            .lock()
            .await
            .active_db_creature_combat_snapshot(attacker, victim);
        snapshot
    }

    fn split_packets_for_session(
        current_session_id: SessionId,
        packets: Vec<(SessionId, OutboundWorldPacket)>,
        tick: &mut DbCreatureVictimCombatAdvanceTick,
    ) {
        for (session_id, packet) in packets {
            if session_id == current_session_id {
                tick.direct_packets.push(packet);
            } else {
                tick.observer_packets.push((session_id, packet));
            }
        }
    }

    fn push_creature_broadcast_packet(
        map: &MapRuntime,
        victim: ObjectGuid,
        current_session_id: SessionId,
        position: WorldPosition,
        packet: OutboundWorldPacket,
        tick: &mut DbCreatureVictimCombatAdvanceTick,
    ) {
        tick.direct_packets.push(packet.clone());
        tick.observer_packets
            .extend(map.nearby_player_packet_broadcast(
                position,
                Some(victim.counter()),
                packet.opcode,
                packet.body,
            ));
        let _ = current_session_id;
    }

    #[allow(clippy::too_many_arguments)]
    async fn advance_db_creature_attack_for_victim(
        &self,
        map: Arc<Mutex<MapRuntime>>,
        world_db_pool: &MySqlPool,
        object_mgr: &ObjectMgr,
        victim: ObjectGuid,
        current_session_id: SessionId,
        attacker: ObjectGuid,
        defense: PlayerMeleeDefenseInput,
        navigation: &DbCreatureNavigationGuardrail,
        now: Instant,
        tick: &mut DbCreatureVictimCombatAdvanceTick,
    ) -> anyhow::Result<bool> {
        let (active, was_fleeing, player_position, player_level) = {
            let mut map_guard = map.lock().await;
            let was_fleeing = map_guard
                .creatures
                .get(&attacker.raw())
                .is_some_and(|creature| creature.is_fleeing());
            let _ = map_guard.advance_db_creature_motion(attacker, now);
            let Some(active) = map_guard.active_db_creature_combat_snapshot(attacker, victim)
            else {
                return Ok(false);
            };
            let Some(player) = map_guard.players.get(&victim.counter()) else {
                return Ok(false);
            };
            (active, was_fleeing, player.position, player.level)
        };

        if was_fleeing && !active.creature.is_fleeing() {
            let body = build_unit_flags_update_body(
                attacker,
                db_creature_unit_flags(&active.creature, true),
            )?;
            let packet = OutboundWorldPacket {
                opcode: SMSG_UPDATE_OBJECT,
                body,
            };
            let map_guard = map.lock().await;
            Self::push_creature_broadcast_packet(
                &map_guard,
                victim,
                current_session_id,
                active.creature.current_position,
                packet,
                tick,
            );
        }

        if active_aura_has_hard_control(&active.creature.active_auras)
            || active.creature.is_fleeing()
        {
            map.lock()
                .await
                .defer_ready_db_creature_swing_retry(attacker, victim, now);
            return Ok(false);
        }

        let should_evade = { map.lock().await.db_creature_should_evade(attacker, now) };
        if should_evade {
            let mut map_guard = map.lock().await;
            let Some(creature) = map_guard.prepare_db_creature_evade(attacker) else {
                return Ok(false);
            };
            let attack_stop_packet = OutboundWorldPacket {
                opcode: SMSG_ATTACKSTOP,
                body: build_attack_stop_body(attacker, victim, false)?,
            };
            Self::push_creature_broadcast_packet(
                &map_guard,
                victim,
                current_session_id,
                creature.current_position,
                attack_stop_packet,
                tick,
            );
            let creature_flags_packet = OutboundWorldPacket {
                opcode: SMSG_UPDATE_OBJECT,
                body: build_unit_flags_update_body(
                    attacker,
                    db_creature_unit_flags(&creature, false),
                )?,
            };
            Self::push_creature_broadcast_packet(
                &map_guard,
                victim,
                current_session_id,
                creature.current_position,
                creature_flags_packet,
                tick,
            );
            let state_packet = OutboundWorldPacket {
                opcode: SMSG_UPDATE_OBJECT,
                body: build_db_creature_state_update_body(attacker, creature.health, 0)?,
            };
            Self::push_creature_broadcast_packet(
                &map_guard,
                victim,
                current_session_id,
                creature.current_position,
                state_packet,
                tick,
            );
            if let Some((returned, motion)) =
                map_guard.start_db_creature_return_home_motion(navigation, attacker, now)
            {
                let packet = OutboundWorldPacket {
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
                };
                Self::push_creature_broadcast_packet(
                    &map_guard,
                    victim,
                    current_session_id,
                    returned.current_position,
                    packet,
                    tick,
                );
            }
            return Ok(false);
        }

        let spell_cast_due_at = {
            map.lock()
                .await
                .active_db_creature_spell_cast_due_at(attacker)
        };
        if let Some(due_at) = spell_cast_due_at {
            if now < due_at {
                return Ok(false);
            }
            let mut map_guard = map.lock().await;
            if let Some(event) = map_guard.complete_ready_db_creature_spell_cast_with_navigation(
                attacker, victim, now, navigation,
            )? {
                let player_died = matches!(
                    &event.effect,
                    DbCreatureCompletedSpellEffect::PlayerDamage(damage)
                        if damage.victim_health == 0
                );
                let local_effect = match &event.effect {
                    DbCreatureCompletedSpellEffect::PlayerDamage(damage) => {
                        Some(DbCreatureVictimCombatLocalEffect::SpellDamage {
                            victim_health: damage.victim_health,
                            player_died: damage.victim_health == 0,
                        })
                    }
                    _ => None,
                };
                let packets = map_guard
                    .materialize_db_creature_completed_spell_cast_packets(attacker, victim, event);
                Self::split_packets_for_session(current_session_id, packets, tick);
                if let Some(local_effect) = local_effect {
                    tick.local_effects.push(local_effect);
                }
                if player_died {
                    map_guard.clear_db_creature_combats_for_victim(victim);
                    return Ok(true);
                }
                return Ok(false);
            }
        }

        let event_ai_scripts = object_mgr
            .creature_ai_scripts(world_db_pool, active.creature.spawn.entry)
            .await?;
        if !event_ai_scripts.is_empty() {
            let ready = map.lock().await.ready_db_creature_event_ai_spell_cast(
                attacker,
                victim,
                &event_ai_scripts,
                now,
            );
            if let Some(ready) = ready {
                if let Some(template) = object_mgr
                    .spell_template(world_db_pool, ready.spell_id)
                    .await?
                {
                    let spell_range = self.spell_range(template.range_index);
                    let spell_info = SpellInfo::from_template(&template);
                    let mut map_guard = map.lock().await;
                    if map_guard
                        .validate_db_creature_spell_against_target(
                            attacker,
                            ready.target,
                            navigation,
                            spell_range,
                            spell_info.requires_behind_target(),
                        )
                        .check
                        == DbCreatureSpellTargetCheck::Clear
                    {
                        if let Some(cast) = map_guard.prepare_db_creature_spell_cast_from_template(
                            attacker,
                            ready.target,
                            &template,
                            self.spell_duration(template.duration_index),
                            spell_range,
                            self.spell_cast_time(template.casting_time_index),
                            now,
                        ) {
                            let cast_time_millis = cast.cast_time_millis;
                            let target = cast.target;
                            if let Some(start_packets) =
                                map_guard.start_db_creature_spell_cast(cast)?
                            {
                                map_guard.apply_db_creature_event_ai_spell_cooldown(
                                    attacker, &ready, now,
                                );
                                Self::split_packets_for_session(
                                    current_session_id,
                                    start_packets,
                                    tick,
                                );
                                if cast_time_millis == 0 {
                                    if let Some(event) = map_guard
                                        .complete_ready_db_creature_spell_cast_with_navigation(
                                            attacker, target, now, navigation,
                                        )?
                                    {
                                        let player_died = matches!(
                                            &event.effect,
                                            DbCreatureCompletedSpellEffect::PlayerDamage(damage)
                                                if damage.victim_health == 0
                                        );
                                        let local_effect = match &event.effect {
                                            DbCreatureCompletedSpellEffect::PlayerDamage(
                                                damage,
                                            ) => Some(
                                                DbCreatureVictimCombatLocalEffect::SpellDamage {
                                                    victim_health: damage.victim_health,
                                                    player_died: damage.victim_health == 0,
                                                },
                                            ),
                                            _ => None,
                                        };
                                        let packets = map_guard
                                            .materialize_db_creature_completed_spell_cast_packets(
                                                attacker, target, event,
                                            );
                                        Self::split_packets_for_session(
                                            current_session_id,
                                            packets,
                                            tick,
                                        );
                                        if let Some(local_effect) = local_effect {
                                            tick.local_effects.push(local_effect);
                                        }
                                        if player_died {
                                            map_guard.clear_db_creature_combats_for_victim(victim);
                                            return Ok(true);
                                        }
                                    }
                                }
                                return Ok(false);
                            }
                        }
                    }
                }
            }
        }

        let spell_list_id = if active.creature.spawn.template.spell_list != 0 {
            active.creature.spawn.template.spell_list
        } else {
            active.creature.spawn.template.entry.saturating_mul(100)
        };
        if spell_list_id != 0 {
            let spell_list = object_mgr
                .creature_spell_list(world_db_pool, spell_list_id)
                .await?;
            if !spell_list.is_empty() {
                let condition_cache =
                    load_db_creature_spell_condition_cache(object_mgr, world_db_pool, &spell_list)
                        .await?;
                let ready = map.lock().await.ready_db_creature_spell_cast(
                    attacker,
                    victim,
                    &spell_list,
                    &condition_cache,
                    now,
                );
                if let Some(ready) = ready {
                    if let Some(template) = object_mgr
                        .spell_template(world_db_pool, ready.spell.spell_id)
                        .await?
                    {
                        if (template.attributes_ex & SPELL_ATTR_EX_NO_AUTOCAST_AI) == 0
                            && (template.attributes & SPELL_ATTR_PASSIVE) == 0
                        {
                            let spell_range = self.spell_range(template.range_index);
                            let spell_info = SpellInfo::from_template(&template);
                            let mut map_guard = map.lock().await;
                            if map_guard
                                .validate_db_creature_spell_against_target(
                                    attacker,
                                    ready.target,
                                    navigation,
                                    spell_range,
                                    spell_info.requires_behind_target(),
                                )
                                .check
                                == DbCreatureSpellTargetCheck::Clear
                            {
                                let target = ready.target;
                                let aura = (target.is_player()
                                    && spell_info.has_effect(SpellEffectDispatch::ApplyAura))
                                .then(|| {
                                    build_active_aura(
                                        &template,
                                        attacker,
                                        active
                                            .creature
                                            .spawn
                                            .template
                                            .max_level
                                            .max(active.creature.spawn.template.min_level),
                                        SpellEffectValueContext::with_spell_rank_level(
                                            &template,
                                            (active
                                                .creature
                                                .spawn
                                                .template
                                                .max_level
                                                .max(active.creature.spawn.template.min_level)
                                                / 5)
                                                as i32,
                                            0,
                                        ),
                                        now,
                                        self.spell_duration(template.duration_index),
                                    )
                                });
                                let effect = if spell_info.has_direct_damage_effect() {
                                    let damage = spell_info.direct_damage();
                                    if target.is_player() && damage > 0 {
                                        Some(ActiveDbCreatureSpellEffect::Damage {
                                            amount: damage,
                                            school: template.school as u8,
                                            dmg_class: template.dmg_class,
                                            attributes_ex2: template.attributes_ex2,
                                            attributes_ex3: template.attributes_ex3,
                                        })
                                    } else {
                                        None
                                    }
                                } else if spell_info.has_direct_heal_effect() {
                                    let heal = spell_info.direct_heal();
                                    if !target.is_player() && heal > 0 {
                                        Some(ActiveDbCreatureSpellEffect::Heal { amount: heal })
                                    } else {
                                        None
                                    }
                                } else {
                                    None
                                };
                                if let Some(effect) = effect {
                                    let mana_cost = if template.power_type == POWER_TYPE_MANA {
                                        template.mana_cost
                                    } else {
                                        0
                                    };
                                    let cast_time_millis = spell_cast_time_millis(
                                        self.spell_cast_time(template.casting_time_index),
                                    );
                                    let cast = ActiveDbCreatureSpellCast {
                                        caster: attacker,
                                        target,
                                        spell_id: template.id,
                                        requires_behind: spell_info.requires_behind_target(),
                                        effect,
                                        aura,
                                        range: spell_range,
                                        mana_cost,
                                        cast_time_millis,
                                        due_at: now
                                            + Duration::from_millis(cast_time_millis as u64),
                                    };
                                    if let Some(start_packets) =
                                        map_guard.start_db_creature_spell_cast(cast)?
                                    {
                                        map_guard.apply_db_creature_spell_cooldowns(
                                            attacker,
                                            &ready.spell,
                                            &template,
                                            now,
                                        );
                                        Self::split_packets_for_session(
                                            current_session_id,
                                            start_packets,
                                            tick,
                                        );
                                        if cast_time_millis == 0 {
                                            if let Some(event) = map_guard
                                                .complete_ready_db_creature_spell_cast_with_navigation(
                                                    attacker,
                                                    target,
                                                    now,
                                                    navigation,
                                                )?
                                            {
                                                let player_died = matches!(
                                                    &event.effect,
                                                    DbCreatureCompletedSpellEffect::PlayerDamage(
                                                        damage
                                                    ) if damage.victim_health == 0
                                                );
                                                let local_effect = match &event.effect {
                                                    DbCreatureCompletedSpellEffect::PlayerDamage(
                                                        damage,
                                                    ) => Some(
                                                        DbCreatureVictimCombatLocalEffect::SpellDamage {
                                                            victim_health: damage.victim_health,
                                                            player_died: damage.victim_health == 0,
                                                        },
                                                    ),
                                                    _ => None,
                                                };
                                                let packets = map_guard
                                                    .materialize_db_creature_completed_spell_cast_packets(
                                                        attacker,
                                                        target,
                                                        event,
                                                    );
                                                Self::split_packets_for_session(
                                                    current_session_id,
                                                    packets,
                                                    tick,
                                                );
                                                if let Some(local_effect) = local_effect {
                                                    tick.local_effects.push(local_effect);
                                                }
                                                if player_died {
                                                    map_guard
                                                        .clear_db_creature_combats_for_victim(victim);
                                                    return Ok(true);
                                                }
                                            }
                                        }
                                        return Ok(false);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        let can_reach = map
            .lock()
            .await
            .db_creature_can_reach_player_with_navigation(attacker, victim, navigation);
        if !can_reach {
            let mut map_guard = map.lock().await;
            let _ = map_guard.defer_ready_db_creature_swing_retry(attacker, victim, now);
            if let Some((creature, motion)) = map_guard.start_db_creature_chase_motion(
                navigation,
                attacker,
                victim,
                player_position,
                now,
            ) {
                let packet = OutboundWorldPacket {
                    opcode: SMSG_MONSTER_MOVE,
                    body: build_monster_move_facing_target_path_body_with_run(
                        attacker,
                        motion.start,
                        &motion.path,
                        motion.spline_id,
                        motion.duration.as_millis().max(1) as u32,
                        victim,
                        motion.run,
                    )?,
                };
                Self::push_creature_broadcast_packet(
                    &map_guard,
                    victim,
                    current_session_id,
                    creature.current_position,
                    packet,
                    tick,
                );
            }
            return Ok(false);
        }

        if !map
            .lock()
            .await
            .db_creature_has_player_in_arc(attacker, victim)
        {
            let mut map_guard = map.lock().await;
            if let Some((creature, position, spline_id)) =
                map_guard.face_db_creature_toward_position(attacker, player_position)
            {
                let packet = OutboundWorldPacket {
                    opcode: SMSG_MONSTER_MOVE,
                    body: build_monster_move_facing_target_body(
                        attacker, position, position, spline_id, 1, victim,
                    )?,
                };
                Self::push_creature_broadcast_packet(
                    &map_guard,
                    victim,
                    current_session_id,
                    creature.current_position,
                    packet,
                    tick,
                );
            }
            let _ = map_guard.defer_ready_db_creature_swing_retry(attacker, victim, now);
            return Ok(false);
        }

        if now < active.combat.next_swing_at {
            return Ok(false);
        }

        let next_swing_delay = active.creature.base_attack_duration();
        let outcome = active.creature.melee_outcome_against_player(defense);
        let mut map_guard = map.lock().await;
        let Some(event) = map_guard.apply_db_creature_player_melee_outcome(
            attacker,
            victim,
            outcome,
            now,
            now + next_swing_delay,
        )?
        else {
            map_guard.clear_db_creature_combat(attacker);
            return Ok(false);
        };
        tick.local_effects
            .push(DbCreatureVictimCombatLocalEffect::Melee {
                attacker,
                damage_taken: event.damage,
                victim_health: event.victim_health,
                rage_gain: rage_gain_from_damage_taken(event.damage, player_level),
                player_died: event.victim_health == 0,
            });
        for packet in event.direct_packets {
            tick.direct_packets.push(packet);
        }
        if let Some(packet) = event.aura_packet {
            tick.direct_packets.push(packet);
        }
        tick.direct_packets.push(OutboundWorldPacket {
            opcode: SMSG_ATTACKERSTATEUPDATE,
            body: build_attacker_state_update_body_for_outcome(attacker, victim, outcome, 0)?,
        });
        tick.direct_packets.push(OutboundWorldPacket {
            opcode: SMSG_UPDATE_OBJECT,
            body: event.health_update_body,
        });
        tick.observer_packets.extend(event.observer_packets);
        if event.victim_health == 0 {
            map_guard.clear_db_creature_combats_for_victim(victim);
            return Ok(true);
        }
        Ok(false)
    }

    #[allow(dead_code)]
    pub(in crate::world) async fn apply_db_creature_player_damage(
        &self,
        map_id: u32,
        attacker: ObjectGuid,
        victim: ObjectGuid,
        damage: u32,
        now: Instant,
        next_swing_at: Instant,
    ) -> anyhow::Result<Option<DbCreaturePlayerDamageEvent>> {
        let map = self.get_or_create_map(map_id, 0).await;
        let event = map.lock().await.apply_db_creature_player_damage(
            attacker,
            victim,
            damage,
            now,
            next_swing_at,
        );
        event
    }

    #[allow(dead_code)]
    pub(in crate::world) async fn apply_db_creature_player_melee_outcome(
        &self,
        map_id: u32,
        attacker: ObjectGuid,
        victim: ObjectGuid,
        outcome: MeleeDamageOutcome,
        now: Instant,
        next_swing_at: Instant,
    ) -> anyhow::Result<Option<DbCreaturePlayerDamageEvent>> {
        let map = { self.maps.lock().await.get(&(map_id, 0)).cloned() };
        let Some(map) = map else {
            return Ok(None);
        };
        let event = map.lock().await.apply_db_creature_player_melee_outcome(
            attacker,
            victim,
            outcome,
            now,
            next_swing_at,
        );
        event
    }

    #[allow(dead_code)]
    pub(in crate::world) async fn ready_db_creature_spell_cast(
        &self,
        map_id: u32,
        attacker: ObjectGuid,
        victim: ObjectGuid,
        spell_list: &[wow_db::CreatureSpellListQuery],
        conditions: &DbCreatureSpellConditionCache,
        now: Instant,
    ) -> Option<ReadyDbCreatureSpellCast> {
        let map = { self.maps.lock().await.get(&(map_id, 0)).cloned() };
        let map = map?;
        let ready = map
            .lock()
            .await
            .ready_db_creature_spell_cast(attacker, victim, spell_list, conditions, now);
        ready
    }

    #[allow(dead_code)]
    pub(in crate::world) async fn ready_db_creature_event_ai_spell_cast(
        &self,
        map_id: u32,
        attacker: ObjectGuid,
        victim: ObjectGuid,
        scripts: &[wow_db::CreatureAiScriptQuery],
        now: Instant,
    ) -> Option<ReadyDbCreatureEventAiSpellCast> {
        let map = { self.maps.lock().await.get(&(map_id, 0)).cloned() };
        let map = map?;
        let ready = map
            .lock()
            .await
            .ready_db_creature_event_ai_spell_cast(attacker, victim, scripts, now);
        ready
    }

    #[allow(dead_code)]
    pub(in crate::world) async fn prepare_db_creature_spell_cast_from_template(
        &self,
        map_id: u32,
        caster: ObjectGuid,
        target: ObjectGuid,
        template: &wow_db::SpellTemplateQuery,
        now: Instant,
    ) -> Option<ActiveDbCreatureSpellCast> {
        let map = { self.maps.lock().await.get(&(map_id, 0)).cloned() };
        let map = map?;
        let duration = self.spell_duration(template.duration_index);
        let range = self.spell_range(template.range_index);
        let cast_time = self.spell_cast_time(template.casting_time_index);
        let cast = map
            .lock()
            .await
            .prepare_db_creature_spell_cast_from_template(
                caster, target, template, duration, range, cast_time, now,
            );
        cast
    }

    #[allow(dead_code)]
    pub(in crate::world) async fn apply_db_creature_event_ai_spell_cooldown(
        &self,
        map_id: u32,
        attacker: ObjectGuid,
        ready: &ReadyDbCreatureEventAiSpellCast,
        now: Instant,
    ) {
        let map = { self.maps.lock().await.get(&(map_id, 0)).cloned() };
        let Some(map) = map else {
            return;
        };
        map.lock()
            .await
            .apply_db_creature_event_ai_spell_cooldown(attacker, ready, now);
    }

    #[allow(dead_code)]
    pub(in crate::world) async fn start_db_creature_spell_cast(
        &self,
        map_id: u32,
        cast: ActiveDbCreatureSpellCast,
    ) -> anyhow::Result<Option<Vec<(SessionId, OutboundWorldPacket)>>> {
        let map = { self.maps.lock().await.get(&(map_id, 0)).cloned() };
        let Some(map) = map else {
            return Ok(None);
        };
        let event = map.lock().await.start_db_creature_spell_cast(cast);
        event
    }

    #[allow(dead_code)]
    pub(in crate::world) async fn apply_db_creature_spell_cooldowns(
        &self,
        map_id: u32,
        attacker: ObjectGuid,
        spell: &wow_db::CreatureSpellListQuery,
        template: &wow_db::SpellTemplateQuery,
        now: Instant,
    ) {
        let map = { self.maps.lock().await.get(&(map_id, 0)).cloned() };
        let Some(map) = map else {
            return;
        };
        map.lock()
            .await
            .apply_db_creature_spell_cooldowns(attacker, spell, template, now);
    }

    #[allow(dead_code)]
    pub(in crate::world) async fn complete_ready_db_creature_spell_cast(
        &self,
        map_id: u32,
        attacker: ObjectGuid,
        victim: ObjectGuid,
        now: Instant,
        navigation: &DbCreatureNavigationGuardrail,
    ) -> anyhow::Result<Option<DbCreatureCompletedSpellCastEvent>> {
        let map = { self.maps.lock().await.get(&(map_id, 0)).cloned() };
        let Some(map) = map else {
            return Ok(None);
        };
        let event = map
            .lock()
            .await
            .complete_ready_db_creature_spell_cast_with_navigation(
                attacker, victim, now, navigation,
            );
        event
    }

    #[allow(dead_code)]
    pub(in crate::world) async fn active_db_creature_spell_cast_due_at(
        &self,
        map_id: u32,
        attacker: ObjectGuid,
    ) -> Option<Instant> {
        let map = { self.maps.lock().await.get(&(map_id, 0)).cloned() };
        let map = map?;
        let due_at = map
            .lock()
            .await
            .active_db_creature_spell_cast_due_at(attacker);
        due_at
    }

    #[allow(dead_code)]
    pub(in crate::world) async fn db_creature_should_evade(
        &self,
        map_id: u32,
        attacker: ObjectGuid,
        now: Instant,
    ) -> bool {
        let map = { self.maps.lock().await.get(&(map_id, 0)).cloned() };
        let Some(map) = map else {
            return false;
        };
        let should_evade = map.lock().await.db_creature_should_evade(attacker, now);
        should_evade
    }

    #[allow(dead_code)]
    pub(in crate::world) async fn defer_ready_db_creature_swing_retry(
        &self,
        map_id: u32,
        attacker: ObjectGuid,
        victim: ObjectGuid,
        now: Instant,
    ) -> Option<CreatureCombatState> {
        let map = self.get_or_create_map(map_id, 0).await;
        let combat = map
            .lock()
            .await
            .defer_ready_db_creature_swing_retry(attacker, victim, now);
        combat
    }

    pub(in crate::world) async fn advance_db_creature_motion(
        &self,
        map_id: u32,
        creature_guid: ObjectGuid,
        now: Instant,
    ) -> Option<DbCreatureRuntime> {
        let map = self.get_or_create_map(map_id, 0).await;
        let creature = map
            .lock()
            .await
            .advance_db_creature_motion(creature_guid, now)
            .map(|(creature, _, _)| creature);
        creature
    }

    #[allow(dead_code)]
    pub(in crate::world) async fn advance_active_db_creature_idle_motions(
        &self,
        map_id: u32,
        navigation: &DbCreatureNavigationGuardrail,
        now: Instant,
    ) -> anyhow::Result<DbCreatureIdleMotionTick> {
        let map = self.get_or_create_map(map_id, 0).await;
        let tick = map
            .lock()
            .await
            .advance_active_db_creature_idle_motions(navigation, now);
        tick
    }

    #[allow(dead_code)]
    pub(in crate::world) async fn advance_all_active_db_creature_idle_motions(
        &self,
        navigation: &DbCreatureNavigationGuardrail,
        now: Instant,
    ) -> anyhow::Result<DbCreatureIdleMotionTick> {
        self.advance_all_active_db_creature_idle_motions_with_interval(
            navigation,
            now,
            Duration::from_millis(WORLD_TICK_MILLIS),
        )
        .await
    }

    pub(in crate::world) async fn advance_all_active_db_creature_idle_motions_with_interval(
        &self,
        navigation: &DbCreatureNavigationGuardrail,
        now: Instant,
        world_tick_interval: Duration,
    ) -> anyhow::Result<DbCreatureIdleMotionTick> {
        let maps = { self.maps.lock().await.values().cloned().collect::<Vec<_>>() };
        let mut creatures = Vec::new();
        let mut packets = Vec::new();
        for map in maps {
            let tick = map
                .lock()
                .await
                .advance_active_db_creature_idle_motions_with_interval(
                    navigation,
                    now,
                    world_tick_interval,
                )?;
            creatures.extend(tick.creatures);
            packets.extend(tick.packets);
        }
        Ok(DbCreatureIdleMotionTick { creatures, packets })
    }

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

    #[allow(dead_code)]
    pub(in crate::world) async fn db_creature_idle_motion_advancement_guids(
        &self,
        map_id: u32,
        now: Instant,
    ) -> Vec<u64> {
        let map = self.get_or_create_map(map_id, 0).await;
        let guids = map
            .lock()
            .await
            .db_creature_idle_motion_advancement_guids(now)
            .guids;
        guids
    }

    #[allow(dead_code)]
    pub(in crate::world) async fn db_creature_idle_motion_start_guids(
        &self,
        map_id: u32,
        now: Instant,
    ) -> Vec<u64> {
        let map = self.get_or_create_map(map_id, 0).await;
        let guids = map.lock().await.db_creature_idle_motion_start_guids(now);
        guids
    }

    #[allow(dead_code)]
    pub(in crate::world) async fn start_db_creature_idle_motion(
        &self,
        map_id: u32,
        navigation: &DbCreatureNavigationGuardrail,
        creature_guid: ObjectGuid,
        now: Instant,
    ) -> Option<(DbCreatureRuntime, StartedCreatureMotion)> {
        let map = self.get_or_create_map(map_id, 0).await;
        let attempt =
            map.lock()
                .await
                .start_db_creature_idle_motion(navigation, creature_guid, now);
        let (creature, motion, _script_ids) = attempt.outcome?;
        motion.map(|motion| (creature, motion))
    }

    pub(in crate::world) async fn start_db_creature_chase_motion(
        &self,
        map_id: u32,
        navigation: &DbCreatureNavigationGuardrail,
        creature_guid: ObjectGuid,
        target: ObjectGuid,
        target_position: WorldPosition,
        now: Instant,
    ) -> Option<(DbCreatureRuntime, StartedCreatureMotion)> {
        let map = self.get_or_create_map(map_id, 0).await;
        let motion = map.lock().await.start_db_creature_chase_motion(
            navigation,
            creature_guid,
            target,
            target_position,
            now,
        );
        motion
    }

    #[allow(clippy::too_many_arguments)]
    pub(in crate::world) async fn process_db_creature_event_ai_hp_actions(
        &self,
        map_id: u32,
        navigation: &DbCreatureNavigationGuardrail,
        attacker: ObjectGuid,
        victim: ObjectGuid,
        scripts: &[wow_db::CreatureAiScriptQuery],
        now: Instant,
        exclude_character_guid: Option<u32>,
    ) -> anyhow::Result<Option<DbCreatureEventAiActionsEvent>> {
        let map = { self.maps.lock().await.get(&(map_id, 0)).cloned() };
        let Some(map) = map else {
            return Ok(None);
        };
        let event = map.lock().await.process_db_creature_event_ai_hp_actions(
            navigation,
            attacker,
            victim,
            scripts,
            now,
            exclude_character_guid,
        );
        event
    }

    #[allow(dead_code)]
    pub(in crate::world) async fn start_db_creature_return_home_motion(
        &self,
        map_id: u32,
        navigation: &DbCreatureNavigationGuardrail,
        creature_guid: ObjectGuid,
        now: Instant,
    ) -> Option<(DbCreatureRuntime, StartedCreatureMotion)> {
        let map = self.get_or_create_map(map_id, 0).await;
        let motion =
            map.lock()
                .await
                .start_db_creature_return_home_motion(navigation, creature_guid, now);
        motion
    }

    pub(in crate::world) async fn stop_db_creature_motion(
        &self,
        map_id: u32,
        creature_guid: ObjectGuid,
    ) -> Option<(DbCreatureRuntime, StoppedCreatureMotion)> {
        let map = self.get_or_create_map(map_id, 0).await;
        let motion = map.lock().await.stop_db_creature_motion(creature_guid);
        motion
    }

    #[allow(dead_code)]
    pub(in crate::world) async fn face_db_creature_toward_position(
        &self,
        map_id: u32,
        creature_guid: ObjectGuid,
        target_position: WorldPosition,
    ) -> Option<(DbCreatureRuntime, WorldPosition, u32)> {
        let map = self.get_or_create_map(map_id, 0).await;
        let result = map
            .lock()
            .await
            .face_db_creature_toward_position(creature_guid, target_position);
        result
    }

    #[allow(dead_code)]
    pub(in crate::world) async fn prepare_db_creature_evade(
        &self,
        map_id: u32,
        creature_guid: ObjectGuid,
    ) -> Option<DbCreatureRuntime> {
        let map = self.get_or_create_map(map_id, 0).await;
        let creature = map.lock().await.prepare_db_creature_evade(creature_guid);
        creature
    }

    pub(in crate::world) async fn select_db_creature_sight_aggro_targets(
        &self,
        map_id: u32,
        character: &ActiveCharacter,
    ) -> Vec<DbCreatureRuntime> {
        let map = { self.maps.lock().await.get(&(map_id, 0)).cloned() };
        let Some(map) = map else {
            return Vec::new();
        };
        let targets = map.lock().await.select_db_creature_sight_aggro_targets(
            &self.faction_templates,
            character,
            Instant::now(),
        );
        targets
    }

    pub(in crate::world) async fn select_db_creature_assist_targets(
        &self,
        map_id: u32,
        caller_guid: ObjectGuid,
        character: &ActiveCharacter,
    ) -> Option<(DbCreatureRuntime, Vec<ObjectGuid>)> {
        let map = self.get_or_create_map(map_id, 0).await;
        let targets = map.lock().await.select_db_creature_assist_targets(
            &self.faction_templates,
            caller_guid,
            character,
        );
        targets
    }

    pub(in crate::world) async fn get_or_create_map(
        &self,
        map_id: u32,
        instance_id: u32,
    ) -> Arc<Mutex<MapRuntime>> {
        let map_key = (map_id, instance_id);
        let mut maps = self.maps.lock().await;
        maps.entry(map_key)
            .or_insert_with(|| {
                Arc::new(Mutex::new(MapRuntime::with_geometry(
                    map_key.0,
                    map_key.1,
                    self.geometry.clone(),
                    self.db_scripts.clone(),
                )))
            })
            .clone()
    }

    async fn movement_actor_for_map(
        &self,
        map_key: (u32, u32),
        map: Arc<Mutex<MapRuntime>>,
    ) -> Option<MovementActorHandle> {
        if !self.movement_actor_settings.enabled {
            return None;
        }
        let mut actors = self.movement_actors.lock().await;
        Some(
            actors
                .entry(map_key)
                .or_insert_with(|| {
                    MovementActorHandle::spawn_proxy(map, self.movement_actor_settings)
                })
                .clone(),
        )
    }
}

fn pending_spell_event_unit_target_generation(
    map: &MapRuntime,
    kind: &PendingSpellEventKind,
) -> Option<(ObjectGuid, u64)> {
    let target = match kind {
        PendingSpellEventKind::Spell { targets } => targets.unit_target?,
        PendingSpellEventKind::RangedAutoAttack { target, .. } => *target,
    };
    target.is_creature().then_some(target).and_then(|target| {
        map.creatures
            .get(&target.raw())
            .map(|creature| (target, creature.life_generation))
    })
}

pub(in crate::world) fn grid_world_center(grid: GridCoord) -> (f32, f32) {
    let (min_x, max_x, min_y, max_y) = grid_world_bounds(grid);
    ((min_x + max_x) * 0.5, (min_y + max_y) * 0.5)
}

pub(in crate::world) fn player_corpse_grid_query_radius() -> f32 {
    GRID_SIZE_YARDS * std::f32::consts::SQRT_2 * 0.5
}
