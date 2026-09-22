use papokin_util::permission::PermissionLvl;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// 表示服务器上的管理员（OP）。
///
/// 包括其 UUID、名称、权限等级，以及是否不受玩家上限限制。
#[derive(Serialize, Deserialize, Clone, Default)]
pub struct Op {
    /// 操作员的 UUID。
    pub uuid: Uuid,
    /// 管理员的名称。
    pub name: String,
    /// 分配给此操作员的权限等级。
    pub level: PermissionLvl,
    /// 此管理员是否不受服务器玩家上限的限制。
    pub bypasses_player_limit: bool,
}

impl Op {
    /// 使用给定的属性创建一个新的操作符。
    ///
    /// # Arguments
    /// * `uuid` – 管理员的 UUID。
    /// * `name` – 管理员的名称。
    /// * `level` – 分配给该管理员的权限等级。
    /// * `bypasses_player_limit` – 操作员是否可绕过服务器的玩家上限。
    #[must_use]
    pub const fn new(
        uuid: Uuid,
        name: String,
        level: PermissionLvl,
        bypasses_player_limit: bool,
    ) -> Self {
        Self {
            uuid,
            name,
            level,
            bypasses_player_limit,
        }
    }
}
