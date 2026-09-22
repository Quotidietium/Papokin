use crate::world::World;
use papokin_macros::Event;
use papokin_util::math::position::BlockPos;
use std::sync::Arc;

/// 世界出生点变化时触发的事件。
#[derive(Event, Clone)]
pub struct SpawnChangeEvent {
    /// 出生点发生变化的世界。
    pub world: Arc<World>,

    /// 之前的出生点位置。
    pub previous_position: BlockPos,

    /// 之前的出生点偏航角。
    pub previous_yaw: f32,

    /// 之前的出生点俯仰角。
    pub previous_pitch: f32,

    /// 新的生成点位置。
    pub new_position: BlockPos,

    /// 新的生成点偏航角。
    pub new_yaw: f32,

    /// 新的生成点俯仰角。
    pub new_pitch: f32,
}

impl SpawnChangeEvent {
    /// 创建新的 `SpawnChangeEvent`。
    #[must_use]
    pub const fn new(
        world: Arc<World>,
        previous_position: BlockPos,
        previous_yaw: f32,
        previous_pitch: f32,
        new_position: BlockPos,
        new_yaw: f32,
        new_pitch: f32,
    ) -> Self {
        Self {
            world,
            previous_position,
            previous_yaw,
            previous_pitch,
            new_position,
            new_yaw,
            new_pitch,
        }
    }
}
