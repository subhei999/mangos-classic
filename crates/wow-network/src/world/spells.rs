use super::*;
use wow_proto::world::WorldOpcode;
use wow_proto::{ServerWorldPacket, SpellCastTargets};

pub(in crate::world) fn spell_aura_state_mask(required_state: u32) -> u32 {
    required_state
        .checked_sub(1)
        .filter(|bit| *bit < 32)
        .map(|bit| 1u32 << bit)
        .unwrap_or(0)
}

pub(in crate::world) fn has_required_spell_aura_state(
    active_aura_state: u32,
    required_state: u32,
) -> bool {
    let required_mask = spell_aura_state_mask(required_state);
    required_mask != 0 && active_aura_state & required_mask != 0
}

pub(in crate::world) async fn spell_melee_cast_failure(
    shared_world: SharedWorldDeps<'_>,
    session: &mut WorldSessionState,
    spell_profile: &SpellCastProfile,
    targets: &SpellCastTargets,
) -> Option<u8> {
    if !spell_profile.requires_melee {
        return None;
    }
    if spell_profile.kind == SpellCastKind::NextMeleeSwing {
        return None;
    }
    let target = targets.unit_target?;
    match db_creature_player_melee_check_from_map(shared_world, session, target).await {
        PlayerMeleeCheck::Clear => {
            if spell_profile.requires_behind
                && !spell_target_is_behind_victim(shared_world, session, target).await
            {
                Some(SPELL_FAILED_NOT_BEHIND)
            } else {
                None
            }
        }
        PlayerMeleeCheck::BadFacing => Some(SPELL_FAILED_UNIT_NOT_INFRONT),
        PlayerMeleeCheck::NavigationBlocked(DbCreatureNavigationResult::LineOfSightBlocked) => {
            Some(SPELL_FAILED_LINE_OF_SIGHT)
        }
        _ => Some(SPELL_FAILED_OUT_OF_RANGE),
    }
}

pub(in crate::world) async fn spell_target_is_behind_victim(
    shared_world: SharedWorldDeps<'_>,
    session: &WorldSessionState,
    target: ObjectGuid,
) -> bool {
    let Some(character) = session.character.active_character.as_ref() else {
        return false;
    };
    let Some(player) = shared_world
        .maps
        .player_runtime_snapshot(character.position.map_id, character.guid)
        .await
    else {
        return false;
    };
    let Some(creature) = shared_world
        .maps
        .db_creature_snapshot(character.position.map_id, target)
        .await
    else {
        return false;
    };
    !has_in_arc(
        creature.current_position,
        player.position,
        std::f32::consts::PI,
    )
}

pub(in crate::world) async fn spell_charge_cast_failure(
    shared_world: SharedWorldDeps<'_>,
    session: &mut WorldSessionState,
    targets: &SpellCastTargets,
) -> Option<u8> {
    let target = targets.unit_target?;
    let Some(character) = session.character.active_character.as_ref() else {
        return Some(SPELL_FAILED_OUT_OF_RANGE);
    };
    let validation = shared_world
        .maps
        .validate_player_charge_against_db_creature(
            character.position.map_id,
            character.guid,
            target,
            &session.movement.db_creature_navigation,
        )
        .await;
    match validation.check {
        PlayerChargeCheck::Clear => None,
        PlayerChargeCheck::NavigationBlocked(DbCreatureNavigationResult::LineOfSightBlocked) => {
            Some(SPELL_FAILED_LINE_OF_SIGHT)
        }
        PlayerChargeCheck::NavigationBlocked(DbCreatureNavigationResult::PathUnavailable) => {
            Some(SPELL_FAILED_NOPATH)
        }
        PlayerChargeCheck::NoActiveCharacter
        | PlayerChargeCheck::MissingTarget
        | PlayerChargeCheck::TargetNotAlive
        | PlayerChargeCheck::NavigationBlocked(_) => Some(SPELL_FAILED_OUT_OF_RANGE),
    }
}

#[allow(clippy::too_many_arguments)]
pub(in crate::world) async fn apply_charge_movement(
    stream: &mut WorldPacketSink,
    shared_world: SharedWorldDeps<'_>,
    session: &mut WorldSessionState,
    caster: ObjectGuid,
    target: ObjectGuid,
    spell_speed: f32,
    spell_id: u32,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    let Some(character) = session.character.active_character.as_ref() else {
        return Ok(());
    };
    let map_id = character.position.map_id;
    let character_guid = character.guid;
    let start = character.position;
    let Some(creature) = shared_world.maps.db_creature_snapshot(map_id, target).await else {
        return Ok(());
    };
    if !creature.is_alive() {
        return Ok(());
    }

    let destination = charge_destination(start, &creature);
    let speed = if spell_speed > 0.0 {
        spell_speed
    } else {
        BASE_CHARGE_SPEED
    };
    let duration_ms = charge_duration_millis(start, destination, speed);
    let move_body = build_monster_move_facing_target_body(
        caster,
        start,
        destination,
        spell_id,
        duration_ms,
        target,
    )?;

    if let Some(character) = session.character.active_character.as_mut() {
        character.position = destination;
    }
    let environment_packets = shared_world
        .maps
        .set_player_position(map_id, character_guid, destination)
        .await?;

    send_packet(
        stream,
        WorldOpcode::SmsgMonsterMove as u16,
        &move_body,
        Some(&mut *header_crypto),
    )
    .await?;
    shared_world.sessions.dispatch(environment_packets).await;
    let observer_packets = shared_world
        .maps
        .broadcast_nearby_player_packet(
            map_id,
            character_guid,
            PLAYER_VISIBILITY_RADIUS_YARDS,
            OutboundWorldPacket {
                opcode: WorldOpcode::SmsgMonsterMove as u16,
                body: move_body,
            },
        )
        .await;
    shared_world.sessions.dispatch(observer_packets).await;

    let next_swing_at = Some(Instant::now());
    mirror_session_player_auto_attack(session, Some(target), next_swing_at);
    shared_world
        .maps
        .set_player_auto_attack(map_id, character_guid, Some(target), next_swing_at)
        .await;

    send_packet(
        stream,
        WorldOpcode::SmsgAttackStart as u16,
        &build_attack_start_body(caster, target),
        Some(&mut *header_crypto),
    )
    .await?;
    broadcast_player_attack_start(shared_world, session, caster, target).await;
    Ok(())
}

pub(in crate::world) fn charge_destination(
    start: WorldPosition,
    target: &DbCreatureRuntime,
) -> WorldPosition {
    let target_position = target.current_position;
    let dx = start.x - target_position.x;
    let dy = start.y - target_position.y;
    let distance = (dx * dx + dy * dy).sqrt();
    let reach = creature_combat_reach(&target.spawn.template).max(DEFAULT_WORLD_OBJECT_SIZE);
    let (offset_x, offset_y) = if distance > f32::EPSILON {
        (dx / distance * reach, dy / distance * reach)
    } else {
        (reach, 0.0)
    };
    WorldPosition::new(
        target_position.map_id,
        target_position.x + offset_x,
        target_position.y + offset_y,
        target_position.z + 1.0,
        angle_towards(target_position, start),
    )
}

pub(in crate::world) fn charge_duration_millis(
    start: WorldPosition,
    destination: WorldPosition,
    speed: f32,
) -> u32 {
    let dx = destination.x - start.x;
    let dy = destination.y - start.y;
    let dz = destination.z - start.z;
    (((dx * dx + dy * dy + dz * dz).sqrt() / speed.max(f32::EPSILON)) * 1000.0)
        .round()
        .max(1.0) as u32
}

pub(in crate::world) fn angle_towards(from: WorldPosition, to: WorldPosition) -> f32 {
    (to.y - from.y).atan2(to.x - from.x)
}

pub(in crate::world) async fn spell_cast_failure(
    shared_world: SharedWorldDeps<'_>,
    world_db_pool: &MySqlPool,
    session: &mut WorldSessionState,
    spell_template: &wow_db::SpellTemplateQuery,
    spell_profile: &SpellCastProfile,
    targets: &SpellCastTargets,
    now: Instant,
) -> anyhow::Result<Option<u8>> {
    if session.death.player_death_state != PlayerDeathState::Alive {
        return Ok(Some(SPELL_FAILED_CASTER_DEAD));
    }
    if let Some(character) = session.character.active_character.as_ref() {
        let player_snapshot = shared_world
            .maps
            .player_runtime_snapshot(character.position.map_id, character.guid)
            .await;
        if session.character.player_health == 0
            && player_snapshot
                .as_ref()
                .is_some_and(|snapshot| snapshot.health == 0)
        {
            return Ok(Some(SPELL_FAILED_CASTER_DEAD));
        }
        if let Some(failure) = shared_world
            .maps
            .player_spell_cast_failure(
                character.position.map_id,
                character.guid,
                Some(spell_template),
                spell_profile,
                spell_template_requires_main_hand_weapon_or_weapon_class(spell_template),
                now,
            )
            .await
        {
            return Ok(Some(failure));
        }
        if spell_template.caster_aura_state != 0
            && !player_snapshot.as_ref().is_some_and(|snapshot| {
                has_required_spell_aura_state(snapshot.aura_state, spell_template.caster_aura_state)
            })
        {
            return Ok(Some(SPELL_FAILED_CASTER_AURASTATE));
        }
    }
    if let Some(failure) =
        spell_equipped_item_cast_failure(world_db_pool, session, spell_template).await?
    {
        return Ok(Some(failure));
    }
    Ok(spell_target_cast_failure(
        shared_world,
        world_db_pool,
        session,
        spell_template,
        spell_profile,
        targets,
    )
    .await)
}

pub(in crate::world) async fn spell_equipped_item_cast_failure(
    world_db_pool: &MySqlPool,
    session: &WorldSessionState,
    spell_template: &wow_db::SpellTemplateQuery,
) -> anyhow::Result<Option<u8>> {
    if !spell_template_requires_equipped_item(spell_template) {
        return Ok(None);
    }
    let equipped_templates =
        load_equipped_item_templates(world_db_pool, &session.inventory.items).await?;
    Ok(spell_equipped_item_cast_failure_with_equipped_templates(
        spell_template,
        &equipped_templates,
    ))
}

fn spell_template_requires_equipped_item(spell_template: &wow_db::SpellTemplateQuery) -> bool {
    spell_template.equipped_item_class >= 0
        || spell_template.attributes_ex3
            & (SPELL_ATTR_EX3_REQUIRES_MAIN_HAND_WEAPON | SPELL_ATTR_EX3_REQUIRES_OFFHAND_WEAPON)
            != 0
}

pub(in crate::world) fn spell_template_requires_main_hand_weapon_or_weapon_class(
    spell_template: &wow_db::SpellTemplateQuery,
) -> bool {
    spell_template.attributes_ex3 & SPELL_ATTR_EX3_REQUIRES_MAIN_HAND_WEAPON != 0
        || spell_template.equipped_item_class == ITEM_CLASS_WEAPON as i32
}

pub(in crate::world) fn spell_equipped_item_cast_failure_with_equipped_templates(
    spell_template: &wow_db::SpellTemplateQuery,
    equipped_templates: &[EquippedItemTemplate],
) -> Option<u8> {
    if spell_template.attributes_ex3 & SPELL_ATTR_EX3_REQUIRES_MAIN_HAND_WEAPON != 0
        && !equipped_slot_has_weapon(equipped_templates, EQUIPMENT_SLOT_MAINHAND)
    {
        return Some(SPELL_FAILED_EQUIPPED_ITEM_CLASS_MAINHAND);
    }
    if spell_template.attributes_ex3 & SPELL_ATTR_EX3_REQUIRES_OFFHAND_WEAPON != 0
        && !equipped_slot_has_weapon(equipped_templates, EQUIPMENT_SLOT_OFFHAND)
    {
        return Some(SPELL_FAILED_EQUIPPED_ITEM_CLASS_OFFHAND);
    }
    if spell_template.equipped_item_class < 0 {
        return None;
    }
    (!equipped_templates
        .iter()
        .any(|item| equipped_item_matches_spell_requirements(spell_template, item)))
    .then_some(SPELL_FAILED_EQUIPPED_ITEM_CLASS)
}

fn equipped_slot_has_weapon(equipped_templates: &[EquippedItemTemplate], slot: u8) -> bool {
    equipped_templates
        .iter()
        .any(|item| item.slot == slot && item.template.class == ITEM_CLASS_WEAPON)
}

fn equipped_item_matches_spell_requirements(
    spell_template: &wow_db::SpellTemplateQuery,
    item: &EquippedItemTemplate,
) -> bool {
    if spell_template.equipped_item_class != item.template.class as i32 {
        return false;
    }
    if spell_template.equipped_item_subclass_mask == 0 {
        return true;
    }
    let Some(item_subclass_bit) = 1u32.checked_shl(item.template.subclass) else {
        return false;
    };
    spell_template.equipped_item_subclass_mask as u32 & item_subclass_bit != 0
}

pub(in crate::world) async fn spell_target_cast_failure(
    shared_world: SharedWorldDeps<'_>,
    world_db_pool: &MySqlPool,
    session: &mut WorldSessionState,
    spell_template: &wow_db::SpellTemplateQuery,
    spell_profile: &SpellCastProfile,
    targets: &SpellCastTargets,
) -> Option<u8> {
    if spell_profile.kind == SpellCastKind::Charge {
        if let Some(failure) = spell_unit_target_cast_failure(
            shared_world,
            world_db_pool,
            session,
            spell_template,
            spell_profile,
            targets,
        )
        .await
        {
            return Some(failure);
        }
        return spell_charge_cast_failure(shared_world, session, targets).await;
    }
    if spell_profile.kind == SpellCastKind::DirectHeal {
        return spell_heal_cast_failure(shared_world, session, spell_template, targets).await;
    }
    if let Some(failure) = spell_unit_target_cast_failure(
        shared_world,
        world_db_pool,
        session,
        spell_template,
        spell_profile,
        targets,
    )
    .await
    {
        return Some(failure);
    }
    if let Some(failure) =
        spell_single_dispel_cast_failure(shared_world, session, spell_template, targets).await
    {
        return Some(failure);
    }
    if spell_template.target_aura_state != 0 {
        let character = session.character.active_character.as_ref()?;
        let target = targets.unit_target?;
        if target.is_player() {
            let target_guid = target.counter();
            let snapshot = shared_world
                .maps
                .player_runtime_snapshot(character.position.map_id, target_guid)
                .await?;
            if !has_required_spell_aura_state(snapshot.aura_state, spell_template.target_aura_state)
            {
                return Some(SPELL_FAILED_TARGET_AURASTATE);
            }
        }
    }
    if let Some(failure) = spell_combo_point_cast_failure(
        shared_world,
        session,
        spell_template,
        spell_profile,
        targets,
    )
    .await
    {
        return Some(failure);
    }
    spell_melee_cast_failure(shared_world, session, spell_profile, targets).await
}

pub(in crate::world) async fn spell_single_dispel_cast_failure(
    shared_world: SharedWorldDeps<'_>,
    session: &WorldSessionState,
    spell_template: &wow_db::SpellTemplateQuery,
    targets: &SpellCastTargets,
) -> Option<u8> {
    if spell_template.effect1 != SPELL_EFFECT_DISPEL
        || spell_template.effect2 != 0
        || spell_template.effect3 != 0
    {
        return None;
    }

    let Ok(dispel_type) = u32::try_from(spell_template.effect_misc_value1) else {
        return None;
    };
    if dispel_type == 0 {
        return None;
    }

    let character = session.character.active_character.as_ref()?;
    let caster = ObjectGuid::new(HighGuid::Player, 0, character.guid);
    let target = targets.unit_target.unwrap_or(caster);

    let has_matching_aura = if target.is_player() {
        let target_guid = target.counter();
        if target_guid == character.guid
            && session
                .auras
                .active_auras
                .iter()
                .any(|aura| active_aura_matches_dispel_type(aura, dispel_type))
        {
            true
        } else {
            shared_world
                .maps
                .player_runtime_snapshot(character.position.map_id, target_guid)
                .await
                .is_some_and(|snapshot| {
                    snapshot
                        .active_auras
                        .iter()
                        .any(|aura| active_aura_matches_dispel_type(aura, dispel_type))
                })
        }
    } else if target.is_creature() {
        shared_world
            .maps
            .db_creature_snapshot(character.position.map_id, target)
            .await
            .is_some_and(|snapshot| {
                snapshot
                    .active_auras
                    .iter()
                    .any(|aura| active_aura_matches_dispel_type(aura, dispel_type))
            })
    } else {
        false
    };

    (!has_matching_aura).then_some(SPELL_FAILED_NOTHING_TO_DISPEL)
}

pub(in crate::world) async fn player_aura_rank_cast_failure(
    deps: SpellCastDeps<'_>,
    session: &WorldSessionState,
    spell_template: &wow_db::SpellTemplateQuery,
    spell_profile: &SpellCastProfile,
    targets: &SpellCastTargets,
    caster: ObjectGuid,
) -> anyhow::Result<Option<u8>> {
    let plan = SpellInfo::from_template(spell_template)
        .player_spell_plan()
        .filter(|plan| plan.profile.kind == spell_profile.kind);
    let Some(plan) = plan else {
        return Ok(None);
    };
    let has_aura_application = plan
        .effects
        .iter()
        .any(|effect| effect.dispatch == SpellEffectDispatch::ApplyAura);
    let has_direct_damage_application = plan.effects.iter().any(|effect| {
        matches!(
            effect.dispatch,
            SpellEffectDispatch::SchoolDamage
                | SpellEffectDispatch::WeaponDamage
                | SpellEffectDispatch::WeaponPercentDamage
        )
    });
    if !has_aura_application
        || !matches!(
            plan.profile.kind,
            SpellCastKind::AuraApplication | SpellCastKind::DirectHeal
        )
        || has_direct_damage_application
    {
        return Ok(None);
    }
    match plan.target {
        SpellPlanTarget::Caster => {
            let resolution = aura_rank_conflict_resolution(
                deps.shared_world.object_mgr,
                deps.world_db_pool,
                spell_template.id,
                caster,
                &session.auras.active_auras,
            )
            .await?;
            Ok(resolution.failure)
        }
        SpellPlanTarget::Unit | SpellPlanTarget::HostileUnit | SpellPlanTarget::FriendlyUnit => {
            let Some(character) = session.character.active_character.as_ref() else {
                return Ok(None);
            };
            let Some(target) = targets.unit_target else {
                return Ok(None);
            };
            let active_auras = if target.is_player() {
                if target.counter() == character.guid {
                    session.auras.active_auras.clone()
                } else {
                    let Some(snapshot) = deps
                        .shared_world
                        .maps
                        .player_runtime_snapshot(character.position.map_id, target.counter())
                        .await
                    else {
                        return Ok(None);
                    };
                    snapshot.active_auras
                }
            } else if target.is_creature() {
                let Some(creature) = deps
                    .shared_world
                    .maps
                    .db_creature_snapshot(character.position.map_id, target)
                    .await
                else {
                    return Ok(None);
                };
                creature.active_auras
            } else {
                return Ok(None);
            };
            let resolution = aura_rank_conflict_resolution(
                deps.shared_world.object_mgr,
                deps.world_db_pool,
                spell_template.id,
                caster,
                &active_auras,
            )
            .await?;
            Ok(resolution.failure)
        }
        SpellPlanTarget::CasterAreaEnemy { .. }
        | SpellPlanTarget::DestinationAreaEnemy
        | SpellPlanTarget::Destination => Ok(None),
    }
}

#[allow(clippy::too_many_arguments)]
pub(in crate::world) async fn begin_failed_hostile_db_creature_spell_retaliation(
    stream: &mut WorldPacketSink,
    shared_world: SharedWorldDeps<'_>,
    session: &mut WorldSessionState,
    caster: ObjectGuid,
    map_id: u32,
    spell_template: &wow_db::SpellTemplateQuery,
    spell_profile: &SpellCastProfile,
    targets: &SpellCastTargets,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    let spell_plan = SpellInfo::from_template(spell_template)
        .player_spell_plan()
        .filter(|plan| plan.profile.kind == spell_profile.kind);
    if !spell_plan
        .as_ref()
        .is_some_and(SpellPlan::should_retaliate_on_failed_hostile_cast)
    {
        return Ok(());
    }
    let Some(target) = targets.unit_target.filter(|target| target.is_creature()) else {
        return Ok(());
    };
    begin_db_creature_retaliation_if_needed(
        stream,
        shared_world,
        map_id,
        session,
        target,
        caster,
        header_crypto,
    )
    .await
}

#[allow(clippy::too_many_arguments)]
pub(in crate::world) async fn player_db_creature_spell_target_outcome(
    shared_world: SharedWorldDeps<'_>,
    session: &WorldSessionState,
    character_guid: u32,
    map_id: u32,
    spell_template: &wow_db::SpellTemplateQuery,
    spell_profile: &SpellCastProfile,
    targets: &SpellCastTargets,
) -> anyhow::Result<Option<PlayerSpellTargetOutcome>> {
    let spell_plan = SpellInfo::from_template(spell_template)
        .player_spell_plan()
        .filter(|plan| plan.profile.kind == spell_profile.kind);
    if !spell_plan
        .as_ref()
        .is_some_and(SpellPlan::uses_db_creature_unit_target_outcome)
    {
        return Ok(None);
    }
    let Some(target) = targets.unit_target.filter(|target| target.is_creature()) else {
        return Ok(None);
    };
    let Some(target_creature) = shared_world.maps.db_creature_snapshot(map_id, target).await else {
        return Ok(None);
    };
    if target_creature.is_evading_home() {
        return Ok(Some(PlayerSpellTargetOutcome {
            target,
            miss_info: Some(SPELL_MISS_EVADE),
        }));
    }
    if spell_template_is_reflectable(spell_template)
        && active_auras_reflect_spell_school(
            &target_creature.active_auras,
            spell_school_mask_from_school(spell_template.school),
        )
    {
        return Ok(Some(PlayerSpellTargetOutcome {
            target,
            miss_info: Some(SPELL_MISS_REFLECT),
        }));
    }
    if active_auras_are_immune_to_spell(&target_creature.active_auras, spell_template) {
        return Ok(Some(PlayerSpellTargetOutcome {
            target,
            miss_info: Some(SPELL_MISS_IMMUNE),
        }));
    }
    let combat_stats = shared_world
        .maps
        .player_combat_stats(map_id, character_guid)
        .await
        .ok_or_else(|| {
            anyhow::anyhow!(
                "map-owned player combat stats missing for character {}",
                character_guid
            )
        })?;
    let character = session.character.active_character.as_ref();
    let (school, dmg_class) = spell_target_outcome_school_and_damage_class(spell_template);
    let outcome = roll_spell_damage_outcome(spell_damage_outcome_input(
        1,
        school,
        dmg_class,
        spell_template.attributes_ex2,
        spell_template.attributes_ex3,
        player_spell_snapshot(
            character.map(|character| character.level).unwrap_or(1),
            character.map(|character| character.class).unwrap_or(1),
            &combat_stats,
        ),
        db_creature_spell_snapshot(&target_creature),
    ));
    Ok(Some(PlayerSpellTargetOutcome {
        target,
        miss_info: outcome.miss_info,
    }))
}

fn spell_target_outcome_school_and_damage_class(
    spell_template: &wow_db::SpellTemplateQuery,
) -> (u8, u32) {
    let school = spell_template.school as u8;
    let dmg_class = if spell_template.dmg_class == SPELL_DAMAGE_CLASS_NONE
        && is_resistable_spell_school(school)
    {
        SPELL_DAMAGE_CLASS_MAGIC
    } else {
        spell_template.dmg_class
    };
    (school, dmg_class)
}

fn spell_uses_warrior_reactive_combo_target(spell_template: &wow_db::SpellTemplateQuery) -> bool {
    spell_template.power_type == POWER_TYPE_RAGE
        && spell_template.spell_family_name == 4
        && (spell_template.attributes_ex & SPELL_ATTR_EX_FINISHING_MOVE_DAMAGE) != 0
        && (spell_template.spell_family_flags & 1) != 0
}

pub(in crate::world) async fn spell_combo_point_cast_failure(
    shared_world: SharedWorldDeps<'_>,
    session: &WorldSessionState,
    spell_template: &wow_db::SpellTemplateQuery,
    spell_profile: &SpellCastProfile,
    targets: &SpellCastTargets,
) -> Option<u8> {
    if !spell_profile.needs_combo_points {
        return None;
    }
    let target = targets.unit_target?;
    let character = session.character.active_character.as_ref()?;
    let snapshot = shared_world
        .maps
        .player_runtime_snapshot(character.position.map_id, character.guid)
        .await?;
    if snapshot.combo_points == 0 || snapshot.combo_target != Some(target) {
        Some(
            if spell_uses_warrior_reactive_combo_target(spell_template) {
                SPELL_FAILED_CASTER_AURASTATE
            } else {
                SPELL_FAILED_NO_COMBO_POINTS
            },
        )
    } else {
        None
    }
}

pub(in crate::world) async fn spell_heal_cast_failure(
    shared_world: SharedWorldDeps<'_>,
    session: &WorldSessionState,
    spell_template: &wow_db::SpellTemplateQuery,
    targets: &SpellCastTargets,
) -> Option<u8> {
    let character = session.character.active_character.as_ref()?;
    let spell_info = SpellInfo::from_template(spell_template);
    let target_kind = spell_info
        .player_spell_plan()
        .filter(|plan| plan.profile.kind == SpellCastKind::DirectHeal)
        .map(|plan| plan.target.target_kind())
        .unwrap_or(SpellTargetKind::Caster);
    if target_kind == SpellTargetKind::Caster {
        return None;
    }
    let target = targets.unit_target?;
    if !target.is_player() {
        return Some(SPELL_FAILED_OUT_OF_RANGE);
    }
    let target_guid = target.counter();
    let Some(snapshot) = shared_world
        .maps
        .player_runtime_snapshot(character.position.map_id, target_guid)
        .await
    else {
        return Some(SPELL_FAILED_OUT_OF_RANGE);
    };
    if snapshot.health == 0 {
        return Some(SPELL_FAILED_OUT_OF_RANGE);
    }
    None
}

async fn spell_friendly_player_target_cast_failure(
    shared_world: SharedWorldDeps<'_>,
    session: &WorldSessionState,
    target: ObjectGuid,
) -> Option<u8> {
    let character = session.character.active_character.as_ref()?;
    if !target.is_player() {
        return Some(SPELL_FAILED_OUT_OF_RANGE);
    }
    let Some(snapshot) = shared_world
        .maps
        .player_runtime_snapshot(character.position.map_id, target.counter())
        .await
    else {
        return Some(SPELL_FAILED_OUT_OF_RANGE);
    };
    (snapshot.health == 0).then_some(SPELL_FAILED_OUT_OF_RANGE)
}

pub(in crate::world) fn creature_type_matches_target_mask(
    creature_type: u32,
    target_creature_type: u32,
) -> bool {
    if target_creature_type == 0 {
        return true;
    }
    let Some(creature_type_bit) = creature_type
        .checked_sub(1)
        .and_then(|bit| 1u32.checked_shl(bit))
    else {
        return false;
    };
    target_creature_type & creature_type_bit != 0
}

fn spell_hostile_creature_power_burn_cast_failure(
    spell_template: &wow_db::SpellTemplateQuery,
    target_creature: &DbCreatureRuntime,
) -> Option<u8> {
    let target_power_type = creature_unit_power_type(&target_creature.spawn.template);
    SpellInfo::from_template(spell_template)
        .effects
        .into_iter()
        .filter(|effect| {
            effect.dispatch == SpellEffectDispatch::PowerBurn
                && plan_effect_target(*effect).is_hostile()
        })
        .find_map(|effect| {
            let Ok(required_power_type) = u32::try_from(effect.misc_value) else {
                return Some(SPELL_FAILED_BAD_TARGETS);
            };
            (target_power_type != required_power_type).then_some(SPELL_FAILED_BAD_TARGETS)
        })
}

async fn spell_hostile_creature_target_cast_failure(
    shared_world: SharedWorldDeps<'_>,
    world_db_pool: &MySqlPool,
    session: &WorldSessionState,
    spell_template: &wow_db::SpellTemplateQuery,
    target: ObjectGuid,
) -> Option<u8> {
    let character = session.character.active_character.as_ref()?;
    if !target.is_creature() {
        return Some(SPELL_FAILED_BAD_TARGETS);
    }
    let Some(snapshot) = shared_world
        .maps
        .db_creature_snapshot(character.position.map_id, target)
        .await
    else {
        return Some(SPELL_FAILED_OUT_OF_RANGE);
    };
    if !creature_type_matches_target_mask(
        snapshot.spawn.template.creature_type,
        spell_template.target_creature_type,
    ) {
        return Some(SPELL_FAILED_BAD_TARGETS);
    }
    if let Some(failure) = spell_hostile_creature_power_burn_cast_failure(spell_template, &snapshot)
    {
        return Some(failure);
    }
    let range = if spell_template.range_index == 0 {
        None
    } else {
        let range = shared_world.maps.spell_range(spell_template.range_index);
        if range.is_none() {
            return Some(SPELL_FAILED_OUT_OF_RANGE);
        }
        range
    };
    let validation = shared_world
        .maps
        .validate_player_spell_against_db_creature(
            character.position.map_id,
            character.guid,
            target,
            &session.movement.db_creature_navigation,
            range,
            spell_requires_infront_target(
                shared_world.object_mgr,
                world_db_pool,
                spell_template.id,
            )
            .await
            .unwrap_or(false),
        )
        .await;
    match validation.check {
        PlayerSpellTargetCheck::Clear => None,
        PlayerSpellTargetCheck::BadFacing => Some(SPELL_FAILED_UNIT_NOT_INFRONT),
        PlayerSpellTargetCheck::NotAttackable => Some(SPELL_FAILED_BAD_TARGETS),
        PlayerSpellTargetCheck::NavigationBlocked(
            DbCreatureNavigationResult::LineOfSightBlocked,
        ) => Some(SPELL_FAILED_LINE_OF_SIGHT),
        PlayerSpellTargetCheck::TooClose => Some(SPELL_FAILED_TOO_CLOSE),
        PlayerSpellTargetCheck::NoActiveCharacter
        | PlayerSpellTargetCheck::MissingTarget
        | PlayerSpellTargetCheck::TargetNotAlive
        | PlayerSpellTargetCheck::NavigationBlocked(_)
        | PlayerSpellTargetCheck::OutOfRange => Some(SPELL_FAILED_OUT_OF_RANGE),
    }
}

pub(in crate::world) async fn spell_unit_target_cast_failure(
    shared_world: SharedWorldDeps<'_>,
    world_db_pool: &MySqlPool,
    session: &WorldSessionState,
    spell_template: &wow_db::SpellTemplateQuery,
    spell_profile: &SpellCastProfile,
    targets: &SpellCastTargets,
) -> Option<u8> {
    let spell_info = SpellInfo::from_template(spell_template);
    let target_kind = spell_info
        .player_spell_plan()
        .filter(|plan| plan.profile.kind == spell_profile.kind)
        .map(|plan| plan.target.target_kind())
        .unwrap_or(SpellTargetKind::Caster);
    if target_kind.requires_unit_target() && targets.unit_target.is_none() {
        return Some(SPELL_FAILED_BAD_IMPLICIT_TARGETS);
    }
    let target = targets.unit_target;
    if target_kind == SpellTargetKind::FriendlyUnit {
        return spell_friendly_player_target_cast_failure(shared_world, session, target?).await;
    }
    if target_kind == SpellTargetKind::Unit {
        let target = target?;
        if target.is_player() {
            return spell_friendly_player_target_cast_failure(shared_world, session, target).await;
        }
        if target.is_creature() {
            if spell_profile.requires_melee
                || matches!(spell_profile.kind, SpellCastKind::NextMeleeSwing)
            {
                return None;
            }
            return spell_hostile_creature_target_cast_failure(
                shared_world,
                world_db_pool,
                session,
                spell_template,
                target,
            )
            .await;
        }
        return Some(SPELL_FAILED_BAD_TARGETS);
    }
    if target_kind != SpellTargetKind::HostileUnit
        || spell_profile.requires_melee
        || matches!(spell_profile.kind, SpellCastKind::NextMeleeSwing)
    {
        return None;
    }
    spell_hostile_creature_target_cast_failure(
        shared_world,
        world_db_pool,
        session,
        spell_template,
        target?,
    )
    .await
}

pub(in crate::world) async fn spell_requires_infront_target(
    object_mgr: &ObjectMgr,
    world_db_pool: &MySqlPool,
    spell_id: u32,
) -> anyhow::Result<bool> {
    Ok(object_mgr
        .spell_facing_flag(world_db_pool, spell_id)
        .await?
        & SPELL_FACING_FLAG_INFRONT
        != 0)
}

pub(in crate::world) async fn resolve_player_spell_cast_targets(
    maps: &MapRuntimeManager,
    map_id: u32,
    character_guid: u32,
    mut targets: SpellCastTargets,
    spell_info: &SpellInfo<'_>,
    kind: SpellCastKind,
) -> SpellCastTargets {
    let target_kind = spell_info
        .player_spell_plan()
        .filter(|plan| plan.profile.kind == kind)
        .map(|plan| plan.target.target_kind())
        .unwrap_or(SpellTargetKind::Caster);
    if target_kind.requires_unit_target() && targets.unit_target.is_none() {
        if let Some(selected_target) = maps.player_selected_target(map_id, character_guid).await {
            targets.target_mask =
                (targets.target_mask | SPELL_CAST_TARGET_UNIT) & !SPELL_CAST_TARGET_UNIT_ENEMY;
            targets.unit_target = Some(selected_target);
        }
    }
    targets
}

pub(in crate::world) fn spell_blocks_mana_regen(template: &wow_db::SpellTemplateQuery) -> bool {
    SpellInfo::from_template(template)
        .player_spell_plan()
        .map(|plan| plan.behavior.blocks_mana_regen)
        .unwrap_or(false)
}

pub(in crate::world) async fn sync_session_player_power_from_map(
    maps: &MapRuntimeManager,
    session: &mut WorldSessionState,
    map_id: u32,
    character_guid: u32,
) {
    if let Some(snapshot) = maps.player_runtime_snapshot(map_id, character_guid).await {
        session.character.player_mana = snapshot.power1;
        session.character.player_rage = snapshot.power2;
        session.character.player_energy = snapshot.power4;
    }
}

pub(in crate::world) async fn send_player_spell_power_update(
    stream: &mut WorldPacketSink,
    caster: ObjectGuid,
    spell_profile: &SpellCastProfile,
    session: &WorldSessionState,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    let power_update = match spell_profile.power {
        SpellPowerCost::Rage { .. } => {
            build_player_rage_update_body(caster, session.character.player_rage)?
        }
        SpellPowerCost::Mana { .. } => {
            build_player_mana_update_body(caster, session.character.player_mana)?
        }
        SpellPowerCost::Energy { .. } => {
            build_player_energy_update_body(caster, session.character.player_energy)?
        }
    };
    send_packet(
        stream,
        WorldOpcode::SmsgUpdateObject as u16,
        &power_update,
        Some(header_crypto),
    )
    .await
}

pub(in crate::world) fn spell_cast_time_millis(cast_time: Option<SpellCastTimeEntry>) -> u32 {
    let Some(cast_time) = cast_time else {
        return 0;
    };
    cast_time
        .cast_time_millis
        .max(cast_time.min_cast_time_millis)
        .max(0) as u32
}

pub(in crate::world) async fn spell_travel_delay_millis(
    shared_world: SharedWorldDeps<'_>,
    session: &WorldSessionState,
    spell_template: &wow_db::SpellTemplateQuery,
    targets: &SpellCastTargets,
) -> u32 {
    if spell_template.speed <= 0.0 {
        return 0;
    }
    let spell_info = SpellInfo::from_template(spell_template);
    let has_missile_damage = spell_info
        .player_spell_plan()
        .map(|plan| {
            plan.effects
                .iter()
                .any(|effect| effect.dispatch == SpellEffectDispatch::SchoolDamage)
        })
        .unwrap_or(false);
    if !has_missile_damage {
        return 0;
    }
    let Some(character) = session.character.active_character.as_ref() else {
        return 0;
    };
    let Some(target) = targets.unit_target.filter(|target| target.is_creature()) else {
        return 0;
    };
    let Some(creature) = shared_world
        .maps
        .db_creature_snapshot(character.position.map_id, target)
        .await
    else {
        return 0;
    };
    let distance = character.position.distance_to(&creature.current_position);
    ((distance / spell_template.speed.max(f32::EPSILON)) * 1000.0)
        .round()
        .max(1.0) as u32
}

mod auras;
mod casting;
mod cooldowns;
mod definitions;
mod effects;
mod packets;
mod plan;
mod skills;
mod spell;
mod spell_mgr;
mod targets;
mod values;

pub(in crate::world) use self::auras::*;
pub(in crate::world) use self::casting::*;
pub(in crate::world) use self::cooldowns::*;
pub(in crate::world) use self::definitions::*;
pub(in crate::world) use self::effects::*;
pub(in crate::world) use self::packets::*;
pub(in crate::world) use self::plan::plan_effect_target;
pub(in crate::world) use self::skills::*;
pub(in crate::world) use self::spell::*;
pub(in crate::world) use self::spell_mgr::*;
pub(in crate::world) use self::targets::*;
pub(in crate::world) use self::values::*;

#[cfg(test)]
pub(in crate::world) fn player_spell_cast_profile(
    template: &wow_db::SpellTemplateQuery,
) -> Option<SpellCastProfile> {
    SpellInfo::from_template(template)
        .prepare_player_cast()
        .map(|prepared| prepared.profile)
}

#[cfg(test)]
pub(in crate::world) fn item_use_spell_cast_profile(
    template: &wow_db::SpellTemplateQuery,
) -> Option<SpellCastProfile> {
    SpellInfo::from_template(template)
        .prepare_item_cast(ObjectGuid::EMPTY)
        .map(|prepared| prepared.profile)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::world) struct AuraRankConflictResolution {
    pub(in crate::world) failure: Option<u8>,
    pub(in crate::world) replace_spell_ids: Vec<u32>,
    pub(in crate::world) replace_any_caster_spell_ids: Vec<u32>,
    pub(in crate::world) stack_limit: u8,
}

impl AuraRankConflictResolution {
    pub(in crate::world) fn clear() -> Self {
        Self {
            failure: None,
            replace_spell_ids: Vec::new(),
            replace_any_caster_spell_ids: Vec::new(),
            stack_limit: 1,
        }
    }
}

pub(in crate::world) async fn aura_rank_conflict_resolution(
    object_mgr: &ObjectMgr,
    world_db_pool: &MySqlPool,
    spell_id: u32,
    caster: ObjectGuid,
    active_auras: &[ActiveAura],
) -> anyhow::Result<AuraRankConflictResolution> {
    let spell_template = object_mgr.spell_template(world_db_pool, spell_id).await?;
    let stack_limit = spell_template
        .as_ref()
        .map(|template| template.stack_amount.clamp(1, u32::from(u8::MAX)) as u8)
        .unwrap_or(1);
    let conflicting_auras = active_auras
        .iter()
        .filter(|aura| aura.spell_id != spell_id || aura.caster != caster)
        .collect::<Vec<_>>();
    if conflicting_auras.is_empty() {
        return Ok(AuraRankConflictResolution {
            stack_limit,
            ..AuraRankConflictResolution::clear()
        });
    }
    if spell_template
        .as_ref()
        .is_some_and(spell_is_priest_power_word_shield)
        && conflicting_auras
            .iter()
            .any(|aura| aura.spell_id == PRIEST_WEAKENED_SOUL_SPELL_ID)
    {
        return Ok(AuraRankConflictResolution {
            failure: Some(SPELL_FAILED_AURA_BOUNCED),
            replace_spell_ids: Vec::new(),
            replace_any_caster_spell_ids: Vec::new(),
            stack_limit,
        });
    }
    let new_chain = object_mgr.spell_chain(world_db_pool, spell_id).await?;
    let new_root = new_chain.map(spell_chain_root);
    let mut replace_spell_ids = Vec::new();
    let mut replace_any_caster_spell_ids = Vec::new();

    for existing in &conflicting_auras {
        let Some(new_chain) = new_chain else {
            continue;
        };
        let existing_chain = object_mgr
            .spell_chain(world_db_pool, existing.spell_id)
            .await?;
        let Some(existing_chain) = existing_chain.filter(|existing_chain| {
            Some(spell_chain_root(*existing_chain)) == new_root
                && existing_chain.spell_id != new_chain.spell_id
        }) else {
            continue;
        };
        if existing_chain.rank >= new_chain.rank {
            if existing.caster == caster || existing.positive {
                return Ok(AuraRankConflictResolution {
                    failure: Some(SPELL_FAILED_AURA_BOUNCED),
                    replace_spell_ids: Vec::new(),
                    replace_any_caster_spell_ids: Vec::new(),
                    stack_limit,
                });
            }
            continue;
        }
        if existing.caster == caster {
            push_unique_spell_id(&mut replace_spell_ids, existing.spell_id);
        } else if existing.positive {
            push_unique_spell_id(&mut replace_any_caster_spell_ids, existing.spell_id);
        }
    }

    if conflicting_auras.iter().all(|existing| {
        existing.caster == caster && replace_spell_ids.contains(&existing.spell_id)
            || replace_any_caster_spell_ids.contains(&existing.spell_id)
    }) {
        return Ok(AuraRankConflictResolution {
            failure: None,
            replace_spell_ids,
            replace_any_caster_spell_ids,
            stack_limit,
        });
    }

    let new_groups = object_mgr
        .spell_group_memberships(world_db_pool, spell_id)
        .await?;
    if !new_groups.is_empty() {
        for existing in &conflicting_auras {
            if existing.caster == caster && replace_spell_ids.contains(&existing.spell_id)
                || replace_any_caster_spell_ids.contains(&existing.spell_id)
            {
                continue;
            }
            let existing_groups = object_mgr
                .spell_group_memberships(world_db_pool, existing.spell_id)
                .await?;
            for group in &new_groups {
                if !existing_groups
                    .iter()
                    .any(|existing_group| existing_group.group_id == group.group_id)
                {
                    continue;
                }
                match group.rule {
                    SPELL_GROUP_RULE_UNIQUE => {
                        push_unique_spell_id(&mut replace_any_caster_spell_ids, existing.spell_id);
                    }
                    SPELL_GROUP_RULE_UNIQUE_PER_CASTER if existing.caster == caster => {
                        push_unique_spell_id(&mut replace_spell_ids, existing.spell_id);
                    }
                    _ => {}
                }
                break;
            }
        }
    }
    Ok(AuraRankConflictResolution {
        failure: None,
        replace_spell_ids,
        replace_any_caster_spell_ids,
        stack_limit,
    })
}

pub(in crate::world) fn spell_chain_root(chain: wow_db::SpellChainQuery) -> u32 {
    if chain.first_spell != 0 {
        chain.first_spell
    } else {
        chain.spell_id
    }
}

pub(in crate::world) fn push_unique_spell_id(spell_ids: &mut Vec<u32>, spell_id: u32) {
    if !spell_ids.contains(&spell_id) {
        spell_ids.push(spell_id);
    }
}

pub(in crate::world) fn build_active_aura(
    template: &wow_db::SpellTemplateQuery,
    caster: ObjectGuid,
    level: u8,
    value_context: SpellEffectValueContext,
    now: Instant,
    duration: Option<SpellDurationEntry>,
) -> ActiveAura {
    let duration_millis = duration
        .map(|duration| {
            if duration.duration_millis == -1 {
                -1
            } else {
                duration.duration_millis.abs()
            }
        })
        .unwrap_or(0);
    let spell_info = SpellInfo::from_template(template);
    let channel_interrupt_flags = spell_info
        .plan_channel()
        .map_or(0, |channel| match channel {
            SpellPlanChannel::SelfAura {
                interrupt_flags, ..
            }
            | SpellPlanChannel::UnitPeriodicTrigger {
                interrupt_flags, ..
            }
            | SpellPlanChannel::PersistentArea {
                interrupt_flags, ..
            } => interrupt_flags,
        });
    ActiveAura {
        spell_id: template.id,
        caster,
        level,
        interrupt_flags: template.aura_interrupt_flags | channel_interrupt_flags,
        positive: active_aura_is_positive(&spell_info),
        visible: true,
        duration_millis: (duration_millis > 0).then_some(duration_millis as u32),
        expires_at: (duration_millis > 0)
            .then_some(now + Duration::from_millis(duration_millis as u64)),
        periodic_damage: spell_periodic_damage_aura(&spell_info, level, value_context, now),
        periodic_regen: spell_periodic_regen_aura(&spell_info, value_context, now),
        stat_modifiers: spell_aura_stat_modifiers(&spell_info, value_context),
        proc_triggers: spell_aura_proc_triggers(&spell_info),
    }
}

pub(in crate::world) async fn resolve_active_aura_transform_displays(
    object_mgr: &ObjectMgr,
    world_db_pool: &MySqlPool,
    aura: &mut ActiveAura,
) -> anyhow::Result<()> {
    for modifier in &mut aura.stat_modifiers {
        let AuraStatModifier::Transform {
            display_id,
            creature_entry,
        } = modifier
        else {
            continue;
        };
        if *creature_entry == 0 {
            continue;
        }
        let Some(template) = object_mgr
            .creature_template(world_db_pool, *creature_entry)
            .await?
        else {
            warn!(
                spell_id = aura.spell_id,
                creature_entry, "Transform aura references missing creature_template entry"
            );
            continue;
        };
        *display_id = choose_creature_display(&template).display_id;
    }
    Ok(())
}

pub(in crate::world) fn active_aura_is_positive(spell_info: &SpellInfo<'_>) -> bool {
    !spell_info.effects.iter().any(|effect| {
        effect.dispatch == SpellEffectDispatch::ApplyAura
            && (effect_targets_direct_hostile_unit(*effect)
                || effect_targets_caster_centered_hostile_area(*effect)
                || effect_targets_destination_hostile_area(*effect)
                || effect.aura_name == SPELL_AURA_PERIODIC_DAMAGE)
    })
}

pub(in crate::world) fn spell_periodic_damage_aura(
    spell_info: &SpellInfo<'_>,
    caster_level: u8,
    value_context: SpellEffectValueContext,
    now: Instant,
) -> Option<PeriodicDamageAura> {
    spell_info
        .effects
        .iter()
        .copied()
        .find(|effect| {
            effect.dispatch == SpellEffectDispatch::ApplyAura
                && effect.aura_name == SPELL_AURA_PERIODIC_DAMAGE
                && effect.amplitude > 0
        })
        .and_then(|effect| {
            let damage = spell_effect_calculated_u32(effect, value_context)?;
            Some(PeriodicDamageAura {
                aura_name: effect.aura_name,
                school: spell_info.template.school,
                damage_class: spell_info.template.dmg_class,
                attributes_ex2: spell_info.template.attributes_ex2,
                attributes_ex3: spell_info.template.attributes_ex3,
                caster_snapshot: spell_periodic_damage_fallback_caster_snapshot(caster_level),
                amount: damage,
                tick_millis: effect.amplitude,
                next_tick_at: now + Duration::from_millis(effect.amplitude as u64),
            })
        })
}

pub(in crate::world) fn spell_periodic_damage_fallback_caster_snapshot(
    caster_level: u8,
) -> SpellCombatUnitSnapshot {
    SpellCombatUnitSnapshot {
        level: caster_level.max(1),
        class: 0,
        intellect: 0,
        resistances: [0; MAX_SPELL_SCHOOL],
    }
}

pub(in crate::world) fn spell_periodic_regen_aura(
    spell_info: &SpellInfo<'_>,
    value_context: SpellEffectValueContext,
    now: Instant,
) -> Option<PeriodicRegenAura> {
    let mut health_amount = 0u32;
    let mut mana_amount = 0u32;
    let mut tick_millis = 0u32;
    for effect in spell_info.effects {
        if effect.dispatch != SpellEffectDispatch::ApplyAura {
            continue;
        }
        let Some(amount) = spell_effect_calculated_u32(effect, value_context) else {
            continue;
        };
        match effect.aura_name {
            SPELL_AURA_PERIODIC_HEAL | SPELL_AURA_OBS_MOD_HEALTH | SPELL_AURA_MOD_REGEN => {
                health_amount = health_amount.saturating_add(amount);
                tick_millis = tick_millis.max(effect.amplitude);
            }
            SPELL_AURA_PERIODIC_ENERGIZE | SPELL_AURA_MOD_POWER_REGEN
                if effect.misc_value == POWER_TYPE_MANA as i32 =>
            {
                mana_amount = mana_amount.saturating_add(amount);
                tick_millis = tick_millis.max(effect.amplitude);
            }
            _ => {}
        }
    }
    if health_amount == 0 && mana_amount == 0 {
        return None;
    }
    let tick_millis = tick_millis.max(2_000);
    Some(PeriodicRegenAura {
        health_amount,
        mana_amount,
        school_mask: spell_school_mask_from_school(spell_info.template.school),
        tick_millis,
        next_tick_at: now + Duration::from_millis(tick_millis as u64),
        interrupts_on_move_and_stand: false,
        suppresses_recent_damage: false,
        makes_player_sit: false,
    })
}

pub(in crate::world) fn mark_active_aura_periodic_regen_as_consumable(aura: &mut ActiveAura) {
    let Some(regen) = aura.periodic_regen.as_mut() else {
        return;
    };
    regen.interrupts_on_move_and_stand = true;
    regen.suppresses_recent_damage = true;
    regen.makes_player_sit = true;
}

pub(in crate::world) fn spell_is_mage_polymorph(template: &wow_db::SpellTemplateQuery) -> bool {
    template.spell_family_name == SPELL_FAMILY_MAGE
        && template.spell_family_flags & 0x0100_0000 != 0
        && SpellInfo::from_template(template)
            .effects
            .iter()
            .any(|effect| effect.aura_name == SPELL_AURA_MOD_CONFUSE)
}

pub(in crate::world) fn spell_is_priest_power_word_shield(
    template: &wow_db::SpellTemplateQuery,
) -> bool {
    template.spell_family_name == SPELL_FAMILY_PRIEST
        && template.spell_family_flags & 0x0000_0001 != 0
        && SpellInfo::from_template(template)
            .effects
            .iter()
            .any(|effect| effect.aura_name == SPELL_AURA_SCHOOL_ABSORB)
}

pub(in crate::world) fn spell_is_single_target_aura_template(
    template: &wow_db::SpellTemplateQuery,
) -> bool {
    match template.mechanic {
        MECHANIC_FEAR | MECHANIC_TURN => true,
        MECHANIC_ROOT | MECHANIC_SLEEP | MECHANIC_KNOCKOUT | MECHANIC_POLYMORPH
        | MECHANIC_BANISH | MECHANIC_SHACKLE => {
            template.spell_family_name != SPELL_FAMILY_GENERIC && template.spell_family_flags != 0
        }
        _ => {
            template.spell_family_name == SPELL_FAMILY_HUNTER
                && SpellInfo::from_template(template)
                    .effects
                    .iter()
                    .any(|effect| effect.aura_name == SPELL_AURA_MOD_STALKED)
        }
    }
}

pub(in crate::world) async fn single_target_aura_descriptor(
    object_mgr: &ObjectMgr,
    world_db_pool: &MySqlPool,
    template: &wow_db::SpellTemplateQuery,
) -> anyhow::Result<Option<SingleTargetAuraDescriptor>> {
    if !spell_is_single_target_aura_template(template) {
        return Ok(None);
    }
    let chain_root = object_mgr
        .spell_chain(world_db_pool, template.id)
        .await?
        .map(spell_chain_root)
        .unwrap_or(template.id);
    Ok(Some(SingleTargetAuraDescriptor {
        spell_id: template.id,
        chain_root,
        spell_family_name: template.spell_family_name,
        spell_family_flags: template.spell_family_flags,
        mechanic: template.mechanic,
    }))
}

pub(in crate::world) fn single_target_aura_descriptors_match(
    left: SingleTargetAuraDescriptor,
    right: SingleTargetAuraDescriptor,
) -> bool {
    left.spell_id == right.spell_id
        || (left.chain_root != 0 && left.chain_root == right.chain_root)
        || (left.spell_family_name != SPELL_FAMILY_GENERIC
            && right.spell_family_name != SPELL_FAMILY_GENERIC
            && left.spell_family_name == right.spell_family_name
            && left.spell_family_flags != 0
            && left.spell_family_flags == right.spell_family_flags)
}

pub(in crate::world) fn spell_diminishing_group(
    template: &wow_db::SpellTemplateQuery,
) -> Option<DiminishingGroupRuntime> {
    (template.mechanic == MECHANIC_POLYMORPH).then_some(DiminishingGroupRuntime::Polymorph)
}

pub(in crate::world) fn db_creature_spell_diminishing_group(
    template: &wow_db::SpellTemplateQuery,
) -> Option<DiminishingGroupRuntime> {
    // CMaNGOS classifies Polymorph as DRTYPE_PLAYER, so ordinary DB creatures
    // are not diminished through PvP DR levels.
    if template.mechanic == MECHANIC_POLYMORPH {
        None
    } else {
        spell_diminishing_group(template)
    }
}

pub(in crate::world) fn diminishing_duration_millis(
    duration_millis: Option<u32>,
    level: DiminishingLevelRuntime,
) -> Option<u32> {
    let duration = duration_millis?;
    Some(match level {
        DiminishingLevelRuntime::Level1 => duration,
        DiminishingLevelRuntime::Level2 => duration / 2,
        DiminishingLevelRuntime::Level3 => duration / 4,
        DiminishingLevelRuntime::Immune => 0,
    })
}

pub(in crate::world) fn spell_aura_proc_triggers(
    spell_info: &SpellInfo<'_>,
) -> Vec<AuraProcTrigger> {
    spell_info
        .effects
        .into_iter()
        .filter(|effect| {
            effect.dispatch == SpellEffectDispatch::ApplyAura
                && effect.aura_name == SPELL_AURA_PROC_TRIGGER_SPELL
                && effect.trigger_spell != 0
        })
        .map(|effect| AuraProcTrigger {
            triggered_spell_id: effect.trigger_spell,
            proc_flags: spell_info.template.proc_flags,
            proc_ex: 0,
            proc_chance: spell_info.template.proc_chance,
            remaining_charges: (spell_info.template.proc_charges > 0)
                .then_some(spell_info.template.proc_charges),
        })
        .collect()
}

pub(in crate::world) fn apply_spell_proc_event_to_active_aura(
    aura: &mut ActiveAura,
    spell_template: &wow_db::SpellTemplateQuery,
    spell_proc_event: Option<wow_db::SpellProcEventQuery>,
) {
    let overridden_chance = spell_proc_event.and_then(|spell_proc_event| {
        (spell_proc_event.custom_chance > 0.0)
            .then_some(spell_proc_event.custom_chance.round().clamp(0.0, 100.0) as u32)
    });
    if let Some(spell_proc_event) = spell_proc_event {
        for trigger in &mut aura.proc_triggers {
            trigger.proc_ex = spell_proc_event.proc_ex;
            if let Some(proc_chance) = overridden_chance {
                trigger.proc_chance = proc_chance;
            }
        }
    }
    if !aura.proc_triggers.is_empty()
        || spell_template.proc_flags == 0
        || spell_template.proc_charges == 0
    {
        return;
    }
    aura.proc_triggers.push(AuraProcTrigger {
        triggered_spell_id: 0,
        proc_flags: spell_template.proc_flags,
        proc_ex: spell_proc_event
            .map(|spell_proc_event| spell_proc_event.proc_ex)
            .unwrap_or(0),
        proc_chance: overridden_chance.unwrap_or(spell_template.proc_chance),
        remaining_charges: Some(spell_template.proc_charges),
    });
}

pub(in crate::world) fn active_aura_proc_trigger_spell_ids(
    active_auras: &mut [ActiveAura],
    proc_flag: u32,
    now: Instant,
) -> Vec<u32> {
    let mut triggered_spell_ids = Vec::new();
    for aura in active_auras
        .iter_mut()
        .filter(|aura| aura.expires_at.is_none_or(|expires_at| now < expires_at))
    {
        for trigger in &mut aura.proc_triggers {
            if trigger.proc_flags & proc_flag == 0 {
                continue;
            }
            if trigger.remaining_charges == Some(0) {
                continue;
            }
            if !aura_proc_roll_succeeds(trigger.proc_chance) {
                continue;
            }
            if let Some(remaining_charges) = trigger.remaining_charges.as_mut() {
                *remaining_charges = remaining_charges.saturating_sub(1);
            }
            triggered_spell_ids.push(trigger.triggered_spell_id);
        }
    }
    triggered_spell_ids
}

pub(in crate::world) fn consume_active_aura_proc_charges(
    active_auras: &mut Vec<ActiveAura>,
    proc_flag: u32,
    proc_ex: u32,
    now: Instant,
) -> bool {
    let mut aura_changed = false;
    for aura in active_auras
        .iter_mut()
        .filter(|aura| aura.expires_at.is_none_or(|expires_at| now < expires_at))
    {
        for trigger in &mut aura.proc_triggers {
            if trigger.triggered_spell_id != 0
                || trigger.proc_flags & proc_flag == 0
                || trigger.remaining_charges == Some(0)
            {
                continue;
            }
            if trigger.proc_ex != 0 && trigger.proc_ex & proc_ex == 0 {
                continue;
            }
            if !aura_proc_roll_succeeds(trigger.proc_chance) {
                continue;
            }
            if let Some(remaining_charges) = trigger.remaining_charges.as_mut() {
                *remaining_charges = remaining_charges.saturating_sub(1);
                aura_changed = true;
            }
        }
    }
    active_auras.retain(|aura| {
        let exhausted_charge_only = aura
            .proc_triggers
            .iter()
            .any(|trigger| trigger.triggered_spell_id == 0 && trigger.remaining_charges == Some(0));
        let has_trigger_spell_proc = aura
            .proc_triggers
            .iter()
            .any(|trigger| trigger.triggered_spell_id != 0);
        if exhausted_charge_only && !has_trigger_spell_proc {
            aura_changed = true;
            return false;
        }
        true
    });
    aura_changed
}

pub(in crate::world) fn aura_proc_roll_succeeds(proc_chance: u32) -> bool {
    if proc_chance >= 100 {
        return true;
    }
    if proc_chance == 0 {
        return false;
    }
    rand::thread_rng().gen_range(1..=10_000) <= proc_chance.saturating_mul(100).min(10_000)
}

pub(in crate::world) fn passive_spell_active_aura(
    template: &wow_db::SpellTemplateQuery,
    caster: ObjectGuid,
    level: u8,
    value_context: SpellEffectValueContext,
    now: Instant,
    duration: Option<SpellDurationEntry>,
) -> Option<ActiveAura> {
    if !spell_needs_passive_cast_at_learn(template) {
        return None;
    }
    let mut aura = build_active_aura(template, caster, level, value_context, now, duration);
    aura.visible = false;
    (!aura.stat_modifiers.is_empty()).then_some(aura)
}

pub(in crate::world) fn spell_needs_passive_cast_at_learn(
    template: &wow_db::SpellTemplateQuery,
) -> bool {
    SpellInfo::from_template(template).needs_passive_cast_at_learn()
}

pub(in crate::world) fn spell_aura_stat_modifiers(
    spell_info: &SpellInfo<'_>,
    value_context: SpellEffectValueContext,
) -> Vec<AuraStatModifier> {
    spell_info
        .effects
        .into_iter()
        .filter(|effect| effect.dispatch == SpellEffectDispatch::ApplyAura)
        .filter_map(|effect| match effect.aura_name {
            SPELL_AURA_MOD_SKILL | SPELL_AURA_MOD_SKILL_TALENT => {
                let skill_id = u16::try_from(effect.misc_value).ok()?;
                Some(AuraStatModifier::Skill {
                    skill_id,
                    amount: spell_effect_calculated_i32(effect, value_context)
                        .clamp(i16::MIN as i32, i16::MAX as i32) as i16,
                    permanent: effect.aura_name == SPELL_AURA_MOD_SKILL_TALENT,
                })
            }
            SPELL_AURA_MOD_BLOCK_PERCENT => Some(AuraStatModifier::BlockPercent {
                percent: spell_effect_calculated_i32(effect, value_context),
            }),
            SPELL_AURA_MOD_CRIT_PERCENT => Some(AuraStatModifier::CritPercent {
                percent: spell_effect_calculated_i32(effect, value_context),
            }),
            SPELL_AURA_MOD_ATTACK_POWER => Some(AuraStatModifier::AttackPower {
                amount: spell_effect_calculated_i32(effect, value_context),
            }),
            SPELL_AURA_MOD_DAMAGE_DONE => Some(AuraStatModifier::DamageDone {
                school_mask: spell_school_mask_from_misc_value(effect.misc_value),
                amount: spell_effect_calculated_i32(effect, value_context),
            }),
            SPELL_AURA_MOD_DAMAGE_TAKEN => Some(AuraStatModifier::DamageTaken {
                school_mask: spell_school_mask_from_misc_value(effect.misc_value),
                amount: spell_effect_calculated_i32(effect, value_context),
            }),
            SPELL_AURA_MOD_THREAT => Some(AuraStatModifier::ThreatPercent {
                school_mask: spell_school_mask_from_misc_value(effect.misc_value),
                percent: spell_effect_calculated_i32(effect, value_context),
            }),
            SPELL_AURA_MOD_TOTAL_THREAT => Some(AuraStatModifier::TotalThreat {
                amount: spell_effect_calculated_i32(effect, value_context),
            }),
            SPELL_AURA_MOD_TAUNT => Some(AuraStatModifier::Taunt),
            SPELL_AURA_MOD_DAMAGE_PERCENT_DONE => Some(AuraStatModifier::DamageDonePercent {
                school_mask: spell_school_mask_from_misc_value(effect.misc_value),
                percent: spell_effect_calculated_i32(effect, value_context),
            }),
            SPELL_AURA_MOD_DAMAGE_PERCENT_TAKEN => Some(AuraStatModifier::DamageTakenPercent {
                school_mask: spell_school_mask_from_misc_value(effect.misc_value),
                percent: spell_effect_calculated_i32(effect, value_context),
            }),
            SPELL_AURA_MOD_INCREASE_SPEED => Some(AuraStatModifier::MoveSpeedPercent {
                percent: spell_effect_calculated_i32(effect, value_context),
            }),
            SPELL_AURA_MOD_DECREASE_SPEED => Some(AuraStatModifier::MoveSpeedPercent {
                percent: spell_effect_calculated_i32(effect, value_context),
            }),
            SPELL_AURA_MOD_MELEE_HASTE => Some(AuraStatModifier::MeleeAttackTimePercent {
                percent: spell_effect_calculated_i32(effect, value_context),
            }),
            SPELL_AURA_MOD_SHAPESHIFT => u8::try_from(effect.misc_value)
                .ok()
                .map(|form| AuraStatModifier::Shapeshift { form }),
            SPELL_AURA_MOD_RESISTANCE => Some(AuraStatModifier::Resistance {
                school_mask: spell_school_mask_from_misc_value(effect.misc_value),
                amount: spell_effect_calculated_i32(effect, value_context),
            }),
            SPELL_AURA_MOD_RESISTANCE_PCT => Some(AuraStatModifier::ResistancePercent {
                school_mask: spell_school_mask_from_misc_value(effect.misc_value),
                percent: spell_effect_calculated_i32(effect, value_context),
            }),
            SPELL_AURA_MOD_HEALING_DONE => Some(AuraStatModifier::HealingDone {
                school_mask: spell_school_mask_from_misc_value(effect.misc_value),
                amount: spell_effect_calculated_i32(effect, value_context),
            }),
            SPELL_AURA_MOD_HEALING => Some(AuraStatModifier::HealingTaken {
                school_mask: spell_school_mask_from_misc_value(effect.misc_value),
                amount: spell_effect_calculated_i32(effect, value_context),
            }),
            SPELL_AURA_MOD_ROOT => Some(AuraStatModifier::Root),
            SPELL_AURA_MOD_STUN => Some(AuraStatModifier::Stun),
            SPELL_AURA_MOD_CONFUSE => Some(AuraStatModifier::Confuse),
            SPELL_AURA_MOD_FEAR => Some(AuraStatModifier::Fear),
            SPELL_AURA_MECHANIC_IMMUNITY => Some(AuraStatModifier::MechanicImmunity {
                mechanic: effect.misc_value.max(0) as u32,
            }),
            SPELL_AURA_MOD_DISARM => Some(AuraStatModifier::Disarm),
            SPELL_AURA_TRANSFORM => {
                let display_id = spell_effect_calculated_u32(effect, value_context).unwrap_or(0);
                Some(AuraStatModifier::Transform {
                    display_id,
                    creature_entry: u32::try_from(effect.misc_value).unwrap_or(0),
                })
            }
            SPELL_AURA_MOD_PACIFY => Some(AuraStatModifier::Pacify),
            SPELL_AURA_MOD_SILENCE => Some(AuraStatModifier::Silence),
            SPELL_AURA_MOD_PACIFY_SILENCE => Some(AuraStatModifier::PacifySilence),
            SPELL_AURA_FEATHER_FALL => Some(AuraStatModifier::FeatherFall),
            SPELL_AURA_HOVER => Some(AuraStatModifier::Hover),
            SPELL_AURA_SCHOOL_ABSORB => Some(AuraStatModifier::SchoolAbsorb {
                school_mask: spell_school_mask_from_misc_value(effect.misc_value),
                amount: spell_effect_calculated_i32(effect, value_context)
                    + spell_effect_healing_done_bonus(effect, value_context),
            }),
            SPELL_AURA_REFLECT_SPELLS_SCHOOL => Some(AuraStatModifier::ReflectSpellsSchool {
                school_mask: spell_school_mask_from_misc_value(effect.misc_value),
                percent: spell_effect_calculated_i32(effect, value_context),
            }),
            SPELL_AURA_MANA_SHIELD => Some(AuraStatModifier::ManaShield {
                school_mask: spell_school_mask_from_misc_value(effect.misc_value),
                amount: spell_effect_calculated_i32(effect, value_context),
                mana_multiplier_millis: (effect.multiple_value.max(0.0) * 1000.0).round() as u32,
            }),
            SPELL_AURA_MOD_POWER_REGEN_PERCENT => Some(AuraStatModifier::PowerRegenPercent {
                power_type: effect.misc_value.max(0) as u32,
                percent: spell_effect_calculated_i32(effect, value_context),
            }),
            SPELL_AURA_MOD_MANA_REGEN_INTERRUPT => {
                Some(AuraStatModifier::ManaRegenInterruptPercent {
                    percent: spell_effect_calculated_i32(effect, value_context),
                })
            }
            SPELL_AURA_MOD_STAT => {
                let stat = usize::try_from(effect.misc_value).ok();
                Some(AuraStatModifier::Stat {
                    stat: stat.filter(|stat| *stat < MAX_STATS),
                    amount: spell_effect_calculated_i32(effect, value_context),
                })
            }
            SPELL_AURA_MOD_TOTAL_STAT_PERCENTAGE => {
                let stat = usize::try_from(effect.misc_value).ok()?;
                (stat < MAX_STATS).then_some(AuraStatModifier::TotalStatPercent {
                    stat,
                    percent: spell_effect_calculated_i32(effect, value_context),
                })
            }
            SPELL_AURA_MOD_REPUTATION_GAIN => Some(AuraStatModifier::ReputationGainPercent {
                percent: spell_effect_calculated_i32(effect, value_context),
            }),
            SPELL_AURA_MOD_STEALTH_DETECT => Some(AuraStatModifier::StealthDetect {
                kind: effect.misc_value,
                amount: spell_effect_calculated_i32(effect, value_context),
            }),
            SPELL_AURA_MOD_INVISIBILITY_DETECTION => Some(AuraStatModifier::InvisibilityDetect {
                kind: effect.misc_value,
                amount: spell_effect_calculated_i32(effect, value_context),
            }),
            SPELL_AURA_TRACK_CREATURES => Some(AuraStatModifier::TrackCreatures {
                creature_type: effect.misc_value,
            }),
            SPELL_AURA_TRACK_RESOURCES => Some(AuraStatModifier::TrackResources {
                resource_type: effect.misc_value,
            }),
            SPELL_AURA_GHOST => Some(AuraStatModifier::Ghost),
            SPELL_AURA_WATER_WALK => Some(AuraStatModifier::WaterWalk),
            SPELL_AURA_DUMMY => Some(AuraStatModifier::Dummy {
                aura_name: effect.aura_name,
                misc_value: effect.misc_value,
                amount: spell_effect_calculated_i32(effect, value_context),
            }),
            _ => None,
        })
        .chain(
            (spell_info.template.dispel > 0).then_some(AuraStatModifier::DispelType {
                dispel_type: spell_info.template.dispel,
            }),
        )
        .collect()
}

pub(in crate::world) fn spell_school_mask_from_misc_value(misc_value: i32) -> u32 {
    if misc_value < 0 {
        u32::MAX
    } else {
        misc_value as u32
    }
}

pub(in crate::world) fn spell_school_mask_from_school(school: u32) -> u32 {
    if school < MAX_SPELL_SCHOOL as u32 {
        1u32 << school
    } else {
        school
    }
}

pub(in crate::world) fn active_aura_skill_bonus(active_auras: &[ActiveAura], skill_id: u16) -> i16 {
    active_auras
        .iter()
        .flat_map(|aura| aura.stat_modifiers.iter())
        .filter_map(|modifier| match modifier {
            AuraStatModifier::Skill {
                skill_id: modifier_skill,
                amount,
                ..
            } if *modifier_skill == skill_id => Some(*amount),
            _ => None,
        })
        .sum()
}

pub(in crate::world) fn active_aura_skill_bonus_pair(
    active_auras: &[ActiveAura],
    skill_id: u16,
) -> u32 {
    let mut temporary = 0i32;
    let mut permanent = 0i32;
    for modifier in active_auras
        .iter()
        .flat_map(|aura| aura.stat_modifiers.iter())
    {
        if let AuraStatModifier::Skill {
            skill_id: modifier_skill,
            amount,
            permanent: is_permanent,
        } = modifier
        {
            if *modifier_skill == skill_id {
                if *is_permanent {
                    permanent += i32::from(*amount);
                } else {
                    temporary += i32::from(*amount);
                }
            }
        }
    }
    make_pair32(
        temporary.clamp(i16::MIN as i32, i16::MAX as i32) as i16 as u16,
        permanent.clamp(i16::MIN as i32, i16::MAX as i32) as i16 as u16,
    )
}

pub(in crate::world) fn current_skill_value_with_active_auras(
    character_skills: &[CharacterSkill],
    active_auras: &[ActiveAura],
    skill_id: u16,
) -> u16 {
    let value = i32::from(current_skill_value(character_skills, skill_id));
    let bonus = i32::from(active_aura_skill_bonus(active_auras, skill_id));
    value.saturating_add(bonus).clamp(0, u16::MAX as i32) as u16
}

pub(in crate::world) fn reputation_gain_percent_from_active_auras(
    active_auras: &[ActiveAura],
) -> i32 {
    active_auras
        .iter()
        .flat_map(|aura| aura.stat_modifiers.iter())
        .filter_map(|modifier| match modifier {
            AuraStatModifier::ReputationGainPercent { percent } => Some(*percent),
            _ => None,
        })
        .sum()
}

pub(in crate::world) fn active_aura_has_root(active_auras: &[ActiveAura]) -> bool {
    active_auras
        .iter()
        .flat_map(|aura| aura.stat_modifiers.iter())
        .any(|modifier| *modifier == AuraStatModifier::Root)
}

pub(in crate::world) fn active_aura_taunt_caster(
    active_auras: &[ActiveAura],
) -> Option<ObjectGuid> {
    active_auras.iter().rev().find_map(|aura| {
        aura.stat_modifiers
            .iter()
            .any(|modifier| *modifier == AuraStatModifier::Taunt)
            .then_some(aura.caster)
    })
}

pub(in crate::world) fn active_aura_has_stun(active_auras: &[ActiveAura]) -> bool {
    active_auras
        .iter()
        .flat_map(|aura| aura.stat_modifiers.iter())
        .any(|modifier| *modifier == AuraStatModifier::Stun)
}

pub(in crate::world) fn active_aura_has_disarm(active_auras: &[ActiveAura]) -> bool {
    active_auras
        .iter()
        .flat_map(|aura| aura.stat_modifiers.iter())
        .any(|modifier| *modifier == AuraStatModifier::Disarm)
}

pub(in crate::world) fn active_aura_has_confuse(active_auras: &[ActiveAura]) -> bool {
    active_auras
        .iter()
        .flat_map(|aura| aura.stat_modifiers.iter())
        .any(|modifier| *modifier == AuraStatModifier::Confuse)
}

pub(in crate::world) fn aura_mechanic_immunities(aura: &ActiveAura) -> Vec<u32> {
    let mut mechanics = Vec::new();
    for modifier in &aura.stat_modifiers {
        let AuraStatModifier::MechanicImmunity { mechanic } = modifier else {
            continue;
        };
        if *mechanic != 0 && !mechanics.contains(mechanic) {
            mechanics.push(*mechanic);
        }
    }
    mechanics
}

pub(in crate::world) fn active_aura_has_mechanic_immunity(
    active_auras: &[ActiveAura],
    mechanic: u32,
) -> bool {
    mechanic != 0
        && active_auras
            .iter()
            .flat_map(|aura| aura.stat_modifiers.iter())
            .any(|modifier| {
                matches!(
                    modifier,
                    AuraStatModifier::MechanicImmunity {
                        mechanic: immunity_mechanic,
                    } if *immunity_mechanic == mechanic
                )
            })
}

pub(in crate::world) fn spell_template_mechanics(
    template: &wow_db::SpellTemplateQuery,
) -> Vec<u32> {
    let mut mechanics = Vec::new();
    for mechanic in [
        template.mechanic,
        template.effect_mechanic1,
        template.effect_mechanic2,
        template.effect_mechanic3,
    ] {
        if mechanic != 0 && !mechanics.contains(&mechanic) {
            mechanics.push(mechanic);
        }
    }
    mechanics
}

pub(in crate::world) fn spell_template_has_mechanic(
    template: &wow_db::SpellTemplateQuery,
    mechanic: u32,
) -> bool {
    mechanic != 0 && spell_template_mechanics(template).contains(&mechanic)
}

pub(in crate::world) fn active_auras_are_immune_to_spell(
    active_auras: &[ActiveAura],
    template: &wow_db::SpellTemplateQuery,
) -> bool {
    spell_template_mechanics(template)
        .into_iter()
        .any(|mechanic| active_aura_has_mechanic_immunity(active_auras, mechanic))
}

pub(in crate::world) fn spell_template_is_reflectable(
    template: &wow_db::SpellTemplateQuery,
) -> bool {
    template.dmg_class == SPELL_DAMAGE_CLASS_MAGIC
        && template.attributes & SPELL_ATTR_IS_ABILITY == 0
        && template.attributes_ex & SPELL_ATTR_EX_NO_REFLECTION == 0
        && template.attributes & SPELL_ATTR_NO_IMMUNITIES == 0
        && template.attributes & SPELL_ATTR_PASSIVE == 0
}

pub(in crate::world) fn active_aura_spell_reflect_chance(
    active_auras: &[ActiveAura],
    school_mask: u32,
) -> u32 {
    active_auras
        .iter()
        .flat_map(|aura| aura.stat_modifiers.iter())
        .filter_map(|modifier| match modifier {
            AuraStatModifier::ReflectSpellsSchool {
                school_mask: modifier_mask,
                percent,
            } if aura_school_mask_matches(*modifier_mask, school_mask) => Some(*percent),
            _ => None,
        })
        .sum::<i32>()
        .clamp(0, 100) as u32
}

pub(in crate::world) fn active_aura_spell_healing_done_bonus(
    active_auras: &[ActiveAura],
    school_mask: u32,
) -> i32 {
    active_auras
        .iter()
        .flat_map(|aura| aura.stat_modifiers.iter())
        .filter_map(|modifier| match modifier {
            AuraStatModifier::HealingDone {
                school_mask: modifier_mask,
                amount,
            } if aura_school_mask_matches(*modifier_mask, school_mask) => Some(*amount),
            _ => None,
        })
        .sum()
}

pub(in crate::world) fn active_auras_reflect_spell_school(
    active_auras: &[ActiveAura],
    school_mask: u32,
) -> bool {
    let reflect_chance = active_aura_spell_reflect_chance(active_auras, school_mask);
    reflect_chance > 0 && aura_proc_roll_succeeds(reflect_chance)
}

pub(in crate::world) async fn mechanic_immunity_purge_spell_ids(
    object_mgr: &ObjectMgr,
    world_db_pool: &MySqlPool,
    spell_template: &wow_db::SpellTemplateQuery,
    active_auras: &[ActiveAura],
    aura: &ActiveAura,
) -> anyhow::Result<Vec<u32>> {
    if spell_template.attributes_ex & SPELL_ATTR_EX_IMMUNITY_PURGES_EFFECT == 0 {
        return Ok(Vec::new());
    }
    let immune_mechanics = aura_mechanic_immunities(aura);
    if immune_mechanics.is_empty() {
        return Ok(Vec::new());
    }
    let mut spell_ids = Vec::new();
    for existing in active_auras {
        let Some(existing_template) = object_mgr
            .spell_template(world_db_pool, existing.spell_id)
            .await?
        else {
            continue;
        };
        if immune_mechanics
            .iter()
            .copied()
            .any(|mechanic| spell_template_has_mechanic(&existing_template, mechanic))
        {
            push_unique_spell_id(&mut spell_ids, existing.spell_id);
        }
    }
    Ok(spell_ids)
}

pub(in crate::world) fn active_aura_has_hard_control(active_auras: &[ActiveAura]) -> bool {
    active_auras
        .iter()
        .flat_map(|aura| aura.stat_modifiers.iter())
        .any(|modifier| {
            matches!(
                modifier,
                AuraStatModifier::Stun
                    | AuraStatModifier::Confuse
                    | AuraStatModifier::Fear
                    | AuraStatModifier::Pacify
                    | AuraStatModifier::PacifySilence
            )
        })
}

pub(in crate::world) fn active_aura_player_spell_cast_failure(
    active_auras: &[ActiveAura],
    spell_template: Option<&wow_db::SpellTemplateQuery>,
    spell_profile: &SpellCastProfile,
    requires_main_hand_weapon: bool,
) -> Option<u8> {
    if active_aura_has_modifier(active_auras, |modifier| *modifier == AuraStatModifier::Stun) {
        return Some(SPELL_FAILED_STUNNED);
    }
    if requires_main_hand_weapon && active_aura_has_disarm(active_auras) {
        return Some(SPELL_FAILED_EQUIPPED_ITEM_CLASS_MAINHAND);
    }
    if spell_cast_is_silence_prevented(spell_profile)
        && active_aura_has_modifier(active_auras, |modifier| {
            matches!(
                modifier,
                AuraStatModifier::Silence | AuraStatModifier::PacifySilence
            )
        })
    {
        return Some(SPELL_FAILED_SILENCED);
    }
    if spell_cast_is_pacify_prevented(spell_profile)
        && active_aura_has_modifier(active_auras, |modifier| {
            matches!(
                modifier,
                AuraStatModifier::Pacify | AuraStatModifier::PacifySilence
            )
        })
    {
        return Some(SPELL_FAILED_PACIFIED);
    }
    if active_aura_has_modifier(active_auras, |modifier| *modifier == AuraStatModifier::Fear)
        && !spell_template.is_some_and(|template| {
            spell_template_purges_mechanic_on_apply(template, MECHANIC_FEAR)
        })
    {
        return Some(SPELL_FAILED_FLEEING);
    }
    if active_aura_has_modifier(active_auras, |modifier| {
        *modifier == AuraStatModifier::Confuse
    }) {
        return Some(SPELL_FAILED_CONFUSED);
    }
    None
}

fn spell_template_purges_mechanic_on_apply(
    spell_template: &wow_db::SpellTemplateQuery,
    mechanic: u32,
) -> bool {
    if spell_template.attributes_ex & SPELL_ATTR_EX_IMMUNITY_PURGES_EFFECT == 0 {
        return false;
    }
    SpellInfo::from_template(spell_template)
        .effects
        .into_iter()
        .any(|effect| {
            effect.aura_name == SPELL_AURA_MECHANIC_IMMUNITY
                && u32::try_from(effect.misc_value).ok() == Some(mechanic)
        })
}

pub(in crate::world) fn active_aura_existing_player_spell_interrupt_failure(
    active_auras: &[ActiveAura],
    spell_profile: &SpellCastProfile,
) -> Option<u8> {
    if active_aura_has_modifier(active_auras, |modifier| *modifier == AuraStatModifier::Stun) {
        return Some(SPELL_FAILED_STUNNED);
    }
    if spell_cast_is_silence_prevented(spell_profile)
        && active_aura_has_modifier(active_auras, |modifier| {
            matches!(
                modifier,
                AuraStatModifier::Silence | AuraStatModifier::PacifySilence
            )
        })
    {
        return Some(SPELL_FAILED_SILENCED);
    }
    if active_aura_has_modifier(active_auras, |modifier| *modifier == AuraStatModifier::Fear) {
        return Some(SPELL_FAILED_FLEEING);
    }
    if active_aura_has_modifier(active_auras, |modifier| {
        *modifier == AuraStatModifier::Confuse
    }) {
        return Some(SPELL_FAILED_CONFUSED);
    }
    None
}

pub(in crate::world) fn active_aura_creature_spell_cast_failure(
    active_auras: &[ActiveAura],
) -> Option<u8> {
    if active_aura_has_modifier(active_auras, |modifier| *modifier == AuraStatModifier::Stun) {
        return Some(SPELL_FAILED_STUNNED);
    }
    if active_aura_has_modifier(active_auras, |modifier| {
        matches!(
            modifier,
            AuraStatModifier::Silence | AuraStatModifier::PacifySilence
        )
    }) {
        return Some(SPELL_FAILED_SILENCED);
    }
    if active_aura_has_modifier(active_auras, |modifier| *modifier == AuraStatModifier::Fear) {
        return Some(SPELL_FAILED_FLEEING);
    }
    if active_aura_has_modifier(active_auras, |modifier| {
        *modifier == AuraStatModifier::Confuse
    }) {
        return Some(SPELL_FAILED_CONFUSED);
    }
    None
}

fn active_aura_has_modifier(
    active_auras: &[ActiveAura],
    predicate: impl Fn(&AuraStatModifier) -> bool,
) -> bool {
    active_auras
        .iter()
        .flat_map(|aura| aura.stat_modifiers.iter())
        .any(predicate)
}

fn active_aura_is_shapeshift(active_aura: &ActiveAura) -> bool {
    active_aura
        .stat_modifiers
        .iter()
        .any(|modifier| matches!(modifier, AuraStatModifier::Shapeshift { .. }))
}

fn spell_cast_is_silence_prevented(spell_profile: &SpellCastProfile) -> bool {
    !matches!(
        spell_profile.kind,
        SpellCastKind::AutoRepeatRanged | SpellCastKind::Charge | SpellCastKind::NextMeleeSwing
    )
}

fn spell_cast_is_pacify_prevented(spell_profile: &SpellCastProfile) -> bool {
    spell_profile.requires_melee
        || matches!(
            spell_profile.kind,
            SpellCastKind::AutoRepeatRanged | SpellCastKind::Charge | SpellCastKind::NextMeleeSwing
        )
}

pub(in crate::world) fn active_aura_dispel_type(active_aura: &ActiveAura) -> Option<u32> {
    active_aura
        .stat_modifiers
        .iter()
        .find_map(|modifier| match modifier {
            AuraStatModifier::DispelType { dispel_type } => Some(*dispel_type),
            _ => None,
        })
}

pub(in crate::world) fn active_aura_matches_dispel_type(
    active_aura: &ActiveAura,
    dispel_type: u32,
) -> bool {
    active_aura_dispel_type(active_aura).is_some_and(|aura_dispel_type| {
        dispel_type == DISPEL_ALL || aura_dispel_type == dispel_type
    })
}

pub(in crate::world) async fn active_aura_spell_ids_with_mechanic(
    object_mgr: &ObjectMgr,
    world_db_pool: &MySqlPool,
    active_auras: &[ActiveAura],
    mechanic: u32,
    count: u32,
) -> anyhow::Result<Vec<u32>> {
    if mechanic == 0 {
        return Ok(Vec::new());
    }

    let mut remaining = count.max(1) as usize;
    let mut spell_ids = Vec::new();
    for aura in active_auras {
        if remaining == 0 {
            break;
        }

        let Some(template) = object_mgr
            .spell_template(world_db_pool, aura.spell_id)
            .await?
        else {
            continue;
        };

        if spell_template_has_mechanic(&template, mechanic) {
            spell_ids.push(aura.spell_id);
            remaining -= 1;
        }
    }

    Ok(spell_ids)
}

pub(in crate::world) fn remove_active_auras_by_spell_ids(
    active_auras: &mut Vec<ActiveAura>,
    spell_ids: &[u32],
) -> Vec<u32> {
    let mut remaining = spell_ids.to_vec();
    let mut removed = Vec::new();

    active_auras.retain(|aura| {
        let Some(index) = remaining
            .iter()
            .position(|spell_id| *spell_id == aura.spell_id)
        else {
            return true;
        };

        removed.push(aura.spell_id);
        remaining.remove(index);
        false
    });

    removed
}

pub(in crate::world) fn active_aura_shapeshift_form(active_auras: &[ActiveAura]) -> Option<u8> {
    active_auras.iter().rev().find_map(|aura| {
        aura.stat_modifiers
            .iter()
            .find_map(|modifier| match modifier {
                AuraStatModifier::Shapeshift { form } => Some(*form),
                _ => None,
            })
    })
}

pub(in crate::world) fn active_aura_blocks_movement(active_auras: &[ActiveAura]) -> bool {
    active_aura_has_root(active_auras) || active_aura_has_stun(active_auras)
}

pub(in crate::world) fn active_aura_transform_display_id(
    active_auras: &[ActiveAura],
) -> Option<u32> {
    active_auras.iter().rev().find_map(|aura| {
        aura.stat_modifiers
            .iter()
            .find_map(|modifier| match modifier {
                AuraStatModifier::Transform { display_id, .. } if *display_id != 0 => {
                    Some(*display_id)
                }
                _ => None,
            })
    })
}

pub(in crate::world) fn active_aura_breaks_on_damage(active_aura: &ActiveAura) -> bool {
    active_aura.interrupt_flags & AURA_INTERRUPT_FLAG_DAMAGE != 0
        && active_aura.stat_modifiers.iter().any(|modifier| {
            matches!(
                modifier,
                AuraStatModifier::Confuse
                    | AuraStatModifier::Stun
                    | AuraStatModifier::Transform { .. }
            )
        })
}

pub(in crate::world) fn active_aura_suppresses_hostile_refs(active_aura: &ActiveAura) -> bool {
    active_aura
        .stat_modifiers
        .iter()
        .any(|modifier| matches!(modifier, AuraStatModifier::Confuse | AuraStatModifier::Fear))
        || (active_aura_breaks_on_damage(active_aura)
            && active_aura
                .stat_modifiers
                .iter()
                .any(|modifier| matches!(modifier, AuraStatModifier::Stun)))
}

pub(in crate::world) fn active_auras_suppress_hostile_refs(active_auras: &[ActiveAura]) -> bool {
    active_auras.iter().any(active_aura_suppresses_hostile_refs)
}

pub(in crate::world) fn active_aura_movement_speed_multiplier(active_auras: &[ActiveAura]) -> f32 {
    if active_aura_blocks_movement(active_auras) {
        return 0.0;
    }

    let strongest_slow = active_auras
        .iter()
        .flat_map(|aura| aura.stat_modifiers.iter())
        .filter_map(|modifier| match modifier {
            AuraStatModifier::MoveSpeedPercent { percent } if *percent < 0 => Some(*percent),
            _ => None,
        })
        .min()
        .unwrap_or(0);
    let strongest_increase = active_auras
        .iter()
        .flat_map(|aura| aura.stat_modifiers.iter())
        .filter_map(|modifier| match modifier {
            AuraStatModifier::MoveSpeedPercent { percent } if *percent > 0 => Some(*percent),
            _ => None,
        })
        .max()
        .unwrap_or(0);

    ((100 + strongest_slow).max(0) as f32 / 100.0)
        * ((100 + strongest_increase).max(1) as f32 / 100.0)
}

pub(in crate::world) fn active_aura_melee_attack_time_multiplier(
    active_auras: &[ActiveAura],
) -> f32 {
    active_auras
        .iter()
        .flat_map(|aura| aura.stat_modifiers.iter())
        .filter_map(|modifier| match modifier {
            AuraStatModifier::MeleeAttackTimePercent { percent } => Some(*percent),
            _ => None,
        })
        .fold(1.0, |multiplier, percent| {
            let effect = if percent >= 0 {
                (100 - percent).max(0) as f32 / 100.0
            } else {
                (100 + percent.saturating_abs()).max(0) as f32 / 100.0
            };
            multiplier * effect
        })
}

pub(in crate::world) fn active_aura_physical_damage_done(active_auras: &[ActiveAura]) -> i32 {
    active_auras
        .iter()
        .flat_map(|aura| aura.stat_modifiers.iter())
        .filter_map(|modifier| match modifier {
            AuraStatModifier::DamageDone {
                school_mask,
                amount,
            } if *school_mask == 0 || *school_mask & 1 != 0 => Some(*amount),
            _ => None,
        })
        .sum()
}

pub(in crate::world) fn active_aura_damage_done_multiplier(
    active_auras: &[ActiveAura],
    school_mask: u32,
) -> f32 {
    active_auras
        .iter()
        .flat_map(|aura| aura.stat_modifiers.iter())
        .filter_map(|modifier| match modifier {
            AuraStatModifier::DamageDonePercent {
                school_mask: modifier_mask,
                percent,
            } if aura_school_mask_matches(*modifier_mask, school_mask) => Some(*percent),
            _ => None,
        })
        .fold(1.0, |multiplier, percent| {
            multiplier * percent_modifier_multiplier(percent)
        })
}

pub(in crate::world) fn active_aura_damage_taken_multiplier(
    active_auras: &[ActiveAura],
    school_mask: u32,
) -> f32 {
    active_auras
        .iter()
        .flat_map(|aura| aura.stat_modifiers.iter())
        .filter_map(|modifier| match modifier {
            AuraStatModifier::DamageTakenPercent {
                school_mask: modifier_mask,
                percent,
            } if aura_school_mask_matches(*modifier_mask, school_mask) => Some(*percent),
            _ => None,
        })
        .fold(1.0, |multiplier, percent| {
            multiplier * percent_modifier_multiplier(percent)
        })
}

pub(in crate::world) fn active_aura_spell_damage_taken_bonus(
    active_auras: &[ActiveAura],
    school_mask: u32,
) -> i32 {
    active_auras
        .iter()
        .flat_map(|aura| aura.stat_modifiers.iter())
        .filter_map(|modifier| match modifier {
            AuraStatModifier::DamageTaken {
                school_mask: modifier_mask,
                amount,
            } if aura_school_mask_matches(*modifier_mask, school_mask) => Some(*amount),
            _ => None,
        })
        .sum()
}

pub(in crate::world) fn active_aura_spell_healing_taken_bonus(
    active_auras: &[ActiveAura],
    school_mask: u32,
) -> i32 {
    active_auras
        .iter()
        .flat_map(|aura| aura.stat_modifiers.iter())
        .filter_map(|modifier| match modifier {
            AuraStatModifier::HealingTaken {
                school_mask: modifier_mask,
                amount,
            } if aura_school_mask_matches(*modifier_mask, school_mask) => Some(*amount),
            _ => None,
        })
        .sum()
}

pub(in crate::world) fn spell_effect_healing_done_bonus(
    effect: SpellInfoEffect,
    context: SpellEffectValueContext,
) -> i32 {
    if effect.bonus_coefficient <= 0.0 || context.spell_healing_bonus_done <= 0 {
        return 0;
    }
    ((context.spell_healing_bonus_done as f32)
        * effect.bonus_coefficient
        * spell_level_penalty_multiplier(context.spell_level))
    .trunc() as i32
}

pub(in crate::world) fn spell_level_penalty_multiplier(spell_level: u32) -> f32 {
    if spell_level > 0 && spell_level < 20 {
        1.0 - ((20.0 - spell_level as f32) * 0.0375)
    } else {
        1.0
    }
}

pub(in crate::world) fn apply_flat_spell_bonus(amount: u32, bonus: i32) -> u32 {
    (i64::from(amount) + i64::from(bonus)).max(0) as u32
}

pub(in crate::world) fn active_aura_threat_multiplier(
    active_auras: &[ActiveAura],
    school_mask: u32,
) -> f32 {
    active_auras
        .iter()
        .flat_map(|aura| aura.stat_modifiers.iter())
        .filter_map(|modifier| match modifier {
            AuraStatModifier::ThreatPercent {
                school_mask: modifier_mask,
                percent,
            } if aura_school_mask_matches(*modifier_mask, school_mask) => Some(*percent),
            _ => None,
        })
        .fold(1.0, |multiplier, percent| {
            multiplier * percent_modifier_multiplier(percent)
        })
}

pub(in crate::world) fn active_aura_total_threat_amount(aura: &ActiveAura) -> i32 {
    aura.stat_modifiers
        .iter()
        .filter_map(|modifier| match modifier {
            AuraStatModifier::TotalThreat { amount } => Some(*amount),
            _ => None,
        })
        .sum()
}

fn aura_school_mask_matches(modifier_mask: u32, school_mask: u32) -> bool {
    modifier_mask == 0 || modifier_mask & school_mask != 0
}

fn percent_modifier_multiplier(percent: i32) -> f32 {
    (100 + percent).max(0) as f32 / 100.0
}

pub(in crate::world) fn active_aura_track_creatures_mask(active_auras: &[ActiveAura]) -> u32 {
    active_auras
        .iter()
        .flat_map(|aura| aura.stat_modifiers.iter())
        .filter_map(|modifier| match modifier {
            AuraStatModifier::TrackCreatures { creature_type } if *creature_type > 0 => {
                u32::try_from(*creature_type - 1)
                    .ok()
                    .filter(|bit| *bit < 32)
                    .map(|bit| 1u32 << bit)
            }
            _ => None,
        })
        .fold(0, |mask, flag| mask | flag)
}

pub(in crate::world) fn active_aura_track_resources_mask(active_auras: &[ActiveAura]) -> u32 {
    active_auras
        .iter()
        .flat_map(|aura| aura.stat_modifiers.iter())
        .filter_map(|modifier| match modifier {
            AuraStatModifier::TrackResources { resource_type } if *resource_type > 0 => {
                u32::try_from(*resource_type - 1)
                    .ok()
                    .filter(|bit| *bit < 32)
                    .map(|bit| 1u32 << bit)
            }
            _ => None,
        })
        .fold(0, |mask, flag| mask | flag)
}

pub(in crate::world) fn active_aura_unit_vis_flags(active_auras: &[ActiveAura]) -> u32 {
    active_auras
        .iter()
        .flat_map(|aura| aura.stat_modifiers.iter())
        .fold(0, |flags, modifier| match modifier {
            AuraStatModifier::Ghost => flags | UNIT_VIS_FLAG_GHOST,
            _ => flags,
        })
}

pub(in crate::world) fn player_world_stats_with_active_auras(
    mut world_stats: PlayerWorldStats,
    active_auras: &[ActiveAura],
) -> PlayerWorldStats {
    for modifier in active_auras
        .iter()
        .flat_map(|aura| aura.stat_modifiers.iter())
    {
        let AuraStatModifier::Stat { stat, amount } = modifier else {
            continue;
        };
        if let Some(stat) = stat {
            world_stats.stats[*stat] = apply_flat_modifier(world_stats.stats[*stat], *amount);
        } else {
            for stat_value in &mut world_stats.stats {
                *stat_value = apply_flat_modifier(*stat_value, *amount);
            }
        }
    }

    for stat in 0..MAX_STATS {
        let percent = active_auras
            .iter()
            .flat_map(|aura| aura.stat_modifiers.iter())
            .filter_map(|modifier| match modifier {
                AuraStatModifier::TotalStatPercent {
                    stat: modifier_stat,
                    percent,
                } if *modifier_stat == stat => Some(*percent),
                _ => None,
            })
            .sum::<i32>();
        if percent != 0 {
            world_stats.stats[stat] = apply_percent_modifier(world_stats.stats[stat], percent);
        }
    }
    world_stats
}

pub(in crate::world) fn player_stat_mod_deltas(
    base_world_stats: &PlayerWorldStats,
    effective_world_stats: &PlayerWorldStats,
) -> [i32; MAX_STATS] {
    let mut deltas = [0i32; MAX_STATS];
    for (offset, delta) in deltas.iter_mut().enumerate() {
        *delta = effective_world_stats.stats[offset] as i32 - base_world_stats.stats[offset] as i32;
    }
    deltas
}

pub(in crate::world) fn apply_flat_modifier(value: u32, amount: i32) -> u32 {
    (value as i64)
        .saturating_add(i64::from(amount))
        .clamp(0, u32::MAX as i64) as u32
}

pub(in crate::world) fn apply_percent_modifier(value: u32, percent: i32) -> u32 {
    let multiplier = 100i64.saturating_add(i64::from(percent));
    if multiplier <= 0 {
        return 0;
    }
    ((i64::from(value) * multiplier) / 100).clamp(0, u32::MAX as i64) as u32
}

pub(in crate::world) async fn consume_used_item(
    stream: &mut WorldPacketSink,
    character_db_pool: &MySqlPool,
    session: &mut WorldSessionState,
    character_guid: u32,
    source_item: &CharacterInventoryItem,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    let destroyed = wow_db::destroy_character_inventory_item_count(
        character_db_pool,
        character_guid,
        source_item.bag,
        source_item.slot,
        1,
    )
    .await?;
    let Some(destroyed) = destroyed else {
        return Ok(());
    };
    session.inventory.items =
        wow_db::get_character_inventory_items(character_db_pool, character_guid).await?;
    match destroyed {
        wow_db::InventoryDestroyResult::CountChanged { item, count } => {
            let body = build_item_stack_count_update_body(item, count)?;
            send_packet(
                stream,
                WorldOpcode::SmsgUpdateObject as u16,
                &body,
                Some(header_crypto),
            )
            .await?;
        }
        wow_db::InventoryDestroyResult::Removed { item } => {
            if source_item.bag == INVENTORY_SLOT_BAG_0 as u32 {
                let body = build_inventory_slots_update_body(
                    character_guid,
                    &session.inventory.items,
                    &[source_item.slot],
                )?;
                send_packet(
                    stream,
                    WorldOpcode::SmsgUpdateObject as u16,
                    &body,
                    Some(header_crypto),
                )
                .await?;
            } else {
                let body = build_destroy_object_body(item);
                send_packet(
                    stream,
                    WorldOpcode::SmsgDestroyObject as u16,
                    &body,
                    Some(header_crypto),
                )
                .await?;
            }
        }
    }
    Ok(())
}

pub(in crate::world) fn item_use_spell(
    template: &ItemTemplateQuery,
    requested_index: u8,
) -> Option<wow_db::ItemTemplateSpell> {
    let requested = template.spells.get(requested_index as usize).copied();
    requested
        .filter(|spell| is_item_use_spell(*spell))
        .or_else(|| {
            template
                .spells
                .into_iter()
                .find(|spell| is_item_use_spell(*spell))
        })
}

pub(in crate::world) fn is_item_use_spell(spell: wow_db::ItemTemplateSpell) -> bool {
    spell.spell_id != 0
        && matches!(
            spell.spell_trigger,
            ITEM_SPELLTRIGGER_ON_USE | ITEM_SPELLTRIGGER_ON_NO_DELAY_USE
        )
}

pub(in crate::world) fn apply_player_aura(session: &mut WorldSessionState, aura: ActiveAura) {
    apply_active_aura(&mut session.auras.active_auras, aura);
}

pub(in crate::world) fn apply_player_aura_replacing_conflicts(
    session: &mut WorldSessionState,
    aura: ActiveAura,
    resolution: &AuraRankConflictResolution,
) {
    apply_active_aura_replacing_conflicts(&mut session.auras.active_auras, aura, resolution);
}

pub(in crate::world) fn apply_active_aura(active_auras: &mut Vec<ActiveAura>, aura: ActiveAura) {
    if let Some(existing) = active_auras
        .iter_mut()
        .find(|existing| existing.spell_id == aura.spell_id && existing.caster == aura.caster)
    {
        *existing = aura;
    } else {
        active_auras.push(aura);
    }
}

pub(in crate::world) fn consumable_regen_conflicting_spell_ids(
    active_auras: &[ActiveAura],
    aura: &ActiveAura,
) -> Vec<u32> {
    let Some(new_regen) = aura.periodic_regen else {
        return Vec::new();
    };
    if !new_regen.makes_player_sit {
        return Vec::new();
    }
    let mut spell_ids = Vec::new();
    for existing in active_auras {
        let Some(existing_regen) = existing.periodic_regen else {
            continue;
        };
        if !existing_regen.makes_player_sit {
            continue;
        }
        let same_food = new_regen.health_amount > 0 && existing_regen.health_amount > 0;
        let same_drink = new_regen.mana_amount > 0 && existing_regen.mana_amount > 0;
        if same_food || same_drink {
            push_unique_spell_id(&mut spell_ids, existing.spell_id);
        }
    }
    spell_ids
}

#[cfg(test)]
pub(in crate::world) fn apply_consumable_regen_aura(
    active_auras: &mut Vec<ActiveAura>,
    aura: ActiveAura,
) {
    let replace_spell_ids = consumable_regen_conflicting_spell_ids(active_auras, &aura);
    if !replace_spell_ids.is_empty() {
        active_auras.retain(|existing| {
            existing.caster != aura.caster || !replace_spell_ids.contains(&existing.spell_id)
        });
    }
    apply_active_aura(active_auras, aura);
}

#[cfg(test)]
pub(in crate::world) fn apply_active_aura_replacing_spell_ids(
    active_auras: &mut Vec<ActiveAura>,
    aura: ActiveAura,
    replace_spell_ids: &[u32],
) {
    if !replace_spell_ids.is_empty() {
        active_auras.retain(|existing| {
            existing.caster != aura.caster || !replace_spell_ids.contains(&existing.spell_id)
        });
    }
    apply_active_aura(active_auras, aura);
}

pub(in crate::world) fn apply_active_aura_replacing_conflicts(
    active_auras: &mut Vec<ActiveAura>,
    aura: ActiveAura,
    resolution: &AuraRankConflictResolution,
) {
    let mut replace_any_caster_spell_ids = resolution.replace_any_caster_spell_ids.clone();
    if active_aura_is_shapeshift(&aura) {
        for existing in active_auras
            .iter()
            .filter(|existing| active_aura_is_shapeshift(existing))
        {
            push_unique_spell_id(&mut replace_any_caster_spell_ids, existing.spell_id);
        }
    }
    if !resolution.replace_spell_ids.is_empty() || !replace_any_caster_spell_ids.is_empty() {
        active_auras.retain(|existing| {
            !replace_any_caster_spell_ids.contains(&existing.spell_id)
                && (existing.caster != aura.caster
                    || !resolution.replace_spell_ids.contains(&existing.spell_id))
        });
    }
    if resolution.stack_limit > 1 {
        let mut matching = active_auras
            .iter_mut()
            .filter(|existing| existing.spell_id == aura.spell_id)
            .collect::<Vec<_>>();
        if !matching.is_empty() {
            for existing in &mut matching {
                existing.duration_millis = aura.duration_millis;
                existing.expires_at = aura.expires_at;
            }
            if matching.len() < usize::from(resolution.stack_limit) {
                active_auras.push(aura);
            }
            return;
        }
    }
    apply_active_aura(active_auras, aura);
}

pub(in crate::world) fn expire_session_auras(session: &mut WorldSessionState, now: Instant) {
    session
        .auras
        .active_auras
        .retain(|aura| aura.expires_at.is_none_or(|expires_at| now < expires_at));
}

pub(in crate::world) fn active_aura_interrupt_flags(aura: &ActiveAura) -> u32 {
    let derived = aura.periodic_regen.map_or(0, |regen| {
        if regen.interrupts_on_move_and_stand {
            AURA_INTERRUPT_FLAG_DAMAGE
                | AURA_INTERRUPT_FLAG_MOVING
                | AURA_INTERRUPT_FLAG_STANDING_CANCELS
        } else {
            0
        }
    });
    aura.interrupt_flags | derived
}

pub(in crate::world) fn remove_active_auras_with_interrupt_flag(
    active_auras: &mut Vec<ActiveAura>,
    interrupt_flag: u32,
) -> bool {
    let before = active_auras.len();
    active_auras.retain(|aura| active_aura_interrupt_flags(aura) & interrupt_flag == 0);
    active_auras.len() != before
}

pub(in crate::world) async fn interrupt_player_consumable_auras(
    stream: &mut WorldPacketSink,
    maps: &Arc<MapRuntimeManager>,
    sessions: &Arc<SessionRegistry>,
    session: &mut WorldSessionState,
    interrupt_flag: u32,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<bool> {
    if !remove_active_auras_with_interrupt_flag(&mut session.auras.active_auras, interrupt_flag) {
        return Ok(false);
    }
    session.character.player_stand_state = PLAYER_STAND_STATE_STAND;
    let Some(character) = session.character.active_character.as_ref() else {
        return Ok(true);
    };
    let map_id = character.position.map_id;
    let character_guid = character.guid;
    let player = ObjectGuid::new(HighGuid::Player, 0, character_guid);
    maps.remove_player_auras_with_interrupt_flag(map_id, character_guid, interrupt_flag)
        .await;
    let aura_packet = OutboundWorldPacket {
        opcode: WorldOpcode::SmsgUpdateObject as u16,
        body: build_player_aura_update_body(
            player,
            character.class,
            session.character.player_stand_state,
            maps.player_runtime_snapshot(map_id, character_guid)
                .await
                .map(|snapshot| snapshot.aura_state)
                .unwrap_or(0),
            &session.auras.active_auras,
        )?,
    };
    let stand_packet = OutboundWorldPacket {
        opcode: WorldOpcode::SmsgUpdateObject as u16,
        body: build_player_stand_state_update_body(
            character,
            session.character.player_stand_state,
        )?,
    };
    send_packet(
        stream,
        aura_packet.opcode,
        &aura_packet.body,
        Some(&mut *header_crypto),
    )
    .await?;
    send_packet(
        stream,
        stand_packet.opcode,
        &stand_packet.body,
        Some(header_crypto),
    )
    .await?;
    maps.sync_player_gameplay_state(map_id, character_guid, session)
        .await;
    let mut observer_packets = maps
        .broadcast_nearby_player_packet(
            map_id,
            character_guid,
            PLAYER_VISIBILITY_RADIUS_YARDS,
            aura_packet,
        )
        .await;
    observer_packets.extend(
        maps.broadcast_nearby_player_packet(
            map_id,
            character_guid,
            PLAYER_VISIBILITY_RADIUS_YARDS,
            stand_packet,
        )
        .await,
    );
    sessions.dispatch(observer_packets).await;
    Ok(true)
}

pub(in crate::world) fn build_player_aura_update_body(
    player: ObjectGuid,
    class: u8,
    stand_state: u8,
    aura_state: u32,
    active_auras: &[ActiveAura],
) -> anyhow::Result<Vec<u8>> {
    let mut block = Vec::new();
    block.push(UPDATE_TYPE_VALUES);
    PackedGuid::write(&mut block, player)?;

    let mut values = vec![None; PLAYER_END_FIELDS];
    set_player_aura_update_values(&mut values, class, stand_state, aura_state, active_auras)?;

    write_update_values(&mut block, &values)?;
    Ok(build_update_object_body(&[block]))
}

pub(in crate::world) fn set_player_aura_update_values(
    values: &mut [Option<u32>],
    class: u8,
    stand_state: u8,
    aura_state: u32,
    active_auras: &[ActiveAura],
) -> anyhow::Result<()> {
    set_unit_aura_update_values(values, active_auras)?;
    set_update_value(values, UNIT_FIELD_AURASTATE, aura_state)?;
    set_update_value(
        values,
        UNIT_FIELD_BYTES_1,
        player_unit_bytes_1_with_auras(class, stand_state, active_auras),
    )?;
    set_update_value(
        values,
        PLAYER_TRACK_CREATURES,
        active_aura_track_creatures_mask(active_auras),
    )?;
    set_update_value(
        values,
        PLAYER_TRACK_RESOURCES,
        active_aura_track_resources_mask(active_auras),
    )?;
    Ok(())
}

pub(in crate::world) fn set_unit_aura_update_values(
    values: &mut [Option<u32>],
    active_auras: &[ActiveAura],
) -> anyhow::Result<()> {
    for slot in 0..MAX_AURA_SLOTS {
        set_update_value(values, UNIT_FIELD_AURA + slot, 0)?;
    }
    for field in 0..MAX_AURA_FLAG_FIELDS {
        set_update_value(values, UNIT_FIELD_AURAFLAGS + field, 0)?;
    }
    for field in 0..MAX_AURA_LEVEL_FIELDS {
        set_update_value(values, UNIT_FIELD_AURALEVELS + field, 0)?;
        set_update_value(values, UNIT_FIELD_AURAAPPLICATIONS + field, 0)?;
    }

    for visible in visible_aura_slots(active_auras) {
        let slot = visible.slot;
        let aura = visible.aura;
        set_update_value(values, UNIT_FIELD_AURA + slot, aura.spell_id)?;
        let flags_index = UNIT_FIELD_AURAFLAGS + (slot / 8);
        let flags_shift = ((slot % 8) * 4) as u32;
        let previous = values[flags_index].unwrap_or(0);
        let flags = if aura.positive {
            POSITIVE_AURA_FLAGS
        } else {
            NEGATIVE_AURA_FLAGS
        };
        set_update_value(values, flags_index, previous | (flags << flags_shift))?;

        let level_index = UNIT_FIELD_AURALEVELS + (slot / 4);
        let level_shift = ((slot % 4) * 8) as u32;
        let previous = values[level_index].unwrap_or(0);
        set_update_value(
            values,
            level_index,
            previous | ((aura.level.max(1) as u32) << level_shift),
        )?;

        let applications_index = UNIT_FIELD_AURAAPPLICATIONS + (slot / 4);
        let applications_shift = ((slot % 4) * 8) as u32;
        let previous = values[applications_index].unwrap_or(0);
        set_update_value(
            values,
            applications_index,
            previous | (u32::from(visible.applications) << applications_shift),
        )?;
    }

    Ok(())
}

pub(in crate::world) async fn handle_item_query_single(
    stream: &mut WorldPacketSink,
    world_db_pool: &MySqlPool,
    request: wow_proto::ItemQuerySingleRequest,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    let item = request.item_id;
    let template = wow_db::get_item_template_query(world_db_pool, item).await?;
    let spell_cooldowns = if let Some(template) = template.as_ref() {
        Some(item_query_spell_cooldowns(world_db_pool, template).await?)
    } else {
        None
    };
    info!(
        item,
        found = template.is_some(),
        "Answering item template query"
    );
    let response = build_item_query_single_response_with_spell_cooldowns(
        item,
        template.as_ref(),
        spell_cooldowns.as_ref(),
    );
    send_packet(
        stream,
        WorldOpcode::SmsgItemQuerySingleResponse as u16,
        &response,
        Some(header_crypto),
    )
    .await
}

pub(in crate::world) async fn item_query_spell_cooldowns(
    world_db_pool: &MySqlPool,
    template: &wow_db::ItemTemplateQuery,
) -> anyhow::Result<[Option<ItemQuerySpellCooldown>; 5]> {
    let mut cooldowns = [None; 5];
    for (index, spell) in template.spells.iter().enumerate() {
        if spell.spell_id == 0 {
            continue;
        }
        let Some(spell_template) =
            wow_db::get_spell_template_query(world_db_pool, spell.spell_id).await?
        else {
            continue;
        };
        cooldowns[index] = Some(ItemQuerySpellCooldown {
            recovery_time: spell_template.recovery_time.min(i32::MAX as u32) as i32,
            category: spell_template.category,
            category_recovery_time: spell_template.category_recovery_time.min(i32::MAX as u32)
                as i32,
        });
    }
    Ok(cooldowns)
}

pub(in crate::world) async fn handle_item_name_query(
    stream: &mut WorldPacketSink,
    world_db_pool: &MySqlPool,
    request: wow_proto::ItemNameQueryRequest,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    let item = request.item_id;
    let Some(template) = wow_db::get_item_template_query(world_db_pool, item).await? else {
        warn!(item, "Ignoring item name query for unknown item");
        return Ok(());
    };
    let response = build_item_name_query_response(&template);
    send_packet(
        stream,
        WorldOpcode::SmsgItemNameQueryResponse as u16,
        &response,
        Some(header_crypto),
    )
    .await
}

pub(in crate::world) async fn handle_page_text_query(
    stream: &mut WorldPacketSink,
    world_db_pool: &MySqlPool,
    request: wow_proto::PageTextQueryRequest,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    let page_text = wow_db::get_page_text_query(world_db_pool, request.page_text_id).await?;
    let response = if let Some(page_text) = page_text {
        wow_proto::SmsgPageTextQueryResponse {
            page_text_id: page_text.id,
            text: page_text.text,
            next_page_text_id: page_text.next_page_text_id,
        }
    } else {
        warn!(
            page_text_id = request.page_text_id,
            item = format_args!("0x{:016X}", request.item_raw_guid),
            "Answering missing page text query with empty page"
        );
        wow_proto::SmsgPageTextQueryResponse {
            page_text_id: request.page_text_id,
            text: String::new(),
            next_page_text_id: 0,
        }
    };
    send_packet(
        stream,
        WorldOpcode::SmsgPageTextQueryResponse as u16,
        &response.body(),
        Some(header_crypto),
    )
    .await
}
