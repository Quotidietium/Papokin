use papokin_data::item_stack::ItemStack;
use papokin_macros::{Event, cancellable};
use std::sync::Arc;

use crate::entity::player::Player;

/// 物品在附魔台中准备附魔时触发的事件。
#[cancellable]
#[derive(Event, Clone)]
pub struct PrepareItemEnchantEvent {
    /// 准备附魔的玩家。
    pub player: Arc<Player>,

    /// 正在附魔的物品。
    pub item: ItemStack,

    /// 3 个槽位各自所需的等级消耗。
    pub level_requirements: [i32; 3],

    /// 3 个槽位各自的附魔提示 ID（无则为 -1）。
    pub enchantment_id: [i32; 3],

    /// 3 个槽位各自的附魔提示等级（无则为 -1）。
    pub enchantment_level: [i32; 3],

    /// 附魔台周围的书架数量。
    pub bookshelf_count: i32,
}

impl PrepareItemEnchantEvent {
    #[must_use]
    pub const fn new(
        player: Arc<Player>,
        item: ItemStack,
        level_requirements: [i32; 3],
        enchantment_id: [i32; 3],
        enchantment_level: [i32; 3],
        bookshelf_count: i32,
    ) -> Self {
        Self {
            player,
            item,
            level_requirements,
            enchantment_id,
            enchantment_level,
            bookshelf_count,
            cancelled: false,
        }
    }
}
