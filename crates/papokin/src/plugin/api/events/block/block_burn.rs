use papokin_data::Block;
use papokin_macros::{Event, cancellable};

use super::BlockEvent;

/// 方块被烧毁时发生的事件。
///
/// 此事件包含关于点燃火焰的方块以及正在燃烧的方块的信息。
#[cancellable]
#[derive(Event, Clone)]
pub struct BlockBurnEvent {
    /// 点燃火焰的方块。
    pub igniting_block: &'static Block,

    /// 正在燃烧的方块。
    pub block: &'static Block,
}

impl BlockEvent for BlockBurnEvent {
    fn get_block(&self) -> &Block {
        self.block
    }
}
