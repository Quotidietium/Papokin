use papokin_macros::{Event, cancellable};
use papokin_util::math::position::BlockPos;

/// 一波袭击生成时触发的事件。
#[cancellable]
#[derive(Event, Clone)]
pub struct RaidSpawnWaveEvent {
    /// 波数。
    pub wave: u32,
    /// 生成位置。
    pub pos: BlockPos,
}

impl RaidSpawnWaveEvent {
    #[must_use]
    pub const fn new(wave: u32, pos: BlockPos) -> Self {
        Self {
            wave,
            pos,
            cancelled: false,
        }
    }
}
