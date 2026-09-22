use std::collections::HashMap;

use serde::{Deserialize, Serialize};

/// 插件系统配置。
#[derive(Deserialize, Serialize, Clone, Debug)]
#[serde(default)]
pub struct PluginsConfig {
    /// 插件系统是否启用。若为 false，则不会加载任何插件。
    pub enabled: bool,
    /// 是否监视插件目录并自动热重载被修改的插件。
    pub hot_reload: bool,
    /// 当插件请求新权限时，服务器是否在控制台中要求确认。
    pub ask_permission_confirmation: bool,
    /// 是否允许加载未签名的 WASM 插件。
    pub allow_unsigned: bool,
    /// 全局预先批准给所有插件使用的权限列表（无需确认）。
    pub allowed_permissions: Vec<String>,
    /// 全局禁止所有插件使用的权限列表。
    pub blocked_permissions: Vec<String>,
    /// 宿主环境变量是否默认无需显式授权即继承到 WASI 环境中。
    pub inherit_env: bool,
    /// 插件中的网络套接字是否默认限制为 localhost/loopback。
    pub loopback_only: bool,
    /// 每个插件实例的可选全局最大内存限制（以 MB 计）。
    /// 如果未设置，内存仅受宿主系统可用内存的限制。
    pub max_memory_mb: Option<u64>,
    /// 每个插件的配置与覆盖项，以插件名称为键。
    ///
    /// 每个条目以其适用的插件命名（例如 `my_plugin`）。
    /// 只需列出你真正想要配置或覆盖的插件即可。
    ///
    /// 示例：
    ///
    /// ```toml
    /// [plugins.overrides.my_plugin]
    /// enabled = true
    /// allow_unsigned = true
    /// max_memory_mb = 128
    /// allowed_permissions = ["fs:read:data", "fs:write:data"]
    /// blocked_permissions = ["network:outbound"]
    /// loopback_only = true
    ///
    /// [plugins.overrides.my_plugin.environment]
    /// MY_API_KEY = "secret_key"
    /// ```
    pub overrides: HashMap<String, PluginOverride>,
    /// Pumpkin 是否应在加载前验证 WASM 插件签名。
    pub verify_signatures: bool,
}

impl Default for PluginsConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            hot_reload: false,
            ask_permission_confirmation: true,
            allow_unsigned: true,
            allowed_permissions: Vec::new(),
            blocked_permissions: Vec::new(),
            inherit_env: false,
            loopback_only: false,
            max_memory_mb: None,
            overrides: HashMap::new(),
            verify_signatures: true,
        }
    }
}

/// 单个插件的设置，允许服务器管理员将其关闭或更改
/// 其权限、未签名执行策略、内存限制或环境变量。
#[derive(Deserialize, Serialize, Clone, Debug)]
#[serde(default)]
pub struct PluginOverride {
    /// 此特定插件是否启用。若设为 `false`，加载时将忽略该插件。
    pub enabled: bool,
    /// 覆盖此插件在未签名的情况下是否允许运行。
    pub allow_unsigned: Option<bool>,
    /// 此特定插件的可选最大内存限制（以 MB 计）。
    /// 如已指定，则覆盖全局 `max_memory_mb` 设置。
    pub max_memory_mb: Option<u64>,
    /// 专门为此插件预先批准的权限（跳过交互式确认）。
    pub allowed_permissions: Vec<String>,
    /// 专门为此插件阻止的附加权限。
    pub blocked_permissions: Vec<String>,
    /// 覆盖此插件的网络访问是否仅限环回（loopback）。
    pub loopback_only: Option<bool>,
    /// 直接传递给该插件 WASI 环境的自定义环境变量。
    pub environment: HashMap<String, String>,
}

impl Default for PluginOverride {
    fn default() -> Self {
        Self {
            enabled: true,
            allow_unsigned: None,
            max_memory_mb: None,
            allowed_permissions: Vec::new(),
            blocked_permissions: Vec::new(),
            loopback_only: None,
            environment: HashMap::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config() {
        let config = PluginsConfig::default();
        assert!(config.enabled);
        assert!(!config.hot_reload);
        assert!(config.ask_permission_confirmation);
        assert!(config.allow_unsigned);
        assert!(config.allowed_permissions.is_empty());
        assert!(config.blocked_permissions.is_empty());
        assert!(!config.inherit_env);
        assert!(!config.loopback_only);
        assert_eq!(config.max_memory_mb, None);
        assert!(config.overrides.is_empty());
        assert!(config.verify_signatures);
    }

    #[test]
    fn parse_toml_overrides() {
        let toml_str = r#"
            enabled = true
            hot_reload = true
            ask_permission_confirmation = false
            allow_unsigned = false
            allowed_permissions = ["fs:read:data"]
            blocked_permissions = ["network:outbound"]
            inherit_env = true
            loopback_only = true
            max_memory_mb = 256
            verify_signatures = false

            [overrides.my_plugin]
            enabled = false
            allow_unsigned = true
            max_memory_mb = 128
            allowed_permissions = ["network:tcp"]
            blocked_permissions = ["fs:write:data"]
            loopback_only = false

            [overrides.my_plugin.environment]
            API_KEY = "12345"
        "#;

        let config: PluginsConfig = toml::from_str(toml_str).unwrap();
        assert!(config.enabled);
        assert!(config.hot_reload);
        assert!(!config.ask_permission_confirmation);
        assert!(!config.allow_unsigned);
        assert_eq!(config.allowed_permissions, vec!["fs:read:data"]);
        assert_eq!(config.blocked_permissions, vec!["network:outbound"]);
        assert!(config.inherit_env);
        assert!(config.loopback_only);
        assert_eq!(config.max_memory_mb, Some(256));
        assert!(!config.verify_signatures);

        let override_cfg = config.overrides.get("my_plugin").unwrap();
        assert!(!override_cfg.enabled);
        assert_eq!(override_cfg.allow_unsigned, Some(true));
        assert_eq!(override_cfg.max_memory_mb, Some(128));
        assert_eq!(override_cfg.allowed_permissions, vec!["network:tcp"]);
        assert_eq!(override_cfg.blocked_permissions, vec!["fs:write:data"]);
        assert_eq!(override_cfg.loopback_only, Some(false));
        assert_eq!(override_cfg.environment.get("API_KEY").unwrap(), "12345");
    }
}
