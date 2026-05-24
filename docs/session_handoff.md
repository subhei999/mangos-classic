# Session Handoff

Short operating brief for the next Rust gameplay-parity session. Keep this file
concise; durable gate status belongs in `docs/playable_gate_board.md`.

## Current Branch And State

- Branch: `codex/rusty-mangos`
- Workspace: `C:\Users\subhe\Documents\New project`
- Current state: workspace remains broadly dirty with many pre-existing spell,
  combat, taxi, and progression edits. This pass added focused uncommitted gear
  stat, random-property enchant stat, and item binding work on top. Inspect
  `git status --short --branch` before editing and do not revert unrelated files.

## Current Goal

User-directed priority: focused pass on gear/stats and BOE/BOP parity with
CMaNGOS behavior.

- Gear primary stats from `item_template` should affect character effective
  stats, max health/mana, armor/AP/crit/dodge, and runtime updates.
- Random-property / enchantment stats from `item_instance.enchantments` and
  `SpellItemEnchantment.dbc` should affect equipped character stats.
- Bind-on-pickup / quest items should be created soulbound.
- Bind-on-equip items should become soulbound when equipped, and item object
  update fields should expose the bound flag to the client.

## What Changed Recently

- `CharacterInventoryItem` now carries `item_instance.flags` so item create and
  value updates can serialize `ITEM_FIELD_FLAGS`.
- Added initial item flags to inventory creation requests and set BOP / quest
  item flags from real `item_template.bonding` data in loot, vendor, quest,
  GM additem, and create-item spell paths.
- Equipment recomputation now folds CMaNGOS item stat ids:
  mana, health, agility, strength, intellect, spirit, and stamina.
- Login and equipment moves now compute effective world stats from DB base
  stats plus equipped gear plus active auras.
- Equipment swaps now refresh world-stat and combat-stat packets and update
  map runtime stats; BOE/BOP/quest equipment binds the item in `item_instance`.
- Mail attachments now reject soulbound items from `item_instance.flags`, in
  line with CMaNGOS `Item::CanBeTraded`; wrapped items remain mailable except
  for COD.
- Auction sell validation now rejects soulbound, timed, conjured, and non-empty
  container items without treating every unrelated item flag as unsellable.
- Stack merges for bind-on-pickup / quest items now bind the surviving
  destination stack, covering old unbound stacks without inventing data.
- `WorldDataFiles` now loads `SpellItemEnchantment.dbc`; equipped item loading
  folds stat and resistance enchantment effects from `item_instance.enchantments`
  so random-property items such as "of Strength" / "of the Bear" affect world
  stats on login and equipment changes.
- Runtime aura recomputation now treats DB base stats plus equipped item/enchant
  stats as the base stat line, so active aura refreshes do not drop gear stats.

## Tests Run

- `CARGO_INCREMENTAL=0 CARGO_TARGET_DIR=target-codex cargo test -p wow-network equipped_item_primary_stats_feed_world_and_combat_stats_like_cmangos -- --nocapture`
- `CARGO_INCREMENTAL=0 CARGO_TARGET_DIR=target-codex cargo test -p wow-network random_property_stat_enchantments_feed_world_stats_like_cmangos -- --nocapture`
- `CARGO_INCREMENTAL=0 CARGO_TARGET_DIR=target-codex cargo test -p wow-network spell_item_enchantment_dbc_parser_reads_stat_effects -- --nocapture`
- `CARGO_INCREMENTAL=0 CARGO_TARGET_DIR=target-codex cargo test -p wow-network item_binding_rules_match_cmangos_pickup_and_equip_boundaries -- --nocapture`
- `CARGO_INCREMENTAL=0 CARGO_TARGET_DIR=target-codex cargo test -p wow-network item_create_block_includes_soulbound_instance_flag -- --nocapture`
- `CARGO_INCREMENTAL=0 CARGO_TARGET_DIR=target-codex cargo test -p wow-network inventory_recomputed_combat_stats_keep_passive_resistance_on_self_update -- --nocapture`
- `CARGO_INCREMENTAL=0 CARGO_TARGET_DIR=target-codex cargo test -p wow-network mail_attachment_transfer_rejects_soulbound_like_cmangos_can_be_traded -- --nocapture`
- `CARGO_INCREMENTAL=0 CARGO_TARGET_DIR=target-codex cargo test -p wow-network mail_attachment_transfer_allows_wrapped_items_except_cod -- --nocapture`
- `CARGO_INCREMENTAL=0 CARGO_TARGET_DIR=target-codex cargo test -p wow-network auction_listing_rejects_soulbound_but_not_unrelated_instance_flags -- --nocapture`
- `.\scripts\test-rust.cmd`
  - failed in pre-existing dirty lanes on unrelated clippy/dead-code findings
    such as `unit_bytes_1`, unused creature aura / periodic damage helpers,
    gossip/spell packet `too_many_arguments`, and existing spell/combat clippy
    warnings.

## Known Blockers / Unproven Areas

- Full `test-rust.cmd` is not green in this workspace because unrelated dirty
  clippy warnings are promoted to errors.
- This pass implements only `SpellItemEnchantment.dbc` stat/resistance effects
  for equipped item enchantment slots. Enchantment equip spells, proc/combat
  spells, damage/totem weapon modifiers, bound-by-enchant behavior, and
  `ItemSet.dbc` bonuses remain future work.
- Existing unbound BOP stacks are guarded on future stack merges, but a DB
  cleanup/migration would still be needed for unmerged historical bad rows.
- Trade has no implemented item exchange path yet beyond cancel plumbing, so
  soulbound trade rejection remains future work with trade itself.

## Recommended Next Task

Continue gear parity by proving the real-client equip path:

- Equip a DB-backed stat item and confirm visible character stats, max
  health/mana, AP/crit/dodge, and relog state.
- Exercise a BOE item through backpack -> equip -> relog and confirm
  `item_instance.flags` plus client tooltip state.
- Add `ItemSet.dbc` loaders before attempting set bonuses or item set equip
  spells, and add CMaNGOS-shaped enchantment apply support for equip spells /
  proc spells / weapon damage modifiers when those systems are ready.

## Key Files

- `crates/wow-db/src/character/inventory.rs`
- `crates/wow-db/src/character/types.rs`
- `crates/wow-network/src/world/entities/player.rs`
- `crates/wow-network/src/world/entities/item.rs`
- `crates/wow-network/src/world/entities/update_data.rs`
- `crates/wow-network/src/world/handlers/inventory.rs`
- `crates/wow-network/src/world/handlers/mail.rs`
- `crates/wow-network/src/world/handlers/auction.rs`
- `crates/wow-network/src/world/server/player_login.rs`
- `crates/wow-network/src/world/map_runtime/world_data.rs`
- `crates/wow-network/src/world/map_runtime/map_manager/players.rs`
- `crates/wow-network/src/world/map_runtime/systems/players.rs`
- `crates/wow-network/src/world/tests/character_inventory_social.rs`
- CMaNGOS reference: `src/game/Entities/Player.cpp::_ApplyItemMods`,
  `_ApplyItemBonuses`, `ApplyEnchantment`, and `VisualizeItem`
