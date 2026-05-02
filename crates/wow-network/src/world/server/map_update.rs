// CMaNGOS reference: src/game/Maps/Map.cpp map-owned object update loop.

async fn run_map_runtime_update_loop(runtime_state: WorldRuntimeState) {
    let navigation = DbCreatureNavigationGuardrail {
        world_data_files: runtime_state.world_data_files.clone(),
        ..DbCreatureNavigationGuardrail::default()
    };
    let mut ticker = tokio::time::interval(Duration::from_millis(WORLD_TICK_MILLIS));
    ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

    loop {
        ticker.tick().await;
        let now = Instant::now();
        match runtime_state
            .maps
            .advance_all_active_db_creature_idle_motions(&navigation, now)
            .await
        {
            Ok(tick) => {
                if !tick.packets.is_empty() {
                    runtime_state.sessions.dispatch(tick.packets).await;
                }
            }
            Err(error) => {
                warn!("Map runtime update tick failed: {error}");
            }
        }
    }
}
