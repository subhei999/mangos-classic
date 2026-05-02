# Session Handoff

Short operating brief for the next Rust migration session. Keep this pruned;
durable roadmap details belong in `docs/rust_migration_plan.md`, gate status in
`docs/playable_gate_board.md`, and branch split details in
`docs/playable_execution_roadmap.md`.

## Current Branch And Worktree

- Branch: `codex/c2-real-client-closure`.
- Purpose: integrate focused Checkpoint 2 real-client parity branches, one at a
  time, then let the user validate against a live 1.12.1 client.
- Latest integrated commits before the current fix:
  - `1de9b747b` (`Merge quest status markers`);
  - `a41449b76` (`Merge combat resource feel`);
  - `8627829d1` (`Merge creature template fidelity`).
- Current uncommitted state: Heroic Strike delayed rage-cost fix plus this
  handoff refresh.
- Re-run `git status --short --branch` before editing.
- Live client stack was rebuilt/restarted after the delayed rage-cost fix:
  - authserver PID `40568` on `127.0.0.1:13724`;
  - worldserver PID `13792` on `127.0.0.1:18085`;
  - logs: `auth-client-13724.log`, `world-client-18085.log`;
  - auto-restart is disabled.

## Current Goal

Current milestone: **Checkpoint 2 Northshire Human Warrior playable slice with
shared multiplayer state**.

Current user direction: **Do not use a Northshire grading harness. The user is
the grader using real-client side-by-side testing. Convert real-client notes
into generalized systems work and priorities; do not hardcode Northshire-only
fixes.**

Important scope rule: stay focused on the current goal, but use judgment. Fix
blockers and safety/data-integrity guardrails when practical. Log useful
follow-ups when they should not be handled immediately.

Gameplay data rule: do not fake or hardcode gameplay values for parity work.
Use DB data, DBC/source-derived values, or CMaNGOS formulas. If the real data
source is not wired yet, leave behavior unimplemented or narrowly guarded and
log the follow-up.

## Recently Landed

- Quest status markers: unavailable quest givers now surface gray quest status
  instead of being invisible to the player.
- Combat resource feel: damage-based rage generation, overkill fields, max-HP
  regen cap sync, and immediate first-swing scheduling are integrated.
- Creature template fidelity: DB-backed walk/run speed, model scale, and
  equipment display fields are integrated for creature create blocks.
- Login-disconnect fix: creature equipment display projections now
  cast `COALESCE(item_template.displayid, 0)` to unsigned integers so MariaDB
  does not return a decimal type that `sqlx` refuses to decode into `u32`.
- Current real-client feedback patch:
  - trainer `Train me` disconnect was caused by an ambiguous `Entry` column in
    the joined creature-template query; the query is now fully
    `creature_template.`-qualified;
  - Heroic Strike/next-melee rage-spending swings no longer award attack rage;
  - manual attack stop/start preserves the map-owned next swing timestamp so
    players cannot reset swing timers by toggling autoattack.
- Current delayed rage-cost patch:
  - next-melee starter spells now validate power at queue time but store the
    cost on the queued swing;
  - Heroic Strike rage is consumed when the queued swing resolves, not when the
    client queues the attack;
  - the queue packet path no longer sends a power update at cast time for
    next-melee spells.

## Still Unmerged Worker Branches

- `codex/c2-loot-multidrop-fidelity`:
  `C:\Users\subhe\Documents\mangos-worktrees\c2-loot-multidrop-fidelity`
- `codex/c2-skills-weapon-progression`:
  `C:\Users\subhe\Documents\mangos-worktrees\c2-skills-weapon-progression`
- `codex/c2-npc-trainer-scripts`:
  `C:\Users\subhe\Documents\mangos-worktrees\c2-npc-trainer-scripts`

Merge these one by one only after inspecting scope, rebuilding, restarting the
client stack, and giving the user concrete real-client success criteria.

## Tests Run

- Before the login-disconnect fix, after the three selected merges:
  - `cargo fmt --check`;
  - `cargo test -p wow-network quest --lib`;
  - `cargo test -p wow-network rage --lib`;
  - `cargo test -p wow-network regen --lib`;
  - `cargo test -p wow-network combat --lib`;
  - `cargo test -p wow-network creature_create --lib`;
  - `cargo test -p wow-network db_creature_random_motion --lib`;
  - `cargo check -p wow-network`;
  - `.\scripts\test-rust.cmd`.
- Current login-disconnect fix:
  - `cargo fmt --check`;
  - `cargo check -p wow-db`;
  - `cargo check -p wow-network`;
  - `cargo test -p wow-network creature_create --lib`;
  - `cargo build -p authserver -p worldserver`;
  - `.\scripts\run-client-stack-18085.cmd -NoAutoRestart`;
  - `Test-NetConnection` passed for `127.0.0.1:13724` and
    `127.0.0.1:18085`;
  - live MariaDB equipment-display join returned numeric display IDs with the
    new cast.
- Current real-client feedback patch:
  - `cargo fmt --check`;
  - `cargo check -p wow-db`;
  - `cargo check -p wow-network`;
  - `cargo test -p wow-network repeated_auto_attack_input_preserves_swing_timer_and_uses_normal_due_tick --lib`;
  - `cargo test -p wow-network heroic_strike_queue_consumes_on_next_swing_only_once --lib`;
  - live MariaDB `get_creature_template_query`-shape query for Lyria Du Lac
    returned successfully with joined equipment rows.
- Current delayed rage-cost patch:
  - `cargo fmt --check`;
  - `cargo check -p wow-network`;
  - `cargo test -p wow-network heroic_strike --lib`;
  - `cargo test -p wow-network starter_spell_cast_failure_rejects_missing_power_gcd_and_duplicate_queue --lib`;
  - `.\scripts\test-rust.cmd`.

## Known Follow-Ups

- Real-client smoke still needs to confirm that Heroic Strike keeps rage while
  queued, consumes rage when the swing lands, does not generate rage from that
  swing, and that prior fixes remain stable: trainer `Train me` no disconnect,
  autoattack stop/start no acceleration.
- User confirmed: Milly quest is no longer incorrectly available at level 2;
  creature speed feels fixed; overkill packet damage is improved but combat log
  still needs explicit `(overkill)` display parity; wolf/drop items and other
  previously fixed items should remain under smoke watch.
- Still observed: gray unavailable quest marker is not visible; creature visual
  scale still does not match; NPCs with weapons can look like they punch while
  holding a sword; CMSG_SETSHEATHED (`0x01E0`) appears frequently and is still
  unhandled.
- Continue user-led side-by-side testing and turn observations into generalized
  system tasks. Current observation list includes NPC work animations, rage
  formula, creature speed/scale/equipment, gray quest markers, overkill, combo
  loot, skills/weapon skills, real loot tables, reputation, quest eligibility,
  first swing timing, creature speech, quest reward items, trainer text/icon,
  training feedback, copper variance, regen caps, and equipped thug weapons.
- Full CMaNGOS loot-table rolling remains tracked as issue #58.
- GitHub issue #62 tracks a starter-zone wrapper readiness race from the old
  grade-report path; the grade surface is removed, but normal starter-zone
  runs can still benefit from a readiness check instead of fixed sleep.

## Key Files

- `crates/wow-db/src/world_data.rs`
- `crates/wow-network/src/world/server/player_login.rs`
- `crates/wow-network/src/world/entities/update_data.rs`
- `crates/wow-network/src/world/combat/lifecycle.rs`
- `crates/wow-network/src/world/combat/melee.rs`
- `crates/wow-network/src/world/maps/map.rs`
- `scripts/run-client-stack-18085.ps1`
- `docs/playable_execution_roadmap.md`
