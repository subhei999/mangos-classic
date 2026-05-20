use super::*;

impl MapRuntimeManager {
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

    pub(in crate::world) async fn cancel_movement_interrupted_player_spell_cast(
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
                active_cast.interrupt_flags & SPELL_INTERRUPT_FLAG_MOVEMENT != 0
            })
        {
            return None;
        }
        map.active_player_spell_casts.remove(&character_guid)
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

    pub(in crate::world) async fn clear_player_active_spell_runtime(
        &self,
        map_id: u32,
        character_guid: u32,
    ) -> anyhow::Result<Vec<(SessionId, OutboundWorldPacket)>> {
        let map = { self.maps.lock().await.get(&(map_id, 0)).cloned() };
        let Some(map) = map else {
            return Ok(Vec::new());
        };
        let mut map = map.lock().await;
        let direct_session_id = map
            .players
            .get(&character_guid)
            .and_then(PlayerRuntime::client_session_id);
        let cleanup = map.clear_player_active_spell_runtime(character_guid)?;
        let mut packets = cleanup.observer_packets;
        if let Some(direct_session_id) = direct_session_id {
            packets.extend(
                cleanup
                    .direct_packets
                    .into_iter()
                    .map(|packet| (direct_session_id, packet)),
            );
        }
        Ok(packets)
    }

    pub(in crate::world) async fn cancel_movement_interrupted_player_channel(
        &self,
        map_id: u32,
        character_guid: u32,
    ) -> anyhow::Result<Option<PlayerChannelEvent>> {
        let Some(map) = self.maps.lock().await.get(&(map_id, 0)).cloned() else {
            return Ok(None);
        };
        let mut map = map.lock().await;
        let event = if let Some(event) = map.cancel_player_channel_for_movement(character_guid)? {
            Some(event)
        } else {
            map.cancel_player_dynamic_object_channel_for_movement(character_guid)?
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
        if active_cast.interrupt_flags & SPELL_INTERRUPT_FLAG_DAMAGE_CANCELS != 0 {
            return None;
        }
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

    pub(in crate::world) async fn cancel_active_player_spell_cast_for_damage(
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
                active_cast.interrupt_flags & SPELL_INTERRUPT_FLAG_DAMAGE_CANCELS != 0
            })
        {
            return None;
        }
        map.active_player_spell_casts.remove(&character_guid)
    }

    pub(in crate::world) async fn push_pending_spell_event(
        &self,
        map_id: u32,
        caster_character_guid: u32,
        spell_id: u32,
        targets: PendingSpellCastTargets,
        target_outcome: Option<PlayerSpellTargetOutcome>,
        due_at: Instant,
    ) {
        let map = self.get_or_create_map(map_id, 0).await;
        let mut map = map.lock().await;
        let event_id = map.next_spell_event_id;
        map.next_spell_event_id = map.next_spell_event_id.saturating_add(1).max(1);
        let kind = PendingSpellEventKind::Spell {
            targets,
            target_outcome,
        };
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
}

impl MapRuntimeManager {
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
}

impl MapRuntimeManager {
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
}
