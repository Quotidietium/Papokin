use papokin_data::item::Item;
use papokin_data::item_id_remap::remap_item_id_for_version;
use papokin_data::item_stack::ItemStack;
use papokin_data::packet::clientbound::play::RECIPE_BOOK_ADD;
use papokin_data::recipes::{
    CookingRecipeType, CraftingRecipeTypes, RECIPES_COOKING, RECIPES_CRAFTING, RecipeCategoryTypes,
    RecipeIngredientTypes, RecipeResultStruct,
};
use papokin_macros::java_packet;
use papokin_util::version::JavaMinecraftVersion;
use std::borrow::Cow;
use std::{collections::HashMap, io::Write};

use crate::codec::item_stack_seralizer::ItemStackTemplateSerializer;
use crate::{ClientPacket, VarInt, WritingError, ser::NetworkWriteExt};

use papokin_data::slot_display_id_remap::remap_slot_display_id_for_version;

// 配方显示类型 ID
const RECIPE_DISPLAY_SHAPELESS: i32 = 0;
const RECIPE_DISPLAY_SHAPED: i32 = 1;
const RECIPE_DISPLAY_FURNACE: i32 = 2;
const RECIPE_DISPLAY_STONECUTTER: i32 = 3;
const RECIPE_DISPLAY_SMITHING: i32 = 4;

// 槽位显示基础类型 ID（26.2）
const SLOT_DISPLAY_EMPTY: u32 = 0;
const SLOT_DISPLAY_ANY_FUEL: u32 = 1;
const SLOT_DISPLAY_ITEM: u32 = 4;
const SLOT_DISPLAY_ITEM_STACK: u32 = 5;
const SLOT_DISPLAY_COMPOSITE: u32 = 10;

const ENTRY_FLAG_NOTIFICATION: u8 = 0x01;
const ENTRY_FLAG_HIGHLIGHT: u8 = 0x02;

// RecipeBookCategory ID
const CATEGORY_CRAFTING_BUILDING: i32 = 0;
const CATEGORY_CRAFTING_REDSTONE: i32 = 1;
const CATEGORY_CRAFTING_EQUIPMENT: i32 = 2;
const CATEGORY_CRAFTING_MISC: i32 = 3;
const CATEGORY_FURNACE_FOOD: i32 = 4;
const CATEGORY_FURNACE_BLOCKS: i32 = 5;
const CATEGORY_FURNACE_MISC: i32 = 6;
const CATEGORY_BLAST_FURNACE_BLOCKS: i32 = 7;
const CATEGORY_BLAST_FURNACE_MISC: i32 = 8;
const CATEGORY_SMOKER_FOOD: i32 = 9;
const CATEGORY_STONECUTTER: i32 = 10;
const CATEGORY_SMITHING: i32 = 11;
const CATEGORY_CAMPFIRE: i32 = 12;

use crate::codec::recipe::DynamicRecipe;

/// 发往客户端的数据包，用于向客户端的配方书添加配方。
/// `replace = true` 表示客户端会替换其当前的配方列表。
#[java_packet(RECIPE_BOOK_ADD)]
pub struct CRecipeBookAdd<'a> {
    pub replace: bool,
    pub dynamic_recipes: &'a [DynamicRecipe],
}

impl<'a> CRecipeBookAdd<'a> {
    #[must_use]
    pub const fn new(replace: bool, dynamic_recipes: &'a [DynamicRecipe]) -> Self {
        Self {
            replace,
            dynamic_recipes,
        }
    }
}

fn item_id_versioned(item: &Item, version: JavaMinecraftVersion) -> i32 {
    remap_item_id_for_version(item.id, version) as i32
}

fn slot_display_item_type(version: JavaMinecraftVersion) -> i32 {
    remap_slot_display_id_for_version(SLOT_DISPLAY_ITEM, version) as i32
}

fn slot_display_composite_type(version: JavaMinecraftVersion) -> i32 {
    remap_slot_display_id_for_version(SLOT_DISPLAY_COMPOSITE, version) as i32
}

fn slot_display_item_stack_type(version: JavaMinecraftVersion) -> i32 {
    remap_slot_display_id_for_version(SLOT_DISPLAY_ITEM_STACK, version) as i32
}

fn write_item_slot_display(
    write: &mut impl Write,
    item: &Item,
    version: JavaMinecraftVersion,
) -> Result<(), WritingError> {
    write.write_var_int(&VarInt(slot_display_item_type(version)))?;
    write.write_var_int(&VarInt(item_id_versioned(item, version)))?;
    Ok(())
}

fn write_item_stack_slot_display(
    write: &mut impl Write,
    item: &Item,
    count: u8,
    version: JavaMinecraftVersion,
) -> Result<(), WritingError> {
    let static_item = Item::from_id(item.id)
        .ok_or_else(|| WritingError::Message(format!("item id {} must exist", item.id)))?;
    let stack = ItemStack::new(count, static_item);
    // 无 `id` 的结果降级为空气，原版会将其拒绝为物品堆
    if stack.is_empty() {
        return write_empty_slot_display(write, version);
    }
    write.write_var_int(&VarInt(slot_display_item_stack_type(version)))?;
    ItemStackTemplateSerializer::from(stack).write_with_version(write, &version)
}

fn write_empty_slot_display(
    write: &mut impl Write,
    version: JavaMinecraftVersion,
) -> Result<(), WritingError> {
    write.write_var_int(&VarInt(
        remap_slot_display_id_for_version(SLOT_DISPLAY_EMPTY, version) as i32,
    ))?;
    Ok(())
}

fn write_any_fuel_slot_display(
    write: &mut impl Write,
    version: JavaMinecraftVersion,
) -> Result<(), WritingError> {
    write.write_var_int(&VarInt(
        remap_slot_display_id_for_version(SLOT_DISPLAY_ANY_FUEL, version) as i32,
    ))?;
    Ok(())
}

fn resolve_item_tag(tag: &str, version: JavaMinecraftVersion) -> Option<Vec<&'static Item>> {
    let tag = tag.strip_prefix('#').unwrap_or(tag);
    let full_tag = if tag.contains(':') {
        Cow::Borrowed(tag)
    } else {
        Cow::Owned(format!("minecraft:{tag}"))
    };

    let item_names =
        papokin_data::tag::get_registry_key_tags(version, papokin_data::tag::RegistryKey::Item)
            .and_then(|map| map.get(full_tag.as_ref()))
            .map(|t| t.0)
            .or_else(|| {
                papokin_data::tag::get_tag_values(
                    papokin_data::tag::RegistryKey::Item,
                    full_tag.as_ref(),
                )
            })?;

    let mut items = Vec::new();
    for name in item_names {
        let key = name.strip_prefix("minecraft:").unwrap_or(name);
        if let Some(item) = Item::from_registry_key(key) {
            items.push(item);
        }
    }
    if items.is_empty() { None } else { Some(items) }
}

fn write_ingredient_slot_display(
    write: &mut impl Write,
    ingredient: &RecipeIngredientTypes,
    version: JavaMinecraftVersion,
) -> Result<(), WritingError> {
    match ingredient {
        RecipeIngredientTypes::Simple(id) => {
            let key = id.strip_prefix("minecraft:").unwrap_or(id);
            if let Some(item) = Item::from_registry_key(key) {
                write_item_slot_display(write, item, version)?;
            } else {
                write_empty_slot_display(write, version)?;
            }
        }
        RecipeIngredientTypes::Tagged(tag) => {
            if let Some(items) = resolve_item_tag(tag, version) {
                if items.len() == 1 {
                    write_item_slot_display(write, items[0], version)?;
                } else {
                    write.write_var_int(&VarInt(slot_display_composite_type(version)))?;
                    write.write_var_int(&VarInt(items.len() as i32))?;
                    for item in &items {
                        write_item_slot_display(write, item, version)?;
                    }
                }
            } else {
                write_empty_slot_display(write, version)?;
            }
        }
        RecipeIngredientTypes::OneOf(ids) => {
            let mut items: Vec<&Item> = Vec::new();
            for id in *ids {
                let key = id.strip_prefix("minecraft:").unwrap_or(id);
                if let Some(item) = Item::from_registry_key(key) {
                    items.push(item);
                }
            }
            if items.is_empty() {
                write_empty_slot_display(write, version)?;
            } else if items.len() == 1 {
                write_item_slot_display(write, items[0], version)?;
            } else {
                write.write_var_int(&VarInt(slot_display_composite_type(version)))?;
                write.write_var_int(&VarInt(items.len() as i32))?;
                for item in &items {
                    write_item_slot_display(write, item, version)?;
                }
            }
        }
    }
    Ok(())
}

/// 将单个 Ingredient 以 `HolderSet`<Item> 的形式写入 craftingRequirements。
///
/// `ByteBufCodecs.holderSet(Registries.ITEM)` 的原版线上格式：
///   VarInt(0)     -> 命名标签引用（后跟 `ResourceLocation`）
///   VarInt(n + 1) -> n 个物品 ID 的直接列表
fn write_ingredient_holderset(
    write: &mut impl Write,
    ingredient: &RecipeIngredientTypes,
    version: JavaMinecraftVersion,
) -> Result<(), WritingError> {
    match ingredient {
        RecipeIngredientTypes::Simple(id) => {
            let key = id.strip_prefix("minecraft:").unwrap_or(id);
            // 1 个物品 -> VarInt(1 + 1) = VarInt(2)
            write.write_var_int(&VarInt(2))?;
            if let Some(item) = Item::from_registry_key(key) {
                write.write_var_int(&VarInt(item_id_versioned(item, version)))?;
            } else {
                // 使用非空回退物品以防客户端 UnsupportedOperationException
                write.write_var_int(&VarInt(0))?;
            }
        }
        RecipeIngredientTypes::Tagged(tag) => {
            if let Some(items) = resolve_item_tag(tag, version) {
                write.write_var_int(&VarInt(items.len() as i32 + 1))?;
                for item in &items {
                    write.write_var_int(&VarInt(item_id_versioned(item, version)))?;
                }
            } else {
                let tag = tag.strip_prefix('#').unwrap_or(tag);
                let full_tag = if tag.contains(':') {
                    tag.to_string()
                } else {
                    format!("minecraft:{tag}")
                };
                write.write_var_int(&VarInt(0))?;
                write.write_string(&full_tag)?;
            }
        }
        RecipeIngredientTypes::OneOf(ids) => {
            let items: Vec<i32> = ids
                .iter()
                .filter_map(|id| {
                    let key = id.strip_prefix("minecraft:").unwrap_or(id);
                    Item::from_registry_key(key).map(|item| item_id_versioned(item, version))
                })
                .collect();
            if items.is_empty() {
                write.write_var_int(&VarInt(2))?;
                write.write_var_int(&VarInt(0))?;
            } else {
                write.write_var_int(&VarInt(items.len() as i32 + 1))?;
                for id in &items {
                    write.write_var_int(&VarInt(*id))?;
                }
            }
        }
    }
    Ok(())
}

/// 写入 `craftingRequirements: Option<List<Ingredient>>` 字段（存在时）。
fn write_crafting_requirements(
    write: &mut impl Write,
    slots: &[&RecipeIngredientTypes],
    version: JavaMinecraftVersion,
) -> Result<(), WritingError> {
    write.write_bool(true)?; // 存在
    write.write_var_int(&VarInt(slots.len() as i32))?;
    for slot in slots {
        write_ingredient_holderset(write, slot, version)?;
    }
    Ok(())
}

fn write_result_slot_display(
    write: &mut impl Write,
    result: &RecipeResultStruct,
    version: JavaMinecraftVersion,
) -> Result<(), WritingError> {
    let key = result.id.strip_prefix("minecraft:").unwrap_or(result.id);
    if let Some(item) = Item::from_registry_key(key) {
        write_item_stack_slot_display(write, item, result.count, version)?;
    } else {
        write_empty_slot_display(write, version)?;
    }
    Ok(())
}

fn write_optional_var_int(write: &mut impl Write, value: Option<i32>) -> Result<(), WritingError> {
    let encoded = value.map_or(Ok(0), |v| {
        v.checked_add(1)
            .ok_or_else(|| WritingError::Message(format!("group id {v} overflow")))
    })?;
    write.write_var_int(&VarInt(encoded))?;
    Ok(())
}
const fn entry_flags(replace: bool, notification: bool, highlight: bool) -> u8 {
    if replace {
        return 0;
    }

    (if notification {
        ENTRY_FLAG_NOTIFICATION
    } else {
        0
    }) | (if highlight { ENTRY_FLAG_HIGHLIGHT } else { 0 })
}

const fn crafting_category(cat: &RecipeCategoryTypes) -> i32 {
    match cat {
        RecipeCategoryTypes::Equipment => CATEGORY_CRAFTING_EQUIPMENT,
        RecipeCategoryTypes::Building | RecipeCategoryTypes::Blocks => CATEGORY_CRAFTING_BUILDING,
        RecipeCategoryTypes::Restone => CATEGORY_CRAFTING_REDSTONE,
        RecipeCategoryTypes::Food | RecipeCategoryTypes::Misc => CATEGORY_CRAFTING_MISC,
    }
}

/// 写入单个 `RecipeDisplayEntry` + 标志字节。
///若已写入则返回 `Ok(true)`；若被跳过（特殊配方）则返回 `Ok(false)`。
#[allow(clippy::too_many_lines, clippy::too_many_arguments)]
fn write_entry(
    write: &mut impl Write,
    display_id: i32,
    version: JavaMinecraftVersion,
    group_id: Option<i32>,
    flags: u8,
    crafting_table: &Item,
    furnace: &Item,
    blast_furnace: &Item,
    smoker: &Item,
    campfire: &Item,
    crafting_recipe: Option<&CraftingRecipeTypes>,
    cooking_recipe: Option<(&CookingRecipeType, i32)>,
) -> Result<bool, WritingError> {
    if let Some(recipe) = crafting_recipe {
        match recipe {
            CraftingRecipeTypes::CraftingShaped {
                category,
                pattern,
                key,
                result,
                ..
            } => {
                // 根据 pattern 计算宽和高
                let height = pattern.len() as i32;
                let width = pattern.first().map_or(0, |r| r.len()) as i32;

                // RecipeDisplayId
                write.write_var_int(&VarInt(display_id))?;
                // RecipeDisplay 类型 = 有序（1）
                write.write_var_int(&VarInt(RECIPE_DISPLAY_SHAPED))?;
                // 宽度、高度
                write.write_var_int(&VarInt(width))?;
                write.write_var_int(&VarInt(height))?;
                // 配料：逐行排列的扁平列表
                write.write_var_int(&VarInt(width * height))?;
                for row in *pattern {
                    for ch in row.chars() {
                        if ch == ' ' {
                            write_empty_slot_display(write, version)?;
                        } else if let Some((_, ingredient)) = key.iter().find(|(k, _)| *k == ch) {
                            write_ingredient_slot_display(write, ingredient, version)?;
                        } else {
                            write_empty_slot_display(write, version)?;
                        }
                    }
                }
                // 结果
                write_result_slot_display(write, result, version)?;
                // 合成工作站（craftingStation）
                write_item_slot_display(write, crafting_table, version)?;
                // 组：OptionalVarInt
                write_optional_var_int(write, group_id)?;
                // 类别（category）
                write.write_var_int(&VarInt(crafting_category(category)))?;
                // craftingRequirements：每个非空网格槽位一个 HolderSet
                // （原料不可为空，因此空槽位必须排除）
                {
                    let mut slots: Vec<&RecipeIngredientTypes> = Vec::new();
                    for row in *pattern {
                        for ch in row.chars() {
                            if ch != ' '
                                && let Some((_, ing)) = key.iter().find(|(k, _)| *k == ch)
                            {
                                slots.push(ing);
                            }
                        }
                    }
                    write_crafting_requirements(write, &slots, version)?;
                };
                write.write_u8(flags)?;
            }
            CraftingRecipeTypes::CraftingShapeless {
                category,
                ingredients,
                result,
                ..
            } => {
                // RecipeDisplayId
                write.write_var_int(&VarInt(display_id))?;
                // RecipeDisplay 类型 = 无序（0）
                write.write_var_int(&VarInt(RECIPE_DISPLAY_SHAPELESS))?;
                // 配料列表
                write.write_var_int(&VarInt(ingredients.len() as i32))?;
                for ing in *ingredients {
                    write_ingredient_slot_display(write, ing, version)?;
                }
                // 结果
                write_result_slot_display(write, result, version)?;
                // 合成工作站（craftingStation）
                write_item_slot_display(write, crafting_table, version)?;
                // 组：OptionalVarInt
                write_optional_var_int(write, group_id)?;
                // 类别（category）
                write.write_var_int(&VarInt(crafting_category(category)))?;
                // craftingRequirements：每种原料一个 HolderSet
                {
                    let slots: Vec<&RecipeIngredientTypes> = ingredients.iter().collect();
                    write_crafting_requirements(write, &slots, version)?;
                };
                write.write_u8(flags)?;
            }
            CraftingRecipeTypes::CraftingTransmute {
                category,
                input,
                material,
                result,
                ..
            } => {
                // 转化（Transmute）显示为含 2 种原料的无序配方
                write.write_var_int(&VarInt(display_id))?;
                write.write_var_int(&VarInt(RECIPE_DISPLAY_SHAPELESS))?;
                // 2 份材料
                write.write_var_int(&VarInt(2))?;
                write_ingredient_slot_display(write, input, version)?;
                write_ingredient_slot_display(write, material, version)?;
                write_result_slot_display(write, result, version)?;
                write_item_slot_display(write, crafting_table, version)?;
                write_optional_var_int(write, group_id)?;
                write.write_var_int(&VarInt(crafting_category(category)))?;
                // craftingRequirements：输入 + 材料
                write_crafting_requirements(write, &[input, material], version)?;
                write.write_u8(flags)?;
            }
            // 跳过 special/decorated_pot 配方，因为它们没有可用的显示
            CraftingRecipeTypes::CraftingDecoratedPot { .. }
            | CraftingRecipeTypes::CraftingSpecial => {
                return Ok(false);
            }
        }
        return Ok(true);
    }

    if let Some((recipe, book_category)) = cooking_recipe {
        let (cooking, station) = match recipe {
            CookingRecipeType::Smelting(r) => (r, furnace),
            CookingRecipeType::Blasting(r) => (r, blast_furnace),
            CookingRecipeType::Smoking(r) => (r, smoker),
            CookingRecipeType::CampfireCooking(r) => (r, campfire),
        };

        write.write_var_int(&VarInt(display_id))?;
        // RecipeDisplay 类型 = 熔炉（2）
        write.write_var_int(&VarInt(RECIPE_DISPLAY_FURNACE))?;
        // 配料
        write_ingredient_slot_display(write, &cooking.ingredient, version)?;
        // 燃料：AnyFuel
        write_any_fuel_slot_display(write, version)?;
        // 结果
        write_result_slot_display(write, &cooking.result, version)?;
        // 合成工作站（craftingStation）
        write_item_slot_display(write, station, version)?;
        // 时长（duration）
        write.write_var_int(&VarInt(cooking.cookingtime))?;
        // 经验
        write.write_f32_be(cooking.experience)?;
        // 组：OptionalVarInt
        write_optional_var_int(write, group_id)?;
        // 类别（category）
        write.write_var_int(&VarInt(book_category))?;
        // craftingRequirements：单一原料
        write_crafting_requirements(write, &[&cooking.ingredient], version)?;
        write.write_u8(flags)?;
        return Ok(true);
    }

    Ok(false)
}

#[allow(clippy::too_many_lines)]
impl ClientPacket for CRecipeBookAdd<'_> {
    fn write_packet_data(
        &self,
        write: impl Write,
        version: &JavaMinecraftVersion,
    ) -> Result<(), WritingError> {
        let mut write = write;

        // 站点物品（这些 ID 在我们支持的所有版本中稳定）
        let crafting_table = Item::from_registry_key("crafting_table")
            .ok_or_else(|| WritingError::Message("crafting_table item must exist".into()))?;
        let furnace = Item::from_registry_key("furnace")
            .ok_or_else(|| WritingError::Message("furnace item must exist".into()))?;
        let blast_furnace = Item::from_registry_key("blast_furnace")
            .ok_or_else(|| WritingError::Message("blast_furnace item must exist".into()))?;
        let smoker = Item::from_registry_key("smoker")
            .ok_or_else(|| WritingError::Message("smoker item must exist".into()))?;
        let campfire = Item::from_registry_key("campfire")
            .ok_or_else(|| WritingError::Message("campfire item must exist".into()))?;

        // 第一遍——统计并跳过 CraftingSpecial 与 CraftingDecoratedPot 条目
        let crafting_count: usize = RECIPES_CRAFTING
            .iter()
            .filter(|r| {
                !matches!(
                    r,
                    CraftingRecipeTypes::CraftingSpecial
                        | CraftingRecipeTypes::CraftingDecoratedPot { .. }
                )
            })
            .count();
        let dynamic_count = self.dynamic_recipes.len();
        let total = crafting_count + RECIPES_COOKING.len() + dynamic_count;

        // 条目计数（VarInt）
        write.write_var_int(&VarInt(total as i32))?;

        let mut display_id: i32 = 0;
        let mut group_ids: HashMap<Cow<'_, str>, i32> = HashMap::new();
        let mut next_group_id: i32 = 0;
        let highlight = !self.replace;

        // 写入合成配方
        for recipe in RECIPES_CRAFTING {
            let (group, notification) = match recipe {
                CraftingRecipeTypes::CraftingShaped {
                    group,
                    show_notification,
                    ..
                } => (group.map(Cow::Borrowed), *show_notification),
                CraftingRecipeTypes::CraftingShapeless { group, .. }
                | CraftingRecipeTypes::CraftingTransmute { group, .. } => {
                    (group.map(Cow::Borrowed), true)
                }
                CraftingRecipeTypes::CraftingDecoratedPot { .. }
                | CraftingRecipeTypes::CraftingSpecial => (None, true),
            };
            let group_id = resolve_group_id_owned(&mut group_ids, &mut next_group_id, group);
            let flags = entry_flags(self.replace, notification, highlight);
            let written = write_entry(
                &mut write,
                display_id,
                *version,
                group_id,
                flags,
                crafting_table,
                furnace,
                blast_furnace,
                smoker,
                campfire,
                Some(recipe),
                None,
            )?;
            if written {
                display_id += 1;
            }
        }

        // 写入烧炼配方
        for recipe in RECIPES_COOKING {
            let (book_category, group) = match recipe {
                CookingRecipeType::Smelting(r) => (
                    match r.category {
                        RecipeCategoryTypes::Food => CATEGORY_FURNACE_FOOD,
                        RecipeCategoryTypes::Blocks => CATEGORY_FURNACE_BLOCKS,
                        _ => CATEGORY_FURNACE_MISC,
                    },
                    r.group,
                ),
                CookingRecipeType::Blasting(r) => (
                    match r.category {
                        RecipeCategoryTypes::Blocks => CATEGORY_BLAST_FURNACE_BLOCKS,
                        _ => CATEGORY_BLAST_FURNACE_MISC,
                    },
                    r.group,
                ),
                CookingRecipeType::Smoking(r) => (CATEGORY_SMOKER_FOOD, r.group),
                CookingRecipeType::CampfireCooking(r) => (CATEGORY_CAMPFIRE, r.group),
            };
            let group_id = resolve_group_id_owned(
                &mut group_ids,
                &mut next_group_id,
                group.map(Cow::Borrowed),
            );
            let flags = entry_flags(self.replace, true, highlight);
            write_entry(
                &mut write,
                display_id,
                *version,
                group_id,
                flags,
                crafting_table,
                furnace,
                blast_furnace,
                smoker,
                campfire,
                None,
                Some((recipe, book_category)),
            )?;
            display_id += 1;
        }

        // 写入动态配方
        for recipe in self.dynamic_recipes {
            match recipe {
                DynamicRecipe::Crafting(crafting) => {
                    let (group, flags) = match crafting {
                        crate::codec::recipe::OwnedCraftingRecipe::Shaped {
                            group,
                            show_notification,
                            ..
                        } => (
                            group.as_deref().map(Cow::Borrowed),
                            entry_flags(self.replace, *show_notification, highlight),
                        ),
                        crate::codec::recipe::OwnedCraftingRecipe::Shapeless { group, .. } => (
                            group.as_deref().map(Cow::Borrowed),
                            entry_flags(self.replace, true, highlight),
                        ),
                    };
                    let group_id =
                        resolve_group_id_owned(&mut group_ids, &mut next_group_id, group);
                    write_dynamic_crafting_entry(
                        &mut write,
                        display_id,
                        *version,
                        group_id,
                        flags,
                        crafting_table,
                        crafting,
                    )?;
                }
                DynamicRecipe::Cooking(cooking) => {
                    let (book_category, group, owned_cooking) = match cooking {
                        crate::codec::recipe::OwnedCookingRecipeType::Smelting(r) => (
                            match r.category {
                                RecipeCategoryTypes::Food => CATEGORY_FURNACE_FOOD,
                                RecipeCategoryTypes::Blocks => CATEGORY_FURNACE_BLOCKS,
                                _ => CATEGORY_FURNACE_MISC,
                            },
                            r.group.as_deref().map(Cow::Borrowed),
                            r,
                        ),
                        crate::codec::recipe::OwnedCookingRecipeType::Blasting(r) => (
                            match r.category {
                                RecipeCategoryTypes::Blocks => CATEGORY_BLAST_FURNACE_BLOCKS,
                                _ => CATEGORY_BLAST_FURNACE_MISC,
                            },
                            r.group.as_deref().map(Cow::Borrowed),
                            r,
                        ),
                        crate::codec::recipe::OwnedCookingRecipeType::Smoking(r) => (
                            CATEGORY_SMOKER_FOOD,
                            r.group.as_deref().map(Cow::Borrowed),
                            r,
                        ),
                        crate::codec::recipe::OwnedCookingRecipeType::CampfireCooking(r) => {
                            (CATEGORY_CAMPFIRE, r.group.as_deref().map(Cow::Borrowed), r)
                        }
                    };
                    let station = match cooking {
                        crate::codec::recipe::OwnedCookingRecipeType::Smelting(_) => furnace,
                        crate::codec::recipe::OwnedCookingRecipeType::Blasting(_) => blast_furnace,
                        crate::codec::recipe::OwnedCookingRecipeType::Smoking(_) => smoker,
                        crate::codec::recipe::OwnedCookingRecipeType::CampfireCooking(_) => {
                            campfire
                        }
                    };

                    let group_id =
                        resolve_group_id_owned(&mut group_ids, &mut next_group_id, group);
                    let flags = entry_flags(self.replace, true, highlight);
                    write_dynamic_cooking_entry(
                        &mut write,
                        display_id,
                        *version,
                        group_id,
                        flags,
                        station,
                        owned_cooking,
                        book_category,
                    )?;
                }
                DynamicRecipe::Brewing(_) => {
                    // 酿造配方不会显示在配方书中
                    continue;
                }
                DynamicRecipe::Stonecutting(stonecutting) => {
                    let flags = entry_flags(self.replace, true, highlight);
                    write_dynamic_stonecutting_entry(
                        &mut write,
                        display_id,
                        *version,
                        flags,
                        stonecutting,
                    )?;
                }
                DynamicRecipe::Smithing(smithing) => {
                    let flags = entry_flags(self.replace, true, highlight);
                    write_dynamic_smithing_entry(
                        &mut write, display_id, *version, flags, smithing,
                    )?;
                }
            }
            display_id += 1;
        }

        // 替换标志
        write.write_bool(self.replace)?;
        Ok(())
    }
}

fn resolve_group_id_owned<'a>(
    group_ids: &mut HashMap<Cow<'a, str>, i32>,
    next_group_id: &mut i32,
    group: Option<Cow<'a, str>>,
) -> Option<i32> {
    let key = group?;
    Some(*group_ids.entry(key).or_insert_with(|| {
        let id = *next_group_id;
        *next_group_id += 1;
        id
    }))
}

fn write_dynamic_ingredient_slot_display(
    write: &mut impl Write,
    ingredient: &crate::codec::recipe::OwnedRecipeIngredient,
    version: JavaMinecraftVersion,
) -> Result<(), WritingError> {
    match ingredient {
        crate::codec::recipe::OwnedRecipeIngredient::Simple(id) => {
            let key = id.strip_prefix("minecraft:").unwrap_or(id);
            if let Some(item) = Item::from_registry_key(key) {
                write_item_slot_display(write, item, version)?;
            } else {
                write_empty_slot_display(write, version)?;
            }
        }
        crate::codec::recipe::OwnedRecipeIngredient::Tagged(tag) => {
            if let Some(items) = resolve_item_tag(tag, version) {
                if items.len() == 1 {
                    write_item_slot_display(write, items[0], version)?;
                } else {
                    write.write_var_int(&VarInt(slot_display_composite_type(version)))?;
                    write.write_var_int(&VarInt(items.len() as i32))?;
                    for item in &items {
                        write_item_slot_display(write, item, version)?;
                    }
                }
            } else {
                write_empty_slot_display(write, version)?;
            }
        }
        crate::codec::recipe::OwnedRecipeIngredient::OneOf(ids) => {
            let items: Vec<&Item> = ids
                .iter()
                .filter_map(|id| {
                    let key = id.strip_prefix("minecraft:").unwrap_or(id);
                    Item::from_registry_key(key)
                })
                .collect();

            if items.is_empty() {
                write_empty_slot_display(write, version)?;
            } else if items.len() == 1 {
                write_item_slot_display(write, items[0], version)?;
            } else {
                write.write_var_int(&VarInt(slot_display_composite_type(version)))?;
                write.write_var_int(&VarInt(items.len() as i32))?;
                for item in &items {
                    write_item_slot_display(write, item, version)?;
                }
            }
        }
    }
    Ok(())
}

fn write_dynamic_ingredient_holderset(
    write: &mut impl Write,
    ingredient: &crate::codec::recipe::OwnedRecipeIngredient,
    version: JavaMinecraftVersion,
) -> Result<(), WritingError> {
    match ingredient {
        crate::codec::recipe::OwnedRecipeIngredient::Simple(id) => {
            let key = id.strip_prefix("minecraft:").unwrap_or(id);
            write.write_var_int(&VarInt(2))?;
            if let Some(item) = Item::from_registry_key(key) {
                write.write_var_int(&VarInt(item_id_versioned(item, version)))?;
            } else {
                write.write_var_int(&VarInt(0))?;
            }
        }
        crate::codec::recipe::OwnedRecipeIngredient::Tagged(tag) => {
            if let Some(items) = resolve_item_tag(tag, version) {
                write.write_var_int(&VarInt(items.len() as i32 + 1))?;
                for item in &items {
                    write.write_var_int(&VarInt(item_id_versioned(item, version)))?;
                }
            } else {
                let tag = tag.strip_prefix('#').unwrap_or(tag);
                let full_tag = if tag.contains(':') {
                    tag.to_string()
                } else {
                    format!("minecraft:{tag}")
                };
                write.write_var_int(&VarInt(0))?;
                write.write_string(&full_tag)?;
            }
        }
        crate::codec::recipe::OwnedRecipeIngredient::OneOf(ids) => {
            let items: Vec<i32> = ids
                .iter()
                .filter_map(|id| {
                    let key = id.strip_prefix("minecraft:").unwrap_or(id);
                    Item::from_registry_key(key).map(|item| item_id_versioned(item, version))
                })
                .collect();
            if items.is_empty() {
                write.write_var_int(&VarInt(2))?;
                write.write_var_int(&VarInt(0))?;
            } else {
                write.write_var_int(&VarInt(items.len() as i32 + 1))?;
                for id in &items {
                    write.write_var_int(&VarInt(*id))?;
                }
            }
        }
    }
    Ok(())
}

fn write_dynamic_result_slot_display(
    write: &mut impl Write,
    result: &crate::codec::recipe::OwnedRecipeResult,
    version: JavaMinecraftVersion,
) -> Result<(), WritingError> {
    let key = result
        .item_id
        .strip_prefix("minecraft:")
        .unwrap_or(&result.item_id);
    if let Some(item) = Item::from_registry_key(key) {
        write_item_stack_slot_display(write, item, result.count, version)?;
    } else {
        write_empty_slot_display(write, version)?;
    }
    Ok(())
}

fn write_dynamic_crafting_entry(
    write: &mut impl Write,
    display_id: i32,
    version: JavaMinecraftVersion,
    group_id: Option<i32>,
    flags: u8,
    crafting_table: &Item,
    recipe: &crate::codec::recipe::OwnedCraftingRecipe,
) -> Result<(), WritingError> {
    match recipe {
        crate::codec::recipe::OwnedCraftingRecipe::Shaped {
            category,
            pattern,
            key,
            result,
            ..
        } => {
            let height = pattern.len() as i32;
            let width = pattern.first().map_or(0, String::len) as i32;

            write.write_var_int(&VarInt(display_id))?;
            write.write_var_int(&VarInt(RECIPE_DISPLAY_SHAPED))?;
            write.write_var_int(&VarInt(width))?;
            write.write_var_int(&VarInt(height))?;
            write.write_var_int(&VarInt(width * height))?;
            for row in pattern {
                for ch in row.chars() {
                    if ch == ' ' {
                        write_empty_slot_display(write, version)?;
                    } else if let Some((_, ingredient)) = key.iter().find(|(k, _)| *k == ch) {
                        write_dynamic_ingredient_slot_display(write, ingredient, version)?;
                    } else {
                        write_empty_slot_display(write, version)?;
                    }
                }
            }
            write_dynamic_result_slot_display(write, result, version)?;
            write_item_slot_display(write, crafting_table, version)?;
            write_optional_var_int(write, group_id)?;
            write.write_var_int(&VarInt(crafting_category(category)))?;

            let mut slots: Vec<&crate::codec::recipe::OwnedRecipeIngredient> = Vec::new();
            for row in pattern {
                for ch in row.chars() {
                    if ch != ' '
                        && let Some((_, ing)) = key.iter().find(|(k, _)| *k == ch)
                    {
                        slots.push(ing);
                    }
                }
            }
            write.write_bool(true)?; // 存在
            write.write_var_int(&VarInt(slots.len() as i32))?;
            for ing in slots {
                write_dynamic_ingredient_holderset(write, ing, version)?;
            }
            write.write_u8(flags)?;
        }
        crate::codec::recipe::OwnedCraftingRecipe::Shapeless {
            category,
            ingredients,
            result,
            ..
        } => {
            write.write_var_int(&VarInt(display_id))?;
            write.write_var_int(&VarInt(RECIPE_DISPLAY_SHAPELESS))?;
            write.write_var_int(&VarInt(ingredients.len() as i32))?;
            for ing in ingredients {
                write_dynamic_ingredient_slot_display(write, ing, version)?;
            }
            write_dynamic_result_slot_display(write, result, version)?;
            write_item_slot_display(write, crafting_table, version)?;
            write_optional_var_int(write, group_id)?;
            write.write_var_int(&VarInt(crafting_category(category)))?;

            write.write_bool(true)?;
            write.write_var_int(&VarInt(ingredients.len() as i32))?;
            for ing in ingredients {
                write_dynamic_ingredient_holderset(write, ing, version)?;
            }
            write.write_u8(flags)?;
        }
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn write_dynamic_cooking_entry(
    write: &mut impl Write,
    display_id: i32,
    version: JavaMinecraftVersion,
    group_id: Option<i32>,
    flags: u8,
    station: &Item,
    cooking: &crate::codec::recipe::OwnedCookingRecipe,
    book_category: i32,
) -> Result<(), WritingError> {
    write.write_var_int(&VarInt(display_id))?;
    write.write_var_int(&VarInt(RECIPE_DISPLAY_FURNACE))?;
    write_dynamic_ingredient_slot_display(write, &cooking.ingredient, version)?;
    write_any_fuel_slot_display(write, version)?;
    write_dynamic_result_slot_display(write, &cooking.result, version)?;
    write_item_slot_display(write, station, version)?;
    write.write_var_int(&VarInt(cooking.cooking_time))?;
    write.write_f32_be(cooking.experience)?;
    write_optional_var_int(write, group_id)?;
    write.write_var_int(&VarInt(book_category))?;
    write.write_bool(true)?;
    write.write_var_int(&VarInt(1))?;
    write_dynamic_ingredient_holderset(write, &cooking.ingredient, version)?;
    write.write_u8(flags)?;
    Ok(())
}

fn write_dynamic_stonecutting_entry(
    write: &mut impl Write,
    display_id: i32,
    version: JavaMinecraftVersion,
    flags: u8,
    recipe: &crate::codec::recipe::OwnedStonecuttingRecipe,
) -> Result<(), WritingError> {
    let stonecutter = Item::from_registry_key("minecraft:stonecutter")
        .ok_or_else(|| WritingError::Message("stonecutter item must exist".into()))?;

    write.write_var_int(&VarInt(display_id))?;
    write.write_var_int(&VarInt(RECIPE_DISPLAY_STONECUTTER))?;
    // 配料
    write_dynamic_ingredient_slot_display(write, &recipe.ingredient, version)?;
    // 结果
    write_dynamic_result_slot_display(write, &recipe.result, version)?;
    // 合成工作站（craftingStation）
    write_item_slot_display(write, stonecutter, version)?;
    // 组：none
    write_optional_var_int(write, None)?;
    // 类别（category）
    write.write_var_int(&VarInt(CATEGORY_STONECUTTER))?;
    // craftingRequirements：单一原料
    write.write_bool(true)?;
    write.write_var_int(&VarInt(1))?;
    write_dynamic_ingredient_holderset(write, &recipe.ingredient, version)?;
    write.write_u8(flags)?;
    Ok(())
}

fn write_dynamic_smithing_entry(
    write: &mut impl Write,
    display_id: i32,
    version: JavaMinecraftVersion,
    flags: u8,
    recipe: &crate::codec::recipe::OwnedSmithingRecipe,
) -> Result<(), WritingError> {
    let smithing_table = Item::from_registry_key("minecraft:smithing_table")
        .ok_or_else(|| WritingError::Message("smithing_table item must exist".into()))?;

    let (template, base, addition, result) = match recipe {
        crate::codec::recipe::OwnedSmithingRecipe::Transform {
            template,
            base,
            addition,
            result,
            ..
        } => (template, base, addition, Some(result)),
        crate::codec::recipe::OwnedSmithingRecipe::Trim {
            template,
            base,
            addition,
            ..
        } => (template, base, addition, None),
    };

    write.write_var_int(&VarInt(display_id))?;
    write.write_var_int(&VarInt(RECIPE_DISPLAY_SMITHING))?;
    // 模板、基底、附加物
    write_dynamic_ingredient_slot_display(write, template, version)?;
    write_dynamic_ingredient_slot_display(write, base, version)?;
    write_dynamic_ingredient_slot_display(write, addition, version)?;
    // 结果：纹饰保留基础物品，因此由基础物品充当展示结果
    if let Some(result) = result {
        write_dynamic_result_slot_display(write, result, version)?;
    } else {
        write_dynamic_ingredient_slot_display(write, base, version)?;
    }
    // 合成工作站（craftingStation）
    write_item_slot_display(write, smithing_table, version)?;
    // 组：none
    write_optional_var_int(write, None)?;
    // 类别（category）
    write.write_var_int(&VarInt(CATEGORY_SMITHING))?;
    // craftingRequirements：模板 + 基底 + 添加物
    write.write_bool(true)?;
    write.write_var_int(&VarInt(3))?;
    write_dynamic_ingredient_holderset(write, template, version)?;
    write_dynamic_ingredient_holderset(write, base, version)?;
    write_dynamic_ingredient_holderset(write, addition, version)?;
    write.write_u8(flags)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn air_result_writes_the_empty_slot_display() {
        let result = RecipeResultStruct {
            id: "minecraft:air",
            count: 1,
        };
        for version in [JavaMinecraftVersion::V_1_21_2, JavaMinecraftVersion::V_26_3] {
            let mut bytes = Vec::new();
            write_result_slot_display(&mut bytes, &result, version).unwrap();
            assert_eq!(
                bytes,
                [remap_slot_display_id_for_version(SLOT_DISPLAY_EMPTY, version) as u8]
            );
        }
    }

    #[test]
    fn vanilla_recipes_serialize_for_every_recipe_book_version() {
        let packet = CRecipeBookAdd::new(true, &[]);
        for version in [JavaMinecraftVersion::V_1_21_2, JavaMinecraftVersion::V_26_3] {
            packet.write_packet_data(Vec::new(), &version).unwrap();
        }
    }

    #[test]
    fn dynamic_stonecutting_entry_uses_stonecutter_display_and_category() {
        use crate::codec::recipe::{
            OwnedRecipeIngredient, OwnedRecipeResult, OwnedStonecuttingRecipe,
        };

        let recipe = OwnedStonecuttingRecipe {
            recipe_id: "test:stone_bricks_from_stone".to_string(),
            ingredient: OwnedRecipeIngredient::Simple("minecraft:stone".to_string()),
            result: OwnedRecipeResult {
                item_id: "minecraft:stone_bricks".to_string(),
                count: 1,
            },
        };

        for version in [JavaMinecraftVersion::V_1_21_2, JavaMinecraftVersion::V_26_3] {
            let mut bytes = Vec::new();
            write_dynamic_stonecutting_entry(&mut bytes, 0, version, 0, &recipe).unwrap();
            // 显示 id 0，然后是切石机显示类型
            assert_eq!(bytes[0], 0);
            assert_eq!(bytes[1], RECIPE_DISPLAY_STONECUTTER as u8);
            // 组为 none (0)，类别为切石（stonecutter），现有要求包含一个仅有一项的 holder 集
            assert!(
                bytes
                    .windows(5)
                    .any(|w| w == [0, CATEGORY_STONECUTTER as u8, 1, 1, 2]),
                "missing stonecutter category in {bytes:?}"
            );
        }
    }

    #[test]
    fn dynamic_smithing_transform_entry_uses_smithing_display_and_category() {
        use crate::codec::recipe::{OwnedRecipeIngredient, OwnedRecipeResult, OwnedSmithingRecipe};

        let recipe = OwnedSmithingRecipe::Transform {
            recipe_id: "test:netherite_upgrade".to_string(),
            template: OwnedRecipeIngredient::Simple(
                "minecraft:netherite_upgrade_smithing_template".to_string(),
            ),
            base: OwnedRecipeIngredient::Simple("minecraft:diamond_chestplate".to_string()),
            addition: OwnedRecipeIngredient::Simple("minecraft:netherite_ingot".to_string()),
            result: OwnedRecipeResult {
                item_id: "minecraft:netherite_chestplate".to_string(),
                count: 1,
            },
            copy_components: true,
        };

        for version in [JavaMinecraftVersion::V_1_21_2, JavaMinecraftVersion::V_26_3] {
            let mut bytes = Vec::new();
            write_dynamic_smithing_entry(&mut bytes, 0, version, 0, &recipe).unwrap();
            // 显示 id 0，然后是锻造（smithing）显示类型
            assert_eq!(bytes[0], 0);
            assert_eq!(bytes[1], RECIPE_DISPLAY_SMITHING as u8);
            // 组为 none (0)，类别为锻造（smithing），现有要求包含三个 holder 集
            assert!(
                bytes
                    .windows(5)
                    .any(|w| w == [0, CATEGORY_SMITHING as u8, 1, 3, 2]),
                "missing smithing category in {bytes:?}"
            );
        }
    }

    #[test]
    fn dynamic_stonecutting_and_smithing_serialize_inside_the_packet() {
        use crate::codec::recipe::{
            DynamicRecipe, OwnedRecipeIngredient, OwnedRecipeResult, OwnedSmithingRecipe,
            OwnedStonecuttingRecipe,
        };

        let recipes = [
            DynamicRecipe::Stonecutting(OwnedStonecuttingRecipe {
                recipe_id: "test:cut".to_string(),
                ingredient: OwnedRecipeIngredient::Simple("minecraft:stone".to_string()),
                result: OwnedRecipeResult {
                    item_id: "minecraft:stone_bricks".to_string(),
                    count: 1,
                },
            }),
            DynamicRecipe::Smithing(OwnedSmithingRecipe::Trim {
                recipe_id: "test:trim".to_string(),
                template: OwnedRecipeIngredient::Simple(
                    "minecraft:silence_armor_trim_smithing_template".to_string(),
                ),
                base: OwnedRecipeIngredient::Simple("minecraft:diamond_chestplate".to_string()),
                addition: OwnedRecipeIngredient::Simple("minecraft:amethyst_shard".to_string()),
            }),
        ];

        let packet = CRecipeBookAdd::new(true, &recipes);
        for version in [JavaMinecraftVersion::V_1_21_2, JavaMinecraftVersion::V_26_3] {
            packet.write_packet_data(Vec::new(), &version).unwrap();
        }
    }
}
