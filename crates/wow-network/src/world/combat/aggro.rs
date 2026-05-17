use super::*;

pub(in crate::world) async fn try_start_db_creature_aggro(
    stream: &mut WorldPacketSink,
    shared_world: SharedWorldDeps<'_>,
    session: &mut WorldSessionState,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    let Some(character) = session.character.active_character.clone() else {
        return Ok(());
    };
    let map_id = character.position.map_id;
    let player = ObjectGuid::new(HighGuid::Player, 0, character.guid);
    let sight_candidates = shared_world
        .maps
        .select_db_creature_sight_aggro_targets(map_id, &character)
        .await;
    for creature in sight_candidates {
        let attacker = creature.guid();
        mirror_session_db_creature(session, attacker.raw(), creature.clone());
        if !db_creature_navigation_check(
            &session.movement.db_creature_navigation,
            creature.current_position,
            character.position,
        )
        .is_clear()
        {
            continue;
        }
        if !begin_shared_db_creature_combat(shared_world, session, attacker, Instant::now()).await {
            continue;
        }
        send_db_creature_combat_start(
            stream,
            shared_world,
            map_id,
            session,
            attacker,
            player,
            header_crypto,
        )
        .await?;

        let assistants = shared_world
            .maps
            .select_db_creature_assist_targets(map_id, attacker, &character)
            .await;
        let assistant_targets = if let Some((caller, assistants)) = assistants {
            mirror_session_db_creature(session, attacker.raw(), caller);
            assistants
        } else {
            Vec::new()
        };
        for assistant in assistant_targets {
            if begin_shared_db_creature_combat(shared_world, session, assistant, Instant::now())
                .await
            {
                send_db_creature_combat_start(
                    stream,
                    shared_world,
                    map_id,
                    session,
                    assistant,
                    player,
                    header_crypto,
                )
                .await?;
            }
        }
    }
    Ok(())
}

pub(in crate::world) async fn send_db_creature_combat_start(
    stream: &mut WorldPacketSink,
    shared_world: SharedWorldDeps<'_>,
    map_id: u32,
    session: &mut WorldSessionState,
    attacker: ObjectGuid,
    player: ObjectGuid,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    let broadcast = CreatureCombatBroadcast {
        shared_world,
        map_id,
        player,
    };
    let attack_start_body = build_attack_start_body(attacker, player);
    send_packet(
        stream,
        SMSG_ATTACKSTART,
        &attack_start_body,
        Some(&mut *header_crypto),
    )
    .await?;
    broadcast_db_creature_packet(
        broadcast,
        session,
        attacker,
        SMSG_ATTACKSTART,
        attack_start_body,
    )
    .await;
    send_player_combat_flag_if_changed(stream, session, true, header_crypto).await?;
    let creature_flags_body = shared_world
        .maps
        .db_creature_snapshot(map_id, attacker)
        .await
        .map(|creature| {
            build_unit_flags_update_body(attacker, db_creature_unit_flags(&creature, true))
        })
        .transpose()?;
    if let Some(creature_flags_body) = creature_flags_body {
        send_packet(
            stream,
            SMSG_UPDATE_OBJECT,
            &creature_flags_body,
            Some(&mut *header_crypto),
        )
        .await?;
        broadcast_db_creature_packet(
            broadcast,
            session,
            attacker,
            SMSG_UPDATE_OBJECT,
            creature_flags_body,
        )
        .await;
    }
    send_db_creature_chase_if_needed(
        stream,
        broadcast,
        session,
        attacker,
        Instant::now(),
        header_crypto,
    )
    .await
}

pub(in crate::world) async fn send_active_db_creature_attack(
    stream: &mut WorldPacketSink,
    context: ActiveDbCreatureAttackContext<'_>,
    session: &mut WorldSessionState,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    let Some(character) = &session.character.active_character else {
        return Ok(());
    };
    let map_id = character.position.map_id;
    let character_guid = character.guid;
    let player = ObjectGuid::new(HighGuid::Player, 0, character_guid);
    let active_combats = context
        .shared_world
        .maps
        .active_db_creature_combats_for_victim(map_id, player)
        .await;
    if active_combats.is_empty() {
        clear_session_active_creature_combats(session);
        send_player_combat_flag_if_changed(stream, session, false, header_crypto).await?;
        return Ok(());
    }
    send_player_combat_flag_if_changed(stream, session, true, header_crypto).await?;
    mirror_session_active_creature_combats(session, &active_combats);
    for combat in active_combats {
        send_single_active_db_creature_attack(stream, context, session, header_crypto, combat)
            .await?;
    }
    Ok(())
}

#[derive(Clone, Copy)]
pub(in crate::world) struct ActiveDbCreatureAttackContext<'a> {
    pub(in crate::world) character_db_pool: &'a MySqlPool,
    pub(in crate::world) world_db_pool: &'a MySqlPool,
    pub(in crate::world) shared_world: SharedWorldDeps<'a>,
    pub(in crate::world) session_id: SessionId,
}

pub(in crate::world) async fn send_single_active_db_creature_attack(
    stream: &mut WorldPacketSink,
    context: ActiveDbCreatureAttackContext<'_>,
    session: &mut WorldSessionState,
    header_crypto: &mut HeaderCrypto,
    combat: CreatureCombatState,
) -> anyhow::Result<()> {
    let ActiveDbCreatureAttackContext {
        character_db_pool,
        world_db_pool,
        shared_world,
        session_id,
    } = context;
    let attacker = combat.attacker;
    let Some(character) = session.character.active_character.as_ref() else {
        return Ok(());
    };
    let character_snapshot = character.clone();
    let map_id = character.position.map_id;
    let player = ObjectGuid::new(HighGuid::Player, 0, character.guid);
    let broadcast = CreatureCombatBroadcast {
        shared_world,
        map_id,
        player,
    };
    if session.death.player_death_state != PlayerDeathState::Alive {
        clear_session_active_creature_combats(session);
        shared_world
            .maps
            .clear_db_creature_combats_for_victim(map_id, player)
            .await;
        return Ok(());
    }
    if combat.victim != player {
        remove_session_active_creature_combat(session, attacker);
        shared_world
            .maps
            .clear_db_creature_combat(map_id, attacker)
            .await;
        return Ok(());
    }
    let now = Instant::now();
    let was_fleeing = shared_world
        .maps
        .db_creature_snapshot(map_id, attacker)
        .await
        .is_some_and(|creature| creature.is_fleeing());
    advance_db_creature_motion_and_share(shared_world, map_id, session, attacker, now).await;
    let Some(active) = shared_world
        .maps
        .active_db_creature_combat_snapshot(map_id, attacker, player)
        .await
    else {
        remove_session_active_creature_combat(session, attacker);
        return Ok(());
    };
    let combat = active.combat;
    mirror_session_db_creature(session, attacker.raw(), active.creature.clone());
    if was_fleeing && !active.creature.is_fleeing() {
        let body =
            build_unit_flags_update_body(attacker, db_creature_unit_flags(&active.creature, true))?;
        send_packet(stream, SMSG_UPDATE_OBJECT, &body, Some(&mut *header_crypto)).await?;
        broadcast_db_creature_snapshot_packet(
            broadcast,
            active.creature.clone(),
            SMSG_UPDATE_OBJECT,
            body,
        )
        .await;
    }
    if active_aura_has_hard_control(&active.creature.active_auras) {
        defer_ready_db_creature_swing_retry(shared_world, map_id, session, attacker, player, now)
            .await;
        return Ok(());
    }
    if active.creature.is_fleeing() {
        defer_ready_db_creature_swing_retry(shared_world, map_id, session, attacker, player, now)
            .await;
        return Ok(());
    }
    if db_creature_should_evade_from_map(shared_world, map_id, attacker, now).await {
        send_db_creature_evade_and_return_home(
            stream,
            broadcast,
            session,
            attacker,
            now,
            header_crypto,
        )
        .await?;
        return Ok(());
    }
    if let Some(due_at) = shared_world
        .maps
        .active_db_creature_spell_cast_due_at(map_id, attacker)
        .await
    {
        if now < due_at {
            return Ok(());
        }
    }
    if complete_ready_db_creature_spell_cast(
        stream,
        shared_world,
        map_id,
        session,
        attacker,
        player,
        now,
        header_crypto,
    )
    .await?
    {
        return Ok(());
    }
    if try_start_db_creature_event_ai_spell_cast(
        stream,
        context.world_db_pool,
        shared_world,
        session_id,
        map_id,
        session,
        &active.creature,
        attacker,
        player,
        now,
        header_crypto,
    )
    .await?
    {
        return Ok(());
    }
    if try_start_db_creature_spell_cast(
        stream,
        context.world_db_pool,
        shared_world,
        session_id,
        map_id,
        session,
        &active.creature,
        attacker,
        player,
        now,
        header_crypto,
    )
    .await?
    {
        return Ok(());
    }
    if !db_creature_can_reach_player_from_map(shared_world, session, map_id, attacker).await {
        defer_ready_db_creature_swing_retry(shared_world, map_id, session, attacker, player, now)
            .await;
        send_db_creature_chase_if_needed(stream, broadcast, session, attacker, now, header_crypto)
            .await?;
        return Ok(());
    }
    if !db_creature_has_player_in_arc_from_map(shared_world, session, map_id, attacker).await {
        send_db_creature_face_target(stream, broadcast, session, attacker, header_crypto).await?;
        defer_ready_db_creature_swing_retry(shared_world, map_id, session, attacker, player, now)
            .await;
        return Ok(());
    }
    if now < combat.next_swing_at {
        return Ok(());
    }

    let next_swing_delay = active.creature.base_attack_duration();
    let combat_stats = shared_world
        .maps
        .player_combat_stats(map_id, character_snapshot.guid)
        .await
        .ok_or_else(|| {
            anyhow::anyhow!(
                "map-owned player combat stats missing for character {}",
                character_snapshot.guid
            )
        })?;
    let defense_input = player_melee_defense_input(
        &character_snapshot,
        &combat_stats,
        &session.character.character_skills,
        &session.auras.active_auras,
    );
    let outcome = active.creature.melee_outcome_against_player(defense_input);
    let Some(event) = shared_world
        .maps
        .apply_db_creature_player_melee_outcome(
            map_id,
            attacker,
            player,
            outcome,
            now,
            now + next_swing_delay,
        )
        .await?
    else {
        remove_session_active_creature_combat(session, attacker);
        shared_world
            .maps
            .clear_db_creature_combat(map_id, attacker)
            .await;
        return Ok(());
    };
    session.character.player_health = event.victim_health;
    let advanced_skill = try_advance_combat_skill_value(
        character_snapshot.level,
        SKILL_DEFENSE,
        combat_stats.intellect,
        false,
        &mut session.character.character_skills,
    );
    if let Some(updated) = advanced_skill {
        wow_db::upsert_character_skill(
            character_db_pool,
            character_snapshot.guid,
            updated.skill,
            updated.value,
            updated.max,
        )
        .await?;
    }
    let rage_gain = rage_gain_from_damage_taken(event.damage, character_snapshot.level);
    if rage_gain > 0 {
        session.character.player_rage = session
            .character
            .player_rage
            .saturating_add(rage_gain)
            .min(POWER_RAGE_DEFAULT);
        shared_world
            .maps
            .set_player_power2(
                map_id,
                character_snapshot.guid,
                session.character.player_rage,
            )
            .await;
    }
    interrupt_player_consumable_auras(
        stream,
        shared_world.maps,
        shared_world.sessions,
        session,
        AURA_INTERRUPT_FLAG_DAMAGE,
        header_crypto,
    )
    .await?;
    if event.damage > 0 {
        let opening_cancelled = cancel_pending_opening_spell_cast(
            stream,
            shared_world.maps,
            shared_world.sessions,
            session,
            SPELL_FAILED_INTERRUPTED,
            header_crypto,
        )
        .await?;
        apply_player_taken_melee_proc_auras(
            stream,
            world_db_pool,
            shared_world,
            session,
            map_id,
            character_snapshot.guid,
            player,
            attacker,
            now,
            header_crypto,
        )
        .await?;
        if !opening_cancelled {
            if let Some(channel_event) = shared_world
                .maps
                .interrupt_active_player_channel_for_damage(map_id, character_snapshot.guid, now)
                .await?
            {
                shared_world
                    .sessions
                    .dispatch(channel_event.observer_packets)
                    .await;
                for packet in channel_event.direct_packets {
                    send_packet(
                        stream,
                        packet.opcode,
                        &packet.body,
                        Some(&mut *header_crypto),
                    )
                    .await?;
                }
            }
            if let Some(delay_millis) = shared_world
                .maps
                .delay_active_player_spell_cast_for_damage(map_id, character_snapshot.guid, now)
                .await
            {
                send_packet(
                    stream,
                    SMSG_SPELL_DELAYED,
                    &build_spell_delayed_body(player, delay_millis)?,
                    Some(&mut *header_crypto),
                )
                .await?;
            }
        }
    }
    mirror_session_active_creature_combat(session, event.combat);
    shared_world.sessions.dispatch(event.observer_packets).await;
    for packet in event.direct_packets {
        send_packet(
            stream,
            packet.opcode,
            &packet.body,
            Some(&mut *header_crypto),
        )
        .await?;
    }
    if let Some(packet) = event.aura_packet {
        send_packet(
            stream,
            packet.opcode,
            &packet.body,
            Some(&mut *header_crypto),
        )
        .await?;
    }
    send_packet(
        stream,
        SMSG_ATTACKERSTATEUPDATE,
        &build_attacker_state_update_body_for_outcome(attacker, player, outcome, 0)?,
        Some(&mut *header_crypto),
    )
    .await?;
    send_packet(
        stream,
        SMSG_UPDATE_OBJECT,
        &event.health_update_body,
        Some(&mut *header_crypto),
    )
    .await?;
    if let Some(updated) = advanced_skill {
        send_packet(
            stream,
            SMSG_UPDATE_OBJECT,
            &build_player_skill_update_body(
                character_snapshot.guid,
                updated,
                &session.auras.active_auras,
            )?,
            Some(&mut *header_crypto),
        )
        .await?;
    }
    if rage_gain > 0 {
        send_packet(
            stream,
            SMSG_UPDATE_OBJECT,
            &build_player_rage_update_body(player, session.character.player_rage)?,
            Some(&mut *header_crypto),
        )
        .await?;
    }
    if session.character.player_health == 0 {
        refresh_session_from_map_owned_player_death(shared_world.maps, map_id, session).await;
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub(in crate::world) async fn try_start_db_creature_spell_cast(
    stream: &mut WorldPacketSink,
    world_db_pool: &MySqlPool,
    shared_world: SharedWorldDeps<'_>,
    current_session_id: SessionId,
    map_id: u32,
    session: &mut WorldSessionState,
    creature: &DbCreatureRuntime,
    attacker: ObjectGuid,
    victim: ObjectGuid,
    now: Instant,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<bool> {
    let spell_list_id = if creature.spawn.template.spell_list != 0 {
        creature.spawn.template.spell_list
    } else {
        creature.spawn.template.entry.saturating_mul(100)
    };
    if spell_list_id == 0 {
        return Ok(false);
    }
    let spell_list = shared_world
        .object_mgr
        .creature_spell_list(world_db_pool, spell_list_id)
        .await?;
    let condition_cache =
        load_db_creature_spell_condition_cache(shared_world.object_mgr, world_db_pool, &spell_list)
            .await?;
    let Some(ready) = shared_world
        .maps
        .ready_db_creature_spell_cast(map_id, attacker, victim, &spell_list, &condition_cache, now)
        .await
    else {
        return Ok(false);
    };
    let Some(template) = shared_world
        .object_mgr
        .spell_template(world_db_pool, ready.spell.spell_id)
        .await?
    else {
        return Ok(false);
    };
    if (template.attributes_ex & SPELL_ATTR_EX_NO_AUTOCAST_AI) != 0
        || (template.attributes & SPELL_ATTR_PASSIVE) != 0
    {
        return Ok(false);
    }
    let spell_range = shared_world.maps.spell_range(template.range_index);
    let spell_info = SpellInfo::from_template(&template);
    if shared_world
        .maps
        .validate_db_creature_spell_against_target(
            map_id,
            attacker,
            ready.target,
            &session.movement.db_creature_navigation,
            spell_range,
            spell_info.requires_behind_target(),
        )
        .await
        .check
        != DbCreatureSpellTargetCheck::Clear
    {
        return Ok(false);
    }
    let target = ready.target;
    let aura =
        (target.is_player() && spell_info.has_effect(SpellEffectDispatch::ApplyAura)).then(|| {
            build_active_aura(
                &template,
                attacker,
                creature
                    .spawn
                    .template
                    .max_level
                    .max(creature.spawn.template.min_level),
                SpellEffectValueContext::with_spell_rank_level(
                    &template,
                    (creature
                        .spawn
                        .template
                        .max_level
                        .max(creature.spawn.template.min_level)
                        / 5) as i32,
                    0,
                ),
                now,
                shared_world.maps.spell_duration(template.duration_index),
            )
        });
    let effect = if spell_info.has_direct_damage_effect() {
        if !target.is_player() {
            return Ok(false);
        }
        let damage = spell_info.direct_damage();
        if damage == 0 {
            return Ok(false);
        }
        ActiveDbCreatureSpellEffect::Damage {
            amount: damage,
            school: template.school as u8,
            dmg_class: template.dmg_class,
            attributes_ex2: template.attributes_ex2,
            attributes_ex3: template.attributes_ex3,
        }
    } else if spell_info.has_direct_heal_effect() {
        if target.is_player() {
            return Ok(false);
        }
        let heal = spell_info.direct_heal();
        if heal == 0 {
            return Ok(false);
        }
        ActiveDbCreatureSpellEffect::Heal { amount: heal }
    } else {
        return Ok(false);
    };
    let mana_cost = if template.power_type == POWER_TYPE_MANA {
        template.mana_cost
    } else {
        0
    };
    let cast_time_millis = spell_cast_time_millis(
        shared_world
            .maps
            .spell_cast_time(template.casting_time_index),
    );
    let cast = ActiveDbCreatureSpellCast {
        caster: attacker,
        target,
        spell_id: template.id,
        requires_behind: spell_info.requires_behind_target(),
        effect,
        aura,
        range: spell_range,
        mana_cost,
        cast_time_millis,
        due_at: now + Duration::from_millis(cast_time_millis as u64),
    };
    let Some(start_packets) = shared_world
        .maps
        .start_db_creature_spell_cast(map_id, cast)
        .await?
    else {
        return Ok(false);
    };
    shared_world
        .maps
        .apply_db_creature_spell_cooldowns(map_id, attacker, &ready.spell, &template, now)
        .await;
    send_or_dispatch_creature_spell_packets(
        stream,
        shared_world,
        session,
        start_packets,
        Some(current_session_id),
        header_crypto,
    )
    .await?;
    if cast_time_millis == 0 {
        complete_ready_db_creature_spell_cast(
            stream,
            shared_world,
            map_id,
            session,
            attacker,
            target,
            now,
            header_crypto,
        )
        .await?;
    }
    Ok(true)
}

#[allow(clippy::too_many_arguments)]
pub(in crate::world) async fn try_start_db_creature_event_ai_spell_cast(
    stream: &mut WorldPacketSink,
    world_db_pool: &MySqlPool,
    shared_world: SharedWorldDeps<'_>,
    current_session_id: SessionId,
    map_id: u32,
    session: &mut WorldSessionState,
    creature: &DbCreatureRuntime,
    attacker: ObjectGuid,
    victim: ObjectGuid,
    now: Instant,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<bool> {
    let scripts = shared_world
        .object_mgr
        .creature_ai_scripts(world_db_pool, creature.spawn.entry)
        .await?;
    if scripts.is_empty() {
        return Ok(false);
    }
    let Some(ready) = shared_world
        .maps
        .ready_db_creature_event_ai_spell_cast(map_id, attacker, victim, &scripts, now)
        .await
    else {
        return Ok(false);
    };
    let Some(template) = shared_world
        .object_mgr
        .spell_template(world_db_pool, ready.spell_id)
        .await?
    else {
        return Ok(false);
    };
    let spell_range = shared_world.maps.spell_range(template.range_index);
    let spell_info = SpellInfo::from_template(&template);
    if shared_world
        .maps
        .validate_db_creature_spell_against_target(
            map_id,
            attacker,
            ready.target,
            &session.movement.db_creature_navigation,
            spell_range,
            spell_info.requires_behind_target(),
        )
        .await
        .check
        != DbCreatureSpellTargetCheck::Clear
    {
        return Ok(false);
    }
    let Some(cast) = shared_world
        .maps
        .prepare_db_creature_spell_cast_from_template(
            map_id,
            attacker,
            ready.target,
            &template,
            now,
        )
        .await
    else {
        return Ok(false);
    };
    let cast_time_millis = cast.cast_time_millis;
    let target = cast.target;
    let Some(start_packets) = shared_world
        .maps
        .start_db_creature_spell_cast(map_id, cast)
        .await?
    else {
        return Ok(false);
    };
    shared_world
        .maps
        .apply_db_creature_event_ai_spell_cooldown(map_id, attacker, &ready, now)
        .await;
    send_or_dispatch_creature_spell_packets(
        stream,
        shared_world,
        session,
        start_packets,
        Some(current_session_id),
        header_crypto,
    )
    .await?;
    if cast_time_millis == 0 {
        complete_ready_db_creature_spell_cast(
            stream,
            shared_world,
            map_id,
            session,
            attacker,
            target,
            now,
            header_crypto,
        )
        .await?;
    }
    Ok(true)
}

#[allow(clippy::too_many_arguments)]
pub(in crate::world) async fn try_start_db_creature_ooc_event_ai_spell_cast(
    stream: &mut WorldPacketSink,
    world_db_pool: &MySqlPool,
    shared_world: SharedWorldDeps<'_>,
    current_session_id: SessionId,
    map_id: u32,
    session: &mut WorldSessionState,
    _creature: &DbCreatureRuntime,
    attacker: ObjectGuid,
    nearby_player: ObjectGuid,
    scripts: &[wow_db::CreatureAiScriptQuery],
    now: Instant,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<bool> {
    let Some(ready) = shared_world
        .maps
        .ready_db_creature_event_ai_ooc_spell_cast(map_id, attacker, scripts, now)
        .await
    else {
        return Ok(false);
    };
    let Some(template) = shared_world
        .object_mgr
        .spell_template(world_db_pool, ready.spell_id)
        .await?
    else {
        return Ok(false);
    };
    let spell_range = shared_world.maps.spell_range(template.range_index);
    let spell_info = SpellInfo::from_template(&template);
    if ready.target != attacker
        && shared_world
            .maps
            .validate_db_creature_spell_against_target(
                map_id,
                attacker,
                ready.target,
                &session.movement.db_creature_navigation,
                spell_range,
                spell_info.requires_behind_target(),
            )
            .await
            .check
            != DbCreatureSpellTargetCheck::Clear
    {
        return Ok(false);
    }
    let Some(cast) = shared_world
        .maps
        .prepare_db_creature_spell_cast_from_template(
            map_id,
            attacker,
            ready.target,
            &template,
            now,
        )
        .await
    else {
        return Ok(false);
    };
    let cast_time_millis = cast.cast_time_millis;
    let target = cast.target;
    let Some(start_packets) = shared_world
        .maps
        .start_db_creature_spell_cast(map_id, cast)
        .await?
    else {
        return Ok(false);
    };
    shared_world
        .maps
        .apply_db_creature_event_ai_spell_cooldown(map_id, attacker, &ready, now)
        .await;
    send_or_dispatch_creature_spell_packets(
        stream,
        shared_world,
        session,
        start_packets,
        Some(current_session_id),
        header_crypto,
    )
    .await?;
    if cast_time_millis == 0 {
        complete_ready_db_creature_spell_cast(
            stream,
            shared_world,
            map_id,
            session,
            attacker,
            if target.is_player() {
                target
            } else {
                nearby_player
            },
            now,
            header_crypto,
        )
        .await?;
    }
    Ok(true)
}

pub(in crate::world) async fn load_db_creature_spell_condition_cache(
    object_mgr: &ObjectMgr,
    world_db_pool: &MySqlPool,
    spell_list: &[wow_db::CreatureSpellListQuery],
) -> anyhow::Result<DbCreatureSpellConditionCache> {
    let mut cache = DbCreatureSpellConditionCache::default();
    for spell in spell_list {
        if spell.target_unit_condition > 0 {
            if let Some(condition) = object_mgr
                .unit_condition(world_db_pool, spell.target_unit_condition)
                .await?
            {
                cache.unit_conditions.insert(condition.id, condition);
            }
        }
        if spell.combat_condition > 0 {
            if let Some(condition) = object_mgr
                .combat_condition(world_db_pool, spell.combat_condition)
                .await?
            {
                collect_db_creature_spell_unit_condition(
                    object_mgr,
                    world_db_pool,
                    condition.self_condition_id,
                    &mut cache,
                )
                .await?;
                collect_db_creature_spell_unit_condition(
                    object_mgr,
                    world_db_pool,
                    condition.target_condition_id,
                    &mut cache,
                )
                .await?;
                collect_db_creature_spell_unit_condition(
                    object_mgr,
                    world_db_pool,
                    condition.friend_condition_id_0,
                    &mut cache,
                )
                .await?;
                collect_db_creature_spell_unit_condition(
                    object_mgr,
                    world_db_pool,
                    condition.friend_condition_id_1,
                    &mut cache,
                )
                .await?;
                collect_db_creature_spell_unit_condition(
                    object_mgr,
                    world_db_pool,
                    condition.enemy_condition_id_0,
                    &mut cache,
                )
                .await?;
                collect_db_creature_spell_unit_condition(
                    object_mgr,
                    world_db_pool,
                    condition.enemy_condition_id_1,
                    &mut cache,
                )
                .await?;
                cache.combat_conditions.insert(condition.id, condition);
            }
        }
    }
    Ok(cache)
}

pub(in crate::world) async fn collect_db_creature_spell_unit_condition(
    object_mgr: &ObjectMgr,
    world_db_pool: &MySqlPool,
    id: i32,
    cache: &mut DbCreatureSpellConditionCache,
) -> anyhow::Result<()> {
    if id == 0 || cache.unit_conditions.contains_key(&id) {
        return Ok(());
    }
    if let Some(condition) = object_mgr.unit_condition(world_db_pool, id).await? {
        cache.unit_conditions.insert(condition.id, condition);
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub(in crate::world) async fn complete_ready_db_creature_spell_cast(
    stream: &mut WorldPacketSink,
    shared_world: SharedWorldDeps<'_>,
    map_id: u32,
    session: &mut WorldSessionState,
    attacker: ObjectGuid,
    victim: ObjectGuid,
    now: Instant,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<bool> {
    let Some(event) = shared_world
        .maps
        .complete_ready_db_creature_spell_cast(
            map_id,
            attacker,
            victim,
            now,
            &session.movement.db_creature_navigation,
        )
        .await?
    else {
        return Ok(false);
    };
    let mut aura_event = event.aura_event;
    let creature_aura_event = event.creature_aura_event;
    match event.effect {
        DbCreatureCompletedSpellEffect::Interrupted(interrupted) => {
            send_or_dispatch_creature_spell_packets(
                stream,
                shared_world,
                session,
                interrupted.observer_packets,
                None,
                header_crypto,
            )
            .await?;
            return Ok(true);
        }
        DbCreatureCompletedSpellEffect::PlayerDamage(damage) => {
            let spell_go_body = event.spell_go_body;
            send_packet(
                stream,
                SMSG_SPELL_GO,
                &spell_go_body,
                Some(&mut *header_crypto),
            )
            .await?;
            broadcast_db_creature_packet(
                CreatureCombatBroadcast {
                    shared_world,
                    map_id,
                    player: victim,
                },
                session,
                attacker,
                SMSG_SPELL_GO,
                spell_go_body,
            )
            .await;
            for packet in damage.direct_packets {
                send_packet(
                    stream,
                    packet.opcode,
                    &packet.body,
                    Some(&mut *header_crypto),
                )
                .await?;
            }
            if let Some(body) = damage.spell_miss_log_body {
                send_packet(stream, SMSG_SPELLLOGMISS, &body, Some(&mut *header_crypto)).await?;
            }
            if let Some(body) = damage.spell_non_melee_log_body {
                send_packet(
                    stream,
                    SMSG_SPELLNONMELEEDAMAGELOG,
                    &body,
                    Some(&mut *header_crypto),
                )
                .await?;
            }
            if let Some(packet) = damage.aura_packet {
                send_packet(
                    stream,
                    packet.opcode,
                    &packet.body,
                    Some(&mut *header_crypto),
                )
                .await?;
            }
            session.character.player_health = damage.victim_health;
            send_packet(
                stream,
                SMSG_UPDATE_OBJECT,
                &damage.health_update_body,
                Some(&mut *header_crypto),
            )
            .await?;
            shared_world
                .sessions
                .dispatch(damage.observer_packets)
                .await;
            if session.character.player_health == 0 {
                aura_event = None;
                refresh_session_from_map_owned_player_death(shared_world.maps, map_id, session)
                    .await;
            }
        }
        DbCreatureCompletedSpellEffect::CreatureHeal(heal) => {
            let spell_go_body = event.spell_go_body;
            send_packet(
                stream,
                SMSG_SPELL_GO,
                &spell_go_body,
                Some(&mut *header_crypto),
            )
            .await?;
            broadcast_db_creature_packet(
                CreatureCombatBroadcast {
                    shared_world,
                    map_id,
                    player: victim,
                },
                session,
                attacker,
                SMSG_SPELL_GO,
                spell_go_body,
            )
            .await;
            send_packet(
                stream,
                SMSG_SPELLHEALLOG,
                &heal.spell_heal_log_body,
                Some(&mut *header_crypto),
            )
            .await?;
            send_packet(
                stream,
                SMSG_UPDATE_OBJECT,
                &heal.health_update_body,
                Some(&mut *header_crypto),
            )
            .await?;
            shared_world.sessions.dispatch(heal.observer_packets).await;
        }
        DbCreatureCompletedSpellEffect::AuraOnly => {
            let spell_go_body = event.spell_go_body;
            send_packet(
                stream,
                SMSG_SPELL_GO,
                &spell_go_body,
                Some(&mut *header_crypto),
            )
            .await?;
            broadcast_db_creature_packet(
                CreatureCombatBroadcast {
                    shared_world,
                    map_id,
                    player: victim,
                },
                session,
                attacker,
                SMSG_SPELL_GO,
                spell_go_body,
            )
            .await;
        }
    }
    if let Some(aura_event) = aura_event {
        for packet in aura_event.direct_packets {
            send_packet(
                stream,
                packet.opcode,
                &packet.body,
                Some(&mut *header_crypto),
            )
            .await?;
        }
        shared_world
            .sessions
            .dispatch(aura_event.observer_packets)
            .await;
    }
    if let Some(creature_aura_event) = creature_aura_event {
        send_packet(
            stream,
            SMSG_UPDATE_OBJECT,
            &creature_aura_event.update_body,
            Some(&mut *header_crypto),
        )
        .await?;
        for packet in creature_aura_event.direct_packets {
            send_packet(
                stream,
                packet.opcode,
                &packet.body,
                Some(&mut *header_crypto),
            )
            .await?;
        }
        shared_world
            .sessions
            .dispatch(creature_aura_event.observer_packets)
            .await;
    }
    Ok(true)
}

pub(in crate::world) async fn send_or_dispatch_creature_spell_packets(
    stream: &mut WorldPacketSink,
    shared_world: SharedWorldDeps<'_>,
    session: &WorldSessionState,
    packets: Vec<(SessionId, OutboundWorldPacket)>,
    current_session_id: Option<SessionId>,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    let own_session_id = if let Some(character) = &session.character.active_character {
        shared_world
            .sessions
            .session_for_character(character.guid)
            .await
    } else {
        None
    };
    let mut dispatch = Vec::new();
    for (session_id, packet) in packets {
        if Some(session_id) == own_session_id || Some(session_id) == current_session_id {
            send_packet(
                stream,
                packet.opcode,
                &packet.body,
                Some(&mut *header_crypto),
            )
            .await?;
        } else {
            dispatch.push((session_id, packet));
        }
    }
    shared_world.sessions.dispatch(dispatch).await;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub(in crate::world) async fn apply_player_taken_melee_proc_auras(
    stream: &mut WorldPacketSink,
    world_db_pool: &MySqlPool,
    shared_world: SharedWorldDeps<'_>,
    session: &mut WorldSessionState,
    map_id: u32,
    character_guid: u32,
    player: ObjectGuid,
    attacker: ObjectGuid,
    now: Instant,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    let trigger_spell_ids = active_aura_proc_trigger_spell_ids(
        &mut session.auras.active_auras,
        PROC_FLAG_TAKE_MELEE_SWING,
        now,
    );
    if trigger_spell_ids.is_empty() {
        return Ok(());
    }

    for spell_id in trigger_spell_ids {
        let Some(template) = shared_world
            .object_mgr
            .spell_template(world_db_pool, spell_id)
            .await?
        else {
            warn!(
                spell_id,
                "Skipping aura proc because triggered spell_template row is missing"
            );
            continue;
        };
        let aura = build_active_aura(
            &template,
            player,
            session
                .character
                .active_character
                .as_ref()
                .map(|character| character.level)
                .unwrap_or(1),
            player_spell_effect_value_context(
                shared_world.maps,
                &template,
                &session.character.character_skills,
                0,
            ),
            now,
            shared_world.maps.spell_duration(template.duration_index),
        );
        if let Some(event) = shared_world
            .maps
            .apply_db_creature_aura(map_id, attacker, character_guid, aura, now)
            .await?
        {
            send_packet(
                stream,
                SMSG_UPDATE_OBJECT,
                &event.update_body,
                Some(&mut *header_crypto),
            )
            .await?;
            for packet in event.direct_packets {
                send_packet(
                    stream,
                    packet.opcode,
                    &packet.body,
                    Some(&mut *header_crypto),
                )
                .await?;
            }
            shared_world.sessions.dispatch(event.observer_packets).await;
        }
    }

    Ok(())
}

pub(in crate::world) async fn refresh_session_from_map_owned_player_death(
    maps: &Arc<MapRuntimeManager>,
    map_id: u32,
    session: &mut WorldSessionState,
) {
    let Some(character) = session.character.active_character.as_ref() else {
        return;
    };
    let Some(snapshot) = maps.player_runtime_snapshot(map_id, character.guid).await else {
        return;
    };
    if snapshot.health != 0 || snapshot.death_state == PlayerDeathState::Alive {
        return;
    }
    session.character.player_health = 0;
    session.death.player_death_state = snapshot.death_state;
    session.death.player_death_presentation_pending =
        snapshot.death_state == PlayerDeathState::JustDied;
    session.character.player_stand_state = snapshot.stand_state;
    session.auras.active_auras = snapshot.active_auras;
    session.character.player_flags = snapshot.flags;
    session.combat.player_in_combat = false;
    mirror_session_player_auto_attack(session, None, None);
    clear_session_active_creature_combats(session);
    if let Some(character) = session.character.active_character.as_mut() {
        character.position = snapshot.position;
        character.movement_flags = snapshot.movement_flags;
        character.client_time = snapshot.client_time;
        character.fall_time = snapshot.fall_time;
        character.jump = snapshot.jump;
    }
}

pub(in crate::world) async fn defer_ready_db_creature_swing_retry(
    shared_world: SharedWorldDeps<'_>,
    map_id: u32,
    session: &mut WorldSessionState,
    attacker: ObjectGuid,
    victim: ObjectGuid,
    now: Instant,
) {
    if let Some(combat) = shared_world
        .maps
        .defer_ready_db_creature_swing_retry(map_id, attacker, victim, now)
        .await
    {
        mirror_session_active_creature_combat(session, combat);
    }
}

pub(in crate::world) async fn send_db_creature_threat_target_switch(
    stream: &mut WorldPacketSink,
    shared_world: SharedWorldDeps<'_>,
    session: &mut WorldSessionState,
    event: Option<DbCreatureThreatTargetSwitchEvent>,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    let Some(event) = event else {
        return Ok(());
    };
    let current_player = session
        .character
        .active_character
        .as_ref()
        .map(|character| ObjectGuid::new(HighGuid::Player, 0, character.guid));
    if current_player == Some(event.old_victim) {
        remove_session_active_creature_combat(session, event.attacker);
    }
    if current_player == Some(event.new_victim) {
        mirror_session_active_creature_combat(session, event.combat);
        send_player_combat_flag_if_changed(stream, session, true, header_crypto).await?;
    }
    for packet in event.direct_packets {
        send_packet(
            stream,
            packet.opcode,
            &packet.body,
            Some(&mut *header_crypto),
        )
        .await?;
    }
    shared_world.sessions.dispatch(event.observer_packets).await;
    Ok(())
}

#[cfg(test)]
pub(in crate::world) fn select_db_creature_aggro_target(
    session: &WorldSessionState,
) -> Option<ObjectGuid> {
    select_db_creature_aggro_targets(session).into_iter().next()
}

#[cfg(test)]
pub(in crate::world) fn select_db_creature_aggro_targets(
    session: &WorldSessionState,
) -> Vec<ObjectGuid> {
    if session.death.player_death_state != PlayerDeathState::Alive {
        return Vec::new();
    }
    let Some(character) = session.character.active_character.as_ref() else {
        return Vec::new();
    };
    let faction_templates = FactionTemplateStore::fallback_bridge();
    let now = Instant::now();
    let mut targets = session
        .visibility
        .db_creatures
        .values()
        .filter(|creature| {
            !session
                .combat
                .active_creature_combats
                .contains_key(&creature.guid().raw())
        })
        .filter(|creature| creature.can_aggro_player(&faction_templates, character, now))
        .filter_map(|creature| {
            let distance_sq = creature.distance_to_player_squared(character)?;
            let attack_distance = db_creature_attack_distance(
                character.level,
                creature.spawn.template.min_level,
                creature.spawn.template.detection_range,
            );
            if distance_sq > attack_distance * attack_distance {
                return None;
            }
            if !db_creature_navigation_check(
                &session.movement.db_creature_navigation,
                creature.current_position,
                character.position,
            )
            .is_clear()
            {
                return None;
            }
            Some((distance_sq, creature.guid()))
        })
        .collect::<Vec<_>>();
    targets.sort_by(|(left_distance, left_guid), (right_distance, right_guid)| {
        left_distance
            .total_cmp(right_distance)
            .then_with(|| left_guid.raw().cmp(&right_guid.raw()))
    });
    targets.into_iter().map(|(_, guid)| guid).collect()
}

#[cfg(test)]
pub(in crate::world) fn select_db_creature_assist_targets(
    session: &mut WorldSessionState,
    caller_guid: ObjectGuid,
) -> Vec<ObjectGuid> {
    if session.death.player_death_state != PlayerDeathState::Alive {
        return Vec::new();
    }
    let Some(character) = session.character.active_character.as_ref() else {
        return Vec::new();
    };
    let Some(caller) = session.visibility.db_creatures.get_mut(&caller_guid.raw()) else {
        return Vec::new();
    };
    if caller.already_called_assistance {
        return Vec::new();
    }
    caller.already_called_assistance = true;
    let caller_position = caller.current_position;
    let caller_faction = caller.spawn.template.faction;
    let faction_templates = FactionTemplateStore::fallback_bridge();
    let now = Instant::now();
    let radius = if caller.spawn.template.call_for_help > 0 {
        caller.spawn.template.call_for_help as f32
    } else {
        DB_CREATURE_ASSISTANCE_RADIUS_YARDS
    };
    let mut targets = session
        .visibility
        .db_creatures
        .values()
        .filter(|creature| creature.guid() != caller_guid)
        .filter(|creature| {
            !session
                .combat
                .active_creature_combats
                .contains_key(&creature.guid().raw())
        })
        .filter(|creature| creature.spawn.template.faction == caller_faction)
        .filter(|creature| creature.can_aggro_player(&faction_templates, character, now))
        .filter_map(|creature| {
            let distance = distance_2d(
                caller_position.x,
                caller_position.y,
                creature.current_position.x,
                creature.current_position.y,
            );
            (distance <= radius).then_some((distance, creature.guid()))
        })
        .collect::<Vec<_>>();
    targets.sort_by(|(left_distance, left_guid), (right_distance, right_guid)| {
        left_distance
            .total_cmp(right_distance)
            .then_with(|| left_guid.raw().cmp(&right_guid.raw()))
    });
    targets.into_iter().map(|(_, guid)| guid).collect()
}

#[cfg(test)]
pub(in crate::world) fn begin_db_creature_combat(
    session: &mut WorldSessionState,
    attacker: ObjectGuid,
    now: Instant,
) -> bool {
    if session.death.player_death_state != PlayerDeathState::Alive {
        return false;
    }
    let Some(character) = &session.character.active_character else {
        return false;
    };
    let victim = ObjectGuid::new(HighGuid::Player, 0, character.guid);
    if session
        .combat
        .active_creature_combats
        .get(&attacker.raw())
        .is_some_and(|combat| combat.victim == victim)
    {
        return false;
    }
    session.combat.active_creature_combats.insert(
        attacker.raw(),
        CreatureCombatState {
            attacker,
            victim,
            next_swing_at: now,
        },
    );
    true
}

pub(in crate::world) async fn begin_shared_db_creature_combat(
    shared_world: SharedWorldDeps<'_>,
    session: &mut WorldSessionState,
    attacker: ObjectGuid,
    now: Instant,
) -> bool {
    if session.death.player_death_state != PlayerDeathState::Alive {
        return false;
    }
    let Some(character) = &session.character.active_character else {
        return false;
    };
    let victim = ObjectGuid::new(HighGuid::Player, 0, character.guid);
    let Some((combat, creature)) = shared_world
        .maps
        .begin_db_creature_combat(character.position.map_id, attacker, victim, now)
        .await
    else {
        return false;
    };
    mirror_session_db_creature(session, attacker.raw(), creature);
    mirror_session_active_creature_combat(session, combat);
    true
}

pub(in crate::world) fn clear_db_creature_combat_if_attacker(
    session: &mut WorldSessionState,
    attacker: ObjectGuid,
) {
    remove_session_active_creature_combat(session, attacker);
}

pub(in crate::world) async fn send_player_combat_flag_if_changed(
    stream: &mut WorldPacketSink,
    session: &mut WorldSessionState,
    in_combat: bool,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    if session.combat.player_in_combat == in_combat {
        return Ok(());
    }
    let Some(character) = &session.character.active_character else {
        return Ok(());
    };
    session.combat.player_in_combat = in_combat;
    let player = ObjectGuid::new(HighGuid::Player, 0, character.guid);
    send_packet(
        stream,
        SMSG_UPDATE_OBJECT,
        &build_unit_flags_update_body(player, player_unit_flags(in_combat))?,
        Some(header_crypto),
    )
    .await
}

pub(in crate::world) fn player_unit_flags(in_combat: bool) -> u32 {
    player_unit_flags_with_looting(in_combat, false)
}

pub(in crate::world) fn player_unit_flags_with_looting(in_combat: bool, looting: bool) -> u32 {
    UNIT_FLAG_PLAYER_CONTROLLED
        | (if looting { UNIT_FLAG_LOOTING } else { 0 })
        | (if in_combat { UNIT_FLAG_IN_COMBAT } else { 0 })
}

pub(in crate::world) fn db_creature_unit_flags(
    creature: &DbCreatureRuntime,
    in_combat: bool,
) -> u32 {
    creature.spawn.template.unit_flags
        | (if in_combat { UNIT_FLAG_IN_COMBAT } else { 0 })
        | (if active_aura_has_stun(&creature.active_auras) {
            UNIT_FLAG_STUNNED
        } else {
            0
        })
        | (if active_aura_has_confuse(&creature.active_auras) {
            UNIT_FLAG_CONFUSED
        } else {
            0
        })
        | (if creature.is_fleeing() {
            UNIT_FLAG_FLEEING
        } else {
            0
        })
}
