//! Read-only combat tracker queries for living entities.
//!
//! Every living entity keeps a combat tracker recording the damage entries of
//! its current combat episode, kill credit, and combat state. The query
//! functions are WIT methods on [`LivingEntity`] handles
//! (`living.get_combat_entries()`, `living.get_killer()`, ...); this module
//! re-exports the [`CombatEntry`] record and provides [`PlayerCombatExt`] so
//! plugins holding a [`Player`] can run the same queries without converting
//! handles manually.
//!
//! All timestamps and durations are in milliseconds, derived from the world
//! tick clock at 20 ticks per second (1 tick = 50 ms).

pub use crate::wit::pumpkin::plugin::combat::CombatEntry;
use crate::wit::pumpkin::plugin::player::Player;
use crate::wit::pumpkin::plugin::world::LivingEntity;

fn living_of(player: &Player) -> Option<LivingEntity> {
    player.as_entity().as_living()
}

/// Combat tracker queries for [`Player`] handles.
///
/// Players are living entities; these helpers forward to the `living-entity`
/// combat tracker methods so plugins do not have to convert handles manually.
pub trait PlayerCombatExt {
    /// Returns the recorded damage entries of the current combat episode,
    /// oldest first. Empty when the player has not taken damage recently (the
    /// tracker resets after a timeout without damage).
    fn get_combat_entries(&self) -> Vec<CombatEntry>;

    /// Returns the damage entry of the attacker credited with the kill: the
    /// highest-damage living attacker, preferring the player attacker when
    /// the player's damage meets the vanilla one-third rule. `None` when no
    /// living attacker is recorded.
    fn get_killer(&self) -> Option<CombatEntry>;

    /// Returns whether the player is currently flagged as in combat.
    fn is_in_combat(&self) -> bool;

    /// Returns the combat duration in milliseconds: combat end minus combat
    /// start, or now minus combat start while still in combat.
    fn get_combat_duration_ms(&self) -> i64;

    /// Returns the name of the damage type of the last confirmed hit: the
    /// vanilla message id (e.g. `"arrow"`) or the namespaced name of a
    /// plugin-registered custom damage type. `None` when no hit is remembered
    /// (forgotten after 40 ticks).
    fn get_last_damage_type_name(&self) -> Option<String>;

    /// Returns whether any attacker in the current combat episode was a player.
    fn has_player_attacker(&self) -> bool;
}

impl PlayerCombatExt for Player {
    fn get_combat_entries(&self) -> Vec<CombatEntry> {
        living_of(self).map_or_else(Vec::new, |living| living.get_combat_entries())
    }

    fn get_killer(&self) -> Option<CombatEntry> {
        living_of(self).and_then(|living| living.get_killer())
    }

    fn is_in_combat(&self) -> bool {
        living_of(self).is_some_and(|living| living.is_in_combat())
    }

    fn get_combat_duration_ms(&self) -> i64 {
        living_of(self).map_or(0, |living| living.get_combat_duration_ms())
    }

    fn get_last_damage_type_name(&self) -> Option<String> {
        living_of(self).and_then(|living| living.get_last_damage_type_name())
    }

    fn has_player_attacker(&self) -> bool {
        living_of(self).is_some_and(|living| living.has_player_attacker())
    }
}
