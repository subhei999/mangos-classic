// Harness-only legacy Checkpoint 1 fixture NPC support gated by
// WORLD_ENABLE_LEGACY_FIXTURE_NPCS. Not production world data.

fn legacy_fixture_npcs_enabled() -> bool {
    std::env::var("WORLD_ENABLE_LEGACY_FIXTURE_NPCS")
        .ok()
        .is_some_and(|value| {
            matches!(
                value.trim().to_ascii_lowercase().as_str(),
                "1" | "true" | "yes" | "on"
            )
        })
}

fn build_rust_guide_create_block(character: &CharacterEnumEntry) -> anyhow::Result<Vec<u8>> {
    let guid = rust_guide_guid();
    let mut block = Vec::new();
    block.push(UPDATE_TYPE_CREATE_OBJECT2);
    PackedGuid::write(&mut block, guid)?;
    block.push(TYPEID_UNIT);

    block.push(UPDATEFLAG_ALL | UPDATEFLAG_LIVING | UPDATEFLAG_HAS_POSITION);
    block.extend_from_slice(&0u32.to_le_bytes()); // movement flags
    block.extend_from_slice(&0u32.to_le_bytes()); // server time placeholder
    block.extend_from_slice(&(character.position_x + 4.0).to_le_bytes());
    block.extend_from_slice(&(character.position_y + 2.0).to_le_bytes());
    block.extend_from_slice(&character.position_z.to_le_bytes());
    block.extend_from_slice(&character.orientation.to_le_bytes());
    block.extend_from_slice(&0u32.to_le_bytes()); // fall time
    block.extend_from_slice(&2.5f32.to_le_bytes()); // walk
    block.extend_from_slice(&7.0f32.to_le_bytes()); // run
    block.extend_from_slice(&4.5f32.to_le_bytes()); // run back
    block.extend_from_slice(&4.722222f32.to_le_bytes()); // swim
    block.extend_from_slice(&2.5f32.to_le_bytes()); // swim back
    block.extend_from_slice(&std::f32::consts::PI.to_le_bytes()); // turn rate
    block.extend_from_slice(&1u32.to_le_bytes()); // UPDATEFLAG_ALL payload

    write_rust_guide_update_values(&mut block, guid)?;
    Ok(block)
}

fn write_rust_guide_update_values(body: &mut Vec<u8>, guid: ObjectGuid) -> anyhow::Result<()> {
    let mut values = vec![None; PLAYER_END_FIELDS];
    set_update_value(&mut values, 0x000, guid.raw() as u32)?;
    set_update_value(&mut values, 0x001, (guid.raw() >> 32) as u32)?;
    set_update_value(&mut values, 0x002, TYPEMASK_OBJECT_UNIT)?;
    set_update_value(&mut values, 0x003, RUST_GUIDE_ENTRY)?;
    set_update_value(&mut values, 0x004, 1.0f32.to_bits())?;
    set_update_value(&mut values, UNIT_FIELD_HEALTH, 42)?;
    set_update_value(&mut values, UNIT_FIELD_MAXHEALTH, 42)?;
    set_update_value(&mut values, UNIT_FIELD_LEVEL, 1)?;
    set_update_value(
        &mut values,
        UNIT_FIELD_FACTIONTEMPLATE,
        RUST_GUIDE_FACTION_TEMPLATE,
    )?;
    set_update_value(&mut values, UNIT_FIELD_BYTES_0, 0)?;
    set_update_value(&mut values, UNIT_FIELD_BASEATTACKTIME, 2000)?;
    set_update_value(&mut values, UNIT_FIELD_BASEATTACKTIME + 1, 2000)?;
    set_update_value(&mut values, UNIT_FIELD_RANGEDATTACKTIME, 2000)?;
    set_update_value(
        &mut values,
        UNIT_FIELD_BOUNDINGRADIUS,
        DEFAULT_WORLD_OBJECT_SIZE.to_bits(),
    )?;
    set_update_value(
        &mut values,
        UNIT_FIELD_COMBATREACH,
        PLAYER_COMBAT_REACH_YARDS.to_bits(),
    )?;
    set_update_value(&mut values, UNIT_FIELD_DISPLAYID, RUST_GUIDE_DISPLAY_ID)?;
    set_update_value(
        &mut values,
        UNIT_FIELD_NATIVEDISPLAYID,
        RUST_GUIDE_DISPLAY_ID,
    )?;
    set_update_value(&mut values, UNIT_FIELD_MINDAMAGE, 0.0f32.to_bits())?;
    set_update_value(&mut values, UNIT_FIELD_MAXDAMAGE, 0.0f32.to_bits())?;
    set_update_value(&mut values, UNIT_FIELD_BYTES_1, 0)?;
    set_update_value(&mut values, UNIT_MOD_CAST_SPEED, 1.0f32.to_bits())?;
    set_update_value(
        &mut values,
        UNIT_NPC_FLAGS,
        UNIT_NPC_FLAG_GOSSIP | UNIT_NPC_FLAG_VENDOR,
    )?;
    write_update_values(body, &values)
}

fn rust_guide_guid() -> ObjectGuid {
    ObjectGuid::new(HighGuid::Unit, RUST_GUIDE_ENTRY, RUST_GUIDE_COUNTER)
}

