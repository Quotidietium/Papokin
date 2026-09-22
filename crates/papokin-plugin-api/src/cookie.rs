//! 客户端 cookie 存储（等同于 Paper 1.20.5+ 的 `ClientCookie`
//! API）。
//!
//! Cookie 是小数据块（至多 [`MAX_COOKIE_PAYLOAD`] 字节），由服务器
//! 存储在 Java 版客户端上；客户端会跨服务器
//! 迁移保存它们，并在服务器询问时交还。典型用途是
//! 必须在代理
//! 迁移后仍保留。
//!
//! Cookie 是 Java 版协议特性。
//!
//! [`PlayerCookieExt::store_cookie`] 和 [`PlayerCookieExt::request_cookie`]
//! 发送游戏阶段的客户端方向数据包，因此只会到达完全连接的
//! （游戏内）玩家。[`PlayerCookieExt::get_cookie`] 从不发送数据包：它
//! 读取服务器端缓存的、客户端自
//! 客户端连接以来发送的响应缓存，因此一个键仅在
//! [`PlayerCookieExt::request_cookie`] 往返完成后才出现。
//!
//! # Examples
//!
//! ```rust,ignore
//! use papokin_plugin_api::cookie::PlayerCookieExt;
//!
//! fn on_join(player: &papokin_plugin_api::Player) {
//!     // Ask the client for the session token it kept from the lobby server.
//!     player.request_cookie("myplugin:session")?;
//!     // ...one the response round trip has completed:
//!     if let Some(token) = player.get_cookie("myplugin:session") {
//!         // validate the token...
//!     }
//!     // Persist a preference on the client for future sessions.
//!     player.store_cookie("myplugin:locale", b"en_us")?;
//! }
//! ```

pub use crate::wit::papokin::plugin::cookie::{
    clear_cookie, get_cookie, request_cookie, store_cookie,
};

use crate::wit::papokin::plugin::player::Player;

/// cookie 负载的最大字节数（原版 5 KiB 上限）。更大的
/// 负载会被宿主拒绝而非被截断。
pub const MAX_COOKIE_PAYLOAD: usize = 5120;

/// 检查 `key` 是否为合法的 cookie 资源标识符：
/// `namespace:path`，或纯路径（原版将缺失的命名空间视为
/// `minecraft`）。命名空间允许 `a-z 0-9 _ . -`，路径额外允许 `/`。
/// 这与宿主侧的验证保持一致，因此通过此处检查的键不会被
/// 被宿主拒绝。
#[must_use]
pub fn is_valid_cookie_key(key: &str) -> bool {
    let (namespace, path) = key
        .split_once(':')
        .map_or(("minecraft", key), |(ns, p)| (ns, p));
    !path.is_empty()
        && namespace
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, '_' | '.' | '-'))
        && path.chars().all(|c| {
            c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, '_' | '.' | '-' | '/')
        })
}

/// [`Player`] 上的客户端 cookie 存储扩展 trait（Paper 的
/// `ClientCookie` API 的对应实现）。
pub trait PlayerCookieExt {
    /// 在玩家客户端上存储 cookie。
    ///
    /// # Errors
    ///当 `key` 不是有效标识符或 `payload` 无效时，返回错误
    /// 超过 [`MAX_COOKIE_PAYLOAD`] 字节。
    fn store_cookie(&self, key: &str, payload: &[u8]) -> Result<(), String>;

    /// 向玩家的客户端请求存储在 `key` 下的 cookie。
    /// 响应会异步到达，并可通过
    /// [`PlayerCookieExt::get_cookie`]。
    ///
    /// # Errors
    ///当 `key` 不是有效标识符时，返回错误。
    fn request_cookie(&self, key: &str) -> Result<(), String>;

    ///返回 cookie `key` 的缓存值；当其尚未缓存时返回 `None`
    /// 客户端尚未上报。从不发送数据包。
    fn get_cookie(&self, key: &str) -> Option<Vec<u8>>;

    /// 清除 cookie `key`：将其从服务器端缓存中丢弃，并
    /// 会用空负载覆盖客户端的副本（原版没有
    /// 专用的删除数据包）。
    ///
    /// # Errors
    ///当 `key` 不是有效标识符时，返回错误。
    fn clear_cookie(&self, key: &str) -> Result<(), String>;
}

impl PlayerCookieExt for Player {
    fn store_cookie(&self, key: &str, payload: &[u8]) -> Result<(), String> {
        store_cookie(self, key, payload)
    }

    fn request_cookie(&self, key: &str) -> Result<(), String> {
        request_cookie(self, key)
    }

    fn get_cookie(&self, key: &str) -> Option<Vec<u8>> {
        get_cookie(self, key)
    }

    fn clear_cookie(&self, key: &str) -> Result<(), String> {
        clear_cookie(self, key)
    }
}
