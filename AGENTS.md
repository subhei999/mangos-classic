# Agent Instructions

This repository is being migrated from CMaNGOS Classic C++ to a faithful,
incremental Rust implementation. Treat this file as the mandatory startup
protocol for Codex and other AI coding agents.

## Startup Protocol

Before planning or editing, read:

1. `docs/session_handoff.md`

Then read only the relevant reference sections for the task:

- `docs/playable_gate_board.md` for gate status, current priority, or playable
  milestone decisions.
- `docs/rust_migration_plan.md` for durable roadmap, crate ownership, or broad
  migration architecture.
- `docs/rust_auth_foundation.md` for authserver setup, auth DB expectations, or
  auth protocol work.

Do not spend tokens reading the full roadmap/auth docs on every task when the
current handoff already contains enough context. If `docs/session_handoff.md`
is stale or missing needed context, refresh it from the reference docs and prune
it back to a concise operating brief.

Then run:

```powershell
git status --short --branch
```

Task start checklist:

- Confirm the current goal from `docs/session_handoff.md`.
- Identify the gate or subsystem touched.
- Check dirty worktree files before editing.
- Identify the smallest useful tests before changing code.
- For gameplay parity, identify the CMaNGOS source or DB/DBC backing first.

Architecture-correct scope rule:

- "Small scoped" must not mean "halfway architecture." When the CMaNGOS
  reference shows that ownership, scheduling, persistence, or authority belongs
  in a different subsystem, implement the smallest complete move to that correct
  owner rather than adding an adapter, shim, or session-local workaround that
  preserves the wrong ownership boundary.
- If a bug was caused by session-owned state, duplicated per-client ticking, or
  stale viewer caches, do not stop after filtering or throttling the symptom.
  Move the source of truth and scheduler to the map/world owner when practical,
  then leave session state as a viewer/input/output cache.
- Prefer thin, architecture-correct vertical slices over tiny patches that make
  the immediate symptom disappear while keeping the wrong control flow. A larger
  patch is acceptable when it removes the bad ownership edge and has focused
  tests proving the new boundary.
- Branch splits and worker scopes are coordination tools, not a reason to defer
  the core fix. If the correct CMaNGOS-shaped fix crosses several nearby files
  in one subsystem, keep it together and test it as one coherent change.
- If an agent intentionally chooses an interim workaround because the full
  ownership move is too risky for the current turn, it must say so explicitly,
  document what remains wrong, and prefer a follow-up that removes the
  workaround before taking unrelated feature work.

Performance reminder for every task:

- While reading CMaNGOS parity paths, keep an audit eye out for large,
  behavior-preserving performance opportunities that would matter with
  thousands of bots or players online.
- Compare the algorithm, data ownership, query pattern, scheduling model, and
  cache behavior CMaNGOS uses against what Rust can do safely and measurably
  better.
- Do not guess or optimize speculatively during parity work. If uncertain,
  implement the CMaNGOS behavior first, then log the optimization as a future
  P4/performance follow-up with evidence and a suggested measurement.

Gameplay data rule:

- Do not fake or hardcode gameplay values when implementing parity behavior.
  Use DB data, DBC/source-derived values, or CMaNGOS formulas. If the real data
  source is not wired yet, leave the behavior unimplemented or narrowly guarded
  and log the follow-up rather than inventing constants.

Playable gate guidance:

- Use `docs/playable_gate_board.md` and `docs/session_handoff.md` as the main
  project compass.
- Treat `codex/rusty-mangos` as the current integration branch. Focused worker
  branches should branch from the latest green `codex/rusty-mangos` commit and
  merge back only after their scoped tests/proof pass.
- Use `docs/playable_execution_roadmap.md` for branch subjects, branch names,
  ownership boundaries, merge order, conflict hot spots, and worker contracts.
- User direction can override the default gate order. When the user names a
  priority, follow that priority and update the docs if it changes the plan.
- When choosing work without explicit user direction, prefer the current
  user-directed next task first, then the highest-value red/yellow gate.
- Do not pile unrelated feature work directly onto `codex/rusty-mangos`.
  Direct integration-branch edits should be limited to small docs/test updates,
  hot integration fixes, or explicit user-directed landing work.

Subagent guidance:

- Use GPT-5.5 as the architect, reviewer, and final integrator for complex
  CMaNGOS parity work, shared-world ownership decisions, safety-critical
  changes, and final verification.
- Prefer GPT-5.3-Codex workers for bounded implementation or investigation
  tasks with clear ownership, especially when the work can run in parallel.
  GPT-5.3-Codex is materially cheaper in Codex token pricing, so it is a good
  default worker model when the parent agent can specify the task precisely and
  review the result.
- Do not use subagents automatically. They are most useful for independent
  codebase searches, CMaNGOS reference comparisons, focused tests/harness work,
  mechanical refactors, or disjoint implementation slices. For small,
  tightly-coupled edits, a single GPT-5.5 pass is often faster and safer.
- When spawning workers, give each one a concrete goal, explicit write scope,
  files or modules it owns, tests to run, and a reminder not to revert
  unrelated worktree changes. Keep write scopes disjoint when using multiple
  workers.
- Give workers stop conditions: stay in scope, do not broaden architecture, do
  not edit docs unless asked, run focused tests, and report changed files plus
  test results.
- The parent agent must inspect and integrate worker output, remove duplication,
  enforce the no-fake/no-hardcoded parity rule, run the relevant tests, update
  docs when the plan changes, and close worker agents when done.

If the task involves Rust code, run the baseline test script before and after
changes when practical:

```powershell
.\scripts\test-rust.cmd
```

If the task touches authserver database behavior or TCP startup, also run:

```powershell
.\scripts\test-rust-db.cmd
```

If the task touches auth protocol behavior, also run:

```powershell
.\scripts\test-auth-flow.cmd
```

## Working Rules

- Keep the C++ CMaNGOS tree as the behavior reference unless the user explicitly
  asks to change it.
- Preserve protocol and schema compatibility with WoW 1.12.1 and
  `sql/base/realmd.sql`.
- Maintain `docs/session_handoff.md` as a short current-state operating brief,
  not a chronological log. At the end of substantial work, prune stale detail
  and update it with:
  - latest commit or current uncommitted state,
  - current goal and recommended next task,
  - what changed recently that still matters,
  - tests run and current confidence,
  - blockers or unproven areas,
  - key files for the next agent.
  Replace obsolete entries instead of appending endlessly; durable roadmap
  history belongs in `docs/rust_migration_plan.md`, the gate dashboard belongs
  in `docs/playable_gate_board.md`, and detailed feature plans belong in their
  own focused docs.
- Do not leave a session without a clean explanation of what is tested and what
  remains unproven.

## Bug Triage And Issue Logging

When you discover a bug, missing behavior, protocol mismatch, DB cleanup gap, or
fidelity issue during implementation or testing, use engineering judgment.
Prefer fixing issues that block the current goal, threaten memory/process
safety, corrupt persistent state, make tests unreliable, or are small local
guardrail fixes.

For issues that are real but better handled later, log them instead of burying
the observation. GitHub Issues are preferred when available; `docs/session_handoff.md`
is an acceptable fallback for lightweight notes, failed GitHub attempts, or
next-agent orientation.

Suggested priority language:

- P0: crash, panic, disconnect, protocol desync, data corruption, or a blocker
  for the current proof.
- P1: guardrail fix for safety, persistence, test reliability, or core
  auth/session/world invariants.
- P2: functional gap that matters but can wait.
- P3: CMaNGOS fidelity polish or visible behavior mismatch that does not block
  the current goal.
- P4: refactor, architecture cleanup, performance follow-up, or tooling debt.

Use this issue title format:

```text
[Rust Rewrite][P2|P3|P4][Subsystem] Short description
```

Examples:

- `[Rust Rewrite][P2][Inventory] Bag move does not persist after relog`
- `[Rust Rewrite][P3][PlayerCreate] Human Warrior stats are hardcoded instead of DBC/source-derived`
- `[Rust Rewrite][P4][World] Split world/mod.rs before adding combat systems`

Apply labels when available:

- `rust-rewrite`
- `parity`
- `bug` or `tech-debt`
- subsystem label if obvious: `auth`, `world`, `characters`, `movement`,
  `inventory`, `combat`, `spells`, `quests`, `db`, `protocol`
- priority label: `P2`, `P3`, or `P4`

Preferred repo labels for this project:

- `rust-rewrite`
- `parity`
- `protocol`
- `db`
- `auth`
- `world`
- `characters`
- `movement`
- `inventory`
- `combat`
- `spells`
- `quests`
- `npc`
- `loot`
- `P0`
- `P1`
- `P2`
- `P3`
- `P4`
- `tech-debt`
- `real-client`
- `cmangos-diff`
- `gate:G1-login`
- `gate:G2-visual-world`
- `gate:G3-visibility-streaming`
- `gate:G4-quest-loop`
- `gate:G5-combat-loot`
- `gate:G6-level-trainer`
- `gate:G7-death-respawn`
- `gate:G8-combat-agency`
- `gate:G9-creature-fidelity`
- `gate:G10-npc-interaction`
- `gate:G11-relog-persistence`
- `gate:G12-multiclient`
- `real-client-required`
- `harness-required`
- `gate-blocker`

Every issue that supports the current playable milestone should have the
matching gate label from `docs/playable_gate_board.md`.

Issue body template:

```md
## Summary

One or two sentences describing the issue.

## Classification

Priority: P2 / P3 / P4
Subsystem:
Discovered while working on:
Current goal blocked? No

## Observed behavior

What happened.

## Expected CMaNGOS / Classic behavior

What should happen, with source path if known.

## Evidence

- Test command:
- Real client observation:
- Relevant logs:
- Relevant packet/opcode:
- Relevant DB tables/rows:

## Suggested future fix

Smallest likely fix or investigation path.

## Why log for later

Explain why this is better handled later.
```

If GitHub issue creation or commenting is unavailable, append the same entry to
`docs/session_handoff.md` under a `Non-blocking Backlog` section and clearly
mark it as a GitHub logging fallback.

### Current-Task Final Response

At the end of every task, report:

- What was implemented.
- Tests run and results.
- Bugs fixed.
- Issues logged, with GitHub issue numbers when available.
- Any known follow-ups or intentionally unfixed discoveries.

### Per-Task Scope Reminder

Use this reminder when starting new work:

```md
Important scope rule:
Stay focused on the current goal, but use judgment. Fix blockers and safety or
data-integrity guardrails when they are practical. Log useful follow-ups when
they should not be handled immediately.

Do not make tiny symptom patches when the CMaNGOS reference shows the wrong
owner or scheduler. If the current bug is caused by session-owned state,
duplicated per-client ticking, stale viewer caches, or misplaced authority,
make the smallest architecture-correct ownership move and prove that boundary
with focused tests.

Keep the final response clear about what changed, what was tested, what bugs
were fixed, and what issues were logged.
```

## Current Next Task

Current user-directed priority is defined in `docs/session_handoff.md` and
`docs/playable_gate_board.md`. The detailed branch split is defined in
`docs/playable_execution_roadmap.md`.

Current next task:

1. Do not add or maintain a Northshire playability grading harness. The user is
   the Checkpoint 2 grader through real-client playtesting.
2. Continue implementation work around the current user-observed missing
   criteria:
   - quest availability restrictions;
   - quest item drops from real loot tables;
   - gameobject quest pickup;
   - warrior level 1-6 spells, global cooldown, and Heroic Strike parity;
   - combat log feedback;
   - health regeneration and rage degeneration;
   - skills and weapon skills;
   - CMaNGOS-like aggro/chase/leash behavior;
   - patrol runtime stability.
3. Split implementation into focused worker branches from
   `docs/playable_execution_roadmap.md`, keeping write scopes disjoint where
   possible. Do not split a subsystem so narrowly that an agent preserves the
   wrong ownership boundary just to keep a patch small; shared map/world
   ownership fixes should land as complete vertical slices.
4. Keep existing G3 movement visibility, shared `MapRuntime`, and
   starter-zone flow tests green while building the missing Northshire systems.
