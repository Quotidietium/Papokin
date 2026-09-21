use pumpkin_macros::{Event, cancellable};
use pumpkin_util::math::vector3::Vector3;

/// An event that occurs when an entity starts pathfinding towards a target.
#[cancellable]
#[derive(Event, Clone)]
pub struct EntityPathfindEvent {
    /// The ID of the pathfinding entity.
    pub entity_id: i32,

    /// The ID of the target entity of the path, if any.
    pub target_id: Option<i32>,

    /// The calculated path as a list of positions.
    pub path: Vec<Vector3<f64>>,
}

impl EntityPathfindEvent {
    #[must_use]
    pub const fn new(entity_id: i32, target_id: Option<i32>, path: Vec<Vector3<f64>>) -> Self {
        Self {
            entity_id,
            target_id,
            path,
            cancelled: false,
        }
    }
}
