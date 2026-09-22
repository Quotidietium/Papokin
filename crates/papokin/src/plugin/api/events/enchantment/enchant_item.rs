use papokin_data::Enchantment;
use papokin_data::item_stack::ItemStack;
use papokin_macros::{Event, cancellable};
use std::sync::Arc;

use crate::entity::player::Player;

/// 物品在附魔台中被附魔时触发的事件。
#[cancellable]
#[derive(Event, Clone)]
pub struct EnchantItemEvent {
    /// 为物品附魔的玩家。
    pub player: Arc<Player>,

    /// 正在附魔的物品。
    pub item: ItemStack,

    /// 所选按钮的索引（0、1 或 2）。
    pub option: i32,

    /// 该附魔所消耗的经验等级。
    pub exp_level_cost: i32,

    /// 要应用的附魔及等级列表。
    pub enchantments_to_add: Vec<(&'static Enchantment, i32)>,
}

impl EnchantItemEvent {
    #[must_use]
    pub const fn new(
        player: Arc<Player>,
        item: ItemStack,
        option: i32,
        exp_level_cost: i32,
        enchantments_to_add: Vec<(&'static Enchantment, i32)>,
    ) -> Self {
        Self {
            player,
            item,
            option,
            exp_level_cost,
            enchantments_to_add,
            cancelled: false,
        }
    }
}
