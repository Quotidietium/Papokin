use std::sync::OnceLock;

use pumpkin_data::{
    item::{Item, JavaToBedrockItemMapping},
    recipes::{CraftingRecipeTypes, RecipeIngredientTypes, RecipeResultStruct},
};
use pumpkin_protocol::{
    bedrock::{
        client::{
            BedrockFurnaceRecipe, BedrockRecipe, BedrockShapedRecipe, BedrockShapelessRecipe,
            BedrockSmithingTransformRecipe, BedrockSmithingTrimRecipe, ContainerMixRecipe,
            ItemDescriptorCount, PotionMixRecipe, RecipeUnlockRequirement,
        },
        network_item::NetworkItemDescriptor,
    },
    codec::{
        recipe::{
            DynamicRecipe, OwnedCookingRecipeType, OwnedCraftingRecipe, OwnedRecipeIngredient,
            OwnedRecipeResult, OwnedSmithingRecipe,
        },
        var_int::VarInt,
        var_uint::VarUInt,
    },
};
use uuid::Uuid;

// These Java tags also exist in Bedrock 1.26.40. Java-only tags are expanded below.
const NATIVE_WOOD_TAGS: &[&str] = &[
    "minecraft:crimson_stems",
    "minecraft:logs",
    "minecraft:logs_that_burn",
    "minecraft:mangrove_logs",
    "minecraft:planks",
    "minecraft:warped_stems",
    "minecraft:wooden_slabs",
];

/// Everything the Bedrock `CraftingData` packet needs: the recipe entries plus
/// the potion/container mix arrays that brewing recipes map to.
pub struct BedrockCraftingData {
    pub recipes: Vec<BedrockRecipe>,
    pub potion_mixes: Vec<PotionMixRecipe>,
    pub container_mixes: Vec<ContainerMixRecipe>,
}

fn vanilla_recipes() -> &'static [BedrockRecipe] {
    static RECIPES: OnceLock<Vec<BedrockRecipe>> = OnceLock::new();
    RECIPES.get_or_init(build).as_slice()
}

/// The vanilla recipe list is built once; dynamic recipes are mapped from the
/// current `RecipeManager` contents on every call.
pub fn crafting_data(dynamic: &[DynamicRecipe]) -> BedrockCraftingData {
    let vanilla = vanilla_recipes();
    let mut recipes = vanilla.to_vec();
    // Vanilla network ids are handed out sequentially starting at 1.
    let mut network_id = vanilla.len() as u32 + 1;
    let potion_mixes = Vec::new();
    let container_mixes = Vec::new();

    for recipe in dynamic {
        match recipe {
            DynamicRecipe::Crafting(crafting) => {
                map_dynamic_crafting(crafting, &mut recipes, &mut network_id);
            }
            DynamicRecipe::Cooking(cooking) => {
                map_dynamic_cooking(cooking, &mut recipes, &mut network_id);
            }
            DynamicRecipe::Stonecutting(stonecutting) => {
                map_dynamic_stonecutting(stonecutting, &mut recipes, &mut network_id);
            }
            DynamicRecipe::Smithing(smithing) => {
                map_dynamic_smithing(smithing, &mut recipes, &mut network_id);
            }
            DynamicRecipe::Brewing(_) => {
                // Brewing maps to PotionMixRecipe entries, but those need the
                // Bedrock potion type meta for input/output and pumpkin-data
                // has no Java -> Bedrock potion meta mapping, so dynamic
                // brewing recipes are not synced to Bedrock yet.
            }
        }
    }

    BedrockCraftingData {
        recipes,
        potion_mixes,
        container_mixes,
    }
}

fn next_recipe_identity(network_id: &mut u32) -> (String, Uuid, VarUInt) {
    let id = *network_id;
    *network_id += 1;
    (
        format!("pumpkin:recipe_{id}"),
        Uuid::from_u128(u128::from(id)),
        VarUInt(id),
    )
}

fn map_dynamic_crafting(
    crafting: &OwnedCraftingRecipe,
    recipes: &mut Vec<BedrockRecipe>,
    network_id: &mut u32,
) {
    match crafting {
        OwnedCraftingRecipe::Shaped {
            key,
            pattern,
            result,
            ..
        } => {
            let height = pattern.len() as i32;
            let width = pattern.iter().map(String::len).max().unwrap_or(0) as i32;
            let mut options = Vec::new();
            for row in pattern {
                for column in 0..width as usize {
                    let symbol = row.chars().nth(column).unwrap_or(' ');
                    options.push(if symbol == ' ' {
                        vec![ItemDescriptorCount::empty()]
                    } else {
                        key.iter()
                            .find(|(key, _)| *key == symbol)
                            .map_or_else(Vec::new, |(_, ingredient)| {
                                owned_ingredient_options(ingredient)
                            })
                    });
                }
            }

            let Some(output) = owned_output_descriptor(result) else {
                return;
            };
            for input in ingredient_variants(&options) {
                let (recipe_id, uuid, recipe_network_id) = next_recipe_identity(network_id);
                recipes.push(BedrockRecipe::Shaped(BedrockShapedRecipe {
                    recipe_id,
                    width: VarInt(width),
                    height: VarInt(height),
                    input,
                    output: vec![output.clone()],
                    uuid,
                    block: "crafting_table".to_string(),
                    priority: VarInt(1),
                    assume_symmetry: true,
                    unlock_requirement: RecipeUnlockRequirement { context: 1 },
                    recipe_network_id,
                }));
            }
        }
        OwnedCraftingRecipe::Shapeless {
            ingredients,
            result,
            ..
        } => {
            let options = ingredients
                .iter()
                .map(owned_ingredient_options)
                .collect::<Vec<_>>();
            let Some(output) = owned_output_descriptor(result) else {
                return;
            };
            for input in ingredient_variants(&options) {
                let (recipe_id, uuid, recipe_network_id) = next_recipe_identity(network_id);
                recipes.push(BedrockRecipe::Shapeless(BedrockShapelessRecipe {
                    recipe_id,
                    input,
                    output: vec![output.clone()],
                    uuid,
                    block: "crafting_table".to_string(),
                    priority: VarInt(1),
                    unlock_requirement: RecipeUnlockRequirement { context: 1 },
                    recipe_network_id,
                }));
            }
        }
    }
}

fn map_dynamic_cooking(
    cooking: &OwnedCookingRecipeType,
    recipes: &mut Vec<BedrockRecipe>,
    network_id: &mut u32,
) {
    let (recipe, block) = match cooking {
        OwnedCookingRecipeType::Smelting(recipe) => (recipe, "furnace"),
        OwnedCookingRecipeType::Blasting(recipe) => (recipe, "blast_furnace"),
        OwnedCookingRecipeType::Smoking(recipe) => (recipe, "smoker"),
        OwnedCookingRecipeType::CampfireCooking(recipe) => (recipe, "campfire"),
    };

    let options = owned_ingredient_options(&recipe.ingredient);
    let Some(output) = owned_output_descriptor(&recipe.result) else {
        return;
    };
    for input in ingredient_variants(&[options]) {
        let [input] = input.as_slice() else {
            continue;
        };
        let (recipe_id, _, recipe_network_id) = next_recipe_identity(network_id);
        recipes.push(BedrockRecipe::Furnace(BedrockFurnaceRecipe {
            recipe_id,
            input: input.clone(),
            output: output.clone(),
            block: block.to_string(),
            recipe_network_id,
        }));
    }
}

fn map_dynamic_stonecutting(
    stonecutting: &pumpkin_protocol::codec::recipe::OwnedStonecuttingRecipe,
    recipes: &mut Vec<BedrockRecipe>,
    network_id: &mut u32,
) {
    let options = owned_ingredient_options(&stonecutting.ingredient);
    let Some(output) = owned_output_descriptor(&stonecutting.result) else {
        return;
    };
    for input in ingredient_variants(&[options]) {
        let (recipe_id, uuid, recipe_network_id) = next_recipe_identity(network_id);
        recipes.push(BedrockRecipe::Shapeless(BedrockShapelessRecipe {
            recipe_id,
            input,
            output: vec![output.clone()],
            uuid,
            block: "stonecutter".to_string(),
            priority: VarInt(1),
            unlock_requirement: RecipeUnlockRequirement { context: 1 },
            recipe_network_id,
        }));
    }
}

fn map_dynamic_smithing(
    smithing: &OwnedSmithingRecipe,
    recipes: &mut Vec<BedrockRecipe>,
    network_id: &mut u32,
) {
    match smithing {
        OwnedSmithingRecipe::Transform {
            template,
            base,
            addition,
            result,
            ..
        } => {
            let options = [template, base, addition]
                .iter()
                .copied()
                .map(owned_ingredient_options)
                .collect::<Vec<_>>();
            let Some(output) = owned_output_descriptor(result) else {
                return;
            };
            for variant in ingredient_variants(&options) {
                let [template, base, addition] = variant.as_slice() else {
                    continue;
                };
                let (recipe_id, _, recipe_network_id) = next_recipe_identity(network_id);
                recipes.push(BedrockRecipe::SmithingTransform(
                    BedrockSmithingTransformRecipe {
                        recipe_id,
                        template: template.clone(),
                        base: base.clone(),
                        addition: addition.clone(),
                        result: output.clone(),
                        block: "smithing_table".to_string(),
                        recipe_network_id,
                    },
                ));
            }
        }
        OwnedSmithingRecipe::Trim {
            template,
            base,
            addition,
            ..
        } => {
            let options = [template, base, addition]
                .iter()
                .copied()
                .map(owned_ingredient_options)
                .collect::<Vec<_>>();
            for variant in ingredient_variants(&options) {
                let [template, base, addition] = variant.as_slice() else {
                    continue;
                };
                let (recipe_id, _, recipe_network_id) = next_recipe_identity(network_id);
                recipes.push(BedrockRecipe::SmithingTrim(BedrockSmithingTrimRecipe {
                    recipe_id,
                    template: template.clone(),
                    base: base.clone(),
                    addition: addition.clone(),
                    block: "smithing_table".to_string(),
                    recipe_network_id,
                }));
            }
        }
    }
}

fn item_descriptor(identifier: &str) -> Option<ItemDescriptorCount> {
    let item = Item::from_registry_key(identifier)?;
    let mapping = JavaToBedrockItemMapping::from_java_item_id(item.id)?;
    Some(ItemDescriptorCount::item(
        mapping.bedrock_item.registry_key.to_string(),
        mapping.bedrock_data as i32,
    ))
}

fn tag_options(tag: &str) -> Vec<ItemDescriptorCount> {
    let tag = tag.strip_prefix('#').unwrap_or(tag);
    if NATIVE_WOOD_TAGS.contains(&tag) {
        return vec![ItemDescriptorCount::tag(tag.to_string())];
    }

    pumpkin_data::tag::get_tag_ids(pumpkin_data::tag::RegistryKey::Item, tag)
        .into_iter()
        .flatten()
        .filter_map(|&id| Item::from_id(id))
        .filter_map(|item| item_descriptor(item.registry_key))
        .fold(Vec::new(), push_unique)
}

fn one_of_options<'a>(identifiers: impl Iterator<Item = &'a str>) -> Vec<ItemDescriptorCount> {
    identifiers
        .filter_map(item_descriptor)
        .fold(Vec::new(), push_unique)
}

fn ingredient_options(ingredient: &RecipeIngredientTypes) -> Vec<ItemDescriptorCount> {
    match ingredient {
        RecipeIngredientTypes::Simple(identifier) => {
            item_descriptor(identifier).into_iter().collect()
        }
        RecipeIngredientTypes::Tagged(tag) => tag_options(tag),
        RecipeIngredientTypes::OneOf(identifiers) => one_of_options(identifiers.iter().copied()),
    }
}

fn owned_ingredient_options(ingredient: &OwnedRecipeIngredient) -> Vec<ItemDescriptorCount> {
    match ingredient {
        OwnedRecipeIngredient::Simple(identifier) => {
            item_descriptor(identifier).into_iter().collect()
        }
        OwnedRecipeIngredient::Tagged(tag) => tag_options(tag),
        OwnedRecipeIngredient::OneOf(identifiers) => {
            one_of_options(identifiers.iter().map(String::as_str))
        }
    }
}

fn push_unique(
    mut options: Vec<ItemDescriptorCount>,
    option: ItemDescriptorCount,
) -> Vec<ItemDescriptorCount> {
    if !options.contains(&option) {
        options.push(option);
    }
    options
}

/// Expands the per-slot option lists of a recipe into the variants Bedrock
/// needs, one entry per variant holding the chosen descriptor of every slot.
///
/// Slots sharing an identical option list must resolve to the same choice (the
/// two planks of a stick recipe, for example), so identical lists collapse into
/// a single dimension. Slots with different lists are independent, so every
/// combination of their choices is a valid variant.
fn ingredient_variants(options: &[Vec<ItemDescriptorCount>]) -> Vec<Vec<ItemDescriptorCount>> {
    if options.iter().any(Vec::is_empty) {
        return Vec::new();
    }

    let mut dimensions: Vec<(&[ItemDescriptorCount], Vec<usize>)> = Vec::new();
    for (slot, list) in options.iter().enumerate() {
        let list = list.as_slice();
        match dimensions.iter_mut().find(|(options, _)| *options == list) {
            Some((_, slots)) => slots.push(slot),
            None => dimensions.push((list, vec![slot])),
        }
    }

    let mut variants = vec![vec![ItemDescriptorCount::empty(); options.len()]];
    for (list, slots) in &dimensions {
        let mut combined = Vec::with_capacity(variants.len() * list.len());
        for variant in &variants {
            for choice in *list {
                let mut variant = variant.clone();
                for &slot in slots {
                    variant[slot] = choice.clone();
                }
                combined.push(variant);
            }
        }
        variants = combined;
    }

    variants
}

fn output_descriptor(result: &RecipeResultStruct) -> Option<NetworkItemDescriptor> {
    bedrock_output(result.id, result.count)
}

fn owned_output_descriptor(result: &OwnedRecipeResult) -> Option<NetworkItemDescriptor> {
    bedrock_output(&result.item_id, result.count)
}

fn bedrock_output(item_id: &str, count: u8) -> Option<NetworkItemDescriptor> {
    let item = Item::from_registry_key(item_id)?;
    let mapping = JavaToBedrockItemMapping::from_java_item_id(item.id)?;
    Some(NetworkItemDescriptor {
        id: VarInt::from(mapping.bedrock_item.id),
        stack_size: count as u16,
        aux_value: VarUInt(mapping.bedrock_data),
        block_runtime_id: VarInt::from(mapping.bedrock_block_state),
        nbt_data: pumpkin_nbt::Nbt::default(),
        place_on_blocks: Vec::new(),
        destroy_blocks: Vec::new(),
        shield_blocking_tick: 0,
    })
}

fn build() -> Vec<BedrockRecipe> {
    let mut recipes = Vec::new();
    let mut network_id = 1u32;

    for recipe in pumpkin_data::recipes::RECIPES_CRAFTING {
        match recipe {
            CraftingRecipeTypes::CraftingShaped {
                key,
                pattern,
                result,
                ..
            } => {
                let height = pattern.len() as i32;
                let width = pattern.iter().map(|row| row.len()).max().unwrap_or(0) as i32;
                let mut options = Vec::new();
                for row in *pattern {
                    for column in 0..width as usize {
                        let symbol = row.chars().nth(column).unwrap_or(' ');
                        options.push(if symbol == ' ' {
                            vec![ItemDescriptorCount::empty()]
                        } else {
                            key.iter()
                                .find(|(key, _)| *key == symbol)
                                .map_or_else(Vec::new, |(_, ingredient)| {
                                    ingredient_options(ingredient)
                                })
                        });
                    }
                }

                let Some(output) = output_descriptor(result) else {
                    continue;
                };
                for input in ingredient_variants(&options) {
                    recipes.push(BedrockRecipe::Shaped(BedrockShapedRecipe {
                        recipe_id: format!("pumpkin:recipe_{network_id}"),
                        width: VarInt(width),
                        height: VarInt(height),
                        input,
                        output: vec![output.clone()],
                        uuid: Uuid::from_u128(u128::from(network_id)),
                        block: "crafting_table".to_string(),
                        priority: VarInt(1),
                        assume_symmetry: true,
                        unlock_requirement: RecipeUnlockRequirement { context: 1 },
                        recipe_network_id: VarUInt(network_id),
                    }));
                    network_id += 1;
                }
            }
            CraftingRecipeTypes::CraftingShapeless {
                ingredients,
                result,
                ..
            } => {
                let options = ingredients
                    .iter()
                    .map(ingredient_options)
                    .collect::<Vec<_>>();
                let Some(output) = output_descriptor(result) else {
                    continue;
                };
                for input in ingredient_variants(&options) {
                    recipes.push(BedrockRecipe::Shapeless(BedrockShapelessRecipe {
                        recipe_id: format!("pumpkin:recipe_{network_id}"),
                        input,
                        output: vec![output.clone()],
                        uuid: Uuid::from_u128(u128::from(network_id)),
                        block: "crafting_table".to_string(),
                        priority: VarInt(1),
                        unlock_requirement: RecipeUnlockRequirement { context: 1 },
                        recipe_network_id: VarUInt(network_id),
                    }));
                    network_id += 1;
                }
            }
            _ => {}
        }
    }

    recipes
}

#[cfg(test)]
mod tests {
    use pumpkin_data::recipes::RecipeIngredientTypes;
    use pumpkin_protocol::bedrock::client::{ItemDescriptorCount, RecipeItemDescriptor};
    use pumpkin_protocol::codec::recipe::{
        DynamicRecipe, OwnedBrewingRecipe, OwnedCookingRecipe, OwnedCookingRecipeType,
        OwnedRecipeIngredient, OwnedRecipeResult, OwnedSmithingRecipe, OwnedStonecuttingRecipe,
    };

    use super::{BedrockRecipe, crafting_data, ingredient_options, ingredient_variants};

    fn item(identifier: &str) -> ItemDescriptorCount {
        ItemDescriptorCount::item(identifier.to_string(), 0)
    }

    fn identifiers(variant: &[ItemDescriptorCount]) -> Vec<&str> {
        variant
            .iter()
            .map(|option| match &option.descriptor {
                RecipeItemDescriptor::Item { identifier, .. } => identifier.as_str(),
                other => panic!("expected an item descriptor, got {other:?}"),
            })
            .collect()
    }

    #[test]
    fn native_planks_tag_is_preserved() {
        let options = ingredient_options(&RecipeIngredientTypes::Tagged("#minecraft:planks"));

        assert_eq!(options.len(), 1);
        assert!(matches!(
            options.first().map(|option| &option.descriptor),
            Some(RecipeItemDescriptor::Tag(tag)) if tag == "minecraft:planks"
        ));
    }

    #[test]
    fn java_only_log_tag_is_expanded() {
        let options = ingredient_options(&RecipeIngredientTypes::Tagged("#minecraft:oak_logs"));
        let identifiers = options
            .iter()
            .filter_map(|option| match &option.descriptor {
                RecipeItemDescriptor::Item { identifier, .. } => Some(identifier.as_str()),
                RecipeItemDescriptor::Empty | RecipeItemDescriptor::Tag(_) => None,
            })
            .collect::<Vec<_>>();

        assert!(identifiers.contains(&"minecraft:oak_log"));
        assert!(identifiers.contains(&"minecraft:stripped_oak_log"));
        assert!(identifiers.contains(&"minecraft:oak_wood"));
        assert!(identifiers.contains(&"minecraft:stripped_oak_wood"));
    }

    #[test]
    fn repeated_wood_type_stays_aligned_across_variants() {
        let options = ingredient_options(&RecipeIngredientTypes::Tagged("#minecraft:oak_logs"));
        let variants = ingredient_variants(&[options.clone(), options]);

        assert!(variants.len() > 1);
        assert!(
            variants
                .iter()
                .all(|variant| variant.first() == variant.get(1))
        );
    }

    #[test]
    fn distinct_ingredients_produce_every_combination() {
        let logs = vec![
            item("minecraft:oak_log"),
            item("minecraft:birch_log"),
            item("minecraft:spruce_log"),
        ];
        let planks = vec![item("minecraft:oak_planks"), item("minecraft:birch_planks")];

        let variants = ingredient_variants(&[logs, planks]);

        let combinations = variants
            .iter()
            .map(|variant| identifiers(variant))
            .collect::<Vec<_>>();
        assert_eq!(combinations.len(), 6);
        for log in [
            "minecraft:oak_log",
            "minecraft:birch_log",
            "minecraft:spruce_log",
        ] {
            for planks in ["minecraft:oak_planks", "minecraft:birch_planks"] {
                assert!(
                    combinations.contains(&vec![log, planks]),
                    "missing combination of {log} with {planks}"
                );
            }
        }
    }

    fn owned_result(item_id: &str) -> OwnedRecipeResult {
        OwnedRecipeResult {
            item_id: item_id.to_string(),
            count: 1,
        }
    }

    #[test]
    fn dynamic_stonecutting_maps_to_stonecutter_shapeless() {
        let dynamic = [DynamicRecipe::Stonecutting(OwnedStonecuttingRecipe {
            recipe_id: "test:cut_stone".to_string(),
            ingredient: OwnedRecipeIngredient::Simple("minecraft:stone".to_string()),
            result: owned_result("minecraft:stone_bricks"),
        })];

        let data = crafting_data(&dynamic);
        let matches = data
            .recipes
            .iter()
            .filter(|recipe| {
                matches!(recipe, BedrockRecipe::Shapeless(shapeless) if shapeless.block == "stonecutter")
            })
            .count();

        assert_eq!(matches, 1);
    }

    #[test]
    fn dynamic_cooking_maps_to_furnace_recipe_with_station_block() {
        let cooking = OwnedCookingRecipe {
            recipe_id: "test:smelt_cobble".to_string(),
            category: pumpkin_data::recipes::RecipeCategoryTypes::Blocks,
            group: None,
            ingredient: OwnedRecipeIngredient::Simple("minecraft:cobblestone".to_string()),
            cooking_time: 160,
            experience: 0.1,
            result: owned_result("minecraft:stone"),
        };
        let dynamic = [DynamicRecipe::Cooking(OwnedCookingRecipeType::Smelting(
            cooking,
        ))];

        let data = crafting_data(&dynamic);
        let matches = data
            .recipes
            .iter()
            .filter(|recipe| {
                matches!(recipe, BedrockRecipe::Furnace(furnace) if furnace.block == "furnace")
            })
            .count();

        assert_eq!(matches, 1);
    }

    #[test]
    fn dynamic_smithing_transform_maps_to_smithing_transform_recipe() {
        let dynamic = [DynamicRecipe::Smithing(OwnedSmithingRecipe::Transform {
            recipe_id: "test:netherite_upgrade".to_string(),
            template: OwnedRecipeIngredient::Simple(
                "minecraft:netherite_upgrade_smithing_template".to_string(),
            ),
            base: OwnedRecipeIngredient::Simple("minecraft:diamond_chestplate".to_string()),
            addition: OwnedRecipeIngredient::Simple("minecraft:netherite_ingot".to_string()),
            result: owned_result("minecraft:netherite_chestplate"),
            copy_components: true,
        })];

        let data = crafting_data(&dynamic);
        let matches = data
            .recipes
            .iter()
            .filter(|recipe| {
                matches!(recipe, BedrockRecipe::SmithingTransform(recipe) if recipe.block == "smithing_table")
            })
            .count();

        assert_eq!(matches, 1);
    }

    #[test]
    fn dynamic_brewing_maps_nowhere_without_potion_meta_mapping() {
        let dynamic = [DynamicRecipe::Brewing(OwnedBrewingRecipe {
            recipe_id: "test:brew".to_string(),
            input_item: "minecraft:potion".to_string(),
            input_potion: Some("minecraft:awkward".to_string()),
            reagent: "minecraft:blaze_powder".to_string(),
            output_item: "minecraft:potion".to_string(),
            output_potion: Some("minecraft:strength".to_string()),
        })];

        let data = crafting_data(&dynamic);
        let vanilla_only = crafting_data(&[]);

        assert_eq!(data.recipes.len(), vanilla_only.recipes.len());
        assert!(data.potion_mixes.is_empty());
        assert!(data.container_mixes.is_empty());
    }
}
