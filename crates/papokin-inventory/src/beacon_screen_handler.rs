use std::{any::Any, sync::Arc};

use crate::inventory::Inventory;
use papokin_data::{item_stack::ItemStack, screen::WindowType};

use crate::{
    player::player_inventory::PlayerInventory,
    screen_handler::{InventoryPlayer, ScreenHandler, ScreenHandlerBehaviour},
    slot::NormalSlot,
};

/// 创建信标容器屏幕处理器。
///
/// 信标只有一个支付槽位，以及用于选择状态效果的专用界面。
pub fn create_beacon_handler(
    sync_id: u8,
    player_inventory: &Arc<PlayerInventory>,
    inventory: Arc<dyn Inventory>,
) -> BeaconScreenHandler {
    BeaconScreenHandler::new(sync_id, player_inventory, inventory)
}

/// 专门用于信标方块的屏幕处理器。
pub struct BeaconScreenHandler {
    /// 信标的物品栏（恰好包含 1 个支付槽位）。
    pub inventory: Arc<dyn Inventory>,
    /// 核心屏幕处理器行为（槽位、同步 ID、监听器）。
    behaviour: ScreenHandlerBehaviour,
}

impl BeaconScreenHandler {
    /// 创建新的信标屏幕处理器。
    fn new(
        sync_id: u8,
        player_inventory: &Arc<PlayerInventory>,
        inventory: Arc<dyn Inventory>,
    ) -> Self {
        let mut handler = Self {
            inventory,
            behaviour: ScreenHandlerBehaviour::new(sync_id, Some(WindowType::Beacon)),
        };

        handler.inventory.on_open();

        // 为信标添加单个支付槽（槽位 0）
        handler.add_slot(Arc::new(NormalSlot::new(handler.inventory.clone(), 0)));

        // 添加玩家的背包槽位（27 个储物槽 + 9 个快捷栏）
        let player_inventory_arc: Arc<dyn Inventory> = player_inventory.clone();
        handler.add_player_slots(&player_inventory_arc);

        handler
    }
}

impl ScreenHandler for BeaconScreenHandler {
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
        self.inventory.on_close();
    }

    /// 专用于信标界面的快速移动逻辑。
    ///
    /// - 从信标支付槽位 (0)：移到玩家物品栏
    /// - 从玩家物品栏 (1+)：移到信标支付槽位
    fn quick_move(&mut self, _player: &dyn InventoryPlayer, slot_index: i32) -> ItemStack {
        let mut stack_left = ItemStack::EMPTY.clone();
        let slot = self.get_behaviour().slots[slot_index as usize].clone();

        if slot.has_stack() {
            let mut slot_stack = slot.get_stack();
            stack_left = slot_stack.clone();

            if slot_index == 0 {
                // 从单个信标槽移到玩家物品栏（槽位 1 至末尾）
                if !self.insert_item(
                    &mut slot_stack,
                    1,
                    self.get_behaviour().slots.len() as i32,
                    true,
                ) {
                    return ItemStack::EMPTY.clone();
                }
            } else {
                // 从玩家物品栏移入信标付款槽（槽位 0）
                if !self.insert_item(&mut slot_stack, 0, 1, false) {
                    return ItemStack::EMPTY.clone();
                }
            }

            if slot_stack.is_empty() {
                slot.set_stack(ItemStack::EMPTY.clone());
            } else {
                slot.set_stack(slot_stack);
            }
        }

        stack_left
    }
}
