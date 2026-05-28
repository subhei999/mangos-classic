use super::*;

impl MapRuntimeManager {
    #[cfg(test)]
    pub(in crate::world) async fn db_creature_return_home_guids(
        &self,
        map_id: u32,
        creature_guids: &[u64],
    ) -> Vec<u64> {
        let map = { self.maps.lock().await.get(&(map_id, 0)).cloned() };
        let Some(map) = map else {
            return Vec::new();
        };
        let guids = map
            .lock()
            .await
            .db_creature_return_home_guids(creature_guids);
        guids
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
}

impl MapRuntimeManager {
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

    pub(in crate::world) async fn apply_db_creature_distract(
        &self,
        map_id: u32,
        creature_guid: ObjectGuid,
        target_position: WorldPosition,
        distract_until: Instant,
    ) -> Option<DbCreatureDistractUpdate> {
        let map = self.get_or_create_map(map_id, 0).await;
        let update = map.lock().await.apply_db_creature_distract(
            creature_guid,
            target_position,
            distract_until,
        );
        update
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
}
