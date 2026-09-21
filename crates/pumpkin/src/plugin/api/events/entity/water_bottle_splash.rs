use pumpkin_macros::{Event, cancellable};

/// An event that occurs when a water bottle splashes on entities.
#[cancellable]
#[derive(Event, Clone)]
pub struct WaterBottleSplashEvent {
    /// The ID of the thrown potion entity.
    pub entity_id: i32,

    /// The IDs of the entities affected by the splash.
    pub affected_entities: Vec<i32>,
}

impl WaterBottleSplashEvent {
    #[must_use]
    pub const fn new(entity_id: i32, affected_entities: Vec<i32>) -> Self {
        Self {
            entity_id,
            affected_entities,
            cancelled: false,
        }
    }
}
