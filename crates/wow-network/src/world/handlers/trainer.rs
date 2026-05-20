use super::*;
use wow_proto::{
    ServerWorldPacket, SmsgLearnedSpellResponse, SmsgPlaySpellImpactResponse,
    SmsgPlaySpellVisualResponse, SmsgTrainerBuyFailedResponse, SmsgTrainerBuySucceededResponse,
    SmsgTrainerListResponse, TrainerListSpellResponse,
};

pub(in crate::world) async fn handle_trainer_list(
    stream: &mut WorldPacketSink,
    character_db_pool: &MySqlPool,
    world_db_pool: &MySqlPool,
    request: wow_proto::TrainerListRequest,
    session: &mut WorldSessionState,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    let guid = ObjectGuid::from_raw(request.raw_guid);
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

pub(in crate::world) async fn send_trainer_list(
    stream: &mut WorldPacketSink,
    character_db_pool: &MySqlPool,
    world_db_pool: &MySqlPool,
    guid: ObjectGuid,
    session: &mut WorldSessionState,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    let Some(character) = &session.character.active_character else {
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
        warn!(
            entry = guid.entry(),
            "Ignoring trainer list request with no DB trainer spells"
        );
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
    if list_spells.is_empty() {
        warn!(
            entry = guid.entry(),
            trainer_name = template.name.as_str(),
            trainer_type = template.trainer_type,
            trainer_class = template.trainer_class,
            character_guid = character.guid,
            character_class = character.class,
            raw_spells = spells.len(),
            "Trainer list is empty after class filter"
        );
    } else {
        let green_spells = list_spells
            .iter()
            .filter(|spell| spell.state == TRAINER_SPELL_GREEN)
            .count();
        let red_spells = list_spells
            .iter()
            .filter(|spell| spell.state == TRAINER_SPELL_RED)
            .count();
        let gray_spells = list_spells
            .iter()
            .filter(|spell| spell.state == TRAINER_SPELL_GRAY)
            .count();
        info!(
            entry = guid.entry(),
            trainer_name = template.name.as_str(),
            trainer_type = template.trainer_type,
            trainer_class = template.trainer_class,
            character_guid = character.guid,
            character_class = character.class,
            raw_spells = spells.len(),
            listed_spells = list_spells.len(),
            green_spells,
            red_spells,
            gray_spells,
            first_spell = list_spells.first().map(|spell| spell.spell).unwrap_or(0),
            "Sending trainer list"
        );
    }
    let body = build_trainer_list_body(
        guid,
        template.trainer_type.max(0) as u32,
        &list_spells,
        &wow_db::get_trainer_greeting(world_db_pool, guid.entry())
            .await?
            .filter(|text| !text.trim().is_empty())
            .unwrap_or_else(|| DB_TRAINER_GREETING.to_string()),
    );
    send_packet(stream, SMSG_TRAINER_LIST, &body, Some(header_crypto)).await
}

pub(in crate::world) async fn handle_trainer_buy_spell(
    stream: &mut WorldPacketSink,
    character_db_pool: &MySqlPool,
    world_db_pool: &MySqlPool,
    request: wow_proto::TrainerBuySpellRequest,
    session: &mut WorldSessionState,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    let Some(character) = &session.character.active_character else {
        warn!("Ignoring trainer buy before character login");
        return Ok(());
    };
    let request = TrainerBuySpellRequest::from(request);
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
    session
        .character
        .active_spells
        .insert(list_spell.learned_spell);
    send_packet(
        stream,
        SMSG_PLAY_SPELL_VISUAL,
        &build_play_spell_visual_body(request.trainer_guid, 0xB3),
        Some(&mut *header_crypto),
    )
    .await?;
    send_packet(
        stream,
        SMSG_PLAY_SPELL_IMPACT,
        &build_play_spell_impact_body(character.guid, 0x016A),
        Some(&mut *header_crypto),
    )
    .await?;
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
    let known_spells = wow_db::get_character_spells(character_db_pool, character.guid).await?;
    send_known_proficiencies(
        stream,
        world_db_pool,
        &known_spells,
        Some(&mut *header_crypto),
    )
    .await?;
    send_packet(
        stream,
        SMSG_INITIAL_SPELLS,
        &build_initial_spells_body(&known_spells),
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
pub(in crate::world) struct TrainerBuySpellRequest {
    pub(in crate::world) trainer_guid: ObjectGuid,
    pub(in crate::world) spell: u32,
}

impl From<wow_proto::TrainerBuySpellRequest> for TrainerBuySpellRequest {
    fn from(request: wow_proto::TrainerBuySpellRequest) -> Self {
        Self {
            trainer_guid: ObjectGuid::from_raw(request.trainer_raw_guid),
            spell: request.spell,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::world) struct TrainerListSpell {
    pub(in crate::world) spell: u32,
    pub(in crate::world) learned_spell: u32,
    pub(in crate::world) state: u8,
    pub(in crate::world) cost: u32,
    pub(in crate::world) req_level: u8,
    pub(in crate::world) req_skill: u32,
    pub(in crate::world) req_skill_value: u32,
    pub(in crate::world) req_ability: [u32; 3],
}

impl TrainerListSpell {
    pub(in crate::world) fn from_query(
        query: &wow_db::TrainerSpellQuery,
        character: &ActiveCharacter,
        known_spells: &[wow_db::CharacterSpell],
    ) -> Self {
        let known = known_spells.iter().any(|spell| {
            spell.spell == query.learned_spell && spell.active != 0 && spell.disabled == 0
        });
        let req_ability = [
            query.req_ability1.unwrap_or(0),
            query.req_ability2.unwrap_or(0),
            query.req_ability3.unwrap_or(0),
        ];
        let missing_required_ability = req_ability.iter().any(|ability| {
            *ability != 0
                && !known_spells.iter().any(|spell| {
                    spell.spell == *ability && spell.active != 0 && spell.disabled == 0
                })
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

pub(in crate::world) fn trainer_spell_matches_class(
    template: &wow_db::CreatureTemplateQuery,
    class: u8,
) -> bool {
    template.trainer_class == 0 || template.trainer_class == class
}

pub(in crate::world) fn build_trainer_list_body(
    trainer: ObjectGuid,
    trainer_type: u32,
    spells: &[TrainerListSpell],
    greeting: &str,
) -> Vec<u8> {
    SmsgTrainerListResponse {
        trainer,
        trainer_type,
        spells: spells
            .iter()
            .map(|spell| TrainerListSpellResponse {
                spell: spell.spell,
                state: spell.state,
                cost: spell.cost,
                req_level: spell.req_level,
                req_skill: spell.req_skill,
                req_skill_value: spell.req_skill_value,
                req_ability: spell.req_ability,
            })
            .collect(),
        greeting: greeting.to_string(),
    }
    .body()
}

pub(in crate::world) fn build_trainer_buy_succeeded_body(
    trainer: ObjectGuid,
    spell: u32,
) -> Vec<u8> {
    SmsgTrainerBuySucceededResponse { trainer, spell }.body()
}

pub(in crate::world) fn build_trainer_buy_failed_body(
    trainer: ObjectGuid,
    spell: u32,
    reason: u32,
) -> Vec<u8> {
    SmsgTrainerBuyFailedResponse {
        trainer,
        spell,
        reason,
    }
    .body()
}

pub(in crate::world) fn build_learned_spell_body(spell: u32) -> Vec<u8> {
    SmsgLearnedSpellResponse { spell }.body()
}

pub(in crate::world) fn build_play_spell_visual_body(
    guid: ObjectGuid,
    spell_visual_kit: u32,
) -> Vec<u8> {
    SmsgPlaySpellVisualResponse {
        guid,
        spell_visual_kit,
    }
    .body()
}

pub(in crate::world) fn build_play_spell_impact_body(guid: u32, spell_visual_kit: u32) -> Vec<u8> {
    SmsgPlaySpellImpactResponse {
        guid: ObjectGuid::new(HighGuid::Player, REALM_ID, guid),
        spell_visual_kit,
    }
    .body()
}

pub(in crate::world) const TRAINER_SPELL_GREEN: u8 = 0;
pub(in crate::world) const TRAINER_SPELL_RED: u8 = 1;
pub(in crate::world) const TRAINER_SPELL_GRAY: u8 = 2;
