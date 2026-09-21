use pumpkin_macros::Event;
use pumpkin_util::math::position::BlockPos;

/// An event fired every tick while an entity is inside a block.
///
/// This event is high-frequency; the server only dispatches it when a plugin
/// has registered a handler for it.
#[derive(Event, Clone)]
pub struct EntityInsideBlockEvent {
    /// The ID of the entity inside the block.
    pub entity_id: i32,

    /// Position of the block the entity is inside of.
    pub block_pos: BlockPos,

    /// The name of the block the entity is inside of (e.g. `minecraft:cobweb`).
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
