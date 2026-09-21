use pumpkin_macros::{Event, cancellable};
use pumpkin_util::math::position::BlockPos;

/// An event that occurs when an item-transporting entity (e.g. a hopper
/// minecart) validates the target block it pulls items from.
#[cancellable]
#[derive(Event, Clone)]
pub struct ItemTransportingEntityValidateTargetEvent {
    /// The ID of the transporting entity.
    pub entity_id: i32,

    /// Position of the target block being validated.
    pub target_pos: BlockPos,
}

impl ItemTransportingEntityValidateTargetEvent {
    #[must_use]
    pub const fn new(entity_id: i32, target_pos: BlockPos) -> Self {
        Self {
            entity_id,
            target_pos,
            cancelled: false,
        }
    }
}
