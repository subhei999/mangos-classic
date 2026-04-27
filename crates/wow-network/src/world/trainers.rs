async fn handle_trainer_list(
    stream: &mut TcpStream,
    character_db_pool: &MySqlPool,
    world_db_pool: &MySqlPool,
    body: &[u8],
    session: &mut WorldSessionState,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    let guid = read_packet_guid(body, "CMSG_TRAINER_LIST")?;
    send_trainer_list(
        stream,
        character_db_pool,
        world_db_pool,
        guid,
        session,
        header_crypto,
    )
    .await
}

async fn send_trainer_list(
    stream: &mut TcpStream,
    character_db_pool: &MySqlPool,
    world_db_pool: &MySqlPool,
    guid: ObjectGuid,
    session: &mut WorldSessionState,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    let Some(character) = &session.active_character else {
        warn!("Ignoring trainer list before character login");
        return Ok(());
    };
    if !guid.is_creature() {
        warn!(
            guid = format_args!("0x{:016X}", guid.raw()),
            "Ignoring trainer list request for non-creature"
        );
        return Ok(());
    }

    let Some(template) = wow_db::get_creature_template_query(world_db_pool, guid.entry()).await?
    else {
        warn!(
            entry = guid.entry(),
            "Ignoring trainer list request for unknown creature template"
        );
        return Ok(());
    };
    let spells = wow_db::get_trainer_spells(world_db_pool, guid.entry()).await?;
    if spells.is_empty() {
        warn!(entry = guid.entry(), "Ignoring trainer list request with no DB trainer spells");
        return Ok(());
    }
    if template.npc_flags & UNIT_NPC_FLAG_TRAINER == 0 {
        warn!(
            entry = guid.entry(),
            "Serving trainer list for creature with DB trainer rows but no trainer NPC flag"
        );
    }

    let known_spells = wow_db::get_character_spells(character_db_pool, character.guid).await?;
    let list_spells: Vec<TrainerListSpell> = spells
        .iter()
        .filter(|_| trainer_spell_matches_class(&template, character.class))
        .map(|spell| TrainerListSpell::from_query(spell, character, &known_spells))
        .collect();
    let body = build_trainer_list_body(
        guid,
        template.trainer_type.max(0) as u32,
        &list_spells,
        DB_TRAINER_GREETING,
    );
    send_packet(stream, SMSG_TRAINER_LIST, &body, Some(header_crypto)).await
}

async fn handle_trainer_buy_spell(
    stream: &mut TcpStream,
    character_db_pool: &MySqlPool,
    world_db_pool: &MySqlPool,
    body: &[u8],
    session: &mut WorldSessionState,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    let Some(character) = &session.active_character else {
        warn!("Ignoring trainer buy before character login");
        return Ok(());
    };
    let request = TrainerBuySpellRequest::read(body)?;
    if !request.trainer_guid.is_creature() {
        return Ok(());
    }
    let Some(template) =
        wow_db::get_creature_template_query(world_db_pool, request.trainer_guid.entry()).await?
    else {
        return Ok(());
    };
    let spells = wow_db::get_trainer_spells(world_db_pool, request.trainer_guid.entry()).await?;
    let Some(spell) = spells.iter().find(|spell| spell.spell == request.spell) else {
        return Ok(());
    };
    let known_spells = wow_db::get_character_spells(character_db_pool, character.guid).await?;
    let list_spell = TrainerListSpell::from_query(spell, character, &known_spells);
    if !trainer_spell_matches_class(&template, character.class)
        || list_spell.state != TRAINER_SPELL_GREEN
    {
        return Ok(());
    }

    let Some(new_money) = wow_db::learn_character_spell(
        character_db_pool,
        character.guid,
        list_spell.learned_spell,
        spell.spell_cost,
    )
    .await?
    else {
        return send_packet(
            stream,
            SMSG_TRAINER_BUY_FAILED,
            &build_trainer_buy_failed_body(request.trainer_guid, request.spell, 0),
            Some(header_crypto),
        )
        .await;
    };
    session.active_spells.insert(list_spell.learned_spell);
    send_packet(
        stream,
        SMSG_TRAINER_BUY_SUCCEEDED,
        &build_trainer_buy_succeeded_body(request.trainer_guid, request.spell),
        Some(&mut *header_crypto),
    )
    .await?;
    send_packet(
        stream,
        SMSG_LEARNED_SPELL,
        &build_learned_spell_body(list_spell.learned_spell),
        Some(&mut *header_crypto),
    )
    .await?;
    send_packet(
        stream,
        SMSG_INITIAL_SPELLS,
        &build_initial_spells_body(
            &wow_db::get_character_spells(character_db_pool, character.guid).await?,
        ),
        Some(&mut *header_crypto),
    )
    .await?;
    if spell.spell_cost > 0 {
        send_packet(
            stream,
            SMSG_UPDATE_OBJECT,
            &build_player_money_update_body(character.guid, new_money)?,
            Some(header_crypto),
        )
        .await?;
    }
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct TrainerBuySpellRequest {
    trainer_guid: ObjectGuid,
    spell: u32,
}

impl TrainerBuySpellRequest {
    fn read(body: &[u8]) -> anyhow::Result<Self> {
        if body.len() < 12 {
            anyhow::bail!(
                "CMSG_TRAINER_BUY_SPELL payload too short: {} bytes",
                body.len()
            );
        }
        Ok(Self {
            trainer_guid: ObjectGuid::from_raw(u64::from_le_bytes(body[0..8].try_into()?)),
            spell: u32::from_le_bytes(body[8..12].try_into()?),
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct TrainerListSpell {
    spell: u32,
    learned_spell: u32,
    state: u8,
    cost: u32,
    req_level: u8,
    req_skill: u32,
    req_skill_value: u32,
    req_ability: [u32; 3],
}

impl TrainerListSpell {
    fn from_query(
        query: &wow_db::TrainerSpellQuery,
        character: &ActiveCharacter,
        known_spells: &[wow_db::CharacterSpell],
    ) -> Self {
        let known = known_spells
            .iter()
            .any(|spell| spell.spell == query.learned_spell && spell.active != 0 && spell.disabled == 0);
        let req_ability = [
            query.req_ability1.unwrap_or(0),
            query.req_ability2.unwrap_or(0),
            query.req_ability3.unwrap_or(0),
        ];
        let missing_required_ability = req_ability.iter().any(|ability| {
            *ability != 0
                && !known_spells
                    .iter()
                    .any(|spell| spell.spell == *ability && spell.active != 0 && spell.disabled == 0)
        });
        let state = if known {
            TRAINER_SPELL_GRAY
        } else if character.level < query.req_level
            || query.req_skill != 0
            || missing_required_ability
        {
            TRAINER_SPELL_RED
        } else {
            TRAINER_SPELL_GREEN
        };
        Self {
            spell: query.spell,
            learned_spell: query.learned_spell,
            state,
            cost: query.spell_cost,
            req_level: query.req_level,
            req_skill: query.req_skill,
            req_skill_value: query.req_skill_value,
            req_ability,
        }
    }
}

fn trainer_spell_matches_class(template: &wow_db::CreatureTemplateQuery, class: u8) -> bool {
    template.trainer_class == 0 || template.trainer_class == class
}

fn build_trainer_list_body(
    trainer: ObjectGuid,
    trainer_type: u32,
    spells: &[TrainerListSpell],
    greeting: &str,
) -> Vec<u8> {
    let mut body = Vec::with_capacity(8 + 4 + 4 + spells.len() * 38 + greeting.len() + 1);
    body.extend_from_slice(&trainer.raw().to_le_bytes());
    body.extend_from_slice(&trainer_type.to_le_bytes());
    body.extend_from_slice(&(spells.len() as u32).to_le_bytes());
    for spell in spells {
        body.extend_from_slice(&spell.spell.to_le_bytes());
        body.push(spell.state);
        body.extend_from_slice(&spell.cost.to_le_bytes());
        body.extend_from_slice(&0u32.to_le_bytes());
        body.extend_from_slice(&0u32.to_le_bytes());
        body.push(spell.req_level);
        body.extend_from_slice(&spell.req_skill.to_le_bytes());
        body.extend_from_slice(&spell.req_skill_value.to_le_bytes());
        for ability in spell.req_ability {
            body.extend_from_slice(&ability.to_le_bytes());
        }
    }
    write_c_string(&mut body, greeting);
    body
}

fn build_trainer_buy_succeeded_body(trainer: ObjectGuid, spell: u32) -> Vec<u8> {
    let mut body = Vec::with_capacity(12);
    body.extend_from_slice(&trainer.raw().to_le_bytes());
    body.extend_from_slice(&spell.to_le_bytes());
    body
}

fn build_trainer_buy_failed_body(trainer: ObjectGuid, spell: u32, reason: u32) -> Vec<u8> {
    let mut body = Vec::with_capacity(16);
    body.extend_from_slice(&trainer.raw().to_le_bytes());
    body.extend_from_slice(&spell.to_le_bytes());
    body.extend_from_slice(&reason.to_le_bytes());
    body
}

fn build_learned_spell_body(spell: u32) -> Vec<u8> {
    spell.to_le_bytes().to_vec()
}

const TRAINER_SPELL_GREEN: u8 = 0;
const TRAINER_SPELL_RED: u8 = 1;
const TRAINER_SPELL_GRAY: u8 = 2;
