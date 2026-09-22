use std::{collections::BTreeMap, fs};

use proc_macro2::TokenStream;
use quote::{ToTokens, quote};
use serde::Deserialize;

/// 从 `recipes.json` 反序列化得到的配方条目，由 `"type"` 字段标记。
#[derive(Deserialize)]
#[serde(tag = "type")]
pub enum RecipeTypes {
    /// 高炉配方。
    #[serde(rename = "minecraft:blasting")]
    Blasting(CookingRecipeStruct),
    /// 营火烹饪配方。
    #[serde(rename = "minecraft:campfire_cooking")]
    CampfireCooking(CookingRecipeStruct),
    /// 有序工作台配方。
    #[serde(rename = "minecraft:crafting_shaped")]
    CraftingShaped(CraftingShapedRecipeStruct),
    /// 无序工作台配方。
    #[serde(rename = "minecraft:crafting_shapeless")]
    CraftingShapeless(CraftingShapelessRecipeStruct),
    /// 转化合成配方（将 NBT/组件从一个槽位保留到另一个槽位）。
    #[serde(rename = "minecraft:crafting_transmute")]
    CraftingTransmute(CraftingTransmuteRecipeStruct),
    /// 饰纹陶罐合成配方。
    #[serde(rename = "minecraft:crafting_decorated_pot")]
    CraftingDecoratedPot(CraftingDecoratedPotStruct),
    /// 熔炉烧炼配方。
    #[serde(rename = "minecraft:smelting")]
    Smelting(CookingRecipeStruct),
    /// 锻造台转化配方。
    #[serde(rename = "minecraft:smithing_transform")]
    SmithingTransform(SmithingTransformRecipeStruct),
    /// 锻造台盔甲纹饰配方。
    #[serde(rename = "minecraft:smithing_trim")]
    SmithingTrim(SmithingTrimRecipeStruct),
    /// 烟熏炉烹饪配方。
    #[serde(rename = "minecraft:smoking")]
    Smoking(CookingRecipeStruct),
    /// 切石机配方。
    #[serde(rename = "minecraft:stonecutting")]
    Stonecutting(StonecuttingRecipeStruct),
    /// 特殊合成配方类型。
    #[serde(rename = "minecraft:crafting_special_bannerduplicate")]
    CraftingSpecialBannerDuplicate,
    #[serde(rename = "minecraft:crafting_special_bookcloning")]
    CraftingSpecialBookCloning,
    #[serde(rename = "minecraft:crafting_special_firework_rocket")]
    CraftingSpecialFireworkRocket,
    #[serde(rename = "minecraft:crafting_special_firework_star")]
    CraftingSpecialFireworkStar,
    #[serde(rename = "minecraft:crafting_special_firework_star_fade")]
    CraftingSpecialFireworkStarFade,
    #[serde(rename = "minecraft:crafting_special_mapextending")]
    CraftingSpecialMapExtending,
    #[serde(rename = "minecraft:crafting_special_repairitem")]
    CraftingSpecialRepairItem,
    #[serde(rename = "minecraft:crafting_special_shielddecoration")]
    CraftingSpecialShieldDecoration,
    #[serde(rename = "minecraft:crafting_dye")]
    CraftingDye,
    #[serde(rename = "minecraft:crafting_imbue")]
    CraftingImbue,
    /// 其他特殊合成配方类型。
    #[serde(other)]
    #[serde(rename = "minecraft:crafting_special_*")]
    CraftingSpecial,
}

/// 反序列化得到的锻造台转化配方。
#[derive(Deserialize)]
pub struct SmithingTransformRecipeStruct {
    template: RecipeIngredientTypes,
    base: RecipeIngredientTypes,
    addition: RecipeIngredientTypes,
    result: RecipeResultStruct,
}

impl ToTokens for SmithingTransformRecipeStruct {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let template = self.template.to_token_stream();
        let base = self.base.to_token_stream();
        let addition = self.addition.to_token_stream();
        let result = self.result.to_token_stream();

        tokens.extend(quote! {
            SmithingTransformRecipe {
                template: #template,
                base: #base,
                addition: #addition,
                result: #result,
            }
        });
    }
}

/// 反序列化得到的锻造台盔甲纹饰配方。
#[derive(Deserialize)]
pub struct SmithingTrimRecipeStruct {
    template: RecipeIngredientTypes,
    base: RecipeIngredientTypes,
    addition: RecipeIngredientTypes,
    pattern: String,
}

impl ToTokens for SmithingTrimRecipeStruct {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let template = self.template.to_token_stream();
        let base = self.base.to_token_stream();
        let addition = self.addition.to_token_stream();
        let pattern = &self.pattern;

        tokens.extend(quote! {
            SmithingTrimRecipe {
                template: #template,
                base: #base,
                addition: #addition,
                pattern: #pattern,
            }
        });
    }
}

/// 反序列化得到的切石机配方。
#[derive(Deserialize)]
pub struct StonecuttingRecipeStruct {
    /// 用于进度追踪的可选配方组。
    group: Option<String>,
    /// 此配方所需的单一原料。
    ingredient: RecipeIngredientTypes,
    /// 此配方产出的物品。
    result: RecipeResultStruct,
}

impl ToTokens for StonecuttingRecipeStruct {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let group = if let Some(group) = &self.group {
            quote! { Some(#group) }
        } else {
            quote! { None }
        };
        let ingredient = self.ingredient.to_token_stream();
        let result = self.result.to_token_stream();

        tokens.extend(quote! {
            StonecutterRecipe {
                group: #group,
                ingredient: #ingredient,
                result: #result,
            }
        });
    }
}

/// 反序列化得到的烧炼配方（熔炉、高炉、烟熏炉或营火）。
#[derive(Deserialize)]
pub struct CookingRecipeStruct {
    /// 此配方的 UI 分类。
    category: Option<RecipeCategoryTypes>,
    /// 用于进度追踪的可选配方组。
    group: Option<String>,
    /// 此配方所需的单一原料。
    ingredient: RecipeIngredientTypes,
    /// 烹饪此配方所需的刻数。
    cookingtime: Option<i32>,
    /// 取出产物时奖励的经验点数。
    experience: f32,
    /// 此配方产出的物品。
    result: RecipeResultStruct,
}

impl CookingRecipeStruct {
    /// 根据产物、原料和烹饪类型生成配方 ID
    /// 格式：minecraft:{result}_from_{`cooking_type`}_{ingredient}
    fn generate_recipe_id(&self, cooking_type: &str) -> String {
        let result_id = self.result.id.as_deref().unwrap_or("air");
        let result_name = result_id.strip_prefix("minecraft:").unwrap_or(result_id);
        let ingredient_name = match &self.ingredient {
            RecipeIngredientTypes::Simple(s) => {
                if s.starts_with('#') {
                    // 带标签的配料 - 去除 # 并将 : 替换为 _
                    s.strip_prefix('#').unwrap_or(s).replace(':', "_")
                } else {
                    s.strip_prefix("minecraft:").unwrap_or(s).to_string()
                }
            }
            RecipeIngredientTypes::OneOf(items) => {
                // 使用第一个物品生成 ID
                items
                    .first()
                    .map_or("unknown", |s| s.strip_prefix("minecraft:").unwrap_or(s))
                    .to_string()
            }
        };
        format!("minecraft:{result_name}_from_{cooking_type}_{ingredient_name}")
    }

    /// 生成烹饪配方的结构体字段，其中包含给定的 `recipe_id`。
    ///
    /// # Arguments
    /// – `tokens` – 要扩展的令牌流。
    /// – `recipe_id` – 预生成的原版格式配方 ID 字符串。
    fn to_tokens_with_id(
        &self,
        tokens: &mut TokenStream,
        recipe_id: &str,
        default_cookingtime: i32,
    ) {
        let category = match &self.category {
            Some(category) => category.to_token_stream(),
            None => RecipeCategoryTypes::Misc.to_token_stream(),
        };
        let group = if let Some(group) = &self.group {
            quote! { Some(#group) }
        } else {
            quote! { None }
        };
        let ingredient = self.ingredient.to_token_stream();
        let cookingtime = self
            .cookingtime
            .unwrap_or(default_cookingtime)
            .to_token_stream();
        let experience = self.experience.to_token_stream();
        let result = self.result.to_token_stream();

        tokens.extend(quote! {
                recipe_id: #recipe_id,
                category: #category,
                group: #group,
                ingredient: #ingredient,
                cookingtime: #cookingtime,
                experience: #experience,
                result: #result,
        });
    }
}

impl ToTokens for CookingRecipeStruct {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let category = match &self.category {
            Some(category) => category.to_token_stream(),
            None => RecipeCategoryTypes::Misc.to_token_stream(),
        };
        let group = if let Some(group) = &self.group {
            quote! { Some(#group) }
        } else {
            quote! { None }
        };
        let ingredient = self.ingredient.to_token_stream();
        let cookingtime = self.cookingtime.unwrap_or(200).to_token_stream();
        let experience = self.experience.to_token_stream();
        let result = self.result.to_token_stream();

        tokens.extend(quote! {
            //CookingRecipeType::Blasting,CampfireCooking,Smelting,Smoking{
                category: #category,
                group: #group,
                ingredient: #ingredient,
                cookingtime: #cookingtime,
                experience: #experience,
                result: #result,
            //}
        });
    }
}

/// 反序列化得到的有序合成配方。
#[derive(Deserialize)]
pub struct CraftingShapedRecipeStruct {
    /// 此配方的 UI 分类。
    category: Option<RecipeCategoryTypes>,
    /// 用于进度追踪的可选配方组。
    group: Option<String>,
    /// 解锁时是否显示弹出通知。
    show_notification: Option<bool>,
    /// 从图案键字符到其配料类型的映射。
    key: BTreeMap<String, RecipeIngredientTypes>,
    /// 以行字符串定义合成网格的布局。
    pattern: Vec<String>,
    /// 此配方产出的物品。
    result: RecipeResultStruct,
}

impl ToTokens for CraftingShapedRecipeStruct {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let category = match &self.category {
            Some(category) => category.to_token_stream(),
            None => RecipeCategoryTypes::Misc.to_token_stream(),
        };
        let group = if let Some(group) = &self.group {
            quote! { Some(#group) }
        } else {
            quote! { None }
        };
        let show_notification = self.show_notification.unwrap_or(true);
        let key = self
            .key
            .iter()
            .map(|(key, ingredient)| {
                let key = key.chars().next().unwrap();
                quote! { (#key, #ingredient) }
            })
            .collect::<Vec<_>>();
        let pattern = self
            .pattern
            .iter()
            .map(quote::ToTokens::to_token_stream)
            .collect::<Vec<_>>();
        let result = self.result.to_token_stream();

        tokens.extend(quote! {
            CraftingRecipeTypes::CraftingShaped {
                category: #category,
                group: #group,
                show_notification: #show_notification,
                key: &[#(#key),*],
                pattern: &[#(#pattern),*],
                result: #result,
            }
        });
    }
}

/// 反序列化得到的无序合成配方。
#[derive(Deserialize)]
pub struct CraftingShapelessRecipeStruct {
    /// 此配方的 UI 分类。
    category: Option<RecipeCategoryTypes>,
    /// 用于进度追踪的可选配方组。
    group: Option<String>,
    /// 所需原料的无序列表。
    ingredients: Vec<RecipeIngredientTypes>,
    /// 此配方产出的物品。
    result: RecipeResultStruct,
}

impl ToTokens for CraftingShapelessRecipeStruct {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let category = match &self.category {
            Some(category) => category.to_token_stream(),
            None => RecipeCategoryTypes::Misc.to_token_stream(),
        };
        let group = if let Some(group) = &self.group {
            quote! { Some(#group) }
        } else {
            quote! { None }
        };
        let ingredients = self
            .ingredients
            .iter()
            .map(quote::ToTokens::to_token_stream)
            .collect::<Vec<_>>();
        let result = self.result.to_token_stream();

        tokens.extend(quote! {
            CraftingRecipeTypes::CraftingShapeless {
                category: #category,
                group: #group,
                ingredients: &[#(#ingredients),*],
                result: #result,
            }
        });
    }
}

/// 反序列化得到的转化合成配方（将组件从输入复制到结果）。
#[derive(Deserialize)]
pub struct CraftingTransmuteRecipeStruct {
    /// 此配方的 UI 分类。
    category: Option<RecipeCategoryTypes>,
    /// 用于进度追踪的可选配方组。
    group: Option<String>,
    /// 其数据组件被复制到结果中的物品。
    input: RecipeIngredientTypes,
    /// 与 `input` 一起消耗的材料物品。
    material: RecipeIngredientTypes,
    /// 结果的基础物品类型（从 `input` 继承组件）。
    result: RecipeResultStruct,
}

impl ToTokens for CraftingTransmuteRecipeStruct {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let category = match &self.category {
            Some(category) => category.to_token_stream(),
            None => RecipeCategoryTypes::Misc.to_token_stream(),
        };
        let group = if let Some(group) = &self.group {
            quote! { Some(#group) }
        } else {
            quote! { None }
        };
        let input = self.input.to_token_stream();
        let material = self.material.to_token_stream();
        let result = self.result.to_token_stream();

        tokens.extend(quote! {
            CraftingRecipeTypes::CraftingTransmute {
                category: #category,
                group: #group,
                input: #input,
                material: #material,
                result: #result,
            }
        });
    }
}

/// 反序列化得到的饰纹陶罐合成配方。
#[derive(Deserialize)]
pub struct CraftingDecoratedPotStruct {
    /// 此配方的 UI 分类。
    category: Option<RecipeCategoryTypes>,
}

impl ToTokens for CraftingDecoratedPotStruct {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let category = match &self.category {
            Some(category) => category.to_token_stream(),
            None => RecipeCategoryTypes::Misc.to_token_stream(),
        };

        tokens.extend(quote! {
            CraftingRecipeTypes::CraftingDecoratedPot {
                category: #category,
            }
        });
    }
}

/// 反序列化得到的配方结果，指定输出物品及数量。
#[derive(Deserialize)]
pub struct RecipeResultStruct {
    /// 结果物品的注册表键。
    id: Option<String>,
    /// 产出的结果物品数量（默认为 1）。
    count: Option<u8>,
    // TODO: components: Option<RecipeResultComponentsStruct>,
}

impl ToTokens for RecipeResultStruct {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let id = self.id.as_deref().unwrap_or("minecraft:air");
        let count = self.count.unwrap_or(1).to_token_stream();

        tokens.extend(quote! {
            RecipeResultStruct {
                id: #id,
                count: #count,
            }
        });
    }
}

/// 反序列化得到的配方配料，为单个物品/标签或备选列表。
#[derive(Deserialize)]
#[serde(untagged)]
pub enum RecipeIngredientTypes {
    /// 单个物品注册表键或标签（以 `#` 为前缀）。
    Simple(String),
    /// 一个可接受的备选物品注册表键列表。
    OneOf(Vec<String>),
}

impl ToTokens for RecipeIngredientTypes {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let name = match self {
            Self::Simple(ingredient) => {
                if ingredient.starts_with('#') {
                    quote! { RecipeIngredientTypes::Tagged(#ingredient) }
                } else {
                    quote! { RecipeIngredientTypes::Simple(#ingredient) }
                }
            }
            Self::OneOf(ingredients) => {
                let ingredients = ingredients
                    .iter()
                    .map(quote::ToTokens::to_token_stream)
                    .collect::<Vec<_>>();
                quote! { RecipeIngredientTypes::OneOf(&[#(#ingredients),*]) }
            }
        };

        tokens.extend(name);
    }
}

/// 反序列化得到的配方 UI 分类，用于在配方书中对配方分组。
#[derive(Deserialize)]
pub enum RecipeCategoryTypes {
    /// 装备配方（工具、武器、盔甲）。
    #[serde(rename = "equipment")]
    Equipment,
    /// 建筑方块配方。
    #[serde(rename = "building")]
    Building,
    /// 红石元件配方。
    #[serde(rename = "redstone")]
    Restone,
    /// 不属于其他类别的杂项配方。
    #[serde(rename = "misc")]
    Misc,
    /// 食物类物品配方。
    #[serde(rename = "food")]
    Food,
    /// 方块配方。
    #[serde(rename = "blocks")]
    Blocks,
}

impl ToTokens for RecipeCategoryTypes {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let name = match self {
            Self::Equipment => {
                quote! { RecipeCategoryTypes::Equipment }
            }
            Self::Building => {
                quote! { RecipeCategoryTypes::Building }
            }
            Self::Restone => {
                quote! { RecipeCategoryTypes::Restone }
            }
            Self::Misc => {
                quote! { RecipeCategoryTypes::Misc }
            }
            Self::Food => {
                quote! { RecipeCategoryTypes::Food }
            }
            Self::Blocks => {
                quote! { RecipeCategoryTypes::Blocks }
            }
        };

        tokens.extend(name);
    }
}

/// 从 26.2 数据包读取配方 JSON 文件，并生成完整的配方常量与辅助函数 `TokenStream`。
pub fn build() -> TokenStream {
    let dir = std::path::Path::new("../../assets/datapacks/26_3/data/minecraft/recipe");
    let mut recipes_assets: BTreeMap<String, RecipeTypes> = BTreeMap::new();
    let mut entries: Vec<_> = fs::read_dir(dir)
        .expect("缺少配方目录")
        .flatten()
        .filter(|e| e.path().extension().is_some_and(|ext| ext == "json"))
        .collect();
    entries.sort_by_key(|e| e.path());

    for entry in entries {
        let path = entry.path();
        let stem = path.file_stem().unwrap().to_string_lossy().into_owned();
        let key = format!("minecraft:{stem}");
        let content = fs::read_to_string(&path).expect("读取配方文件失败");
        let recipe = serde_json::from_str::<RecipeTypes>(&content)
            .unwrap_or_else(|e| panic!("解析配方 {} 失败：{e}", path.display()));
        recipes_assets.insert(key, recipe);
    }

    let mut crafting_recipes = Vec::new();
    let mut cooking_recipes = Vec::new();
    let mut stonecutting_recipes = Vec::new();
    let mut smithing_trim_recipes = Vec::new();
    let mut smithing_transform_recipes = Vec::new();

    for (recipe_id, recipe) in recipes_assets {
        match recipe {
            RecipeTypes::Blasting(recipe) => {
                let mut common_cooking_token = TokenStream::new();
                recipe.to_tokens_with_id(&mut common_cooking_token, &recipe_id, 100);
                let blasting_token = quote! {
                    CookingRecipeType::Blasting (CookingRecipe {
                        #common_cooking_token
                    })
                };
                cooking_recipes.push(blasting_token);
            }
            RecipeTypes::CampfireCooking(recipe) => {
                let mut common_cooking_token = TokenStream::new();
                recipe.to_tokens_with_id(&mut common_cooking_token, &recipe_id, 100);
                let campfire_token = quote! {
                    CookingRecipeType::CampfireCooking (CookingRecipe {
                        #common_cooking_token
                    })
                };
                cooking_recipes.push(campfire_token);
            }
            RecipeTypes::CraftingShaped(recipe) => {
                crafting_recipes.push(recipe.to_token_stream());
            }
            RecipeTypes::CraftingShapeless(recipe) => {
                crafting_recipes.push(recipe.to_token_stream());
            }
            RecipeTypes::CraftingTransmute(recipe) => {
                crafting_recipes.push(recipe.to_token_stream());
            }
            RecipeTypes::CraftingDecoratedPot(recipe) => {
                crafting_recipes.push(recipe.to_token_stream());
            }
            RecipeTypes::Smelting(recipe) => {
                let mut common_cooking_token = TokenStream::new();
                recipe.to_tokens_with_id(&mut common_cooking_token, &recipe_id, 200);
                let smelting_token = quote! {
                    CookingRecipeType::Smelting(CookingRecipe {
                        #common_cooking_token
                    })
                };
                cooking_recipes.push(smelting_token);
            }
            RecipeTypes::SmithingTransform(recipe) => {
                smithing_transform_recipes.push(recipe.to_token_stream());
            }
            RecipeTypes::SmithingTrim(recipe) => {
                smithing_trim_recipes.push(recipe.to_token_stream());
            }
            RecipeTypes::Smoking(recipe) => {
                let mut common_cooking_token = TokenStream::new();
                recipe.to_tokens_with_id(&mut common_cooking_token, &recipe_id, 100);
                let smoking_token = quote! {
                    CookingRecipeType::Smoking(CookingRecipe{
                        #common_cooking_token
                    })
                };
                cooking_recipes.push(smoking_token);
            }
            RecipeTypes::Stonecutting(recipe) => {
                stonecutting_recipes.push(recipe.to_token_stream());
            }
            RecipeTypes::CraftingSpecial
            | RecipeTypes::CraftingSpecialBannerDuplicate
            | RecipeTypes::CraftingSpecialBookCloning
            | RecipeTypes::CraftingSpecialFireworkRocket
            | RecipeTypes::CraftingSpecialFireworkStar
            | RecipeTypes::CraftingSpecialFireworkStarFade
            | RecipeTypes::CraftingSpecialMapExtending
            | RecipeTypes::CraftingSpecialRepairItem
            | RecipeTypes::CraftingSpecialShieldDecoration
            | RecipeTypes::CraftingDye
            | RecipeTypes::CraftingImbue => {}
        }
    }

    quote! {
        use crate::tag::Taggable;
        use crate::item::Item;
        use serde::{Serialize, Deserialize};

        #[derive(Clone, Debug, Serialize)]
        pub enum CraftingRecipeTypes {
            CraftingShaped {
                category: RecipeCategoryTypes,
                group: Option<&'static str>,
                show_notification: bool,
                key: &'static [(char, RecipeIngredientTypes)],
                pattern: &'static [&'static str],
                result: RecipeResultStruct,
            },
            CraftingShapeless {
                category: RecipeCategoryTypes,
                group: Option<&'static str>,
                ingredients: &'static [RecipeIngredientTypes],
                result: RecipeResultStruct,
            },
            CraftingTransmute {
                category: RecipeCategoryTypes,
                group: Option<&'static str>,
                input: RecipeIngredientTypes,
                material: RecipeIngredientTypes,
                result: RecipeResultStruct,
            },
            CraftingDecoratedPot {
                category: RecipeCategoryTypes,
            },
            CraftingSpecial,
        }

        #[allow(dead_code)]
        #[derive(Clone, Debug, Serialize)]
        pub struct CookingRecipe {
            /// 兼容原版的配方 ID（例如 "minecraft:iron_ingot_from_smelting_iron_ore"）
            pub recipe_id: &'static str,
            pub category: RecipeCategoryTypes,
            pub group: Option<&'static str>,
            pub ingredient: RecipeIngredientTypes,
            pub cookingtime: i32,
            pub experience: f32,
            pub result: RecipeResultStruct,
        }

        #[derive(Clone, Debug, Serialize)]
        pub enum CookingRecipeType {
            Blasting(CookingRecipe),
            Smelting(CookingRecipe),
            Smoking(CookingRecipe),
            CampfireCooking(CookingRecipe),
        }
        #[derive(Clone, Debug, Serialize)]
        pub enum CookingRecipeKind {
            Blasting,
            Smelting,
            Smoking,
            CampfireCooking,
        }

        impl From<&CookingRecipeType> for CookingRecipeKind {
            fn from(recipe_type: &CookingRecipeType) -> Self {
                match recipe_type {
                    CookingRecipeType::Blasting(_) => Self::Blasting,
                    CookingRecipeType::Smelting(_) => Self::Smelting,
                    CookingRecipeType::Smoking(_) => Self::Smoking,
                    CookingRecipeType::CampfireCooking(_) => Self::CampfireCooking,
                }
            }
        }

        impl From<CookingRecipeType> for CookingRecipeKind {
            fn from(recipe_type: CookingRecipeType) -> Self {
                match recipe_type {
                    CookingRecipeType::Blasting(_) => Self::Blasting,
                    CookingRecipeType::Smelting(_) => Self::Smelting,
                    CookingRecipeType::Smoking(_) => Self::Smoking,
                    CookingRecipeType::CampfireCooking(_) => Self::CampfireCooking,
                }
            }
        }

        impl CookingRecipeKind {
            #[must_use]
            pub const fn to_type(self, recipe: CookingRecipe) -> CookingRecipeType {
                match self {
                    Self::Blasting => CookingRecipeType::Blasting(recipe),
                    Self::Smelting => CookingRecipeType::Smelting(recipe),
                    Self::Smoking => CookingRecipeType::Smoking(recipe),
                    Self::CampfireCooking => CookingRecipeType::CampfireCooking(recipe),
                }
            }
        }

        #[derive(Clone, Debug, Serialize)]
        pub struct StonecutterRecipe {
            pub group: Option<&'static str>,
            pub ingredient: RecipeIngredientTypes,
            pub result: RecipeResultStruct,
        }

        #[derive(Clone, Debug, Serialize)]
        pub struct RecipeResultStruct {
            pub id: &'static str,
            pub count: u8,
        }

        #[derive(Clone, Debug, Serialize)]
        pub enum RecipeIngredientTypes {
            Simple(&'static str),
            Tagged(&'static str),
            OneOf(&'static [&'static str]),
        }

        impl RecipeIngredientTypes {
            #[must_use]
            pub fn match_item(&self, item: &Item) -> bool {
                match self {
                    Self::Simple(ingredient) => {
                        let name = format!("minecraft:{}", item.registry_key);
                        name == *ingredient
                    }
                    Self::Tagged(tag) => item
                        .is_tagged_with(tag)
                        .expect("合成配方使用了无效的标签"),
                    Self::OneOf(ingredients) => {
                        let name = format!("minecraft:{}", item.registry_key);
                        ingredients.contains(&name.as_str())
                    }
                }
            }
        }

        #[derive(Clone, Debug, Serialize)]
        pub enum RecipeCategoryTypes {
            Equipment,
            Building,
            Restone,
            Misc,
            Food,
            Blocks,
        }

        pub static RECIPES_CRAFTING: &[CraftingRecipeTypes] = &[
            #(#crafting_recipes),*
        ];
        pub static RECIPES_COOKING: &[CookingRecipeType] = &[
            #(#cooking_recipes ),*
        ];
        pub static RECIPES_STONECUTTING: &[StonecutterRecipe] = &[
            #(#stonecutting_recipes),*
        ];
        pub static RECIPES_SMITHING_TRIM: &[SmithingTrimRecipe] = &[
            #(#smithing_trim_recipes),*
        ];
        pub static RECIPES_SMITHING_TRANSFORM: &[SmithingTransformRecipe] = &[
            #(#smithing_transform_recipes),*
        ];

        #[derive(Clone, Debug, Serialize)]
        pub struct SmithingTrimRecipe {
            pub template: RecipeIngredientTypes,
            pub base: RecipeIngredientTypes,
            pub addition: RecipeIngredientTypes,
            pub pattern: &'static str,
        }

        impl SmithingTrimRecipe {
            #[must_use]
            pub fn matches(&self, template: &Item, base: &Item, addition: &Item) -> bool {
                self.template.match_item(template)
                    && self.base.match_item(base)
                    && self.addition.match_item(addition)
            }
        }

        #[must_use]
        pub fn get_smithing_trim_recipe(
            template: &Item,
            base: &Item,
            addition: &Item,
        ) -> Option<&'static SmithingTrimRecipe> {
            RECIPES_SMITHING_TRIM
                .iter()
                .find(|recipe| recipe.matches(template, base, addition))
        }

        #[derive(Clone, Debug, Serialize)]
        pub struct SmithingTransformRecipe {
            pub template: RecipeIngredientTypes,
            pub base: RecipeIngredientTypes,
            pub addition: RecipeIngredientTypes,
            pub result: RecipeResultStruct,
        }

        impl SmithingTransformRecipe {
            #[must_use]
            pub fn matches(&self, template: &Item, base: &Item, addition: &Item) -> bool {
                self.template.match_item(template)
                    && self.base.match_item(base)
                    && self.addition.match_item(addition)
            }
        }

        #[must_use]
        pub fn get_smithing_transform_recipe(
            template: &Item,
            base: &Item,
            addition: &Item,
        ) -> Option<&'static SmithingTransformRecipe> {
            RECIPES_SMITHING_TRANSFORM
                .iter()
                .find(|recipe| recipe.matches(template, base, addition))
        }

        /// 返回给定物品的纹饰材料注册表键（若它是有效的纹饰材料）。
        #[must_use]
        pub fn get_trim_material_for_item(item: &Item) -> Option<&'static str> {
            match item.registry_key {
                "amethyst_shard" => Some("minecraft:amethyst"),
                "copper_ingot" => Some("minecraft:copper"),
                "diamond" => Some("minecraft:diamond"),
                "emerald" => Some("minecraft:emerald"),
                "gold_ingot" => Some("minecraft:gold"),
                "iron_ingot" => Some("minecraft:iron"),
                "lapis_lazuli" => Some("minecraft:lapis"),
                "netherite_ingot" => Some("minecraft:netherite"),
                "quartz" => Some("minecraft:quartz"),
                "redstone" => Some("minecraft:redstone"),
                "resin_brick" | "resin_clump" => Some("minecraft:resin"),
                _ => None,
            }
        }

        #[must_use]
        pub fn get_cooking_recipe_with_ingredient(ingredient: &Item, recipe_type: CookingRecipeKind) -> Option<&'static CookingRecipe> {
            RECIPES_COOKING
                .iter()
                .find_map(|recipe| match (recipe, &recipe_type) {
                    (CookingRecipeType::Blasting(cooking_recipe), CookingRecipeKind::Blasting)
                    | (CookingRecipeType::Smelting(cooking_recipe), CookingRecipeKind::Smelting)
                    | (CookingRecipeType::Smoking(cooking_recipe), CookingRecipeKind::Smoking)
                    | (
                        CookingRecipeType::CampfireCooking(cooking_recipe),
                        CookingRecipeKind::CampfireCooking,
                    ) => {
                        cooking_recipe.ingredient.match_item(ingredient).then_some(cooking_recipe)
                    }
                    _ => None,
                })
        }

        /// 根据配方 ID 获取配方的经验值。
        /// 用于从熔炉取出时计算经验。
        /// 配方 ID 采用原版格式，例如 `"minecraft:iron_ingot_from_smelting_iron_ore"`
        #[must_use]
        pub fn get_recipe_experience(recipe_id: &str) -> Option<f32> {
            RECIPES_COOKING.iter().find_map(|recipe| {
                let cooking_recipe = match recipe {
                    CookingRecipeType::Blasting(r)
                    | CookingRecipeType::Smelting(r)
                    | CookingRecipeType::Smoking(r)
                    | CookingRecipeType::CampfireCooking(r) => r,
                };
                (cooking_recipe.recipe_id == recipe_id).then_some(cooking_recipe.experience)
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_recipes() {
        let _ = build();
    }
}
