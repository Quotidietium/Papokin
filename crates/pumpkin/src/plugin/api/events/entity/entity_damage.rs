use pumpkin_data::damage::DamageType;
use pumpkin_data::damage_ext::ResolvedDamageType;
use pumpkin_macros::{Event, cancellable};

/// An event that occurs when an entity takes damage.
#[cancellable]
#[derive(Event, Clone)]
pub struct EntityDamageEvent {
    /// The ID of the entity taking damage.
    pub entity_id: i32,

    /// The amount of damage taken.
    pub damage: f32,

    /// The damage type. When the damage was dealt with a plugin-registered
    /// custom damage type, this is `DamageType::GENERIC` and
    /// [`Self::custom_damage_type`] carries the custom type's namespaced id.
    pub damage_type: DamageType,

    /// The namespaced id of the plugin-registered custom damage type this
    /// damage was dealt with, if any.
    pub custom_damage_type: Option<String>,
}

impl EntityDamageEvent {
    #[must_use]
    pub const fn new(entity_id: i32, damage_type: DamageType, damage_amount: f32) -> Self {
        Self {
            entity_id,
            damage: damage_amount,
            damage_type,
            custom_damage_type: None,
            cancelled: false,
        }
    }

    /// Creates the event for a resolved (vanilla or custom) damage type.
    /// Custom types are reported through [`Self::custom_damage_type`] with
    /// `DamageType::GENERIC` as the static stand-in.
    #[must_use]
    pub fn new_resolved(
        entity_id: i32,
        damage_type: &ResolvedDamageType,
        damage_amount: f32,
    ) -> Self {
        Self {
            entity_id,
            damage: damage_amount,
            damage_type: damage_type.vanilla_or(DamageType::GENERIC),
            custom_damage_type: damage_type.custom_name().map(str::to_string),
            cancelled: false,
        }
    }
}
