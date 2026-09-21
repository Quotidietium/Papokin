use pumpkin_data::entity::EntityType;
use pumpkin_macros::Event;

/// An event that occurs when a thrown egg decides whether to hatch.
///
/// Not cancellable; plugins change the outcome by editing `will_hatch`,
/// `num_hatches` and `hatching_type`.
#[derive(Event, Clone)]
pub struct ThrownEggHatchEvent {
    /// The ID of the egg entity.
    pub egg_id: i32,

    /// Whether the egg will hatch.
    pub will_hatch: bool,

    /// The number of entities hatching from the egg.
    pub num_hatches: u8,

    /// The entity type hatching.
    pub hatching_type: &'static EntityType,
}

impl ThrownEggHatchEvent {
    #[must_use]
    pub const fn new(
        egg_id: i32,
        will_hatch: bool,
        num_hatches: u8,
        hatching_type: &'static EntityType,
    ) -> Self {
        Self {
            egg_id,
            will_hatch,
            num_hatches,
            hatching_type,
        }
    }
}
