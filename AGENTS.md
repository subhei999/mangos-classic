# Agent Instructions

This repository is being migrated from CMaNGOS Classic C++ to a faithful,
incremental Rust implementation. Treat this file as the mandatory startup
protocol for Codex and other AI coding agents.

## Startup Protocol

Before planning or editing, read these files in order:

1. `docs/session_handoff.md`
2. `docs/rust_migration_plan.md`
3. `docs/rust_auth_foundation.md`

Then run:

```powershell
git status --short --branch
```

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
- Prefer small, runnable vertical slices over broad scaffolding.
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

## Bug Triage And Non-Blocker Logging Policy

This project is a vertical-slice Rust rewrite of CMaNGOS Classic. Do not let
testing discoveries expand the task into unrelated horizontal work.

When you discover a bug, missing behavior, protocol mismatch, DB cleanup gap, or
fidelity issue during implementation or testing, classify it before fixing it.

### Fix Immediately Only If It Is P0/P1

P0 Current-slice blocker:

- Prevents the current requested vertical slice from passing.
- Prevents the real WoW 1.12.1 client from continuing the tested flow.
- Causes crash, panic, disconnect, protocol desync, or test harness failure.
- Corrupts DB state used by the current slice.
- Invalidates the result currently being proven.

P1 Guardrail:

- Could silently corrupt persistent state.
- Could make future tests unreliable.
- Is a small, local, low-risk fix with clear source reference.
- Is required to keep auth/session/DB invariants trustworthy.

Fix P0/P1 issues in the current task if the fix is local and directly supports
the requested slice.

### Do Not Fix P2/P3/P4 During The Current Task

P2 Out-of-scope functional bug:

- Real bug, but outside the current slice.
- Example: testing Human Warrior wolf combat reveals Mage mana regen is wrong.

P3 Fidelity polish:

- Behavior differs from CMaNGOS but does not block the current slice.
- Example: exact stat formula, cinematic flag, obscure packet field, visual
  polish.

P4 Refactor / architecture desire:

- Code organization improvement not required for the current slice.
- Example: `world/mod.rs` should be split before combat systems, unless the
  current change would make it worse.

For P2/P3/P4, do not implement the fix unless the user explicitly asks. Log it
as a GitHub issue or append it to the backlog section described below.

### GitHub Issue Logging Requirement

For every non-blocking P2/P3/P4 issue discovered, create or update a GitHub
issue when GitHub tooling is available.

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

Issue body template:

```md
## Summary

One or two sentences describing the issue.

## Classification

Priority: P2 / P3 / P4
Subsystem:
Discovered while working on:
Current slice blocked? No

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

Smallest likely vertical-slice-safe fix.

## Do not fix now rationale

Explain why this is outside the current task.
```

If GitHub issue creation is unavailable, append the same entry to
`docs/session_handoff.md` under a `Non-blocking Backlog` section.

### Current-Task Final Response Requirement

At the end of every task, report:

- What was implemented.
- Tests run and results.
- P0/P1 bugs fixed immediately.
- P2/P3/P4 issues logged, with GitHub issue numbers or file references.
- Any discovered issues intentionally not fixed.

Do not silently ignore non-blocking issues. Do not expand the task scope to fix
logged non-blockers.

### Per-Task Scope Reminder

Use this reminder when starting a new slice:

```md
Important scope rule:
We are proving one vertical slice only. Fix P0/P1 bugs that block this slice.
Do not chase unrelated horizontal parity issues. For any non-blocking bug,
mismatch, missing subsystem, or cleanup gap you discover, create a GitHub issue
using the repo's bug triage policy, then continue the requested task.
```

## Fast Model Delegation

A very fast, lower-reasoning model can be useful after the senior agent has
identified the C++ reference path, packet/schema shape, and exact Rust seams.
Use it for bounded fill-in work where mistakes are easy to review:

- expanding repetitive test matrices after one golden case is proven;
- adding fixture rows, enum/table mappings, and obvious constant lists from a
  cited C++ or SQL reference;
- drafting doc updates, handoff summaries, and checklist maintenance;
- filling mechanical packet parser/serializer assertions from known byte
  layouts;
- scanning for similar cleanup tables or call sites once the main behavior has
  been established.

Do not use the fast model as the authority for protocol behavior, security or
unsafe-code conclusions, database delete semantics, SRP/auth details, packet
encryption, movement validation, or CMaNGOS parity decisions. Those require the
main agent to read the C++/SQL reference, implement or review the change, and
run the appropriate Rust and Docker-backed tests.

When delegating to a fast model, give it a narrow file set, the exact reference
paths, and the expected output format. Treat the result as a draft patch or
research note, not as accepted truth.

### Fast-Model Task Template

Use this compact prompt when handing a bounded fill-in task to Codex 5.3 Spark
or another fast model:

```md
Task: <one narrow vertical-slice helper task>
GitHub issue: <exact issue URL/number, only if already labeled fast-model-safe>

Scope:
- Files you may read/edit: <exact paths>
- Reference paths supplied by the main agent: <exact C++/SQL/DBC/docs paths>
- Allowed output: <tests | mechanical constants | fixture rows | doc draft | narrow patch>

Forbidden:
- Do not decide protocol behavior, CMaNGOS parity, architecture, security,
  SRP/session logic, packet crypto, DB delete semantics, movement validation,
  broad refactors, or gameplay parity conclusions.
- Do not expand scope beyond the listed files.
- Do not select unlabeled GitHub work; only pull issues explicitly marked
  fast-model-safe and keep the issue scope as the task boundary.

Expected result:
- Changed files:
- Tests run:
- Notes for main-agent review:
```

The main agent remains responsible for reading the authoritative C++/SQL/DBC
references, deciding P0/P1 fixes, reviewing any fast-model output, and running
the required Rust, Docker-backed, packet, or real-client tests.

## Current Next Task

Begin **Checkpoint 1: First Playable World**. First run a real-client smoke
pass against the Rust auth/world stack, then continue world bootstrap and
gameplay parity slices:

- Launch with `scripts/run-client-stack-18085.cmd`.
- Verify WoW 1.12.1 can authenticate, create/select a character, enter world,
  move, logout to character select, and delete a non-loaded character.
- Verify non-human race/gender display ids in-world after the Rust display-id
  mapping fix.
- After that, start with player `SMSG_UPDATE_OBJECT` parity plus
  starter/default leftovers, then proceed through the detailed Checkpoint 1
  roadmap in `docs/rust_migration_plan.md`.
