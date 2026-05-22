# Session Handoff

Short operating brief for the next Rust migration session. Durable roadmap
history belongs in `docs/rust_migration_plan.md`, gate status in
`docs/playable_gate_board.md`, and focused feature plans in their own docs.

## Current Branch And State

- Branch: `codex/auctionhouse`
- Workspace: `C:\Users\subhe\Documents\mangos-worktrees\auctionhouse`
- Branch now contains the latest `codex/rusty-mangos` spell-system baseline
  plus commit `95765e16b Implement auction house parity flow`.
- Current state is the auction-house implementation rebased by merge onto the
  current integration branch. The only manual merge touch was this handoff.

## Current Goal

Latest user-directed priority: bring the auction house branch into
`codex/rusty-mangos`, test it on top of the current integration baseline, and
merge it once the integrated result is good.

## What Changed Recently

- Inherited the current generic spell-system baseline from
  `codex/rusty-mangos`, including the recent `SpellPlan` sweep, cone targeting,
  absorb combat-log fixes, Evocation-style mana regen modifiers, and
  Counterspell school lockout work.
- Fixed the first live auction-create session crash after landing AH support:
  `create_auction_from_inventory` now casts the `MAX(id) + 1` auction id query
  to `UNSIGNED`, avoiding MySQL `DECIMAL` decode mismatch when the first create
  request allocates a new auction row.
- Fixed the next live cancel-auction session crash after create:
  auction existence probes in the DB layer now decode `SELECT 1 ... LIMIT 1`
  as integer presence checks instead of `u8`, avoiding MySQL `INT` vs
  `TINYINT UNSIGNED` decode mismatches in cancel, bid, and expiry paths.
- Fixed the next live mail-access session crash after auction cancel:
  mail-item reads now cast signed `mail_items` ids/templates and `MAX(id)`
  allocators to unsigned values where Rust expects unsigned ids, avoiding
  `item_guid` decode mismatches when opening auction-generated mail.
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
  - opens the AH from both direct `MSG_AUCTION_HELLO` and gossip service
    selection,
  - reads browse data from `auction` plus `item_instance`,
  - pages owner, bidder, and search results with CMaNGOS-style list behavior.
- Added auction mutation flows:
  - sell/create with DBC-backed deposit data, inventory ownership transfer, and
    auction row creation in one DB transaction,
  - cancel/remove with bidder refund mail, owner return mail, cancel cut, and
    online notifications,
  - bid/buyout with increment validation, self-raise delta charging, outbid
    refund mail, buyout settlement mail, and live owner/bidder notifications.
- Added world-owned expiry processing:
  - expired auctions settle once globally from the map tick,
  - no-bid expirations return the item to the owner by mail,
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
  - DB rows keep the real entry `houseid`,
  - alliance city houses share one market,
  - horde city houses share one market,
  - neutral goblin houses stay separate,
  - when `AllowTwoSide.Interaction.Auction = true`, all houses share the global
    market while still using the entry house id for UI and DBC rates.

## Tests Run

- Pre-merge integration baseline in `codex/rusty-mangos`:
  - `.\scripts\test-rust.cmd`
- Auction branch before merge:
  - `cargo fmt -p wow-config -p wow-db -p wow-network -p worldserver`
  - `cargo check -p wow-network -p wow-config -p worldserver`
  - `cargo test -p wow-config world_config -- --nocapture`
  - `cargo test -p wow-network auction -- --nocapture`
  - `cargo test -p wow-network parse_world_client_packet_decodes_control_requests -- --nocapture`
- Still needed on the integrated branch after this merge-up:
  - rerun focused auction/config coverage,
  - rerun `.\scripts\test-rust.cmd`,
  - perform live 1.12 client smoke for auction flows.
- Post-landing auction-create hotfix validation:
  - `cargo check -p wow-db -p wow-network -p worldserver`
  - `cargo test -p wow-network auction -- --nocapture`
  - `.\scripts\restart-game-stack.cmd --release`
- Post-landing auction-cancel hotfix validation:
  - `cargo check -p wow-db -p wow-network -p worldserver`
  - `cargo test -p wow-network auction -- --nocapture`
  - `.\scripts\restart-game-stack.cmd --release`
- Post-landing auction-mail hotfix validation:
  - `cargo check -p wow-db -p wow-network -p worldserver`
  - `cargo test -p wow-network mail -- --nocapture`
  - `cargo test -p wow-network auction -- --nocapture`
  - `.\scripts\restart-game-stack.cmd --release`

## Known Blockers / Unproven Areas

- The merged code still needs post-integration validation on this branch before
  it should be merged back into `codex/rusty-mangos`.
- Full auction flow remains unproven against a live 1.12 client session:
  browse, sell, cancel, bid, buyout, outbid mail, grouped markets, neutral
  separation, optional global market, and expiry settlement all still need
  smoke confirmation.
- The previous first-create crash should now be fixed; re-test live auction
  creation before chasing any later AH issues.
- The previous cancel-auction crash should now be fixed; re-test live cancel
  before chasing any later AH issues.
- The previous mail-access crash after auction cancel should now be fixed;
  re-test opening the generated mail before chasing any later AH issues.
- Search `usable` filtering still does not include the extra CMaNGOS
  recipe-known suppression path.
- The unrelated local MySQL auth issue can still block the two known EventAI
  immolate tests in broad DB-backed runs if those tests are exercised again.

## Recommended Next Task

Validate the integrated auction branch, then merge it into
`codex/rusty-mangos` if clean:

- run focused auction/config/protocol tests on this merged branch,
- run `.\scripts\test-rust.cmd`,
- smoke faction, neutral, and optional cross-faction auction access in a live
  client,
- fix any packet or UI mismatches directly on `codex/auctionhouse`,
- merge `codex/auctionhouse` back into `codex/rusty-mangos` once the integrated
  result is proven.

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
- `crates/wow-network/src/world/spells/plan.rs`
- `crates/wow-network/src/world/tests/spells.rs`
- `crates/wow-network/src/world/tests/query_gossip_data.rs`
- `src/game/AuctionHouse/AuctionHouseHandler.cpp`
- `src/game/AuctionHouse/AuctionHouseMgr.cpp`
- `src/game/World/World.cpp`
