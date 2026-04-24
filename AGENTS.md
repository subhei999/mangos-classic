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
