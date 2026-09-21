//! Chunk handles, copy-on-read chunk snapshots, and chunk loading controls.
//!
//! The [`World`](crate::World) resource exposes the chunk lifecycle directly:
//!
//! - `get_chunk(x, z)` returns a live [`Chunk`](crate::wit::pumpkin::plugin::world::Chunk)
//!   handle for an already-loaded chunk.
//! - `is_chunk_loaded(x, z)` queries the loading state without touching the
//!   loading pipeline.
//! - `load_chunk(x, z)` holds a plugin ticket and triggers asynchronous
//!   loading/generation when needed; pair every call with
//!   `unload_chunk(x, z)` to release the ticket again.
//! - `set_chunk_forced(x, z, forced)` / `is_chunk_forced(x, z)` manage the
//!   forced (persistently loaded) mark.
//! - `get_chunk_snapshot(x, z)` returns a [`ChunkSnapshot`]: a read-only,
//!   point-in-time copy that stays valid after the live chunk changes or
//!   unloads.
//!
//! ```rust,ignore
//! fn inspect(world: &pumpkin_plugin_api::World) {
//!     let was_loaded = world.load_chunk(3, -7);
//!     if let Some(snapshot) = world.get_chunk_snapshot(3, -7) {
//!         let surface = snapshot.top_block_y(8, 8);
//!         let state = snapshot.block_state_id_at(8, surface, 8);
//!         tracing::info!("surface block state {state} at y={surface} (was_loaded={was_loaded})");
//!     }
//!     world.unload_chunk(3, -7);
//! }
//! ```

use crate::wit::pumpkin::plugin::biomes::Biome;
use crate::wit::pumpkin::plugin::world::{BlockPos, BlockState, ChunkSnapshot};

impl ChunkSnapshot {
    /// Returns the block state ID at chunk-local coordinates: `x` and `z` in
    /// the range `[0, 15]`, `y` absolute. Out-of-range positions return the
    /// air block state ID.
    #[must_use]
    pub fn block_state_id_at(&self, x: i32, y: i32, z: i32) -> u16 {
        self.get_block_state_id(BlockPos { x, y, z })
    }

    /// Returns detailed block state information at chunk-local coordinates:
    /// `x` and `z` in the range `[0, 15]`, `y` absolute.
    #[must_use]
    pub fn block_state_at(&self, x: i32, y: i32, z: i32) -> BlockState {
        self.get_block_state(BlockPos { x, y, z })
    }

    /// Returns the biome at chunk-local coordinates: `x` and `z` in the range
    /// `[0, 15]`, `y` absolute.
    #[must_use]
    pub fn biome_at(&self, x: i32, y: i32, z: i32) -> Biome {
        self.get_biome(BlockPos { x, y, z })
    }

    /// Returns the highest non-air block Y coordinate at chunk-local `(x, z)`
    /// (both in `[0, 15]`), or `min_y - 1` when the column is entirely air.
    #[must_use]
    pub fn top_block_y(&self, x: i32, z: i32) -> i32 {
        self.get_top_block_y(x, z)
    }

    /// Returns the height of the snapshot in blocks
    /// (`section_count * 16`).
    #[must_use]
    pub fn height(&self) -> u32 {
        self.get_section_count() * 16
    }

    /// Copies all 4096 block state IDs of one 16x16x16 section
    /// (`section_index` 0 is the bottom section), ordered Y-major, then Z,
    /// then X. This is the paged bulk-read path: a full chunk is
    /// `get_section_count()` calls of roughly 8 KiB each.
    #[must_use]
    pub fn section_block_states(&self, section_index: u32) -> Vec<u16> {
        self.dump_section_block_states(section_index)
    }

    /// Copies all 64 biome IDs of one section (4x4x4 quart resolution),
    /// ordered Y-major, then Z, then X.
    #[must_use]
    pub fn section_biomes(&self, section_index: u32) -> Vec<u8> {
        self.dump_section_biomes(section_index)
    }

    /// Copies every block state ID of the whole chunk, all sections from the
    /// bottom up (section-major; within a section Y-major, then Z, then X).
    ///
    /// Note this transfers roughly 192 KiB across the host boundary; prefer
    /// [`ChunkSnapshot::section_block_states`] when only part of the chunk is
    /// needed.
    #[must_use]
    pub fn all_block_states(&self) -> Vec<u16> {
        let mut out = Vec::with_capacity(self.get_section_count() as usize * 4096);
        for section in 0..self.get_section_count() {
            out.extend_from_slice(&self.dump_section_block_states(section));
        }
        out
    }
}
