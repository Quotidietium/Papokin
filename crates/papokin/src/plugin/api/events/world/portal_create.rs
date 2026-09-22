use papokin_macros::{Event, cancellable};
use papokin_util::math::position::BlockPos;

/// 创建的传送门类型。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PortalType {
    Nether,
    End,
    Custom,
}

/// 下界或末地传送门框架/方块被创建时发生的事件。
#[cancellable]
#[derive(Event, Clone)]
pub struct PortalCreateEvent {
    /// 传送门创建处的方块位置。
    pub block_pos: BlockPos,

    /// 创建的传送门类型。
    pub portal_type: PortalType,
}

impl PortalCreateEvent {
    #[must_use]
    pub const fn new(block_pos: BlockPos, portal_type: PortalType) -> Self {
        Self {
            block_pos,
            portal_type,
            cancelled: false,
        }
    }
}
