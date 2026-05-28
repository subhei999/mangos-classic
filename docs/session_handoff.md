# Session Handoff

Short operating brief for the next Rust gameplay-parity session. Keep this file
concise; durable audit state belongs in `docs/spell_class_audit.md`.

## Current Branch And State

- Branch: `codex/launcher-reputation-hygiene`
- Workspace: `C:\Users\subhe\Documents\New project`
- Current state: dirty worktree with unrelated spell/runtime edits already in
  flight. Do not revert unrelated files.
- Latest local spell-audit change: uncommitted Warlock `Unending Breath`
  closure on top of the earlier `Eye of Kilrogg` summon / possession /
  farsight ownership work.
- Use `target-codex/` for focused cargo runs in this workspace when needed.

## Current Goal

User-directed priority: continue the generic non-talent class-spell audit,
using CMaNGOS Classic as the behavior reference.

- Gate/subsystem: world spell runtime and remaining Warlock generic aura /
  runtime / create-use ownership coverage.
- Current class/family boundary: Warlock is now closed through
  `Unending Breath`; the next in-order family is `Hellfire`.

## What Changed Recently

- Warlock scan advanced past `Summon Voidwalker`, `Searing Pain`, and
  `Create Firestone` without new runtime work. `Summon Voidwalker` closes on
  the existing generic `SPELL_EFFECT_SUMMON_PET` lane already proven by
  `Summon Imp`; `Searing Pain` closes on the existing hostile direct
  spell-damage lane; `Create Firestone` closes on the existing generic
  `SPELL_EFFECT_CREATE_ITEM` lane already proven by Mage conjure-item coverage.
- This pass hit the first real new blocker at `Unending Breath` and closed it
  generically. Live spell `5697` applies aura `82`
  (`SPELL_AURA_WATER_BREATHING`); the Rust runtime now maps that aura into an
  explicit player water-breathing modifier, marks aura `82` implemented in
  spell coverage, and deactivates the drowning mirror timer while the aura is
  active so underwater breath damage stops immediately.

## Tests Run

- `cargo test -p wow-network unending_breath_live_row_uses_generic_water_breathing_aura_path -- --nocapture`
  - passed
- `cargo test -p wow-network map_runtime_water_breathing_aura_stops_underwater_breath_timer -- --nocapture`
  - passed
- `cargo test -p wow-network map_runtime_underwater_breath_timer_applies_drowning_damage_and_log -- --nocapture`
  - passed

## Known Blockers / Unproven Areas

- `.\scripts\test-rust.cmd` was not run this pass; the workspace still has
  unrelated dirty files and a pre-existing warning baseline.
- Local `npc_trainer` / `npc_trainer_template` data remains empty in this dump,
  including several Warlock families, so learn-path confirmation still depends
  on `spell_chain`, live spell rows, and CMaNGOS references.
- `Life Tap` remains script-owned/deferred for this audit pass.
- `Create Healthstone` is also script-owned/deferred for this audit pass.
- `Create Firestone` use-path behavior is still a separate later family
  question only if a future audit needs the created off-hand enchant/item-use
  flow; this pass only closed the generic create-item cast lane.

## Recommended Next Task

- Continue the Warlock audit at `Hellfire`; `Unending Breath` is closed for the
  current generic pass.

## Key Files

- `crates/wow-network/src/world/tests/spells.rs`
- `crates/wow-network/src/world/tests/player_runtime_auras.rs`
- `crates/wow-network/src/world/map_runtime/systems/players.rs`
- `crates/wow-network/src/world/spells.rs`
- `crates/wow-network/src/world/session.rs`
- `crates/wow-network/src/world/spells/definitions.rs`
- `crates/wow-network/src/world/spells/effects/coverage.rs`
- `docs/spell_class_audit.md`
- CMaNGOS reference:
  - `src/game/Spells/SpellAuras.cpp`
  - `src/game/Entities/Player.cpp`
