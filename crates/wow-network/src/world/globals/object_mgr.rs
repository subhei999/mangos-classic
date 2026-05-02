// CMaNGOS reference: src/game/Globals/ObjectMgr.{h,cpp}
// Shared DB-derived object/template data. This mirrors the CMaNGOS shape where
// immutable world data is loaded through ObjectMgr instead of being re-queried
// from every WorldSession opcode path.

#[derive(Debug, Default)]
struct ObjectMgr {
    quest_templates:
        tokio::sync::Mutex<std::collections::HashMap<u32, Option<wow_db::QuestTemplateQuery>>>,
    creature_start_quest_ids: tokio::sync::Mutex<std::collections::HashMap<u32, Vec<u32>>>,
    creature_complete_quest_ids: tokio::sync::Mutex<std::collections::HashMap<u32, Vec<u32>>>,
    gameobject_start_quest_ids: tokio::sync::Mutex<std::collections::HashMap<u32, Vec<u32>>>,
    gameobject_complete_quest_ids: tokio::sync::Mutex<std::collections::HashMap<u32, Vec<u32>>>,
    quest_prev_quests: tokio::sync::Mutex<std::collections::HashMap<u32, Vec<i32>>>,
    quest_prev_chain_quests: tokio::sync::Mutex<std::collections::HashMap<u32, Vec<u32>>>,
    exclusive_group_quests: tokio::sync::Mutex<std::collections::HashMap<i32, Vec<u32>>>,
    creature_loot_templates:
        tokio::sync::Mutex<std::collections::HashMap<u32, Vec<wow_db::CreatureLootQuery>>>,
    gameobject_loot_templates:
        tokio::sync::Mutex<std::collections::HashMap<u32, Vec<wow_db::CreatureLootQuery>>>,
    stats: ObjectMgrCacheStats,
}

#[derive(Debug, Default)]
struct ObjectMgrCacheStats {
    quest_template_db_loads: std::sync::atomic::AtomicU64,
    quest_relation_db_loads: std::sync::atomic::AtomicU64,
    quest_chain_db_loads: std::sync::atomic::AtomicU64,
    loot_template_db_loads: std::sync::atomic::AtomicU64,
}

#[cfg(test)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct ObjectMgrCacheSnapshot {
    quest_template_db_loads: u64,
    quest_relation_db_loads: u64,
    quest_chain_db_loads: u64,
    loot_template_db_loads: u64,
}

#[derive(Debug, Clone, Copy)]
enum QuestRelationKind {
    CreatureStart,
    CreatureComplete,
    GameObjectStart,
    GameObjectComplete,
}

impl ObjectMgr {
    async fn quest_template(
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

    async fn creature_start_quests(
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

    async fn creature_complete_quests(
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

    async fn gameobject_start_quests(
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

    async fn gameobject_complete_quests(
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

    async fn creature_starts_quest(
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

    async fn creature_completes_quest(
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

    async fn gameobject_starts_quest(
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

    async fn gameobject_completes_quest(
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

    async fn quest_prev_quests(
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

    async fn quest_prev_chain_quests(
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

    async fn exclusive_group_quests(
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

    async fn creature_loot_items(
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

    async fn gameobject_loot_items(
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

    async fn quest_relation_ids(
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

    async fn quest_templates_for_ids(
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
    async fn prime_quest_template_for_test(
        &self,
        quest: u32,
        template: Option<wow_db::QuestTemplateQuery>,
    ) {
        self.quest_templates.lock().await.insert(quest, template);
    }

    #[cfg(test)]
    async fn prime_creature_start_quest_ids_for_test(&self, creature_entry: u32, quests: Vec<u32>) {
        self.creature_start_quest_ids
            .lock()
            .await
            .insert(creature_entry, quests);
    }

    #[cfg(test)]
    async fn prime_creature_complete_quest_ids_for_test(
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
    async fn prime_quest_prev_quests_for_test(&self, quest: u32, prev_quests: Vec<i32>) {
        self.quest_prev_quests
            .lock()
            .await
            .insert(quest, prev_quests);
    }

    #[cfg(test)]
    async fn prime_quest_prev_chain_quests_for_test(&self, quest: u32, prev_chain_quests: Vec<u32>) {
        self.quest_prev_chain_quests
            .lock()
            .await
            .insert(quest, prev_chain_quests);
    }

    #[cfg(test)]
    async fn prime_exclusive_group_quests_for_test(&self, exclusive_group: i32, quests: Vec<u32>) {
        self.exclusive_group_quests
            .lock()
            .await
            .insert(exclusive_group, quests);
    }

    #[cfg(test)]
    async fn prime_creature_loot_template_for_test(
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
    fn cache_stats_snapshot(&self) -> ObjectMgrCacheSnapshot {
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
            loot_template_db_loads: self
                .stats
                .loot_template_db_loads
                .load(std::sync::atomic::Ordering::Relaxed),
        }
    }
}
