//! 通用容器屏幕处理器。
//!
//! 此模块为简单容器提供通用的屏幕处理器，例如：
//! - 箱子（单箱、大箱子、末影箱）
//! - 漏斗
//! - 发射器/投掷器
//! - 木桶
//!
//! 这些容器具有简单的网格布局，没有特殊行为
//! （不能熔炼、不能合成，仅用于存放物品）。

use std::{any::Any, sync::Arc};

use crate::inventory::Inventory;
use papokin_data::{item_stack::ItemStack, screen::WindowType};

use crate::{
    player::player_inventory::PlayerInventory,
    screen_handler::{InventoryPlayer, ScreenHandler, ScreenHandlerBehaviour},
    slot::NormalSlot,
};

/// 创建通用 9x3 容器（单个箱子）。
///
/// 用于单个箱子、末影箱及类似容器。
pub fn create_generic_9x3(
    sync_id: u8,
    player_inventory: &Arc<PlayerInventory>,
    inventory: Arc<dyn Inventory>,
    player: &dyn InventoryPlayer,
) -> GenericContainerScreenHandler {
    GenericContainerScreenHandler::new(
        WindowType::Generic9x3,
        sync_id,
        player_inventory,
        inventory,
        3,
        9,
        player.is_spectator(),
    )
}

/// 创建通用 9x6 容器（双联箱子）。
///
/// 用于大箱子及类似的大型容器。
pub fn create_generic_9x6(
    sync_id: u8,
    player_inventory: &Arc<PlayerInventory>,
    inventory: Arc<dyn Inventory>,
    player: &dyn InventoryPlayer,
) -> GenericContainerScreenHandler {
    GenericContainerScreenHandler::new(
        WindowType::Generic9x6,
        sync_id,
        player_inventory,
        inventory,
        6,
        9,
        player.is_spectator(),
    )
}

/// 创建通用 3x3 容器。
///
/// 用于发射器、投掷器及类似容器。
pub fn create_generic_3x3(
    sync_id: u8,
    player_inventory: &Arc<PlayerInventory>,
    inventory: Arc<dyn Inventory>,
    player: &dyn InventoryPlayer,
) -> GenericContainerScreenHandler {
    GenericContainerScreenHandler::new(
        WindowType::Generic3x3,
        sync_id,
        player_inventory,
        inventory,
        3,
        3,
        player.is_spectator(),
    )
}

/// 创建合成器容器（9 个槽位，3x3 布局）。
pub fn create_crafter_3x3(
    sync_id: u8,
    player_inventory: &Arc<PlayerInventory>,
    inventory: Arc<dyn Inventory>,
    player: &dyn InventoryPlayer,
) -> GenericContainerScreenHandler {
    GenericContainerScreenHandler::new(
        WindowType::Crafter3x3,
        sync_id,
        player_inventory,
        inventory,
        3,
        3,
        player.is_spectator(),
    )
}

/// 创建漏斗容器（5 个槽位）。
///
/// 漏斗只有一排共 5 个槽位。
pub fn create_hopper(
    sync_id: u8,
    player_inventory: &Arc<PlayerInventory>,
    inventory: Arc<dyn Inventory>,
    player: &dyn InventoryPlayer,
) -> GenericContainerScreenHandler {
    GenericContainerScreenHandler::new(
        WindowType::Hopper,
        sync_id,
        player_inventory,
        inventory,
        1,
        5,
        player.is_spectator(),
    )
}

/// 通用容器界面处理器。
///
/// 处理无特殊行为的简单网格型容器。
/// 容器网格之后是玩家的物品栏（27 个槽位 + 9 个快捷栏槽位）。
pub struct GenericContainerScreenHandler {
    /// 容器的物品栏。
    pub inventory: Arc<dyn Inventory>,
    /// 容器网格的行数。
    pub rows: u8,
    /// 容器网格的列数。
    pub columns: u8,
    /// 打开者是否处于旁观模式。
    pub is_spectator: bool,
    /// 核心屏幕处理器行为（槽位、同步 ID、监听器）。
    behaviour: ScreenHandlerBehaviour,
}

impl GenericContainerScreenHandler {
    /// 创建新的通用容器屏幕处理器。
    ///
    /// # Arguments
    /// - `screen_type` - 此容器的窗口类型
    /// - `sync_id` - 用于客户端-服务器匹配的同步 ID
    /// - `player_inventory` - 玩家的物品栏
    /// - `inventory` - 容器的物品栏
    /// - `rows` - 容器的行数
    /// - `columns` - 容器的列数
    /// - `is_spectator` - 打开者是否为旁观模式
    fn new(
        screen_type: WindowType,
        sync_id: u8,
        player_inventory: &Arc<PlayerInventory>,
        inventory: Arc<dyn Inventory>,
        rows: u8,
        columns: u8,
        is_spectator: bool,
    ) -> Self {
        let mut handler = Self {
            inventory,
            rows,
            columns,
            is_spectator,
            behaviour: ScreenHandlerBehaviour::new(sync_id, Some(screen_type)),
        };

        if !is_spectator {
            handler.inventory.on_open();
        }

        handler.add_inventory_slots();
        let player_inventory: Arc<dyn Inventory> = player_inventory.clone();
        handler.add_player_slots(&player_inventory);

        handler
    }

    /// 为容器的物品栏网格添加槽位。
    fn add_inventory_slots(&mut self) {
        for i in 0..self.rows {
            for j in 0..self.columns {
                self.add_slot(Arc::new(NormalSlot::new(
                    self.inventory.clone(),
                    (j + i * self.columns) as usize,
                )));
            }
        }
    }
}

impl ScreenHandler for GenericContainerScreenHandler {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }

    fn get_behaviour(&self) -> &ScreenHandlerBehaviour {
        &self.behaviour
    }

    fn get_behaviour_mut(&mut self) -> &mut ScreenHandlerBehaviour {
        &mut self.behaviour
    }

    fn on_closed(&mut self, player: &dyn InventoryPlayer) {
        self.default_on_closed(player);
        if !self.is_spectator && !player.is_spectator() {
            self.inventory.on_close();
        }
    }

    /// 通用容器的快速移动逻辑。
    ///
    /// - 从容器：移到玩家物品栏（从末尾开始）
    /// - 从玩家物品栏：移到容器（从头开始）
    fn quick_move(&mut self, _player: &dyn InventoryPlayer, slot_index: i32) -> ItemStack {
        let mut stack_left = ItemStack::EMPTY.clone();
        // 假设 slot_index 的边界检查已由调用方通过，或在 quick_move 规范内
        let slot = self.get_behaviour().slots[slot_index as usize].clone();

        if slot.has_stack() {
            let mut slot_stack = slot.get_stack();
            stack_left = slot_stack.clone();

            if slot_index < i32::from(self.rows * 9) {
                // 从物品栏移到玩家区域（结束）
                if !self.insert_item(
                    &mut slot_stack,
                    (self.rows * 9).into(),
                    self.get_behaviour().slots.len() as i32,
                    true,
                ) {
                    return ItemStack::EMPTY.clone();
                }
            } else if !self.insert_item(&mut slot_stack, 0, (self.rows * 9).into(), false) {
                // 从玩家区域移到物品栏（开始）
                return ItemStack::EMPTY.clone();
            }

            // 检查 insert_item 之后槽位物品堆的结果状态
            if slot_stack.is_empty() {
                slot.set_stack(ItemStack::EMPTY.clone());
            } else {
                slot.set_stack(slot_stack);
            }
        }

        stack_left
    }
}
