use std::any::Any;
use std::sync::{
    Arc, Mutex as StdMutex, RwLock,
    atomic::AtomicI32,
    atomic::{AtomicBool, Ordering},
};

use crate::block::entities::PropertyDelegate;
use papokin_data::data_component_impl::BrewingFuelImpl;
use papokin_data::item::Item;
use papokin_data::item_stack::ItemStack;
use papokin_data::potion::Potion;
use papokin_data::potion_brewing::BREWING_RECIPES;
use papokin_data::sound::{Sound, SoundCategory};
use papokin_inventory::{Inventory, sync_read_items_from_nbt, sync_write_items_to_nbt};
use papokin_nbt::compound::NbtCompound;
use papokin_protocol::codec::recipe::DynamicRecipe;
use papokin_util::math::position::BlockPos;
use papokin_util::math::vector3::Vector3;

pub struct BrewingStandBlockEntity {
    pub position: BlockPos,
    pub items: RwLock<[ItemStack; Self::INVENTORY_SIZE]>,
    pub dirty: AtomicBool,
    pub comparator_dirty: AtomicBool,
    pub brew_time: AtomicI32,
    pub fuel: AtomicI32,
    pub last_potion_count: StdMutex<Option<[bool; 3]>>,
    pub ingredient_item: StdMutex<Option<&'static papokin_data::item::Item>>,
}

impl BrewingStandBlockEntity {
    pub const INVENTORY_SIZE: usize = 5; // 3 个药水槽 + 1 个材料 + 1 个燃料
    pub const ID: &'static str = "minecraft:brewing_stand";

    #[must_use]
    pub fn new(position: BlockPos) -> Self {
        use std::array::from_fn;
        Self {
            position,
            items: RwLock::new(from_fn(|_| ItemStack::EMPTY.clone())),
            dirty: AtomicBool::new(false),
            comparator_dirty: AtomicBool::new(false),
            brew_time: AtomicI32::new(0),
            fuel: AtomicI32::new(0),
            last_potion_count: StdMutex::new(None),
            ingredient_item: StdMutex::new(None),
        }
    }

    /// 原版 `setChanged` 只持久化实体本身，比较器改为响应槽位变化。
    fn mark_timer_dirty(&self) {
        self.dirty.store(true, Ordering::Relaxed);
    }

    /// 检查当前材料是否与已存储的材料匹配
    fn ingredient_matches(&self, ingredient: &ItemStack) -> bool {
        self.ingredient_item
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .is_some_and(|stored| !ingredient.is_empty() && ingredient.get_item().id == stored.id)
    }

    /// 检查任意药水槽位是否存在使用该材料的有效配方
    fn is_brewable(&self, ingredient: &ItemStack, world: &Arc<crate::world::World>) -> bool {
        if ingredient.is_empty() {
            return false;
        }

        let ingredient_id = ingredient.get_item().id;

        // 检查药水配方（水瓶 -> 药水、药水升级等）
        let Ok(items) = self.items.read() else {
            return false;
        };
        for slot_idx in 0..3usize {
            let slot = &items[slot_idx];
            if slot.is_empty() {
                continue;
            }

            let slot_item_id = slot.get_item().id;
            let potion_id = slot
                .get_data_component::<papokin_data::data_component_impl::PotionContentsImpl>()
                .and_then(|pc| pc.potion_id);

            // 1. 检查来自数据包的数据驱动 BREWING_RECIPES
            for recipe in &BREWING_RECIPES {
                if slot_item_id == recipe.from_item().id
                    && ingredient_id == recipe.ingredient().id
                    && potion_id == Some(recipe.from_potion().id as i32)
                {
                    return true;
                }
            }

            // 2. 检查已加载数据包中的动态酿造配方
            if let Some(server) = world.server.upgrade() {
                let dynamic_recipes = server.recipe_manager.get_dynamic_recipes_internal();
                let slot_key = format!("minecraft:{}", slot.get_item().registry_key);
                let ing_key = format!("minecraft:{}", ingredient.get_item().registry_key);
                for dyn_recipe in &dynamic_recipes {
                    if let DynamicRecipe::Brewing(r) = dyn_recipe
                        && r.input_item == slot_key
                        && r.reagent == ing_key
                    {
                        if let Some(req_pot) = &r.input_potion {
                            let current_pot = potion_id
                                .and_then(|id| Potion::from_id(id as u8))
                                .map(|p| format!("minecraft:{}", p.name));
                            if current_pot.as_deref() != Some(req_pot.as_str()) {
                                continue;
                            }
                        }
                        return true;
                    }
                }
            }
        }

        false
    }

    /// 对所有有效的药水槽执行酿造过程
    #[expect(clippy::too_many_lines)]
    fn do_brew(&self, world: &Arc<crate::world::World>, ingredient: &ItemStack) {
        if ingredient.is_empty() {
            return;
        }

        let ingredient_id = ingredient.get_item().id;

        // 在修改物品前触发 BrewEvent
        if let Some(server) = world.server.upgrade() {
            let mut brew_event = crate::plugin::api::events::inventory::brew::BrewEvent::new(
                self.position,
                self.fuel.load(Ordering::Relaxed) as u8,
            );
            server
                .plugin_manager
                .fire_blocking(&server, &mut brew_event);
            if brew_event.cancelled {
                return;
            }
        }

        // 酿造药水槽
        let mut ingredient_used = false;
        {
            let Ok(mut items) = self.items.write() else {
                return;
            };

            for slot_idx in 0..3usize {
                let slot = &mut items[slot_idx];
                if slot.is_empty() {
                    continue;
                }

                let slot_item_id = slot.get_item().id;
                let potion_id = slot
                    .get_data_component::<papokin_data::data_component_impl::PotionContentsImpl>()
                    .and_then(|pc| pc.potion_id);

                // 1. 尝试数据驱动的 BREWING_RECIPES
                let mut item_brewed = false;
                for recipe in &BREWING_RECIPES {
                    if slot_item_id == recipe.from_item().id
                        && recipe.ingredient().id == ingredient_id
                        && potion_id == Some(recipe.from_potion().id as i32)
                    {
                        let mut new_slot = ItemStack::new(1, recipe.to_item());
                        let mut pc = slot
                            .get_data_component::<papokin_data::data_component_impl::PotionContentsImpl>()
                            .cloned()
                            .unwrap_or_else(|| papokin_data::data_component_impl::PotionContentsImpl {
                                potion_id: None,
                                custom_color: None,
                                custom_effects: Vec::new(),
                                custom_name: None,
                            });
                        pc.potion_id = Some(recipe.to_potion().id as i32);
                        new_slot.set_data_component(pc);
                        *slot = new_slot;
                        item_brewed = true;
                        ingredient_used = true;
                        break;
                    }
                }

                if item_brewed {
                    continue;
                }

                // 2. 尝试已加载数据包中的动态酿造配方
                if let Some(server) = world.server.upgrade() {
                    let dynamic_recipes = server.recipe_manager.get_dynamic_recipes_internal();
                    let slot_key = format!("minecraft:{}", slot.get_item().registry_key);
                    let ing_key = format!("minecraft:{}", ingredient.get_item().registry_key);
                    for dyn_recipe in &dynamic_recipes {
                        if let DynamicRecipe::Brewing(r) = dyn_recipe
                            && r.input_item == slot_key
                            && r.reagent == ing_key
                        {
                            if let Some(req_pot) = &r.input_potion {
                                let current_pot = potion_id
                                    .and_then(|id| Potion::from_id(id as u8))
                                    .map(|p| format!("minecraft:{}", p.name));
                                if current_pot.as_deref() != Some(req_pot.as_str()) {
                                    continue;
                                }
                            }
                            if let Some(target_item) = Item::from_registry_key(
                                r.output_item
                                    .strip_prefix("minecraft:")
                                    .unwrap_or(&r.output_item),
                            ) {
                                let mut new_slot = ItemStack::new(1, target_item);
                                let mut pc = slot
                                    .get_data_component::<papokin_data::data_component_impl::PotionContentsImpl>()
                                    .cloned()
                                    .unwrap_or_else(|| papokin_data::data_component_impl::PotionContentsImpl {
                                        potion_id: None,
                                        custom_color: None,
                                        custom_effects: Vec::new(),
                                        custom_name: None,
                                    });
                                if let Some(out_pot_name) = &r.output_potion
                                    && let Some(out_pot) = Potion::from_name(
                                        out_pot_name
                                            .strip_prefix("minecraft:")
                                            .unwrap_or(out_pot_name),
                                    )
                                {
                                    pc.potion_id = Some(out_pot.id as i32);
                                }
                                new_slot.set_data_component(pc);
                                *slot = new_slot;
                                ingredient_used = true;
                                break;
                            }
                        }
                    }
                }
            }

            // 消耗一份原料
            if ingredient_used {
                items[3].decrement(1);
            }
        }

        if !ingredient_used {
            return;
        }

        // 检查剩余材料是否匹配，否则清空
        if let Ok(items) = self.items.read() {
            let remaining = &items[3];
            if remaining.is_empty()
                || !self
                    .ingredient_item
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .is_some_and(|stored| remaining.get_item().id == stored.id)
            {
                *self
                    .ingredient_item
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner) = None;
            }
        }

        // 检查是否可以立即开始下一波酿造
        if let Ok(items) = self.items.read() {
            let ingredient = items[3].clone();
            drop(items);
            if self.fuel.load(Ordering::Relaxed) > 0 && self.is_brewable(&ingredient, world) {
                self.fuel.fetch_sub(1, Ordering::Relaxed);
                self.brew_time.store(400, Ordering::Relaxed);
                *self
                    .ingredient_item
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner) =
                    Some(ingredient.get_item());
            } else {
                self.brew_time.store(0, Ordering::Relaxed);
            }
        }

        // 在方块中心播放音效
        let pos = Vector3::new(
            self.position.0.x as f64 + 0.5,
            self.position.0.y as f64 + 0.5,
            self.position.0.z as f64 + 0.5,
        );
        world.play_sound(Sound::BlockBrewingStandBrew, SoundCategory::Blocks, &pos);

        // 标记为脏以触发更新
        self.mark_dirty();
    }

    fn try_refill_fuel(&self, world: &Arc<crate::world::World>) -> bool {
        let expected_fuel = if self.fuel.load(Ordering::Relaxed) <= 0
            && let Ok(items) = self.items.try_read()
            && !items[4].is_empty()
            && items[4].get_data_component::<BrewingFuelImpl>().is_some()
        {
            items[4].clone()
        } else {
            return false;
        };

        let fuel_power = if let Some(server) = world.server.upgrade() {
            let mut fuel_event = crate::plugin::api::events::inventory::brewing_stand_fuel::BrewingStandFuelEvent::new(
                self.position,
                20,
            );
            server
                .plugin_manager
                .fire_blocking(&server, &mut fuel_event);

            if fuel_event.cancelled {
                return false;
            }

            fuel_event.fuel_power
        } else {
            20
        };

        if self.fuel.load(Ordering::Relaxed) <= 0
            && let Ok(mut items) = self.items.try_write()
            && !items[4].is_empty()
            && items[4].are_equal(&expected_fuel)
        {
            self.fuel.store(i32::from(fuel_power), Ordering::Relaxed);
            items[4].decrement(1);
            // 燃料槽缩小了，比较器需要更新。
            self.comparator_dirty.store(true, Ordering::Relaxed);
            true
        } else {
            false
        }
    }
}

impl papokin_inventory::Inventory for BrewingStandBlockEntity {
    fn size(&self) -> usize {
        Self::INVENTORY_SIZE
    }

    fn is_empty(&self) -> bool {
        let items = self
            .items
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        for slot in items.iter() {
            if !slot.is_empty() {
                return false;
            }
        }
        true
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
        let taken = if items[slot].item_count <= amount {
            std::mem::replace(&mut items[slot], ItemStack::EMPTY.clone())
        } else {
            let mut taken = items[slot].clone();
            taken.item_count = amount;
            items[slot].item_count -= amount;
            taken
        };
        self.mark_dirty();
        taken
    }

    fn set_stack(&self, slot: usize, stack: ItemStack) {
        let mut items = self
            .items
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        items[slot] = stack;
        self.mark_dirty();
    }

    fn on_open(&self) {}

    fn on_close(&self) {}

    fn mark_dirty(&self) {
        self.dirty.store(true, Ordering::Relaxed);
        self.comparator_dirty.store(true, Ordering::Relaxed);
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn is_valid_slot_for(&self, slot: usize, stack: &ItemStack) -> bool {
        if stack.is_empty() {
            return true;
        }

        match slot {
            // 槽位 0-2 - 药水
            0..=2 => stack
                .get_data_component::<papokin_data::data_component_impl::PotionContentsImpl>()
                .is_some(),
            // 槽位 3 - 材料（必须被标记为可酿造）
            3 => {
                // 燃料物品应放在槽位 4，而不是材料槽。
                if stack.get_data_component::<BrewingFuelImpl>().is_some() {
                    return false;
                }
                // 允许任何非燃料物品（材料校验在酿造时进行）
                true
            }
            // 槽位 4 - 燃料（`minecraft:brewing_fuel` 数据组件，26.3+）
            4 => stack.get_data_component::<BrewingFuelImpl>().is_some(),
            _ => false,
        }
    }
}

impl papokin_inventory::Clearable for BrewingStandBlockEntity {
    fn clear(&self) {
        let mut items = self
            .items
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        items.fill_with(|| ItemStack::EMPTY.clone());
        self.mark_dirty();
    }
}

impl crate::block::entities::BlockEntity for BrewingStandBlockEntity {
    fn resource_location(&self) -> &'static str {
        Self::ID
    }

    fn get_position(&self) -> BlockPos {
        self.position
    }

    fn from_nbt(nbt: &NbtCompound, position: BlockPos) -> Self
    where
        Self: Sized,
    {
        let mut entity = Self::new(position);

        // 如果 NBT 中存在，加载酿造时间 / 燃料
        if let Some(bt) = nbt
            .get_short("BrewTime")
            .map(i32::from)
            .or_else(|| nbt.get_int("BrewTime"))
        {
            entity.brew_time.store(bt, Ordering::Relaxed);
        }
        if let Some(f) = nbt
            .get_byte("Fuel")
            .map(i32::from)
            .or_else(|| nbt.get_int("Fuel"))
        {
            entity.fuel.store(f, Ordering::Relaxed);
        }

        // 从 NBT 加载物品栏物品
        let items = entity
            .items
            .get_mut()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        sync_read_items_from_nbt(nbt, items);

        // 如果槽位 3 中有材料，记住其基础物品用于匹配
        let ingredient_item = (!items[3].is_empty()).then(|| items[3].get_item());

        // 重新计算 last_potion_count，使加载后的视觉效果正确
        let mut current: [bool; 3] = [false; 3];
        for (i, slot) in items.iter().take(3).enumerate() {
            current[i] = !slot.is_empty()
                && (slot
                    .get_data_component::<papokin_data::data_component_impl::PotionContentsImpl>()
                    .is_some()
                    || slot.get_item().id == papokin_data::item::Item::GLASS_BOTTLE.id);
        }

        if let Some(item) = ingredient_item {
            *entity
                .ingredient_item
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(item);
        }

        *entity
            .last_potion_count
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(current);

        entity
    }

    fn write_nbt(&self, nbt: &mut NbtCompound) {
        // 持久化酿造状态
        nbt.put_short("BrewTime", self.brew_time.load(Ordering::Relaxed) as i16);
        nbt.put_byte("Fuel", self.fuel.load(Ordering::Relaxed) as i8);

        // 将物品栏内容保存到 NBT
        self.write_inventory_nbt(nbt, true);
    }

    fn get_inventory(self: Arc<Self>) -> Option<Arc<dyn Inventory>> {
        Some(self)
    }

    fn chunk_data_nbt(&self) -> Option<NbtCompound> {
        let mut nbt = NbtCompound::new();
        nbt.put_short("BrewTime", self.brew_time.load(Ordering::Relaxed) as i16);
        nbt.put_byte("Fuel", self.fuel.load(Ordering::Relaxed) as i8);
        if let Ok(items) = self.items.try_read() {
            sync_write_items_to_nbt(&*items, &mut nbt);
        }
        Some(nbt)
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

    fn clear_dirty(&self) {
        self.dirty.store(false, Ordering::Relaxed);
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    #[allow(clippy::too_many_lines)]
    fn tick(&self, world: &Arc<crate::world::World>) {
        // 如有需要，从燃料物品补充燃料计数器
        let fuel_refilled = self.try_refill_fuel(world);

        // 获取当前材料并检查酿造状态
        let Ok(items) = self.items.try_read() else {
            return;
        };
        let ingredient = items[3].clone();
        drop(items);
        let brewable = self.is_brewable(&ingredient, world);
        let is_brewing = self.brew_time.load(Ordering::Relaxed) > 0;

        // 处理酿造状态机
        if is_brewing {
            // 减少酿造时间
            let new_brew_time = self.brew_time.fetch_sub(1, Ordering::Relaxed) - 1;
            let is_done_brewing = new_brew_time == 0;

            if is_done_brewing && brewable {
                // 酿造完成
                self.do_brew(world, &ingredient);
            } else if !brewable || !self.ingredient_matches(&ingredient) {
                // 取消酿造
                self.brew_time.store(0, Ordering::Relaxed);
                self.mark_timer_dirty();
            } else {
                // 继续酿造
                self.mark_timer_dirty();
            }
        } else if brewable && self.fuel.load(Ordering::Relaxed) > 0 {
            let brew_time = if let Some(server) = world.server.upgrade() {
                let mut start_event =
                    crate::plugin::api::events::block::brewing_start::BrewingStartEvent::new(
                        self.position,
                        world.clone(),
                        400,
                    );
                server
                    .plugin_manager
                    .fire_blocking(&server, &mut start_event);
                if start_event.cancelled {
                    return;
                }
                start_event.brewing_time
            } else {
                400
            };

            // 开始新的酿造周期
            self.fuel.fetch_sub(1, Ordering::Relaxed);
            self.brew_time.store(brew_time, Ordering::Relaxed);
            *self
                .ingredient_item
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(ingredient.get_item());
            self.mark_timer_dirty();
        } else if fuel_refilled {
            // 若燃料已补充则标记为脏以更新燃料指示
            self.mark_timer_dirty();
        }

        // 确保药水槽位内容（及其数据）变化时通知客户端
        // 计算三个瓶子槽位的当前存在位
        let mut current: [bool; 3] = [false; 3];
        if let Ok(items_guard) = self.items.try_read() {
            for (i, slot) in items_guard.iter().take(3).enumerate() {
                // 当药水槽位含有物品和 PotionContents 组件或为玻璃瓶时，视为“存在”
                current[i] = !slot.is_empty() && (slot.get_data_component::<papokin_data::data_component_impl::PotionContentsImpl>().is_some() || slot.get_item().id == Item::GLASS_BOTTLE.id);
            }
        }

        // 如果药水存在状态改变，更新 last_potion_count 并更新方块状态，以便客户端
        let mut needs_update = false;
        {
            let mut last_guard = self
                .last_potion_count
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            if last_guard.as_ref() != Some(&current) {
                *last_guard = Some(current);
                needs_update = true;
            }
        }

        if needs_update {
            // 更新酿造台的方块状态属性，以反映瓶子的有无
            let (block, state) = world.get_block_and_state(&self.position);
            // 使用生成的方块属性辅助函数来产生设置了相应位的新状态 id
            let mut props =
                papokin_data::block_properties::BrewingStandLikeProperties::from_state_id(state.id);
            // 为清晰起见，生成的字段名使用原始标识符
            props.r#has_bottle_0 = current[0];
            props.r#has_bottle_1 = current[1];
            props.r#has_bottle_2 = current[2];

            world.set_block_state(
                &self.position,
                props.to_state_id(block),
                crate::world::BlockFlags::NOTIFY_ALL,
            );

            // 同时标记为脏，以便将背包/容器更新发送给打开的界面。
            // 触发这些位翻转的槽位变更已经标记过比较器了。
            self.mark_timer_dirty();
        }
    }

    fn to_property_delegate(self: Arc<Self>) -> Option<Arc<dyn PropertyDelegate>> {
        Some(self as Arc<dyn PropertyDelegate>)
    }
}

impl PropertyDelegate for BrewingStandBlockEntity {
    fn get_property(&self, index: i32) -> i32 {
        match index {
            0 => self.brew_time.load(Ordering::Relaxed),
            1 => self.fuel.load(Ordering::Relaxed),
            _ => 0,
        }
    }

    fn set_property(&self, _index: i32, _value: i32) {}

    fn get_properties_size(&self) -> i32 {
        2
    }
}
