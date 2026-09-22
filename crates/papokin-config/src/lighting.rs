use serde::{Deserialize, Serialize};

/// 光照引擎的计算模式。
#[derive(Deserialize, Serialize, Clone, Copy, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum LightingEngineConfig {
    /// 默认的原版光照传播。
    #[default]
    Default,
    /// 处处为完整天光（无阴影）。
    Full,
    /// 所有位置均为完全黑暗的光照（零光照）。
    Dark,
}
