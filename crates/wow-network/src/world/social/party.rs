use super::*;
use wow_proto::{
    GroupListMemberResponse, LootRollItemResponse, PartyMemberStatsResponse, ServerWorldPacket,
    SmsgEmptyGroupListResponse, SmsgGroupInviteResponse, SmsgGroupListResponse,
    SmsgGroupSetLeaderResponse, SmsgLootAllPassedResponse, SmsgLootRollResponse,
    SmsgLootRollWonResponse, SmsgLootStartRollResponse, SmsgPartyCommandResultResponse,
};

// CMaNGOS reference: src/game/Groups/GroupHandler.cpp and Group.cpp.

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(in crate::world) struct PartyId(pub(in crate::world) u64);

impl PartyId {
    pub(in crate::world) fn next() -> Self {
        static NEXT_PARTY_ID: AtomicU64 = AtomicU64::new(1);
        Self(NEXT_PARTY_ID.fetch_add(1, Ordering::Relaxed))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::world) struct PartyMember {
    pub(in crate::world) guid: u32,
    pub(in crate::world) name: String,
}

#[derive(Debug, Clone)]
pub(in crate::world) struct Party {
    pub(in crate::world) id: PartyId,
    pub(in crate::world) leader: u32,
    pub(in crate::world) members: Vec<PartyMember>,
    pub(in crate::world) raid: bool,
    pub(in crate::world) subgroups: HashMap<u32, u8>,
    pub(in crate::world) assistants: HashSet<u32>,
    pub(in crate::world) loot_method: u8,
    pub(in crate::world) master_looter: u32,
    pub(in crate::world) loot_threshold: u8,
    pub(in crate::world) next_looter: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::world) struct PendingPartyInvite {
    pub(in crate::world) party_id: PartyId,
    pub(in crate::world) inviter: PartyMember,
    pub(in crate::world) invitee: PartyMember,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub(in crate::world) struct PartyMembership {
    pub(in crate::world) party_id: PartyId,
    pub(in crate::world) leader: u32,
    pub(in crate::world) raid: bool,
}

#[derive(Debug, Default)]
pub(in crate::world) struct PartyManager {
    pub(in crate::world) parties: Mutex<HashMap<PartyId, Party>>,
    pub(in crate::world) membership: Mutex<HashMap<u32, PartyId>>,
    pub(in crate::world) invites_by_invitee: Mutex<HashMap<u32, PendingPartyInvite>>,
    pub(in crate::world) loot_rolls: Mutex<HashMap<(u64, u8), LootRollState>>,
}

#[derive(Debug, Clone)]
pub(in crate::world) struct LootRollState {
    pub(in crate::world) party_id: PartyId,
    pub(in crate::world) map_id: u32,
    pub(in crate::world) loot_guid: ObjectGuid,
    pub(in crate::world) loot_slot: u8,
    pub(in crate::world) loot: DbCreatureLootRuntime,
    pub(in crate::world) voters: Vec<u32>,
    pub(in crate::world) votes: HashMap<u32, LootRollChoice>,
    pub(in crate::world) ends_at: Instant,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::world) struct LootRollChoice {
    pub(in crate::world) vote: LootRollVote,
    pub(in crate::world) number: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::world) enum LootRollVote {
    Pass = 0,
    Need = 1,
    Greed = 2,
}

#[derive(Debug)]
pub(in crate::world) struct LootRollStart {
    pub(in crate::world) packets: Vec<(u32, OutboundWorldPacket)>,
}

#[derive(Debug)]
pub(in crate::world) struct LootRollVoteOutcome {
    pub(in crate::world) map_id: u32,
    pub(in crate::world) loot_guid: ObjectGuid,
    pub(in crate::world) loot_slot: u8,
    pub(in crate::world) winner: Option<u32>,
    pub(in crate::world) loot: Option<DbCreatureLootRuntime>,
    pub(in crate::world) packets: Vec<(u32, OutboundWorldPacket)>,
}

pub(in crate::world) const GROUP_LOOT_ROLL_TIMEOUT: Duration = Duration::from_millis(60_000);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::world) enum PartyResult {
    Ok,
    BadPlayerName,
    TargetNotInGroup,
    GroupFull,
    AlreadyInGroup,
    NotInGroup,
    NotLeader,
}

impl PartyResult {
    pub(in crate::world) fn code(self) -> u32 {
        match self {
            PartyResult::Ok => 0,
            PartyResult::BadPlayerName => 1,
            PartyResult::TargetNotInGroup => 2,
            PartyResult::GroupFull => 3,
            PartyResult::AlreadyInGroup => 4,
            PartyResult::NotInGroup => 5,
            PartyResult::NotLeader => 6,
        }
    }
}

#[derive(Debug)]
pub(in crate::world) struct PartyInviteOutcome {
    pub(in crate::world) result: PartyResult,
    pub(in crate::world) invitee_session: Option<SessionId>,
    pub(in crate::world) invite_packet: Option<OutboundWorldPacket>,
}

#[derive(Debug)]
pub(in crate::world) struct PartyMutationOutcome {
    pub(in crate::world) result: PartyResult,
    pub(in crate::world) packets: Vec<(u32, OutboundWorldPacket)>,
}

impl PartyManager {
    #[allow(dead_code)]
    pub(in crate::world) async fn membership(
        &self,
        character_guid: u32,
    ) -> Option<PartyMembership> {
        let party_id = self.membership.lock().await.get(&character_guid).copied()?;
        let parties = self.parties.lock().await;
        let party = parties.get(&party_id)?;
        Some(PartyMembership {
            party_id,
            leader: party.leader,
            raid: party.raid,
        })
    }

    #[allow(dead_code)]
    pub(in crate::world) async fn same_party(&self, left: u32, right: u32) -> bool {
        if left == right {
            return true;
        }
        let membership = self.membership.lock().await;
        membership.get(&left).is_some_and(|party_id| {
            membership
                .get(&right)
                .is_some_and(|other_party_id| other_party_id == party_id)
        })
    }

    pub(in crate::world) async fn party_members(&self, character_guid: u32) -> Vec<PartyMember> {
        let Some(party_id) = self.membership.lock().await.get(&character_guid).copied() else {
            return Vec::new();
        };
        self.parties
            .lock()
            .await
            .get(&party_id)
            .map(|party| party.members.clone())
            .unwrap_or_default()
    }

    pub(in crate::world) async fn group_list_packet_for(
        &self,
        character_guid: u32,
    ) -> Option<OutboundWorldPacket> {
        let party_id = self.membership.lock().await.get(&character_guid).copied()?;
        let parties = self.parties.lock().await;
        let party = parties.get(&party_id)?;
        Some(OutboundWorldPacket {
            opcode: SMSG_GROUP_LIST,
            body: build_group_list_body(party, character_guid),
        })
    }

    pub(in crate::world) async fn loot_owner_for(&self, character_guid: u32) -> CreatureLootOwner {
        self.membership
            .lock()
            .await
            .get(&character_guid)
            .map(|party_id| CreatureLootOwner::Party(party_id.0))
            .unwrap_or(CreatureLootOwner::Player(character_guid))
    }

    pub(in crate::world) async fn loot_method_for(
        &self,
        character_guid: u32,
    ) -> Option<(u8, u8, u32)> {
        let party_id = self.membership.lock().await.get(&character_guid).copied()?;
        let parties = self.parties.lock().await;
        let party = parties.get(&party_id)?;
        Some((party.loot_method, party.loot_threshold, party.master_looter))
    }

    pub(in crate::world) async fn assign_current_looter(&self, character_guid: u32) -> Option<u32> {
        let party_id = self.membership.lock().await.get(&character_guid).copied()?;
        let mut parties = self.parties.lock().await;
        let party = parties.get_mut(&party_id)?;
        if party.members.is_empty() {
            return None;
        }
        if !party
            .members
            .iter()
            .any(|member| member.guid == party.next_looter)
        {
            party.next_looter = party.members[0].guid;
        }
        let current = party.next_looter;
        if let Some(index) = party
            .members
            .iter()
            .position(|member| member.guid == current)
        {
            let next_index = (index + 1) % party.members.len();
            party.next_looter = party.members[next_index].guid;
        }
        Some(current)
    }

    #[allow(dead_code)]
    pub(in crate::world) async fn master_loot_members_for(
        &self,
        character_guid: u32,
    ) -> Option<Vec<u32>> {
        let party_id = self.membership.lock().await.get(&character_guid).copied()?;
        let parties = self.parties.lock().await;
        let party = parties.get(&party_id)?;
        if party.loot_method != 2 || party.master_looter != character_guid {
            return None;
        }
        Some(party.members.iter().map(|member| member.guid).collect())
    }

    pub(in crate::world) async fn start_loot_roll(
        &self,
        character_guid: u32,
        map_id: u32,
        loot_guid: ObjectGuid,
        loot_slot: u8,
        loot: DbCreatureLootRuntime,
    ) -> Option<LootRollStart> {
        let party_id = self.membership.lock().await.get(&character_guid).copied()?;
        let party = self.parties.lock().await.get(&party_id).cloned()?;
        if party.loot_method != 3 {
            return None;
        }
        let voters = party
            .members
            .iter()
            .map(|member| member.guid)
            .collect::<Vec<_>>();
        if voters.len() <= 1 {
            return None;
        }
        let mut rolls = self.loot_rolls.lock().await;
        if rolls.contains_key(&(loot_guid.raw(), loot_slot)) {
            return None;
        }
        rolls.insert(
            (loot_guid.raw(), loot_slot),
            LootRollState {
                party_id,
                map_id,
                loot_guid,
                loot_slot,
                loot: loot.clone(),
                voters: voters.clone(),
                votes: HashMap::new(),
                ends_at: Instant::now() + GROUP_LOOT_ROLL_TIMEOUT,
            },
        );
        let packet = OutboundWorldPacket {
            opcode: SMSG_LOOT_START_ROLL,
            body: build_loot_start_roll_body(loot_guid, loot_slot, &loot),
        };
        Some(LootRollStart {
            packets: voters
                .into_iter()
                .map(|guid| (guid, packet.clone()))
                .collect(),
        })
    }

    pub(in crate::world) async fn record_loot_roll_vote(
        &self,
        character_guid: u32,
        loot_guid: ObjectGuid,
        loot_slot: u8,
        vote: LootRollVote,
    ) -> Option<LootRollVoteOutcome> {
        let key = (loot_guid.raw(), loot_slot);
        let mut rolls = self.loot_rolls.lock().await;
        let roll = rolls.get_mut(&key)?;
        if !roll.voters.contains(&character_guid) || roll.votes.contains_key(&character_guid) {
            return None;
        }
        let vote_number = roll_number_for_vote(vote);
        let display_number = displayed_roll_number_for_vote(vote);
        let display_vote = displayed_roll_type_for_vote(vote);
        roll.votes.insert(
            character_guid,
            LootRollChoice {
                vote,
                number: vote_number,
            },
        );
        let packets = roll
            .voters
            .iter()
            .map(|guid| {
                (
                    *guid,
                    OutboundWorldPacket {
                        opcode: SMSG_LOOT_ROLL,
                        body: build_loot_roll_body(
                            loot_guid,
                            loot_slot,
                            character_guid,
                            &roll.loot,
                            display_number,
                            display_vote,
                        ),
                    },
                )
            })
            .collect::<Vec<_>>();
        if roll.votes.len() < roll.voters.len() {
            return Some(LootRollVoteOutcome {
                map_id: roll.map_id,
                loot_guid,
                loot_slot,
                winner: None,
                loot: None,
                packets,
            });
        }
        let finished = rolls.remove(&key)?;
        Some(finish_loot_roll(finished, packets))
    }

    pub(in crate::world) async fn expire_loot_rolls(
        &self,
        now: Instant,
    ) -> Vec<LootRollVoteOutcome> {
        let mut rolls = self.loot_rolls.lock().await;
        let expired_keys = rolls
            .iter()
            .filter_map(|(key, roll)| (roll.ends_at <= now).then_some(*key))
            .collect::<Vec<_>>();
        let mut outcomes = Vec::new();
        for key in expired_keys {
            let Some(mut finished) = rolls.remove(&key) else {
                continue;
            };
            let missing_voters = finished
                .voters
                .iter()
                .copied()
                .filter(|guid| !finished.votes.contains_key(guid))
                .collect::<Vec<_>>();
            let mut packets = Vec::new();
            for voter in missing_voters {
                finished.votes.insert(
                    voter,
                    LootRollChoice {
                        vote: LootRollVote::Pass,
                        number: 128,
                    },
                );
                let pass_packet = OutboundWorldPacket {
                    opcode: SMSG_LOOT_ROLL,
                    body: build_loot_roll_body(
                        finished.loot_guid,
                        finished.loot_slot,
                        voter,
                        &finished.loot,
                        128,
                        128,
                    ),
                };
                packets.extend(
                    finished
                        .voters
                        .iter()
                        .map(|guid| (*guid, pass_packet.clone())),
                );
            }
            outcomes.push(finish_loot_roll(finished, packets));
        }
        outcomes
    }

    pub(in crate::world) async fn invite(
        &self,
        inviter: PartyMember,
        invitee: PartyMember,
        invitee_session: SessionId,
    ) -> PartyInviteOutcome {
        if inviter.guid == invitee.guid {
            return PartyInviteOutcome {
                result: PartyResult::BadPlayerName,
                invitee_session: None,
                invite_packet: None,
            };
        }
        if self.membership.lock().await.contains_key(&invitee.guid)
            || self
                .invites_by_invitee
                .lock()
                .await
                .contains_key(&invitee.guid)
        {
            return PartyInviteOutcome {
                result: PartyResult::AlreadyInGroup,
                invitee_session: None,
                invite_packet: None,
            };
        }

        let party_id =
            if let Some(existing) = self.membership.lock().await.get(&inviter.guid).copied() {
                let parties = self.parties.lock().await;
                let Some(party) = parties.get(&existing) else {
                    return PartyInviteOutcome {
                        result: PartyResult::NotInGroup,
                        invitee_session: None,
                        invite_packet: None,
                    };
                };
                if party.leader != inviter.guid {
                    return PartyInviteOutcome {
                        result: PartyResult::NotLeader,
                        invitee_session: None,
                        invite_packet: None,
                    };
                }
                if party.members.len() >= party_capacity(party.raid) {
                    return PartyInviteOutcome {
                        result: PartyResult::GroupFull,
                        invitee_session: None,
                        invite_packet: None,
                    };
                }
                existing
            } else {
                PartyId::next()
            };

        self.invites_by_invitee.lock().await.insert(
            invitee.guid,
            PendingPartyInvite {
                party_id,
                inviter: inviter.clone(),
                invitee,
            },
        );
        PartyInviteOutcome {
            result: PartyResult::Ok,
            invitee_session: Some(invitee_session),
            invite_packet: Some(OutboundWorldPacket {
                opcode: SMSG_GROUP_INVITE,
                body: build_group_invite_body(&inviter.name),
            }),
        }
    }

    pub(in crate::world) async fn accept(&self, invitee_guid: u32) -> PartyMutationOutcome {
        let Some(invite) = self.invites_by_invitee.lock().await.remove(&invitee_guid) else {
            return PartyMutationOutcome {
                result: PartyResult::NotInGroup,
                packets: Vec::new(),
            };
        };
        let mut membership = self.membership.lock().await;
        if membership.contains_key(&invite.invitee.guid) {
            return PartyMutationOutcome {
                result: PartyResult::AlreadyInGroup,
                packets: Vec::new(),
            };
        }
        let mut parties = self.parties.lock().await;
        let party = parties.entry(invite.party_id).or_insert_with(|| Party {
            id: invite.party_id,
            leader: invite.inviter.guid,
            members: vec![invite.inviter.clone()],
            raid: false,
            subgroups: HashMap::from([(invite.inviter.guid, 0)]),
            assistants: HashSet::new(),
            loot_method: 3,
            master_looter: 0,
            loot_threshold: 2,
            next_looter: invite.inviter.guid,
        });
        if party.members.len() >= party_capacity(party.raid) {
            return PartyMutationOutcome {
                result: PartyResult::GroupFull,
                packets: Vec::new(),
            };
        }
        if !party
            .members
            .iter()
            .any(|member| member.guid == invite.invitee.guid)
        {
            party
                .subgroups
                .insert(invite.invitee.guid, first_free_subgroup(&party.subgroups));
            party.members.push(invite.invitee);
        }
        for member in &party.members {
            membership.insert(member.guid, party.id);
        }
        let packets = party_update_packets(party);
        PartyMutationOutcome {
            result: PartyResult::Ok,
            packets,
        }
    }

    pub(in crate::world) async fn decline(
        &self,
        invitee_guid: u32,
    ) -> Option<(u32, OutboundWorldPacket)> {
        let invite = self.invites_by_invitee.lock().await.remove(&invitee_guid)?;
        Some((
            invite.inviter.guid,
            OutboundWorldPacket {
                opcode: SMSG_GROUP_DECLINE,
                body: build_group_invite_body(&invite.invitee.name),
            },
        ))
    }

    pub(in crate::world) async fn leave(&self, character_guid: u32) -> PartyMutationOutcome {
        let Some(party_id) = self.membership.lock().await.get(&character_guid).copied() else {
            return PartyMutationOutcome {
                result: PartyResult::NotInGroup,
                packets: Vec::new(),
            };
        };
        self.remove_member(party_id, character_guid, true).await
    }

    pub(in crate::world) async fn kick(
        &self,
        leader_guid: u32,
        kicked_guid: u32,
    ) -> PartyMutationOutcome {
        let Some(party_id) = self.membership.lock().await.get(&leader_guid).copied() else {
            return PartyMutationOutcome {
                result: PartyResult::NotInGroup,
                packets: Vec::new(),
            };
        };
        let parties = self.parties.lock().await;
        let Some(party) = parties.get(&party_id) else {
            return PartyMutationOutcome {
                result: PartyResult::NotInGroup,
                packets: Vec::new(),
            };
        };
        if party.leader != leader_guid {
            return PartyMutationOutcome {
                result: PartyResult::NotLeader,
                packets: Vec::new(),
            };
        }
        if !party
            .members
            .iter()
            .any(|member| member.guid == kicked_guid)
            || kicked_guid == leader_guid
        {
            return PartyMutationOutcome {
                result: PartyResult::TargetNotInGroup,
                packets: Vec::new(),
            };
        }
        drop(parties);
        self.remove_member(party_id, kicked_guid, false).await
    }

    pub(in crate::world) async fn set_leader(
        &self,
        leader_guid: u32,
        new_leader_guid: u32,
    ) -> PartyMutationOutcome {
        let Some(party_id) = self.membership.lock().await.get(&leader_guid).copied() else {
            return PartyMutationOutcome {
                result: PartyResult::NotInGroup,
                packets: Vec::new(),
            };
        };
        let mut parties = self.parties.lock().await;
        let Some(party) = parties.get_mut(&party_id) else {
            return PartyMutationOutcome {
                result: PartyResult::NotInGroup,
                packets: Vec::new(),
            };
        };
        if party.leader != leader_guid {
            return PartyMutationOutcome {
                result: PartyResult::NotLeader,
                packets: Vec::new(),
            };
        }
        if !party
            .members
            .iter()
            .any(|member| member.guid == new_leader_guid)
        {
            return PartyMutationOutcome {
                result: PartyResult::TargetNotInGroup,
                packets: Vec::new(),
            };
        }
        party.leader = new_leader_guid;
        party.assistants.remove(&new_leader_guid);
        let new_leader_name = party
            .members
            .iter()
            .find(|member| member.guid == new_leader_guid)
            .map(|member| member.name.clone())
            .unwrap_or_default();
        let mut packets = party_notification_packets(
            party,
            SMSG_GROUP_SET_LEADER,
            build_group_set_leader_body(&new_leader_name),
        );
        packets.extend(party_update_packets(party));
        PartyMutationOutcome {
            result: PartyResult::Ok,
            packets,
        }
    }

    pub(in crate::world) async fn convert_to_raid(&self, leader_guid: u32) -> PartyMutationOutcome {
        let Some(party_id) = self.membership.lock().await.get(&leader_guid).copied() else {
            return PartyMutationOutcome {
                result: PartyResult::NotInGroup,
                packets: Vec::new(),
            };
        };
        let mut parties = self.parties.lock().await;
        let Some(party) = parties.get_mut(&party_id) else {
            return PartyMutationOutcome {
                result: PartyResult::NotInGroup,
                packets: Vec::new(),
            };
        };
        if party.leader != leader_guid {
            return PartyMutationOutcome {
                result: PartyResult::NotLeader,
                packets: Vec::new(),
            };
        }
        if party.members.len() < 2 {
            return PartyMutationOutcome {
                result: PartyResult::TargetNotInGroup,
                packets: Vec::new(),
            };
        }
        party.raid = true;
        let packets = party_update_packets(party);
        PartyMutationOutcome {
            result: PartyResult::Ok,
            packets,
        }
    }

    pub(in crate::world) async fn change_subgroup(
        &self,
        actor_guid: u32,
        member_name: &str,
        subgroup: u8,
    ) -> PartyMutationOutcome {
        if subgroup >= 8 {
            return PartyMutationOutcome {
                result: PartyResult::BadPlayerName,
                packets: Vec::new(),
            };
        }
        let Some(party_id) = self.membership.lock().await.get(&actor_guid).copied() else {
            return PartyMutationOutcome {
                result: PartyResult::NotInGroup,
                packets: Vec::new(),
            };
        };
        let mut parties = self.parties.lock().await;
        let Some(party) = parties.get_mut(&party_id) else {
            return PartyMutationOutcome {
                result: PartyResult::NotInGroup,
                packets: Vec::new(),
            };
        };
        if !party.raid {
            return PartyMutationOutcome {
                result: PartyResult::NotInGroup,
                packets: Vec::new(),
            };
        }
        if party.leader != actor_guid && !party.assistants.contains(&actor_guid) {
            return PartyMutationOutcome {
                result: PartyResult::NotLeader,
                packets: Vec::new(),
            };
        }
        if subgroup_count(&party.subgroups, subgroup) >= 5 {
            return PartyMutationOutcome {
                result: PartyResult::GroupFull,
                packets: Vec::new(),
            };
        }
        let Some(member) = party
            .members
            .iter()
            .find(|member| member.name.eq_ignore_ascii_case(member_name))
        else {
            return PartyMutationOutcome {
                result: PartyResult::TargetNotInGroup,
                packets: Vec::new(),
            };
        };
        party.subgroups.insert(member.guid, subgroup);
        let packets = party_update_packets(party);
        PartyMutationOutcome {
            result: PartyResult::Ok,
            packets,
        }
    }

    pub(in crate::world) async fn set_assistant(
        &self,
        leader_guid: u32,
        assistant_guid: u32,
        enabled: bool,
    ) -> PartyMutationOutcome {
        let Some(party_id) = self.membership.lock().await.get(&leader_guid).copied() else {
            return PartyMutationOutcome {
                result: PartyResult::NotInGroup,
                packets: Vec::new(),
            };
        };
        let mut parties = self.parties.lock().await;
        let Some(party) = parties.get_mut(&party_id) else {
            return PartyMutationOutcome {
                result: PartyResult::NotInGroup,
                packets: Vec::new(),
            };
        };
        if party.leader != leader_guid {
            return PartyMutationOutcome {
                result: PartyResult::NotLeader,
                packets: Vec::new(),
            };
        }
        if !party.raid
            || !party
                .members
                .iter()
                .any(|member| member.guid == assistant_guid)
        {
            return PartyMutationOutcome {
                result: PartyResult::TargetNotInGroup,
                packets: Vec::new(),
            };
        }
        if enabled {
            party.assistants.insert(assistant_guid);
        } else {
            party.assistants.remove(&assistant_guid);
        }
        let packets = party_update_packets(party);
        PartyMutationOutcome {
            result: PartyResult::Ok,
            packets,
        }
    }

    pub(in crate::world) async fn set_loot_method(
        &self,
        leader_guid: u32,
        loot_method: u8,
        master_looter: u32,
        loot_threshold: u8,
    ) -> PartyMutationOutcome {
        let Some(party_id) = self.membership.lock().await.get(&leader_guid).copied() else {
            return PartyMutationOutcome {
                result: PartyResult::NotInGroup,
                packets: Vec::new(),
            };
        };
        let mut parties = self.parties.lock().await;
        let Some(party) = parties.get_mut(&party_id) else {
            return PartyMutationOutcome {
                result: PartyResult::NotInGroup,
                packets: Vec::new(),
            };
        };
        if party.leader != leader_guid {
            return PartyMutationOutcome {
                result: PartyResult::NotLeader,
                packets: Vec::new(),
            };
        }
        if loot_method > 3 {
            return PartyMutationOutcome {
                result: PartyResult::BadPlayerName,
                packets: Vec::new(),
            };
        }
        if loot_method == 2
            && !party
                .members
                .iter()
                .any(|member| member.guid == master_looter)
        {
            return PartyMutationOutcome {
                result: PartyResult::TargetNotInGroup,
                packets: Vec::new(),
            };
        }
        party.loot_method = loot_method;
        party.master_looter = if loot_method == 2 { master_looter } else { 0 };
        party.loot_threshold = loot_threshold;
        let packets = party_update_packets(party);
        PartyMutationOutcome {
            result: PartyResult::Ok,
            packets,
        }
    }

    pub(in crate::world) async fn remove_member(
        &self,
        party_id: PartyId,
        character_guid: u32,
        allow_leader_transfer: bool,
    ) -> PartyMutationOutcome {
        let mut membership = self.membership.lock().await;
        let mut parties = self.parties.lock().await;
        let Some(mut party) = parties.remove(&party_id) else {
            return PartyMutationOutcome {
                result: PartyResult::NotInGroup,
                packets: Vec::new(),
            };
        };
        if !party
            .members
            .iter()
            .any(|member| member.guid == character_guid)
        {
            parties.insert(party_id, party);
            return PartyMutationOutcome {
                result: PartyResult::TargetNotInGroup,
                packets: Vec::new(),
            };
        }
        party.members.retain(|member| member.guid != character_guid);
        party.subgroups.remove(&character_guid);
        party.assistants.remove(&character_guid);
        membership.remove(&character_guid);

        let mut packets = Vec::new();
        if !allow_leader_transfer {
            packets.push((
                character_guid,
                OutboundWorldPacket {
                    opcode: SMSG_GROUP_UNINVITE,
                    body: Vec::new(),
                },
            ));
        }
        packets.push((
            character_guid,
            OutboundWorldPacket {
                opcode: SMSG_GROUP_LIST,
                body: build_empty_group_list_body(),
            },
        ));
        if party.members.len() <= 1 {
            for member in &party.members {
                membership.remove(&member.guid);
                packets.push((
                    member.guid,
                    OutboundWorldPacket {
                        opcode: SMSG_GROUP_DESTROYED,
                        body: Vec::new(),
                    },
                ));
                packets.push((
                    member.guid,
                    OutboundWorldPacket {
                        opcode: SMSG_GROUP_LIST,
                        body: build_empty_group_list_body(),
                    },
                ));
            }
            self.loot_rolls
                .lock()
                .await
                .retain(|_, roll| roll.party_id != party_id);
            return PartyMutationOutcome {
                result: PartyResult::Ok,
                packets,
            };
        }
        if party.leader == character_guid && allow_leader_transfer {
            party.leader = party.members[0].guid;
            party.assistants.remove(&party.leader);
            let new_leader_name = party.members[0].name.clone();
            packets.extend(party_notification_packets(
                &party,
                SMSG_GROUP_SET_LEADER,
                build_group_set_leader_body(&new_leader_name),
            ));
        }
        for member in &party.members {
            membership.insert(member.guid, party.id);
        }
        packets.extend(party_update_packets(&party));
        parties.insert(party_id, party);
        PartyMutationOutcome {
            result: PartyResult::Ok,
            packets,
        }
    }
}

pub(in crate::world) fn party_notification_packets(
    party: &Party,
    opcode: u16,
    body: Vec<u8>,
) -> Vec<(u32, OutboundWorldPacket)> {
    party
        .members
        .iter()
        .map(|member| {
            (
                member.guid,
                OutboundWorldPacket {
                    opcode,
                    body: body.clone(),
                },
            )
        })
        .collect()
}

pub(in crate::world) fn party_update_packets(party: &Party) -> Vec<(u32, OutboundWorldPacket)> {
    party
        .members
        .iter()
        .map(|member| {
            (
                member.guid,
                OutboundWorldPacket {
                    opcode: SMSG_GROUP_LIST,
                    body: build_group_list_body(party, member.guid),
                },
            )
        })
        .collect()
}

pub(in crate::world) fn build_group_invite_body(name: &str) -> Vec<u8> {
    SmsgGroupInviteResponse {
        name: name.to_string(),
    }
    .body()
}

pub(in crate::world) fn build_group_set_leader_body(name: &str) -> Vec<u8> {
    SmsgGroupSetLeaderResponse {
        name: name.to_string(),
    }
    .body()
}

pub(in crate::world) fn build_party_command_result_body(
    operation: u32,
    member: &str,
    result: PartyResult,
) -> Vec<u8> {
    SmsgPartyCommandResultResponse {
        operation,
        member: member.to_string(),
        result: result.code(),
    }
    .body()
}

pub(in crate::world) fn build_empty_group_list_body() -> Vec<u8> {
    SmsgEmptyGroupListResponse.body()
}

pub(in crate::world) fn build_group_list_body(party: &Party, receiver_guid: u32) -> Vec<u8> {
    let receiver = party
        .members
        .iter()
        .position(|member| member.guid == receiver_guid)
        .unwrap_or(0);
    SmsgGroupListResponse {
        raid: party.raid,
        receiver_group_flags: member_group_flags(party, receiver_guid).unwrap_or(receiver as u8),
        members: party
            .members
            .iter()
            .filter(|member| member.guid != receiver_guid)
            .map(|member| GroupListMemberResponse {
                name: member.name.clone(),
                guid: ObjectGuid::new(HighGuid::Player, 0, member.guid),
                online: 1,
                group_flags: member_group_flags(party, member.guid).unwrap_or(0),
            })
            .collect(),
        leader: ObjectGuid::new(HighGuid::Player, 0, party.leader),
        loot_method: (party.members.len() > 1).then_some(party.loot_method),
        master_looter: ObjectGuid::new(HighGuid::Player, 0, party.master_looter),
        loot_threshold: party.loot_threshold,
    }
    .body()
}

pub(in crate::world) fn member_group_flags(party: &Party, character_guid: u32) -> Option<u8> {
    let subgroup = *party.subgroups.get(&character_guid)?;
    Some(
        subgroup
            | if party.assistants.contains(&character_guid) {
                0x80
            } else {
                0
            },
    )
}

pub(in crate::world) fn party_capacity(raid: bool) -> usize {
    if raid {
        40
    } else {
        5
    }
}

pub(in crate::world) fn first_free_subgroup(subgroups: &HashMap<u32, u8>) -> u8 {
    (0..8)
        .find(|subgroup| subgroup_count(subgroups, *subgroup) < 5)
        .unwrap_or(0)
}

pub(in crate::world) fn subgroup_count(subgroups: &HashMap<u32, u8>, subgroup: u8) -> usize {
    subgroups
        .values()
        .filter(|&&value| value == subgroup)
        .count()
}

pub(in crate::world) async fn dispatch_party_member_packets(
    sessions: &SessionRegistry,
    packets: Vec<(u32, OutboundWorldPacket)>,
) {
    for (character_guid, packet) in packets {
        if let Some(session_id) = sessions.session_for_character(character_guid).await {
            sessions.send_packet(session_id, packet).await;
        }
    }
}

pub(in crate::world) async fn handle_group_invite(
    stream: &mut WorldPacketSink,
    parties: &PartyManager,
    sessions: &SessionRegistry,
    request: wow_proto::GroupInviteRequest,
    session: &WorldSessionState,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    let member_name = request.member_name;
    let Some(character) = session.character.active_character.as_ref() else {
        return Ok(());
    };
    let Some((invitee_guid, invitee_name, invitee_session)) =
        sessions.online_character_by_name(&member_name).await
    else {
        send_packet(
            stream,
            SMSG_PARTY_COMMAND_RESULT,
            &build_party_command_result_body(0, &member_name, PartyResult::BadPlayerName),
            Some(header_crypto),
        )
        .await?;
        return Ok(());
    };
    let outcome = parties
        .invite(
            PartyMember {
                guid: character.guid,
                name: character.name.clone(),
            },
            PartyMember {
                guid: invitee_guid,
                name: invitee_name,
            },
            invitee_session,
        )
        .await;
    if let (Some(session_id), Some(packet)) = (outcome.invitee_session, outcome.invite_packet) {
        sessions.send_packet(session_id, packet).await;
    }
    send_packet(
        stream,
        SMSG_PARTY_COMMAND_RESULT,
        &build_party_command_result_body(0, &member_name, outcome.result),
        Some(header_crypto),
    )
    .await
}

pub(in crate::world) async fn handle_group_accept(
    stream: &mut WorldPacketSink,
    parties: &PartyManager,
    sessions: &SessionRegistry,
    session: &WorldSessionState,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    let Some(character) = session.character.active_character.as_ref() else {
        return Ok(());
    };
    let outcome = parties.accept(character.guid).await;
    dispatch_party_member_packets(sessions, outcome.packets).await;
    if outcome.result != PartyResult::Ok {
        send_packet(
            stream,
            SMSG_PARTY_COMMAND_RESULT,
            &build_party_command_result_body(0, "", outcome.result),
            Some(header_crypto),
        )
        .await?;
    }
    Ok(())
}

pub(in crate::world) async fn handle_group_decline(
    parties: &PartyManager,
    sessions: &SessionRegistry,
    session: &WorldSessionState,
) -> anyhow::Result<()> {
    let Some(character) = session.character.active_character.as_ref() else {
        return Ok(());
    };
    if let Some((leader_guid, packet)) = parties.decline(character.guid).await {
        if let Some(session_id) = sessions.session_for_character(leader_guid).await {
            sessions.send_packet(session_id, packet).await;
        }
    }
    Ok(())
}

pub(in crate::world) async fn handle_group_disband(
    stream: &mut WorldPacketSink,
    parties: &PartyManager,
    sessions: &SessionRegistry,
    session: &WorldSessionState,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    let Some(character) = session.character.active_character.as_ref() else {
        return Ok(());
    };
    let outcome = parties.leave(character.guid).await;
    dispatch_party_member_packets(sessions, outcome.packets).await;
    send_packet(
        stream,
        SMSG_PARTY_COMMAND_RESULT,
        &build_party_command_result_body(2, &character.name, outcome.result),
        Some(header_crypto),
    )
    .await
}

pub(in crate::world) async fn handle_group_uninvite(
    stream: &mut WorldPacketSink,
    parties: &PartyManager,
    sessions: &SessionRegistry,
    request: wow_proto::GroupUninviteRequest,
    session: &WorldSessionState,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    let member_name = request.member_name;
    let Some(character) = session.character.active_character.as_ref() else {
        return Ok(());
    };
    let kicked_guid = parties
        .party_members(character.guid)
        .await
        .into_iter()
        .find(|member| member.name.eq_ignore_ascii_case(&member_name))
        .map(|member| member.guid);
    let outcome = if let Some(kicked_guid) = kicked_guid {
        parties.kick(character.guid, kicked_guid).await
    } else {
        PartyMutationOutcome {
            result: PartyResult::TargetNotInGroup,
            packets: Vec::new(),
        }
    };
    dispatch_party_member_packets(sessions, outcome.packets).await;
    send_packet(
        stream,
        SMSG_PARTY_COMMAND_RESULT,
        &build_party_command_result_body(2, &member_name, outcome.result),
        Some(header_crypto),
    )
    .await
}

pub(in crate::world) async fn handle_group_uninvite_guid(
    stream: &mut WorldPacketSink,
    parties: &PartyManager,
    sessions: &SessionRegistry,
    request: wow_proto::GroupUninviteGuidRequest,
    session: &WorldSessionState,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    let kicked = ObjectGuid::from_raw(request.raw_guid);
    let Some(character) = session.character.active_character.as_ref() else {
        return Ok(());
    };
    let outcome = parties.kick(character.guid, kicked.counter()).await;
    dispatch_party_member_packets(sessions, outcome.packets).await;
    send_packet(
        stream,
        SMSG_PARTY_COMMAND_RESULT,
        &build_party_command_result_body(2, "", outcome.result),
        Some(header_crypto),
    )
    .await
}

pub(in crate::world) async fn handle_group_set_leader(
    parties: &PartyManager,
    sessions: &SessionRegistry,
    request: wow_proto::GroupSetLeaderRequest,
    session: &WorldSessionState,
) -> anyhow::Result<()> {
    let new_leader = ObjectGuid::from_raw(request.raw_guid);
    let Some(character) = session.character.active_character.as_ref() else {
        return Ok(());
    };
    let outcome = parties
        .set_leader(character.guid, new_leader.counter())
        .await;
    dispatch_party_member_packets(sessions, outcome.packets).await;
    Ok(())
}

pub(in crate::world) async fn handle_group_raid_convert(
    stream: &mut WorldPacketSink,
    parties: &PartyManager,
    sessions: &SessionRegistry,
    session: &WorldSessionState,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    let Some(character) = session.character.active_character.as_ref() else {
        return Ok(());
    };
    let outcome = parties.convert_to_raid(character.guid).await;
    if outcome.result == PartyResult::Ok {
        send_packet(
            stream,
            SMSG_PARTY_COMMAND_RESULT,
            &build_party_command_result_body(0, "", PartyResult::Ok),
            Some(&mut *header_crypto),
        )
        .await?;
    }
    dispatch_party_member_packets(sessions, outcome.packets).await;
    Ok(())
}

pub(in crate::world) async fn handle_group_change_subgroup(
    parties: &PartyManager,
    sessions: &SessionRegistry,
    request: wow_proto::GroupChangeSubGroupRequest,
    session: &WorldSessionState,
) -> anyhow::Result<()> {
    let Some(character) = session.character.active_character.as_ref() else {
        return Ok(());
    };
    let outcome = parties
        .change_subgroup(character.guid, &request.member_name, request.subgroup)
        .await;
    dispatch_party_member_packets(sessions, outcome.packets).await;
    Ok(())
}

pub(in crate::world) async fn handle_group_assistant_leader(
    parties: &PartyManager,
    sessions: &SessionRegistry,
    request: wow_proto::GroupAssistantLeaderRequest,
    session: &WorldSessionState,
) -> anyhow::Result<()> {
    let assistant = ObjectGuid::from_raw(request.raw_guid);
    let Some(character) = session.character.active_character.as_ref() else {
        return Ok(());
    };
    let outcome = parties
        .set_assistant(character.guid, assistant.counter(), request.enabled)
        .await;
    dispatch_party_member_packets(sessions, outcome.packets).await;
    Ok(())
}

pub(in crate::world) async fn handle_loot_method(
    parties: &PartyManager,
    sessions: &SessionRegistry,
    request: wow_proto::LootMethodRequest,
    session: &WorldSessionState,
) -> anyhow::Result<()> {
    let master_looter = ObjectGuid::from_raw(request.master_looter_raw_guid);
    let Some(character) = session.character.active_character.as_ref() else {
        return Ok(());
    };
    let outcome = parties
        .set_loot_method(
            character.guid,
            request.loot_method as u8,
            master_looter.counter(),
            request.loot_threshold as u8,
        )
        .await;
    dispatch_party_member_packets(sessions, outcome.packets).await;
    Ok(())
}

pub(in crate::world) async fn handle_request_party_member_stats(
    stream: &mut WorldPacketSink,
    maps: &Arc<MapRuntimeManager>,
    request: wow_proto::RequestPartyMemberStatsRequest,
    session: &WorldSessionState,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    let requested = ObjectGuid::from_raw(request.raw_guid);
    let Some(character) = session.character.active_character.as_ref() else {
        return Ok(());
    };
    let snapshot = maps
        .player_runtime_snapshot(character.position.map_id, requested.counter())
        .await;
    let body = build_party_member_stats_full_body(requested, snapshot.as_ref())?;
    send_packet(
        stream,
        SMSG_PARTY_MEMBER_STATS_FULL,
        &body,
        Some(header_crypto),
    )
    .await
}

pub(in crate::world) async fn handle_loot_roll(
    stream: &mut WorldPacketSink,
    deps: LootMutationDeps<'_>,
    request: wow_proto::LootRollRequest,
    session: &mut WorldSessionState,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    let LootMutationDeps {
        shared_world,
        parties,
        ..
    } = deps;
    let loot_guid = ObjectGuid::from_raw(request.loot_raw_guid);
    let loot_slot = request.loot_slot;
    let Some(vote) = loot_roll_vote_from_client(request.vote) else {
        return Ok(());
    };
    let Some(character) = session.character.active_character.as_ref() else {
        return Ok(());
    };
    if let Some(outcome) = parties
        .record_loot_roll_vote(character.guid, loot_guid, loot_slot, vote)
        .await
    {
        dispatch_party_member_packets(shared_world.sessions, outcome.packets.clone()).await;
        resolve_loot_roll_outcome(stream, deps, outcome, session, header_crypto).await?;
    }
    Ok(())
}

pub(in crate::world) async fn handle_loot_roll_timeouts(
    stream: &mut WorldPacketSink,
    deps: LootMutationDeps<'_>,
    session: &mut WorldSessionState,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    let outcomes = deps.parties.expire_loot_rolls(Instant::now()).await;
    for outcome in outcomes {
        dispatch_party_member_packets(deps.shared_world.sessions, outcome.packets.clone()).await;
        resolve_loot_roll_outcome(stream, deps, outcome, session, header_crypto).await?;
    }
    Ok(())
}

pub(in crate::world) const MEMBER_STATUS_OFFLINE: u8 = 0;
pub(in crate::world) const MEMBER_STATUS_ONLINE: u8 = 1;
pub(in crate::world) const GROUP_UPDATE_FLAG_STATUS: u32 = 0x00000001;
pub(in crate::world) const GROUP_UPDATE_FULL_NO_PET: u32 = 0x000007FF;

pub(in crate::world) fn build_party_member_stats_full_body(
    requested: ObjectGuid,
    snapshot: Option<&PlayerRuntimeSnapshot>,
) -> anyhow::Result<Vec<u8>> {
    let Some(snapshot) = snapshot else {
        return Ok(PartyMemberStatsResponse {
            requested,
            update_flags: GROUP_UPDATE_FLAG_STATUS,
            status: MEMBER_STATUS_OFFLINE,
            health: None,
            max_health: None,
            power_type: None,
            power: None,
            max_power: None,
            level: None,
            map: None,
            x: None,
            y: None,
            aura_mask: None,
            pet_guid: None,
        }
        .body());
    };
    let power_type = group_power_type(snapshot.class);
    let (power, max_power) = match power_type {
        1 => (snapshot.power2, POWER_RAGE_DEFAULT),
        3 => (POWER_ENERGY_DEFAULT, POWER_ENERGY_DEFAULT),
        _ => (snapshot.power1, snapshot.max_power1),
    };
    Ok(PartyMemberStatsResponse {
        requested,
        update_flags: GROUP_UPDATE_FULL_NO_PET,
        status: MEMBER_STATUS_ONLINE,
        health: Some(snapshot.health.min(u16::MAX as u32) as u16),
        max_health: Some(snapshot.max_health.min(u16::MAX as u32) as u16),
        power_type: Some(power_type),
        power: Some(power.min(u16::MAX as u32) as u16),
        max_power: Some(max_power.min(u16::MAX as u32) as u16),
        level: Some(snapshot.level as u16),
        map: Some(snapshot.position.map_id.min(u16::MAX as u32) as u16),
        x: Some(snapshot.position.x.max(0.0).min(u16::MAX as f32) as u16),
        y: Some(snapshot.position.y.max(0.0).min(u16::MAX as f32) as u16),
        aura_mask: Some(0),
        pet_guid: Some(0),
    }
    .body())
}

pub(in crate::world) fn group_power_type(class: u8) -> u8 {
    match class {
        1 => POWER_RAGE,
        4 => POWER_ENERGY,
        _ => POWER_MANA,
    }
}

pub(in crate::world) fn build_loot_start_roll_body(
    loot_guid: ObjectGuid,
    loot_slot: u8,
    loot: &DbCreatureLootRuntime,
) -> Vec<u8> {
    SmsgLootStartRollResponse {
        item: loot_roll_item_response(loot_guid, loot_slot, loot),
        roll_time_millis: 60_000,
        vote_mask: 0x0F,
    }
    .body()
}

pub(in crate::world) fn build_loot_roll_body(
    loot_guid: ObjectGuid,
    loot_slot: u8,
    character_guid: u32,
    loot: &DbCreatureLootRuntime,
    roll_number: u8,
    roll_type: u8,
) -> Vec<u8> {
    SmsgLootRollResponse {
        item: loot_roll_item_response(loot_guid, loot_slot, loot),
        roller: ObjectGuid::new(HighGuid::Player, 0, character_guid),
        roll_number,
        roll_type,
        auto_pass: 0,
    }
    .body()
}

pub(in crate::world) fn build_loot_roll_won_body(
    loot_guid: ObjectGuid,
    loot_slot: u8,
    loot: &DbCreatureLootRuntime,
    winner_guid: u32,
    roll_number: u8,
    vote: LootRollVote,
) -> Vec<u8> {
    SmsgLootRollWonResponse {
        item: loot_roll_item_response(loot_guid, loot_slot, loot),
        winner: ObjectGuid::new(HighGuid::Player, 0, winner_guid),
        roll_number,
        vote: vote as u8,
    }
    .body()
}

pub(in crate::world) fn build_loot_all_passed_body(
    loot_guid: ObjectGuid,
    loot_slot: u8,
    loot: &DbCreatureLootRuntime,
) -> Vec<u8> {
    SmsgLootAllPassedResponse {
        item: loot_roll_item_response(loot_guid, loot_slot, loot),
    }
    .body()
}

pub(in crate::world) fn loot_roll_item_response(
    loot_guid: ObjectGuid,
    loot_slot: u8,
    loot: &DbCreatureLootRuntime,
) -> LootRollItemResponse {
    LootRollItemResponse {
        loot_guid,
        loot_slot,
        item: loot.item,
        random_suffix: 0,
        random_property: 0,
    }
}

pub(in crate::world) fn finish_loot_roll(
    finished: LootRollState,
    mut packets: Vec<(u32, OutboundWorldPacket)>,
) -> LootRollVoteOutcome {
    let winner = select_loot_roll_winner(&finished);
    if let Some((winner_guid, winner_vote, winner_number)) = winner {
        packets.extend(final_loot_roll_packets(&finished, winner_vote));
        let won_packet = OutboundWorldPacket {
            opcode: SMSG_LOOT_ROLL_WON,
            body: build_loot_roll_won_body(
                finished.loot_guid,
                finished.loot_slot,
                &finished.loot,
                winner_guid,
                winner_number,
                winner_vote,
            ),
        };
        packets.extend(
            finished
                .voters
                .iter()
                .map(|guid| (*guid, won_packet.clone())),
        );
        LootRollVoteOutcome {
            map_id: finished.map_id,
            loot_guid: finished.loot_guid,
            loot_slot: finished.loot_slot,
            winner: Some(winner_guid),
            loot: Some(finished.loot),
            packets,
        }
    } else {
        let passed_packet = OutboundWorldPacket {
            opcode: SMSG_LOOT_ALL_PASSED,
            body: build_loot_all_passed_body(
                finished.loot_guid,
                finished.loot_slot,
                &finished.loot,
            ),
        };
        packets.extend(
            finished
                .voters
                .iter()
                .map(|guid| (*guid, passed_packet.clone())),
        );
        LootRollVoteOutcome {
            map_id: finished.map_id,
            loot_guid: finished.loot_guid,
            loot_slot: finished.loot_slot,
            winner: None,
            loot: Some(finished.loot),
            packets,
        }
    }
}

pub(in crate::world) fn final_loot_roll_packets(
    finished: &LootRollState,
    winner_vote: LootRollVote,
) -> Vec<(u32, OutboundWorldPacket)> {
    let mut packets = Vec::new();
    for (voter, choice) in &finished.votes {
        match choice.vote {
            LootRollVote::Pass => continue,
            LootRollVote::Greed if winner_vote == LootRollVote::Need => continue,
            LootRollVote::Need | LootRollVote::Greed => {}
        }
        let roll_packet = OutboundWorldPacket {
            opcode: SMSG_LOOT_ROLL,
            body: build_loot_roll_body(
                finished.loot_guid,
                finished.loot_slot,
                *voter,
                &finished.loot,
                choice.number,
                choice.vote as u8,
            ),
        };
        packets.extend(
            finished
                .voters
                .iter()
                .map(|guid| (*guid, roll_packet.clone())),
        );
    }
    packets
}

pub(in crate::world) fn select_loot_roll_winner(
    roll: &LootRollState,
) -> Option<(u32, LootRollVote, u8)> {
    [LootRollVote::Need, LootRollVote::Greed]
        .into_iter()
        .filter_map(|vote| {
            roll.votes
                .iter()
                .filter(|(_, choice)| choice.vote == vote)
                .map(|(guid, choice)| (*guid, vote, choice.number))
                .max_by_key(|(_, _, roll_number)| *roll_number)
        })
        .next()
}

pub(in crate::world) fn roll_number_for_vote(vote: LootRollVote) -> u8 {
    match vote {
        LootRollVote::Pass => 128,
        LootRollVote::Need | LootRollVote::Greed => rand::thread_rng().gen_range(1..=100),
    }
}

pub(in crate::world) fn displayed_roll_number_for_vote(vote: LootRollVote) -> u8 {
    match vote {
        LootRollVote::Pass => 128,
        LootRollVote::Need => 0,
        LootRollVote::Greed => 128,
    }
}

pub(in crate::world) fn displayed_roll_type_for_vote(vote: LootRollVote) -> u8 {
    match vote {
        LootRollVote::Pass => 128,
        LootRollVote::Need => 0,
        LootRollVote::Greed => 2,
    }
}

pub(in crate::world) fn loot_roll_vote_from_client(value: u8) -> Option<LootRollVote> {
    match value {
        0 => Some(LootRollVote::Pass),
        1 => Some(LootRollVote::Need),
        2 | 3 => Some(LootRollVote::Greed),
        _ => None,
    }
}
