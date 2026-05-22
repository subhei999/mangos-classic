use super::*;
use wow_proto::{
    MsgCorpseQueryResponse, ServerWorldPacket, SmsgCorpseReclaimDelayResponse,
    SmsgForceMoveRootResponse, SmsgForceMoveUnrootResponse,
};

// CMaNGOS reference: src/game/Handlers/CharacterHandler.cpp death/corpse packet builders.

#[derive(Clone, Copy)]
pub(in crate::world) struct PlayerDeathUpdate {
    pub(in crate::world) player: ObjectGuid,
    pub(in crate::world) health: u32,
    pub(in crate::world) player_flags: u32,
    pub(in crate::world) field_bytes: u32,
    pub(in crate::world) unit_flags: u32,
    pub(in crate::world) race: u8,
    pub(in crate::world) class: u8,
    pub(in crate::world) stand_state: u8,
}

pub(in crate::world) fn build_player_death_update_body(
    update: PlayerDeathUpdate,
) -> anyhow::Result<Vec<u8>> {
    let mut block = Vec::new();

    block.push(UPDATE_TYPE_VALUES);

    PackedGuid::write(&mut block, update.player)?;

    let mut values = vec![None; PLAYER_END_FIELDS];

    set_update_value(&mut values, UNIT_FIELD_HEALTH, update.health)?;

    set_update_value(&mut values, UNIT_FIELD_POWER2, 0)?;

    set_update_value(&mut values, UNIT_FIELD_FLAGS, update.unit_flags)?;

    set_update_value(
        &mut values,
        UNIT_FIELD_BYTES_1,
        unit_bytes_1_for_class(update.class) | u32::from(update.stand_state),
    )?;

    set_update_value(&mut values, PLAYER_FLAGS_FIELD, update.player_flags)?;

    set_update_value(&mut values, PLAYER_FIELD_BYTES, update.field_bytes)?;

    set_player_ghost_aura_update_values(
        &mut values,
        update.player_flags & PLAYER_FLAGS_GHOST != 0,
        update.race,
        1,
    )?;

    write_update_values(&mut block, &values)?;

    Ok(build_update_object_body(&[block]))
}

pub(in crate::world) fn build_force_move_root_body(
    player: ObjectGuid,
    counter: u32,
) -> anyhow::Result<Vec<u8>> {
    Ok(SmsgForceMoveRootResponse { player, counter }.body())
}

pub(in crate::world) fn build_force_move_unroot_body(
    player: ObjectGuid,
    counter: u32,
) -> anyhow::Result<Vec<u8>> {
    Ok(SmsgForceMoveUnrootResponse { player, counter }.body())
}

pub(in crate::world) fn set_player_ghost_aura_update_values(
    values: &mut [Option<u32>],

    ghost: bool,

    race: u8,

    level: u8,
) -> anyhow::Result<()> {
    set_unit_aura_update_values(values, &[])?;
    let has_wisp_form = ghost && race == PLAYER_RACE_NIGHT_ELF;
    let ghost_slot = MAX_POSITIVE_AURA_SLOTS;
    set_update_value(
        values,
        UNIT_FIELD_AURA + ghost_slot,
        if has_wisp_form {
            NIGHT_ELF_WISP_FORM_SPELL_ID
        } else if ghost {
            GHOST_SPELL_ID
        } else {
            0
        },
    )?;
    if has_wisp_form {
        set_update_value(values, UNIT_FIELD_AURA + ghost_slot + 1, GHOST_SPELL_ID)?;
    }

    let flags_index = UNIT_FIELD_AURAFLAGS + (ghost_slot / 8);
    set_update_value(
        values,
        flags_index,
        if has_wisp_form {
            GHOST_AURA_FLAGS | (GHOST_AURA_FLAGS << 4)
        } else if ghost {
            GHOST_AURA_FLAGS
        } else {
            0
        },
    )?;

    let level_index = UNIT_FIELD_AURALEVELS + (ghost_slot / 4);
    set_update_value(
        values,
        level_index,
        if has_wisp_form {
            let level = level.max(1) as u32;
            level | (level << 8)
        } else if ghost {
            level.max(1) as u32
        } else {
            0
        },
    )?;

    set_update_value(values, UNIT_FIELD_AURAAPPLICATIONS, 0)
}

pub(in crate::world) fn build_corpse_reclaim_delay_body(delay_millis: u32) -> Vec<u8> {
    SmsgCorpseReclaimDelayResponse { delay_millis }.body()
}

pub(in crate::world) fn build_corpse_query_body(corpse_position: Option<WorldPosition>) -> Vec<u8> {
    MsgCorpseQueryResponse {
        corpse_position: corpse_position.map(world_location_response),
    }
    .body()
}
