use papokin_data::item::Item;
use papokin_data::item_stack::ItemStack;
use papokin_data::recipes::RecipeIngredientTypes;
use papokin_inventory::player::player_inventory::PlayerInventory;
use papokin_protocol::codec::recipe::OwnedRecipeIngredient;

#[derive(Clone, Copy)]
pub enum GenericIngredient<'a> {
    Vanilla(&'a RecipeIngredientTypes),
    Dynamic(&'a OwnedRecipeIngredient),
}

impl GenericIngredient<'_> {
    #[must_use]
    pub fn match_item(&self, item: &Item) -> bool {
        match self {
            Self::Vanilla(v) => v.match_item(item),
            Self::Dynamic(d) => d.match_item(item),
        }
    }
}

pub fn take_n_ingredient(
    inventory: &PlayerInventory,
    ingredient: &GenericIngredient<'_>,
    count: u8,
) -> ItemStack {
    let mut taken = 0u8;
    let mut result: Option<ItemStack> = None;

    let mut main_inventory = inventory
        .main_inventory
        .write()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    for stack in main_inventory.iter_mut() {
        if !stack.is_empty() && ingredient.match_item(stack.item) {
            // 标签类原料可匹配多种物品：只从与首个已取堆叠完全相同的
            // 物品上继续取，否则把不同物品的数量合并进同一堆叠会把
            // 后取的物品嬗变成先取的物品（例如白桦木板变成橡木木板）。
            if let Some(r) = &result
                && !r.are_items_and_components_equal(stack)
            {
                continue;
            }
            // 封顶到该物品的最大堆叠数，避免把超过上限的堆叠塞进合成格
            let cap = result
                .as_ref()
                .map_or_else(|| stack.get_max_stack_size(), ItemStack::get_max_stack_size)
                .min(count);
            let to_take = (cap - taken).min(stack.item_count);
            let sub_stack = stack.split(to_take);
            taken += sub_stack.item_count;

            match &mut result {
                None => result = Some(sub_stack),
                Some(r) => r.item_count += sub_stack.item_count,
            }

            if taken >= cap {
                break;
            }
        }
    }
    result.unwrap_or_else(|| ItemStack::EMPTY.clone())
}

pub fn compute_biggest_craftable(
    ingredients: &[GenericIngredient<'_>],
    inventory: &PlayerInventory,
) -> u8 {
    let mut available: Vec<(&'static Item, u32)> = Vec::new();
    let main_inventory = inventory
        .main_inventory
        .read()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    for stack in main_inventory.iter() {
        if !stack.is_empty() {
            if let Some(e) = available.iter_mut().find(|(i, _)| i.id == stack.item.id) {
                e.1 += u32::from(stack.item_count);
            } else {
                available.push((stack.item, u32::from(stack.item_count)));
            }
        }
    }

    'outer: for amount in (1u32..=64).rev() {
        let mut budget = available.clone();
        for ing in ingredients {
            let Some(idx) = budget
                .iter()
                .position(|(item, count)| *count >= amount && ing.match_item(item))
            else {
                continue 'outer;
            };
            budget[idx].1 -= amount;
        }
        return amount as u8;
    }
    0
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use papokin_data::item_stack::ItemStack;
    use papokin_inventory::entity_equipment::EntityEquipment;
    use papokin_inventory::inventory::Inventory;

    use super::*;

    fn inventory() -> PlayerInventory {
        PlayerInventory::new(
            Arc::new(Mutex::new(EntityEquipment::new())),
            Arc::new(rustc_hash::FxHashMap::default()),
        )
    }

    /// 标签/多选原料可匹配多种物品：跨类型合并会把后取的物品
    /// 嬗变成先取的物品（白桦木板变成橡木木板）。取出必须保持
    /// 物品身份，不同类型的匹配堆叠不得混入同一结果堆叠。
    #[test]
    fn take_n_ingredient_preserves_item_identity_across_matches() {
        let inv = inventory();
        inv.set_stack(0, ItemStack::new(10, &Item::OAK_PLANKS));
        inv.set_stack(1, ItemStack::new(10, &Item::BIRCH_PLANKS));
        let ingredient_types =
            RecipeIngredientTypes::OneOf(&["minecraft:oak_planks", "minecraft:birch_planks"]);
        let ingredient = GenericIngredient::Vanilla(&ingredient_types);

        let taken = take_n_ingredient(&inv, &ingredient, 20);

        // 只能取到一种木板；另一种必须原样留在物品栏中
        assert_eq!(taken.item_count, 10);
        assert_eq!(
            inv.get_stack(0).item_count + inv.get_stack(1).item_count,
            10
        );
        let remaining = if taken.item.id == Item::OAK_PLANKS.id {
            inv.get_stack(1)
        } else {
            inv.get_stack(0)
        };
        assert_eq!(remaining.item_count, 10);
        assert_ne!(remaining.item.id, taken.item.id);
    }

    /// 取出数量必须封顶到该物品的最大堆叠数：
    /// 请求 64 个末影珍珠（最大堆叠 16）时只能取出 16 个，
    /// 避免把超堆叠塞进合成格。
    #[test]
    fn take_n_ingredient_caps_at_max_stack_size() {
        let inv = inventory();
        for slot in 0..4 {
            inv.set_stack(slot, ItemStack::new(16, &Item::ENDER_PEARL));
        }
        let ingredient_types = RecipeIngredientTypes::Simple("minecraft:ender_pearl");
        let ingredient = GenericIngredient::Vanilla(&ingredient_types);

        let taken = take_n_ingredient(&inv, &ingredient, 64);

        assert_eq!(taken.item.id, Item::ENDER_PEARL.id);
        assert_eq!(taken.item_count, 16);
        // 剩余 48 个仍在物品栏中，总量守恒
        let remaining: u32 = (0..4)
            .map(|slot| u32::from(inv.get_stack(slot).item_count))
            .sum();
        assert_eq!(remaining, 48);
    }
}
