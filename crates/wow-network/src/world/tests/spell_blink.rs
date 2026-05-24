#[tokio::test]
async fn blink_live_rank_one_row_uses_generic_front_leap_teleport_path() {
    let world_db_pool =
        MySqlPool::connect_lazy("mysql://mangos:mangos@127.0.0.1:3307/mangos").unwrap();

    let blink = wow_db::get_spell_template_query(&world_db_pool, 1953)
        .await
        .unwrap()
        .expect("Blink rank 1 should exist in the local spell_template");

    assert_eq!(blink.spell_name, "Blink");
    assert_eq!(blink.rank.as_deref(), Some(""));
    assert_eq!(blink.spell_level, 20);
    assert_eq!(blink.effect1, SPELL_EFFECT_LEAP);
    assert_eq!(blink.effect_implicit_target_a1, TARGET_UNIT_CASTER);
    assert_eq!(blink.effect_implicit_target_b1, TARGET_LOCATION_CASTER_FRONT_LEAP);
    assert_eq!(blink.effect_radius_index1, 9);
    assert_eq!(
        spell_effect_support(SPELL_EFFECT_LEAP),
        SpellMechanicSupport::Implemented
    );

    let profile = player_spell_cast_profile(&blink).expect("blink cast profile");
    assert_eq!(profile.kind, SpellCastKind::Teleport);
    assert!(matches!(
        profile.power,
        SpellPowerCost::Mana { cost } if cost == blink.mana_cost
    ));
    assert!(!profile.requires_melee);
    assert!(!profile.requires_behind);
    assert!(!profile.needs_combo_points);

    let plan = SpellInfo::from_template(&blink)
        .player_spell_plan()
        .expect("Blink rank 1 should build a generic player spell plan");
    assert_eq!(plan.target, SpellPlanTarget::Caster);
    assert!(plan.effects.iter().any(|effect| {
        effect.dispatch == SpellEffectDispatch::Leap
            && effect.target == SpellPlanEffectTarget::CasterFrontLeap
    }));
}

#[test]
fn blink_front_leap_destination_stops_before_blocked_los_segment() {
    let start = WorldPosition::new(0, 10.0, 20.0, 30.0, 0.0);

    let destination = near_teleport_front_leap_destination(
        start,
        6.0,
        Some,
        |_from, to| to.x <= 14.0,
    )
    .expect("blocked Blink path should still return the last valid step");

    assert_eq!(destination.map_id, 0);
    assert!((destination.x - 14.0).abs() < f32::EPSILON);
    assert!((destination.y - 20.0).abs() < f32::EPSILON);
    assert!((destination.z - 30.0).abs() < f32::EPSILON);
}

#[test]
fn blink_front_leap_destination_stops_before_steep_slope_step() {
    let start = WorldPosition::new(0, 10.0, 20.0, 30.0, 0.0);

    let destination = near_teleport_front_leap_destination(
        start,
        6.0,
        |position| {
            Some(if position.x >= 14.0 {
                WorldPosition::new(
                    position.map_id,
                    position.x,
                    position.y,
                    position.z + 5.0,
                    position.orientation,
                )
            } else {
                position
            })
        },
        |_from, _to| true,
    )
    .expect("steep Blink path should clamp to the last walkable step");

    assert_eq!(destination.map_id, 0);
    assert!((destination.x - 12.0).abs() < f32::EPSILON);
    assert!((destination.y - 20.0).abs() < f32::EPSILON);
    assert!((destination.z - 30.0).abs() < f32::EPSILON);
}
