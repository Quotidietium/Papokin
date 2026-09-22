/// 为箱式方块实体实现 `BlockEntity` trait。
/// 参数：
/// - $`struct_name`: 箱子结构体的类型（例如 `ChestBlockEntity`）
/// - $`resource_id`: 资源位置字符串（例如 "minecraft:chest"）
#[macro_export]
macro_rules! impl_block_entity_for_chest {
    ($struct_name:ty) => {
        impl $crate::block::entities::BlockEntity for $struct_name {
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
                // 先读取延迟的战利品表字段。
                let loot_table_key = nbt.get_string("LootTable").map(|s| s.to_string());
                let loot_table_seed = nbt.get_long("LootTableSeed").unwrap_or(0);

                let mut chest = Self {
                    position,
                    items: std::sync::RwLock::new(std::array::from_fn(|_| {
                        ItemStack::EMPTY.clone()
                    })),
                    dirty: std::sync::atomic::AtomicBool::new(false),
                    comparator_dirty: std::sync::atomic::AtomicBool::new(false),
                    viewers: $crate::block::viewer::ViewerCountTracker::new(),
                    loot_table: StdMutex::new(loot_table_key),
                    loot_table_seed,
                };

                // 仅在没有待处理的战利品表时才读取已保存的物品。
                let has_loot_table = chest
                    .loot_table
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .is_some();
                if !has_loot_table {
                    papokin_inventory::sync_read_items_from_nbt(
                        nbt,
                        chest
                            .items
                            .get_mut()
                            .unwrap_or_else(std::sync::PoisonError::into_inner),
                    );
                }

                chest
            }

            fn write_nbt(&self, nbt: &mut papokin_nbt::compound::NbtCompound) {
                use papokin_inventory::Inventory;

                let loot_table_key = {
                    let guard = self
                        .loot_table
                        .lock()
                        .unwrap_or_else(std::sync::PoisonError::into_inner);
                    guard.clone()
                };

                if let Some(key) = loot_table_key {
                    // 持久化延迟战利品：写入键与种子；跳过物品。
                    nbt.put_string("LootTable", key);
                    if self.loot_table_seed != 0 {
                        nbt.put_long("LootTableSeed", self.loot_table_seed);
                    }
                } else {
                    // 战利品已生成，因此持久化实际物品。
                    self.write_inventory_nbt(nbt, true);
                }
            }

            fn tick(&self, world: &Arc<$crate::world::World>) {
                $crate::block::viewer::ViewerCountTrackerExt::update_viewer_count::<$struct_name>(
                    &self.viewers,
                    self,
                    world,
                    &self.position,
                );
            }

            fn get_inventory(self: Arc<Self>) -> Option<Arc<dyn papokin_inventory::Inventory>> {
                Some(self)
            }

            fn is_comparator_dirty(&self) -> bool {
                self.comparator_dirty
                    .load(std::sync::atomic::Ordering::Relaxed)
            }

            fn clear_comparator_dirty(&self) {
                self.comparator_dirty
                    .store(false, std::sync::atomic::Ordering::Relaxed);
            }

            fn is_dirty(&self) -> bool {
                self.dirty.load(std::sync::atomic::Ordering::Relaxed)
            }

            fn clear_dirty(&self) {
                self.dirty
                    .store(false, std::sync::atomic::Ordering::Relaxed);
            }

            fn chunk_data_nbt(&self) -> Option<papokin_nbt::compound::NbtCompound> {
                let mut nbt = papokin_nbt::compound::NbtCompound::new();
                let has_loot_table = self
                    .loot_table
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .is_some();
                if !has_loot_table {
                    if let Ok(items) = self.items.try_read() {
                        papokin_inventory::sync_write_items_to_nbt(&*items, &mut nbt);
                    }
                }
                Some(nbt)
            }

            fn as_any(&self) -> &dyn std::any::Any {
                self
            }

            fn take_loot_table(&self) -> Option<(String, i64)> {
                let mut guard = self
                    .loot_table
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                guard.take().map(|key| (key, self.loot_table_seed))
            }

            fn has_loot_table(&self) -> bool {
                self.loot_table
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .is_some()
            }
        }
    };
}

/// 为箱式方块实体实现 Inventory trait。
#[macro_export]
macro_rules! impl_inventory_for_chest {
    ($struct_name:ty) => {
        impl papokin_inventory::Inventory for $struct_name {
            fn size(&self) -> usize {
                Self::INVENTORY_SIZE
            }

            fn is_empty(&self) -> bool {
                let items = self
                    .items
                    .read()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                items.iter().all(|s| s.is_empty())
            }

            fn get_stack(&self, slot: usize) -> ItemStack {
                let items = self
                    .items
                    .read()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                items[slot].clone()
            }

            fn remove_stack(&self, slot: usize) -> ItemStack {
                let mut items = self
                    .items
                    .write()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                let removed = std::mem::replace(&mut items[slot], ItemStack::EMPTY.clone());
                self.mark_dirty();
                removed
            }

            fn remove_stack_specific(&self, slot: usize, amount: u8) -> ItemStack {
                let mut items = self
                    .items
                    .write()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                let res = if !items[slot].is_empty() && amount > 0 {
                    items[slot].split(amount)
                } else {
                    ItemStack::EMPTY.clone()
                };
                self.mark_dirty();
                res
            }

            fn set_stack(&self, slot: usize, stack: ItemStack) {
                let mut items = self
                    .items
                    .write()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                items[slot] = stack;
                self.mark_dirty();
            }

            fn on_open(&self) {
                self.viewers.open_container();
            }

            fn on_close(&self) {
                self.viewers.close_container();
            }

            fn mark_dirty(&self) {
                self.dirty.store(true, std::sync::atomic::Ordering::Relaxed);
                self.comparator_dirty
                    .store(true, std::sync::atomic::Ordering::Relaxed);
            }

            fn as_any(&self) -> &dyn std::any::Any {
                self
            }
        }
    };
}

/// 为箱式方块实体实现 Clearable trait。
#[macro_export]
macro_rules! impl_clearable_for_chest {
    ($struct_name:ty) => {
        impl papokin_inventory::Clearable for $struct_name {
            fn clear(&self) {
                let mut items = self
                    .items
                    .write()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                items.fill_with(|| ItemStack::EMPTY.clone());
                <$struct_name as papokin_inventory::Inventory>::mark_dirty(self);
            }
        }
    };
}

/// 为箱式方块实体实现 `ViewerCountListener` trait。
///
/// 该行为由结构体上的 `EMITS_REDSTONE` 常量控制。
/// 当 `EMITS_REDSTONE` 为 true 时，观察者数量变化时更新邻居以传递红石信号。
#[macro_export]
macro_rules! impl_viewer_count_listener_for_chest {
    ($struct_name:ty) => {
        impl $crate::block::viewer::ViewerCountListener for $struct_name {
            fn on_container_open(
                &self,
                world: &Arc<$crate::world::World>,
                _position: &papokin_util::math::position::BlockPos,
            ) {
                self.play_sound(world, papokin_data::sound::Sound::BlockChestOpen);
            }

            fn on_container_close(
                &self,
                world: &Arc<$crate::world::World>,
                _position: &papokin_util::math::position::BlockPos,
            ) {
                self.play_sound(world, papokin_data::sound::Sound::BlockChestClose);
            }

            fn on_viewer_count_update(
                &self,
                world: &Arc<$crate::world::World>,
                position: &papokin_util::math::position::BlockPos,
                old: u16,
                new: u16,
            ) {
                // 触发方块动画
                world.add_synced_block_event(*position, Self::LID_ANIMATION_EVENT_TYPE, new as u8);

                // 观看者数量变化时更新红石信号的相邻方块
                // 这由结构体上的 EMITS_REDSTONE 常量控制
                if Self::EMITS_REDSTONE && old != new {
                    // 更新直接相邻的方块
                    world.update_neighbors(position, None);

                    // 同时更新下方方块的邻块（强充能方块）
                    // 这确保下方方块相邻的红石元件会收到通知
                    let below_pos = position.down();
                    world.update_neighbors(&below_pos, None);
                }
            }
        }
    };
}

/// 为箱式方块实体实现辅助方法。
///
/// 包含 `play_sound` 方法，用于处理单箱与双箱的声音定位，
/// 以及 `new()` 和 `get_viewer_count()` 方法。
#[macro_export]
macro_rules! impl_chest_helper_methods {
    ($struct_name:ty) => {
        impl $struct_name {
            /// 返回当前正在查看此箱子的玩家数量
            pub fn get_viewer_count(&self) -> u16 {
                self.viewers.get_viewer_count()
            }

            #[must_use]
            pub fn new(position: papokin_util::math::position::BlockPos) -> Self {
                use std::array::from_fn;
                use std::sync::Mutex as StdMutex;
                use std::sync::RwLock;
                use std::sync::atomic::AtomicBool;

                Self {
                    position,
                    items: RwLock::new(from_fn(|_| ItemStack::EMPTY.clone())),
                    dirty: AtomicBool::new(false),
                    comparator_dirty: AtomicBool::new(false),
                    viewers: $crate::block::viewer::ViewerCountTracker::new(),
                    loot_table: StdMutex::new(None),
                    loot_table_seed: 0,
                }
            }

            fn play_sound(
                &self,
                world: &Arc<$crate::world::World>,
                sound: papokin_data::sound::Sound,
            ) {
                let mut rng = papokin_util::random::xoroshiro128::Xoroshiro::from_seed(
                    papokin_util::random::get_seed(),
                );

                let state = world.get_block_state(&self.position);
                let properties =
                    papokin_data::block_properties::ChestLikeProperties::from_state_id(state.id);
                let position = match properties.r#type {
                    papokin_data::block_properties::ChestType::Left => return,
                    papokin_data::block_properties::ChestType::Single => {
                        papokin_util::math::vector3::Vector3::new(
                            self.position.0.x as f64 + 0.5,
                            self.position.0.y as f64 + 0.5,
                            self.position.0.z as f64 + 0.5,
                        )
                    }
                    papokin_data::block_properties::ChestType::Right => {
                        let direction = papokin_data::HorizontalFacingExt::to_block_direction(
                            &properties.facing,
                        )
                        .to_offset();
                        papokin_util::math::vector3::Vector3::new(
                            self.position.0.x as f64 + 0.5 + direction.x as f64 * 0.5,
                            self.position.0.y as f64 + 0.5,
                            self.position.0.z as f64 + 0.5 + direction.z as f64 * 0.5,
                        )
                    }
                };

                world.play_sound_fine(
                    sound,
                    papokin_data::sound::SoundCategory::Blocks,
                    &position,
                    0.5,
                    papokin_util::random::RandomImpl::next_f32(&mut rng) * 0.1 + 0.9,
                );
            }
        }
    };
}
