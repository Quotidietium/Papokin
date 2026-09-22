use async_trait::async_trait;
use papokin_data::BlockStateId;
use papokin_nbt::NbtCompound;
use papokin_util::math::position::BlockPos;
use papokin_world::world::BlockFlags;

use crate::error::GameTestResult;
use crate::model::GameTestRotation;

#[async_trait]
pub trait GameTestWorld: Send + Sync {
    async fn block_state_id(&self, position: &BlockPos) -> BlockStateId;

    async fn set_block_state(
        &self,
        position: &BlockPos,
        block_state_id: BlockStateId,
        flags: BlockFlags,
    ) -> GameTestResult<()>;

    async fn rotate_block_state(
        &self,
        block_state_id: BlockStateId,
        rotation: GameTestRotation,
    ) -> GameTestResult<BlockStateId>;

    async fn set_block_entity_nbt(
        &self,
        position: &BlockPos,
        nbt: &NbtCompound,
    ) -> GameTestResult<()>;

    /// 移除与该半开世界空间盒相交的所有非玩家实体
    /// `[min, max)`。原版在每次放置 `GameTest` 结构之前都会这样做，
    /// 以及在成功结构周围再次进行，因此重跑绝不会继承实体
    /// 上一次尝试所生成的实体。
    async fn clear_non_player_entities(&self, min: &BlockPos, max: &BlockPos)
    -> GameTestResult<()>;

    /// 移除 `[min, max)` 范围内已调度的方块刻，此时结构已被
    /// 被替换，与原版 `GameTest` 的放置清理行为一致。
    async fn clear_scheduled_block_ticks(
        &self,
        min: &BlockPos,
        max: &BlockPos,
    ) -> GameTestResult<()>;

    /// 在结构替换后，移除 `[min, max)` 范围内排队的方块事件。
    async fn clear_block_events(&self, min: &BlockPos, max: &BlockPos) -> GameTestResult<()>;

    /// 返回与半开结构盒相交的每个区块是否
    /// `[min, max)` 内的区块已加载且当前正在运行刻。
    ///
    /// 原版 `GameTestInfo` 在结构放置后会等待此条件一次
    /// 再推进设置/测试时钟。非服务器适配器可以使用默认
    /// 因为它们没有单独的区块刻进生命周期。
    async fn test_area_loaded_and_ticking(&self, _min: &BlockPos, _max: &BlockPos) -> bool {
        true
    }

    async fn set_test_instance_running(&self, position: &BlockPos) -> GameTestResult<()>;

    async fn set_test_instance_success(&self, position: &BlockPos) -> GameTestResult<()>;

    async fn set_test_instance_failure(
        &self,
        position: &BlockPos,
        message: &str,
        marker: Option<(BlockPos, String)>,
    ) -> GameTestResult<()>;

    async fn trigger_test_block(&self, position: &BlockPos) -> GameTestResult<()>;

    async fn reset_test_block(&self, position: &BlockPos) -> GameTestResult<()>;

    async fn test_block_triggered(&self, position: &BlockPos) -> GameTestResult<bool>;

    async fn test_block_message(&self, position: &BlockPos) -> GameTestResult<String>;

    async fn surface_height(&self, x: i32, z: i32) -> i32;
}
