use papokin_macros::{Event, cancellable};

/// 生成战利品时发生的事件。
#[cancellable]
#[derive(Event, Clone)]
pub struct LootGenerateEvent {
    /// 战利品表标识符。
    pub loot_table: String,
}

impl LootGenerateEvent {
    #[must_use]
    pub const fn new(loot_table: String) -> Self {
        Self {
            loot_table,
            cancelled: false,
        }
    }
}
