#[derive(Debug, Clone)]
pub struct PlayerbotSpawnConfig {
    pub guid: u32,
    pub name: String,
    pub race: u8,
    pub class: u8,
    pub gender: u8,
    pub level: u8,
    pub position: WorldPosition,
    pub travel_destination: Option<WorldPosition>,
    pub player_bytes: u32,
    pub player_bytes2: u32,
}

#[derive(Debug, Clone)]
struct PlayerbotRosterEntry {
    guid: u32,
    name: String,
    race: u8,
    gender: u8,
    class: u8,
}

#[derive(Debug, Default)]
struct PlayerbotRoster {
    bots: HashMap<u32, PlayerbotRosterEntry>,
}

impl PlayerbotRoster {
    fn insert(&mut self, bot: PlayerbotRosterEntry) {
        self.bots.insert(bot.guid, bot);
    }

    fn name_query(&self, guid: u32) -> Option<CharacterNameQuery> {
        let bot = self.bots.get(&guid)?;
        Some(CharacterNameQuery {
            guid: bot.guid,
            name: bot.name.clone(),
            race: bot.race,
            gender: bot.gender,
            class: bot.class,
        })
    }
}

async fn initialize_playerbots(
    maps: &Arc<MapRuntimeManager>,
    world_db_pool: &MySqlPool,
    configs: &[PlayerbotSpawnConfig],
) -> anyhow::Result<PlayerbotRoster> {
    let mut roster = PlayerbotRoster::default();
    let mut seen = HashSet::new();
    let mut stats_cache = HashMap::new();
    for config in configs {
        if !seen.insert(config.guid) {
            anyhow::bail!("duplicate playerbot guid {}", config.guid);
        }
        let stats_key = (config.race, config.class, config.level);
        let world_stats = match stats_cache.get(&stats_key) {
            Some(world_stats) => *world_stats,
            None => {
                let world_stats = wow_db::get_player_world_stats(
                    world_db_pool,
                    config.race,
                    config.class,
                    config.level,
                )
                .await?;
                stats_cache.insert(stats_key, world_stats);
                world_stats
            }
        };
        let runtime = build_playerbot_runtime(config, world_stats)?;
        maps.add_player(runtime).await?;
        roster.insert(PlayerbotRosterEntry {
            guid: config.guid,
            name: config.name.clone(),
            race: config.race,
            gender: config.gender,
            class: config.class,
        });
        crate::observability::record_playerbot_name(config.guid, config.name.clone());
        info!(
            guid = config.guid,
            name = %config.name,
            map = config.position.map_id,
            x = config.position.x,
            y = config.position.y,
            z = config.position.z,
            "Loaded playerbot actor into MapRuntime",
        );
    }
    Ok(roster)
}

fn build_playerbot_runtime(
    config: &PlayerbotSpawnConfig,
    world_stats: PlayerWorldStats,
) -> anyhow::Result<PlayerRuntime> {
    let max_health = world_stats.max_health().max(1);
    let max_mana = world_stats.max_mana();
    let combat_stats =
        player_combat_stats_for_values(config.class, config.level, &world_stats, &[]);
    Ok(PlayerRuntime {
        guid: config.guid,
        account_id: None,
        controller: PlayerController::Bot {
            bot_id: BotId(config.guid as u64),
        },
        bot_runtime: Some(PlayerbotRuntimeState {
            bot_id: BotId(config.guid as u64),
            home_position: config.position,
            next_think_at: Instant::now() + playerbot_next_roam_delay(config.guid, 0),
            next_combat_think_at: Instant::now() + playerbot_next_combat_think_delay(config.guid),
            active_leg: None,
            route: Vec::new(),
            travel_destination: config.travel_destination,
            engage_target: None,
            movement_start_retries_remaining: 0,
            roam_step: 0,
        }),
        selected_target: None,
        unit_target: None,
        active_combat_target: None,
        active_combat_next_swing_at: None,
        looting: false,
        death_state: PlayerDeathState::Alive,
        combo_target: None,
        combo_points: 0,
        position: config.position,
        movement_flags: 0,
        client_time: 0,
        server_time: 0,
        fall_time: 0,
        last_fall_z: None,
        last_fall_time: 0,
        environment: PlayerEnvironmentRuntime::default(),
        jump: JumpInfo::default(),
        cell: cell_coord_for_position(config.position),
        visible_objects: HashSet::new(),
        last_creature_visibility_position: None,
        last_gameobject_visibility_position: None,
        last_player_corpse_visibility_position: None,
        visual: PlayerVisualState {
            gender: config.gender,
            player_bytes: config.player_bytes,
            player_bytes2: config.player_bytes2,
            equipment_cache: None,
            guildid: None,
        },
        visible_equipment: [0; ENUM_EQUIPMENT_SLOTS],
        flags: 0,
        level: config.level,
        race: config.race,
        class: config.class,
        spirit: world_stats.stats[4],
        gender: config.gender,
        base_world_stats: world_stats,
        effective_world_stats: world_stats,
        health: max_health,
        max_health,
        xp: 0,
        power1: max_mana,
        max_power1: max_mana,
        last_mana_use_at: None,
        power2: 0,
        power4: create_power_for_class_power(config.class, POWER_ENERGY),
        max_power4: create_power_for_class_power(config.class, POWER_ENERGY),
        player_bytes: config.player_bytes,
        player_bytes2: config.player_bytes2,
        stand_state: PLAYER_STAND_STATE_STAND,
        active_spells: HashSet::new(),
        inventory: Vec::new(),
        quest_statuses: HashMap::new(),
        active_auras: Vec::new(),
        spell_global_cooldowns_until: HashMap::new(),
        spell_cooldowns_until: HashMap::new(),
        queued_next_melee_spell: None,
        base_combat_stats: combat_stats,
        combat_stats,
    })
}
