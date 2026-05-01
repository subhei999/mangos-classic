// Shared packet-body helpers.

fn push_cstring(body: &mut Vec<u8>, value: &str) {
    body.extend_from_slice(value.as_bytes());
    body.push(0);
}

fn read_packet_guid(body: &[u8], packet_name: &str) -> anyhow::Result<ObjectGuid> {
    if body.len() < 8 {
        anyhow::bail!("{packet_name} payload must include an 8-byte GUID");
    }
    Ok(ObjectGuid::from_raw(u64::from_le_bytes(
        body[0..8].try_into()?,
    )))
}
