use crate::block::entities::BlockEntity;
use crate::entity::experience_orb::ExperienceOrbEntity;
use crate::world::World;
use papokin_data::block_properties::{FacingHopper, HopperLikeProperties};
use papokin_data::item_stack::ItemStack;
use papokin_data::tag;
use papokin_data::tag::Taggable;
use papokin_data::{BlockId, BlockStateId};
use papokin_inventory::{Clearable, Inventory, sync_write_items_to_nbt};
use papokin_nbt::compound::NbtCompound;
use papokin_nbt::tag::NbtTag;
use papokin_util::math::position::BlockPos;
use papokin_util::math::vector3::Vector3;
use std::any::Any;
use std::array::from_fn;
use std::sync::Arc;
use std::sync::RwLock;
use std::sync::atomic::Ordering;
use std::sync::atomic::{AtomicBool, AtomicI32, AtomicI64};

pub struct HopperBlockEntity {
    pub position: BlockPos,
    pub items: RwLock<[ItemStack; Self::INVENTORY_SIZE]>,
    pub dirty: AtomicBool,
    pub comparator_dirty: AtomicBool,
    pub facing: FacingHopper,
    pub cooldown_time: AtomicI32,
    pub ticked_game_time: AtomicI64,
}

#[must_use]
pub fn to_offset(facing: &FacingHopper) -> Vector3<i32> {
    match facing {
        FacingHopper::Down => (0, -1, 0),
        FacingHopper::North => (0, 0, -1),
        FacingHopper::South => (0, 0, 1),
        FacingHopper::West => (-1, 0, 0),
        FacingHopper::East => (1, 0, 0),
    }
    .into()
}

/// 一个状态快照的属性，其他任何方块则为 `None`。`from_state_id` 会解析
/// 无论传入什么状态它都接受，因此只有方块 id 能拒绝替换状态。
fn hopper_properties(block: BlockId, state_id: BlockStateId) -> Option<HopperLikeProperties> {
    (block == BlockId::HOPPER).then(|| HopperLikeProperties::from_state_id(state_id))
}

/// 从槽位取出的一件物品，附带移除前后槽位所持有的内容。
///
/// 一次比较并交换：offer 的执行不持有来源锁（若跨
/// [`HopperBlockEntity::add_one_item`] 会让相对放置的两个漏斗死锁），因此回滚
/// 必须与 `remainder` 比较，以区分未被触碰的槽位和他人的写入。
struct Extraction {
    one_item: ItemStack,
    snapshot: ItemStack,
    remainder: ItemStack,
}

impl BlockEntity for HopperBlockEntity {
    fn write_nbt(&self, nbt: &mut NbtCompound) {
        nbt.put(
            "TransferCooldown",
            NbtTag::Int(self.cooldown_time.load(Ordering::Relaxed)),
        );
        self.write_inventory_nbt(nbt, true);
    }

    fn from_nbt(nbt: &papokin_nbt::compound::NbtCompound, position: BlockPos) -> Self
    where
        Self: Sized,
    {
        let mut hopper = Self {
            position,
            items: RwLock::new(from_fn(|_| ItemStack::EMPTY.clone())),
            dirty: AtomicBool::new(false),
            comparator_dirty: AtomicBool::new(false),
            facing: FacingHopper::Down,
            cooldown_time: AtomicI32::from(nbt.get_int("TransferCooldown").unwrap_or(-1)),
            ticked_game_time: AtomicI64::new(0),
        };

        papokin_inventory::sync_read_items_from_nbt(
            nbt,
            hopper
                .items
                .get_mut()
                .unwrap_or_else(std::sync::PoisonError::into_inner),
        );

        hopper
    }

    fn tick(&self, world: &Arc<World>) {
        self.ticked_game_time
            .store(world.get_world_age(), Ordering::Relaxed);
        // 当另一个 Rayon 工作线程替换方块时，方块实体会比方块多存活一刻，
        // 因此要像 `trial_spawner.rs::tick` 那样加防护。对 id 和状态只取一次快照：第二次
        // 读取到的可能已是替换后的内容，两者将无法配对。
        let (block, state) = world.get_block_and_state(&self.position);
        let Some(properties) = hopper_properties(block.id, state.id) else {
            return;
        };
        if self.cooldown_time.fetch_sub(1, Ordering::Relaxed) <= 0 {
            self.cooldown_time.store(0, Ordering::Relaxed);
            if properties.enabled
                && let Some(entity) = world.get_block_entity(&self.position)
                && let Some(hopper) = entity.as_any().downcast_ref::<Self>()
            {
                hopper.try_move_items(properties, world);
            }
        }
    }

    fn resource_location(&self) -> &'static str {
        Self::ID
    }

    fn get_position(&self) -> BlockPos {
        self.position
    }

    fn get_inventory(self: Arc<Self>) -> Option<Arc<dyn Inventory>> {
        Some(self)
    }

    fn set_block_state(&mut self, block_state: BlockStateId) {
        // TODO !!!重要!!! 加载区块时设置方块状态
        self.facing = HopperLikeProperties::from_state_id(block_state).facing;
    }

    fn is_comparator_dirty(&self) -> bool {
        self.comparator_dirty.load(Ordering::Relaxed)
    }

    fn clear_comparator_dirty(&self) {
        self.comparator_dirty.store(false, Ordering::Relaxed);
    }

    fn is_dirty(&self) -> bool {
        self.dirty.load(Ordering::Relaxed)
    }

    fn clear_dirty(&self) {
        self.dirty.store(false, Ordering::Relaxed);
    }

    fn chunk_data_nbt(&self) -> Option<NbtCompound> {
        let mut nbt = NbtCompound::new();
        nbt.put(
            "TransferCooldown",
            NbtTag::Int(self.cooldown_time.load(Ordering::Relaxed)),
        );
        if let Ok(items) = self.items.try_read() {
            sync_write_items_to_nbt(items.as_slice(), &mut nbt);
        }
        Some(nbt)
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

impl HopperBlockEntity {
    pub const INVENTORY_SIZE: usize = 5;
    pub const ID: &'static str = "minecraft:hopper";

    #[must_use]
    pub fn new(position: BlockPos, facing: FacingHopper) -> Self {
        Self {
            position,
            items: RwLock::new(from_fn(|_| ItemStack::EMPTY.clone())),
            dirty: AtomicBool::new(false),
            comparator_dirty: AtomicBool::new(false),
            facing,
            cooldown_time: AtomicI32::new(-1),
            ticked_game_time: AtomicI64::new(0),
        }
    }
    fn try_move_items(&self, state: HopperLikeProperties, world: &Arc<World>) {
        if self.cooldown_time.load(Ordering::Relaxed) <= 0 && state.enabled {
            let mut success = if self.is_empty() {
                false
            } else {
                self.eject_items(world)
            };
            if !self.inventory_full() {
                success |= self.suck_in_items(world);
            }
            if success {
                self.cooldown_time.store(8, Ordering::Relaxed);
                self.mark_dirty();
            }
        }
    }

    fn inventory_full(&self) -> bool {
        let items = self
            .items
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        for item in items.iter() {
            if item.is_empty() || item.item_count != item.get_max_stack_size() {
                return false;
            }
        }
        true
    }

    #[allow(clippy::too_many_lines)]
    fn suck_in_items(&self, world: &Arc<World>) -> bool {
        // TODO getEntityContainer
        let pos_up = &self.position.up();
        let mut search_event = crate::plugin::api::events::inventory::hopper_inventory_search::HopperInventorySearchEvent::new(
            self.position,
            *pos_up,
        );
        if let Some(server) = world.server.upgrade() {
            server
                .plugin_manager
                .fire_blocking(&server, &mut search_event);
        }
        if search_event.cancelled {
            return false;
        }

        if let Some(entity) = world.get_block_entity(pos_up)
            && let Some(container) = entity.clone().get_inventory()
        {
            // TODO 检查 WorldlyContainer
            for i in 0..container.size() {
                let mut item = container.get_stack(i);
                if !item.is_empty() && container.can_transfer_to(self, i, &item) {
                    //TODO WorldlyContainer
                    let _backup = item.clone();
                    let one_item = item.split(1);
                    if Self::add_one_item(container.as_ref(), self, &one_item) {
                        container.set_stack(i, item);
                        // 如果从熔炉输出槽（索引 2）取出物品，则以经验球形式掉落经验
                        let furnace_output_slot: usize = 2;
                        if i == furnace_output_slot
                            && let Some(experience_container) =
                                entity.clone().to_experience_container()
                        {
                            let xp = experience_container.extract_experience();
                            if xp > 0 {
                                let pos = self.position.to_f64();
                                ExperienceOrbEntity::spawn(world, pos, xp as u32);
                            }
                        }
                        return true;
                    }
                }
            }
            return false;
        }
        let (block, state) = world.get_block_and_state(pos_up);
        if !(state.is_solid() && block.has_tag(&tag::Block::MINECRAFT_DOES_NOT_BLOCK_HOPPERS)) {
            let pos_up_f = pos_up.to_f64();
            let search_box = papokin_util::math::boundingbox::BoundingBox::new(
                pos_up_f,
                pos_up_f.add_raw(1.0, 1.0, 1.0),
            );
            let entities = world.get_entities_at_box(&search_box);
            for entity_base in entities {
                if let Some(item_entity) = entity_base.get_item_entity() {
                    let (is_empty, registry_key) = {
                        let stack = item_entity
                            .get_item_stack()
                            .lock()
                            .unwrap_or_else(std::sync::PoisonError::into_inner);
                        (stack.is_empty(), stack.item.registry_key.to_string())
                    };
                    if !is_empty {
                        let mut pickup_event =
                            crate::plugin::api::events::inventory::inventory_pickup_item::InventoryPickupItemEvent::new(
                                self.position,
                                item_entity.get_entity().entity_id,
                                registry_key,
                            );
                        if let Some(server) = world.server.upgrade() {
                            server
                                .plugin_manager
                                .fire_blocking(&server, &mut pickup_event);
                        }
                        if pickup_event.cancelled {
                            continue;
                        }
                        let (backup, one_item, is_empty) = {
                            let mut stack = item_entity
                                .get_item_stack()
                                .lock()
                                .unwrap_or_else(std::sync::PoisonError::into_inner);
                            if stack.is_empty() {
                                continue;
                            }
                            let backup = stack.clone();
                            let one_item = stack.split(1);
                            let is_empty = stack.is_empty();
                            (backup, one_item, is_empty)
                        };
                        if Self::add_one_item(self, self, &one_item) {
                            if is_empty {
                                item_entity.get_entity().remove();
                            }
                            return true;
                        }
                        let mut stack = item_entity
                            .get_item_stack()
                            .lock()
                            .unwrap_or_else(std::sync::PoisonError::into_inner);
                        *stack = backup;
                    }
                }
            }
        }
        false
    }

    /// 从 `slot` 中拆分出一个物品。读写共用同一把锁，因此快照即为该
    /// 实际发生移除的条目而非更早的条目。若届时槽位已为空则返回 `None`。
    fn take_one(&self, slot: usize) -> Option<Extraction> {
        let mut items = self
            .items
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if items[slot].is_empty() {
            return None;
        }
        let snapshot = items[slot].clone();
        let one_item = items[slot].split(1);
        let remainder = items[slot].clone();
        self.mark_dirty();
        Some(Extraction {
            one_item,
            snapshot,
            remainder,
        })
    }

    /// 在交易要约失败后撤销 [`Self::take_one`]，当槽位没有……时交还物品
    /// 它的空间了。
    ///
    /// 快照只匹配无人写入过的槽位，因此在匹配时恢复，而在
    /// 不匹配，那么无论哪个方向，强行写入都会抵消另一次写入。
    fn put_back(&self, slot: usize, extraction: Extraction) -> Option<ItemStack> {
        let mut items = self
            .items
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        self.mark_dirty();
        let current = &mut items[slot];
        if current.are_equal(&extraction.remainder) {
            *current = extraction.snapshot;
            return None;
        }
        if current.is_empty() {
            *current = extraction.one_item;
            return None;
        }
        if current.are_items_and_components_equal(&extraction.one_item)
            && current.item_count < current.get_max_stack_size()
        {
            current.item_count += 1;
            return None;
        }
        Some(extraction.one_item)
    }

    fn eject_items(&self, world: &Arc<World>) -> bool {
        // TODO getEntityContainer

        if let Some(entity) = world.get_block_entity(&self.position.offset(to_offset(&self.facing)))
            && let Some(container) = entity.get_inventory()
        {
            // TODO 检查 WorldlyContainer
            let mut is_full = true;
            for i in 0..container.size() {
                let item = container.get_stack(i);
                if item.item_count < item.get_max_stack_size() {
                    is_full = false;
                    break;
                }
            }
            if is_full {
                return false;
            }
            let target_pos = self.position.offset(to_offset(&self.facing));
            for slot in 0..Self::INVENTORY_SIZE {
                let item = self.get_stack(slot);
                if item.is_empty() {
                    continue;
                }
                let mut move_event = crate::plugin::api::events::inventory::inventory_move_item::InventoryMoveItemEvent::new(
                    self.position,
                    target_pos,
                    item.item.registry_key.to_string(),
                    1,
                );
                if let Some(server) = world.server.upgrade() {
                    server
                        .plugin_manager
                        .fire_blocking(&server, &mut move_event);
                }
                if move_event.cancelled {
                    continue;
                }
                // 原版 `HopperBlockEntity.ejectItems`：真正把物品从
                // 从漏斗中取出后再提供给目标，失败时放回。读取
                // 仅克隆而不写回会使源堆栈保持原样，从而复制出
                // 物品进入目标容器，而漏斗仍保留完整堆叠。
                let Some(extraction) = self.take_one(slot) else {
                    // 在事件触发期间被清空。空的物品堆会走 `add_one_item` 的
                    // `dst.is_empty()` 分支，并报告一次从未发生的转移。
                    continue;
                };
                if Self::add_one_item(self, container.as_ref(), &extraction.one_item) {
                    return true;
                }
                if let Some(leftover) = self.put_back(slot, extraction) {
                    // 槽位现在属于他人且已满 -> 掉落优于覆盖或清除。
                    let pos = self.position.to_centered_f64();
                    world.scatter_stack(pos.x, pos.y, pos.z, leftover);
                }
            }
        }
        false
    }
    pub fn add_one_item(from: &dyn Inventory, to: &dyn Inventory, item: &ItemStack) -> bool {
        let mut success = false;
        let to_empty = to.is_empty();
        for j in 0..to.size() {
            if to.is_valid_slot_for(j, item) {
                let mut dst = to.get_stack(j);
                if dst.is_empty() {
                    dst = item.clone();
                    to.set_stack(j, dst);
                    success = true;
                } else if dst.item_count < dst.get_max_stack_size()
                    && dst.are_items_and_components_equal(item)
                {
                    dst.item_count += 1;
                    to.set_stack(j, dst);
                    success = true;
                }
                if success {
                    if to_empty
                        && let Some(hopper) = to.as_any().downcast_ref::<Self>()
                        && hopper.cooldown_time.load(Ordering::Relaxed) <= 8
                    {
                        if let Some(from_hopper) = from.as_any().downcast_ref::<Self>() {
                            if from_hopper.cooldown_time.load(Ordering::Relaxed)
                                >= hopper.cooldown_time.load(Ordering::Relaxed)
                            {
                                hopper.cooldown_time.store(7, Ordering::Relaxed);
                            } else {
                                hopper.cooldown_time.store(8, Ordering::Relaxed);
                            }
                        } else {
                            hopper.cooldown_time.store(8, Ordering::Relaxed);
                        }
                    }
                    to.mark_dirty();
                    return true;
                }
            }
        }
        false
    }
}

impl Inventory for HopperBlockEntity {
    fn size(&self) -> usize {
        Self::INVENTORY_SIZE
    }

    fn is_empty(&self) -> bool {
        let items = self
            .items
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        items.iter().all(ItemStack::is_empty)
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

    fn mark_dirty(&self) {
        self.dirty.store(true, Ordering::Relaxed);
        self.comparator_dirty.store(true, Ordering::Relaxed);
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl Clearable for HopperBlockEntity {
    fn clear(&self) {
        let mut items = self
            .items
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        items.fill_with(|| ItemStack::EMPTY.clone());
        self.mark_dirty();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use papokin_data::{Block, item::Item};

    #[test]
    fn hopper_state_yields_its_properties() {
        let properties =
            hopper_properties(Block::HOPPER.id, Block::HOPPER.default_state.id).unwrap();

        assert!(properties.enabled);
        assert_eq!(properties.facing, FacingHopper::Down);
    }

    #[test]
    fn disabled_hopper_state_is_read_as_disabled() {
        let disabled = HopperLikeProperties {
            facing: FacingHopper::North,
            enabled: false,
        }
        .to_state_id(&Block::HOPPER);

        let properties = hopper_properties(Block::HOPPER.id, disabled).unwrap();

        assert!(!properties.enabled);
        assert_eq!(properties.facing, FacingHopper::North);
    }

    /// 其中每一个都能通过 `from_state_id` 干净地解码为某个 `facing`/`enabled`。
    /// 状态本身没有任何外来标记，因此只能凭 id 予以拒绝。
    #[test]
    fn replacement_state_yields_no_properties() {
        for replacement in [Block::AIR, Block::CHEST, Block::DROPPER, Block::PISTON] {
            assert!(
                hopper_properties(replacement.id, replacement.default_state.id).is_none(),
                "{} was accepted as a hopper",
                replacement.name
            );
        }
    }

    /// 另一个方向：位于外部 ID 下的漏斗状态仍是外部的。固定该 ID
    /// 来决定，而不是对状态做范围检查。
    #[test]
    fn hopper_state_id_under_another_block_yields_no_properties() {
        assert!(hopper_properties(Block::CHEST.id, Block::HOPPER.default_state.id).is_none());
    }

    fn hopper_holding(stack: ItemStack) -> HopperBlockEntity {
        let hopper = HopperBlockEntity::new(BlockPos::new(0, 0, 0), FacingHopper::Down);
        hopper.set_stack(0, stack);
        hopper
    }

    #[test]
    fn take_one_splits_a_single_item_off() {
        let hopper = hopper_holding(ItemStack::new(10, &Item::DIAMOND));

        let extraction = hopper.take_one(0).unwrap();

        assert_eq!(extraction.one_item.item_count, 1);
        assert_eq!(extraction.snapshot.item_count, 10);
        assert_eq!(extraction.remainder.item_count, 9);
        assert_eq!(hopper.get_stack(0).item_count, 9);
    }

    #[test]
    fn take_one_on_an_empty_slot_extracts_nothing() {
        let hopper = hopper_holding(ItemStack::EMPTY.clone());

        assert!(hopper.take_one(0).is_none());
    }

    #[test]
    fn untouched_slot_gets_the_snapshot_back() {
        let hopper = hopper_holding(ItemStack::new(10, &Item::DIAMOND));
        let extraction = hopper.take_one(0).unwrap();

        assert!(hopper.put_back(0, extraction).is_none());
        assert_eq!(hopper.get_stack(0).item_count, 10);
    }

    /// 快照显示为 10，而槽位为 3，因为在该报价挂出期间有人取走了 7。
    /// 恢复快照会把那 7 个凭空变回来，所以只有这一个物品会返还。
    #[test]
    fn changed_count_takes_back_one_item_not_the_snapshot() {
        let hopper = hopper_holding(ItemStack::new(10, &Item::DIAMOND));
        let extraction = hopper.take_one(0).unwrap();
        hopper.set_stack(0, ItemStack::new(3, &Item::DIAMOND));

        assert!(hopper.put_back(0, extraction).is_none());
        assert_eq!(hopper.get_stack(0).item_count, 4);
    }

    /// 外部物品无法吸收这唯一的一件物品，也不得覆盖它，因此没有任何合适的位置，
    /// 物品就会被退回。
    #[test]
    fn foreign_item_in_the_slot_is_left_alone() {
        let hopper = hopper_holding(ItemStack::new(1, &Item::DIAMOND));
        let extraction = hopper.take_one(0).unwrap();
        hopper.set_stack(0, ItemStack::new(64, &Item::DIRT));

        let leftover = hopper.put_back(0, extraction).unwrap();

        assert_eq!(leftover.get_item().id, Item::DIAMOND.id);
        assert_eq!(leftover.item_count, 1);
        let current = hopper.get_stack(0);
        assert_eq!(current.get_item().id, Item::DIRT.id);
        assert_eq!(current.item_count, 64);
    }

    /// 在此期间已被清空，因此有空间且无需覆盖任何内容。
    #[test]
    fn emptied_slot_takes_the_single_item() {
        let hopper = hopper_holding(ItemStack::new(10, &Item::DIAMOND));
        let extraction = hopper.take_one(0).unwrap();
        hopper.set_stack(0, ItemStack::EMPTY.clone());

        assert!(hopper.put_back(0, extraction).is_none());
        assert_eq!(hopper.get_stack(0).item_count, 1);
    }

    /// 是相同物品但已无剩余空间。再增加就会超过 `get_max_stack_size`。
    #[test]
    fn full_slot_of_the_same_item_hands_the_item_back() {
        let hopper = hopper_holding(ItemStack::new(10, &Item::DIAMOND));
        let extraction = hopper.take_one(0).unwrap();
        let max = ItemStack::new(1, &Item::DIAMOND).get_max_stack_size();
        hopper.set_stack(0, ItemStack::new(max, &Item::DIAMOND));

        let leftover = hopper.put_back(0, extraction).unwrap();

        assert_eq!(leftover.item_count, 1);
        assert_eq!(hopper.get_stack(0).item_count, max);
    }
}
