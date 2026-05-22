use std::cmp::Reverse;
use wow_proto::world::WorldOpcode;

const WARRIOR_HEROIC_STRIKE_RANK_1: u32 = 78;
const HUNTER_RAPTOR_STRIKE_RANK_1: u32 = 2973;
const HEROIC_STRIKE_RAGE_COST: u32 = 150;
const HEROIC_STRIKE_FIXTURE_DAMAGE: u32 = 11;
const RAPTOR_STRIKE_MANA_COST: u32 = 15;
const RAPTOR_STRIKE_FIXTURE_DAMAGE: u32 = 12;
const SPELL_CAST_TARGET_LOCKED: u16 = 0x4000;

fn test_smsg_pong_opcode() -> u16 {
    u32::from(WorldOpcode::SmsgPong) as u16
}

fn read_char_create_request(body: &[u8]) -> wow_proto::CharCreateRequest {
    let mut body = body;
    wow_proto::CharCreateRequest::read(&mut body).unwrap()
}

fn read_gossip_select_option_request(body: &[u8]) -> wow_proto::GossipSelectOptionRequest {
    let mut body = body;
    wow_proto::GossipSelectOptionRequest::read(&mut body).unwrap()
}

fn read_trainer_buy_spell_request(body: &[u8]) -> wow_proto::TrainerBuySpellRequest {
    let mut body = body;
    wow_proto::TrainerBuySpellRequest::read(&mut body).unwrap()
}

fn read_buy_item_request(body: &[u8]) -> wow_proto::BuyItemRequest {
    let mut body = body;
    wow_proto::BuyItemRequest::read(&mut body).unwrap()
}

fn read_use_item_request(body: &[u8]) -> wow_proto::UseItemRequest {
    let mut body = body;
    wow_proto::UseItemRequest::read(&mut body).unwrap()
}

fn read_destroy_item_request(body: &[u8]) -> wow_proto::DestroyItemRequest {
    let mut body = body;
    wow_proto::DestroyItemRequest::read(&mut body).unwrap()
}

fn read_split_item_request(body: &[u8]) -> wow_proto::SplitItemRequest {
    let mut body = body;
    wow_proto::SplitItemRequest::read(&mut body).unwrap()
}

fn read_message_chat_request(body: &[u8]) -> wow_proto::MessageChatRequest {
    let mut body = body;
    wow_proto::MessageChatRequest::read(&mut body).unwrap()
}

fn read_join_channel_request(body: &[u8]) -> wow_proto::JoinChannelRequest {
    let mut body = body;
    wow_proto::JoinChannelRequest::read(&mut body).unwrap()
}

fn read_text_emote_request(body: &[u8]) -> wow_proto::TextEmoteRequest {
    let mut body = body;
    wow_proto::TextEmoteRequest::read(&mut body).unwrap()
}

fn read_cast_spell_request(body: &[u8]) -> wow_proto::CastSpellRequest {
    let mut body = body;
    wow_proto::CastSpellRequest::read(&mut body).unwrap()
}

fn read_attack_swing_request(body: &[u8]) -> wow_proto::AttackSwingRequest {
    let mut body = body;
    wow_proto::AttackSwingRequest::read(&mut body).unwrap()
}

fn read_loot_request(body: &[u8]) -> wow_proto::LootRequest {
    let mut body = body;
    wow_proto::LootRequest::read(&mut body).unwrap()
}

fn read_loot_release_request(body: &[u8]) -> wow_proto::LootReleaseRequest {
    let mut body = body;
    wow_proto::LootReleaseRequest::read(&mut body).unwrap()
}

#[derive(Debug, Clone, PartialEq)]
struct CharCreatePacket {
    name: String,
    race: u8,
    class: u8,
    gender: u8,
    skin: u8,
    face: u8,
    hair_style: u8,
    hair_color: u8,
    facial_hair: u8,
    outfit_id: u8,
}

impl CharCreatePacket {
    fn read(body: &[u8]) -> anyhow::Result<Self> {
        let request = read_char_create_request(body);
        Ok(Self {
            name: request.name,
            race: request.race,
            class: request.class,
            gender: request.gender,
            skin: request.skin,
            face: request.face,
            hair_style: request.hair_style,
            hair_color: request.hair_color,
            facial_hair: request.facial_hair,
            outfit_id: request.outfit_id,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct GossipSelectOption {
    guid: ObjectGuid,
    option: u32,
}

impl GossipSelectOption {
    fn read(body: &[u8]) -> anyhow::Result<Self> {
        let request = read_gossip_select_option_request(body);
        Ok(Self {
            guid: ObjectGuid::from_raw(request.raw_guid),
            option: request.option,
        })
    }

    fn is_supported_browse_option(&self) -> bool {
        self.option == 0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct TrainerBuySpellRequest {
    trainer_guid: ObjectGuid,
    spell: u32,
}

impl TrainerBuySpellRequest {
    fn read(body: &[u8]) -> anyhow::Result<Self> {
        let request = read_trainer_buy_spell_request(body);
        Ok(Self {
            trainer_guid: ObjectGuid::from_raw(request.trainer_raw_guid),
            spell: request.spell,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct BuyItemRequest {
    vendor_guid: ObjectGuid,
    item: u32,
    count: u8,
}

impl BuyItemRequest {
    fn read(body: &[u8]) -> anyhow::Result<Self> {
        let request = read_buy_item_request(body);
        Ok(Self {
            vendor_guid: ObjectGuid::from_raw(request.vendor_raw_guid),
            item: request.item,
            count: request.count,
        })
    }
}

#[derive(Debug, Clone, PartialEq)]
struct UseItemPacket {
    bag: u8,
    slot: u8,
    spell_index: u8,
    targets: SpellCastTargets,
}

impl UseItemPacket {
    fn read(body: &[u8]) -> anyhow::Result<Self> {
        let request = read_use_item_request(body);
        Ok(Self {
            bag: normalize_client_bag(request.bag),
            slot: request.slot,
            spell_index: request.spell_index,
            targets: request.targets,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct DestroyItemRequest {
    bag: u8,
    slot: u8,
    count: u8,
}

impl DestroyItemRequest {
    fn read(body: &[u8]) -> anyhow::Result<Self> {
        let request = read_destroy_item_request(body);
        Ok(Self {
            bag: normalize_client_bag(request.bag),
            slot: request.slot,
            count: request.count,
        })
    }

    fn is_supported_destroy(&self) -> bool {
        is_supported_storage_position(self.bag, self.slot)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct SplitItemRequest {
    src_bag: u8,
    src_slot: u8,
    dst_bag: u8,
    dst_slot: u8,
    count: u8,
}

impl SplitItemRequest {
    fn read(body: &[u8]) -> anyhow::Result<Self> {
        let request = read_split_item_request(body);
        Ok(Self {
            src_bag: normalize_client_bag(request.src_bag),
            src_slot: request.src_slot,
            dst_bag: normalize_client_bag(request.dst_bag),
            dst_slot: request.dst_slot,
            count: request.count,
        })
    }

    fn is_supported_split(&self) -> bool {
        self.count != 0
            && is_supported_storage_position(self.src_bag, self.src_slot)
            && is_supported_storage_position(self.dst_bag, self.dst_slot)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ChatMessage {
    chat_type: u32,
    language: u32,
    message: String,
}

impl ChatMessage {
    fn read(body: &[u8]) -> anyhow::Result<Self> {
        let request = read_message_chat_request(body);
        Ok(Self {
            chat_type: request.chat_type,
            language: request.language,
            message: request.message,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct JoinChannelRequest {
    channel_name: String,
    password: String,
}

impl JoinChannelRequest {
    fn read(body: &[u8]) -> anyhow::Result<Self> {
        let request = read_join_channel_request(body);
        Ok(Self {
            channel_name: request.channel_name,
            password: request.password,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct TextEmote {
    text_emote: u32,
    emote_num: u32,
    target_guid: u64,
}

impl TextEmote {
    fn read(body: &[u8]) -> anyhow::Result<Self> {
        let request = read_text_emote_request(body);
        Ok(Self {
            text_emote: request.text_emote,
            emote_num: request.emote_num,
            target_guid: request.target_raw_guid,
        })
    }
}

#[derive(Debug, Clone, PartialEq)]
struct CastSpellPacket {
    spell_id: u32,
    targets: SpellCastTargets,
}

impl CastSpellPacket {
    fn read(body: &[u8]) -> anyhow::Result<Self> {
        let request = read_cast_spell_request(body);
        Ok(Self {
            spell_id: request.spell_id,
            targets: request.targets,
        })
    }
}

fn decode_update_values(body: &[u8]) -> Vec<Option<u32>> {
    let block_count = body[0] as usize;
    let mask_start = 1;
    let mut value_cursor = mask_start + block_count * 4;
    let mut values = vec![None; block_count * 32];

    for (index, value_slot) in values.iter_mut().enumerate() {
        let mask_offset = mask_start + (index / 32) * 4;
        let mask = u32::from_le_bytes(
            body[mask_offset..mask_offset + 4]
                .try_into()
                .expect("update mask block"),
        );
        if mask & (1 << (index % 32)) == 0 {
            continue;
        }

        let value = u32::from_le_bytes(
            body[value_cursor..value_cursor + 4]
                .try_into()
                .expect("update value"),
        );
        *value_slot = Some(value);
        value_cursor += 4;
    }

    values
}

fn update_values_encoded_len(body: &[u8]) -> usize {
    let block_count = body[0] as usize;
    let mask_start = 1;
    let mask_len = block_count * 4;
    let value_count = (0..block_count)
        .map(|block| {
            let mask_offset = mask_start + block * 4;
            u32::from_le_bytes(
                body[mask_offset..mask_offset + 4]
                    .try_into()
                    .expect("update mask block"),
            )
            .count_ones() as usize
        })
        .sum::<usize>();

    mask_start + mask_len + value_count * 4
}

fn decode_values_update_block(block: &[u8], guid: ObjectGuid) -> (Vec<Option<u32>>, &[u8]) {
    assert_eq!(block[0], UPDATE_TYPE_VALUES);
    let values_start = 1 + PackedGuid::packed_size(guid);
    let values_len = update_values_encoded_len(&block[values_start..]);
    (
        decode_update_values(&block[values_start..values_start + values_len]),
        &block[values_start + values_len..],
    )
}

fn decode_create_update_block(
    block: &[u8],
    guid: ObjectGuid,
    type_id: u8,
) -> (Vec<Option<u32>>, &[u8]) {
    assert_eq!(block[0], UPDATE_TYPE_CREATE_OBJECT);
    let type_id_offset = 1 + PackedGuid::packed_size(guid);
    assert_eq!(block[type_id_offset], type_id);
    assert_eq!(block[type_id_offset + 1], UPDATEFLAG_ALL);
    assert_eq!(
        &block[type_id_offset + 2..type_id_offset + 6],
        &1u32.to_le_bytes()
    );

    let values_start = type_id_offset + 6;
    let values_len = update_values_encoded_len(&block[values_start..]);
    (
        decode_update_values(&block[values_start..values_start + values_len]),
        &block[values_start + values_len..],
    )
}

fn decode_positioned_create_update_block(
    block: &[u8],
    guid: ObjectGuid,
    type_id: u8,
) -> (Vec<Option<u32>>, &[u8]) {
    assert_eq!(block[0], UPDATE_TYPE_CREATE_OBJECT2);
    let type_id_offset = 1 + PackedGuid::packed_size(guid);
    assert_eq!(block[type_id_offset], type_id);
    assert_eq!(
        block[type_id_offset + 1],
        UPDATEFLAG_ALL | UPDATEFLAG_HAS_POSITION
    );
    assert_eq!(
        &block[type_id_offset + 18..type_id_offset + 22],
        &1u32.to_le_bytes()
    );

    let values_start = type_id_offset + 22;
    let values_len = update_values_encoded_len(&block[values_start..]);
    (
        decode_update_values(&block[values_start..values_start + values_len]),
        &block[values_start + values_len..],
    )
}

fn test_character(race: u8, class: u8) -> CharacterEnumEntry {
    CharacterEnumEntry {
        guid: 7,
        name: "Ada".to_string(),
        race,
        class,
        gender: 0,
        player_bytes: 0x0403_0201,
        player_bytes2: 5,
        level: 1,
        xp: 0,
        rest_bonus: 0.0,
        logout_time: 0,
        is_logout_resting: 0,
        zone: 12,
        map: 0,
        position_x: -8949.95,
        position_y: -132.493,
        position_z: 83.5312,
        orientation: 0.0,
        guildid: None,
        player_flags: 0,
        at_login: 0,
        money: 12345,
        cinematic: 0,
        ammo_id: 0,
        health: 0,
        power1: 0,
        power2: 0,
        power3: 0,
        power4: 0,
        power5: 0,
        watched_faction: u32::MAX,
        explored_zones: None,
        pet_entry: None,
        pet_modelid: None,
        pet_level: None,
        equipment_cache: None,
    }
}

fn test_skill(skill: u16, value: u16, max: u16) -> CharacterSkill {
    CharacterSkill { skill, value, max }
}

fn test_item_template(
    entry: u32,
    class: u32,
    inventory_type: u32,
    dmg_min1: f32,
    dmg_max1: f32,
    armor: u32,
) -> ItemTemplateQuery {
    ItemTemplateQuery {
        entry,
        class,
        subclass: 0,
        name: format!("Item {entry}"),
        displayid: 0,
        quality: 0,
        flags: 0,
        buy_price: 0,
        sell_price: 0,
        inventory_type,
        allowable_class: -1,
        allowable_race: -1,
        item_level: 1,
        required_level: 0,
        required_skill: 0,
        required_skill_rank: 0,
        required_spell: 0,
        required_honor_rank: 0,
        required_city_rank: 0,
        required_reputation_faction: 0,
        required_reputation_rank: 0,
        max_count: 0,
        stackable: 1,
        container_slots: 0,
        stats: [wow_db::ItemTemplateStat::default(); 10],
        damage: [
            wow_db::ItemTemplateDamage {
                damage_min: dmg_min1,
                damage_max: dmg_max1,
                damage_type: 0,
            },
            wow_db::ItemTemplateDamage::default(),
            wow_db::ItemTemplateDamage::default(),
            wow_db::ItemTemplateDamage::default(),
            wow_db::ItemTemplateDamage::default(),
        ],
        dmg_min1,
        dmg_max1,
        dmg_type1: 0,
        armor,
        holy_res: 0,
        fire_res: 0,
        nature_res: 0,
        frost_res: 0,
        shadow_res: 0,
        arcane_res: 0,
        delay: 2000,
        ammo_type: 0,
        ranged_mod_range: 0.0,
        spells: [wow_db::ItemTemplateSpell::default(); 5],
        bonding: 0,
        description: String::new(),
        page_text: 0,
        language_id: 0,
        page_material: 0,
        start_quest: 0,
        lock_id: 0,
        material: 0,
        sheath: 0,
        random_property: 0,
        block: 0,
        itemset: 0,
        max_durability: 0,
        area: 0,
        map: 0,
        bag_family: 0,
    }
}

fn equipped_template(slot: u8, template: ItemTemplateQuery) -> EquippedItemTemplate {
    EquippedItemTemplate { slot, template }
}

fn test_creature_template(entry: u32) -> CreatureTemplateQuery {
    CreatureTemplateQuery {
        entry,
        name: format!("Creature {entry}"),
        subname: Some("DB Spawn".to_string()),
        min_level: 4,
        max_level: 6,
        display_id1: 123,
        display_id2: 0,
        display_id3: 0,
        display_id4: 0,
        display_id_probability1: 100,
        display_id_probability2: 0,
        display_id_probability3: 0,
        display_id_probability4: 0,
        model_gender1: 0,
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
        model_bounding_radius: DEFAULT_WORLD_OBJECT_SIZE,
        model_combat_reach: PLAYER_COMBAT_REACH_YARDS,
        faction: 35,
        scale: 1.0,
        speed_walk: 1.0,
        speed_run: 1.0,
        detection_range: 20,
        call_for_help: 0,
        pursuit: 15_000,
        leash: 0,
        family: 0,
        creature_type: 7,
        creature_type_flags: 0,
        inhabit_type: 3,
        npc_flags: UNIT_NPC_FLAG_GOSSIP,
        unit_flags: 0x20,
        dynamic_flags: 0,
        static_flags2: 0,
        unit_class: 1,
        rank: 1,
        health_multiplier: 1.0,
        power_multiplier: 1.0,
        damage_multiplier: 1.0,
        damage_variance: 1.0,
        armor_multiplier: 1.0,
        min_level_health: 80,
        max_level_health: 120,
        min_level_mana: 0,
        max_level_mana: 0,
        min_melee_dmg: 3.0,
        max_melee_dmg: 5.0,
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
        min_loot_gold: 2,
        max_loot_gold: 4,
        melee_base_attack_time: 1800,
        ranged_base_attack_time: 2200,
        damage_school: 0,
        trainer_type: 0,
        trainer_class: 0,
        pet_spell_data_id: 0,
        spell_list: 0,
        civilian: 0,
        corpse_decay: 0,
        movement_type: DB_MOTION_TYPE_IDLE,
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
    }
}

fn test_creature_spawn(entry: u32) -> CreatureSpawnQuery {
    CreatureSpawnQuery {
        guid: 44,
        entry,
        map: 0,
        game_event: None,
        addon_emote: 0,
        position_x: -8950.0,
        position_y: -130.0,
        position_z: 83.5,
        orientation: 1.25,
        spawn_time_secs_min: 120,
        spawn_time_secs_max: 120,
        spawn_dist: 0.0,
        movement_type: DB_MOTION_TYPE_IDLE,
        formation_waypoint_path_id: None,
        template: test_creature_template(entry),
        waypoint_path: Vec::new(),
    }
}

fn test_creature_spell_list_row(
    id: u32,
    position: u32,
    spell_id: u32,
    initial_millis: u32,
    repeat_millis: u32,
) -> wow_db::CreatureSpellListQuery {
    wow_db::CreatureSpellListQuery {
        id,
        chance_support_action: 0,
        chance_ranged_attack: 100,
        position,
        spell_id,
        flags: CREATURE_SPELL_LIST_FLAG_RANGED_ACTION,
        combat_condition: -1,
        target_id: CREATURE_SPELL_LIST_TARGET_CURRENT,
        script_id: 0,
        availability: 100,
        probability: 0,
        initial_min: initial_millis,
        initial_max: initial_millis,
        repeat_min: repeat_millis,
        repeat_max: repeat_millis,
        recovery_time: 0,
        category: 0,
        category_recovery_time: 0,
        target_type: CREATURE_SPELL_LIST_TARGETING_HARDCODED,
        target_param1: 0,
        target_param2: 0,
        target_param3: 0,
        target_unit_condition: -1,
    }
}

fn test_creature_ai_flee_script(
    id: i32,
    creature_id: i32,
    max_hp_percent: i32,
) -> wow_db::CreatureAiScriptQuery {
    wow_db::CreatureAiScriptQuery {
        id,
        creature_id,
        event_type: EVENT_AI_EVENT_HP,
        event_chance: 100,
        event_flags: 0,
        event_param1: max_hp_percent,
        event_param2: 0,
        event_param3: 0,
        event_param4: 0,
        event_param5: 0,
        event_param6: 0,
        action1_type: EVENT_AI_ACTION_FLEE_FOR_ASSIST,
        action1_param1: 0,
        action1_param2: 0,
        action1_param3: 0,
        action2_type: 0,
        action2_param1: 0,
        action2_param2: 0,
        action2_param3: 0,
        action3_type: 0,
        action3_param1: 0,
        action3_param2: 0,
        action3_param3: 0,
    }
}

fn test_creature_ai_set_walk_script(
    id: i32,
    creature_id: i32,
    max_hp_percent: i32,
    walk_setting: i32,
) -> wow_db::CreatureAiScriptQuery {
    wow_db::CreatureAiScriptQuery {
        id,
        creature_id,
        event_type: EVENT_AI_EVENT_HP,
        event_chance: 100,
        event_flags: 0,
        event_param1: max_hp_percent,
        event_param2: 0,
        event_param3: 0,
        event_param4: 0,
        event_param5: 0,
        event_param6: 0,
        action1_type: EVENT_AI_ACTION_SET_WALK,
        action1_param1: walk_setting,
        action1_param2: 0,
        action1_param3: 0,
        action2_type: 0,
        action2_param1: 0,
        action2_param2: 0,
        action2_param3: 0,
        action3_type: 0,
        action3_param1: 0,
        action3_param2: 0,
        action3_param3: 0,
    }
}

fn test_creature_ai_cast_script(
    id: i32,
    creature_id: i32,
    event_type: u8,
    event_params: [i32; 4],
    spell_id: u32,
    target: i32,
) -> wow_db::CreatureAiScriptQuery {
    wow_db::CreatureAiScriptQuery {
        id,
        creature_id,
        event_type,
        event_chance: 100,
        event_flags: EVENT_AI_FLAG_REPEATABLE,
        event_param1: event_params[0],
        event_param2: event_params[1],
        event_param3: event_params[2],
        event_param4: event_params[3],
        event_param5: 0,
        event_param6: 0,
        action1_type: EVENT_AI_ACTION_CAST,
        action1_param1: spell_id as i32,
        action1_param2: target,
        action1_param3: 0,
        action2_type: 0,
        action2_param1: 0,
        action2_param2: 0,
        action2_param3: 0,
        action3_type: 0,
        action3_param1: 0,
        action3_param2: 0,
        action3_param3: 0,
    }
}

fn test_unit_condition_row(
    id: i32,
    variable: u32,
    operation: u32,
    value: i32,
) -> wow_db::UnitConditionQuery {
    wow_db::UnitConditionQuery {
        id,
        flags: 0,
        variable_0: variable,
        variable_1: 0,
        variable_2: 0,
        variable_3: 0,
        variable_4: 0,
        variable_5: 0,
        variable_6: 0,
        variable_7: 0,
        op_0: operation,
        op_1: 0,
        op_2: 0,
        op_3: 0,
        op_4: 0,
        op_5: 0,
        op_6: 0,
        op_7: 0,
        value_0: value,
        value_1: 0,
        value_2: 0,
        value_3: 0,
        value_4: 0,
        value_5: 0,
        value_6: 0,
        value_7: 0,
    }
}

fn test_combat_condition_row(id: i32, self_condition_id: i32) -> wow_db::CombatConditionQuery {
    wow_db::CombatConditionQuery {
        id,
        world_state_expression_id: 0,
        self_condition_id,
        target_condition_id: 0,
        friend_condition_logic: 0,
        enemy_condition_logic: 0,
        friend_condition_id_0: 0,
        friend_condition_id_1: 0,
        friend_condition_op_0: 0,
        friend_condition_op_1: 0,
        friend_condition_count_0: 0,
        friend_condition_count_1: 0,
        enemy_condition_id_0: 0,
        enemy_condition_id_1: 0,
        enemy_condition_op_0: 0,
        enemy_condition_op_1: 0,
        enemy_condition_count_0: 0,
        enemy_condition_count_1: 0,
    }
}

fn test_gameobject_template(entry: u32, object_type: u8) -> wow_db::GameObjectTemplateQuery {
    wow_db::GameObjectTemplateQuery {
        entry,
        object_type,
        display_id: 12_345,
        name: format!("GO {entry}"),
        icon_name: "Attack".to_string(),
        faction: 0,
        flags: 0,
        size: 1.0,
        raw_data: [0; 24],
    }
}

fn test_gameobject_spawn(entry: u32, object_type: u8) -> wow_db::GameObjectSpawnQuery {
    wow_db::GameObjectSpawnQuery {
        guid: 77,
        entry,
        map: 0,
        game_event: None,
        position_x: -8948.0,
        position_y: -131.0,
        position_z: 83.4,
        orientation: 0.75,
        rotation0: 0.0,
        rotation1: 0.0,
        rotation2: 0.0,
        rotation3: 1.0,
        spawn_time_secs_min: 45,
        spawn_time_secs_max: 45,
        state: -1,
        anim_progress: 100,
        template: test_gameobject_template(entry, object_type),
    }
}

fn test_condition(
    condition_entry: u32,
    condition_type: i16,
    value1: u32,
    value2: u32,
) -> wow_db::ConditionQuery {
    wow_db::ConditionQuery {
        condition_entry,
        condition_type,
        value1,
        value2,
        value3: 0,
        value4: 0,
        flags: 0,
    }
}

fn test_waypoint(point: u32, x: f32, y: f32, wait_time: u32) -> wow_db::CreatureWaypointQuery {
    wow_db::CreatureWaypointQuery {
        point,
        position_x: x,
        position_y: y,
        position_z: 83.5,
        orientation: None,
        wait_time,
        script_id: 0,
    }
}

fn test_db_script_command(id: u32, command: u32, delay: u32) -> wow_db::DbScriptCommandQuery {
    wow_db::DbScriptCommandQuery {
        id,
        delay,
        priority: 0,
        command,
        datalong: 0,
        datalong2: 0,
        datalong3: 0,
        data_flags: 0,
        dataint: 0,
        dataint2: 0,
        dataint3: 0,
        dataint4: 0,
        condition_id: 0,
    }
}

fn test_quest_template(entry: u32) -> QuestTemplateQuery {
    QuestTemplateQuery {
        entry,
        method: 2,
        zone_or_sort: 12,
        min_level: 1,
        max_level: 255,
        quest_level: 1,
        quest_type: 0,
        required_classes: 0,
        required_races: 0,
        required_skill: 0,
        required_skill_value: 0,
        required_condition: 0,
        rep_objective_faction: 0,
        rep_objective_value: 0,
        required_min_rep_faction: 0,
        required_min_rep_value: 0,
        required_max_rep_faction: 0,
        required_max_rep_value: 0,
        special_flags: 0,
        prev_quest_id: 0,
        next_quest_id: 0,
        exclusive_group: 0,
        next_quest_in_chain: 0,
        rew_or_req_money: 0,
        rew_money_max_level: 0,
        rew_spell: 0,
        rew_spell_cast: 0,
        src_item_id: 0,
        src_item_count: 0,
        quest_flags: 0,
        title: "Test Quest".to_string(),
        details: String::new(),
        objectives: String::new(),
        offer_reward_text: String::new(),
        request_items_text: String::new(),
        end_text: String::new(),
        req_creature_or_go_id: [0; 4],
        req_creature_or_go_count: [0; 4],
        req_item_id: [0; 4],
        req_item_count: [0; 4],
        req_source_id: [0; 4],
        req_source_count: [0; 4],
        rew_choice_item_id: [0; 6],
        rew_choice_item_count: [0; 6],
        rew_item_id: [0; 4],
        rew_item_count: [0; 4],
        rew_rep_faction: [0; 5],
        rew_rep_value: [0; 5],
        point_map_id: 0,
        point_x: 0.0,
        point_y: 0.0,
        point_opt: 0,
        details_emote: [0; 4],
        details_emote_delay: [0; 4],
        complete_emote: 0,
        complete_emote_delay: 0,
        incomplete_emote: 0,
        incomplete_emote_delay: 0,
        offer_reward_emote: [0; 4],
        offer_reward_emote_delay: [0; 4],
        objective_text: [String::new(), String::new(), String::new(), String::new()],
    }
}
