//! 战利品表查询与生成。
//!
//! 本模块暴露服务器的原版战利品表：用
//! [`has_loot_table`] 检查表是否存在，用
//! [`generate_loot`] / [`generate_loot_with_context`] 进行确定性掷点，或用
//! [`fill_inventory`] 把一次掷点散布进容器物品栏（与
//! 原版首次打开战利品箱时相同的洗牌-拆分流程）。
//!
//! 这些都是针对静态数据包表格的纯数据操作：
//! 不会向世界生成任何东西，也不需要玩家上下文，因此
//! 可在任何地方安全调用，包括无头启动阶段。
//!
//! 键名可带可选的 `minecraft:` 命名空间前缀：
//! `"chests/simple_dungeon"` 与 `"minecraft:chests/simple_dungeon"` 会解析
//! 到同一张表。
//!
//! # Examples
//!
//! ```rust,ignore
//! use papokin_plugin_api::loot::{self, LootContext};
//! use papokin_plugin_api::ItemStack;
//!
//! // Deterministic roll of the simple dungeon chest table.
//! let stacks = loot::generate_loot("minecraft:chests/simple_dungeon", 42)?;
//!
//! // Entity-style roll with a looting sword as the tool. The tool handle is
//! // consumed, so pass a freshly built snapshot.
//! let context = LootContext::new()
//!     .killed_by_player(true)
//!     .tool(ItemStack::new("minecraft:diamond_sword", 1));
//! let drops = loot::generate_loot_with_context("minecraft:entities/zombie", 7, context)?;
//! ```

pub use crate::wit::papokin::plugin::loot::{
    LootContext, fill_inventory, generate_loot, generate_loot_with_context, has_loot_table,
};

use crate::ItemStack;

impl LootContext {
    ///返回一个空上下文：无幸运值、非玩家击杀、无爆炸，
    /// 无工具。与 [`generate_loot`] 内部使用的方式等价。
    #[must_use]
    pub const fn new() -> Self {
        Self {
            luck: 0.0,
            killed_by_player: false,
            explosion_radius: None,
            tool: None,
        }
    }

    /// 设置幸运值（原版 `generic.luck` 属性）。
    ///
    /// 注意：服务器的战利品生成器目前不读取 `luck`（尚无任何
    /// 条件尚未消费它）；将其暴露是为了让日后获得
    /// 感知幸运值的条件无需更改 API。
    #[must_use]
    pub const fn luck(mut self, luck: f32) -> Self {
        self.luck = luck;
        self
    }

    /// 设置掉落是否由玩家击杀产生；用于驱动
    /// 实体战利品表所使用的 `killed-by-player` 条件。
    #[must_use]
    pub const fn killed_by_player(mut self, killed_by_player: bool) -> Self {
        self.killed_by_player = killed_by_player;
        self
    }

    /// 设置爆炸半径；用于驱动 `survives-explosion` 条件。
    /// `None` 表示没有爆炸，因此该条件总是通过。
    #[must_use]
    pub const fn explosion_radius(mut self, radius: Option<f32>) -> Self {
        self.explosion_radius = radius;
        self
    }

    /// 设置用于破坏/击杀的工具；其附魔将驱动
    /// 精准采集、剪刀、时运与抢夺的检查。
    ///
    /// 物品堆句柄会被上下文消耗：请传入新构建的或
    /// 一次性快照。
    #[must_use]
    pub fn tool(mut self, tool: ItemStack) -> Self {
        self.tool = Some(tool);
        self
    }
}

impl Default for LootContext {
    fn default() -> Self {
        Self::new()
    }
}
