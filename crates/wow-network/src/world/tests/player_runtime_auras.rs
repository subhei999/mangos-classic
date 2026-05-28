fn test_player_runtime(guid: u32, session_id: SessionId, position: WorldPosition) -> PlayerRuntime {
    test_player_runtime_with_controller(guid, PlayerController::Client { session_id }, position)
}

fn test_bot_player_runtime(guid: u32, bot_id: BotId, position: WorldPosition) -> PlayerRuntime {
    let mut player =
        test_player_runtime_with_controller(guid, PlayerController::Bot { bot_id }, position);
    player.account_id = None;
    player.bot_runtime = Some(PlayerbotRuntimeState {
        bot_id,
        home_position: position,
        next_think_at: Instant::now() + PLAYERBOT_ROAM_THINK_INTERVAL,
        next_combat_think_at: Instant::now() + playerbot_next_combat_think_delay(guid),
        active_leg: None,
        route: Vec::new(),
        combat_enabled: true,
        local_roam_only: false,
        force_active: false,
        travel_destination: None,
        engage_target: None,
        movement_start_retries_remaining: 0,
        roam_step: 0,
    });
    player
}

fn test_player_runtime_with_controller(
    guid: u32,
    controller: PlayerController,
    position: WorldPosition,
) -> PlayerRuntime {
    let world_stats = PlayerWorldStats {
        base_health: 20,
        base_mana: 0,
        stats: [23, 20, 22, 20, 20],
        next_level_xp: 400,
    };
    PlayerRuntime {
        guid,
        account_id: Some(guid),
        controller,
        bot_runtime: None,
        selected_target: None,
        unit_target: None,
        farsight_target: None,
        active_combat_target: None,
        active_combat_attack_kind: PlayerAutoAttackKind::Melee,
        active_combat_next_swing_at: None,
        ranged_auto_attack_next_shot_at: None,
        in_combat: false,
        looting: false,
        position,
        movement_flags: 0,
        client_time: 0,
        server_time: 0,
        fall_time: 0,
        last_fall_z: None,
        last_fall_time: 0,
        environment: PlayerEnvironmentRuntime::default(),
        jump: JumpInfo::default(),
        cell: cell_coord_for_position(position),
        visible_objects: HashSet::new(),
        next_sight_aggro_check_at: None,
        last_sight_aggro_check_position: None,
        last_player_visibility_refresh_position: None,
        last_creature_visibility_position: None,
        last_gameobject_visibility_position: None,
        last_player_corpse_visibility_position: None,
        visual: PlayerVisualState {
            gender: 0,
            player_bytes: 0,
            player_bytes2: 0,
            equipment_cache: None,
            guildid: None,
        },
        visible_equipment: [0; ENUM_EQUIPMENT_SLOTS],
        flags: 0,
        death_state: PlayerDeathState::Alive,
        level: 1,
        race: 1,
        class: 1,
        spirit: 20,
        gender: 0,
        base_world_stats: world_stats,
        effective_world_stats: world_stats,
        health: 20,
        max_health: 20,
        xp: 0,
        rest_bonus: 0.0,
        power1: 0,
        max_power1: 0,
        last_mana_use_at: None,
        power2: 0,
        power4: 0,
        max_power4: POWER_ENERGY_DEFAULT,
        player_bytes: 0,
        player_bytes2: 0,
        aura_state: 0,
        reactive_defense_expires_at: None,
        reactive_overpower_expires_at: None,
        combo_target: None,
        combo_points: 0,
        stand_state: PLAYER_STAND_STATE_STAND,
        active_spells: HashSet::new(),
        inventory: Vec::new(),
        quest_statuses: HashMap::new(),
        explored_zones: [0; PLAYER_EXPLORED_ZONES_SIZE],
        active_auras: Vec::new(),
        spell_global_cooldowns_until: HashMap::new(),
        spell_cooldowns_until: HashMap::new(),
        spell_cooldown_categories: HashMap::new(),
        spell_cooldown_item_ids: HashMap::new(),
        queued_next_melee_spell: None,
        base_combat_stats: test_player_combat_stats(),
        combat_stats: test_player_combat_stats(),
    }
}

fn test_control_aura(modifier: AuraStatModifier, now: Instant) -> ActiveAura {
    ActiveAura {
        spell_id: 900_100,
        caster: ObjectGuid::new(HighGuid::Unit, 0, 900),
        level: 1,
        interrupt_flags: 0,
        positive: false,
        visible: true,
        duration_millis: Some(30_000),
        expires_at: Some(now + Duration::from_secs(30)),
        periodic_damage: None,
        periodic_regen: None,
        stat_modifiers: vec![modifier],
        proc_triggers: Vec::new(),
    }
}

#[test]
fn gm_flag_prevents_player_world_damage() {
    let mut player =
        test_player_runtime(7, SessionId(7), WorldPosition::new(0, 0.0, 0.0, 0.0, 0.0));
    player.flags |= PLAYER_FLAGS_GM;
    let player_guid = ObjectGuid::new(HighGuid::Player, 0, player.guid);
    let source = ObjectGuid::new(HighGuid::Unit, 0, 42);

    let applied = apply_player_runtime_world_damage(
        &mut player,
        player_guid,
        Some(source),
        10,
        WorldDamageKind::Melee,
        Instant::now(),
    )
    .unwrap();

    assert!(applied.is_none());
    assert_eq!(player.health, 20);
    assert_eq!(player.death_state, PlayerDeathState::Alive);
}

#[test]
fn lethal_player_world_damage_clears_rage_immediately() {
    let mut player =
        test_player_runtime(7, SessionId(7), WorldPosition::new(0, 0.0, 0.0, 0.0, 0.0));
    player.class = 1;
    player.power2 = 100;
    let player_guid = ObjectGuid::new(HighGuid::Player, 0, player.guid);

    let applied = apply_player_runtime_world_damage(
        &mut player,
        player_guid,
        Some(ObjectGuid::new(HighGuid::Unit, 0, 42)),
        999,
        WorldDamageKind::Melee,
        Instant::now(),
    )
    .unwrap()
    .expect("lethal damage should apply");

    assert!(applied.died);
    assert_eq!(player.power2, 0);
    let (values, trailing) =
        decode_values_update_block(&applied.health_packet.body[5..], player_guid);
    assert!(trailing.is_empty());
    assert_eq!(values[UNIT_FIELD_POWER2], Some(0));
}

#[test]
fn player_damage_uses_generic_school_absorb_and_mana_shield_auras() {
    let mut player =
        test_player_runtime(7, SessionId(7), WorldPosition::new(0, 0.0, 0.0, 0.0, 0.0));
    player.max_power1 = 20;
    player.power1 = 10;
    player.active_auras.push(ActiveAura {
        spell_id: 543,
        caster: ObjectGuid::new(HighGuid::Player, 0, 7),
        level: 12,
        interrupt_flags: 0,
        positive: true,
        visible: true,
        duration_millis: Some(30_000),
        expires_at: None,
        periodic_damage: None,
        periodic_regen: None,
        stat_modifiers: vec![AuraStatModifier::SchoolAbsorb {
            school_mask: 4,
            amount: 8,
        }],
        proc_triggers: Vec::new(),
    });
    player.active_auras.push(ActiveAura {
        spell_id: 1463,
        caster: ObjectGuid::new(HighGuid::Player, 0, 7),
        level: 12,
        interrupt_flags: 0,
        positive: true,
        visible: true,
        duration_millis: Some(60_000),
        expires_at: None,
        periodic_damage: None,
        periodic_regen: None,
        stat_modifiers: vec![AuraStatModifier::ManaShield {
            school_mask: u32::MAX,
            amount: 20,
            mana_multiplier_millis: 500,
        }],
        proc_triggers: Vec::new(),
    });
    let player_guid = ObjectGuid::new(HighGuid::Player, 0, 7);
    let source = ObjectGuid::new(HighGuid::Unit, 0, 42);

    let applied = apply_player_runtime_world_damage_with_school_mask(
        &mut player,
        player_guid,
        Some(source),
        14,
        WorldDamageKind::SpellDirect,
        4,
        Instant::now(),
    )
    .unwrap()
    .unwrap();

    assert_eq!(applied.applied_damage, 0);
    assert_eq!(player.health, 20);
    assert_eq!(player.power1, 7);
    assert!(applied.aura_packet.is_some());
    assert!(applied
        .direct_packets
        .iter()
        .any(|packet| packet.opcode == WorldOpcode::SmsgUpdateObject as u16));
    assert_eq!(player.active_auras.len(), 1);
    assert_eq!(
        player.active_auras[0].stat_modifiers,
        vec![AuraStatModifier::ManaShield {
            school_mask: u32::MAX,
            amount: 14,
            mana_multiplier_millis: 500,
        }]
    );
}

#[test]
fn absorb_auras_only_match_their_school_mask() {
    let mut player =
        test_player_runtime(7, SessionId(7), WorldPosition::new(0, 0.0, 0.0, 0.0, 0.0));
    player.active_auras.push(ActiveAura {
        spell_id: 543,
        caster: ObjectGuid::new(HighGuid::Player, 0, 7),
        level: 12,
        interrupt_flags: 0,
        positive: true,
        visible: true,
        duration_millis: Some(30_000),
        expires_at: None,
        periodic_damage: None,
        periodic_regen: None,
        stat_modifiers: vec![AuraStatModifier::SchoolAbsorb {
            school_mask: 4,
            amount: 8,
        }],
        proc_triggers: Vec::new(),
    });
    let player_guid = ObjectGuid::new(HighGuid::Player, 0, 7);

    let applied = apply_player_runtime_world_damage_with_school_mask(
        &mut player,
        player_guid,
        None,
        5,
        WorldDamageKind::SpellDirect,
        16,
        Instant::now(),
    )
    .unwrap()
    .unwrap();

    assert_eq!(applied.applied_damage, 5);
    assert_eq!(player.health, 15);
    assert_eq!(
        player.active_auras[0].stat_modifiers,
        vec![AuraStatModifier::SchoolAbsorb {
            school_mask: 4,
            amount: 8,
        }]
    );
}

#[test]
fn map_player_melee_damage_uses_mana_shield_physical_mask() {
    let now = Instant::now();
    let mut map = MapRuntime::new(0, 0);
    let mut player =
        test_player_runtime(7, SessionId(7), WorldPosition::new(0, 0.0, 0.0, 0.0, 0.0));
    player.base_world_stats.base_mana = 200;
    player.effective_world_stats.base_mana = 200;
    player.max_power1 = 200;
    player.power1 = 200;
    map.add_player(player).unwrap();
    let player_guid = ObjectGuid::new(HighGuid::Player, 0, 7);
    let aura = ActiveAura {
        spell_id: 1463,
        caster: player_guid,
        level: 12,
        interrupt_flags: 0,
        positive: true,
        visible: true,
        duration_millis: Some(60_000),
        expires_at: None,
        periodic_damage: None,
        periodic_regen: None,
        stat_modifiers: vec![AuraStatModifier::ManaShield {
            school_mask: SPELL_SCHOOL_MASK_NORMAL,
            amount: 120,
            mana_multiplier_millis: 2000,
        }],
        proc_triggers: Vec::new(),
    };
    map.apply_player_aura_replacing_spell_ids(7, aura, &[])
        .unwrap()
        .unwrap();

    let applied = map
        .apply_player_world_damage(
            player_guid,
            Some(ObjectGuid::new(HighGuid::Unit, 0, 42)),
            10,
            WorldDamageKind::Melee,
            now,
        )
        .unwrap()
        .unwrap();

    let player = map.players.get(&7).unwrap();
    assert_eq!(applied.applied_damage, 0);
    assert_eq!(player.health, 20);
    assert_eq!(player.power1, 180);
    assert!(applied.aura_packet.is_some());
}

#[test]
fn polymorph_transform_updates_creature_display_and_breaks_on_damage() {
    let now = Instant::now();
    let mut map = MapRuntime::new(0, 0);
    map.add_player(test_player_runtime(
        7,
        SessionId(7),
        WorldPosition::new(0, 0.0, 0.0, 0.0, 0.0),
    ))
    .unwrap();
    let mut spawn = test_creature_spawn(6);
    spawn.guid = 901_146;
    spawn.template.display_id1 = 123;
    let creature_guid = creature_spawn_guid(&spawn);
    map.creatures
        .insert(creature_guid.raw(), DbCreatureRuntime::new(spawn));
    let aura = ActiveAura {
        spell_id: 118,
        caster: ObjectGuid::new(HighGuid::Player, 0, 7),
        level: 12,
        interrupt_flags: AURA_INTERRUPT_FLAG_DAMAGE,
        positive: false,
        visible: true,
        duration_millis: Some(30_000),
        expires_at: Some(now + Duration::from_secs(30)),
        periodic_damage: None,
        periodic_regen: None,
        stat_modifiers: vec![
            AuraStatModifier::Confuse,
            AuraStatModifier::Transform {
                display_id: 100,
                creature_entry: 0,
            },
        ],
        proc_triggers: Vec::new(),
    };

    let event = map
        .apply_db_creature_aura_replacing_spell_ids(creature_guid, 7, aura, &[], None, None, now)
        .unwrap()
        .unwrap();
    let creature = map.creatures.get(&creature_guid.raw()).unwrap();
    assert_eq!(creature.aura_display_id_override, Some(100));
    assert_eq!(db_creature_effective_display_id(creature), 100);
    assert!(event
        .direct_packets
        .iter()
        .any(|packet| packet.opcode == WorldOpcode::SmsgUpdateObject as u16));
    assert!(
        event
            .direct_packets
            .iter()
            .all(|packet| packet.opcode != WorldOpcode::SmsgUpdateAuraDuration as u16),
        "CMaNGOS only sends SMSG_UPDATE_AURA_DURATION to the aura target when that target is a player"
    );

    let damage = map
        .apply_db_creature_damage(DbCreatureDamageRequest {
            creature_guid,
            killer: ObjectGuid::new(HighGuid::Player, 0, 7),
            damage: 1,
            now,
            now_epoch_secs: 1_000,
            exclude_character_guid: None,
            corpse_loot: None,
            melee_outcome: None,
            spell_damage_outcome: None,
            spell_id: Some(133),
            spell_school: 2,
            suppress_attacker_state: false,
        })
        .unwrap()
        .unwrap();

    let creature = map.creatures.get(&creature_guid.raw()).unwrap();
    assert!(creature.active_auras.is_empty());
    assert_eq!(creature.aura_display_id_override, None);
    assert_eq!(db_creature_effective_display_id(creature), 123);
    assert!(damage
        .direct_packets
        .iter()
        .any(|packet| packet.opcode == WorldOpcode::SmsgUpdateObject as u16));
}

#[test]
fn db_creature_polymorph_uses_no_pve_diminishing_group() {
    let mut template = test_spell_template(118);
    template.mechanic = MECHANIC_POLYMORPH;

    assert_eq!(
        spell_diminishing_group(&template),
        Some(DiminishingGroupRuntime::Polymorph)
    );
    assert_eq!(db_creature_spell_diminishing_group(&template), None);
}

#[test]
fn single_target_polymorph_replaces_previous_target_for_same_caster() {
    let now = Instant::now();
    let mut map = MapRuntime::new(0, 0);
    let caster = ObjectGuid::new(HighGuid::Player, 0, 7);
    let descriptor = SingleTargetAuraDescriptor {
        spell_id: 118,
        chain_root: 118,
        spell_family_name: SPELL_FAMILY_MAGE,
        spell_family_flags: 0x0100_0000,
        mechanic: MECHANIC_POLYMORPH,
    };
    let aura = ActiveAura {
        spell_id: 118,
        caster,
        level: 12,
        interrupt_flags: AURA_INTERRUPT_FLAG_DAMAGE,
        positive: false,
        visible: true,
        duration_millis: Some(30_000),
        expires_at: Some(now + Duration::from_secs(30)),
        periodic_damage: None,
        periodic_regen: None,
        stat_modifiers: vec![
            AuraStatModifier::Confuse,
            AuraStatModifier::Transform {
                display_id: 100,
                creature_entry: 0,
            },
        ],
        proc_triggers: Vec::new(),
    };

    let mut first = test_creature_spawn(6);
    first.guid = 401;
    let first_guid = creature_spawn_guid(&first);
    let mut second = test_creature_spawn(6);
    second.guid = 402;
    second.position_x = 5.0;
    let second_guid = creature_spawn_guid(&second);
    map.creatures
        .insert(first_guid.raw(), DbCreatureRuntime::new(first));
    map.creatures
        .insert(second_guid.raw(), DbCreatureRuntime::new(second));

    map.apply_db_creature_aura_replacing_spell_ids(
        first_guid,
        7,
        aura.clone(),
        &[],
        Some(descriptor),
        None,
        now,
    )
    .unwrap()
    .unwrap();
    map.apply_db_creature_aura_replacing_spell_ids(
        second_guid,
        7,
        aura,
        &[],
        Some(descriptor),
        None,
        now,
    )
    .unwrap()
    .unwrap();

    assert!(map
        .creatures
        .get(&first_guid.raw())
        .unwrap()
        .active_auras
        .is_empty());
    assert_eq!(
        map.creatures
            .get(&second_guid.raw())
            .unwrap()
            .active_auras
            .len(),
        1
    );
}

#[test]
fn polymorph_breaks_on_periodic_aura_damage() {
    let now = Instant::now();
    let mut map = MapRuntime::new(0, 0);
    let mut spawn = test_creature_spawn(6);
    spawn.guid = 403;
    let creature_guid = creature_spawn_guid(&spawn);
    let mut creature = DbCreatureRuntime::new(spawn);
    creature.health = 100;
    creature.active_auras.push(ActiveAura {
        spell_id: 118,
        caster: ObjectGuid::new(HighGuid::Player, 0, 7),
        level: 12,
        interrupt_flags: AURA_INTERRUPT_FLAG_DAMAGE,
        positive: false,
        visible: true,
        duration_millis: Some(30_000),
        expires_at: Some(now + Duration::from_secs(30)),
        periodic_damage: None,
        periodic_regen: None,
        stat_modifiers: vec![
            AuraStatModifier::Confuse,
            AuraStatModifier::Transform {
                display_id: 100,
                creature_entry: 0,
            },
        ],
        proc_triggers: Vec::new(),
    });
    creature.active_auras.push(ActiveAura {
        spell_id: 772,
        caster: ObjectGuid::new(HighGuid::Player, 0, 7),
        level: 1,
        interrupt_flags: 0,
        positive: false,
        visible: true,
        duration_millis: Some(9_000),
        expires_at: Some(now + Duration::from_secs(9)),
        periodic_damage: Some(PeriodicDamageAura {
            aura_name: SPELL_AURA_PERIODIC_DAMAGE,
            school: 1,
            damage_class: 2,
            attributes_ex2: 0,
            attributes_ex3: 0,
            caster_snapshot: SpellCombatUnitSnapshot {
                level: 1,
                class: 0,
                intellect: 0,
                resistances: [0; MAX_SPELL_SCHOOL],
            },
            profile: PeriodicDamageProfile::Flat,
            amount: 7,
            tick_millis: 3_000,
            next_tick_at: now,
        }),
        periodic_regen: None,
        stat_modifiers: Vec::new(),
        proc_triggers: Vec::new(),
    });
    map.creatures.insert(creature_guid.raw(), creature);

    map.advance_db_creature_auras(now, 1_000).unwrap();

    let creature = map.creatures.get(&creature_guid.raw()).unwrap();
    assert_eq!(creature.health, 93);
    assert!(creature
        .active_auras
        .iter()
        .all(|aura| aura.spell_id != 118));
    assert_eq!(creature.aura_display_id_override, None);
}

#[test]
fn polymorph_natural_expiration_clears_confused_motion_and_display() {
    let now = Instant::now();
    let mut map = MapRuntime::new(0, 0);
    map.add_player(test_player_runtime(
        7,
        SessionId(7),
        WorldPosition::new(0, 0.0, 0.0, 0.0, 0.0),
    ))
    .unwrap();
    let mut spawn = test_creature_spawn(6);
    spawn.guid = 404;
    spawn.position_x = 2.0;
    spawn.template.display_id1 = 123;
    let creature_guid = creature_spawn_guid(&spawn);
    map.creatures
        .insert(creature_guid.raw(), DbCreatureRuntime::new(spawn));
    let aura = ActiveAura {
        spell_id: 118,
        caster: ObjectGuid::new(HighGuid::Player, 0, 7),
        level: 12,
        interrupt_flags: AURA_INTERRUPT_FLAG_DAMAGE,
        positive: false,
        visible: true,
        duration_millis: Some(30_000),
        expires_at: Some(now + Duration::from_secs(30)),
        periodic_damage: None,
        periodic_regen: None,
        stat_modifiers: vec![
            AuraStatModifier::Confuse,
            AuraStatModifier::Transform {
                display_id: 100,
                creature_entry: 0,
            },
        ],
        proc_triggers: Vec::new(),
    };

    map.apply_db_creature_aura_replacing_spell_ids(creature_guid, 7, aura, &[], None, None, now)
        .unwrap()
        .expect("polymorph should apply");
    {
        let creature = map.creatures.get(&creature_guid.raw()).unwrap();
        assert_eq!(creature.aura_display_id_override, Some(100));
        assert!(active_aura_has_confuse(&creature.active_auras));
        assert!(
            matches!(creature.motion, CreatureMotionState::Confused(_))
                || creature.next_confused_move_at.is_some(),
            "polymorph should put the creature under confused motion control"
        );
    }

    map.advance_db_creature_auras(now + Duration::from_secs(31), 1_000)
        .unwrap();

    let creature = map.creatures.get(&creature_guid.raw()).unwrap();
    assert!(creature.active_auras.is_empty());
    assert_eq!(creature.aura_display_id_override, None);
    assert_eq!(db_creature_effective_display_id(creature), 123);
    assert_eq!(creature.next_confused_move_at, None);
    assert_eq!(creature.confused_origin, None);
    assert!(
        !matches!(creature.motion, CreatureMotionState::Confused(_)),
        "natural polymorph expiry must leave confused motion control"
    );
}

#[test]
fn polymorph_natural_expiration_reconciles_single_target_and_diminishing_trackers() {
    let now = Instant::now();
    let mut map = MapRuntime::new(0, 0);
    let caster = ObjectGuid::new(HighGuid::Player, 0, 7);
    let descriptor = SingleTargetAuraDescriptor {
        spell_id: 118,
        chain_root: 118,
        spell_family_name: SPELL_FAMILY_MAGE,
        spell_family_flags: 0x0100_0000,
        mechanic: MECHANIC_POLYMORPH,
    };
    let mut spawn = test_creature_spawn(6);
    spawn.guid = 407;
    let creature_guid = creature_spawn_guid(&spawn);
    map.creatures
        .insert(creature_guid.raw(), DbCreatureRuntime::new(spawn));
    let aura = ActiveAura {
        spell_id: 118,
        caster,
        level: 12,
        interrupt_flags: AURA_INTERRUPT_FLAG_DAMAGE,
        positive: false,
        visible: true,
        duration_millis: Some(30_000),
        expires_at: Some(now + Duration::from_secs(30)),
        periodic_damage: None,
        periodic_regen: None,
        stat_modifiers: vec![
            AuraStatModifier::Confuse,
            AuraStatModifier::Transform {
                display_id: 100,
                creature_entry: 0,
            },
        ],
        proc_triggers: Vec::new(),
    };

    map.apply_db_creature_aura_replacing_spell_ids(
        creature_guid,
        7,
        aura,
        &[],
        Some(descriptor),
        Some(DiminishingGroupRuntime::Polymorph),
        now,
    )
    .unwrap()
    .expect("polymorph should apply");
    assert_eq!(
        map.tracked_single_target_auras
            .get(&caster.raw())
            .map(Vec::len),
        Some(1)
    );
    assert!(map
        .active_diminishing_auras
        .contains_key(&(creature_guid.raw(), caster.raw(), 118)));

    map.advance_db_creature_auras(now + Duration::from_secs(31), 1_000)
        .unwrap();

    assert!(
        !map.tracked_single_target_auras.contains_key(&caster.raw()),
        "expired polymorph should not leave a stale single-target tracker"
    );
    assert!(
        !map.active_diminishing_auras
            .contains_key(&(creature_guid.raw(), caster.raw(), 118)),
        "expired polymorph should retire the active diminishing aura"
    );
    assert_eq!(
        map.diminishing_states
            .get(&creature_guid.raw())
            .and_then(|groups| groups.get(&DiminishingGroupRuntime::Polymorph))
            .map(|state| state.active_stack_count),
        Some(0)
    );
}

#[test]
fn db_creature_evade_removes_polymorph_aura_display_and_diminishing_tracker() {
    let now = Instant::now();
    let mut map = MapRuntime::new(0, 0);
    let caster = ObjectGuid::new(HighGuid::Player, 0, 7);
    let descriptor = SingleTargetAuraDescriptor {
        spell_id: 118,
        chain_root: 118,
        spell_family_name: SPELL_FAMILY_MAGE,
        spell_family_flags: 0x0100_0000,
        mechanic: MECHANIC_POLYMORPH,
    };
    let mut spawn = test_creature_spawn(6);
    spawn.guid = 408;
    spawn.template.display_id1 = 123;
    let creature_guid = creature_spawn_guid(&spawn);
    map.creatures
        .insert(creature_guid.raw(), DbCreatureRuntime::new(spawn));
    map.begin_db_creature_combat(creature_guid, caster, now)
        .unwrap();
    let aura = ActiveAura {
        spell_id: 118,
        caster,
        level: 12,
        interrupt_flags: AURA_INTERRUPT_FLAG_DAMAGE,
        positive: false,
        visible: true,
        duration_millis: Some(30_000),
        expires_at: Some(now + Duration::from_secs(30)),
        periodic_damage: None,
        periodic_regen: None,
        stat_modifiers: vec![
            AuraStatModifier::Confuse,
            AuraStatModifier::Transform {
                display_id: 100,
                creature_entry: 0,
            },
        ],
        proc_triggers: Vec::new(),
    };

    map.apply_db_creature_aura_replacing_spell_ids(
        creature_guid,
        7,
        aura,
        &[],
        Some(descriptor),
        Some(DiminishingGroupRuntime::Polymorph),
        now,
    )
    .unwrap()
    .expect("polymorph should apply");
    {
        let creature = map.creatures.get(&creature_guid.raw()).unwrap();
        assert_eq!(creature.aura_display_id_override, Some(100));
        assert_eq!(db_creature_effective_display_id(creature), 100);
        assert!(active_aura_has_confuse(&creature.active_auras));
    }
    assert!(map
        .active_diminishing_auras
        .contains_key(&(creature_guid.raw(), caster.raw(), 118)));

    let evaded = map
        .prepare_db_creature_evade(creature_guid)
        .expect("evade should reset the shared creature");

    assert!(evaded.active_auras.is_empty());
    assert_eq!(evaded.aura_display_id_override, None);
    assert_eq!(db_creature_effective_display_id(&evaded), 123);
    assert_eq!(evaded.next_confused_move_at, None);
    assert_eq!(evaded.confused_origin, None);
    assert!(!matches!(evaded.motion, CreatureMotionState::Confused(_)));
    assert!(!map.tracked_single_target_auras.contains_key(&caster.raw()));
    assert!(!map
        .active_diminishing_auras
        .contains_key(&(creature_guid.raw(), caster.raw(), 118)));
    assert_eq!(
        map.diminishing_states
            .get(&creature_guid.raw())
            .and_then(|groups| groups.get(&DiminishingGroupRuntime::Polymorph))
            .map(|state| state.active_stack_count),
        Some(0)
    );
}

#[tokio::test]
async fn transform_aura_resolves_creature_entry_to_display_id() {
    let object_mgr = ObjectMgr::default();
    let mut sheep = test_creature_template(16_372);
    sheep.display_id1 = 856;
    sheep.display_id2 = 0;
    object_mgr
        .prime_creature_template_for_test(16_372, Some(sheep))
        .await;
    let world_db_pool = MySqlPool::connect_lazy("mysql://root@127.0.0.1/world").unwrap();
    let mut aura = ActiveAura {
        spell_id: 118,
        caster: ObjectGuid::new(HighGuid::Player, 0, 7),
        level: 12,
        interrupt_flags: AURA_INTERRUPT_FLAG_DAMAGE,
        positive: false,
        visible: true,
        duration_millis: Some(30_000),
        expires_at: None,
        periodic_damage: None,
        periodic_regen: None,
        stat_modifiers: vec![AuraStatModifier::Transform {
            display_id: 0,
            creature_entry: 16_372,
        }],
        proc_triggers: Vec::new(),
    };

    resolve_active_aura_transform_displays(&object_mgr, &world_db_pool, &mut aura)
        .await
        .unwrap();

    assert_eq!(
        active_aura_transform_display_id(std::slice::from_ref(&aura)),
        Some(856)
    );
}

#[test]
fn blink_missing_client_destination_uses_front_leap_radius() {
    let mut maps = MapRuntimeManager::default();
    maps.spell_radii.insert(
        9,
        SpellRadiusEntry {
            radius: 20.0,
            radius_per_level: 0.0,
            max_radius: 20.0,
        },
    );
    let mut blink = test_spell_template(1953);
    blink.effect1 = SPELL_EFFECT_LEAP;
    blink.effect_radius_index1 = 9;
    blink.effect_implicit_target_a1 = TARGET_UNIT_CASTER;
    blink.effect_implicit_target_b1 = TARGET_LOCATION_CASTER_FRONT_LEAP;
    let effect = SpellInfo::from_template(&blink).effects[0];
    let session = WorldSessionState {
        character: CharacterSessionState {
            active_character: Some(ActiveCharacter {
                guid: 7,
                name: "Ada".to_string(),
                race: 1,
                class: 8,
                level: 20,
                xp: 0,
                position: WorldPosition::new(0, 10.0, 20.0, 30.0, 0.0),
                movement_flags: 0,
                client_time: 0,
                fall_time: 0,
                jump: JumpInfo::default(),
            }),
            ..CharacterSessionState::default()
        },
        ..WorldSessionState::default()
    };

    let destination =
        player_near_teleport_forward_destination(&maps, &session, &blink, effect, 0).unwrap();

    assert_eq!(destination.map_id, 0);
    assert!((destination.x - 30.0).abs() < f32::EPSILON);
    assert!((destination.y - 20.0).abs() < f32::EPSILON);
    assert!((destination.z - 30.0).abs() < f32::EPSILON);
}

#[test]
fn control_absorb_and_dispel_aura_metadata_comes_from_spell_template() {
    let mut blink = test_spell_template(1953);
    blink.effect1 = SPELL_EFFECT_LEAP;
    blink.effect_implicit_target_a1 = TARGET_UNIT_CASTER;
    assert_eq!(
        player_spell_cast_profile(&blink).map(|profile| profile.kind),
        Some(SpellCastKind::Teleport)
    );
    assert!(spell_template_coverage_issues(&blink).is_empty());

    let mut polymorph = test_spell_template(118);
    polymorph.dispel = 1;
    polymorph.aura_interrupt_flags = AURA_INTERRUPT_FLAG_DAMAGE;
    polymorph.effect1 = SPELL_EFFECT_APPLY_AURA;
    polymorph.effect_apply_aura_name1 = SPELL_AURA_MOD_CONFUSE;
    polymorph.effect_implicit_target_a1 = TARGET_UNIT_ENEMY;
    polymorph.effect2 = SPELL_EFFECT_APPLY_AURA;
    polymorph.effect_apply_aura_name2 = SPELL_AURA_TRANSFORM;
    polymorph.effect_base_points2 = 0;
    polymorph.effect_die_sides2 = 0;
    polymorph.effect_base_dice2 = 0;
    polymorph.effect_misc_value2 = 16372;
    polymorph.effect_implicit_target_a2 = TARGET_UNIT_ENEMY;
    let modifiers = spell_aura_stat_modifiers(
        &SpellInfo::from_template(&polymorph),
        test_spell_effect_value_context(&polymorph),
    );
    assert!(modifiers.contains(&AuraStatModifier::Confuse));
    assert!(modifiers.contains(&AuraStatModifier::Transform {
        display_id: 0,
        creature_entry: 16372,
    }));
    assert!(modifiers.contains(&AuraStatModifier::DispelType { dispel_type: 1 }));
    let aura = build_active_aura(
        &polymorph,
        ObjectGuid::new(HighGuid::Player, 0, 7),
        12,
        test_spell_effect_value_context(&polymorph),
        Instant::now(),
        None,
    );
    assert!(active_aura_has_hard_control(std::slice::from_ref(&aura)));
    assert!(!active_aura_blocks_movement(std::slice::from_ref(&aura)));
    assert_eq!(aura.interrupt_flags, AURA_INTERRUPT_FLAG_DAMAGE);
    assert_eq!(
        active_aura_transform_display_id(std::slice::from_ref(&aura)),
        None
    );

    let mut slow_fall = test_spell_template(130);
    slow_fall.effect1 = SPELL_EFFECT_APPLY_AURA;
    slow_fall.effect_apply_aura_name1 = SPELL_AURA_FEATHER_FALL;
    slow_fall.effect_implicit_target_a1 = TARGET_UNIT_CASTER;
    assert_eq!(
        spell_aura_stat_modifiers(
            &SpellInfo::from_template(&slow_fall),
            test_spell_effect_value_context(&slow_fall),
        ),
        vec![AuraStatModifier::FeatherFall]
    );

    let mut mana_shield = test_spell_template(1463);
    mana_shield.effect1 = SPELL_EFFECT_APPLY_AURA;
    mana_shield.effect_apply_aura_name1 = SPELL_AURA_MANA_SHIELD;
    mana_shield.effect_base_points1 = 119;
    mana_shield.effect_die_sides1 = 1;
    mana_shield.effect_misc_value1 = 1;
    mana_shield.effect_multiple_value1 = 2.0;
    assert_eq!(
        spell_aura_stat_modifiers(
            &SpellInfo::from_template(&mana_shield),
            test_spell_effect_value_context(&mana_shield),
        ),
        vec![AuraStatModifier::ManaShield {
            school_mask: SPELL_SCHOOL_MASK_NORMAL,
            amount: 120,
            mana_multiplier_millis: 2000,
        }]
    );
}

#[test]
fn player_dispel_removes_matching_aura_without_touching_other_dispel_types() {
    let now = Instant::now();
    let mut map = MapRuntime::new(0, 0);
    insert_map_runtime_player_for_test(&mut map, 7, WorldPosition::new(0, 0.0, 0.0, 0.0, 0.0));
    let player = map.players.get_mut(&7).unwrap();
    player.active_auras.push(ActiveAura {
        spell_id: 702,
        caster: ObjectGuid::new(HighGuid::Unit, 0, 99),
        level: 12,
        interrupt_flags: 0,
        positive: false,
        visible: true,
        duration_millis: Some(30_000),
        expires_at: None,
        periodic_damage: None,
        periodic_regen: None,
        stat_modifiers: vec![AuraStatModifier::DispelType { dispel_type: 2 }],
        proc_triggers: Vec::new(),
    });
    player.active_auras.push(ActiveAura {
        spell_id: 133,
        caster: ObjectGuid::new(HighGuid::Unit, 0, 99),
        level: 12,
        interrupt_flags: 0,
        positive: false,
        visible: true,
        duration_millis: Some(30_000),
        expires_at: None,
        periodic_damage: None,
        periodic_regen: None,
        stat_modifiers: vec![AuraStatModifier::DispelType { dispel_type: 1 }],
        proc_triggers: Vec::new(),
    });

    let event = map
        .remove_player_auras_by_dispel_type(7, 2, 1, now)
        .unwrap()
        .unwrap();

    assert_eq!(event.removed_spell_ids, vec![702]);
    let player = map.players.get(&7).unwrap();
    assert_eq!(player.active_auras.len(), 1);
    assert_eq!(player.active_auras[0].spell_id, 133);
}

fn test_player_combat_stats() -> PlayerCombatStats {
    let world_stats = PlayerWorldStats {
        base_health: 20,
        base_mana: 0,
        stats: [23, 20, 22, 20, 20],
        next_level_xp: 400,
    };
    player_combat_stats_for_values(1, 1, &world_stats, &[])
}

#[test]
fn map_owned_area_discovery_sets_explored_zone_bit_once() {
    let mut map = MapRuntime::new(0, 0);
    let position = WorldPosition::new(0, -8950.0, -130.0, 83.5, 0.0);
    map.add_player(test_player_runtime(7, SessionId(7), position))
        .unwrap();

    let event = map.discover_player_area(7, 64).unwrap().unwrap();

    assert_eq!(event.area_flag, 64);
    assert_eq!(event.offset, 2);
    assert_eq!(event.field_value, 1);
    assert_eq!(event.explored_zones[2], 1);
    assert_eq!(map.players.get(&7).unwrap().explored_zones[2], 1);
    assert_eq!(event.update_body[5], UPDATE_TYPE_VALUES);
    let (values, trailing) = decode_values_update_block(
        &event.update_body[5..],
        ObjectGuid::new(HighGuid::Player, 0, 7),
    );
    assert!(trailing.is_empty());
    assert_eq!(values[PLAYER_EXPLORED_ZONES_1 + 2], Some(1));

    assert!(map.discover_player_area(7, 64).unwrap().is_none());
}

#[test]
fn map_owned_player_heal_clamps_health_and_notifies_observers() {
    let mut map = MapRuntime::new(0, 0);
    let target_position = WorldPosition::new(0, -8950.0, -130.0, 83.5, 0.0);
    let observer_position = WorldPosition::new(0, -8952.0, -130.0, 83.5, 0.0);
    map.add_player(test_player_runtime(7, SessionId(7), target_position))
        .unwrap();
    map.add_player(test_player_runtime(8, SessionId(8), observer_position))
        .unwrap();
    map.players.get_mut(&7).unwrap().health = 12;
    map.players.get_mut(&7).unwrap().max_health = 20;

    let event = map
        .apply_player_heal(7, 50)
        .unwrap()
        .expect("living player should receive heal");

    assert_eq!(map.players.get(&7).unwrap().health, 20);
    assert_eq!(event.healed_character_guid, 7);
    assert_eq!(event.health, 20);
    assert_eq!(event.direct_session_id, SessionId(7));
    assert_eq!(event.direct_packets.len(), 1);
    assert_eq!(event.observer_packets.len(), 1);
    assert_eq!(event.observer_packets[0].0, SessionId(8));
}

#[test]
fn sinister_strike_spell_power_checks_and_spends_energy() {
    let now = Instant::now();
    let mut map = MapRuntime::new(0, 0);
    let position = WorldPosition::new(0, 0.0, 0.0, 0.0, 0.0);
    let mut player = test_player_runtime(7, SessionId(7), position);
    player.class = 4;
    player.power4 = 40;
    player.max_power4 = POWER_ENERGY_DEFAULT;
    map.add_player(player).unwrap();
    let profile = player_spell_cast_profile(&sinister_strike_spell_template()).unwrap();

    assert_eq!(
        map.player_spell_cast_failure(7, None, &profile, false, now),
        Some(SPELL_FAILED_NO_POWER)
    );

    map.players.get_mut(&7).unwrap().power4 = 100;
    assert_eq!(
        map.player_spell_cast_failure(7, None, &profile, false, now),
        None
    );
    assert_eq!(
        map.spend_player_spell_power(7, &profile, now, false),
        Ok(())
    );
    assert_eq!(map.players.get(&7).unwrap().power4, 55);
}

#[test]
fn map_owned_rogue_combo_points_accumulate_and_switch_targets() {
    let mut map = MapRuntime::new(0, 0);
    let position = WorldPosition::new(0, 0.0, 0.0, 0.0, 0.0);
    let mut player = test_player_runtime(7, SessionId(7), position);
    player.class = 4;
    player.player_bytes = 0x0403_0201;
    map.add_player(player).unwrap();
    let first_target = ObjectGuid::new(HighGuid::Unit, 6, 46);
    let second_target = ObjectGuid::new(HighGuid::Unit, 6, 47);

    let first = map
        .add_player_combo_points(7, first_target, 1)
        .expect("rogue should gain first combo point");
    assert_eq!(first.combo_target, first_target);
    assert_eq!(first.combo_points, 1);
    assert_eq!(first.player_bytes, 0x0403_0201);

    let capped = map
        .add_player_combo_points(7, first_target, 5)
        .expect("combo points should cap on same target");
    assert_eq!(capped.combo_points, 5);

    let switched = map
        .add_player_combo_points(7, second_target, 1)
        .expect("new target should replace combo target");
    assert_eq!(switched.combo_target, second_target);
    assert_eq!(switched.combo_points, 1);
}

#[test]
fn creature_dot_death_clears_auras_before_respawn() {
    let now = Instant::now();
    let mut map = MapRuntime::new(0, 0);
    let player_position = WorldPosition::new(0, -8950.0, -130.0, 83.5, 0.0);
    map.add_player(test_player_runtime(7, SessionId(7), player_position))
        .unwrap();
    let mut spawn = test_creature_spawn(6);
    spawn.guid = 901_100;
    spawn.template.min_level_health = 5;
    spawn.template.max_level_health = 5;
    let creature_guid = creature_spawn_guid(&spawn);
    let mut creature = DbCreatureRuntime::new(spawn);
    creature.motion = CreatureMotionState::Chase(CreatureChaseMotion {
        target: ObjectGuid::new(HighGuid::Player, 0, 7),
        start: creature.current_position,
        destination: player_position,
        path: vec![player_position],
        started_at: now,
        duration: Duration::from_secs(3),
        recheck_at: now + Duration::from_secs(1),
        run: true,
    });
    creature.active_auras.push(ActiveAura {
        spell_id: 772,
        caster: ObjectGuid::new(HighGuid::Player, 0, 7),
        level: 1,
        interrupt_flags: 0,
        positive: false,
        visible: true,
        duration_millis: Some(9_000),
        expires_at: Some(now + Duration::from_secs(9)),
        periodic_damage: Some(PeriodicDamageAura {
            aura_name: SPELL_AURA_PERIODIC_DAMAGE,
            school: 1,
            damage_class: 2,
            attributes_ex2: 0,
            attributes_ex3: 0,
            caster_snapshot: SpellCombatUnitSnapshot {
                level: 1,
                class: 0,
                intellect: 0,
                resistances: [0; MAX_SPELL_SCHOOL],
            },
            profile: PeriodicDamageProfile::Flat,
            amount: 7,
            tick_millis: 3_000,
            next_tick_at: now,
        }),
        periodic_regen: None,
        stat_modifiers: Vec::new(),
        proc_triggers: Vec::new(),
    });
    map.creatures.insert(creature_guid.raw(), creature);
    let player_guid = ObjectGuid::new(HighGuid::Player, 0, 7);
    assert!(map
        .begin_db_creature_combat(creature_guid, player_guid, now)
        .is_some());
    assert!(map.player_runtime_snapshot(7).unwrap().in_combat);

    let packets = map.advance_db_creature_auras(now, 1_000).unwrap();
    let creature = map.creatures.get(&creature_guid.raw()).unwrap();
    assert_eq!(creature.health, 0);
    assert_eq!(creature.life_state, DbCreatureLifeState::Corpse);
    assert!(matches!(creature.motion, CreatureMotionState::Idle));
    assert!(creature.active_auras.is_empty());
    assert!(!map
        .active_creature_combats
        .contains_key(&creature_guid.raw()));
    assert!(!map.player_runtime_snapshot(7).unwrap().in_combat);
    assert!(packets
        .iter()
        .any(|(_, packet)| packet.opcode == WorldOpcode::SmsgPeriodicAuraLog as u16));
    assert!(packets
        .iter()
        .any(|(_, packet)| packet.opcode == WorldOpcode::SmsgMonsterMove as u16));
    let player_combat_clear_body =
        build_unit_flags_update_body(player_guid, UNIT_FLAG_PLAYER_CONTROLLED).unwrap();
    assert!(packets.iter().any(|(session_id, packet)| {
        *session_id == SessionId(7)
            && packet.opcode == WorldOpcode::SmsgUpdateObject as u16
            && packet.body == player_combat_clear_body
    }));

    let generation_after_death = creature.life_generation;
    let creature = map.creatures.get_mut(&creature_guid.raw()).unwrap();
    creature.remove_corpse();
    creature.respawn(now);
    assert!(creature.active_auras.is_empty());
    assert!(creature.life_generation > generation_after_death);

    let packets = map
        .advance_db_creature_auras(now + Duration::from_secs(3), 1_003)
        .unwrap();
    assert!(packets
        .iter()
        .all(|(_, packet)| packet.opcode != WorldOpcode::SmsgPeriodicAuraLog as u16));
}

#[tokio::test]
async fn pending_spell_impact_drops_after_target_respawn_generation_changes() {
    let maps = MapRuntimeManager::default();
    let mut spawn = test_creature_spawn(6);
    spawn.guid = 901_101;
    let creature_guid = creature_spawn_guid(&spawn);
    maps.share_db_creature_snapshots(0, vec![DbCreatureRuntime::new(spawn)])
        .await;
    let now = Instant::now();
    maps.push_pending_spell_event(
        0,
        7,
        133,
        PendingSpellCastTargets {
            target_mask: SPELL_CAST_TARGET_UNIT,
            unit_target: Some(creature_guid),
            gameobject_target: None,
            source_location: None,
            destination: None,
        },
        None,
        now,
    )
    .await;

    let map = { maps.maps.lock().await.get(&(0, 0)).cloned().unwrap() };
    {
        let mut map = map.lock().await;
        let creature = map.creatures.get_mut(&creature_guid.raw()).unwrap();
        creature.begin_corpse(now, 1_000);
        creature.remove_corpse();
        creature.respawn(Instant::now());
    }

    assert!(maps.take_due_pending_spell_event(0, 7, now).await.is_none());
}

#[test]
fn map_owned_player_aura_applies_attack_power_mod_and_expires() {
    let mut map = MapRuntime::new(0, 0);
    let position = WorldPosition::new(0, -8950.0, -130.0, 83.5, 0.0);
    map.add_player(test_player_runtime(7, SessionId(7), position))
        .unwrap();
    let base_attack_power = map
        .players
        .get(&7)
        .unwrap()
        .base_combat_stats
        .melee_attack_power;
    let now = Instant::now();
    let aura = ActiveAura {
        spell_id: 6673,
        caster: ObjectGuid::new(HighGuid::Player, 0, 7),
        level: 3,
        interrupt_flags: 0,
        positive: true,
        visible: true,
        duration_millis: Some(2_000),
        expires_at: Some(now + Duration::from_secs(2)),
        periodic_damage: None,
        periodic_regen: None,
        stat_modifiers: vec![AuraStatModifier::AttackPower { amount: 15 }],
        proc_triggers: Vec::new(),
    };

    let event = map.apply_player_aura(7, aura).unwrap().unwrap();
    let player = map.players.get(&7).unwrap();
    assert_eq!(event.direct_packets.len(), 4);
    let duration_packet = event
        .direct_packets
        .iter()
        .find(|packet| packet.opcode == WorldOpcode::SmsgUpdateAuraDuration as u16)
        .expect("timed aura apply should send owner duration update");
    assert_eq!(duration_packet.body[0], 0);
    assert!(u32::from_le_bytes(duration_packet.body[1..5].try_into().unwrap()) <= 2_000);
    assert_eq!(player.active_auras.len(), 1);
    assert_eq!(player.combat_stats.melee_attack_power, base_attack_power);
    assert_eq!(player.combat_stats.melee_attack_power_mod_positive, 15);

    assert!(map
        .advance_player_aura_expirations(now + Duration::from_secs(1))
        .unwrap()
        .is_empty());
    let packets = map
        .advance_player_aura_expirations(now + Duration::from_secs(2))
        .unwrap();
    let player = map.players.get(&7).unwrap();
    assert_eq!(packets.len(), 3);
    assert!(player.active_auras.is_empty());
    assert_eq!(player.combat_stats.melee_attack_power, base_attack_power);
    assert_eq!(player.combat_stats.melee_attack_power_mod_positive, 0);
}

#[test]
fn map_owned_resistance_aura_updates_character_panel_fields() {
    let mut map = MapRuntime::new(0, 0);
    let position = WorldPosition::new(0, -8950.0, -130.0, 83.5, 0.0);
    map.add_player(test_player_runtime(7, SessionId(7), position))
        .unwrap();
    let base = map.players.get(&7).unwrap().base_combat_stats;
    let now = Instant::now();
    let aura = ActiveAura {
        spell_id: 168,
        caster: ObjectGuid::new(HighGuid::Player, 0, 7),
        level: 1,
        interrupt_flags: 0,
        positive: true,
        visible: true,
        duration_millis: Some(1_800_000),
        expires_at: Some(now + Duration::from_secs(1_800)),
        periodic_damage: None,
        periodic_regen: None,
        stat_modifiers: vec![AuraStatModifier::Resistance {
            school_mask: (1 << 0) | (1 << 4),
            amount: 30,
        }],
        proc_triggers: Vec::new(),
    };

    let event = map.apply_player_aura(7, aura).unwrap().unwrap();
    let player = map.players.get(&7).unwrap();
    assert_eq!(player.combat_stats.resistances[0], base.resistances[0] + 30);
    assert_eq!(player.combat_stats.resistances[4], base.resistances[4] + 30);
    assert_eq!(player.combat_stats.armor, base.armor + 30);
    assert_eq!(player.combat_stats.resistance_buff_mod_positive[0], 30);
    assert_eq!(player.combat_stats.resistance_buff_mod_positive[4], 30);

    let combat_update = event
        .direct_packets
        .iter()
        .filter(|packet| packet.opcode == WorldOpcode::SmsgUpdateObject as u16)
        .map(|packet| {
            decode_values_update_block(&packet.body[5..], ObjectGuid::new(HighGuid::Player, 0, 7)).0
        })
        .find(|values| values[UNIT_FIELD_RESISTANCES].is_some())
        .expect("resistance aura should send character-panel combat stat fields");
    assert_eq!(
        combat_update[UNIT_FIELD_RESISTANCES],
        Some(base.resistances[0] + 30)
    );
    assert_eq!(
        combat_update[UNIT_FIELD_RESISTANCES + 4],
        Some(base.resistances[4] + 30)
    );
    assert_eq!(
        combat_update[PLAYER_FIELD_RESISTANCEBUFFMODSPOSITIVE],
        Some(30)
    );
    assert_eq!(
        combat_update[PLAYER_FIELD_RESISTANCEBUFFMODSPOSITIVE + 4],
        Some(30)
    );
}

#[test]
fn spell_aura_mod_stat_and_resistance_use_generic_template_metadata() {
    let mut stat_template = test_spell_template(1459);
    stat_template.effect1 = SPELL_EFFECT_APPLY_AURA;
    stat_template.effect_apply_aura_name1 = SPELL_AURA_MOD_STAT;
    stat_template.effect_base_points1 = 4;
    stat_template.effect_misc_value1 = 3;
    let stat_aura = build_active_aura(
        &stat_template,
        ObjectGuid::new(HighGuid::Player, 0, 7),
        1,
        test_spell_effect_value_context(&stat_template),
        Instant::now(),
        None,
    );
    assert_eq!(
        stat_aura.stat_modifiers,
        vec![AuraStatModifier::Stat {
            stat: Some(3),
            amount: 5
        }]
    );

    let world_stats = PlayerWorldStats {
        base_health: 20,
        base_mana: 20,
        stats: [10, 11, 12, 13, 14],
        next_level_xp: 400,
    };
    let effective = player_world_stats_with_active_auras(world_stats, &[stat_aura]);
    assert_eq!(effective.stats, [10, 11, 12, 18, 14]);
    let player_guid = ObjectGuid::new(HighGuid::Player, 0, 7);
    let world_stats_body =
        build_player_world_stats_update_body(7, &world_stats, &effective, 20, 20).unwrap();
    let (values, trailing) = decode_values_update_block(&world_stats_body[5..], player_guid);
    assert!(trailing.is_empty());
    assert_eq!(values[UNIT_FIELD_STAT0 + 3], Some(18));
    assert_eq!(values[PLAYER_FIELD_POSSTAT0 + 3], Some(5));
    assert_eq!(values[UNIT_FIELD_MAXPOWER1], Some(effective.max_mana()));
    let zero_health_body =
        build_player_world_stats_update_body(7, &world_stats, &effective, 0, 20).unwrap();
    let (values, trailing) = decode_values_update_block(&zero_health_body[5..], player_guid);
    assert!(trailing.is_empty());
    assert_eq!(
        values[UNIT_FIELD_HEALTH],
        Some(0),
        "aura/stat refreshes after death must not resurrect the client to 1 HP"
    );

    let mut resistance_template = test_spell_template(687);
    resistance_template.effect1 = SPELL_EFFECT_APPLY_AURA;
    resistance_template.effect_apply_aura_name1 = SPELL_AURA_MOD_RESISTANCE;
    resistance_template.effect_base_points1 = 19;
    resistance_template.effect_misc_value1 = (1 << 0) | (1 << 5);
    let resistance_aura = build_active_aura(
        &resistance_template,
        ObjectGuid::new(HighGuid::Player, 0, 7),
        1,
        test_spell_effect_value_context(&resistance_template),
        Instant::now(),
        None,
    );
    assert_eq!(
        resistance_aura.stat_modifiers,
        vec![AuraStatModifier::Resistance {
            school_mask: (1 << 0) | (1 << 5),
            amount: 20
        }]
    );

    let mut proc_template = test_spell_template(168);
    proc_template.effect1 = SPELL_EFFECT_APPLY_AURA;
    proc_template.effect_apply_aura_name1 = SPELL_AURA_MOD_RESISTANCE;
    proc_template.effect_base_points1 = 29;
    proc_template.effect_misc_value1 = 1;
    proc_template.effect2 = SPELL_EFFECT_APPLY_AURA;
    proc_template.effect_apply_aura_name2 = SPELL_AURA_PROC_TRIGGER_SPELL;
    proc_template.effect_trigger_spell2 = 6136;
    proc_template.proc_flags = PROC_FLAG_TAKE_MELEE_SWING;
    proc_template.proc_chance = 100;
    let proc_aura = build_active_aura(
        &proc_template,
        ObjectGuid::new(HighGuid::Player, 0, 7),
        1,
        test_spell_effect_value_context(&proc_template),
        Instant::now(),
        None,
    );
    assert_eq!(
        proc_aura.proc_triggers,
        vec![AuraProcTrigger {
            triggered_spell_id: 6136,
            proc_flags: PROC_FLAG_TAKE_MELEE_SWING,
            proc_ex: 0,
            proc_chance: 100,
            remaining_charges: None,
        }]
    );
}

#[test]
fn chilled_aura_template_modifies_movement_and_attack_speed() {
    let mut chilled_template = test_spell_template(6136);
    chilled_template.effect1 = SPELL_EFFECT_APPLY_AURA;
    chilled_template.effect_apply_aura_name1 = SPELL_AURA_MOD_MELEE_HASTE;
    chilled_template.effect_base_points1 = -26;
    chilled_template.effect2 = SPELL_EFFECT_APPLY_AURA;
    chilled_template.effect_apply_aura_name2 = SPELL_AURA_MOD_DECREASE_SPEED;
    chilled_template.effect_base_points2 = -31;

    let chilled = build_active_aura(
        &chilled_template,
        ObjectGuid::new(HighGuid::Player, 0, 7),
        1,
        test_spell_effect_value_context(&chilled_template),
        Instant::now(),
        None,
    );
    assert_eq!(
        chilled.stat_modifiers,
        vec![
            AuraStatModifier::MeleeAttackTimePercent { percent: -25 },
            AuraStatModifier::MoveSpeedPercent { percent: -30 },
        ]
    );
    assert_eq!(
        active_aura_melee_attack_time_multiplier(std::slice::from_ref(&chilled)),
        1.25
    );
    assert_eq!(active_aura_movement_speed_multiplier(&[chilled]), 0.7);
}

#[test]
fn root_aura_template_stops_movement_until_expiration() {
    let now = Instant::now();
    let mut root_template = test_spell_template(122);
    root_template.effect1 = SPELL_EFFECT_APPLY_AURA;
    root_template.effect_apply_aura_name1 = SPELL_AURA_MOD_ROOT;
    let root = build_active_aura(
        &root_template,
        ObjectGuid::new(HighGuid::Player, 0, 7),
        1,
        test_spell_effect_value_context(&root_template),
        now,
        Some(SpellDurationEntry {
            duration_millis: 1_000,
            duration_per_level_millis: 0,
            max_duration_millis: 1_000,
        }),
    );

    assert_eq!(root.stat_modifiers, vec![AuraStatModifier::Root]);
    assert!(active_aura_has_root(std::slice::from_ref(&root)));
    assert_eq!(
        active_aura_movement_speed_multiplier(std::slice::from_ref(&root)),
        0.0
    );

    let mut map = MapRuntime::new(0, 0);
    map.add_player(test_player_runtime(
        7,
        SessionId(7),
        WorldPosition::new(0, -8950.0, -130.0, 83.5, 0.0),
    ))
    .unwrap();

    let event = map.apply_player_aura(7, root).unwrap().unwrap();
    assert!(event
        .direct_packets
        .iter()
        .any(|packet| packet.opcode == WorldOpcode::SmsgForceMoveRoot as u16));

    assert!(map
        .advance_player_aura_expirations(now + Duration::from_millis(999))
        .unwrap()
        .is_empty());
    let packets = map
        .advance_player_aura_expirations(now + Duration::from_millis(1_000))
        .unwrap();
    assert!(packets
        .iter()
        .any(|(_, packet)| packet.opcode == WorldOpcode::SmsgForceMoveUnroot as u16));
    assert!(!active_aura_has_root(
        &map.players.get(&7).unwrap().active_auras
    ));
}

#[test]
fn movement_speed_aura_forces_owner_run_speed_until_expiration() {
    let now = Instant::now();
    let mut map = MapRuntime::new(0, 0);
    map.add_player(test_player_runtime(
        7,
        SessionId(7),
        WorldPosition::new(0, -8950.0, -130.0, 83.5, 0.0),
    ))
    .unwrap();
    let slow = ActiveAura {
        spell_id: 1604,
        caster: ObjectGuid::new(HighGuid::Unit, 6, 45),
        level: 6,
        interrupt_flags: 0,
        positive: false,
        visible: true,
        duration_millis: Some(4_000),
        expires_at: Some(now + Duration::from_secs(4)),
        periodic_damage: None,
        periodic_regen: None,
        stat_modifiers: vec![AuraStatModifier::MoveSpeedPercent { percent: -50 }],
        proc_triggers: Vec::new(),
    };

    let event = map.apply_player_aura(7, slow).unwrap().unwrap();
    let apply_speed_packet = event
        .direct_packets
        .iter()
        .find(|packet| packet.opcode == WorldOpcode::SmsgForceRunSpeedChange as u16)
        .expect("speed slow should force the owning client's run speed");
    assert_eq!(
        f32::from_le_bytes(
            apply_speed_packet.body[apply_speed_packet.body.len() - 4..]
                .try_into()
                .unwrap()
        ),
        PLAYER_AURA_BASE_RUN_SPEED_YARDS_PER_SEC * 0.5
    );

    let packets = map
        .advance_player_aura_expirations(now + Duration::from_secs(4))
        .unwrap();
    let restore_packet = packets
        .iter()
        .map(|(_, packet)| packet)
        .find(|packet| packet.opcode == WorldOpcode::SmsgForceRunSpeedChange as u16)
        .expect("speed slow expiration should restore the owning client's run speed");
    assert_eq!(
        f32::from_le_bytes(
            restore_packet.body[restore_packet.body.len() - 4..]
                .try_into()
                .unwrap()
        ),
        PLAYER_AURA_BASE_RUN_SPEED_YARDS_PER_SEC
    );
}

#[test]
fn utility_visibility_auras_use_generic_template_metadata() {
    let caster = ObjectGuid::new(HighGuid::Unit, 6, 68);
    let now = Instant::now();

    let mut guard_detection = test_spell_template(18950);
    guard_detection.spell_name = "Invisibility and Stealth Detection".to_string();
    guard_detection.effect1 = SPELL_EFFECT_APPLY_AURA;
    guard_detection.effect_apply_aura_name1 = SPELL_AURA_MOD_INVISIBILITY_DETECTION;
    guard_detection.effect_base_points1 = 99_998;
    guard_detection.effect_misc_value1 = 1;
    guard_detection.effect2 = SPELL_EFFECT_APPLY_AURA;
    guard_detection.effect_apply_aura_name2 = SPELL_AURA_MOD_STEALTH_DETECT;
    guard_detection.effect_base_points2 = 99_998;
    guard_detection.effect_misc_value2 = 0;
    let aura = build_active_aura(
        &guard_detection,
        caster,
        60,
        test_spell_effect_value_context(&guard_detection),
        now,
        None,
    );
    assert_eq!(
        aura.stat_modifiers,
        vec![
            AuraStatModifier::InvisibilityDetect {
                kind: 1,
                amount: 99_999,
            },
            AuraStatModifier::StealthDetect {
                kind: 0,
                amount: 99_999,
            },
        ]
    );
    assert!(spell_template_coverage_issues(&guard_detection).is_empty());

    let mut perception = test_spell_template(20600);
    perception.spell_name = "Perception".to_string();
    perception.effect1 = SPELL_EFFECT_APPLY_AURA;
    perception.effect_apply_aura_name1 = SPELL_AURA_MOD_STEALTH_DETECT;
    perception.effect_base_points1 = 49;
    let aura = build_active_aura(
        &perception,
        ObjectGuid::new(HighGuid::Player, 0, 7),
        1,
        test_spell_effect_value_context(&perception),
        now,
        None,
    );
    assert_eq!(
        aura.stat_modifiers,
        vec![AuraStatModifier::StealthDetect {
            kind: 0,
            amount: 50,
        }]
    );
    assert!(matches!(
        spell_aura_support(SPELL_AURA_MOD_STEALTH_DETECT),
        SpellMechanicSupport::Implemented
    ));

    let mut stealth = test_spell_template(1784);
    stealth.spell_name = "Stealth".to_string();
    stealth.effect1 = SPELL_EFFECT_APPLY_AURA;
    stealth.effect_apply_aura_name1 = SPELL_AURA_MOD_STEALTH;
    stealth.effect_base_points1 = 4;
    stealth.effect_misc_value1 = 0;
    let aura = build_active_aura(
        &stealth,
        ObjectGuid::new(HighGuid::Player, 0, 7),
        1,
        test_spell_effect_value_context(&stealth),
        now,
        None,
    );
    assert_eq!(
        aura.stat_modifiers,
        vec![AuraStatModifier::Stealth { kind: 0, amount: 5 }]
    );
    assert_eq!(
        active_aura_unit_vis_flags(std::slice::from_ref(&aura)),
        UNIT_VIS_FLAG_CREEP
    );
    assert_eq!(
        active_aura_player_field_bytes2(std::slice::from_ref(&aura)),
        PLAYER_FIELD_BYTE2_STEALTH << 8
    );

    let player = ObjectGuid::new(HighGuid::Player, 0, 7);
    let body = build_player_aura_update_body(
        player,
        4,
        PLAYER_STAND_STATE_STAND,
        0,
        std::slice::from_ref(&aura),
    )
    .unwrap();
    let (values, trailing) = decode_values_update_block(&body[5..], player);
    assert!(trailing.is_empty());
    assert_eq!(values[UNIT_FIELD_BYTES_1], Some(UNIT_VIS_FLAG_CREEP << 24));
    assert_eq!(
        values[PLAYER_FIELD_BYTES2],
        Some(PLAYER_FIELD_BYTE2_STEALTH << 8)
    );
    assert!(matches!(
        spell_aura_support(SPELL_AURA_MOD_STEALTH),
        SpellMechanicSupport::Implemented
    ));

    let mut shroud = test_spell_template(10848);
    shroud.spell_name = "Shroud of Death".to_string();
    shroud.effect1 = SPELL_EFFECT_APPLY_AURA;
    shroud.effect_apply_aura_name1 = SPELL_AURA_DUMMY;
    shroud.effect_misc_value1 = 42;
    let aura = build_active_aura(
        &shroud,
        caster,
        60,
        test_spell_effect_value_context(&shroud),
        now,
        None,
    );
    assert_eq!(
        aura.stat_modifiers,
        vec![AuraStatModifier::Dummy {
            aura_name: SPELL_AURA_DUMMY,
            misc_value: 42,
            amount: 1,
        }]
    );
}

#[test]
fn tracking_auras_update_player_tracking_fields() {
    let mut tracking = test_spell_template(999_440);
    tracking.effect1 = SPELL_EFFECT_APPLY_AURA;
    tracking.effect_apply_aura_name1 = SPELL_AURA_TRACK_CREATURES;
    tracking.effect_misc_value1 = 1;
    tracking.effect2 = SPELL_EFFECT_APPLY_AURA;
    tracking.effect_apply_aura_name2 = SPELL_AURA_TRACK_CREATURES;
    tracking.effect_misc_value2 = 8;
    tracking.effect3 = SPELL_EFFECT_APPLY_AURA;
    tracking.effect_apply_aura_name3 = SPELL_AURA_TRACK_RESOURCES;
    tracking.effect_misc_value3 = 2;
    let aura = build_active_aura(
        &tracking,
        ObjectGuid::new(HighGuid::Player, 0, 7),
        1,
        test_spell_effect_value_context(&tracking),
        Instant::now(),
        None,
    );

    assert_eq!(
        active_aura_track_creatures_mask(std::slice::from_ref(&aura)),
        (1 << 0) | (1 << 7)
    );
    assert_eq!(
        active_aura_track_resources_mask(std::slice::from_ref(&aura)),
        1 << 1
    );

    let player = ObjectGuid::new(HighGuid::Player, 0, 7);
    let body = build_player_aura_update_body(
        player,
        1,
        PLAYER_STAND_STATE_STAND,
        spell_aura_state_mask(AURA_STATE_DEFENSE),
        &[aura],
    )
    .unwrap();
    let (values, trailing) = decode_values_update_block(&body[5..], player);
    assert!(trailing.is_empty());
    assert_eq!(
        values[UNIT_FIELD_AURASTATE],
        Some(spell_aura_state_mask(AURA_STATE_DEFENSE))
    );
    assert_eq!(values[PLAYER_TRACK_CREATURES], Some((1 << 0) | (1 << 7)));
    assert_eq!(values[PLAYER_TRACK_RESOURCES], Some(1 << 1));
}

#[test]
fn defensive_stance_template_builds_shapeshift_modifier() {
    let mut stance = test_spell_template(71);
    stance.spell_name = "Defensive Stance".to_string();
    stance.effect1 = SPELL_EFFECT_APPLY_AURA;
    stance.effect_apply_aura_name1 = SPELL_AURA_MOD_SHAPESHIFT;
    stance.effect_misc_value1 = i32::from(FORM_DEFENSIVESTANCE);

    let aura = build_active_aura(
        &stance,
        ObjectGuid::new(HighGuid::Player, 0, 7),
        10,
        test_spell_effect_value_context(&stance),
        Instant::now(),
        None,
    );

    assert_eq!(
        aura.stat_modifiers,
        vec![AuraStatModifier::Shapeshift {
            form: FORM_DEFENSIVESTANCE,
        }]
    );
    assert!(matches!(
        spell_aura_support(SPELL_AURA_MOD_SHAPESHIFT),
        SpellMechanicSupport::Implemented
    ));
}

#[test]
fn defensive_stance_passive_template_builds_combat_modifiers() {
    let mut passive = test_spell_template(7376);
    passive.spell_name = "Defensive Stance Passive".to_string();
    passive.effect1 = SPELL_EFFECT_APPLY_AURA;
    passive.effect_apply_aura_name1 = SPELL_AURA_MOD_DAMAGE_PERCENT_TAKEN;
    passive.effect_base_points1 = -11;
    passive.effect_misc_value1 = 127;
    passive.effect2 = SPELL_EFFECT_APPLY_AURA;
    passive.effect_apply_aura_name2 = SPELL_AURA_MOD_DAMAGE_PERCENT_DONE;
    passive.effect_base_points2 = -11;
    passive.effect_misc_value2 = 127;
    passive.effect3 = SPELL_EFFECT_APPLY_AURA;
    passive.effect_apply_aura_name3 = SPELL_AURA_MOD_THREAT;
    passive.effect_base_points3 = 29;
    passive.effect_misc_value3 = 127;

    let aura = build_active_aura(
        &passive,
        ObjectGuid::new(HighGuid::Player, 0, 7),
        10,
        test_spell_effect_value_context(&passive),
        Instant::now(),
        None,
    );

    assert_eq!(
        aura.stat_modifiers,
        vec![
            AuraStatModifier::DamageTakenPercent {
                school_mask: 127,
                percent: -10,
            },
            AuraStatModifier::DamageDonePercent {
                school_mask: 127,
                percent: -10,
            },
            AuraStatModifier::ThreatPercent {
                school_mask: 127,
                percent: 30,
            },
        ]
    );
    assert!(matches!(
        spell_aura_support(SPELL_AURA_MOD_DAMAGE_PERCENT_TAKEN),
        SpellMechanicSupport::Implemented
    ));
    assert!(matches!(
        spell_aura_support(SPELL_AURA_MOD_DAMAGE_PERCENT_DONE),
        SpellMechanicSupport::Implemented
    ));
    assert!(matches!(
        spell_aura_support(SPELL_AURA_MOD_THREAT),
        SpellMechanicSupport::Implemented
    ));
}

#[test]
fn defensive_stance_passive_updates_damage_fields_damage_taken_and_threat() {
    let mut map = MapRuntime::new(0, 0);
    map.add_player(test_player_runtime(
        7,
        SessionId(7),
        WorldPosition::new(0, -8950.0, -130.0, 83.5, 0.0),
    ))
    .unwrap();

    let base = map.players.get(&7).unwrap().base_combat_stats;
    let aura = ActiveAura {
        spell_id: 7376,
        caster: ObjectGuid::new(HighGuid::Player, 0, 7),
        level: 10,
        interrupt_flags: 0,
        positive: true,
        visible: false,
        duration_millis: None,
        expires_at: None,
        periodic_damage: None,
        periodic_regen: None,
        stat_modifiers: vec![
            AuraStatModifier::DamageTakenPercent {
                school_mask: 127,
                percent: -10,
            },
            AuraStatModifier::DamageDonePercent {
                school_mask: 127,
                percent: -10,
            },
            AuraStatModifier::ThreatPercent {
                school_mask: 127,
                percent: 30,
            },
        ],
        proc_triggers: Vec::new(),
    };

    let event = map.apply_player_aura(7, aura).unwrap().unwrap();
    let player = map.players.get(&7).unwrap();
    assert!((player.combat_stats.main_min_damage - base.main_min_damage * 0.9).abs() < 0.001);
    assert!((player.combat_stats.main_max_damage - base.main_max_damage * 0.9).abs() < 0.001);

    let combat_update = event
        .direct_packets
        .iter()
        .filter(|packet| packet.opcode == WorldOpcode::SmsgUpdateObject as u16)
        .map(|packet| {
            decode_values_update_block(&packet.body[5..], ObjectGuid::new(HighGuid::Player, 0, 7)).0
        })
        .find(|values| values[UNIT_FIELD_MINDAMAGE].is_some())
        .expect("passive aura should send combat-stat refresh");
    assert_eq!(
        combat_update[UNIT_FIELD_MINDAMAGE],
        Some(player.combat_stats.main_min_damage.to_bits())
    );
    assert_eq!(
        combat_update[UNIT_FIELD_MAXDAMAGE],
        Some(player.combat_stats.main_max_damage.to_bits())
    );

    let player_guid = ObjectGuid::new(HighGuid::Player, 0, 7);
    let applied = map
        .apply_player_world_damage(
            player_guid,
            Some(ObjectGuid::new(HighGuid::Unit, 0, 42)),
            10,
            WorldDamageKind::Melee,
            Instant::now(),
        )
        .unwrap()
        .unwrap();
    assert_eq!(applied.applied_damage, 9);

    let creature_guid = ObjectGuid::new(HighGuid::Unit, 0, 99);
    map.add_db_creature_threat(creature_guid, player_guid, 10.0);
    let threats = map.db_creature_threat_entries(creature_guid);
    assert_eq!(threats.len(), 1);
    assert!((threats[0].threat - 13.0).abs() < f32::EPSILON);
}

#[test]
fn shield_wall_aura_reduces_player_world_damage_by_seventy_five_percent() {
    let mut map = MapRuntime::new(0, 0);
    map.add_player(test_player_runtime(
        7,
        SessionId(7),
        WorldPosition::new(0, -8950.0, -130.0, 83.5, 0.0),
    ))
    .unwrap();

    let aura = ActiveAura {
        spell_id: 871,
        caster: ObjectGuid::new(HighGuid::Player, 0, 7),
        level: 28,
        interrupt_flags: 0,
        positive: true,
        visible: false,
        duration_millis: Some(10_000),
        expires_at: Some(Instant::now() + Duration::from_secs(10)),
        periodic_damage: None,
        periodic_regen: None,
        stat_modifiers: vec![AuraStatModifier::DamageTakenPercent {
            school_mask: 127,
            percent: -75,
        }],
        proc_triggers: Vec::new(),
    };

    map.apply_player_aura(7, aura).unwrap().unwrap();
    let player_guid = ObjectGuid::new(HighGuid::Player, 0, 7);

    let melee = map
        .apply_player_world_damage(
            player_guid,
            Some(ObjectGuid::new(HighGuid::Unit, 0, 42)),
            20,
            WorldDamageKind::Melee,
            Instant::now(),
        )
        .unwrap()
        .unwrap();
    assert_eq!(melee.applied_damage, 5);
    assert_eq!(melee.remaining_health, 15);
}

#[test]
fn berserker_stance_passive_template_builds_combat_modifiers() {
    let mut passive = test_spell_template(7381);
    passive.spell_name = "Berserker Stance Passive".to_string();
    passive.effect1 = SPELL_EFFECT_APPLY_AURA;
    passive.effect_apply_aura_name1 = SPELL_AURA_MOD_CRIT_PERCENT;
    passive.effect_base_points1 = 2;
    passive.effect_misc_value1 = 127;
    passive.effect2 = SPELL_EFFECT_APPLY_AURA;
    passive.effect_apply_aura_name2 = SPELL_AURA_MOD_DAMAGE_PERCENT_TAKEN;
    passive.effect_base_points2 = 9;
    passive.effect_misc_value2 = 127;
    passive.effect3 = SPELL_EFFECT_APPLY_AURA;
    passive.effect_apply_aura_name3 = SPELL_AURA_MOD_THREAT;
    passive.effect_base_points3 = -21;
    passive.effect_misc_value3 = 0;

    let aura = build_active_aura(
        &passive,
        ObjectGuid::new(HighGuid::Player, 0, 7),
        30,
        test_spell_effect_value_context(&passive),
        Instant::now(),
        None,
    );

    assert_eq!(
        aura.stat_modifiers,
        vec![
            AuraStatModifier::CritPercent { percent: 3 },
            AuraStatModifier::DamageTakenPercent {
                school_mask: 127,
                percent: 10,
            },
            AuraStatModifier::ThreatPercent {
                school_mask: 0,
                percent: -20,
            },
        ]
    );
    assert!(matches!(
        spell_aura_support(SPELL_AURA_MOD_CRIT_PERCENT),
        SpellMechanicSupport::Implemented
    ));
    assert!(matches!(
        spell_aura_support(SPELL_AURA_MOD_DAMAGE_PERCENT_TAKEN),
        SpellMechanicSupport::Implemented
    ));
    assert!(matches!(
        spell_aura_support(SPELL_AURA_MOD_THREAT),
        SpellMechanicSupport::Implemented
    ));
}

#[test]
fn berserker_stance_passive_updates_player_crit_fields() {
    let mut map = MapRuntime::new(0, 0);
    map.add_player(test_player_runtime(
        7,
        SessionId(7),
        WorldPosition::new(0, -8950.0, -130.0, 83.5, 0.0),
    ))
    .unwrap();

    let base = map.players.get(&7).unwrap().base_combat_stats;
    let aura = ActiveAura {
        spell_id: 7381,
        caster: ObjectGuid::new(HighGuid::Player, 0, 7),
        level: 30,
        interrupt_flags: 0,
        positive: true,
        visible: false,
        duration_millis: None,
        expires_at: None,
        periodic_damage: None,
        periodic_regen: None,
        stat_modifiers: vec![
            AuraStatModifier::CritPercent { percent: 3 },
            AuraStatModifier::DamageTakenPercent {
                school_mask: 127,
                percent: 10,
            },
            AuraStatModifier::ThreatPercent {
                school_mask: 0,
                percent: -20,
            },
        ],
        proc_triggers: Vec::new(),
    };

    let event = map.apply_player_aura(7, aura).unwrap().unwrap();
    let player = map.players.get(&7).unwrap();
    assert!((player.combat_stats.crit_percent - (base.crit_percent + 3.0)).abs() < 0.001);
    assert!(
        (player.combat_stats.ranged_crit_percent - (base.ranged_crit_percent + 3.0)).abs() < 0.001
    );

    let combat_update = event
        .direct_packets
        .iter()
        .filter(|packet| packet.opcode == WorldOpcode::SmsgUpdateObject as u16)
        .map(|packet| {
            decode_values_update_block(&packet.body[5..], ObjectGuid::new(HighGuid::Player, 0, 7)).0
        })
        .find(|values| values[PLAYER_CRIT_PERCENTAGE].is_some())
        .expect("passive aura should send crit-stat refresh");
    assert_eq!(
        combat_update[PLAYER_CRIT_PERCENTAGE],
        Some(player.combat_stats.crit_percent.to_bits())
    );
    assert_eq!(
        combat_update[PLAYER_RANGED_CRIT_PERCENTAGE],
        Some(player.combat_stats.ranged_crit_percent.to_bits())
    );
}

#[tokio::test]
async fn fade_live_rank_one_builds_total_threat_modifier() {
    let world_db_pool =
        MySqlPool::connect_lazy("mysql://mangos:mangos@127.0.0.1:3307/mangos").unwrap();
    let fade = wow_db::get_spell_template_query(&world_db_pool, 586)
        .await
        .unwrap()
        .expect("Fade rank 1 should exist in the local spell_template");
    let chain = wow_db::get_spell_chain_query(&world_db_pool, 586)
        .await
        .unwrap()
        .expect("Fade rank 1 should exist in spell_chain");

    assert_eq!(chain.spell_id, 586);
    assert_eq!(chain.prev_spell, 0);
    assert_eq!(chain.first_spell, 586);
    assert_eq!(chain.rank, 1);
    assert_eq!(fade.rank.as_deref(), Some("Rank 1"));
    assert_eq!(fade.spell_level, 8);
    assert!(matches!(
        spell_aura_support(SPELL_AURA_MOD_TOTAL_THREAT),
        SpellMechanicSupport::Implemented
    ));

    let aura = build_active_aura(
        &fade,
        ObjectGuid::new(HighGuid::Player, 0, 7),
        fade.spell_level.try_into().unwrap(),
        test_spell_effect_value_context(&fade),
        Instant::now(),
        None,
    );
    assert!(aura
        .stat_modifiers
        .contains(&AuraStatModifier::TotalThreat { amount: -55 }));
}

#[test]
fn fade_temporarily_reduces_db_creature_threat_and_restores_on_expiry() {
    let now = Instant::now();
    let mut map = MapRuntime::new(0, 0);
    let first_position = WorldPosition::new(0, 1.0, 0.0, 0.0, 0.0);
    let second_position = WorldPosition::new(0, 2.0, 0.0, 0.0, 0.0);
    insert_map_runtime_player_for_test(&mut map, 7, first_position);
    insert_map_runtime_player_for_test(&mut map, 8, second_position);

    let mut spawn = test_creature_spawn(6);
    spawn.guid = 460;
    spawn.position_x = 0.0;
    spawn.position_y = 0.0;
    let attacker = creature_spawn_guid(&spawn);
    map.share_db_creature_snapshots(vec![DbCreatureRuntime::new(spawn)]);

    let faded_player = ObjectGuid::new(HighGuid::Player, 0, 7);
    let rival_player = ObjectGuid::new(HighGuid::Player, 0, 8);
    map.begin_db_creature_combat(attacker, faded_player, now)
        .expect("creature combat should start");
    map.add_db_creature_threat(attacker, faded_player, 155.0);
    map.add_db_creature_threat(attacker, rival_player, 120.0);

    let fade = ActiveAura {
        spell_id: 586,
        caster: faded_player,
        level: 8,
        interrupt_flags: 0,
        positive: true,
        visible: true,
        duration_millis: Some(10_000),
        expires_at: Some(now + Duration::from_secs(10)),
        periodic_damage: None,
        periodic_regen: None,
        stat_modifiers: vec![AuraStatModifier::TotalThreat { amount: -55 }],
        proc_triggers: Vec::new(),
    };

    map.apply_player_aura(7, fade).unwrap().unwrap();

    let threats = map.db_creature_threat_entries(attacker);
    assert_eq!(
        threats
            .iter()
            .find(|entry| entry.victim == faded_player)
            .map(|entry| entry.threat),
        Some(100.0)
    );
    assert_eq!(
        map.active_creature_combats
            .get(&attacker.raw())
            .map(|combat| combat.victim),
        Some(rival_player),
        "Fade should immediately lower the faded player below a nearby melee rival"
    );

    let _ = map
        .advance_player_aura_expirations(now + Duration::from_millis(10_001))
        .unwrap();

    let threats = map.db_creature_threat_entries(attacker);
    assert_eq!(
        threats
            .iter()
            .find(|entry| entry.victim == faded_player)
            .map(|entry| entry.threat),
        Some(155.0)
    );
    assert_eq!(
        map.active_creature_combats
            .get(&attacker.raw())
            .map(|combat| combat.victim),
        Some(faded_player),
        "Fade expiry should restore the stored threat reduction and let the creature retarget"
    );
}

#[test]
fn shapeshift_aura_updates_player_form_bytes_and_replaces_previous_form() {
    let mut map = MapRuntime::new(0, 0);
    map.add_player(test_player_runtime(
        7,
        SessionId(7),
        WorldPosition::new(0, -8950.0, -130.0, 83.5, 0.0),
    ))
    .unwrap();

    let now = Instant::now();
    let battle = ActiveAura {
        spell_id: 2457,
        caster: ObjectGuid::new(HighGuid::Player, 0, 7),
        level: 1,
        interrupt_flags: 0,
        positive: true,
        visible: true,
        duration_millis: None,
        expires_at: None,
        periodic_damage: None,
        periodic_regen: None,
        stat_modifiers: vec![AuraStatModifier::Shapeshift {
            form: FORM_BATTLESTANCE,
        }],
        proc_triggers: Vec::new(),
    };
    map.apply_player_aura(7, battle).unwrap().unwrap();

    let defensive = ActiveAura {
        spell_id: 71,
        caster: ObjectGuid::new(HighGuid::Player, 0, 7),
        level: 10,
        interrupt_flags: 0,
        positive: true,
        visible: true,
        duration_millis: Some(1_000),
        expires_at: Some(now + Duration::from_secs(1)),
        periodic_damage: None,
        periodic_regen: None,
        stat_modifiers: vec![AuraStatModifier::Shapeshift {
            form: FORM_DEFENSIVESTANCE,
        }],
        proc_triggers: Vec::new(),
    };

    let event = map.apply_player_aura(7, defensive).unwrap().unwrap();
    assert_eq!(map.players.get(&7).unwrap().active_auras.len(), 1);
    assert_eq!(
        active_aura_shapeshift_form(&map.players.get(&7).unwrap().active_auras),
        Some(FORM_DEFENSIVESTANCE)
    );

    let update = event
        .direct_packets
        .iter()
        .find_map(|packet| {
            (packet.opcode == WorldOpcode::SmsgUpdateObject as u16).then(|| {
                decode_values_update_block(
                    &packet.body[5..],
                    ObjectGuid::new(HighGuid::Player, 0, 7),
                )
                .0
            })
        })
        .expect("shapeshift aura should emit a values update");
    assert_eq!(
        update[UNIT_FIELD_BYTES_1],
        Some(player_unit_bytes_1_with_auras(
            1,
            PLAYER_STAND_STATE_STAND,
            &map.players.get(&7).unwrap().active_auras,
        ),)
    );

    let packets = map
        .advance_player_aura_expirations(now + Duration::from_secs(1))
        .unwrap();
    assert!(map.players.get(&7).unwrap().active_auras.is_empty());
    assert!(packets.iter().any(|(_, packet)| {
        if packet.opcode != WorldOpcode::SmsgUpdateObject as u16 {
            return false;
        }
        let (values, _) =
            decode_values_update_block(&packet.body[5..], ObjectGuid::new(HighGuid::Player, 0, 7));
        values[UNIT_FIELD_BYTES_1]
            == Some(player_unit_bytes_1_with_auras(
                1,
                PLAYER_STAND_STATE_STAND,
                &[],
            ))
    }));
}

#[test]
fn ghost_hover_and_water_walk_auras_are_distinct_runtime_modifiers() {
    let now = Instant::now();
    let caster = ObjectGuid::new(HighGuid::Unit, 6, 68);

    let mut ghost_template = test_spell_template(9036);
    ghost_template.spell_name = "Ghost".to_string();
    ghost_template.effect1 = SPELL_EFFECT_APPLY_AURA;
    ghost_template.effect_apply_aura_name1 = SPELL_AURA_GHOST;
    let ghost = build_active_aura(
        &ghost_template,
        caster,
        60,
        test_spell_effect_value_context(&ghost_template),
        now,
        None,
    );
    assert_eq!(ghost.stat_modifiers, vec![AuraStatModifier::Ghost]);
    assert_eq!(
        active_aura_unit_vis_flags(std::slice::from_ref(&ghost)),
        UNIT_VIS_FLAG_GHOST
    );

    let creature = ObjectGuid::new(HighGuid::Unit, 6, 42);
    let body = build_db_creature_aura_update_body(creature, std::slice::from_ref(&ghost)).unwrap();
    let (values, trailing) = decode_values_update_block(&body[5..], creature);
    assert!(trailing.is_empty());
    assert_eq!(values[UNIT_FIELD_BYTES_1], Some(UNIT_VIS_FLAG_GHOST << 24));

    let clear = build_db_creature_aura_update_body(creature, &[]).unwrap();
    let (values, trailing) = decode_values_update_block(&clear[5..], creature);
    assert!(trailing.is_empty());
    assert_eq!(values[UNIT_FIELD_BYTES_1], Some(0));

    let mut hover_template = test_spell_template(1706);
    hover_template.spell_name = "Levitate Hover".to_string();
    hover_template.effect1 = SPELL_EFFECT_APPLY_AURA;
    hover_template.effect_apply_aura_name1 = SPELL_AURA_HOVER;
    let hover = build_active_aura(
        &hover_template,
        ObjectGuid::new(HighGuid::Player, 0, 7),
        34,
        test_spell_effect_value_context(&hover_template),
        now,
        None,
    );
    assert_eq!(hover.stat_modifiers, vec![AuraStatModifier::Hover]);

    let mut water_walk_template = test_spell_template(546);
    water_walk_template.spell_name = "Water Walking".to_string();
    water_walk_template.effect1 = SPELL_EFFECT_APPLY_AURA;
    water_walk_template.effect_apply_aura_name1 = SPELL_AURA_WATER_WALK;
    let water_walk = build_active_aura(
        &water_walk_template,
        ObjectGuid::new(HighGuid::Player, 0, 7),
        1,
        test_spell_effect_value_context(&water_walk_template),
        now,
        None,
    );
    assert_eq!(water_walk.stat_modifiers, vec![AuraStatModifier::WaterWalk]);

    let mut water_breathing_template = test_spell_template(5697);
    water_breathing_template.spell_name = "Unending Breath".to_string();
    water_breathing_template.effect1 = SPELL_EFFECT_APPLY_AURA;
    water_breathing_template.effect_apply_aura_name1 = SPELL_AURA_WATER_BREATHING;
    let water_breathing = build_active_aura(
        &water_breathing_template,
        ObjectGuid::new(HighGuid::Player, 0, 7),
        16,
        test_spell_effect_value_context(&water_breathing_template),
        now,
        None,
    );
    assert_eq!(
        water_breathing.stat_modifiers,
        vec![AuraStatModifier::WaterBreathing]
    );
}

#[test]
fn active_aura_proc_trigger_spell_ids_filter_flags_and_expiration() {
    let now = Instant::now();
    let active = ActiveAura {
        spell_id: 168,
        caster: ObjectGuid::new(HighGuid::Player, 0, 7),
        level: 1,
        interrupt_flags: 0,
        positive: true,
        visible: true,
        duration_millis: Some(60_000),
        expires_at: Some(now + Duration::from_secs(60)),
        periodic_damage: None,
        periodic_regen: None,
        stat_modifiers: Vec::new(),
        proc_triggers: vec![AuraProcTrigger {
            triggered_spell_id: 6136,
            proc_flags: PROC_FLAG_TAKE_MELEE_SWING,
            proc_ex: 0,
            proc_chance: 100,
            remaining_charges: None,
        }],
    };
    let expired = ActiveAura {
        expires_at: Some(now - Duration::from_secs(1)),
        ..active.clone()
    };

    let mut active_auras = vec![active];
    assert_eq!(
        active_aura_proc_trigger_spell_ids(&mut active_auras, PROC_FLAG_TAKE_MELEE_SWING, now),
        vec![6136]
    );
    active_auras[0].proc_triggers[0].remaining_charges = Some(1);
    assert_eq!(
        active_aura_proc_trigger_spell_ids(&mut active_auras, PROC_FLAG_TAKE_MELEE_SWING, now),
        vec![6136]
    );
    assert_eq!(active_auras[0].proc_triggers[0].remaining_charges, Some(0));
    assert!(
        active_aura_proc_trigger_spell_ids(&mut active_auras, PROC_FLAG_TAKE_MELEE_SWING, now)
            .is_empty()
    );
    let mut expired_auras = vec![expired];
    assert!(active_aura_proc_trigger_spell_ids(
        &mut expired_auras,
        PROC_FLAG_TAKE_MELEE_SWING,
        now
    )
    .is_empty());
    assert!(
        active_aura_proc_trigger_spell_ids(&mut [], PROC_FLAG_TAKE_MELEE_SWING, now).is_empty()
    );
}

#[test]
fn map_owned_consumable_regen_aura_ticks_health_and_mana() {
    let mut map = MapRuntime::new(0, 0);
    let position = WorldPosition::new(0, -8950.0, -130.0, 83.5, 0.0);
    map.add_player(test_player_runtime(7, SessionId(7), position))
        .unwrap();
    let now = Instant::now();
    {
        let player = map.players.get_mut(&7).unwrap();
        player.health = 10;
        player.max_health = 50;
        player.power1 = 5;
        player.max_power1 = 40;
        player.spirit = 0;
        player.active_auras.push(ActiveAura {
            spell_id: 1127,
            caster: ObjectGuid::new(HighGuid::Player, 0, 7),
            level: 1,
            interrupt_flags: 0,
            positive: true,
            visible: true,
            duration_millis: Some(30_000),
            expires_at: Some(now + Duration::from_secs(30)),
            periodic_damage: None,
            periodic_regen: Some(PeriodicRegenAura {
                health_amount: 7,
                mana_amount: 9,
                school_mask: 0,
                tick_millis: 2_000,
                next_tick_at: now,
                interrupts_on_move_and_stand: false,
                suppresses_recent_damage: false,
                makes_player_sit: false,
            }),
            stat_modifiers: Vec::new(),
            proc_triggers: Vec::new(),
        });
    }
    map.next_player_regen_tick_at = Some(now);

    let packets = map.advance_player_regen_tick(now).unwrap();
    let player = map.players.get(&7).unwrap();
    assert_eq!(player.health, 17);
    assert_eq!(player.power1, 14);
    assert!(
        packets
            .iter()
            .filter(|(_, packet)| packet.opcode == WorldOpcode::SmsgUpdateObject as u16)
            .count()
            >= 2
    );
    assert!(packets
        .iter()
        .any(|(_, packet)| packet.opcode == WorldOpcode::SmsgSpellHealLog as u16));
    assert!(packets
        .iter()
        .any(|(_, packet)| packet.opcode == WorldOpcode::SmsgSpellEnergizeLog as u16));
}

#[test]
fn map_runtime_periodic_regen_applies_healing_taken_bonus() {
    let mut map = MapRuntime::new(0, 0);
    let now = Instant::now();
    let mut player = test_player_runtime(
        7,
        SessionId::next(),
        WorldPosition::new(0, 0.0, 0.0, 0.0, 0.0),
    );
    player.class = 5;
    player.health = 10;
    player.max_health = 30;
    player.environment.last_damage_at = Some(now);
    player.active_auras.push(ActiveAura {
        spell_id: 604,
        caster: ObjectGuid::new(HighGuid::Player, 0, 7),
        level: 22,
        interrupt_flags: 0,
        positive: true,
        visible: true,
        duration_millis: Some(600_000),
        expires_at: None,
        periodic_damage: None,
        periodic_regen: None,
        stat_modifiers: vec![AuraStatModifier::HealingTaken {
            school_mask: spell_school_mask_from_school(1),
            amount: -5,
        }],
        proc_triggers: Vec::new(),
    });
    player.active_auras.push(ActiveAura {
        spell_id: 139,
        caster: ObjectGuid::new(HighGuid::Player, 0, 7),
        level: 8,
        interrupt_flags: 0,
        positive: true,
        visible: true,
        duration_millis: Some(15_000),
        expires_at: Some(now + Duration::from_secs(15)),
        periodic_damage: None,
        periodic_regen: Some(PeriodicRegenAura {
            health_amount: 7,
            mana_amount: 0,
            school_mask: spell_school_mask_from_school(1),
            tick_millis: 2_000,
            next_tick_at: now,
            interrupts_on_move_and_stand: false,
            suppresses_recent_damage: false,
            makes_player_sit: false,
        }),
        stat_modifiers: Vec::new(),
        proc_triggers: Vec::new(),
    });
    map.add_player(player).unwrap();
    map.next_player_regen_tick_at = Some(now);

    let packets = map.advance_player_regen_tick(now).unwrap();
    let player = map.players.get(&7).unwrap();
    assert_eq!(player.health, 12);
    assert!(packets
        .iter()
        .any(|(_, packet)| packet.opcode == WorldOpcode::SmsgSpellHealLog as u16));
}

fn decode_other_player_create_values(block: &[u8], guid: ObjectGuid) -> Vec<Option<u32>> {
    assert_eq!(block[0], UPDATE_TYPE_CREATE_OBJECT2);
    let type_id_offset = 1 + PackedGuid::packed_size(guid);
    assert_eq!(block[type_id_offset], TYPEID_PLAYER);
    let flags_offset = type_id_offset + 1;
    assert_eq!(
        block[flags_offset],
        UPDATEFLAG_ALL | UPDATEFLAG_LIVING | UPDATEFLAG_HAS_POSITION
    );
    let movement_start = type_id_offset + 2;
    let movement = MovementInfo::read(&block[movement_start..]).unwrap();
    let movement_len = if movement.flags & MOVEFLAG_JUMPING != 0 {
        44
    } else {
        28
    };
    let values_start = flags_offset + 1 + movement_len + 28;
    decode_update_values(&block[values_start..])
}

#[test]
fn other_player_create_block_includes_equipment_and_movement_state() {
    let guid = ObjectGuid::new(HighGuid::Player, 0, 7);
    let position = WorldPosition::new(0, -8950.0, -130.0, 83.5, 1.25);
    let mut player = test_player_runtime(7, SessionId(7), position);
    player.movement_flags = 0x21;
    player.client_time = 1234;
    player.server_time = 5678;
    player.fall_time = 456;
    player.visible_equipment[EQUIPMENT_SLOT_MAINHAND as usize] = 25;
    let block = build_other_player_create_block(&player).unwrap();
    let type_id_offset = 1 + PackedGuid::packed_size(guid);
    let movement_start = type_id_offset + 2;
    assert_eq!(
        &block[movement_start..movement_start + 4],
        &0x21u32.to_le_bytes()
    );
    assert_eq!(
        &block[movement_start + 4..movement_start + 8],
        &5678u32.to_le_bytes()
    );
    assert_eq!(
        &block[movement_start + 24..movement_start + 28],
        &456u32.to_le_bytes()
    );

    let values = decode_other_player_create_values(&block, guid);
    assert_eq!(
        values[0x104 + EQUIPMENT_SLOT_MAINHAND as usize * 12],
        Some(25)
    );
}

#[test]
fn other_player_create_block_preserves_jump_launch_state() {
    let guid = ObjectGuid::new(HighGuid::Player, 0, 7);
    let position = WorldPosition::new(0, -8950.0, -130.0, 83.5, 1.25);
    let mut player = test_player_runtime(7, SessionId(7), position);
    player.movement_flags = MOVEFLAG_JUMPING;
    player.client_time = 1234;
    player.server_time = 5678;
    player.fall_time = 456;
    player.jump = JumpInfo {
        z_speed: 7.0,
        cos_angle: 0.25,
        sin_angle: 0.75,
        xy_speed: 4.5,
    };

    let block = build_other_player_create_block(&player).unwrap();
    let movement_start = 1 + PackedGuid::packed_size(guid) + 2;
    let movement = MovementInfo::read(&block[movement_start..]).unwrap();

    assert_eq!(movement.flags, MOVEFLAG_JUMPING);
    assert_eq!(movement.client_time, 5678);
    assert_eq!(movement.fall_time, 456);
    assert_eq!(movement.jump, player.jump);
}

#[test]
fn other_player_create_block_preserves_dead_corpse_state() {
    let guid = ObjectGuid::new(HighGuid::Player, 0, 7);
    let position = WorldPosition::new(0, -8950.0, -130.0, 83.5, 1.25);
    let mut player = test_player_runtime(7, SessionId(7), position);
    player.health = 0;
    player.death_state = PlayerDeathState::Corpse;
    player.stand_state = PLAYER_STAND_STATE_DEAD;

    let values =
        decode_other_player_create_values(&build_other_player_create_block(&player).unwrap(), guid);

    assert_eq!(values[UNIT_FIELD_HEALTH], Some(0));
    assert_eq!(
        values[UNIT_FIELD_BYTES_1],
        Some(unit_bytes_1_for_class(player.class) | u32::from(PLAYER_STAND_STATE_DEAD))
    );
}

#[test]
fn player_movement_broadcast_body_preserves_jump_launch_state() {
    let movement = MovementInfo {
        flags: MOVEFLAG_JUMPING,
        client_time: 1234,
        position: WorldPosition::new(0, 1.25, 2.5, 3.75, 1.0),
        fall_time: 456,
        jump: JumpInfo {
            z_speed: 7.0,
            cos_angle: 0.25,
            sin_angle: 0.75,
            xy_speed: 4.5,
        },
    };
    let body = build_player_movement_broadcast_body(7, &movement, 5678).unwrap();
    let guid = ObjectGuid::new(HighGuid::Player, 0, 7);
    let movement = MovementInfo::read(&body[PackedGuid::packed_size(guid)..]).unwrap();

    assert_eq!(movement.flags, MOVEFLAG_JUMPING);
    assert_eq!(movement.client_time, 5678);
    assert_eq!(movement.position.x, 1.25);
    assert_eq!(movement.position.y, 2.5);
    assert_eq!(movement.position.z, 3.75);
    assert_eq!(movement.position.orientation, 1.0);
    assert_eq!(movement.fall_time, 456);
    assert_eq!(
        movement.jump,
        JumpInfo {
            z_speed: 7.0,
            cos_angle: 0.25,
            sin_angle: 0.75,
            xy_speed: 4.5,
        }
    );
}

#[test]
fn map_runtime_broadcasts_turning_movement_to_observers() {
    let mut map = MapRuntime::new(0, 0);
    let player_position = WorldPosition::new(0, -8950.0, -130.0, 83.5, 0.0);
    let observer_position = WorldPosition::new(0, -8952.0, -130.0, 83.5, 0.0);
    map.add_player(test_player_runtime(1, SessionId(1), player_position))
        .unwrap();
    map.add_player(test_player_runtime(2, SessionId(2), observer_position))
        .unwrap();
    let movement = MovementInfo {
        flags: 0,
        client_time: 1234,
        position: WorldPosition::new(0, -8950.0, -130.0, 83.5, 2.25),
        fall_time: 0,
        jump: JumpInfo::default(),
    };

    for opcode in [
        WorldOpcode::MsgMoveStartTurnLeft as u32,
        WorldOpcode::MsgMoveStartTurnRight as u32,
        WorldOpcode::MsgMoveStopTurn as u32,
        WorldOpcode::MsgMoveSetFacing as u32,
    ] {
        let packets = map
            .update_player_position(1, opcode as u16, &movement, 5678)
            .unwrap();

        let packet = packets
            .iter()
            .find(|(session, packet)| *session == SessionId(2) && packet.opcode == opcode as u16)
            .unwrap_or_else(|| panic!("observer should receive {}", movement_opcode_name(opcode)));
        let guid = ObjectGuid::new(HighGuid::Player, 0, 1);
        let broadcast =
            MovementInfo::read(&packet.1.body[PackedGuid::packed_size(guid)..]).unwrap();
        assert_eq!(broadcast.client_time, 5678);
        assert_eq!(broadcast.position.orientation, 2.25);
        if opcode == WorldOpcode::MsgMoveSetFacing as u32 {
            assert_eq!(broadcast.position.x, movement.position.x);
            assert_eq!(broadcast.position.y, movement.position.y);
            assert_eq!(broadcast.position.z, movement.position.z);
            let stored = map.players.get(&1).unwrap();
            assert_eq!(stored.position.x, movement.position.x);
            assert_eq!(stored.position.y, movement.position.y);
            assert_eq!(stored.position.z, movement.position.z);
        }
    }
}

#[test]
fn map_runtime_set_facing_preserves_packet_position_in_late_create_block() {
    let mut map = MapRuntime::new(0, 0);
    let player_position = WorldPosition::new(0, -8950.0, -130.0, 83.5, 0.0);
    let observer_position = WorldPosition::new(0, -8952.0, -130.0, 83.5, 0.0);
    map.add_player(test_player_runtime(1, SessionId(1), player_position))
        .unwrap();
    map.add_player(test_player_runtime(2, SessionId(2), observer_position))
        .unwrap();

    let set_facing = MovementInfo {
        flags: MOVEFLAG_FORWARD,
        client_time: 1234,
        position: WorldPosition::new(0, -8948.5, -129.25, 84.0, 2.25),
        fall_time: 0,
        jump: JumpInfo::default(),
    };

    map.update_player_position(1, WorldOpcode::MsgMoveSetFacing as u16, &set_facing, 5678)
        .unwrap();

    let stored = map.players.get(&1).unwrap();
    assert_eq!(stored.position.x, set_facing.position.x);
    assert_eq!(stored.position.y, set_facing.position.y);
    assert_eq!(stored.position.z, set_facing.position.z);
    assert_eq!(stored.position.orientation, 2.25);

    let block = build_other_player_create_block(stored).unwrap();
    let guid = ObjectGuid::new(HighGuid::Player, 0, 1);
    let movement_start = 1 + PackedGuid::packed_size(guid) + 2;
    let late_visible = MovementInfo::read(&block[movement_start..]).unwrap();
    assert_eq!(late_visible.position.x, set_facing.position.x);
    assert_eq!(late_visible.position.y, set_facing.position.y);
    assert_eq!(late_visible.position.z, set_facing.position.z);
    assert_eq!(late_visible.position.orientation, 2.25);
}

#[test]
fn map_runtime_broadcasts_stop_with_final_idle_orientation() {
    let mut map = MapRuntime::new(0, 0);
    let player_position = WorldPosition::new(0, -8950.0, -130.0, 83.5, 0.0);
    let observer_position = WorldPosition::new(0, -8952.0, -130.0, 83.5, 0.0);
    map.add_player(test_player_runtime(1, SessionId(1), player_position))
        .unwrap();
    map.add_player(test_player_runtime(2, SessionId(2), observer_position))
        .unwrap();
    let movement = MovementInfo {
        flags: 0,
        client_time: 1234,
        position: WorldPosition::new(0, -8950.0, -130.0, 83.5, 2.25),
        fall_time: 0,
        jump: JumpInfo::default(),
    };

    let packets = map
        .update_player_position(1, WorldOpcode::MsgMoveStop as u16, &movement, 5678)
        .unwrap();

    let packet = packets
        .iter()
        .find(|(session, packet)| {
            *session == SessionId(2) && packet.opcode == WorldOpcode::MsgMoveStop as u16
        })
        .expect("observer should receive final stop movement");
    let guid = ObjectGuid::new(HighGuid::Player, 0, 1);
    let broadcast = MovementInfo::read(&packet.1.body[PackedGuid::packed_size(guid)..]).unwrap();
    assert_eq!(broadcast.flags, 0);
    assert_eq!(broadcast.client_time, 5678);
    assert_eq!(broadcast.position.orientation, 2.25);
    let stored = map.players.get(&1).unwrap();
    assert_eq!(stored.movement_flags, 0);
    assert_eq!(stored.position.orientation, 2.25);

    let block = build_other_player_create_block(stored).unwrap();
    let movement_start = 1 + PackedGuid::packed_size(guid) + 2;
    let late_visible = MovementInfo::read(&block[movement_start..]).unwrap();
    assert_eq!(late_visible.flags, 0);
    assert_eq!(late_visible.position.orientation, 2.25);
}

#[test]
fn map_runtime_coalesces_stale_heartbeat_broadcasts_to_observers() {
    let mut map = MapRuntime::new(0, 0);
    let player_position = WorldPosition::new(0, -8950.0, -130.0, 83.5, 0.0);
    let observer_position = WorldPosition::new(0, -8952.0, -130.0, 83.5, 0.0);
    map.add_player(test_player_runtime(1, SessionId(1), player_position))
        .unwrap();
    map.add_player(test_player_runtime(2, SessionId(2), observer_position))
        .unwrap();

    let first = MovementInfo {
        flags: MOVEFLAG_FORWARD,
        client_time: 1,
        position: WorldPosition::new(0, -8949.0, -130.0, 83.5, 0.0),
        fall_time: 0,
        jump: JumpInfo::default(),
    };
    let first_packets = map
        .update_player_position(1, WorldOpcode::MsgMoveHeartbeat as u16, &first, 1000)
        .unwrap();
    assert!(first_packets.iter().any(|(session, packet)| {
        *session == SessionId(2) && packet.opcode == WorldOpcode::MsgMoveHeartbeat as u16
    }));

    let stale = MovementInfo {
        flags: MOVEFLAG_FORWARD,
        client_time: 2,
        position: WorldPosition::new(0, -8948.0, -130.0, 83.5, 0.0),
        fall_time: 0,
        jump: JumpInfo::default(),
    };
    let stale_packets = map
        .update_player_position(1, WorldOpcode::MsgMoveHeartbeat as u16, &stale, 1049)
        .unwrap();
    assert!(!stale_packets.iter().any(|(session, packet)| {
        *session == SessionId(2) && packet.opcode == WorldOpcode::MsgMoveHeartbeat as u16
    }));
    assert_eq!(map.players.get(&1).unwrap().position.x, stale.position.x);

    let fresh = MovementInfo {
        flags: MOVEFLAG_FORWARD,
        client_time: 3,
        position: WorldPosition::new(0, -8947.0, -130.0, 83.5, 0.0),
        fall_time: 0,
        jump: JumpInfo::default(),
    };
    let fresh_packets = map
        .update_player_position(1, WorldOpcode::MsgMoveHeartbeat as u16, &fresh, 1100)
        .unwrap();
    let packet = fresh_packets
        .iter()
        .find(|(session, packet)| {
            *session == SessionId(2) && packet.opcode == WorldOpcode::MsgMoveHeartbeat as u16
        })
        .expect("observer should receive the latest heartbeat after coalesce interval");
    let guid = ObjectGuid::new(HighGuid::Player, 0, 1);
    let broadcast = MovementInfo::read(&packet.1.body[PackedGuid::packed_size(guid)..]).unwrap();
    assert_eq!(broadcast.client_time, 1100);
    assert_eq!(broadcast.position.x, fresh.position.x);
}

#[test]
fn map_runtime_defers_player_visibility_enter_until_refresh_phase() {
    let mut map = MapRuntime::new(0, 0);
    let mover_start = WorldPosition::new(0, -8950.0, -130.0, 83.5, 0.0);
    let observer_position = WorldPosition::new(
        0,
        mover_start.x + PLAYER_VISIBILITY_RADIUS_YARDS + 10.0,
        mover_start.y,
        mover_start.z,
        0.0,
    );
    map.add_player(test_player_runtime(1, SessionId(1), mover_start))
        .unwrap();
    map.add_player(test_player_runtime(2, SessionId(2), observer_position))
        .unwrap();

    let movement = MovementInfo {
        flags: 0,
        client_time: 1234,
        position: WorldPosition::new(
            0,
            observer_position.x - (PLAYER_VISIBILITY_RADIUS_YARDS - 1.0),
            observer_position.y,
            observer_position.z,
            0.0,
        ),
        fall_time: 0,
        jump: JumpInfo::default(),
    };

    let packets = map
        .update_player_position(1, WorldOpcode::MsgMoveStop as u16, &movement, 5678)
        .unwrap();

    assert!(packets
        .iter()
        .all(|(_, packet)| packet.opcode != WorldOpcode::SmsgUpdateObject as u16));
    assert_eq!(
        map.pending_player_visibility_refresh_old_positions.get(&1),
        Some(&mover_start)
    );
    assert!(!map
        .players
        .get(&1)
        .unwrap()
        .visible_objects
        .contains(&ObjectGuid::new(HighGuid::Player, 0, 2)));
    assert!(!map
        .players
        .get(&2)
        .unwrap()
        .visible_objects
        .contains(&ObjectGuid::new(HighGuid::Player, 0, 1)));

    let tick = map.advance_player_visibility_refresh_tick().unwrap();

    assert_eq!(tick.refreshed_players, 1);
    assert!(tick
        .packets
        .iter()
        .any(|(session, packet)| *session == SessionId(2)
            && packet.opcode == WorldOpcode::SmsgUpdateObject as u16));
    assert!(tick
        .packets
        .iter()
        .any(|(session, packet)| *session == SessionId(1)
            && packet.opcode == WorldOpcode::SmsgUpdateObject as u16));
    assert!(map
        .players
        .get(&1)
        .unwrap()
        .visible_objects
        .contains(&ObjectGuid::new(HighGuid::Player, 0, 2)));
    assert!(map
        .players
        .get(&2)
        .unwrap()
        .visible_objects
        .contains(&ObjectGuid::new(HighGuid::Player, 0, 1)));
}

#[test]
fn map_runtime_skips_player_visibility_refresh_below_relocation_limit() {
    let mut map = MapRuntime::new(0, 0);
    let mover_start = WorldPosition::new(0, -8950.0, -130.0, 83.5, 0.0);
    let observer_position = WorldPosition::new(
        0,
        mover_start.x + PLAYER_VISIBILITY_RADIUS_YARDS + 5.0,
        mover_start.y,
        mover_start.z,
        0.0,
    );
    map.add_player(test_player_runtime(1, SessionId(1), mover_start))
        .unwrap();
    map.add_player(test_player_runtime(2, SessionId(2), observer_position))
        .unwrap();

    let movement = MovementInfo {
        flags: MOVEFLAG_FORWARD,
        client_time: 1234,
        position: WorldPosition::new(
            0,
            mover_start.x + PLAYER_VISIBILITY_RELOCATION_LOWER_LIMIT_YARDS - 1.0,
            mover_start.y,
            mover_start.z,
            0.0,
        ),
        fall_time: 0,
        jump: JumpInfo::default(),
    };

    map.update_player_position(1, WorldOpcode::MsgMoveHeartbeat as u16, &movement, 5678)
        .unwrap();

    assert!(map.pending_player_visibility_refreshes.is_empty());
    assert!(map
        .pending_player_visibility_refresh_old_positions
        .is_empty());
    assert!(!map
        .players
        .get(&1)
        .unwrap()
        .visible_objects
        .contains(&ObjectGuid::new(HighGuid::Player, 0, 2)));
    assert!(!map
        .players
        .get(&2)
        .unwrap()
        .visible_objects
        .contains(&ObjectGuid::new(HighGuid::Player, 0, 1)));
}

#[test]
fn map_runtime_visibility_refresh_keeps_earliest_old_position_across_multiple_moves() {
    let mut map = MapRuntime::new(0, 0);
    let start = WorldPosition::new(0, -8950.0, -130.0, 83.5, 0.0);
    map.add_player(test_player_runtime(1, SessionId(1), start))
        .unwrap();

    let first = MovementInfo {
        flags: MOVEFLAG_FORWARD,
        client_time: 1,
        position: WorldPosition::new(
            0,
            start.x + PLAYER_VISIBILITY_RELOCATION_LOWER_LIMIT_YARDS,
            start.y,
            start.z,
            0.0,
        ),
        fall_time: 0,
        jump: JumpInfo::default(),
    };
    map.update_player_position(1, WorldOpcode::MsgMoveHeartbeat as u16, &first, 2)
        .unwrap();
    assert_eq!(
        map.pending_player_visibility_refresh_old_positions.get(&1),
        Some(&start)
    );

    let second = MovementInfo {
        flags: MOVEFLAG_FORWARD,
        client_time: 3,
        position: WorldPosition::new(
            0,
            start.x + PLAYER_VISIBILITY_RELOCATION_LOWER_LIMIT_YARDS + 4.0,
            start.y,
            start.z,
            0.0,
        ),
        fall_time: 0,
        jump: JumpInfo::default(),
    };
    map.update_player_position(1, WorldOpcode::MsgMoveHeartbeat as u16, &second, 4)
        .unwrap();

    assert_eq!(
        map.pending_player_visibility_refresh_old_positions.get(&1),
        Some(&start)
    );

    let tick = map.advance_player_visibility_refresh_tick().unwrap();
    assert_eq!(tick.refreshed_players, 1);
    assert!(map
        .pending_player_visibility_refresh_old_positions
        .is_empty());
}

#[tokio::test]
async fn map_runtime_manager_movement_actor_disabled_keeps_direct_mutex_path() {
    let maps = MapRuntimeManager::default()
        .with_movement_actor_settings_for_test(MovementActorSettings::for_test(false, 16, 8));
    let player_position = WorldPosition::new(0, -8950.0, -130.0, 83.5, 0.0);
    let observer_position = WorldPosition::new(0, -8952.0, -130.0, 83.5, 0.0);
    maps.add_player(test_player_runtime(1, SessionId(1), player_position))
        .await
        .unwrap();
    maps.add_player(test_player_runtime(2, SessionId(2), observer_position))
        .await
        .unwrap();
    let movement = MovementInfo {
        flags: 0,
        client_time: 1234,
        position: WorldPosition::new(0, -8950.0, -130.0, 83.5, 2.25),
        fall_time: 0,
        jump: JumpInfo::default(),
    };

    let packets = match maps
        .update_player_position(0, 1, WorldOpcode::MsgMoveStop as u16, &movement, 5678)
        .await
        .unwrap()
    {
        MovementUpdateOutcome::Applied { packets } => packets,
    };

    assert!(maps.movement_actors.lock().await.is_empty());
    assert!(packets
        .iter()
        .any(|(session, packet)| *session == SessionId(2)
            && packet.opcode == WorldOpcode::MsgMoveStop as u16));
}

#[tokio::test]
async fn map_runtime_manager_movement_actor_matches_direct_path_packets() {
    let direct_maps = MapRuntimeManager::default();
    let actor_maps = MapRuntimeManager::default()
        .with_movement_actor_settings_for_test(MovementActorSettings::for_test(true, 16, 8));
    let player_position = WorldPosition::new(0, -8950.0, -130.0, 83.5, 0.0);
    let observer_position = WorldPosition::new(0, -8952.0, -130.0, 83.5, 0.0);
    for maps in [&direct_maps, &actor_maps] {
        maps.add_player(test_player_runtime(1, SessionId(1), player_position))
            .await
            .unwrap();
        maps.add_player(test_player_runtime(2, SessionId(2), observer_position))
            .await
            .unwrap();
    }
    let movement = MovementInfo {
        flags: 0,
        client_time: 1234,
        position: WorldPosition::new(0, -8950.0, -130.0, 83.5, 2.25),
        fall_time: 0,
        jump: JumpInfo::default(),
    };

    let direct_packets = match direct_maps
        .update_player_position(0, 1, WorldOpcode::MsgMoveStop as u16, &movement, 5678)
        .await
        .unwrap()
    {
        MovementUpdateOutcome::Applied { packets } => packets,
    }
    .into_iter()
    .map(|(session, packet)| (session, packet.opcode, packet.body))
    .collect::<Vec<_>>();
    let actor_packets = match actor_maps
        .update_player_position(0, 1, WorldOpcode::MsgMoveStop as u16, &movement, 5678)
        .await
        .unwrap()
    {
        MovementUpdateOutcome::Applied { packets } => packets,
    }
    .into_iter()
    .map(|(session, packet)| (session, packet.opcode, packet.body))
    .collect::<Vec<_>>();

    assert_eq!(actor_packets, direct_packets);
    let direct_snapshot = direct_maps.player_runtime_snapshot(0, 1).await.unwrap();
    let actor_snapshot = actor_maps.player_runtime_snapshot(0, 1).await.unwrap();
    assert_eq!(
        actor_snapshot.position.orientation,
        direct_snapshot.position.orientation
    );
    assert_eq!(
        actor_snapshot.movement_flags,
        direct_snapshot.movement_flags
    );
    assert_eq!(actor_maps.movement_actors.lock().await.len(), 1);
}

#[tokio::test]
async fn map_runtime_manager_movement_actor_surfaces_full_mailbox_backpressure() {
    let maps = MapRuntimeManager::default()
        .with_movement_actor_settings_for_test(MovementActorSettings::for_test(true, 1, 1));
    let position = WorldPosition::new(0, -8950.0, -130.0, 83.5, 0.0);
    maps.add_player(test_player_runtime(1, SessionId(1), position))
        .await
        .unwrap();
    let (sender, receiver) = mpsc::channel(1);
    let (reply, _reply_rx) = tokio::sync::oneshot::channel();
    sender
        .try_send(MovementActorCommand::UpdatePlayerPosition(
            MovementUpdateCommand {
                character_guid: 1,
                opcode: WorldOpcode::MsgMoveStop as u16,
                movement: MovementInfo {
                    flags: 0,
                    client_time: 1,
                    position,
                    fall_time: 0,
                    jump: JumpInfo::default(),
                },
                server_time: 1,
                enqueued_at: Instant::now(),
                reply,
            },
        ))
        .unwrap();
    let _receiver = receiver;
    maps.movement_actors
        .lock()
        .await
        .insert((0, 0), MovementActorHandle { sender });

    let movement = MovementInfo {
        flags: 0,
        client_time: 2,
        position: WorldPosition::new(0, -8950.0, -130.0, 83.5, 1.5),
        fall_time: 0,
        jump: JumpInfo::default(),
    };
    let error = maps
        .update_player_position(0, 1, WorldOpcode::MsgMoveStop as u16, &movement, 2)
        .await
        .expect_err("mailbox should report backpressure");

    assert!(error.to_string().contains("mailbox is full"));
}

#[tokio::test]
async fn map_runtime_manager_movement_actor_completes_all_concurrent_replies() {
    let maps = Arc::new(
        MapRuntimeManager::default()
            .with_movement_actor_settings_for_test(MovementActorSettings::for_test(true, 32, 16)),
    );
    let observer_position = WorldPosition::new(0, -8955.0, -130.0, 83.5, 0.0);
    maps.add_player(test_player_runtime(99, SessionId(99), observer_position))
        .await
        .unwrap();
    for guid in 1..=8 {
        maps.add_player(test_player_runtime(
            guid,
            SessionId(guid as u64),
            WorldPosition::new(0, -8950.0 + guid as f32, -130.0, 83.5, 0.0),
        ))
        .await
        .unwrap();
    }

    let mut tasks = Vec::new();
    for guid in 1..=8 {
        let maps = maps.clone();
        tasks.push(tokio::spawn(async move {
            let movement = MovementInfo {
                flags: 0,
                client_time: guid,
                position: WorldPosition::new(0, -8950.0 + guid as f32, -129.5, 83.5, 0.25),
                fall_time: 0,
                jump: JumpInfo::default(),
            };
            maps.update_player_position(0, guid, WorldOpcode::MsgMoveStop as u16, &movement, guid)
                .await
        }));
    }

    for task in tasks {
        assert!(task.await.unwrap().is_ok());
    }
}
#[test]
fn other_player_create_block_includes_public_unit_target() {
    let guid = ObjectGuid::new(HighGuid::Player, 0, 7);
    let selected = ObjectGuid::new(HighGuid::Unit, 0, 99);
    let mut player =
        test_player_runtime(7, SessionId(7), WorldPosition::new(0, 1.0, 2.0, 3.0, 0.0));
    player.selected_target = None;
    player.unit_target = Some(selected);

    let values =
        decode_other_player_create_values(&build_other_player_create_block(&player).unwrap(), guid);
    assert_eq!(values[UNIT_FIELD_TARGET], Some(selected.raw() as u32));
    assert_eq!(
        values[UNIT_FIELD_TARGET + 1],
        Some((selected.raw() >> 32) as u32)
    );
}

#[test]
fn player_looting_state_sets_unit_flag_for_observers_and_late_visibility() {
    let mut map = MapRuntime::new(0, 0);
    let player_position = WorldPosition::new(0, -8950.0, -130.0, 83.5, 0.0);
    let observer_position = WorldPosition::new(0, -8952.0, -130.0, 83.5, 0.0);
    map.add_player(test_player_runtime(1, SessionId(1), player_position))
        .unwrap();
    map.add_player(test_player_runtime(2, SessionId(2), observer_position))
        .unwrap();

    let packets = map.set_player_looting_state(1, true).unwrap();

    let packet = packets
        .iter()
        .find(|(session, packet)| {
            *session == SessionId(2) && packet.opcode == WorldOpcode::SmsgUpdateObject as u16
        })
        .expect("observer should receive player looting flag update");
    let player = ObjectGuid::new(HighGuid::Player, 0, 1);
    let (values, trailing) = decode_values_update_block(&packet.1.body[5..], player);
    assert!(trailing.is_empty());
    assert_eq!(
        values[UNIT_FIELD_FLAGS],
        Some(UNIT_FLAG_PLAYER_CONTROLLED | UNIT_FLAG_LOOTING)
    );

    let values = decode_other_player_create_values(
        &build_other_player_create_block(&map.players[&1]).unwrap(),
        player,
    );
    assert_eq!(
        values[UNIT_FIELD_FLAGS],
        Some(UNIT_FLAG_PLAYER_CONTROLLED | UNIT_FLAG_LOOTING)
    );
}

#[test]
fn disarm_aura_sets_player_unit_flags_in_runtime_updates() {
    let now = Instant::now();
    let mut map = MapRuntime::new(0, 0);
    let player_position = WorldPosition::new(0, -8950.0, -130.0, 83.5, 0.0);
    let observer_position = WorldPosition::new(0, -8952.0, -130.0, 83.5, 0.0);
    map.add_player(test_player_runtime(1, SessionId(1), player_position))
        .unwrap();
    map.add_player(test_player_runtime(2, SessionId(2), observer_position))
        .unwrap();

    let event = map
        .apply_player_aura(1, test_control_aura(AuraStatModifier::Disarm, now))
        .unwrap()
        .expect("disarm aura should apply");
    let player = ObjectGuid::new(HighGuid::Player, 0, 1);
    let combat_update = event
        .observer_packets
        .iter()
        .filter(|(_, packet)| packet.opcode == WorldOpcode::SmsgUpdateObject as u16)
        .map(|(_, packet)| decode_values_update_block(&packet.body[5..], player).0)
        .find(|values| values[UNIT_FIELD_FLAGS].is_some())
        .expect("observer should receive disarm unit flag update");
    assert_eq!(
        combat_update[UNIT_FIELD_FLAGS],
        Some(UNIT_FLAG_PLAYER_CONTROLLED | UNIT_FLAG_DISARMED)
    );

    let values = decode_other_player_create_values(
        &build_other_player_create_block(&map.players[&1]).unwrap(),
        player,
    );
    assert_eq!(
        values[UNIT_FIELD_FLAGS],
        Some(UNIT_FLAG_PLAYER_CONTROLLED | UNIT_FLAG_DISARMED)
    );
}

#[test]
fn auto_attack_intent_does_not_mark_player_in_combat() {
    let mut map = MapRuntime::new(0, 0);
    let player_position = WorldPosition::new(0, -8950.0, -130.0, 83.5, 0.0);
    map.add_player(test_player_runtime(1, SessionId(1), player_position))
        .unwrap();
    let target = ObjectGuid::new(HighGuid::Unit, 6, 46);

    map.set_player_auto_attack(1, Some(target), Some(Instant::now()));

    let snapshot = map.player_runtime_snapshot(1).unwrap();
    assert_eq!(snapshot.active_combat_target, Some(target));
    assert!(
        !snapshot.in_combat,
        "CMaNGOS keeps attack intent separate from UNIT_FLAG_IN_COMBAT"
    );
    let player = ObjectGuid::new(HighGuid::Player, 0, 1);
    let values = decode_other_player_create_values(
        &build_other_player_create_block(&map.players[&1]).unwrap(),
        player,
    );
    assert_eq!(values[UNIT_FIELD_FLAGS], Some(UNIT_FLAG_PLAYER_CONTROLLED));
}

#[test]
fn creature_combat_ownership_marks_player_in_combat() {
    let mut map = MapRuntime::new(0, 0);
    let player_position = WorldPosition::new(0, -8950.0, -130.0, 83.5, 0.0);
    map.add_player(test_player_runtime(1, SessionId(1), player_position))
        .unwrap();
    let mut spawn = test_creature_spawn(6);
    spawn.guid = 46;
    spawn.position_x = player_position.x + 2.0;
    spawn.position_y = player_position.y;
    let attacker = creature_spawn_guid(&spawn);
    map.share_db_creature_snapshots(vec![DbCreatureRuntime::new(spawn)]);
    let victim = ObjectGuid::new(HighGuid::Player, 0, 1);

    map.begin_db_creature_combat(attacker, victim, Instant::now())
        .expect("creature combat should start");

    assert!(map.player_runtime_snapshot(1).unwrap().in_combat);
    let values = decode_other_player_create_values(
        &build_other_player_create_block(&map.players[&1]).unwrap(),
        victim,
    );
    assert_eq!(
        values[UNIT_FIELD_FLAGS],
        Some(UNIT_FLAG_PLAYER_CONTROLLED | UNIT_FLAG_IN_COMBAT)
    );

    map.clear_db_creature_combat(attacker);

    assert!(!map.player_runtime_snapshot(1).unwrap().in_combat);
    let values = decode_other_player_create_values(
        &build_other_player_create_block(&map.players[&1]).unwrap(),
        victim,
    );
    assert_eq!(values[UNIT_FIELD_FLAGS], Some(UNIT_FLAG_PLAYER_CONTROLLED));
}

#[test]
fn polymorph_preserves_combat_but_suppresses_creature_reaction_like_cmangos() {
    let now = Instant::now();
    let mut map = MapRuntime::new(0, 0);
    let player_position = WorldPosition::new(0, -8950.0, -130.0, 83.5, 0.0);
    map.add_player(test_player_runtime(7, SessionId(7), player_position))
        .unwrap();
    let mut spawn = test_creature_spawn(6);
    spawn.guid = 470;
    spawn.position_x = player_position.x + 2.0;
    spawn.position_y = player_position.y;
    let attacker = creature_spawn_guid(&spawn);
    map.share_db_creature_snapshots(vec![DbCreatureRuntime::new(spawn)]);
    let victim = ObjectGuid::new(HighGuid::Player, 0, 7);

    map.begin_db_creature_combat(attacker, victim, now)
        .expect("creature combat should start");
    assert!(map.player_runtime_snapshot(7).unwrap().in_combat);
    assert!(map.active_creature_combats.contains_key(&attacker.raw()));
    assert!(map.creature_threats.contains_key(&attacker.raw()));
    assert!(map.creature_combat_leash.contains_key(&attacker.raw()));

    let aura = ActiveAura {
        spell_id: 118,
        caster: victim,
        level: 12,
        interrupt_flags: AURA_INTERRUPT_FLAG_DAMAGE,
        positive: false,
        visible: true,
        duration_millis: Some(30_000),
        expires_at: Some(now + Duration::from_secs(30)),
        periodic_damage: None,
        periodic_regen: None,
        stat_modifiers: vec![
            AuraStatModifier::Confuse,
            AuraStatModifier::Transform {
                display_id: 100,
                creature_entry: 0,
            },
        ],
        proc_triggers: Vec::new(),
    };

    map.apply_db_creature_aura_replacing_spell_ids(attacker, 7, aura, &[], None, None, now)
        .unwrap()
        .expect("polymorph should apply");

    assert!(map.player_runtime_snapshot(7).unwrap().in_combat);
    assert!(map.active_creature_combats.contains_key(&attacker.raw()));
    assert!(map.creature_threats.contains_key(&attacker.raw()));
    assert!(map.creature_combat_leash.contains_key(&attacker.raw()));
    let creature = map.creatures.get(&attacker.raw()).unwrap();
    assert!(active_aura_has_confuse(&creature.active_auras));
    assert!(
        active_auras_suppress_hostile_refs(&creature.active_auras),
        "CMaNGOS suppresses hostile references while Polymorph is active without deleting them"
    );
    assert!(
        matches!(creature.motion, CreatureMotionState::Confused(_))
            || creature.next_confused_move_at.is_some(),
        "Polymorph should transfer motion to confused wandering instead of chase or return-home"
    );
}

#[test]
fn polymorphed_creature_keeps_confused_wandering_while_in_combat() {
    let now = Instant::now();
    let mut map = MapRuntime::new(0, 0);
    let player_position = WorldPosition::new(0, -8950.0, -130.0, 83.5, 0.0);
    map.add_player(test_player_runtime(7, SessionId(7), player_position))
        .unwrap();
    let mut spawn = test_creature_spawn(6);
    spawn.guid = 471;
    spawn.position_x = player_position.x + 2.0;
    spawn.position_y = player_position.y;
    spawn.position_z = player_position.z;
    let attacker = creature_spawn_guid(&spawn);
    map.share_db_creature_snapshots(vec![DbCreatureRuntime::new(spawn)]);
    let victim = ObjectGuid::new(HighGuid::Player, 0, 7);

    map.begin_db_creature_combat(attacker, victim, now)
        .expect("creature combat should start");
    let aura = ActiveAura {
        spell_id: 118,
        caster: victim,
        level: 12,
        interrupt_flags: AURA_INTERRUPT_FLAG_DAMAGE,
        positive: false,
        visible: true,
        duration_millis: Some(30_000),
        expires_at: Some(now + Duration::from_secs(30)),
        periodic_damage: None,
        periodic_regen: None,
        stat_modifiers: vec![AuraStatModifier::Confuse],
        proc_triggers: Vec::new(),
    };

    map.apply_db_creature_aura_replacing_spell_ids(attacker, 7, aura, &[], None, None, now)
        .unwrap()
        .expect("polymorph should apply");
    let first_duration = {
        let creature = map.creatures.get(&attacker.raw()).unwrap();
        let CreatureMotionState::Confused(motion) = &creature.motion else {
            panic!("polymorph should immediately start confused wandering");
        };
        motion.duration
    };

    let first_finished_at = now + first_duration + Duration::from_millis(1);
    map.advance_active_db_creature_idle_motions_with_interval(
        &DbCreatureNavigationGuardrail::default(),
        first_finished_at,
        Duration::from_millis(1),
    )
    .unwrap();
    let next_confused_due = {
        let creature = map.creatures.get(&attacker.raw()).unwrap();
        assert!(
            map.active_creature_combats.contains_key(&attacker.raw()),
            "Polymorph must preserve combat while confused motion keeps ticking"
        );
        assert!(matches!(creature.motion, CreatureMotionState::Idle));
        creature
            .next_confused_move_at
            .expect("finished confused spline should schedule another confused wander")
    };

    map.advance_active_db_creature_idle_motions_with_interval(
        &DbCreatureNavigationGuardrail::default(),
        next_confused_due,
        Duration::from_millis(1),
    )
    .unwrap();

    let creature = map.creatures.get(&attacker.raw()).unwrap();
    assert!(
        matches!(creature.motion, CreatureMotionState::Confused(_)),
        "combat-preserved Polymorph should keep starting confused wander splines"
    );
}

#[test]
fn rooted_polymorph_does_not_start_confused_wandering_until_root_ends() {
    let now = Instant::now();
    let mut map = MapRuntime::new(0, 0);
    let player_position = WorldPosition::new(0, -8950.0, -130.0, 83.5, 0.0);
    map.add_player(test_player_runtime(7, SessionId(7), player_position))
        .unwrap();
    let mut spawn = test_creature_spawn(6);
    spawn.guid = 472;
    spawn.position_x = player_position.x + 2.0;
    spawn.position_y = player_position.y;
    spawn.position_z = player_position.z;
    let target = creature_spawn_guid(&spawn);
    map.share_db_creature_snapshots(vec![DbCreatureRuntime::new(spawn)]);
    let caster = ObjectGuid::new(HighGuid::Player, 0, 7);
    let root = ActiveAura {
        spell_id: 122,
        caster,
        level: 12,
        interrupt_flags: 0,
        positive: false,
        visible: true,
        duration_millis: Some(8_000),
        expires_at: Some(now + Duration::from_secs(8)),
        periodic_damage: None,
        periodic_regen: None,
        stat_modifiers: vec![AuraStatModifier::Root],
        proc_triggers: Vec::new(),
    };
    let polymorph = ActiveAura {
        spell_id: 118,
        caster,
        level: 12,
        interrupt_flags: AURA_INTERRUPT_FLAG_DAMAGE,
        positive: false,
        visible: true,
        duration_millis: Some(30_000),
        expires_at: Some(now + Duration::from_secs(30)),
        periodic_damage: None,
        periodic_regen: None,
        stat_modifiers: vec![AuraStatModifier::Confuse],
        proc_triggers: Vec::new(),
    };

    map.apply_db_creature_aura_replacing_spell_ids(target, 7, root, &[], None, None, now)
        .unwrap()
        .expect("root should apply");
    map.apply_db_creature_aura_replacing_spell_ids(target, 7, polymorph, &[], None, None, now)
        .unwrap()
        .expect("polymorph should apply");

    let creature = map.creatures.get(&target.raw()).unwrap();
    assert!(active_aura_has_root(&creature.active_auras));
    assert!(active_aura_has_confuse(&creature.active_auras));
    assert!(matches!(creature.motion, CreatureMotionState::Idle));
    assert_eq!(
        creature.next_confused_move_at,
        Some(now),
        "root should defer the confused wander rather than consuming its due time"
    );

    let tick = map
        .advance_active_db_creature_idle_motions_with_interval(
            &DbCreatureNavigationGuardrail::default(),
            now,
            Duration::from_millis(1),
        )
        .unwrap();
    assert!(tick.creatures.is_empty());
    assert!(matches!(
        map.creatures.get(&target.raw()).unwrap().motion,
        CreatureMotionState::Idle
    ));
}

#[test]
fn confused_creature_cannot_sight_aggro_until_control_ends() {
    let mut spawn = test_creature_spawn(38);
    spawn.template.faction = 17;
    spawn.template.npc_flags = 0;
    spawn.template.creature_type = 7;
    spawn.template.min_level = 1;
    spawn.template.detection_range = 20;
    let mut runtime = DbCreatureRuntime::new(spawn);
    let now = Instant::now();
    runtime.active_auras.push(ActiveAura {
        spell_id: 118,
        caster: ObjectGuid::new(HighGuid::Player, 0, 7),
        level: 12,
        interrupt_flags: AURA_INTERRUPT_FLAG_DAMAGE,
        positive: false,
        visible: true,
        duration_millis: Some(30_000),
        expires_at: Some(now + Duration::from_secs(30)),
        periodic_damage: None,
        periodic_regen: None,
        stat_modifiers: vec![AuraStatModifier::Confuse],
        proc_triggers: Vec::new(),
    });
    let character = ActiveCharacter {
        guid: 7,
        name: "Ada".to_string(),
        race: 1,
        class: 1,
        level: 1,
        xp: 0,
        position: runtime.current_position,
        movement_flags: 0,
        client_time: 0,
        fall_time: 0,
        jump: JumpInfo::default(),
    };
    let faction_templates = FactionTemplateStore::fallback_bridge();

    assert!(!runtime.can_aggro_player(&faction_templates, &character, now));

    runtime.active_auras.clear();
    assert!(runtime.can_aggro_player(&faction_templates, &character, now));
}

#[test]
fn confused_creature_cannot_start_chase_motion() {
    let mut spawn = test_creature_spawn(6);
    spawn.position_x = 0.0;
    spawn.position_y = 0.0;
    spawn.position_z = 0.0;
    let creature_guid = creature_spawn_guid(&spawn);
    let player = ObjectGuid::new(HighGuid::Player, 0, 7);
    let player_position = WorldPosition::new(0, 12.0, 0.0, 0.0, 0.0);
    let now = Instant::now();
    let mut runtime = DbCreatureRuntime::new(spawn);
    runtime.active_auras.push(ActiveAura {
        spell_id: 118,
        caster: player,
        level: 12,
        interrupt_flags: AURA_INTERRUPT_FLAG_DAMAGE,
        positive: false,
        visible: true,
        duration_millis: Some(30_000),
        expires_at: Some(now + Duration::from_secs(30)),
        periodic_damage: None,
        periodic_regen: None,
        stat_modifiers: vec![AuraStatModifier::Confuse],
        proc_triggers: Vec::new(),
    });
    runtime.begin_confused_motion(now);

    assert!(start_db_creature_chase_motion_runtime(
        &DbCreatureNavigationGuardrail::default(),
        None,
        &mut runtime,
        DbCreatureChaseTarget {
            guid: player,
            position: player_position,
            movement_flags: 0,
        },
        None,
        now,
    )
    .is_none());
    assert_eq!(runtime.guid(), creature_guid);
    assert!(!matches!(runtime.motion, CreatureMotionState::Chase(_)));
}

#[test]
fn player_visible_equipment_update_block_updates_observer_item_visual() {
    let guid = ObjectGuid::new(HighGuid::Player, 0, 7);
    let mut visible_equipment = [0; ENUM_EQUIPMENT_SLOTS];
    visible_equipment[EQUIPMENT_SLOT_MAINHAND as usize] = 25;

    let block = build_player_visible_equipment_update_block(
        7,
        &visible_equipment,
        &[EQUIPMENT_SLOT_MAINHAND],
    )
    .unwrap();
    let (values, trailing) = decode_values_update_block(&block, guid);
    assert!(trailing.is_empty());
    assert_eq!(
        values[0x104 + EQUIPMENT_SLOT_MAINHAND as usize * 12],
        Some(25)
    );
}

#[test]
fn map_runtime_idle_motion_start_guids_require_player_interest() {
    let mut map = MapRuntime::new(0, 0);
    let center = WorldPosition::new(0, -8950.0, -130.0, 83.5, 0.0);
    let grid = grid_coord_for_position(center);
    let mut spawn = test_creature_spawn(6);
    spawn.guid = 305;
    spawn.position_x = center.x;
    spawn.position_y = center.y;
    spawn.position_z = center.z;
    spawn.movement_type = DB_MOTION_TYPE_RANDOM;
    spawn.spawn_dist = 5.0;
    let now = Instant::now();
    let mut runtime = DbCreatureRuntime::new(spawn);
    runtime.next_random_move_at = Some(now);
    map.insert_loaded_creature_grid(grid, vec![runtime]);

    map.add_player(test_player_runtime(8, SessionId(8), center))
        .unwrap();
    let packets = map.remove_player(8);
    assert!(packets.is_empty());
    assert_eq!(map.grids.get(&grid).unwrap().state, GridState::Idle);

    assert_eq!(
        map.db_creature_idle_motion_start_guids(now),
        Vec::<u64>::new(),
        "CMaNGOS-shaped idle patrol starts should pause once no player keeps the area active"
    );
}

#[test]
fn map_runtime_confused_motion_start_guids_use_control_scheduler() {
    let mut map = MapRuntime::new(0, 0);
    let center = WorldPosition::new(0, -8950.0, -130.0, 83.5, 0.0);
    let grid = grid_coord_for_position(center);
    let mut spawn = test_creature_spawn(6);
    spawn.guid = 405;
    spawn.position_x = center.x;
    spawn.position_y = center.y;
    spawn.position_z = center.z;
    spawn.movement_type = DB_MOTION_TYPE_RANDOM;
    spawn.spawn_dist = 5.0;
    let guid = creature_spawn_guid(&spawn);
    let now = Instant::now();
    let mut runtime = DbCreatureRuntime::new(spawn);
    runtime.active_auras.push(ActiveAura {
        spell_id: 118,
        caster: ObjectGuid::new(HighGuid::Player, 0, 7),
        level: 1,
        interrupt_flags: AURA_INTERRUPT_FLAG_DAMAGE,
        visible: true,
        positive: false,
        duration_millis: Some(8_000),
        expires_at: Some(now + Duration::from_secs(8)),
        periodic_damage: None,
        periodic_regen: None,
        stat_modifiers: vec![AuraStatModifier::Confuse],
        proc_triggers: Vec::new(),
    });
    runtime.begin_confused_motion(now);
    map.insert_loaded_creature_grid(grid, vec![runtime]);
    map.add_player(test_player_runtime(8, SessionId(8), center))
        .unwrap();

    assert_eq!(
        map.db_creature_idle_motion_start_guids(now),
        Vec::<u64>::new()
    );
    assert_eq!(
        map.db_creature_confused_motion_start_guids(now),
        vec![guid.raw()]
    );
}

#[test]
fn map_runtime_tick_starts_confused_motion_from_control_scheduler() {
    let mut map = MapRuntime::new(0, 0);
    let center = WorldPosition::new(0, -8950.0, -130.0, 83.5, 0.0);
    let grid = grid_coord_for_position(center);
    let mut spawn = test_creature_spawn(6);
    spawn.guid = 406;
    spawn.position_x = center.x;
    spawn.position_y = center.y;
    spawn.position_z = center.z;
    let guid = creature_spawn_guid(&spawn);
    let now = Instant::now();
    let mut runtime = DbCreatureRuntime::new(spawn);
    runtime.active_auras.push(ActiveAura {
        spell_id: 118,
        caster: ObjectGuid::new(HighGuid::Player, 0, 7),
        level: 1,
        interrupt_flags: AURA_INTERRUPT_FLAG_DAMAGE,
        visible: true,
        positive: false,
        duration_millis: Some(8_000),
        expires_at: Some(now + Duration::from_secs(8)),
        periodic_damage: None,
        periodic_regen: None,
        stat_modifiers: vec![AuraStatModifier::Confuse],
        proc_triggers: Vec::new(),
    });
    runtime.begin_confused_motion(now);
    map.insert_loaded_creature_grid(grid, vec![runtime]);
    map.add_player(test_player_runtime(8, SessionId(8), center))
        .unwrap();

    let tick = map
        .advance_active_db_creature_idle_motions(&DbCreatureNavigationGuardrail::default(), now)
        .expect("control-motion tick should succeed");

    assert_eq!(tick.creatures.len(), 1);
    assert_eq!(tick.creatures[0].guid(), guid);
    assert!(matches!(
        tick.creatures[0].motion,
        CreatureMotionState::Confused(_)
    ));
    assert!(
        tick.packets
            .iter()
            .any(|(_, packet)| packet.opcode == WorldOpcode::SmsgMonsterMove as u16),
        "confused control start should broadcast movement"
    );
}

#[test]
fn map_runtime_idle_motion_start_guids_ignore_far_same_grid_creatures() {
    let mut map = MapRuntime::new(0, 0);
    let center = WorldPosition::new(0, -8950.0, -130.0, 83.5, 0.0);
    let far_same_grid = WorldPosition::new(
        0,
        center.x + CREATURE_SPAWN_RADIUS_YARDS + 40.0,
        center.y,
        center.z,
        center.orientation,
    );
    assert_eq!(
        grid_coord_for_position(center),
        grid_coord_for_position(far_same_grid),
        "test fixture needs far creatures to share the player's grid"
    );
    let grid = grid_coord_for_position(center);
    let now = Instant::now();
    let mut runtimes = Vec::new();
    let far_ready_count = 8;
    for guid in 300..(300 + far_ready_count) {
        let mut spawn = test_creature_spawn(6);
        spawn.guid = guid;
        spawn.position_x = far_same_grid.x;
        spawn.position_y = far_same_grid.y;
        spawn.position_z = far_same_grid.z;
        spawn.movement_type = DB_MOTION_TYPE_WAYPOINT;
        spawn.waypoint_path = vec![test_waypoint(1, far_same_grid.x + 5.0, far_same_grid.y, 0)];
        let mut runtime = DbCreatureRuntime::new(spawn);
        runtime.next_waypoint_move_at = Some(now);
        runtimes.push(runtime);
    }

    let mut visible_spawn = test_creature_spawn(6);
    visible_spawn.guid = 999;
    visible_spawn.position_x = center.x + 5.0;
    visible_spawn.position_y = center.y;
    visible_spawn.position_z = center.z;
    visible_spawn.movement_type = DB_MOTION_TYPE_WAYPOINT;
    visible_spawn.waypoint_path = vec![test_waypoint(1, center.x + 10.0, center.y, 0)];
    let visible_guid = creature_spawn_guid(&visible_spawn);
    let mut visible_runtime = DbCreatureRuntime::new(visible_spawn);
    visible_runtime.next_waypoint_move_at = Some(now);
    runtimes.push(visible_runtime);

    map.insert_loaded_creature_grid(grid, runtimes);
    map.add_player(test_player_runtime(8, SessionId(8), center))
        .unwrap();

    assert_eq!(
        map.db_creature_idle_motion_start_guids(now),
        vec![visible_guid.raw()],
        "same-grid creatures outside visibility should not starve the nearby patrol start budget"
    );
}

#[test]
fn map_runtime_idle_motion_tick_is_once_per_map_tick() {
    let mut map = MapRuntime::new(0, 0);
    let center = WorldPosition::new(0, -8950.0, -130.0, 83.5, 0.0);
    let now = Instant::now();
    let mut runtimes = Vec::new();
    let ready_creatures = 5;
    for guid in 300..(300 + ready_creatures) {
        let mut spawn = test_creature_spawn(6);
        spawn.guid = guid;
        spawn.position_x = center.x;
        spawn.position_y = center.y;
        spawn.position_z = center.z;
        spawn.movement_type = DB_MOTION_TYPE_WAYPOINT;
        spawn.waypoint_path = vec![test_waypoint(1, center.x + 5.0, center.y, 0)];
        let mut runtime = DbCreatureRuntime::new(spawn);
        runtime.next_waypoint_move_at = Some(now);
        runtimes.push(runtime);
    }
    map.insert_loaded_creature_grid(grid_coord_for_position(center), runtimes);
    map.add_player(test_player_runtime(8, SessionId(8), center))
        .unwrap();

    let first = map
        .advance_active_db_creature_idle_motions(&DbCreatureNavigationGuardrail::default(), now)
        .unwrap();
    assert_eq!(
        first
            .packets
            .iter()
            .filter(|(_, packet)| packet.opcode == WorldOpcode::SmsgMonsterMove as u16)
            .count(),
        ready_creatures as usize
    );

    let duplicate = map
        .advance_active_db_creature_idle_motions(
            &DbCreatureNavigationGuardrail::default(),
            now + Duration::from_millis(1),
        )
        .unwrap();
    assert!(duplicate.creatures.is_empty());
    assert!(duplicate.packets.is_empty());

    let next = map
        .advance_active_db_creature_idle_motions(
            &DbCreatureNavigationGuardrail::default(),
            now + Duration::from_millis(WORLD_TICK_MILLIS),
        )
        .unwrap();
    assert!(next.packets.is_empty());
}

#[test]
fn map_runtime_idle_motion_tracking_drops_active_mover_on_combat_claim() {
    let mut map = MapRuntime::new(0, 0);
    let center = WorldPosition::new(0, -8950.0, -130.0, 83.5, 0.0);
    let now = Instant::now();
    let mut spawn = test_creature_spawn(6);
    spawn.guid = 333;
    spawn.position_x = center.x;
    spawn.position_y = center.y;
    spawn.position_z = center.z;
    spawn.movement_type = DB_MOTION_TYPE_WAYPOINT;
    spawn.waypoint_path = vec![test_waypoint(1, center.x + 5.0, center.y, 0)];
    let guid = creature_spawn_guid(&spawn);
    let mut runtime = DbCreatureRuntime::new(spawn);
    runtime.next_waypoint_move_at = Some(now);
    map.insert_loaded_creature_grid(grid_coord_for_position(center), vec![runtime]);
    map.add_player(test_player_runtime(8, SessionId(8), center))
        .unwrap();

    let tick = map
        .advance_active_db_creature_idle_motions(&DbCreatureNavigationGuardrail::default(), now)
        .unwrap();
    assert_eq!(tick.creatures.len(), 1);
    assert!(map.active_db_creature_motion_guids.contains(&guid.raw()));

    let victim = ObjectGuid::new(HighGuid::Player, 0, 8);
    map.begin_db_creature_combat(guid, victim, now + Duration::from_millis(1))
        .expect("moving creature should enter combat");

    assert!(map
        .db_creature_idle_motion_advancement_guids(now + Duration::from_millis(1))
        .guids
        .is_empty());
    assert!(!map.active_db_creature_motion_guids.contains(&guid.raw()));
}

#[test]
fn map_runtime_idle_motion_advancement_queue_only_returns_due_movers() {
    let mut map = MapRuntime::new(0, 0);
    let now = Instant::now();
    let center = WorldPosition::new(0, -8950.0, -130.0, 83.5, 0.0);
    let grid = grid_coord_for_position(center);
    let future_due_at = now + Duration::from_secs(5);

    let mut due_spawn = test_creature_spawn(6);
    due_spawn.guid = 501;
    due_spawn.position_x = center.x;
    due_spawn.position_y = center.y;
    due_spawn.position_z = center.z;
    let due_guid = creature_spawn_guid(&due_spawn);
    let due_destination = WorldPosition::new(0, center.x + 5.0, center.y, center.z, 0.0);
    let mut due_runtime = DbCreatureRuntime::new(due_spawn);
    due_runtime.motion = CreatureMotionState::Waypoint(CreatureWaypointMotion {
        node_index: 0,
        start: center,
        destination: due_destination,
        path: vec![due_destination],
        started_at: now,
        duration: Duration::from_secs(2),
    });
    due_runtime.next_waypoint_move_at = None;

    let mut future_spawn = test_creature_spawn(6);
    future_spawn.guid = 502;
    future_spawn.position_x = center.x + 10.0;
    future_spawn.position_y = center.y;
    future_spawn.position_z = center.z;
    let future_guid = creature_spawn_guid(&future_spawn);
    let future_destination = WorldPosition::new(0, center.x + 15.0, center.y, center.z, 0.0);
    let mut future_runtime = DbCreatureRuntime::new(future_spawn);
    future_runtime.motion = CreatureMotionState::Waypoint(CreatureWaypointMotion {
        node_index: 0,
        start: WorldPosition::new(0, center.x + 10.0, center.y, center.z, 0.0),
        destination: future_destination,
        path: vec![future_destination],
        started_at: now,
        duration: Duration::from_secs(2),
    });
    future_runtime.next_waypoint_move_at = None;

    map.insert_loaded_creature_grid(grid, vec![due_runtime, future_runtime]);
    map.add_player(test_player_runtime(8, SessionId(8), center))
        .unwrap();
    map.active_db_creature_motion_guids.insert(due_guid.raw());
    map.active_db_creature_motion_guids
        .insert(future_guid.raw());
    map.db_creature_motion_advance_due_at
        .insert(due_guid.raw(), now);
    map.idle_db_creature_motion_advances
        .push(Reverse(ScheduledDbCreatureMotionAdvance {
            due_at: now,
            guid: due_guid.raw(),
        }));
    map.db_creature_motion_advance_due_at
        .insert(future_guid.raw(), future_due_at);
    map.idle_db_creature_motion_advances
        .push(Reverse(ScheduledDbCreatureMotionAdvance {
            due_at: future_due_at,
            guid: future_guid.raw(),
        }));

    let ready = map.db_creature_idle_motion_advancement_guids(now);

    assert_eq!(ready.guids, vec![due_guid.raw()]);
    assert!(
        !map.db_creature_motion_advance_due_at
            .contains_key(&due_guid.raw()),
        "ready mover should be removed from the due map until it is rescheduled by advancement"
    );
    assert_eq!(
        map.db_creature_motion_advance_due_at
            .get(&future_guid.raw()),
        Some(&future_due_at),
        "future mover should stay queued for its later due time"
    );
}

#[test]
fn map_runtime_idle_motion_start_reschedules_advancement_without_spatial_refresh() {
    let now = Instant::now();
    let mut map = MapRuntime::with_geometry(
        0,
        0,
        Arc::new(WorldGeometry::default()),
        Arc::new(DbScriptRegistry::default()),
    );
    let position = WorldPosition::new(0, -8950.0, -130.0, 83.5, 0.0);
    insert_map_runtime_player_for_test(&mut map, 1, position);

    let mut spawn = test_creature_spawn(6);
    spawn.guid = 777;
    spawn.position_x = position.x;
    spawn.position_y = position.y;
    spawn.position_z = position.z;
    spawn.movement_type = DB_MOTION_TYPE_RANDOM;
    spawn.spawn_dist = 5.0;
    let guid = creature_spawn_guid(&spawn);
    let mut runtime = DbCreatureRuntime::new(spawn);
    runtime.next_random_move_at = Some(now);

    map.insert_loaded_creature_grid(grid_coord_for_position(position), vec![runtime]);
    let expected_due_at = now + Duration::from_millis(WORLD_TICK_MILLIS);
    map.next_idle_motion_tick_at = Some(expected_due_at);

    let (snapshot, motion, script_ids) = map
        .start_db_creature_idle_motion(&DbCreatureNavigationGuardrail::default(), guid, now)
        .outcome
        .expect("idle creature should start moving");

    assert!(motion.is_some());
    assert!(script_ids.is_empty());
    assert_eq!(snapshot.guid(), guid);
    assert!(map.active_db_creature_motion_guids.contains(&guid.raw()));
    assert_eq!(
        map.db_creature_motion_advance_due_at.get(&guid.raw()),
        Some(&expected_due_at),
        "motion start should still reschedule the next advancement tick even without a spatial refresh"
    );
}

#[test]
fn map_runtime_idle_motion_tick_uses_configured_world_tick_interval() {
    let mut map = MapRuntime::new(0, 0);
    let now = Instant::now();
    let tick = Duration::from_millis(250);

    map.advance_active_db_creature_idle_motions_with_interval(
        &DbCreatureNavigationGuardrail::default(),
        now,
        tick,
    )
    .unwrap();

    assert_eq!(map.next_idle_motion_tick_at, Some(now + tick));
}

#[test]
fn map_runtime_idle_motion_zero_distance_waypoint_does_not_block_other_starts() {
    let mut map = MapRuntime::new(0, 0);
    let center = WorldPosition::new(0, -8950.0, -130.0, 83.5, 0.0);
    let now = Instant::now();
    let mut runtimes = Vec::new();

    let mut blocked_spawn = test_creature_spawn(6);
    blocked_spawn.guid = 200;
    blocked_spawn.position_x = center.x;
    blocked_spawn.position_y = center.y;
    blocked_spawn.position_z = center.z;
    blocked_spawn.movement_type = DB_MOTION_TYPE_WAYPOINT;
    blocked_spawn.waypoint_path = vec![
        test_waypoint(1, center.x, center.y, 60_000),
        test_waypoint(2, center.x + 10.0, center.y, 0),
    ];
    let blocked_guid = creature_spawn_guid(&blocked_spawn);
    let mut blocked_runtime = DbCreatureRuntime::new(blocked_spawn);
    blocked_runtime.next_waypoint_move_at = Some(now);
    runtimes.push(blocked_runtime);

    let ready_creatures = 4;
    for guid in 300..(300 + ready_creatures) {
        let mut spawn = test_creature_spawn(6);
        spawn.guid = guid;
        spawn.position_x = center.x;
        spawn.position_y = center.y;
        spawn.position_z = center.z;
        spawn.movement_type = DB_MOTION_TYPE_WAYPOINT;
        spawn.waypoint_path = vec![test_waypoint(1, center.x + 5.0, center.y, 0)];
        let mut runtime = DbCreatureRuntime::new(spawn);
        runtime.next_waypoint_move_at = Some(now);
        runtimes.push(runtime);
    }
    map.insert_loaded_creature_grid(grid_coord_for_position(center), runtimes);
    map.add_player(test_player_runtime(8, SessionId(8), center))
        .unwrap();

    let first = map
        .advance_active_db_creature_idle_motions(&DbCreatureNavigationGuardrail::default(), now)
        .unwrap();
    assert_eq!(
        first
            .packets
            .iter()
            .filter(|(_, packet)| packet.opcode == WorldOpcode::SmsgMonsterMove as u16)
            .count(),
        ready_creatures as usize
    );
    let blocked = map.creatures.get(&blocked_guid.raw()).unwrap();
    assert_eq!(blocked.waypoint_next_index, 1);
    assert_eq!(
        blocked.next_waypoint_move_at,
        Some(now + Duration::from_millis(60_000))
    );

    let next = map
        .advance_active_db_creature_idle_motions(
            &DbCreatureNavigationGuardrail::default(),
            now + Duration::from_millis(WORLD_TICK_MILLIS),
        )
        .unwrap();
    assert!(next.packets.is_empty());
}

#[test]
fn map_runtime_idle_motion_newly_loaded_due_creature_wakes_start_schedule() {
    let mut map = MapRuntime::new(0, 0);
    let center = WorldPosition::new(0, -8950.0, -130.0, 83.5, 0.0);
    let now = Instant::now();

    let mut future_spawn = test_creature_spawn(6);
    future_spawn.guid = 200;
    future_spawn.position_x = center.x;
    future_spawn.position_y = center.y;
    future_spawn.position_z = center.z;
    future_spawn.movement_type = DB_MOTION_TYPE_WAYPOINT;
    future_spawn.waypoint_path = vec![test_waypoint(1, center.x + 5.0, center.y, 0)];
    let mut future_runtime = DbCreatureRuntime::new(future_spawn);
    future_runtime.next_waypoint_move_at = Some(now + Duration::from_secs(5));
    map.insert_loaded_creature_grid(grid_coord_for_position(center), vec![future_runtime]);
    map.add_player(test_player_runtime(8, SessionId(8), center))
        .unwrap();

    let first = map
        .advance_active_db_creature_idle_motions(&DbCreatureNavigationGuardrail::default(), now)
        .unwrap();
    assert!(first.packets.is_empty());

    let due_position = WorldPosition::new(0, center.x + 10.0, center.y, center.z, 0.0);
    let mut due_spawn = test_creature_spawn(6);
    due_spawn.guid = 201;
    due_spawn.position_x = due_position.x;
    due_spawn.position_y = due_position.y;
    due_spawn.position_z = due_position.z;
    due_spawn.movement_type = DB_MOTION_TYPE_WAYPOINT;
    due_spawn.waypoint_path = vec![test_waypoint(1, due_position.x + 5.0, due_position.y, 0)];
    let due_guid = creature_spawn_guid(&due_spawn);
    let mut due_runtime = DbCreatureRuntime::new(due_spawn);
    due_runtime.next_waypoint_move_at = Some(now);
    map.insert_loaded_creature_grid(grid_coord_for_position(due_position), vec![due_runtime]);

    let next = map
        .advance_active_db_creature_idle_motions(
            &DbCreatureNavigationGuardrail::default(),
            now + Duration::from_millis(WORLD_TICK_MILLIS),
        )
        .unwrap();
    assert_eq!(
        next.packets
            .iter()
            .filter(|(_, packet)| packet.opcode == WorldOpcode::SmsgMonsterMove as u16)
            .count(),
        1
    );
    let due = map.creatures.get(&due_guid.raw()).unwrap();
    assert!(matches!(due.motion, CreatureMotionState::Waypoint(_)));
}

#[tokio::test]
async fn shared_db_creature_idle_motion_prioritizes_player_interest_over_far_guid_order() {
    let maps = Arc::new(MapRuntimeManager::default());
    let sessions = Arc::new(SessionRegistry::default());
    let player_position = WorldPosition::new(0, 0.0, 0.0, 83.5, 0.0);
    maps.add_player(test_player_runtime(1, SessionId(1), player_position))
        .await
        .unwrap();

    let (direct_tx, mut direct_rx) = mpsc::unbounded_channel();
    sessions
        .register(
            SessionId(1),
            SessionHandle {
                account_id: 1,
                character_guid: Some(1),
                character_name: Some("Player1".to_string()),
                outbound: WorldPacketSender::Unbounded(direct_tx),
                disconnect: None,
            },
        )
        .await;
    let now = Instant::now();

    let mut runtimes = Vec::new();
    for guid in 300..304 {
        let mut spawn = test_creature_spawn(6);
        spawn.guid = guid;
        spawn.position_x = 4000.0;
        spawn.position_y = 4000.0;
        spawn.position_z = 83.5;
        spawn.movement_type = DB_MOTION_TYPE_RANDOM;
        spawn.spawn_dist = 5.0;
        let mut runtime = DbCreatureRuntime::new(spawn);
        runtime.next_random_move_at = Some(now);
        runtimes.push(runtime);
    }

    let mut valid_spawn = test_creature_spawn(6);
    valid_spawn.guid = 304;
    valid_spawn.position_x = 0.0;
    valid_spawn.position_y = 0.0;
    valid_spawn.position_z = 83.5;
    valid_spawn.movement_type = DB_MOTION_TYPE_WAYPOINT;
    valid_spawn.waypoint_path = vec![test_waypoint(1, 5.0, 0.0, 0)];
    let valid_guid = creature_spawn_guid(&valid_spawn);
    let mut valid_runtime = DbCreatureRuntime::new(valid_spawn);
    valid_runtime.next_waypoint_move_at = Some(now);
    runtimes.push(valid_runtime);

    maps.share_db_creature_snapshots(0, runtimes).await;

    let tick = maps
        .advance_all_active_db_creature_idle_motions(&DbCreatureNavigationGuardrail::default(), now)
        .await
        .unwrap();
    sessions.dispatch(tick.packets).await;

    assert_eq!(
        direct_rx.try_recv().unwrap().opcode,
        WorldOpcode::SmsgUpdateObject as u16,
        "nearby patrol should recreate local visibility before motion when the session has not streamed the creature yet"
    );
    assert_eq!(
        direct_rx.try_recv().unwrap().opcode,
        WorldOpcode::SmsgMonsterMove as u16,
        "nearby patrol should still start even when lower GUID patrols exist in unrelated far grids"
    );
    let creature = maps
        .db_creature_snapshots(0, &[valid_guid.raw()])
        .await
        .pop()
        .expect("valid creature should stay loaded");
    assert!(
        matches!(creature.motion, CreatureMotionState::Waypoint(_)),
        "nearby creature should enter waypoint motion instead of starving behind far-map GUID order"
    );
}

#[test]
fn map_runtime_player_health_update_refreshes_shared_state_and_observers() {
    let mut map = MapRuntime::new(0, 0);
    let player_position = WorldPosition::new(0, -8950.0, -130.0, 83.5, 0.0);
    let observer_position = WorldPosition::new(0, -8952.0, -130.0, 83.5, 0.0);
    map.add_player(test_player_runtime(1, SessionId(1), player_position))
        .unwrap();
    map.add_player(test_player_runtime(2, SessionId(2), observer_position))
        .unwrap();

    let packets = map.update_player_health(1, 10).unwrap();

    assert_eq!(map.players.get(&1).unwrap().health, 10);
    assert_eq!(packets.len(), 1);
    assert_eq!(packets[0].0, SessionId(2));
    assert_eq!(packets[0].1.opcode, WorldOpcode::SmsgUpdateObject as u16);
    let player = ObjectGuid::new(HighGuid::Player, 0, 1);
    let (values, trailing) = decode_values_update_block(&packets[0].1.body[5..], player);
    assert!(trailing.is_empty());
    assert_eq!(values[UNIT_FIELD_HEALTH], Some(10));
}

#[test]
fn map_runtime_player_power1_update_refreshes_shared_state_and_observers() {
    let mut map = MapRuntime::new(0, 0);
    let player_position = WorldPosition::new(0, -8950.0, -130.0, 83.5, 0.0);
    let observer_position = WorldPosition::new(0, -8952.0, -130.0, 83.5, 0.0);
    let mut player = test_player_runtime(1, SessionId(1), player_position);
    player.max_power1 = 100;
    player.power1 = 20;
    map.add_player(player).unwrap();
    map.add_player(test_player_runtime(2, SessionId(2), observer_position))
        .unwrap();

    let packets = map.update_player_power1(1, 75).unwrap();

    assert_eq!(map.players.get(&1).unwrap().power1, 75);
    assert_eq!(packets.len(), 1);
    assert_eq!(packets[0].0, SessionId(2));
    assert_eq!(packets[0].1.opcode, WorldOpcode::SmsgUpdateObject as u16);
    let player = ObjectGuid::new(HighGuid::Player, 0, 1);
    let (values, trailing) = decode_values_update_block(&packets[0].1.body[5..], player);
    assert!(trailing.is_empty());
    assert_eq!(values[UNIT_FIELD_POWER1], Some(75));
}

#[test]
fn map_owned_consumable_resource_updates_survive_later_session_sync() {
    let mut map = MapRuntime::new(0, 0);
    let position = WorldPosition::new(0, -8950.0, -130.0, 83.5, 0.0);
    let mut player = test_player_runtime(7, SessionId(7), position);
    let world_stats = PlayerWorldStats {
        base_health: 80,
        base_mana: 80,
        stats: [20, 20, 20, 20, 20],
        next_level_xp: 400,
    };
    player.base_world_stats = world_stats;
    player.effective_world_stats = world_stats;
    player.health = 20;
    player.max_health = world_stats.max_health();
    player.power1 = 30;
    player.max_power1 = world_stats.max_mana();
    map.add_player(player).unwrap();

    map.apply_player_heal(7, 45).unwrap().unwrap();
    map.update_player_power1(7, 80).unwrap();

    let session = WorldSessionState {
        character: CharacterSessionState {
            active_character: Some(ActiveCharacter {
                guid: 7,
                name: "Ada".to_string(),
                race: 1,
                class: 1,
                level: 1,
                xp: 0,
                position,
                movement_flags: 0,
                client_time: 0,
                fall_time: 0,
                jump: JumpInfo::default(),
            }),
            player_health: 20,
            player_mana: 30,
            player_stand_state: PLAYER_STAND_STATE_STAND,
            ..CharacterSessionState::default()
        },
        ..WorldSessionState::default()
    };

    map.sync_player_gameplay_state(7, &session);
    let snapshot = map.player_runtime_snapshot(7).unwrap();
    assert_eq!(
        snapshot.health, 65,
        "alive session sync must not rewind a map-owned item heal"
    );
    assert_eq!(
        snapshot.power1, 80,
        "alive session sync must not rewind a map-owned item energize"
    );
}

#[test]
fn map_runtime_session_sync_persists_rested_xp_visual_state() {
    let mut map = MapRuntime::new(0, 0);
    let position = WorldPosition::new(0, -8950.0, -130.0, 83.5, 0.0);
    map.add_player(test_player_runtime(7, SessionId(7), position))
        .unwrap();
    let rested_player_bytes2 = player_bytes2_with_rest_bonus(0, 11.0);
    let session = WorldSessionState {
        character: CharacterSessionState {
            active_character: Some(ActiveCharacter {
                guid: 7,
                name: "Ada".to_string(),
                race: 1,
                class: 1,
                level: 1,
                xp: 0,
                position,
                movement_flags: 0,
                client_time: 0,
                fall_time: 0,
                jump: JumpInfo::default(),
            }),
            player_visual: Some(PlayerVisualState {
                gender: 0,
                player_bytes: 0,
                player_bytes2: rested_player_bytes2,
                equipment_cache: None,
                guildid: None,
            }),
            ..CharacterSessionState::default()
        },
        rest: RestSessionState {
            rest_bonus: 11.0,
            ..RestSessionState::default()
        },
        ..WorldSessionState::default()
    };

    map.sync_player_gameplay_state(7, &session);

    let snapshot = map.player_runtime_snapshot(7).unwrap();
    assert_eq!(snapshot.rest_bonus, 11.0);
    assert_eq!(snapshot.player_bytes2, rested_player_bytes2);
    let session_snapshot = map.player_runtime_session_snapshot(7).unwrap();
    assert_eq!(session_snapshot.rest_bonus, 11.0);
}

#[test]
fn map_runtime_fall_land_applies_cmangos_fall_damage_and_log() {
    let mut map = MapRuntime::new(0, 0);
    let start = WorldPosition::new(0, -8950.0, -130.0, 100.0, 0.0);
    map.add_player(test_player_runtime(1, SessionId(1), start))
        .unwrap();

    let falling = MovementInfo {
        flags: MOVEFLAG_JUMPING,
        client_time: 1,
        position: WorldPosition::new(0, -8950.0, -130.0, 90.0, 0.0),
        fall_time: 500,
        jump: JumpInfo::default(),
    };
    map.update_player_position(1, WorldOpcode::MsgMoveHeartbeat as u16, &falling, 1)
        .unwrap();

    let landing = MovementInfo {
        flags: 0,
        client_time: 2,
        position: WorldPosition::new(0, -8950.0, -130.0, 70.0, 0.0),
        fall_time: 900,
        jump: JumpInfo::default(),
    };
    let packets = map
        .update_player_position(1, WorldOpcode::MsgMoveFallLand as u16, &landing, 2)
        .unwrap();

    assert_eq!(map.players.get(&1).unwrap().health, 15);
    assert!(packets
        .iter()
        .any(|(session, packet)| *session == SessionId(1)
            && packet.opcode == WorldOpcode::SmsgEnvironmentalDamageLog as u16));
    let health_packet = packets
        .iter()
        .find(|(session, packet)| {
            *session == SessionId(1) && packet.opcode == WorldOpcode::SmsgUpdateObject as u16
        })
        .expect("direct fall health update");
    let player = ObjectGuid::new(HighGuid::Player, 0, 1);
    let (values, trailing) = decode_values_update_block(&health_packet.1.body[5..], player);
    assert!(trailing.is_empty());
    assert_eq!(values[UNIT_FIELD_HEALTH], Some(15));
}

#[test]
fn map_runtime_underwater_breath_timer_applies_drowning_damage_and_log() {
    let mut map = MapRuntime::new(0, 0);
    let position = WorldPosition::new(0, -8950.0, -130.0, 83.5, 0.0);
    map.add_player(test_player_runtime(1, SessionId(1), position))
        .unwrap();
    let now = Instant::now();
    map.refresh_player_environment_flags_for_test(1, now, |_geometry, _position| {
        ENVIRONMENT_FLAG_LIQUID | ENVIRONMENT_FLAG_UNDERWATER | ENVIRONMENT_FLAG_IN_WATER
    })
    .unwrap();

    let start_packets = map
        .advance_player_environment_tick_with_flags(now, |_geometry, _position| 0)
        .unwrap();
    assert!(start_packets
        .iter()
        .any(|(session, packet)| *session == SessionId(1)
            && packet.opcode == WorldOpcode::SmsgStartMirrorTimer as u16));

    let packets = map
        .advance_player_environment_tick_with_flags(
            now + Duration::from_secs(60),
            |_geometry, _position| {
                ENVIRONMENT_FLAG_LIQUID | ENVIRONMENT_FLAG_UNDERWATER | ENVIRONMENT_FLAG_IN_WATER
            },
        )
        .unwrap();

    assert_eq!(map.players.get(&1).unwrap().health, 16);
    assert!(packets
        .iter()
        .any(|(session, packet)| *session == SessionId(1)
            && packet.opcode == WorldOpcode::SmsgEnvironmentalDamageLog as u16
            && packet.body.get(PackedGuid::packed_size(ObjectGuid::new(
                HighGuid::Player,
                0,
                1
            ))) == Some(&DAMAGE_DROWNING)));
}

#[test]
fn map_runtime_water_breathing_aura_stops_underwater_breath_timer() {
    let mut map = MapRuntime::new(0, 0);
    let position = WorldPosition::new(0, -8950.0, -130.0, 83.5, 0.0);
    map.add_player(test_player_runtime(1, SessionId(1), position))
        .unwrap();
    let now = Instant::now();
    map.refresh_player_environment_flags_for_test(1, now, |_geometry, _position| {
        ENVIRONMENT_FLAG_LIQUID | ENVIRONMENT_FLAG_UNDERWATER | ENVIRONMENT_FLAG_IN_WATER
    })
    .unwrap();

    let start_packets = map
        .advance_player_environment_tick_with_flags(now, |_geometry, _position| 0)
        .unwrap();
    assert!(start_packets
        .iter()
        .any(|(session, packet)| *session == SessionId(1)
            && packet.opcode == WorldOpcode::SmsgStartMirrorTimer as u16));

    let mut water_breathing_template = test_spell_template(5697);
    water_breathing_template.spell_name = "Unending Breath".to_string();
    water_breathing_template.effect1 = SPELL_EFFECT_APPLY_AURA;
    water_breathing_template.effect_apply_aura_name1 = SPELL_AURA_WATER_BREATHING;
    let water_breathing = build_active_aura(
        &water_breathing_template,
        ObjectGuid::new(HighGuid::Player, 0, 7),
        16,
        test_spell_effect_value_context(&water_breathing_template),
        now,
        None,
    );
    map.players
        .get_mut(&1)
        .expect("player should still exist in map runtime")
        .active_auras
        .push(water_breathing);

    let packets = map
        .advance_player_environment_tick_with_flags(
            now + Duration::from_secs(60),
            |_geometry, _position| {
                ENVIRONMENT_FLAG_LIQUID | ENVIRONMENT_FLAG_UNDERWATER | ENVIRONMENT_FLAG_IN_WATER
            },
        )
        .unwrap();

    assert_eq!(map.players.get(&1).unwrap().health, 20);
    assert!(!map.players.get(&1).unwrap().environment.breath.active);
    assert!(packets
        .iter()
        .any(|(session, packet)| *session == SessionId(1)
            && packet.opcode == WorldOpcode::SmsgStopMirrorTimer as u16));
    assert!(!packets
        .iter()
        .any(|(session, packet)| *session == SessionId(1)
            && packet.opcode == WorldOpcode::SmsgEnvironmentalDamageLog as u16));
}

#[test]
fn map_runtime_player_environment_tick_skips_playerbots() {
    let mut map = MapRuntime::new(0, 0);
    let position = WorldPosition::new(0, -8950.0, -130.0, 83.5, 0.0);
    map.add_player(test_player_runtime(1, SessionId(1), position))
        .unwrap();
    for guid in 2..18 {
        map.add_player(test_bot_player_runtime(
            guid,
            BotId(u64::from(guid)),
            position,
        ))
        .unwrap();
    }
    let mut environment_checks = 0usize;

    let packets = map
        .advance_player_environment_tick_with_flags(Instant::now(), |_geometry, _position| {
            environment_checks += 1;
            ENVIRONMENT_FLAG_LIQUID | ENVIRONMENT_FLAG_UNDERWATER | ENVIRONMENT_FLAG_IN_WATER
        })
        .unwrap();

    assert_eq!(environment_checks, 0);
    assert!(packets.is_empty());
    for guid in 2..18 {
        assert_eq!(map.players.get(&guid).unwrap().environment.flags, 0);
    }
}

#[test]
fn map_runtime_add_player_refreshes_environment_cache_on_login() {
    let mut map = MapRuntime::new(0, 0);
    let position = WorldPosition::new(0, -8950.0, -130.0, 83.5, 0.0);
    map.add_player(test_player_runtime(1, SessionId(1), position))
        .unwrap();
    let environment = &map.players.get(&1).unwrap().environment;
    assert_eq!(environment.last_flags_position, Some(position));
    assert_eq!(environment.next_flags_refresh_at, None);
    assert!(map.active_player_environment_guids.is_empty());
}

#[test]
fn map_runtime_player_environment_tick_uses_cached_safe_flags_without_geometry_refresh() {
    let mut map = MapRuntime::new(0, 0);
    let position = WorldPosition::new(0, -8950.0, -130.0, 83.5, 0.0);
    map.add_player(test_player_runtime(1, SessionId(1), position))
        .unwrap();
    let now = Instant::now();
    let mut environment_checks = 0usize;
    let last_tick_at = map.players.get(&1).unwrap().environment.last_tick_at;

    let packets = map
        .advance_player_environment_tick_with_flags(now, |_geometry, _position| {
            environment_checks += 1;
            0
        })
        .unwrap();

    assert_eq!(environment_checks, 0);
    assert!(packets.is_empty());
    assert_eq!(
        map.players.get(&1).unwrap().environment.last_tick_at,
        last_tick_at
    );
    assert!(map.active_player_environment_guids.is_empty());
}

#[test]
fn map_runtime_update_player_position_refreshes_environment_cache() {
    let mut map = MapRuntime::new(0, 0);
    let start = WorldPosition::new(0, 0.0, 0.0, 0.0, 0.0);
    map.add_player(test_player_runtime(1, SessionId(1), start))
        .unwrap();

    let moved = WorldPosition::new(0, 2.0, 0.0, 0.0, 0.0);
    map.update_player_position(
        1,
        WorldOpcode::MsgMoveHeartbeat as u16,
        &MovementInfo {
            flags: 0,
            client_time: 1,
            position: moved,
            fall_time: 0,
            jump: JumpInfo::default(),
        },
        2,
    )
    .unwrap();

    assert_eq!(
        map.players.get(&1).unwrap().environment.last_flags_position,
        Some(moved)
    );
}

#[test]
fn map_runtime_set_player_position_refreshes_environment_cache() {
    let mut map = MapRuntime::new(0, 0);
    let start = WorldPosition::new(0, 0.0, 0.0, 0.0, 0.0);
    map.add_player(test_player_runtime(1, SessionId(1), start))
        .unwrap();
    let destination = WorldPosition::new(0, 5.0, 0.0, 0.0, 0.0);

    map.set_player_position(1, destination).unwrap();

    assert_eq!(
        map.players.get(&1).unwrap().environment.last_flags_position,
        Some(destination)
    );
}

#[test]
fn map_runtime_set_player_position_rebuilds_player_visibility_immediately() {
    let mut map = MapRuntime::new(0, 0);
    let mover_start = WorldPosition::new(0, 0.0, 0.0, 0.0, 0.0);
    let old_observer_position = WorldPosition::new(0, 5.0, 0.0, 0.0, 0.0);
    let destination = WorldPosition::new(0, 500.0, 0.0, 0.0, 1.25);
    let new_observer_position = WorldPosition::new(0, 505.0, 0.0, 0.0, 0.0);
    map.add_player(test_player_runtime(1, SessionId(1), mover_start))
        .unwrap();
    map.add_player(test_player_runtime(2, SessionId(2), old_observer_position))
        .unwrap();
    map.add_player(test_player_runtime(3, SessionId(3), new_observer_position))
        .unwrap();

    let packets = map.set_player_position(1, destination).unwrap();

    assert!(packets.iter().any(|(session, packet)| {
        *session == SessionId(1) && packet.opcode == WorldOpcode::SmsgDestroyObject as u16
    }));
    assert!(packets.iter().any(|(session, packet)| {
        *session == SessionId(2) && packet.opcode == WorldOpcode::SmsgDestroyObject as u16
    }));
    let create_for_new_observer = packets
        .iter()
        .find(|(session, packet)| {
            *session == SessionId(3) && packet.opcode == WorldOpcode::SmsgUpdateObject as u16
        })
        .expect("new nearby observer should receive a fresh player create block");
    let player_guid = ObjectGuid::new(HighGuid::Player, 0, 1);
    let type_id_offset = 5 + 1 + PackedGuid::packed_size(player_guid);
    let movement_start = type_id_offset + 2;
    let movement = MovementInfo::read(&create_for_new_observer.1.body[movement_start..]).unwrap();
    assert_eq!(movement.position, destination);
    assert!(packets.iter().any(|(session, packet)| {
        *session == SessionId(1) && packet.opcode == WorldOpcode::SmsgUpdateObject as u16
    }));
    assert!(map.pending_player_visibility_refreshes.is_empty());
    assert!(map
        .pending_player_visibility_refresh_old_positions
        .is_empty());
    assert!(!map
        .players
        .get(&1)
        .unwrap()
        .visible_objects
        .contains(&ObjectGuid::new(HighGuid::Player, 0, 2)));
    assert!(!map
        .players
        .get(&2)
        .unwrap()
        .visible_objects
        .contains(&ObjectGuid::new(HighGuid::Player, 0, 1)));
    assert!(map
        .players
        .get(&1)
        .unwrap()
        .visible_objects
        .contains(&ObjectGuid::new(HighGuid::Player, 0, 3)));
    assert!(map
        .players
        .get(&3)
        .unwrap()
        .visible_objects
        .contains(&ObjectGuid::new(HighGuid::Player, 0, 1)));
}

#[test]
fn map_runtime_player_environment_tick_periodically_rechecks_active_cached_flags() {
    let mut map = MapRuntime::new(0, 0);
    let position = WorldPosition::new(0, -8950.0, -130.0, 83.5, 0.0);
    map.add_player(test_player_runtime(1, SessionId(1), position))
        .unwrap();
    let now = Instant::now();
    let mut environment_checks = 0usize;

    map.refresh_player_environment_flags_for_test(1, now, |_geometry, _position| {
        environment_checks += 1;
        ENVIRONMENT_FLAG_LIQUID | ENVIRONMENT_FLAG_UNDERWATER | ENVIRONMENT_FLAG_IN_WATER
    })
    .unwrap();
    assert_eq!(environment_checks, 1);
    assert!(map.active_player_environment_guids.contains(&1));

    let start_packets = map
        .advance_player_environment_tick_with_flags(
            now + Duration::from_millis(100),
            |_geometry, _position| {
                environment_checks += 1;
                ENVIRONMENT_FLAG_LIQUID | ENVIRONMENT_FLAG_UNDERWATER | ENVIRONMENT_FLAG_IN_WATER
            },
        )
        .unwrap();
    assert!(start_packets
        .iter()
        .any(|(session, packet)| *session == SessionId(1)
            && packet.opcode == WorldOpcode::SmsgStartMirrorTimer as u16));
    assert_eq!(environment_checks, 1);

    map.advance_player_environment_tick_with_flags(
        now + PLAYER_ENVIRONMENT_ACTIVE_FLAGS_REFRESH_INTERVAL + Duration::from_millis(1),
        |_geometry, _position| {
            environment_checks += 1;
            ENVIRONMENT_FLAG_LIQUID | ENVIRONMENT_FLAG_UNDERWATER | ENVIRONMENT_FLAG_IN_WATER
        },
    )
    .unwrap();
    assert_eq!(environment_checks, 2);
    assert!(map.active_player_environment_guids.contains(&1));
}

#[test]
fn map_runtime_environmental_damage_interrupts_regen() {
    let mut map = MapRuntime::new(0, 0);
    let position = WorldPosition::new(0, -8950.0, -130.0, 83.5, 0.0);
    let now = Instant::now();
    let mut player = test_player_runtime(1, SessionId(1), position);
    player.health = 80;
    player.max_health = 100;
    player.spirit = 100;
    player.stand_state = PLAYER_STAND_STATE_SIT;
    player.active_auras.push(ActiveAura {
        spell_id: 430,
        caster: ObjectGuid::new(HighGuid::Player, 0, 1),
        level: 1,
        interrupt_flags: 0,
        positive: true,
        visible: true,
        duration_millis: None,
        expires_at: None,
        periodic_damage: None,
        periodic_regen: Some(PeriodicRegenAura {
            health_amount: 30,
            mana_amount: 0,
            school_mask: 0,
            tick_millis: 2_000,
            next_tick_at: now + Duration::from_secs(60),
            interrupts_on_move_and_stand: true,
            suppresses_recent_damage: true,
            makes_player_sit: true,
        }),
        stat_modifiers: Vec::new(),
        proc_triggers: Vec::new(),
    });
    map.add_player(player).unwrap();
    map.advance_player_regen_tick(now).unwrap();
    map.refresh_player_environment_flags_for_test(1, now, |_geometry, _position| {
        ENVIRONMENT_FLAG_LIQUID | ENVIRONMENT_FLAG_UNDERWATER | ENVIRONMENT_FLAG_IN_WATER
    })
    .unwrap();

    map.advance_player_environment_tick_with_flags(now, |_geometry, _position| 0)
        .unwrap();
    map.advance_player_environment_tick_with_flags(
        now + Duration::from_secs(60),
        |_geometry, _position| {
            ENVIRONMENT_FLAG_LIQUID | ENVIRONMENT_FLAG_UNDERWATER | ENVIRONMENT_FLAG_IN_WATER
        },
    )
    .unwrap();
    map.advance_player_regen_tick(now + Duration::from_secs(60))
        .unwrap();

    let player = map.players.get(&1).unwrap();
    assert_eq!(player.health, 60);
    assert!(player.active_auras.is_empty());
    assert_eq!(player.stand_state, PLAYER_STAND_STATE_STAND);
}

#[test]
fn map_runtime_magma_environmental_timer_applies_lava_damage_without_client_timer() {
    let mut map = MapRuntime::new(0, 0);
    let position = WorldPosition::new(0, -8950.0, -130.0, 83.5, 0.0);
    let mut player = test_player_runtime(1, SessionId(1), position);
    player.health = 1_000;
    player.max_health = 1_000;
    map.add_player(player).unwrap();
    let now = Instant::now();
    map.refresh_player_environment_flags_for_test(1, now, |_geometry, _position| {
        ENVIRONMENT_FLAG_LIQUID | ENVIRONMENT_FLAG_IN_MAGMA
    })
    .unwrap();

    let start_packets = map
        .advance_player_environment_tick_with_flags(now, |_geometry, _position| 0)
        .unwrap();
    assert!(start_packets
        .iter()
        .all(|(_, packet)| packet.opcode != WorldOpcode::SmsgStartMirrorTimer as u16));

    let packets = map
        .advance_player_environment_tick_with_flags(
            now + Duration::from_secs(1),
            |_geometry, _position| ENVIRONMENT_FLAG_LIQUID | ENVIRONMENT_FLAG_IN_MAGMA,
        )
        .unwrap();

    let remaining = map.players.get(&1).unwrap().health;
    assert!((390..=395).contains(&remaining));
    assert!(packets
        .iter()
        .any(|(session, packet)| *session == SessionId(1)
            && packet.opcode == WorldOpcode::SmsgEnvironmentalDamageLog as u16
            && packet.body.get(PackedGuid::packed_size(ObjectGuid::new(
                HighGuid::Player,
                0,
                1
            ))) == Some(&DAMAGE_LAVA)));
}

#[test]
fn map_runtime_player_movement_preserves_db_creature_visibility_set() {
    let mut map = MapRuntime::new(0, 0);
    let start = WorldPosition::new(0, -8950.0, -130.0, 83.5, 0.0);
    map.add_player(test_player_runtime(1, SessionId(1), start))
        .unwrap();
    let creature_guid = ObjectGuid::new(HighGuid::Unit, 6, 77);
    map.update_player_db_creature_visibility(1, &[creature_guid], &[]);

    let movement = MovementInfo {
        flags: 0,
        client_time: 1,
        position: WorldPosition::new(0, -8949.0, -130.0, 83.5, 0.0),
        fall_time: 0,
        jump: JumpInfo::default(),
    };
    map.update_player_position(1, WorldOpcode::MsgMoveHeartbeat as u16, &movement, 2)
        .unwrap();

    assert!(map
        .players
        .get(&1)
        .unwrap()
        .visible_objects
        .contains(&creature_guid));
}

#[test]
fn map_runtime_stages_db_creature_visibility_from_player_visible_set() {
    let mut map = MapRuntime::new(0, 0);
    let player_position = WorldPosition::new(0, -8950.0, -130.0, 83.5, 0.0);
    map.add_player(test_player_runtime(1, SessionId(1), player_position))
        .unwrap();
    let mut spawn = test_creature_spawn(6);
    spawn.guid = 77;
    spawn.position_x = player_position.x + 5.0;
    spawn.position_y = player_position.y;
    let runtime = DbCreatureRuntime::new(spawn);
    let creature_guid = runtime.guid();
    map.share_db_creature_snapshots(vec![runtime.clone()]);

    let first = map.stage_player_db_creature_visibility(1, player_position, vec![runtime.clone()]);

    assert_eq!(first.create_guids, vec![creature_guid]);
    assert!(first.destroy_guids.is_empty());
    assert!(map
        .players
        .get(&1)
        .unwrap()
        .visible_objects
        .contains(&creature_guid));

    let far_position = WorldPosition::new(
        0,
        player_position.x + CREATURE_VISIBILITY_UNLOAD_RADIUS_YARDS + 10.0,
        player_position.y,
        player_position.z,
        player_position.orientation,
    );
    let second = map.stage_player_db_creature_visibility(1, far_position, Vec::new());

    assert!(second.create_guids.is_empty());
    assert_eq!(second.destroy_guids, vec![creature_guid]);
    assert!(!map
        .players
        .get(&1)
        .unwrap()
        .visible_objects
        .contains(&creature_guid));
}

#[test]
fn map_runtime_ghost_player_only_stages_creatures_visible_to_ghosts() {
    let mut map = MapRuntime::new(0, 0);
    let player_position = WorldPosition::new(0, -8950.0, -130.0, 83.5, 0.0);
    let mut player = test_player_runtime(1, SessionId(1), player_position);
    player.flags = PLAYER_FLAGS_GHOST;
    map.add_player(player).unwrap();

    let mut normal_spawn = test_creature_spawn(6);
    normal_spawn.guid = 77;
    normal_spawn.position_x = player_position.x + 5.0;
    normal_spawn.position_y = player_position.y;
    let normal = DbCreatureRuntime::new(normal_spawn);

    let mut ghost_visible_spawn = test_creature_spawn(6);
    ghost_visible_spawn.guid = 78;
    ghost_visible_spawn.position_x = player_position.x + 6.0;
    ghost_visible_spawn.position_y = player_position.y;
    ghost_visible_spawn.template.creature_type_flags = CREATURE_TYPE_FLAG_VISIBLE_TO_GHOSTS;
    let ghost_visible = DbCreatureRuntime::new(ghost_visible_spawn);
    let ghost_visible_guid = ghost_visible.guid();

    let stage = map.stage_player_db_creature_visibility(
        1,
        player_position,
        vec![normal.clone(), ghost_visible.clone()],
    );

    assert_eq!(stage.create_guids, vec![ghost_visible_guid]);
    assert!(stage.destroy_guids.is_empty());
    let visible = &map.players.get(&1).unwrap().visible_objects;
    assert!(!visible.contains(&normal.guid()));
    assert!(visible.contains(&ghost_visible_guid));
}

#[test]
fn map_runtime_alive_player_destroys_visible_ghost_visible_creature() {
    let mut map = MapRuntime::new(0, 0);
    let player_position = WorldPosition::new(0, -8950.0, -130.0, 83.5, 0.0);
    map.add_player(test_player_runtime(1, SessionId(1), player_position))
        .unwrap();

    let mut ghost_visible_spawn = test_creature_spawn(197);
    ghost_visible_spawn.guid = 79;
    ghost_visible_spawn.position_x = player_position.x + 5.0;
    ghost_visible_spawn.position_y = player_position.y;
    ghost_visible_spawn.template.creature_type_flags = CREATURE_TYPE_FLAG_VISIBLE_TO_GHOSTS;
    let ghost_visible = DbCreatureRuntime::new(ghost_visible_spawn);
    let ghost_visible_guid = ghost_visible.guid();
    map.share_db_creature_snapshots(vec![ghost_visible.clone()]);
    map.players
        .get_mut(&1)
        .unwrap()
        .visible_objects
        .insert(ghost_visible_guid);

    let stage =
        map.stage_player_db_creature_visibility(1, player_position, vec![ghost_visible.clone()]);

    assert!(stage.create_guids.is_empty());
    assert_eq!(stage.destroy_guids, vec![ghost_visible_guid]);
    assert!(!map
        .players
        .get(&1)
        .unwrap()
        .visible_objects
        .contains(&ghost_visible_guid));
}

#[test]
fn map_runtime_ghost_player_stages_ghost_visible_creature() {
    let mut map = MapRuntime::new(0, 0);
    let player_position = WorldPosition::new(0, -8950.0, -130.0, 83.5, 0.0);
    let mut player = test_player_runtime(1, SessionId(1), player_position);
    player.flags = PLAYER_FLAGS_GHOST;
    map.add_player(player).unwrap();

    let mut ghost_visible_spawn = test_creature_spawn(197);
    ghost_visible_spawn.guid = 79;
    ghost_visible_spawn.position_x = player_position.x + 5.0;
    ghost_visible_spawn.position_y = player_position.y;
    ghost_visible_spawn.template.creature_type_flags = CREATURE_TYPE_FLAG_VISIBLE_TO_GHOSTS;
    let ghost_visible = DbCreatureRuntime::new(ghost_visible_spawn);
    let ghost_visible_guid = ghost_visible.guid();

    let stage = map.stage_player_db_creature_visibility(1, player_position, vec![ghost_visible]);

    assert_eq!(stage.create_guids, vec![ghost_visible_guid]);
    assert!(stage.destroy_guids.is_empty());
    assert!(map
        .players
        .get(&1)
        .unwrap()
        .visible_objects
        .contains(&ghost_visible_guid));
}

#[test]
fn map_runtime_ghost_visible_creature_visibility_updates_after_death_state_sync_without_movement() {
    let mut map = MapRuntime::new(0, 0);
    let player_position = WorldPosition::new(0, -8950.0, -130.0, 83.5, 0.0);
    map.add_player(test_player_runtime(1, SessionId(1), player_position))
        .unwrap();

    let mut ghost_visible_spawn = test_creature_spawn(197);
    ghost_visible_spawn.guid = 80;
    ghost_visible_spawn.position_x = player_position.x + 5.0;
    ghost_visible_spawn.position_y = player_position.y;
    ghost_visible_spawn.template.creature_type_flags = CREATURE_TYPE_FLAG_VISIBLE_TO_GHOSTS;
    let ghost_visible = DbCreatureRuntime::new(ghost_visible_spawn);
    let ghost_visible_guid = ghost_visible.guid();
    map.share_db_creature_snapshots(vec![ghost_visible.clone()]);

    let alive_stage =
        map.stage_player_db_creature_visibility(1, player_position, vec![ghost_visible.clone()]);
    assert!(alive_stage.create_guids.is_empty());
    assert!(alive_stage.destroy_guids.is_empty());

    let mut session = WorldSessionState {
        character: CharacterSessionState {
            active_character: Some(ActiveCharacter {
                guid: 1,
                name: "Ada".to_string(),
                race: 1,
                class: 1,
                level: 1,
                xp: 0,
                position: player_position,
                movement_flags: 0,
                client_time: 0,
                fall_time: 0,
                jump: JumpInfo::default(),
            }),
            player_flags: PLAYER_FLAGS_GHOST,
            player_health: PLAYER_SURVIVOR_HEALTH_FLOOR,
            ..CharacterSessionState::default()
        },
        death: DeathSessionState {
            player_death_state: PlayerDeathState::Ghost,
            ..DeathSessionState::default()
        },
        ..WorldSessionState::default()
    };
    map.sync_player_gameplay_state(1, &session);
    map.reset_player_visibility_scan_positions(1);

    let ghost_stage =
        map.stage_player_db_creature_visibility(1, player_position, vec![ghost_visible.clone()]);
    assert_eq!(ghost_stage.create_guids, vec![ghost_visible_guid]);
    assert!(ghost_stage.destroy_guids.is_empty());
    assert!(map
        .players
        .get(&1)
        .unwrap()
        .visible_objects
        .contains(&ghost_visible_guid));

    session.character.player_flags &= !PLAYER_FLAGS_GHOST;
    session.death.player_death_state = PlayerDeathState::Alive;
    session.character.player_health = 42;
    map.sync_player_gameplay_state(1, &session);
    map.reset_player_visibility_scan_positions(1);

    let resurrect_stage =
        map.stage_player_db_creature_visibility(1, player_position, vec![ghost_visible]);
    assert!(resurrect_stage.create_guids.is_empty());
    assert_eq!(resurrect_stage.destroy_guids, vec![ghost_visible_guid]);
    assert!(!map
        .players
        .get(&1)
        .unwrap()
        .visible_objects
        .contains(&ghost_visible_guid));
}

#[test]
fn map_runtime_player_gameplay_sync_owns_session_mutable_state() {
    let mut map = MapRuntime::new(0, 0);
    let position = WorldPosition::new(0, -8950.0, -130.0, 83.5, 0.0);
    let mut player = test_player_runtime(1, SessionId(1), position);
    let world_stats = PlayerWorldStats {
        base_health: 20,
        base_mana: 80,
        stats: [23, 20, 22, 20, 20],
        next_level_xp: 400,
    };
    player.base_world_stats = world_stats;
    player.effective_world_stats = world_stats;
    player.max_power1 = world_stats.max_mana();
    map.add_player(player).unwrap();
    let mut session = WorldSessionState {
        character: CharacterSessionState {
            active_character: Some(ActiveCharacter {
                guid: 1,
                name: "Ada".to_string(),
                race: 1,
                class: 1,
                level: 1,
                xp: 0,
                position,
                movement_flags: 0,
                client_time: 0,
                fall_time: 0,
                jump: JumpInfo::default(),
            }),
            player_health: 15,
            player_mana: 7,
            player_rage: 11,
            ..CharacterSessionState::default()
        },
        ..WorldSessionState::default()
    };
    session
        .character
        .active_spells
        .insert(WARRIOR_HEROIC_STRIKE_RANK_1);
    session.inventory.items.push(CharacterInventoryItem {
        bag: 0,
        slot: 23,
        item: 100,
        item_template: RUST_VENDOR_BAG_ITEM,
        count: 1,
        flags: 0,
        random_property_id: 0,
        charges: String::new(),
        enchantments: String::new(),
        durability: 0,
    });
    session.quests.quest_statuses.insert(
        33,
        CharacterQuestStatus {
            quest: 33,
            status: QUEST_STATUS_INCOMPLETE,
            rewarded: 0,
            explored: 0,
            mobcount1: 1,
            mobcount2: 0,
            mobcount3: 0,
            mobcount4: 0,
        },
    );

    map.sync_player_gameplay_state(1, &session);

    let snapshot = map.player_runtime_snapshot(1).unwrap();
    assert_eq!(
        snapshot.health, 20,
        "alive player health is map-owned and must not be overwritten by a stale session cache"
    );
    assert_eq!(
        snapshot.power1, 0,
        "alive player mana is map-owned and must not be overwritten by a stale session cache"
    );
    assert_eq!(
        snapshot.power2, 0,
        "alive player rage is map-owned and must not be overwritten by a stale session cache"
    );
    assert!(snapshot
        .active_spells
        .contains(&WARRIOR_HEROIC_STRIKE_RANK_1));
    assert_eq!(snapshot.inventory.len(), 1);
    assert_eq!(snapshot.quest_statuses.get(&33).unwrap().mobcount1, 1);
}

#[test]
fn map_runtime_gameplay_sync_preserves_dead_player_zero_health() {
    let mut map = MapRuntime::new(0, 0);
    let position = WorldPosition::new(0, -8950.0, -130.0, 83.5, 0.0);
    map.add_player(test_player_runtime(7, SessionId(7), position))
        .unwrap();

    let mut session = WorldSessionState {
        character: CharacterSessionState {
            active_character: Some(ActiveCharacter {
                guid: 7,
                name: "Ada".to_string(),
                race: 1,
                class: 1,
                level: 1,
                xp: 0,
                position,
                movement_flags: 0,
                client_time: 0,
                fall_time: 0,
                jump: JumpInfo::default(),
            }),
            player_health: 0,
            ..CharacterSessionState::default()
        },
        death: DeathSessionState {
            player_death_state: PlayerDeathState::Corpse,
            ..DeathSessionState::default()
        },
        ..WorldSessionState::default()
    };

    map.sync_player_gameplay_state(7, &session);
    assert_eq!(
        map.player_runtime_snapshot(7).unwrap().health,
        0,
        "stat/aura refresh during session sync must not resurrect a corpse to 1 health"
    );

    session.death.player_death_state = PlayerDeathState::Alive;
    session.character.player_health = 42;
    if let Some(character) = session.character.active_character.as_mut() {
        character.position = WorldPosition::new(0, -8940.0, -120.0, 90.0, 1.0);
        character.movement_flags = MOVEFLAG_JUMPING;
        character.fall_time = 456;
        character.jump = JumpInfo {
            z_speed: 7.0,
            cos_angle: 1.0,
            sin_angle: 0.0,
            xy_speed: 4.5,
        };
    }
    map.sync_player_gameplay_state(7, &session);
    let snapshot = map.player_runtime_snapshot(7).unwrap();
    assert_eq!(
        snapshot.health, 0,
        "a stale alive session sync must not resurrect a map-owned corpse"
    );
    assert_eq!(snapshot.position, position);
    assert_eq!(snapshot.movement_flags, 0);
    assert_eq!(snapshot.fall_time, 0);
    map.update_player_health(7, 42).unwrap();
    map.sync_player_gameplay_state(7, &session);
    assert_eq!(map.player_runtime_snapshot(7).unwrap().health, 42);
}

#[tokio::test]
async fn session_cache_refresh_preserves_map_owned_regen_before_session_sync() {
    let maps = Arc::new(MapRuntimeManager::default());
    let position = WorldPosition::new(0, -8950.0, -130.0, 83.5, 0.0);
    let now = Instant::now();
    let mut player = test_player_runtime(7, SessionId(7), position);
    player.class = 1;
    player.spirit = 30;
    player.health = 10;
    player.max_health = 80;
    let world_stats = PlayerWorldStats {
        base_health: 80,
        base_mana: 100,
        stats: [20, 20, 30, 20, 30],
        next_level_xp: 400,
    };
    player.base_world_stats = world_stats;
    player.effective_world_stats = world_stats;
    player.power1 = 20;
    player.max_power1 = world_stats.max_mana();
    player.power2 = 100;
    player.active_auras.push(ActiveAura {
        spell_id: 430,
        caster: ObjectGuid::new(HighGuid::Player, 0, 7),
        level: 1,
        interrupt_flags: 0,
        positive: true,
        visible: true,
        duration_millis: Some(30_000),
        expires_at: Some(now + Duration::from_secs(30)),
        periodic_damage: None,
        periodic_regen: Some(PeriodicRegenAura {
            health_amount: 0,
            mana_amount: 9,
            school_mask: 0,
            tick_millis: 2_000,
            next_tick_at: now + Duration::from_secs(2),
            interrupts_on_move_and_stand: false,
            suppresses_recent_damage: false,
            makes_player_sit: false,
        }),
        stat_modifiers: Vec::new(),
        proc_triggers: Vec::new(),
    });
    maps.add_player(player).await.unwrap();

    assert!(maps
        .advance_all_player_regen_ticks(now)
        .await
        .unwrap()
        .is_empty());
    let packets = maps
        .advance_all_player_regen_ticks(now + Duration::from_secs(2))
        .await
        .unwrap();
    assert!(packets.len() >= 4);

    let stale_session_health = 10;
    let stale_session_mana = 20;
    let stale_session_rage = 100;
    let mut session = WorldSessionState {
        character: CharacterSessionState {
            active_character: Some(ActiveCharacter {
                guid: 7,
                name: "Ada".to_string(),
                race: 1,
                class: 1,
                level: 1,
                xp: 0,
                position,
                movement_flags: 0,
                client_time: 0,
                fall_time: 0,
                jump: JumpInfo::default(),
            }),
            player_health: stale_session_health,
            player_mana: stale_session_mana,
            player_rage: stale_session_rage,
            ..CharacterSessionState::default()
        },
        ..WorldSessionState::default()
    };

    refresh_active_player_session_cache(&maps, &mut session).await;
    assert!(session.character.player_health > stale_session_health);
    assert!(session.character.player_mana > stale_session_mana);
    assert!(session.character.player_rage < stale_session_rage);

    sync_active_player_gameplay_state(&maps, &session).await;
    let snapshot = maps.player_runtime_snapshot(0, 7).await.unwrap();
    assert_eq!(snapshot.health, session.character.player_health);
    assert_eq!(snapshot.power1, session.character.player_mana);
    assert_eq!(snapshot.power2, session.character.player_rage);
}

#[test]
fn sync_player_gameplay_state_ignores_stale_session_health_for_regen_cap() {
    let mut map = MapRuntime::new(0, 0);
    let position = WorldPosition::new(0, -8950.0, -130.0, 83.5, 0.0);
    let mut player = test_player_runtime(7, SessionId(7), position);
    player.class = 1;
    let world_stats = PlayerWorldStats {
        base_health: 58,
        base_mana: 0,
        stats: [23, 20, 22, 20, 70],
        next_level_xp: 400,
    };
    player.base_world_stats = world_stats;
    player.effective_world_stats = world_stats;
    player.spirit = world_stats.stats[4];
    player.health = 60;
    player.max_health = world_stats.max_health();
    map.add_player(player).unwrap();

    let session = WorldSessionState {
        character: CharacterSessionState {
            active_character: Some(ActiveCharacter {
                guid: 7,
                name: "Ada".to_string(),
                race: 1,
                class: 1,
                level: 2,
                xp: 0,
                position,
                movement_flags: 0,
                client_time: 0,
                fall_time: 0,
                jump: JumpInfo::default(),
            }),
            player_health: 98,
            ..CharacterSessionState::default()
        },
        ..WorldSessionState::default()
    };
    map.sync_player_gameplay_state(7, &session);
    assert_eq!(
        map.players.get(&7).unwrap().max_health,
        world_stats.max_health()
    );
    assert_eq!(
        map.players.get(&7).unwrap().health,
        60,
        "alive player health remains map-owned during ordinary session sync"
    );

    map.players.get_mut(&7).unwrap().health = 40;
    let now = Instant::now();
    assert!(map.advance_player_regen_tick(now).unwrap().is_empty());
    map.advance_player_regen_tick(now + Duration::from_secs(2))
        .unwrap();
    assert!(map.players.get(&7).unwrap().health > 40);
    assert!(map.players.get(&7).unwrap().health <= world_stats.max_health());
}

#[test]
fn player_reward_level_up_refreshes_world_stats_for_regen_cap() {
    let mut map = MapRuntime::new(0, 0);
    let position = WorldPosition::new(0, -8950.0, -130.0, 83.5, 0.0);
    let mut player = test_player_runtime(7, SessionId(7), position);
    let old_stats = PlayerWorldStats {
        base_health: 20,
        base_mana: 0,
        stats: [23, 20, 22, 20, 20],
        next_level_xp: 400,
    };
    player.base_world_stats = old_stats;
    player.effective_world_stats = old_stats;
    player.max_health = old_stats.max_health();
    player.health = old_stats.max_health();
    map.add_player(player).unwrap();

    let new_stats = PlayerWorldStats {
        base_health: 80,
        base_mana: 0,
        stats: [24, 21, 24, 20, 40],
        next_level_xp: 900,
    };
    let new_max_health = new_stats.max_health();
    map.update_player_reward_state(
        7,
        PlayerRewardRuntimeUpdate {
            level: 2,
            xp: 0,
            rest_bonus: 0.0,
            player_bytes2: 0,
            health: 40,
            max_health: new_max_health,
            power1: 0,
            max_power1: 0,
            power2: 0,
            world_stats: Some(new_stats),
            combat_stats: Some(test_player_combat_stats()),
            quest_statuses: HashMap::new(),
        },
    );

    assert_eq!(map.players.get(&7).unwrap().max_health, new_max_health);
    assert_eq!(map.players.get(&7).unwrap().base_world_stats, new_stats);

    let now = Instant::now();
    assert!(map.advance_player_regen_tick(now).unwrap().is_empty());
    map.advance_player_regen_tick(now + Duration::from_secs(2))
        .unwrap();
    let snapshot = map.player_runtime_snapshot(7).unwrap();
    assert!(
        snapshot.health > 40,
        "regen should use the newly leveled max-health cap"
    );
    assert!(snapshot.health <= new_max_health);
}

#[test]
fn stale_session_sync_does_not_undo_attack_intent_regen() {
    let mut map = MapRuntime::new(0, 0);
    let position = WorldPosition::new(0, -8950.0, -130.0, 83.5, 0.0);
    let mut player = test_player_runtime(7, SessionId(7), position);
    player.class = 1;
    player.spirit = 40;
    player.health = 40;
    player.max_health = 80;
    let world_stats = PlayerWorldStats {
        base_health: 80,
        base_mana: 0,
        stats: [20, 20, 20, 20, 40],
        next_level_xp: 400,
    };
    player.base_world_stats = world_stats;
    player.effective_world_stats = world_stats;
    player.power2 = 100;
    map.add_player(player).unwrap();

    let target = ObjectGuid::new(HighGuid::Unit, 6, 46);
    map.set_player_auto_attack(7, Some(target), Some(Instant::now()));
    let now = Instant::now();
    assert!(map.advance_player_regen_tick(now).unwrap().is_empty());
    map.advance_player_regen_tick(now + Duration::from_secs(2))
        .unwrap();

    let regened = map.player_runtime_snapshot(7).unwrap();
    assert!(!regened.in_combat);
    assert_eq!(regened.active_combat_target, Some(target));
    assert!(regened.health > 40);
    assert!(regened.power2 < 100);

    let stale_session = WorldSessionState {
        character: CharacterSessionState {
            active_character: Some(ActiveCharacter {
                guid: 7,
                name: "Ada".to_string(),
                race: 1,
                class: 1,
                level: 1,
                xp: 0,
                position,
                movement_flags: 0,
                client_time: 0,
                fall_time: 0,
                jump: JumpInfo::default(),
            }),
            player_health: 40,
            player_rage: 100,
            ..CharacterSessionState::default()
        },
        ..WorldSessionState::default()
    };
    map.sync_player_gameplay_state(7, &stale_session);

    let snapshot = map.player_runtime_snapshot(7).unwrap();
    assert_eq!(snapshot.health, regened.health);
    assert_eq!(snapshot.power2, regened.power2);
}

#[test]
fn rage_gain_from_damage_matches_classic_era_white_hit_and_damage_taken_formulas() {
    assert_eq!(
        rage_gain_from_main_hand_white_damage(0, 1, BASE_ATTACK_TIME_MS, MeleeHitOutcome::Normal),
        0
    );
    assert_eq!(
        rage_gain_from_main_hand_white_damage(3, 1, BASE_ATTACK_TIME_MS, MeleeHitOutcome::Normal),
        49,
        "low starter white hits get the Classic main-hand speed component"
    );
    assert_eq!(
        rage_gain_from_main_hand_white_damage(1, 1, BASE_ATTACK_TIME_MS, MeleeHitOutcome::Normal),
        19,
        "the Classic cap prevents tiny hits from generating more than 15 * damage / conversion"
    );
    assert_eq!(
        rage_gain_from_main_hand_white_damage(100, 1, BASE_ATTACK_TIME_MS, MeleeHitOutcome::Normal),
        534
    );
    assert_eq!(
        rage_gain_from_main_hand_white_damage(100, 1, BASE_ATTACK_TIME_MS, MeleeHitOutcome::Crit),
        569
    );
    assert_eq!(rage_gain_from_damage_taken(100, 1), 333);
}

#[test]
fn map_runtime_player_regen_tick_restores_health_and_mana_from_spirit() {
    let mut map = MapRuntime::new(0, 0);
    let position = WorldPosition::new(0, -8950.0, -130.0, 83.5, 0.0);
    let mut player = test_player_runtime(1, SessionId(1), position);
    player.class = 8; // Mage
    player.spirit = 40;
    player.max_health = 80;
    player.health = 40;
    player.max_power1 = 100;
    player.power1 = 10;
    map.add_player(player).unwrap();
    let now = Instant::now();

    assert!(map.advance_player_regen_tick(now).unwrap().is_empty());
    let packets = map
        .advance_player_regen_tick(now + Duration::from_secs(2))
        .unwrap();

    assert_eq!(packets.len(), 2);
    let runtime = map.players.get(&1).unwrap();
    assert!(runtime.health > 40);
    assert!(runtime.power1 > 10);
}

#[test]
fn map_runtime_mana_regen_obeys_recent_mana_use_interrupt() {
    let mut map = MapRuntime::new(0, 0);
    let position = WorldPosition::new(0, -8950.0, -130.0, 83.5, 0.0);
    let mut player = test_player_runtime(1, SessionId(1), position);
    player.class = 8;
    player.spirit = 80;
    player.max_power1 = 100;
    player.power1 = 10;
    let now = Instant::now();
    player.last_mana_use_at = Some(now);
    map.add_player(player).unwrap();

    assert!(map.advance_player_regen_tick(now).unwrap().is_empty());
    assert!(map
        .advance_player_regen_tick(now + Duration::from_secs(2))
        .unwrap()
        .is_empty());
    assert_eq!(map.players.get(&1).unwrap().power1, 10);

    let packets = map
        .advance_player_regen_tick(now + Duration::from_secs(6))
        .unwrap();
    assert_eq!(packets.len(), 1);
    assert!(map.players.get(&1).unwrap().power1 > 10);
}

#[test]
fn map_runtime_evocation_modifiers_regen_mana_during_interrupt() {
    let mut map = MapRuntime::new(0, 0);
    let position = WorldPosition::new(0, -8950.0, -130.0, 83.5, 0.0);
    let mut player = test_player_runtime(1, SessionId(1), position);
    player.class = 8;
    player.spirit = 80;
    player.max_power1 = 500;
    player.power1 = 10;
    let now = Instant::now();
    player.last_mana_use_at = Some(now);
    player.active_auras.push(ActiveAura {
        spell_id: 12051,
        caster: ObjectGuid::new(HighGuid::Player, 0, 1),
        level: 20,
        interrupt_flags: 0,
        positive: true,
        visible: true,
        duration_millis: Some(8_000),
        expires_at: Some(now + Duration::from_secs(8)),
        periodic_damage: None,
        periodic_regen: None,
        stat_modifiers: vec![
            AuraStatModifier::PowerRegenPercent {
                power_type: POWER_TYPE_MANA,
                percent: 1500,
            },
            AuraStatModifier::ManaRegenInterruptPercent { percent: 100 },
        ],
        proc_triggers: Vec::new(),
    });
    map.add_player(player).unwrap();

    assert!(map.advance_player_regen_tick(now).unwrap().is_empty());
    let packets = map
        .advance_player_regen_tick(now + Duration::from_secs(2))
        .unwrap();

    assert_eq!(packets.len(), 1);
    assert!(
        map.players.get(&1).unwrap().power1 > 10,
        "Evocation's generic aura modifiers should restore mana during the recent-cast window"
    );
}

#[tokio::test]
async fn mage_armor_live_rank_one_regens_mana_during_interrupt_and_updates_arcane_resistance() {
    let world_db_pool =
        MySqlPool::connect_lazy("mysql://mangos:mangos@127.0.0.1:3307/mangos").unwrap();
    let mage_armor = wow_db::get_spell_template_query(&world_db_pool, 6117)
        .await
        .unwrap()
        .expect("Mage Armor rank 1 should exist in the local spell_template");

    let mut map = MapRuntime::new(0, 0);
    let position = WorldPosition::new(0, -8950.0, -130.0, 83.5, 0.0);
    let mut player = test_player_runtime(7, SessionId(7), position);
    player.class = 8;
    let world_stats = PlayerWorldStats {
        base_health: 20,
        base_mana: 500,
        stats: [23, 20, 22, 20, 80],
        next_level_xp: 400,
    };
    player.base_world_stats = world_stats;
    player.effective_world_stats = world_stats;
    player.spirit = world_stats.stats[4];
    player.max_power1 = world_stats.max_mana();
    player.power1 = 10;
    let now = Instant::now();
    player.last_mana_use_at = Some(now);
    map.add_player(player).unwrap();
    let base = map.players.get(&7).unwrap().base_combat_stats;

    let aura = build_active_aura(
        &mage_armor,
        ObjectGuid::new(HighGuid::Player, 0, 7),
        mage_armor.spell_level.try_into().unwrap(),
        test_spell_effect_value_context(&mage_armor),
        now,
        None,
    );
    let event = map.apply_player_aura(7, aura).unwrap().unwrap();
    let player = map.players.get(&7).unwrap();
    assert_eq!(player.combat_stats.resistances[6], base.resistances[6] + 5);
    assert_eq!(player.combat_stats.resistance_buff_mod_positive[6], 5);

    let combat_update = event
        .direct_packets
        .iter()
        .filter(|packet| packet.opcode == WorldOpcode::SmsgUpdateObject as u16)
        .map(|packet| {
            decode_values_update_block(&packet.body[5..], ObjectGuid::new(HighGuid::Player, 0, 7)).0
        })
        .find(|values| values[UNIT_FIELD_RESISTANCES + 6].is_some())
        .expect("Mage Armor should update arcane resistance fields");
    assert_eq!(
        combat_update[UNIT_FIELD_RESISTANCES + 6],
        Some(base.resistances[6] + 5)
    );
    assert_eq!(
        combat_update[PLAYER_FIELD_RESISTANCEBUFFMODSPOSITIVE + 6],
        Some(5)
    );

    assert!(map.advance_player_regen_tick(now).unwrap().is_empty());
    let packets = map
        .advance_player_regen_tick(now + Duration::from_secs(2))
        .unwrap();
    assert_eq!(packets.len(), 2);
    assert_eq!(
        map.players.get(&7).unwrap().power1,
        19,
        "Mage Armor should restore 30% of spirit-based mana regen during the recent-cast window"
    );
}

#[tokio::test]
async fn demon_skin_live_rank_one_regens_health_while_in_combat() {
    let world_db_pool =
        MySqlPool::connect_lazy("mysql://mangos:mangos@127.0.0.1:3307/mangos").unwrap();
    let demon_skin = wow_db::get_spell_template_query(&world_db_pool, 687)
        .await
        .unwrap()
        .expect("Demon Skin rank 1 should exist in the local spell_template");

    let mut map = MapRuntime::new(0, 0);
    let position = WorldPosition::new(0, -8950.0, -130.0, 83.5, 0.0);
    let mut player = test_player_runtime(7, SessionId(7), position);
    player.class = 9;
    player.in_combat = true;
    player.max_health = 40;
    player.health = 20;
    let now = Instant::now();
    player.environment.last_damage_at = Some(now);
    map.add_player(player).unwrap();

    let aura = build_active_aura(
        &demon_skin,
        ObjectGuid::new(HighGuid::Player, 0, 7),
        demon_skin.spell_level.try_into().unwrap(),
        test_spell_effect_value_context(&demon_skin),
        now,
        None,
    );
    let event = map.apply_player_aura(7, aura).unwrap().unwrap();
    assert!(
        event
            .direct_packets
            .iter()
            .any(|packet| { packet.opcode == WorldOpcode::SmsgUpdateObject as u16 }),
        "Demon Skin should still update the player's visible aura/object state"
    );

    assert!(map.advance_player_regen_tick(now).unwrap().is_empty());
    let packets = map
        .advance_player_regen_tick(now + Duration::from_secs(2))
        .unwrap();

    assert_eq!(packets.len(), 1);
    assert_eq!(packets[0].1.opcode, WorldOpcode::SmsgUpdateObject as u16);
    assert_eq!(
        map.players.get(&7).unwrap().health,
        26,
        "Demon Skin should add its flat in-combat health regen through the shared player regen tick"
    );
}

#[tokio::test]
async fn ice_armor_live_rank_one_updates_armor_and_frost_resistance_fields() {
    let world_db_pool =
        MySqlPool::connect_lazy("mysql://mangos:mangos@127.0.0.1:3307/mangos").unwrap();
    let ice_armor = wow_db::get_spell_template_query(&world_db_pool, 7302)
        .await
        .unwrap()
        .expect("Ice Armor rank 1 should exist in the local spell_template");

    let mut map = MapRuntime::new(0, 0);
    let position = WorldPosition::new(0, -8950.0, -130.0, 83.5, 0.0);
    let mut player = test_player_runtime(7, SessionId(7), position);
    player.class = 8;
    map.add_player(player).unwrap();
    let base = map.players.get(&7).unwrap().base_combat_stats;
    let now = Instant::now();

    let aura = build_active_aura(
        &ice_armor,
        ObjectGuid::new(HighGuid::Player, 0, 7),
        ice_armor.spell_level.try_into().unwrap(),
        test_spell_effect_value_context(&ice_armor),
        now,
        None,
    );
    let armor_bonus = aura
        .stat_modifiers
        .iter()
        .find_map(|modifier| match modifier {
            AuraStatModifier::Resistance {
                school_mask: 1,
                amount,
            } => Some(*amount as u32),
            _ => None,
        })
        .expect("Ice Armor rank 1 should expose a physical armor bonus");
    let frost_bonus = aura
        .stat_modifiers
        .iter()
        .find_map(|modifier| match modifier {
            AuraStatModifier::Resistance {
                school_mask,
                amount,
            } if *school_mask == (1 << 4) => Some(*amount as u32),
            _ => None,
        })
        .expect("Ice Armor rank 1 should expose a frost resistance bonus");

    let event = map.apply_player_aura(7, aura).unwrap().unwrap();
    let player = map.players.get(&7).unwrap();
    assert_eq!(
        player.combat_stats.resistances[0],
        base.resistances[0] + armor_bonus
    );
    assert_eq!(
        player.combat_stats.resistances[4],
        base.resistances[4] + frost_bonus
    );
    assert_eq!(player.combat_stats.armor, base.armor + armor_bonus);
    assert_eq!(
        player.combat_stats.resistance_buff_mod_positive[0],
        armor_bonus as i32
    );
    assert_eq!(
        player.combat_stats.resistance_buff_mod_positive[4],
        frost_bonus as i32
    );

    let combat_update = event
        .direct_packets
        .iter()
        .filter(|packet| packet.opcode == WorldOpcode::SmsgUpdateObject as u16)
        .map(|packet| {
            decode_values_update_block(&packet.body[5..], ObjectGuid::new(HighGuid::Player, 0, 7)).0
        })
        .find(|values| values[UNIT_FIELD_RESISTANCES].is_some())
        .expect("Ice Armor should update armor and frost resistance fields");
    assert_eq!(
        combat_update[UNIT_FIELD_RESISTANCES],
        Some(base.resistances[0] + armor_bonus)
    );
    assert_eq!(
        combat_update[UNIT_FIELD_RESISTANCES + 4],
        Some(base.resistances[4] + frost_bonus)
    );
    assert_eq!(
        combat_update[PLAYER_FIELD_RESISTANCEBUFFMODSPOSITIVE],
        Some(armor_bonus)
    );
    assert_eq!(
        combat_update[PLAYER_FIELD_RESISTANCEBUFFMODSPOSITIVE + 4],
        Some(frost_bonus)
    );
}

#[test]
fn map_runtime_player_regen_tick_restores_rogue_energy() {
    let mut map = MapRuntime::new(0, 0);
    let position = WorldPosition::new(0, -8950.0, -130.0, 83.5, 0.0);
    let mut player = test_player_runtime(1, SessionId(1), position);
    player.class = 4;
    player.power4 = 55;
    player.max_power4 = POWER_ENERGY_DEFAULT;
    map.add_player(player).unwrap();
    let now = Instant::now();

    assert!(map.advance_player_regen_tick(now).unwrap().is_empty());
    let packets = map
        .advance_player_regen_tick(now + Duration::from_secs(2))
        .unwrap();

    assert_eq!(map.players.get(&1).unwrap().power4, 75);
    assert_eq!(packets.len(), 1);
    assert_eq!(packets[0].1.opcode, WorldOpcode::SmsgUpdateObject as u16);
}

#[test]
fn map_runtime_player_regen_tick_broadcasts_visible_player_updates() {
    let mut map = MapRuntime::new(0, 0);
    let position = WorldPosition::new(0, -8950.0, -130.0, 83.5, 0.0);
    let mut player = test_player_runtime(1, SessionId(1), position);
    player.class = 8; // Mage
    player.spirit = 40;
    player.max_health = 80;
    player.health = 40;
    player.max_power1 = 100;
    player.power1 = 10;
    map.add_player(player).unwrap();
    map.add_player(test_player_runtime(
        2,
        SessionId(2),
        WorldPosition::new(0, position.x + 4.0, position.y, position.z, 0.0),
    ))
    .unwrap();
    let now = Instant::now();

    assert!(map.advance_player_regen_tick(now).unwrap().is_empty());
    let packets = map
        .advance_player_regen_tick(now + Duration::from_secs(2))
        .unwrap();

    assert_eq!(
        packets
            .iter()
            .filter(|(session_id, _)| *session_id == SessionId(1))
            .count(),
        2
    );
    let observer_packets = packets
        .iter()
        .filter(|(session_id, _)| *session_id == SessionId(2))
        .map(|(_, packet)| packet)
        .collect::<Vec<_>>();
    assert_eq!(observer_packets.len(), 2);
    let player_guid = ObjectGuid::new(HighGuid::Player, 0, 1);
    let observer_values = observer_packets
        .iter()
        .map(|packet| decode_values_update_block(&packet.body[5..], player_guid).0)
        .collect::<Vec<_>>();
    assert!(observer_values
        .iter()
        .any(|values| values[UNIT_FIELD_HEALTH].is_some()));
    assert!(observer_values
        .iter()
        .any(|values| values[UNIT_FIELD_POWER1].is_some()));
}

#[test]
fn map_runtime_player_regen_tick_degenerates_warrior_rage_out_of_combat() {
    let mut map = MapRuntime::new(0, 0);
    let position = WorldPosition::new(0, -8950.0, -130.0, 83.5, 0.0);
    let mut player = test_player_runtime(1, SessionId(1), position);
    player.class = 1; // Warrior
    player.power2 = 100;
    map.add_player(player).unwrap();
    let now = Instant::now();

    assert!(map.advance_player_regen_tick(now).unwrap().is_empty());
    let packets = map
        .advance_player_regen_tick(now + Duration::from_secs(2))
        .unwrap();

    assert_eq!(packets.len(), 1);
    assert_eq!(map.players.get(&1).unwrap().power2, 75);
}

#[test]
fn map_runtime_player_regen_tick_skips_dead_or_ghost_players() {
    let mut map = MapRuntime::new(0, 0);
    let position = WorldPosition::new(0, -8950.0, -130.0, 83.5, 0.0);
    let mut dead = test_player_runtime(1, SessionId(1), position);
    dead.health = 0;
    dead.max_power1 = 100;
    dead.power1 = 1;
    dead.power2 = 100;
    map.add_player(dead).unwrap();
    let mut ghost = test_player_runtime(2, SessionId(2), position);
    ghost.flags = PLAYER_FLAGS_GHOST;
    ghost.health = 1;
    ghost.max_power1 = 100;
    ghost.power1 = 1;
    ghost.power2 = 100;
    map.add_player(ghost).unwrap();
    let now = Instant::now();

    assert!(map.advance_player_regen_tick(now).unwrap().is_empty());
    let packets = map
        .advance_player_regen_tick(now + Duration::from_secs(2))
        .unwrap();

    assert!(packets.is_empty());
    assert_eq!(map.players.get(&1).unwrap().power1, 1);
    assert_eq!(map.players.get(&2).unwrap().power2, 100);
}

#[test]
fn player_selection_update_body_sets_unit_target_guid() {
    let player = ObjectGuid::new(HighGuid::Player, 0, 7);
    let selected = ObjectGuid::new(HighGuid::Unit, 0, 99);
    let body = build_player_selection_update_body(7, Some(selected)).unwrap();
    let (values, trailing) = decode_values_update_block(&body[5..], player);
    assert!(trailing.is_empty());
    assert_eq!(values[UNIT_FIELD_TARGET], Some(selected.raw() as u32));
    assert_eq!(
        values[UNIT_FIELD_TARGET + 1],
        Some((selected.raw() >> 32) as u32)
    );
}

#[test]
fn player_selection_update_body_clears_unit_target_guid() {
    let player = ObjectGuid::new(HighGuid::Player, 0, 7);
    let body = build_player_selection_update_body(7, None).unwrap();
    let (values, trailing) = decode_values_update_block(&body[5..], player);
    assert!(trailing.is_empty());
    assert_eq!(values[UNIT_FIELD_TARGET], Some(0));
    assert_eq!(values[UNIT_FIELD_TARGET + 1], Some(0));
}

#[test]
fn bind_sight_aura_maps_to_farsight_modifier_and_support() {
    let mut template = test_spell_template(2585);
    template.effect1 = SPELL_EFFECT_APPLY_AURA;
    template.effect_apply_aura_name1 = SPELL_AURA_BIND_SIGHT;

    let modifiers = spell_aura_stat_modifiers(
        &SpellInfo::from_template(&template),
        test_spell_effect_value_context(&template),
    );

    assert_eq!(modifiers, vec![AuraStatModifier::FarSight]);
    assert_eq!(
        spell_aura_support(SPELL_AURA_BIND_SIGHT),
        SpellMechanicSupport::Implemented
    );
    assert_eq!(
        spell_aura_support(SPELL_AURA_FAR_SIGHT),
        SpellMechanicSupport::Implemented
    );
}

#[test]
fn player_farsight_update_body_sets_private_target_guid() {
    let player = ObjectGuid::new(HighGuid::Player, 0, 7);
    let farsight_target = ObjectGuid::new(HighGuid::Unit, 0, 99);
    let body = build_player_farsight_update_body(7, Some(farsight_target)).unwrap();
    let (values, trailing) = decode_values_update_block(&body[5..], player);
    assert!(trailing.is_empty());
    assert_eq!(values[PLAYER_FARSIGHT], Some(farsight_target.raw() as u32));
    assert_eq!(
        values[PLAYER_FARSIGHT + 1],
        Some((farsight_target.raw() >> 32) as u32)
    );
}

#[test]
fn db_creature_bind_sight_aura_sets_and_clears_player_farsight_target() {
    let now = Instant::now();
    let player = ObjectGuid::new(HighGuid::Player, 0, 7);
    let mut map = MapRuntime::new(0, 0);
    map.add_player(test_player_runtime(
        7,
        SessionId(7),
        WorldPosition::new(0, 0.0, 0.0, 0.0, 0.0),
    ))
    .unwrap();
    let mut spawn = test_creature_spawn(4277);
    spawn.guid = 900_4277;
    let creature_guid = creature_spawn_guid(&spawn);
    map.creatures
        .insert(creature_guid.raw(), DbCreatureRuntime::new(spawn));
    let aura = ActiveAura {
        spell_id: 2585,
        caster: player,
        level: 24,
        interrupt_flags: 0,
        positive: true,
        visible: false,
        duration_millis: Some(60_000),
        expires_at: Some(now + Duration::from_secs(60)),
        periodic_damage: None,
        periodic_regen: None,
        stat_modifiers: vec![AuraStatModifier::FarSight],
        proc_triggers: Vec::new(),
    };

    let applied = map
        .apply_db_creature_aura_replacing_spell_ids(creature_guid, 7, aura, &[], None, None, now)
        .unwrap()
        .unwrap();
    assert_eq!(
        map.players.get(&7).unwrap().farsight_target,
        Some(creature_guid)
    );
    let (_, applied_packet) = applied
        .observer_packets
        .iter()
        .find(|(session_id, packet)| {
            *session_id == SessionId(7) && packet.opcode == WorldOpcode::SmsgUpdateObject as u16
        })
        .cloned()
        .expect("caster should receive farsight update");
    let (applied_values, trailing) = decode_values_update_block(&applied_packet.body[5..], player);
    assert!(trailing.is_empty());
    assert_eq!(
        applied_values[PLAYER_FARSIGHT],
        Some(creature_guid.raw() as u32)
    );

    let removed = map
        .remove_db_creature_auras_by_spell_ids(creature_guid, 99, &[2585], now)
        .unwrap()
        .unwrap();
    assert_eq!(map.players.get(&7).unwrap().farsight_target, None);
    let (_, removed_packet) = removed
        .aura_update
        .observer_packets
        .iter()
        .find(|(session_id, packet)| {
            *session_id == SessionId(7) && packet.opcode == WorldOpcode::SmsgUpdateObject as u16
        })
        .cloned()
        .expect("caster should receive farsight clear");
    let (removed_values, trailing) = decode_values_update_block(&removed_packet.body[5..], player);
    assert!(trailing.is_empty());
    assert_eq!(removed_values[PLAYER_FARSIGHT], Some(0));
    assert_eq!(removed_values[PLAYER_FARSIGHT + 1], Some(0));
}

#[test]
fn player_farsight_refresh_only_changes_owner_player_visibility() {
    let mut map = MapRuntime::new(0, 0);
    let owner_position = WorldPosition::new(0, -8950.0, -130.0, 83.5, 0.0);
    let eye_position = WorldPosition::new(0, -8780.0, -130.0, 83.5, 0.0);
    let body_observer_position = WorldPosition::new(0, -8945.0, -130.0, 83.5, 0.0);
    let eye_observer_position = WorldPosition::new(0, -8775.0, -130.0, 83.5, 0.0);

    map.add_player(test_player_runtime(1, SessionId(1), owner_position))
        .unwrap();
    map.add_player(test_player_runtime(2, SessionId(2), body_observer_position))
        .unwrap();
    map.add_player(test_player_runtime(3, SessionId(3), eye_observer_position))
        .unwrap();

    let owner_guid = ObjectGuid::new(HighGuid::Player, 0, 1);
    let body_observer_guid = ObjectGuid::new(HighGuid::Player, 0, 2);
    let eye_observer_guid = ObjectGuid::new(HighGuid::Player, 0, 3);
    assert!(map
        .players
        .get(&1)
        .unwrap()
        .visible_objects
        .contains(&body_observer_guid));
    assert!(!map
        .players
        .get(&1)
        .unwrap()
        .visible_objects
        .contains(&eye_observer_guid));
    assert!(map
        .players
        .get(&2)
        .unwrap()
        .visible_objects
        .contains(&owner_guid));
    assert!(!map
        .players
        .get(&3)
        .unwrap()
        .visible_objects
        .contains(&owner_guid));

    let mut eye_spawn = test_creature_spawn(4277);
    eye_spawn.guid = 900_4277;
    eye_spawn.position_x = eye_position.x;
    eye_spawn.position_y = eye_position.y;
    eye_spawn.position_z = eye_position.z;
    eye_spawn.orientation = eye_position.orientation;
    let eye_runtime = DbCreatureRuntime::new(eye_spawn);
    let eye_guid = eye_runtime.guid();
    map.insert_loaded_creature_grid(grid_coord_for_position(eye_position), vec![eye_runtime]);

    let farsight_packets = map.update_player_farsight(1, Some(eye_guid)).unwrap();
    assert!(farsight_packets
        .iter()
        .all(|(session, _)| *session == SessionId(1)));
    assert!(
        farsight_packets.iter().any(|(_, packet)| {
            packet.opcode == WorldOpcode::SmsgDestroyObject as u16
                && packet.body == body_observer_guid.raw().to_le_bytes()
        }),
        "owner should lose nearby body-only players when the camera swaps to the eye"
    );
    assert!(
        farsight_packets.iter().any(|(_, packet)| {
            packet.opcode == WorldOpcode::SmsgUpdateObject as u16
                && packet
                    .body
                    .windows(eye_observer_guid.raw().to_le_bytes().len())
                    .any(|window| window == eye_observer_guid.raw().to_le_bytes())
        }),
        "owner should gain players near the eye viewpoint"
    );
    assert!(!map
        .players
        .get(&1)
        .unwrap()
        .visible_objects
        .contains(&body_observer_guid));
    assert!(map
        .players
        .get(&1)
        .unwrap()
        .visible_objects
        .contains(&eye_observer_guid));
    assert!(
        map.players
            .get(&2)
            .unwrap()
            .visible_objects
            .contains(&owner_guid),
        "other players should still see the owner's real body"
    );
    assert!(
        !map.players
            .get(&3)
            .unwrap()
            .visible_objects
            .contains(&owner_guid),
        "swapping the camera should not make distant players see the owner body"
    );

    let clear_packets = map.update_player_farsight(1, None).unwrap();
    assert!(clear_packets
        .iter()
        .all(|(session, _)| *session == SessionId(1)));
    assert!(map
        .players
        .get(&1)
        .unwrap()
        .visible_objects
        .contains(&body_observer_guid));
    assert!(!map
        .players
        .get(&1)
        .unwrap()
        .visible_objects
        .contains(&eye_observer_guid));
}

#[test]
fn map_runtime_player_selection_update_refreshes_shared_state_and_observers() {
    let mut map = MapRuntime::new(0, 0);
    let player_position = WorldPosition::new(0, -8950.0, -130.0, 83.5, 0.0);
    let observer_position = WorldPosition::new(0, -8952.0, -130.0, 83.5, 0.0);
    map.add_player(test_player_runtime(1, SessionId(1), player_position))
        .unwrap();
    map.add_player(test_player_runtime(2, SessionId(2), observer_position))
        .unwrap();

    let selected = ObjectGuid::new(HighGuid::Unit, 0, 77);
    let packets = map.update_player_selection(1, Some(selected)).unwrap();

    assert_eq!(packets.len(), 1);
    assert_eq!(packets[0].0, SessionId(2));
    assert_eq!(packets[0].1.opcode, WorldOpcode::SmsgUpdateObject as u16);
    let (values, trailing) = decode_values_update_block(
        &packets[0].1.body[5..],
        ObjectGuid::new(HighGuid::Player, 0, 1),
    );
    assert!(trailing.is_empty());
    assert_eq!(values[UNIT_FIELD_TARGET], Some(selected.raw() as u32));
    assert_eq!(
        values[UNIT_FIELD_TARGET + 1],
        Some((selected.raw() >> 32) as u32)
    );
    assert_eq!(map.players.get(&1).unwrap().selected_target, Some(selected));
    assert_eq!(map.players.get(&1).unwrap().unit_target, Some(selected));
}

#[test]
fn map_runtime_player_target_obsolete_update_does_not_change_logical_selection() {
    let mut map = MapRuntime::new(0, 0);
    let player_position = WorldPosition::new(0, -8950.0, -130.0, 83.5, 0.0);
    let observer_position = WorldPosition::new(0, -8952.0, -130.0, 83.5, 0.0);
    let selected = ObjectGuid::new(HighGuid::Unit, 0, 77);
    let public_target = ObjectGuid::new(HighGuid::Unit, 0, 88);
    let mut player = test_player_runtime(1, SessionId(1), player_position);
    player.selected_target = Some(selected);
    player.unit_target = Some(selected);
    map.add_player(player).unwrap();
    map.add_player(test_player_runtime(2, SessionId(2), observer_position))
        .unwrap();

    let packets = map.update_player_target(1, Some(public_target)).unwrap();

    assert_eq!(packets.len(), 1);
    assert_eq!(packets[0].0, SessionId(2));
    assert_eq!(packets[0].1.opcode, WorldOpcode::SmsgUpdateObject as u16);
    let (values, trailing) = decode_values_update_block(
        &packets[0].1.body[5..],
        ObjectGuid::new(HighGuid::Player, 0, 1),
    );
    assert!(trailing.is_empty());
    assert_eq!(values[UNIT_FIELD_TARGET], Some(public_target.raw() as u32));
    assert_eq!(
        values[UNIT_FIELD_TARGET + 1],
        Some((public_target.raw() >> 32) as u32)
    );
    assert_eq!(map.players.get(&1).unwrap().selected_target, Some(selected));
    assert_eq!(
        map.players.get(&1).unwrap().unit_target,
        Some(public_target)
    );

    map.update_player_target(1, None).unwrap();
    assert_eq!(map.players.get(&1).unwrap().selected_target, Some(selected));
    assert_eq!(map.players.get(&1).unwrap().unit_target, None);
}

#[test]
fn map_runtime_player_stand_state_update_refreshes_observers_and_late_visibility() {
    let mut map = MapRuntime::new(0, 0);
    let player_position = WorldPosition::new(0, -8950.0, -130.0, 83.5, 0.0);
    let observer_position = WorldPosition::new(0, -8952.0, -130.0, 83.5, 0.0);
    map.add_player(test_player_runtime(1, SessionId(1), player_position))
        .unwrap();
    map.add_player(test_player_runtime(2, SessionId(2), observer_position))
        .unwrap();

    let packets = map
        .set_player_stand_state(1, PLAYER_STAND_STATE_SLEEP)
        .unwrap();

    let packet = packets
        .iter()
        .find(|(session, packet)| {
            *session == SessionId(2) && packet.opcode == WorldOpcode::SmsgUpdateObject as u16
        })
        .expect("observer should receive player stand-state update");
    let player_guid = ObjectGuid::new(HighGuid::Player, 0, 1);
    let (values, trailing) = decode_values_update_block(&packet.1.body[5..], player_guid);
    assert!(trailing.is_empty());
    assert_eq!(
        values[UNIT_FIELD_BYTES_1],
        Some(unit_bytes_1_for_class(1) | u32::from(PLAYER_STAND_STATE_SLEEP))
    );
    assert_eq!(
        map.players.get(&1).unwrap().stand_state,
        PLAYER_STAND_STATE_SLEEP
    );

    let values = decode_other_player_create_values(
        &build_other_player_create_block(&map.players[&1]).unwrap(),
        player_guid,
    );
    assert_eq!(
        values[UNIT_FIELD_BYTES_1],
        Some(unit_bytes_1_for_class(1) | u32::from(PLAYER_STAND_STATE_SLEEP))
    );
}

#[tokio::test]
async fn shared_creature_combat_start_broadcasts_to_nearby_observer() {
    let maps = Arc::new(MapRuntimeManager::default());
    let sessions = Arc::new(SessionRegistry::default());
    let object_mgr = ObjectMgr::default();
    let shared_world = SharedWorldDeps {
        object_mgr: &object_mgr,
        maps: &maps,
        sessions: &sessions,
    };
    let victim_position = WorldPosition::new(0, -8950.0, -130.0, 83.5, 0.0);
    let observer_position = WorldPosition::new(0, -8952.0, -130.0, 83.5, 0.0);
    let victim_session_id = SessionId(1);
    let observer_session_id = SessionId(2);
    let (victim_tx, mut victim_rx) = mpsc::unbounded_channel();
    let (observer_tx, mut observer_rx) = mpsc::unbounded_channel();
    let mut sink = WorldPacketSink::new(victim_tx);
    sessions
        .register(
            observer_session_id,
            SessionHandle {
                account_id: 2,
                character_guid: Some(2),
                character_name: Some("Player2".to_string()),
                outbound: WorldPacketSender::Unbounded(observer_tx),
                disconnect: None,
            },
        )
        .await;
    maps.add_player(test_player_runtime(1, victim_session_id, victim_position))
        .await
        .unwrap();
    maps.add_player(test_player_runtime(
        2,
        observer_session_id,
        observer_position,
    ))
    .await
    .unwrap();

    let mut spawn = test_creature_spawn(6);
    spawn.guid = 177;
    spawn.position_x = victim_position.x + 12.0;
    spawn.position_y = victim_position.y;
    spawn.position_z = victim_position.z;
    let attacker = creature_spawn_guid(&spawn);
    let creature = DbCreatureRuntime::new(spawn);
    maps.share_db_creature_snapshots(0, vec![creature.clone()])
        .await;
    let mut session = WorldSessionState {
        character: CharacterSessionState {
            active_character: Some(ActiveCharacter {
                guid: 1,
                name: "Ada".to_string(),
                race: 1,
                class: 1,
                level: 1,
                xp: 0,
                position: victim_position,
                movement_flags: 0,
                client_time: 0,
                fall_time: 0,
                jump: JumpInfo::default(),
            }),
            player_health: 20,
            ..CharacterSessionState::default()
        },
        death: DeathSessionState {
            player_death_state: PlayerDeathState::Alive,
            ..DeathSessionState::default()
        },
        ..WorldSessionState::default()
    };
    session
        .visibility
        .db_creatures
        .insert(attacker.raw(), creature);
    let player = ObjectGuid::new(HighGuid::Player, 0, 1);
    let mut header_crypto = HeaderCrypto::new(&[0; 40]);

    send_db_creature_combat_start(
        &mut sink,
        shared_world,
        0,
        &mut session,
        attacker,
        player,
        &mut header_crypto,
    )
    .await
    .unwrap();

    let victim_packets = std::iter::from_fn(|| victim_rx.try_recv().ok()).collect::<Vec<_>>();
    let observer_packets = std::iter::from_fn(|| observer_rx.try_recv().ok()).collect::<Vec<_>>();

    assert!(victim_packets
        .iter()
        .any(|packet| packet.opcode == WorldOpcode::SmsgAttackStart as u16));
    assert!(observer_packets
        .iter()
        .any(|packet| packet.opcode == WorldOpcode::SmsgAttackStart as u16));
    assert!(observer_packets
        .iter()
        .any(|packet| packet.opcode == WorldOpcode::SmsgUpdateObject as u16));
    assert!(observer_packets
        .iter()
        .any(|packet| packet.opcode == WorldOpcode::SmsgMonsterMove as u16));
}

#[test]
fn player_death_evades_active_db_creature_and_starts_return_home() {
    let mut map = MapRuntime::new(0, 0);
    let home = WorldPosition::new(0, -8950.0, -130.0, 83.5, 0.0);
    let death_position = WorldPosition::new(0, -8940.0, -130.0, 83.5, 0.0);
    let observer_position = WorldPosition::new(0, -8941.0, -130.0, 83.5, 0.0);
    let victim_session_id = SessionId(1);
    let observer_session_id = SessionId(2);
    map.add_player(test_player_runtime(1, victim_session_id, death_position))
        .unwrap();
    map.add_player(test_player_runtime(
        2,
        observer_session_id,
        observer_position,
    ))
    .unwrap();

    let mut spawn = test_creature_spawn(6);
    spawn.guid = 197;
    spawn.position_x = home.x;
    spawn.position_y = home.y;
    spawn.position_z = home.z;
    let attacker = creature_spawn_guid(&spawn);
    let mut creature = DbCreatureRuntime::new(spawn);
    creature.current_position = death_position;
    creature.health = 1;
    creature.lootable = true;
    let max_health = creature.max_health();
    map.creatures.insert(attacker.raw(), creature);
    let player = ObjectGuid::new(HighGuid::Player, 0, 1);
    let now = Instant::now();
    map.begin_db_creature_combat(attacker, player, now)
        .expect("combat should start");
    let death = map
        .apply_player_world_damage(
            player,
            Some(attacker),
            999,
            WorldDamageKind::SpellDirect,
            now,
        )
        .unwrap()
        .expect("damage should apply");
    let snapshot = map.creatures.get(&attacker.raw()).unwrap();

    assert!(map.active_db_creature_combats_for_victim(player).is_empty());
    assert_eq!(snapshot.health, max_health);
    assert!(!snapshot.lootable);
    assert!(matches!(
        snapshot.motion,
        CreatureMotionState::ReturnHome(CreatureReturnHomeMotion { destination, .. })
            if distance_2d(destination.x, destination.y, home.x, home.y) <= f32::EPSILON
    ));
    assert!(death
        .direct_packets
        .iter()
        .any(|packet| packet.opcode == WorldOpcode::SmsgAttackStop as u16));
    assert!(death
        .direct_packets
        .iter()
        .any(|packet| packet.opcode == WorldOpcode::SmsgMonsterMove as u16));
    assert!(death
        .observer_packets
        .iter()
        .any(|(_, packet)| packet.opcode == WorldOpcode::SmsgAttackStop as u16));
    assert!(death
        .observer_packets
        .iter()
        .any(|(_, packet)| packet.opcode == WorldOpcode::SmsgMonsterMove as u16));
}

#[test]
fn map_runtime_player_world_damage_makes_zero_health_corpse_state_authoritative() {
    let mut map = MapRuntime::new(0, 0);
    let player_guid = ObjectGuid::new(HighGuid::Player, 0, 7);
    let position = WorldPosition::new(0, 0.0, 0.0, 0.0, 0.0);
    map.add_player(test_player_runtime(7, SessionId::next(), position))
        .unwrap();
    let jump = JumpInfo {
        z_speed: 7.0,
        cos_angle: 0.25,
        sin_angle: 0.75,
        xy_speed: 4.5,
    };
    {
        let player = map.players.get_mut(&7).unwrap();
        player.movement_flags = MOVEFLAG_JUMPING;
        player.fall_time = 456;
        player.jump = jump.clone();
    }

    let death = map
        .apply_player_world_damage(
            player_guid,
            Some(ObjectGuid::new(HighGuid::Unit, 0, 3196)),
            999,
            WorldDamageKind::SpellDirect,
            Instant::now(),
        )
        .unwrap()
        .expect("damage should apply");

    assert!(death.died);
    assert_eq!(death.remaining_health, 0);
    assert!(death.death_presentation_deferred);
    assert!(
        !death
            .direct_packets
            .iter()
            .any(|packet| packet.opcode == WorldOpcode::SmsgForceMoveRoot as u16),
        "airborne death must not root the client until landing"
    );
    let mut packed = Vec::new();
    PackedGuid::write(&mut packed, player_guid).unwrap();
    let values_start = 4 + 1 + 1 + packed.len();
    let values = decode_update_values(&death.health_packet.body[values_start..]);
    assert_eq!(values[UNIT_FIELD_HEALTH], Some(0));
    assert_eq!(values[UNIT_FIELD_BYTES_1], None);
    assert_eq!(values[PLAYER_FIELD_BYTES], None);
    let snapshot = map.player_runtime_snapshot(7).unwrap();
    assert_eq!(snapshot.health, 0);
    assert_eq!(snapshot.death_state, PlayerDeathState::JustDied);
    assert_eq!(snapshot.stand_state, PLAYER_STAND_STATE_STAND);
    let player = map.players.get(&7).unwrap();
    assert_eq!(player.movement_flags, MOVEFLAG_JUMPING);
    assert_eq!(player.fall_time, 456);
    assert_eq!(player.jump, jump);

    assert!(map
        .present_player_death_if_ready(7, Instant::now(), false)
        .unwrap()
        .is_empty());
    {
        let player = map.players.get_mut(&7).unwrap();
        player.movement_flags = 0;
        player.fall_time = 0;
    }
    let presentation = map
        .present_player_death_if_ready(7, Instant::now(), false)
        .unwrap();
    assert!(presentation
        .iter()
        .any(|(_, packet)| packet.opcode == WorldOpcode::SmsgForceMoveRoot as u16));
    let snapshot = map.player_runtime_snapshot(7).unwrap();
    assert_eq!(snapshot.death_state, PlayerDeathState::Corpse);
    assert_eq!(snapshot.stand_state, PLAYER_STAND_STATE_DEAD);

    let second = map
        .apply_player_world_damage(
            player_guid,
            Some(ObjectGuid::new(HighGuid::Unit, 0, 3196)),
            1,
            WorldDamageKind::PeriodicAura,
            Instant::now(),
        )
        .unwrap();
    assert!(
        second.is_none(),
        "dead players must not accept a second damage/death transition"
    );
}

#[test]
fn map_runtime_airborne_death_presentation_fallback_forces_after_timeout() {
    let mut map = MapRuntime::new(0, 0);
    let player_guid = ObjectGuid::new(HighGuid::Player, 0, 7);
    let position = WorldPosition::new(0, 11.0, 22.0, 33.0, 0.0);
    map.add_player(test_player_runtime(7, SessionId::next(), position))
        .unwrap();
    {
        let player = map.players.get_mut(&7).unwrap();
        player.movement_flags = MOVEFLAG_JUMPING;
        player.fall_time = 456;
    }
    let killed_at = Instant::now();

    let death = map
        .apply_player_world_damage(
            player_guid,
            Some(ObjectGuid::new(HighGuid::Unit, 0, 3196)),
            999,
            WorldDamageKind::SpellDirect,
            killed_at,
        )
        .unwrap()
        .expect("damage should apply");

    assert!(death.died);
    assert!(death.death_presentation_deferred);
    assert!(map
        .advance_player_death_presentations(killed_at + Duration::from_millis(2_999))
        .unwrap()
        .is_empty());
    assert_eq!(
        map.player_runtime_snapshot(7).unwrap().death_state,
        PlayerDeathState::JustDied
    );

    let presentation = map
        .advance_player_death_presentations(killed_at + Duration::from_secs(3))
        .unwrap();

    assert!(presentation
        .iter()
        .any(|(_, packet)| packet.opcode == WorldOpcode::SmsgForceMoveRoot as u16));
    let snapshot = map.player_runtime_snapshot(7).unwrap();
    assert_eq!(snapshot.death_state, PlayerDeathState::Corpse);
    assert_eq!(snapshot.stand_state, PLAYER_STAND_STATE_DEAD);
    assert_eq!(snapshot.movement_flags & MOVEFLAG_JUMPING, 0);
    assert_eq!(snapshot.fall_time, 0);
    assert_eq!(snapshot.position, position);
    assert!(map
        .advance_player_death_presentations(killed_at + Duration::from_secs(4))
        .unwrap()
        .is_empty());
}

#[tokio::test]
async fn repop_refreshes_session_after_map_presents_delayed_death() {
    let maps = Arc::new(MapRuntimeManager::default());
    let position = WorldPosition::new(0, -8950.0, -130.0, 83.5, 0.0);
    let mut player = test_player_runtime(7, SessionId(7), position);
    player.health = 0;
    player.death_state = PlayerDeathState::Corpse;
    player.stand_state = PLAYER_STAND_STATE_DEAD;
    maps.add_player(player).await.unwrap();

    let mut session = WorldSessionState {
        character: CharacterSessionState {
            active_character: Some(ActiveCharacter {
                guid: 7,
                name: "Ada".to_string(),
                race: 1,
                class: 1,
                level: 1,
                xp: 0,
                position,
                movement_flags: MOVEFLAG_JUMPING,
                client_time: 123,
                fall_time: 456,
                jump: JumpInfo::default(),
            }),
            player_health: 0,
            player_stand_state: PLAYER_STAND_STATE_STAND,
            ..CharacterSessionState::default()
        },
        death: DeathSessionState {
            player_death_state: PlayerDeathState::JustDied,
            player_death_presentation_pending: true,
            ..DeathSessionState::default()
        },
        ..WorldSessionState::default()
    };

    let packets = refresh_session_death_state_before_repop(&maps, &mut session)
        .await
        .unwrap();

    assert!(packets.is_empty());
    assert_eq!(session.death.player_death_state, PlayerDeathState::Corpse);
    assert!(!session.death.player_death_presentation_pending);
    assert_eq!(
        session.character.player_stand_state,
        PLAYER_STAND_STATE_DEAD
    );
    assert_eq!(session.character.player_health, 0);
    let character = session.character.active_character.as_ref().unwrap();
    assert_eq!(character.position, position);
    assert_eq!(character.movement_flags, 0);
    assert_eq!(character.fall_time, 0);
}

#[tokio::test]
async fn repop_forces_pending_just_died_presentation_before_refresh() {
    let maps = Arc::new(MapRuntimeManager::default());
    let position = WorldPosition::new(0, -8950.0, -130.0, 83.5, 0.0);
    let mut player = test_player_runtime(7, SessionId(7), position);
    player.health = 0;
    player.death_state = PlayerDeathState::JustDied;
    player.movement_flags = MOVEFLAG_JUMPING;
    player.fall_time = 456;
    player.last_fall_z = Some(90.0);
    maps.add_player(player).await.unwrap();

    let mut session = WorldSessionState {
        character: CharacterSessionState {
            active_character: Some(ActiveCharacter {
                guid: 7,
                name: "Ada".to_string(),
                race: 1,
                class: 1,
                level: 1,
                xp: 0,
                position,
                movement_flags: MOVEFLAG_JUMPING,
                client_time: 123,
                fall_time: 456,
                jump: JumpInfo::default(),
            }),
            player_health: 0,
            player_stand_state: PLAYER_STAND_STATE_STAND,
            ..CharacterSessionState::default()
        },
        death: DeathSessionState {
            player_death_state: PlayerDeathState::JustDied,
            player_death_presentation_pending: true,
            ..DeathSessionState::default()
        },
        ..WorldSessionState::default()
    };

    let packets = refresh_session_death_state_before_repop(&maps, &mut session)
        .await
        .unwrap();

    assert!(packets
        .iter()
        .any(|(_, packet)| packet.opcode == WorldOpcode::SmsgForceMoveRoot as u16));
    assert_eq!(session.death.player_death_state, PlayerDeathState::Corpse);
    assert!(!session.death.player_death_presentation_pending);
    assert_eq!(
        session.character.player_stand_state,
        PLAYER_STAND_STATE_DEAD
    );
    assert_eq!(session.character.player_health, 0);
    let snapshot = maps.player_runtime_snapshot(0, 7).await.unwrap();
    assert_eq!(snapshot.death_state, PlayerDeathState::Corpse);
    assert_eq!(snapshot.movement_flags & MOVEFLAG_JUMPING, 0);
    assert_eq!(snapshot.fall_time, 0);
}

#[test]
fn map_runtime_dot_death_landing_presents_even_with_nonzero_fall_time() {
    let mut map = MapRuntime::new(0, 0);
    let player_guid = ObjectGuid::new(HighGuid::Player, 0, 7);
    let start = WorldPosition::new(0, 11.0, 22.0, 33.0, 0.0);
    map.add_player(test_player_runtime(7, SessionId::next(), start))
        .unwrap();
    {
        let player = map.players.get_mut(&7).unwrap();
        player.movement_flags = MOVEFLAG_JUMPING;
        player.fall_time = 456;
        player.last_fall_z = Some(40.0);
    }
    let killed_at = Instant::now();

    let death = map
        .apply_player_world_damage(
            player_guid,
            Some(ObjectGuid::new(HighGuid::Unit, 0, 3196)),
            999,
            WorldDamageKind::PeriodicAura,
            killed_at,
        )
        .unwrap()
        .expect("damage should apply");

    assert!(death.died);
    assert!(death.death_presentation_deferred);
    assert_eq!(
        map.player_runtime_snapshot(7).unwrap().death_state,
        PlayerDeathState::JustDied
    );

    let landing = MovementInfo {
        flags: 0,
        client_time: 2,
        position: WorldPosition::new(0, 11.0, 22.0, 30.0, 0.0),
        fall_time: 900,
        jump: JumpInfo::default(),
    };
    let packets = map
        .update_player_position(7, WorldOpcode::MsgMoveFallLand as u16, &landing, 2)
        .unwrap();

    assert!(packets
        .iter()
        .any(|(_, packet)| packet.opcode == WorldOpcode::SmsgForceMoveRoot as u16));
    assert!(packets.iter().any(|(_, packet)| {
        if packet.opcode != WorldOpcode::SmsgUpdateObject as u16 {
            return false;
        }
        let player = ObjectGuid::new(HighGuid::Player, 0, 7);
        let (values, _) = decode_values_update_block(&packet.body[5..], player);
        values[UNIT_FIELD_HEALTH] == Some(0)
            && values[UNIT_FIELD_BYTES_1].is_some_and(|bytes| {
                bytes & u32::from(PLAYER_STAND_STATE_DEAD) == u32::from(PLAYER_STAND_STATE_DEAD)
            })
    }));
    let snapshot = map.player_runtime_snapshot(7).unwrap();
    assert_eq!(snapshot.death_state, PlayerDeathState::Corpse);
    assert_eq!(snapshot.stand_state, PLAYER_STAND_STATE_DEAD);
    assert_eq!(snapshot.fall_time, 0);
    assert_eq!(snapshot.movement_flags & MOVEFLAG_JUMPING, 0);
    assert!(map
        .advance_player_death_presentations(killed_at + Duration::from_secs(3))
        .unwrap()
        .is_empty());
}

#[test]
fn grounded_movement_clears_stale_fall_tracking_before_dot_death() {
    let mut map = MapRuntime::new(0, 0);
    let player_guid = ObjectGuid::new(HighGuid::Player, 0, 7);
    let start = WorldPosition::new(0, 11.0, 22.0, 33.0, 0.0);
    map.add_player(test_player_runtime(7, SessionId::next(), start))
        .unwrap();
    {
        let player = map.players.get_mut(&7).unwrap();
        player.fall_time = 900;
        player.last_fall_z = Some(40.0);
        player.last_fall_time = 900;
    }

    let grounded = MovementInfo {
        flags: MOVEFLAG_FORWARD,
        client_time: 2,
        position: WorldPosition::new(0, 12.0, 22.0, 32.5, 0.0),
        fall_time: 900,
        jump: JumpInfo::default(),
    };
    map.update_player_position(7, WorldOpcode::MsgMoveHeartbeat as u16, &grounded, 2)
        .unwrap();
    {
        let player = map.players.get(&7).unwrap();
        assert_eq!(player.fall_time, 0);
        assert_eq!(player.last_fall_z, None);
        assert_eq!(player.last_fall_time, 0);
    }

    let death = map
        .apply_player_world_damage(
            player_guid,
            Some(ObjectGuid::new(HighGuid::Unit, 0, 3196)),
            999,
            WorldDamageKind::PeriodicAura,
            Instant::now(),
        )
        .unwrap()
        .expect("damage should apply");

    assert!(death.died);
    assert!(!death.death_presentation_deferred);
    assert!(death
        .direct_packets
        .iter()
        .any(|packet| packet.opcode == WorldOpcode::SmsgForceMoveRoot as u16));
    let snapshot = map.player_runtime_snapshot(7).unwrap();
    assert_eq!(snapshot.death_state, PlayerDeathState::Corpse);
    assert_eq!(snapshot.stand_state, PLAYER_STAND_STATE_DEAD);
}

#[test]
fn map_runtime_grounded_player_world_damage_sends_corpse_presentation_update() {
    let mut map = MapRuntime::new(0, 0);
    let player_guid = ObjectGuid::new(HighGuid::Player, 0, 7);
    map.add_player(test_player_runtime(
        7,
        SessionId::next(),
        WorldPosition::new(0, 0.0, 0.0, 0.0, 0.0),
    ))
    .unwrap();

    let death = map
        .apply_player_world_damage(
            player_guid,
            Some(ObjectGuid::new(HighGuid::Unit, 0, 3196)),
            999,
            WorldDamageKind::SpellDirect,
            Instant::now(),
        )
        .unwrap()
        .expect("damage should apply");

    let mut packed = Vec::new();
    PackedGuid::write(&mut packed, player_guid).unwrap();
    let values_start = 4 + 1 + 1 + packed.len();
    let values = decode_update_values(&death.health_packet.body[values_start..]);
    assert_eq!(values[UNIT_FIELD_HEALTH], Some(0));
    assert_eq!(
        values[UNIT_FIELD_BYTES_1],
        Some(unit_bytes_1_for_class(1) | u32::from(PLAYER_STAND_STATE_DEAD))
    );
    assert_eq!(
        values[PLAYER_FIELD_BYTES],
        Some(PLAYER_FIELD_BYTE_RELEASE_TIMER)
    );
}

#[tokio::test]
async fn far_attack_swing_starts_intent_without_in_combat_flag() {
    let maps = Arc::new(MapRuntimeManager::default());
    let sessions = Arc::new(SessionRegistry::default());
    let object_mgr = ObjectMgr::default();
    let shared_world = SharedWorldDeps {
        object_mgr: &object_mgr,
        maps: &maps,
        sessions: &sessions,
    };
    let parties = PartyManager::default();
    let player_position = WorldPosition::new(0, -8950.0, -130.0, 83.5, 0.0);
    maps.add_player(test_player_runtime(1, SessionId(1), player_position))
        .await
        .unwrap();
    let mut spawn = test_creature_spawn(6);
    spawn.guid = 46;
    spawn.position_x = player_position.x + 40.0;
    spawn.position_y = player_position.y;
    let target = creature_spawn_guid(&spawn);
    maps.share_db_creature_snapshots(0, vec![DbCreatureRuntime::new(spawn)])
        .await;
    let mut body = Vec::new();
    body.extend_from_slice(&target.raw().to_le_bytes());
    let (tx, mut rx) = mpsc::unbounded_channel();
    let mut sink = WorldPacketSink::new(tx);
    let mut session = WorldSessionState {
        character: CharacterSessionState {
            active_character: Some(ActiveCharacter {
                guid: 1,
                name: "Ada".to_string(),
                race: 1,
                class: 1,
                level: 1,
                xp: 0,
                position: player_position,
                movement_flags: 0,
                client_time: 0,
                fall_time: 0,
                jump: JumpInfo::default(),
            }),
            ..CharacterSessionState::default()
        },
        ..WorldSessionState::default()
    };
    let mut header_crypto = HeaderCrypto::new(&[0; 40]);

    handle_attack_swing(
        &mut sink,
        shared_world,
        &parties,
        read_attack_swing_request(&body),
        &mut session,
        &mut header_crypto,
    )
    .await
    .unwrap();

    let snapshot = maps.player_runtime_snapshot(0, 1).await.unwrap();
    assert_eq!(snapshot.active_combat_target, Some(target));
    assert!(!snapshot.in_combat);
    let packets = std::iter::from_fn(|| rx.try_recv().ok()).collect::<Vec<_>>();
    assert!(packets
        .iter()
        .any(|packet| packet.opcode == WorldOpcode::SmsgAttackStart as u16));
    assert!(
        packets
            .iter()
            .all(|packet| packet.opcode != WorldOpcode::SmsgUpdateObject as u16),
        "out-of-range attack intent must not publish UNIT_FLAG_IN_COMBAT"
    );
}

#[tokio::test]
async fn player_attack_stop_broadcasts_to_nearby_observer() {
    let maps = Arc::new(MapRuntimeManager::default());
    let sessions = Arc::new(SessionRegistry::default());
    let object_mgr = ObjectMgr::default();
    let shared_world = SharedWorldDeps {
        object_mgr: &object_mgr,
        maps: &maps,
        sessions: &sessions,
    };
    let player_position = WorldPosition::new(0, -8950.0, -130.0, 83.5, 0.0);
    let observer_position = WorldPosition::new(0, -8952.0, -130.0, 83.5, 0.0);
    let player_session_id = SessionId(1);
    let observer_session_id = SessionId(2);
    let (player_tx, mut player_rx) = mpsc::unbounded_channel();
    let (observer_tx, mut observer_rx) = mpsc::unbounded_channel();
    let mut sink = WorldPacketSink::new(player_tx);
    sessions
        .register(
            observer_session_id,
            SessionHandle {
                account_id: 2,
                character_guid: Some(2),
                character_name: Some("Player2".to_string()),
                outbound: WorldPacketSender::Unbounded(observer_tx),
                disconnect: None,
            },
        )
        .await;
    maps.add_player(test_player_runtime(1, player_session_id, player_position))
        .await
        .unwrap();
    maps.add_player(test_player_runtime(
        2,
        observer_session_id,
        observer_position,
    ))
    .await
    .unwrap();

    let target = ObjectGuid::new(HighGuid::Unit, 0, 77);
    maps.set_player_auto_attack(0, 1, Some(target), Some(Instant::now()))
        .await;
    let mut session = WorldSessionState {
        character: CharacterSessionState {
            active_character: Some(ActiveCharacter {
                guid: 1,
                name: "Ada".to_string(),
                race: 1,
                class: 1,
                level: 1,
                xp: 0,
                position: player_position,
                movement_flags: 0,
                client_time: 0,
                fall_time: 0,
                jump: JumpInfo::default(),
            }),
            ..CharacterSessionState::default()
        },
        ..WorldSessionState::default()
    };
    let mut header_crypto = HeaderCrypto::new(&[0; 40]);

    handle_attack_stop(&mut sink, shared_world, &mut session, &mut header_crypto)
        .await
        .unwrap();

    let player_packets = std::iter::from_fn(|| player_rx.try_recv().ok()).collect::<Vec<_>>();
    let observer_packets = std::iter::from_fn(|| observer_rx.try_recv().ok()).collect::<Vec<_>>();

    assert!(player_packets
        .iter()
        .any(|packet| packet.opcode == WorldOpcode::SmsgAttackStop as u16));
    assert!(observer_packets
        .iter()
        .any(|packet| packet.opcode == WorldOpcode::SmsgAttackStop as u16));
    assert_eq!(
        maps.player_auto_attack_target(0, 1).await,
        None,
        "shared map runtime should clear the player's auto-attack target"
    );
}

#[tokio::test]
async fn player_attack_stop_clears_queued_next_melee_spell_without_active_target() {
    let maps = Arc::new(MapRuntimeManager::default());
    let sessions = Arc::new(SessionRegistry::default());
    let object_mgr = ObjectMgr::default();
    let shared_world = SharedWorldDeps {
        object_mgr: &object_mgr,
        maps: &maps,
        sessions: &sessions,
    };
    let player_position = WorldPosition::new(0, -8950.0, -130.0, 83.5, 0.0);
    maps.add_player(test_player_runtime(1, SessionId(1), player_position))
        .await
        .unwrap();
    let target = ObjectGuid::new(HighGuid::Unit, 0, 77);
    maps.queue_player_next_melee_spell(
        0,
        1,
        QueuedNextMeleeSpell {
            spell_id: WARRIOR_HEROIC_STRIKE_RANK_1,
            target,
            bonus_damage: HEROIC_STRIKE_FIXTURE_DAMAGE,
            rage_cost: HEROIC_STRIKE_RAGE_COST,
            mana_cost: 0,
        },
    )
    .await;
    let (tx, _rx) = mpsc::unbounded_channel();
    let mut sink = WorldPacketSink::new(tx);
    let mut session = WorldSessionState {
        character: CharacterSessionState {
            active_character: Some(ActiveCharacter {
                guid: 1,
                name: "Ada".to_string(),
                race: 1,
                class: 1,
                level: 1,
                xp: 0,
                position: player_position,
                movement_flags: 0,
                client_time: 0,
                fall_time: 0,
                jump: JumpInfo::default(),
            }),
            ..CharacterSessionState::default()
        },
        ..WorldSessionState::default()
    };
    let mut header_crypto = HeaderCrypto::new(&[0; 40]);

    handle_attack_stop(&mut sink, shared_world, &mut session, &mut header_crypto)
        .await
        .unwrap();

    assert!(
        maps.player_runtime_snapshot(0, 1)
            .await
            .unwrap()
            .queued_next_melee_spell
            .is_none(),
        "CMaNGOS MeleeAttackStop interrupts CURRENT_MELEE_SPELL even if no Rust auto-attack target is active"
    );
}

#[tokio::test]
async fn player_attack_stop_preserves_ranged_auto_repeat_spell() {
    let maps = Arc::new(MapRuntimeManager::default());
    let sessions = Arc::new(SessionRegistry::default());
    let object_mgr = ObjectMgr::default();
    let shared_world = SharedWorldDeps {
        object_mgr: &object_mgr,
        maps: &maps,
        sessions: &sessions,
    };
    let player_position = WorldPosition::new(0, -8950.0, -130.0, 83.5, 0.0);
    maps.add_player(test_player_runtime(1, SessionId(1), player_position))
        .await
        .unwrap();
    let target = ObjectGuid::new(HighGuid::Unit, 0, 77);
    let spell_id = 75;
    let now = Instant::now();
    let next_swing = now + Duration::from_millis(2_800);
    maps.set_player_ranged_auto_attack(0, 1, Some(target), Some(next_swing), spell_id)
        .await;

    let (tx, mut rx) = mpsc::unbounded_channel();
    let mut sink = WorldPacketSink::new(tx);
    let mut session = WorldSessionState {
        character: CharacterSessionState {
            active_character: Some(ActiveCharacter {
                guid: 1,
                name: "Ada".to_string(),
                race: 1,
                class: 3,
                level: 1,
                xp: 0,
                position: player_position,
                movement_flags: 0,
                client_time: 0,
                fall_time: 0,
                jump: JumpInfo::default(),
            }),
            ..CharacterSessionState::default()
        },
        ..WorldSessionState::default()
    };
    let mut header_crypto = HeaderCrypto::new(&[0; 40]);

    handle_attack_stop(&mut sink, shared_world, &mut session, &mut header_crypto)
        .await
        .unwrap();

    let direct_packets = std::iter::from_fn(|| rx.try_recv().ok()).collect::<Vec<_>>();
    assert!(
        direct_packets
            .iter()
            .all(|packet| packet.opcode != WorldOpcode::SmsgAttackStop as u16),
        "CMSG_ATTACKSTOP is melee-only and must not acknowledge by stopping Auto Shot"
    );
    let snapshot = maps.player_runtime_snapshot(0, 1).await.unwrap();
    assert_eq!(snapshot.active_combat_target, Some(target));
    assert_eq!(
        snapshot.active_combat_attack_kind,
        PlayerAutoAttackKind::Ranged {
            spell_id,
            phase: PlayerRangedAutoAttackPhase::Windup,
        }
    );
    assert_eq!(snapshot.active_combat_next_swing_at, Some(next_swing));
    assert_eq!(
        maps.player_auto_attack_due(0, 1, now + Duration::from_millis(100))
            .await,
        None
    );
    let snapshot = maps.player_runtime_snapshot(0, 1).await.unwrap();
    assert_eq!(
        snapshot.active_combat_attack_kind,
        PlayerAutoAttackKind::Ranged {
            spell_id,
            phase: PlayerRangedAutoAttackPhase::Windup,
        }
    );
    assert_eq!(snapshot.active_combat_next_swing_at, Some(next_swing));
    assert_eq!(
        maps.player_auto_attack_due(
            0,
            1,
            next_swing - Duration::from_millis(PLAYER_RANGED_AUTO_ATTACK_WINDUP_MILLIS)
        )
        .await,
        None,
        "CMaNGOS keeps the shoot wind-up as an internal auto-repeat delay, not as a separate client packet"
    );
    assert_eq!(
        maps.player_auto_attack_due(0, 1, next_swing).await,
        Some(PlayerAutoAttackDue {
            target,
            kind: PlayerAutoAttackKind::Ranged {
                spell_id,
                phase: PlayerRangedAutoAttackPhase::Shooting,
            },
        }),
        "the ranged scheduler should still deliver the next Auto Shot after melee attack stop"
    );
}

#[tokio::test]
async fn ranged_auto_repeat_cancel_transitions_to_melee_when_target_is_in_reach() {
    let maps = Arc::new(MapRuntimeManager::default());
    let sessions = Arc::new(SessionRegistry::default());
    let object_mgr = ObjectMgr::default();
    let shared_world = SharedWorldDeps {
        object_mgr: &object_mgr,
        maps: &maps,
        sessions: &sessions,
    };
    let player_position = WorldPosition::new(0, -8950.0, -130.0, 83.5, 0.0);
    maps.add_player(test_player_runtime(1, SessionId(1), player_position))
        .await
        .unwrap();
    let mut spawn = test_creature_spawn(6);
    spawn.guid = 77;
    spawn.position_x = player_position.x + 1.0;
    spawn.position_y = player_position.y;
    spawn.position_z = player_position.z;
    let target = creature_spawn_guid(&spawn);
    maps.share_db_creature_snapshots(0, vec![DbCreatureRuntime::new(spawn)])
        .await;
    maps.set_player_ranged_auto_attack_started(
        0,
        1,
        Some(target),
        Instant::now() + Duration::from_millis(PLAYER_RANGED_AUTO_ATTACK_WINDUP_MILLIS),
        75,
    )
    .await;

    let (tx, mut rx) = mpsc::unbounded_channel();
    let mut sink = WorldPacketSink::new(tx);
    let mut session = WorldSessionState {
        character: CharacterSessionState {
            active_character: Some(ActiveCharacter {
                guid: 1,
                name: "Ada".to_string(),
                race: 1,
                class: 3,
                level: 1,
                xp: 0,
                position: player_position,
                movement_flags: 0,
                client_time: 0,
                fall_time: 0,
                jump: JumpInfo::default(),
            }),
            player_health: 46,
            ..CharacterSessionState::default()
        },
        death: DeathSessionState {
            player_death_state: PlayerDeathState::Alive,
            ..DeathSessionState::default()
        },
        ..WorldSessionState::default()
    };
    let mut header_crypto = HeaderCrypto::new(&[0; 40]);

    let transitioned = try_transition_ranged_auto_repeat_to_melee(
        &mut sink,
        shared_world,
        &mut session,
        &mut header_crypto,
        target,
    )
    .await
    .unwrap();

    assert!(transitioned);
    let direct_packets = std::iter::from_fn(|| rx.try_recv().ok()).collect::<Vec<_>>();
    assert!(direct_packets
        .iter()
        .any(|packet| packet.opcode == WorldOpcode::SmsgAttackStart as u16));
    let snapshot = maps.player_runtime_snapshot(0, 1).await.unwrap();
    assert_eq!(snapshot.active_combat_target, Some(target));
    assert_eq!(
        snapshot.active_combat_attack_kind,
        PlayerAutoAttackKind::Melee
    );
    assert!(snapshot.active_combat_next_swing_at.is_some());
}

#[tokio::test]
async fn ranged_auto_attack_projectile_delay_uses_spell_speed_and_min_distance() {
    let maps = Arc::new(MapRuntimeManager::default());
    let sessions = Arc::new(SessionRegistry::default());
    let object_mgr = ObjectMgr::default();
    let shared_world = SharedWorldDeps {
        object_mgr: &object_mgr,
        maps: &maps,
        sessions: &sessions,
    };
    let player_position = WorldPosition::new(0, -8950.0, -130.0, 83.5, 0.0);
    maps.add_player(test_player_runtime(1, SessionId(1), player_position))
        .await
        .unwrap();
    let mut spawn = test_creature_spawn(6);
    spawn.guid = 77;
    spawn.position_x = player_position.x + 2.0;
    spawn.position_y = player_position.y;
    spawn.position_z = player_position.z;
    let target = creature_spawn_guid(&spawn);
    maps.share_db_creature_snapshots(0, vec![DbCreatureRuntime::new(spawn)])
        .await;
    let session = WorldSessionState {
        character: CharacterSessionState {
            active_character: Some(ActiveCharacter {
                guid: 1,
                name: "Ada".to_string(),
                race: 1,
                class: 3,
                level: 1,
                xp: 0,
                position: player_position,
                movement_flags: 0,
                client_time: 0,
                fall_time: 0,
                jump: JumpInfo::default(),
            }),
            ..CharacterSessionState::default()
        },
        ..WorldSessionState::default()
    };

    assert_eq!(
        ranged_auto_attack_travel_delay_millis(
            shared_world,
            &session,
            &auto_shot_spell_template(),
            target,
        )
        .await,
        125,
        "CMaNGOS uses spell projectile speed with a 5 yard minimum travel distance"
    );
}

#[tokio::test]
async fn auto_shot_pending_impact_delays_damage_until_projectile_due() {
    let (tx, mut rx) = mpsc::unbounded_channel();
    let mut stream = WorldPacketSink::new(tx);
    let mut header_crypto = HeaderCrypto::new(&[0; 40]);
    let character_db_pool = MySqlPool::connect_lazy("mysql://root@127.0.0.1/characters").unwrap();
    let world_db_pool = MySqlPool::connect_lazy("mysql://root@127.0.0.1/world").unwrap();
    let maps = Arc::new(MapRuntimeManager::default());
    let sessions = Arc::new(SessionRegistry::default());
    let object_mgr = ObjectMgr::default();
    let shared_world = SharedWorldDeps {
        object_mgr: &object_mgr,
        maps: &maps,
        sessions: &sessions,
    };
    let player_position = WorldPosition::new(0, -8950.0, -130.0, 83.5, 0.0);
    maps.add_player(test_player_runtime(1, SessionId(1), player_position))
        .await
        .unwrap();
    let mut spawn = test_creature_spawn(6);
    spawn.guid = 78;
    spawn.position_x = player_position.x + 20.0;
    spawn.position_y = player_position.y;
    spawn.position_z = player_position.z;
    let target = creature_spawn_guid(&spawn);
    maps.share_db_creature_snapshots(0, vec![DbCreatureRuntime::new(spawn)])
        .await;
    let mut session = WorldSessionState {
        character: CharacterSessionState {
            active_character: Some(ActiveCharacter {
                guid: 1,
                name: "Ada".to_string(),
                race: 1,
                class: 3,
                level: 1,
                xp: 0,
                position: player_position,
                movement_flags: 0,
                client_time: 0,
                fall_time: 0,
                jump: JumpInfo::default(),
            }),
            ..CharacterSessionState::default()
        },
        death: DeathSessionState {
            player_death_state: PlayerDeathState::Alive,
            ..DeathSessionState::default()
        },
        ..WorldSessionState::default()
    };
    let due_at = Instant::now() + Duration::from_millis(40);
    maps.push_pending_ranged_auto_attack_event(
        0,
        1,
        PendingRangedAutoAttackImpact {
            spell_id: 75,
            target,
            outcome: MeleeDamageOutcome::normal_hit(7),
            weapon_skill_id: None,
            due_at,
        },
    )
    .await;

    complete_pending_player_spell_cast(
        &mut stream,
        SpellCastDeps {
            character_db_pool: &character_db_pool,
            world_db_pool: &world_db_pool,
            account_id: 1,
            shared_world,
            parties: &PartyManager::default(),
        },
        &mut session,
        &mut header_crypto,
    )
    .await
    .unwrap();
    assert_eq!(
        maps.db_creature_snapshot(0, target).await.unwrap().health,
        120,
        "Auto Shot damage must not land before the projectile impact event is due"
    );
    assert!(std::iter::from_fn(|| rx.try_recv().ok()).next().is_none());

    tokio::time::sleep(Duration::from_millis(50)).await;
    complete_pending_player_spell_cast(
        &mut stream,
        SpellCastDeps {
            character_db_pool: &character_db_pool,
            world_db_pool: &world_db_pool,
            account_id: 1,
            shared_world,
            parties: &PartyManager::default(),
        },
        &mut session,
        &mut header_crypto,
    )
    .await
    .unwrap();

    let packets = std::iter::from_fn(|| rx.try_recv().ok()).collect::<Vec<_>>();
    let opcodes = packets
        .iter()
        .map(|packet| packet.opcode)
        .collect::<Vec<_>>();
    assert!(
        opcodes.contains(&(WorldOpcode::SmsgSpellNonMeleeDamageLog as u16)),
        "Auto Shot impact is spell weapon damage in CMaNGOS and should not force a melee swing state"
    );
    assert!(!opcodes.contains(&(WorldOpcode::SmsgAttackerStateUpdate as u16)));
    assert!(opcodes.contains(&(WorldOpcode::SmsgUpdateObject as u16)));
    assert_eq!(
        maps.db_creature_snapshot(0, target).await.unwrap().health,
        113
    );
    assert!(maps
        .next_pending_player_spell_cast_due_at(0, 1)
        .await
        .is_none());
}

#[tokio::test]
async fn shared_chase_motion_advances_map_position_for_other_attackers() {
    let maps = Arc::new(MapRuntimeManager::default());
    let sessions = Arc::new(SessionRegistry::default());
    let object_mgr = ObjectMgr::default();
    let shared_world = SharedWorldDeps {
        object_mgr: &object_mgr,
        maps: &maps,
        sessions: &sessions,
    };
    let victim_position = WorldPosition::new(0, 10.0, 0.0, 0.0, 0.0);
    let mut spawn = test_creature_spawn(6);
    spawn.guid = 188;
    spawn.position_x = 0.0;
    spawn.position_y = 0.0;
    spawn.position_z = 0.0;
    let attacker = creature_spawn_guid(&spawn);
    let creature = DbCreatureRuntime::new(spawn);
    maps.share_db_creature_snapshots(0, vec![creature.clone()])
        .await;
    let mut owner_session = WorldSessionState {
        character: CharacterSessionState {
            active_character: Some(ActiveCharacter {
                guid: 1,
                name: "Ada".to_string(),
                race: 1,
                class: 1,
                level: 1,
                xp: 0,
                position: victim_position,
                movement_flags: 0,
                client_time: 0,
                fall_time: 0,
                jump: JumpInfo::default(),
            }),
            player_health: 20,
            ..CharacterSessionState::default()
        },
        death: DeathSessionState {
            player_death_state: PlayerDeathState::Alive,
            ..DeathSessionState::default()
        },
        ..WorldSessionState::default()
    };
    owner_session
        .visibility
        .db_creatures
        .insert(attacker.raw(), creature);
    let now = Instant::now();
    let motion = start_db_creature_chase_motion(
        &mut owner_session,
        attacker,
        ObjectGuid::new(HighGuid::Player, 0, 1),
        now,
    )
    .expect("chase should start");
    maps.update_db_creature_snapshot(
        0,
        owner_session
            .visibility
            .db_creatures
            .get(&attacker.raw())
            .cloned()
            .unwrap(),
    )
    .await;

    let half_duration = Duration::from_millis((motion.duration.as_millis() as u64 / 2).max(1));
    advance_db_creature_motion_and_share(
        shared_world,
        0,
        &mut owner_session,
        attacker,
        now + half_duration,
    )
    .await;

    let shared = maps
        .db_creature_snapshots(0, &[attacker.raw()])
        .await
        .pop()
        .expect("shared creature snapshot");
    let owner = owner_session
        .visibility
        .db_creatures
        .get(&attacker.raw())
        .unwrap();
    assert!(shared.current_position.x > motion.start.x);
    assert_eq!(shared.current_position.x, owner.current_position.x);

    let mut observer_session = WorldSessionState {
        character: CharacterSessionState {
            active_character: Some(ActiveCharacter {
                guid: 2,
                name: "Ben".to_string(),
                race: 1,
                class: 1,
                level: 1,
                xp: 0,
                position: shared.current_position,
                movement_flags: 0,
                client_time: 0,
                fall_time: 0,
                jump: JumpInfo::default(),
            }),
            player_health: 20,
            ..CharacterSessionState::default()
        },
        death: DeathSessionState {
            player_death_state: PlayerDeathState::Alive,
            ..DeathSessionState::default()
        },
        ..WorldSessionState::default()
    };
    observer_session.visibility.db_creatures.insert(
        attacker.raw(),
        DbCreatureRuntime::new(test_creature_spawn(6)),
    );
    sync_session_db_creatures_from_map(shared_world, &mut observer_session).await;
    assert_eq!(
        db_creature_player_melee_check(&observer_session, attacker),
        PlayerMeleeCheck::Clear
    );
}

#[tokio::test]
async fn repeated_auto_attack_input_preserves_swing_timer_and_uses_normal_due_tick() {
    let maps = Arc::new(MapRuntimeManager::default());
    let sessions = Arc::new(SessionRegistry::default());
    let object_mgr = ObjectMgr::default();
    let shared_world = SharedWorldDeps {
        object_mgr: &object_mgr,
        maps: &maps,
        sessions: &sessions,
    };
    let map_id = 0;
    let character_guid = 1;
    let position = WorldPosition::new(map_id, -8950.0, -130.0, 83.5, 0.0);
    maps.add_player(test_player_runtime(character_guid, SessionId(1), position))
        .await
        .unwrap();
    let target = ObjectGuid::new(HighGuid::Unit, 6, 77);
    let now = Instant::now();
    let swing_delay = Duration::from_millis(1200);

    let mut session = WorldSessionState {
        character: CharacterSessionState {
            active_character: Some(ActiveCharacter {
                guid: character_guid,
                name: "Ada".to_string(),
                race: 1,
                class: 1,
                level: 1,
                xp: 0,
                position,
                movement_flags: 0,
                client_time: 0,
                fall_time: 0,
                jump: JumpInfo::default(),
            }),
            ..CharacterSessionState::default()
        },
        ..WorldSessionState::default()
    };

    let first_next =
        scheduled_player_auto_attack_next_swing(shared_world, &session, target, now, swing_delay)
            .await;
    assert_eq!(first_next, now, "first swing should be immediately due");
    maps.set_player_auto_attack(map_id, character_guid, Some(target), Some(first_next))
        .await;

    let repeated_next = scheduled_player_auto_attack_next_swing(
        shared_world,
        &session,
        target,
        now + Duration::from_millis(150),
        swing_delay,
    )
    .await;
    assert_eq!(repeated_next, first_next);

    assert_eq!(
        maps.player_auto_attack_due(
            map_id,
            character_guid,
            first_next - Duration::from_millis(1)
        )
        .await,
        None
    );
    assert_eq!(
        maps.player_auto_attack_due(
            map_id,
            character_guid,
            first_next + Duration::from_millis(1)
        )
        .await,
        Some(PlayerAutoAttackDue {
            target,
            kind: PlayerAutoAttackKind::Melee,
        })
    );
    maps.set_active_player_spell_cast(
        map_id,
        character_guid,
        ActivePlayerSpellCast {
            spell_id: 133,
            source: ActivePlayerSpellCastSource::Player,
            profile: player_spell_cast_profile(&fireball_spell_template()).unwrap(),
            targets: PendingSpellCastTargets {
                target_mask: SPELL_CAST_TARGET_UNIT,
                unit_target: Some(target),
                gameobject_target: None,
                source_location: None,
                destination: None,
            },
            due_at: now + Duration::from_secs(3),
            cast_time_millis: 3_000,
            interrupt_flags: SPELL_INTERRUPT_FLAG_DAMAGE_PUSHBACK,
            damage_pushback_count: 0,
        },
    )
    .await;
    assert_eq!(
        maps.player_auto_attack_due(map_id, character_guid, first_next + Duration::from_secs(1))
            .await,
        None,
        "map-owned active casts suppress white swings until the cast completes or cancels"
    );
    maps.cancel_active_player_spell_cast(map_id, character_guid)
        .await;

    let future_next = now + swing_delay;
    maps.set_player_auto_attack(map_id, character_guid, None, Some(future_next))
        .await;
    let restarted_next = scheduled_player_auto_attack_next_swing(
        shared_world,
        &session,
        target,
        now + Duration::from_millis(250),
        swing_delay,
    )
    .await;
    assert_eq!(
        restarted_next, future_next,
        "manual attack stop/start must preserve the existing swing cooldown"
    );
    assert_eq!(
        maps.player_auto_attack_due(map_id, character_guid, future_next)
            .await,
        None,
        "a preserved cooldown without an active target must not swing by itself"
    );

    session.character.active_character = None;
}

#[tokio::test]
async fn active_cast_suppresses_melee_without_extending_resume_timer_to_cast_end() {
    let maps = Arc::new(MapRuntimeManager::default());
    let map_id = 0;
    let character_guid = 1;
    let position = WorldPosition::new(map_id, -8950.0, -130.0, 83.5, 0.0);
    let mut player = test_player_runtime(character_guid, SessionId(1), position);
    player.combat_stats.main_attack_time_ms = 1_800;
    maps.add_player(player).await.unwrap();
    let target = ObjectGuid::new(HighGuid::Unit, 6, 77);
    let now = Instant::now();
    maps.set_player_auto_attack(
        map_id,
        character_guid,
        Some(target),
        Some(now + Duration::from_millis(1_800)),
    )
    .await;
    maps.set_active_player_spell_cast(
        map_id,
        character_guid,
        ActivePlayerSpellCast {
            spell_id: 133,
            source: ActivePlayerSpellCastSource::Player,
            profile: player_spell_cast_profile(&fireball_spell_template()).unwrap(),
            targets: PendingSpellCastTargets {
                target_mask: 0,
                unit_target: None,
                gameobject_target: None,
                source_location: None,
                destination: None,
            },
            due_at: now + Duration::from_secs(10),
            cast_time_millis: 10_000,
            interrupt_flags: SPELL_INTERRUPT_FLAG_COMBAT,
            damage_pushback_count: 0,
        },
    )
    .await;

    assert_eq!(
        maps.player_auto_attack_due(map_id, character_guid, now)
            .await,
        None,
        "white swings must stay suppressed while a combat-interruptible cast is active"
    );
    assert_eq!(
        maps.player_auto_attack_due(map_id, character_guid, now + Duration::from_secs(9))
            .await,
        None,
        "active cast ownership suppresses an overdue swing without pushing the stored timer to cast end"
    );
    maps.cancel_active_player_spell_cast(map_id, character_guid)
        .await;
    assert_eq!(
        maps.player_auto_attack_due(map_id, character_guid, now + Duration::from_secs(9))
            .await,
        Some(PlayerAutoAttackDue {
            target,
            kind: PlayerAutoAttackKind::Melee,
        }),
        "interrupting a long cast should resume auto-attack from the real swing timer, not wait for the full cast duration"
    );
}

#[tokio::test]
async fn combat_flag_spell_completion_resets_ready_melee_swing_timer() {
    let maps = Arc::new(MapRuntimeManager::default());
    let map_id = 0;
    let character_guid = 1;
    let position = WorldPosition::new(map_id, -8950.0, -130.0, 83.5, 0.0);
    maps.add_player(test_player_runtime(character_guid, SessionId(1), position))
        .await
        .unwrap();
    let target = ObjectGuid::new(HighGuid::Unit, 6, 77);
    let now = Instant::now();
    let melee_delay = Duration::from_millis(1_800);
    maps.set_player_auto_attack(
        map_id,
        character_guid,
        Some(target),
        Some(now - Duration::from_millis(1)),
    )
    .await;

    let adjusted = maps
        .retime_player_auto_attack_after_spell_cast(
            map_id,
            character_guid,
            now,
            melee_delay,
            Duration::from_millis(PLAYER_RANGED_AUTO_ATTACK_WINDUP_MILLIS),
            false,
        )
        .await;

    let expected_next = now + melee_delay;
    assert_eq!(
        adjusted,
        PlayerAutoAttackAfterSpellCast::MeleeRetimed {
            target,
            next_swing_at: expected_next
        }
    );
    assert_eq!(
        maps.player_auto_attack_due(
            map_id,
            character_guid,
            expected_next - Duration::from_millis(1)
        )
        .await,
        None,
        "normal spell casts should reset the white swing timer instead of releasing an overdue swing"
    );
    assert_eq!(
        maps.player_auto_attack_due(map_id, character_guid, expected_next)
            .await,
        Some(PlayerAutoAttackDue {
            target,
            kind: PlayerAutoAttackKind::Melee,
        })
    );
}

#[tokio::test]
async fn combat_flag_spell_completion_delays_auto_shot_without_shortening_weapon_cooldown() {
    let maps = Arc::new(MapRuntimeManager::default());
    let map_id = 0;
    let character_guid = 1;
    let position = WorldPosition::new(map_id, -8950.0, -130.0, 83.5, 0.0);
    maps.add_player(test_player_runtime(character_guid, SessionId(1), position))
        .await
        .unwrap();
    let target = ObjectGuid::new(HighGuid::Unit, 6, 77);
    let now = Instant::now();
    let spell_id = 75;
    maps.set_player_ranged_auto_attack_started(
        map_id,
        character_guid,
        Some(target),
        now - Duration::from_millis(1),
        spell_id,
    )
    .await;

    let adjusted = maps
        .retime_player_auto_attack_after_spell_cast(
            map_id,
            character_guid,
            now,
            Duration::from_millis(2_000),
            Duration::from_millis(PLAYER_RANGED_AUTO_ATTACK_WINDUP_MILLIS),
            false,
        )
        .await;

    let expected_next = now + Duration::from_millis(PLAYER_RANGED_AUTO_ATTACK_WINDUP_MILLIS);
    assert_eq!(
        adjusted,
        PlayerAutoAttackAfterSpellCast::RangedRetimed {
            target,
            spell_id,
            next_shot_at: expected_next,
        }
    );
    assert_eq!(
        maps.player_auto_attack_due(
            map_id,
            character_guid,
            expected_next - Duration::from_millis(1)
        )
        .await,
        None
    );

    let long_cooldown = now + Duration::from_millis(2_400);
    maps.set_player_ranged_next_shot_at(map_id, character_guid, long_cooldown)
        .await;
    let adjusted = maps
        .retime_player_auto_attack_after_spell_cast(
            map_id,
            character_guid,
            now + Duration::from_millis(100),
            Duration::from_millis(2_000),
            Duration::from_millis(PLAYER_RANGED_AUTO_ATTACK_WINDUP_MILLIS),
            false,
        )
        .await;
    assert_eq!(
        adjusted,
        PlayerAutoAttackAfterSpellCast::RangedRetimed {
            target,
            spell_id,
            next_shot_at: long_cooldown,
        },
        "the 500 ms post-cast windup must not shorten an existing ranged weapon cooldown"
    );
}

#[tokio::test]
async fn combat_flag_spell_completion_cancels_wand_auto_repeat_when_required() {
    let maps = Arc::new(MapRuntimeManager::default());
    let map_id = 0;
    let character_guid = 1;
    let position = WorldPosition::new(map_id, -8950.0, -130.0, 83.5, 0.0);
    maps.add_player(test_player_runtime(character_guid, SessionId(1), position))
        .await
        .unwrap();
    let target = ObjectGuid::new(HighGuid::Unit, 6, 77);
    let now = Instant::now();
    let shoot_spell_id = 5019;
    maps.set_player_ranged_auto_attack_started(
        map_id,
        character_guid,
        Some(target),
        now - Duration::from_millis(1),
        shoot_spell_id,
    )
    .await;

    let adjusted = maps
        .retime_player_auto_attack_after_spell_cast(
            map_id,
            character_guid,
            now,
            Duration::from_millis(2_000),
            Duration::from_millis(PLAYER_RANGED_AUTO_ATTACK_WINDUP_MILLIS),
            true,
        )
        .await;

    assert_eq!(
        adjusted,
        PlayerAutoAttackAfterSpellCast::RangedCanceled {
            target,
            spell_id: shoot_spell_id,
        }
    );
    assert_eq!(
        maps.player_auto_attack_due(map_id, character_guid, now + Duration::from_secs(10))
            .await,
        None,
        "wand Shoot has CMaNGOS' casting-cancels-autorepeat attribute and should not resume after a spell cast"
    );
}

#[test]
fn combat_interrupt_flag_drives_spell_cast_attack_timer_reset() {
    let mut fireball = fireball_spell_template();
    fireball.interrupt_flags = SPELL_INTERRUPT_FLAG_COMBAT | SPELL_INTERRUPT_FLAG_DAMAGE_PUSHBACK;
    let profile = SpellInfo::from_template(&fireball)
        .prepare_player_cast()
        .unwrap()
        .profile;
    assert!(spell_resets_auto_attack_timers_on_cast(&fireball, &profile));

    let mut auto_shot = auto_shot_spell_template();
    auto_shot.interrupt_flags = SPELL_INTERRUPT_FLAG_COMBAT;
    let profile = SpellInfo::from_template(&auto_shot)
        .prepare_player_cast()
        .unwrap()
        .profile;
    assert!(
        !spell_resets_auto_attack_timers_on_cast(&auto_shot, &profile),
        "the reset applies to normal spells, not the auto-repeat shot itself"
    );

    auto_shot.attributes_ex3 = SPELL_ATTR_EX3_CASTING_CANCELS_AUTOREPEAT;
    assert!(auto_repeat_spell_cancels_when_casting(&auto_shot));
}

#[tokio::test]
async fn ranged_auto_attack_uses_cmangos_windup_before_weapon_timer() {
    let maps = Arc::new(MapRuntimeManager::default());
    let map_id = 0;
    let character_guid = 1;
    let spell_id = 75;
    let position = WorldPosition::new(map_id, -8950.0, -130.0, 83.5, 0.0);
    maps.add_player(test_player_runtime(character_guid, SessionId(1), position))
        .await
        .unwrap();
    let target = ObjectGuid::new(HighGuid::Unit, 6, 77);
    let now = Instant::now();

    maps.set_player_ranged_auto_attack(map_id, character_guid, Some(target), None, spell_id)
        .await;
    assert_eq!(
        maps.player_auto_attack_due(map_id, character_guid, now)
            .await,
        None,
        "Auto Shot should wait for CMaNGOS' 500 ms internal shoot wind-up before the first triggered shot"
    );
    let snapshot = maps
        .player_runtime_snapshot(map_id, character_guid)
        .await
        .unwrap();
    assert_eq!(
        snapshot.active_combat_next_swing_at,
        Some(now + Duration::from_millis(PLAYER_RANGED_AUTO_ATTACK_WINDUP_MILLIS))
    );
    assert_eq!(
        snapshot.active_combat_attack_kind,
        PlayerAutoAttackKind::Ranged {
            spell_id,
            phase: PlayerRangedAutoAttackPhase::Windup,
        }
    );
    assert_eq!(
        maps.player_auto_attack_due(
            map_id,
            character_guid,
            now + Duration::from_millis(PLAYER_RANGED_AUTO_ATTACK_WINDUP_MILLIS - 1)
        )
        .await,
        None
    );
    assert_eq!(
        maps.player_auto_attack_due(
            map_id,
            character_guid,
            now + Duration::from_millis(PLAYER_RANGED_AUTO_ATTACK_WINDUP_MILLIS)
        )
        .await,
        Some(PlayerAutoAttackDue {
            target,
            kind: PlayerAutoAttackKind::Ranged {
                spell_id,
                phase: PlayerRangedAutoAttackPhase::Shooting,
            },
        })
    );

    let next_weapon_swing =
        now + Duration::from_millis(PLAYER_RANGED_AUTO_ATTACK_WINDUP_MILLIS + 2_800);
    maps.set_player_ranged_next_shot_at(map_id, character_guid, next_weapon_swing)
        .await;
    assert_eq!(
        maps.player_auto_attack_due(
            map_id,
            character_guid,
            next_weapon_swing - Duration::from_millis(PLAYER_RANGED_AUTO_ATTACK_WINDUP_MILLIS + 1)
        )
        .await,
        None
    );
    assert_eq!(
        maps.player_auto_attack_due(
            map_id,
            character_guid,
            next_weapon_swing - Duration::from_millis(PLAYER_RANGED_AUTO_ATTACK_WINDUP_MILLIS)
        )
        .await,
        None,
        "repeat Auto Shot should send its triggered start/go pair at release, after the weapon timer is ready"
    );
    assert_eq!(
        maps.player_auto_attack_due(map_id, character_guid, next_weapon_swing)
            .await,
        Some(PlayerAutoAttackDue {
            target,
            kind: PlayerAutoAttackKind::Ranged {
                spell_id,
                phase: PlayerRangedAutoAttackPhase::Shooting,
            },
        })
    );
}

#[tokio::test]
async fn ranged_auto_repeat_restart_preserves_weapon_cooldown_after_cancel() {
    let maps = Arc::new(MapRuntimeManager::default());
    let map_id = 0;
    let character_guid = 1;
    let spell_id = 75;
    let position = WorldPosition::new(map_id, -8950.0, -130.0, 83.5, 0.0);
    maps.add_player(test_player_runtime(character_guid, SessionId(1), position))
        .await
        .unwrap();
    let target = ObjectGuid::new(HighGuid::Unit, 6, 77);
    let now = Instant::now();
    let requested_first_shot = now + Duration::from_millis(PLAYER_RANGED_AUTO_ATTACK_WINDUP_MILLIS);

    let first_shot_at = maps
        .set_player_ranged_auto_attack_started(
            map_id,
            character_guid,
            Some(target),
            requested_first_shot,
            spell_id,
        )
        .await;
    assert_eq!(first_shot_at, requested_first_shot);

    let next_weapon_shot = now + Duration::from_millis(2_800);
    maps.set_player_ranged_next_shot_at(map_id, character_guid, next_weapon_shot)
        .await;
    maps.set_player_auto_attack(map_id, character_guid, None, None)
        .await;

    let requested_restart_shot = now + Duration::from_millis(1_200);
    let restart_shot_at = maps
        .set_player_ranged_auto_attack_started(
            map_id,
            character_guid,
            Some(target),
            requested_restart_shot,
            spell_id,
        )
        .await;
    assert_eq!(
        restart_shot_at, next_weapon_shot,
        "canceling and restarting Auto Shot must not shorten the ranged weapon cooldown"
    );
    let snapshot = maps
        .player_runtime_snapshot(map_id, character_guid)
        .await
        .unwrap();
    assert_eq!(snapshot.active_combat_next_swing_at, Some(next_weapon_shot));
    assert_eq!(
        maps.player_auto_attack_due(map_id, character_guid, requested_restart_shot)
            .await,
        None
    );
    assert_eq!(
        maps.player_auto_attack_due(map_id, character_guid, next_weapon_shot)
            .await,
        Some(PlayerAutoAttackDue {
            target,
            kind: PlayerAutoAttackKind::Ranged {
                spell_id,
                phase: PlayerRangedAutoAttackPhase::Shooting,
            },
        })
    );
}

#[tokio::test]
async fn ranged_auto_attack_movement_does_not_shortcut_long_weapon_cooldown() {
    let maps = Arc::new(MapRuntimeManager::default());
    let map_id = 0;
    let character_guid = 1;
    let spell_id = 75;
    let position = WorldPosition::new(map_id, -8950.0, -130.0, 83.5, 0.0);
    maps.add_player(test_player_runtime(character_guid, SessionId(1), position))
        .await
        .unwrap();
    let target = ObjectGuid::new(HighGuid::Unit, 6, 77);
    let now = Instant::now();
    let long_weapon_cooldown = now + Duration::from_millis(2_800);

    maps.set_player_ranged_auto_attack(
        map_id,
        character_guid,
        Some(target),
        Some(long_weapon_cooldown),
        spell_id,
    )
    .await;
    let moving = MovementInfo {
        flags: MOVEFLAG_FORWARD,
        client_time: 1,
        position,
        fall_time: 0,
        jump: JumpInfo::default(),
    };
    maps.update_player_position(
        map_id,
        character_guid,
        WorldOpcode::MsgMoveStartForward as u16,
        &moving,
        1,
    )
    .await
    .unwrap();
    assert_eq!(
        maps.player_auto_attack_due(map_id, character_guid, now + Duration::from_millis(100))
            .await,
        None
    );
    let standing = MovementInfo {
        flags: 0,
        client_time: 2,
        position,
        fall_time: 0,
        jump: JumpInfo::default(),
    };
    maps.update_player_position(
        map_id,
        character_guid,
        WorldOpcode::MsgMoveStop as u16,
        &standing,
        2,
    )
    .await
    .unwrap();
    let stop_time = now + Duration::from_millis(200);
    assert_eq!(
        maps.player_auto_attack_due(map_id, character_guid, stop_time)
            .await,
        None
    );
    let snapshot = maps
        .player_runtime_snapshot(map_id, character_guid)
        .await
        .unwrap();
    assert_eq!(
        snapshot.active_combat_next_swing_at,
        Some(long_weapon_cooldown),
        "movement reset should preserve a weapon cooldown longer than the 500 ms shoot wind-up"
    );
    assert_eq!(
        maps.player_auto_attack_due(
            map_id,
            character_guid,
            long_weapon_cooldown - Duration::from_millis(PLAYER_RANGED_AUTO_ATTACK_WINDUP_MILLIS)
        )
        .await,
        None
    );
    assert_eq!(
        maps.player_auto_attack_due(map_id, character_guid, long_weapon_cooldown)
            .await,
        Some(PlayerAutoAttackDue {
            target,
            kind: PlayerAutoAttackKind::Ranged {
                spell_id,
                phase: PlayerRangedAutoAttackPhase::Shooting,
            },
        })
    );
}

#[tokio::test]
async fn map_owned_active_cast_damage_pushback_extends_remaining_cast_time() {
    let maps = MapRuntimeManager::default();
    let map_id = 0;
    let character_guid = 7;
    let now = Instant::now();
    let mut fireball = fireball_spell_template();
    fireball.interrupt_flags = SPELL_INTERRUPT_FLAG_DAMAGE_PUSHBACK;
    maps.set_active_player_spell_cast(
        map_id,
        character_guid,
        ActivePlayerSpellCast {
            spell_id: 133,
            source: ActivePlayerSpellCastSource::Player,
            profile: player_spell_cast_profile(&fireball).unwrap(),
            targets: PendingSpellCastTargets {
                target_mask: 0,
                unit_target: None,
                gameobject_target: None,
                source_location: None,
                destination: None,
            },
            due_at: now + Duration::from_millis(1_500),
            cast_time_millis: 1_500,
            interrupt_flags: fireball.interrupt_flags,
            damage_pushback_count: 0,
        },
    )
    .await;

    let delay = maps
        .delay_active_player_spell_cast_for_damage(
            map_id,
            character_guid,
            now + Duration::from_millis(500),
        )
        .await;

    assert_eq!(
        delay,
        Some(500),
        "CMaNGOS caps damage pushback so remaining cast time never exceeds original cast time"
    );
    assert_eq!(
        maps.next_pending_player_spell_cast_due_at(map_id, character_guid)
            .await,
        Some(now + Duration::from_millis(2_000))
    );
}

#[tokio::test]
async fn map_owned_active_cast_without_damage_flags_ignores_damage_interrupt() {
    let maps = MapRuntimeManager::default();
    let map_id = 0;
    let character_guid = 7;
    let now = Instant::now();
    let fireball = fireball_spell_template();
    maps.set_active_player_spell_cast(
        map_id,
        character_guid,
        ActivePlayerSpellCast {
            spell_id: 133,
            source: ActivePlayerSpellCastSource::Player,
            profile: player_spell_cast_profile(&fireball).unwrap(),
            targets: PendingSpellCastTargets {
                target_mask: 0,
                unit_target: None,
                gameobject_target: None,
                source_location: None,
                destination: None,
            },
            due_at: now + Duration::from_millis(1_500),
            cast_time_millis: 1_500,
            interrupt_flags: 0,
            damage_pushback_count: 0,
        },
    )
    .await;

    assert!(
        maps.cancel_active_player_spell_cast_for_damage(map_id, character_guid)
            .await
            .is_none(),
        "CMaNGOS only cancels player casts on damage when DAMAGE_CANCELS is present"
    );
    assert_eq!(
        maps.delay_active_player_spell_cast_for_damage(
            map_id,
            character_guid,
            now + Duration::from_millis(500),
        )
        .await,
        None,
        "CMaNGOS Spell::Delayed is a no-op without DAMAGE_PUSHBACK"
    );
    assert_eq!(
        maps.next_pending_player_spell_cast_due_at(map_id, character_guid)
            .await,
        Some(now + Duration::from_millis(1_500))
    );
}

#[tokio::test]
async fn map_owned_active_cast_damage_cancels_when_flagged() {
    let maps = MapRuntimeManager::default();
    let map_id = 0;
    let character_guid = 7;
    let now = Instant::now();
    let mut fireball = fireball_spell_template();
    fireball.interrupt_flags =
        SPELL_INTERRUPT_FLAG_DAMAGE_CANCELS | SPELL_INTERRUPT_FLAG_DAMAGE_PUSHBACK;
    maps.set_active_player_spell_cast(
        map_id,
        character_guid,
        ActivePlayerSpellCast {
            spell_id: 133,
            source: ActivePlayerSpellCastSource::Player,
            profile: player_spell_cast_profile(&fireball).unwrap(),
            targets: PendingSpellCastTargets {
                target_mask: 0,
                unit_target: None,
                gameobject_target: None,
                source_location: None,
                destination: None,
            },
            due_at: now + Duration::from_millis(1_500),
            cast_time_millis: 1_500,
            interrupt_flags: fireball.interrupt_flags,
            damage_pushback_count: 0,
        },
    )
    .await;

    let cancelled = maps
        .cancel_active_player_spell_cast_for_damage(map_id, character_guid)
        .await
        .expect("damage-cancel flagged cast should be removed");

    assert_eq!(cancelled.spell_id, 133);
    assert_eq!(
        maps.next_pending_player_spell_cast_due_at(map_id, character_guid)
            .await,
        None,
        "DAMAGE_CANCELS wins over pushback and removes the active cast"
    );
}

#[test]
fn spell_delayed_packet_uses_full_caster_guid_for_client_cast_bar() {
    let caster = ObjectGuid::new(HighGuid::Player, 0, 7);
    let body = build_spell_delayed_body(caster, 500).unwrap();

    assert_eq!(body.len(), 12);
    assert_eq!(&body[0..8], &caster.raw().to_le_bytes());
    assert_eq!(&body[8..12], &500u32.to_le_bytes());
}

#[tokio::test]
async fn killing_blow_target_clear_preserves_weapon_swing_cooldown_for_retarget() {
    let maps = Arc::new(MapRuntimeManager::default());
    let sessions = Arc::new(SessionRegistry::default());
    let object_mgr = ObjectMgr::default();
    let shared_world = SharedWorldDeps {
        object_mgr: &object_mgr,
        maps: &maps,
        sessions: &sessions,
    };
    let map_id = 0;
    let character_guid = 1;
    let position = WorldPosition::new(map_id, -8950.0, -130.0, 83.5, 0.0);
    maps.add_player(test_player_runtime(character_guid, SessionId(1), position))
        .await
        .unwrap();

    let killed_target = ObjectGuid::new(HighGuid::Unit, 6, 77);
    let next_target = ObjectGuid::new(HighGuid::Unit, 6, 78);
    let now = Instant::now();
    let weapon_delay = Duration::from_millis(1600);
    let next_swing = now + weapon_delay;
    let session = WorldSessionState {
        character: CharacterSessionState {
            active_character: Some(ActiveCharacter {
                guid: character_guid,
                name: "Ada".to_string(),
                race: 1,
                class: 1,
                level: 1,
                xp: 0,
                position,
                movement_flags: 0,
                client_time: 0,
                fall_time: 0,
                jump: JumpInfo::default(),
            }),
            ..CharacterSessionState::default()
        },
        ..WorldSessionState::default()
    };

    maps.set_player_auto_attack(map_id, character_guid, Some(killed_target), Some(now))
        .await;
    maps.set_player_auto_attack(map_id, character_guid, None, Some(next_swing))
        .await;

    let retargeted_next = scheduled_player_auto_attack_next_swing(
        shared_world,
        &session,
        next_target,
        now + Duration::from_millis(100),
        weapon_delay,
    )
    .await;
    assert_eq!(
        retargeted_next, next_swing,
        "a killing blow should clear the dead target, not the player's weapon cooldown"
    );
    assert_eq!(
        maps.player_auto_attack_due(
            map_id,
            character_guid,
            next_swing - Duration::from_millis(1)
        )
        .await,
        None
    );
    maps.set_player_auto_attack(
        map_id,
        character_guid,
        Some(next_target),
        Some(retargeted_next),
    )
    .await;
    assert_eq!(
        maps.player_auto_attack_due(map_id, character_guid, next_swing)
            .await,
        Some(PlayerAutoAttackDue {
            target: next_target,
            kind: PlayerAutoAttackKind::Melee,
        })
    );
}
