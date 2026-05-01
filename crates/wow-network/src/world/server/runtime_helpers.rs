// Shared worldserver runtime helpers used by login, maps, and death.

fn current_unix_epoch_secs_u64() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or_default()
}

async fn build_db_creature_runtimes_with_respawns(
    character_db_pool: &MySqlPool,
    spawns: Vec<CreatureSpawnQuery>,
) -> anyhow::Result<Vec<DbCreatureRuntime>> {
    let now = Instant::now();
    let now_epoch_secs = current_unix_epoch_secs_u64();
    let guids = spawns.iter().map(|spawn| spawn.guid).collect::<Vec<_>>();
    let respawn_times =
        wow_db::get_creature_respawn_times(character_db_pool, &guids, 0, now_epoch_secs).await?;
    Ok(spawns
        .into_iter()
        .map(|spawn| {
            let respawn_epoch_secs = respawn_times.get(&spawn.guid).copied();
            DbCreatureRuntime::new_with_persisted_respawn(
                spawn,
                now,
                now_epoch_secs,
                respawn_epoch_secs,
            )
        })
        .collect())
}

fn visible_db_creature_runtimes(creatures: &[DbCreatureRuntime]) -> Vec<DbCreatureRuntime> {
    creatures
        .iter()
        .filter(|creature| creature.life_state != DbCreatureLifeState::Dead)
        .cloned()
        .collect()
}

