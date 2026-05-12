#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SpellPacketPhase {
    Start,
    CastResult,
    Go,
    Failure,
    Cooldown,
}

fn build_cast_result_ok_body(spell_id: u32) -> Vec<u8> {
    let mut body = Vec::with_capacity(5);
    body.extend_from_slice(&spell_id.to_le_bytes());
    body.push(0);
    body
}

fn build_cast_result_failure_body(spell_id: u32, failure: u8) -> Vec<u8> {
    let mut body = Vec::with_capacity(6);
    body.extend_from_slice(&spell_id.to_le_bytes());
    body.push(2);
    body.push(failure);
    body
}

fn build_spell_go_body(
    caster: ObjectGuid,
    spell_id: u32,
    targets: &SpellCastTargets,
) -> anyhow::Result<Vec<u8>> {
    build_spell_go_body_with_source(
        caster,
        caster,
        spell_id,
        CAST_FLAG_SPELL_GO,
        targets,
        None,
    )
}

fn build_spell_go_body_with_miss(
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

fn build_spell_go_body_with_source(
    source: ObjectGuid,
    caster: ObjectGuid,
    spell_id: u32,
    cast_flags: u16,
    targets: &SpellCastTargets,
    miss_info: Option<u8>,
) -> anyhow::Result<Vec<u8>> {
    let mut body = Vec::with_capacity(40);
    PackedGuid::write(&mut body, source)?;
    PackedGuid::write(&mut body, caster)?;
    body.extend_from_slice(&spell_id.to_le_bytes());
    body.extend_from_slice(&cast_flags.to_le_bytes());

    if let Some(miss_info) = miss_info {
        if let Some(target) = targets.unit_target.or(targets.gameobject_target) {
            body.push(0);
            body.push(1);
            body.extend_from_slice(&target.raw().to_le_bytes());
            body.push(miss_info);
        } else {
            body.push(0);
            body.push(0);
        }
    } else {
        if let Some(target) = targets.unit_target.or(targets.gameobject_target) {
            body.push(1);
            body.extend_from_slice(&target.raw().to_le_bytes());
        } else {
            body.push(0);
        }
        body.push(0);
    }
    targets.write(&mut body)?;
    Ok(body)
}

fn build_spell_start_body(
    caster: ObjectGuid,
    spell_id: u32,
    cast_time_ms: u32,
    targets: &SpellCastTargets,
) -> anyhow::Result<Vec<u8>> {
    build_spell_start_body_with_source(caster, caster, spell_id, cast_time_ms, targets)
}

fn build_spell_start_body_with_source(
    source: ObjectGuid,
    caster: ObjectGuid,
    spell_id: u32,
    cast_time_ms: u32,
    targets: &SpellCastTargets,
) -> anyhow::Result<Vec<u8>> {
    let mut body = Vec::with_capacity(44);
    PackedGuid::write(&mut body, source)?;
    PackedGuid::write(&mut body, caster)?;
    body.extend_from_slice(&spell_id.to_le_bytes());
    body.extend_from_slice(&CAST_FLAG_SPELL_START.to_le_bytes());
    body.extend_from_slice(&cast_time_ms.to_le_bytes());
    targets.write(&mut body)?;
    Ok(body)
}
