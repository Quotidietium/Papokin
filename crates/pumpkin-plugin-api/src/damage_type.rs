//! Plugin custom damage type registration and builder utilities.
//!
//! This module provides a fluent, type-safe API for defining, registering, and querying
//! custom damage types. Custom damage types are synced to clients through the `damage_type`
//! registry and can be dealt to living entities by name via
//! `damage_by_name`.
//!
//! Registration is only possible while the server is starting up (plugin loading);
//! once the registries are frozen, registration fails with
//! [`DamageTypeError::RegistrationFailed`].
//!
//! # Examples
//!
//! ```rust,ignore
//! use pumpkin_plugin_api::{
//!     damage_type::{DamageEffects, DamageScaling, DamageTypeBuilder},
//!     Context,
//! };
//!
//! fn register_damage_types(context: &Context) {
//!     context
//!         .register_damage_type(
//!             DamageTypeBuilder::new("my_plugin:frost", "frost")
//!                 .scaling(DamageScaling::Always)
//!                 .exhaustion(0.1)
//!                 .effects(DamageEffects::Freezing),
//!         )
//!         .expect("failed to register custom damage type");
//! }
//! ```

pub use crate::wit::pumpkin::plugin::damage_types::{
    CustomDamageType, DamageEffects, DamageScaling, DamageTypeManager, DeathMessageType,
};
use crate::{Context, Server};
use std::fmt;

/// Errors that can occur when building or registering a custom damage type.
#[derive(Clone, Debug, PartialEq)]
pub enum DamageTypeError {
    /// The damage type name was empty.
    EmptyName,
    /// The death message id was empty.
    EmptyMessageId,
    /// Registration with the server damage type manager failed.
    RegistrationFailed(String),
}

impl fmt::Display for DamageTypeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyName => write!(f, "damage type name cannot be empty"),
            Self::EmptyMessageId => write!(f, "damage type message id cannot be empty"),
            Self::RegistrationFailed(msg) => {
                write!(f, "failed to register damage type: {msg}")
            }
        }
    }
}

impl std::error::Error for DamageTypeError {}

/// Fluent builder for constructing and validating a [`CustomDamageType`].
pub struct DamageTypeBuilder {
    name: String,
    message_id: String,
    scaling: DamageScaling,
    exhaustion: f32,
    effects: Option<DamageEffects>,
    death_message_type: DeathMessageType,
}

impl DamageTypeBuilder {
    /// Creates a new damage type builder with the given namespaced name and death message id.
    ///
    /// The name must be a namespaced id outside the `minecraft` namespace
    /// (e.g. `"my_plugin:frost"`); the message id is the death message key
    /// suffix (`death.attack.<message_id>`).
    ///
    /// # Example
    /// ```rust,ignore
    /// let builder = DamageTypeBuilder::new("my_plugin:frost", "frost");
    /// ```
    #[must_use]
    pub fn new(name: impl Into<String>, message_id: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            message_id: message_id.into(),
            scaling: DamageScaling::WhenCausedByLivingNonPlayer,
            exhaustion: 0.1,
            effects: None,
            death_message_type: DeathMessageType::Default,
        }
    }

    /// Sets when the damage amount scales with difficulty
    /// (default: [`DamageScaling::WhenCausedByLivingNonPlayer`]).
    #[must_use]
    pub const fn scaling(mut self, scaling: DamageScaling) -> Self {
        self.scaling = scaling;
        self
    }

    /// Sets the exhaustion applied to players taking this damage (default: 0.1).
    #[must_use]
    pub const fn exhaustion(mut self, exhaustion: f32) -> Self {
        self.exhaustion = exhaustion;
        self
    }

    /// Sets the hurt sound/visual effect override (default: none).
    #[must_use]
    pub const fn effects(mut self, effects: DamageEffects) -> Self {
        self.effects = Some(effects);
        self
    }

    /// Sets how the death message is composed (default: [`DeathMessageType::Default`]).
    #[must_use]
    pub const fn death_message_type(mut self, death_message_type: DeathMessageType) -> Self {
        self.death_message_type = death_message_type;
        self
    }

    /// Validates the damage type parameters.
    ///
    /// # Errors
    /// Returns [`DamageTypeError`] if validation fails.
    pub fn validate(&self) -> Result<(), DamageTypeError> {
        if self.name.trim().is_empty() {
            return Err(DamageTypeError::EmptyName);
        }
        if self.message_id.trim().is_empty() {
            return Err(DamageTypeError::EmptyMessageId);
        }
        Ok(())
    }

    /// Builds the validated [`CustomDamageType`] record.
    ///
    /// # Errors
    /// Returns [`DamageTypeError`] if validation fails.
    pub fn build(self) -> Result<CustomDamageType, DamageTypeError> {
        self.validate()?;
        Ok(CustomDamageType {
            name: self.name,
            message_id: self.message_id,
            scaling: self.scaling,
            exhaustion: self.exhaustion,
            effects: self.effects,
            death_message_type: self.death_message_type,
        })
    }

    /// Registers this custom damage type directly with the provided [`DamageTypeManager`].
    ///
    /// # Errors
    /// Returns [`DamageTypeError`] if validation or registration fails.
    pub fn register(self, manager: &DamageTypeManager) -> Result<(), DamageTypeError> {
        manager.register(self)
    }

    /// Registers this custom damage type with the server.
    ///
    /// # Errors
    /// Returns [`DamageTypeError`] if validation or registration fails.
    pub fn register_to_server(self, server: &Server) -> Result<(), DamageTypeError> {
        let manager = server.get_damage_type_manager();
        manager.register(self)
    }

    /// Registers this custom damage type with the plugin context.
    ///
    /// # Errors
    /// Returns [`DamageTypeError`] if validation or registration fails.
    pub fn register_to_context(self, context: &Context) -> Result<(), DamageTypeError> {
        let manager = context.get_damage_type_manager();
        manager.register(self)
    }
}

/// Trait for damage types that can be registered with a [`DamageTypeManager`].
pub trait RegistrableDamageType {
    /// Registers this damage type with the provided damage type manager.
    ///
    /// # Errors
    /// Returns [`DamageTypeError`] if validation or registration fails.
    fn register(self, manager: &DamageTypeManager) -> Result<(), DamageTypeError>;
}

impl RegistrableDamageType for DamageTypeBuilder {
    fn register(self, manager: &DamageTypeManager) -> Result<(), DamageTypeError> {
        let damage_type = self.build()?;
        manager
            .register_damage_type(&damage_type)
            .map_err(DamageTypeError::RegistrationFailed)
    }
}

impl RegistrableDamageType for CustomDamageType {
    fn register(self, manager: &DamageTypeManager) -> Result<(), DamageTypeError> {
        manager
            .register_damage_type(&self)
            .map_err(DamageTypeError::RegistrationFailed)
    }
}

impl DamageTypeManager {
    /// Registers a custom damage type with the server.
    ///
    /// Accepts a [`DamageTypeBuilder`] or a [`CustomDamageType`].
    ///
    /// # Errors
    /// Returns [`DamageTypeError`] if validation or registration fails.
    pub fn register(&self, damage_type: impl RegistrableDamageType) -> Result<(), DamageTypeError> {
        damage_type.register(self)
    }

    /// Gets a registered custom damage type by its namespaced name.
    #[must_use]
    pub fn get(&self, name: &str) -> Option<CustomDamageType> {
        self.get_damage_type(name)
    }

    /// Checks if a custom damage type name is registered on the server.
    #[must_use]
    pub fn has(&self, name: &str) -> bool {
        self.has_damage_type(name)
    }

    /// Returns the names of all registered custom damage types, in registration order.
    #[must_use]
    pub fn get_all_custom_names(&self) -> Vec<String> {
        self.get_all_custom_damage_type_names()
    }
}

impl Context {
    /// Returns the global damage type manager for registering and querying custom damage types.
    #[must_use]
    pub fn get_damage_type_manager(&self) -> DamageTypeManager {
        self.get_server().get_damage_type_manager()
    }

    /// Registers a custom damage type with the server.
    ///
    /// # Errors
    /// Returns [`DamageTypeError`] if validation or registration fails.
    pub fn register_damage_type(
        &self,
        damage_type: impl RegistrableDamageType,
    ) -> Result<(), DamageTypeError> {
        self.get_damage_type_manager().register(damage_type)
    }
}

impl Server {
    /// Registers a custom damage type with the server.
    ///
    /// # Errors
    /// Returns [`DamageTypeError`] if validation or registration fails.
    pub fn register_damage_type(
        &self,
        damage_type: impl RegistrableDamageType,
    ) -> Result<(), DamageTypeError> {
        self.get_damage_type_manager().register(damage_type)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn damage_type_builder_defaults() {
        let builder = DamageTypeBuilder::new("my_plugin:frost", "frost");
        assert!(matches!(
            builder.scaling,
            DamageScaling::WhenCausedByLivingNonPlayer
        ));
        assert!((builder.exhaustion - 0.1).abs() < f32::EPSILON);
        assert!(builder.effects.is_none());
        assert!(matches!(
            builder.death_message_type,
            DeathMessageType::Default
        ));
    }

    #[test]
    fn damage_type_builder_validation() {
        let builder = DamageTypeBuilder::new("", "frost");
        assert_eq!(builder.validate(), Err(DamageTypeError::EmptyName));

        let builder = DamageTypeBuilder::new("my_plugin:frost", "");
        assert_eq!(builder.validate(), Err(DamageTypeError::EmptyMessageId));
    }
}
