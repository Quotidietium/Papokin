use papokin_macros::{Event, cancellable};
use papokin_util::math::position::BlockPos;

/// 正在生长的结构类型。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TreeType {
    Oak,
    Spruce,
    Birch,
    Jungle,
    Acacia,
    DarkOak,
    Mangrove,
    Cherry,
    Poplar,
    Azalea,
    BrownMushroom,
    RedMushroom,
    Custom,
}

/// 结构（如树或蘑菇）生长时发生的事件。
#[cancellable]
#[derive(Event, Clone)]
pub struct StructureGrowEvent {
    /// 生长开始时的起始方块位置。
    pub block_pos: BlockPos,

    /// 正在生长的结构类型。
    pub species: TreeType,

    /// 是否使用了骨粉。
    pub bone_meal: bool,
}

impl StructureGrowEvent {
    #[must_use]
    pub const fn new(block_pos: BlockPos, species: TreeType, bone_meal: bool) -> Self {
        Self {
            block_pos,
            species,
            bone_meal,
            cancelled: false,
        }
    }
}
