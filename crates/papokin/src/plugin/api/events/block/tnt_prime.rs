use papokin_macros::{Event, cancellable};
use papokin_util::math::position::BlockPos;

/// TNT 被点燃时触发的事件。
#[cancellable]
#[derive(Event, Clone)]
pub struct TNTPrimeEvent {
    pub block_pos: BlockPos,
    pub prime_reason: String,
}

impl TNTPrimeEvent {
    #[must_use]
    pub const fn new(block_pos: BlockPos, prime_reason: String) -> Self {
        Self {
            block_pos,
            prime_reason,
            cancelled: false,
        }
    }
}
