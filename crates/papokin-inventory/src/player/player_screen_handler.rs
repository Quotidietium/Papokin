//! 玩家物品栏屏幕处理器。
//!
//! 本模块处理玩家物品栏界面（按 E 键打开）。
//! 其中包括：
//! - 2x2 合成网格（物品栏内合成）
//! - 盔甲槽位（头部、胸部、腿部、脚部）
//! - 主物品栏（27 个槽位）
//! - 快捷栏（9 个槽位）
//! - 副手槽位
//!
//! # Slot Layout
//!
//! 玩家屏幕处理器使用以下槽位索引：
//! - 0：合成结果
//! - 1-4：合成网格（2x2）
//! - 5-8：盔甲槽位（头部、胸部、腿部、脚部）
//! - 9-35：主物品栏
//! - 36-44：快捷栏
//! - 45：副手

use super::player_inventory::PlayerInventory;
use crate::crafting::crafting_inventory::CraftingInventory;
use crate::crafting::crafting_screen_handler::CraftingScreenHandler;
use crate::crafting::recipes::{RecipeFinderScreenHandler, RecipeInputInventory};
use crate::inventory::Inventory;
use crate::screen_handler::{InventoryPlayer, ScreenHandler, ScreenHandlerBehaviour};
use crate::slot::{ArmorSlot, NormalSlot, Slot};
use papokin_data::data_component_impl::{EquipmentSlot, EquipmentType, EquippableImpl};
use papokin_data::item_stack::ItemStack;
use papokin_data::screen::WindowType;
use std::any::Any;
use std::sync::Arc;

/// 玩家物品栏的屏幕处理器。
///
/// 管理玩家的物品栏 UI，包括合成、盔甲等
/// 主物品栏。这是按下 E 键时显示的默认界面。
pub struct PlayerScreenHandler {
    /// 核心屏幕处理器行为（槽位、同步 ID、监听器）。
    behaviour: ScreenHandlerBehaviour,
    /// 2x2 合成格物品栏。
    crafting_inventory: Arc<dyn RecipeInputInventory>,
}

impl RecipeFinderScreenHandler for PlayerScreenHandler {}

impl CraftingScreenHandler<CraftingInventory> for PlayerScreenHandler {}

// TODO: 完整实现此功能
impl PlayerScreenHandler {
    /// 用于盔甲显示的装备槽位顺序。
    const EQUIPMENT_SLOT_ORDER: [EquipmentSlot; 4] = [
        EquipmentSlot::HEAD,
        EquipmentSlot::CHEST,
        EquipmentSlot::LEGS,
        EquipmentSlot::FEET,
    ];

    /// 检查槽位索引是否位于快捷栏内。
    ///
    /// 快捷栏槽位在协议中为 36-44（0 起始索引的 36-44）。
    #[must_use]
    pub fn is_in_hotbar(slot: u8) -> bool {
        (36..=45).contains(&slot)
    }

    /// 根据索引获取槽位。
    pub fn get_slot(&self, slot: usize) -> Arc<dyn Slot> {
        self.behaviour.slots[slot].clone()
    }

    /// 创建一个新的玩家屏幕处理器。
    ///
    /// # Arguments
    /// - `player_inventory` - 玩家的物品栏
    /// - `window_type` - 窗口类型（玩家物品栏通常为 None）
    /// - `sync_id` - 同步 ID
    pub fn new(
        player_inventory: &Arc<PlayerInventory>,
        window_type: Option<WindowType>,
        sync_id: u8,
        provider: Option<Arc<dyn crate::crafting::recipe_provider::RecipeProvider>>,
    ) -> Self {
        let crafting_inventory: Arc<dyn RecipeInputInventory> =
            Arc::new(CraftingInventory::new(2, 2));

        let mut player_screen_handler = Self {
            behaviour: ScreenHandlerBehaviour::new(sync_id, window_type),
            crafting_inventory: crafting_inventory.clone(),
        };

        player_screen_handler.add_recipe_slots(crafting_inventory, provider);

        // 添加盔甲槽位（头、胸、腿、脚）
        for i in 0..4 {
            player_screen_handler.add_slot(Arc::new(ArmorSlot::new(
                player_inventory.clone(),
                39 - i,
                Self::EQUIPMENT_SLOT_ORDER[i].clone(),
            )));
        }

        let player_inventory: Arc<dyn Inventory> = player_inventory.clone();

        // 添加主背包与快捷栏
        player_screen_handler.add_player_slots(&player_inventory);

        // 副手槽位（玩家物品栏中索引 40，界面处理器中索引 45）
        // TODO: 为副手添加 onEquipStack 回调
        player_screen_handler.add_slot(Arc::new(NormalSlot::new(player_inventory.clone(), 40)));

        player_screen_handler
    }
}

impl ScreenHandler for PlayerScreenHandler {
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
        //TODO: this.craftingResultInventory.clear();
        self.drop_inventory(player, self.crafting_inventory.clone());
    }

    /// 对给定槽位执行快速移动（Shift 点击）。
    ///
    /// 快速移动的逻辑取决于来源槽位：
    /// - 合成结果 (0) -> 玩家物品栏（从末尾开始）
    /// - 合成网格 (1-4) -> 玩家物品栏（从头开始）
    /// - 盔甲槽位 (5-8) -> 玩家物品栏，会卸下装备
    /// - 盔甲物品 -> 盔甲槽位（若为空）
    /// - 副手物品 -> 若为空则放入副手槽位
    /// - 主物品栏（9-35）-> 快捷栏
    /// - 快捷栏 (36-44) -> 主物品栏
    fn quick_move(&mut self, player: &dyn InventoryPlayer, slot_index: i32) -> ItemStack {
        let slot = self.get_behaviour().slots[slot_index as usize].clone();

        // TODO: 可装备组件

        if slot.has_stack() {
            let mut slot_stack = slot.get_stack();
            let stack_prev = slot_stack.clone();

            let equipment_slot = slot_stack
                .get_data_component::<EquippableImpl>()
                .map_or(&EquipmentSlot::MAIN_HAND, |equippable| equippable.slot);

            // 基于源槽位的快速移动逻辑
            let success = if slot_index == 0 {
                // 从合成结果槽（0）-> 玩家物品栏（9-45，从末尾处）
                self.insert_item(&mut slot_stack, 9, 45, true)
            } else if (1..5).contains(&slot_index) {
                // 从合成材料槽（1-4）-> 玩家物品栏（9-45，从起始处）
                self.insert_item(&mut slot_stack, 9, 45, false)
            } else if (5..9).contains(&slot_index) {
                // 从盔甲槽（5-8）-> 玩家物品栏（9-45，从起始处）
                let result = self.insert_item(&mut slot_stack, 9, 45, false);

                if result {
                    player.enqueue_equipment_change(equipment_slot, ItemStack::EMPTY);
                }
                result
            } else if equipment_slot.slot_type() == EquipmentType::HumanoidArmor
                && self
                    .get_slot((8 - equipment_slot.get_entity_slot_id()) as usize)
                    .get_cloned_stack()
                    .is_empty()
            {
                // 放入空的盔甲槽位（5-8）
                let index = 8 - equipment_slot.get_entity_slot_id();
                let result = self.insert_item(&mut slot_stack, index, index + 1, false);

                if result {
                    player.enqueue_equipment_change(equipment_slot, &stack_prev);
                }
                result
            } else if matches!(equipment_slot, EquipmentSlot::OffHand(_))
                && slot_index != 45
                && self.get_slot(45).get_cloned_stack().is_empty()
            {
                // 放入空的副手槽位（45）
                let index = 45;
                self.insert_item(&mut slot_stack, index, index + 1, false)
            } else if (9..36).contains(&slot_index) {
                // 从主物品栏（9-35）-> 快捷栏（36-44）
                self.insert_item(&mut slot_stack, 36, 45, false)
            } else if (36..45).contains(&slot_index) {
                // 从快捷栏（36-44）-> 主物品栏（9-35）
                self.insert_item(&mut slot_stack, 9, 36, false)
            } else {
                // 回退为移动到玩家背包区域
                self.insert_item(&mut slot_stack, 9, 45, false)
            };

            if !success {
                return ItemStack::EMPTY.clone();
            }

            let stack = slot_stack.clone();

            if stack.is_empty() {
                slot.set_stack_prev(ItemStack::EMPTY.clone(), stack_prev.clone());
            } else {
                slot.set_stack(stack.clone());
            }

            if stack.item_count == stack_prev.item_count {
                return ItemStack::EMPTY.clone();
            }

            let mut taken_stack = stack_prev.clone();
            taken_stack.set_count(stack_prev.item_count - stack.item_count);
            slot.on_take_item(player, &taken_stack);

            if slot_index == 0 {
                // 来自合成结果槽（0）
                // 通知结果槽位重新填充
                slot.on_quick_move_crafted(stack.clone(), stack_prev.clone());
                // 对于合成结果槽，丢弃所有剩余物品
                if !stack.is_empty() {
                    player.drop_item(stack, false);
                }
            }

            return stack_prev;
        }

        // 无变化
        ItemStack::EMPTY.clone()
    }
}
