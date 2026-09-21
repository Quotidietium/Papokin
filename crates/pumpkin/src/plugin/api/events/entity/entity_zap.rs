use pumpkin_macros::{Event, cancellable};

/// An event that occurs when an entity is zapped by lightning.
#[cancellable]
#[derive(Event, Clone)]
pub struct EntityZapEvent {
    /// The ID of the entity being zapped.
    pub entity_id: i32,

    /// The ID of the lightning entity.
    pub lightning_id: i32,

    /// The cause of the zap.
    pub cause: String,
}

impl EntityZapEvent {
    #[must_use]
    pub const fn new(entity_id: i32, lightning_id: i32, cause: String) -> Self {
        Self {
            entity_id,
            lightning_id,
            cause,
            cancelled: false,
        }
    }
}
