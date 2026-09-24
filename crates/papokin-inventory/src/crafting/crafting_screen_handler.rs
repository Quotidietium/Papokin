//! 合成屏幕处理器实现。
//!
//! 此模块为合成机制提供屏幕处理器：
//! - [`CraftingScreenHandler`] - 合成屏幕处理器的 trait
//! - [`CraftingTableScreenHandler`] - 3x3 合成台界面
//! - [`ResultSlot`] - 显示合成产物的特殊结果槽位
//!
//! # Recipe Matching
//!
//! 合成配方会与合成网格中的物品进行匹配。
//! 该系统支持：
//! - 有序配方（特定图案）
//! - 无序配方（任意摆放）
//! - 转化配方（升级物品）
//! - 特殊配方（如饰纹陶罐）

use std::any::Any;
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::{Arc, Mutex};

use super::recipe_provider::{GenericRecipe, RecipeProvider};
use super::recipes::{RecipeFinderScreenHandler, RecipeInputInventory};
use crate::crafting::crafting_inventory::CraftingInventory;
use crate::player::player_inventory::PlayerInventory;
use crate::screen_handler::{
    InventoryPlayer, ScreenHandler, ScreenHandlerBehaviour, ScreenHandlerListener,
};
use crate::slot::{NormalSlot, Slot};

use crate::inventory::Inventory;
use papokin_data::item_stack::ItemStack;
use papokin_data::recipes::{CraftingRecipeTypes, RECIPES_CRAFTING};
use papokin_data::screen::WindowType;
use papokin_data::statistic::StatisticCategory;
use papokin_data::tag;
use papokin_data::tag::Taggable;
use papokin_protocol::codec::recipe::{DynamicRecipe, OwnedCraftingRecipe};

/// 合成界面中的结果槽位。
pub struct ResultSlot {
    /// 提供配方输入的合成物品栏（网格）。
    pub inventory: Arc<dyn RecipeInputInventory>,
    /// 此槽位的协议 ID（由界面处理器分配）。
    pub id: AtomicU8,
    /// 缓存的结果物品堆。
    pub result: Arc<Mutex<ItemStack>>,
    /// 动态配方的提供者。
    pub recipe_provider: Option<Arc<dyn RecipeProvider>>,
}

pub struct RecipeResult {
    pub item_id: String,
    pub count: u8,
    /// 嬗变配方（染色等）的输入堆叠：结果需在其上换物品 id 以保留
    /// 组件（潜影盒内容物、收纳袋内容、自定义名称等），非嬗变配方为 None。
    pub transmute_source: Option<ItemStack>,
}

/// 检查配方图案是否水平对称。
fn is_symmetrical_horizontally(pattern: &[&str]) -> bool {
    let width = pattern.first().map_or(0, |s| s.len());
    for row in pattern {
        if row.len() != width {
            return false;
        }
        for j in 0..width / 2 {
            if row.chars().nth(j) != row.chars().nth(width - j - 1) {
                return false;
            }
        }
    }
    true
}

/// 检查合成配方是否与当前物品栏状态匹配。
#[expect(clippy::too_many_lines)]
fn recipe_matches(
    recipe: GenericRecipe<'_>,
    input_height: usize,
    input_width: usize,
    top_x: usize,
    top_y: usize,
    count: usize,
    inventory: &dyn RecipeInputInventory,
) -> Option<RecipeResult> {
    match recipe {
        GenericRecipe::Vanilla(CraftingRecipeTypes::CraftingShaped {
            key,
            pattern,
            result,
            ..
        }) => {
            #[allow(clippy::redundant_closure_for_method_calls)]
            if pattern.len() != input_height
                || pattern.first().map_or(0, |f| f.len()) != input_width
            {
                return None;
            }

            if count
                != pattern
                    .iter()
                    .map(|l| l.chars().filter(|c| *c != ' ').count())
                    .sum::<usize>()
            {
                return None;
            }

            let x_offset = top_x;
            let y_offset = top_y;

            let mut matched = true;
            'outer: for (y, row_str) in pattern.iter().enumerate() {
                for (x, current_key) in row_str.chars().enumerate() {
                    let slot = inventory
                        .get_stack((y + y_offset) * inventory.get_width() + (x + x_offset));
                    if current_key == ' ' {
                        if !slot.is_empty() {
                            matched = false;
                            break 'outer;
                        }
                        continue;
                    }

                    let Some(ingredient) = key
                        .iter()
                        .find_map(|(k, v)| (*k == current_key).then_some(v))
                    else {
                        matched = false;
                        break 'outer;
                    };

                    if !ingredient.match_item(slot.item) {
                        matched = false;
                        break 'outer;
                    }
                }
            }

            if !matched && !is_symmetrical_horizontally(pattern) {
                matched = true;
                'outer: for y in 0..pattern.len() {
                    for x in 0..pattern[y].len() {
                        let Some(current_key) = pattern[y].chars().nth(x) else {
                            matched = false;
                            break 'outer;
                        };
                        let slot = inventory.get_stack(
                            (y + y_offset) * inventory.get_height()
                                + (x_offset + input_width - 1 - x),
                        );
                        if current_key == ' ' {
                            if !slot.is_empty() {
                                matched = false;
                                break 'outer;
                            }
                            continue;
                        }
                        let Some(ingredient) = key
                            .iter()
                            .find_map(|(k, v)| (*k == current_key).then_some(v))
                        else {
                            matched = false;
                            break 'outer;
                        };
                        if !ingredient.match_item(slot.item) {
                            matched = false;
                            break 'outer;
                        }
                    }
                }
            }

            matched.then_some(RecipeResult {
                item_id: result.id.to_string(),
                count: result.count,
                transmute_source: None,
            })
        }
        GenericRecipe::Vanilla(CraftingRecipeTypes::CraftingShapeless {
            ingredients,
            result,
            ..
        }) => {
            if count != ingredients.len() {
                return None;
            }
            let mut ingredient_used = vec![false; ingredients.len()];
            'next_slot: for i in 0..inventory.size() {
                let slot = inventory.get_stack(i);
                if slot.is_empty() {
                    continue 'next_slot;
                }
                for i in 0..ingredients.len() {
                    if !ingredient_used[i] && ingredients[i].match_item(slot.item) {
                        ingredient_used[i] = true;
                        continue 'next_slot;
                    }
                }
                return None;
            }
            Some(RecipeResult {
                item_id: result.id.to_string(),
                count: result.count,
                transmute_source: None,
            })
        }
        GenericRecipe::Vanilla(CraftingRecipeTypes::CraftingTransmute {
            input,
            material,
            result,
            ..
        }) => {
            if count != 2 {
                return None;
            }
            // 原版要求恰好一个输入与一个材料：两个都匹配输入（例如
            // 两只潜影盒而无染料）不得合成。输入堆叠需记录下来，
            // 供结果继承其组件。
            let mut source: Option<ItemStack> = None;
            let mut has_material = false;
            for i in 0..inventory.size() {
                let slot = inventory.get_stack(i);
                if slot.is_empty() {
                    continue;
                }
                if source.is_none() && input.match_item(slot.item) {
                    source = Some(slot);
                } else if !has_material && material.match_item(slot.item) {
                    has_material = true;
                } else {
                    return None;
                }
            }
            if source.is_none() || !has_material {
                return None;
            }
            Some(RecipeResult {
                item_id: result.id.to_string(),
                count: result.count,
                transmute_source: source,
            })
        }
        GenericRecipe::Vanilla(CraftingRecipeTypes::CraftingDecoratedPot { .. }) => {
            if count != 4 || inventory.get_width() != 3 || inventory.get_height() != 3 {
                return None;
            }
            for position in (1..=7).step_by(2) {
                let slot = inventory.get_stack(position);
                if slot.is_empty()
                    || !slot
                        .item
                        .has_tag(&tag::Item::MINECRAFT_DECORATED_POT_INGREDIENTS)
                {
                    return None;
                }
            }
            Some(RecipeResult {
                item_id: "minecraft:decorated_pot".to_string(),
                count: 1,
                transmute_source: None,
            })
        }
        GenericRecipe::Dynamic(OwnedCraftingRecipe::Shaped {
            pattern,
            key,
            result,
            ..
        }) => {
            #[allow(clippy::redundant_closure_for_method_calls)]
            if pattern.len() != input_height
                || pattern.first().map_or(0, |f| f.len()) != input_width
            {
                return None;
            }
            if count
                != pattern
                    .iter()
                    .map(|l| l.chars().filter(|c| *c != ' ').count())
                    .sum::<usize>()
            {
                return None;
            }
            let x_offset = top_x;
            let y_offset = top_y;
            let mut matched = true;
            'outer: for (y, row_str) in pattern.iter().enumerate() {
                for (x, current_key) in row_str.chars().enumerate() {
                    let slot = inventory
                        .get_stack((y + y_offset) * inventory.get_width() + (x + x_offset));
                    if current_key == ' ' {
                        if !slot.is_empty() {
                            matched = false;
                            break 'outer;
                        }
                        continue;
                    }
                    let Some(ingredient) =
                        key.iter().find(|(k, _)| *k == current_key).map(|(_, v)| v)
                    else {
                        matched = false;
                        break 'outer;
                    };
                    if !ingredient.match_item(slot.item) {
                        matched = false;
                        break 'outer;
                    }
                }
            }
            matched.then_some(RecipeResult {
                item_id: result.item_id.clone(),
                count: result.count,
                transmute_source: None,
            })
        }
        GenericRecipe::Dynamic(OwnedCraftingRecipe::Shapeless {
            ingredients,
            result,
            ..
        }) => {
            if count != ingredients.len() {
                return None;
            }
            let mut ingredient_used = vec![false; ingredients.len()];
            'next_slot: for i in 0..inventory.size() {
                let slot = inventory.get_stack(i);
                if slot.is_empty() {
                    continue 'next_slot;
                }
                for i in 0..ingredients.len() {
                    if !ingredient_used[i] && ingredients[i].match_item(slot.item) {
                        ingredient_used[i] = true;
                        continue 'next_slot;
                    }
                }
                return None;
            }
            Some(RecipeResult {
                item_id: result.item_id.clone(),
                count: result.count,
                transmute_source: None,
            })
        }
        _ => None,
    }
}

#[must_use]
pub fn match_crafting_recipe(
    inventory: &dyn RecipeInputInventory,
    provider: Option<&dyn RecipeProvider>,
) -> Option<RecipeResult> {
    let mut count: usize = 0;
    let inventory_width = inventory.get_width();
    let mut top_x = 9;
    let mut top_y = 9;
    let mut bottom_x = 0;
    let mut bottom_y = 0;
    for i in 0..inventory.size() {
        let x = i % inventory_width;
        let y = i / inventory_width;
        let slot = inventory.get_stack(i);
        if !slot.is_empty() {
            top_x = top_x.min(x);
            top_y = top_y.min(y);
            bottom_x = bottom_x.max(x);
            bottom_y = bottom_y.max(y);
            count += 1;
        }
    }
    if count == 0 {
        return None;
    }
    let input_width = bottom_x + 1 - top_x;
    let input_height = bottom_y + 1 - top_y;

    for recipe in RECIPES_CRAFTING {
        if let Some(result) = recipe_matches(
            GenericRecipe::Vanilla(recipe),
            input_height,
            input_width,
            top_x,
            top_y,
            count,
            inventory,
        ) {
            return Some(result);
        }
    }

    if let Some(provider) = provider {
        let dynamic = provider.get_dynamic_recipes();
        for recipe in &dynamic {
            if let DynamicRecipe::Crafting(crafting) = recipe
                && let Some(result) = recipe_matches(
                    GenericRecipe::Dynamic(crafting),
                    input_height,
                    input_width,
                    top_x,
                    top_y,
                    count,
                    inventory,
                )
            {
                return Some(result);
            }
        }
    }

    None
}

impl ResultSlot {
    pub fn new(
        inventory: Arc<dyn RecipeInputInventory>,
        provider: Option<Arc<dyn RecipeProvider>>,
    ) -> Self {
        Self {
            inventory,
            id: AtomicU8::new(0),
            result: Arc::new(Mutex::new(ItemStack::EMPTY.clone())),
            recipe_provider: provider,
        }
    }

    fn match_recipe(&self) -> Option<RecipeResult> {
        match_crafting_recipe(&*self.inventory, self.recipe_provider.as_deref())
    }

    fn refill_output(&self) -> ItemStack {
        let result = if let Some(matched) = self.match_recipe() {
            let key = matched
                .item_id
                .strip_prefix("minecraft:")
                .unwrap_or(&matched.item_id);
            let item = papokin_data::item::Item::from_registry_key(key)
                .unwrap_or(&papokin_data::item::Item::AIR);
            if let Some(source) = matched.transmute_source {
                // 嬗变配方（染色潜影盒/收纳袋等）：在输入堆叠上仅更换
                // 物品与数量，保留全部组件（内容物、自定义名称等）。
                // copy_with_count 会生成新的堆叠 uid，避免与网格输入撞 id。
                let mut stack = source.copy_with_count(matched.count);
                stack.item = item;
                stack
            } else {
                ItemStack::new(matched.count, item)
            }
        } else {
            ItemStack::EMPTY.clone()
        };
        *self
            .result
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = result.clone();
        result
    }
}

impl Slot for ResultSlot {
    fn get_inventory(&self) -> Arc<dyn Inventory> {
        self.inventory.clone()
    }
    fn get_index(&self) -> usize {
        999
    }
    fn set_id(&self, id: usize) {
        self.id.store(id as u8, Ordering::Relaxed);
    }
    fn on_quick_move_crafted(&self, _stack: ItemStack, _stack_prev: ItemStack) {
        self.refill_output();
    }
    fn on_take_item(&self, player: &dyn InventoryPlayer, stack: &ItemStack) {
        player.increment_stat(
            StatisticCategory::Crafted,
            stack.item.id as i32,
            stack.item_count as i32,
        );
        for i in 0..self.inventory.size() {
            let before = self.inventory.get_stack(i);
            if before.is_empty() {
                continue;
            }
            self.inventory.remove_stack_specific(i, 1);

            // 原版合成剩余物：奶桶→桶、炖菜→碗、蜂蜜瓶→玻璃瓶等。
            // 槽位耗尽时剩余物留置槽内；槽内仍有剩余时返还玩家物品栏
            // （满则掉落），此前两者都被直接吞掉。
            let Some(remainder_id) =
                papokin_data::recipe_remainder::get_recipe_remainder_id(before.item.id)
            else {
                continue;
            };
            let Some(remainder_item) = papokin_data::item::Item::from_id(remainder_id) else {
                continue;
            };
            if before.item_count == 1 {
                self.inventory
                    .set_stack(i, ItemStack::new(1, remainder_item));
            } else {
                player
                    .get_inventory()
                    .offer_or_drop_stack(ItemStack::new(1, remainder_item), player);
            }
        }
        self.mark_dirty();
    }
    fn can_insert(&self, _stack: &ItemStack) -> bool {
        false
    }
    fn get_stack(&self) -> ItemStack {
        self.result
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    }
    fn get_cloned_stack(&self) -> ItemStack {
        self.result
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    }
    fn has_stack(&self) -> bool {
        !self
            .result
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .is_empty()
    }
    fn set_stack(&self, _stack: ItemStack) {
        self.refill_output();
    }
    fn set_stack_prev(&self, _stack: ItemStack, _previous_stack: ItemStack) {
        self.refill_output();
    }
    fn mark_dirty(&self) {
        self.inventory.mark_dirty();
    }
    fn get_max_item_count(&self) -> u8 {
        let mut count = u8::MAX;
        for i in 0..self.inventory.size() {
            let slot = self.inventory.get_stack(i);
            if !slot.is_empty() {
                count = count.min(slot.item_count);
            }
        }
        count
    }
    fn take_stack(&self, _amount: u8) -> ItemStack {
        if self.has_stack() {
            self.result
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .clone()
        } else {
            ItemStack::EMPTY.clone()
        }
    }
}

impl ScreenHandlerListener for ResultSlot {
    fn on_slot_update(&self, screen_handler: &ScreenHandlerBehaviour, slot: u8, _stack: ItemStack) {
        if (0..=(self.inventory.get_width() * self.inventory.get_height()))
            .contains(&(slot as usize))
        {
            let result = self.refill_output();
            let next_revision = screen_handler.next_revision();
            if let Some(sync_handler) = screen_handler.sync_handler.as_ref() {
                sync_handler.update_slot(screen_handler, 0, &result, next_revision);
            }
        }
    }
}

pub trait CraftingScreenHandler<I: RecipeInputInventory>:
    RecipeFinderScreenHandler + ScreenHandler
{
    fn add_recipe_slots(
        &mut self,
        crafing_inventory: Arc<dyn RecipeInputInventory>,
        provider: Option<Arc<dyn RecipeProvider>>,
    ) {
        let result_slot = Arc::new(ResultSlot::new(crafing_inventory.clone(), provider));
        self.add_slot(result_slot.clone());
        let width = crafing_inventory.get_width();
        let height = crafing_inventory.get_height();
        for i in 0..width {
            for j in 0..height {
                let input_slot = NormalSlot::new(crafing_inventory.clone(), j + i * width);
                self.add_slot(Arc::new(input_slot));
            }
        }
        self.add_listener(result_slot);
    }
}

pub struct CraftingTableScreenHandler {
    behaviour: ScreenHandlerBehaviour,
    crafting_inventory: Arc<dyn RecipeInputInventory>,
}

impl CraftingTableScreenHandler {
    pub fn new(
        sync_id: u8,
        player_inventory: &Arc<PlayerInventory>,
        provider: Option<Arc<dyn RecipeProvider>>,
    ) -> Self {
        let crafting_inventory: Arc<dyn RecipeInputInventory> =
            Arc::new(CraftingInventory::new(3, 3));
        let mut crafting_table_handler = Self {
            behaviour: ScreenHandlerBehaviour::new(sync_id, Some(WindowType::Crafting)),
            crafting_inventory: crafting_inventory.clone(),
        };
        crafting_table_handler.add_recipe_slots(crafting_inventory, provider);
        let player_inventory: Arc<dyn Inventory> = player_inventory.clone();
        crafting_table_handler.add_player_slots(&player_inventory);
        crafting_table_handler
    }
}

impl RecipeFinderScreenHandler for CraftingTableScreenHandler {}

impl ScreenHandler for CraftingTableScreenHandler {
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
        self.drop_inventory(player, self.crafting_inventory.clone());
    }
    fn quick_move(&mut self, player: &dyn InventoryPlayer, slot_index: i32) -> ItemStack {
        let slot = self.get_behaviour().slots[slot_index as usize].clone();
        if slot.has_stack() {
            let mut slot_stack = slot.get_stack();
            let stack_prev = slot_stack.clone();
            if slot_index == 0 {
                if !self.insert_item(&mut slot_stack, 10, 46, true) {
                    return ItemStack::EMPTY.clone();
                }
            } else if (1..=9).contains(&slot_index) {
                if !self.insert_item(&mut slot_stack, 10, 46, false) {
                    return ItemStack::EMPTY.clone();
                }
            } else if (10..46).contains(&slot_index) {
                if !self.insert_item(&mut slot_stack, 1, 10, false) {
                    if slot_index < 37 {
                        if !self.insert_item(&mut slot_stack, 37, 46, false) {
                            return ItemStack::EMPTY.clone();
                        }
                    } else if !self.insert_item(&mut slot_stack, 10, 37, false) {
                        return ItemStack::EMPTY.clone();
                    }
                }
            } else if !self.insert_item(&mut slot_stack, 10, 46, false) {
                return ItemStack::EMPTY.clone();
            }
            let stack = slot_stack.clone();
            drop(slot_stack);
            if stack.is_empty() {
                slot.set_stack_prev(ItemStack::EMPTY.clone(), stack_prev.clone());
            } else {
                slot.mark_dirty();
            }
            if stack.item_count == stack_prev.item_count {
                return ItemStack::EMPTY.clone();
            }

            let mut taken_stack = stack_prev.clone();
            taken_stack.set_count(stack_prev.item_count - stack.item_count);
            slot.on_take_item(player, &taken_stack);

            if slot_index == 0 {
                slot.on_quick_move_crafted(stack.clone(), stack_prev.clone());
                if !stack.is_empty() {
                    player.drop_item(stack, false);
                }
            }
            return stack_prev;
        }
        ItemStack::EMPTY.clone()
    }
}

impl CraftingScreenHandler<CraftingInventory> for CraftingTableScreenHandler {}

#[cfg(test)]
mod tests {
    use std::any::Any;
    use std::sync::Mutex;

    use papokin_data::data_component_impl::EquipmentSlot;
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
    use crate::screen_handler::InventoryPlayer;

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

    /// `ResultSlot::safe_take` 必须恰好扣一份原料并返回缓存结果：
    /// 调用方重复触发 `on_take_item` 会双倍吞原料；循环 `safe_take` 则会
    /// 因结果缓存不随取出变化而无限刷物品并挂死线程（两者均已修复，
    /// 此测试固定单次取出语义）。
    #[test]
    fn result_slot_safe_take_consumes_one_ingredient_set() {
        let inventory = Arc::new(CraftingInventory::new(2, 2));
        inventory.set_stack(0, ItemStack::new(2, &Item::OAK_LOG));
        inventory.set_stack(1, ItemStack::new(1, &Item::OAK_LOG));
        let slot = ResultSlot::new(inventory.clone(), None);
        *slot
            .result
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) =
            ItemStack::new(4, &Item::OAK_PLANKS);

        let player_inventory = Arc::new(PlayerInventory::new(
            Arc::new(Mutex::new(EntityEquipment::new())),
            Arc::new(rustc_hash::FxHashMap::default()),
        ));
        let player = TestPlayer {
            inventory: player_inventory,
        };

        let taken = slot.safe_take(4, u8::MAX, &player);
        assert_eq!(taken.item_count, 4);
        assert_eq!(taken.item.id, Item::OAK_PLANKS.id);
        // 每个输入槽恰好扣 1 个，不多不少
        assert_eq!(inventory.get_stack(0).item_count, 1);
        assert_eq!(inventory.get_stack(1).item_count, 0);
    }

    /// 收纳袋右键交互不得作用于结果槽：吸入结果会绕过 `on_take_item`
    /// （不消耗原料），且结果被配方重算补满，形成无限复制。门控后
    /// 右键点击结果槽应保持结果、原料与袋内容三方均不变。
    #[test]
    fn bundle_cursor_interaction_is_blocked_on_result_slot() {
        use papokin_data::data_component_impl::BundleContentsImpl;
        use papokin_protocol::java::server::play::SlotActionType;

        let player_inventory = Arc::new(PlayerInventory::new(
            Arc::new(Mutex::new(EntityEquipment::new())),
            Arc::new(rustc_hash::FxHashMap::default()),
        ));
        let player = TestPlayer {
            inventory: player_inventory,
        };
        let mut handler = CraftingTableScreenHandler::new(1, &player.inventory, None);

        // 竖排两块橡木木板 = 木棍配方（3x3 网格的 0 与 3 号位）
        let grid = handler.get_behaviour().slots[1].get_inventory();
        grid.set_stack(0, ItemStack::new(1, &Item::OAK_PLANKS));
        grid.set_stack(3, ItemStack::new(1, &Item::OAK_PLANKS));
        handler.send_content_updates();
        let result_slot = handler.get_behaviour().slots[0].clone();
        assert_eq!(result_slot.get_cloned_stack().item.id, Item::STICK.id);

        // 光标放一只装有 8 块石头的收纳袋
        let mut bundle = ItemStack::new(1, &Item::BUNDLE);
        assert!(
            bundle
                .get_data_component_mut::<BundleContentsImpl>()
                .expect("收纳袋应默认带有内容组件")
                .try_insert(&mut ItemStack::new(8, &Item::STONE))
        );
        *handler
            .get_behaviour_mut()
            .cursor_stack
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = bundle;

        handler.on_slot_click(0, 1, SlotActionType::Pickup, &player);

        // 结果槽、原料网格与袋内容都必须保持不变
        assert_eq!(result_slot.get_cloned_stack().item.id, Item::STICK.id);
        assert_eq!(result_slot.get_cloned_stack().item_count, 4);
        assert_eq!(grid.get_stack(0).item_count, 1);
        assert_eq!(grid.get_stack(3).item_count, 1);
        let cursor = handler
            .get_behaviour()
            .cursor_stack
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let contents = cursor
            .get_data_component::<BundleContentsImpl>()
            .expect("光标应仍是收纳袋");
        assert_eq!(contents.items.len(), 1);
        assert_eq!(contents.items[0].item.id, Item::STONE.id);
        assert_eq!(contents.items[0].item_count, 8);
    }

    /// 对照组：普通存储槽上的收纳袋右键吸入必须仍然可用，
    /// 且数量守恒（槽位清空、袋内增加相同数量）。
    #[test]
    fn bundle_cursor_sucks_items_from_plain_storage_slot() {
        use papokin_data::data_component_impl::BundleContentsImpl;
        use papokin_protocol::java::server::play::SlotActionType;

        let player_inventory = Arc::new(PlayerInventory::new(
            Arc::new(Mutex::new(EntityEquipment::new())),
            Arc::new(rustc_hash::FxHashMap::default()),
        ));
        // 主物品栏第一格（界面槽位 10）放 8 块石头
        player_inventory.set_stack(9, ItemStack::new(8, &Item::STONE));
        let player = TestPlayer {
            inventory: player_inventory,
        };
        let mut handler = CraftingTableScreenHandler::new(1, &player.inventory, None);

        let mut bundle = ItemStack::new(1, &Item::BUNDLE);
        bundle
            .get_data_component_mut::<BundleContentsImpl>()
            .expect("收纳袋应默认带有内容组件");
        *handler
            .get_behaviour_mut()
            .cursor_stack
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = bundle;

        handler.on_slot_click(10, 1, SlotActionType::Pickup, &player);

        assert!(
            handler.get_behaviour().slots[10]
                .get_cloned_stack()
                .is_empty(),
            "石头应被吸入光标收纳袋"
        );
        let cursor = handler
            .get_behaviour()
            .cursor_stack
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let contents = cursor
            .get_data_component::<BundleContentsImpl>()
            .expect("光标应仍是收纳袋");
        assert_eq!(contents.get_weight(), 8);
        assert_eq!(contents.items[0].item.id, Item::STONE.id);
    }
}
