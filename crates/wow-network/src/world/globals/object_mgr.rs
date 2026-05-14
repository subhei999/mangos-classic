use super::*;

// CMaNGOS reference: src/game/Globals/ObjectMgr.{h,cpp}
// Shared DB-derived object/template data. This mirrors the CMaNGOS shape where
// immutable world data is loaded through ObjectMgr instead of being re-queried
// from every WorldSession opcode path.

#[derive(Debug, Default)]
pub(in crate::world) struct ObjectMgr {
    pub(in crate::world) quest_templates:
        tokio::sync::Mutex<std::collections::HashMap<u32, Option<wow_db::QuestTemplateQuery>>>,
    pub(in crate::world) creature_start_quest_ids:
        tokio::sync::Mutex<std::collections::HashMap<u32, Vec<u32>>>,
    pub(in crate::world) creature_complete_quest_ids:
        tokio::sync::Mutex<std::collections::HashMap<u32, Vec<u32>>>,
    pub(in crate::world) gameobject_start_quest_ids:
        tokio::sync::Mutex<std::collections::HashMap<u32, Vec<u32>>>,
    pub(in crate::world) gameobject_complete_quest_ids:
        tokio::sync::Mutex<std::collections::HashMap<u32, Vec<u32>>>,
    pub(in crate::world) gameobject_objective_quest_ids:
        tokio::sync::Mutex<std::collections::HashMap<u32, Vec<u32>>>,
    pub(in crate::world) quest_prev_quests:
        tokio::sync::Mutex<std::collections::HashMap<u32, Vec<i32>>>,
    pub(in crate::world) quest_prev_chain_quests:
        tokio::sync::Mutex<std::collections::HashMap<u32, Vec<u32>>>,
    pub(in crate::world) exclusive_group_quests:
        tokio::sync::Mutex<std::collections::HashMap<i32, Vec<u32>>>,
    pub(in crate::world) condition_entries:
        tokio::sync::Mutex<std::collections::HashMap<u32, Option<wow_db::ConditionQuery>>>,
    pub(in crate::world) unit_conditions:
        tokio::sync::Mutex<std::collections::HashMap<i32, Option<wow_db::UnitConditionQuery>>>,
    pub(in crate::world) combat_conditions:
        tokio::sync::Mutex<std::collections::HashMap<i32, Option<wow_db::CombatConditionQuery>>>,
    pub(in crate::world) game_event_schedules:
        tokio::sync::Mutex<Vec<wow_db::GameEventScheduleQuery>>,
    pub(in crate::world) creature_loot_templates:
        tokio::sync::Mutex<std::collections::HashMap<u32, Vec<wow_db::CreatureLootQuery>>>,
    pub(in crate::world) reference_loot_templates:
        tokio::sync::Mutex<std::collections::HashMap<u32, Vec<wow_db::CreatureLootQuery>>>,
    pub(in crate::world) gameobject_loot_templates:
        tokio::sync::Mutex<std::collections::HashMap<u32, Vec<wow_db::CreatureLootQuery>>>,
    pub(in crate::world) spell_templates:
        tokio::sync::Mutex<std::collections::HashMap<u32, Option<wow_db::SpellTemplateQuery>>>,
    pub(in crate::world) spell_chains:
        tokio::sync::Mutex<std::collections::HashMap<u32, Option<wow_db::SpellChainQuery>>>,
    pub(in crate::world) spell_group_memberships:
        tokio::sync::Mutex<std::collections::HashMap<u32, Vec<wow_db::SpellGroupMembershipQuery>>>,
    pub(in crate::world) creature_spell_lists:
        tokio::sync::Mutex<std::collections::HashMap<u32, Vec<wow_db::CreatureSpellListQuery>>>,
    pub(in crate::world) exploration_base_xp:
        tokio::sync::Mutex<std::collections::HashMap<u8, u32>>,
    pub(in crate::world) stats: ObjectMgrCacheStats,
}

#[derive(Debug, Default)]
pub(in crate::world) struct ObjectMgrCacheStats {
    pub(in crate::world) quest_template_db_loads: std::sync::atomic::AtomicU64,
    pub(in crate::world) quest_relation_db_loads: std::sync::atomic::AtomicU64,
    pub(in crate::world) quest_chain_db_loads: std::sync::atomic::AtomicU64,
    pub(in crate::world) condition_db_loads: std::sync::atomic::AtomicU64,
    pub(in crate::world) unit_condition_db_loads: std::sync::atomic::AtomicU64,
    pub(in crate::world) combat_condition_db_loads: std::sync::atomic::AtomicU64,
    pub(in crate::world) loot_template_db_loads: std::sync::atomic::AtomicU64,
    pub(in crate::world) spell_template_db_loads: std::sync::atomic::AtomicU64,
    pub(in crate::world) spell_chain_db_loads: std::sync::atomic::AtomicU64,
    pub(in crate::world) spell_group_db_loads: std::sync::atomic::AtomicU64,
    pub(in crate::world) creature_spell_list_db_loads: std::sync::atomic::AtomicU64,
    pub(in crate::world) exploration_base_xp_db_loads: std::sync::atomic::AtomicU64,
}

#[cfg(test)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(in crate::world) struct ObjectMgrCacheSnapshot {
    pub(in crate::world) quest_template_db_loads: u64,
    pub(in crate::world) quest_relation_db_loads: u64,
    pub(in crate::world) quest_chain_db_loads: u64,
    pub(in crate::world) condition_db_loads: u64,
    pub(in crate::world) unit_condition_db_loads: u64,
    pub(in crate::world) combat_condition_db_loads: u64,
    pub(in crate::world) loot_template_db_loads: u64,
    pub(in crate::world) spell_template_db_loads: u64,
    pub(in crate::world) spell_chain_db_loads: u64,
    pub(in crate::world) spell_group_db_loads: u64,
    pub(in crate::world) creature_spell_list_db_loads: u64,
    pub(in crate::world) exploration_base_xp_db_loads: u64,
}

#[derive(Debug, Clone, Copy)]
pub(in crate::world) enum QuestRelationKind {
    CreatureStart,
    CreatureComplete,
    GameObjectStart,
    GameObjectComplete,
}

impl ObjectMgr {
    pub(in crate::world) async fn load_conditions(
        &self,
        world_db_pool: &MySqlPool,
    ) -> anyhow::Result<()> {
        let conditions = wow_db::get_conditions(world_db_pool).await?;
        let unit_conditions = wow_db::get_unit_conditions(world_db_pool).await?;
        let combat_conditions = wow_db::get_combat_conditions(world_db_pool).await?;
        self.stats
            .condition_db_loads
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        self.stats
            .unit_condition_db_loads
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        self.stats
            .combat_condition_db_loads
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let mut cache = self.condition_entries.lock().await;
        cache.clear();
        for condition in conditions {
            cache.insert(condition.condition_entry, Some(condition));
        }
        let mut cache = self.unit_conditions.lock().await;
        cache.clear();
        for condition in unit_conditions {
            cache.insert(condition.id, Some(condition));
        }
        let mut cache = self.combat_conditions.lock().await;
        cache.clear();
        for condition in combat_conditions {
            cache.insert(condition.id, Some(condition));
        }
        Ok(())
    }

    pub(in crate::world) async fn set_game_event_schedules(
        &self,
        schedules: Vec<wow_db::GameEventScheduleQuery>,
    ) {
        *self.game_event_schedules.lock().await = schedules;
    }

    pub(in crate::world) async fn active_game_event_state(&self) -> GameEventState {
        let schedules = self.game_event_schedules.lock().await.clone();
        GameEventState::from_schedules_at(&schedules, current_unix_epoch_secs() as i64)
    }

    pub(in crate::world) async fn active_holidays(&self) -> HashSet<u32> {
        let schedules = self.game_event_schedules.lock().await.clone();
        let active_events =
            GameEventState::from_schedules_at(&schedules, current_unix_epoch_secs() as i64);
        schedules
            .iter()
            .filter(|schedule| schedule.holiday != 0 && active_events.is_active(schedule.entry))
            .map(|schedule| schedule.holiday)
            .collect()
    }

    pub(in crate::world) async fn exploration_base_xp(
        &self,
        world_db_pool: &MySqlPool,
        level: u8,
    ) -> anyhow::Result<u32> {
        {
            let cache = self.exploration_base_xp.lock().await;
            if !cache.is_empty() {
                return Ok(cache.get(&level).copied().unwrap_or(0));
            }
        }

        let rows = wow_db::get_exploration_base_xp_rows(world_db_pool).await?;
        self.stats
            .exploration_base_xp_db_loads
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let mut cache = self.exploration_base_xp.lock().await;
        if cache.is_empty() {
            for row in rows {
                cache.insert(row.level, row.basexp);
            }
        }
        Ok(cache.get(&level).copied().unwrap_or(0))
    }

    pub(in crate::world) async fn condition_entry(
        &self,
        world_db_pool: &MySqlPool,
        condition_entry: u32,
    ) -> anyhow::Result<Option<wow_db::ConditionQuery>> {
        {
            let cache = self.condition_entries.lock().await;
            if let Some(condition) = cache.get(&condition_entry) {
                return Ok(condition.clone());
            }
        }

        let condition = wow_db::get_condition(world_db_pool, condition_entry).await?;
        self.stats
            .condition_db_loads
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        self.condition_entries
            .lock()
            .await
            .insert(condition_entry, condition.clone());
        Ok(condition)
    }

    pub(in crate::world) async fn unit_condition(
        &self,
        world_db_pool: &MySqlPool,
        id: i32,
    ) -> anyhow::Result<Option<wow_db::UnitConditionQuery>> {
        {
            let cache = self.unit_conditions.lock().await;
            if let Some(condition) = cache.get(&id) {
                return Ok(condition.clone());
            }
        }

        let condition = wow_db::get_unit_condition(world_db_pool, id).await?;
        self.stats
            .unit_condition_db_loads
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        self.unit_conditions
            .lock()
            .await
            .insert(id, condition.clone());
        Ok(condition)
    }

    pub(in crate::world) async fn combat_condition(
        &self,
        world_db_pool: &MySqlPool,
        id: i32,
    ) -> anyhow::Result<Option<wow_db::CombatConditionQuery>> {
        {
            let cache = self.combat_conditions.lock().await;
            if let Some(condition) = cache.get(&id) {
                return Ok(condition.clone());
            }
        }

        let condition = wow_db::get_combat_condition(world_db_pool, id).await?;
        self.stats
            .combat_condition_db_loads
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        self.combat_conditions
            .lock()
            .await
            .insert(id, condition.clone());
        Ok(condition)
    }

    pub(in crate::world) async fn quest_template(
        &self,
        world_db_pool: &MySqlPool,
        quest: u32,
    ) -> anyhow::Result<Option<wow_db::QuestTemplateQuery>> {
        let mut cache = self.quest_templates.lock().await;
        if let Some(template) = cache.get(&quest) {
            return Ok(template.clone());
        }

        let template = wow_db::get_quest_template_query(world_db_pool, quest).await?;
        self.stats
            .quest_template_db_loads
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        cache.insert(quest, template.clone());
        Ok(template)
    }

    pub(in crate::world) async fn creature_start_quests(
        &self,
        world_db_pool: &MySqlPool,
        creature_entry: u32,
    ) -> anyhow::Result<Vec<wow_db::QuestTemplateQuery>> {
        let quest_ids = self
            .quest_relation_ids(
                world_db_pool,
                QuestRelationKind::CreatureStart,
                creature_entry,
            )
            .await?;
        self.quest_templates_for_ids(world_db_pool, quest_ids).await
    }

    pub(in crate::world) async fn creature_complete_quests(
        &self,
        world_db_pool: &MySqlPool,
        creature_entry: u32,
    ) -> anyhow::Result<Vec<wow_db::QuestTemplateQuery>> {
        let quest_ids = self
            .quest_relation_ids(
                world_db_pool,
                QuestRelationKind::CreatureComplete,
                creature_entry,
            )
            .await?;
        self.quest_templates_for_ids(world_db_pool, quest_ids).await
    }

    pub(in crate::world) async fn gameobject_start_quests(
        &self,
        world_db_pool: &MySqlPool,
        gameobject_entry: u32,
    ) -> anyhow::Result<Vec<wow_db::QuestTemplateQuery>> {
        let quest_ids = self
            .quest_relation_ids(
                world_db_pool,
                QuestRelationKind::GameObjectStart,
                gameobject_entry,
            )
            .await?;
        self.quest_templates_for_ids(world_db_pool, quest_ids).await
    }

    pub(in crate::world) async fn gameobject_complete_quests(
        &self,
        world_db_pool: &MySqlPool,
        gameobject_entry: u32,
    ) -> anyhow::Result<Vec<wow_db::QuestTemplateQuery>> {
        let quest_ids = self
            .quest_relation_ids(
                world_db_pool,
                QuestRelationKind::GameObjectComplete,
                gameobject_entry,
            )
            .await?;
        self.quest_templates_for_ids(world_db_pool, quest_ids).await
    }

    pub(in crate::world) async fn gameobject_objective_quests(
        &self,
        world_db_pool: &MySqlPool,
        gameobject_entry: u32,
    ) -> anyhow::Result<Vec<wow_db::QuestTemplateQuery>> {
        let quest_ids = {
            let cache = self.gameobject_objective_quest_ids.lock().await;
            cache.get(&gameobject_entry).cloned()
        };
        let quest_ids = if let Some(quest_ids) = quest_ids {
            quest_ids
        } else {
            let quest_ids =
                wow_db::get_gameobject_objective_quest_ids(world_db_pool, gameobject_entry).await?;
            self.stats
                .quest_relation_db_loads
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            self.gameobject_objective_quest_ids
                .lock()
                .await
                .insert(gameobject_entry, quest_ids.clone());
            quest_ids
        };
        self.quest_templates_for_ids(world_db_pool, quest_ids).await
    }

    pub(in crate::world) async fn creature_starts_quest(
        &self,
        world_db_pool: &MySqlPool,
        creature_entry: u32,
        quest: u32,
    ) -> anyhow::Result<bool> {
        Ok(self
            .quest_relation_ids(
                world_db_pool,
                QuestRelationKind::CreatureStart,
                creature_entry,
            )
            .await?
            .contains(&quest))
    }

    pub(in crate::world) async fn creature_completes_quest(
        &self,
        world_db_pool: &MySqlPool,
        creature_entry: u32,
        quest: u32,
    ) -> anyhow::Result<bool> {
        Ok(self
            .quest_relation_ids(
                world_db_pool,
                QuestRelationKind::CreatureComplete,
                creature_entry,
            )
            .await?
            .contains(&quest))
    }

    pub(in crate::world) async fn gameobject_starts_quest(
        &self,
        world_db_pool: &MySqlPool,
        gameobject_entry: u32,
        quest: u32,
    ) -> anyhow::Result<bool> {
        Ok(self
            .quest_relation_ids(
                world_db_pool,
                QuestRelationKind::GameObjectStart,
                gameobject_entry,
            )
            .await?
            .contains(&quest))
    }

    pub(in crate::world) async fn gameobject_completes_quest(
        &self,
        world_db_pool: &MySqlPool,
        gameobject_entry: u32,
        quest: u32,
    ) -> anyhow::Result<bool> {
        Ok(self
            .quest_relation_ids(
                world_db_pool,
                QuestRelationKind::GameObjectComplete,
                gameobject_entry,
            )
            .await?
            .contains(&quest))
    }

    pub(in crate::world) async fn quest_prev_quests(
        &self,
        world_db_pool: &MySqlPool,
        quest: u32,
    ) -> anyhow::Result<Vec<i32>> {
        let mut cache = self.quest_prev_quests.lock().await;
        if let Some(quests) = cache.get(&quest) {
            return Ok(quests.clone());
        }

        let quests = wow_db::get_quest_prev_quests(world_db_pool, quest).await?;
        self.stats
            .quest_chain_db_loads
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        cache.insert(quest, quests.clone());
        Ok(quests)
    }

    pub(in crate::world) async fn quest_prev_chain_quests(
        &self,
        world_db_pool: &MySqlPool,
        quest: u32,
    ) -> anyhow::Result<Vec<u32>> {
        let mut cache = self.quest_prev_chain_quests.lock().await;
        if let Some(quests) = cache.get(&quest) {
            return Ok(quests.clone());
        }

        let quests = wow_db::get_quest_prev_chain_quests(world_db_pool, quest).await?;
        self.stats
            .quest_chain_db_loads
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        cache.insert(quest, quests.clone());
        Ok(quests)
    }

    pub(in crate::world) async fn exclusive_group_quests(
        &self,
        world_db_pool: &MySqlPool,
        exclusive_group: i32,
    ) -> anyhow::Result<Vec<u32>> {
        let mut cache = self.exclusive_group_quests.lock().await;
        if let Some(quests) = cache.get(&exclusive_group) {
            return Ok(quests.clone());
        }

        let quests = wow_db::get_exclusive_group_quests(world_db_pool, exclusive_group).await?;
        self.stats
            .quest_chain_db_loads
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        cache.insert(exclusive_group, quests.clone());
        Ok(quests)
    }

    pub(in crate::world) async fn creature_loot_items(
        &self,
        world_db_pool: &MySqlPool,
        creature_entry: u32,
    ) -> anyhow::Result<Vec<wow_db::CreatureLootQuery>> {
        let mut cache = self.creature_loot_templates.lock().await;
        if let Some(rows) = cache.get(&creature_entry) {
            return Ok(rows.clone());
        }

        let rows = wow_db::get_creature_loot_items(world_db_pool, creature_entry).await?;
        self.stats
            .loot_template_db_loads
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        cache.insert(creature_entry, rows.clone());
        Ok(rows)
    }

    pub(in crate::world) async fn gameobject_loot_items(
        &self,
        world_db_pool: &MySqlPool,
        loot_entry: u32,
    ) -> anyhow::Result<Vec<wow_db::CreatureLootQuery>> {
        let mut cache = self.gameobject_loot_templates.lock().await;
        if let Some(rows) = cache.get(&loot_entry) {
            return Ok(rows.clone());
        }

        let rows = wow_db::get_gameobject_loot_items(world_db_pool, loot_entry).await?;
        self.stats
            .loot_template_db_loads
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        cache.insert(loot_entry, rows.clone());
        Ok(rows)
    }

    pub(in crate::world) async fn reference_loot_items(
        &self,
        world_db_pool: &MySqlPool,
        reference_entry: u32,
    ) -> anyhow::Result<Vec<wow_db::CreatureLootQuery>> {
        let mut cache = self.reference_loot_templates.lock().await;
        if let Some(rows) = cache.get(&reference_entry) {
            return Ok(rows.clone());
        }

        let rows = wow_db::get_reference_loot_items(world_db_pool, reference_entry).await?;
        self.stats
            .loot_template_db_loads
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        cache.insert(reference_entry, rows.clone());
        Ok(rows)
    }

    pub(in crate::world) async fn spell_template(
        &self,
        world_db_pool: &MySqlPool,
        spell: u32,
    ) -> anyhow::Result<Option<wow_db::SpellTemplateQuery>> {
        let mut cache = self.spell_templates.lock().await;
        if let Some(template) = cache.get(&spell) {
            return Ok(template.clone());
        }

        let template = wow_db::get_spell_template_query(world_db_pool, spell).await?;
        self.stats
            .spell_template_db_loads
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        cache.insert(spell, template.clone());
        Ok(template)
    }

    pub(in crate::world) async fn spell_chain(
        &self,
        world_db_pool: &MySqlPool,
        spell: u32,
    ) -> anyhow::Result<Option<wow_db::SpellChainQuery>> {
        let mut cache = self.spell_chains.lock().await;
        if let Some(chain) = cache.get(&spell) {
            return Ok(*chain);
        }

        let chain = wow_db::get_spell_chain_query(world_db_pool, spell).await?;
        self.stats
            .spell_chain_db_loads
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        cache.insert(spell, chain);
        Ok(chain)
    }

    pub(in crate::world) async fn spell_group_memberships(
        &self,
        world_db_pool: &MySqlPool,
        spell: u32,
    ) -> anyhow::Result<Vec<wow_db::SpellGroupMembershipQuery>> {
        {
            let cache = self.spell_group_memberships.lock().await;
            if let Some(memberships) = cache.get(&spell) {
                return Ok(memberships.clone());
            }
        }

        let memberships = wow_db::get_spell_group_memberships(world_db_pool, spell).await?;
        self.stats
            .spell_group_db_loads
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        self.spell_group_memberships
            .lock()
            .await
            .insert(spell, memberships.clone());
        Ok(memberships)
    }

    pub(in crate::world) async fn creature_spell_list(
        &self,
        world_db_pool: &MySqlPool,
        list_id: u32,
    ) -> anyhow::Result<Vec<wow_db::CreatureSpellListQuery>> {
        {
            let cache = self.creature_spell_lists.lock().await;
            if let Some(list) = cache.get(&list_id) {
                return Ok(list.clone());
            }
        }

        let list = wow_db::get_creature_spell_list(world_db_pool, list_id).await?;
        self.stats
            .creature_spell_list_db_loads
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        self.creature_spell_lists
            .lock()
            .await
            .insert(list_id, list.clone());
        Ok(list)
    }

    pub(in crate::world) async fn quest_relation_ids(
        &self,
        world_db_pool: &MySqlPool,
        kind: QuestRelationKind,
        entry: u32,
    ) -> anyhow::Result<Vec<u32>> {
        let cache = match kind {
            QuestRelationKind::CreatureStart => &self.creature_start_quest_ids,
            QuestRelationKind::CreatureComplete => &self.creature_complete_quest_ids,
            QuestRelationKind::GameObjectStart => &self.gameobject_start_quest_ids,
            QuestRelationKind::GameObjectComplete => &self.gameobject_complete_quest_ids,
        };
        let mut cache = cache.lock().await;
        if let Some(quests) = cache.get(&entry) {
            return Ok(quests.clone());
        }

        let query = match kind {
            QuestRelationKind::CreatureStart => {
                "SELECT quest FROM creature_questrelation WHERE id = ? ORDER BY quest"
            }
            QuestRelationKind::CreatureComplete => {
                "SELECT quest FROM creature_involvedrelation WHERE id = ? ORDER BY quest"
            }
            QuestRelationKind::GameObjectStart => {
                "SELECT quest FROM gameobject_questrelation WHERE id = ? ORDER BY quest"
            }
            QuestRelationKind::GameObjectComplete => {
                "SELECT quest FROM gameobject_involvedrelation WHERE id = ? ORDER BY quest"
            }
        };
        let quests: Vec<u32> = sqlx::query_scalar(query)
            .bind(entry)
            .fetch_all(world_db_pool)
            .await?;
        self.stats
            .quest_relation_db_loads
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        cache.insert(entry, quests.clone());
        Ok(quests)
    }

    pub(in crate::world) async fn quest_templates_for_ids(
        &self,
        world_db_pool: &MySqlPool,
        quest_ids: Vec<u32>,
    ) -> anyhow::Result<Vec<wow_db::QuestTemplateQuery>> {
        let mut quests = Vec::with_capacity(quest_ids.len());
        for quest_id in quest_ids {
            if let Some(template) = self.quest_template(world_db_pool, quest_id).await? {
                quests.push(template);
            }
        }
        Ok(quests)
    }

    #[cfg(test)]
    pub(in crate::world) async fn prime_quest_template_for_test(
        &self,
        quest: u32,
        template: Option<wow_db::QuestTemplateQuery>,
    ) {
        self.quest_templates.lock().await.insert(quest, template);
    }

    #[cfg(test)]
    pub(in crate::world) async fn prime_creature_start_quest_ids_for_test(
        &self,
        creature_entry: u32,
        quests: Vec<u32>,
    ) {
        self.creature_start_quest_ids
            .lock()
            .await
            .insert(creature_entry, quests);
    }

    #[cfg(test)]
    pub(in crate::world) async fn prime_creature_complete_quest_ids_for_test(
        &self,
        creature_entry: u32,
        quests: Vec<u32>,
    ) {
        self.creature_complete_quest_ids
            .lock()
            .await
            .insert(creature_entry, quests);
    }

    #[cfg(test)]
    pub(in crate::world) async fn prime_quest_prev_quests_for_test(
        &self,
        quest: u32,
        prev_quests: Vec<i32>,
    ) {
        self.quest_prev_quests
            .lock()
            .await
            .insert(quest, prev_quests);
    }

    #[cfg(test)]
    pub(in crate::world) async fn prime_quest_prev_chain_quests_for_test(
        &self,
        quest: u32,
        prev_chain_quests: Vec<u32>,
    ) {
        self.quest_prev_chain_quests
            .lock()
            .await
            .insert(quest, prev_chain_quests);
    }

    #[cfg(test)]
    pub(in crate::world) async fn prime_exclusive_group_quests_for_test(
        &self,
        exclusive_group: i32,
        quests: Vec<u32>,
    ) {
        self.exclusive_group_quests
            .lock()
            .await
            .insert(exclusive_group, quests);
    }

    #[cfg(test)]
    pub(in crate::world) async fn prime_condition_for_test(
        &self,
        condition_entry: u32,
        condition: Option<wow_db::ConditionQuery>,
    ) {
        self.condition_entries
            .lock()
            .await
            .insert(condition_entry, condition);
    }

    #[cfg(test)]
    pub(in crate::world) async fn prime_game_event_schedules_for_test(
        &self,
        schedules: Vec<wow_db::GameEventScheduleQuery>,
    ) {
        self.set_game_event_schedules(schedules).await;
    }

    #[cfg(test)]
    pub(in crate::world) async fn prime_creature_loot_template_for_test(
        &self,
        creature_entry: u32,
        rows: Vec<wow_db::CreatureLootQuery>,
    ) {
        self.creature_loot_templates
            .lock()
            .await
            .insert(creature_entry, rows);
    }

    #[cfg(test)]
    pub(in crate::world) async fn prime_spell_template_for_test(
        &self,
        spell: u32,
        template: Option<wow_db::SpellTemplateQuery>,
    ) {
        self.spell_templates.lock().await.insert(spell, template);
    }

    #[cfg(test)]
    pub(in crate::world) async fn prime_spell_chain_for_test(
        &self,
        spell: u32,
        chain: Option<wow_db::SpellChainQuery>,
    ) {
        self.spell_chains.lock().await.insert(spell, chain);
    }

    #[cfg(test)]
    pub(in crate::world) async fn prime_spell_group_memberships_for_test(
        &self,
        spell: u32,
        memberships: Vec<wow_db::SpellGroupMembershipQuery>,
    ) {
        self.spell_group_memberships
            .lock()
            .await
            .insert(spell, memberships);
    }

    #[cfg(test)]
    pub(in crate::world) fn cache_stats_snapshot(&self) -> ObjectMgrCacheSnapshot {
        ObjectMgrCacheSnapshot {
            quest_template_db_loads: self
                .stats
                .quest_template_db_loads
                .load(std::sync::atomic::Ordering::Relaxed),
            quest_relation_db_loads: self
                .stats
                .quest_relation_db_loads
                .load(std::sync::atomic::Ordering::Relaxed),
            quest_chain_db_loads: self
                .stats
                .quest_chain_db_loads
                .load(std::sync::atomic::Ordering::Relaxed),
            condition_db_loads: self
                .stats
                .condition_db_loads
                .load(std::sync::atomic::Ordering::Relaxed),
            unit_condition_db_loads: self
                .stats
                .unit_condition_db_loads
                .load(std::sync::atomic::Ordering::Relaxed),
            combat_condition_db_loads: self
                .stats
                .combat_condition_db_loads
                .load(std::sync::atomic::Ordering::Relaxed),
            loot_template_db_loads: self
                .stats
                .loot_template_db_loads
                .load(std::sync::atomic::Ordering::Relaxed),
            spell_template_db_loads: self
                .stats
                .spell_template_db_loads
                .load(std::sync::atomic::Ordering::Relaxed),
            spell_chain_db_loads: self
                .stats
                .spell_chain_db_loads
                .load(std::sync::atomic::Ordering::Relaxed),
            spell_group_db_loads: self
                .stats
                .spell_group_db_loads
                .load(std::sync::atomic::Ordering::Relaxed),
            creature_spell_list_db_loads: self
                .stats
                .creature_spell_list_db_loads
                .load(std::sync::atomic::Ordering::Relaxed),
            exploration_base_xp_db_loads: self
                .stats
                .exploration_base_xp_db_loads
                .load(std::sync::atomic::Ordering::Relaxed),
        }
    }
}
