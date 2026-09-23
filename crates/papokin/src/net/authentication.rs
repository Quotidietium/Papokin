use std::{collections::HashMap, net::IpAddr, sync::Arc};

use base64::{Engine, engine::general_purpose};
use papokin_config::{AuthenticationConfig, networking::auth::TextureConfig};
use papokin_protocol::Property;
use reqwest::{StatusCode, Url};
use rsa::RsaPublicKey;
use rsa::pkcs8::DecodePublicKey;
use serde::Deserialize;
use thiserror::Error;
use uuid::Uuid;

use super::GameProfile;
use crate::plugin::api::events::player::{
    fill_profile::FillProfileEvent, lookup_profile::LookupProfileEvent,
    pre_fill_profile::PreFillProfileEvent, pre_lookup_profile::PreLookupProfileEvent,
};
use crate::server::Server;

#[derive(Deserialize, Clone, Debug)]
#[expect(dead_code)]
#[serde(rename_all = "camelCase")]
pub struct ProfileTextures {
    timestamp: i64,
    profile_id: Uuid,
    profile_name: String,
    // Mojang 总会发送此内容，但第三方认证服务器（drasl、Blessing Skin 等）
    // 省略它。它在此处未被使用，因此采用默认值而不是让玩家资料解析失败。
    #[serde(default)]
    signature_required: bool,
    textures: HashMap<String, Texture>,
}

#[derive(Deserialize, Clone, Debug)]
#[expect(dead_code)]
pub struct Texture {
    url: String,
    metadata: Option<HashMap<String, String>>,
}

#[derive(Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct JsonPublicKey {
    pub public_key: String,
}
#[derive(Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct MojangPublicKeys {
    pub profile_property_keys: Vec<JsonPublicKey>,
    pub player_certificate_keys: Vec<JsonPublicKey>,
    pub authentication_keys: Option<Vec<JsonPublicKey>>,
}

const MOJANG_AUTHENTICATION_URL: &str = "https://sessionserver.mojang.com/session/minecraft/hasJoined?username={username}&serverId={server_hash}";
const MOJANG_PREVENT_PROXY_AUTHENTICATION_URL: &str = "https://sessionserver.mojang.com/session/minecraft/hasJoined?username={username}&serverId={server_hash}&ip={ip}";
const MOJANG_SERVICES_URL: &str = "https://api.minecraftservices.com/";
const MOJANG_PROFILE_BY_NAME_URL: &str =
    "https://api.mojang.com/users/profiles/minecraft/{username}";
const MOJANG_PROFILE_BY_UUID_URL: &str =
    "https://sessionserver.mojang.com/session/minecraft/profile/{uuid}?unsigned=false";

fn create_client(auth_config: &AuthenticationConfig) -> reqwest::Client {
    crate::http_client::client_builder()
        .connect_timeout(std::time::Duration::from_millis(
            auth_config.connect_timeout as u64,
        ))
        .timeout(std::time::Duration::from_millis(
            auth_config.read_timeout as u64,
        ))
        .build()
        .unwrap_or_default()
}

/// 百分号编码用户名（RFC 3986 非保留字符集之外全部转义）。
///
/// 玩家名来自不可信的登录包，可能包含 `&`/`?`/`#`/`/` 等
/// URL 结构字符；直接替换进模板会污染认证服务器的查询串
/// 甚至路径，因此必须先编码（查询串与路径位置均安全）。
fn percent_encode_username(username: &str) -> String {
    let mut encoded = String::with_capacity(username.len());
    for byte in username.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => {
                encoded.push(char::from(byte));
            }
            _ => {
                let _ = std::fmt::Write::write_fmt(&mut encoded, format_args!("%{byte:02X}"));
            }
        }
    }
    encoded
}

fn format_auth_url(url_template: &str, username: &str, server_hash: &str, ip: &IpAddr) -> String {
    url_template
        .replace("{username}", &percent_encode_username(username))
        .replace("{server_hash}", server_hash)
        .replace("{ip}", &ip.to_string())
}

/// 向 Mojang 的（或自定义/备用）认证服务器发送 GET 请求，以验证客户端的 Minecraft 账户。
///
/// **目的：**
///
/// 此函数用于确保连接到服务器的客户端拥有有效的正版 Minecraft 账户。
/// 如果主认证服务器宕机或不可达，则回退尝试已配置的备用服务器。
///
/// **工作原理：**
///
/// 1. 拥有正版账户的客户端向会话服务器发送登录请求。
/// 2. 会话服务器验证客户端的凭据，并将该玩家加入其服务器会话列表。
/// 3. Pumpkin 尝试向主认证服务器验证玩家身份，若主服务器不可用则回退到辅助认证服务器。
///
/// 参见 <https://pumpkinmc.org/developer/networking/authentication>
pub async fn authenticate(
    username: &str,
    server_hash: &str,
    ip: &IpAddr,
    auth_config: &AuthenticationConfig,
) -> Result<GameProfile, AuthError> {
    let primary_url = if auth_config.prevent_proxy_connections {
        auth_config
            .prevent_proxy_connection_auth_url
            .as_deref()
            .unwrap_or(MOJANG_PREVENT_PROXY_AUTHENTICATION_URL)
    } else {
        auth_config
            .url
            .as_deref()
            .unwrap_or(MOJANG_AUTHENTICATION_URL)
    };

    let mut candidate_urls = Vec::with_capacity(1 + auth_config.fallbacks.len());
    candidate_urls.push(primary_url);
    for fallback in &auth_config.fallbacks {
        candidate_urls.push(fallback.as_str());
    }

    let client = create_client(auth_config);

    let mut unverified_count = 0;
    let mut last_unknown_status = None;

    for url_template in candidate_urls {
        let address = format_auth_url(url_template, username, server_hash, ip);

        let response = match client.get(&address).send().await {
            Ok(resp) => resp,
            Err(err) => {
                tracing::warn!("'{address}' 处的认证服务器已宕机或不可达：{err}");
                continue;
            }
        };

        let status = response.status();
        if status.is_server_error() {
            tracing::warn!("'{address}' 处的认证服务器返回了服务器错误：{status}");
            continue;
        }

        match status {
            StatusCode::OK => match response.json::<GameProfile>().await {
                Ok(profile) => return Ok(profile),
                Err(err) => {
                    tracing::warn!("解析来自 '{address}' 的 GameProfile 响应失败：{err}");
                }
            },
            StatusCode::NO_CONTENT => {
                unverified_count += 1;
            }
            other => {
                last_unknown_status = Some(other);
            }
        }
    }

    if unverified_count > 0 {
        Err(AuthError::UnverifiedUsername)
    } else if let Some(status) = last_unknown_status {
        Err(AuthError::UnknownStatusCode(status))
    } else {
        Err(AuthError::FailedResponse)
    }
}

pub fn validate_textures(property: &Property, config: &TextureConfig) -> Result<(), TextureError> {
    let from64 = general_purpose::STANDARD
        .decode(property.value.as_bytes())
        .map_err(|e| TextureError::DecodeError(e.to_string()))?;
    let textures: ProfileTextures =
        serde_json::from_slice(&from64).map_err(|e| TextureError::JSONError(e.to_string()))?;
    for texture in textures.textures {
        let url = Url::parse(&texture.1.url).map_err(|_| TextureError::InvalidURL)?;
        is_texture_url_valid(&url, config)?;
    }
    Ok(())
}

pub fn is_texture_url_valid(url: &Url, config: &TextureConfig) -> Result<(), TextureError> {
    let scheme = url.scheme();
    if !config
        .allowed_url_schemes
        .iter()
        .any(|allowed_scheme| scheme.ends_with(allowed_scheme))
    {
        return Err(TextureError::DisallowedUrlScheme(scheme.to_string()));
    }
    let Some(domain) = url.domain() else {
        return Err(TextureError::InvalidURL);
    };
    if !config
        .allowed_url_domains
        .iter()
        .any(|allowed_domain| domain.ends_with(allowed_domain))
    {
        return Err(TextureError::DisallowedUrlDomain(domain.to_string()));
    }
    Ok(())
}

pub async fn fetch_mojang_public_keys(
    auth_config: &AuthenticationConfig,
) -> Result<Vec<RsaPublicKey>, AuthError> {
    let services_url = auth_config
        .services_url
        .as_deref()
        .unwrap_or(MOJANG_SERVICES_URL);

    let url = format!("{services_url}/publickeys");

    let client = create_client(auth_config);
    let response = client
        .get(&url)
        .send()
        .await
        .map_err(|_| AuthError::FailedResponse)?;

    match response.status() {
        StatusCode::OK => {}
        StatusCode::NO_CONTENT => Err(AuthError::FailedResponse)?,
        other => Err(AuthError::UnknownStatusCode(other))?,
    }

    let public_keys: MojangPublicKeys =
        response.json().await.map_err(|_| AuthError::FailedParse)?;

    let as_rsa_keys = public_keys
        .player_certificate_keys
        .into_iter()
        .map(|key| {
            let decoded_key = general_purpose::STANDARD
                .decode(key.public_key.as_bytes())
                .map_err(|_| AuthError::FailedParse)?;
            RsaPublicKey::from_public_key_der(&decoded_key).map_err(|_| AuthError::FailedParse)
        })
        .collect::<Result<Vec<_>, _>>()?;

    Ok(as_rsa_keys)
}

#[derive(Deserialize, Clone, Debug)]
struct MojangProfileByNameResponse {
    id: String,
    name: String,
}

pub async fn lookup_profile_by_name(
    name: &str,
    auth_config: &AuthenticationConfig,
    server: Option<&Arc<Server>>,
) -> Result<Option<(Uuid, String)>, AuthError> {
    // 预查询钩子：在发起任何请求之前触发的纯通知。
    if let Some(server) = server {
        let mut pre_lookup = PreLookupProfileEvent::new(name);
        server.plugin_manager.fire(server, &mut pre_lookup).await;
    }

    let primary_url = auth_config
        .profile_by_name_url
        .as_deref()
        .unwrap_or(MOJANG_PROFILE_BY_NAME_URL);

    let mut candidate_urls = Vec::with_capacity(1 + auth_config.profile_by_name_fallbacks.len());
    candidate_urls.push(primary_url);
    for fallback in &auth_config.profile_by_name_fallbacks {
        candidate_urls.push(fallback.as_str());
    }

    let client = create_client(auth_config);

    let mut not_found_count = 0;
    let mut last_unknown_status = None;

    for url_template in candidate_urls {
        let address = url_template.replace("{username}", &percent_encode_username(name));

        let response = match client.get(&address).send().await {
            Ok(resp) => resp,
            Err(err) => {
                tracing::warn!("'{address}' 处的档案查询服务器已宕机或不可达：{err}");
                continue;
            }
        };

        let status = response.status();
        if status.is_server_error() {
            tracing::warn!("'{address}' 处的档案查询服务器返回了服务器错误：{status}");
            continue;
        }

        match status {
            StatusCode::OK => match response.json::<MojangProfileByNameResponse>().await {
                Ok(profile) => {
                    let parsed_uuid =
                        Uuid::parse_str(&profile.id).map_err(|_| AuthError::FailedParse)?;
                    // 查询钩子：档案解析完成时触发。`properties`
                    // 在这里始终为空；填充步骤稍后会获取它们。
                    if let Some(server) = server {
                        let mut lookup = LookupProfileEvent::new(
                            name,
                            parsed_uuid,
                            Some(profile.name.clone()),
                            Vec::new(),
                        );
                        server.plugin_manager.fire(server, &mut lookup).await;
                    }
                    return Ok(Some((parsed_uuid, profile.name)));
                }
                Err(err) => {
                    tracing::warn!("解析来自 '{address}' 的按名称查询档案响应失败：{err}");
                }
            },
            StatusCode::NO_CONTENT | StatusCode::NOT_FOUND => {
                not_found_count += 1;
            }
            other => {
                last_unknown_status = Some(other);
            }
        }
    }

    if not_found_count > 0 {
        Ok(None)
    } else if let Some(status) = last_unknown_status {
        Err(AuthError::UnknownStatusCode(status))
    } else {
        Err(AuthError::FailedResponse)
    }
}

pub fn lookup_profile_by_name_blocking(
    name: &str,
    auth_config: &AuthenticationConfig,
    server: Option<&Arc<Server>>,
) -> Result<Option<(Uuid, String)>, AuthError> {
    if let Ok(handle) = tokio::runtime::Handle::try_current() {
        tokio::task::block_in_place(|| {
            handle.block_on(lookup_profile_by_name(name, auth_config, server))
        })
    } else {
        tokio::runtime::Runtime::new()
            .map_err(|_e| AuthError::FailedParse)?
            .block_on(lookup_profile_by_name(name, auth_config, server))
    }
}

pub async fn fetch_profile_by_uuid(
    uuid: Uuid,
    auth_config: &AuthenticationConfig,
    server: Option<&Arc<Server>>,
) -> Result<Option<GameProfile>, AuthError> {
    // 预填充钩子：处理器可用缓存数据填充 `properties`；
    // 获取仍会继续进行（宿主尚未做短路处理）。
    if let Some(server) = server {
        let mut pre_fill = PreFillProfileEvent::new(uuid, None, Vec::new());
        server.plugin_manager.fire(server, &mut pre_fill).await;
    }

    let primary_url = auth_config
        .profile_by_uuid_url
        .as_deref()
        .unwrap_or(MOJANG_PROFILE_BY_UUID_URL);

    let mut candidate_urls = Vec::with_capacity(1 + auth_config.profile_by_uuid_fallbacks.len());
    candidate_urls.push(primary_url);
    for fallback in &auth_config.profile_by_uuid_fallbacks {
        candidate_urls.push(fallback.as_str());
    }

    let client = create_client(auth_config);

    let mut not_found_count = 0;
    let mut last_unknown_status = None;

    let uuid_simple = uuid.simple().to_string();

    for url_template in candidate_urls {
        let address = url_template
            .replace("{uuid}", &uuid_simple)
            .replace("{uuid_hyphenated}", &uuid.to_string());

        let response = match client.get(&address).send().await {
            Ok(resp) => resp,
            Err(err) => {
                tracing::warn!("'{address}' 处的档案获取服务器已宕机或不可达：{err}");
                continue;
            }
        };

        let status = response.status();
        if status.is_server_error() {
            tracing::warn!("'{address}' 处的档案获取服务器返回了服务器错误：{status}");
            continue;
        }

        match status {
            StatusCode::OK => match response.json::<GameProfile>().await {
                Ok(profile) => {
                    // Fill 钩子：在档案（含属性）就绪后触发一次
                    // 已从认证服务器获取。
                    if let Some(server) = server {
                        let properties = profile
                            .properties
                            .load()
                            .iter()
                            .map(|property| (property.name.to_string(), property.value.to_string()))
                            .collect();
                        let mut fill = FillProfileEvent::new(
                            profile.id,
                            Some(profile.name.clone()),
                            properties,
                        );
                        server.plugin_manager.fire(server, &mut fill).await;
                    }
                    return Ok(Some(profile));
                }
                Err(err) => {
                    tracing::warn!("解析来自 '{address}' 的 GameProfile 响应失败：{err}");
                }
            },
            StatusCode::NO_CONTENT | StatusCode::NOT_FOUND => {
                not_found_count += 1;
            }
            other => {
                last_unknown_status = Some(other);
            }
        }
    }

    if not_found_count > 0 {
        Ok(None)
    } else if let Some(status) = last_unknown_status {
        Err(AuthError::UnknownStatusCode(status))
    } else {
        Err(AuthError::FailedResponse)
    }
}

#[derive(Error, Debug)]
pub enum AuthError {
    #[error("认证服务器已宕机")]
    FailedResponse,
    #[error("用户名验证失败")]
    UnverifiedUsername,
    #[error("你已被认证服务器封禁")]
    Banned,
    #[error("纹理错误 {0}")]
    TextureError(TextureError),
    #[error("你执行了认证服务器不允许的操作")]
    DisallowedAction,
    #[error("无法将 JSON 解析为游戏档案")]
    FailedParse,
    #[error("未知状态码 {0}")]
    UnknownStatusCode(StatusCode),
}

#[derive(Error, Debug)]
pub enum TextureError {
    #[error("无效的 URL")]
    InvalidURL,
    #[error("玩家纹理的 URL 方案无效：{0}")]
    DisallowedUrlScheme(String),
    #[error("玩家纹理的 URL 域名无效：{0}")]
    DisallowedUrlDomain(String),
    #[error("解码 base64 玩家纹理失败：{0}")]
    DecodeError(String),
    #[error("解析玩家纹理的 JSON 失败：{0}")]
    JSONError(String),
}

#[cfg(test)]
mod tests {
    use super::ProfileTextures;

    // 第三方认证服务器（drasl、Blessing Skin、littleskin.cn）不会发送
    // `signatureRequired`。档案仍必须能解析。参见 issue #301。
    #[test]
    fn parses_profile_without_signature_required() {
        let json = r#"{
            "timestamp": 0,
            "profileId": "069a79f444e94726a5befca90e38aaf5",
            "profileName": "Notch",
            "textures": {}
        }"#;
        let profile: ProfileTextures =
            serde_json::from_slice(json.as_bytes()).expect("配置文件应能解析");
        assert!(!profile.signature_required);
    }

    #[test]
    fn parses_profile_with_signature_required() {
        let json = r#"{
            "timestamp": 0,
            "profileId": "069a79f444e94726a5befca90e38aaf5",
            "profileName": "Notch",
            "signatureRequired": true,
            "textures": {}
        }"#;
        let profile: ProfileTextures =
            serde_json::from_slice(json.as_bytes()).expect("配置文件应能解析");
        assert!(profile.signature_required);
    }

    #[test]
    fn format_auth_url() {
        let template =
            "https://auth.example.com/hasJoined?username={username}&serverId={server_hash}&ip={ip}";
        let formatted = super::format_auth_url(
            template,
            "Player1",
            "hash123",
            &"127.0.0.1".parse().unwrap(),
        );
        assert_eq!(
            formatted,
            "https://auth.example.com/hasJoined?username=Player1&serverId=hash123&ip=127.0.0.1"
        );
    }

    #[test]
    fn url_encodes_structural_characters_in_username() {
        // 结构字符必须转义，玩家名不能污染查询串或路径。
        assert_eq!(super::percent_encode_username("a&b=c"), "a%26b%3Dc");
        assert_eq!(super::percent_encode_username("a?b#c"), "a%3Fb%23c");
        assert_eq!(super::percent_encode_username("a/b d"), "a%2Fb%20d");
        assert_eq!(super::percent_encode_username("Steve_99"), "Steve_99");
        assert_eq!(super::percent_encode_username("玩家"), "%E7%8E%A9%E5%AE%B6");
    }

    #[test]
    fn auth_config_fallbacks_deserialization() {
        let json_str = r#"{
            "enabled": true,
            "url": "https://primary.auth/hasJoined?username={username}&serverId={server_hash}",
            "fallbacks": [
                "https://fallback1.auth/hasJoined?username={username}&serverId={server_hash}",
                "https://fallback2.auth/hasJoined?username={username}&serverId={server_hash}"
            ]
        }"#;
        let config: papokin_config::AuthenticationConfig =
            serde_json::from_str(json_str).expect("配置应能反序列化");
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
    fn auth_config_fallback_urls_alias_deserialization() {
        let json_str = r#"{
            "enabled": true,
            "fallback_urls": [
                "https://fallback1.auth/hasJoined?username={username}&serverId={server_hash}"
            ]
        }"#;
        let config: papokin_config::AuthenticationConfig =
            serde_json::from_str(json_str).expect("带别名的配置应能反序列化");
        assert_eq!(config.fallbacks.len(), 1);
        assert_eq!(
            config.fallbacks[0],
            "https://fallback1.auth/hasJoined?username={username}&serverId={server_hash}"
        );
    }
}
