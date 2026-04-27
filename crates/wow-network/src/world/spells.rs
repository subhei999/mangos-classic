async fn handle_cast_spell(
    stream: &mut TcpStream,
    character_db_pool: &MySqlPool,
    world_db_pool: &MySqlPool,
    body: &[u8],
    session: &mut WorldSessionState,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    let packet = CastSpellPacket::read(body)?;
    let Some(character) = &session.active_character else {
        warn!(
            spell_id = packet.spell_id,
            "Ignoring spell cast before character login"
        );
        return Ok(());
    };
    let character_guid = character.guid;

    let Some(starter_spell) = supported_starter_spell(packet.spell_id) else {
        warn!(
            spell_id = packet.spell_id,
            "Ignoring unsupported spell cast in starter spell fixture slice"
        );
        return Ok(());
    };
    if !session.active_spells.contains(&packet.spell_id) {
        warn!(
            spell_id = packet.spell_id,
            character_guid,
            "Ignoring starter spell cast for spell not active on character"
        );
        return Ok(());
    }

    let caster = ObjectGuid::new(HighGuid::Player, 0, character_guid);
    let targets = normalize_fixture_spell_targets(packet.targets);
    match starter_spell.power {
        StarterSpellPower::Rage { cost } => {
            session.player_rage = session.player_rage.saturating_sub(cost);
        }
        StarterSpellPower::Mana { cost } => {
            session.player_mana = session.player_mana.saturating_sub(cost);
        }
    }
    send_packet(
        stream,
        SMSG_CAST_RESULT,
        &build_cast_result_ok_body(packet.spell_id),
        Some(&mut *header_crypto),
    )
    .await?;
    send_packet(
        stream,
        SMSG_SPELL_GO,
        &build_spell_go_body(caster, packet.spell_id, &targets)?,
        Some(&mut *header_crypto),
    )
    .await?;
    if targets.unit_target == Some(rust_combat_dummy_guid())
        && !session.combat_dummy_lootable
        && session.combat_dummy_health > 0
    {
        let damage = session.combat_dummy_health.min(starter_spell.damage);
        session.combat_dummy_health = session.combat_dummy_health.saturating_sub(damage);
        if session.combat_dummy_health == 0 {
            session.combat_dummy_lootable = true;
            session.combat_dummy_looting = false;
            session.combat_dummy_loot_money_available = true;
            session.combat_dummy_loot_item_available = true;
            session.active_combat_target = None;
        }
        send_packet(
            stream,
            SMSG_ATTACKERSTATEUPDATE,
            &build_attacker_state_update_body_with_spell_id(
                caster,
                rust_combat_dummy_guid(),
                damage,
                packet.spell_id,
            )?,
            Some(&mut *header_crypto),
        )
        .await?;
        send_packet(
            stream,
            SMSG_UPDATE_OBJECT,
            &build_combat_dummy_state_update_body(
                session.combat_dummy_health,
                if session.combat_dummy_health == 0 {
                    UNIT_DYNFLAG_LOOTABLE
                } else {
                    0
                },
            )?,
            Some(&mut *header_crypto),
        )
        .await?;
    } else if let Some(target) = targets.unit_target {
        if let Some(damage) = apply_db_creature_damage(session, target, starter_spell.damage) {
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
                .expect("creature damage target checked above");
            send_packet(
                stream,
                SMSG_ATTACKERSTATEUPDATE,
                &build_attacker_state_update_body_with_spell_id(
                    caster,
                    target,
                    damage,
                    packet.spell_id,
                )?,
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
            if is_dead {
                finalize_db_creature_death(
                    stream,
                    character_db_pool,
                    world_db_pool,
                    session,
                    caster,
                    target,
                    header_crypto,
                )
                .await?;
            } else {
                begin_db_creature_combat(session, target, Instant::now());
            }
        }
    }
    let power_update = match starter_spell.power {
        StarterSpellPower::Rage { .. } => build_player_rage_update_body(caster, session.player_rage)?,
        StarterSpellPower::Mana { .. } => build_player_mana_update_body(caster, session.player_mana)?,
    };
    send_packet(stream, SMSG_UPDATE_OBJECT, &power_update, Some(header_crypto)).await
}

fn normalize_fixture_spell_targets(mut targets: SpellCastTargets) -> SpellCastTargets {
    targets.target_mask =
        (targets.target_mask | SPELL_CAST_TARGET_UNIT) & !SPELL_CAST_TARGET_UNIT_ENEMY;
    targets.unit_target = Some(targets.unit_target.unwrap_or_else(rust_combat_dummy_guid));
    targets
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct SupportedStarterSpell {
    damage: u32,
    power: StarterSpellPower,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StarterSpellPower {
    Rage { cost: u32 },
    Mana { cost: u32 },
}

fn supported_starter_spell(spell_id: u32) -> Option<SupportedStarterSpell> {
    match spell_id {
        WARRIOR_HEROIC_STRIKE_RANK_1 => Some(SupportedStarterSpell {
            damage: HEROIC_STRIKE_FIXTURE_DAMAGE,
            power: StarterSpellPower::Rage {
                cost: HEROIC_STRIKE_RAGE_COST,
            },
        }),
        HUNTER_RAPTOR_STRIKE_RANK_1 => Some(SupportedStarterSpell {
            damage: RAPTOR_STRIKE_FIXTURE_DAMAGE,
            power: StarterSpellPower::Mana {
                cost: RAPTOR_STRIKE_MANA_COST,
            },
        }),
        _ => None,
    }
}

fn build_cast_result_ok_body(spell_id: u32) -> Vec<u8> {
    let mut body = Vec::with_capacity(5);
    body.extend_from_slice(&spell_id.to_le_bytes());
    body.push(0);
    body
}

fn build_spell_go_body(
    caster: ObjectGuid,
    spell_id: u32,
    targets: &SpellCastTargets,
) -> anyhow::Result<Vec<u8>> {
    let mut body = Vec::with_capacity(40);
    PackedGuid::write(&mut body, caster)?;
    PackedGuid::write(&mut body, caster)?;
    body.extend_from_slice(&spell_id.to_le_bytes());
    body.extend_from_slice(&CAST_FLAG_SPELL_GO.to_le_bytes());

    if let Some(target) = targets.unit_target {
        body.push(1);
        body.extend_from_slice(&target.raw().to_le_bytes());
    } else {
        body.push(0);
    }
    body.push(0); // miss count
    targets.write(&mut body)?;
    Ok(body)
}

async fn handle_item_query_single(
    stream: &mut TcpStream,
    world_db_pool: &MySqlPool,
    body: &[u8],
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    if body.len() < 4 {
        anyhow::bail!(
            "CMSG_ITEM_QUERY_SINGLE payload too short: {} bytes",
            body.len()
        );
    }

    let item = u32::from_le_bytes(body[0..4].try_into()?);
    let template = wow_db::get_item_template_query(world_db_pool, item).await?;
    info!(
        item,
        found = template.is_some(),
        "Answering item template query"
    );
    let response = build_item_query_single_response(item, template.as_ref());
    send_packet(
        stream,
        SMSG_ITEM_QUERY_SINGLE_RESPONSE,
        &response,
        Some(header_crypto),
    )
    .await
}

