use super::*;

impl MapRuntimeManager {
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
}
