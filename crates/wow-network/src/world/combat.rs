async fn handle_attack_swing(
    stream: &mut TcpStream,
    character_db_pool: &MySqlPool,
    world_db_pool: &MySqlPool,
    body: &[u8],
    session: &mut WorldSessionState,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    let target = read_packet_guid(body, "CMSG_ATTACKSWING")?;
    let Some(character_guid) = session.active_character.as_ref().map(|character| character.guid)
    else {
        warn!("Ignoring attack swing before character login");
        return Ok(());
    };

    if target == rust_combat_dummy_guid() {
        if session.combat_dummy_lootable || session.combat_dummy_health == 0 {
            warn!("Ignoring attack swing against dead combat dummy");
            return Ok(());
        }

        session.active_combat_target = Some(target);
        session.active_combat_next_swing_at =
            Some(Instant::now() + Duration::from_millis(RUST_COMBAT_SWING_MILLIS));
        let attacker = ObjectGuid::new(HighGuid::Player, 0, character_guid);
        send_packet(
            stream,
            SMSG_ATTACKSTART,
            &build_attack_start_body(attacker, target),
            Some(&mut *header_crypto),
        )
        .await?;
        return send_combat_dummy_swing(stream, session, header_crypto).await;
    }

    if !session
        .db_creatures
        .get(&target.raw())
        .is_some_and(DbCreatureRuntime::is_alive)
    {
        warn!(
            target = format_args!("0x{:016X}", target.raw()),
            "Ignoring attack swing against unknown target"
        );
        return Ok(());
    }

    session.active_combat_target = Some(target);
    let now = Instant::now();
    session.active_combat_next_swing_at =
        Some(now + Duration::from_millis(RUST_COMBAT_SWING_MILLIS));
    begin_db_creature_combat(session, target, now);
    let attacker = ObjectGuid::new(HighGuid::Player, 0, character_guid);
    send_packet(
        stream,
        SMSG_ATTACKSTART,
        &build_attack_start_body(attacker, target),
        Some(&mut *header_crypto),
    )
    .await?;
    send_db_creature_swing(
        stream,
        character_db_pool,
        world_db_pool,
        session,
        header_crypto,
        target,
    )
    .await
}

impl DbCreatureRuntime {
    fn new(spawn: CreatureSpawnQuery) -> Self {
        let health = creature_health(&spawn.template);
        let home_position = db_creature_spawn_position(&spawn);
        Self {
            spawn,
            home_position,
            current_position: home_position,
            motion: CreatureMotionState::Idle,
            next_spline_id: 0,
            health,
            lootable: false,
            looting: false,
            loot_money_available: false,
            loot_item: None,
        }
    }

    fn guid(&self) -> ObjectGuid {
        creature_spawn_guid(&self.spawn)
    }

    fn is_alive(&self) -> bool {
        self.health > 0 && !self.lootable
    }

    fn max_health(&self) -> u32 {
        creature_health(&self.spawn.template)
    }

    fn hit_damage(&self) -> u32 {
        self.spawn.template.max_melee_dmg.ceil().max(1.0) as u32
    }

    fn base_attack_duration(&self) -> Duration {
        Duration::from_millis(self.spawn.template.melee_base_attack_time.max(1) as u64)
    }

    fn loot_money(&self) -> u32 {
        self.spawn
            .template
            .max_loot_gold
            .max(self.spawn.template.min_loot_gold)
    }

    fn dynamic_flags(&self) -> u32 {
        if self.lootable {
            UNIT_DYNFLAG_LOOTABLE
        } else {
            self.spawn.template.dynamic_flags
        }
    }

    fn respawn(&mut self) {
        self.health = self.max_health();
        self.lootable = false;
        self.looting = false;
        self.loot_money_available = false;
        self.loot_item = None;
        self.current_position = self.home_position;
        self.motion = CreatureMotionState::Idle;
    }

    fn can_aggro_player(&self, character: &ActiveCharacter) -> bool {
        self.is_alive()
            && self.spawn.map == character.position.map_id
            && self.spawn.template.civilian == 0
            && self.spawn.template.creature_type != CREATURE_TYPE_CRITTER
            && self.spawn.template.npc_flags == 0
            && is_starter_aggro_creature_entry(self.spawn.entry)
    }

    fn distance_to_player_squared(&self, character: &ActiveCharacter) -> Option<f32> {
        (self.current_position.map_id == character.position.map_id).then(|| {
            let dx = self.current_position.x - character.position.x;
            let dy = self.current_position.y - character.position.y;
            dx * dx + dy * dy
        })
    }
}

fn db_creature_spawn_position(spawn: &CreatureSpawnQuery) -> WorldPosition {
    WorldPosition::new(
        spawn.map,
        spawn.position_x,
        spawn.position_y,
        spawn.position_z,
        spawn.orientation,
    )
}

const REAL_KOBOLD_VERMIN_ENTRY: u32 = 6;
const REAL_DEFIAS_THUG_ENTRY: u32 = 38;
const FIXTURE_KOBOLD_VERMIN_ENTRY: u32 = 910_005;

fn is_starter_aggro_creature_entry(entry: u32) -> bool {
    matches!(
        entry,
        REAL_KOBOLD_VERMIN_ENTRY | REAL_DEFIAS_THUG_ENTRY | FIXTURE_KOBOLD_VERMIN_ENTRY
    )
}

fn apply_db_creature_damage(
    session: &mut WorldSessionState,
    target: ObjectGuid,
    requested_damage: u32,
) -> Option<u32> {
    let creature = session.db_creatures.get_mut(&target.raw())?;
    if !creature.is_alive() {
        return None;
    }

    let damage = creature.health.min(requested_damage.max(1));
    creature.health = creature.health.saturating_sub(damage);
    if creature.health == 0 {
        creature.lootable = true;
        creature.looting = false;
        creature.loot_money_available = creature.loot_money() > 0;
        creature.loot_item = None;
        session.active_combat_target = None;
        session.active_combat_next_swing_at = None;
        clear_db_creature_combat_if_attacker(session, target);
    }
    Some(damage)
}

async fn handle_combat_tick(
    stream: &mut TcpStream,
    character_db_pool: &MySqlPool,
    world_db_pool: &MySqlPool,
    session: &mut WorldSessionState,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    let now = Instant::now();
    if let Some(target) = session.active_combat_target {
        if session
            .active_combat_next_swing_at
            .is_none_or(|next_swing_at| now >= next_swing_at)
        {
            session.active_combat_next_swing_at =
                Some(now + Duration::from_millis(RUST_COMBAT_SWING_MILLIS));
            if target == rust_combat_dummy_guid() {
                send_combat_dummy_swing(stream, session, header_crypto).await?;
            } else {
                send_db_creature_swing(
                    stream,
                    character_db_pool,
                    world_db_pool,
                    session,
                    header_crypto,
                    target,
                )
                .await?;
            }
        }
    }

    if session.active_creature_combat.is_none() {
        try_start_db_creature_aggro(stream, session, header_crypto).await?;
    }
    send_active_db_creature_attack(stream, session, header_crypto).await
}

async fn send_db_creature_swing(
    stream: &mut TcpStream,
    character_db_pool: &MySqlPool,
    world_db_pool: &MySqlPool,
    session: &mut WorldSessionState,
    header_crypto: &mut HeaderCrypto,
    target: ObjectGuid,
) -> anyhow::Result<()> {
    let Some(character) = &session.active_character else {
        return Ok(());
    };
    let attacker = ObjectGuid::new(HighGuid::Player, 0, character.guid);
    let requested_damage = session
        .db_creatures
        .get(&target.raw())
        .map(DbCreatureRuntime::hit_damage)
        .unwrap_or(1);
    let Some(damage) = apply_db_creature_damage(session, target, requested_damage) else {
        session.active_combat_target = None;
        session.active_combat_next_swing_at = None;
        return Ok(());
    };
    session.player_rage =
        (session.player_rage + RUST_COMBAT_DUMMY_RAGE_GAIN).min(POWER_RAGE_DEFAULT);
    let (health, dynamic_flags, is_dead) = session
        .db_creatures
        .get(&target.raw())
        .map(|creature| {
            (
                creature.health,
                creature.dynamic_flags(),
                creature.health == 0,
            )
        })
        .expect("DB creature existed before damage");

    send_packet(
        stream,
        SMSG_ATTACKERSTATEUPDATE,
        &build_attacker_state_update_body(attacker, target, damage)?,
        Some(&mut *header_crypto),
    )
    .await?;
    send_packet(
        stream,
        SMSG_UPDATE_OBJECT,
        &build_db_creature_state_update_body(target, health, dynamic_flags)?,
        Some(&mut *header_crypto),
    )
    .await?;
    send_packet(
        stream,
        SMSG_UPDATE_OBJECT,
        &build_player_rage_update_body(attacker, session.player_rage)?,
        Some(&mut *header_crypto),
    )
    .await?;

    if is_dead {
        finalize_db_creature_death(
            stream,
            character_db_pool,
            world_db_pool,
            session,
            attacker,
            target,
            header_crypto,
        )
        .await?;
    }

    Ok(())
}

async fn finalize_db_creature_death(
    stream: &mut TcpStream,
    character_db_pool: &MySqlPool,
    world_db_pool: &MySqlPool,
    session: &mut WorldSessionState,
    killer: ObjectGuid,
    killed: ObjectGuid,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    if let Some(creature) = session.db_creatures.get_mut(&killed.raw()) {
        if creature.health == 0 {
            creature.lootable = true;
            creature.looting = false;
            creature.loot_money_available = creature.loot_money() > 0;
        }
    }
    session.active_combat_target = None;
    session.active_combat_next_swing_at = None;
    clear_db_creature_combat_if_attacker(session, killed);
    grant_db_creature_kill_credit(
        stream,
        character_db_pool,
        world_db_pool,
        session,
        killed,
        header_crypto,
    )
    .await?;
    grant_db_creature_xp(
        stream,
        character_db_pool,
        world_db_pool,
        session,
        killed,
        header_crypto,
    )
    .await?;
    send_packet(
        stream,
        SMSG_ATTACKSTOP,
        &build_attack_stop_body(killer, killed, true)?,
        Some(header_crypto),
    )
    .await
}

async fn grant_db_creature_xp(
    stream: &mut TcpStream,
    character_db_pool: &MySqlPool,
    world_db_pool: &MySqlPool,
    session: &mut WorldSessionState,
    killed: ObjectGuid,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    let Some(character) = &session.active_character else {
        return Ok(());
    };
    let Some(creature) = session.db_creatures.get(&killed.raw()) else {
        return Ok(());
    };
    let xp = creature_xp_reward(character.level, &creature.spawn.template);
    award_character_xp(
        stream,
        character_db_pool,
        world_db_pool,
        session,
        Some(killed),
        xp,
        header_crypto,
    )
    .await
}

async fn award_character_xp(
    stream: &mut TcpStream,
    character_db_pool: &MySqlPool,
    world_db_pool: &MySqlPool,
    session: &mut WorldSessionState,
    source: Option<ObjectGuid>,
    xp: u32,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    if xp == 0 {
        return Ok(());
    }
    let Some(character) = session.active_character.as_ref() else {
        return Ok(());
    };
    if character.level >= DEFAULT_MAX_PLAYER_LEVEL {
        return Ok(());
    }

    let guid = character.guid;
    let race = character.race;
    let class = character.class;
    let old_level = character.level;
    let old_xp = character.xp;
    let previous_stats = wow_db::get_player_world_stats(world_db_pool, race, class, old_level).await?;
    let mut new_level = old_level;
    let mut new_xp = old_xp.saturating_add(xp);
    let mut next_level_xp = previous_stats.next_level_xp;
    while next_level_xp > 0
        && new_xp >= next_level_xp
        && new_level < DEFAULT_MAX_PLAYER_LEVEL
    {
        new_xp -= next_level_xp;
        new_level += 1;
        next_level_xp = wow_db::get_player_next_level_xp(world_db_pool, new_level).await?;
    }
    let new_stats = wow_db::get_player_world_stats(world_db_pool, race, class, new_level).await?;
    let leveled = new_level != old_level;
    let max_health = new_stats.max_health().max(1);
    let max_mana = new_stats.max_mana();
    let health = if leveled {
        max_health
    } else {
        session.player_health.max(1).min(max_health)
    };
    let power1 = if max_mana == 0 {
        0
    } else if leveled {
        max_mana
    } else {
        session.player_mana.min(max_mana)
    };
    let power2 = session.player_rage.min(POWER_RAGE_DEFAULT);
    let power3 = 0;
    let power4 = create_power_for_class_power(class, POWER_ENERGY);
    let power5 = 0;

    wow_db::update_character_progression_state(
        character_db_pool,
        guid,
        wow_db::CharacterProgressionState {
            level: new_level,
            xp: new_xp,
            health,
            power1,
            power2,
            power3,
            power4,
            power5,
        },
    )
    .await?;
    if let Some(character) = session.active_character.as_mut() {
        character.level = new_level;
        character.xp = new_xp;
    }
    session.player_health = health;
    session.player_mana = power1;
    session.player_rage = power2;

    send_packet(
        stream,
        SMSG_LOG_XPGAIN,
        &build_log_xp_gain_body(source, xp),
        Some(&mut *header_crypto),
    )
    .await?;
    if leveled {
        send_packet(
            stream,
            SMSG_LEVELUP_INFO,
            &build_levelup_info_body(new_level, &previous_stats, &new_stats),
            Some(&mut *header_crypto),
        )
        .await?;
    }
    send_packet(
        stream,
        SMSG_UPDATE_OBJECT,
        &build_player_progression_update_body(PlayerProgressionUpdate {
            character_guid: guid,
            level: new_level,
            xp: new_xp,
            health,
            power1,
            power2,
            power3,
            power4,
            power5,
            world_stats: &new_stats,
        })?,
        Some(header_crypto),
    )
    .await
}

fn creature_xp_reward(player_level: u8, template: &CreatureTemplateQuery) -> u32 {
    if template.civilian != 0 || template.creature_type == CREATURE_TYPE_CRITTER {
        return 0;
    }

    let mut xp_gain = base_creature_xp_gain(player_level as u32, template.min_level as u32);
    if xp_gain == 0.0 {
        return 0;
    }
    if template.rank == CREATURE_ELITE_NORMAL || template.rank == CREATURE_ELITE_RARE_ELITE {
        xp_gain *= 2.5;
    }
    xp_gain *= template.experience_multiplier;
    nearbyint_to_u32(xp_gain)
}

fn base_creature_xp_gain(player_level: u32, mob_level: u32) -> f32 {
    let base_xp = player_level * 5 + 45;
    if mob_level >= player_level {
        let level_diff = (mob_level - player_level).min(4);
        return base_xp as f32 * (1.0 + 0.05 * level_diff as f32);
    }
    if mob_level > gray_level(player_level) {
        let level_diff = player_level - mob_level;
        return base_xp as f32 * (1.0 - (level_diff as f32 / zero_difference(player_level) as f32));
    }
    0.0
}

fn gray_level(player_level: u32) -> u32 {
    if player_level <= 5 {
        0
    } else if player_level <= 39 {
        player_level - 5 - player_level / 10
    } else if player_level <= 59 {
        player_level - 1 - player_level / 5
    } else {
        player_level - 9
    }
}

fn zero_difference(unit_level: u32) -> u32 {
    match unit_level {
        0..=7 => 5,
        8..=9 => 6,
        10..=11 => 7,
        12..=15 => 8,
        16..=19 => 9,
        20..=29 => 11,
        30..=39 => 12,
        40..=44 => 13,
        45..=49 => 14,
        50..=54 => 15,
        55..=59 => 16,
        _ => 17,
    }
}

fn quest_xp_reward(player_level: u8, quest: &QuestTemplateQuery) -> u32 {
    if quest.rew_money_max_level == 0 {
        return 0;
    }
    let quest_level = quest.quest_level;
    let divisor = match quest_level {
        65.. => 6.0,
        64 => 4.8,
        63 => 3.6,
        62 => 2.4,
        61 => 1.2,
        1..=60 => 0.6,
        _ => return 0,
    };
    let full_xp = quest.rew_money_max_level as f32 / divisor;
    let player_level = player_level as u32;
    let factor = if player_level <= quest_level + 5 {
        1.0
    } else if player_level == quest_level + 6 {
        0.8
    } else if player_level == quest_level + 7 {
        0.6
    } else if player_level == quest_level + 8 {
        0.4
    } else if player_level == quest_level + 9 {
        0.2
    } else {
        0.1
    };
    quest_xp_ceil(full_xp * factor)
}

fn quest_xp_ceil(value: f32) -> u32 {
    value.ceil().max(0.0) as u32
}

fn nearbyint_to_u32(value: f32) -> u32 {
    if value <= 0.0 {
        0
    } else {
        value.round_ties_even() as u32
    }
}

async fn send_combat_dummy_swing(
    stream: &mut TcpStream,
    session: &mut WorldSessionState,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    let Some(character) = &session.active_character else {
        return Ok(());
    };
    let attacker = ObjectGuid::new(HighGuid::Player, 0, character.guid);
    let target = rust_combat_dummy_guid();

    let damage = session
        .combat_dummy_health
        .min(RUST_COMBAT_DUMMY_HIT_DAMAGE);
    session.combat_dummy_health = session.combat_dummy_health.saturating_sub(damage);
    session.player_rage =
        (session.player_rage + RUST_COMBAT_DUMMY_RAGE_GAIN).min(POWER_RAGE_DEFAULT);

    send_packet(
        stream,
        SMSG_ATTACKERSTATEUPDATE,
        &build_attacker_state_update_body(attacker, target, damage)?,
        Some(&mut *header_crypto),
    )
    .await?;
    send_packet(
        stream,
        SMSG_UPDATE_OBJECT,
        &build_combat_dummy_state_update_body(session.combat_dummy_health, 0)?,
        Some(&mut *header_crypto),
    )
    .await?;
    send_packet(
        stream,
        SMSG_UPDATE_OBJECT,
        &build_player_rage_update_body(attacker, session.player_rage)?,
        Some(&mut *header_crypto),
    )
    .await?;

    if session.combat_dummy_health == 0 {
        session.combat_dummy_lootable = true;
        session.combat_dummy_looting = false;
        session.combat_dummy_loot_money_available = true;
        session.combat_dummy_loot_item_available = true;
        send_packet(
            stream,
            SMSG_ATTACKSTOP,
            &build_attack_stop_body(attacker, target, true)?,
            Some(&mut *header_crypto),
        )
        .await?;
        send_packet(
            stream,
            SMSG_UPDATE_OBJECT,
            &build_combat_dummy_state_update_body(0, UNIT_DYNFLAG_LOOTABLE)?,
            Some(header_crypto),
        )
        .await?;
        session.active_combat_target = None;
        session.active_combat_next_swing_at = None;
    }

    Ok(())
}

async fn try_start_db_creature_aggro(
    stream: &mut TcpStream,
    session: &mut WorldSessionState,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    let Some(character) = &session.active_character else {
        return Ok(());
    };
    let Some(attacker) = select_db_creature_aggro_target(session) else {
        return Ok(());
    };
    let player = ObjectGuid::new(HighGuid::Player, 0, character.guid);
    begin_db_creature_combat(session, attacker, Instant::now());
    send_packet(
        stream,
        SMSG_ATTACKSTART,
        &build_attack_start_body(attacker, player),
        Some(header_crypto),
    )
    .await?;
    send_db_creature_chase_if_needed(stream, session, attacker, player, Instant::now(), header_crypto)
        .await?;
    Ok(())
}

async fn send_active_db_creature_attack(
    stream: &mut TcpStream,
    session: &mut WorldSessionState,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    let Some(character) = &session.active_character else {
        return Ok(());
    };
    let Some(combat) = session.active_creature_combat else {
        return Ok(());
    };
    let attacker = combat.attacker;
    let player = ObjectGuid::new(HighGuid::Player, 0, character.guid);
    if combat.victim != player {
        session.active_creature_combat = None;
        return Ok(());
    }
    if !session
        .db_creatures
        .get(&attacker.raw())
        .is_some_and(DbCreatureRuntime::is_alive)
    {
        session.active_creature_combat = None;
        return Ok(());
    }
    let now = Instant::now();
    advance_db_creature_motion(session, attacker, now);
    if !db_creature_can_reach_player(session, attacker) {
        send_db_creature_chase_if_needed(stream, session, attacker, player, now, header_crypto)
            .await?;
        return Ok(());
    }
    if now < combat.next_swing_at {
        return Ok(());
    }

    let next_swing_delay = session
        .db_creatures
        .get(&attacker.raw())
        .map(DbCreatureRuntime::base_attack_duration)
        .unwrap_or_else(|| Duration::from_millis(RUST_COMBAT_SWING_MILLIS));
    let damage = retaliation_damage_for_db_creature(session, attacker);
    if damage == 0 {
        session.active_creature_combat = None;
        return Ok(());
    }
    if let Some(combat) = &mut session.active_creature_combat {
        combat.next_swing_at = now + next_swing_delay;
    }
    send_packet(
        stream,
        SMSG_ATTACKERSTATEUPDATE,
        &build_attacker_state_update_body(attacker, player, damage)?,
        Some(&mut *header_crypto),
    )
    .await?;
    send_packet(
        stream,
        SMSG_UPDATE_OBJECT,
        &build_player_health_update_body(player, session.player_health)?,
        Some(header_crypto),
    )
    .await
}

fn select_db_creature_aggro_target(session: &WorldSessionState) -> Option<ObjectGuid> {
    let character = session.active_character.as_ref()?;
    session
        .db_creatures
        .values()
        .filter(|creature| creature.can_aggro_player(character))
        .filter_map(|creature| {
            let distance_sq = creature.distance_to_player_squared(character)?;
            let attack_distance =
                db_creature_attack_distance(character.level, creature.spawn.template.min_level);
            (distance_sq <= attack_distance * attack_distance).then_some((distance_sq, creature.guid()))
        })
        .min_by(|(left_distance, left_guid), (right_distance, right_guid)| {
            left_distance
                .total_cmp(right_distance)
                .then_with(|| left_guid.raw().cmp(&right_guid.raw()))
        })
        .map(|(_, guid)| guid)
}

fn begin_db_creature_combat(session: &mut WorldSessionState, attacker: ObjectGuid, now: Instant) {
    let Some(character) = &session.active_character else {
        return;
    };
    session.active_creature_combat = Some(CreatureCombatState {
        attacker,
        victim: ObjectGuid::new(HighGuid::Player, 0, character.guid),
        next_swing_at: now,
    });
}

fn clear_db_creature_combat_if_attacker(session: &mut WorldSessionState, attacker: ObjectGuid) {
    if session
        .active_creature_combat
        .as_ref()
        .is_some_and(|combat| combat.attacker == attacker)
    {
        session.active_creature_combat = None;
    }
}

fn db_creature_attack_distance(player_level: u8, creature_level: u8) -> f32 {
    let mut level_diff = player_level as i32 - creature_level as i32;
    if level_diff < -25 {
        level_diff = -25;
    }
    (18.0 - level_diff as f32).max(5.0)
}

fn db_creature_can_reach_player(session: &WorldSessionState, attacker: ObjectGuid) -> bool {
    let Some(character) = &session.active_character else {
        return false;
    };
    let Some(creature) = session.db_creatures.get(&attacker.raw()) else {
        return false;
    };
    creature.distance_to_player_squared(character).is_some_and(|distance_sq| {
        distance_sq < DB_CREATURE_MELEE_REACH_YARDS * DB_CREATURE_MELEE_REACH_YARDS
    })
}

async fn send_db_creature_chase_if_needed(
    stream: &mut TcpStream,
    session: &mut WorldSessionState,
    attacker: ObjectGuid,
    player: ObjectGuid,
    now: Instant,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    if db_creature_can_reach_player(session, attacker) {
        return Ok(());
    }
    let Some((start, destination, spline_id, duration)) =
        start_db_creature_chase_motion(session, attacker, player, now)
    else {
        return Ok(());
    };
    send_packet(
        stream,
        SMSG_MONSTER_MOVE,
        &build_monster_move_body(
            attacker,
            start,
            destination,
            spline_id,
            duration.as_millis().max(1) as u32,
        )?,
        Some(header_crypto),
    )
    .await
}

fn advance_db_creature_motion(
    session: &mut WorldSessionState,
    creature_guid: ObjectGuid,
    now: Instant,
) {
    let Some(creature) = session.db_creatures.get_mut(&creature_guid.raw()) else {
        return;
    };
    let CreatureMotionState::Chase(chase) = &creature.motion else {
        return;
    };
    let elapsed = now.saturating_duration_since(chase.started_at);
    if elapsed >= chase.duration {
        creature.current_position = chase.destination;
        creature.motion = CreatureMotionState::Idle;
        return;
    }

    let duration_secs = chase.duration.as_secs_f32();
    if duration_secs <= f32::EPSILON {
        creature.current_position = chase.destination;
        creature.motion = CreatureMotionState::Idle;
        return;
    }

    let progress = (elapsed.as_secs_f32() / duration_secs).clamp(0.0, 1.0);
    creature.current_position = interpolate_position(chase.start, chase.destination, progress);
}

fn start_db_creature_chase_motion(
    session: &mut WorldSessionState,
    creature_guid: ObjectGuid,
    target: ObjectGuid,
    now: Instant,
) -> Option<(WorldPosition, WorldPosition, u32, Duration)> {
    let target_position = session.active_character.as_ref()?.position;
    let creature = session.db_creatures.get_mut(&creature_guid.raw())?;
    let start = creature.current_position;
    let destination = db_creature_chase_destination(start, target_position)?;
    if let CreatureMotionState::Chase(chase) = &creature.motion {
        if chase.target == target {
            if now < chase.recheck_at {
                return None;
            }
            let destination_delta = distance_2d(
                chase.destination.x,
                chase.destination.y,
                destination.x,
                destination.y,
            );
            if destination_delta <= DB_CREATURE_CHASE_REPATH_YARDS {
                return None;
            }
        }
    }
    let move_distance = distance_2d(start.x, start.y, destination.x, destination.y);
    if move_distance <= f32::EPSILON {
        return None;
    }
    let duration = Duration::from_millis(
        ((move_distance / DB_CREATURE_RUN_SPEED_YARDS_PER_SEC) * 1000.0)
            .ceil()
            .max(1.0) as u64,
    );
    let spline_id = creature.next_spline_id;
    creature.next_spline_id = creature.next_spline_id.wrapping_add(1);
    creature.motion = CreatureMotionState::Chase(CreatureChaseMotion {
        target,
        start,
        destination,
        started_at: now,
        duration,
        recheck_at: now + Duration::from_millis(DB_CREATURE_CHASE_RECHECK_MILLIS),
    });
    Some((start, destination, spline_id, duration))
}

fn db_creature_chase_destination(
    start: WorldPosition,
    target_position: WorldPosition,
) -> Option<WorldPosition> {
    if start.map_id != target_position.map_id {
        return None;
    }
    let dx = target_position.x - start.x;
    let dy = target_position.y - start.y;
    let distance = distance_2d(start.x, start.y, target_position.x, target_position.y);
    let stop_distance =
        DB_CREATURE_MELEE_REACH_YARDS * DB_CREATURE_CHASE_DEFAULT_RANGE_FACTOR;
    if distance <= stop_distance {
        return None;
    }
    let travel = distance - stop_distance;
    let nx = dx / distance;
    let ny = dy / distance;
    Some(WorldPosition::new(
        start.map_id,
        start.x + nx * travel,
        start.y + ny * travel,
        start.z,
        dy.atan2(dx),
    ))
}

fn interpolate_position(
    start: WorldPosition,
    destination: WorldPosition,
    progress: f32,
) -> WorldPosition {
    WorldPosition::new(
        start.map_id,
        start.x + (destination.x - start.x) * progress,
        start.y + (destination.y - start.y) * progress,
        start.z + (destination.z - start.z) * progress,
        destination.orientation,
    )
}

fn distance_2d(left_x: f32, left_y: f32, right_x: f32, right_y: f32) -> f32 {
    let dx = left_x - right_x;
    let dy = left_y - right_y;
    (dx * dx + dy * dy).sqrt()
}

async fn handle_attack_stop(
    stream: &mut TcpStream,
    session: &mut WorldSessionState,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    let Some(character) = &session.active_character else {
        return Ok(());
    };
    let attacker = ObjectGuid::new(HighGuid::Player, 0, character.guid);
    let victim = session.active_combat_target.unwrap_or_else(rust_combat_dummy_guid);
    session.active_combat_target = None;
    session.active_combat_next_swing_at = None;
    send_packet(
        stream,
        SMSG_ATTACKSTOP,
        &build_attack_stop_body(attacker, victim, false)?,
        Some(header_crypto),
    )
    .await
}

