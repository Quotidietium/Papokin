use crate::{entity::player::ChatMode, server::Server};
use arc_swap::ArcSwap;
use std::{
    net::SocketAddr,
    num::NonZero,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
};

use papokin_data::translation;
use papokin_protocol::Property;
use papokin_util::{Hand, ProfileAction, text::TextComponent};
use serde::{Deserialize, Deserializer};
use sha1::Digest;
use sha2::Sha256;

use thiserror::Error;
use uuid::Uuid;
pub mod authentication;
pub mod chat;
pub mod chunk_sender;
pub use chunk_sender::ChunkSender;
pub mod java;
pub mod lan_broadcast;
pub mod packet_limiter;
pub use packet_limiter::PacketRateLimiter;
mod proxy;
pub mod query;
pub mod rcon;

#[derive(Deserialize, Debug)]
pub struct GameProfile {
    pub id: Uuid,
    pub name: String,
    #[serde(deserialize_with = "from_vec")]
    pub properties: ArcSwap<Vec<Property>>,
    #[serde(rename = "profileActions")]
    pub profile_actions: Option<Vec<ProfileAction>>,
}

impl Clone for GameProfile {
    fn clone(&self) -> Self {
        Self {
            id: self.id,
            name: self.name.clone(),
            properties: ArcSwap::new(self.properties.load().clone()),
            profile_actions: self.profile_actions.clone(),
        }
    }
}

fn from_vec<'de, D>(deserializer: D) -> Result<ArcSwap<Vec<Property>>, D::Error>
where
    D: Deserializer<'de>,
{
    let v = Vec::<Property>::deserialize(deserializer)?;
    Ok(ArcSwap::new(Arc::new(v)))
}

pub fn offline_uuid(username: &str) -> Result<Uuid, uuid::Error> {
    Uuid::from_slice(&Sha256::digest(username)[..16])
}

/// 表示玩家的配置设置。
///
/// 此结构体包含玩家可自定义的各种选项，影响其游戏体验。
///
/// **用法：**
///
/// 此结构体通常用于存储和管理玩家的偏好设置。玩家加入时或更改设置时会将其发送到服务器。
#[derive(Clone)]
pub struct PlayerConfig {
    /// 玩家的首选语言。
    pub locale: String, // 16
    /// 渲染区块的最大距离。
    pub view_distance: NonZero<u8>,
    /// 玩家的聊天模式设置
    pub chat_mode: ChatMode,
    /// 是否启用聊天颜色。
    pub chat_colors: bool,
    /// 玩家的皮肤配置选项。
    pub skin_parts: u8,
    /// 玩家的主手（左或右）。
    pub main_hand: Hand,
    /// 是否启用文本过滤。
    pub text_filtering: bool,
    /// 玩家是否希望出现在服务器列表中。
    pub server_listing: bool,
}

impl Default for PlayerConfig {
    fn default() -> Self {
        Self {
            locale: "en_us".to_string(),
            view_distance: NonZero::new(8).unwrap_or(NonZero::<u8>::MIN),
            chat_mode: ChatMode::Enabled,
            chat_colors: true,
            skin_parts: 0x7F,
            main_hand: Hand::Right,
            text_filtering: false,
            server_listing: false,
        }
    }
}

pub enum PacketHandlerResult {
    Stop,
    ReadyToPlay(GameProfile, PlayerConfig),
}

/// 客户端可排队的负载字节上限，超过后将被视为停滞/溢出并断开连接。
pub const MAX_PENDING_BYTES: usize = 64 * 1024 * 1024; // 64 MB

/// 以防御性方式递减原子的待处理字节计数器而不发生下溢。
#[inline]
pub fn decrement_pending_bytes(pending_bytes: &AtomicUsize, bytes: usize) {
    let _ = pending_bytes.fetch_update(Ordering::Release, Ordering::Relaxed, |val| {
        Some(val.saturating_sub(bytes))
    });
}

#[allow(clippy::too_many_lines)]
pub async fn can_not_join(
    profile: &GameProfile,
    address: &SocketAddr,
    server: &Server,
) -> Option<TextComponent> {
    const FORMAT_DESCRIPTION: &[time::format_description::FormatItem<'static>] = time::macros::format_description!(
        "[year]-[month]-[day] at [hour]:[minute]:[second] [offset_hour sign:mandatory]:[offset_minute]"
    );

    // 插件管理器通过 `Arc<Server>` 分发；可以从任意
    // 已加载的世界（世界持有指向服务器的弱反向引用）。
    let server_arc = server_arc(server);

    let banned_player_reason = {
        let mut banned_players = server
            .data
            .banned_player_list
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        banned_players.get_entry(profile).map(|entry| {
            let text = TextComponent::translate(
                translation::java::MULTIPLAYER_DISCONNECT_BANNED_REASON,
                [TextComponent::text(entry.reason.clone())],
            );
            match entry.expires {
                Some(expires) => text.add_child(TextComponent::translate(
                    translation::java::MULTIPLAYER_DISCONNECT_BANNED_EXPIRATION,
                    [TextComponent::text(
                        expires.format(FORMAT_DESCRIPTION).unwrap_or_default(),
                    )],
                )),
                None => text,
            }
        })
    };
    if let Some(reason) = banned_player_reason {
        if let Some(server_arc) = &server_arc {
            // 插件可通过允许登录来覆盖该拒绝。
            if let Some(kick_message) =
                validate_login_with_plugins(server_arc, address, reason).await
            {
                return Some(kick_message);
            }
        } else {
            return Some(reason);
        }
    }

    if server.white_list.load(Ordering::Relaxed) {
        let vanilla_allowed = {
            let ops = server
                .data
                .operator_config
                .read()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            let whitelist = server
                .data
                .whitelist_config
                .read()
                .unwrap_or_else(std::sync::PoisonError::into_inner);

            ops.get_entry(&profile.id).is_some() || whitelist.is_whitelisted(profile)
        };

        let not_whitelisted_reason = || {
            TextComponent::translate(
                translation::java::MULTIPLAYER_DISCONNECT_NOT_WHITELISTED,
                &[],
            )
        };

        let denied_reason = if let Some(server_arc) = &server_arc {
            let mut verify_event =
                crate::plugin::api::events::server::profile_whitelist_verify::ProfileWhitelistVerifyEvent::new(
                    profile.id,
                    profile.name.clone(),
                    not_whitelisted_reason(),
                    if vanilla_allowed {
                        crate::plugin::api::events::server::profile_whitelist_verify::WhitelistVerifyResult::Allowed
                    } else {
                        crate::plugin::api::events::server::profile_whitelist_verify::WhitelistVerifyResult::Denied
                    },
                );
            server_arc
                .plugin_manager
                .fire(server_arc, &mut verify_event)
                .await;
            match verify_event.result {
                crate::plugin::api::events::server::profile_whitelist_verify::WhitelistVerifyResult::Allowed => None,
                crate::plugin::api::events::server::profile_whitelist_verify::WhitelistVerifyResult::Denied => {
                    Some(verify_event.kick_message)
                }
            }
        } else if vanilla_allowed {
            None
        } else {
            Some(not_whitelisted_reason())
        };

        if let Some(reason) = denied_reason {
            if let Some(server_arc) = &server_arc {
                // 插件可通过允许登录来覆盖该拒绝。
                if let Some(kick_message) =
                    validate_login_with_plugins(server_arc, address, reason).await
                {
                    return Some(kick_message);
                }
            } else {
                return Some(reason);
            }
        }
    }

    let banned_ip_reason = {
        let mut banned_ips = server
            .data
            .banned_ip_list
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        banned_ips.get_entry(&address.ip()).map(|entry| {
            let text = TextComponent::translate(
                translation::java::MULTIPLAYER_DISCONNECT_BANNED_IP_REASON,
                [TextComponent::text(entry.reason.clone())],
            );
            match entry.expires {
                Some(expires) => text.add_child(TextComponent::translate(
                    translation::java::MULTIPLAYER_DISCONNECT_BANNED_IP_EXPIRATION,
                    [TextComponent::text(
                        expires.format(FORMAT_DESCRIPTION).unwrap_or_default(),
                    )],
                )),
                None => text,
            }
        })
    };
    if let Some(reason) = banned_ip_reason {
        if let Some(server_arc) = &server_arc {
            // 插件可通过允许登录来覆盖该拒绝。
            if let Some(kick_message) =
                validate_login_with_plugins(server_arc, address, reason).await
            {
                return Some(kick_message);
            }
        } else {
            return Some(reason);
        }
    }

    None
}

/// 通过任意已加载的世界为 `&Server` 找回对应的 `Arc<Server>`
/// (世界持有指向其服务器的弱引用)。仅当
/// 在尚未加载任何世界时；而一旦建立连接后这不可能发生
/// 已接受。
pub(crate) fn server_arc(server: &Server) -> Option<Arc<Server>> {
    server
        .worlds
        .load()
        .first()
        .and_then(|world| world.server.upgrade())
}

/// 正在连接的玩家是否没有已保存的玩家数据，即首次
/// 首次。
pub(crate) fn is_first_join(server: &Server, uuid: &Uuid) -> bool {
    server
        .player_data_storage
        .load_data(uuid)
        .ok()
        .flatten()
        .is_none()
}

/// 触发 [`PlayerConnectionValidateLoginEvent`](crate::plugin::api::events::server::player_connection_validate_login::PlayerConnectionValidateLoginEvent)
/// 用于被拒绝的登录并报告最终裁决：`Some(kick_message)`
/// 当登录仍被拒绝时（可能带有插件修改后的消息），
/// 当插件允许登录时返回 `None`。
async fn validate_login_with_plugins(
    server: &Arc<Server>,
    address: &SocketAddr,
    reason: TextComponent,
) -> Option<TextComponent> {
    use crate::plugin::api::events::server::player_connection_validate_login::{
        ConnectionValidationResult, PlayerConnectionValidateLoginEvent,
    };

    let mut event = PlayerConnectionValidateLoginEvent::new(
        address.ip().to_string(),
        reason,
        ConnectionValidationResult::Denied,
    );
    server.plugin_manager.fire(server, &mut event).await;
    match event.result {
        ConnectionValidationResult::Denied => Some(event.kick_message),
        ConnectionValidationResult::Allowed => None,
    }
}

#[derive(Error, Debug)]
pub enum EncryptionError {
    #[error("解密共享密钥失败")]
    FailedDecrypt,
    #[error("共享密钥长度错误")]
    SharedWrongLength,
    #[error("加密已启用")]
    AlreadyEncrypted,
    #[error("没有待处理的加密请求")]
    NoPendingVerifyToken,
    #[error("验证令牌不匹配")]
    VerifyTokenMismatch,
}

fn is_valid_player_name(name: &str) -> bool {
    if name.len() > 16 {
        return false;
    }
    !name.chars().any(|c| c.is_control() || c == ' ')
}

#[cfg(test)]
mod tests {
    use crate::net::is_valid_player_name;

    /// 测试用例：最大长度的标准合法英文名称。
    #[test]
    fn valid_max_length_ascii() {
        let name = "player_name_1234"; // 16 个字符（16 字节）
        assert!(
            is_valid_player_name(name),
            "Max length ASCII name should be valid"
        );
    }

    /// 测试用例：较短的合法 ASCII 名称。
    #[test]
    fn valid_short_ascii() {
        let name = "GamerX";
        assert!(
            is_valid_player_name(name),
            "Short ASCII name should be valid"
        );
    }

    /// 测试用例：包含允许标点符号的名称（码位 33-126）。
    #[test]
    fn valid_with_punctuation() {
        let name = "!-@#$%.^&*_+-=";
        assert!(
            is_valid_player_name(name),
            "Name with valid punctuation should be valid"
        );
    }

    /// 测试用例：允许的高码位 Unicode 字符（如中文/CJK）。
    #[test]
    fn valid_unicode_chinese() {
        let name = "玩家一号"; // 4 个字符，12 字节
        assert!(
            is_valid_player_name(name),
            "Chinese characters should be valid"
        );
    }

    /// 测试用例：混合有效的 ASCII 与 Unicode 字符。
    #[test]
    fn valid_mixed_chars() {
        let name = "Player_玩家"; // 9 个字符
        assert!(
            is_valid_player_name(name),
            "Mixed ASCII and Unicode should be valid"
        );
    }

    /// 测试用例：超过 16 字节限制的名称（ASCII）。
    #[test]
    fn invalid_length_ascii_over() {
        let name = "this_name_is_too_long"; // 21 个字符（21 字节）
        assert!(
            !is_valid_player_name(name),
            "Name over 16 bytes (ASCII) should be invalid"
        );
    }

    /// 测试用例：超过 16 字节限制的名称（Unicode）。
    #[test]
    fn invalid_length_unicode_over() {
        let name = "超长玩家名称哈哈"; // 8 个汉字 * 每字符 3 字节 = 24 字节
        assert!(
            !is_valid_player_name(name),
            "Name over 16 bytes (Unicode) should be invalid by byte count"
        );
    }

    /// 测试用例：包含标准空格符的名称（码位 32）。
    #[test]
    fn invalid_contains_space() {
        let name = "Player Name";
        assert!(
            !is_valid_player_name(name),
            "Name containing a space should be invalid"
        );
    }

    /// 测试用例：空字符串（长度 0，为完整性而包含）。
    #[test]
    fn invalid_empty_string() {
        let name = "";
        assert!(
            is_valid_player_name(name),
            "Empty string should be valid (length <= 16 and no invalid chars)"
        );
    }

    /// 测试用例：包含控制字符的名称（例如 Null，码位 0）。
    #[test]
    fn invalid_contains_null() {
        let name = "Player\0Name";
        assert!(
            !is_valid_player_name(name),
            "Name containing a null character should be invalid"
        );
    }

    /// 测试用例：包含换行符的名称（码位 10）。
    #[test]
    fn invalid_contains_newline() {
        let name = "Player\nName";
        assert!(
            !is_valid_player_name(name),
            "Name containing a newline should be invalid"
        );
    }

    /// 测试用例：包含 DEL 控制字符的名称（码位 127）。
    #[test]
    fn invalid_contains_del() {
        // DEL 字符即 char::from_u32(127).unwrap()
        let name = format!("Player{}Name", 127u8 as char);
        assert!(
            !is_valid_player_name(&name),
            "Name containing DEL (127) should be invalid"
        );
    }

    #[test]
    fn decrement_pending_bytes_saturating() {
        use super::decrement_pending_bytes;
        use std::sync::atomic::{AtomicUsize, Ordering};

        let counter = AtomicUsize::new(100);
        decrement_pending_bytes(&counter, 40);
        assert_eq!(counter.load(Ordering::Relaxed), 60);

        // 下溢保护：递减超过当前值时应钳制为 0
        decrement_pending_bytes(&counter, 100);
        assert_eq!(counter.load(Ordering::Relaxed), 0);
    }
}
