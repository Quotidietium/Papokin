use std::any::Any;
use std::sync::Arc;

use crate::inventory::Inventory;
use papokin_data::data_component_impl::EquipmentSlot;
use papokin_data::item_stack::ItemStack;

use crate::player::player_inventory::PlayerInventory;
use crate::screen_handler::{InventoryPlayer, ScreenHandler, ScreenHandlerBehaviour};
use crate::slot::{ArmorSlot, NormalSlot};

pub struct MountScreenHandler {
    behaviour: ScreenHandlerBehaviour,
    pub mount_inventory: Arc<dyn Inventory>,
    pub saddle_inventory: Arc<dyn Inventory>,
    pub armor_inventory: Arc<dyn Inventory>,
    pub inventory_columns: usize,
}

impl MountScreenHandler {
    pub const SLOT_SADDLE: usize = 0;
    pub const SLOT_BODY_ARMOR: usize = 1;
    pub const SLOT_INVENTORY_START: usize = 2;
    pub const INVENTORY_ROWS: usize = 3;

    #[must_use]
    pub const fn get_inventory_size(inventory_columns: usize) -> usize {
        inventory_columns * Self::INVENTORY_ROWS
    }

    pub fn new(
        sync_id: u8,
        player_inventory: &Arc<PlayerInventory>,
        mount_inventory: Arc<dyn Inventory>,
        saddle_inventory: Arc<dyn Inventory>,
        armor_inventory: Arc<dyn Inventory>,
        inventory_columns: usize,
    ) -> Self {
        let mut handler = Self {
            behaviour: ScreenHandlerBehaviour::new(sync_id, None),
            mount_inventory,
            saddle_inventory: saddle_inventory.clone(),
            armor_inventory: armor_inventory.clone(),
            inventory_columns,
        };

        handler.add_slot(Arc::new(ArmorSlot::new(
            saddle_inventory,
            0,
            EquipmentSlot::SADDLE,
        )));
        handler.add_slot(Arc::new(ArmorSlot::new(
            armor_inventory,
            0,
            EquipmentSlot::BODY,
        )));

        let mount_size = handler.mount_inventory.size();
        for i in 0..mount_size {
            handler.add_slot(Arc::new(NormalSlot::new(
                handler.mount_inventory.clone(),
                i,
            )));
        }

        let pi: Arc<dyn Inventory> = player_inventory.clone();
        handler.add_player_slots(&pi);

        handler
    }
}

impl ScreenHandler for MountScreenHandler {
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
    }

    fn quick_move(&mut self, _player: &dyn InventoryPlayer, slot_index: i32) -> ItemStack {
        let mut clicked = ItemStack::EMPTY.clone();
        let slot = self.get_behaviour().slots.get(slot_index as usize).cloned();

        if let Some(slot) = slot {
            if !slot.has_stack() {
                return clicked;
            }

            let mut stack = slot.get_stack();
            clicked = stack.clone();

            let mount_container_size = self.mount_inventory.size();
            let player_container_start = 2 + mount_container_size as i32;
            let total_slots = self.get_behaviour().slots.len() as i32;

            if slot_index < player_container_start {
                if !self.insert_item(&mut stack, player_container_start, total_slots, true) {
                    return ItemStack::EMPTY.clone();
                }
            } else if self.get_behaviour().slots[1].can_insert(&stack)
                && !self.get_behaviour().slots[1].has_stack()
            {
                if !self.insert_item(&mut stack, 1, 2, false) {
                    return ItemStack::EMPTY.clone();
                }
            } else if self.get_behaviour().slots[0].can_insert(&stack)
                && !self.get_behaviour().slots[0].has_stack()
            {
                if !self.insert_item(&mut stack, 0, 1, false) {
                    return ItemStack::EMPTY.clone();
                }
            } else if mount_container_size == 0
                || !self.insert_item(&mut stack, 2, player_container_start, false)
            {
                let player_container_end = player_container_start + 27;
                let player_hotbar_start = player_container_end;
                let player_hotbar_end = player_hotbar_start + 9;

                if (player_hotbar_start..player_hotbar_end).contains(&slot_index) {
                    if !self.insert_item(
                        &mut stack,
                        player_container_start,
                        player_container_end,
                        false,
                    ) {
                        return ItemStack::EMPTY.clone();
                    }
                } else if (player_container_start..player_container_end).contains(&slot_index) {
                    if !self.insert_item(&mut stack, player_hotbar_start, player_hotbar_end, false)
                    {
                        return ItemStack::EMPTY.clone();
                    }
                } else if !self.insert_item(
                    &mut stack,
                    player_hotbar_start,
                    player_container_end,
                    false,
                ) {
                    return ItemStack::EMPTY.clone();
                }
                // 注意：这里绝不能提前返回——insert_item 已把物品复制进
                // 目标槽位，必须落到下方把剩余数量写回源槽位，否则源槽
                // 仍是原物品堆而目标槽多了一份（复制物品 BUG）。
            }

            if stack.is_empty() {
                slot.set_stack(ItemStack::EMPTY.clone());
            } else {
                slot.set_stack(stack);
            }
        }

        clicked
    }
}

pub struct NautilusInventoryScreenHandler {
    pub mount_handler: MountScreenHandler,
}

impl NautilusInventoryScreenHandler {
    pub fn new(
        sync_id: u8,
        player_inventory: &Arc<PlayerInventory>,
        nautilus_inventory: Arc<dyn Inventory>,
        saddle_inventory: Arc<dyn Inventory>,
        armor_inventory: Arc<dyn Inventory>,
        inventory_columns: usize,
    ) -> Self {
        Self {
            mount_handler: MountScreenHandler::new(
                sync_id,
                player_inventory,
                nautilus_inventory,
                saddle_inventory,
                armor_inventory,
                inventory_columns,
            ),
        }
    }
}

impl ScreenHandler for NautilusInventoryScreenHandler {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }

    fn get_behaviour(&self) -> &ScreenHandlerBehaviour {
        self.mount_handler.get_behaviour()
    }

    fn get_behaviour_mut(&mut self) -> &mut ScreenHandlerBehaviour {
        self.mount_handler.get_behaviour_mut()
    }

    fn on_closed(&mut self, player: &dyn InventoryPlayer) {
        self.mount_handler.on_closed(player);
    }

    fn quick_move(&mut self, player: &dyn InventoryPlayer, slot_index: i32) -> ItemStack {
        self.mount_handler.quick_move(player, slot_index)
    }
}

#[cfg(test)]
mod tests {
    use std::any::Any;
    use std::sync::Mutex;

    use papokin_data::item::Item;
    use papokin_data::screen::WindowType;
    use papokin_data::sound::Sound;
    use papokin_data::statistic::StatisticCategory;
    use papokin_protocol::java::client::play::{
        CSetContainerContent, CSetContainerProperty, CSetContainerSlot, CSetCursorItem,
        CSetPlayerInventory, CSetSelectedSlot,
    };

    use super::*;
    use crate::entity_equipment::EntityEquipment;
    use crate::inventory::SimpleInventory;

    struct TestPlayer {
        inventory: Arc<PlayerInventory>,
    }

    impl InventoryPlayer for TestPlayer {
        fn as_any(&self) -> &dyn Any {
            self
        }

        fn drop_item(&self, _item: ItemStack, _retain_ownership: bool) {}

        fn get_inventory(&self) -> Arc<PlayerInventory> {
            self.inventory.clone()
        }

        fn has_infinite_materials(&self) -> bool {
            false
        }

        fn is_creative(&self) -> bool {
            false
        }

        fn experience_level(&self) -> i32 {
            0
        }

        fn add_experience_levels(&self, _levels: i32) {}

        fn enchantment_seed(&self) -> i32 {
            0
        }

        fn set_enchantment_seed(&self, _seed: i32) {}

        fn enqueue_inventory_packet(
            &self,
            _packet: &CSetContainerContent,
            _window_type: Option<WindowType>,
        ) {
        }

        fn enqueue_slot_packet(
            &self,
            _packet: &CSetContainerSlot,
            _window_type: Option<WindowType>,
            _total_slots: usize,
        ) {
        }

        fn enqueue_cursor_packet(&self, _packet: &CSetCursorItem) {}

        fn enqueue_property_packet(&self, _packet: &CSetContainerProperty) {}

        fn enqueue_slot_set_packet(&self, _packet: &CSetPlayerInventory) {}

        fn enqueue_set_held_item_packet(&self, _packet: &CSetSelectedSlot) {}

        fn enqueue_equipment_change(&self, _slot: &EquipmentSlot, _stack: &ItemStack) {}

        fn award_experience(&self, _amount: i32) {}

        fn increment_stat(&self, _category: StatisticCategory, _stat_id: i32, _amount: i32) {}

        fn play_block_sound(&self, _sound: Sound, _pitch: f32) {}
    }

    fn player_inventory() -> Arc<PlayerInventory> {
        Arc::new(PlayerInventory::new(
            Arc::new(Mutex::new(EntityEquipment::new())),
            Arc::new(rustc_hash::FxHashMap::default()),
        ))
    }

    fn horse_handler(player_inventory: &Arc<PlayerInventory>) -> MountScreenHandler {
        MountScreenHandler::new(
            1,
            player_inventory,
            Arc::new(SimpleInventory::new(0)),
            Arc::new(SimpleInventory::new(1)),
            Arc::new(SimpleInventory::new(1)),
            0,
        )
    }

    fn total_stone(handler: &MountScreenHandler) -> u32 {
        handler
            .get_behaviour()
            .slots
            .iter()
            .filter(|s| s.get_cloned_stack().item.id == Item::STONE.id)
            .map(|s| u32::from(s.get_cloned_stack().item_count))
            .sum()
    }

    /// shift 点击移动后必须把剩余写回源槽位：否则目标槽多了一份而
    /// 源槽原封不动（复制物品 BUG）。无箱子坐骑必走这条分支。
    #[test]
    fn quick_move_full_stack_moves_without_duplication() {
        let player_inventory = player_inventory();
        player_inventory.set_stack(0, ItemStack::new(64, &Item::STONE));
        let player = TestPlayer {
            inventory: player_inventory.clone(),
        };
        let mut handler = horse_handler(&player_inventory);

        // 处理器槽位 29 对应背包快捷栏 0
        let moved = handler.quick_move(&player, 29);
        assert_eq!(moved.item_count, 64, "必须返回移动前的物品堆");
        assert!(player_inventory.get_stack(0).is_empty(), "源槽位必须清空");
        assert_eq!(total_stone(&handler), 64, "物品总数必须守恒");
    }

    /// 目标区域只能容纳一部分时：已移动的写入目标，剩余必须写回
    /// 源槽位，两边加总等于移动前数量。
    #[test]
    fn quick_move_partial_stack_writes_back_remainder() {
        let player_inventory = player_inventory();
        // 主物品栏（背包索引 9-35）只留 10 个石头的可合并空间
        player_inventory.set_stack(9, ItemStack::new(54, &Item::STONE));
        for i in 10..36 {
            player_inventory.set_stack(i, ItemStack::new(64, &Item::DIRT));
        }
        player_inventory.set_stack(0, ItemStack::new(64, &Item::STONE));
        let player = TestPlayer {
            inventory: player_inventory.clone(),
        };
        let mut handler = horse_handler(&player_inventory);

        handler.quick_move(&player, 29);

        assert_eq!(
            player_inventory.get_stack(9).item_count,
            64,
            "可合并槽位必须被填满"
        );
        assert_eq!(
            player_inventory.get_stack(0).item_count,
            54,
            "剩余必须写回源槽位而不是源槽原封不动"
        );
        assert_eq!(total_stone(&handler), 118, "物品总数必须守恒（54+64）");
    }
}
