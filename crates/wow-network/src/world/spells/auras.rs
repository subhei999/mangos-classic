use super::*;
use wow_proto::world::WorldOpcode;
use wow_proto::{ServerWorldPacket, SmsgUpdateAuraDurationResponse};

#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::world) enum AuraOwnerKind {
    Player,
    DbCreature,
}

#[derive(Debug, Clone, Copy)]
pub(in crate::world) struct VisibleAuraSlot<'a> {
    pub(in crate::world) slot: usize,
    pub(in crate::world) aura: &'a ActiveAura,
    pub(in crate::world) applications: u8,
}

pub(in crate::world) fn visible_aura_slots(
    active_auras: &[ActiveAura],
) -> Vec<VisibleAuraSlot<'_>> {
    let mut positive_slot = 0;
    let mut negative_slot = MAX_POSITIVE_AURA_SLOTS;
    let mut slots: Vec<VisibleAuraSlot<'_>> = Vec::new();
    for aura in active_auras.iter().filter(|aura| aura.visible) {
        if let Some(existing) = slots.iter_mut().find(|existing| {
            existing.aura.positive == aura.positive
                && existing.aura.spell_id == aura.spell_id
                && aura_supports_visible_applications(existing.aura)
                && aura_supports_visible_applications(aura)
                && existing.aura.stat_modifiers == aura.stat_modifiers
        }) {
            existing.applications = existing.applications.saturating_add(1);
            continue;
        }
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
        slots.push(VisibleAuraSlot {
            slot,
            aura,
            applications: 0,
        });
    }
    slots
}

fn aura_supports_visible_applications(aura: &ActiveAura) -> bool {
    aura.periodic_damage.is_none() && aura.periodic_regen.is_none() && aura.proc_triggers.is_empty()
}

pub(in crate::world) fn build_aura_duration_update_body(
    slot: u8,
    remaining_millis: u32,
) -> Vec<u8> {
    SmsgUpdateAuraDurationResponse {
        slot,
        remaining_millis,
    }
    .body()
}

pub(in crate::world) fn build_player_aura_duration_update_packets(
    active_auras: &[ActiveAura],
    now: Instant,
) -> Vec<OutboundWorldPacket> {
    visible_aura_slots(active_auras)
        .into_iter()
        .filter_map(|visible| {
            let slot = visible.slot;
            let aura = visible.aura;
            aura.remaining_duration_millis(now)
                .map(|remaining_millis| OutboundWorldPacket {
                    opcode: WorldOpcode::SmsgUpdateAuraDuration as u16,
                    body: build_aura_duration_update_body(slot as u8, remaining_millis),
                })
        })
        .collect()
}
