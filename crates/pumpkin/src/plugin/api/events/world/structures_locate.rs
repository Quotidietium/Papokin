use crate::world::World;
use pumpkin_macros::Event;
use pumpkin_util::math::position::BlockPos;
use std::sync::Arc;

/// An event that occurs when the server locates structures, e.g. for the
/// `/locate structure` command.
///
/// The located positions can be modified by handlers: replacing the results
/// changes which position is reported.
#[derive(Event, Clone)]
pub struct StructuresLocateEvent {
    /// The world the search runs in.
    pub world: Arc<World>,

    /// The origin of the search.
    pub origin: BlockPos,

    /// The identifier of the structure being located.
    pub structure: String,

    /// The search radius in chunks.
    pub radius: i32,

    /// The located structure positions (modifiable).
    pub results: Vec<BlockPos>,
}

impl StructuresLocateEvent {
    #[must_use]
    pub const fn new(
        world: Arc<World>,
        origin: BlockPos,
        structure: String,
        radius: i32,
        results: Vec<BlockPos>,
    ) -> Self {
        Self {
            world,
            origin,
            structure,
            radius,
            results,
        }
    }
}
