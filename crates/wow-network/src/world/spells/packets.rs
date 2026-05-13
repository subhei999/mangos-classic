use super::*;
use wow_proto::{
    ServerWorldPacket, SmsgCastResultResponse, SmsgSpellGoResponse, SmsgSpellStartResponse,
};

#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::world) enum SpellPacketPhase {
    Start,
    CastResult,
    Go,
    Failure,
    Cooldown,
}

pub(in crate::world) fn build_cast_result_ok_body(spell_id: u32) -> Vec<u8> {
    SmsgCastResultResponse {
        spell_id,
        status: 0,
        failure: None,
    }
    .body()
}

pub(in crate::world) fn build_cast_result_failure_body(spell_id: u32, failure: u8) -> Vec<u8> {
    SmsgCastResultResponse {
        spell_id,
        status: 2,
        failure: Some(failure),
    }
    .body()
}

pub(in crate::world) fn build_spell_go_body(
    caster: ObjectGuid,
    spell_id: u32,
    targets: &SpellCastTargets,
) -> anyhow::Result<Vec<u8>> {
    build_spell_go_body_with_source(caster, caster, spell_id, CAST_FLAG_SPELL_GO, targets, None)
}

pub(in crate::world) fn build_spell_go_body_with_miss(
    caster: ObjectGuid,
    spell_id: u32,
    targets: &SpellCastTargets,
    miss_info: u8,
) -> anyhow::Result<Vec<u8>> {
    build_spell_go_body_with_source(
        caster,
        caster,
        spell_id,
        CAST_FLAG_SPELL_GO,
        targets,
        Some(miss_info),
    )
}

pub(in crate::world) fn build_spell_go_body_with_source(
    source: ObjectGuid,
    caster: ObjectGuid,
    spell_id: u32,
    cast_flags: u16,
    targets: &SpellCastTargets,
    miss_info: Option<u8>,
) -> anyhow::Result<Vec<u8>> {
    Ok(SmsgSpellGoResponse {
        source,
        caster,
        spell_id,
        cast_flags,
        targets: *targets,
        miss_info,
    }
    .body())
}

pub(in crate::world) fn build_spell_start_body(
    caster: ObjectGuid,
    spell_id: u32,
    cast_time_ms: u32,
    targets: &SpellCastTargets,
) -> anyhow::Result<Vec<u8>> {
    build_spell_start_body_with_source(caster, caster, spell_id, cast_time_ms, targets)
}

pub(in crate::world) fn build_spell_start_body_with_source(
    source: ObjectGuid,
    caster: ObjectGuid,
    spell_id: u32,
    cast_time_ms: u32,
    targets: &SpellCastTargets,
) -> anyhow::Result<Vec<u8>> {
    Ok(SmsgSpellStartResponse {
        source,
        caster,
        spell_id,
        cast_flags: CAST_FLAG_SPELL_START,
        cast_time_ms,
        targets: *targets,
    }
    .body())
}
