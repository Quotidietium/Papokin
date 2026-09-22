use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// 表示服务器白名单中的一个条目。
///
/// 存储玩家的 UUID 和用户名。
#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct WhitelistEntry {
    /// 被加入白名单的玩家的 UUID。
    pub uuid: Uuid,
    /// 白名单玩家的用户名。
    pub name: String,
}

impl WhitelistEntry {
    /// 使用给定的 UUID 和名称创建一个新的白名单条目。
    ///
    /// # Arguments
    /// * `uuid` – 玩家的 UUID。
    /// * `name` – 玩家的用户名。
    #[must_use]
    pub const fn new(uuid: Uuid, name: String) -> Self {
        Self { uuid, name }
    }
}
