# Northshire Playability Grade Harness

This checklist is the worker-facing grade surface for the current nine
Northshire gaps from `docs/session_handoff.md` and
`docs/playable_execution_roadmap.md`.

## Run Commands

- Full starter-zone lock + grade report:
  - `.\scripts\test-starter-zone-flow.cmd -NorthshireGrade`
- Direct Rust command:
  - `cargo run -p starter-zone-flow-test -- --northshire-grade`

## Grade Shape

The harness report prints one row per criterion with:

- `Parity`: `PASS` / `FAIL` against current CMaNGOS-like behavior expectations.
- `Harness`: `PASS` / `TODO` for whether a concrete automated probe exists.

Current target criteria (all nine rows are emitted every run):

1. Quest eligibility restrictions.
2. Quest item drops from real loot tables.
3. Gameobject quest pickup.
4. Warrior level 1-6 / GCD / Heroic Strike parity.
5. Combat log feedback.
6. Health regeneration and rage degeneration.
7. Skills and weapon skills.
8. CMaNGOS-like aggro/chase/leash behavior.
9. Patrol runtime stability.

## Expected Near-Term Usage

- Treat `Parity=FAIL` as the baseline before the focused implementation branches
  land.
- Treat `Harness=TODO` as a direct test-gap queue for follow-up workers.
- Promote rows to `Parity=PASS` only when the underlying behavior is wired and
  proven by both harness and real-client smoke where required.
