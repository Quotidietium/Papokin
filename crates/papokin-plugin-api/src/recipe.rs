//! 插件配方注册与构建器工具。
//!
//! 本模块提供流畅、类型安全的 API，用于定义并注册自定义
//! 合成配方（有序与无序）、烹饪配方（熔炼、高炉、
//! 烟熏、营火）、切石配方、锻造升级与纹饰配方，
//! 以及酿造配方。
//!
//! # Examples
//!
//! ## Registering a Shaped Recipe
//! ```rust,ignore
//! use papokin_plugin_api::{
//!     recipe::{Ingredient, RecipeCategory, ShapedRecipeBuilder},
//!     ItemStack, Server,
//! };
//!
//! fn register_recipes(server: &Server) {
//!     let manager = server.get_recipe_manager();
//!
//!     manager.register(
//!         ShapedRecipeBuilder::new("my_plugin:super_sword", ItemStack::new("minecraft:diamond_sword", 1))
//!             .pattern([
//!                 " D ",
//!                 " D ",
//!                 " S ",
//!             ])
//!             .key('D', "minecraft:diamond_block")
//!             .key('S', "minecraft:stick")
//!             .category(RecipeCategory::Equipment)
//!             .group("swords")
//!             .show_notification(true)
//!     ).expect("failed to register shaped recipe");
//! }
//! ```
//!
//! ## Registering a Shapeless Recipe
//! ```rust,ignore
//! use papokin_plugin_api::{
//!     recipe::{Ingredient, RecipeCategory, ShapelessRecipeBuilder},
//!     ItemStack, Server,
//! };
//!
//! fn register_recipes(server: &Server) {
//!     let manager = server.get_recipe_manager();
//!
//!     manager.register(
//!         ShapelessRecipeBuilder::new("my_plugin:flint_from_gravel", ItemStack::new("minecraft:flint", 1))
//!             .ingredient_count("minecraft:gravel", 3)
//!             .category(RecipeCategory::Misc)
//!     ).expect("failed to register shapeless recipe");
//! }
//! ```
//!
//! ## Registering a Smelting / Cooking Recipe
//! ```rust,ignore
//! use papokin_plugin_api::{
//!     recipe::{CookingRecipeBuilder, RecipeCategory},
//!     ItemStack, Server,
//! };
//!
//! fn register_recipes(server: &Server) {
//!     let manager = server.get_recipe_manager();
//!
//!     manager.register(
//!         CookingRecipeBuilder::smelting(
//!             "my_plugin:fast_iron",
//!             "minecraft:raw_iron",
//!             ItemStack::new("minecraft:iron_ingot", 1),
//!         )
//!         .cooking_time(100)
//!         .experience(0.7)
//!         .category(RecipeCategory::Misc)
//!     ).expect("failed to register smelting recipe");
//! }
//! ```
//!
//! ## Registering a Stonecutting Recipe
//! ```rust,ignore
//! use papokin_plugin_api::{recipe::StonecuttingRecipeBuilder, ItemStack, Server};
//!
//! fn register_recipes(server: &Server) {
//!     server.register_recipe(
//!         StonecuttingRecipeBuilder::new(
//!             "my_plugin:glass_panes",
//!             "minecraft:glass",
//!             ItemStack::new("minecraft:glass_pane", 4),
//!         )
//!     ).expect("failed to register stonecutting recipe");
//! }
//! ```
//!
//! ## Registering a Smithing Transform Recipe
//! ```rust,ignore
//! use papokin_plugin_api::{recipe::SmithingTransformRecipeBuilder, ItemStack, Server};
//!
//! fn register_recipes(server: &Server) {
//!     server.register_recipe(
//!         SmithingTransformRecipeBuilder::new(
//!             "my_plugin:netherite_chestplate",
//!             "minecraft:netherite_upgrade_smithing_template",
//!             "minecraft:diamond_chestplate",
//!             "minecraft:netherite_ingot",
//!             ItemStack::new("minecraft:netherite_chestplate", 1),
//!         )
//!         .copy_components(true)
//!     ).expect("failed to register smithing transform recipe");
//! }
//! ```
//!
//! ## Registering a Smithing Trim Recipe
//! ```rust,ignore
//! use papokin_plugin_api::{recipe::SmithingTrimRecipeBuilder, Server};
//!
//! fn register_recipes(server: &Server) {
//!     server.register_recipe(
//!         SmithingTrimRecipeBuilder::new(
//!             "my_plugin:coast_trim",
//!             "minecraft:coast_armor_trim_smithing_template",
//!             "minecraft:iron_chestplate",
//!             "minecraft:amethyst_shard",
//!         )
//!     ).expect("failed to register smithing trim recipe");
//! }
//! ```
//!
//! ## Registering a Brewing Recipe
//! ```rust,ignore
//! use papokin_plugin_api::{recipe::BrewingRecipeBuilder, Server};
//!
//! fn register_recipes(server: &Server) {
//!     server.register_recipe(
//!         BrewingRecipeBuilder::new(
//!             "my_plugin:splash_water",
//!             "minecraft:potion",
//!             "minecraft:gunpowder",
//!             "minecraft:splash_potion",
//!         )
//!         .input_potion("minecraft:water")
//!         .output_potion("minecraft:water")
//!     ).expect("failed to register brewing recipe");
//! }
//! ```

use std::collections::HashMap;

pub use crate::wit::papokin::plugin::recipe::{
    BrewingRecipe, CookingRecipe, CookingType, Ingredient as WitIngredient, RecipeCategory,
    RecipeManager, ShapedRecipe, ShapelessRecipe, SmithingTransformRecipe, SmithingTrimRecipe,
    StonecuttingRecipe,
};
use crate::{Context, ItemStack, Server};

/// 表示配方中的一种原料。
///
/// 原料可以是：
/// - 具体物品 ID（如 `"minecraft:diamond"` 或 `"diamond"`）。
/// - 表示一组物品的标签（如 `"#minecraft:logs"` 或 `"#logs"`）。
/// - 多个具体物品之一。
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Ingredient {
    /// 按注册表键指定的具体物品（如 `"minecraft:diamond"`）。
    Item(String),
    /// 表示多个物品的标签（如 `"minecraft:logs"`）。
    Tag(String),
    /// 多个具体物品之一。
    OneOf(Vec<String>),
}

impl Ingredient {
    /// 创建匹配指定物品 ID 的原料。
    ///
    /// 若未提供命名空间（如 `"diamond"`），则自动补上 `"minecraft:"`。
    #[must_use]
    pub fn item(id: impl AsRef<str>) -> Self {
        let s = id.as_ref();
        let normalized = if s.contains(':') {
            s.to_string()
        } else {
            format!("minecraft:{s}")
        };
        Self::Item(normalized)
    }

    /// 创建匹配一组物品标签的原料。
    ///
    /// 若未提供命名空间（如 `"logs"`），则自动补上 `"minecraft:"`。
    /// 前导 `#` 字符会被自动去除。
    #[must_use]
    pub fn tag(tag: impl AsRef<str>) -> Self {
        let mut s = tag.as_ref();
        if let Some(stripped) = s.strip_prefix('#') {
            s = stripped;
        }
        let normalized = if s.contains(':') {
            s.to_string()
        } else {
            format!("minecraft:{s}")
        };
        Self::Tag(normalized)
    }

    /// 创建匹配所给物品 ID 中任意一个的原料。
    pub fn one_of<I, S>(items: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        let list: Vec<String> = items
            .into_iter()
            .map(|s| {
                let s = s.as_ref();
                if s.contains(':') {
                    s.to_string()
                } else {
                    format!("minecraft:{s}")
                }
            })
            .collect();
        Self::OneOf(list)
    }

    /// [`Ingredient::one_of`] 的别名。
    pub fn items<I, S>(items: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        Self::one_of(items)
    }
}

impl From<&str> for Ingredient {
    fn from(s: &str) -> Self {
        if s.starts_with('#') {
            Self::tag(&s[1..])
        } else {
            Self::item(s)
        }
    }
}

impl From<String> for Ingredient {
    fn from(s: String) -> Self {
        s.as_str().into()
    }
}

impl From<&String> for Ingredient {
    fn from(s: &String) -> Self {
        s.as_str().into()
    }
}

impl<const N: usize> From<[&str; N]> for Ingredient {
    fn from(arr: [&str; N]) -> Self {
        Self::one_of(arr)
    }
}

impl<const N: usize> From<[String; N]> for Ingredient {
    fn from(arr: [String; N]) -> Self {
        Self::one_of(arr)
    }
}

impl From<Vec<String>> for Ingredient {
    fn from(vec: Vec<String>) -> Self {
        Self::one_of(vec)
    }
}

impl From<Vec<&str>> for Ingredient {
    fn from(vec: Vec<&str>) -> Self {
        Self::one_of(vec)
    }
}

impl From<&[String]> for Ingredient {
    fn from(slice: &[String]) -> Self {
        Self::one_of(slice)
    }
}

impl From<&[&str]> for Ingredient {
    fn from(slice: &[&str]) -> Self {
        Self::one_of(slice)
    }
}

impl From<Ingredient> for WitIngredient {
    fn from(ing: Ingredient) -> Self {
        match ing {
            Ingredient::Item(id) => Self::Item(id),
            Ingredient::Tag(tag) => Self::Tag(tag),
            Ingredient::OneOf(items) => Self::OneOf(items),
        }
    }
}

impl From<WitIngredient> for Ingredient {
    fn from(ing: WitIngredient) -> Self {
        match ing {
            WitIngredient::Item(id) => Self::Item(id),
            WitIngredient::Tag(tag) => Self::Tag(tag),
            WitIngredient::OneOf(items) => Self::OneOf(items),
        }
    }
}

/// 将物品或药水标识符归一化为完整命名空间的注册表键。
///
/// 若未提供命名空间（如 `"potion"`），则自动补上 `"minecraft:"`。
/// 空字符串原样返回，以便校验能标记它们。
fn normalize_id(id: impl AsRef<str>) -> String {
    let s = id.as_ref();
    if s.is_empty() || s.contains(':') {
        s.to_string()
    } else {
        format!("minecraft:{s}")
    }
}

/// 构建或校验配方时可能发生的错误。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RecipeError {
    /// 配方标识符为空。
    EmptyId,
    /// pattern 为空。
    EmptyPattern,
    /// pattern 尺寸超出合成网格（最大 3x3）。
    PatternTooLarge {
        /// 图案的宽度。
        width: usize,
        /// pattern 的高度。
        height: usize,
    },
    /// pattern 中的各行长度不一致。
    InconsistentRowWidth {
        /// 基于首行得到的期望宽度。
        expected: usize,
        /// 出错行的宽度。
        found: usize,
    },
    /// pattern 中某个字符没有匹配的原料键。
    MissingKey(char),
    /// 无序配方未提供原料。
    NoIngredients,
    /// 无序配方中的材料过多（最多 9 个）。
    TooManyIngredients(usize),
    /// 烹饪配方未提供输入原料。
    MissingCookingInput,
    /// 酿造配方的必填字段为空。
    EmptyBrewingField(&'static str),
}

impl std::fmt::Display for RecipeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EmptyId => write!(f, "配方标识符不能为空"),
            Self::EmptyPattern => write!(f, "有序配方图案不能为空"),
            Self::PatternTooLarge { width, height } => {
                write!(f, "有序配方图案过大（{width}x{height}，最大 3x3）")
            }
            Self::InconsistentRowWidth { expected, found } => {
                write!(f, "有序配方图案行宽不一致（预期 {expected}，实际 {found}）")
            }
            Self::MissingKey(ch) => write!(f, "图案包含没有对应材料键的字符 '{ch}'"),
            Self::NoIngredients => write!(f, "无序配方至少需要一个材料"),
            Self::TooManyIngredients(count) => {
                write!(f, "无序配方材料过多（{count}，最大 9）")
            }
            Self::MissingCookingInput => write!(f, "烹饪配方需要一个输入材料"),
            Self::EmptyBrewingField(field) => {
                write!(f, "酿造配方字段 '{field}' 不能为空")
            }
        }
    }
}

impl std::error::Error for RecipeError {}

/// 用于构建并注册有序合成配方的构建器。
pub struct ShapedRecipeBuilder {
    id: String,
    pattern: Vec<String>,
    keys: HashMap<char, Ingredient>,
    output: ItemStack,
    group: Option<String>,
    category: Option<RecipeCategory>,
    show_notification: Option<bool>,
}

impl ShapedRecipeBuilder {
    /// 以唯一配方 ID 和输出物品堆创建新的有序配方构建器。
    #[must_use]
    pub fn new(id: impl Into<String>, output: ItemStack) -> Self {
        Self {
            id: id.into(),
            pattern: Vec::new(),
            keys: HashMap::new(),
            output,
            group: None,
            category: None,
            show_notification: None,
        }
    }

    /// 设置有序配方的完整图案。
    ///
    /// 迭代器中的每个字符串代表合成网格的一行（如 `["# #", " s ", "# #"]`）。
    #[must_use]
    pub fn pattern<I, S>(mut self, pattern: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        self.pattern = pattern
            .into_iter()
            .map(|s| s.as_ref().to_string())
            .collect();
        self
    }

    /// [`ShapedRecipeBuilder::pattern`] 的别名。
    #[must_use]
    pub fn shape<I, S>(self, pattern: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        self.pattern(pattern)
    }

    /// 向有序配方 pattern 添加一行。
    #[must_use]
    pub fn row(mut self, row: impl AsRef<str>) -> Self {
        self.pattern.push(row.as_ref().to_string());
        self
    }

    /// 为 pattern 中使用的字符符号定义原料。
    #[must_use]
    pub fn key(mut self, symbol: char, ingredient: impl Into<Ingredient>) -> Self {
        self.keys.insert(symbol, ingredient.into());
        self
    }

    /// 设置配方分组。
    #[must_use]
    pub fn group(mut self, group: impl Into<String>) -> Self {
        self.group = Some(group.into());
        self
    }

    /// 设置配方书中的配方类别。
    #[must_use]
    pub const fn category(mut self, category: RecipeCategory) -> Self {
        self.category = Some(category);
        self
    }

    /// 设置玩家解锁此配方时是否显示 toast 通知。
    #[must_use]
    pub const fn show_notification(mut self, show: bool) -> Self {
        self.show_notification = Some(show);
        self
    }

    /// 验证配方配置。
    ///
    /// # Errors
    /// 若 ID 或图案为空、各行不匹配，返回 [`RecipeError`]，
    /// 尺寸超过 3x3，或字符缺少配料键。
    pub fn validate(&self) -> Result<(), RecipeError> {
        if self.id.is_empty() {
            return Err(RecipeError::EmptyId);
        }
        if self.pattern.is_empty() {
            return Err(RecipeError::EmptyPattern);
        }
        let height = self.pattern.len();
        let width = self.pattern[0].chars().count();
        if height > 3 || width > 3 || width == 0 {
            return Err(RecipeError::PatternTooLarge { width, height });
        }
        for row in &self.pattern {
            let row_width = row.chars().count();
            if row_width != width {
                return Err(RecipeError::InconsistentRowWidth {
                    expected: width,
                    found: row_width,
                });
            }
            for ch in row.chars() {
                if ch != ' ' && !self.keys.contains_key(&ch) {
                    return Err(RecipeError::MissingKey(ch));
                }
            }
        }
        Ok(())
    }

    /// 校验后构建有序配方结构。
    ///
    /// # Errors
    /// 若校验失败，返回 [`RecipeError`]。
    pub fn build(self) -> Result<(String, ShapedRecipe), RecipeError> {
        self.validate()?;
        let key_list: Vec<(String, WitIngredient)> = self
            .keys
            .into_iter()
            .map(|(k, v)| (k.to_string(), v.into()))
            .collect();

        let recipe = ShapedRecipe {
            pattern: self.pattern,
            key: key_list,
            output: self.output,
            group: self.group,
            category: self.category,
            show_notification: self.show_notification,
        };
        Ok((self.id, recipe))
    }

    /// 直接向所提供的 [`RecipeManager`] 注册此有序配方。
    ///
    /// # Errors
    /// 若校验失败，返回 [`RecipeError`]。
    pub fn register(self, manager: &RecipeManager) -> Result<(), RecipeError> {
        let (id, recipe) = self.build()?;
        manager.register_shaped(&id, recipe);
        Ok(())
    }

    /// 向服务器注册此有序配方。
    ///
    /// # Errors
    /// 若校验失败，返回 [`RecipeError`]。
    pub fn register_to_server(self, server: &Server) -> Result<(), RecipeError> {
        let manager = server.get_recipe_manager();
        self.register(&manager)
    }

    /// 向插件上下文注册此有序配方。
    ///
    /// # Errors
    /// 若校验失败，返回 [`RecipeError`]。
    pub fn register_to_context(self, context: &Context) -> Result<(), RecipeError> {
        let manager = context.get_recipe_manager();
        self.register(&manager)
    }
}

/// 用于构建并注册无序合成配方的构建器。
pub struct ShapelessRecipeBuilder {
    id: String,
    ingredients: Vec<Ingredient>,
    output: ItemStack,
    group: Option<String>,
    category: Option<RecipeCategory>,
}

impl ShapelessRecipeBuilder {
    /// 以唯一配方 ID 和输出物品堆创建新的无序配方构建器。
    #[must_use]
    pub fn new(id: impl Into<String>, output: ItemStack) -> Self {
        Self {
            id: id.into(),
            ingredients: Vec::new(),
            output,
            group: None,
            category: None,
        }
    }

    /// 向配方添加一种原料。
    #[must_use]
    pub fn ingredient(mut self, ingredient: impl Into<Ingredient>) -> Self {
        self.ingredients.push(ingredient.into());
        self
    }

    /// [`ShapelessRecipeBuilder::ingredient`] 的别名。
    #[must_use]
    pub fn add_ingredient(self, ingredient: impl Into<Ingredient>) -> Self {
        self.ingredient(ingredient)
    }

    /// 向配方添加多份相同原料。
    #[must_use]
    pub fn ingredient_count(mut self, ingredient: impl Into<Ingredient>, count: usize) -> Self {
        let ing = ingredient.into();
        for _ in 0..count {
            self.ingredients.push(ing.clone());
        }
        self
    }

    /// 向配方添加多种原料。
    #[must_use]
    pub fn ingredients<I, T>(mut self, ingredients: I) -> Self
    where
        I: IntoIterator<Item = T>,
        T: Into<Ingredient>,
    {
        for ing in ingredients {
            self.ingredients.push(ing.into());
        }
        self
    }

    /// 设置配方分组。
    #[must_use]
    pub fn group(mut self, group: impl Into<String>) -> Self {
        self.group = Some(group.into());
        self
    }

    /// 设置配方书中的配方类别。
    #[must_use]
    pub const fn category(mut self, category: RecipeCategory) -> Self {
        self.category = Some(category);
        self
    }

    /// 验证配方配置。
    ///
    /// # Errors
    /// 若 ID 为空、原料列表为空或超过 9 个物品，返回 [`RecipeError`]。
    pub fn validate(&self) -> Result<(), RecipeError> {
        if self.id.is_empty() {
            return Err(RecipeError::EmptyId);
        }
        if self.ingredients.is_empty() {
            return Err(RecipeError::NoIngredients);
        }
        if self.ingredients.len() > 9 {
            return Err(RecipeError::TooManyIngredients(self.ingredients.len()));
        }
        Ok(())
    }

    /// 校验后构建无序配方结构。
    ///
    /// # Errors
    /// 若校验失败，返回 [`RecipeError`]。
    pub fn build(self) -> Result<(String, ShapelessRecipe), RecipeError> {
        self.validate()?;
        let ingredients: Vec<WitIngredient> =
            self.ingredients.into_iter().map(Into::into).collect();

        let recipe = ShapelessRecipe {
            ingredients,
            output: self.output,
            group: self.group,
            category: self.category,
        };
        Ok((self.id, recipe))
    }

    /// 直接向所提供的 [`RecipeManager`] 注册此无序配方。
    ///
    /// # Errors
    /// 若校验失败，返回 [`RecipeError`]。
    pub fn register(self, manager: &RecipeManager) -> Result<(), RecipeError> {
        let (id, recipe) = self.build()?;
        manager.register_shapeless(&id, recipe);
        Ok(())
    }

    /// 向服务器注册此无序配方。
    ///
    /// # Errors
    /// 若校验失败，返回 [`RecipeError`]。
    pub fn register_to_server(self, server: &Server) -> Result<(), RecipeError> {
        let manager = server.get_recipe_manager();
        self.register(&manager)
    }

    /// 向插件上下文注册此无序配方。
    ///
    /// # Errors
    /// 若校验失败，返回 [`RecipeError`]。
    pub fn register_to_context(self, context: &Context) -> Result<(), RecipeError> {
        let manager = context.get_recipe_manager();
        self.register(&manager)
    }
}

/// 用于构建并注册烹饪配方（熔炉熔炼、高炉、烟熏炉、营火）的构建器。
pub struct CookingRecipeBuilder {
    id: String,
    cooking_type: CookingType,
    ingredient: Option<Ingredient>,
    output: ItemStack,
    cooking_time: u32,
    experience: f32,
    group: Option<String>,
    category: Option<RecipeCategory>,
}

impl CookingRecipeBuilder {
    /// 创建新的通用烹饪配方构建器。
    #[must_use]
    pub fn new(
        id: impl Into<String>,
        cooking_type: CookingType,
        ingredient: impl Into<Ingredient>,
        output: ItemStack,
    ) -> Self {
        let cooking_time = match cooking_type {
            CookingType::Smelting => 200,
            CookingType::Blasting | CookingType::Smoking => 100,
            CookingType::Campfire => 600,
        };
        Self {
            id: id.into(),
            cooking_type,
            ingredient: Some(ingredient.into()),
            output,
            cooking_time,
            experience: 0.0,
            group: None,
            category: None,
        }
    }

    /// 创建熔炉熔炼（smelting）配方构建器。
    ///
    /// 默认烹饪时间为 200 刻（10 秒）。
    #[must_use]
    pub fn smelting(
        id: impl Into<String>,
        ingredient: impl Into<Ingredient>,
        output: ItemStack,
    ) -> Self {
        Self::new(id, CookingType::Smelting, ingredient, output)
    }

    /// 创建高炉（blasting）配方构建器。
    ///
    /// 默认烹饪时间为 100 刻（5 秒）。
    #[must_use]
    pub fn blasting(
        id: impl Into<String>,
        ingredient: impl Into<Ingredient>,
        output: ItemStack,
    ) -> Self {
        Self::new(id, CookingType::Blasting, ingredient, output)
    }

    /// 创建烟熏炉（smoking）配方构建器。
    ///
    /// 默认烹饪时间为 100 刻（5 秒）。
    #[must_use]
    pub fn smoking(
        id: impl Into<String>,
        ingredient: impl Into<Ingredient>,
        output: ItemStack,
    ) -> Self {
        Self::new(id, CookingType::Smoking, ingredient, output)
    }

    /// 创建营火烹饪配方构建器。
    ///
    /// 默认烹饪时间为 600 刻（30 秒）。
    #[must_use]
    pub fn campfire(
        id: impl Into<String>,
        ingredient: impl Into<Ingredient>,
        output: ItemStack,
    ) -> Self {
        Self::new(id, CookingType::Campfire, ingredient, output)
    }

    /// 设置烹饪设备类型。
    #[must_use]
    pub const fn cooking_type(mut self, cooking_type: CookingType) -> Self {
        self.cooking_type = cooking_type;
        self
    }

    /// 设置输入原料。
    #[must_use]
    pub fn ingredient(mut self, ingredient: impl Into<Ingredient>) -> Self {
        self.ingredient = Some(ingredient.into());
        self
    }

    /// [`CookingRecipeBuilder::ingredient`] 的别名。
    #[must_use]
    pub fn input(self, ingredient: impl Into<Ingredient>) -> Self {
        self.ingredient(ingredient)
    }

    /// 以刻为单位设置烹饪时间（20 刻 = 1 秒）。
    #[must_use]
    pub const fn cooking_time(mut self, ticks: u32) -> Self {
        self.cooking_time = ticks;
        self
    }

    /// 设置取出烹饪完成物品时奖励的经验点数。
    #[must_use]
    pub const fn experience(mut self, exp: f32) -> Self {
        self.experience = exp;
        self
    }

    /// 设置配方分组。
    #[must_use]
    pub fn group(mut self, group: impl Into<String>) -> Self {
        self.group = Some(group.into());
        self
    }

    /// 设置配方书中的配方类别。
    #[must_use]
    pub const fn category(mut self, category: RecipeCategory) -> Self {
        self.category = Some(category);
        self
    }

    /// 验证配方配置。
    ///
    /// # Errors
    /// 若 ID 为空或未设置输入原料，返回 [`RecipeError`]。
    pub fn validate(&self) -> Result<(), RecipeError> {
        if self.id.is_empty() {
            return Err(RecipeError::EmptyId);
        }
        if self.ingredient.is_none() {
            return Err(RecipeError::MissingCookingInput);
        }
        Ok(())
    }

    /// 校验后构建烹饪配方结构。
    ///
    /// # Errors
    /// 若校验失败，返回 [`RecipeError`]。
    pub fn build(self) -> Result<(String, CookingType, CookingRecipe), RecipeError> {
        self.validate()?;
        let ingredient = self
            .ingredient
            .ok_or(RecipeError::MissingCookingInput)?
            .into();
        let recipe = CookingRecipe {
            ingredient,
            output: self.output,
            experience: self.experience,
            cooking_time: self.cooking_time,
            group: self.group,
            category: self.category,
        };
        Ok((self.id, self.cooking_type, recipe))
    }

    /// 直接向所提供的 [`RecipeManager`] 注册此烹饪配方。
    ///
    /// # Errors
    /// 若校验失败，返回 [`RecipeError`]。
    pub fn register(self, manager: &RecipeManager) -> Result<(), RecipeError> {
        let (id, cooking_type, recipe) = self.build()?;
        manager.register_cooking(&id, cooking_type, recipe);
        Ok(())
    }

    /// 向服务器注册此烹饪配方。
    ///
    /// # Errors
    /// 若校验失败，返回 [`RecipeError`]。
    pub fn register_to_server(self, server: &Server) -> Result<(), RecipeError> {
        let manager = server.get_recipe_manager();
        self.register(&manager)
    }

    /// 向插件上下文注册此烹饪配方。
    ///
    /// # Errors
    /// 若校验失败，返回 [`RecipeError`]。
    pub fn register_to_context(self, context: &Context) -> Result<(), RecipeError> {
        let manager = context.get_recipe_manager();
        self.register(&manager)
    }
}

/// 用于构建并注册切石配方的构建器。
pub struct StonecuttingRecipeBuilder {
    id: String,
    ingredient: Ingredient,
    output: ItemStack,
}

impl StonecuttingRecipeBuilder {
    /// 创建新的切石配方构建器，参数为唯一配方 ID、输入原料、
    /// 以及输出物品堆。
    #[must_use]
    pub fn new(
        id: impl Into<String>,
        ingredient: impl Into<Ingredient>,
        output: ItemStack,
    ) -> Self {
        Self {
            id: id.into(),
            ingredient: ingredient.into(),
            output,
        }
    }

    /// 设置输入原料。
    #[must_use]
    pub fn ingredient(mut self, ingredient: impl Into<Ingredient>) -> Self {
        self.ingredient = ingredient.into();
        self
    }

    /// [`StonecuttingRecipeBuilder::ingredient`] 的别名。
    #[must_use]
    pub fn input(self, ingredient: impl Into<Ingredient>) -> Self {
        self.ingredient(ingredient)
    }

    /// 验证配方配置。
    ///
    /// # Errors
    /// 若 ID 为空，返回 [`RecipeError`]。
    pub fn validate(&self) -> Result<(), RecipeError> {
        if self.id.is_empty() {
            return Err(RecipeError::EmptyId);
        }
        Ok(())
    }

    /// 校验后构建切石配方结构。
    ///
    /// # Errors
    /// 若校验失败，返回 [`RecipeError`]。
    pub fn build(self) -> Result<(String, StonecuttingRecipe), RecipeError> {
        self.validate()?;
        let recipe = StonecuttingRecipe {
            ingredient: self.ingredient.into(),
            output: self.output,
        };
        Ok((self.id, recipe))
    }

    /// 直接向所提供的 [`RecipeManager`] 注册此切石配方。
    ///
    /// # Errors
    /// 若校验失败，返回 [`RecipeError`]。
    pub fn register(self, manager: &RecipeManager) -> Result<(), RecipeError> {
        let (id, recipe) = self.build()?;
        manager.register_stonecutting(&id, recipe);
        Ok(())
    }

    /// 向服务器注册此切石配方。
    ///
    /// # Errors
    /// 若校验失败，返回 [`RecipeError`]。
    pub fn register_to_server(self, server: &Server) -> Result<(), RecipeError> {
        let manager = server.get_recipe_manager();
        self.register(&manager)
    }

    /// 向插件上下文注册此切石配方。
    ///
    /// # Errors
    /// 若校验失败，返回 [`RecipeError`]。
    pub fn register_to_context(self, context: &Context) -> Result<(), RecipeError> {
        let manager = context.get_recipe_manager();
        self.register(&manager)
    }
}

/// 用于构建并注册锻造升级配方（如下界合金升级）的构建器。
pub struct SmithingTransformRecipeBuilder {
    id: String,
    template: Ingredient,
    base: Ingredient,
    addition: Ingredient,
    output: ItemStack,
    copy_components: bool,
}

impl SmithingTransformRecipeBuilder {
    /// 创建新的锻造升级配方构建器，参数为唯一配方 ID、模板、
    /// 基底和附加成分，以及输出物品堆。
    #[must_use]
    pub fn new(
        id: impl Into<String>,
        template: impl Into<Ingredient>,
        base: impl Into<Ingredient>,
        addition: impl Into<Ingredient>,
        output: ItemStack,
    ) -> Self {
        Self {
            id: id.into(),
            template: template.into(),
            base: base.into(),
            addition: addition.into(),
            output,
            copy_components: false,
        }
    }

    /// 设置模板物品（例如下界合金升级锻造模板）。
    #[must_use]
    pub fn template(mut self, template: impl Into<Ingredient>) -> Self {
        self.template = template.into();
        self
    }

    /// 设置要转化的基础物品。
    #[must_use]
    pub fn base(mut self, base: impl Into<Ingredient>) -> Self {
        self.base = base.into();
        self
    }

    /// 设置该转化所消耗的附加物品。
    #[must_use]
    pub fn addition(mut self, addition: impl Into<Ingredient>) -> Self {
        self.addition = addition.into();
        self
    }

    /// 设置是否将数据组件从基础物品复制到结果物品（默认 `false`）。
    #[must_use]
    pub const fn copy_components(mut self, copy: bool) -> Self {
        self.copy_components = copy;
        self
    }

    /// 验证配方配置。
    ///
    /// # Errors
    /// 若 ID 为空，返回 [`RecipeError`]。
    pub fn validate(&self) -> Result<(), RecipeError> {
        if self.id.is_empty() {
            return Err(RecipeError::EmptyId);
        }
        Ok(())
    }

    /// 校验后构建锻造升级配方结构。
    ///
    /// # Errors
    /// 若校验失败，返回 [`RecipeError`]。
    pub fn build(self) -> Result<(String, SmithingTransformRecipe), RecipeError> {
        self.validate()?;
        let recipe = SmithingTransformRecipe {
            template: self.template.into(),
            base: self.base.into(),
            addition: self.addition.into(),
            output: self.output,
            copy_components: self.copy_components,
        };
        Ok((self.id, recipe))
    }

    /// 直接向所提供的 [`RecipeManager`] 注册此锻造转换配方。
    ///
    /// # Errors
    /// 若校验失败，返回 [`RecipeError`]。
    pub fn register(self, manager: &RecipeManager) -> Result<(), RecipeError> {
        let (id, recipe) = self.build()?;
        manager.register_smithing_transform(&id, recipe);
        Ok(())
    }

    /// 向服务器注册此锻造转换配方。
    ///
    /// # Errors
    /// 若校验失败，返回 [`RecipeError`]。
    pub fn register_to_server(self, server: &Server) -> Result<(), RecipeError> {
        let manager = server.get_recipe_manager();
        self.register(&manager)
    }

    /// 向插件上下文注册此锻造转换配方。
    ///
    /// # Errors
    /// 若校验失败，返回 [`RecipeError`]。
    pub fn register_to_context(self, context: &Context) -> Result<(), RecipeError> {
        let manager = context.get_recipe_manager();
        self.register(&manager)
    }
}

/// 用于构建并注册锻造纹饰配方（装饰性盔甲纹饰）的构建器。
pub struct SmithingTrimRecipeBuilder {
    id: String,
    template: Ingredient,
    base: Ingredient,
    addition: Ingredient,
}

impl SmithingTrimRecipeBuilder {
    /// 创建新的锻造纹饰配方构建器，参数为唯一配方 ID、模板、
    /// 基底以及附加（纹饰材料）成分。
    #[must_use]
    pub fn new(
        id: impl Into<String>,
        template: impl Into<Ingredient>,
        base: impl Into<Ingredient>,
        addition: impl Into<Ingredient>,
    ) -> Self {
        Self {
            id: id.into(),
            template: template.into(),
            base: base.into(),
            addition: addition.into(),
        }
    }

    /// 设置纹饰模板物品。
    #[must_use]
    pub fn template(mut self, template: impl Into<Ingredient>) -> Self {
        self.template = template.into();
        self
    }

    /// 设置要进行纹饰的基础物品（通常是盔甲）。
    #[must_use]
    pub fn base(mut self, base: impl Into<Ingredient>) -> Self {
        self.base = base.into();
        self
    }

    /// 设置纹饰材料物品（例如 `"minecraft:diamond"`）。
    #[must_use]
    pub fn addition(mut self, addition: impl Into<Ingredient>) -> Self {
        self.addition = addition.into();
        self
    }

    /// 验证配方配置。
    ///
    /// # Errors
    /// 若 ID 为空，返回 [`RecipeError`]。
    pub fn validate(&self) -> Result<(), RecipeError> {
        if self.id.is_empty() {
            return Err(RecipeError::EmptyId);
        }
        Ok(())
    }

    /// 校验后构建锻造纹饰配方结构。
    ///
    /// # Errors
    /// 若校验失败，返回 [`RecipeError`]。
    pub fn build(self) -> Result<(String, SmithingTrimRecipe), RecipeError> {
        self.validate()?;
        let recipe = SmithingTrimRecipe {
            template: self.template.into(),
            base: self.base.into(),
            addition: self.addition.into(),
        };
        Ok((self.id, recipe))
    }

    /// 直接向所提供的 [`RecipeManager`] 注册此锻造纹饰配方。
    ///
    /// # Errors
    /// 若校验失败，返回 [`RecipeError`]。
    pub fn register(self, manager: &RecipeManager) -> Result<(), RecipeError> {
        let (id, recipe) = self.build()?;
        manager.register_smithing_trim(&id, &recipe);
        Ok(())
    }

    /// 向服务器注册此锻造纹饰配方。
    ///
    /// # Errors
    /// 若校验失败，返回 [`RecipeError`]。
    pub fn register_to_server(self, server: &Server) -> Result<(), RecipeError> {
        let manager = server.get_recipe_manager();
        self.register(&manager)
    }

    /// 向插件上下文注册此锻造纹饰配方。
    ///
    /// # Errors
    /// 若校验失败，返回 [`RecipeError`]。
    pub fn register_to_context(self, context: &Context) -> Result<(), RecipeError> {
        let manager = context.get_recipe_manager();
        self.register(&manager)
    }
}

/// 用于构建并注册酿造配方的构建器。
pub struct BrewingRecipeBuilder {
    id: String,
    input_item: String,
    input_potion: Option<String>,
    reagent: String,
    output_item: String,
    output_potion: Option<String>,
}

impl BrewingRecipeBuilder {
    /// 创建新的酿造配方构建器，参数为唯一配方 ID、容器物品 id、
    /// 试剂物品 id，以及生成的容器物品 id。
    ///
    /// 无命名空间的物品 id 会归一化到 `minecraft:` 命名空间。
    #[must_use]
    pub fn new(
        id: impl Into<String>,
        input_item: impl Into<String>,
        reagent: impl Into<String>,
        output_item: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            input_item: normalize_id(input_item.into()),
            input_potion: None,
            reagent: normalize_id(reagent.into()),
            output_item: normalize_id(output_item.into()),
            output_potion: None,
        }
    }

    /// 设置容器物品 ID（例如 `"minecraft:potion"`）。
    #[must_use]
    pub fn input_item(mut self, input_item: impl Into<String>) -> Self {
        self.input_item = normalize_id(input_item.into());
        self
    }

    /// 设置所需的输入药水类型 ID（例如 `"minecraft:water"`）。
    #[must_use]
    pub fn input_potion(mut self, potion: impl Into<String>) -> Self {
        self.input_potion = Some(normalize_id(potion.into()));
        self
    }

    /// 设置试剂物品 ID（例如 `"minecraft:blaze_powder"`）。
    #[must_use]
    pub fn reagent(mut self, reagent: impl Into<String>) -> Self {
        self.reagent = normalize_id(reagent.into());
        self
    }

    /// 设置生成的容器物品 ID。
    #[must_use]
    pub fn output_item(mut self, output_item: impl Into<String>) -> Self {
        self.output_item = normalize_id(output_item.into());
        self
    }

    /// 设置生成的药水类型 ID。
    #[must_use]
    pub fn output_potion(mut self, potion: impl Into<String>) -> Self {
        self.output_potion = Some(normalize_id(potion.into()));
        self
    }

    /// 验证配方配置。
    ///
    /// # Errors
    /// 若 ID 为空或必填字段为空，返回 [`RecipeError`]。
    pub fn validate(&self) -> Result<(), RecipeError> {
        if self.id.is_empty() {
            return Err(RecipeError::EmptyId);
        }
        if self.input_item.is_empty() {
            return Err(RecipeError::EmptyBrewingField("input_item"));
        }
        if self.reagent.is_empty() {
            return Err(RecipeError::EmptyBrewingField("reagent"));
        }
        if self.output_item.is_empty() {
            return Err(RecipeError::EmptyBrewingField("output_item"));
        }
        Ok(())
    }

    /// 校验后构建酿造配方结构。
    ///
    /// # Errors
    /// 若校验失败，返回 [`RecipeError`]。
    pub fn build(self) -> Result<(String, BrewingRecipe), RecipeError> {
        self.validate()?;
        let recipe = BrewingRecipe {
            input_item: self.input_item,
            input_potion: self.input_potion,
            reagent: self.reagent,
            output_item: self.output_item,
            output_potion: self.output_potion,
        };
        Ok((self.id, recipe))
    }

    /// 直接向所提供的 [`RecipeManager`] 注册此酿造配方。
    ///
    /// # Errors
    /// 若校验失败，返回 [`RecipeError`]。
    pub fn register(self, manager: &RecipeManager) -> Result<(), RecipeError> {
        let (id, recipe) = self.build()?;
        manager.register_brewing(&id, &recipe);
        Ok(())
    }

    /// 向服务器注册此酿造配方。
    ///
    /// # Errors
    /// 若校验失败，返回 [`RecipeError`]。
    pub fn register_to_server(self, server: &Server) -> Result<(), RecipeError> {
        let manager = server.get_recipe_manager();
        self.register(&manager)
    }

    /// 向插件上下文注册此酿造配方。
    ///
    /// # Errors
    /// 若校验失败，返回 [`RecipeError`]。
    pub fn register_to_context(self, context: &Context) -> Result<(), RecipeError> {
        let manager = context.get_recipe_manager();
        self.register(&manager)
    }
}

/// 可通过 [`RecipeManager`] 注册的配方类型 trait。
pub trait RegistrableRecipe {
    /// 向所提供的配方管理器注册此配方。
    ///
    /// # Errors
    /// 若校验或注册失败，返回 [`RecipeError`]。
    fn register(self, manager: &RecipeManager) -> Result<(), RecipeError>;
}

impl RegistrableRecipe for ShapedRecipeBuilder {
    fn register(self, manager: &RecipeManager) -> Result<(), RecipeError> {
        let (id, recipe) = self.build()?;
        manager.register_shaped(&id, recipe);
        Ok(())
    }
}

impl RegistrableRecipe for ShapelessRecipeBuilder {
    fn register(self, manager: &RecipeManager) -> Result<(), RecipeError> {
        let (id, recipe) = self.build()?;
        manager.register_shapeless(&id, recipe);
        Ok(())
    }
}

impl RegistrableRecipe for CookingRecipeBuilder {
    fn register(self, manager: &RecipeManager) -> Result<(), RecipeError> {
        let (id, cooking_type, recipe) = self.build()?;
        manager.register_cooking(&id, cooking_type, recipe);
        Ok(())
    }
}

impl RegistrableRecipe for StonecuttingRecipeBuilder {
    fn register(self, manager: &RecipeManager) -> Result<(), RecipeError> {
        let (id, recipe) = self.build()?;
        manager.register_stonecutting(&id, recipe);
        Ok(())
    }
}

impl RegistrableRecipe for SmithingTransformRecipeBuilder {
    fn register(self, manager: &RecipeManager) -> Result<(), RecipeError> {
        let (id, recipe) = self.build()?;
        manager.register_smithing_transform(&id, recipe);
        Ok(())
    }
}

impl RegistrableRecipe for SmithingTrimRecipeBuilder {
    fn register(self, manager: &RecipeManager) -> Result<(), RecipeError> {
        let (id, recipe) = self.build()?;
        manager.register_smithing_trim(&id, &recipe);
        Ok(())
    }
}

impl RegistrableRecipe for BrewingRecipeBuilder {
    fn register(self, manager: &RecipeManager) -> Result<(), RecipeError> {
        let (id, recipe) = self.build()?;
        manager.register_brewing(&id, &recipe);
        Ok(())
    }
}

impl Context {
    /// 返回全局配方管理器，用于注册自定义配方。
    #[must_use]
    pub fn get_recipe_manager(&self) -> RecipeManager {
        self.get_server().get_recipe_manager()
    }

    /// 向服务器注册一个自定义配方。
    ///
    /// 接受任意配方构建器（[`ShapedRecipeBuilder`]、[`ShapelessRecipeBuilder`]、
    /// [`CookingRecipeBuilder`]、[`StonecuttingRecipeBuilder`]、
    /// [`SmithingTransformRecipeBuilder`]、[`SmithingTrimRecipeBuilder`]、
    /// [`BrewingRecipeBuilder`]）。
    ///
    /// # Errors
    /// 若校验失败，返回 [`RecipeError`]。
    pub fn register_recipe(&self, recipe: impl RegistrableRecipe) -> Result<(), RecipeError> {
        self.get_recipe_manager().register(recipe)
    }

    /// 注册一个有序合成配方。
    ///
    /// # Errors
    /// 若校验失败，返回 [`RecipeError`]。
    pub fn register_shaped_recipe(&self, builder: ShapedRecipeBuilder) -> Result<(), RecipeError> {
        self.register_recipe(builder)
    }

    /// 注册一个无序合成配方。
    ///
    /// # Errors
    /// 若校验失败，返回 [`RecipeError`]。
    pub fn register_shapeless_recipe(
        &self,
        builder: ShapelessRecipeBuilder,
    ) -> Result<(), RecipeError> {
        self.register_recipe(builder)
    }

    /// 注册一个烹饪配方。
    ///
    /// # Errors
    /// 若校验失败，返回 [`RecipeError`]。
    pub fn register_cooking_recipe(
        &self,
        builder: CookingRecipeBuilder,
    ) -> Result<(), RecipeError> {
        self.register_recipe(builder)
    }

    /// 注册一个切石配方。
    ///
    /// # Errors
    /// 若校验失败，返回 [`RecipeError`]。
    pub fn register_stonecutting_recipe(
        &self,
        builder: StonecuttingRecipeBuilder,
    ) -> Result<(), RecipeError> {
        self.register_recipe(builder)
    }

    /// 注册一个锻造转换配方。
    ///
    /// # Errors
    /// 若校验失败，返回 [`RecipeError`]。
    pub fn register_smithing_transform_recipe(
        &self,
        builder: SmithingTransformRecipeBuilder,
    ) -> Result<(), RecipeError> {
        self.register_recipe(builder)
    }

    /// 注册一个锻造纹饰配方。
    ///
    /// # Errors
    /// 若校验失败，返回 [`RecipeError`]。
    pub fn register_smithing_trim_recipe(
        &self,
        builder: SmithingTrimRecipeBuilder,
    ) -> Result<(), RecipeError> {
        self.register_recipe(builder)
    }

    /// 注册酿造配方。
    ///
    /// # Errors
    /// 若校验失败，返回 [`RecipeError`]。
    pub fn register_brewing_recipe(
        &self,
        builder: BrewingRecipeBuilder,
    ) -> Result<(), RecipeError> {
        self.register_recipe(builder)
    }
}

impl Server {
    /// 向服务器注册一个自定义配方。
    ///
    /// 接受任意配方构建器（[`ShapedRecipeBuilder`]、[`ShapelessRecipeBuilder`]、
    /// [`CookingRecipeBuilder`]、[`StonecuttingRecipeBuilder`]、
    /// [`SmithingTransformRecipeBuilder`]、[`SmithingTrimRecipeBuilder`]、
    /// [`BrewingRecipeBuilder`]）。
    ///
    /// # Errors
    /// 若校验失败，返回 [`RecipeError`]。
    pub fn register_recipe(&self, recipe: impl RegistrableRecipe) -> Result<(), RecipeError> {
        self.get_recipe_manager().register(recipe)
    }

    /// 注册一个有序合成配方。
    ///
    /// # Errors
    /// 若校验失败，返回 [`RecipeError`]。
    pub fn register_shaped_recipe(&self, builder: ShapedRecipeBuilder) -> Result<(), RecipeError> {
        self.register_recipe(builder)
    }

    /// 注册一个无序合成配方。
    ///
    /// # Errors
    /// 若校验失败，返回 [`RecipeError`]。
    pub fn register_shapeless_recipe(
        &self,
        builder: ShapelessRecipeBuilder,
    ) -> Result<(), RecipeError> {
        self.register_recipe(builder)
    }

    /// 注册一个烹饪配方。
    ///
    /// # Errors
    /// 若校验失败，返回 [`RecipeError`]。
    pub fn register_cooking_recipe(
        &self,
        builder: CookingRecipeBuilder,
    ) -> Result<(), RecipeError> {
        self.register_recipe(builder)
    }

    /// 注册一个切石配方。
    ///
    /// # Errors
    /// 若校验失败，返回 [`RecipeError`]。
    pub fn register_stonecutting_recipe(
        &self,
        builder: StonecuttingRecipeBuilder,
    ) -> Result<(), RecipeError> {
        self.register_recipe(builder)
    }

    /// 注册一个锻造转换配方。
    ///
    /// # Errors
    /// 若校验失败，返回 [`RecipeError`]。
    pub fn register_smithing_transform_recipe(
        &self,
        builder: SmithingTransformRecipeBuilder,
    ) -> Result<(), RecipeError> {
        self.register_recipe(builder)
    }

    /// 注册一个锻造纹饰配方。
    ///
    /// # Errors
    /// 若校验失败，返回 [`RecipeError`]。
    pub fn register_smithing_trim_recipe(
        &self,
        builder: SmithingTrimRecipeBuilder,
    ) -> Result<(), RecipeError> {
        self.register_recipe(builder)
    }

    /// 注册酿造配方。
    ///
    /// # Errors
    /// 若校验失败，返回 [`RecipeError`]。
    pub fn register_brewing_recipe(
        &self,
        builder: BrewingRecipeBuilder,
    ) -> Result<(), RecipeError> {
        self.register_recipe(builder)
    }
}

impl RecipeManager {
    /// 向服务器注册一个自定义配方。
    ///
    /// 接受任意配方构建器（[`ShapedRecipeBuilder`]、[`ShapelessRecipeBuilder`]、
    /// [`CookingRecipeBuilder`]、[`StonecuttingRecipeBuilder`]、
    /// [`SmithingTransformRecipeBuilder`]、[`SmithingTrimRecipeBuilder`]、
    /// [`BrewingRecipeBuilder`]）。
    ///
    /// # Examples
    /// ```rust,ignore
    /// manager.register(
    ///     ShapedRecipeBuilder::new("my:sword", output)
    ///         .pattern([" D ", " D ", " S "])
    ///         .key('D', "diamond")
    ///         .key('S', "stick")
    /// )?;
    /// ```
    ///
    /// # Errors
    /// 若配方校验失败，返回 [`RecipeError`]。
    pub fn register(&self, recipe: impl RegistrableRecipe) -> Result<(), RecipeError> {
        recipe.register(self)
    }

    /// 从构建器注册一个有序合成配方。
    ///
    /// # Errors
    /// 若校验失败，返回 [`RecipeError`]。
    pub fn register_shaped_recipe(&self, builder: ShapedRecipeBuilder) -> Result<(), RecipeError> {
        self.register(builder)
    }

    /// 从构建器注册一个无序合成配方。
    ///
    /// # Errors
    /// 若校验失败，返回 [`RecipeError`]。
    pub fn register_shapeless_recipe(
        &self,
        builder: ShapelessRecipeBuilder,
    ) -> Result<(), RecipeError> {
        self.register(builder)
    }

    /// 从构建器注册一个烹饪配方。
    ///
    /// # Errors
    /// 若校验失败，返回 [`RecipeError`]。
    pub fn register_cooking_recipe(
        &self,
        builder: CookingRecipeBuilder,
    ) -> Result<(), RecipeError> {
        self.register(builder)
    }

    /// 从构建器注册一个切石配方。
    ///
    /// # Errors
    /// 若校验失败，返回 [`RecipeError`]。
    pub fn register_stonecutting_recipe(
        &self,
        builder: StonecuttingRecipeBuilder,
    ) -> Result<(), RecipeError> {
        self.register(builder)
    }

    /// 从构建器注册一个锻造转换配方。
    ///
    /// # Errors
    /// 若校验失败，返回 [`RecipeError`]。
    pub fn register_smithing_transform_recipe(
        &self,
        builder: SmithingTransformRecipeBuilder,
    ) -> Result<(), RecipeError> {
        self.register(builder)
    }

    /// 从构建器注册一个锻造纹饰配方。
    ///
    /// # Errors
    /// 若校验失败，返回 [`RecipeError`]。
    pub fn register_smithing_trim_recipe(
        &self,
        builder: SmithingTrimRecipeBuilder,
    ) -> Result<(), RecipeError> {
        self.register(builder)
    }

    /// 从构建器注册酿造配方。
    ///
    /// # Errors
    /// 若校验失败，返回 [`RecipeError`]。
    pub fn register_brewing_recipe(
        &self,
        builder: BrewingRecipeBuilder,
    ) -> Result<(), RecipeError> {
        self.register(builder)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ingredient_normalization() {
        assert_eq!(
            Ingredient::from("diamond"),
            Ingredient::Item("minecraft:diamond".into())
        );
        assert_eq!(
            Ingredient::from("custom:ruby"),
            Ingredient::Item("custom:ruby".into())
        );
        assert_eq!(
            Ingredient::from("#logs"),
            Ingredient::Tag("minecraft:logs".into())
        );
        assert_eq!(
            Ingredient::from("#minecraft:planks"),
            Ingredient::Tag("minecraft:planks".into())
        );
        assert_eq!(
            Ingredient::from(["coal", "charcoal"]),
            Ingredient::OneOf(vec!["minecraft:coal".into(), "minecraft:charcoal".into()])
        );
    }

    #[test]
    fn shaped_recipe_validation_errors() {
        let dummy_output = unsafe { std::mem::zeroed() };
        let builder = ShapedRecipeBuilder::new("test:recipe", dummy_output);
        assert_eq!(builder.validate(), Err(RecipeError::EmptyPattern));
        std::mem::forget(builder);

        let dummy_output = unsafe { std::mem::zeroed() };
        let builder = ShapedRecipeBuilder::new("", dummy_output).pattern(["X"]);
        assert_eq!(builder.validate(), Err(RecipeError::EmptyId));
        std::mem::forget(builder);

        let dummy_output = unsafe { std::mem::zeroed() };
        let builder = ShapedRecipeBuilder::new("test:recipe", dummy_output)
            .pattern(["X", "X", "X", "X"])
            .key('X', "diamond");
        assert_eq!(
            builder.validate(),
            Err(RecipeError::PatternTooLarge {
                width: 1,
                height: 4
            })
        );
        std::mem::forget(builder);

        let dummy_output = unsafe { std::mem::zeroed() };
        let builder = ShapedRecipeBuilder::new("test:recipe", dummy_output)
            .pattern(["XX", "X"])
            .key('X', "diamond");
        assert_eq!(
            builder.validate(),
            Err(RecipeError::InconsistentRowWidth {
                expected: 2,
                found: 1
            })
        );
        std::mem::forget(builder);

        let dummy_output = unsafe { std::mem::zeroed() };
        let builder = ShapedRecipeBuilder::new("test:recipe", dummy_output)
            .pattern(["XY"])
            .key('X', "diamond");
        assert_eq!(builder.validate(), Err(RecipeError::MissingKey('Y')));
        std::mem::forget(builder);
    }

    #[test]
    fn shapeless_recipe_validation_errors() {
        let dummy_output = unsafe { std::mem::zeroed() };
        let builder = ShapelessRecipeBuilder::new("test:recipe", dummy_output);
        assert_eq!(builder.validate(), Err(RecipeError::NoIngredients));
        std::mem::forget(builder);

        let dummy_output = unsafe { std::mem::zeroed() };
        let builder =
            ShapelessRecipeBuilder::new("test:recipe", dummy_output).ingredient_count("stick", 10);
        assert_eq!(builder.validate(), Err(RecipeError::TooManyIngredients(10)));
        std::mem::forget(builder);
    }

    #[test]
    fn cooking_recipe_presets() {
        let dummy_output = unsafe { std::mem::zeroed() };
        let smelting = CookingRecipeBuilder::smelting("test:smelt", "raw_iron", dummy_output);
        assert_eq!(smelting.cooking_time, 200);
        assert_eq!(smelting.cooking_type, CookingType::Smelting);
        std::mem::forget(smelting);

        let dummy_output = unsafe { std::mem::zeroed() };
        let blasting = CookingRecipeBuilder::blasting("test:blast", "iron_ore", dummy_output);
        assert_eq!(blasting.cooking_time, 100);
        assert_eq!(blasting.cooking_type, CookingType::Blasting);
        std::mem::forget(blasting);

        let dummy_output = unsafe { std::mem::zeroed() };
        let campfire = CookingRecipeBuilder::campfire("test:camp", "beef", dummy_output);
        assert_eq!(campfire.cooking_time, 600);
        assert_eq!(campfire.cooking_type, CookingType::Campfire);
        std::mem::forget(campfire);
    }

    #[test]
    fn stonecutting_recipe_validation_errors() {
        let dummy_output = unsafe { std::mem::zeroed() };
        let builder = StonecuttingRecipeBuilder::new("", "glass", dummy_output);
        assert_eq!(builder.validate(), Err(RecipeError::EmptyId));
        std::mem::forget(builder);

        let dummy_output = unsafe { std::mem::zeroed() };
        let builder = StonecuttingRecipeBuilder::new("test:cut", "glass", dummy_output);
        assert_eq!(builder.validate(), Ok(()));
        std::mem::forget(builder);
    }

    #[test]
    fn smithing_recipe_defaults() {
        let dummy_output = unsafe { std::mem::zeroed() };
        let transform = SmithingTransformRecipeBuilder::new(
            "test:netherite",
            "netherite_upgrade_smithing_template",
            "diamond_chestplate",
            "netherite_ingot",
            dummy_output,
        );
        assert!(!transform.copy_components);
        assert_eq!(transform.validate(), Ok(()));
        std::mem::forget(transform);

        let trim = SmithingTrimRecipeBuilder::new(
            "test:trim",
            "coast_armor_trim_smithing_template",
            "iron_chestplate",
            "amethyst_shard",
        );
        assert_eq!(trim.validate(), Ok(()));

        let trim = SmithingTrimRecipeBuilder::new("", "template", "base", "addition");
        assert_eq!(trim.validate(), Err(RecipeError::EmptyId));
    }

    #[test]
    fn brewing_recipe_validation_and_normalization() {
        let builder =
            BrewingRecipeBuilder::new("test:brew", "potion", "gunpowder", "splash_potion")
                .input_potion("water")
                .output_potion("minecraft:thick");
        assert_eq!(builder.validate(), Ok(()));
        assert_eq!(builder.input_item, "minecraft:potion");
        assert_eq!(builder.reagent, "minecraft:gunpowder");
        assert_eq!(builder.output_item, "minecraft:splash_potion");
        assert_eq!(builder.input_potion.as_deref(), Some("minecraft:water"));
        assert_eq!(builder.output_potion.as_deref(), Some("minecraft:thick"));

        let builder = BrewingRecipeBuilder::new("", "potion", "gunpowder", "splash_potion");
        assert_eq!(builder.validate(), Err(RecipeError::EmptyId));

        let builder = BrewingRecipeBuilder::new("test:brew", "", "gunpowder", "splash_potion");
        assert_eq!(
            builder.validate(),
            Err(RecipeError::EmptyBrewingField("input_item"))
        );

        let builder = BrewingRecipeBuilder::new("test:brew", "potion", "", "splash_potion");
        assert_eq!(
            builder.validate(),
            Err(RecipeError::EmptyBrewingField("reagent"))
        );

        let builder = BrewingRecipeBuilder::new("test:brew", "potion", "gunpowder", "");
        assert_eq!(
            builder.validate(),
            Err(RecipeError::EmptyBrewingField("output_item"))
        );
    }
}
