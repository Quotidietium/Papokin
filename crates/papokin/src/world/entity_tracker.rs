use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};

use bytes::BufMut;
use crossbeam::atomic::AtomicCell;
use dashmap::DashMap;
use dashmap::DashSet;
use papokin_protocol::ClientPacket;
use papokin_protocol::codec::item_stack_seralizer::ItemStackSerializer;
use papokin_protocol::codec::var_int::VarInt;
use papokin_protocol::java::client::play::{
    CEntityVelocity, CHeadRot, CRemoveEntities, CSetEntityMetadata, CSetEquipment, CSetPassengers,
    Metadata,
};
use papokin_util::GameMode;
use papokin_util::math::get_section_cord;
use papokin_util::math::vector2::Vector2;
use papokin_util::math::vector3::Vector3;
use papokin_util::version::JavaMinecraftVersion;
use rustc_hash::FxHashSet;
use uuid::Uuid;

use crate::entity::EntityBase;
use crate::entity::player::Player;
use crate::net::java::JavaClient;
use crate::plugin::player::player_hide_entity::PlayerHideEntityEvent;
use crate::plugin::player::player_show_entity::PlayerShowEntityEvent;
use crate::world::World;
use crate::world::chunker::get_view_distance;

pub struct TrackedEntity {
    pub entity: Arc<dyn EntityBase>,
    pub entity_id: i32,
    pub tracking_range: u32,
    pub update_interval: u32,
    pub track_deltas: bool,
    pub seen_by: DashSet<Uuid>,
    pub last_section_pos: AtomicCell<Vector3<i32>>,
    /// 串行化“配对决定 + 配对/解配对包入队”与“移除收尾
    /// （置位 `finalized` + 快照广播 destroy + 清空 `seen_by`）”，
    /// 关闭二者交错时客户端“只收到 spawn、漏掉 destroy”的
    /// 幽灵实体窗口。锁序约束：本锁是叶子锁——持锁期间只做
    /// `seen_by`/标志位读写与数据包入队，绝不取 `entity_map` 分片
    /// 锁、绝不在锁内 `fire_blocking`（插件回调可能重入）。
    pair_lock: Mutex<()>,
    /// 移除收尾是否已执行（在 `pair_lock` 内置位/读取）。
    finalized: AtomicBool,
}

/// `update_player` 单次评估得到的配对决定。
enum PairingDecision {
    /// 新配对：decide 阶段已写入 `seen_by`，待事件放行后补发 spawn。
    Pair,
    /// 取消配对：decide 阶段已移出 `seen_by`，待事件放行后补发 destroy。
    Unpair,
    /// 无变化。
    Unchanged,
}

/// 4× 滞回收缩判定（与轮次 21 编码缓存、capacity 衰减原语同参数）：
/// 容量超 `len × 4 + 64` 才把桶数组收缩到当前负载适配值。
fn maybe_shrink_buckets<K: Eq + std::hash::Hash, V>(map: &dashmap::DashMap<K, V>) {
    if map.capacity() > map.len().saturating_mul(4) + 64 {
        map.shrink_to_fit();
    }
}

impl TrackedEntity {
    #[must_use]
    pub fn new(
        entity: Arc<dyn EntityBase>,
        range: u32,
        update_interval: u32,
        track_deltas: bool,
    ) -> Self {
        let entity_id = entity.get_entity().entity_id;
        let pos = entity.get_entity().pos.load();
        let last_section_pos = Vector3::new(
            get_section_cord(pos.x.floor() as i32),
            get_section_cord(pos.y.floor() as i32),
            get_section_cord(pos.z.floor() as i32),
        );
        Self {
            entity,
            entity_id,
            tracking_range: range,
            update_interval,
            track_deltas,
            seen_by: DashSet::new(),
            last_section_pos: AtomicCell::new(last_section_pos),
            pair_lock: Mutex::new(()),
            finalized: AtomicBool::new(false),
        }
    }

    fn collect_indirect_passengers(
        entity: &Arc<dyn EntityBase>,
        result: &mut Vec<Arc<dyn EntityBase>>,
    ) {
        if let Ok(passengers) = entity.get_entity().passengers.try_lock() {
            for passenger in passengers.iter() {
                result.push(passenger.clone());
                Self::collect_indirect_passengers(passenger, result);
            }
        }
    }

    #[must_use]
    pub fn get_effective_range(&self) -> u32 {
        let mut effective_range = self.tracking_range;
        let mut passengers = Vec::new();
        Self::collect_indirect_passengers(&self.entity, &mut passengers);
        for passenger in passengers {
            let passenger_range = passenger.get_entity().entity_type.client_tracking_range;
            if passenger_range > effective_range {
                effective_range = passenger_range;
            }
        }
        effective_range
    }

    fn broadcast_to_player(&self, player: &Player) -> bool {
        self.entity.get_player().is_none_or(|target_player| {
            player.gamemode.load() == GameMode::Spectator
                || target_player.gamemode.load() != GameMode::Spectator
        })
    }

    pub fn update_player(&self, player: &Arc<Player>, world: &World) {
        if player.get_entity().entity_id == self.entity_id {
            return;
        }

        let is_visible = self.should_be_visible(player);

        // 决定阶段：在 `pair_lock` 内检查移除状态并写入 `seen_by`，
        // 与 finalize_removal 的“快照+广播+清空”互斥，保证本玩家的
        // insert 要么进入移除快照（随后收到 destroy），要么观察到
        // finalized 而根本不配对——不会出现只发 spawn 的中间态。
        let decision = {
            let _guard = self
                .pair_lock
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            if self.finalized.load(Ordering::Relaxed) {
                return;
            }
            if is_visible {
                if self.seen_by.insert(player.gameprofile.id) {
                    PairingDecision::Pair
                } else {
                    PairingDecision::Unchanged
                }
            } else if self.seen_by.remove(&player.gameprofile.id).is_some() {
                PairingDecision::Unpair
            } else {
                PairingDecision::Unchanged
            }
        };

        // 行动阶段：插件事件全部在锁外触发。锁内 fire_blocking 会与
        // 重入本实体 pair_lock 的插件回调（如事件处理器里移除同一实体）
        // 互锁，见 pair_lock 的字段注释。
        match decision {
            PairingDecision::Pair => self.apply_new_pairing(player, world),
            PairingDecision::Unpair => self.apply_removed_pairing(player, world),
            PairingDecision::Unchanged => {}
        }
    }

    /// 原版 `isChunkTracked` 语义的可见性判定：距离 + 观战规则 +
    /// 注视段 + 区块已就绪（绝不在区块数据包之前生成）。
    fn should_be_visible(&self, player: &Player) -> bool {
        let player_entity = player.get_entity();
        let player_pos = player_entity.pos.load();
        let entity_pos = self.entity.get_entity().pos.load();
        let dx = player_pos.x - entity_pos.x;
        let dz = player_pos.z - entity_pos.z;
        let dist_sq = dx.mul_add(dx, dz * dz);

        let player_vd = get_view_distance(player).get() as i32;
        let effective_range = self.get_effective_range();
        let visible_range_blocks = f64::from((effective_range as i32).min(player_vd) * 16);
        let range_sq = visible_range_blocks * visible_range_blocks;

        let entity_chunk = self.entity.get_entity().chunk_pos.load();
        let in_view = player
            .watched_section
            .load()
            .is_within_distance(entity_chunk.x, entity_chunk.y);

        dist_sq <= range_sq
            && self.broadcast_to_player(player)
            && in_view
            && player
                .chunk_sender
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .is_chunk_ready(&entity_chunk)
    }

    /// Pair 决定的行动阶段：show 事件放行后在 `pair_lock` 内补发
    /// spawn；事件被否决或实体在事件期间被移除则回滚 `seen_by`。
    fn apply_new_pairing(&self, player: &Arc<Player>, world: &World) {
        let mut show_event = PlayerShowEntityEvent {
            player: player.clone(),
            entity_id: self.entity_id,
            cancelled: false,
        };
        if let Some(server) = world.server.upgrade() {
            server
                .plugin_manager
                .fire_blocking(&server, &mut show_event);
        }
        if show_event.cancelled {
            // 保持实体未配对状态，以便在下一次追踪更新时重新评估
            // 可见性。（实体若已被并发移除，finalize_removal 的清空
            // 会覆盖这次回滚，无影响。）
            self.seen_by.remove(&player.gameprofile.id);
            return;
        }
        // 事件处理期间实体可能被移除：不再补发 spawn，并回滚 decide
        // 阶段写入的 seen_by 标记。spawn 包在锁内入队，与
        // finalize_removal 的 destroy 入队经同一把锁定序，客户端
        // 不会先收 destroy 后收 spawn。
        if !self.try_commit_pairing(player) {
            return;
        }
        // 追踪通知，在配对添加完成后触发。
        let mut track_event =
            crate::plugin::api::events::player::player_track_entity::PlayerTrackEntityEvent::new(
                player.clone(),
                self.entity_id,
            );
        if let Some(server) = world.server.upgrade() {
            server
                .plugin_manager
                .fire_blocking(&server, &mut track_event);
        }
    }

    /// show 事件之后的复核与入队：在 `pair_lock` 内确认实体未被移除
    /// 后补发 spawn。返回 false 表示实体已被移除（`seen_by` 已回滚）。
    fn try_commit_pairing(&self, player: &Arc<Player>) -> bool {
        let _guard = self
            .pair_lock
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if self.finalized.load(Ordering::Relaxed) {
            self.seen_by.remove(&player.gameprofile.id);
            return false;
        }
        self.add_pairing(player);
        true
    }

    /// Unpair 决定的行动阶段：hide 事件放行后发送 destroy 并触发
    /// untrack 通知；事件被否决则在锁内复活 `seen_by` 保持配对。
    fn apply_removed_pairing(&self, player: &Arc<Player>, world: &World) {
        let mut hide_event = PlayerHideEntityEvent {
            player: player.clone(),
            entity_id: self.entity_id,
            cancelled: false,
        };
        if let Some(server) = world.server.upgrade() {
            server
                .plugin_manager
                .fire_blocking(&server, &mut hide_event);
        }
        if hide_event.cancelled {
            // 已否决：该实体对此玩家保持配对（可见）。
            // 实体若已被并发移除则不复活 seen_by（finalize
            // 即将清空它）。
            let _guard = self
                .pair_lock
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            if !self.finalized.load(Ordering::Relaxed) {
                self.seen_by.insert(player.gameprofile.id);
            }
        } else {
            self.remove_pairing(player);
            // 取消跟踪通知，在配对被移除后触发。
            let mut untrack_event =
                crate::plugin::api::events::player::player_untrack_entity::PlayerUntrackEntityEvent::new(
                    player.clone(),
                    self.entity_id,
                );
            if let Some(server) = world.server.upgrade() {
                server
                    .plugin_manager
                    .fire_blocking(&server, &mut untrack_event);
            }
        }
    }

    pub fn update_players(&self, players: &[Arc<Player>], world: &World) {
        for player in players {
            self.update_player(player, world);
        }
    }

    #[allow(clippy::too_many_lines)]
    pub fn add_pairing(&self, player: &Arc<Player>) {
        self.entity.send_java_spawn_packet(&player.client);
        player.try_restore_vehicle(&self.entity);

        let client = &player.client;

        if let Some(target_player) = self.entity.get_player() {
            let skin_parts = target_player.config.load().skin_parts;
            let target_entity = target_player.get_entity();
            let target_id = target_entity.entity_id;

            let version = client.version.load();
            if version >= JavaMinecraftVersion::V_1_21 {
                let mut buf = Vec::new();
                for meta in [
                    Metadata::new(
                        papokin_data::tracked_data::player::PLAYER_MODE_CUSTOMISATION,
                        skin_parts,
                    ),
                    Metadata::new(
                        papokin_data::tracked_data::player::PLAYER_MODE_CUSTOMIZATION_ID,
                        skin_parts,
                    ),
                ] {
                    let _ = meta.write(&mut buf, &version);
                }
                buf.put_u8(255);
                let meta_packet = CSetEntityMetadata::new(target_id.into(), buf.into());
                if let Ok(packet_data) =
                    JavaClient::serialize_packet_for_version(&meta_packet, version)
                {
                    client.try_enqueue_packet(packet_data);
                }
            }

            let head_yaw = target_entity.head_yaw.load();
            let head_rot_packet = CHeadRot::new(
                target_id.into(),
                (head_yaw * 256.0 / 360.0).rem_euclid(256.0) as u8,
            );
            if let Ok(data) = client.serialize_packet(&head_rot_packet) {
                client.try_enqueue_packet(data);
            }
        } else if self.entity.get_living_entity().is_some() {
            let head_yaw = self.entity.get_entity().head_yaw.load();
            let head_rot_packet = CHeadRot::new(
                self.entity_id.into(),
                (head_yaw * 256.0 / 360.0).rem_euclid(256.0) as u8,
            );
            if let Ok(data) = client.serialize_packet(&head_rot_packet) {
                client.try_enqueue_packet(data);
            }
        }

        let vel = self.entity.get_entity().velocity.load();
        if vel.length_squared() > 1e-4 {
            let motion = CEntityVelocity::new(self.entity_id.into(), vel);
            if let Ok(data) = client.serialize_packet(&motion) {
                client.try_enqueue_packet(data);
            }
        }

        {
            let version = client.version.load();
            // TODO: 支持旧版本
            if version >= JavaMinecraftVersion::V_1_21
                && let Some(non_default) = self
                    .entity
                    .get_entity()
                    .synched_data
                    .get_non_default_values_for_version(&version)
            {
                let packet = CSetEntityMetadata::new(self.entity_id.into(), non_default);
                if let Ok(packet_data) = JavaClient::serialize_packet_for_version(&packet, version)
                {
                    client.try_enqueue_packet(packet_data);
                }
            }
        }

        // 属性同步：原版在配对数据里随 spawn 下发实体的全部属性，
        // 客户端依此渲染骑乘血条（最大生命）与移动/攻击动画速度。
        if let Some(living) = self.entity.get_living_entity() {
            let packet = crate::entity::attributes::full_sync_packet_for_living(living);
            if !packet.properties.is_empty()
                && let Ok(data) = client.serialize_packet(&packet)
            {
                client.try_enqueue_packet(data);
            }
        }

        if let Some(living) = self.entity.get_living_entity()
            && let Ok(equipment_guard) = living.entity_equipment.try_lock()
        {
            let mut equipment_list = Vec::new();
            for (slot, item_stack) in &equipment_guard.equipment {
                if !item_stack.is_empty() {
                    equipment_list.push((slot.discriminant(), item_stack.clone()));
                }
            }
            if !equipment_list.is_empty() {
                let equipment: Vec<(i8, ItemStackSerializer)> = equipment_list
                    .iter()
                    .map(|(slot, stack)| (*slot, ItemStackSerializer::from(stack.clone())))
                    .collect();
                let packet = CSetEquipment::new(self.entity_id.into(), equipment);
                if let Ok(data) = client.serialize_packet(&packet) {
                    client.try_enqueue_packet(data);
                }
            }
        }

        if let Ok(passengers) = self.entity.get_entity().passengers.try_lock()
            && !passengers.is_empty()
        {
            let passenger_ids: Vec<VarInt> = passengers
                .iter()
                .map(|p| VarInt(p.get_entity().entity_id))
                .collect();
            let packet = CSetPassengers::new(VarInt(self.entity_id), &passenger_ids);
            if let Ok(data) = client.serialize_packet(&packet) {
                client.try_enqueue_packet(data);
            }
        }

        if let Ok(vehicle_guard) = self.entity.get_entity().vehicle.try_lock()
            && let Some(vehicle) = vehicle_guard.as_ref()
            && let Ok(vehicle_passengers) = vehicle.get_entity().passengers.try_lock()
        {
            let passenger_ids: Vec<VarInt> = vehicle_passengers
                .iter()
                .map(|p| VarInt(p.get_entity().entity_id))
                .collect();
            let packet =
                CSetPassengers::new(VarInt(vehicle.get_entity().entity_id), &passenger_ids);
            if let Ok(data) = client.serialize_packet(&packet) {
                client.try_enqueue_packet(data);
            }
        }
    }

    pub fn remove_pairing(&self, player: &Player) {
        let entity_ids = [self.entity_id.into()];
        let packet = CRemoveEntities::new(&entity_ids);
        if let Ok(data) = player.client.serialize_packet(&packet) {
            player.client.try_enqueue_packet(data);
        }
    }

    pub fn broadcast_removed(&self, world: &World) {
        let entity_ids = [self.entity_id.into()];
        let je_packet = CRemoveEntities::new(&entity_ids);

        let players = world.players.load();
        let recipients = players
            .iter()
            .filter(|p| self.seen_by.contains(&p.gameprofile.id));

        // 轮次 18：单趟内联版本槽广播（原为 BTreeMap 分组两步式）
        World::broadcast_java_clients(&je_packet, recipients.map(|p| &*p.client));

        self.seen_by.clear();
    }

    /// 原版 `TrackedEntity.removePlayer`：在客户端上取消生成，仅当已配对时。
    pub fn remove_player(&self, player: &Player) {
        let _guard = self
            .pair_lock
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if self.finalized.load(Ordering::Relaxed) {
            return;
        }
        if self.seen_by.remove(&player.gameprofile.id).is_some() {
            self.remove_pairing(player);
        }
    }

    pub fn send_to_tracking_players<P: ClientPacket + Sync>(&self, packet: &P, world: &World) {
        let players = world.players.load();
        let recipients = players
            .iter()
            .filter(|p| self.seen_by.contains(&p.gameprofile.id));
        // 轮次 18：单趟内联版本槽广播（原为 BTreeMap 分组两步式）；
        // 本函数是实体移动/属性包的主扇出，每移动实体每 tick 至少一次
        World::broadcast_java_clients(packet, recipients.map(|p| &*p.client));
    }

    pub fn send_to_tracking_players_and_self<P: ClientPacket + Sync>(
        &self,
        packet: &P,
        world: &World,
    ) {
        self.send_to_tracking_players(packet, world);
        if let Some(player) = self.entity.get_player() {
            player.try_send_client_packet(packet);
        }
    }

    /// 移除收尾：在 `pair_lock` 内置位 `finalized`、按 `seen_by` 快照广播
    /// destroy 并清空集合。与 `update_player` 的决定/入队临界区互斥，
    /// 客户端不会“只收到 spawn 而漏掉 destroy”（幽灵实体）。
    /// 由 `EntityTracker::remove_entity` 在实体离开 `entity_map` 后调用，
    /// 此后对该 `TrackedEntity` 的一切配对都会被拒绝。
    pub fn finalize_removal(&self, world: &World) {
        let _guard = self
            .pair_lock
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        self.finalized.store(true, Ordering::Relaxed);
        self.broadcast_removed(world);
    }

    pub fn send_to_tracking_players_filtered<P: ClientPacket + Sync, F: Fn(&Player) -> bool>(
        &self,
        packet: &P,
        world: &World,
        filter: F,
    ) {
        let players = world.players.load();
        let recipients = players
            .iter()
            .filter(|p| self.seen_by.contains(&p.gameprofile.id) && filter(p));
        // 轮次 18：单趟内联版本槽广播（原为 BTreeMap 分组两步式）
        World::broadcast_java_clients(packet, recipients.map(|p| &*p.client));
    }
}

/// `update_all` 驻留缓冲：受追踪实体快照 + 跨区移动玩家列表（轮次 16）
type UpdateAllScratch = (Vec<Arc<TrackedEntity>>, Vec<Arc<Player>>);

pub struct EntityTracker {
    pub entity_map: DashMap<i32, Arc<TrackedEntity>>,
    /// `update_all` 的快照/跨区移动玩家驻留缓冲（轮次 16）：仅 tick
    /// 主循环单线程调用 `update_all`，插件回调不存在重入 `update_all`
    /// 的路径（`update_player_position` 不触碰本缓冲），持锁迭代安全。
    update_all_scratch: std::sync::Mutex<UpdateAllScratch>,
}

impl Default for EntityTracker {
    fn default() -> Self {
        Self::new()
    }
}

impl EntityTracker {
    #[must_use]
    pub fn new() -> Self {
        Self {
            entity_map: DashMap::new(),
            update_all_scratch: std::sync::Mutex::new((Vec::new(), Vec::new())),
        }
    }

    #[must_use]
    pub fn get_tracked_entity(&self, entity_id: i32) -> Option<Arc<TrackedEntity>> {
        self.entity_map.get(&entity_id).map(|r| r.value().clone())
    }

    #[must_use]
    pub fn has_entity_with_id(&self, entity_id: i32) -> bool {
        self.entity_map.contains_key(&entity_id)
    }

    #[must_use]
    pub fn is_tracked_by_any_player(&self, entity_id: i32) -> bool {
        self.entity_map
            .get(&entity_id)
            .is_some_and(|t| !t.seen_by.is_empty())
    }

    pub fn for_each_entity_tracked_by<F: FnMut(&Arc<dyn EntityBase>)>(
        &self,
        player: &Player,
        mut f: F,
    ) {
        // 先克隆快照再迭代：迭代期间回调 f 可能触发插件事件或增删
        // 实体，DashMap 迭代器持分片读锁，与之互斥会造成死锁。
        let tracked_entities: Vec<Arc<TrackedEntity>> =
            self.entity_map.iter().map(|e| e.value().clone()).collect();
        for tracked in &tracked_entities {
            if tracked.seen_by.contains(&player.gameprofile.id) {
                f(&tracked.entity);
            }
        }
    }

    pub fn add_entity(&self, entity: &Arc<dyn EntityBase>, world: &World) {
        let entity_type = entity.get_entity().entity_type;
        let range = entity_type.client_tracking_range;
        if range == 0 {
            return;
        }
        let update_interval = entity_type.update_interval;
        let track_deltas = entity_type.track_deltas;
        let entity_id = entity.get_entity().entity_id;

        let tracked = Arc::new(TrackedEntity::new(
            entity.clone(),
            range,
            update_interval,
            track_deltas,
        ));
        self.entity_map.insert(entity_id, tracked.clone());

        let players = world.players.load();
        tracked.update_players(players.as_ref(), world);
    }

    /// 只能在玩家自身的 `CLogin` 数据包发送之后调用。
    pub fn pair_new_player_with_tracked_entities(&self, player_arc: &Arc<Player>, world: &World) {
        let entity_id = player_arc.get_entity().entity_id;
        // 快照后迭代：update_player 会触发阻塞插件事件，不能在
        // entity_map 分片读锁下进行。
        let tracked_entities: Vec<Arc<TrackedEntity>> =
            self.entity_map.iter().map(|e| e.value().clone()).collect();
        for tracked in &tracked_entities {
            if tracked.entity_id != entity_id {
                tracked.update_player(player_arc, world);
            }
        }
    }

    /// 将刚为 `player` 排入数据包队列的区块中的实体进行配对。
    pub fn update_player_chunks(
        &self,
        player: &Arc<Player>,
        world: &World,
        chunks: &[Vector2<i32>],
    ) {
        let chunks: FxHashSet<_> = chunks.iter().copied().collect();
        let entity_id = player.get_entity().entity_id;
        // 快照后迭代，理由同 pair_new_player_with_tracked_entities。
        let tracked_entities: Vec<Arc<TrackedEntity>> =
            self.entity_map.iter().map(|e| e.value().clone()).collect();
        for tracked in &tracked_entities {
            if tracked.entity_id != entity_id
                && chunks.contains(&tracked.entity.get_entity().chunk_pos.load())
            {
                tracked.update_player(player, world);
            }
        }
    }

    pub fn remove_entity(&self, entity: &dyn EntityBase, world: &World) {
        let entity_id = entity.get_entity().entity_id;
        if let Some(player) = entity.get_player() {
            let tracked_entities: Vec<Arc<TrackedEntity>> =
                self.entity_map.iter().map(|e| e.value().clone()).collect();
            for tracked in &tracked_entities {
                tracked.remove_player(player);
            }
        }

        if let Some((_, tracked)) = self.entity_map.remove(&entity_id) {
            tracked.finalize_removal(world);
        }
    }

    pub fn update_player_position(&self, player: &Arc<Player>, world: &World) {
        let pos = player.get_entity().pos.load();
        let new_pos = Vector3::new(
            get_section_cord(pos.x.floor() as i32),
            get_section_cord(pos.y.floor() as i32),
            get_section_cord(pos.z.floor() as i32),
        );
        if let Some(tracked) = self.get_tracked_entity(player.get_entity().entity_id) {
            tracked.last_section_pos.store(new_pos);
        }
        // 快照后迭代：不能在 entity_map 分片读锁下触发阻塞插件事件
        // 或让插件回调写 entity_map（跨线程分片锁死锁）。
        let tracked_entities: Vec<Arc<TrackedEntity>> =
            self.entity_map.iter().map(|e| e.value().clone()).collect();
        for tracked in &tracked_entities {
            if tracked.entity_id == player.get_entity().entity_id {
                let players = world.players.load();
                tracked.update_players(players.as_ref(), world);
            } else {
                tracked.update_player(player, world);
            }
        }
    }

    pub fn update_entity_position(&self, entity: &dyn EntityBase, world: &World) {
        // 用克隆而非 map.get 的 Ref 守卫：后者在 update_players 触发
        // 阻塞插件事件期间持续持有分片读锁，同样会死锁。
        if let Some(tracked) = self.get_tracked_entity(entity.get_entity().entity_id) {
            let pos = entity.get_entity().pos.load();
            let new_pos = Vector3::new(
                get_section_cord(pos.x.floor() as i32),
                get_section_cord(pos.y.floor() as i32),
                get_section_cord(pos.z.floor() as i32),
            );
            tracked.last_section_pos.store(new_pos);
            let players = world.players.load();
            tracked.update_players(players.as_ref(), world);
        }
    }

    pub fn update_all(&self, world: &World) {
        let players = world.players.load();
        // 轮次 16：快照与移动玩家列表跨 tick 驻留复用（clear 重填）。
        // 锁仅本函数获取；`update_players` 触发的插件回调无重入
        // `update_all` 的路径，持锁迭代不构成死锁。
        let scratch = &mut *self
            .update_all_scratch
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let (tracked_entities, moved_players) = scratch;
        // 轮次 20：衰减清理（容量超 4× 上轮长度才收缩，稳态零收缩）
        papokin_util::capacity::decay_clear_vec(moved_players);

        // 快照后迭代，理由同 update_player_position。
        papokin_util::capacity::decay_clear_vec(tracked_entities);
        tracked_entities.extend(self.entity_map.iter().map(|e| e.value().clone()));
        for tracked in tracked_entities.iter() {
            let pos = tracked.entity.get_entity().pos.load();
            let new_pos = Vector3::new(
                get_section_cord(pos.x.floor() as i32),
                get_section_cord(pos.y.floor() as i32),
                get_section_cord(pos.z.floor() as i32),
            );
            let old_pos = tracked.last_section_pos.load();
            if old_pos != new_pos {
                tracked.update_players(players.as_ref(), world);
                if let Some(player) = tracked.entity.get_player()
                    && let Some(player_arc) = world.get_player_by_uuid(player.gameprofile.id)
                {
                    moved_players.push(player_arc);
                }
                tracked.last_section_pos.store(new_pos);
            }
        }

        if !moved_players.is_empty() {
            for tracked in tracked_entities.iter() {
                tracked.update_players(moved_players, world);
            }
        }

        for tracked in tracked_entities.iter() {
            if tracked.entity.get_entity().synched_data.is_dirty() {
                tracked.entity.get_entity().send_dirty_entity_data();
            }
        }

        // 轮次 22：事件高峰（刷怪塔/袭击万级实体）回落后回收桶数组——
        // dashmap 的 remove 只减计数不缩桶，峰值槽位会驻留到世界卸载。
        // 此处无任何分片守卫在手（上方快照 extend 早已结束，
        // tracked_entities 持的是 Arc 克隆）；update_all 由世界 tick
        // 单线程调用且被 scratch 互斥锁序列化，shrink 不会与自身重叠；
        // 并发生成/消失路径的 insert/remove 仅在命中同一分片时短暂
        // park。4× 滞回使实体数回涨 4 倍内不再收缩，防振荡。
        maybe_shrink_buckets(&self.entity_map);
    }
}

#[cfg(test)]
mod tests {
    use super::maybe_shrink_buckets;

    #[test]
    fn maybe_shrink_buckets_reclaims_after_mass_removal() {
        let map: dashmap::DashMap<i32, u64> = dashmap::DashMap::new();
        for i in 0..8192 {
            map.insert(i, i as u64);
        }
        let cap_before = map.capacity();
        for i in 0..8192 {
            map.remove(&i);
        }

        maybe_shrink_buckets(&map);

        let cap_after = map.capacity();
        assert!(
            cap_after.saturating_mul(4) <= cap_before,
            "全量移除后桶容量应至少回收 4 倍: {cap_before} -> {cap_after}"
        );
    }

    #[test]
    fn maybe_shrink_buckets_holds_within_hysteresis() {
        let map: dashmap::DashMap<i32, u64> = dashmap::DashMap::new();
        for i in 0..8192 {
            map.insert(i, i as u64);
        }
        // 移除 1/4：3/4 占用远高于 4× 滞回线（阈值 24640，远超任意
        // 分片数下 8192 条目的容量）。注意 dashmap 的 remove 可能
        // 顺带收缩个别分片，容量一律以移除后实测为准
        for i in 0..2048 {
            map.remove(&i);
        }
        let cap_after_removal = map.capacity();
        assert!(
            cap_after_removal <= map.len().saturating_mul(4) + 64,
            "用例前提：3/4 占用必在滞回线内（{cap_after_removal} vs len {})",
            map.len()
        );

        maybe_shrink_buckets(&map);

        assert_eq!(map.capacity(), cap_after_removal, "滞回线内不得收缩");
    }
}
