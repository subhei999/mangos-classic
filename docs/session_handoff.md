# Session Handoff

Short operating brief for the next Rust migration session. Keep this pruned;
durable roadmap details belong in `docs/rust_migration_plan.md`, gate status in
`docs/playable_gate_board.md`, and branch split details in
`docs/playable_execution_roadmap.md`.

## Current Branch And Worktree

- Branch: `codex/c2-quest-reward-transaction`.
- Purpose: focused Checkpoint 2 worker slice for CMaNGOS-like quest reward
  transactions, quest reputation rewards, and real-client reputation packet
  feedback.
- Base intent: branch from the latest integration state, prove the slice with
  focused tests plus the full Rust suite, then merge/rebase into
  `codex/rusty-mangos` or the current Checkpoint 2 integration branch.
- Current uncommitted code changes:
  - quest templates now load `RewRepFaction1..5` and `RewRepValue1..5`;
  - quest reward completion persists money and quest reputation changes in the
    same DB transaction;
  - changed reputation rows are returned to the world layer so the client can
    receive immediate visible/standing packets;
  - reputation packet helpers send CMaNGOS-shaped
    `SMSG_SET_FACTION_VISIBLE` / `SMSG_SET_FACTION_STANDING`;
  - Northshire Stormwind quest reward rows were verified in the local world DB.
- Re-run `git status --short --branch` before editing.
- Live client stack was rebuilt/restarted after this slice:
  - authserver PID `47804` on `127.0.0.1:13724`;
  - worldserver PID `27128` on `127.0.0.1:18085`;
  - logs: `auth-client-13724.log`, `world-client-18085.log`;
  - auto-restart is disabled.

## Current Goal

Current milestone: **Checkpoint 2 Northshire Human Warrior playable slice with
shared multiplayer state**.

Current user direction: **Do not use a Northshire grading harness. The user is
the grader using real-client side-by-side testing. Convert real-client notes
into generalized systems work and priorities; do not hardcode Northshire-only
fixes. Anything less than CMaNGOS/source-backed gameplay math is considered
fake.**

Important scope rule: stay focused on the current goal, but use judgment. Fix
blockers and safety/data-integrity guardrails when practical. Log useful
follow-ups when they should not be handled immediately.

Gameplay data rule: do not fake or hardcode gameplay values for parity work.
Use DB data, DBC/source-derived values, or CMaNGOS formulas. If the real data
source is not wired yet, leave behavior unimplemented or narrowly guarded and
log the follow-up.

## Recently Landed

- Quest status markers: unavailable quest givers now surface gray quest status
  instead of being invisible to the player, and quest accept/reward handlers
  proactively resend visible questgiver status after state changes.
- Combat resource feel: damage-based rage generation, overkill fields, max-HP
  regen cap sync, immediate first-swing scheduling, no attack-rage from
  Heroic-Strike-style rage-spending swings, delayed Heroic Strike rage spend,
  and map-owned swing timers that cannot be accelerated by toggling autoattack.
- Creature fidelity: DB-backed walk/run speed, CMaNGOS-style DBC display scale
  fallback, and virtual item/equipment bytes for weapon visuals are integrated.
- Loot fidelity: creature and gameobject loot rolls keep DB group IDs, support
  multiple independent drops, pick at most one row per group, preserve loot
  slots after partial pickup, gate quest drops on active incomplete objectives,
  and roll corpse copper from DB min/max gold.
- Combat skill math: character skills load on login, PvE weapon/defense
  skill-ups follow CMaNGOS two-roll logic with Intellect bonus for weapon
  skills, melee hit tables use skill vs defense, and level-up skill caps now
  update the client immediately.
- Trainer/gossip parity: trainer gossip uses the book icon and
  `I seek training.`, trainer list greetings come from DB/fallback text, and
  buying a trainer spell sends source-backed visual/impact packets before
  `SMSG_TRAINER_BUY_SUCCEEDED`.

## Current Slice Details

- DB-backed quest reputation rewards:
  - `QuestTemplateQuery` now carries `rew_rep_faction` and `rew_rep_value`
    arrays loaded from `quest_template`.
  - `reward_character_quest` now marks the quest rewarded, updates money, and
    upserts `character_reputation` rows in one transaction.
  - Reputation standing is clamped to CMaNGOS caps (`42999` / `-42000`) and
    written with `FACTION_FLAG_VISIBLE`.
- World/client feedback:
  - quest completion computes CMaNGOS-like quest reputation gains from quest
    level and player level;
  - mapped factions send visible and standing packets immediately after reward;
  - login initial reputation packets use the same faction-to-reputation-list
    bridge.
- Local DB evidence:
  - quest `3100` Simple Letter uses `SrcItemId=9542`, `SrcItemCount=1`, and
    Stormwind reputation `72 => 75`;
  - quests `7`, `33`, `783`, and `5261` also carry Stormwind reward rows.
- Known architecture compromise:
  - the faction-to-reputation-list bridge is static and DBC-derived for known
    1.12 factions. It is sufficient for Northshire proof but should be replaced
    by a real `Faction.dbc` loader before broader faction coverage.

## Tests Run

- `cargo fmt`
- `cargo fmt --check`
- `cargo check -p wow-db`
- `cargo check -p wow-network`
- `cargo test -p wow-network reputation --lib`
- `cargo test -p wow-network quest_reward --lib`
- `cargo test -p wow-network initial_reputations --lib`
- `.\scripts\test-rust.cmd` passed with `wow_network` at 328 lib tests.
- `.\scripts\run-client-stack-18085.cmd -NoAutoRestart`
- `Test-NetConnection` passed for `127.0.0.1:13724` and `127.0.0.1:18085`.

## Real-Client Success Criteria For Current Smoke

- Turn in `A Threat Within`, `Kobold Camp Cleanup`,
  `Wolves Across the Border`, and `Eagan Peltskinner`.
- Confirm Stormwind reputation gain appears immediately in the real client and
  the reputation pane standing increases.
- Relog after gaining reputation; standing should persist and initialize
  correctly.
- Accept quest `Simple Letter` (`3100`) and confirm item `Simple Letter`
  (`9542`) appears in the backpack on accept. If not, the next target is the
  source-item create/update packet or inventory slot sync path, because the DB
  source item fields and accept-grant path are present.
- Confirm prior parity fixes still hold: gray unavailable quest `!`, no trainer
  disconnect on `Train me`, no autoattack toggle acceleration, correct creature
  scale/equipment animation, variable copper, combo loot, and skill cap UI
  updates.

## Known Follow-Ups

- GitHub issue #63 tracks replacing the static reputation-list bridge with a
  real `Faction.dbc` loader.
- Full trainer spell-cast animation is still incomplete: CMaNGOS starts the
  trainer spell after buy success, while the current Rust slice sends the
  source-backed visual/impact packets and direct learned-spell updates.
- PvP skill max behavior, offhand/ranged skill progression, flat hit bonuses,
  weapon-specific auras, and gear hit modifiers remain later combat slices.
- Weapon-skill training from NPCs is separate from combat progression and still
  belongs with trainer/script parity.
- CMSG_SETSHEATHED (`0x01E0`) appears frequently and is still unhandled.
- GitHub issue #62 tracks a starter-zone wrapper readiness race from the old
  grade-report path; the grade surface is removed, but normal starter-zone runs
  can still benefit from a readiness check instead of fixed sleep.

## Key Files

- `crates/wow-db/src/world_data.rs`
- `crates/wow-db/src/character/state.rs`
- `crates/wow-db/src/character/types.rs`
- `crates/wow-network/src/world/quests.rs`
- `crates/wow-network/src/world/reputation/reputation_mgr.rs`
- `crates/wow-network/src/world/server/world_session.rs`
- `crates/wow-network/src/world/tests.rs`
- `docs/playable_execution_roadmap.md`
