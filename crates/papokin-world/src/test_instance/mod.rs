//! 内嵌的 `GameTest` 测试实例资源。
//!
//! 测试实例是数据包注册表条目，存储于
//! `data/<namespace>/test_instance/*.json`。它们独立于结构
//! 模板：测试实例引用某个结构，但其本身并不是
//! 结构模板，因此不属于 [`crate::generation::structure::template::TemplateCache`]。

/// 原版的隐式命名空间。
const DEFAULT_NAMESPACE: &str = "minecraft";

/// 将资源 ID 规范化为完全限定的 `namespace:path` 形式。
fn canonicalize(name: &str) -> String {
    if name.contains(':') {
        name.to_owned()
    } else {
        format!("{DEFAULT_NAMESPACE}:{name}")
    }
}

include!(concat!(env!("OUT_DIR"), "/test_instance_embeddings.rs"));

/// 返回内嵌测试实例的原始 JSON。
///
/// `id` 可为裸形式（`foo`）或带命名空间（`minecraft:foo`、`papokin:foo`）。
#[must_use]
pub fn json(id: &str) -> Option<&'static str> {
    get_test_instance_json(&canonicalize(id))
}

///返回所有内嵌测试实例的资源 ID。
///
/// 名称是完全限定的，例如 `papokin:creeper_should_run_from_cat`。
#[must_use]
#[allow(clippy::used_underscore_items)]
pub const fn all_names() -> &'static [&'static str] {
    _generated_all_test_instance_names()
}
