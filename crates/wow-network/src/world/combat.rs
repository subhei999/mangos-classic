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
        let next_random_move_at = Self::initial_random_move_at(&spawn);
        let next_waypoint_move_at = Self::initial_waypoint_move_at(&spawn);
        Self {
            spawn,
            home_position,
            current_position: home_position,
            motion: CreatureMotionState::Idle,
            next_random_move_at,
            next_waypoint_move_at,
            waypoint_next_index: 0,
            waypoint_forward: true,
            already_called_assistance: false,
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

    fn is_evading_home(&self) -> bool {
        matches!(self.motion, CreatureMotionState::ReturnHome(_))
    }

    fn default_movement_type(&self) -> u8 {
        if self.spawn.movement_type != DB_MOTION_TYPE_IDLE {
            self.spawn.movement_type
        } else {
            self.spawn.template.movement_type
        }
    }

    fn random_wander_radius(&self) -> f32 {
        if self.default_movement_type() == DB_MOTION_TYPE_RANDOM {
            self.spawn.spawn_dist.max(0.0)
        } else {
            0.0
        }
    }

    fn has_waypoint_movement(&self) -> bool {
        matches!(
            self.default_movement_type(),
            DB_MOTION_TYPE_WAYPOINT | DB_MOTION_TYPE_LINEAR_WAYPOINT
        ) && !self.spawn.waypoint_path.is_empty()
    }

    fn initial_random_move_at(spawn: &CreatureSpawnQuery) -> Option<Instant> {
        let movement_type = if spawn.movement_type != DB_MOTION_TYPE_IDLE {
            spawn.movement_type
        } else {
            spawn.template.movement_type
        };
        (movement_type == DB_MOTION_TYPE_RANDOM && spawn.spawn_dist > 0.0).then(|| {
            Instant::now()
                + Duration::from_millis(db_creature_random_pause_millis(creature_spawn_guid(spawn).raw(), 0))
        })
    }

    fn initial_waypoint_move_at(spawn: &CreatureSpawnQuery) -> Option<Instant> {
        let movement_type = if spawn.movement_type != DB_MOTION_TYPE_IDLE {
            spawn.movement_type
        } else {
            spawn.template.movement_type
        };
        matches!(
            movement_type,
            DB_MOTION_TYPE_WAYPOINT | DB_MOTION_TYPE_LINEAR_WAYPOINT
        )
        .then_some(Instant::now())
        .filter(|_| !spawn.waypoint_path.is_empty())
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
        self.next_random_move_at = Self::initial_random_move_at(&self.spawn);
        self.next_waypoint_move_at = Self::initial_waypoint_move_at(&self.spawn);
        self.waypoint_next_index = 0;
        self.waypoint_forward = true;
        self.already_called_assistance = false;
    }

    fn can_aggro_player(&self, character: &ActiveCharacter) -> bool {
        self.is_alive()
            && !self.is_evading_home()
            && self.spawn.map == character.position.map_id
            && self.spawn.template.civilian == 0
            && self.spawn.template.creature_type != CREATURE_TYPE_CRITTER
            && self.spawn.template.npc_flags == 0
            && can_creature_attack_player_on_sight(self.spawn.template.faction, character.race)
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FactionReaction {
    Hostile,
    Neutral,
    Friendly,
}

#[derive(Debug, Clone, Copy)]
struct FactionTemplateEntry {
    faction: u32,
    faction_group_mask: u32,
    friend_group_mask: u32,
    enemy_group_mask: u32,
    enemy_faction: [u32; 4],
    friend_faction: [u32; 4],
}

const FACTION_GROUP_MASK_PLAYER: u32 = 1;
const FACTION_GROUP_MASK_ALLIANCE: u32 = 2;
const FACTION_GROUP_MASK_HORDE: u32 = 4;
const FACTION_GROUP_MASK_MONSTER: u32 = 8;

fn can_creature_attack_player_on_sight(creature_faction: u32, player_race: u8) -> bool {
    can_faction_attack_on_sight(creature_faction, faction_for_race(player_race))
}

fn can_faction_attack_on_sight(creature_faction: u32, player_faction: u32) -> bool {
    faction_reaction_to(creature_faction, player_faction) == FactionReaction::Hostile
}

fn faction_reaction_to(this_faction: u32, other_faction: u32) -> FactionReaction {
    let Some(this_template) = faction_template_entry(this_faction) else {
        return FactionReaction::Neutral;
    };
    let Some(other_template) = faction_template_entry(other_faction) else {
        return FactionReaction::Neutral;
    };
    faction_template_reaction(this_template, other_template)
}

fn faction_template_reaction(
    this_template: FactionTemplateEntry,
    other_template: FactionTemplateEntry,
) -> FactionReaction {
    if other_template.faction_group_mask & this_template.enemy_group_mask != 0 {
        return FactionReaction::Hostile;
    }
    if other_template.faction != 0
        && this_template
            .enemy_faction
            .contains(&other_template.faction)
    {
        return FactionReaction::Hostile;
    }
    if other_template.faction_group_mask & this_template.friend_group_mask != 0 {
        return FactionReaction::Friendly;
    }
    if other_template.faction != 0
        && this_template
            .friend_faction
            .contains(&other_template.faction)
    {
        return FactionReaction::Friendly;
    }
    if this_template.faction_group_mask & other_template.friend_group_mask != 0 {
        return FactionReaction::Friendly;
    }
    if this_template.faction != 0
        && other_template
            .friend_faction
            .contains(&this_template.faction)
    {
        return FactionReaction::Friendly;
    }
    FactionReaction::Neutral
}

fn faction_template_entry(id: u32) -> Option<FactionTemplateEntry> {
    match id {
        // Rust currently serializes generic player faction templates during
        // bootstrap. These preserve the CMaNGOS group-mask relation shape until
        // the real FactionTemplate.dbc loader is wired.
        1 => Some(faction_template(
            1,
            FACTION_GROUP_MASK_PLAYER | FACTION_GROUP_MASK_ALLIANCE,
            FACTION_GROUP_MASK_ALLIANCE,
            FACTION_GROUP_MASK_HORDE,
        )),
        2 => Some(faction_template(
            2,
            FACTION_GROUP_MASK_PLAYER | FACTION_GROUP_MASK_HORDE,
            FACTION_GROUP_MASK_HORDE,
            FACTION_GROUP_MASK_ALLIANCE,
        )),
        // Local RealClassicDb Northshire rows observed in creature_template:
        // 12 Marshal McBride/Llane Beshere friendly, 17 Defias Thug hostile,
        // 25 Kobold family hostile, 32 Young Wolf neutral. The fixture combat
        // factions keep the same relationship categories.
        12 | RUST_GUIDE_FACTION_TEMPLATE => Some(faction_template(
            id,
            FACTION_GROUP_MASK_ALLIANCE,
            FACTION_GROUP_MASK_PLAYER | FACTION_GROUP_MASK_ALLIANCE,
            FACTION_GROUP_MASK_HORDE,
        )),
        14 | 17 | 25 => Some(faction_template(
            id,
            FACTION_GROUP_MASK_MONSTER,
            FACTION_GROUP_MASK_MONSTER,
            FACTION_GROUP_MASK_PLAYER,
        )),
        32 => Some(faction_template(id, 0, 0, 0)),
        _ => None,
    }
}

fn faction_template(
    faction: u32,
    faction_group_mask: u32,
    friend_group_mask: u32,
    enemy_group_mask: u32,
) -> FactionTemplateEntry {
    FactionTemplateEntry {
        faction,
        faction_group_mask,
        friend_group_mask,
        enemy_group_mask,
        enemy_faction: [0; 4],
        friend_faction: [0; 4],
    }
}

fn apply_db_creature_damage(
    session: &mut WorldSessionState,
    target: ObjectGuid,
    requested_damage: u32,
) -> Option<u32> {
    let creature = session.db_creatures.get_mut(&target.raw())?;
    if !creature.is_alive() || creature.is_evading_home() {
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
    advance_db_creature_return_home_motions(session, now);
    advance_db_creature_idle_motions(stream, session, now, header_crypto).await?;
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

    try_start_db_creature_aggro(stream, session, header_crypto).await?;
    send_active_db_creature_attack(stream, session, header_crypto).await
}

fn advance_db_creature_return_home_motions(session: &mut WorldSessionState, now: Instant) {
    let return_home_guids = session
        .db_creatures
        .iter()
        .filter_map(|(guid, creature)| {
            matches!(creature.motion, CreatureMotionState::ReturnHome(_)).then_some(*guid)
        })
        .collect::<Vec<_>>();
    for guid in return_home_guids {
        advance_db_creature_motion(session, ObjectGuid::from_raw(guid), now);
    }
}

async fn advance_db_creature_idle_motions(
    stream: &mut TcpStream,
    session: &mut WorldSessionState,
    now: Instant,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    let idle_motion_guids = session
        .db_creatures
        .iter()
        .filter_map(|(guid, creature)| {
            should_tick_db_creature_idle_motion(session, *guid, creature, now).then_some(*guid)
        })
        .collect::<Vec<_>>();

    for guid in idle_motion_guids {
        let creature_guid = ObjectGuid::from_raw(guid);
        advance_db_creature_motion(session, creature_guid, now);
        let motion = start_db_creature_random_motion(session, creature_guid, now)
            .or_else(|| start_db_creature_waypoint_motion(session, creature_guid, now));
        if let Some(motion) = motion {
            send_packet(
                stream,
                SMSG_MONSTER_MOVE,
                &build_monster_move_walk_path_body(
                    creature_guid,
                    motion.start,
                    &motion.path,
                    motion.spline_id,
                    motion.duration.as_millis().max(1) as u32,
                )?,
                Some(&mut *header_crypto),
            )
            .await?;
        }
    }

    Ok(())
}

fn should_tick_db_creature_idle_motion(
    session: &WorldSessionState,
    guid: u64,
    creature: &DbCreatureRuntime,
    now: Instant,
) -> bool {
    creature.is_alive()
        && !session.active_creature_combats.contains_key(&guid)
        && session.active_combat_target.is_none_or(|target| target.raw() != guid)
        && (matches!(
            creature.motion,
            CreatureMotionState::Random(_) | CreatureMotionState::Waypoint(_)
        ) || creature.next_random_move_at.is_some_and(|at| now >= at)
            || creature.next_waypoint_move_at.is_some_and(|at| now >= at))
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
    match db_creature_player_melee_check(session, target) {
        PlayerMeleeCheck::Clear => {}
        PlayerMeleeCheck::MissingTarget | PlayerMeleeCheck::TargetNotAlive => {
            session.active_combat_target = None;
            session.active_combat_next_swing_at = None;
            return Ok(());
        }
        _ => {
            session.active_combat_next_swing_at =
                Some(Instant::now() + Duration::from_millis(PLAYER_MELEE_RETRY_MILLIS));
            return Ok(());
        }
    }
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
    if !is_dead {
        begin_db_creature_combat(session, target, Instant::now());
    }

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
    if session.active_creature_combats.is_empty() {
        send_player_combat_flag_if_changed(stream, session, false, header_crypto).await?;
    }
    send_db_creature_combat_flag(stream, session, killed, false, header_crypto).await?;
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
    let player = ObjectGuid::new(HighGuid::Player, 0, character.guid);
    for attacker in select_db_creature_aggro_targets(session) {
        if !begin_db_creature_combat(session, attacker, Instant::now()) {
            continue;
        }
        send_db_creature_combat_start(stream, session, attacker, player, header_crypto).await?;

        for assistant in select_db_creature_assist_targets(session, attacker) {
            if begin_db_creature_combat(session, assistant, Instant::now()) {
                send_db_creature_combat_start(stream, session, assistant, player, header_crypto)
                    .await?;
            }
        }
    }
    Ok(())
}

async fn send_db_creature_combat_start(
    stream: &mut TcpStream,
    session: &mut WorldSessionState,
    attacker: ObjectGuid,
    player: ObjectGuid,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    send_packet(
        stream,
        SMSG_ATTACKSTART,
        &build_attack_start_body(attacker, player),
        Some(&mut *header_crypto),
    )
    .await?;
    send_player_combat_flag_if_changed(stream, session, true, header_crypto).await?;
    send_db_creature_combat_flag(stream, session, attacker, true, header_crypto).await?;
    send_db_creature_chase_if_needed(
        stream,
        session,
        attacker,
        player,
        Instant::now(),
        header_crypto,
    )
    .await
}

async fn send_active_db_creature_attack(
    stream: &mut TcpStream,
    session: &mut WorldSessionState,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    let Some(character) = &session.active_character else {
        return Ok(());
    };
    let player = ObjectGuid::new(HighGuid::Player, 0, character.guid);
    let active_combats = session
        .active_creature_combats
        .values()
        .copied()
        .collect::<Vec<_>>();
    for combat in active_combats {
        send_single_active_db_creature_attack(stream, session, header_crypto, combat, player)
            .await?;
    }
    Ok(())
}

async fn send_single_active_db_creature_attack(
    stream: &mut TcpStream,
    session: &mut WorldSessionState,
    header_crypto: &mut HeaderCrypto,
    combat: CreatureCombatState,
    player: ObjectGuid,
) -> anyhow::Result<()> {
    let attacker = combat.attacker;
    if combat.victim != player {
        session.active_creature_combats.remove(&attacker.raw());
        return Ok(());
    }
    if !session
        .db_creatures
        .get(&attacker.raw())
        .is_some_and(DbCreatureRuntime::is_alive)
    {
        session.active_creature_combats.remove(&attacker.raw());
        return Ok(());
    }
    let now = Instant::now();
    advance_db_creature_motion(session, attacker, now);
    if db_creature_should_evade(session, attacker) {
        send_db_creature_evade_and_return_home(stream, session, attacker, player, now, header_crypto)
            .await?;
        return Ok(());
    }
    if !db_creature_can_reach_player(session, attacker) {
        defer_ready_db_creature_swing_retry(session, attacker, player, now);
        send_db_creature_chase_if_needed(stream, session, attacker, player, now, header_crypto)
            .await?;
        return Ok(());
    }
    if !db_creature_has_player_in_arc(session, attacker) {
        send_db_creature_face_target(stream, session, attacker, player, header_crypto).await?;
        defer_ready_db_creature_swing_retry(session, attacker, player, now);
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
        session.active_creature_combats.remove(&attacker.raw());
        return Ok(());
    }
    if let Some(combat) = session.active_creature_combats.get_mut(&attacker.raw()) {
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

fn defer_ready_db_creature_swing_retry(
    session: &mut WorldSessionState,
    attacker: ObjectGuid,
    victim: ObjectGuid,
    now: Instant,
) {
    let Some(combat) = session.active_creature_combats.get_mut(&attacker.raw()) else {
        return;
    };
    if combat.attacker == attacker && combat.victim == victim && now >= combat.next_swing_at {
        combat.next_swing_at = now + Duration::from_millis(DB_CREATURE_MELEE_RETRY_MILLIS);
    }
}

#[cfg(test)]
fn select_db_creature_aggro_target(session: &WorldSessionState) -> Option<ObjectGuid> {
    select_db_creature_aggro_targets(session).into_iter().next()
}

fn select_db_creature_aggro_targets(session: &WorldSessionState) -> Vec<ObjectGuid> {
    let Some(character) = session.active_character.as_ref() else {
        return Vec::new();
    };
    let mut targets = session
        .db_creatures
        .values()
        .filter(|creature| {
            !session
                .active_creature_combats
                .contains_key(&creature.guid().raw())
        })
        .filter(|creature| creature.can_aggro_player(character))
        .filter_map(|creature| {
            if !db_creature_navigation_check(
                &session.db_creature_navigation,
                creature.current_position,
                character.position,
            )
            .is_clear()
            {
                return None;
            }
            let distance_sq = creature.distance_to_player_squared(character)?;
            let attack_distance = db_creature_attack_distance(
                character.level,
                creature.spawn.template.min_level,
                creature.spawn.template.detection_range,
            );
            (distance_sq <= attack_distance * attack_distance).then_some((distance_sq, creature.guid()))
        })
        .collect::<Vec<_>>();
    targets.sort_by(|(left_distance, left_guid), (right_distance, right_guid)| {
            left_distance
                .total_cmp(right_distance)
                .then_with(|| left_guid.raw().cmp(&right_guid.raw()))
        });
    targets.into_iter().map(|(_, guid)| guid).collect()
}

fn select_db_creature_assist_targets(
    session: &mut WorldSessionState,
    caller_guid: ObjectGuid,
) -> Vec<ObjectGuid> {
    let Some(character) = session.active_character.as_ref() else {
        return Vec::new();
    };
    let Some(caller) = session.db_creatures.get_mut(&caller_guid.raw()) else {
        return Vec::new();
    };
    if caller.already_called_assistance {
        return Vec::new();
    }
    caller.already_called_assistance = true;
    let caller_position = caller.current_position;
    let caller_faction = caller.spawn.template.faction;
    let radius = if caller.spawn.template.call_for_help > 0 {
        caller.spawn.template.call_for_help as f32
    } else {
        DB_CREATURE_ASSISTANCE_RADIUS_YARDS
    };
    let mut targets = session
        .db_creatures
        .values()
        .filter(|creature| creature.guid() != caller_guid)
        .filter(|creature| {
            !session
                .active_creature_combats
                .contains_key(&creature.guid().raw())
        })
        .filter(|creature| creature.spawn.template.faction == caller_faction)
        .filter(|creature| creature.can_aggro_player(character))
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

fn begin_db_creature_combat(
    session: &mut WorldSessionState,
    attacker: ObjectGuid,
    now: Instant,
) -> bool {
    let Some(character) = &session.active_character else {
        return false;
    };
    let victim = ObjectGuid::new(HighGuid::Player, 0, character.guid);
    if session
        .active_creature_combats
        .get(&attacker.raw())
        .is_some_and(|combat| combat.victim == victim)
    {
        return false;
    }
    session.active_creature_combats.insert(
        attacker.raw(),
        CreatureCombatState {
            attacker,
            victim,
            next_swing_at: now,
        },
    );
    true
}

fn clear_db_creature_combat_if_attacker(session: &mut WorldSessionState, attacker: ObjectGuid) {
    session.active_creature_combats.remove(&attacker.raw());
}

async fn send_player_combat_flag_if_changed(
    stream: &mut TcpStream,
    session: &mut WorldSessionState,
    in_combat: bool,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    if session.player_in_combat == in_combat {
        return Ok(());
    }
    let Some(character) = &session.active_character else {
        return Ok(());
    };
    session.player_in_combat = in_combat;
    let player = ObjectGuid::new(HighGuid::Player, 0, character.guid);
    send_packet(
        stream,
        SMSG_UPDATE_OBJECT,
        &build_unit_flags_update_body(player, player_unit_flags(in_combat))?,
        Some(header_crypto),
    )
    .await
}

async fn send_db_creature_combat_flag(
    stream: &mut TcpStream,
    session: &WorldSessionState,
    creature_guid: ObjectGuid,
    in_combat: bool,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    let Some(creature) = session.db_creatures.get(&creature_guid.raw()) else {
        return Ok(());
    };
    send_packet(
        stream,
        SMSG_UPDATE_OBJECT,
        &build_unit_flags_update_body(creature_guid, db_creature_unit_flags(creature, in_combat))?,
        Some(header_crypto),
    )
    .await
}

fn player_unit_flags(in_combat: bool) -> u32 {
    UNIT_FLAG_PLAYER_CONTROLLED | (if in_combat { UNIT_FLAG_IN_COMBAT } else { 0 })
}

fn db_creature_unit_flags(creature: &DbCreatureRuntime, in_combat: bool) -> u32 {
    creature.spawn.template.unit_flags | (if in_combat { UNIT_FLAG_IN_COMBAT } else { 0 })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PlayerMeleeCheck {
    Clear,
    NoActiveCharacter,
    MissingTarget,
    TargetNotAlive,
    NavigationBlocked(DbCreatureNavigationResult),
    OutOfRange,
    BadFacing,
}

fn db_creature_player_melee_check(
    session: &WorldSessionState,
    target: ObjectGuid,
) -> PlayerMeleeCheck {
    let Some(character) = &session.active_character else {
        return PlayerMeleeCheck::NoActiveCharacter;
    };
    let Some(creature) = session.db_creatures.get(&target.raw()) else {
        return PlayerMeleeCheck::MissingTarget;
    };
    if !creature.is_alive() || creature.is_evading_home() {
        return PlayerMeleeCheck::TargetNotAlive;
    }
    let navigation = db_creature_navigation_check(
        &session.db_creature_navigation,
        character.position,
        creature.current_position,
    );
    if !navigation.is_clear() {
        return PlayerMeleeCheck::NavigationBlocked(navigation);
    }
    if !player_can_reach_with_melee_attack(character.position, creature.current_position) {
        return PlayerMeleeCheck::OutOfRange;
    }
    if !has_in_arc(
        character.position,
        creature.current_position,
        PLAYER_MELEE_ARC_RADIANS,
    ) {
        return PlayerMeleeCheck::BadFacing;
    }
    PlayerMeleeCheck::Clear
}

fn db_creature_attack_distance(player_level: u8, creature_level: u8, detection_range: u32) -> f32 {
    if detection_range == 0 {
        return 0.0;
    }
    let mut level_diff = player_level as i32 - creature_level as i32;
    if level_diff < -25 {
        level_diff = -25;
    }
    (detection_range as f32 - level_diff as f32).max(5.0)
}

fn player_can_reach_with_melee_attack(
    player_position: WorldPosition,
    target_position: WorldPosition,
) -> bool {
    if player_position.map_id != target_position.map_id {
        return false;
    }
    let dx = player_position.x - target_position.x;
    let dy = player_position.y - target_position.y;
    let dz = player_position.z - target_position.z;
    dx * dx + dy * dy + dz * dz < PLAYER_MELEE_REACH_YARDS * PLAYER_MELEE_REACH_YARDS
}

fn has_in_arc(source: WorldPosition, target: WorldPosition, arc: f32) -> bool {
    if source.map_id != target.map_id {
        return false;
    }
    let angle = normalize_orientation((target.y - source.y).atan2(target.x - source.x));
    let mut delta = normalize_orientation(angle - source.orientation);
    if delta > std::f32::consts::PI {
        delta -= 2.0 * std::f32::consts::PI;
    }
    delta >= -(arc / 2.0) && delta <= arc / 2.0
}

fn normalize_orientation(angle: f32) -> f32 {
    angle.rem_euclid(2.0 * std::f32::consts::PI)
}

fn db_creature_can_reach_player(session: &WorldSessionState, attacker: ObjectGuid) -> bool {
    let Some(character) = &session.active_character else {
        return false;
    };
    let Some(creature) = session.db_creatures.get(&attacker.raw()) else {
        return false;
    };
    if !db_creature_navigation_check(
        &session.db_creature_navigation,
        creature.current_position,
        character.position,
    )
    .is_clear()
    {
        return false;
    }
    creature.distance_to_player_squared(character).is_some_and(|distance_sq| {
        distance_sq < DB_CREATURE_MELEE_REACH_YARDS * DB_CREATURE_MELEE_REACH_YARDS
    })
}

fn db_creature_has_player_in_arc(session: &WorldSessionState, attacker: ObjectGuid) -> bool {
    let Some(character) = &session.active_character else {
        return false;
    };
    let Some(creature) = session.db_creatures.get(&attacker.raw()) else {
        return false;
    };
    has_in_arc(
        creature.current_position,
        character.position,
        PLAYER_MELEE_ARC_RADIANS,
    )
}

async fn send_db_creature_face_target(
    stream: &mut TcpStream,
    session: &mut WorldSessionState,
    attacker: ObjectGuid,
    player: ObjectGuid,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    let Some((position, spline_id)) = face_db_creature_toward_player(session, attacker) else {
        return Ok(());
    };
    send_packet(
        stream,
        SMSG_MONSTER_MOVE,
        &build_monster_move_facing_target_body(attacker, position, position, spline_id, 1, player)?,
        Some(header_crypto),
    )
    .await
}

fn face_db_creature_toward_player(
    session: &mut WorldSessionState,
    attacker: ObjectGuid,
) -> Option<(WorldPosition, u32)> {
    let character_position = session
        .active_character
        .as_ref()
        .map(|character| character.position)?;
    let creature = session.db_creatures.get_mut(&attacker.raw())?;
    let dx = character_position.x - creature.current_position.x;
    let dy = character_position.y - creature.current_position.y;
    creature.current_position.orientation = normalize_orientation(dy.atan2(dx));
    let spline_id = creature.next_spline_id;
    creature.next_spline_id = creature.next_spline_id.wrapping_add(1);
    Some((creature.current_position, spline_id))
}

fn db_creature_should_evade(session: &WorldSessionState, attacker: ObjectGuid) -> bool {
    let Some(creature) = session.db_creatures.get(&attacker.raw()) else {
        return false;
    };
    if matches!(creature.motion, CreatureMotionState::ReturnHome(_)) {
        return false;
    }
    distance_2d(
        creature.current_position.x,
        creature.current_position.y,
        creature.home_position.x,
        creature.home_position.y,
    ) > DB_CREATURE_LEASH_RADIUS_YARDS
}

async fn send_db_creature_evade_and_return_home(
    stream: &mut TcpStream,
    session: &mut WorldSessionState,
    attacker: ObjectGuid,
    player: ObjectGuid,
    now: Instant,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    prepare_db_creature_evade(session, attacker);

    send_packet(
        stream,
        SMSG_ATTACKSTOP,
        &build_attack_stop_body(attacker, player, false)?,
        Some(&mut *header_crypto),
    )
    .await?;
    if session.active_creature_combats.is_empty() {
        send_player_combat_flag_if_changed(stream, session, false, header_crypto).await?;
    }
    send_db_creature_combat_flag(stream, session, attacker, false, header_crypto).await?;
    let health = session
        .db_creatures
        .get(&attacker.raw())
        .map(|creature| creature.health)
        .unwrap_or_default();
    send_packet(
        stream,
        SMSG_UPDATE_OBJECT,
        &build_db_creature_state_update_body(attacker, health, 0)?,
        Some(&mut *header_crypto),
    )
    .await?;
    if let Some(motion) = start_db_creature_return_home_motion(session, attacker, now) {
        send_packet(
            stream,
            SMSG_MONSTER_MOVE,
            &build_monster_move_path_body_inner(
                attacker,
                motion.start,
                &motion.path,
                motion.spline_id,
                motion.duration.as_millis().max(1) as u32,
                None,
                true,
            )?,
            Some(header_crypto),
        )
        .await?;
    }
    Ok(())
}

fn prepare_db_creature_evade(session: &mut WorldSessionState, attacker: ObjectGuid) {
    if let Some(creature) = session.db_creatures.get_mut(&attacker.raw()) {
        creature.health = creature.max_health();
        creature.lootable = false;
        creature.looting = false;
        creature.loot_money_available = false;
        creature.loot_item = None;
    }
    if session.active_combat_target == Some(attacker) {
        session.active_combat_target = None;
        session.active_combat_next_swing_at = None;
    }
    clear_db_creature_combat_if_attacker(session, attacker);
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
    let Some(motion) =
        start_db_creature_chase_motion(session, attacker, player, now)
    else {
        return Ok(());
    };
    send_packet(
        stream,
        SMSG_MONSTER_MOVE,
        &build_monster_move_facing_target_path_body(
            attacker,
            motion.start,
            &motion.path,
            motion.spline_id,
            motion.duration.as_millis().max(1) as u32,
            player,
        )?,
        Some(header_crypto),
    )
    .await
}

#[derive(Debug, Clone)]
struct StartedCreatureMotion {
    start: WorldPosition,
    path: Vec<WorldPosition>,
    spline_id: u32,
    duration: Duration,
}

fn advance_db_creature_motion(
    session: &mut WorldSessionState,
    creature_guid: ObjectGuid,
    now: Instant,
) {
    let Some(creature) = session.db_creatures.get_mut(&creature_guid.raw()) else {
        return;
    };
    match &creature.motion {
        CreatureMotionState::Idle => {}
        CreatureMotionState::Random(random) => {
            let Some(position) = advance_timed_path_motion(
                random.start,
                &random.path,
                random.started_at,
                random.duration,
                now,
            ) else {
                creature.current_position = random.destination;
                creature.motion = CreatureMotionState::Idle;
                creature.next_random_move_at = Some(
                    now + Duration::from_millis(db_creature_random_pause_millis(
                        creature.guid().raw(),
                        creature.next_spline_id,
                    )),
                );
                creature.next_waypoint_move_at =
                    DbCreatureRuntime::initial_waypoint_move_at(&creature.spawn);
                return;
            };
            creature.current_position = position;
        }
        CreatureMotionState::Waypoint(waypoint) => {
            let Some(position) = advance_timed_path_motion(
                waypoint.start,
                &waypoint.path,
                waypoint.started_at,
                waypoint.duration,
                now,
            ) else {
                creature.current_position = waypoint.destination;
                let arrived_node = waypoint.node_index;
                let wait_time = creature
                    .spawn
                    .waypoint_path
                    .get(arrived_node)
                    .map(|node| node.wait_time)
                    .unwrap_or(0);
                advance_db_creature_waypoint_index(creature, arrived_node);
                creature.motion = CreatureMotionState::Idle;
                creature.next_waypoint_move_at =
                    Some(now + Duration::from_millis(wait_time as u64));
                return;
            };
            creature.current_position = position;
        }
        CreatureMotionState::Chase(chase) => {
            let Some(position) = advance_timed_path_motion(
                chase.start,
                &chase.path,
                chase.started_at,
                chase.duration,
                now,
            ) else {
                creature.current_position = chase.destination;
                creature.motion = CreatureMotionState::Idle;
                return;
            };
            creature.current_position = position;
        }
        CreatureMotionState::ReturnHome(return_home) => {
            let Some(position) = advance_timed_path_motion(
                return_home.start,
                &return_home.path,
                return_home.started_at,
                return_home.duration,
                now,
            ) else {
                creature.current_position = return_home.destination;
                creature.motion = CreatureMotionState::Idle;
                creature.already_called_assistance = false;
                creature.next_random_move_at = Some(
                    now + Duration::from_millis(db_creature_random_pause_millis(
                        creature.guid().raw(),
                        creature.next_spline_id,
                    )),
                );
                return;
            };
            creature.current_position = position;
        }
    }
}

fn advance_db_creature_waypoint_index(creature: &mut DbCreatureRuntime, arrived_node: usize) {
    let node_count = creature.spawn.waypoint_path.len();
    if node_count == 0 {
        creature.waypoint_next_index = 0;
        return;
    }
    if creature.default_movement_type() == DB_MOTION_TYPE_LINEAR_WAYPOINT && node_count > 1 {
        if creature.waypoint_forward && arrived_node + 1 >= node_count {
            creature.waypoint_forward = false;
        } else if !creature.waypoint_forward && arrived_node == 0 {
            creature.waypoint_forward = true;
        }
        creature.waypoint_next_index = if creature.waypoint_forward {
            arrived_node.saturating_add(1).min(node_count - 1)
        } else {
            arrived_node.saturating_sub(1)
        };
    } else {
        creature.waypoint_next_index = (arrived_node + 1) % node_count;
    }
}

fn start_db_creature_random_motion(
    session: &mut WorldSessionState,
    creature_guid: ObjectGuid,
    now: Instant,
) -> Option<StartedCreatureMotion> {
    let creature = session.db_creatures.get(&creature_guid.raw())?;
    if !matches!(creature.motion, CreatureMotionState::Idle) {
        return None;
    }
    let radius = creature.random_wander_radius();
    if radius <= 0.0 || creature.next_random_move_at.is_none_or(|at| now < at) {
        return None;
    }
    let start = creature.current_position;
    let raw_destination = db_creature_random_destination(
        creature.home_position,
        radius,
        creature.guid().raw(),
        creature.next_spline_id,
    )?;
    let path = db_creature_path_to_destination(
        &session.db_creature_navigation,
        start,
        raw_destination,
        CreaturePathMode::Full,
    )
    .unwrap_or_else(|| vec![raw_destination]);
    let destination = *path.last()?;
    let move_distance = distance_2d(start.x, start.y, destination.x, destination.y);
    if move_distance <= f32::EPSILON {
        session
            .db_creatures
            .get_mut(&creature_guid.raw())?
            .next_random_move_at =
            Some(now + Duration::from_millis(DB_CREATURE_RANDOM_DELAY_MIN_MILLIS));
        return None;
    }
    let duration = db_creature_walk_path_motion_duration(start, &path);
    let creature = session.db_creatures.get_mut(&creature_guid.raw())?;
    let spline_id = creature.next_spline_id;
    creature.next_spline_id = creature.next_spline_id.wrapping_add(1);
    creature.motion = CreatureMotionState::Random(CreatureRandomMotion {
        start,
        destination,
        path: path.clone(),
        started_at: now,
        duration,
    });
    creature.next_random_move_at = None;
    Some(StartedCreatureMotion {
        start,
        path,
        spline_id,
        duration,
    })
}

fn start_db_creature_waypoint_motion(
    session: &mut WorldSessionState,
    creature_guid: ObjectGuid,
    now: Instant,
) -> Option<StartedCreatureMotion> {
    let creature = session.db_creatures.get(&creature_guid.raw())?;
    if !matches!(creature.motion, CreatureMotionState::Idle) {
        return None;
    }
    if !creature.has_waypoint_movement() || creature.next_waypoint_move_at.is_none_or(|at| now < at)
    {
        return None;
    }
    let node_index = creature
        .waypoint_next_index
        .min(creature.spawn.waypoint_path.len().saturating_sub(1));
    let node = creature.spawn.waypoint_path.get(node_index)?;
    let start = creature.current_position;
    let raw_destination = WorldPosition::new(
        creature.spawn.map,
        node.position_x,
        node.position_y,
        node.position_z,
        node.orientation.unwrap_or(creature.current_position.orientation),
    );
    let path = db_creature_path_to_destination(
        &session.db_creature_navigation,
        start,
        raw_destination,
        CreaturePathMode::Full,
    )
    .unwrap_or_else(|| vec![raw_destination]);
    let destination = *path.last()?;
    let move_distance = distance_2d(start.x, start.y, destination.x, destination.y);
    if move_distance <= f32::EPSILON {
        let creature = session.db_creatures.get_mut(&creature_guid.raw())?;
        creature.current_position = destination;
        advance_db_creature_waypoint_index(creature, node_index);
        let wait_time = creature
            .spawn
            .waypoint_path
            .get(node_index)
            .map(|node| node.wait_time)
            .unwrap_or(0);
        creature.next_waypoint_move_at = Some(now + Duration::from_millis(wait_time as u64));
        return None;
    }
    let duration = db_creature_walk_path_motion_duration(start, &path);
    let creature = session.db_creatures.get_mut(&creature_guid.raw())?;
    let spline_id = creature.next_spline_id;
    creature.next_spline_id = creature.next_spline_id.wrapping_add(1);
    creature.motion = CreatureMotionState::Waypoint(CreatureWaypointMotion {
        node_index,
        start,
        destination,
        path: path.clone(),
        started_at: now,
        duration,
    });
    creature.next_waypoint_move_at = None;
    Some(StartedCreatureMotion {
        start,
        path,
        spline_id,
        duration,
    })
}

fn start_db_creature_chase_motion(
    session: &mut WorldSessionState,
    creature_guid: ObjectGuid,
    target: ObjectGuid,
    now: Instant,
) -> Option<StartedCreatureMotion> {
    let target_position = session.active_character.as_ref()?.position;
    let creature = session.db_creatures.get(&creature_guid.raw())?;
    let start = creature.current_position;
    let path = db_creature_chase_path(
        &session.db_creature_navigation,
        start,
        target_position,
    )?;
    let destination = *path.last()?;
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
    let duration = db_creature_path_motion_duration(start, &path);
    let creature = session.db_creatures.get_mut(&creature_guid.raw())?;
    let spline_id = creature.next_spline_id;
    creature.next_spline_id = creature.next_spline_id.wrapping_add(1);
    creature.motion = CreatureMotionState::Chase(CreatureChaseMotion {
        target,
        start,
        destination,
        path: path.clone(),
        started_at: now,
        duration,
        recheck_at: now + Duration::from_millis(DB_CREATURE_CHASE_RECHECK_MILLIS),
    });
    Some(StartedCreatureMotion {
        start,
        path,
        spline_id,
        duration,
    })
}

fn start_db_creature_return_home_motion(
    session: &mut WorldSessionState,
    creature_guid: ObjectGuid,
    now: Instant,
) -> Option<StartedCreatureMotion> {
    let creature = session.db_creatures.get(&creature_guid.raw())?;
    let start = creature.current_position;
    let raw_destination = creature.home_position;
    if start.map_id != raw_destination.map_id
        || !world_position_is_finite(start)
        || !world_position_is_finite(raw_destination)
    {
        let creature = session.db_creatures.get_mut(&creature_guid.raw())?;
        creature.current_position = raw_destination;
        creature.motion = CreatureMotionState::Idle;
        return None;
    }
    let path = db_creature_path_to_destination(
        &session.db_creature_navigation,
        start,
        raw_destination,
        CreaturePathMode::Full,
    )
    .unwrap_or_else(|| vec![raw_destination]);
    let destination = *path.last()?;
    let move_distance = distance_2d(start.x, start.y, destination.x, destination.y);
    if move_distance <= f32::EPSILON {
        let creature = session.db_creatures.get_mut(&creature_guid.raw())?;
        creature.current_position = destination;
        creature.motion = CreatureMotionState::Idle;
        return None;
    }
    let duration = db_creature_path_motion_duration(start, &path);
    let creature = session.db_creatures.get_mut(&creature_guid.raw())?;
    let spline_id = creature.next_spline_id;
    creature.next_spline_id = creature.next_spline_id.wrapping_add(1);
    creature.motion = CreatureMotionState::ReturnHome(CreatureReturnHomeMotion {
        start,
        destination,
        path: path.clone(),
        started_at: now,
        duration,
    });
    Some(StartedCreatureMotion {
        start,
        path,
        spline_id,
        duration,
    })
}

#[derive(Debug, Clone, Copy)]
enum CreaturePathMode {
    Full,
    StopShort(f32),
}

fn db_creature_chase_path(
    navigation: &DbCreatureNavigationGuardrail,
    start: WorldPosition,
    target_position: WorldPosition,
) -> Option<Vec<WorldPosition>> {
    if !db_creature_navigation_check(navigation, start, target_position).is_clear() {
        return None;
    }
    let stop_distance =
        DB_CREATURE_MELEE_REACH_YARDS * DB_CREATURE_CHASE_DEFAULT_RANGE_FACTOR;
    db_creature_path_to_destination(
        navigation,
        start,
        target_position,
        CreaturePathMode::StopShort(stop_distance),
    )
}

fn db_creature_path_to_destination(
    navigation: &DbCreatureNavigationGuardrail,
    start: WorldPosition,
    target_position: WorldPosition,
    mode: CreaturePathMode,
) -> Option<Vec<WorldPosition>> {
    if let Some(path) = db_creature_mmap_path(navigation, start, target_position, mode) {
        return Some(path);
    }
    db_creature_straight_path(start, target_position, mode)
}

fn db_creature_straight_path(
    start: WorldPosition,
    target_position: WorldPosition,
    mode: CreaturePathMode,
) -> Option<Vec<WorldPosition>> {
    if start.map_id != target_position.map_id
        || !world_position_is_finite(start)
        || !world_position_is_finite(target_position)
    {
        return None;
    }
    let dx = target_position.x - start.x;
    let dy = target_position.y - start.y;
    let distance = distance_2d(start.x, start.y, target_position.x, target_position.y);
    let travel = match mode {
        CreaturePathMode::Full => distance,
        CreaturePathMode::StopShort(stop_distance) => {
            if distance <= stop_distance {
                return None;
            }
            distance - stop_distance
        }
    };
    if travel <= f32::EPSILON || distance <= f32::EPSILON {
        return None;
    }
    let nx = dx / distance;
    let ny = dy / distance;
    Some(vec![WorldPosition::new(
        start.map_id,
        start.x + nx * travel,
        start.y + ny * travel,
        start.z,
        dy.atan2(dx),
    )])
}

fn db_creature_navigation_check(
    navigation: &DbCreatureNavigationGuardrail,
    start: WorldPosition,
    target_position: WorldPosition,
) -> DbCreatureNavigationResult {
    if start.map_id != target_position.map_id {
        return DbCreatureNavigationResult::MapMismatch;
    }
    if !world_position_is_finite(start) || !world_position_is_finite(target_position) {
        return DbCreatureNavigationResult::InvalidCoordinate;
    }
    if !navigation.line_of_sight_clear || !db_creature_has_line_of_sight(start, target_position) {
        return DbCreatureNavigationResult::LineOfSightBlocked;
    }
    if !navigation.path_available || !db_creature_has_valid_path(navigation, start, target_position)
    {
        return DbCreatureNavigationResult::PathUnavailable;
    }
    DbCreatureNavigationResult::Clear
}

fn world_position_is_finite(position: WorldPosition) -> bool {
    position.x.is_finite()
        && position.y.is_finite()
        && position.z.is_finite()
        && position.orientation.is_finite()
}

fn db_creature_has_line_of_sight(
    _start: WorldPosition,
    _target_position: WorldPosition,
) -> bool {
    true
}

fn db_creature_has_valid_path(
    navigation: &DbCreatureNavigationGuardrail,
    start: WorldPosition,
    target_position: WorldPosition,
) -> bool {
    if navigation.world_data_files.mmap_tiles.is_empty() {
        return true;
    }

    let map_id = start.map_id;
    let Some((start_tile_x, start_tile_y)) = mmap_tile_for_position(start) else {
        return false;
    };
    let Some((target_tile_x, target_tile_y)) = mmap_tile_for_position(target_position) else {
        return false;
    };

    navigation.world_data_files.has_mmap_support_for_map(map_id)
        && navigation
            .world_data_files
            .has_mmap_tile(map_id, start_tile_x, start_tile_y)
        && navigation
            .world_data_files
            .has_mmap_tile(map_id, target_tile_x, target_tile_y)
}

fn mmap_tile_for_position(position: WorldPosition) -> Option<(u32, u32)> {
    const MAX_NUMBER_OF_GRIDS: i32 = 64;
    const CENTER_GRID_ID: f32 = 32.0;
    const SIZE_OF_GRIDS: f32 = 533.333_3;

    if !world_position_is_finite(position) {
        return None;
    }
    let tile_x = (CENTER_GRID_ID - position.x / SIZE_OF_GRIDS) as i32;
    let tile_y = (CENTER_GRID_ID - position.y / SIZE_OF_GRIDS) as i32;
    (0..MAX_NUMBER_OF_GRIDS)
        .contains(&tile_x)
        .then_some(())?;
    (0..MAX_NUMBER_OF_GRIDS)
        .contains(&tile_y)
        .then_some(())?;
    Some((tile_x as u32, tile_y as u32))
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
struct NativeMmapPathPoint {
    x: f32,
    y: f32,
    z: f32,
}

extern "C" {
    fn wow_mmap_find_path(
        data_dir: *const std::os::raw::c_char,
        map_id: u32,
        start_tile_x: u32,
        start_tile_y: u32,
        target_tile_x: u32,
        target_tile_y: u32,
        start_x: f32,
        start_y: f32,
        start_z: f32,
        target_x: f32,
        target_y: f32,
        target_z: f32,
        out_points: *mut NativeMmapPathPoint,
        max_points: i32,
    ) -> i32;
}

#[cfg(test)]
fn db_creature_mmap_next_path_corner(
    navigation: &DbCreatureNavigationGuardrail,
    start: WorldPosition,
    target_position: WorldPosition,
) -> Option<WorldPosition> {
    db_creature_mmap_path(navigation, start, target_position, CreaturePathMode::Full)?
        .into_iter()
        .next()
}

fn db_creature_mmap_path(
    navigation: &DbCreatureNavigationGuardrail,
    start: WorldPosition,
    target_position: WorldPosition,
    mode: CreaturePathMode,
) -> Option<Vec<WorldPosition>> {
    const MAX_NATIVE_MMAP_PATH_POINTS: usize = 16;

    let data_dir = navigation.world_data_files.data_dir_for_native.as_ref()?;
    let (start_tile_x, start_tile_y) = mmap_tile_for_position(start)?;
    let (target_tile_x, target_tile_y) = mmap_tile_for_position(target_position)?;
    if !navigation.world_data_files.has_mmap_support_for_map(start.map_id)
        || !navigation
            .world_data_files
            .has_mmap_tile(start.map_id, start_tile_x, start_tile_y)
        || !navigation
            .world_data_files
            .has_mmap_tile(start.map_id, target_tile_x, target_tile_y)
    {
        return None;
    }

    let mut points = [NativeMmapPathPoint {
        x: 0.0,
        y: 0.0,
        z: 0.0,
    }; MAX_NATIVE_MMAP_PATH_POINTS];
    let count = unsafe {
        wow_mmap_find_path(
            data_dir.as_ptr(),
            start.map_id,
            start_tile_x,
            start_tile_y,
            target_tile_x,
            target_tile_y,
            start.x,
            start.y,
            start.z,
            target_position.x,
            target_position.y,
            target_position.z,
            points.as_mut_ptr(),
            MAX_NATIVE_MMAP_PATH_POINTS as i32,
        )
    };
    if count < 2 {
        return None;
    }

    let path = native_mmap_points_to_world_path(start, &points[..count as usize])?;
    db_creature_trim_path_for_mode(start, path, mode)
}

fn native_mmap_points_to_world_path(
    start: WorldPosition,
    points: &[NativeMmapPathPoint],
) -> Option<Vec<WorldPosition>> {
    let mut path = Vec::new();
    let mut previous = start;
    for point in points.iter().skip(1) {
        if !point.x.is_finite() || !point.y.is_finite() || !point.z.is_finite() {
            return None;
        }
        if distance_2d(previous.x, previous.y, point.x, point.y) <= f32::EPSILON {
            continue;
        }
        let position = WorldPosition::new(
            start.map_id,
            point.x,
            point.y,
            point.z,
            (point.y - previous.y).atan2(point.x - previous.x),
        );
        path.push(position);
        previous = position;
    }
    (!path.is_empty()).then_some(path)
}

fn db_creature_trim_path_for_mode(
    start: WorldPosition,
    path: Vec<WorldPosition>,
    mode: CreaturePathMode,
) -> Option<Vec<WorldPosition>> {
    match mode {
        CreaturePathMode::Full => (!path.is_empty()).then_some(path),
        CreaturePathMode::StopShort(stop_distance) => {
            db_creature_trim_path_to_travel_distance(start, path, stop_distance)
        }
    }
}

fn db_creature_trim_path_to_travel_distance(
    start: WorldPosition,
    path: Vec<WorldPosition>,
    stop_distance: f32,
) -> Option<Vec<WorldPosition>> {
    let total = path_distance_2d(start, &path);
    if total <= stop_distance {
        return None;
    }
    let target_distance = total - stop_distance;
    let mut remaining = target_distance;
    let mut previous = start;
    let mut trimmed = Vec::new();
    for point in path {
        let segment = distance_2d(previous.x, previous.y, point.x, point.y);
        if segment <= f32::EPSILON {
            previous = point;
            continue;
        }
        if remaining > segment {
            trimmed.push(point);
            remaining -= segment;
            previous = point;
            continue;
        }
        let progress = (remaining / segment).clamp(0.0, 1.0);
        if progress <= f32::EPSILON {
            break;
        }
        trimmed.push(interpolate_position(previous, point, progress));
        break;
    }
    (!trimmed.is_empty()).then_some(trimmed)
}

fn db_creature_path_motion_duration(start: WorldPosition, path: &[WorldPosition]) -> Duration {
    Duration::from_millis(
        ((path_distance_2d(start, path) / DB_CREATURE_RUN_SPEED_YARDS_PER_SEC) * 1000.0)
            .ceil()
            .max(1.0) as u64,
    )
}

fn db_creature_walk_path_motion_duration(
    start: WorldPosition,
    path: &[WorldPosition],
) -> Duration {
    Duration::from_millis(
        ((path_distance_2d(start, path) / DB_CREATURE_WALK_SPEED_YARDS_PER_SEC) * 1000.0)
            .ceil()
            .max(1.0) as u64,
    )
}

fn db_creature_random_destination(
    home: WorldPosition,
    radius: f32,
    guid: u64,
    spline_id: u32,
) -> Option<WorldPosition> {
    if radius <= 0.0 || !world_position_is_finite(home) {
        return None;
    }
    let angle_seed = db_creature_pseudo_random_unit(guid, spline_id, 0);
    let radius_seed = db_creature_pseudo_random_unit(guid, spline_id, 1);
    let angle = angle_seed * 2.0 * std::f32::consts::PI;
    let distance = radius * radius_seed.sqrt().clamp(0.2, 1.0);
    Some(WorldPosition::new(
        home.map_id,
        home.x + angle.cos() * distance,
        home.y + angle.sin() * distance,
        home.z,
        angle,
    ))
}

fn db_creature_random_pause_millis(guid: u64, spline_id: u32) -> u64 {
    let span = DB_CREATURE_RANDOM_DELAY_MAX_MILLIS - DB_CREATURE_RANDOM_DELAY_MIN_MILLIS;
    DB_CREATURE_RANDOM_DELAY_MIN_MILLIS
        + (db_creature_pseudo_random_unit(guid, spline_id, 2) * span as f32) as u64
}

fn db_creature_pseudo_random_unit(guid: u64, spline_id: u32, salt: u32) -> f32 {
    let mut value = guid
        .wrapping_mul(0x9E37_79B9_7F4A_7C15)
        .wrapping_add((spline_id as u64) << 32)
        .wrapping_add(salt as u64);
    value ^= value >> 33;
    value = value.wrapping_mul(0xFF51_AFD7_ED55_8CCD);
    value ^= value >> 33;
    ((value & 0xFFFF_FFFF) as f32) / (u32::MAX as f32)
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

fn advance_timed_path_motion(
    start: WorldPosition,
    path: &[WorldPosition],
    started_at: Instant,
    duration: Duration,
    now: Instant,
) -> Option<WorldPosition> {
    let elapsed = now.saturating_duration_since(started_at);
    if elapsed >= duration {
        return None;
    }
    let duration_secs = duration.as_secs_f32();
    if duration_secs <= f32::EPSILON || path.is_empty() {
        return None;
    }
    let travel_distance =
        path_distance_2d(start, path) * (elapsed.as_secs_f32() / duration_secs).clamp(0.0, 1.0);
    position_along_path(start, path, travel_distance)
}

fn position_along_path(
    start: WorldPosition,
    path: &[WorldPosition],
    mut travel_distance: f32,
) -> Option<WorldPosition> {
    let mut previous = start;
    for point in path {
        let segment = distance_2d(previous.x, previous.y, point.x, point.y);
        if segment <= f32::EPSILON {
            previous = *point;
            continue;
        }
        if travel_distance > segment {
            travel_distance -= segment;
            previous = *point;
            continue;
        }
        return Some(interpolate_position(
            previous,
            *point,
            (travel_distance / segment).clamp(0.0, 1.0),
        ));
    }
    path.last().copied()
}

fn path_distance_2d(start: WorldPosition, path: &[WorldPosition]) -> f32 {
    let mut distance = 0.0;
    let mut previous = start;
    for point in path {
        distance += distance_2d(previous.x, previous.y, point.x, point.y);
        previous = *point;
    }
    distance
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

