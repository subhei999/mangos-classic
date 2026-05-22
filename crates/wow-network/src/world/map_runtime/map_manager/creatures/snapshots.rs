use super::*;

impl MapRuntimeManager {
    pub(in crate::world) async fn db_creature_snapshots(
        &self,
        map_id: u32,
        creature_guids: &[u64],
    ) -> Vec<DbCreatureRuntime> {
        let map = self.get_or_create_map(map_id, 0).await;
        let snapshots = map.lock().await.db_creature_snapshots(creature_guids);
        snapshots
    }

    pub(in crate::world) async fn db_creature_snapshot(
        &self,
        map_id: u32,
        creature_guid: ObjectGuid,
    ) -> Option<DbCreatureRuntime> {
        let map = { self.maps.lock().await.get(&(map_id, 0)).cloned() };
        let map = map?;
        let snapshot = map.lock().await.db_creature_snapshot(creature_guid);
        snapshot
    }

    #[allow(dead_code)]
    pub(in crate::world) async fn db_creature_combat_snapshot(
        &self,
        map_id: u32,
        creature_guid: ObjectGuid,
    ) -> Option<DbCreatureRuntime> {
        let map = { self.maps.lock().await.get(&(map_id, 0)).cloned() };
        let map = map?;
        let creature = map.lock().await.db_creature_combat_snapshot(creature_guid);
        creature
    }

    pub(in crate::world) async fn validate_player_melee_against_db_creature(
        &self,
        map_id: u32,
        character_guid: u32,
        target: ObjectGuid,
        navigation: &DbCreatureNavigationGuardrail,
    ) -> DbCreaturePlayerMeleeValidation {
        let map = { self.maps.lock().await.get(&(map_id, 0)).cloned() };
        let Some(map) = map else {
            return DbCreaturePlayerMeleeValidation {
                check: PlayerMeleeCheck::MissingTarget,
            };
        };
        let validation = map.lock().await.validate_player_melee_against_db_creature(
            character_guid,
            target,
            navigation,
        );
        validation
    }

    pub(in crate::world) async fn validate_player_charge_against_db_creature(
        &self,
        map_id: u32,
        character_guid: u32,
        target: ObjectGuid,
        navigation: &DbCreatureNavigationGuardrail,
    ) -> PlayerChargeValidation {
        let map = { self.maps.lock().await.get(&(map_id, 0)).cloned() };
        let Some(map) = map else {
            return PlayerChargeValidation {
                check: PlayerChargeCheck::MissingTarget,
            };
        };
        let validation = map.lock().await.validate_player_charge_against_db_creature(
            character_guid,
            target,
            navigation,
        );
        validation
    }

    pub(in crate::world) async fn validate_player_spell_against_db_creature(
        &self,
        map_id: u32,
        character_guid: u32,
        target: ObjectGuid,
        navigation: &DbCreatureNavigationGuardrail,
        range: Option<SpellRangeEntry>,
        requires_infront: bool,
    ) -> PlayerSpellTargetValidation {
        let map = { self.maps.lock().await.get(&(map_id, 0)).cloned() };
        let Some(map) = map else {
            return PlayerSpellTargetValidation {
                check: PlayerSpellTargetCheck::MissingTarget,
            };
        };
        let validation = map.lock().await.validate_player_spell_against_db_creature(
            &self.faction_templates,
            character_guid,
            target,
            navigation,
            range,
            requires_infront,
        );
        validation
    }

    #[allow(dead_code)]
    pub(in crate::world) async fn validate_db_creature_spell_against_target(
        &self,
        map_id: u32,
        caster: ObjectGuid,
        target: ObjectGuid,
        navigation: &DbCreatureNavigationGuardrail,
        range: Option<SpellRangeEntry>,
        requires_behind: bool,
    ) -> DbCreatureSpellTargetValidation {
        let map = { self.maps.lock().await.get(&(map_id, 0)).cloned() };
        let Some(map) = map else {
            return DbCreatureSpellTargetValidation {
                check: DbCreatureSpellTargetCheck::MissingCaster,
            };
        };
        let validation = map.lock().await.validate_db_creature_spell_against_target(
            caster,
            target,
            navigation,
            range,
            requires_behind,
        );
        validation
    }

    #[allow(dead_code)]
    pub(in crate::world) async fn update_db_creature_snapshot(
        &self,
        map_id: u32,
        creature: DbCreatureRuntime,
    ) {
        let map = self.get_or_create_map(map_id, 0).await;
        map.lock().await.update_db_creature_snapshot(creature);
    }

    pub(in crate::world) async fn update_db_creature_snapshot_and_broadcast(
        &self,
        map_id: u32,
        creature: DbCreatureRuntime,
        exclude_character_guid: Option<u32>,
        packet: OutboundWorldPacket,
    ) -> Vec<(SessionId, OutboundWorldPacket)> {
        let map = self.get_or_create_map(map_id, 0).await;
        let packets = map.lock().await.update_db_creature_snapshot_and_broadcast(
            creature,
            exclude_character_guid,
            packet,
        );
        packets
    }
}
