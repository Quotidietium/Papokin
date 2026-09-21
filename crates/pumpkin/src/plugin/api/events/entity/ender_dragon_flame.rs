use pumpkin_macros::{Event, cancellable};

/// An event that occurs when an ender dragon breathes fire, creating an area
/// effect cloud.
#[cancellable]
#[derive(Event, Clone)]
pub struct EnderDragonFlameEvent {
    /// The ID of the ender dragon entity.
    pub entity_id: i32,

    /// The ID of the area effect cloud created by the breath.
    pub area_effect_cloud_id: i32,
}

impl EnderDragonFlameEvent {
    #[must_use]
    pub const fn new(entity_id: i32, area_effect_cloud_id: i32) -> Self {
        Self {
            entity_id,
            area_effect_cloud_id,
            cancelled: false,
        }
    }
}
