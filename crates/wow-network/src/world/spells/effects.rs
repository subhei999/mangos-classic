use super::*;

pub(in crate::world) async fn spell_cone_radians_for_spell(
    deps: SpellCastDeps<'_>,
    spell_id: u32,
) -> anyhow::Result<f32> {
    if deps.shared_world.maps.has_spell_cone(spell_id) {
        return Ok(deps.shared_world.maps.spell_cone_radians(spell_id));
    }
    let cone_spell_id = deps
        .shared_world
        .object_mgr
        .spell_chain(deps.world_db_pool, spell_id)
        .await?
        .map(spell_chain_root)
        .unwrap_or(spell_id);
    Ok(deps.shared_world.maps.spell_cone_radians(cone_spell_id))
}

mod areas;
mod auras;
mod coverage;
mod damage;
mod dispatch;
mod healing;
mod interrupts;
mod items;
mod movement;
mod utility;

pub(in crate::world) use self::areas::*;
pub(in crate::world) use self::auras::*;
pub(in crate::world) use self::coverage::*;
pub(in crate::world) use self::damage::*;
pub(in crate::world) use self::dispatch::*;
pub(in crate::world) use self::healing::*;
pub(in crate::world) use self::interrupts::*;
pub(in crate::world) use self::items::*;
pub(in crate::world) use self::movement::*;
pub(in crate::world) use self::utility::*;
