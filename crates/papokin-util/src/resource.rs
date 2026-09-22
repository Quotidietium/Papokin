use crate::identifier::Identifier;

/// 一个注册表作用域的资源键，用于标识特定注册表内的元素。
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct ResourceKey {
    pub registry_name: Identifier,
    /// 该资源的唯一标识符。
    pub identifier: Identifier,
}

impl ResourceKey {
    /// 创建与特定注册表名称关联的新 `ResourceKey`。
    #[must_use]
    pub const fn new(registry_name: Identifier, identifier: Identifier) -> Self {
        Self {
            registry_name,
            identifier,
        }
    }

    /// 若资源键的注册表名称与给定注册表标识符匹配，则对其进行类型转换。
    #[must_use]
    pub fn cast(&self, registry: &Identifier) -> Option<&Self> {
        (self.registry_name == *registry).then_some(self)
    }
}
