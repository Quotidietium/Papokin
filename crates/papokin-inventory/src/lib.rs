//! Pumpkin 物品栏系统。
//!
//! 本 crate 为 Pumpkin Minecraft 服务器提供物品栏管理系统。
//! 它负责处理玩家物品栏、容器界面（如箱子、熔炉、合成台），
//! 槽位交互、物品拖拽以及服务器与客户端之间的物品栏同步。
//!
//! # Core Concepts
//!
//! - [`Inventory`] - 表示任意物品存储的 trait（玩家物品栏、箱子、熔炉等）
//! - [`ScreenHandler`] - 管理容器的 UI 界面，处理槽位布局与交互
//! - [`Slot`] - 表示物品栏中可容纳物品的单个槽位
//! - [`PlayerInventory`] - 玩家的 36 格主物品栏及装备槽位
//! - [`SyncHandler`] - 在服务器与客户端之间同步物品栏状态
//!
//! # Module Structure

#![deny(clippy::unwrap_used)]
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]
//!
//! - [`player`] - 玩家物品栏与屏幕处理器实现
//! - [`crafting`] - 合成台与物品栏合成机制
//! - [`furnace_like`] - 熔炉、烟熏炉与高炉的屏幕处理器
//! - [`brewing`] - 酿造台处理
//! - [`slot`] - 槽位 trait 及其实现（普通槽位、盔甲槽位）
//! - [`container_click`] - 鼠标与键盘点击处理
//! - [`drag_handler`] - 跨多个槽位的物品拖拽
//! - [`sync_handler`] - 客户端与服务器间的物品栏同步
//! - [`window_property`] - 容器 UI 属性（熔炉进度、附魔等级等）
//!
//! [`Inventory`]: inventory::Inventory
//! [`ScreenHandler`]: screen_handler::ScreenHandler
//! [`Slot`]: slot::Slot
//! [`PlayerInventory`]: PlayerInventory
//! [`SyncHandler`]: sync_handler::SyncHandler

pub mod anvil;
pub mod beacon_screen_handler;
pub mod brewing;
pub mod cartography_table_screen_handler;
pub mod container_click;
pub mod crafting;
pub mod double;
pub mod drag_handler;
pub mod enchanting;
pub mod entity_equipment;
mod error;
pub mod furnace_like;
pub mod generic_container_screen_handler;
pub mod grindstone_screen_handler;
pub mod gui_builder;
pub mod inventory;
pub mod lectern_screen_handler;
pub mod loom_screen_handler;
pub mod merchant;
pub mod mount_screen_handler;
pub mod player;
pub mod screen_handler;
pub mod slot;
pub mod smithing_table_screen_handler;
pub mod stonecutter_screen_handler;
pub mod sync_handler;
pub mod viewer;
pub mod window_property;

use rustc_hash::FxHashMap;

pub use error::InventoryError;
pub use inventory::{
    Clearable, ComparableInventory, Inventory, SimpleInventory, split_stack_slice,
    sync_read_items_from_nbt, sync_write_items_to_nbt,
};
use papokin_data::data_component_impl::EquipmentSlot;
pub use viewer::ViewerCountTracker;
pub use window_property::{ExperienceContainer, PropertyDelegate};

use crate::player::player_inventory::PlayerInventory;

/// 为玩家物品栏构建槽位索引到装备槽位的映射。
///
/// 这会创建 UI 槽位索引与装备槽位之间的映射
/// (头部、胸部、腿部、脚部、副手)，供玩家屏幕处理器使用。
///
/// # Returns
/// 一个 `FxHashMap`，键为槽位索引，值为对应的 [`EquipmentSlot`]。
#[must_use]
pub fn build_equipment_slots() -> FxHashMap<usize, EquipmentSlot> {
    let mut equipment_slots = FxHashMap::default();
    equipment_slots.insert(
        EquipmentSlot::FEET.get_offset_entity_slot_id(PlayerInventory::MAIN_SIZE as i32) as usize,
        EquipmentSlot::FEET,
    );
    equipment_slots.insert(
        EquipmentSlot::LEGS.get_offset_entity_slot_id(PlayerInventory::MAIN_SIZE as i32) as usize,
        EquipmentSlot::LEGS,
    );
    equipment_slots.insert(
        EquipmentSlot::CHEST.get_offset_entity_slot_id(PlayerInventory::MAIN_SIZE as i32) as usize,
        EquipmentSlot::CHEST,
    );
    equipment_slots.insert(
        EquipmentSlot::HEAD.get_offset_entity_slot_id(PlayerInventory::MAIN_SIZE as i32) as usize,
        EquipmentSlot::HEAD,
    );

    equipment_slots.insert(PlayerInventory::OFF_HAND_SLOT, EquipmentSlot::OFF_HAND);
    equipment_slots
}
