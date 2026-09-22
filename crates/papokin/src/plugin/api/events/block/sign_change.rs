use crate::entity::player::Player;
use papokin_macros::{Event, cancellable};
use papokin_util::math::position::BlockPos;
use std::sync::Arc;

/// 告示牌文本被修改时触发的事件。
#[cancellable]
#[derive(Event, Clone)]
pub struct SignChangeEvent {
    pub player: Arc<Player>,
    pub block_pos: BlockPos,
    pub lines: Vec<String>,
}

impl SignChangeEvent {
    #[must_use]
    pub const fn new(player: Arc<Player>, block_pos: BlockPos, lines: Vec<String>) -> Self {
        Self {
            player,
            block_pos,
            lines,
            cancelled: false,
        }
    }
}
