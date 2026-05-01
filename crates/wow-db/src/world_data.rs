use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use sqlx::mysql::MySqlPool;
use sqlx::{FromRow, QueryBuilder};

use crate::pool::DbError;

#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct CreatureTemplateQuery {
    pub entry: u32,
    pub name: String,
    pub subname: Option<String>,
    pub min_level: u8,
    pub max_level: u8,
    pub display_id1: u32,
    pub display_id2: u32,
    pub display_id3: u32,
    pub display_id4: u32,
    pub model_bounding_radius: f32,
    pub model_combat_reach: f32,
    pub faction: u32,
    pub scale: f32,
    pub detection_range: u32,
    pub call_for_help: u32,
    pub family: i32,
    pub creature_type: u32,
    pub npc_flags: u32,
    pub unit_flags: u32,
    pub dynamic_flags: u32,
    pub unit_class: u8,
    pub rank: u32,
    pub health_multiplier: f32,
    pub power_multiplier: f32,
    pub damage_multiplier: f32,
    pub damage_variance: f32,
    pub armor_multiplier: f32,
    pub min_level_health: u32,
    pub max_level_health: u32,
    pub min_level_mana: u32,
    pub max_level_mana: u32,
    pub min_melee_dmg: f32,
    pub max_melee_dmg: f32,
    pub min_ranged_dmg: f32,
    pub max_ranged_dmg: f32,
    pub armor: u32,
    pub melee_attack_power: u32,
    pub ranged_attack_power: u32,
    pub min_loot_gold: u32,
    pub max_loot_gold: u32,
    pub melee_base_attack_time: u32,
    pub ranged_base_attack_time: u32,
    pub damage_school: i8,
    pub trainer_type: i8,
    pub trainer_class: u8,
    pub pet_spell_data_id: u32,
    pub civilian: u8,
    pub corpse_decay: u32,
    pub movement_type: u8,
    pub experience_multiplier: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreatureSpawnQuery {
    pub guid: u32,
    pub entry: u32,
    pub map: u32,
    pub position_x: f32,
    pub position_y: f32,
    pub position_z: f32,
    pub orientation: f32,
    pub spawn_time_secs_min: u32,
    pub spawn_time_secs_max: u32,
    pub spawn_dist: f32,
    pub movement_type: u8,
    pub formation_waypoint_path_id: Option<u32>,
    pub template: CreatureTemplateQuery,
    pub waypoint_path: Vec<CreatureWaypointQuery>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CreatureWaypointQuery {
    pub point: u32,
    pub position_x: f32,
    pub position_y: f32,
    pub position_z: f32,
    pub orientation: Option<f32>,
    pub wait_time: u32,
    pub script_id: u32,
}

#[derive(Debug, Clone, FromRow, Serialize, Deserialize, PartialEq)]
pub struct GraveyardQuery {
    pub id: u32,
    pub map: u32,
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub o: f32,
    pub name: String,
}

#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct VendorItemQuery {
    pub item: u32,
    pub max_count: u32,
    pub slot: u8,
    pub display_id: u32,
    pub buy_price: u32,
    pub max_durability: u32,
    pub buy_count: u32,
    pub container_slots: u32,
}

#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct CreatureLootQuery {
    pub item: u32,
    pub min_count: u32,
    pub max_count: u32,
    pub display_id: u32,
}

#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct TrainerSpellQuery {
    pub spell: u32,
    pub learned_spell: u32,
    pub spell_cost: u32,
    pub req_skill: u32,
    pub req_skill_value: u32,
    pub req_level: u8,
    pub req_ability1: Option<u32>,
    pub req_ability2: Option<u32>,
    pub req_ability3: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuestTemplateQuery {
    pub entry: u32,
    pub method: u32,
    pub zone_or_sort: i16,
    pub quest_level: u32,
    pub quest_type: u32,
    pub rep_objective_faction: u32,
    pub rep_objective_value: i32,
    pub next_quest_in_chain: u32,
    pub rew_or_req_money: i32,
    pub rew_money_max_level: u32,
    pub rew_spell: u32,
    pub rew_spell_cast: u32,
    pub src_item_id: u32,
    pub quest_flags: u32,
    pub title: String,
    pub details: String,
    pub objectives: String,
    pub offer_reward_text: String,
    pub request_items_text: String,
    pub end_text: String,
    pub req_creature_or_go_id: [i32; 4],
    pub req_creature_or_go_count: [u32; 4],
    pub req_item_id: [u32; 4],
    pub req_item_count: [u32; 4],
    pub rew_choice_item_id: [u32; 6],
    pub rew_choice_item_count: [u32; 6],
    pub rew_item_id: [u32; 4],
    pub rew_item_count: [u32; 4],
    pub point_map_id: u32,
    pub point_x: f32,
    pub point_y: f32,
    pub point_opt: u32,
    pub details_emote: [u32; 4],
    pub details_emote_delay: [u32; 4],
    pub complete_emote: u32,
    pub complete_emote_delay: u32,
    pub incomplete_emote: u32,
    pub incomplete_emote_delay: u32,
    pub offer_reward_emote: [u32; 4],
    pub offer_reward_emote_delay: [u32; 4],
    pub objective_text: [String; 4],
}

impl QuestTemplateQuery {
    pub fn required_creature_index(&self, creature_entry: u32) -> Option<usize> {
        self.req_creature_or_go_id
            .iter()
            .position(|entry| *entry > 0 && *entry as u32 == creature_entry)
    }

    pub fn required_creature_count(&self, index: usize) -> u32 {
        self.req_creature_or_go_count[index]
    }
}

pub async fn get_creature_template_query(
    pool: &MySqlPool,
    entry: u32,
) -> Result<Option<CreatureTemplateQuery>, DbError> {
    let row = sqlx::query_as::<_, CreatureTemplateQuery>(
        "SELECT Entry AS entry, Name AS name, SubName AS subname, \
                MinLevel AS min_level, MaxLevel AS max_level, \
                DisplayId1 AS display_id1, DisplayId2 AS display_id2, \
                DisplayId3 AS display_id3, DisplayId4 AS display_id4, \
                COALESCE(creature_model_info.bounding_radius, 0) AS model_bounding_radius, \
                COALESCE(creature_model_info.combat_reach, 0) AS model_combat_reach, \
                Faction AS faction, Scale AS scale, Detection AS detection_range, \
                CallForHelp AS call_for_help, Family AS family, \
                CreatureType AS creature_type, NpcFlags AS npc_flags, \
                UnitFlags AS unit_flags, DynamicFlags AS dynamic_flags, \
                UnitClass AS unit_class, Rank AS rank, \
                HealthMultiplier AS health_multiplier, PowerMultiplier AS power_multiplier, \
                DamageMultiplier AS damage_multiplier, DamageVariance AS damage_variance, \
                ArmorMultiplier AS armor_multiplier, \
                MinLevelHealth AS min_level_health, MaxLevelHealth AS max_level_health, \
                MinLevelMana AS min_level_mana, MaxLevelMana AS max_level_mana, \
                MinMeleeDmg AS min_melee_dmg, MaxMeleeDmg AS max_melee_dmg, \
                MinRangedDmg AS min_ranged_dmg, MaxRangedDmg AS max_ranged_dmg, \
                Armor AS armor, MeleeAttackPower AS melee_attack_power, \
                RangedAttackPower AS ranged_attack_power, \
                MinLootGold AS min_loot_gold, MaxLootGold AS max_loot_gold, \
                MeleeBaseAttackTime AS melee_base_attack_time, \
                RangedBaseAttackTime AS ranged_base_attack_time, \
                DamageSchool AS damage_school, \
                TrainerType AS trainer_type, TrainerClass AS trainer_class, \
                PetSpellDataId AS pet_spell_data_id, Civilian AS civilian, \
                CorpseDecay AS corpse_decay, \
                MovementType AS movement_type, \
                ExperienceMultiplier AS experience_multiplier \
         FROM creature_template \
         LEFT JOIN creature_model_info \
           ON creature_model_info.modelid = COALESCE(NULLIF(DisplayId1, 0), NULLIF(DisplayId2, 0), NULLIF(DisplayId3, 0), NULLIF(DisplayId4, 0), 0) \
         WHERE Entry = ?",
    )
    .bind(entry)
    .fetch_optional(pool)
    .await?;

    Ok(row)
}

pub async fn get_creature_loot_items(
    pool: &MySqlPool,
    creature_entry: u32,
) -> Result<Vec<CreatureLootQuery>, DbError> {
    let rows = sqlx::query_as::<_, CreatureLootRow>(
        "SELECT creature_loot_template.item, \
                CAST(GREATEST(creature_loot_template.mincountOrRef, 1) AS UNSIGNED) AS min_count, \
                CAST(GREATEST(creature_loot_template.maxcount, creature_loot_template.mincountOrRef, 1) AS UNSIGNED) AS max_count, \
                item_template.displayid AS display_id \
         FROM creature_loot_template \
         JOIN item_template ON creature_loot_template.item = item_template.entry \
         WHERE creature_loot_template.entry = ? \
           AND creature_loot_template.condition_id = 0 \
           AND creature_loot_template.ChanceOrQuestChance > 0 \
           AND creature_loot_template.groupid = 0 \
           AND creature_loot_template.mincountOrRef > 0 \
         ORDER BY creature_loot_template.item \
         LIMIT 1",
    )
    .bind(creature_entry)
    .fetch_all(pool)
    .await?;

    Ok(rows.into_iter().map(CreatureLootRow::into_query).collect())
}

pub async fn get_quest_template_query(
    pool: &MySqlPool,
    quest: u32,
) -> Result<Option<QuestTemplateQuery>, DbError> {
    let row = sqlx::query_as::<_, QuestTemplateRow>(
        "SELECT CAST(entry AS UNSIGNED) AS entry, CAST(Method AS UNSIGNED) AS method, \
                ZoneOrSort AS zone_or_sort, \
                CAST(QuestLevel AS UNSIGNED) AS quest_level, CAST(Type AS UNSIGNED) AS quest_type, \
                CAST(RepObjectiveFaction AS UNSIGNED) AS rep_objective_faction, \
                RepObjectiveValue AS rep_objective_value, \
                CAST(NextQuestInChain AS UNSIGNED) AS next_quest_in_chain, \
                RewOrReqMoney AS rew_or_req_money, RewMoneyMaxLevel AS rew_money_max_level, \
                CAST(RewSpell AS UNSIGNED) AS rew_spell, CAST(RewSpellCast AS UNSIGNED) AS rew_spell_cast, \
                CAST(SrcItemId AS UNSIGNED) AS src_item_id, CAST(QuestFlags AS UNSIGNED) AS quest_flags, \
                COALESCE(Title, '') AS title, COALESCE(Details, '') AS details, \
                COALESCE(Objectives, '') AS objectives, \
                COALESCE(OfferRewardText, '') AS offer_reward_text, \
                COALESCE(RequestItemsText, '') AS request_items_text, \
                COALESCE(EndText, '') AS end_text, \
                ReqCreatureOrGOId1 AS req_creature_or_go_id1, \
                ReqCreatureOrGOId2 AS req_creature_or_go_id2, \
                ReqCreatureOrGOId3 AS req_creature_or_go_id3, \
                ReqCreatureOrGOId4 AS req_creature_or_go_id4, \
                CAST(ReqCreatureOrGOCount1 AS UNSIGNED) AS req_creature_or_go_count1, \
                CAST(ReqCreatureOrGOCount2 AS UNSIGNED) AS req_creature_or_go_count2, \
                CAST(ReqCreatureOrGOCount3 AS UNSIGNED) AS req_creature_or_go_count3, \
                CAST(ReqCreatureOrGOCount4 AS UNSIGNED) AS req_creature_or_go_count4, \
                CAST(ReqItemId1 AS UNSIGNED) AS req_item_id1, CAST(ReqItemId2 AS UNSIGNED) AS req_item_id2, \
                CAST(ReqItemId3 AS UNSIGNED) AS req_item_id3, CAST(ReqItemId4 AS UNSIGNED) AS req_item_id4, \
                CAST(ReqItemCount1 AS UNSIGNED) AS req_item_count1, CAST(ReqItemCount2 AS UNSIGNED) AS req_item_count2, \
                CAST(ReqItemCount3 AS UNSIGNED) AS req_item_count3, CAST(ReqItemCount4 AS UNSIGNED) AS req_item_count4, \
                CAST(RewChoiceItemId1 AS UNSIGNED) AS rew_choice_item_id1, CAST(RewChoiceItemId2 AS UNSIGNED) AS rew_choice_item_id2, \
                CAST(RewChoiceItemId3 AS UNSIGNED) AS rew_choice_item_id3, CAST(RewChoiceItemId4 AS UNSIGNED) AS rew_choice_item_id4, \
                CAST(RewChoiceItemId5 AS UNSIGNED) AS rew_choice_item_id5, CAST(RewChoiceItemId6 AS UNSIGNED) AS rew_choice_item_id6, \
                CAST(RewChoiceItemCount1 AS UNSIGNED) AS rew_choice_item_count1, CAST(RewChoiceItemCount2 AS UNSIGNED) AS rew_choice_item_count2, \
                CAST(RewChoiceItemCount3 AS UNSIGNED) AS rew_choice_item_count3, CAST(RewChoiceItemCount4 AS UNSIGNED) AS rew_choice_item_count4, \
                CAST(RewChoiceItemCount5 AS UNSIGNED) AS rew_choice_item_count5, CAST(RewChoiceItemCount6 AS UNSIGNED) AS rew_choice_item_count6, \
                CAST(RewItemId1 AS UNSIGNED) AS rew_item_id1, CAST(RewItemId2 AS UNSIGNED) AS rew_item_id2, \
                CAST(RewItemId3 AS UNSIGNED) AS rew_item_id3, CAST(RewItemId4 AS UNSIGNED) AS rew_item_id4, \
                CAST(RewItemCount1 AS UNSIGNED) AS rew_item_count1, CAST(RewItemCount2 AS UNSIGNED) AS rew_item_count2, \
                CAST(RewItemCount3 AS UNSIGNED) AS rew_item_count3, CAST(RewItemCount4 AS UNSIGNED) AS rew_item_count4, \
                CAST(PointMapId AS UNSIGNED) AS point_map_id, PointX AS point_x, PointY AS point_y, \
                CAST(PointOpt AS UNSIGNED) AS point_opt, CAST(DetailsEmote1 AS UNSIGNED) AS details_emote1, \
                CAST(DetailsEmote2 AS UNSIGNED) AS details_emote2, CAST(DetailsEmote3 AS UNSIGNED) AS details_emote3, \
                CAST(DetailsEmote4 AS UNSIGNED) AS details_emote4, CAST(DetailsEmoteDelay1 AS UNSIGNED) AS details_emote_delay1, \
                CAST(DetailsEmoteDelay2 AS UNSIGNED) AS details_emote_delay2, CAST(DetailsEmoteDelay3 AS UNSIGNED) AS details_emote_delay3, \
                CAST(DetailsEmoteDelay4 AS UNSIGNED) AS details_emote_delay4, CAST(CompleteEmote AS UNSIGNED) AS complete_emote, \
                CAST(CompleteEmoteDelay AS UNSIGNED) AS complete_emote_delay, CAST(IncompleteEmote AS UNSIGNED) AS incomplete_emote, \
                CAST(IncompleteEmoteDelay AS UNSIGNED) AS incomplete_emote_delay, CAST(OfferRewardEmote1 AS UNSIGNED) AS offer_reward_emote1, \
                CAST(OfferRewardEmote2 AS UNSIGNED) AS offer_reward_emote2, CAST(OfferRewardEmote3 AS UNSIGNED) AS offer_reward_emote3, \
                CAST(OfferRewardEmote4 AS UNSIGNED) AS offer_reward_emote4, CAST(OfferRewardEmoteDelay1 AS UNSIGNED) AS offer_reward_emote_delay1, \
                CAST(OfferRewardEmoteDelay2 AS UNSIGNED) AS offer_reward_emote_delay2, CAST(OfferRewardEmoteDelay3 AS UNSIGNED) AS offer_reward_emote_delay3, \
                CAST(OfferRewardEmoteDelay4 AS UNSIGNED) AS offer_reward_emote_delay4, \
                COALESCE(ObjectiveText1, '') AS objective_text1, \
                COALESCE(ObjectiveText2, '') AS objective_text2, \
                COALESCE(ObjectiveText3, '') AS objective_text3, \
                COALESCE(ObjectiveText4, '') AS objective_text4 \
         FROM quest_template \
         WHERE entry = ?",
    )
    .bind(quest)
    .fetch_optional(pool)
    .await?;

    Ok(row.map(QuestTemplateRow::into_query))
}

pub async fn get_closest_graveyard(
    pool: &MySqlPool,
    map: u32,
    x: f32,
    y: f32,
    z: f32,
    faction: u32,
) -> Result<Option<GraveyardQuery>, DbError> {
    let row = sqlx::query_as::<_, GraveyardQuery>(
        "SELECT world_safe_locs.id AS id, world_safe_locs.map AS map, \
                CAST(world_safe_locs.x AS DOUBLE) AS x, \
                CAST(world_safe_locs.y AS DOUBLE) AS y, \
                CAST(world_safe_locs.z AS DOUBLE) AS z, \
                CAST(world_safe_locs.o AS DOUBLE) AS o, \
                world_safe_locs.name AS name \
         FROM game_graveyard_zone \
         JOIN world_safe_locs ON game_graveyard_zone.ghost_loc = world_safe_locs.id \
         WHERE game_graveyard_zone.link_kind = 0 \
           AND world_safe_locs.map = ? \
           AND (game_graveyard_zone.faction = 0 OR game_graveyard_zone.faction = ?) \
         ORDER BY ((world_safe_locs.x - ?) * (world_safe_locs.x - ?)) + \
                  ((world_safe_locs.y - ?) * (world_safe_locs.y - ?)) + \
                  ((world_safe_locs.z - ?) * (world_safe_locs.z - ?)) ASC, \
                  world_safe_locs.id ASC \
         LIMIT 1",
    )
    .bind(map)
    .bind(faction)
    .bind(x)
    .bind(x)
    .bind(y)
    .bind(y)
    .bind(z)
    .bind(z)
    .fetch_optional(pool)
    .await?;

    Ok(row)
}

pub async fn get_closest_spirit_healer(
    pool: &MySqlPool,
    map: u32,
    x: f32,
    y: f32,
    z: f32,
) -> Result<Option<GraveyardQuery>, DbError> {
    let row = sqlx::query_as::<_, GraveyardQuery>(
        "SELECT creature.guid AS id, creature.map AS map, \
                CAST(creature.position_x AS DOUBLE) AS x, \
                CAST(creature.position_y AS DOUBLE) AS y, \
                CAST(creature.position_z AS DOUBLE) AS z, \
                CAST(creature.orientation AS DOUBLE) AS o, \
                creature_template.Name AS name \
         FROM creature \
         JOIN creature_template ON creature.id = creature_template.Entry \
         WHERE creature.map = ? AND creature.id = 6491 \
         ORDER BY ((creature.position_x - ?) * (creature.position_x - ?)) + \
                  ((creature.position_y - ?) * (creature.position_y - ?)) + \
                  ((creature.position_z - ?) * (creature.position_z - ?)) ASC, \
                  creature.guid ASC \
         LIMIT 1",
    )
    .bind(map)
    .bind(x)
    .bind(x)
    .bind(y)
    .bind(y)
    .bind(z)
    .bind(z)
    .fetch_optional(pool)
    .await?;

    Ok(row)
}

pub async fn get_creature_start_quests(
    pool: &MySqlPool,
    creature_entry: u32,
) -> Result<Vec<QuestTemplateQuery>, DbError> {
    let quest_ids: Vec<u32> =
        sqlx::query_scalar("SELECT quest FROM creature_questrelation WHERE id = ? ORDER BY quest")
            .bind(creature_entry)
            .fetch_all(pool)
            .await?;
    let mut quests = Vec::new();
    for quest in quest_ids {
        if let Some(template) = get_quest_template_query(pool, quest).await? {
            quests.push(template);
        }
    }
    Ok(quests)
}

pub async fn creature_starts_quest(
    pool: &MySqlPool,
    creature_entry: u32,
    quest: u32,
) -> Result<bool, DbError> {
    let count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM creature_questrelation WHERE id = ? AND quest = ?",
    )
    .bind(creature_entry)
    .bind(quest)
    .fetch_one(pool)
    .await?;
    Ok(count > 0)
}

pub async fn creature_completes_quest(
    pool: &MySqlPool,
    creature_entry: u32,
    quest: u32,
) -> Result<bool, DbError> {
    let count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM creature_involvedrelation WHERE id = ? AND quest = ?",
    )
    .bind(creature_entry)
    .bind(quest)
    .fetch_one(pool)
    .await?;
    Ok(count > 0)
}

pub async fn get_item_display_id(pool: &MySqlPool, item: u32) -> Result<Option<u32>, DbError> {
    sqlx::query_scalar("SELECT displayid FROM item_template WHERE entry = ?")
        .bind(item)
        .fetch_optional(pool)
        .await
        .map_err(Into::into)
}

pub async fn get_vendor_items(
    pool: &MySqlPool,
    creature_entry: u32,
) -> Result<Vec<VendorItemQuery>, DbError> {
    let rows = sqlx::query_as::<_, VendorItemRow>(
        "SELECT npc_vendor.item, npc_vendor.maxcount AS max_count, npc_vendor.slot, \
                item_template.displayid AS display_id, item_template.BuyPrice AS buy_price, \
                item_template.MaxDurability AS max_durability, \
                item_template.BuyCount AS buy_count, \
                item_template.ContainerSlots AS container_slots \
         FROM npc_vendor \
         JOIN item_template ON npc_vendor.item = item_template.entry \
         WHERE npc_vendor.entry = ? \
           AND npc_vendor.condition_id = 0 \
           AND item_template.ContainerSlots = 0 \
         ORDER BY CASE WHEN npc_vendor.slot = 0 THEN 255 ELSE npc_vendor.slot END, \
                  npc_vendor.item \
         LIMIT 128",
    )
    .bind(creature_entry)
    .fetch_all(pool)
    .await?;

    Ok(rows.into_iter().map(VendorItemRow::into_query).collect())
}

pub async fn get_trainer_spells(
    pool: &MySqlPool,
    creature_entry: u32,
) -> Result<Vec<TrainerSpellQuery>, DbError> {
    let rows = sqlx::query_as::<_, TrainerSpellQuery>(
        "SELECT npc_trainer.spell, \
                CAST(COALESCE( \
                    CASE \
                        WHEN spell_template.Effect1 = 36 \
                         AND spell_template.EffectTriggerSpell1 <> 0 \
                         AND spell_template.EffectImplicitTargetA1 IN (0, 1) \
                            THEN spell_template.EffectTriggerSpell1 \
                        WHEN spell_template.Effect2 = 36 \
                         AND spell_template.EffectTriggerSpell2 <> 0 \
                         AND spell_template.EffectImplicitTargetA2 IN (0, 1) \
                            THEN spell_template.EffectTriggerSpell2 \
                        WHEN spell_template.Effect3 = 36 \
                         AND spell_template.EffectTriggerSpell3 <> 0 \
                         AND spell_template.EffectImplicitTargetA3 IN (0, 1) \
                            THEN spell_template.EffectTriggerSpell3 \
                        ELSE npc_trainer.spell \
                    END, npc_trainer.spell) AS UNSIGNED) AS learned_spell, \
                spellcost AS spell_cost, \
                reqskill AS req_skill, reqskillvalue AS req_skill_value, \
                reqlevel AS req_level, ReqAbility1 AS req_ability1, \
                ReqAbility2 AS req_ability2, ReqAbility3 AS req_ability3 \
         FROM npc_trainer \
         LEFT JOIN spell_template ON spell_template.Id = npc_trainer.spell \
         WHERE entry = ? AND condition_id = 0 \
         ORDER BY reqlevel, spell \
         LIMIT 128",
    )
    .bind(creature_entry)
    .fetch_all(pool)
    .await?;

    Ok(rows)
}

pub async fn get_nearby_creature_spawns(
    pool: &MySqlPool,
    map: u32,
    position_x: f32,
    position_y: f32,
    radius: f32,
    limit: u32,
) -> Result<Vec<CreatureSpawnQuery>, DbError> {
    let rows = sqlx::query_as::<_, CreatureSpawnRow>(
        "SELECT creature.guid, creature.id AS entry, creature.map, \
                CAST(creature.position_x AS DOUBLE) AS position_x, \
                CAST(creature.position_y AS DOUBLE) AS position_y, \
                CAST(creature.position_z AS DOUBLE) AS position_z, \
                CAST(creature.orientation AS DOUBLE) AS orientation, \
                creature.spawntimesecsmin AS spawn_time_secs_min, \
                creature.spawntimesecsmax AS spawn_time_secs_max, \
                CAST(creature.spawndist AS DOUBLE) AS spawn_dist, \
                creature.MovementType AS movement_type, \
                CAST(spawn_group_formation.MovementType AS UNSIGNED) AS formation_movement_type, \
                CAST(spawn_group_formation.PathId AS UNSIGNED) AS formation_waypoint_path_id, \
                creature_template.Entry AS template_entry, creature_template.Name AS template_name, \
                creature_template.SubName AS template_subname, \
                creature_template.MinLevel AS template_min_level, \
                creature_template.MaxLevel AS template_max_level, \
                creature_template.DisplayId1 AS template_display_id1, \
                creature_template.DisplayId2 AS template_display_id2, \
                creature_template.DisplayId3 AS template_display_id3, \
                creature_template.DisplayId4 AS template_display_id4, \
                COALESCE(creature_model_info.bounding_radius, 0) AS template_model_bounding_radius, \
                COALESCE(creature_model_info.combat_reach, 0) AS template_model_combat_reach, \
                creature_template.Faction AS template_faction, creature_template.Scale AS template_scale, \
                creature_template.Detection AS template_detection_range, \
                creature_template.CallForHelp AS template_call_for_help, \
                creature_template.Family AS template_family, \
                creature_template.CreatureType AS template_creature_type, \
                creature_template.NpcFlags AS template_npc_flags, \
                creature_template.UnitFlags AS template_unit_flags, \
                creature_template.DynamicFlags AS template_dynamic_flags, \
                creature_template.UnitClass AS template_unit_class, \
                creature_template.Rank AS template_rank, \
                creature_template.HealthMultiplier AS template_health_multiplier, \
                creature_template.PowerMultiplier AS template_power_multiplier, \
                creature_template.DamageMultiplier AS template_damage_multiplier, \
                creature_template.DamageVariance AS template_damage_variance, \
                creature_template.ArmorMultiplier AS template_armor_multiplier, \
                creature_template.MinLevelHealth AS template_min_level_health, \
                creature_template.MaxLevelHealth AS template_max_level_health, \
                creature_template.MinLevelMana AS template_min_level_mana, \
                creature_template.MaxLevelMana AS template_max_level_mana, \
                creature_template.MinMeleeDmg AS template_min_melee_dmg, \
                creature_template.MaxMeleeDmg AS template_max_melee_dmg, \
                creature_template.MinRangedDmg AS template_min_ranged_dmg, \
                creature_template.MaxRangedDmg AS template_max_ranged_dmg, \
                creature_template.Armor AS template_armor, \
                creature_template.MeleeAttackPower AS template_melee_attack_power, \
                creature_template.RangedAttackPower AS template_ranged_attack_power, \
                creature_template.MinLootGold AS template_min_loot_gold, \
                creature_template.MaxLootGold AS template_max_loot_gold, \
                creature_template.MeleeBaseAttackTime AS template_melee_base_attack_time, \
                creature_template.RangedBaseAttackTime AS template_ranged_base_attack_time, \
                creature_template.DamageSchool AS template_damage_school, \
                creature_template.TrainerType AS template_trainer_type, \
                creature_template.TrainerClass AS template_trainer_class, \
                creature_template.PetSpellDataId AS template_pet_spell_data_id, \
                creature_template.Civilian AS template_civilian, \
                creature_template.CorpseDecay AS template_corpse_decay, \
                creature_template.MovementType AS template_movement_type, \
                creature_template.ExperienceMultiplier AS template_experience_multiplier \
         FROM creature \
         JOIN creature_template ON creature.id = creature_template.Entry \
         LEFT JOIN creature_model_info \
           ON creature_model_info.modelid = COALESCE(NULLIF(creature_template.DisplayId1, 0), NULLIF(creature_template.DisplayId2, 0), NULLIF(creature_template.DisplayId3, 0), NULLIF(creature_template.DisplayId4, 0), 0) \
         LEFT JOIN spawn_group_spawn \
           ON spawn_group_spawn.Guid = creature.guid AND spawn_group_spawn.SlotId = 0 \
         LEFT JOIN spawn_group_formation \
           ON spawn_group_formation.Id = spawn_group_spawn.Id \
         WHERE creature.map = ? \
           AND creature.position_x BETWEEN ? AND ? \
           AND creature.position_y BETWEEN ? AND ? \
           AND (((creature.position_x - ?) * (creature.position_x - ?)) + \
                ((creature.position_y - ?) * (creature.position_y - ?))) <= ? \
         ORDER BY ((creature.position_x - ?) * (creature.position_x - ?)) + \
                  ((creature.position_y - ?) * (creature.position_y - ?)) ASC, \
                  creature.guid ASC \
         LIMIT ?",
    )
    .bind(map)
    .bind(position_x - radius)
    .bind(position_x + radius)
    .bind(position_y - radius)
    .bind(position_y + radius)
    .bind(position_x)
    .bind(position_x)
    .bind(position_y)
    .bind(position_y)
    .bind(radius * radius)
    .bind(position_x)
    .bind(position_x)
    .bind(position_y)
    .bind(position_y)
    .bind(limit)
    .fetch_all(pool)
    .await?;

    let mut spawns = rows
        .into_iter()
        .map(CreatureSpawnRow::into_query)
        .collect::<Vec<_>>();
    for spawn in &mut spawns {
        if creature_effective_movement_type(spawn) == 2
            || creature_effective_movement_type(spawn) == 4
        {
            spawn.waypoint_path = get_creature_default_waypoint_path(
                pool,
                spawn.entry,
                spawn.guid,
                spawn.formation_waypoint_path_id,
            )
            .await?;
        }
    }

    Ok(spawns)
}

pub async fn get_creature_spawns_in_rect(
    pool: &MySqlPool,
    map: u32,
    min_x: f32,
    max_x: f32,
    min_y: f32,
    max_y: f32,
) -> Result<Vec<CreatureSpawnQuery>, DbError> {
    let rows = sqlx::query_as::<_, CreatureSpawnRow>(
        "SELECT creature.guid, creature.id AS entry, creature.map, \
                CAST(creature.position_x AS DOUBLE) AS position_x, \
                CAST(creature.position_y AS DOUBLE) AS position_y, \
                CAST(creature.position_z AS DOUBLE) AS position_z, \
                CAST(creature.orientation AS DOUBLE) AS orientation, \
                creature.spawntimesecsmin AS spawn_time_secs_min, \
                creature.spawntimesecsmax AS spawn_time_secs_max, \
                CAST(creature.spawndist AS DOUBLE) AS spawn_dist, \
                creature.MovementType AS movement_type, \
                CAST(spawn_group_formation.MovementType AS UNSIGNED) AS formation_movement_type, \
                CAST(spawn_group_formation.PathId AS UNSIGNED) AS formation_waypoint_path_id, \
                creature_template.Entry AS template_entry, creature_template.Name AS template_name, \
                creature_template.SubName AS template_subname, \
                creature_template.MinLevel AS template_min_level, \
                creature_template.MaxLevel AS template_max_level, \
                creature_template.DisplayId1 AS template_display_id1, \
                creature_template.DisplayId2 AS template_display_id2, \
                creature_template.DisplayId3 AS template_display_id3, \
                creature_template.DisplayId4 AS template_display_id4, \
                COALESCE(creature_model_info.bounding_radius, 0) AS template_model_bounding_radius, \
                COALESCE(creature_model_info.combat_reach, 0) AS template_model_combat_reach, \
                creature_template.Faction AS template_faction, creature_template.Scale AS template_scale, \
                creature_template.Detection AS template_detection_range, \
                creature_template.CallForHelp AS template_call_for_help, \
                creature_template.Family AS template_family, \
                creature_template.CreatureType AS template_creature_type, \
                creature_template.NpcFlags AS template_npc_flags, \
                creature_template.UnitFlags AS template_unit_flags, \
                creature_template.DynamicFlags AS template_dynamic_flags, \
                creature_template.UnitClass AS template_unit_class, \
                creature_template.Rank AS template_rank, \
                creature_template.HealthMultiplier AS template_health_multiplier, \
                creature_template.PowerMultiplier AS template_power_multiplier, \
                creature_template.DamageMultiplier AS template_damage_multiplier, \
                creature_template.DamageVariance AS template_damage_variance, \
                creature_template.ArmorMultiplier AS template_armor_multiplier, \
                creature_template.MinLevelHealth AS template_min_level_health, \
                creature_template.MaxLevelHealth AS template_max_level_health, \
                creature_template.MinLevelMana AS template_min_level_mana, \
                creature_template.MaxLevelMana AS template_max_level_mana, \
                creature_template.MinMeleeDmg AS template_min_melee_dmg, \
                creature_template.MaxMeleeDmg AS template_max_melee_dmg, \
                creature_template.MinRangedDmg AS template_min_ranged_dmg, \
                creature_template.MaxRangedDmg AS template_max_ranged_dmg, \
                creature_template.Armor AS template_armor, \
                creature_template.MeleeAttackPower AS template_melee_attack_power, \
                creature_template.RangedAttackPower AS template_ranged_attack_power, \
                creature_template.MinLootGold AS template_min_loot_gold, \
                creature_template.MaxLootGold AS template_max_loot_gold, \
                creature_template.MeleeBaseAttackTime AS template_melee_base_attack_time, \
                creature_template.RangedBaseAttackTime AS template_ranged_base_attack_time, \
                creature_template.DamageSchool AS template_damage_school, \
                creature_template.TrainerType AS template_trainer_type, \
                creature_template.TrainerClass AS template_trainer_class, \
                creature_template.PetSpellDataId AS template_pet_spell_data_id, \
                creature_template.Civilian AS template_civilian, \
                creature_template.CorpseDecay AS template_corpse_decay, \
                creature_template.MovementType AS template_movement_type, \
                creature_template.ExperienceMultiplier AS template_experience_multiplier \
         FROM creature \
         JOIN creature_template ON creature.id = creature_template.Entry \
         LEFT JOIN creature_model_info \
           ON creature_model_info.modelid = COALESCE(NULLIF(creature_template.DisplayId1, 0), NULLIF(creature_template.DisplayId2, 0), NULLIF(creature_template.DisplayId3, 0), NULLIF(creature_template.DisplayId4, 0), 0) \
         LEFT JOIN spawn_group_spawn \
           ON spawn_group_spawn.Guid = creature.guid AND spawn_group_spawn.SlotId = 0 \
         LEFT JOIN spawn_group_formation \
           ON spawn_group_formation.Id = spawn_group_spawn.Id \
         WHERE creature.map = ? \
           AND creature.position_x BETWEEN ? AND ? \
           AND creature.position_y BETWEEN ? AND ? \
         ORDER BY creature.guid ASC",
    )
    .bind(map)
    .bind(min_x)
    .bind(max_x)
    .bind(min_y)
    .bind(max_y)
    .fetch_all(pool)
    .await?;

    let mut spawns = rows
        .into_iter()
        .map(CreatureSpawnRow::into_query)
        .collect::<Vec<_>>();
    for spawn in &mut spawns {
        if creature_effective_movement_type(spawn) == 2
            || creature_effective_movement_type(spawn) == 4
        {
            spawn.waypoint_path = get_creature_default_waypoint_path(
                pool,
                spawn.entry,
                spawn.guid,
                spawn.formation_waypoint_path_id,
            )
            .await?;
        }
    }

    Ok(spawns)
}

pub async fn get_creature_respawn_times(
    pool: &MySqlPool,
    guids: &[u32],
    instance: u32,
    now_epoch_secs: u64,
) -> Result<HashMap<u32, u64>, DbError> {
    if guids.is_empty() {
        return Ok(HashMap::new());
    }

    sqlx::query("DELETE FROM creature_respawn WHERE instance = ? AND respawntime <= ?")
        .bind(instance)
        .bind(now_epoch_secs)
        .execute(pool)
        .await?;

    let mut builder =
        QueryBuilder::new("SELECT guid, respawntime FROM creature_respawn WHERE instance = ");
    builder.push_bind(instance);
    builder.push(" AND guid IN (");
    let mut separated = builder.separated(", ");
    for guid in guids {
        separated.push_bind(*guid);
    }
    separated.push_unseparated(")");

    let rows = builder
        .build_query_as::<CreatureRespawnRow>()
        .fetch_all(pool)
        .await?;

    Ok(rows
        .into_iter()
        .map(|row| (row.guid, row.respawntime))
        .collect())
}

pub async fn save_creature_respawn_time(
    pool: &MySqlPool,
    guid: u32,
    respawn_time_epoch_secs: u64,
    instance: u32,
    now_epoch_secs: u64,
) -> Result<(), DbError> {
    let mut tx = pool.begin().await?;
    sqlx::query("DELETE FROM creature_respawn WHERE guid = ? AND instance = ?")
        .bind(guid)
        .bind(instance)
        .execute(&mut *tx)
        .await?;
    if respawn_time_epoch_secs > now_epoch_secs {
        sqlx::query("INSERT INTO creature_respawn (guid, respawntime, instance) VALUES (?, ?, ?)")
            .bind(guid)
            .bind(respawn_time_epoch_secs)
            .bind(instance)
            .execute(&mut *tx)
            .await?;
    }
    tx.commit().await?;
    Ok(())
}

pub async fn get_creature_default_waypoint_path(
    pool: &MySqlPool,
    entry: u32,
    guid: u32,
    formation_path_id: Option<u32>,
) -> Result<Vec<CreatureWaypointQuery>, DbError> {
    if let Some(path_id) = formation_path_id.filter(|path_id| *path_id != 0) {
        let formation_path = get_waypoint_path(pool, path_id).await?;
        if !formation_path.is_empty() {
            return Ok(formation_path);
        }
    }
    let guid_path = get_creature_guid_waypoint_path(pool, guid).await?;
    if !guid_path.is_empty() {
        return Ok(guid_path);
    }
    get_creature_template_waypoint_path(pool, entry, 0).await
}

async fn get_waypoint_path(
    pool: &MySqlPool,
    path_id: u32,
) -> Result<Vec<CreatureWaypointQuery>, DbError> {
    let rows = sqlx::query_as::<_, CreatureWaypointRow>(
        "SELECT Point AS point, \
                CAST(PositionX AS DOUBLE) AS position_x, \
                CAST(PositionY AS DOUBLE) AS position_y, \
                CAST(PositionZ AS DOUBLE) AS position_z, \
                CAST(Orientation AS DOUBLE) AS orientation, \
                WaitTime AS wait_time, ScriptId AS script_id \
         FROM waypoint_path \
         WHERE PathId = ? \
         ORDER BY Point",
    )
    .bind(path_id)
    .fetch_all(pool)
    .await?;

    Ok(creature_waypoint_rows_into_path(rows))
}

async fn get_creature_guid_waypoint_path(
    pool: &MySqlPool,
    guid: u32,
) -> Result<Vec<CreatureWaypointQuery>, DbError> {
    let rows = sqlx::query_as::<_, CreatureWaypointRow>(
        "SELECT Point AS point, \
                CAST(PositionX AS DOUBLE) AS position_x, \
                CAST(PositionY AS DOUBLE) AS position_y, \
                CAST(PositionZ AS DOUBLE) AS position_z, \
                CAST(Orientation AS DOUBLE) AS orientation, \
                WaitTime AS wait_time, ScriptId AS script_id \
         FROM creature_movement \
         WHERE Id = ? \
         ORDER BY Point",
    )
    .bind(guid)
    .fetch_all(pool)
    .await?;

    Ok(creature_waypoint_rows_into_path(rows))
}

async fn get_creature_template_waypoint_path(
    pool: &MySqlPool,
    entry: u32,
    path_id: u32,
) -> Result<Vec<CreatureWaypointQuery>, DbError> {
    let rows = sqlx::query_as::<_, CreatureWaypointRow>(
        "SELECT Point AS point, \
                CAST(PositionX AS DOUBLE) AS position_x, \
                CAST(PositionY AS DOUBLE) AS position_y, \
                CAST(PositionZ AS DOUBLE) AS position_z, \
                CAST(Orientation AS DOUBLE) AS orientation, \
                WaitTime AS wait_time, ScriptId AS script_id \
         FROM creature_movement_template \
         WHERE Entry = ? AND PathId = ? \
         ORDER BY Point",
    )
    .bind(entry)
    .bind(path_id)
    .fetch_all(pool)
    .await?;

    Ok(creature_waypoint_rows_into_path(rows))
}

fn creature_effective_movement_type(spawn: &CreatureSpawnQuery) -> u8 {
    if spawn.movement_type != 0 {
        spawn.movement_type
    } else {
        spawn.template.movement_type
    }
}

fn creature_waypoint_rows_into_path(rows: Vec<CreatureWaypointRow>) -> Vec<CreatureWaypointQuery> {
    if rows.iter().any(|row| row.point == 0) {
        return Vec::new();
    }
    rows.into_iter()
        .map(CreatureWaypointRow::into_query)
        .collect()
}

#[derive(Debug, Clone, FromRow)]
struct VendorItemRow {
    item: u32,
    max_count: u8,
    slot: u8,
    display_id: u32,
    buy_price: u32,
    max_durability: u16,
    buy_count: u8,
    container_slots: u8,
}

#[derive(Debug, Clone, FromRow)]
struct CreatureLootRow {
    item: u32,
    min_count: u32,
    max_count: u32,
    display_id: u32,
}

#[derive(Debug, Clone, FromRow)]
struct QuestTemplateRow {
    entry: u32,
    method: u32,
    zone_or_sort: i16,
    quest_level: u32,
    quest_type: u32,
    rep_objective_faction: u32,
    rep_objective_value: i32,
    next_quest_in_chain: u32,
    rew_or_req_money: i32,
    rew_money_max_level: u32,
    rew_spell: u32,
    rew_spell_cast: u32,
    src_item_id: u32,
    quest_flags: u32,
    title: String,
    details: String,
    objectives: String,
    offer_reward_text: String,
    request_items_text: String,
    end_text: String,
    req_creature_or_go_id1: i32,
    req_creature_or_go_id2: i32,
    req_creature_or_go_id3: i32,
    req_creature_or_go_id4: i32,
    req_creature_or_go_count1: u32,
    req_creature_or_go_count2: u32,
    req_creature_or_go_count3: u32,
    req_creature_or_go_count4: u32,
    req_item_id1: u32,
    req_item_id2: u32,
    req_item_id3: u32,
    req_item_id4: u32,
    req_item_count1: u32,
    req_item_count2: u32,
    req_item_count3: u32,
    req_item_count4: u32,
    rew_choice_item_id1: u32,
    rew_choice_item_id2: u32,
    rew_choice_item_id3: u32,
    rew_choice_item_id4: u32,
    rew_choice_item_id5: u32,
    rew_choice_item_id6: u32,
    rew_choice_item_count1: u32,
    rew_choice_item_count2: u32,
    rew_choice_item_count3: u32,
    rew_choice_item_count4: u32,
    rew_choice_item_count5: u32,
    rew_choice_item_count6: u32,
    rew_item_id1: u32,
    rew_item_id2: u32,
    rew_item_id3: u32,
    rew_item_id4: u32,
    rew_item_count1: u32,
    rew_item_count2: u32,
    rew_item_count3: u32,
    rew_item_count4: u32,
    point_map_id: u32,
    point_x: f32,
    point_y: f32,
    point_opt: u32,
    details_emote1: u32,
    details_emote2: u32,
    details_emote3: u32,
    details_emote4: u32,
    details_emote_delay1: u32,
    details_emote_delay2: u32,
    details_emote_delay3: u32,
    details_emote_delay4: u32,
    complete_emote: u32,
    complete_emote_delay: u32,
    incomplete_emote: u32,
    incomplete_emote_delay: u32,
    offer_reward_emote1: u32,
    offer_reward_emote2: u32,
    offer_reward_emote3: u32,
    offer_reward_emote4: u32,
    offer_reward_emote_delay1: u32,
    offer_reward_emote_delay2: u32,
    offer_reward_emote_delay3: u32,
    offer_reward_emote_delay4: u32,
    objective_text1: String,
    objective_text2: String,
    objective_text3: String,
    objective_text4: String,
}

impl QuestTemplateRow {
    fn into_query(self) -> QuestTemplateQuery {
        QuestTemplateQuery {
            entry: self.entry,
            method: self.method,
            zone_or_sort: self.zone_or_sort,
            quest_level: self.quest_level,
            quest_type: self.quest_type,
            rep_objective_faction: self.rep_objective_faction,
            rep_objective_value: self.rep_objective_value,
            next_quest_in_chain: self.next_quest_in_chain,
            rew_or_req_money: self.rew_or_req_money,
            rew_money_max_level: self.rew_money_max_level,
            rew_spell: self.rew_spell,
            rew_spell_cast: self.rew_spell_cast,
            src_item_id: self.src_item_id,
            quest_flags: self.quest_flags,
            title: self.title,
            details: self.details,
            objectives: self.objectives,
            offer_reward_text: self.offer_reward_text,
            request_items_text: self.request_items_text,
            end_text: self.end_text,
            req_creature_or_go_id: [
                self.req_creature_or_go_id1,
                self.req_creature_or_go_id2,
                self.req_creature_or_go_id3,
                self.req_creature_or_go_id4,
            ],
            req_creature_or_go_count: [
                self.req_creature_or_go_count1,
                self.req_creature_or_go_count2,
                self.req_creature_or_go_count3,
                self.req_creature_or_go_count4,
            ],
            req_item_id: [
                self.req_item_id1,
                self.req_item_id2,
                self.req_item_id3,
                self.req_item_id4,
            ],
            req_item_count: [
                self.req_item_count1,
                self.req_item_count2,
                self.req_item_count3,
                self.req_item_count4,
            ],
            rew_choice_item_id: [
                self.rew_choice_item_id1,
                self.rew_choice_item_id2,
                self.rew_choice_item_id3,
                self.rew_choice_item_id4,
                self.rew_choice_item_id5,
                self.rew_choice_item_id6,
            ],
            rew_choice_item_count: [
                self.rew_choice_item_count1,
                self.rew_choice_item_count2,
                self.rew_choice_item_count3,
                self.rew_choice_item_count4,
                self.rew_choice_item_count5,
                self.rew_choice_item_count6,
            ],
            rew_item_id: [
                self.rew_item_id1,
                self.rew_item_id2,
                self.rew_item_id3,
                self.rew_item_id4,
            ],
            rew_item_count: [
                self.rew_item_count1,
                self.rew_item_count2,
                self.rew_item_count3,
                self.rew_item_count4,
            ],
            point_map_id: self.point_map_id,
            point_x: self.point_x,
            point_y: self.point_y,
            point_opt: self.point_opt,
            details_emote: [
                self.details_emote1,
                self.details_emote2,
                self.details_emote3,
                self.details_emote4,
            ],
            details_emote_delay: [
                self.details_emote_delay1,
                self.details_emote_delay2,
                self.details_emote_delay3,
                self.details_emote_delay4,
            ],
            complete_emote: self.complete_emote,
            complete_emote_delay: self.complete_emote_delay,
            incomplete_emote: self.incomplete_emote,
            incomplete_emote_delay: self.incomplete_emote_delay,
            offer_reward_emote: [
                self.offer_reward_emote1,
                self.offer_reward_emote2,
                self.offer_reward_emote3,
                self.offer_reward_emote4,
            ],
            offer_reward_emote_delay: [
                self.offer_reward_emote_delay1,
                self.offer_reward_emote_delay2,
                self.offer_reward_emote_delay3,
                self.offer_reward_emote_delay4,
            ],
            objective_text: [
                self.objective_text1,
                self.objective_text2,
                self.objective_text3,
                self.objective_text4,
            ],
        }
    }
}

impl CreatureLootRow {
    fn into_query(self) -> CreatureLootQuery {
        CreatureLootQuery {
            item: self.item,
            min_count: self.min_count,
            max_count: self.max_count,
            display_id: self.display_id,
        }
    }
}

impl VendorItemRow {
    fn into_query(self) -> VendorItemQuery {
        VendorItemQuery {
            item: self.item,
            max_count: self.max_count as u32,
            slot: self.slot,
            display_id: self.display_id,
            buy_price: self.buy_price,
            max_durability: self.max_durability as u32,
            buy_count: self.buy_count as u32,
            container_slots: self.container_slots as u32,
        }
    }
}

#[derive(Debug, Clone, FromRow)]
struct CreatureWaypointRow {
    point: u32,
    position_x: f64,
    position_y: f64,
    position_z: f64,
    orientation: f64,
    wait_time: u32,
    script_id: u32,
}

#[derive(Debug, FromRow)]
struct CreatureRespawnRow {
    guid: u32,
    respawntime: u64,
}

impl CreatureWaypointRow {
    fn into_query(self) -> CreatureWaypointQuery {
        CreatureWaypointQuery {
            point: self.point,
            position_x: self.position_x as f32,
            position_y: self.position_y as f32,
            position_z: self.position_z as f32,
            orientation: (self.orientation as f32 != 100.0).then_some(self.orientation as f32),
            wait_time: self.wait_time,
            script_id: self.script_id,
        }
    }
}

#[derive(Debug, Clone, FromRow)]
struct CreatureSpawnRow {
    guid: u32,
    entry: u32,
    map: u32,
    position_x: f64,
    position_y: f64,
    position_z: f64,
    orientation: f64,
    spawn_time_secs_min: u32,
    spawn_time_secs_max: u32,
    spawn_dist: f64,
    movement_type: u8,
    formation_movement_type: Option<u8>,
    formation_waypoint_path_id: Option<u32>,
    template_entry: u32,
    template_name: String,
    template_subname: Option<String>,
    template_min_level: u8,
    template_max_level: u8,
    template_display_id1: u32,
    template_display_id2: u32,
    template_display_id3: u32,
    template_display_id4: u32,
    template_model_bounding_radius: f32,
    template_model_combat_reach: f32,
    template_faction: u32,
    template_scale: f32,
    template_detection_range: u32,
    template_call_for_help: u32,
    template_family: i32,
    template_creature_type: u32,
    template_npc_flags: u32,
    template_unit_flags: u32,
    template_dynamic_flags: u32,
    template_unit_class: u8,
    template_rank: u32,
    template_health_multiplier: f32,
    template_power_multiplier: f32,
    template_damage_multiplier: f32,
    template_damage_variance: f32,
    template_armor_multiplier: f32,
    template_min_level_health: u32,
    template_max_level_health: u32,
    template_min_level_mana: u32,
    template_max_level_mana: u32,
    template_min_melee_dmg: f32,
    template_max_melee_dmg: f32,
    template_min_ranged_dmg: f32,
    template_max_ranged_dmg: f32,
    template_armor: u32,
    template_melee_attack_power: u32,
    template_ranged_attack_power: u32,
    template_min_loot_gold: u32,
    template_max_loot_gold: u32,
    template_melee_base_attack_time: u32,
    template_ranged_base_attack_time: u32,
    template_damage_school: i8,
    template_trainer_type: i8,
    template_trainer_class: u8,
    template_pet_spell_data_id: u32,
    template_civilian: u8,
    template_corpse_decay: u32,
    template_movement_type: u8,
    template_experience_multiplier: f32,
}

impl CreatureSpawnRow {
    fn into_query(self) -> CreatureSpawnQuery {
        CreatureSpawnQuery {
            guid: self.guid,
            entry: self.entry,
            map: self.map,
            position_x: self.position_x as f32,
            position_y: self.position_y as f32,
            position_z: self.position_z as f32,
            orientation: self.orientation as f32,
            spawn_time_secs_min: self.spawn_time_secs_min,
            spawn_time_secs_max: self.spawn_time_secs_max,
            spawn_dist: self.spawn_dist as f32,
            movement_type: self.formation_movement_type.unwrap_or(self.movement_type),
            formation_waypoint_path_id: self.formation_waypoint_path_id,
            template: CreatureTemplateQuery {
                entry: self.template_entry,
                name: self.template_name,
                subname: self.template_subname,
                min_level: self.template_min_level,
                max_level: self.template_max_level,
                display_id1: self.template_display_id1,
                display_id2: self.template_display_id2,
                display_id3: self.template_display_id3,
                display_id4: self.template_display_id4,
                model_bounding_radius: self.template_model_bounding_radius,
                model_combat_reach: self.template_model_combat_reach,
                faction: self.template_faction,
                scale: self.template_scale,
                detection_range: self.template_detection_range,
                call_for_help: self.template_call_for_help,
                family: self.template_family,
                creature_type: self.template_creature_type,
                npc_flags: self.template_npc_flags,
                unit_flags: self.template_unit_flags,
                dynamic_flags: self.template_dynamic_flags,
                unit_class: self.template_unit_class,
                rank: self.template_rank,
                health_multiplier: self.template_health_multiplier,
                power_multiplier: self.template_power_multiplier,
                damage_multiplier: self.template_damage_multiplier,
                damage_variance: self.template_damage_variance,
                armor_multiplier: self.template_armor_multiplier,
                min_level_health: self.template_min_level_health,
                max_level_health: self.template_max_level_health,
                min_level_mana: self.template_min_level_mana,
                max_level_mana: self.template_max_level_mana,
                min_melee_dmg: self.template_min_melee_dmg,
                max_melee_dmg: self.template_max_melee_dmg,
                min_ranged_dmg: self.template_min_ranged_dmg,
                max_ranged_dmg: self.template_max_ranged_dmg,
                armor: self.template_armor,
                melee_attack_power: self.template_melee_attack_power,
                ranged_attack_power: self.template_ranged_attack_power,
                min_loot_gold: self.template_min_loot_gold,
                max_loot_gold: self.template_max_loot_gold,
                melee_base_attack_time: self.template_melee_base_attack_time,
                ranged_base_attack_time: self.template_ranged_base_attack_time,
                damage_school: self.template_damage_school,
                trainer_type: self.template_trainer_type,
                trainer_class: self.template_trainer_class,
                pet_spell_data_id: self.template_pet_spell_data_id,
                civilian: self.template_civilian,
                corpse_decay: self.template_corpse_decay,
                movement_type: self.template_movement_type,
                experience_multiplier: self.template_experience_multiplier,
            },
            waypoint_path: Vec::new(),
        }
    }
}
