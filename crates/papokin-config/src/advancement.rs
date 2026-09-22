use serde::{Deserialize, Serialize};
/// 进度相关配置
///
/// 控制是否应保存和加载进度
#[derive(Deserialize, Serialize)]
#[serde(default)]
pub struct AdvancementConfig {
    /// 是否启用进度保存。
    pub save_advancements: bool,
}

impl Default for AdvancementConfig {
    fn default() -> Self {
        Self {
            save_advancements: true,
        }
    }
}
