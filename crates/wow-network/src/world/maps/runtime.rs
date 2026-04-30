#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct GridCoord {
    x: u32,
    y: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct CellCoord {
    x: u32,
    y: u32,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
struct PlayerRuntime {
    guid: u32,
    account_id: u32,
    session_id: SessionId,
    position: WorldPosition,
    cell: CellCoord,
    visible_objects: HashSet<ObjectGuid>,
    visual: PlayerVisualState,
    flags: u32,
    level: u8,
    race: u8,
    class: u8,
    gender: u8,
    health: u32,
    max_health: u32,
    power1: u32,
    max_power1: u32,
    power2: u32,
    player_bytes: u32,
    player_bytes2: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum GridState {
    Loaded,
    Active,
}

#[derive(Debug, Default)]
#[allow(dead_code)]
struct CellRuntime {
    players: HashSet<u32>,
    creatures: HashSet<u32>,
    corpses: HashSet<u64>,
}

#[derive(Debug)]
struct GridRuntime {
    state: GridState,
    cells: HashMap<CellCoord, CellRuntime>,
    active_player_count: u32,
    last_touched: Instant,
}

impl Default for GridRuntime {
    fn default() -> Self {
        Self {
            state: GridState::Loaded,
            cells: HashMap::new(),
            active_player_count: 0,
            last_touched: Instant::now(),
        }
    }
}

#[derive(Debug)]
#[allow(dead_code)]
struct MapRuntime {
    map_id: u32,
    instance_id: u32,
    grids: HashMap<GridCoord, GridRuntime>,
    players: HashMap<u32, PlayerRuntime>,
    creatures: HashMap<u64, DbCreatureRuntime>,
    active_creature_combats: HashMap<u64, CreatureCombatState>,
    corpses: HashMap<u64, PlayerCorpseRuntime>,
}

impl MapRuntime {
    fn new(map_id: u32, instance_id: u32) -> Self {
        Self {
            map_id,
            instance_id,
            grids: HashMap::new(),
            players: HashMap::new(),
            creatures: HashMap::new(),
            active_creature_combats: HashMap::new(),
            corpses: HashMap::new(),
        }
    }

    fn add_player(&mut self, player: PlayerRuntime) -> anyhow::Result<Vec<(SessionId, OutboundWorldPacket)>> {
        let mut player = player;
        let player_guid = player.guid;
        let player_grid = grid_coord_for_position(player.position);
        let player_cell = cell_coord_for_position(player.position);
        player.cell = player_cell;
        let player_object = ObjectGuid::new(HighGuid::Player, 0, player_guid);
        let new_player_packet = OutboundWorldPacket {
            opcode: SMSG_UPDATE_OBJECT,
            body: build_update_object_body(&[build_other_player_create_block(&player)?]),
        };
        let mut packets = Vec::new();
        let mut visible_others = Vec::new();

        for other_guid in self.nearby_player_guids(player.position, PLAYER_VISIBILITY_RADIUS_YARDS, Some(player_guid)) {
            let Some(other) = self.players.get(&other_guid) else {
                continue;
            };

            visible_others.push(other.guid);
            packets.push((
                player.session_id,
                OutboundWorldPacket {
                    opcode: SMSG_UPDATE_OBJECT,
                    body: build_update_object_body(&[build_other_player_create_block(other)?]),
                },
            ));
            packets.push((other.session_id, new_player_packet.clone()));
        }
        for other_guid in &visible_others {
            player
                .visible_objects
                .insert(ObjectGuid::new(HighGuid::Player, 0, *other_guid));
            if let Some(other) = self.players.get_mut(other_guid) {
                other.visible_objects.insert(player_object);
            }
        }

        let grid = self.grids.entry(player_grid).or_default();
        grid.state = GridState::Active;
        grid.active_player_count = grid.active_player_count.saturating_add(1);
        grid.last_touched = Instant::now();
        grid.cells.entry(player_cell).or_default().players.insert(player_guid);
        self.players.insert(player_guid, player);

        Ok(packets)
    }

    fn update_player_position(
        &mut self,
        character_guid: u32,
        opcode: u16,
        movement: &MovementInfo,
    ) -> anyhow::Result<Vec<(SessionId, OutboundWorldPacket)>> {
        let Some(current_player) = self.players.get(&character_guid).cloned() else {
            return Ok(Vec::new());
        };
        let player_object = ObjectGuid::new(HighGuid::Player, 0, character_guid);
        let old_cell = current_player.cell;
        let old_grid = grid_coord_for_position(current_player.position);
        let new_cell = cell_coord_for_position(movement.position);
        let new_grid = grid_coord_for_position(movement.position);

        let old_visible = current_player
            .visible_objects
            .iter()
            .filter_map(|guid| {
                if guid.is_player() {
                    Some(guid.counter())
                } else {
                    None
                }
            })
            .collect::<HashSet<_>>();
        let new_visible = self
            .nearby_player_guids(
                movement.position,
                PLAYER_VISIBILITY_RADIUS_YARDS,
                Some(character_guid),
            )
            .into_iter()
            .collect::<HashSet<_>>();

        let mut packets = Vec::new();
        for other_guid in old_visible.difference(&new_visible).copied().collect::<Vec<_>>() {
            let Some(other) = self.players.get_mut(&other_guid) else {
                continue;
            };
            other.visible_objects.remove(&player_object);
            packets.push((
                current_player.session_id,
                OutboundWorldPacket {
                    opcode: SMSG_DESTROY_OBJECT,
                    body: build_destroy_guid_body(ObjectGuid::new(HighGuid::Player, 0, other_guid)),
                },
            ));
            packets.push((
                other.session_id,
                OutboundWorldPacket {
                    opcode: SMSG_DESTROY_OBJECT,
                    body: build_destroy_guid_body(player_object),
                },
            ));
        }

        let entering = new_visible.difference(&old_visible).copied().collect::<Vec<_>>();
        let mut entering_for_mover = Vec::new();
        let moving_player_create = {
            let mut moved_player = current_player.clone();
            moved_player.position = movement.position;
            moved_player.cell = new_cell;
            build_other_player_create_block(&moved_player)?
        };
        for other_guid in entering {
            let Some(other) = self.players.get_mut(&other_guid) else {
                continue;
            };
            other.visible_objects.insert(player_object);
            entering_for_mover.push(other_guid);
            packets.push((
                other.session_id,
                OutboundWorldPacket {
                    opcode: SMSG_UPDATE_OBJECT,
                    body: build_update_object_body(std::slice::from_ref(&moving_player_create)),
                },
            ));
        }
        for other_guid in &entering_for_mover {
            if let Some(other) = self.players.get(other_guid) {
                packets.push((
                    current_player.session_id,
                    OutboundWorldPacket {
                        opcode: SMSG_UPDATE_OBJECT,
                        body: build_update_object_body(&[build_other_player_create_block(other)?]),
                    },
                ));
            }
        }

        let movement_packet = OutboundWorldPacket {
            opcode,
            body: build_player_movement_broadcast_body(character_guid, movement)?,
        };
        for other_guid in &new_visible {
            let Some(other) = self.players.get(other_guid) else {
                continue;
            };
            packets.push((other.session_id, movement_packet.clone()));
        }

        if old_grid != new_grid || old_cell != new_cell {
            if let Some(grid) = self.grids.get_mut(&old_grid) {
                if let Some(cell) = grid.cells.get_mut(&old_cell) {
                    cell.players.remove(&character_guid);
                }
                grid.last_touched = Instant::now();
            }
            let grid = self.grids.entry(new_grid).or_default();
            grid.state = GridState::Active;
            grid.last_touched = Instant::now();
            grid.cells
                .entry(new_cell)
                .or_default()
                .players
                .insert(character_guid);
        }

        if let Some(player) = self.players.get_mut(&character_guid) {
            player.position = movement.position;
            player.cell = new_cell;
            player.visible_objects = new_visible
                .iter()
                .map(|guid| ObjectGuid::new(HighGuid::Player, 0, *guid))
                .collect();
        }

        Ok(packets)
    }

    fn remove_player(&mut self, character_guid: u32) -> Vec<(SessionId, OutboundWorldPacket)> {
        let Some(player) = self.players.remove(&character_guid) else {
            return Vec::new();
        };

        if let Some(grid) = self.grids.get_mut(&grid_coord_for_position(player.position)) {
            grid.active_player_count = grid.active_player_count.saturating_sub(1);
            grid.last_touched = Instant::now();
            if let Some(cell) = grid.cells.get_mut(&player.cell) {
                cell.players.remove(&character_guid);
            }
        }

        let destroy = OutboundWorldPacket {
            opcode: SMSG_DESTROY_OBJECT,
            body: build_destroy_guid_body(ObjectGuid::new(HighGuid::Player, 0, character_guid)),
        };
        self.nearby_player_guids(player.position, PLAYER_VISIBILITY_RADIUS_YARDS, Some(character_guid))
            .into_iter()
            .filter_map(|other_guid| {
                self.players
                    .get(&other_guid)
                    .map(|other| (other.session_id, destroy.clone()))
            })
            .collect()
    }

    fn broadcast_nearby_player_packet(
        &self,
        sender_guid: u32,
        radius: f32,
        packet: OutboundWorldPacket,
    ) -> Vec<(SessionId, OutboundWorldPacket)> {
        let Some(sender) = self.players.get(&sender_guid) else {
            return Vec::new();
        };
        self.nearby_player_guids(sender.position, radius, Some(sender_guid))
            .into_iter()
            .filter_map(|other_guid| {
                self.players
                    .get(&other_guid)
                    .map(|other| (other.session_id, packet.clone()))
            })
            .collect()
    }

    fn share_db_creature_snapshots(
        &mut self,
        creatures: Vec<DbCreatureRuntime>,
    ) -> Vec<DbCreatureRuntime> {
        creatures
            .into_iter()
            .map(|creature| {
                let guid = creature.guid().raw();
                let shared = self.creatures.entry(guid).or_insert_with(|| {
                    let cell = cell_coord_for_position(creature.current_position);
                    let grid = grid_coord_for_position(creature.current_position);
                    self.grids
                        .entry(grid)
                        .or_default()
                        .cells
                        .entry(cell)
                        .or_default()
                        .creatures
                        .insert(creature.guid().counter());
                    creature
                });
                shared.clone()
            })
            .collect()
    }

    fn db_creature_snapshots(&self, creature_guids: &[u64]) -> Vec<DbCreatureRuntime> {
        creature_guids
            .iter()
            .filter_map(|guid| self.creatures.get(guid).cloned())
            .collect()
    }

    #[allow(dead_code)]
    fn update_db_creature_snapshot(&mut self, creature: DbCreatureRuntime) {
        let guid = creature.guid().raw();
        let new_grid = grid_coord_for_position(creature.current_position);
        let new_cell = cell_coord_for_position(creature.current_position);
        if let Some(previous) = self.creatures.get(&guid) {
            let previous_grid = grid_coord_for_position(previous.current_position);
            let previous_cell = cell_coord_for_position(previous.current_position);
            if previous_grid != new_grid || previous_cell != new_cell {
                if let Some(grid) = self.grids.get_mut(&previous_grid) {
                    if let Some(cell) = grid.cells.get_mut(&previous_cell) {
                        cell.creatures.remove(&creature.guid().counter());
                    }
                }
            }
        }
        self.grids
            .entry(new_grid)
            .or_default()
            .cells
            .entry(new_cell)
            .or_default()
            .creatures
            .insert(creature.guid().counter());
        self.creatures.insert(guid, creature);
    }

    fn update_db_creature_snapshot_and_broadcast(
        &mut self,
        creature: DbCreatureRuntime,
        exclude_character_guid: Option<u32>,
        packet: OutboundWorldPacket,
    ) -> Vec<(SessionId, OutboundWorldPacket)> {
        let position = creature.current_position;
        self.update_db_creature_snapshot(creature);
        self.nearby_player_guids(position, CREATURE_SPAWN_RADIUS_YARDS, exclude_character_guid)
            .into_iter()
            .filter_map(|player_guid| {
                self.players
                    .get(&player_guid)
                    .map(|player| (player.session_id, packet.clone()))
            })
            .collect()
    }

    fn open_db_creature_loot(
        &mut self,
        creature_guid: u64,
        loot_item: Option<DbCreatureLootRuntime>,
    ) -> Option<DbCreatureRuntime> {
        let creature = self.creatures.get_mut(&creature_guid)?;
        if !creature.lootable {
            return None;
        }
        if creature.loot_item.is_none() {
            creature.loot_item = loot_item;
        }
        creature.looting = true;
        Some(creature.clone())
    }

    fn take_db_creature_loot_money(
        &mut self,
        creature_guid: u64,
    ) -> Option<(u32, DbCreatureRuntime)> {
        let creature = self.creatures.get_mut(&creature_guid)?;
        if !creature.looting || !creature.loot_money_available {
            return None;
        }
        let money = creature.loot_money();
        creature.loot_money_available = false;
        Some((money, creature.clone()))
    }

    fn take_db_creature_loot_item(
        &mut self,
        creature_guid: u64,
    ) -> Option<(DbCreatureLootRuntime, DbCreatureRuntime)> {
        let creature = self.creatures.get_mut(&creature_guid)?;
        if !creature.looting {
            return None;
        }
        let loot = creature.loot_item.take()?;
        Some((loot, creature.clone()))
    }

    fn restore_db_creature_loot_item(
        &mut self,
        creature_guid: u64,
        loot: DbCreatureLootRuntime,
    ) -> Option<DbCreatureRuntime> {
        let creature = self.creatures.get_mut(&creature_guid)?;
        if creature.loot_item.is_none() {
            creature.loot_item = Some(loot);
        }
        Some(creature.clone())
    }

    fn release_db_creature_loot(&mut self, creature_guid: u64, now: Instant) -> Option<DbCreatureRuntime> {
        let creature = self.creatures.get_mut(&creature_guid)?;
        creature.looting = false;
        creature.reduce_corpse_decay_after_loot(now);
        Some(creature.clone())
    }

    fn begin_db_creature_combat(
        &mut self,
        attacker: ObjectGuid,
        victim: ObjectGuid,
        now: Instant,
    ) -> Option<CreatureCombatState> {
        if self
            .active_creature_combats
            .get(&attacker.raw())
            .is_some_and(|combat| combat.victim == victim)
        {
            return None;
        }
        if self.active_creature_combats.contains_key(&attacker.raw()) {
            return None;
        }
        let combat = CreatureCombatState {
            attacker,
            victim,
            next_swing_at: now,
        };
        self.active_creature_combats.insert(attacker.raw(), combat);
        Some(combat)
    }

    fn clear_db_creature_combat(&mut self, attacker: ObjectGuid) {
        self.active_creature_combats.remove(&attacker.raw());
    }

    fn clear_db_creature_combats_for_victim(&mut self, victim: ObjectGuid) {
        self.active_creature_combats
            .retain(|_, combat| combat.victim != victim);
    }

    fn active_db_creature_combats_for_victim(
        &self,
        victim: ObjectGuid,
    ) -> Vec<CreatureCombatState> {
        let mut combats = self
            .active_creature_combats
            .values()
            .filter(|combat| combat.victim == victim)
            .copied()
            .collect::<Vec<_>>();
        combats.sort_by_key(|combat| combat.attacker.raw());
        combats
    }

    fn set_db_creature_next_swing(
        &mut self,
        attacker: ObjectGuid,
        next_swing_at: Instant,
    ) -> Option<CreatureCombatState> {
        let combat = self.active_creature_combats.get_mut(&attacker.raw())?;
        combat.next_swing_at = next_swing_at;
        Some(*combat)
    }

    fn defer_ready_db_creature_swing_retry(
        &mut self,
        attacker: ObjectGuid,
        victim: ObjectGuid,
        now: Instant,
    ) -> Option<CreatureCombatState> {
        let combat = self.active_creature_combats.get_mut(&attacker.raw())?;
        if combat.attacker == attacker && combat.victim == victim && now >= combat.next_swing_at {
            combat.next_swing_at = now + Duration::from_millis(DB_CREATURE_MELEE_RETRY_MILLIS);
        }
        Some(*combat)
    }

    fn nearby_player_guids(
        &self,
        position: WorldPosition,
        radius: f32,
        exclude_guid: Option<u32>,
    ) -> Vec<u32> {
        let mut players = HashSet::new();
        self.visit_nearby_cells(position, radius, |cell| {
            players.extend(cell.players.iter().copied());
        });
        let mut players = players
            .into_iter()
            .filter(|guid| Some(*guid) != exclude_guid)
            .filter(|guid| {
                self.players
                    .get(guid)
                    .is_some_and(|player| is_position_inside_radius(player.position, position, radius))
            })
            .collect::<Vec<_>>();
        players.sort_unstable();
        players
    }

    fn visit_nearby_cells(
        &self,
        position: WorldPosition,
        radius: f32,
        mut visitor: impl FnMut(&CellRuntime),
    ) {
        for (grid_coord, cell_coord) in calculate_cell_area(position, radius) {
            let Some(grid) = self.grids.get(&grid_coord) else {
                continue;
            };
            if let Some(cell) = grid.cells.get(&cell_coord) {
                visitor(cell);
            }
        }
    }
}

#[derive(Debug, Default)]
struct MapRuntimeManager {
    maps: Mutex<MapRuntimeHandles>,
}

type MapRuntimeHandles = HashMap<(u32, u32), Arc<Mutex<MapRuntime>>>;

impl MapRuntimeManager {
    async fn add_player(
        &self,
        player: PlayerRuntime,
    ) -> anyhow::Result<Vec<(SessionId, OutboundWorldPacket)>> {
        let map_key = (player.position.map_id, 0);
        let map = {
            let mut maps = self.maps.lock().await;
            maps.entry(map_key)
                .or_insert_with(|| Arc::new(Mutex::new(MapRuntime::new(map_key.0, map_key.1))))
                .clone()
        };
        let packets = map.lock().await.add_player(player);
        packets
    }

    async fn remove_player(
        &self,
        map_id: u32,
        character_guid: u32,
    ) -> Vec<(SessionId, OutboundWorldPacket)> {
        let map = { self.maps.lock().await.get(&(map_id, 0)).cloned() };
        let Some(map) = map else {
            return Vec::new();
        };
        let packets = map.lock().await.remove_player(character_guid);
        packets
    }

    async fn update_player_position(
        &self,
        map_id: u32,
        character_guid: u32,
        opcode: u16,
        movement: &MovementInfo,
    ) -> anyhow::Result<Vec<(SessionId, OutboundWorldPacket)>> {
        let map = { self.maps.lock().await.get(&(map_id, 0)).cloned() };
        let Some(map) = map else {
            return Ok(Vec::new());
        };
        let packets = map
            .lock()
            .await
            .update_player_position(character_guid, opcode, movement);
        packets
    }

    async fn broadcast_nearby_player_packet(
        &self,
        map_id: u32,
        character_guid: u32,
        radius: f32,
        packet: OutboundWorldPacket,
    ) -> Vec<(SessionId, OutboundWorldPacket)> {
        let map = { self.maps.lock().await.get(&(map_id, 0)).cloned() };
        let Some(map) = map else {
            return Vec::new();
        };
        let packets = map
            .lock()
            .await
            .broadcast_nearby_player_packet(character_guid, radius, packet);
        packets
    }

    async fn share_db_creature_snapshots(
        &self,
        map_id: u32,
        creatures: Vec<DbCreatureRuntime>,
    ) -> Vec<DbCreatureRuntime> {
        let map = self.get_or_create_map(map_id, 0).await;
        let creatures = map.lock().await.share_db_creature_snapshots(creatures);
        creatures
    }

    async fn db_creature_snapshots(
        &self,
        map_id: u32,
        creature_guids: &[u64],
    ) -> Vec<DbCreatureRuntime> {
        let map = self.get_or_create_map(map_id, 0).await;
        let snapshots = map.lock().await.db_creature_snapshots(creature_guids);
        snapshots
    }

    #[allow(dead_code)]
    async fn update_db_creature_snapshot(&self, map_id: u32, creature: DbCreatureRuntime) {
        let map = self.get_or_create_map(map_id, 0).await;
        map.lock().await.update_db_creature_snapshot(creature);
    }

    async fn update_db_creature_snapshot_and_broadcast(
        &self,
        map_id: u32,
        creature: DbCreatureRuntime,
        exclude_character_guid: Option<u32>,
        packet: OutboundWorldPacket,
    ) -> Vec<(SessionId, OutboundWorldPacket)> {
        let map = self.get_or_create_map(map_id, 0).await;
        let packets = map
            .lock()
            .await
            .update_db_creature_snapshot_and_broadcast(
                creature,
                exclude_character_guid,
                packet,
        );
        packets
    }

    async fn open_db_creature_loot(
        &self,
        map_id: u32,
        creature_guid: u64,
        loot_item: Option<DbCreatureLootRuntime>,
    ) -> Option<DbCreatureRuntime> {
        let map = self.get_or_create_map(map_id, 0).await;
        let creature = map
            .lock()
            .await
            .open_db_creature_loot(creature_guid, loot_item);
        creature
    }

    async fn take_db_creature_loot_money(
        &self,
        map_id: u32,
        creature_guid: u64,
    ) -> Option<(u32, DbCreatureRuntime)> {
        let map = self.get_or_create_map(map_id, 0).await;
        let loot = map.lock().await.take_db_creature_loot_money(creature_guid);
        loot
    }

    async fn take_db_creature_loot_item(
        &self,
        map_id: u32,
        creature_guid: u64,
    ) -> Option<(DbCreatureLootRuntime, DbCreatureRuntime)> {
        let map = self.get_or_create_map(map_id, 0).await;
        let loot = map.lock().await.take_db_creature_loot_item(creature_guid);
        loot
    }

    async fn restore_db_creature_loot_item(
        &self,
        map_id: u32,
        creature_guid: u64,
        loot: DbCreatureLootRuntime,
    ) -> Option<DbCreatureRuntime> {
        let map = self.get_or_create_map(map_id, 0).await;
        let creature = map
            .lock()
            .await
            .restore_db_creature_loot_item(creature_guid, loot);
        creature
    }

    async fn release_db_creature_loot(
        &self,
        map_id: u32,
        creature_guid: u64,
        now: Instant,
    ) -> Option<DbCreatureRuntime> {
        let map = self.get_or_create_map(map_id, 0).await;
        let creature = map.lock().await.release_db_creature_loot(creature_guid, now);
        creature
    }

    async fn begin_db_creature_combat(
        &self,
        map_id: u32,
        attacker: ObjectGuid,
        victim: ObjectGuid,
        now: Instant,
    ) -> Option<CreatureCombatState> {
        let map = self.get_or_create_map(map_id, 0).await;
        let combat = map
            .lock()
            .await
            .begin_db_creature_combat(attacker, victim, now);
        combat
    }

    async fn clear_db_creature_combat(&self, map_id: u32, attacker: ObjectGuid) {
        let map = self.get_or_create_map(map_id, 0).await;
        map.lock().await.clear_db_creature_combat(attacker);
    }

    async fn clear_db_creature_combats_for_victim(&self, map_id: u32, victim: ObjectGuid) {
        let map = self.get_or_create_map(map_id, 0).await;
        map.lock()
            .await
            .clear_db_creature_combats_for_victim(victim);
    }

    async fn active_db_creature_combats_for_victim(
        &self,
        map_id: u32,
        victim: ObjectGuid,
    ) -> Vec<CreatureCombatState> {
        let map = self.get_or_create_map(map_id, 0).await;
        let combats = map
            .lock()
            .await
            .active_db_creature_combats_for_victim(victim);
        combats
    }

    async fn set_db_creature_next_swing(
        &self,
        map_id: u32,
        attacker: ObjectGuid,
        next_swing_at: Instant,
    ) -> Option<CreatureCombatState> {
        let map = self.get_or_create_map(map_id, 0).await;
        let combat = map
            .lock()
            .await
            .set_db_creature_next_swing(attacker, next_swing_at);
        combat
    }

    async fn defer_ready_db_creature_swing_retry(
        &self,
        map_id: u32,
        attacker: ObjectGuid,
        victim: ObjectGuid,
        now: Instant,
    ) -> Option<CreatureCombatState> {
        let map = self.get_or_create_map(map_id, 0).await;
        let combat = map
            .lock()
            .await
            .defer_ready_db_creature_swing_retry(attacker, victim, now);
        combat
    }

    async fn get_or_create_map(&self, map_id: u32, instance_id: u32) -> Arc<Mutex<MapRuntime>> {
        let map_key = (map_id, instance_id);
        let mut maps = self.maps.lock().await;
        maps.entry(map_key)
            .or_insert_with(|| Arc::new(Mutex::new(MapRuntime::new(map_key.0, map_key.1))))
            .clone()
    }
}

const PLAYER_VISIBILITY_RADIUS_YARDS: f32 = CREATURE_SPAWN_RADIUS_YARDS;
const CHAT_SAY_RADIUS_YARDS: f32 = 25.0;
const CHAT_YELL_RADIUS_YARDS: f32 = 300.0;
const CHAT_EMOTE_RADIUS_YARDS: f32 = CHAT_SAY_RADIUS_YARDS;
const MAX_NUMBER_OF_GRIDS: u32 = 64;
const MAX_NUMBER_OF_CELLS: u32 = 16;
const MAP_SIZE_YARDS: f32 = 34133.333;
const GRID_SIZE_YARDS: f32 = 533.333_3;
const CELL_COUNT_PER_GRID: f32 = MAX_NUMBER_OF_CELLS as f32;
const CELL_SIZE_YARDS: f32 = GRID_SIZE_YARDS / CELL_COUNT_PER_GRID;
const TOTAL_CELL_COUNT_PER_AXIS: u32 = MAX_NUMBER_OF_GRIDS * MAX_NUMBER_OF_CELLS;

fn grid_coord_for_position(position: WorldPosition) -> GridCoord {
    GridCoord {
        x: global_cell_axis_for_world_axis(position.y) / MAX_NUMBER_OF_CELLS,
        y: global_cell_axis_for_world_axis(position.x) / MAX_NUMBER_OF_CELLS,
    }
}

fn cell_coord_for_position(position: WorldPosition) -> CellCoord {
    let global_x = global_cell_axis_for_world_axis(position.y);
    let global_y = global_cell_axis_for_world_axis(position.x);
    CellCoord {
        x: global_x % MAX_NUMBER_OF_CELLS,
        y: global_y % MAX_NUMBER_OF_CELLS,
    }
}

fn calculate_cell_area(position: WorldPosition, radius: f32) -> Vec<(GridCoord, CellCoord)> {
    let radius = radius.max(0.0);
    let min_global_x = global_cell_axis_for_world_axis(position.y + radius);
    let max_global_x = global_cell_axis_for_world_axis(position.y - radius);
    let min_global_y = global_cell_axis_for_world_axis(position.x + radius);
    let max_global_y = global_cell_axis_for_world_axis(position.x - radius);

    let mut cells = Vec::new();
    for global_x in min_global_x..=max_global_x {
        for global_y in min_global_y..=max_global_y {
            cells.push((
                GridCoord {
                    x: global_x / MAX_NUMBER_OF_CELLS,
                    y: global_y / MAX_NUMBER_OF_CELLS,
                },
                CellCoord {
                    x: global_x % MAX_NUMBER_OF_CELLS,
                    y: global_y % MAX_NUMBER_OF_CELLS,
                },
            ));
        }
    }
    cells
}

fn global_cell_axis_for_world_axis(axis: f32) -> u32 {
    let half = MAP_SIZE_YARDS / 2.0;
    ((half - axis) / CELL_SIZE_YARDS)
        .floor()
        .clamp(0.0, (TOTAL_CELL_COUNT_PER_AXIS - 1) as f32) as u32
}
