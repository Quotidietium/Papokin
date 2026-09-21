use pumpkin_macros::{Event, cancellable};

/// An event fired every tick for each active potion effect on an entity.
///
/// This event is high-frequency; the server only dispatches it when a plugin
/// has registered a handler for it.
#[cancellable]
#[derive(Event, Clone)]
pub struct EntityEffectTickEvent {
    /// The ID of the entity the effect is ticking on.
    pub entity_id: i32,

    /// The identifier of the effect type (e.g. `minecraft:speed`).
    pub effect_type: String,

    /// The amplifier of the effect.
    pub amplifier: i32,

    /// The remaining duration of the effect in ticks.
    pub duration: i32,
}

impl EntityEffectTickEvent {
    #[must_use]
    pub const fn new(entity_id: i32, effect_type: String, amplifier: i32, duration: i32) -> Self {
        Self {
            entity_id,
            effect_type,
            amplifier,
            duration,
            cancelled: false,
        }
    }
}
