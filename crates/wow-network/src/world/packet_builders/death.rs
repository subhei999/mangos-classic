use super::*;
use wow_proto::{
    MsgCorpseQueryResponse, ServerWorldPacket, SmsgCorpseReclaimDelayResponse,
    SmsgForceMoveRootResponse, SmsgForceMoveUnrootResponse,
};

// CMaNGOS reference: src/game/Handlers/CharacterHandler.cpp death/corpse packet builders.

pub(in crate::world) fn build_player_death_update_body(
    player: ObjectGuid,

    health: u32,

    player_flags: u32,

    field_bytes: u32,

    unit_flags: u32,

    class: u8,

    stand_state: u8,
) -> anyhow::Result<Vec<u8>> {
    let mut block = Vec::new();

    block.push(UPDATE_TYPE_VALUES);

    PackedGuid::write(&mut block, player)?;

    let mut values = vec![None; PLAYER_END_FIELDS];

    set_update_value(&mut values, UNIT_FIELD_HEALTH, health)?;

    set_update_value(&mut values, UNIT_FIELD_FLAGS, unit_flags)?;

    set_update_value(
        &mut values,
        UNIT_FIELD_BYTES_1,
        unit_bytes_1_for_class(class) | u32::from(stand_state),
    )?;

    set_update_value(&mut values, PLAYER_FLAGS_FIELD, player_flags)?;

    set_update_value(&mut values, PLAYER_FIELD_BYTES, field_bytes)?;

    set_player_ghost_aura_update_values(&mut values, player_flags & PLAYER_FLAGS_GHOST != 0, 1)?;

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

    level: u8,
) -> anyhow::Result<()> {
    set_unit_aura_update_values(values, &[])?;
    set_update_value(
        values,
        UNIT_FIELD_AURA,
        if ghost { GHOST_SPELL_ID } else { 0 },
    )?;

    set_update_value(
        values,
        UNIT_FIELD_AURAFLAGS,
        if ghost { GHOST_AURA_FLAGS } else { 0 },
    )?;

    set_update_value(
        values,
        UNIT_FIELD_AURALEVELS,
        if ghost { level.max(1) as u32 } else { 0 },
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
