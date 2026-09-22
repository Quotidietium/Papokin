//! 合成物品栏实现。
//!
//! 此模块为合成网格提供临时物品栏。
//! 合成物品栏用于：
//! - 玩家物品栏中的 2x2 合成网格
//! - 合成台中的 3x3 合成网格
//! - 其他基于配方的合成机制
//!
//! 与普通物品栏不同，合成网格通常会在
//! 容器关闭时被清空，其内容会在合成时被消耗。

use std::any::Any;
use std::sync::RwLock;

use crate::inventory::{Clearable, Inventory};
use papokin_data::item_stack::ItemStack;

use super::recipes::RecipeInputInventory;

/// 用于合成网格的临时物品栏。
///
/// 合成物品栏以网格形式存放物品，供合成配方使用。
/// 网格尺寸可变（玩家物品栏为 2x2，工作台为 3x3）。
///
/// # Usage
///
/// 玩家放入合成网格的物品会存储在此处。
/// 取出合成结果时，会从此物品栏中消耗原料。
#[derive(Default)]
pub struct CraftingInventory {
    /// 合成网格的宽度（通常为 2 或 3）。
    pub width: u8,
    /// 合成网格的高度（通常为 2 或 3）。
    pub height: u8,
    /// 合成网格中的物品，按行存储。
    pub items: RwLock<Vec<ItemStack>>,
}

impl CraftingInventory {
    /// 使用给定尺寸创建新的合成物品栏。
    ///
    /// # Arguments
    /// - `width` - 网格宽度（例如玩家合成网格为 2，工作台网格为 3）
    /// - `height` - 网格高度（例如玩家合成 2 格，合成台 3 格）
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// // 2x2 player inventory crafting grid
    /// let player_crafting = CraftingInventory::new(2, 2);
    ///
    /// // 3x3 crafting table grid
    /// let table_crafting = CraftingInventory::new(3, 3);
    /// ```
    #[must_use]
    pub fn new(width: u8, height: u8) -> Self {
        Self {
            width,
            height,
            items: RwLock::new(vec![
                ItemStack::EMPTY.clone();
                width as usize * height as usize
            ]),
        }
    }
}

impl Inventory for CraftingInventory {
    fn size(&self) -> usize {
        (self.width as usize) * (self.height as usize)
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
        if slot < items.len() {
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
        if slot < items.len() && !items[slot].is_empty() && amount > 0 {
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
        if slot < items.len() {
            items[slot] = stack;
        }
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl RecipeInputInventory for CraftingInventory {
    fn get_width(&self) -> usize {
        self.width as usize
    }

    fn get_height(&self) -> usize {
        self.height as usize
    }
}

impl Clearable for CraftingInventory {
    fn clear(&self) {
        let mut items = self
            .items
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        items.fill_with(|| ItemStack::EMPTY.clone());
    }
}
