use papokin_data::{item_stack::ItemStack, recipes::CookingRecipe};

use crate::block::entities::{BlockEntity, PropertyDelegate};
pub use papokin_inventory::ExperienceContainer;
use papokin_inventory::{Clearable, Inventory};

/// 用于从烧炼类方块实体中提取熔炼经验的 trait。
pub trait CookingBlockEntityBase:
    Sync + Send + Inventory + PropertyDelegate + BlockEntity + Clearable
{
    fn get_cooking_time_spent(&self) -> u16;
    fn get_cooking_total_time(&self) -> u16;
    fn get_lit_time_remaining(&self) -> u16;
    fn get_lit_total_time(&self) -> u16;

    /// 记录某配方已被使用（用于取出物品时计算经验）
    /// 使用结果物品 ID 作为配方标识符
    fn add_recipe_used(&self, recipe: &CookingRecipe);
    /// 提取并重置累积的经验，将总量作为整数返回
    /// 根据记录的配方计算 XP，并清空 `recipes_used` 映射
    fn extract_experience_from_recipes(&self) -> i32;

    fn get_input_item(&self) -> ItemStack;
    fn get_fuel_item(&self) -> ItemStack;
    fn get_output_item(&self) -> ItemStack;

    fn set_cooking_time_spent(&self, spent_time: u16);
    fn set_cooking_total_time(&self, total_time: u16);
    fn set_lit_time_remaining(&self, remaining_time: u16);
    fn set_lit_total_time(&self, total_time: u16);

    fn is_burning(&self) -> bool;
    fn can_accept_recipe_output(&self, recipe: Option<&CookingRecipe>, max_count: u8) -> bool;
    fn craft_recipe(&self, recipe: Option<&CookingRecipe>) -> bool;
}

#[macro_export]
macro_rules! impl_cooking_block_entity_base {
    ($struct_name:ty) => {
        impl CookingBlockEntityBase for $struct_name {
            fn get_cooking_time_spent(&self) -> u16 {
                self.cooking_time_spent.load(Ordering::Relaxed)
            }

            fn get_cooking_total_time(&self) -> u16 {
                self.cooking_total_time.load(Ordering::Relaxed)
            }

            fn get_lit_time_remaining(&self) -> u16 {
                self.lit_time_remaining.load(Ordering::Relaxed)
            }

            fn get_lit_total_time(&self) -> u16 {
                self.lit_total_time.load(Ordering::Relaxed)
            }

            fn get_input_item(&self) -> ItemStack {
                let items = self
                    .items
                    .read()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                items[0].clone()
            }

            fn get_fuel_item(&self) -> ItemStack {
                let items = self
                    .items
                    .read()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                items[1].clone()
            }

            fn get_output_item(&self) -> ItemStack {
                let items = self
                    .items
                    .read()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                items[2].clone()
            }

            fn set_cooking_time_spent(&self, spent_time: u16) {
                self.cooking_time_spent.store(spent_time, Ordering::Relaxed);
            }

            fn set_cooking_total_time(&self, total_time: u16) {
                self.cooking_total_time.store(total_time, Ordering::Relaxed);
            }

            fn set_lit_time_remaining(&self, remaining_time: u16) {
                self.lit_time_remaining
                    .store(remaining_time, Ordering::Relaxed);
            }

            fn set_lit_total_time(&self, total_time: u16) {
                self.lit_total_time.store(total_time, Ordering::Relaxed);
            }

            fn is_burning(&self) -> bool {
                self.get_lit_time_remaining() > 0
            }

            fn add_recipe_used(&self, recipe: &papokin_data::recipes::CookingRecipe) {
                // 按配方 ID 跟踪配方使用情况，用于计算经验值
                let recipe_id = recipe.recipe_id.to_string();
                let mut recipes = self
                    .recipes_used
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                *recipes.entry(recipe_id).or_insert(0) += 1;
            }

            fn extract_experience_from_recipes(&self) -> i32 {
                // 根据已追踪的配方计算总经验并清空映射（原版行为）
                let mut recipes = self
                    .recipes_used
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                let mut total_xp: f32 = 0.0;
                for (recipe_id, count) in recipes.iter() {
                    // 查找配方的经验值
                    if let Some(xp) = papokin_data::recipes::get_recipe_experience(recipe_id) {
                        total_xp += xp * (*count as f32);
                    }
                }
                recipes.clear();
                total_xp.floor() as i32
            }

            fn can_accept_recipe_output(
                &self,
                recipe: Option<&papokin_data::recipes::CookingRecipe>,
                max_count: u8,
            ) -> bool {
                let Some(recipe) = recipe else { return false };
                let Ok(items) = self.items.try_read() else {
                    return false;
                };

                if items[0].is_empty() {
                    return false;
                }

                let Some(output_item) = papokin_data::item::Item::from_registry_key(
                    recipe
                        .result
                        .id
                        .strip_prefix("minecraft:")
                        .unwrap_or(&recipe.result.id),
                ) else {
                    return false;
                };
                let output_stack = ItemStack::new(recipe.result.count, output_item);

                let side_item_stack = &items[2];
                if side_item_stack.is_empty() {
                    return true;
                }

                // 必须组件级一致且能容纳完整产出数量：仅比对物品 id 会让
                // 产物槽中带有组件的同种物品（如铁砧改名锭）在产出时被
                // 静默吞掉原料；只查 < max 会让 count > 1 的配方产出超堆叠
                side_item_stack.are_items_and_components_equal(&output_stack)
                    && u16::from(side_item_stack.item_count) + u16::from(recipe.result.count)
                        <= u16::from(max_count.min(side_item_stack.get_max_stack_size()))
            }
            fn craft_recipe(&self, recipe: Option<&papokin_data::recipes::CookingRecipe>) -> bool {
                let can_accept_output =
                    self.can_accept_recipe_output(recipe, self.get_max_count_per_stack());
                if let Some(recipe) = recipe {
                    if can_accept_output {
                        let Ok(mut items) = self.items.try_write() else {
                            return false;
                        };
                        let Some(output_item) = papokin_data::item::Item::from_registry_key(
                            recipe
                                .result
                                .id
                                .strip_prefix("minecraft:")
                                .unwrap_or(&recipe.result.id),
                        ) else {
                            return false;
                        };
                        let output_item_stack = ItemStack::new(recipe.result.count, output_item);

                        // 产物必须实际落槽才能消耗原料：预检（读锁）与
                        // 此处写锁之间产物槽可能被界面线程改动，若不做
                        // 放置确认会吞掉原料而不产出（复制/吞物缝隙）。
                        let placed = if items[2].are_equal(ItemStack::EMPTY) {
                            items[2] = output_item_stack;
                            true
                        } else if items[2].are_items_and_components_equal(&output_item_stack) {
                            // 叠加完整产出数量：数据包配方 result.count 可大于 1，
                            // 只加 1 会让原料消耗与产出不守恒
                            items[2].increment(recipe.result.count);
                            true
                        } else {
                            false
                        };
                        if !placed {
                            return false;
                        }

                        // 跟踪配方使用情况以计算经验值（原版 RecipesUsed 格式）
                        self.add_recipe_used(recipe);

                        if items[0].item.id == papokin_data::item::Item::WET_SPONGE.id
                            && !items[1].is_empty()
                            && items[1].item.id == papokin_data::item::Item::BUCKET.id
                        {
                            items[1] = ItemStack::new(1, &papokin_data::item::Item::WATER_BUCKET);
                        }

                        items[0].decrement(1);
                        return true;
                    }
                }

                false
            }
        }
    };
}

#[macro_export]
macro_rules! impl_property_delegate_for_cooking {
    ($struct_name:ty) => {
        impl $crate::block::entities::PropertyDelegate for $struct_name {
            fn get_property(&self, index: i32) -> i32 {
                match index {
                    0 => self.get_lit_time_remaining() as i32,
                    1 => self.get_lit_total_time() as i32,
                    2 => self.get_cooking_time_spent() as i32,
                    3 => self.get_cooking_total_time() as i32,
                    _ => 0,
                }
            }

            fn set_property(&self, index: i32, value: i32) {
                let value = value as u16;
                match index {
                    0 => self.set_lit_time_remaining(value),
                    1 => self.set_lit_total_time(value),
                    2 => self.set_cooking_time_spent(value),
                    3 => self.set_cooking_total_time(value),
                    _ => {}
                }
            }

            fn get_properties_size(&self) -> i32 {
                4
            }
        }
    };
}

#[macro_export]
macro_rules! impl_clearable_for_cooking {
    ($struct_name:ty) => {
        impl papokin_inventory::Clearable for $struct_name {
            fn clear(&self) {
                let mut items = self
                    .items
                    .write()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                items.fill_with(|| ItemStack::EMPTY.clone());
                self.mark_dirty();
            }
        }
    };
}

#[macro_export]
macro_rules! impl_experience_container_for_cooking {
    ($struct_name:ty) => {
        impl $crate::block::entities::furnace_like_block_entity::ExperienceContainer
            for $struct_name
        {
            fn extract_experience(&self) -> i32 {
                // 委托给 CookingBlockEntityBase 方法
                self.extract_experience_from_recipes()
            }
        }
    };
}

#[macro_export]
macro_rules! impl_inventory_for_cooking {
    ($struct_name:ty) => {
        impl papokin_inventory::Inventory for $struct_name {
            fn size(&self) -> usize {
                Self::INVENTORY_SIZE
            }

            fn is_empty(&self) -> bool {
                let items = self
                    .items
                    .read()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                items.iter().all(|s| s.is_empty())
            }

            fn get_stack(&self, slot: usize) -> ItemStack {
                let items = self
                    .items
                    .read()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                items[slot].clone()
            }

            fn remove_stack(&self, slot: usize) -> ItemStack {
                let mut items = self
                    .items
                    .write()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                let removed = std::mem::replace(&mut items[slot], ItemStack::EMPTY.clone());
                self.mark_dirty();
                removed
            }

            fn remove_stack_specific(&self, slot: usize, amount: u8) -> ItemStack {
                let mut items = self
                    .items
                    .write()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                let res = if !items[slot].is_empty() && amount > 0 {
                    items[slot].split(amount)
                } else {
                    ItemStack::EMPTY.clone()
                };
                self.mark_dirty();
                res
            }

            fn set_stack(&self, slot: usize, stack: ItemStack) {
                let mut items = self
                    .items
                    .write()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                let is_same_item = !stack.is_empty()
                    && ItemStack::are_items_and_components_equal(&items[slot], &stack);

                items[slot] = stack.clone();

                if slot == 0 && !is_same_item {
                    if let Some(recipe) = papokin_data::recipes::get_cooking_recipe_with_ingredient(
                        stack.item,
                        CookingRecipeKind::Smelting,
                    ) {
                        self.set_cooking_total_time(recipe.cookingtime as u16);
                    } else {
                        self.set_cooking_total_time(0);
                    }
                    self.set_cooking_time_spent(0);
                }

                // 设置物品堆叠时始终视为背包已变更
                self.mark_dirty();
            }

            fn mark_dirty(&self) {
                self.dirty.store(true, Ordering::Relaxed);
                self.comparator_dirty.store(true, Ordering::Relaxed);
            }

            fn as_any(&self) -> &dyn std::any::Any {
                self
            }
        }
    };
}

#[macro_export]
macro_rules! impl_block_entity_for_cooking {
    ($struct_name:ty,$recipe_kind:expr) => {
        impl $crate::block::entities::BlockEntity for $struct_name {
            #[expect(clippy::too_many_lines)]
            fn tick(
                &self,
                world: &Arc<$crate::world::World>,
            ) {
                let is_burning = self.is_burning();
                let mut is_dirty = false;
                if self.is_burning() {
                    self.lit_time_remaining.fetch_sub(1, Ordering::Relaxed);
                }

                let (top_item, bottom_item) = if let Ok(items_guard) = self.items.try_read() {
                    (items_guard[0].clone(), items_guard[1].clone())
                } else {
                    return;
                };

                let is_top_items_empty = top_item.is_empty();

                let furnace_recipe = papokin_data::recipes::get_cooking_recipe_with_ingredient(
                    top_item.item,
                    $recipe_kind,
                );

                let can_accept_output = self
                    .can_accept_recipe_output(furnace_recipe, self.get_max_count_per_stack());

                let bottom_items_is_empty = bottom_item.is_empty();
                if self.is_burning() || !bottom_items_is_empty && !is_top_items_empty {
                    if !self.is_burning() && can_accept_output {
                        let base_fuel_ticks =
                            papokin_data::fuels::get_item_burn_ticks(bottom_item.item.id)
                                .unwrap_or(0);

                        let adjusted_fuel_ticks = if matches!(
                            $recipe_kind,
                            CookingRecipeKind::Blasting | CookingRecipeKind::Smoking
                        ) {
                            base_fuel_ticks / 2
                        } else {
                            base_fuel_ticks
                        };

                        let mut burn_ticks = adjusted_fuel_ticks;
                        let mut burn_cancelled = false;
                        if let Some(server) = world.server.upgrade() {
                            let mut burn_event = $crate::plugin::api::events::inventory::furnace_burn::FurnaceBurnEvent::new(
                                self.position,
                                bottom_item.item.registry_key.to_string(),
                                adjusted_fuel_ticks as u32,
                            );
                            server.plugin_manager.fire_blocking(&server, &mut burn_event);
                            if burn_event.cancelled {
                                burn_cancelled = true;
                            } else {
                                // 钳制到 u16 范围：插件可上报任意 u32，截断会回绕
                                burn_ticks = burn_event.burn_time.min(u32::from(u16::MAX)) as u16;
                            }
                        }
                        if !burn_cancelled {
                            self.set_lit_time_remaining(burn_ticks);
                            self.set_lit_total_time(burn_ticks);

                            if self.is_burning() {
                                is_dirty = true;
                                if let Ok(mut items_guard) = self.items.try_write() {
                                    if !items_guard[1].is_empty() {
                                        items_guard[1].decrement(1);
                                        if let Some(remainder_id) =
                                            papokin_data::recipe_remainder::get_recipe_remainder_id(
                                                items_guard[1].item.id,
                                            )
                                            && items_guard[1].is_empty()
                                            && let Some(remainder_item) =
                                                papokin_data::item::Item::from_id(remainder_id)
                                        {
                                            items_guard[1] = ItemStack::new(1, remainder_item);
                                        }
                                    }
                                }
                            }
                        }
                    }

                    if self.is_burning() && can_accept_output {
                        let mut start_cancelled = false;
                        if self.get_cooking_time_spent() == 0 {
                            if let Some(server) = world.server.upgrade() {
                                // 初始总时长必须取自配方：新熔炉的 cooking_total_time
                                // 为 0，直接传入会让首次熔炼永远无法完成
                                // （spent 要等 u16 回绕后才等于 0）。
                                let recipe_cook_time = furnace_recipe
                                    .map_or(0, |recipe| recipe.cookingtime.max(0) as u32);
                                let mut start_event = $crate::plugin::api::events::inventory::furnace_start_smelt::FurnaceStartSmeltEvent::new(
                                    self.position,
                                    top_item.item.registry_key.to_string(),
                                    recipe_cook_time,
                                );
                                server.plugin_manager.fire_blocking(&server, &mut start_event);
                                if start_event.cancelled {
                                    start_cancelled = true;
                                } else {
                                    // 钳制到 u16 范围，理由同燃料时长
                                    self.set_cooking_total_time(
                                        start_event.cooking_time.min(u32::from(u16::MAX)) as u16,
                                    );
                                }
                            } else if let Some(recipe) = furnace_recipe {
                                // 无服务器句柄时（不应发生）也要保证总时长来自配方
                                self.set_cooking_total_time(recipe.cookingtime.max(0) as u16);
                            }
                        }
                        if !start_cancelled {
                            self.cooking_time_spent.fetch_add(1, Ordering::Relaxed);

                            // 用 >= 而非 ==：插件下调总时长后 spent 可能已超过它
                            if self.get_cooking_time_spent() >= self.get_cooking_total_time() {
                                self.set_cooking_time_spent(0);
                                if let Some(cooking_recipe) = furnace_recipe {
                                    // 数据包配方可给出负值/超大值，钳制到 u16 范围
                                    self.set_cooking_total_time(
                                        cooking_recipe.cookingtime.clamp(0, i32::from(u16::MAX))
                                            as u16,
                                    );

                                    let mut smelt_cancelled = false;
                                    if let Some(server) = world.server.upgrade() {
                                        let mut smelt_event = $crate::plugin::api::events::inventory::furnace_smelt::FurnaceSmeltEvent::new(
                                            self.position,
                                            top_item.item.registry_key.to_string(),
                                            cooking_recipe.result.id.to_string(),
                                        );
                                        server.plugin_manager.fire_blocking(&server, &mut smelt_event);
                                        if smelt_event.cancelled {
                                            smelt_cancelled = true;
                                        }
                                    }
                                    if !smelt_cancelled {
                                        self.craft_recipe(Some(cooking_recipe));
                                        is_dirty = true;
                                    }
                                }
                            }
                        }
                    } else {
                        self.set_cooking_time_spent(0);
                    }
                } else if !self.is_burning() && self.get_cooking_time_spent() > 0 {
                    let _ = self.cooking_time_spent.try_update(
                        Ordering::Acquire,
                        Ordering::Acquire,
                        |v| {
                            Some(
                                v.saturating_sub(2)
                                    .min(self.cooking_total_time.load(Ordering::Acquire)),
                            )
                        },
                    );
                }

                if is_burning != self.is_burning() {
                    is_dirty = true;
                    let (furnace_block, furnace_block_state) =
                        world.get_block_and_state(&self.position);
                    let mut props =
                        papokin_data::block_properties::FurnaceLikeProperties::from_state_id(furnace_block_state.id);

                    props.lit = self.is_burning();
                    world.set_block_state(
                        &self.position,
                        props.to_state_id(furnace_block),
                        $crate::world::BlockFlags::NOTIFY_ALL,
                    );
                }

                if is_dirty {
                    self.mark_dirty();
                }
            }

            fn resource_location(&self) -> &'static str {
                Self::ID
            }

            fn get_position(&self) -> BlockPos {
                self.position
            }

            fn from_nbt(nbt: &papokin_nbt::compound::NbtCompound, position: BlockPos) -> Self
            where
                Self: Sized,
            {
                let cooking_total_time = AtomicU16::new(
                    nbt.get_short("cooking_total_time")
                        .map_or(0, |cooking_total_time| cooking_total_time as u16),
                );
                let cooking_time_spent = AtomicU16::new(
                    nbt.get_short("cooking_time_spent")
                        .map_or(0, |cooking_time_spent| cooking_time_spent as u16),
                );
                let lit_total_time = AtomicU16::new(
                    nbt.get_short("lit_total_time")
                        .map_or(0, |lit_total_time| lit_total_time as u16),
                );
                let lit_time_remaining = AtomicU16::new(
                    nbt.get_short("lit_time_remaining")
                        .map_or(0, |lit_time_remaining| lit_time_remaining as u16),
                );
                // 从 NBT 加载 RecipesUsed（原版格式：配方 ID -> 合成次数的映射）
                let mut recipes_used_map = HashMap::new();
                if let Some(recipes_compound) = nbt.get_compound("RecipesUsed") {
                    for (recipe_id, tag) in &recipes_compound.child_tags {
                        if let papokin_nbt::tag::NbtTag::Int(count) = tag {
                            recipes_used_map.insert(recipe_id.to_string(), *count as u32);
                        }
                    }
                }

                let mut furnace = Self {
                    position,
                    dirty: AtomicBool::new(false),
                    comparator_dirty: AtomicBool::new(false),
                    items: std::sync::RwLock::new(from_fn(|_| ItemStack::EMPTY.clone())),
                    cooking_total_time,
                    cooking_time_spent,
                    lit_total_time,
                    lit_time_remaining,
                    recipes_used: std::sync::Mutex::new(recipes_used_map),
                };
                papokin_inventory::sync_read_items_from_nbt(nbt, furnace.items.get_mut().unwrap_or_else(std::sync::PoisonError::into_inner));

                furnace
            }

            fn write_nbt(&self, nbt: &mut papokin_nbt::compound::NbtCompound) {
                nbt.put_short("cooking_total_time", self.get_cooking_total_time() as i16);
                nbt.put_short("cooking_time_spent", self.get_cooking_time_spent() as i16);
                nbt.put_short("lit_total_time", self.get_lit_total_time() as i16);
                nbt.put_short("lit_time_remaining", self.get_lit_time_remaining() as i16);

                // 以原版格式保存 RecipesUsed（配方 ID -> 合成次数 的映射）
                {
                    let recipes = self
                        .recipes_used
                        .lock()
                        .unwrap_or_else(std::sync::PoisonError::into_inner);
                    if !recipes.is_empty() {
                        let mut recipes_compound = papokin_nbt::compound::NbtCompound::new();
                        for (recipe_id, count) in recipes.iter() {
                            recipes_compound.put(
                                recipe_id.as_str(),
                                papokin_nbt::tag::NbtTag::Int(*count as i32),
                            );
                        }
                        nbt.put(
                            "RecipesUsed",
                            papokin_nbt::tag::NbtTag::Compound(recipes_compound),
                        );
                    }
                }

                self.write_inventory_nbt(nbt, true);
            }

            fn get_inventory(
                self: Arc<Self>,
            ) -> Option<Arc<dyn papokin_inventory::Inventory>> {
                Some(self)
            }

            fn is_comparator_dirty(&self) -> bool {
                self.comparator_dirty.load(Ordering::Relaxed)
            }

            fn clear_comparator_dirty(&self) {
                self.comparator_dirty.store(false, Ordering::Relaxed);
            }


            fn is_dirty(&self) -> bool {
                self.dirty.load(Ordering::Relaxed)
            }

            fn chunk_data_nbt(&self) -> Option<papokin_nbt::compound::NbtCompound> {
                let mut nbt = papokin_nbt::compound::NbtCompound::new();
                nbt.put_short("cooking_total_time", self.get_cooking_total_time() as i16);
                nbt.put_short("cooking_time_spent", self.get_cooking_time_spent() as i16);
                nbt.put_short("lit_total_time", self.get_lit_total_time() as i16);
                nbt.put_short("lit_time_remaining", self.get_lit_time_remaining() as i16);

                if let Ok(recipes) = self.recipes_used.lock() {
                    if !recipes.is_empty() {
                        let mut recipes_compound = papokin_nbt::compound::NbtCompound::new();
                        for (recipe_id, count) in recipes.iter() {
                            recipes_compound.put(
                                recipe_id.as_str(),
                                papokin_nbt::tag::NbtTag::Int(*count as i32),
                            );
                        }
                        nbt.put(
                            "RecipesUsed",
                            papokin_nbt::tag::NbtTag::Compound(recipes_compound),
                        );
                    }
                }

                if let Ok(guard) = self.items.try_read() {
                    papokin_inventory::sync_write_items_to_nbt(&*guard, &mut nbt);
                }
                Some(nbt)
            }

            fn as_any(&self) -> &dyn std::any::Any {
                self
            }

            fn to_property_delegate(
                self: Arc<Self>,
            ) -> Option<Arc<dyn $crate::block::entities::PropertyDelegate>> {
                Some(self as Arc<dyn $crate::block::entities::PropertyDelegate>)
            }

            fn to_experience_container(
                self: Arc<Self>,
            ) -> Option<
                Arc<dyn $crate::block::entities::furnace_like_block_entity::ExperienceContainer>,
            > {
                Some(
                    self as Arc<
                        dyn $crate::block::entities::furnace_like_block_entity::ExperienceContainer,
                    >,
                )
            }
        }
    };
}

#[cfg(test)]
mod tests {
    use papokin_data::item::Item;
    use papokin_data::recipes::{
        CookingRecipe, CookingRecipeKind, RecipeCategoryTypes, RecipeIngredientTypes,
        RecipeResultStruct, get_cooking_recipe_with_ingredient,
    };
    use papokin_util::math::position::BlockPos;

    use super::*;
    use crate::block::entities::furnace::FurnaceBlockEntity;

    fn furnace_with(input: ItemStack, output: ItemStack) -> FurnaceBlockEntity {
        let entity = FurnaceBlockEntity::new(BlockPos::new(0, 0, 0));
        let mut items = entity.items.write().unwrap();
        items[0] = input;
        items[2] = output;
        drop(items);
        entity
    }

    fn raw_iron_recipe() -> &'static CookingRecipe {
        get_cooking_recipe_with_ingredient(&Item::RAW_IRON, CookingRecipeKind::Smelting)
            .expect("粗铁烧炼配方必须存在")
    }

    /// 基本守恒：空产物槽时产出一个结果、原料恰减一，并记录配方用量。
    #[test]
    fn craft_recipe_consumes_one_input_and_produces_result() {
        let entity = furnace_with(ItemStack::new(3, &Item::RAW_IRON), ItemStack::EMPTY.clone());
        assert!(entity.craft_recipe(Some(raw_iron_recipe())));
        let items = entity.items.read().unwrap();
        assert_eq!(items[0].item_count, 2);
        assert_eq!(items[2].item.id, Item::IRON_INGOT.id);
        assert_eq!(items[2].item_count, 1);
        drop(items);
        assert_eq!(entity.recipes_used.lock().unwrap().values().sum::<u32>(), 1);
    }

    /// 数据包配方 result.count 可大于 1：叠加时必须加上完整数量，
    /// 否则原料消耗与产出不守恒。
    #[test]
    fn craft_recipe_stacks_full_result_count() {
        let multi_recipe = CookingRecipe {
            recipe_id: "test:multi_output",
            category: RecipeCategoryTypes::Misc,
            group: None,
            ingredient: RecipeIngredientTypes::Simple("minecraft:raw_iron"),
            cookingtime: 100,
            experience: 0.0,
            result: RecipeResultStruct {
                id: "minecraft:iron_ingot",
                count: 4,
            },
        };
        let entity = furnace_with(
            ItemStack::new(2, &Item::RAW_IRON),
            ItemStack::new(1, &Item::IRON_INGOT),
        );
        assert!(entity.craft_recipe(Some(&multi_recipe)));
        let items = entity.items.read().unwrap();
        assert_eq!(items[2].item_count, 5, "1 已有 + 4 产出");
        assert_eq!(items[0].item_count, 1);
    }

    /// 产物槽剩余空间不足完整产出时必须拒绝，不得造成超堆叠。
    #[test]
    fn craft_recipe_rejects_when_output_cannot_fit_full_count() {
        let multi_recipe = CookingRecipe {
            recipe_id: "test:multi_output",
            category: RecipeCategoryTypes::Misc,
            group: None,
            ingredient: RecipeIngredientTypes::Simple("minecraft:raw_iron"),
            cookingtime: 100,
            experience: 0.0,
            result: RecipeResultStruct {
                id: "minecraft:iron_ingot",
                count: 4,
            },
        };
        let entity = furnace_with(
            ItemStack::new(2, &Item::RAW_IRON),
            ItemStack::new(62, &Item::IRON_INGOT),
        );
        assert!(
            !entity.craft_recipe(Some(&multi_recipe)),
            "62 + 4 超过 64 上限时必须拒绝"
        );
        let items = entity.items.read().unwrap();
        assert_eq!(items[0].item_count, 2, "拒绝时原料不得消耗");
        assert_eq!(items[2].item_count, 62);
    }

    /// 产物槽是同种物品但带组件（如铁砧改名锭）时不得产出：
    /// 仅比对物品 id 会让产出无处落槽而吞掉原料。
    #[test]
    fn craft_recipe_rejects_component_mismatched_output() {
        let mut renamed = ItemStack::new(1, &Item::IRON_INGOT);
        renamed
            .get_data_component_mut::<papokin_data::data_component_impl::ItemNameImpl>()
            .expect("默认组件应存在")
            .name = "改名铁锭".into();
        let entity = furnace_with(ItemStack::new(2, &Item::RAW_IRON), renamed);
        assert!(!entity.craft_recipe(Some(raw_iron_recipe())));
        let items = entity.items.read().unwrap();
        assert_eq!(items[0].item_count, 2, "拒绝时原料不得消耗");
        assert_eq!(items[2].item_count, 1);
    }

    /// 烧炼经验按配方累计：粗铁 0.7 xp/个，两个后 floor(1.4) = 1，
    /// 提取后映射必须清空（原版 `RecipesUsed` 语义）。
    #[test]
    fn experience_accumulates_per_recipe_and_clears_on_extract() {
        let entity = furnace_with(ItemStack::new(4, &Item::RAW_IRON), ItemStack::EMPTY.clone());
        let recipe = raw_iron_recipe();
        assert!(entity.craft_recipe(Some(recipe)));
        assert!(entity.craft_recipe(Some(recipe)));
        assert_eq!(entity.extract_experience_from_recipes(), 1);
        assert!(
            entity.recipes_used.lock().unwrap().is_empty(),
            "提取后配方用量映射必须清空"
        );
        assert_eq!(entity.extract_experience_from_recipes(), 0);
    }
}
