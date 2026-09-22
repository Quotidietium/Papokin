//! 插件配置文件（等同于 Bukkit 的 `getConfig`）。
//!
//! 宿主将你的插件配置存储在 `<data folder>/config.toml`。调用
//! [`Context::load_config`] 读取它（合并覆盖
//! [`Plugin::config_defaults`](crate::Plugin::config_defaults) 返回的默认值——新的默认
//! 键会自动加入，用户值在插件升级后保留。
//!
//! # Example
//!
//! ```rust,ignore
//! impl Plugin for MyPlugin {
//!     fn config_defaults(&self) -> String {
//!         "welcome-message = \"Hello!\"\nmax-teleports = 5\n".into()
//!     }
//! }
//!
//! // Somewhere in the plugin:
//! let config = context.load_config()?;
//! ```
//!
//! 用 `toml` crate（或任意 TOML 解析器）解析返回的 TOML。

use crate::Context;

impl Context {
    /// 加载插件配置，并合并覆盖 TOML `defaults` 字符串。
    ///
    /// 合并后的文档会被持久化写回磁盘。
    ///
    /// # Errors
    ///当任一文档不是有效的 TOML 时，返回错误。
    pub fn load_config_with_defaults(&self, defaults: &str) -> crate::Result<String> {
        crate::wit::papokin::plugin::config::load_config(defaults)
    }

    /// 加载插件配置，并合并覆盖 [`Plugin::config_defaults`](crate::Plugin::config_defaults)。
    ///
    /// 合并后的文档会被持久化写回磁盘。
    ///
    /// # Errors
    ///当任一文档不是有效的 TOML 时，返回错误。
    pub fn load_config(&self) -> crate::Result<String> {
        let defaults = crate::config_defaults();
        crate::wit::papokin::plugin::config::load_config(&defaults)
    }

    /// 用给定的 TOML 内容覆盖插件的配置文件。
    ///
    /// # Errors
    ///当内容不是有效的 TOML 或写入失败时，返回错误。
    pub fn save_config(&self, content: &str) -> crate::Result<()> {
        crate::wit::papokin::plugin::config::save_config(content)
    }
}
