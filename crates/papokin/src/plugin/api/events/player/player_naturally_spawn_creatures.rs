use papokin_macros::{Event, cancellable};
use std::sync::Arc;

use crate::entity::player::Player;

use super::PlayerEvent;

/// 生物即将自然生成时发生的事件
/// 玩家周围。
///
/// 取消会抑制该区块的自然生成批次。宿主
/// 侧尚未接线（自然生成器由另一个智能体负责）；
/// 参见事件接线报告中的集成说明。
#[cancellable]
#[derive(Event, Clone)]
pub struct PlayerNaturallySpawnCreaturesEvent {
    /// 生物围绕其生成的玩家。
    pub player: Arc<Player>,

    /// 生成区块的 X 坐标。
    pub chunk_x: i32,

    /// 生成区块的 Z 坐标。
    pub chunk_z: i32,
}

impl PlayerNaturallySpawnCreaturesEvent {
    /// 创建 `PlayerNaturallySpawnCreaturesEvent` 的新实例。
    pub const fn new(player: Arc<Player>, chunk_x: i32, chunk_z: i32) -> Self {
        Self {
            player,
            chunk_x,
            chunk_z,
            cancelled: false,
        }
    }
}

impl PlayerEvent for PlayerNaturallySpawnCreaturesEvent {
    fn get_player(&self) -> &Arc<Player> {
        &self.player
    }
}
