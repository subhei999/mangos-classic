#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AuraOwnerKind {
    Player,
    DbCreature,
}

fn visible_aura_slots(active_auras: &[ActiveAura]) -> Vec<(usize, &ActiveAura)> {
    let mut positive_slot = 0;
    let mut negative_slot = MAX_POSITIVE_AURA_SLOTS;
    let mut slots = Vec::new();
    for aura in active_auras.iter().filter(|aura| aura.visible) {
        let slot = if aura.positive {
            if positive_slot >= MAX_POSITIVE_AURA_SLOTS {
                continue;
            }
            let slot = positive_slot;
            positive_slot += 1;
            slot
        } else {
            if negative_slot >= MAX_AURA_SLOTS {
                continue;
            }
            let slot = negative_slot;
            negative_slot += 1;
            slot
        };
        slots.push((slot, aura));
    }
    slots
}

fn build_aura_duration_update_body(slot: u8, remaining_millis: u32) -> Vec<u8> {
    let mut body = Vec::with_capacity(5);
    body.push(slot);
    body.extend_from_slice(&remaining_millis.to_le_bytes());
    body
}

fn build_player_aura_duration_update_packets(
    active_auras: &[ActiveAura],
    now: Instant,
) -> Vec<OutboundWorldPacket> {
    visible_aura_slots(active_auras)
        .into_iter()
        .filter_map(|(slot, aura)| {
            aura.remaining_duration_millis(now)
                .map(|remaining_millis| OutboundWorldPacket {
                    opcode: SMSG_UPDATE_AURA_DURATION,
                    body: build_aura_duration_update_body(slot as u8, remaining_millis),
                })
        })
        .collect()
}
