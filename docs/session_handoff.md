# Session Handoff

Short operating brief for the next Rust migration session. Durable roadmap
history belongs in `docs/rust_migration_plan.md`, gate status in
`docs/playable_gate_board.md`, and branch split details in
`docs/playable_execution_roadmap.md`.

## Current Branch And Worktree

- Branch: `codex/multiplayer-cross-action-parity`, tracking
  `origin/codex/multiplayer-cross-action-parity`.
- Integration target remains `codex/rusty-mangos`.
- Playerbots are disabled by default for normal multiplayer/Northshire testing:
  `config/worldserver.local.toml` has `[playerbots] enabled = false` and
  `[playerbots.random] enabled = false`; the stack launcher keeps them off
  unless explicitly passed a playerbot flag.
- Current user-directed priority: finish Northshire multiplayer/gameplay parity
  issues found by real-client testing, using CMaNGOS as the behavior reference
  and keeping shared world authority in `MapRuntime`.
- Current local work implements a parallel issue sweep across inventory,
  spell lifecycle, spell proc/aura behavior, creature display gender, and local
  addon cleanup. It is tested and ready to commit.

## Recent Implemented Work

- Multiplayer cross-player parity, already committed and pushed, fixed nearby
  text-emote delivery, observer emote animations/states, looting visual flags,
  stop/turn orientation broadcasting, stand/sit/sleep/kneel observer state, and
  stale auto-attack cleanup after DB-creature death.
- Current inventory slice adds equipped secondary bag capacity, auto-equipping
  bags into available bag slots, dropping gear onto a bag icon to place it in
  the first valid contained slot, vendor/autostore stack merging, and tests for
  stack merge plus equipped bag storage.
- Current spell lifecycle slice adds CMaNGOS-style cast pushback from creature
  melee hits (`SMSG_SPELL_DELAYED`), auto-standing when casting from sit/sleep,
  and nearby observer cleanup packets when an active cast is interrupted by
  movement/cancel so lingering hand/fireball animations clear.
- Current spell proc slice loads DB-backed proc trigger metadata from
  `spell_template` and applies triggered aura spells from successful creature
  melee hits against players. Frost Armor-style `SPELL_AURA_PROC_TRIGGER_SPELL`
  now applies its triggered effect to the attacker when proc flags/chance allow.
- Current creature display slice loads `DisplayIdProbability*`,
  `creature_model_info.gender`, and `modelid_other_gender`, then serializes the
  selected display id/gender in DB-creature create blocks. Respawn reselects a
  native display and movement-script morph reset returns to that native display.
- Local addon cleanup removed
  `C:\World of Warcraft Classic\Interface\AddOns\NorthshireAuraTimers`. No repo
  commit is needed for that file-system-only cleanup.
- Full `.\scripts\test-rust.cmd` passed after stopping stale local
  `authserver.exe`/`worldserver.exe` processes that had locked target binaries.

## Tests Run

- `cargo fmt --package wow-db --package wow-network --check`
- `cargo check -p wow-network`
- `cargo check -p wow-db`
- `cargo test -p wow-network inventory --lib`
- `cargo test -p wow-network spell --lib`
- `cargo test -p wow-network combat --lib`
- `cargo test -p wow-network creature --lib`
- `git diff --check`
- `.\scripts\test-rust.cmd`

## Current Follow-Ups

- Real-client smoke the new fixes: equip a bag, buy duplicate bread stacks,
  drag equipped gear onto a bag icon, cast while sitting, get hit while casting,
  cancel/move during a cast while another player watches, and test Frost Armor
  or another proc-on-hit aura against a creature.
- Debuff timers on hostile creature target portraits remain unimplemented or
  client-limited until proven otherwise. CMaNGOS sends aura duration updates to
  player aura targets; do not fake hostile portrait timers with an addon.
- Full spell outcome infrastructure is still needed: spell crit, full resist,
  partial resist, absorb, and miss result structs should be added before
  `apply_player_direct_damage_effect` and `MapRuntime::apply_db_creature_damage`
  branch further.
- Creature radius/reach still mostly use first-display template fallback. A
  later visual-size slice should derive radius/reach from the selected model row
  where CMaNGOS does.
- Loot autostore paths still have backpack-heavy legacy branches. Move them
  onto the shared inventory store planner when the next loot/inventory slice is
  opened.
- Continue Northshire missing criteria from the playable board: quest
  availability restrictions, quest item drops from real loot tables,
  gameobject quest pickup, remaining warrior level 1-6 spell parity, combat log
  feedback, health/rage regen behavior, skills/weapon skills,
  CMaNGOS-like aggro/chase/leash behavior, and patrol runtime stability.

## Key Files

- `crates/wow-db/src/world_data.rs`
- `crates/wow-network/src/world/inventory.rs`
- `crates/wow-network/src/world/vendors.rs`
- `crates/wow-network/src/world/spells.rs`
- `crates/wow-network/src/world/spells/spell_mgr.rs`
- `crates/wow-network/src/world/combat/aggro.rs`
- `crates/wow-network/src/world/combat/runtime.rs`
- `crates/wow-network/src/world/entities/creature.rs`
- `crates/wow-network/src/world/maps/map.rs`
- `crates/wow-network/src/world/maps/map_manager.rs`
- `crates/wow-network/src/world/maps/map/creature_motion.rs`
- `crates/wow-network/src/world/server/session_loop.rs`
- `crates/wow-network/src/world/server/movement.rs`
- `crates/wow-network/src/world/session.rs`
- `crates/wow-network/src/world/opcodes.rs`
- `crates/wow-network/src/world/packet_builders/combat.rs`
- `crates/wow-network/src/world/tests.rs`
- `docs/playable_gate_board.md`
- `docs/playable_execution_roadmap.md`
