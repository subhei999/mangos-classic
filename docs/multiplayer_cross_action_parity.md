# Multiplayer Cross-Player Action Parity

Working audit for CMaNGOS-vs-Rust cross-player communication. This is a
discovery and implementation checklist, not a Northshire grading harness. The
user remains the final real-client judge for the playable slice.

## Status Key

- Green: implemented and automated proof exists.
- Yellow: implemented or partly implemented, but proof or edge coverage is
  incomplete.
- Red: missing or known non-parity behavior.
- P1: protocol, dupe, stale-state, or demo-breaking risk.
- P2: visible multiplayer parity gap likely to affect normal play.
- P3/P4: broader social or fidelity backlog.

## Matrix

| Area | Client action / opcode | CMaNGOS source | Expected cross-player packets | Rust source | Status | Automated proof | Real-client proof | Priority | Owner |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| Presence | Login nearby player / `CMSG_PLAYER_LOGIN` | `Map::Add(Player*)`, visibility visitors | Other players receive create; joining player receives nearby players | `server/player_login.rs`, `maps/map/players.rs` | Green | Two-client world and starter flow tests | User saw multiple players | P1 | MapRuntime |
| Presence | Logout/disconnect / `CMSG_LOGOUT_*` | `Player::CleanupsBeforeDelete`, visibility remove | Nearby players receive `SMSG_DESTROY_OBJECT` | `server/logout.rs`, `maps/map/players.rs` | Green-ish | Two-client logout visibility test | Needs final smoke | P1 | MapRuntime |
| Visibility | Range enter/leave by movement | CMaNGOS grid/cell visibility update | Create/destroy for players, creatures, corpses, gameobjects | `server/movement.rs`, `server/visibility.rs`, `maps/map/*snapshots.rs` | Yellow | G3/G12 tests | Movement verified; broader corpse/gameobject proof pending | P1 | MapRuntime |
| Movement | Walk/run/jump/fall/turn / `MSG_MOVE_*` | `MovementHandler.cpp`, `Unit::SendMessageToSet` | Nearby players receive movement opcode with mover guid and timing | `server/movement.rs`, `maps/map/players.rs` | Green-ish | Movement broadcast tests | User saw movement/jump basics | P1 | MapRuntime |
| Chat | `/say`, `/yell`, custom `/emote` / `CMSG_MESSAGECHAT` | `ChatHandler.cpp::HandleMessagechatOpcode` | `SMSG_MESSAGECHAT` to self and in-range listeners | `chat.rs::handle_message_chat` | Yellow | `/say` two-client proof; yell/emote need matrix proof | `/say` observed | P2 | Session + MapRuntime |
| Emotes | Built-in text emotes / `CMSG_TEXT_EMOTE` | `ChatHandler.cpp::HandleTextEmoteOpcode` | `SMSG_TEXT_EMOTE` plus `SMSG_EMOTE` or emote-state update to nearby listeners | `chat.rs::handle_text_emote` | Green-ish | Focused text-emote broadcast tests | Pending `/wave` `/dance` smoke | P2 | Session + MapRuntime |
| Targeting | Selection / `CMSG_SET_SELECTION` | Player selection state | Usually local state; server uses target for later actions | `session_loop.rs`, combat/spell paths | Yellow | Selection feeds attack/spell tests | Pending | P2 | Session cache + MapRuntime validation |
| Combat | Attack start/stop / `CMSG_ATTACKSWING`, `CMSG_ATTACKSTOP` | `Unit::Attack`, `MeleeAttackStop` | `SMSG_ATTACKSTART`, `SMSG_ATTACKSTOP` to nearby observers | `combat/entrypoints.rs`, `combat/stop.rs`, `maps/map/creature_combat.rs` | Yellow | Shared wolf and unit tests | Pending broader smoke | P1 | MapRuntime |
| Combat | Melee damage/miss/death | `Unit::DealDamage`, `AttackerStateUpdate` | `SMSG_ATTACKERSTATEUPDATE`, health updates, death/corpse flags | `maps/map/creature_damage.rs`, `spells/effects.rs` | Yellow | Shared wolf damage/death proof | Pending broader smoke | P1 | MapRuntime |
| Spells | Cast start/go/logs / `CMSG_CAST_SPELL`, `CMSG_USE_ITEM` | `Spell::prepare`, `Spell::cast`, spell log builders | Nearby observers see `SMSG_SPELL_START`, `SMSG_SPELL_GO`, damage/heal/miss logs, aura updates | `spells.rs`, `spells/effects.rs`, `spells/auras.rs` | Yellow | Starter spell packet tests | Pending | P1 | Spell system + MapRuntime |
| Loot | Corpse open/money/item/release / `CMSG_LOOT*` | `LootHandler.cpp`, `LootMgr.cpp`, `Player::SendLoot` | Authorized opener gets response; other openers get money/item clear; observers see corpse flag updates; duplicate claims denied | `loot.rs`, `maps/map/creature_loot.rs` | Yellow | Unit and shared wolf duplicate-denial proof | Pending real-client two-looter smoke | P1 | MapRuntime |
| Loot | Group loot / `CMSG_LOOT_ROLL`, `CMSG_LOOT_METHOD`, `CMSG_LOOT_MASTER_GIVE` | `Group.cpp`, `LootMgr.cpp` | Group receives roll/start/won/all-passed/master-list packets; item awarded once | `social/party.rs`, `loot.rs` | Yellow | Unit tests cover roll/master basics | Real-client proof pending | P1 | Party + MapRuntime |
| Party | Invite/accept/leave/kick/leader/list/stats | `GroupHandler.cpp`, `Group.cpp` | Targeted invite/result/list/member stat packets | `social/party.rs` | Yellow | Unit coverage; limited flow proof | Pending two-account smoke | P2 | PartyManager |
| Channels | Join/list/channel chat / `CMSG_JOIN_CHANNEL`, channel chat | `Channel.cpp`, `ChannelHandler.cpp` | `SMSG_CHANNEL_NOTIFY`, channel `SMSG_MESSAGECHAT` to members | `chat.rs::handle_join_channel` | Red-Yellow | Join notify only | Pending | P3 | Social |
| Trade | Trade request/items/money/accept/cancel | `TradeHandler.cpp`, `TradeData` | Both traders receive status and trade updates; atomic exchange; anti-dupe | Not implemented beyond cancel noise handling | Red | None | None | P3 | Future social |
| Duel | Duel request/flag/start/bounds/winner | `DuelHandler.cpp`, `Player::DuelComplete` | Duel request/start/out-of-bounds/winner packets to both players and nearby flag object visibility | Not implemented | Red | None | None | P3 | Future PvP/social |
| Inspect/who | Inspect, who list | `QueryHandler.cpp`, `MiscHandler.cpp` | Targeted responses, mostly not nearby broadcast | Not implemented or partial unknown-op handling | Red | None | None | P4 | Future social |
| Relog torture | Logout/relog during combat/corpse/loot/group | CMaNGOS player cleanup and map object visibility cleanup | No stale combat ticks, duplicate loot, missing destroy/create, or wrong corpse flags | `logout.rs`, `maps/map/*`, `loot.rs`, `social/party.rs` | Yellow-Red | Some ownership/relog unit coverage | Pending | P1 | MapRuntime + Session |

## Current Findings

- Text emotes were the clearest P2 visible gap: Rust handled
  `CMSG_TEXT_EMOTE` as a sender-only path, while CMaNGOS broadcasts localized
  `SMSG_TEXT_EMOTE` to nearby listeners and triggers visible animation.
- Loot/corpse state is map-owned and already has meaningful unit coverage, but
  the cross-client proof is still too narrow. The next audit-driven slice should
  prove observer money/item removal packets, late visibility, group-loot, and
  relog/disconnect cases.
- Trade, duel, inspect, and full channel behavior are broad social backlog
  unless they surface as protocol spam, client desync, or a demo-blocking
  multiplayer issue.

## Manual Real-Client Checklist

- Two players in Northshire: login together, run, jump, turn, leave/re-enter
  visibility range, logout/relog.
- Chat and emotes: `/say`, `/yell`, typed `/emote`, `/wave`, `/point`,
  `/dance`, `/sleep`, targeted emotes against a player and creature.
- Combat and loot: both players observe attack start/stop, damage, spell logs,
  death, corpse sparkle, loot open, money/item claim, duplicate denial, release,
  and late re-entry after corpse creation.
- Party: invite/accept, party chat, member stats, loot method changes,
  free-for-all, round-robin/current-looter, group loot, master loot.
