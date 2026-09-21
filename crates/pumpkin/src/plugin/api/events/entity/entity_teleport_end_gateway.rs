use pumpkin_macros::{Event, cancellable};
use pumpkin_util::math::{position::BlockPos, vector3::Vector3};

/// An event that occurs when an entity teleports through an end gateway.
#[cancellable]
#[derive(Event, Clone)]
pub struct EntityTeleportEndGatewayEvent {
    /// The ID of the teleporting entity.
    pub entity_id: i32,

    /// Position of the end gateway block.
    pub gateway: BlockPos,

    /// The position the entity teleports from.
    pub from_position: Vector3<f64>,

    /// The position the entity teleports to.
    pub to_position: Vector3<f64>,
}

impl EntityTeleportEndGatewayEvent {
    #[must_use]
    pub const fn new(
        entity_id: i32,
        gateway: BlockPos,
        from_position: Vector3<f64>,
        to_position: Vector3<f64>,
    ) -> Self {
        Self {
            entity_id,
            gateway,
            from_position,
            to_position,
            cancelled: false,
        }
    }
}
