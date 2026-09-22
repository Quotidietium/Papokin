//! 插件可写的标签覆盖工具。
//!
//! 标签以静态的、按版本划分的表格编译进服务器。插件使用
//! [`TagManager`] 在服务器启动期间向现有标签追加条目、
//! 从中移除条目，或创建全新标签；合并结果会
//! 在 update-tags 数据包中发送给连接的客户端。
//!
//! 仅能在服务器启动（插件加载）期间修改；
//! 标签表冻结后，所有变更调用都会以 [`TagError`] 失败。
//!
//! # Examples
//!
//! ```rust,ignore
//! use papokin_plugin_api::Context;
//!
//! fn register_tags(context: &Context) {
//!     let manager = context.get_tag_manager();
//!     manager
//!         .add("damage_type", "my_plugin:custom_damage", "my_plugin:frost")
//!         .expect("failed to add to tag");
//!     assert!(manager
//!         .get_values("damage_type", "my_plugin:custom_damage")
//!         .iter()
//!         .any(|name| name == "my_plugin:frost"));
//! }
//! ```

use crate::Context;
pub use crate::wit::papokin::plugin::tag::TagManager;
use std::fmt;

/// 修改标签时可能发生的错误。
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TagError {
    /// 向标签添加条目失败（如注册表键未知或标签表已冻结）。
    AddFailed(String),
    /// 从标签中移除条目失败（例如注册表键未知或标签表已冻结）。
    RemoveFailed(String),
    /// 创建标签失败（如注册表键未知或标签表已冻结）。
    CreateFailed(String),
}

impl fmt::Display for TagError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AddFailed(msg) => write!(f, "向标签添加条目失败：{msg}"),
            Self::RemoveFailed(msg) => write!(f, "从标签移除条目失败：{msg}"),
            Self::CreateFailed(msg) => write!(f, "创建标签失败：{msg}"),
        }
    }
}

impl std::error::Error for TagError {}

impl TagManager {
    /// 将 `entry_name` 追加到 `registry_key` 的 `tag_name` 标签，标签不存在时
    /// 当它还不存在时。
    ///
    /// * `registry_key`：标签所属的注册表，如 `"item"`、`"block"` 或
    ///   `"damage_type"`（接受 `"minecraft:"` 前缀）。
    /// * `tag_name`：带命名空间的标签名，如 `"minecraft:anvil"` 或 `"my_plugin:things"`。
    /// * `entry_name`：条目的资源名：原版名（`"minecraft:stone"`
    ///   或裸 `"stone"`），或插件注册的自定义命名空间 id（`"my_plugin:frost"`）。
    ///
    /// # Errors
    /// 返回 [`TagError::AddFailed`]，当注册表键未知或标签
    /// 表即被冻结。
    pub fn add(
        &self,
        registry_key: &str,
        tag_name: &str,
        entry_name: &str,
    ) -> Result<(), TagError> {
        self.add_to_tag(registry_key, tag_name, entry_name)
            .map_err(TagError::AddFailed)
    }

    /// 从 `registry_key` 的标签 `tag_name` 中移除 `entry_name`。该条目可以
    /// 来自静态表，或来自之前的 [`TagManager::add`]。
    ///
    /// # Errors
    /// 返回 [`TagError::RemoveFailed`]，当注册表键未知或标签
    /// 表即被冻结。
    pub fn remove(
        &self,
        registry_key: &str,
        tag_name: &str,
        entry_name: &str,
    ) -> Result<(), TagError> {
        self.remove_from_tag(registry_key, tag_name, entry_name)
            .map_err(TagError::RemoveFailed)
    }

    /// 创建一个新的、初始为空的标签。[`TagManager::add`] 会隐式创建标签
    /// 是隐式创建；显式创建对于发布空标签很有用。
    ///
    /// # Errors
    /// 返回 [`TagError::CreateFailed`]，当注册表键未知或标签
    /// 表即被冻结。
    pub fn create(&self, registry_key: &str, tag_name: &str) -> Result<(), TagError> {
        self.create_tag(registry_key, tag_name)
            .map_err(TagError::CreateFailed)
    }

    /// 某个标签条目名的合并视图：静态表加上插件添加、
    /// 减去插件移除的部分。标签未知时为空。
    #[must_use]
    pub fn get_values(&self, registry_key: &str, tag_name: &str) -> Vec<String> {
        self.get_tag_values(registry_key, tag_name)
    }
}

impl Context {
    /// 返回全局标签管理器，用于修改和查询标签表。
    #[must_use]
    pub fn get_tag_manager(&self) -> TagManager {
        self.get_server().get_tag_manager()
    }

    /// 将 `entry_name` 追加到 `registry_key` 的 `tag_name` 标签，标签不存在时
    /// 当它还不存在时。
    ///
    /// # Errors
    /// 返回 [`TagError::AddFailed`]，当注册表键未知或标签
    /// 表即被冻结。
    pub fn add_to_tag(
        &self,
        registry_key: &str,
        tag_name: &str,
        entry_name: &str,
    ) -> Result<(), TagError> {
        self.get_tag_manager()
            .add(registry_key, tag_name, entry_name)
    }
}
