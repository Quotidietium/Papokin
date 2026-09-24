use std::any::Any;
use std::sync::Arc;

use crate::inventory::Inventory;
use papokin_data::data_component_impl::EquipmentSlot;
use papokin_data::item_stack::ItemStack;

use crate::player::player_inventory::PlayerInventory;
use crate::screen_handler::{InventoryPlayer, ScreenHandler, ScreenHandlerBehaviour};
use crate::slot::{ArmorSlot, NormalSlot};

/// 羊驼坐骑界面（对齐原版 `LlamaScreen` 布局）：槽 0 为驼物
/// （地毯，绑 `BODY` 装备槽的单格适配器），其后为驮箱格
/// （按 strength 3-15 格截断暴露），最后是玩家物品栏。
/// 槽位序必须与客户端 `LlamaScreenHandler` 一致，否则同步错位。
pub struct LlamaScreenHandler {
    behaviour: ScreenHandlerBehaviour,
    pub chest_inventory: Arc<dyn Inventory>,
    /// 实际暴露的驮箱格数（底层物品栏 15 格按 strength 截断）。
    /// `quick_move` 的槽位分区必须按暴露数算，否则截断时玩家槽
    /// 起点错位、点击错槽。
    exposed_chest_slots: usize,
}

impl LlamaScreenHandler {
    pub const SLOT_CARPET: usize = 0;
    pub const SLOT_CHEST_START: usize = 1;

    /// 原版带箱羊驼的驮箱格数：strength（1-5）× 3 列。
    #[must_use]
    pub const fn get_chest_slot_count(strength: i32) -> usize {
        let strength = if strength < 1 {
            1
        } else if strength > 5 {
            5
        } else {
            strength
        };
        (strength * 3) as usize
    }

    #[allow(clippy::needless_pass_by_value)]
    pub fn new(
        sync_id: u8,
        player_inventory: &Arc<PlayerInventory>,
        carpet_inventory: Arc<dyn Inventory>,
        chest_inventory: Arc<dyn Inventory>,
        chest_slots: usize,
    ) -> Self {
        let mut handler = Self {
            behaviour: ScreenHandlerBehaviour::new(sync_id, None),
            chest_inventory,
            exposed_chest_slots: 0,
        };

        handler.add_slot(Arc::new(ArmorSlot::new(
            carpet_inventory,
            0,
            EquipmentSlot::BODY,
        )));

        let mount_size = handler.chest_inventory.size().min(chest_slots);
        handler.exposed_chest_slots = mount_size;
        for i in 0..mount_size {
            handler.add_slot(Arc::new(NormalSlot::new(
                handler.chest_inventory.clone(),
                i,
            )));
        }

        let pi: Arc<dyn Inventory> = player_inventory.clone();
        handler.add_player_slots(&pi);

        handler
    }
}

impl ScreenHandler for LlamaScreenHandler {
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

            let chest_size = self.exposed_chest_slots as i32;
            let player_container_start = 1 + chest_size;
            let total_slots = self.get_behaviour().slots.len() as i32;

            if slot_index < player_container_start {
                if !self.insert_item(&mut stack, player_container_start, total_slots, true) {
                    return ItemStack::EMPTY.clone();
                }
            } else if self.get_behaviour().slots[0].can_insert(&stack)
                && !self.get_behaviour().slots[0].has_stack()
            {
                if !self.insert_item(&mut stack, 0, 1, false) {
                    return ItemStack::EMPTY.clone();
                }
            } else if chest_size == 0
                || !self.insert_item(&mut stack, 1, player_container_start, false)
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

    fn llama_handler(
        player_inventory: &Arc<PlayerInventory>,
        chest_slots: usize,
    ) -> (LlamaScreenHandler, Arc<SimpleInventory>) {
        let chest = Arc::new(SimpleInventory::new(15));
        (
            LlamaScreenHandler::new(
                1,
                player_inventory,
                Arc::new(SimpleInventory::new(1)),
                chest.clone(),
                chest_slots,
            ),
            chest,
        )
    }

    fn total_stone(handler: &LlamaScreenHandler) -> u32 {
        handler
            .get_behaviour()
            .slots
            .iter()
            .filter(|s| s.get_cloned_stack().item.id == Item::STONE.id)
            .map(|s| u32::from(s.get_cloned_stack().item_count))
            .sum()
    }

    /// 驮箱格数按 strength 截断暴露（3-15 格），处理器槽位总数
    /// 必须与 1（地毯）+ 暴露格数 + 玩家 36 格一致——多暴露会
    /// 与客户端 `LlamaScreen` 布局错位，点击错槽。
    #[test]
    fn chest_slots_are_clamped_by_strength() {
        assert_eq!(LlamaScreenHandler::get_chest_slot_count(1), 3);
        assert_eq!(LlamaScreenHandler::get_chest_slot_count(5), 15);
        // 越界 strength 不产生越界格数
        assert_eq!(LlamaScreenHandler::get_chest_slot_count(0), 3);
        assert_eq!(LlamaScreenHandler::get_chest_slot_count(9), 15);

        let player_inventory = player_inventory();
        let (handler, _) = llama_handler(&player_inventory, 6);
        assert_eq!(handler.get_behaviour().slots.len(), 1 + 6 + 36);
        // 截断不得超出底层物品栏容量（15 格）
        let (handler, _) = llama_handler(&player_inventory, 99);
        assert_eq!(handler.get_behaviour().slots.len(), 1 + 15 + 36);
    }

    /// 带箱羊驼形态：shift 点击玩家快捷栏物品必须落进驮箱格且
    /// 总量守恒（不能复制也不能凭空消失）。
    #[test]
    fn quick_move_moves_player_stack_into_chest_slots() {
        let player_inventory = player_inventory();
        player_inventory.set_stack(0, ItemStack::new(64, &Item::STONE));
        let player = TestPlayer {
            inventory: player_inventory.clone(),
        };
        let (mut handler, chest) = llama_handler(&player_inventory, 15);

        // 1（地毯）+ 15（驮箱）+ 27（主物品栏）= 43 为快捷栏 0
        handler.quick_move(&player, 43);

        assert!(player_inventory.get_stack(0).is_empty(), "源槽位必须清空");
        assert_eq!(total_stone(&handler), 64, "物品总数必须守恒");
        let chest_stone: u32 = (0..15)
            .map(|i| u32::from(chest.get_stack(i).item_count))
            .sum();
        assert_eq!(chest_stone, 64, "石头必须全部落入驮箱格");
    }

    /// 驮箱格物品 shift 点击移入玩家背包且总量守恒。
    #[test]
    fn quick_move_moves_chest_stack_to_player() {
        let player_inventory = player_inventory();
        let player = TestPlayer {
            inventory: player_inventory.clone(),
        };
        let (mut handler, chest) = llama_handler(&player_inventory, 15);
        chest.set_stack(0, ItemStack::new(32, &Item::DIRT));

        // 驮箱格 0 对应处理器槽位 1
        handler.quick_move(&player, 1);

        assert!(chest.get_stack(0).is_empty(), "驮箱源槽位必须清空");
        let dirt_in_player: u32 = (0..36)
            .map(|i| u32::from(player_inventory.get_stack(i).item_count))
            .sum();
        assert_eq!(dirt_in_player, 32, "泥土必须全部落入玩家背包");
    }

    /// 地毯槽只接受地毯类物品（BODY 装备白名单），石头不得进入。
    #[test]
    fn carpet_slot_only_accepts_carpets() {
        let player_inventory = player_inventory();
        let (handler, _) = llama_handler(&player_inventory, 0);
        let carpet_slot = &handler.get_behaviour().slots[0];

        assert!(
            carpet_slot.can_insert(&ItemStack::new(1, &Item::WHITE_CARPET)),
            "白地毯应可放入驼物槽"
        );
        assert!(
            !carpet_slot.can_insert(&ItemStack::new(1, &Item::STONE)),
            "石头不得放入驼物槽"
        );
    }
}
