/// 资源的完全限定标识符，通常为 `namespace:path` 形式。
pub type ResourceLocation = String;

/// 将一个类型转换为 `ResourceLocation`。
pub trait ToResourceLocation: Sized {
    /// 将当前实例转换为 `ResourceLocation`。
    ///
    /// # Returns
    /// 一个 `String`，表示完全限定的资源标识符。
    fn to_resource_location(&self) -> ResourceLocation;
}

/// 从 `ResourceLocation` 构造一个类型。
pub trait FromResourceLocation: Sized {
    /// 尝试从给定的 `ResourceLocation` 创建实例。
    ///
    /// # Arguments
    /// * `resource_location` - 要解析的资源标识符。
    ///
    /// # Returns
    /// 解析成功时返回 `Some(Self)`，输入无效时返回 `None`。
    fn from_resource_location(resource_location: &ResourceLocation) -> Option<Self>;
}
