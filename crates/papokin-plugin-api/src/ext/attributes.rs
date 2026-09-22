use crate::wit::papokin::plugin::attributes::{AttributeModifier, ModifierOperation};

impl AttributeModifier {
    /// 创建新的属性修饰符。
    #[must_use]
    pub fn new(id: impl Into<String>, amount: f64, operation: ModifierOperation) -> Self {
        Self {
            id: id.into(),
            amount,
            operation,
        }
    }

    /// 创建加算属性修饰符。
    #[must_use]
    pub fn add(id: impl Into<String>, amount: f64) -> Self {
        Self::new(id, amount, ModifierOperation::Add)
    }

    /// 创建乘算基础属性修饰符（`base * (1 + amount)`）。
    #[must_use]
    pub fn multiply_base(id: impl Into<String>, amount: f64) -> Self {
        Self::new(id, amount, ModifierOperation::MultiplyBase)
    }

    /// 创建乘算总值属性修饰符（`total * (1 + amount)`）。
    #[must_use]
    pub fn multiply_total(id: impl Into<String>, amount: f64) -> Self {
        Self::new(id, amount, ModifierOperation::MultiplyTotal)
    }
}

use crate::wit::papokin::plugin::attributes::Attribute;
use crate::wit::papokin::plugin::enchantments::AttributeModifierSlot;
use crate::wit::papokin::plugin::item_stack::ItemAttributeModifier;

impl ItemAttributeModifier {
    /// 为指定装备槽创建新的物品属性修饰符。
    #[must_use]
    pub const fn new(
        attribute: Attribute,
        modifier: AttributeModifier,
        slot: AttributeModifierSlot,
    ) -> Self {
        Self {
            attribute,
            modifier,
            slot,
        }
    }
}
