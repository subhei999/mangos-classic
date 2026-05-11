const INVTYPE_BAG: u32 = 18;
const ITEM_CLASS_CONTAINER: u32 = 1;
const ITEM_CLASS_QUIVER: u32 = 11;
const ITEM_SUBCLASS_CONTAINER: u32 = 0;
const ITEM_SUBCLASS_SOUL_CONTAINER: u32 = 1;
const ITEM_SUBCLASS_HERB_CONTAINER: u32 = 2;
const ITEM_SUBCLASS_ENCHANTING_CONTAINER: u32 = 3;
const ITEM_SUBCLASS_ENGINEERING_CONTAINER: u32 = 4;
const ITEM_SUBCLASS_QUIVER: u32 = 2;
const ITEM_SUBCLASS_AMMO_POUCH: u32 = 3;
const BAG_FAMILY_ARROWS: i32 = 1;
const BAG_FAMILY_BULLETS: i32 = 2;
const BAG_FAMILY_SOUL_SHARDS: i32 = 3;
const BAG_FAMILY_HERBS: i32 = 6;
const BAG_FAMILY_ENCHANTING_SUPP: i32 = 7;
const BAG_FAMILY_ENGINEERING_SUPP: i32 = 8;

async fn handle_inventory_swap(
    stream: &mut WorldPacketSink,
    deps: InventoryDeps<'_>,
    opcode: u32,
    body: &[u8],
    session: &mut WorldSessionState,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    let Some(character) = &session.active_character else {
        warn!(
            opcode = inventory_opcode_name(opcode),
            "Ignoring inventory move before character login"
        );
        return Ok(());
    };
    let character_guid = character.guid;
    let Some(move_request) = (match opcode {
        CMSG_AUTOEQUIP_ITEM => {
            let auto_equip =
                InventoryMoveRequest::read_auto_equip(body, deps.world_db_pool, session).await?;
            if auto_equip.is_none() {
                send_auto_equip_failure_if_known(
                    stream,
                    body,
                    deps.world_db_pool,
                    session,
                    header_crypto,
                )
                .await?;
                return Ok(());
            }
            auto_equip
        }
        CMSG_AUTOSTORE_BAG_ITEM => {
            InventoryMoveRequest::read_auto_store_bag(body, deps.world_db_pool, session).await?
        }
        _ => Some(InventoryMoveRequest::read(opcode, body)?),
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

    let equipped_bags = load_equipped_bag_infos(deps.world_db_pool, &session.inventory).await?;
    if !move_request.uses_existing_storage(&equipped_bags) {
        info!(
            opcode = inventory_opcode_name(opcode),
            src_bag = move_request.src_bag,
            src_slot = move_request.src_slot,
            dst_bag = move_request.dst_bag,
            dst_slot = move_request.dst_slot,
            "Rejected inventory move outside equipped bag capacity"
        );
        return send_inventory_change_failure(
            stream,
            EQUIP_ERR_ITEM_DOESNT_GO_TO_SLOT,
            None,
            None,
            header_crypto,
        )
        .await;
    }

    if move_request.src_bag == move_request.dst_bag && move_request.src_slot == move_request.dst_slot {
        return Ok(());
    }

    let Some(src_item) = session
        .inventory
        .iter()
        .find(|item| item.bag == move_request.src_bag as u32 && item.slot == move_request.src_slot)
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

    let dst_item = session
        .inventory
        .iter()
        .find(|item| item.bag == move_request.dst_bag as u32 && item.slot == move_request.dst_slot);

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

    if move_request.dst_bag == INVENTORY_SLOT_BAG_0
        && (move_request.dst_slot < EQUIPMENT_SLOT_END || is_bag_slot(move_request.dst_slot))
    {
        let Some(template) =
            wow_db::get_item_template_query(deps.world_db_pool, src_item.item_template).await?
        else {
            warn!(
                opcode = inventory_opcode_name(opcode),
                guid = character_guid,
                item_template = src_item.item_template,
                "Rejected equip move for missing item template"
            );
            return Ok(());
        };
        let fits_destination = if move_request.dst_slot < EQUIPMENT_SLOT_END {
            item_fits_equipment_slot(template.inventory_type, move_request.dst_slot)
        } else {
            template.container_slots > 0
        };
        if !fits_destination {
            info!(
                opcode = inventory_opcode_name(opcode),
                guid = character_guid,
                item_template = src_item.item_template,
                inventory_type = template.inventory_type,
                dst_slot = move_request.dst_slot,
                "Rejected inventory move for incompatible equipment/bag slot"
            );
            return send_inventory_change_failure(
                stream,
                EQUIP_ERR_ITEM_DOESNT_GO_TO_SLOT,
                Some(ObjectGuid::new(HighGuid::Item, 0, src_item.item)),
                None,
                header_crypto,
            )
            .await;
        }
        if move_request.dst_slot < EQUIPMENT_SLOT_END {
            let skills = wow_db::get_character_skills(deps.character_db_pool, character_guid).await?;
            let equip_result = character_can_equip_item_template(
                character.level,
                character.race,
                character.class,
                &template,
                &skills,
                &session.active_spells,
                &session.character_reputations,
            );
            if equip_result != 0 {
                info!(
                    opcode = inventory_opcode_name(opcode),
                    guid = character_guid,
                    item_template = src_item.item_template,
                    class = character.class,
                    race = character.race,
                    item_class = template.class,
                    item_subclass = template.subclass,
                    "Rejected inventory move due to class/race/proficiency requirements"
                );
                return send_inventory_change_failure_with_required_level(
                    stream,
                    equip_result,
                    Some(ObjectGuid::new(HighGuid::Item, 0, src_item.item)),
                    None,
                    (equip_result == EQUIP_ERR_CANT_EQUIP_LEVEL_I).then_some(template.required_level),
                    header_crypto,
                )
                .await;
            }
        }
    }

    if move_request.src_bag == INVENTORY_SLOT_BAG_0
        && is_bag_slot(move_request.src_slot)
        && !is_bag_slot(move_request.dst_slot)
        && session
            .inventory
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

    let max_stack = if let Some(dst_item) = dst_item.filter(|item| {
        item.item_template == src_item.item_template && item.item != src_item.item
    }) {
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

    session.inventory = wow_db::get_character_inventory_items(deps.character_db_pool, character_guid)
        .await?;
    let changed_equipment_slots = bag0_changed_slots(&move_request)
        .into_iter()
        .filter(|slot| *slot < EQUIPMENT_SLOT_END)
        .collect::<Vec<_>>();
    let mut combat_stats_update_body = None;
    if !changed_equipment_slots.is_empty() {
        if let Some(character) = session.active_character.as_ref() {
            let world_stats = wow_db::get_player_world_stats(
                deps.world_db_pool,
                character.race,
                character.class,
                character.level,
            )
            .await?;
            let equipped_templates =
                load_equipped_item_templates(deps.world_db_pool, &session.inventory).await?;
            let combat_stats = player_combat_stats_for_values(
                character.class,
                character.level,
                &world_stats,
                &equipped_templates,
            );
            combat_stats_update_body =
                Some(build_player_combat_stats_update_body(character_guid, &combat_stats)?);
            let packets = deps
                .shared_world
                .maps
                .update_player_combat_stats(character.position.map_id, character_guid, combat_stats)
                .await?;
            deps.shared_world.sessions.dispatch(packets).await;

            let visible_equipment = visible_equipment_for_inventory(
                session
                    .player_visual
                    .as_ref()
                    .and_then(|visual| visual.equipment_cache.as_deref()),
                &session.inventory,
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
            let blocks =
                build_inventory_move_update_blocks(character_guid, &session.inventory, &move_request)?;
            let body = build_update_object_body(&blocks);
            send_packet(stream, SMSG_UPDATE_OBJECT, &body, Some(&mut *header_crypto)).await?;
            if let Some(body) = combat_stats_update_body {
                send_packet(stream, SMSG_UPDATE_OBJECT, &body, Some(header_crypto)).await?;
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
                blocks.push(build_item_stack_count_update_block(source_item, source_count)?);
            } else {
                blocks.extend(build_inventory_position_update_blocks(
                    character_guid,
                    &session.inventory,
                    move_request.src_bag,
                    move_request.src_slot,
                )?);
            }
            blocks.push(build_item_stack_count_update_block(
                destination_item,
                destination_count,
            )?);
            let body = build_update_object_body(&blocks);
            send_packet(stream, SMSG_UPDATE_OBJECT, &body, Some(&mut *header_crypto)).await?;
            if let Some(body) = combat_stats_update_body {
                send_packet(stream, SMSG_UPDATE_OBJECT, &body, Some(header_crypto)).await?;
            }
            Ok(())
        }
    }
}

async fn send_auto_equip_failure_if_known(
    stream: &mut WorldPacketSink,
    body: &[u8],
    world_db_pool: &MySqlPool,
    session: &WorldSessionState,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    if body.len() < 2 {
        return Ok(());
    }
    let src_bag = normalize_client_bag(body[0]);
    let src_slot = body[1];
    let Some(src_item) = session
        .inventory
        .iter()
        .find(|item| item.bag == src_bag as u32 && item.slot == src_slot)
    else {
        return Ok(());
    };
    let result =
        if wow_db::get_item_template_query(world_db_pool, src_item.item_template)
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

struct InventoryDeps<'a> {
    character_db_pool: &'a MySqlPool,
    world_db_pool: &'a MySqlPool,
    shared_world: SharedWorldDeps<'a>,
}

async fn handle_destroy_item(
    stream: &mut WorldPacketSink,
    deps: QuestMutationDeps<'_>,
    body: &[u8],
    session: &mut WorldSessionState,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    let character_db_pool = deps.character_db_pool;
    let world_db_pool = deps.world_db_pool;
    let Some(character) = &session.active_character else {
        warn!("Ignoring item destroy before character login");
        return Ok(());
    };
    let character_guid = character.guid;
    let request = DestroyItemRequest::read(body)?;

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

    let Some(template) = wow_db::get_item_template_query(world_db_pool, source_item.item_template).await?
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

    session.inventory = wow_db::get_character_inventory_items(character_db_pool, character_guid)
        .await?;
    match destroyed {
        wow_db::InventoryDestroyResult::CountChanged { item, count } => {
            let body = build_item_stack_count_update_body(item, count)?;
            send_packet(stream, SMSG_UPDATE_OBJECT, &body, Some(&mut *header_crypto)).await?;
        }
        wow_db::InventoryDestroyResult::Removed { item } => {
            if request.bag == INVENTORY_SLOT_BAG_0 {
                let body =
                    build_inventory_slots_update_body(character_guid, &session.inventory, &[request.slot])?;
                send_packet(stream, SMSG_UPDATE_OBJECT, &body, Some(&mut *header_crypto)).await?;
            } else {
                let body = build_destroy_object_body(item);
                send_packet(stream, SMSG_DESTROY_OBJECT, &body, Some(&mut *header_crypto)).await?;
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

async fn handle_split_item(
    stream: &mut WorldPacketSink,
    character_db_pool: &MySqlPool,
    body: &[u8],
    session: &mut WorldSessionState,
    header_crypto: &mut HeaderCrypto,
) -> anyhow::Result<()> {
    let Some(character) = &session.active_character else {
        warn!("Ignoring item split before character login");
        return Ok(());
    };
    let character_guid = character.guid;
    let request = SplitItemRequest::read(body)?;
    if !request.is_supported_split() || request.src_bag == request.dst_bag && request.src_slot == request.dst_slot {
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

    session.inventory = wow_db::get_character_inventory_items(character_db_pool, character_guid)
        .await?;
    let mut blocks = vec![build_item_stack_count_update_block(
        split.source_item,
        split.source_count,
    )?];
    if let Some(new_item) = session.inventory.iter().find(|item| item.item == split.new_item) {
        let owner_guid = ObjectGuid::new(HighGuid::Player, 0, character_guid);
        let contained_guid = item_contained_guid(owner_guid, &session.inventory, new_item);
        blocks.push(build_item_create_update_block(
            owner_guid,
            contained_guid,
            new_item,
            None,
        )?);
        blocks.extend(build_inventory_position_update_blocks(
            character_guid,
            &session.inventory,
            new_item.bag as u8,
            new_item.slot,
        )?);
    }
    let body = build_update_object_body(&blocks);
    send_packet(stream, SMSG_UPDATE_OBJECT, &body, Some(header_crypto)).await
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct InventoryMoveRequest {
    src_bag: u8,
    src_slot: u8,
    dst_bag: u8,
    dst_slot: u8,
}

impl InventoryMoveRequest {
    fn read(opcode: u32, body: &[u8]) -> anyhow::Result<Self> {
        match opcode {
            CMSG_SWAP_INV_ITEM => {
                if body.len() < 2 {
                    anyhow::bail!("CMSG_SWAP_INV_ITEM payload too short: {} bytes", body.len());
                }
                Ok(Self {
                    src_bag: INVENTORY_SLOT_BAG_0,
                    src_slot: body[0],
                    dst_bag: INVENTORY_SLOT_BAG_0,
                    dst_slot: body[1],
                })
            }
            CMSG_SWAP_ITEM => {
                if body.len() < 4 {
                    anyhow::bail!("CMSG_SWAP_ITEM payload too short: {} bytes", body.len());
                }
                Ok(Self {
                    dst_bag: normalize_client_bag(body[0]),
                    dst_slot: body[1],
                    src_bag: normalize_client_bag(body[2]),
                    src_slot: body[3],
                })
            }
            _ => anyhow::bail!("unsupported inventory opcode 0x{opcode:04X}"),
        }
    }

    async fn read_auto_equip(
        body: &[u8],
        world_db_pool: &MySqlPool,
        session: &WorldSessionState,
    ) -> anyhow::Result<Option<Self>> {
        if body.len() < 2 {
            anyhow::bail!("CMSG_AUTOEQUIP_ITEM payload too short: {} bytes", body.len());
        }
        let src_bag = normalize_client_bag(body[0]);
        let src_slot = body[1];
        let Some(src_item) = session
            .inventory
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
        let Some(dst_slot) = preferred_equipment_slot_for_inventory(&template, &session.inventory)
        else {
            return Ok(None);
        };
        Ok(Some(Self {
            src_bag,
            src_slot,
            dst_bag: INVENTORY_SLOT_BAG_0,
            dst_slot,
        }))
    }

    async fn read_auto_store_bag(
        body: &[u8],
        world_db_pool: &MySqlPool,
        session: &WorldSessionState,
    ) -> anyhow::Result<Option<Self>> {
        if body.len() < 3 {
            anyhow::bail!(
                "CMSG_AUTOSTORE_BAG_ITEM payload too short: {} bytes",
                body.len()
            );
        }
        let src_bag = normalize_client_bag(body[0]);
        let src_slot = body[1];
        let dst_bag = normalize_client_bag(body[2]);
        let Some(src_item) = session
            .inventory
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
        let equipped_bags = load_equipped_bag_infos(world_db_pool, &session.inventory).await?;
        let Some((dst_bag, dst_slot)) =
            first_autostore_destination(&session.inventory, src_item, &template, &equipped_bags, dst_bag)
        else {
            return Ok(None);
        };
        Ok(Some(Self {
            src_bag,
            src_slot,
            dst_bag,
            dst_slot,
        }))
    }

    fn is_supported_inventory_move(&self) -> bool {
        is_supported_move_position(self.src_bag, self.src_slot)
            && is_supported_move_position(self.dst_bag, self.dst_slot)
    }

    fn uses_existing_storage(&self, equipped_bags: &[EquippedBagInfo]) -> bool {
        move_position_exists(self.src_bag, self.src_slot, equipped_bags)
            && move_position_exists(self.dst_bag, self.dst_slot, equipped_bags)
    }

    fn moves_equipped_bag_into_itself(&self) -> bool {
        (self.src_bag == INVENTORY_SLOT_BAG_0
            && is_bag_slot(self.src_slot)
            && self.dst_bag == self.src_slot)
            || (self.dst_bag == INVENTORY_SLOT_BAG_0
                && is_bag_slot(self.dst_slot)
                && self.src_bag == self.dst_slot)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct DestroyItemRequest {
    bag: u8,
    slot: u8,
    count: u8,
}

impl DestroyItemRequest {
    fn read(body: &[u8]) -> anyhow::Result<Self> {
        if body.len() < 6 {
            anyhow::bail!("CMSG_DESTROYITEM payload too short: {} bytes", body.len());
        }
        Ok(Self {
            bag: normalize_client_bag(body[0]),
            slot: body[1],
            count: body[2],
        })
    }

    fn is_supported_destroy(&self) -> bool {
        is_supported_storage_position(self.bag, self.slot)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct SplitItemRequest {
    src_bag: u8,
    src_slot: u8,
    dst_bag: u8,
    dst_slot: u8,
    count: u8,
}

impl SplitItemRequest {
    fn read(body: &[u8]) -> anyhow::Result<Self> {
        if body.len() < 5 {
            anyhow::bail!("CMSG_SPLIT_ITEM payload too short: {} bytes", body.len());
        }
        Ok(Self {
            src_bag: normalize_client_bag(body[0]),
            src_slot: body[1],
            dst_bag: normalize_client_bag(body[2]),
            dst_slot: body[3],
            count: body[4],
        })
    }

    fn is_supported_split(&self) -> bool {
        self.count != 0
            && is_supported_storage_position(self.src_bag, self.src_slot)
            && is_supported_storage_position(self.dst_bag, self.dst_slot)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct BuyItemRequest {
    vendor_guid: ObjectGuid,
    item: u32,
    count: u8,
}

impl BuyItemRequest {
    fn read(body: &[u8]) -> anyhow::Result<Self> {
        if body.len() < 14 {
            anyhow::bail!("CMSG_BUY_ITEM payload too short: {} bytes", body.len());
        }
        Ok(Self {
            vendor_guid: ObjectGuid::from_raw(u64::from_le_bytes(body[0..8].try_into()?)),
            item: u32::from_le_bytes(body[8..12].try_into()?),
            count: body[12],
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct SellItemRequest {
    vendor_guid: ObjectGuid,
    item_guid: ObjectGuid,
    count: u8,
}

impl SellItemRequest {
    fn read(body: &[u8]) -> anyhow::Result<Self> {
        if body.len() < 17 {
            anyhow::bail!("CMSG_SELL_ITEM payload too short: {} bytes", body.len());
        }
        Ok(Self {
            vendor_guid: ObjectGuid::from_raw(u64::from_le_bytes(body[0..8].try_into()?)),
            item_guid: ObjectGuid::from_raw(u64::from_le_bytes(body[8..16].try_into()?)),
            count: body[16],
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct BuybackItemRequest {
    vendor_guid: ObjectGuid,
    slot: u8,
}

impl BuybackItemRequest {
    fn read(body: &[u8]) -> anyhow::Result<Self> {
        if body.len() < 12 {
            anyhow::bail!("CMSG_BUYBACK_ITEM payload too short: {} bytes", body.len());
        }
        Ok(Self {
            vendor_guid: ObjectGuid::from_raw(u64::from_le_bytes(body[0..8].try_into()?)),
            slot: u32::from_le_bytes(body[8..12].try_into()?) as u8,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct EquippedBagInfo {
    slot: u8,
    container_slots: u8,
    class: u32,
    subclass: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct StoreSlot {
    bag: u8,
    slot: u8,
    count: u32,
    existing_item: Option<u32>,
}

async fn load_equipped_bag_infos(
    world_db_pool: &MySqlPool,
    inventory: &[CharacterInventoryItem],
) -> anyhow::Result<Vec<EquippedBagInfo>> {
    let mut bags = Vec::new();
    for slot in INVENTORY_SLOT_BAG_START..INVENTORY_SLOT_BAG_END {
        let Some(item) = inventory
            .iter()
            .find(|item| item.bag == INVENTORY_SLOT_BAG_0 as u32 && item.slot == slot)
        else {
            continue;
        };
        let Some(template) = wow_db::get_item_template_query(world_db_pool, item.item_template).await?
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

fn preferred_equipment_slot_for_inventory(
    template: &ItemTemplateQuery,
    inventory: &[CharacterInventoryItem],
) -> Option<u8> {
    if template.inventory_type == INVTYPE_BAG && template.container_slots > 0 {
        return (INVENTORY_SLOT_BAG_START..INVENTORY_SLOT_BAG_END).find(|slot| {
            inventory
                .iter()
                .all(|item| item.bag != INVENTORY_SLOT_BAG_0 as u32 || item.slot != *slot)
        });
    }
    preferred_equipment_slot(template.inventory_type)
}

fn move_position_exists(bag: u8, slot: u8, equipped_bags: &[EquippedBagInfo]) -> bool {
    if bag == INVENTORY_SLOT_BAG_0 {
        return slot < EQUIPMENT_SLOT_END || is_bag_slot(slot) || is_backpack_item_slot(slot);
    }
    equipped_bags
        .iter()
        .any(|equipped| equipped.slot == bag && slot < equipped.container_slots)
}

fn storage_position_exists(bag: u8, slot: u8, equipped_bags: &[EquippedBagInfo]) -> bool {
    if bag == INVENTORY_SLOT_BAG_0 {
        return is_backpack_item_slot(slot);
    }
    equipped_bags
        .iter()
        .any(|equipped| equipped.slot == bag && slot < equipped.container_slots)
}

fn item_can_go_into_bag(item: &ItemTemplateQuery, bag: &EquippedBagInfo) -> bool {
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

fn bag_accepts_item(bag: u8, template: &ItemTemplateQuery, equipped_bags: &[EquippedBagInfo]) -> bool {
    if bag == INVENTORY_SLOT_BAG_0 {
        return true;
    }
    equipped_bags
        .iter()
        .find(|equipped| equipped.slot == bag)
        .is_some_and(|equipped| item_can_go_into_bag(template, equipped))
}

fn is_normal_container_bag(bag: &EquippedBagInfo) -> bool {
    bag.class == ITEM_CLASS_CONTAINER && bag.subclass == ITEM_SUBCLASS_CONTAINER
}

fn storage_slot_range(bag: u8, equipped_bags: &[EquippedBagInfo]) -> Option<(u8, u8)> {
    if bag == INVENTORY_SLOT_BAG_0 {
        return Some((INVENTORY_SLOT_ITEM_START, INVENTORY_SLOT_ITEM_END));
    }
    equipped_bags
        .iter()
        .find(|equipped| equipped.slot == bag)
        .map(|equipped| (0, equipped.container_slots))
}

fn inventory_store_bag_order(
    template: &ItemTemplateQuery,
    equipped_bags: &[EquippedBagInfo],
    specific_bag: Option<u8>,
) -> Vec<u8> {
    if let Some(bag) = specific_bag {
        return if bag_accepts_item(bag, template, equipped_bags) {
            vec![bag]
        } else {
            Vec::new()
        };
    }

    let mut bags = vec![INVENTORY_SLOT_BAG_0];
    if template.bag_family != 0 {
        bags.extend(
            equipped_bags
                .iter()
                .filter(|bag| !is_normal_container_bag(bag) && item_can_go_into_bag(template, bag))
                .map(|bag| bag.slot),
        );
    }
    bags.extend(
        equipped_bags
            .iter()
            .filter(|bag| is_normal_container_bag(bag) && item_can_go_into_bag(template, bag))
            .map(|bag| bag.slot),
    );
    bags
}

fn plan_store_item(
    inventory: &[CharacterInventoryItem],
    template: &ItemTemplateQuery,
    count: u32,
    equipped_bags: &[EquippedBagInfo],
    specific_bag: Option<u8>,
    skip_item: Option<u32>,
) -> Option<Vec<StoreSlot>> {
    if count == 0 {
        return Some(Vec::new());
    }

    let max_stack = template.stackable.max(1);
    let mut remaining = count;
    let mut dest = Vec::new();
    let bags = inventory_store_bag_order(template, equipped_bags, specific_bag);

    if max_stack > 1 {
        for bag in &bags {
            for item in inventory
                .iter()
                .filter(|item| {
                    item.bag == *bag as u32
                        && item.item_template == template.entry
                        && Some(item.item) != skip_item
                        && item.count < max_stack
                        && storage_position_exists(*bag, item.slot, equipped_bags)
                })
            {
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
        let Some((start, end)) = storage_slot_range(*bag, equipped_bags) else {
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

fn first_autostore_destination(
    inventory: &[CharacterInventoryItem],
    source: &CharacterInventoryItem,
    template: &ItemTemplateQuery,
    equipped_bags: &[EquippedBagInfo],
    dst_bag: u8,
) -> Option<(u8, u8)> {
    plan_store_item(
        inventory,
        template,
        source.count,
        equipped_bags,
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

fn normalize_client_bag(bag: u8) -> u8 {
    if bag == CLIENT_INVENTORY_SLOT_BAG_0 {
        INVENTORY_SLOT_BAG_0
    } else {
        bag
    }
}

fn is_backpack_item_slot(slot: u8) -> bool {
    (INVENTORY_SLOT_ITEM_START..INVENTORY_SLOT_ITEM_END).contains(&slot)
}

fn is_bag_slot(slot: u8) -> bool {
    (INVENTORY_SLOT_BAG_START..INVENTORY_SLOT_BAG_END).contains(&slot)
}

fn is_supported_storage_position(bag: u8, slot: u8) -> bool {
    (bag == INVENTORY_SLOT_BAG_0 && slot < INVENTORY_SLOT_ITEM_END)
        || (is_bag_slot(bag) && slot < MAX_BAG_SIZE)
}

fn is_supported_move_position(bag: u8, slot: u8) -> bool {
    (bag == INVENTORY_SLOT_BAG_0
        && (slot < EQUIPMENT_SLOT_END || is_bag_slot(slot) || is_backpack_item_slot(slot)))
        || (is_bag_slot(bag) && slot < MAX_BAG_SIZE)
}

fn bag0_changed_slots(request: &InventoryMoveRequest) -> Vec<u8> {
    let mut slots = Vec::with_capacity(2);
    if request.src_bag == INVENTORY_SLOT_BAG_0 {
        slots.push(request.src_slot);
    }
    if request.dst_bag == INVENTORY_SLOT_BAG_0 && request.dst_slot != request.src_slot {
        slots.push(request.dst_slot);
    }
    slots
}

fn build_inventory_move_update_blocks(
    character_guid: u32,
    inventory: &[CharacterInventoryItem],
    request: &InventoryMoveRequest,
) -> anyhow::Result<Vec<Vec<u8>>> {
    let mut blocks = Vec::new();
    let bag0_slots = bag0_changed_slots(request);
    if !bag0_slots.is_empty() {
        blocks.push(build_inventory_slots_update_block(
            character_guid,
            inventory,
            &bag0_slots,
        )?);
    }
    blocks.extend(build_container_position_update_blocks(
        character_guid,
        inventory,
        request.src_bag,
        request.src_slot,
    )?);
    if request.dst_bag != request.src_bag || request.dst_slot != request.src_slot {
        blocks.extend(build_container_position_update_blocks(
            character_guid,
            inventory,
            request.dst_bag,
            request.dst_slot,
        )?);
    }

    Ok(blocks)
}

fn build_inventory_position_update_blocks(
    character_guid: u32,
    inventory: &[CharacterInventoryItem],
    bag: u8,
    slot: u8,
) -> anyhow::Result<Vec<Vec<u8>>> {
    if bag == INVENTORY_SLOT_BAG_0 {
        return Ok(vec![build_inventory_slots_update_block(
            character_guid,
            inventory,
            &[slot],
        )?]);
    }
    build_container_position_update_blocks(character_guid, inventory, bag, slot)
}

fn build_container_position_update_blocks(
    character_guid: u32,
    inventory: &[CharacterInventoryItem],
    bag: u8,
    slot: u8,
) -> anyhow::Result<Vec<Vec<u8>>> {
    if !is_bag_slot(bag) {
        return Ok(Vec::new());
    }
    let mut blocks = Vec::new();
    if let Some(block) = build_container_slot_update_block(inventory, bag, slot)? {
        blocks.push(block);
    }
    if let Some(item) = inventory
        .iter()
        .find(|item| item.bag == bag as u32 && item.slot == slot)
    {
        let owner_guid = ObjectGuid::new(HighGuid::Player, 0, character_guid);
        blocks.push(build_item_contained_update_block(owner_guid, inventory, item)?);
    }
    Ok(blocks)
}

fn first_empty_backpack_slot(inventory: &[CharacterInventoryItem]) -> Option<u8> {
    (INVENTORY_SLOT_ITEM_START..INVENTORY_SLOT_ITEM_END).find(|slot| {
        inventory
            .iter()
            .all(|item| item.bag != INVENTORY_SLOT_BAG_0 as u32 || item.slot != *slot)
    })
}

fn build_rust_guide_vendor_inventory() -> Vec<u8> {
    build_vendor_inventory_body(
        rust_guide_guid(),
        &[
            VendorListItem {
                item: RUST_VENDOR_BAG_ITEM,
                display: RUST_VENDOR_BAG_DISPLAY,
                max_count: 0,
                price: 0,
                durability: 0,
                buy_count: 1,
            },
        ],
    )
}

#[derive(Debug, Clone, Copy)]
struct VendorListItem {
    item: u32,
    display: u32,
    max_count: u32,
    price: u32,
    durability: u32,
    buy_count: u32,
}

impl From<&wow_db::VendorItemQuery> for VendorListItem {
    fn from(item: &wow_db::VendorItemQuery) -> Self {
        Self {
            item: item.item,
            display: item.display_id,
            max_count: item.max_count,
            price: item.buy_price,
            durability: item.max_durability,
            buy_count: item.buy_count,
        }
    }
}

fn build_vendor_inventory_body(vendor_guid: ObjectGuid, items: &[VendorListItem]) -> Vec<u8> {
    if items.is_empty() {
        let mut body = Vec::with_capacity(10);
        body.extend_from_slice(&vendor_guid.raw().to_le_bytes());
        body.push(0);
        body.push(0);
        return body;
    }

    let mut body = Vec::with_capacity(8 + 1 + items.len().min(128) * 28);
    body.extend_from_slice(&vendor_guid.raw().to_le_bytes());
    body.push(items.len().min(128) as u8);
    for (index, item) in items.iter().take(128).enumerate() {
        let available_count = if item.max_count == 0 {
            u32::MAX
        } else {
            item.max_count
        };
        write_vendor_item(
            &mut body,
            (index + 1) as u32,
            available_count,
            *item,
        );
    }
    body
}

fn write_vendor_item(body: &mut Vec<u8>, slot: u32, available_count: u32, item: VendorListItem) {
    body.extend_from_slice(&slot.to_le_bytes());
    body.extend_from_slice(&item.item.to_le_bytes());
    body.extend_from_slice(&item.display.to_le_bytes());
    body.extend_from_slice(&available_count.to_le_bytes());
    body.extend_from_slice(&item.price.to_le_bytes());
    body.extend_from_slice(&item.durability.to_le_bytes());
    body.extend_from_slice(&item.buy_count.to_le_bytes());
}

fn build_buy_item_body(vendor_guid: ObjectGuid, vendor_slot: u32, count: u8) -> Vec<u8> {
    let mut body = Vec::with_capacity(20);
    body.extend_from_slice(&vendor_guid.raw().to_le_bytes());
    body.extend_from_slice(&vendor_slot.to_le_bytes());
    body.extend_from_slice(&u32::MAX.to_le_bytes());
    body.extend_from_slice(&(count as u32).to_le_bytes());
    body
}

fn build_buy_failed_body(vendor_guid: ObjectGuid, item: u32, result: u8) -> Vec<u8> {
    let mut body = Vec::with_capacity(13);
    body.extend_from_slice(&vendor_guid.raw().to_le_bytes());
    body.extend_from_slice(&item.to_le_bytes());
    body.push(result);
    body
}

fn build_sell_item_error_body(vendor_guid: ObjectGuid, item_guid: ObjectGuid, result: u8) -> Vec<u8> {
    let mut body = Vec::with_capacity(17);
    body.extend_from_slice(&vendor_guid.raw().to_le_bytes());
    body.extend_from_slice(&item_guid.raw().to_le_bytes());
    body.push(result);
    body
}

fn rust_guide_vendor_slot(item: u32) -> Option<u32> {
    match item {
        RUST_VENDOR_BAG_ITEM => Some(1),
        _ => None,
    }
}

fn preferred_equipment_slot(inventory_type: u32) -> Option<u8> {
    match inventory_type {
        1 => Some(0),   // INVTYPE_HEAD
        2 => Some(1),   // INVTYPE_NECK
        3 => Some(2),   // INVTYPE_SHOULDERS
        4 => Some(3),   // INVTYPE_BODY
        5 | 20 => Some(4), // INVTYPE_CHEST / ROBE
        6 => Some(5),   // INVTYPE_WAIST
        7 => Some(6),   // INVTYPE_LEGS
        8 => Some(7),   // INVTYPE_FEET
        9 => Some(8),   // INVTYPE_WRISTS
        10 => Some(9),  // INVTYPE_HANDS
        11 => Some(10), // INVTYPE_FINGER
        12 => Some(12), // INVTYPE_TRINKET
        13 | 17 | 21 => Some(15), // one-hand/two-hand/main-hand weapon
        14 | 22 | 23 => Some(16), // shield/offhand/held-in-offhand
        15 | 25 | 26 => Some(17), // ranged/thrown/ranged right
        18 => Some(19), // INVTYPE_BAG
        16 => Some(14), // INVTYPE_CLOAK
        19 => Some(18), // INVTYPE_TABARD
        _ => None,
    }
}

fn item_fits_equipment_slot(inventory_type: u32, slot: u8) -> bool {
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

fn character_can_equip_item_template(
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
    let use_result =
        character_can_use_item_template(level, race, class, template, skills, active_spells, reputations);
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

fn character_can_use_item_template(
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

fn reputation_rank_from_standing(standing: i32) -> u32 {
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

fn item_proficiency_skill(template: &ItemTemplateQuery) -> Option<u32> {
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
            0 => Some(44),  // Axes
            1 => Some(172), // Two-Handed Axes
            2 => Some(45),  // Bows
            3 => Some(46),  // Guns
            4 => Some(54),  // Maces
            5 => Some(160), // Two-Handed Maces
            6 => Some(229), // Polearms
            7 => Some(43),  // Swords
            8 => Some(55),  // Two-Handed Swords
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

fn inventory_opcode_name(opcode: u32) -> &'static str {
    match opcode {
        CMSG_AUTOEQUIP_ITEM => "CMSG_AUTOEQUIP_ITEM",
        CMSG_AUTOSTORE_BAG_ITEM => "CMSG_AUTOSTORE_BAG_ITEM",
        CMSG_SWAP_INV_ITEM => "CMSG_SWAP_INV_ITEM",
        CMSG_SWAP_ITEM => "CMSG_SWAP_ITEM",
        CMSG_SPLIT_ITEM => "CMSG_SPLIT_ITEM",
        CMSG_DESTROYITEM => "CMSG_DESTROYITEM",
        _ => "UNKNOWN_INVENTORY_OPCODE",
    }
}

async fn send_inventory_change_failure(
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

async fn send_inventory_change_failure_with_required_level(
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
        SMSG_INVENTORY_CHANGE_FAILURE,
        &body,
        Some(header_crypto),
    )
    .await
}

fn build_inventory_change_failure_body(
    result: u8,
    item: Option<ObjectGuid>,
    item2: Option<ObjectGuid>,
    required_level: Option<u32>,
) -> Vec<u8> {
    let mut body = Vec::with_capacity(if result == EQUIP_ERR_CANT_EQUIP_LEVEL_I {
        22
    } else {
        18
    });
    body.push(result);
    if result == EQUIP_ERR_CANT_EQUIP_LEVEL_I {
        body.extend_from_slice(&required_level.unwrap_or(0).to_le_bytes());
    }
    body.extend_from_slice(&item.map(|guid| guid.raw()).unwrap_or(0).to_le_bytes());
    body.extend_from_slice(&item2.map(|guid| guid.raw()).unwrap_or(0).to_le_bytes());
    body.push(0);
    body
}
