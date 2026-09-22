//! 跨插件服务注册表（等同于 Bukkit 的 `ServicesManager`）。
//!
//! 服务是一种命名能力，由一个插件提供、其他插件
//! 消费。由于插件运行在相互隔离的沙箱中，消费者在此查找
//! 提供者，并通过 [`crate::ipc`] 调用它（参见
//! [`crate::Context::send_ipc_message`]）。
//!
//! # Example
//!
//! ```rust,ignore
//! // Provider (in on_enable):
//! context.register_service("my-plugin:economy", 0)?;
//!
//! // Consumer:
//! if let Some(provider) = context.get_service_provider("my-plugin:economy") {
//!     let reply = context.send_ipc_message(&provider.plugin, b"balance:Steve".to_vec())?;
//! }
//! ```

pub use crate::wit::papokin::plugin::services::ServiceProvider;

use crate::Context;

impl Context {
    /// 将本插件注册为 `service` 的提供者。
    ///
    /// 重复注册同一服务会更新其优先级。注册
    /// 会在插件被卸载或禁用时自动移除。
    ///
    /// # Errors
    ///若宿主拒绝了注册，则返回错误字符串。
    pub fn register_service(&self, service: &str, priority: i32) -> crate::Result<()> {
        crate::wit::papokin::plugin::services::register_service(service, priority)
    }

    /// 移除本插件对 `service` 的注册。
    pub fn unregister_service(&self, service: &str) {
        crate::wit::papokin::plugin::services::unregister_service(service);
    }

    /// 返回 `service` 当前激活的最高优先级提供者（如有）。
    #[must_use]
    pub fn get_service_provider(&self, service: &str) -> Option<ServiceProvider> {
        crate::wit::papokin::plugin::services::get_service_provider(service)
    }

    ///返回 `service` 的所有活动提供者，按优先级降序排序
    /// 优先级排序（其次按注册顺序）。
    #[must_use]
    pub fn get_service_providers(&self, service: &str) -> Vec<ServiceProvider> {
        crate::wit::papokin::plugin::services::get_service_providers(service)
    }
}
