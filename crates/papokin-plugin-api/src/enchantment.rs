//! 插件自定义附魔注册与构建器工具。
//!
//! 本模块提供流畅、类型安全的 API，用于定义、注册和查询
//! 自定义附魔，以及将自定义附魔应用到 [`ItemStack`](crate::ItemStack)。
//!
//! # Examples
//!
//! ## Defining and Registering a Custom Enchantment
//! ```rust,ignore
//! use papokin_plugin_api::{
//!     enchantment::{AttributeModifierSlot, EnchantmentBuilder},
//!     text::TextComponent,
//!     Server,
//! };
//!
//! fn register_enchantments(server: &Server) {
//!     let manager = server.get_enchantment_manager();
//!
//!     manager.register(
//!         EnchantmentBuilder::new("my_plugin:lifesteal", TextComponent::text("Life Steal"))
//!             .max_level(3)
//!             .anvil_cost(4)
//!             .supported_items("#minecraft:enchantable/weapon")
//!             .weight(2)
//!             .slots([AttributeModifierSlot::MainHand])
//!             .exclusive_with("custom:poison_touch")
//!     ).expect("failed to register custom enchantment");
//! }
//! ```
//!
//! ## Applying Custom Enchantments to an [`ItemStack`](crate::ItemStack)
//! ```rust,ignore
//! use papokin_plugin_api::ItemStack;
//!
//! fn give_sword() -> ItemStack {
//!     let mut sword = ItemStack::new("minecraft:diamond_sword", 1);
//!     sword.add_custom_enchantment("my_plugin:lifesteal", 2);
//!     assert!(sword.has_custom_enchantment("my_plugin:lifesteal"));
//!     assert_eq!(sword.get_custom_enchantment_level("my_plugin:lifesteal"), Some(2));
//!     sword
//! }
//! ```

pub use crate::wit::papokin::plugin::enchantments::{
    AttributeModifierSlot, CustomEnchantment, Enchantment, EnchantmentManager,
};
pub use crate::wit::papokin::plugin::item_stack::CustomEnchantmentValue;
use crate::{Context, Server, TextComponent};
use std::fmt;

/// 构建或注册自定义附魔时可能发生的错误。
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EnchantmentError {
    /// 附魔标识符为空。
    EmptyId,
    /// 附魔最大等级无效（必须 >= 1）。
    InvalidMaxLevel,
    /// 向服务器的附魔管理器注册失败。
    RegistrationFailed(String),
}

impl fmt::Display for EnchantmentError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyId => write!(f, "附魔标识符不能为空"),
            Self::InvalidMaxLevel => write!(f, "附魔最大等级必须至少为 1"),
            Self::RegistrationFailed(msg) => {
                write!(f, "注册附魔失败：{msg}")
            }
        }
    }
}

impl std::error::Error for EnchantmentError {}

/// 用于构建并校验 [`CustomEnchantment`] 的流式构建器。
pub struct EnchantmentBuilder {
    id: String,
    description: TextComponent,
    max_level: u32,
    anvil_cost: u32,
    supported_items: String,
    weight: u32,
    slots: Vec<AttributeModifierSlot>,
    exclusive_set: Vec<String>,
}

impl EnchantmentBuilder {
    /// 以给定的唯一标识符和描述创建新的附魔构建器。
    ///
    /// # Example
    /// ```rust,ignore
    /// let builder = EnchantmentBuilder::new("my_plugin:lifesteal", TextComponent::text("Life Steal"));
    /// ```
    #[must_use]
    pub fn new(id: impl Into<String>, description: impl Into<TextComponent>) -> Self {
        Self {
            id: id.into(),
            description: description.into(),
            max_level: 1,
            anvil_cost: 4,
            supported_items: "#minecraft:enchantable/weapon".into(),
            weight: 5,
            slots: vec![AttributeModifierSlot::MainHand],
            exclusive_set: Vec::new(),
        }
    }

    /// 设置该附魔的最高等级（默认：1）。
    #[must_use]
    pub const fn max_level(mut self, max_level: u32) -> Self {
        self.max_level = max_level;
        self
    }

    /// 设置铁砧基础费用倍率（默认：4）。
    #[must_use]
    pub const fn anvil_cost(mut self, anvil_cost: u32) -> Self {
        self.anvil_cost = anvil_cost;
        self
    }

    /// 设置支持的物品模式或标签（例如 `"#minecraft:enchantable/weapon"`）。
    #[must_use]
    pub fn supported_items(mut self, items: impl Into<String>) -> Self {
        self.supported_items = items.into();
        self
    }

    /// 设置附魔的权重/稀有度（1..=10，数值越高越常见，默认：5）。
    #[must_use]
    pub const fn weight(mut self, weight: u32) -> Self {
        self.weight = weight;
        self
    }

    /// 为此附魔添加一个生效装备槽。
    #[must_use]
    pub fn slot(mut self, slot: AttributeModifierSlot) -> Self {
        self.slots.push(slot);
        self
    }

    /// 替换此附魔生效的装备栏位。
    #[must_use]
    pub fn slots(mut self, slots: impl IntoIterator<Item = AttributeModifierSlot>) -> Self {
        self.slots = slots.into_iter().collect();
        self
    }

    /// 添加一个互斥/冲突的附魔 ID。
    #[must_use]
    pub fn exclusive_with(mut self, id: impl Into<String>) -> Self {
        self.exclusive_set.push(id.into());
        self
    }

    /// 替换互斥 / 冲突附魔列表。
    #[must_use]
    pub fn exclusive_set(mut self, set: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.exclusive_set = set.into_iter().map(Into::into).collect();
        self
    }

    /// 验证附魔参数。
    ///
    /// # Errors
    /// 若校验失败，返回 [`EnchantmentError`]。
    pub fn validate(&self) -> Result<(), EnchantmentError> {
        if self.id.trim().is_empty() {
            return Err(EnchantmentError::EmptyId);
        }
        if self.max_level == 0 {
            return Err(EnchantmentError::InvalidMaxLevel);
        }
        Ok(())
    }

    /// 构建校验后的 [`CustomEnchantment`] 记录。
    ///
    /// # Errors
    /// 若校验失败，返回 [`EnchantmentError`]。
    pub fn build(self) -> Result<CustomEnchantment, EnchantmentError> {
        self.validate()?;
        Ok(CustomEnchantment {
            id: self.id,
            description: self.description,
            max_level: self.max_level,
            anvil_cost: self.anvil_cost,
            supported_items: self.supported_items,
            weight: self.weight,
            slots: self.slots,
            exclusive_set: self.exclusive_set,
        })
    }

    /// 直接向所提供的 [`EnchantmentManager`] 注册此自定义附魔。
    ///
    /// # Errors
    /// 若校验或注册失败，返回 [`EnchantmentError`]。
    pub fn register(self, manager: &EnchantmentManager) -> Result<(), EnchantmentError> {
        manager.register(self)
    }

    /// 向服务器注册此自定义附魔。
    ///
    /// # Errors
    /// 若校验或注册失败，返回 [`EnchantmentError`]。
    pub fn register_to_server(self, server: &Server) -> Result<(), EnchantmentError> {
        let manager = server.get_enchantment_manager();
        manager.register(self)
    }

    /// 向插件上下文注册此自定义附魔。
    ///
    /// # Errors
    /// 若校验或注册失败，返回 [`EnchantmentError`]。
    pub fn register_to_context(self, context: &Context) -> Result<(), EnchantmentError> {
        let manager = context.get_enchantment_manager();
        manager.register(self)
    }
}

/// 可通过 [`EnchantmentManager`] 注册的附魔类型 trait。
pub trait RegistrableEnchantment {
    /// 向所提供的附魔管理器注册此附魔。
    ///
    /// # Errors
    /// 若校验或注册失败，返回 [`EnchantmentError`]。
    fn register(self, manager: &EnchantmentManager) -> Result<(), EnchantmentError>;
}

impl RegistrableEnchantment for EnchantmentBuilder {
    fn register(self, manager: &EnchantmentManager) -> Result<(), EnchantmentError> {
        let enchantment = self.build()?;
        manager
            .register_enchantment(enchantment)
            .map_err(EnchantmentError::RegistrationFailed)
    }
}

impl RegistrableEnchantment for CustomEnchantment {
    fn register(self, manager: &EnchantmentManager) -> Result<(), EnchantmentError> {
        manager
            .register_enchantment(self)
            .map_err(EnchantmentError::RegistrationFailed)
    }
}

impl EnchantmentManager {
    /// 向服务器注册一个自定义附魔。
    ///
    /// 接受 [`EnchantmentBuilder`] 或 [`CustomEnchantment`]。
    ///
    /// # Errors
    /// 若注册或校验失败，返回 [`EnchantmentError`]。
    pub fn register(
        &self,
        enchantment: impl RegistrableEnchantment,
    ) -> Result<(), EnchantmentError> {
        enchantment.register(self)
    }

    /// 按 ID 获取附魔定义（自定义或原版）。
    #[must_use]
    pub fn get(&self, id: &str) -> Option<CustomEnchantment> {
        self.get_enchantment(id)
    }

    /// 检查服务器上是否注册了某个附魔 ID。
    #[must_use]
    pub fn has(&self, id: &str) -> bool {
        self.has_enchantment(id)
    }

    ///返回所有已注册的附魔 ID（自定义与原版）。
    #[must_use]
    pub fn get_all_ids(&self) -> Vec<String> {
        self.get_all_enchantment_ids()
    }
}

impl Context {
    /// 返回全局附魔管理器，用于注册和查询自定义附魔。
    #[must_use]
    pub fn get_enchantment_manager(&self) -> EnchantmentManager {
        self.get_server().get_enchantment_manager()
    }

    /// 向服务器注册一个自定义附魔。
    ///
    /// # Errors
    /// 若校验或注册失败，返回 [`EnchantmentError`]。
    pub fn register_enchantment(
        &self,
        enchantment: impl RegistrableEnchantment,
    ) -> Result<(), EnchantmentError> {
        self.get_enchantment_manager().register(enchantment)
    }

    /// 按 ID 获取附魔定义。
    #[must_use]
    pub fn get_enchantment(&self, id: &str) -> Option<CustomEnchantment> {
        self.get_server().get_enchantment(id)
    }
}

impl Server {
    /// 向服务器注册一个自定义附魔。
    ///
    /// # Errors
    /// 若校验或注册失败，返回 [`EnchantmentError`]。
    pub fn register_enchantment(
        &self,
        enchantment: impl RegistrableEnchantment,
    ) -> Result<(), EnchantmentError> {
        self.get_enchantment_manager().register(enchantment)
    }
}

/// 将正整数转换为标准罗马数字表示（如 `1 -> "I"`、`3 -> "III"`、`5 -> "V"`）。
#[must_use]
pub fn to_roman_numeral(level: u32) -> String {
    match level {
        1 => "I".into(),
        2 => "II".into(),
        3 => "III".into(),
        4 => "IV".into(),
        5 => "V".into(),
        6 => "VI".into(),
        7 => "VII".into(),
        8 => "VIII".into(),
        9 => "IX".into(),
        10 => "X".into(),
        _ => level.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn enchantment_builder_defaults() {
        let dummy_text: TextComponent = unsafe { std::mem::zeroed() };
        let builder = EnchantmentBuilder::new("custom:freeze", dummy_text);
        assert_eq!(builder.max_level, 1);
        assert_eq!(builder.anvil_cost, 4);
        assert_eq!(builder.weight, 5);
        assert_eq!(builder.slots, vec![AttributeModifierSlot::MainHand]);
        std::mem::forget(builder);
    }

    #[test]
    fn enchantment_builder_validation() {
        let dummy_text: TextComponent = unsafe { std::mem::zeroed() };
        let builder = EnchantmentBuilder::new("", dummy_text);
        assert_eq!(builder.validate(), Err(EnchantmentError::EmptyId));
        std::mem::forget(builder);

        let dummy_text: TextComponent = unsafe { std::mem::zeroed() };
        let builder = EnchantmentBuilder::new("custom:poison", dummy_text).max_level(0);
        assert_eq!(builder.validate(), Err(EnchantmentError::InvalidMaxLevel));
        std::mem::forget(builder);
    }

    #[test]
    fn roman_numerals() {
        assert_eq!(to_roman_numeral(1), "I");
        assert_eq!(to_roman_numeral(2), "II");
        assert_eq!(to_roman_numeral(3), "III");
        assert_eq!(to_roman_numeral(4), "IV");
        assert_eq!(to_roman_numeral(5), "V");
        assert_eq!(to_roman_numeral(10), "X");
        assert_eq!(to_roman_numeral(255), "255");
    }
}
