//! 客户端 cookie 存储（Java 版的 `cookie` 协议特性，
//! 相当于 Paper 1.20.5+ 的 `ClientCookie` API）。
//!
//! Cookie 是小数据块（至多 [`MAX_COOKIE_PAYLOAD`] 字节），由服务器
//! 存储在 Java 客户端上；客户端会在跨服务器转移时保留这些数据，
//! 并在服务器通过 cookie 请求数据包询问时将其回报。
//!
//! 缓存保存在连接对象上：cookie 响应数据包可以
//! 在登录与配置阶段到达（由
//! [`super::pending::PendingConnection`]）处理，也适用于游戏阶段（由
//! [`super::JavaClient`]）。`PendingConnection` 在
//! 连接挂起期间持有该存储，`JavaClient::from_pending` 会将其移交，因此
//! 在登录或配置阶段上报的 cookie 对插件依然可见，
//! 玩家进入游戏后。

use std::collections::HashMap;

use papokin_protocol::java::client::play::{CPlayCookieRequest, CStoreCookie};

use super::JavaClient;

/// 原版 cookie 负载上限（5 KiB）；对应 `MAX_COOKIE_LENGTH`，位于
/// `papokin-protocol` 的 cookie 数据包定义。
pub const MAX_COOKIE_PAYLOAD: usize = 5120;

/// 客户端已上报 cookie 的服务端缓存。
///
/// 以 Cookie 的资源标识符（`namespace:path`）为键。与
/// 连接结构体基于锁的字段风格（`std::sync::Mutex`、
/// 例如 `JavaClient::pending_keep_alives`）。
pub type CookieStore = std::sync::Mutex<HashMap<String, Vec<u8>>>;

/// 创建一个空的 Cookie 存储。
#[must_use]
pub fn new_cookie_store() -> CookieStore {
    std::sync::Mutex::new(HashMap::new())
}

/// 以原版语义将客户端方向的 cookie 响应应用到存储中。
///
/// `Some(payload)` 表示插入或替换条目，而 `None` 表示
/// 客户端没有该 cookie，该条目随即被移除。
pub fn apply_cookie_response(store: &CookieStore, key: &str, payload: Option<&[u8]>) {
    let mut cookies = store
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    match payload {
        Some(payload) => {
            cookies.insert(key.to_string(), payload.to_vec());
        }
        None => {
            cookies.remove(key);
        }
    }
}

impl JavaClient {
    /// 在客户端上存储 cookie（游玩阶段的 `CStoreCookie` 数据包）。
    ///
    /// `key` 必须已通过资源标识符校验，且
    /// `payload` 不得超过 [`MAX_COOKIE_PAYLOAD`] 字节。
    pub async fn store_cookie(&self, key: &str, payload: &[u8]) {
        let key = key.to_string();
        self.send_packet(&CStoreCookie::new(&key, payload)).await;
    }

    /// 向客户端请求存储在 `key` 下的 cookie（游玩阶段的数据包）。
    /// `CPlayCookieRequest` 数据包）。客户端的响应会在
    /// 异步完成并存入 cookie 存储，可通过
    /// [`JavaClient::get_cached_cookie`]。
    pub async fn request_cookie(&self, key: &str) {
        let key = key.to_string();
        self.send_packet(&CPlayCookieRequest::new(&key)).await;
    }

    ///返回 cookie `key` 的缓存值；当其尚未缓存时返回 `None`
    /// 客户端未上报。从不发送数据包。
    pub fn get_cached_cookie(&self, key: &str) -> Option<Vec<u8>> {
        self.cookies
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .get(key)
            .cloned()
    }

    /// 清除 cookie `key`：将其从服务器端缓存中丢弃，并
    /// 会用空负载覆盖客户端的副本（原版没有
    /// 专用的删除数据包）。
    pub async fn clear_cookie(&self, key: &str) {
        self.cookies
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .remove(key);
        self.store_cookie(key, &[]).await;
    }
}
