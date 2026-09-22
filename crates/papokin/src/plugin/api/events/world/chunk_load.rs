use crate::world::World;
use papokin_macros::{Event, cancellable};
use papokin_world::chunk::ChunkData;
use std::sync::Arc;
use tokio::sync::RwLock;

/// 区块在世界中加载时触发的事件。
///
/// 此事件包含关于世界和正在加载的区块的信息。
#[cancellable]
#[derive(Event, Clone)]
pub struct ChunkLoad {
    /// 正在加载区块的世界。
    pub world: Arc<World>,

    /// 正在加载的区块数据，包裹在读写锁中以保证并发访问安全。
    pub chunk: Arc<RwLock<ChunkData>>,
}
