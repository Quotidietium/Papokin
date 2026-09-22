use papokin_data::Block;
use papokin_macros::{Event, cancellable};
use std::sync::Arc;

use crate::entity::player::Player;

use super::BlockEvent;

/// 玩家尝试在方块上建造时发生的事件。
///
/// 此事件包含关于要建造的方块以及是否允许建造的信息，
/// 尝试建造的玩家，以及正在其上建造的方块。
#[cancellable]
#[derive(Event, Clone)]
pub struct BlockCanBuildEvent {
    /// 玩家尝试建造的方块。
    pub block_to_build: &'static Block,

    /// 一个布尔值，表示是否允许建造。
    pub buildable: bool,

    /// 尝试建造的玩家。
    pub player: Arc<Player>,

    /// 在其上进行建造的方块。
    pub block: &'static Block,
}

impl BlockEvent for BlockCanBuildEvent {
    fn get_block(&self) -> &Block {
        self.block
    }
}
