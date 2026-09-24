//! 物品栏槽位实现。
//!
//! 本模块定义 [`Slot`] trait 及其实现。槽位表示
//! 物品栏中可以存放物品的各个独立位置。
//!
//! # Slot Types
//!
//! - [`NormalSlot`] - 无限制的基础物品栏槽位
//! - [`ArmorSlot`] - 只接受相应物品类型的盔甲槽位
//!   （头盔在头部槽位、胸甲在胸部槽位等）
//!
//! # Slot Operations
//!
//! 槽位支持多种操作：
//! - 获取/设置物品堆
//! - 检查物品能否放入
//! - 从槽位取出物品
//! - 将槽位标记为已更改（脏）
//! - 槽位交互事件的回调

use std::sync::{
    Arc,
    atomic::{AtomicU8, Ordering},
};

use crate::screen_handler::InventoryPlayer;

use crate::inventory::Inventory;
use papokin_data::data_component_impl::EquipmentSlot;
use papokin_data::item::Item;
use papokin_data::item_stack::ItemStack;

/// 物品栏中的一个槽位。
///
/// Slot trait 定义了物品栏中各个位置的行为方式。
/// 不同的槽位类型（普通槽位、盔甲槽位、结果槽位）实现此
/// trait 来强制执行它们各自的限制。
// Slot.java
pub trait Slot: Send + Sync {
    /// 返回包含此槽位的物品栏。
    fn get_inventory(&self) -> Arc<dyn Inventory>;

    /// 返回此槽位在其物品栏中的索引。
    fn get_index(&self) -> usize;

    /// 设置此槽位的协议 ID。
    fn set_id(&self, index: usize);

    /// 从此槽位快速移动物品时的回调。
    ///
    /// 用于通知结果槽位（如合成输出）它们
    /// 需要重新填充其内容。
    ///
    /// # Note
    /// 在更改物品堆并释放之后**必须**调用此方法
    /// 持有任何锁，以避免死锁。
    ///
    /// 另见：[`ScreenHandler::quick_move`](crate::screen_handler::ScreenHandler::quick_move)
    fn on_quick_move_crafted(&self, _stack: ItemStack, _stack_prev: ItemStack) {}

    /// 从此槽位取出物品时的回调。
    ///
    /// 另见：[`safe_take`]
    fn on_take_item(&self, _player: &dyn InventoryPlayer, _stack: &ItemStack) {
        self.mark_dirty();
    }

    /// 槽位点击的插件回调。
    ///
    /// 当玩家点击此槽位时调用。可供
    /// 插件拦截或修改点击行为。
    fn on_click(&self, _player: &dyn InventoryPlayer) {}

    /// 检查给定物品堆能否插入此槽位。
    fn can_insert(&self, _stack: &ItemStack) -> bool {
        true
    }

    /// 获取此槽位中的物品堆。
    fn get_stack(&self) -> ItemStack {
        self.get_inventory().get_stack(self.get_index())
    }

    /// 获取此槽位中物品堆的副本。
    fn get_cloned_stack(&self) -> ItemStack {
        self.get_stack()
    }

    /// 检查此槽位是否有非空物品堆。
    fn has_stack(&self) -> bool {
        !self.get_stack().is_empty()
    }

    /// 设置此槽位中的物品堆。
    ///
    /// # Note
    /// 调用此方法前，请确保已释放对槽位物品堆持有的所有锁。
    fn set_stack(&self, stack: ItemStack) {
        self.set_stack_no_callbacks(stack);
    }

    /// 设置物品堆并保留对原物品堆的引用。
    ///
    /// 某些槽位（如盔甲槽）需要知道原物品堆才能执行回调。
    fn set_stack_prev(&self, stack: ItemStack, _previous_stack: ItemStack) {
        self.set_stack_no_callbacks(stack);
    }

    /// 设置物品堆但不调用回调。
    fn set_stack_no_callbacks(&self, stack: ItemStack) {
        let inv = self.get_inventory();
        inv.set_stack(self.get_index(), stack);
        self.mark_dirty();
    }

    /// 将此槽位标记为已更改。
    ///
    /// 必须由具体类型实现。
    fn mark_dirty(&self);

    /// 获取此槽位的最大物品数量。
    fn get_max_item_count(&self) -> u8 {
        self.get_inventory().get_max_count_per_stack()
    }

    /// 获取此槽位中给定物品堆的最大物品数量。
    fn get_max_item_count_for_stack(&self, stack: &ItemStack) -> u8 {
        self.get_max_item_count().min(stack.get_max_stack_size())
    }

    /// 从此槽位移除特定数量的物品。
    ///
    /// Mojang 名称：`remove`
    fn take_stack(&self, amount: u8) -> ItemStack {
        let inv = self.get_inventory();
        inv.remove_stack_specific(self.get_index(), amount)
    }

    /// 检查玩家能否从此槽位取出物品。
    ///
    /// Mojang 名称：`mayPickup`
    fn can_take_items(&self, _player: &dyn InventoryPlayer) -> bool {
        true
    }

    /// 检查此槽位能否被玩家修改。
    ///
    /// Mojang 名称：`allowModification`
    fn allow_modification(&self, player: &dyn InventoryPlayer) -> bool {
        self.can_insert(&self.get_cloned_stack()) && self.can_take_items(player)
    }

    /// 尝试在给定范围内取出一个物品堆。
    ///
    ///若无法取出物品或槽位为空，则返回 `None`。
    /// 对于结果槽位，不能只取出部分物品堆。
    ///
    /// Mojang 名称：`tryRemove`
    fn try_take_stack_range(
        &self,
        min: u8,
        max: u8,
        player: &dyn InventoryPlayer,
    ) -> Option<ItemStack> {
        if !self.can_take_items(player) {
            return None;
        }
        if !self.allow_modification(player) && self.get_cloned_stack().item_count > max {
            // 如果不允许修改该槽位，则无法从其中取出部分堆叠。
            return None;
        }
        let min = min.min(max);
        let stack = self.take_stack(min);

        if stack.is_empty() {
            None
        } else {
            if self.get_cloned_stack().is_empty() {
                self.set_stack_prev(ItemStack::EMPTY.clone(), stack.clone());
            }

            Some(stack)
        }
    }

    /// 安全地尝试从该槽位取出一组物品。
    ///
    ///若无法取出则返回空物品堆。会触发回调。
    ///
    /// Mojang 名称：`safeTake`
    fn safe_take(&self, min: u8, max: u8, player: &dyn InventoryPlayer) -> ItemStack {
        let stack = self.try_take_stack_range(min, max, player);

        if let Some(stack) = &stack {
            self.on_take_item(player, stack);
        }

        stack.unwrap_or_else(|| ItemStack::EMPTY.clone())
    }

    /// 将物品堆插入此槽位。
    ///
    ///返回无法放入的剩余物品。
    fn insert_stack(&self, stack: ItemStack) -> ItemStack {
        let stack_item_count = stack.item_count;
        self.insert_stack_count(stack, stack_item_count)
    }

    /// 从物品堆中插入指定数量。
    ///
    ///返回剩余的物品。
    fn insert_stack_count(&self, mut stack: ItemStack, count: u8) -> ItemStack {
        if !stack.is_empty() && self.can_insert(&stack) {
            let mut stack_self = self.get_stack();
            // saturating_sub：槽位因存档损坏出现超堆叠时 u8 裸减会
            // 下溢（debug panic/release 回绕为巨值继续塞入）
            let min_count = count.min(stack.item_count).min(
                self.get_max_item_count_for_stack(&stack)
                    .saturating_sub(stack_self.item_count),
            );

            if min_count != 0 {
                if stack_self.is_empty() {
                    self.set_stack(stack.split(min_count));
                } else if stack.are_items_and_components_equal(&stack_self) {
                    stack.decrement(min_count);
                    stack_self.increment(min_count);
                    self.set_stack(stack_self);
                }
            }
        }
        if stack.is_empty() {
            ItemStack::EMPTY.clone()
        } else {
            stack
        }
    }
}

/// 一个普通物品栏槽位。
///
/// 在原版 Minecraft 中称为 `Slot`。这是基础的
/// 槽位实现，没有任何特殊限制。
pub struct NormalSlot {
    /// 包含此槽位的物品栏。
    pub inventory: Arc<dyn Inventory>,
    /// 此槽位在其物品栏中的索引。
    pub index: usize,
    /// 此槽位的协议 ID（由界面处理器分配）。
    pub id: AtomicU8,
}

impl NormalSlot {
    /// 创建一个新的普通槽位。
    ///
    /// # Arguments
    /// - `inventory` - 所属的物品栏
    /// - `index` - 物品栏内的槽位索引
    pub fn new(inventory: Arc<dyn Inventory>, index: usize) -> Self {
        Self {
            inventory,
            index,
            id: AtomicU8::new(0),
        }
    }
}

impl Slot for NormalSlot {
    fn get_inventory(&self) -> Arc<dyn Inventory> {
        self.inventory.clone()
    }

    fn get_index(&self) -> usize {
        self.index
    }

    fn set_id(&self, id: usize) {
        self.id.store(id as u8, Ordering::Relaxed);
    }

    fn mark_dirty(&self) {
        self.inventory.mark_dirty();
    }
}

/// 一个盔甲装备槽位。
///
/// 根据装备栏位类型限制可放入的物品：
/// - 头部：头盔、头颅、雕刻南瓜
/// - 胸部：胸甲、鞘翅
/// - 腿部：护腿
/// - 脚部：靴子
// ArmorSlot.java
pub struct ArmorSlot {
    /// 包含此槽位的物品栏（通常是玩家物品栏）。
    pub inventory: Arc<dyn Inventory>,
    /// 此槽位在其物品栏中的索引。
    pub index: usize,
    /// 此槽位的协议 ID（由界面处理器分配）。
    pub id: AtomicU8,
    /// 装备槽位类型（头部、胸部、腿部、脚部或副手）。
    pub equipment_slot: EquipmentSlot,
}

impl ArmorSlot {
    /// 创建新的盔甲槽位。
    ///
    /// # Arguments
    /// - `inventory` - 所属的物品栏
    /// - `index` - 槽位索引
    /// - `equipment_slot` - 装备槽位类型（头部、胸部、腿部、脚部）
    pub fn new(inventory: Arc<dyn Inventory>, index: usize, equipment_slot: EquipmentSlot) -> Self {
        Self {
            inventory,
            index,
            id: AtomicU8::new(0),
            equipment_slot,
        }
    }
}

impl Slot for ArmorSlot {
    fn get_inventory(&self) -> Arc<dyn Inventory> {
        self.inventory.clone()
    }

    fn get_index(&self) -> usize {
        self.index
    }

    fn set_id(&self, id: usize) {
        self.id.store(id as u8, Ordering::Relaxed);
    }

    /// 限制只能插入相应类型的盔甲。
    fn can_insert(&self, stack: &ItemStack) -> bool {
        match self.equipment_slot {
            EquipmentSlot::Head(_) => {
                stack.is_helmet() || stack.is_skull() || stack.item == &Item::CARVED_PUMPKIN
            }
            EquipmentSlot::Chest(_) => stack.is_chestplate() || stack.item == &Item::ELYTRA,
            EquipmentSlot::Legs(_) => stack.is_leggings(),
            EquipmentSlot::Feet(_) => stack.is_boots(),
            EquipmentSlot::Saddle(_) => stack
                .get_data_component::<papokin_data::data_component_impl::EquippableImpl>()
                .map_or_else(
                    || stack.item == &Item::SADDLE,
                    |equippable| matches!(equippable.slot, EquipmentSlot::Saddle(_)),
                ),
            EquipmentSlot::Body(_) => stack
                .get_data_component::<papokin_data::data_component_impl::EquippableImpl>()
                .map_or_else(
                    || {
                        stack.item.registry_key.ends_with("_horse_armor")
                            || stack.item.registry_key.ends_with("_nautilus_armor")
                            || stack.item.registry_key.ends_with("_carpet")
                            || stack.item == &Item::WOLF_ARMOR
                    },
                    |equippable| matches!(equippable.slot, EquipmentSlot::Body(_)),
                ),
            _ => true,
        }
    }

    fn set_stack_prev(&self, stack: ItemStack, _previous_stack: ItemStack) {
        self.set_stack_no_callbacks(stack);
    }

    fn mark_dirty(&self) {
        self.inventory.mark_dirty();
    }

    /// 盔甲槽只能容纳一个物品。
    fn get_max_item_count(&self) -> u8 {
        1
    }

    /// TODO: 检查绑定诅咒附魔。
    fn can_take_items(&self, _player: &dyn InventoryPlayer) -> bool {
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::inventory::SimpleInventory;
    use papokin_data::item::Item;

    /// 超堆叠槽位（仅损坏存档/NBT 可达）插入同类物品时不得下溢：
    /// u8 裸减在 debug 下 panic、release 下回绕为巨值继续塞入。
    #[test]
    fn insert_into_overstacked_slot_is_rejected_safely() {
        let inventory = Arc::new(SimpleInventory::new(1));
        inventory.set_stack(0, ItemStack::new(100, &Item::STONE)); // 超堆叠（上限 64）
        let slot = NormalSlot::new(inventory.clone(), 0);

        let remainder = slot.insert_stack_count(ItemStack::new(10, &Item::STONE), 10);
        assert_eq!(remainder.item_count, 10, "超堆叠槽位不应再接受物品");
        assert_eq!(inventory.get_stack(0).item_count, 100, "既有堆叠不得被改动");
    }
}
