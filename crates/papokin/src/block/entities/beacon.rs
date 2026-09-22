use papokin_data::data_component_impl::IDSetContent;
use papokin_data::tag::Taggable;
use std::any::Any;
use std::sync::atomic::{AtomicBool, AtomicI32, Ordering};
use std::sync::{Arc, Mutex};

use papokin_data::effect::StatusEffect;
use papokin_data::item_stack::ItemStack;
use papokin_nbt::compound::NbtCompound;
use papokin_util::math::boundingbox::BoundingBox;
use papokin_util::math::position::BlockPos;

use crate::block::entities::BlockEntity;
use crate::entity::player::Player;
use crate::plugin::api::events::block::beacon_activated::BeaconActivatedEvent;
use crate::plugin::api::events::block::beacon_deactivated::BeaconDeactivatedEvent;
use crate::plugin::api::events::block::beacon_effect::BeaconEffectEvent;
use crate::world::World;
use papokin_inventory::{Clearable, Inventory};

pub struct BeaconBlockEntity {
    pub position: BlockPos,
    pub primary_effect: AtomicI32,
    pub secondary_effect: AtomicI32,
    pub levels: AtomicI32,
    pub dirty: AtomicBool,
    pub payment: Arc<Mutex<ItemStack>>,

    // 原版对齐字段
    pub custom_name: Mutex<Option<String>>,
    pub lock_key: Mutex<Option<String>>,
    pub last_check_y: AtomicI32,
}

impl BeaconBlockEntity {
    pub const ID: &'static str = "minecraft:beacon";

    // ContainerData 属性常量
    pub const DATA_LEVELS: usize = 0;
    pub const DATA_PRIMARY: usize = 1;
    pub const DATA_SECONDARY: usize = 2;
    pub const NUM_DATA_VALUES: usize = 3;

    #[must_use]
    pub fn new(position: BlockPos) -> Self {
        Self {
            position,
            primary_effect: AtomicI32::new(-1),
            secondary_effect: AtomicI32::new(-1),
            levels: AtomicI32::new(0),
            dirty: AtomicBool::new(false),
            payment: Arc::new(Mutex::new(ItemStack::EMPTY.clone())),
            custom_name: Mutex::new(None),
            lock_key: Mutex::new(None),
            last_check_y: AtomicI32::new(position.0.y - 1),
        }
    }

    /// 复刻 Java 的 `ContainerData`，用于向 `BeaconMenu` 同步数值
    pub fn get_data(&self, id: usize) -> i32 {
        match id {
            Self::DATA_LEVELS => self.levels.load(Ordering::Relaxed),
            Self::DATA_PRIMARY => self.primary_effect.load(Ordering::Relaxed),
            Self::DATA_SECONDARY => self.secondary_effect.load(Ordering::Relaxed),
            _ => 0,
        }
    }

    pub fn set_data(&self, id: usize, value: i32) {
        match id {
            Self::DATA_LEVELS => self.levels.store(value, Ordering::Relaxed),
            Self::DATA_PRIMARY => self.primary_effect.store(value, Ordering::Relaxed),
            Self::DATA_SECONDARY => self.secondary_effect.store(value, Ordering::Relaxed),
            _ => {}
        }
        self.mark_dirty();
    }

    #[must_use]
    pub const fn is_valid_primary_effect(effect_id: i32, levels: i32) -> bool {
        match effect_id {
            // 迅捷（1）、急迫（3）
            1 | 3 => levels >= 1,
            // 抗性提升（11）、跳跃提升（8）
            11 | 8 => levels >= 2,
            // 力量 (5)
            5 => levels >= 3,
            _ => false,
        }
    }

    #[must_use]
    pub const fn is_valid_secondary_effect(
        primary_id: i32,
        secondary_id: i32,
        levels: i32,
    ) -> bool {
        if secondary_id <= 0 {
            return true;
        }
        if levels < 4 {
            return false;
        }
        // 生命恢复（10）或与主效果相同
        secondary_id == 10 || secondary_id == primary_id
    }

    #[must_use]
    pub fn validate_effects(primary: Option<i32>, secondary: Option<i32>, levels: i32) -> bool {
        let primary_id = primary.unwrap_or(0);
        let secondary_id = secondary.unwrap_or(0);

        if primary_id > 0 && !Self::is_valid_primary_effect(primary_id, levels) {
            return false;
        }

        if secondary_id > 0 && !Self::is_valid_secondary_effect(primary_id, secondary_id, levels) {
            return false;
        }

        true
    }

    pub fn update_base(&self, world: &Arc<World>) -> i32 {
        let x = self.position.0.x;
        let y = self.position.0.y;
        let z = self.position.0.z;

        let mut current_level = 0;

        for level in 1..=4 {
            let layer_y = y - level;
            if layer_y < world.dimension.min_y {
                break;
            }

            let mut layer_valid = true;
            for dx in -level..=level {
                for dz in -level..=level {
                    let block_pos = BlockPos::new(x + dx, layer_y, z + dz);
                    let state = world.get_block_state(&block_pos);
                    let block = world.get_block(&block_pos);

                    if !block.has_tag(&papokin_data::tag::Block::MINECRAFT_BEACON_BASE_BLOCKS) {
                        layer_valid = false;
                        break;
                    }

                    // 可选：可以在此处进行更严格的方块类型校验
                    let _ = state;
                }
                if !layer_valid {
                    break;
                }
            }

            if layer_valid {
                current_level = level;
            } else {
                break;
            }
        }

        current_level
    }

    pub fn apply_effects(&self, world: &Arc<World>, levels: i32) {
        let primary_id = self.primary_effect.load(Ordering::Relaxed);
        let secondary_id = self.secondary_effect.load(Ordering::Relaxed);

        if primary_id <= 0 {
            return;
        }

        let primary_effect = StatusEffect::from_id(primary_id as u16);
        let secondary_effect = StatusEffect::from_id(secondary_id as u16);

        // 原版时长：(9 + levels * 2) * 20 刻
        let duration_ticks = (9 + levels * 2) * 20;

        // 基础增幅：若辅助效果与主效果一致，主效果获得增幅 1（II 级）
        let base_amp = i32::from(levels >= 4 && primary_id == secondary_id);

        // 原版范围为每个水平方向 level * 10 + 10 个方块
        let range = f64::from(levels * 10 + 10);
        let pos = self.position.0.to_f64();
        let box_min = [pos.x - range, pos.y - range, pos.z - range];
        let box_max = [
            pos.x + range + 1.0,
            pos.y + range + 1.0 + 384.0,
            pos.z + range + 1.0,
        ];
        let bounds = BoundingBox::new_array(box_min, box_max);

        let server = world.server.upgrade();

        // 将效果应用于范围内所有玩家
        let players = world.players.load();
        for player in players.iter() {
            if !bounds.intersects(&player.living_entity.entity.bounding_box.load()) {
                continue;
            }

            if let Some(effect) = primary_effect {
                if let Some(server) = &server {
                    let mut event = BeaconEffectEvent::new(
                        player.clone(),
                        effect.minecraft_name.to_string(),
                        true,
                        self.position,
                    );
                    server.plugin_manager.fire_blocking(server, &mut event);
                }
                player.add_effect(papokin_data::potion::Effect {
                    effect_type: effect,
                    duration: duration_ticks,
                    amplifier: base_amp as u8,
                    ambient: true,
                    show_particles: true,
                    show_icon: true,
                    blend: false,
                });
            }

            if levels >= 4
                && primary_id != secondary_id
                && let Some(effect) = secondary_effect
            {
                if let Some(server) = &server {
                    let mut event = BeaconEffectEvent::new(
                        player.clone(),
                        effect.minecraft_name.to_string(),
                        false,
                        self.position,
                    );
                    server.plugin_manager.fire_blocking(server, &mut event);
                }
                player.add_effect(papokin_data::potion::Effect {
                    effect_type: effect,
                    duration: duration_ticks,
                    amplifier: 0,
                    ambient: true,
                    show_particles: true,
                    show_icon: true,
                    blend: false,
                });
            }
        }
    }

    /// `levels` 等级下信标效果范围内的最近玩家，用于
    /// 用于关联信标的激活/停用事件。
    fn nearest_player_in_range(&self, world: &Arc<World>, levels: i32) -> Option<Arc<Player>> {
        let range = f64::from(levels.max(1) * 10 + 10);
        let pos = self.position.0.to_f64();
        let bounds = BoundingBox::new_array(
            [pos.x - range, pos.y - range, pos.z - range],
            [
                pos.x + range + 1.0,
                pos.y + range + 1.0 + 384.0,
                pos.z + range + 1.0,
            ],
        );

        let players = world.players.load();
        let mut nearest: Option<(f64, Arc<Player>)> = None;
        for player in players.iter() {
            if !bounds.intersects(&player.living_entity.entity.bounding_box.load()) {
                continue;
            }
            let distance = player
                .living_entity
                .entity
                .pos
                .load()
                .squared_distance_to_vec(&pos);
            let closer = nearest.as_ref().is_none_or(|(best, _)| distance < *best);
            if closer {
                nearest = Some((distance, player.clone()));
            }
        }
        nearest.map(|(_, player)| player)
    }
}

impl BlockEntity for BeaconBlockEntity {
    fn resource_location(&self) -> &'static str {
        Self::ID
    }

    fn get_position(&self) -> BlockPos {
        self.position
    }

    fn from_nbt(nbt: &papokin_nbt::compound::NbtCompound, position: BlockPos) -> Self
    where
        Self: Sized,
    {
        let primary = nbt
            .get_string("primary_effect")
            .and_then(|s| {
                StatusEffect::from_minecraft_name(s)
                    .or_else(|| StatusEffect::from_name(s))
                    .map(|e| e.id as i32)
            })
            .or_else(|| nbt.get_int("primary_effect"))
            .unwrap_or(-1);
        let secondary = nbt
            .get_string("secondary_effect")
            .and_then(|s| {
                StatusEffect::from_minecraft_name(s)
                    .or_else(|| StatusEffect::from_name(s))
                    .map(|e| e.id as i32)
            })
            .or_else(|| nbt.get_int("secondary_effect"))
            .unwrap_or(-1);
        let levels = nbt.get_int("Levels").unwrap_or(0);
        let custom_name = nbt
            .get_string("CustomName")
            .or_else(|| nbt.get_string("custom_name"))
            .map(std::string::ToString::to_string);
        let lock_key = nbt.get_string("Lock").map(std::string::ToString::to_string);

        Self {
            position,
            primary_effect: AtomicI32::new(primary),
            secondary_effect: AtomicI32::new(secondary),
            levels: AtomicI32::new(levels),
            dirty: AtomicBool::new(false),
            payment: Arc::new(Mutex::new(ItemStack::EMPTY.clone())),
            custom_name: Mutex::new(custom_name),
            lock_key: Mutex::new(lock_key),
            last_check_y: AtomicI32::new(position.0.y - 1),
        }
    }

    fn write_nbt(&self, nbt: &mut NbtCompound) {
        let primary = self.primary_effect.load(Ordering::Relaxed);
        if primary >= 0 {
            if let Some(eff) = <StatusEffect as IDSetContent>::from_id(primary as u16) {
                nbt.put_string("primary_effect", eff.minecraft_name.to_string());
            } else {
                nbt.put_int("primary_effect", primary);
            }
        }
        let secondary = self.secondary_effect.load(Ordering::Relaxed);
        if secondary >= 0 {
            if let Some(eff) = <StatusEffect as IDSetContent>::from_id(secondary as u16) {
                nbt.put_string("secondary_effect", eff.minecraft_name.to_string());
            } else {
                nbt.put_int("secondary_effect", secondary);
            }
        }
        nbt.put_int("Levels", self.levels.load(Ordering::Relaxed));

        if let Some(name) = &*self
            .custom_name
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
        {
            nbt.put_string("CustomName", name.clone());
        }
        if let Some(lock) = &*self
            .lock_key
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
        {
            nbt.put_string("Lock", lock.clone());
        }
    }

    fn tick(&self, world: &Arc<World>) {
        // 每 80 刻检查一次属性，与 Java 版一致
        if world.get_time_of_day() % 80 == 0 {
            let previous_levels = self.levels.load(Ordering::Relaxed);
            let levels = self.update_base(world);
            self.levels.store(levels, Ordering::Relaxed);

            // 在信标等级变化时通知插件信标（取消）激活
            if previous_levels == 0 && levels > 0 {
                if let Some(server) = world.server.upgrade()
                    && let Some(player) = self.nearest_player_in_range(world, levels)
                {
                    let mut event = BeaconActivatedEvent::new(player, self.position);
                    server.plugin_manager.fire_blocking(&server, &mut event);
                }
            } else if previous_levels > 0
                && levels == 0
                && let Some(server) = world.server.upgrade()
            {
                let player = self.nearest_player_in_range(world, previous_levels);
                let mut event = BeaconDeactivatedEvent::new(player, self.position);
                server.plugin_manager.fire_blocking(&server, &mut event);
            }

            if levels > 0 {
                self.apply_effects(world, levels);
            }
        }
    }

    fn chunk_data_nbt(&self) -> Option<NbtCompound> {
        let mut nbt = NbtCompound::new();
        let primary = self.primary_effect.load(Ordering::Relaxed);
        if primary >= 0 {
            if let Some(eff) = <StatusEffect as IDSetContent>::from_id(primary as u16) {
                nbt.put_string("primary_effect", eff.minecraft_name.to_string());
            } else {
                nbt.put_int("primary_effect", primary);
            }
        }
        let secondary = self.secondary_effect.load(Ordering::Relaxed);
        if secondary >= 0 {
            if let Some(eff) = <StatusEffect as IDSetContent>::from_id(secondary as u16) {
                nbt.put_string("secondary_effect", eff.minecraft_name.to_string());
            } else {
                nbt.put_int("secondary_effect", secondary);
            }
        }
        nbt.put_int("Levels", self.levels.load(Ordering::Relaxed));
        if let Ok(name) = self.custom_name.try_lock()
            && let Some(ref name) = *name
        {
            nbt.put_string("CustomName", name.clone());
        }
        if let Ok(lock) = self.lock_key.try_lock()
            && let Some(ref lock) = *lock
        {
            nbt.put_string("Lock", lock.clone());
        }
        Some(nbt)
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

impl Inventory for BeaconBlockEntity {
    fn size(&self) -> usize {
        1
    }

    fn is_empty(&self) -> bool {
        self.payment
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .is_empty()
    }

    fn get_stack(&self, slot: usize) -> ItemStack {
        if slot == 0 {
            self.payment
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .clone()
        } else {
            ItemStack::EMPTY.clone()
        }
    }

    fn remove_stack(&self, slot: usize) -> ItemStack {
        if slot == 0 {
            let mut removed = ItemStack::EMPTY.clone();
            let mut guard = self
                .payment
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            std::mem::swap(&mut removed, &mut *guard);
            self.mark_dirty();
            removed
        } else {
            ItemStack::EMPTY.clone()
        }
    }

    fn remove_stack_specific(&self, slot: usize, amount: u8) -> ItemStack {
        if slot == 0 {
            let mut stack = self
                .payment
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            if stack.is_empty() {
                return ItemStack::EMPTY.clone();
            }
            let res = stack.split(amount);
            self.mark_dirty();
            res
        } else {
            ItemStack::EMPTY.clone()
        }
    }

    fn set_stack(&self, slot: usize, stack: ItemStack) {
        if slot == 0 {
            *self
                .payment
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner) = stack;
            self.mark_dirty();
        }
    }

    fn mark_dirty(&self) {
        self.dirty.store(true, Ordering::Relaxed);
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl Clearable for BeaconBlockEntity {
    fn clear(&self) {
        if let Ok(mut payment) = self.payment.try_lock() {
            *payment = ItemStack::EMPTY.clone();
        }
    }
}
