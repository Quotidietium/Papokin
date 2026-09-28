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
pub use chunk_sender::{ChunkSender, EncodedChunk};
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

    // 关停中拒绝新登录：/stop 后、任务等待超时（60s）前已被 accept
    // 的连接仍可完成登录入世——此时 save_all_players 与 kick-all 均
    // 已跑完，迟到玩家的会话只能靠其自身断开路径保存，进程随后
    // 退出即整段进度丢失。
    if crate::SERVER_IS_STOPPING.load(Ordering::Acquire) {
        return Some(TextComponent::text("服务器正在关闭，请稍后再试"));
    }

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
    if name.is_empty() || name.len() > 16 {
        return false;
    }
    !name
        .chars()
        .any(|c| c.is_control() || c == ' ' || is_text_injection_char(c))
}

/// 判定用户名中的文本注入字符：Minecraft 格式代码 `§`，以及零宽
/// 与双向控制字符（U+200B-200F、U+2028-202E、U+2060-206F、软连字符）。
/// 前者可向聊天/tab 列表注入彩色与加粗，后者可制造视觉同名玩家。
/// 其余 Unicode（含中文）是本项目既定允许的用户名字符集。
const fn is_text_injection_char(c: char) -> bool {
    matches!(c as u32,
        0x00A7            // § 格式代码
        | 0x00AD          // 软连字符
        | 0x200B..=0x200F // 零宽空格 + LRM/RLM
        | 0x2028..=0x202E // 行/段分隔 + 双向覆盖/嵌入
        | 0x2060..=0x206F // 词连接符 + 双向隔离符
    )
}

/// 事件/日志用的握手地址清洗。
///
/// 握手包的 `server_address` 是完全不可信的原始串（最长 32767
/// 字符）：BungeeCord 转发把 `host\0ip\0uuid` 负载塞在这里，普通
/// 客户端可塞任意控制字符与注入字符。BungeeCord 解析路径必须用
/// 原始串，本函数只用于把它交给插件事件（`PlayerHandshakeEvent`、
/// `ServerListPingEvent`）与日志之前：剔除控制字符与文本注入字符，
/// 并截断到 255 字符，防止日志洪泛与事件消费方被塞入超长垃圾。
pub(crate) fn sanitize_handshake_address(raw: &str) -> String {
    const MAX_EVENT_ADDRESS_CHARS: usize = 255;
    let mut cleaned: String = raw
        .chars()
        .filter(|c| !c.is_control() && !is_text_injection_char(*c))
        .take(MAX_EVENT_ADDRESS_CHARS)
        .collect();
    let raw_len = raw.chars().count();
    if raw_len > MAX_EVENT_ADDRESS_CHARS {
        cleaned.push('…');
    }
    cleaned
}

#[cfg(test)]
mod tests {
    use crate::net::{is_valid_player_name, sanitize_handshake_address};

    /// 测试用例：握手地址清洗应剔除 `BungeeCord` 负载与控制字符。
    #[test]
    fn sanitize_strips_bungeecord_payload_and_controls() {
        let raw = "localhost\u{0}127.0.0.1\u{0}some-uuid\u{0}extra";
        let cleaned = sanitize_handshake_address(raw);
        assert_eq!(cleaned, "localhost127.0.0.1some-uuidextra");
    }

    /// 测试用例：清洗应剔除文本注入字符（§ 与零宽/双向控制符）。
    #[test]
    fn sanitize_strips_injection_chars() {
        let raw = "host\u{00A7}name\u{200B}\u{202E}end";
        assert_eq!(sanitize_handshake_address(raw), "hostnameend");
    }

    /// 测试用例：超长原始串截断到 255 字符并追加省略号。
    #[test]
    fn sanitize_truncates_overlong_input() {
        let raw = "a".repeat(40_000);
        let cleaned = sanitize_handshake_address(&raw);
        assert_eq!(cleaned.chars().count(), 256); // 255 + 省略号
        assert!(cleaned.ends_with('…'));
    }

    /// 测试用例：正常短地址原样保留。
    #[test]
    fn sanitize_keeps_normal_address() {
        assert_eq!(sanitize_handshake_address("example.com"), "example.com");
    }

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

    /// 测试用例：空字符串——离线模式下会生成空名的无名玩家，
    /// 且原版客户端不可能发出，必须拒绝。
    #[test]
    fn invalid_empty_string() {
        let name = "";
        assert!(
            !is_valid_player_name(name),
            "Empty string should be invalid"
        );
    }

    /// 测试用例：包含 Minecraft 格式代码 `§` 的名称——可向聊天与
    /// tab 列表注入彩色/加粗文本。
    #[test]
    fn invalid_contains_format_code() {
        let name = "Play§rer";
        assert!(
            !is_valid_player_name(name),
            "Name containing § format code should be invalid"
        );
    }

    /// 测试用例：包含零宽/双向控制字符的名称——可制造视觉同名玩家
    /// （例如在正常名称中插入 U+200B 或 RTL 覆盖符）。
    #[test]
    fn invalid_contains_zero_width_or_bidi() {
        for name in [
            "Play\u{200B}er",
            "Play\u{202E}er",
            "Play\u{2060}er",
            "Play\u{00AD}er",
        ] {
            assert!(
                !is_valid_player_name(name),
                "Name containing zero-width/bidi control char should be invalid: {name:?}"
            );
        }
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
