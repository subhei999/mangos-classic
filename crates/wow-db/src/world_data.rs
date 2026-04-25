use serde::{Deserialize, Serialize};
use sqlx::mysql::MySqlPool;
use sqlx::FromRow;

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
    pub faction: u32,
    pub scale: f32,
    pub family: i32,
    pub creature_type: u32,
    pub npc_flags: u32,
    pub unit_flags: u32,
    pub dynamic_flags: u32,
    pub rank: u32,
    pub min_level_health: u32,
    pub max_level_health: u32,
    pub min_melee_dmg: f32,
    pub max_melee_dmg: f32,
    pub melee_base_attack_time: u32,
    pub ranged_base_attack_time: u32,
    pub pet_spell_data_id: u32,
    pub civilian: u8,
}

#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct CreatureSpawnQuery {
    pub guid: u32,
    pub entry: u32,
    pub map: u32,
    pub position_x: f32,
    pub position_y: f32,
    pub position_z: f32,
    pub orientation: f32,
    pub template: CreatureTemplateQuery,
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

pub async fn get_creature_template_query(
    pool: &MySqlPool,
    entry: u32,
) -> Result<Option<CreatureTemplateQuery>, DbError> {
    let row = sqlx::query_as::<_, CreatureTemplateQuery>(
        "SELECT Entry AS entry, Name AS name, SubName AS subname, \
                MinLevel AS min_level, MaxLevel AS max_level, \
                DisplayId1 AS display_id1, DisplayId2 AS display_id2, \
                DisplayId3 AS display_id3, DisplayId4 AS display_id4, \
                Faction AS faction, Scale AS scale, Family AS family, \
                CreatureType AS creature_type, NpcFlags AS npc_flags, \
                UnitFlags AS unit_flags, DynamicFlags AS dynamic_flags, Rank AS rank, \
                MinLevelHealth AS min_level_health, MaxLevelHealth AS max_level_health, \
                MinMeleeDmg AS min_melee_dmg, MaxMeleeDmg AS max_melee_dmg, \
                MeleeBaseAttackTime AS melee_base_attack_time, \
                RangedBaseAttackTime AS ranged_base_attack_time, \
                PetSpellDataId AS pet_spell_data_id, Civilian AS civilian \
         FROM creature_template \
         WHERE Entry = ?",
    )
    .bind(entry)
    .fetch_optional(pool)
    .await?;

    Ok(row)
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
                creature_template.Entry AS template_entry, creature_template.Name AS template_name, \
                creature_template.SubName AS template_subname, \
                creature_template.MinLevel AS template_min_level, \
                creature_template.MaxLevel AS template_max_level, \
                creature_template.DisplayId1 AS template_display_id1, \
                creature_template.DisplayId2 AS template_display_id2, \
                creature_template.DisplayId3 AS template_display_id3, \
                creature_template.DisplayId4 AS template_display_id4, \
                creature_template.Faction AS template_faction, creature_template.Scale AS template_scale, \
                creature_template.Family AS template_family, \
                creature_template.CreatureType AS template_creature_type, \
                creature_template.NpcFlags AS template_npc_flags, \
                creature_template.UnitFlags AS template_unit_flags, \
                creature_template.DynamicFlags AS template_dynamic_flags, \
                creature_template.Rank AS template_rank, \
                creature_template.MinLevelHealth AS template_min_level_health, \
                creature_template.MaxLevelHealth AS template_max_level_health, \
                creature_template.MinMeleeDmg AS template_min_melee_dmg, \
                creature_template.MaxMeleeDmg AS template_max_melee_dmg, \
                creature_template.MeleeBaseAttackTime AS template_melee_base_attack_time, \
                creature_template.RangedBaseAttackTime AS template_ranged_base_attack_time, \
                creature_template.PetSpellDataId AS template_pet_spell_data_id, \
                creature_template.Civilian AS template_civilian \
         FROM creature \
         JOIN creature_template ON creature.id = creature_template.Entry \
         WHERE creature.map = ? \
           AND creature.position_x BETWEEN ? AND ? \
           AND creature.position_y BETWEEN ? AND ? \
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
    .bind(limit)
    .fetch_all(pool)
    .await?;

    Ok(rows.into_iter().map(CreatureSpawnRow::into_query).collect())
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
struct CreatureSpawnRow {
    guid: u32,
    entry: u32,
    map: u32,
    position_x: f64,
    position_y: f64,
    position_z: f64,
    orientation: f64,
    template_entry: u32,
    template_name: String,
    template_subname: Option<String>,
    template_min_level: u8,
    template_max_level: u8,
    template_display_id1: u32,
    template_display_id2: u32,
    template_display_id3: u32,
    template_display_id4: u32,
    template_faction: u32,
    template_scale: f32,
    template_family: i32,
    template_creature_type: u32,
    template_npc_flags: u32,
    template_unit_flags: u32,
    template_dynamic_flags: u32,
    template_rank: u32,
    template_min_level_health: u32,
    template_max_level_health: u32,
    template_min_melee_dmg: f32,
    template_max_melee_dmg: f32,
    template_melee_base_attack_time: u32,
    template_ranged_base_attack_time: u32,
    template_pet_spell_data_id: u32,
    template_civilian: u8,
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
                faction: self.template_faction,
                scale: self.template_scale,
                family: self.template_family,
                creature_type: self.template_creature_type,
                npc_flags: self.template_npc_flags,
                unit_flags: self.template_unit_flags,
                dynamic_flags: self.template_dynamic_flags,
                rank: self.template_rank,
                min_level_health: self.template_min_level_health,
                max_level_health: self.template_max_level_health,
                min_melee_dmg: self.template_min_melee_dmg,
                max_melee_dmg: self.template_max_melee_dmg,
                melee_base_attack_time: self.template_melee_base_attack_time,
                ranged_base_attack_time: self.template_ranged_base_attack_time,
                pet_spell_data_id: self.template_pet_spell_data_id,
                civilian: self.template_civilian,
            },
        }
    }
}
