use sha1::{Digest, Sha1};
use sqlx::mysql::MySqlPool;
use std::collections::HashSet;
use std::io::Cursor;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::Mutex;
use tokio::time::{timeout, Duration};
use tracing::{error, info, warn};
use wow_common::guid::{write_guid, HighGuid, ObjectGuid, PackedGuid};
use wow_common::position::WorldPosition;
use wow_crypto::HeaderCrypto;
use wow_db::{
    CharacterAction, CharacterDeleteOptions, CharacterEnumEntry, CharacterInventoryItem,
    CharacterNameQuery, CharacterReputation, CharacterSkill, CharacterSpell, CreatureSpawnQuery,
    CreatureTemplateQuery, ItemTemplateQuery, NewCharacter, PlayerWorldStats,
};

const CMSG_CHAR_CREATE: u32 = 0x0036;
const CMSG_CHAR_ENUM: u32 = 0x0037;
const CMSG_CHAR_DELETE: u32 = 0x0038;
const CMSG_PLAYER_LOGIN: u32 = 0x003D;
const SMSG_CHAR_CREATE: u16 = 0x003A;
const SMSG_CHAR_DELETE: u16 = 0x003C;
const CMSG_PLAYER_LOGOUT: u32 = 0x004A;
const CMSG_LOGOUT_REQUEST: u32 = 0x004B;
const SMSG_LOGOUT_RESPONSE: u16 = 0x004C;
const SMSG_LOGOUT_COMPLETE: u16 = 0x004D;
const CMSG_LOGOUT_CANCEL: u32 = 0x004E;
const SMSG_LOGOUT_CANCEL_ACK: u16 = 0x004F;
const CMSG_NAME_QUERY: u32 = 0x0050;
const SMSG_NAME_QUERY_RESPONSE: u16 = 0x0051;
const CMSG_ITEM_QUERY_SINGLE: u32 = 0x0056;
const SMSG_ITEM_QUERY_SINGLE_RESPONSE: u16 = 0x0058;
const CMSG_CREATURE_QUERY: u32 = 0x0060;
const SMSG_CREATURE_QUERY_RESPONSE: u16 = 0x0061;
const CMSG_MESSAGECHAT: u32 = 0x0095;
const SMSG_MESSAGECHAT: u16 = 0x0096;
const CMSG_JOIN_CHANNEL: u32 = 0x0097;
const MSG_MOVE_START_FORWARD: u32 = 0x00B5;
const MSG_MOVE_START_BACKWARD: u32 = 0x00B6;
const MSG_MOVE_STOP: u32 = 0x00B7;
const MSG_MOVE_START_STRAFE_LEFT: u32 = 0x00B8;
const MSG_MOVE_START_STRAFE_RIGHT: u32 = 0x00B9;
const MSG_MOVE_STOP_STRAFE: u32 = 0x00BA;
const MSG_MOVE_JUMP: u32 = 0x00BB;
const MSG_MOVE_START_TURN_LEFT: u32 = 0x00BC;
const MSG_MOVE_START_TURN_RIGHT: u32 = 0x00BD;
const MSG_MOVE_STOP_TURN: u32 = 0x00BE;
const MSG_MOVE_START_PITCH_UP: u32 = 0x00BF;
const MSG_MOVE_START_PITCH_DOWN: u32 = 0x00C0;
const MSG_MOVE_STOP_PITCH: u32 = 0x00C1;
const MSG_MOVE_SET_RUN_MODE: u32 = 0x00C2;
const MSG_MOVE_SET_WALK_MODE: u32 = 0x00C3;
const MSG_MOVE_FALL_LAND: u32 = 0x00C9;
const MSG_MOVE_START_SWIM: u32 = 0x00CA;
const MSG_MOVE_STOP_SWIM: u32 = 0x00CB;
const MSG_MOVE_SET_FACING: u32 = 0x00DA;
const MSG_MOVE_SET_PITCH: u32 = 0x00DB;
const MSG_MOVE_HEARTBEAT: u32 = 0x00EE;
const CMSG_MOVE_FALL_RESET: u32 = 0x02CA;
const CMSG_TUTORIAL_FLAG: u32 = 0x00FE;
const CMSG_TUTORIAL_CLEAR: u32 = 0x00FF;
const CMSG_TUTORIAL_RESET: u32 = 0x0100;
const CMSG_TEXT_EMOTE: u32 = 0x0104;
const SMSG_EMOTE: u16 = 0x0103;
const SMSG_TEXT_EMOTE: u16 = 0x0105;
const CMSG_AUTOSTORE_LOOT_ITEM: u32 = 0x0108;
const CMSG_AUTOEQUIP_ITEM: u32 = 0x010A;
const CMSG_SWAP_ITEM: u32 = 0x010C;
const CMSG_SWAP_INV_ITEM: u32 = 0x010D;
const CMSG_SPLIT_ITEM: u32 = 0x010E;
const CMSG_DESTROYITEM: u32 = 0x0111;
const SMSG_INVENTORY_CHANGE_FAILURE: u16 = 0x0112;
const SMSG_TRIGGER_CINEMATIC: u16 = 0x00FA;
const SMSG_DESTROY_OBJECT: u16 = 0x00AA;
const CMSG_CANCEL_TRADE: u32 = 0x011C;
const SMSG_INITIALIZE_FACTIONS: u16 = 0x0122;
const CMSG_CAST_SPELL: u32 = 0x012E;
const CMSG_CANCEL_CAST: u32 = 0x012F;
const SMSG_CAST_RESULT: u16 = 0x0130;
const SMSG_SPELL_GO: u16 = 0x0132;
const CMSG_SET_SELECTION: u32 = 0x013D;
const CMSG_ATTACKSWING: u32 = 0x0141;
const CMSG_ATTACKSTOP: u32 = 0x0142;
const SMSG_ATTACKSTART: u16 = 0x0143;
const SMSG_ATTACKSTOP: u16 = 0x0144;
const SMSG_ATTACKERSTATEUPDATE: u16 = 0x014A;
const CMSG_LOOT: u32 = 0x015D;
const CMSG_LOOT_MONEY: u32 = 0x015E;
const CMSG_LOOT_RELEASE: u32 = 0x015F;
const SMSG_LOOT_RESPONSE: u16 = 0x0160;
const SMSG_LOOT_RELEASE_RESPONSE: u16 = 0x0161;
const SMSG_LOOT_REMOVED: u16 = 0x0162;
const SMSG_LOOT_MONEY_NOTIFY: u16 = 0x0163;
const SMSG_LOOT_CLEAR_MONEY: u16 = 0x0165;
const CMSG_GOSSIP_HELLO: u32 = 0x017B;
const CMSG_GOSSIP_SELECT_OPTION: u32 = 0x017C;
const SMSG_GOSSIP_MESSAGE: u16 = 0x017D;
const SMSG_GOSSIP_COMPLETE: u16 = 0x017E;
const CMSG_NPC_TEXT_QUERY: u32 = 0x017F;
const SMSG_NPC_TEXT_UPDATE: u16 = 0x0180;
const CMSG_LIST_INVENTORY: u32 = 0x019E;
const SMSG_LIST_INVENTORY: u16 = 0x019F;
const CMSG_SELL_ITEM: u32 = 0x01A0;
const SMSG_SELL_ITEM: u16 = 0x01A1;
const CMSG_BUY_ITEM: u32 = 0x01A2;
const SMSG_BUY_ITEM: u16 = 0x01A4;
const SMSG_BUY_FAILED: u16 = 0x01A5;
const CMSG_QUERY_TIME: u32 = 0x01CE;
const SMSG_QUERY_TIME_RESPONSE: u16 = 0x01CF;
const CMSG_ZONEUPDATE: u32 = 0x01F4;
const CMSG_REQUEST_ACCOUNT_DATA: u32 = 0x020A;
const CMSG_UPDATE_ACCOUNT_DATA: u32 = 0x020B;
const SMSG_UPDATE_ACCOUNT_DATA: u16 = 0x020C;
const CMSG_GMTICKET_GETTICKET: u32 = 0x0211;
const SMSG_GMTICKET_GETTICKET: u16 = 0x0212;
const CMSG_SET_ACTIVE_MOVER: u32 = 0x026A;
const CMSG_CANCEL_AUTO_REPEAT_SPELL: u32 = 0x026D;
const MSG_QUERY_NEXT_MAIL_TIME: u32 = 0x0284;
const CMSG_MEETINGSTONE_INFO: u32 = 0x0296;
const CMSG_REQUEST_RAID_INFO: u32 = 0x02CD;
const CMSG_MOVE_TIME_SKIPPED: u32 = 0x02CE;
const CMSG_BATTLEFIELD_STATUS: u32 = 0x02D3;
const SMSG_CHAR_ENUM: u16 = 0x003B;
const SMSG_CHARACTER_LOGIN_FAILED: u16 = 0x0041;
const SMSG_LOGIN_SETTIMESPEED: u16 = 0x0042;
const SMSG_TUTORIAL_FLAGS: u16 = 0x00FD;
const SMSG_UPDATE_OBJECT: u16 = 0x00A9;
const SMSG_ACTION_BUTTONS: u16 = 0x0129;
const SMSG_INITIAL_SPELLS: u16 = 0x012A;
const SMSG_BINDPOINTUPDATE: u16 = 0x0155;
const SMSG_ACCOUNT_DATA_TIMES: u16 = 0x0209;
const SMSG_LOGIN_VERIFY_WORLD: u16 = 0x0236;
const SMSG_INIT_WORLD_STATES: u16 = 0x02C2;
const SMSG_AUTH_CHALLENGE: u16 = 0x01EC;
const CMSG_AUTH_SESSION: u32 = 0x01ED;
const SMSG_AUTH_RESPONSE: u16 = 0x01EE;
const CMSG_PING: u32 = 0x01DC;
const SMSG_PONG: u16 = 0x01DD;
const AUTH_OK: u8 = 0x0C;
const AUTH_FAILED: u8 = 0x0D;
const AUTH_VERSION_MISMATCH: u8 = 0x14;
const AUTH_UNKNOWN_ACCOUNT: u8 = 0x15;
const CHAT_MSG_SAY: u32 = 0x00;
const CHAT_MSG_YELL: u32 = 0x05;
const CHAT_MSG_EMOTE: u32 = 0x08;
const CHAT_TAG_NONE: u8 = 0;
const TEXTEMOTE_DANCE: u32 = 34;
const TEXTEMOTE_POINT: u32 = 72;
const TEXTEMOTE_SLEEP: u32 = 87;
const TEXTEMOTE_WAVE: u32 = 101;
const EMOTE_ONESHOT_WAVE: u32 = 3;
const EMOTE_STATE_DANCE: u32 = 10;
const EMOTE_STATE_SLEEP: u32 = 12;
const EMOTE_ONESHOT_POINT: u32 = 25;
const WARRIOR_HEROIC_STRIKE_RANK_1: u32 = 78;
const CHAR_CREATE_SUCCESS: u8 = 0x2E;
const CHAR_CREATE_FAILED: u8 = 0x30;
const CHAR_CREATE_NAME_IN_USE: u8 = 0x31;
const CHAR_CREATE_SERVER_LIMIT: u8 = 0x34;
const CHAR_DELETE_SUCCESS: u8 = 0x39;
const CHAR_DELETE_FAILED: u8 = 0x3A;
const CHAR_NAME_NO_NAME: u8 = 0x43;
const CHAR_NAME_TOO_SHORT: u8 = 0x44;
const CHAR_NAME_TOO_LONG: u8 = 0x45;
const CHAR_NAME_INVALID_CHARACTER: u8 = 0x46;
const CHAR_LOGIN_NO_CHARACTER: u8 = 0x05;

const SERVER_SEED: u32 = 0xC0DEC0DE;
const PLAYER_FLAGS_GHOST: u32 = 0x0000_0010;
const PLAYER_FLAGS_HIDE_HELM: u32 = 0x0000_0400;
const PLAYER_FLAGS_HIDE_CLOAK: u32 = 0x0000_0800;
const CHARACTER_FLAG_HIDE_HELM: u32 = 0x0000_0400;
const CHARACTER_FLAG_HIDE_CLOAK: u32 = 0x0000_0800;
const CHARACTER_FLAG_GHOST: u32 = 0x0000_2000;
const CHARACTER_FLAG_RENAME: u32 = 0x0000_4000;
const AT_LOGIN_RENAME: u32 = 0x01;
const AT_LOGIN_FIRST: u32 = 0x20;
const ENUM_EQUIPMENT_SLOTS: usize = 20;
const ACCOUNT_DATA_TYPES: usize = 8;
const MD5_DIGEST_LEN: usize = 16;
const MAX_ACTION_BUTTONS: usize = 120;
const TYPEID_ITEM: u8 = 1;
const TYPEID_CONTAINER: u8 = 2;
const TYPEID_UNIT: u8 = 3;
const TYPEID_PLAYER: u8 = 4;
const TYPEMASK_OBJECT_ITEM: u32 = 0x0003;
const TYPEMASK_OBJECT_CONTAINER: u32 = 0x0007;
const TYPEMASK_OBJECT_UNIT: u32 = 0x0009;
const TYPEMASK_OBJECT_UNIT_PLAYER: u32 = 0x0019;
const UPDATE_TYPE_VALUES: u8 = 0;
const UPDATE_TYPE_CREATE_OBJECT: u8 = 2;
const UPDATE_TYPE_CREATE_OBJECT2: u8 = 3;
const UPDATEFLAG_SELF: u8 = 0x01;
const UPDATEFLAG_ALL: u8 = 0x10;
const UPDATEFLAG_LIVING: u8 = 0x20;
const UPDATEFLAG_HAS_POSITION: u8 = 0x40;
const ITEM_END_FIELDS: usize = 0x30;
const CONTAINER_FIELD_NUM_SLOTS: usize = ITEM_END_FIELDS;
const CONTAINER_FIELD_SLOT_1: usize = ITEM_END_FIELDS + 0x02;
const CONTAINER_END_FIELDS: usize = ITEM_END_FIELDS + 0x4A;
const PLAYER_END_FIELDS: usize = 0x502;
const MOVEFLAG_JUMPING: u32 = 0x0000_2000;
const MOVEFLAG_SWIMMING: u32 = 0x0020_0000;
const MOVEFLAG_ONTRANSPORT: u32 = 0x0200_0000;
const MOVEFLAG_SPLINE_ELEVATION: u32 = 0x0400_0000;
const REALM_ID: u32 = 1;
const MAX_CHARACTERS_PER_REALM: u8 = 10;
const FORM_BATTLESTANCE: u8 = 0x11;
const EQUIPMENT_SLOT_END: u8 = 19;
const EQUIPMENT_SLOT_MAINHAND: u8 = 15;
const EQUIPMENT_SLOT_OFFHAND: u8 = 16;
const EQUIPMENT_SLOT_RANGED: u8 = 17;
const INVENTORY_SLOT_BAG_START: u8 = 19;
const INVENTORY_SLOT_BAG_END: u8 = 23;
const POWER_MANA: u8 = 0;
const POWER_RAGE: u8 = 1;
const POWER_FOCUS: u8 = 2;
const POWER_ENERGY: u8 = 3;
const POWER_HAPPINESS: u8 = 4;
const POWER_RAGE_DEFAULT: u32 = 1000;
const POWER_ENERGY_DEFAULT: u32 = 100;
const BASE_ATTACK_TIME_MS: u32 = 2000;
const MAX_SPELL_SCHOOL: usize = 7;
const MAX_STATS: usize = 5;
const ITEM_CLASS_WEAPON: u32 = 2;
const ITEM_CLASS_ARMOR: u32 = 4;
const INVTYPE_SHIELD: u32 = 14;
const REPUTATION_LIST_SLOTS: usize = 64;
const UNIT_FLAG_PLAYER_CONTROLLED: u32 = 0x0000_0008;
const UNIT_FIELD_HEALTH: usize = 0x016;
const UNIT_FIELD_POWER1: usize = 0x017;
const UNIT_FIELD_POWER2: usize = 0x018;
const UNIT_FIELD_POWER3: usize = 0x019;
const UNIT_FIELD_POWER4: usize = 0x01A;
const UNIT_FIELD_POWER5: usize = 0x01B;
const UNIT_FIELD_MAXHEALTH: usize = 0x01C;
const UNIT_FIELD_MAXPOWER1: usize = 0x01D;
const UNIT_FIELD_MAXPOWER2: usize = 0x01E;
const UNIT_FIELD_MAXPOWER3: usize = 0x01F;
const UNIT_FIELD_MAXPOWER4: usize = 0x020;
const UNIT_FIELD_MAXPOWER5: usize = 0x021;
const UNIT_FIELD_LEVEL: usize = 0x022;
const UNIT_FIELD_FACTIONTEMPLATE: usize = 0x023;
const UNIT_FIELD_BYTES_0: usize = 0x024;
const UNIT_FIELD_FLAGS: usize = 0x02E;
const UNIT_FIELD_AURASTATE: usize = 0x07D;
const UNIT_FIELD_BASEATTACKTIME: usize = 0x07E;
const UNIT_FIELD_RANGEDATTACKTIME: usize = 0x080;
const UNIT_FIELD_BOUNDINGRADIUS: usize = 0x081;
const UNIT_FIELD_COMBATREACH: usize = 0x082;
const UNIT_FIELD_DISPLAYID: usize = 0x083;
const UNIT_FIELD_NATIVEDISPLAYID: usize = 0x084;
const UNIT_FIELD_MOUNTDISPLAYID: usize = 0x085;
const UNIT_FIELD_MINDAMAGE: usize = 0x086;
const UNIT_FIELD_MAXDAMAGE: usize = 0x087;
const UNIT_FIELD_MINOFFHANDDAMAGE: usize = 0x088;
const UNIT_FIELD_MAXOFFHANDDAMAGE: usize = 0x089;
const UNIT_FIELD_BYTES_1: usize = 0x08A;
const UNIT_DYNAMIC_FLAGS: usize = 0x08F;
const UNIT_MOD_CAST_SPEED: usize = 0x091;
const UNIT_NPC_FLAGS: usize = 0x093;
const UNIT_NPC_EMOTESTATE: usize = 0x094;
const UNIT_FIELD_STAT0: usize = 0x096;
const UNIT_FIELD_RESISTANCES: usize = 0x09B;
const UNIT_FIELD_BASE_MANA: usize = 0x0A2;
const UNIT_FIELD_BASE_HEALTH: usize = 0x0A3;
const UNIT_FIELD_BYTES_2: usize = 0x0A4;
const UNIT_FIELD_ATTACK_POWER: usize = 0x0A5;
const UNIT_FIELD_ATTACK_POWER_MODS: usize = 0x0A6;
const UNIT_FIELD_ATTACK_POWER_MULTIPLIER: usize = 0x0A7;
const UNIT_FIELD_RANGED_ATTACK_POWER: usize = 0x0A8;
const UNIT_FIELD_RANGED_ATTACK_POWER_MODS: usize = 0x0A9;
const UNIT_FIELD_RANGED_ATTACK_POWER_MULTIPLIER: usize = 0x0AA;
const UNIT_FIELD_MINRANGEDDAMAGE: usize = 0x0AB;
const UNIT_FIELD_MAXRANGEDDAMAGE: usize = 0x0AC;
const UNIT_FIELD_POWER_COST_MODIFIER: usize = 0x0AD;
const UNIT_FIELD_POWER_COST_MULTIPLIER: usize = 0x0B4;
const PLAYER_FLAGS_FIELD: usize = 0x0BE;
const PLAYER_BYTES: usize = 0x0C1;
const PLAYER_BYTES_2: usize = 0x0C2;
const PLAYER_BYTES_3: usize = 0x0C3;
const PLAYER_FIELD_INV_SLOT_HEAD: usize = 0x1E6;
const PLAYER_FIELD_PACK_SLOT_1: usize = 0x214;
const PLAYER_XP: usize = 0x2CC;
const PLAYER_NEXT_LEVEL_XP: usize = 0x2CD;
const PLAYER_SKILL_INFO_1_1: usize = 0x2CE;
const PLAYER_MAX_SKILLS: usize = 128;
const PLAYER_CHARACTER_POINTS1: usize = 0x44E;
const PLAYER_CHARACTER_POINTS2: usize = 0x44F;
const PLAYER_TRACK_CREATURES: usize = 0x450;
const PLAYER_TRACK_RESOURCES: usize = 0x451;
const PLAYER_BLOCK_PERCENTAGE: usize = 0x452;
const PLAYER_DODGE_PERCENTAGE: usize = 0x453;
const PLAYER_PARRY_PERCENTAGE: usize = 0x454;
const PLAYER_CRIT_PERCENTAGE: usize = 0x455;
const PLAYER_RANGED_CRIT_PERCENTAGE: usize = 0x456;
const PLAYER_EXPLORED_ZONES_1: usize = 0x457;
const PLAYER_EXPLORED_ZONES_SIZE: usize = 64;
const PLAYER_REST_STATE_EXPERIENCE: usize = 0x497;
const PLAYER_FIELD_COINAGE: usize = 0x498;
const PLAYER_FIELD_POSSTAT0: usize = 0x499;
const PLAYER_FIELD_NEGSTAT0: usize = 0x49E;
const PLAYER_FIELD_RESISTANCEBUFFMODSPOSITIVE: usize = 0x4A3;
const PLAYER_FIELD_RESISTANCEBUFFMODSNEGATIVE: usize = 0x4AA;
const PLAYER_FIELD_MOD_DAMAGE_DONE_POS: usize = 0x4B1;
const PLAYER_FIELD_MOD_DAMAGE_DONE_NEG: usize = 0x4B8;
const PLAYER_FIELD_MOD_DAMAGE_DONE_PCT: usize = 0x4BF;
const PLAYER_FIELD_BYTES: usize = 0x4C6;
const PLAYER_AMMO_ID: usize = 0x4C7;
const PLAYER_SELF_RES_SPELL: usize = 0x4C8;
const PLAYER_FIELD_PVP_MEDALS: usize = 0x4C9;
const PLAYER_FIELD_BYTES2: usize = 0x4EC;
const PLAYER_FIELD_WATCHED_FACTION_INDEX: usize = 0x4ED;
const INVENTORY_SLOT_BAG_0: u8 = 0;
const CLIENT_INVENTORY_SLOT_BAG_0: u8 = 255;
const INVENTORY_SLOT_ITEM_START: u8 = 23;
const INVENTORY_SLOT_ITEM_END: u8 = 39;
const MAX_BAG_SIZE: u8 = 36;
const ITEM_FLAG_NO_USER_DESTROY: u32 = 0x0000_0020;
const EQUIP_ERR_CANT_DROP_SOULBOUND: u8 = 24;
const EQUIP_ERR_COULDNT_SPLIT_ITEMS: u8 = 27;
const BUY_ERR_NOT_ENOUGHT_MONEY: u8 = 2;
const SELL_ERR_CANT_SELL_ITEM: u8 = 2;
const SELL_ERR_CANT_FIND_VENDOR: u8 = 3;
const UNIT_NPC_FLAG_GOSSIP: u32 = 0x0000_0001;
const UNIT_NPC_FLAG_VENDOR: u32 = 0x0000_0004;
const UNIT_DYNFLAG_LOOTABLE: u32 = 0x0000_0001;
const HITINFO_NORMALSWING2: u32 = 0x0000_0002;
const VICTIMSTATE_NORMAL: u32 = 1;
const RUST_GUIDE_ENTRY: u32 = 900_001;
const RUST_GUIDE_COUNTER: u32 = 1;
const RUST_GUIDE_NAME: &str = "Rust Guide";
const RUST_GUIDE_SUBNAME: &str = "Checkpoint 1";
const RUST_GUIDE_DISPLAY_ID: u32 = 49;
const RUST_GUIDE_FACTION_TEMPLATE: u32 = 35;
const RUST_GUIDE_GOSSIP_TEXT_ID: u32 = 900_001;
const RUST_GUIDE_GOSSIP_OPTION: &str = "Keep going.";
const RUST_GUIDE_GOSSIP_TEXT: &str = "The Rust world stack is answering NPC gossip now.";
const DB_VENDOR_GOSSIP_TEXT_ID: u32 = 900_010;
const DB_VENDOR_GOSSIP_OPTION: &str = "Browse goods.";
const DB_VENDOR_GOSSIP_TEXT: &str =
    "The Rust world stack is answering DB-backed vendor gossip now.";
const RUST_COMBAT_DUMMY_ENTRY: u32 = 900_002;
const RUST_COMBAT_DUMMY_COUNTER: u32 = 2;
const RUST_COMBAT_DUMMY_NAME: &str = "Rust Combat Dummy";
const RUST_COMBAT_DUMMY_SUBNAME: &str = "Checkpoint 1";
const RUST_COMBAT_DUMMY_DISPLAY_ID: u32 = 51;
const RUST_COMBAT_DUMMY_FACTION_TEMPLATE: u32 = 14;
const RUST_COMBAT_DUMMY_HEALTH: u32 = 30;
const RUST_COMBAT_DUMMY_HIT_DAMAGE: u32 = 10;
const RUST_COMBAT_SWING_MILLIS: u64 = 2_000;
const CREATURE_SPAWN_RADIUS_YARDS: f32 = 120.0;
const CREATURE_SPAWN_LIMIT: u32 = 32;
const HEROIC_STRIKE_RAGE_COST: u32 = 150;
const RUST_COMBAT_DUMMY_RAGE_GAIN: u32 = HEROIC_STRIKE_RAGE_COST;
const HEROIC_STRIKE_FIXTURE_DAMAGE: u32 = 11;
const CLIENT_LOOT_CORPSE: u8 = 1;
const LOOT_SLOT_NORMAL: u8 = 0;
const RUST_COMBAT_DUMMY_LOOT_ITEM: u32 = 117;
const RUST_COMBAT_DUMMY_LOOT_ITEM_COUNT: u32 = 2;
const RUST_COMBAT_DUMMY_LOOT_ITEM_DISPLAY: u32 = 2473;
const RUST_COMBAT_DUMMY_LOOT_MONEY: u32 = 7;
const RUST_VENDOR_BAG_ITEM: u32 = 2102;
const RUST_VENDOR_BAG_DISPLAY: u32 = 1816;
const SPELL_CAST_TARGET_UNIT: u16 = 0x0002;
const SPELL_CAST_TARGET_UNIT_ENEMY: u16 = 0x0080;
const CAST_FLAG_SPELL_GO: u16 = 0x0100;

pub struct WorldServer {
    bind_addr: SocketAddr,
    login_db_pool: MySqlPool,
    character_db_pool: MySqlPool,
    world_db_pool: MySqlPool,
    runtime_state: WorldRuntimeState,
}

type OnlineCharacters = Arc<Mutex<HashSet<u32>>>;

#[derive(Clone)]
struct WorldRuntimeState {
    online_characters: OnlineCharacters,
    delete_options: CharacterDeleteOptions,
}

impl WorldServer {
    pub fn new(
        bind_addr: SocketAddr,
        login_db_pool: MySqlPool,
        character_db_pool: MySqlPool,
        world_db_pool: MySqlPool,
        delete_options: CharacterDeleteOptions,
    ) -> Self {
        Self {
            bind_addr,
            login_db_pool,
            character_db_pool,
            world_db_pool,
            runtime_state: WorldRuntimeState {
                online_characters: Arc::new(Mutex::new(HashSet::new())),
                delete_options,
            },
        }
    }

    pub async fn run(&self) -> anyhow::Result<()> {
        let listener = TcpListener::bind(self.bind_addr).await?;
        info!("World server listening on {}", self.bind_addr);

        loop {
            match listener.accept().await {
                Ok((socket, peer)) => {
                    info!(%peer, "Accepted world connection");
                    let login_pool = self.login_db_pool.clone();
                    let character_pool = self.character_db_pool.clone();
                    let world_pool = self.world_db_pool.clone();
                    let runtime_state = self.runtime_state.clone();
                    tokio::spawn(async move {
                        if let Err(e) = handle_client(
                            socket,
                            login_pool,
                            character_pool,
                            world_pool,
                            runtime_state,
                        )
                        .await
                        {
                            warn!(%peer, "World session ended with error: {}", e);
                        }
                    });
                }
                Err(e) => error!("Failed to accept world connection: {}", e),
            }
        }
    }
}

async fn handle_client(
    mut stream: TcpStream,
    login_db_pool: MySqlPool,
    character_db_pool: MySqlPool,
    world_db_pool: MySqlPool,
    runtime_state: WorldRuntimeState,
) -> anyhow::Result<()> {
    send_packet(
        &mut stream,
        SMSG_AUTH_CHALLENGE,
        &SERVER_SEED.to_le_bytes(),
        None,
    )
    .await?;

    let (opcode, payload) = read_client_packet(&mut stream, None).await?;
    if opcode != CMSG_AUTH_SESSION {
        anyhow::bail!("expected CMSG_AUTH_SESSION, got 0x{opcode:04X}");
    }

    let auth = AuthSessionPacket::read(&payload)?;
    info!(
        account = %auth.account,
        build = auth.client_build,
        client_seed = format_args!("0x{:08X}", auth.client_seed),
        addon_bytes = auth.addon_data.len(),
        "Received CMSG_AUTH_SESSION"
    );

    if !matches!(auth.client_build, 5875 | 6005 | 6141) {
        send_auth_response(&mut stream, AUTH_VERSION_MISMATCH).await?;
        anyhow::bail!("unsupported world client build {}", auth.client_build);
    }

    let account = wow_db::account::get_account_by_username(&login_db_pool, &auth.account)
        .await?
        .ok_or_else(|| anyhow::anyhow!("unknown account {}", auth.account))?;

    if account.sessionkey.trim().is_empty() {
        send_auth_response(&mut stream, AUTH_UNKNOWN_ACCOUNT).await?;
        anyhow::bail!("account {} has no auth session key", auth.account);
    }

    let session_key = hex_to_array40(&account.sessionkey)?;
    if !verify_world_digest(&auth, &session_key) {
        send_auth_response(&mut stream, AUTH_FAILED).await?;
        anyhow::bail!("world auth digest mismatch for account {}", auth.account);
    }

    info!(
        account = %auth.account,
        account_id = account.id,
        "World auth session verified"
    );

    let mut header_crypto = HeaderCrypto::new(&session_key);
    send_auth_ok(&mut stream, Some(&mut header_crypto)).await?;
    let mut session = WorldSessionState::default();

    loop {
        match timeout(
            Duration::from_millis(RUST_COMBAT_SWING_MILLIS),
            read_client_packet(&mut stream, Some(&mut header_crypto)),
        )
        .await
        {
            Ok(Ok((opcode, body))) => {
                info!(
                    opcode = format_args!("0x{opcode:04X}"),
                    bytes = body.len(),
                    "Received world packet after auth"
                );

                match opcode {
                    CMSG_CHAR_CREATE => {
                        handle_char_create(
                            &mut stream,
                            &login_db_pool,
                            &character_db_pool,
                            &world_db_pool,
                            account.id,
                            &body,
                            &mut header_crypto,
                        )
                        .await?;
                    }
                    CMSG_CHAR_ENUM => {
                        let characters =
                            wow_db::get_character_enum_entries(&character_db_pool, account.id)
                                .await?;
                        info!(
                            account = %auth.account,
                            count = characters.len(),
                            "Sending character enum"
                        );
                        send_char_enum(&mut stream, &characters, Some(&mut header_crypto)).await?;
                    }
                    CMSG_CHAR_DELETE => {
                        handle_char_delete(
                            &mut stream,
                            &login_db_pool,
                            &character_db_pool,
                            account.id,
                            &body,
                            &mut header_crypto,
                            &runtime_state,
                        )
                        .await?;
                    }
                    CMSG_PLAYER_LOGIN => {
                        handle_player_login(
                            &mut stream,
                            PlayerLoginDeps {
                                character_db_pool: &character_db_pool,
                                world_db_pool: &world_db_pool,
                                online_characters: &runtime_state.online_characters,
                            },
                            account.id,
                            &body,
                            &mut header_crypto,
                            &mut session,
                        )
                        .await?;
                    }
                    CMSG_PING => {
                        handle_ping(&mut stream, &body, Some(&mut header_crypto)).await?;
                    }
                    CMSG_NAME_QUERY => {
                        handle_name_query(
                            &mut stream,
                            &character_db_pool,
                            &body,
                            &mut header_crypto,
                        )
                        .await?;
                    }
                    CMSG_ITEM_QUERY_SINGLE => {
                        handle_item_query_single(
                            &mut stream,
                            &world_db_pool,
                            &body,
                            &mut header_crypto,
                        )
                        .await?;
                    }
                    CMSG_CREATURE_QUERY => {
                        handle_creature_query(
                            &mut stream,
                            &world_db_pool,
                            &body,
                            &mut header_crypto,
                        )
                        .await?;
                    }
                    CMSG_MESSAGECHAT => {
                        handle_message_chat(&mut stream, &body, &session, &mut header_crypto)
                            .await?;
                    }
                    CMSG_QUERY_TIME => {
                        handle_query_time(&mut stream, &mut header_crypto).await?;
                    }
                    CMSG_REQUEST_ACCOUNT_DATA => {
                        handle_request_account_data(&mut stream, &body, &mut header_crypto).await?;
                    }
                    CMSG_UPDATE_ACCOUNT_DATA => {
                        handle_update_account_data(&body);
                    }
                    CMSG_TUTORIAL_FLAG => {
                        handle_tutorial_flag(&character_db_pool, account.id, &body).await?;
                    }
                    CMSG_TUTORIAL_CLEAR => {
                        handle_tutorial_clear(&character_db_pool, account.id).await?;
                    }
                    CMSG_TUTORIAL_RESET => {
                        handle_tutorial_reset(&character_db_pool, account.id).await?;
                    }
                    CMSG_TEXT_EMOTE => {
                        handle_text_emote(&mut stream, &body, &session, &mut header_crypto).await?;
                    }
                    CMSG_CAST_SPELL => {
                        handle_cast_spell(&mut stream, &body, &mut session, &mut header_crypto)
                            .await?;
                    }
                    CMSG_AUTOEQUIP_ITEM | CMSG_SWAP_ITEM | CMSG_SWAP_INV_ITEM => {
                        handle_inventory_swap(
                            &mut stream,
                            &character_db_pool,
                            &world_db_pool,
                            opcode,
                            &body,
                            &mut session,
                            &mut header_crypto,
                        )
                        .await?;
                    }
                    CMSG_DESTROYITEM => {
                        handle_destroy_item(
                            &mut stream,
                            &character_db_pool,
                            &world_db_pool,
                            &body,
                            &mut session,
                            &mut header_crypto,
                        )
                        .await?;
                    }
                    CMSG_SPLIT_ITEM => {
                        handle_split_item(
                            &mut stream,
                            &character_db_pool,
                            &body,
                            &mut session,
                            &mut header_crypto,
                        )
                        .await?;
                    }
                    CMSG_CANCEL_CAST | CMSG_CANCEL_AUTO_REPEAT_SPELL => {
                        info!(
                            opcode = expected_noop_opcode_name(opcode),
                            "Ignoring spell cancel opcode for fixture spell slice"
                        );
                    }
                    CMSG_GOSSIP_HELLO => {
                        handle_gossip_hello(&mut stream, &world_db_pool, &body, &mut header_crypto)
                            .await?;
                    }
                    CMSG_GOSSIP_SELECT_OPTION => {
                        handle_gossip_select_option(
                            &mut stream,
                            &world_db_pool,
                            &body,
                            &mut header_crypto,
                        )
                        .await?;
                    }
                    CMSG_NPC_TEXT_QUERY => {
                        handle_npc_text_query(&mut stream, &body, &mut header_crypto).await?;
                    }
                    CMSG_LIST_INVENTORY => {
                        handle_list_inventory(
                            &mut stream,
                            &world_db_pool,
                            &body,
                            &mut header_crypto,
                        )
                        .await?;
                    }
                    CMSG_SELL_ITEM => {
                        handle_sell_item(
                            &mut stream,
                            &character_db_pool,
                            &world_db_pool,
                            &body,
                            &mut session,
                            &mut header_crypto,
                        )
                        .await?;
                    }
                    CMSG_BUY_ITEM => {
                        handle_buy_item(
                            &mut stream,
                            &character_db_pool,
                            &world_db_pool,
                            &body,
                            &mut session,
                            &mut header_crypto,
                        )
                        .await?;
                    }
                    CMSG_ATTACKSWING => {
                        handle_attack_swing(&mut stream, &body, &mut session, &mut header_crypto)
                            .await?;
                    }
                    CMSG_ATTACKSTOP => {
                        handle_attack_stop(&mut stream, &mut session, &mut header_crypto).await?;
                    }
                    CMSG_LOOT => {
                        handle_loot(&mut stream, &body, &mut session, &mut header_crypto).await?;
                    }
                    CMSG_AUTOSTORE_LOOT_ITEM => {
                        handle_autostore_loot_item(
                            &mut stream,
                            &character_db_pool,
                            &world_db_pool,
                            &body,
                            &mut session,
                            &mut header_crypto,
                        )
                        .await?;
                    }
                    CMSG_LOOT_MONEY => {
                        handle_loot_money(
                            &mut stream,
                            &character_db_pool,
                            &mut session,
                            &mut header_crypto,
                        )
                        .await?;
                    }
                    CMSG_LOOT_RELEASE => {
                        handle_loot_release(&mut stream, &body, &mut session, &mut header_crypto)
                            .await?;
                    }
                    CMSG_GMTICKET_GETTICKET => {
                        handle_gmticket_getticket(&mut stream, &mut header_crypto).await?;
                    }
                    CMSG_SET_ACTIVE_MOVER => {
                        handle_set_active_mover(&body, &session)?;
                    }
                    MSG_QUERY_NEXT_MAIL_TIME => {
                        handle_query_next_mail_time(
                            &mut stream,
                            &character_db_pool,
                            &session,
                            &mut header_crypto,
                        )
                        .await?;
                    }
                    CMSG_LOGOUT_REQUEST => {
                        handle_logout_request(
                            &mut stream,
                            &character_db_pool,
                            account.id,
                            &mut header_crypto,
                            &mut session,
                            &runtime_state.online_characters,
                        )
                        .await?;
                    }
                    CMSG_LOGOUT_CANCEL => {
                        handle_logout_cancel(&mut stream, &mut header_crypto).await?;
                    }
                    CMSG_PLAYER_LOGOUT => {
                        info!("Received client-side player logout notification");
                    }
                    _ if is_movement_opcode(opcode) => {
                        handle_movement(opcode, &body, &mut session)?;
                    }
                    _ if is_expected_noop_opcode(opcode) => {
                        info!(
                            opcode = expected_noop_opcode_name(opcode),
                            bytes = body.len(),
                            "Ignoring expected world bootstrap opcode"
                        );
                    }
                    _ => {
                        warn!(
                            opcode = format_args!("0x{opcode:04X}"),
                            "Unhandled authenticated world opcode"
                        );
                    }
                }
            }
            Ok(Err(e)) => {
                persist_active_character_position(&character_db_pool, account.id, &session).await?;
                unregister_active_character(&runtime_state.online_characters, &mut session).await;
                info!("World client disconnected or read failed: {}", e);
                return Ok(());
            }
            Err(_) => {
                handle_combat_tick(&mut stream, &mut session, &mut header_crypto).await?;
            }
        }
    }
}

#[derive(Debug, Default)]
struct WorldSessionState {
    active_character: Option<ActiveCharacter>,
    combat_dummy_health: u32,
    active_combat_target: Option<ObjectGuid>,
    combat_dummy_lootable: bool,
    combat_dummy_looting: bool,
    combat_dummy_loot_money_available: bool,
    combat_dummy_loot_item_available: bool,
    player_rage: u32,
    inventory: Vec<CharacterInventoryItem>,
}

#[derive(Debug)]
struct ActiveCharacter {
    guid: u32,
    name: String,
    position: WorldPosition,
    movement_flags: u32,
    client_time: u32,
    fall_time: u32,
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
        let name_end = body
            .iter()
            .position(|b| *b == 0)
            .ok_or_else(|| anyhow::anyhow!("CMSG_CHAR_CREATE name is not NUL-terminated"))?;
        let name = String::from_utf8(body[..name_end].to_vec())?;
        let cursor = name_end + 1;
        ensure_available(body, cursor + 9)?;

        Ok(Self {
            name,
            race: body[cursor],
            class: body[cursor + 1],
            gender: body[cursor + 2],
            skin: body[cursor + 3],
            face: body[cursor + 4],
            hair_style: body[cursor + 5],
            hair_color: body[cursor + 6],
            facial_hair: body[cursor + 7],
            outfit_id: body[cursor + 8],
        })
    }
}

fn normalize_character_name(name: &str) -> Result<String, u8> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err(CHAR_NAME_NO_NAME);
    }
    if trimmed.len() < 2 {
        return Err(CHAR_NAME_TOO_SHORT);
    }
    if trimmed.len() > 12 {
        return Err(CHAR_NAME_TOO_LONG);
    }
    if !trimmed.bytes().all(|b| b.is_ascii_alphabetic()) {
        return Err(CHAR_NAME_INVALID_CHARACTER);
    }

    let mut chars = trimmed.chars();
    let first = chars.next().expect("empty name checked above");
    let normalized = first.to_ascii_uppercase().to_string() + &chars.as_str().to_ascii_lowercase();
    Ok(normalized)
}

fn is_valid_race_class(race: u8, class: u8) -> bool {
    matches!(
        (race, class),
        (1, 1 | 2 | 4 | 5 | 8 | 9)
            | (2, 1 | 3 | 4 | 7 | 9)
            | (3, 1..=5)
            | (4, 1 | 3 | 4 | 5 | 11)
            | (5, 1 | 4 | 5 | 8 | 9)
            | (6, 1 | 3 | 7 | 11)
            | (7, 1 | 4 | 8 | 9)
            | (8, 1 | 3 | 4 | 5 | 7 | 8)
    )
}

async fn send_auth_response(stream: &mut TcpStream, response: u8) -> anyhow::Result<()> {
    send_packet(stream, SMSG_AUTH_RESPONSE, &[response], None).await
}

async fn send_auth_ok(
    stream: &mut TcpStream,
    header_crypto: Option<&mut HeaderCrypto>,
) -> anyhow::Result<()> {
    let mut body = Vec::with_capacity(11);
    body.push(AUTH_OK);
    body.extend_from_slice(&0u32.to_le_bytes()); // BillingTimeRemaining
    body.push(0); // BillingPlanFlags
    body.extend_from_slice(&0u32.to_le_bytes()); // BillingTimeRested
    body.push(0); // expansion
    send_packet(stream, SMSG_AUTH_RESPONSE, &body, header_crypto).await
}

async fn send_char_enum(
    stream: &mut TcpStream,
    characters: &[CharacterEnumEntry],
    header_crypto: Option<&mut HeaderCrypto>,
) -> anyhow::Result<()> {
    let body = build_char_enum_body(characters)?;
    send_packet(stream, SMSG_CHAR_ENUM, &body, header_crypto).await
}

async fn handle_char_delete(
    stream: &mut TcpStream,
    login_db_pool: &MySqlPool,
    character_db_pool: &MySqlPool,
    account_id: u32,
    body: &[u8],
    header_crypto: &mut HeaderCrypto,
    runtime_state: &WorldRuntimeState,
) -> anyhow::Result<()> {
    if body.len() != 8 {
        warn!("Rejected malformed CMSG_CHAR_DELETE bytes={}", body.len());
        return send_char_delete_result(stream, CHAR_DELETE_FAILED, Some(header_crypto)).await;
    }

    let raw_guid = u64::from_le_bytes(body.try_into()?);
    let guid = ObjectGuid::from_raw(raw_guid).counter();
    if runtime_state.online_characters.lock().await.contains(&guid) {
        warn!(account_id, guid, "Rejected loaded character delete");
        return send_char_delete_result(stream, CHAR_DELETE_FAILED, Some(header_crypto)).await;
    }
    if wow_db::is_guild_leader(character_db_pool, guid).await? {
        warn!(account_id, guid, "Rejected guild leader character delete");
        return send_char_delete_result(stream, CHAR_DELETE_FAILED, Some(header_crypto)).await;
    }

    let deleted = wow_db::delete_character_with_options(
        character_db_pool,
        account_id,
        guid,
        runtime_state.delete_options,
    )
    .await?;
    if deleted {
        let count = wow_db::refresh_realm_character_count(
            login_db_pool,
            character_db_pool,
            account_id,
            REALM_ID,
        )
        .await?;
        info!(account_id, guid, count, "Deleted character");
        send_char_delete_result(stream, CHAR_DELETE_SUCCESS, Some(header_crypto)).await
    } else {
        warn!(account_id, guid, "Rejected character delete");
        send_char_delete_result(stream, CHAR_DELETE_FAILED, Some(header_crypto)).await
    }
}

async fn send_char_delete_result(
    stream: &mut TcpStream,
    result: u8,
    header_crypto: Option<&mut HeaderCrypto>,
) -> anyhow::Result<()> {
    send_packet(stream, SMSG_CHAR_DELETE, &[result], header_crypto).await
}

async fn handle_char_create(
    stream: &mut TcpStream,
    login_db_pool: &MySqlPool,
    character_db_pool: &MySqlPool,
    world_db_pool: &MySqlPool,
    account_id: u32,
    body: &[u8],
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    let create = match CharCreatePacket::read(body) {
        Ok(create) => create,
        Err(e) => {
            warn!("Rejected malformed CMSG_CHAR_CREATE: {}", e);
            send_char_create_result(stream, CHAR_CREATE_FAILED, Some(header_crypto)).await?;
            return Ok(());
        }
    };

    let name = match normalize_character_name(&create.name) {
        Ok(name) => name,
        Err(code) => {
            send_char_create_result(stream, code, Some(header_crypto)).await?;
            return Ok(());
        }
    };

    if !is_valid_race_class(create.race, create.class) || create.gender > 1 {
        warn!(
            account_id,
            race = create.race,
            class = create.class,
            gender = create.gender,
            "Rejected invalid character create attributes"
        );
        send_char_create_result(stream, CHAR_CREATE_FAILED, Some(header_crypto)).await?;
        return Ok(());
    }

    if wow_db::character_name_exists(character_db_pool, &name).await? {
        send_char_create_result(stream, CHAR_CREATE_NAME_IN_USE, Some(header_crypto)).await?;
        return Ok(());
    }

    let char_count = wow_db::character_count_for_account(character_db_pool, account_id).await?;
    if char_count >= MAX_CHARACTERS_PER_REALM {
        send_char_create_result(stream, CHAR_CREATE_SERVER_LIMIT, Some(header_crypto)).await?;
        return Ok(());
    }

    let created = wow_db::create_character(
        character_db_pool,
        world_db_pool,
        NewCharacter {
            account_id,
            name,
            race: create.race,
            class: create.class,
            gender: create.gender,
            skin: create.skin,
            face: create.face,
            hair_style: create.hair_style,
            hair_color: create.hair_color,
            facial_hair: create.facial_hair,
        },
    )
    .await?;

    let new_count = wow_db::refresh_realm_character_count(
        login_db_pool,
        character_db_pool,
        account_id,
        REALM_ID,
    )
    .await?;

    info!(
        account_id,
        guid = created.guid,
        name = %created.name,
        race = created.race,
        class = created.class,
        count = new_count,
        "Created character"
    );

    send_char_create_result(stream, CHAR_CREATE_SUCCESS, Some(header_crypto)).await
}

async fn send_char_create_result(
    stream: &mut TcpStream,
    result: u8,
    header_crypto: Option<&mut HeaderCrypto>,
) -> anyhow::Result<()> {
    send_packet(stream, SMSG_CHAR_CREATE, &[result], header_crypto).await
}

async fn handle_player_login(
    stream: &mut TcpStream,
    deps: PlayerLoginDeps<'_>,
    account_id: u32,
    body: &[u8],
    header_crypto: &mut HeaderCrypto,
    session: &mut WorldSessionState,
) -> anyhow::Result<()> {
    if body.len() != 8 {
        anyhow::bail!(
            "CMSG_PLAYER_LOGIN payload must be 8 bytes, got {}",
            body.len()
        );
    }

    let guid_raw = u64::from_le_bytes(body.try_into()?);
    let guid = ObjectGuid::from_raw(guid_raw);
    let character_guid = guid.counter();
    let characters = wow_db::get_character_enum_entries(deps.character_db_pool, account_id).await?;
    let Some(character) = characters
        .iter()
        .find(|character| character.guid == character_guid)
    else {
        warn!(
            account_id,
            guid = format_args!("0x{guid_raw:016X}"),
            "Character login rejected: character not found for account"
        );
        send_packet(
            stream,
            SMSG_CHARACTER_LOGIN_FAILED,
            &[CHAR_LOGIN_NO_CHARACTER],
            Some(header_crypto),
        )
        .await?;
        return Ok(());
    };

    if deps
        .online_characters
        .lock()
        .await
        .contains(&character.guid)
    {
        warn!(
            account_id,
            guid = character.guid,
            "Character login rejected: character already loaded"
        );
        send_packet(
            stream,
            SMSG_CHARACTER_LOGIN_FAILED,
            &[CHAR_LOGIN_NO_CHARACTER],
            Some(header_crypto),
        )
        .await?;
        return Ok(());
    }

    info!(
        account_id,
        guid = character.guid,
        name = %character.name,
        map = character.map,
        "Character login selected"
    );
    unregister_active_character(deps.online_characters, session).await;
    deps.online_characters.lock().await.insert(character.guid);
    session.active_character = Some(ActiveCharacter {
        guid: character.guid,
        name: character.name.clone(),
        position: WorldPosition::new(
            character.map,
            character.position_x,
            character.position_y,
            character.position_z,
            character.orientation,
        ),
        movement_flags: 0,
        client_time: 0,
        fall_time: 0,
    });
    session.combat_dummy_health = RUST_COMBAT_DUMMY_HEALTH;
    session.combat_dummy_lootable = false;
    session.combat_dummy_looting = false;
    session.combat_dummy_loot_money_available = false;
    session.combat_dummy_loot_item_available = false;
    session.player_rage = character.power2.min(POWER_RAGE_DEFAULT);
    session.inventory =
        wow_db::get_character_inventory_items(deps.character_db_pool, character.guid).await?;
    let world_stats = wow_db::get_player_world_stats(
        deps.world_db_pool,
        character.race,
        character.class,
        character.level,
    )
    .await?;
    let tutorial_flags = wow_db::get_tutorial_flags(deps.character_db_pool, account_id).await?;
    let cinematic_sequence = if character.cinematic == 0 {
        cinematic_sequence_for_race(character.race)
    } else {
        None
    };
    if character.cinematic == 0 || character.at_login & AT_LOGIN_FIRST != 0 {
        let rows = wow_db::mark_character_first_login_seen(
            deps.character_db_pool,
            account_id,
            character.guid,
        )
        .await?;
        if rows == 0 {
            warn!(
                account_id,
                guid = character.guid,
                "No character row updated while marking first-login state seen"
            );
        }
    }

    send_enter_world_bootstrap(
        stream,
        EnterWorldBootstrap {
            character_db_pool: deps.character_db_pool,
            world_db_pool: deps.world_db_pool,
            character,
            inventory: &session.inventory,
            world_stats: &world_stats,
            tutorial_flags: &tutorial_flags,
            cinematic_sequence,
        },
        Some(header_crypto),
    )
    .await?;

    Ok(())
}

struct PlayerLoginDeps<'a> {
    character_db_pool: &'a MySqlPool,
    world_db_pool: &'a MySqlPool,
    online_characters: &'a OnlineCharacters,
}

async fn handle_logout_request(
    stream: &mut TcpStream,
    character_db_pool: &MySqlPool,
    account_id: u32,
    header_crypto: &mut HeaderCrypto,
    session: &mut WorldSessionState,
    online_characters: &OnlineCharacters,
) -> anyhow::Result<()> {
    if let Some(character) = &session.active_character {
        info!(
            guid = character.guid,
            name = %character.name,
            x = character.position.x,
            y = character.position.y,
            z = character.position.z,
            o = character.position.orientation,
            "Completing instant logout to character selection"
        );
    } else {
        info!("Completing logout request before character login");
    }

    let mut body = Vec::with_capacity(5);
    body.extend_from_slice(&0u32.to_le_bytes()); // no logout failure reason
    body.push(1); // instant logout, matching rested/GM-style response shape
    send_packet(
        stream,
        SMSG_LOGOUT_RESPONSE,
        &body,
        Some(&mut *header_crypto),
    )
    .await?;
    send_packet(stream, SMSG_LOGOUT_COMPLETE, &[], Some(header_crypto)).await?;
    persist_active_character_position(character_db_pool, account_id, session).await?;
    unregister_active_character(online_characters, session).await;
    Ok(())
}

async fn unregister_active_character(
    online_characters: &OnlineCharacters,
    session: &mut WorldSessionState,
) {
    if let Some(character) = session.active_character.take() {
        online_characters.lock().await.remove(&character.guid);
    }
}

async fn persist_active_character_position(
    character_db_pool: &MySqlPool,
    account_id: u32,
    session: &WorldSessionState,
) -> anyhow::Result<()> {
    let Some(character) = &session.active_character else {
        return Ok(());
    };

    let rows = wow_db::update_character_position(
        character_db_pool,
        account_id,
        character.guid,
        character.position,
    )
    .await?;

    if rows == 0 {
        warn!(
            account_id,
            guid = character.guid,
            "No character row updated while persisting position"
        );
    } else {
        info!(
            account_id,
            guid = character.guid,
            name = %character.name,
            x = character.position.x,
            y = character.position.y,
            z = character.position.z,
            o = character.position.orientation,
            "Persisted character position"
        );
    }

    Ok(())
}

async fn handle_logout_cancel(
    stream: &mut TcpStream,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    send_packet(stream, SMSG_LOGOUT_CANCEL_ACK, &[], Some(header_crypto)).await
}

fn handle_movement(
    opcode: u32,
    body: &[u8],
    session: &mut WorldSessionState,
) -> anyhow::Result<()> {
    let movement = MovementInfo::read(body)?;
    if let Some(character) = &mut session.active_character {
        character.position.x = movement.position.x;
        character.position.y = movement.position.y;
        character.position.z = movement.position.z;
        character.position.orientation = movement.position.orientation;
        character.movement_flags = movement.flags;
        character.client_time = movement.client_time;
        character.fall_time = movement.fall_time;
        info!(
            opcode = movement_opcode_name(opcode),
            guid = character.guid,
            name = %character.name,
            flags = format_args!("0x{:08X}", movement.flags),
            client_time = movement.client_time,
            x = movement.position.x,
            y = movement.position.y,
            z = movement.position.z,
            o = movement.position.orientation,
            "Updated in-memory character movement"
        );
    } else {
        warn!(
            opcode = movement_opcode_name(opcode),
            "Received movement packet before character login"
        );
    }
    Ok(())
}

include!("bootstrap.rs");
include!("interactions.rs");
include!("wire.rs");
#[cfg(test)]
mod tests;
