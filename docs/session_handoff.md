# Session Handoff

Short operating brief for the next Rust migration session. Durable roadmap
history belongs in `docs/rust_migration_plan.md`, gate status in
`docs/playable_gate_board.md`, and branch split details in
`docs/playable_execution_roadmap.md`.

## Current Branch And State

- Branch: `codex/g12-movement-actor-proxy`.
- Worktree:
  `C:\Users\subhe\Documents\mangos-worktrees\g12-movement-actor-proxy`.
- Base commit: `e1625858a4ea4dd6f7cc2bf9d85d2cb1063c84c8`.
- Current local work is uncommitted and adds a feature-flagged movement actor
  proxy path that coexists with the existing `Arc<Mutex<MapRuntime>>`
  ownership model.
- The user remains the Northshire Checkpoint 2 grader through real-client
  playtesting. Do not add or maintain a Northshire grading harness.

## Current Goal

- Prove out a map actor architecture using movement as the first thin slice
  without rewriting full `MapRuntime` ownership yet.
- Keep the current `MapRuntimeManager` map registry and mutex path intact.
- Route movement through an explicit bounded mailbox when
  `world.experimental_movement_actor = true`, while preserving the legacy
  direct mutex path when disabled.
- The touched subsystem is G12 shared runtime hardening with a world/map
  runtime architecture focus, not a gameplay-feature rewrite.

## What Changed Recently

- Added `world::map_runtime::movement_actor`, a bounded `tokio::mpsc` movement
  actor that receives `UpdatePlayerPosition` commands, batches mailbox drains,
  dedupes by `character_guid`, and replies to every caller through a oneshot.
- The actor is a Phase 1 proxy only: it still locks the existing
  `Arc<Mutex<MapRuntime>>` internally and calls
  `MapRuntime::update_player_position(...)`. This keeps ownership safe while
  proving command routing, batching, and backpressure.
- `MapRuntimeManager::update_player_position(...)` now supports two backends:
  actor path when enabled and available, or the existing direct mutex path as
  fallback.
- Added config plumbing for `world.experimental_movement_actor` from
  `config/worldserver.toml` through `wow-config`, `worldserver`, and
  `WorldServerOptions`.
- Added movement-path observability for actor queue depth, enqueue failures,
  enqueue latency, processing time, reply latency, batch size, and movement
  map-mutex wait/hold timing.
- Direct mutex movement updates now record the same movement mutex wait/hold
  timing so actor and non-actor paths can be compared honestly.

## Tests Run

- `.\scripts\test-rust.cmd` passed before the movement-actor changes.
- `cargo test -p wow-config --lib` passed.
- `cargo test -p wow-network movement_actor --lib` passed.
- `cargo test -p wow-network prometheus_render_includes_histogram_and_opcode_labels --lib`
  passed.
- `cargo test -p wow-network --lib` passed: 749 tests.
- `cargo fmt` applied required formatting.
- `.\scripts\test-rust.cmd` passed after formatting and code changes.

## Current Confidence

- The feature-flagged proxy path is wired end-to-end and coexists safely with
  the old mutex path.
- Tests cover actor-disabled fallback, actor-path packet parity against the
  direct path, bounded-mailbox backpressure, concurrent reply completion, and
  batch dedupe semantics for repeated player movement within one drained batch.
- Confidence is good for the proxy architecture slice, but there is not yet a
  synthetic benchmark proving whether batching materially improves burst
  movement throughput under larger player counts.

## Known Follow-Ups

- Add the requested synthetic movement benchmark comparing direct mutex,
  movement actor proxy, and batched actor paths at roughly 100, 1,000, and
  5,000 player update bursts.
- Export or report movement metrics in a form that is easy to compare between
  benchmark runs if the current Prometheus output is too raw.
- Decide whether the next step should stay movement-only or grow toward a true
  map-owned actor once enough evidence exists that command routing and
  backpressure are worth the ownership move.
- If batching semantics need to change, keep the safety rule explicit:
  superseded movement commands currently complete with `Ok(Vec::new())`
  instead of silently dropping replies.

## Key Files

- `crates/wow-network/src/world/map_runtime/movement_actor.rs`
- `crates/wow-network/src/world/map_runtime/map_manager.rs`
- `crates/wow-network/src/world/map_runtime/map.rs`
- `crates/wow-network/src/world/map_runtime/mod.rs`
- `crates/wow-network/src/world/mod.rs`
- `crates/wow-network/src/world/server/movement.rs`
- `crates/wow-network/src/world/session.rs`
- `crates/wow-network/src/world/tests.rs`
- `crates/wow-network/src/observability.rs`
- `crates/wow-config/src/lib.rs`
- `bins/worldserver/src/main.rs`
- `config/worldserver.toml`
