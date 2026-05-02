# Session Handoff

Short operating brief for the next Rust migration session. Keep this pruned;
durable roadmap details belong in `docs/rust_migration_plan.md`, gate status in
`docs/playable_gate_board.md`, and branch split details in
`docs/playable_execution_roadmap.md`.

## Current Branch And Worktree

- Branch: `codex/rusty-mangos`.
- Latest local `HEAD`: `8a9188c88` (`Fix Heroic Strike queued spell packets`).
- Worktree was clean after the Heroic Strike packet fix commit; re-run
  `git status --short --branch` before editing.
- Local branch was ahead of `origin/codex/rusty-mangos` pending the end-of-day
  push.
- Live client stack was rebuilt/restarted after the fix:
  - authserver PID `41260` on `127.0.0.1:13724`;
  - worldserver PID `30740` on `127.0.0.1:18085`;
  - logs: `auth-client-13724.log`, `world-client-18085.log`;
  - auto-restart is disabled.

## Current Goal

Current milestone: **Northshire Human Warrior playable slice with shared
multiplayer state**.

Current user direction: **C2 workstream integration has been merged into
`codex/rusty-mangos`; use real-client smoke feedback before taking the next
implementation slice.**

Important scope rule: stay focused on the current goal, but use judgment. Fix
blockers and safety/data-integrity guardrails when practical. Log useful
follow-ups when they should not be handled immediately.

Gameplay data rule: do not fake or hardcode gameplay values for parity work.
Use DB data, DBC/source-derived values, or CMaNGOS formulas. If the real data
source is not wired yet, leave behavior unimplemented or narrowly guarded and
log the follow-up.

## Recently Landed

- Loot, quest eligibility, gameobject quest interaction, fixture NPC removal,
  resurrection-at-ghost-position, auto-attack toggle, regen/rage ticks,
  warrior armor proficiency, CMaNGOS-shaped aggro/leash data, and warrior
  starter spell/GCD work have all been merged into `codex/rusty-mangos`.
- Heroic Strike was corrected after real-client smoke showed it queued but did
  not show as queued on the action bar and appeared as white melee damage.
  CMaNGOS reference showed next-melee spells are cast from
  `Unit::AttackerStateUpdate()` and return before the normal melee
  attacker-state packet.
- Current Rust behavior for supported next-melee starter spells:
  - cast/queue sends `SMSG_CAST_RESULT` and `SMSG_SPELL_START`;
  - `SMSG_SPELL_GO` is delayed until the swing fires;
  - queued swing impact sends `SMSG_SPELL_GO` plus
    `SMSG_SPELLNONMELEEDAMAGELOG`;
  - the queued spell no longer reports as a white
    `SMSG_ATTACKERSTATEUPDATE` hit with a spell id.
- This packet-shape fix applies to the shared `StarterSpellKind::NextMeleeSwing`
  path, with Heroic Strike-specific regression coverage.

## Tests Run

- `cargo fmt --check`
- `cargo test -p wow-network heroic_strike --lib`
- `cargo test -p wow-network starter_spell --lib`
- `cargo test -p wow-network map_runtime_db_creature_spell_damage_includes_combat_log_packet --lib`
- `cargo test -p wow-network --lib`
- `.\scripts\test-rust.cmd`

Note: one `.\scripts\test-rust.cmd` run initially failed because the live
`authserver.exe` binary was locked by the running stack. After stopping
auth/world, the same script passed.

## Known Follow-Ups

- Real-client smoke still needs to confirm whether `SMSG_SPELL_START` makes the
  Heroic Strike action bar show the queued state and whether yellow damage
  appears correctly.
- Heroic Strike currently uses the starter-spell next-swing framework and fixed
  rank data already present in the code. Broader warrior spell parity still
  needs DB/DBC/source-derived spell effects, cooldowns, ranks, and combat log
  details beyond the C2 starter slice.
- Full CMaNGOS loot-table rolling remains tracked as issue #58.
- Keep an eye on C2 smoke regressions around regen/rage ticks, leash feel,
  equipment proficiency, quest item drops, and gameobject quest pickup.

## Key Files

- `crates/wow-network/src/world/spells.rs`
- `crates/wow-network/src/world/combat/lifecycle.rs`
- `crates/wow-network/src/world/maps/map.rs`
- `crates/wow-network/src/world/maps/map/creature_damage.rs`
- `crates/wow-network/src/world/tests.rs`
