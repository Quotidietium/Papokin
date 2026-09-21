//! Structure template registration and placement.
//!
//! This module lets a plugin register vanilla `.nbt` structure templates
//! (gzipped NBT) at runtime and place them into a world by name. Registered
//! templates live in the same server-side template cache the `/place template`
//! command resolves against, so a template registered here is immediately
//! placeable by both this API and the command — and `/place template` can
//! place templates a plugin registered.
//!
//! Placement goes through the same block-change pipeline as the
//! `/place template` command, so both Java Edition and Bedrock Edition
//! clients see the placed structure.
//!
//! # Examples
//!
//! ## Registering and placing a template
//! ```rust,ignore
//! use pumpkin_plugin_api::structure::{
//!     self, BlockPos, Rotation, WorldStructureExt,
//! };
//!
//! fn build(world: &pumpkin_plugin_api::wit::pumpkin::plugin::world::World, nbt: &[u8]) {
//!     structure::register_structure("my_plugin:hall", nbt).expect("valid template");
//!     assert!(structure::has_structure("my_plugin:hall"));
//!
//!     let origin = BlockPos { x: 0, y: 64, z: 0 };
//!     world
//!         .place_structure("my_plugin:hall", origin, Some(Rotation::Clockwise90), None)
//!         .expect("template exists");
//! }
//! ```

pub use crate::wit::pumpkin::plugin::common::BlockPos;
pub use crate::wit::pumpkin::plugin::structure::{
    Mirror, Rotation, has_structure, list_structures, register_structure,
};

use crate::wit::pumpkin::plugin::structure::place_structure;
use crate::wit::pumpkin::plugin::world::World;

/// Extension trait on [`World`] for placing structure templates.
pub trait WorldStructureExt {
    /// Places a registered or embedded structure template with its origin at
    /// `pos`.
    ///
    /// `rotation` and `mirror` default to [`Rotation::None`] /
    /// [`Mirror::None`] when `None` is passed. Returns an error when no
    /// template with `name` exists. The placement is block-buffered and
    /// flushed through the server's regular block-update pipeline, so every
    /// client version observes the change.
    ///
    /// # Errors
    ///
    /// Returns an error string when the template name cannot be resolved.
    fn place_structure(
        &self,
        name: &str,
        pos: BlockPos,
        rotation: Option<Rotation>,
        mirror: Option<Mirror>,
    ) -> Result<bool, String>;
}

impl WorldStructureExt for World {
    fn place_structure(
        &self,
        name: &str,
        pos: BlockPos,
        rotation: Option<Rotation>,
        mirror: Option<Mirror>,
    ) -> Result<bool, String> {
        place_structure(self, name, pos, rotation, mirror)
    }
}
