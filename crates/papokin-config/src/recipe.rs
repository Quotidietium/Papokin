use serde::{Deserialize, Serialize};

/// 配方相关配置。
#[derive(Deserialize, Serialize)]
#[serde(default)]
pub struct RecipeConfig {
    /// 是否向客户端发送配方，从而启用配方书。
    pub send_recipes: bool,
}

impl Default for RecipeConfig {
    fn default() -> Self {
        Self { send_recipes: true }
    }
}
