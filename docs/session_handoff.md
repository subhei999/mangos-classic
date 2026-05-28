# Session Handoff

Short operating brief for the next Rust gameplay-parity session. Keep this file
concise; durable audit state belongs in `docs/spell_class_audit.md`.

## Current Branch And State

- Branch: `codex/rusty-mangos`
- Workspace: `C:\Users\subhe\Documents\New project`
- Current state: dirty worktree on top of `dffa155b4` with unrelated creature
  chase / Heroic Strike edits plus in-flight Warlock `Hellfire` spell-audit
  work. Do not revert unrelated dirty files.
- Local branch is ahead of `origin/codex/rusty-mangos` by the launcher hygiene
  and gameplay audit consolidation commits.
- Use `target-codex/` for focused cargo runs in this workspace when needed; it
  is ignored.

## Current Goal

Automation spell-audit priority: continue the Warlock non-talent scan and stop
at the first real generic blocker, which is still `Hellfire`.

- Gate/subsystem: generic spell planning, channel runtime, and area/trigger
  ownership for Warlock spells.
- CMaNGOS reference:
  `src/game/Spells/Spell.cpp`,
  `src/game/Spells/SpellAuras.cpp`,
  `src/game/Spells/Scripts/Scripting/ClassScripts/Warlock.cpp`,
  plus local live spell rows / `spell_chain` / `spell_bonus_data`.

## What Changed Recently

- Warlock audit is closed through `Unending Breath`; `docs/spell_class_audit.md`
  remains the durable tracker.
- `Hellfire` rank 1 was reclassified from a direct persistent-area spell to a
  self `SPELL_AURA_PERIODIC_TRIGGER_SPELL` wrapper: live row `1949` ticks
  trigger spell `5857` every 1000 ms.
- A useful generic fix landed underneath the investigation:
  caster-centered persistent-area effects can now derive their origin from the
  caster position instead of requiring a client destination.
- Focused synthetic proof for that persistent-area origin lane is green, but it
  does not close live `Hellfire`; the remaining blocker is the wrapper-owned
  self periodic-trigger hostile-AoE channel path.

## Tests Run

- `cargo test -p wow-network hellfire_uses_caster_centered_persistent_area_profile -- --nocapture`
  - passed
- `cargo test -p wow-network hellfire_creates_caster_centered_channel_dynamic_object_and_ticks_area_damage -- --nocapture`
  - passed
- `cargo test -p wow-network blizzard_creates_channel_dynamic_object_and_ticks_area_damage -- --nocapture`
  - passed
- `cargo test -p wow-network arcane_missiles_live_rank_one_rows_use_generic_periodic_trigger_channel_and_hostile_missile -- --nocapture`
  - passed
- `cargo test -p wow-network arcane_missiles_without_selected_target_fails_before_spending_mana -- --nocapture`
  - passed

## Known Blockers / Unproven Areas

- Fresh `cargo test` rebuilds are currently blocked by unrelated dirty-tree
  compile errors outside the spell audit, including missing chase symbol
  renames and one stale `DbCreatureChaseTarget` field use in tests.
- Live `Hellfire` is still not closed: the missing generic lane is the self
  periodic-trigger hostile-AoE channel path used by wrapper row `1949`.
- `.\scripts\test-rust.cmd` was not rerun after the latest dirty changes.
- The branch has not been pushed.

## Recommended Next Task

- Keep Warlock as the active class and resume at `Hellfire`.
- Implement the smallest generic runtime path for self-targeted
  `SPELL_AURA_PERIODIC_TRIGGER_SPELL` wrappers that tick hostile caster-area
  trigger spells, then rerun the smallest focused spell tests.
- After that lane is green, continue to the next Warlock family in order.

## Key Files

- `crates/wow-network/src/world/spells/effects/areas.rs`
- `crates/wow-network/src/world/map_runtime/map_manager/spells.rs`
- `crates/wow-network/src/world/map_runtime/systems/player_channels.rs`
- `crates/wow-network/src/world/spells/plan.rs`
- `crates/wow-network/src/world/spells/spell_mgr.rs`
- `crates/wow-network/src/world/tests/spells.rs`
- `sql/base/dbc/original_data/Spell.sql`
- `src/game/Spells/Spell.cpp`
- `src/game/Spells/SpellAuras.cpp`
- `src/game/Spells/Scripts/Scripting/ClassScripts/Warlock.cpp`
