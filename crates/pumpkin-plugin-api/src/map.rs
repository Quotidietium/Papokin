//! Custom map rendering (MapView).
//!
//! This module lets a plugin create and draw onto server-side maps: set
//! individual pixels or whole 128x128 canvases with ordinary RGB(A) colors,
//! re-render the vanilla terrain view, lock the canvas, and place cursor
//! icons (map decorations) on top. Obtain a [`MapView`] either by creating a
//! fresh map with [`create_map`] (or the [`WorldMapExt`] convenience trait)
//! or by attaching to an existing map id with [`get_map`].
//!
//! Colors are given as `0xAARRGGBB` and are quantized to the vanilla map
//! palette on the host, so plugins never need to deal with palette indices
//! themselves (raw palette access is still available through
//! [`MapView::set_colors_data`] / [`MapView::get_colors_data`]).
//!
//! Every mutation immediately pushes an updated `CMapItemData` packet to
//! Java Edition players holding the map and marks it dirty for the per-tick
//! map sync. Bedrock Edition map packets are not wired up yet (the Bedrock
//! `ClientboundMapItemData` packet still has to be implemented in
//! `pumpkin-protocol`); until then Bedrock players simply see no map
//! updates.
//!
//! # Examples
//!
//! ## Drawing on a brand-new map
//! ```rust,ignore
//! use pumpkin_plugin_api::map::{self, MapCursor, WorldMapExt, cursor_types};
//!
//! fn draw(world: &pumpkin_plugin_api::wit::pumpkin::plugin::world::World) {
//!     let map = world.create_map(0, 0, 0);
//!     // Red diagonal line.
//!     for i in 0..128 {
//!         map.set_pixel(i, i, map::rgb(255, 0, 0));
//!     }
//!     map.add_cursor(&MapCursor {
//!         icon_type: cursor_types::RED_X,
//!         x: 64,
//!         z: 64,
//!         direction: 0,
//!         display_name: Some("X marks the spot".to_string()),
//!     });
//!     map.lock();
//! }
//! ```

pub use crate::wit::pumpkin::plugin::map::{MapCursor, MapView, create_map, get_map};

use crate::wit::pumpkin::plugin::world::World;

/// Width and height of a map canvas in pixels.
pub const MAP_CANVAS_SIZE: u32 = 128;

/// Number of pixels (palette bytes) in a full map canvas.
pub const MAP_CANVAS_PIXELS: usize = (MAP_CANVAS_SIZE * MAP_CANVAS_SIZE) as usize;

/// Packs RGBA channels into the `0xAARRGGBB` color accepted by
/// [`MapView::set_pixel`]. Alpha `0` clears the pixel to the vanilla
/// "unexplored" color; any other alpha value is treated as opaque.
#[must_use]
pub const fn rgba(r: u8, g: u8, b: u8, a: u8) -> u32 {
    ((a as u32) << 24) | ((r as u32) << 16) | ((g as u32) << 8) | b as u32
}

/// Packs RGB channels into an opaque `0xFFRRGGBB` color.
#[must_use]
pub const fn rgb(r: u8, g: u8, b: u8) -> u32 {
    rgba(r, g, b, 255)
}

/// Vanilla map cursor (decoration) type ids for use with
/// [`MapView::add_cursor`], mirroring
/// `pumpkin_data::map_decoration::MapDecorationType`.
pub mod cursor_types {
    /// White player marker.
    pub const PLAYER: i32 = 0;
    /// Item frame marker.
    pub const FRAME: i32 = 1;
    /// Red marker (players on the same team in some game modes).
    pub const RED_MARKER: i32 = 2;
    /// Blue marker.
    pub const BLUE_MARKER: i32 = 3;
    /// Target X marker.
    pub const TARGET_X: i32 = 4;
    /// Target point marker.
    pub const TARGET_POINT: i32 = 5;
    /// Player marker shown when off the map edge.
    pub const PLAYER_OFF_MAP: i32 = 6;
    /// Player marker shown when far off the map.
    pub const PLAYER_OFF_LIMITS: i32 = 7;
    /// Woodland mansion icon.
    pub const MANSION: i32 = 8;
    /// Ocean monument icon.
    pub const MONUMENT: i32 = 9;
    /// White banner marker.
    pub const BANNER_WHITE: i32 = 10;
    /// Orange banner marker.
    pub const BANNER_ORANGE: i32 = 11;
    /// Magenta banner marker.
    pub const BANNER_MAGENTA: i32 = 12;
    /// Light blue banner marker.
    pub const BANNER_LIGHT_BLUE: i32 = 13;
    /// Yellow banner marker.
    pub const BANNER_YELLOW: i32 = 14;
    /// Lime banner marker.
    pub const BANNER_LIME: i32 = 15;
    /// Pink banner marker.
    pub const BANNER_PINK: i32 = 16;
    /// Gray banner marker.
    pub const BANNER_GRAY: i32 = 17;
    /// Light gray banner marker.
    pub const BANNER_LIGHT_GRAY: i32 = 18;
    /// Cyan banner marker.
    pub const BANNER_CYAN: i32 = 19;
    /// Purple banner marker.
    pub const BANNER_PURPLE: i32 = 20;
    /// Blue banner marker.
    pub const BANNER_BLUE: i32 = 21;
    /// Brown banner marker.
    pub const BANNER_BROWN: i32 = 22;
    /// Green banner marker.
    pub const BANNER_GREEN: i32 = 23;
    /// Red banner marker.
    pub const BANNER_RED: i32 = 24;
    /// Black banner marker.
    pub const BANNER_BLACK: i32 = 25;
    /// Red X (treasure map destination).
    pub const RED_X: i32 = 26;
    /// Desert village icon.
    pub const VILLAGE_DESERT: i32 = 27;
    /// Plains village icon.
    pub const VILLAGE_PLAINS: i32 = 28;
    /// Savanna village icon.
    pub const VILLAGE_SAVANNA: i32 = 29;
    /// Snowy village icon.
    pub const VILLAGE_SNOWY: i32 = 30;
    /// Taiga village icon.
    pub const VILLAGE_TAIGA: i32 = 31;
    /// Jungle temple icon.
    pub const JUNGLE_TEMPLE: i32 = 32;
    /// Swamp hut icon.
    pub const SWAMP_HUT: i32 = 33;
    /// Trial chambers icon.
    pub const TRIAL_CHAMBERS: i32 = 34;
}

/// Extension trait on [`World`] for creating maps.
pub trait WorldMapExt {
    /// Creates a new map centered at (`center_x`, `center_z`) in this world
    /// and returns a drawable view over it. `scale` is clamped to 0..=4.
    fn create_map(&self, center_x: i32, center_z: i32, scale: u8) -> MapView;
}

impl WorldMapExt for World {
    fn create_map(&self, center_x: i32, center_z: i32, scale: u8) -> MapView {
        create_map(self, center_x, center_z, scale)
    }
}
