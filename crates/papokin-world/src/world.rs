use crate::generation::proto_chunk::GenerationCache;
use bitflags::bitflags;
use papokin_data::{Block, BlockState, BlockStateId, Mirror, Rotation, chunk::Biome};
use papokin_nbt::compound::NbtCompound;
use papokin_util::math::position::BlockPos;
use thiserror::Error;

bitflags! {
    /// 用于控制方块状态变更副作用的标志。
    /// 这些与 Minecraft 的 `setBlockState` 方法所使用的内部位掩码一致
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct BlockFlags: u32 {
        /// 会使周围方块收到邻居更新。
        /// 这就是使侦测器能够检测到变化、使红石元件做出反应的原因
        const NOTIFY_NEIGHBORS                      = 0b000_0000_0001;
        /// 通知监听者（如客户端）方块已发生变化。
        /// 在服务器上，这会触发向范围内的玩家发送数据包
        const NOTIFY_LISTENERS                      = 0b000_0000_0010;
        /// 将邻居通知与监听器通知合并在一起。
        /// 这是大多数方块放置所使用的“标准”更新
        const NOTIFY_ALL                            = 0b000_0000_0011;
        /// 即使与当前状态相同，也强制设置方块状态
        /// 供调试棒（Debug Stick）之类的物品绕过“虚拟”变更检查
        const FORCE_STATE                           = 0b000_0000_0100;
        /// 防止前一个方块在被替换时掉落物品
        /// 常用于方块被“转换”而非被破坏的场合
        const SKIP_DROPS                            = 0b000_0000_1000;
        /// 表示方块正在被移动（通常由活塞推动）
        /// 这可防止某些“破坏时”逻辑在移动完成之前触发
        const MOVED                                 = 0b000_0001_0000;
        /// 防止红石线立即重新计算其形状/信号
        /// 在大规模红石更新期间使用，以减少计算卡顿
        const SKIP_REDSTONE_WIRE_STATE_REPLACEMENT  = 0b000_0010_0000;
        /// 如果已设置，则跳过方块实体（容器）的 `on_replaced` 回调
        /// 如果你要移动方块实体且还不想让其掉落内容物，这会很有用
        const SKIP_BLOCK_ENTITY_REPLACED_CALLBACK   = 0b000_0100_0000;
        /// 防止为新方块状态触发 `on_added` 逻辑
        /// 用此方法避免递归放置循环或不必要的初始化
        const SKIP_BLOCK_ADDED_CALLBACK             = 0b000_1000_0000;
    }
}

#[derive(Debug, Error)]
pub enum GetBlockError {
    InvalidBlockId,
    BlockOutOfWorldBounds,
}

impl std::fmt::Display for GetBlockError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}

pub trait WorldPortalExt: Send + Sync {
    fn can_place_at(
        &self,
        block: &Block,
        state: &BlockState,
        block_accessor: &dyn BlockAccessor,
        block_pos: &BlockPos,
    ) -> bool;

    fn mirror(&self, block: &Block, state_id: BlockStateId, mirror: Mirror) -> &'static BlockState;

    fn rotate(
        &self,
        block: &Block,
        state_id: BlockStateId,
        rotation: Rotation,
    ) -> &'static BlockState;

    fn spawn_mobs_for_chunk_generation(
        &self,
        cache: &mut dyn GenerationCache,
        biome: &'static Biome,
        chunk_x: i32,
        chunk_z: i32,
    );

    fn spawn_structure_entities(&self, _entities: Vec<NbtCompound>) {}
}

pub trait BlockAccessor: Send + Sync {
    fn get_block(&self, position: &BlockPos) -> &'static Block;

    fn get_block_state(&self, position: &BlockPos) -> &'static BlockState;

    fn get_block_state_id(&self, position: &BlockPos) -> BlockStateId;

    fn get_block_and_state(&self, position: &BlockPos) -> (&'static Block, &'static BlockState);
}
