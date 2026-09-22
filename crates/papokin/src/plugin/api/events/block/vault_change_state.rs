use papokin_data::block_properties::VaultState;
use papokin_macros::{Event, cancellable};
use papokin_util::math::position::BlockPos;

/// 宝库方块状态变化时发生的事件。
#[cancellable]
#[derive(Event, Clone)]
pub struct VaultChangeStateEvent {
    /// 宝库方块的位置。
    pub block_pos: BlockPos,

    /// 宝库变更前的状态。
    pub previous_state: VaultState,

    /// 宝库变更后的状态。
    pub new_state: VaultState,
}

impl VaultChangeStateEvent {
    #[must_use]
    pub const fn new(
        block_pos: BlockPos,
        previous_state: VaultState,
        new_state: VaultState,
    ) -> Self {
        Self {
            block_pos,
            previous_state,
            new_state,
            cancelled: false,
        }
    }
}
