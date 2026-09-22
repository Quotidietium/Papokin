use super::chunk_state::{Chunk, StagedChunkEnum};
use super::dag::{EdgeKey, NodeKey};
use slotmap::Key;

pub struct ChunkHolder {
    pub target_stage: StagedChunkEnum,
    /// 生成依赖项（例如 `StructureReferences` 相邻区块）所需的最低阶段。
    /// 当相邻区块的生成任务需要此区块时，可能超过 `target_stage`
    /// 但不在玩家的视野半径内。到此阶段为止的任务会被调度，但
    /// 区块不会公开，依赖满足后即被卸载。
    pub dependency_stage: StagedChunkEnum,
    pub current_stage: StagedChunkEnum,
    pub chunk: Option<Chunk>,
    pub occupied: NodeKey,
    pub occupied_by: EdgeKey,
    pub public: bool,
    pub tasks: [NodeKey; StagedChunkEnum::COUNT],
}

impl Default for ChunkHolder {
    fn default() -> Self {
        Self {
            target_stage: StagedChunkEnum::None,
            dependency_stage: StagedChunkEnum::None,
            current_stage: StagedChunkEnum::None,
            chunk: None,
            occupied: NodeKey::null(),
            occupied_by: EdgeKey::null(),
            public: false,
            tasks: [NodeKey::null(); StagedChunkEnum::COUNT],
        }
    }
}
