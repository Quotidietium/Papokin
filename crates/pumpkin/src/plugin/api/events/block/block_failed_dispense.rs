use pumpkin_data::item_stack::ItemStack;
use pumpkin_macros::{Event, cancellable};
use pumpkin_util::math::position::BlockPos;

/// An event that occurs when a dispenser fails to dispense an item
/// (e.g. because the dispenser is empty).
#[cancellable]
#[derive(Event, Clone)]
pub struct BlockFailedDispenseEvent {
    /// Position of the dispenser block.
    pub block_pos: BlockPos,

    /// The item that failed to be dispensed. Empty when the dispenser held
    /// no item to dispense.
    pub item: ItemStack,
}

impl BlockFailedDispenseEvent {
    #[must_use]
    pub const fn new(block_pos: BlockPos, item: ItemStack) -> Self {
        Self {
            block_pos,
            item,
            cancelled: false,
        }
    }
}
