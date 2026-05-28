use std::collections::{HashMap, HashSet};
use std::sync::OnceLock;

use rand::seq::SliceRandom;
use rand::Rng;
use serde::{Deserialize, Serialize};
use sqlx::mysql::MySqlPool;
use sqlx::{FromRow, MySql, QueryBuilder};

use crate::character::ItemRandomPropertyRoll;
use crate::pool::DbError;

const CMANGOS_SPELL_FIXES_SQL: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../sql/base/dbc/cmangos_fixes/Spell.sql"
));

static SPELL_EFFECT_BONUS_COEFFICIENT_FIXES: OnceLock<HashMap<u32, [f32; 3]>> = OnceLock::new();

#[derive(Debug, Clone, Copy, FromRow, Serialize, Deserialize, PartialEq, Eq)]
pub struct ExplorationBaseXpQuery {
    pub level: u8,
    pub basexp: u32,
}

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
    pub display_id_probability1: u32,
    pub display_id_probability2: u32,
    pub display_id_probability3: u32,
    pub display_id_probability4: u32,
    pub model_gender1: u8,
    pub model_gender2: u8,
    pub model_gender3: u8,
    pub model_gender4: u8,
    pub model_other_gender1: u32,
    pub model_other_gender2: u32,
    pub model_other_gender3: u32,
    pub model_other_gender4: u32,
    pub model_other_gender_gender1: u8,
    pub model_other_gender_gender2: u8,
    pub model_other_gender_gender3: u8,
    pub model_other_gender_gender4: u8,
    pub model_bounding_radius: f32,
    pub model_combat_reach: f32,
    pub faction: u32,
    pub scale: f32,
    pub speed_walk: f32,
    pub speed_run: f32,
    pub detection_range: u32,
    pub call_for_help: u32,
    pub pursuit: u32,
    pub leash: u32,
    pub family: i32,
    pub creature_type: u32,
    pub creature_type_flags: u32,
    pub inhabit_type: u32,
    pub npc_flags: u32,
    pub unit_flags: u32,
    pub dynamic_flags: u32,
    pub static_flags2: u32,
    pub extra_flags: u32,
    pub unit_class: u8,
    pub base_strength: Option<u32>,
    pub rank: u32,
    pub health_multiplier: f32,
    pub power_multiplier: f32,
    pub damage_multiplier: f32,
    pub damage_variance: f32,
    pub armor_multiplier: f32,
    pub strength_multiplier: f32,
    pub min_level_health: u32,
    pub max_level_health: u32,
    pub min_level_mana: u32,
    pub max_level_mana: u32,
    pub min_melee_dmg: f32,
    pub max_melee_dmg: f32,
    pub min_ranged_dmg: f32,
    pub max_ranged_dmg: f32,
    pub armor: u32,
    pub resistance_holy: i16,
    pub resistance_fire: i16,
    pub resistance_nature: i16,
    pub resistance_frost: i16,
    pub resistance_shadow: i16,
    pub resistance_arcane: i16,
    pub melee_attack_power: u32,
    pub ranged_attack_power: u32,
    pub min_loot_gold: u32,
    pub max_loot_gold: u32,
    pub pickpocket_loot_id: u32,
    pub melee_base_attack_time: u32,
    pub ranged_base_attack_time: u32,
    pub damage_school: i8,
    pub trainer_type: i8,
    pub trainer_class: u8,
    pub pet_spell_data_id: u32,
    pub spell_list: u32,
    pub civilian: u8,
    pub corpse_decay: u32,
    pub movement_type: u8,
    pub equipment_template_id: u32,
    pub equip_display_id1: u32,
    pub equip_display_id2: u32,
    pub equip_display_id3: u32,
    pub equip_class1: u32,
    pub equip_class2: u32,
    pub equip_class3: u32,
    pub equip_subclass1: u32,
    pub equip_subclass2: u32,
    pub equip_subclass3: u32,
    pub equip_material1: i32,
    pub equip_material2: i32,
    pub equip_material3: i32,
    pub equip_inventory_type1: u32,
    pub equip_inventory_type2: u32,
    pub equip_inventory_type3: u32,
    pub equip_sheath1: u32,
    pub equip_sheath2: u32,
    pub equip_sheath3: u32,
    pub experience_multiplier: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreatureSpawnQuery {
    pub guid: u32,
    pub entry: u32,
    pub map: u32,
    pub game_event: Option<i16>,
    pub guid_pool_id: Option<u16>,
    pub entry_pool_id: Option<u16>,
    pub pool_max_limit: Option<u32>,
    pub pool_chance: f32,
    pub addon_emote: u32,
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
pub struct DbScriptCommandQuery {
    pub id: u32,
    pub delay: u32,
    pub priority: u32,
    pub command: u32,
    pub datalong: u32,
    pub datalong2: u32,
    pub datalong3: u32,
    pub data_flags: u32,
    pub dataint: i32,
    pub dataint2: i32,
    pub dataint3: i32,
    pub dataint4: i32,
    pub condition_id: u32,
}

#[derive(Debug, Clone, FromRow, Serialize, Deserialize, PartialEq)]
pub struct ScriptTextQuery {
    pub entry: i32,
    pub content_default: String,
    pub sound: u32,
    pub chat_type: u8,
    pub language: u8,
    pub emote: u32,
    pub broadcast_text_id: i32,
}

#[derive(Debug, Clone, FromRow, Serialize, Deserialize, PartialEq)]
pub struct BroadcastTextQuery {
    pub id: i32,
    pub text: Option<String>,
    pub text1: Option<String>,
    pub chat_type: u32,
    pub language: u32,
    pub sound: u32,
    pub emote: u32,
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

#[derive(Debug, Clone, FromRow, Serialize, Deserialize, PartialEq, Eq)]
pub struct PageTextQuery {
    #[sqlx(rename = "entry")]
    pub id: u32,
    pub text: String,
    #[sqlx(rename = "next_page")]
    pub next_page_text_id: u32,
}

#[derive(Debug, Clone, FromRow, Serialize, Deserialize, PartialEq, Eq)]
pub struct GossipMenuQuery {
    pub entry: u32,
    #[sqlx(rename = "text_id")]
    pub text_id: u32,
    pub script_id: u32,
    pub condition_id: u32,
}

#[derive(Debug, Clone, FromRow, Serialize, Deserialize, PartialEq, Eq)]
pub struct GossipMenuOptionQuery {
    pub menu_id: u32,
    pub id: u32,
    pub option_icon: u8,
    pub option_text: Option<String>,
    pub option_id: u32,
    pub npc_option_npcflag: u32,
    pub action_menu_id: i32,
    pub action_poi_id: u32,
    pub action_script_id: u32,
    pub box_coded: u8,
    pub box_text: Option<String>,
    pub condition_id: u32,
}

#[derive(Debug, Clone, FromRow, Serialize, Deserialize, PartialEq, Eq)]
pub struct NpcTextQuery {
    #[sqlx(rename = "ID")]
    pub id: u32,
    #[sqlx(rename = "text0_0")]
    pub text0_0: String,
    #[sqlx(rename = "text0_1")]
    pub text0_1: String,
}

#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct VendorItemQuery {
    pub item: u32,
    pub max_count: u32,
    pub incr_time: u32,
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
    pub group_id: u8,
    pub min_count: u32,
    pub max_count: u32,
    pub display_id: u32,
    pub chance_or_quest_chance: f32,
}

#[derive(Debug, Clone, FromRow, Serialize, Deserialize, PartialEq)]
pub struct CreatureSpellListQuery {
    pub id: u32,
    pub chance_support_action: u32,
    pub chance_ranged_attack: u32,
    pub position: u32,
    pub spell_id: u32,
    pub flags: u32,
    pub combat_condition: i32,
    pub target_id: u32,
    pub script_id: u32,
    pub availability: u32,
    pub probability: u32,
    pub initial_min: u32,
    pub initial_max: u32,
    pub repeat_min: u32,
    pub repeat_max: u32,
    pub recovery_time: u32,
    pub category: u32,
    pub category_recovery_time: u32,
    pub target_type: u32,
    pub target_param1: i32,
    pub target_param2: i32,
    pub target_param3: i32,
    pub target_unit_condition: i32,
}

#[derive(Debug, Clone, FromRow, Serialize, Deserialize, PartialEq, Eq)]
pub struct CreatureAiScriptQuery {
    pub id: i32,
    pub creature_id: i32,
    pub event_type: u8,
    pub event_chance: u32,
    pub event_flags: u32,
    pub event_param1: i32,
    pub event_param2: i32,
    pub event_param3: i32,
    pub event_param4: i32,
    pub event_param5: i32,
    pub event_param6: i32,
    pub action1_type: u8,
    pub action1_param1: i32,
    pub action1_param2: i32,
    pub action1_param3: i32,
    pub action2_type: u8,
    pub action2_param1: i32,
    pub action2_param2: i32,
    pub action2_param3: i32,
    pub action3_type: u8,
    pub action3_param1: i32,
    pub action3_param2: i32,
    pub action3_param3: i32,
}

#[derive(Debug, Clone, FromRow, Serialize, Deserialize, PartialEq, Eq)]
pub struct UnitConditionQuery {
    pub id: i32,
    pub flags: u32,
    pub variable_0: u32,
    pub variable_1: u32,
    pub variable_2: u32,
    pub variable_3: u32,
    pub variable_4: u32,
    pub variable_5: u32,
    pub variable_6: u32,
    pub variable_7: u32,
    pub op_0: u32,
    pub op_1: u32,
    pub op_2: u32,
    pub op_3: u32,
    pub op_4: u32,
    pub op_5: u32,
    pub op_6: u32,
    pub op_7: u32,
    pub value_0: i32,
    pub value_1: i32,
    pub value_2: i32,
    pub value_3: i32,
    pub value_4: i32,
    pub value_5: i32,
    pub value_6: i32,
    pub value_7: i32,
}

impl UnitConditionQuery {
    pub fn variables(&self) -> [u32; 8] {
        [
            self.variable_0,
            self.variable_1,
            self.variable_2,
            self.variable_3,
            self.variable_4,
            self.variable_5,
            self.variable_6,
            self.variable_7,
        ]
    }

    pub fn operations(&self) -> [u32; 8] {
        [
            self.op_0, self.op_1, self.op_2, self.op_3, self.op_4, self.op_5, self.op_6, self.op_7,
        ]
    }

    pub fn values(&self) -> [i32; 8] {
        [
            self.value_0,
            self.value_1,
            self.value_2,
            self.value_3,
            self.value_4,
            self.value_5,
            self.value_6,
            self.value_7,
        ]
    }
}

#[derive(Debug, Clone, FromRow, Serialize, Deserialize, PartialEq, Eq)]
pub struct CombatConditionQuery {
    pub id: i32,
    pub world_state_expression_id: i32,
    pub self_condition_id: i32,
    pub target_condition_id: i32,
    pub friend_condition_logic: i32,
    pub enemy_condition_logic: i32,
    pub friend_condition_id_0: i32,
    pub friend_condition_id_1: i32,
    pub friend_condition_op_0: i32,
    pub friend_condition_op_1: i32,
    pub friend_condition_count_0: i32,
    pub friend_condition_count_1: i32,
    pub enemy_condition_id_0: i32,
    pub enemy_condition_id_1: i32,
    pub enemy_condition_op_0: i32,
    pub enemy_condition_op_1: i32,
    pub enemy_condition_count_0: i32,
    pub enemy_condition_count_1: i32,
}

impl CreatureLootQuery {
    pub fn is_quest_drop(&self) -> bool {
        self.chance_or_quest_chance < 0.0
    }

    pub fn is_reference(&self) -> bool {
        self.min_count == 0 && self.display_id == 0
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameObjectTemplateQuery {
    pub entry: u32,
    pub object_type: u8,
    pub display_id: u32,
    pub name: String,
    pub icon_name: String,
    pub faction: u32,
    pub flags: u32,
    pub size: f32,
    pub raw_data: [u32; 24],
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameObjectSpawnQuery {
    pub guid: u32,
    pub entry: u32,
    pub map: u32,
    pub game_event: Option<i16>,
    pub guid_pool_id: Option<u16>,
    pub entry_pool_id: Option<u16>,
    pub pool_max_limit: Option<u32>,
    pub pool_chance: f32,
    pub position_x: f32,
    pub position_y: f32,
    pub position_z: f32,
    pub orientation: f32,
    pub rotation0: f32,
    pub rotation1: f32,
    pub rotation2: f32,
    pub rotation3: f32,
    pub spawn_time_secs_min: i32,
    pub spawn_time_secs_max: i32,
    pub state: i8,
    pub anim_progress: u8,
    pub template: GameObjectTemplateQuery,
}

#[derive(Debug, Clone, FromRow, Serialize, Deserialize, PartialEq)]
pub struct GameEventScheduleQuery {
    pub entry: u16,
    pub schedule_type: u32,
    pub occurrence: u32,
    pub length: u32,
    pub holiday: u32,
    pub linked_to: u16,
    pub description: Option<String>,
    pub start_time_unix: Option<i64>,
    pub end_time_unix: Option<i64>,
}

#[derive(Debug, Clone, FromRow, Serialize, Deserialize, PartialEq, Eq)]
pub struct ConditionQuery {
    pub condition_entry: u32,
    pub condition_type: i16,
    pub value1: u32,
    pub value2: u32,
    pub value3: u32,
    pub value4: u32,
    pub flags: u8,
}

const GAMEOBJECT_SPAWN_SELECT: &str =
    "SELECT gameobject.guid, \
                CAST(COALESCE(NULLIF(gameobject.id, 0), gameobject_spawn_entry_choice.entry) AS UNSIGNED) AS entry, \
                gameobject.map, \
                CAST(game_event_gameobject.event AS SIGNED) AS game_event, \
                CAST(pool_gameobject.pool_entry AS UNSIGNED) AS guid_pool_id, \
                CAST(pool_gameobject_template.pool_entry AS UNSIGNED) AS entry_pool_id, \
                CAST(COALESCE(guid_pool_template.max_limit, entry_pool_template.max_limit) AS UNSIGNED) AS pool_max_limit, \
                CAST(COALESCE(pool_gameobject.chance, pool_gameobject_template.chance, 0) AS DOUBLE) AS pool_chance, \
                CAST(gameobject.position_x AS DOUBLE) AS position_x, \
                CAST(gameobject.position_y AS DOUBLE) AS position_y, \
                CAST(gameobject.position_z AS DOUBLE) AS position_z, \
                CAST(gameobject.orientation AS DOUBLE) AS orientation, \
                CAST(gameobject.rotation0 AS DOUBLE) AS rotation0, \
                CAST(gameobject.rotation1 AS DOUBLE) AS rotation1, \
                CAST(gameobject.rotation2 AS DOUBLE) AS rotation2, \
                CAST(gameobject.rotation3 AS DOUBLE) AS rotation3, \
                gameobject.spawntimesecsmin AS spawn_time_secs_min, \
                gameobject.spawntimesecsmax AS spawn_time_secs_max, \
                CAST(COALESCE(gameobject_addon.state, -1) AS SIGNED) AS state, \
                CAST(COALESCE(gameobject_addon.animprogress, 100) AS UNSIGNED) AS anim_progress, \
                gameobject_template.entry AS template_entry, \
                gameobject_template.type AS template_object_type, \
                gameobject_template.displayId AS template_display_id, \
                gameobject_template.name AS template_name, \
                gameobject_template.IconName AS template_icon_name, \
                CAST(gameobject_template.faction AS UNSIGNED) AS template_faction, \
                CAST(gameobject_template.flags AS UNSIGNED) AS template_flags, \
                gameobject_template.size AS template_size, \
                CAST(gameobject_template.data0 AS SIGNED) AS template_data0, \
                CAST(gameobject_template.data1 AS SIGNED) AS template_data1, \
                CAST(gameobject_template.data2 AS SIGNED) AS template_data2, \
                CAST(gameobject_template.data3 AS SIGNED) AS template_data3, \
                CAST(gameobject_template.data4 AS SIGNED) AS template_data4, \
                CAST(gameobject_template.data5 AS SIGNED) AS template_data5, \
                CAST(gameobject_template.data6 AS SIGNED) AS template_data6, \
                CAST(gameobject_template.data7 AS SIGNED) AS template_data7, \
                CAST(gameobject_template.data8 AS SIGNED) AS template_data8, \
                CAST(gameobject_template.data9 AS SIGNED) AS template_data9, \
                CAST(gameobject_template.data10 AS SIGNED) AS template_data10, \
                CAST(gameobject_template.data11 AS SIGNED) AS template_data11, \
                CAST(gameobject_template.data12 AS SIGNED) AS template_data12, \
                CAST(gameobject_template.data13 AS SIGNED) AS template_data13, \
                CAST(gameobject_template.data14 AS SIGNED) AS template_data14, \
                CAST(gameobject_template.data15 AS SIGNED) AS template_data15, \
                CAST(gameobject_template.data16 AS SIGNED) AS template_data16, \
                CAST(gameobject_template.data17 AS SIGNED) AS template_data17, \
                CAST(gameobject_template.data18 AS SIGNED) AS template_data18, \
                CAST(gameobject_template.data19 AS SIGNED) AS template_data19, \
                CAST(gameobject_template.data20 AS SIGNED) AS template_data20, \
                CAST(gameobject_template.data21 AS SIGNED) AS template_data21, \
                CAST(gameobject_template.data22 AS SIGNED) AS template_data22, \
                CAST(gameobject_template.data23 AS SIGNED) AS template_data23 \
         FROM gameobject \
         LEFT JOIN ( \
             SELECT guid, CAST(SUBSTRING_INDEX(GROUP_CONCAT(entry ORDER BY RAND()), ',', 1) AS UNSIGNED) AS entry \
             FROM gameobject_spawn_entry \
             GROUP BY guid \
         ) AS gameobject_spawn_entry_choice ON gameobject.guid = gameobject_spawn_entry_choice.guid \
         JOIN gameobject_template ON COALESCE(NULLIF(gameobject.id, 0), gameobject_spawn_entry_choice.entry) = gameobject_template.entry \
         LEFT JOIN gameobject_addon ON gameobject.guid = gameobject_addon.guid \
         LEFT JOIN game_event_gameobject ON gameobject.guid = game_event_gameobject.guid \
         LEFT JOIN pool_gameobject ON gameobject.guid = pool_gameobject.guid \
         LEFT JOIN pool_gameobject_template ON gameobject.id = pool_gameobject_template.id \
         LEFT JOIN pool_template AS guid_pool_template ON pool_gameobject.pool_entry = guid_pool_template.entry \
         LEFT JOIN pool_template AS entry_pool_template ON pool_gameobject_template.pool_entry = entry_pool_template.entry";

const CREATURE_SPAWN_SELECT: &str = "SELECT creature.guid, \
                CAST(COALESCE(NULLIF(creature.id, 0), creature_spawn_entry_choice.entry) AS UNSIGNED) AS entry, \
                creature.map, \
                CAST(game_event_creature.event AS SIGNED) AS game_event, \
                CAST(pool_creature.pool_entry AS UNSIGNED) AS guid_pool_id, \
                CAST(pool_creature_template.pool_entry AS UNSIGNED) AS entry_pool_id, \
                CAST(COALESCE(guid_pool_template.max_limit, entry_pool_template.max_limit) AS UNSIGNED) AS pool_max_limit, \
                CAST(COALESCE(pool_creature.chance, pool_creature_template.chance, 0) AS DOUBLE) AS pool_chance, \
                CAST(COALESCE(creature_addon.emote, creature_template_addon.emote, 0) AS UNSIGNED) AS addon_emote, \
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
                creature_template.DisplayIdProbability1 AS template_display_id_probability1, \
                creature_template.DisplayIdProbability2 AS template_display_id_probability2, \
                creature_template.DisplayIdProbability3 AS template_display_id_probability3, \
                creature_template.DisplayIdProbability4 AS template_display_id_probability4, \
                CAST(COALESCE(cmi1.gender, 2) AS UNSIGNED) AS template_model_gender1, \
                CAST(COALESCE(cmi2.gender, 2) AS UNSIGNED) AS template_model_gender2, \
                CAST(COALESCE(cmi3.gender, 2) AS UNSIGNED) AS template_model_gender3, \
                CAST(COALESCE(cmi4.gender, 2) AS UNSIGNED) AS template_model_gender4, \
                CAST(COALESCE(cmi1.modelid_other_gender, 0) AS UNSIGNED) AS template_model_other_gender1, \
                CAST(COALESCE(cmi2.modelid_other_gender, 0) AS UNSIGNED) AS template_model_other_gender2, \
                CAST(COALESCE(cmi3.modelid_other_gender, 0) AS UNSIGNED) AS template_model_other_gender3, \
                CAST(COALESCE(cmi4.modelid_other_gender, 0) AS UNSIGNED) AS template_model_other_gender4, \
                CAST(COALESCE(cmi1_other.gender, 2) AS UNSIGNED) AS template_model_other_gender_gender1, \
                CAST(COALESCE(cmi2_other.gender, 2) AS UNSIGNED) AS template_model_other_gender_gender2, \
                CAST(COALESCE(cmi3_other.gender, 2) AS UNSIGNED) AS template_model_other_gender_gender3, \
                CAST(COALESCE(cmi4_other.gender, 2) AS UNSIGNED) AS template_model_other_gender_gender4, \
                CAST(COALESCE(creature_model_info.bounding_radius, 0) AS DOUBLE) AS template_model_bounding_radius, \
                CAST(COALESCE(creature_model_info.combat_reach, 0) AS DOUBLE) AS template_model_combat_reach, \
                creature_template.Faction AS template_faction, creature_template.Scale AS template_scale, \
                creature_template.SpeedWalk AS template_speed_walk, creature_template.SpeedRun AS template_speed_run, \
                creature_template.Detection AS template_detection_range, \
                creature_template.CallForHelp AS template_call_for_help, \
                creature_template.Pursuit AS template_pursuit, \
                creature_template.Leash AS template_leash, \
                creature_template.Family AS template_family, \
                creature_template.CreatureType AS template_creature_type, \
                creature_template.CreatureTypeFlags AS template_creature_type_flags, \
                creature_template.InhabitType AS template_inhabit_type, \
                creature_template.NpcFlags AS template_npc_flags, \
                creature_template.UnitFlags AS template_unit_flags, \
                creature_template.DynamicFlags AS template_dynamic_flags, \
                creature_template.StaticFlags2 AS template_static_flags2, \
                creature_template.ExtraFlags AS template_extra_flags, \
                creature_template.UnitClass AS template_unit_class, \
                CAST(cls.Strength AS UNSIGNED) AS template_base_strength, \
                creature_template.Rank AS template_rank, \
                creature_template.HealthMultiplier AS template_health_multiplier, \
                creature_template.PowerMultiplier AS template_power_multiplier, \
                creature_template.DamageMultiplier AS template_damage_multiplier, \
                creature_template.DamageVariance AS template_damage_variance, \
                creature_template.ArmorMultiplier AS template_armor_multiplier, \
                creature_template.StrengthMultiplier AS template_strength_multiplier, \
                creature_template.MinLevelHealth AS template_min_level_health, \
                creature_template.MaxLevelHealth AS template_max_level_health, \
                creature_template.MinLevelMana AS template_min_level_mana, \
                creature_template.MaxLevelMana AS template_max_level_mana, \
                creature_template.MinMeleeDmg AS template_min_melee_dmg, \
                creature_template.MaxMeleeDmg AS template_max_melee_dmg, \
                creature_template.MinRangedDmg AS template_min_ranged_dmg, \
                creature_template.MaxRangedDmg AS template_max_ranged_dmg, \
                creature_template.Armor AS template_armor, \
                creature_template.ResistanceHoly AS template_resistance_holy, \
                creature_template.ResistanceFire AS template_resistance_fire, \
                creature_template.ResistanceNature AS template_resistance_nature, \
                creature_template.ResistanceFrost AS template_resistance_frost, \
                creature_template.ResistanceShadow AS template_resistance_shadow, \
                creature_template.ResistanceArcane AS template_resistance_arcane, \
                creature_template.MeleeAttackPower AS template_melee_attack_power, \
                creature_template.RangedAttackPower AS template_ranged_attack_power, \
                creature_template.MinLootGold AS template_min_loot_gold, \
                creature_template.MaxLootGold AS template_max_loot_gold, \
                CAST(creature_template.PickpocketLootId AS UNSIGNED) AS template_pickpocket_loot_id, \
                creature_template.MeleeBaseAttackTime AS template_melee_base_attack_time, \
                creature_template.RangedBaseAttackTime AS template_ranged_base_attack_time, \
                creature_template.DamageSchool AS template_damage_school, \
                creature_template.TrainerType AS template_trainer_type, \
                creature_template.TrainerClass AS template_trainer_class, \
                creature_template.PetSpellDataId AS template_pet_spell_data_id, \
                CAST(creature_template.SpellList AS UNSIGNED) AS template_spell_list, \
                creature_template.Civilian AS template_civilian, \
                creature_template.CorpseDecay AS template_corpse_decay, \
                creature_template.MovementType AS template_movement_type, \
                creature_template.EquipmentTemplateId AS template_equipment_template_id, \
                CAST(COALESCE(equip_1.displayid, 0) AS UNSIGNED) AS template_equip_display_id1, \
                CAST(COALESCE(equip_2.displayid, 0) AS UNSIGNED) AS template_equip_display_id2, \
                CAST(COALESCE(equip_3.displayid, 0) AS UNSIGNED) AS template_equip_display_id3, \
                CAST(COALESCE(equip_1.class, 0) AS UNSIGNED) AS template_equip_class1, \
                CAST(COALESCE(equip_2.class, 0) AS UNSIGNED) AS template_equip_class2, \
                CAST(COALESCE(equip_3.class, 0) AS UNSIGNED) AS template_equip_class3, \
                CAST(COALESCE(equip_1.subclass, 0) AS UNSIGNED) AS template_equip_subclass1, \
                CAST(COALESCE(equip_2.subclass, 0) AS UNSIGNED) AS template_equip_subclass2, \
                CAST(COALESCE(equip_3.subclass, 0) AS UNSIGNED) AS template_equip_subclass3, \
                CAST(COALESCE(equip_1.Material, 0) AS SIGNED) AS template_equip_material1, \
                CAST(COALESCE(equip_2.Material, 0) AS SIGNED) AS template_equip_material2, \
                CAST(COALESCE(equip_3.Material, 0) AS SIGNED) AS template_equip_material3, \
                CAST(COALESCE(equip_1.InventoryType, 0) AS UNSIGNED) AS template_equip_inventory_type1, \
                CAST(COALESCE(equip_2.InventoryType, 0) AS UNSIGNED) AS template_equip_inventory_type2, \
                CAST(COALESCE(equip_3.InventoryType, 0) AS UNSIGNED) AS template_equip_inventory_type3, \
                CAST(COALESCE(equip_1.sheath, 0) AS UNSIGNED) AS template_equip_sheath1, \
                CAST(COALESCE(equip_2.sheath, 0) AS UNSIGNED) AS template_equip_sheath2, \
                CAST(COALESCE(equip_3.sheath, 0) AS UNSIGNED) AS template_equip_sheath3, \
                creature_template.ExperienceMultiplier AS template_experience_multiplier \
         FROM creature \
         LEFT JOIN ( \
             SELECT guid, CAST(SUBSTRING_INDEX(GROUP_CONCAT(entry ORDER BY RAND()), ',', 1) AS UNSIGNED) AS entry \
             FROM creature_spawn_entry \
             GROUP BY guid \
         ) AS creature_spawn_entry_choice ON creature.guid = creature_spawn_entry_choice.guid \
         JOIN creature_template ON COALESCE(NULLIF(creature.id, 0), creature_spawn_entry_choice.entry) = creature_template.Entry \
         LEFT JOIN creature_template_classlevelstats AS cls \
           ON cls.Class = creature_template.UnitClass \
          AND cls.Level = CASE \
                WHEN creature_template.MaxLevel > creature_template.MinLevel THEN creature_template.MaxLevel \
                ELSE creature_template.MinLevel \
              END \
         LEFT JOIN game_event_creature ON creature.guid = game_event_creature.guid \
         LEFT JOIN pool_creature ON creature.guid = pool_creature.guid \
         LEFT JOIN pool_creature_template ON creature.id = pool_creature_template.id \
         LEFT JOIN pool_template AS guid_pool_template ON pool_creature.pool_entry = guid_pool_template.entry \
         LEFT JOIN pool_template AS entry_pool_template ON pool_creature_template.pool_entry = entry_pool_template.entry \
         LEFT JOIN creature_addon ON creature.guid = creature_addon.guid \
         LEFT JOIN creature_template_addon ON creature_template.Entry = creature_template_addon.entry \
         LEFT JOIN creature_model_info \
           ON creature_model_info.modelid = COALESCE(NULLIF(creature_template.DisplayId1, 0), NULLIF(creature_template.DisplayId2, 0), NULLIF(creature_template.DisplayId3, 0), NULLIF(creature_template.DisplayId4, 0), 0) \
         LEFT JOIN creature_model_info AS cmi1 ON cmi1.modelid = creature_template.DisplayId1 \
         LEFT JOIN creature_model_info AS cmi2 ON cmi2.modelid = creature_template.DisplayId2 \
         LEFT JOIN creature_model_info AS cmi3 ON cmi3.modelid = creature_template.DisplayId3 \
         LEFT JOIN creature_model_info AS cmi4 ON cmi4.modelid = creature_template.DisplayId4 \
         LEFT JOIN creature_model_info AS cmi1_other ON cmi1_other.modelid = cmi1.modelid_other_gender \
         LEFT JOIN creature_model_info AS cmi2_other ON cmi2_other.modelid = cmi2.modelid_other_gender \
         LEFT JOIN creature_model_info AS cmi3_other ON cmi3_other.modelid = cmi3.modelid_other_gender \
         LEFT JOIN creature_model_info AS cmi4_other ON cmi4_other.modelid = cmi4.modelid_other_gender \
         LEFT JOIN creature_equip_template ON creature_equip_template.entry = creature_template.EquipmentTemplateId \
         LEFT JOIN item_template AS equip_1 ON equip_1.entry = creature_equip_template.equipentry1 \
         LEFT JOIN item_template AS equip_2 ON equip_2.entry = creature_equip_template.equipentry2 \
         LEFT JOIN item_template AS equip_3 ON equip_3.entry = creature_equip_template.equipentry3 \
         LEFT JOIN spawn_group_spawn \
           ON spawn_group_spawn.Guid = creature.guid AND spawn_group_spawn.SlotId = 0 \
         LEFT JOIN spawn_group_formation \
           ON spawn_group_formation.Id = spawn_group_spawn.Id";

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

#[derive(Debug, Clone, FromRow, Serialize, Deserialize, PartialEq)]
pub struct SpellTemplateQuery {
    pub id: u32,
    pub spell_name: String,
    pub rank: Option<String>,
    pub school: u32,
    pub dispel: u32,
    pub mechanic: u32,
    pub attributes: u32,
    pub attributes_ex: u32,
    pub attributes_ex2: u32,
    pub attributes_ex3: u32,
    pub attributes_serverside: u32,
    pub target_creature_type: u32,
    pub interrupt_flags: u32,
    pub aura_interrupt_flags: u32,
    pub channel_interrupt_flags: u32,
    pub caster_aura_state: u32,
    pub target_aura_state: u32,
    pub casting_time_index: u32,
    pub range_index: u32,
    pub speed: f32,
    pub recovery_time: u32,
    pub category: u32,
    pub category_recovery_time: u32,
    pub start_recovery_category: u32,
    pub start_recovery_time: u32,
    pub max_level: u32,
    pub base_level: u32,
    pub spell_level: u32,
    pub power_type: u32,
    pub mana_cost: u32,
    pub mana_cost_per_level: u32,
    pub duration_index: u32,
    pub stack_amount: u32,
    pub effect1: u32,
    pub effect2: u32,
    pub effect3: u32,
    pub effect_base_points1: i32,
    pub effect_base_points2: i32,
    pub effect_base_points3: i32,
    pub effect_die_sides1: i32,
    pub effect_die_sides2: i32,
    pub effect_die_sides3: i32,
    pub effect_base_dice1: u32,
    pub effect_base_dice2: u32,
    pub effect_base_dice3: u32,
    pub effect_dice_per_level1: f32,
    pub effect_dice_per_level2: f32,
    pub effect_dice_per_level3: f32,
    pub effect_real_points_per_level1: f32,
    pub effect_real_points_per_level2: f32,
    pub effect_real_points_per_level3: f32,
    pub effect_points_per_combo_point1: f32,
    pub effect_points_per_combo_point2: f32,
    pub effect_points_per_combo_point3: f32,
    pub effect_bonus_coefficient1: f32,
    pub effect_bonus_coefficient2: f32,
    pub effect_bonus_coefficient3: f32,
    pub effect_multiple_value1: f32,
    pub effect_multiple_value2: f32,
    pub effect_multiple_value3: f32,
    pub effect_misc_value1: i32,
    pub effect_misc_value2: i32,
    pub effect_misc_value3: i32,
    pub effect_trigger_spell1: u32,
    pub effect_trigger_spell2: u32,
    pub effect_trigger_spell3: u32,
    pub effect_apply_aura_name1: u32,
    pub effect_apply_aura_name2: u32,
    pub effect_apply_aura_name3: u32,
    pub effect_amplitude1: u32,
    pub effect_amplitude2: u32,
    pub effect_amplitude3: u32,
    pub effect_mechanic1: u32,
    pub effect_mechanic2: u32,
    pub effect_mechanic3: u32,
    pub effect_implicit_target_a1: u32,
    pub effect_implicit_target_a2: u32,
    pub effect_implicit_target_a3: u32,
    pub effect_implicit_target_b1: u32,
    pub effect_implicit_target_b2: u32,
    pub effect_implicit_target_b3: u32,
    pub effect_chain_target1: u32,
    pub effect_chain_target2: u32,
    pub effect_chain_target3: u32,
    pub effect_radius_index1: u32,
    pub effect_radius_index2: u32,
    pub effect_radius_index3: u32,
    pub max_affected_targets: u32,
    pub effect_item_type1: u32,
    pub effect_item_type2: u32,
    pub effect_item_type3: u32,
    pub equipped_item_class: i32,
    pub equipped_item_subclass_mask: i32,
    pub spell_family_name: u32,
    pub spell_family_flags: u64,
    pub dmg_class: u32,
    pub proc_flags: u32,
    pub proc_chance: u32,
    pub proc_charges: u32,
}

#[derive(Debug, Clone, Copy, FromRow, Serialize, Deserialize, PartialEq, Eq)]
pub struct SpellChainQuery {
    pub spell_id: u32,
    pub prev_spell: u32,
    pub first_spell: u32,
    pub rank: u8,
    pub req_spell: u32,
}

#[derive(Debug, Clone, Copy, FromRow, Serialize, Deserialize, PartialEq)]
pub struct SpellTargetPositionQuery {
    pub id: u32,
    pub target_map: u32,
    pub target_position_x: f32,
    pub target_position_y: f32,
    pub target_position_z: f32,
    pub target_orientation: f32,
}

#[derive(Debug, Clone, Copy, FromRow, Serialize, Deserialize, PartialEq, Eq)]
pub struct SpellGroupMembershipQuery {
    pub spell_id: u32,
    pub group_id: u32,
    pub rule: u32,
}

#[derive(Debug, Clone, Copy, FromRow, Serialize, Deserialize, PartialEq)]
pub struct SpellProcEventQuery {
    pub entry: u32,
    pub proc_ex: u32,
    pub custom_chance: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuestTemplateQuery {
    pub entry: u32,
    pub method: u32,
    pub zone_or_sort: i16,
    pub min_level: u8,
    pub max_level: u8,
    pub quest_level: u32,
    pub quest_type: u32,
    pub required_classes: u32,
    pub required_races: u32,
    pub required_skill: u32,
    pub required_skill_value: u32,
    pub required_condition: u32,
    pub rep_objective_faction: u32,
    pub rep_objective_value: i32,
    pub required_min_rep_faction: u32,
    pub required_min_rep_value: i32,
    pub required_max_rep_faction: u32,
    pub required_max_rep_value: i32,
    pub special_flags: u32,
    pub prev_quest_id: i32,
    pub next_quest_id: i32,
    pub exclusive_group: i32,
    pub next_quest_in_chain: u32,
    pub rew_or_req_money: i32,
    pub rew_money_max_level: u32,
    pub rew_spell: u32,
    pub rew_spell_cast: u32,
    pub src_item_id: u32,
    pub src_item_count: u32,
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
    pub req_source_id: [u32; 4],
    pub req_source_count: [u32; 4],
    pub rew_choice_item_id: [u32; 6],
    pub rew_choice_item_count: [u32; 6],
    pub rew_item_id: [u32; 4],
    pub rew_item_count: [u32; 4],
    pub rew_rep_faction: [u32; 5],
    pub rew_rep_value: [i32; 5],
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
    pub fn is_repeatable(&self) -> bool {
        (self.special_flags & 0x1) != 0
    }

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
    let _query_timer = crate::observability::DbQueryTimer::start("creature_template_load");
    let row = sqlx::query_as::<_, CreatureTemplateQuery>(
        "SELECT creature_template.Entry AS entry, creature_template.Name AS name, creature_template.SubName AS subname, \
                creature_template.MinLevel AS min_level, creature_template.MaxLevel AS max_level, \
                creature_template.DisplayId1 AS display_id1, creature_template.DisplayId2 AS display_id2, \
                creature_template.DisplayId3 AS display_id3, creature_template.DisplayId4 AS display_id4, \
                creature_template.DisplayIdProbability1 AS display_id_probability1, \
                creature_template.DisplayIdProbability2 AS display_id_probability2, \
                creature_template.DisplayIdProbability3 AS display_id_probability3, \
                creature_template.DisplayIdProbability4 AS display_id_probability4, \
                CAST(COALESCE(cmi1.gender, 2) AS UNSIGNED) AS model_gender1, \
                CAST(COALESCE(cmi2.gender, 2) AS UNSIGNED) AS model_gender2, \
                CAST(COALESCE(cmi3.gender, 2) AS UNSIGNED) AS model_gender3, \
                CAST(COALESCE(cmi4.gender, 2) AS UNSIGNED) AS model_gender4, \
                CAST(COALESCE(cmi1.modelid_other_gender, 0) AS UNSIGNED) AS model_other_gender1, \
                CAST(COALESCE(cmi2.modelid_other_gender, 0) AS UNSIGNED) AS model_other_gender2, \
                CAST(COALESCE(cmi3.modelid_other_gender, 0) AS UNSIGNED) AS model_other_gender3, \
                CAST(COALESCE(cmi4.modelid_other_gender, 0) AS UNSIGNED) AS model_other_gender4, \
                CAST(COALESCE(cmi1_other.gender, 2) AS UNSIGNED) AS model_other_gender_gender1, \
                CAST(COALESCE(cmi2_other.gender, 2) AS UNSIGNED) AS model_other_gender_gender2, \
                CAST(COALESCE(cmi3_other.gender, 2) AS UNSIGNED) AS model_other_gender_gender3, \
                CAST(COALESCE(cmi4_other.gender, 2) AS UNSIGNED) AS model_other_gender_gender4, \
                CAST(COALESCE(creature_model_info.bounding_radius, 0) AS DOUBLE) AS model_bounding_radius, \
                CAST(COALESCE(creature_model_info.combat_reach, 0) AS DOUBLE) AS model_combat_reach, \
                creature_template.Faction AS faction, creature_template.Scale AS scale, \
                creature_template.SpeedWalk AS speed_walk, creature_template.SpeedRun AS speed_run, \
                creature_template.Detection AS detection_range, \
                creature_template.CallForHelp AS call_for_help, creature_template.Pursuit AS pursuit, creature_template.Leash AS leash, \
                creature_template.Family AS family, \
                creature_template.CreatureType AS creature_type, \
                creature_template.CreatureTypeFlags AS creature_type_flags, \
                creature_template.InhabitType AS inhabit_type, \
                creature_template.NpcFlags AS npc_flags, \
                creature_template.UnitFlags AS unit_flags, creature_template.DynamicFlags AS dynamic_flags, \
                creature_template.StaticFlags2 AS static_flags2, \
                creature_template.ExtraFlags AS extra_flags, \
                creature_template.UnitClass AS unit_class, creature_template.Rank AS rank, \
                CAST(cls.Strength AS UNSIGNED) AS base_strength, \
                creature_template.HealthMultiplier AS health_multiplier, creature_template.PowerMultiplier AS power_multiplier, \
                creature_template.DamageMultiplier AS damage_multiplier, creature_template.DamageVariance AS damage_variance, \
                creature_template.ArmorMultiplier AS armor_multiplier, \
                creature_template.StrengthMultiplier AS strength_multiplier, \
                creature_template.MinLevelHealth AS min_level_health, creature_template.MaxLevelHealth AS max_level_health, \
                creature_template.MinLevelMana AS min_level_mana, creature_template.MaxLevelMana AS max_level_mana, \
                creature_template.MinMeleeDmg AS min_melee_dmg, creature_template.MaxMeleeDmg AS max_melee_dmg, \
                creature_template.MinRangedDmg AS min_ranged_dmg, creature_template.MaxRangedDmg AS max_ranged_dmg, \
                creature_template.Armor AS armor, \
                creature_template.ResistanceHoly AS resistance_holy, \
                creature_template.ResistanceFire AS resistance_fire, \
                creature_template.ResistanceNature AS resistance_nature, \
                creature_template.ResistanceFrost AS resistance_frost, \
                creature_template.ResistanceShadow AS resistance_shadow, \
                creature_template.ResistanceArcane AS resistance_arcane, \
                creature_template.MeleeAttackPower AS melee_attack_power, \
                creature_template.RangedAttackPower AS ranged_attack_power, \
                creature_template.MinLootGold AS min_loot_gold, creature_template.MaxLootGold AS max_loot_gold, \
                CAST(creature_template.PickpocketLootId AS UNSIGNED) AS pickpocket_loot_id, \
                creature_template.MeleeBaseAttackTime AS melee_base_attack_time, \
                creature_template.RangedBaseAttackTime AS ranged_base_attack_time, \
                creature_template.DamageSchool AS damage_school, \
                creature_template.TrainerType AS trainer_type, creature_template.TrainerClass AS trainer_class, \
                creature_template.PetSpellDataId AS pet_spell_data_id, creature_template.Civilian AS civilian, \
                CAST(creature_template.SpellList AS UNSIGNED) AS spell_list, \
                creature_template.CorpseDecay AS corpse_decay, \
                creature_template.MovementType AS movement_type, creature_template.EquipmentTemplateId AS equipment_template_id, \
                CAST(COALESCE(equip_1.displayid, 0) AS UNSIGNED) AS equip_display_id1, \
                CAST(COALESCE(equip_2.displayid, 0) AS UNSIGNED) AS equip_display_id2, \
                CAST(COALESCE(equip_3.displayid, 0) AS UNSIGNED) AS equip_display_id3, \
                CAST(COALESCE(equip_1.class, 0) AS UNSIGNED) AS equip_class1, \
                CAST(COALESCE(equip_2.class, 0) AS UNSIGNED) AS equip_class2, \
                CAST(COALESCE(equip_3.class, 0) AS UNSIGNED) AS equip_class3, \
                CAST(COALESCE(equip_1.subclass, 0) AS UNSIGNED) AS equip_subclass1, \
                CAST(COALESCE(equip_2.subclass, 0) AS UNSIGNED) AS equip_subclass2, \
                CAST(COALESCE(equip_3.subclass, 0) AS UNSIGNED) AS equip_subclass3, \
                CAST(COALESCE(equip_1.Material, 0) AS SIGNED) AS equip_material1, \
                CAST(COALESCE(equip_2.Material, 0) AS SIGNED) AS equip_material2, \
                CAST(COALESCE(equip_3.Material, 0) AS SIGNED) AS equip_material3, \
                CAST(COALESCE(equip_1.InventoryType, 0) AS UNSIGNED) AS equip_inventory_type1, \
                CAST(COALESCE(equip_2.InventoryType, 0) AS UNSIGNED) AS equip_inventory_type2, \
                CAST(COALESCE(equip_3.InventoryType, 0) AS UNSIGNED) AS equip_inventory_type3, \
                CAST(COALESCE(equip_1.sheath, 0) AS UNSIGNED) AS equip_sheath1, \
                CAST(COALESCE(equip_2.sheath, 0) AS UNSIGNED) AS equip_sheath2, \
                CAST(COALESCE(equip_3.sheath, 0) AS UNSIGNED) AS equip_sheath3, \
                creature_template.ExperienceMultiplier AS experience_multiplier \
         FROM creature_template \
         LEFT JOIN creature_template_classlevelstats AS cls \
           ON cls.Class = creature_template.UnitClass \
          AND cls.Level = CASE \
                WHEN creature_template.MaxLevel > creature_template.MinLevel THEN creature_template.MaxLevel \
                ELSE creature_template.MinLevel \
              END \
         LEFT JOIN creature_model_info \
           ON creature_model_info.modelid = COALESCE(NULLIF(creature_template.DisplayId1, 0), NULLIF(creature_template.DisplayId2, 0), NULLIF(creature_template.DisplayId3, 0), NULLIF(creature_template.DisplayId4, 0), 0) \
         LEFT JOIN creature_model_info AS cmi1 ON cmi1.modelid = creature_template.DisplayId1 \
         LEFT JOIN creature_model_info AS cmi2 ON cmi2.modelid = creature_template.DisplayId2 \
         LEFT JOIN creature_model_info AS cmi3 ON cmi3.modelid = creature_template.DisplayId3 \
         LEFT JOIN creature_model_info AS cmi4 ON cmi4.modelid = creature_template.DisplayId4 \
         LEFT JOIN creature_model_info AS cmi1_other ON cmi1_other.modelid = cmi1.modelid_other_gender \
         LEFT JOIN creature_model_info AS cmi2_other ON cmi2_other.modelid = cmi2.modelid_other_gender \
         LEFT JOIN creature_model_info AS cmi3_other ON cmi3_other.modelid = cmi3.modelid_other_gender \
         LEFT JOIN creature_model_info AS cmi4_other ON cmi4_other.modelid = cmi4.modelid_other_gender \
         LEFT JOIN creature_equip_template ON creature_equip_template.entry = creature_template.EquipmentTemplateId \
         LEFT JOIN item_template AS equip_1 ON equip_1.entry = creature_equip_template.equipentry1 \
         LEFT JOIN item_template AS equip_2 ON equip_2.entry = creature_equip_template.equipentry2 \
         LEFT JOIN item_template AS equip_3 ON equip_3.entry = creature_equip_template.equipentry3 \
         WHERE creature_template.Entry = ?",
    )
    .bind(entry)
    .fetch_optional(pool)
    .await?;

    Ok(row)
}

pub async fn get_spell_template_query(
    pool: &MySqlPool,
    spell: u32,
) -> Result<Option<SpellTemplateQuery>, DbError> {
    let _query_timer = crate::observability::DbQueryTimer::start("spell_template_load");
    let mut row = sqlx::query_as::<_, SpellTemplateQuery>(
        "SELECT Id AS id, SpellName AS spell_name, Rank1 AS rank, School AS school, \
                Dispel AS dispel, Mechanic AS mechanic, \
                Attributes AS attributes, AttributesEx AS attributes_ex, CastingTimeIndex AS casting_time_index, \
                RangeIndex AS range_index, \
                Speed AS speed, \
                AttributesEx2 AS attributes_ex2, AttributesEx3 AS attributes_ex3, \
                AttributesServerside AS attributes_serverside, \
                TargetCreatureType AS target_creature_type, \
                InterruptFlags AS interrupt_flags, AuraInterruptFlags AS aura_interrupt_flags, \
                ChannelInterruptFlags AS channel_interrupt_flags, \
                CasterAuraState AS caster_aura_state, TargetAuraState AS target_aura_state, \
                RecoveryTime AS recovery_time, Category AS category, CategoryRecoveryTime AS category_recovery_time, \
                StartRecoveryCategory AS start_recovery_category, StartRecoveryTime AS start_recovery_time, \
                MaxLevel AS max_level, BaseLevel AS base_level, SpellLevel AS spell_level, \
                PowerType AS power_type, ManaCost AS mana_cost, ManaCostPerlevel AS mana_cost_per_level, DurationIndex AS duration_index, \
                StackAmount AS stack_amount, \
                Effect1 AS effect1, Effect2 AS effect2, Effect3 AS effect3, \
                EffectBasePoints1 AS effect_base_points1, EffectBasePoints2 AS effect_base_points2, \
                EffectBasePoints3 AS effect_base_points3, \
                EffectDieSides1 AS effect_die_sides1, EffectDieSides2 AS effect_die_sides2, \
                EffectDieSides3 AS effect_die_sides3, \
                EffectBaseDice1 AS effect_base_dice1, EffectBaseDice2 AS effect_base_dice2, \
                EffectBaseDice3 AS effect_base_dice3, \
                EffectDicePerLevel1 AS effect_dice_per_level1, \
                EffectDicePerLevel2 AS effect_dice_per_level2, \
                EffectDicePerLevel3 AS effect_dice_per_level3, \
                EffectRealPointsPerLevel1 AS effect_real_points_per_level1, \
                EffectRealPointsPerLevel2 AS effect_real_points_per_level2, \
                EffectRealPointsPerLevel3 AS effect_real_points_per_level3, \
                EffectPointsPerComboPoint1 AS effect_points_per_combo_point1, \
                EffectPointsPerComboPoint2 AS effect_points_per_combo_point2, \
                EffectPointsPerComboPoint3 AS effect_points_per_combo_point3, \
                CAST(0 AS DOUBLE) AS effect_bonus_coefficient1, \
                CAST(0 AS DOUBLE) AS effect_bonus_coefficient2, \
                CAST(0 AS DOUBLE) AS effect_bonus_coefficient3, \
                EffectMultipleValue1 AS effect_multiple_value1, \
                EffectMultipleValue2 AS effect_multiple_value2, \
                EffectMultipleValue3 AS effect_multiple_value3, \
                EffectMiscValue1 AS effect_misc_value1, EffectMiscValue2 AS effect_misc_value2, \
                EffectMiscValue3 AS effect_misc_value3, \
                EffectTriggerSpell1 AS effect_trigger_spell1, \
                EffectTriggerSpell2 AS effect_trigger_spell2, \
                EffectTriggerSpell3 AS effect_trigger_spell3, \
                EffectApplyAuraName1 AS effect_apply_aura_name1, \
                EffectApplyAuraName2 AS effect_apply_aura_name2, \
                EffectApplyAuraName3 AS effect_apply_aura_name3, \
                EffectAmplitude1 AS effect_amplitude1, EffectAmplitude2 AS effect_amplitude2, \
                EffectAmplitude3 AS effect_amplitude3, \
                EffectMechanic1 AS effect_mechanic1, EffectMechanic2 AS effect_mechanic2, \
                EffectMechanic3 AS effect_mechanic3, \
                EffectImplicitTargetA1 AS effect_implicit_target_a1, \
                EffectImplicitTargetA2 AS effect_implicit_target_a2, \
                EffectImplicitTargetA3 AS effect_implicit_target_a3, \
                EffectImplicitTargetB1 AS effect_implicit_target_b1, \
                EffectImplicitTargetB2 AS effect_implicit_target_b2, \
                EffectImplicitTargetB3 AS effect_implicit_target_b3, \
                EffectChainTarget1 AS effect_chain_target1, \
                EffectChainTarget2 AS effect_chain_target2, \
                EffectChainTarget3 AS effect_chain_target3, \
                EffectRadiusIndex1 AS effect_radius_index1, \
                EffectRadiusIndex2 AS effect_radius_index2, \
                EffectRadiusIndex3 AS effect_radius_index3, \
                MaxAffectedTargets AS max_affected_targets, \
                EffectItemType1 AS effect_item_type1, \
                EffectItemType2 AS effect_item_type2, \
                EffectItemType3 AS effect_item_type3, \
                EquippedItemClass AS equipped_item_class, \
                EquippedItemSubClassMask AS equipped_item_subclass_mask, \
                SpellFamilyName AS spell_family_name, SpellFamilyFlags AS spell_family_flags, \
                DmgClass AS dmg_class, procFlags AS proc_flags, procChance AS proc_chance, \
                procCharges AS proc_charges \
         FROM spell_template WHERE Id = ?",
    )
    .bind(spell)
    .fetch_optional(pool)
    .await?;

    if let Some(template) = row.as_mut() {
        apply_spell_template_attribute_fixes(template);
        apply_spell_effect_bonus_coefficient_fixes(template);
    }

    Ok(row)
}

pub async fn get_spell_proc_event_query(
    pool: &MySqlPool,
    spell: u32,
) -> Result<Option<SpellProcEventQuery>, DbError> {
    let _query_timer = crate::observability::DbQueryTimer::start("spell_proc_event_load");
    sqlx::query_as::<_, SpellProcEventQuery>(
        "SELECT entry, procEx AS proc_ex, CustomChance AS custom_chance \
         FROM spell_proc_event WHERE entry = ?",
    )
    .bind(spell)
    .fetch_optional(pool)
    .await
    .map_err(Into::into)
}

fn apply_spell_effect_bonus_coefficient_fixes(template: &mut SpellTemplateQuery) {
    let Some(coefficients) = spell_effect_bonus_coefficient_fixes().get(&template.id) else {
        return;
    };
    template.effect_bonus_coefficient1 = coefficients[0];
    template.effect_bonus_coefficient2 = coefficients[1];
    template.effect_bonus_coefficient3 = coefficients[2];
}

fn apply_spell_template_attribute_fixes(template: &mut SpellTemplateQuery) {
    const SPELL_ATTR_EX_INITIATES_COMBAT_ENABLES_AUTO_ATTACK: u32 = 0x0000_0200;
    const SPELL_ATTR_EX2_INITIATE_COMBAT_POST_CAST: u32 = 0x0010_0000;

    match template.id {
        // The local spell_template export drops the standard Rogue opener bits
        // that packed CMaNGOS Spell.sql keeps for Cheap Shot.
        1833 | 8621 | 11293 | 11294 => {
            template.attributes_ex |= SPELL_ATTR_EX_INITIATES_COMBAT_ENABLES_AUTO_ATTACK;
            template.attributes_ex2 |= SPELL_ATTR_EX2_INITIATE_COMBAT_POST_CAST;
        }
        _ => {}
    }
}

fn spell_effect_bonus_coefficient_fixes() -> &'static HashMap<u32, [f32; 3]> {
    SPELL_EFFECT_BONUS_COEFFICIENT_FIXES.get_or_init(|| {
        let mut fixes = HashMap::new();

        for line in CMANGOS_SPELL_FIXES_SQL.lines() {
            let line = line.trim();
            let Some(line) = line.strip_prefix("UPDATE spell_template SET EffectBonusCoefficient")
            else {
                continue;
            };
            let Some((coefficient_part, id_part)) = line.split_once(" WHERE Id IN (") else {
                continue;
            };
            let Some((slot, value)) = coefficient_part.split_once('=') else {
                continue;
            };
            let Ok(slot) = slot.trim().parse::<usize>() else {
                continue;
            };
            if !(1..=3).contains(&slot) {
                continue;
            }
            let Ok(value) = value.trim().parse::<f32>() else {
                continue;
            };
            let Some((ids, _)) = id_part.split_once(')') else {
                continue;
            };
            for id in ids.split(',') {
                let Ok(id) = id.trim().parse::<u32>() else {
                    continue;
                };
                fixes.entry(id).or_insert([0.0; 3])[slot - 1] = value;
            }
        }

        fixes
    })
}

pub async fn get_page_text_query(
    pool: &MySqlPool,
    page_text_id: u32,
) -> Result<Option<PageTextQuery>, DbError> {
    let row = sqlx::query_as::<_, PageTextQuery>(
        "SELECT entry, text, next_page FROM page_text WHERE entry = ?",
    )
    .bind(page_text_id)
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

pub async fn get_creature_gossip_menu_query(
    pool: &MySqlPool,
    creature_entry: u32,
) -> Result<Option<GossipMenuQuery>, DbError> {
    let row = sqlx::query_as::<_, GossipMenuQuery>(
        "SELECT gm.entry, gm.text_id, gm.script_id, gm.condition_id \
         FROM creature_template ct \
         JOIN gossip_menu gm ON gm.entry = ct.GossipMenuId \
         WHERE ct.Entry = ? \
         ORDER BY gm.entry LIMIT 1",
    )
    .bind(creature_entry)
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

pub async fn get_creature_gossip_menu_id(
    pool: &MySqlPool,
    creature_entry: u32,
) -> Result<Option<u32>, DbError> {
    sqlx::query_scalar("SELECT GossipMenuId FROM creature_template WHERE Entry = ?")
        .bind(creature_entry)
        .fetch_optional(pool)
        .await
        .map_err(Into::into)
}

pub async fn get_gossip_menu_queries(
    pool: &MySqlPool,
    menu_id: u32,
) -> Result<Vec<GossipMenuQuery>, DbError> {
    sqlx::query_as::<_, GossipMenuQuery>(
        "SELECT entry, text_id, script_id, condition_id \
         FROM gossip_menu \
         WHERE entry = ? \
         ORDER BY condition_id ASC, text_id ASC",
    )
    .bind(menu_id)
    .fetch_all(pool)
    .await
    .map_err(Into::into)
}

pub async fn get_gossip_menu_option_queries(
    pool: &MySqlPool,
    menu_id: u32,
) -> Result<Vec<GossipMenuOptionQuery>, DbError> {
    sqlx::query_as::<_, GossipMenuOptionQuery>(
        "SELECT menu_id, id, option_icon, option_text, option_id, npc_option_npcflag, \
                action_menu_id, action_poi_id, action_script_id, box_coded, box_text, condition_id \
         FROM gossip_menu_option \
         WHERE menu_id = ? \
         ORDER BY id ASC",
    )
    .bind(menu_id)
    .fetch_all(pool)
    .await
    .map_err(Into::into)
}

pub async fn get_npc_text_query(
    pool: &MySqlPool,
    text_id: u32,
) -> Result<Option<NpcTextQuery>, DbError> {
    let row =
        sqlx::query_as::<_, NpcTextQuery>("SELECT ID, text0_0, text0_1 FROM npc_text WHERE ID = ?")
            .bind(text_id)
            .fetch_optional(pool)
            .await?;
    Ok(row)
}

pub async fn has_npc_text_query(pool: &MySqlPool, text_id: u32) -> Result<bool, DbError> {
    if get_npc_text_query(pool, text_id).await?.is_some() {
        return Ok(true);
    }
    if !world_table_exists(pool, "npc_text_broadcast_text").await? {
        return Ok(false);
    }

    let count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM npc_text_broadcast_text WHERE Id = ?")
            .bind(text_id)
            .fetch_one(pool)
            .await?;
    Ok(count > 0)
}

pub async fn get_npc_text_primary_query(
    pool: &MySqlPool,
    text_id: u32,
) -> Result<Option<String>, DbError> {
    if let Some(row) = get_npc_text_query(pool, text_id).await? {
        let text = if row.text0_0.is_empty() {
            row.text0_1
        } else {
            row.text0_0
        };
        return Ok((!text.is_empty()).then_some(text));
    }

    if !world_table_exists(pool, "npc_text_broadcast_text").await?
        || !world_table_exists(pool, "broadcast_text").await?
    {
        return Ok(None);
    }

    let text: Option<String> = sqlx::query_scalar(
        "SELECT COALESCE(NULLIF(bt.Text, ''), bt.Text1) \
         FROM npc_text_broadcast_text nt \
         JOIN broadcast_text bt ON bt.Id = nt.BroadcastTextId0 \
         WHERE nt.Id = ? AND nt.BroadcastTextId0 <> 0 \
         LIMIT 1",
    )
    .bind(text_id)
    .fetch_optional(pool)
    .await?
    .flatten();
    Ok(text.filter(|text| !text.is_empty()))
}

pub async fn get_spell_chain_query(
    pool: &MySqlPool,
    spell: u32,
) -> Result<Option<SpellChainQuery>, DbError> {
    let _query_timer = crate::observability::DbQueryTimer::start("spell_chain_load");
    sqlx::query_as::<_, SpellChainQuery>(
        "SELECT CAST(spell_id AS UNSIGNED) AS spell_id, \
                CAST(prev_spell AS UNSIGNED) AS prev_spell, \
                CAST(first_spell AS UNSIGNED) AS first_spell, \
                CAST(rank AS UNSIGNED) AS rank, \
                CAST(req_spell AS UNSIGNED) AS req_spell \
         FROM spell_chain WHERE spell_id = ?",
    )
    .bind(spell)
    .fetch_optional(pool)
    .await
    .map_err(Into::into)
}

pub async fn get_spell_target_position_query(
    pool: &MySqlPool,
    spell: u32,
) -> Result<Option<SpellTargetPositionQuery>, DbError> {
    let _query_timer = crate::observability::DbQueryTimer::start("spell_target_position_load");
    sqlx::query_as::<_, SpellTargetPositionQuery>(
        "SELECT CAST(id AS UNSIGNED) AS id, \
                CAST(target_map AS UNSIGNED) AS target_map, \
                target_position_x, \
                target_position_y, \
                target_position_z, \
                target_orientation \
         FROM spell_target_position WHERE id = ?",
    )
    .bind(spell)
    .fetch_optional(pool)
    .await
    .map_err(Into::into)
}

pub async fn get_spell_facing_flag_query(
    pool: &MySqlPool,
    spell: u32,
) -> Result<Option<u32>, DbError> {
    let _query_timer = crate::observability::DbQueryTimer::start("spell_facing_load");
    sqlx::query_scalar::<_, u32>(
        "SELECT CAST(facingcasterflag AS UNSIGNED) AS facingcasterflag \
         FROM spell_facing WHERE entry = ?",
    )
    .bind(spell)
    .fetch_optional(pool)
    .await
    .map_err(Into::into)
}

pub async fn get_spell_group_memberships(
    pool: &MySqlPool,
    spell: u32,
) -> Result<Vec<SpellGroupMembershipQuery>, DbError> {
    let _query_timer = crate::observability::DbQueryTimer::start("spell_group_membership_load");
    sqlx::query_as::<_, SpellGroupMembershipQuery>(
        "SELECT CAST(spell_group_spell.SpellId AS UNSIGNED) AS spell_id, \
                CAST(spell_group_spell.Id AS UNSIGNED) AS group_id, \
                CAST(spell_group.Rule AS UNSIGNED) AS rule \
         FROM spell_group_spell \
         INNER JOIN spell_group ON spell_group.Id = spell_group_spell.Id \
         WHERE spell_group_spell.SpellId = ?",
    )
    .bind(spell)
    .fetch_all(pool)
    .await
    .map_err(Into::into)
}

pub async fn get_creature_spell_list(
    pool: &MySqlPool,
    list_id: u32,
) -> Result<Vec<CreatureSpellListQuery>, DbError> {
    let _query_timer = crate::observability::DbQueryTimer::start("creature_spell_list_load");
    sqlx::query_as::<_, CreatureSpellListQuery>(
        "SELECT CAST(entry.Id AS UNSIGNED) AS id, \
                CAST(entry.ChanceSupportAction AS UNSIGNED) AS chance_support_action, \
                CAST(entry.ChanceRangedAttack AS UNSIGNED) AS chance_ranged_attack, \
                CAST(list.Position AS UNSIGNED) AS position, \
                CAST(list.SpellId AS UNSIGNED) AS spell_id, \
                CAST(list.Flags AS UNSIGNED) AS flags, \
                list.CombatCondition AS combat_condition, \
                CAST(list.TargetId AS UNSIGNED) AS target_id, \
                CAST(list.ScriptId AS UNSIGNED) AS script_id, \
                CAST(list.Availability AS UNSIGNED) AS availability, \
                CAST(list.Probability AS UNSIGNED) AS probability, \
                CAST(list.InitialMin AS UNSIGNED) AS initial_min, \
                CAST(list.InitialMax AS UNSIGNED) AS initial_max, \
                CAST(list.RepeatMin AS UNSIGNED) AS repeat_min, \
                CAST(list.RepeatMax AS UNSIGNED) AS repeat_max, \
                CAST(COALESCE(template.RecoveryTime, 0) AS UNSIGNED) AS recovery_time, \
                CAST(COALESCE(template.Category, 0) AS UNSIGNED) AS category, \
                CAST(COALESCE(template.CategoryRecoveryTime, 0) AS UNSIGNED) AS category_recovery_time, \
                CAST(COALESCE(target.Type, 0) AS UNSIGNED) AS target_type, \
                COALESCE(target.Param1, 0) AS target_param1, \
                COALESCE(target.Param2, 0) AS target_param2, \
                COALESCE(target.Param3, 0) AS target_param3, \
                COALESCE(target.UnitCondition, -1) AS target_unit_condition \
         FROM creature_spell_list_entry entry \
         JOIN creature_spell_list list ON list.Id = entry.Id \
         LEFT JOIN spell_template template ON template.Id = list.SpellId \
         LEFT JOIN creature_spell_targeting target ON target.Id = list.TargetId \
         WHERE entry.Id = ? \
         ORDER BY list.Position",
    )
    .bind(list_id)
    .fetch_all(pool)
    .await
    .map_err(DbError::from)
}

pub async fn get_creature_ai_scripts_for_entry(
    pool: &MySqlPool,
    entry: u32,
) -> Result<Vec<CreatureAiScriptQuery>, DbError> {
    let _query_timer = crate::observability::DbQueryTimer::start("creature_ai_scripts_load");
    sqlx::query_as::<_, CreatureAiScriptQuery>(
        "SELECT id, creature_id, event_type, event_chance, event_flags, \
                event_param1, event_param2, event_param3, event_param4, event_param5, event_param6, \
                action1_type, action1_param1, action1_param2, action1_param3, \
                action2_type, action2_param1, action2_param2, action2_param3, \
                action3_type, action3_param1, action3_param2, action3_param3 \
         FROM creature_ai_scripts \
         WHERE creature_id = ? \
         ORDER BY id",
    )
    .bind(entry as i32)
    .fetch_all(pool)
    .await
    .map_err(DbError::from)
}

async fn world_table_exists(pool: &MySqlPool, table_name: &str) -> Result<bool, DbError> {
    let table_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) \
         FROM information_schema.tables \
         WHERE table_schema = DATABASE() AND table_name = ?",
    )
    .bind(table_name)
    .fetch_one(pool)
    .await?;
    Ok(table_count != 0)
}

pub async fn get_exploration_base_xp_rows(
    pool: &MySqlPool,
) -> Result<Vec<ExplorationBaseXpQuery>, DbError> {
    let _query_timer = crate::observability::DbQueryTimer::start("exploration_base_xp_load");
    let rows = sqlx::query_as::<_, ExplorationBaseXpQuery>(
        "SELECT CAST(level AS UNSIGNED) AS level, CAST(basexp AS UNSIGNED) AS basexp \
         FROM exploration_basexp",
    )
    .fetch_all(pool)
    .await?;

    Ok(rows)
}

pub async fn get_creature_loot_items(
    pool: &MySqlPool,
    creature_entry: u32,
) -> Result<Vec<CreatureLootQuery>, DbError> {
    let _query_timer = crate::observability::DbQueryTimer::start("creature_loot_load");
    let rows = sqlx::query_as::<_, CreatureLootRow>(
        "SELECT creature_loot_template.item, \
                creature_loot_template.groupid AS group_id, \
                CAST(CASE WHEN creature_loot_template.mincountOrRef < 0 THEN 0 \
                     ELSE GREATEST(creature_loot_template.mincountOrRef, 1) END AS UNSIGNED) AS min_count, \
                CAST(CASE WHEN creature_loot_template.mincountOrRef < 0 THEN GREATEST(creature_loot_template.maxcount, 1) \
                     ELSE GREATEST(creature_loot_template.maxcount, creature_loot_template.mincountOrRef, 1) END AS UNSIGNED) AS max_count, \
                CAST(COALESCE(item_template.displayid, 0) AS UNSIGNED) AS display_id, \
                creature_loot_template.ChanceOrQuestChance AS chance_or_quest_chance \
         FROM creature_loot_template \
         LEFT JOIN item_template \
           ON creature_loot_template.item = item_template.entry \
          AND creature_loot_template.mincountOrRef > 0 \
         WHERE creature_loot_template.entry = ? \
           AND creature_loot_template.condition_id = 0 \
           AND (creature_loot_template.mincountOrRef < 0 OR item_template.entry IS NOT NULL) \
         ORDER BY creature_loot_template.groupid, creature_loot_template.item",
    )
    .bind(creature_entry)
    .fetch_all(pool)
    .await?;

    Ok(rows.into_iter().map(CreatureLootRow::into_query).collect())
}

pub async fn get_pickpocket_loot_items(
    pool: &MySqlPool,
    loot_entry: u32,
) -> Result<Vec<CreatureLootQuery>, DbError> {
    let _query_timer = crate::observability::DbQueryTimer::start("pickpocket_loot_load");
    let rows = sqlx::query_as::<_, CreatureLootRow>(
        "SELECT pickpocketing_loot_template.item, \
                pickpocketing_loot_template.groupid AS group_id, \
                CAST(CASE WHEN pickpocketing_loot_template.mincountOrRef < 0 THEN 0 \
                     ELSE GREATEST(pickpocketing_loot_template.mincountOrRef, 1) END AS UNSIGNED) AS min_count, \
                CAST(CASE WHEN pickpocketing_loot_template.mincountOrRef < 0 THEN GREATEST(pickpocketing_loot_template.maxcount, 1) \
                     ELSE GREATEST(pickpocketing_loot_template.maxcount, pickpocketing_loot_template.mincountOrRef, 1) END AS UNSIGNED) AS max_count, \
                CAST(COALESCE(item_template.displayid, 0) AS UNSIGNED) AS display_id, \
                pickpocketing_loot_template.ChanceOrQuestChance AS chance_or_quest_chance \
         FROM pickpocketing_loot_template \
         LEFT JOIN item_template \
           ON pickpocketing_loot_template.item = item_template.entry \
          AND pickpocketing_loot_template.mincountOrRef > 0 \
         WHERE pickpocketing_loot_template.entry = ? \
           AND pickpocketing_loot_template.condition_id = 0 \
           AND (pickpocketing_loot_template.mincountOrRef < 0 OR item_template.entry IS NOT NULL) \
         ORDER BY pickpocketing_loot_template.groupid, pickpocketing_loot_template.item",
    )
    .bind(loot_entry)
    .fetch_all(pool)
    .await?;

    Ok(rows.into_iter().map(CreatureLootRow::into_query).collect())
}

pub async fn get_reference_loot_items(
    pool: &MySqlPool,
    reference_entry: u32,
) -> Result<Vec<CreatureLootQuery>, DbError> {
    let _query_timer = crate::observability::DbQueryTimer::start("reference_loot_load");
    let rows = sqlx::query_as::<_, CreatureLootRow>(
        "SELECT reference_loot_template.item, \
                reference_loot_template.groupid AS group_id, \
                CAST(CASE WHEN reference_loot_template.mincountOrRef < 0 THEN 0 \
                     ELSE GREATEST(reference_loot_template.mincountOrRef, 1) END AS UNSIGNED) AS min_count, \
                CAST(CASE WHEN reference_loot_template.mincountOrRef < 0 THEN GREATEST(reference_loot_template.maxcount, 1) \
                     ELSE GREATEST(reference_loot_template.maxcount, reference_loot_template.mincountOrRef, 1) END AS UNSIGNED) AS max_count, \
                CAST(COALESCE(item_template.displayid, 0) AS UNSIGNED) AS display_id, \
                reference_loot_template.ChanceOrQuestChance AS chance_or_quest_chance \
         FROM reference_loot_template \
         LEFT JOIN item_template \
           ON reference_loot_template.item = item_template.entry \
          AND reference_loot_template.mincountOrRef > 0 \
         WHERE reference_loot_template.entry = ? \
           AND reference_loot_template.condition_id = 0 \
           AND (reference_loot_template.mincountOrRef < 0 OR item_template.entry IS NOT NULL) \
         ORDER BY reference_loot_template.groupid, reference_loot_template.item",
    )
    .bind(reference_entry)
    .fetch_all(pool)
    .await?;

    Ok(rows.into_iter().map(CreatureLootRow::into_query).collect())
}

pub async fn get_gameobject_loot_items(
    pool: &MySqlPool,
    loot_entry: u32,
) -> Result<Vec<CreatureLootQuery>, DbError> {
    let _query_timer = crate::observability::DbQueryTimer::start("gameobject_loot_load");
    let rows = sqlx::query_as::<_, CreatureLootRow>(
        "SELECT gameobject_loot_template.item, \
                gameobject_loot_template.groupid AS group_id, \
                CAST(CASE WHEN gameobject_loot_template.mincountOrRef < 0 THEN 0 \
                     ELSE GREATEST(gameobject_loot_template.mincountOrRef, 1) END AS UNSIGNED) AS min_count, \
                CAST(CASE WHEN gameobject_loot_template.mincountOrRef < 0 THEN GREATEST(gameobject_loot_template.maxcount, 1) \
                     ELSE GREATEST(gameobject_loot_template.maxcount, gameobject_loot_template.mincountOrRef, 1) END AS UNSIGNED) AS max_count, \
                CAST(COALESCE(item_template.displayid, 0) AS UNSIGNED) AS display_id, \
                gameobject_loot_template.ChanceOrQuestChance AS chance_or_quest_chance \
         FROM gameobject_loot_template \
         LEFT JOIN item_template \
           ON gameobject_loot_template.item = item_template.entry \
          AND gameobject_loot_template.mincountOrRef > 0 \
         WHERE gameobject_loot_template.entry = ? \
           AND gameobject_loot_template.condition_id = 0 \
           AND (gameobject_loot_template.mincountOrRef < 0 OR item_template.entry IS NOT NULL) \
         ORDER BY gameobject_loot_template.groupid, gameobject_loot_template.item",
    )
    .bind(loot_entry)
    .fetch_all(pool)
    .await?;

    Ok(rows.into_iter().map(CreatureLootRow::into_query).collect())
}

pub async fn get_gameobject_template_query(
    pool: &MySqlPool,
    entry: u32,
) -> Result<Option<GameObjectTemplateQuery>, DbError> {
    let _query_timer = crate::observability::DbQueryTimer::start("gameobject_template_load");
    let row = sqlx::query_as::<_, GameObjectTemplateRow>(
        "SELECT entry, \
                type AS object_type, \
                displayId AS display_id, \
                name, \
                IconName AS icon_name, \
                CAST(faction AS UNSIGNED) AS faction, \
                CAST(flags AS UNSIGNED) AS flags, \
                size, \
                CAST(data0 AS SIGNED) AS data0, \
                CAST(data1 AS SIGNED) AS data1, \
                CAST(data2 AS SIGNED) AS data2, \
                CAST(data3 AS SIGNED) AS data3, \
                CAST(data4 AS SIGNED) AS data4, \
                CAST(data5 AS SIGNED) AS data5, \
                CAST(data6 AS SIGNED) AS data6, \
                CAST(data7 AS SIGNED) AS data7, \
                CAST(data8 AS SIGNED) AS data8, \
                CAST(data9 AS SIGNED) AS data9, \
                CAST(data10 AS SIGNED) AS data10, \
                CAST(data11 AS SIGNED) AS data11, \
                CAST(data12 AS SIGNED) AS data12, \
                CAST(data13 AS SIGNED) AS data13, \
                CAST(data14 AS SIGNED) AS data14, \
                CAST(data15 AS SIGNED) AS data15, \
                CAST(data16 AS SIGNED) AS data16, \
                CAST(data17 AS SIGNED) AS data17, \
                CAST(data18 AS SIGNED) AS data18, \
                CAST(data19 AS SIGNED) AS data19, \
                CAST(data20 AS SIGNED) AS data20, \
                CAST(data21 AS SIGNED) AS data21, \
                CAST(data22 AS SIGNED) AS data22, \
                CAST(data23 AS SIGNED) AS data23 \
         FROM gameobject_template \
         WHERE entry = ?",
    )
    .bind(entry)
    .fetch_optional(pool)
    .await?;

    Ok(row.map(GameObjectTemplateRow::into_query))
}

pub async fn get_quest_template_query(
    pool: &MySqlPool,
    quest: u32,
) -> Result<Option<QuestTemplateQuery>, DbError> {
    let _query_timer = crate::observability::DbQueryTimer::start("quest_template_load");
    let row = sqlx::query_as::<_, QuestTemplateRow>(
        "SELECT CAST(entry AS UNSIGNED) AS entry, CAST(Method AS UNSIGNED) AS method, \
                ZoneOrSort AS zone_or_sort, \
                CAST(MinLevel AS UNSIGNED) AS min_level, CAST(MaxLevel AS UNSIGNED) AS max_level, \
                CAST(QuestLevel AS UNSIGNED) AS quest_level, CAST(Type AS UNSIGNED) AS quest_type, \
                CAST(RequiredClasses AS UNSIGNED) AS required_classes, \
                CAST(RequiredRaces AS UNSIGNED) AS required_races, \
                CAST(RequiredSkill AS UNSIGNED) AS required_skill, \
                CAST(RequiredSkillValue AS UNSIGNED) AS required_skill_value, \
                CAST(RequiredCondition AS UNSIGNED) AS required_condition, \
                CAST(RepObjectiveFaction AS UNSIGNED) AS rep_objective_faction, \
                RepObjectiveValue AS rep_objective_value, \
                CAST(RequiredMinRepFaction AS UNSIGNED) AS required_min_rep_faction, \
                RequiredMinRepValue AS required_min_rep_value, \
                CAST(RequiredMaxRepFaction AS UNSIGNED) AS required_max_rep_faction, \
                RequiredMaxRepValue AS required_max_rep_value, \
                CAST(SpecialFlags AS UNSIGNED) AS special_flags, \
                PrevQuestId AS prev_quest_id, NextQuestId AS next_quest_id, \
                ExclusiveGroup AS exclusive_group, \
                CAST(NextQuestInChain AS UNSIGNED) AS next_quest_in_chain, \
                RewOrReqMoney AS rew_or_req_money, RewMoneyMaxLevel AS rew_money_max_level, \
                CAST(RewSpell AS UNSIGNED) AS rew_spell, CAST(RewSpellCast AS UNSIGNED) AS rew_spell_cast, \
                CAST(SrcItemId AS UNSIGNED) AS src_item_id, CAST(SrcItemCount AS UNSIGNED) AS src_item_count, \
                CAST(QuestFlags AS UNSIGNED) AS quest_flags, \
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
                CAST(ReqSourceId1 AS UNSIGNED) AS req_source_id1, CAST(ReqSourceId2 AS UNSIGNED) AS req_source_id2, \
                CAST(ReqSourceId3 AS UNSIGNED) AS req_source_id3, CAST(ReqSourceId4 AS UNSIGNED) AS req_source_id4, \
                CAST(ReqSourceCount1 AS UNSIGNED) AS req_source_count1, CAST(ReqSourceCount2 AS UNSIGNED) AS req_source_count2, \
                CAST(ReqSourceCount3 AS UNSIGNED) AS req_source_count3, CAST(ReqSourceCount4 AS UNSIGNED) AS req_source_count4, \
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
                CAST(RewRepFaction1 AS UNSIGNED) AS rew_rep_faction1, CAST(RewRepFaction2 AS UNSIGNED) AS rew_rep_faction2, \
                CAST(RewRepFaction3 AS UNSIGNED) AS rew_rep_faction3, CAST(RewRepFaction4 AS UNSIGNED) AS rew_rep_faction4, \
                CAST(RewRepFaction5 AS UNSIGNED) AS rew_rep_faction5, RewRepValue1 AS rew_rep_value1, \
                RewRepValue2 AS rew_rep_value2, RewRepValue3 AS rew_rep_value3, RewRepValue4 AS rew_rep_value4, \
                RewRepValue5 AS rew_rep_value5, \
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
    let _query_timer = crate::observability::DbQueryTimer::start("graveyard_load");
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
    let _query_timer = crate::observability::DbQueryTimer::start("spirit_healer_load");
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
    let _query_timer = crate::observability::DbQueryTimer::start("creature_start_quests_load");
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

pub async fn get_creature_complete_quests(
    pool: &MySqlPool,
    creature_entry: u32,
) -> Result<Vec<QuestTemplateQuery>, DbError> {
    let _query_timer = crate::observability::DbQueryTimer::start("creature_complete_quests_load");
    let quest_ids: Vec<u32> = sqlx::query_scalar(
        "SELECT quest FROM creature_involvedrelation WHERE id = ? ORDER BY quest",
    )
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

pub async fn get_gameobject_start_quests(
    pool: &MySqlPool,
    gameobject_entry: u32,
) -> Result<Vec<QuestTemplateQuery>, DbError> {
    let _query_timer = crate::observability::DbQueryTimer::start("gameobject_start_quests_load");
    let quest_ids: Vec<u32> = sqlx::query_scalar(
        "SELECT quest FROM gameobject_questrelation WHERE id = ? ORDER BY quest",
    )
    .bind(gameobject_entry)
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

pub async fn get_gameobject_complete_quests(
    pool: &MySqlPool,
    gameobject_entry: u32,
) -> Result<Vec<QuestTemplateQuery>, DbError> {
    let _query_timer = crate::observability::DbQueryTimer::start("gameobject_complete_quests_load");
    let quest_ids: Vec<u32> = sqlx::query_scalar(
        "SELECT quest FROM gameobject_involvedrelation WHERE id = ? ORDER BY quest",
    )
    .bind(gameobject_entry)
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

pub async fn get_area_trigger_quest(
    pool: &MySqlPool,
    trigger_id: u32,
) -> Result<Option<u32>, DbError> {
    let _query_timer = crate::observability::DbQueryTimer::start("area_trigger_quest_lookup");
    sqlx::query_scalar(
        "SELECT quest FROM areatrigger_involvedrelation WHERE id = ? ORDER BY quest LIMIT 1",
    )
    .bind(trigger_id)
    .fetch_optional(pool)
    .await
    .map_err(Into::into)
}

pub async fn get_gameobject_objective_quest_ids(
    pool: &MySqlPool,
    gameobject_entry: u32,
) -> Result<Vec<u32>, DbError> {
    let _query_timer =
        crate::observability::DbQueryTimer::start("gameobject_objective_quest_ids_load");
    let objective_entry = -(gameobject_entry as i32);
    let quest_ids: Vec<u32> = sqlx::query_scalar(
        "SELECT entry FROM quest_template \
         WHERE ReqCreatureOrGOId1 = ? OR ReqCreatureOrGOId2 = ? \
            OR ReqCreatureOrGOId3 = ? OR ReqCreatureOrGOId4 = ? \
         ORDER BY entry",
    )
    .bind(objective_entry)
    .bind(objective_entry)
    .bind(objective_entry)
    .bind(objective_entry)
    .fetch_all(pool)
    .await?;
    Ok(quest_ids)
}

pub async fn creature_starts_quest(
    pool: &MySqlPool,
    creature_entry: u32,
    quest: u32,
) -> Result<bool, DbError> {
    let _query_timer = crate::observability::DbQueryTimer::start("creature_starts_quest_check");
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
    let _query_timer = crate::observability::DbQueryTimer::start("creature_completes_quest_check");
    let count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM creature_involvedrelation WHERE id = ? AND quest = ?",
    )
    .bind(creature_entry)
    .bind(quest)
    .fetch_one(pool)
    .await?;
    Ok(count > 0)
}

pub async fn get_quest_prev_quests(pool: &MySqlPool, quest: u32) -> Result<Vec<i32>, DbError> {
    let _query_timer = crate::observability::DbQueryTimer::start("quest_prev_quests_load");
    let mut quests: Vec<i32> = Vec::new();

    if let Some(prev_quest_id) =
        sqlx::query_scalar::<_, i32>("SELECT PrevQuestId FROM quest_template WHERE entry = ?")
            .bind(quest)
            .fetch_optional(pool)
            .await?
    {
        if prev_quest_id != 0 {
            quests.push(prev_quest_id);
        }
    }

    let next_positive: Vec<u32> = sqlx::query_scalar(
        "SELECT CAST(entry AS UNSIGNED) AS entry FROM quest_template WHERE NextQuestId = ? ORDER BY entry",
    )
    .bind(quest as i32)
    .fetch_all(pool)
    .await?;
    quests.extend(next_positive.into_iter().map(|entry| entry as i32));

    let next_negative: Vec<u32> = sqlx::query_scalar(
        "SELECT CAST(entry AS UNSIGNED) AS entry FROM quest_template WHERE NextQuestId = ? ORDER BY entry",
    )
    .bind(-(quest as i32))
    .fetch_all(pool)
    .await?;
    quests.extend(next_negative.into_iter().map(|entry| -(entry as i32)));

    Ok(quests)
}

pub async fn get_quest_prev_chain_quests(
    pool: &MySqlPool,
    quest: u32,
) -> Result<Vec<u32>, DbError> {
    let _query_timer = crate::observability::DbQueryTimer::start("quest_prev_chain_load");
    sqlx::query_scalar(
        "SELECT CAST(entry AS UNSIGNED) AS entry \
         FROM quest_template \
         WHERE NextQuestInChain = ? \
         ORDER BY entry",
    )
    .bind(quest)
    .fetch_all(pool)
    .await
    .map_err(Into::into)
}

pub async fn get_exclusive_group_quests(
    pool: &MySqlPool,
    exclusive_group: i32,
) -> Result<Vec<u32>, DbError> {
    let _query_timer = crate::observability::DbQueryTimer::start("quest_exclusive_group_load");
    sqlx::query_scalar(
        "SELECT CAST(entry AS UNSIGNED) AS entry \
         FROM quest_template \
         WHERE ExclusiveGroup = ? \
         ORDER BY entry",
    )
    .bind(exclusive_group)
    .fetch_all(pool)
    .await
    .map_err(Into::into)
}

pub async fn gameobject_starts_quest(
    pool: &MySqlPool,
    gameobject_entry: u32,
    quest: u32,
) -> Result<bool, DbError> {
    let _query_timer = crate::observability::DbQueryTimer::start("gameobject_starts_quest_check");
    let count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM gameobject_questrelation WHERE id = ? AND quest = ?",
    )
    .bind(gameobject_entry)
    .bind(quest)
    .fetch_one(pool)
    .await?;
    Ok(count > 0)
}

pub async fn gameobject_completes_quest(
    pool: &MySqlPool,
    gameobject_entry: u32,
    quest: u32,
) -> Result<bool, DbError> {
    let _query_timer =
        crate::observability::DbQueryTimer::start("gameobject_completes_quest_check");
    let count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM gameobject_involvedrelation WHERE id = ? AND quest = ?",
    )
    .bind(gameobject_entry)
    .bind(quest)
    .fetch_one(pool)
    .await?;
    Ok(count > 0)
}

pub async fn get_item_display_id(pool: &MySqlPool, item: u32) -> Result<Option<u32>, DbError> {
    let _query_timer = crate::observability::DbQueryTimer::start("item_display_load");
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
    let _query_timer = crate::observability::DbQueryTimer::start("vendor_items_load");
    let rows = sqlx::query_as::<_, VendorItemRow>(
        "SELECT vendor_items.item, vendor_items.max_count, vendor_items.incr_time, vendor_items.slot, \
                item_template.displayid AS display_id, item_template.BuyPrice AS buy_price, \
                item_template.MaxDurability AS max_durability, \
                item_template.BuyCount AS buy_count, \
                item_template.ContainerSlots AS container_slots \
         FROM ( \
             SELECT npc_vendor.item, npc_vendor.maxcount AS max_count, npc_vendor.incrtime AS incr_time, npc_vendor.slot \
             FROM npc_vendor \
             WHERE npc_vendor.entry = ? \
               AND npc_vendor.condition_id = 0 \
             UNION ALL \
             SELECT npc_vendor_template.item, npc_vendor_template.maxcount AS max_count, \
                    npc_vendor_template.incrtime AS incr_time, npc_vendor_template.slot \
             FROM creature_template \
             JOIN npc_vendor_template \
               ON npc_vendor_template.entry = creature_template.VendorTemplateId \
             WHERE creature_template.Entry = ? \
               AND npc_vendor_template.condition_id = 0 \
         ) AS vendor_items \
         JOIN item_template ON vendor_items.item = item_template.entry \
         ORDER BY CASE WHEN vendor_items.slot = 0 THEN 255 ELSE vendor_items.slot END, \
                  vendor_items.item \
         LIMIT 128",
    )
    .bind(creature_entry)
    .bind(creature_entry)
    .fetch_all(pool)
    .await?;

    Ok(rows.into_iter().map(VendorItemRow::into_query).collect())
}

pub async fn get_item_random_property_rolls(
    pool: &MySqlPool,
    random_property: u32,
) -> Result<Vec<ItemRandomPropertyRoll>, DbError> {
    let rows = sqlx::query_as::<_, (u32, f32)>(
        "SELECT ench, chance FROM item_enchantment_template \
         WHERE entry = ? AND chance > 0 AND chance <= 100 \
         ORDER BY ench",
    )
    .bind(random_property)
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|(enchantment_id, chance)| ItemRandomPropertyRoll {
            enchantment_id,
            chance,
        })
        .collect())
}

pub async fn get_trainer_spells(
    pool: &MySqlPool,
    creature_entry: u32,
) -> Result<Vec<TrainerSpellQuery>, DbError> {
    let _query_timer = crate::observability::DbQueryTimer::start("trainer_spells_load");
    let rows = sqlx::query_as::<_, TrainerSpellQuery>(
        "SELECT trainer_spells.spell, \
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
                        ELSE trainer_spells.spell \
                    END, trainer_spells.spell) AS UNSIGNED) AS learned_spell, \
                trainer_spells.spellcost AS spell_cost, \
                trainer_spells.reqskill AS req_skill, \
                trainer_spells.reqskillvalue AS req_skill_value, \
                trainer_spells.reqlevel AS req_level, \
                trainer_spells.ReqAbility1 AS req_ability1, \
                trainer_spells.ReqAbility2 AS req_ability2, \
                trainer_spells.ReqAbility3 AS req_ability3 \
         FROM ( \
             SELECT npc_trainer.spell, npc_trainer.spellcost, npc_trainer.reqskill, \
                    npc_trainer.reqskillvalue, npc_trainer.reqlevel, \
                    npc_trainer.ReqAbility1, npc_trainer.ReqAbility2, npc_trainer.ReqAbility3 \
             FROM npc_trainer \
             WHERE npc_trainer.entry = ? \
               AND npc_trainer.condition_id = 0 \
             UNION ALL \
             SELECT npc_trainer_template.spell, npc_trainer_template.spellcost, \
                    npc_trainer_template.reqskill, npc_trainer_template.reqskillvalue, \
                    npc_trainer_template.reqlevel, npc_trainer_template.ReqAbility1, \
                    npc_trainer_template.ReqAbility2, npc_trainer_template.ReqAbility3 \
             FROM creature_template \
             JOIN npc_trainer_template \
               ON npc_trainer_template.entry = creature_template.TrainerTemplateId \
             WHERE creature_template.Entry = ? \
               AND npc_trainer_template.condition_id = 0 \
         ) AS trainer_spells \
         LEFT JOIN spell_template ON spell_template.Id = trainer_spells.spell \
         ORDER BY reqlevel, spell \
         LIMIT 128",
    )
    .bind(creature_entry)
    .bind(creature_entry)
    .fetch_all(pool)
    .await?;

    Ok(rows)
}

pub async fn get_trainer_greeting(
    pool: &MySqlPool,
    creature_entry: u32,
) -> Result<Option<String>, DbError> {
    let _query_timer = crate::observability::DbQueryTimer::start("trainer_greeting_load");
    sqlx::query_scalar("SELECT Text FROM trainer_greeting WHERE Entry = ?")
        .bind(creature_entry)
        .fetch_optional(pool)
        .await
        .map_err(Into::into)
}

pub async fn is_tavern_area_trigger(pool: &MySqlPool, trigger_id: u32) -> Result<bool, DbError> {
    let _query_timer = crate::observability::DbQueryTimer::start("tavern_area_trigger_lookup");
    let found: Option<i32> =
        sqlx::query_scalar("SELECT 1 FROM areatrigger_tavern WHERE id = ? LIMIT 1")
            .bind(trigger_id)
            .fetch_optional(pool)
            .await?;
    Ok(found.is_some())
}

async fn table_has_column(pool: &MySqlPool, table: &str, column: &str) -> Result<bool, DbError> {
    let count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM INFORMATION_SCHEMA.COLUMNS \
         WHERE TABLE_SCHEMA = DATABASE() AND TABLE_NAME = ? AND COLUMN_NAME = ?",
    )
    .bind(table)
    .bind(column)
    .fetch_one(pool)
    .await?;
    Ok(count > 0)
}

pub async fn get_game_event_schedules(
    pool: &MySqlPool,
) -> Result<Vec<GameEventScheduleQuery>, DbError> {
    let _query_timer = crate::observability::DbQueryTimer::start("game_event_schedule_load");
    let schedule_type_expr = if table_has_column(pool, "game_event", "schedule_type").await? {
        "game_event.schedule_type"
    } else {
        "1"
    };
    let sql = format!(
        "SELECT game_event.entry, \
                CAST({schedule_type_expr} AS UNSIGNED) AS schedule_type, \
                CAST(game_event.occurence AS UNSIGNED) AS occurrence, \
                CAST(game_event.length AS UNSIGNED) AS length, \
                CAST(game_event.holiday AS UNSIGNED) AS holiday, \
                CAST(game_event.linkedTo AS UNSIGNED) AS linked_to, \
                game_event.description, \
                CAST(UNIX_TIMESTAMP(game_event_time.start_time) AS SIGNED) AS start_time_unix, \
                CAST(UNIX_TIMESTAMP(game_event_time.end_time) AS SIGNED) AS end_time_unix \
         FROM game_event \
         LEFT JOIN game_event_time ON game_event_time.entry = game_event.entry \
         ORDER BY game_event.entry ASC"
    );
    sqlx::query_as::<_, GameEventScheduleQuery>(&sql)
        .fetch_all(pool)
        .await
        .map_err(Into::into)
}

pub async fn get_conditions(pool: &MySqlPool) -> Result<Vec<ConditionQuery>, DbError> {
    let _query_timer = crate::observability::DbQueryTimer::start("conditions_load");
    sqlx::query_as::<_, ConditionQuery>(
        "SELECT condition_entry, type AS condition_type, \
                CAST(value1 AS UNSIGNED) AS value1, \
                CAST(value2 AS UNSIGNED) AS value2, \
                CAST(value3 AS UNSIGNED) AS value3, \
                CAST(value4 AS UNSIGNED) AS value4, \
                CAST(flags AS UNSIGNED) AS flags \
         FROM conditions \
         ORDER BY condition_entry ASC",
    )
    .fetch_all(pool)
    .await
    .map_err(Into::into)
}

pub async fn get_condition(
    pool: &MySqlPool,
    condition_entry: u32,
) -> Result<Option<ConditionQuery>, DbError> {
    let _query_timer = crate::observability::DbQueryTimer::start("condition_load");
    sqlx::query_as::<_, ConditionQuery>(
        "SELECT condition_entry, type AS condition_type, \
                CAST(value1 AS UNSIGNED) AS value1, \
                CAST(value2 AS UNSIGNED) AS value2, \
                CAST(value3 AS UNSIGNED) AS value3, \
                CAST(value4 AS UNSIGNED) AS value4, \
                CAST(flags AS UNSIGNED) AS flags \
         FROM conditions \
         WHERE condition_entry = ?",
    )
    .bind(condition_entry)
    .fetch_optional(pool)
    .await
    .map_err(Into::into)
}

pub async fn get_unit_conditions(pool: &MySqlPool) -> Result<Vec<UnitConditionQuery>, DbError> {
    let _query_timer = crate::observability::DbQueryTimer::start("unit_condition_load");
    if !world_table_exists(pool, "unit_condition").await? {
        return Ok(Vec::new());
    }

    sqlx::query_as::<_, UnitConditionQuery>(
        "SELECT Id AS id, CAST(Flags AS UNSIGNED) AS flags, \
                CAST(Variable_0 AS UNSIGNED) AS variable_0, \
                CAST(Variable_1 AS UNSIGNED) AS variable_1, \
                CAST(Variable_2 AS UNSIGNED) AS variable_2, \
                CAST(Variable_3 AS UNSIGNED) AS variable_3, \
                CAST(Variable_4 AS UNSIGNED) AS variable_4, \
                CAST(Variable_5 AS UNSIGNED) AS variable_5, \
                CAST(Variable_6 AS UNSIGNED) AS variable_6, \
                CAST(Variable_7 AS UNSIGNED) AS variable_7, \
                CAST(Op_0 AS UNSIGNED) AS op_0, \
                CAST(Op_1 AS UNSIGNED) AS op_1, \
                CAST(Op_2 AS UNSIGNED) AS op_2, \
                CAST(Op_3 AS UNSIGNED) AS op_3, \
                CAST(Op_4 AS UNSIGNED) AS op_4, \
                CAST(Op_5 AS UNSIGNED) AS op_5, \
                CAST(Op_6 AS UNSIGNED) AS op_6, \
                CAST(Op_7 AS UNSIGNED) AS op_7, \
                Value_0 AS value_0, Value_1 AS value_1, \
                Value_2 AS value_2, Value_3 AS value_3, \
                Value_4 AS value_4, Value_5 AS value_5, \
                Value_6 AS value_6, Value_7 AS value_7 \
         FROM unit_condition \
         ORDER BY Id ASC",
    )
    .fetch_all(pool)
    .await
    .map_err(Into::into)
}

pub async fn get_unit_condition(
    pool: &MySqlPool,
    id: i32,
) -> Result<Option<UnitConditionQuery>, DbError> {
    let _query_timer = crate::observability::DbQueryTimer::start("unit_condition_single_load");
    if !world_table_exists(pool, "unit_condition").await? {
        return Ok(None);
    }

    sqlx::query_as::<_, UnitConditionQuery>(
        "SELECT Id AS id, CAST(Flags AS UNSIGNED) AS flags, \
                CAST(Variable_0 AS UNSIGNED) AS variable_0, \
                CAST(Variable_1 AS UNSIGNED) AS variable_1, \
                CAST(Variable_2 AS UNSIGNED) AS variable_2, \
                CAST(Variable_3 AS UNSIGNED) AS variable_3, \
                CAST(Variable_4 AS UNSIGNED) AS variable_4, \
                CAST(Variable_5 AS UNSIGNED) AS variable_5, \
                CAST(Variable_6 AS UNSIGNED) AS variable_6, \
                CAST(Variable_7 AS UNSIGNED) AS variable_7, \
                CAST(Op_0 AS UNSIGNED) AS op_0, \
                CAST(Op_1 AS UNSIGNED) AS op_1, \
                CAST(Op_2 AS UNSIGNED) AS op_2, \
                CAST(Op_3 AS UNSIGNED) AS op_3, \
                CAST(Op_4 AS UNSIGNED) AS op_4, \
                CAST(Op_5 AS UNSIGNED) AS op_5, \
                CAST(Op_6 AS UNSIGNED) AS op_6, \
                CAST(Op_7 AS UNSIGNED) AS op_7, \
                Value_0 AS value_0, Value_1 AS value_1, \
                Value_2 AS value_2, Value_3 AS value_3, \
                Value_4 AS value_4, Value_5 AS value_5, \
                Value_6 AS value_6, Value_7 AS value_7 \
         FROM unit_condition \
         WHERE Id = ?",
    )
    .bind(id)
    .fetch_optional(pool)
    .await
    .map_err(Into::into)
}

pub async fn get_combat_conditions(pool: &MySqlPool) -> Result<Vec<CombatConditionQuery>, DbError> {
    let _query_timer = crate::observability::DbQueryTimer::start("combat_condition_load");
    if !world_table_exists(pool, "combat_condition").await? {
        return Ok(Vec::new());
    }

    sqlx::query_as::<_, CombatConditionQuery>(
        "SELECT Id AS id, WorldStateExpressionID AS world_state_expression_id, \
                SelfConditionID AS self_condition_id, \
                TargetConditionID AS target_condition_id, \
                FriendConditionLogic AS friend_condition_logic, \
                EnemyConditionLogic AS enemy_condition_logic, \
                FriendConditionID_0 AS friend_condition_id_0, \
                FriendConditionID_1 AS friend_condition_id_1, \
                FriendConditionOp_0 AS friend_condition_op_0, \
                FriendConditionOp_1 AS friend_condition_op_1, \
                FriendConditionCount_0 AS friend_condition_count_0, \
                FriendConditionCount_1 AS friend_condition_count_1, \
                EnemyConditionID_0 AS enemy_condition_id_0, \
                EnemyConditionID_1 AS enemy_condition_id_1, \
                EnemyConditionOp_0 AS enemy_condition_op_0, \
                EnemyConditionOp_1 AS enemy_condition_op_1, \
                EnemyConditionCount_0 AS enemy_condition_count_0, \
                EnemyConditionCount_1 AS enemy_condition_count_1 \
         FROM combat_condition \
         ORDER BY Id ASC",
    )
    .fetch_all(pool)
    .await
    .map_err(Into::into)
}

pub async fn get_combat_condition(
    pool: &MySqlPool,
    id: i32,
) -> Result<Option<CombatConditionQuery>, DbError> {
    let _query_timer = crate::observability::DbQueryTimer::start("combat_condition_single_load");
    if !world_table_exists(pool, "combat_condition").await? {
        return Ok(None);
    }

    sqlx::query_as::<_, CombatConditionQuery>(
        "SELECT Id AS id, WorldStateExpressionID AS world_state_expression_id, \
                SelfConditionID AS self_condition_id, \
                TargetConditionID AS target_condition_id, \
                FriendConditionLogic AS friend_condition_logic, \
                EnemyConditionLogic AS enemy_condition_logic, \
                FriendConditionID_0 AS friend_condition_id_0, \
                FriendConditionID_1 AS friend_condition_id_1, \
                FriendConditionOp_0 AS friend_condition_op_0, \
                FriendConditionOp_1 AS friend_condition_op_1, \
                FriendConditionCount_0 AS friend_condition_count_0, \
                FriendConditionCount_1 AS friend_condition_count_1, \
                EnemyConditionID_0 AS enemy_condition_id_0, \
                EnemyConditionID_1 AS enemy_condition_id_1, \
                EnemyConditionOp_0 AS enemy_condition_op_0, \
                EnemyConditionOp_1 AS enemy_condition_op_1, \
                EnemyConditionCount_0 AS enemy_condition_count_0, \
                EnemyConditionCount_1 AS enemy_condition_count_1 \
         FROM combat_condition \
         WHERE Id = ?",
    )
    .bind(id)
    .fetch_optional(pool)
    .await
    .map_err(Into::into)
}

pub async fn get_dbscripts_on_creature_movement(
    pool: &MySqlPool,
) -> Result<Vec<DbScriptCommandQuery>, DbError> {
    let _query_timer =
        crate::observability::DbQueryTimer::start("dbscripts_creature_movement_load");
    sqlx::query_as::<_, DbScriptCommandQuery>(
        "SELECT id, delay, priority, command, datalong, datalong2, datalong3, \
                data_flags, dataint, dataint2, dataint3, dataint4, condition_id \
         FROM dbscripts_on_creature_movement \
         ORDER BY id ASC, delay ASC, priority ASC",
    )
    .fetch_all(pool)
    .await
    .map_err(Into::into)
}

pub async fn get_script_texts(pool: &MySqlPool) -> Result<Vec<ScriptTextQuery>, DbError> {
    let _query_timer = crate::observability::DbQueryTimer::start("script_texts_load");
    sqlx::query_as::<_, ScriptTextQuery>(
        "SELECT entry, content_default, sound, type AS chat_type, language, emote, \
                broadcast_text_id \
         FROM script_texts \
         ORDER BY entry ASC",
    )
    .fetch_all(pool)
    .await
    .map_err(Into::into)
}

pub async fn get_broadcast_texts(pool: &MySqlPool) -> Result<Vec<BroadcastTextQuery>, DbError> {
    let _query_timer = crate::observability::DbQueryTimer::start("broadcast_texts_load");
    if !world_table_exists(pool, "broadcast_text").await? {
        return Ok(Vec::new());
    }

    sqlx::query_as::<_, BroadcastTextQuery>(
        "SELECT Id AS id, Text AS text, Text1 AS text1, \
                CAST(ChatTypeID AS UNSIGNED) AS chat_type, \
                CAST(LanguageID AS UNSIGNED) AS language, \
                CAST(SoundEntriesID1 AS UNSIGNED) AS sound, \
                CAST(EmoteID1 AS UNSIGNED) AS emote \
         FROM broadcast_text \
         ORDER BY Id ASC",
    )
    .fetch_all(pool)
    .await
    .map_err(Into::into)
}

pub async fn get_nearby_creature_spawns(
    pool: &MySqlPool,
    map: u32,
    position_x: f32,
    position_y: f32,
    radius: f32,
    limit: u32,
) -> Result<Vec<CreatureSpawnQuery>, DbError> {
    let _query_timer = crate::observability::DbQueryTimer::start("creature_nearby_load");
    let rows = sqlx::query_as::<_, CreatureSpawnRow>(
        "SELECT creature.guid, \
                CAST(COALESCE(NULLIF(creature.id, 0), creature_spawn_entry_choice.entry) AS UNSIGNED) AS entry, \
                creature.map, \
                CAST(game_event_creature.event AS SIGNED) AS game_event, \
                CAST(pool_creature.pool_entry AS UNSIGNED) AS guid_pool_id, \
                CAST(pool_creature_template.pool_entry AS UNSIGNED) AS entry_pool_id, \
                CAST(COALESCE(guid_pool_template.max_limit, entry_pool_template.max_limit) AS UNSIGNED) AS pool_max_limit, \
                CAST(COALESCE(pool_creature.chance, pool_creature_template.chance, 0) AS DOUBLE) AS pool_chance, \
                CAST(COALESCE(creature_addon.emote, creature_template_addon.emote, 0) AS UNSIGNED) AS addon_emote, \
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
                creature_template.DisplayIdProbability1 AS template_display_id_probability1, \
                creature_template.DisplayIdProbability2 AS template_display_id_probability2, \
                creature_template.DisplayIdProbability3 AS template_display_id_probability3, \
                creature_template.DisplayIdProbability4 AS template_display_id_probability4, \
                CAST(COALESCE(cmi1.gender, 2) AS UNSIGNED) AS template_model_gender1, \
                CAST(COALESCE(cmi2.gender, 2) AS UNSIGNED) AS template_model_gender2, \
                CAST(COALESCE(cmi3.gender, 2) AS UNSIGNED) AS template_model_gender3, \
                CAST(COALESCE(cmi4.gender, 2) AS UNSIGNED) AS template_model_gender4, \
                CAST(COALESCE(cmi1.modelid_other_gender, 0) AS UNSIGNED) AS template_model_other_gender1, \
                CAST(COALESCE(cmi2.modelid_other_gender, 0) AS UNSIGNED) AS template_model_other_gender2, \
                CAST(COALESCE(cmi3.modelid_other_gender, 0) AS UNSIGNED) AS template_model_other_gender3, \
                CAST(COALESCE(cmi4.modelid_other_gender, 0) AS UNSIGNED) AS template_model_other_gender4, \
                CAST(COALESCE(cmi1_other.gender, 2) AS UNSIGNED) AS template_model_other_gender_gender1, \
                CAST(COALESCE(cmi2_other.gender, 2) AS UNSIGNED) AS template_model_other_gender_gender2, \
                CAST(COALESCE(cmi3_other.gender, 2) AS UNSIGNED) AS template_model_other_gender_gender3, \
                CAST(COALESCE(cmi4_other.gender, 2) AS UNSIGNED) AS template_model_other_gender_gender4, \
                CAST(COALESCE(creature_model_info.bounding_radius, 0) AS DOUBLE) AS template_model_bounding_radius, \
                CAST(COALESCE(creature_model_info.combat_reach, 0) AS DOUBLE) AS template_model_combat_reach, \
                creature_template.Faction AS template_faction, creature_template.Scale AS template_scale, \
                creature_template.SpeedWalk AS template_speed_walk, creature_template.SpeedRun AS template_speed_run, \
                creature_template.Detection AS template_detection_range, \
                creature_template.CallForHelp AS template_call_for_help, \
                creature_template.Pursuit AS template_pursuit, \
                creature_template.Leash AS template_leash, \
                creature_template.Family AS template_family, \
                creature_template.CreatureType AS template_creature_type, \
                creature_template.CreatureTypeFlags AS template_creature_type_flags, \
                creature_template.InhabitType AS template_inhabit_type, \
                creature_template.NpcFlags AS template_npc_flags, \
                creature_template.UnitFlags AS template_unit_flags, \
                creature_template.DynamicFlags AS template_dynamic_flags, \
                creature_template.StaticFlags2 AS template_static_flags2, \
                creature_template.ExtraFlags AS template_extra_flags, \
                creature_template.UnitClass AS template_unit_class, \
                CAST(cls.Strength AS UNSIGNED) AS template_base_strength, \
                creature_template.Rank AS template_rank, \
                creature_template.HealthMultiplier AS template_health_multiplier, \
                creature_template.PowerMultiplier AS template_power_multiplier, \
                creature_template.DamageMultiplier AS template_damage_multiplier, \
                creature_template.DamageVariance AS template_damage_variance, \
                creature_template.ArmorMultiplier AS template_armor_multiplier, \
                creature_template.StrengthMultiplier AS template_strength_multiplier, \
                creature_template.MinLevelHealth AS template_min_level_health, \
                creature_template.MaxLevelHealth AS template_max_level_health, \
                creature_template.MinLevelMana AS template_min_level_mana, \
                creature_template.MaxLevelMana AS template_max_level_mana, \
                creature_template.MinMeleeDmg AS template_min_melee_dmg, \
                creature_template.MaxMeleeDmg AS template_max_melee_dmg, \
                creature_template.MinRangedDmg AS template_min_ranged_dmg, \
                creature_template.MaxRangedDmg AS template_max_ranged_dmg, \
                creature_template.Armor AS template_armor, \
                creature_template.ResistanceHoly AS template_resistance_holy, \
                creature_template.ResistanceFire AS template_resistance_fire, \
                creature_template.ResistanceNature AS template_resistance_nature, \
                creature_template.ResistanceFrost AS template_resistance_frost, \
                creature_template.ResistanceShadow AS template_resistance_shadow, \
                creature_template.ResistanceArcane AS template_resistance_arcane, \
                creature_template.MeleeAttackPower AS template_melee_attack_power, \
                creature_template.RangedAttackPower AS template_ranged_attack_power, \
                creature_template.MinLootGold AS template_min_loot_gold, \
                creature_template.MaxLootGold AS template_max_loot_gold, \
                CAST(creature_template.PickpocketLootId AS UNSIGNED) AS template_pickpocket_loot_id, \
                creature_template.MeleeBaseAttackTime AS template_melee_base_attack_time, \
                creature_template.RangedBaseAttackTime AS template_ranged_base_attack_time, \
                creature_template.DamageSchool AS template_damage_school, \
                creature_template.TrainerType AS template_trainer_type, \
                creature_template.TrainerClass AS template_trainer_class, \
                creature_template.PetSpellDataId AS template_pet_spell_data_id, \
                CAST(creature_template.SpellList AS UNSIGNED) AS template_spell_list, \
                creature_template.Civilian AS template_civilian, \
                creature_template.CorpseDecay AS template_corpse_decay, \
                creature_template.MovementType AS template_movement_type, \
                creature_template.EquipmentTemplateId AS template_equipment_template_id, \
                CAST(COALESCE(equip_1.displayid, 0) AS UNSIGNED) AS template_equip_display_id1, \
                CAST(COALESCE(equip_2.displayid, 0) AS UNSIGNED) AS template_equip_display_id2, \
                CAST(COALESCE(equip_3.displayid, 0) AS UNSIGNED) AS template_equip_display_id3, \
                CAST(COALESCE(equip_1.class, 0) AS UNSIGNED) AS template_equip_class1, \
                CAST(COALESCE(equip_2.class, 0) AS UNSIGNED) AS template_equip_class2, \
                CAST(COALESCE(equip_3.class, 0) AS UNSIGNED) AS template_equip_class3, \
                CAST(COALESCE(equip_1.subclass, 0) AS UNSIGNED) AS template_equip_subclass1, \
                CAST(COALESCE(equip_2.subclass, 0) AS UNSIGNED) AS template_equip_subclass2, \
                CAST(COALESCE(equip_3.subclass, 0) AS UNSIGNED) AS template_equip_subclass3, \
                CAST(COALESCE(equip_1.Material, 0) AS SIGNED) AS template_equip_material1, \
                CAST(COALESCE(equip_2.Material, 0) AS SIGNED) AS template_equip_material2, \
                CAST(COALESCE(equip_3.Material, 0) AS SIGNED) AS template_equip_material3, \
                CAST(COALESCE(equip_1.InventoryType, 0) AS UNSIGNED) AS template_equip_inventory_type1, \
                CAST(COALESCE(equip_2.InventoryType, 0) AS UNSIGNED) AS template_equip_inventory_type2, \
                CAST(COALESCE(equip_3.InventoryType, 0) AS UNSIGNED) AS template_equip_inventory_type3, \
                CAST(COALESCE(equip_1.sheath, 0) AS UNSIGNED) AS template_equip_sheath1, \
                CAST(COALESCE(equip_2.sheath, 0) AS UNSIGNED) AS template_equip_sheath2, \
                CAST(COALESCE(equip_3.sheath, 0) AS UNSIGNED) AS template_equip_sheath3, \
                creature_template.ExperienceMultiplier AS template_experience_multiplier \
         FROM creature \
         LEFT JOIN ( \
             SELECT guid, CAST(SUBSTRING_INDEX(GROUP_CONCAT(entry ORDER BY RAND()), ',', 1) AS UNSIGNED) AS entry \
             FROM creature_spawn_entry \
             GROUP BY guid \
         ) AS creature_spawn_entry_choice ON creature.guid = creature_spawn_entry_choice.guid \
         JOIN creature_template ON COALESCE(NULLIF(creature.id, 0), creature_spawn_entry_choice.entry) = creature_template.Entry \
         LEFT JOIN creature_template_classlevelstats AS cls \
           ON cls.Class = creature_template.UnitClass \
          AND cls.Level = CASE \
                WHEN creature_template.MaxLevel > creature_template.MinLevel THEN creature_template.MaxLevel \
                ELSE creature_template.MinLevel \
              END \
         LEFT JOIN game_event_creature ON creature.guid = game_event_creature.guid \
         LEFT JOIN pool_creature ON creature.guid = pool_creature.guid \
         LEFT JOIN pool_creature_template ON creature.id = pool_creature_template.id \
         LEFT JOIN pool_template AS guid_pool_template ON pool_creature.pool_entry = guid_pool_template.entry \
         LEFT JOIN pool_template AS entry_pool_template ON pool_creature_template.pool_entry = entry_pool_template.entry \
         LEFT JOIN creature_addon ON creature.guid = creature_addon.guid \
         LEFT JOIN creature_template_addon ON creature_template.Entry = creature_template_addon.entry \
         LEFT JOIN creature_model_info \
           ON creature_model_info.modelid = COALESCE(NULLIF(creature_template.DisplayId1, 0), NULLIF(creature_template.DisplayId2, 0), NULLIF(creature_template.DisplayId3, 0), NULLIF(creature_template.DisplayId4, 0), 0) \
         LEFT JOIN creature_model_info AS cmi1 ON cmi1.modelid = creature_template.DisplayId1 \
         LEFT JOIN creature_model_info AS cmi2 ON cmi2.modelid = creature_template.DisplayId2 \
         LEFT JOIN creature_model_info AS cmi3 ON cmi3.modelid = creature_template.DisplayId3 \
         LEFT JOIN creature_model_info AS cmi4 ON cmi4.modelid = creature_template.DisplayId4 \
         LEFT JOIN creature_model_info AS cmi1_other ON cmi1_other.modelid = cmi1.modelid_other_gender \
         LEFT JOIN creature_model_info AS cmi2_other ON cmi2_other.modelid = cmi2.modelid_other_gender \
         LEFT JOIN creature_model_info AS cmi3_other ON cmi3_other.modelid = cmi3.modelid_other_gender \
         LEFT JOIN creature_model_info AS cmi4_other ON cmi4_other.modelid = cmi4.modelid_other_gender \
         LEFT JOIN creature_equip_template ON creature_equip_template.entry = creature_template.EquipmentTemplateId \
         LEFT JOIN item_template AS equip_1 ON equip_1.entry = creature_equip_template.equipentry1 \
         LEFT JOIN item_template AS equip_2 ON equip_2.entry = creature_equip_template.equipentry2 \
         LEFT JOIN item_template AS equip_3 ON equip_3.entry = creature_equip_template.equipentry3 \
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

    let mut spawns = filter_creature_pool_spawns(
        rows.into_iter()
            .map(CreatureSpawnRow::into_query)
            .collect::<Vec<_>>(),
    );
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

pub async fn get_nearby_gameobject_spawns(
    pool: &MySqlPool,
    map: u32,
    position_x: f32,
    position_y: f32,
    radius: f32,
    limit: u32,
) -> Result<Vec<GameObjectSpawnQuery>, DbError> {
    let _query_timer = crate::observability::DbQueryTimer::start("gameobject_nearby_load");
    let rows = sqlx::query_as::<_, GameObjectSpawnRow>(
        "SELECT gameobject.guid, \
                CAST(COALESCE(NULLIF(gameobject.id, 0), gameobject_spawn_entry_choice.entry) AS UNSIGNED) AS entry, \
                gameobject.map, \
                CAST(game_event_gameobject.event AS SIGNED) AS game_event, \
                CAST(pool_gameobject.pool_entry AS UNSIGNED) AS guid_pool_id, \
                CAST(pool_gameobject_template.pool_entry AS UNSIGNED) AS entry_pool_id, \
                CAST(COALESCE(guid_pool_template.max_limit, entry_pool_template.max_limit) AS UNSIGNED) AS pool_max_limit, \
                CAST(COALESCE(pool_gameobject.chance, pool_gameobject_template.chance, 0) AS DOUBLE) AS pool_chance, \
                CAST(gameobject.position_x AS DOUBLE) AS position_x, \
                CAST(gameobject.position_y AS DOUBLE) AS position_y, \
                CAST(gameobject.position_z AS DOUBLE) AS position_z, \
                CAST(gameobject.orientation AS DOUBLE) AS orientation, \
                CAST(gameobject.rotation0 AS DOUBLE) AS rotation0, \
                CAST(gameobject.rotation1 AS DOUBLE) AS rotation1, \
                CAST(gameobject.rotation2 AS DOUBLE) AS rotation2, \
                CAST(gameobject.rotation3 AS DOUBLE) AS rotation3, \
                gameobject.spawntimesecsmin AS spawn_time_secs_min, \
                gameobject.spawntimesecsmax AS spawn_time_secs_max, \
                CAST(COALESCE(gameobject_addon.state, -1) AS SIGNED) AS state, \
                CAST(COALESCE(gameobject_addon.animprogress, 100) AS UNSIGNED) AS anim_progress, \
                gameobject_template.entry AS template_entry, \
                gameobject_template.type AS template_object_type, \
                gameobject_template.displayId AS template_display_id, \
                gameobject_template.name AS template_name, \
                gameobject_template.IconName AS template_icon_name, \
                CAST(gameobject_template.faction AS UNSIGNED) AS template_faction, \
                CAST(gameobject_template.flags AS UNSIGNED) AS template_flags, \
                gameobject_template.size AS template_size, \
                CAST(gameobject_template.data0 AS SIGNED) AS template_data0, \
                CAST(gameobject_template.data1 AS SIGNED) AS template_data1, \
                CAST(gameobject_template.data2 AS SIGNED) AS template_data2, \
                CAST(gameobject_template.data3 AS SIGNED) AS template_data3, \
                CAST(gameobject_template.data4 AS SIGNED) AS template_data4, \
                CAST(gameobject_template.data5 AS SIGNED) AS template_data5, \
                CAST(gameobject_template.data6 AS SIGNED) AS template_data6, \
                CAST(gameobject_template.data7 AS SIGNED) AS template_data7, \
                CAST(gameobject_template.data8 AS SIGNED) AS template_data8, \
                CAST(gameobject_template.data9 AS SIGNED) AS template_data9, \
                CAST(gameobject_template.data10 AS SIGNED) AS template_data10, \
                CAST(gameobject_template.data11 AS SIGNED) AS template_data11, \
                CAST(gameobject_template.data12 AS SIGNED) AS template_data12, \
                CAST(gameobject_template.data13 AS SIGNED) AS template_data13, \
                CAST(gameobject_template.data14 AS SIGNED) AS template_data14, \
                CAST(gameobject_template.data15 AS SIGNED) AS template_data15, \
                CAST(gameobject_template.data16 AS SIGNED) AS template_data16, \
                CAST(gameobject_template.data17 AS SIGNED) AS template_data17, \
                CAST(gameobject_template.data18 AS SIGNED) AS template_data18, \
                CAST(gameobject_template.data19 AS SIGNED) AS template_data19, \
                CAST(gameobject_template.data20 AS SIGNED) AS template_data20, \
                CAST(gameobject_template.data21 AS SIGNED) AS template_data21, \
                CAST(gameobject_template.data22 AS SIGNED) AS template_data22, \
                CAST(gameobject_template.data23 AS SIGNED) AS template_data23 \
         FROM gameobject \
         LEFT JOIN ( \
             SELECT guid, CAST(SUBSTRING_INDEX(GROUP_CONCAT(entry ORDER BY RAND()), ',', 1) AS UNSIGNED) AS entry \
             FROM gameobject_spawn_entry \
             GROUP BY guid \
         ) AS gameobject_spawn_entry_choice ON gameobject.guid = gameobject_spawn_entry_choice.guid \
         JOIN gameobject_template ON COALESCE(NULLIF(gameobject.id, 0), gameobject_spawn_entry_choice.entry) = gameobject_template.entry \
         LEFT JOIN gameobject_addon ON gameobject.guid = gameobject_addon.guid \
         LEFT JOIN game_event_gameobject ON gameobject.guid = game_event_gameobject.guid \
         LEFT JOIN pool_gameobject ON gameobject.guid = pool_gameobject.guid \
         LEFT JOIN pool_gameobject_template ON gameobject.id = pool_gameobject_template.id \
         LEFT JOIN pool_template AS guid_pool_template ON pool_gameobject.pool_entry = guid_pool_template.entry \
         LEFT JOIN pool_template AS entry_pool_template ON pool_gameobject_template.pool_entry = entry_pool_template.entry \
         WHERE gameobject.map = ? \
           AND gameobject.position_x BETWEEN ? AND ? \
           AND gameobject.position_y BETWEEN ? AND ? \
           AND (((gameobject.position_x - ?) * (gameobject.position_x - ?)) + \
                ((gameobject.position_y - ?) * (gameobject.position_y - ?))) <= ? \
         ORDER BY ((gameobject.position_x - ?) * (gameobject.position_x - ?)) + \
                  ((gameobject.position_y - ?) * (gameobject.position_y - ?)) ASC, \
                  gameobject.guid ASC \
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

    Ok(filter_gameobject_pool_spawns(
        rows.into_iter()
            .map(GameObjectSpawnRow::into_query)
            .collect(),
    ))
}

pub async fn get_gameobject_spawns_in_rect(
    pool: &MySqlPool,
    map: u32,
    min_x: f32,
    max_x: f32,
    min_y: f32,
    max_y: f32,
) -> Result<Vec<GameObjectSpawnQuery>, DbError> {
    let _query_timer = crate::observability::DbQueryTimer::start("gameobject_grid_load");
    let mut builder: QueryBuilder<MySql> = QueryBuilder::new(GAMEOBJECT_SPAWN_SELECT);
    builder.push(" WHERE gameobject.map = ");
    builder.push_bind(map);
    builder.push(" AND gameobject.position_x BETWEEN ");
    builder.push_bind(min_x);
    builder.push(" AND ");
    builder.push_bind(max_x);
    builder.push(" AND gameobject.position_y BETWEEN ");
    builder.push_bind(min_y);
    builder.push(" AND ");
    builder.push_bind(max_y);
    builder.push(" ORDER BY gameobject.guid ASC");
    let rows = builder
        .build_query_as::<GameObjectSpawnRow>()
        .fetch_all(pool)
        .await?;

    Ok(filter_gameobject_pool_spawns(
        rows.into_iter()
            .map(GameObjectSpawnRow::into_query)
            .collect(),
    ))
}

pub async fn get_all_static_gameobject_spawns(
    pool: &MySqlPool,
) -> Result<Vec<GameObjectSpawnQuery>, DbError> {
    let _query_timer =
        crate::observability::DbQueryTimer::start("static_gameobject_spawn_cache_load");
    let mut builder: QueryBuilder<MySql> = QueryBuilder::new(GAMEOBJECT_SPAWN_SELECT);
    builder.push(" ORDER BY gameobject.map ASC, gameobject.guid ASC");
    let rows = builder
        .build_query_as::<GameObjectSpawnRow>()
        .fetch_all(pool)
        .await?;

    Ok(filter_gameobject_pool_spawns(
        rows.into_iter()
            .map(GameObjectSpawnRow::into_query)
            .collect(),
    ))
}

pub async fn get_creature_spawns_in_rect(
    pool: &MySqlPool,
    map: u32,
    min_x: f32,
    max_x: f32,
    min_y: f32,
    max_y: f32,
) -> Result<Vec<CreatureSpawnQuery>, DbError> {
    let _query_timer = crate::observability::DbQueryTimer::start("creature_grid_load");
    let mut builder: QueryBuilder<MySql> = QueryBuilder::new(CREATURE_SPAWN_SELECT);
    builder.push(" WHERE creature.map = ");
    builder.push_bind(map);
    builder.push(" AND creature.position_x BETWEEN ");
    builder.push_bind(min_x);
    builder.push(" AND ");
    builder.push_bind(max_x);
    builder.push(" AND creature.position_y BETWEEN ");
    builder.push_bind(min_y);
    builder.push(" AND ");
    builder.push_bind(max_y);
    builder.push(" ORDER BY creature.guid ASC");
    let rows = builder
        .build_query_as::<CreatureSpawnRow>()
        .fetch_all(pool)
        .await?;

    creature_spawns_from_rows_with_waypoints(pool, rows).await
}

pub async fn get_all_static_creature_spawns(
    pool: &MySqlPool,
) -> Result<Vec<CreatureSpawnQuery>, DbError> {
    let _query_timer =
        crate::observability::DbQueryTimer::start("static_creature_spawn_cache_load");
    let mut builder: QueryBuilder<MySql> = QueryBuilder::new(CREATURE_SPAWN_SELECT);
    builder.push(" ORDER BY creature.map ASC, creature.guid ASC");
    let rows = builder
        .build_query_as::<CreatureSpawnRow>()
        .fetch_all(pool)
        .await?;

    let mut spawns = filter_creature_pool_spawns(
        rows.into_iter()
            .map(CreatureSpawnRow::into_query)
            .collect::<Vec<_>>(),
    );
    let waypoint_paths = load_bulk_creature_waypoint_paths(pool, &spawns).await?;
    attach_bulk_creature_waypoint_paths(&mut spawns, &waypoint_paths);
    Ok(spawns)
}

async fn creature_spawns_from_rows_with_waypoints(
    pool: &MySqlPool,
    rows: Vec<CreatureSpawnRow>,
) -> Result<Vec<CreatureSpawnQuery>, DbError> {
    let mut spawns = filter_creature_pool_spawns(
        rows.into_iter()
            .map(CreatureSpawnRow::into_query)
            .collect::<Vec<_>>(),
    );
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
    let _query_timer = crate::observability::DbQueryTimer::start("creature_respawn_load");

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
    let _query_timer = crate::observability::DbQueryTimer::start("creature_respawn_save");
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
    let _query_timer = crate::observability::DbQueryTimer::start("creature_waypoint_path_load");
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

fn spawn_pool_id(guid_pool_id: Option<u16>, entry_pool_id: Option<u16>) -> Option<u16> {
    guid_pool_id.or(entry_pool_id)
}

fn filter_creature_pool_spawns(spawns: Vec<CreatureSpawnQuery>) -> Vec<CreatureSpawnQuery> {
    let mut direct = Vec::new();
    let mut pooled: HashMap<u16, (u32, Vec<CreatureSpawnQuery>)> = HashMap::new();

    for spawn in spawns {
        let Some(pool_id) = spawn_pool_id(spawn.guid_pool_id, spawn.entry_pool_id) else {
            direct.push(spawn);
            continue;
        };
        let limit = spawn.pool_max_limit.unwrap_or(0);
        pooled
            .entry(pool_id)
            .or_insert_with(|| (limit, Vec::new()))
            .1
            .push(spawn);
    }

    let mut rng = rand::thread_rng();
    for (_pool_id, (limit, members)) in pooled {
        direct.extend(roll_creature_pool_members(members, limit, &mut rng));
    }

    direct.sort_by_key(|spawn| (spawn.map, spawn.guid));
    direct
}

fn filter_gameobject_pool_spawns(spawns: Vec<GameObjectSpawnQuery>) -> Vec<GameObjectSpawnQuery> {
    let mut direct = Vec::new();
    let mut pooled: HashMap<u16, (u32, Vec<GameObjectSpawnQuery>)> = HashMap::new();

    for spawn in spawns {
        let Some(pool_id) = spawn_pool_id(spawn.guid_pool_id, spawn.entry_pool_id) else {
            direct.push(spawn);
            continue;
        };
        let limit = spawn.pool_max_limit.unwrap_or(0);
        pooled
            .entry(pool_id)
            .or_insert_with(|| (limit, Vec::new()))
            .1
            .push(spawn);
    }

    let mut rng = rand::thread_rng();
    for (_pool_id, (limit, members)) in pooled {
        direct.extend(roll_gameobject_pool_members(members, limit, &mut rng));
    }

    direct.sort_by_key(|spawn| (spawn.map, spawn.guid));
    direct
}

fn roll_creature_pool_members<R: Rng + ?Sized>(
    mut members: Vec<CreatureSpawnQuery>,
    limit: u32,
    rng: &mut R,
) -> Vec<CreatureSpawnQuery> {
    let mut selected = Vec::new();
    for _ in 0..limit {
        let Some(index) = roll_pool_member_index(&members, |spawn| spawn.pool_chance, rng) else {
            break;
        };
        selected.push(members.swap_remove(index));
    }
    selected
}

fn roll_gameobject_pool_members<R: Rng + ?Sized>(
    mut members: Vec<GameObjectSpawnQuery>,
    limit: u32,
    rng: &mut R,
) -> Vec<GameObjectSpawnQuery> {
    let mut selected = Vec::new();
    for _ in 0..limit {
        let Some(index) = roll_pool_member_index(&members, |spawn| spawn.pool_chance, rng) else {
            break;
        };
        selected.push(members.swap_remove(index));
    }
    selected
}

fn roll_pool_member_index<T, R: Rng + ?Sized>(
    members: &[T],
    chance: impl Fn(&T) -> f32,
    rng: &mut R,
) -> Option<usize> {
    let mut explicit = members
        .iter()
        .enumerate()
        .filter(|(_, member)| chance(member) > 0.0)
        .map(|(index, _)| index)
        .collect::<Vec<_>>();
    explicit.shuffle(rng);

    let mut first_explicit = None;
    if !explicit.is_empty() {
        let roll = rng.gen_range(0.0..100.0);
        for index in explicit {
            first_explicit.get_or_insert(index);
            if roll < chance(&members[index]) {
                return Some(index);
            }
        }
    }

    let mut equal = members
        .iter()
        .enumerate()
        .filter(|(_, member)| chance(member) <= 0.0)
        .map(|(index, _)| index)
        .collect::<Vec<_>>();
    equal.shuffle(rng);
    equal.into_iter().next().or(first_explicit)
}

#[derive(Debug, Default)]
struct BulkCreatureWaypointPaths {
    formation_paths: HashMap<u32, Vec<CreatureWaypointQuery>>,
    guid_paths: HashMap<u32, Vec<CreatureWaypointQuery>>,
    template_paths: HashMap<(u32, u32), Vec<CreatureWaypointQuery>>,
}

async fn load_bulk_creature_waypoint_paths(
    pool: &MySqlPool,
    spawns: &[CreatureSpawnQuery],
) -> Result<BulkCreatureWaypointPaths, DbError> {
    let path_spawns = spawns
        .iter()
        .filter(|spawn| {
            let movement_type = creature_effective_movement_type(spawn);
            movement_type == 2 || movement_type == 4
        })
        .collect::<Vec<_>>();
    if path_spawns.is_empty() {
        return Ok(BulkCreatureWaypointPaths::default());
    }

    let mut formation_path_ids = path_spawns
        .iter()
        .filter_map(|spawn| {
            spawn
                .formation_waypoint_path_id
                .filter(|path_id| *path_id != 0)
        })
        .collect::<HashSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    formation_path_ids.sort_unstable();

    let mut guid_ids = path_spawns
        .iter()
        .map(|spawn| spawn.guid)
        .collect::<HashSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    guid_ids.sort_unstable();

    let mut template_entries = path_spawns
        .iter()
        .map(|spawn| spawn.entry)
        .collect::<HashSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    template_entries.sort_unstable();

    Ok(BulkCreatureWaypointPaths {
        formation_paths: get_waypoint_paths(pool, &formation_path_ids).await?,
        guid_paths: get_creature_guid_waypoint_paths(pool, &guid_ids).await?,
        template_paths: get_creature_template_waypoint_paths(pool, &template_entries, 0).await?,
    })
}

fn attach_bulk_creature_waypoint_paths(
    spawns: &mut [CreatureSpawnQuery],
    paths: &BulkCreatureWaypointPaths,
) {
    for spawn in spawns {
        let movement_type = creature_effective_movement_type(spawn);
        if movement_type != 2 && movement_type != 4 {
            continue;
        }

        if let Some(formation_path) = spawn
            .formation_waypoint_path_id
            .filter(|path_id| *path_id != 0)
            .and_then(|path_id| paths.formation_paths.get(&path_id))
            .filter(|path| !path.is_empty())
        {
            spawn.waypoint_path = formation_path.clone();
            continue;
        }

        if let Some(guid_path) = paths
            .guid_paths
            .get(&spawn.guid)
            .filter(|path| !path.is_empty())
        {
            spawn.waypoint_path = guid_path.clone();
            continue;
        }

        spawn.waypoint_path = paths
            .template_paths
            .get(&(spawn.entry, 0))
            .cloned()
            .unwrap_or_default();
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

async fn get_waypoint_paths(
    pool: &MySqlPool,
    path_ids: &[u32],
) -> Result<HashMap<u32, Vec<CreatureWaypointQuery>>, DbError> {
    if path_ids.is_empty() {
        return Ok(HashMap::new());
    }
    let _query_timer =
        crate::observability::DbQueryTimer::start("creature_waypoint_path_bulk_load");
    let mut builder: QueryBuilder<MySql> = QueryBuilder::new(
        "SELECT PathId AS path_id, Point AS point, \
                CAST(PositionX AS DOUBLE) AS position_x, \
                CAST(PositionY AS DOUBLE) AS position_y, \
                CAST(PositionZ AS DOUBLE) AS position_z, \
                CAST(Orientation AS DOUBLE) AS orientation, \
                WaitTime AS wait_time, ScriptId AS script_id \
         FROM waypoint_path \
         WHERE PathId IN (",
    );
    let mut separated = builder.separated(", ");
    for path_id in path_ids {
        separated.push_bind(*path_id);
    }
    separated.push_unseparated(") ORDER BY PathId, Point");

    let rows = builder
        .build_query_as::<WaypointPathRow>()
        .fetch_all(pool)
        .await?;
    let mut grouped: HashMap<u32, Vec<CreatureWaypointRow>> = HashMap::new();
    for row in rows {
        grouped
            .entry(row.path_id)
            .or_default()
            .push(row.into_waypoint_row());
    }
    Ok(grouped
        .into_iter()
        .map(|(path_id, rows)| (path_id, creature_waypoint_rows_into_path(rows)))
        .collect())
}

async fn get_creature_guid_waypoint_paths(
    pool: &MySqlPool,
    guids: &[u32],
) -> Result<HashMap<u32, Vec<CreatureWaypointQuery>>, DbError> {
    if guids.is_empty() {
        return Ok(HashMap::new());
    }
    let _query_timer =
        crate::observability::DbQueryTimer::start("creature_guid_waypoint_path_bulk_load");
    let mut builder: QueryBuilder<MySql> = QueryBuilder::new(
        "SELECT Id AS guid, Point AS point, \
                CAST(PositionX AS DOUBLE) AS position_x, \
                CAST(PositionY AS DOUBLE) AS position_y, \
                CAST(PositionZ AS DOUBLE) AS position_z, \
                CAST(Orientation AS DOUBLE) AS orientation, \
                WaitTime AS wait_time, ScriptId AS script_id \
         FROM creature_movement \
         WHERE Id IN (",
    );
    let mut separated = builder.separated(", ");
    for guid in guids {
        separated.push_bind(*guid);
    }
    separated.push_unseparated(") ORDER BY Id, Point");

    let rows = builder
        .build_query_as::<CreatureGuidWaypointRow>()
        .fetch_all(pool)
        .await?;
    let mut grouped: HashMap<u32, Vec<CreatureWaypointRow>> = HashMap::new();
    for row in rows {
        grouped
            .entry(row.guid)
            .or_default()
            .push(row.into_waypoint_row());
    }
    Ok(grouped
        .into_iter()
        .map(|(guid, rows)| (guid, creature_waypoint_rows_into_path(rows)))
        .collect())
}

async fn get_creature_template_waypoint_paths(
    pool: &MySqlPool,
    entries: &[u32],
    path_id: u32,
) -> Result<HashMap<(u32, u32), Vec<CreatureWaypointQuery>>, DbError> {
    if entries.is_empty() {
        return Ok(HashMap::new());
    }
    let _query_timer =
        crate::observability::DbQueryTimer::start("creature_template_waypoint_path_bulk_load");
    let mut builder: QueryBuilder<MySql> = QueryBuilder::new(
        "SELECT Entry AS entry, PathId AS path_id, Point AS point, \
                CAST(PositionX AS DOUBLE) AS position_x, \
                CAST(PositionY AS DOUBLE) AS position_y, \
                CAST(PositionZ AS DOUBLE) AS position_z, \
                CAST(Orientation AS DOUBLE) AS orientation, \
                WaitTime AS wait_time, ScriptId AS script_id \
         FROM creature_movement_template \
         WHERE PathId = ",
    );
    builder.push_bind(path_id);
    builder.push(" AND Entry IN (");
    let mut separated = builder.separated(", ");
    for entry in entries {
        separated.push_bind(*entry);
    }
    separated.push_unseparated(") ORDER BY Entry, PathId, Point");

    let rows = builder
        .build_query_as::<CreatureTemplateWaypointRow>()
        .fetch_all(pool)
        .await?;
    let mut grouped: HashMap<(u32, u32), Vec<CreatureWaypointRow>> = HashMap::new();
    for row in rows {
        grouped
            .entry((row.entry, row.path_id))
            .or_default()
            .push(row.into_waypoint_row());
    }
    Ok(grouped
        .into_iter()
        .map(|(key, rows)| (key, creature_waypoint_rows_into_path(rows)))
        .collect())
}

#[cfg(test)]
mod world_data_tests {
    use super::*;

    #[test]
    fn bulk_creature_waypoint_attachment_preserves_cmangos_precedence() {
        let mut spawns = vec![
            test_creature_spawn(1, 10, 2, Some(100)),
            test_creature_spawn(2, 20, 2, Some(200)),
            test_creature_spawn(3, 30, 2, None),
            test_creature_spawn(4, 40, 0, Some(400)),
        ];
        spawns[3].template.movement_type = 0;

        let paths = BulkCreatureWaypointPaths {
            formation_paths: HashMap::from([
                (100, vec![test_waypoint(11)]),
                (200, Vec::new()),
                (400, vec![test_waypoint(44)]),
            ]),
            guid_paths: HashMap::from([
                (1, vec![test_waypoint(12)]),
                (2, vec![test_waypoint(22)]),
                (3, Vec::new()),
            ]),
            template_paths: HashMap::from([
                ((10, 0), vec![test_waypoint(13)]),
                ((20, 0), vec![test_waypoint(23)]),
                ((30, 0), vec![test_waypoint(33)]),
                ((40, 0), vec![test_waypoint(43)]),
            ]),
        };

        attach_bulk_creature_waypoint_paths(&mut spawns, &paths);

        assert_eq!(spawns[0].waypoint_path, vec![test_waypoint(11)]);
        assert_eq!(spawns[1].waypoint_path, vec![test_waypoint(22)]);
        assert_eq!(spawns[2].waypoint_path, vec![test_waypoint(33)]);
        assert!(spawns[3].waypoint_path.is_empty());
    }

    #[test]
    fn waypoint_rows_with_zero_point_are_invalid_for_bulk_fallback() {
        let path =
            creature_waypoint_rows_into_path(vec![test_waypoint_row(0), test_waypoint_row(1)]);

        assert!(path.is_empty());
    }

    #[test]
    fn creature_pool_filter_keeps_pool_limit_members() {
        let direct = test_creature_spawn(10, 110, 0, None);
        let mut pooled_first = test_creature_spawn(20, 120, 0, None);
        pooled_first.guid_pool_id = Some(7);
        pooled_first.pool_max_limit = Some(1);
        let mut pooled_second = test_creature_spawn(30, 130, 0, None);
        pooled_second.guid_pool_id = Some(7);
        pooled_second.pool_max_limit = Some(1);

        let spawns = filter_creature_pool_spawns(vec![pooled_second, direct, pooled_first]);

        assert_eq!(spawns.len(), 2);
        assert!(spawns.iter().any(|spawn| spawn.guid == 10));
        assert_eq!(
            spawns
                .iter()
                .filter(|spawn| spawn.guid_pool_id == Some(7))
                .count(),
            1
        );
    }

    #[test]
    fn gameobject_pool_filter_keeps_pool_limit_members() {
        let direct = test_gameobject_spawn(10, 110);
        let mut pooled_first = test_gameobject_spawn(20, 120);
        pooled_first.guid_pool_id = Some(9);
        pooled_first.pool_max_limit = Some(1);
        let mut pooled_second = test_gameobject_spawn(30, 130);
        pooled_second.guid_pool_id = Some(9);
        pooled_second.pool_max_limit = Some(1);

        let spawns = filter_gameobject_pool_spawns(vec![pooled_second, direct, pooled_first]);

        assert_eq!(spawns.len(), 2);
        assert!(spawns.iter().any(|spawn| spawn.guid == 10));
        assert_eq!(
            spawns
                .iter()
                .filter(|spawn| spawn.guid_pool_id == Some(9))
                .count(),
            1
        );
    }

    #[test]
    fn pool_roll_prefers_explicit_chance_hit_before_equal_members() {
        let mut rng = rand::thread_rng();
        let mut explicit = test_creature_spawn(20, 120, 0, None);
        explicit.guid_pool_id = Some(7);
        explicit.pool_max_limit = Some(1);
        explicit.pool_chance = 100.0;
        let mut equal = test_creature_spawn(30, 130, 0, None);
        equal.guid_pool_id = Some(7);
        equal.pool_max_limit = Some(1);

        let selected = roll_creature_pool_members(vec![equal, explicit], 1, &mut rng);

        assert_eq!(selected.len(), 1);
        assert_eq!(selected[0].guid, 20);
    }

    fn test_waypoint(point: u32) -> CreatureWaypointQuery {
        CreatureWaypointQuery {
            point,
            position_x: point as f32,
            position_y: point as f32 + 1.0,
            position_z: point as f32 + 2.0,
            orientation: Some(point as f32 + 3.0),
            wait_time: point * 10,
            script_id: point * 100,
        }
    }

    fn test_waypoint_row(point: u32) -> CreatureWaypointRow {
        CreatureWaypointRow {
            point,
            position_x: point as f64,
            position_y: point as f64 + 1.0,
            position_z: point as f64 + 2.0,
            orientation: point as f64 + 3.0,
            wait_time: point * 10,
            script_id: point * 100,
        }
    }

    fn test_creature_spawn(
        guid: u32,
        entry: u32,
        movement_type: u8,
        formation_waypoint_path_id: Option<u32>,
    ) -> CreatureSpawnQuery {
        CreatureSpawnQuery {
            guid,
            entry,
            map: 0,
            game_event: None,
            guid_pool_id: None,
            entry_pool_id: None,
            pool_max_limit: None,
            pool_chance: 0.0,
            addon_emote: 0,
            position_x: 0.0,
            position_y: 0.0,
            position_z: 0.0,
            orientation: 0.0,
            spawn_time_secs_min: 25,
            spawn_time_secs_max: 25,
            spawn_dist: 0.0,
            movement_type,
            formation_waypoint_path_id,
            template: CreatureTemplateQuery {
                entry,
                name: format!("Creature {entry}"),
                subname: None,
                min_level: 1,
                max_level: 1,
                display_id1: 0,
                display_id2: 0,
                display_id3: 0,
                display_id4: 0,
                display_id_probability1: 0,
                display_id_probability2: 0,
                display_id_probability3: 0,
                display_id_probability4: 0,
                model_gender1: 2,
                model_gender2: 2,
                model_gender3: 2,
                model_gender4: 2,
                model_other_gender1: 0,
                model_other_gender2: 0,
                model_other_gender3: 0,
                model_other_gender4: 0,
                model_other_gender_gender1: 2,
                model_other_gender_gender2: 2,
                model_other_gender_gender3: 2,
                model_other_gender_gender4: 2,
                model_bounding_radius: 0.0,
                model_combat_reach: 0.0,
                faction: 0,
                scale: 1.0,
                speed_walk: 1.0,
                speed_run: 1.0,
                detection_range: 20,
                call_for_help: 0,
                pursuit: 0,
                leash: 0,
                family: 0,
                creature_type: 0,
                creature_type_flags: 0,
                inhabit_type: 3,
                npc_flags: 0,
                unit_flags: 0,
                dynamic_flags: 0,
                static_flags2: 0,
                extra_flags: 0,
                unit_class: 1,
                base_strength: Some(20),
                rank: 0,
                health_multiplier: 1.0,
                power_multiplier: 1.0,
                damage_multiplier: 1.0,
                damage_variance: 1.0,
                armor_multiplier: 1.0,
                strength_multiplier: 1.0,
                min_level_health: 42,
                max_level_health: 42,
                min_level_mana: 0,
                max_level_mana: 0,
                min_melee_dmg: 1.0,
                max_melee_dmg: 2.0,
                min_ranged_dmg: 0.0,
                max_ranged_dmg: 0.0,
                armor: 0,
                resistance_holy: 0,
                resistance_fire: 0,
                resistance_nature: 0,
                resistance_frost: 0,
                resistance_shadow: 0,
                resistance_arcane: 0,
                melee_attack_power: 0,
                ranged_attack_power: 0,
                min_loot_gold: 0,
                max_loot_gold: 0,
                pickpocket_loot_id: 0,
                melee_base_attack_time: 2000,
                ranged_base_attack_time: 2000,
                damage_school: 0,
                trainer_type: 0,
                trainer_class: 0,
                pet_spell_data_id: 0,
                spell_list: 0,
                civilian: 0,
                corpse_decay: 0,
                movement_type: 0,
                equipment_template_id: 0,
                equip_display_id1: 0,
                equip_display_id2: 0,
                equip_display_id3: 0,
                equip_class1: 0,
                equip_class2: 0,
                equip_class3: 0,
                equip_subclass1: 0,
                equip_subclass2: 0,
                equip_subclass3: 0,
                equip_material1: 0,
                equip_material2: 0,
                equip_material3: 0,
                equip_inventory_type1: 0,
                equip_inventory_type2: 0,
                equip_inventory_type3: 0,
                equip_sheath1: 0,
                equip_sheath2: 0,
                equip_sheath3: 0,
                experience_multiplier: 1.0,
            },
            waypoint_path: Vec::new(),
        }
    }

    fn test_gameobject_spawn(guid: u32, entry: u32) -> GameObjectSpawnQuery {
        GameObjectSpawnQuery {
            guid,
            entry,
            map: 0,
            game_event: None,
            guid_pool_id: None,
            entry_pool_id: None,
            pool_max_limit: None,
            pool_chance: 0.0,
            position_x: 0.0,
            position_y: 0.0,
            position_z: 0.0,
            orientation: 0.0,
            rotation0: 0.0,
            rotation1: 0.0,
            rotation2: 0.0,
            rotation3: 1.0,
            spawn_time_secs_min: 25,
            spawn_time_secs_max: 25,
            state: -1,
            anim_progress: 100,
            template: GameObjectTemplateQuery {
                entry,
                object_type: 3,
                display_id: 0,
                name: format!("GameObject {entry}"),
                icon_name: String::new(),
                faction: 0,
                flags: 0,
                size: 1.0,
                raw_data: [0; 24],
            },
        }
    }
}

#[derive(Debug, Clone, FromRow)]
struct VendorItemRow {
    item: u32,
    max_count: u8,
    incr_time: u32,
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
    group_id: u8,
    min_count: u32,
    max_count: u32,
    display_id: u32,
    chance_or_quest_chance: f32,
}

#[derive(Debug, Clone, FromRow)]
struct QuestTemplateRow {
    entry: u32,
    method: u32,
    zone_or_sort: i16,
    min_level: u8,
    max_level: u8,
    quest_level: u32,
    quest_type: u32,
    required_classes: u32,
    required_races: u32,
    required_skill: u32,
    required_skill_value: u32,
    required_condition: u32,
    rep_objective_faction: u32,
    rep_objective_value: i32,
    required_min_rep_faction: u32,
    required_min_rep_value: i32,
    required_max_rep_faction: u32,
    required_max_rep_value: i32,
    special_flags: u32,
    prev_quest_id: i32,
    next_quest_id: i32,
    exclusive_group: i32,
    next_quest_in_chain: u32,
    rew_or_req_money: i32,
    rew_money_max_level: u32,
    rew_spell: u32,
    rew_spell_cast: u32,
    src_item_id: u32,
    src_item_count: u32,
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
    req_source_id1: u32,
    req_source_id2: u32,
    req_source_id3: u32,
    req_source_id4: u32,
    req_source_count1: u32,
    req_source_count2: u32,
    req_source_count3: u32,
    req_source_count4: u32,
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
    rew_rep_faction1: u32,
    rew_rep_faction2: u32,
    rew_rep_faction3: u32,
    rew_rep_faction4: u32,
    rew_rep_faction5: u32,
    rew_rep_value1: i32,
    rew_rep_value2: i32,
    rew_rep_value3: i32,
    rew_rep_value4: i32,
    rew_rep_value5: i32,
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
            min_level: self.min_level,
            max_level: self.max_level,
            quest_level: self.quest_level,
            quest_type: self.quest_type,
            required_classes: self.required_classes,
            required_races: self.required_races,
            required_skill: self.required_skill,
            required_skill_value: self.required_skill_value,
            required_condition: self.required_condition,
            rep_objective_faction: self.rep_objective_faction,
            rep_objective_value: self.rep_objective_value,
            required_min_rep_faction: self.required_min_rep_faction,
            required_min_rep_value: self.required_min_rep_value,
            required_max_rep_faction: self.required_max_rep_faction,
            required_max_rep_value: self.required_max_rep_value,
            special_flags: self.special_flags,
            prev_quest_id: self.prev_quest_id,
            next_quest_id: self.next_quest_id,
            exclusive_group: self.exclusive_group,
            next_quest_in_chain: self.next_quest_in_chain,
            rew_or_req_money: self.rew_or_req_money,
            rew_money_max_level: self.rew_money_max_level,
            rew_spell: self.rew_spell,
            rew_spell_cast: self.rew_spell_cast,
            src_item_id: self.src_item_id,
            src_item_count: self.src_item_count,
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
            req_source_id: [
                self.req_source_id1,
                self.req_source_id2,
                self.req_source_id3,
                self.req_source_id4,
            ],
            req_source_count: [
                self.req_source_count1,
                self.req_source_count2,
                self.req_source_count3,
                self.req_source_count4,
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
            rew_rep_faction: [
                self.rew_rep_faction1,
                self.rew_rep_faction2,
                self.rew_rep_faction3,
                self.rew_rep_faction4,
                self.rew_rep_faction5,
            ],
            rew_rep_value: [
                self.rew_rep_value1,
                self.rew_rep_value2,
                self.rew_rep_value3,
                self.rew_rep_value4,
                self.rew_rep_value5,
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
            group_id: self.group_id,
            min_count: self.min_count,
            max_count: self.max_count,
            display_id: self.display_id,
            chance_or_quest_chance: self.chance_or_quest_chance,
        }
    }
}

impl VendorItemRow {
    fn into_query(self) -> VendorItemQuery {
        VendorItemQuery {
            item: self.item,
            max_count: self.max_count as u32,
            incr_time: self.incr_time,
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

#[derive(Debug, Clone, FromRow)]
struct WaypointPathRow {
    path_id: u32,
    point: u32,
    position_x: f64,
    position_y: f64,
    position_z: f64,
    orientation: f64,
    wait_time: u32,
    script_id: u32,
}

#[derive(Debug, Clone, FromRow)]
struct CreatureGuidWaypointRow {
    guid: u32,
    point: u32,
    position_x: f64,
    position_y: f64,
    position_z: f64,
    orientation: f64,
    wait_time: u32,
    script_id: u32,
}

#[derive(Debug, Clone, FromRow)]
struct CreatureTemplateWaypointRow {
    entry: u32,
    path_id: u32,
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

#[derive(Debug, Clone, FromRow)]
struct GameObjectTemplateRow {
    entry: u32,
    object_type: u8,
    display_id: u32,
    name: String,
    icon_name: String,
    faction: u32,
    flags: u32,
    size: f32,
    data0: i32,
    data1: i32,
    data2: i32,
    data3: i32,
    data4: i32,
    data5: i32,
    data6: i32,
    data7: i32,
    data8: i32,
    data9: i32,
    data10: i32,
    data11: i32,
    data12: i32,
    data13: i32,
    data14: i32,
    data15: i32,
    data16: i32,
    data17: i32,
    data18: i32,
    data19: i32,
    data20: i32,
    data21: i32,
    data22: i32,
    data23: i32,
}

#[derive(Debug, Clone, FromRow)]
struct GameObjectSpawnRow {
    guid: u32,
    entry: u32,
    map: u32,
    game_event: Option<i16>,
    guid_pool_id: Option<u16>,
    entry_pool_id: Option<u16>,
    pool_max_limit: Option<u32>,
    pool_chance: f64,
    position_x: f64,
    position_y: f64,
    position_z: f64,
    orientation: f64,
    rotation0: f64,
    rotation1: f64,
    rotation2: f64,
    rotation3: f64,
    spawn_time_secs_min: i32,
    spawn_time_secs_max: i32,
    state: i8,
    anim_progress: u8,
    template_entry: u32,
    template_object_type: u8,
    template_display_id: u32,
    template_name: String,
    template_icon_name: String,
    template_faction: u32,
    template_flags: u32,
    template_size: f32,
    template_data0: i32,
    template_data1: i32,
    template_data2: i32,
    template_data3: i32,
    template_data4: i32,
    template_data5: i32,
    template_data6: i32,
    template_data7: i32,
    template_data8: i32,
    template_data9: i32,
    template_data10: i32,
    template_data11: i32,
    template_data12: i32,
    template_data13: i32,
    template_data14: i32,
    template_data15: i32,
    template_data16: i32,
    template_data17: i32,
    template_data18: i32,
    template_data19: i32,
    template_data20: i32,
    template_data21: i32,
    template_data22: i32,
    template_data23: i32,
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

impl WaypointPathRow {
    fn into_waypoint_row(self) -> CreatureWaypointRow {
        CreatureWaypointRow {
            point: self.point,
            position_x: self.position_x,
            position_y: self.position_y,
            position_z: self.position_z,
            orientation: self.orientation,
            wait_time: self.wait_time,
            script_id: self.script_id,
        }
    }
}

impl CreatureGuidWaypointRow {
    fn into_waypoint_row(self) -> CreatureWaypointRow {
        CreatureWaypointRow {
            point: self.point,
            position_x: self.position_x,
            position_y: self.position_y,
            position_z: self.position_z,
            orientation: self.orientation,
            wait_time: self.wait_time,
            script_id: self.script_id,
        }
    }
}

impl CreatureTemplateWaypointRow {
    fn into_waypoint_row(self) -> CreatureWaypointRow {
        CreatureWaypointRow {
            point: self.point,
            position_x: self.position_x,
            position_y: self.position_y,
            position_z: self.position_z,
            orientation: self.orientation,
            wait_time: self.wait_time,
            script_id: self.script_id,
        }
    }
}

impl GameObjectTemplateRow {
    fn into_query(self) -> GameObjectTemplateQuery {
        GameObjectTemplateQuery {
            entry: self.entry,
            object_type: self.object_type,
            display_id: self.display_id,
            name: self.name,
            icon_name: self.icon_name,
            faction: self.faction,
            flags: self.flags,
            size: self.size,
            raw_data: [
                self.data0 as u32,
                self.data1 as u32,
                self.data2 as u32,
                self.data3 as u32,
                self.data4 as u32,
                self.data5 as u32,
                self.data6 as u32,
                self.data7 as u32,
                self.data8 as u32,
                self.data9 as u32,
                self.data10 as u32,
                self.data11 as u32,
                self.data12 as u32,
                self.data13 as u32,
                self.data14 as u32,
                self.data15 as u32,
                self.data16 as u32,
                self.data17 as u32,
                self.data18 as u32,
                self.data19 as u32,
                self.data20 as u32,
                self.data21 as u32,
                self.data22 as u32,
                self.data23 as u32,
            ],
        }
    }
}

#[derive(Debug, Clone, FromRow)]
struct CreatureSpawnRow {
    guid: u32,
    entry: u32,
    map: u32,
    game_event: Option<i16>,
    guid_pool_id: Option<u16>,
    entry_pool_id: Option<u16>,
    pool_max_limit: Option<u32>,
    pool_chance: f64,
    addon_emote: u32,
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
    template_display_id_probability1: u32,
    template_display_id_probability2: u32,
    template_display_id_probability3: u32,
    template_display_id_probability4: u32,
    template_model_gender1: u8,
    template_model_gender2: u8,
    template_model_gender3: u8,
    template_model_gender4: u8,
    template_model_other_gender1: u32,
    template_model_other_gender2: u32,
    template_model_other_gender3: u32,
    template_model_other_gender4: u32,
    template_model_other_gender_gender1: u8,
    template_model_other_gender_gender2: u8,
    template_model_other_gender_gender3: u8,
    template_model_other_gender_gender4: u8,
    template_model_bounding_radius: f32,
    template_model_combat_reach: f32,
    template_faction: u32,
    template_scale: f32,
    template_speed_walk: f32,
    template_speed_run: f32,
    template_detection_range: u32,
    template_call_for_help: u32,
    template_pursuit: u32,
    template_leash: u32,
    template_family: i32,
    template_creature_type: u32,
    template_creature_type_flags: u32,
    template_inhabit_type: u32,
    template_npc_flags: u32,
    template_unit_flags: u32,
    template_dynamic_flags: u32,
    template_static_flags2: u32,
    template_extra_flags: u32,
    template_unit_class: u8,
    template_base_strength: Option<u32>,
    template_rank: u32,
    template_health_multiplier: f32,
    template_power_multiplier: f32,
    template_damage_multiplier: f32,
    template_damage_variance: f32,
    template_armor_multiplier: f32,
    template_strength_multiplier: f32,
    template_min_level_health: u32,
    template_max_level_health: u32,
    template_min_level_mana: u32,
    template_max_level_mana: u32,
    template_min_melee_dmg: f32,
    template_max_melee_dmg: f32,
    template_min_ranged_dmg: f32,
    template_max_ranged_dmg: f32,
    template_armor: u32,
    template_resistance_holy: i16,
    template_resistance_fire: i16,
    template_resistance_nature: i16,
    template_resistance_frost: i16,
    template_resistance_shadow: i16,
    template_resistance_arcane: i16,
    template_melee_attack_power: u32,
    template_ranged_attack_power: u32,
    template_min_loot_gold: u32,
    template_max_loot_gold: u32,
    template_pickpocket_loot_id: u32,
    template_melee_base_attack_time: u32,
    template_ranged_base_attack_time: u32,
    template_damage_school: i8,
    template_trainer_type: i8,
    template_trainer_class: u8,
    template_pet_spell_data_id: u32,
    template_spell_list: u32,
    template_civilian: u8,
    template_corpse_decay: u32,
    template_movement_type: u8,
    template_equipment_template_id: u32,
    template_equip_display_id1: u32,
    template_equip_display_id2: u32,
    template_equip_display_id3: u32,
    template_equip_class1: u32,
    template_equip_class2: u32,
    template_equip_class3: u32,
    template_equip_subclass1: u32,
    template_equip_subclass2: u32,
    template_equip_subclass3: u32,
    template_equip_material1: i32,
    template_equip_material2: i32,
    template_equip_material3: i32,
    template_equip_inventory_type1: u32,
    template_equip_inventory_type2: u32,
    template_equip_inventory_type3: u32,
    template_equip_sheath1: u32,
    template_equip_sheath2: u32,
    template_equip_sheath3: u32,
    template_experience_multiplier: f32,
}

impl CreatureSpawnRow {
    fn into_query(self) -> CreatureSpawnQuery {
        CreatureSpawnQuery {
            guid: self.guid,
            entry: self.entry,
            map: self.map,
            game_event: self.game_event,
            guid_pool_id: self.guid_pool_id,
            entry_pool_id: self.entry_pool_id,
            pool_max_limit: self.pool_max_limit,
            pool_chance: self.pool_chance as f32,
            addon_emote: self.addon_emote,
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
                display_id_probability1: self.template_display_id_probability1,
                display_id_probability2: self.template_display_id_probability2,
                display_id_probability3: self.template_display_id_probability3,
                display_id_probability4: self.template_display_id_probability4,
                model_gender1: self.template_model_gender1,
                model_gender2: self.template_model_gender2,
                model_gender3: self.template_model_gender3,
                model_gender4: self.template_model_gender4,
                model_other_gender1: self.template_model_other_gender1,
                model_other_gender2: self.template_model_other_gender2,
                model_other_gender3: self.template_model_other_gender3,
                model_other_gender4: self.template_model_other_gender4,
                model_other_gender_gender1: self.template_model_other_gender_gender1,
                model_other_gender_gender2: self.template_model_other_gender_gender2,
                model_other_gender_gender3: self.template_model_other_gender_gender3,
                model_other_gender_gender4: self.template_model_other_gender_gender4,
                model_bounding_radius: self.template_model_bounding_radius,
                model_combat_reach: self.template_model_combat_reach,
                faction: self.template_faction,
                scale: self.template_scale,
                speed_walk: self.template_speed_walk,
                speed_run: self.template_speed_run,
                detection_range: self.template_detection_range,
                call_for_help: self.template_call_for_help,
                pursuit: self.template_pursuit,
                leash: self.template_leash,
                family: self.template_family,
                creature_type: self.template_creature_type,
                creature_type_flags: self.template_creature_type_flags,
                inhabit_type: self.template_inhabit_type,
                npc_flags: self.template_npc_flags,
                unit_flags: self.template_unit_flags,
                dynamic_flags: self.template_dynamic_flags,
                static_flags2: self.template_static_flags2,
                extra_flags: self.template_extra_flags,
                unit_class: self.template_unit_class,
                base_strength: self.template_base_strength,
                rank: self.template_rank,
                health_multiplier: self.template_health_multiplier,
                power_multiplier: self.template_power_multiplier,
                damage_multiplier: self.template_damage_multiplier,
                damage_variance: self.template_damage_variance,
                armor_multiplier: self.template_armor_multiplier,
                strength_multiplier: self.template_strength_multiplier,
                min_level_health: self.template_min_level_health,
                max_level_health: self.template_max_level_health,
                min_level_mana: self.template_min_level_mana,
                max_level_mana: self.template_max_level_mana,
                min_melee_dmg: self.template_min_melee_dmg,
                max_melee_dmg: self.template_max_melee_dmg,
                min_ranged_dmg: self.template_min_ranged_dmg,
                max_ranged_dmg: self.template_max_ranged_dmg,
                armor: self.template_armor,
                resistance_holy: self.template_resistance_holy,
                resistance_fire: self.template_resistance_fire,
                resistance_nature: self.template_resistance_nature,
                resistance_frost: self.template_resistance_frost,
                resistance_shadow: self.template_resistance_shadow,
                resistance_arcane: self.template_resistance_arcane,
                melee_attack_power: self.template_melee_attack_power,
                ranged_attack_power: self.template_ranged_attack_power,
                min_loot_gold: self.template_min_loot_gold,
                max_loot_gold: self.template_max_loot_gold,
                pickpocket_loot_id: self.template_pickpocket_loot_id,
                melee_base_attack_time: self.template_melee_base_attack_time,
                ranged_base_attack_time: self.template_ranged_base_attack_time,
                damage_school: self.template_damage_school,
                trainer_type: self.template_trainer_type,
                trainer_class: self.template_trainer_class,
                pet_spell_data_id: self.template_pet_spell_data_id,
                spell_list: self.template_spell_list,
                civilian: self.template_civilian,
                corpse_decay: self.template_corpse_decay,
                movement_type: self.template_movement_type,
                equipment_template_id: self.template_equipment_template_id,
                equip_display_id1: self.template_equip_display_id1,
                equip_display_id2: self.template_equip_display_id2,
                equip_display_id3: self.template_equip_display_id3,
                equip_class1: self.template_equip_class1,
                equip_class2: self.template_equip_class2,
                equip_class3: self.template_equip_class3,
                equip_subclass1: self.template_equip_subclass1,
                equip_subclass2: self.template_equip_subclass2,
                equip_subclass3: self.template_equip_subclass3,
                equip_material1: self.template_equip_material1,
                equip_material2: self.template_equip_material2,
                equip_material3: self.template_equip_material3,
                equip_inventory_type1: self.template_equip_inventory_type1,
                equip_inventory_type2: self.template_equip_inventory_type2,
                equip_inventory_type3: self.template_equip_inventory_type3,
                equip_sheath1: self.template_equip_sheath1,
                equip_sheath2: self.template_equip_sheath2,
                equip_sheath3: self.template_equip_sheath3,
                experience_multiplier: self.template_experience_multiplier,
            },
            waypoint_path: Vec::new(),
        }
    }
}

impl GameObjectSpawnRow {
    fn into_query(self) -> GameObjectSpawnQuery {
        GameObjectSpawnQuery {
            guid: self.guid,
            entry: self.entry,
            map: self.map,
            game_event: self.game_event,
            guid_pool_id: self.guid_pool_id,
            entry_pool_id: self.entry_pool_id,
            pool_max_limit: self.pool_max_limit,
            pool_chance: self.pool_chance as f32,
            position_x: self.position_x as f32,
            position_y: self.position_y as f32,
            position_z: self.position_z as f32,
            orientation: self.orientation as f32,
            rotation0: self.rotation0 as f32,
            rotation1: self.rotation1 as f32,
            rotation2: self.rotation2 as f32,
            rotation3: self.rotation3 as f32,
            spawn_time_secs_min: self.spawn_time_secs_min,
            spawn_time_secs_max: self.spawn_time_secs_max,
            state: self.state,
            anim_progress: self.anim_progress,
            template: GameObjectTemplateQuery {
                entry: self.template_entry,
                object_type: self.template_object_type,
                display_id: self.template_display_id,
                name: self.template_name,
                icon_name: self.template_icon_name,
                faction: self.template_faction,
                flags: self.template_flags,
                size: self.template_size,
                raw_data: [
                    self.template_data0 as u32,
                    self.template_data1 as u32,
                    self.template_data2 as u32,
                    self.template_data3 as u32,
                    self.template_data4 as u32,
                    self.template_data5 as u32,
                    self.template_data6 as u32,
                    self.template_data7 as u32,
                    self.template_data8 as u32,
                    self.template_data9 as u32,
                    self.template_data10 as u32,
                    self.template_data11 as u32,
                    self.template_data12 as u32,
                    self.template_data13 as u32,
                    self.template_data14 as u32,
                    self.template_data15 as u32,
                    self.template_data16 as u32,
                    self.template_data17 as u32,
                    self.template_data18 as u32,
                    self.template_data19 as u32,
                    self.template_data20 as u32,
                    self.template_data21 as u32,
                    self.template_data22 as u32,
                    self.template_data23 as u32,
                ],
            },
        }
    }
}
