use super::*;

#[derive(Debug, Clone)]
pub(in crate::world) struct GameObjectRuntime {
    pub(in crate::world) spawn: wow_db::GameObjectSpawnQuery,
    pub(in crate::world) client_visible: bool,
    pub(in crate::world) consumed_until: Option<Instant>,
}

impl GameObjectRuntime {
    pub(in crate::world) fn new(spawn: wow_db::GameObjectSpawnQuery) -> Self {
        Self {
            spawn,
            client_visible: true,
            consumed_until: None,
        }
    }

    pub(in crate::world) fn guid(&self) -> ObjectGuid {
        gameobject_spawn_guid(&self.spawn)
    }

    pub(in crate::world) fn position(&self) -> WorldPosition {
        gameobject_spawn_position(&self.spawn)
    }

    pub(in crate::world) fn is_consumed(&self, now: Instant) -> bool {
        self.consumed_until.is_some_and(|until| now < until)
    }

    pub(in crate::world) fn mark_consumed(&mut self, now: Instant) {
        let delay = gameobject_respawn_delay(&self.spawn);
        self.client_visible = false;
        self.consumed_until = Some(now + delay);
    }
}

pub(in crate::world) fn build_db_gameobject_create_blocks(
    gameobjects: &[DbGameObjectRuntime],
    quest_statuses: &HashMap<u32, CharacterQuestStatus>,
) -> anyhow::Result<Vec<Vec<u8>>> {
    gameobjects
        .iter()
        .map(|gameobject| {
            build_db_gameobject_runtime_create_block_for_quest_statuses(gameobject, quest_statuses)
        })
        .collect()
}

#[cfg(test)]
pub(in crate::world) fn build_db_gameobject_runtime_create_block(
    gameobject: &DbGameObjectRuntime,
) -> anyhow::Result<Vec<u8>> {
    build_db_gameobject_runtime_create_block_with_dynamic_flags(gameobject, 0)
}

pub(in crate::world) fn build_db_gameobject_runtime_create_block_for_quest_statuses(
    gameobject: &DbGameObjectRuntime,
    quest_statuses: &HashMap<u32, CharacterQuestStatus>,
) -> anyhow::Result<Vec<u8>> {
    build_db_gameobject_runtime_create_block_with_dynamic_flags(
        gameobject,
        gameobject_dynamic_flags_for_quest_statuses(gameobject, quest_statuses),
    )
}

pub(in crate::world) fn build_db_gameobject_dynamic_flags_update_block(
    gameobject: &DbGameObjectRuntime,
    quest_statuses: &HashMap<u32, CharacterQuestStatus>,
) -> anyhow::Result<Vec<u8>> {
    let guid = gameobject.guid();
    let mut block = Vec::new();
    block.push(UPDATE_TYPE_VALUES);
    PackedGuid::write(&mut block, guid)?;

    let mut values = vec![None; GAMEOBJECT_END_FIELDS];
    set_update_value(
        &mut values,
        GAMEOBJECT_DYN_FLAGS,
        gameobject_dynamic_flags_for_quest_statuses(gameobject, quest_statuses),
    )?;
    write_update_values(&mut block, &values)?;
    Ok(block)
}

pub(in crate::world) fn build_db_gameobject_runtime_create_block_with_dynamic_flags(
    gameobject: &DbGameObjectRuntime,
    dynamic_flags: u32,
) -> anyhow::Result<Vec<u8>> {
    let guid = gameobject.guid();
    let mut block = Vec::new();
    block.push(UPDATE_TYPE_CREATE_OBJECT2);
    PackedGuid::write(&mut block, guid)?;
    block.push(TYPEID_GAMEOBJECT);

    block.push(UPDATEFLAG_ALL | UPDATEFLAG_HAS_POSITION);
    block.extend_from_slice(&gameobject.spawn.position_x.to_le_bytes());
    block.extend_from_slice(&gameobject.spawn.position_y.to_le_bytes());
    block.extend_from_slice(&gameobject.spawn.position_z.to_le_bytes());
    block.extend_from_slice(&gameobject.spawn.orientation.to_le_bytes());
    block.extend_from_slice(&1u32.to_le_bytes());

    let mut values = vec![None; GAMEOBJECT_END_FIELDS];
    set_update_value(&mut values, 0x000, guid.raw() as u32)?;
    set_update_value(&mut values, 0x001, (guid.raw() >> 32) as u32)?;
    set_update_value(&mut values, 0x002, TYPEMASK_OBJECT_GAMEOBJECT)?;
    set_update_value(&mut values, 0x003, gameobject.spawn.entry)?;
    set_update_value(
        &mut values,
        0x004,
        if gameobject.spawn.template.size > 0.0 {
            gameobject.spawn.template.size.to_bits()
        } else {
            1.0f32.to_bits()
        },
    )?;
    set_object_guid_update_values(&mut values, GAMEOBJECT_FIELD_CREATED_BY, None)?;
    set_update_value(
        &mut values,
        GAMEOBJECT_DISPLAYID,
        gameobject.spawn.template.display_id,
    )?;
    set_update_value(
        &mut values,
        GAMEOBJECT_FLAGS,
        gameobject.spawn.template.flags & !GO_FLAG_IN_USE,
    )?;
    set_update_value(
        &mut values,
        GAMEOBJECT_ROTATION,
        gameobject.spawn.rotation0.to_bits(),
    )?;
    set_update_value(
        &mut values,
        GAMEOBJECT_ROTATION + 1,
        gameobject.spawn.rotation1.to_bits(),
    )?;
    set_update_value(
        &mut values,
        GAMEOBJECT_ROTATION + 2,
        gameobject.spawn.rotation2.to_bits(),
    )?;
    set_update_value(
        &mut values,
        GAMEOBJECT_ROTATION + 3,
        gameobject.spawn.rotation3.to_bits(),
    )?;
    set_update_value(
        &mut values,
        GAMEOBJECT_STATE,
        if gameobject.spawn.state >= 0 {
            gameobject.spawn.state as u32
        } else {
            1
        },
    )?;
    set_update_value(
        &mut values,
        GAMEOBJECT_POS_X,
        gameobject.spawn.position_x.to_bits(),
    )?;
    set_update_value(
        &mut values,
        GAMEOBJECT_POS_Y,
        gameobject.spawn.position_y.to_bits(),
    )?;
    set_update_value(
        &mut values,
        GAMEOBJECT_POS_Z,
        gameobject.spawn.position_z.to_bits(),
    )?;
    set_update_value(
        &mut values,
        GAMEOBJECT_FACING,
        gameobject.spawn.orientation.to_bits(),
    )?;
    set_update_value(&mut values, GAMEOBJECT_DYN_FLAGS, dynamic_flags)?;
    set_update_value(
        &mut values,
        GAMEOBJECT_FACTION,
        gameobject.spawn.template.faction,
    )?;
    set_update_value(
        &mut values,
        GAMEOBJECT_TYPE_ID,
        gameobject.spawn.template.object_type as u32,
    )?;
    set_update_value(&mut values, GAMEOBJECT_LEVEL, 0)?;
    set_update_value(&mut values, GAMEOBJECT_ARTKIT, 0)?;
    set_update_value(
        &mut values,
        GAMEOBJECT_ANIMPROGRESS,
        gameobject.spawn.anim_progress as u32,
    )?;
    write_update_values(&mut block, &values)?;
    Ok(block)
}

pub(in crate::world) fn gameobject_dynamic_flags_for_quest_statuses(
    gameobject: &DbGameObjectRuntime,
    quest_statuses: &HashMap<u32, CharacterQuestStatus>,
) -> u32 {
    if !gameobject_activates_for_quest_statuses(gameobject, quest_statuses) {
        return 0;
    }

    match gameobject.spawn.template.object_type {
        GO_TYPE_CHEST | GO_TYPE_QUESTGIVER => GO_DYNFLAG_LO_ACTIVATE | GO_DYNFLAG_LO_SPARKLE,
        GO_TYPE_GENERIC | GO_TYPE_SPELL_FOCUS | GO_TYPE_GOOBER => GO_DYNFLAG_LO_ACTIVATE,
        _ => 0,
    }
}

pub(in crate::world) fn gameobject_activates_for_quest_statuses(
    gameobject: &DbGameObjectRuntime,
    quest_statuses: &HashMap<u32, CharacterQuestStatus>,
) -> bool {
    let template = &gameobject.spawn.template;
    if template.flags & GO_FLAG_INTERACT_COND == 0 {
        return matches!(
            template.object_type,
            GO_TYPE_CHEST
                | GO_TYPE_QUESTGIVER
                | GO_TYPE_GENERIC
                | GO_TYPE_SPELL_FOCUS
                | GO_TYPE_GOOBER
        );
    }

    if let Some(required_quest) = gameobject_required_active_quest(template) {
        return quest_statuses.get(&required_quest).is_some_and(|status| {
            status.status == QUEST_STATUS_INCOMPLETE && status.rewarded == 0
        });
    }

    template.object_type == GO_TYPE_CHEST && gameobject_chest_has_loot_id(template)
}

pub(in crate::world) fn gameobject_spawn_guid(
    gameobject: &wow_db::GameObjectSpawnQuery,
) -> ObjectGuid {
    ObjectGuid::new(HighGuid::GameObject, gameobject.entry, gameobject.guid)
}

pub(in crate::world) fn gameobject_spawn_position(
    gameobject: &wow_db::GameObjectSpawnQuery,
) -> WorldPosition {
    WorldPosition::new(
        gameobject.map,
        gameobject.position_x,
        gameobject.position_y,
        gameobject.position_z,
        gameobject.orientation,
    )
}

pub(in crate::world) fn gameobject_respawn_delay(spawn: &wow_db::GameObjectSpawnQuery) -> Duration {
    let min = spawn.spawn_time_secs_min.max(0) as u64;
    let max = spawn
        .spawn_time_secs_max
        .max(spawn.spawn_time_secs_min)
        .max(0) as u64;
    if max <= min {
        Duration::from_secs(min)
    } else {
        Duration::from_secs(rand::thread_rng().gen_range(min..=max))
    }
}
