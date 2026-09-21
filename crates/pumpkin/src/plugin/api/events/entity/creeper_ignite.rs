use pumpkin_macros::{Event, cancellable};

/// An event that occurs when a creeper is ignited.
#[cancellable]
#[derive(Event, Clone)]
pub struct CreeperIgniteEvent {
    /// The ID of the creeper being ignited.
    pub entity_id: i32,

    /// The ID of the entity that ignited the creeper, if any.
    pub igniter_id: Option<i32>,
}

impl CreeperIgniteEvent {
    #[must_use]
    pub const fn new(entity_id: i32, igniter_id: Option<i32>) -> Self {
        Self {
            entity_id,
            igniter_id,
            cancelled: false,
        }
    }
}
