// Shared packet-body helpers.

pub(in crate::world) fn push_cstring(body: &mut Vec<u8>, value: &str) {
    body.extend_from_slice(value.as_bytes());
    body.push(0);
}
