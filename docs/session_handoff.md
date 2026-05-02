# Session Handoff

Short operating brief for the next Rust migration session. Keep this pruned;
durable roadmap details belong in `docs/rust_migration_plan.md`, gate status in
`docs/playable_gate_board.md`, and branch split details in
`docs/playable_execution_roadmap.md`.

## Current Branch And Worktree

- Branch: `codex/c2-real-client-closure`, created from local
  `codex/rusty-mangos` after the user rejected the Northshire grading harness
  direction.
- Latest local `HEAD`: `8a9188c88` (`Fix Heroic Strike queued spell packets`).
- Current uncommitted state: completion-tree/docs cleanup plus removal of the
  Northshire grading harness surface:
  - new `docs/wow_completion_tree.md`;
  - new `docs/wow_completion_tree.toml`;
  - new `tools/render_wow_tree.py`;
  - new `scripts/render-wow-tree.cmd`;
  - generated `docs/generated/wow_completion_tree.html`, now defaulting to a
    compact vertical SVG node-link explorer with pan/zoom and an outline toggle;
  - deleted `docs/northshire_playability_grade.md`;
  - removed the starter-zone grade-report command path;
  - roadmap/gate docs now treat user real-client playtesting as the Checkpoint
    2 closure authority.
- Re-run `git status --short --branch` before editing.
- `codex/rusty-mangos` was previously ahead of `origin/codex/rusty-mangos`
  pending the end-of-day push.
- Live client stack was rebuilt/restarted after the fix:
  - authserver PID `41260` on `127.0.0.1:13724`;
  - worldserver PID `30740` on `127.0.0.1:18085`;
  - logs: `auth-client-13724.log`, `world-client-18085.log`;
  - auto-restart is disabled.

## Current Goal

Current milestone: **Northshire Human Warrior playable slice with shared
multiplayer state**.

Current user direction: **Do not use a Northshire grading harness. Keep useful
automation as regression proof, but the user will grade Checkpoint 2 through
real-client playtesting.**

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
- Current branch adds a whole-game completion tree and static dashboard:
  - root `WOW`;
  - top-level systems for auth, protocol, character lifecycle, world/runtime,
    movement, objects, combat, spells, quests, items/economy, progression,
    persistence, social, instances, PvP, scripts, data, and tooling;
  - stable `WOW.<SYSTEM>.<SUBSYSTEM>.<REQUIREMENT>` IDs;
  - Red/Yellow/Green leaf status semantics;
  - parent rollup and completion percentage rules;
  - a CP2 overlay mapping the current missing criteria to specific leaf IDs;
  - `docs/wow_completion_tree.toml` as the editable status source;
  - `.\scripts\render-wow-tree.cmd` to regenerate
    `docs/generated/wow_completion_tree.html`;
  - tree/outline view toggle, status filters, CP2 filter, search, system cards,
    SVG branch links, compact top-down tree layout, root-only initial view,
    click-to-expand nodes, one horizontal band per tree depth, drag-to-pan,
    wheel zoom, zoom/reset buttons, parent-centered child distribution, and
    single-path expansion.

## Tests Run

- `cargo fmt --check`
- `cargo test -p wow-network heroic_strike --lib`
- `cargo test -p wow-network starter_spell --lib`
- `cargo test -p wow-network map_runtime_db_creature_spell_damage_includes_combat_log_packet --lib`
- `cargo test -p wow-network --lib`
- `.\scripts\test-rust.cmd`
- Current docs-only completion-tree update:
  - `git diff --check` passed with only the existing CRLF warning for
    touched Markdown files;
  - `python -m py_compile tools/render_wow_tree.py` passed;
  - `.\scripts\render-wow-tree.cmd` passed and rendered
    `docs/generated/wow_completion_tree.html`;
  - Playwright/Edge smoke opened the generated file, verified 18 system cards,
    177 leaves, search for `Heroic Strike`, and CP2 filtering;
  - After the actual-tree view change, Playwright/Edge smoke verified 259 SVG
    tree nodes, 258 tree links, 18 system cards, and `Heroic Strike` search
    preserving a filtered path;
  - After the vertical pan/zoom change, Playwright/Edge smoke verified 259 SVG
    tree nodes, 258 tree links, 18 system cards, 3 zoom toolbar buttons, plus
    transform changes after zoom and drag-pan;
  - After the compact expand/collapse change, Playwright/Edge smoke verified
    the initial tree shows one root node, clicking `WOW` reveals 18 system nodes
    and 18 links, clicking `WOW.COMBAT` expands the next layer, and zoom still
    changes the SVG transform;
  - After removing wrapped sibling rows, Playwright/Edge smoke verified `WOW`
    expansion has exactly two node y-bands (`54`, `158`) and opening Combat has
    exactly three y-bands (`54`, `158`, `262`), so one visual row now maps to
    one real tree level.
  - After the single-path/parent-centered update, Playwright/Edge smoke verified
    opening Combat gives exactly three y-bands, Combat children are centered on
    the Combat node, then opening Quests closes the Combat child branch and
    opens the Quest child branch.
  - The in-app browser bridge could not be controlled in this session because
    its Node runtime was older than the browser plugin requires; manually
    refreshing the open file tab will show the regenerated tree view.
- Current cleanup after the user rejected the Northshire grade:
  - removed the static grade report from `starter-zone-flow-test`;
  - removed the grade-report switch from `test-starter-zone-flow.ps1`;
  - deleted `docs/northshire_playability_grade.md`;
  - removed the dedicated grade node from the completion tree source.
  - `cargo fmt --check` passed;
  - `cargo check -p starter-zone-flow-test` passed;
  - `python -m py_compile tools/render_wow_tree.py` passed;
  - `.\scripts\render-wow-tree.cmd` passed and regenerated
    `docs/generated/wow_completion_tree.html` with 176 leaves;
  - `git diff --check` passed with only existing CRLF conversion warnings;
  - `.\scripts\test-rust.cmd` passed.

Note: one `.\scripts\test-rust.cmd` run initially failed because the live
`authserver.exe` binary was locked by the running stack. After stopping
auth/world, the same script passed.

## Known Follow-Ups

- Real-client smoke still needs to confirm whether `SMSG_SPELL_START` makes the
  Heroic Strike action bar show the queued state and whether yellow damage
  appears correctly.
- GitHub issue #62 tracks a starter-zone wrapper readiness race discovered
  during the now-removed grade-report invocation. The obsolete flag is gone, but
  the underlying fixed-sleep startup race can still affect normal starter-zone
  runs.
- Useful follow-up: add a CI/check mode for `tools/render_wow_tree.py` that
  fails if `docs/generated/wow_completion_tree.html` is stale, defaults missing
  leaf statuses to Red, rejects hand-marked parent statuses, and emits
  CP2/Gate-specific Markdown tables from the same TOML.
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
- `docs/wow_completion_tree.md`
- `docs/wow_completion_tree.toml`
- `docs/generated/wow_completion_tree.html`
- `tools/render_wow_tree.py`
- `scripts/render-wow-tree.cmd`
