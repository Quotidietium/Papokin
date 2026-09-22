//! 合成模块。
//!
//! 本模块处理合成机制，包括：
//! - [`CraftingInventory`] - 用于合成槽位的临时物品栏
//! - [`CraftingScreenHandler`] - 合成台与物品栏合成使用的屏幕处理器
//! - [`recipes`] - 配方匹配与合成结果计算

pub mod crafting_inventory;
pub mod crafting_screen_handler;
pub mod recipe_provider;
pub mod recipes;
