use super::*;

pub(in crate::world) async fn dispatch_npc_packet(
    ctx: &mut WorldPacketDispatchContext<'_>,
    packet: &packets::ParsedWorldClientPacket,
) -> anyhow::Result<()> {
    match packet {
        packets::ParsedWorldClientPacket::CreatureQuery(_) => {
            handle_creature_query(
                &mut *ctx.stream,
                ctx.world_db_pool,
                packet.creature_query()?,
                &mut *ctx.header_crypto,
            )
            .await
        }
        packets::ParsedWorldClientPacket::GameObjectQuery(_) => {
            handle_gameobject_query(
                &mut *ctx.stream,
                ctx.world_db_pool,
                packet.gameobject_query()?,
                &mut *ctx.header_crypto,
            )
            .await
        }
        packets::ParsedWorldClientPacket::GossipHello(_) => {
            handle_gossip_hello(
                &mut *ctx.stream,
                &ctx.runtime_state.object_mgr,
                ctx.world_db_pool,
                &ctx.runtime_state.maps,
                packet.gossip_hello()?,
                &mut *ctx.session,
                &mut *ctx.header_crypto,
            )
            .await
        }
        packets::ParsedWorldClientPacket::GossipSelectOption(_) => {
            handle_gossip_select_option(
                &mut *ctx.stream,
                GossipSelectDeps {
                    character_db_pool: ctx.character_db_pool,
                    world_db_pool: ctx.world_db_pool,
                    world_data_files: ctx.runtime_state.world_data_files.as_ref(),
                    auction_config: ctx.runtime_state.auction_config,
                    vendor_stock: &ctx.runtime_state.vendor_stock,
                    object_mgr: ctx.runtime_state.object_mgr.as_ref(),
                    maps: &ctx.runtime_state.maps,
                    sessions: &ctx.runtime_state.sessions,
                    account_id: ctx.account_id,
                },
                packet.gossip_select_option()?,
                &mut *ctx.session,
                &mut *ctx.header_crypto,
            )
            .await
        }
        packets::ParsedWorldClientPacket::GameObjectUse(_) => {
            handle_gameobject_use(
                &mut *ctx.stream,
                GameObjectUseDeps {
                    character_db_pool: ctx.character_db_pool,
                    world_db_pool: ctx.world_db_pool,
                    object_mgr: ctx.runtime_state.object_mgr.as_ref(),
                    maps: &ctx.runtime_state.maps,
                    sessions: &ctx.runtime_state.sessions,
                },
                packet.gameobject_use()?,
                &mut *ctx.session,
                &mut *ctx.header_crypto,
            )
            .await
        }
        packets::ParsedWorldClientPacket::NpcTextQuery(_) => {
            handle_npc_text_query(
                &mut *ctx.stream,
                ctx.world_db_pool,
                packet.npc_text_query()?,
                &mut *ctx.header_crypto,
            )
            .await
        }
        packets::ParsedWorldClientPacket::ListInventory(_) => {
            handle_list_inventory(
                &mut *ctx.stream,
                ctx.world_db_pool,
                &ctx.runtime_state.vendor_stock,
                packet.list_inventory()?,
                &mut *ctx.header_crypto,
            )
            .await
        }
        packets::ParsedWorldClientPacket::SellItem(_) => {
            handle_sell_item(
                &mut *ctx.stream,
                QuestMutationDeps {
                    character_db_pool: ctx.character_db_pool,
                    object_mgr: ctx.runtime_state.object_mgr.as_ref(),
                    world_db_pool: ctx.world_db_pool,
                    shared_world: SharedWorldDeps {
                        object_mgr: ctx.runtime_state.object_mgr.as_ref(),
                        maps: &ctx.runtime_state.maps,
                        sessions: &ctx.runtime_state.sessions,
                    },
                },
                packet.sell_item()?,
                &mut *ctx.session,
                &mut *ctx.header_crypto,
            )
            .await
        }
        packets::ParsedWorldClientPacket::BuybackItem(_) => {
            handle_buyback_item(
                &mut *ctx.stream,
                QuestMutationDeps {
                    character_db_pool: ctx.character_db_pool,
                    object_mgr: ctx.runtime_state.object_mgr.as_ref(),
                    world_db_pool: ctx.world_db_pool,
                    shared_world: SharedWorldDeps {
                        object_mgr: ctx.runtime_state.object_mgr.as_ref(),
                        maps: &ctx.runtime_state.maps,
                        sessions: &ctx.runtime_state.sessions,
                    },
                },
                packet.buyback_item()?,
                &mut *ctx.session,
                &mut *ctx.header_crypto,
            )
            .await
        }
        packets::ParsedWorldClientPacket::BuyItem(_) => {
            handle_buy_item(
                &mut *ctx.stream,
                ctx.character_db_pool,
                ctx.world_db_pool,
                &ctx.runtime_state.vendor_stock,
                packet.buy_item()?,
                &mut *ctx.session,
                &mut *ctx.header_crypto,
            )
            .await
        }
        packets::ParsedWorldClientPacket::BuyItemInSlot(_) => {
            handle_buy_item_in_slot(
                &mut *ctx.stream,
                ctx.character_db_pool,
                ctx.world_db_pool,
                &ctx.runtime_state.vendor_stock,
                packet.buy_item_in_slot()?,
                &mut *ctx.session,
                &mut *ctx.header_crypto,
            )
            .await
        }
        packets::ParsedWorldClientPacket::TrainerList(_) => {
            handle_trainer_list(
                &mut *ctx.stream,
                ctx.character_db_pool,
                ctx.world_db_pool,
                ctx.runtime_state.maps.as_ref(),
                packet.trainer_list()?,
                &mut *ctx.session,
                &mut *ctx.header_crypto,
            )
            .await
        }
        packets::ParsedWorldClientPacket::TrainerBuySpell(_) => {
            handle_trainer_buy_spell(
                &mut *ctx.stream,
                ctx.character_db_pool,
                ctx.world_db_pool,
                ctx.runtime_state.maps.as_ref(),
                packet.trainer_buy_spell()?,
                &mut *ctx.session,
                &mut *ctx.header_crypto,
            )
            .await
        }
        packets::ParsedWorldClientPacket::AuctionHello(_) => {
            send_auction_hello(
                &mut *ctx.stream,
                &ctx.runtime_state.maps,
                ctx.runtime_state.world_data_files.as_ref(),
                ctx.runtime_state.auction_config,
                ObjectGuid::from_raw(packet.auction_hello()?.auctioneer_raw_guid),
                &*ctx.session,
                &mut *ctx.header_crypto,
            )
            .await
        }
        packets::ParsedWorldClientPacket::AuctionSellItem(_) => {
            handle_auction_sell_item(
                &mut *ctx.stream,
                AuctionMutationDeps {
                    quest: QuestMutationDeps {
                        character_db_pool: ctx.character_db_pool,
                        object_mgr: ctx.runtime_state.object_mgr.as_ref(),
                        world_db_pool: ctx.world_db_pool,
                        shared_world: SharedWorldDeps {
                            object_mgr: ctx.runtime_state.object_mgr.as_ref(),
                            maps: &ctx.runtime_state.maps,
                            sessions: &ctx.runtime_state.sessions,
                        },
                    },
                    world_data_files: ctx.runtime_state.world_data_files.as_ref(),
                    auction_config: ctx.runtime_state.auction_config,
                    maps: &ctx.runtime_state.maps,
                },
                packet.auction_sell_item()?,
                &mut *ctx.session,
                &mut *ctx.header_crypto,
            )
            .await
        }
        packets::ParsedWorldClientPacket::AuctionRemoveItem(_) => {
            handle_auction_remove_item(
                &mut *ctx.stream,
                AuctionMutationDeps {
                    quest: QuestMutationDeps {
                        character_db_pool: ctx.character_db_pool,
                        object_mgr: ctx.runtime_state.object_mgr.as_ref(),
                        world_db_pool: ctx.world_db_pool,
                        shared_world: SharedWorldDeps {
                            object_mgr: ctx.runtime_state.object_mgr.as_ref(),
                            maps: &ctx.runtime_state.maps,
                            sessions: &ctx.runtime_state.sessions,
                        },
                    },
                    world_data_files: ctx.runtime_state.world_data_files.as_ref(),
                    auction_config: ctx.runtime_state.auction_config,
                    maps: &ctx.runtime_state.maps,
                },
                packet.auction_remove_item()?,
                &mut *ctx.session,
                &mut *ctx.header_crypto,
            )
            .await
        }
        packets::ParsedWorldClientPacket::AuctionPlaceBid(_) => {
            handle_auction_place_bid(
                &mut *ctx.stream,
                AuctionMutationDeps {
                    quest: QuestMutationDeps {
                        character_db_pool: ctx.character_db_pool,
                        object_mgr: ctx.runtime_state.object_mgr.as_ref(),
                        world_db_pool: ctx.world_db_pool,
                        shared_world: SharedWorldDeps {
                            object_mgr: ctx.runtime_state.object_mgr.as_ref(),
                            maps: &ctx.runtime_state.maps,
                            sessions: &ctx.runtime_state.sessions,
                        },
                    },
                    world_data_files: ctx.runtime_state.world_data_files.as_ref(),
                    auction_config: ctx.runtime_state.auction_config,
                    maps: &ctx.runtime_state.maps,
                },
                packet.auction_place_bid()?,
                &mut *ctx.session,
                &mut *ctx.header_crypto,
            )
            .await
        }
        packets::ParsedWorldClientPacket::AuctionListItems(_) => {
            handle_auction_list_items(
                &mut *ctx.stream,
                AuctionBrowseDeps {
                    character_db_pool: ctx.character_db_pool,
                    world_db_pool: ctx.world_db_pool,
                    world_data_files: ctx.runtime_state.world_data_files.as_ref(),
                    auction_config: ctx.runtime_state.auction_config,
                    maps: &ctx.runtime_state.maps,
                },
                packet.auction_list_items()?,
                &*ctx.session,
                &mut *ctx.header_crypto,
            )
            .await
        }
        packets::ParsedWorldClientPacket::AuctionListOwnerItems(_) => {
            handle_auction_list_owner_items(
                &mut *ctx.stream,
                AuctionBrowseDeps {
                    character_db_pool: ctx.character_db_pool,
                    world_db_pool: ctx.world_db_pool,
                    world_data_files: ctx.runtime_state.world_data_files.as_ref(),
                    auction_config: ctx.runtime_state.auction_config,
                    maps: &ctx.runtime_state.maps,
                },
                packet.auction_list_owner_items()?,
                &*ctx.session,
                &mut *ctx.header_crypto,
            )
            .await
        }
        packets::ParsedWorldClientPacket::AuctionListBidderItems(_) => {
            handle_auction_list_bidder_items(
                &mut *ctx.stream,
                AuctionBrowseDeps {
                    character_db_pool: ctx.character_db_pool,
                    world_db_pool: ctx.world_db_pool,
                    world_data_files: ctx.runtime_state.world_data_files.as_ref(),
                    auction_config: ctx.runtime_state.auction_config,
                    maps: &ctx.runtime_state.maps,
                },
                packet.auction_list_bidder_items()?,
                &*ctx.session,
                &mut *ctx.header_crypto,
            )
            .await
        }
        other => anyhow::bail!("npc router received opcode 0x{:04X}", other.opcode()),
    }
}
