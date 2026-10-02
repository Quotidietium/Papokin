//! 插件生命周期的查看与管理（相当于 Paper 的 `PluginManager` 插件面）。
//!
//! 本模块向插件暴露完整的动态管理能力：列出/查询插件、动态加载、
//! 卸载、启用与禁用。第三方插件可据此实现自有的插件管理器，
//! 替代内置的 `plugman` 指令。
//!
//! # Examples
//!
//! ## 列出全部插件
//! ```rust,ignore
//! use papokin_plugin_api::Server;
//!
//! fn log_plugins(server: &Server) {
//!     let manager = server.get_plugin_manager();
//!     for plugin in manager.list_plugins() {
//!         println!("插件 {} (v{})：启用 = {}", plugin.name, plugin.version, plugin.is_active);
//!     }
//! }
//! ```
//!
//! ## 禁用并重新启用某个插件
//! ```rust,ignore
//! use papokin_plugin_api::Server;
//!
//! fn restart_plugin(server: &Server, name: &str) -> Result<(), String> {
//!     let manager = server.get_plugin_manager();
//!     manager.disable_plugin(name)?;
//!     manager.enable_plugin(name)
//! }
//! ```
//!
//! ## 从插件目录动态加载
//! ```rust,ignore
//! use papokin_plugin_api::Server;
//!
//! fn load_extra(server: &Server) -> Result<(), String> {
//!     let manager = server.get_plugin_manager();
//!     manager.load_plugin("my-addon.wasm")
//! }
//! ```

pub use crate::wit::papokin::plugin::plugin_manager::{PluginInfo, PluginManager, PluginState};
