use super::*;

#[derive(Clone)]
pub(in crate::world) struct WorldRuntimeState {
    pub(in crate::world) online_characters: OnlineCharacters,
    pub(in crate::world) delete_options: CharacterDeleteOptions,
    pub(in crate::world) character_db_pool: MySqlPool,
    pub(in crate::world) world_db_pool: MySqlPool,
    pub(in crate::world) world_data_files: Arc<WorldDataFiles>,
    pub(in crate::world) world_tick_interval: Duration,
    pub(in crate::world) auction_config: AuctionRuntimeConfig,
    pub(in crate::world) game_event_schedules: Arc<Vec<wow_db::GameEventScheduleQuery>>,
    pub(in crate::world) sessions: Arc<SessionRegistry>,
    pub(in crate::world) maps: Arc<MapRuntimeManager>,
    pub(in crate::world) parties: Arc<PartyManager>,
    pub(in crate::world) object_mgr: Arc<ObjectMgr>,
    pub(in crate::world) playerbots: Arc<PlayerbotRoster>,
    pub(in crate::world) vendor_stock: VendorStockState,
}

#[derive(Clone, Copy)]
pub(in crate::world) struct SharedWorldDeps<'a> {
    pub(in crate::world) object_mgr: &'a ObjectMgr,
    pub(in crate::world) maps: &'a Arc<MapRuntimeManager>,
    pub(in crate::world) sessions: &'a Arc<SessionRegistry>,
}
