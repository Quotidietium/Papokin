//! 插件可写的自定义注册表条目工具。
//!
//! 插件通过 [`RegistryManager`] 注册自定义条目（如新的伤害类型），
//! 在服务器启动期间完成。客户端连接时，这些条目会追加到
//! 注册表数据包所发送的原版同步注册表，因此自定义条目的
//! 网络 id 是该域的原版条目数加上条目的注册索引。
//!
//! 仅能在服务器启动（插件加载）期间注册；
//! 注册表冻结后，注册会以
//! [`RegistryError::RegistrationFailed`] 失败。
//!
//! 大多数插件应优先使用构建在本管理器之上的专用管理器
//! （[`DamageTypeManager`](crate::damage_type::DamageTypeManager)、附魔
//! 管理器）会为你序列化条目负载；本管理器是通用的
//! 后备方案，适用于没有专用 API 的注册表。
//!
//! # Examples
//!
//! ```rust,ignore
//! use papokin_plugin_api::Context;
//!
//! fn register_entries(context: &Context) {
//!     let manager = context.get_registry_manager();
//!     let index = manager
//!         .register("damage_type", "my_plugin:frost", serialized_nbt)
//!         .expect("failed to register registry entry");
//!     assert!(manager.has("damage_type", "my_plugin:frost"));
//!     assert_eq!(manager.network_id("damage_type", "my_plugin:frost").is_some(), true);
//! }
//! ```

pub use crate::wit::papokin::plugin::registry::RegistryManager;
use crate::{Context, Server};
use std::fmt;

/// 注册自定义注册表条目时可能发生的错误。
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RegistryError {
    /// 向服务器的注册表管理器注册失败（例如名称重复或
    /// 已冻结的注册表）。
    RegistrationFailed(String),
}

impl fmt::Display for RegistryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::RegistrationFailed(msg) => {
                write!(f, "注册注册表条目失败：{msg}")
            }
        }
    }
}

impl std::error::Error for RegistryError {}

impl RegistryManager {
    /// 在 `domain` 下注册自定义条目（例如 `"damage_type"`，不带
    /// `"minecraft:"` 前缀）及其序列化的 NBT 载荷（网络格式）。
    ///
    /// 返回条目在其域内的索引；对于某种域，则返回条目的网络 ID。
    /// 客户端则是该客户端的原版条目数加上此索引。
    ///
    /// # Errors
    /// 返回 [`RegistryError::RegistrationFailed`]，当名称与
    /// 已存在的自定义条目，或注册表已冻结。
    pub fn register(&self, domain: &str, name: &str, nbt: Vec<u8>) -> Result<u16, RegistryError> {
        self.register_entry(domain, name, &nbt)
            .map_err(RegistryError::RegistrationFailed)
    }

    /// 自定义条目在服务器原生协议版本下的网络 id：
    /// 该域的原版条目数加上该条目的注册索引。
    ///
    ///对于未知的域或名称，以及尚未加载的注册表，返回 `None`。
    /// 同步给客户端。
    #[must_use]
    pub fn network_id(&self, domain: &str, name: &str) -> Option<u16> {
        self.custom_network_id(domain, name)
    }

    /// 是否有以该名称注册的自定义条目位于 `domain` 下。
    #[must_use]
    pub fn has(&self, domain: &str, name: &str) -> bool {
        self.has_entry(domain, name)
    }

    /// 某域全部自定义条目的名称，按注册顺序。
    #[must_use]
    pub fn names(&self, domain: &str) -> Vec<String> {
        self.get_entries(domain)
    }
}

impl Context {
    /// 返回全局注册表管理器，用于注册和查询自定义
    /// 注册表条目。
    #[must_use]
    pub fn get_registry_manager(&self) -> RegistryManager {
        self.get_server().get_registry_manager()
    }

    /// 向服务器注册一个自定义注册表条目。
    ///
    /// # Errors
    /// 返回 [`RegistryError::RegistrationFailed`]，当名称与
    /// 已存在的自定义条目，或注册表已冻结。
    pub fn register_registry_entry(
        &self,
        domain: &str,
        name: &str,
        nbt: Vec<u8>,
    ) -> Result<u16, RegistryError> {
        self.get_registry_manager().register(domain, name, nbt)
    }
}

impl Server {
    /// 向服务器注册一个自定义注册表条目。
    ///
    /// # Errors
    /// 返回 [`RegistryError::RegistrationFailed`]，当名称与
    /// 已存在的自定义条目，或注册表已冻结。
    pub fn register_registry_entry(
        &self,
        domain: &str,
        name: &str,
        nbt: Vec<u8>,
    ) -> Result<u16, RegistryError> {
        self.get_registry_manager().register(domain, name, nbt)
    }
}
