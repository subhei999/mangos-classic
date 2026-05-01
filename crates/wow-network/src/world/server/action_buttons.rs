// CMaNGOS reference: src/game/Entities/MiscHandler.cpp
// `WorldSession::HandleSetActionButtonOpcode`

const ACTION_BUTTON_TYPE_SPELL: u8 = 0x00;
const ACTION_BUTTON_TYPE_CLICK: u8 = 0x01;
const ACTION_BUTTON_TYPE_MACRO: u8 = 0x40;
const ACTION_BUTTON_TYPE_CMACRO: u8 = ACTION_BUTTON_TYPE_CLICK | ACTION_BUTTON_TYPE_MACRO;
const ACTION_BUTTON_TYPE_ITEM: u8 = 0x80;
const MAX_ACTION_BUTTON_ACTION_VALUE: u32 = 0x0100_0000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct SetActionButtonRequest {
    button: u8,
    packed_data: u32,
}

impl SetActionButtonRequest {
    fn read(body: &[u8]) -> anyhow::Result<Self> {
        if body.len() != 5 {
            anyhow::bail!(
                "CMSG_SET_ACTION_BUTTON payload must be 5 bytes, got {}",
                body.len()
            );
        }

        Ok(Self {
            button: body[0],
            packed_data: u32::from_le_bytes(body[1..5].try_into()?),
        })
    }

    fn action(self) -> u32 {
        self.packed_data & 0x00FF_FFFF
    }

    fn action_type(self) -> u8 {
        ((self.packed_data & 0xFF00_0000) >> 24) as u8
    }

    fn removes_binding(self) -> bool {
        self.packed_data == 0
    }
}

async fn handle_set_action_button(
    character_db_pool: &MySqlPool,
    body: &[u8],
    session: &WorldSessionState,
) -> anyhow::Result<()> {
    let request = SetActionButtonRequest::read(body)?;
    let Some(character) = &session.active_character else {
        warn!(
            button = request.button,
            "Ignoring action button update before character login"
        );
        return Ok(());
    };

    if request.button as usize >= MAX_ACTION_BUTTONS {
        warn!(
            guid = character.guid,
            button = request.button,
            "Ignoring out-of-range action button index"
        );
        return Ok(());
    }

    if request.removes_binding() {
        wow_db::delete_character_action(character_db_pool, character.guid, request.button).await?;
        return Ok(());
    }

    let action = request.action();
    let action_type = request.action_type();
    if action >= MAX_ACTION_BUTTON_ACTION_VALUE {
        warn!(
            guid = character.guid,
            button = request.button,
            action,
            "Ignoring action button binding with out-of-range action value"
        );
        return Ok(());
    }

    if !is_supported_action_button_type(action_type) {
        warn!(
            guid = character.guid,
            button = request.button,
            action,
            action_type,
            "Ignoring action button binding with unsupported type"
        );
        return Ok(());
    }

    if action_type == ACTION_BUTTON_TYPE_SPELL && !session.active_spells.contains(&action) {
        warn!(
            guid = character.guid,
            button = request.button,
            spell = action,
            "Ignoring action button binding for unknown active spell"
        );
        return Ok(());
    }

    wow_db::upsert_character_action(
        character_db_pool,
        character.guid,
        request.button,
        action,
        action_type,
    )
    .await?;
    Ok(())
}

fn is_supported_action_button_type(action_type: u8) -> bool {
    matches!(
        action_type,
        ACTION_BUTTON_TYPE_SPELL
            | ACTION_BUTTON_TYPE_CLICK
            | ACTION_BUTTON_TYPE_MACRO
            | ACTION_BUTTON_TYPE_CMACRO
            | ACTION_BUTTON_TYPE_ITEM
    )
}
