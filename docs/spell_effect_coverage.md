# Spell Effect Coverage

CMaNGOS treats spells as data interpreted by generic effect and aura handlers.
Rust should follow that shape: do not special-case individual spell IDs unless
CMaNGOS uses a script hook or special spell-family rule.

## Current Coverage Model

`crates/wow-network/src/world/spells/effects.rs` owns the Rust coverage
registry:

- `CMANGOS_MAX_SPELL_EFFECTS = 130`
- `CMANGOS_TOTAL_AURAS = 192`
- `spell_effect_support(effect_id)`
- `spell_aura_support(aura_type)`
- `spell_template_coverage_issues(template)`
- `spell_coverage_issues_for_spell_ids(object_mgr, world_db_pool, spell_ids)`

Each known CMaNGOS effect or aura ID must be classified as one of:

- `Implemented`: generic Rust runtime behavior exists.
- `KnownNoOp`: CMaNGOS treats it as empty, unused, or obsolete for Classic data.
- `Pending`: the ID is known but needs a real subsystem or handler.
- `Unknown`: outside the CMaNGOS Classic ID range.

Focused tests enforce that all CMaNGOS Classic IDs are classified, and that the
starter warrior spell fixture set is covered.

## Development Procedure

Before implementing a new class, creature spell list, quest item spell, game
object spell, or item-use spell:

1. Build the reachable spell ID list from DB/DBC/source.
2. Run the Rust coverage audit for those spell IDs.
3. Group pending mechanics by generic effect/aura handler, not by spell ID.
4. Implement the smallest CMaNGOS-shaped generic handler and add a focused test
   using a real or source-shaped spell template.
5. Move the effect or aura from `Pending` to `Implemented` only when the runtime
   behavior, packet/update output, and ownership boundary are covered.

## Guardrails

- Do not invent gameplay values. Use DB, DBC, or CMaNGOS formulas.
- Do not mark a mechanic `Implemented` because one spell appears to work if the
  generic effect or aura is still missing important behavior.
- Use `KnownNoOp` only when CMaNGOS has the effect/aura as empty, unused, or
  obsolete for Classic data.
- If a mechanic needs a missing owner system, keep it `Pending("subsystem")`
  and implement that owner first.
