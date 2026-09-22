use serde::{Deserialize, Serialize};

/// 玩家对战（PvP）机制的配置。
///
/// 控制是否启用 PVP、战斗效果以及玩家保护。
#[derive(Deserialize, Serialize)]
#[serde(default)]
pub struct PVPConfig {
    /// 服务器是否启用 PVP。
    pub enabled: bool,
    /// 被击中时是否显示红色受伤动画和 FOV 摆动。
    pub hurt_animation: bool,
    /// 创造模式的玩家是否免受 PVP 影响。
    pub protect_creative: bool,
    /// 是否应用攻击的击退。
    pub knockback: bool,
    /// 玩家攻击时是否挥手。
    pub swing: bool,
}

impl Default for PVPConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            hurt_animation: true,
            protect_creative: true,
            knockback: true,
            swing: true,
        }
    }
}
