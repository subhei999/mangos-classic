
#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct CharacterEnumEntry {
    pub guid: u32,
    pub name: String,
    pub race: u8,
    pub class: u8,
    pub gender: u8,
    #[sqlx(rename = "playerBytes")]
    pub player_bytes: u32,
    #[sqlx(rename = "playerBytes2")]
    pub player_bytes2: u32,
    pub level: u8,
    pub xp: u32,
    pub zone: u32,
    pub map: u32,
    pub position_x: f32,
    pub position_y: f32,
    pub position_z: f32,
    pub orientation: f32,
    pub guildid: Option<u32>,
    #[sqlx(rename = "playerFlags")]
    pub player_flags: u32,
    pub at_login: u32,
    pub money: u32,
    pub cinematic: u8,
    pub health: u32,
    pub power1: u32,
    pub power2: u32,
    pub power3: u32,
    pub power4: u32,
    pub power5: u32,
    #[sqlx(rename = "watchedFaction")]
    pub watched_faction: u32,
    #[sqlx(rename = "exploredZones")]
    pub explored_zones: Option<String>,
    pub pet_entry: Option<u32>,
    pub pet_modelid: Option<u32>,
    pub pet_level: Option<u32>,
    #[sqlx(rename = "equipmentCache")]
    pub equipment_cache: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct NewCharacter {
    pub account_id: u32,
    pub name: String,
    pub race: u8,
    pub class: u8,
    pub gender: u8,
    pub skin: u8,
    pub face: u8,
    pub hair_style: u8,
    pub hair_color: u8,
    pub facial_hair: u8,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CreatedCharacter {
    pub guid: u32,
    pub account_id: u32,
    pub name: String,
    pub race: u8,
    pub class: u8,
    pub position: WorldPosition,
    pub zone: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CharacterDeleteMethod {
    HardDelete,
    Unlink,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CharacterDeleteOptions {
    pub method: CharacterDeleteMethod,
    pub min_level_for_unlink: u8,
    pub force_hard_delete: bool,
}

impl CharacterDeleteOptions {
    pub fn hard_delete() -> Self {
        Self {
            method: CharacterDeleteMethod::HardDelete,
            min_level_for_unlink: 0,
            force_hard_delete: true,
        }
    }
}

#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct CharacterNameQuery {
    pub guid: u32,
    pub name: String,
    pub race: u8,
    pub gender: u8,
    pub class: u8,
}

#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct CharacterSpell {
    pub spell: u32,
    pub active: u8,
    pub disabled: u8,
}

#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct CharacterAction {
    pub button: u8,
    pub action: u32,
    #[sqlx(rename = "type")]
    pub action_type: u8,
}

#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct CharacterReputation {
    pub faction: u32,
    pub standing: i32,
    pub flags: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CharacterQuestRewardResult {
    pub money: u32,
    pub reputations: Vec<CharacterReputation>,
}

#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct CharacterInventoryItem {
    pub bag: u32,
    pub slot: u8,
    pub item: u32,
    pub item_template: u32,
    pub count: u32,
    pub durability: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StarterItemTemplateRef {
    pub race: u8,
    pub class: u8,
    pub item_id: u32,
    pub slot: u8,
    pub amount: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InventoryDestroyResult {
    Removed { item: u32 },
    CountChanged { item: u32, count: u32 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InventoryMoveResult {
    Swapped,
    Merged {
        source_item: u32,
        source_count: Option<u32>,
        destination_item: u32,
        destination_count: u32,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InventorySplitResult {
    pub source_item: u32,
    pub source_count: u32,
    pub new_item: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ItemTemplateQuery {
    pub entry: u32,
    pub class: u32,
    pub subclass: u32,
    pub name: String,
    pub displayid: u32,
    pub quality: u32,
    pub flags: u32,
    pub buy_price: u32,
    pub sell_price: u32,
    pub inventory_type: u32,
    pub allowable_class: i32,
    pub allowable_race: i32,
    pub item_level: u32,
    pub required_level: u32,
    pub required_skill: u32,
    pub required_skill_rank: u32,
    pub required_spell: u32,
    pub required_honor_rank: u32,
    pub required_city_rank: u32,
    pub required_reputation_faction: u32,
    pub required_reputation_rank: u32,
    pub max_count: u32,
    pub stackable: u32,
    pub container_slots: u32,
    pub dmg_min1: f32,
    pub dmg_max1: f32,
    pub dmg_type1: u32,
    pub armor: u32,
    pub holy_res: u32,
    pub fire_res: u32,
    pub nature_res: u32,
    pub frost_res: u32,
    pub shadow_res: u32,
    pub arcane_res: u32,
    pub delay: u32,
    pub ammo_type: u32,
    pub ranged_mod_range: f32,
    pub bonding: u32,
    pub description: String,
    pub page_text: u32,
    pub language_id: u32,
    pub page_material: u32,
    pub start_quest: u32,
    pub lock_id: u32,
    pub material: i32,
    pub sheath: u32,
    pub random_property: u32,
    pub block: u32,
    pub itemset: u32,
    pub max_durability: u32,
    pub area: u32,
    pub map: i32,
    pub bag_family: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlayerWorldStats {
    pub base_health: u32,
    pub base_mana: u32,
    pub stats: [u32; 5],
    pub next_level_xp: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct CharacterProgressionState {
    pub level: u8,
    pub xp: u32,
    pub health: u32,
    pub power1: u32,
    pub power2: u32,
    pub power3: u32,
    pub power4: u32,
    pub power5: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, FromRow, Serialize, Deserialize)]
pub struct CharacterSkill {
    pub skill: u16,
    pub value: u16,
    pub max: u16,
}

#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct CharacterQuestStatus {
    pub quest: u32,
    pub status: u32,
    pub rewarded: u8,
    pub mobcount1: u32,
    pub mobcount2: u32,
    pub mobcount3: u32,
    pub mobcount4: u32,
}

#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct PlayerCorpseQuery {
    pub guid: u32,
    pub player: u32,
    pub position_x: f32,
    pub position_y: f32,
    pub position_z: f32,
    pub orientation: f32,
    pub map: u32,
    pub time: u64,
    pub corpse_type: u8,
    pub instance: u32,
    pub race: u8,
    pub class: u8,
    pub gender: u8,
    #[sqlx(rename = "playerBytes")]
    pub player_bytes: u32,
    #[sqlx(rename = "playerBytes2")]
    pub player_bytes2: u32,
    #[sqlx(rename = "equipmentCache")]
    pub equipment_cache: Option<String>,
    pub guildid: Option<u32>,
    #[sqlx(rename = "playerFlags")]
    pub player_flags: u32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct NewPlayerCorpse {
    pub guid: u32,
    pub player: u32,
    pub position: WorldPosition,
    pub time: u64,
    pub corpse_type: u8,
    pub instance: u32,
}

impl PlayerWorldStats {
    pub fn max_health(self) -> u32 {
        self.base_health + health_bonus_from_stamina(self.stats[2])
    }

    pub fn max_mana(self) -> u32 {
        if self.base_mana == 0 {
            return 0;
        }

        self.base_mana + mana_bonus_from_intellect(self.stats[3])
    }
}

