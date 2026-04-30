# Agent Instructions

This repository is being migrated from CMaNGOS Classic C++ to a faithful,
incremental Rust implementation. Treat this file as the mandatory startup
protocol for Codex and other AI coding agents.

## Startup Protocol

Before planning or editing, read these files in order:

1. `docs/session_handoff.md`
2. `docs/playable_gate_board.md`
3. `docs/rust_migration_plan.md`
4. `docs/rust_auth_foundation.md`

Then run:

```powershell
git status --short --branch
```

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

Playable gate guidance:

- Use `docs/playable_gate_board.md` and `docs/session_handoff.md` as the main
  project compass.
- User direction can override the default gate order. When the user names a
  priority, follow that priority and update the docs if it changes the plan.
- When choosing work without explicit user direction, prefer the current
  user-directed next task first, then the highest-value red/yellow gate.

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
- Update `docs/session_handoff.md` at the end of substantial work with:
  - latest commit,
  - what changed,
  - tests run,
  - blockers,
  - recommended next task,
  - key files for the next agent.
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
they should not be handled immediately. Keep the final response clear about what
changed, what was tested, what bugs were fixed, and what issues were logged.
```

## Current Next Task

Current user-directed priority is defined in `docs/session_handoff.md` and
`docs/playable_gate_board.md`.

Current next task:

1. Derisk Multiplayer / Shared MapRuntime.
2. Keep one monolithic worldserver, but stop treating each TCP session as its
   own mini-world.
3. Introduce a shared in-process `MapRuntime` / grid layer inside
   `WorldRuntimeState`.
4. Route player visibility, movement, `/say`, and DB creature state through the
   shared runtime.
5. Prove two Northshire clients can see each other spawn/move/logout, exchange
   nearby `/say`, and observe one shared DB creature state without duplicated
   kill/loot state.
6. Keep existing G3 movement visibility and starter-zone flow tests green.
