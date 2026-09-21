//! End dragon battle (DragonBattle) inspection and control.
//!
//! This module mirrors the Paper `DragonBattle` API over the server's
//! `DragonFight` manager. Obtain a [`DragonFight`] handle from an End world
//! with [`get_dragon_fight`] or the [`WorldDragonFightExt`] convenience
//! trait; for any other dimension the call returns `None`.
//!
//! The handle lets a plugin query the tracked dragon ([`DragonFight::get_dragon_uuid`],
//! [`DragonFight::is_dragon_alive`], [`DragonFight::has_been_killed_previously`]),
//! observe and drive the respawn animation ([`DragonFight::get_respawn_stage`],
//! [`DragonFight::set_respawn_stage`], [`DragonFight::initiate_respawn`],
//! [`DragonFight::abort_respawn`]), query the alive spike crystals and the
//! exit portal location, and spawn the fight's structures (crystals, exit
//! portal, gateways).
//!
//! The fight's boss bar is intentionally not re-exposed here; use the
//! `boss-bar` WIT interface instead. All state changes go through the same
//! server-side `DragonFight` code paths vanilla uses, so both Java Edition
//! and Bedrock Edition clients observe them.
//!
//! # Examples
//!
//! ## Querying the fight of the End world
//! ```rust,ignore
//! use pumpkin_plugin_api::dragon::WorldDragonFightExt;
//!
//! fn check(world: &pumpkin_plugin_api::wit::pumpkin::plugin::world::World) {
//!     let Some(fight) = world.get_dragon_fight() else {
//!         return; // Not The End.
//!     };
//!     if !fight.is_dragon_alive() && !fight.get_respawn_stage().is_some() {
//!         fight.initiate_respawn();
//!     }
//! }
//! ```

pub use crate::wit::pumpkin::plugin::common::BlockPos;
pub use crate::wit::pumpkin::plugin::dragon::{DragonFight, DragonRespawnStage, get_dragon_fight};
pub use crate::wit::pumpkin::plugin::uuid::Uuid;

use crate::wit::pumpkin::plugin::world::World;

/// Extension trait on [`World`] for obtaining its dragon fight.
pub trait WorldDragonFightExt {
    /// Returns this world's [`DragonFight`], or `None` when the world is not
    /// The End (only The End has a dragon fight).
    #[must_use]
    fn get_dragon_fight(&self) -> Option<DragonFight>;
}

impl WorldDragonFightExt for World {
    fn get_dragon_fight(&self) -> Option<DragonFight> {
        get_dragon_fight(self)
    }
}
