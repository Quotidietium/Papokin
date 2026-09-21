use pumpkin_data::block_properties::VaultState;
use pumpkin_macros::{Event, cancellable};
use pumpkin_util::math::position::BlockPos;

/// An event that occurs when a vault block changes state.
#[cancellable]
#[derive(Event, Clone)]
pub struct VaultChangeStateEvent {
    /// Position of the vault block.
    pub block_pos: BlockPos,

    /// The state the vault is changing from.
    pub previous_state: VaultState,

    /// The state the vault is changing to.
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
