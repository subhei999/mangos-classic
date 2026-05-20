use super::*;

impl MapRuntimeManager {
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
    ) -> Option<Vec<(SessionId, OutboundWorldPacket)>> {
        let map = { self.maps.lock().await.get(&(map_id, 0)).cloned() };
        let map = map?;
        let packets = map
            .lock()
            .await
            .disconnect_player_for_linger(character_guid, now);
        packets
    }

    pub(in crate::world) async fn expire_all_disconnected_players(
        &self,
        now: Instant,
    ) -> Vec<ExpiredDisconnectedPlayer> {
        let maps = { self.maps.lock().await.values().cloned().collect::<Vec<_>>() };
        let mut expired = Vec::new();
        for map in maps {
            expired.extend(map.lock().await.expire_disconnected_players(now));
        }
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

    pub(in crate::world) async fn player_runtime_session_snapshot(
        &self,
        map_id: u32,
        character_guid: u32,
    ) -> Option<PlayerRuntimeSessionSnapshot> {
        let map = { self.maps.lock().await.get(&(map_id, 0)).cloned() };
        let map = map?;
        let snapshot = map
            .lock()
            .await
            .player_runtime_session_snapshot(character_guid);
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
}

impl MapRuntimeManager {
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
}

impl MapRuntimeManager {
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
}
