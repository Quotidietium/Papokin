use serde::{Deserialize, Serialize};

/// 玩家数据持久化的配置。
///
/// 控制是否保存玩家数据以及保存间隔。
#[derive(Deserialize, Serialize)]
#[serde(default)]
pub struct PlayerDataConfig {
    /// 是否启用玩家数据保存。
    pub save_player_data: bool,
    /// 自动保存玩家数据的时间间隔（秒）。
    pub save_player_cron_interval: u64,
}

impl Default for PlayerDataConfig {
    fn default() -> Self {
        Self {
            save_player_data: true,
            save_player_cron_interval: 300,
        }
    }
}
