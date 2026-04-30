async fn handle_creature_query(
    stream: &mut WorldPacketSink,
    world_db_pool: &MySqlPool,
    body: &[u8],
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    let query = CreatureQuery::read(body)?;
    let db_template = wow_db::get_creature_template_query(world_db_pool, query.entry).await?;
    info!(
        entry = query.entry,
        guid = format_args!("0x{:016X}", query.guid.raw()),
        found = db_template.is_some()
            || matches!(query.entry, RUST_GUIDE_ENTRY | RUST_COMBAT_DUMMY_ENTRY),
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct CreatureQuery {
    entry: u32,
    guid: ObjectGuid,
}

impl CreatureQuery {
    fn read(body: &[u8]) -> anyhow::Result<Self> {
        let mut cursor = 0;
        let entry = read_u32(body, &mut cursor)?;
        ensure_available(body, cursor + 8)?;
        let guid = ObjectGuid::from_raw(u64::from_le_bytes(body[cursor..cursor + 8].try_into()?));
        Ok(Self { entry, guid })
    }
}

fn build_creature_query_response(entry: u32, db_template: Option<&CreatureTemplateQuery>) -> Vec<u8> {
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
    body.extend_from_slice(&0u32.to_le_bytes()); // type flags
    body.extend_from_slice(&template.creature_type.to_le_bytes());
    body.extend_from_slice(&(template.family as u32).to_le_bytes());
    body.extend_from_slice(&template.rank.to_le_bytes());
    body.extend_from_slice(&0u32.to_le_bytes()); // unknown
    body.extend_from_slice(&template.pet_spell_data_id.to_le_bytes());
    body.extend_from_slice(&template.display_id.to_le_bytes());
    body.extend_from_slice(&(template.civilian as u16).to_le_bytes());
    body
}

struct FixtureCreatureTemplate {
    name: &'static str,
    subname: &'static str,
    display_id: u32,
}

struct CreatureQueryTemplate<'a> {
    name: &'a str,
    subname: &'a str,
    creature_type: u32,
    family: i32,
    rank: u32,
    pet_spell_data_id: u32,
    display_id: u32,
    civilian: u8,
}

fn creature_query_template<'a>(
    entry: u32,
    db_template: Option<&'a CreatureTemplateQuery>,
) -> Option<CreatureQueryTemplate<'a>> {
    if let Some(template) = db_template {
        return Some(CreatureQueryTemplate {
            name: &template.name,
            subname: template.subname.as_deref().unwrap_or(""),
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
        creature_type: 7,
        family: 0,
        rank: 0,
        pet_spell_data_id: 0,
        display_id: template.display_id,
        civilian: 0,
    })
}

fn fixture_creature_template(entry: u32) -> Option<FixtureCreatureTemplate> {
    match entry {
        RUST_GUIDE_ENTRY => Some(FixtureCreatureTemplate {
            name: RUST_GUIDE_NAME,
            subname: RUST_GUIDE_SUBNAME,
            display_id: RUST_GUIDE_DISPLAY_ID,
        }),
        RUST_COMBAT_DUMMY_ENTRY => Some(FixtureCreatureTemplate {
            name: RUST_COMBAT_DUMMY_NAME,
            subname: RUST_COMBAT_DUMMY_SUBNAME,
            display_id: RUST_COMBAT_DUMMY_DISPLAY_ID,
        }),
        _ => None,
    }
}

