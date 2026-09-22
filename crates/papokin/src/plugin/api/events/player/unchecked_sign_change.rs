use papokin_macros::{Event, cancellable};
use papokin_util::math::position::BlockPos;
use std::sync::Arc;

use crate::entity::player::Player;

use super::PlayerEvent;

/// 玩家更改告示牌时发生的事件，在任何
/// 校验检查（例如 `SignChangeEvent` 过滤）都会被应用。
///
/// 取消将完全丢弃此次告示牌更新。`lines` 是原始文本行
/// 与客户端发送的完全一致。这是一个 Java 协议事件。
#[cancellable]
#[derive(Event, Clone)]
pub struct UncheckedSignChangeEvent {
    /// 编辑告示牌的玩家。
    pub player: Arc<Player>,

    /// 告示牌方块的位置。
    pub block_pos: BlockPos,

    /// 告示牌的原始文本行。
    pub lines: Vec<String>,
}

impl UncheckedSignChangeEvent {
    /// 创建 `UncheckedSignChangeEvent` 的新实例。
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

impl PlayerEvent for UncheckedSignChangeEvent {
    fn get_player(&self) -> &Arc<Player> {
        &self.player
    }
}
