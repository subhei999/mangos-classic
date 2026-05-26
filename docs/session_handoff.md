# Session Handoff

Short operating brief for the next Rust spell-audit session. Keep this file
concise; durable audit state belongs in `docs/spell_class_audit.md`.

## Current Branch And State

- Branch: `codex/rusty-mangos`
- Workspace: `C:\Users\subhe\Documents\New project`
- Current state: dirty worktree with broad pre-existing gameplay, combat,
  quest, trainer, and spell-system edits. Do not revert unrelated files.
- Use `C:\Users\subhe\Documents\New project\.codex-target` for focused cargo
  runs in this workspace because the default `target/` tree is currently not
  writable for local test builds.

## Current Goal

User-directed priority: continue the generic non-talent class spell audit from
`docs/spell_class_audit.md`.

- Current class: `Priest`
- Next in-order family: `Prayer of Healing`

## What Changed Recently

- Priest `Holy Fire` is now closed as a proof-backed generic family.
- Focused live tests now prove the mixed hostile direct-damage plus periodic
  damage aura lane for `Holy Fire`, using the local spell DB and real DBC
  cast-time/range/duration data.
- The runtime proof confirms base-hit damage, hostile debuff aura application,
  and periodic damage packets against creatures without requiring any
  Priest-family script.

## Tests Run

- `CARGO_TARGET_DIR=C:\Users\subhe\Documents\New project\.codex-target cargo test -p wow-network holy_fire_live_rank_one -- --nocapture`
  - passed
- `CARGO_TARGET_DIR=C:\Users\subhe\Documents\New project\.codex-target cargo test -p wow-network shadow_word_pain_live_rank_one_applies_periodic_damage_to_hostile_creature -- --nocapture`
  - passed

## Known Blockers / Unproven Areas

- Full `test-rust.cmd` was not run in this automation pass; only focused spell
  tests were exercised.
- The workspace remains broadly dirty, so keep changes tightly scoped and read
  nearby diffs before editing shared spell/runtime files.
- Offensive `spell_bonus_data` rows such as `Holy Fire` are still not surfaced
  through live `SpellTemplateQuery.effect_bonus_coefficient*` fields, so
  offensive coefficient loading remains a separate cross-cutting follow-up.
- `Prayer of Healing` is next in order. Confirm the party/group target lane and
  proof status before editing code.

## Recommended Next Task

- Resume the Priest audit at `Prayer of Healing`.
- Confirm the live rank chain plus party/group target backing first, then stop
  at the first missing generic piece for that family.
- Stop after the focused `Prayer of Healing` increment; do not broaden into
  later Priest families in the same run.

## Key Files

- `docs/spell_class_audit.md`
- `crates/wow-network/src/world/tests/spells.rs`
- CMaNGOS reference:
  - `src/game/Spells/Spell.cpp`
  - `src/game/Spells/SpellTargets.cpp`
