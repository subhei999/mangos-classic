# Session Handoff

Short operating brief for the next Rust migration session. Keep this pruned;
durable roadmap details belong in `docs/rust_migration_plan.md`, gate status in
`docs/playable_gate_board.md`, and branch split details in
`docs/playable_execution_roadmap.md`.

## Current Branch And Worktree

- Branch: `codex/c2-real-client-closure`.
- Purpose: integrate focused Checkpoint 2 real-client parity slices, one at a
  time, then let the user validate against a live 1.12.1 client.
- Latest integrated work:
  - quest status markers and questgiver refresh;
  - combat resource feel, overkill damage fields, regen cap sync, first-swing
    timing, and delayed Heroic Strike rage spend;
  - creature template fidelity for speed, display scale, and equipment fields;
  - DB loot multi-drop fidelity for creature/gameobject loot, grouped loot rows,
    slot-aware autostore, quest-drop gating, and randomized corpse copper;
  - CMaNGOS-like PvE combat skill progression and skill-vs-defense melee math.
- Re-run `git status --short --branch` before editing.
- Live client stack was rebuilt/restarted after the combat skill patch:
  - authserver PID `38244` on `127.0.0.1:13724`;
  - worldserver PID `42116` on `127.0.0.1:18085`;
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
  Heroic-Strike-style rage-spending swings, and map-owned swing timers that
  cannot be accelerated by toggling autoattack.
- Delayed Heroic Strike cost: next-melee starter spells validate power at queue
  time but consume the stored rage/mana cost only when the queued swing resolves.
- Creature fidelity: DB-backed walk/run speed, CMaNGOS-style DBC display scale
  fallback, and virtual item/equipment bytes for weapon visuals are integrated.
- Loot fidelity: creature and gameobject loot rolls keep DB group IDs, support
  multiple independent drops, pick at most one row per group, preserve loot
  slots after partial pickup, gate quest drops on active incomplete objectives,
  and roll corpse copper from DB min/max gold.
- Combat skill math:
  - character skills are loaded into session state on login and persisted when
    they advance;
  - player main-hand attacks use the equipped weapon subclass to choose the
    matching weapon skill, or unarmed when no main-hand weapon is equipped;
  - player attacks against DB creatures use actual weapon skill against creature
    defense (`level * 5`) for miss/glancing/crit calculations;
  - creature attacks use the player's actual defense skill for incoming melee
    avoidance;
  - weapon and defense skill-ups follow the CMaNGOS `UpdateCombatSkills` /
    `UpdateSkill` two-roll flow, including the Intellect bonus for weapon
    skills only;
  - skill value/max updates are persisted and sent to the real client with a
    targeted `SMSG_UPDATE_OBJECT` skill field update.

## Still Unmerged Worker Branches

- `codex/c2-npc-trainer-scripts`:
  `C:\Users\subhe\Documents\mangos-worktrees\c2-npc-trainer-scripts`

Merge remaining worker branches one by one only after inspecting scope,
rebuilding, restarting the client stack, and giving the user concrete
real-client success criteria. The old `codex/c2-skills-weapon-progression` and
loot branches were manually ported into the integration branch instead of being
merged directly.

## Tests Run

- Previous integrated slices:
  - `.\scripts\test-rust.cmd` passed after quest/status, combat resource,
    delayed rage-cost, creature/status, scale, and loot fidelity patches;
  - targeted quest, rage, regen, combat, creature create, random motion,
    creature display scale, creature/gameobject/quest loot, and loot packet
    tests passed for their respective slices;
  - real-client stack was restarted after each user-facing slice and ports
    checked with `Test-NetConnection`.
- Current combat skill patch:
  - `cargo fmt --check`;
  - `cargo check -p wow-db`;
  - `cargo check -p wow-network`;
  - `cargo test -p wow-network skill --lib`;
  - `cargo test -p wow-network melee --lib`;
  - `cargo test -p wow-network combat --lib`;
  - `cargo test -p wow-network heroic_strike --lib`;
  - `cargo test -p wow-network creature_loot --lib`;
  - `.\scripts\test-rust.cmd` passed with `wow_network` at 324 lib tests;
  - `.\scripts\run-client-stack-18085.cmd -NoAutoRestart`;
  - `Test-NetConnection` passed for `127.0.0.1:13724` and
    `127.0.0.1:18085`.

## Real-Client Success Criteria For Current Smoke

- Fight starter mobs with the starting weapon:
  - weapon skill can increase during melee and updates in the Skills UI;
  - defense skill can increase when the player is hit;
  - skill changes persist through relog;
  - miss behavior should improve as weapon skill rises because the hit table now
    uses actual weapon skill vs creature defense.
- Confirm Heroic Strike still keeps rage while queued, consumes rage when the
  swing resolves, and does not award attack rage from that swing.
- Confirm prior parity fixes still hold: gray unavailable quest `!`, no trainer
  disconnect on `Train me`, no autoattack toggle acceleration, correct creature
  scale/equipment animation, variable copper, and combo loot.

## Known Follow-Ups

- PvP skill max behavior is not wired in this slice; current math is PvE DB
  creature combat.
- Flat hit bonuses, weapon-specific auras, and gear hit modifiers still need a
  later combat-math slice. Classic does not use TBC-style combat ratings, but
  hit modifiers still matter.
- Offhand and ranged skill progression are not fully wired; current closure
  slice covers main-hand/base attack and defense for the Human Warrior starter
  path.
- Weapon-skill training from NPCs is separate from combat progression and still
  belongs with trainer/script parity.
- Skill-up chat/combat feedback should be checked in the real client; the field
  update is implemented, but display text/effects may need a follow-up.
- CMSG_SETSHEATHED (`0x01E0`) appears frequently and is still unhandled.
- Full CMaNGOS loot-table rolling remains tracked as issue #58.
- GitHub issue #62 tracks a starter-zone wrapper readiness race from the old
  grade-report path; the grade surface is removed, but normal starter-zone runs
  can still benefit from a readiness check instead of fixed sleep.

## Key Files

- `crates/wow-db/src/character/state.rs`
- `crates/wow-network/src/world/combat/lifecycle.rs`
- `crates/wow-network/src/world/combat/outcome.rs`
- `crates/wow-network/src/world/combat/aggro.rs`
- `crates/wow-network/src/world/entities/player.rs`
- `crates/wow-network/src/world/server/player_login.rs`
- `crates/wow-network/src/world/server/world_session.rs`
- `crates/wow-network/src/world/session.rs`
- `crates/wow-network/src/world/tests.rs`
- `docs/playable_execution_roadmap.md`
