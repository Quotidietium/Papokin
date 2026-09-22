use crate::world::World;
use papokin_macros::{Event, cancellable};
use papokin_world::chunk::ChunkData;
use std::sync::Arc;

/// 区块发送给客户端时触发的事件。
///
/// 此事件包含关于世界和正在发送的区块的信息。
#[cancellable]
#[derive(Event, Clone)]
pub struct ChunkSend {
    /// 发送该区块所来自的世界。
    pub world: Arc<World>,

    /// 正在发送的区块数据。
    pub chunk: Arc<ChunkData>,
}

impl ChunkSend {
    pub const fn new(world: Arc<World>, chunk: Arc<ChunkData>) -> Self {
        Self {
            world,
            chunk,
            cancelled: false,
        }
    }
}
