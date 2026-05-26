use super::*;
use wow_proto::world::WorldOpcode;
use wow_proto::{
    ServerWorldPacket, SmsgBuyFailedResponse, SmsgBuyItemResponse,
    SmsgInventoryChangeFailureResponse, SmsgListInventoryResponse, SmsgSellItemResponse,
    VendorListItemResponse,
};

pub(in crate::world) async fn dispatch_inventory_packet(
    ctx: &mut WorldPacketDispatchContext<'_>,
    packet: &packets::ParsedWorldClientPacket,
) -> anyhow::Result<()> {
    match packet {
        packets::ParsedWorldClientPacket::InventoryMove(_) => {
            handle_inventory_swap(
                &mut *ctx.stream,
                InventoryDeps {
                    character_db_pool: ctx.character_db_pool,
                    world_db_pool: ctx.world_db_pool,
                    shared_world: SharedWorldDeps {
                        object_mgr: ctx.runtime_state.object_mgr.as_ref(),
                        maps: &ctx.runtime_state.maps,
                        sessions: &ctx.runtime_state.sessions,
                    },
                },
                packet.inventory_move()?,
                &mut *ctx.session,
                &mut *ctx.header_crypto,
            )
            .await
        }
        packets::ParsedWorldClientPacket::DestroyItem(_) => {
            handle_destroy_item(
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
                packet.destroy_item()?,
                &mut *ctx.session,
                &mut *ctx.header_crypto,
            )
            .await
        }
        packets::ParsedWorldClientPacket::SplitItem(_) => {
            handle_split_item(
                &mut *ctx.stream,
                ctx.character_db_pool,
                packet.split_item()?,
                &mut *ctx.session,
                &mut *ctx.header_crypto,
            )
            .await
        }
        packets::ParsedWorldClientPacket::ReadItem(_) => {
            handle_read_item(
                &mut *ctx.stream,
                ctx.world_db_pool,
                packet.read_item()?,
                &*ctx.session,
                &mut *ctx.header_crypto,
            )
            .await
        }
        packets::ParsedWorldClientPacket::SetAmmo(_) => {
            handle_set_ammo(
                &mut *ctx.stream,
                InventoryDeps {
                    character_db_pool: ctx.character_db_pool,
                    world_db_pool: ctx.world_db_pool,
                    shared_world: SharedWorldDeps {
                        object_mgr: ctx.runtime_state.object_mgr.as_ref(),
                        maps: &ctx.runtime_state.maps,
                        sessions: &ctx.runtime_state.sessions,
                    },
                },
                packet.set_ammo()?,
                &mut *ctx.session,
                &mut *ctx.header_crypto,
            )
            .await
        }
        packets::ParsedWorldClientPacket::AutoBankItem(_) => {
            handle_auto_bank_item(
                &mut *ctx.stream,
                InventoryDeps {
                    character_db_pool: ctx.character_db_pool,
                    world_db_pool: ctx.world_db_pool,
                    shared_world: SharedWorldDeps {
                        object_mgr: ctx.runtime_state.object_mgr.as_ref(),
                        maps: &ctx.runtime_state.maps,
                        sessions: &ctx.runtime_state.sessions,
                    },
                },
                packet.auto_bank_item()?,
                &mut *ctx.session,
                &mut *ctx.header_crypto,
            )
            .await
        }
        packets::ParsedWorldClientPacket::AutoStoreBankItem(_) => {
            handle_auto_store_bank_item(
                &mut *ctx.stream,
                InventoryDeps {
                    character_db_pool: ctx.character_db_pool,
                    world_db_pool: ctx.world_db_pool,
                    shared_world: SharedWorldDeps {
                        object_mgr: ctx.runtime_state.object_mgr.as_ref(),
                        maps: &ctx.runtime_state.maps,
                        sessions: &ctx.runtime_state.sessions,
                    },
                },
                packet.auto_store_bank_item()?,
                &mut *ctx.session,
                &mut *ctx.header_crypto,
            )
            .await
        }
        other => anyhow::bail!("inventory router received opcode 0x{:04X}", other.opcode()),
    }
}

pub(in crate::world) async fn handle_read_item(
    stream: &mut WorldPacketSink,
    world_db_pool: &MySqlPool,
    request: wow_proto::ReadItemRequest,
    session: &WorldSessionState,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    let bag = normalize_client_bag(request.bag);
    let Some(item) = session
        .inventory
        .items
        .iter()
        .find(|item| item.bag == bag as u32 && item.slot == request.slot)
    else {
        warn!(
            bag = request.bag,
            slot = request.slot,
            "Ignoring read item request for empty inventory slot"
        );
        return Ok(());
    };
    let item_guid = ObjectGuid::new(HighGuid::Item, 0, item.item);
    let readable = wow_db::get_item_template_query(world_db_pool, item.item_template)
        .await?
        .is_some_and(|template| template.page_text != 0);
    if readable {
        let response = wow_proto::SmsgReadItemOkResponse { item: item_guid }.body();
        send_packet(
            stream,
            WorldOpcode::SmsgReadItemOk as u16,
            &response,
            Some(header_crypto),
        )
        .await
    } else {
        let response = wow_proto::SmsgReadItemFailedResponse { item: item_guid }.body();
        send_packet(
            stream,
            WorldOpcode::SmsgReadItemFailed as u16,
            &response,
            Some(header_crypto),
        )
        .await
    }
}

pub(in crate::world) const INVTYPE_BAG: u32 = 18;
pub(in crate::world) const INVTYPE_2HWEAPON: u32 = 17;
pub(in crate::world) const INVTYPE_AMMO: u32 = 24;
pub(in crate::world) const INVTYPE_THROWN: u32 = 25;
pub(in crate::world) const ITEM_CLASS_CONTAINER: u32 = 1;
pub(in crate::world) const ITEM_CLASS_PROJECTILE: u32 = 6;
pub(in crate::world) const ITEM_CLASS_QUIVER: u32 = 11;
pub(in crate::world) const ITEM_SUBCLASS_CONTAINER: u32 = 0;
pub(in crate::world) const ITEM_SUBCLASS_SOUL_CONTAINER: u32 = 1;
pub(in crate::world) const ITEM_SUBCLASS_HERB_CONTAINER: u32 = 2;
pub(in crate::world) const ITEM_SUBCLASS_ENCHANTING_CONTAINER: u32 = 3;
pub(in crate::world) const ITEM_SUBCLASS_ENGINEERING_CONTAINER: u32 = 4;
pub(in crate::world) const ITEM_SUBCLASS_QUIVER: u32 = 2;
pub(in crate::world) const ITEM_SUBCLASS_AMMO_POUCH: u32 = 3;
pub(in crate::world) const ITEM_SUBCLASS_ARROW: u32 = 2;
pub(in crate::world) const ITEM_SUBCLASS_BULLET: u32 = 3;
pub(in crate::world) const ITEM_SUBCLASS_WEAPON_BOW: u32 = 2;
pub(in crate::world) const ITEM_SUBCLASS_WEAPON_GUN: u32 = 3;
pub(in crate::world) const ITEM_SUBCLASS_WEAPON_CROSSBOW: u32 = 18;
pub(in crate::world) const INVTYPE_HOLDABLE: u32 = 23;
pub(in crate::world) const INVTYPE_RELIC: u32 = 28;
pub(in crate::world) const ITEM_DYNFLAG_BINDED: u32 = 0x0000_0001;
pub(in crate::world) const BIND_WHEN_PICKED_UP: u32 = 1;
pub(in crate::world) const BIND_WHEN_EQUIPPED: u32 = 2;
pub(in crate::world) const BIND_QUEST_ITEM: u32 = 4;
pub(in crate::world) const BIND_QUEST_ITEM1: u32 = 5;
pub(in crate::world) const BAG_FAMILY_ARROWS: i32 = 1;
pub(in crate::world) const BAG_FAMILY_BULLETS: i32 = 2;
pub(in crate::world) const BAG_FAMILY_SOUL_SHARDS: i32 = 3;
pub(in crate::world) const BAG_FAMILY_HERBS: i32 = 6;
pub(in crate::world) const BAG_FAMILY_ENCHANTING_SUPP: i32 = 7;
pub(in crate::world) const BAG_FAMILY_ENGINEERING_SUPP: i32 = 8;

pub(in crate::world) fn item_binding_flags_on_pickup(template: &ItemTemplateQuery) -> u32 {
    if item_binds_when_picked_up(template) {
        ITEM_DYNFLAG_BINDED
    } else {
        0
    }
}

pub(in crate::world) fn item_instance_is_soulbound(flags: u32) -> bool {
    flags & ITEM_DYNFLAG_BINDED != 0
}

pub(in crate::world) fn item_binds_when_picked_up(template: &ItemTemplateQuery) -> bool {
    matches!(
        template.bonding,
        BIND_WHEN_PICKED_UP | BIND_QUEST_ITEM | BIND_QUEST_ITEM1
    )
}

pub(in crate::world) fn merged_destination_stack_needs_pickup_bind(
    template: &ItemTemplateQuery,
    destination_flags: u32,
) -> bool {
    item_binds_when_picked_up(template) && !item_instance_is_soulbound(destination_flags)
}

pub(in crate::world) fn item_binds_when_equipped(template: &ItemTemplateQuery) -> bool {
    matches!(
        template.bonding,
        BIND_WHEN_EQUIPPED | BIND_WHEN_PICKED_UP | BIND_QUEST_ITEM | BIND_QUEST_ITEM1
    )
}

pub(in crate::world) async fn handle_inventory_swap(
    stream: &mut WorldPacketSink,
    deps: InventoryDeps<'_>,
    request: wow_proto::InventoryMoveClientRequest,
    session: &mut WorldSessionState,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    let opcode = inventory_move_client_request_opcode(request);
    let Some(character) = &session.character.active_character else {
        warn!(
            opcode = inventory_opcode_name(opcode),
            "Ignoring inventory move before character login"
        );
        return Ok(());
    };
    let character_guid = character.guid;
    let Some(move_request) = (match request {
        wow_proto::InventoryMoveClientRequest::AutoEquip { src_bag, src_slot } => {
            let auto_equip = InventoryMoveRequest::read_auto_equip(
                src_bag,
                src_slot,
                deps.world_db_pool,
                session,
            )
            .await?;
            if auto_equip.is_none() {
                send_auto_equip_failure_if_known(
                    stream,
                    src_bag,
                    src_slot,
                    deps.world_db_pool,
                    session,
                    header_crypto,
                )
                .await?;
                return Ok(());
            }
            auto_equip
        }
        wow_proto::InventoryMoveClientRequest::AutoStoreBag {
            src_bag,
            src_slot,
            dst_bag,
        } => {
            let auto_store = InventoryMoveRequest::read_auto_store_bag(
                src_bag,
                src_slot,
                dst_bag,
                deps.world_db_pool,
                session,
            )
            .await?;
            if auto_store.is_none() {
                send_auto_store_bag_failure_if_known(
                    stream,
                    src_bag,
                    src_slot,
                    dst_bag,
                    deps.world_db_pool,
                    session,
                    header_crypto,
                )
                .await?;
                return Ok(());
            }
            auto_store
        }
        _ => Some(InventoryMoveRequest::from_client_request(request)?),
    }) else {
        info!(
            opcode = inventory_opcode_name(opcode),
            "Ignoring unsupported inventory auto move source or destination"
        );
        return Ok(());
    };

    if !move_request.is_supported_inventory_move() {
        info!(
            opcode = inventory_opcode_name(opcode),
            src_bag = move_request.src_bag,
            src_slot = move_request.src_slot,
            dst_bag = move_request.dst_bag,
            dst_slot = move_request.dst_slot,
            "Ignoring unsupported inventory move outside bag-0 or equipped bag storage"
        );
        return Ok(());
    }

    let bag_model = InventoryBagModel::load(
        deps.world_db_pool,
        &session.inventory.items,
        bank_bag_slot_count(session),
    )
    .await?;
    if !move_request.uses_existing_storage(&bag_model) {
        info!(
            opcode = inventory_opcode_name(opcode),
            src_bag = move_request.src_bag,
            src_slot = move_request.src_slot,
            dst_bag = move_request.dst_bag,
            dst_slot = move_request.dst_slot,
            "Rejected inventory move outside existing inventory or bank storage"
        );
        return send_inventory_change_failure(
            stream,
            if move_request.references_unpurchased_bank_bag_slot(bag_model.bank_bag_slot_count()) {
                EQUIP_ERR_MUST_PURCHASE_THAT_BAG_SLOT
            } else if move_request.references_bank_storage() {
                EQUIP_ERR_TOO_FAR_AWAY_FROM_BANK
            } else {
                EQUIP_ERR_ITEM_DOESNT_GO_TO_SLOT
            },
            None,
            None,
            header_crypto,
        )
        .await;
    }

    if move_request.src_bag == move_request.dst_bag
        && move_request.src_slot == move_request.dst_slot
    {
        return Ok(());
    }

    let Some(src_item) =
        session.inventory.items.iter().find(|item| {
            item.bag == move_request.src_bag as u32 && item.slot == move_request.src_slot
        })
    else {
        warn!(
            opcode = inventory_opcode_name(opcode),
            guid = character_guid,
            src_bag = move_request.src_bag,
            src_slot = move_request.src_slot,
            dst_bag = move_request.dst_bag,
            dst_slot = move_request.dst_slot,
            "Rejected inventory move without source item"
        );
        return Ok(());
    };

    let Some(src_template) =
        wow_db::get_item_template_query(deps.world_db_pool, src_item.item_template).await?
    else {
        warn!(
            opcode = inventory_opcode_name(opcode),
            guid = character_guid,
            item_template = src_item.item_template,
            "Rejected inventory move for missing source item template"
        );
        return Ok(());
    };

    let move_request = if let Some(resolved) = resolve_bag_icon_move_destination(
        &session.inventory.items,
        src_item,
        &src_template,
        &bag_model,
        &move_request,
    ) {
        resolved
    } else {
        move_request
    };

    let dst_item =
        session.inventory.items.iter().find(|item| {
            item.bag == move_request.dst_bag as u32 && item.slot == move_request.dst_slot
        });
    let dst_template = if let Some(dst_item) = dst_item {
        let Some(template) =
            wow_db::get_item_template_query(deps.world_db_pool, dst_item.item_template).await?
        else {
            warn!(
                opcode = inventory_opcode_name(opcode),
                guid = character_guid,
                item_template = dst_item.item_template,
                "Rejected inventory move for missing destination item template"
            );
            return Ok(());
        };
        Some(template)
    } else {
        None
    };

    if move_request.moves_equipped_bag_into_itself() {
        info!(
            opcode = inventory_opcode_name(opcode),
            guid = character_guid,
            src_bag = move_request.src_bag,
            src_slot = move_request.src_slot,
            dst_bag = move_request.dst_bag,
            dst_slot = move_request.dst_slot,
            "Rejected moving equipped bag into itself"
        );
        return send_inventory_change_failure(
            stream,
            EQUIP_ERR_NONEMPTY_BAG_OVER_OTHER_BAG,
            Some(ObjectGuid::new(HighGuid::Item, 0, src_item.item)),
            dst_item.map(|item| ObjectGuid::new(HighGuid::Item, 0, item.item)),
            header_crypto,
        )
        .await;
    }

    if move_request.src_bag == INVENTORY_SLOT_BAG_0 {
        if let Some(error) = item_can_leave_bag0_equipment_or_bag_slot(
            move_request.src_slot,
            &src_template,
            session.combat.player_in_combat,
        ) {
            info!(
                opcode = inventory_opcode_name(opcode),
                guid = character_guid,
                item_template = src_item.item_template,
                src_slot = move_request.src_slot,
                "Rejected inventory move because equipped item cannot be removed now"
            );
            return send_inventory_change_failure(
                stream,
                error,
                Some(ObjectGuid::new(HighGuid::Item, 0, src_item.item)),
                None,
                header_crypto,
            )
            .await;
        }
    }

    if move_request.dst_bag == INVENTORY_SLOT_BAG_0 {
        if let Some(error) = dst_template.as_ref().and_then(|template| {
            item_can_leave_bag0_equipment_or_bag_slot(
                move_request.dst_slot,
                template,
                session.combat.player_in_combat,
            )
        }) {
            info!(
                opcode = inventory_opcode_name(opcode),
                guid = character_guid,
                dst_slot = move_request.dst_slot,
                "Rejected inventory move because destination equipped item cannot be removed now"
            );
            return send_inventory_change_failure(
                stream,
                error,
                dst_item.map(|item| ObjectGuid::new(HighGuid::Item, 0, item.item)),
                Some(ObjectGuid::new(HighGuid::Item, 0, src_item.item)),
                header_crypto,
            )
            .await;
        }
    }

    let moves_src_into_bag0_equip_or_bag = move_request.dst_bag == INVENTORY_SLOT_BAG_0
        && (move_request.dst_slot < EQUIPMENT_SLOT_END || is_bag_slot(move_request.dst_slot));
    let moves_dst_into_bag0_equip_or_bag = dst_item.is_some()
        && move_request.src_bag == INVENTORY_SLOT_BAG_0
        && (move_request.src_slot < EQUIPMENT_SLOT_END || is_bag_slot(move_request.src_slot));
    if moves_src_into_bag0_equip_or_bag || moves_dst_into_bag0_equip_or_bag {
        let skills = wow_db::get_character_skills(deps.character_db_pool, character_guid).await?;
        if let Some((equip_result, required_level)) = item_can_enter_bag0_equipment_or_bag_slot(
            move_request.dst_slot,
            &src_template,
            CharacterEquipValidationContext {
                level: character.level,
                race: character.race,
                class: character.class,
                equipped_bags: bag_model.equipped_bags(),
                ignore_bag_slot: (move_request.src_bag == INVENTORY_SLOT_BAG_0)
                    .then_some(move_request.src_slot)
                    .filter(|slot| is_bag_slot(*slot)),
                skills: &skills,
                active_spells: &session.character.active_spells,
                reputations: &session.character.character_reputations,
                in_combat: session.combat.player_in_combat,
            },
        )
        .filter(|_| moves_src_into_bag0_equip_or_bag)
        {
            info!(
                opcode = inventory_opcode_name(opcode),
                guid = character_guid,
                item_template = src_item.item_template,
                inventory_type = src_template.inventory_type,
                dst_slot = move_request.dst_slot,
                "Rejected inventory move for incompatible equipment/bag destination"
            );
            return send_inventory_change_failure_with_required_level(
                stream,
                equip_result,
                Some(ObjectGuid::new(HighGuid::Item, 0, src_item.item)),
                dst_item.map(|item| ObjectGuid::new(HighGuid::Item, 0, item.item)),
                required_level,
                header_crypto,
            )
            .await;
        }
        if let Some((equip_result, required_level)) = dst_item
            .zip(dst_template.as_ref())
            .filter(|_| moves_dst_into_bag0_equip_or_bag)
            .and_then(|(_, template)| {
                item_can_enter_bag0_equipment_or_bag_slot(
                    move_request.src_slot,
                    template,
                    CharacterEquipValidationContext {
                        level: character.level,
                        race: character.race,
                        class: character.class,
                        equipped_bags: bag_model.equipped_bags(),
                        ignore_bag_slot: (move_request.dst_bag == INVENTORY_SLOT_BAG_0)
                            .then_some(move_request.dst_slot)
                            .filter(|slot| is_bag_slot(*slot)),
                        skills: &skills,
                        active_spells: &session.character.active_spells,
                        reputations: &session.character.character_reputations,
                        in_combat: session.combat.player_in_combat,
                    },
                )
            })
        {
            info!(
                opcode = inventory_opcode_name(opcode),
                guid = character_guid,
                dst_slot = move_request.dst_slot,
                src_slot = move_request.src_slot,
                "Rejected inventory move because displaced item cannot occupy source slot"
            );
            return send_inventory_change_failure_with_required_level(
                stream,
                equip_result,
                dst_item.map(|item| ObjectGuid::new(HighGuid::Item, 0, item.item)),
                Some(ObjectGuid::new(HighGuid::Item, 0, src_item.item)),
                required_level,
                header_crypto,
            )
            .await;
        }
    }

    if let Some(error) = two_handed_equipment_conflict_after_move(
        deps.world_db_pool,
        &session.inventory.items,
        &move_request,
        src_template.inventory_type,
        dst_template
            .as_ref()
            .map(|template| template.inventory_type),
    )
    .await?
    {
        info!(
            opcode = inventory_opcode_name(opcode),
            guid = character_guid,
            src_bag = move_request.src_bag,
            src_slot = move_request.src_slot,
            dst_bag = move_request.dst_bag,
            dst_slot = move_request.dst_slot,
            "Rejected inventory move because a two-handed weapon conflicts with offhand equipment"
        );
        return send_inventory_change_failure(
            stream,
            error,
            Some(ObjectGuid::new(HighGuid::Item, 0, src_item.item)),
            dst_item.map(|item| ObjectGuid::new(HighGuid::Item, 0, item.item)),
            header_crypto,
        )
        .await;
    }

    if move_request.src_bag == INVENTORY_SLOT_BAG_0
        && is_bag_slot(move_request.src_slot)
        && !is_bag_slot(move_request.dst_slot)
        && session
            .inventory
            .items
            .iter()
            .any(|item| item.bag == move_request.src_slot as u32)
    {
        info!(
            opcode = inventory_opcode_name(opcode),
            guid = character_guid,
            src_slot = move_request.src_slot,
            "Rejected moving non-empty equipped bag into non-bag storage"
        );
        return send_inventory_change_failure(
            stream,
            EQUIP_ERR_CAN_ONLY_DO_WITH_EMPTY_BAGS,
            Some(ObjectGuid::new(HighGuid::Item, 0, src_item.item)),
            dst_item.map(|item| ObjectGuid::new(HighGuid::Item, 0, item.item)),
            header_crypto,
        )
        .await;
    }

    if !bag_model.any_storage_bag_accepts_item(move_request.dst_bag, &src_template) {
        return send_inventory_change_failure(
            stream,
            EQUIP_ERR_ITEM_DOESNT_GO_TO_SLOT,
            Some(ObjectGuid::new(HighGuid::Item, 0, src_item.item)),
            dst_item.map(|item| ObjectGuid::new(HighGuid::Item, 0, item.item)),
            header_crypto,
        )
        .await;
    }
    if let Some((dst_item, dst_template)) = dst_item.zip(dst_template.as_ref()) {
        if !bag_model.any_storage_bag_accepts_item(move_request.src_bag, dst_template) {
            return send_inventory_change_failure(
                stream,
                EQUIP_ERR_ITEM_DOESNT_GO_TO_SLOT,
                Some(ObjectGuid::new(HighGuid::Item, 0, dst_item.item)),
                Some(ObjectGuid::new(HighGuid::Item, 0, src_item.item)),
                header_crypto,
            )
            .await;
        }
    }

    let max_stack = if let Some(dst_item) = dst_item
        .filter(|item| item.item_template == src_item.item_template && item.item != src_item.item)
    {
        let Some(template) =
            wow_db::get_item_template_query(deps.world_db_pool, dst_item.item_template).await?
        else {
            return Ok(());
        };
        Some(template.stackable)
    } else {
        None
    };

    let moved = wow_db::swap_character_inventory_slots_with_stack(
        deps.character_db_pool,
        character_guid,
        move_request.src_bag as u32,
        move_request.src_slot,
        move_request.dst_bag as u32,
        move_request.dst_slot,
        max_stack,
    )
    .await?;
    let Some(moved) = moved else {
        warn!(
            opcode = inventory_opcode_name(opcode),
            guid = character_guid,
            src_bag = move_request.src_bag,
            src_slot = move_request.src_slot,
            dst_bag = move_request.dst_bag,
            dst_slot = move_request.dst_slot,
            "Rejected inventory move without source item"
        );
        return Ok(());
    };
    let moved_is_swap = matches!(moved, wow_db::InventoryMoveResult::Swapped);
    let mut bound_item_guids = Vec::new();
    if moved_is_swap {
        if move_request.dst_bag == INVENTORY_SLOT_BAG_0
            && move_request.dst_slot < EQUIPMENT_SLOT_END
            && item_binds_when_equipped(&src_template)
            && !item_instance_is_soulbound(src_item.flags)
            && wow_db::bind_character_inventory_item(
                deps.character_db_pool,
                character_guid,
                src_item.item,
            )
            .await?
        {
            bound_item_guids.push(src_item.item);
        }
        if move_request.src_bag == INVENTORY_SLOT_BAG_0
            && move_request.src_slot < EQUIPMENT_SLOT_END
        {
            if let Some((dst_item, dst_template)) = dst_item.zip(dst_template.as_ref()) {
                if item_binds_when_equipped(dst_template)
                    && !item_instance_is_soulbound(dst_item.flags)
                    && wow_db::bind_character_inventory_item(
                        deps.character_db_pool,
                        character_guid,
                        dst_item.item,
                    )
                    .await?
                {
                    bound_item_guids.push(dst_item.item);
                }
            }
        }
    }
    if let wow_db::InventoryMoveResult::Merged {
        destination_item, ..
    } = moved
    {
        if session
            .inventory
            .items
            .iter()
            .find(|item| item.item == destination_item)
            .is_some_and(|item| {
                merged_destination_stack_needs_pickup_bind(&src_template, item.flags)
            })
            && wow_db::bind_character_inventory_item(
                deps.character_db_pool,
                character_guid,
                destination_item,
            )
            .await?
        {
            bound_item_guids.push(destination_item);
        }
    }

    session.inventory.items =
        wow_db::get_character_inventory_items(deps.character_db_pool, character_guid).await?;
    if let Some(character) = session.character.active_character.as_ref() {
        deps.shared_world
            .maps
            .update_player_inventory(
                character.position.map_id,
                character_guid,
                session.inventory.items.clone(),
            )
            .await;
    }
    let changed_equipment_slots = bag0_changed_slots(&move_request)
        .into_iter()
        .filter(|slot| *slot < EQUIPMENT_SLOT_END)
        .collect::<Vec<_>>();
    let mut combat_stats_update_body = None;
    let mut world_stats_update_body = None;
    if !changed_equipment_slots.is_empty() {
        if let Some(character) = session.character.active_character.clone() {
            let in_combat = session.combat.player_in_combat;
            let should_interrupt_spell_cast = moved_is_swap
                && combat_equipment_slot_change_interrupts_active_spell_cast(
                    &changed_equipment_slots,
                    in_combat,
                );
            let should_reset_main_hand_swing = moved_is_swap
                && combat_equipment_slot_change_resets_main_hand_swing(
                    &changed_equipment_slots,
                    in_combat,
                );
            let world_stats = wow_db::get_player_world_stats(
                deps.world_db_pool,
                character.race,
                character.class,
                character.level,
            )
            .await?;
            let equipped_templates = load_equipped_item_templates_with_enchantments(
                deps.world_db_pool,
                &session.inventory.items,
                &session
                    .movement
                    .db_creature_navigation
                    .world_data_files
                    .spell_item_enchantments,
            )
            .await?;
            let ammo_template = load_selected_ammo_template(
                deps.world_db_pool,
                &session.inventory.items,
                session.character.player_ammo_id,
            )
            .await?;
            let (base_combat_stats, combat_stats) = inventory_recomputed_combat_stats(
                character.class,
                character.level,
                world_stats,
                &equipped_templates,
                ammo_template.as_ref(),
                &session.character.character_skills,
                &session.character.active_spells,
                &session.auras.active_auras,
            );
            let equipped_world_stats =
                player_world_stats_with_equipment(world_stats, &equipped_templates);
            let effective_world_stats = player_world_stats_with_active_auras(
                equipped_world_stats,
                &session.auras.active_auras,
            );
            world_stats_update_body = Some(build_player_world_stats_update_body(
                character_guid,
                &world_stats,
                &effective_world_stats,
                session.character.player_health,
                session.character.player_mana,
            )?);
            session.character.player_health = session
                .character
                .player_health
                .min(effective_world_stats.max_health().max(1));
            session.character.player_mana = session
                .character
                .player_mana
                .min(effective_world_stats.max_mana());
            combat_stats_update_body = Some(build_player_combat_stats_update_body(
                character_guid,
                &combat_stats,
            )?);
            let packets = deps
                .shared_world
                .maps
                .update_player_world_stats(
                    character.position.map_id,
                    character_guid,
                    equipped_world_stats,
                    effective_world_stats,
                )
                .await?;
            deps.shared_world.sessions.dispatch(packets).await;
            let next_main_hand_swing_at = should_reset_main_hand_swing
                .then(|| player_main_hand_next_swing_at(Instant::now(), &combat_stats));
            let packets = deps
                .shared_world
                .maps
                .update_player_combat_stats(
                    character.position.map_id,
                    character_guid,
                    base_combat_stats,
                )
                .await?;
            deps.shared_world.sessions.dispatch(packets).await;
            if let Some(next_swing_at) = next_main_hand_swing_at {
                mirror_session_player_next_swing_at(session, Some(next_swing_at));
                deps.shared_world
                    .maps
                    .set_player_next_swing_at(
                        character.position.map_id,
                        character_guid,
                        Some(next_swing_at),
                    )
                    .await;
            }
            if should_interrupt_spell_cast {
                cancel_pending_player_spell_cast(
                    stream,
                    deps.shared_world.maps,
                    deps.shared_world.sessions,
                    session,
                    SPELL_FAILED_INTERRUPTED,
                    header_crypto,
                )
                .await?;
            }

            let visible_equipment = visible_equipment_for_inventory(
                session
                    .character
                    .player_visual
                    .as_ref()
                    .and_then(|visual| visual.equipment_cache.as_deref()),
                &session.inventory.items,
            );
            let packets = deps
                .shared_world
                .maps
                .update_player_visible_equipment(
                    character.position.map_id,
                    character_guid,
                    visible_equipment,
                    &changed_equipment_slots,
                )
                .await?;
            deps.shared_world.sessions.dispatch(packets).await;
        }
    }
    match moved {
        wow_db::InventoryMoveResult::Swapped => {
            let mut blocks = build_inventory_move_update_blocks(
                character_guid,
                &session.inventory.items,
                &move_request,
            )?;
            for item_guid in &bound_item_guids {
                if let Some(item) = session
                    .inventory
                    .items
                    .iter()
                    .find(|item| item.item == *item_guid)
                {
                    blocks.push(build_item_flags_update_block(item.item, item.flags)?);
                }
            }
            let body = build_update_object_body(&blocks);
            send_packet(
                stream,
                WorldOpcode::SmsgUpdateObject as u16,
                &body,
                Some(&mut *header_crypto),
            )
            .await?;
            if let Some(body) = combat_stats_update_body {
                send_packet(
                    stream,
                    WorldOpcode::SmsgUpdateObject as u16,
                    &body,
                    Some(header_crypto),
                )
                .await?;
            }
            if let Some(body) = world_stats_update_body {
                send_packet(
                    stream,
                    WorldOpcode::SmsgUpdateObject as u16,
                    &body,
                    Some(header_crypto),
                )
                .await?;
            }
            Ok(())
        }
        wow_db::InventoryMoveResult::Merged {
            source_item,
            source_count,
            destination_item,
            destination_count,
        } => {
            let mut blocks = Vec::new();
            if let Some(source_count) = source_count {
                blocks.push(build_item_stack_count_update_block(
                    source_item,
                    source_count,
                )?);
            } else {
                blocks.extend(build_inventory_position_update_blocks(
                    character_guid,
                    &session.inventory.items,
                    move_request.src_bag,
                    move_request.src_slot,
                )?);
            }
            blocks.push(build_item_stack_count_update_block(
                destination_item,
                destination_count,
            )?);
            for item_guid in &bound_item_guids {
                if let Some(item) = session
                    .inventory
                    .items
                    .iter()
                    .find(|item| item.item == *item_guid)
                {
                    blocks.push(build_item_flags_update_block(item.item, item.flags)?);
                }
            }
            let body = build_update_object_body(&blocks);
            send_packet(
                stream,
                WorldOpcode::SmsgUpdateObject as u16,
                &body,
                Some(&mut *header_crypto),
            )
            .await?;
            if let Some(body) = combat_stats_update_body {
                send_packet(
                    stream,
                    WorldOpcode::SmsgUpdateObject as u16,
                    &body,
                    Some(header_crypto),
                )
                .await?;
            }
            if let Some(body) = world_stats_update_body {
                send_packet(
                    stream,
                    WorldOpcode::SmsgUpdateObject as u16,
                    &body,
                    Some(header_crypto),
                )
                .await?;
            }
            Ok(())
        }
    }
}

pub(in crate::world) async fn send_auto_equip_failure_if_known(
    stream: &mut WorldPacketSink,
    src_bag: u8,
    src_slot: u8,
    world_db_pool: &MySqlPool,
    session: &WorldSessionState,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    let src_bag = normalize_client_bag(src_bag);
    let Some(src_item) = session
        .inventory
        .items
        .iter()
        .find(|item| item.bag == src_bag as u32 && item.slot == src_slot)
    else {
        return Ok(());
    };
    let result = if wow_db::get_item_template_query(world_db_pool, src_item.item_template)
        .await?
        .is_some()
    {
        EQUIP_ERR_ITEM_CANT_BE_EQUIPPED
    } else {
        EQUIP_ERR_ITEM_NOT_FOUND
    };
    send_inventory_change_failure(
        stream,
        result,
        Some(ObjectGuid::new(HighGuid::Item, 0, src_item.item)),
        None,
        header_crypto,
    )
    .await
}

pub(in crate::world) async fn send_auto_store_bag_failure_if_known(
    stream: &mut WorldPacketSink,
    src_bag: u8,
    src_slot: u8,
    dst_bag: u8,
    world_db_pool: &MySqlPool,
    session: &WorldSessionState,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    let src_bag = normalize_client_bag(src_bag);
    let dst_bag = normalize_client_bag(dst_bag);
    let Some(src_item) = session
        .inventory
        .items
        .iter()
        .find(|item| item.bag == src_bag as u32 && item.slot == src_slot)
    else {
        return Ok(());
    };
    let result = if wow_db::get_item_template_query(world_db_pool, src_item.item_template)
        .await?
        .is_some()
    {
        if is_bank_bag_slot(dst_bag) {
            EQUIP_ERR_BANK_FULL
        } else {
            EQUIP_ERR_INVENTORY_FULL
        }
    } else {
        EQUIP_ERR_ITEM_NOT_FOUND
    };
    send_inventory_change_failure(
        stream,
        result,
        Some(ObjectGuid::new(HighGuid::Item, 0, src_item.item)),
        None,
        header_crypto,
    )
    .await
}

pub(in crate::world) struct InventoryDeps<'a> {
    pub(in crate::world) character_db_pool: &'a MySqlPool,
    pub(in crate::world) world_db_pool: &'a MySqlPool,
    pub(in crate::world) shared_world: SharedWorldDeps<'a>,
}

pub(in crate::world) async fn handle_auto_bank_item(
    stream: &mut WorldPacketSink,
    deps: InventoryDeps<'_>,
    request: wow_proto::BankItemRequest,
    session: &mut WorldSessionState,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    let src_bag = normalize_client_bag(request.src_bag);
    let Some(character) = &session.character.active_character else {
        warn!("Ignoring autobank item before character login");
        return Ok(());
    };
    let Some((dst_bag, dst_slot)) =
        first_auto_bank_destination(deps.world_db_pool, session, src_bag, request.src_slot).await?
    else {
        return send_inventory_change_failure(
            stream,
            EQUIP_ERR_BANK_FULL,
            None,
            None,
            header_crypto,
        )
        .await;
    };
    debug!(
        guid = character.guid,
        src_bag,
        src_slot = request.src_slot,
        dst_bag,
        dst_slot,
        "Auto-banking item into bank storage"
    );
    handle_inventory_swap(
        stream,
        deps,
        wow_proto::InventoryMoveClientRequest::SwapItem {
            dst_bag: client_bag_id(dst_bag),
            dst_slot,
            src_bag: client_bag_id(src_bag),
            src_slot: request.src_slot,
        },
        session,
        header_crypto,
    )
    .await
}

pub(in crate::world) async fn handle_auto_store_bank_item(
    stream: &mut WorldPacketSink,
    deps: InventoryDeps<'_>,
    request: wow_proto::BankItemRequest,
    session: &mut WorldSessionState,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    let src_bag = normalize_client_bag(request.src_bag);
    let Some((dst_bag, dst_slot)) =
        first_auto_store_bank_destination(deps.world_db_pool, session, src_bag, request.src_slot)
            .await?
    else {
        return send_inventory_change_failure(
            stream,
            if is_bank_position(src_bag, request.src_slot) {
                EQUIP_ERR_INVENTORY_FULL
            } else {
                EQUIP_ERR_BANK_FULL
            },
            None,
            None,
            header_crypto,
        )
        .await;
    };
    handle_inventory_swap(
        stream,
        deps,
        wow_proto::InventoryMoveClientRequest::SwapItem {
            dst_bag: client_bag_id(dst_bag),
            dst_slot,
            src_bag: client_bag_id(src_bag),
            src_slot: request.src_slot,
        },
        session,
        header_crypto,
    )
    .await
}

pub(in crate::world) fn inventory_move_client_request_opcode(
    request: wow_proto::InventoryMoveClientRequest,
) -> u32 {
    match request {
        wow_proto::InventoryMoveClientRequest::AutoEquip { .. } => {
            WorldOpcode::CmsgAutoequipItem as u32
        }
        wow_proto::InventoryMoveClientRequest::AutoStoreBag { .. } => {
            WorldOpcode::CmsgAutostoreBagItem as u32
        }
        wow_proto::InventoryMoveClientRequest::SwapItem { .. } => WorldOpcode::CmsgSwapItem as u32,
        wow_proto::InventoryMoveClientRequest::SwapInvItem { .. } => {
            WorldOpcode::CmsgSwapInvItem as u32
        }
    }
}

pub(in crate::world) async fn handle_destroy_item(
    stream: &mut WorldPacketSink,
    deps: QuestMutationDeps<'_>,
    request: wow_proto::DestroyItemRequest,
    session: &mut WorldSessionState,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    let character_db_pool = deps.character_db_pool;
    let world_db_pool = deps.world_db_pool;
    let Some(character) = &session.character.active_character else {
        warn!("Ignoring item destroy before character login");
        return Ok(());
    };
    let character_guid = character.guid;
    let request = DestroyItemRequest::from(request);

    if !request.is_supported_destroy() {
        info!(
            bag = request.bag,
            slot = request.slot,
            count = request.count,
            "Ignoring unsupported item destroy outside bag-0 or equipped bag storage"
        );
        return Ok(());
    }

    let Some(source_item) = session
        .inventory
        .items
        .iter()
        .find(|item| item.bag == request.bag as u32 && item.slot == request.slot)
    else {
        warn!(
            guid = character_guid,
            bag = request.bag,
            slot = request.slot,
            "Rejected item destroy without source item"
        );
        return Ok(());
    };

    let Some(template) =
        wow_db::get_item_template_query(world_db_pool, source_item.item_template).await?
    else {
        warn!(
            guid = character_guid,
            item_template = source_item.item_template,
            "Rejected item destroy for missing item template"
        );
        return Ok(());
    };
    if template.flags & ITEM_FLAG_NO_USER_DESTROY != 0 {
        info!(
            guid = character_guid,
            item_template = source_item.item_template,
            "Rejected no-user-destroy item destroy"
        );
        return send_inventory_change_failure(
            stream,
            EQUIP_ERR_CANT_DROP_SOULBOUND,
            None,
            None,
            header_crypto,
        )
        .await;
    }

    let destroyed = wow_db::destroy_character_inventory_item_count(
        character_db_pool,
        character_guid,
        request.bag as u32,
        request.slot,
        request.count as u32,
    )
    .await?;
    let Some(destroyed) = destroyed else {
        warn!(
            guid = character_guid,
            bag = request.bag,
            slot = request.slot,
            "Rejected item destroy without DB source item"
        );
        return Ok(());
    };

    session.inventory.items =
        wow_db::get_character_inventory_items(character_db_pool, character_guid).await?;
    match destroyed {
        wow_db::InventoryDestroyResult::CountChanged { item, count } => {
            let body = build_item_stack_count_update_body(item, count)?;
            send_packet(
                stream,
                WorldOpcode::SmsgUpdateObject as u16,
                &body,
                Some(&mut *header_crypto),
            )
            .await?;
        }
        wow_db::InventoryDestroyResult::Removed { item } => {
            let update_blocks = build_inventory_position_update_blocks(
                character_guid,
                &session.inventory.items,
                request.bag,
                request.slot,
            )?;
            if !update_blocks.is_empty() {
                let body = build_update_object_body(&update_blocks);
                send_packet(
                    stream,
                    WorldOpcode::SmsgUpdateObject as u16,
                    &body,
                    Some(&mut *header_crypto),
                )
                .await?;
            }
            if request.bag != INVENTORY_SLOT_BAG_0 {
                let body = build_destroy_object_body(item);
                send_packet(
                    stream,
                    WorldOpcode::SmsgDestroyObject as u16,
                    &body,
                    Some(&mut *header_crypto),
                )
                .await?;
            }
        }
    };

    revalidate_completed_item_quests_after_inventory_change(
        stream,
        deps,
        session,
        character_guid,
        header_crypto,
    )
    .await
}

pub(in crate::world) async fn handle_split_item(
    stream: &mut WorldPacketSink,
    character_db_pool: &MySqlPool,
    request: wow_proto::SplitItemRequest,
    session: &mut WorldSessionState,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    let Some(character) = &session.character.active_character else {
        warn!("Ignoring item split before character login");
        return Ok(());
    };
    let character_guid = character.guid;
    let request = SplitItemRequest::from(request);
    if !request.is_supported_split()
        || request.src_bag == request.dst_bag && request.src_slot == request.dst_slot
    {
        info!(
            src_bag = request.src_bag,
            src_slot = request.src_slot,
            dst_bag = request.dst_bag,
            dst_slot = request.dst_slot,
            count = request.count,
            "Ignoring unsupported item split outside bag-0 or equipped bag storage"
        );
        return Ok(());
    }

    let split = wow_db::split_character_inventory_item(
        character_db_pool,
        character_guid,
        request.src_bag as u32,
        request.src_slot,
        request.dst_bag as u32,
        request.dst_slot,
        request.count as u32,
    )
    .await?;
    let Some(split) = split else {
        warn!(
            guid = character_guid,
            src_bag = request.src_bag,
            src_slot = request.src_slot,
            dst_bag = request.dst_bag,
            dst_slot = request.dst_slot,
            "Rejected item split"
        );
        return send_inventory_change_failure(
            stream,
            EQUIP_ERR_COULDNT_SPLIT_ITEMS,
            None,
            None,
            header_crypto,
        )
        .await;
    };

    session.inventory.items =
        wow_db::get_character_inventory_items(character_db_pool, character_guid).await?;
    let mut blocks = vec![build_item_stack_count_update_block(
        split.source_item,
        split.source_count,
    )?];
    if let Some(new_item) = session
        .inventory
        .items
        .iter()
        .find(|item| item.item == split.new_item)
    {
        blocks.extend(build_stored_item_create_update_blocks(
            character_guid,
            &session.inventory.items,
            new_item,
            None,
        )?);
    }
    let body = build_update_object_body(&blocks);
    send_packet(
        stream,
        WorldOpcode::SmsgUpdateObject as u16,
        &body,
        Some(header_crypto),
    )
    .await
}

pub(in crate::world) async fn handle_set_ammo(
    stream: &mut WorldPacketSink,
    deps: InventoryDeps<'_>,
    request: wow_proto::SetAmmoRequest,
    session: &mut WorldSessionState,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    let Some(character) = session.character.active_character.as_ref().cloned() else {
        warn!("Ignoring ammo selection before character login");
        return Ok(());
    };
    if session.death.player_death_state != PlayerDeathState::Alive {
        return send_inventory_change_failure(
            stream,
            EQUIP_ERR_YOU_ARE_DEAD,
            None,
            None,
            header_crypto,
        )
        .await;
    }

    if request.item == 0 {
        return apply_player_ammo_selection(stream, deps, session, &character, 0, header_crypto)
            .await;
    }

    let Some(template) = wow_db::get_item_template_query(deps.world_db_pool, request.item).await?
    else {
        return send_inventory_change_failure(
            stream,
            EQUIP_ERR_ITEM_NOT_FOUND,
            None,
            None,
            header_crypto,
        )
        .await;
    };
    let source_item = session
        .inventory
        .items
        .iter()
        .find(|item| item.item_template == request.item && item.count > 0);
    if source_item.is_none() {
        return send_inventory_change_failure(
            stream,
            EQUIP_ERR_ITEM_NOT_FOUND,
            None,
            None,
            header_crypto,
        )
        .await;
    }
    if !is_selectable_ammo_template(&template) {
        return send_inventory_change_failure(
            stream,
            EQUIP_ERR_ITEM_CANT_BE_EQUIPPED,
            source_item.map(|item| ObjectGuid::new(HighGuid::Item, 0, item.item)),
            None,
            header_crypto,
        )
        .await;
    }
    let use_result = character_can_use_item_template(
        character.level,
        character.race,
        character.class,
        &template,
        &session.character.character_skills,
        &session.character.active_spells,
        &session.character.character_reputations,
    );
    if use_result != 0 {
        return send_inventory_change_failure_with_required_level(
            stream,
            use_result,
            source_item.map(|item| ObjectGuid::new(HighGuid::Item, 0, item.item)),
            None,
            (use_result == EQUIP_ERR_CANT_EQUIP_LEVEL_I).then_some(template.required_level),
            header_crypto,
        )
        .await;
    }

    apply_player_ammo_selection(
        stream,
        deps,
        session,
        &character,
        template.entry,
        header_crypto,
    )
    .await
}

pub(in crate::world) async fn apply_player_ammo_selection(
    stream: &mut WorldPacketSink,
    deps: InventoryDeps<'_>,
    session: &mut WorldSessionState,
    character: &ActiveCharacter,
    ammo_id: u32,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    if session.character.player_ammo_id == ammo_id {
        return Ok(());
    }
    wow_db::update_character_ammo_id(deps.character_db_pool, character.guid, ammo_id).await?;
    session.character.player_ammo_id = ammo_id;

    let world_stats = wow_db::get_player_world_stats(
        deps.world_db_pool,
        character.race,
        character.class,
        character.level,
    )
    .await?;
    let equipped_templates = load_equipped_item_templates_with_enchantments(
        deps.world_db_pool,
        &session.inventory.items,
        &session
            .movement
            .db_creature_navigation
            .world_data_files
            .spell_item_enchantments,
    )
    .await?;
    let ammo_template =
        load_selected_ammo_template(deps.world_db_pool, &session.inventory.items, ammo_id).await?;
    let (base_combat_stats, combat_stats) = inventory_recomputed_combat_stats(
        character.class,
        character.level,
        world_stats,
        &equipped_templates,
        ammo_template.as_ref(),
        &session.character.character_skills,
        &session.character.active_spells,
        &session.auras.active_auras,
    );
    let observer_packets = deps
        .shared_world
        .maps
        .update_player_combat_stats(character.position.map_id, character.guid, base_combat_stats)
        .await?;
    deps.shared_world.sessions.dispatch(observer_packets).await;

    send_packet(
        stream,
        WorldOpcode::SmsgUpdateObject as u16,
        &build_player_ammo_update_body(character.guid, ammo_id)?,
        Some(&mut *header_crypto),
    )
    .await?;
    send_packet(
        stream,
        WorldOpcode::SmsgUpdateObject as u16,
        &build_player_combat_stats_update_body(character.guid, &combat_stats)?,
        Some(header_crypto),
    )
    .await
}

pub(in crate::world) fn is_selectable_ammo_template(template: &ItemTemplateQuery) -> bool {
    template.class == ITEM_CLASS_PROJECTILE
        && template.inventory_type == INVTYPE_AMMO
        && matches!(
            template.subclass,
            ITEM_SUBCLASS_ARROW | ITEM_SUBCLASS_BULLET
        )
}

pub(in crate::world) async fn load_selected_ammo_template(
    world_db_pool: &MySqlPool,
    inventory: &[CharacterInventoryItem],
    ammo_id: u32,
) -> anyhow::Result<Option<ItemTemplateQuery>> {
    if ammo_id == 0
        || !inventory
            .iter()
            .any(|item| item.item_template == ammo_id && item.count > 0)
    {
        return Ok(None);
    }
    let Some(template) = wow_db::get_item_template_query(world_db_pool, ammo_id).await? else {
        return Ok(None);
    };
    Ok(is_selectable_ammo_template(&template).then_some(template))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::world) struct InventoryMoveRequest {
    pub(in crate::world) src_bag: u8,
    pub(in crate::world) src_slot: u8,
    pub(in crate::world) dst_bag: u8,
    pub(in crate::world) dst_slot: u8,
}

impl InventoryMoveRequest {
    #[cfg(test)]
    pub(in crate::world) fn read(opcode: u32, body: &[u8]) -> anyhow::Result<Self> {
        let mut body = body;
        let request = match WorldOpcode::try_from(opcode).ok() {
            Some(WorldOpcode::CmsgSwapInvItem) => {
                wow_proto::InventoryMoveClientRequest::read_swap_inv_item(&mut body)?
            }
            Some(WorldOpcode::CmsgSwapItem) => {
                wow_proto::InventoryMoveClientRequest::read_swap_item(&mut body)?
            }
            _ => anyhow::bail!("unsupported inventory opcode 0x{opcode:04X}"),
        };
        Self::from_client_request(request)
    }

    pub(in crate::world) fn from_client_request(
        request: wow_proto::InventoryMoveClientRequest,
    ) -> anyhow::Result<Self> {
        match request {
            wow_proto::InventoryMoveClientRequest::SwapInvItem { src_slot, dst_slot } => Ok(Self {
                src_bag: INVENTORY_SLOT_BAG_0,
                src_slot,
                dst_bag: INVENTORY_SLOT_BAG_0,
                dst_slot,
            }),
            wow_proto::InventoryMoveClientRequest::SwapItem {
                dst_bag,
                dst_slot,
                src_bag,
                src_slot,
            } => Ok(Self {
                dst_bag: normalize_client_bag(dst_bag),
                dst_slot,
                src_bag: normalize_client_bag(src_bag),
                src_slot,
            }),
            wow_proto::InventoryMoveClientRequest::AutoEquip { .. }
            | wow_proto::InventoryMoveClientRequest::AutoStoreBag { .. } => {
                anyhow::bail!("auto inventory requests require async slot resolution")
            }
        }
    }

    pub(in crate::world) async fn read_auto_equip(
        src_bag: u8,
        src_slot: u8,
        world_db_pool: &MySqlPool,
        session: &WorldSessionState,
    ) -> anyhow::Result<Option<Self>> {
        let src_bag = normalize_client_bag(src_bag);
        let Some(src_item) = session
            .inventory
            .items
            .iter()
            .find(|item| item.bag == src_bag as u32 && item.slot == src_slot)
        else {
            return Ok(None);
        };
        let Some(template) =
            wow_db::get_item_template_query(world_db_pool, src_item.item_template).await?
        else {
            return Ok(None);
        };
        let Some(dst_slot) = preferred_equipment_slot_for_inventory(
            &template,
            &session.inventory.items,
            &session.character.active_spells,
        ) else {
            return Ok(None);
        };
        Ok(Some(Self {
            src_bag,
            src_slot,
            dst_bag: INVENTORY_SLOT_BAG_0,
            dst_slot,
        }))
    }

    pub(in crate::world) async fn read_auto_store_bag(
        src_bag: u8,
        src_slot: u8,
        dst_bag: u8,
        world_db_pool: &MySqlPool,
        session: &WorldSessionState,
    ) -> anyhow::Result<Option<Self>> {
        let src_bag = normalize_client_bag(src_bag);
        let dst_bag = normalize_client_bag(dst_bag);
        let Some(src_item) = session
            .inventory
            .items
            .iter()
            .find(|item| item.bag == src_bag as u32 && item.slot == src_slot)
        else {
            return Ok(None);
        };
        let Some(template) =
            wow_db::get_item_template_query(world_db_pool, src_item.item_template).await?
        else {
            return Ok(None);
        };
        if is_bank_bag_slot(dst_bag) {
            let bank_bags = load_bank_bag_infos(world_db_pool, &session.inventory.items).await?;
            return Ok(first_bank_store_destination(
                &session.inventory.items,
                src_item,
                &template,
                &bank_bags,
                bank_bag_slot_count(session),
                Some(dst_bag),
            )
            .map(|(dst_bag, dst_slot)| Self {
                src_bag,
                src_slot,
                dst_bag,
                dst_slot,
            }));
        }
        let equipped_bags =
            load_equipped_bag_infos(world_db_pool, &session.inventory.items).await?;
        let Some((dst_bag, dst_slot)) = first_autostore_destination(
            &session.inventory.items,
            src_item,
            &template,
            &equipped_bags,
            dst_bag,
        ) else {
            return Ok(None);
        };
        Ok(Some(Self {
            src_bag,
            src_slot,
            dst_bag,
            dst_slot,
        }))
    }

    pub(in crate::world) fn is_supported_inventory_move(&self) -> bool {
        is_supported_move_position(self.src_bag, self.src_slot)
            && is_supported_move_position(self.dst_bag, self.dst_slot)
    }

    pub(in crate::world) fn uses_existing_storage(&self, bag_model: &InventoryBagModel) -> bool {
        bag_model.move_position_exists(InventoryPosition::new(self.src_bag, self.src_slot))
            && bag_model.move_position_exists(InventoryPosition::new(self.dst_bag, self.dst_slot))
    }

    pub(in crate::world) fn references_bank_storage(&self) -> bool {
        is_bank_position(self.src_bag, self.src_slot)
            || is_bank_position(self.dst_bag, self.dst_slot)
    }

    pub(in crate::world) fn references_unpurchased_bank_bag_slot(
        &self,
        bank_bag_slot_count: u8,
    ) -> bool {
        unpurchased_bank_bag_slot(self.src_bag, self.src_slot, bank_bag_slot_count)
            || unpurchased_bank_bag_slot(self.dst_bag, self.dst_slot, bank_bag_slot_count)
    }

    pub(in crate::world) fn moves_equipped_bag_into_itself(&self) -> bool {
        (self.src_bag == INVENTORY_SLOT_BAG_0
            && is_bag_slot(self.src_slot)
            && self.dst_bag == self.src_slot)
            || (self.dst_bag == INVENTORY_SLOT_BAG_0
                && is_bag_slot(self.dst_slot)
                && self.src_bag == self.dst_slot)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::world) struct DestroyItemRequest {
    pub(in crate::world) bag: u8,
    pub(in crate::world) slot: u8,
    pub(in crate::world) count: u8,
}

impl DestroyItemRequest {
    pub(in crate::world) fn is_supported_destroy(&self) -> bool {
        is_supported_storage_position(self.bag, self.slot)
    }
}

impl From<wow_proto::DestroyItemRequest> for DestroyItemRequest {
    fn from(request: wow_proto::DestroyItemRequest) -> Self {
        Self {
            bag: normalize_client_bag(request.bag),
            slot: request.slot,
            count: request.count,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::world) struct SplitItemRequest {
    pub(in crate::world) src_bag: u8,
    pub(in crate::world) src_slot: u8,
    pub(in crate::world) dst_bag: u8,
    pub(in crate::world) dst_slot: u8,
    pub(in crate::world) count: u8,
}

impl SplitItemRequest {
    pub(in crate::world) fn is_supported_split(&self) -> bool {
        self.count != 0
            && is_supported_storage_position(self.src_bag, self.src_slot)
            && is_supported_storage_position(self.dst_bag, self.dst_slot)
    }
}

impl From<wow_proto::SplitItemRequest> for SplitItemRequest {
    fn from(request: wow_proto::SplitItemRequest) -> Self {
        Self {
            src_bag: normalize_client_bag(request.src_bag),
            src_slot: request.src_slot,
            dst_bag: normalize_client_bag(request.dst_bag),
            dst_slot: request.dst_slot,
            count: request.count,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::world) struct EquippedBagInfo {
    pub(in crate::world) slot: u8,
    pub(in crate::world) container_slots: u8,
    pub(in crate::world) class: u32,
    pub(in crate::world) subclass: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::world) struct StoreSlot {
    pub(in crate::world) bag: u8,
    pub(in crate::world) slot: u8,
    pub(in crate::world) count: u32,
    pub(in crate::world) existing_item: Option<u32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(in crate::world) struct InventoryPosition {
    pub(in crate::world) bag: u8,
    pub(in crate::world) slot: u8,
}

impl InventoryPosition {
    pub(in crate::world) const fn new(bag: u8, slot: u8) -> Self {
        Self { bag, slot }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::world) enum InventoryStorageScope {
    Inventory,
    Bank,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::world) struct InventoryBagModel {
    equipped_bags: Vec<EquippedBagInfo>,
    bank_bags: Vec<EquippedBagInfo>,
    bank_bag_slot_count: u8,
}

impl InventoryBagModel {
    pub(in crate::world) async fn load(
        world_db_pool: &MySqlPool,
        inventory: &[CharacterInventoryItem],
        bank_bag_slot_count: u8,
    ) -> anyhow::Result<Self> {
        Ok(Self {
            equipped_bags: load_equipped_bag_infos(world_db_pool, inventory).await?,
            bank_bags: load_bank_bag_infos(world_db_pool, inventory).await?,
            bank_bag_slot_count,
        })
    }

    pub(in crate::world) async fn load_inventory(
        world_db_pool: &MySqlPool,
        inventory: &[CharacterInventoryItem],
    ) -> anyhow::Result<Self> {
        Ok(Self {
            equipped_bags: load_equipped_bag_infos(world_db_pool, inventory).await?,
            bank_bags: Vec::new(),
            bank_bag_slot_count: 0,
        })
    }

    pub(in crate::world) async fn load_bank(
        world_db_pool: &MySqlPool,
        inventory: &[CharacterInventoryItem],
        bank_bag_slot_count: u8,
    ) -> anyhow::Result<Self> {
        Ok(Self {
            equipped_bags: Vec::new(),
            bank_bags: load_bank_bag_infos(world_db_pool, inventory).await?,
            bank_bag_slot_count,
        })
    }

    pub(in crate::world) fn inventory_only(equipped_bags: &[EquippedBagInfo]) -> Self {
        Self {
            equipped_bags: equipped_bags.to_vec(),
            bank_bags: Vec::new(),
            bank_bag_slot_count: 0,
        }
    }

    pub(in crate::world) fn bank_only(
        bank_bags: &[EquippedBagInfo],
        bank_bag_slot_count: u8,
    ) -> Self {
        Self {
            equipped_bags: Vec::new(),
            bank_bags: bank_bags.to_vec(),
            bank_bag_slot_count,
        }
    }

    pub(in crate::world) fn equipped_bags(&self) -> &[EquippedBagInfo] {
        &self.equipped_bags
    }

    pub(in crate::world) fn bank_bag_slot_count(&self) -> u8 {
        self.bank_bag_slot_count
    }

    pub(in crate::world) fn move_position_exists(&self, position: InventoryPosition) -> bool {
        if position.bag == INVENTORY_SLOT_BAG_0 {
            return position.slot < EQUIPMENT_SLOT_END
                || is_inventory_bag_slot(position.slot)
                || is_backpack_item_slot(position.slot)
                || is_bank_item_slot(position.slot)
                || (is_bank_bag_slot(position.slot)
                    && purchased_bank_bag_slot_index(position.slot)
                        .is_some_and(|index| index < self.bank_bag_slot_count));
        }
        self.equipped_bags
            .iter()
            .chain(self.bank_bags.iter())
            .any(|bag| bag.slot == position.bag && position.slot < bag.container_slots)
    }

    pub(in crate::world) fn storage_position_exists(
        &self,
        scope: InventoryStorageScope,
        position: InventoryPosition,
    ) -> bool {
        if position.bag == INVENTORY_SLOT_BAG_0 {
            return match scope {
                InventoryStorageScope::Inventory => is_backpack_item_slot(position.slot),
                InventoryStorageScope::Bank => {
                    is_bank_item_slot(position.slot)
                        || (is_bank_bag_slot(position.slot)
                            && purchased_bank_bag_slot_index(position.slot)
                                .is_some_and(|index| index < self.bank_bag_slot_count))
                }
            };
        }
        self.bags_for_scope(scope).iter().any(|bag| {
            bag.slot == position.bag
                && position.slot < bag.container_slots
                && (scope == InventoryStorageScope::Inventory
                    || purchased_bank_bag_slot_index(position.bag)
                        .is_some_and(|index| index < self.bank_bag_slot_count))
        })
    }

    pub(in crate::world) fn bag_accepts_item(
        &self,
        scope: InventoryStorageScope,
        bag: u8,
        template: &ItemTemplateQuery,
    ) -> bool {
        if bag == INVENTORY_SLOT_BAG_0 {
            return true;
        }
        self.bags_for_scope(scope)
            .iter()
            .find(|equipped| equipped.slot == bag)
            .is_some_and(|equipped| item_can_go_into_bag(template, equipped))
    }

    pub(in crate::world) fn any_storage_bag_accepts_item(
        &self,
        bag: u8,
        template: &ItemTemplateQuery,
    ) -> bool {
        if bag == INVENTORY_SLOT_BAG_0 {
            return true;
        }
        self.bag_accepts_item(InventoryStorageScope::Inventory, bag, template)
            || self.bag_accepts_item(InventoryStorageScope::Bank, bag, template)
    }

    pub(in crate::world) fn slot_range(
        &self,
        scope: InventoryStorageScope,
        bag: u8,
    ) -> Option<(u8, u8)> {
        if bag == INVENTORY_SLOT_BAG_0 {
            return Some(match scope {
                InventoryStorageScope::Inventory => {
                    (INVENTORY_SLOT_ITEM_START, INVENTORY_SLOT_ITEM_END)
                }
                InventoryStorageScope::Bank => (BANK_SLOT_ITEM_START, BANK_SLOT_ITEM_END),
            });
        }
        self.bags_for_scope(scope)
            .iter()
            .find(|equipped| {
                equipped.slot == bag
                    && (scope == InventoryStorageScope::Inventory
                        || purchased_bank_bag_slot_index(bag)
                            .is_some_and(|index| index < self.bank_bag_slot_count))
            })
            .map(|equipped| (0, equipped.container_slots))
    }

    pub(in crate::world) fn store_bag_order(
        &self,
        scope: InventoryStorageScope,
        template: &ItemTemplateQuery,
        specific_bag: Option<u8>,
    ) -> Vec<u8> {
        if let Some(bag) = specific_bag {
            return if self.bag_accepts_item(scope, bag, template) {
                vec![bag]
            } else {
                Vec::new()
            };
        }

        let mut bags = vec![INVENTORY_SLOT_BAG_0];
        if template.bag_family != 0 {
            bags.extend(
                self.bags_for_scope(scope)
                    .iter()
                    .filter(|bag| {
                        !is_normal_container_bag(bag) && item_can_go_into_bag(template, bag)
                    })
                    .map(|bag| bag.slot),
            );
        }
        bags.extend(
            self.bags_for_scope(scope)
                .iter()
                .filter(|bag| is_normal_container_bag(bag) && item_can_go_into_bag(template, bag))
                .map(|bag| bag.slot),
        );
        bags
    }

    pub(in crate::world) fn plan_store_item(
        &self,
        scope: InventoryStorageScope,
        inventory: &[CharacterInventoryItem],
        template: &ItemTemplateQuery,
        count: u32,
        specific_bag: Option<u8>,
        skip_item: Option<u32>,
    ) -> Option<Vec<StoreSlot>> {
        if count == 0 {
            return Some(Vec::new());
        }

        let max_stack = template.stackable.max(1);
        let mut remaining = count;
        let mut dest = Vec::new();
        let bags = self.store_bag_order(scope, template, specific_bag);

        if max_stack > 1 {
            for bag in &bags {
                for item in inventory.iter().filter(|item| {
                    item.bag == *bag as u32
                        && item.item_template == template.entry
                        && Some(item.item) != skip_item
                        && item.count < max_stack
                        && self
                            .storage_position_exists(scope, InventoryPosition::new(*bag, item.slot))
                }) {
                    let move_count = remaining.min(max_stack - item.count);
                    if move_count == 0 {
                        continue;
                    }
                    dest.push(StoreSlot {
                        bag: *bag,
                        slot: item.slot,
                        count: move_count,
                        existing_item: Some(item.item),
                    });
                    remaining -= move_count;
                    if remaining == 0 {
                        return Some(dest);
                    }
                }
            }
        }

        for bag in &bags {
            let Some((start, end)) = self.slot_range(scope, *bag) else {
                continue;
            };
            for slot in start..end {
                if inventory.iter().any(|item| {
                    item.bag == *bag as u32 && item.slot == slot && Some(item.item) != skip_item
                }) {
                    continue;
                }
                let move_count = remaining.min(max_stack);
                dest.push(StoreSlot {
                    bag: *bag,
                    slot,
                    count: move_count,
                    existing_item: None,
                });
                remaining -= move_count;
                if remaining == 0 {
                    return Some(dest);
                }
            }
        }

        None
    }

    fn bags_for_scope(&self, scope: InventoryStorageScope) -> &[EquippedBagInfo] {
        match scope {
            InventoryStorageScope::Inventory => &self.equipped_bags,
            InventoryStorageScope::Bank => &self.bank_bags,
        }
    }
}

pub(in crate::world) async fn load_equipped_bag_infos(
    world_db_pool: &MySqlPool,
    inventory: &[CharacterInventoryItem],
) -> anyhow::Result<Vec<EquippedBagInfo>> {
    load_bag_infos_for_slots(
        world_db_pool,
        inventory,
        INVENTORY_SLOT_BAG_START..INVENTORY_SLOT_BAG_END,
    )
    .await
}

pub(in crate::world) async fn load_bank_bag_infos(
    world_db_pool: &MySqlPool,
    inventory: &[CharacterInventoryItem],
) -> anyhow::Result<Vec<EquippedBagInfo>> {
    load_bag_infos_for_slots(
        world_db_pool,
        inventory,
        BANK_SLOT_BAG_START..BANK_SLOT_BAG_END,
    )
    .await
}

pub(in crate::world) async fn load_bag_infos_for_slots(
    world_db_pool: &MySqlPool,
    inventory: &[CharacterInventoryItem],
    slots: std::ops::Range<u8>,
) -> anyhow::Result<Vec<EquippedBagInfo>> {
    let mut bags = Vec::new();
    for slot in slots {
        let Some(item) = inventory
            .iter()
            .find(|item| item.bag == INVENTORY_SLOT_BAG_0 as u32 && item.slot == slot)
        else {
            continue;
        };
        let Some(template) =
            wow_db::get_item_template_query(world_db_pool, item.item_template).await?
        else {
            continue;
        };
        if template.container_slots == 0 {
            continue;
        }
        bags.push(EquippedBagInfo {
            slot,
            container_slots: template.container_slots.min(MAX_BAG_SIZE as u32) as u8,
            class: template.class,
            subclass: template.subclass,
        });
    }
    Ok(bags)
}

pub(in crate::world) fn preferred_equipment_slot_for_inventory(
    template: &ItemTemplateQuery,
    inventory: &[CharacterInventoryItem],
    active_spells: &HashSet<u32>,
) -> Option<u8> {
    if template.inventory_type == INVTYPE_BAG && template.container_slots > 0 {
        return (INVENTORY_SLOT_BAG_START..INVENTORY_SLOT_BAG_END).find(|slot| {
            inventory
                .iter()
                .all(|item| item.bag != INVENTORY_SLOT_BAG_0 as u32 || item.slot != *slot)
        });
    }
    if template.inventory_type == 13
        && character_can_dual_wield(active_spells)
        && inventory.iter().any(|item| {
            item.bag == INVENTORY_SLOT_BAG_0 as u32 && item.slot == EQUIPMENT_SLOT_MAINHAND
        })
        && inventory.iter().all(|item| {
            item.bag != INVENTORY_SLOT_BAG_0 as u32 || item.slot != EQUIPMENT_SLOT_OFFHAND
        })
    {
        return Some(EQUIPMENT_SLOT_OFFHAND);
    }
    preferred_equipment_slot(template.inventory_type)
}

pub(in crate::world) fn item_can_go_into_bag(
    item: &ItemTemplateQuery,
    bag: &EquippedBagInfo,
) -> bool {
    match bag.class {
        ITEM_CLASS_CONTAINER => match bag.subclass {
            ITEM_SUBCLASS_CONTAINER => true,
            ITEM_SUBCLASS_SOUL_CONTAINER => item.bag_family == BAG_FAMILY_SOUL_SHARDS,
            ITEM_SUBCLASS_HERB_CONTAINER => item.bag_family == BAG_FAMILY_HERBS,
            ITEM_SUBCLASS_ENCHANTING_CONTAINER => item.bag_family == BAG_FAMILY_ENCHANTING_SUPP,
            ITEM_SUBCLASS_ENGINEERING_CONTAINER => item.bag_family == BAG_FAMILY_ENGINEERING_SUPP,
            _ => false,
        },
        ITEM_CLASS_QUIVER => match bag.subclass {
            ITEM_SUBCLASS_QUIVER => item.bag_family == BAG_FAMILY_ARROWS,
            ITEM_SUBCLASS_AMMO_POUCH => item.bag_family == BAG_FAMILY_BULLETS,
            _ => false,
        },
        _ => false,
    }
}

pub(in crate::world) fn is_normal_container_bag(bag: &EquippedBagInfo) -> bool {
    bag.class == ITEM_CLASS_CONTAINER && bag.subclass == ITEM_SUBCLASS_CONTAINER
}

pub(in crate::world) async fn first_auto_bank_destination(
    world_db_pool: &MySqlPool,
    session: &WorldSessionState,
    src_bag: u8,
    src_slot: u8,
) -> anyhow::Result<Option<(u8, u8)>> {
    let Some(source) = session
        .inventory
        .items
        .iter()
        .find(|item| item.bag == src_bag as u32 && item.slot == src_slot)
    else {
        return Ok(None);
    };
    let Some(template) =
        wow_db::get_item_template_query(world_db_pool, source.item_template).await?
    else {
        return Ok(None);
    };
    let bag_model = InventoryBagModel::load_bank(
        world_db_pool,
        &session.inventory.items,
        bank_bag_slot_count(session),
    )
    .await?;
    Ok(bag_model
        .plan_store_item(
            InventoryStorageScope::Bank,
            &session.inventory.items,
            &template,
            source.count,
            None,
            Some(source.item),
        )
        .and_then(single_store_destination))
}

pub(in crate::world) async fn first_auto_store_bank_destination(
    world_db_pool: &MySqlPool,
    session: &WorldSessionState,
    src_bag: u8,
    src_slot: u8,
) -> anyhow::Result<Option<(u8, u8)>> {
    let Some(source) = session
        .inventory
        .items
        .iter()
        .find(|item| item.bag == src_bag as u32 && item.slot == src_slot)
    else {
        return Ok(None);
    };
    if !is_bank_position(src_bag, src_slot) {
        return first_auto_bank_destination(world_db_pool, session, src_bag, src_slot).await;
    }
    let Some(template) =
        wow_db::get_item_template_query(world_db_pool, source.item_template).await?
    else {
        return Ok(None);
    };
    let bag_model =
        InventoryBagModel::load_inventory(world_db_pool, &session.inventory.items).await?;
    Ok(bag_model
        .plan_store_item(
            InventoryStorageScope::Inventory,
            &session.inventory.items,
            &template,
            source.count,
            Some(INVENTORY_SLOT_BAG_0),
            Some(source.item),
        )
        .and_then(single_store_destination))
}

pub(in crate::world) fn first_autostore_destination(
    inventory: &[CharacterInventoryItem],
    source: &CharacterInventoryItem,
    template: &ItemTemplateQuery,
    equipped_bags: &[EquippedBagInfo],
    dst_bag: u8,
) -> Option<(u8, u8)> {
    InventoryBagModel::inventory_only(equipped_bags)
        .plan_store_item(
            InventoryStorageScope::Inventory,
            inventory,
            template,
            source.count,
            Some(dst_bag),
            Some(source.item),
        )
        .and_then(|dest| {
            if dest.len() == 1 {
                dest.first().map(|slot| (slot.bag, slot.slot))
            } else {
                None
            }
        })
}

pub(in crate::world) fn first_bank_store_destination(
    inventory: &[CharacterInventoryItem],
    source: &CharacterInventoryItem,
    template: &ItemTemplateQuery,
    bank_bags: &[EquippedBagInfo],
    bank_bag_slot_count: u8,
    specific_bag: Option<u8>,
) -> Option<(u8, u8)> {
    InventoryBagModel::bank_only(bank_bags, bank_bag_slot_count)
        .plan_store_item(
            InventoryStorageScope::Bank,
            inventory,
            template,
            source.count,
            specific_bag,
            Some(source.item),
        )
        .and_then(|dest| {
            if dest.len() == 1 {
                dest.first().map(|slot| (slot.bag, slot.slot))
            } else {
                None
            }
        })
}

pub(in crate::world) fn resolve_bag_icon_move_destination(
    inventory: &[CharacterInventoryItem],
    source: &CharacterInventoryItem,
    template: &ItemTemplateQuery,
    bag_model: &InventoryBagModel,
    request: &InventoryMoveRequest,
) -> Option<InventoryMoveRequest> {
    if request.dst_bag != INVENTORY_SLOT_BAG_0 || !is_bag_slot(request.dst_slot) {
        return None;
    }
    if template.inventory_type == INVTYPE_BAG && template.container_slots > 0 {
        return None;
    }
    let destination = if is_bank_bag_slot(request.dst_slot) {
        bag_model
            .plan_store_item(
                InventoryStorageScope::Bank,
                inventory,
                template,
                source.count,
                Some(request.dst_slot),
                Some(source.item),
            )
            .and_then(single_store_destination)
    } else {
        bag_model
            .plan_store_item(
                InventoryStorageScope::Inventory,
                inventory,
                template,
                source.count,
                Some(request.dst_slot),
                Some(source.item),
            )
            .and_then(single_store_destination)
    }?;
    Some(InventoryMoveRequest {
        src_bag: request.src_bag,
        src_slot: request.src_slot,
        dst_bag: destination.0,
        dst_slot: destination.1,
    })
}

fn single_store_destination(dest: Vec<StoreSlot>) -> Option<(u8, u8)> {
    if dest.len() == 1 {
        dest.first().map(|slot| (slot.bag, slot.slot))
    } else {
        None
    }
}

pub(in crate::world) fn normalize_client_bag(bag: u8) -> u8 {
    if bag == CLIENT_INVENTORY_SLOT_BAG_0 {
        INVENTORY_SLOT_BAG_0
    } else {
        bag
    }
}

pub(in crate::world) fn client_bag_id(bag: u8) -> u8 {
    if bag == INVENTORY_SLOT_BAG_0 {
        CLIENT_INVENTORY_SLOT_BAG_0
    } else {
        bag
    }
}

pub(in crate::world) fn is_backpack_item_slot(slot: u8) -> bool {
    (INVENTORY_SLOT_ITEM_START..INVENTORY_SLOT_ITEM_END).contains(&slot)
}

pub(in crate::world) fn is_inventory_bag_slot(slot: u8) -> bool {
    (INVENTORY_SLOT_BAG_START..INVENTORY_SLOT_BAG_END).contains(&slot)
}

pub(in crate::world) fn is_bank_item_slot(slot: u8) -> bool {
    (BANK_SLOT_ITEM_START..BANK_SLOT_ITEM_END).contains(&slot)
}

pub(in crate::world) fn is_bank_bag_slot(slot: u8) -> bool {
    (BANK_SLOT_BAG_START..BANK_SLOT_BAG_END).contains(&slot)
}

pub(in crate::world) fn is_bag_slot(slot: u8) -> bool {
    is_inventory_bag_slot(slot) || is_bank_bag_slot(slot)
}

pub(in crate::world) fn is_bank_position(bag: u8, slot: u8) -> bool {
    (bag == INVENTORY_SLOT_BAG_0 && (is_bank_item_slot(slot) || is_bank_bag_slot(slot)))
        || is_bank_bag_slot(bag)
}

pub(in crate::world) fn bank_bag_slot_count(session: &WorldSessionState) -> u8 {
    session
        .character
        .player_visual
        .as_ref()
        .map(|visual| ((visual.player_bytes2 >> 16) & 0xFF) as u8)
        .unwrap_or(0)
}

pub(in crate::world) fn with_bank_bag_slot_count(player_bytes2: u32, count: u8) -> u32 {
    (player_bytes2 & !(0xFF << 16)) | (u32::from(count) << 16)
}

pub(in crate::world) fn purchased_bank_bag_slot_index(slot: u8) -> Option<u8> {
    is_bank_bag_slot(slot).then_some(slot - BANK_SLOT_BAG_START)
}

pub(in crate::world) fn unpurchased_bank_bag_slot(
    bag: u8,
    slot: u8,
    bank_bag_slot_count: u8,
) -> bool {
    if bag == INVENTORY_SLOT_BAG_0 {
        return purchased_bank_bag_slot_index(slot)
            .is_some_and(|index| index >= bank_bag_slot_count);
    }
    purchased_bank_bag_slot_index(bag).is_some_and(|index| index >= bank_bag_slot_count)
}

pub(in crate::world) fn is_supported_storage_position(bag: u8, slot: u8) -> bool {
    (bag == INVENTORY_SLOT_BAG_0 && (slot < INVENTORY_SLOT_ITEM_END || is_bank_item_slot(slot)))
        || (is_bag_slot(bag) && slot < MAX_BAG_SIZE)
}

pub(in crate::world) fn is_supported_move_position(bag: u8, slot: u8) -> bool {
    (bag == INVENTORY_SLOT_BAG_0
        && (slot < EQUIPMENT_SLOT_END
            || is_bag_slot(slot)
            || is_backpack_item_slot(slot)
            || is_bank_item_slot(slot)))
        || (is_bag_slot(bag) && slot < MAX_BAG_SIZE)
}

pub(in crate::world) fn bag0_changed_slots(request: &InventoryMoveRequest) -> Vec<u8> {
    let mut slots = Vec::with_capacity(2);
    if request.src_bag == INVENTORY_SLOT_BAG_0 {
        slots.push(request.src_slot);
    }
    if request.dst_bag == INVENTORY_SLOT_BAG_0 && request.dst_slot != request.src_slot {
        slots.push(request.dst_slot);
    }
    slots
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::world) struct InventoryUpdatePlan {
    player_slots: Vec<u8>,
    container_slots_by_bag: std::collections::BTreeMap<u8, Vec<u8>>,
    contained_positions: Vec<InventoryPosition>,
}

impl InventoryUpdatePlan {
    pub(in crate::world) fn for_move(request: &InventoryMoveRequest) -> Self {
        let mut plan = Self::new();
        plan.touch_position(InventoryPosition::new(request.src_bag, request.src_slot));
        if request.dst_bag != request.src_bag || request.dst_slot != request.src_slot {
            plan.touch_position(InventoryPosition::new(request.dst_bag, request.dst_slot));
        }
        plan
    }

    pub(in crate::world) fn for_position(position: InventoryPosition) -> Self {
        let mut plan = Self::new();
        plan.touch_position(position);
        plan
    }

    pub(in crate::world) fn for_positions(
        positions: impl IntoIterator<Item = InventoryPosition>,
    ) -> Self {
        let mut plan = Self::new();
        for position in positions {
            plan.touch_position(position);
        }
        plan
    }

    pub(in crate::world) fn build_blocks(
        &self,
        character_guid: u32,
        inventory: &[CharacterInventoryItem],
    ) -> anyhow::Result<Vec<Vec<u8>>> {
        let mut blocks = Vec::new();
        if !self.player_slots.is_empty() {
            blocks.push(build_inventory_slots_update_block(
                character_guid,
                inventory,
                &self.player_slots,
            )?);
        }
        for (bag, slots) in &self.container_slots_by_bag {
            if let Some(block) = build_container_slots_update_block(inventory, *bag, slots)? {
                blocks.push(block);
            }
        }
        for position in &self.contained_positions {
            blocks.extend(build_contained_item_position_update_blocks(
                character_guid,
                inventory,
                position.bag,
                position.slot,
            )?);
        }
        Ok(blocks)
    }

    fn new() -> Self {
        Self {
            player_slots: Vec::new(),
            container_slots_by_bag: std::collections::BTreeMap::new(),
            contained_positions: Vec::new(),
        }
    }

    fn touch_position(&mut self, position: InventoryPosition) {
        if position.bag == INVENTORY_SLOT_BAG_0 {
            if !self.player_slots.contains(&position.slot) {
                self.player_slots.push(position.slot);
            }
            if !self.contained_positions.contains(&position) {
                self.contained_positions.push(position);
            }
            return;
        }
        if is_bag_slot(position.bag) {
            let slots = self.container_slots_by_bag.entry(position.bag).or_default();
            if !slots.contains(&position.slot) {
                slots.push(position.slot);
            }
            if !self.contained_positions.contains(&position) {
                self.contained_positions.push(position);
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub(in crate::world) fn inventory_recomputed_combat_stats(
    class: u8,
    level: u8,
    world_stats: PlayerWorldStats,
    equipped_templates: &[EquippedItemTemplate],
    ammo_template: Option<&ItemTemplateQuery>,
    skills: &[CharacterSkill],
    active_spells: &HashSet<u32>,
    active_auras: &[ActiveAura],
) -> (PlayerCombatStats, PlayerCombatStats) {
    let equipped_world_stats = player_world_stats_with_equipment(world_stats, equipped_templates);
    let effective_world_stats =
        player_world_stats_with_active_auras(equipped_world_stats, active_auras);
    let base_combat_stats = player_combat_stats_for_values_with_known_spells_and_ammo(
        class,
        level,
        &effective_world_stats,
        skills,
        active_spells,
        equipped_templates,
        ammo_template,
    );
    let effective_combat_stats = combat_stats_with_active_auras(base_combat_stats, active_auras);
    (base_combat_stats, effective_combat_stats)
}

pub(in crate::world) fn combat_equipment_slot_change_interrupts_active_spell_cast(
    changed_equipment_slots: &[u8],
    in_combat: bool,
) -> bool {
    in_combat
        && changed_equipment_slots.iter().any(|slot| {
            matches!(
                *slot,
                EQUIPMENT_SLOT_MAINHAND | EQUIPMENT_SLOT_OFFHAND | EQUIPMENT_SLOT_RANGED
            )
        })
}

pub(in crate::world) fn combat_equipment_slot_change_resets_main_hand_swing(
    changed_equipment_slots: &[u8],
    in_combat: bool,
) -> bool {
    in_combat && changed_equipment_slots.contains(&EQUIPMENT_SLOT_MAINHAND)
}

pub(in crate::world) fn build_inventory_move_update_blocks(
    character_guid: u32,
    inventory: &[CharacterInventoryItem],
    request: &InventoryMoveRequest,
) -> anyhow::Result<Vec<Vec<u8>>> {
    InventoryUpdatePlan::for_move(request).build_blocks(character_guid, inventory)
}

pub(in crate::world) fn build_inventory_position_update_blocks(
    character_guid: u32,
    inventory: &[CharacterInventoryItem],
    bag: u8,
    slot: u8,
) -> anyhow::Result<Vec<Vec<u8>>> {
    build_inventory_positions_update_blocks(
        character_guid,
        inventory,
        [InventoryPosition::new(bag, slot)],
    )
}

pub(in crate::world) fn build_inventory_positions_update_blocks(
    character_guid: u32,
    inventory: &[CharacterInventoryItem],
    positions: impl IntoIterator<Item = InventoryPosition>,
) -> anyhow::Result<Vec<Vec<u8>>> {
    InventoryUpdatePlan::for_positions(positions).build_blocks(character_guid, inventory)
}

pub(in crate::world) fn build_stored_item_create_update_blocks(
    character_guid: u32,
    inventory: &[CharacterInventoryItem],
    item: &CharacterInventoryItem,
    container_slots: Option<u32>,
) -> anyhow::Result<Vec<Vec<u8>>> {
    let owner_guid = ObjectGuid::new(HighGuid::Player, 0, character_guid);
    let contained_guid = item_contained_guid(owner_guid, inventory, item);
    let mut blocks = vec![build_item_create_update_block(
        owner_guid,
        contained_guid,
        item,
        container_slots,
    )?];
    let bag = u8::try_from(item.bag).map_err(|_| {
        anyhow::anyhow!("inventory item {} has invalid bag {}", item.item, item.bag)
    })?;
    blocks.extend(
        InventoryUpdatePlan::for_position(InventoryPosition::new(bag, item.slot))
            .build_blocks(character_guid, inventory)?,
    );
    Ok(blocks)
}

pub(in crate::world) fn build_contained_item_position_update_blocks(
    character_guid: u32,
    inventory: &[CharacterInventoryItem],
    bag: u8,
    slot: u8,
) -> anyhow::Result<Vec<Vec<u8>>> {
    if bag != INVENTORY_SLOT_BAG_0 && !is_bag_slot(bag) {
        return Ok(Vec::new());
    }
    let mut blocks = Vec::new();
    if let Some(item) = inventory
        .iter()
        .find(|item| item.bag == bag as u32 && item.slot == slot)
    {
        let owner_guid = ObjectGuid::new(HighGuid::Player, 0, character_guid);
        blocks.push(build_item_contained_update_block(
            owner_guid, inventory, item,
        )?);
    }
    Ok(blocks)
}

#[derive(Debug, Clone, Copy)]
pub(in crate::world) struct VendorListItem {
    pub(in crate::world) item: u32,
    pub(in crate::world) display: u32,
    pub(in crate::world) max_count: u32,
    pub(in crate::world) price: u32,
    pub(in crate::world) durability: u32,
    pub(in crate::world) buy_count: u32,
}

impl From<&wow_db::VendorItemQuery> for VendorListItem {
    fn from(item: &wow_db::VendorItemQuery) -> Self {
        Self::from_vendor_item(item, item.max_count)
    }
}

impl VendorListItem {
    pub(in crate::world) fn from_vendor_item(
        item: &wow_db::VendorItemQuery,
        max_count: u32,
    ) -> Self {
        Self {
            item: item.item,
            display: item.display_id,
            max_count,
            price: item.buy_price,
            durability: item.max_durability,
            buy_count: item.buy_count,
        }
    }
}

pub(in crate::world) fn build_vendor_inventory_body(
    vendor_guid: ObjectGuid,
    items: &[VendorListItem],
) -> Vec<u8> {
    SmsgListInventoryResponse {
        vendor_guid,
        items: items
            .iter()
            .map(|item| VendorListItemResponse {
                item: item.item,
                display: item.display,
                max_count: item.max_count,
                price: item.price,
                durability: item.durability,
                buy_count: item.buy_count,
            })
            .collect(),
    }
    .body()
}

pub(in crate::world) fn build_buy_item_body(
    vendor_guid: ObjectGuid,
    vendor_slot: u32,
    remaining_count: u32,
    count: u8,
) -> Vec<u8> {
    SmsgBuyItemResponse {
        vendor_guid,
        vendor_slot,
        remaining_count,
        count,
    }
    .body()
}

pub(in crate::world) fn build_buy_failed_body(
    vendor_guid: ObjectGuid,
    item: u32,
    result: u8,
) -> Vec<u8> {
    SmsgBuyFailedResponse {
        vendor_guid,
        item,
        result,
    }
    .body()
}

pub(in crate::world) fn build_sell_item_error_body(
    vendor_guid: ObjectGuid,
    item_guid: ObjectGuid,
    result: u8,
) -> Vec<u8> {
    SmsgSellItemResponse {
        vendor_guid,
        item_guid,
        result,
    }
    .body()
}

pub(in crate::world) fn preferred_equipment_slot(inventory_type: u32) -> Option<u8> {
    match inventory_type {
        1 => Some(0),             // INVTYPE_HEAD
        2 => Some(1),             // INVTYPE_NECK
        3 => Some(2),             // INVTYPE_SHOULDERS
        4 => Some(3),             // INVTYPE_BODY
        5 | 20 => Some(4),        // INVTYPE_CHEST / ROBE
        6 => Some(5),             // INVTYPE_WAIST
        7 => Some(6),             // INVTYPE_LEGS
        8 => Some(7),             // INVTYPE_FEET
        9 => Some(8),             // INVTYPE_WRISTS
        10 => Some(9),            // INVTYPE_HANDS
        11 => Some(10),           // INVTYPE_FINGER
        12 => Some(12),           // INVTYPE_TRINKET
        13 | 17 | 21 => Some(15), // one-hand/two-hand/main-hand weapon
        14 | 22 | 23 => Some(16), // shield/offhand/held-in-offhand
        15 | 25 | 26 => Some(17), // ranged/thrown/ranged right
        18 => Some(19),           // INVTYPE_BAG
        16 => Some(14),           // INVTYPE_CLOAK
        19 => Some(18),           // INVTYPE_TABARD
        _ => None,
    }
}

pub(in crate::world) async fn two_handed_equipment_conflict_after_move(
    world_db_pool: &MySqlPool,
    inventory: &[CharacterInventoryItem],
    request: &InventoryMoveRequest,
    src_inventory_type: u32,
    dst_inventory_type: Option<u32>,
) -> anyhow::Result<Option<u8>> {
    let current_mainhand =
        equipped_slot_inventory_type(world_db_pool, inventory, EQUIPMENT_SLOT_MAINHAND).await?;
    let current_offhand =
        equipped_slot_inventory_type(world_db_pool, inventory, EQUIPMENT_SLOT_OFFHAND).await?;
    let post_mainhand = equipment_slot_inventory_type_after_move(
        EQUIPMENT_SLOT_MAINHAND,
        current_mainhand,
        request,
        src_inventory_type,
        dst_inventory_type,
    );
    let post_offhand = equipment_slot_inventory_type_after_move(
        EQUIPMENT_SLOT_OFFHAND,
        current_offhand,
        request,
        src_inventory_type,
        dst_inventory_type,
    );
    Ok(two_handed_equipment_conflict(post_mainhand, post_offhand))
}

async fn equipped_slot_inventory_type(
    world_db_pool: &MySqlPool,
    inventory: &[CharacterInventoryItem],
    slot: u8,
) -> anyhow::Result<Option<u32>> {
    let Some(item) = inventory
        .iter()
        .find(|item| item.bag == INVENTORY_SLOT_BAG_0 as u32 && item.slot == slot)
    else {
        return Ok(None);
    };
    Ok(
        wow_db::get_item_template_query(world_db_pool, item.item_template)
            .await?
            .map(|template| template.inventory_type),
    )
}

pub(in crate::world) fn equipment_slot_inventory_type_after_move(
    slot: u8,
    current_inventory_type: Option<u32>,
    request: &InventoryMoveRequest,
    src_inventory_type: u32,
    dst_inventory_type: Option<u32>,
) -> Option<u32> {
    if request.dst_bag == INVENTORY_SLOT_BAG_0 && request.dst_slot == slot {
        return Some(src_inventory_type);
    }
    if request.src_bag == INVENTORY_SLOT_BAG_0 && request.src_slot == slot {
        return dst_inventory_type;
    }
    current_inventory_type
}

pub(in crate::world) fn two_handed_equipment_conflict(
    mainhand_inventory_type: Option<u32>,
    offhand_inventory_type: Option<u32>,
) -> Option<u8> {
    (mainhand_inventory_type == Some(INVTYPE_2HWEAPON) && offhand_inventory_type.is_some())
        .then_some(EQUIP_ERR_CANT_EQUIP_WITH_TWOHANDED)
}

pub(in crate::world) fn item_fits_equipment_slot(inventory_type: u32, slot: u8) -> bool {
    match slot {
        0 => inventory_type == 1,
        1 => inventory_type == 2,
        2 => inventory_type == 3,
        3 => inventory_type == 4,
        4 => matches!(inventory_type, 5 | 20),
        5 => inventory_type == 6,
        6 => inventory_type == 7,
        7 => inventory_type == 8,
        8 => inventory_type == 9,
        9 => inventory_type == 10,
        10 | 11 => inventory_type == 11,
        12 | 13 => inventory_type == 12,
        14 => inventory_type == 16,
        15 => matches!(inventory_type, 13 | 17 | 21),
        16 => matches!(inventory_type, 14 | 22 | 23),
        17 => matches!(inventory_type, 15 | 25 | 26),
        18 => inventory_type == 19,
        19..=22 => inventory_type == INVTYPE_BAG,
        _ => false,
    }
}

pub(in crate::world) const SPELL_PASSIVE_DUAL_WIELD: u32 = 674;

pub(in crate::world) fn character_can_dual_wield(active_spells: &HashSet<u32>) -> bool {
    active_spells.contains(&SPELL_PASSIVE_DUAL_WIELD)
}

pub(in crate::world) fn item_fits_equipment_slot_for_character(
    template: &ItemTemplateQuery,
    slot: u8,
    active_spells: &HashSet<u32>,
) -> bool {
    if slot == EQUIPMENT_SLOT_OFFHAND
        && template.inventory_type == 13
        && character_can_dual_wield(active_spells)
    {
        return true;
    }
    item_fits_equipment_slot(template.inventory_type, slot)
}

pub(in crate::world) fn item_can_change_equip_state_in_combat(
    template: &ItemTemplateQuery,
) -> bool {
    matches!(
        template.inventory_type,
        INVTYPE_RELIC | INVTYPE_SHIELD | INVTYPE_HOLDABLE
    ) || matches!(template.class, ITEM_CLASS_WEAPON | ITEM_CLASS_PROJECTILE)
}

pub(in crate::world) fn item_can_leave_bag0_equipment_or_bag_slot(
    slot: u8,
    template: &ItemTemplateQuery,
    in_combat: bool,
) -> Option<u8> {
    if !(slot < EQUIPMENT_SLOT_END || is_bag_slot(slot)) {
        return None;
    }
    (in_combat && !item_can_change_equip_state_in_combat(template))
        .then_some(EQUIP_ERR_NOT_IN_COMBAT)
}

#[derive(Debug, Clone, Copy)]
pub(in crate::world) struct CharacterEquipValidationContext<'a> {
    pub(in crate::world) level: u8,
    pub(in crate::world) race: u8,
    pub(in crate::world) class: u8,
    pub(in crate::world) equipped_bags: &'a [EquippedBagInfo],
    pub(in crate::world) ignore_bag_slot: Option<u8>,
    pub(in crate::world) skills: &'a [CharacterSkill],
    pub(in crate::world) active_spells: &'a HashSet<u32>,
    pub(in crate::world) reputations: &'a [CharacterReputation],
    pub(in crate::world) in_combat: bool,
}

pub(in crate::world) fn item_can_enter_bag0_equipment_or_bag_slot(
    slot: u8,
    template: &ItemTemplateQuery,
    context: CharacterEquipValidationContext<'_>,
) -> Option<(u8, Option<u32>)> {
    if !(slot < EQUIPMENT_SLOT_END || is_bag_slot(slot)) {
        return None;
    }
    if context.in_combat && !item_can_change_equip_state_in_combat(template) {
        return Some((EQUIP_ERR_NOT_IN_COMBAT, None));
    }
    let fits_destination = if slot < EQUIPMENT_SLOT_END {
        item_fits_equipment_slot_for_character(template, slot, context.active_spells)
    } else {
        template.inventory_type == INVTYPE_BAG && template.container_slots > 0
    };
    if !fits_destination {
        return Some((EQUIP_ERR_ITEM_DOESNT_GO_TO_SLOT, None));
    }
    if slot < EQUIPMENT_SLOT_END {
        let equip_result = character_can_equip_item_template(
            context.level,
            context.race,
            context.class,
            template,
            context.skills,
            context.active_spells,
            context.reputations,
        );
        if equip_result != 0 {
            return Some((
                equip_result,
                (equip_result == EQUIP_ERR_CANT_EQUIP_LEVEL_I).then_some(template.required_level),
            ));
        }
    } else {
        let use_result = character_can_use_item_template(
            context.level,
            context.race,
            context.class,
            template,
            context.skills,
            context.active_spells,
            context.reputations,
        );
        if use_result != 0 {
            return Some((
                use_result,
                (use_result == EQUIP_ERR_CANT_EQUIP_LEVEL_I).then_some(template.required_level),
            ));
        }
        if let Some(unique_error) = bag_slot_unique_equip_error(
            slot,
            template,
            context.equipped_bags,
            context.ignore_bag_slot,
        ) {
            return Some((unique_error, None));
        }
    }
    None
}

pub(in crate::world) fn bag_slot_unique_equip_error(
    slot: u8,
    template: &ItemTemplateQuery,
    equipped_bags: &[EquippedBagInfo],
    ignore_bag_slot: Option<u8>,
) -> Option<u8> {
    if template.class != ITEM_CLASS_QUIVER {
        return None;
    }
    let duplicate = equipped_bags.iter().find(|bag| {
        bag.slot != slot && Some(bag.slot) != ignore_bag_slot && bag.class == ITEM_CLASS_QUIVER
    });
    if let Some(duplicate) = duplicate {
        return Some(match duplicate.subclass {
            ITEM_SUBCLASS_AMMO_POUCH => EQUIP_ERR_CAN_EQUIP_ONLY1_AMMOPOUCH,
            _ => EQUIP_ERR_CAN_EQUIP_ONLY1_QUIVER,
        });
    }
    None
}

pub(in crate::world) fn character_can_equip_item_template(
    level: u8,
    race: u8,
    class: u8,
    template: &ItemTemplateQuery,
    skills: &[CharacterSkill],
    active_spells: &HashSet<u32>,
    reputations: &[CharacterReputation],
) -> u8 {
    if template.inventory_type == 0 {
        return EQUIP_ERR_ITEM_CANT_BE_EQUIPPED;
    }
    let use_result = character_can_use_item_template(
        level,
        race,
        class,
        template,
        skills,
        active_spells,
        reputations,
    );
    if use_result != 0 {
        return use_result;
    }
    if item_proficiency_skill(template).is_some_and(|skill| {
        !skills
            .iter()
            .any(|known| u32::from(known.skill) == skill && known.value > 0)
    }) {
        return EQUIP_ERR_NO_REQUIRED_PROFICIENCY;
    }
    0
}

pub(in crate::world) fn character_can_use_item_template(
    level: u8,
    race: u8,
    class: u8,
    template: &ItemTemplateQuery,
    skills: &[CharacterSkill],
    active_spells: &HashSet<u32>,
    reputations: &[CharacterReputation],
) -> u8 {
    if template.allowable_class != -1 {
        let class_mask = quest_race_or_class_mask(class);
        if class_mask == 0 || (template.allowable_class as u32 & class_mask) == 0 {
            return EQUIP_ERR_YOU_CAN_NEVER_USE_THAT_ITEM;
        }
    }
    if template.allowable_race != -1 {
        let race_mask = quest_race_or_class_mask(race);
        if race_mask == 0 || (template.allowable_race as u32 & race_mask) == 0 {
            return EQUIP_ERR_YOU_CAN_NEVER_USE_THAT_ITEM;
        }
    }
    if template.required_skill != 0 {
        let skill_value = skills
            .iter()
            .find(|skill| u32::from(skill.skill) == template.required_skill)
            .map(|skill| u32::from(skill.value))
            .unwrap_or(0);
        if skill_value == 0 {
            return EQUIP_ERR_NO_REQUIRED_PROFICIENCY;
        }
        if skill_value < template.required_skill_rank {
            return EQUIP_ERR_CANT_EQUIP_SKILL;
        }
    }
    if template.required_spell != 0 && !active_spells.contains(&template.required_spell) {
        return EQUIP_ERR_NO_REQUIRED_PROFICIENCY;
    }
    if template.required_honor_rank != 0 || template.required_city_rank != 0 {
        return EQUIP_ERR_CANT_EQUIP_RANK;
    }
    if u32::from(level) < template.required_level {
        return EQUIP_ERR_CANT_EQUIP_LEVEL_I;
    }
    if template.required_reputation_faction != 0 {
        let rank = reputations
            .iter()
            .find(|reputation| reputation.faction == template.required_reputation_faction)
            .map(|reputation| reputation_rank_from_standing(reputation.standing))
            .unwrap_or(3);
        if rank < template.required_reputation_rank {
            return EQUIP_ERR_CANT_EQUIP_REPUTATION;
        }
    }

    0
}

pub(in crate::world) fn reputation_rank_from_standing(standing: i32) -> u32 {
    match standing {
        i32::MIN..=-6001 => 0,
        -6000..=-3001 => 1,
        -3000..=-1 => 2,
        0..=2999 => 3,
        3000..=8999 => 4,
        9000..=20999 => 5,
        21000..=41999 => 6,
        _ => 7,
    }
}

pub(in crate::world) fn item_proficiency_skill(template: &ItemTemplateQuery) -> Option<u32> {
    // CMaNGOS reference: src/game/Entities/Item.cpp Item::GetSkill().
    match template.class {
        ITEM_CLASS_ARMOR => match template.subclass {
            1 => Some(415), // Cloth
            2 => Some(414), // Leather
            3 => Some(413), // Mail
            4 => Some(293), // Plate Mail
            6 => Some(433), // Shield
            _ => None,
        },
        ITEM_CLASS_WEAPON => match template.subclass {
            0 => Some(44),   // Axes
            1 => Some(172),  // Two-Handed Axes
            2 => Some(45),   // Bows
            3 => Some(46),   // Guns
            4 => Some(54),   // Maces
            5 => Some(160),  // Two-Handed Maces
            6 => Some(229),  // Polearms
            7 => Some(43),   // Swords
            8 => Some(55),   // Two-Handed Swords
            10 => Some(136), // Staves
            13 => Some(473), // Fist Weapons
            15 => Some(173), // Daggers
            16 => Some(176), // Thrown
            17 => Some(253), // Spears use CMaNGOS' subclass skill table entry.
            18 => Some(226), // Crossbows
            19 => Some(228), // Wands
            20 => Some(356), // Fishing Poles
            _ => None,
        },
        _ => None,
    }
}

pub(in crate::world) fn inventory_opcode_name(opcode: u32) -> &'static str {
    match WorldOpcode::try_from(opcode).ok() {
        Some(WorldOpcode::CmsgAutoequipItem) => "CMSG_AUTOEQUIP_ITEM",
        Some(WorldOpcode::CmsgAutostoreBagItem) => "CMSG_AUTOSTORE_BAG_ITEM",
        Some(WorldOpcode::CmsgSwapInvItem) => "CMSG_SWAP_INV_ITEM",
        Some(WorldOpcode::CmsgSwapItem) => "CMSG_SWAP_ITEM",
        Some(WorldOpcode::CmsgSplitItem) => "CMSG_SPLIT_ITEM",
        Some(WorldOpcode::CmsgDestroyItem) => "CMSG_DESTROYITEM",
        _ => "UNKNOWN_INVENTORY_OPCODE",
    }
}

pub(in crate::world) async fn send_inventory_change_failure(
    stream: &mut WorldPacketSink,
    result: u8,
    item: Option<ObjectGuid>,
    item2: Option<ObjectGuid>,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    send_inventory_change_failure_with_required_level(
        stream,
        result,
        item,
        item2,
        None,
        header_crypto,
    )
    .await
}

pub(in crate::world) async fn send_inventory_change_failure_with_required_level(
    stream: &mut WorldPacketSink,
    result: u8,
    item: Option<ObjectGuid>,
    item2: Option<ObjectGuid>,
    required_level: Option<u32>,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    let body = build_inventory_change_failure_body(result, item, item2, required_level);
    send_packet(
        stream,
        WorldOpcode::SmsgInventoryChangeFailure as u16,
        &body,
        Some(header_crypto),
    )
    .await
}

pub(in crate::world) fn build_inventory_change_failure_body(
    result: u8,
    item: Option<ObjectGuid>,
    item2: Option<ObjectGuid>,
    required_level: Option<u32>,
) -> Vec<u8> {
    SmsgInventoryChangeFailureResponse {
        result,
        required_level: (result == EQUIP_ERR_CANT_EQUIP_LEVEL_I)
            .then_some(required_level.unwrap_or(0)),
        item_guid: item,
        item2_guid: item2,
    }
    .body()
}
