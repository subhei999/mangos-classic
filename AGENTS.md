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

## Current Next Task

Expand the auth compatibility harness toward drop-in replacement coverage:

- Add negative cases for unknown account, bad proof, banned account, and
  unsupported build behavior.
- Compare packet shapes against the C++ `realmd` reference.
- Keep the harness runnable through `scripts/test-auth-flow.cmd`.
