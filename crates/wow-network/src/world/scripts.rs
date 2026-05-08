// CMaNGOS reference: src/game/DBScripts/ScriptMgr.* and
// src/game/Globals/ObjectMgr.cpp DoDisplayText.

#[derive(Debug, Default)]
struct DbScriptRegistry {
    movement_scripts: HashMap<u32, Vec<wow_db::DbScriptCommandQuery>>,
    script_texts: HashMap<i32, wow_db::ScriptTextQuery>,
    broadcast_texts: HashMap<i32, wow_db::BroadcastTextQuery>,
}

impl DbScriptRegistry {
    async fn load(world_db_pool: &MySqlPool) -> anyhow::Result<Self> {
        let movement_scripts = wow_db::get_dbscripts_on_creature_movement(world_db_pool)
            .await?
            .into_iter()
            .fold(HashMap::<u32, Vec<_>>::new(), |mut scripts, command| {
                scripts.entry(command.id).or_default().push(command);
                scripts
            });
        let script_texts = wow_db::get_script_texts(world_db_pool)
            .await?
            .into_iter()
            .map(|text| (text.entry, text))
            .collect();
        let broadcast_texts = wow_db::get_broadcast_texts(world_db_pool)
            .await?
            .into_iter()
            .map(|text| (text.id, text))
            .collect();

        Ok(Self {
            movement_scripts,
            script_texts,
            broadcast_texts,
        })
    }

    fn movement_script(&self, id: u32) -> Option<&[wow_db::DbScriptCommandQuery]> {
        self.movement_scripts.get(&id).map(Vec::as_slice)
    }

    fn display_text(&self, entry: i32) -> Option<DbScriptDisplayText<'_>> {
        if let Some(text) = self.broadcast_texts.get(&entry) {
            let content = text
                .text
                .as_deref()
                .filter(|value| !value.is_empty())
                .or_else(|| text.text1.as_deref().filter(|value| !value.is_empty()))?;
            return Some(DbScriptDisplayText {
                content,
                chat_type: text.chat_type,
                language: text.language,
                emote: text.emote,
            });
        }

        let text = self.script_texts.get(&entry)?;
        Some(DbScriptDisplayText {
            content: &text.content_default,
            chat_type: text.chat_type as u32,
            language: text.language as u32,
            emote: text.emote,
        })
    }
}

#[derive(Debug, Clone, Copy)]
struct DbScriptDisplayText<'a> {
    content: &'a str,
    chat_type: u32,
    language: u32,
    emote: u32,
}

#[derive(Debug, Clone)]
struct PendingDbScriptAction {
    due_at: Instant,
    source: ObjectGuid,
    command: wow_db::DbScriptCommandQuery,
}

const SCRIPT_COMMAND_TALK: u32 = 0;
const SCRIPT_COMMAND_EMOTE: u32 = 1;
const SCRIPT_COMMAND_MORPH_TO_ENTRY_OR_MODEL: u32 = 23;
const SCRIPT_FLAG_COMMAND_ADDITIONAL: u32 = 0x008;
const CHAT_TYPE_SAY: u32 = 0;
const CHAT_TYPE_YELL: u32 = 1;
const CHAT_TYPE_TEXT_EMOTE: u32 = 2;
const CHAT_TYPE_BOSS_EMOTE: u32 = 3;
const CHAT_TYPE_ZONE_YELL: u32 = 6;
const CHAT_TYPE_ZONE_EMOTE: u32 = 7;
const CHAT_MSG_MONSTER_SAY: u32 = 0x0B;
const CHAT_MSG_MONSTER_YELL: u32 = 0x0C;
const CHAT_MSG_MONSTER_EMOTE: u32 = 0x0D;
const CHAT_MSG_RAID_BOSS_EMOTE: u32 = 0x5A;

fn db_script_chat_opcode_and_radius(chat_type: u32) -> Option<(u32, f32)> {
    match chat_type {
        CHAT_TYPE_SAY => Some((CHAT_MSG_MONSTER_SAY, CHAT_SAY_RADIUS_YARDS)),
        CHAT_TYPE_YELL => Some((CHAT_MSG_MONSTER_YELL, CHAT_YELL_RADIUS_YARDS)),
        CHAT_TYPE_TEXT_EMOTE => Some((CHAT_MSG_MONSTER_EMOTE, CHAT_EMOTE_RADIUS_YARDS)),
        CHAT_TYPE_BOSS_EMOTE => Some((CHAT_MSG_RAID_BOSS_EMOTE, CHAT_YELL_RADIUS_YARDS)),
        CHAT_TYPE_ZONE_YELL => Some((CHAT_MSG_MONSTER_YELL, f32::INFINITY)),
        CHAT_TYPE_ZONE_EMOTE => Some((CHAT_MSG_MONSTER_EMOTE, f32::INFINITY)),
        _ => None,
    }
}

fn db_script_random_nonzero_i32(values: [i32; 4]) -> Option<i32> {
    let values = values
        .into_iter()
        .filter(|value| *value != 0)
        .collect::<Vec<_>>();
    if values.is_empty() {
        None
    } else {
        Some(values[rand::thread_rng().gen_range(0..values.len())])
    }
}

fn db_script_random_nonzero_u32(values: [u32; 5]) -> Option<u32> {
    let values = values
        .into_iter()
        .filter(|value| *value != 0)
        .collect::<Vec<_>>();
    if values.is_empty() {
        None
    } else {
        Some(values[rand::thread_rng().gen_range(0..values.len())])
    }
}

fn build_monster_message_chat_body(
    chat_msg: u32,
    language: u32,
    sender: ObjectGuid,
    sender_name: &str,
    message: &str,
) -> Vec<u8> {
    let mut body = Vec::with_capacity(1 + 4 + 8 + sender_name.len() + message.len() + 24);
    body.push(chat_msg as u8);
    body.extend_from_slice(&language.to_le_bytes());
    match chat_msg {
        CHAT_MSG_MONSTER_EMOTE | CHAT_MSG_RAID_BOSS_EMOTE => {
            body.extend_from_slice(&((sender_name.len() + 1) as u32).to_le_bytes());
            write_c_string(&mut body, sender_name);
            body.extend_from_slice(&0u64.to_le_bytes());
        }
        CHAT_MSG_MONSTER_SAY | CHAT_MSG_MONSTER_YELL => {
            body.extend_from_slice(&sender.raw().to_le_bytes());
            body.extend_from_slice(&((sender_name.len() + 1) as u32).to_le_bytes());
            write_c_string(&mut body, sender_name);
            body.extend_from_slice(&0u64.to_le_bytes());
        }
        _ => {
            body.extend_from_slice(&sender.raw().to_le_bytes());
        }
    }
    body.extend_from_slice(&((message.len() + 1) as u32).to_le_bytes());
    write_c_string(&mut body, message);
    body.push(CHAT_TAG_NONE);
    body
}
