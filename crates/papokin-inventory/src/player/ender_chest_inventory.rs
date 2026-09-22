//! 末影箱物品栏实现。
//!
//! 末影箱是跨维度持久保存的玩家专属存储。
//! 每个玩家都有自己的末影箱内容物，可从
//! 任何末影箱方块。该物品栏在所有末影箱之间同步，
//! 针对该玩家。
//!
//! # Viewer Tracking
//!
//! 末影箱会追踪玩家打开和关闭的时机，以便正确
//! 管理用于动画目的的观看者数量。

use std::any::Any;
use std::sync::{Arc, Mutex, RwLock};

use crate::{
    inventory::{Clearable, Inventory},
    viewer::ViewerCountTracker,
};
use papokin_data::item_stack::ItemStack;

/// 玩家的末影箱物品栏。
///
/// 存储 27 个格子（如同单个箱子），为每位玩家私有。
/// 其内容跨维度持久保存，且可从任意
/// 末影箱方块。
pub struct EnderChestInventory {
    /// 末影箱中的 27 个物品槽位。
    pub items: RwLock<[ItemStack; Self::INVENTORY_SIZE]>,
    /// 用于盖子动画的观看者计数跟踪器。
    ///
    /// 跟踪有多少玩家打开了末影箱，以驱动箱盖动画。
    pub tracker: Mutex<Option<Arc<ViewerCountTracker>>>,
}

impl Default for EnderChestInventory {
    fn default() -> Self {
        Self::new()
    }
}

impl EnderChestInventory {
    /// 末影箱物品栏的大小（27 个槽位）。
    pub const INVENTORY_SIZE: usize = 27;

    /// 创建新的空末影箱物品栏。
    #[must_use]
    pub fn new() -> Self {
        Self {
            items: RwLock::new(std::array::from_fn(|_| ItemStack::EMPTY.clone())),
            tracker: Mutex::new(None),
        }
    }

    /// 设置此物品栏的查看者计数跟踪器。
    ///
    /// 用于根据观看者数量为末影箱盖子做动画。
    pub fn set_tracker(&self, tracker: Arc<ViewerCountTracker>) {
        let old = self
            .tracker
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .replace(tracker);
        if let Some(old_tracker) = old {
            old_tracker.close_container();
        }
    }

    /// 检查此物品栏是否已设置追踪器。
    pub fn has_tracker(&self) -> bool {
        self.tracker
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .is_some()
    }

    /// 检查给定追踪器是否与此物品栏关联。
    pub fn is_tracker(&self, tracker: &Arc<ViewerCountTracker>) -> bool {
        if let Some(value) = self
            .tracker
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .as_ref()
        {
            return Arc::ptr_eq(value, tracker);
        }
        false
    }
}

impl Inventory for EnderChestInventory {
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
        items
            .get(slot)
            .cloned()
            .unwrap_or_else(|| ItemStack::EMPTY.clone())
    }

    fn remove_stack(&self, slot: usize) -> ItemStack {
        let mut items = self
            .items
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if slot < Self::INVENTORY_SIZE {
            std::mem::replace(&mut items[slot], ItemStack::EMPTY.clone())
        } else {
            ItemStack::EMPTY.clone()
        }
    }

    fn remove_stack_specific(&self, slot: usize, amount: u8) -> ItemStack {
        let mut items = self
            .items
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if slot < Self::INVENTORY_SIZE && !items[slot].is_empty() && amount > 0 {
            items[slot].split(amount)
        } else {
            ItemStack::EMPTY.clone()
        }
    }

    fn set_stack(&self, slot: usize, stack: ItemStack) {
        let mut items = self
            .items
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if slot < Self::INVENTORY_SIZE {
            items[slot] = stack;
        }
    }

    fn on_open(&self) {
        if let Some(tracker) = self
            .tracker
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .as_ref()
        {
            tracker.open_container();
        }
    }

    fn on_close(&self) {
        let tracker = self
            .tracker
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .take();
        if let Some(tracker) = tracker {
            tracker.close_container();
        }
    }

    fn mark_dirty(&self) {}

    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl Clearable for EnderChestInventory {
    fn clear(&self) {
        let mut items = self
            .items
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        items.fill_with(|| ItemStack::EMPTY.clone());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use papokin_data::item::Item;

    #[test]
    fn new_inventory() {
        let ec = EnderChestInventory::new();
        assert_eq!(ec.size(), 27);
        assert!(ec.is_empty());
        assert!(!ec.has_tracker());
    }

    #[test]
    fn set_and_get_stack() {
        let ec = EnderChestInventory::new();
        let stack = ItemStack::new(1, &Item::DIRT);
        ec.set_stack(0, stack.clone());
        assert_eq!(ec.get_stack(0).item.id, Item::DIRT.id);
        assert_eq!(ec.get_stack(0).item_count, 1);
        assert!(!ec.is_empty());

        // 越界不应 panic
        ec.set_stack(100, stack);
        assert!(ec.get_stack(100).is_empty());
    }

    #[test]
    fn remove_stack() {
        let ec = EnderChestInventory::new();
        let stack = ItemStack::new(5, &Item::DIAMOND);
        ec.set_stack(10, stack);

        let removed_specific = ec.remove_stack_specific(10, 2);
        assert_eq!(removed_specific.item_count, 2);
        assert_eq!(ec.get_stack(10).item_count, 3);

        let removed_all = ec.remove_stack(10);
        assert_eq!(removed_all.item_count, 3);
        assert!(ec.get_stack(10).is_empty());
        assert!(ec.is_empty());
    }

    #[test]
    fn clear_inventory() {
        let ec = EnderChestInventory::new();
        ec.set_stack(0, ItemStack::new(1, &Item::STONE));
        ec.set_stack(26, ItemStack::new(1, &Item::OAK_LOG));
        assert!(!ec.is_empty());

        ec.clear();
        assert!(ec.is_empty());
    }

    #[test]
    fn tracker_lifecycle() {
        let ec = EnderChestInventory::new();
        let tracker1 = Arc::new(ViewerCountTracker::new());
        let tracker2 = Arc::new(ViewerCountTracker::new());

        ec.set_tracker(tracker1.clone());
        assert!(ec.has_tracker());
        assert!(ec.is_tracker(&tracker1));
        assert!(!ec.is_tracker(&tracker2));

        ec.on_open();
        assert_eq!(tracker1.get_viewer_count(), 1);

        // 在一个 tracker 打开时设置新的 tracker 应关闭旧的 tracker
        ec.set_tracker(tracker2.clone());
        assert_eq!(tracker1.get_viewer_count(), 0);
        assert!(ec.is_tracker(&tracker2));

        ec.on_open();
        assert_eq!(tracker2.get_viewer_count(), 1);

        ec.on_close();
        assert_eq!(tracker2.get_viewer_count(), 0);
        assert!(!ec.has_tracker());
    }
}
