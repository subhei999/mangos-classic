use super::*;

// CMaNGOS reference: src/game/Entities/DynamicObject.{h,cpp}

pub(in crate::world) const DYNAMIC_OBJECT_AREA_SPELL: u8 = 0;
pub(in crate::world) const DYNAMICOBJECT_CASTER: usize = 0x006;
pub(in crate::world) const DYNAMICOBJECT_BYTES: usize = 0x008;
pub(in crate::world) const DYNAMICOBJECT_SPELLID: usize = 0x009;
pub(in crate::world) const DYNAMICOBJECT_RADIUS: usize = 0x00A;
pub(in crate::world) const DYNAMICOBJECT_POS_X: usize = 0x00B;
pub(in crate::world) const DYNAMICOBJECT_POS_Y: usize = 0x00C;
pub(in crate::world) const DYNAMICOBJECT_POS_Z: usize = 0x00D;
pub(in crate::world) const DYNAMICOBJECT_FACING: usize = 0x00E;
pub(in crate::world) const DYNAMICOBJECT_END_FIELDS: usize = 0x00F;

#[derive(Debug, Clone)]
pub(in crate::world) struct DynamicObjectRuntime {
    pub(in crate::world) guid: ObjectGuid,
    pub(in crate::world) caster: ObjectGuid,
    pub(in crate::world) caster_character_guid: u32,
    pub(in crate::world) spell_id: u32,
    pub(in crate::world) position: WorldPosition,
    pub(in crate::world) radius: f32,
    pub(in crate::world) expires_at: Instant,
    pub(in crate::world) periodic_damage: Option<PeriodicDamageAura>,
    pub(in crate::world) channeled: bool,
    pub(in crate::world) channel_interrupt_flags: u32,
    pub(in crate::world) damage_delay_count: u8,
}

pub(in crate::world) fn build_dynamic_object_create_block(
    dynamic_object: &DynamicObjectRuntime,
) -> anyhow::Result<Vec<u8>> {
    let mut block = Vec::new();
    block.push(UPDATE_TYPE_CREATE_OBJECT2);
    PackedGuid::write(&mut block, dynamic_object.guid)?;
    block.push(TYPEID_DYNAMICOBJECT);
    block.push(UPDATEFLAG_ALL | UPDATEFLAG_HAS_POSITION);
    block.extend_from_slice(&dynamic_object.position.x.to_le_bytes());
    block.extend_from_slice(&dynamic_object.position.y.to_le_bytes());
    block.extend_from_slice(&dynamic_object.position.z.to_le_bytes());
    block.extend_from_slice(&dynamic_object.position.orientation.to_le_bytes());
    block.extend_from_slice(&1u32.to_le_bytes());

    let mut values = vec![None; DYNAMICOBJECT_END_FIELDS];
    set_update_value(&mut values, 0x000, dynamic_object.guid.raw() as u32)?;
    set_update_value(&mut values, 0x001, (dynamic_object.guid.raw() >> 32) as u32)?;
    set_update_value(&mut values, 0x002, TYPEMASK_OBJECT_DYNAMICOBJECT)?;
    set_update_value(&mut values, 0x003, dynamic_object.spell_id)?;
    set_update_value(&mut values, 0x004, 1.0f32.to_bits())?;
    set_update_value(
        &mut values,
        DYNAMICOBJECT_CASTER,
        dynamic_object.caster.raw() as u32,
    )?;
    set_update_value(
        &mut values,
        DYNAMICOBJECT_CASTER + 1,
        (dynamic_object.caster.raw() >> 32) as u32,
    )?;
    set_update_value(
        &mut values,
        DYNAMICOBJECT_BYTES,
        u32::from(DYNAMIC_OBJECT_AREA_SPELL),
    )?;
    set_update_value(&mut values, DYNAMICOBJECT_SPELLID, dynamic_object.spell_id)?;
    set_update_value(
        &mut values,
        DYNAMICOBJECT_RADIUS,
        dynamic_object.radius.to_bits(),
    )?;
    set_update_value(
        &mut values,
        DYNAMICOBJECT_POS_X,
        dynamic_object.position.x.to_bits(),
    )?;
    set_update_value(
        &mut values,
        DYNAMICOBJECT_POS_Y,
        dynamic_object.position.y.to_bits(),
    )?;
    set_update_value(
        &mut values,
        DYNAMICOBJECT_POS_Z,
        dynamic_object.position.z.to_bits(),
    )?;
    set_update_value(
        &mut values,
        DYNAMICOBJECT_FACING,
        dynamic_object.position.orientation.to_bits(),
    )?;
    write_update_values(&mut block, &values)?;
    Ok(block)
}

pub(in crate::world) fn build_player_channel_update_body(
    player: ObjectGuid,
    channel_object: Option<ObjectGuid>,
    spell_id: u32,
) -> anyhow::Result<Vec<u8>> {
    let mut block = Vec::new();
    block.push(UPDATE_TYPE_VALUES);
    PackedGuid::write(&mut block, player)?;
    let raw = channel_object.map(ObjectGuid::raw).unwrap_or(0);
    let mut values = vec![None; PLAYER_END_FIELDS];
    set_update_value(&mut values, UNIT_FIELD_CHANNEL_OBJECT, raw as u32)?;
    set_update_value(
        &mut values,
        UNIT_FIELD_CHANNEL_OBJECT + 1,
        (raw >> 32) as u32,
    )?;
    set_update_value(&mut values, UNIT_CHANNEL_SPELL, spell_id)?;
    write_update_values(&mut block, &values)?;
    Ok(build_update_object_body(&[block]))
}

pub(in crate::world) fn build_channel_start_body(
    _caster: ObjectGuid,
    spell_id: u32,
    duration_millis: u32,
) -> anyhow::Result<Vec<u8>> {
    let mut body = Vec::new();
    body.extend_from_slice(&spell_id.to_le_bytes());
    body.extend_from_slice(&duration_millis.to_le_bytes());
    Ok(body)
}

pub(in crate::world) fn build_channel_update_body(
    _caster: ObjectGuid,
    remaining_millis: u32,
) -> anyhow::Result<Vec<u8>> {
    let mut body = Vec::new();
    body.extend_from_slice(&remaining_millis.to_le_bytes());
    Ok(body)
}
