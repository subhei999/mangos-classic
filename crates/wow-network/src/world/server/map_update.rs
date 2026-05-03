// CMaNGOS reference: src/game/Maps/Map.cpp map-owned object update loop.

async fn run_map_runtime_update_loop(runtime_state: WorldRuntimeState) {
    let navigation = DbCreatureNavigationGuardrail {
        world_data_files: runtime_state.world_data_files.clone(),
        ..DbCreatureNavigationGuardrail::default()
    };
    let tick_budget = runtime_state.world_tick_interval;
    let mut ticker = tokio::time::interval(tick_budget);
    ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    let mut next_tick_at = Instant::now();

    loop {
        ticker.tick().await;
        let now = Instant::now();
        let tick_started_at = now;
        let tick_lag = now.saturating_duration_since(next_tick_at);
        let phase_started_at = Instant::now();
        match runtime_state
            .maps
            .advance_all_active_db_creature_idle_motions_with_interval(
                &navigation,
                now,
                tick_budget,
            )
            .await
        {
            Ok(tick) => {
                crate::observability::record_map_phase_duration(
                    crate::observability::MapTickPhase::IdleMotion,
                    phase_started_at.elapsed(),
                );
                if !tick.packets.is_empty() {
                    let dispatch_started_at = Instant::now();
                    runtime_state.sessions.dispatch(tick.packets).await;
                    crate::observability::record_map_phase_duration(
                        crate::observability::MapTickPhase::IdleMotionDispatch,
                        dispatch_started_at.elapsed(),
                    );
                }
            }
            Err(error) => {
                crate::observability::record_map_phase_duration(
                    crate::observability::MapTickPhase::IdleMotion,
                    phase_started_at.elapsed(),
                );
                crate::observability::record_map_tick_error();
                warn!("Map runtime update tick failed: {error}");
            }
        }
        let phase_started_at = Instant::now();
        match runtime_state.maps.advance_all_player_regen_ticks(now).await {
            Ok(packets) => {
                crate::observability::record_map_phase_duration(
                    crate::observability::MapTickPhase::PlayerRegen,
                    phase_started_at.elapsed(),
                );
                if !packets.is_empty() {
                    let dispatch_started_at = Instant::now();
                    runtime_state.sessions.dispatch(packets).await;
                    crate::observability::record_map_phase_duration(
                        crate::observability::MapTickPhase::PlayerRegenDispatch,
                        dispatch_started_at.elapsed(),
                    );
                }
            }
            Err(error) => {
                crate::observability::record_map_phase_duration(
                    crate::observability::MapTickPhase::PlayerRegen,
                    phase_started_at.elapsed(),
                );
                crate::observability::record_map_tick_error();
                warn!("Map runtime player regen tick failed: {error}");
            }
        }
        let phase_started_at = Instant::now();
        match runtime_state
            .maps
            .advance_all_player_aura_expirations(now)
            .await
        {
            Ok(packets) => {
                crate::observability::record_map_phase_duration(
                    crate::observability::MapTickPhase::AuraExpiration,
                    phase_started_at.elapsed(),
                );
                if !packets.is_empty() {
                    let dispatch_started_at = Instant::now();
                    runtime_state.sessions.dispatch(packets).await;
                    crate::observability::record_map_phase_duration(
                        crate::observability::MapTickPhase::AuraExpirationDispatch,
                        dispatch_started_at.elapsed(),
                    );
                }
            }
            Err(error) => {
                crate::observability::record_map_phase_duration(
                    crate::observability::MapTickPhase::AuraExpiration,
                    phase_started_at.elapsed(),
                );
                crate::observability::record_map_tick_error();
                warn!("Map runtime player aura expiration tick failed: {error}");
            }
        }
        runtime_state.maps.record_observability_snapshots().await;
        crate::observability::record_map_tick(tick_started_at.elapsed(), tick_lag, tick_budget);
        while next_tick_at <= now {
            next_tick_at += tick_budget;
        }
    }
}
