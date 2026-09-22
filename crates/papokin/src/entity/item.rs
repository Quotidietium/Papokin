use crate::entity::player::statistics::StatisticCategory;
use crate::server::Server;
use core::f32;
use papokin_data::damage::DamageType;
use papokin_data::data_component_impl::DamageResistantImpl;
use papokin_data::data_component_impl::DamageResistantType;
use papokin_data::item_stack::ItemStack;
use papokin_nbt::compound::NbtCompound;
use papokin_protocol::codec::item_stack_seralizer::ItemStackSerializer;
use papokin_protocol::java::client::play::{CSetEntityMetadata, Metadata};
use papokin_util::math::atomic_f32::AtomicF32;
use papokin_util::math::vector3::Vector3;
use papokin_util::version::JavaMinecraftVersion;
use std::sync::atomic::Ordering::{AcqRel, Relaxed};

use std::sync::{
    Arc, Mutex,
    atomic::{
        AtomicBool, AtomicU8, AtomicU32,
        Ordering::{self},
    },
};

use super::{Entity, EntityBase, living::LivingEntity, player::Player};

pub struct ItemEntity {
    entity: Entity,
    item_age: AtomicU32,
    // 这些不能是原子值，因为我们会根据它们本身的类型修改其状态；我们会运行
    // 陷入 ABA 问题
    item_stack: Mutex<ItemStack>,
    pickup_delay: AtomicU8,
    health: AtomicF32,
    never_despawn: AtomicBool,
    never_pickup: AtomicBool,
    merge_reserved: AtomicBool,
}

struct ItemMergeReservation<'a> {
    low: &'a AtomicBool,
    high: &'a AtomicBool,
}

impl<'a> ItemMergeReservation<'a> {
    fn acquire(low: &'a ItemEntity, high: &'a ItemEntity) -> Option<Self> {
        if std::ptr::eq(low, high)
            || low
                .merge_reserved
                .compare_exchange(false, true, AcqRel, Ordering::Acquire)
                .is_err()
        {
            return None;
        }

        if high
            .merge_reserved
            .compare_exchange(false, true, AcqRel, Ordering::Acquire)
            .is_err()
        {
            low.merge_reserved.store(false, Ordering::Release);
            return None;
        }

        Some(Self {
            low: &low.merge_reserved,
            high: &high.merge_reserved,
        })
    }
}

impl Drop for ItemMergeReservation<'_> {
    fn drop(&mut self) {
        self.high.store(false, Ordering::Release);
        self.low.store(false, Ordering::Release);
    }
}

const ITEM_UPDATE_INTERVAL: u32 = 20;

impl ItemEntity {
    pub const DEFAULT_PICKUP_DELAY: u8 = 10;

    pub fn new(entity: Entity, item_stack: ItemStack) -> Self {
        entity.velocity.store(Vector3::new(
            rand::random::<f64>().mul_add(0.2, -0.1),
            0.2,
            rand::random::<f64>().mul_add(0.2, -0.1),
        ));
        entity.yaw.store(rand::random::<f32>() * 360.0);

        // 为某些物品设置防火免疫
        if let Some(res) = item_stack.get_data_component::<DamageResistantImpl>()
            && res.res_type == DamageResistantType::Fire
        {
            entity.fire_immune.store(true, Ordering::Relaxed);
        }

        Self {
            entity,
            item_stack: Mutex::new(item_stack),
            item_age: AtomicU32::new(0),
            pickup_delay: AtomicU8::new(Self::DEFAULT_PICKUP_DELAY),
            health: AtomicF32::new(5.0),
            never_despawn: AtomicBool::new(false),
            never_pickup: AtomicBool::new(false),
            merge_reserved: AtomicBool::new(false),
        }
    }

    pub fn new_with_velocity(
        entity: Entity,
        item_stack: ItemStack,
        velocity: Vector3<f64>,
        pickup_delay: u8,
    ) -> Self {
        entity.velocity.store(velocity);
        entity.yaw.store(rand::random::<f32>() * 360.0);

        // 为某些物品设置防火免疫
        if let Some(res) = item_stack.get_data_component::<DamageResistantImpl>()
            && res.res_type == DamageResistantType::Fire
        {
            entity.fire_immune.store(true, Ordering::Relaxed);
        }

        Self {
            entity,
            item_stack: Mutex::new(item_stack),
            item_age: AtomicU32::new(0),
            pickup_delay: AtomicU8::new(pickup_delay), // 原版拾取延迟为 10 刻
            health: AtomicF32::new(5.0),
            never_despawn: AtomicBool::new(false),
            never_pickup: AtomicBool::new(false),
            merge_reserved: AtomicBool::new(false),
        }
    }

    /// 创建一个用于从 NBT 恢复且不带随机速度的 `ItemEntity`。
    /// 速度和位置将由 `Entity::read_nbt_non_mut` 设置。
    pub fn new_empty(entity: Entity) -> Self {
        Self {
            entity,
            item_stack: Mutex::new(ItemStack::new(1, &papokin_data::item::Item::AIR)),
            item_age: AtomicU32::new(0),
            pickup_delay: AtomicU8::new(0),
            health: AtomicF32::new(5.0),
            never_despawn: AtomicBool::new(false),
            never_pickup: AtomicBool::new(false),
            merge_reserved: AtomicBool::new(false),
        }
    }

    pub const fn get_item_stack(&self) -> &Mutex<ItemStack> {
        &self.item_stack
    }

    pub fn get_pickup_delay(&self) -> u8 {
        self.pickup_delay.load(Ordering::Relaxed)
    }

    pub fn set_pickup_delay(&self, pickup_delay: u8) {
        self.pickup_delay.store(pickup_delay, Ordering::Relaxed);
    }

    pub const fn get_entity(&self) -> &Entity {
        &self.entity
    }

    pub fn can_merge(&self) -> bool {
        let Ok(item_stack) = self.item_stack.try_lock() else {
            return false;
        };

        item_stack.item_count < item_stack.get_max_stack_size()
    }

    pub fn try_merge(&self) {
        if !self.can_merge() || self.never_despawn.load(Ordering::Relaxed) {
            return;
        }

        let bounding_box = self.entity.bounding_box.load().expand(0.5, 0.0, 0.5);

        let world = self.entity.world.load();
        let entities = world.entities.load();
        let items: Vec<&Self> = entities
            .iter()
            .filter_map(|entity: &Arc<dyn EntityBase>| {
                entity.get_item_entity().filter(|item| {
                    item.entity.entity_id != self.entity.entity_id
                        && !item.never_despawn.load(Ordering::Relaxed)
                        && item.entity.bounding_box.load().intersects(&bounding_box)
                })
            })
            .collect();

        for item in items {
            if item.can_merge() {
                if let Some(this_base) = world.get_entity_by_id(self.entity.entity_id)
                    && let Some(this_item) = this_base.get_item_entity()
                {
                    this_item.try_merge_with(item);
                }

                if self.entity.removed.load(Ordering::SeqCst) {
                    break;
                }
            }
        }
    }

    #[expect(clippy::too_many_lines)]
    fn try_merge_with(&self, other: &Self) {
        // 始终按 entity_id 顺序加锁，以避免当两个
        // 物品试图并发地相互合并。
        let (low, high) = if self.entity.entity_id < other.entity.entity_id {
            (self, other)
        } else {
            (other, self)
        };
        let Some(_reservation) = ItemMergeReservation::acquire(low, high) else {
            return;
        };

        let (expected_low, expected_high, target_is_self) = {
            let low_stack = low
                .item_stack
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            let high_stack = high
                .item_stack
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            let (self_stack, other_stack) = if self.entity.entity_id < other.entity.entity_id {
                (&*low_stack, &*high_stack)
            } else {
                (&*high_stack, &*low_stack)
            };

            if !self_stack.are_equal(other_stack)
                || self_stack.item_count + other_stack.item_count > self_stack.get_max_stack_size()
            {
                return;
            }

            (
                low_stack.clone(),
                high_stack.clone(),
                other_stack.item_count < self_stack.item_count,
            )
        };
        let (target, source) = if target_is_self {
            (self, other)
        } else {
            (other, self)
        };
        if self.entity.removed.load(Ordering::SeqCst) || other.entity.removed.load(Ordering::SeqCst)
        {
            return;
        }

        let mut event = crate::plugin::api::events::entity::item_merge::ItemMergeEvent {
            entity_id: target.entity.entity_id,
            target_id: source.entity.entity_id,
            cancelled: false,
        };
        let server = self.entity.world.load().server.upgrade();
        if let Some(server) = server {
            server.plugin_manager.fire_blocking(&server, &mut event);
        }
        if event.cancelled
            || self.entity.removed.load(Ordering::SeqCst)
            || other.entity.removed.load(Ordering::SeqCst)
        {
            return;
        }

        let low_stack = low
            .item_stack
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let high_stack = high
            .item_stack
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if low_stack.uid != expected_low.uid
            || !low_stack.are_equal(&expected_low)
            || high_stack.uid != expected_high.uid
            || !high_stack.are_equal(&expected_high)
            || self.entity.removed.load(Ordering::SeqCst)
            || other.entity.removed.load(Ordering::SeqCst)
        {
            return;
        }

        let (self_stack, other_stack) = if self.entity.entity_id < other.entity.entity_id {
            (low_stack, high_stack)
        } else {
            (high_stack, low_stack)
        };
        if !self_stack.are_equal(&other_stack)
            || self_stack.item_count + other_stack.item_count > self_stack.get_max_stack_size()
            || (other_stack.item_count < self_stack.item_count) != target_is_self
        {
            return;
        }
        let (mut stack1, mut stack2) = if target_is_self {
            (self_stack, other_stack)
        } else {
            (other_stack, self_stack)
        };

        // 原版代码加了 .min(64)。使用原版物品数据时不需要

        let max_size = stack1.get_max_stack_size();

        let j = stack2.item_count.min(max_size - stack1.item_count);

        stack1.increment(j);

        stack2.decrement(j);

        let empty1 = stack1.item_count == 0;

        let empty2 = stack2.item_count == 0;

        drop(stack1);

        drop(stack2);

        let never_despawn = source.never_despawn.load(Ordering::Relaxed);

        target.never_despawn.store(never_despawn, Ordering::Relaxed);

        if !never_despawn {
            let age = target
                .item_age
                .load(Ordering::Relaxed)
                .min(source.item_age.load(Ordering::Relaxed));

            target.item_age.store(age, Ordering::Relaxed);
        }

        let never_pickup = source.never_pickup.load(Ordering::Relaxed);

        target.never_pickup.store(never_pickup, Ordering::Relaxed);

        if !never_pickup {
            let source_delay = source.pickup_delay.load(Ordering::Relaxed);
            target
                .pickup_delay
                .fetch_max(source_delay, Ordering::Relaxed);
        }

        if empty1 {
            target.entity.remove();
        } else {
            target.init_data_tracker();
        }

        if empty2 {
            source.entity.remove();
        } else {
            source.init_data_tracker();
        }
    }

    fn decrement_pickup_delay(&self) {
        self.pickup_delay
            .try_update(Ordering::Relaxed, Ordering::Relaxed, |val| {
                Some(val.saturating_sub(1))
            })
            .ok();
    }

    fn apply_fluid_drag_or_gravity(&self, mut velo: Vector3<f64>) -> Vector3<f64> {
        let entity = &self.entity;

        if entity.touching_water.load(Ordering::SeqCst) && entity.water_height.load() > 0.1 {
            velo.x *= 0.99;
            velo.z *= 0.99;
            if velo.y < 0.06 {
                velo.y += 5.0e-4;
            }
        } else if entity.touching_lava.load(Ordering::SeqCst) && entity.lava_height.load() > 0.1 {
            velo.x *= 0.95;
            velo.z *= 0.95;
            if velo.y < 0.06 {
                velo.y += 5.0e-4;
            }
        } else {
            velo.y -= <Self as EntityBase>::get_gravity(self);
        }

        velo
    }

    fn update_no_physics_and_push_out(&self) {
        let entity = &self.entity;
        let pos = entity.pos.load();
        let bounding_box = entity.bounding_box.load();

        let no_physics = !entity
            .world
            .load()
            .is_space_empty(bounding_box.expand(-1.0e-7, -1.0e-7, -1.0e-7));

        entity.no_physics.store(no_physics, Ordering::Relaxed);

        if no_physics {
            entity.push_out_of_blocks(Vector3::new(
                pos.x,
                f64::midpoint(bounding_box.min.y, bounding_box.max.y),
                pos.z,
            ));
        }
    }

    fn should_tick_move(&self, move_velo: Vector3<f64>) -> Option<bool> {
        let entity = &self.entity;

        let mut tick_move = !entity.on_ground.load(Ordering::SeqCst)
            || move_velo.horizontal_length_squared() > 1.0e-5;

        if !tick_move {
            let Ok(item_age) = i32::try_from(self.item_age.load(Ordering::Relaxed)) else {
                entity.remove();
                return None;
            };

            tick_move = (item_age + entity.entity_id) % 4 == 0;
        }

        Some(tick_move)
    }

    fn move_and_apply_friction(&self, caller: &dyn EntityBase, move_velo: Vector3<f64>) {
        let entity = &self.entity;

        entity.move_entity(caller, move_velo);
        entity.tick_block_collisions(caller);

        let mut friction = 0.98;
        let on_ground = entity.on_ground.load(Ordering::SeqCst);

        let mut velo = entity.velocity.load();
        if on_ground {
            let block_affecting_velo = entity.get_block_with_y_offset(0.999_999).1;
            friction *= f64::from(block_affecting_velo.slipperiness) * 0.98;
        }

        velo = velo.multiply(friction, 0.98, friction);

        if on_ground && velo.y < 0.0 {
            velo.y = 0.0;
        }

        entity.velocity.store(velo);
    }

    fn process_age_and_merge(&self) -> bool {
        if self.never_despawn.load(Ordering::Relaxed) {
            return true;
        }

        let entity = &self.entity;
        let age = self.item_age.fetch_add(1, Ordering::Relaxed) + 1;

        if age >= 6000 {
            let entity_id = entity.entity_id;
            let world = entity.world.load_full();
            let mut despawn_event =
                crate::plugin::api::events::entity::item_despawn::ItemDespawnEvent::new(entity_id);
            if let Some(server) = world.server.upgrade() {
                server
                    .plugin_manager
                    .fire_blocking(&server, &mut despawn_event);
            }
            if !despawn_event.cancelled
                && let Some(e) = world.get_entity_by_id(entity_id)
            {
                e.get_entity().remove();
            }
            return false;
        }

        let n = if entity
            .last_pos
            .load()
            .sub(&entity.pos.load())
            .length_squared()
            == 0.0
        {
            40
        } else {
            2
        };

        if age.is_multiple_of(n) && self.can_merge() {
            self.try_merge();
        }

        true
    }

    fn sync_motion_if_dirty(&self, caller: &dyn EntityBase, original_velo: Vector3<f64>) {
        let entity = &self.entity;

        entity.update_fluid_state(caller);

        let velocity_dirty = entity.velocity_dirty.swap(false, Ordering::SeqCst)
            || entity.touching_water.load(Ordering::SeqCst)
            || entity.touching_lava.load(Ordering::SeqCst)
            || entity.velocity.load().sub(&original_velo).length_squared() > 0.1;
        let moved = entity.pos.load() != entity.last_sent_pos.load();
        let position_dirty = moved
            && self
                .item_age
                .load(Ordering::Relaxed)
                .is_multiple_of(ITEM_UPDATE_INTERVAL);

        if position_dirty || velocity_dirty {
            entity.send_pos_rot();
        }
        if velocity_dirty {
            entity.send_velocity();
        }
    }
}

impl EntityBase for ItemEntity {
    fn tick(&self, caller: &dyn EntityBase, _server: &Server) {
        let entity = &self.entity;
        self.decrement_pickup_delay();

        let original_velo = entity.velocity.load();
        entity
            .velocity
            .store(self.apply_fluid_drag_or_gravity(original_velo));

        self.update_no_physics_and_push_out();

        let move_velo = entity.velocity.load(); // 以防 push_out_of_blocks 修改了它

        let Some(tick_move) = self.should_tick_move(move_velo) else {
            return;
        };

        if tick_move {
            self.move_and_apply_friction(caller, move_velo);
        }

        if self.process_age_and_merge() {
            self.sync_motion_if_dirty(caller, original_velo);
        }
    }

    fn init_data_tracker(&self) {
        self.entity.set_synced_data(
            papokin_data::tracked_data::item::ITEM,
            ItemStackSerializer::from(
                self.item_stack
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .clone(),
            ),
        );
    }

    fn damage_with_context(
        &self,
        _caller: &dyn EntityBase,
        amount: f32,
        damage_type: DamageType,
        _position: Option<Vector3<f64>>,
        _source: Option<&dyn EntityBase>,
        _cause: Option<&dyn EntityBase>,
    ) -> bool {
        // 检查实体是否 fire_immune
        let is_fire_damage = damage_type == DamageType::IN_FIRE
            || damage_type == DamageType::ON_FIRE
            || damage_type == DamageType::LAVA;
        if is_fire_damage && self.entity.fire_immune.load(Ordering::Relaxed) {
            return false;
        }

        loop {
            let current = self.health.load(Relaxed);
            let new = current - amount;
            if self
                .health
                .compare_exchange(current, new, AcqRel, Relaxed)
                .is_ok()
            {
                if new <= 0.0 {
                    self.entity.remove();
                }
                return true;
            }
        }
    }

    fn on_player_collision(&self, player: &Arc<Player>) {
        if self.pickup_delay.load(Ordering::Relaxed) > 0
            || player.living_entity.health.load() <= 0.0
            || player.is_spectator()
        {
            return;
        }

        let (item_id, count_before) = {
            let stack = self
                .item_stack
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            (stack.item.id, stack.item_count)
        };

        let mut local_stack = self
            .item_stack
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone();
        let inserted = player.inventory.insert_stack_anywhere(&mut local_stack);
        let count_after = local_stack.item_count;
        let is_empty = local_stack.is_empty();
        *self
            .item_stack
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = local_stack;

        if inserted || player.is_creative() {
            player.inventory_changed.store(true, Ordering::Relaxed);

            let amount_picked_up = if player.is_creative() {
                count_before
            } else {
                count_before - count_after
            };

            if amount_picked_up > 0 {
                player.increment_stat(
                    StatisticCategory::PickedUp,
                    item_id as i32,
                    amount_picked_up as i32,
                );
            }

            player
                .living_entity
                .pickup(&self.entity, amount_picked_up.into());

            player
                .current_screen_handler
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .send_content_updates();

            if is_empty {
                self.entity.remove();
            } else {
                self.init_data_tracker();
            }
        }
    }

    fn get_entity(&self) -> &Entity {
        &self.entity
    }

    fn get_living_entity(&self) -> Option<&LivingEntity> {
        None
    }

    fn get_item_entity(&self) -> Option<&ItemEntity> {
        Some(self)
    }

    fn get_gravity(&self) -> f64 {
        0.04
    }

    fn write_custom_nbt(&self, nbt: &mut NbtCompound) {
        let item = self
            .item_stack
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let mut item_compound = NbtCompound::new();
        item.write_item_stack(&mut item_compound);
        nbt.put_compound("Item", item_compound);

        nbt.put_short("Age", self.item_age.load(Ordering::Relaxed) as i16);
        nbt.put_short(
            "PickupDelay",
            self.pickup_delay.load(Ordering::Relaxed) as i16,
        );
        nbt.put_short("Health", self.health.load(Relaxed) as i16);
    }

    fn read_custom_nbt(&self, nbt: &NbtCompound) {
        // 从 "Item" 复合标签恢复物品堆
        if let Some(item_compound) = nbt.get_compound("Item")
            && let Some(stack) = ItemStack::read_item_stack(item_compound)
        {
            *self
                .item_stack
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner) = stack;
        }

        // 原版将 Age 存储为 short
        self.item_age
            .store(nbt.get_short("Age").unwrap_or(0) as u32, Ordering::Relaxed);

        // 原版将 PickupDelay 存储为 short
        if let Some(delay) = nbt.get_short("PickupDelay") {
            self.pickup_delay.store(delay as u8, Ordering::Relaxed);
        }

        // 原版将 Health 存储为 short
        if let Some(health) = nbt.get_short("Health") {
            self.health.store(health as f32, Relaxed);
        }
    }

    fn cast_any(&self) -> &dyn std::any::Any {
        self
    }

    fn send_java_spawn_packet(&self, client: &crate::net::java::JavaClient) {
        let spawn_packet = self.entity.create_spawn_packet();
        if let Ok(data) = client.serialize_packet(&spawn_packet) {
            client.try_enqueue_packet(data);
        }

        if client.version.load() >= JavaMinecraftVersion::V_1_21 {
            let metadata = Metadata::new(
                papokin_data::tracked_data::item::ITEM,
                ItemStackSerializer::from(
                    self.item_stack
                        .lock()
                        .unwrap_or_else(std::sync::PoisonError::into_inner)
                        .clone(),
                ),
            );
            let mut data = Vec::new();
            if metadata.write(&mut data, &client.version.load()).is_ok() {
                data.push(255);
                let meta_packet =
                    CSetEntityMetadata::new(self.entity.entity_id.into(), data.into());
                if let Ok(meta_data) = client.serialize_packet(&meta_packet) {
                    client.try_enqueue_packet(meta_data);
                }
            }
        }
    }
}
