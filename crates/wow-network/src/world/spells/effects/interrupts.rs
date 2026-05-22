use super::*;

pub(in crate::world) async fn apply_player_interrupt_cast_effect(
    deps: SpellCastDeps<'_>,
    map_id: u32,
    spell_template: &wow_db::SpellTemplateQuery,
    targets: &SpellCastTargets,
    now: Instant,
) -> anyhow::Result<()> {
    let Some(target) = targets.unit_target else {
        return Ok(());
    };
    if target.is_creature() {
        let school_lockout_duration = deps
            .shared_world
            .maps
            .spell_duration(spell_template.duration_index)
            .and_then(|duration| {
                (duration.duration_millis > 0)
                    .then(|| Duration::from_millis(duration.duration_millis as u64))
            });
        if let Some(event) = deps
            .shared_world
            .maps
            .interrupt_db_creature_spell_cast(
                map_id,
                target,
                SPELL_FAILED_INTERRUPTED,
                school_lockout_duration,
                now,
            )
            .await?
        {
            deps.shared_world
                .sessions
                .dispatch(event.observer_packets)
                .await;
        }
    }
    Ok(())
}
