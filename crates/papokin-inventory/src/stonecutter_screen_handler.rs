use std::any::Any;
use std::sync::Arc;
use std::sync::atomic::{AtomicU8, Ordering};

use crate::player::player_inventory::PlayerInventory;
use crate::screen_handler::{InventoryPlayer, ScreenHandler, ScreenHandlerBehaviour};
use crate::slot::{NormalSlot, Slot};

use crate::crafting::recipe_provider::RecipeProvider;
use crate::inventory::Inventory;
use crate::inventory::SimpleInventory;
use papokin_data::item::Item;
use papokin_data::item_stack::ItemStack;
use papokin_data::recipes::{RECIPES_STONECUTTING, StonecutterRecipe};
use papokin_data::screen::WindowType;
use papokin_data::statistic::StatisticCategory;
use papokin_protocol::codec::recipe::DynamicRecipe;
use papokin_protocol::java::server::play::SlotActionType;

/// 一个可用的切石结果，可以是原版（数据驱动）或动态的
/// (由插件提供)。只保留输出：切石机会消耗一个
/// 每个配方的输入物品。
pub struct AvailableStonecuttingRecipe {
    pub result_id: String,
    pub result_count: u8,
}

impl AvailableStonecuttingRecipe {
    fn from_vanilla(recipe: &StonecutterRecipe) -> Self {
        Self {
            result_id: recipe.result.id.to_string(),
            result_count: recipe.result.count,
        }
    }
}

pub struct StonecutterScreenHandler {
    behaviour: ScreenHandlerBehaviour,
    pub input_inventory: Arc<SimpleInventory>,
    pub output_inventory: Arc<SimpleInventory>,
    pub selected_recipe: AtomicU8,
    pub dynamic_recipe_provider: Option<Arc<dyn RecipeProvider>>,
}

impl StonecutterScreenHandler {
    pub fn new(sync_id: u8, player_inventory: &Arc<PlayerInventory>) -> Self {
        Self::with_dynamic_recipe_provider(sync_id, player_inventory, None)
    }

    pub fn with_dynamic_recipe_provider(
        sync_id: u8,
        player_inventory: &Arc<PlayerInventory>,
        dynamic_recipe_provider: Option<Arc<dyn RecipeProvider>>,
    ) -> Self {
        let behaviour = ScreenHandlerBehaviour::new(sync_id, Some(WindowType::Stonecutter));
        let input_inventory = Arc::new(SimpleInventory::new(1));
        let output_inventory = Arc::new(SimpleInventory::new(1));

        let mut handler = Self {
            behaviour,
            input_inventory: input_inventory.clone(),
            output_inventory: output_inventory.clone(),
            selected_recipe: AtomicU8::new(u8::MAX),
            dynamic_recipe_provider,
        };

        handler.add_slot(Arc::new(NormalSlot::new(
            input_inventory.clone() as Arc<dyn Inventory>,
            0,
        )));
        handler.add_slot(Arc::new(StonecutterOutputSlot::new(
            output_inventory as Arc<dyn Inventory>,
            input_inventory as Arc<dyn Inventory>,
            0,
        )));

        let player_inventory: Arc<dyn Inventory> = player_inventory.clone();

        handler.add_player_slots(&player_inventory);

        handler
    }

    fn update_output(&self) {
        let input_lock = self.input_inventory.get_stack(0);

        if input_lock.is_empty() {
            self.output_inventory.set_stack(0, ItemStack::EMPTY.clone());
            self.selected_recipe.store(u8::MAX, Ordering::Relaxed);
            return;
        }

        let available_recipes = self.get_available_recipes(&input_lock);
        let recipe_index = self.selected_recipe.load(Ordering::Relaxed);

        if recipe_index != u8::MAX && (recipe_index as usize) < available_recipes.len() {
            let recipe = &available_recipes[recipe_index as usize];
            let item = Item::from_registry_key(&recipe.result_id).unwrap_or(&Item::AIR);
            let result = ItemStack::new(recipe.result_count, item);
            self.output_inventory.set_stack(0, result);
        } else {
            self.output_inventory.set_stack(0, ItemStack::EMPTY.clone());
        }
    }

    /// 返回 `button_id` 将选择的配方的结果物品 ID，
    /// 按钮 id 未映射到可用配方时返回 `None`。
    #[must_use]
    pub fn recipe_id_for_button(&self, button_id: i32) -> Option<String> {
        if button_id < 0 {
            return None;
        }
        let input = self.input_inventory.get_stack(0);
        let recipes = self.get_available_recipes(&input);
        recipes
            .get(button_id as usize)
            .map(|recipe| recipe.result_id.clone())
    }

    /// 先是原版配方，然后是来自提供者的动态切石配方。
    fn get_available_recipes(&self, input: &ItemStack) -> Vec<AvailableStonecuttingRecipe> {
        let item = input.item;
        let mut available: Vec<AvailableStonecuttingRecipe> = RECIPES_STONECUTTING
            .iter()
            .filter(|r| r.ingredient.match_item(item))
            .map(AvailableStonecuttingRecipe::from_vanilla)
            .collect();

        if let Some(provider) = &self.dynamic_recipe_provider {
            for recipe in provider.get_dynamic_recipes() {
                if let DynamicRecipe::Stonecutting(stonecutting) = recipe
                    && stonecutting.ingredient.match_item(item)
                {
                    available.push(AvailableStonecuttingRecipe {
                        result_id: stonecutting.result.item_id,
                        result_count: stonecutting.result.count,
                    });
                }
            }
        }

        available
    }
}

impl ScreenHandler for StonecutterScreenHandler {
    fn get_behaviour(&self) -> &ScreenHandlerBehaviour {
        &self.behaviour
    }

    fn get_behaviour_mut(&mut self) -> &mut ScreenHandlerBehaviour {
        &mut self.behaviour
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }

    fn on_slot_click(
        &mut self,
        slot_index: i32,
        button: i32,
        action_type: SlotActionType,
        player: &dyn InventoryPlayer,
    ) {
        self.internal_on_slot_click(slot_index, button, action_type, player);
        // 无条件重算：双击收集（PickupAll）可从任意背包槽触发并吸走
        // 输入槽，此前仅点击 0 号槽才重算——输入被吸走后结果槽保持
        // 陈旧非空，取出时对空输入 no-op = 免费产出
        self.update_output();
    }

    fn on_button_click(&mut self, _player: &dyn InventoryPlayer, button_id: i32) -> bool {
        if (0..256).contains(&button_id) && self.recipe_id_for_button(button_id).is_some() {
            self.selected_recipe
                .store(button_id as u8, Ordering::Relaxed);
            self.update_output();
            true
        } else {
            false
        }
    }

    fn on_closed(&mut self, player: &dyn InventoryPlayer) {
        // 归还输入槽物品：此前未覆写 on_closed，输入物品随界面一起
        // 丢弃（关界面/断线/死亡 = 物品凭空消失）
        self.default_on_closed(player);
        self.drop_inventory(player, self.input_inventory.clone());
    }

    fn quick_move(&mut self, player: &dyn InventoryPlayer, slot_index: i32) -> ItemStack {
        let mut stack = ItemStack::EMPTY.clone();
        let slot = self.get_behaviour().slots.get(slot_index as usize).cloned();

        if let Some(slot) = slot {
            let mut slot_stack = slot.get_cloned_stack();
            if !slot_stack.is_empty() {
                stack = slot_stack.clone();
                if slot_index < 2 {
                    // 从切石机到玩家
                    if !self.insert_item(&mut slot_stack, 2, 38, true) {
                        return ItemStack::EMPTY.clone();
                    }
                    slot.on_quick_move_crafted(slot_stack.clone(), stack.clone());
                } else {
                    // 从玩家到切石机
                    // 尝试输入槽（0）
                    if !self.insert_item(&mut slot_stack, 0, 1, false) {
                        return ItemStack::EMPTY.clone();
                    }
                }

                if slot_stack.is_empty() {
                    slot.set_stack(ItemStack::EMPTY.clone());
                } else {
                    slot.set_stack(slot_stack.clone());
                }

                if slot_index == 1 {
                    let mut taken_stack = stack.clone();
                    taken_stack.set_count(stack.item_count - slot_stack.item_count);
                    slot.on_take_item(player, &taken_stack);
                }
            }
        }
        stack
    }
}

pub struct StonecutterOutputSlot {
    pub inventory: Arc<dyn Inventory>,
    pub input_inventory: Arc<dyn Inventory>,
    pub index: usize,
    pub id: AtomicU8,
}

impl StonecutterOutputSlot {
    pub fn new(
        inventory: Arc<dyn Inventory>,
        input_inventory: Arc<dyn Inventory>,
        index: usize,
    ) -> Self {
        Self {
            inventory,
            input_inventory,
            index,
            id: AtomicU8::new(0),
        }
    }
}

impl Slot for StonecutterOutputSlot {
    fn get_inventory(&self) -> Arc<dyn Inventory> {
        self.inventory.clone()
    }

    fn get_index(&self) -> usize {
        self.index
    }

    fn set_id(&self, id: usize) {
        self.id.store(id as u8, Ordering::Relaxed);
    }

    fn on_take_item(&self, player: &dyn InventoryPlayer, stack: &ItemStack) {
        player.increment_stat(
            StatisticCategory::Crafted,
            stack.item.id as i32,
            stack.item_count as i32,
        );
        self.input_inventory.remove_stack_specific(0, 1);
        self.mark_dirty();
    }

    fn can_insert(&self, _stack: &ItemStack) -> bool {
        false
    }

    fn get_stack(&self) -> ItemStack {
        self.inventory.get_stack(self.index)
    }

    fn get_cloned_stack(&self) -> ItemStack {
        self.inventory.get_stack(self.index)
    }

    fn has_stack(&self) -> bool {
        !self.inventory.get_stack(self.index).is_empty()
    }

    fn set_stack(&self, stack: ItemStack) {
        self.inventory.set_stack(self.index, stack);
    }

    fn set_stack_prev(&self, _stack: ItemStack, _previous_stack: ItemStack) {
        // 什么也不做
    }

    fn mark_dirty(&self) {
        self.inventory.mark_dirty();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entity_equipment::EntityEquipment;
    use crate::inventory::Inventory;
    use crate::player::player_inventory::PlayerInventory;
    use crate::screen_handler::{InventoryPlayer, ScreenHandler};
    use papokin_data::item::Item;
    use papokin_data::item_stack::ItemStack;
    use papokin_data::screen::WindowType;
    use papokin_protocol::java::client::play::{
        CSetContainerContent, CSetContainerProperty, CSetContainerSlot, CSetCursorItem,
        CSetPlayerInventory, CSetSelectedSlot,
    };
    use std::any::Any;
    use std::sync::Mutex;

    struct DummyPlayer {
        inventory: Arc<PlayerInventory>,
    }

    impl DummyPlayer {
        fn new() -> Self {
            Self {
                inventory: Arc::new(PlayerInventory::new(
                    Arc::new(Mutex::new(EntityEquipment::new())),
                    Arc::new(rustc_hash::FxHashMap::default()),
                )),
            }
        }
    }

    impl InventoryPlayer for DummyPlayer {
        fn as_any(&self) -> &dyn Any {
            self
        }

        fn drop_item(&self, _item: ItemStack, _retain_ownership: bool) {}

        fn is_creative(&self) -> bool {
            false
        }

        fn has_infinite_materials(&self) -> bool {
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

        fn get_inventory(&self) -> Arc<PlayerInventory> {
            self.inventory.clone()
        }

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

        fn enqueue_equipment_change(
            &self,
            _slot: &papokin_data::data_component_impl::EquipmentSlot,
            _stack: &ItemStack,
        ) {
        }

        fn award_experience(&self, _amount: i32) {}

        fn increment_stat(
            &self,
            _category: papokin_data::statistic::StatisticCategory,
            _stat_id: i32,
            _amount: i32,
        ) {
        }

        fn play_block_sound(&self, _sound: papokin_data::sound::Sound, _pitch: f32) {}
    }

    /// 输入槽被并发路径清空（如双击收集 `PickupAll` 从任意背包槽
    /// 触发）后，任意一次点击都必须重算输出槽——陈旧非空输出 +
    /// 对空输入 no-op 扣料 = 免费产出
    #[test]
    fn any_click_recomputes_output_after_input_drain() {
        let player = DummyPlayer::new();
        let mut handler = StonecutterScreenHandler::new(1, &player.inventory);

        // 放入石头并选中第一个可用配方（按钮 0），输出应有产物
        handler
            .input_inventory
            .set_stack(0, ItemStack::new(4, &Item::STONE));
        assert!(
            <StonecutterScreenHandler as ScreenHandler>::on_button_click(&mut handler, &player, 0),
            "石头至少应有一个可选切石配方"
        );
        assert!(
            !handler.output_inventory.get_stack(0).is_empty(),
            "选中配方后输出槽应有产物"
        );

        // 输入被"偷走"（等价于 PickupAll 吸走），随后一次与输入/
        // 输出无关的普通点击（如背包空槽 Pickup）也必须清空输出
        handler
            .input_inventory
            .set_stack(0, ItemStack::EMPTY.clone());
        <StonecutterScreenHandler as ScreenHandler>::on_slot_click(
            &mut handler,
            10,
            0,
            SlotActionType::Pickup,
            &player,
        );
        assert!(
            handler.output_inventory.get_stack(0).is_empty(),
            "输入清空后输出槽必须重算为空，否则可免费取产物"
        );
    }
}
