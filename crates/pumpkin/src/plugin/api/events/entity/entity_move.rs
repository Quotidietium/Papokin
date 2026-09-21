use pumpkin_macros::Event;
use pumpkin_util::math::vector3::Vector3;

/// An event that occurs when an entity moves.
///
/// This event is extremely high-frequency; the server only dispatches it when
/// a plugin has registered a handler for it.
#[derive(Event, Clone)]
pub struct EntityMoveEvent {
    /// The ID of the entity that moved.
    pub entity_id: i32,

    /// The position the entity moved from.
    pub from_position: Vector3<f64>,

    /// The position the entity moved to.
    pub to_position: Vector3<f64>,
}

impl EntityMoveEvent {
    #[must_use]
    pub const fn new(
        entity_id: i32,
        from_position: Vector3<f64>,
        to_position: Vector3<f64>,
    ) -> Self {
        Self {
            entity_id,
            from_position,
            to_position,
        }
    }
}
