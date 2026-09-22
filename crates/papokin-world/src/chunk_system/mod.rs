#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

/*
TODO
1. 添加原型区块脏标记
2. 更好的优先级
5. 为加载票据添加生命周期
6. 解决实体无法卸载的问题
*/

pub type HashMapType<K, V> = rustc_hash::FxHashMap<K, V>;
pub type HashSetType<K> = rustc_hash::FxHashSet<K>;
pub type ChunkPos = papokin_util::math::vector2::Vector2<i32>;
pub type ChunkLevel = HashMapType<ChunkPos, i8>;
pub type IOLock = std::sync::Arc<(
    std::sync::Mutex<HashMapType<ChunkPos, u8>>,
    tokio::sync::Notify,
)>;

pub mod channel;
pub mod chunk_holder;
pub mod chunk_listener;
pub mod chunk_loading;
pub mod chunk_state;
pub mod dag;
pub mod generation;
pub mod generation_cache;
pub mod schedule;
pub mod worker_logic;

#[cfg(test)]
mod tests;

pub use channel::LevelChannel;
pub use chunk_holder::ChunkHolder;
pub use chunk_listener::ChunkListener;
pub use chunk_loading::ChunkLoading;
pub use chunk_state::{Chunk, StagedChunkEnum};
pub use dag::DAG;
pub use generation::generate_single_chunk;
pub use generation_cache::Cache;
pub use schedule::GenerationSchedule;
