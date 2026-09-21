use pumpkin_macros::{Event, cancellable};
use pumpkin_util::math::position::BlockPos;

/// An event that occurs when a target block is hit by a projectile or entity.
#[cancellable]
#[derive(Event, Clone)]
pub struct TargetHitEvent {
    /// The entity id of the shooter/owner that hit the target, if any.
    pub shooter_id: Option<i32>,

    /// Position of the target block.
    pub block_pos: BlockPos,

    /// The redstone signal strength emitted (0-15).
    pub signal_strength: i32,
}

impl TargetHitEvent {
    #[must_use]
    pub const fn new(shooter_id: Option<i32>, block_pos: BlockPos, signal_strength: i32) -> Self {
        Self {
            shooter_id,
            block_pos,
            signal_strength,
            cancelled: false,
        }
    }
}
