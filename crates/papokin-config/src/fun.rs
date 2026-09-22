use serde::{Deserialize, Serialize};

/// 不得不承认，我们玩这个游戏的唯一原因就是乐趣。🙃
#[derive(Deserialize, Serialize)]
#[serde(default)]
pub struct FunConfig {
    /// 是否启用愚人节特性。
    pub april_fools: bool,
}

impl Default for FunConfig {
    fn default() -> Self {
        Self { april_fools: true }
    }
}
