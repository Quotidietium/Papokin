use crate::wit::papokin::plugin::biomes::Biome;
use crate::wit::papokin::plugin::world::ChunkBuffer as WitChunkBuffer;
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

pub use crate::wit::papokin::plugin::biomes::Biome as PluginBiome;
pub use crate::wit::papokin::plugin::world::GenerationPhase;

/// 对 WIT `chunk-buffer` 资源的封装，代表一个 16x16 的区块列
/// 正在生成。
pub struct ChunkBuffer {
    inner: WitChunkBuffer,
}

impl ChunkBuffer {
    /// 创建新的 `ChunkBuffer` 包装。
    #[must_use]
    pub const fn new(inner: WitChunkBuffer) -> Self {
        Self { inner }
    }

    ///返回区块 X 坐标。
    #[must_use]
    pub fn x(&self) -> i32 {
        self.inner.get_x()
    }

    ///返回区块 Z 坐标。
    #[must_use]
    pub fn z(&self) -> i32 {
        self.inner.get_z()
    }

    /// 返回世界的最小 Y 坐标。
    #[must_use]
    pub fn min_y(&self) -> i32 {
        self.inner.get_min_y()
    }

    /// 返回区块列的高度（以方块为单位）。
    #[must_use]
    pub fn height(&self) -> u32 {
        self.inner.get_height()
    }

    /// 在区块本地坐标 `(x, y, z)` 处设置方块状态 ID，其中 `0 <= x < 16` 且 `0 <= z < 16`。
    pub fn set_block(&mut self, x: u8, y: i32, z: u8, state_id: u16) {
        self.inner.set_block_state_id(x, y, z, state_id);
    }

    /// 获取区块本地坐标 `(x, y, z)` 处的方块状态 ID。
    #[must_use]
    pub fn get_block(&self, x: u8, y: i32, z: u8) -> u16 {
        self.inner.get_block_state_id(x, y, z)
    }

    /// 用某个方块状态 ID 填充给定 Y 高度的一整个 16x16 水平层。
    pub fn fill_layer(&mut self, y: i32, state_id: u16) {
        self.inner.fill_layer(y, state_id);
    }

    /// 用某个方块状态 ID 填充本地 `(x, z)` 处从 `min_y` 到 `max_y` 的竖直列。
    pub fn fill_range(&mut self, x: u8, min_y: i32, max_y: i32, z: u8, state_id: u16) {
        self.inner.fill_range(x, min_y, max_y, z, state_id);
    }

    /// 用某个方块状态 ID 填充一个三维长方体。
    pub fn fill_cuboid(
        &mut self,
        min_x: u8,
        min_y: i32,
        min_z: u8,
        max_x: u8,
        max_y: i32,
        max_z: u8,
        state_id: u16,
    ) {
        self.inner
            .fill_cuboid(min_x, min_y, min_z, max_x, max_y, max_z, state_id);
    }

    /// 在区块本地坐标 `(x, y, z)` 处设置生物群系。
    pub fn set_biome(&mut self, x: u8, y: i32, z: u8, biome: Biome) {
        self.inner.set_biome(x, y, z, biome);
    }

    /// 用单一生物群系填充整个区块列。
    pub fn fill_biome(&mut self, biome: Biome) {
        self.inner.fill_biome(biome);
    }
}

/// 用于在插件中实现自定义世界生成逻辑的 trait。
#[allow(unused_variables)]
pub trait ChunkGenerator: Send + Sync + 'static {
    /// 步骤 1：为整个区块柱分配生物群系。
    fn generate_biomes(&self, chunk: &mut ChunkBuffer) {}

    /// 步骤 2：在区块中生成基本地形 / 噪声形状。
    fn generate_noise(&self, chunk: &mut ChunkBuffer) {}

    /// 步骤 3：应用地表规则（例如草方块、沙子、石头层）。
    fn generate_surface(&self, chunk: &mut ChunkBuffer) {}

    /// 步骤 4：向区块填充地物、结构、装饰、矿石等。
    fn generate_features(&self, chunk: &mut ChunkBuffer) {}
}

pub(crate) static GENERATOR_HANDLERS: Mutex<BTreeMap<u32, Arc<dyn ChunkGenerator>>> =
    Mutex::new(BTreeMap::new());
static NEXT_GENERATOR_ID: Mutex<u32> = Mutex::new(0);

/// 向服务器运行时注册自定义区块生成器的管理器。
pub struct GeneratorManager;

impl GeneratorManager {
    /// 注册一个自定义区块生成器，并返回其唯一的生成器 ID。
    ///
    /// 随后即可通过 `world.set_chunk_generator(id)` 将此生成器设置到世界上。
    pub fn register<G: ChunkGenerator>(generator: G) -> u32 {
        let mut id_lock = NEXT_GENERATOR_ID.lock().unwrap_or_else(|e| e.into_inner());
        let id = *id_lock;
        *id_lock += 1;

        let mut handlers = GENERATOR_HANDLERS.lock().unwrap_or_else(|e| e.into_inner());
        handlers.insert(id, Arc::new(generator));
        id
    }
}
