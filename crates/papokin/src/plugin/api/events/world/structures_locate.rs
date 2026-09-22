use crate::world::World;
use papokin_macros::Event;
use papokin_util::math::position::BlockPos;
use std::sync::Arc;

/// 服务器定位结构（例如用于 locate 命令）时发生的事件
/// `/locate structure` 命令。
///
/// 处理器可修改定位到的位置：替换结果
/// 会改变所报告的位置。
#[derive(Event, Clone)]
pub struct StructuresLocateEvent {
    /// 执行搜索的世界。
    pub world: Arc<World>,

    /// 搜索的起点。
    pub origin: BlockPos,

    /// 正在定位的结构的标识符。
    pub structure: String,

    /// 搜索半径（以区块为单位）。
    pub radius: i32,

    /// 定位到的结构位置（可修改）。
    pub results: Vec<BlockPos>,
}

impl StructuresLocateEvent {
    #[must_use]
    pub const fn new(
        world: Arc<World>,
        origin: BlockPos,
        structure: String,
        radius: i32,
        results: Vec<BlockPos>,
    ) -> Self {
        Self {
            world,
            origin,
            structure,
            radius,
            results,
        }
    }
}
