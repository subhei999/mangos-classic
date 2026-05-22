# Session Handoff

Short operating brief for the next Rust migration session. Durable roadmap
history belongs in `docs/rust_migration_plan.md`, gate status in
`docs/playable_gate_board.md`, and focused feature plans in their own docs.

## Current Branch And State

- Branch: `codex/rusty-mangos`
- Workspace: `C:\Users\subhe\Documents\New project`
- Latest pushed checkpoint before the current uncommitted work:
  `31cde46d6 Filter launcher probe noise from world logs`
- Current uncommitted state:
  - prior melee/AP parity work:
    - `crates/wow-network/src/world/constants.rs`
    - `crates/wow-network/src/world/entities/player.rs`
    - `crates/wow-network/src/world/tests/combat_core.rs`
  - current inventory/vendor parity work:
    - `crates/wow-db/src/world_data.rs`
    - `crates/wow-network/src/world/entities/item.rs`
    - `crates/wow-network/src/world/handlers/gm.rs`
    - `crates/wow-network/src/world/handlers/gossip.rs`
    - `crates/wow-network/src/world/handlers/inventory.rs`
    - `crates/wow-network/src/world/handlers/loot.rs`
    - `crates/wow-network/src/world/handlers/mail.rs`
    - `crates/wow-network/src/world/handlers/npc.rs`
    - `crates/wow-network/src/world/handlers/quest.rs`
    - `crates/wow-network/src/world/handlers/vendor.rs`
    - `crates/wow-network/src/world/mod.rs`
    - `crates/wow-network/src/world/packet_builders/loot.rs`
    - `crates/wow-network/src/world/packets.rs`
    - `crates/wow-network/src/world/server/dispatch.rs`
    - `crates/wow-network/src/world/session_runtime.rs`
    - `crates/wow-network/src/world/spells/effects/items.rs`
    - `crates/wow-network/src/world/tests/character_inventory_social.rs`
    - `crates/wow-network/src/world/tests/loot_vendor_trainer.rs`
    - `crates/wow-proto/src/world_packets.rs`
  - this handoff:
    - `docs/session_handoff.md`

## Current Goal

Latest user-directed priority: stabilize secondary-bag inventory by moving
toward an authoritative bag model instead of continuing one-off bag bug fixes.
The immediate symptom remains the live-client report that swapping items in
non-backpack bags can leave the swapped item gray in its new slot. The same
session also includes the recent inventory/vendor mismatch work:

- vendor targeted buys into secondary-bag slots,
- buyback restore state when the item lands in a secondary bag,
- quiver/ammo pouch equip and storage restrictions,
- limited-quantity vendor stock.

## What Changed Recently

- Added an authoritative Rust-side bag boundary in
  `crates/wow-network/src/world/handlers/inventory.rs`:
  - `InventoryPosition` names `(bag, slot)` coordinates explicitly,
  - `InventoryStorageScope` distinguishes inventory storage from bank storage,
  - `InventoryBagModel` owns equipped bags, bank bags, purchased bank bag slot
    count, slot ranges, bag-family acceptance, move-position validity, and store
    planning,
  - `InventoryUpdatePlan` owns dirty player slots, dirty container slots, and
    contained-item updates.
- Migrated inventory swap validation, bag-icon autostore resolution, inventory
  store planning, bank store planning, and inventory update block generation to
  route through the model/update plan. Removed several now-dead scattered helper
  paths from the production build.
- Added `build_container_slots_update_block` so item-owned container fields can
  be coalesced as one object update when several slots in the same bag change.
- Added multi-position inventory updates so removal/restore paths can coalesce
  all touched slots through one `InventoryUpdatePlan`, instead of rebuilding
  bag/player updates one slot at a time.
- Added `build_stored_item_create_update_blocks` as the common create/update
  path for newly stored items. It emits the new item plus the model-owned
  player/container position updates, so callers no longer hand-assemble owner
  and contained GUID fields differently.
- Migrated the main item-grant/store callers onto `InventoryBagModel`
  autostore planning and the shared create/update helper:
  - generic vendor buys, buy-in-slot, and buyback restore planning,
  - quest reward item grants,
  - GM add-item,
  - loot grants and loot packet-builder grants,
  - mail item take and text-item creation,
  - spell-created items.
- Migrated quest source-item grant create updates, quest required-item removal
  updates, mail attachment removal updates, vendor full-sell slot clears, and
  buyback restore slot updates onto the common model-owned update helpers.
- The new model test caught and fixed a real architecture bug: unpurchased bank
  bag contents were treated as addressable by the model's non-root storage check.
- Fixed the likely secondary-bag swap gray-item source: swaps inside the same
  equipped bag were emitting multiple value-update blocks for the same container
  item in one `SMSG_UPDATE_OBJECT`. The update builder now coalesces changed
  container slots per bag object, matching the existing bag-0 player-field
  coalescing and the CMaNGOS dirty-object shape more closely.
- Added a regression test proving an internal secondary-bag swap sends one bag
  container update block with both changed slot GUIDs, followed by contained
  updates for the moved items.
- Added a regression test proving two removed slots in the same equipped bag
  also send one coalesced container update block, which closes the same class of
  duplicate/stale secondary-bag update bug for removal flows.
- Fixed a live-found secondary-bag sell bug: a bag item stored in secondary-bag
  slot `19` was rejected as "merchant doesn't want that item" because the sell
  path treated the item slot number as an equipped bag id and thought the bag was
  non-empty. Sell validation now only checks contained items when the sold bag
  item is actually in an equipped/bank bag slot.
- Current explanation for why secondary-bag bugs cluster: backpack slots are
  direct player inventory fields, while secondary bags are item-owned container
  fields plus contained-item fields plus DB `bag`/`slot` rows. Code paths that
  only think in `(bag, slot)` can look symmetric while still missing a container
  object update, over-sending duplicate updates, or skipping bag-family/equipped
  bag validation.
- Confirmed the buyback gray-item bug source: buyback restore into a secondary
  bag was sending a fresh item create block for an already-existing item, which
  duplicated the client update path. The restore flow now only sends position /
  contained updates for existing buyback items.
- Tightened bag-slot equip validation so bag slots apply item use requirements
  just like CMaNGOS. High-level bags no longer bypass `RequiredLevel`, and
  quiver / ammo pouch equip uniqueness now follows the CMaNGOS special-case
  restriction.
- Tightened manual inventory swap validation so special bags cannot be bypassed
  by drag-and-drop. Quivers and ammo pouches now reject non-matching contents on
  the direct move/swap path, not just autostore.
- Added runtime limited-stock tracking for vendor items using DB `maxcount`,
  `incrtime`, and item `BuyCount`, including restock behavior modeled after the
  CMaNGOS creature vendor logic.
- Added the missing `CMSG_BUY_ITEM_IN_SLOT` path so targeted vendor purchases
  into a chosen bag slot are parsed, dispatched, validated, and stored instead
  of relying only on the generic autostore buy path.
- Extended vendor inventory list / buy responses so limited-stock vendors now
  report live remaining counts instead of behaving as unlimited.

## Tests Run

- `cargo fmt -p wow-network -p wow-proto -p wow-db`
- `cargo fmt -p wow-network`
- Focused Rust tests:
  - `cargo test -p wow-network inventory_bag_model_owns_scope_ranges_and_bank_purchase_state -- --nocapture`
  - `cargo test -p wow-network equipped_bag_internal_swap_coalesces_container_slot_updates -- --nocapture`
  - `cargo test -p wow-network equipped_bag_multi_destroy_update_coalesces_container_slots -- --nocapture`
  - `cargo test -p wow-network equipped_bag -- --nocapture`
  - `cargo test -p wow-network inventory_store_plan -- --nocapture`
  - `cargo test -p wow-network bank_store_plan -- --nocapture`
  - `cargo test -p wow-network quest_source_item -- --nocapture`
  - `cargo test -p wow-network quest_reward_storage -- --nocapture`
  - `cargo test -p wow-network create_item -- --nocapture`
  - `cargo test -p wow-network mail -- --nocapture`
  - `cargo test -p wow-network inventory_bag_slot_validation_applies_use_requirements_and_quiver_uniqueness -- --nocapture`
  - `cargo test -p wow-network inventory_special_bag_storage_validation_rejects_nonmatching_items -- --nocapture`
  - `cargo test -p wow-network inventory_store_plan_uses_last_secondary_bag_slot -- --nocapture`
  - `cargo test -p wow-network vendor_limited_stock_helpers_decrement_and_restock_like_cmangos -- --nocapture`
  - `cargo test -p wow-network vendor_limited_stock_helper_rejects_sold_out_purchase -- --nocapture`
  - `cargo test -p wow-network vendor_buy_item_in_slot_plan_accepts_last_secondary_bag_slot -- --nocapture`
  - `cargo test -p wow-network vendor_sell_bag_in_secondary_bag_slot_matching_bag_id_is_not_treated_as_non_empty -- --nocapture`
  - `cargo test -p wow-network buyback -- --nocapture`
  - `cargo test -p wow-network parses_buy_item_in_slot_packet -- --nocapture`
  - `cargo test -p wow-network db_vendor_inventory_uses_cmangos_list_shape -- --nocapture`
- `.\scripts\test-rust.cmd`
  - current result after the secondary-bag swap fix: only the two pre-existing
    local DB-auth failures remain:
    - `world::tests::map_runtime_manager_advances_3196_event_ai_immolate_with_delayed_completion`
    - `world::tests::map_runtime_direct_completion_after_manager_started_3196_immolate_does_not_hang`
  - failure cause remains local MySQL auth in this environment:
    `1698 (28000): Access denied for user 'root'@'localhost'`

## Known Blockers / Unproven Areas

- Full workspace green is still blocked locally by the unrelated MySQL auth
  issue on the two EventAI immolate tests above.
- The authoritative bag model is now in place for storage planning/update
  planning, and the main item-grant/removal callers now use it. A few
  compatibility helpers remain inside the inventory handler for single-position
  move/destroy cases, and the vendor sell split path still creates an item into
  a buyback slot, which is a vendor holding slot rather than normal bag storage.
- The secondary-bag swap fix is packet-shape covered, but still needs a live
  client smoke test: swap two occupied slots inside the same equipped bag and
  confirm neither icon remains gray/pending.
- The new `CMSG_BUY_ITEM_IN_SLOT` path is covered by parsing and slot-planning
  tests, but it has not yet been smoke-tested in a live client against a real
  vendor UI.
- The buyback secondary-bag fix is validated from the update-path analysis and
  surrounding inventory/update tests, but it still deserves a direct in-client
  sell -> buyback -> swap smoke test to confirm the gray pending-state symptom
  is gone.
- The secondary-bag last-slot bag sell fix is unit-covered for the slot-id
  collision, but still needs the exact live repro repeated after rebuild:
  move/buy a bag into the last slot of a secondary bag, then sell it.

## Recommended Next Task

Recommended next task: keep consolidating inventory-adjacent callers onto
`InventoryBagModel` and `InventoryUpdatePlan`, then run an in-client smoke pass
on the five reported vendor/inventory scenarios:

- swap two occupied slots inside the same secondary bag,
- buy a vendor item directly into the last slot of a secondary bag,
- sell -> buyback -> swap when buyback restores into a secondary bag,
- equip / use quivers and ammo pouches across level and uniqueness boundaries,
- buy out and restock a limited-quantity vendor item.

If any mismatch remains, compare the live packet/update sequence with the
CMaNGOS path now that the major ownership and stock behaviors are in place.

## Key Files

- `crates/wow-network/src/world/handlers/vendor.rs`
- `crates/wow-network/src/world/handlers/inventory.rs`
- `crates/wow-network/src/world/handlers/npc.rs`
- `crates/wow-network/src/world/handlers/gossip.rs`
- `crates/wow-network/src/world/packets.rs`
- `crates/wow-network/src/world/session_runtime.rs`
- `crates/wow-db/src/world_data.rs`
- `crates/wow-proto/src/world_packets.rs`
- `src/game/Entities/ItemHandler.cpp`
- `src/game/Entities/Player.cpp`
- `src/game/Entities/Creature.cpp`
