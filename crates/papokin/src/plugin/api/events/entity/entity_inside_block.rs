use papokin_macros::Event;
use papokin_util::math::position::BlockPos;

/// 实体处于方块内时每刻触发的事件。
///
/// 此事件是高频事件；服务器仅在插件需要时才分发它，
/// 已为其注册了处理程序。
#[derive(Event, Clone)]
pub struct EntityInsideBlockEvent {
    /// 方块内部的实体 ID。
    pub entity_id: i32,

    /// 实体所处方块的位置。
    pub block_pos: BlockPos,

    /// 实体所处方块的名称（例如 `minecraft:cobweb`）。
    pub block_name: String,
}

impl EntityInsideBlockEvent {
    #[must_use]
    pub const fn new(entity_id: i32, block_pos: BlockPos, block_name: String) -> Self {
        Self {
            entity_id,
            block_pos,
            block_name,
        }
    }
}
