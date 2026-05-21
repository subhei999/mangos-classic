# Agent Instructions

This repository is being migrated from CMaNGOS Classic C++ to a faithful,
incremental Rust implementation. Treat this file as the mandatory startup
protocol for Codex and other AI coding agents.

## Startup Protocol

Before planning or editing, read:

1. `docs/session_handoff.md`


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

