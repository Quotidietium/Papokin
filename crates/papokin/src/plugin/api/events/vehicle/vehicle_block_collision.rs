use papokin_macros::{Event, cancellable};
use papokin_util::math::position::BlockPos;

/// 载具与方块碰撞时发生的事件。
#[cancellable]
#[derive(Event, Clone)]
pub struct VehicleBlockCollisionEvent {
    /// 载具实体的 ID。
    pub vehicle_id: i32,
    /// 发生碰撞的方块的位置。
    pub block_pos: BlockPos,
}

impl VehicleBlockCollisionEvent {
    #[must_use]
    pub const fn new(vehicle_id: i32, block_pos: BlockPos) -> Self {
        Self {
            vehicle_id,
            block_pos,
            cancelled: false,
        }
    }
}
