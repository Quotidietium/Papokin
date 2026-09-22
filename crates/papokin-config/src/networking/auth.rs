use papokin_util::ProfileAction;
use serde::{Deserialize, Serialize};

/// 服务器身份验证的配置。
///
/// 处理 Mojang 身份验证、代理限制、玩家资料和皮肤纹理。
#[derive(Deserialize, Serialize, Clone)]
#[serde(default)]
pub struct AuthenticationConfig {
    /// 是否使用 Mojang 身份验证。
    pub enabled: bool,
    /// 可选的自定义身份验证 URL。
    pub url: Option<String>,
    /// 主服务器/官方服务器宕机时使用的备用认证服务器 URL。
    #[serde(alias = "fallback_urls")]
    pub fallbacks: Vec<String>,
    /// 可选的按用户名查询档案的自定义 URL（模板参数 `{username}`）。
    pub profile_by_name_url: Option<String>,
    /// 可选的按用户名查询档案的回退 URL。
    #[serde(alias = "profile_by_name_fallback_urls")]
    pub profile_by_name_fallbacks: Vec<String>,
    /// 可选的按 UUID 查询档案的自定义 URL（模板参数 `{uuid}`）。
    pub profile_by_uuid_url: Option<String>,
    /// 可选的按 UUID 查询档案的回退 URL。
    #[serde(alias = "profile_by_uuid_fallback_urls")]
    pub profile_by_uuid_fallbacks: Vec<String>,
    /// 连接超时（毫秒）。
    pub connect_timeout: u32,
    /// 读取超时时间（毫秒）。
    pub read_timeout: u32,
    /// 是否阻止通过代理的连接。
    pub prevent_proxy_connections: bool,
    /// 在阻止代理连接时使用的可选身份验证 URL。
    pub prevent_proxy_connection_auth_url: Option<String>,
    /// 公共服务 URL（由 Drasl 和 Mojang 使用）。
    pub services_url: Option<String>,
    /// 玩家档案处理。
    pub player_profile: PlayerProfileConfig,
    /// 纹理处理配置。
    pub textures: TextureConfig,
}

impl Default for AuthenticationConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            prevent_proxy_connections: false,
            player_profile: PlayerProfileConfig::default(),
            textures: TextureConfig::default(),
            url: None,
            fallbacks: Vec::new(),
            profile_by_name_url: None,
            profile_by_name_fallbacks: Vec::new(),
            profile_by_uuid_url: None,
            profile_by_uuid_fallbacks: Vec::new(),
            prevent_proxy_connection_auth_url: None,
            services_url: None,
            connect_timeout: 5000,
            read_timeout: 5000,
        }
    }
}

/// 玩家档案处理的配置。
///
/// 控制是否允许被封禁的玩家，以及允许哪些档案操作。
#[derive(Deserialize, Serialize, Clone)]
#[serde(default)]
pub struct PlayerProfileConfig {
    /// 允许被 Mojang 标记的玩家（例如被封禁、被强制改名）。
    pub allow_banned_players: bool,
    /// 取决于 [`PlayerProfileConfig::allow_banned_players`]。
    pub allowed_actions: Vec<ProfileAction>,
}

impl Default for PlayerProfileConfig {
    fn default() -> Self {
        Self {
            allow_banned_players: false,
            allowed_actions: vec![
                ProfileAction::ForcedNameChange,
                ProfileAction::UsingBannedSkin,
            ],
        }
    }
}

/// 玩家材质的配置。
///
/// 控制是否应用材质、允许的 URL 协议/域名，以及材质类型。
#[derive(Deserialize, Serialize, Clone)]
#[serde(default)]
pub struct TextureConfig {
    /// 是否使用玩家材质。
    pub enabled: bool,
    /// 材质 URL 允许使用的 URL 协议。
    pub allowed_url_schemes: Vec<String>,
    /// 材质 URL 允许使用的 URL 域。
    pub allowed_url_domains: Vec<String>,
    /// 具体的纹理类型。
    pub types: TextureTypes,
}

impl Default for TextureConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            allowed_url_schemes: vec!["http".into(), "https".into()],
            allowed_url_domains: vec![".minecraft.net".into(), ".mojang.com".into()],
            types: TextureTypes::default(),
        }
    }
}

/// 指定支持哪些玩家纹理类型。
#[derive(Deserialize, Serialize, Clone)]
#[serde(default)]
pub struct TextureTypes {
    /// 使用玩家皮肤。
    pub skin: bool,
    /// 使用玩家披风。
    pub cape: bool,
    /// 使用玩家鞘翅。
    pub elytra: bool,
}

impl Default for TextureTypes {
    fn default() -> Self {
        Self {
            skin: true,
            cape: true,
            elytra: true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::AuthenticationConfig;

    #[test]
    fn auth_config_fallbacks_toml_deserialization() {
        let toml_str = r#"
enabled = true
url = "https://primary.auth/hasJoined?username={username}&serverId={server_hash}"
fallbacks = [
    "https://fallback1.auth/hasJoined?username={username}&serverId={server_hash}",
    "https://fallback2.auth/hasJoined?username={username}&serverId={server_hash}"
]
"#;
        let config: AuthenticationConfig = toml::from_str(toml_str).expect("配置应当能反序列化");
        assert_eq!(
            config.url.as_deref(),
            Some("https://primary.auth/hasJoined?username={username}&serverId={server_hash}")
        );
        assert_eq!(config.fallbacks.len(), 2);
        assert_eq!(
            config.fallbacks[0],
            "https://fallback1.auth/hasJoined?username={username}&serverId={server_hash}"
        );
    }

    #[test]
    fn auth_config_fallback_urls_alias_toml_deserialization() {
        let toml_str = r#"
enabled = true
fallback_urls = [
    "https://fallback1.auth/hasJoined?username={username}&serverId={server_hash}"
]
"#;
        let config: AuthenticationConfig =
            toml::from_str(toml_str).expect("带别名的配置应当能反序列化");
        assert_eq!(config.fallbacks.len(), 1);
        assert_eq!(
            config.fallbacks[0],
            "https://fallback1.auth/hasJoined?username={username}&serverId={server_hash}"
        );
    }

    #[test]
    fn auth_config_profile_urls_toml_deserialization() {
        let toml_str = r#"
enabled = true
profile_by_name_url = "https://custom.auth/users/profiles/minecraft/{username}"
profile_by_name_fallback_urls = [
    "https://fallback.auth/users/profiles/minecraft/{username}"
]
profile_by_uuid_url = "https://custom.auth/session/minecraft/profile/{uuid}?unsigned=false"
profile_by_uuid_fallback_urls = [
    "https://fallback.auth/session/minecraft/profile/{uuid}?unsigned=false"
]
"#;
        let config: AuthenticationConfig =
            toml::from_str(toml_str).expect("profile 配置应当能反序列化");
        assert_eq!(
            config.profile_by_name_url.as_deref(),
            Some("https://custom.auth/users/profiles/minecraft/{username}")
        );
        assert_eq!(config.profile_by_name_fallbacks.len(), 1);
        assert_eq!(
            config.profile_by_uuid_url.as_deref(),
            Some("https://custom.auth/session/minecraft/profile/{uuid}?unsigned=false")
        );
        assert_eq!(config.profile_by_uuid_fallbacks.len(), 1);
    }
}
