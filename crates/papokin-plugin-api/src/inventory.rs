//! 插件物品栏与容器管理工具。
//!
//! 本模块提供统一的 API，用于查看、修改
//! 玩家物品栏、末影箱、自定义 GUI 和容器方块实体并与之交互。
//!
//! # Examples
//!
//! ## Inspecting and Modifying a Player's Inventory
//! ```rust,ignore
//! use papokin_plugin_api::{Player, ItemStack};
//!
//! fn equip_player(player: &Player) {
//!     let inv = player.get_inventory();
//!     inv.set_helmet(Some(ItemStack::new("minecraft:diamond_helmet", 1)));
//!     inv.set_boots(Some(ItemStack::new("minecraft:diamond_boots", 1)));
//!     
//!     let storage = inv.as_inventory();
//!     storage.set_item(0, Some(ItemStack::new("minecraft:diamond_sword", 1)));
//! }
//! ```
//!
//! ## Interacting with Custom GUIs
//! ```rust,ignore
//! use papokin_plugin_api::gui::Gui;
//!
//! fn setup_gui(gui: &Gui) {
//!     let inv = gui.get_inventory();
//!     inv.clear();
//! }
//! ```

pub use crate::wit::papokin::plugin::inventory::{Inventory, PlayerInventory};
