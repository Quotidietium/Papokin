//! 双重物品栏实现。
//!
//! 此模块提供一种组合物品栏，可将两个物品栏
//! 合并为一个。这用于大箱子等大型容器，它们
//! 由两个单箱物品栏组成，被视为一个 54 槽的物品栏。
//!
//! 第一个物品栏的槽位在前，其后是第二个物品栏的
//! 槽位。操作会被委托给相应的底层物品栏
//! 基于槽位索引进行。

use std::{any::Any, sync::Arc};

use crate::inventory::{Clearable, Inventory};
use papokin_data::item_stack::ItemStack;

/// 一个组合了两个物品栏的复合物品栏。
///
/// 用于大箱子和其他横跨……的大型容器
/// 多个方块实体。合并后的物品栏大小为
/// 两个物品栏大小之和。
pub struct DoubleInventory {
    /// 第一个物品栏（较小的槽位索引，0 到 first.size()-1）。
    first: Arc<dyn Inventory>,
    /// 第二个物品栏（槽位索引较高，从 `first.size()` 到 total-1）。
    second: Arc<dyn Inventory>,
}

impl DoubleInventory {
    /// 创建新的双联物品栏。
    ///
    /// # Arguments
    /// - `first` - 第一个物品栏（槽位索引较小）
    /// - `second` - 第二个物品栏（槽位索引较大的一侧）
    ///
    /// # Returns
    /// 对新双倍物品栏的共享引用。
    pub fn new(first: Arc<dyn Inventory>, second: Arc<dyn Inventory>) -> Arc<Self> {
        Arc::new(Self { first, second })
    }
}

impl Inventory for DoubleInventory {
    /// 双联视图是每次开屏新建的临时实例（不同玩家各持一份），
    /// 以自身地址为键无法跨玩家对上号；递归收集底层两个半箱
    /// 的身份键，使点击串行化命中同一组共享实例。
    fn collect_click_lock_ids(&self, _self_id: usize, out: &mut Vec<usize>) {
        self.first
            .collect_click_lock_ids(crate::inventory::inventory_lock_id(&self.first), out);
        self.second
            .collect_click_lock_ids(crate::inventory::inventory_lock_id(&self.second), out);
    }

    fn size(&self) -> usize {
        self.first.size() + self.second.size()
    }

    fn is_empty(&self) -> bool {
        self.first.is_empty() && self.second.is_empty()
    }

    fn get_stack(&self, slot: usize) -> ItemStack {
        if slot >= self.first.size() {
            self.second.get_stack(slot - self.first.size())
        } else {
            self.first.get_stack(slot)
        }
    }

    fn remove_stack(&self, slot: usize) -> ItemStack {
        if slot >= self.first.size() {
            self.second.remove_stack(slot - self.first.size())
        } else {
            self.first.remove_stack(slot)
        }
    }

    fn remove_stack_specific(&self, slot: usize, amount: u8) -> ItemStack {
        if slot >= self.first.size() {
            self.second
                .remove_stack_specific(slot - self.first.size(), amount)
        } else {
            self.first.remove_stack_specific(slot, amount)
        }
    }

    fn set_stack(&self, slot: usize, stack: ItemStack) {
        if slot >= self.first.size() {
            self.second.set_stack(slot - self.first.size(), stack);
        } else {
            self.first.set_stack(slot, stack);
        }
    }

    fn on_open(&self) {
        self.first.on_open();
        self.second.on_open();
    }

    fn on_close(&self) {
        self.first.on_close();
        self.second.on_close();
    }

    fn get_max_count_per_stack(&self) -> u8 {
        self.first.get_max_count_per_stack()
    }

    fn mark_dirty(&self) {
        self.first.mark_dirty();
        self.second.mark_dirty();
    }

    fn is_valid_slot_for(&self, slot: usize, stack: &ItemStack) -> bool {
        if slot >= self.first.size() {
            self.second
                .is_valid_slot_for(slot - self.first.size(), stack)
        } else {
            self.first.is_valid_slot_for(slot, stack)
        }
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl Clearable for DoubleInventory {
    fn clear(&self) {
        self.first.clear();
        self.second.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::inventory::{SimpleInventory, inventory_lock_id};

    #[test]
    fn double_inventory_resolves_lock_ids_to_shared_halves() {
        let first: Arc<dyn Inventory> = Arc::new(SimpleInventory::new(27));
        let second: Arc<dyn Inventory> = Arc::new(SimpleInventory::new(27));
        let first_id = inventory_lock_id(&first);
        let second_id = inventory_lock_id(&second);

        // 两个玩家各开一次大箱 = 两个不同的临时包装实例；
        // 点击串行化键必须解析到同一对共享半箱，而不是包装自身
        let view_a: Arc<dyn Inventory> = DoubleInventory::new(first.clone(), second.clone());
        let view_b: Arc<dyn Inventory> = DoubleInventory::new(first.clone(), second.clone());

        let mut ids_a = Vec::new();
        let mut ids_b = Vec::new();
        view_a.collect_click_lock_ids(1, &mut ids_a);
        view_b.collect_click_lock_ids(2, &mut ids_b);
        ids_a.sort_unstable();
        ids_b.sort_unstable();

        assert_eq!(ids_a, ids_b, "不同包装实例必须解析出相同键集合");
        assert!(ids_a.contains(&first_id) && ids_a.contains(&second_id));
        // 传入的包装自身 id（1/2，非真实指针）不应混入
        assert!(!ids_a.contains(&1) && !ids_a.contains(&2));
    }
}
