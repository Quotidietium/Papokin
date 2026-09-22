//! 区块句柄、读取时复制的区块快照，以及区块加载控制。
//!
//! [`World`](crate::World) 资源直接暴露区块生命周期：
//!
//! - `get_chunk(x, z)` 返回实时的 [`Chunk`](crate::wit::papokin::plugin::world::Chunk)
//!   已加载区块的句柄。
//! - `is_chunk_loaded(x, z)` 查询加载状态，不触碰
//!   加载管线。
//! - `load_chunk(x, z)` 持有一个插件票据并触发异步
//!   加载/生成；每次调用都要与
//!   `unload_chunk(x, z)` 再次释放票据。
//! - `set_chunk_forced(x, z, forced)` / `is_chunk_forced(x, z)` 管理
//!   forced（持久加载）标记。
//! - `get_chunk_snapshot(x, z)` 返回 [`ChunkSnapshot`]：只读的
//!   时间点副本，在实时区块发生变化或
//!   卸载后仍然有效。
//!
//! ```rust,ignore
//! fn inspect(world: &papokin_plugin_api::World) {
//!     let was_loaded = world.load_chunk(3, -7);
//!     if let Some(snapshot) = world.get_chunk_snapshot(3, -7) {
//!         let surface = snapshot.top_block_y(8, 8);
//!         let state = snapshot.block_state_id_at(8, surface, 8);
//!         tracing::info!("surface block state {state} at y={surface} (was_loaded={was_loaded})");
//!     }
//!     world.unload_chunk(3, -7);
//! }
//! ```

use crate::wit::papokin::plugin::biomes::Biome;
use crate::wit::papokin::plugin::world::{BlockPos, BlockState, ChunkSnapshot};

impl ChunkSnapshot {
    ///返回区块局部坐标处的方块状态 ID：`x` 和 `z` 在规定范围内，
    /// 范围 `[0, 15]` 内，`y` 为绝对值。越界位置返回
    /// 空气方块状态 ID。
    #[must_use]
    pub fn block_state_id_at(&self, x: i32, y: i32, z: i32) -> u16 {
        self.get_block_state_id(BlockPos { x, y, z })
    }

    ///返回区块局部坐标处的详细方块状态信息：
    /// `x` 和 `z` 在 `[0, 15]` 范围内，`y` 为绝对坐标。
    #[must_use]
    pub fn block_state_at(&self, x: i32, y: i32, z: i32) -> BlockState {
        self.get_block_state(BlockPos { x, y, z })
    }

    ///返回区块局部坐标处的生物群系：`x` 和 `z` 在规定范围内，
    /// `[0, 15]`，`y` 为绝对坐标。
    #[must_use]
    pub fn biome_at(&self, x: i32, y: i32, z: i32) -> Biome {
        self.get_biome(BlockPos { x, y, z })
    }

    /// 返回区块内局部坐标 `(x, z)` 处最高的非空气方块 Y 坐标
    /// （均在 `[0, 15]` 内），若整列皆为空气则为 `min_y - 1`。
    #[must_use]
    pub fn top_block_y(&self, x: i32, z: i32) -> i32 {
        self.get_top_block_y(x, z)
    }

    /// 返回快照的高度（以方块为单位）
    /// （`section_count * 16`）。
    #[must_use]
    pub fn height(&self) -> u32 {
        self.get_section_count() * 16
    }

    /// 复制一个 16x16x16 section 的全部 4096 个方块状态 ID
    /// （`section_index` 0 为最底层 section），按 Y 主序、再 Z、
    /// 然后是 X。这是分页批量读取路径：一个完整的区块会被
    /// 每次 `get_section_count()` 调用约 8 KiB。
    #[must_use]
    pub fn section_block_states(&self, section_index: u32) -> Vec<u16> {
        self.dump_section_block_states(section_index)
    }

    /// 复制一个 section 的全部 64 个生物群系 ID（4x4x4 quart 分辨率），
    /// 按 Y 优先、然后 Z、再按 X 的顺序排列。
    #[must_use]
    pub fn section_biomes(&self, section_index: u32) -> Vec<u8> {
        self.dump_section_biomes(section_index)
    }

    /// 复制整个区块的每个方块状态 ID，包括从
    /// 自底向上（按分区优先；分区内先按 Y，再按 Z，再按 X）。
    ///
    /// 注意：这会跨宿主边界传输约 192 KiB；请优先
    /// [`ChunkSnapshot::section_block_states`]，当区块只有一部分
    /// 需要时。
    #[must_use]
    pub fn all_block_states(&self) -> Vec<u16> {
        let mut out = Vec::with_capacity(self.get_section_count() as usize * 4096);
        for section in 0..self.get_section_count() {
            out.extend_from_slice(&self.dump_section_block_states(section));
        }
        out
    }
}
