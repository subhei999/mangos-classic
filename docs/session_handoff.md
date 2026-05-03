# Session Handoff

Short operating brief for the next Rust migration session. Keep this pruned;
durable roadmap details belong in `docs/rust_migration_plan.md`, gate status in
`docs/playable_gate_board.md`, and branch split details in
`docs/playable_execution_roadmap.md`.

## Current Branch And Worktree

- Branch: `codex/c2-spell-aura-system`.
- Current state: uncommitted spell/aura foundation changes are present.
- Purpose: Checkpoint 2 warrior spell slice that moves starter spell execution
  away from spell-id fixtures and toward the CMaNGOS `Spell.dbc` /
  `spell_template` model.
- Base intent: this branch is stacked on the current C2 closure/testing state
  rather than the stale old `codex/c2-warrior-spells-gcd` branch.
- Re-run `git status --short --branch` before editing.
- Live client stack was rebuilt/restarted after this slice:
  - authserver PID `44896` on `127.0.0.1:13724`;
  - worldserver PID `26968` on `127.0.0.1:18085`;
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

Immediate focus: continue warrior level 1-6 spell parity using a real
CMaNGOS-style spell/aura pipeline, not one-off Heroic Strike or Battle Shout
patches.

Important scope rule: stay focused on the current goal, but use judgment. Fix
blockers and safety/data-integrity guardrails when practical. Log useful
follow-ups when they should not be handled immediately.

Gameplay data rule: do not fake or hardcode gameplay values for parity work.
Use DB data, DBC/source-derived values, or CMaNGOS formulas. If the real data
source is not wired yet, leave behavior unimplemented or narrowly guarded and
log the follow-up.

## Recently Landed Or Confirmed

- Quest status markers, quest source items, quest reputation rewards, and
  gameobject quest pickups are working in the real client.
- Combat resource feel has improved: damage-based rage generation, overkill
  fields, max-HP regen cap sync, immediate first-swing scheduling, no
  attack-rage from Heroic-Strike-style rage-spending swings, delayed Heroic
  Strike rage spend, and map-owned swing timers that cannot be accelerated by
  toggling autoattack.
- Creature fidelity: DB-backed walk/run speed, CMaNGOS-style DBC display scale
  fallback, and virtual item/equipment bytes for weapon visuals are integrated.
- Loot fidelity: DB-backed multi-drop creature/gameobject loot and variable
  copper are integrated.
- Combat skill math: character skills load on login, PvE weapon/defense
  skill-ups follow CMaNGOS two-roll logic with Intellect bonus for weapon
  skills, melee hit tables use skill vs defense, and level-up skill caps now
  update the client immediately.
- Trainer/gossip parity: trainer gossip uses the book icon and
  `I seek training.`, trainer list greetings come from DB/fallback text, and
  buying a trainer spell sends source-backed visual/impact packets before
  `SMSG_TRAINER_BUY_SUCCEEDED`.

## Current Slice Details

- `wow-db` now exposes `SpellTemplateQuery` over the local
  `spell_template` table exported from `Spell.dbc`.
- `ObjectMgr` now caches spell templates, matching the existing quest/loot
  immutable world-data cache shape.
- Starter spell execution now derives from CMaNGOS fields:
  - on-next-swing from `SPELL_ATTR_ON_NEXT_SWING*`;
  - power type and cost from `PowerType` / `ManaCost`;
  - GCD from `StartRecoveryCategory` / `StartRecoveryTime`;
  - spell cooldown from `max(RecoveryTime, CategoryRecoveryTime)`;
  - simple damage/bonus values from DBC effect base points.
- Heroic Strike remains next-swing queued and spends rage when the swing fires,
  but its queue shape now comes from the spell template instead of a spell-id
  fixture.
- Battle Shout now proves the first aura vertical slice: source-backed 10 rage
  cost, 1.5s GCD, `SMSG_SPELL_GO`, visible positive aura slot update,
  `SpellDuration.dbc` duration lookup, map-owned aura expiration, and the
  CMaNGOS `SPELL_AURA_MOD_ATTACK_POWER` stat modifier path. Timed auras also
  send CMaNGOS-shaped `SMSG_UPDATE_AURA_DURATION` (`slot`, remaining
  milliseconds), fixing the real-client `0 sec remaining` display.
- `WorldDataFiles` now parses `dbc/SpellDuration.dbc` with CMaNGOS `niii`
  layout. `MapRuntime` owns active player aura state, keeps base combat stats
  separate from aura-modified combat stats, and expires auras from the map
  update loop so expiration packets can be sent without waiting for client
  input.

## Tests Run

- `cargo fmt`
- `cargo test -p wow-network starter`
- `cargo test -p wow-network battle_shout_uses_spell_template_gcd_cost_and_aura_slot`
- `cargo test -p wow-network spell_duration_dbc_parser_reads_cmangos_duration_fields`
- `cargo test -p wow-network map_owned_player_aura_applies_attack_power_mod_and_expires`
- `cargo test -p wow-network heroic_strike_cast_sends_spell_start_until_next_swing`
- `cargo test -p wow-network` passed with 334 lib tests.
- First `.\scripts\test-rust.cmd` reached tests/checks but failed the final
  `authserver` build because the old running server locked
  `target\debug\authserver.exe` on Windows.
- Stopped the old auth/world server processes, then reran
  `.\scripts\test-rust.cmd`; it passed.
- `.\scripts\run-client-stack-18085.cmd -NoAutoRestart` restarted the client
  stack successfully after the aura duration/stat-mod slice and again after
  adding `SMSG_UPDATE_AURA_DURATION`.

## Real-Client Success Criteria For Current Smoke

- Log in through `set realmlist 127.0.0.1:13724`.
- Warrior with Heroic Strike:
  - queuing Heroic Strike should send the cast/start feedback immediately;
  - rage should not be consumed until the next melee swing lands;
  - toggling autoattack must not accelerate swing timers.
- Warrior with Battle Shout after training:
  - cast should require enough rage and spend 10 rage;
  - cast should trigger GCD;
  - client should receive spell visual/GO and show the positive aura icon;
  - Attack Power should gain the Battle Shout positive modifier while the aura
    is active;
  - the aura tooltip should show a real countdown rather than `0 sec remaining`;
  - when the DBC duration expires, the aura icon and AP modifier should clear
    without needing another client action.
- Re-check prior parity while in the same session: gray unavailable quest `!`,
  no trainer disconnect on `Train me`, correct creature scale/equipment
  animation, variable copper, combo loot, skill cap UI updates, Simple Letter on
  accept, and quest reputation feedback.

## Known Follow-Ups

- Battle Shout now covers duration, expiration, and flat AP stat modification.
  Aura persistence across logout/relog, other aura modifier families, dispel
  rules, charges/stacks, periodic ticks, and aura save/load remain future spell
  system slices.
- The spell pipeline should next cover level 1-6 warrior abilities from
  `spell_template`: Rend, Charge, Thunder Clap, stances, Overpower availability,
  and any trainer-learned spell edge cases.
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
- `crates/wow-network/src/world/globals/object_mgr.rs`
- `crates/wow-network/src/world/session.rs`
- `crates/wow-network/src/world/spells.rs`
- `crates/wow-network/src/world/tests.rs`
- `docs/playable_execution_roadmap.md`
