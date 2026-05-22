# Session Handoff

Short operating brief for the next Rust migration session. Durable roadmap
history belongs in `docs/rust_migration_plan.md`, gate status in
`docs/playable_gate_board.md`, and focused feature plans in their own docs.

## Current Branch And State

- Branch: `codex/auctionhouse`
- Workspace: `C:\Users\subhe\Documents\mangos-worktrees\auctionhouse`
- Base checkpoint for this worker branch: `58851c5fd`
- Current uncommitted state completes auction house Phase 1 and Phase 2 on the
  code side, including config parity knobs and grouped market behavior for
  alliance, horde, neutral, and optional cross-faction auction access.

## Current Goal

Latest user-directed priority: implement auction house support on this dedicated
branch, keeping parity with CMaNGOS and isolating the work from the main dirty
integration workspace.

## What Changed Recently

- Added Classic auction protocol coverage in `wow-proto` and `wow-network` for:
  - `MSG_AUCTION_HELLO`
  - `CMSG_AUCTION_LIST_ITEMS`
  - `CMSG_AUCTION_LIST_OWNER_ITEMS`
  - `CMSG_AUCTION_LIST_BIDDER_ITEMS`
  - `CMSG_AUCTION_SELL_ITEM`
  - `CMSG_AUCTION_REMOVE_ITEM`
  - `CMSG_AUCTION_PLACE_BID`
  - `SMSG_AUCTION_LIST_RESULT`
  - `SMSG_AUCTION_OWNER_LIST_RESULT`
  - `SMSG_AUCTION_BIDDER_LIST_RESULT`
  - `SMSG_AUCTION_COMMAND_RESULT`
  - `SMSG_AUCTION_REMOVED_NOTIFICATION`
  - `SMSG_AUCTION_OWNER_NOTIFICATION`
- Added a dedicated world auction handler that:
  - validates auctioneer interaction against live creature snapshots,
  - opens the AH both from direct `MSG_AUCTION_HELLO` and gossip service
    selection,
  - reads browse data from `auction` plus `item_instance`,
  - pages owner, bidder, and search results with CMaNGOS-compatible list
    behavior.
- Added Phase 2 mutation flows:
  - sell/create with DBC-backed deposit data, inventory ownership transfer, and
    auction row creation in one DB transaction,
  - cancel/remove with bidder refund mail, owner return mail, cancel cut, and
    online notifications,
  - bid/buyout with increment validation, self-raise delta charging, outbid
    refund mail, buyout settlement mail, and live owner/bidder notifications.
- Added world-owned expiry processing:
  - expired auctions are discovered and settled once globally from the map tick,
  - no-bid auctions return the item to the owner by mail,
  - sold expirations mail owner profit and winner item delivery,
  - online owners and bidders receive the same live notifications as direct
    mutation paths.
- Added `AuctionHouse.dbc` parsing so deposit and cut percentages are
  data-backed instead of guessed.
- Wired the missing CMaNGOS auction config knobs through `wow-config`,
  `worldserver`, and world runtime state:
  - `AllowTwoSide.Interaction.Auction`
  - `Rate.Auction.Time`
  - `Rate.Auction.Deposit`
  - `Rate.Auction.Cut`
  - `Auction.Deposit.Min`
- Corrected market grouping behavior to match CMaNGOS ownership boundaries:
  - DB rows continue to store the real entry `houseid`,
  - alliance city houses share one market,
  - horde city houses share one market,
  - neutral goblin houses stay separate,
  - when `AllowTwoSide.Interaction.Auction = true`, all houses share the global
    market while still using the entry house id for UI and DBC rates.
- Refactored auction handler dependencies into small dep structs so the parity
  slice stays clippy-clean.

## Tests Run

- Focused parity/config validation:
  - `cargo fmt -p wow-config -p wow-db -p wow-network -p worldserver`
  - `cargo check -p wow-network -p wow-config -p worldserver`
  - `cargo test -p wow-config world_config -- --nocapture`
  - `cargo test -p wow-network auction -- --nocapture`
  - `cargo test -p wow-network parse_world_client_packet_decodes_control_requests -- --nocapture`
- Broad workspace baseline:
  - `.\scripts\test-rust.cmd`
    - clippy/check passed,
    - same known local baseline failure remains:
      - `world::tests::map_runtime_manager_advances_3196_event_ai_immolate_with_delayed_completion`
      - `world::tests::map_runtime_direct_completion_after_manager_started_3196_immolate_does_not_hang`
    - cause remains local MySQL auth:
      `1698 (28000): Access denied for user 'root'@'localhost'`

## Known Blockers / Unproven Areas

- Code-side auction Phase 2 is complete, but the full AH flow is still unproven
  against a live 1.12 client session in this worktree.
- Search `usable` filtering still does not include the extra CMaNGOS
  recipe-known suppression path.
- Full workspace green remains blocked locally by the unrelated MySQL auth issue
  on the two EventAI immolate tests above.

## Recommended Next Task

Run live 1.12 client smoke for the completed AH stack:

- open faction and neutral auctioneers,
- create auctions with multiple durations and confirm deposit behavior,
- cancel auctions with and without active bids,
- bid, outbid, and buy out across two characters,
- verify grouped city-house visibility and neutral separation,
- flip `AllowTwoSide.Interaction.Auction` and confirm all houses share one
  market,
- let an auction expire and confirm settlement mail plus online notifications.

If live testing reveals packet or UI mismatches, fix those directly on this
branch before considering it ready to merge back.

## Key Files

- `crates/wow-network/src/world/handlers/auction.rs`
- `crates/wow-network/src/world/handlers/gossip.rs`
- `crates/wow-network/src/world/handlers/npc.rs`
- `crates/wow-network/src/world/server/map_update.rs`
- `crates/wow-network/src/world/map_runtime/world_data.rs`
- `crates/wow-network/src/world/mod.rs`
- `crates/wow-network/src/world/session_runtime.rs`
- `crates/wow-proto/src/world_packets.rs`
- `crates/wow-db/src/character/auction.rs`
- `crates/wow-config/src/lib.rs`
- `bins/worldserver/src/main.rs`
- `src/game/AuctionHouse/AuctionHouseHandler.cpp`
- `src/game/AuctionHouse/AuctionHouseMgr.cpp`
- `src/game/World/World.cpp`
