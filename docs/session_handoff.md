# Session Handoff

Short operating brief for the next Rust migration session. Keep this pruned;
durable roadmap details belong in `docs/rust_migration_plan.md`, gate status in
`docs/playable_gate_board.md`, and branch split details in
`docs/playable_execution_roadmap.md`.

## Current Branch And Worktree

- Branch: `codex/rusty-mangos`.
- Base branch: `origin/codex/rusty-mangos`.
- Current HEAD: `9be50f988`.
- Current state: uncommitted quest availability, quest/source-item loot,
  quest source-item storage, visible gameobject quest dynamic-refresh, and
  map-owned player regen observer-broadcast work is complete and tested.
  Latest automation slice also loads `quest_template.RequiredCondition` and
  conservatively hides/rejects condition-gated quests until Rust has a real
  CMaNGOS condition evaluator.
- Git blocker: `git add` / `git commit` still fail with
  `Unable to create ... .git/index.lock: Permission denied`. No
  `.git/index.lock` file is visible; stale `git.exe` processes were stopped,
  but `.git` currently shows an explicit sandbox deny ACL for write/delete.
  No commit was created.
- Suggested commit message once Git is writable:
  `Tighten DB-backed quest availability gates`.
- Scoped dirty files:
  - `crates/wow-db/src/world_data.rs`
  - `crates/wow-network/src/world/entities/gameobject.rs`
  - `crates/wow-network/src/world/loot.rs`
  - `crates/wow-network/src/world/maps/map/gameobject_snapshots.rs`
  - `crates/wow-network/src/world/maps/map/players.rs`
  - `crates/wow-network/src/world/maps/map_manager.rs`
  - `crates/wow-network/src/world/opcodes.rs`
  - `crates/wow-network/src/world/quests.rs`
  - `crates/wow-network/src/world/server/player_login.rs`
  - `crates/wow-network/src/world/server/world_session.rs`
  - `crates/wow-network/src/world/session.rs`
  - `crates/wow-network/src/world/tests.rs`
  - `docs/session_handoff.md`

Run `git status --short --branch` before editing.

## Current Goal

Current milestone remains **Checkpoint 2 Northshire Human Warrior playable
slice with shared multiplayer state**. The active priority order is still the
user-observed Northshire criteria in `docs/playable_gate_board.md`: quest
availability restrictions, quest loot drops, gameobject quest pickup, warrior
level 1-6 spell/resource/skill behavior, combat log feedback,
CMaNGOS-like aggro/leash, and patrol stability.

Important scope rule: stay focused on the current goal, but fix blockers and
safety/data-integrity guardrails when practical. Do not make tiny symptom
patches that preserve the wrong owner or scheduler.

Gameplay data rule: do not fake or hardcode gameplay values for parity work.
Use DB data, DBC/source-derived values, or CMaNGOS formulas. If the real source
is not wired yet, leave behavior unimplemented or narrowly guarded and log the
follow-up.

## Recently Changed

- Added the 1.12.1 / CMaNGOS `SMSG_QUESTLOG_FULL` opcode (`0x0195`) and made
  quest accept reject full quest logs before creating hidden DB quest rows.
- Quest marker visibility and acceptance now apply CMaNGOS-shaped level,
  race/class, skill, reputation, prerequisite, repeatability, and negative
  exclusive-group rules using DB-backed quest template fields.
- Player reputation is loaded into `WorldSessionState` on login and refreshed
  from quest reward results so reputation-gated quest availability can update
  without relogging.
- `QuestTemplateQuery` now carries DB-backed `RequiredSkill`,
  `RequiredSkillValue`, `RequiredMinRepFaction`, `RequiredMinRepValue`,
  `RequiredMaxRepFaction`, `RequiredMaxRepValue`, `RequiredCondition`,
  `ReqSourceId*`, and `ReqSourceCount*`.
- CMaNGOS `Player::SatisfyQuestCondition` has a narrow Rust guard:
  `RequiredCondition = 0` behaves normally, while nonzero required conditions
  are not exposed or accepted until `ObjectMgr::IsConditionSatisfied` parity is
  implemented. This avoids over-broad quest availability without inventing
  condition results.
- DB creature and gameobject loot selection now applies CMaNGOS
  `Player::HasQuestForItem` source-item behavior for negative
  `ChanceOrQuestChance` rows: active incomplete quests can enable
  `ReqSourceId*` drops, and zero `ReqSourceCount*` uses the DB item
  `maxcount`/`stackable` limit instead of an invented constant.
- Quest accept now mirrors the CMaNGOS
  `CanAddQuest -> CanGiveQuestSourceItemIfNeed -> CanStoreNewItem` source-item
  storage guard: if a source item cannot fit in an existing stack or empty
  backpack slot, Rust sends inventory-change failure and does not accept the
  quest. Source-item grants can now merge into existing stacks or split across
  multiple new backpack stacks.
- Added a CMaNGOS `Player::UpdateForQuestWorldObjects`-shaped refresh for
  visible DB gameobjects after quest state changes. `MapRuntime` now exposes
  the player's visible DB gameobject GUIDs/snapshots, and quest refresh sends
  `GAMEOBJECT_DYN_FLAGS` updates for quest-relevant visible gameobjects so
  accepted gameobject objectives can become clickable/sparkled without relog.
- Map-owned player health/mana regen and warrior rage degeneration now fan out
  unit-field update packets to nearby players as well as the affected player,
  matching the CMaNGOS `WorldObject::BuildUpdateData` pattern where the object
  itself and visible clients receive value updates.

## Tests Run

- Baseline before the latest `RequiredCondition` edits:
  `.\scripts\test-rust.cmd` passed.
- `cargo fmt --check` passed.
- `cargo test -p wow-network map_runtime_player_regen_tick --lib` passed with
  4 focused tests, including
  `map_runtime_player_regen_tick_broadcasts_visible_player_updates`.
- `cargo test -p wow-network quest_dialog_status_hides_unwired_required_condition --lib`
  passed.
- Post-change `.\scripts\test-rust.cmd` passed with 370 `wow-network` tests.

## Known Follow-Ups

- Resolve the local Git index-lock permission blocker, then commit the current
  scoped work. The latest useful commit message would be
  `Tighten DB-backed quest availability gates`.
- Continue quest availability parity from CMaNGOS `CanTakeQuest` /
  `CanSeeStartQuest`: real `RequiredCondition` evaluation, breadcrumb quests,
  dependent breadcrumbs, weekly/timed constraints, active quest flags, exact
  event-controlled quest active state, and broader source-item/bag storage
  semantics are not all wired yet.
- Loot-table fidelity still does not process references or condition rows; the
  source-item slice only covers direct DB loot rows already loaded by the Rust
  loot query.
- Gameobject quest pickup still needs real-client proof on the Northshire route
  after the dynamic-flag refresh lands.
- Continue warrior level 1-6 spell/GCD/resource/skill behavior, aggro/leash
  parity, and patrol runtime stability per `docs/playable_gate_board.md`.

## Key Files

- `crates/wow-network/src/world/quests.rs`
- `crates/wow-network/src/world/gameobjects.rs`
- `crates/wow-network/src/world/entities/gameobject.rs`
- `crates/wow-network/src/world/maps/map/gameobject_snapshots.rs`
- `crates/wow-network/src/world/maps/map/players.rs`
- `crates/wow-network/src/world/maps/map_manager.rs`
- `crates/wow-network/src/world/loot.rs`
- `crates/wow-network/src/world/tests.rs`
- `crates/wow-network/src/world/globals/object_mgr.rs`
- `crates/wow-db/src/world_data.rs`
- `docs/playable_gate_board.md`
- `docs/playable_execution_roadmap.md`
