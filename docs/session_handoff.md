# Session Handoff

Short operating brief for the next Rust migration session. Durable roadmap
history belongs in `docs/rust_migration_plan.md`, gate status in
`docs/playable_gate_board.md`, and branch split details in
`docs/playable_execution_roadmap.md`.

## Current Branch And State

- Branch: `codex/rusty-mangos`, in the main checkout at
  `C:\Users\subhe\Documents\New project`.
- HEAD at task start/current base: `318189dcc`.
- Worktree is intentionally dirty with the current player spell-system parity
  slice plus a focused HP/mana regen and spell-power packet timing fix. Touched
  files are limited to spell/world DB metadata, spell execution, map-runtime
  aura/movement helpers, opcodes/session state, and
  `crates/wow-network/src/world/tests.rs`.
- Current user-directed priority: player spell-system parity for generic
  mechanics behind spells such as Conjure Food, Frost Nova, and ranked buffs.
  Keep gameplay values backed by CMaNGOS/DBC/DB data; do not add spell-ID
  special cases for these families.
- Playerbots remain disabled by default for normal multiplayer/Northshire
  testing: `config/worldserver.local.toml` has `[playerbots] enabled = false`
  and `[playerbots.random] enabled = false`.

## Current Goal And Recommended Next Task

- Goal: continue closing user-observed missing spell behavior while preserving
  the existing melee, ranged, item-use, aura tick, and starter-zone flows.
- Recommended next task: real-client smoke the expanded spell slice. Verify
  Conjure Food creates the DBC/DB item into inventory, merges stacks before
  empty slots, fails cleanly when bags are full, and does not spend
  resources/cooldowns on preflight failure. Verify Frost Nova resolves nearby
  hostile targets from DBC radius/implicit target data, roots players/creatures,
  stops rooted creature movement/chase, and unroots on expiration. Verify ranked
  buffs such as Arcane Intellect refresh on same spell, higher rank replaces
  lower across casters, lower rank after higher bounces, `spell_group` unique
  categories replace correctly, and stats do not double-apply.
- After this proof, continue the broader spell backlog: full SpellStacker
  aura-effect stackability matrix, dispels, summons, pets, channeled AoE,
  totems, shapeshifts, stealth, fear/stun/confuse, advanced proc rules, and the
  custom script-hook system once the generic engine needs it.

## Recent Implemented Work

- Extended world spell metadata loading from `spell_template` with dispel,
  mechanic, stack amount, per-effect mechanic, implicit target B, radius index,
  and item type fields. Added `spell_chain` DB lookup/cache in `ObjectMgr`.
- Added `SpellRadius.dbc` loading into `WorldDataFiles`/`MapRuntimeManager` so
  AoE spell radius comes from DBC data rather than constants.
- Extended `SpellInfoEffect` and spell profile derivation for
  `SPELL_EFFECT_CREATE_ITEM`, effect item type, secondary implicit targets, and
  caster-centered hostile AoE targets (`15` / `36`).
- Implemented generic create-item spell handling. Item id comes from
  `EffectItemTypeN`; count comes from the CMaNGOS effect roll value with a
  minimum of 1 and item stack-size cap. Cast preflight checks item-template
  existence and storage space, then the effect uses the existing inventory store
  plan, stack merge/add paths, persistence helpers, item push result, and update
  packets.
- Implemented `SPELL_AURA_MOD_ROOT` as a generic aura modifier. Players now send
  force-root/unroot packets when root state changes or expires. Creatures stop
  active motion when newly rooted, and chase/random/waypoint/return-home motion
  will not start while root aura state is active.
- Added caster-centered hostile AoE aura application for player spells. The
  effect resolves DBC radius metadata, finds nearby hostile DB creatures from
  map-owned spatial/faction state, applies the aura to each target, and starts
  retaliation.
- Added rank-aware aura conflict checks backed by `spell_chain`. Same spell from
  the same caster refreshes; higher rank in the same chain replaces lower-rank
  auras; lower/equal different-rank recasts bounce with
  `SPELL_FAILED_AURA_BOUNCED`; replacement paths avoid duplicate stat
  application in session and map-owned aura state.
- Added `spell_group` / `spell_group_spell` DB lookup and ObjectMgr caching.
  Aura conflict preflight now honors CMaNGOS group rules: `UNIQUE` replaces
  matching aura groups regardless of caster, while `UNIQUE_PER_CASTER` only
  replaces the caster's own matching group. Rank checks also bounce stronger
  positive auras from other casters and replace weaker positive ranks across
  casters.
- Broadened generic implicit target handling for direct friendly unit aura
  targets (`TARGET_UNIT_FRIEND`, party/raid unit variants, chain-heal target).
  Friendly player-target aura casts now update map-owned target aura state and
  dispatch direct/observer aura packets instead of silently doing nothing or
  falling back to self.
- Kept unsupported player spell effects visible with warning logs so new spell
  families are easier to triage.
- Fixed a parity wrinkle found during tests: caster-centered hostile root auras
  are classified as debuffs, not positive self buffs.
- Fixed a critical player power timing issue: spell mana/rage/energy is still
  spent from map-owned state at cast completion, but the client-visible power
  `SMSG_UPDATE_OBJECT` now goes out immediately before cast result/`SMSG_SPELL_GO`
  instead of waiting for delayed projectile impact. This matches the CMaNGOS
  `Spell::cast` ordering where `TakePower()` happens before `SendSpellGo()`.
- Strengthened the map-owned regen/session-cache regression so food/drink-style
  mana ticks, health regen, and rage decay survive refresh/sync without stale
  session state pushing bars backwards.

## Tests Run

- Baseline before spell changes:
  `$env:CARGO_TARGET_DIR='target\codex-spells-baseline'; .\scripts\test-rust.cmd`
  passed fully.
- During implementation:
  `$env:CARGO_TARGET_DIR='target\codex-spells-dev'; cargo test -p wow-network --lib`
  initially exposed a DB access bug in aura-rank conflict preflight when no
  same-caster different-rank aura was active; after the early-return fix it
  passed with 634 tests, and later with 640 tests after focused spell tests were
  added.
- Added focused tests for SpellRadius DBC parsing, create-item metadata/stack
  cap, full-backpack storage planning, Frost-Nova-style caster-centered hostile
  root targeting/radius/debuff classification, root movement/root packet
  expiration, and ranked aura replacement/bounce/stat behavior.
- Final verification:
  `$env:CARGO_TARGET_DIR='target\codex-spells-final'; .\scripts\test-rust.cmd`
  passed fully after clippy cleanup, including fmt, clippy, workspace unit/doc
  tests, `wow-network` 640 tests, `wow-proto` 23 tests, and authserver/worldserver
  builds in the isolated target dir.
- Regen/power timing investigation:
  baseline `$env:CARGO_TARGET_DIR='target\codex-regen-baseline'; .\scripts\test-rust.cmd`
  initially failed on a pre-existing `cargo fmt --check` mismatch in
  `crates/wow-network/src/world/spells.rs`; `cargo fmt` fixed it. Focused
  tests passed for
  `session_cache_refresh_preserves_map_owned_regen_before_session_sync` and
  `cast_time_spell_sends_start_before_delayed_go_and_effects`. Final
  verification `$env:CARGO_TARGET_DIR='target\codex-regen-final'; .\scripts\test-rust.cmd`
  passed fully, including `wow-network` 643 tests.
- Baseline for the follow-up stacker/target slice:
  `$env:CARGO_TARGET_DIR='target\codex-spellstack-baseline'; cargo test -p wow-network --lib`
  passed with 640 tests.
- Focused follow-up checks:
  `$env:CARGO_TARGET_DIR='target\codex-spellstack-dev'; cargo test -p wow-network --lib conflict`
  passed with the rank/group conflict tests, and
  `$env:CARGO_TARGET_DIR='target\codex-spellstack-dev'; cargo test -p wow-network --lib direct_friendly_unit`
  passed. Full crate rerun:
  `$env:CARGO_TARGET_DIR='target\codex-spellstack-dev'; cargo test -p wow-network --lib`
  passed with 643 tests.
- Final verification after the stacker/target follow-up:
  `$env:CARGO_TARGET_DIR='target\codex-spellstack-final'; .\scripts\test-rust.cmd`
  passed fully, including fmt, clippy, workspace unit/doc tests,
  `wow-network` 643 tests, `wow-proto` 23 tests, and authserver/worldserver
  builds in the isolated target dir.

## Real-Client Verification Needed

- Conjure Food live cast: inventory creation, stack merge, item push/update
  packets, bag-full failure, missing-template logging if DB data is absent, and
  resource/cooldown behavior around failed preflight.
- Frost Nova live cast: root animation/state, hostile-only AoE selection,
  creature movement stop/resume, expiration unroot, combat retaliation, and no
  friendly/self accidental roots.
- Ranked/grouped buffs live cast: higher/lower rank interactions across casters,
  `spell_group` unique category replacement, visible aura slot replacement,
  bounce failure text, and no doubled character-panel stats.
- Friendly unit buffs live cast: Arcane Intellect or similar direct friendly
  target auras should apply to the selected friendly player, not self or hostile
  creatures.
- Regression smoke: existing damage, heal, DoT, Battle Shout, Heroic Strike,
  Auto Shot, item-use, and aura tick behavior.
- HP/mana real-client smoke: verify food/drink bars only increase while seated
  and out of interrupting actions, no stale lower-value snapback occurs after
  client input, normal mana regen resumes after the five-second rule, and
  projectile spell mana visibly drops on cast launch rather than on impact.

## Current Follow-Ups

- The create-item path has focused metadata/planner tests but still needs a
  real DB/client proof for actual persisted inventory creation and packet
  sequencing.
- The AoE target resolver currently covers the caster-centered hostile target
  families needed by Frost Nova. Direct friendly player targets are now
  classified/applied, but party/raid area targets, chain jumps, cone targets,
  destination-location AoE, and gameobject/unit-location target payloads remain
  future spell-engine work.
- The DB-backed `spell_group` foundation is in, but full Classic SpellStacker
  parity still needs the CMaNGOS per-aura stackability matrix, exclusive dispel
  categories, diminishing-return interaction, and special proc/aura rules.
- Custom spell script hooks are intentionally deferred; the user wants generic
  systems 2 and 3 first and script architecture later.

## Key Files

- `crates/wow-db/src/world_data.rs`
- `crates/wow-network/src/world/globals/object_mgr.rs`
- `crates/wow-network/src/world/map_runtime/world_data.rs`
- `crates/wow-network/src/world/map_runtime/map_manager.rs`
- `crates/wow-network/src/world/map_runtime/map/{players.rs,creature_damage.rs,spatial.rs}`
- `crates/wow-network/src/world/combat/motion.rs`
- `crates/wow-network/src/world/spells.rs`
- `crates/wow-network/src/world/spells/{effects.rs,spell_mgr.rs,targets.rs}`
- `crates/wow-network/src/world/session.rs`
- `crates/wow-network/src/world/opcodes.rs`
- `crates/wow-network/src/world/tests.rs`
