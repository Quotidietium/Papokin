//! Loot table query and generation.
//!
//! This module exposes the server's vanilla loot tables: check that a table
//! exists with [`has_loot_table`], roll it deterministically with
//! [`generate_loot`] / [`generate_loot_with_context`], or scatter a roll into
//! a container inventory with [`fill_inventory`] (the same shuffle-and-split
//! vanilla uses when a loot chest is first opened).
//!
//! All of these are pure data operations against the static datapack tables:
//! nothing is spawned into a world and no player context is required, so they
//! are safe to call from anywhere, including headless startup, and behave
//! identically for Java and Bedrock sessions.
//!
//! Keys accept an optional `minecraft:` namespace prefix:
//! `"chests/simple_dungeon"` and `"minecraft:chests/simple_dungeon"` resolve
//! to the same table.
//!
//! # Examples
//!
//! ```rust,ignore
//! use pumpkin_plugin_api::loot::{self, LootContext};
//! use pumpkin_plugin_api::ItemStack;
//!
//! // Deterministic roll of the simple dungeon chest table.
//! let stacks = loot::generate_loot("minecraft:chests/simple_dungeon", 42)?;
//!
//! // Entity-style roll with a looting sword as the tool. The tool handle is
//! // consumed, so pass a freshly built snapshot.
//! let context = LootContext::new()
//!     .killed_by_player(true)
//!     .tool(ItemStack::new("minecraft:diamond_sword", 1));
//! let drops = loot::generate_loot_with_context("minecraft:entities/zombie", 7, context)?;
//! ```

pub use crate::wit::pumpkin::plugin::loot::{
    LootContext, fill_inventory, generate_loot, generate_loot_with_context, has_loot_table,
};

use crate::ItemStack;

impl LootContext {
    /// Returns an empty context: no luck, not player-killed, no explosion,
    /// no tool. Equivalent to what [`generate_loot`] uses internally.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            luck: 0.0,
            killed_by_player: false,
            explosion_radius: None,
            tool: None,
        }
    }

    /// Sets the luck value (vanilla `generic.luck` attribute).
    ///
    /// Note: the server's loot generator does not currently read `luck` (no
    /// condition consumes it yet); it is exposed so tables that gain
    /// luck-aware conditions need no API change.
    #[must_use]
    pub const fn luck(mut self, luck: f32) -> Self {
        self.luck = luck;
        self
    }

    /// Sets whether the drop results from a player kill; drives the
    /// `killed-by-player` condition used by entity loot tables.
    #[must_use]
    pub const fn killed_by_player(mut self, killed_by_player: bool) -> Self {
        self.killed_by_player = killed_by_player;
        self
    }

    /// Sets the explosion radius; drives the `survives-explosion` condition.
    /// `None` means no explosion, so the condition always passes.
    #[must_use]
    pub const fn explosion_radius(mut self, radius: Option<f32>) -> Self {
        self.explosion_radius = radius;
        self
    }

    /// Sets the tool used for the break/kill; its enchantments drive the
    /// silk-touch, shears, fortune, and looting checks.
    ///
    /// The stack handle is consumed by the context: pass a freshly built or
    /// disposable snapshot.
    #[must_use]
    pub fn tool(mut self, tool: ItemStack) -> Self {
        self.tool = Some(tool);
        self
    }
}

impl Default for LootContext {
    fn default() -> Self {
        Self::new()
    }
}
