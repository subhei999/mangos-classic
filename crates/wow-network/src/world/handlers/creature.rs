use super::*;

pub(in crate::world) async fn handle_creature_query(
    stream: &mut WorldPacketSink,
    world_db_pool: &MySqlPool,
    query: wow_proto::CreatureQueryRequest,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    let db_template = wow_db::get_creature_template_query(world_db_pool, query.entry).await?;
    let guid = ObjectGuid::from_raw(query.raw_guid);
    info!(
        entry = query.entry,
        guid = format_args!("0x{:016X}", guid.raw()),
        found = db_template.is_some() || query.entry == RUST_GUIDE_ENTRY,
        "Answering creature template query"
    );
    let response = build_creature_query_response(query.entry, db_template.as_ref());
    send_packet(
        stream,
        SMSG_CREATURE_QUERY_RESPONSE,
        &response,
        Some(header_crypto),
    )
    .await
}

pub(in crate::world) fn build_creature_query_response(
    entry: u32,
    db_template: Option<&CreatureTemplateQuery>,
) -> Vec<u8> {
    let Some(template) = creature_query_template(entry, db_template) else {
        return (entry | 0x8000_0000).to_le_bytes().to_vec();
    };

    let mut body = Vec::with_capacity(100);
    body.extend_from_slice(&entry.to_le_bytes());
    write_c_string(&mut body, template.name);
    body.push(0);
    body.push(0);
    body.push(0);
    write_c_string(&mut body, template.subname);
    body.extend_from_slice(&template.creature_type_flags.to_le_bytes());
    body.extend_from_slice(&template.creature_type.to_le_bytes());
    body.extend_from_slice(&(template.family as u32).to_le_bytes());
    body.extend_from_slice(&template.rank.to_le_bytes());
    body.extend_from_slice(&0u32.to_le_bytes()); // unknown
    body.extend_from_slice(&template.pet_spell_data_id.to_le_bytes());
    body.extend_from_slice(&template.display_id.to_le_bytes());
    body.extend_from_slice(&(template.civilian as u16).to_le_bytes());
    body
}

pub(in crate::world) struct FixtureCreatureTemplate {
    pub(in crate::world) name: &'static str,
    pub(in crate::world) subname: &'static str,
    pub(in crate::world) display_id: u32,
}

pub(in crate::world) struct CreatureQueryTemplate<'a> {
    pub(in crate::world) name: &'a str,
    pub(in crate::world) subname: &'a str,
    pub(in crate::world) creature_type_flags: u32,
    pub(in crate::world) creature_type: u32,
    pub(in crate::world) family: i32,
    pub(in crate::world) rank: u32,
    pub(in crate::world) pet_spell_data_id: u32,
    pub(in crate::world) display_id: u32,
    pub(in crate::world) civilian: u8,
}

pub(in crate::world) fn creature_query_template<'a>(
    entry: u32,
    db_template: Option<&'a CreatureTemplateQuery>,
) -> Option<CreatureQueryTemplate<'a>> {
    if let Some(template) = db_template {
        return Some(CreatureQueryTemplate {
            name: &template.name,
            subname: template.subname.as_deref().unwrap_or(""),
            creature_type_flags: template.creature_type_flags,
            creature_type: template.creature_type,
            family: template.family,
            rank: template.rank,
            pet_spell_data_id: template.pet_spell_data_id,
            display_id: creature_display_id(template),
            civilian: template.civilian,
        });
    }

    let template = fixture_creature_template(entry)?;
    Some(CreatureQueryTemplate {
        name: template.name,
        subname: template.subname,
        creature_type_flags: 0,
        creature_type: 7,
        family: 0,
        rank: 0,
        pet_spell_data_id: 0,
        display_id: template.display_id,
        civilian: 0,
    })
}

pub(in crate::world) fn fixture_creature_template(entry: u32) -> Option<FixtureCreatureTemplate> {
    match entry {
        RUST_GUIDE_ENTRY => Some(FixtureCreatureTemplate {
            name: RUST_GUIDE_NAME,
            subname: RUST_GUIDE_SUBNAME,
            display_id: RUST_GUIDE_DISPLAY_ID,
        }),
        _ => None,
    }
}
