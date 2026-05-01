// CMaNGOS reference: src/game/Handlers/GossipDef.cpp gossip packet builders.

#[cfg(test)]
fn build_rust_guide_gossip_message() -> Vec<u8> {
    build_gossip_message(
        rust_guide_guid(),
        RUST_GUIDE_GOSSIP_TEXT_ID,
        &[(0, RUST_GUIDE_GOSSIP_OPTION)],
    )
}

#[cfg(test)]
fn build_rust_guide_npc_text_update(text_id: u32) -> Vec<u8> {
    build_npc_text_update(text_id, RUST_GUIDE_GOSSIP_TEXT)
}

fn build_gossip_message(guid: ObjectGuid, text_id: u32, options: &[(u32, &str)]) -> Vec<u8> {
    let option_text_len: usize = options.iter().map(|(_, text)| text.len() + 1).sum();

    let mut body = Vec::with_capacity(16 + options.len() * 6 + option_text_len);

    body.extend_from_slice(&guid.raw().to_le_bytes());

    body.extend_from_slice(&text_id.to_le_bytes());

    body.extend_from_slice(&(options.len() as u32).to_le_bytes());

    for (option_index, option_text) in options {
        body.extend_from_slice(&option_index.to_le_bytes());

        body.push(0); // icon

        body.push(0); // coded

        write_c_string(&mut body, option_text);
    }

    body.extend_from_slice(&0u32.to_le_bytes()); // quest option count

    body
}

fn build_npc_text_update(text_id: u32, primary_text: &str) -> Vec<u8> {
    let mut body = Vec::with_capacity(220);

    body.extend_from_slice(&text_id.to_le_bytes());

    for index in 0..8 {
        body.extend_from_slice(&(if index == 0 { 1.0f32 } else { 0.0f32 }).to_le_bytes());

        let text = if index == 0 { primary_text } else { "" };

        write_c_string(&mut body, text);

        write_c_string(&mut body, text);

        body.extend_from_slice(&0u32.to_le_bytes()); // language

        for _ in 0..3 {
            body.extend_from_slice(&0u32.to_le_bytes()); // emote delay

            body.extend_from_slice(&0u32.to_le_bytes()); // emote id
        }
    }

    body
}
