/// 一个可用于将某物视为动态类型映射视图的 trait。
/// 此 trait 的 [`Value`] 是此类 map 型结构的*动态类型*。
pub trait MapLike {
    type Value;

    /// 以此映射结构 *动态类型* 的键获取此映射视图中的值。
    fn get(&self, key: &Self::Value) -> Option<&Self::Value>;

    /// 以 `&str` 作为此映射结构 *动态类型* 的键，并使用提供的、与此映射结构 *动态类型* 匹配的 [`DynamicOps`]，获取此映射视图中的值。
    fn get_str(&self, key: &str) -> Option<&Self::Value>;

    ///返回一个 `Iterator`，遍历此 map-like 中的每个键值对，键和值都是其*动态类型*。
    fn iter(&self) -> impl Iterator<Item = (Self::Value, &Self::Value)> + '_;
}
