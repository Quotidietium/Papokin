use papokin_macros::Event;
use std::sync::Arc;

use crate::entity::player::Player;
use crate::world::World;

use super::PlayerEvent;

/// 区块在玩家客户端卸载时触发的事件。
///
/// 这是一个纯通知；区块数据已经（或即将）被保存
/// 而）在客户端被丢弃。
#[derive(Event, Clone)]
pub struct PlayerChunkUnloadEvent {
    /// 为其卸载区块的玩家。
    pub player: Arc<Player>,

    /// 区块所在的世界。
    pub target_world: Arc<World>,

    /// 区块的 X 坐标。
    pub chunk_x: i32,

    /// 区块的 Z 坐标。
    pub chunk_z: i32,
}

impl PlayerChunkUnloadEvent {
    /// 创建 `PlayerChunkUnloadEvent` 的新实例。
    pub const fn new(
        player: Arc<Player>,
        target_world: Arc<World>,
        chunk_x: i32,
        chunk_z: i32,
    ) -> Self {
        Self {
            player,
            target_world,
            chunk_x,
            chunk_z,
        }
    }
}

impl PlayerEvent for PlayerChunkUnloadEvent {
    fn get_player(&self) -> &Arc<Player> {
        &self.player
    }
}
